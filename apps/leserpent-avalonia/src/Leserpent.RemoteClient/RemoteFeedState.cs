public enum RemoteFeedPhase
{
    Connecting,
    Live,
    Reconnecting,
    Stale,
    Stopped,
}

public sealed record RemoteFeedState(
    RemoteFeedPhase Phase,
    ulong? Revision,
    IReadOnlyList<RemoteRuntimeProjection> Runtimes,
    int ConsecutiveFailures,
    bool IsStale,
    string Detail,
    ulong SnapshotGeneration = 0,
    ulong? SnapshotRevision = null)
{
    public static RemoteFeedState Initial { get; } = new(
        RemoteFeedPhase.Connecting,
        null,
        Array.Empty<RemoteRuntimeProjection>(),
        0,
        false,
        "Connecting");
}

public sealed class RemoteFeedStateMachine(int maxReconnectAttempts = 8)
{
    private ulong snapshotGeneration;
    public RemoteFeedState State { get; private set; } = RemoteFeedState.Initial;
    public bool ResyncRequested { get; private set; }

    public RemoteFeedState Hydrate(RemoteSnapshotCache cache)
    {
        if (State.Revision is not null)
        {
            throw new InvalidOperationException("remote state is already initialized");
        }
        State = new RemoteFeedState(
            RemoteFeedPhase.Connecting,
            cache.Revision,
            cache.Runtimes,
            0,
            true,
            $"Showing cached revision {cache.Revision}; connecting");
        return State;
    }

    public RemoteFeedState Accept(RemoteEvent remoteEvent)
    {
        switch (remoteEvent)
        {
            case RemoteEvent.Snapshot snapshot:
                RequireMonotonic(snapshot.Revision);
                snapshotGeneration = checked(snapshotGeneration + 1);
                State = new RemoteFeedState(
                    RemoteFeedPhase.Live,
                    snapshot.Revision,
                    snapshot.Runtimes,
                    0,
                    false,
                    $"Live at revision {snapshot.Revision}",
                    snapshotGeneration,
                    snapshot.Revision);
                ResyncRequested = false;
                break;
            case RemoteEvent.Heartbeat heartbeat:
                RequireMonotonic(heartbeat.Revision);
                State = State with
                {
                    Phase = RemoteFeedPhase.Live,
                    Revision = heartbeat.Revision,
                    ConsecutiveFailures = 0,
                    IsStale = false,
                    Detail = $"Live at revision {heartbeat.Revision}",
                };
                break;
            case RemoteEvent.ResyncRequired resync:
                State = State with
                {
                    Phase = RemoteFeedPhase.Reconnecting,
                    Revision = resync.CurrentRevision,
                    IsStale = State.Runtimes.Count > 0,
                    Detail = $"Revision {resync.RequestedAfter} cannot resume; resynchronizing",
                };
                ResyncRequested = true;
                break;
            default:
                throw new InvalidDataException("unknown remote event");
        }
        return State;
    }

    public RemoteFeedState ConnectionLost(string detail)
    {
        var failures = checked(State.ConsecutiveFailures + 1);
        var exhausted = failures >= maxReconnectAttempts;
        State = State with
        {
            Phase = exhausted ? RemoteFeedPhase.Stale : RemoteFeedPhase.Reconnecting,
            ConsecutiveFailures = failures,
            IsStale = State.Runtimes.Count > 0,
            Detail = exhausted
                ? $"Offline after {failures} attempts: {detail}"
                : $"Reconnecting ({failures}/{maxReconnectAttempts}): {detail}",
        };
        return State;
    }

    public RemoteFeedState ResetForResync()
    {
        State = State with
        {
            Phase = RemoteFeedPhase.Reconnecting,
            Revision = null,
            IsStale = State.Runtimes.Count > 0,
            Detail = "Refreshing the complete remote snapshot",
        };
        ResyncRequested = false;
        return State;
    }

    public RemoteFeedState Resume()
    {
        if (State.Phase is not (RemoteFeedPhase.Stale or RemoteFeedPhase.Stopped))
        {
            throw new InvalidOperationException("remote state is not restartable");
        }
        State = State with
        {
            Phase = RemoteFeedPhase.Connecting,
            ConsecutiveFailures = 0,
            IsStale = State.Runtimes.Count > 0,
            Detail = State.Revision is { } revision
                ? $"Reconnecting from revision {revision}"
                : "Reconnecting for a complete snapshot",
        };
        ResyncRequested = false;
        return State;
    }

    public RemoteFeedState Stop()
    {
        State = State with
        {
            Phase = RemoteFeedPhase.Stopped,
            IsStale = State.Runtimes.Count > 0,
            Detail = "Stopped",
        };
        return State;
    }

    private void RequireMonotonic(ulong revision)
    {
        if (State.Revision is { } current && revision < current)
        {
            throw new InvalidDataException("remote event revision moved backwards");
        }
    }
}
