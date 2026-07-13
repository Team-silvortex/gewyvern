using System.Collections.Concurrent;
using Leserpent.ControlPlane;

namespace Leserpent;

public sealed class OrchestraExecutionCoordinator(
    RegistryService registry,
    CapabilityDiscoveryService discovery,
    ILogger<OrchestraExecutionCoordinator> logger) : IDisposable
{
    private readonly ConcurrentDictionary<string, ActiveExecution> activeByRuntime = new(StringComparer.OrdinalIgnoreCase);

    public (OrchestraRunSummary? Run, OrchestraRunSummary? ActiveRun) TryStart(
        RuntimeSummary runtime,
        string planId,
        string planRevision,
        string approvedBy,
        string? approvalNote,
        OrchestraRunSummary? retriedFrom = null)
    {
        var runId = $"orun_{Guid.NewGuid():N}";
        var cancellation = new CancellationTokenSource();
        var active = new ActiveExecution(runId, cancellation);
        if (!activeByRuntime.TryAdd(runtime.RuntimeId, active))
        {
            cancellation.Dispose();
            var existing = activeByRuntime[runtime.RuntimeId];
            return (null, registry.GetOrchestraRun(runtime.RuntimeId, existing.RunId));
        }

        var run = registry.StartOrchestraRun(
            runId,
            runtime.RuntimeId,
            planId,
            (retriedFrom?.Attempt ?? 0) + 1,
            retriedFrom?.RunId,
            approvedBy,
            approvalNote,
            planRevision);
        _ = RunAsync(runtime, run, active);
        return (run, null);
    }

    public OrchestraRunSummary? Cancel(string runtimeId, string runId)
    {
        if (!activeByRuntime.TryGetValue(runtimeId, out var active)
            || !string.Equals(active.RunId, runId, StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }

        active.Cancellation.Cancel();
        return registry.GetOrchestraRun(runtimeId, runId);
    }

    private async Task RunAsync(RuntimeSummary runtime, OrchestraRunSummary run, ActiveExecution active)
    {
        registry.TransitionOrchestraRun(runtime.RuntimeId, run.RunId, "running");
        try
        {
            var steps = await Program.ExecuteOrchestraPlanAsync(
                run.PlanId,
                runtime,
                registry,
                discovery,
                active.Cancellation.Token);
            var outcome = steps.All(step => string.Equals(step.Outcome, "ok", StringComparison.OrdinalIgnoreCase))
                ? "succeeded"
                : "degraded";
            registry.RecordRecoveryActivity(
                runtime.RuntimeId,
                $"orchestra:{run.PlanId}",
                outcome,
                string.Join("; ", steps.Select(step => step.Summary)));
            registry.TransitionOrchestraRun(runtime.RuntimeId, run.RunId, outcome, steps);
        }
        catch (OperationCanceledException) when (active.Cancellation.IsCancellationRequested)
        {
            registry.TransitionOrchestraRun(
                runtime.RuntimeId,
                run.RunId,
                "cancelled",
                new[] { new OrchestraExecutionStepResult("cancel", "cancelled", "execution cancelled by operator") });
        }
        catch (Exception ex)
        {
            logger.LogError(ex, "Orchestra run {RunId} failed for runtime {RuntimeId}", run.RunId, runtime.RuntimeId);
            registry.TransitionOrchestraRun(
                runtime.RuntimeId,
                run.RunId,
                "failed",
                new[] { new OrchestraExecutionStepResult("execute", "failed", ex.Message) });
        }
        finally
        {
            activeByRuntime.TryRemove(new KeyValuePair<string, ActiveExecution>(runtime.RuntimeId, active));
            active.Cancellation.Dispose();
        }
    }

    public void Dispose()
    {
        foreach (var active in activeByRuntime.Values)
        {
            active.Cancellation.Cancel();
        }
        activeByRuntime.Clear();
    }

    private sealed record ActiveExecution(string RunId, CancellationTokenSource Cancellation);
}
