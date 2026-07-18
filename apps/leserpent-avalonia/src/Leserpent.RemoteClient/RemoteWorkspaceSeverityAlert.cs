public enum WorkspaceSeverityAlertLevel
{
    None,
    Warning,
    Error,
}

public sealed class RemoteWorkspaceSeverityAlert
{
    public WorkspaceSeverityAlertLevel Level { get; private set; }
    public ulong SignalRevision { get; private set; }
    public bool IsPending => Level != WorkspaceSeverityAlertLevel.None;

    public bool Observe(ulong revision, RemoteWorkspaceSnapshotChange change)
    {
        if (change.NewErrors > 0)
        {
            Level = WorkspaceSeverityAlertLevel.Error;
            SignalRevision = revision;
            return true;
        }
        if (change.NewWarnings > 0)
        {
            if (Level != WorkspaceSeverityAlertLevel.Error)
            {
                Level = WorkspaceSeverityAlertLevel.Warning;
                SignalRevision = revision;
            }
            return true;
        }
        return false;
    }

    public bool Acknowledge()
    {
        if (!IsPending)
        {
            return false;
        }
        Level = WorkspaceSeverityAlertLevel.None;
        SignalRevision = 0;
        return true;
    }

    public string Describe() => Level switch
    {
        WorkspaceSeverityAlertLevel.Error =>
            $"unacknowledged error signal from revision {SignalRevision}",
        WorkspaceSeverityAlertLevel.Warning =>
            $"unacknowledged warning signal from revision {SignalRevision}",
        _ => "no unacknowledged severity signal",
    };

    public static void VerifyContract()
    {
        var alert = new RemoteWorkspaceSeverityAlert();
        var unchanged = Change();
        if (alert.Observe(7, unchanged) || alert.IsPending)
        {
            throw new InvalidDataException("initial snapshot created a severity alert");
        }
        var warning = Change(newWarnings: 1);
        if (!alert.Observe(8, warning)
            || alert.Level != WorkspaceSeverityAlertLevel.Warning
            || alert.SignalRevision != 8
            || alert.Describe() != "unacknowledged warning signal from revision 8")
        {
            throw new InvalidDataException("workspace warning alert was not retained");
        }
        if (alert.Observe(8, unchanged)
            || alert.Level != WorkspaceSeverityAlertLevel.Warning)
        {
            throw new InvalidDataException("unchanged refresh discarded a pending alert");
        }
        if (!alert.Observe(9, Change(newErrors: 1))
            || alert.Level != WorkspaceSeverityAlertLevel.Error
            || alert.SignalRevision != 9)
        {
            throw new InvalidDataException("workspace error did not upgrade the pending alert");
        }
        if (!alert.Observe(10, warning)
            || alert.Level != WorkspaceSeverityAlertLevel.Error
            || alert.SignalRevision != 9)
        {
            throw new InvalidDataException("workspace warning downgraded a pending error");
        }
        if (!alert.Acknowledge() || alert.IsPending || alert.Acknowledge())
        {
            throw new InvalidDataException("workspace severity acknowledgement drifted");
        }
    }

    private static RemoteWorkspaceSnapshotChange Change(
        int newErrors = 0,
        int newWarnings = 0) => new(
            false, 0, 0, newErrors, newWarnings, 0, 0, 0, 0, false);
}
