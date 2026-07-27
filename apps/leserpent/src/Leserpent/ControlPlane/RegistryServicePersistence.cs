using System.Collections.Immutable;

namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    private (int RuntimeCount, int SessionCount) RestorePersistedState(PersistedControlPlaneState? state)
    {
        if (state is null)
        {
            return (0, 0);
        }

        var restoredRuntimeCount = 0;
        foreach (var runtime in state.Runtimes)
        {
            var restored = new RuntimeRecord(
                runtime.RuntimeId,
                runtime.Name.Trim(),
                runtime.Endpoint.Trim(),
                null,
                NormalizeOptionalEndpoint(runtime.SidecarEndpoint),
                null,
                runtime.RegisteredAt,
                runtime.UpdatedAt,
                NormalizeCapabilities(runtime.Capabilities),
                string.IsNullOrWhiteSpace(runtime.CapabilitySource) ? "manual" : runtime.CapabilitySource.Trim(),
                runtime.CapabilityFetchedAt,
                runtime.CapabilityFetchError,
                NormalizeTags(runtime.Tags),
                runtime.Status,
                runtime.SidecarStatus);
            runtimes[restored.RuntimeId] = restored;
            restoredRuntimeCount += 1;
        }

        var restoredSessionCount = 0;
        foreach (var session in state.Sessions)
        {
            var restored = new SessionRecord(
                session.SessionId,
                session.RuntimeId.Trim(),
                session.PipelineKind.Trim(),
                session.RequestedBy.Trim(),
                session.Status.Trim(),
                session.CreatedAt,
                session.UpdatedAt,
                NormalizeRequirements(session.Requirements));
            sessions[restored.SessionId] = restored;
            restoredSessionCount += 1;
        }

        foreach (var group in (state.OrchestraRuns ?? Array.Empty<OrchestraRunSummary>())
            .Where(run => runtimes.ContainsKey(run.RuntimeId))
            .OrderBy(run => run.ExecutedAt)
            .GroupBy(run => run.RuntimeId, StringComparer.OrdinalIgnoreCase))
        {
            orchestraRuns[group.Key] = group
                .TakeLast(MaxOrchestraRunsPerRuntime)
                .Select(NormalizeRestoredOrchestraRun)
                .Aggregate(ImmutableQueue<OrchestraRunSummary>.Empty, static (queue, run) => queue.Enqueue(run));
        }

        return (restoredRuntimeCount, restoredSessionCount);
    }

    private void RestorePendingRuntimeDeletions(PersistedControlPlaneState? state)
    {
        var intents =
            ControlPlaneStateValidator.NormalizePendingRuntimeDeletions(
                state);
        foreach (var persisted in intents)
        {
            if (!pendingRuntimeDeletions.TryAdd(
                persisted.IntentId,
                persisted))
            {
                throw new InvalidDataException(
                    "control-plane state contains a duplicate pending runtime deletion intent");
            }
            foreach (var runtimeId in persisted.RuntimeIds)
            {
                deletingRuntimes.Add(runtimeId);
            }
        }
    }

    private static bool IsValidDeletionIdentifier(string value) =>
        ControlPlaneStateValidator.IsValidDeletionIdentifier(value);

    private static ImmutableQueue<PersistedRuntimeDeletionRetryAudit>
        NormalizeRuntimeDeletionRetryAudit(PersistedControlPlaneState? state)
    {
        return ControlPlaneStateValidator
            .NormalizeRuntimeDeletionRetryAudit(state)
            .Aggregate(
            ImmutableQueue<PersistedRuntimeDeletionRetryAudit>.Empty,
            static (queue, audit) => queue.Enqueue(audit));
    }

    private static ImmutableQueue<
        PersistedRuntimeDeletionReconciliationAudit>
        NormalizeRuntimeDeletionReconciliationAudit(
            PersistedControlPlaneState? state)
    {
        return ControlPlaneStateValidator
            .NormalizeRuntimeDeletionReconciliationAudit(state)
            .Aggregate(
                ImmutableQueue<
                    PersistedRuntimeDeletionReconciliationAudit>.Empty,
                static (queue, audit) => queue.Enqueue(audit));
    }

    private static PersistedOrchestraDeleteCheckpointMonitor?
        NormalizeOrchestraDeleteCheckpointMonitor(
            PersistedControlPlaneState? state) =>
        ControlPlaneStateValidator
            .NormalizeOrchestraDeleteCheckpointMonitor(
                state?.OrchestraDeleteCheckpointMonitor);

    private static ImmutableQueue<
        PersistedOrchestraDeleteCheckpointAlertDelivery>
        NormalizeOrchestraDeleteCheckpointAlertOutbox(
            PersistedControlPlaneState? state) =>
        ControlPlaneStateValidator
            .NormalizeOrchestraDeleteCheckpointAlertOutbox(
                state?.OrchestraDeleteCheckpointAlertOutbox)
            .Aggregate(
                ImmutableQueue<
                    PersistedOrchestraDeleteCheckpointAlertDelivery>.Empty,
                static (queue, delivery) =>
                    queue.Enqueue(delivery));

    private static bool IsValidRuntimeDeletionRetryActor(string value) =>
        ControlPlaneStateValidator.IsValidRuntimeDeletionRetryActor(
            value);

    private void RestoreOrMigrateOrchestraRuns()
    {
        var databaseRuns = orchestraRunStore.LoadAll();
        if (!string.IsNullOrWhiteSpace(orchestraRunStore.LastError))
        {
            if (orchestraRunStore
                .DeleteReplayHorizonAvailabilityMayBeTransient)
            {
                return;
            }
            throw new OrchestraPersistenceException(
                "failed to validate persisted Orchestra history");
        }
        if (databaseRuns.Count == 0)
        {
            var legacyRuns = orchestraRuns.Values.SelectMany(static queue => queue).ToArray();
            if (legacyRuns.Length > 0)
            {
                if (!orchestraRunStore.ReplaceAll(legacyRuns))
                {
                    throw new OrchestraPersistenceException("failed to migrate legacy Orchestra runs into the database");
                }
            }
            return;
        }

        orchestraRuns.Clear();
        foreach (var group in databaseRuns
            .Where(run => runtimes.ContainsKey(run.RuntimeId))
            .OrderBy(run => run.ExecutedAt)
            .GroupBy(run => run.RuntimeId, StringComparer.OrdinalIgnoreCase))
        {
            var restored = group
                .TakeLast(MaxOrchestraRunsPerRuntime)
                .ToArray();
            var normalized = restored.Select(NormalizeRestoredOrchestraRun).ToArray();
            orchestraRuns[group.Key] = normalized.Aggregate(
                ImmutableQueue<OrchestraRunSummary>.Empty,
                static (queue, run) => queue.Enqueue(run));
            for (var index = 0; index < normalized.Length; index += 1)
            {
                var run = normalized[index];
                var previous = restored[index];
                var events = orchestraRunStore.LoadEvents(
                    previous.RuntimeId,
                    previous.RunId);
                if (!string.IsNullOrWhiteSpace(
                        orchestraRunStore.LastError))
                {
                    throw new OrchestraPersistenceException(
                        "failed to validate persisted Orchestra event history");
                }
                if (events.Count == 0)
                {
                    if (!orchestraRunStore.Upsert(
                            previous,
                            ControlPlaneStateValidator
                                .CreateLegacyOrchestraImportEvent(
                                    previous)))
                    {
                        throw new OrchestraPersistenceException(
                            $"failed to backfill Orchestra event history for run {run.RunId}");
                    }
                }

                if (!string.Equals(
                        previous.Outcome,
                        run.Outcome,
                        StringComparison.Ordinal))
                {
                    var recoveryEvent = new OrchestraRunEvent(
                        0,
                        run.RunId,
                        run.RuntimeId,
                        "service_restart_recovery",
                        previous.Outcome,
                        run.Outcome,
                        "Service restart interrupted execution; retry explicitly if the plan is still applicable",
                        run.CompletedAt ?? DateTimeOffset.UtcNow);
                    if (!orchestraRunStore.Upsert(
                            run,
                            recoveryEvent))
                    {
                        throw new OrchestraPersistenceException(
                            $"failed to persist restored Orchestra run {run.RunId}");
                    }
                }

                events = orchestraRunStore.LoadEvents(
                    run.RuntimeId,
                    run.RunId);
                if (!string.IsNullOrWhiteSpace(
                        orchestraRunStore.LastError))
                {
                    throw new OrchestraPersistenceException(
                        "failed to validate restored Orchestra event history");
                }
                ControlPlaneStateValidator
                    .ValidateOrchestraEventSequence(
                        run,
                        events,
                        run.RuntimeId,
                        run.RunId);
            }
        }
    }

    private (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames)
        DeleteRuntimesWhere(Func<RuntimeRecord, bool> predicate)
    {
        var runtimeIds = runtimes.Values
            .Where(predicate)
            .Select(runtime => runtime.RuntimeId)
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();

        if (runtimeIds.Length == 0)
        {
            return (0, 0, Array.Empty<string>());
        }

        var runtimeIdSet = runtimeIds.ToHashSet(StringComparer.OrdinalIgnoreCase);
        var removedRuntimeNames = new List<string>(runtimeIds.Length);
        lock (orchestraRunSync)
        {
            var activeRuns = FindActiveOrchestraRuns(runtimeIds);
            if (activeRuns.Count > 0)
            {
                throw new OrchestraRuntimeBusyException(activeRuns);
            }
            if (!orchestraRunStore.DeleteRuntimes(runtimeIds))
            {
                throw new OrchestraPersistenceException("failed to delete Orchestra history for selected runtimes");
            }
            foreach (var runtimeId in runtimeIds)
            {
                if (runtimes.TryRemove(runtimeId, out var runtime))
                {
                    removedRuntimeNames.Add(runtime.Name);
                }
                orchestraRuns.TryRemove(runtimeId, out _);
            }
        }

        var removedSessionIds = sessions.Values
            .Where(session => runtimeIdSet.Contains(session.RuntimeId))
            .Select(session => session.SessionId)
            .ToArray();

        foreach (var sessionId in removedSessionIds)
        {
            sessions.TryRemove(sessionId, out _);
        }

        foreach (var runtimeId in runtimeIdSet)
        {
            recoveryActivities.TryRemove(runtimeId, out _);
        }

        if (removedRuntimeNames.Count > 0 || removedSessionIds.Length > 0)
        {
            PersistState();
        }

        removedRuntimeNames.Sort(StringComparer.OrdinalIgnoreCase);
        return (removedRuntimeNames.Count, removedSessionIds.Length, removedRuntimeNames);
    }

    private void PersistState()
    {
        lock (persistenceSync)
        {
            var state = ExportState();
            stateStore.Save(
                state.Runtimes,
                state.Sessions,
                state.OrchestraRuns,
                state.PendingRuntimeDeletions,
                state.RuntimeDeletionRetryAudit,
                state.RuntimeDeletionReconciliationAudit,
                state.OrchestraDeleteCheckpointMonitor,
                state.OrchestraDeleteCheckpointAlertOutbox);
        }
    }

    private void PersistStateStrict()
    {
        lock (persistenceSync)
        {
            var state = ExportState();
            stateStore.SaveStrict(
                state.Runtimes,
                state.Sessions,
                state.OrchestraRuns,
                state.PendingRuntimeDeletions,
                state.RuntimeDeletionRetryAudit,
                state.RuntimeDeletionReconciliationAudit,
                state.OrchestraDeleteCheckpointMonitor,
                state.OrchestraDeleteCheckpointAlertOutbox);
        }
    }
}
