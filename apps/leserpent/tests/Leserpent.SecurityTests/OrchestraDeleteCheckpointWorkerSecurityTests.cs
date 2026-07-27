using System.Net;
using System.Diagnostics;
using System.Text.Json;
using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class OrchestraDeleteCheckpointWorkerSecurityTests
{
    [Fact]
    public async Task DuplicateHostsLeaseFenceAuthorityAndAlertDelivery()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var now = DateTimeOffset.UtcNow;
            var horizon = new OrchestraDeleteReplayHorizon(
                Capacity: 4096,
                Retained: 1,
                OldestGeneration: 1,
                NewestGeneration: 1,
                NextGeneration: 2,
                EvictedThroughGeneration: 0,
                ProtectedFromGeneration: 1,
                CheckpointedThroughGeneration: null);
            SeedCheckpointAlertState(statePath, now, horizon);
            var authority = new SharedCheckpointAuthority(horizon);
            var firstStateStore = CreateStateStore(statePath);
            var secondStateStore = CreateStateStore(statePath);
            var firstLease =
                new OrchestraDeleteCheckpointWorkerLease(
                    firstStateStore);
            var secondLease =
                new OrchestraDeleteCheckpointWorkerLease(
                    secondStateStore);
            var firstRegistry = new RegistryService(
                firstStateStore,
                new LeaseCheckpointStore(authority, "first"),
                firstLease);
            var secondRegistry = new RegistryService(
                secondStateStore,
                new LeaseCheckpointStore(authority, "second"),
                secondLease);
            var firstSink = new RecordingAlertSink();
            var secondSink = new RecordingAlertSink();
            var options = new OrchestraDeleteCheckpointWorkerOptions(
                TimeSpan.FromMilliseconds(20),
                TimeSpan.FromMilliseconds(5));
            using var firstWorker =
                new OrchestraDeleteCheckpointService(
                    firstRegistry,
                    firstSink,
                    NullLogger<
                        OrchestraDeleteCheckpointService>.Instance,
                    options,
                    firstLease);
            using var secondWorker =
                new OrchestraDeleteCheckpointService(
                    secondRegistry,
                    secondSink,
                    NullLogger<
                        OrchestraDeleteCheckpointService>.Instance,
                    options,
                    secondLease);
            try
            {
                await firstWorker.StartAsync(
                    CancellationToken.None);
                await WaitUntilAsync(
                    () => firstLease.IsHeld,
                    TimeSpan.FromSeconds(2));
                await secondWorker.StartAsync(
                    CancellationToken.None);
                await WaitUntilAsync(
                    () =>
                        firstSink.EventIds.Count == 1 &&
                        authority.CheckpointMutationCount == 1,
                    TimeSpan.FromSeconds(3));
                _ = secondRegistry
                    .GetOrchestraDeleteReplayCheckpointStatus();
                await Task.Delay(TimeSpan.FromMilliseconds(100));

                Assert.False(secondLease.IsHeld);
                Assert.Empty(secondSink.EventIds);
                Assert.Equal(
                    0,
                    authority.ReadCount("second"));
                Assert.Equal(
                    new[] { "first" },
                    authority.CheckpointMutationOwners);
                Assert.Empty(
                    firstRegistry.ExportState()
                        .OrchestraDeleteCheckpointAlertOutbox!);
            }
            finally
            {
                await firstWorker.StopAsync(
                    CancellationToken.None);
                await secondWorker.StopAsync(
                    CancellationToken.None);
                firstLease.Dispose();
                secondLease.Dispose();
            }

            var takeoverStateStore = CreateStateStore(statePath);
            var takeoverLease =
                new OrchestraDeleteCheckpointWorkerLease(
                    takeoverStateStore);
            var takeoverRegistry = new RegistryService(
                takeoverStateStore,
                new LeaseCheckpointStore(authority, "takeover"),
                takeoverLease);
            var takeoverSink = new RecordingAlertSink();
            using var takeoverWorker =
                new OrchestraDeleteCheckpointService(
                    takeoverRegistry,
                    takeoverSink,
                    NullLogger<
                        OrchestraDeleteCheckpointService>.Instance,
                    options,
                    takeoverLease);
            try
            {
                await takeoverWorker.StartAsync(
                    CancellationToken.None);
                await WaitUntilAsync(
                    () => takeoverLease.IsHeld &&
                        authority.ReadCount("takeover") > 0,
                    TimeSpan.FromSeconds(2));
                await Task.Delay(TimeSpan.FromMilliseconds(50));

                Assert.Equal(
                    1,
                    authority.CheckpointMutationCount);
                Assert.Empty(takeoverSink.EventIds);
            }
            finally
            {
                await takeoverWorker.StopAsync(
                    CancellationToken.None);
                takeoverLease.Dispose();
            }
        }
        finally
        {
            DeleteFiles(statePath);
        }
    }

    [Fact]
    public void WorkerLeaseRejectsSymbolicLinkAndReleasesOnDispose()
    {
        var statePath = TemporaryPath("json");
        var stateStore = CreateStateStore(statePath);
        var first =
            new OrchestraDeleteCheckpointWorkerLease(stateStore);
        var second =
            new OrchestraDeleteCheckpointWorkerLease(stateStore);
        try
        {
            Assert.True(first.TryAcquire());
            Assert.False(second.TryAcquire());
            first.Dispose();
            Assert.True(second.TryAcquire());
            second.Dispose();

            if (!OperatingSystem.IsWindows())
            {
                File.Delete(first.LeasePath);
                var target = $"{first.LeasePath}.target";
                File.WriteAllText(target, string.Empty);
                File.CreateSymbolicLink(first.LeasePath, target);
                var forged =
                    new OrchestraDeleteCheckpointWorkerLease(
                        stateStore);
                Assert.Throws<InvalidDataException>(() =>
                {
                    _ = forged.TryAcquire();
                });
                File.Delete(target);
            }
        }
        finally
        {
            first.Dispose();
            second.Dispose();
            DeleteFiles(statePath);
        }
    }

    [Fact]
    public void WorkerLeaseLosesOwnershipWhenOwnerRecordIsReplaced()
    {
        var statePath = TemporaryPath("json");
        var stateStore = CreateStateStore(statePath);
        using var first =
            new OrchestraDeleteCheckpointWorkerLease(stateStore);
        using var second =
            new OrchestraDeleteCheckpointWorkerLease(stateStore);
        try
        {
            Assert.True(first.TryAcquire());
            File.Delete(first.LeasePath);

            Assert.False(first.IsHeld);
            Assert.True(second.TryAcquire());

            first.Dispose();
            Assert.True(second.IsHeld);

            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(
                    second.LeasePath,
                    UnixFileMode.UserRead |
                    UnixFileMode.UserWrite |
                    UnixFileMode.GroupRead);
                Assert.False(second.IsHeld);
                second.Dispose();
                File.Delete(second.LeasePath);
            }
        }
        finally
        {
            DeleteFiles(statePath);
        }
    }

    [Fact]
    public async Task WorkerLeaseExcludesARealSecondProcessAndRecoversAfterExit()
    {
        var statePath = TemporaryPath("json");
        using var lease =
            new OrchestraDeleteCheckpointWorkerLease(
                CreateStateStore(statePath));
        using var harness = StartLeaseHarness(statePath);
        try
        {
            using var timeout =
                new CancellationTokenSource(
                    TimeSpan.FromSeconds(10));
            var ready = await harness.StandardOutput
                .ReadLineAsync(timeout.Token);
            if (!string.Equals(
                    ready,
                    "checkpoint-worker-lease-held",
                    StringComparison.Ordinal))
            {
                var error = await harness.StandardError
                    .ReadToEndAsync(timeout.Token);
                throw new InvalidOperationException(
                    $"lease harness did not become ready: {ready} {error}");
            }

            Assert.False(lease.TryAcquire());
            await harness.StandardInput.WriteLineAsync(
                "release");
            harness.StandardInput.Close();
            await harness.WaitForExitAsync(timeout.Token);
            Assert.Equal(0, harness.ExitCode);
            Assert.True(lease.TryAcquire());
            lease.Dispose();

            using var crashedHarness =
                StartLeaseHarness(statePath);
            var crashReady = await crashedHarness.StandardOutput
                .ReadLineAsync(timeout.Token);
            Assert.Equal(
                "checkpoint-worker-lease-held",
                crashReady);
            crashedHarness.Kill(entireProcessTree: true);
            await crashedHarness.WaitForExitAsync(
                timeout.Token);
            Assert.True(lease.TryAcquire());
        }
        finally
        {
            if (!harness.HasExited)
            {
                harness.Kill(entireProcessTree: true);
                await harness.WaitForExitAsync();
            }
            DeleteFiles(statePath);
        }
    }

    [Fact]
    public async Task AuthenticatedSinkUsesPrivateTokenAndStableIdempotency()
    {
        var tokenPath = TemporaryPath("token");
        const string token =
            "checkpoint-alert-token-0123456789abcdef";
        try
        {
            File.WriteAllText(tokenPath, token + "\n");
            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(
                    tokenPath,
                    UnixFileMode.UserRead |
                    UnixFileMode.UserWrite);
            }
            var handler = new RecordingHttpHandler();
            var client = new HttpClient(handler)
            {
                Timeout = TimeSpan.FromSeconds(1),
            };
            var sink = OrchestraDeleteCheckpointAlertSinkFactory
                .Create(
                    Configuration(
                        ("LESERPENT_CHECKPOINT_ALERT_ENDPOINT",
                            "https://alerts.example.test/v1/checkpoint"),
                        ("LESERPENT_CHECKPOINT_ALERT_TOKEN_FILE",
                            tokenPath)),
                    new SingleHttpClientFactory(client),
                    new LoggingOrchestraDeleteCheckpointAlertSink(
                        NullLogger<
                            LoggingOrchestraDeleteCheckpointAlertSink>
                            .Instance));
            var delivery = Delivery();

            await sink.DeliverAsync(
                delivery,
                CancellationToken.None);

            Assert.Equal(
                HttpMethod.Post,
                handler.Method);
            Assert.Equal(
                "https://alerts.example.test/v1/checkpoint",
                handler.Uri?.AbsoluteUri);
            Assert.Equal("Bearer", handler.AuthorizationScheme);
            Assert.Equal(token, handler.AuthorizationParameter);
            Assert.Equal(delivery.EventId, handler.IdempotencyKey);
            Assert.Equal(
                delivery.AlertGeneration.ToString(),
                handler.AlertGeneration);
            using var document = JsonDocument.Parse(
                Assert.IsType<string>(handler.Body));
            Assert.Equal(
                1,
                document.RootElement
                    .GetProperty("version")
                    .GetInt32());
            Assert.Equal(
                delivery.EventId,
                document.RootElement
                    .GetProperty("eventId")
                    .GetString());
            Assert.Equal(
                "orchestra_delete_checkpoint_unavailable",
                document.RootElement
                    .GetProperty("kind")
                    .GetString());
        }
        finally
        {
            File.Delete(tokenPath);
        }
    }

    [Fact]
    public void AuthenticatedSinkRejectsInlinePartialAndUnsafeSecrets()
    {
        var logging =
            new LoggingOrchestraDeleteCheckpointAlertSink(
                NullLogger<
                    LoggingOrchestraDeleteCheckpointAlertSink>
                    .Instance);
        var clients = new SingleHttpClientFactory(
            new HttpClient(new RecordingHttpHandler()));

        Assert.Throws<InvalidOperationException>(() =>
            OrchestraDeleteCheckpointAlertSinkFactory.Create(
                Configuration(
                    ("LESERPENT_CHECKPOINT_ALERT_TOKEN",
                        "inline-secret-that-must-not-be-used")),
                clients,
                logging));
        Assert.Throws<InvalidOperationException>(() =>
            OrchestraDeleteCheckpointAlertSinkFactory.Create(
                Configuration(
                    ("LESERPENT_CHECKPOINT_ALERT_ENDPOINT",
                        "https://alerts.example.test/v1/checkpoint")),
                clients,
                logging));
        Assert.Throws<InvalidOperationException>(() =>
            OrchestraDeleteCheckpointAlertSinkFactory.Create(
                Configuration(
                    ("LESERPENT_CHECKPOINT_ALERT_ENDPOINT",
                        "http://alerts.example.test/v1/checkpoint"),
                    ("LESERPENT_CHECKPOINT_ALERT_TOKEN_FILE",
                        "/tmp/not-read")),
                clients,
                logging));

        if (!OperatingSystem.IsWindows())
        {
            var tokenPath = TemporaryPath("token");
            var linkPath = TemporaryPath("token-link");
            try
            {
                File.WriteAllText(
                    tokenPath,
                    "checkpoint-alert-token-0123456789abcdef");
                File.SetUnixFileMode(
                    tokenPath,
                    UnixFileMode.UserRead |
                    UnixFileMode.UserWrite |
                    UnixFileMode.GroupRead);
                Assert.Throws<InvalidDataException>(() =>
                    OrchestraDeleteCheckpointAlertSinkFactory.Create(
                        Configuration(
                            ("LESERPENT_CHECKPOINT_ALERT_ENDPOINT",
                                "https://alerts.example.test/v1/checkpoint"),
                            ("LESERPENT_CHECKPOINT_ALERT_TOKEN_FILE",
                                tokenPath)),
                        clients,
                        logging));

                File.SetUnixFileMode(
                    tokenPath,
                    UnixFileMode.UserRead |
                    UnixFileMode.UserWrite);
                File.CreateSymbolicLink(
                    linkPath,
                    tokenPath);
                Assert.Throws<InvalidDataException>(() =>
                    OrchestraDeleteCheckpointAlertSinkFactory.Create(
                        Configuration(
                            ("LESERPENT_CHECKPOINT_ALERT_ENDPOINT",
                                "https://alerts.example.test/v1/checkpoint"),
                            ("LESERPENT_CHECKPOINT_ALERT_TOKEN_FILE",
                                linkPath)),
                        clients,
                        logging));
            }
            finally
            {
                File.Delete(linkPath);
                File.Delete(tokenPath);
            }
        }
    }

    private static void SeedCheckpointAlertState(
        string statePath,
        DateTimeOffset now,
        OrchestraDeleteReplayHorizon horizon)
    {
        var raisedAt = now.AddSeconds(-3);
        CreateStateStore(statePath).SaveStrict(
            Array.Empty<PersistedRuntimeState>(),
            Array.Empty<PersistedSessionState>(),
            runtimeDeletionReconciliationAudit: new[]
            {
                new PersistedRuntimeDeletionReconciliationAudit(
                    "lease-reconciliation-1",
                    "lease-intent-1",
                    new[] { "runtime-lease-1" },
                    ExpectedRevision: 1,
                    DaemonRevision: 1,
                    RequestedBy: "lease-proof",
                    ReconciledAt: raisedAt,
                    OrchestraCleanupCommandId:
                        "lease-cleanup-command-1",
                    OrchestraCleanupGeneration: 1),
            },
            orchestraDeleteCheckpointMonitor:
                new PersistedOrchestraDeleteCheckpointMonitor(
                    horizon,
                    OrchestraDeleteReplayAdmissionPressure.Healthy,
                    ConsecutiveFailureCount: 1,
                    LastAttemptAt: now.AddSeconds(-2),
                    NextRetryAt: now.AddSeconds(-1),
                    LastSucceededAt: now.AddSeconds(-4),
                    LastFailureCode:
                        "orchestra_checkpoint_unavailable",
                    AlertGeneration: 1,
                    AlertRaisedAt: raisedAt,
                    AcknowledgedAlertGeneration: null,
                    AcknowledgedBy: null,
                    AcknowledgedAt: null),
            orchestraDeleteCheckpointAlertOutbox: new[]
            {
                new PersistedOrchestraDeleteCheckpointAlertDelivery(
                    "orchestra-checkpoint-alert-1",
                    AlertGeneration: 1,
                    RaisedAt: raisedAt,
                    OrchestraDeleteReplayAdmissionPressure.Healthy,
                    FailureCount: 1,
                    FailureCode:
                        "orchestra_checkpoint_unavailable",
                    EnqueuedAt: raisedAt,
                    AttemptCount: 0,
                    LastAttemptAt: null,
                    NextAttemptAt: null,
                    LastDeliveryFailureCode: null),
            });
    }

    private static PersistedOrchestraDeleteCheckpointAlertDelivery
        Delivery() =>
        new(
            "orchestra-checkpoint-alert-7",
            AlertGeneration: 7,
            RaisedAt: DateTimeOffset.Parse(
                "2026-07-27T00:00:00Z"),
            OrchestraDeleteReplayAdmissionPressure.Critical,
            FailureCount: 1,
            FailureCode: "orchestra_checkpoint_unavailable",
            EnqueuedAt: DateTimeOffset.Parse(
                "2026-07-27T00:00:00Z"),
            AttemptCount: 1,
            LastAttemptAt: DateTimeOffset.Parse(
                "2026-07-27T00:00:01Z"),
            NextAttemptAt: DateTimeOffset.Parse(
                "2026-07-27T00:00:02Z"),
            LastDeliveryFailureCode: null);

    private static IConfiguration Configuration(
        params (string Key, string? Value)[] values) =>
        new ConfigurationBuilder()
            .AddInMemoryCollection(
                values.ToDictionary(
                    static item => item.Key,
                    static item => item.Value,
                    StringComparer.Ordinal))
            .Build();

    private static ControlPlaneStateStore CreateStateStore(
        string statePath) =>
        new(
            Configuration(
                ("LESERPENT_STATE_PATH", statePath)),
            new TestHostEnvironment
            {
                ContentRootPath =
                    Path.GetDirectoryName(statePath)!,
            },
            NullLogger<ControlPlaneStateStore>.Instance);

    private static Process StartLeaseHarness(string statePath)
    {
        var start = new ProcessStartInfo
        {
            FileName =
                Environment.GetEnvironmentVariable(
                    "DOTNET_HOST_PATH") ?? "dotnet",
            UseShellExecute = false,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };
        start.ArgumentList.Add(FindCrashHarnessAssembly());
        start.ArgumentList.Add(
            "checkpoint-worker-lease-hold");
        start.ArgumentList.Add(statePath);
        return Process.Start(start) ??
            throw new InvalidOperationException(
                "failed to start checkpoint lease harness");
    }

    private static string FindCrashHarnessAssembly()
    {
        var repositoryRoot = FindRepositoryRoot();
        var configuration = new DirectoryInfo(
            Path.TrimEndingDirectorySeparator(
                AppContext.BaseDirectory))
            .Parent?.Name ?? "Debug";
        return Path.Combine(
            repositoryRoot,
            "apps",
            "leserpent",
            "tests",
            "Leserpent.RuntimeDeletionCrashHarness",
            "bin",
            configuration,
            "net10.0",
            "Leserpent.RuntimeDeletionCrashHarness.dll");
    }

    private static string FindRepositoryRoot()
    {
        for (var directory =
                new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            if (File.Exists(
                    Path.Combine(
                        directory.FullName,
                        "Cargo.toml")))
            {
                return directory.FullName;
            }
        }
        throw new DirectoryNotFoundException(
            "could not locate the gewyvern repository root");
    }

    private static string TemporaryPath(string extension) =>
        Path.Combine(
            Path.GetTempPath(),
            $"leserpent-checkpoint-worker-{Guid.NewGuid():N}.{extension}");

    private static void DeleteFiles(string statePath)
    {
        File.Delete(statePath);
        File.Delete($"{statePath}.bak");
        File.Delete(
            $"{Path.GetFullPath(statePath)}.checkpoint-worker.lease");
        File.Delete(
            $"{Path.GetFullPath(statePath)}.checkpoint-worker.lease.target");
    }

    private static async Task WaitUntilAsync(
        Func<bool> predicate,
        TimeSpan timeout)
    {
        var deadline = DateTimeOffset.UtcNow + timeout;
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (predicate())
            {
                return;
            }
            await Task.Delay(TimeSpan.FromMilliseconds(10));
        }
        Assert.True(predicate(), "condition did not converge");
    }

    private sealed class RecordingAlertSink :
        IOrchestraDeleteCheckpointAlertSink
    {
        private readonly object sync = new();
        private readonly List<string> eventIds = new();

        public IReadOnlyList<string> EventIds
        {
            get
            {
                lock (sync)
                {
                    return eventIds.ToArray();
                }
            }
        }

        public Task DeliverAsync(
            PersistedOrchestraDeleteCheckpointAlertDelivery delivery,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            lock (sync)
            {
                eventIds.Add(delivery.EventId);
            }
            return Task.CompletedTask;
        }
    }

    private sealed class SharedCheckpointAuthority(
        OrchestraDeleteReplayHorizon horizon)
    {
        private readonly object sync = new();
        private readonly Dictionary<string, int> reads =
            new(StringComparer.Ordinal);
        private readonly List<string> checkpointMutationOwners =
            new();

        public int CheckpointMutationCount
        {
            get
            {
                lock (sync)
                {
                    return checkpointMutationOwners.Count;
                }
            }
        }

        public IReadOnlyList<string> CheckpointMutationOwners
        {
            get
            {
                lock (sync)
                {
                    return checkpointMutationOwners.ToArray();
                }
            }
        }

        public int ReadCount(string owner)
        {
            lock (sync)
            {
                return reads.GetValueOrDefault(owner);
            }
        }

        public OrchestraDeleteReplayHorizon Read(string owner)
        {
            lock (sync)
            {
                reads[owner] = reads.GetValueOrDefault(owner) + 1;
                return horizon;
            }
        }

        public OrchestraDeleteReplayHorizon Checkpoint(
            string owner,
            OrchestraDeleteReplayCheckpoint checkpoint)
        {
            lock (sync)
            {
                checkpointMutationOwners.Add(owner);
                horizon = horizon with
                {
                    ProtectedFromGeneration =
                        checkpoint.MinimumRetainedGeneration,
                    CheckpointedThroughGeneration =
                        checkpoint.ObservedThroughGeneration,
                };
                return horizon;
            }
        }
    }

    private sealed class LeaseCheckpointStore(
        SharedCheckpointAuthority authority,
        string owner) : IOrchestraRunStore
    {
        public string Provider => "lease-proof";
        public string Location => "memory";
        public int SchemaVersion => 18;
        public bool SupportsDeleteReplayHorizon => true;
        public string? LastError => null;
        public IReadOnlyList<OrchestraRunSummary> LoadAll() =>
            Array.Empty<OrchestraRunSummary>();
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(
            string runtimeId,
            string runId) =>
            Array.Empty<OrchestraRunEvent>();
        public bool Upsert(
            OrchestraRunSummary run,
            OrchestraRunEvent? eventRecord = null) =>
            true;
        public bool ReplaceAll(
            IReadOnlyList<OrchestraRunSummary> runs) =>
            true;
        public bool DeleteRuntimes(
            IReadOnlyCollection<string> runtimeIds) =>
            true;
        public OrchestraDeleteReplayHorizon?
            GetDeleteReplayHorizon() =>
            authority.Read(owner);
        public OrchestraDeleteReplayHorizon?
            CheckpointDeleteReplayHorizon(
                OrchestraDeleteReplayCheckpoint checkpoint) =>
            authority.Checkpoint(owner, checkpoint);
    }

    private sealed class RecordingHttpHandler :
        HttpMessageHandler
    {
        public HttpMethod? Method { get; private set; }
        public Uri? Uri { get; private set; }
        public string? AuthorizationScheme { get; private set; }
        public string? AuthorizationParameter { get; private set; }
        public string? IdempotencyKey { get; private set; }
        public string? AlertGeneration { get; private set; }
        public string? Body { get; private set; }

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            Method = request.Method;
            Uri = request.RequestUri;
            AuthorizationScheme =
                request.Headers.Authorization?.Scheme;
            AuthorizationParameter =
                request.Headers.Authorization?.Parameter;
            IdempotencyKey = request.Headers
                .GetValues("Idempotency-Key")
                .Single();
            AlertGeneration = request.Headers
                .GetValues("X-Leserpent-Alert-Generation")
                .Single();
            Body = request.Content is null
                ? null
                : await request.Content.ReadAsStringAsync(
                    cancellationToken);
            return new HttpResponseMessage(HttpStatusCode.NoContent);
        }
    }

    private sealed class SingleHttpClientFactory(
        HttpClient client) : IHttpClientFactory
    {
        public HttpClient CreateClient(string name) => client;
    }

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } =
            Environments.Development;
        public string ApplicationName { get; set; } =
            "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = string.Empty;
        public IFileProvider ContentRootFileProvider { get; set; } =
            new NullFileProvider();
    }
}
