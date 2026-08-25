using System.Net;
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

        app.MapGet("/v1/runtimes/cleanup-plan", async Task<IResult> (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RuntimeCleanupProjectionService cleanupReads,
            CancellationToken cancellationToken) =>
        {
            var filter = new RuntimeListFilter(environmentTag, cluster, role);
            try
            {
                return Results.Ok(await cleanupReads.ReadAsync(filter, cancellationToken));
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
        });

        app.MapPost("/v1/runtimes/registration-plan", async Task<IResult> (
            RuntimeRegistrationPlanRequest request,
            RuntimeRegistrationPlanProjectionService registrationPlans,
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
            if (validation is not null)
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "invalid_runtime_registration_plan",
                    validation));
            }
            try
            {
                return Results.Ok(await registrationPlans.BuildAsync(
                    request,
                    cancellationToken));
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
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

        app.MapGet("/v1/runtimes/{id}/sidecar", async Task<IResult> (
            string id,
            RuntimeReadProjectionService runtimeReads,
            CancellationToken cancellationToken) =>
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
                : Results.Ok(new RuntimeSidecarRefreshResponse(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    runtime.SidecarEndpoint,
                    runtime.HasSidecarAdminToken,
                    runtime.SidecarStatus));
        });

        app.MapGet("/v1/runtimes/{id}/protocol-reading", async Task<IResult> (string id, RuntimeCommandExecutionContextService commandContexts, CapabilityDiscoveryService discovery, CancellationToken cancellationToken) =>
        {
            RuntimeCommandExecutionContext? context;
            try
            {
                context = await commandContexts.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
            if (context is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            try
            {
                var runtime = context.Runtime;
                var reading = await discovery.DiscoverProtocolReadingAsync(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    cancellationToken,
                    context.ControlAccess.AdminToken);
                return reading is null
                    ? Results.NotFound(new ApiErrorResponse("protocol_reading_unavailable", RuntimeId: id))
                    : Results.Ok(reading);
            }
            catch (Exception ex)
            {
                return Results.BadRequest(new ApiErrorResponse("protocol_reading_unavailable", ex.Message, RuntimeId: id));
            }
        });

        app.MapPost("/v1/runtimes/register", async Task<IResult> (
            RuntimeRegistrationRequest request,
            RuntimeRegistrationExecutionService registrations,
            CancellationToken cancellationToken) =>
        {
            try
            {
                return Results.Ok(await registrations.ExecuteAsync(
                    request,
                    cancellationToken));
            }
            catch (RuntimeRegistrationExecutionException ex)
            {
                return RuntimeRegistrationExecutionFailure(ex);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex);
            }
        });

        app.MapPost("/v1/runtimes/{id}/deployments", async Task<IResult> (
            string id,
            RuntimeDeploymentRequest request,
            RegistryService registry,
            RuntimeCommandExecutionContextService commandContexts,
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
            RuntimeCommandExecutionContext? context;
            try
            {
                context = await commandContexts.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
            if (context is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }
            var runtime = context.Runtime;
            var access = context.ControlAccess;
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
                    ? await deploymentAuthority.DeployAsync(
                        access,
                        context.AuthorityRevision
                            ?? throw new DaemonDeploymentException(
                                "daemon_deployment_revision_missing",
                                "daemon-authoritative deployment requires a runtime revision"),
                        normalizedRequest,
                        cancellationToken)
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
            RuntimeCommandExecutionContextService commandContexts,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            RuntimeCommandExecutionContext? context;
            try
            {
                context = await commandContexts.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
            if (context is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

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
                return RuntimeRegistrationAuthorityFailure(ex, id);
            }
            runtime = commit.Context.Runtime;
            capabilityDiscovery = commit.CapabilityDiscovery!;
            var refreshed = registry.RefreshRuntimeCapabilities(id, capabilityDiscovery);
            return refreshed is null
                ? Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id))
                : Results.Ok(new RuntimeCapabilityRefreshResponse(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    refreshed.Capabilities,
                    refreshed.CapabilitySource,
                    refreshed.CapabilityFetchedAt,
                    refreshed.CapabilityFetchError));
        });

        app.MapPost("/v1/runtimes/{id}/recovery", (
            string id,
            RuntimeRecoveryCommandRequest request,
            RegistryService registry,
            CapabilityDiscoveryService discovery,
            RuntimeCommandExecutionContextService commandContexts,
            ICompatibilityBridge compatibilityBridge,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeRecoveryAsync(
                id,
                request,
                commandContexts,
                registry,
                discovery,
                compatibilityBridge,
                registrationAuthority,
                cancellationToken));

        app.MapPost("/v1/runtimes/{id}/refresh-status", async (
            string id,
            RegistryService registry,
            RuntimeCommandExecutionContextService commandContexts,
            CapabilityDiscoveryService discovery,
            ICompatibilityBridge compatibilityBridge,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            RuntimeCommandExecutionContext? context;
            try
            {
                context = await commandContexts.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
            if (context is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            var runtime = context.Runtime;
            var statusDiscovery = await discovery.DiscoverStatusAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                context.ControlAccess.AdminToken);
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
                return RuntimeRegistrationAuthorityFailure(ex, id);
            }
            runtime = commit.Context.Runtime;
            statusDiscovery = commit.StatusDiscovery!;
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
                : Results.Ok(new RuntimeStatusRefreshResponse(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    refreshed.Status));
        });

        app.MapPost("/v1/runtimes/{id}/refresh-sidecar", async (
            string id,
            RegistryService registry,
            RuntimeCommandExecutionContextService commandContexts,
            CapabilityDiscoveryService discovery,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
        {
            RuntimeCommandExecutionContext? context;
            try
            {
                context = await commandContexts.InspectAsync(id, cancellationToken);
            }
            catch (DaemonRuntimeProjectionException ex)
            {
                return RuntimeProjectionFailure(ex, id);
            }
            if (context is null)
            {
                return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: id));
            }

            var runtime = context.Runtime;
            var sidecarAccess = context.SidecarAccess;
            if (sidecarAccess is null)
            {
                return Results.BadRequest(new ApiErrorResponse("runtime_has_no_sidecar_endpoint", RuntimeId: id));
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
                return RuntimeRegistrationAuthorityFailure(ex, id);
            }
            runtime = commit.Context.Runtime;
            sidecarDiscovery = commit.SidecarDiscovery!;
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
                : Results.Ok(new RuntimeSidecarRefreshResponse(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    runtime.SidecarEndpoint,
                    runtime.HasSidecarAdminToken,
                    refreshed.SidecarStatus));
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
            RuntimeCleanupProjectionService cleanupReads,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeCleanupAsync(
                RuntimeCleanupPolicy.FailedKind,
                new RuntimeListFilter(environmentTag, cluster, role),
                request,
                registry,
                cleanupReads,
                registrationAuthority,
                cancellationToken));

        app.MapPost("/v1/runtimes/delete-unobserved", (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RuntimeCleanupRequest request,
            RegistryService registry,
            RuntimeCleanupProjectionService cleanupReads,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeCleanupAsync(
                RuntimeCleanupPolicy.UnobservedKind,
                new RuntimeListFilter(environmentTag, cluster, role),
                request,
                registry,
                cleanupReads,
                registrationAuthority,
                cancellationToken));

        app.MapPost("/v1/runtimes/delete-slice", (
            [FromQuery(Name = "environment")] string? environmentTag,
            string? cluster,
            string? role,
            RuntimeCleanupRequest request,
            RegistryService registry,
            RuntimeCleanupProjectionService cleanupReads,
            IRuntimeRegistrationAuthority registrationAuthority,
            CancellationToken cancellationToken) =>
            ExecuteRuntimeCleanupAsync(
                RuntimeCleanupPolicy.SliceKind,
                new RuntimeListFilter(environmentTag, cluster, role),
                request,
                registry,
                cleanupReads,
                registrationAuthority,
                cancellationToken));
    }

    internal static async Task<IResult> ExecuteRuntimeCleanupAsync(
        string kind,
        RuntimeListFilter filter,
        RuntimeCleanupRequest request,
        RegistryService registry,
        RuntimeCleanupProjectionService cleanupReads,
        IRuntimeRegistrationAuthority registrationAuthority,
        CancellationToken cancellationToken)
    {
        try
        {
            var selection = await cleanupReads.SelectAsync(
                kind,
                filter,
                request,
                cancellationToken);
            if (selection.RuntimeIds.Count == 0)
            {
                return Results.Ok(new RuntimeBulkDeleteResponse(
                    true,
                    filter,
                    0,
                    0,
                    Array.Empty<string>()));
            }
            using var reservation = registry.ReserveRuntimeDeletion(
                selection.RuntimeIds,
                requireAllTargets: true,
                expectedSessionIds: selection.SessionIds);
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
        catch (DaemonRuntimeProjectionException ex)
        {
            return RuntimeProjectionFailure(ex);
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
        RuntimeCommandExecutionContextService commandContexts,
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

        RuntimeCommandExecutionContext? context;
        try
        {
            context = await commandContexts.InspectAsync(runtimeId, cancellationToken);
        }
        catch (DaemonRuntimeProjectionException ex)
        {
            return RuntimeProjectionFailure(ex, runtimeId);
        }
        if (context is null)
        {
            return Results.NotFound(new ApiErrorResponse("runtime_not_found", RuntimeId: runtimeId));
        }
        var runtime = context.Runtime;
        var controlAccess = context.ControlAccess;
        var sidecarAccess = context.SidecarAccess;
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
                controlAccess.AdminToken);
        }
        if (kind is "all" or "status")
        {
            statusDiscovery = await discovery.DiscoverStatusAsync(
                runtime.Endpoint,
                null,
                cancellationToken,
                controlAccess.AdminToken);
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
            return RuntimeRegistrationAuthorityFailure(ex, runtimeId);
        }
        runtime = commit.Context.Runtime;
        capabilityDiscovery = commit.CapabilityDiscovery;
        statusDiscovery = commit.StatusDiscovery;
        sidecarDiscovery = commit.SidecarDiscovery;

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

    private static IResult RuntimeRegistrationExecutionFailure(
        RuntimeRegistrationExecutionException error) =>
        error.Kind switch
        {
            RuntimeRegistrationExecutionFailureKind.InvalidRequest =>
                Results.BadRequest(new ApiErrorResponse(
                    error.Code,
                    error.Message,
                    RuntimeId: error.RuntimeId)),
            RuntimeRegistrationExecutionFailureKind.Conflict =>
                Results.Conflict(new ApiErrorResponse(
                    error.Code,
                    error.Message,
                    RuntimeId: error.RuntimeId)),
            RuntimeRegistrationExecutionFailureKind.NotFound =>
                Results.NotFound(new ApiErrorResponse(
                    error.Code,
                    error.Message,
                    RuntimeId: error.RuntimeId)),
            _ => Results.Json(
                new ApiErrorResponse(
                    error.Code,
                    error.Message,
                    RuntimeId: error.RuntimeId),
                LeserpentJsonContext.Default.ApiErrorResponse,
                statusCode: StatusCodes.Status502BadGateway),
        };

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
            LeserpentJsonContext.Default.ApiErrorResponse,
            statusCode: StatusCodes.Status502BadGateway);
}
