using System.Text.Json;
using Microsoft.Data.Sqlite;

namespace Leserpent.ControlPlane;

public sealed class SqliteOrchestraRunStore : IOrchestraRunStore
{
    private const int CurrentSchemaVersion = 3;
    private const int MaxRunsPerRuntime = 32;
    private readonly string connectionString;
    private readonly ILogger<SqliteOrchestraRunStore> logger;

    public SqliteOrchestraRunStore(
        IConfiguration configuration,
        IHostEnvironment environment,
        ILogger<SqliteOrchestraRunStore> logger)
    {
        Location = configuration["LESERPENT_DATABASE_PATH"] ?? DefaultDatabasePath(environment);
        this.logger = logger;
        var directory = Path.GetDirectoryName(Location);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }
        connectionString = new SqliteConnectionStringBuilder
        {
            DataSource = Location,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Cache = SqliteCacheMode.Shared,
            Pooling = true,
        }.ToString();
        Initialize();
    }

    public string Provider => "sqlite";
    public string Location { get; }
    public int SchemaVersion => CurrentSchemaVersion;
    public string? LastError { get; private set; }

    public IReadOnlyList<OrchestraRunSummary> LoadAll()
    {
        try
        {
            using var connection = OpenConnection();
            using var command = connection.CreateCommand();
            command.CommandText = """
                SELECT run_id, runtime_id, plan_id, outcome, executed_at, steps_json,
                       completed_at, attempt, retried_from_run_id, approved_by,
                       approval_note, plan_revision, request_id
                FROM orchestra_runs
                ORDER BY executed_at DESC;
                """;
            using var reader = command.ExecuteReader();
            var runs = new List<OrchestraRunSummary>();
            while (reader.Read())
            {
                var run = ReadRun(reader);
                ControlPlaneStateValidator
                    .ValidateOrchestraStoreEnvelope(run, null);
                runs.Add(run);
            }
            LastError = null;
            return runs;
        }
        catch (Exception ex)
        {
            RecordError(ex, "load Orchestra runs");
            return Array.Empty<OrchestraRunSummary>();
        }
    }

    public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId)
    {
        try
        {
            using var connection = OpenConnection();
            var events = ReadEvents(
                connection,
                null,
                runtimeId,
                runId);
            ControlPlaneStateValidator
                .ValidateOrchestraEventSequence(
                    null,
                    events,
                    runtimeId,
                    runId);
            LastError = null;
            return events;
        }
        catch (Exception ex)
        {
            RecordError(ex, $"load Orchestra events for run {runId}");
            return Array.Empty<OrchestraRunEvent>();
        }
    }

    public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null)
    {
        try
        {
            ControlPlaneStateValidator.ValidateOrchestraStoreEnvelope(
                run,
                eventRecord);
            using var connection = OpenConnection();
            using var transaction = connection.BeginTransaction();
            Upsert(connection, transaction, run);
            if (eventRecord is not null)
            {
                InsertEvent(connection, transaction, eventRecord);
                var events = ReadEvents(
                    connection,
                    transaction,
                    run.RuntimeId,
                    run.RunId);
                ControlPlaneStateValidator
                    .ValidateOrchestraEventSequence(
                        run,
                        events,
                        run.RuntimeId,
                        run.RunId);
            }
            TrimRuntime(connection, transaction, run.RuntimeId);
            transaction.Commit();
            LastError = null;
            return true;
        }
        catch (Exception ex)
        {
            RecordError(ex, $"upsert Orchestra run {run.RunId}");
            return false;
        }
    }

    public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs)
    {
        try
        {
            foreach (var run in runs)
            {
                ControlPlaneStateValidator
                    .ValidateOrchestraStoreEnvelope(run, null);
            }
            using var connection = OpenConnection();
            using var transaction = connection.BeginTransaction();
            using (var delete = connection.CreateCommand())
            {
                delete.Transaction = transaction;
                delete.CommandText = "DELETE FROM orchestra_run_events; DELETE FROM orchestra_runs;";
                delete.ExecuteNonQuery();
            }
            foreach (var run in runs.OrderBy(run => run.ExecutedAt))
            {
                Upsert(connection, transaction, run);
                InsertEvent(
                    connection,
                    transaction,
                    ControlPlaneStateValidator
                        .CreateLegacyOrchestraImportEvent(run));
            }
            transaction.Commit();
            LastError = null;
            return true;
        }
        catch (Exception ex)
        {
            RecordError(ex, "replace Orchestra runs");
            return false;
        }
    }

    public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds)
    {
        if (runtimeIds.Count == 0)
        {
            return true;
        }
        try
        {
            using var connection = OpenConnection();
            using var transaction = connection.BeginTransaction();
            foreach (var runtimeId in runtimeIds.Distinct(StringComparer.OrdinalIgnoreCase))
            {
                using var command = connection.CreateCommand();
                command.Transaction = transaction;
                command.CommandText = """
                    DELETE FROM orchestra_run_events WHERE runtime_id = $runtime_id;
                    DELETE FROM orchestra_runs WHERE runtime_id = $runtime_id;
                    """;
                command.Parameters.AddWithValue("$runtime_id", runtimeId);
                command.ExecuteNonQuery();
            }
            transaction.Commit();
            LastError = null;
            return true;
        }
        catch (Exception ex)
        {
            RecordError(ex, $"delete Orchestra runs for {runtimeIds.Count} runtime(s)");
            return false;
        }
    }

    public OrchestraDeleteReceipt? DeleteRuntimes(
        OrchestraDeleteCommand command)
    {
        var runtimeIds = command.RuntimeIds
            .Order(StringComparer.Ordinal)
            .ToArray();
        if (string.IsNullOrWhiteSpace(command.CommandId) ||
            runtimeIds.Length is < 1 or > 128 ||
            runtimeIds.Distinct(StringComparer.Ordinal).Count() !=
                runtimeIds.Length)
        {
            LastError = "orchestra_store_operation_failed";
            return null;
        }
        try
        {
            using var connection = OpenConnection();
            using var transaction = connection.BeginTransaction();
            using (var replay = connection.CreateCommand())
            {
                replay.Transaction = transaction;
                replay.CommandText = """
                    SELECT generation, runtime_ids_json, deleted_runtime_count,
                           deleted_run_count, deleted_event_count,
                           committed_at_unix_ms
                    FROM orchestra_delete_operations
                    WHERE operation_id = $operation_id;
                    """;
                replay.Parameters.AddWithValue(
                    "$operation_id",
                    command.CommandId);
                using var reader = replay.ExecuteReader();
                if (reader.Read())
                {
                    var retainedRuntimeIds = JsonSerializer.Deserialize(
                            reader.GetString(1),
                            LeserpentJsonContext.Default.StringArray)
                        ?? throw new InvalidDataException(
                            "Orchestra delete receipt targets are missing");
                    if (!retainedRuntimeIds.SequenceEqual(
                            runtimeIds,
                            StringComparer.Ordinal))
                    {
                        throw new InvalidDataException(
                            "Orchestra delete command was reused for different targets");
                    }
                    var retained = new OrchestraDeleteReceipt(
                        command.CommandId,
                        checked((ulong)reader.GetInt64(0)),
                        retainedRuntimeIds,
                        checked((uint)reader.GetInt64(2)),
                        checked((ulong)reader.GetInt64(3)),
                        checked((ulong)reader.GetInt64(4)),
                        DateTimeOffset.FromUnixTimeMilliseconds(
                            reader.GetInt64(5)),
                        true);
                    reader.Close();
                    transaction.Commit();
                    LastError = null;
                    return retained;
                }
            }
            using (var capacity = connection.CreateCommand())
            {
                capacity.Transaction = transaction;
                capacity.CommandText =
                    "SELECT COUNT(*) FROM orchestra_delete_operations;";
                if (Convert.ToInt64(capacity.ExecuteScalar()) >= 4096)
                {
                    throw new InvalidOperationException(
                        "Orchestra delete receipt capacity is exhausted");
                }
            }
            uint deletedRuntimeCount = 0;
            ulong deletedRunCount = 0;
            ulong deletedEventCount = 0;
            foreach (var runtimeId in runtimeIds)
            {
                var runtimeRunCount = CountRows(
                    connection,
                    transaction,
                    "orchestra_runs",
                    runtimeId);
                deletedRunCount += runtimeRunCount;
                deletedEventCount += CountRows(
                    connection,
                    transaction,
                    "orchestra_run_events",
                    runtimeId);
                if (runtimeRunCount > 0)
                {
                    deletedRuntimeCount++;
                }
                DeleteRuntimeRows(connection, transaction, runtimeId);
            }
            var committedAtUnixMs =
                DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();
            var committedAt = DateTimeOffset
                .FromUnixTimeMilliseconds(committedAtUnixMs);
            using var insert = connection.CreateCommand();
            insert.Transaction = transaction;
            insert.CommandText = """
                INSERT INTO orchestra_delete_operations (
                    operation_id, runtime_ids_json, deleted_runtime_count,
                    deleted_run_count, deleted_event_count,
                    committed_at_unix_ms)
                VALUES (
                    $operation_id, $runtime_ids_json,
                    $deleted_runtime_count, $deleted_run_count,
                    $deleted_event_count, $committed_at_unix_ms);
                """;
            insert.Parameters.AddWithValue(
                "$operation_id",
                command.CommandId);
            insert.Parameters.AddWithValue(
                "$runtime_ids_json",
                JsonSerializer.Serialize(
                    runtimeIds,
                    LeserpentJsonContext.Default.StringArray));
            insert.Parameters.AddWithValue(
                "$deleted_runtime_count",
                deletedRuntimeCount);
            insert.Parameters.AddWithValue(
                "$deleted_run_count",
                checked((long)deletedRunCount));
            insert.Parameters.AddWithValue(
                "$deleted_event_count",
                checked((long)deletedEventCount));
            insert.Parameters.AddWithValue(
                "$committed_at_unix_ms",
                committedAtUnixMs);
            insert.ExecuteNonQuery();
            using var generationQuery = connection.CreateCommand();
            generationQuery.Transaction = transaction;
            generationQuery.CommandText = "SELECT last_insert_rowid();";
            var generation = checked(
                (ulong)Convert.ToInt64(
                    generationQuery.ExecuteScalar()));
            transaction.Commit();
            LastError = null;
            return new OrchestraDeleteReceipt(
                command.CommandId,
                generation,
                runtimeIds,
                deletedRuntimeCount,
                deletedRunCount,
                deletedEventCount,
                committedAt,
                false);
        }
        catch (Exception ex)
        {
            RecordError(ex, "idempotently delete Orchestra runs");
            return null;
        }
    }

    private void Initialize()
    {
        using var connection = OpenConnection();
        using (var versionCommand = connection.CreateCommand())
        {
            versionCommand.CommandText = "PRAGMA user_version;";
            var version = Convert.ToInt32(versionCommand.ExecuteScalar());
            if (version > CurrentSchemaVersion)
            {
                throw new InvalidOperationException(
                    $"Orchestra database schema {version} is newer than supported schema {CurrentSchemaVersion}");
            }
        }
        using var command = connection.CreateCommand();
        command.CommandText = """
            CREATE TABLE IF NOT EXISTS orchestra_runs (
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
            CREATE INDEX IF NOT EXISTS idx_orchestra_runs_runtime_time
                ON orchestra_runs(runtime_id, executed_at DESC);
            CREATE INDEX IF NOT EXISTS idx_orchestra_runs_outcome_time
                ON orchestra_runs(outcome, executed_at DESC);
            CREATE UNIQUE INDEX IF NOT EXISTS ux_orchestra_runs_runtime_request
                ON orchestra_runs(runtime_id, request_id)
                WHERE request_id IS NOT NULL;
            CREATE TABLE IF NOT EXISTS orchestra_run_events (
                event_id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                runtime_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                from_outcome TEXT NULL,
                to_outcome TEXT NOT NULL,
                summary TEXT NOT NULL,
                recorded_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_orchestra_run_events_run
                ON orchestra_run_events(runtime_id, run_id, event_id);
            CREATE INDEX IF NOT EXISTS idx_orchestra_run_events_runtime_time
                ON orchestra_run_events(runtime_id, recorded_at DESC);
            CREATE TABLE IF NOT EXISTS orchestra_delete_operations (
                generation INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_id TEXT NOT NULL UNIQUE,
                runtime_ids_json TEXT NOT NULL,
                deleted_runtime_count INTEGER NOT NULL,
                deleted_run_count INTEGER NOT NULL,
                deleted_event_count INTEGER NOT NULL,
                committed_at_unix_ms INTEGER NOT NULL
            );
            PRAGMA user_version = 3;
            """;
        command.ExecuteNonQuery();
    }

    private static ulong CountRows(
        SqliteConnection connection,
        SqliteTransaction transaction,
        string table,
        string runtimeId)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText =
            $"SELECT COUNT(*) FROM {table} WHERE runtime_id = $runtime_id;";
        command.Parameters.AddWithValue("$runtime_id", runtimeId);
        return checked((ulong)Convert.ToInt64(command.ExecuteScalar()));
    }

    private static void DeleteRuntimeRows(
        SqliteConnection connection,
        SqliteTransaction transaction,
        string runtimeId)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            DELETE FROM orchestra_run_events WHERE runtime_id = $runtime_id;
            DELETE FROM orchestra_runs WHERE runtime_id = $runtime_id;
            """;
        command.Parameters.AddWithValue("$runtime_id", runtimeId);
        command.ExecuteNonQuery();
    }

    private SqliteConnection OpenConnection()
    {
        var connection = new SqliteConnection(connectionString);
        connection.Open();
        using var pragma = connection.CreateCommand();
        pragma.CommandText = "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;";
        pragma.ExecuteNonQuery();
        return connection;
    }

    private void Upsert(SqliteConnection connection, SqliteTransaction transaction, OrchestraRunSummary run)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            INSERT INTO orchestra_runs (
                run_id, runtime_id, plan_id, outcome, executed_at, steps_json,
                completed_at, attempt, retried_from_run_id, approved_by,
                approval_note, plan_revision, request_id)
            VALUES (
                $run_id, $runtime_id, $plan_id, $outcome, $executed_at, $steps_json,
                $completed_at, $attempt, $retried_from_run_id, $approved_by,
                $approval_note, $plan_revision, $request_id)
            ON CONFLICT(run_id) DO UPDATE SET
                outcome = excluded.outcome,
                steps_json = excluded.steps_json,
                completed_at = excluded.completed_at,
                attempt = excluded.attempt,
                retried_from_run_id = excluded.retried_from_run_id,
                approved_by = excluded.approved_by,
                approval_note = excluded.approval_note,
                plan_revision = excluded.plan_revision,
                request_id = excluded.request_id;
            """;
        command.Parameters.AddWithValue("$run_id", run.RunId);
        command.Parameters.AddWithValue("$runtime_id", run.RuntimeId);
        command.Parameters.AddWithValue("$plan_id", run.PlanId);
        command.Parameters.AddWithValue("$outcome", run.Outcome);
        command.Parameters.AddWithValue("$executed_at", run.ExecutedAt.ToString("O"));
        command.Parameters.AddWithValue(
            "$steps_json",
            JsonSerializer.Serialize(run.Steps.ToArray(), LeserpentJsonContext.Default.OrchestraExecutionStepResultArray));
        command.Parameters.AddWithValue("$completed_at", DbValue(run.CompletedAt?.ToString("O")));
        command.Parameters.AddWithValue("$attempt", run.Attempt);
        command.Parameters.AddWithValue("$retried_from_run_id", DbValue(run.RetriedFromRunId));
        command.Parameters.AddWithValue("$approved_by", DbValue(run.ApprovedBy));
        command.Parameters.AddWithValue("$approval_note", DbValue(run.ApprovalNote));
        command.Parameters.AddWithValue("$plan_revision", DbValue(run.PlanRevision));
        command.Parameters.AddWithValue("$request_id", DbValue(run.RequestId));
        command.ExecuteNonQuery();
    }

    private static void InsertEvent(
        SqliteConnection connection,
        SqliteTransaction transaction,
        OrchestraRunEvent eventRecord)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            INSERT INTO orchestra_run_events (
                run_id, runtime_id, event_type, from_outcome, to_outcome, summary, recorded_at)
            VALUES (
                $run_id, $runtime_id, $event_type, $from_outcome, $to_outcome, $summary, $recorded_at);
            """;
        command.Parameters.AddWithValue("$run_id", eventRecord.RunId);
        command.Parameters.AddWithValue("$runtime_id", eventRecord.RuntimeId);
        command.Parameters.AddWithValue("$event_type", eventRecord.EventType);
        command.Parameters.AddWithValue("$from_outcome", DbValue(eventRecord.FromOutcome));
        command.Parameters.AddWithValue("$to_outcome", eventRecord.ToOutcome);
        command.Parameters.AddWithValue("$summary", eventRecord.Summary);
        command.Parameters.AddWithValue("$recorded_at", eventRecord.RecordedAt.ToString("O"));
        command.ExecuteNonQuery();
    }

    private static IReadOnlyList<OrchestraRunEvent> ReadEvents(
        SqliteConnection connection,
        SqliteTransaction? transaction,
        string runtimeId,
        string runId)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            SELECT event_id, run_id, runtime_id, event_type, from_outcome,
                   to_outcome, summary, recorded_at
            FROM orchestra_run_events
            WHERE runtime_id = $runtime_id AND run_id = $run_id
            ORDER BY event_id;
            """;
        command.Parameters.AddWithValue("$runtime_id", runtimeId);
        command.Parameters.AddWithValue("$run_id", runId);
        using var reader = command.ExecuteReader();
        var events = new List<OrchestraRunEvent>();
        while (reader.Read())
        {
            events.Add(new OrchestraRunEvent(
                reader.GetInt64(0),
                reader.GetString(1),
                reader.GetString(2),
                reader.GetString(3),
                reader.IsDBNull(4)
                    ? null
                    : reader.GetString(4),
                reader.GetString(5),
                reader.GetString(6),
                DateTimeOffset.Parse(reader.GetString(7))));
        }
        return events;
    }

    private static void TrimRuntime(SqliteConnection connection, SqliteTransaction transaction, string runtimeId)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            DELETE FROM orchestra_runs
            WHERE runtime_id = $runtime_id
              AND run_id NOT IN (
                  SELECT run_id FROM orchestra_runs
                  WHERE runtime_id = $runtime_id
                  ORDER BY executed_at DESC
                  LIMIT $limit
              );
            """;
        command.Parameters.AddWithValue("$runtime_id", runtimeId);
        command.Parameters.AddWithValue("$limit", MaxRunsPerRuntime);
        command.ExecuteNonQuery();

        using var deleteOrphanEvents = connection.CreateCommand();
        deleteOrphanEvents.Transaction = transaction;
        deleteOrphanEvents.CommandText = """
            DELETE FROM orchestra_run_events
            WHERE runtime_id = $runtime_id
              AND run_id NOT IN (
                  SELECT run_id FROM orchestra_runs WHERE runtime_id = $runtime_id
              );
            """;
        deleteOrphanEvents.Parameters.AddWithValue("$runtime_id", runtimeId);
        deleteOrphanEvents.ExecuteNonQuery();
    }

    private OrchestraRunSummary ReadRun(SqliteDataReader reader) =>
        new(
            reader.GetString(0),
            reader.GetString(1),
            reader.GetString(2),
            reader.GetString(3),
            DateTimeOffset.Parse(reader.GetString(4)),
            JsonSerializer.Deserialize(reader.GetString(5), LeserpentJsonContext.Default.OrchestraExecutionStepResultArray)
                ?? Array.Empty<OrchestraExecutionStepResult>(),
            reader.IsDBNull(6) ? null : DateTimeOffset.Parse(reader.GetString(6)),
            reader.GetInt32(7),
            reader.IsDBNull(8) ? null : reader.GetString(8),
            reader.IsDBNull(9) ? null : reader.GetString(9),
            reader.IsDBNull(10) ? null : reader.GetString(10),
            reader.IsDBNull(11) ? null : reader.GetString(11),
            reader.IsDBNull(12) ? null : reader.GetString(12));

    private static object DbValue(string? value) => value is null ? DBNull.Value : value;

    private void RecordError(Exception exception, string operation)
    {
        LastError = "orchestra_store_operation_failed";
        logger.LogError(exception, "Failed to {Operation} in SQLite database {DatabasePath}", operation, Location);
    }

    private static string DefaultDatabasePath(IHostEnvironment environment)
    {
        var localData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return !string.IsNullOrWhiteSpace(localData)
            ? Path.Combine(localData, "leserpent", "control-plane.db")
            : Path.Combine(environment.ContentRootPath, ".leserpent-state", "control-plane.db");
    }
}
