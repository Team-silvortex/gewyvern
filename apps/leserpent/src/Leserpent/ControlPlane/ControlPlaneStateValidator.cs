namespace Leserpent.ControlPlane;

internal static class ControlPlaneStateValidator
{
    internal const int MaxPendingRuntimeDeletionIntents = 256;
    internal const int MaxRuntimeDeletionAttempts = 1_000_000;
    internal const int MaxRuntimeDeletionRetryAuditEntries = 256;
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

        ValidateProjectionGraph(state);
        _ = NormalizePendingRuntimeDeletions(state, observedAt);
        _ = NormalizeRuntimeDeletionRetryAudit(state, observedAt);
    }

    internal static void ValidateProjectionGraph(
        PersistedControlPlaneState state)
    {
        ValidateRuntimeSessionGraph(state);
        ValidateLegacyOrchestraRunGraph(state);
    }

    internal static void ValidateRuntimeSessionGraph(
        PersistedControlPlaneState state)
    {
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
        }
    }

    internal static void ValidateLegacyOrchestraRunGraph(
        PersistedControlPlaneState state)
    {
        var canonicalRuntimeIds = state.Runtimes
            .Select(static runtime => runtime.RuntimeId)
            .ToHashSet(StringComparer.Ordinal);
        var runIds = new HashSet<string>(
            StringComparer.OrdinalIgnoreCase);
        foreach (var run in state.OrchestraRuns ??
            Array.Empty<OrchestraRunSummary>())
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
