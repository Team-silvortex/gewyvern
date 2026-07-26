namespace Leserpent.ControlPlane;

internal static class ControlPlaneStateValidator
{
    internal const int MaxPendingRuntimeDeletionIntents = 256;
    internal const int MaxRuntimeDeletionAttempts = 1_000_000;
    internal const int MaxRuntimeDeletionRetryAuditEntries = 256;
    internal const int MaxRuntimeDeletionReconciliationAuditEntries = 256;
    internal const int MaxOrchestraRunSteps = 256;
    internal const int MaxOrchestraRunAttempts = 1_000_000;
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
        _ = NormalizeRuntimeDeletionReconciliationAudit(
            state,
            observedAt);
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
            (runtime.CapabilityFetchedAt is not null &&
                (runtime.CapabilityFetchedAt == default ||
                    runtime.CapabilityFetchedAt >
                        observedAt.AddMinutes(5))) ||
            !IsValidCapabilityPosture(runtime) ||
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
                !IsCanonicalText(capability.Description, 1_024))
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
        DateTimeOffset observedAt)
    {
        var posture = status.StatusSource switch
        {
            "unobserved" =>
                status.StatusFetchedAt is null &&
                status.StatusFetchError is null &&
                !status.HasLatestSnapshot,
            "gewyvern-api" =>
                status.StatusFetchedAt is not null &&
                status.StatusFetchError is null,
            "fetch_failed" =>
                status.StatusFetchedAt is null &&
                string.Equals(
                    status.StatusFetchError,
                    RuntimeDiagnosticCodes.RuntimeStatusFetchFailed,
                    StringComparison.Ordinal) &&
                !status.HasLatestSnapshot,
            _ => false,
        };
        return posture &&
            IsOptionalObservedTimestamp(
                status.StatusFetchedAt,
                observedAt) &&
            IsOptionalCanonicalText(status.SnapshotKind, 128) &&
            IsOptionalCanonicalText(status.ResilienceStatus, 128) &&
            IsOptionalCanonicalText(status.ResilienceSummary, 1_024) &&
            IsOptionalCanonicalText(status.SocketServiceStatus, 128) &&
            status.TargetCount is null or >= 0 &&
            status.SocketConsecutiveIdleTimeouts is null or >= 0 &&
            status.SocketTotalIdleTimeouts is null or >= 0;
    }

    private static bool IsValidRuntimeSidecarStatus(
        RuntimeSidecarStatusSnapshot status,
        DateTimeOffset observedAt)
    {
        var posture = status.StatusSource switch
        {
            "etragon-api" =>
                status.StatusFetchedAt is not null &&
                status.StatusFetchError is null &&
                (status.LastError is null ||
                    string.Equals(
                        status.LastError,
                        RuntimeDiagnosticCodes.SidecarReportedError,
                        StringComparison.Ordinal)),
            "fetch_failed" =>
                status.StatusFetchedAt is null &&
                string.Equals(
                    status.StatusFetchError,
                    RuntimeDiagnosticCodes.SidecarFetchFailed,
                    StringComparison.Ordinal) &&
                (status.LastError is null ||
                    string.Equals(
                        status.LastError,
                        RuntimeDiagnosticCodes.SidecarFetchFailed,
                        StringComparison.Ordinal)) &&
                !status.Healthy,
            _ => false,
        };
        return posture &&
            IsCanonicalText(status.DaemonStatus, 128) &&
            IsOptionalObservedTimestamp(
                status.StatusFetchedAt,
                observedAt) &&
            status.TargetCount is null or >= 0 &&
            status.LearnedRoutes >= 0 &&
            (status.Memory is null ||
                IsValidSidecarMemory(status.Memory, observedAt));
    }

    private static bool IsValidSidecarMemory(
        RuntimeSidecarMemorySnapshot memory,
        DateTimeOffset observedAt)
    {
        if (memory.SlotCount < 0 ||
            memory.HistoryCount < 0 ||
            !IsOptionalCanonicalText(memory.LatestSlot, 128) ||
            !IsOptionalCanonicalText(memory.LatestLabel, 256) ||
            !IsOptionalCanonicalText(memory.LatestSource, 128) ||
            (memory.FetchError is not null &&
                !string.Equals(
                    memory.FetchError,
                    RuntimeDiagnosticCodes.SidecarMemoryFetchFailed,
                    StringComparison.Ordinal)) ||
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
            IsOptionalCanonicalText(slot.Label, 256) &&
            IsOptionalCanonicalText(slot.Note, 1_024) &&
            IsCanonicalText(slot.Source, 128) &&
            IsOptionalObservedTimestamp(slot.SavedAt, observedAt) &&
            slot.PatternCount >= 0 &&
            slot.LabelCount >= 0);
    }

    private static bool IsValidCapabilityPosture(
        PersistedRuntimeState runtime)
    {
        var failed = string.Equals(
            runtime.CapabilityFetchError,
            RuntimeDiagnosticCodes.CapabilityFetchFailed,
            StringComparison.Ordinal);
        return runtime.CapabilitySource switch
        {
            "manual" =>
                runtime.CapabilityFetchedAt is null &&
                (runtime.CapabilityFetchError is null || failed),
            "gewyvern-api" =>
                (runtime.CapabilityFetchedAt is not null &&
                    runtime.CapabilityFetchError is null) ||
                (runtime.CapabilityFetchedAt is null && failed),
            "fetch_failed" =>
                runtime.CapabilityFetchedAt is null && failed,
            _ => false,
        };
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
            ValidateOrchestraRunPayload(run, observedAt);
            if (run.RequestId is not null)
            {
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

    internal static void ValidateOrchestraStoreEnvelope(
        OrchestraRunSummary run,
        OrchestraRunEvent? eventRecord,
        DateTimeOffset? now = null)
    {
        var observedAt = now ?? DateTimeOffset.UtcNow;
        ValidateOrchestraRunPayload(run, observedAt);
        if (eventRecord is null)
        {
            return;
        }

        ValidateOrchestraEventPayload(eventRecord, observedAt);
        if (!string.Equals(
                eventRecord.RunId,
                run.RunId,
                StringComparison.Ordinal) ||
            !string.Equals(
                eventRecord.RuntimeId,
                run.RuntimeId,
                StringComparison.Ordinal) ||
            !string.Equals(
                eventRecord.ToOutcome,
                run.Outcome,
                StringComparison.Ordinal) ||
            eventRecord.RecordedAt < run.ExecutedAt ||
            (run.CompletedAt is not null &&
                eventRecord.RecordedAt < run.CompletedAt))
        {
            throw new InvalidDataException(
                "Orchestra persistence event does not match its run");
        }
    }

    internal static void ValidateOrchestraEventPayload(
        OrchestraRunEvent eventRecord,
        DateTimeOffset? now = null)
    {
        var observedAt = now ?? DateTimeOffset.UtcNow;
        if (eventRecord is null ||
            eventRecord.EventId < 0 ||
            !IsStableIdentity(eventRecord.RunId) ||
            !IsStableIdentity(eventRecord.RuntimeId) ||
            !IsStableIdentity(eventRecord.EventType) ||
            (eventRecord.FromOutcome is not null &&
                !IsKnownOrchestraOutcome(
                    eventRecord.FromOutcome)) ||
            !IsKnownOrchestraOutcome(eventRecord.ToOutcome) ||
            !IsBoundedCanonicalText(
                eventRecord.Summary,
                1_024,
                allowEmpty: true) ||
            eventRecord.RecordedAt == default ||
            eventRecord.RecordedAt >
                observedAt.AddMinutes(5))
        {
            throw new InvalidDataException(
                "Orchestra persistence event payload is invalid");
        }
    }

    internal static void ValidateOrchestraEventSequence(
        OrchestraRunSummary? run,
        IReadOnlyList<OrchestraRunEvent> events,
        string runtimeId,
        string runId,
        DateTimeOffset? now = null)
    {
        if (events is null)
        {
            throw new InvalidDataException(
                "Orchestra persistence event sequence is missing");
        }

        var observedAt = now ?? DateTimeOffset.UtcNow;
        OrchestraRunEvent? previous = null;
        foreach (var eventRecord in events)
        {
            ValidateOrchestraEventPayload(eventRecord, observedAt);
            if (eventRecord.EventId < 1 ||
                !string.Equals(
                    eventRecord.RuntimeId,
                    runtimeId,
                    StringComparison.Ordinal) ||
                !string.Equals(
                    eventRecord.RunId,
                    runId,
                    StringComparison.Ordinal) ||
                (previous is null &&
                    eventRecord.FromOutcome is not null) ||
                (previous is not null &&
                    (eventRecord.EventId <= previous.EventId ||
                        eventRecord.RecordedAt < previous.RecordedAt ||
                        !string.Equals(
                            eventRecord.FromOutcome,
                            previous.ToOutcome,
                            StringComparison.Ordinal) ||
                        !IsValidOrchestraTransition(
                            previous.ToOutcome,
                            eventRecord.ToOutcome))))
            {
                throw new InvalidDataException(
                    "Orchestra persistence event sequence is invalid");
            }
            previous = eventRecord;
        }

        if (run is null)
        {
            return;
        }

        ValidateOrchestraRunPayload(run, observedAt);
        if (!string.Equals(
                run.RuntimeId,
                runtimeId,
                StringComparison.Ordinal) ||
            !string.Equals(
                run.RunId,
                runId,
                StringComparison.Ordinal) ||
            previous is null ||
            events[0].RecordedAt < run.ExecutedAt ||
            !string.Equals(
                previous.ToOutcome,
                run.Outcome,
                StringComparison.Ordinal) ||
            (run.CompletedAt is not null &&
                previous.RecordedAt < run.CompletedAt))
        {
            throw new InvalidDataException(
                "Orchestra persistence event sequence does not match its run");
        }
    }

    internal static OrchestraRunEvent CreateLegacyOrchestraImportEvent(
        OrchestraRunSummary run) =>
        new(
            0,
            run.RunId,
            run.RuntimeId,
            "legacy_import",
            null,
            run.Outcome,
            "Imported from Leserpent 1.x persistence",
            run.CompletedAt ?? run.ExecutedAt);

    internal static bool IsValidOrchestraTransition(
        string current,
        string next)
    {
        if (string.Equals(
                current,
                next,
                StringComparison.Ordinal))
        {
            return false;
        }
        if (string.Equals(
                current,
                "queued",
                StringComparison.Ordinal))
        {
            return string.Equals(
                    next,
                    "running",
                    StringComparison.Ordinal) ||
                string.Equals(
                    next,
                    "cancelled",
                    StringComparison.Ordinal) ||
                string.Equals(
                    next,
                    "failed",
                    StringComparison.Ordinal);
        }
        return string.Equals(
                current,
                "running",
                StringComparison.Ordinal) &&
            IsTerminalOrchestraOutcome(next);
    }

    private static void ValidateOrchestraRunPayload(
        OrchestraRunSummary run,
        DateTimeOffset observedAt)
    {
        if (run is null ||
            !IsStableIdentity(run.RunId) ||
            !IsStableIdentity(run.RuntimeId) ||
            !IsStableIdentity(run.PlanId) ||
            !IsKnownOrchestraOutcome(run.Outcome) ||
            run.ExecutedAt == default ||
            run.ExecutedAt > observedAt.AddMinutes(5) ||
            (IsActiveOrchestraOutcome(run.Outcome) &&
                run.CompletedAt is not null) ||
            (run.CompletedAt is not null &&
                (run.CompletedAt < run.ExecutedAt ||
                    run.CompletedAt >
                        observedAt.AddMinutes(5))) ||
            run.Attempt is < 1 or > MaxOrchestraRunAttempts ||
            (run.RetriedFromRunId is null && run.Attempt != 1) ||
            (run.RetriedFromRunId is not null &&
                (run.Attempt < 2 ||
                    !IsStableIdentity(run.RetriedFromRunId) ||
                    string.Equals(
                        run.RunId,
                        run.RetriedFromRunId,
                        StringComparison.OrdinalIgnoreCase))) ||
            !IsOptionalCanonicalText(run.ApprovedBy, 256) ||
            !IsOptionalCanonicalText(run.ApprovalNote, 1_024) ||
            (run.PlanRevision is not null &&
                !IsStableIdentity(run.PlanRevision)) ||
            (run.RequestId is not null &&
                !IsStableIdentity(run.RequestId)))
        {
            throw new InvalidDataException(
                "Orchestra persistence run payload is invalid");
        }

        if (run.Steps is null ||
            run.Steps.Count > MaxOrchestraRunSteps ||
            run.Steps.Any(static step =>
                step is null ||
                !IsStableIdentity(step.Step) ||
                !IsStableIdentity(step.Outcome) ||
                !IsBoundedCanonicalText(
                    step.Summary,
                    1_024,
                    allowEmpty: true)))
        {
            throw new InvalidDataException(
                "Orchestra persistence step payload is invalid");
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
            var unregistrationCommandId =
                persisted.UnregistrationCommandId?.Trim() ?? string.Empty;
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
                !IsValidDeletionIdentifier(unregistrationCommandId) ||
                !string.Equals(
                    unregistrationCommandId,
                    RuntimeDeletionCommandIdentity.ForIntent(intentId),
                    StringComparison.Ordinal) ||
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
                persisted.Revision,
                unregistrationCommandId,
                persisted.UnregistrationReplayHorizonFloor,
                persisted.UnregistrationMutationMayHaveStarted));
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

    internal static IReadOnlyList<
        PersistedRuntimeDeletionReconciliationAudit>
        NormalizeRuntimeDeletionReconciliationAudit(
            PersistedControlPlaneState? state,
            DateTimeOffset? now = null)
    {
        var persistedAudit =
            state?.RuntimeDeletionReconciliationAudit ??
                Array.Empty<
                    PersistedRuntimeDeletionReconciliationAudit>();
        if (persistedAudit.Count >
            MaxRuntimeDeletionReconciliationAuditEntries)
        {
            throw new InvalidDataException(
                $"control-plane state contains more than {MaxRuntimeDeletionReconciliationAuditEntries} runtime deletion reconciliation audit entries");
        }

        var observedAt = now ?? DateTimeOffset.UtcNow;
        var requestIds = new HashSet<string>(StringComparer.Ordinal);
        var normalized = new List<
            PersistedRuntimeDeletionReconciliationAudit>(
                persistedAudit.Count);
        foreach (var persisted in persistedAudit)
        {
            var requestId =
                persisted.RequestId?.Trim() ?? string.Empty;
            var intentId =
                persisted.IntentId?.Trim() ?? string.Empty;
            var requestedBy =
                persisted.RequestedBy?.Trim() ?? string.Empty;
            var cleanupCommandId =
                persisted.OrchestraCleanupCommandId?.Trim();
            var hasCleanupReceipt =
                cleanupCommandId is not null ||
                persisted.OrchestraCleanupGeneration is not null;
            var runtimeIds =
                (persisted.RuntimeIds ?? Array.Empty<string>())
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
                persisted.ExpectedRevision >
                    MaxRuntimeDeletionRevision ||
                persisted.DaemonRevision == 0 ||
                (hasCleanupReceipt &&
                    (!IsValidDeletionIdentifier(
                        cleanupCommandId ?? string.Empty) ||
                     persisted.OrchestraCleanupGeneration is null or 0)) ||
                persisted.ReconciledAt == default ||
                persisted.ReconciledAt >
                    observedAt.AddMinutes(5))
            {
                throw new InvalidDataException(
                    "control-plane state contains an invalid runtime deletion reconciliation audit entry");
            }

            normalized.Add(
                new PersistedRuntimeDeletionReconciliationAudit(
                    requestId,
                    intentId,
                    Array.AsReadOnly(runtimeIds),
                    persisted.ExpectedRevision,
                    persisted.DaemonRevision,
                    requestedBy,
                    persisted.ReconciledAt,
                    cleanupCommandId,
                    persisted.OrchestraCleanupGeneration));
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

    private static bool IsBoundedCanonicalText(
        string? value,
        int maxLength,
        bool allowEmpty) =>
        value is not null &&
        value.Length <= maxLength &&
        (allowEmpty || value.Length > 0) &&
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
        if ((!intent.UnregistrationMutationMayHaveStarted &&
                intent.UnregistrationReplayHorizonFloor is not null) ||
            intent.UnregistrationReplayHorizonFloor == 0)
        {
            return false;
        }
        if (intent.AttemptCount == 0)
        {
            return intent.LastAttemptAt is null &&
                intent.NextAttemptAt is null &&
                intent.LastFailureCode is null &&
                intent.Revision ==
                    (intent.UnregistrationReplayHorizonFloor is null
                        ? 1
                        : 2);
        }
        var minimumRevision =
            (long)intent.AttemptCount + 1 +
            (intent.UnregistrationReplayHorizonFloor is null
                ? 0
                : 1);
        if (intent.AttemptCount is < 0 or
                > MaxRuntimeDeletionAttempts ||
            intent.Revision < minimumRevision ||
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
