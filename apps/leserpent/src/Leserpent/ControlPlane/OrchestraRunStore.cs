namespace Leserpent.ControlPlane;

public sealed class OrchestraPersistenceException : InvalidOperationException
{
    public OrchestraPersistenceException(string message)
        : base(message)
    {
    }

    public OrchestraPersistenceException(string message, Exception innerException)
        : base(message, innerException)
    {
    }
}

public sealed record OrchestraActiveRunConflict(string RuntimeId, string RunId, string Outcome);

public sealed record OrchestraDeleteCommand(
    string CommandId,
    IReadOnlyList<string> RuntimeIds);

public sealed record OrchestraDeleteReceipt(
    string CommandId,
    ulong OperationGeneration,
    IReadOnlyList<string> RuntimeIds,
    uint DeletedRuntimeCount,
    ulong DeletedRunCount,
    ulong DeletedEventCount,
    DateTimeOffset CommittedAt,
    bool Replayed);

public sealed record OrchestraDeleteReplayHorizon(
    ulong Capacity,
    ulong Retained,
    ulong? OldestGeneration,
    ulong? NewestGeneration,
    ulong NextGeneration,
    ulong EvictedThroughGeneration,
    ulong? ProtectedFromGeneration);

public sealed record OrchestraDeleteReplayCheckpoint(
    ulong MinimumRetainedGeneration,
    ulong ObservedThroughGeneration);

public sealed class OrchestraRuntimeBusyException(IReadOnlyList<OrchestraActiveRunConflict> activeRuns)
    : InvalidOperationException("one or more runtimes have active Orchestra runs")
{
    public IReadOnlyList<OrchestraActiveRunConflict> ActiveRuns { get; } = activeRuns;
}

public interface IOrchestraRunStore
{
    string Provider { get; }
    string Location { get; }
    int SchemaVersion { get; }
    bool SupportsDeleteReplayHorizon => false;
    string? LastError { get; }
    IReadOnlyList<OrchestraRunSummary> LoadAll();
    IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId);
    bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null);
    bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs);
    bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds);
    OrchestraDeleteReceipt? DeleteRuntimes(OrchestraDeleteCommand command) =>
        null;
    OrchestraDeleteReplayHorizon? GetDeleteReplayHorizon() => null;
    OrchestraDeleteReplayHorizon? CheckpointDeleteReplayHorizon(
        OrchestraDeleteReplayCheckpoint checkpoint) =>
        null;
}

public sealed class InMemoryOrchestraRunStore : IOrchestraRunStore
{
    private readonly Dictionary<string, OrchestraRunSummary> runs = new(StringComparer.OrdinalIgnoreCase);
    private readonly List<OrchestraRunEvent> events = new();
    private readonly Dictionary<string, OrchestraDeleteReceipt>
        deleteReceipts = new(StringComparer.Ordinal);
    private long nextEventId;
    private ulong nextDeleteGeneration = 1;
    private ulong evictedDeleteGeneration;
    private ulong? protectedDeleteGeneration;

    public string Provider => "memory";
    public string Location => "memory";
    public int SchemaVersion => 0;
    public bool SupportsDeleteReplayHorizon => false;
    public string? LastError => null;

    public IReadOnlyList<OrchestraRunSummary> LoadAll() =>
        runs.Values.OrderByDescending(run => run.ExecutedAt).ToArray();

    public IReadOnlyList<OrchestraRunEvent> LoadEvents(
        string runtimeId,
        string runId)
    {
        var retained = events.Where(item =>
                string.Equals(item.RuntimeId, runtimeId, StringComparison.OrdinalIgnoreCase)
                && string.Equals(item.RunId, runId, StringComparison.OrdinalIgnoreCase))
            .OrderBy(item => item.EventId)
            .ToArray();
        ControlPlaneStateValidator.ValidateOrchestraEventSequence(
            runs.GetValueOrDefault(runId),
            retained,
            runtimeId,
            runId);
        return retained;
    }

    public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null)
    {
        try
        {
            ControlPlaneStateValidator.ValidateOrchestraStoreEnvelope(
                run,
                eventRecord);
        }
        catch (InvalidDataException)
        {
            return false;
        }

        if (eventRecord is not null)
        {
            var persistedEvent = eventRecord with
            {
                EventId = Interlocked.Increment(
                    ref nextEventId),
            };
            var retained = events
                .Where(item => string.Equals(
                    item.RunId,
                    run.RunId,
                    StringComparison.Ordinal))
                .Append(persistedEvent)
                .OrderBy(item => item.EventId)
                .ToArray();
            try
            {
                ControlPlaneStateValidator
                    .ValidateOrchestraEventSequence(
                        run,
                        retained,
                        run.RuntimeId,
                        run.RunId);
            }
            catch (InvalidDataException)
            {
                return false;
            }
            events.Add(persistedEvent);
        }
        runs[run.RunId] = run;
        return true;
    }

    public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> replacement)
    {
        try
        {
            foreach (var run in replacement)
            {
                ControlPlaneStateValidator.ValidateOrchestraStoreEnvelope(
                    run,
                    null);
            }
        }
        catch (InvalidDataException)
        {
            return false;
        }

        runs.Clear();
        events.Clear();
        foreach (var run in replacement)
        {
            if (!Upsert(
                    run,
                    ControlPlaneStateValidator
                        .CreateLegacyOrchestraImportEvent(run)))
            {
                runs.Clear();
                events.Clear();
                return false;
            }
        }
        return true;
    }

    public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds)
    {
        var runtimeIdSet = runtimeIds.ToHashSet(StringComparer.OrdinalIgnoreCase);
        foreach (var runId in runs.Values
            .Where(run => runtimeIdSet.Contains(run.RuntimeId))
            .Select(run => run.RunId)
            .ToArray())
        {
            runs.Remove(runId);
        }
        events.RemoveAll(item => runtimeIdSet.Contains(item.RuntimeId));
        return true;
    }

    public OrchestraDeleteReceipt? DeleteRuntimes(
        OrchestraDeleteCommand command)
    {
        var runtimeIds = command.RuntimeIds
            .Order(StringComparer.Ordinal)
            .ToArray();
        if (deleteReceipts.TryGetValue(
                command.CommandId,
                out var retained))
        {
            return retained.RuntimeIds.SequenceEqual(
                    runtimeIds,
                    StringComparer.Ordinal)
                ? retained with { Replayed = true }
                : null;
        }
        if (deleteReceipts.Count >= 4096)
        {
            return null;
        }
        var runtimeIdSet =
            runtimeIds.ToHashSet(StringComparer.OrdinalIgnoreCase);
        var deletedRuns = runs.Values
            .Where(run => runtimeIdSet.Contains(run.RuntimeId))
            .ToArray();
        var deletedEvents = events
            .Where(item => runtimeIdSet.Contains(item.RuntimeId))
            .ToArray();
        if (!DeleteRuntimes(runtimeIds))
        {
            return null;
        }
        var receipt = new OrchestraDeleteReceipt(
            command.CommandId,
            nextDeleteGeneration++,
            runtimeIds,
            (uint)deletedRuns
                .Select(run => run.RuntimeId)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .Count(),
            (ulong)deletedRuns.Length,
            (ulong)deletedEvents.Length,
            DateTimeOffset.UtcNow,
            false);
        deleteReceipts[command.CommandId] = receipt;
        protectedDeleteGeneration ??= receipt.OperationGeneration;
        return receipt;
    }

    public OrchestraDeleteReplayHorizon GetDeleteReplayHorizon()
    {
        var generations = deleteReceipts.Values
            .Select(static receipt => receipt.OperationGeneration)
            .Order()
            .ToArray();
        return new OrchestraDeleteReplayHorizon(
            4096,
            checked((ulong)generations.Length),
            generations.FirstOrDefault() is var oldest && oldest > 0
                ? oldest
                : null,
            generations.LastOrDefault() is var newest && newest > 0
                ? newest
                : null,
            nextDeleteGeneration,
            evictedDeleteGeneration,
            protectedDeleteGeneration);
    }

    public OrchestraDeleteReplayHorizon? CheckpointDeleteReplayHorizon(
        OrchestraDeleteReplayCheckpoint checkpoint)
    {
        var horizon = GetDeleteReplayHorizon();
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
            return null;
        }
        var expected = checked(
            checkpoint.ObservedThroughGeneration -
            checkpoint.MinimumRetainedGeneration + 1);
        var retained = deleteReceipts.Values.Count(receipt =>
            receipt.OperationGeneration >=
                checkpoint.MinimumRetainedGeneration &&
            receipt.OperationGeneration <=
                checkpoint.ObservedThroughGeneration);
        if (checked((ulong)retained) != expected)
        {
            return null;
        }
        protectedDeleteGeneration =
            checkpoint.MinimumRetainedGeneration;
        var evicted = deleteReceipts
            .Where(pair => pair.Value.OperationGeneration <
                checkpoint.MinimumRetainedGeneration)
            .ToArray();
        foreach (var pair in evicted)
        {
            deleteReceipts.Remove(pair.Key);
            evictedDeleteGeneration = Math.Max(
                evictedDeleteGeneration,
                pair.Value.OperationGeneration);
        }
        return GetDeleteReplayHorizon();
    }
}
