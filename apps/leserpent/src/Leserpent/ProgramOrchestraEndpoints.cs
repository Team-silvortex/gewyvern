using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Leserpent.ControlPlane;

namespace Leserpent;

public partial class Program
{
    private static void MapOrchestraEndpoints(WebApplication app)
    {
        app.MapGet("/v1/orchestra/plans/{id}", (string id, RegistryService registry) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }
            var attention = registry.GetRuntimeAttention(id);
            var reasons = attention?.Reasons ?? Array.Empty<string>();
            var severity = attention?.Severity ?? "none";
            var needsAttention = attention?.NeedsAttention ?? false;
            var plans = OrchestraPlanner.Build(runtime, reasons, severity, needsAttention);

            return Results.Ok(new OrchestraRuntimePlanResponse(
                runtime.RuntimeId,
                runtime.Name,
                runtime.Endpoint,
                runtime.Tags,
                runtime.Status.StatusSource,
                severity,
                needsAttention,
                reasons,
                plans));
        });

        app.MapGet("/v1/orchestra/runtimes/{id}/runs", (string id, RegistryService registry) =>
        {
            if (registry.GetRuntime(id) is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }

            return Results.Ok(new { runtimeId = id, runs = registry.ListOrchestraRuns(id) });
        });

        app.MapGet("/v1/orchestra/runtimes/{id}/runs/{runId}/events", (
            string id,
            string runId,
            RegistryService registry) =>
        {
            if (registry.GetRuntime(id) is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }
            if (registry.GetOrchestraRun(id, runId) is null)
            {
                return Results.NotFound(new { error = "orchestra_run_not_found", runtimeId = id, runId });
            }

            return Results.Ok(new
            {
                runtimeId = id,
                runId,
                events = registry.ListOrchestraRunEvents(id, runId),
            });
        });

        app.MapGet("/v1/orchestra/runs", (RegistryService registry) =>
            Results.Ok(registry.GetOrchestraFleetBoard()));

        app.MapPost("/v1/orchestra/plans/{id}/{planId}/execute", (
            string id,
            string planId,
            OrchestraExecuteRequest request,
            RegistryService registry,
            OrchestraExecutionCoordinator coordinator) =>
        {
            var requestIdError = ValidateOrchestraRequestId(request.RequestId);
            if (requestIdError is not null)
            {
                return Results.BadRequest(new { error = "invalid_orchestra_request_id", reason = requestIdError });
            }
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }
            var replay = registry.GetOrchestraRunByRequestId(id, request.RequestId!.Trim());
            if (replay is not null)
            {
                return string.Equals(replay.PlanId, planId, StringComparison.OrdinalIgnoreCase)
                    ? Results.Accepted($"/v1/orchestra/runtimes/{id}/runs", new { run = replay, replayed = true })
                    : Results.Conflict(new { error = "orchestra_request_id_reused_for_different_plan", runtimeId = id, planId, requestId = request.RequestId });
            }

            var attention = registry.GetRuntimeAttention(id);
            var reasons = attention?.Reasons ?? Array.Empty<string>();
            var plans = OrchestraPlanner.Build(runtime, reasons, attention?.Severity ?? "none", attention?.NeedsAttention ?? false);
            var selectedPlan = plans.FirstOrDefault(plan =>
                string.Equals(plan.PlanId, planId, StringComparison.OrdinalIgnoreCase));
            if (selectedPlan is null)
            {
                return Results.NotFound(new { error = "orchestra_plan_not_found", runtimeId = id, planId });
            }

            if (!string.Equals(selectedPlan.ExecutionMode, "automatic", StringComparison.OrdinalIgnoreCase))
            {
                return Results.Conflict(new
                {
                    error = "orchestra_plan_requires_operator_input",
                    runtimeId = id,
                    planId,
                    suggestedSurfaces = selectedPlan.SuggestedSurfaces,
                });
            }

            if (string.IsNullOrWhiteSpace(request.ExpectedRevision)
                || !string.Equals(request.ExpectedRevision, selectedPlan.Revision, StringComparison.Ordinal))
            {
                return Results.Conflict(new
                {
                    error = "orchestra_plan_revision_changed",
                    runtimeId = id,
                    planId,
                    currentPlan = selectedPlan,
                });
            }
            if (string.Equals(selectedPlan.ApprovalMode, "operator_confirmation", StringComparison.OrdinalIgnoreCase)
                && !request.Confirmed)
            {
                return Results.Conflict(new { error = "orchestra_plan_confirmation_required", runtimeId = id, planId });
            }
            var approvalError = ValidateOrchestraApproval(selectedPlan, request.ApprovedBy, request.ApprovalNote);
            if (approvalError is not null)
            {
                return Results.BadRequest(new { error = "invalid_orchestra_approval", reason = approvalError, runtimeId = id, planId });
            }

            var started = coordinator.TryStart(
                runtime,
                selectedPlan.PlanId,
                selectedPlan.Revision,
                NormalizeApprovalActor(selectedPlan, request.ApprovedBy),
                NormalizeApprovalNote(request.ApprovalNote),
                request.RequestId!.Trim());
            if (started.RequestConflict)
            {
                return Results.Conflict(new { error = "orchestra_request_id_reused_for_different_plan", runtimeId = id, planId, requestId = request.RequestId });
            }
            if (started.PersistenceFailed)
            {
                return Results.Json(new { error = "orchestra_persistence_unavailable", runtimeId = id, planId }, statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            return started.Run is null
                ? Results.Conflict(new { error = "orchestra_runtime_busy", runtimeId = id, activeRun = started.ActiveRun })
                : Results.Accepted($"/v1/orchestra/runtimes/{id}/runs", new { run = started.Run, replayed = started.Replayed });
        });

        app.MapPost("/v1/orchestra/runtimes/{id}/runs/{runId}/cancel", (
            string id,
            string runId,
            RegistryService registry,
            OrchestraExecutionCoordinator coordinator) =>
        {
            var run = registry.GetOrchestraRun(id, runId);
            if (run is null)
            {
                return Results.NotFound(new { error = "orchestra_run_not_found", runtimeId = id, runId });
            }
            if (RegistryService.IsTerminalOrchestraOutcome(run.Outcome))
            {
                return Results.Conflict(new { error = "orchestra_run_already_terminal", runtimeId = id, runId, outcome = run.Outcome });
            }

            var cancelling = coordinator.Cancel(id, runId);
            return cancelling is null
                ? Results.Conflict(new { error = "orchestra_run_not_active", runtimeId = id, runId })
                : Results.Accepted($"/v1/orchestra/runtimes/{id}/runs", new { run = cancelling });
        });

        app.MapPost("/v1/orchestra/runtimes/{id}/runs/{runId}/retry", (
            string id,
            string runId,
            OrchestraRetryRequest request,
            RegistryService registry,
            OrchestraExecutionCoordinator coordinator) =>
        {
            var requestIdError = ValidateOrchestraRequestId(request.RequestId);
            if (requestIdError is not null)
            {
                return Results.BadRequest(new { error = "invalid_orchestra_request_id", reason = requestIdError });
            }
            var replay = registry.GetOrchestraRunByRequestId(id, request.RequestId!.Trim());
            if (replay is not null)
            {
                return string.Equals(replay.RetriedFromRunId, runId, StringComparison.OrdinalIgnoreCase)
                    ? Results.Accepted($"/v1/orchestra/runtimes/{id}/runs", new { run = replay, replayed = true })
                    : Results.Conflict(new { error = "orchestra_request_id_reused_for_different_retry", runtimeId = id, runId, requestId = request.RequestId });
            }
            var previous = registry.GetOrchestraRun(id, runId);
            if (previous is null)
            {
                return Results.NotFound(new { error = "orchestra_run_not_found", runtimeId = id, runId });
            }
            if (!RegistryService.IsTerminalOrchestraOutcome(previous.Outcome))
            {
                return Results.Conflict(new { error = "orchestra_run_not_terminal", runtimeId = id, runId, outcome = previous.Outcome });
            }

            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }
            var attention = registry.GetRuntimeAttention(id);
            var plans = OrchestraPlanner.Build(
                runtime,
                attention?.Reasons ?? Array.Empty<string>(),
                attention?.Severity ?? "none",
                attention?.NeedsAttention ?? false);
            var plan = plans.FirstOrDefault(candidate => string.Equals(candidate.PlanId, previous.PlanId, StringComparison.OrdinalIgnoreCase));
            if (plan is null || !string.Equals(plan.ExecutionMode, "automatic", StringComparison.OrdinalIgnoreCase))
            {
                return Results.Conflict(new { error = "orchestra_run_not_retryable", runtimeId = id, runId, planId = previous.PlanId });
            }
            if (string.Equals(plan.ApprovalMode, "operator_confirmation", StringComparison.OrdinalIgnoreCase)
                && !request.Confirmed)
            {
                return Results.Conflict(new { error = "orchestra_plan_confirmation_required", runtimeId = id, planId = plan.PlanId });
            }
            var approvalError = ValidateOrchestraApproval(plan, request.ApprovedBy, request.ApprovalNote);
            if (approvalError is not null)
            {
                return Results.BadRequest(new { error = "invalid_orchestra_approval", reason = approvalError, runtimeId = id, planId = plan.PlanId });
            }

            var started = coordinator.TryStart(
                runtime,
                plan.PlanId,
                plan.Revision,
                NormalizeApprovalActor(plan, request.ApprovedBy),
                NormalizeApprovalNote(request.ApprovalNote),
                request.RequestId!.Trim(),
                previous);
            if (started.RequestConflict)
            {
                return Results.Conflict(new { error = "orchestra_request_id_reused_for_different_plan", runtimeId = id, planId = plan.PlanId, requestId = request.RequestId });
            }
            if (started.PersistenceFailed)
            {
                return Results.Json(new { error = "orchestra_persistence_unavailable", runtimeId = id, planId = plan.PlanId }, statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            return started.Run is null
                ? Results.Conflict(new { error = "orchestra_runtime_busy", runtimeId = id, activeRun = started.ActiveRun })
                : Results.Accepted($"/v1/orchestra/runtimes/{id}/runs", new { run = started.Run, replayed = started.Replayed });
        });

        app.MapPost("/v1/orchestra/plans/{id}/session", (
            string id,
            OrchestraSessionHandoffRequest request,
            RegistryService registry) =>
        {
            if (string.IsNullOrWhiteSpace(request.PipelineKind)
                || string.IsNullOrWhiteSpace(request.RequestedBy)
                || request.PipelineKind.Trim().Length > 128
                || request.RequestedBy.Trim().Length > 80)
            {
                return Results.BadRequest(new
                {
                    error = "invalid_orchestra_session_handoff",
                    reason = "pipelineKind and requestedBy are required and must stay within their length limits",
                });
            }

            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }

            var attention = registry.GetRuntimeAttention(id);
            var reasons = attention?.Reasons ?? Array.Empty<string>();
            var plans = OrchestraPlanner.Build(runtime, reasons, attention?.Severity ?? "none", attention?.NeedsAttention ?? false);
            var sessionPlan = plans.Single(plan => string.Equals(plan.PlanId, "session_preparation", StringComparison.Ordinal));
            var requirements = sessionPlan.RequiredCapabilities
                .Select(capability => new SessionCapabilityRequirement(capability, "fully_supported"))
                .ToArray();
            var result = registry.CreateSession(new SessionCreateRequest(
                id,
                request.PipelineKind,
                request.RequestedBy,
                requirements));

            if (result.Rejections.Count > 0)
            {
                return Results.BadRequest(new
                {
                    error = "capability_requirements_not_satisfied",
                    rejections = result.Rejections,
                });
            }

            if (result.Session is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }

            var steps = new[]
            {
                new OrchestraExecutionStepResult(
                    "create_session",
                    "ok",
                    $"session {result.Session.SessionId} created for pipeline {result.Session.PipelineKind}"),
            };
            var currentRuntime = registry.GetRuntime(id);
            if (currentRuntime is null)
            {
                return Results.Conflict(new { error = "runtime_removed_during_orchestra_handoff", runtimeId = id });
            }
            var run = registry.RecordOrchestraRun(
                id,
                "session_preparation",
                "ok",
                steps,
                request.RequestedBy.Trim(),
                "guided session handoff",
                sessionPlan.Revision);
            var currentAttention = registry.GetRuntimeAttention(id);
            var currentReasons = currentAttention?.Reasons ?? Array.Empty<string>();
            var currentPlans = OrchestraPlanner.Build(
                currentRuntime,
                currentReasons,
                currentAttention?.Severity ?? "none",
                currentAttention?.NeedsAttention ?? false);
            return Results.Ok(new OrchestraSessionHandoffResponse(
                run,
                result.Session,
                BuildOrchestraResponse(currentRuntime, currentAttention, currentReasons, currentPlans)));
        });
    }

    private static OrchestraRuntimePlanResponse BuildOrchestraResponse(
        RuntimeSummary runtime,
        RuntimeAttentionView? attention,
        IReadOnlyList<string> reasons,
        IReadOnlyList<OrchestraPlan> plans) =>
        new(
            runtime.RuntimeId,
            runtime.Name,
            runtime.Endpoint,
            runtime.Tags,
            runtime.Status.StatusSource,
            attention?.Severity ?? "none",
            attention?.NeedsAttention ?? false,
            reasons,
            plans);

    internal static string? ValidateOrchestraApproval(OrchestraPlan plan, string? approvedBy, string? approvalNote)
    {
        if (!string.IsNullOrWhiteSpace(approvedBy) && approvedBy.Trim().Length > 80)
        {
            return "approvedBy must not exceed 80 characters";
        }
        if (!string.IsNullOrWhiteSpace(approvalNote) && approvalNote.Trim().Length > 500)
        {
            return "approvalNote must not exceed 500 characters";
        }
        if (!string.Equals(plan.ApprovalMode, "operator_confirmation", StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }
        if (string.IsNullOrWhiteSpace(approvedBy))
        {
            return "approvedBy is required for operator-confirmed plans";
        }
        return string.IsNullOrWhiteSpace(approvalNote)
            ? "approvalNote is required for operator-confirmed plans"
            : null;
    }

    internal static string? ValidateOrchestraRequestId(string? requestId)
    {
        if (string.IsNullOrWhiteSpace(requestId))
        {
            return "requestId is required";
        }
        var normalized = requestId.Trim();
        if (normalized.Length is < 8 or > 128)
        {
            return "requestId must contain between 8 and 128 characters";
        }
        return normalized.All(character => char.IsAsciiLetterOrDigit(character) || character is '-' or '_' or '.' or ':')
            ? null
            : "requestId contains unsupported characters";
    }

    private static string NormalizeApprovalActor(OrchestraPlan plan, string? approvedBy) =>
        string.Equals(plan.ApprovalMode, "operator_confirmation", StringComparison.OrdinalIgnoreCase)
            ? approvedBy!.Trim()
            : string.IsNullOrWhiteSpace(approvedBy) ? "automatic" : approvedBy.Trim();

    private static string? NormalizeApprovalNote(string? approvalNote) =>
        string.IsNullOrWhiteSpace(approvalNote) ? null : approvalNote.Trim();

}

internal static class OrchestraPlanner
{
    internal static IReadOnlyList<OrchestraPlan> Build(
        RuntimeSummary runtime,
        IReadOnlyList<string> reasons,
        string severity,
        bool needsAttention)
    {
        var plans = new List<OrchestraPlan>
        {
            BuildRuntimeTriagePlan(runtime, reasons, severity, needsAttention),
            BuildAnalysisRecoveryPlan(runtime, reasons),
        };

        if (!string.IsNullOrWhiteSpace(runtime.SidecarEndpoint))
        {
            plans.Add(BuildSidecarCoordinationPlan(runtime, reasons));
        }

        plans.Add(BuildSessionPreparationPlan(runtime));
        return plans.Select(plan => ApplyExecutionPolicy(plan, runtime)).ToArray();
    }

    private static OrchestraPlan ApplyExecutionPolicy(OrchestraPlan plan, RuntimeSummary runtime)
    {
        var approvalMode = string.Equals(plan.RiskLevel, "medium", StringComparison.OrdinalIgnoreCase)
            || string.Equals(plan.ExecutionReadiness, "review_first", StringComparison.OrdinalIgnoreCase)
            ? "operator_confirmation"
            : "none";
        var revisionPayload = JsonSerializer.Serialize(new
        {
            runtime.RuntimeId,
            runtime.Endpoint,
            runtime.SidecarEndpoint,
            plan.PlanId,
            plan.Intent,
            plan.RiskLevel,
            plan.ExecutionReadiness,
            plan.ExecutionMode,
            Reasons = plan.Reasons.OrderBy(static value => value, StringComparer.Ordinal).ToArray(),
            Capabilities = plan.RequiredCapabilities.OrderBy(static value => value, StringComparer.Ordinal).ToArray(),
            Steps = plan.Steps.Select(step => new { step.Key, step.Kind }).ToArray(),
        });
        var revision = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(revisionPayload))).ToLowerInvariant()[..16];
        return plan with { ApprovalMode = approvalMode, Revision = revision };
    }

    private static OrchestraPlan BuildRuntimeTriagePlan(
        RuntimeSummary runtime,
        IReadOnlyList<string> reasons,
        string severity,
        bool needsAttention)
    {
        var title = needsAttention
            ? "Triage runtime posture before deeper action"
            : "Validate runtime posture before orchestration";
        var summary = needsAttention
            ? $"Start with status refresh and attention review because this runtime is currently marked {severity}."
            : "Refresh the runtime surface and confirm the current snapshot before launching any higher-level workflow.";

        return new OrchestraPlan(
            "runtime_triage",
            "triage",
            title,
            summary,
            needsAttention ? "low_to_medium" : "low",
            "ready_now",
            "automatic",
            reasons,
            Array.Empty<string>(),
            new[]
            {
                new OrchestraPlanStep("refresh_status", "Refresh runtime status", "Re-fetch the current runtime posture so the control plane is not acting on stale data.", "refresh"),
                new OrchestraPlanStep("review_attention", "Review attention reasons", "Inspect the runtime attention reasons and recent recovery history before choosing a disruptive path.", "review"),
                new OrchestraPlanStep("open_detail", "Open runtime detail", "Use the runtime detail pane to confirm snapshot, resilience, and operator-facing artifacts.", "inspect"),
            },
            new[]
            {
                new OrchestraSuggestedSurface("Runtime detail", $"/?tab=runtimes&runtimeId={Uri.EscapeDataString(runtime.RuntimeId)}"),
                new OrchestraSuggestedSurface("Runtime attention", $"/?tab=runtimes&runtimeId={Uri.EscapeDataString(runtime.RuntimeId)}&runtimeMainTab=detail"),
            });
    }

    private static OrchestraPlan BuildAnalysisRecoveryPlan(RuntimeSummary runtime, IReadOnlyList<string> reasons)
    {
        var missingSnapshot = reasons.Contains("no_latest_snapshot", StringComparer.OrdinalIgnoreCase);
        var missingAnalysis = reasons.Contains("no_analysis_json", StringComparer.OrdinalIgnoreCase);
        var summary = missingSnapshot || missingAnalysis
            ? "This runtime is missing one or more analysis artifacts, so the safest path is to refresh the full runtime surface and verify that summary/analysis outputs return together."
            : "Use this path when you want a clean runtime evidence pass before session launch, policy review, or UI handoff.";

        return new OrchestraPlan(
            "analysis_recovery",
            "recover_analysis",
            "Recover analysis-ready runtime evidence",
            summary,
            missingSnapshot || missingAnalysis ? "medium" : "low_to_medium",
            "ready_now",
            "automatic",
            reasons.Where(reason =>
                string.Equals(reason, "no_latest_snapshot", StringComparison.OrdinalIgnoreCase) ||
                string.Equals(reason, "no_analysis_json", StringComparison.OrdinalIgnoreCase))
                .ToArray(),
            Array.Empty<string>(),
            new[]
            {
                new OrchestraPlanStep("refresh_all", "Refresh the full runtime surface", "Ask the control plane to refresh capabilities, status, and any sidecar-linked posture in one pass.", "refresh"),
                new OrchestraPlanStep("verify_artifacts", "Verify summary and analysis artifacts", "Confirm that summary/analysis evidence returned before treating this runtime as ready for orchestration.", "verify"),
                new OrchestraPlanStep("handoff_to_runtime_panel", "Hand off to child panel", "Open the runtime child panel only after evidence is present so the UI is anchored to current artifacts.", "handoff"),
            },
            new[]
            {
                new OrchestraSuggestedSurface("Runtimes workspace", $"/?tab=runtimes&runtimeId={Uri.EscapeDataString(runtime.RuntimeId)}"),
                new OrchestraSuggestedSurface("Child panel", $"/?tab=runtimes&runtimeId={Uri.EscapeDataString(runtime.RuntimeId)}&runtimeMainTab=panel"),
            });
    }

    private static OrchestraPlan BuildSidecarCoordinationPlan(RuntimeSummary runtime, IReadOnlyList<string> reasons)
    {
        var sidecarFetchFailed = reasons.Contains("sidecar_status_fetch_failed", StringComparer.OrdinalIgnoreCase);
        var readiness = runtime.SidecarStatus?.Healthy == true ? "ready_now" : "review_first";

        return new OrchestraPlan(
            "sidecar_coordination",
            "coordinate_sidecar",
            "Coordinate runtime and sidecar diagnostic posture",
            sidecarFetchFailed
                ? "The sidecar path needs its own recovery pass before you trust merged enrichments or diagnostic opinion."
                : "Use this path to compare runtime-native posture with sidecar memory, enrichment, and diagnostic overlays.",
            sidecarFetchFailed ? "medium" : "low_to_medium",
            readiness,
            "automatic",
            reasons.Where(reason => string.Equals(reason, "sidecar_status_fetch_failed", StringComparison.OrdinalIgnoreCase)).ToArray(),
            new[] { "sidecar_status" },
            new[]
            {
                new OrchestraPlanStep("refresh_sidecar", "Refresh sidecar posture", "Fetch sidecar status and memory posture without disturbing the main runtime intake path.", "refresh"),
                new OrchestraPlanStep("compare_sources", "Compare runtime and sidecar signals", "Check whether external context, enrichment, and diagnostic opinion line up with runtime-native status.", "review"),
                new OrchestraPlanStep("decide_handoff", "Decide whether to trust merged analysis", "Use the comparison to decide whether the child panel should open merged or runtime-native views first.", "decision"),
            },
            new[]
            {
                new OrchestraSuggestedSurface("Runtime detail", $"/?tab=runtimes&runtimeId={Uri.EscapeDataString(runtime.RuntimeId)}&runtimeMainTab=detail"),
                new OrchestraSuggestedSurface("Child panel", $"/?tab=runtimes&runtimeId={Uri.EscapeDataString(runtime.RuntimeId)}&runtimeMainTab=panel"),
            });
    }

    private static OrchestraPlan BuildSessionPreparationPlan(RuntimeSummary runtime)
    {
        var supportedCapabilities = runtime.Capabilities
            .Where(capability => string.Equals(capability.Support, "fully_supported", StringComparison.OrdinalIgnoreCase))
            .Select(capability => capability.Key)
            .OrderBy(static key => key, StringComparer.OrdinalIgnoreCase)
            .ToArray();

        var readiness = supportedCapabilities.Length > 0 ? "ready_now" : "review_first";
        var summary = supportedCapabilities.Length > 0
            ? $"This runtime currently advertises {supportedCapabilities.Length} fully-supported capabilities that can be used to preflight a session request."
            : "This runtime does not currently advertise any fully-supported capabilities, so session planning should wait for a cleaner capability refresh.";

        return new OrchestraPlan(
            "session_preparation",
            "prepare_session",
            "Prepare a session-oriented orchestration handoff",
            summary,
            supportedCapabilities.Length > 0 ? "low" : "medium",
            readiness,
            "guided",
            Array.Empty<string>(),
            supportedCapabilities,
            new[]
            {
                new OrchestraPlanStep("choose_pipeline_kind", "Choose a pipeline kind", "Select the narrowest pipeline kind that matches the operator goal instead of jumping to a broad session.", "planning"),
                new OrchestraPlanStep("validate_capability_fit", "Validate capability fit", "Compare requested session requirements with the runtime capability surface before creating the session.", "verify"),
                new OrchestraPlanStep("handoff_to_sessions", "Hand off into sessions", "Create or queue the session only after capability fit and runtime posture both look acceptable.", "handoff"),
            },
            new[]
            {
                new OrchestraSuggestedSurface("Sessions", "/?tab=sessions"),
                new OrchestraSuggestedSurface("Runtime register/detail", $"/?tab=runtimes&runtimeId={Uri.EscapeDataString(runtime.RuntimeId)}"),
            });
    }
}
