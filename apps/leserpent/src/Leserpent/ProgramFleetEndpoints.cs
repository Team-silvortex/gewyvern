using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Mvc;

namespace Leserpent;

public partial class Program
{
    private static void MapFleetEndpoints(WebApplication app)
    {
        app.MapGet("/v1/fleet/summary", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            FleetReadProjectionService fleetReads,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            try
            {
                return Results.Ok(new FleetSummaryResponse(
                    filter,
                    await fleetReads.GetSummaryAsync(filter, cancellationToken)));
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
        });

        app.MapGet("/v1/fleet/runtimes-needing-attention", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            FleetReadProjectionService fleetReads,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            try
            {
                return Results.Ok(new FleetAttentionListResponse(
                    filter,
                    await fleetReads.GetRuntimesNeedingAttentionAsync(filter, cancellationToken)));
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
        });

        app.MapGet("/v1/fleet/attention-summary", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            FleetReadProjectionService fleetReads,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            try
            {
                return Results.Ok(new FleetAttentionSummaryResponse(
                    filter,
                    await fleetReads.GetAttentionSummaryAsync(filter, cancellationToken)));
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
        });

        app.MapPost("/v1/fleet/refresh-all", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RegistryService registry,
            RuntimeCommandExecutionContextService commandContexts,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetRefreshAllItem>();
            IReadOnlyList<RuntimeCommandExecutionContext> contexts;
            try
            {
                contexts = await commandContexts.ListAsync(filter, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
            foreach (var context in contexts)
            {
                var runtime = context.Runtime;
                var runtimeAdminToken = context.ControlAccess.AdminToken;
                var capabilityDiscovery = await discovery.DiscoverAsync(
                    runtime.Endpoint,
                    null,
                    cancellationToken,
                    runtimeAdminToken);
                var statusDiscovery = await discovery.DiscoverStatusAsync(
                    runtime.Endpoint,
                    null,
                    cancellationToken,
                    runtimeAdminToken);
                var sidecarAccess = context.SidecarAccess;
                var sidecarDiscovery = sidecarAccess is null
                    ? null
                    : await discovery.DiscoverSidecarStatusAsync(
                        sidecarAccess.SidecarEndpoint,
                        null,
                        sidecarAccess.SidecarAdminToken,
                        cancellationToken);
                RuntimeDiscoveryCommit commit;
                try
                {
                    commit = await commandContexts.CommitDiscoveryAsync(
                        context,
                        registrationAuthority,
                        cancellationToken,
                        capabilityDiscovery,
                        statusDiscovery,
                        sidecarDiscovery);
                }
                catch (DaemonRuntimeRegistrationException ex)
                {
                    return RuntimeRegistrationAuthorityFailure(ex, runtime.RuntimeId);
                }
                runtime = commit.Context.Runtime;
                capabilityDiscovery = commit.CapabilityDiscovery!;
                statusDiscovery = commit.StatusDiscovery!;
                sidecarDiscovery = commit.SidecarDiscovery;

                var capabilityResult = registry.RefreshRuntimeCapabilities(
                    runtime.RuntimeId,
                    capabilityDiscovery);
                var statusResult = registry.RefreshRuntimeStatus(
                    runtime.RuntimeId,
                    statusDiscovery);
                if (capabilityResult is not null && statusResult is not null)
                {
                    var sidecarResult = sidecarDiscovery is null
                        ? null
                        : registry.RefreshRuntimeSidecar(runtime.RuntimeId, sidecarDiscovery);

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
                        runtime.RuntimeId,
                        runtime.Name,
                        runtime.Endpoint,
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

        app.MapPost("/v1/fleet/refresh-capabilities", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RegistryService registry,
            RuntimeCommandExecutionContextService commandContexts,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetCapabilityRefreshItem>();
            IReadOnlyList<RuntimeCommandExecutionContext> contexts;
            try
            {
                contexts = await commandContexts.ListAsync(filter, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
            foreach (var context in contexts)
            {
                var runtime = context.Runtime;
                var capabilityDiscovery = await discovery.DiscoverAsync(
                    runtime.Endpoint,
                    null,
                    cancellationToken,
                    context.ControlAccess.AdminToken);
                RuntimeDiscoveryCommit commit;
                try
                {
                    commit = await commandContexts.CommitDiscoveryAsync(
                        context,
                        registrationAuthority,
                        cancellationToken,
                        capabilityDiscovery: capabilityDiscovery);
                }
                catch (DaemonRuntimeRegistrationException ex)
                {
                    return RuntimeRegistrationAuthorityFailure(ex, runtime.RuntimeId);
                }
                runtime = commit.Context.Runtime;
                capabilityDiscovery = commit.CapabilityDiscovery!;
                var result = registry.RefreshRuntimeCapabilities(
                    runtime.RuntimeId,
                    capabilityDiscovery);
                if (result is not null)
                {
                    refreshed.Add(new FleetCapabilityRefreshItem(
                        runtime.RuntimeId,
                        runtime.Name,
                        runtime.Endpoint,
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
            RuntimeCommandExecutionContextService commandContexts,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetSidecarRefreshItem>();
            IReadOnlyList<RuntimeCommandExecutionContext> contexts;
            try
            {
                contexts = await commandContexts.ListAsync(filter, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
            foreach (var context in contexts)
            {
                var runtime = context.Runtime;
                var sidecarAccess = context.SidecarAccess;
                if (sidecarAccess is null)
                {
                    continue;
                }

                var sidecarDiscovery = await discovery.DiscoverSidecarStatusAsync(
                    sidecarAccess.SidecarEndpoint,
                    null,
                    sidecarAccess.SidecarAdminToken,
                    cancellationToken);
                RuntimeDiscoveryCommit commit;
                try
                {
                    commit = await commandContexts.CommitDiscoveryAsync(
                        context,
                        registrationAuthority,
                        cancellationToken,
                        sidecarDiscovery: sidecarDiscovery);
                }
                catch (DaemonRuntimeRegistrationException ex)
                {
                    return RuntimeRegistrationAuthorityFailure(ex, runtime.RuntimeId);
                }
                runtime = commit.Context.Runtime;
                sidecarDiscovery = commit.SidecarDiscovery!;
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
                        runtime.RuntimeId,
                        runtime.Name,
                        runtime.Endpoint,
                        runtime.SidecarEndpoint,
                        runtime.Tags,
                        result.SidecarStatus));
                }
            }

            return Results.Ok(new FleetSidecarRefreshEnvelope(
                filter,
                new FleetSidecarRefreshResponse(refreshed.Count, refreshed)));
        });

        app.MapPost("/v1/fleet/refresh-status", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RegistryService registry,
            RuntimeCommandExecutionContextService commandContexts,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            var refreshed = new List<FleetStatusRefreshItem>();
            IReadOnlyList<RuntimeCommandExecutionContext> contexts;
            try
            {
                contexts = await commandContexts.ListAsync(filter, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
            foreach (var context in contexts)
            {
                var runtime = context.Runtime;
                var statusDiscovery = await discovery.DiscoverStatusAsync(
                    runtime.Endpoint,
                    null,
                    cancellationToken,
                    context.ControlAccess.AdminToken);
                RuntimeDiscoveryCommit commit;
                try
                {
                    commit = await commandContexts.CommitDiscoveryAsync(
                        context,
                        registrationAuthority,
                        cancellationToken,
                        statusDiscovery: statusDiscovery);
                }
                catch (DaemonRuntimeRegistrationException ex)
                {
                    return RuntimeRegistrationAuthorityFailure(ex, runtime.RuntimeId);
                }
                runtime = commit.Context.Runtime;
                statusDiscovery = commit.StatusDiscovery!;
                var result = registry.RefreshRuntimeStatus(runtime.RuntimeId, statusDiscovery);
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
                        runtime.RuntimeId,
                        runtime.Name,
                        runtime.Endpoint,
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
