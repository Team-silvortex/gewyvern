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
    public async Task ProlongedCheckpointOutageRetainsPressureBackoffAndAcknowledgement()
    {
        var statePath = TemporaryStatePath();
        try
        {
            var clock = new ManualTimeProvider(
                DateTimeOffset.Parse("2026-07-27T00:00:00Z"));
            var runStore = new OutageCheckpointOrchestraRunStore(
                new OrchestraDeleteReplayHorizon(
                    Capacity: 4096,
                    Retained: 4000,
                    OldestGeneration: 1,
                    NewestGeneration: 4000,
                    NextGeneration: 4001,
                    EvictedThroughGeneration: 0,
                    ProtectedFromGeneration: 1,
                    CheckpointedThroughGeneration: 3990));
            var registry = new RegistryService(
                CreateStateStore(statePath),
                runStore,
                clock);
            var healthy = Assert.IsType<
                OrchestraDeleteReplayCheckpointStatus>(
                    registry
                        .GetOrchestraDeleteReplayCheckpointStatus());
            Assert.Equal(
                OrchestraDeleteReplayAdmissionPressure.Critical,
                healthy.AdmissionPressure);
            Assert.Equal(10UL, healthy.Horizon.CheckpointLagGenerations);
            Assert.False(healthy.ObservationStale);

            runStore.Outage = true;
            var attemptsBeforeOutage = runStore.HorizonReadCount;
            var failed = Assert.IsType<
                OrchestraDeleteReplayCheckpointStatus>(
                    registry
                        .GetOrchestraDeleteReplayCheckpointStatus());
            Assert.Equal(
                checked(attemptsBeforeOutage + 1),
                runStore.HorizonReadCount);
            Assert.True(failed.ObservationStale);
            Assert.True(failed.AlertActive);
            Assert.False(failed.AlertAcknowledged);
            Assert.Equal(1U, failed.ConsecutiveFailureCount);
            Assert.Equal(
                clock.GetUtcNow().AddSeconds(1),
                failed.NextRetryAt);
            Assert.Equal(
                OrchestraDeleteReplayAdmissionPressure.Critical,
                failed.AdmissionPressure);
            Assert.Equal(10UL, failed.Horizon.CheckpointLagGenerations);

            _ = registry.GetOrchestraDeleteReplayCheckpointStatus();
            Assert.Equal(
                checked(attemptsBeforeOutage + 1),
                runStore.HorizonReadCount);

            clock.Advance(TimeSpan.FromSeconds(1));
            var secondFailure = Assert.IsType<
                OrchestraDeleteReplayCheckpointStatus>(
                    registry
                        .GetOrchestraDeleteReplayCheckpointStatus());
            Assert.Equal(2U, secondFailure.ConsecutiveFailureCount);
            Assert.Equal(
                clock.GetUtcNow().AddSeconds(2),
                secondFailure.NextRetryAt);

            await using (var app = await BuildTestAppAsync(
                registry,
                FakeDaemonProjectionReader.Disabled))
            {
                var request =
                    new OrchestraDeleteCheckpointAlertAcknowledgeRequest(
                        secondFailure.AlertGeneration,
                        "operator-a",
                        true);
                using var message = new HttpRequestMessage(
                    HttpMethod.Post,
                    "/v1/persistence/orchestra-cleanup-replay-status/acknowledge")
                {
                    Content = JsonContent.Create(
                        request,
                        LeserpentJsonContext.Default
                            .OrchestraDeleteCheckpointAlertAcknowledgeRequest),
                };
                message.Headers.Add(
                    ControlPlaneSecurityPolicy.IntentHeader,
                    ControlPlaneSecurityPolicy.MutateIntent);
                var response = await app.GetTestClient().SendAsync(
                    message);
                Assert.Equal(HttpStatusCode.OK, response.StatusCode);
                var acknowledgement =
                    await response.Content.ReadFromJsonAsync(
                        LeserpentJsonContext.Default
                            .OrchestraDeleteCheckpointAlertAcknowledgeResponse);
                Assert.NotNull(acknowledgement);
                Assert.True(acknowledgement.Acknowledged);
                Assert.False(acknowledgement.Replayed);
                Assert.True(
                    acknowledgement.Status.AlertAcknowledged);
                Assert.Equal(
                    "operator-a",
                    acknowledgement.Status.AcknowledgedBy);
            }

            var attemptsBeforeRestart = runStore.HorizonReadCount;
            var restarted = new RegistryService(
                CreateStateStore(statePath),
                runStore,
                clock);
            var restored = Assert.IsType<
                OrchestraDeleteReplayCheckpointStatus>(
                    restarted
                        .GetOrchestraDeleteReplayCheckpointStatus());
            Assert.Equal(attemptsBeforeRestart, runStore.HorizonReadCount);
            Assert.True(restored.ObservationStale);
            Assert.True(restored.AlertAcknowledged);
            Assert.Equal("operator-a", restored.AcknowledgedBy);
            Assert.Equal(
                secondFailure.AlertGeneration,
                restored.AlertGeneration);

            for (var expectedFailure = 3U;
                 expectedFailure <= 7;
                 expectedFailure += 1)
            {
                clock.Advance(
                    restored.NextRetryAt!.Value -
                    clock.GetUtcNow());
                restored = Assert.IsType<
                    OrchestraDeleteReplayCheckpointStatus>(
                        restarted
                            .GetOrchestraDeleteReplayCheckpointStatus());
                Assert.Equal(
                    expectedFailure,
                    restored.ConsecutiveFailureCount);
                Assert.True(
                    restored.NextRetryAt - restored.LastAttemptAt <=
                    TimeSpan.FromSeconds(30));
                Assert.True(restored.AlertAcknowledged);
            }
            Assert.Equal(
                TimeSpan.FromSeconds(30),
                restored.NextRetryAt - restored.LastAttemptAt);

            runStore.Outage = false;
            clock.Advance(
                restored.NextRetryAt!.Value -
                clock.GetUtcNow());
            var recovered = Assert.IsType<
                OrchestraDeleteReplayCheckpointStatus>(
                    restarted
                        .GetOrchestraDeleteReplayCheckpointStatus());
            Assert.False(recovered.ObservationStale);
            Assert.False(recovered.AlertActive);
            Assert.Equal(0U, recovered.ConsecutiveFailureCount);
            Assert.Null(recovered.NextRetryAt);
            Assert.Null(recovered.LastFailureCode);

            runStore.Outage = true;
            var nextIncident = Assert.IsType<
                OrchestraDeleteReplayCheckpointStatus>(
                    restarted
                        .GetOrchestraDeleteReplayCheckpointStatus());
            Assert.True(nextIncident.AlertActive);
            Assert.False(nextIncident.AlertAcknowledged);
            Assert.Equal(
                checked(recovered.AlertGeneration + 1),
                nextIncident.AlertGeneration);
            Assert.Null(nextIncident.AcknowledgedBy);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task HostedCheckpointWorkerRecoversAndDrainsDurableAlertWithoutPolling()
    {
        var statePath = TemporaryStatePath();
        var runStore = new OutageCheckpointOrchestraRunStore(
            new OrchestraDeleteReplayHorizon(
                Capacity: 4096,
                Retained: 4000,
                OldestGeneration: 1,
                NewestGeneration: 4000,
                NextGeneration: 4001,
                EvictedThroughGeneration: 0,
                ProtectedFromGeneration: 1,
                CheckpointedThroughGeneration: 3990));
        var options = new OrchestraDeleteCheckpointWorkerOptions(
            TimeSpan.FromMilliseconds(20),
            TimeSpan.FromMilliseconds(5));
        try
        {
            var registry = new RegistryService(
                CreateStateStore(statePath),
                runStore);
            runStore.Outage = true;
            var failingSink = new RecordingCheckpointAlertSink
            {
                FailDeliveries = true,
            };
            var firstWorker = new OrchestraDeleteCheckpointService(
                registry,
                failingSink,
                NullLogger<
                    OrchestraDeleteCheckpointService>.Instance,
                options);
            try
            {
                await firstWorker.StartAsync(
                    CancellationToken.None);
                await WaitUntilAsync(
                    () =>
                    {
                        var state = registry.ExportState();
                        return state
                                .OrchestraDeleteCheckpointMonitor?
                                .ConsecutiveFailureCount > 0 &&
                            state
                                .OrchestraDeleteCheckpointAlertOutbox?
                                .SingleOrDefault()?
                                .LastDeliveryFailureCode ==
                            "checkpoint_alert_delivery_failed";
                    },
                    TimeSpan.FromSeconds(3));
            }
            finally
            {
                await firstWorker.StopAsync(
                    CancellationToken.None);
                firstWorker.Dispose();
            }

            var persisted = registry.ExportState();
            var pending = Assert.Single(
                persisted.OrchestraDeleteCheckpointAlertOutbox!);
            Assert.True(pending.AttemptCount > 0);
            Assert.Equal(
                "checkpoint_alert_delivery_failed",
                pending.LastDeliveryFailureCode);
            Assert.Contains(
                pending.EventId,
                failingSink.AttemptedEventIds);

            runStore.Outage = false;
            var restarted = new RegistryService(
                CreateStateStore(statePath),
                runStore);
            var restored = Assert.Single(
                restarted.ExportState()
                    .OrchestraDeleteCheckpointAlertOutbox!);
            Assert.Equal(pending.EventId, restored.EventId);
            Assert.Equal(
                pending.AttemptCount,
                restored.AttemptCount);

            var recoveredSink =
                new RecordingCheckpointAlertSink();
            var restartedWorker =
                new OrchestraDeleteCheckpointService(
                    restarted,
                    recoveredSink,
                    NullLogger<
                        OrchestraDeleteCheckpointService>.Instance,
                    options);
            try
            {
                await restartedWorker.StartAsync(
                    CancellationToken.None);
                await WaitUntilAsync(
                    () =>
                    {
                        var state = restarted.ExportState();
                        return state
                                .OrchestraDeleteCheckpointMonitor?
                                .ConsecutiveFailureCount == 0 &&
                            state
                                .OrchestraDeleteCheckpointAlertOutbox?
                                .Count == 0;
                    },
                    TimeSpan.FromSeconds(4));
            }
            finally
            {
                await restartedWorker.StopAsync(
                    CancellationToken.None);
                restartedWorker.Dispose();
            }

            Assert.Equal(
                new[] { pending.EventId },
                recoveredSink.AttemptedEventIds);
            Assert.True(runStore.HorizonReadCount >= 2);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void CheckpointAlertOutboxBackoffCapsAtThirtySecondsAndSurvivesRestart()
    {
        var statePath = TemporaryStatePath();
        try
        {
            var clock = new ManualTimeProvider(
                DateTimeOffset.Parse("2026-07-27T00:00:00Z"));
            var runStore = new OutageCheckpointOrchestraRunStore(
                new OrchestraDeleteReplayHorizon(
                    Capacity: 4096,
                    Retained: 4000,
                    OldestGeneration: 1,
                    NewestGeneration: 4000,
                    NextGeneration: 4001,
                    EvictedThroughGeneration: 0,
                    ProtectedFromGeneration: 1,
                    CheckpointedThroughGeneration: 3990));
            var registry = new RegistryService(
                CreateStateStore(statePath),
                runStore,
                clock);
            runStore.Outage = true;
            registry.RunOrchestraDeleteCheckpointMaintenance();
            var original = Assert.Single(
                registry.ExportState()
                    .OrchestraDeleteCheckpointAlertOutbox!);
            var expectedDelays = new[] { 1, 2, 4, 8, 16, 30, 30 };

            foreach (var expectedDelay in expectedDelays)
            {
                var claimed = Assert.IsType<
                    PersistedOrchestraDeleteCheckpointAlertDelivery>(
                        registry
                            .ClaimDueOrchestraDeleteCheckpointAlertDelivery());
                Assert.Equal(original.EventId, claimed.EventId);
                Assert.Equal(
                    TimeSpan.FromSeconds(expectedDelay),
                    claimed.NextAttemptAt - claimed.LastAttemptAt);
                registry
                    .RecordOrchestraDeleteCheckpointAlertDeliveryFailure(
                        claimed.EventId);
                clock.Advance(TimeSpan.FromSeconds(expectedDelay));
            }

            var persisted = Assert.Single(
                registry.ExportState()
                    .OrchestraDeleteCheckpointAlertOutbox!);
            Assert.Equal(7U, persisted.AttemptCount);
            Assert.Equal(original.EventId, persisted.EventId);
            Assert.Equal(
                "checkpoint_alert_delivery_failed",
                persisted.LastDeliveryFailureCode);

            runStore.Outage = false;
            var restarted = new RegistryService(
                CreateStateStore(statePath),
                runStore,
                clock);
            var restored = Assert.Single(
                restarted.ExportState()
                    .OrchestraDeleteCheckpointAlertOutbox!);
            Assert.Equal(persisted, restored);
        }
        finally
        {
            DeleteStateFiles(statePath);
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
        IOrchestraRunStore runStore) =>
        new(CreateStateStore(statePath), runStore);

    private static ControlPlaneStateStore CreateStateStore(
        string statePath)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_STATE_PATH"] = statePath,
            })
            .Build();
        return new ControlPlaneStateStore(
            configuration,
            new TestHostEnvironment
            {
                ContentRootPath =
                    Path.GetDirectoryName(statePath)!,
            },
            NullLogger<ControlPlaneStateStore>.Instance);
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

    private sealed class OutageCheckpointOrchestraRunStore(
        OrchestraDeleteReplayHorizon horizon) : IOrchestraRunStore
    {
        public bool Outage { get; set; }
        public int HorizonReadCount { get; private set; }
        public string Provider => "outage-test";
        public string Location => "memory";
        public int SchemaVersion => 18;
        public bool SupportsDeleteReplayHorizon => true;
        public bool DeleteReplayHorizonAvailabilityMayBeTransient =>
            true;
        public string? LastError { get; private set; }
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
            GetDeleteReplayHorizon()
        {
            HorizonReadCount += 1;
            LastError = Outage
                ? "orchestra_store_operation_failed"
                : null;
            return Outage ? null : horizon;
        }
    }

    private sealed class ManualTimeProvider(
        DateTimeOffset current) : TimeProvider
    {
        public override DateTimeOffset GetUtcNow() => current;

        public void Advance(TimeSpan duration)
        {
            Assert.True(duration >= TimeSpan.Zero);
            current += duration;
        }
    }

    private sealed class RecordingCheckpointAlertSink :
        IOrchestraDeleteCheckpointAlertSink
    {
        private readonly object sync = new();
        private readonly List<string> attemptedEventIds = new();

        public bool FailDeliveries { get; init; }

        public IReadOnlyList<string> AttemptedEventIds
        {
            get
            {
                lock (sync)
                {
                    return attemptedEventIds.ToArray();
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
                attemptedEventIds.Add(delivery.EventId);
            }
            return FailDeliveries
                ? Task.FromException(
                    new IOException("alert sink unavailable"))
                : Task.CompletedTask;
        }
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
