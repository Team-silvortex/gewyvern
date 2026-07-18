public sealed class RemoteWorkspaceLogRefreshPlan
{
    public const int IncrementalPollsBeforeFullSnapshot = 11;

    public int IncrementalPollsSinceFullSnapshot { get; private set; }

    public ulong? SelectCursor(
        bool allowIncremental,
        RemoteWorkspaceSnapshot? previous)
    {
        if (!allowIncremental
            || IncrementalPollsSinceFullSnapshot >= IncrementalPollsBeforeFullSnapshot
            || previous?.Logs.Count is null or 0)
        {
            return null;
        }
        return previous.Logs[^1].Sequence;
    }

    public void RecordSuccess(bool usedIncremental)
    {
        IncrementalPollsSinceFullSnapshot = usedIncremental
            ? IncrementalPollsSinceFullSnapshot + 1
            : 0;
    }

    public static bool RequiresFullFallback(
        RemoteWorkspaceSnapshot previous,
        RemoteWorkspaceSnapshot incremental) =>
        incremental.Revision != previous.Revision
        || incremental.Logs.Count == RemoteWorkspaceClient.MaxLogEntries;

    public static void VerifyContract()
    {
        var plan = new RemoteWorkspaceLogRefreshPlan();
        var snapshot = Snapshot(7, [new RemoteLogProjection(4, "info", "four")]);
        if (plan.SelectCursor(allowIncremental: false, snapshot) is not null)
        {
            throw new InvalidDataException("manual workspace reload selected a log cursor");
        }
        for (var poll = 0; poll < IncrementalPollsBeforeFullSnapshot; poll++)
        {
            if (plan.SelectCursor(allowIncremental: true, snapshot) != 4)
            {
                throw new InvalidDataException("live workspace refresh lost its bounded cursor");
            }
            plan.RecordSuccess(usedIncremental: true);
        }
        if (plan.SelectCursor(allowIncremental: true, snapshot) is not null)
        {
            throw new InvalidDataException("live workspace refresh skipped periodic full resync");
        }
        plan.RecordSuccess(usedIncremental: false);
        if (plan.IncrementalPollsSinceFullSnapshot != 0
            || plan.SelectCursor(allowIncremental: true, Snapshot(7, [])) is not null)
        {
            throw new InvalidDataException("full workspace refresh did not reset cursor cadence");
        }
        if (!RequiresFullFallback(snapshot, Snapshot(8, []))
            || !RequiresFullFallback(snapshot, Snapshot(
                7,
                Enumerable.Range(5, RemoteWorkspaceClient.MaxLogEntries)
                    .Select(sequence => new RemoteLogProjection(
                        (ulong)sequence,
                        "info",
                        "full"))
                    .ToArray()))
            || RequiresFullFallback(snapshot, Snapshot(
                7,
                [new RemoteLogProjection(5, "info", "five")])))
        {
            throw new InvalidDataException("workspace incremental fallback policy drifted");
        }
    }

    private static RemoteWorkspaceSnapshot Snapshot(
        ulong revision,
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
            [],
            logs);
}
