using Leserpent.ControlPlane;

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
}
