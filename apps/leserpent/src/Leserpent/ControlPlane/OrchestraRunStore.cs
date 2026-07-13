namespace Leserpent.ControlPlane;

public interface IOrchestraRunStore
{
    string Provider { get; }
    string Location { get; }
    int SchemaVersion { get; }
    string? LastError { get; }
    IReadOnlyList<OrchestraRunSummary> LoadAll();
    IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId);
    bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null);
    void ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs);
    void DeleteRuntime(string runtimeId);
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

    public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) =>
        events.Where(item =>
                string.Equals(item.RuntimeId, runtimeId, StringComparison.OrdinalIgnoreCase)
                && string.Equals(item.RunId, runId, StringComparison.OrdinalIgnoreCase))
            .OrderBy(item => item.EventId)
            .ToArray();

    public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null)
    {
        runs[run.RunId] = run;
        if (eventRecord is not null)
        {
            events.Add(eventRecord with { EventId = Interlocked.Increment(ref nextEventId) });
        }
        return true;
    }

    public void ReplaceAll(IReadOnlyList<OrchestraRunSummary> replacement)
    {
        runs.Clear();
        events.Clear();
        foreach (var run in replacement)
        {
            runs[run.RunId] = run;
        }
    }

    public void DeleteRuntime(string runtimeId)
    {
        foreach (var runId in runs.Values
            .Where(run => string.Equals(run.RuntimeId, runtimeId, StringComparison.OrdinalIgnoreCase))
            .Select(run => run.RunId)
            .ToArray())
        {
            runs.Remove(runId);
        }
        events.RemoveAll(item => string.Equals(item.RuntimeId, runtimeId, StringComparison.OrdinalIgnoreCase));
    }
}
