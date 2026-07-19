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
    public void SqliteStoreUpsertsRunsAndEnforcesRequestIdUniqueness()
    {
        var databasePath = TemporaryPath("db");
        var store = CreateSqliteStore(databasePath);
        var queued = CreateRun("run-1", "request-unique-1", "queued");

        store.Upsert(queued, CreateEvent(queued, null, "queued"));
        store.Upsert(
            queued with { Outcome = "succeeded", CompletedAt = DateTimeOffset.UtcNow },
            CreateEvent(queued, "queued", "succeeded"));
        store.Upsert(CreateRun("run-2", "request-unique-1", "queued"));
        var duplicateError = store.LastError;

        var loaded = store.LoadAll();
        Assert.Single(loaded);
        Assert.Equal("succeeded", loaded[0].Outcome);
        var events = store.LoadEvents("runtime-1", "run-1");
        Assert.Equal(2, events.Count);
        Assert.Equal("queued", events[0].ToOutcome);
        Assert.Equal("queued", events[1].FromOutcome);
        Assert.Equal("succeeded", events[1].ToOutcome);
        Assert.True(events[1].EventId > events[0].EventId);
        Assert.NotNull(duplicateError);
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
        Assert.Equal(2, store.SchemaVersion);
        Assert.Equal("legacy-run", run.RunId);

        store.Upsert(run with { Outcome = "succeeded", CompletedAt = DateTimeOffset.UtcNow },
            CreateEvent(run, "queued", "succeeded"));
        Assert.Single(store.LoadEvents("runtime-1", "legacy-run"));
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
        var candidates = Enumerable.Range(0, 32)
            .Select(index => CreateRun(
                $"concurrent-run-{index}",
                $"concurrent-request-{index}",
                "succeeded"))
            .ToArray();

        await Task.WhenAll(candidates.Select(run => Task.Run(() =>
            store.Save(
                Array.Empty<PersistedRuntimeState>(),
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
    public void ControlPlaneStateStorePreservesSnapshotAndCleansTempAfterBackupFailure()
    {
        var statePath = TemporaryPath("json");
        var backupPath = $"{statePath}.bak";
        var store = CreateStateStore(statePath);
        var original = CreateRun("original-run", "original-request", "succeeded");

        try
        {
            store.Save(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                new[] { original });
            Directory.CreateDirectory(backupPath);

            store.Save(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                new[] { CreateRun("replacement-run", "replacement-request", "succeeded") });

            Assert.True(store.IsDirty);
            Assert.NotNull(store.LastSaveError);
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
        public string Provider => "failure-injection";
        public string Location => "memory";
        public int SchemaVersion => 2;
        public string? LastError => "injected migration write failure";
        public IReadOnlyList<OrchestraRunSummary> AttemptedRuns { get; private set; } =
            Array.Empty<OrchestraRunSummary>();

        public IReadOnlyList<OrchestraRunSummary> LoadAll() => Array.Empty<OrchestraRunSummary>();

        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) =>
            Array.Empty<OrchestraRunEvent>();

        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null) => false;

        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs)
        {
            AttemptedRuns = runs.ToArray();
            return false;
        }

        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds) => false;
    }
}
