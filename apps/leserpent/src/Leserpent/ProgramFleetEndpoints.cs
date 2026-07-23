using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Mvc;

namespace Leserpent;

public partial class Program
{
    private static void MapFleetEndpoints(WebApplication app)
    {
        app.MapGet("/v1/fleet/summary", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
            Results.Ok(new FleetSummaryResponse(
                new RuntimeListFilter(environmentTag, cluster, role),
                registry.GetFleetSummary(new RuntimeListFilter(environmentTag, cluster, role)))));

        app.MapGet("/v1/fleet/runtimes-needing-attention", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
            Results.Ok(new FleetAttentionListResponse(
                new RuntimeListFilter(environmentTag, cluster, role),
                registry.GetRuntimesNeedingAttention(new RuntimeListFilter(environmentTag, cluster, role)))));

        app.MapGet("/v1/fleet/attention-summary", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
            Results.Ok(new FleetAttentionSummaryResponse(
                new RuntimeListFilter(environmentTag, cluster, role),
                registry.GetFleetAttentionSummary(new RuntimeListFilter(environmentTag, cluster, role)))));

        app.MapPost("/v1/fleet/refresh-all", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetRefreshAllItem>();
            foreach (var runtime in registry.ListRuntimes(filter))
            {
                var runtimeAdminToken = registry.GetRuntimeControlAccess(runtime.RuntimeId)?.AdminToken;
                var capabilityResult = registry.RefreshRuntimeCapabilities(
                    runtime.RuntimeId,
                    await discovery.DiscoverAsync(runtime.Endpoint, null, cancellationToken, runtimeAdminToken));
                var statusResult = registry.RefreshRuntimeStatus(
                    runtime.RuntimeId,
                    await discovery.DiscoverStatusAsync(runtime.Endpoint, null, cancellationToken, runtimeAdminToken));

                if (capabilityResult is not null && statusResult is not null)
                {
                    var sidecarAccess = registry.GetRuntimeSidecarAccess(runtime.RuntimeId);
                    RuntimeSidecarRefreshResponse? sidecarResult = null;
                    if (sidecarAccess is not null)
                    {
                        var sidecarDiscovery = await discovery.DiscoverSidecarStatusAsync(
                            sidecarAccess.SidecarEndpoint,
                            null,
                            sidecarAccess.SidecarAdminToken,
                            cancellationToken);
                        try
                        {
                            if (sidecarDiscovery.SidecarStatus is not null)
                            {
                                await registrationAuthority.SubmitSidecarStatusAsync(
                                    runtime.RuntimeId,
                                    sidecarDiscovery.SidecarStatus,
                                    cancellationToken);
                            }
                        }
                        catch (DaemonRuntimeRegistrationException ex)
                        {
                            return RuntimeRegistrationAuthorityFailure(ex, runtime.RuntimeId);
                        }
                        sidecarResult = registry.RefreshRuntimeSidecar(
                            runtime.RuntimeId,
                            sidecarDiscovery);
                    }

                    registry.RecordRecoveryActivity(
                        runtime.RuntimeId,
                        "refresh_all",
                        DetermineRefreshOutcome(
                            statusResult.Status.StatusSource,
                            statusResult.Status.StatusFetchError,
                            sidecarResult?.SidecarStatus?.StatusSource,
                            sidecarResult?.SidecarStatus?.StatusFetchError),
                        BuildRecoverySummary(
                            statusResult.Status.StatusSource,
                            statusResult.Status.StatusFetchError,
                            sidecarResult?.SidecarStatus?.StatusSource,
                            sidecarResult?.SidecarStatus?.StatusFetchError));

                    refreshed.Add(new FleetRefreshAllItem(
                        capabilityResult.RuntimeId,
                        capabilityResult.Name,
                        capabilityResult.Endpoint,
                        runtime.SidecarEndpoint,
                        runtime.HasSidecarAdminToken,
                        runtime.Tags,
                        capabilityResult.Capabilities,
                        capabilityResult.CapabilitySource,
                        capabilityResult.CapabilityFetchedAt,
                        capabilityResult.CapabilityFetchError,
                        statusResult.Status,
                        sidecarResult?.SidecarStatus));
                }
            }

            return Results.Ok(new FleetRefreshAllEnvelope(
                filter,
                new FleetRefreshAllResponse(refreshed.Count, refreshed)));
        });

        app.MapPost("/v1/fleet/refresh-capabilities", async ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetCapabilityRefreshItem>();
            foreach (var runtime in registry.ListRuntimes(filter))
            {
                var result = registry.RefreshRuntimeCapabilities(
                    runtime.RuntimeId,
                    await discovery.DiscoverAsync(runtime.Endpoint, null, cancellationToken, registry.GetRuntimeControlAccess(runtime.RuntimeId)?.AdminToken));
                if (result is not null)
                {
                    refreshed.Add(new FleetCapabilityRefreshItem(
                        result.RuntimeId,
                        result.Name,
                        result.Endpoint,
                        runtime.Tags,
                        result.Capabilities,
                        result.CapabilitySource,
                        result.CapabilityFetchedAt,
                        result.CapabilityFetchError));
                }
            }

            return Results.Ok(new FleetCapabilityRefreshEnvelope(
                filter,
                new FleetCapabilityRefreshResponse(refreshed.Count, refreshed)));
        });

        app.MapPost("/v1/fleet/refresh-sidecars", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetSidecarRefreshItem>();
            foreach (var runtime in registry.ListRuntimes(filter))
            {
                var sidecarAccess = registry.GetRuntimeSidecarAccess(runtime.RuntimeId);
                if (sidecarAccess is null)
                {
                    continue;
                }

                var sidecarDiscovery = await discovery.DiscoverSidecarStatusAsync(
                    sidecarAccess.SidecarEndpoint,
                    null,
                    sidecarAccess.SidecarAdminToken,
                    cancellationToken);
                try
                {
                    if (sidecarDiscovery.SidecarStatus is not null)
                    {
                        await registrationAuthority.SubmitSidecarStatusAsync(
                            runtime.RuntimeId,
                            sidecarDiscovery.SidecarStatus,
                            cancellationToken);
                    }
                }
                catch (DaemonRuntimeRegistrationException ex)
                {
                    return RuntimeRegistrationAuthorityFailure(ex, runtime.RuntimeId);
                }
                var result = registry.RefreshRuntimeSidecar(runtime.RuntimeId, sidecarDiscovery);
                if (result is not null)
                {
                    registry.RecordRecoveryActivity(
                        runtime.RuntimeId,
                        "refresh_sidecar",
                        DetermineRefreshOutcome(
                            null,
                            null,
                            result.SidecarStatus?.StatusSource,
                            result.SidecarStatus?.StatusFetchError),
                        BuildRecoverySummary(
                            null,
                            null,
                            result.SidecarStatus?.StatusSource,
                            result.SidecarStatus?.StatusFetchError));
                    refreshed.Add(new FleetSidecarRefreshItem(
                        result.RuntimeId,
                        result.Name,
                        result.Endpoint,
                        result.SidecarEndpoint,
                        runtime.Tags,
                        result.SidecarStatus));
                }
            }

            return Results.Ok(new FleetSidecarRefreshEnvelope(
                filter,
                new FleetSidecarRefreshResponse(refreshed.Count, refreshed)));
        });

        app.MapPost("/v1/fleet/refresh-status", async ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetStatusRefreshItem>();
            foreach (var runtime in registry.ListRuntimes(filter))
            {
                var result = registry.RefreshRuntimeStatus(
                    runtime.RuntimeId,
                    await discovery.DiscoverStatusAsync(runtime.Endpoint, null, cancellationToken, registry.GetRuntimeControlAccess(runtime.RuntimeId)?.AdminToken));
                if (result is not null)
                {
                    registry.RecordRecoveryActivity(
                        runtime.RuntimeId,
                        "refresh_status",
                        DetermineRefreshOutcome(
                            result.Status.StatusSource,
                            result.Status.StatusFetchError,
                            null,
                            null),
                        BuildRecoverySummary(
                            result.Status.StatusSource,
                            result.Status.StatusFetchError,
                            null,
                            null));
                    refreshed.Add(new FleetStatusRefreshItem(
                        result.RuntimeId,
                        result.Name,
                        result.Endpoint,
                        runtime.Tags,
                        result.Status));
                }
            }

            return Results.Ok(new FleetStatusRefreshEnvelope(
                filter,
                new FleetStatusRefreshResponse(refreshed.Count, refreshed)));
        });
    }
}
