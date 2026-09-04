using System.Buffers;
using System.Net.Security;
using System.Net.WebSockets;
using System.Security.Authentication;
using System.Security.Cryptography.X509Certificates;

public sealed record RemoteClientOptions(
    Uri Endpoint,
    string CertificateAuthorityPath,
    string Token,
    string CachePath)
{
    public static RemoteClientOptions Create(
        string endpoint,
        string certificateAuthorityPath,
        string token,
        string? cachePath = null)
    {
        var uri = ParseEndpoint(endpoint);
        ValidateToken(token);
        ValidateCertificateFile(certificateAuthorityPath);
        return new RemoteClientOptions(
            uri,
            certificateAuthorityPath,
            token,
            cachePath ?? RemoteSnapshotStore.DefaultPath(uri));
    }

    public static Uri ParseEndpoint(string endpoint)
    {
        if (!Uri.TryCreate(endpoint, UriKind.Absolute, out var uri)
            || uri.Scheme != Uri.UriSchemeHttps
            || !string.IsNullOrEmpty(uri.UserInfo)
            || !string.IsNullOrEmpty(uri.Query)
            || !string.IsNullOrEmpty(uri.Fragment)
            || uri.AbsolutePath != "/")
        {
            throw new ArgumentException("remote endpoint must be an HTTPS origin", nameof(endpoint));
        }
        return uri;
    }

    public static void ValidateToken(string token)
    {
        if (token.Length is < 32 or > 256
            || token.Any(character => character is < '!' or > '~'))
        {
            throw new ArgumentException(
                "remote token must contain 32 to 256 visible ASCII characters",
                nameof(token));
        }
    }

    private static void ValidateCertificateFile(string path)
    {
        if (!Path.IsPathFullyQualified(path))
        {
            throw new ArgumentException("remote CA path must be absolute", nameof(path));
        }
        var info = new FileInfo(path);
        if (!info.Exists || info.Length is <= 0 or > 1024 * 1024
            || (info.Attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new ArgumentException("remote CA must be a bounded regular file", nameof(path));
        }
    }
}

public sealed class RemoteEventClient : IAsyncDisposable
{
    private const string EventSubprotocol = "leserpent.events.v1";
    private static readonly TimeSpan ConnectTimeout = TimeSpan.FromSeconds(5);
    private static readonly TimeSpan EventTimeout = TimeSpan.FromSeconds(35);
    private static readonly TimeSpan MaxReconnectDelay = TimeSpan.FromSeconds(10);
    private readonly RemoteClientOptions options;
    private readonly RemoteSnapshotStore store;
    private readonly RemoteFeedStateMachine stateMachine = new();
    private readonly RemoteFeedPublisher publisher = new();
    private readonly X509Certificate2 trustedRoot;
    private readonly RemoteEventLifecycle lifecycle;

    public RemoteEventClient(RemoteClientOptions options)
    {
        this.options = options;
        store = new RemoteSnapshotStore(options.Endpoint, options.CachePath);
        trustedRoot = RemoteTls.LoadRoot(options.CertificateAuthorityPath);
        TrustIdentity = RemoteTrustIdentity.Create(options.Endpoint, trustedRoot);
        lifecycle = new RemoteEventLifecycle(ReleaseResources);
        try
        {
            if (store.Load() is { } cache)
            {
                Publish(stateMachine.Hydrate(cache));
            }
        }
        catch (InvalidDataException)
        {
            store.Clear();
        }
    }

    public event Action<RemoteFeedState>? StateChanged
    {
        add => publisher.StateChanged += value;
        remove => publisher.StateChanged -= value;
    }
    public RemoteFeedState State => stateMachine.State;
    public RemoteTrustIdentity TrustIdentity { get; }
    public int SubscriberFailureCount => publisher.SubscriberFailureCount;

    public void Start() => _ = lifecycle.Start(
        cancellationToken => RunAsync(cancellationToken));

    public async Task RestartAsync(CancellationToken cancellationToken = default)
    {
        var previous = lifecycle.RunningOrThrow();
        await previous.Task.WaitAsync(cancellationToken).ConfigureAwait(false);

        var restartGate = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        _ = lifecycle.Restart(previous, shutdownToken =>
        {
            var resumed = stateMachine.Resume();
            return RunAsync(shutdownToken, restartGate.Task, resumed);
        });
        restartGate.SetResult();
    }

    public ValueTask DisposeAsync() => new(lifecycle.DisposeAsync());

    public static async Task VerifyLifecycleContractAsync()
    {
        RemoteFeedPublisher.VerifyContract();
        await RemoteEventLifecycle.VerifyContractAsync().ConfigureAwait(false);
    }

    private async Task RunAsync(
        CancellationToken cancellationToken,
        Task? startSignal = null,
        RemoteFeedState? startingState = null)
    {
        if (startSignal is not null)
        {
            await startSignal.ConfigureAwait(false);
            Publish(startingState!);
        }
        while (!cancellationToken.IsCancellationRequested)
        {
            try
            {
                await RunConnectionAsync(cancellationToken).ConfigureAwait(false);
                throw new WebSocketException("remote event stream closed");
            }
            catch (ResyncRequiredException)
            {
                store.Clear();
                Publish(stateMachine.ResetForResync());
                continue;
            }
            catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
            {
                break;
            }
            catch (Exception error) when (error is IOException
                or InvalidDataException
                or WebSocketException
                or AuthenticationException
                or TaskCanceledException)
            {
                var state = stateMachine.ConnectionLost(error.GetType().Name);
                Publish(state);
                if (state.Phase == RemoteFeedPhase.Stale)
                {
                    break;
                }
                await Task.Delay(ReconnectDelay(state.ConsecutiveFailures), cancellationToken)
                    .ConfigureAwait(false);
            }
        }
        if (cancellationToken.IsCancellationRequested)
        {
            Publish(stateMachine.Stop());
        }
    }

    private async Task RunConnectionAsync(CancellationToken cancellationToken)
    {
        using var socket = new ClientWebSocket();
        socket.Options.AddSubProtocol(EventSubprotocol);
        socket.Options.SetRequestHeader("Authorization", $"Bearer {options.Token}");
        socket.Options.RemoteCertificateValidationCallback = ValidateServerCertificate;

        var uri = EventUri(stateMachine.State.Revision);
        using (var connect = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken))
        {
            connect.CancelAfter(ConnectTimeout);
            await socket.ConnectAsync(uri, connect.Token).ConfigureAwait(false);
        }
        if (socket.SubProtocol != EventSubprotocol)
        {
            throw new AuthenticationException("remote event subprotocol was not negotiated");
        }

        while (socket.State == WebSocketState.Open)
        {
            var payload = await ReceiveMessageAsync(socket, cancellationToken).ConfigureAwait(false);
            var remoteEvent = RemoteEventCodec.Decode(payload);
            var state = stateMachine.Accept(remoteEvent);
            if (remoteEvent is RemoteEvent.Snapshot snapshot)
            {
                store.Save(snapshot);
            }
            Publish(state);
            if (stateMachine.ResyncRequested)
            {
                throw new ResyncRequiredException();
            }
        }
    }

    private async Task<byte[]> ReceiveMessageAsync(
        ClientWebSocket socket,
        CancellationToken cancellationToken)
    {
        var rented = ArrayPool<byte>.Shared.Rent(16 * 1024);
        try
        {
            using var payload = new MemoryStream();
            while (true)
            {
                using var receive = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                receive.CancelAfter(EventTimeout);
                var result = await socket.ReceiveAsync(rented.AsMemory(), receive.Token)
                    .ConfigureAwait(false);
                if (result.MessageType == WebSocketMessageType.Close)
                {
                    throw new WebSocketException("remote event stream closed");
                }
                if (result.MessageType != WebSocketMessageType.Text)
                {
                    throw new InvalidDataException("remote event must be a text frame");
                }
                if (payload.Length + result.Count > RemoteEventCodec.MaxMessageBytes)
                {
                    throw new InvalidDataException("remote event exceeds the message limit");
                }
                payload.Write(rented, 0, result.Count);
                if (result.EndOfMessage)
                {
                    return payload.ToArray();
                }
            }
        }
        finally
        {
            ArrayPool<byte>.Shared.Return(rented, clearArray: true);
        }
    }

    private bool ValidateServerCertificate(
        object sender,
        X509Certificate? certificate,
        X509Chain? chain,
        SslPolicyErrors errors)
    {
        _ = sender;
        _ = chain;
        return RemoteTls.ValidateServerCertificate(certificate, errors, trustedRoot);
    }

    private Uri EventUri(ulong? revision)
    {
        var builder = new UriBuilder(options.Endpoint)
        {
            Scheme = "wss",
            Path = "/v1/events",
            Query = revision is null ? string.Empty : $"after_revision={revision.Value}",
        };
        return builder.Uri;
    }

    private static TimeSpan ReconnectDelay(int failures)
    {
        var milliseconds = 250 * Math.Pow(2, Math.Clamp(failures - 1, 0, 8));
        return TimeSpan.FromMilliseconds(Math.Min(milliseconds, MaxReconnectDelay.TotalMilliseconds));
    }

    private void Publish(RemoteFeedState state) => publisher.Publish(state);

    private void ReleaseResources()
    {
        publisher.Clear();
        trustedRoot.Dispose();
    }

    private sealed class ResyncRequiredException : Exception;
}
