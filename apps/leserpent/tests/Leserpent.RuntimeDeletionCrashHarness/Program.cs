using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;

if (args.Length == 2 &&
    string.Equals(
        args[0],
        "checkpoint-worker-lease-hold",
        StringComparison.Ordinal))
{
    var leaseStatePath = Path.GetFullPath(args[1]);
    var leaseConfiguration = new ConfigurationBuilder()
        .AddInMemoryCollection(new Dictionary<string, string?>
        {
            ["LESERPENT_STATE_PATH"] = leaseStatePath,
        })
        .Build();
    var leaseStore = new ControlPlaneStateStore(
        leaseConfiguration,
        new HarnessEnvironment
        {
            ContentRootPath =
                Path.GetDirectoryName(leaseStatePath)!,
        },
        NullLogger<ControlPlaneStateStore>.Instance);
    using var lease =
        new OrchestraDeleteCheckpointWorkerLease(leaseStore);
    if (!lease.TryAcquire())
    {
        Console.Error.WriteLine("checkpoint worker lease unavailable");
        return 73;
    }
    Console.WriteLine("checkpoint-worker-lease-held");
    await Console.Out.FlushAsync();
    _ = await Console.In.ReadLineAsync();
    return 0;
}

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
IOrchestraRunStore orchestraRunStore =
    string.Equals(
        phase,
        "reconciliation_cross_authority",
        StringComparison.Ordinal)
        ? new PauseAfterOrchestraCleanupRunStore(
            new DaemonOrchestraRunStore(
                configuration,
                NullLogger<DaemonOrchestraRunStore>.Instance),
            markerPath)
        : new InMemoryOrchestraRunStore();
var registry = new RegistryService(stateStore, orchestraRunStore);
var authority = new DaemonRuntimeRegistrationAuthority(configuration);

if (string.Equals(
    phase,
    "retry_rollover_persist",
    StringComparison.Ordinal))
{
    var state = registry.ExportState();
    var audit = state.RuntimeDeletionRetryAudit?.ToArray()
        ?? Array.Empty<PersistedRuntimeDeletionRetryAudit>();
    if (audit.Length != 256)
    {
        throw new InvalidOperationException(
            "retry rollover harness requires exactly 256 baseline audit records");
    }
    var newest = audit[^1];
    var replacement = new PersistedRuntimeDeletionRetryAudit(
        "retry-atomic-rollover-256",
        "rdel_atomic_rollover_256",
        newest.RuntimeIds.ToArray(),
        2,
        3,
        "atomic-rollover",
        newest.RequestedAt.AddTicks(1));
    var replacementAudit = audit
        .Skip(1)
        .Append(replacement)
        .ToArray();
    await WriteMarkerAsync(
        markerPath,
        $"retry_rollover_ready {audit[0].RequestId} {replacement.RequestId}\n");
    await WaitForTriggerAsync($"{markerPath}.trigger");
    stateStore.SaveStrict(
        state.Runtimes,
        state.Sessions,
        state.OrchestraRuns,
        state.PendingRuntimeDeletions,
        replacementAudit);
    await WriteMarkerAsync(
        $"{markerPath}.committed",
        $"retry_rollover_committed {replacement.RequestId}\n");
    await Task.Delay(Timeout.InfiniteTimeSpan);
}
else if (string.Equals(
    phase,
    "reconciliation_commit",
    StringComparison.Ordinal))
{
    var intent = registry.ListPendingRuntimeDeletions().Single();
    var daemonSnapshot = await authority.SnapshotAsync(
        CancellationToken.None);
    var request = new RuntimeDeletionReconcileRequest(
        intent.Revision,
        daemonSnapshot.Revision,
        runtimeId,
        "reconciliation-crash-campaign",
        true);
    var reconciliation = registry.BeginRuntimeDeletionReconciliation(
        intent.IntentId,
        request);
    using var reservation = reconciliation.Reservation
        ?? throw new InvalidOperationException(
            "reconciliation crash harness unexpectedly replayed");
    await WriteMarkerAsync(
        markerPath,
        $"reconciliation_ready {intent.IntentId} {intent.Revision} {daemonSnapshot.Revision}\n");
    await WaitForTriggerAsync($"{markerPath}.trigger");
    registry.CompleteRuntimeDeletionReconciliation(
        reservation,
        request,
        daemonSnapshot);
    await WriteMarkerAsync(
        $"{markerPath}.committed",
        $"reconciliation_committed {request.RequestId}\n");
    await Task.Delay(Timeout.InfiniteTimeSpan);
}
else if (string.Equals(
    phase,
    "reconciliation_cross_authority",
    StringComparison.Ordinal))
{
    var intent = registry.ListPendingRuntimeDeletions().Single();
    var daemonSnapshot = await authority.SnapshotAsync(
        CancellationToken.None);
    var request = new RuntimeDeletionReconcileRequest(
        intent.Revision,
        daemonSnapshot.Revision,
        runtimeId,
        "reconciliation-cross-authority-campaign",
        true);
    var reconciliation = registry.BeginRuntimeDeletionReconciliation(
        intent.IntentId,
        request);
    using var reservation = reconciliation.Reservation
        ?? throw new InvalidOperationException(
            "cross-authority reconciliation unexpectedly replayed");
    registry.CompleteRuntimeDeletionReconciliation(
        reservation,
        request,
        daemonSnapshot);
    await WriteMarkerAsync(
        $"{markerPath}.committed",
        $"reconciliation_cross_authority_committed {request.RequestId}\n");
    await Task.Delay(Timeout.InfiniteTimeSpan);
}
else if (string.Equals(phase, "mixed_overlapping", StringComparison.Ordinal))
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
else if (string.Equals(
    phase,
    "lost_ack_daemon_committed",
    StringComparison.Ordinal))
{
    var request = CrashBoundaryRequest(runtimeId);
    registry.RegisterRuntime(request, runtimeId);
    await authority.RegisterAsync(
        request,
        runtimeId,
        CancellationToken.None);
    string intentId;
    using (var reservation = registry.ReserveRuntimeDeletion(
        new[] { runtimeId }))
    {
        intentId = reservation.IntentId;
    }

    var lostAcknowledgementAuthority =
        new LostAcknowledgementUnregisterAuthority(
            authority,
            markerPath,
            intentId);
    var recovery = new RuntimeDeletionRecoveryService(
        registry,
        lostAcknowledgementAuthority,
        NullLogger<RuntimeDeletionRecoveryService>.Instance);
    await recovery.StartAsync(CancellationToken.None);
    await Task.Delay(Timeout.InfiniteTimeSpan);
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
    await WriteMarkerAsync(
        markerPath,
        $"{phase} {intentId}\n");
    await Task.Delay(Timeout.InfiniteTimeSpan);
}

static async Task WriteMarkerAsync(
    string markerPath,
    string content)
{
    var markerBytes = System.Text.Encoding.UTF8.GetBytes(content);
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
}

static async Task WaitForTriggerAsync(string triggerPath)
{
    while (!File.Exists(triggerPath))
    {
        await Task.Delay(1);
    }
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

internal sealed class LostAcknowledgementUnregisterAuthority(
    IRuntimeRegistrationAuthority inner,
    string markerPath,
    string intentId) : IRuntimeRegistrationAuthority
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
        throw new InvalidOperationException(
            "lost-ack harness requires the durable command identity");

    public async Task UnregisterAsync(
        IReadOnlyCollection<string> runtimeIds,
        string commandId,
        CancellationToken cancellationToken)
    {
        await inner.UnregisterAsync(
            runtimeIds,
            commandId,
            cancellationToken);
        await WriteBoundaryMarkerAsync(
            $"{intentId} {commandId}\n");
        await Task.Delay(Timeout.InfiniteTimeSpan);
    }

    public Task<RuntimeUnregistrationReceiptLookup>
        LookupUnregistrationReceiptAsync(
            string commandId,
            CancellationToken cancellationToken) =>
        inner.LookupUnregistrationReceiptAsync(
            commandId,
            cancellationToken);

    private async Task WriteBoundaryMarkerAsync(string content)
    {
        var markerBytes =
            System.Text.Encoding.UTF8.GetBytes(content);
        var markerTempPath =
            $"{markerPath}.{Environment.ProcessId}.tmp";
        using (var marker = new FileStream(
            markerTempPath,
            FileMode.CreateNew,
            FileAccess.Write,
            FileShare.None))
        {
            await marker.WriteAsync(markerBytes);
            marker.Flush(flushToDisk: true);
        }
        File.Move(
            markerTempPath,
            markerPath,
            overwrite: true);
    }
}

internal sealed class PauseAfterOrchestraCleanupRunStore(
    IOrchestraRunStore inner,
    string markerPath) : IOrchestraRunStore
{
    public string Provider => inner.Provider;
    public string Location => inner.Location;
    public int SchemaVersion => inner.SchemaVersion;
    public bool SupportsDeleteReplayHorizon =>
        inner.SupportsDeleteReplayHorizon;
    public string? LastError => inner.LastError;

    public IReadOnlyList<OrchestraRunSummary> LoadAll() =>
        inner.LoadAll();

    public IReadOnlyList<OrchestraRunEvent> LoadEvents(
        string runtimeId,
        string runId) =>
        inner.LoadEvents(runtimeId, runId);

    public bool Upsert(
        OrchestraRunSummary run,
        OrchestraRunEvent? eventRecord = null) =>
        inner.Upsert(run, eventRecord);

    public bool ReplaceAll(
        IReadOnlyList<OrchestraRunSummary> runs) =>
        inner.ReplaceAll(runs);

    public bool DeleteRuntimes(
        IReadOnlyCollection<string> runtimeIds)
    {
        if (!inner.DeleteRuntimes(runtimeIds))
        {
            return false;
        }
        WriteBoundaryMarker(
            $"orchestra_cleanup_committed {string.Join(',', runtimeIds)}\n");
        WaitForTrigger();
        return true;
    }

    public OrchestraDeleteReceipt? DeleteRuntimes(
        OrchestraDeleteCommand command)
    {
        var receipt = inner.DeleteRuntimes(command);
        if (receipt is null)
        {
            return null;
        }
        WriteBoundaryMarker(
            $"orchestra_cleanup_committed {receipt.CommandId} {receipt.OperationGeneration} {receipt.Replayed}\n");
        WaitForTrigger();
        return receipt;
    }

    public OrchestraDeleteReplayHorizon? GetDeleteReplayHorizon() =>
        inner.GetDeleteReplayHorizon();

    public OrchestraDeleteReplayHorizon? CheckpointDeleteReplayHorizon(
        OrchestraDeleteReplayCheckpoint checkpoint) =>
        inner.CheckpointDeleteReplayHorizon(checkpoint);

    private void WriteBoundaryMarker(string content)
    {
        var markerBytes =
            System.Text.Encoding.UTF8.GetBytes(content);
        var markerTempPath =
            $"{markerPath}.{Environment.ProcessId}.tmp";
        using (var marker = new FileStream(
            markerTempPath,
            FileMode.CreateNew,
            FileAccess.Write,
            FileShare.None))
        {
            marker.Write(markerBytes);
            marker.Flush(flushToDisk: true);
        }
        File.Move(
            markerTempPath,
            markerPath,
            overwrite: true);
    }

    private void WaitForTrigger()
    {
        var triggerPath = $"{markerPath}.trigger";
        while (!File.Exists(triggerPath))
        {
            Thread.Sleep(1);
        }
    }
}
