namespace Leserpent.ControlPlane;

internal static class RuntimeRefreshOutcomePolicy
{
    internal static string Determine(
        string? runtimeStatusSource,
        string? runtimeStatusError,
        string? sidecarStatusSource,
        string? sidecarStatusError)
    {
        if (string.Equals(
                runtimeStatusSource,
                "fetch_failed",
                StringComparison.OrdinalIgnoreCase)
            || string.Equals(
                sidecarStatusSource,
                "fetch_failed",
                StringComparison.OrdinalIgnoreCase))
        {
            var combined = string.Join(
                " ",
                new[] { runtimeStatusError, sidecarStatusError }
                    .Where(static value => !string.IsNullOrWhiteSpace(value)));
            if (LooksLikeAuthFailure(combined))
            {
                return "auth_failed";
            }
            if (LooksLikeNetworkFailure(combined))
            {
                return "network_failed";
            }
            if (LooksLikeIncompleteData(combined))
            {
                return "incomplete_data";
            }
            return "degraded";
        }

        return "ok";
    }

    private static bool LooksLikeAuthFailure(string message) =>
        message.Contains("401", StringComparison.OrdinalIgnoreCase)
        || message.Contains("403", StringComparison.OrdinalIgnoreCase)
        || message.Contains("unauthorized", StringComparison.OrdinalIgnoreCase)
        || message.Contains("forbidden", StringComparison.OrdinalIgnoreCase)
        || message.Contains("token", StringComparison.OrdinalIgnoreCase);

    private static bool LooksLikeNetworkFailure(string message) =>
        message.Contains("connection", StringComparison.OrdinalIgnoreCase)
        || message.Contains("refused", StringComparison.OrdinalIgnoreCase)
        || message.Contains("timed out", StringComparison.OrdinalIgnoreCase)
        || message.Contains("timeout", StringComparison.OrdinalIgnoreCase)
        || message.Contains("dns", StringComparison.OrdinalIgnoreCase)
        || message.Contains("socket", StringComparison.OrdinalIgnoreCase)
        || message.Contains("host", StringComparison.OrdinalIgnoreCase);

    private static bool LooksLikeIncompleteData(string message) =>
        message.Contains("decode", StringComparison.OrdinalIgnoreCase)
        || message.Contains("payload", StringComparison.OrdinalIgnoreCase)
        || message.Contains("json", StringComparison.OrdinalIgnoreCase)
        || message.Contains("snapshot", StringComparison.OrdinalIgnoreCase);
}
