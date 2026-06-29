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
                imported = await request.ReadFromJsonAsync<PersistedControlPlaneState>(cancellationToken: cancellationToken);
            }
            catch (Exception ex)
            {
                return Results.BadRequest(new
                {
                    error = "invalid_persistence_import",
                    reason = ex.Message,
                });
            }

            if (imported is null)
            {
                return Results.BadRequest(new
                {
                    error = "invalid_persistence_import",
                    reason = "request body did not contain a control-plane state document",
                });
            }

            if (!stateStore.IsCompatible(imported))
            {
                return Results.BadRequest(new
                {
                    error = "incompatible_persistence_import",
                    schemaVersion = imported.SchemaVersion,
                    expectedSchemaVersion = stateStore.SchemaVersion,
                });
            }

            var importValidation = await security.ValidateImportAsync(imported, cancellationToken);
            if (importValidation is not null)
            {
                return Results.BadRequest(new
                {
                    error = "invalid_persistence_import",
                    reason = importValidation,
                });
            }

            return Results.Ok(registry.ImportState(imported));
        });
    }
}
