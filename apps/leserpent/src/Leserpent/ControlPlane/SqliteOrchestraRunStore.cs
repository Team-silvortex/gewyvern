using System.Text.Json;
using Microsoft.Data.Sqlite;

namespace Leserpent.ControlPlane;

public sealed class SqliteOrchestraRunStore : IOrchestraRunStore
{
    private const int CurrentSchemaVersion = 5;
    private const int MaxRunsPerRuntime = 32;
    private const ulong MaxDeleteReceipts = 4096;
    private readonly Func<bool> canWrite;
    private readonly object initializationSync = new();
    private readonly ILogger<SqliteOrchestraRunStore> logger;
    private bool initialized;
    private volatile bool pendingWriterInitialization;

    public SqliteOrchestraRunStore(
        IConfiguration configuration,
        IHostEnvironment environment,
        ILogger<SqliteOrchestraRunStore> logger,
        bool writable = true)
        : this(
            configuration,
            environment,
            logger,
            () => writable)
    {
    }

    public SqliteOrchestraRunStore(
        IConfiguration configuration,
        IHostEnvironment environment,
        ILogger<SqliteOrchestraRunStore> logger,
        Func<bool> canWrite)
    {
        Location = configuration["LESERPENT_DATABASE_PATH"] ?? DefaultDatabasePath(environment);
        this.logger = logger;
        this.canWrite = canWrite ?? throw new ArgumentNullException(nameof(canWrite));
        if (canWrite())
        {
            EnsureInitialized();
        }
        else
        {
            pendingWriterInitialization = DatabasePathIsMissing(Location);
        }
    }

    public string Provider => "sqlite";
    public string Location { get; }
    public int SchemaVersion => CurrentSchemaVersion;
    public bool SupportsDeleteReplayHorizon => true;
    public string? LastError { get; private set; }

    public IReadOnlyList<OrchestraRunSummary> LoadAll()
    {
        try
        {
            if (IsPendingWriterInitialization())
            {
                LastError = null;
                return Array.Empty<OrchestraRunSummary>();
            }
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
            if (IsPendingWriterInitialization())
            {
                LastError = null;
                return Array.Empty<OrchestraRunEvent>();
            }
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
        if (!AllowWrite())
        {
            return false;
        }
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
        if (!AllowWrite())
        {
            return false;
        }
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
        if (!AllowWrite())
        {
            return false;
        }
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
        if (!AllowWrite())
        {
            return null;
        }
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
                if (checked((ulong)Convert.ToInt64(
                        capacity.ExecuteScalar())) >=
                    MaxDeleteReceipts)
                {
                    throw new InvalidOperationException(
                        "Orchestra delete replay horizon is pinned by reconciliation audit");
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
            using var protect = connection.CreateCommand();
            protect.Transaction = transaction;
            protect.CommandText = """
                UPDATE orchestra_delete_replay_horizon
                SET protected_from_generation =
                    COALESCE(protected_from_generation, $generation)
                WHERE id = 1;
                """;
            protect.Parameters.AddWithValue(
                "$generation",
                checked((long)generation));
            if (protect.ExecuteNonQuery() != 1)
            {
                throw new InvalidDataException(
                    "Orchestra delete replay protection is inconsistent");
            }
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

    public OrchestraDeleteReplayHorizon? GetDeleteReplayHorizon()
    {
        try
        {
            if (IsPendingWriterInitialization())
            {
                LastError = null;
                return new OrchestraDeleteReplayHorizon(
                    MaxDeleteReceipts,
                    0,
                    null,
                    null,
                    1,
                    0,
                    null,
                    null);
            }
            using var connection = OpenConnection();
            var horizon = ReadDeleteReplayHorizon(
                connection,
                null);
            LastError = null;
            return horizon;
        }
        catch (Exception ex)
        {
            RecordError(ex, "query Orchestra delete replay horizon");
            return null;
        }
    }

    public OrchestraDeleteReplayHorizon? CheckpointDeleteReplayHorizon(
        OrchestraDeleteReplayCheckpoint checkpoint)
    {
        if (!AllowWrite())
        {
            return null;
        }
        try
        {
            using var connection = OpenConnection();
            using var transaction = connection.BeginTransaction();
            var horizon = ReadDeleteReplayHorizon(
                connection,
                transaction);
            if (checkpoint.MinimumRetainedGeneration == 0 ||
                checkpoint.ObservedThroughGeneration <
                    checkpoint.MinimumRetainedGeneration ||
                horizon.NewestGeneration is null ||
                checkpoint.ObservedThroughGeneration >
                    horizon.NewestGeneration ||
                checkpoint.MinimumRetainedGeneration <=
                    horizon.EvictedThroughGeneration ||
                horizon.ProtectedFromGeneration is not null &&
                    checkpoint.MinimumRetainedGeneration <
                        horizon.ProtectedFromGeneration)
            {
                throw new InvalidDataException(
                    "Orchestra delete replay checkpoint is outside the retained horizon");
            }
            var expected = checked(
                checkpoint.ObservedThroughGeneration -
                checkpoint.MinimumRetainedGeneration + 1);
            using (var range = connection.CreateCommand())
            {
                range.Transaction = transaction;
                range.CommandText = """
                    SELECT COUNT(*) FROM orchestra_delete_operations
                    WHERE generation BETWEEN $minimum AND $observed;
                    """;
                range.Parameters.AddWithValue(
                    "$minimum",
                    checked((long)checkpoint.MinimumRetainedGeneration));
                range.Parameters.AddWithValue(
                    "$observed",
                    checked((long)checkpoint.ObservedThroughGeneration));
                if (checked((ulong)Convert.ToInt64(
                        range.ExecuteScalar())) != expected)
                {
                    throw new InvalidDataException(
                        "Orchestra delete replay checkpoint has a receipt gap");
                }
            }
            using (var update = connection.CreateCommand())
            {
                update.Transaction = transaction;
                update.CommandText = """
                    UPDATE orchestra_delete_replay_horizon
                    SET protected_from_generation = $minimum,
                        checkpointed_through_generation = $observed
                    WHERE id = 1 AND (
                        protected_from_generation IS NULL OR
                        protected_from_generation <= $minimum);
                    """;
                update.Parameters.AddWithValue(
                    "$minimum",
                    checked((long)checkpoint.MinimumRetainedGeneration));
                update.Parameters.AddWithValue(
                    "$observed",
                    checked((long)checkpoint.ObservedThroughGeneration));
                if (update.ExecuteNonQuery() != 1)
                {
                    throw new InvalidDataException(
                        "Orchestra delete replay checkpoint conflicted");
                }
            }
            CompactDeleteReplayHorizon(
                connection,
                transaction,
                checkpoint.MinimumRetainedGeneration);
            var checkpointed = ReadDeleteReplayHorizon(
                connection,
                transaction);
            transaction.Commit();
            LastError = null;
            return checkpointed;
        }
        catch (Exception ex)
        {
            RecordError(ex, "checkpoint Orchestra delete replay horizon");
            return null;
        }
    }

    private static OrchestraDeleteReplayHorizon ReadDeleteReplayHorizon(
        SqliteConnection connection,
        SqliteTransaction? transaction)
    {
        using var command = connection.CreateCommand();
        command.Transaction = transaction;
        command.CommandText = """
            SELECT
                COUNT(*),
                MIN(generation),
                MAX(generation),
                COALESCE((
                    SELECT seq + 1 FROM sqlite_sequence
                    WHERE name = 'orchestra_delete_operations'
                ), 1),
                (
                    SELECT evicted_through_generation
                    FROM orchestra_delete_replay_horizon WHERE id = 1
                ),
                (
                    SELECT protected_from_generation
                    FROM orchestra_delete_replay_horizon WHERE id = 1
                ),
                (
                    SELECT checkpointed_through_generation
                    FROM orchestra_delete_replay_horizon WHERE id = 1
                )
            FROM orchestra_delete_operations;
            """;
        using var reader = command.ExecuteReader();
        if (!reader.Read())
        {
            throw new InvalidDataException(
                "Orchestra delete replay horizon is missing");
        }
        var retained = checked((ulong)reader.GetInt64(0));
        var oldest = reader.IsDBNull(1)
            ? null
            : checked((ulong?)reader.GetInt64(1));
        var newest = reader.IsDBNull(2)
            ? null
            : checked((ulong?)reader.GetInt64(2));
        var next = checked((ulong)reader.GetInt64(3));
        var evicted = checked((ulong)reader.GetInt64(4));
        var protectedFrom = reader.IsDBNull(5)
            ? null
            : checked((ulong?)reader.GetInt64(5));
        var checkpointedThrough = reader.IsDBNull(6)
            ? null
            : checked((ulong?)reader.GetInt64(6));
        var contiguous = oldest is null && newest is null
            ? retained == 0 &&
                checked(evicted + 1) == next &&
                protectedFrom is null
                && checkpointedThrough is null
            : oldest is not null && newest is not null &&
                retained > 0 &&
                checked(evicted + 1) == oldest &&
                checked(newest.Value + 1) == next &&
                checked(newest.Value - oldest.Value + 1) == retained &&
                protectedFrom is not null &&
                protectedFrom >= oldest &&
                protectedFrom <= newest &&
                (checkpointedThrough is null ||
                    checkpointedThrough >= protectedFrom &&
                    checkpointedThrough <= newest);
        if (retained > MaxDeleteReceipts ||
            next == 0 ||
            evicted >= next ||
            !contiguous)
        {
            throw new InvalidDataException(
                "Orchestra delete replay horizon metadata is inconsistent");
        }
        return new OrchestraDeleteReplayHorizon(
            MaxDeleteReceipts,
            retained,
            oldest,
            newest,
            next,
            evicted,
            protectedFrom,
            checkpointedThrough);
    }

    private static void CompactDeleteReplayHorizon(
        SqliteConnection connection,
        SqliteTransaction transaction,
        ulong protectedFrom)
    {
        long count;
        long? highWater;
        using (var plan = connection.CreateCommand())
        {
            plan.Transaction = transaction;
            plan.CommandText = """
                SELECT COUNT(*), MAX(generation)
                FROM orchestra_delete_operations
                WHERE generation < $protected_from;
                """;
            plan.Parameters.AddWithValue(
                "$protected_from",
                checked((long)protectedFrom));
            using var reader = plan.ExecuteReader();
            _ = reader.Read();
            count = reader.GetInt64(0);
            highWater = reader.IsDBNull(1)
                ? null
                : reader.GetInt64(1);
        }
        if (count == 0)
        {
            return;
        }
        using var delete = connection.CreateCommand();
        delete.Transaction = transaction;
        delete.CommandText = """
            DELETE FROM orchestra_delete_operations
            WHERE generation < $protected_from;
            """;
        delete.Parameters.AddWithValue(
            "$protected_from",
            checked((long)protectedFrom));
        var deleted = delete.ExecuteNonQuery();
        using var update = connection.CreateCommand();
        update.Transaction = transaction;
        update.CommandText = """
            UPDATE orchestra_delete_replay_horizon
            SET evicted_through_generation = $high_water
            WHERE id = 1 AND evicted_through_generation < $high_water;
            """;
        update.Parameters.AddWithValue(
            "$high_water",
            highWater ??
                throw new InvalidDataException(
                    "Orchestra delete replay compaction plan is incomplete"));
        if (deleted != count || update.ExecuteNonQuery() != 1)
        {
            throw new InvalidDataException(
                "Orchestra delete replay compaction is inconsistent");
        }
    }

    private void Initialize(SqliteConnection connection)
    {
        int version;
        using (var versionCommand = connection.CreateCommand())
        {
            versionCommand.CommandText = "PRAGMA user_version;";
            version = Convert.ToInt32(versionCommand.ExecuteScalar());
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
            CREATE TABLE IF NOT EXISTS orchestra_delete_replay_horizon (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                evicted_through_generation INTEGER NOT NULL
                    CHECK (evicted_through_generation >= 0),
                protected_from_generation INTEGER NULL
                    CHECK (protected_from_generation >= 1)
            );
            INSERT OR IGNORE INTO orchestra_delete_replay_horizon (
                id, evicted_through_generation,
                protected_from_generation)
            SELECT 1, 0, MIN(generation)
            FROM orchestra_delete_operations;
            """;
        command.ExecuteNonQuery();
        if (version < 5)
        {
            using var migration = connection.CreateCommand();
            migration.CommandText = """
                ALTER TABLE orchestra_delete_replay_horizon
                    ADD COLUMN checkpointed_through_generation INTEGER NULL
                    CHECK (checkpointed_through_generation >= 1);
                UPDATE orchestra_delete_replay_horizon
                SET checkpointed_through_generation =
                    protected_from_generation;
                PRAGMA user_version = 5;
                """;
            migration.ExecuteNonQuery();
        }
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
        var writable = canWrite();
        if (writable)
        {
            EnsureInitialized();
        }
        return OpenConnection(writable);
    }

    private SqliteConnection OpenConnection(bool writable)
    {
        var connectionString = new SqliteConnectionStringBuilder
        {
            DataSource = Location,
            Mode = writable
                ? SqliteOpenMode.ReadWriteCreate
                : SqliteOpenMode.ReadOnly,
            Cache = SqliteCacheMode.Shared,
            Pooling = true,
        }.ToString();
        var connection = new SqliteConnection(connectionString);
        connection.Open();
        using var pragma = connection.CreateCommand();
        pragma.CommandText = writable
            ? "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;"
            : "PRAGMA query_only=ON; PRAGMA busy_timeout=5000; PRAGMA foreign_keys=ON;";
        pragma.ExecuteNonQuery();
        return connection;
    }

    private void EnsureInitialized()
    {
        lock (initializationSync)
        {
            if (initialized || !canWrite())
            {
                return;
            }
            var directory = Path.GetDirectoryName(Location);
            if (!string.IsNullOrWhiteSpace(directory))
            {
                Directory.CreateDirectory(directory);
            }
            using var connection = OpenConnection(writable: true);
            Initialize(connection);
            initialized = true;
            pendingWriterInitialization = false;
        }
    }

    private bool IsPendingWriterInitialization()
    {
        if (!pendingWriterInitialization)
        {
            return false;
        }
        if (canWrite() || !DatabasePathIsMissing(Location))
        {
            pendingWriterInitialization = false;
            return false;
        }
        return true;
    }

    private static bool DatabasePathIsMissing(string path)
    {
        try
        {
            _ = File.GetAttributes(path);
            return false;
        }
        catch (FileNotFoundException)
        {
            return true;
        }
        catch (DirectoryNotFoundException)
        {
            return true;
        }
        catch
        {
            return false;
        }
    }

    private bool AllowWrite()
    {
        if (canWrite())
        {
            return true;
        }
        LastError = "orchestra_store_read_only";
        return false;
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
