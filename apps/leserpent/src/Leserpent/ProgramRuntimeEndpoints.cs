using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Mvc;

namespace Leserpent;

public partial class Program
{
    private static void MapRuntimeEndpoints(WebApplication app)
    {
        app.MapGet("/v1/runtimes", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
            Results.Ok(new RuntimeCollectionResponse(
                new RuntimeListFilter(environmentTag, cluster, role),
                registry.ListRuntimes(new RuntimeListFilter(environmentTag, cluster, role)))));

        app.MapGet("/v1/runtimes/{id}", (string id, RegistryService registry) =>
        {
            var runtime = registry.GetRuntime(id);
            return runtime is null ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id)) : Results.Ok(runtime);
        });

        app.MapGet("/v1/runtimes/{id}/attention", (string id, RegistryService registry) =>
        {
            var attention = registry.GetRuntimeAttention(id);
            return attention is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(attention);
        });

        app.MapGet("/v1/runtimes/{id}/status", (string id, RegistryService registry) =>
        {
            var runtime = registry.GetRuntime(id);
            return runtime is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(new RuntimeStatusRefreshResponse(runtime.RuntimeId, runtime.Name, runtime.Endpoint, runtime.Status));
        });

        app.MapGet("/v1/runtimes/{id}/sidecar", (string id, RegistryService registry) =>
        {
            var runtime = registry.GetRuntime(id);
            return runtime is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
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
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            try
            {
                var reading = await discovery.DiscoverProtocolReadingAsync(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    cancellationToken);
                return reading is null
                    ? Results.NotFound(new ApiErrorResponse("protocol_reading_unavailable", RuntimeId: id))
                    : Results.Ok(reading);
            }
            catch (Exception ex)
            {
                return Results.BadRequest(new ApiErrorResponse("protocol_reading_unavailable", ex.Message, RuntimeId: id));
            }
        });

        app.MapPost("/v1/runtimes/register", async (RuntimeRegistrationRequest request, RegistryService registry, CapabilityDiscoveryService discovery, ControlPlaneSecurityPolicy security, CancellationToken cancellationToken) =>
        {
            if (string.IsNullOrWhiteSpace(request.Name) || string.IsNullOrWhiteSpace(request.Endpoint))
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "invalid_runtime_registration",
                    "name and endpoint are required"));
            }

            var registrationValidation = await security.ValidateRegistrationAsync(request, cancellationToken);
            if (registrationValidation is not null)
            {
                return Results.BadRequest(new ApiErrorResponse("invalid_runtime_registration", registrationValidation));
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
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            var refreshed = registry.RefreshRuntimeCapabilities(
                id,
                await discovery.DiscoverAsync(runtime.Endpoint, null, cancellationToken));
            return refreshed is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/refresh-status", async (string id, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
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
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/refresh-sidecar", async (string id, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            var sidecarAccess = registry.GetRuntimeSidecarAccess(id);
            if (sidecarAccess is null)
            {
                return Results.BadRequest(new ApiErrorResponse("runtime_has_no_sidecar_endpoint", RuntimeId: id));
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
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/delete", (string id, RegistryService registry) =>
        {
            (RuntimeSummary? RemovedRuntime, int RemovedSessionCount) deleted;
            try
            {
                deleted = registry.DeleteRuntime(id);
            }
            catch (OrchestraRuntimeBusyException ex)
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_delete_orchestra_active",
                    RuntimeId: id,
                    ActiveRuns: ex.ActiveRuns));
            }
            catch (OrchestraPersistenceException ex)
            {
                return Results.Json(
                    new ApiErrorResponse("runtime_delete_persistence_unavailable", ex.Message, RuntimeId: id),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            return deleted.RemovedRuntime is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(new RuntimeDeleteResponse(
                    true,
                    deleted.RemovedRuntime.RuntimeId,
                    deleted.RemovedRuntime.Name,
                    deleted.RemovedRuntime.Endpoint,
                    deleted.RemovedSessionCount));
        });

        app.MapPost("/v1/runtimes/delete-failed", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames) deleted;
            try
            {
                deleted = registry.DeleteFailedRuntimes(filter);
            }
            catch (OrchestraRuntimeBusyException ex)
            {
                return Results.Conflict(new ApiErrorResponse("runtime_delete_orchestra_active", ActiveRuns: ex.ActiveRuns));
            }
            catch (OrchestraPersistenceException ex)
            {
                return Results.Json(
                    new ApiErrorResponse("runtime_delete_persistence_unavailable", ex.Message),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            return Results.Ok(new RuntimeBulkDeleteResponse(
                true,
                filter,
                deleted.RemovedRuntimeCount,
                deleted.RemovedSessionCount,
                deleted.RemovedRuntimeNames));
        });

        app.MapPost("/v1/runtimes/delete-unobserved", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames) deleted;
            try
            {
                deleted = registry.DeleteUnobservedRuntimes(filter);
            }
            catch (OrchestraRuntimeBusyException ex)
            {
                return Results.Conflict(new ApiErrorResponse("runtime_delete_orchestra_active", ActiveRuns: ex.ActiveRuns));
            }
            catch (OrchestraPersistenceException ex)
            {
                return Results.Json(
                    new ApiErrorResponse("runtime_delete_persistence_unavailable", ex.Message),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            return Results.Ok(new RuntimeBulkDeleteResponse(
                true,
                filter,
                deleted.RemovedRuntimeCount,
                deleted.RemovedSessionCount,
                deleted.RemovedRuntimeNames));
        });

        app.MapPost("/v1/runtimes/delete-slice", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames) deleted;
            try
            {
                deleted = registry.DeleteRuntimes(filter);
            }
            catch (OrchestraRuntimeBusyException ex)
            {
                return Results.Conflict(new ApiErrorResponse("runtime_delete_orchestra_active", ActiveRuns: ex.ActiveRuns));
            }
            catch (OrchestraPersistenceException ex)
            {
                return Results.Json(
                    new ApiErrorResponse("runtime_delete_persistence_unavailable", ex.Message),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            return Results.Ok(new RuntimeBulkDeleteResponse(
                true,
                filter,
                deleted.RemovedRuntimeCount,
                deleted.RemovedSessionCount,
                deleted.RemovedRuntimeNames));
        });
    }
}
