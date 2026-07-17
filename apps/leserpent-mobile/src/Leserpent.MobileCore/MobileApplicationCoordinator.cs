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

public sealed class MobileApplicationCoordinator : IAsyncDisposable
{
    private readonly IMobileCredentialVault vault;
    private readonly IMobileRemoteSessionFactory sessionFactory;
    private readonly SemaphoreSlim transitions = new(1, 1);
    private readonly object stateGate = new();
    private MobileApplicationSnapshot snapshot = new(
        MobileApplicationPhase.Unconfigured,
        null,
        null);
    private MobileRemoteLifecycle? lifecycle;
    private Action<MobileRemoteLifecycleSnapshot>? lifecycleHandler;

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

    private static string SafeError(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
