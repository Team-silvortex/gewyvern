using System.Net;
using System.Diagnostics;
using System.Net.Http.Json;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Text.Json;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
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
            var firstHealth =
                new OrchestraDeleteCheckpointWorkerHealth(
                    firstLease,
                    firstSink);
            var secondHealth =
                new OrchestraDeleteCheckpointWorkerHealth(
                    secondLease,
                    secondSink);
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
                    firstLease,
                    firstHealth);
            using var secondWorker =
                new OrchestraDeleteCheckpointService(
                    secondRegistry,
                    secondSink,
                    NullLogger<
                        OrchestraDeleteCheckpointService>.Instance,
                    options,
                    secondLease,
                    secondHealth);
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
                var firstSnapshot = firstHealth.Snapshot();
                Assert.Equal("owner", firstSnapshot.WorkerState);
                Assert.True(firstSnapshot.LeaseHeld);
                Assert.NotNull(
                    firstSnapshot.LastAlertDeliverySucceededAt);
                Assert.Equal(
                    0U,
                    firstSnapshot.ConsecutiveAlertDeliveryFailures);
                var secondSnapshot = secondHealth.Snapshot();
                Assert.Equal("standby", secondSnapshot.WorkerState);
                Assert.False(secondSnapshot.LeaseHeld);
                Assert.Null(
                    secondSnapshot.LastAlertDeliveryAttemptAt);
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
            var takeoverHealth =
                new OrchestraDeleteCheckpointWorkerHealth(
                    takeoverLease,
                    takeoverSink);
            using var takeoverWorker =
                new OrchestraDeleteCheckpointService(
                    takeoverRegistry,
                    takeoverSink,
                    NullLogger<
                        OrchestraDeleteCheckpointService>.Instance,
                    options,
                    takeoverLease,
                    takeoverHealth);
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
                Assert.Equal(
                    "owner",
                    takeoverHealth.Snapshot().WorkerState);
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
    public async Task RealDuplicateHostsExposeOneOwnerAndFreshProcessTakeover()
    {
        const string adminToken =
            "leserpent-duplicate-host-admin-token-012345";
        var statePath = TemporaryPath("json");
        var databasePath = TemporaryPath("db");
        var ports = ReserveTcpPorts(3);
        var firstUrl = LoopbackUrl(ports[0]);
        var secondUrl = LoopbackUrl(ports[1]);
        var takeoverUrl = LoopbackUrl(ports[2]);
        using var first = StartControlPlaneHost(
            firstUrl,
            statePath,
            databasePath,
            adminToken);
        Process? second = null;
        Process? takeover = null;
        try
        {
            var firstHealth = await WaitForWorkerHealthAsync(
                first,
                firstUrl,
                adminToken,
                TimeSpan.FromSeconds(15));
            Assert.Equal("owner", firstHealth.WorkerState);
            Assert.True(firstHealth.LeaseHeld);
            var firstWriter = await WaitForWriterHealthAsync(
                first,
                firstUrl,
                adminToken,
                TimeSpan.FromSeconds(5));
            Assert.Equal("owner", firstWriter.State);
            Assert.True(firstWriter.LeaseHeld);
            Assert.Equal(
                HttpStatusCode.OK,
                await SaveControlPlaneAsync(
                    firstUrl,
                    adminToken));
            second = StartControlPlaneHost(
                secondUrl,
                statePath,
                databasePath,
                adminToken);
            var secondHealth = await WaitForWorkerHealthAsync(
                second,
                secondUrl,
                adminToken,
                TimeSpan.FromSeconds(15));
            Assert.Equal("standby", secondHealth.WorkerState);
            Assert.False(secondHealth.LeaseHeld);
            var secondWriter = await WaitForWriterHealthAsync(
                second,
                secondUrl,
                adminToken,
                TimeSpan.FromSeconds(5));
            Assert.Equal("standby", secondWriter.State);
            Assert.False(secondWriter.LeaseHeld);
            Assert.Equal(
                HttpStatusCode.Conflict,
                await SaveControlPlaneAsync(
                    secondUrl,
                    adminToken));

            first.Kill(entireProcessTree: true);
            await first.WaitForExitAsync();
            var retainedStandby = await WaitForWorkerHealthAsync(
                second,
                secondUrl,
                adminToken,
                TimeSpan.FromSeconds(5));
            Assert.Equal("standby", retainedStandby.WorkerState);
            Assert.False(retainedStandby.LeaseHeld);
            var retainedStandbyWriter =
                await WaitForWriterHealthAsync(
                    second,
                    secondUrl,
                    adminToken,
                    TimeSpan.FromSeconds(5));
            Assert.Equal("standby", retainedStandbyWriter.State);
            Assert.False(retainedStandbyWriter.LeaseHeld);
            Assert.Equal(
                HttpStatusCode.Conflict,
                await SaveControlPlaneAsync(
                    secondUrl,
                    adminToken));

            takeover = StartControlPlaneHost(
                takeoverUrl,
                statePath,
                databasePath,
                adminToken);
            var takeoverHealth = await WaitForWorkerHealthAsync(
                takeover,
                takeoverUrl,
                adminToken,
                TimeSpan.FromSeconds(15));
            Assert.Equal("owner", takeoverHealth.WorkerState);
            Assert.True(takeoverHealth.LeaseHeld);
            var takeoverWriter = await WaitForWriterHealthAsync(
                takeover,
                takeoverUrl,
                adminToken,
                TimeSpan.FromSeconds(5));
            Assert.Equal("owner", takeoverWriter.State);
            Assert.True(takeoverWriter.LeaseHeld);
            Assert.Equal(
                HttpStatusCode.OK,
                await SaveControlPlaneAsync(
                    takeoverUrl,
                    adminToken));

            WriteDuplicateHostEvidence(
                firstHealth,
                secondHealth,
                retainedStandby,
                takeoverHealth,
                firstWriter,
                secondWriter,
                retainedStandbyWriter,
                takeoverWriter);
        }
        finally
        {
            await StopProcessAsync(first);
            if (second is not null)
            {
                await StopProcessAsync(second);
                second.Dispose();
            }
            if (takeover is not null)
            {
                await StopProcessAsync(takeover);
                takeover.Dispose();
            }
            DeleteFiles(statePath);
            File.Delete(databasePath);
            File.Delete($"{databasePath}-wal");
            File.Delete($"{databasePath}-shm");
        }
    }

    [Fact]
    public async Task WorkerHealthReportsSanitizedDeliveryFailure()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var horizon = new OrchestraDeleteReplayHorizon(
                Capacity: 4096,
                Retained: 1,
                OldestGeneration: 1,
                NewestGeneration: 1,
                NextGeneration: 2,
                EvictedThroughGeneration: 0,
                ProtectedFromGeneration: 1,
                CheckpointedThroughGeneration: null);
            SeedCheckpointAlertState(
                statePath,
                DateTimeOffset.UtcNow,
                horizon);
            var stateStore = CreateStateStore(statePath);
            using var lease =
                new OrchestraDeleteCheckpointWorkerLease(
                    stateStore);
            var sink = new FailingAlertSink(
                "secret endpoint and bearer token must not escape");
            var health =
                new OrchestraDeleteCheckpointWorkerHealth(
                    lease,
                    sink);
            var registry = new RegistryService(
                stateStore,
                new LeaseCheckpointStore(
                    new SharedCheckpointAuthority(horizon),
                    "failure"),
                lease);
            using var worker =
                new OrchestraDeleteCheckpointService(
                    registry,
                    sink,
                    NullLogger<
                        OrchestraDeleteCheckpointService>.Instance,
                    new OrchestraDeleteCheckpointWorkerOptions(
                        TimeSpan.FromMilliseconds(20),
                        TimeSpan.FromMilliseconds(5)),
                    lease,
                    health);

            await worker.StartAsync(CancellationToken.None);
            await WaitUntilAsync(
                () => health.Snapshot()
                    .ConsecutiveAlertDeliveryFailures > 0,
                TimeSpan.FromSeconds(2));
            var snapshot = health.Snapshot();

            Assert.Equal("owner", snapshot.WorkerState);
            Assert.Equal("custom", snapshot.AlertSinkMode);
            Assert.True(snapshot.ExternalAlertSinkConfigured);
            Assert.Equal(
                "sink_delivery_failed",
                snapshot.LastAlertDeliveryFailureCode);
            Assert.NotNull(snapshot.LastAlertDeliveryAttemptAt);
            Assert.Null(snapshot.LastAlertDeliverySucceededAt);
            Assert.DoesNotContain(
                "secret",
                JsonSerializer.Serialize(snapshot),
                StringComparison.OrdinalIgnoreCase);

            await worker.StopAsync(CancellationToken.None);
        }
        finally
        {
            DeleteFiles(statePath);
        }
    }

    [Fact]
    public async Task WorkerHealthEndpointRequiresRemoteAdminToken()
    {
        const string adminToken =
            "leserpent-worker-health-admin-token-012345";
        var statePath = TemporaryPath("json");
        var stateStore = CreateStateStore(statePath);
        using var lease =
            new OrchestraDeleteCheckpointWorkerLease(stateStore);
        try
        {
            Assert.True(lease.TryAcquire());
            var sink =
                new LoggingOrchestraDeleteCheckpointAlertSink(
                    NullLogger<
                        LoggingOrchestraDeleteCheckpointAlertSink>
                        .Instance);
            var health =
                new OrchestraDeleteCheckpointWorkerHealth(
                    lease,
                    sink);
            health.MarkOwner();
            await using var app =
                await BuildWorkerHealthTestAppAsync(
                    health,
                    adminToken);
            var client = app.GetTestClient();

            var denied = await client.GetAsync(
                "/v1/persistence/orchestra-cleanup-worker-health");
            Assert.Equal(
                HttpStatusCode.Forbidden,
                denied.StatusCode);

            using var request = new HttpRequestMessage(
                HttpMethod.Get,
                "/v1/persistence/orchestra-cleanup-worker-health");
            request.Headers.Add(
                ControlPlaneSecurityPolicy.AdminTokenHeader,
                adminToken);
            var allowed = await client.SendAsync(request);
            Assert.Equal(HttpStatusCode.OK, allowed.StatusCode);
            var snapshot = await allowed.Content.ReadFromJsonAsync(
                LeserpentJsonContext.Default
                    .OrchestraDeleteCheckpointWorkerHealthSnapshot);
            Assert.NotNull(snapshot);
            Assert.Equal(1, snapshot.Version);
            Assert.Equal("owner", snapshot.WorkerState);
            Assert.True(snapshot.LeaseHeld);
            Assert.Equal(
                "structured_logging",
                snapshot.AlertSinkMode);
            Assert.False(snapshot.ExternalAlertSinkConfigured);

            var json = await allowed.Content.ReadAsStringAsync();
            Assert.DoesNotContain(statePath, json, StringComparison.Ordinal);
            Assert.DoesNotContain(
                adminToken,
                json,
                StringComparison.Ordinal);
            Assert.DoesNotContain(
                "endpoint",
                json,
                StringComparison.OrdinalIgnoreCase);
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

    private static Process StartControlPlaneHost(
        string url,
        string statePath,
        string databasePath,
        string adminToken)
    {
        var start = new ProcessStartInfo
        {
            FileName =
                Environment.GetEnvironmentVariable(
                    "DOTNET_HOST_PATH") ?? "dotnet",
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };
        start.ArgumentList.Add(
            typeof(Leserpent.Program).Assembly.Location);
        start.ArgumentList.Add("--urls");
        start.ArgumentList.Add(url);
        start.Environment["ASPNETCORE_ENVIRONMENT"] =
            Environments.Production;
        start.Environment["LESERPENT_STATE_PATH"] = statePath;
        start.Environment["LESERPENT_DATABASE_PATH"] =
            databasePath;
        start.Environment["LESERPENT_ADMIN_TOKEN"] = adminToken;
        return Process.Start(start) ??
            throw new InvalidOperationException(
                "failed to start Leserpent control-plane host");
    }

    private static async Task<
        OrchestraDeleteCheckpointWorkerHealthSnapshot>
        WaitForWorkerHealthAsync(
            Process process,
            string url,
            string adminToken,
            TimeSpan timeout)
    {
        using var client = new HttpClient(
            new HttpClientHandler
            {
                AllowAutoRedirect = false,
            })
        {
            Timeout = TimeSpan.FromSeconds(1),
        };
        var deadline = DateTimeOffset.UtcNow + timeout;
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (process.HasExited)
            {
                var error = await process.StandardError
                    .ReadToEndAsync();
                throw new InvalidOperationException(
                    $"Leserpent host exited with {process.ExitCode}: {error}");
            }
            try
            {
                using var request = new HttpRequestMessage(
                    HttpMethod.Get,
                    $"{url}/v1/persistence/orchestra-cleanup-worker-health");
                request.Headers.Add(
                    ControlPlaneSecurityPolicy.AdminTokenHeader,
                    adminToken);
                using var response = await client.SendAsync(request);
                if (response.StatusCode == HttpStatusCode.OK)
                {
                    var health = await response.Content
                        .ReadFromJsonAsync(
                            LeserpentJsonContext.Default
                                .OrchestraDeleteCheckpointWorkerHealthSnapshot);
                    if (health is not null &&
                        !string.Equals(
                            health.WorkerState,
                            "starting",
                            StringComparison.Ordinal))
                    {
                        return health;
                    }
                }
            }
            catch (HttpRequestException)
            {
            }
            catch (TaskCanceledException)
            {
            }
            await Task.Delay(TimeSpan.FromMilliseconds(50));
        }
        throw new TimeoutException(
            $"Leserpent host at {url} did not expose worker health");
    }

    private static async Task<ControlPlaneWriterHealthSnapshot>
        WaitForWriterHealthAsync(
            Process process,
            string url,
            string adminToken,
            TimeSpan timeout)
    {
        using var client = new HttpClient(
            new HttpClientHandler
            {
                AllowAutoRedirect = false,
            })
        {
            Timeout = TimeSpan.FromSeconds(1),
        };
        var deadline = DateTimeOffset.UtcNow + timeout;
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (process.HasExited)
            {
                var error = await process.StandardError
                    .ReadToEndAsync();
                throw new InvalidOperationException(
                    $"Leserpent host exited with {process.ExitCode}: {error}");
            }
            try
            {
                using var request = new HttpRequestMessage(
                    HttpMethod.Get,
                    $"{url}/v1/persistence/control-writer-health");
                request.Headers.Add(
                    ControlPlaneSecurityPolicy.AdminTokenHeader,
                    adminToken);
                using var response = await client.SendAsync(request);
                if (response.StatusCode == HttpStatusCode.OK)
                {
                    var health = await response.Content
                        .ReadFromJsonAsync(
                            LeserpentJsonContext.Default
                                .ControlPlaneWriterHealthSnapshot);
                    if (health is not null &&
                        !string.Equals(
                            health.State,
                            "starting",
                            StringComparison.Ordinal))
                    {
                        return health;
                    }
                }
            }
            catch (HttpRequestException)
            {
            }
            catch (TaskCanceledException)
            {
            }
            await Task.Delay(TimeSpan.FromMilliseconds(50));
        }
        throw new TimeoutException(
            $"Leserpent host at {url} did not expose writer health");
    }

    private static async Task<HttpStatusCode> SaveControlPlaneAsync(
        string url,
        string adminToken)
    {
        using var client = new HttpClient(
            new HttpClientHandler
            {
                AllowAutoRedirect = false,
            });
        using var request = new HttpRequestMessage(
            HttpMethod.Post,
            $"{url}/v1/persistence/save");
        request.Headers.Add(
            ControlPlaneSecurityPolicy.AdminTokenHeader,
            adminToken);
        request.Headers.Add(
            ControlPlaneSecurityPolicy.IntentHeader,
            ControlPlaneSecurityPolicy.MutateIntent);
        using var response = await client.SendAsync(request);
        if (response.StatusCode == HttpStatusCode.Conflict)
        {
            var error = await response.Content.ReadFromJsonAsync(
                LeserpentJsonContext.Default.ApiErrorResponse);
            Assert.Equal(
                ControlPlaneWriterUnavailableException.ErrorCode,
                error?.Error);
        }
        return response.StatusCode;
    }

    private static async Task StopProcessAsync(Process process)
    {
        if (!process.HasExited)
        {
            process.Kill(entireProcessTree: true);
            await process.WaitForExitAsync();
        }
    }

    private static int[] ReserveTcpPorts(int count)
    {
        var listeners = new List<TcpListener>(count);
        try
        {
            for (var index = 0; index < count; index += 1)
            {
                var listener = new TcpListener(
                    IPAddress.Loopback,
                    0);
                listener.Start();
                listeners.Add(listener);
            }
            return listeners
                .Select(static listener =>
                    ((IPEndPoint)listener.LocalEndpoint).Port)
                .ToArray();
        }
        finally
        {
            foreach (var listener in listeners)
            {
                listener.Stop();
            }
        }
    }

    private static string LoopbackUrl(int port) =>
        $"http://127.0.0.1:{port}";

    private static void WriteDuplicateHostEvidence(
        OrchestraDeleteCheckpointWorkerHealthSnapshot first,
        OrchestraDeleteCheckpointWorkerHealthSnapshot second,
        OrchestraDeleteCheckpointWorkerHealthSnapshot retainedStandby,
        OrchestraDeleteCheckpointWorkerHealthSnapshot takeover,
        ControlPlaneWriterHealthSnapshot firstWriter,
        ControlPlaneWriterHealthSnapshot secondWriter,
        ControlPlaneWriterHealthSnapshot retainedStandbyWriter,
        ControlPlaneWriterHealthSnapshot takeoverWriter)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_CHECKPOINT_WORKER_DUPLICATE_HOST_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }
        if (!Path.IsPathFullyQualified(evidencePath))
        {
            throw new InvalidOperationException(
                "duplicate-host evidence path must be absolute");
        }
        var directory = Path.GetDirectoryName(evidencePath);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }
        var evidence = new
        {
            schemaVersion = 2,
            campaign =
                "leserpent_checkpoint_worker_duplicate_host",
            recordedAt = DateTimeOffset.UtcNow,
            operatingSystem = RuntimeInformation.OSDescription,
            architecture =
                RuntimeInformation.OSArchitecture.ToString()
                    .ToLowerInvariant(),
            firstHost = new
            {
                first.WorkerState,
                first.LeaseHeld,
            },
            secondHost = new
            {
                second.WorkerState,
                second.LeaseHeld,
            },
            standbyAfterOwnerTermination = new
            {
                retainedStandby.WorkerState,
                retainedStandby.LeaseHeld,
            },
            freshProcessTakeover = new
            {
                takeover.WorkerState,
                takeover.LeaseHeld,
            },
            controlPlaneWriter = new
            {
                firstHost = new
                {
                    firstWriter.State,
                    firstWriter.LeaseHeld,
                    saveStatus = 200,
                },
                secondHost = new
                {
                    secondWriter.State,
                    secondWriter.LeaseHeld,
                    saveStatus = 409,
                },
                standbyAfterOwnerTermination = new
                {
                    retainedStandbyWriter.State,
                    retainedStandbyWriter.LeaseHeld,
                    saveStatus = 409,
                },
                freshProcessTakeover = new
                {
                    takeoverWriter.State,
                    takeoverWriter.LeaseHeld,
                    saveStatus = 200,
                },
                fixedStandbyError =
                    ControlPlaneWriterUnavailableException
                        .ErrorCode,
            },
            ownerCountBeforeTermination =
                new[] { first, second }
                    .Count(static health =>
                        health.WorkerState == "owner" &&
                        health.LeaseHeld),
            authenticatedHealthEndpoint = true,
            secretFreeHealthPayload = true,
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions
                {
                    PropertyNamingPolicy =
                        JsonNamingPolicy.CamelCase,
                    WriteIndented = true,
                }));
    }

    private static async Task<WebApplication>
        BuildWorkerHealthTestAppAsync(
            OrchestraDeleteCheckpointWorkerHealth health,
            string adminToken)
    {
        var builder = WebApplication.CreateBuilder();
        builder.WebHost.UseTestServer();
        builder.Configuration.AddInMemoryCollection(
            new Dictionary<string, string?>
            {
                ["LESERPENT_ADMIN_TOKEN"] = adminToken,
            });
        builder.Services.AddSingleton(health);
        builder.Services.AddSingleton<ControlPlaneSecurityPolicy>();
        var app = builder.Build();
        app.Use(async (context, next) =>
        {
            context.Connection.RemoteIpAddress =
                IPAddress.Parse("192.0.2.10");
            context.Request.Scheme = "https";
            await next();
        });
        app.Use(async (context, next) =>
        {
            var security = context.RequestServices
                .GetRequiredService<ControlPlaneSecurityPolicy>();
            if (!security.TryAuthorize(
                    context,
                    out var statusCode,
                    out var payload))
            {
                context.Response.StatusCode = statusCode;
                await context.Response.WriteAsJsonAsync(payload);
                return;
            }
            await next();
        });
        app.MapGet(
            "/v1/persistence/orchestra-cleanup-worker-health",
            (OrchestraDeleteCheckpointWorkerHealth workerHealth) =>
                Results.Json(
                    workerHealth.Snapshot(),
                    LeserpentJsonContext.Default
                        .OrchestraDeleteCheckpointWorkerHealthSnapshot));
        await app.StartAsync();
        return app;
    }

    private static string FindCrashHarnessAssembly() =>
        CrashHarnessAssemblyLocator.Find();

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
        File.Delete(
            $"{Path.GetFullPath(statePath)}.control-writer.lease");
        File.Delete(
            $"{Path.GetFullPath(statePath)}.control-writer.lease.target");
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

    private sealed class FailingAlertSink(
        string failureMessage) :
        IOrchestraDeleteCheckpointAlertSink
    {
        public Task DeliverAsync(
            PersistedOrchestraDeleteCheckpointAlertDelivery delivery,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            throw new HttpRequestException(failureMessage);
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
