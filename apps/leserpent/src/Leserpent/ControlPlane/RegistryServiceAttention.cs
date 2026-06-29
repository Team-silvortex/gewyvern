using System.Collections.Immutable;

namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    private static IReadOnlyList<string> GetAttentionReasons(
        RuntimeStatusSnapshot status,
        RuntimeSidecarStatusSnapshot? sidecarStatus)
    {
        var reasons = new List<string>();
        var idleReady = IsIdleReadyStatus(status);
        if (string.Equals(status.StatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase))
        {
            reasons.Add("status_fetch_failed");
        }

        if (string.Equals(sidecarStatus?.StatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase))
        {
            reasons.Add("sidecar_status_fetch_failed");
        }

        if (!status.HasLatestSnapshot && !idleReady)
        {
            reasons.Add("no_latest_snapshot");
        }

        if (!status.HasAnalysisJson && !idleReady)
        {
            reasons.Add("no_analysis_json");
        }

        return reasons;
    }

    private static bool IsIdleReadyStatus(RuntimeStatusSnapshot status) =>
        string.Equals(status.ResilienceStatus, "idle_ready", StringComparison.OrdinalIgnoreCase)
        || string.Equals(status.SocketServiceStatus, "idle", StringComparison.OrdinalIgnoreCase);

    private static IReadOnlyList<RuntimeSuggestedAction> GetSuggestedActions(
        IReadOnlyList<string> reasons,
        IReadOnlyList<RuntimeRecoveryActivity> recentActivities)
    {
        var actions = new List<RuntimeSuggestedAction>();

        if (reasons.Contains("status_fetch_failed", StringComparer.OrdinalIgnoreCase))
        {
            actions.Add(new RuntimeSuggestedAction("refresh_status", 1, "retry runtime status first because the primary snapshot fetch failed"));
            actions.Add(new RuntimeSuggestedAction("refresh_all", 3, "if status still looks stale, rerun the full runtime refresh path"));
        }

        if (reasons.Contains("sidecar_status_fetch_failed", StringComparer.OrdinalIgnoreCase))
        {
            actions.Add(new RuntimeSuggestedAction("refresh_sidecar", 2, "retry the sidecar separately so diagnostics can recover without disturbing runtime intake"));
        }

        if (reasons.Contains("no_latest_snapshot", StringComparer.OrdinalIgnoreCase))
        {
            actions.Add(new RuntimeSuggestedAction("refresh_status", 1, "request a fresh runtime snapshot before trusting downstream analysis"));
        }

        if (reasons.Contains("no_analysis_json", StringComparer.OrdinalIgnoreCase))
        {
            actions.Add(new RuntimeSuggestedAction("refresh_all", 2, "refresh the full runtime surface so summary and analysis artifacts can repopulate together"));
        }

        return actions
            .GroupBy(action => action.Action, StringComparer.OrdinalIgnoreCase)
            .Select(group => group.OrderBy(action => action.Priority).First())
            .Select(action => AdaptSuggestedAction(action, recentActivities))
            .OrderBy(action => action.Priority)
            .ToArray();
    }

    private static RuntimeSuggestedAction AdaptSuggestedAction(
        RuntimeSuggestedAction action,
        IReadOnlyList<RuntimeRecoveryActivity> recentActivities)
    {
        var latestMatch = recentActivities.FirstOrDefault(activity =>
            string.Equals(activity.Action, action.Action, StringComparison.OrdinalIgnoreCase));
        if (latestMatch is null)
        {
            return action;
        }

        if (string.Equals(latestMatch.Outcome, "ok", StringComparison.OrdinalIgnoreCase))
        {
            return action with
            {
                Priority = Math.Max(1, action.Priority - 1),
                Hint = $"{action.Hint}; this worked recently at {latestMatch.RecordedAt:O}"
            };
        }

        if (IsFailedOutcome(latestMatch.Outcome))
        {
            var cooldownWindow = CooldownForOutcome(latestMatch.Outcome);
            var cooldownRemaining = latestMatch.RecordedAt.Add(cooldownWindow) - DateTimeOffset.UtcNow;
            return action with
            {
                Priority = action.Priority + FailurePriorityPenalty(latestMatch.Outcome),
                Hint = $"{action.Hint}; this was recently attempted with outcome {latestMatch.Outcome}",
                CoolingDown = cooldownRemaining > TimeSpan.Zero,
                CooldownSecondsRemaining = cooldownRemaining > TimeSpan.Zero
                    ? Math.Max(1, (int)Math.Ceiling(cooldownRemaining.TotalSeconds))
                    : 0
            };
        }

        return action;
    }

    private static bool IsFailedOutcome(string outcome) =>
        string.Equals(outcome, "degraded", StringComparison.OrdinalIgnoreCase) ||
        string.Equals(outcome, "auth_failed", StringComparison.OrdinalIgnoreCase) ||
        string.Equals(outcome, "network_failed", StringComparison.OrdinalIgnoreCase) ||
        string.Equals(outcome, "incomplete_data", StringComparison.OrdinalIgnoreCase);

    private static int FailurePriorityPenalty(string outcome) =>
        string.Equals(outcome, "auth_failed", StringComparison.OrdinalIgnoreCase) ? 4 :
        string.Equals(outcome, "network_failed", StringComparison.OrdinalIgnoreCase) ? 3 :
        string.Equals(outcome, "incomplete_data", StringComparison.OrdinalIgnoreCase) ? 1 :
        2;

    private static TimeSpan CooldownForOutcome(string outcome) =>
        string.Equals(outcome, "auth_failed", StringComparison.OrdinalIgnoreCase) ? AuthFailedRecoveryCooldown :
        string.Equals(outcome, "network_failed", StringComparison.OrdinalIgnoreCase) ? NetworkFailedRecoveryCooldown :
        string.Equals(outcome, "incomplete_data", StringComparison.OrdinalIgnoreCase) ? IncompleteDataRecoveryCooldown :
        GenericFailedRecoveryCooldown;

    private static string GetAttentionSeverity(IReadOnlyList<string> reasons) =>
        reasons.Contains("status_fetch_failed", StringComparer.OrdinalIgnoreCase)
            ? "critical"
            : reasons.Contains("sidecar_status_fetch_failed", StringComparer.OrdinalIgnoreCase)
                ? "warning"
            : "warning";

    private static int AttentionSeverityRank(string severity) =>
        string.Equals(severity, "critical", StringComparison.OrdinalIgnoreCase) ? 1 : 0;

    private IReadOnlyList<RuntimeRecoveryActivity> GetRecentRecoveryActivities(string runtimeId)
    {
        if (!recoveryActivities.TryGetValue(runtimeId, out var queue) || queue.IsEmpty)
        {
            return Array.Empty<RuntimeRecoveryActivity>();
        }

        return queue
            .Reverse()
            .ToArray();
    }

    private static ImmutableQueue<RuntimeRecoveryActivity> TrimRecoveryQueue(ImmutableQueue<RuntimeRecoveryActivity> queue)
    {
        while (queue.Count() > MaxRecoveryActivitiesPerRuntime)
        {
            queue = queue.Dequeue();
        }

        return queue;
    }
}
