internal enum WorkspaceReloadOutcome
{
    Skipped,
    Loaded,
    Failed,
    Closed,
}

internal enum WorkspaceLiveRefreshState
{
    Off,
    Waiting,
    Refreshing,
    Suspended,
}

internal sealed class RemoteWorkspaceLiveRefresh
{
    public static readonly TimeSpan Interval = TimeSpan.FromSeconds(5);

    public WorkspaceLiveRefreshState State { get; private set; }
    public bool IsRequested => State != WorkspaceLiveRefreshState.Off;
    public bool ShouldSchedule => State == WorkspaceLiveRefreshState.Waiting;

    public void Start(bool windowActive)
    {
        State = windowActive
            ? WorkspaceLiveRefreshState.Waiting
            : WorkspaceLiveRefreshState.Suspended;
    }

    public void Pause() => State = WorkspaceLiveRefreshState.Off;

    public void Activate()
    {
        if (State == WorkspaceLiveRefreshState.Suspended)
        {
            State = WorkspaceLiveRefreshState.Waiting;
        }
    }

    public void Deactivate()
    {
        if (State == WorkspaceLiveRefreshState.Waiting)
        {
            State = WorkspaceLiveRefreshState.Suspended;
        }
    }

    public bool TryBegin()
    {
        if (State != WorkspaceLiveRefreshState.Waiting)
        {
            return false;
        }
        State = WorkspaceLiveRefreshState.Refreshing;
        return true;
    }

    public void Complete(bool succeeded, bool windowActive)
    {
        if (State == WorkspaceLiveRefreshState.Off)
        {
            return;
        }
        if (State != WorkspaceLiveRefreshState.Refreshing)
        {
            throw new InvalidOperationException(
                "live refresh completion requires an in-flight query");
        }
        State = succeeded
            ? windowActive
                ? WorkspaceLiveRefreshState.Waiting
                : WorkspaceLiveRefreshState.Suspended
            : WorkspaceLiveRefreshState.Off;
    }

    public static void VerifyContract()
    {
        var refresh = new RemoteWorkspaceLiveRefresh();
        refresh.Start(windowActive: true);
        if (!refresh.ShouldSchedule || !refresh.TryBegin() || refresh.TryBegin())
        {
            throw new InvalidDataException("live refresh admitted a reentrant query");
        }
        refresh.Complete(succeeded: true, windowActive: false);
        if (refresh.State != WorkspaceLiveRefreshState.Suspended)
        {
            throw new InvalidDataException("live refresh did not suspend off-window");
        }
        refresh.Activate();
        if (!refresh.TryBegin())
        {
            throw new InvalidDataException("live refresh did not resume explicitly requested work");
        }
        refresh.Pause();
        refresh.Complete(succeeded: true, windowActive: true);
        if (refresh.State != WorkspaceLiveRefreshState.Off)
        {
            throw new InvalidDataException("live refresh resurrected a paused request");
        }
        refresh.Start(windowActive: true);
        _ = refresh.TryBegin();
        refresh.Complete(succeeded: false, windowActive: true);
        if (refresh.IsRequested || refresh.ShouldSchedule)
        {
            throw new InvalidDataException("live refresh retried after a failed query");
        }
        if (Interval != TimeSpan.FromSeconds(5))
        {
            throw new InvalidDataException("live refresh interval drifted");
        }
    }
}
