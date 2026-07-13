using Leserpent.ControlPlane;

namespace Leserpent;

public partial class Program
{
    private static void MapPersistenceEndpoints(WebApplication app)
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
        });
    }
}
