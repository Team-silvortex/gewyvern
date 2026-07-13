using Leserpent.ControlPlane;

namespace Leserpent;

public interface IOrchestraPlanExecutor
{
    Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteAsync(
        string planId,
        RuntimeSummary runtime,
        CancellationToken cancellationToken);
}

public sealed class OrchestraPlanExecutor(
    RegistryService registry,
    CapabilityDiscoveryService discovery) : IOrchestraPlanExecutor
{
    public async Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteAsync(
        string planId,
        RuntimeSummary runtime,
        CancellationToken cancellationToken)
    {
        var results = new List<OrchestraExecutionStepResult>();

        if (string.Equals(planId, "analysis_recovery", StringComparison.OrdinalIgnoreCase))
        {
            var capabilities = registry.RefreshRuntimeCapabilities(
                runtime.RuntimeId,
                await discovery.DiscoverAsync(runtime.Endpoint, null, cancellationToken, registry.GetRuntimeControlAccess(runtime.RuntimeId)?.AdminToken));
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
                await discovery.DiscoverStatusAsync(runtime.Endpoint, null, cancellationToken, registry.GetRuntimeControlAccess(runtime.RuntimeId)?.AdminToken));
            var error = status?.Status.StatusFetchError;
            var outcome = status is null
                ? "degraded"
                : Program.DetermineRefreshOutcome(status.Status.StatusSource, error, null, null);
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
                    : Program.DetermineRefreshOutcome(null, null, sidecar.SidecarStatus?.StatusSource, error);
                results.Add(new OrchestraExecutionStepResult(
                    "refresh_sidecar",
                    outcome,
                    error ?? (sidecar is null ? "runtime unavailable during sidecar refresh" : "sidecar status refreshed")));
            }
        }

        return results;
    }
}
