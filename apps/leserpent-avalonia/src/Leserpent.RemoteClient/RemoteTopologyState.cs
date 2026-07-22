public enum RemoteTopologyPhase
{
    Awaiting,
    Loading,
    Live,
    Cached,
    Retained,
    Unavailable,
}

public sealed record RemoteTopologyState(
    RemoteTopologyPhase Phase,
    RemoteTopologySnapshot? Snapshot,
    int ConsecutiveFailures,
    string Detail)
{
    public static RemoteTopologyState Initial { get; } = new(
        RemoteTopologyPhase.Awaiting,
        null,
        0,
        "Awaiting topology");
}

public sealed class RemoteTopologyStateMachine
{
    public RemoteTopologyState State { get; private set; } = RemoteTopologyState.Initial;

    public RemoteTopologyState BeginRefresh()
    {
        State = State with
        {
            Phase = RemoteTopologyPhase.Loading,
            Detail = State.Snapshot is null
                ? "Loading topology"
                : "Refreshing topology",
        };
        return State;
    }

    public RemoteTopologyState Accept(RemoteTopologySnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        if (!snapshot.IsStale && snapshot.Health is null)
        {
            throw new InvalidDataException(
                "live daemon topology has no authority health proof");
        }
        if (snapshot.Health is not null)
        {
            _ = RemoteAuthorityHealthPresentation.Create(snapshot.Health);
        }
        if (State.Snapshot is { } previous && snapshot.Revision < previous.Revision)
        {
            throw new InvalidDataException(
                "daemon topology revision moved backwards");
        }
        State = new RemoteTopologyState(
            snapshot.IsStale ? RemoteTopologyPhase.Cached : RemoteTopologyPhase.Live,
            snapshot,
            0,
            snapshot.IsStale ? "Cached topology" : "Live topology");
        return State;
    }

    public RemoteTopologyState Reject()
    {
        var failures = checked(State.ConsecutiveFailures + 1);
        if (State.Snapshot is { } previous)
        {
            State = new RemoteTopologyState(
                RemoteTopologyPhase.Retained,
                previous with { IsStale = true },
                failures,
                "Refresh failed; retaining the last topology");
        }
        else
        {
            State = new RemoteTopologyState(
                RemoteTopologyPhase.Unavailable,
                null,
                failures,
                "Topology unavailable");
        }
        return State;
    }

    public static void VerifyContract()
    {
        var machine = new RemoteTopologyStateMachine();
        if (machine.BeginRefresh() is not
            {
                Phase: RemoteTopologyPhase.Loading,
                Snapshot: null,
            })
        {
            throw new InvalidDataException("initial topology loading state drifted");
        }
        if (machine.Reject() is not
            {
                Phase: RemoteTopologyPhase.Unavailable,
                Snapshot: null,
                ConsecutiveFailures: 1,
            })
        {
            throw new InvalidDataException("empty topology failure state drifted");
        }
        var live = new RemoteTopologySnapshot(
            7,
            Array.Empty<RemoteRuntimeProjection>(),
            Health: new RemoteHealth("ready", true, 1, null));
        if (machine.Accept(live) is not
            {
                Phase: RemoteTopologyPhase.Live,
                Snapshot.IsStale: false,
                ConsecutiveFailures: 0,
            })
        {
            throw new InvalidDataException("live topology state drifted");
        }
        machine.BeginRefresh();
        if (machine.Reject() is not
            {
                Phase: RemoteTopologyPhase.Retained,
                Snapshot.Revision: 7,
                Snapshot.IsStale: true,
                ConsecutiveFailures: 1,
            })
        {
            throw new InvalidDataException("retained topology state drifted");
        }
        if (machine.Accept(live with { IsStale = true }) is not
            {
                Phase: RemoteTopologyPhase.Cached,
                Snapshot.IsStale: true,
                ConsecutiveFailures: 0,
            })
        {
            throw new InvalidDataException("cached topology state drifted");
        }
        try
        {
            machine.Accept(live with { Revision = 6 });
            throw new InvalidDataException("topology accepted a revision regression");
        }
        catch (InvalidDataException error) when (
            error.Message == "daemon topology revision moved backwards")
        {
        }
        try
        {
            new RemoteTopologyStateMachine().Accept(live with { Health = null });
            throw new InvalidDataException("topology accepted live state without health proof");
        }
        catch (InvalidDataException error) when (
            error.Message == "live daemon topology has no authority health proof")
        {
        }
    }
}
