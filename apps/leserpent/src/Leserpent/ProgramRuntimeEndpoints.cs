using Leserpent.ControlPlane;

namespace Leserpent;

public partial class Program
{
    private static void MapRuntimeEndpoints(WebApplication app)
    {
        app.MapGet("/v1/runtimes", (string? environment, string? cluster, string? role, RegistryService registry) =>
            Results.Ok(new
            {
                filter = new RuntimeListFilter(environment, cluster, role),
                runtimes = registry.ListRuntimes(new RuntimeListFilter(environment, cluster, role)),
            }));

        app.MapGet("/v1/runtimes/{id}", (string id, RegistryService registry) =>
        {
            var runtime = registry.GetRuntime(id);
            return runtime is null ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id }) : Results.Ok(runtime);
        });

        app.MapGet("/v1/runtimes/{id}/attention", (string id, RegistryService registry) =>
        {
            var attention = registry.GetRuntimeAttention(id);
            return attention is null
                ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id })
                : Results.Ok(attention);
        });

        app.MapGet("/v1/runtimes/{id}/status", (string id, RegistryService registry) =>
        {
            var runtime = registry.GetRuntime(id);
            return runtime is null
                ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id })
                : Results.Ok(new RuntimeStatusRefreshResponse(runtime.RuntimeId, runtime.Name, runtime.Endpoint, runtime.Status));
        });

        app.MapGet("/v1/runtimes/{id}/sidecar", (string id, RegistryService registry) =>
        {
            var runtime = registry.GetRuntime(id);
            return runtime is null
                ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id })
                : Results.Ok(new RuntimeSidecarRefreshResponse(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    runtime.SidecarEndpoint,
                    runtime.HasSidecarAdminToken,
                    runtime.SidecarStatus));
        });

        app.MapGet("/v1/runtimes/{id}/protocol-reading", async Task<IResult> (string id, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }

            try
            {
                var reading = await discovery.DiscoverProtocolReadingAsync(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    cancellationToken);
                return reading is null
                    ? Results.NotFound(new { error = "protocol_reading_unavailable", runtimeId = id })
                    : Results.Ok(reading);
            }
            catch (Exception ex)
            {
                return Results.BadRequest(new { error = "protocol_reading_unavailable", runtimeId = id, reason = ex.Message });
            }
        });

        app.MapPost("/v1/runtimes/register", async (RuntimeRegistrationRequest request, RegistryService registry, CapabilityDiscoveryService discovery, ControlPlaneSecurityPolicy security, CancellationToken cancellationToken) =>
        {
            if (string.IsNullOrWhiteSpace(request.Name) || string.IsNullOrWhiteSpace(request.Endpoint))
            {
                return Results.BadRequest(new
                {
                    error = "invalid_runtime_registration",
                    reason = "name and endpoint are required",
                });
            }

            var registrationValidation = await security.ValidateRegistrationAsync(request, cancellationToken);
            if (registrationValidation is not null)
            {
                return Results.BadRequest(new
                {
                    error = "invalid_runtime_registration",
                    reason = registrationValidation,
                });
            }

            if (request.FetchCapabilities)
            {
                var capabilityDiscovery = await discovery.DiscoverAsync(request.Endpoint, request.CapabilityEndpoint, cancellationToken);
                var statusDiscovery = await discovery.DiscoverStatusAsync(request.Endpoint, request.StatusEndpoint, cancellationToken);
                var sidecarDiscovery = string.IsNullOrWhiteSpace(request.SidecarEndpoint)
                    ? null
                    : await discovery.DiscoverSidecarStatusAsync(request.SidecarEndpoint!, request.SidecarStatusEndpoint, request.SidecarAdminToken, cancellationToken);
                var registered = registry.RegisterRuntimeFromDiscovery(request, capabilityDiscovery, statusDiscovery, sidecarDiscovery);
                registry.RecordRecoveryActivity(
                    registered.RuntimeId,
                    "register_runtime",
                    DetermineRefreshOutcome(
                        registered.Status.StatusSource,
                        registered.Status.StatusFetchError,
                        registered.SidecarStatus?.StatusSource,
                        registered.SidecarStatus?.StatusFetchError),
                    "runtime registered through discovery");
                return Results.Ok(registered);
            }

            var manualRegistered = registry.RegisterRuntime(request);
            registry.RecordRecoveryActivity(
                manualRegistered.RuntimeId,
                "register_runtime",
                "ok",
                "runtime registered with manual capability intake");
            return Results.Ok(manualRegistered);
        });

        app.MapPost("/v1/runtimes/{id}/refresh-capabilities", async (string id, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }

            var refreshed = registry.RefreshRuntimeCapabilities(
                id,
                await discovery.DiscoverAsync(runtime.Endpoint, null, cancellationToken));
            return refreshed is null
                ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id })
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/refresh-status", async (string id, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }

            var refreshed = registry.RefreshRuntimeStatus(
                id,
                await discovery.DiscoverStatusAsync(runtime.Endpoint, null, cancellationToken));
            if (refreshed is not null)
            {
                registry.RecordRecoveryActivity(
                    id,
                    "refresh_status",
                    DetermineRefreshOutcome(
                        refreshed.Status.StatusSource,
                        refreshed.Status.StatusFetchError,
                        null,
                        null),
                    BuildRecoverySummary(
                        refreshed.Status.StatusSource,
                        refreshed.Status.StatusFetchError,
                        null,
                        null));
            }
            return refreshed is null
                ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id })
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/refresh-sidecar", async (string id, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new { error = "runtime_not_found", runtimeId = id });
            }

            var sidecarAccess = registry.GetRuntimeSidecarAccess(id);
            if (sidecarAccess is null)
            {
                return Results.BadRequest(new { error = "runtime_has_no_sidecar_endpoint", runtimeId = id });
            }

            var refreshed = registry.RefreshRuntimeSidecar(
                id,
                await discovery.DiscoverSidecarStatusAsync(
                    sidecarAccess.SidecarEndpoint,
                    null,
                    sidecarAccess.SidecarAdminToken,
                    cancellationToken));
            if (refreshed is not null)
            {
                registry.RecordRecoveryActivity(
                    id,
                    "refresh_sidecar",
                    DetermineRefreshOutcome(
                        null,
                        null,
                        refreshed.SidecarStatus?.StatusSource,
                        refreshed.SidecarStatus?.StatusFetchError),
                    BuildRecoverySummary(
                        null,
                        null,
                        refreshed.SidecarStatus?.StatusSource,
                        refreshed.SidecarStatus?.StatusFetchError));
            }
            return refreshed is null
                ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id })
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/delete", (string id, RegistryService registry) =>
        {
            var deleted = registry.DeleteRuntime(id);
            return deleted.RemovedRuntime is null
                ? Results.NotFound(new { error = "runtime_not_found", runtimeId = id })
                : Results.Ok(new
                {
                    deleted = true,
                    runtimeId = deleted.RemovedRuntime.RuntimeId,
                    name = deleted.RemovedRuntime.Name,
                    endpoint = deleted.RemovedRuntime.Endpoint,
                    removedSessionCount = deleted.RemovedSessionCount,
                });
        });

        app.MapPost("/v1/runtimes/delete-failed", (string? environment, string? cluster, string? role, RegistryService registry) =>
        {
            var filter = new RuntimeListFilter(environment, cluster, role);
            var deleted = registry.DeleteFailedRuntimes(filter);
            return Results.Ok(new
            {
                deleted = true,
                filter,
                removedRuntimeCount = deleted.RemovedRuntimeCount,
                removedSessionCount = deleted.RemovedSessionCount,
                removedRuntimeNames = deleted.RemovedRuntimeNames,
            });
        });

        app.MapPost("/v1/runtimes/delete-unobserved", (string? environment, string? cluster, string? role, RegistryService registry) =>
        {
            var filter = new RuntimeListFilter(environment, cluster, role);
            var deleted = registry.DeleteUnobservedRuntimes(filter);
            return Results.Ok(new
            {
                deleted = true,
                filter,
                removedRuntimeCount = deleted.RemovedRuntimeCount,
                removedSessionCount = deleted.RemovedSessionCount,
                removedRuntimeNames = deleted.RemovedRuntimeNames,
            });
        });

        app.MapPost("/v1/runtimes/delete-slice", (string? environment, string? cluster, string? role, RegistryService registry) =>
        {
            var filter = new RuntimeListFilter(environment, cluster, role);
            var deleted = registry.DeleteRuntimes(filter);
            return Results.Ok(new
            {
                deleted = true,
                filter,
                removedRuntimeCount = deleted.RemovedRuntimeCount,
                removedSessionCount = deleted.RemovedSessionCount,
                removedRuntimeNames = deleted.RemovedRuntimeNames,
            });
        });
    }
}
