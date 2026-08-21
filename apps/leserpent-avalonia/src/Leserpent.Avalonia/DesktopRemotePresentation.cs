internal enum DesktopRemoteCatalogDomain
{
    Shell,
    Operation,
}

internal sealed record DesktopRemoteText(
    DesktopRemoteCatalogDomain Domain,
    string Key,
    IReadOnlyList<object> Values)
{
    public static DesktopRemoteText Shell(string key, params object[] values) =>
        new(DesktopRemoteCatalogDomain.Shell, key, values);

    public static DesktopRemoteText Operation(string key, params object[] values) =>
        new(DesktopRemoteCatalogDomain.Operation, key, values);

    public string Resolve(DesktopLocalization localization)
    {
        var values = Values.Select(value => value switch
        {
            DesktopRemoteText nested => nested.Resolve(localization),
            DesktopRemoteSemanticValue semantic => localization.Resolve(
                new LocalizedText
                {
                    Key = semantic.Key,
                    Fallback = semantic.Fallback,
                }),
            _ => value,
        }).ToArray();
        return Domain switch
        {
            DesktopRemoteCatalogDomain.Shell => DesktopRemoteShellCatalogs.Format(
                localization,
                Key,
                values),
            DesktopRemoteCatalogDomain.Operation =>
                DesktopRemoteOperationCatalogs.Format(localization, Key, values),
            _ => throw new ArgumentOutOfRangeException(nameof(Domain)),
        };
    }
}

internal sealed record DesktopRemoteSemanticValue(string Key, string Fallback);

internal sealed record DesktopRemoteCredentialText(
    DesktopRemoteText Label,
    DesktopRemoteText AutomationName,
    DesktopRemoteText Help,
    bool IsEnvironmentFallback);

internal static class DesktopRemotePresentation
{
    public static DesktopRemoteText Feed(RemoteFeedState state) => state.Phase switch
    {
        RemoteFeedPhase.Connecting when state.IsStale && state.Revision is { } revision =>
            DesktopRemoteText.Shell("feed.cached_connecting", revision),
        RemoteFeedPhase.Connecting => DesktopRemoteText.Shell("feed.connecting"),
        RemoteFeedPhase.Live when state.Revision is { } revision =>
            DesktopRemoteText.Shell("feed.live", revision),
        RemoteFeedPhase.Live => DesktopRemoteText.Shell("feed.connecting"),
        RemoteFeedPhase.Reconnecting when state.ConsecutiveFailures > 0 =>
            DesktopRemoteText.Shell("feed.reconnecting", state.ConsecutiveFailures),
        RemoteFeedPhase.Reconnecting when state.Revision is { } revision =>
            DesktopRemoteText.Shell("feed.resynchronizing", revision),
        RemoteFeedPhase.Reconnecting => DesktopRemoteText.Shell("feed.refreshing_snapshot"),
        RemoteFeedPhase.Stale => DesktopRemoteText.Shell(
            "feed.offline",
            Math.Max(1, state.ConsecutiveFailures)),
        RemoteFeedPhase.Stopped => DesktopRemoteText.Shell("feed.stopped"),
        _ => throw new ArgumentOutOfRangeException(nameof(state)),
    };

    public static DesktopRemoteText Revision(RemoteFeedState state) =>
        state.Revision is { } revision
            ? DesktopRemoteText.Shell("feed.revision", revision)
            : DesktopRemoteText.Shell("feed.awaiting_snapshot");

    public static DesktopRemoteText RuntimeCount(
        int visible,
        int total,
        bool automationName = false) => automationName
        ? DesktopRemoteText.Shell("a11y.count", visible, total)
        : visible == total
            ? DesktopRemoteText.Shell("count.all", total)
            : DesktopRemoteText.Shell("count.filtered", visible, total);

    public static DesktopRemoteText AuthorityHealth(RemoteAuthorityHealthState state)
    {
        if (state.Phase == RemoteAuthorityHealthPhase.Ready && state.Health is { } health)
        {
            var replay = health.OrchestraDeleteReplayHorizon;
            if (replay is not null
                && replay.AdmissionPressure
                    != RemoteOrchestraDeleteReplayAdmissionPressure.Healthy)
            {
                var key = replay.AdmissionPressure switch
                {
                    RemoteOrchestraDeleteReplayAdmissionPressure.Warning =>
                        "health.replay_warning",
                    RemoteOrchestraDeleteReplayAdmissionPressure.Critical =>
                        "health.replay_critical",
                    RemoteOrchestraDeleteReplayAdmissionPressure.Blocked =>
                        "health.replay_blocked",
                    _ => throw new InvalidDataException(
                        "remote authority replay pressure is invalid"),
                };
                return DesktopRemoteText.Shell(
                    key,
                    replay.AvailableCapacity,
                    replay.Capacity,
                    replay.CheckpointLagGenerations);
            }
            if (health.EffectQueue is { } queue)
            {
                return DesktopRemoteText.Shell(
                    queue.Saturated ? "health.queue_saturated" : "health.queue",
                    queue.Active,
                    queue.Capacity);
            }
            return DesktopRemoteText.Shell("health.ready");
        }

        return state.Phase switch
        {
            RemoteAuthorityHealthPhase.Idle => DesktopRemoteText.Shell("health.idle"),
            RemoteAuthorityHealthPhase.Checking =>
                DesktopRemoteText.Shell("health.checking"),
            RemoteAuthorityHealthPhase.Ready => DesktopRemoteText.Shell("health.ready"),
            RemoteAuthorityHealthPhase.Unavailable => state.Failure switch
            {
                RemoteAuthorityHealthFailure.AuthorityRejected =>
                    DesktopRemoteText.Shell("health.rejected"),
                RemoteAuthorityHealthFailure.InvalidRequest =>
                    DesktopRemoteText.Shell("health.invalid_request"),
                RemoteAuthorityHealthFailure.InvalidResponse =>
                    DesktopRemoteText.Shell("health.invalid_response"),
                RemoteAuthorityHealthFailure.TimedOut =>
                    DesktopRemoteText.Shell("health.timeout"),
                RemoteAuthorityHealthFailure.TransportUnavailable
                    or RemoteAuthorityHealthFailure.Unexpected =>
                    DesktopRemoteText.Shell("health.unavailable"),
                _ => throw new InvalidDataException(
                    "remote authority health failure is invalid"),
            },
            RemoteAuthorityHealthPhase.Stopped => DesktopRemoteText.Shell("health.stopped"),
            _ => throw new ArgumentOutOfRangeException(nameof(state)),
        };
    }

    public static DesktopRemoteCredentialText Credential(RemoteTokenSource source)
    {
        if (source == RemoteTokenSource.LocalProcess)
        {
            return new DesktopRemoteCredentialText(
                DesktopRemoteText.Shell("credential.local.label"),
                DesktopRemoteText.Shell("credential.local.a11y"),
                DesktopRemoteText.Shell("credential.local.help"),
                false);
        }
        if (source == RemoteTokenSource.Environment)
        {
            return new DesktopRemoteCredentialText(
                DesktopRemoteText.Shell("credential.environment.label"),
                DesktopRemoteText.Shell("credential.environment.a11y"),
                DesktopRemoteText.Shell("credential.environment.help"),
                true);
        }

        var platform = OperatingSystem.IsMacOS()
            ? "Keychain"
            : OperatingSystem.IsLinux()
                ? "Secret Service"
                : "Platform Store";
        return new DesktopRemoteCredentialText(
            DesktopRemoteText.Shell("credential.platform.label", platform.ToUpperInvariant()),
            DesktopRemoteText.Shell("credential.platform.a11y", platform),
            DesktopRemoteText.Shell("credential.platform.help", platform),
            false);
    }

    public static DesktopRemoteText AdmissionReason(
        RemoteMutationAdmissionFailure failure) => DesktopRemoteText.Operation(
            failure switch
            {
                RemoteMutationAdmissionFailure.InvalidRuntimeId =>
                    "reason.invalid_runtime_id",
                RemoteMutationAdmissionFailure.InFlight => "reason.in_flight",
                RemoteMutationAdmissionFailure.RevisionFencePending =>
                    "reason.revision_fence",
                RemoteMutationAdmissionFailure.ObservationFencePending =>
                    "reason.observation_fence",
                RemoteMutationAdmissionFailure.AuthoritativeSnapshotRequired =>
                    "reason.authoritative_snapshot",
                RemoteMutationAdmissionFailure.RuntimeUnavailable =>
                    "reason.runtime_unavailable",
                RemoteMutationAdmissionFailure.RuntimeRevisionChanged =>
                    "reason.runtime_revision_changed",
                RemoteMutationAdmissionFailure.AuthenticatedDeploymentRequired =>
                    "reason.authenticated_deployment",
                RemoteMutationAdmissionFailure.OperationInactive =>
                    "reason.operation_inactive",
                _ => throw new ArgumentOutOfRangeException(nameof(failure)),
            });

    public static DesktopRemoteText WorkspaceReason(
        RemoteWorkspaceLaunchDisposition disposition,
        int capacity) => disposition switch
    {
        RemoteWorkspaceLaunchDisposition.RejectInvalidRuntimeId =>
            DesktopRemoteText.Operation("reason.invalid_runtime_id"),
        RemoteWorkspaceLaunchDisposition.RejectCapacity =>
            DesktopRemoteText.Operation("reason.workspace_capacity", capacity),
        RemoteWorkspaceLaunchDisposition.RejectRemoved =>
            DesktopRemoteText.Operation("reason.workspace_removed"),
        RemoteWorkspaceLaunchDisposition.RejectUnavailable =>
            DesktopRemoteText.Operation("reason.workspace_unavailable"),
        _ => DesktopRemoteText.Operation("reason.workspace_incomplete"),
    };

    public static DesktopRemoteText MutationFailure(
        RemoteMutationFailure failure,
        DesktopRemoteSemanticValue operation,
        string unavailable) => failure.Kind switch
    {
        RemoteMutationFailureKind.RemoteRejected => DesktopRemoteText.Operation(
            "failure.remote_rejected",
            operation,
            failure.Code ?? unavailable,
            failure.Detail ?? unavailable),
        RemoteMutationFailureKind.InvalidRequest => DesktopRemoteText.Operation(
            "failure.invalid_request",
            operation,
            failure.Detail ?? unavailable),
        RemoteMutationFailureKind.InvalidResponse => DesktopRemoteText.Operation(
            "failure.invalid_response",
            operation,
            failure.Detail ?? unavailable),
        RemoteMutationFailureKind.Timeout =>
            DesktopRemoteText.Operation("failure.timeout", operation),
        RemoteMutationFailureKind.Transport =>
            DesktopRemoteText.Operation("failure.transport", operation),
        RemoteMutationFailureKind.Unexpected =>
            DesktopRemoteText.Operation("failure.unexpected", operation),
        _ => throw new ArgumentOutOfRangeException(nameof(failure)),
    };

    public static DesktopRemoteSemanticValue MutationLabel(RemoteMutationKind kind) => kind switch
    {
        RemoteMutationKind.Refresh => new("runtime.refresh", "Refresh runtime"),
        RemoteMutationKind.CapabilityRefresh =>
            new("runtime.capabilities.refresh", "Discover capabilities"),
        RemoteMutationKind.Deployment => new("runtime.deploy", "Deploy pipeline"),
        _ => throw new ArgumentOutOfRangeException(nameof(kind)),
    };

    public static DesktopRemoteSemanticValue RemoteActionLabel { get; } =
        new("remote.title", "Remote runtime");

    public static DesktopRemoteSemanticValue WorkspaceLabel { get; } =
        new("runtime.inspect", "Inspect runtime");
}
