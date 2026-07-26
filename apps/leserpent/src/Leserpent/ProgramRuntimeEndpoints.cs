using System.Net;
using System.Security.Cryptography;
using System.Text;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Mvc;

namespace Leserpent;

public partial class Program
{
    private static void MapRuntimeEndpoints(WebApplication app)
    {
        app.MapGet("/v1/runtimes", async Task<IResult> ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RuntimeReadProjectionService runtimeReads, ICompatibilityBridge compatibilityBridge, CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            IReadOnlyList<RuntimeSummary> runtimes;
            try
            {
                runtimes = await runtimeReads.ListAsync(filter, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
            var response = new RuntimeCollectionResponse(
                filter,
                runtimes);
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

        app.MapGet("/v1/runtimes/cleanup-plan", ([FromQuery(Name = "environment")] string? environmentTag, string? cluster, string? role, RegistryService registry) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            return Results.Ok(registry.GetRuntimeCleanupPlan(filter));
        });

        app.MapPost("/v1/runtimes/registration-plan", async Task<IResult> (
            RuntimeRegistrationPlanRequest request,
            RegistryService registry,
            ControlPlaneSecurityPolicy security,
            CancellationToken cancellationToken) =>
        {
            if (string.IsNullOrWhiteSpace(request.Name) || string.IsNullOrWhiteSpace(request.Endpoint))
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "invalid_runtime_registration_plan",
                    "name and endpoint are required"));
            }
            var validation = await security.ValidateRegistrationPlanAsync(request, cancellationToken);
            return validation is null
                ? Results.Ok(registry.GetRuntimeRegistrationPlan(request))
                : Results.BadRequest(new ApiErrorResponse("invalid_runtime_registration_plan", validation));
        });

        app.MapGet("/v1/runtimes/{id}", async Task<IResult> (string id, RuntimeReadProjectionService runtimeReads, CancellationToken cancellationToken) =>
        {
            RuntimeSummary? runtime;
            try
            {
                runtime = await runtimeReads.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
            return runtime is null ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id)) : Results.Ok(runtime);
        });

        app.MapGet("/v1/runtimes/{id}/attention", async Task<IResult> (string id, RuntimeReadProjectionService runtimeReads, RegistryService registry, CancellationToken cancellationToken) =>
        {
            RuntimeSummary? runtime;
            try
            {
                runtime = await runtimeReads.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
            var attention = runtime is null
                ? null
                : registry.GetRuntimeAttention(id, runtime);
            return attention is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(attention);
        });

        app.MapGet("/v1/runtimes/{id}/status", async Task<IResult> (string id, RuntimeReadProjectionService runtimeReads, CancellationToken cancellationToken) =>
        {
            RuntimeSummary? runtime;
            try
            {
                runtime = await runtimeReads.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
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

        app.MapGet("/v1/runtimes/{id}/protocol-reading", async Task<IResult> (string id, RuntimeReadProjectionService runtimeReads, RegistryService registry, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            RuntimeSummary? runtime;
            try
            {
                runtime = await runtimeReads.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
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

        app.MapPost("/v1/runtimes/register", async (RuntimeRegistrationRequest request, RegistryService registry, CapabilityDiscoveryService discovery, IRuntimeRegistrationAuthority registrationAuthority, ControlPlaneSecurityPolicy security, CancellationToken cancellationToken) =>
        {
            if (string.IsNullOrWhiteSpace(request.Name) || string.IsNullOrWhiteSpace(request.Endpoint))
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "invalid_runtime_registration",
                    "name and endpoint are required"));
            }

            request = request with
            {
                Name = request.Name.Trim(),
                Endpoint = request.Endpoint.Trim(),
            };

            var registrationValidation = await security.ValidateRegistrationAsync(request, cancellationToken);
            if (registrationValidation is not null)
            {
                return Results.BadRequest(new ApiErrorResponse("invalid_runtime_registration", registrationValidation));
            }

            var plan = registry.GetRuntimeRegistrationPlan(new RuntimeRegistrationPlanRequest(
                request.Name,
                request.Endpoint,
                request.SidecarEndpoint));
            if (!plan.Allowed)
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_registration_plan_changed",
                    "runtime endpoint is already registered to another runtime",
                    RuntimeId: plan.ExistingRuntimeId));
            }
            if (!string.IsNullOrWhiteSpace(request.RegistrationPlanToken)
                && !string.Equals(request.RegistrationPlanToken, plan.PlanToken, StringComparison.Ordinal))
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_registration_plan_changed",
                    "runtime registration plan changed; review the current target before retrying",
                    RuntimeId: plan.ExistingRuntimeId));
            }

            var shouldUseAuthority = registrationAuthority.Enabled;
            var runtimeId = shouldUseAuthority
                ? plan.ExistingRuntimeId ?? BuildRuntimeIdFromRegistration(request.Name, request.Endpoint)
                : null;

            try
            {
                if (request.FetchCapabilities)
                {
                    var capabilityDiscovery = await discovery.DiscoverAsync(request.Endpoint, request.CapabilityEndpoint, cancellationToken, request.PairingToken);
                    var statusDiscovery = await discovery.DiscoverStatusAsync(request.Endpoint, request.StatusEndpoint, cancellationToken, request.PairingToken);
                    var sidecarDiscovery = string.IsNullOrWhiteSpace(request.SidecarEndpoint)
                        ? null
                        : await discovery.DiscoverSidecarStatusAsync(request.SidecarEndpoint!, request.SidecarStatusEndpoint, request.SidecarAdminToken, cancellationToken);
                    if (runtimeId is not null)
                    {
                        _ = await registrationAuthority.RegisterAsync(
                            request,
                            runtimeId,
                            cancellationToken,
                            update: plan.Action == RuntimeRegistrationPolicy.UpdateAction,
                            capabilityDiscovery: capabilityDiscovery,
                            statusDiscovery: statusDiscovery,
                            sidecarDiscovery: sidecarDiscovery);
                    }
                    var registered = registry.RegisterRuntimeFromDiscovery(request, capabilityDiscovery, statusDiscovery, sidecarDiscovery, runtimeId);
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

                if (runtimeId is not null)
                {
                    _ = await registrationAuthority.RegisterAsync(
                        request,
                        runtimeId,
                        cancellationToken,
                        update: plan.Action == RuntimeRegistrationPolicy.UpdateAction);
                }

                var manualRegistered = registry.RegisterRuntime(request, runtimeId);
                registry.RecordRecoveryActivity(
                    manualRegistered.RuntimeId,
                    "register_runtime",
                    "ok",
                    "runtime registered with manual capability intake");
                return Results.Ok(manualRegistered);
            }
            catch (RuntimeRegistrationPlanException ex)
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_registration_plan_changed",
                    ex.Message,
                    RuntimeId: ex.Plan.ExistingRuntimeId));
            }
            catch (DaemonRuntimeRegistrationException ex)
            {
                return RuntimeRegistrationAuthorityFailure(ex, plan.ExistingRuntimeId);
            }
        });

        app.MapPost("/v1/runtimes/{id}/deployments", async Task<IResult> (
            string id,
            RuntimeDeploymentRequest request,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            ICompatibilityBridge compatibilityBridge,
            IDeploymentAuthority deploymentAuthority,
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
            RuntimeDeploymentRequest normalizedRequest;
            try
            {
                var normalized = await compatibilityBridge.NormalizeRuntimeDeploymentRequestAsync(
                    new RuntimeDeploymentCompatibilityEnvelope(runtime.RuntimeId, request),
                    cancellationToken);
                if (!string.Equals(normalized.RuntimeId, runtime.RuntimeId, StringComparison.Ordinal))
                {
                    throw new CompatibilityBridgeException(
                        "Rust compatibility bridge returned a mismatched runtime identity");
                }
                normalizedRequest = normalized.Request;
            }
            catch (CompatibilityBridgeException ex)
            {
                return CompatibilityBridgeFailure(ex);
            }

            try
            {
                var deployed = deploymentAuthority.Enabled
                    ? await deploymentAuthority.DeployAsync(access, normalizedRequest, cancellationToken)
                    : await discovery.DeployAsync(access, normalizedRequest, cancellationToken);
                if (!deployed.Replayed || registry.GetOrchestraRunByRequestId(id, normalizedRequest.RequestId) is null)
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
                        normalizedRequest.RequestedBy,
                        "authenticated direct deployment",
                        requestId: normalizedRequest.RequestId);
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
                    new ApiErrorResponse("orchestra_persistence_unavailable", ex.Message, RuntimeId: id, RequestId: normalizedRequest.RequestId),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            catch (DaemonDeploymentException ex)
            {
                if (string.Equals(ex.Code, "runtime_deployment_request_conflict", StringComparison.Ordinal))
                {
                    return Results.Conflict(new ApiErrorResponse(
                        ex.Code,
                        "the runtime has already used this requestId for a different deployment",
                        RuntimeId: id,
                        RequestId: normalizedRequest.RequestId));
                }
                return Results.Json(
                    new ApiErrorResponse("runtime_deployment_rejected", ex.Message, RuntimeId: id, RequestId: normalizedRequest.RequestId),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status502BadGateway);
            }
            catch (HttpRequestException ex)
            {
                if (ex.StatusCode == HttpStatusCode.Conflict)
                {
                    return Results.Conflict(new ApiErrorResponse(
                        "runtime_deployment_request_conflict",
                        "the runtime has already used this requestId for a different deployment",
                        RuntimeId: id,
                        RequestId: normalizedRequest.RequestId));
                }
                return Results.Json(
                    new ApiErrorResponse("runtime_deployment_rejected", ex.Message, RuntimeId: id, RequestId: normalizedRequest.RequestId),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status502BadGateway);
            }
            catch (Exception ex) when (ex is not OperationCanceledException)
            {
                return Results.BadRequest(new ApiErrorResponse("runtime_deployment_failed", ex.Message, RuntimeId: id, RequestId: normalizedRequest.RequestId));
            }
        });

        app.MapPost("/v1/runtimes/{id}/refresh-capabilities", async (
            string id,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            var runtime = registry.GetRuntime(id);
            if (runtime is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            var capabilityDiscovery = await discovery.DiscoverAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                registry.GetRuntimeControlAccess(id)?.AdminToken);
            try
            {
                await registrationAuthority.SubmitDiscoveryAsync(
                    id,
                    cancellationToken,
                    capabilityDiscovery: capabilityDiscovery);
            }
            catch (DaemonRuntimeRegistrationException ex)
            {
                return RuntimeRegistrationAuthorityFailure(ex, id);
            }
            var refreshed = registry.RefreshRuntimeCapabilities(id, capabilityDiscovery);
            return refreshed is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(refreshed);
        });

        app.MapPost("/v1/runtimes/{id}/recovery", (
            string id,
            RuntimeRecoveryCommandRequest request,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            RuntimeReadProjectionService runtimeReads,
            ICompatibilityBridge compatibilityBridge,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeRecoveryAsync(
                id,
                request,
                runtimeReads,
                registry,
                discovery,
                compatibilityBridge,
                registrationAuthority,
                cancellationToken));

        app.MapPost("/v1/runtimes/{id}/refresh-status", async (
            string id,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            ICompatibilityBridge compatibilityBridge,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
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
            try
            {
                await registrationAuthority.SubmitDiscoveryAsync(
                    id,
                    cancellationToken,
                    statusDiscovery: statusDiscovery);
            }
            catch (DaemonRuntimeRegistrationException ex)
            {
                return RuntimeRegistrationAuthorityFailure(ex, id);
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

        app.MapPost("/v1/runtimes/{id}/refresh-sidecar", async (
            string id,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
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

            var sidecarDiscovery = await discovery.DiscoverSidecarStatusAsync(
                sidecarAccess.SidecarEndpoint,
                null,
                sidecarAccess.SidecarAdminToken,
                cancellationToken);
            try
            {
                if (sidecarDiscovery.SidecarStatus is not null)
                {
                    await registrationAuthority.SubmitDiscoveryAsync(
                        id,
                        cancellationToken,
                        sidecarDiscovery: sidecarDiscovery);
                }
            }
            catch (DaemonRuntimeRegistrationException ex)
            {
                return RuntimeRegistrationAuthorityFailure(ex, id);
            }
            var refreshed = registry.RefreshRuntimeSidecar(id, sidecarDiscovery);
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

        app.MapPost("/v1/runtimes/{id}/delete", async (
            string id,
            RegistryService registry,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            RuntimeDeletionReservation reservation;
            try
            {
                reservation = registry.ReserveRuntimeDeletion(new[] { id });
            }
            catch (OrchestraRuntimeBusyException ex)
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_delete_orchestra_active",
                    RuntimeId: id,
                    ActiveRuns: ex.ActiveRuns));
            }
            catch (RuntimeDeletionInProgressException)
            {
                return Results.Conflict(new ApiErrorResponse(
                    "runtime_delete_in_progress",
                    RuntimeId: id));
            }
            catch (OrchestraPersistenceException ex)
            {
                return Results.Json(
                    new ApiErrorResponse(
                        "runtime_delete_persistence_unavailable",
                        ex.Message,
                        RuntimeId: id),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            using (reservation)
            {
                if (reservation.RuntimeIds.Count == 0)
                {
                    return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
                }
                (RuntimeSummary? RemovedRuntime, int RemovedSessionCount) deleted;
                try
                {
                    await RuntimeDeletionAuthorityWorkflow.ExecuteAsync(
                        registry,
                        reservation,
                        registrationAuthority,
                        cancellationToken);
                    deleted = registry.DeleteRuntime(id);
                    registry.CompleteRuntimeDeletion(reservation);
                }
                catch (DaemonRuntimeRegistrationException ex)
                {
                    return RuntimeUnregistrationAuthorityFailure(ex, id);
                }
                catch (RuntimeUnregistrationReplayAmbiguousException ex)
                {
                    return RuntimeUnregistrationReplayAmbiguous(
                        ex,
                        id);
                }
                catch (OrchestraPersistenceException ex)
                {
                    return Results.Json(
                        new ApiErrorResponse(
                            "runtime_delete_persistence_unavailable",
                            ex.Message,
                            RuntimeId: id),
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
            }
        });

        app.MapPost("/v1/runtimes/delete-failed", (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RuntimeCleanupRequest request,
            RegistryService registry,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeCleanupAsync(
                RuntimeCleanupPolicy.FailedKind,
                new RuntimeListFilter(environmentTag, cluster, role),
                request,
                registry,
                registrationAuthority,
                cancellationToken));

        app.MapPost("/v1/runtimes/delete-unobserved", (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RuntimeCleanupRequest request,
            RegistryService registry,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeCleanupAsync(
                RuntimeCleanupPolicy.UnobservedKind,
                new RuntimeListFilter(environmentTag, cluster, role),
                request,
                registry,
                registrationAuthority,
                cancellationToken));

        app.MapPost("/v1/runtimes/delete-slice", (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RuntimeCleanupRequest request,
            RegistryService registry,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeCleanupAsync(
                RuntimeCleanupPolicy.SliceKind,
                new RuntimeListFilter(environmentTag, cluster, role),
                request,
                registry,
                registrationAuthority,
                cancellationToken));
    }

    private static async Task<IResult> ExecuteRuntimeCleanupAsync(
        string kind,
        RuntimeListFilter filter,
        RuntimeCleanupRequest request,
        RegistryService registry,
        IRuntimeRegistrationAuthority registrationAuthority,
        CancellationToken cancellationToken)
    {
        try
        {
            var targetIds = registry.GetPlannedRuntimeCleanupTargetIds(kind, filter, request);
            using var reservation = registry.ReserveRuntimeDeletion(
                targetIds,
                requireAllTargets: true);
            await RuntimeDeletionAuthorityWorkflow.ExecuteAsync(
                registry,
                reservation,
                registrationAuthority,
                cancellationToken);
            var deleted = registry.DeleteRuntimesById(reservation.RuntimeIds);
            registry.CompleteRuntimeDeletion(reservation);
            return Results.Ok(new RuntimeBulkDeleteResponse(
                true,
                filter,
                deleted.RemovedRuntimeCount,
                deleted.RemovedSessionCount,
                deleted.RemovedRuntimeNames));
        }
        catch (RuntimeCleanupPlanMismatchException ex)
        {
            return Results.Conflict(new ApiErrorResponse("runtime_cleanup_plan_changed", ex.Message));
        }
        catch (RuntimeDeletionInProgressException ex)
        {
            return Results.Conflict(new ApiErrorResponse(
                "runtime_delete_in_progress",
                RuntimeId: ex.RuntimeIds.FirstOrDefault()));
        }
        catch (OrchestraRuntimeBusyException ex)
        {
            return Results.Conflict(new ApiErrorResponse(
                "runtime_delete_orchestra_active",
                ActiveRuns: ex.ActiveRuns));
        }
        catch (OrchestraPersistenceException ex)
        {
            return Results.Json(
                new ApiErrorResponse("runtime_delete_persistence_unavailable", ex.Message),
                LeserpentJsonContext.Default.ApiErrorResponse,
                statusCode: StatusCodes.Status503ServiceUnavailable);
        }
        catch (DaemonRuntimeRegistrationException ex)
        {
            return RuntimeUnregistrationAuthorityFailure(ex, null);
        }
        catch (RuntimeUnregistrationReplayAmbiguousException ex)
        {
            return RuntimeUnregistrationReplayAmbiguous(ex, null);
        }
    }

    private static async Task<IResult> ExecuteRuntimeRecoveryAsync(
        string runtimeId,
        RuntimeRecoveryCommandRequest request,
        RuntimeReadProjectionService runtimeReads,
        RegistryService registry,
        CapabilityDiscoveryService discovery,
        ICompatibilityBridge compatibilityBridge,
        IRuntimeRegistrationAuthority registrationAuthority,
        CancellationToken cancellationToken)
    {
        var kind = request.Kind?.Trim().ToLowerInvariant();
        if (kind is not ("all" or "status" or "capabilities" or "sidecar"))
        {
            return Results.BadRequest(new ApiErrorResponse(
                "invalid_runtime_recovery_kind",
                "kind must be all, status, capabilities, or sidecar",
                RuntimeId: runtimeId));
        }

        RuntimeSummary? runtime;
        try
        {
            runtime = await runtimeReads.InspectAsync(runtimeId, cancellationToken);
        }
        catch (DaemonRuntimeProjectionException ex)
        {
            return RuntimeProjectionFailure(ex, runtimeId);
        }
        if (runtime is null)
        {
            return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: runtimeId));
        }
        var controlAccess = registry.GetRuntimeControlAccess(runtimeId);
        var sidecarAccess = registry.GetRuntimeSidecarAccess(runtimeId);
        if (kind == "sidecar" && sidecarAccess is null)
        {
            return Results.BadRequest(new ApiErrorResponse("runtime_has_no_sidecar_endpoint", RuntimeId: runtimeId));
        }

        CapabilityDiscoveryResult? capabilityDiscovery = null;
        RuntimeStatusDiscoveryResult? statusDiscovery = null;
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null;
        if (kind is "all" or "capabilities")
        {
            capabilityDiscovery = await discovery.DiscoverAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                controlAccess?.AdminToken);
        }
        if (kind is "all" or "status")
        {
            statusDiscovery = await discovery.DiscoverStatusAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                controlAccess?.AdminToken);
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
        }
        if (sidecarAccess is not null && (kind is "all" or "sidecar"))
        {
            sidecarDiscovery = await discovery.DiscoverSidecarStatusAsync(
                sidecarAccess.SidecarEndpoint,
                null,
                sidecarAccess.SidecarAdminToken,
                cancellationToken);
        }

        try
        {
            await registrationAuthority.SubmitDiscoveryAsync(
                runtimeId,
                cancellationToken,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);
        }
        catch (DaemonRuntimeRegistrationException ex)
        {
            return RuntimeRegistrationAuthorityFailure(ex, runtimeId);
        }

        var steps = new List<RuntimeRecoveryStepResult>();
        if (capabilityDiscovery is not null)
        {
            var refreshed = registry.RefreshRuntimeCapabilities(runtimeId, capabilityDiscovery);
            if (refreshed is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: runtimeId));
            }
            steps.Add(BuildRecoveryStep(
                "capabilities",
                capabilityDiscovery.CapabilitySource,
                capabilityDiscovery.CapabilityFetchError,
                null,
                null));
        }
        if (statusDiscovery is not null)
        {
            var refreshed = registry.RefreshRuntimeStatus(runtimeId, statusDiscovery);
            if (refreshed is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: runtimeId));
            }
            steps.Add(BuildRecoveryStep(
                "status",
                refreshed.Status.StatusSource,
                refreshed.Status.StatusFetchError,
                null,
                null));
        }
        if (sidecarDiscovery is not null)
        {
            var refreshed = registry.RefreshRuntimeSidecar(runtimeId, sidecarDiscovery);
            if (refreshed is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: runtimeId));
            }
            steps.Add(BuildRecoveryStep(
                "sidecar",
                null,
                null,
                refreshed.SidecarStatus?.StatusSource,
                refreshed.SidecarStatus?.StatusFetchError));
        }

        var outcome = steps
            .Select(step => step.Outcome)
            .OrderByDescending(RecoveryOutcomePriority)
            .FirstOrDefault() ?? "ok";
        var summary = string.Join(" · ", steps.Select(step => $"{step.Kind}:{step.Outcome}"));
        registry.RecordRecoveryActivity(runtimeId, $"refresh_{kind}", outcome, summary);
        return Results.Ok(new RuntimeRecoveryCommandResponse(runtimeId, kind, outcome, steps));
    }

    private static RuntimeRecoveryStepResult BuildRecoveryStep(
        string kind,
        string? runtimeStatusSource,
        string? runtimeStatusError,
        string? sidecarStatusSource,
        string? sidecarStatusError) =>
        new(
            kind,
            DetermineRefreshOutcome(runtimeStatusSource, runtimeStatusError, sidecarStatusSource, sidecarStatusError),
            BuildRecoverySummary(runtimeStatusSource, runtimeStatusError, sidecarStatusSource, sidecarStatusError));

    private static int RecoveryOutcomePriority(string outcome) => outcome switch
    {
        "auth_failed" => 4,
        "network_failed" => 3,
        "incomplete_data" => 2,
        "degraded" => 1,
        _ => 0,
    };

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

    private static string BuildRuntimeIdFromRegistration(string name, string endpoint)
    {
        var normalizedName = name.Trim();
        var normalizedEndpoint = endpoint.Trim();
        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes($"{normalizedName}\u0000{normalizedEndpoint}"));
        return Convert.ToHexString(bytes).ToLowerInvariant()[..32];
    }

    private static IResult RuntimeRegistrationAuthorityFailure(
        DaemonRuntimeRegistrationException error,
        string? existingRuntimeId)
    {
        return error.Code switch
        {
            "runtime_already_exists" or "idempotency_conflict" or "revision_conflict" => Results.Conflict(
                new ApiErrorResponse(
                    "runtime_registration_conflict",
                    error.Message,
                    RuntimeId: existingRuntimeId)),
            "runtime_not_found" => Results.NotFound(new ApiErrorResponse(
                "runtime_not_found",
                error.Message,
                RuntimeId: existingRuntimeId)),
            _ => Results.Json(
                new ApiErrorResponse("runtime_registration_rejected", error.Message),
                LeserpentJsonContext.Default.ApiErrorResponse,
                statusCode: StatusCodes.Status502BadGateway),
        };
    }

    private static IResult RuntimeUnregistrationAuthorityFailure(
        DaemonRuntimeRegistrationException error,
        string? runtimeId) =>
        error.Code switch
        {
            "idempotency_conflict" or "revision_conflict" => Results.Conflict(
                new ApiErrorResponse(
                    "runtime_delete_conflict",
                    error.Message,
                    RuntimeId: runtimeId)),
            "runtime_not_found" => Results.NotFound(new ApiErrorResponse(
                "runtime_not_found",
                error.Message,
                RuntimeId: runtimeId)),
            _ => Results.Json(
                new ApiErrorResponse(
                    "runtime_delete_rejected",
                    error.Message,
                    RuntimeId: runtimeId),
                LeserpentJsonContext.Default.ApiErrorResponse,
                statusCode: StatusCodes.Status502BadGateway),
        };

    private static IResult RuntimeUnregistrationReplayAmbiguous(
        RuntimeUnregistrationReplayAmbiguousException error,
        string? runtimeId) =>
        Results.Conflict(new ApiErrorResponse(
            "runtime_delete_replay_ambiguous",
            error.Message,
            RuntimeId: runtimeId));

    private static IResult RuntimeProjectionFailure(
        DaemonRuntimeProjectionException exception,
        string? runtimeId = null) =>
        Results.Json(
            new ApiErrorResponse(exception.Code, exception.Message, RuntimeId: runtimeId),
            statusCode: StatusCodes.Status502BadGateway);
}
