using System.Net;
using System.Net.Http.Json;
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

public sealed class RuntimeDeletionReconciliationEndpointTests
{
    [Fact]
    public async Task CleanupReplayStatusExposesCheckpointLagAndRecoveryThresholds()
    {
        var statePath = TemporaryStatePath();
        var databasePath = statePath + ".db";
        try
        {
            var orchestraStore = CreateSqliteStore(databasePath);
            var registry = CreateRegistry(statePath, orchestraStore);
            var receipt = orchestraStore.DeleteRuntimes(
                new OrchestraDeleteCommand(
                    "orchestra-cleanup-status",
                    new[] { "runtime-cleanup-status" }));
            Assert.NotNull(receipt);

            await using var app = await BuildTestAppAsync(
                registry,
                FakeDaemonProjectionReader.Disabled);
            var response = await app.GetTestClient().GetAsync(
                "/v1/persistence/orchestra-cleanup-replay-status");
            Assert.Equal(HttpStatusCode.OK, response.StatusCode);
            var json = await response.Content.ReadAsStringAsync();
            using var document = JsonDocument.Parse(json);
            Assert.Equal(
                512UL,
                document.RootElement
                    .GetProperty("warningAvailableCapacity")
                    .GetUInt64());
            Assert.Equal(
                128UL,
                document.RootElement
                    .GetProperty("criticalAvailableCapacity")
                    .GetUInt64());
            Assert.Equal(
                768UL,
                document.RootElement
                    .GetProperty("warningRecoveryAvailableCapacity")
                    .GetUInt64());
            Assert.Equal(
                256UL,
                document.RootElement
                    .GetProperty("criticalRecoveryAvailableCapacity")
                    .GetUInt64());
            var status = await response.Content.ReadFromJsonAsync(
                LeserpentJsonContext.Default
                    .OrchestraDeleteReplayCheckpointStatus);
            Assert.NotNull(status);
            Assert.Equal(1UL, status.Horizon.CheckpointLagGenerations);
            Assert.Null(status.Horizon.CheckpointedThroughGeneration);
            Assert.Null(status.MinimumAuditedGeneration);
            Assert.Null(status.ObservedThroughAuditedGeneration);
            Assert.False(status.LastAutomaticCheckpointAdvanced);
            Assert.Null(status.LastAutomaticCheckpointAt);
        }
        finally
        {
            DeleteStateFiles(statePath);
            File.Delete(databasePath);
            File.Delete(databasePath + "-wal");
            File.Delete(databasePath + "-shm");
        }
    }

    [Fact]
    public async Task ReconciliationIsRevisionBoundDurableAndReplayable()
    {
        var statePath = TemporaryStatePath();
        try
        {
            var registry = CreateRegistry(
                statePath,
                new InMemoryOrchestraRunStore());
            var intent = CreateReplayAmbiguousIntent(
                registry,
                "runtime-reconcile-success");
            var daemon = new FakeDaemonProjectionReader(
                new DaemonRuntimeProjectionSnapshot(
                    17,
                    Array.Empty<DaemonRuntimeProjection>()));
            var request = new RuntimeDeletionReconcileRequest(
                intent.Revision,
                17,
                "reconcile-success-request",
                "operator-a",
                true);

            await using (var app = await BuildTestAppAsync(
                registry,
                daemon))
            {
                var client = app.GetTestClient();
                var planResponse = await client.GetAsync(
                    ReconciliationPlanPath(intent.IntentId));
                Assert.Equal(
                    HttpStatusCode.OK,
                    planResponse.StatusCode);
                var plan = await planResponse.Content.ReadFromJsonAsync(
                    LeserpentJsonContext.Default
                        .RuntimeDeletionReconciliationPlan);
                Assert.NotNull(plan);
                Assert.True(plan.CanReconcile);
                Assert.Equal(intent.Revision, plan.IntentRevision);
                Assert.Equal(17UL, plan.DaemonRevision);
                Assert.Empty(plan.ReappearedRuntimeIds);

                var accepted = await SendReconcileAsync(
                    client,
                    intent.IntentId,
                    request);
                Assert.Equal(HttpStatusCode.OK, accepted.StatusCode);
                var response = await accepted.Content.ReadFromJsonAsync(
                    LeserpentJsonContext.Default
                        .RuntimeDeletionReconcileResponse);
                Assert.NotNull(response);
                Assert.True(response.Accepted);
                Assert.False(response.Replayed);
                Assert.Equal(
                    request.RequestId,
                    response.Audit.RequestId);
                Assert.StartsWith(
                    "orchestra-cleanup-",
                    response.Audit.OrchestraCleanupCommandId,
                    StringComparison.Ordinal);
                Assert.Equal(
                    1UL,
                    response.Audit.OrchestraCleanupGeneration);
            }

            Assert.Null(registry.GetRuntime(
                "runtime-reconcile-success"));
            Assert.Empty(registry.ListPendingRuntimeDeletions());
            Assert.Single(
                registry.ListRuntimeDeletionReconciliationAudit());

            var restarted = CreateRegistry(
                statePath,
                new InMemoryOrchestraRunStore());
            Assert.Null(restarted.GetRuntime(
                "runtime-reconcile-success"));
            var restoredAudit = Assert.Single(
                restarted
                    .ListRuntimeDeletionReconciliationAudit());
            Assert.Equal(request.RequestId, restoredAudit.RequestId);
            Assert.StartsWith(
                "orchestra-cleanup-",
                restoredAudit.OrchestraCleanupCommandId,
                StringComparison.Ordinal);
            Assert.Equal(
                1UL,
                restoredAudit.OrchestraCleanupGeneration);

            await using var replayApp = await BuildTestAppAsync(
                restarted,
                FakeDaemonProjectionReader.Disabled);
            var replay = await SendReconcileAsync(
                replayApp.GetTestClient(),
                intent.IntentId,
                request);
            Assert.Equal(HttpStatusCode.OK, replay.StatusCode);
            var replayed = await replay.Content.ReadFromJsonAsync(
                LeserpentJsonContext.Default
                    .RuntimeDeletionReconcileResponse);
            Assert.NotNull(replayed);
            Assert.True(replayed.Replayed);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task ReappearedRuntimeIdentityBlocksReconciliation()
    {
        var statePath = TemporaryStatePath();
        try
        {
            var registry = CreateRegistry(
                statePath,
                new InMemoryOrchestraRunStore());
            const string runtimeId = "runtime-reconcile-reappeared";
            var intent = CreateReplayAmbiguousIntent(
                registry,
                runtimeId);
            var daemon = new FakeDaemonProjectionReader(
                new DaemonRuntimeProjectionSnapshot(
                    23,
                    new[] { Projection(runtimeId, 23) }));

            await using var app = await BuildTestAppAsync(
                registry,
                daemon);
            var client = app.GetTestClient();
            var planResponse = await client.GetAsync(
                ReconciliationPlanPath(intent.IntentId));
            var plan = await planResponse.Content.ReadFromJsonAsync(
                LeserpentJsonContext.Default
                    .RuntimeDeletionReconciliationPlan);
            Assert.NotNull(plan);
            Assert.False(plan.CanReconcile);
            Assert.Equal(
                new[] { runtimeId },
                plan.ReappearedRuntimeIds);

            var rejected = await SendReconcileAsync(
                client,
                intent.IntentId,
                new RuntimeDeletionReconcileRequest(
                    intent.Revision,
                    23,
                    "reconcile-reappeared-request",
                    "operator-a",
                    true));
            Assert.Equal(
                HttpStatusCode.Conflict,
                rejected.StatusCode);
            var error = await rejected.Content.ReadFromJsonAsync(
                LeserpentJsonContext.Default.ApiErrorResponse);
            Assert.Equal(
                "runtime_deletion_reconciliation_target_reappeared",
                error!.Error);
            Assert.NotNull(registry.GetRuntime(runtimeId));
            Assert.Single(registry.ListPendingRuntimeDeletions());
            Assert.Empty(
                registry.ListRuntimeDeletionReconciliationAudit());
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task DaemonRevisionDriftBlocksReconciliation()
    {
        var statePath = TemporaryStatePath();
        try
        {
            var registry = CreateRegistry(
                statePath,
                new InMemoryOrchestraRunStore());
            var intent = CreateReplayAmbiguousIntent(
                registry,
                "runtime-reconcile-drift");
            var daemon = new FakeDaemonProjectionReader(
                new DaemonRuntimeProjectionSnapshot(
                    29,
                    Array.Empty<DaemonRuntimeProjection>()),
                new DaemonRuntimeProjectionSnapshot(
                    30,
                    Array.Empty<DaemonRuntimeProjection>()));

            await using var app = await BuildTestAppAsync(
                registry,
                daemon);
            var client = app.GetTestClient();
            var planResponse = await client.GetAsync(
                ReconciliationPlanPath(intent.IntentId));
            Assert.Equal(
                HttpStatusCode.OK,
                planResponse.StatusCode);

            var rejected = await SendReconcileAsync(
                client,
                intent.IntentId,
                new RuntimeDeletionReconcileRequest(
                    intent.Revision,
                    29,
                    "reconcile-drift-request",
                    "operator-a",
                    true));
            Assert.Equal(
                HttpStatusCode.Conflict,
                rejected.StatusCode);
            var error = await rejected.Content.ReadFromJsonAsync(
                LeserpentJsonContext.Default.ApiErrorResponse);
            Assert.Equal(
                "runtime_deletion_reconciliation_daemon_revision_changed",
                error!.Error);
            Assert.Single(registry.ListPendingRuntimeDeletions());
            Assert.Empty(
                registry.ListRuntimeDeletionReconciliationAudit());
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task ReconciliationRequiresConfirmationAndRollsBackStoreFailure()
    {
        var statePath = TemporaryStatePath();
        try
        {
            var runStore = new FailingDeleteOrchestraRunStore();
            var registry = CreateRegistry(statePath, runStore);
            const string runtimeId = "runtime-reconcile-rollback";
            var intent = CreateReplayAmbiguousIntent(
                registry,
                runtimeId);
            var daemon = new FakeDaemonProjectionReader(
                new DaemonRuntimeProjectionSnapshot(
                    31,
                    Array.Empty<DaemonRuntimeProjection>()));

            await using var app = await BuildTestAppAsync(
                registry,
                daemon);
            var client = app.GetTestClient();
            var unconfirmed = await SendReconcileAsync(
                client,
                intent.IntentId,
                new RuntimeDeletionReconcileRequest(
                    intent.Revision,
                    31,
                    "reconcile-unconfirmed-request",
                    "operator-a",
                    false));
            Assert.Equal(
                HttpStatusCode.BadRequest,
                unconfirmed.StatusCode);

            var zeroRevision = await SendReconcileAsync(
                client,
                intent.IntentId,
                new RuntimeDeletionReconcileRequest(
                    intent.Revision,
                    0,
                    "reconcile-zero-revision-request",
                    "operator-a",
                    true));
            Assert.Equal(
                HttpStatusCode.BadRequest,
                zeroRevision.StatusCode);

            runStore.FailDeletes = true;
            var failed = await SendReconcileAsync(
                client,
                intent.IntentId,
                new RuntimeDeletionReconcileRequest(
                    intent.Revision,
                    31,
                    "reconcile-rollback-request",
                    "operator-a",
                    true));
            Assert.Equal(
                HttpStatusCode.ServiceUnavailable,
                failed.StatusCode);
            Assert.NotNull(registry.GetRuntime(runtimeId));
            Assert.Single(registry.ListPendingRuntimeDeletions());
            Assert.Empty(
                registry.ListRuntimeDeletionReconciliationAudit());

            var diskReloaded = CreateRegistry(
                statePath,
                new InMemoryOrchestraRunStore());
            Assert.NotNull(diskReloaded.GetRuntime(runtimeId));
            Assert.Single(
                diskReloaded.ListPendingRuntimeDeletions());
            Assert.Empty(
                diskReloaded
                    .ListRuntimeDeletionReconciliationAudit());
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    private static PersistedRuntimeDeletionIntent
        CreateReplayAmbiguousIntent(
            RegistryService registry,
            string runtimeId)
    {
        registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                $"Runtime {runtimeId}",
                $"https://{runtimeId}.example",
                "pairing-token"),
            runtimeId);
        using (var reservation =
            registry.ReserveRuntimeDeletion(new[] { runtimeId }))
        {
            registry.FenceRuntimeDeletionMutation(
                reservation,
                replayHorizonFloor: 1);
            var attemptedAt = DateTimeOffset.UtcNow;
            registry.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        reservation,
                        RuntimeDeletionFailureCodes.ReplayAmbiguous,
                        attemptedAt),
                });
        }
        return Assert.Single(
            registry.ListPendingRuntimeDeletions());
    }

    private static DaemonRuntimeProjection Projection(
        string runtimeId,
        ulong revision) =>
        new(
            runtimeId,
            runtimeId,
            $"https://{runtimeId}.example",
            null,
            DateTimeOffset.UtcNow.AddMinutes(-1),
            DateTimeOffset.UtcNow,
            revision,
            new RuntimeTags(null, null, null),
            new RuntimeStatusSnapshot(
                "ready",
                null,
                null,
                true,
                null,
                null,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                false),
            null,
            null);

    private static string ReconciliationPlanPath(string intentId) =>
        $"/v1/persistence/runtime-deletions/{intentId}/reconciliation-plan";

    private static async Task<HttpResponseMessage> SendReconcileAsync(
        HttpClient client,
        string intentId,
        RuntimeDeletionReconcileRequest request)
    {
        var message = new HttpRequestMessage(
            HttpMethod.Post,
            $"/v1/persistence/runtime-deletions/{intentId}/reconcile")
        {
            Content = JsonContent.Create(request),
        };
        message.Headers.Add(
            ControlPlaneSecurityPolicy.IntentHeader,
            ControlPlaneSecurityPolicy.MutateIntent);
        return await client.SendAsync(message);
    }

    private static string TemporaryStatePath() =>
        Path.Combine(
            Path.GetTempPath(),
            $"leserpent-reconciliation-{Guid.NewGuid():N}.json");

    private static void DeleteStateFiles(string statePath)
    {
        File.Delete(statePath);
        File.Delete($"{statePath}.bak");
    }

    private static RegistryService CreateRegistry(
        string statePath,
        IOrchestraRunStore runStore)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_STATE_PATH"] = statePath,
            })
            .Build();
        return new RegistryService(
            new ControlPlaneStateStore(
                configuration,
                new TestHostEnvironment
                {
                    ContentRootPath =
                        Path.GetDirectoryName(statePath)!,
                },
                NullLogger<ControlPlaneStateStore>.Instance),
            runStore);
    }

    private static SqliteOrchestraRunStore CreateSqliteStore(
        string databasePath)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_DATABASE_PATH"] = databasePath,
            })
            .Build();
        return new SqliteOrchestraRunStore(
            configuration,
            new TestHostEnvironment
            {
                ContentRootPath =
                    Path.GetDirectoryName(databasePath)!,
            },
            NullLogger<SqliteOrchestraRunStore>.Instance);
    }

    private static async Task<WebApplication> BuildTestAppAsync(
        RegistryService registry,
        IDaemonRuntimeProjectionReader daemon)
    {
        var builder = WebApplication.CreateBuilder();
        builder.WebHost.UseTestServer();
        builder.Services.AddSingleton(registry);
        builder.Services.AddSingleton<
            IDaemonRuntimeProjectionReader>(daemon);
        builder.Services.AddSingleton<RuntimeDeletionRecoverySignal>();
        builder.Services.AddSingleton<ControlPlaneSecurityPolicy>();
        var app = builder.Build();
        app.Use(async (context, next) =>
        {
            context.Connection.RemoteIpAddress = IPAddress.Loopback;
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
        Leserpent.Program.MapPersistenceEndpoints(app);
        await app.StartAsync();
        return app;
    }

    private sealed class FakeDaemonProjectionReader :
        IDaemonRuntimeProjectionReader
    {
        private readonly Queue<DaemonRuntimeProjectionSnapshot>
            snapshots;
        private DaemonRuntimeProjectionSnapshot? last;

        public static FakeDaemonProjectionReader Disabled { get; } =
            new(false);

        public FakeDaemonProjectionReader(
            params DaemonRuntimeProjectionSnapshot[] snapshots)
        {
            this.snapshots = new(snapshots);
            last = snapshots.LastOrDefault();
            Enabled = true;
        }

        private FakeDaemonProjectionReader(bool enabled)
        {
            snapshots = new();
            Enabled = enabled;
        }

        public bool Enabled { get; }

        public Task<IReadOnlyList<DaemonRuntimeProjection>> ListAsync(
            RuntimeListFilter filter,
            CancellationToken cancellationToken) =>
            Task.FromResult<IReadOnlyList<DaemonRuntimeProjection>>(
                last?.Runtimes ??
                    Array.Empty<DaemonRuntimeProjection>());

        public Task<DaemonRuntimeProjectionSnapshot> SnapshotAsync(
            CancellationToken cancellationToken)
        {
            if (snapshots.Count > 0)
            {
                last = snapshots.Dequeue();
            }
            return Task.FromResult(
                last ??
                    new DaemonRuntimeProjectionSnapshot(
                        0,
                        Array.Empty<DaemonRuntimeProjection>()));
        }

        public Task<DaemonRuntimeProjection?> InspectAsync(
            string runtimeId,
            CancellationToken cancellationToken) =>
            Task.FromResult(
                last?.Runtimes.FirstOrDefault(runtime =>
                    string.Equals(
                        runtime.RuntimeId,
                        runtimeId,
                        StringComparison.Ordinal)));
    }

    private sealed class FailingDeleteOrchestraRunStore :
        IOrchestraRunStore
    {
        private readonly InMemoryOrchestraRunStore inner = new();

        public bool FailDeletes { get; set; }
        public string Provider => inner.Provider;
        public string Location => inner.Location;
        public int SchemaVersion => inner.SchemaVersion;
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
            IReadOnlyCollection<string> runtimeIds) =>
            !FailDeletes && inner.DeleteRuntimes(runtimeIds);
        public OrchestraDeleteReceipt? DeleteRuntimes(
            OrchestraDeleteCommand command) =>
            FailDeletes ? null : inner.DeleteRuntimes(command);
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
