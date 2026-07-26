namespace Leserpent.ControlPlane;

internal static class ControlPlaneStateValidator
{
    internal const int MaxPendingRuntimeDeletionIntents = 256;
    internal const int MaxRuntimeDeletionAttempts = 1_000_000;
    internal const int MaxRuntimeDeletionRetryAuditEntries = 256;
    internal const int MaxOrchestraRunSteps = 256;
    internal const int MaxRuntimeCapabilities = 256;
    internal const int MaxSessionRequirements = 256;
    internal const int MaxSidecarMemorySlots = 256;
    internal const long MaxRuntimeDeletionRevision = 1_000_000_000;
    internal static readonly TimeSpan MaxRuntimeDeletionRetryDelay =
        TimeSpan.FromSeconds(30);

    internal static void Validate(
        PersistedControlPlaneState state,
        DateTimeOffset? now = null)
    {
        var observedAt = now ?? DateTimeOffset.UtcNow;
        if (state.SavedAt == default ||
            state.SavedAt > observedAt.AddMinutes(5) ||
            state.Runtimes is null ||
            state.Sessions is null)
        {
            throw new InvalidDataException(
                "control-plane state metadata is invalid");
        }

        ValidateProjectionGraph(state, observedAt);
        _ = NormalizePendingRuntimeDeletions(state, observedAt);
        _ = NormalizeRuntimeDeletionRetryAudit(state, observedAt);
    }

    internal static void ValidateProjectionGraph(
        PersistedControlPlaneState state,
        DateTimeOffset? now = null)
    {
        ValidateRuntimeSessionGraph(state, now);
        ValidateLegacyOrchestraRunGraph(state, now);
    }

    internal static void ValidateRuntimeSessionGraph(
        PersistedControlPlaneState state,
        DateTimeOffset? now = null)
    {
        var observedAt = now ?? DateTimeOffset.UtcNow;
        var runtimeIds = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        var canonicalRuntimeIds = new HashSet<string>(
            StringComparer.Ordinal);
        foreach (var runtime in state.Runtimes)
        {
            if (runtime is null ||
                !IsStableIdentity(runtime.RuntimeId) ||
                !runtimeIds.Add(runtime.RuntimeId))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid or duplicate runtime identity");
            }
            canonicalRuntimeIds.Add(runtime.RuntimeId);
            ValidateRuntimePayload(runtime, observedAt);
        }

        var sessionIds = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        foreach (var session in state.Sessions)
        {
            if (session is null ||
                !IsStableIdentity(session.SessionId) ||
                !sessionIds.Add(session.SessionId))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid or duplicate session identity");
            }
            if (!IsStableIdentity(session.RuntimeId) ||
                !canonicalRuntimeIds.Contains(session.RuntimeId))
            {
                throw new InvalidDataException(
                    "control-plane state contains a session without a registered runtime");
            }
            ValidateSessionPayload(session, observedAt);
        }
    }

    private static void ValidateRuntimePayload(
        PersistedRuntimeState runtime,
        DateTimeOffset observedAt)
    {
        if (!IsCanonicalText(runtime.Name, 256) ||
            !IsCanonicalText(runtime.Endpoint, 2_048) ||
            (runtime.SidecarEndpoint is not null &&
                !IsCanonicalText(runtime.SidecarEndpoint, 2_048)) ||
            runtime.RegisteredAt == default ||
            runtime.RegisteredAt > observedAt.AddMinutes(5) ||
            runtime.UpdatedAt < runtime.RegisteredAt ||
            runtime.UpdatedAt > observedAt.AddMinutes(5) ||
            !IsCanonicalText(runtime.CapabilitySource, 128) ||
            (runtime.CapabilityFetchedAt is not null &&
                (runtime.CapabilityFetchedAt == default ||
                    runtime.CapabilityFetchedAt >
                        observedAt.AddMinutes(5))) ||
            runtime.Capabilities is null ||
            runtime.Capabilities.Count > MaxRuntimeCapabilities ||
            runtime.Tags is null ||
            runtime.Status is null)
        {
            throw new InvalidDataException(
                "control-plane state contains an invalid runtime payload");
        }

        var capabilityKeys = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        foreach (var capability in runtime.Capabilities)
        {
            if (capability is null ||
                !IsCanonicalText(capability.Key, 128) ||
                !capabilityKeys.Add(capability.Key) ||
                !IsKnownCapabilitySupport(capability.Support) ||
                capability.Description is null)
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid runtime capability");
            }
        }

        if (!IsOptionalCanonicalText(runtime.Tags.Environment, 128) ||
            !IsOptionalCanonicalText(runtime.Tags.Cluster, 128) ||
            !IsOptionalCanonicalText(runtime.Tags.Role, 128) ||
            !IsValidRuntimeStatus(runtime.Status, observedAt) ||
            (runtime.SidecarStatus is not null &&
                !IsValidRuntimeSidecarStatus(
                    runtime.SidecarStatus,
                    observedAt)))
        {
            throw new InvalidDataException(
                "control-plane state contains invalid runtime metadata");
        }
    }

    private static void ValidateSessionPayload(
        PersistedSessionState session,
        DateTimeOffset observedAt)
    {
        if (!IsCanonicalText(session.PipelineKind, 128) ||
            !IsCanonicalText(session.RequestedBy, 256) ||
            !IsKnownSessionStatus(session.Status) ||
            session.CreatedAt == default ||
            session.CreatedAt > observedAt.AddMinutes(5) ||
            session.UpdatedAt < session.CreatedAt ||
            session.UpdatedAt > observedAt.AddMinutes(5) ||
            session.Requirements is null ||
            session.Requirements.Count > MaxSessionRequirements)
        {
            throw new InvalidDataException(
                "control-plane state contains an invalid session payload");
        }

        var requirementKeys = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        foreach (var requirement in session.Requirements)
        {
            if (requirement is null ||
                !IsCanonicalText(requirement.Key, 128) ||
                !requirementKeys.Add(requirement.Key) ||
                !IsKnownCapabilitySupport(
                    requirement.MinimumSupport))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid session capability requirement");
            }
        }
    }

    private static bool IsValidRuntimeStatus(
        RuntimeStatusSnapshot status,
        DateTimeOffset observedAt) =>
        IsCanonicalText(status.StatusSource, 128) &&
        IsOptionalObservedTimestamp(
            status.StatusFetchedAt,
            observedAt) &&
        IsOptionalCanonicalText(status.SnapshotKind, 128) &&
        status.TargetCount is null or >= 0 &&
        status.SocketConsecutiveIdleTimeouts is null or >= 0 &&
        status.SocketTotalIdleTimeouts is null or >= 0;

    private static bool IsValidRuntimeSidecarStatus(
        RuntimeSidecarStatusSnapshot status,
        DateTimeOffset observedAt) =>
        IsCanonicalText(status.StatusSource, 128) &&
        IsCanonicalText(status.DaemonStatus, 128) &&
        IsOptionalObservedTimestamp(
            status.StatusFetchedAt,
            observedAt) &&
        status.TargetCount is null or >= 0 &&
        status.LearnedRoutes >= 0 &&
        (status.Memory is null ||
            IsValidSidecarMemory(status.Memory, observedAt));

    private static bool IsValidSidecarMemory(
        RuntimeSidecarMemorySnapshot memory,
        DateTimeOffset observedAt)
    {
        if (memory.SlotCount < 0 ||
            memory.HistoryCount < 0 ||
            !IsOptionalCanonicalText(memory.LatestSlot, 128) ||
            !IsOptionalCanonicalText(memory.LatestSource, 128) ||
            memory.Slots is null ||
            memory.Slots.Count > MaxSidecarMemorySlots)
        {
            return false;
        }

        var slotIds = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        return memory.Slots.All(slot =>
            slot is not null &&
            IsCanonicalText(slot.Slot, 128) &&
            slotIds.Add(slot.Slot) &&
            IsCanonicalText(slot.Source, 128) &&
            IsOptionalObservedTimestamp(slot.SavedAt, observedAt) &&
            slot.PatternCount >= 0 &&
            slot.LabelCount >= 0);
    }

    internal static void ValidateLegacyOrchestraRunGraph(
        PersistedControlPlaneState state,
        DateTimeOffset? now = null)
    {
        var observedAt = now ?? DateTimeOffset.UtcNow;
        var canonicalRuntimeIds = state.Runtimes
            .Select(static runtime => runtime.RuntimeId)
            .ToHashSet(StringComparer.Ordinal);
        var runIds = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        var runs = state.OrchestraRuns ??
            Array.Empty<OrchestraRunSummary>();
        var runsById = new Dictionary<string, OrchestraRunSummary>(
            StringComparer.OrdinalIgnoreCase);
        var requestIdsByRuntime =
            new Dictionary<string, HashSet<string>>(StringComparer.Ordinal);
        foreach (var run in runs)
        {
            if (run is null ||
                !IsStableIdentity(run.RunId) ||
                !runIds.Add(run.RunId))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid or duplicate Orchestra run identity");
            }
            if (!IsStableIdentity(run.RuntimeId) ||
                !canonicalRuntimeIds.Contains(run.RuntimeId))
            {
                throw new InvalidDataException(
                    "control-plane state contains an Orchestra run without a registered runtime");
            }
            if (!IsStableIdentity(run.PlanId) ||
                !IsKnownOrchestraOutcome(run.Outcome) ||
                run.ExecutedAt == default ||
                run.ExecutedAt > observedAt.AddMinutes(5) ||
                (IsActiveOrchestraOutcome(run.Outcome) &&
                    run.CompletedAt is not null) ||
                (run.CompletedAt is not null &&
                    (run.CompletedAt < run.ExecutedAt ||
                        run.CompletedAt >
                            observedAt.AddMinutes(5))))
            {
                throw new InvalidDataException(
                    "control-plane state contains invalid Orchestra lifecycle metadata");
            }
            if (run.Steps is null ||
                run.Steps.Count > MaxOrchestraRunSteps ||
                run.Steps.Any(static step =>
                    step is null ||
                    !IsStableIdentity(step.Step) ||
                    !IsStableIdentity(step.Outcome) ||
                    step.Summary is null))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid Orchestra step payload");
            }
            if (run.Attempt < 1 ||
                (run.RetriedFromRunId is null && run.Attempt != 1) ||
                (run.RetriedFromRunId is not null &&
                    (run.Attempt < 2 ||
                        !IsStableIdentity(run.RetriedFromRunId) ||
                        string.Equals(
                            run.RunId,
                            run.RetriedFromRunId,
                            StringComparison.OrdinalIgnoreCase))))
            {
                throw new InvalidDataException(
                    "control-plane state contains invalid Orchestra retry lineage");
            }
            if (run.RequestId is not null)
            {
                if (!IsStableIdentity(run.RequestId))
                {
                    throw new InvalidDataException(
                        "control-plane state contains a duplicate Orchestra request identity");
                }
                if (!requestIdsByRuntime.TryGetValue(
                        run.RuntimeId,
                        out var requestIds))
                {
                    requestIds = new HashSet<string>(
                        StringComparer.Ordinal);
                    requestIdsByRuntime.Add(
                        run.RuntimeId,
                        requestIds);
                }
                if (!requestIds.Add(run.RequestId))
                {
                    throw new InvalidDataException(
                        "control-plane state contains a duplicate Orchestra request identity");
                }
            }
            runsById.Add(run.RunId, run);
        }

        foreach (var run in runs)
        {
            if (run.RetriedFromRunId is null ||
                !runsById.TryGetValue(
                    run.RetriedFromRunId,
                    out var parent))
            {
                continue;
            }
            if (!string.Equals(
                    parent.RunId,
                    run.RetriedFromRunId,
                    StringComparison.Ordinal) ||
                !string.Equals(
                    parent.RuntimeId,
                    run.RuntimeId,
                    StringComparison.Ordinal) ||
                !string.Equals(
                    parent.PlanId,
                    run.PlanId,
                    StringComparison.OrdinalIgnoreCase) ||
                !IsTerminalOrchestraOutcome(parent.Outcome) ||
                (long)run.Attempt != (long)parent.Attempt + 1 ||
                run.ExecutedAt < parent.ExecutedAt)
            {
                throw new InvalidDataException(
                    "control-plane state contains invalid Orchestra retry lineage");
            }
        }
    }

    internal static IReadOnlyList<PersistedRuntimeDeletionIntent>
        NormalizePendingRuntimeDeletions(
            PersistedControlPlaneState? state,
            DateTimeOffset? now = null)
    {
        var intents = state?.PendingRuntimeDeletions
            ?? Array.Empty<PersistedRuntimeDeletionIntent>();
        if (intents.Count > MaxPendingRuntimeDeletionIntents)
        {
            throw new InvalidDataException(
                $"control-plane state contains more than {MaxPendingRuntimeDeletionIntents} pending runtime deletion intents");
        }

        var observedAt = now ?? DateTimeOffset.UtcNow;
        var intentIds = new HashSet<string>(StringComparer.Ordinal);
        var claimedRuntimeIds = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        var normalized = new List<PersistedRuntimeDeletionIntent>(
            intents.Count);
        foreach (var persisted in intents)
        {
            var intentId = persisted.IntentId?.Trim() ?? string.Empty;
            var runtimeIds = (persisted.RuntimeIds ?? Array.Empty<string>())
                .Select(static runtimeId =>
                    runtimeId?.Trim() ?? string.Empty)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .OrderBy(
                    static runtimeId => runtimeId,
                    StringComparer.OrdinalIgnoreCase)
                .ToArray();
            if (!IsValidDeletionIdentifier(intentId) ||
                !intentIds.Add(intentId) ||
                runtimeIds.Length is < 1 or > 128 ||
                runtimeIds.Any(static runtimeId =>
                    !IsValidDeletionIdentifier(runtimeId)) ||
                runtimeIds.Any(runtimeId =>
                    !claimedRuntimeIds.Add(runtimeId)) ||
                persisted.PreparedAt == default ||
                persisted.PreparedAt > observedAt.AddMinutes(5) ||
                !IsValidRuntimeDeletionRetryState(
                    persisted,
                    observedAt))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid pending runtime deletion intent");
            }

            normalized.Add(new PersistedRuntimeDeletionIntent(
                intentId,
                Array.AsReadOnly(runtimeIds),
                persisted.PreparedAt,
                persisted.AttemptCount,
                persisted.LastAttemptAt,
                persisted.NextAttemptAt,
                persisted.LastFailureCode,
                persisted.Revision));
        }

        return normalized;
    }

    internal static IReadOnlyList<PersistedRuntimeDeletionRetryAudit>
        NormalizeRuntimeDeletionRetryAudit(
            PersistedControlPlaneState? state,
            DateTimeOffset? now = null)
    {
        var persistedAudit = state?.RuntimeDeletionRetryAudit
            ?? Array.Empty<PersistedRuntimeDeletionRetryAudit>();
        if (persistedAudit.Count > MaxRuntimeDeletionRetryAuditEntries)
        {
            throw new InvalidDataException(
                $"control-plane state contains more than {MaxRuntimeDeletionRetryAuditEntries} runtime deletion retry audit entries");
        }

        var observedAt = now ?? DateTimeOffset.UtcNow;
        var requestIds = new HashSet<string>(StringComparer.Ordinal);
        var normalized = new List<PersistedRuntimeDeletionRetryAudit>(
            persistedAudit.Count);
        foreach (var persisted in persistedAudit)
        {
            var requestId = persisted.RequestId?.Trim() ?? string.Empty;
            var intentId = persisted.IntentId?.Trim() ?? string.Empty;
            var requestedBy = persisted.RequestedBy?.Trim() ?? string.Empty;
            var runtimeIds = (persisted.RuntimeIds ?? Array.Empty<string>())
                .Select(static runtimeId =>
                    runtimeId?.Trim() ?? string.Empty)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .OrderBy(
                    static runtimeId => runtimeId,
                    StringComparer.OrdinalIgnoreCase)
                .ToArray();
            if (!IsValidDeletionIdentifier(requestId) ||
                !requestIds.Add(requestId) ||
                !IsValidDeletionIdentifier(intentId) ||
                runtimeIds.Length is < 1 or > 128 ||
                runtimeIds.Any(static runtimeId =>
                    !IsValidDeletionIdentifier(runtimeId)) ||
                !IsValidRuntimeDeletionRetryActor(requestedBy) ||
                persisted.ExpectedRevision < 1 ||
                persisted.ExpectedRevision >=
                    MaxRuntimeDeletionRevision ||
                persisted.ResultingRevision !=
                    persisted.ExpectedRevision + 1 ||
                persisted.RequestedAt == default ||
                persisted.RequestedAt > observedAt.AddMinutes(5))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid runtime deletion retry audit entry");
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

        return normalized;
    }

    internal static bool IsValidDeletionIdentifier(string value) =>
        value.Length is > 0 and <= 128 &&
        value.All(static character =>
            char.IsAsciiLetterOrDigit(character) ||
            character is '.' or '-' or '_');

    internal static bool IsValidRuntimeDeletionRetryActor(string value) =>
        value.Length is > 0 and <= 80 &&
        value.All(static character =>
            char.IsAsciiLetterOrDigit(character) ||
            character is '.' or '-' or '_' or '@');

    internal static bool IsTerminalOrchestraOutcome(string outcome) =>
        string.Equals(
            outcome,
            "succeeded",
            StringComparison.OrdinalIgnoreCase) ||
        string.Equals(
            outcome,
            "degraded",
            StringComparison.OrdinalIgnoreCase) ||
        string.Equals(
            outcome,
            "failed",
            StringComparison.OrdinalIgnoreCase) ||
        string.Equals(
            outcome,
            "cancelled",
            StringComparison.OrdinalIgnoreCase) ||
        string.Equals(
            outcome,
            "ok",
            StringComparison.OrdinalIgnoreCase);

    internal static bool IsActiveOrchestraOutcome(string outcome) =>
        string.Equals(
            outcome,
            "queued",
            StringComparison.OrdinalIgnoreCase) ||
        string.Equals(
            outcome,
            "running",
            StringComparison.OrdinalIgnoreCase);

    private static bool IsKnownOrchestraOutcome(string outcome) =>
        IsActiveOrchestraOutcome(outcome) ||
        IsTerminalOrchestraOutcome(outcome);

    private static bool IsKnownCapabilitySupport(string? support) =>
        string.Equals(
            support,
            "fully_supported",
            StringComparison.Ordinal) ||
        string.Equals(
            support,
            "risky",
            StringComparison.Ordinal) ||
        string.Equals(
            support,
            "not_supported",
            StringComparison.Ordinal);

    private static bool IsKnownSessionStatus(string? status) =>
        string.Equals(
            status,
            "running",
            StringComparison.Ordinal) ||
        string.Equals(
            status,
            "stopped",
            StringComparison.Ordinal);

    private static bool IsCanonicalText(
        string? value,
        int maxLength) =>
        value is not null &&
        value.Length is > 0 &&
        value.Length <= maxLength &&
        string.Equals(
            value,
            value.Trim(),
            StringComparison.Ordinal) &&
        value.All(static character => !char.IsControl(character));

    private static bool IsOptionalCanonicalText(
        string? value,
        int maxLength) =>
        value is null || IsCanonicalText(value, maxLength);

    private static bool IsOptionalObservedTimestamp(
        DateTimeOffset? value,
        DateTimeOffset observedAt) =>
        value is null ||
        (value != default &&
            value <= observedAt.AddMinutes(5));

    private static bool IsStableIdentity(string? value) =>
        value is not null &&
        value.Length is > 0 and <= 128 &&
        string.Equals(value, value.Trim(), StringComparison.Ordinal) &&
        value.All(static character => !char.IsControl(character));

    private static bool IsValidRuntimeDeletionRetryState(
        PersistedRuntimeDeletionIntent intent,
        DateTimeOffset observedAt)
    {
        if (intent.AttemptCount == 0)
        {
            return intent.LastAttemptAt is null &&
                intent.NextAttemptAt is null &&
                intent.LastFailureCode is null &&
                intent.Revision == 1;
        }
        if (intent.AttemptCount is < 0 or
                > MaxRuntimeDeletionAttempts ||
            intent.Revision < (long)intent.AttemptCount + 1 ||
            intent.Revision > MaxRuntimeDeletionRevision ||
            intent.LastAttemptAt is null ||
            intent.NextAttemptAt is null ||
            !RuntimeDeletionFailureCodes.IsValid(
                intent.LastFailureCode))
        {
            return false;
        }

        return intent.LastAttemptAt >= intent.PreparedAt &&
            intent.LastAttemptAt <= observedAt.AddMinutes(5) &&
            intent.NextAttemptAt >= intent.LastAttemptAt &&
            intent.NextAttemptAt <=
                intent.LastAttemptAt.Value.Add(
                    MaxRuntimeDeletionRetryDelay);
    }
}
