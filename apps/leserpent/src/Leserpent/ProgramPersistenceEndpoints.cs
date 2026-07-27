using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Mvc;

namespace Leserpent;

public partial class Program
{
    internal static void MapPersistenceEndpoints(WebApplication app)
    {
        app.MapPost("/v1/persistence/save", (RegistryService registry) =>
            Results.Ok(new PersistenceSaveResponse(true, registry.SaveNow())));

        app.MapGet("/v1/persistence/export", (RegistryService registry) =>
        {
            var state = registry.ExportState();
            return ResultExtensions.FileDownloadJson(
                state,
                $"leserpent-control-plane-state-{state.SavedAt:yyyyMMddHHmmss}.json");
        });

        app.MapGet(
            "/v1/persistence/runtime-deletions",
            (RegistryService registry) =>
                Results.Json(
                    registry.ListPendingRuntimeDeletions().ToArray(),
                    LeserpentJsonContext.Default
                        .PersistedRuntimeDeletionIntentArray));

        app.MapGet(
            "/v1/persistence/runtime-deletion-retry-audit",
            (RegistryService registry) =>
                Results.Json(
                    registry.ListRuntimeDeletionRetryAudit().ToArray(),
                    LeserpentJsonContext.Default
                        .PersistedRuntimeDeletionRetryAuditArray));

        app.MapGet(
            "/v1/persistence/runtime-deletion-reconciliation-audit",
            (RegistryService registry) =>
                Results.Json(
                    registry
                        .ListRuntimeDeletionReconciliationAudit()
                        .ToArray(),
                    LeserpentJsonContext.Default
                        .PersistedRuntimeDeletionReconciliationAuditArray));

        app.MapGet(
            "/v1/persistence/orchestra-cleanup-replay-status",
            (RegistryService registry) =>
            {
                var status = registry
                    .GetOrchestraDeleteReplayCheckpointStatus();
                return status is null
                    ? Results.NotFound()
                    : Results.Json(
                        status,
                        LeserpentJsonContext.Default
                            .OrchestraDeleteReplayCheckpointStatus);
            });

        app.MapGet(
            "/v1/persistence/orchestra-cleanup-worker-health",
            (
                [FromServices]
                OrchestraDeleteCheckpointWorkerHealth health) =>
                Results.Json(
                    health.Snapshot(),
                    LeserpentJsonContext.Default
                        .OrchestraDeleteCheckpointWorkerHealthSnapshot));

        app.MapGet(
            "/v1/persistence/control-writer-health",
            ([FromServices] ControlPlaneWriterFence writer) =>
                Results.Json(
                    writer.Snapshot(),
                    LeserpentJsonContext.Default
                        .ControlPlaneWriterHealthSnapshot));

        app.MapPost(
            "/v1/persistence/orchestra-cleanup-replay-status/acknowledge",
            (
                OrchestraDeleteCheckpointAlertAcknowledgeRequest request,
                RegistryService registry) =>
            {
                try
                {
                    return Results.Json(
                        registry
                            .AcknowledgeOrchestraDeleteCheckpointAlert(
                                request),
                        LeserpentJsonContext.Default
                            .OrchestraDeleteCheckpointAlertAcknowledgeResponse);
                }
                catch (OrchestraDeleteCheckpointAlertException ex)
                    when (string.Equals(
                        ex.Code,
                        "invalid_orchestra_checkpoint_alert_acknowledgement",
                        StringComparison.Ordinal))
                {
                    return Results.BadRequest(
                        new ApiErrorResponse(ex.Code, ex.Message));
                }
                catch (OrchestraDeleteCheckpointAlertException ex)
                    when (string.Equals(
                        ex.Code,
                        "orchestra_checkpoint_horizon_unavailable",
                        StringComparison.Ordinal))
                {
                    return Results.Json(
                        new ApiErrorResponse(ex.Code, ex.Message),
                        LeserpentJsonContext.Default.ApiErrorResponse,
                        statusCode:
                            StatusCodes.Status503ServiceUnavailable);
                }
                catch (OrchestraDeleteCheckpointAlertException ex)
                {
                    return Results.Conflict(
                        new ApiErrorResponse(ex.Code, ex.Message));
                }
                catch (ControlPlaneStatePersistenceException ex)
                {
                    return Results.Json(
                        new ApiErrorResponse(
                            "orchestra_checkpoint_alert_persistence_unavailable",
                            ex.Message),
                        LeserpentJsonContext.Default.ApiErrorResponse,
                        statusCode:
                            StatusCodes.Status503ServiceUnavailable);
                }
            });

        app.MapGet(
            "/v1/persistence/runtime-deletions/{intentId}/reconciliation-plan",
            async (
                string intentId,
                RegistryService registry,
                [FromServices]
                IDaemonRuntimeProjectionReader daemon,
                CancellationToken cancellationToken) =>
            {
                try
                {
                    if (!daemon.Enabled)
                    {
                        return RuntimeDeletionReconciliationUnavailable(
                            "authoritative daemon runtime projection is disabled");
                    }

                    var intent = registry
                        .GetRuntimeDeletionReconciliationIntent(intentId);
                    var snapshot = await daemon.SnapshotAsync(
                        cancellationToken);
                    if (snapshot.Revision == 0)
                    {
                        throw new RuntimeDeletionReconciliationException(
                            "runtime_deletion_reconciliation_daemon_revision_invalid",
                            "daemon runtime projection revision is not valid for reconciliation");
                    }
                    var targetIds = intent.RuntimeIds.ToHashSet(
                        StringComparer.OrdinalIgnoreCase);
                    var reappeared = snapshot.Runtimes
                        .Select(static runtime => runtime.RuntimeId)
                        .Where(targetIds.Contains)
                        .Distinct(StringComparer.OrdinalIgnoreCase)
                        .OrderBy(
                            static runtimeId => runtimeId,
                            StringComparer.OrdinalIgnoreCase)
                        .ToArray();
                    return Results.Json(
                        new RuntimeDeletionReconciliationPlan(
                            intent.IntentId,
                            intent.Revision,
                            snapshot.Revision,
                            intent.RuntimeIds.ToArray(),
                            reappeared,
                            reappeared.Length == 0),
                        LeserpentJsonContext.Default
                            .RuntimeDeletionReconciliationPlan);
                }
                catch (RuntimeDeletionReconciliationException ex)
                {
                    return RuntimeDeletionReconciliationFailure(ex);
                }
                catch (DaemonRuntimeProjectionException ex)
                {
                    return RuntimeDeletionReconciliationDaemonFailure(ex);
                }
            });

        app.MapPost(
            "/v1/persistence/runtime-deletions/{intentId}/reconcile",
            async (
                string intentId,
                RuntimeDeletionReconcileRequest request,
                RegistryService registry,
                [FromServices]
                IDaemonRuntimeProjectionReader daemon,
                CancellationToken cancellationToken) =>
            {
                try
                {
                    var start =
                        registry.BeginRuntimeDeletionReconciliation(
                            intentId,
                            request);
                    if (start.Replay is not null)
                    {
                        return Results.Json(
                            start.Replay,
                            LeserpentJsonContext.Default
                                .RuntimeDeletionReconcileResponse);
                    }

                    using var reservation = start.Reservation!;
                    if (!daemon.Enabled)
                    {
                        return RuntimeDeletionReconciliationUnavailable(
                            "authoritative daemon runtime projection is disabled");
                    }

                    var snapshot = await daemon.SnapshotAsync(
                        cancellationToken);
                    var response =
                        registry.CompleteRuntimeDeletionReconciliation(
                            reservation,
                            request,
                            snapshot);
                    return Results.Json(
                        response,
                        LeserpentJsonContext.Default
                            .RuntimeDeletionReconcileResponse);
                }
                catch (RuntimeDeletionReconciliationException ex)
                {
                    return RuntimeDeletionReconciliationFailure(ex);
                }
                catch (DaemonRuntimeProjectionException ex)
                {
                    return RuntimeDeletionReconciliationDaemonFailure(ex);
                }
                catch (OrchestraRuntimeBusyException ex)
                {
                    return Results.Conflict(new ApiErrorResponse(
                        "runtime_deletion_reconciliation_runtime_busy",
                        ex.Message));
                }
                catch (OrchestraPersistenceException ex)
                {
                    return Results.Json(
                        new ApiErrorResponse(
                            "runtime_deletion_reconciliation_persistence_unavailable",
                            ex.Message),
                        LeserpentJsonContext.Default.ApiErrorResponse,
                        statusCode:
                            StatusCodes.Status503ServiceUnavailable);
                }
            });

        app.MapPost(
            "/v1/persistence/runtime-deletions/{intentId}/retry-now",
            (
                string intentId,
                RuntimeDeletionRetryNowRequest request,
                RegistryService registry,
                RuntimeDeletionRecoverySignal recoverySignal) =>
            {
                try
                {
                    var response = registry.RetryRuntimeDeletionNow(
                        intentId,
                        request);
                    if (!response.Replayed &&
                        response.PendingIntent is not null)
                    {
                        recoverySignal.Pulse();
                    }
                    return Results.Json(
                        response,
                        LeserpentJsonContext.Default
                            .RuntimeDeletionRetryNowResponse);
                }
                catch (RuntimeDeletionRetryException ex) when (
                    string.Equals(
                        ex.Code,
                        "invalid_runtime_deletion_retry",
                        StringComparison.Ordinal))
                {
                    return Results.BadRequest(new ApiErrorResponse(
                        ex.Code,
                        ex.Message));
                }
                catch (RuntimeDeletionRetryException ex) when (
                    string.Equals(
                        ex.Code,
                        "runtime_deletion_intent_not_found",
                        StringComparison.Ordinal))
                {
                    return Results.NotFound(new ApiErrorResponse(
                        ex.Code,
                        ex.Message));
                }
                catch (RuntimeDeletionRetryException ex)
                {
                    return Results.Conflict(new ApiErrorResponse(
                        ex.Code,
                        ex.Message));
                }
                catch (OrchestraPersistenceException ex)
                {
                    return Results.Json(
                        new ApiErrorResponse(
                            "runtime_deletion_retry_persistence_unavailable",
                            ex.Message),
                        LeserpentJsonContext.Default.ApiErrorResponse,
                        statusCode:
                            StatusCodes.Status503ServiceUnavailable);
                }
            });

        app.MapPost("/v1/persistence/import", async (HttpRequest request, RegistryService registry, ControlPlaneStateStore stateStore, ControlPlaneSecurityPolicy security, CancellationToken cancellationToken) =>
        {
            PersistedControlPlaneState? imported;
            try
            {
                imported = await request.ReadFromJsonAsync(
                    LeserpentJsonContext.Default.PersistedControlPlaneState,
                    cancellationToken);
            }
            catch (Exception ex)
            {
                return Results.BadRequest(new ApiErrorResponse("invalid_persistence_import", ex.Message));
            }

            if (imported is null)
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "invalid_persistence_import",
                    "request body did not contain a control-plane state document"));
            }

            if (!stateStore.IsCompatible(imported))
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "incompatible_persistence_import",
                    SchemaVersion: imported.SchemaVersion,
                    ExpectedSchemaVersion: stateStore.SchemaVersion));
            }

            var importValidation = await security.ValidateImportAsync(imported, cancellationToken);
            if (importValidation is not null)
            {
                return Results.BadRequest(new ApiErrorResponse("invalid_persistence_import", importValidation));
            }

            try
            {
                return Results.Ok(registry.ImportState(imported));
            }
            catch (OrchestraPersistenceException ex)
            {
                return Results.Json(
                    new ApiErrorResponse("persistence_import_unavailable", ex.Message),
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    statusCode: StatusCodes.Status503ServiceUnavailable);
            }
            catch (RuntimeDeletionInProgressException ex)
            {
                return Results.Conflict(new ApiErrorResponse(
                    "persistence_import_runtime_delete_in_progress",
                    "control-plane state cannot be imported while runtime deletion is pending",
                    RuntimeId: ex.RuntimeIds.FirstOrDefault()));
            }
            catch (InvalidDataException ex)
            {
                return Results.BadRequest(new ApiErrorResponse(
                    "invalid_persistence_import",
                    ex.Message));
            }
        });
    }

    private static IResult RuntimeDeletionReconciliationFailure(
        RuntimeDeletionReconciliationException exception)
    {
        var response = new ApiErrorResponse(
            exception.Code,
            exception.Message);
        if (string.Equals(
                exception.Code,
                "invalid_runtime_deletion_reconciliation",
                StringComparison.Ordinal))
        {
            return Results.BadRequest(response);
        }
        if (string.Equals(
                exception.Code,
                "runtime_deletion_intent_not_found",
                StringComparison.Ordinal))
        {
            return Results.NotFound(response);
        }
        return Results.Conflict(response);
    }

    private static IResult RuntimeDeletionReconciliationUnavailable(
        string message) =>
        Results.Json(
            new ApiErrorResponse(
                "runtime_deletion_reconciliation_authority_unavailable",
                message),
            LeserpentJsonContext.Default.ApiErrorResponse,
            statusCode: StatusCodes.Status503ServiceUnavailable);

    private static IResult RuntimeDeletionReconciliationDaemonFailure(
        DaemonRuntimeProjectionException exception) =>
        Results.Json(
            new ApiErrorResponse(
                "runtime_deletion_reconciliation_authority_failed",
                exception.Message),
            LeserpentJsonContext.Default.ApiErrorResponse,
            statusCode: StatusCodes.Status502BadGateway);
}
