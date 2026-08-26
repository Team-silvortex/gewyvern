using System.Text;
using System.Text.Json;

public sealed class RemoteRegistrationClient : IDisposable
{
    private readonly RemoteWireTransport transport;

    public RemoteRegistrationClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteRegistrationDetails> InspectAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken = default)
    {
        RemoteQueryValidation.RequireIdentifier(runtimeId, "runtime ID");
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        var request = new WireQueryRequestEnvelope
        {
            Request = new WireQueryRequest
            {
                Payload = new RuntimeQueryEnvelope
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["runtime.read"],
                    Query = new RuntimeQuery
                    {
                        Kind = "runtime_inspect",
                        RuntimeId = runtimeId,
                    },
                },
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            request,
            RemoteWorkspaceJsonContext.Default.WireQueryRequestEnvelope);
        var response = await transport.PostAsync(
            payload,
            "runtime_registration_inspect",
            cancellationToken).ConfigureAwait(false);
        return DecodeInspect(response, runtimeId);
    }

    public Task<RemoteRegistrationPlan> PlanRegisterAsync(
        RemoteRegistrationIntent intent,
        string principal,
        CancellationToken cancellationToken = default) => PlanAsync(
            RemoteRegistrationMode.Register,
            intent,
            expectedRevision: null,
            principal,
            cancellationToken);

    public Task<RemoteRegistrationPlan> PlanUpdateAsync(
        RemoteRegistrationIntent intent,
        ulong expectedRevision,
        string principal,
        CancellationToken cancellationToken = default) => PlanAsync(
            RemoteRegistrationMode.Update,
            intent,
            expectedRevision,
            principal,
            cancellationToken);

    public async Task<RemoteRegistrationResult> ApplyAsync(
        RemoteRegistrationPlan plan,
        string principal,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(plan);
        ValidateCoordinates(plan.Mode, plan.Intent, plan.ExpectedRevision, principal);
        if (plan.PlannedRevision == 0)
        {
            throw new ArgumentException(
                "runtime registration plan requires a predicted revision");
        }
        var result = await SendAsync(
            plan.CommandId,
            plan.Mode,
            plan.Intent,
            plan.ExpectedRevision,
            principal,
            dryRun: false,
            cancellationToken).ConfigureAwait(false);
        ValidateResult(
            result,
            plan.CommandId,
            plan.Mode,
            plan.Intent,
            plan.ExpectedRevision,
            "applied");
        if (result.Runtime.Revision < plan.PlannedRevision)
        {
            throw new InvalidDataException(
                "remote registration result regressed from its reviewed plan");
        }
        return new RemoteRegistrationResult(
            result.CommandId,
            plan.Mode,
            result.Runtime.Id,
            result.Runtime.Revision);
    }

    public void Dispose() => transport.Dispose();

    public static void VerifyContract()
    {
        var register = new RemoteRegistrationIntent(
            "runtime-new",
            "Runtime New",
            "https://runtime-new.invalid:9443",
            "https://runtime-new.invalid:9444",
            "production",
            "edge-a",
            "capture");
        var plannedRequest = EncodeRequest(
            "gui-registration-plan",
            RemoteRegistrationMode.Register,
            register,
            expectedRevision: null,
            "operator-a",
            dryRun: true);
        using (var document = JsonDocument.Parse(plannedRequest))
        {
            var payload = document.RootElement.GetProperty("request").GetProperty("payload");
            var command = payload.GetProperty("command");
            if (payload.TryGetProperty("expected_revision", out _)
                || payload.GetProperty("dry_run").GetBoolean() is not true
                || payload.GetProperty("confirmation").GetString() != "not_required"
                || payload.GetProperty("capabilities")[0].GetString() != "runtime.register"
                || command.GetProperty("kind").GetString() != "runtime_register"
                || command.GetProperty("runtime_id").GetString() != register.RuntimeId
                || command.GetProperty("sidecar_endpoint").GetString()
                    != register.SidecarEndpoint
                || Encoding.UTF8.GetString(plannedRequest).Contains(
                    "token",
                    StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException(
                    "runtime registration plan request contract drifted");
            }
        }

        var update = register with { Name = "Runtime Updated" };
        var updateRequest = EncodeRequest(
            "gui-registration-update",
            RemoteRegistrationMode.Update,
            update,
            expectedRevision: 7,
            "operator-a",
            dryRun: false);
        using (var document = JsonDocument.Parse(updateRequest))
        {
            var payload = document.RootElement.GetProperty("request").GetProperty("payload");
            if (payload.GetProperty("expected_revision").GetUInt64() != 7
                || payload.GetProperty("dry_run").GetBoolean()
                || payload.GetProperty("confirmation").GetString() != "confirmed"
                || payload.GetProperty("command").GetProperty("kind").GetString()
                    != "runtime_registration_update")
            {
                throw new InvalidDataException(
                    "runtime registration update request contract drifted");
            }
        }

        const string inspectResponse = """
            {"schema_version":1,"response":{"kind":"query","payload":{"kind":"runtime_inspect","revision":8,"runtime":{"id":"runtime-new","name":"Runtime New","endpoint":"https://runtime-new.invalid:9443","sidecar_endpoint":"https://runtime-new.invalid:9444","registered_at_unix_ms":1000,"updated_at_unix_ms":1001,"revision":7,"refresh_count":0,"refresh_status":"never_requested","tags":{"environment":"production","cluster":"edge-a","role":"capture"},"status":{"status_source":"gewyvern","status_fetched_at":null,"status_fetch_error":null,"has_latest_snapshot":false,"snapshot_kind":null,"target_count":null,"has_summary_json":false,"has_analysis_json":false,"has_training_example_json":false,"has_training_dataset_manifest":false,"has_export_json":false,"has_report_json":false,"has_report_html":false,"has_external_sidecar_context":false,"has_external_evidence_chain_enrichment":false,"has_external_diagnostic_opinion":false,"resilience_degraded":false,"resilience_status":null,"resilience_summary":null,"socket_service_status":null,"socket_consecutive_idle_timeouts":null,"socket_total_idle_timeouts":null},"sidecar_status":null,"capabilities":{"source":"","service":"","version":"","latest_snapshot":false,"authenticated_deployment":false,"serve_required":false,"external_sidecar_context":false,"target_path_segment_encoding":"","target_direct_path_chars":"","endpoints":[],"extensions":{}},"capabilities_observed_for_revision":null}}}}
            """;
        var details = DecodeInspect(Encoding.UTF8.GetBytes(inspectResponse), "runtime-new");
        if (details.Revision != 7 || details.Intent != register)
        {
            throw new InvalidDataException(
                "runtime registration inspection projection drifted");
        }

        const string planResponse = """
            {"schema_version":1,"response":{"kind":"command","payload":{"command_id":"gui-registration-plan","status":"planned","runtime":{"id":"runtime-new","name":"Runtime New","endpoint":"https://runtime-new.invalid:9443","sidecar_endpoint":"https://runtime-new.invalid:9444","registered_at_unix_ms":null,"updated_at_unix_ms":null,"revision":8,"refresh_count":0,"refresh_status":"never_requested","tags":{"environment":"production","cluster":"edge-a","role":"capture"},"status":{"status_source":"gewyvern","status_fetched_at":null,"status_fetch_error":null,"has_latest_snapshot":false,"snapshot_kind":null,"target_count":null,"has_summary_json":false,"has_analysis_json":false,"has_training_example_json":false,"has_training_dataset_manifest":false,"has_export_json":false,"has_report_json":false,"has_report_html":false,"has_external_sidecar_context":false,"has_external_evidence_chain_enrichment":false,"has_external_diagnostic_opinion":false,"resilience_degraded":false,"resilience_status":null,"resilience_summary":null,"socket_service_status":null,"socket_consecutive_idle_timeouts":null,"socket_total_idle_timeouts":null},"sidecar_status":null,"capabilities":{"source":"","service":"","version":"","latest_snapshot":false,"authenticated_deployment":false,"serve_required":false,"external_sidecar_context":false,"target_path_segment_encoding":"","target_direct_path_chars":"","endpoints":[],"extensions":{}},"capabilities_observed_for_revision":null},"events":[{"kind":"runtime_registered","runtime_id":"runtime-new","revision":8,"command_id":"gui-registration-plan"}]}}}
            """;
        var result = DecodeCommand(Encoding.UTF8.GetBytes(planResponse));
        ValidateResult(
            result,
            "gui-registration-plan",
            RemoteRegistrationMode.Register,
            register,
            expectedRevision: null,
            "planned");
        try
        {
            ValidateCoordinates(
                RemoteRegistrationMode.Update,
                register with { Endpoint = "bad\nendpoint" },
                expectedRevision: 7,
                "operator-a");
            throw new InvalidDataException(
                "runtime registration accepted a control character");
        }
        catch (ArgumentException)
        {
        }
    }

    private async Task<RemoteRegistrationPlan> PlanAsync(
        RemoteRegistrationMode mode,
        RemoteRegistrationIntent intent,
        ulong? expectedRevision,
        string principal,
        CancellationToken cancellationToken)
    {
        ValidateCoordinates(mode, intent, expectedRevision, principal);
        var commandId = $"gui-registration-{Guid.NewGuid():N}";
        var result = await SendAsync(
            commandId,
            mode,
            intent,
            expectedRevision,
            principal,
            dryRun: true,
            cancellationToken).ConfigureAwait(false);
        ValidateResult(
            result,
            commandId,
            mode,
            intent,
            expectedRevision,
            "planned");
        return new RemoteRegistrationPlan(
            commandId,
            mode,
            intent,
            expectedRevision,
            result.Runtime.Revision);
    }

    private async Task<WireRegistrationCommandResult> SendAsync(
        string commandId,
        RemoteRegistrationMode mode,
        RemoteRegistrationIntent intent,
        ulong? expectedRevision,
        string principal,
        bool dryRun,
        CancellationToken cancellationToken)
    {
        var payload = EncodeRequest(
            commandId,
            mode,
            intent,
            expectedRevision,
            principal,
            dryRun);
        var response = await transport.PostAsync(
            payload,
            dryRun ? "runtime_registration_plan" : "runtime_registration_apply",
            cancellationToken).ConfigureAwait(false);
        return DecodeCommand(response);
    }

    private static byte[] EncodeRequest(
        string commandId,
        RemoteRegistrationMode mode,
        RemoteRegistrationIntent intent,
        ulong? expectedRevision,
        string principal,
        bool dryRun)
    {
        ValidateCoordinates(mode, intent, expectedRevision, principal);
        RemoteQueryValidation.RequireIdentifier(commandId, "command ID");
        var request = new WireRegistrationCommandRequestEnvelope
        {
            Request = new WireRegistrationCommandRequest
            {
                Payload = new WireRegistrationCommandEnvelope
                {
                    CommandId = commandId,
                    IdempotencyKey = commandId,
                    ExpectedRevision = expectedRevision,
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["runtime.register"],
                    Confirmation = dryRun ? "not_required" : "confirmed",
                    DryRun = dryRun,
                    Command = new WireRegistrationCommand
                    {
                        Kind = mode == RemoteRegistrationMode.Register
                            ? "runtime_register"
                            : "runtime_registration_update",
                        RuntimeId = intent.RuntimeId,
                        Name = intent.Name,
                        Endpoint = intent.Endpoint,
                        SidecarEndpoint = intent.SidecarEndpoint,
                        Tags = new RuntimeTags
                        {
                            Environment = intent.Environment,
                            Cluster = intent.Cluster,
                            Role = intent.Role,
                        },
                    },
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            request,
            RemoteRegistrationJsonContext.Default
                .WireRegistrationCommandRequestEnvelope);
    }

    private static RemoteRegistrationDetails DecodeInspect(
        ReadOnlySpan<byte> payload,
        string expectedRuntimeId)
    {
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteRegistrationJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException(
                    "remote registration inspection response is empty");
            ValidateEnvelope(envelope, "query", "inspection");
            var result = envelope.Response.Payload.Deserialize(
                RemoteRegistrationJsonContext.Default.RuntimeInspectQueryResult)
                ?? throw new InvalidDataException(
                    "remote registration inspection payload is empty");
            if (result.Kind != "runtime_inspect" || result.Runtime is null)
            {
                throw new InvalidDataException(
                    "remote registration inspection returned an unexpected query");
            }
            RemoteWorkspaceCodec.ValidateRuntime(result.Runtime, expectedRuntimeId);
            if (result.Runtime.Revision == 0
                || result.Runtime.Revision > result.Revision)
            {
                throw new InvalidDataException(
                    "remote registration inspection revision is inconsistent");
            }
            return new RemoteRegistrationDetails(
                IntentFrom(result.Runtime),
                result.Runtime.Revision);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException(
                "remote registration inspection JSON is invalid",
                error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException(
                "remote registration inspection field type is invalid",
                error);
        }
    }

    private static WireRegistrationCommandResult DecodeCommand(
        ReadOnlySpan<byte> payload)
    {
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteRegistrationJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException(
                    "remote registration response is empty");
            ValidateEnvelope(envelope, "command", "command");
            return envelope.Response.Payload.Deserialize(
                RemoteRegistrationJsonContext.Default.WireRegistrationCommandResult)
                ?? throw new InvalidDataException(
                    "remote registration command payload is empty");
        }
        catch (JsonException error)
        {
            throw new InvalidDataException(
                "remote registration response JSON is invalid",
                error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException(
                "remote registration response field type is invalid",
                error);
        }
    }

    private static void ValidateEnvelope(
        WireResponseEnvelope envelope,
        string expectedKind,
        string operation)
    {
        if (envelope.SchemaVersion != 1)
        {
            throw new InvalidDataException(
                $"unsupported remote registration {operation} schema");
        }
        if (envelope.Response.Kind == "error")
        {
            throw new RemoteRegistrationException(
                RequireResponseString(envelope.Response.Payload, "code"),
                RequireResponseString(envelope.Response.Payload, "message"));
        }
        if (envelope.Response.Kind != expectedKind)
        {
            throw new InvalidDataException(
                $"remote registration {operation} returned an unexpected response kind");
        }
    }

    private static void ValidateResult(
        WireRegistrationCommandResult result,
        string commandId,
        RemoteRegistrationMode mode,
        RemoteRegistrationIntent intent,
        ulong? expectedRevision,
        string expectedStatus)
    {
        if (result.CommandId != commandId || result.Status != expectedStatus)
        {
            throw new InvalidDataException(
                "remote registration response changed the reviewed command identity");
        }
        RemoteWorkspaceCodec.ValidateRuntime(result.Runtime, intent.RuntimeId);
        var projected = IntentFrom(result.Runtime);
        if (projected != intent
            || result.Runtime.Revision == 0
            || expectedRevision is { } revision
                && result.Runtime.Revision <= revision)
        {
            throw new InvalidDataException(
                "remote registration response changed the reviewed intent");
        }
        var expectedEventKind = mode == RemoteRegistrationMode.Register
            ? "runtime_registered"
            : "runtime_registration_updated";
        if (result.Events is not [var domainEvent]
            || domainEvent.Kind != expectedEventKind
            || domainEvent.CommandId != commandId
            || domainEvent.RuntimeId != intent.RuntimeId
            || domainEvent.Revision != result.Runtime.Revision)
        {
            throw new InvalidDataException(
                "remote registration response event proof is invalid");
        }
    }

    private static RemoteRegistrationIntent IntentFrom(WireRuntimeProjection runtime) => new(
        runtime.Id,
        runtime.Name,
        runtime.Endpoint,
        runtime.SidecarEndpoint,
        runtime.Tags.Environment,
        runtime.Tags.Cluster,
        runtime.Tags.Role);

    private static void ValidateCoordinates(
        RemoteRegistrationMode mode,
        RemoteRegistrationIntent intent,
        ulong? expectedRevision,
        string principal)
    {
        ArgumentNullException.ThrowIfNull(intent);
        if (!Enum.IsDefined(mode))
        {
            throw new ArgumentOutOfRangeException(nameof(mode));
        }
        RemoteQueryValidation.RequireIdentifier(intent.RuntimeId, "runtime ID");
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        RequireField(intent.Name, 128, "runtime name");
        RequireField(intent.Endpoint, 2048, "runtime endpoint");
        RequireOptionalField(intent.SidecarEndpoint, 2048, "sidecar endpoint");
        RequireOptionalField(intent.Environment, 128, "environment tag");
        RequireOptionalField(intent.Cluster, 128, "cluster tag");
        RequireOptionalField(intent.Role, 128, "role tag");
        if (mode == RemoteRegistrationMode.Register && expectedRevision is not null)
        {
            throw new ArgumentException(
                "new runtime registration does not accept a revision fence");
        }
        if (mode == RemoteRegistrationMode.Update
            && expectedRevision is not > 0)
        {
            throw new ArgumentException(
                "runtime registration update requires a revision fence");
        }
    }

    private static void RequireField(string value, int maxBytes, string label)
    {
        if (string.IsNullOrEmpty(value)
            || value != value.Trim()
            || value.Any(char.IsControl)
            || Encoding.UTF8.GetByteCount(value) > maxBytes)
        {
            throw new ArgumentException($"invalid {label}");
        }
    }

    private static void RequireOptionalField(
        string? value,
        int maxBytes,
        string label)
    {
        if (value is not null)
        {
            RequireField(value, maxBytes, label);
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
                $"remote registration response field '{property}' is invalid");
        }
        return value;
    }
}
