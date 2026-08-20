public static class RemoteFeedAuthorityPolicy
{
    public static bool HasAuthoritativeSnapshot(RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        return state.Phase == RemoteFeedPhase.Live
            && !state.IsStale
            && state.SnapshotGeneration > 0
            && state.SnapshotRevision is { } snapshotRevision
            && state.Revision is { } revision
            && revision >= snapshotRevision;
    }

    public static void VerifyContract()
    {
        var runtime = new RemoteRuntimeProjection
        {
            Id = "runtime-a",
            Name = "Runtime A",
            Revision = 7,
            Tags = new RuntimeTags(),
            Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
        };
        var cached = new RemoteFeedState(
            RemoteFeedPhase.Connecting,
            7,
            [runtime],
            0,
            true,
            "cached");
        var heartbeatOnly = cached with
        {
            Phase = RemoteFeedPhase.Live,
            Revision = 8,
            IsStale = false,
        };
        var authoritative = heartbeatOnly with
        {
            SnapshotGeneration = 1,
            SnapshotRevision = 7,
        };
        var inconsistent = authoritative with
        {
            Revision = 6,
            SnapshotRevision = 7,
        };
        if (HasAuthoritativeSnapshot(cached)
            || HasAuthoritativeSnapshot(heartbeatOnly)
            || HasAuthoritativeSnapshot(authoritative with { IsStale = true })
            || HasAuthoritativeSnapshot(inconsistent)
            || !HasAuthoritativeSnapshot(authoritative))
        {
            throw new InvalidDataException(
                "remote authority snapshot policy drifted");
        }
    }
}
