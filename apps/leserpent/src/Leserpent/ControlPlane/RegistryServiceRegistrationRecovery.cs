using System.Collections.Concurrent;

namespace Leserpent.ControlPlane;

internal enum RuntimeRegistrationIntentResolutionKind
{
    None,
    Exact,
    Conflict,
}

internal sealed record RuntimeRegistrationIntentResolution(
    RuntimeRegistrationIntentResolutionKind Kind,
    PersistedRuntimeRegistrationIntent? Intent);

internal sealed class RuntimeRegistrationIntentConflictException(
    PersistedRuntimeRegistrationIntent existing) :
    InvalidOperationException(
        "another runtime registration intent already owns this target")
{
    internal PersistedRuntimeRegistrationIntent Existing { get; } = existing;
}

public sealed partial class RegistryService
{
    private readonly ConcurrentDictionary<
        string,
        PersistedRuntimeRegistrationIntent>
        pendingRuntimeRegistrations = new(StringComparer.Ordinal);

    internal RuntimeRegistrationPlan? GetRuntimeRegistrationRecoveryPlan(
        RuntimeRegistrationPlanRequest request)
    {
        lock (runtimeRegistrationSync)
        {
            var exact = pendingRuntimeRegistrations.Values.FirstOrDefault(
                intent => RuntimeRegistrationIntentPolicy.CoordinatesMatch(
                    intent,
                    request));
            if (exact is not null)
            {
                return RuntimeRegistrationPolicy.BuildRecovery(
                    request,
                    exact,
                    allowed: true);
            }
            var overlapping = pendingRuntimeRegistrations.Values.FirstOrDefault(
                intent => RuntimeRegistrationIntentPolicy.Overlaps(
                    intent,
                    request));
            return overlapping is null
                ? null
                : RuntimeRegistrationPolicy.BuildRecovery(
                    request,
                    overlapping,
                    allowed: false);
        }
    }

    internal RuntimeRegistrationIntentResolution
        ResolveRuntimeRegistrationIntent(
            RuntimeRegistrationRequest request)
    {
        lock (runtimeRegistrationSync)
        {
            var exact = pendingRuntimeRegistrations.Values.FirstOrDefault(
                intent => RuntimeRegistrationIntentPolicy.MatchesCommand(
                    intent,
                    request));
            if (exact is not null)
            {
                return new RuntimeRegistrationIntentResolution(
                    RuntimeRegistrationIntentResolutionKind.Exact,
                    CloneRuntimeRegistrationIntent(exact));
            }
            var planRequest = new RuntimeRegistrationPlanRequest(
                request.Name,
                request.Endpoint,
                request.SidecarEndpoint);
            var overlapping = pendingRuntimeRegistrations.Values.FirstOrDefault(
                intent => RuntimeRegistrationIntentPolicy.Overlaps(
                    intent,
                    planRequest));
            return overlapping is null
                ? new RuntimeRegistrationIntentResolution(
                    RuntimeRegistrationIntentResolutionKind.None,
                    null)
                : new RuntimeRegistrationIntentResolution(
                    RuntimeRegistrationIntentResolutionKind.Conflict,
                    CloneRuntimeRegistrationIntent(overlapping));
        }
    }

    internal PersistedRuntimeRegistrationIntent
        PrepareRuntimeRegistrationIntent(
            PersistedRuntimeRegistrationIntent proposed)
    {
        RequireControlPlaneWriter();
        var normalized =
            ControlPlaneStateValidator.NormalizeRuntimeRegistrationIntent(
                proposed,
                timeProvider.GetUtcNow());
        lock (runtimeRegistrationSync)
        {
            if (pendingRuntimeRegistrations.TryGetValue(
                    normalized.CommandId,
                    out var replay))
            {
                return CloneRuntimeRegistrationIntent(replay);
            }
            var planRequest = new RuntimeRegistrationPlanRequest(
                normalized.Name,
                normalized.Endpoint,
                normalized.SidecarEndpoint);
            var conflict = pendingRuntimeRegistrations.Values.FirstOrDefault(
                intent =>
                    string.Equals(
                        intent.RuntimeId,
                        normalized.RuntimeId,
                        StringComparison.OrdinalIgnoreCase) ||
                    RuntimeRegistrationIntentPolicy.Overlaps(
                        intent,
                        planRequest));
            if (conflict is not null ||
                pendingRuntimeRegistrations.Count >=
                    ControlPlaneStateValidator
                        .MaxPendingRuntimeRegistrationIntents)
            {
                throw new RuntimeRegistrationIntentConflictException(
                    conflict ?? normalized);
            }

            pendingRuntimeRegistrations[normalized.CommandId] = normalized;
            try
            {
                PersistStateStrict();
            }
            catch
            {
                pendingRuntimeRegistrations.TryRemove(
                    normalized.CommandId,
                    out _);
                throw;
            }
            return CloneRuntimeRegistrationIntent(normalized);
        }
    }

    internal PersistedRuntimeRegistrationIntent
        BeginRuntimeRegistrationAttempt(string commandId)
    {
        RequireControlPlaneWriter();
        lock (runtimeRegistrationSync)
        {
            var current = RequireRuntimeRegistrationIntent(commandId);
            if (current.AttemptCount >=
                ControlPlaneStateValidator.MaxRuntimeRegistrationAttempts)
            {
                throw new InvalidOperationException(
                    "runtime registration intent exhausted its attempt budget");
            }
            var updated = current with
            {
                AttemptCount = current.AttemptCount + 1,
                LastAttemptAt = timeProvider.GetUtcNow(),
                LastFailureCode = null,
            };
            return ReplaceRuntimeRegistrationIntent(current, updated);
        }
    }

    internal void RecordRuntimeRegistrationFailure(
        string commandId,
        string failureCode)
    {
        RequireControlPlaneWriter();
        lock (runtimeRegistrationSync)
        {
            var current = RequireRuntimeRegistrationIntent(commandId);
            var updated = current with
            {
                LastFailureCode = failureCode,
            };
            _ = ReplaceRuntimeRegistrationIntent(current, updated);
        }
    }

    internal void CompleteRuntimeRegistrationIntent(string commandId)
    {
        RequireControlPlaneWriter();
        lock (runtimeRegistrationSync)
        {
            if (!pendingRuntimeRegistrations.TryRemove(
                    commandId,
                    out var removed))
            {
                return;
            }
            try
            {
                PersistStateStrict();
            }
            catch
            {
                pendingRuntimeRegistrations[commandId] = removed;
                throw;
            }
        }
    }

    internal IReadOnlyList<PersistedRuntimeRegistrationIntent>
        ListPendingRuntimeRegistrations() =>
        pendingRuntimeRegistrations.Values
            .OrderBy(static intent => intent.PreparedAt)
            .ThenBy(static intent => intent.CommandId, StringComparer.Ordinal)
            .Select(CloneRuntimeRegistrationIntent)
            .ToArray();

    private void RestorePendingRuntimeRegistrations(
        PersistedControlPlaneState? state)
    {
        foreach (var intent in
            ControlPlaneStateValidator.NormalizePendingRuntimeRegistrations(
                state))
        {
            if (!pendingRuntimeRegistrations.TryAdd(
                    intent.CommandId,
                    intent))
            {
                throw new InvalidDataException(
                    "control-plane state contains a duplicate runtime registration intent");
            }
        }
    }

    private PersistedRuntimeRegistrationIntent
        RequireRuntimeRegistrationIntent(string commandId) =>
        pendingRuntimeRegistrations.TryGetValue(commandId, out var intent)
            ? intent
            : throw new InvalidOperationException(
                "runtime registration intent no longer exists");

    private PersistedRuntimeRegistrationIntent ReplaceRuntimeRegistrationIntent(
        PersistedRuntimeRegistrationIntent previous,
        PersistedRuntimeRegistrationIntent updated)
    {
        var normalized =
            ControlPlaneStateValidator.NormalizeRuntimeRegistrationIntent(
                updated,
                timeProvider.GetUtcNow());
        pendingRuntimeRegistrations[previous.CommandId] = normalized;
        try
        {
            PersistStateStrict();
        }
        catch
        {
            pendingRuntimeRegistrations[previous.CommandId] = previous;
            throw;
        }
        return CloneRuntimeRegistrationIntent(normalized);
    }

    private static PersistedRuntimeRegistrationIntent
        CloneRuntimeRegistrationIntent(
            PersistedRuntimeRegistrationIntent intent) =>
        ControlPlaneStateValidator.NormalizeRuntimeRegistrationIntent(intent);
}
