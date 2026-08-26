using System.Globalization;
using System.Text;
using System.Text.Json;

public sealed class RemoteOrchestraClient : IDisposable
{
    public const ushort DefaultPageSize = 32;
    public const ushort MaxPageSize = 64;
    public const uint MaxOffset = 10_000;
    private const string OrchestraCapability = "orchestra.write";
    private static readonly HashSet<string> Outcomes =
    [
        "queued",
        "running",
        "succeeded",
        "degraded",
        "failed",
        "cancelled",
        "ok",
    ];

    private readonly RemoteWireTransport transport;

    public RemoteOrchestraClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteOrchestraHistoryPage> LoadRunsAsync(
        string? runtimeId,
        uint offset,
        ushort limit,
        string principal,
        CancellationToken cancellationToken = default)
    {
        RequireOptionalIdentifier(runtimeId, "runtime ID");
        RequirePage(offset, limit);
        RequireIdentifier(principal, "principal");
        var payload = EncodeHistoryRequest(runtimeId, null, offset, limit, principal);
        var response = await transport.PostAsync(
            payload,
            "Orchestra history",
            cancellationToken).ConfigureAwait(false);
        return DecodeHistoryResponse(response, runtimeId, null, offset, limit);
    }

    public async Task<RemoteOrchestraHistoryPage> LoadEventsAsync(
        string runtimeId,
        string runId,
        uint offset,
        ushort limit,
        string principal,
        CancellationToken cancellationToken = default)
    {
        RequireIdentifier(runtimeId, "runtime ID");
        RequireIdentifier(runId, "run ID");
        RequirePage(offset, limit);
        RequireIdentifier(principal, "principal");
        var payload = EncodeHistoryRequest(runtimeId, runId, offset, limit, principal);
        var response = await transport.PostAsync(
            payload,
            "Orchestra event history",
            cancellationToken).ConfigureAwait(false);
        return DecodeHistoryResponse(response, runtimeId, runId, offset, limit);
    }

    public async Task<RemoteOrchestraDeleteReceipt> DeleteRuntimeHistoryAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken = default)
    {
        RequireIdentifier(runtimeId, "runtime ID");
        RequireIdentifier(principal, "principal");
        var commandId = $"gui-orchestra-{Guid.NewGuid():N}";
        var payload = EncodeDeleteRequest(runtimeId, principal, commandId);
        var response = await transport.PostAsync(
            payload,
            "Orchestra history cleanup",
            cancellationToken).ConfigureAwait(false);
        return DecodeDeleteResponse(response, runtimeId, commandId);
    }

    public void Dispose() => transport.Dispose();

    public static void VerifyContract()
    {
        var history = EncodeHistoryRequest(
            "runtime-alpha",
            "orun-alpha",
            32,
            16,
            "operator-a");
        using (var document = JsonDocument.Parse(history))
        {
            var request = document.RootElement.GetProperty("request");
            var payload = request.GetProperty("payload");
            if (request.GetProperty("kind").GetString() != "orchestra_history"
                || payload.GetProperty("capabilities")[0].GetString()
                    != OrchestraCapability
                || payload.GetProperty("runtime_id").GetString() != "runtime-alpha"
                || payload.GetProperty("run_id").GetString() != "orun-alpha"
                || payload.GetProperty("offset").GetUInt32() != 32
                || payload.GetProperty("limit").GetUInt16() != 16)
            {
                throw new InvalidDataException("Orchestra history request contract drifted");
            }
        }

        var runPage = DecodeHistoryResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_history","payload":{"runs":[{"runId":"orun-alpha","runtimeId":"runtime-alpha","planId":"direct-deployment","outcome":"succeeded","executedAt":"2026-08-26T08:00:00Z","steps":[{"step":"deploy","outcome":"succeeded","summary":"deployment accepted"}],"completedAt":"2026-08-26T08:00:01Z","attempt":1,"retriedFromRunId":null,"approvedBy":"operator-a","approvalNote":"verified","planRevision":"revision-1","requestId":"request-1"}],"events":[],"next_offset":1}}}
                """),
            "runtime-alpha",
            null,
            0,
            1);
        if (runPage.Runs is not [{ RunId: "orun-alpha" }]
            || runPage.Events.Count != 0
            || runPage.NextOffset != 1)
        {
            throw new InvalidDataException("Orchestra run history response contract drifted");
        }

        var eventPage = DecodeHistoryResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_history","payload":{"runs":[],"events":[{"eventId":9,"runId":"orun-alpha","runtimeId":"runtime-alpha","eventType":"guided_completion","fromOutcome":"running","toOutcome":"succeeded","summary":"deployment accepted","recordedAt":"2026-08-26T08:00:01Z"}],"next_offset":null}}}
                """),
            "runtime-alpha",
            "orun-alpha",
            0,
            32);
        if (eventPage.Events is not [{ EventId: 9 }]
            || eventPage.Runs.Count != 0)
        {
            throw new InvalidDataException("Orchestra event history response contract drifted");
        }

        var delete = EncodeDeleteRequest(
            "runtime-alpha",
            "operator-a",
            "gui-orchestra-command");
        using (var document = JsonDocument.Parse(delete))
        {
            var request = document.RootElement.GetProperty("request");
            var payload = request.GetProperty("payload");
            if (request.GetProperty("kind").GetString()
                    != "orchestra_delete_command"
                || payload.GetProperty("runtime_ids")[0].GetString()
                    != "runtime-alpha"
                || payload.GetProperty("command_id").GetString()
                    != "gui-orchestra-command")
            {
                throw new InvalidDataException("Orchestra cleanup request contract drifted");
            }
        }
        var receipt = DecodeDeleteResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_delete_receipt","payload":{"command_id":"gui-orchestra-command","operation_generation":4,"runtime_ids":["runtime-alpha"],"deleted_runtime_count":1,"deleted_run_count":2,"deleted_event_count":5,"committed_at_unix_ms":1787731200000,"replayed":false}}}
                """),
            "runtime-alpha",
            "gui-orchestra-command");
        if (receipt.OperationGeneration != 4
            || receipt.DeletedRunCount != 2
            || receipt.DeletedEventCount != 5)
        {
            throw new InvalidDataException("Orchestra cleanup receipt contract drifted");
        }

        ExpectInvalid(() => DecodeHistoryResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_history","payload":{"runs":[],"events":[],"next_offset":null,"forged":true}}}
                """),
            null,
            null,
            0,
            32));
        ExpectInvalid(() => DecodeHistoryResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_history","payload":{"runs":[],"events":[{"eventId":9,"runId":"other-run","runtimeId":"runtime-alpha","eventType":"guided_completion","fromOutcome":null,"toOutcome":"ok","summary":"","recordedAt":"2026-08-26T08:00:01Z"}],"next_offset":null}}}
                """),
            "runtime-alpha",
            "orun-alpha",
            0,
            32));
        ExpectInvalid(() => DecodeDeleteResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_delete_receipt","payload":{"command_id":"other-command","operation_generation":4,"runtime_ids":["runtime-alpha"],"deleted_runtime_count":1,"deleted_run_count":2,"deleted_event_count":5,"committed_at_unix_ms":1787731200000,"replayed":false}}}
                """),
            "runtime-alpha",
            "gui-orchestra-command"));
    }

    private static byte[] EncodeHistoryRequest(
        string? runtimeId,
        string? runId,
        uint offset,
        ushort limit,
        string principal)
    {
        var envelope = new OrchestraHistoryRequestEnvelope
        {
            Request = new OrchestraHistoryRequest
            {
                Payload = new OrchestraHistoryRequestPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = [OrchestraCapability],
                    RuntimeId = runtimeId,
                    RunId = runId,
                    Offset = offset,
                    Limit = limit,
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteOrchestraJsonContext.Default.OrchestraHistoryRequestEnvelope);
    }

    private static byte[] EncodeDeleteRequest(
        string runtimeId,
        string principal,
        string commandId)
    {
        var envelope = new OrchestraDeleteRequestEnvelope
        {
            Request = new OrchestraDeleteRequest
            {
                Payload = new OrchestraDeleteRequestPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = [OrchestraCapability],
                    CommandId = commandId,
                    RuntimeIds = [runtimeId],
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteOrchestraJsonContext.Default.OrchestraDeleteRequestEnvelope);
    }

    private static RemoteOrchestraHistoryPage DecodeHistoryResponse(
        ReadOnlySpan<byte> payload,
        string? runtimeId,
        string? runId,
        uint offset,
        ushort limit)
    {
        var response = DecodeEnvelope(payload, "orchestra_history");
        try
        {
            var page = response.Deserialize(
                RemoteOrchestraJsonContext.Default.RemoteOrchestraHistoryPage)
                ?? throw new InvalidDataException("Orchestra history response is empty");
            ValidateHistoryPage(page, runtimeId, runId, offset, limit);
            return page;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("Orchestra history response JSON is invalid", error);
        }
    }

    private static RemoteOrchestraDeleteReceipt DecodeDeleteResponse(
        ReadOnlySpan<byte> payload,
        string runtimeId,
        string commandId)
    {
        var response = DecodeEnvelope(payload, "orchestra_delete_receipt");
        try
        {
            var receipt = response.Deserialize(
                RemoteOrchestraJsonContext.Default.RemoteOrchestraDeleteReceipt)
                ?? throw new InvalidDataException("Orchestra cleanup response is empty");
            if (receipt.CommandId != commandId
                || receipt.RuntimeIds is not [var receiptRuntimeId]
                || receiptRuntimeId != runtimeId
                || receipt.OperationGeneration == 0
                || receipt.DeletedRuntimeCount > 1
                || receipt.DeletedRunCount > 32
                || receipt.DeletedEventCount > 96
                || receipt.CommittedAtUnixMs <= 0)
            {
                throw new InvalidDataException("Orchestra cleanup response identity is invalid");
            }
            return receipt;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("Orchestra cleanup response JSON is invalid", error);
        }
    }

    private static JsonElement DecodeEnvelope(
        ReadOnlySpan<byte> payload,
        string expectedKind)
    {
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteMutationJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException("Orchestra response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported Orchestra response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                var code = RequireResponseText(envelope.Response.Payload, "code", 128);
                var message = RequireResponseText(envelope.Response.Payload, "message", 1024);
                throw new RemoteOrchestraException(code, message);
            }
            if (envelope.Response.Kind != expectedKind)
            {
                throw new InvalidDataException("Orchestra response kind is invalid");
            }
            return envelope.Response.Payload;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("Orchestra response envelope JSON is invalid", error);
        }
        catch (KeyNotFoundException error)
        {
            throw new InvalidDataException("Orchestra response is missing a required field", error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException("Orchestra response has an invalid field type", error);
        }
    }

    private static void ValidateHistoryPage(
        RemoteOrchestraHistoryPage page,
        string? runtimeId,
        string? runId,
        uint offset,
        ushort limit)
    {
        if (page.Runs is null
            || page.Events is null
            || page.Runs.Any(run => run is null)
            || page.Events.Any(orchestraEvent => orchestraEvent is null)
            || page.Runs.Count > limit
            || page.Events.Count > limit
            || (runId is null && page.Events.Count != 0)
            || (runId is not null && page.Runs.Count != 0))
        {
            throw new InvalidDataException("Orchestra history response shape is invalid");
        }
        var itemCount = runId is null ? page.Runs.Count : page.Events.Count;
        if (page.NextOffset is uint nextOffset
            && (itemCount != limit
                || nextOffset != checked(offset + limit)))
        {
            throw new InvalidDataException("Orchestra history pagination is invalid");
        }

        var runIds = new HashSet<string>(StringComparer.Ordinal);
        foreach (var run in page.Runs)
        {
            ValidateRun(run);
            if (!runIds.Add(run.RunId)
                || (runtimeId is not null && run.RuntimeId != runtimeId))
            {
                throw new InvalidDataException("Orchestra run history identity is invalid");
            }
        }
        var eventIds = new HashSet<ulong>();
        foreach (var orchestraEvent in page.Events)
        {
            ValidateEvent(orchestraEvent);
            if (!eventIds.Add(orchestraEvent.EventId)
                || orchestraEvent.RuntimeId != runtimeId
                || orchestraEvent.RunId != runId)
            {
                throw new InvalidDataException("Orchestra event history identity is invalid");
            }
        }
    }

    private static void ValidateRun(RemoteOrchestraRun run)
    {
        RequireIdentifier(run.RunId, "run ID");
        RequireIdentifier(run.RuntimeId, "runtime ID");
        RequireBoundedText(run.PlanId, 128, false, "plan ID");
        RequireOutcome(run.Outcome, "run outcome");
        RequireTimestamp(run.ExecutedAt, "run execution timestamp");
        RequireOptionalTimestamp(run.CompletedAt, "run completion timestamp");
        if (run.Steps is null
            || run.Steps.Any(step => step is null)
            || run.Attempt is < 1 or > 1_000_000
            || run.Steps.Count > 256)
        {
            throw new InvalidDataException("Orchestra run bounds are invalid");
        }
        RequireOptionalIdentifier(run.RetriedFromRunId, "retried run ID");
        RequireOptionalText(run.ApprovedBy, 256, "approver");
        RequireOptionalText(run.ApprovalNote, 1024, "approval note");
        RequireOptionalText(run.PlanRevision, 128, "plan revision");
        RequireOptionalText(run.RequestId, 128, "request ID");
        foreach (var step in run.Steps)
        {
            RequireBoundedText(step.Step, 128, false, "Orchestra step");
            RequireBoundedText(step.Outcome, 128, false, "Orchestra step outcome");
            RequireBoundedText(step.Summary, 1024, true, "Orchestra step summary");
        }
    }

    private static void ValidateEvent(RemoteOrchestraEvent orchestraEvent)
    {
        if (orchestraEvent.EventId == 0)
        {
            throw new InvalidDataException("Orchestra event identity is invalid");
        }
        RequireIdentifier(orchestraEvent.RunId, "run ID");
        RequireIdentifier(orchestraEvent.RuntimeId, "runtime ID");
        RequireBoundedText(orchestraEvent.EventType, 128, false, "event type");
        if (orchestraEvent.FromOutcome is not null)
        {
            RequireOutcome(orchestraEvent.FromOutcome, "source outcome");
        }
        RequireOutcome(orchestraEvent.ToOutcome, "target outcome");
        RequireBoundedText(orchestraEvent.Summary, 1024, true, "event summary");
        RequireTimestamp(orchestraEvent.RecordedAt, "event timestamp");
    }

    private static string RequireResponseText(
        JsonElement element,
        string property,
        int maximum)
    {
        var value = element.GetProperty(property).GetString()
            ?? throw new InvalidDataException(
                $"Orchestra response field '{property}' is invalid");
        RequireBoundedText(value, maximum, false, property);
        return value;
    }

    private static void RequirePage(uint offset, ushort limit)
    {
        if (offset > MaxOffset || limit is 0 or > MaxPageSize)
        {
            throw new ArgumentOutOfRangeException(
                nameof(limit),
                "Orchestra history page is out of bounds");
        }
    }

    private static void RequireOptionalIdentifier(string? value, string label)
    {
        if (value is not null)
        {
            RequireIdentifier(value, label);
        }
    }

    private static void RequireIdentifier(string value, string label)
    {
        if (value.Length is < 1 or > 128
            || !value.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '-' or '_' or '.' or ':'))
        {
            throw new ArgumentException($"invalid {label}");
        }
    }

    private static void RequireOptionalText(string? value, int maximum, string label)
    {
        if (value is not null)
        {
            RequireBoundedText(value, maximum, false, label);
        }
    }

    private static void RequireBoundedText(
        string value,
        int maximum,
        bool allowEmpty,
        string label)
    {
        if (Encoding.UTF8.GetByteCount(value) > maximum
            || (!allowEmpty && value.Length == 0)
            || value != value.Trim()
            || value.Any(char.IsControl))
        {
            throw new InvalidDataException($"invalid {label}");
        }
    }

    private static void RequireOutcome(string value, string label)
    {
        if (!Outcomes.Contains(value))
        {
            throw new InvalidDataException($"invalid {label}");
        }
    }

    private static void RequireOptionalTimestamp(string? value, string label)
    {
        if (value is not null)
        {
            RequireTimestamp(value, label);
        }
    }

    private static void RequireTimestamp(string value, string label)
    {
        RequireBoundedText(value, 64, false, label);
        if (!DateTimeOffset.TryParse(
            value,
            CultureInfo.InvariantCulture,
            DateTimeStyles.RoundtripKind,
            out _))
        {
            throw new InvalidDataException($"invalid {label}");
        }
    }

    private static void ExpectInvalid(Action action)
    {
        try
        {
            action();
            throw new InvalidDataException("invalid Orchestra fixture was accepted");
        }
        catch (InvalidDataException)
        {
        }
    }
}
