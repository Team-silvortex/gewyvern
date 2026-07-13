using System.Collections.Immutable;

namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    public IReadOnlyList<OrchestraRunSummary> ListOrchestraRuns(string runtimeId) =>
        orchestraRuns.TryGetValue(runtimeId, out var queue)
            ? queue.Reverse().ToArray()
            : Array.Empty<OrchestraRunSummary>();

    public OrchestraRunSummary RecordOrchestraRun(
        string runtimeId,
        string planId,
        string outcome,
        IReadOnlyList<OrchestraExecutionStepResult> steps)
    {
        var run = new OrchestraRunSummary(
            $"orun_{Guid.NewGuid():N}",
            runtimeId,
            planId,
            outcome,
            DateTimeOffset.UtcNow,
            steps);
        orchestraRuns.AddOrUpdate(
            runtimeId,
            _ => ImmutableQueue<OrchestraRunSummary>.Empty.Enqueue(run),
            (_, queue) => TrimOrchestraRuns(queue.Enqueue(run)));
        PersistState();
        return run;
    }

    internal static ImmutableQueue<OrchestraRunSummary> TrimOrchestraRuns(ImmutableQueue<OrchestraRunSummary> queue)
    {
        while (queue.Count() > MaxOrchestraRunsPerRuntime)
        {
            queue = queue.Dequeue();
        }

        return queue;
    }
}
