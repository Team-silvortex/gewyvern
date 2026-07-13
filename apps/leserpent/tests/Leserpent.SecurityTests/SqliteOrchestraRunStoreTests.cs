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

        store.DeleteRuntime("runtime-1");

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
    }

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = string.Empty;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
