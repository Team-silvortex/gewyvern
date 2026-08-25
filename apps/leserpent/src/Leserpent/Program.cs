using System.IO.Compression;
using System.Text.Json;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.ResponseCompression;

namespace Leserpent;

public partial class Program
{
    public static void Main(string[] args)
    {
        var builder = WebApplication.CreateBuilder(new WebApplicationOptions
        {
            Args = args,
            ContentRootPath = AppContext.BaseDirectory,
        });

        builder.Services.AddAuthorization();
        builder.Services.ConfigureHttpJsonOptions(options =>
            options.SerializerOptions.TypeInfoResolverChain.Insert(0, LeserpentJsonContext.Default));
        builder.Services.AddOpenApi();
        builder.Services.AddResponseCompression(options =>
        {
            options.EnableForHttps = true;
            options.Providers.Add<BrotliCompressionProvider>();
            options.Providers.Add<GzipCompressionProvider>();
            options.MimeTypes = ResponseCompressionDefaults.MimeTypes.Concat(new[]
            {
                "application/javascript",
                "application/json",
                "image/svg+xml",
            });
        });
        builder.Services.Configure<BrotliCompressionProviderOptions>(options =>
            options.Level = CompressionLevel.Fastest);
        builder.Services.Configure<GzipCompressionProviderOptions>(options =>
            options.Level = CompressionLevel.Fastest);
        builder.Services.AddSingleton<ControlPlaneSecurityPolicy>();
        builder.Services.AddSingleton<ControlPlaneStateStore>();
        builder.Services.AddSingleton<SqliteOrchestraRunStore>(services =>
        {
            var writerFence = services.GetRequiredService<
                ControlPlaneWriterFence>();
            return new SqliteOrchestraRunStore(
                services.GetRequiredService<IConfiguration>(),
                services.GetRequiredService<IHostEnvironment>(),
                services.GetRequiredService<
                    ILogger<SqliteOrchestraRunStore>>(),
                () => writerFence.IsWriter);
        });
        builder.Services.AddSingleton<DaemonOrchestraRunStore>();
        builder.Services.AddSingleton<IOrchestraRunStore>(services =>
        {
            var daemon = services.GetRequiredService<DaemonOrchestraRunStore>();
            return daemon.Enabled ? daemon : services.GetRequiredService<SqliteOrchestraRunStore>();
        });
        builder.Services.AddSingleton<ControlPlaneWriterLease>();
        builder.Services.AddSingleton<DaemonAuthorityWriterSession>();
        builder.Services.AddSingleton<ControlPlaneWriterFence>();
        builder.Services.AddSingleton<IHostedService>(services =>
            services.GetRequiredService<ControlPlaneWriterFence>());
        builder.Services.AddSingleton<
            OrchestraDeleteCheckpointWorkerLease>();
        builder.Services.AddSingleton<RegistryService>(services =>
            new RegistryService(
                services.GetRequiredService<
                    ControlPlaneStateStore>(),
                services.GetRequiredService<
                    IOrchestraRunStore>(),
                services.GetRequiredService<
                    OrchestraDeleteCheckpointWorkerLease>(),
                services.GetRequiredService<
                    ControlPlaneWriterFence>()));
        builder.Services.AddSingleton<ICompatibilityBridge, RustCompatibilityBridge>();
        builder.Services.AddSingleton<IDeploymentAuthority, DaemonDeploymentAuthority>();
        builder.Services.AddSingleton<DaemonRuntimeRegistrationAuthority>();
        builder.Services.AddSingleton<IRuntimeRegistrationAuthority>(services =>
            services.GetRequiredService<DaemonRuntimeRegistrationAuthority>());
        builder.Services.AddSingleton<IDaemonRuntimeProjectionReader>(services =>
            services.GetRequiredService<DaemonRuntimeRegistrationAuthority>());
        builder.Services.AddSingleton<RuntimeDeletionRecoverySignal>();
        builder.Services.AddHostedService<RuntimeDeletionRecoveryService>();
        builder.Services.AddSingleton(
            OrchestraDeleteCheckpointWorkerOptions.Default);
        builder.Services.AddSingleton<
            LoggingOrchestraDeleteCheckpointAlertSink>();
        builder.Services.AddHttpClient(
                OrchestraDeleteCheckpointAlertSinkFactory
                    .HttpClientName,
                client =>
                {
                    client.Timeout = TimeSpan.FromSeconds(5);
                })
            .ConfigurePrimaryHttpMessageHandler(() =>
                new SocketsHttpHandler
                {
                    AllowAutoRedirect = false,
                });
        builder.Services.AddSingleton<
            IOrchestraDeleteCheckpointAlertSink>(services =>
                OrchestraDeleteCheckpointAlertSinkFactory.Create(
                    services.GetRequiredService<IConfiguration>(),
                    services.GetRequiredService<
                        IHttpClientFactory>(),
                    services.GetRequiredService<
                        LoggingOrchestraDeleteCheckpointAlertSink>()));
        builder.Services.AddSingleton<
            OrchestraDeleteCheckpointWorkerHealth>();
        builder.Services.AddHostedService<
            OrchestraDeleteCheckpointService>();
        builder.Services.AddSingleton<RuntimeReadProjectionService>();
        builder.Services.AddSingleton<RuntimeCommandExecutionContextService>();
        builder.Services.AddSingleton<RuntimeRegistrationCommitProjectionService>();
        builder.Services.AddSingleton<FleetReadProjectionService>();
        builder.Services.AddSingleton<RuntimeCleanupProjectionService>();
        builder.Services.AddSingleton<OrchestraRuntimeProjectionService>();
        builder.Services.AddHttpClient<CapabilityDiscoveryService>();
        builder.Services.AddSingleton<IOrchestraPlanExecutor, OrchestraPlanExecutor>();
        builder.Services.AddSingleton<OrchestraExecutionCoordinator>();

        var app = builder.Build();

        if (app.Environment.IsDevelopment())
        {
            app.MapOpenApi();
        }

        app.UseResponseCompression();
        app.Use(async (context, next) =>
        {
            BrowserSecurityHeaders.Apply(context.Response);
            await next();
        });
        app.UseDefaultFiles();
        app.UseRouting();
        app.MapStaticAssets();
        app.UseHttpsRedirection();
        app.Use(async (context, next) =>
        {
            if (!LanguagePackRequestPolicy.TryAccept(context.Request, out var payload))
            {
                context.Response.StatusCode = StatusCodes.Status400BadRequest;
                await context.Response.WriteAsJsonAsync(
                    payload,
                    LeserpentJsonContext.Default.ApiErrorResponse,
                    cancellationToken: context.RequestAborted);
                return;
            }

            await next();
        });
        app.Use(async (context, next) =>
        {
            var security = context.RequestServices.GetRequiredService<ControlPlaneSecurityPolicy>();
            if (!security.TryAuthorize(context, out var statusCode, out var payload))
            {
                context.Response.StatusCode = statusCode;
                await context.Response.WriteAsJsonAsync(
                    payload,
                    LeserpentJsonContext.Default.ApiErrorResponse);
                return;
            }

            await next();
        });
        app.Use(async (context, next) =>
        {
            if (ControlPlaneMutationPolicy.IsMutation(
                    context.Request))
            {
                var writer = context.RequestServices
                    .GetRequiredService<ControlPlaneWriterFence>();
                if (!writer.IsWriter)
                {
                    context.Response.StatusCode =
                        StatusCodes.Status409Conflict;
                    await context.Response.WriteAsJsonAsync(
                        new ApiErrorResponse(
                            ControlPlaneWriterUnavailableException
                                .ErrorCode,
                            "This leserpentd instance is read-only because another process owns the control-plane writer lease."),
                        LeserpentJsonContext.Default.ApiErrorResponse,
                        cancellationToken:
                            context.RequestAborted);
                    return;
                }
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

    internal static ServiceRuntimePosture BuildRuntimePosture(
        ControlPlaneStateStore stateStore,
        IOrchestraRunStore orchestraRunStore,
        ICompatibilityBridge compatibilityBridge,
        IDeploymentAuthority deploymentAuthority)
    {
        var loadFailed = stateStore.LoadProvenance.Outcome ==
            ControlPlaneStateLoadOutcome.Failed;
        var persistenceReady = !loadFailed &&
            string.IsNullOrWhiteSpace(stateStore.LastSaveError)
            && string.IsNullOrWhiteSpace(orchestraRunStore.LastError);
        var persistenceDegraded =
            stateStore.LoadProvenance.Degraded ||
            !string.IsNullOrWhiteSpace(stateStore.LastSaveError) ||
            !string.IsNullOrWhiteSpace(orchestraRunStore.LastError);
        return new ServiceRuntimePosture(
            CoreReady: true,
            PersistenceReady: persistenceReady,
            DegradedButOperable: persistenceDegraded,
            OptionalAdapters: new[]
            {
                new ServiceOptionalAdapter(
                    "rust_compatibility_bridge",
                    compatibilityBridge.Enabled ? "configured" : "optional_unconfigured",
                    compatibilityBridge.Enabled
                        ? "The 1.x runtime list and status refresh compatibility checks are routed through Rust."
                        : "Set LESERPENT_RUST_BRIDGE_BIN to an absolute bridge binary path to enable Rust compatibility checks."),
                new ServiceOptionalAdapter(
                    "leserpentd_deployment_authority",
                    deploymentAuthority.Enabled ? "configured" : "optional_unconfigured",
                    deploymentAuthority.Enabled
                        ? "Configured deployments are submitted to leserpentd and resolved from its persisted typed receipt."
                        : "Set LESERPENT_DAEMON_SOCKET and LESERPENT_DAEMON_TOKEN together to route deployment authority through leserpentd."),
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

    internal static string DetermineRefreshOutcome(
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
    public static IResult FileDownloadJson(PersistedControlPlaneState payload, string fileName) =>
        Results.File(
            JsonSerializer.SerializeToUtf8Bytes(payload, LeserpentJsonContext.Default.PersistedControlPlaneState),
            "application/json",
            fileName);
}
