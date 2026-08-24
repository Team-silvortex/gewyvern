using System.Security.Cryptography;
using System.Text;

namespace Leserpent.ControlPlane;

public static class RuntimeCleanupPolicy
{
    public const string FailedKind = "failed";
    public const string UnobservedKind = "unobserved";
    public const string SliceKind = "slice";

    public static RuntimeCleanupPlan Build(
        RuntimeListFilter filter,
        IReadOnlyList<RuntimeSummary> runtimes,
        IReadOnlyList<SessionSummary> sessions)
    {
        var failed = runtimes.Where(IsFailed).ToArray();
        var unobserved = runtimes.Where(IsDeletableUnobserved).ToArray();
        return new RuntimeCleanupPlan(
            filter,
            IsProtected(filter) ? "protected" : "normal",
            BuildAction(FailedKind, filter, failed, sessions),
            BuildAction(UnobservedKind, filter, unobserved, sessions),
            BuildAction(SliceKind, filter, runtimes, sessions, $"CLEAR {runtimes.Count}"));
    }

    public static bool IsFailed(RuntimeSummary runtime) =>
        string.Equals(runtime.Status.StatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase);

    public static bool IsDeletableUnobserved(RuntimeSummary runtime) =>
        string.Equals(runtime.Status.StatusSource, "unobserved", StringComparison.OrdinalIgnoreCase) &&
        !(string.Equals(runtime.Status.ResilienceStatus, "idle_ready", StringComparison.OrdinalIgnoreCase) &&
          runtime.Status.ResilienceDegraded is false);

    public static RuntimeCleanupActionPlan RequireMatchingAction(
        RuntimeCleanupPlan plan,
        string kind,
        RuntimeCleanupRequest request)
    {
        var action = kind switch
        {
            FailedKind => plan.Failed,
            UnobservedKind => plan.Unobserved,
            SliceKind => plan.Slice,
            _ => throw new ArgumentOutOfRangeException(nameof(kind), kind, "unknown runtime cleanup kind"),
        };
        if (string.IsNullOrWhiteSpace(request.PlanToken) ||
            !string.Equals(request.PlanToken, action.PlanToken, StringComparison.Ordinal))
        {
            throw new RuntimeCleanupPlanMismatchException(
                "runtime cleanup plan changed; review the current targets before retrying");
        }
        if (action.Challenge is not null &&
            !string.Equals(request.Challenge?.Trim(), action.Challenge, StringComparison.Ordinal))
        {
            throw new RuntimeCleanupPlanMismatchException(
                "runtime cleanup challenge does not match the current plan");
        }
        return action;
    }

    internal static IReadOnlyList<string> GetAffectedSessionIds(
        IReadOnlyCollection<string> runtimeIds,
        IReadOnlyList<SessionSummary> sessions)
    {
        var targetIds = runtimeIds.ToHashSet(StringComparer.OrdinalIgnoreCase);
        return sessions
            .Where(session => targetIds.Contains(session.RuntimeId))
            .Select(session => session.SessionId)
            .OrderBy(static sessionId => sessionId, StringComparer.Ordinal)
            .ToArray();
    }

    private static RuntimeCleanupActionPlan BuildAction(
        string kind,
        RuntimeListFilter filter,
        IReadOnlyList<RuntimeSummary> runtimes,
        IReadOnlyList<SessionSummary> sessions,
        string? challenge = null)
    {
        var targets = runtimes
            .OrderBy(runtime => runtime.RuntimeId, StringComparer.OrdinalIgnoreCase)
            .Select(runtime => new RuntimeCleanupTarget(runtime.RuntimeId, runtime.Name))
            .ToArray();
        var affectedSessionIds = GetAffectedSessionIds(
            targets.Select(target => target.RuntimeId).ToArray(),
            sessions);
        string[] canonicalParts =
        [
            "runtime-cleanup-plan-v2",
            kind,
            filter.Environment?.Trim() ?? string.Empty,
            filter.Cluster?.Trim() ?? string.Empty,
            filter.Role?.Trim() ?? string.Empty,
            .. targets.Select(target => $"runtime:{target.RuntimeId.ToLowerInvariant()}"),
            .. affectedSessionIds.Select(sessionId => $"session:{sessionId.ToLowerInvariant()}"),
        ];
        var canonical = string.Join('\n', canonicalParts);
        var token = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(canonical))).ToLowerInvariant();
        return new RuntimeCleanupActionPlan(
            kind,
            targets.Length,
            affectedSessionIds.Count,
            targets,
            token,
            challenge);
    }

    private static bool IsProtected(RuntimeListFilter filter) =>
        new[] { filter.Environment, filter.Cluster, filter.Role }
            .Where(value => !string.IsNullOrWhiteSpace(value))
            .Any(value => value!.Contains("prod", StringComparison.OrdinalIgnoreCase) ||
                          value.Contains("live", StringComparison.OrdinalIgnoreCase));
}

public sealed class RuntimeCleanupPlanMismatchException(string reason) : InvalidOperationException(reason);
