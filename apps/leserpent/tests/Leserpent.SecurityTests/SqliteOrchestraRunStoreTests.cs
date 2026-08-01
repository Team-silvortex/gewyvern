using System.Text.Json;
using Leserpent;
using Leserpent.ControlPlane;
using Microsoft.Data.Sqlite;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class SqliteOrchestraRunStoreTests
{
    [Fact]
    public void SqliteStoreActivatesAndRevokesWritesWithTheWriterFence()
    {
        var databasePath = TemporaryPath("db");
        var writable = false;
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_DATABASE_PATH"] = databasePath,
            })
            .Build();
        var store = new SqliteOrchestraRunStore(
            configuration,
            new TestHostEnvironment
            {
                ContentRootPath = Path.GetDirectoryName(databasePath)!,
            },
            NullLogger<SqliteOrchestraRunStore>.Instance,
            () => writable);
        var first = CreateRun("run-fenced-1", "request-fenced-1", "queued");

        Assert.False(store.Upsert(first));
        Assert.Equal("orchestra_store_read_only", store.LastError);
        Assert.False(File.Exists(databasePath));

        writable = true;
        Assert.True(store.Upsert(first));
        Assert.True(File.Exists(databasePath));
        Assert.Single(store.LoadAll());

        writable = false;
        var second = CreateRun("run-fenced-2", "request-fenced-2", "queued");
        Assert.False(store.Upsert(second));
        Assert.Equal("orchestra_store_read_only", store.LastError);

        writable = true;
        Assert.Single(store.LoadAll());
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqliteStoreUpsertsRunsAndEnforcesRequestIdUniqueness()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        var queued = CreateRun("run-1", "request-unique-1", "queued");

        store.Upsert(queued, CreateEvent(queued, null, "queued"));
        var running = queued with
        {
            Outcome = "running",
        };
        store.Upsert(
            running,
            CreateEvent(
                running,
                "queued",
                "running"));
        var succeeded = queued with
        {
            Outcome = "succeeded",
            CompletedAt = DateTimeOffset.UtcNow,
        };
        store.Upsert(
            succeeded,
            CreateEvent(
                succeeded,
                "running",
                "succeeded"));
        store.Upsert(CreateRun("run-2", "request-unique-1", "queued"));
        var duplicateError = store.LastError;

        var loaded = store.LoadAll();
        Assert.Single(loaded);
        Assert.Equal("succeeded", loaded[0].Outcome);
        var events = store.LoadEvents("runtime-1", "run-1");
        Assert.Equal(3, events.Count);
        Assert.Equal("queued", events[0].ToOutcome);
        Assert.Equal("queued", events[1].FromOutcome);
        Assert.Equal("running", events[1].ToOutcome);
        Assert.Equal("running", events[2].FromOutcome);
        Assert.Equal("succeeded", events[2].ToOutcome);
        Assert.True(events[2].EventId > events[1].EventId);
        Assert.Equal(
            "orchestra_store_operation_failed",
            duplicateError);
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqliteStoreRetainsNewestThirtyTwoRunsPerRuntime()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        for (var index = 0; index < 40; index += 1)
        {
            var run = CreateRun(
                $"run-{index}",
                $"request-retention-{index}",
                "succeeded",
                DateTimeOffset.UnixEpoch.AddMinutes(index));
            store.Upsert(run, CreateEvent(run, null, "succeeded"));
        }

        var loaded = store.LoadAll();
        Assert.Equal(32, loaded.Count);
        Assert.DoesNotContain(loaded, run => run.RunId == "run-7");
        Assert.Contains(loaded, run => run.RunId == "run-8");
        Assert.Contains(loaded, run => run.RunId == "run-39");
        Assert.Empty(store.LoadEvents("runtime-1", "run-7"));
        Assert.Single(store.LoadEvents("runtime-1", "run-8"));
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqliteStoreMigratesVersionOneDatabaseInPlace()
    {
        var databasePath = TemporaryPath("db");
        CreateVersionOneDatabase(databasePath);

        var store = CreateSqliteStore(databasePath);
        var run = Assert.Single(store.LoadAll());
        Assert.Equal(5, store.SchemaVersion);
        Assert.Equal("legacy-run", run.RunId);

        store.Upsert(
            run,
            ControlPlaneStateValidator
                .CreateLegacyOrchestraImportEvent(run));
        var running = run with
        {
            Outcome = "running",
        };
        store.Upsert(
            running,
            CreateEvent(
                running,
                "queued",
                "running"));
        var succeeded = run with
        {
            Outcome = "succeeded",
            CompletedAt = DateTimeOffset.UtcNow,
        };
        store.Upsert(
            succeeded,
            CreateEvent(
                succeeded,
                "running",
                "succeeded"));
        Assert.Equal(
            3,
            store.LoadEvents("runtime-1", "legacy-run").Count);
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqliteStoreDeletesRunEventsWithRuntime()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        var run = CreateRun("run-delete", "request-delete", "queued");
        store.Upsert(run, CreateEvent(run, null, "queued"));

        store.DeleteRuntimes(new[] { "runtime-1" });

        Assert.Empty(store.LoadAll());
        Assert.Empty(store.LoadEvents("runtime-1", "run-delete"));
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqliteStoreReplaysTypedDeleteReceiptAcrossRestart()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        var run = CreateRun(
            "run-delete-receipt",
            "request-delete-receipt",
            "queued");
        Assert.True(store.Upsert(
            run,
            CreateEvent(run, null, "queued")));
        var command = new OrchestraDeleteCommand(
            "orchestra-delete-receipt",
            new[] { "runtime-1" });

        var first = store.DeleteRuntimes(command);
        Assert.NotNull(first);
        Assert.False(first.Replayed);
        Assert.Equal(1UL, first.OperationGeneration);
        Assert.Equal(1UL, first.DeletedRunCount);
        Assert.Equal(1UL, first.DeletedEventCount);

        var restarted = CreateSqliteStore(databasePath);
        var replay = restarted.DeleteRuntimes(command);
        Assert.NotNull(replay);
        Assert.True(replay.Replayed);
        Assert.Equal(
            first.OperationGeneration,
            replay.OperationGeneration);
        Assert.Equal(first.CommittedAt, replay.CommittedAt);
        Assert.Null(restarted.DeleteRuntimes(
            command with
            {
                RuntimeIds = new[] { "runtime-conflict" },
            }));
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqliteDeleteReplayCheckpointCompactsOnlyAuditedPrefix()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        var commands = Enumerable.Range(1, 3)
            .Select(index => new OrchestraDeleteCommand(
                $"orchestra-cleanup-{index}",
                new[] { "runtime-1" }))
            .ToArray();
        foreach (var (command, index) in commands.Select(
            static (command, index) => (command, index)))
        {
            Assert.Equal(
                checked((ulong)index + 1),
                store.DeleteRuntimes(command)!.OperationGeneration);
        }
        Assert.Equal(
            new OrchestraDeleteReplayHorizon(
                4096,
                3,
                1,
                3,
                4,
                0,
                1),
            store.GetDeleteReplayHorizon());
        Assert.Null(store.CheckpointDeleteReplayHorizon(
            new OrchestraDeleteReplayCheckpoint(3, 2)));

        var checkpointed = store.CheckpointDeleteReplayHorizon(
            new OrchestraDeleteReplayCheckpoint(2, 3));
        Assert.Equal(
            new OrchestraDeleteReplayHorizon(
                4096,
                2,
                2,
                3,
                4,
                1,
                2,
                3),
            checkpointed);
        Assert.Null(store.CheckpointDeleteReplayHorizon(
            new OrchestraDeleteReplayCheckpoint(1, 3)));

        var restarted = CreateSqliteStore(databasePath);
        Assert.Equal(
            checkpointed,
            restarted.GetDeleteReplayHorizon());
        foreach (var index in new[] { 1, 2 })
        {
            var replay = restarted.DeleteRuntimes(commands[index]);
            Assert.NotNull(replay);
            Assert.True(replay.Replayed);
            Assert.Equal(
                checked((ulong)index + 1),
                replay.OperationGeneration);
        }
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqlitePinnedDeleteReplayHorizonReportsSaturationAndCheckpointRestoresAdmission()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        using (var connection = new SqliteConnection(
            $"Data Source={databasePath}"))
        {
            connection.Open();
            using var transaction = connection.BeginTransaction();
            using (var insert = connection.CreateCommand())
            {
                insert.Transaction = transaction;
                insert.CommandText = """
                    INSERT INTO orchestra_delete_operations (
                        generation, operation_id, runtime_ids_json,
                        deleted_runtime_count, deleted_run_count,
                        deleted_event_count, committed_at_unix_ms)
                    VALUES (
                        $generation, $operation_id, '["runtime-1"]',
                        0, 0, 0, $generation);
                    """;
                var generationParameter =
                    insert.Parameters.Add("$generation", SqliteType.Integer);
                var operationParameter =
                    insert.Parameters.Add("$operation_id", SqliteType.Text);
                for (var generation = 1; generation <= 4096; generation++)
                {
                    generationParameter.Value = generation;
                    operationParameter.Value =
                        $"orchestra-cleanup-saturated-{generation}";
                    Assert.Equal(1, insert.ExecuteNonQuery());
                }
            }
            using (var protect = connection.CreateCommand())
            {
                protect.Transaction = transaction;
                protect.CommandText = """
                    UPDATE orchestra_delete_replay_horizon
                    SET protected_from_generation = 1
                    WHERE id = 1;
                    """;
                Assert.Equal(1, protect.ExecuteNonQuery());
            }
            transaction.Commit();
        }

        var saturated = store.GetDeleteReplayHorizon();
        Assert.NotNull(saturated);
        Assert.Equal(0UL, saturated.AvailableCapacity);
        Assert.True(saturated.Saturated);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionState
                .BlockedByReconciliationAudit,
            saturated.AdmissionState);
        Assert.Equal(
            OrchestraDeleteReplayOperatorAction
                .PersistAuditAndAdvanceCheckpoint,
            saturated.OperatorAction);
        Assert.Null(store.DeleteRuntimes(
            new OrchestraDeleteCommand(
                "orchestra-cleanup-saturated-overflow",
                new[] { "runtime-1" })));
        Assert.Equal(
            "orchestra_store_operation_failed",
            store.LastError);

        var checkpointed = store.CheckpointDeleteReplayHorizon(
            new OrchestraDeleteReplayCheckpoint(4096, 4096));
        Assert.NotNull(checkpointed);
        Assert.Equal(4095UL, checkpointed.AvailableCapacity);
        Assert.False(checkpointed.Saturated);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionState.Ready,
            checkpointed.AdmissionState);
        Assert.Null(checkpointed.OperatorAction);
        var admitted = store.DeleteRuntimes(
            new OrchestraDeleteCommand(
                "orchestra-cleanup-saturated-admitted",
                new[] { "runtime-1" }));
        Assert.NotNull(admitted);
        Assert.Equal(4097UL, admitted.OperationGeneration);

        DeleteDatabase(databasePath);
    }

    [Fact]
    public void DeleteReplayAdmissionPressureUsesStableProtectedCapacityThresholds()
    {
        static OrchestraDeleteReplayHorizon Horizon(
            ulong available,
            ulong? protectedFrom = 1) =>
            new(
                4096,
                4096 - available,
                1,
                4096 - available,
                4097 - available,
                0,
                protectedFrom);

        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Healthy,
            Horizon(513).AdmissionPressure);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Warning,
            Horizon(512).AdmissionPressure);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Critical,
            Horizon(128).AdmissionPressure);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Blocked,
            Horizon(0).AdmissionPressure);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Healthy,
            Horizon(0, null).AdmissionPressure);
        Assert.Null(Horizon(513).OperatorAction);
        Assert.Equal(
            OrchestraDeleteReplayOperatorAction
                .PersistAuditAndAdvanceCheckpoint,
            Horizon(512).OperatorAction);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Critical,
            Horizon(256).AdmissionPressureWithHysteresis(
                OrchestraDeleteReplayAdmissionPressure.Critical));
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Warning,
            Horizon(257).AdmissionPressureWithHysteresis(
                OrchestraDeleteReplayAdmissionPressure.Critical));
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Warning,
            Horizon(768).AdmissionPressureWithHysteresis(
                OrchestraDeleteReplayAdmissionPressure.Warning));
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Healthy,
            Horizon(769).AdmissionPressureWithHysteresis(
                OrchestraDeleteReplayAdmissionPressure.Warning));
        Assert.Equal(Horizon(512).Retained, Horizon(512).CheckpointLagGenerations);
        Assert.Equal(
            584UL,
            (Horizon(512) with
            {
                CheckpointedThroughGeneration = 3000,
            }).CheckpointLagGenerations);
    }

    [Fact]
    public void SqliteSchemaThreeReceiptsMigrateLosslesslyToReplayHorizon()
    {
        var databasePath = TemporaryPath("db");
        var command = new OrchestraDeleteCommand(
            "orchestra-cleanup-v3",
            new[] { "runtime-1" });
        var first = CreateSqliteStore(databasePath)
            .DeleteRuntimes(command);
        Assert.NotNull(first);
        using (var connection = new SqliteConnection(
            $"Data Source={databasePath}"))
        {
            connection.Open();
            using var downgrade = connection.CreateCommand();
            downgrade.CommandText = """
                DROP TABLE orchestra_delete_replay_horizon;
                PRAGMA user_version = 3;
                """;
            downgrade.ExecuteNonQuery();
        }

        var migrated = CreateSqliteStore(databasePath);
        Assert.Equal(
            new OrchestraDeleteReplayHorizon(
                4096,
                1,
                1,
                1,
                2,
                0,
                1,
                1),
            migrated.GetDeleteReplayHorizon());
        var replay = migrated.DeleteRuntimes(command);
        Assert.NotNull(replay);
        Assert.True(replay.Replayed);
        Assert.Equal(first.OperationGeneration, replay.OperationGeneration);
        Assert.Equal(first.CommittedAt, replay.CommittedAt);
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void RegistryAuditCheckpointProtectsThenCompactsReceiptsAcrossRestart()
    {
        var statePath = TemporaryPath("json");
        var databasePath = TemporaryPath("db");
        try
        {
            var orchestraStore = CreateSqliteStore(databasePath);
            var reconciledAt = DateTimeOffset.UtcNow;
            var audits = Enumerable.Range(1, 3)
                .Select(index =>
                {
                    var commandId = $"orchestra-cleanup-audit-{index}";
                    var receipt = orchestraStore.DeleteRuntimes(
                        new OrchestraDeleteCommand(
                            commandId,
                            new[] { $"runtime-audit-{index}" }));
                    Assert.NotNull(receipt);
                    Assert.Equal(
                        checked((ulong)index),
                        receipt.OperationGeneration);
                    return new PersistedRuntimeDeletionReconciliationAudit(
                        $"reconcile-request-{index}",
                        $"delete-intent-{index}",
                        new[] { $"runtime-audit-{index}" },
                        1,
                        checked((ulong)index),
                        "operator-a",
                        reconciledAt,
                        commandId,
                        receipt.OperationGeneration);
                })
                .ToArray();
            var stateStore = CreateStateStore(statePath);
            stateStore.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                runtimeDeletionReconciliationAudit: audits);

            var initialRegistry = new RegistryService(
                CreateStateStore(statePath),
                CreateSqliteStore(databasePath));
            var initialStatus = initialRegistry
                .GetOrchestraDeleteReplayCheckpointStatus();
            Assert.NotNull(initialStatus);
            Assert.Equal(1UL, initialStatus.MinimumAuditedGeneration);
            Assert.Equal(3UL, initialStatus.ObservedThroughAuditedGeneration);
            Assert.Equal(1UL, initialStatus.Horizon.OldestGeneration);
            Assert.Equal(3UL, initialStatus.Horizon.CheckpointedThroughGeneration);
            Assert.Equal(0UL, initialStatus.Horizon.CheckpointLagGenerations);
            Assert.True(initialStatus.LastAutomaticCheckpointAdvanced);
            Assert.NotNull(initialStatus.LastAutomaticCheckpointAt);

            stateStore.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                runtimeDeletionReconciliationAudit: audits[1..]);
            var compactionRegistry = new RegistryService(
                CreateStateStore(statePath),
                CreateSqliteStore(databasePath));
            compactionRegistry
                .RunOrchestraDeleteCheckpointMaintenance();
            var compacted = CreateSqliteStore(databasePath)
                .GetDeleteReplayHorizon();
            Assert.NotNull(compacted);
            Assert.Equal(2UL, compacted.OldestGeneration);
            Assert.Equal(1UL, compacted.EvictedThroughGeneration);
            Assert.Equal(2UL, compacted.ProtectedFromGeneration);
            Assert.Equal(3UL, compacted.CheckpointedThroughGeneration);
            Assert.Equal(0UL, compacted.CheckpointLagGenerations);

            var fourthReceipt = CreateSqliteStore(databasePath)
                .DeleteRuntimes(
                    new OrchestraDeleteCommand(
                        "orchestra-cleanup-audit-4",
                        new[] { "runtime-audit-4" }));
            Assert.NotNull(fourthReceipt);
            Assert.Equal(4UL, fourthReceipt.OperationGeneration);
            var lagging = CreateSqliteStore(databasePath)
                .GetDeleteReplayHorizon();
            Assert.NotNull(lagging);
            Assert.Equal(3UL, lagging.CheckpointedThroughGeneration);
            Assert.Equal(1UL, lagging.CheckpointLagGenerations);
            var fourthAudit =
                new PersistedRuntimeDeletionReconciliationAudit(
                    "reconcile-request-4",
                    "delete-intent-4",
                    new[] { "runtime-audit-4" },
                    1,
                    4,
                    "operator-a",
                    reconciledAt.AddSeconds(1),
                    fourthReceipt.CommandId,
                    fourthReceipt.OperationGeneration);
            stateStore.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                runtimeDeletionReconciliationAudit:
                    audits[1..].Append(fourthAudit).ToArray());
            var restartedRegistry = new RegistryService(
                CreateStateStore(statePath),
                CreateSqliteStore(databasePath));
            var converged = restartedRegistry
                .GetOrchestraDeleteReplayCheckpointStatus();
            Assert.NotNull(converged);
            Assert.Equal(2UL, converged.MinimumAuditedGeneration);
            Assert.Equal(4UL, converged.ObservedThroughAuditedGeneration);
            Assert.Equal(4UL, converged.Horizon.CheckpointedThroughGeneration);
            Assert.Equal(0UL, converged.Horizon.CheckpointLagGenerations);
            Assert.True(converged.LastAutomaticCheckpointAdvanced);

            stateStore.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                runtimeDeletionReconciliationAudit: audits);
            var outsideHorizonRegistry = new RegistryService(
                    CreateStateStore(statePath),
                    CreateSqliteStore(databasePath));
            var error = Assert.Throws<OrchestraPersistenceException>(
                outsideHorizonRegistry
                    .RunOrchestraDeleteCheckpointMaintenance);
            Assert.Contains(
                "outside the durable replay horizon",
                error.Message,
                StringComparison.Ordinal);
        }
        finally
        {
            DeleteState(statePath);
            DeleteDatabase(databasePath);
        }
    }

    [Fact]
    public void RegistryMigratesLegacyJsonRunsWhenSqliteIsEmpty()
    {
        var statePath = TemporaryPath("json");
        var databasePath = TemporaryPath("db");
        var legacyStore = CreateStateStore(statePath);
        var legacyRegistry = new RegistryService(legacyStore);
        var registered = legacyRegistry.RegisterRuntime(new RuntimeRegistrationRequest(
            "runtime",
            "http://127.0.0.1:49152",
            "pairing-token"));
        legacyRegistry.RecordOrchestraRun(
            registered.RuntimeId,
            "runtime_triage",
            "succeeded",
            Array.Empty<OrchestraExecutionStepResult>(),
            "operator",
            "legacy migration",
            "revision-1",
            "request-migrate-1");

        var sqlite = CreateSqliteStore(databasePath);
        var migratedRegistry = new RegistryService(CreateStateStore(statePath), sqlite);

        var migrated = migratedRegistry.ListOrchestraRuns(registered.RuntimeId);
        Assert.Single(migrated);
        Assert.Equal("request-migrate-1", migrated[0].RequestId);
        Assert.Single(sqlite.LoadAll());
        DeleteState(statePath);
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void SqliteReplaceAllRollsBackOnMigrationWriteFailureAndAllowsRetry()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        var existing = CreateRun("existing-run", "request-existing", "succeeded");
        store.Upsert(existing);
        var first = CreateRun("replacement-a", "request-collision", "succeeded");
        var second = CreateRun("replacement-b", "request-collision", "succeeded");

        Assert.False(store.ReplaceAll(new[] { first, second }));
        Assert.NotNull(store.LastError);
        Assert.Equal("existing-run", Assert.Single(store.LoadAll()).RunId);

        Assert.True(store.ReplaceAll(new[]
        {
            first,
            second with { RequestId = "request-retry-b" },
        }));
        Assert.Equal(2, store.LoadAll().Count);
        Assert.Null(store.LastError);
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void RegistryMigrationFailurePreservesLegacyJsonForRetryAndOperatorRollback()
    {
        var statePath = TemporaryPath("json");
        var databasePath = TemporaryPath("db");
        var legacyRegistry = new RegistryService(CreateStateStore(statePath));
        var registered = legacyRegistry.RegisterRuntime(new RuntimeRegistrationRequest(
            "runtime",
            "http://127.0.0.1:49152",
            "pairing-token"));
        legacyRegistry.RecordOrchestraRun(
            registered.RuntimeId,
            "runtime_triage",
            "succeeded",
            Array.Empty<OrchestraExecutionStepResult>(),
            "operator",
            "rollback migration",
            "revision-1",
            "request-rollback-1");
        var retainedJson = File.ReadAllBytes(statePath);
        var failingStore = new FailingMigrationStore();

        Assert.Throws<OrchestraPersistenceException>(() =>
            new RegistryService(CreateStateStore(statePath), failingStore));
        Assert.Single(failingStore.AttemptedRuns);
        Assert.Equal(retainedJson, File.ReadAllBytes(statePath));

        var sqlite = CreateSqliteStore(databasePath);
        var retried = new RegistryService(CreateStateStore(statePath), sqlite);
        Assert.Equal("request-rollback-1", Assert.Single(
            retried.ListOrchestraRuns(registered.RuntimeId)).RequestId);
        Assert.Equal(retainedJson, File.ReadAllBytes(statePath));

        var rolledBack = new RegistryService(CreateStateStore(statePath));
        Assert.Equal("request-rollback-1", Assert.Single(
            rolledBack.ListOrchestraRuns(registered.RuntimeId)).RequestId);
        DeleteState(statePath);
        DeleteDatabase(databasePath);
    }

    [Fact]
    public void RegistrySingleRuntimeDeleteRemovesSqliteRunsAndEvents()
    {
        var statePath = TemporaryPath("json");
        var databasePath = TemporaryPath("db");
        var sqlite = CreateSqliteStore(databasePath);
        var registry = new RegistryService(CreateStateStore(statePath), sqlite);
        var registered = registry.RegisterRuntime(new RuntimeRegistrationRequest(
            "runtime",
            "http://127.0.0.1:49152",
            "pairing-token"));
        var run = registry.RecordOrchestraRun(
            registered.RuntimeId,
            "session_preparation",
            "ok",
            Array.Empty<OrchestraExecutionStepResult>());

        var deleted = registry.DeleteRuntime(registered.RuntimeId);

        Assert.NotNull(deleted.RemovedRuntime);
        Assert.Empty(sqlite.LoadAll());
        Assert.Empty(sqlite.LoadEvents(registered.RuntimeId, run.RunId));
        DeleteState(statePath);
        DeleteDatabase(databasePath);
    }

    [Fact]
    public async Task ControlPlaneStateStoreSerializesConcurrentDurableSaves()
    {
        var statePath = TemporaryPath("json");
        var store = CreateStateStore(statePath);
        var runtimes = new[] { CreateRuntimeState() };
        var candidates = Enumerable.Range(0, 32)
            .Select(index => CreateRun(
                $"concurrent-run-{index}",
                $"concurrent-request-{index}",
                "succeeded"))
            .ToArray();

        await Task.WhenAll(candidates.Select(run => Task.Run(() =>
            store.Save(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { run }))));

        var restored = Assert.IsType<PersistedControlPlaneState>(store.Load());
        var persistedRuns = Assert.IsAssignableFrom<IReadOnlyList<OrchestraRunSummary>>(
            restored.OrchestraRuns);
        var persistedRun = Assert.Single(persistedRuns);
        Assert.Contains(candidates, candidate => candidate.RunId == persistedRun.RunId);
        Assert.False(store.IsDirty);
        Assert.Null(store.LastSaveError);
        Assert.Empty(Directory.EnumerateFiles(
            Path.GetDirectoryName(statePath)!,
            $"{Path.GetFileName(statePath)}.*.tmp"));
        DeleteState(statePath);
    }

    [Fact]
    public void ControlPlaneStateStoreMigratesSchemaOneState()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var schemaOne = new PersistedControlPlaneState(
                1,
                DateTimeOffset.Parse("2026-01-01T00:00:00Z"),
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                Array.Empty<OrchestraRunSummary>());
            File.WriteAllText(
                statePath,
                JsonSerializer.Serialize(
                    schemaOne,
                    new LeserpentJsonContext(new JsonSerializerOptions())
                        .PersistedControlPlaneState));

            var store = CreateStateStore(statePath);
            var loaded = store.Load();
            Assert.True(loaded is not null, store.LastSaveError);
            var restored = Assert.IsType<PersistedControlPlaneState>(loaded);

            Assert.Equal(8, restored.SchemaVersion);
            Assert.Empty(Assert.IsAssignableFrom<IReadOnlyList<PersistedRuntimeDeletionIntent>>(
                restored.PendingRuntimeDeletions));
            Assert.Empty(
                restored.RuntimeDeletionReconciliationAudit!);
            Assert.Null(
                restored.OrchestraDeleteCheckpointMonitor);
            Assert.Empty(
                restored.OrchestraDeleteCheckpointAlertOutbox!);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData(2)]
    [InlineData(3)]
    [InlineData(4)]
    [InlineData(5)]
    public void ControlPlaneStateStoreMigratesLegacyDeletionIntent(
        int schemaVersion)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var preparedAt = DateTimeOffset.UtcNow.AddMinutes(-1);
            var legacyState = new PersistedControlPlaneState(
                schemaVersion,
                preparedAt,
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                Array.Empty<OrchestraRunSummary>(),
                new[]
                {
                    new PersistedRuntimeDeletionIntent(
                        "rdel_schema_two",
                        new[] { "runtime-schema-two" },
                        preparedAt,
                        UnregistrationCommandId:
                            schemaVersion >= 4
                                ? RuntimeDeletionCommandIdentity
                                    .ForIntent(
                                        "rdel_schema_two")
                                : string.Empty),
                });
            File.WriteAllText(
                statePath,
                JsonSerializer.Serialize(
                    legacyState,
                    new LeserpentJsonContext(new JsonSerializerOptions())
                        .PersistedControlPlaneState));

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());
            var intent = Assert.Single(restored.PendingRuntimeDeletions!);

            Assert.Equal(8, restored.SchemaVersion);
            Assert.Equal(0, intent.AttemptCount);
            Assert.Null(intent.LastAttemptAt);
            Assert.Null(intent.NextAttemptAt);
            Assert.Null(intent.LastFailureCode);
            Assert.Equal(1, intent.Revision);
            Assert.Equal(
                RuntimeDeletionCommandIdentity.ForIntent(
                    intent.IntentId),
                intent.UnregistrationCommandId);
            Assert.Null(
                intent.UnregistrationReplayHorizonFloor);
            Assert.Equal(
                schemaVersion < 5,
                intent.UnregistrationMutationMayHaveStarted);
            Assert.Empty(restored.RuntimeDeletionRetryAudit!);
            Assert.Empty(
                restored.RuntimeDeletionReconciliationAudit!);
            Assert.Null(
                restored.OrchestraDeleteCheckpointMonitor);
            Assert.Empty(
                restored.OrchestraDeleteCheckpointAlertOutbox!);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsSchemaFiveIntentWithoutCommandIdentity()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var preparedAt = DateTimeOffset.UtcNow.AddMinutes(-1);
            var schemaFive = new PersistedControlPlaneState(
                5,
                preparedAt,
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                Array.Empty<OrchestraRunSummary>(),
                new[]
                {
                    new PersistedRuntimeDeletionIntent(
                        "rdel_schema_four_missing_command",
                        new[] { "runtime-schema-four" },
                        preparedAt),
                });
            File.WriteAllText(
                statePath,
                JsonSerializer.Serialize(
                    schemaFive,
                    new LeserpentJsonContext(new JsonSerializerOptions())
                        .PersistedControlPlaneState));

            var store = CreateStateStore(statePath);
            Assert.Null(store.Load());
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsForgedCheckpointAlertAcknowledgement()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var now = DateTimeOffset.UtcNow;
            var monitor =
                new PersistedOrchestraDeleteCheckpointMonitor(
                    new OrchestraDeleteReplayHorizon(
                        4096,
                        4000,
                        1,
                        4000,
                        4001,
                        0,
                        1,
                        3990),
                    OrchestraDeleteReplayAdmissionPressure.Critical,
                    1,
                    now,
                    now.AddSeconds(1),
                    now.AddSeconds(-1),
                    "orchestra_checkpoint_unavailable",
                    1,
                    now,
                    AcknowledgedAlertGeneration: 2,
                    AcknowledgedBy: "operator-a",
                    AcknowledgedAt: now);
            var error = Assert.Throws<
                ControlPlaneStatePersistenceException>(() =>
                CreateStateStore(statePath).SaveStrict(
                    Array.Empty<PersistedRuntimeState>(),
                    Array.Empty<PersistedSessionState>(),
                    orchestraDeleteCheckpointMonitor: monitor));

            Assert.IsType<InvalidDataException>(
                error.InnerException);
            Assert.False(File.Exists(statePath));
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsForgedCheckpointAlertOutboxGeneration()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var now = DateTimeOffset.UtcNow;
            var monitor =
                new PersistedOrchestraDeleteCheckpointMonitor(
                    new OrchestraDeleteReplayHorizon(
                        4096,
                        4000,
                        1,
                        4000,
                        4001,
                        0,
                        1,
                        3990),
                    OrchestraDeleteReplayAdmissionPressure.Critical,
                    1,
                    now,
                    now.AddSeconds(1),
                    now.AddSeconds(-1),
                    "orchestra_checkpoint_unavailable",
                    1,
                    now,
                    AcknowledgedAlertGeneration: null,
                    AcknowledgedBy: null,
                    AcknowledgedAt: null);
            var forged = new[]
            {
                new PersistedOrchestraDeleteCheckpointAlertDelivery(
                    "orchestra-checkpoint-alert-2",
                    2,
                    now,
                    OrchestraDeleteReplayAdmissionPressure.Critical,
                    1,
                    "orchestra_checkpoint_unavailable",
                    now,
                    AttemptCount: 0,
                    LastAttemptAt: null,
                    NextAttemptAt: null,
                    LastDeliveryFailureCode: null),
            };
            var error = Assert.Throws<
                ControlPlaneStatePersistenceException>(() =>
                CreateStateStore(statePath).SaveStrict(
                    Array.Empty<PersistedRuntimeState>(),
                    Array.Empty<PersistedSessionState>(),
                    orchestraDeleteCheckpointMonitor: monitor,
                    orchestraDeleteCheckpointAlertOutbox:
                        forged));

            Assert.IsType<InvalidDataException>(
                error.InnerException);
            Assert.False(File.Exists(statePath));
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreReportsTypedCleanAndEmptyLoadProvenance()
    {
        var emptyPath = TemporaryPath("json");
        var statePath = TemporaryPath("json");
        try
        {
            var emptyStore = CreateStateStore(emptyPath);
            Assert.Null(emptyStore.Load());
            Assert.Equal(
                new ControlPlaneStateLoadProvenance(
                    ControlPlaneStateLoadSource.Empty,
                    ControlPlaneStateLoadOutcome.Empty,
                    Degraded: false,
                    ControlPlaneStateLoadFailureCode.NotFound,
                    ControlPlaneStateLoadFailureCode.NotFound),
                emptyStore.LoadProvenance);

            var writer = CreateStateStore(statePath);
            writer.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>());
            var reader = CreateStateStore(statePath);
            Assert.NotNull(reader.Load());
            Assert.Equal(
                new ControlPlaneStateLoadProvenance(
                    ControlPlaneStateLoadSource.Primary,
                    ControlPlaneStateLoadOutcome.Clean,
                    Degraded: false,
                    PrimaryFailureCode: null,
                    BackupFailureCode: null),
                reader.LoadProvenance);
        }
        finally
        {
            DeleteState(emptyPath);
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreReportsSecretFreeBackupRecoveryProvenance()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var writer = CreateStateStore(statePath);
            writer.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>());
            writer.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>());
            File.WriteAllText(statePath, "{");

            var reader = CreateStateStore(statePath);
            Assert.NotNull(reader.Load());
            Assert.Equal(
                new ControlPlaneStateLoadProvenance(
                    ControlPlaneStateLoadSource.Backup,
                    ControlPlaneStateLoadOutcome.Recovered,
                    Degraded: true,
                    ControlPlaneStateLoadFailureCode.InvalidJson,
                    BackupFailureCode: null),
                reader.LoadProvenance);
            Assert.Null(reader.LastSaveError);
            var serialized = JsonSerializer.Serialize(
                reader.LoadProvenance,
                LeserpentJsonContext.Default
                    .ControlPlaneStateLoadProvenance);
            Assert.Equal(
                "{\"source\":\"backup\",\"outcome\":\"recovered\",\"degraded\":true,\"primaryFailureCode\":\"invalid_json\",\"backupFailureCode\":null}",
                serialized);
            Assert.DoesNotContain(statePath, serialized, StringComparison.Ordinal);
            var configuration = new ConfigurationBuilder().Build();
            var posture = Program.BuildRuntimePosture(
                reader,
                new InMemoryOrchestraRunStore(),
                new RustCompatibilityBridge(
                    configuration,
                    NullLogger<RustCompatibilityBridge>.Instance),
                new DaemonDeploymentAuthority(configuration));
            Assert.True(posture.PersistenceReady);
            Assert.True(posture.DegradedButOperable);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreReportsTypedFailureWhenBothGenerationsAreCorrupt()
    {
        var statePath = TemporaryPath("json");
        try
        {
            File.WriteAllText(statePath, "{");
            File.WriteAllText($"{statePath}.bak", "{");

            var reader = CreateStateStore(statePath);
            Assert.Null(reader.Load());
            Assert.Equal(
                new ControlPlaneStateLoadProvenance(
                    ControlPlaneStateLoadSource.None,
                    ControlPlaneStateLoadOutcome.Failed,
                    Degraded: true,
                    ControlPlaneStateLoadFailureCode.InvalidJson,
                    ControlPlaneStateLoadFailureCode.InvalidJson),
                reader.LoadProvenance);
            Assert.Equal(
                "control_plane_state_backup_load_failed",
                reader.LastSaveError);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreDoesNotPoisonGoodBackupOnFirstPostRecoverySave()
    {
        var statePath = TemporaryPath("json");
        var backupPath = $"{statePath}.bak";
        try
        {
            var writer = CreateStateStore(statePath);
            var runtimes = new[] { CreateRuntimeState() };
            writer.SaveStrict(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { CreateRun("baseline-run", "baseline-request", "succeeded") });
            writer.SaveStrict(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { CreateRun("baseline-run", "baseline-request", "succeeded") });
            var knownGoodBackup = File.ReadAllBytes(backupPath);
            File.WriteAllText(statePath, "{");

            var recovered = CreateStateStore(statePath);
            Assert.NotNull(recovered.Load());
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                recovered.LoadProvenance.Source);
            recovered.SaveStrict(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { CreateRun("repaired-run", "repaired-request", "succeeded") });

            Assert.Equal(knownGoodBackup, File.ReadAllBytes(backupPath));
            var repaired = Assert.IsType<PersistedControlPlaneState>(
                CreateStateStore(statePath).Load());
            Assert.Equal(
                "repaired-run",
                Assert.Single(repaired.OrchestraRuns!).RunId);
            var preserved = Assert.IsType<PersistedControlPlaneState>(
                CreateStateStore(backupPath).Load());
            Assert.Equal(
                "baseline-run",
                Assert.Single(preserved.OrchestraRuns!).RunId);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsSemanticInvalidPrimaryWithoutPromotingIt()
    {
        var statePath = TemporaryPath("json");
        var backupPath = $"{statePath}.bak";
        try
        {
            var writer = CreateStateStore(statePath);
            var runtimes = new[] { CreateRuntimeState() };
            writer.SaveStrict(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { CreateRun("baseline-run", "baseline-request", "succeeded") });
            writer.SaveStrict(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { CreateRun("baseline-run", "baseline-request", "succeeded") });
            var knownGoodBackup = File.ReadAllBytes(backupPath);
            WriteSemanticInvalidState(statePath);

            var recovered = CreateStateStore(statePath);
            var loaded = Assert.IsType<PersistedControlPlaneState>(
                recovered.Load());
            Assert.Equal(
                "baseline-run",
                Assert.Single(loaded.OrchestraRuns!).RunId);
            Assert.Equal(
                new ControlPlaneStateLoadProvenance(
                    ControlPlaneStateLoadSource.Backup,
                    ControlPlaneStateLoadOutcome.Recovered,
                    Degraded: true,
                    ControlPlaneStateLoadFailureCode.SemanticInvalid,
                    BackupFailureCode: null),
                recovered.LoadProvenance);

            recovered.SaveStrict(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { CreateRun("repaired-run", "repaired-request", "succeeded") });
            Assert.Equal(knownGoodBackup, File.ReadAllBytes(backupPath));
            var repaired = Assert.IsType<PersistedControlPlaneState>(
                CreateStateStore(statePath).Load());
            Assert.Equal(
                "repaired-run",
                Assert.Single(repaired.OrchestraRuns!).RunId);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreFailsWhenBothGenerationsAreSemanticInvalid()
    {
        var statePath = TemporaryPath("json");
        try
        {
            WriteSemanticInvalidState(statePath);
            WriteSemanticInvalidState($"{statePath}.bak");

            var reader = CreateStateStore(statePath);
            Assert.Null(reader.Load());
            Assert.Equal(
                new ControlPlaneStateLoadProvenance(
                    ControlPlaneStateLoadSource.None,
                    ControlPlaneStateLoadOutcome.Failed,
                    Degraded: true,
                    ControlPlaneStateLoadFailureCode.SemanticInvalid,
                    ControlPlaneStateLoadFailureCode.SemanticInvalid),
                reader.LoadProvenance);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsDuplicateRuntimeIdentities()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var duplicateRuntimes = valid.Runtimes
                .Append(valid.Runtimes[0] with
                {
                    RuntimeId =
                        valid.Runtimes[0].RuntimeId.ToUpperInvariant(),
                })
                .ToArray();
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    Runtimes = duplicateRuntimes,
                });
            var knownGoodBackup = File.ReadAllBytes($"{statePath}.bak");

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.Runtimes);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
            Assert.Throws<ControlPlaneStatePersistenceException>(() =>
                store.SaveStrict(
                    duplicateRuntimes,
                    valid.Sessions));
            Assert.Equal(
                knownGoodBackup,
                File.ReadAllBytes($"{statePath}.bak"));
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsDuplicateSessionIdentities()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    Sessions = valid.Sessions
                        .Append(valid.Sessions[0] with
                        {
                            SessionId =
                                valid.Sessions[0].SessionId.ToUpperInvariant(),
                        })
                        .ToArray(),
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.Sessions);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void RegistryImportRejectsOrphanSessionBeforeReplacingProjection()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var registry = new RegistryService(CreateStateStore(statePath));
            var runtime = registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "existing-runtime",
                    "http://127.0.0.1:49152",
                    "pairing-token"),
                "runtime-existing");
            var imported = CreateRuntimeSessionState();
            imported = imported with
            {
                Sessions = new[]
                {
                    imported.Sessions[0] with
                    {
                        RuntimeId = "runtime-missing",
                    },
                },
            };

            Assert.Throws<InvalidDataException>(() =>
                registry.ImportState(imported));
            Assert.Equal(
                "existing-runtime",
                registry.GetRuntime(runtime.RuntimeId)?.Name);
            Assert.Single(registry.ListRuntimes());
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsDuplicateLegacyOrchestraRunIdentities()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var run = Assert.Single(valid.OrchestraRuns!);
            var duplicateRuns = valid.OrchestraRuns!
                .Append(run with
                {
                    RunId = run.RunId.ToUpperInvariant(),
                })
                .ToArray();
            WriteState($"{statePath}.bak", valid);
            var knownGoodBackup = File.ReadAllBytes($"{statePath}.bak");
            WriteState(
                statePath,
                valid with
                {
                    OrchestraRuns = duplicateRuns,
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.OrchestraRuns!);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
            Assert.Throws<ControlPlaneStatePersistenceException>(() =>
                store.SaveStrict(
                    valid.Runtimes,
                    valid.Sessions,
                    duplicateRuns));
            Assert.Equal(
                knownGoodBackup,
                File.ReadAllBytes($"{statePath}.bak"));
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void RegistryImportRejectsOrphanLegacyOrchestraRunBeforeReplacingProjection()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var registry = new RegistryService(CreateStateStore(statePath));
            var runtime = registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "existing-runtime",
                    "http://127.0.0.1:49154",
                    "pairing-token"),
                "runtime-existing");
            var imported = CreateRuntimeSessionState();
            imported = imported with
            {
                OrchestraRuns = new[]
                {
                    Assert.Single(imported.OrchestraRuns!) with
                    {
                        RuntimeId = "runtime-missing",
                    },
                },
            };

            Assert.Throws<InvalidDataException>(() =>
                registry.ImportState(imported));
            Assert.Equal(
                "existing-runtime",
                registry.GetRuntime(runtime.RuntimeId)?.Name);
            Assert.Single(registry.ListRuntimes());
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsDuplicatePerRuntimeOrchestraRequestIds()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var run = Assert.Single(valid.OrchestraRuns!);
            var duplicateRequests = valid.OrchestraRuns!
                .Append(run with
                {
                    RunId = "orun_duplicate_request",
                })
                .ToArray();
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    OrchestraRuns = duplicateRequests,
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.OrchestraRuns!);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Throws<ControlPlaneStatePersistenceException>(() =>
                store.SaveStrict(
                    valid.Runtimes,
                    valid.Sessions,
                    duplicateRequests));
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreRejectsInvalidRetainedOrchestraRetryLineage()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var parent = Assert.Single(valid.OrchestraRuns!);
            var invalidRetry = parent with
            {
                RunId = "orun_invalid_retry",
                Attempt = 3,
                RetriedFromRunId = parent.RunId,
                RequestId = "request-invalid-retry",
                ExecutedAt = parent.ExecutedAt.AddSeconds(1),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    OrchestraRuns = new[] { parent, invalidRetry },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.OrchestraRuns!);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStoreAllowsCrossRuntimeRequestReuseAndEvictedRetryParent()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var firstRuntime = Assert.Single(valid.Runtimes);
            var firstRun = Assert.Single(valid.OrchestraRuns!);
            var secondRuntime = firstRuntime with
            {
                RuntimeId = "runtime-secondary",
                Name = "secondary runtime",
                Endpoint = "http://127.0.0.1:49156",
            };
            var retainedRetry = firstRun with
            {
                RunId = "orun_retained_without_parent",
                RuntimeId = secondRuntime.RuntimeId,
                Attempt = 2,
                RetriedFromRunId = "orun_evicted_parent",
            };
            var store = CreateStateStore(statePath);

            store.SaveStrict(
                new[] { firstRuntime, secondRuntime },
                valid.Sessions,
                new[] { firstRun, retainedRetry });
            var restored = Assert.IsType<PersistedControlPlaneState>(
                CreateStateStore(statePath).Load());

            Assert.Equal(2, restored.OrchestraRuns!.Count);
            Assert.Equal(
                firstRun.RequestId,
                restored.OrchestraRuns!
                    .Single(run =>
                        run.RuntimeId == secondRuntime.RuntimeId)
                    .RequestId);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData("unknown_outcome")]
    [InlineData("active_completion")]
    [InlineData("reversed_completion")]
    [InlineData("future_execution")]
    public void ControlPlaneStateStoreRejectsInvalidOrchestraLifecycleMetadata(
        string invalidKind)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var run = Assert.Single(valid.OrchestraRuns!);
            var invalid = invalidKind switch
            {
                "unknown_outcome" => run with
                {
                    Outcome = "mystery",
                },
                "active_completion" => run with
                {
                    Outcome = "running",
                },
                "reversed_completion" => run with
                {
                    CompletedAt = run.ExecutedAt.AddSeconds(-1),
                },
                "future_execution" => run with
                {
                    ExecutedAt = DateTimeOffset.UtcNow.AddMinutes(6),
                    CompletedAt = DateTimeOffset.UtcNow.AddMinutes(6),
                },
                _ => throw new InvalidOperationException(
                    $"unknown invalid lifecycle kind {invalidKind}"),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    OrchestraRuns = new[] { invalid },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.OrchestraRuns!);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData("null_steps")]
    [InlineData("too_many_steps")]
    [InlineData("invalid_step")]
    [InlineData("oversized_summary")]
    [InlineData("control_summary")]
    public void ControlPlaneStateStoreRejectsInvalidOrchestraStepPayload(
        string invalidKind)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var run = Assert.Single(valid.OrchestraRuns!);
            var invalid = invalidKind switch
            {
                "null_steps" => run with
                {
                    Steps = null!,
                },
                "too_many_steps" => run with
                {
                    Steps = Enumerable.Range(
                            0,
                            ControlPlaneStateValidator
                                .MaxOrchestraRunSteps + 1)
                        .Select(index =>
                            new OrchestraExecutionStepResult(
                                $"step-{index}",
                                "ok",
                                "bounded step"))
                        .ToArray(),
                },
                "invalid_step" => run with
                {
                    Steps = new[]
                    {
                        new OrchestraExecutionStepResult(
                            " invalid-step ",
                            "ok",
                            "invalid identity"),
                    },
                },
                "oversized_summary" => run with
                {
                    Steps = new[]
                    {
                        new OrchestraExecutionStepResult(
                            "bounded-step",
                            "ok",
                            new string('x', 1_025)),
                    },
                },
                "control_summary" => run with
                {
                    Steps = new[]
                    {
                        new OrchestraExecutionStepResult(
                            "bounded-step",
                            "ok",
                            "header\ncredential=secret"),
                    },
                },
                _ => throw new InvalidOperationException(
                    $"unknown invalid step kind {invalidKind}"),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    OrchestraRuns = new[] { invalid },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.OrchestraRuns!);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData("blank_approved_by")]
    [InlineData("oversized_approval_note")]
    [InlineData("invalid_plan_revision")]
    [InlineData("excessive_attempt")]
    public void ControlPlaneStateStoreRejectsInvalidOrchestraOperatorMetadata(
        string invalidKind)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var run = Assert.Single(valid.OrchestraRuns!);
            var invalid = invalidKind switch
            {
                "blank_approved_by" => run with
                {
                    ApprovedBy = " ",
                },
                "oversized_approval_note" => run with
                {
                    ApprovalNote = new string('x', 1_025),
                },
                "invalid_plan_revision" => run with
                {
                    PlanRevision = " revision ",
                },
                "excessive_attempt" => run with
                {
                    Attempt =
                        ControlPlaneStateValidator
                            .MaxOrchestraRunAttempts + 1,
                    RetriedFromRunId = "orun_evicted_parent",
                },
                _ => throw new InvalidOperationException(
                    $"unknown invalid operator metadata kind {invalidKind}"),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    OrchestraRuns = new[] { invalid },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.OrchestraRuns!);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData("mismatched_runtime")]
    [InlineData("mismatched_outcome")]
    [InlineData("reversed_event_time")]
    [InlineData("unsafe_event_summary")]
    [InlineData("oversized_approval")]
    public void OrchestraStoresRejectInvalidRunEventEnvelopesAtomically(
        string invalidKind)
    {
        var databasePath = TemporaryPath("db");
        try
        {
            var run = CreateRun(
                "run-invalid-envelope",
                "request-invalid-envelope",
                "succeeded");
            var eventRecord = CreateEvent(
                run,
                null,
                "succeeded");
            var invalidRun = invalidKind == "oversized_approval"
                ? run with
                {
                    ApprovedBy = new string('x', 257),
                }
                : run;
            var invalidEvent = invalidKind switch
            {
                "mismatched_runtime" => eventRecord with
                {
                    RuntimeId = "runtime-other",
                },
                "mismatched_outcome" => eventRecord with
                {
                    ToOutcome = "failed",
                },
                "reversed_event_time" => eventRecord with
                {
                    RecordedAt = run.ExecutedAt.AddSeconds(-1),
                },
                "unsafe_event_summary" => eventRecord with
                {
                    Summary = "header\ncredential=secret",
                },
                "oversized_approval" => eventRecord,
                _ => throw new InvalidOperationException(
                    $"unknown invalid envelope kind {invalidKind}"),
            };
            var sqlite = CreateSqliteStore(databasePath);
            var memory = new InMemoryOrchestraRunStore();

            Assert.False(sqlite.Upsert(invalidRun, invalidEvent));
            Assert.Equal(
                "orchestra_store_operation_failed",
                sqlite.LastError);
            Assert.Empty(sqlite.LoadAll());
            Assert.False(memory.Upsert(invalidRun, invalidEvent));
            Assert.Empty(memory.LoadAll());
        }
        finally
        {
            DeleteDatabase(databasePath);
        }
    }

    [Fact]
    public void RegistryFailsClosedBeforeMigratingWhenOrchestraReadFails()
    {
        var statePath = TemporaryPath("json");
        try
        {
            WriteState(statePath, CreateRuntimeSessionState());
            var store = new FailingReadStore();

            Assert.Throws<OrchestraPersistenceException>(() =>
                new RegistryService(
                    CreateStateStore(statePath),
                    store));
            Assert.False(store.ReplaceAttempted);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void RegistryBackfillsLegacyEventOriginBeforeRestartRecovery()
    {
        var statePath = TemporaryPath("json");
        var databasePath = TemporaryPath("db");
        try
        {
            var registry = new RegistryService(
                CreateStateStore(statePath));
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "runtime",
                    "http://127.0.0.1:49157",
                    "pairing-token"),
                "runtime-1");
            CreateVersionOneDatabase(databasePath);

            var restored = new RegistryService(
                CreateStateStore(statePath),
                CreateSqliteStore(databasePath));
            var run = Assert.Single(
                restored.ListOrchestraRuns("runtime-1"));
            var events = restored.ListOrchestraRunEvents(
                "runtime-1",
                run.RunId);

            Assert.Equal("failed", run.Outcome);
            Assert.Equal(2, events.Count);
            Assert.Equal("legacy_import", events[0].EventType);
            Assert.Null(events[0].FromOutcome);
            Assert.Equal("queued", events[0].ToOutcome);
            Assert.Equal(
                "service_restart_recovery",
                events[1].EventType);
            Assert.Equal("queued", events[1].FromOutcome);
            Assert.Equal("failed", events[1].ToOutcome);
            Assert.True(
                events[1].EventId > events[0].EventId);
        }
        finally
        {
            DeleteState(statePath);
            DeleteDatabase(databasePath);
        }
    }

    [Theory]
    [InlineData("broken_from")]
    [InlineData("reversed_time")]
    [InlineData("non_monotonic_id")]
    public void SqliteStoreRejectsCorruptedEventSequenceOnRead(
        string invalidKind)
    {
        var databasePath = TemporaryPath("db");
        try
        {
            var store = CreateSqliteStore(databasePath);
            var queued = CreateRun(
                "run-corrupt-sequence",
                "request-corrupt-sequence",
                "queued");
            var running = queued with
            {
                Outcome = "running",
            };
            Assert.True(store.Upsert(
                queued,
                CreateEvent(queued, null, "queued")));
            Assert.True(store.Upsert(
                running,
                CreateEvent(
                    running,
                    "queued",
                    "running")));

            using (var connection = new SqliteConnection(
                $"Data Source={databasePath}"))
            {
                connection.Open();
                using var command = connection.CreateCommand();
                command.CommandText = invalidKind switch
                {
                    "broken_from" => """
                        UPDATE orchestra_run_events
                        SET from_outcome = 'failed'
                        WHERE from_outcome = 'queued';
                        """,
                    "reversed_time" => """
                        UPDATE orchestra_run_events
                        SET recorded_at = '2020-01-01T00:00:00Z'
                        WHERE from_outcome = 'queued';
                        """,
                    "non_monotonic_id" => """
                        UPDATE orchestra_run_events
                        SET event_id = 0
                        WHERE from_outcome = 'queued';
                        """,
                    _ => throw new InvalidOperationException(
                        $"unknown event corruption kind {invalidKind}"),
                };
                command.ExecuteNonQuery();
            }

            Assert.Empty(store.LoadEvents(
                running.RuntimeId,
                running.RunId));
            Assert.Equal(
                "orchestra_store_operation_failed",
                store.LastError);
        }
        finally
        {
            DeleteDatabase(databasePath);
        }
    }

    [Fact]
    public void InMemoryStoreRejectsIllegalEventTransition()
    {
        var store = new InMemoryOrchestraRunStore();
        var queued = CreateRun(
            "run-memory-sequence",
            "request-memory-sequence",
            "queued");
        var running = queued with
        {
            Outcome = "running",
        };

        Assert.True(store.Upsert(
            queued,
            CreateEvent(queued, null, "queued")));
        Assert.False(store.Upsert(
            running,
            CreateEvent(
                running,
                "failed",
                "running")));
        Assert.Equal(
            "queued",
            Assert.Single(store.LoadAll()).Outcome);
        Assert.Single(store.LoadEvents(
            queued.RuntimeId,
            queued.RunId));
    }

    [Fact]
    public void EventSequenceRejectsTerminalRunMismatch()
    {
        var succeeded = CreateRun(
            "run-terminal-mismatch",
            "request-terminal-mismatch",
            "succeeded");
        var failed = succeeded with
        {
            Outcome = "failed",
        };
        var persistedEvent = CreateEvent(
            succeeded,
            null,
            "succeeded") with
        {
            EventId = 1,
        };

        Assert.Throws<InvalidDataException>(() =>
            ControlPlaneStateValidator
                .ValidateOrchestraEventSequence(
                    failed,
                    new[] { persistedEvent },
                    failed.RuntimeId,
                    failed.RunId));
    }

    [Fact]
    public void ControlPlaneStateStoreAllowsLegacyTerminalWithoutCompletionTimestamp()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var legacyRun = Assert.Single(valid.OrchestraRuns!) with
            {
                CompletedAt = null,
            };
            var store = CreateStateStore(statePath);

            store.SaveStrict(
                valid.Runtimes,
                valid.Sessions,
                new[] { legacyRun });
            var restored = Assert.IsType<PersistedControlPlaneState>(
                CreateStateStore(statePath).Load());

            Assert.Null(Assert.Single(restored.OrchestraRuns!).CompletedAt);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData("blank_name")]
    [InlineData("reversed_timestamp")]
    [InlineData("null_capabilities")]
    [InlineData("too_many_capabilities")]
    [InlineData("duplicate_capabilities")]
    [InlineData("null_tags")]
    [InlineData("null_status")]
    [InlineData("negative_status_count")]
    public void ControlPlaneStateStoreRejectsInvalidRuntimePayload(
        string invalidKind)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var runtime = Assert.Single(valid.Runtimes);
            var capability = new RuntimeCapability(
                "capture",
                "fully_supported",
                "capture traffic");
            var invalid = invalidKind switch
            {
                "blank_name" => runtime with
                {
                    Name = " ",
                },
                "reversed_timestamp" => runtime with
                {
                    UpdatedAt = runtime.RegisteredAt.AddSeconds(-1),
                },
                "null_capabilities" => runtime with
                {
                    Capabilities = null!,
                },
                "too_many_capabilities" => runtime with
                {
                    Capabilities = Enumerable.Range(
                            0,
                            ControlPlaneStateValidator
                                .MaxRuntimeCapabilities + 1)
                        .Select(index =>
                            capability with
                            {
                                Key = $"capability-{index}",
                            })
                        .ToArray(),
                },
                "duplicate_capabilities" => runtime with
                {
                    Capabilities = new[]
                    {
                        capability,
                        capability with
                        {
                            Key = capability.Key.ToUpperInvariant(),
                        },
                    },
                },
                "null_tags" => runtime with
                {
                    Tags = null!,
                },
                "null_status" => runtime with
                {
                    Status = null!,
                },
                "negative_status_count" => runtime with
                {
                    Status = runtime.Status with
                    {
                        TargetCount = -1,
                    },
                },
                _ => throw new InvalidOperationException(
                    $"unknown invalid runtime payload kind {invalidKind}"),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    Runtimes = new[] { invalid },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.Runtimes);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData("blank_pipeline")]
    [InlineData("unknown_status")]
    [InlineData("reversed_timestamp")]
    [InlineData("null_requirements")]
    [InlineData("too_many_requirements")]
    [InlineData("duplicate_requirements")]
    public void ControlPlaneStateStoreRejectsInvalidSessionPayload(
        string invalidKind)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var session = Assert.Single(valid.Sessions);
            var requirement = new SessionCapabilityRequirement(
                "capture",
                "fully_supported");
            var invalid = invalidKind switch
            {
                "blank_pipeline" => session with
                {
                    PipelineKind = " ",
                },
                "unknown_status" => session with
                {
                    Status = "unknown",
                },
                "reversed_timestamp" => session with
                {
                    UpdatedAt = session.CreatedAt.AddSeconds(-1),
                },
                "null_requirements" => session with
                {
                    Requirements = null!,
                },
                "too_many_requirements" => session with
                {
                    Requirements = Enumerable.Range(
                            0,
                            ControlPlaneStateValidator
                                .MaxSessionRequirements + 1)
                        .Select(index =>
                            requirement with
                            {
                                Key = $"requirement-{index}",
                            })
                        .ToArray(),
                },
                "duplicate_requirements" => session with
                {
                    Requirements = new[]
                    {
                        requirement,
                        requirement with
                        {
                            Key = requirement.Key.ToUpperInvariant(),
                        },
                    },
                },
                _ => throw new InvalidOperationException(
                    $"unknown invalid session payload kind {invalidKind}"),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    Sessions = new[] { invalid },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Single(restored.Sessions);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Theory]
    [InlineData("null_slots")]
    [InlineData("duplicate_slots")]
    [InlineData("negative_counts")]
    public void ControlPlaneStateStoreRejectsInvalidSidecarMemoryPayload(
        string invalidKind)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var runtime = Assert.Single(valid.Runtimes);
            var slot = new RuntimeSidecarMemorySlotSummary(
                "slot-a",
                "baseline",
                null,
                "manual",
                DateTimeOffset.UtcNow.AddMinutes(-1),
                3,
                2);
            var memory = new RuntimeSidecarMemorySnapshot(
                true,
                1,
                2,
                slot.Slot,
                slot.Label,
                slot.Source,
                new[] { slot });
            var invalidMemory = invalidKind switch
            {
                "null_slots" => memory with
                {
                    Slots = null!,
                },
                "duplicate_slots" => memory with
                {
                    Slots = new[]
                    {
                        slot,
                        slot with
                        {
                            Slot = slot.Slot.ToUpperInvariant(),
                        },
                    },
                },
                "negative_counts" => memory with
                {
                    HistoryCount = -1,
                },
                _ => throw new InvalidOperationException(
                    $"unknown invalid sidecar payload kind {invalidKind}"),
            };
            var invalidRuntime = runtime with
            {
                SidecarStatus = new RuntimeSidecarStatusSnapshot(
                    "etragon-api",
                    DateTimeOffset.UtcNow,
                    null,
                    true,
                    "ready",
                    1,
                    false,
                    0,
                    false,
                    false,
                    Memory: invalidMemory),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    Runtimes = new[] { invalidRuntime },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Null(Assert.Single(restored.Runtimes).SidecarStatus);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void DiscoveryFailuresExposeOnlyStableDiagnosticCodes()
    {
        const string secret =
            "upstream failure credential=secret-token";

        var capability = CapabilityDiscoveryResult.Failed(
            "http://runtime.test/v1/capabilities",
            secret);
        var status = RuntimeStatusDiscoveryResult.Failed(
            "http://runtime.test/v1/latest/meta",
            secret);
        var sidecar = RuntimeSidecarDiscoveryResult.Failed(
            "http://sidecar.test/v1/latest/status",
            secret);

        Assert.Equal(
            "capability_fetch_failed",
            capability.CapabilityFetchError);
        Assert.Equal(
            "runtime_status_fetch_failed",
            status.Status.StatusFetchError);
        Assert.Equal(
            "sidecar_fetch_failed",
            sidecar.SidecarStatus?.StatusFetchError);
        Assert.Equal(
            "sidecar_fetch_failed",
            sidecar.SidecarStatus?.LastError);
        Assert.DoesNotContain(
            "secret-token",
            JsonSerializer.Serialize(
                new
                {
                    capability,
                    status,
                    sidecar,
                }),
            StringComparison.Ordinal);
    }

    [Theory]
    [InlineData("raw_capability_error")]
    [InlineData("incoherent_runtime_status")]
    [InlineData("raw_runtime_status_error")]
    [InlineData("raw_sidecar_error")]
    [InlineData("incoherent_sidecar_status")]
    [InlineData("raw_memory_error")]
    [InlineData("oversized_resilience_summary")]
    public void ControlPlaneStateStoreRejectsUnsafeRuntimeDiagnostics(
        string invalidKind)
    {
        var statePath = TemporaryPath("json");
        try
        {
            var valid = CreateRuntimeSessionState();
            var runtime = Assert.Single(valid.Runtimes);
            var invalid = invalidKind switch
            {
                "raw_capability_error" => runtime with
                {
                    CapabilityFetchError =
                        "credential=secret-token",
                },
                "incoherent_runtime_status" => runtime with
                {
                    Status = runtime.Status with
                    {
                        StatusSource = "gewyvern-api",
                    },
                },
                "raw_runtime_status_error" => runtime with
                {
                    Status = runtime.Status with
                    {
                        StatusSource = "fetch_failed",
                        StatusFetchError =
                            "upstream status secret-token",
                    },
                },
                "raw_sidecar_error" => runtime with
                {
                    SidecarStatus =
                        new RuntimeSidecarStatusSnapshot(
                            "fetch_failed",
                            null,
                            "upstream sidecar secret-token",
                            false,
                            "fetch_failed",
                            null,
                            false,
                            0,
                            false,
                            false,
                            "upstream sidecar secret-token"),
                },
                "incoherent_sidecar_status" => runtime with
                {
                    SidecarStatus =
                        new RuntimeSidecarStatusSnapshot(
                            "etragon-api",
                            null,
                            null,
                            true,
                            "ready",
                            1,
                            false,
                            0,
                            false,
                            false),
                },
                "raw_memory_error" => runtime with
                {
                    SidecarStatus =
                        new RuntimeSidecarStatusSnapshot(
                            "etragon-api",
                            DateTimeOffset.UtcNow,
                            null,
                            true,
                            "ready",
                            1,
                            false,
                            0,
                            false,
                            false,
                            Memory:
                                new RuntimeSidecarMemorySnapshot(
                                    false,
                                    0,
                                    0,
                                    null,
                                    null,
                                    null,
                                    Array.Empty<
                                        RuntimeSidecarMemorySlotSummary>(),
                                    "memory secret-token")),
                },
                "oversized_resilience_summary" => runtime with
                {
                    Status = runtime.Status with
                    {
                        ResilienceSummary = new string('x', 1_025),
                    },
                },
                _ => throw new InvalidOperationException(
                    $"unknown unsafe diagnostic kind {invalidKind}"),
            };
            WriteState($"{statePath}.bak", valid);
            WriteState(
                statePath,
                valid with
                {
                    Runtimes = new[] { invalid },
                });

            var store = CreateStateStore(statePath);
            var restored = Assert.IsType<PersistedControlPlaneState>(
                store.Load());

            Assert.Null(
                Assert.Single(restored.Runtimes)
                    .CapabilityFetchError);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                store.LoadProvenance.Source);
            Assert.DoesNotContain(
                "secret-token",
                store.LastSaveError ?? string.Empty,
                StringComparison.Ordinal);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    [Fact]
    public void ControlPlaneStateStorePreservesSnapshotAndCleansTempAfterBackupFailure()
    {
        var statePath = TemporaryPath("json");
        var backupPath = $"{statePath}.bak";
        var store = CreateStateStore(statePath);
        var runtimes = new[] { CreateRuntimeState() };
        var original = CreateRun("original-run", "original-request", "succeeded");

        try
        {
            store.Save(
                runtimes,
                Array.Empty<PersistedSessionState>(),
                new[] { original });
            Directory.CreateDirectory(backupPath);

            Assert.Throws<ControlPlaneStatePersistenceException>(() =>
                store.SaveStrict(
                    runtimes,
                    Array.Empty<PersistedSessionState>(),
                    new[] { CreateRun("replacement-run", "replacement-request", "succeeded") }));

            Assert.True(store.IsDirty);
            Assert.Equal(
                "control_plane_state_save_failed",
                store.LastSaveError);
            Assert.Empty(Directory.EnumerateFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}.*.tmp"));

            var restored = Assert.IsType<PersistedControlPlaneState>(
                CreateStateStore(statePath).Load());
            var restoredRuns = Assert.IsAssignableFrom<IReadOnlyList<OrchestraRunSummary>>(
                restored.OrchestraRuns);
            Assert.Equal("original-run", Assert.Single(restoredRuns).RunId);
        }
        finally
        {
            if (Directory.Exists(backupPath))
            {
                Directory.Delete(backupPath, recursive: true);
            }
            DeleteState(statePath);
        }
    }

    private static void WriteSemanticInvalidState(string path)
    {
        var now = DateTimeOffset.UtcNow;
        var state = new PersistedControlPlaneState(
            3,
            now,
            Array.Empty<PersistedRuntimeState>(),
            Array.Empty<PersistedSessionState>(),
            Array.Empty<OrchestraRunSummary>(),
            new[]
            {
                new PersistedRuntimeDeletionIntent(
                    "rdel_semantic_invalid",
                    new[] { "runtime-semantic-invalid" },
                    now.AddSeconds(-2),
                    AttemptCount: 1,
                    LastAttemptAt: now.AddSeconds(-1),
                    NextAttemptAt: now,
                    LastFailureCode:
                        "authority_failure\ncredential=secret",
                    Revision: 2),
            });
        File.WriteAllText(
            path,
            JsonSerializer.Serialize(
                state,
                new LeserpentJsonContext(
                    new JsonSerializerOptions())
                    .PersistedControlPlaneState));
    }

    private static PersistedControlPlaneState CreateRuntimeSessionState()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var registry = new RegistryService(CreateStateStore(statePath));
            var runtime = registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "runtime",
                    "http://127.0.0.1:49153",
                    "pairing-token"),
                "runtime-primary");
            Assert.NotNull(registry.CreateSession(
                new SessionCreateRequest(
                    runtime.RuntimeId,
                    "diagnostic",
                    "operator")).Session);
            registry.RecordOrchestraRun(
                runtime.RuntimeId,
                "runtime_triage",
                "succeeded",
                Array.Empty<OrchestraExecutionStepResult>(),
                "operator",
                "topology validation",
                "revision-1",
                "request-topology-validation");
            return registry.ExportState();
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    private static PersistedRuntimeState CreateRuntimeState()
    {
        var statePath = TemporaryPath("json");
        try
        {
            var registry = new RegistryService(CreateStateStore(statePath));
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "runtime",
                    "http://127.0.0.1:49155",
                    "pairing-token"),
                "runtime-1");
            return Assert.Single(registry.ExportState().Runtimes);
        }
        finally
        {
            DeleteState(statePath);
        }
    }

    private static void WriteState(
        string path,
        PersistedControlPlaneState state)
    {
        File.WriteAllText(
            path,
            JsonSerializer.Serialize(
                state,
                new LeserpentJsonContext(
                    new JsonSerializerOptions())
                    .PersistedControlPlaneState));
    }

    private static OrchestraRunSummary CreateRun(
        string runId,
        string requestId,
        string outcome,
        DateTimeOffset? executedAt = null) =>
        new(
            runId,
            "runtime-1",
            "runtime_triage",
            outcome,
            executedAt ?? DateTimeOffset.UtcNow,
            Array.Empty<OrchestraExecutionStepResult>(),
            RegistryService.IsTerminalOrchestraOutcome(outcome) ? executedAt ?? DateTimeOffset.UtcNow : null,
            1,
            null,
            "operator",
            "test",
            "revision-1",
            requestId);

    private static OrchestraRunEvent CreateEvent(
        OrchestraRunSummary run,
        string? fromOutcome,
        string toOutcome) =>
        new(0, run.RunId, run.RuntimeId, "state_transition", fromOutcome, toOutcome, "test event", DateTimeOffset.UtcNow);

    private static void CreateVersionOneDatabase(string databasePath)
    {
        using var connection = new SqliteConnection($"Data Source={databasePath}");
        connection.Open();
        using var command = connection.CreateCommand();
        command.CommandText = """
            CREATE TABLE orchestra_runs (
                run_id TEXT PRIMARY KEY,
                runtime_id TEXT NOT NULL,
                plan_id TEXT NOT NULL,
                outcome TEXT NOT NULL,
                executed_at TEXT NOT NULL,
                steps_json TEXT NOT NULL,
                completed_at TEXT NULL,
                attempt INTEGER NOT NULL,
                retried_from_run_id TEXT NULL,
                approved_by TEXT NULL,
                approval_note TEXT NULL,
                plan_revision TEXT NULL,
                request_id TEXT NULL
            );
            INSERT INTO orchestra_runs VALUES (
                'legacy-run', 'runtime-1', 'runtime_triage', 'queued',
                '2026-01-01T00:00:00.0000000+00:00', '[]', NULL, 1,
                NULL, 'operator', 'legacy', 'revision-1', 'legacy-request');
            PRAGMA user_version = 1;
            """;
        command.ExecuteNonQuery();
    }

    private static SqliteOrchestraRunStore CreateSqliteStore(string databasePath)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?> { ["LESERPENT_DATABASE_PATH"] = databasePath })
            .Build();
        return new SqliteOrchestraRunStore(
            configuration,
            new TestHostEnvironment { ContentRootPath = Path.GetDirectoryName(databasePath)! },
            NullLogger<SqliteOrchestraRunStore>.Instance);
    }

    private static ControlPlaneStateStore CreateStateStore(string statePath)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?> { ["LESERPENT_STATE_PATH"] = statePath })
            .Build();
        return new ControlPlaneStateStore(
            configuration,
            new TestHostEnvironment { ContentRootPath = Path.GetDirectoryName(statePath)! },
            NullLogger<ControlPlaneStateStore>.Instance);
    }

    private static string TemporaryPath(string extension) =>
        Path.Combine(Path.GetTempPath(), $"leserpent-sqlite-test-{Guid.NewGuid():N}.{extension}");

    private static void DeleteDatabase(string path)
    {
        File.Delete(path);
        File.Delete($"{path}-wal");
        File.Delete($"{path}-shm");
    }

    private static void DeleteState(string path)
    {
        File.Delete(path);
        File.Delete($"{path}.bak");
        File.Delete($"{path}.tmp");
        foreach (var tempPath in Directory.EnumerateFiles(
            Path.GetDirectoryName(path)!,
            $"{Path.GetFileName(path)}.*.tmp"))
        {
            File.Delete(tempPath);
        }
    }

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = string.Empty;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }

    private sealed class FailingMigrationStore : IOrchestraRunStore
    {
        private string? lastError;

        public string Provider => "failure-injection";
        public string Location => "memory";
        public int SchemaVersion => 2;
        public string? LastError => lastError;
        public IReadOnlyList<OrchestraRunSummary> AttemptedRuns { get; private set; } =
            Array.Empty<OrchestraRunSummary>();

        public IReadOnlyList<OrchestraRunSummary> LoadAll()
        {
            lastError = null;
            return Array.Empty<OrchestraRunSummary>();
        }

        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) =>
            Array.Empty<OrchestraRunEvent>();

        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null) => false;

        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs)
        {
            AttemptedRuns = runs.ToArray();
            lastError = "orchestra_store_operation_failed";
            return false;
        }

        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds) => false;
    }

    private sealed class FailingReadStore : IOrchestraRunStore
    {
        public string Provider => "failure-injection";
        public string Location => "memory";
        public int SchemaVersion => 2;
        public string? LastError => "orchestra_store_operation_failed";
        public bool ReplaceAttempted { get; private set; }

        public IReadOnlyList<OrchestraRunSummary> LoadAll() =>
            Array.Empty<OrchestraRunSummary>();

        public IReadOnlyList<OrchestraRunEvent> LoadEvents(
            string runtimeId,
            string runId) =>
            Array.Empty<OrchestraRunEvent>();

        public bool Upsert(
            OrchestraRunSummary run,
            OrchestraRunEvent? eventRecord = null) =>
            false;

        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs)
        {
            ReplaceAttempted = true;
            return false;
        }

        public bool DeleteRuntimes(
            IReadOnlyCollection<string> runtimeIds) =>
            false;
    }
}
