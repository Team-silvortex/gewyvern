using System.Collections.Concurrent;
using Leserpent.ControlPlane;

namespace Leserpent;

public sealed class OrchestraExecutionCoordinator(
    RegistryService registry,
    IOrchestraPlanExecutor executor,
    ILogger<OrchestraExecutionCoordinator> logger) : IDisposable, IAsyncDisposable
{
    private static readonly TimeSpan ShutdownTimeout = TimeSpan.FromSeconds(5);
    private readonly ConcurrentDictionary<string, ActiveExecution> activeByRuntime = new(StringComparer.OrdinalIgnoreCase);
    private readonly object startGate = new();

    public (OrchestraRunSummary? Run, OrchestraRunSummary? ActiveRun, bool Replayed, bool RequestConflict, bool PersistenceFailed) TryStart(
        RuntimeSummary runtime,
        string planId,
        string planRevision,
        string approvedBy,
        string? approvalNote,
        string requestId,
        OrchestraRunSummary? retriedFrom = null)
    {
        lock (startGate)
        {
            var replay = registry.GetOrchestraRunByRequestId(runtime.RuntimeId, requestId);
            if (replay is not null)
            {
                var conflict = !string.Equals(replay.PlanId, planId, StringComparison.OrdinalIgnoreCase);
                return (conflict ? null : replay, null, !conflict, conflict, false);
            }

            var runId = $"orun_{Guid.NewGuid():N}";
            var cancellation = new CancellationTokenSource();
            var active = new ActiveExecution(runtime.RuntimeId, runId, cancellation);
            if (!activeByRuntime.TryAdd(runtime.RuntimeId, active))
            {
                cancellation.Dispose();
                var existing = activeByRuntime[runtime.RuntimeId];
                return (null, registry.GetOrchestraRun(runtime.RuntimeId, existing.RunId), false, false, false);
            }

            OrchestraRunSummary run;
            try
            {
                run = registry.StartOrchestraRun(
                    runId,
                    runtime.RuntimeId,
                    planId,
                    (retriedFrom?.Attempt ?? 0) + 1,
                    retriedFrom?.RunId,
                    approvedBy,
                    approvalNote,
                    planRevision,
                    requestId);
            }
            catch (Exception ex)
            {
                activeByRuntime.TryRemove(new KeyValuePair<string, ActiveExecution>(runtime.RuntimeId, active));
                cancellation.Dispose();
                logger.LogError(ex, "Failed to persist queued Orchestra run for runtime {RuntimeId}", runtime.RuntimeId);
                return (null, null, false, false, true);
            }
            active.ExecutionTask = RunAsync(runtime, run, active);
            return (run, null, false, false, false);
        }
    }

    public OrchestraRunSummary? Cancel(string runtimeId, string runId)
    {
        if (!activeByRuntime.TryGetValue(runtimeId, out var active)
            || !string.Equals(active.RunId, runId, StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }

        TryCancel(active);
        return registry.GetOrchestraRun(runtimeId, runId);
    }

    private async Task RunAsync(RuntimeSummary runtime, OrchestraRunSummary run, ActiveExecution active)
    {
        try
        {
            if (registry.TransitionOrchestraRun(runtime.RuntimeId, run.RunId, "running") is null)
            {
                logger.LogError(
                    "Orchestra run {RunId} could not persist its running state; execution was not started",
                    run.RunId);
                return;
            }
            var steps = await executor.ExecuteAsync(
                run.PlanId,
                runtime,
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
        CancelAllActiveRuns();
    }

    public async ValueTask DisposeAsync()
    {
        var active = activeByRuntime.Values.ToArray();
        CancelAllActiveRuns();
        if (active.Length == 0)
        {
            return;
        }

        try
        {
            await Task.WhenAll(active.Select(execution => execution.ExecutionTask)).WaitAsync(ShutdownTimeout);
        }
        catch (TimeoutException)
        {
            logger.LogWarning("Timed out waiting for {Count} Orchestra run(s) during shutdown", active.Length);
            foreach (var execution in active)
            {
                registry.TransitionOrchestraRun(
                    execution.RuntimeId,
                    execution.RunId,
                    "cancelled",
                    new[]
                    {
                        new OrchestraExecutionStepResult(
                            "shutdown_timeout",
                            "cancelled",
                            "service shutdown timed out while waiting for executor cancellation"),
                    });
            }
        }
    }

    private void CancelAllActiveRuns()
    {
        foreach (var active in activeByRuntime.Values)
        {
            TryCancel(active);
        }
    }

    private static void TryCancel(ActiveExecution active)
    {
        try
        {
            active.Cancellation.Cancel();
        }
        catch (ObjectDisposedException)
        {
            // Completion won the race; the persisted run is already terminal.
        }
    }

    private sealed class ActiveExecution(
        string runtimeId,
        string runId,
        CancellationTokenSource cancellation)
    {
        public string RuntimeId { get; } = runtimeId;
        public string RunId { get; } = runId;
        public CancellationTokenSource Cancellation { get; } = cancellation;
        public Task ExecutionTask { get; set; } = Task.CompletedTask;
    }
}
