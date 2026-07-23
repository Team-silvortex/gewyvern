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
        var intents = state?.PendingRuntimeDeletions
            ?? Array.Empty<PersistedRuntimeDeletionIntent>();
        if (intents.Count > MaxPendingRuntimeDeletionIntents)
        {
            throw new InvalidDataException(
                $"control-plane state contains more than {MaxPendingRuntimeDeletionIntents} pending runtime deletion intents");
        }

        var claimedRuntimeIds = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var persisted in intents)
        {
            var intentId = persisted.IntentId?.Trim() ?? string.Empty;
            var runtimeIds = (persisted.RuntimeIds ?? Array.Empty<string>())
                .Select(static runtimeId => runtimeId?.Trim() ?? string.Empty)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .OrderBy(static runtimeId => runtimeId, StringComparer.OrdinalIgnoreCase)
                .ToArray();
            if (!IsValidDeletionIdentifier(intentId) ||
                runtimeIds.Length is < 1 or > 128 ||
                runtimeIds.Any(static runtimeId => !IsValidDeletionIdentifier(runtimeId)) ||
                runtimeIds.Any(runtimeId => !claimedRuntimeIds.Add(runtimeId)) ||
                persisted.PreparedAt == default ||
                persisted.PreparedAt > DateTimeOffset.UtcNow.AddMinutes(5) ||
                !IsValidRuntimeDeletionRetryState(persisted))
            {
                throw new InvalidDataException(
                    $"control-plane state contains an invalid pending runtime deletion intent '{intentId}'");
            }

            var intent = new PersistedRuntimeDeletionIntent(
                intentId,
                Array.AsReadOnly(runtimeIds),
                persisted.PreparedAt,
                persisted.AttemptCount,
                persisted.LastAttemptAt,
                persisted.NextAttemptAt,
                persisted.LastFailureCode,
                persisted.Revision);
            if (!pendingRuntimeDeletions.TryAdd(intent.IntentId, intent))
            {
                throw new InvalidDataException(
                    $"control-plane state contains duplicate runtime deletion intent '{intent.IntentId}'");
            }
            foreach (var runtimeId in runtimeIds)
            {
                deletingRuntimes.Add(runtimeId);
            }
        }
    }

    private static bool IsValidDeletionIdentifier(string value) =>
        value.Length is > 0 and <= 128 &&
        value.All(static character =>
            char.IsAsciiLetterOrDigit(character) ||
            character is '.' or '-' or '_');

    private static bool IsValidRuntimeDeletionRetryState(
        PersistedRuntimeDeletionIntent intent)
    {
        if (intent.AttemptCount == 0)
        {
            return intent.LastAttemptAt is null &&
                intent.NextAttemptAt is null &&
                intent.LastFailureCode is null &&
                intent.Revision == 1;
        }
        if (intent.AttemptCount is < 0 or > MaxRuntimeDeletionAttempts ||
            intent.Revision < (long)intent.AttemptCount + 1 ||
            intent.Revision > MaxRuntimeDeletionRevision ||
            intent.LastAttemptAt is null ||
            intent.NextAttemptAt is null ||
            !RuntimeDeletionFailureCodes.IsValid(intent.LastFailureCode))
        {
            return false;
        }

        return intent.LastAttemptAt >= intent.PreparedAt &&
            intent.LastAttemptAt <= DateTimeOffset.UtcNow.AddMinutes(5) &&
            intent.NextAttemptAt >= intent.LastAttemptAt &&
            intent.NextAttemptAt <=
                intent.LastAttemptAt.Value.Add(MaxRuntimeDeletionRetryDelay);
    }

    private static ImmutableQueue<PersistedRuntimeDeletionRetryAudit>
        NormalizeRuntimeDeletionRetryAudit(PersistedControlPlaneState? state)
    {
        var persistedAudit = state?.RuntimeDeletionRetryAudit
            ?? Array.Empty<PersistedRuntimeDeletionRetryAudit>();
        if (persistedAudit.Count > MaxRuntimeDeletionRetryAuditEntries)
        {
            throw new InvalidDataException(
                $"control-plane state contains more than {MaxRuntimeDeletionRetryAuditEntries} runtime deletion retry audit entries");
        }

        var requestIds = new HashSet<string>(StringComparer.Ordinal);
        var normalized = new List<PersistedRuntimeDeletionRetryAudit>(
            persistedAudit.Count);
        foreach (var persisted in persistedAudit)
        {
            var requestId = persisted.RequestId?.Trim() ?? string.Empty;
            var intentId = persisted.IntentId?.Trim() ?? string.Empty;
            var requestedBy = persisted.RequestedBy?.Trim() ?? string.Empty;
            var runtimeIds = (persisted.RuntimeIds ?? Array.Empty<string>())
                .Select(static runtimeId => runtimeId?.Trim() ?? string.Empty)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .OrderBy(static runtimeId => runtimeId, StringComparer.OrdinalIgnoreCase)
                .ToArray();
            if (!IsValidDeletionIdentifier(requestId) ||
                !requestIds.Add(requestId) ||
                !IsValidDeletionIdentifier(intentId) ||
                runtimeIds.Length is < 1 or > 128 ||
                runtimeIds.Any(static runtimeId =>
                    !IsValidDeletionIdentifier(runtimeId)) ||
                !IsValidRuntimeDeletionRetryActor(requestedBy) ||
                persisted.ExpectedRevision < 1 ||
                persisted.ExpectedRevision >= MaxRuntimeDeletionRevision ||
                persisted.ResultingRevision !=
                    persisted.ExpectedRevision + 1 ||
                persisted.RequestedAt == default ||
                persisted.RequestedAt >
                    DateTimeOffset.UtcNow.AddMinutes(5))
            {
                throw new InvalidDataException(
                    $"control-plane state contains an invalid runtime deletion retry audit entry '{requestId}'");
            }

            normalized.Add(new PersistedRuntimeDeletionRetryAudit(
                requestId,
                intentId,
                Array.AsReadOnly(runtimeIds),
                persisted.ExpectedRevision,
                persisted.ResultingRevision,
                requestedBy,
                persisted.RequestedAt));
        }

        return normalized
            .OrderBy(static audit => audit.RequestedAt)
            .ThenBy(static audit => audit.RequestId, StringComparer.Ordinal)
            .Aggregate(
                ImmutableQueue<PersistedRuntimeDeletionRetryAudit>.Empty,
                static (queue, audit) => queue.Enqueue(audit));
    }

    private static bool IsValidRuntimeDeletionRetryActor(string value) =>
        value.Length is > 0 and <= 80 &&
        value.All(static character =>
            char.IsAsciiLetterOrDigit(character) ||
            character is '.' or '-' or '_' or '@');

    private void RestoreOrMigrateOrchestraRuns()
    {
        var databaseRuns = orchestraRunStore.LoadAll();
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
                var recoveryEvent = string.Equals(previous.Outcome, run.Outcome, StringComparison.OrdinalIgnoreCase)
                    ? null
                    : new OrchestraRunEvent(
                        0,
                        run.RunId,
                        run.RuntimeId,
                        "service_restart_recovery",
                        previous.Outcome,
                        run.Outcome,
                        "Service restart interrupted execution; retry explicitly if the plan is still applicable",
                        run.CompletedAt ?? DateTimeOffset.UtcNow);
                if (!orchestraRunStore.Upsert(run, recoveryEvent))
                {
                    throw new OrchestraPersistenceException($"failed to persist restored Orchestra run {run.RunId}");
                }
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
                state.RuntimeDeletionRetryAudit);
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
                state.RuntimeDeletionRetryAudit);
        }
    }
}
