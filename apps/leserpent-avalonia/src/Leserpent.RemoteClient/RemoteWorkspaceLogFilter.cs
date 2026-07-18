public sealed record RemoteWorkspaceLogView(
    RemoteWorkspaceSnapshot Snapshot,
    int VisibleLogCount,
    int TotalLogCount,
    string Query,
    string Level)
{
    public bool IsActive => Query.Length > 0 || Level != RemoteWorkspaceLogFilter.AllLevels;
}

public static class RemoteWorkspaceLogFilter
{
    public const int MaxQueryLength = 128;
    public const string AllLevels = "all";

    public static readonly IReadOnlyList<string> Levels =
        [AllLevels, "trace", "debug", "info", "warning", "error"];

    public static RemoteWorkspaceLogView Apply(
        RemoteWorkspaceSnapshot snapshot,
        string? query,
        string? level)
    {
        var normalizedQuery = NormalizeQuery(query);
        var normalizedLevel = string.IsNullOrWhiteSpace(level)
            ? AllLevels
            : level.Trim().ToLowerInvariant();
        if (!Levels.Contains(normalizedLevel, StringComparer.Ordinal))
        {
            throw new ArgumentException("log level filter is invalid", nameof(level));
        }

        var logs = snapshot.Logs.Where(entry =>
            (normalizedLevel == AllLevels
                || string.Equals(entry.Level, normalizedLevel, StringComparison.Ordinal))
            && (normalizedQuery.Length == 0
                || entry.Display.Contains(
                    normalizedQuery,
                    StringComparison.OrdinalIgnoreCase)))
            .ToArray();
        var filtered = logs.Length == snapshot.Logs.Count
            ? snapshot
            : snapshot with { Logs = logs };
        return new RemoteWorkspaceLogView(
            filtered,
            logs.Length,
            snapshot.Logs.Count,
            normalizedQuery,
            normalizedLevel);
    }

    public static void VerifyContract()
    {
        var snapshot = new RemoteWorkspaceSnapshot(
            11,
            new RemoteRuntimeProjection
            {
                Id = "runtime-a",
                Name = "Payments",
                Revision = 11,
                Tags = new RuntimeTags(),
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            },
            [],
            [
                new RemoteLogProjection(1, "info", "listener ready"),
                new RemoteLogProjection(2, "warning", "Retry budget low"),
                new RemoteLogProjection(3, "error", "retry budget exhausted"),
            ]);

        Require(Apply(snapshot, "READY", AllLevels), 1, "case-insensitive query");
        Require(Apply(snapshot, "retry", "warning"), 1, "combined filter");
        Require(Apply(snapshot, "retry\0 budget", "error"), 1, "sanitized query");
        var empty = Apply(snapshot, "missing", AllLevels);
        if (!empty.IsActive
            || empty.VisibleLogCount != 0
            || empty.TotalLogCount != 3
            || snapshot.Logs.Count != 3)
        {
            throw new InvalidDataException("log filter empty-state contract drifted");
        }
        var bounded = Apply(snapshot, new string('x', MaxQueryLength + 10), AllLevels);
        if (bounded.Query.Length != MaxQueryLength)
        {
            throw new InvalidDataException("log filter query bound drifted");
        }
        try
        {
            Apply(snapshot, null, "critical");
        }
        catch (ArgumentException)
        {
            return;
        }
        throw new InvalidDataException("log filter accepted an unknown level");
    }

    private static void Require(
        RemoteWorkspaceLogView view,
        int expectedCount,
        string caseName)
    {
        if (view.VisibleLogCount != expectedCount || view.TotalLogCount != 3)
        {
            throw new InvalidDataException($"log filter failed: {caseName}");
        }
    }

    private static string NormalizeQuery(string? query) => new string((query ?? string.Empty)
        .Where(character => !char.IsControl(character))
        .Take(MaxQueryLength)
        .ToArray()).Trim();
}
