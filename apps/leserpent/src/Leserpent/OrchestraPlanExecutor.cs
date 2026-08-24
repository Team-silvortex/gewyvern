using Leserpent.ControlPlane;

namespace Leserpent;

public interface IOrchestraPlanExecutor
{
    Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteAsync(
        string planId,
        RuntimeSummary runtime,
        CancellationToken cancellationToken);
}

internal sealed class OrchestraPlanExecutor(
    RegistryService registry,
    RuntimeCommandExecutionContextService commandContexts,
    CapabilityDiscoveryService discovery,
    IRuntimeRegistrationAuthority registrationAuthority) : IOrchestraPlanExecutor
{
    public async Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteAsync(
        string planId,
        RuntimeSummary runtime,
        CancellationToken cancellationToken)
    {
        var results = new List<OrchestraExecutionStepResult>();
        var context = await commandContexts.InspectAsync(
            runtime.RuntimeId,
            cancellationToken);
        if (context is null)
        {
            return new[]
            {
                new OrchestraExecutionStepResult(
                    "resolve_runtime",
                    "failed",
                    "runtime unavailable before Orchestra execution"),
            };
        }
        runtime = context.Runtime;

        var analysisRecovery = string.Equals(
            planId,
            "analysis_recovery",
            StringComparison.OrdinalIgnoreCase);
        var refreshCapabilities = analysisRecovery;
        var refreshStatus = analysisRecovery || string.Equals(
            planId,
            "runtime_triage",
            StringComparison.OrdinalIgnoreCase);
        var refreshSidecar = context.SidecarAccess is not null
            && (analysisRecovery || string.Equals(
                planId,
                "sidecar_coordination",
                StringComparison.OrdinalIgnoreCase));

        var capabilityDiscovery = refreshCapabilities
            ? await discovery.DiscoverAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                context.ControlAccess.AdminToken)
            : null;
        var statusDiscovery = refreshStatus
            ? await discovery.DiscoverStatusAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                context.ControlAccess.AdminToken)
            : null;
        var sidecarDiscovery = refreshSidecar
            ? await discovery.DiscoverSidecarStatusAsync(
                context.SidecarAccess!.SidecarEndpoint,
                null,
                context.SidecarAccess.SidecarAdminToken,
                cancellationToken)
            : null;

        await registrationAuthority.SubmitDiscoveryAtRevisionAsync(
            runtime.RuntimeId,
            context.AuthorityRevision,
            cancellationToken,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery);

        if (capabilityDiscovery is not null)
        {
            var capabilities = registry.RefreshRuntimeCapabilities(
                runtime.RuntimeId,
                capabilityDiscovery);
            results.Add(new OrchestraExecutionStepResult(
                "refresh_capabilities",
                capabilities is not null && capabilities.CapabilityFetchError is null ? "ok" : "degraded",
                capabilities?.CapabilityFetchError ?? (capabilities is null ? "runtime unavailable during capability refresh" : "capabilities refreshed")));
        }
        if (statusDiscovery is not null)
        {
            var status = registry.RefreshRuntimeStatus(
                runtime.RuntimeId,
                statusDiscovery);
            var error = status?.Status.StatusFetchError;
            var outcome = status is null
                ? "degraded"
                : Program.DetermineRefreshOutcome(status.Status.StatusSource, error, null, null);
            results.Add(new OrchestraExecutionStepResult(
                "refresh_status",
                outcome,
                error ?? (status is null ? "runtime unavailable during status refresh" : "runtime status refreshed")));
        }
        if (sidecarDiscovery is not null)
        {
            var sidecar = registry.RefreshRuntimeSidecar(
                runtime.RuntimeId,
                sidecarDiscovery);
            var error = sidecar?.SidecarStatus?.StatusFetchError;
            var outcome = sidecar is null
                ? "degraded"
                : Program.DetermineRefreshOutcome(null, null, sidecar.SidecarStatus?.StatusSource, error);
            results.Add(new OrchestraExecutionStepResult(
                "refresh_sidecar",
                outcome,
                error ?? (sidecar is null ? "runtime unavailable during sidecar refresh" : "sidecar status refreshed")));
        }

        return results;
    }
}
