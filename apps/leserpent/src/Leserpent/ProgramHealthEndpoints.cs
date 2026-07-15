using Leserpent.ControlPlane;

namespace Leserpent;

public partial class Program
{
    private static void MapHealthEndpoints(WebApplication app)
    {
        app.MapGet("/health", (ControlPlaneStateStore stateStore, IOrchestraRunStore orchestraRunStore, RegistryService registry, ControlPlaneSecurityPolicy security, ICompatibilityBridge compatibilityBridge) =>
            Results.Ok(new HealthResponse(
                true,
                "leserpent",
                "control-plane",
                typeof(Program).Assembly.GetName().Version?.ToString() ?? "dev",
                new HealthSecurityResponse(
                    security.ApiMode,
                    security.AdminTokenConfigured,
                    security.PublicEndpointDiscoveryAllowed),
                BuildRuntimePosture(stateStore, orchestraRunStore, compatibilityBridge),
                new HealthPersistenceResponse(
                    stateStore.StatePath,
                    stateStore.BackupStatePath,
                    stateStore.LastSavedAt,
                    stateStore.SchemaVersion,
                    stateStore.IsDirty,
                    stateStore.LastSaveError,
                    registry.RestoredRuntimeCount,
                    registry.RestoredSessionCount,
                    registry.RestoredFromSavedAt),
                new HealthOrchestraPersistenceResponse(
                    orchestraRunStore.Provider,
                    orchestraRunStore.Location,
                    orchestraRunStore.SchemaVersion,
                    orchestraRunStore.LastError,
                    string.IsNullOrWhiteSpace(orchestraRunStore.LastError)))));

        app.MapGet("/v1/capabilities", (ControlPlaneStateStore stateStore, IOrchestraRunStore orchestraRunStore, RegistryService registry, ControlPlaneSecurityPolicy security, ICompatibilityBridge compatibilityBridge) =>
            Results.Ok(new ServiceCapabilities(
                "leserpent",
                typeof(Program).Assembly.GetName().Version?.ToString() ?? "dev",
                "control-plane",
                new[]
                {
                    "/health",
                    "/v1/capabilities",
                    "/v1/persistence/export",
                    "/v1/persistence/import",
                    "/v1/persistence/save",
                    "/v1/fleet/summary",
                    "/v1/fleet/attention-summary",
                    "/v1/fleet/refresh-all",
                    "/v1/fleet/refresh-capabilities",
                    "/v1/fleet/refresh-sidecars",
                    "/v1/fleet/refresh-status",
                    "/v1/fleet/runtimes-needing-attention",
                    "/v1/orchestra/plans/{id}",
                    "/v1/orchestra/plans/{id}/{planId}/execute",
                    "/v1/orchestra/plans/{id}/session",
                    "/v1/orchestra/runtimes/{id}/runs",
                    "/v1/orchestra/runtimes/{id}/runs/{runId}/events",
                    "/v1/orchestra/runs",
                    "/v1/orchestra/runtimes/{id}/runs/{runId}/cancel",
                    "/v1/orchestra/runtimes/{id}/runs/{runId}/retry",
                    "/v1/runtimes",
                    "/v1/runtimes/{id}",
                    "/v1/runtimes/{id}/attention",
                    "/v1/runtimes/{id}/deployments",
                    "/v1/runtimes/{id}/sidecar",
                    "/v1/runtimes/{id}/refresh-sidecar",
                    "/v1/runtimes/{id}/status",
                    "/v1/runtimes/register",
                    "/v1/runtimes/delete-failed",
                    "/v1/runtimes/delete-unobserved",
                    "/v1/runtimes/delete-slice",
                    "/v1/runtimes/{id}/delete",
                    "/v1/runtimes/{id}/refresh-capabilities",
                    "/v1/runtimes/{id}/refresh-status",
                    "/v1/sessions",
                    "/v1/sessions/{id}",
                    "/v1/sessions/{id}/stop",
                },
                new ServicePersistenceCapabilities(
                    stateStore.StatePath,
                    stateStore.BackupStatePath,
                    stateStore.LastSavedAt,
                    true,
                    stateStore.SchemaVersion,
                    stateStore.IsDirty,
                    stateStore.LastSaveError,
                    registry.RestoredRuntimeCount,
                    registry.RestoredSessionCount,
                    registry.RestoredFromSavedAt,
                    orchestraRunStore.Provider,
                    orchestraRunStore.Location,
                    orchestraRunStore.LastError,
                    orchestraRunStore.SchemaVersion),
                new ServiceSecurityCapabilities(
                    security.ApiMode,
                    security.AdminTokenConfigured,
                    security.PublicEndpointDiscoveryAllowed),
                BuildRuntimePosture(stateStore, orchestraRunStore, compatibilityBridge))));
    }
}
