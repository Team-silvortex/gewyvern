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

        app.MapPost("/v1/orchestra/plans/{id}/{planId}/execute", async Task<IResult> (
            string id,
            string planId,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
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

            var steps = await ExecuteOrchestraPlanAsync(planId, runtime, registry, discovery, cancellationToken);
            var currentRuntime = registry.GetRuntime(id);
            if (currentRuntime is null)
            {
                return Results.Conflict(new { error = "runtime_removed_during_orchestra_execution", runtimeId = id, planId });
            }
            var currentAttention = registry.GetRuntimeAttention(id);
            var currentReasons = currentAttention?.Reasons ?? Array.Empty<string>();
            var currentPlans = OrchestraPlanner.Build(
                currentRuntime,
                currentReasons,
                currentAttention?.Severity ?? "none",
                currentAttention?.NeedsAttention ?? false);
            var outcome = steps.All(step => string.Equals(step.Outcome, "ok", StringComparison.OrdinalIgnoreCase))
                ? "ok"
                : "degraded";

            registry.RecordRecoveryActivity(id, $"orchestra:{planId}", outcome, string.Join("; ", steps.Select(step => step.Summary)));
            var run = registry.RecordOrchestraRun(id, planId, outcome, steps);
            return Results.Ok(new OrchestraExecutionResponse(
                run.RunId,
                id,
                planId,
                outcome,
                run.ExecutedAt,
                steps,
                BuildOrchestraResponse(currentRuntime, currentAttention, currentReasons, currentPlans)));
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
            var run = registry.RecordOrchestraRun(id, "session_preparation", "ok", steps);
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

    private static async Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteOrchestraPlanAsync(
        string planId,
        RuntimeSummary runtime,
        RegistryService registry,
        CapabilityDiscoveryService discovery,
        CancellationToken cancellationToken)
    {
        var results = new List<OrchestraExecutionStepResult>();

        if (string.Equals(planId, "analysis_recovery", StringComparison.OrdinalIgnoreCase))
        {
            var capabilities = registry.RefreshRuntimeCapabilities(
                runtime.RuntimeId,
                await discovery.DiscoverAsync(runtime.Endpoint, null, cancellationToken));
            results.Add(new OrchestraExecutionStepResult(
                "refresh_capabilities",
                capabilities is not null && capabilities.CapabilityFetchError is null ? "ok" : "degraded",
                capabilities?.CapabilityFetchError ?? (capabilities is null ? "runtime unavailable during capability refresh" : "capabilities refreshed")));
        }

        if (string.Equals(planId, "runtime_triage", StringComparison.OrdinalIgnoreCase)
            || string.Equals(planId, "analysis_recovery", StringComparison.OrdinalIgnoreCase))
        {
            var status = registry.RefreshRuntimeStatus(
                runtime.RuntimeId,
                await discovery.DiscoverStatusAsync(runtime.Endpoint, null, cancellationToken));
            var error = status?.Status.StatusFetchError;
            var outcome = status is null
                ? "degraded"
                : DetermineRefreshOutcome(status.Status.StatusSource, error, null, null);
            results.Add(new OrchestraExecutionStepResult(
                "refresh_status",
                outcome,
                error ?? (status is null ? "runtime unavailable during status refresh" : "runtime status refreshed")));
        }

        if (string.Equals(planId, "sidecar_coordination", StringComparison.OrdinalIgnoreCase)
            || (string.Equals(planId, "analysis_recovery", StringComparison.OrdinalIgnoreCase)
                && !string.IsNullOrWhiteSpace(runtime.SidecarEndpoint)))
        {
            var sidecarAccess = registry.GetRuntimeSidecarAccess(runtime.RuntimeId);
            if (sidecarAccess is not null)
            {
                var sidecar = registry.RefreshRuntimeSidecar(
                    runtime.RuntimeId,
                    await discovery.DiscoverSidecarStatusAsync(
                        sidecarAccess.SidecarEndpoint,
                        null,
                        sidecarAccess.SidecarAdminToken,
                        cancellationToken));
                var error = sidecar?.SidecarStatus?.StatusFetchError;
                var outcome = sidecar is null
                    ? "degraded"
                    : DetermineRefreshOutcome(null, null, sidecar.SidecarStatus?.StatusSource, error);
                results.Add(new OrchestraExecutionStepResult(
                    "refresh_sidecar",
                    outcome,
                    error ?? (sidecar is null ? "runtime unavailable during sidecar refresh" : "sidecar status refreshed")));
            }
        }

        return results;
    }
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
        return plans;
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
