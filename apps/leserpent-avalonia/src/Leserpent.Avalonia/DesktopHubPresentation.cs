internal sealed record DesktopHubText(
    string Key,
    IReadOnlyList<object> Values)
{
    public static DesktopHubText Catalog(
        string key,
        params object[] values) => new(key, values);

    public string Resolve(DesktopLocalization localization)
    {
        var values = Values.Select(value => value is DesktopHubText nested
            ? nested.Resolve(localization)
            : value).ToArray();
        return DesktopHubCatalogs.Format(localization, Key, values);
    }
}

internal sealed record DesktopHubAuthorityHealth(
    DesktopRemoteText Text,
    bool RequiresAttention)
{
    public string Resolve(DesktopLocalization localization) =>
        Text.Resolve(localization);
}

internal static class DesktopHubPresentation
{
    public static DesktopHubText Text(
        string key,
        params object[] values) => DesktopHubText.Catalog(key, values);

    public static DesktopHubText Phase(RemoteTopologyPhase phase) => Text(
        phase switch
        {
            RemoteTopologyPhase.Live => "phase.live",
            RemoteTopologyPhase.Cached => "phase.cached",
            RemoteTopologyPhase.Retained => "phase.retained",
            _ => throw new ArgumentOutOfRangeException(nameof(phase)),
        });

    public static DesktopHubText RuntimeStatus(
        RemoteRuntimeProjection runtime)
    {
        ArgumentNullException.ThrowIfNull(runtime);
        if (runtime.Status.StatusFetchError is { Length: > 0 })
        {
            return Text("runtime.status.failed");
        }
        return Text(runtime.RefreshStatus switch
        {
            RefreshStatus.NeverRequested => "runtime.status.never_requested",
            RefreshStatus.Pending => "runtime.status.pending",
            RefreshStatus.Ready => "runtime.status.ready",
            RefreshStatus.Failed => "runtime.status.failed",
            _ => throw new ArgumentOutOfRangeException(nameof(runtime)),
        });
    }

    public static DesktopHubText FilterSummary(
        RemoteTopologySearchResult result,
        bool automationName,
        bool topologyLoading)
    {
        ArgumentNullException.ThrowIfNull(result);
        if (result.Filter.Length > 0)
        {
            return Text(
                automationName ? "a11y.filter.active" : "filter.active",
                result.VisibleAuthorityCount,
                result.TotalAuthorityCount,
                result.VisibleRuntimeCount,
                result.TotalRuntimeCount);
        }
        return automationName
            ? Text(
                "a11y.filter.all",
                result.TotalAuthorityCount,
                result.TotalRuntimeCount)
            : topologyLoading && result.TotalRuntimeCount == 0
                ? Text("filter.loading", result.TotalAuthorityCount)
                : Text(
                    "filter.all",
                    result.TotalAuthorityCount,
                    result.TotalRuntimeCount);
    }

    public static DesktopHubText RefreshSummary(
        RemoteTopologyRefreshSummary summary) => summary.RequiresAttention
        ? Text(
            "status.refresh_attention",
            summary.LiveCount,
            summary.StaleCount,
            summary.UnavailableCount)
        : Text("status.refresh_complete", summary.LiveCount);

    public static DesktopHubText LoadingSummary(RemoteTopologyState state) =>
        state switch
        {
            { Phase: RemoteTopologyPhase.Loading, Snapshot: null } =>
                Text("summary.loading"),
            { Phase: RemoteTopologyPhase.Loading, Snapshot: { } snapshot } =>
                Text("summary.refreshing", snapshot.Revision),
            _ => throw new ArgumentException(
                "Hub loading presentation requires a loading topology state",
                nameof(state)),
        };

    public static DesktopHubText TopologySummary(
        RemoteTopologyState state,
        int visibleRuntimeCount,
        bool automationName,
        string daemonName)
    {
        var snapshot = state.Snapshot
            ?? throw new ArgumentException(
                "Hub topology presentation requires a snapshot",
                nameof(state));
        if (visibleRuntimeCount < 0
            || visibleRuntimeCount > snapshot.Runtimes.Count)
        {
            throw new ArgumentOutOfRangeException(nameof(visibleRuntimeCount));
        }
        var phase = Phase(state.Phase);
        if (automationName)
        {
            return Text(
                "a11y.summary",
                daemonName,
                snapshot.Runtimes.Count,
                snapshot.Revision,
                phase);
        }
        return visibleRuntimeCount == snapshot.Runtimes.Count
            ? Text(
                "summary.full",
                phase,
                snapshot.Revision,
                snapshot.Runtimes.Count)
            : Text(
                "summary.filtered",
                phase,
                snapshot.Revision,
                visibleRuntimeCount,
                snapshot.Runtimes.Count);
    }

    public static DesktopHubAuthorityHealth AuthorityHealth(RemoteHealth health)
    {
        ArgumentNullException.ThrowIfNull(health);
        var shared = RemoteAuthorityHealthPresentation.Create(health);
        var state = new RemoteAuthorityHealthState(
            0,
            RemoteAuthorityHealthPhase.Ready,
            RemoteAuthorityHealthFailure.None,
            shared.Label,
            shared.AutomationName,
            shared.IsSaturated,
            shared.RequiresAttention,
            health);
        return new DesktopHubAuthorityHealth(
            DesktopRemotePresentation.AuthorityHealth(state),
            shared.RequiresAttention);
    }

    public static void VerifyContract()
    {
        var localization = DesktopLocalization.ForVerification();
        var simplified = DesktopLocalization.ForVerification("zh-CN");
        var runtime = new RemoteRuntimeProjection
        {
            Id = "runtime-fixture",
            Name = "Fixture",
            RefreshStatus = RefreshStatus.Ready,
            Tags = new RuntimeTags(),
            Status = new RuntimeStatusSnapshot
            {
                StatusSource = "fixture",
            },
        };
        var summary = new RemoteTopologyRefreshSummary(1, 3, 1, 1, 1);
        var emptySearch = new RemoteTopologySearchResult(
            new Dictionary<string, IReadOnlyList<RemoteRuntimeProjection>>(
                StringComparer.Ordinal)
            {
                ["fixture"] = Array.Empty<RemoteRuntimeProjection>(),
            },
            new HashSet<string>(StringComparer.Ordinal) { "fixture" },
            1,
            1,
            0,
            0,
            string.Empty);
        var health = AuthorityHealth(new RemoteHealth(
            "ready",
            true,
            1,
            new RemoteEffectQueueHealth(2, 1, 0, 0, 3, 0, 16, false)));
        if (Phase(RemoteTopologyPhase.Live).Resolve(localization) != "LIVE"
            || Phase(RemoteTopologyPhase.Cached).Resolve(simplified) != "缓存"
            || RuntimeStatus(runtime).Resolve(simplified) != "就绪"
            || RefreshSummary(summary).Resolve(localization)
                != "Topology refresh complete with attention: 1 live, 1 stale, 1 unavailable."
            || FilterSummary(emptySearch, false, true).Resolve(localization)
                != "Daemon authorities: 1 / topology loading"
            || FilterSummary(emptySearch, false, false).Resolve(localization)
                != "Daemon authorities: 1 / runtimes: 0"
            || health.Resolve(simplified) != "队列 / 3/16"
            || health.RequiresAttention)
        {
            throw new InvalidDataException(
                "Hub typed localization projection drifted");
        }

        runtime.Status.StatusFetchError = "fixture";
        if (RuntimeStatus(runtime).Resolve(localization) != "FAILED")
        {
            throw new InvalidDataException(
                "Hub runtime failure localization projection drifted");
        }
    }
}
