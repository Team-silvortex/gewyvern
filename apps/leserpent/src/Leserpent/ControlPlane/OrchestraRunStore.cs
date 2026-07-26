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
    string? LastError { get; }
    IReadOnlyList<OrchestraRunSummary> LoadAll();
    IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId);
    bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null);
    bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs);
    bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds);
}

public sealed class InMemoryOrchestraRunStore : IOrchestraRunStore
{
    private readonly Dictionary<string, OrchestraRunSummary> runs = new(StringComparer.OrdinalIgnoreCase);
    private readonly List<OrchestraRunEvent> events = new();
    private long nextEventId;

    public string Provider => "memory";
    public string Location => "memory";
    public int SchemaVersion => 0;
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
}
