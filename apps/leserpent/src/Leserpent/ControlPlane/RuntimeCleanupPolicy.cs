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
        var targetIds = targets.Select(target => target.RuntimeId).ToHashSet(StringComparer.OrdinalIgnoreCase);
        var sessionCount = sessions.Count(session => targetIds.Contains(session.RuntimeId));
        string[] canonicalParts =
        [
            kind,
            filter.Environment?.Trim() ?? string.Empty,
            filter.Cluster?.Trim() ?? string.Empty,
            filter.Role?.Trim() ?? string.Empty,
            .. targets.Select(target => target.RuntimeId.ToLowerInvariant()),
        ];
        var canonical = string.Join('\n', canonicalParts);
        var token = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(canonical))).ToLowerInvariant();
        return new RuntimeCleanupActionPlan(kind, targets.Length, sessionCount, targets, token, challenge);
    }

    private static bool IsProtected(RuntimeListFilter filter) =>
        new[] { filter.Environment, filter.Cluster, filter.Role }
            .Where(value => !string.IsNullOrWhiteSpace(value))
            .Any(value => value!.Contains("prod", StringComparison.OrdinalIgnoreCase) ||
                          value.Contains("live", StringComparison.OrdinalIgnoreCase));
}

public sealed class RuntimeCleanupPlanMismatchException(string reason) : InvalidOperationException(reason);
