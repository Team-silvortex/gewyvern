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

        return (restoredRuntimeCount, restoredSessionCount);
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
        foreach (var runtimeId in runtimeIds)
        {
            if (runtimes.TryRemove(runtimeId, out var runtime))
            {
                removedRuntimeNames.Add(runtime.Name);
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
        var state = ExportState();
        stateStore.Save(state.Runtimes, state.Sessions);
    }
}
