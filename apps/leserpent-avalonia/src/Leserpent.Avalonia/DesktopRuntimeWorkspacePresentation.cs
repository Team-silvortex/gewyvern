internal sealed record DesktopRuntimeWorkspaceText(
    string? Key,
    IReadOnlyList<object> Values,
    IReadOnlyList<DesktopRuntimeWorkspaceText>? Parts)
{
    public static DesktopRuntimeWorkspaceText Catalog(
        string key,
        params object[] values) => new(key, values, null);

    public static DesktopRuntimeWorkspaceText Join(
        IEnumerable<DesktopRuntimeWorkspaceText> parts) => new(
            null,
            Array.Empty<object>(),
            parts.ToArray());

    public string Resolve(DesktopLocalization localization)
    {
        if (Parts is not null)
        {
            return string.Join(" / ", Parts.Select(part => part.Resolve(localization)));
        }
        if (Key is null)
        {
            throw new InvalidDataException(
                "runtime workspace presentation has no catalog key");
        }
        var values = Values.Select(value => value is DesktopRuntimeWorkspaceText nested
            ? nested.Resolve(localization)
            : value).ToArray();
        return DesktopRuntimeWorkspaceCatalogs.Format(localization, Key, values);
    }
}

internal enum DesktopRuntimeWorkspaceQueryFailure
{
    RemoteRejected,
    InvalidResponse,
    Blocked,
    Timeout,
    Transport,
}

internal static class DesktopRuntimeWorkspacePresentation
{
    public static DesktopRuntimeWorkspaceText Text(
        string key,
        params object[] values) => DesktopRuntimeWorkspaceText.Catalog(key, values);

    public static DesktopRuntimeWorkspaceText LogLevel(string level) => Text(
        level switch
        {
            RemoteWorkspaceLogFilter.AllLevels => "level.all",
            "trace" => "level.trace",
            "debug" => "level.debug",
            "info" => "level.info",
            "warning" => "level.warning",
            "error" => "level.error",
            _ => throw new ArgumentOutOfRangeException(nameof(level)),
        });

    public static DesktopRuntimeWorkspaceText LiveDescription(
        RemoteWorkspaceLiveRefresh refresh)
    {
        var intervalSeconds = Seconds(refresh.NextInterval);
        return refresh.State switch
        {
            WorkspaceLiveRefreshState.Waiting when refresh.ConsecutiveFailures == 0 =>
                Text("live.waiting", intervalSeconds),
            WorkspaceLiveRefreshState.Waiting => Text(
                "live.recovering",
                refresh.ConsecutiveFailures,
                intervalSeconds),
            WorkspaceLiveRefreshState.Refreshing => Text("live.refreshing"),
            WorkspaceLiveRefreshState.Suspended => Text("live.suspended"),
            _ => Text(
                "live.idle",
                Seconds(RemoteWorkspaceLiveRefresh.Interval)),
        };
    }

    public static DesktopRuntimeWorkspaceText LogFilterSummary(
        RemoteWorkspaceLogView view) => view.IsActive
        ? Text("filter.some", view.VisibleLogCount, view.TotalLogCount)
        : Text("filter.all", view.TotalLogCount);

    public static DesktopRuntimeWorkspaceText Change(
        RemoteWorkspaceSnapshotChange change)
    {
        if (change.IsInitial)
        {
            return Text("change.initial");
        }
        var parts = new List<DesktopRuntimeWorkspaceText>();
        if (change.RevisionAdvance > 0)
        {
            parts.Add(Text("change.revision", change.RevisionAdvance));
        }
        if (change.LogSequenceReset)
        {
            parts.Add(Text("change.log_sequence_reset"));
        }
        if (change.AddedLogs > 0)
        {
            parts.Add(Text("change.logs_added", change.AddedLogs));
        }
        if (change.NewErrors > 0)
        {
            parts.Add(Text("change.errors_new", change.NewErrors));
        }
        if (change.NewWarnings > 0)
        {
            parts.Add(Text("change.warnings_new", change.NewWarnings));
        }
        if (change.ExpiredLogs > 0)
        {
            parts.Add(Text("change.logs_expired", change.ExpiredLogs));
        }
        if (change.ChangedLogs > 0)
        {
            parts.Add(Text("change.logs_changed", change.ChangedLogs));
        }
        if (change.AddedCommands > 0)
        {
            parts.Add(Text("change.commands_added", change.AddedCommands));
        }
        if (change.UpdatedCommands > 0)
        {
            parts.Add(Text("change.commands_updated", change.UpdatedCommands));
        }
        return parts.Count == 0
            ? Text("change.none")
            : DesktopRuntimeWorkspaceText.Join(parts);
    }

    public static DesktopRuntimeWorkspaceText Alert(
        RemoteWorkspaceSeverityAlert alert) => alert.Level switch
    {
        WorkspaceSeverityAlertLevel.Error =>
            Text("alert.error", alert.SignalRevision),
        WorkspaceSeverityAlertLevel.Warning =>
            Text("alert.warning", alert.SignalRevision),
        WorkspaceSeverityAlertLevel.None => Text("alert.none"),
        _ => throw new ArgumentOutOfRangeException(nameof(alert)),
    };

    public static DesktopRuntimeWorkspaceText Loaded(
        ulong revision,
        bool liveRequested,
        bool incremental,
        RemoteWorkspaceSnapshotChange change,
        RemoteWorkspaceSeverityAlert alert)
    {
        var changeText = Change(change);
        if (liveRequested)
        {
            var kind = Text(incremental
                ? "snapshot.incremental"
                : "snapshot.full");
            var intervalSeconds = Seconds(RemoteWorkspaceLiveRefresh.Interval);
            return alert.IsPending
                ? Text(
                    "status.live_alert",
                    revision,
                    kind,
                    changeText,
                    Alert(alert),
                    intervalSeconds)
                : Text(
                    "status.live",
                    revision,
                    kind,
                    changeText,
                    intervalSeconds);
        }
        return alert.IsPending
            ? Text("status.workspace_alert", revision, changeText, Alert(alert))
            : Text("status.workspace", revision, changeText);
    }

    public static DesktopRuntimeWorkspaceText QueryFailure(
        DesktopRuntimeWorkspaceQueryFailure failure,
        params string[] detail) => failure switch
    {
        DesktopRuntimeWorkspaceQueryFailure.RemoteRejected when detail.Length == 2 =>
            Text("failure.rejected", detail[0], detail[1]),
        DesktopRuntimeWorkspaceQueryFailure.InvalidResponse when detail.Length == 1 =>
            Text("failure.response", detail[0]),
        DesktopRuntimeWorkspaceQueryFailure.Blocked when detail.Length == 1 =>
            Text("failure.blocked", detail[0]),
        DesktopRuntimeWorkspaceQueryFailure.Timeout when detail.Length == 0 =>
            Text("failure.timeout"),
        DesktopRuntimeWorkspaceQueryFailure.Transport when detail.Length == 0 =>
            Text("failure.transport"),
        _ => throw new ArgumentException(
            "runtime workspace query failure detail is invalid",
            nameof(detail)),
    };

    public static DesktopRuntimeWorkspaceText LiveFailure(
        RemoteWorkspaceLiveRefresh refresh,
        bool unexpected)
    {
        if (!refresh.IsRequested)
        {
            return Text(
                "status.live_stopped",
                RemoteWorkspaceLiveRefresh.MaxConsecutiveFailures);
        }
        var reason = Text(unexpected
            ? "live.reason.unexpected"
            : "live.reason.authenticated");
        var recovery = refresh.State == WorkspaceLiveRefreshState.Suspended
            ? Text("live.recovery.active")
            : Text("live.recovery.delay", Seconds(refresh.NextInterval));
        return Text(
            "status.live_recovering",
            reason,
            refresh.ConsecutiveFailures,
            RemoteWorkspaceLiveRefresh.MaxConsecutiveFailures,
            recovery);
    }

    public static void VerifyContract()
    {
        var change = new RemoteWorkspaceSnapshotChange(
            false,
            1,
            2,
            1,
            1,
            0,
            0,
            0,
            0,
            false);
        if (Change(change).Resolve(DesktopLocalization.ForVerification())
                != "revision +1 / +2 logs / 1 new error / 1 new warning"
            || Change(change).Resolve(DesktopLocalization.ForVerification("zh-CN"))
                != "修订 +1 / +2 条日志 / 1 个新错误 / 1 个新警告"
            || LogLevel("warning").Resolve(DesktopLocalization.ForVerification("de"))
                != "Warnung")
        {
            throw new InvalidDataException(
                "runtime workspace typed localization projection drifted");
        }
    }

    private static int Seconds(TimeSpan interval) =>
        Math.Max(1, (int)Math.Ceiling(interval.TotalSeconds));
}
