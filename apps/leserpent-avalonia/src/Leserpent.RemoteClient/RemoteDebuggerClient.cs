using System.Text;
using System.Text.Json;

public sealed class RemoteDebuggerClient : IDisposable
{
    public const int MaxSourceBytes = 64 * 1024;
    public const int MaxSessions = 32;
    public const ulong MinTimeoutMs = 100;
    public const ulong MaxTimeoutMs = 5 * 60 * 1000;
    private const ulong MaxProjectedDeadlineMs = 24 * 60 * 60 * 1000;
    private const int MaxFrames = 64;
    private const int MaxDisplayBytes = 512;
    private static readonly HashSet<string> EffectKinds =
    [
        "runtime_list", "runtime_inspect", "runtime_history", "runtime_logs",
        "runtime_refresh", "runtime_capabilities_refresh", "runtime_deploy",
        "debugger_cancel", "ui_activate", "ui_focus", "ui_navigate_focus",
        "ui_scroll_into_view", "ui_assert_visible", "ui_assert_hidden",
        "ui_wait_hidden", "ui_assert_realized", "ui_wait_realized",
        "ui_wait_visible", "ui_wait_enabled", "ui_wait_focused",
        "ui_assert_focused", "ui_wait_unfocused", "ui_assert_unfocused",
        "ui_assert_enabled", "ui_assert_disabled", "ui_wait_disabled",
        "ui_assert_child_count", "ui_wait_child_count", "ui_open_window",
        "ui_close_window", "ui_assert_window_open", "ui_wait_window_open",
        "ui_assert_window_closed", "ui_wait_window_closed", "ui_set_selection",
        "ui_assert_selection", "ui_wait_selection", "ui_assert_text",
        "ui_wait_text", "ui_assert_automation_id", "ui_wait_automation_id",
        "ui_assert_node_kind", "ui_wait_node_kind", "ui_assert_action_kind",
        "ui_wait_action_kind", "ui_assert_action_label", "ui_wait_action_label",
        "ui_assert_action_available", "ui_wait_action_available",
        "ui_assert_action_unavailable_reason", "ui_wait_action_unavailable_reason",
        "ui_submit_form", "ui_cancel_form", "ui_set_form_value",
        "ui_assert_form_value", "ui_wait_form_value", "ui_assert_form_field",
        "ui_assert_form_field_input_kind", "ui_assert_form_field_required",
        "ui_assert_form_field_max_length", "ui_assert_form_field_placeholder",
        "ui_wait_form_field", "ui_wait_form_field_input_kind",
        "ui_wait_form_field_required", "ui_wait_form_field_max_length",
        "ui_wait_form_field_placeholder", "ui_assert_accessible_name",
        "ui_wait_accessible_name", "ui_assert_accessible_description",
        "ui_wait_accessible_description",
    ];
    private readonly object planAuthority = new();
    private readonly RemoteWireTransport transport;

    public RemoteDebuggerClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteDebuggerSession> StartAsync(
        string sessionId,
        string source,
        ulong? expectedRevision,
        ulong timeoutMs,
        string principal,
        CancellationToken cancellationToken = default)
    {
        ValidateStart(sessionId, source, expectedRevision, timeoutMs, principal);
        var payload = EncodeStart(
            sessionId,
            source,
            expectedRevision,
            timeoutMs,
            principal);
        var response = await transport.PostAsync(
            payload,
            "debugger_session_start",
            cancellationToken).ConfigureAwait(false);
        var envelope = DecodeEnvelope(response, "debugger_session_started", "session start");
        var result = Deserialize(
            envelope.Response.Payload,
            RemoteDebuggerJsonContext.Default.WireDebuggerSessionResponse,
            "debugger session start payload");
        var session = DecodeSession(result.Session, sessionId);
        if (session.Projection.State != RemoteDebuggerState.WaitingEffect
            || session.Projection.PendingEffect is null
            || session.Projection.Revision != (expectedRevision ?? 1))
        {
            throw new InvalidDataException(
                "remote debugger did not return the requested suspended session");
        }
        return session;
    }

    public async Task<IReadOnlyList<RemoteDebuggerSession>> ListAsync(
        string principal,
        string? sessionId = null,
        CancellationToken cancellationToken = default)
    {
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        if (sessionId is not null)
        {
            RemoteQueryValidation.RequireIdentifier(sessionId, "debugger session ID");
        }
        var request = new WireDebuggerSessionsRequestEnvelope
        {
            Request = new WireDebuggerSessionsRequest
            {
                Payload = new WireDebuggerSessionsPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["debugger.control"],
                    SessionId = sessionId,
                },
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            request,
            RemoteDebuggerJsonContext.Default.WireDebuggerSessionsRequestEnvelope);
        var response = await transport.PostAsync(
            payload,
            "debugger_sessions",
            cancellationToken).ConfigureAwait(false);
        var envelope = DecodeEnvelope(response, "debugger_sessions", "session query");
        var result = Deserialize(
            envelope.Response.Payload,
            RemoteDebuggerJsonContext.Default.WireDebuggerSessionsResponse,
            "debugger sessions payload");
        if (result.Sessions.Count > MaxSessions)
        {
            throw new InvalidDataException("remote debugger session count is unbounded");
        }
        var sessions = result.Sessions
            .Select(view => DecodeSession(view, sessionId))
            .ToArray();
        if (sessions.Select(candidate => candidate.Projection.SessionId)
            .Distinct(StringComparer.Ordinal).Count() != sessions.Length
            || sessionId is not null && sessions.Length != 1)
        {
            throw new InvalidDataException("remote debugger session identities are inconsistent");
        }
        return sessions;
    }

    public async Task<RemoteDebuggerCancelPlan> PlanCancelAsync(
        RemoteDebuggerSession session,
        string principal,
        CancellationToken cancellationToken = default)
    {
        ValidateCancellable(session, principal);
        var commandId = $"gui-debugger-cancel-{Guid.NewGuid():N}";
        var response = await SendCancelAsync(
            commandId,
            session.Projection.SessionId,
            session.Projection.Revision,
            principal,
            dryRun: true,
            cancellationToken).ConfigureAwait(false);
        var reviewed = ValidateCancelResponse(
            response,
            commandId,
            session,
            expectedStatus: "planned");
        if (response.AuditedAtMs is not null)
        {
            throw new InvalidDataException("debugger dry-run unexpectedly wrote an audit record");
        }
        return new RemoteDebuggerCancelPlan(
            commandId,
            session.Projection.SessionId,
            session.Projection.Revision,
            reviewed,
            planAuthority,
            principal);
    }

    public async Task<RemoteDebuggerCancelResult> ApplyCancelAsync(
        RemoteDebuggerCancelPlan plan,
        string principal,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ValidateIssuedPlan(plan, planAuthority, principal);
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        RemoteQueryValidation.RequireIdentifier(plan.CommandId, "command ID");
        ValidateCancellable(plan.ReviewedSession, principal);
        if (plan.SessionId != plan.ReviewedSession.Projection.SessionId
            || plan.ExpectedRevision != plan.ReviewedSession.Projection.Revision
            || plan.ExpectedRevision == ulong.MaxValue)
        {
            throw new ArgumentException("debugger cancel plan coordinates are invalid");
        }
        var response = await SendCancelAsync(
            plan.CommandId,
            plan.SessionId,
            plan.ExpectedRevision,
            principal,
            dryRun: false,
            cancellationToken).ConfigureAwait(false);
        var applied = DecodeSession(response.Session, plan.SessionId);
        if (response.CommandId != plan.CommandId
            || response.Status != "applied"
            || response.AuditedAtMs is not > 0
            || applied.Projection.State != RemoteDebuggerState.Cancelled
            || applied.Projection.Revision != plan.ExpectedRevision + 1
            || applied.Projection.PendingEffect is not null
            || applied.Projection.Fault is not null)
        {
            throw new InvalidDataException(
                "remote debugger cancellation changed its reviewed coordinates");
        }
        return new RemoteDebuggerCancelResult(
            response.CommandId,
            applied,
            response.AuditedAtMs.Value);
    }

    public void Dispose() => transport.Dispose();

    public static void VerifyContract()
    {
        var authority = new object();
        var ownershipFixture = new RemoteDebuggerSession(
            new RemoteDebuggerProjection(
                1,
                "session-ownership",
                RemoteDebuggerState.WaitingEffect,
                1,
                1,
                100,
                new RemoteDebuggerPendingEffect("effect-ownership", "runtime_list", null),
                [],
                null),
            null!);
        var forgedPlan = new RemoteDebuggerCancelPlan(
            "command-ownership",
            "session-ownership",
            1,
            ownershipFixture);
        var issuedPlan = new RemoteDebuggerCancelPlan(
            "command-ownership",
            "session-ownership",
            1,
            ownershipFixture,
            authority,
            "operator-ownership");
        if (forgedPlan.Authority is not null
            || forgedPlan.Principal is not null
            || !ReferenceEquals(issuedPlan.Authority, authority)
            || issuedPlan.Principal != "operator-ownership")
        {
            throw new InvalidDataException("debugger cancel plan authority drifted");
        }
        try
        {
            ValidateIssuedPlan(forgedPlan, authority, "operator-ownership");
            throw new InvalidDataException("forged debugger cancel plan was accepted");
        }
        catch (ArgumentException)
        {
        }
        ValidateIssuedPlan(issuedPlan, authority, "operator-ownership");
        try
        {
            ValidateIssuedPlan(issuedPlan, authority, "operator-drifted");
            throw new InvalidDataException("debugger cancel principal drift was accepted");
        }
        catch (ArgumentException)
        {
        }

        var start = EncodeStart(
            "session-a",
            "fn main() = runtime.list()",
            7,
            30_000,
            "operator-a");
        using (var document = JsonDocument.Parse(start))
        {
            var request = document.RootElement.GetProperty("request");
            var payload = request.GetProperty("payload");
            if (request.GetProperty("kind").GetString() != "debugger_session_start"
                || payload.GetProperty("session_id").GetString() != "session-a"
                || payload.GetProperty("expected_revision").GetUInt64() != 7
                || payload.GetProperty("timeout_ms").GetUInt64() != 30_000
                || payload.GetProperty("capabilities")[0].GetString() != "debugger.control"
                || Encoding.UTF8.GetString(start).Contains(
                    "token",
                    StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException("debugger start request contract drifted");
            }
        }

        const string response = """
            {"schema_version":1,"response":{"kind":"debugger_session_started","payload":{"session":{"projection":{"revision":7,"session_id":"session-a","state":"waiting_effect","program_counter":1,"fuel_remaining":9999,"deadline_remaining_ms":29999,"pending_effect":{"effect_id":"effect-a","kind":"runtime_list","runtime_id":null},"frames":[],"fault":null},"document":{"schema_version":1,"revision":7,"root":{"id":"debug-session-a","kind":"debugger_workspace","runtime_id":null,"debugger_session_id":"session-a","text":null,"accessibility":{"label":{"key":"debugger.workspace","fallback":"Leselang debugger workspace"},"description":null},"action":null,"children":[{"id":"debug-session-a-cancel","kind":"action","runtime_id":null,"debugger_session_id":null,"text":{"key":"debugger.cancel","fallback":"Cancel effect"},"accessibility":{"label":{"key":"debugger.cancel","fallback":"Cancel pending debugger effect"},"description":null},"action":{"kind":"debugger_cancel","session_id":"session-a"},"children":[]}]}}}}}}
            """;
        var envelope = DecodeEnvelope(
            Encoding.UTF8.GetBytes(response),
            "debugger_session_started",
            "contract");
        var decoded = Deserialize(
            envelope.Response.Payload,
            RemoteDebuggerJsonContext.Default.WireDebuggerSessionResponse,
            "contract payload");
        var session = DecodeSession(decoded.Session, "session-a");
        if (session.Projection.PendingEffect?.Kind != "runtime_list"
            || session.Document.Root.Action is not null)
        {
            throw new InvalidDataException("debugger response projection contract drifted");
        }

        using var mutable = JsonDocument.Parse(response);
        var forged = mutable.RootElement.GetRawText().Replace(
            "\"document\":",
            "\"source\":\"secret\",\"document\":",
            StringComparison.Ordinal);
        try
        {
            var forgedEnvelope = DecodeEnvelope(
                Encoding.UTF8.GetBytes(forged),
                "debugger_session_started",
                "contract");
            _ = Deserialize(
                forgedEnvelope.Response.Payload,
                RemoteDebuggerJsonContext.Default.WireDebuggerSessionResponse,
                "contract payload");
            throw new InvalidDataException(
                "debugger response accepted an unknown source field");
        }
        catch (InvalidDataException error) when (error.InnerException is JsonException)
        {
        }
    }

    private async Task<WireDebuggerCancelResponse> SendCancelAsync(
        string commandId,
        string sessionId,
        ulong expectedRevision,
        string principal,
        bool dryRun,
        CancellationToken cancellationToken)
    {
        var payload = EncodeCancel(
            commandId,
            sessionId,
            expectedRevision,
            principal,
            dryRun);
        var response = await transport.PostAsync(
            payload,
            dryRun ? "debugger_cancel_plan" : "debugger_cancel_apply",
            cancellationToken).ConfigureAwait(false);
        var envelope = DecodeEnvelope(response, "debugger_cancelled", "cancellation");
        return Deserialize(
            envelope.Response.Payload,
            RemoteDebuggerJsonContext.Default.WireDebuggerCancelResponse,
            "debugger cancellation payload");
    }

    private static byte[] EncodeStart(
        string sessionId,
        string source,
        ulong? expectedRevision,
        ulong timeoutMs,
        string principal)
    {
        ValidateStart(sessionId, source, expectedRevision, timeoutMs, principal);
        var request = new WireDebuggerSessionStartRequestEnvelope
        {
            Request = new WireDebuggerSessionStartRequest
            {
                Payload = new WireDebuggerSessionStartPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["debugger.control"],
                    SessionId = sessionId,
                    Source = source,
                    ExpectedRevision = expectedRevision,
                    TimeoutMs = timeoutMs,
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            request,
            RemoteDebuggerJsonContext.Default.WireDebuggerSessionStartRequestEnvelope);
    }

    private static byte[] EncodeCancel(
        string commandId,
        string sessionId,
        ulong expectedRevision,
        string principal,
        bool dryRun)
    {
        RemoteQueryValidation.RequireIdentifier(commandId, "command ID");
        RemoteQueryValidation.RequireIdentifier(sessionId, "debugger session ID");
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        if (expectedRevision == 0)
        {
            throw new ArgumentException("debugger cancellation requires a revision fence");
        }
        var request = new WireDebuggerCommandRequestEnvelope
        {
            Request = new WireDebuggerCommandRequest
            {
                Payload = new WireDebuggerCommandEnvelope
                {
                    CommandId = commandId,
                    IdempotencyKey = commandId,
                    ExpectedRevision = expectedRevision,
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["debugger.control"],
                    Confirmation = dryRun ? "not_required" : "confirmed",
                    DryRun = dryRun,
                    Command = new WireDebuggerCancelCommand { SessionId = sessionId },
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            request,
            RemoteDebuggerJsonContext.Default.WireDebuggerCommandRequestEnvelope);
    }

    private static WireResponseEnvelope DecodeEnvelope(
        ReadOnlySpan<byte> payload,
        string expectedKind,
        string operation)
    {
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteDebuggerJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException($"remote debugger {operation} response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException(
                    $"unsupported remote debugger {operation} schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw new RemoteDebuggerException(
                    RequireResponseString(envelope.Response.Payload, "code"),
                    RequireResponseString(envelope.Response.Payload, "message"));
            }
            if (envelope.Response.Kind != expectedKind)
            {
                throw new InvalidDataException(
                    $"remote debugger {operation} returned an unexpected response kind");
            }
            return envelope;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException(
                $"remote debugger {operation} response JSON is invalid",
                error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException(
                $"remote debugger {operation} response field type is invalid",
                error);
        }
    }

    private static T Deserialize<T>(
        JsonElement payload,
        System.Text.Json.Serialization.Metadata.JsonTypeInfo<T> type,
        string label)
    {
        try
        {
            return payload.Deserialize(type)
                ?? throw new InvalidDataException($"remote {label} is empty");
        }
        catch (JsonException error)
        {
            throw new InvalidDataException($"remote {label} is invalid", error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException($"remote {label} field type is invalid", error);
        }
    }

    private static RemoteDebuggerSession DecodeSession(
        WireDebuggerSessionView view,
        string? expectedSessionId)
    {
        var projection = DecodeProjection(view.Projection, expectedSessionId);
        UiDocument document;
        try
        {
            document = view.Document.Deserialize(RendererJsonContext.Default.UiDocument)
                ?? throw new InvalidDataException("remote debugger document is empty");
            var semantic = new SemanticRenderer();
            semantic.Mount(document);
            document = semantic.Document;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("remote debugger document JSON is invalid", error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException("remote debugger document is invalid", error);
        }
        ValidateDocument(document, projection);
        return new RemoteDebuggerSession(projection, document);
    }

    private static RemoteDebuggerProjection DecodeProjection(
        WireDebuggerProjection projection,
        string? expectedSessionId)
    {
        RemoteQueryValidation.RequireIdentifier(
            projection.SessionId,
            "debugger session ID");
        if (expectedSessionId is not null && projection.SessionId != expectedSessionId)
        {
            throw new InvalidDataException("remote debugger changed the session identity");
        }
        var state = projection.State switch
        {
            "running" => RemoteDebuggerState.Running,
            "waiting_effect" => RemoteDebuggerState.WaitingEffect,
            "yielded" => RemoteDebuggerState.Yielded,
            "completed" => RemoteDebuggerState.Completed,
            "failed" => RemoteDebuggerState.Failed,
            "cancelled" => RemoteDebuggerState.Cancelled,
            _ => throw new InvalidDataException("remote debugger state is invalid"),
        };
        if (projection.Revision == 0
            || projection.DeadlineRemainingMs is > MaxProjectedDeadlineMs
            || projection.Frames.Count > MaxFrames
            || (state == RemoteDebuggerState.WaitingEffect) != (projection.PendingEffect is not null)
            || (state == RemoteDebuggerState.Failed) != (projection.Fault is not null))
        {
            throw new InvalidDataException("remote debugger projection is inconsistent");
        }
        RemoteDebuggerPendingEffect? pending = null;
        if (projection.PendingEffect is { } effect)
        {
            RemoteQueryValidation.RequireIdentifier(effect.EffectId, "debugger effect ID");
            if (!EffectKinds.Contains(effect.Kind))
            {
                throw new InvalidDataException("remote debugger effect kind is invalid");
            }
            if (effect.RuntimeId is not null)
            {
                RemoteQueryValidation.RequireIdentifier(effect.RuntimeId, "runtime ID");
            }
            var runtimeBound = effect.Kind is "runtime_inspect" or "runtime_history"
                or "runtime_logs" or "runtime_refresh"
                or "runtime_capabilities_refresh" or "runtime_deploy";
            if (runtimeBound != (effect.RuntimeId is not null))
            {
                throw new InvalidDataException("remote debugger effect binding is invalid");
            }
            pending = new RemoteDebuggerPendingEffect(
                effect.EffectId,
                effect.Kind,
                effect.RuntimeId);
        }
        var frames = projection.Frames.Select(frame =>
        {
            RemoteQueryValidation.RequireIdentifier(frame.FrameId, "debugger frame ID");
            RequireDisplay(frame.Display, "debugger frame");
            return new RemoteDebuggerFrame(frame.FrameId, frame.Instruction, frame.Display);
        }).ToArray();
        if (frames.Select(frame => frame.FrameId)
            .Distinct(StringComparer.Ordinal).Count() != frames.Length)
        {
            throw new InvalidDataException("remote debugger frame identities are duplicated");
        }
        RemoteDebuggerFault? fault = null;
        if (projection.Fault is { } wireFault)
        {
            RemoteQueryValidation.RequireIdentifier(wireFault.Code, "debugger fault code");
            RequireDisplay(wireFault.Display, "debugger fault");
            fault = new RemoteDebuggerFault(wireFault.Code, wireFault.Display);
        }
        return new RemoteDebuggerProjection(
            projection.Revision,
            projection.SessionId,
            state,
            projection.ProgramCounter,
            projection.FuelRemaining,
            projection.DeadlineRemainingMs,
            pending,
            frames,
            fault);
    }

    private static void ValidateDocument(
        UiDocument document,
        RemoteDebuggerProjection projection)
    {
        if (document.SchemaVersion != 1
            || document.Revision != projection.Revision
            || document.Root.Kind != UiNodeKind.DebuggerWorkspace
            || document.Root.DebuggerSessionId != projection.SessionId
            || document.Root.RuntimeId is not null)
        {
            throw new InvalidDataException(
                "remote debugger document does not match its projection");
        }
        var actions = Nodes(document.Root)
            .Where(node => node.Action is not null)
            .ToArray();
        var expectedActions = projection.State == RemoteDebuggerState.WaitingEffect ? 1 : 0;
        if (actions.Length != expectedActions
            || actions.Any(node => node.Action is not
                {
                    Kind: ActionKind.DebuggerCancel,
                    RuntimeId: null,
                    Form: null,
                } action || action.SessionId != projection.SessionId))
        {
            throw new InvalidDataException(
                "remote debugger document action binding is inconsistent");
        }
    }

    private static IEnumerable<UiNode> Nodes(UiNode root)
    {
        var stack = new Stack<UiNode>();
        stack.Push(root);
        while (stack.Count > 0)
        {
            var node = stack.Pop();
            yield return node;
            for (var index = node.Children.Count - 1; index >= 0; index--)
            {
                stack.Push(node.Children[index]);
            }
        }
    }

    private static RemoteDebuggerSession ValidateCancelResponse(
        WireDebuggerCancelResponse response,
        string commandId,
        RemoteDebuggerSession expected,
        string expectedStatus)
    {
        var session = DecodeSession(response.Session, expected.Projection.SessionId);
        if (response.CommandId != commandId
            || response.Status != expectedStatus
            || !SameWaitingProjection(session.Projection, expected.Projection))
        {
            throw new InvalidDataException(
                "remote debugger dry-run changed its reviewed coordinates");
        }
        return session;
    }

    private static bool SameWaitingProjection(
        RemoteDebuggerProjection left,
        RemoteDebuggerProjection right) =>
        left.Revision == right.Revision
        && left.SessionId == right.SessionId
        && left.State == RemoteDebuggerState.WaitingEffect
        && right.State == RemoteDebuggerState.WaitingEffect
        && left.ProgramCounter == right.ProgramCounter
        && left.FuelRemaining == right.FuelRemaining
        && left.DeadlineRemainingMs == right.DeadlineRemainingMs
        && left.PendingEffect == right.PendingEffect
        && left.Frames.SequenceEqual(right.Frames)
        && left.Fault == right.Fault;

    private static void ValidateCancellable(
        RemoteDebuggerSession session,
        string principal)
    {
        ArgumentNullException.ThrowIfNull(session);
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        RemoteQueryValidation.RequireIdentifier(
            session.Projection.SessionId,
            "debugger session ID");
        ValidateDocument(session.Document, session.Projection);
        if (session.Projection.State != RemoteDebuggerState.WaitingEffect
            || session.Projection.PendingEffect is null
            || session.Projection.Revision == 0)
        {
            throw new ArgumentException("debugger session is not cancellable");
        }
    }

    private static void ValidateIssuedPlan(
        RemoteDebuggerCancelPlan plan,
        object authority,
        string principal)
    {
        if (!ReferenceEquals(plan.Authority, authority))
        {
            throw new ArgumentException(
                "debugger cancel plan was not issued by this client");
        }
        if (!string.Equals(plan.Principal, principal, StringComparison.Ordinal))
        {
            throw new ArgumentException(
                "debugger cancel plan principal changed after review");
        }
    }

    private static void ValidateStart(
        string sessionId,
        string source,
        ulong? expectedRevision,
        ulong timeoutMs,
        string principal)
    {
        RemoteQueryValidation.RequireIdentifier(sessionId, "debugger session ID");
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        if (string.IsNullOrEmpty(source)
            || source.Contains('\0')
            || Encoding.UTF8.GetByteCount(source) > MaxSourceBytes
            || expectedRevision == 0
            || timeoutMs is < MinTimeoutMs or > MaxTimeoutMs)
        {
            throw new ArgumentException("invalid debugger session start request");
        }
    }

    private static void RequireDisplay(string value, string label)
    {
        if (Encoding.UTF8.GetByteCount(value) > MaxDisplayBytes
            || value.Any(char.IsControl))
        {
            throw new InvalidDataException($"remote {label} display is invalid");
        }
    }

    private static string RequireResponseString(JsonElement payload, string property)
    {
        var value = payload.GetProperty(property).GetString();
        if (string.IsNullOrWhiteSpace(value)
            || value.Length > 4096
            || value.Any(char.IsControl))
        {
            throw new InvalidDataException(
                $"remote debugger response field '{property}' is invalid");
        }
        return value;
    }
}
