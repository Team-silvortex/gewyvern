public interface IMobileCredentialVault
{
    ValueTask<string?> LoadAsync(Uri endpoint, CancellationToken cancellationToken);
    ValueTask StoreAsync(Uri endpoint, string token, CancellationToken cancellationToken);
    ValueTask DeleteAsync(Uri endpoint, CancellationToken cancellationToken);
}

public interface IMobileRemoteSession : IAsyncDisposable
{
    event Action<RemoteFeedState>? StateChanged;
    RemoteFeedState State { get; }
    void Start();
    Task<RemoteWorkspaceSnapshot> LoadWorkspaceAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken);
    Task<RemoteMutationResult> ExecuteMutationAsync(
        RemoteUiActionIntent intent,
        string principal,
        CancellationToken cancellationToken);
}

public interface IMobileRemoteSessionFactory
{
    IMobileRemoteSession Create(RemoteClientOptions options);
}

public enum MobileLifecyclePhase
{
    Inactive,
    Foreground,
    Background,
    Stopped,
}

public sealed record MobileRemoteLifecycleSnapshot(
    MobileLifecyclePhase Phase,
    int Generation,
    RemoteFeedState Feed);

public sealed class MobileRemoteGenerationRetiredException(
    string message,
    Exception? innerException = null) : OperationCanceledException(message, innerException);

public sealed class MobileRemoteLifecycle : IAsyncDisposable
{
    private readonly Uri endpoint;
    private readonly string certificateAuthorityPath;
    private readonly string? cachePath;
    private readonly IMobileCredentialVault vault;
    private readonly IMobileRemoteSessionFactory sessionFactory;
    private readonly SemaphoreSlim transitions = new(1, 1);
    private readonly object stateGate = new();
    private MobileRemoteLifecycleSnapshot snapshot = new(
        MobileLifecyclePhase.Inactive,
        0,
        RemoteFeedState.Initial);
    private IMobileRemoteSession? session;
    private Action<RemoteFeedState>? sessionHandler;

    public MobileRemoteLifecycle(
        string endpoint,
        string certificateAuthorityPath,
        string? cachePath,
        IMobileCredentialVault vault,
        IMobileRemoteSessionFactory? sessionFactory = null)
    {
        this.endpoint = RemoteClientOptions.ParseEndpoint(endpoint);
        this.certificateAuthorityPath = Path.GetFullPath(certificateAuthorityPath);
        this.cachePath = cachePath is null ? null : Path.GetFullPath(cachePath);
        this.vault = vault;
        this.sessionFactory = sessionFactory ?? MobileRemoteSessionFactory.Instance;
    }

    public event Action<MobileRemoteLifecycleSnapshot>? StateChanged;

    public MobileRemoteLifecycleSnapshot State
    {
        get
        {
            lock (stateGate)
            {
                return snapshot;
            }
        }
    }

    public async ValueTask EnterForegroundAsync(CancellationToken cancellationToken = default)
    {
        await transitions.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            var phase = State.Phase;
            if (phase is not (MobileLifecyclePhase.Inactive or MobileLifecyclePhase.Background))
            {
                throw new InvalidOperationException("mobile remote lifecycle cannot enter foreground");
            }
            var token = await vault.LoadAsync(endpoint, cancellationToken).ConfigureAwait(false)
                ?? throw new InvalidDataException("mobile credential vault has no remote token");
            RemoteClientOptions.ValidateToken(token);
            var options = RemoteClientOptions.Create(
                RemoteTokenResolver.Account(endpoint),
                certificateAuthorityPath,
                token,
                cachePath);
            var candidate = sessionFactory.Create(options);
            int generation;
            Action<RemoteFeedState> handler;
            lock (stateGate)
            {
                generation = checked(snapshot.Generation + 1);
                handler = feed => AcceptSessionState(generation, feed);
                session = candidate;
                sessionHandler = handler;
                snapshot = new MobileRemoteLifecycleSnapshot(
                    MobileLifecyclePhase.Foreground,
                    generation,
                    candidate.State);
            }
            candidate.StateChanged += handler;
            Publish(State);
            try
            {
                candidate.Start();
            }
            catch
            {
                lock (stateGate)
                {
                    session = null;
                    sessionHandler = null;
                    snapshot = new MobileRemoteLifecycleSnapshot(
                        MobileLifecyclePhase.Background,
                        checked(snapshot.Generation + 1),
                        Stale(snapshot.Feed, "Mobile foreground connection failed"));
                }
                Publish(State);
                await ReleaseSessionAsync(candidate, handler).ConfigureAwait(false);
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
            IMobileRemoteSession active;
            Action<RemoteFeedState> handler;
            lock (stateGate)
            {
                if (snapshot.Phase != MobileLifecyclePhase.Foreground
                    || session is null
                    || sessionHandler is null)
                {
                    throw new InvalidOperationException("mobile remote lifecycle is not foreground");
                }
                active = session;
                handler = sessionHandler;
                session = null;
                sessionHandler = null;
                snapshot = new MobileRemoteLifecycleSnapshot(
                    MobileLifecyclePhase.Background,
                    checked(snapshot.Generation + 1),
                    Stale(snapshot.Feed, "Suspended in background"));
            }
            Publish(State);
            await ReleaseSessionAsync(active, handler).ConfigureAwait(false);
        }
        finally
        {
            transitions.Release();
        }
    }

    public Task<RemoteWorkspaceSnapshot> LoadWorkspaceAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken = default) => UseForegroundSessionAsync(
            active => active.LoadWorkspaceAsync(
                runtimeId,
                principal,
                cancellationToken));

    public Task<RemoteMutationResult> ExecuteMutationAsync(
        RemoteUiActionIntent intent,
        string principal,
        CancellationToken cancellationToken = default) => UseForegroundSessionAsync(
            active => active.ExecuteMutationAsync(intent, principal, cancellationToken));

    public async ValueTask DisposeAsync()
    {
        await transitions.WaitAsync().ConfigureAwait(false);
        try
        {
            IMobileRemoteSession? active;
            Action<RemoteFeedState>? handler;
            lock (stateGate)
            {
                if (snapshot.Phase == MobileLifecyclePhase.Stopped)
                {
                    return;
                }
                active = session;
                handler = sessionHandler;
                session = null;
                sessionHandler = null;
                snapshot = new MobileRemoteLifecycleSnapshot(
                    MobileLifecyclePhase.Stopped,
                    checked(snapshot.Generation + 1),
                    Stale(snapshot.Feed, "Mobile lifecycle stopped"));
            }
            Publish(State);
            if (active is not null && handler is not null)
            {
                await ReleaseSessionAsync(active, handler).ConfigureAwait(false);
            }
        }
        finally
        {
            transitions.Release();
        }
    }

    private void AcceptSessionState(int generation, RemoteFeedState feed)
    {
        MobileRemoteLifecycleSnapshot accepted;
        lock (stateGate)
        {
            if (snapshot.Phase != MobileLifecyclePhase.Foreground
                || snapshot.Generation != generation)
            {
                return;
            }
            snapshot = snapshot with { Feed = feed };
            accepted = snapshot;
        }
        Publish(accepted);
    }

    private static RemoteFeedState Stale(RemoteFeedState feed, string detail) => feed with
    {
        Phase = RemoteFeedPhase.Stale,
        IsStale = feed.Runtimes.Count > 0,
        Detail = detail,
    };

    private static async ValueTask ReleaseSessionAsync(
        IMobileRemoteSession active,
        Action<RemoteFeedState> handler)
    {
        active.StateChanged -= handler;
        await active.DisposeAsync().ConfigureAwait(false);
    }

    private async Task<T> UseForegroundSessionAsync<T>(
        Func<IMobileRemoteSession, Task<T>> operation)
    {
        ArgumentNullException.ThrowIfNull(operation);
        IMobileRemoteSession active;
        int generation;
        lock (stateGate)
        {
            if (snapshot.Phase != MobileLifecyclePhase.Foreground || session is null)
            {
                throw new InvalidOperationException(
                    "mobile remote operation requires a foreground session");
            }
            active = session;
            generation = snapshot.Generation;
        }
        try
        {
            var result = await operation(active).ConfigureAwait(false);
            RequireCurrentSession(active, generation, null);
            return result;
        }
        catch (Exception error) when (error is not MobileRemoteGenerationRetiredException)
        {
            RequireCurrentSession(active, generation, error);
            throw;
        }
    }

    private void RequireCurrentSession(
        IMobileRemoteSession active,
        int generation,
        Exception? error)
    {
        lock (stateGate)
        {
            if (snapshot.Phase == MobileLifecyclePhase.Foreground
                && snapshot.Generation == generation
                && ReferenceEquals(session, active))
            {
                return;
            }
        }
        throw new MobileRemoteGenerationRetiredException(
            "mobile remote operation belongs to a retired foreground generation",
            error);
    }

    private void Publish(MobileRemoteLifecycleSnapshot value) => StateChanged?.Invoke(value);
}

public sealed class MobileRemoteSessionFactory : IMobileRemoteSessionFactory
{
    public static MobileRemoteSessionFactory Instance { get; } = new();

    private MobileRemoteSessionFactory()
    {
    }

    public IMobileRemoteSession Create(RemoteClientOptions options) =>
        new MobileRemoteSession(
            new RemoteEventClient(options),
            new RemoteWorkspaceClient(options),
            new RemoteMutationClient(options));
}

internal sealed class MobileRemoteSession(
    RemoteEventClient eventClient,
    RemoteWorkspaceClient workspaceClient,
    RemoteMutationClient mutationClient) : IMobileRemoteSession
{
    public event Action<RemoteFeedState>? StateChanged
    {
        add => eventClient.StateChanged += value;
        remove => eventClient.StateChanged -= value;
    }

    public RemoteFeedState State => eventClient.State;

    public void Start() => eventClient.Start();

    public Task<RemoteWorkspaceSnapshot> LoadWorkspaceAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken) => workspaceClient.LoadAsync(
            runtimeId,
            principal,
            cancellationToken: cancellationToken);

    public Task<RemoteMutationResult> ExecuteMutationAsync(
        RemoteUiActionIntent intent,
        string principal,
        CancellationToken cancellationToken) => intent.Kind switch
        {
            ActionKind.RuntimeRefresh => mutationClient.RefreshAsync(
                intent.Runtime.Id,
                intent.Runtime.Revision,
                principal,
                cancellationToken),
            ActionKind.RuntimeCapabilitiesRefresh => mutationClient.RefreshCapabilitiesAsync(
                intent.Runtime.Id,
                intent.Runtime.Revision,
                principal,
                cancellationToken),
            ActionKind.RuntimeDeploy when intent.PipelineKind is { } pipelineKind =>
                mutationClient.DeployAsync(
                    intent.Runtime.Id,
                    intent.Runtime.Revision,
                    principal,
                    pipelineKind,
                    intent.Target,
                    cancellationToken),
            _ => throw new ArgumentException(
                "mobile remote mutation intent is unsupported",
                nameof(intent)),
        };

    public async ValueTask DisposeAsync()
    {
        try
        {
            await eventClient.DisposeAsync().ConfigureAwait(false);
        }
        finally
        {
            workspaceClient.Dispose();
            mutationClient.Dispose();
        }
    }
}
