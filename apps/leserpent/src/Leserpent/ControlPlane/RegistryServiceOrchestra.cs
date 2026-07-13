using System.Collections.Immutable;

namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    public IReadOnlyList<OrchestraRunSummary> ListOrchestraRuns(string runtimeId) =>
        orchestraRuns.TryGetValue(runtimeId, out var queue)
            ? queue.Reverse().ToArray()
            : Array.Empty<OrchestraRunSummary>();

    public OrchestraRunSummary? GetOrchestraRun(string runtimeId, string runId) =>
        ListOrchestraRuns(runtimeId).FirstOrDefault(run =>
            string.Equals(run.RunId, runId, StringComparison.OrdinalIgnoreCase));

    public OrchestraFleetBoardResponse GetOrchestraFleetBoard()
    {
        var items = runtimes.Values
            .SelectMany(runtime => ListOrchestraRuns(runtime.RuntimeId).Select(run =>
                new OrchestraFleetRunItem(runtime.RuntimeId, runtime.Name, runtime.Tags, run)))
            .OrderByDescending(item => item.Run.ExecutedAt)
            .ToArray();
        return new OrchestraFleetBoardResponse(
            items.Select(item => item.RuntimeId).Distinct(StringComparer.OrdinalIgnoreCase).Count(),
            items.Length,
            items.Count(item => IsActiveOrchestraOutcome(item.Run.Outcome)),
            items.Count(item => string.Equals(item.Run.Outcome, "failed", StringComparison.OrdinalIgnoreCase)),
            items.Count(item => string.Equals(item.Run.Outcome, "degraded", StringComparison.OrdinalIgnoreCase)),
            items.Count(item => IsRetryableOrchestraRun(item.Run)),
            items);
    }

    public OrchestraRunSummary StartOrchestraRun(
        string runId,
        string runtimeId,
        string planId,
        int attempt = 1,
        string? retriedFromRunId = null,
        string? approvedBy = null,
        string? approvalNote = null,
        string? planRevision = null)
    {
        var now = DateTimeOffset.UtcNow;
        var run = new OrchestraRunSummary(
            runId,
            runtimeId,
            planId,
            "queued",
            now,
            Array.Empty<OrchestraExecutionStepResult>(),
            null,
            attempt,
            retriedFromRunId,
            approvedBy,
            approvalNote,
            planRevision);
        AppendOrchestraRun(runtimeId, run);
        return run;
    }

    public OrchestraRunSummary? TransitionOrchestraRun(
        string runtimeId,
        string runId,
        string outcome,
        IReadOnlyList<OrchestraExecutionStepResult>? steps = null)
    {
        OrchestraRunSummary? updated = null;
        orchestraRuns.AddOrUpdate(
            runtimeId,
            _ => ImmutableQueue<OrchestraRunSummary>.Empty,
            (_, queue) => ImmutableQueue.CreateRange(queue.Select(run =>
            {
                if (!string.Equals(run.RunId, runId, StringComparison.OrdinalIgnoreCase))
                {
                    return run;
                }

                if (!CanTransitionOrchestraOutcome(run.Outcome, outcome))
                {
                    return run;
                }

                updated = run with
                {
                    Outcome = outcome,
                    Steps = steps ?? run.Steps,
                    CompletedAt = IsTerminalOrchestraOutcome(outcome) ? DateTimeOffset.UtcNow : null,
                };
                return updated;
            })));
        if (updated is not null)
        {
            PersistState();
        }
        return updated;
    }

    public OrchestraRunSummary RecordOrchestraRun(
        string runtimeId,
        string planId,
        string outcome,
        IReadOnlyList<OrchestraExecutionStepResult> steps,
        string? approvedBy = null,
        string? approvalNote = null,
        string? planRevision = null)
    {
        var now = DateTimeOffset.UtcNow;
        var run = new OrchestraRunSummary(
            $"orun_{Guid.NewGuid():N}",
            runtimeId,
            planId,
            outcome,
            now,
            steps,
            IsTerminalOrchestraOutcome(outcome) ? now : null,
            1,
            null,
            approvedBy,
            approvalNote,
            planRevision);
        orchestraRuns.AddOrUpdate(
            runtimeId,
            _ => ImmutableQueue<OrchestraRunSummary>.Empty.Enqueue(run),
            (_, queue) => TrimOrchestraRuns(queue.Enqueue(run)));
        PersistState();
        return run;
    }

    private void AppendOrchestraRun(string runtimeId, OrchestraRunSummary run)
    {
        orchestraRuns.AddOrUpdate(
            runtimeId,
            _ => ImmutableQueue<OrchestraRunSummary>.Empty.Enqueue(run),
            (_, queue) => TrimOrchestraRuns(queue.Enqueue(run)));
        PersistState();
    }

    internal static bool IsTerminalOrchestraOutcome(string outcome) =>
        string.Equals(outcome, "succeeded", StringComparison.OrdinalIgnoreCase)
        || string.Equals(outcome, "degraded", StringComparison.OrdinalIgnoreCase)
        || string.Equals(outcome, "failed", StringComparison.OrdinalIgnoreCase)
        || string.Equals(outcome, "cancelled", StringComparison.OrdinalIgnoreCase)
        || string.Equals(outcome, "ok", StringComparison.OrdinalIgnoreCase);

    internal static bool IsActiveOrchestraOutcome(string outcome) =>
        string.Equals(outcome, "queued", StringComparison.OrdinalIgnoreCase)
        || string.Equals(outcome, "running", StringComparison.OrdinalIgnoreCase);

    internal static bool IsRetryableOrchestraRun(OrchestraRunSummary run) =>
        IsTerminalOrchestraOutcome(run.Outcome)
        && !string.Equals(run.PlanId, "session_preparation", StringComparison.OrdinalIgnoreCase);

    internal static bool CanTransitionOrchestraOutcome(string current, string next)
    {
        if (string.Equals(current, next, StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }
        if (string.Equals(current, "queued", StringComparison.OrdinalIgnoreCase))
        {
            return string.Equals(next, "running", StringComparison.OrdinalIgnoreCase)
                || string.Equals(next, "cancelled", StringComparison.OrdinalIgnoreCase)
                || string.Equals(next, "failed", StringComparison.OrdinalIgnoreCase);
        }
        if (string.Equals(current, "running", StringComparison.OrdinalIgnoreCase))
        {
            return IsTerminalOrchestraOutcome(next);
        }
        return false;
    }

    internal static OrchestraRunSummary NormalizeRestoredOrchestraRun(OrchestraRunSummary run)
    {
        if (!string.Equals(run.Outcome, "queued", StringComparison.OrdinalIgnoreCase)
            && !string.Equals(run.Outcome, "running", StringComparison.OrdinalIgnoreCase))
        {
            return run;
        }

        return run with
        {
            Outcome = "failed",
            CompletedAt = DateTimeOffset.UtcNow,
            Steps = run.Steps.Concat(new[]
            {
                new OrchestraExecutionStepResult(
                    "service_restart",
                    "failed",
                    "service restart interrupted execution; retry explicitly if the plan is still applicable"),
            }).ToArray(),
        };
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
