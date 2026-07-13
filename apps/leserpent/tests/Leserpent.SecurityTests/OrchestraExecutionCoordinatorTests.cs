using Leserpent;
using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class OrchestraExecutionCoordinatorTests
{
    [Fact]
    public async Task CoordinatorEnforcesSingleActiveRunAndCancellationReachesTerminalState()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        var executor = new BlockingExecutor();
        using var coordinator = new OrchestraExecutionCoordinator(
            registry,
            executor,
            NullLogger<OrchestraExecutionCoordinator>.Instance);

        var first = coordinator.TryStart(runtime, "runtime_triage", "revision-1", "operator", "test cancellation", "request-cancel-1");
        Assert.NotNull(first.Run);
        await executor.Started.Task.WaitAsync(TimeSpan.FromSeconds(2));

        var second = coordinator.TryStart(runtime, "analysis_recovery", "revision-2", "operator", "must be rejected", "request-cancel-2");
        Assert.Null(second.Run);
        Assert.Equal(first.Run.RunId, second.ActiveRun?.RunId);

        Assert.NotNull(coordinator.Cancel(runtime.RuntimeId, first.Run.RunId));
        var terminal = await WaitForTerminalRunAsync(registry, runtime.RuntimeId, first.Run.RunId);
        Assert.Equal("cancelled", terminal.Outcome);
        Assert.Contains(terminal.Steps, step => step.Step == "cancel");

        DeleteStateFiles(statePath);
    }

    [Fact]
    public async Task CoordinatorArchivesExecutorFailure()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        using var coordinator = new OrchestraExecutionCoordinator(
            registry,
            new FailingExecutor(),
            NullLogger<OrchestraExecutionCoordinator>.Instance);

        var started = coordinator.TryStart(runtime, "runtime_triage", "revision-1", "automatic", null, "request-failure-1");
        Assert.NotNull(started.Run);

        var terminal = await WaitForTerminalRunAsync(registry, runtime.RuntimeId, started.Run.RunId);
        Assert.Equal("failed", terminal.Outcome);
        Assert.Contains(terminal.Steps, step => step.Summary.Contains("executor failed", StringComparison.Ordinal));

        DeleteStateFiles(statePath);
    }

    [Fact]
    public async Task AsyncShutdownWaitsForCancellationToBePersisted()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        var executor = new BlockingExecutor();
        var coordinator = new OrchestraExecutionCoordinator(
            registry,
            executor,
            NullLogger<OrchestraExecutionCoordinator>.Instance);

        var started = coordinator.TryStart(runtime, "runtime_triage", "revision-1", "operator", "shutdown test", "request-shutdown-1");
        Assert.NotNull(started.Run);
        await executor.Started.Task.WaitAsync(TimeSpan.FromSeconds(2));

        await coordinator.DisposeAsync();

        var run = registry.GetOrchestraRun(runtime.RuntimeId, started.Run.RunId);
        Assert.NotNull(run);
        Assert.Equal("cancelled", run.Outcome);
        Assert.NotNull(run.CompletedAt);
        DeleteStateFiles(statePath);
    }

    [Fact]
    public async Task CoordinatorReplaysSameRequestWithoutExecutingTwice()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        var executor = new CountingExecutor();
        using var coordinator = new OrchestraExecutionCoordinator(
            registry,
            executor,
            NullLogger<OrchestraExecutionCoordinator>.Instance);

        var first = coordinator.TryStart(runtime, "runtime_triage", "revision-1", "automatic", null, "request-idempotent-1");
        Assert.NotNull(first.Run);
        await WaitForTerminalRunAsync(registry, runtime.RuntimeId, first.Run.RunId);

        var replay = coordinator.TryStart(runtime, "runtime_triage", "revision-1", "automatic", null, "request-idempotent-1");
        var conflict = coordinator.TryStart(runtime, "analysis_recovery", "revision-2", "automatic", null, "request-idempotent-1");

        Assert.True(replay.Replayed);
        Assert.Equal(first.Run.RunId, replay.Run?.RunId);
        Assert.Equal(1, executor.CallCount);
        Assert.True(conflict.RequestConflict);
        Assert.Null(conflict.Run);
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void CoordinatorDoesNotExecuteWhenQueuedRunCannotBePersisted()
    {
        var (registry, statePath) = CreateRegistry(new FailingRunStore());
        var runtime = RegisterRuntime(registry);
        var executor = new CountingExecutor();
        using var coordinator = new OrchestraExecutionCoordinator(
            registry,
            executor,
            NullLogger<OrchestraExecutionCoordinator>.Instance);

        var first = coordinator.TryStart(runtime, "runtime_triage", "revision-1", "automatic", null, "request-persist-fail-1");
        var second = coordinator.TryStart(runtime, "runtime_triage", "revision-1", "automatic", null, "request-persist-fail-2");

        Assert.True(first.PersistenceFailed);
        Assert.True(second.PersistenceFailed);
        Assert.Equal(0, executor.CallCount);
        Assert.Empty(registry.ListOrchestraRuns(runtime.RuntimeId));
        DeleteStateFiles(statePath);
    }

    [Fact]
    public async Task CoordinatorDoesNotExecuteWhenRunningTransitionCannotBePersisted()
    {
        var runStore = new TransitionFailingRunStore();
        var (registry, statePath) = CreateRegistry(runStore);
        var runtime = RegisterRuntime(registry);
        var executor = new CountingExecutor();
        using var coordinator = new OrchestraExecutionCoordinator(
            registry,
            executor,
            NullLogger<OrchestraExecutionCoordinator>.Instance);

        var started = coordinator.TryStart(
            runtime,
            "runtime_triage",
            "revision-1",
            "automatic",
            null,
            "request-transition-fail-1");
        await Task.Delay(50);

        Assert.NotNull(started.Run);
        Assert.Equal(0, executor.CallCount);
        Assert.Equal("queued", registry.GetOrchestraRun(runtime.RuntimeId, started.Run.RunId)?.Outcome);
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void GuidedRunIsNotPublishedWhenPersistenceFails()
    {
        var (registry, statePath) = CreateRegistry(new FailingRunStore());
        var runtime = RegisterRuntime(registry);

        Assert.Throws<OrchestraPersistenceException>(() => registry.RecordOrchestraRun(
            runtime.RuntimeId,
            "session_preparation",
            "ok",
            Array.Empty<OrchestraExecutionStepResult>()));

        Assert.Empty(registry.ListOrchestraRuns(runtime.RuntimeId));
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void ImportRestoresPreviousRegistryWhenOrchestraReplacementFails()
    {
        var (registry, statePath) = CreateRegistry(new FailingRunStore());
        var runtime = RegisterRuntime(registry);
        var current = registry.ExportState();
        var replacement = current with
        {
            Runtimes = current.Runtimes
                .Select(item => item with { Name = "replacement-runtime" })
                .ToArray(),
        };

        Assert.Throws<OrchestraPersistenceException>(() => registry.ImportState(replacement));

        Assert.Equal("runtime", registry.GetRuntime(runtime.RuntimeId)?.Name);
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void RuntimeDeleteKeepsRegistryIntactWhenAuditCleanupFails()
    {
        var (registry, statePath) = CreateRegistry(new DeleteFailingRunStore());
        var runtime = RegisterRuntime(registry);
        var session = registry.CreateSession(new SessionCreateRequest(
            runtime.RuntimeId,
            "diagnostic",
            "operator",
            Array.Empty<SessionCapabilityRequirement>())).Session;
        var run = registry.RecordOrchestraRun(
            runtime.RuntimeId,
            "session_preparation",
            "ok",
            Array.Empty<OrchestraExecutionStepResult>());

        Assert.Throws<OrchestraPersistenceException>(() => registry.DeleteRuntime(runtime.RuntimeId));

        Assert.NotNull(registry.GetRuntime(runtime.RuntimeId));
        Assert.NotNull(registry.GetSession(session!.SessionId));
        Assert.NotNull(registry.GetOrchestraRun(runtime.RuntimeId, run.RunId));
        DeleteStateFiles(statePath);
    }

    [Fact]
    public async Task RuntimeDeleteRejectsActiveOrchestraRun()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        var executor = new BlockingExecutor();
        using var coordinator = new OrchestraExecutionCoordinator(
            registry,
            executor,
            NullLogger<OrchestraExecutionCoordinator>.Instance);
        var started = coordinator.TryStart(
            runtime,
            "runtime_triage",
            "revision-1",
            "automatic",
            null,
            "request-delete-active-1");
        await executor.Started.Task.WaitAsync(TimeSpan.FromSeconds(2));

        var conflict = Assert.Throws<OrchestraRuntimeBusyException>(() =>
            registry.DeleteRuntime(runtime.RuntimeId));

        Assert.Contains(conflict.ActiveRuns, item => item.RunId == started.Run!.RunId);
        Assert.NotNull(registry.GetRuntime(runtime.RuntimeId));
        coordinator.Cancel(runtime.RuntimeId, started.Run!.RunId);
        await WaitForTerminalRunAsync(registry, runtime.RuntimeId, started.Run.RunId);
        DeleteStateFiles(statePath);
    }

    [Fact]
    public async Task BatchDeleteIsAtomicWhenSelectionContainsActiveRun()
    {
        var (registry, statePath) = CreateRegistry();
        var activeRuntime = RegisterRuntime(registry);
        var idleRuntime = RegisterRuntime(registry);
        var executor = new BlockingExecutor();
        using var coordinator = new OrchestraExecutionCoordinator(
            registry,
            executor,
            NullLogger<OrchestraExecutionCoordinator>.Instance);
        var started = coordinator.TryStart(
            activeRuntime,
            "runtime_triage",
            "revision-1",
            "automatic",
            null,
            "request-batch-delete-active-1");
        await executor.Started.Task.WaitAsync(TimeSpan.FromSeconds(2));

        Assert.Throws<OrchestraRuntimeBusyException>(() => registry.DeleteRuntimes());

        Assert.NotNull(registry.GetRuntime(activeRuntime.RuntimeId));
        Assert.NotNull(registry.GetRuntime(idleRuntime.RuntimeId));
        coordinator.Cancel(activeRuntime.RuntimeId, started.Run!.RunId);
        await WaitForTerminalRunAsync(registry, activeRuntime.RuntimeId, started.Run.RunId);
        DeleteStateFiles(statePath);
    }

    private static async Task<OrchestraRunSummary> WaitForTerminalRunAsync(
        RegistryService registry,
        string runtimeId,
        string runId)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(3);
        while (DateTimeOffset.UtcNow < deadline)
        {
            var run = registry.GetOrchestraRun(runtimeId, runId);
            if (run is not null && RegistryService.IsTerminalOrchestraOutcome(run.Outcome))
            {
                return run;
            }
            await Task.Delay(10);
        }
        throw new TimeoutException($"run {runId} did not reach a terminal state");
    }

    private static (RegistryService Registry, string StatePath) CreateRegistry(IOrchestraRunStore? runStore = null)
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-orchestra-test-{Guid.NewGuid():N}.json");
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?> { ["LESERPENT_STATE_PATH"] = statePath })
            .Build();
        var environment = new TestHostEnvironment { ContentRootPath = Path.GetDirectoryName(statePath)! };
        var store = new ControlPlaneStateStore(
            configuration,
            environment,
            NullLogger<ControlPlaneStateStore>.Instance);
        return (new RegistryService(store, runStore ?? new InMemoryOrchestraRunStore()), statePath);
    }

    private static RuntimeSummary RegisterRuntime(RegistryService registry)
    {
        var registered = registry.RegisterRuntime(new RuntimeRegistrationRequest(
            "runtime",
            "http://127.0.0.1:49152",
            "pairing-token"));
        return registry.GetRuntime(registered.RuntimeId)!;
    }

    private static void DeleteStateFiles(string statePath)
    {
        File.Delete(statePath);
        File.Delete($"{statePath}.bak");
        File.Delete($"{statePath}.tmp");
    }

    private sealed class BlockingExecutor : IOrchestraPlanExecutor
    {
        public TaskCompletionSource Started { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);

        public async Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteAsync(
            string planId,
            RuntimeSummary runtime,
            CancellationToken cancellationToken)
        {
            Started.TrySetResult();
            await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
            return Array.Empty<OrchestraExecutionStepResult>();
        }
    }

    private sealed class FailingExecutor : IOrchestraPlanExecutor
    {
        public Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteAsync(
            string planId,
            RuntimeSummary runtime,
            CancellationToken cancellationToken) =>
            throw new InvalidOperationException("executor failed intentionally");
    }

    private sealed class CountingExecutor : IOrchestraPlanExecutor
    {
        private int callCount;
        public int CallCount => callCount;

        public Task<IReadOnlyList<OrchestraExecutionStepResult>> ExecuteAsync(
            string planId,
            RuntimeSummary runtime,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref callCount);
            return Task.FromResult<IReadOnlyList<OrchestraExecutionStepResult>>(new[]
            {
                new OrchestraExecutionStepResult("execute", "ok", "completed"),
            });
        }
    }

    private sealed class FailingRunStore : IOrchestraRunStore
    {
        public string Provider => "failing-test";
        public string Location => "test";
        public int SchemaVersion => 0;
        public string? LastError => "write failed";
        public IReadOnlyList<OrchestraRunSummary> LoadAll() => Array.Empty<OrchestraRunSummary>();
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) => Array.Empty<OrchestraRunEvent>();
        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null) => false;
        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs) => false;
        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds) => false;
    }

    private sealed class TransitionFailingRunStore : IOrchestraRunStore
    {
        private readonly InMemoryOrchestraRunStore inner = new();
        private int writeCount;

        public string Provider => "transition-failing-test";
        public string Location => "test";
        public int SchemaVersion => 0;
        public string? LastError => writeCount > 1 ? "write failed" : null;
        public IReadOnlyList<OrchestraRunSummary> LoadAll() => inner.LoadAll();
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) => inner.LoadEvents(runtimeId, runId);
        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null) =>
            Interlocked.Increment(ref writeCount) == 1 && inner.Upsert(run, eventRecord);
        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs) => inner.ReplaceAll(runs);
        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds) => inner.DeleteRuntimes(runtimeIds);
    }

    private sealed class DeleteFailingRunStore : IOrchestraRunStore
    {
        private readonly InMemoryOrchestraRunStore inner = new();

        public string Provider => "delete-failing-test";
        public string Location => "test";
        public int SchemaVersion => 0;
        public string? LastError => "delete failed";
        public IReadOnlyList<OrchestraRunSummary> LoadAll() => inner.LoadAll();
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) => inner.LoadEvents(runtimeId, runId);
        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null) => inner.Upsert(run, eventRecord);
        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs) => inner.ReplaceAll(runs);
        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds) => false;
    }

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = string.Empty;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
