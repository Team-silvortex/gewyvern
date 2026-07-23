using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;

if (args.Length != 5)
{
    Console.Error.WriteLine(
        "usage: Leserpent.RuntimeDeletionCrashHarness STATE_PATH SOCKET_PATH MARKER_PATH RUNTIME_ID PHASE");
    return 64;
}

var statePath = Path.GetFullPath(args[0]);
var socketPath = Path.GetFullPath(args[1]);
var markerPath = Path.GetFullPath(args[2]);
var runtimeId = args[3];
var phase = args[4];
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

if (string.Equals(phase, "mixed_overlapping", StringComparison.Ordinal))
{
    var phases = new[]
    {
        "intent_persisted",
        "daemon_committed",
        "local_cleanup_persisted",
    };
    var reservations = new List<RuntimeDeletionReservation>();
    foreach (var boundary in phases)
    {
        var targetId = $"{runtimeId}-{boundary.Replace('_', '-')}";
        var request = CrashBoundaryRequest(targetId);
        registry.RegisterRuntime(request, targetId);
        await authority.RegisterAsync(request, targetId, CancellationToken.None);
        var reservation = registry.ReserveRuntimeDeletion(new[] { targetId });
        reservations.Add(reservation);
        await Task.Delay(2);
        if (!string.Equals(boundary, "intent_persisted", StringComparison.Ordinal))
        {
            await authority.UnregisterAsync(reservation.RuntimeIds, CancellationToken.None);
        }
        if (string.Equals(boundary, "local_cleanup_persisted", StringComparison.Ordinal))
        {
            registry.DeleteRuntime(targetId);
        }
    }
    await PauseAtBoundaryAsync(
        markerPath,
        string.Join(',', reservations.Select(static reservation => reservation.IntentId)),
        phase);
}
else if (string.Equals(phase, "high_cardinality", StringComparison.Ordinal))
{
    var reservations = new List<RuntimeDeletionReservation>();
    foreach (var index in Enumerable.Range(0, 32))
    {
        var targetId = $"{runtimeId}-queue-{index:D2}";
        var request = CrashBoundaryRequest(targetId);
        registry.RegisterRuntime(request, targetId);
        await authority.RegisterAsync(request, targetId, CancellationToken.None);
        reservations.Add(registry.ReserveRuntimeDeletion(new[] { targetId }));
        await Task.Delay(2);
    }
    await PauseAtBoundaryAsync(
        markerPath,
        string.Join(',', reservations.Select(static reservation => reservation.IntentId)),
        phase);
}
else if (
    string.Equals(phase, "retry_acknowledged", StringComparison.Ordinal) ||
    string.Equals(phase, "retry_daemon_committed", StringComparison.Ordinal))
{
    var request = CrashBoundaryRequest(runtimeId);
    registry.RegisterRuntime(request, runtimeId);
    await authority.RegisterAsync(
        request,
        runtimeId,
        CancellationToken.None);
    using (registry.ReserveRuntimeDeletion(new[] { runtimeId }))
    {
    }

    var failingAuthority = new FailingUnregisterAuthority(authority);
    var failureRecovery = new RuntimeDeletionRecoveryService(
        registry,
        failingAuthority,
        NullLogger<RuntimeDeletionRecoveryService>.Instance);
    await failureRecovery.StartAsync(CancellationToken.None);
    var deferredIntent = await WaitForDeferredIntentAsync(registry);
    await failureRecovery.StopAsync(CancellationToken.None);
    failureRecovery.Dispose();

    var retryResponse = registry.RetryRuntimeDeletionNow(
        deferredIntent.IntentId,
        new RuntimeDeletionRetryNowRequest(
            deferredIntent.Revision,
            $"retry-crash-{runtimeId}",
            "crash-harness"));
    if (retryResponse.Replayed ||
        retryResponse.PendingIntent?.Revision != deferredIntent.Revision + 1)
    {
        throw new InvalidOperationException(
            "retry-now acknowledgement did not advance the durable intent");
    }

    if (string.Equals(phase, "retry_acknowledged", StringComparison.Ordinal))
    {
        await PauseAtBoundaryAsync(
            markerPath,
            deferredIntent.IntentId,
            phase);
    }
    else
    {
        using var recoveryReservation = registry
            .ClaimPendingRuntimeDeletions(1)
            .Single();
        await authority.UnregisterAsync(
            recoveryReservation.RuntimeIds,
            CancellationToken.None);
        await PauseAtBoundaryAsync(
            markerPath,
            deferredIntent.IntentId,
            phase);
    }
}
else
{
    var request = CrashBoundaryRequest(runtimeId);
    registry.RegisterRuntime(request, runtimeId);
    await authority.RegisterAsync(
        request,
        runtimeId,
        CancellationToken.None);
    using var reservation = registry.ReserveRuntimeDeletion(new[] { runtimeId });
    switch (phase)
    {
        case "intent_persisted":
            await PauseAtBoundaryAsync(markerPath, reservation.IntentId, phase);
            break;
        case "daemon_committed":
            await authority.UnregisterAsync(reservation.RuntimeIds, CancellationToken.None);
            await PauseAtBoundaryAsync(markerPath, reservation.IntentId, phase);
            break;
        case "local_cleanup_persisted":
            await authority.UnregisterAsync(reservation.RuntimeIds, CancellationToken.None);
            registry.DeleteRuntime(runtimeId);
            await PauseAtBoundaryAsync(markerPath, reservation.IntentId, phase);
            break;
        default:
            Console.Error.WriteLine($"unknown runtime deletion crash phase: {phase}");
            return 64;
    }
}

return 0;

static RuntimeRegistrationRequest CrashBoundaryRequest(string runtimeId) =>
    new(
        $"Crash Boundary Runtime {runtimeId}",
        $"https://{runtimeId}.example",
        "test-only-pairing-token");

static async Task PauseAtBoundaryAsync(
    string markerPath,
    string intentId,
    string phase)
{
    var markerBytes = System.Text.Encoding.UTF8.GetBytes(
        $"{phase} {intentId}\n");
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
}

static async Task<PersistedRuntimeDeletionIntent> WaitForDeferredIntentAsync(
    RegistryService registry)
{
    var deadline = DateTimeOffset.UtcNow.AddSeconds(5);
    while (DateTimeOffset.UtcNow < deadline)
    {
        var intent = registry.ListPendingRuntimeDeletions().Single();
        if (intent.AttemptCount == 1 &&
            intent.NextAttemptAt is not null &&
            intent.LastFailureCode == "authority_unavailable")
        {
            return intent;
        }
        await Task.Delay(10);
    }
    throw new TimeoutException(
        "runtime deletion failure metadata did not become durable");
}

internal sealed class HarnessEnvironment : IHostEnvironment
{
    public string EnvironmentName { get; set; } = Environments.Development;
    public string ApplicationName { get; set; } = "Leserpent.RuntimeDeletionCrashHarness";
    public string ContentRootPath { get; set; } = AppContext.BaseDirectory;
    public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
}

internal sealed class FailingUnregisterAuthority(
    IRuntimeRegistrationAuthority inner) : IRuntimeRegistrationAuthority
{
    public bool Enabled => inner.Enabled;

    public Task<string> RegisterAsync(
        RuntimeRegistrationRequest request,
        string runtimeId,
        CancellationToken cancellationToken,
        bool update = false,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
        inner.RegisterAsync(
            request,
            runtimeId,
            cancellationToken,
            update,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery);

    public Task SubmitDiscoveryAsync(
        string runtimeId,
        CancellationToken cancellationToken,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
        inner.SubmitDiscoveryAsync(
            runtimeId,
            cancellationToken,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery);

    public Task UnregisterAsync(
        IReadOnlyCollection<string> runtimeIds,
        CancellationToken cancellationToken) =>
        throw new IOException(
            "test authority is unavailable before retry acknowledgement");
}
