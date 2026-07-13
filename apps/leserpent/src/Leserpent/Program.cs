using System.Text.Json;
using Leserpent.ControlPlane;

namespace Leserpent;

public partial class Program
{
    public static void Main(string[] args)
    {
        var builder = WebApplication.CreateBuilder(args);

        builder.Services.AddAuthorization();
        builder.Services.AddOpenApi();
        builder.Services.AddSingleton<ControlPlaneSecurityPolicy>();
        builder.Services.AddSingleton<ControlPlaneStateStore>();
        builder.Services.AddSingleton<RegistryService>();
        builder.Services.AddHttpClient<CapabilityDiscoveryService>();

        var app = builder.Build();

        if (app.Environment.IsDevelopment())
        {
            app.MapOpenApi();
        }

        app.UseDefaultFiles();
        app.UseStaticFiles();
        app.UseHttpsRedirection();
        app.Use(async (context, next) =>
        {
            var security = context.RequestServices.GetRequiredService<ControlPlaneSecurityPolicy>();
            if (!security.TryAuthorize(context, out var statusCode, out var payload))
            {
                context.Response.StatusCode = statusCode;
                await context.Response.WriteAsJsonAsync(payload);
                return;
            }

            await next();
        });
        app.UseAuthorization();

        MapHealthEndpoints(app);
        MapPersistenceEndpoints(app);
        MapFleetEndpoints(app);
        MapRuntimeEndpoints(app);
        MapOrchestraEndpoints(app);
        MapSessionEndpoints(app);

        app.MapFallbackToFile("index.html");

        app.Run();
    }

    private static ServiceRuntimePosture BuildRuntimePosture(ControlPlaneStateStore stateStore)
    {
        var persistenceReady = string.IsNullOrWhiteSpace(stateStore.LastSaveError);
        return new ServiceRuntimePosture(
            CoreReady: true,
            PersistenceReady: persistenceReady,
            DegradedButOperable: !persistenceReady,
            OptionalAdapters: new[]
            {
                new ServiceOptionalAdapter(
                    "docker_scenarios",
                    "optional_unconfigured",
                    "Docker-backed scenario launch and stack validation are optional helpers, not startup requirements."),
                new ServiceOptionalAdapter(
                    "local_process_launch",
                    "optional_unconfigured",
                    "Local process launch helpers should remain optional rather than part of the core boot contract."),
                new ServiceOptionalAdapter(
                    "remote_ssh_management",
                    "optional_unconfigured",
                    "Remote SSH-based management is an optional adapter and should not be required for local control-plane operation."),
                new ServiceOptionalAdapter(
                    "kubernetes_integration",
                    "optional_unconfigured",
                    "Future scheduler integration is optional and should not block the control plane from starting."),
            });
    }

    private static string DetermineRefreshOutcome(
        string? runtimeStatusSource,
        string? runtimeStatusError,
        string? sidecarStatusSource,
        string? sidecarStatusError)
    {
        if (string.Equals(runtimeStatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase) ||
            string.Equals(sidecarStatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase))
        {
            var combined = string.Join(" · ", new[] { runtimeStatusError, sidecarStatusError }.Where(static x => !string.IsNullOrWhiteSpace(x)));
            if (LooksLikeAuthFailure(combined))
            {
                return "auth_failed";
            }

            if (LooksLikeNetworkFailure(combined))
            {
                return "network_failed";
            }

            if (LooksLikeIncompleteData(combined))
            {
                return "incomplete_data";
            }

            return "degraded";
        }

        return "ok";
    }

    private static string BuildRecoverySummary(
        string? runtimeStatusSource,
        string? runtimeStatusError,
        string? sidecarStatusSource,
        string? sidecarStatusError)
    {
        var parts = new List<string>();
        if (!string.IsNullOrWhiteSpace(runtimeStatusSource))
        {
            parts.Add($"runtime:{runtimeStatusSource}");
        }

        if (!string.IsNullOrWhiteSpace(runtimeStatusError))
        {
            parts.Add($"runtime-error:{runtimeStatusError}");
        }

        if (!string.IsNullOrWhiteSpace(sidecarStatusSource))
        {
            parts.Add($"sidecar:{sidecarStatusSource}");
        }

        if (!string.IsNullOrWhiteSpace(sidecarStatusError))
        {
            parts.Add($"sidecar-error:{sidecarStatusError}");
        }

        return parts.Count > 0
            ? string.Join(" · ", parts)
            : "refresh completed";
    }

    private static bool LooksLikeAuthFailure(string message) =>
        message.Contains("401", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("403", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("unauthorized", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("forbidden", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("token", StringComparison.OrdinalIgnoreCase);

    private static bool LooksLikeNetworkFailure(string message) =>
        message.Contains("connection", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("refused", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("timed out", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("timeout", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("dns", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("socket", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("host", StringComparison.OrdinalIgnoreCase);

    private static bool LooksLikeIncompleteData(string message) =>
        message.Contains("decode", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("payload", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("json", StringComparison.OrdinalIgnoreCase) ||
        message.Contains("snapshot", StringComparison.OrdinalIgnoreCase);
}

internal static class ResultExtensions
{
    private static readonly JsonSerializerOptions SerializerOptions = new()
    {
        WriteIndented = true,
    };

    public static IResult FileDownloadJson<T>(T payload, string fileName) =>
        Results.File(
            JsonSerializer.SerializeToUtf8Bytes(payload, SerializerOptions),
            "application/json",
            fileName);
}
