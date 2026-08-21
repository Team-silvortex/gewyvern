public enum MobileApplicationPhase
{
    Unconfigured,
    Inactive,
    Foreground,
    Background,
    Faulted,
    Stopped,
}

public sealed record MobileApplicationSnapshot(
    MobileApplicationPhase Phase,
    MobileRemoteLifecycleSnapshot? Remote,
    string? Error);

public sealed class MobileRemoteMutationException(
    string message,
    RemoteMutationFailure failure) : Exception(message)
{
    public RemoteMutationFailure Failure { get; } = failure;
}

public sealed class MobileApplicationCoordinator : IAsyncDisposable
{
    private const string MobilePrincipal = "leserpent-mobile";
    private readonly IMobileCredentialVault vault;
    private readonly IMobileRemoteSessionFactory sessionFactory;
    private readonly SemaphoreSlim transitions = new(1, 1);
    private readonly SemaphoreSlim operations = new(1, 1);
    private readonly object stateGate = new();
    private readonly object mutationGate = new();
    private MobileApplicationSnapshot snapshot = new(
        MobileApplicationPhase.Unconfigured,
        null,
        null);
    private MobileRemoteLifecycle? lifecycle;
    private Action<MobileRemoteLifecycleSnapshot>? lifecycleHandler;
    private RemoteMutationCoordinator mutationCoordinator = new();

    public MobileApplicationCoordinator(
        IMobileCredentialVault vault,
        IMobileRemoteSessionFactory? sessionFactory = null)
    {
        this.vault = vault;
        this.sessionFactory = sessionFactory ?? MobileRemoteSessionFactory.Instance;
    }

    public event Action<MobileApplicationSnapshot>? StateChanged;

    public MobileApplicationSnapshot State
    {
        get
        {
            lock (stateGate)
            {
                return snapshot;
            }
        }
    }

    public RemoteMutationAvailability MutationAvailability
    {
        get
        {
            var feed = State.Remote?.Feed ?? RemoteFeedState.Initial;
            lock (mutationGate)
            {
                return mutationCoordinator.Availability(feed);
            }
        }
    }

    public async ValueTask ConfigureAsync(
        string endpoint,
        string certificateAuthorityPath,
        string? cachePath,
        string? token,
        CancellationToken cancellationToken = default)
    {
        await transitions.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            ThrowIfStopped();
            var endpointUri = RemoteClientOptions.ParseEndpoint(endpoint);
            if (token is not null)
            {
                await vault.StoreAsync(endpointUri, token, cancellationToken)
                    .ConfigureAwait(false);
            }
            var candidate = new MobileRemoteLifecycle(
                endpointUri.AbsoluteUri,
                certificateAuthorityPath,
                cachePath,
                vault,
                sessionFactory);
            Action<MobileRemoteLifecycleSnapshot> handler = AcceptLifecycleState;
            candidate.StateChanged += handler;
            var previous = lifecycle;
            var previousHandler = lifecycleHandler;
            lifecycle = candidate;
            lifecycleHandler = handler;
            lock (mutationGate)
            {
                mutationCoordinator.CancelActive();
                mutationCoordinator = new RemoteMutationCoordinator();
            }
            Publish(new MobileApplicationSnapshot(
                MobileApplicationPhase.Inactive,
                candidate.State,
                null));
            if (previous is not null && previousHandler is not null)
            {
                previous.StateChanged -= previousHandler;
                await previous.DisposeAsync().ConfigureAwait(false);
            }
        }
        finally
        {
            transitions.Release();
        }
    }

    public async ValueTask EnterForegroundAsync(CancellationToken cancellationToken = default)
    {
        await transitions.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            ThrowIfStopped();
            var active = lifecycle
                ?? throw new InvalidOperationException("mobile application is not configured");
            if (active.State.Phase == MobileLifecyclePhase.Foreground)
            {
                return;
            }
            try
            {
                await active.EnterForegroundAsync(cancellationToken).ConfigureAwait(false);
            }
            catch (Exception error) when (error is not OperationCanceledException)
            {
                Publish(new MobileApplicationSnapshot(
                    MobileApplicationPhase.Faulted,
                    active.State,
                    SafeError(error.Message)));
                throw;
            }
        }
        finally
        {
            transitions.Release();
        }
    }

    public async ValueTask EnterBackgroundAsync()
    {
        await transitions.WaitAsync().ConfigureAwait(false);
        try
        {
            ThrowIfStopped();
            if (lifecycle is not { State.Phase: MobileLifecyclePhase.Foreground } active)
            {
                return;
            }
            await active.EnterBackgroundAsync().ConfigureAwait(false);
        }
        finally
        {
            transitions.Release();
        }
    }

    public async Task<RemoteWorkspaceSnapshot> LoadWorkspaceAsync(
        string runtimeId,
        CancellationToken cancellationToken = default)
    {
        var (active, feed) = RequireForegroundRemote();
        RemoteMutationAvailability availability;
        lock (mutationGate)
        {
            availability = mutationCoordinator.Availability(feed);
        }
        if (!availability.InspectEnabled)
        {
            throw new InvalidOperationException(
                availability.InspectUnavailableReason
                ?? "runtime inspection is unavailable");
        }
        if (!feed.Runtimes.Any(runtime =>
                StringComparer.Ordinal.Equals(runtime.Id, runtimeId)))
        {
            throw new InvalidOperationException(
                "runtime inspection target is no longer available");
        }
        return await active.LoadWorkspaceAsync(
            runtimeId,
            MobilePrincipal,
            cancellationToken).ConfigureAwait(false);
    }

    public async Task<RemoteMutationResult> ExecuteMutationAsync(
        RemoteUiActionIntent intent,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(intent);
        await operations.WaitAsync(cancellationToken).ConfigureAwait(false);
        RemoteMutationOperation? operation = null;
        RemoteMutationCoordinator? owner = null;
        try
        {
            var (active, feed) = RequireForegroundRemote();
            var kind = MutationKind(intent);
            RemoteMutationAdmission admission;
            lock (mutationGate)
            {
                owner = mutationCoordinator;
                admission = owner.Begin(
                    new RemoteMutationRequest(
                        intent.Runtime.Id,
                        intent.Runtime.Revision,
                        kind),
                    feed);
                if (admission.Accepted && admission.Operation is { } admitted)
                {
                    var confirmed = owner.Confirm(admitted, feed);
                    admission = confirmed;
                }
            }
            if (!admission.Accepted || admission.Operation is not { } accepted)
            {
                throw new InvalidOperationException(
                    $"Remote change blocked: {RemoteMutationCoordinator.DescribeFailure(admission.Failure)}");
            }
            operation = accepted;
            RemoteMutationResult result;
            try
            {
                result = await active.ExecuteMutationAsync(
                    intent,
                    MobilePrincipal,
                    cancellationToken).ConfigureAwait(false);
            }
            catch (Exception error)
            {
                RemoteMutationFailure failure;
                lock (mutationGate)
                {
                    failure = owner.CompleteFailure(
                        operation,
                        error,
                        State.Remote?.Feed ?? feed,
                        cancellationToken.IsCancellationRequested
                            || error is MobileRemoteGenerationRetiredException);
                }
                operation = null;
                if (failure.Disposition == RemoteMutationFailureDisposition.Cancelled)
                {
                    throw new OperationCanceledException(
                        "mobile remote mutation owner was retired",
                        error,
                        cancellationToken);
                }
                throw new MobileRemoteMutationException(
                    failure.OperatorMessage ?? "Remote mutation failed safely",
                    failure);
            }
            lock (mutationGate)
            {
                owner.Accept(
                    operation,
                    result,
                    State.Remote?.Feed ?? feed);
            }
            operation = null;
            return result;
        }
        finally
        {
            if (operation is not null && owner is not null)
            {
                lock (mutationGate)
                {
                    owner.Abandon(
                        operation,
                        State.Remote?.Feed ?? RemoteFeedState.Initial);
                }
            }
            operations.Release();
        }
    }

    public async ValueTask DisposeAsync()
    {
        await transitions.WaitAsync().ConfigureAwait(false);
        try
        {
            if (State.Phase == MobileApplicationPhase.Stopped)
            {
                return;
            }
            var active = lifecycle;
            var handler = lifecycleHandler;
            lifecycle = null;
            lifecycleHandler = null;
            lock (mutationGate)
            {
                mutationCoordinator.CancelActive();
                mutationCoordinator = new RemoteMutationCoordinator();
            }
            if (active is not null && handler is not null)
            {
                active.StateChanged -= handler;
                await active.DisposeAsync().ConfigureAwait(false);
            }
            Publish(new MobileApplicationSnapshot(
                MobileApplicationPhase.Stopped,
                active?.State,
                null));
        }
        finally
        {
            transitions.Release();
        }
    }

    private void AcceptLifecycleState(MobileRemoteLifecycleSnapshot remote)
    {
        lock (mutationGate)
        {
            mutationCoordinator.Observe(remote.Feed);
        }
        var phase = remote.Phase switch
        {
            MobileLifecyclePhase.Inactive => MobileApplicationPhase.Inactive,
            MobileLifecyclePhase.Foreground => MobileApplicationPhase.Foreground,
            MobileLifecyclePhase.Background => MobileApplicationPhase.Background,
            MobileLifecyclePhase.Stopped => MobileApplicationPhase.Stopped,
            _ => throw new InvalidDataException("unknown mobile lifecycle phase"),
        };
        Publish(new MobileApplicationSnapshot(phase, remote, null));
    }

    private void ThrowIfStopped()
    {
        if (State.Phase == MobileApplicationPhase.Stopped)
        {
            throw new ObjectDisposedException(nameof(MobileApplicationCoordinator));
        }
    }

    private void Publish(MobileApplicationSnapshot value)
    {
        lock (stateGate)
        {
            snapshot = value;
        }
        StateChanged?.Invoke(value);
    }

    private (MobileRemoteLifecycle Lifecycle, RemoteFeedState Feed) RequireForegroundRemote()
    {
        ThrowIfStopped();
        var current = State;
        if (current.Phase != MobileApplicationPhase.Foreground
            || current.Remote is not { Phase: MobileLifecyclePhase.Foreground } remote
            || lifecycle is not { } active)
        {
            throw new InvalidOperationException(
                "mobile remote operation requires a foreground application");
        }
        return (active, remote.Feed);
    }

    private static RemoteMutationKind MutationKind(RemoteUiActionIntent intent) =>
        intent.Kind switch
        {
            ActionKind.RuntimeRefresh when intent.PipelineKind is null && intent.Target is null =>
                RemoteMutationKind.Refresh,
            ActionKind.RuntimeCapabilitiesRefresh
                when intent.PipelineKind is null && intent.Target is null =>
                RemoteMutationKind.CapabilityRefresh,
            ActionKind.RuntimeDeploy when intent.PipelineKind is not null =>
                RemoteMutationKind.Deployment,
            _ => throw new ArgumentException(
                "mobile remote mutation intent is invalid",
                nameof(intent)),
        };

    private static string SafeError(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
