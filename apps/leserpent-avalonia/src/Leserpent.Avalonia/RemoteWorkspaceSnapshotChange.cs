internal sealed record RemoteWorkspaceSnapshotChange(
    bool IsInitial,
    ulong RevisionAdvance,
    int AddedLogs,
    int NewErrors,
    int NewWarnings,
    int ExpiredLogs,
    int ChangedLogs,
    int AddedCommands,
    int UpdatedCommands,
    bool LogSequenceReset)
{
    public string Describe()
    {
        if (IsInitial)
        {
            return "initial snapshot";
        }
        var parts = new List<string>();
        if (RevisionAdvance > 0)
        {
            parts.Add($"revision +{RevisionAdvance}");
        }
        if (LogSequenceReset)
        {
            parts.Add("log sequence reset");
        }
        if (AddedLogs > 0)
        {
            parts.Add($"+{AddedLogs} logs");
        }
        if (NewErrors > 0)
        {
            parts.Add($"{NewErrors} new error");
        }
        if (NewWarnings > 0)
        {
            parts.Add($"{NewWarnings} new warning");
        }
        if (ExpiredLogs > 0)
        {
            parts.Add($"{ExpiredLogs} logs expired");
        }
        if (ChangedLogs > 0)
        {
            parts.Add($"{ChangedLogs} logs changed");
        }
        if (AddedCommands > 0)
        {
            parts.Add($"+{AddedCommands} commands");
        }
        if (UpdatedCommands > 0)
        {
            parts.Add($"{UpdatedCommands} commands updated");
        }
        return parts.Count == 0 ? "no changes" : string.Join(" / ", parts);
    }
}

internal static class RemoteWorkspaceSnapshotChanges
{
    private const int MaxHistoryEntries = 32;
    private const int MaxLogEntries = 256;

    public static RemoteWorkspaceSnapshotChange Compare(
        RemoteWorkspaceSnapshot? previous,
        RemoteWorkspaceSnapshot current)
    {
        var currentLogs = LogIndex(current.Logs);
        var currentCommands = CommandIndex(current.History);
        if (previous is null)
        {
            return new RemoteWorkspaceSnapshotChange(
                true, 0, 0, 0, 0, 0, 0, 0, 0, false);
        }
        var priorLogs = LogIndex(previous.Logs);
        var priorCommands = CommandIndex(previous.History);
        if (!string.Equals(
                previous.Runtime.Id,
                current.Runtime.Id,
                StringComparison.Ordinal))
        {
            throw new InvalidDataException("workspace snapshot runtime identity changed");
        }
        if (current.Revision < previous.Revision)
        {
            throw new InvalidDataException("workspace snapshot revision regressed");
        }
        var addedLogs = currentLogs.Keys.Count(sequence => !priorLogs.ContainsKey(sequence));
        var newErrors = CountNewLevel(priorLogs, currentLogs, "error");
        var newWarnings = CountNewLevel(priorLogs, currentLogs, "warning");
        var expiredLogs = priorLogs.Keys.Count(sequence => !currentLogs.ContainsKey(sequence));
        var changedLogs = currentLogs.Count(entry =>
            priorLogs.TryGetValue(entry.Key, out var prior)
            && (prior.Level != entry.Value.Level || prior.Display != entry.Value.Display));
        var addedCommands = currentCommands.Keys.Count(id => !priorCommands.ContainsKey(id));
        var updatedCommands = currentCommands.Count(entry =>
            priorCommands.TryGetValue(entry.Key, out var prior)
            && (prior.Revision != entry.Value.Revision
                || prior.Status != entry.Value.Status));
        var sequenceReset = previous.Logs.Count > 0
            && current.Logs.Count > 0
            && current.Logs[^1].Sequence < previous.Logs[^1].Sequence;
        return new RemoteWorkspaceSnapshotChange(
            false,
            current.Revision - previous.Revision,
            addedLogs,
            newErrors,
            newWarnings,
            expiredLogs,
            changedLogs,
            addedCommands,
            updatedCommands,
            sequenceReset);
    }

    public static void VerifyContract()
    {
        var initial = Snapshot(
            7,
            [new RemoteHistoryProjection("command-a", 7, "planned")],
            [
                new RemoteLogProjection(1, "info", "one"),
                new RemoteLogProjection(2, "info", "two"),
            ]);
        if (Compare(null, initial).Describe() != "initial snapshot"
            || Compare(null, initial).NewErrors != 0
            || Compare(null, initial).NewWarnings != 0
            || Compare(initial, initial).Describe() != "no changes"
            || Compare(initial, initial).NewErrors != 0
            || Compare(initial, initial).NewWarnings != 0)
        {
            throw new InvalidDataException("workspace initial change summary drifted");
        }
        var next = Snapshot(
            8,
            [
                new RemoteHistoryProjection("command-a", 7, "applied"),
                new RemoteHistoryProjection("command-b", 8, "applied"),
            ],
            [
                new RemoteLogProjection(2, "warning", "two changed"),
                new RemoteLogProjection(3, "error", "three"),
            ]);
        var changed = Compare(initial, next);
        if (changed.RevisionAdvance != 1
            || changed.AddedLogs != 1
            || changed.NewErrors != 1
            || changed.NewWarnings != 1
            || changed.ExpiredLogs != 1
            || changed.ChangedLogs != 1
            || changed.AddedCommands != 1
            || changed.UpdatedCommands != 1
            || changed.LogSequenceReset
            || changed.Describe()
                != "revision +1 / +1 logs / 1 new error / 1 new warning / 1 logs expired / 1 logs changed / +1 commands / 1 commands updated")
        {
            throw new InvalidDataException("workspace rolling change summary drifted");
        }
        var reset = Snapshot(
            8,
            next.History,
            [new RemoteLogProjection(1, "info", "reset")]);
        if (!Compare(next, reset).LogSequenceReset)
        {
            throw new InvalidDataException("workspace log sequence reset was hidden");
        }
        RequireRejected(
            () => Compare(next, initial),
            "workspace snapshot revision regression was accepted");
        var duplicateHistory = Snapshot(
            8,
            [
                new RemoteHistoryProjection("command-a", 7, "planned"),
                new RemoteHistoryProjection("command-a", 8, "applied"),
            ],
            next.Logs);
        RequireRejected(
            () => Compare(initial, duplicateHistory),
            "workspace duplicate command identity was accepted");
        RequireRejected(
            () => Compare(null, Snapshot(
                8,
                next.History,
                [
                    new RemoteLogProjection(2, "info", "two"),
                    new RemoteLogProjection(2, "error", "duplicate"),
                ])),
            "workspace duplicate log sequence was accepted");
        RequireRejected(
            () => Compare(initial, Snapshot(
                8,
                next.History,
                [
                    new RemoteLogProjection(3, "info", "three"),
                    new RemoteLogProjection(2, "info", "two"),
                ])),
            "workspace unordered log sequence was accepted");
        RequireRejected(
            () => Compare(null, Snapshot(
                8,
                next.History,
                [new RemoteLogProjection(3, "fatal", "unsupported")])),
            "workspace invalid log level was accepted");
        RequireRejected(
            () => Compare(null, Snapshot(
                8,
                next.History,
                Enumerable.Range(1, MaxLogEntries + 1)
                    .Select(sequence => new RemoteLogProjection(
                        (ulong)sequence,
                        "info",
                        "bounded"))
                    .ToArray())),
            "workspace oversized log window was accepted");
        RequireRejected(
            () => Compare(null, Snapshot(
                8,
                Enumerable.Range(1, MaxHistoryEntries + 1)
                    .Select(index => new RemoteHistoryProjection(
                        $"command-{index}",
                        (ulong)index,
                        "applied"))
                    .ToArray(),
                next.Logs)),
            "workspace oversized history window was accepted");
    }

    private static int CountNewLevel(
        IReadOnlyDictionary<ulong, RemoteLogProjection> previous,
        IReadOnlyDictionary<ulong, RemoteLogProjection> current,
        string level) => current.Count(entry =>
            string.Equals(entry.Value.Level, level, StringComparison.Ordinal)
            && (!previous.TryGetValue(entry.Key, out var prior)
                || !string.Equals(prior.Level, level, StringComparison.Ordinal)));

    private static Dictionary<string, RemoteHistoryProjection> CommandIndex(
        IReadOnlyList<RemoteHistoryProjection> history)
    {
        if (history.Count > MaxHistoryEntries)
        {
            throw new InvalidDataException(
                "workspace history exceeds its retained item limit");
        }
        var commands = new Dictionary<string, RemoteHistoryProjection>(
            StringComparer.Ordinal);
        foreach (var entry in history)
        {
            if (!commands.TryAdd(entry.CommandId, entry))
            {
                throw new InvalidDataException(
                    "workspace history contains a duplicate command ID");
            }
        }
        return commands;
    }

    private static Dictionary<ulong, RemoteLogProjection> LogIndex(
        IReadOnlyList<RemoteLogProjection> logs)
    {
        if (logs.Count > MaxLogEntries)
        {
            throw new InvalidDataException(
                "workspace logs exceed their retained item limit");
        }
        var entries = new Dictionary<ulong, RemoteLogProjection>();
        ulong? previousSequence = null;
        foreach (var entry in logs)
        {
            if (entry.Level is not ("trace" or "debug" or "info" or "warning" or "error"))
            {
                throw new InvalidDataException("workspace log level is invalid");
            }
            if (previousSequence is { } previous && entry.Sequence <= previous)
            {
                throw new InvalidDataException(
                    "workspace log sequence is not strictly increasing");
            }
            entries.Add(entry.Sequence, entry);
            previousSequence = entry.Sequence;
        }
        return entries;
    }

    private static void RequireRejected(Action action, string message)
    {
        try
        {
            action();
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException(message);
    }

    private static RemoteWorkspaceSnapshot Snapshot(
        ulong revision,
        IReadOnlyList<RemoteHistoryProjection> history,
        IReadOnlyList<RemoteLogProjection> logs) => new(
            revision,
            new RemoteRuntimeProjection
            {
                Id = "runtime-a",
                Name = "Runtime A",
                Revision = revision,
                Tags = new RuntimeTags(),
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            },
            history,
            logs);
}
