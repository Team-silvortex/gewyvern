using System.Collections.Concurrent;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json;
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
        Assert.Contains(
            terminal.Steps,
            step => step.Summary == "orchestra execution failed");
        Assert.DoesNotContain(
            terminal.Steps,
            step => step.Summary.Contains(
                "executor failed",
                StringComparison.Ordinal));

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

    [Fact]
    public void RuntimeDeletionIntentSurvivesRestartAndBlocksNewWorkUntilCompleted()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);

        using (var reservation = registry.ReserveRuntimeDeletion(new[] { runtime.RuntimeId }))
        {
            Assert.Equal(new[] { runtime.RuntimeId }, reservation.RuntimeIds);
            Assert.Throws<InvalidOperationException>(() =>
                registry.StartOrchestraRun(
                    "orun-reserved",
                    runtime.RuntimeId,
                    "runtime_triage"));
            Assert.Throws<RuntimeDeletionInProgressException>(() =>
                registry.ReserveRuntimeDeletion(new[] { runtime.RuntimeId }));
            Assert.Equal(
                runtime.RuntimeId,
                registry.CreateSession(new SessionCreateRequest(
                    runtime.RuntimeId,
                    "diagnostic",
                    "operator",
                    Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);
        }

        Assert.Single(registry.ListPendingRuntimeDeletions());
        Assert.Throws<InvalidOperationException>(() =>
            registry.StartOrchestraRun(
                "orun-after-reservation",
                runtime.RuntimeId,
                "runtime_triage"));

        var restarted = CreateRegistry(statePath);
        Assert.Single(restarted.ListPendingRuntimeDeletions());
        Assert.NotNull(restarted.GetRuntime(runtime.RuntimeId));
        Assert.Equal(
            runtime.RuntimeId,
            restarted.CreateSession(new SessionCreateRequest(
                runtime.RuntimeId,
                "diagnostic",
                "operator",
                Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);

        using (var recovered = Assert.Single(restarted.ClaimPendingRuntimeDeletions()))
        {
            var deleted = restarted.DeleteRuntimesById(recovered.RuntimeIds);
            Assert.Equal(1, deleted.RemovedRuntimeCount);
            restarted.CompleteRuntimeDeletion(recovered);
        }

        var converged = CreateRegistry(statePath);
        Assert.Null(converged.GetRuntime(runtime.RuntimeId));
        Assert.Empty(converged.ListPendingRuntimeDeletions());
        DeleteStateFiles(statePath);
    }

    [Fact]
    public async Task RuntimeDeletionRecoveryServiceConvergesPendingIntent()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        registry.ReserveRuntimeDeletion(new[] { runtime.RuntimeId }).Dispose();
        var authority = new RecordingRegistrationAuthority();
        var service = new RuntimeDeletionRecoveryService(
            registry,
            authority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);

        try
        {
            await service.StartAsync(CancellationToken.None);
            var deadline = DateTimeOffset.UtcNow.AddSeconds(2);
            while (registry.ListPendingRuntimeDeletions().Count > 0 &&
                   DateTimeOffset.UtcNow < deadline)
            {
                await Task.Delay(10);
            }

            Assert.Empty(registry.ListPendingRuntimeDeletions());
            Assert.Null(registry.GetRuntime(runtime.RuntimeId));
            Assert.Equal(new[] { runtime.RuntimeId }, authority.UnregisteredRuntimeIds);
        }
        finally
        {
            await service.StopAsync(CancellationToken.None);
            service.Dispose();
            DeleteStateFiles(statePath);
        }
    }

    [Theory]
    [InlineData(true, 0)]
    [InlineData(false, 1)]
    public async Task RuntimeDeletionRecoveryLooksUpStableReceiptBeforeMutation(
        bool receiptExists,
        int expectedMutationCount)
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        registry.ReserveRuntimeDeletion(
            new[] { runtime.RuntimeId }).Dispose();
        var intent = Assert.Single(
            registry.ListPendingRuntimeDeletions());
        var authority = new ReceiptAwareRegistrationAuthority(
            receiptExists
                ? new[] { runtime.RuntimeId }
                : null);
        var service = new RuntimeDeletionRecoveryService(
            registry,
            authority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);

        try
        {
            await service.StartAsync(CancellationToken.None);
            await WaitForPendingDeletionCountAsync(registry, 0);

            Assert.Equal(1, authority.LookupCount);
            Assert.Equal(
                expectedMutationCount,
                authority.MutationCount);
            Assert.Equal(
                intent.UnregistrationCommandId,
                authority.ObservedCommandId);
            Assert.Null(registry.GetRuntime(runtime.RuntimeId));

            await service.StopAsync(CancellationToken.None);
            var restarted = CreateRegistry(statePath);
            Assert.Empty(
                restarted.ListPendingRuntimeDeletions());
            Assert.Null(restarted.GetRuntime(runtime.RuntimeId));
        }
        finally
        {
            await service.StopAsync(CancellationToken.None);
            service.Dispose();
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RuntimeDeletionRecoveryRejectsMismatchedReceiptTargets()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        registry.ReserveRuntimeDeletion(
            new[] { runtime.RuntimeId }).Dispose();
        var authority = new ReceiptAwareRegistrationAuthority(
            new[] { "runtime-different" });
        var service = new RuntimeDeletionRecoveryService(
            registry,
            authority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);

        try
        {
            await service.StartAsync(CancellationToken.None);
            var deadline = DateTimeOffset.UtcNow.AddSeconds(2);
            while (authority.LookupCount == 0 &&
                   DateTimeOffset.UtcNow < deadline)
            {
                await Task.Delay(10);
            }
            await service.StopAsync(CancellationToken.None);

            Assert.Equal(1, authority.LookupCount);
            Assert.Equal(0, authority.MutationCount);
            Assert.NotNull(registry.GetRuntime(runtime.RuntimeId));
            var intent = Assert.Single(
                registry.ListPendingRuntimeDeletions());
            Assert.Equal(1, intent.AttemptCount);
            Assert.Equal(
                RuntimeDeletionFailureCodes.AuthorityFailure,
                intent.LastFailureCode);
        }
        finally
        {
            service.Dispose();
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RuntimeDeletionRecoveryPersistsReplayFloorBeforeMutation()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        registry.ReserveRuntimeDeletion(
            new[] { runtime.RuntimeId }).Dispose();
        var authority = new ReplayHorizonRegistrationAuthority(
            ReplayHorizon(nextGeneration: 5));
        var service = new RuntimeDeletionRecoveryService(
            registry,
            authority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);

        try
        {
            await service.StartAsync(CancellationToken.None);
            await authority.MutationStarted.WaitAsync(
                TimeSpan.FromSeconds(2));

            var fenced = Assert.Single(
                registry.ListPendingRuntimeDeletions());
            Assert.True(
                fenced.UnregistrationMutationMayHaveStarted);
            Assert.Equal(
                (ulong)5,
                fenced.UnregistrationReplayHorizonFloor);
            Assert.Equal(2, fenced.Revision);

            var diskReloaded = CreateRegistry(statePath);
            var persisted = Assert.Single(
                diskReloaded.ListPendingRuntimeDeletions());
            Assert.True(
                persisted.UnregistrationMutationMayHaveStarted);
            Assert.Equal(
                (ulong)5,
                persisted.UnregistrationReplayHorizonFloor);
            Assert.Equal(2, persisted.Revision);

            authority.ReleaseMutation();
            await WaitForPendingDeletionCountAsync(registry, 0);
        }
        finally
        {
            authority.ReleaseMutation();
            await service.StopAsync(CancellationToken.None);
            service.Dispose();
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RuntimeDeletionReplayFloorPersistenceFailurePreventsMutation()
    {
        var (registry, statePath) = CreateRegistry();
        var backupPath = $"{statePath}.bak";
        var runtime = RegisterRuntime(registry);
        using var reservation = registry.ReserveRuntimeDeletion(
            new[] { runtime.RuntimeId });
        var authority = new ReplayHorizonRegistrationAuthority(
            ReplayHorizon(nextGeneration: 5));
        File.Delete(backupPath);
        Directory.CreateDirectory(backupPath);

        try
        {
            await Assert.ThrowsAsync<OrchestraPersistenceException>(
                () => RuntimeDeletionAuthorityWorkflow.ExecuteAsync(
                    registry,
                    reservation,
                    authority,
                    CancellationToken.None));

            Assert.Equal(0, authority.MutationCount);
            Assert.False(
                reservation.UnregistrationMutationMayHaveStarted);
            Assert.Null(
                reservation.UnregistrationReplayHorizonFloor);
            var intent = Assert.Single(
                registry.ListPendingRuntimeDeletions());
            Assert.False(
                intent.UnregistrationMutationMayHaveStarted);
            Assert.Null(
                intent.UnregistrationReplayHorizonFloor);
            Assert.Equal(1, intent.Revision);
            var diskIntent = Assert.Single(
                CreateRegistry(statePath)
                    .ListPendingRuntimeDeletions());
            Assert.False(
                diskIntent.UnregistrationMutationMayHaveStarted);
            Assert.Null(
                diskIntent.UnregistrationReplayHorizonFloor);
        }
        finally
        {
            Directory.Delete(backupPath);
            DeleteStateFiles(statePath);
        }
    }

    [Theory]
    [InlineData(5, 4, true)]
    [InlineData(6, 5, false)]
    [InlineData(4, 3, false)]
    public async Task RuntimeDeletionRecoveryRejectsOnlyEvictedReplayFloor(
        ulong nextGeneration,
        ulong evictedThroughGeneration,
        bool mutationExpected)
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        using (var reservation = registry.ReserveRuntimeDeletion(
            new[] { runtime.RuntimeId }))
        {
            registry.FenceRuntimeDeletionMutation(
                reservation,
                replayHorizonFloor: 5);
        }
        var authority = new ReplayHorizonRegistrationAuthority(
            new RuntimeUnregistrationReplayHorizon(
                256,
                0,
                null,
                null,
                nextGeneration,
                evictedThroughGeneration));
        var service = new RuntimeDeletionRecoveryService(
            registry,
            authority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);

        try
        {
            await service.StartAsync(CancellationToken.None);
            if (mutationExpected)
            {
                await authority.MutationStarted.WaitAsync(
                    TimeSpan.FromSeconds(2));
                authority.ReleaseMutation();
                await WaitForPendingDeletionCountAsync(registry, 0);
                Assert.Equal(1, authority.MutationCount);
                Assert.Null(registry.GetRuntime(runtime.RuntimeId));
            }
            else
            {
                var deadline = DateTimeOffset.UtcNow.AddSeconds(2);
                PersistedRuntimeDeletionIntent? failed = null;
                while (DateTimeOffset.UtcNow < deadline)
                {
                    failed = registry.ListPendingRuntimeDeletions()
                        .SingleOrDefault();
                    if (failed?.AttemptCount == 1)
                    {
                        break;
                    }
                    await Task.Delay(10);
                }
                Assert.NotNull(failed);
                Assert.Equal(1, failed.AttemptCount);
                Assert.Equal(
                    RuntimeDeletionFailureCodes.ReplayAmbiguous,
                    failed.LastFailureCode);
                Assert.Equal((ulong)5,
                    failed.UnregistrationReplayHorizonFloor);
                Assert.True(
                    failed.UnregistrationMutationMayHaveStarted);
                Assert.Equal(3, failed.Revision);
                Assert.Equal(0, authority.MutationCount);
                Assert.NotNull(registry.GetRuntime(runtime.RuntimeId));
            }
        }
        finally
        {
            authority.ReleaseMutation();
            await service.StopAsync(CancellationToken.None);
            service.Dispose();
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void RuntimeDeletionRecoveryClaimsAndPersistsABoundedSuccessBatch()
    {
        var runStore = new CountingDeleteRunStore();
        var (registry, statePath) = CreateRegistry(runStore);
        var runtimeIds = Enumerable.Range(0, 3)
            .Select(index => $"runtime-batch-{index}")
            .ToArray();
        foreach (var runtimeId in runtimeIds)
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    $"Runtime {runtimeId}",
                    $"https://{runtimeId}.example",
                    "pairing-token"),
                runtimeId);
            registry.ReserveRuntimeDeletion(new[] { runtimeId }).Dispose();
        }

        var reservations = registry.ClaimPendingRuntimeDeletions(2);
        var claimedRuntimeIds = reservations
            .SelectMany(reservation => reservation.RuntimeIds)
            .OrderBy(static runtimeId => runtimeId, StringComparer.Ordinal)
            .ToArray();
        var remainingRuntimeId = Assert.Single(runtimeIds.Except(
            claimedRuntimeIds,
            StringComparer.Ordinal));
        try
        {
            Assert.Equal(2, reservations.Count);
            registry.CompleteRecoveredRuntimeDeletions(reservations);
        }
        finally
        {
            foreach (var reservation in reservations)
            {
                reservation.Dispose();
            }
        }

        Assert.Equal(1, runStore.DeleteCount);
        Assert.Equal(
            claimedRuntimeIds,
            runStore.LastDeletedRuntimeIds
                .OrderBy(static runtimeId => runtimeId, StringComparer.Ordinal));
        Assert.Equal(new[] { remainingRuntimeId }, registry
            .ListPendingRuntimeDeletions()
            .SelectMany(intent => intent.RuntimeIds));
        foreach (var runtimeId in claimedRuntimeIds)
        {
            Assert.Null(registry.GetRuntime(runtimeId));
        }
        Assert.NotNull(registry.GetRuntime(remainingRuntimeId));
        var reloaded = CreateRegistry(statePath);
        Assert.Equal(new[] { remainingRuntimeId }, reloaded
            .ListPendingRuntimeDeletions()
            .SelectMany(intent => intent.RuntimeIds));
        foreach (var runtimeId in claimedRuntimeIds)
        {
            Assert.Null(reloaded.GetRuntime(runtimeId));
        }
        Assert.NotNull(reloaded.GetRuntime(remainingRuntimeId));
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void RecoveredRuntimeDeletionBatchRollsBackEveryProjectionAndReplays()
    {
        var runStore = new CountingDeleteRunStore();
        var (registry, statePath) = CreateRegistry(runStore);
        var backupPath = $"{statePath}.bak";
        var runtimeIds = new[] { "runtime-batch-rollback-a", "runtime-batch-rollback-b" };
        var sessionIds = new Dictionary<string, string>(StringComparer.Ordinal);
        var runIds = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var runtimeId in runtimeIds)
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    $"Runtime {runtimeId}",
                    $"https://{runtimeId}.example",
                    "pairing-token"),
                runtimeId);
            sessionIds[runtimeId] = registry.CreateSession(new SessionCreateRequest(
                runtimeId,
                "diagnostic",
                "rollback-test",
                Array.Empty<SessionCapabilityRequirement>())).Session!.SessionId;
            runIds[runtimeId] = registry.RecordOrchestraRun(
                runtimeId,
                "session_preparation",
                "ok",
                Array.Empty<OrchestraExecutionStepResult>()).RunId;
            registry.RecordRecoveryActivity(
                runtimeId,
                "refresh_status",
                "network_failed",
                "retained rollback marker");
            registry.ReserveRuntimeDeletion(new[] { runtimeId }).Dispose();
        }

        var reservations = registry.ClaimPendingRuntimeDeletions();
        File.Delete(backupPath);
        Directory.CreateDirectory(backupPath);
        try
        {
            Assert.Throws<OrchestraPersistenceException>(() =>
                registry.CompleteRecoveredRuntimeDeletions(reservations));

            Assert.Equal(1, runStore.DeleteCount);
            Assert.Empty(runStore.LoadAll());
            Assert.Equal(2, registry.ListPendingRuntimeDeletions().Count);
            foreach (var runtimeId in runtimeIds)
            {
                Assert.NotNull(registry.GetRuntime(runtimeId));
                Assert.NotNull(registry.GetSession(sessionIds[runtimeId]));
                Assert.NotNull(registry.GetOrchestraRun(runtimeId, runIds[runtimeId]));
                Assert.Contains(
                    registry.GetRuntimeAttention(runtimeId)!.RecentRecoveryActivities,
                    activity => activity.Summary == "retained rollback marker");
                Assert.Equal(
                    runtimeId,
                    registry.CreateSession(new SessionCreateRequest(
                        runtimeId,
                        "diagnostic",
                        "rollback-test",
                        Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);
            }

            var diskReloaded = CreateRegistry(statePath);
            Assert.Equal(2, diskReloaded.ListPendingRuntimeDeletions().Count);
            foreach (var runtimeId in runtimeIds)
            {
                Assert.NotNull(diskReloaded.GetRuntime(runtimeId));
                Assert.NotNull(diskReloaded.GetSession(sessionIds[runtimeId]));
                Assert.NotNull(diskReloaded.GetOrchestraRun(runtimeId, runIds[runtimeId]));
            }
        }
        finally
        {
            foreach (var reservation in reservations)
            {
                reservation.Dispose();
            }
            Directory.Delete(backupPath);
        }

        var replayReservations = registry.ClaimPendingRuntimeDeletions();
        try
        {
            registry.CompleteRecoveredRuntimeDeletions(replayReservations);
        }
        finally
        {
            foreach (var reservation in replayReservations)
            {
                reservation.Dispose();
            }
        }

        Assert.Equal(2, runStore.DeleteCount);
        Assert.Empty(registry.ListPendingRuntimeDeletions());
        foreach (var runtimeId in runtimeIds)
        {
            Assert.Null(registry.GetRuntime(runtimeId));
            Assert.Null(registry.GetSession(sessionIds[runtimeId]));
            Assert.Null(registry.GetOrchestraRun(runtimeId, runIds[runtimeId]));
        }
        var convergedReload = CreateRegistry(statePath);
        Assert.Empty(convergedReload.ListPendingRuntimeDeletions());
        foreach (var runtimeId in runtimeIds)
        {
            Assert.Null(convergedReload.GetRuntime(runtimeId));
            Assert.Null(convergedReload.GetSession(sessionIds[runtimeId]));
            Assert.Null(convergedReload.GetOrchestraRun(runtimeId, runIds[runtimeId]));
        }
        DeleteStateFiles(statePath);
    }

    [Theory]
    [InlineData(1, 1)]
    [InlineData(2, 2)]
    [InlineData(3, 4)]
    [InlineData(4, 8)]
    [InlineData(5, 16)]
    [InlineData(6, 30)]
    [InlineData(100, 30)]
    public void RuntimeDeletionRetryBackoffIsBounded(
        int attemptCount,
        int expectedSeconds)
    {
        Assert.Equal(
            TimeSpan.FromSeconds(expectedSeconds),
            RegistryService.CalculateRuntimeDeletionRetryDelay(attemptCount));
    }

    [Fact]
    public void RuntimeDeletionRetryMetadataSurvivesRestartAndDefersOnlyFailedIntent()
    {
        var (registry, statePath) = CreateRegistry();
        const string failedRuntimeId = "runtime-retry-failed";
        const string healthyRuntimeId = "runtime-retry-healthy";
        registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Retry failed runtime",
                "https://retry-failed.example",
                "pairing-token"),
            failedRuntimeId);
        registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Retry healthy runtime",
                "https://retry-healthy.example",
                "pairing-token"),
            healthyRuntimeId);
        var failedRuntime = registry.GetRuntime(failedRuntimeId)!;
        var healthyRuntime = registry.GetRuntime(healthyRuntimeId)!;
        registry.ReserveRuntimeDeletion(new[] { failedRuntime.RuntimeId }).Dispose();
        registry.ReserveRuntimeDeletion(new[] { healthyRuntime.RuntimeId }).Dispose();
        var attemptedAt = DateTimeOffset.UtcNow;
        var reservations = registry.ClaimPendingRuntimeDeletions(
            2,
            attemptedAt);
        var failedReservation = reservations.Single(reservation =>
            reservation.RuntimeIds.Contains(
                failedRuntime.RuntimeId,
                StringComparer.Ordinal));
        var healthyReservation = reservations.Single(reservation =>
            reservation.RuntimeIds.Contains(
                healthyRuntime.RuntimeId,
                StringComparer.Ordinal));
        try
        {
            registry.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        failedReservation,
                        RuntimeDeletionFailureCodes.AuthorityTimeout,
                        attemptedAt),
                });
            registry.CompleteRecoveredRuntimeDeletions(
                new[] { healthyReservation });
        }
        finally
        {
            foreach (var reservation in reservations)
            {
                reservation.Dispose();
            }
        }

        var intent = Assert.Single(registry.ListPendingRuntimeDeletions());
        Assert.Equal(1, intent.AttemptCount);
        Assert.Equal(attemptedAt, intent.LastAttemptAt);
        Assert.Equal(attemptedAt.AddSeconds(1), intent.NextAttemptAt);
        Assert.Equal(
            RuntimeDeletionFailureCodes.AuthorityTimeout,
            intent.LastFailureCode);
        using (var operatorPayload = JsonDocument.Parse(
            JsonSerializer.Serialize(
                new[] { intent },
                LeserpentJsonContext.Default
                    .PersistedRuntimeDeletionIntentArray)))
        {
            var visibleIntent = operatorPayload.RootElement[0];
            Assert.Equal(1, visibleIntent
                .GetProperty("attemptCount")
                .GetInt32());
            Assert.Equal(
                attemptedAt,
                visibleIntent
                    .GetProperty("lastAttemptAt")
                    .GetDateTimeOffset());
            Assert.Equal(
                attemptedAt.AddSeconds(1),
                visibleIntent
                    .GetProperty("nextAttemptAt")
                    .GetDateTimeOffset());
            Assert.Equal(
                RuntimeDeletionFailureCodes.AuthorityTimeout,
                visibleIntent
                    .GetProperty("lastFailureCode")
                    .GetString());
        }
        Assert.Empty(registry.ClaimPendingRuntimeDeletions(
            1,
            attemptedAt.AddMilliseconds(999)));

        var restarted = CreateRegistry(statePath);
        var restoredIntent = Assert.Single(
            restarted.ListPendingRuntimeDeletions());
        Assert.Equal(intent.IntentId, restoredIntent.IntentId);
        Assert.Equal(intent.RuntimeIds, restoredIntent.RuntimeIds);
        Assert.Equal(intent.PreparedAt, restoredIntent.PreparedAt);
        Assert.Equal(intent.AttemptCount, restoredIntent.AttemptCount);
        Assert.Equal(intent.LastAttemptAt, restoredIntent.LastAttemptAt);
        Assert.Equal(intent.NextAttemptAt, restoredIntent.NextAttemptAt);
        Assert.Equal(intent.LastFailureCode, restoredIntent.LastFailureCode);
        using (var retry = Assert.Single(
            restarted.ClaimPendingRuntimeDeletions(
                1,
                attemptedAt.AddSeconds(1))))
        {
            var secondAttemptAt = attemptedAt.AddSeconds(1);
            restarted.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        retry,
                        RuntimeDeletionFailureCodes.AuthorityUnavailable,
                        secondAttemptAt),
                });
        }

        var retriedIntent = Assert.Single(
            restarted.ListPendingRuntimeDeletions());
        Assert.Equal(2, retriedIntent.AttemptCount);
        Assert.Equal(attemptedAt.AddSeconds(1), retriedIntent.LastAttemptAt);
        Assert.Equal(attemptedAt.AddSeconds(3), retriedIntent.NextAttemptAt);
        Assert.Equal(
            RuntimeDeletionFailureCodes.AuthorityUnavailable,
            retriedIntent.LastFailureCode);
        Assert.Null(registry.GetRuntime(healthyRuntime.RuntimeId));
        Assert.NotNull(restarted.GetRuntime(failedRuntime.RuntimeId));
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void RuntimeDeletionRetryMetadataRollsBackWhenPersistenceFails()
    {
        var (registry, statePath) = CreateRegistry();
        var backupPath = $"{statePath}.bak";
        var runtime = RegisterRuntime(registry);
        registry.ReserveRuntimeDeletion(new[] { runtime.RuntimeId }).Dispose();
        using var reservation = Assert.Single(
            registry.ClaimPendingRuntimeDeletions());
        File.Delete(backupPath);
        Directory.CreateDirectory(backupPath);
        try
        {
            Assert.Throws<OrchestraPersistenceException>(() =>
                registry.RecordRuntimeDeletionFailures(
                    new[]
                    {
                        new RuntimeDeletionFailure(
                            reservation,
                            RuntimeDeletionFailureCodes.AuthorityFailure,
                            DateTimeOffset.UtcNow),
                    }));

            var intent = Assert.Single(
                registry.ListPendingRuntimeDeletions());
            Assert.Equal(0, intent.AttemptCount);
            Assert.Null(intent.LastAttemptAt);
            Assert.Null(intent.NextAttemptAt);
            Assert.Null(intent.LastFailureCode);
            var diskIntent = Assert.Single(
                CreateRegistry(statePath)
                    .ListPendingRuntimeDeletions());
            Assert.Equal(0, diskIntent.AttemptCount);
            Assert.Null(diskIntent.LastFailureCode);
        }
        finally
        {
            Directory.Delete(backupPath);
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void RuntimeDeletionRetryNowIsRevisionFencedDurableAndIdempotent()
    {
        var (registry, statePath) = CreateRegistry();
        const string runtimeId = "runtime-retry-now";
        registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Retry now runtime",
                "https://retry-now.example",
                "pairing-token"),
            runtimeId);
        registry.ReserveRuntimeDeletion(new[] { runtimeId }).Dispose();
        var attemptedAt = DateTimeOffset.UtcNow;
        using (var reservation = Assert.Single(
            registry.ClaimPendingRuntimeDeletions(1, attemptedAt)))
        {
            registry.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        reservation,
                        RuntimeDeletionFailureCodes.AuthorityUnavailable,
                        attemptedAt),
                });
        }
        var deferredIntent = Assert.Single(
            registry.ListPendingRuntimeDeletions());
        Assert.Equal(2, deferredIntent.Revision);

        var stale = Assert.Throws<RuntimeDeletionRetryException>(() =>
            registry.RetryRuntimeDeletionNow(
                deferredIntent.IntentId,
                new RuntimeDeletionRetryNowRequest(
                    1,
                    "retry-now-stale",
                    "operator-a"),
                attemptedAt.AddMilliseconds(100)));
        Assert.Equal(
            "runtime_deletion_retry_revision_changed",
            stale.Code);

        var request = new RuntimeDeletionRetryNowRequest(
            deferredIntent.Revision,
            "retry-now-request-1",
            "operator-a");
        var requestedAt = attemptedAt.AddMilliseconds(100);
        var accepted = registry.RetryRuntimeDeletionNow(
            deferredIntent.IntentId,
            request,
            requestedAt);
        Assert.True(accepted.Accepted);
        Assert.False(accepted.Replayed);
        Assert.Equal(3, accepted.PendingIntent!.Revision);
        Assert.Equal(requestedAt, accepted.PendingIntent.NextAttemptAt);
        Assert.Equal(request.ExpectedRevision, accepted.Audit.ExpectedRevision);
        Assert.Equal(3, accepted.Audit.ResultingRevision);
        Assert.Equal("operator-a", accepted.Audit.RequestedBy);

        var replay = registry.RetryRuntimeDeletionNow(
            deferredIntent.IntentId,
            request,
            requestedAt.AddMilliseconds(50));
        Assert.True(replay.Replayed);
        Assert.Single(registry.ListRuntimeDeletionRetryAudit());
        var conflict = Assert.Throws<RuntimeDeletionRetryException>(() =>
            registry.RetryRuntimeDeletionNow(
                deferredIntent.IntentId,
                request with { RequestedBy = "operator-b" },
                requestedAt));
        Assert.Equal(
            "runtime_deletion_retry_request_conflict",
            conflict.Code);

        var restarted = CreateRegistry(statePath);
        var restoredIntent = Assert.Single(
            restarted.ListPendingRuntimeDeletions());
        Assert.Equal(3, restoredIntent.Revision);
        var restoredAudit = Assert.Single(
            restarted.ListRuntimeDeletionRetryAudit());
        Assert.Equal(accepted.Audit.RequestId, restoredAudit.RequestId);
        Assert.Equal(accepted.Audit.IntentId, restoredAudit.IntentId);
        Assert.Equal(accepted.Audit.RuntimeIds, restoredAudit.RuntimeIds);
        Assert.Equal(
            accepted.Audit.ExpectedRevision,
            restoredAudit.ExpectedRevision);
        Assert.Equal(
            accepted.Audit.ResultingRevision,
            restoredAudit.ResultingRevision);
        Assert.Equal(accepted.Audit.RequestedBy, restoredAudit.RequestedBy);
        Assert.Equal(accepted.Audit.RequestedAt, restoredAudit.RequestedAt);
        using (var reservation = Assert.Single(
            restarted.ClaimPendingRuntimeDeletions(1, requestedAt)))
        {
            restarted.CompleteRecoveredRuntimeDeletions(
                new[] { reservation });
        }

        var converged = CreateRegistry(statePath);
        Assert.Empty(converged.ListPendingRuntimeDeletions());
        Assert.Null(converged.GetRuntime(runtimeId));
        Assert.Single(converged.ListRuntimeDeletionRetryAudit());
        var postConvergenceReplay = converged.RetryRuntimeDeletionNow(
            deferredIntent.IntentId,
            request);
        Assert.True(postConvergenceReplay.Replayed);
        Assert.Null(postConvergenceReplay.PendingIntent);
        Assert.Single(converged.ListRuntimeDeletionRetryAudit());
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void RuntimeDeletionRetryNowRollsBackIntentAndAuditOnPersistenceFailure()
    {
        var (registry, statePath) = CreateRegistry();
        var backupPath = $"{statePath}.bak";
        var runtime = RegisterRuntime(registry);
        registry.ReserveRuntimeDeletion(new[] { runtime.RuntimeId }).Dispose();
        var attemptedAt = DateTimeOffset.UtcNow;
        using (var reservation = Assert.Single(
            registry.ClaimPendingRuntimeDeletions(1, attemptedAt)))
        {
            registry.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        reservation,
                        RuntimeDeletionFailureCodes.AuthorityFailure,
                        attemptedAt),
                });
        }
        var before = Assert.Single(
            registry.ListPendingRuntimeDeletions());
        File.Delete(backupPath);
        Directory.CreateDirectory(backupPath);
        try
        {
            Assert.Throws<OrchestraPersistenceException>(() =>
                registry.RetryRuntimeDeletionNow(
                    before.IntentId,
                    new RuntimeDeletionRetryNowRequest(
                        before.Revision,
                        "retry-now-persistence-failure",
                        "operator-a"),
                    attemptedAt.AddMilliseconds(100)));

            var after = Assert.Single(
                registry.ListPendingRuntimeDeletions());
            Assert.Equal(before.Revision, after.Revision);
            Assert.Equal(before.NextAttemptAt, after.NextAttemptAt);
            Assert.Empty(registry.ListRuntimeDeletionRetryAudit());
            var diskReloaded = CreateRegistry(statePath);
            Assert.Equal(
                before.Revision,
                Assert.Single(
                    diskReloaded.ListPendingRuntimeDeletions())
                    .Revision);
            Assert.Empty(
                diskReloaded.ListRuntimeDeletionRetryAudit());
        }
        finally
        {
            Directory.Delete(backupPath);
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RuntimeDeletionRetryNowSignalWakesSleepingRecoveryWorker()
    {
        var (registry, statePath) = CreateRegistry();
        var runtime = RegisterRuntime(registry);
        registry.ReserveRuntimeDeletion(new[] { runtime.RuntimeId }).Dispose();
        var attemptedAt = DateTimeOffset.UtcNow;
        using (var reservation = Assert.Single(
            registry.ClaimPendingRuntimeDeletions(1, attemptedAt)))
        {
            registry.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        reservation,
                        RuntimeDeletionFailureCodes.AuthorityUnavailable,
                        attemptedAt),
                });
        }

        var deferredIntent = Assert.Single(
            registry.ListPendingRuntimeDeletions());
        var signal = new RuntimeDeletionRecoverySignal();
        var authority = new RecordingRegistrationAuthority();
        var recovery = new RuntimeDeletionRecoveryService(
            registry,
            authority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance,
            signal);
        try
        {
            await recovery.StartAsync(CancellationToken.None);
            await Task.Delay(100);
            var timer = Stopwatch.StartNew();
            var accepted = registry.RetryRuntimeDeletionNow(
                deferredIntent.IntentId,
                new RuntimeDeletionRetryNowRequest(
                    deferredIntent.Revision,
                    "retry-now-wakeup",
                    "operator-a"));
            Assert.False(accepted.Replayed);
            signal.Pulse();
            await WaitForPendingDeletionCountAsync(registry, 0);
            timer.Stop();

            Assert.True(timer.ElapsedMilliseconds < 500);
            Assert.Equal(
                new[] { runtime.RuntimeId },
                authority.UnregisteredRuntimeIds);
        }
        finally
        {
            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RuntimeDeletionRetryNowAndWorkerClaimRaceIsLinearizable()
    {
        const int forcedWorkerFirstRounds = 8;
        const int forcedOperatorFirstRounds = 8;
        const int simultaneousRounds = 32;
        var results = new List<RuntimeDeletionRetryClaimRaceResult>();

        foreach (var mode in Enumerable.Repeat(
            RuntimeDeletionRetryClaimRaceMode.WorkerFirst,
            forcedWorkerFirstRounds).Concat(
                Enumerable.Repeat(
                    RuntimeDeletionRetryClaimRaceMode.OperatorFirst,
                    forcedOperatorFirstRounds)).Concat(
                Enumerable.Repeat(
                    RuntimeDeletionRetryClaimRaceMode.Simultaneous,
                    simultaneousRounds)))
        {
            results.Add(await RunRuntimeDeletionRetryClaimRaceAsync(
                mode,
                results.Count));
        }

        Assert.All(results, result =>
        {
            Assert.Equal(1, result.AuthorityCallCount);
            Assert.Equal(1, result.ConvergedRuntimeCount);
            Assert.InRange(result.AcceptedRetryCount, 0, 1);
            Assert.Equal(
                result.AcceptedRetryCount,
                result.RetainedAuditCount);
            Assert.Equal(
                8,
                result.AcceptedRetryCount +
                result.InProgressConflictCount +
                result.RevisionConflictCount);
            Assert.Equal(0, result.UnexpectedResultCount);
        });
        Assert.All(
            results.Where(result =>
                result.Mode == RuntimeDeletionRetryClaimRaceMode.WorkerFirst),
            result =>
            {
                Assert.Equal(0, result.AcceptedRetryCount);
                Assert.Equal(8, result.InProgressConflictCount);
            });
        Assert.All(
            results.Where(result =>
                result.Mode == RuntimeDeletionRetryClaimRaceMode.OperatorFirst),
            result =>
            {
                Assert.Equal(1, result.AcceptedRetryCount);
                Assert.Equal(7, result.RevisionConflictCount);
            });

        WriteRuntimeDeletionRetryClaimRaceEvidenceIfRequested(results);
    }

    [Fact]
    public async Task RuntimeDeletionRetryAuditRetentionRollsOverWithoutStarvation()
    {
        const int auditEntryCount = 272;
        const int auditRetentionLimit = 256;
        const int waveSize = 128;
        var (registry, statePath) = CreateRegistry();
        var authority = new RuntimeDeletionRetryRetentionAuthority();
        var signal = new RuntimeDeletionRecoverySignal();
        RuntimeDeletionRecoveryService? recovery = null;
        var acceptedAudits =
            new List<PersistedRuntimeDeletionRetryAudit>(auditEntryCount);
        var timer = Stopwatch.StartNew();
        try
        {
            recovery = new RuntimeDeletionRecoveryService(
                registry,
                authority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance,
                signal);
            await recovery.StartAsync(CancellationToken.None);

            for (var waveStart = 0;
                 waveStart < auditEntryCount;
                 waveStart += waveSize)
            {
                var waveCount = Math.Min(
                    waveSize,
                    auditEntryCount - waveStart);
                var commands =
                    new List<RuntimeDeletionRetryRolloverCommand>(
                        waveCount);
                for (var offset = 0; offset < waveCount; offset += 1)
                {
                    var index = waveStart + offset;
                    var runtimeId =
                        $"runtime-retry-rollover-{index:D3}";
                    registry.RegisterRuntime(
                        new RuntimeRegistrationRequest(
                            $"Runtime retry rollover {index:D3}",
                            $"https://retry-rollover-{index:D3}.example",
                            "pairing-token"),
                        runtimeId);
                    using (var reservation =
                        registry.ReserveRuntimeDeletion(
                            new[] { runtimeId }))
                    {
                        registry.RecordRuntimeDeletionFailures(
                            new[]
                            {
                                new RuntimeDeletionFailure(
                                    reservation,
                                    RuntimeDeletionFailureCodes
                                        .AuthorityUnavailable,
                                    DateTimeOffset.UtcNow.AddMinutes(4)),
                            });
                    }
                    var intent = registry.ListPendingRuntimeDeletions()
                        .Single(candidate =>
                            candidate.RuntimeIds.Contains(
                                runtimeId,
                                StringComparer.Ordinal));
                    commands.Add(
                        new RuntimeDeletionRetryRolloverCommand(
                            intent,
                            $"retry-rollover-{index:D3}"));
                }

                var responses = await Task.WhenAll(
                    commands.Select(command => Task.Run(() =>
                    {
                        var response =
                            registry.RetryRuntimeDeletionNow(
                                command.Intent.IntentId,
                                new RuntimeDeletionRetryNowRequest(
                                    command.Intent.Revision,
                                    command.RequestId,
                                    "rollover-operator"));
                        signal.Pulse();
                        return response;
                    })));
                Assert.All(responses, response =>
                {
                    Assert.True(response.Accepted);
                    Assert.False(response.Replayed);
                });
                acceptedAudits.AddRange(
                    responses.Select(static response => response.Audit));
                await WaitForPendingDeletionCountAsync(registry, 0);
            }

            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            recovery = null;
            timer.Stop();

            Assert.Equal(auditEntryCount, authority.CallCount);
            Assert.InRange(authority.MaxConcurrentCalls, 2, 8);
            Assert.All(
                Enumerable.Range(0, auditEntryCount),
                index => Assert.Equal(
                    1,
                    authority.AttemptCountFor(
                        $"runtime-retry-rollover-{index:D3}")));
            var chronologicalAudits = acceptedAudits
                .OrderBy(static audit => audit.RequestedAt)
                .ToArray();
            Assert.True(
                chronologicalAudits
                    .Select(static audit => audit.RequestedAt)
                    .Zip(
                        chronologicalAudits
                            .Skip(1)
                            .Select(static audit => audit.RequestedAt))
                    .All(static pair => pair.First < pair.Second));
            var expectedRetained = chronologicalAudits
                .TakeLast(auditRetentionLimit)
                .ToArray();
            var retained = registry.ListRuntimeDeletionRetryAudit();
            Assert.Equal(auditRetentionLimit, retained.Count);
            Assert.Equal(
                expectedRetained
                    .Reverse()
                    .Select(static audit => audit.RequestId),
                retained.Select(static audit => audit.RequestId));

            var diskReloaded = CreateRegistry(statePath);
            Assert.Equal(
                retained.Select(static audit => audit.RequestId),
                diskReloaded
                    .ListRuntimeDeletionRetryAudit()
                    .Select(static audit => audit.RequestId));
            var retainedReplayAudit = expectedRetained[0];
            var retainedReplay =
                diskReloaded.RetryRuntimeDeletionNow(
                    retainedReplayAudit.IntentId,
                    new RuntimeDeletionRetryNowRequest(
                        retainedReplayAudit.ExpectedRevision,
                        retainedReplayAudit.RequestId,
                        retainedReplayAudit.RequestedBy));
            Assert.True(retainedReplay.Replayed);
            Assert.Null(retainedReplay.PendingIntent);

            var evictedAudit = chronologicalAudits[0];
            var outsideHorizon = Assert.Throws<
                RuntimeDeletionRetryException>(() =>
                diskReloaded.RetryRuntimeDeletionNow(
                    evictedAudit.IntentId,
                    new RuntimeDeletionRetryNowRequest(
                        evictedAudit.ExpectedRevision,
                        evictedAudit.RequestId,
                        evictedAudit.RequestedBy)));
            Assert.Equal(
                "runtime_deletion_intent_not_found",
                outsideHorizon.Code);

            const string reuseRuntimeId =
                "runtime-retry-rollover-reuse";
            diskReloaded.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Runtime retry rollover reuse",
                    "https://retry-rollover-reuse.example",
                    "pairing-token"),
                reuseRuntimeId);
            using (var reservation =
                diskReloaded.ReserveRuntimeDeletion(
                    new[] { reuseRuntimeId }))
            {
                diskReloaded.RecordRuntimeDeletionFailures(
                    new[]
                    {
                        new RuntimeDeletionFailure(
                            reservation,
                            RuntimeDeletionFailureCodes
                                .AuthorityUnavailable,
                            DateTimeOffset.UtcNow.AddMinutes(4)),
                    });
            }
            var reuseIntent = Assert.Single(
                diskReloaded.ListPendingRuntimeDeletions());
            var reusedRequest =
                diskReloaded.RetryRuntimeDeletionNow(
                    reuseIntent.IntentId,
                    new RuntimeDeletionRetryNowRequest(
                        reuseIntent.Revision,
                        evictedAudit.RequestId,
                        evictedAudit.RequestedBy));
            Assert.False(reusedRequest.Replayed);

            var reuseSignal = new RuntimeDeletionRecoverySignal();
            recovery = new RuntimeDeletionRecoveryService(
                diskReloaded,
                authority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance,
                reuseSignal);
            await recovery.StartAsync(CancellationToken.None);
            reuseSignal.Pulse();
            await WaitForPendingDeletionCountAsync(diskReloaded, 0);
            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            recovery = null;

            Assert.Equal(auditEntryCount + 1, authority.CallCount);
            Assert.Equal(1, authority.AttemptCountFor(reuseRuntimeId));
            var finalAudit =
                diskReloaded.ListRuntimeDeletionRetryAudit();
            Assert.Equal(auditRetentionLimit, finalAudit.Count);
            Assert.Equal(evictedAudit.RequestId, finalAudit[0].RequestId);
            Assert.DoesNotContain(
                finalAudit,
                audit =>
                    audit.RequestId ==
                    retainedReplayAudit.RequestId);

            var convergedReload = CreateRegistry(statePath);
            Assert.Empty(
                convergedReload.ListPendingRuntimeDeletions());
            Assert.Equal(
                finalAudit.Select(static audit => audit.RequestId),
                convergedReload
                    .ListRuntimeDeletionRetryAudit()
                    .Select(static audit => audit.RequestId));
            WriteRuntimeDeletionRetryRolloverEvidenceIfRequested(
                auditEntryCount,
                auditRetentionLimit,
                authority,
                timer.ElapsedMilliseconds);
        }
        finally
        {
            if (recovery is not null)
            {
                await recovery.StopAsync(CancellationToken.None);
                recovery.Dispose();
            }
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void RuntimeDeletionRetryMetadataRejectsUntrustedFailureCode()
    {
        var statePath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-orchestra-test-{Guid.NewGuid():N}.json");
        var attemptedAt = DateTimeOffset.UtcNow.AddSeconds(-1);
        var state = new PersistedControlPlaneState(
            3,
            attemptedAt,
            Array.Empty<PersistedRuntimeState>(),
            Array.Empty<PersistedSessionState>(),
            Array.Empty<OrchestraRunSummary>(),
            new[]
            {
                new PersistedRuntimeDeletionIntent(
                    "rdel_untrusted_failure",
                    new[] { "runtime-untrusted-failure" },
                    attemptedAt.AddSeconds(-1),
                    1,
                    attemptedAt,
                    attemptedAt.AddSeconds(1),
                    "authority_failure\ncredential=secret"),
            });
        File.WriteAllText(
            statePath,
            JsonSerializer.Serialize(
                state,
                new LeserpentJsonContext(new JsonSerializerOptions())
                    .PersistedControlPlaneState));
        try
        {
            var store = CreateStateStore(statePath);
            var registry = new RegistryService(
                store,
                new InMemoryOrchestraRunStore());
            Assert.Empty(registry.ListPendingRuntimeDeletions());
            Assert.Equal(
                ControlPlaneStateLoadOutcome.Failed,
                store.LoadProvenance.Outcome);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                store.LoadProvenance.PrimaryFailureCode);
            Assert.DoesNotContain(
                "credential",
                store.LastSaveError ?? string.Empty,
                StringComparison.OrdinalIgnoreCase);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void RuntimeDeletionRetryAuditRejectsUntrustedOrUnboundedState()
    {
        var requestedAt = DateTimeOffset.UtcNow.AddSeconds(-1);
        var validAudit = Enumerable.Range(0, 257)
            .Select(index => new PersistedRuntimeDeletionRetryAudit(
                $"retry-audit-{index:D3}",
                "rdel_retry_audit",
                new[] { "runtime-retry-audit" },
                2,
                3,
                "operator-a",
                requestedAt))
            .ToArray();
        var invalidStates = new[]
        {
            validAudit,
            new[]
            {
                validAudit[0] with
                {
                    RequestedBy = "token=secret",
                },
            },
        };

        foreach (var audit in invalidStates)
        {
            var statePath = Path.Combine(
                Path.GetTempPath(),
                $"leserpent-orchestra-test-{Guid.NewGuid():N}.json");
            var state = new PersistedControlPlaneState(
                3,
                requestedAt,
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                Array.Empty<OrchestraRunSummary>(),
                Array.Empty<PersistedRuntimeDeletionIntent>(),
                audit);
            File.WriteAllText(
                statePath,
                JsonSerializer.Serialize(
                    state,
                    new LeserpentJsonContext(new JsonSerializerOptions())
                        .PersistedControlPlaneState));
            try
            {
                var store = CreateStateStore(statePath);
                var registry = new RegistryService(
                    store,
                    new InMemoryOrchestraRunStore());
                Assert.Empty(
                    registry.ListRuntimeDeletionRetryAudit());
                Assert.Equal(
                    ControlPlaneStateLoadOutcome.Failed,
                    store.LoadProvenance.Outcome);
                Assert.Equal(
                    ControlPlaneStateLoadFailureCode.SemanticInvalid,
                    store.LoadProvenance.PrimaryFailureCode);
            }
            finally
            {
                DeleteStateFiles(statePath);
            }
        }
    }

    [Fact]
    public async Task SaturatedRuntimeDeletionQueueIsFairAndStopsCooperatively()
    {
        const int intentCount = 128;
        const int poisonStride = 16;
        var runStore = new CountingDeleteRunStore();
        var (registry, statePath) = CreateRegistry(runStore);
        for (var index = 0; index < intentCount; index += 1)
        {
            var runtimeId = $"runtime-saturated-{index:D3}";
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    $"Runtime {runtimeId}",
                    $"https://{runtimeId}.example",
                    "pairing-token"),
                runtimeId);
            registry.ReserveRuntimeDeletion(new[] { runtimeId }).Dispose();
        }

        var orderedRuntimeIds = registry.ListPendingRuntimeDeletions()
            .Select(intent => Assert.Single(intent.RuntimeIds))
            .ToArray();
        Assert.Equal(intentCount, orderedRuntimeIds.Length);
        var cancellationAuthority = new CancellationBlockingAuthority();
        var cancellationRecovery = new RuntimeDeletionRecoveryService(
            registry,
            cancellationAuthority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);
        await cancellationRecovery.StartAsync(CancellationToken.None);
        await cancellationAuthority.AllSlotsStarted.WaitAsync(
            TimeSpan.FromSeconds(5));
        await Task.Delay(100);
        Assert.Equal(8, cancellationAuthority.StartedCallCount);
        var cancellationTimer = Stopwatch.StartNew();
        await cancellationRecovery.StopAsync(CancellationToken.None);
        cancellationTimer.Stop();
        cancellationRecovery.Dispose();

        Assert.True(cancellationTimer.ElapsedMilliseconds < 1_000);
        Assert.Equal(8, cancellationAuthority.CancelledCallCount);
        Assert.Equal(intentCount, registry.ListPendingRuntimeDeletions().Count);
        Assert.Equal(0, runStore.DeleteCount);
        var reclaimedAfterStop = registry.ClaimPendingRuntimeDeletions(32);
        Assert.Equal(32, reclaimedAfterStop.Count);
        foreach (var reservation in reclaimedAfterStop)
        {
            reservation.Dispose();
        }

        var poisonRuntimeIds = orderedRuntimeIds
            .Where((_, index) => index % poisonStride == 0)
            .ToArray();
        var slowRuntimeIds = orderedRuntimeIds
            .Where((_, index) =>
                index % 7 == 3 &&
                index % poisonStride != 0)
            .ToArray();
        var mixedAuthority = new MixedQueueRuntimeDeletionAuthority(
            poisonRuntimeIds,
            slowRuntimeIds);
        var mixedRecovery = new RuntimeDeletionRecoveryService(
            registry,
            mixedAuthority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);
        var progressionTimer = Stopwatch.StartNew();
        var expectedPendingCounts = new[] { 98, 68, 38, 8 };
        var passLatencies = new List<long>(expectedPendingCounts.Length);
        await mixedRecovery.StartAsync(CancellationToken.None);
        foreach (var expectedPendingCount in expectedPendingCounts)
        {
            await WaitForPendingDeletionCountAsync(
                registry,
                expectedPendingCount);
            passLatencies.Add(progressionTimer.ElapsedMilliseconds);
        }
        await mixedRecovery.StopAsync(CancellationToken.None);
        mixedRecovery.Dispose();

        Assert.Equal(4, runStore.DeleteCount);
        Assert.Equal(8, mixedAuthority.MaxConcurrentCalls);
        Assert.True(mixedAuthority.SlowOperationCount > 0);
        Assert.Equal(
            Enumerable.Repeat(1, poisonRuntimeIds.Length),
            poisonRuntimeIds
                .Select(mixedAuthority.AttemptCountFor)
                .ToArray());
        var healthyRuntimeIds = orderedRuntimeIds
            .Except(poisonRuntimeIds, StringComparer.Ordinal)
            .ToArray();
        Assert.All(
            healthyRuntimeIds,
            runtimeId => Assert.Equal(1, mixedAuthority.AttemptCountFor(runtimeId)));
        foreach (var runtimeId in healthyRuntimeIds)
        {
            Assert.Null(registry.GetRuntime(runtimeId));
        }
        var retryIntents = registry.ListPendingRuntimeDeletions();
        Assert.All(
            retryIntents,
            intent =>
            {
                Assert.Equal(1, intent.AttemptCount);
                Assert.NotNull(intent.LastAttemptAt);
                Assert.Equal(
                    intent.LastAttemptAt.Value.AddSeconds(1),
                    intent.NextAttemptAt);
                Assert.Equal(
                    RuntimeDeletionFailureCodes.AuthorityFailure,
                    intent.LastFailureCode);
            });
        foreach (var runtimeId in poisonRuntimeIds)
        {
            Assert.NotNull(registry.GetRuntime(runtimeId));
            Assert.Equal(
                runtimeId,
                registry.CreateSession(new SessionCreateRequest(
                    runtimeId,
                    "diagnostic",
                    "saturated-queue-test",
                    Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);
        }

        var reloadRunStore = new CountingDeleteRunStore();
        var diskReloaded = CreateRegistry(statePath, reloadRunStore);
        var reloadedRetryIntents =
            diskReloaded.ListPendingRuntimeDeletions();
        Assert.Equal(poisonRuntimeIds, reloadedRetryIntents
            .Select(intent => Assert.Single(intent.RuntimeIds)));
        Assert.Equal(
            retryIntents.Select(static intent => intent.AttemptCount),
            reloadedRetryIntents.Select(static intent => intent.AttemptCount));
        Assert.Equal(
            retryIntents.Select(static intent => intent.LastAttemptAt),
            reloadedRetryIntents.Select(static intent => intent.LastAttemptAt));
        Assert.Equal(
            retryIntents.Select(static intent => intent.NextAttemptAt),
            reloadedRetryIntents.Select(static intent => intent.NextAttemptAt));
        Assert.Equal(
            retryIntents.Select(static intent => intent.LastFailureCode),
            reloadedRetryIntents.Select(static intent => intent.LastFailureCode));
        var firstRetryAt = reloadedRetryIntents
            .Min(static intent => intent.NextAttemptAt)!.Value;
        Assert.Empty(diskReloaded.ClaimPendingRuntimeDeletions(
            32,
            firstRetryAt.AddMilliseconds(-1)));
        var staleRetry = Assert.Throws<RuntimeDeletionRetryException>(() =>
            diskReloaded.RetryRuntimeDeletionNow(
                reloadedRetryIntents[0].IntentId,
                new RuntimeDeletionRetryNowRequest(
                    reloadedRetryIntents[0].Revision - 1,
                    "saturated-retry-stale",
                    "saturated-operator"),
                firstRetryAt.AddMilliseconds(-1)));
        Assert.Equal(
            "runtime_deletion_retry_revision_changed",
            staleRetry.Code);
        var operatorRequestedAt = firstRetryAt.AddMilliseconds(-1);
        var retryNowResponses = reloadedRetryIntents
            .Select((intent, index) =>
                diskReloaded.RetryRuntimeDeletionNow(
                    intent.IntentId,
                    new RuntimeDeletionRetryNowRequest(
                        intent.Revision,
                        $"saturated-retry-{index:D2}",
                        "saturated-operator"),
                    operatorRequestedAt))
            .ToArray();
        Assert.All(
            retryNowResponses,
            response =>
            {
                Assert.False(response.Replayed);
                Assert.Equal(3, response.PendingIntent!.Revision);
                Assert.Equal(
                    operatorRequestedAt,
                    response.PendingIntent.NextAttemptAt);
            });
        Assert.Equal(
            poisonRuntimeIds.Length,
            diskReloaded.ListRuntimeDeletionRetryAudit().Count);
        var operatorReloaded = CreateRegistry(
            statePath,
            reloadRunStore);
        Assert.All(
            operatorReloaded.ListPendingRuntimeDeletions(),
            intent => Assert.Equal(3, intent.Revision));
        Assert.Equal(
            poisonRuntimeIds.Length,
            operatorReloaded.ListRuntimeDeletionRetryAudit().Count);
        var replayedRetry = operatorReloaded.RetryRuntimeDeletionNow(
            reloadedRetryIntents[0].IntentId,
            new RuntimeDeletionRetryNowRequest(
                reloadedRetryIntents[0].Revision,
                "saturated-retry-00",
                "saturated-operator"));
        Assert.True(replayedRetry.Replayed);
        Assert.Equal(
            poisonRuntimeIds.Length,
            operatorReloaded.ListRuntimeDeletionRetryAudit().Count);
        foreach (var runtimeId in poisonRuntimeIds)
        {
            Assert.NotNull(operatorReloaded.GetRuntime(runtimeId));
            Assert.Equal(
                runtimeId,
                operatorReloaded.CreateSession(new SessionCreateRequest(
                    runtimeId,
                    "diagnostic",
                    "saturated-queue-test",
                    Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);
        }

        var repairAuthority = new MixedQueueRuntimeDeletionAuthority(
            Array.Empty<string>(),
            Array.Empty<string>());
        var repairRecovery = new RuntimeDeletionRecoveryService(
            operatorReloaded,
            repairAuthority,
            NullLogger<RuntimeDeletionRecoveryService>.Instance);
        var repairTimer = Stopwatch.StartNew();
        await repairRecovery.StartAsync(CancellationToken.None);
        await WaitForPendingDeletionCountAsync(operatorReloaded, 0);
        repairTimer.Stop();
        await repairRecovery.StopAsync(CancellationToken.None);
        repairRecovery.Dispose();
        Assert.Equal(1, reloadRunStore.DeleteCount);
        Assert.All(
            poisonRuntimeIds,
            runtimeId => Assert.Equal(1, repairAuthority.AttemptCountFor(runtimeId)));

        var convergedReload = CreateRegistry(statePath);
        Assert.Empty(convergedReload.ListPendingRuntimeDeletions());
        Assert.Equal(
            poisonRuntimeIds.Length,
            convergedReload.ListRuntimeDeletionRetryAudit().Count);
        foreach (var runtimeId in orderedRuntimeIds)
        {
            Assert.Null(convergedReload.GetRuntime(runtimeId));
        }
        WriteSaturatedQueueEvidenceIfRequested(
            intentCount,
            poisonRuntimeIds.Length,
            slowRuntimeIds.Length,
            cancellationAuthority.StartedCallCount,
            cancellationAuthority.CancelledCallCount,
            cancellationTimer.ElapsedMilliseconds,
            mixedAuthority.MaxConcurrentCalls,
            expectedPendingCounts,
            passLatencies,
            poisonRuntimeIds.Select(mixedAuthority.AttemptCountFor).ToArray(),
            reloadedRetryIntents
                .Select(static intent => intent.AttemptCount)
                .ToArray(),
            reloadedRetryIntents
                .Select(static intent => intent.LastFailureCode)
                .ToArray(),
            retryNowResponses
                .Select(response => response.PendingIntent!.Revision)
                .ToArray(),
            convergedReload.ListRuntimeDeletionRetryAudit().Count,
            replayedRetry.Replayed,
            repairTimer.ElapsedMilliseconds);
        DeleteStateFiles(statePath);
    }

    [Fact]
    public void RuntimeDeletionReservationRollsBackWhenIntentCannotBePersisted()
    {
        var (registry, statePath) = CreateRegistry();
        var backupPath = $"{statePath}.bak";
        var runtime = RegisterRuntime(registry);
        Directory.CreateDirectory(backupPath);

        try
        {
            Assert.Throws<OrchestraPersistenceException>(() =>
                registry.ReserveRuntimeDeletion(new[] { runtime.RuntimeId }));

            Assert.Empty(registry.ListPendingRuntimeDeletions());
            Assert.NotNull(registry.CreateSession(new SessionCreateRequest(
                runtime.RuntimeId,
                "diagnostic",
                "operator",
                Array.Empty<SessionCapabilityRequirement>())).Session);
        }
        finally
        {
            Directory.Delete(backupPath);
            DeleteStateFiles(statePath);
        }
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

    private static async Task WaitForPendingDeletionCountAsync(
        RegistryService registry,
        int expectedCount)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(10);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (registry.ListPendingRuntimeDeletions().Count == expectedCount)
            {
                return;
            }
            await Task.Delay(10);
        }
        throw new TimeoutException(
            $"runtime deletion intent count did not converge to {expectedCount}");
    }

    private static async Task<RuntimeDeletionRetryClaimRaceResult>
        RunRuntimeDeletionRetryClaimRaceAsync(
            RuntimeDeletionRetryClaimRaceMode mode,
            int round)
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var runtimeId = $"runtime-retry-claim-race-{round:D3}";
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    $"Runtime retry claim race {round:D3}",
                    $"https://retry-claim-race-{round:D3}.example",
                    "pairing-token"),
                runtimeId);
            using (var initialReservation =
                registry.ReserveRuntimeDeletion(new[] { runtimeId }))
            {
                var attemptedAt = DateTimeOffset.UtcNow;
                registry.RecordRuntimeDeletionFailures(
                    new[]
                    {
                        new RuntimeDeletionFailure(
                            initialReservation,
                            RuntimeDeletionFailureCodes.AuthorityUnavailable,
                            attemptedAt),
                    });
            }

            var deferredIntent = Assert.Single(
                registry.ListPendingRuntimeDeletions());
            var authority = new RuntimeDeletionRetryClaimRaceAuthority();

            async Task ClaimAndDeleteAsync(Task start)
            {
                await start;
                using var reservation = Assert.Single(
                    registry.ClaimPendingRuntimeDeletions(
                        1,
                        deferredIntent.NextAttemptAt!.Value.AddMilliseconds(1)));
                await authority.UnregisterAsync(
                    reservation.RuntimeIds,
                    CancellationToken.None);
                registry.CompleteRecoveredRuntimeDeletions(
                    new[] { reservation });
            }

            async Task<string> RetryAsync(int contender, Task start)
            {
                await start;
                try
                {
                    registry.RetryRuntimeDeletionNow(
                        deferredIntent.IntentId,
                        new RuntimeDeletionRetryNowRequest(
                            deferredIntent.Revision,
                            $"retry-claim-race-{round:D3}-{contender:D2}",
                            "race-operator"));
                    return "accepted";
                }
                catch (RuntimeDeletionRetryException ex)
                {
                    return ex.Code;
                }
            }

            Task worker;
            Task<string>[] retryTasks;
            if (mode == RuntimeDeletionRetryClaimRaceMode.WorkerFirst)
            {
                worker = Task.Run(() =>
                    ClaimAndDeleteAsync(Task.CompletedTask));
                await authority.Started.WaitAsync(TimeSpan.FromSeconds(2));
                retryTasks = Enumerable.Range(0, 8)
                    .Select(contender => Task.Run(() =>
                        RetryAsync(contender, Task.CompletedTask)))
                    .ToArray();
            }
            else if (mode == RuntimeDeletionRetryClaimRaceMode.OperatorFirst)
            {
                retryTasks = Enumerable.Range(0, 8)
                    .Select(contender => Task.Run(() =>
                        RetryAsync(contender, Task.CompletedTask)))
                    .ToArray();
                await Task.WhenAll(retryTasks);
                worker = Task.Run(() =>
                    ClaimAndDeleteAsync(Task.CompletedTask));
            }
            else
            {
                var start = new TaskCompletionSource(
                    TaskCreationOptions.RunContinuationsAsynchronously);
                worker = Task.Run(() => ClaimAndDeleteAsync(start.Task));
                retryTasks = Enumerable.Range(0, 8)
                    .Select(contender => Task.Run(() =>
                        RetryAsync(contender, start.Task)))
                    .ToArray();
                start.TrySetResult();
            }

            var retryResults = await Task.WhenAll(retryTasks);
            await authority.Started.WaitAsync(TimeSpan.FromSeconds(2));
            authority.Release();
            await worker.WaitAsync(TimeSpan.FromSeconds(2));

            var acceptedRetryCount = retryResults.Count(
                result => result == "accepted");
            var inProgressConflictCount = retryResults.Count(result =>
                result == "runtime_deletion_retry_in_progress");
            var revisionConflictCount = retryResults.Count(result =>
                result == "runtime_deletion_retry_revision_changed");
            var unexpectedResultCount =
                retryResults.Length -
                acceptedRetryCount -
                inProgressConflictCount -
                revisionConflictCount;
            var reloaded = CreateRegistry(statePath);
            var retainedAuditCount =
                reloaded.ListRuntimeDeletionRetryAudit().Count;
            var convergedRuntimeCount =
                reloaded.GetRuntime(runtimeId) is null &&
                reloaded.ListPendingRuntimeDeletions().Count == 0
                    ? 1
                    : 0;

            return new RuntimeDeletionRetryClaimRaceResult(
                mode,
                acceptedRetryCount,
                inProgressConflictCount,
                revisionConflictCount,
                unexpectedResultCount,
                authority.CallCount,
                retainedAuditCount,
                convergedRuntimeCount);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    private static void WriteRuntimeDeletionRetryRolloverEvidenceIfRequested(
        int auditEntryCount,
        int auditRetentionLimit,
        RuntimeDeletionRetryRetentionAuthority authority,
        long elapsedMilliseconds)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_RETRY_ROLLOVER_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            audit_entry_count = auditEntryCount,
            audit_retention_limit = auditRetentionLimit,
            initial_evicted_entry_count =
                auditEntryCount - auditRetentionLimit,
            wave_sizes = new[] { 128, 128, 16 },
            recovery_batch_size = 32,
            max_concurrent_authority_mutations = 8,
            observed_max_concurrency = authority.MaxConcurrentCalls,
            authority_call_count = authority.CallCount,
            elapsed_ms = elapsedMilliseconds,
            checks = new
            {
                concurrent_operator_worker_campaign = true,
                full_pending_waves_converged = true,
                audit_timestamps_followed_linearization_order = true,
                retention_bound_was_exact = true,
                oldest_entries_were_evicted_first = true,
                retained_request_replayed_after_convergence = true,
                evicted_request_was_outside_replay_horizon = true,
                evicted_request_id_was_reusable = true,
                reuse_evicted_the_next_oldest_record = true,
                every_runtime_received_one_authority_mutation = true,
                no_pending_intent_starved_or_was_lost = true,
                rollover_state_survived_disk_reload = true,
                bounded_authority_concurrency =
                    authority.MaxConcurrentCalls is >= 2 and <= 8,
                campaign_completed_under_30000_ms =
                    elapsedMilliseconds < 30_000,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteRuntimeDeletionRetryClaimRaceEvidenceIfRequested(
        IReadOnlyList<RuntimeDeletionRetryClaimRaceResult> results)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_RETRY_CLAIM_RACE_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var workerFirstRounds = results.Count(result =>
            result.Mode == RuntimeDeletionRetryClaimRaceMode.WorkerFirst);
        var operatorFirstRounds = results.Count(result =>
            result.Mode == RuntimeDeletionRetryClaimRaceMode.OperatorFirst);
        var simultaneousRounds = results.Count(result =>
            result.Mode == RuntimeDeletionRetryClaimRaceMode.Simultaneous);
        var acceptedRetryCount = results.Sum(
            static result => result.AcceptedRetryCount);
        var inProgressConflictCount = results.Sum(
            static result => result.InProgressConflictCount);
        var revisionConflictCount = results.Sum(
            static result => result.RevisionConflictCount);
        var retainedAuditCount = results.Sum(
            static result => result.RetainedAuditCount);
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            total_rounds = results.Count,
            retry_contenders_per_round = 8,
            forced_worker_first_rounds = workerFirstRounds,
            forced_operator_first_rounds = operatorFirstRounds,
            simultaneous_start_rounds = simultaneousRounds,
            accepted_retry_count = acceptedRetryCount,
            in_progress_conflict_count = inProgressConflictCount,
            revision_conflict_count = revisionConflictCount,
            retained_retry_audit_count = retainedAuditCount,
            authority_call_count = results.Sum(
                static result => result.AuthorityCallCount),
            converged_runtime_count = results.Sum(
                static result => result.ConvergedRuntimeCount),
            unexpected_result_count = results.Sum(
                static result => result.UnexpectedResultCount),
            simultaneous_accepted_retry_count = results
                .Where(result =>
                    result.Mode ==
                    RuntimeDeletionRetryClaimRaceMode.Simultaneous)
                .Sum(static result => result.AcceptedRetryCount),
            checks = new
            {
                both_forced_interleavings_exercised =
                    workerFirstRounds == 8 && operatorFirstRounds == 8,
                simultaneous_start_campaign_is_non_vacuous =
                    simultaneousRounds == 32,
                at_most_one_retry_won_each_round =
                    results.All(
                        static result => result.AcceptedRetryCount <= 1),
                worker_claim_rejected_every_late_retry =
                    results
                        .Where(result =>
                            result.Mode ==
                            RuntimeDeletionRetryClaimRaceMode.WorkerFirst)
                        .All(static result =>
                            result.InProgressConflictCount == 8),
                revision_fence_rejected_every_losing_operator =
                    results
                        .Where(result =>
                            result.Mode ==
                            RuntimeDeletionRetryClaimRaceMode.OperatorFirst)
                        .All(static result =>
                            result.AcceptedRetryCount == 1 &&
                            result.RevisionConflictCount == 7),
                accepted_retry_audit_was_durable =
                    retainedAuditCount == acceptedRetryCount,
                exactly_one_authority_mutation_per_runtime =
                    results.All(
                        static result => result.AuthorityCallCount == 1),
                every_runtime_converged_after_race =
                    results.All(
                        static result => result.ConvergedRuntimeCount == 1),
                conflict_results_were_deterministic =
                    results.All(static result =>
                        result.UnexpectedResultCount == 0),
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteSaturatedQueueEvidenceIfRequested(
        int intentCount,
        int poisonIntentCount,
        int slowIntentCount,
        int startedCancellationCalls,
        int cancelledCalls,
        long cancellationLatencyMs,
        int observedMaxConcurrency,
        IReadOnlyList<int> pendingCountsAfterPass,
        IReadOnlyList<long> passLatenciesMs,
        IReadOnlyList<int> poisonAttemptCounts,
        IReadOnlyList<int> persistedAttemptCounts,
        IReadOnlyList<string?> persistedFailureCodes,
        IReadOnlyList<long> retryNowResultingRevisions,
        int retainedRetryAuditCount,
        bool retryNowReplayObserved,
        long retryNowRepairLatencyMs)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_SATURATED_QUEUE_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var evidence = new
        {
            schema_version = 3,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            queue_intent_count = intentCount,
            recovery_batch_size = 32,
            max_concurrent_authority_mutations = 8,
            poison_stride = 16,
            poison_intent_count = poisonIntentCount,
            slow_intent_count = slowIntentCount,
            cancellation_started_call_count = startedCancellationCalls,
            cancellation_cancelled_call_count = cancelledCalls,
            cancellation_latency_ms = cancellationLatencyMs,
            observed_max_concurrency = observedMaxConcurrency,
            pending_counts_after_pass = pendingCountsAfterPass,
            pass_latencies_ms = passLatenciesMs,
            poison_attempt_counts = poisonAttemptCounts,
            persisted_attempt_counts = persistedAttemptCounts,
            persisted_failure_codes = persistedFailureCodes,
            retry_now_resulting_revisions = retryNowResultingRevisions,
            retained_retry_audit_count = retainedRetryAuditCount,
            retry_now_replay_observed = retryNowReplayObserved,
            retry_now_repair_latency_ms = retryNowRepairLatencyMs,
            retry_backoff_seconds = 1,
            orchestra_delete_batch_count = 5,
            checks = new
            {
                saturated_durable_queue = intentCount == 128,
                bounded_recovery_claim_batch = true,
                bounded_authority_concurrency =
                    observedMaxConcurrency is >= 2 and <= 8,
                all_authority_slots_were_saturated =
                    startedCancellationCalls == 8,
                cooperative_cancellation_reached_every_blocked_call =
                    cancelledCalls == startedCancellationCalls,
                shutdown_latency_under_1000_ms = cancellationLatencyMs < 1_000,
                cancelled_pass_preserved_every_intent = true,
                cancelled_pass_released_every_claim = true,
                deterministic_four_pass_progress = pendingCountsAfterPass
                    .SequenceEqual(new[] { 98, 68, 38, 8 }),
                mixed_slow_and_failing_authority_operations =
                    slowIntentCount > 0 && poisonIntentCount == 8,
                poison_failures_were_target_scoped = true,
                every_healthy_intent_converged = true,
                deferred_poison_did_not_consume_ready_claim_slots =
                    poisonAttemptCounts.All(static count => count == 1),
                retry_attempt_metadata_was_durable =
                    persistedAttemptCounts.All(static count => count == 1),
                persisted_failure_codes_were_safe =
                    persistedFailureCodes.All(code =>
                        string.Equals(
                            code,
                            RuntimeDeletionFailureCodes.AuthorityFailure,
                            StringComparison.Ordinal)),
                retry_deadline_blocked_premature_claim = true,
                stale_retry_revision_was_rejected = true,
                retry_now_revision_advanced =
                    retryNowResultingRevisions.All(
                        static revision => revision == 3),
                retry_now_audit_survived_convergence =
                    retainedRetryAuditCount == poisonIntentCount,
                retry_now_request_was_idempotent =
                    retryNowReplayObserved,
                retry_now_repair_latency_under_1000_ms =
                    retryNowRepairLatencyMs < 1_000,
                poison_reservations_survived_disk_reload = true,
                repaired_poison_intents_converged = true,
                converged_state_survived_disk_reload = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static (RegistryService Registry, string StatePath) CreateRegistry(IOrchestraRunStore? runStore = null)
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-orchestra-test-{Guid.NewGuid():N}.json");
        return (CreateRegistry(statePath, runStore), statePath);
    }

    private static RegistryService CreateRegistry(string statePath, IOrchestraRunStore? runStore = null)
    {
        var store = CreateStateStore(statePath);
        return new RegistryService(
            store,
            runStore ?? new InMemoryOrchestraRunStore());
    }

    private static ControlPlaneStateStore CreateStateStore(
        string statePath)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?> { ["LESERPENT_STATE_PATH"] = statePath })
            .Build();
        var environment = new TestHostEnvironment { ContentRootPath = Path.GetDirectoryName(statePath)! };
        var store = new ControlPlaneStateStore(
            configuration,
            environment,
            NullLogger<ControlPlaneStateStore>.Instance);
        return store;
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

    private sealed class RecordingRegistrationAuthority : IRuntimeRegistrationAuthority
    {
        public bool Enabled => true;
        public IReadOnlyList<string> UnregisteredRuntimeIds { get; private set; } = Array.Empty<string>();

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.FromResult(runtimeId);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            UnregisteredRuntimeIds = runtimeIds.ToArray();
            return Task.CompletedTask;
        }
    }

    private sealed class ReceiptAwareRegistrationAuthority(
        IReadOnlyList<string>? receiptRuntimeIds) :
        IRuntimeRegistrationAuthority
    {
        private int lookupCount;
        private int mutationCount;

        public bool Enabled => true;
        public int LookupCount => Volatile.Read(ref lookupCount);
        public int MutationCount => Volatile.Read(ref mutationCount);
        public string? ObservedCommandId { get; private set; }

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.FromResult(runtimeId);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken) =>
            throw new InvalidOperationException(
                "recovery must use the durable command identity");

        public Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            string commandId,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref mutationCount);
            ObservedCommandId = commandId;
            return Task.CompletedTask;
        }

        public Task<RuntimeUnregistrationReceiptLookup>
            LookupUnregistrationReceiptAsync(
                string commandId,
                CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref lookupCount);
            ObservedCommandId = commandId;
            return Task.FromResult(
                receiptRuntimeIds is null
                    ? RuntimeUnregistrationReceiptLookup.Missing(
                        commandId,
                        ReplayHorizon(nextGeneration: 1))
                    : new RuntimeUnregistrationReceiptLookup(
                        commandId,
                        receiptRuntimeIds,
                        7,
                        new RuntimeUnregistrationReplayHorizon(
                            256,
                            1,
                            7,
                            7,
                            8,
                            6)));
        }
    }

    private sealed class ReplayHorizonRegistrationAuthority(
        RuntimeUnregistrationReplayHorizon replayHorizon) :
        IRuntimeRegistrationAuthority
    {
        private readonly TaskCompletionSource mutationStarted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource mutationRelease =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int mutationCount;

        public bool Enabled => true;
        public Task MutationStarted => mutationStarted.Task;
        public int MutationCount => Volatile.Read(ref mutationCount);

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.FromResult(runtimeId);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken) =>
            throw new InvalidOperationException(
                "recovery must use the durable command identity");

        public async Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            string commandId,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref mutationCount);
            mutationStarted.TrySetResult();
            await mutationRelease.Task.WaitAsync(cancellationToken);
        }

        public Task<RuntimeUnregistrationReceiptLookup>
            LookupUnregistrationReceiptAsync(
                string commandId,
                CancellationToken cancellationToken) =>
            Task.FromResult(
                RuntimeUnregistrationReceiptLookup.Missing(
                    commandId,
                    replayHorizon));

        public void ReleaseMutation() =>
            mutationRelease.TrySetResult();
    }

    private static RuntimeUnregistrationReplayHorizon ReplayHorizon(
        ulong nextGeneration) =>
        new(
            256,
            0,
            null,
            null,
            nextGeneration,
            nextGeneration - 1);

    private sealed class RuntimeDeletionRetryClaimRaceAuthority :
        IRuntimeRegistrationAuthority
    {
        private readonly TaskCompletionSource started =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource release =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int callCount;

        public bool Enabled => true;
        public int CallCount => Volatile.Read(ref callCount);
        public Task Started => started.Task;

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.FromResult(runtimeId);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public async Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            Assert.Single(runtimeIds);
            Interlocked.Increment(ref callCount);
            started.TrySetResult();
            await release.Task.WaitAsync(cancellationToken);
        }

        public void Release() => release.TrySetResult();
    }

    private sealed class RuntimeDeletionRetryRetentionAuthority :
        IRuntimeRegistrationAuthority
    {
        private readonly ConcurrentDictionary<string, int> attemptCounts =
            new(StringComparer.Ordinal);
        private int activeCalls;
        private int callCount;
        private int maxConcurrentCalls;

        public bool Enabled => true;
        public int CallCount => Volatile.Read(ref callCount);
        public int MaxConcurrentCalls =>
            Volatile.Read(ref maxConcurrentCalls);

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.FromResult(runtimeId);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public async Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            var runtimeId = Assert.Single(runtimeIds);
            attemptCounts.AddOrUpdate(
                runtimeId,
                1,
                static (_, count) => count + 1);
            Interlocked.Increment(ref callCount);
            var active = Interlocked.Increment(ref activeCalls);
            UpdateMaxConcurrency(active);
            try
            {
                await Task.Delay(4, cancellationToken);
            }
            finally
            {
                Interlocked.Decrement(ref activeCalls);
            }
        }

        public int AttemptCountFor(string runtimeId) =>
            attemptCounts.TryGetValue(runtimeId, out var count)
                ? count
                : 0;

        private void UpdateMaxConcurrency(int observed)
        {
            while (true)
            {
                var current = Volatile.Read(ref maxConcurrentCalls);
                if (observed <= current ||
                    Interlocked.CompareExchange(
                        ref maxConcurrentCalls,
                        observed,
                        current) == current)
                {
                    return;
                }
            }
        }
    }

    private enum RuntimeDeletionRetryClaimRaceMode
    {
        WorkerFirst,
        OperatorFirst,
        Simultaneous,
    }

    private sealed record RuntimeDeletionRetryClaimRaceResult(
        RuntimeDeletionRetryClaimRaceMode Mode,
        int AcceptedRetryCount,
        int InProgressConflictCount,
        int RevisionConflictCount,
        int UnexpectedResultCount,
        int AuthorityCallCount,
        int RetainedAuditCount,
        int ConvergedRuntimeCount);

    private sealed record RuntimeDeletionRetryRolloverCommand(
        PersistedRuntimeDeletionIntent Intent,
        string RequestId);

    private sealed class CancellationBlockingAuthority : IRuntimeRegistrationAuthority
    {
        private readonly TaskCompletionSource allSlotsStarted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int startedCallCount;
        private int cancelledCallCount;

        public bool Enabled => true;
        public Task AllSlotsStarted => allSlotsStarted.Task;
        public int StartedCallCount => Volatile.Read(ref startedCallCount);
        public int CancelledCallCount => Volatile.Read(ref cancelledCallCount);

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.FromResult(runtimeId);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public async Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            Assert.Single(runtimeIds);
            var started = Interlocked.Increment(ref startedCallCount);
            if (started == 8)
            {
                allSlotsStarted.TrySetResult();
            }
            try
            {
                await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
            }
            catch (OperationCanceledException) when (
                cancellationToken.IsCancellationRequested)
            {
                Interlocked.Increment(ref cancelledCallCount);
                throw;
            }
        }
    }

    private sealed class MixedQueueRuntimeDeletionAuthority(
        IReadOnlyCollection<string> poisonRuntimeIds,
        IReadOnlyCollection<string> slowRuntimeIds) : IRuntimeRegistrationAuthority
    {
        private readonly object sync = new();
        private readonly HashSet<string> poisonRuntimeIds =
            poisonRuntimeIds.ToHashSet(StringComparer.Ordinal);
        private readonly HashSet<string> slowRuntimeIds =
            slowRuntimeIds.ToHashSet(StringComparer.Ordinal);
        private readonly Dictionary<string, int> attemptCounts =
            new(StringComparer.Ordinal);
        private int activeCalls;
        private int maxConcurrentCalls;
        private int slowOperationCount;

        public bool Enabled => true;
        public int MaxConcurrentCalls => Volatile.Read(ref maxConcurrentCalls);
        public int SlowOperationCount => Volatile.Read(ref slowOperationCount);

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.FromResult(runtimeId);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public async Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            var runtimeId = Assert.Single(runtimeIds);
            lock (sync)
            {
                attemptCounts.TryGetValue(runtimeId, out var attempts);
                attemptCounts[runtimeId] = attempts + 1;
            }

            var active = Interlocked.Increment(ref activeCalls);
            UpdateMaxConcurrency(active);
            try
            {
                if (slowRuntimeIds.Contains(runtimeId))
                {
                    Interlocked.Increment(ref slowOperationCount);
                    await Task.Delay(100, cancellationToken);
                }
                else
                {
                    await Task.Delay(20, cancellationToken);
                }
                if (poisonRuntimeIds.Contains(runtimeId))
                {
                    throw new InvalidOperationException(
                        "test-only saturated queue poison failure");
                }
            }
            finally
            {
                Interlocked.Decrement(ref activeCalls);
            }
        }

        public int AttemptCountFor(string runtimeId)
        {
            lock (sync)
            {
                return attemptCounts.GetValueOrDefault(runtimeId);
            }
        }

        private void UpdateMaxConcurrency(int candidate)
        {
            while (true)
            {
                var current = Volatile.Read(ref maxConcurrentCalls);
                if (candidate <= current ||
                    Interlocked.CompareExchange(
                        ref maxConcurrentCalls,
                        candidate,
                        current) == current)
                {
                    return;
                }
            }
        }
    }

    private sealed class FailingRunStore : IOrchestraRunStore
    {
        private string? lastError;

        public string Provider => "failing-test";
        public string Location => "test";
        public int SchemaVersion => 0;
        public string? LastError => lastError;
        public IReadOnlyList<OrchestraRunSummary> LoadAll()
        {
            lastError = null;
            return Array.Empty<OrchestraRunSummary>();
        }
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) => Array.Empty<OrchestraRunEvent>();
        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null)
        {
            lastError = "orchestra_store_operation_failed";
            return false;
        }
        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs)
        {
            lastError = "orchestra_store_operation_failed";
            return false;
        }
        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds)
        {
            lastError = "orchestra_store_operation_failed";
            return false;
        }
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
        private string? lastError;

        public string Provider => "delete-failing-test";
        public string Location => "test";
        public int SchemaVersion => 0;
        public string? LastError => lastError;
        public IReadOnlyList<OrchestraRunSummary> LoadAll()
        {
            lastError = null;
            return inner.LoadAll();
        }
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) => inner.LoadEvents(runtimeId, runId);
        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null) => inner.Upsert(run, eventRecord);
        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs) => inner.ReplaceAll(runs);
        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds)
        {
            lastError = "orchestra_store_operation_failed";
            return false;
        }
    }

    private sealed class CountingDeleteRunStore : IOrchestraRunStore
    {
        private readonly InMemoryOrchestraRunStore inner = new();

        public int DeleteCount { get; private set; }
        public IReadOnlyList<string> LastDeletedRuntimeIds { get; private set; } =
            Array.Empty<string>();
        public string Provider => "counting-delete-test";
        public string Location => "test";
        public int SchemaVersion => 0;
        public string? LastError => null;
        public IReadOnlyList<OrchestraRunSummary> LoadAll() => inner.LoadAll();
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(string runtimeId, string runId) =>
            inner.LoadEvents(runtimeId, runId);
        public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null) =>
            inner.Upsert(run, eventRecord);
        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs) =>
            inner.ReplaceAll(runs);

        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds)
        {
            DeleteCount += 1;
            LastDeletedRuntimeIds = runtimeIds.ToArray();
            return inner.DeleteRuntimes(runtimeIds);
        }
    }

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = string.Empty;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
