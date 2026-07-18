public enum WorkspaceReloadOutcome
{
    Skipped,
    Loaded,
    Failed,
    Closed,
}

public enum WorkspaceLiveRefreshState
{
    Off,
    Waiting,
    Refreshing,
    Suspended,
}

public sealed class RemoteWorkspaceLiveRefresh
{
    public static readonly TimeSpan Interval = TimeSpan.FromSeconds(5);
    public const int MaxConsecutiveFailures = 3;

    public WorkspaceLiveRefreshState State { get; private set; }
    public int ConsecutiveFailures { get; private set; }
    public bool IsRequested => State != WorkspaceLiveRefreshState.Off;
    public bool ShouldSchedule => State == WorkspaceLiveRefreshState.Waiting;
    public TimeSpan NextInterval => ConsecutiveFailures switch
    {
        0 => Interval,
        1 => TimeSpan.FromSeconds(10),
        _ => TimeSpan.FromSeconds(20),
    };

    public void Start(bool windowActive)
    {
        ConsecutiveFailures = 0;
        State = windowActive
            ? WorkspaceLiveRefreshState.Waiting
            : WorkspaceLiveRefreshState.Suspended;
    }

    public void Pause()
    {
        ConsecutiveFailures = 0;
        State = WorkspaceLiveRefreshState.Off;
    }

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

    public bool RecoverAfterExternalSuccess()
    {
        if (ConsecutiveFailures == 0
            || State is WorkspaceLiveRefreshState.Off
                or WorkspaceLiveRefreshState.Refreshing)
        {
            return false;
        }
        ConsecutiveFailures = 0;
        return true;
    }

    public void Defer(bool windowActive)
    {
        if (State != WorkspaceLiveRefreshState.Refreshing)
        {
            throw new InvalidOperationException(
                "live refresh deferral requires an admitted query");
        }
        State = windowActive
            ? WorkspaceLiveRefreshState.Waiting
            : WorkspaceLiveRefreshState.Suspended;
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
        if (succeeded)
        {
            ConsecutiveFailures = 0;
        }
        else
        {
            ConsecutiveFailures++;
            if (ConsecutiveFailures >= MaxConsecutiveFailures)
            {
                State = WorkspaceLiveRefreshState.Off;
                return;
            }
        }
        State = windowActive
            ? WorkspaceLiveRefreshState.Waiting
            : WorkspaceLiveRefreshState.Suspended;
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
        if (!refresh.IsRequested
            || !refresh.ShouldSchedule
            || refresh.ConsecutiveFailures != 1
            || refresh.NextInterval != TimeSpan.FromSeconds(10))
        {
            throw new InvalidDataException("live refresh lost its first bounded retry");
        }
        if (!refresh.RecoverAfterExternalSuccess()
            || refresh.ConsecutiveFailures != 0
            || refresh.NextInterval != Interval
            || refresh.RecoverAfterExternalSuccess())
        {
            throw new InvalidDataException("external query success did not reset live backoff");
        }
        _ = refresh.TryBegin();
        refresh.Complete(succeeded: false, windowActive: true);
        _ = refresh.TryBegin();
        refresh.Defer(windowActive: true);
        if (refresh.ConsecutiveFailures != 1
            || !refresh.ShouldSchedule
            || refresh.NextInterval != TimeSpan.FromSeconds(10))
        {
            throw new InvalidDataException("deferred live query changed its backoff state");
        }
        _ = refresh.TryBegin();
        refresh.Complete(succeeded: true, windowActive: true);
        if (refresh.ConsecutiveFailures != 0 || refresh.NextInterval != Interval)
        {
            throw new InvalidDataException("live refresh success did not reset backoff");
        }
        for (var failure = 0; failure < MaxConsecutiveFailures; failure++)
        {
            if (!refresh.TryBegin())
            {
                throw new InvalidDataException("live refresh stopped before its failure bound");
            }
            refresh.Complete(succeeded: false, windowActive: true);
        }
        if (refresh.IsRequested
            || refresh.ShouldSchedule
            || refresh.ConsecutiveFailures != MaxConsecutiveFailures)
        {
            throw new InvalidDataException("live refresh exceeded its bounded retry limit");
        }
        if (Interval != TimeSpan.FromSeconds(5))
        {
            throw new InvalidDataException("live refresh interval drifted");
        }
    }
}
