using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;

if (args.Length != 4)
{
    Console.Error.WriteLine(
        "usage: Leserpent.RuntimeDeletionCrashHarness STATE_PATH SOCKET_PATH MARKER_PATH RUNTIME_ID");
    return 64;
}

var statePath = Path.GetFullPath(args[0]);
var socketPath = Path.GetFullPath(args[1]);
var markerPath = Path.GetFullPath(args[2]);
var runtimeId = args[3];
var token = Environment.GetEnvironmentVariable("LESERPENT_DAEMON_TOKEN")
    ?? throw new InvalidOperationException("LESERPENT_DAEMON_TOKEN is required");
var configuration = new ConfigurationBuilder()
    .AddInMemoryCollection(new Dictionary<string, string?>
    {
        ["LESERPENT_STATE_PATH"] = statePath,
        ["LESERPENT_DAEMON_SOCKET"] = socketPath,
        ["LESERPENT_DAEMON_TOKEN"] = token,
        ["LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS"] = "10000",
    })
    .Build();
var environment = new HarnessEnvironment
{
    ContentRootPath = Path.GetDirectoryName(statePath)!,
};
var stateStore = new ControlPlaneStateStore(
    configuration,
    environment,
    NullLogger<ControlPlaneStateStore>.Instance);
var registry = new RegistryService(stateStore, new InMemoryOrchestraRunStore());
var authority = new DaemonRuntimeRegistrationAuthority(configuration);
var request = new RuntimeRegistrationRequest(
    "Crash Boundary Runtime",
    "https://runtime.example",
    "test-only-pairing-token");

registry.RegisterRuntime(request, runtimeId);
await authority.RegisterAsync(
    request,
    runtimeId,
    CancellationToken.None);
using var reservation = registry.ReserveRuntimeDeletion(new[] { runtimeId });
await authority.UnregisterAsync(reservation.RuntimeIds, CancellationToken.None);

var markerBytes = System.Text.Encoding.UTF8.GetBytes($"{reservation.IntentId}\n");
var markerTempPath = $"{markerPath}.{Environment.ProcessId}.tmp";
using (var marker = new FileStream(
    markerTempPath,
    FileMode.CreateNew,
    FileAccess.Write,
    FileShare.None))
{
    await marker.WriteAsync(markerBytes);
    marker.Flush(flushToDisk: true);
}
File.Move(markerTempPath, markerPath, overwrite: true);

await Task.Delay(Timeout.InfiniteTimeSpan);
return 0;

internal sealed class HarnessEnvironment : IHostEnvironment
{
    public string EnvironmentName { get; set; } = Environments.Development;
    public string ApplicationName { get; set; } = "Leserpent.RuntimeDeletionCrashHarness";
    public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
    public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
}
