using System.Net;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Mvc;

namespace Leserpent;

public partial class Program
{
    private static void MapRuntimeEndpoints(WebApplication app)
    {
        app.MapGet("/v1/runtimes", async Task<IResult> ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry, ICompatibilityBridge compatibilityBridge, CancellationToken cancellationToken) =>
        {
            var response = new RuntimeCollectionResponse(
                new RuntimeListFilter(environmentTag, cluster, role),
                registry.ListRuntimes(new RuntimeListFilter(environmentTag, cluster, role)));
            try
            {
                await compatibilityBridge.ValidateRuntimeListAsync(response, cancellationToken);
                return Results.Ok(response);
            }
            catch (CompatibilityBridgeException ex)
            {
                return CompatibilityBridgeFailure(ex);
            }
        });

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
                    cancellationToken,
                    registry.GetRuntimeControlAccess(id)?.AdminToken);
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
                var capabilityDiscovery = await discovery.DiscoverAsync(request.Endpoint, request.CapabilityEndpoint, cancellationToken, request.PairingToken);
                var statusDiscovery = await discovery.DiscoverStatusAsync(request.Endpoint, request.StatusEndpoint, cancellationToken, request.PairingToken);
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

        app.MapPost("/v1/runtimes/{id}/deployments", async Task<IResult> (
            string id,
            RuntimeDeploymentRequest request,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            CancellationToken cancellationToken) =>
        {
            var validationError = ValidateRuntimeDeployment(request);
            if (validationError is not null)
            {
                return Results.BadRequest(new ApiErrorResponse("invalid_runtime_deployment", validationError, RuntimeId: id, RequestId: request.RequestId));
            }

            var runtime = registry.GetRuntime(id);
            var access = registry.GetRuntimeControlAccess(id);
            if (runtime is null || access is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }
            if (string.IsNullOrWhiteSpace(access.AdminToken))
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_not_authenticated",
                    "register the runtime with its gewyvern admin token before direct deployment",
                    RuntimeId: id));
            }
            var deploymentCapability = runtime.Capabilities.FirstOrDefault(capability =>
                string.Equals(capability.Key, "control.authenticated_deployment", StringComparison.OrdinalIgnoreCase));
            if (!string.Equals(deploymentCapability?.Support, "fully_supported", StringComparison.OrdinalIgnoreCase))
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_deployment_not_supported",
                    "refresh capabilities against a gewyvern runtime that advertises authenticated deployment control",
                    RuntimeId: id));
            }

            try
            {
                var deployed = await discovery.DeployAsync(access, request, cancellationToken);
                if (!deployed.Replayed || registry.GetOrchestraRunByRequestId(id, request.RequestId.Trim()) is null)
                {
                    registry.RecordOrchestraRun(
                        id,
                        "direct_deployment",
                        "ok",
                        new[]
                        {
                            new OrchestraExecutionStepResult(
                                "authenticated_deploy",
                                "ok",
                                $"runtime accepted deployment {deployed.DeploymentId} for {deployed.PipelineKind}"),
                        },
                        request.RequestedBy.Trim(),
                        "authenticated direct deployment",
                        requestId: request.RequestId.Trim());
                }
                registry.RecordRecoveryActivity(
                    id,
                    "direct_deployment",
                    "ok",
                    $"deployment {deployed.DeploymentId} accepted by authenticated runtime");
                return Results.Accepted($"/v1/orchestra/runtimes/{id}/runs", deployed);
            }
            catch (OrchestraPersistenceException ex)
            {
                return Results.Json(
                    new ApiErrorResponse("orchestra_persistence_unavailable", ex.Message, RuntimeId: id, RequestId: request.RequestId),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            catch (HttpRequestException ex)
            {
                if (ex.StatusCode == HttpStatusCode.Conflict)
                {
                    return Results.Conflict(new ApiErrorResponse(
                        "runtime_deployment_request_conflict",
                        "the runtime has already used this requestId for a different deployment",
                        RuntimeId: id,
                        RequestId: request.RequestId));
                }
                return Results.Json(
                    new ApiErrorResponse("runtime_deployment_rejected", ex.Message, RuntimeId: id, RequestId: request.RequestId),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status502BadGateway);
            }
            catch (Exception ex) when (ex is not OperationCanceledException)
            {
                return Results.BadRequest(new ApiErrorResponse("runtime_deployment_failed", ex.Message, RuntimeId: id, RequestId: request.RequestId));
            }
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
                await discovery.DiscoverAsync(runtime.Endpoint, null, cancellationToken, registry.GetRuntimeControlAccess(id)?.AdminToken));
            return refreshed is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/refresh-status", async (string id, RegistryService registry, CapabilityDiscoveryService discovery, ICompatibilityBridge compatibilityBridge, CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            var statusDiscovery = await discovery.DiscoverStatusAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                registry.GetRuntimeControlAccess(id)?.AdminToken);
            try
            {
                await compatibilityBridge.ValidateStatusRefreshAsync(
                    new RuntimeStatusRefreshResponse(
                        runtime.RuntimeId,
                        runtime.Name,
                        runtime.Endpoint,
                        statusDiscovery.Status),
                    cancellationToken);
            }
            catch (CompatibilityBridgeException ex)
            {
                return CompatibilityBridgeFailure(ex);
            }
            var refreshed = registry.RefreshRuntimeStatus(id, statusDiscovery);
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

    private static string? ValidateRuntimeDeployment(RuntimeDeploymentRequest request)
    {
        if (!request.Confirmed)
        {
            return "confirmed=true is required for remote deployment";
        }
        if (string.IsNullOrWhiteSpace(request.PipelineKind) || request.PipelineKind.Trim().Length > 128)
        {
            return "pipelineKind is required and must not exceed 128 characters";
        }
        if (string.IsNullOrWhiteSpace(request.RequestedBy) || request.RequestedBy.Trim().Length > 128)
        {
            return "requestedBy is required and must not exceed 128 characters";
        }
        if (string.IsNullOrWhiteSpace(request.RequestId)
            || request.RequestId.Trim().Length > 128
            || request.RequestId.Trim().Any(character => !char.IsAsciiLetterOrDigit(character) && character is not '-' and not '_' and not '.'))
        {
            return "requestId must contain 1-128 ASCII letters, digits, dots, dashes, or underscores";
        }
        if (request.Target?.Trim().Length > 256)
        {
            return "target must not exceed 256 characters";
        }
        return null;
    }

    private static IResult CompatibilityBridgeFailure(CompatibilityBridgeException error) =>
        Results.Json(
            new ApiErrorResponse("compatibility_bridge_failed", error.Message),
            LeserpentJsonContext.Default.ApiErrorResponse,
            statusCode: StatusCodes.Status502BadGateway);
}
