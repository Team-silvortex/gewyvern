using System.Globalization;
using System.Text;
using System.Text.Json;

public sealed class RemoteOrchestraClient : IDisposable
{
    public const ushort DefaultPageSize = 32;
    public const ushort MaxPageSize = 64;
    public const uint MaxOffset = 10_000;
    private const string OrchestraCapability = "orchestra.write";
    private static readonly HashSet<string> AttentionSeverities =
        ["healthy", "warning", "critical"];
    private static readonly HashSet<string> RiskLevels =
        ["low", "medium", "high"];
    private static readonly HashSet<string> ExecutionReadiness =
        ["ready_now", "review_first"];
    private static readonly HashSet<string> ExecutionModes =
        ["automatic", "guided"];
    private static readonly HashSet<string> ApprovalModes =
        ["none", "operator_confirmation"];
    private static readonly HashSet<string> StepKinds =
        ["refresh", "review"];
    private static readonly HashSet<string> ControlOperations =
        ["run", "cancel", "retry"];
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

    public async Task<RemoteOrchestraPlanCatalog> LoadPlansAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken = default)
    {
        RequireIdentifier(runtimeId, "runtime ID");
        RequireIdentifier(principal, "principal");
        var payload = EncodePlanCatalogRequest(runtimeId, principal);
        var response = await transport.PostAsync(
            payload,
            "Orchestra plan catalog",
            cancellationToken).ConfigureAwait(false);
        return DecodePlanCatalogResponse(response, runtimeId);
    }

    public async Task<RemoteOrchestraRunReceipt> RunPlanAsync(
        string runtimeId,
        RemoteOrchestraPlan plan,
        string principal,
        string? approvedBy,
        string? approvalNote,
        CancellationToken cancellationToken = default)
    {
        RequireIdentifier(runtimeId, "runtime ID");
        RequireIdentifier(principal, "principal");
        ValidatePlan(plan);
        RequireExecutablePlan(plan);
        ValidateApproval(plan, approvedBy, approvalNote);
        var commandId = NewCommandId("run");
        var payload = EncodeRunRequest(
            runtimeId,
            plan,
            principal,
            approvedBy,
            approvalNote,
            commandId);
        var response = await transport.PostAsync(
            payload,
            "Orchestra plan execution",
            cancellationToken).ConfigureAwait(false);
        return DecodeRunReceipt(
            response,
            "run",
            commandId,
            runtimeId,
            plan.PlanId,
            plan.Revision,
            null,
            null);
    }

    public async Task<RemoteOrchestraRunReceipt> CancelRunAsync(
        RemoteOrchestraRun run,
        string principal,
        CancellationToken cancellationToken = default)
    {
        ValidateRun(run);
        RequireIdentifier(principal, "principal");
        if (run.Outcome is not ("queued" or "running"))
        {
            throw new InvalidOperationException("only active Orchestra runs can be cancelled");
        }
        var commandId = NewCommandId("cancel");
        var payload = EncodeCancelRequest(run, principal, commandId);
        var response = await transport.PostAsync(
            payload,
            "Orchestra run cancellation",
            cancellationToken).ConfigureAwait(false);
        return DecodeRunReceipt(
            response,
            "cancel",
            commandId,
            run.RuntimeId,
            run.PlanId,
            run.PlanRevision,
            run.RunId,
            run.Attempt);
    }

    public async Task<RemoteOrchestraRunReceipt> RetryRunAsync(
        RemoteOrchestraRun run,
        RemoteOrchestraPlan plan,
        string principal,
        string? approvedBy,
        string? approvalNote,
        CancellationToken cancellationToken = default)
    {
        ValidateRun(run);
        ValidatePlan(plan);
        RequireIdentifier(principal, "principal");
        RequireExecutablePlan(plan);
        ValidateApproval(plan, approvedBy, approvalNote);
        if (run.Outcome is "queued" or "running"
            || run.RuntimeId.Length == 0
            || run.PlanId != plan.PlanId)
        {
            throw new InvalidOperationException(
                "only a terminal run with a matching current plan can be retried");
        }
        var commandId = NewCommandId("retry");
        var payload = EncodeRetryRequest(
            run,
            plan,
            principal,
            approvedBy,
            approvalNote,
            commandId);
        var response = await transport.PostAsync(
            payload,
            "Orchestra run retry",
            cancellationToken).ConfigureAwait(false);
        return DecodeRunReceipt(
            response,
            "retry",
            commandId,
            run.RuntimeId,
            plan.PlanId,
            plan.Revision,
            run.RunId,
            run.Attempt);
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
        var planRequest = EncodePlanCatalogRequest(
            "runtime-alpha",
            "operator-a");
        using (var document = JsonDocument.Parse(planRequest))
        {
            var request = document.RootElement.GetProperty("request");
            var payload = request.GetProperty("payload");
            if (request.GetProperty("kind").GetString()
                    != "orchestra_plan_catalog"
                || payload.GetProperty("runtime_id").GetString()
                    != "runtime-alpha"
                || payload.GetProperty("capabilities")[0].GetString()
                    != OrchestraCapability)
            {
                throw new InvalidDataException(
                    "Orchestra plan request contract drifted");
            }
        }
        var catalog = DecodePlanCatalogResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_plan_catalog","payload":{"runtime_id":"runtime-alpha","runtime_name":"Alpha runtime","runtime_revision":7,"status_source":"gewyvern-api","attention_severity":"warning","needs_attention":true,"attention_reasons":["no_analysis_json"],"plans":[{"plan_id":"runtime_triage","intent":"triage","title":"Refresh and verify runtime posture","summary":"Refresh authoritative status.","risk_level":"low","execution_readiness":"ready_now","execution_mode":"automatic","approval_mode":"none","revision":"orchestra-v1-7-runtime_triage","reasons":["no_analysis_json"],"required_capabilities":[],"steps":[{"key":"refresh_status","title":"Refresh runtime status","detail":"Run the bounded native adapter.","kind":"refresh"}]},{"plan_id":"session_preparation","intent":"prepare_session","title":"Prepare a session handoff","summary":"Review the handoff.","risk_level":"medium","execution_readiness":"review_first","execution_mode":"guided","approval_mode":"operator_confirmation","revision":"orchestra-v1-7-session_preparation","reasons":[],"required_capabilities":[],"steps":[{"key":"review_session","title":"Review session requirements","detail":"Review without execution authority.","kind":"review"}]}]}}}
                """),
            "runtime-alpha");
        if (catalog.RuntimeRevision != 7
            || catalog.Plans is not
                [{ PlanId: "runtime_triage" }, { PlanId: "session_preparation" }])
        {
            throw new InvalidDataException(
                "Orchestra plan response contract drifted");
        }

        var runRequest = EncodeRunRequest(
            "runtime-alpha",
            catalog.Plans[0],
            "operator-a",
            null,
            null,
            "gui-orchestra-run-contract");
        using (var document = JsonDocument.Parse(runRequest))
        {
            var request = document.RootElement.GetProperty("request");
            var payload = request.GetProperty("payload");
            if (request.GetProperty("kind").GetString()
                    != "orchestra_run_command"
                || payload.GetProperty("expected_plan_revision").GetString()
                    != catalog.Plans[0].Revision
                || !payload.GetProperty("confirmed").GetBoolean()
                || payload.TryGetProperty("approval_note", out _))
            {
                throw new InvalidDataException(
                    "Orchestra run request contract drifted");
            }
        }
        var runReceipt = DecodeRunReceipt(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_run_receipt","payload":{"command_id":"gui-orchestra-run-contract","operation":"run","run":{"runId":"orun-gui-orchestra-run-contract","runtimeId":"runtime-alpha","planId":"runtime_triage","outcome":"queued","executedAt":"2026-08-26T08:00:00Z","steps":[],"completedAt":null,"attempt":1,"retriedFromRunId":null,"approvedBy":null,"approvalNote":null,"planRevision":"orchestra-v1-7-runtime_triage","requestId":"gui-orchestra-run-contract"},"replayed":false}}}
                """),
            "run",
            "gui-orchestra-run-contract",
            "runtime-alpha",
            "runtime_triage",
            "orchestra-v1-7-runtime_triage",
            null,
            null);
        if (runReceipt.Run.RunId != "orun-gui-orchestra-run-contract"
            || runReceipt.Replayed)
        {
            throw new InvalidDataException(
                "Orchestra run receipt contract drifted");
        }

        var cancelRequest = EncodeCancelRequest(
            runReceipt.Run,
            "operator-a",
            "gui-orchestra-cancel-contract");
        var retrySource = new RemoteOrchestraRun
        {
            RunId = runReceipt.Run.RunId,
            RuntimeId = runReceipt.Run.RuntimeId,
            PlanId = runReceipt.Run.PlanId,
            Outcome = "cancelled",
            ExecutedAt = runReceipt.Run.ExecutedAt,
            CompletedAt = "2026-08-26T08:00:01Z",
            Steps = [],
            Attempt = 1,
            PlanRevision = runReceipt.Run.PlanRevision,
            RequestId = runReceipt.Run.RequestId,
        };
        var retryRequest = EncodeRetryRequest(
            retrySource,
            catalog.Plans[0],
            "operator-a",
            null,
            null,
            "gui-orchestra-retry-contract");
        using (var cancelDocument = JsonDocument.Parse(cancelRequest))
        using (var retryDocument = JsonDocument.Parse(retryRequest))
        {
            if (cancelDocument.RootElement.GetProperty("request")
                    .GetProperty("kind").GetString()
                    != "orchestra_cancel_command"
                || retryDocument.RootElement.GetProperty("request")
                    .GetProperty("kind").GetString()
                    != "orchestra_retry_command")
            {
                throw new InvalidDataException(
                    "Orchestra control request contract drifted");
            }
        }

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
        ExpectInvalid(() => DecodePlanCatalogResponse(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_plan_catalog","payload":{"runtime_id":"runtime-alpha","runtime_name":"Alpha runtime","runtime_revision":7,"status_source":"gewyvern-api","attention_severity":"healthy","needs_attention":false,"attention_reasons":[],"plans":[],"forged":true}}}
                """),
            "runtime-alpha"));
        ExpectInvalid(() => DecodeRunReceipt(
            Encoding.UTF8.GetBytes(
                """
                {"schema_version":1,"response":{"kind":"orchestra_run_receipt","payload":{"command_id":"other-command","operation":"run","run":{"runId":"orun-other-command","runtimeId":"runtime-alpha","planId":"runtime_triage","outcome":"queued","executedAt":"2026-08-26T08:00:00Z","steps":[],"completedAt":null,"attempt":1,"retriedFromRunId":null,"approvedBy":null,"approvalNote":null,"planRevision":"orchestra-v1-7-runtime_triage","requestId":"other-command"},"replayed":false}}}
                """),
            "run",
            "gui-orchestra-run-contract",
            "runtime-alpha",
            "runtime_triage",
            "orchestra-v1-7-runtime_triage",
            null,
            null));
    }

    private static byte[] EncodePlanCatalogRequest(
        string runtimeId,
        string principal)
    {
        var envelope = new OrchestraPlanCatalogRequestEnvelope
        {
            Request = new OrchestraPlanCatalogRequest
            {
                Payload = new OrchestraPlanCatalogRequestPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = [OrchestraCapability],
                    RuntimeId = runtimeId,
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteOrchestraJsonContext.Default.OrchestraPlanCatalogRequestEnvelope);
    }

    private static byte[] EncodeRunRequest(
        string runtimeId,
        RemoteOrchestraPlan plan,
        string principal,
        string? approvedBy,
        string? approvalNote,
        string commandId)
    {
        var envelope = new OrchestraRunCommandRequestEnvelope
        {
            Request = new OrchestraRunCommandRequest
            {
                Payload = new OrchestraRunCommandRequestPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = [OrchestraCapability],
                    CommandId = commandId,
                    RuntimeId = runtimeId,
                    PlanId = plan.PlanId,
                    ExpectedPlanRevision = plan.Revision,
                    Confirmed = true,
                    ApprovedBy = approvedBy,
                    ApprovalNote = approvalNote,
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteOrchestraJsonContext.Default.OrchestraRunCommandRequestEnvelope);
    }

    private static byte[] EncodeCancelRequest(
        RemoteOrchestraRun run,
        string principal,
        string commandId)
    {
        var envelope = new OrchestraCancelCommandRequestEnvelope
        {
            Request = new OrchestraCancelCommandRequest
            {
                Payload = new OrchestraCancelCommandRequestPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = [OrchestraCapability],
                    CommandId = commandId,
                    RuntimeId = run.RuntimeId,
                    RunId = run.RunId,
                    Confirmed = true,
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteOrchestraJsonContext.Default.OrchestraCancelCommandRequestEnvelope);
    }

    private static byte[] EncodeRetryRequest(
        RemoteOrchestraRun run,
        RemoteOrchestraPlan plan,
        string principal,
        string? approvedBy,
        string? approvalNote,
        string commandId)
    {
        var envelope = new OrchestraRetryCommandRequestEnvelope
        {
            Request = new OrchestraRetryCommandRequest
            {
                Payload = new OrchestraRetryCommandRequestPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = [OrchestraCapability],
                    CommandId = commandId,
                    RuntimeId = run.RuntimeId,
                    RunId = run.RunId,
                    ExpectedPlanRevision = plan.Revision,
                    Confirmed = true,
                    ApprovedBy = approvedBy,
                    ApprovalNote = approvalNote,
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteOrchestraJsonContext.Default.OrchestraRetryCommandRequestEnvelope);
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

    private static RemoteOrchestraPlanCatalog DecodePlanCatalogResponse(
        ReadOnlySpan<byte> payload,
        string runtimeId)
    {
        var response = DecodeEnvelope(payload, "orchestra_plan_catalog");
        try
        {
            var catalog = response.Deserialize(
                RemoteOrchestraJsonContext.Default.RemoteOrchestraPlanCatalog)
                ?? throw new InvalidDataException(
                    "Orchestra plan catalog response is empty");
            ValidatePlanCatalog(catalog, runtimeId);
            return catalog;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException(
                "Orchestra plan catalog response JSON is invalid",
                error);
        }
    }

    private static RemoteOrchestraRunReceipt DecodeRunReceipt(
        ReadOnlySpan<byte> payload,
        string expectedOperation,
        string commandId,
        string runtimeId,
        string planId,
        string? planRevision,
        string? sourceRunId,
        uint? sourceAttempt)
    {
        var response = DecodeEnvelope(payload, "orchestra_run_receipt");
        try
        {
            var receipt = response.Deserialize(
                RemoteOrchestraJsonContext.Default.RemoteOrchestraRunReceipt)
                ?? throw new InvalidDataException(
                    "Orchestra control response is empty");
            ValidateRun(receipt.Run);
            RequireIdentifier(receipt.CommandId, "command ID");
            RequireBoundedText(receipt.Operation, 16, false, "control operation");
            var terminal = receipt.Run.Outcome is not ("queued" or "running");
            if (!ControlOperations.Contains(receipt.Operation)
                || receipt.Operation != expectedOperation
                || receipt.CommandId != commandId
                || receipt.Run.RuntimeId != runtimeId
                || receipt.Run.PlanId != planId
                || (terminal && receipt.Run.CompletedAt is null)
                || (!terminal && receipt.Run.CompletedAt is not null))
            {
                throw new InvalidDataException(
                    "Orchestra control response identity is invalid");
            }

            switch (expectedOperation)
            {
                case "run" when receipt.Run.RunId != $"orun-{commandId}"
                    || receipt.Run.RequestId != commandId
                    || receipt.Run.PlanRevision != planRevision
                    || receipt.Run.Attempt != 1
                    || receipt.Run.RetriedFromRunId is not null:
                case "cancel" when receipt.Run.RunId != sourceRunId
                    || receipt.Run.Outcome != "cancelled"
                    || receipt.Run.Attempt != sourceAttempt:
                case "retry" when receipt.Run.RunId != $"orun-{commandId}"
                    || receipt.Run.RequestId != commandId
                    || receipt.Run.RetriedFromRunId != sourceRunId
                    || receipt.Run.PlanRevision != planRevision
                    || sourceAttempt is null
                    || receipt.Run.Attempt != checked(sourceAttempt.Value + 1):
                    throw new InvalidDataException(
                        "Orchestra control response lineage is invalid");
            }
            return receipt;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException(
                "Orchestra control response JSON is invalid",
                error);
        }
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

    private static void ValidatePlanCatalog(
        RemoteOrchestraPlanCatalog catalog,
        string runtimeId)
    {
        RequireIdentifier(catalog.RuntimeId, "runtime ID");
        RequireBoundedText(catalog.RuntimeName, 256, false, "runtime name");
        RequireBoundedText(catalog.StatusSource, 128, false, "status source");
        if (catalog.RuntimeId != runtimeId
            || catalog.RuntimeRevision == 0
            || !AttentionSeverities.Contains(catalog.AttentionSeverity)
            || catalog.AttentionReasons is null
            || catalog.Plans is null
            || catalog.AttentionReasons.Any(reason => reason is null)
            || catalog.Plans.Any(plan => plan is null)
            || catalog.AttentionReasons.Count > 16
            || catalog.Plans.Count is < 1 or > 16
            || (catalog.AttentionSeverity == "healthy"
                && (catalog.NeedsAttention || catalog.AttentionReasons.Count != 0))
            || (catalog.AttentionSeverity != "healthy"
                && (!catalog.NeedsAttention || catalog.AttentionReasons.Count == 0)))
        {
            throw new InvalidDataException(
                "Orchestra plan catalog shape is invalid");
        }
        foreach (var reason in catalog.AttentionReasons)
        {
            RequireBoundedText(reason, 128, false, "attention reason");
        }
        var planIds = new HashSet<string>(StringComparer.Ordinal);
        var revisions = new HashSet<string>(StringComparer.Ordinal);
        foreach (var plan in catalog.Plans)
        {
            ValidatePlan(plan);
            if (!planIds.Add(plan.PlanId) || !revisions.Add(plan.Revision))
            {
                throw new InvalidDataException(
                    "Orchestra plan catalog identity is invalid");
            }
        }
    }

    private static void ValidatePlan(RemoteOrchestraPlan plan)
    {
        RequireIdentifier(plan.PlanId, "plan ID");
        RequireIdentifier(plan.Intent, "plan intent");
        RequireBoundedText(plan.Title, 256, false, "plan title");
        RequireBoundedText(plan.Summary, 1024, false, "plan summary");
        RequireBoundedText(plan.Revision, 128, false, "plan revision");
        if (!RiskLevels.Contains(plan.RiskLevel)
            || !ExecutionReadiness.Contains(plan.ExecutionReadiness)
            || !ExecutionModes.Contains(plan.ExecutionMode)
            || !ApprovalModes.Contains(plan.ApprovalMode)
            || plan.Reasons is null
            || plan.RequiredCapabilities is null
            || plan.Steps is null
            || plan.Reasons.Any(reason => reason is null)
            || plan.RequiredCapabilities.Any(capability => capability is null)
            || plan.Steps.Any(step => step is null)
            || plan.Reasons.Count > 16
            || plan.RequiredCapabilities.Count > 16
            || plan.Steps.Count is < 1 or > 32
            || !plan.Revision.EndsWith(
                $"-{plan.PlanId}",
                StringComparison.Ordinal)
            || (plan.ExecutionMode == "automatic"
                && plan.ExecutionReadiness != "ready_now")
            || (plan.ExecutionMode == "guided"
                && plan.ExecutionReadiness != "review_first"))
        {
            throw new InvalidDataException("Orchestra plan shape is invalid");
        }
        foreach (var reason in plan.Reasons)
        {
            RequireBoundedText(reason, 128, false, "plan reason");
        }
        foreach (var capability in plan.RequiredCapabilities)
        {
            RequireIdentifier(capability, "required capability");
        }
        var stepKeys = new HashSet<string>(StringComparer.Ordinal);
        foreach (var step in plan.Steps)
        {
            RequireIdentifier(step.Key, "plan step key");
            RequireBoundedText(step.Title, 256, false, "plan step title");
            RequireBoundedText(step.Detail, 1024, false, "plan step detail");
            if (!StepKinds.Contains(step.Kind) || !stepKeys.Add(step.Key))
            {
                throw new InvalidDataException(
                    "Orchestra plan step shape is invalid");
            }
        }
    }

    private static void RequireExecutablePlan(RemoteOrchestraPlan plan)
    {
        if (plan.ExecutionMode != "automatic"
            || plan.ExecutionReadiness != "ready_now")
        {
            throw new InvalidOperationException(
                "guided Orchestra plans are not executable by this client");
        }
    }

    private static void ValidateApproval(
        RemoteOrchestraPlan plan,
        string? approvedBy,
        string? approvalNote)
    {
        RequireOptionalBoundedArgument(approvedBy, 80, "approver");
        RequireOptionalBoundedArgument(approvalNote, 500, "approval note");
        if (plan.ApprovalMode == "operator_confirmation"
            && (approvedBy is null || approvalNote is null))
        {
            throw new ArgumentException(
                "this Orchestra plan requires an approver and approval note");
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

    private static void RequireIdentifier(string? value, string label)
    {
        if (value is null
            || value.Length is < 1 or > 128
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
        string? value,
        int maximum,
        bool allowEmpty,
        string label)
    {
        if (value is null
            || Encoding.UTF8.GetByteCount(value) > maximum
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

    private static void RequireOptionalBoundedArgument(
        string? value,
        int maximum,
        string label)
    {
        if (value is null)
        {
            return;
        }
        try
        {
            RequireBoundedText(value, maximum, false, label);
        }
        catch (InvalidDataException error)
        {
            throw new ArgumentException(error.Message, label, error);
        }
    }

    private static string NewCommandId(string operation) =>
        $"gui-orchestra-{operation}-{Guid.NewGuid():N}";

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
