public enum RemoteAuthorityHealthPhase
{
    Idle,
    Checking,
    Ready,
    Unavailable,
    Stopped,
}

public enum RemoteAuthorityHealthFailure
{
    None,
    AuthorityRejected,
    InvalidRequest,
    InvalidResponse,
    TransportUnavailable,
    TimedOut,
    Unexpected,
}

public sealed record RemoteAuthorityHealthState(
    ulong Generation,
    RemoteAuthorityHealthPhase Phase,
    RemoteAuthorityHealthFailure Failure,
    string Label,
    string AutomationName,
    bool IsSaturated,
    bool RequiresAttention,
    RemoteHealth? Health = null)
{
    public bool IsRefreshEnabled => Phase is
        RemoteAuthorityHealthPhase.Idle
        or RemoteAuthorityHealthPhase.Ready
        or RemoteAuthorityHealthPhase.Unavailable;
}

public sealed class RemoteAuthorityHealthCoordinator : IDisposable
{
    private sealed record ActiveRefresh(
        ulong Generation,
        RemoteAuthorityHealthState Baseline,
        CancellationTokenSource Cancellation,
        TaskCompletionSource<RemoteAuthorityHealthState> Completion);

    private readonly object sync = new();
    private readonly Func<CancellationToken, Task<RemoteHealth>> loadAsync;
    private ActiveRefresh? active;
    private bool stopped;
    private ulong generation;
    private RemoteAuthorityHealthState state = Idle(0);

    public RemoteAuthorityHealthCoordinator(
        Func<CancellationToken, Task<RemoteHealth>> loadAsync)
    {
        ArgumentNullException.ThrowIfNull(loadAsync);
        this.loadAsync = loadAsync;
    }

    public RemoteAuthorityHealthState State
    {
        get
        {
            lock (sync)
            {
                return state;
            }
        }
    }

    public Task<RemoteAuthorityHealthState> RefreshAsync(
        CancellationToken cancellationToken = default)
    {
        if (cancellationToken.IsCancellationRequested)
        {
            return Task.FromCanceled<RemoteAuthorityHealthState>(cancellationToken);
        }

        ActiveRefresh operation;
        lock (sync)
        {
            ObjectDisposedException.ThrowIf(stopped, this);
            if (active is not null)
            {
                return active.Completion.Task;
            }

            var refreshGeneration = generation = checked(generation + 1);
            operation = new ActiveRefresh(
                refreshGeneration,
                state,
                CancellationTokenSource.CreateLinkedTokenSource(cancellationToken),
                new TaskCompletionSource<RemoteAuthorityHealthState>(
                    TaskCreationOptions.RunContinuationsAsynchronously));
            active = operation;
            state = Checking(refreshGeneration);
        }

        _ = RunAsync(operation);
        return operation.Completion.Task;
    }

    public void Stop()
    {
        ActiveRefresh? retired;
        lock (sync)
        {
            if (stopped)
            {
                return;
            }
            stopped = true;
            retired = active;
            active = null;
            state = Stopped(state.Generation);
            retired?.Completion.TrySetCanceled(
                new CancellationToken(canceled: true));
        }

        if (retired is null)
        {
            return;
        }
        try
        {
            retired.Cancellation.Cancel();
        }
        catch (AggregateException)
        {
            // A hostile cancellation callback cannot keep the lifecycle alive.
        }
        catch (ObjectDisposedException)
        {
            // Completion may win the race after ownership was retired.
        }
    }

    public void Dispose() => Stop();

    public static async Task VerifyContractAsync()
    {
        var readyHealth = new RemoteHealth(
            "ready",
            true,
            1,
            new RemoteEffectQueueHealth(2, 1, 4, 0, 3, 4, 16, false));
        var release = new TaskCompletionSource<RemoteHealth>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var starts = 0;
        using var coordinator = new RemoteAuthorityHealthCoordinator(_ =>
        {
            Interlocked.Increment(ref starts);
            return release.Task;
        });
        if (coordinator.State is not
            {
                Generation: 0,
                Phase: RemoteAuthorityHealthPhase.Idle,
                Failure: RemoteAuthorityHealthFailure.None,
                IsRefreshEnabled: true,
            })
        {
            throw new InvalidDataException(
                "authority health coordinator did not start idle");
        }

        var refresh = coordinator.RefreshAsync();
        var joined = coordinator.RefreshAsync();
        if (!ReferenceEquals(refresh, joined)
            || starts != 1
            || coordinator.State is not
            {
                Generation: 1,
                Phase: RemoteAuthorityHealthPhase.Checking,
                IsRefreshEnabled: false,
            })
        {
            throw new InvalidDataException(
                "authority health coordinator did not preserve single-flight ownership");
        }
        release.SetResult(readyHealth);
        var ready = await refresh.ConfigureAwait(false);
        if (ready is not
            {
                Generation: 1,
                Phase: RemoteAuthorityHealthPhase.Ready,
                Failure: RemoteAuthorityHealthFailure.None,
                Label: "QUEUE / 3/16",
                IsSaturated: false,
                RequiresAttention: false,
                IsRefreshEnabled: true,
            }
            || coordinator.State != ready)
        {
            throw new InvalidDataException(
                "authority health coordinator lost its ready projection");
        }

        await RequireFailureAsync(
            RemoteAuthorityHealthFailure.AuthorityRejected,
            new RemoteHealthException("not_ready", "authority rejected fixture"))
            .ConfigureAwait(false);
        await RequireFailureAsync(
            RemoteAuthorityHealthFailure.InvalidRequest,
            new ArgumentException("invalid health fixture"))
            .ConfigureAwait(false);
        await RequireFailureAsync(
            RemoteAuthorityHealthFailure.InvalidResponse,
            new InvalidDataException("invalid health response fixture"))
            .ConfigureAwait(false);
        await RequireFailureAsync(
            RemoteAuthorityHealthFailure.TransportUnavailable,
            new HttpRequestException("transport fixture"))
            .ConfigureAwait(false);
        await RequireFailureAsync(
            RemoteAuthorityHealthFailure.TimedOut,
            new OperationCanceledException("timeout fixture"))
            .ConfigureAwait(false);
        await RequireFailureAsync(
            RemoteAuthorityHealthFailure.TimedOut,
            new TimeoutException("explicit timeout fixture"))
            .ConfigureAwait(false);
        await RequireFailureAsync(
            RemoteAuthorityHealthFailure.Unexpected,
            new InvalidOperationException("unexpected fixture"))
            .ConfigureAwait(false);
        using var nullResult = new RemoteAuthorityHealthCoordinator(
            _ => Task.FromResult<RemoteHealth>(null!));
        if (await nullResult.RefreshAsync().ConfigureAwait(false) is not
            {
                Phase: RemoteAuthorityHealthPhase.Unavailable,
                Failure: RemoteAuthorityHealthFailure.InvalidResponse,
            })
        {
            throw new InvalidDataException(
                "authority health coordinator accepted an empty loader result");
        }

        var cancelledStarts = 0;
        using var cancelledBeforeAdmission = new RemoteAuthorityHealthCoordinator(_ =>
        {
            Interlocked.Increment(ref cancelledStarts);
            return Task.FromResult(readyHealth);
        });
        using var alreadyCancelled = new CancellationTokenSource();
        alreadyCancelled.Cancel();
        await RequireCancelledAsync(
            cancelledBeforeAdmission.RefreshAsync(alreadyCancelled.Token),
            "pre-cancelled authority health refresh was admitted")
            .ConfigureAwait(false);
        if (cancelledStarts != 0
            || cancelledBeforeAdmission.State.Phase != RemoteAuthorityHealthPhase.Idle)
        {
            throw new InvalidDataException(
                "pre-cancelled authority health refresh changed lifecycle state");
        }

        var cancellationCalls = 0;
        using var cancellable = new RemoteAuthorityHealthCoordinator(async token =>
        {
            if (Interlocked.Increment(ref cancellationCalls) == 1)
            {
                return readyHealth;
            }
            await Task.Delay(Timeout.InfiniteTimeSpan, token).ConfigureAwait(false);
            return readyHealth;
        });
        _ = await cancellable.RefreshAsync().ConfigureAwait(false);
        using var cancellation = new CancellationTokenSource();
        var cancelledRefresh = cancellable.RefreshAsync(cancellation.Token);
        cancellation.Cancel();
        await RequireCancelledAsync(
            cancelledRefresh,
            "cancelled authority health refresh completed")
            .ConfigureAwait(false);
        if (cancellable.State is not
            {
                Generation: 2,
                Phase: RemoteAuthorityHealthPhase.Ready,
                Label: "QUEUE / 3/16",
                IsRefreshEnabled: true,
            })
        {
            throw new InvalidDataException(
                "cancelled authority health refresh did not restore its prior projection");
        }

        var ignoredRelease = new TaskCompletionSource<RemoteHealth>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var loaderSettled = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        using var stopped = new RemoteAuthorityHealthCoordinator(async _ =>
        {
            try
            {
                return await ignoredRelease.Task.ConfigureAwait(false);
            }
            finally
            {
                loaderSettled.TrySetResult();
            }
        });
        var retiredRefresh = stopped.RefreshAsync();
        stopped.Stop();
        await RequireCancelledAsync(
            retiredRefresh,
            "stopped authority health refresh remained active")
            .ConfigureAwait(false);
        ignoredRelease.SetResult(readyHealth);
        await loaderSettled.Task.ConfigureAwait(false);
        await Task.Yield();
        if (stopped.State is not
            {
                Generation: 1,
                Phase: RemoteAuthorityHealthPhase.Stopped,
                IsRefreshEnabled: false,
            })
        {
            throw new InvalidDataException(
                "retired authority health completion crossed the stop fence");
        }
        try
        {
            _ = stopped.RefreshAsync();
            throw new InvalidDataException(
                "stopped authority health coordinator accepted another refresh");
        }
        catch (ObjectDisposedException)
        {
        }
        stopped.Stop();
    }

    private async Task RunAsync(ActiveRefresh operation)
    {
        try
        {
            var health = await loadAsync(operation.Cancellation.Token)
                .ConfigureAwait(false);
            if (health is null)
            {
                throw new InvalidDataException(
                    "authority health loader returned no result");
            }
            var presentation = RemoteAuthorityHealthPresentation.Create(health);
            Publish(
                operation,
                new RemoteAuthorityHealthState(
                    operation.Generation,
                    RemoteAuthorityHealthPhase.Ready,
                    RemoteAuthorityHealthFailure.None,
                    presentation.Label,
                    presentation.AutomationName,
                    presentation.IsSaturated,
                    presentation.RequiresAttention,
                    health));
        }
        catch (OperationCanceledException) when (
            operation.Cancellation.IsCancellationRequested)
        {
            Cancel(operation);
        }
        catch (RemoteHealthException)
        {
            Fail(operation, RemoteAuthorityHealthFailure.AuthorityRejected);
        }
        catch (ArgumentException)
        {
            Fail(operation, RemoteAuthorityHealthFailure.InvalidRequest);
        }
        catch (InvalidDataException)
        {
            Fail(operation, RemoteAuthorityHealthFailure.InvalidResponse);
        }
        catch (HttpRequestException)
        {
            Fail(operation, RemoteAuthorityHealthFailure.TransportUnavailable);
        }
        catch (IOException)
        {
            Fail(operation, RemoteAuthorityHealthFailure.TransportUnavailable);
        }
        catch (OperationCanceledException)
        {
            Fail(operation, RemoteAuthorityHealthFailure.TimedOut);
        }
        catch (TimeoutException)
        {
            Fail(operation, RemoteAuthorityHealthFailure.TimedOut);
        }
        catch (Exception)
        {
            Fail(operation, RemoteAuthorityHealthFailure.Unexpected);
        }
        finally
        {
            operation.Cancellation.Dispose();
        }
    }

    private void Publish(
        ActiveRefresh operation,
        RemoteAuthorityHealthState completed)
    {
        lock (sync)
        {
            if (stopped || !ReferenceEquals(active, operation))
            {
                return;
            }
            active = null;
            state = completed;
            operation.Completion.TrySetResult(completed);
        }
    }

    private void Cancel(ActiveRefresh operation)
    {
        RemoteAuthorityHealthState restored;
        lock (sync)
        {
            if (stopped || !ReferenceEquals(active, operation))
            {
                return;
            }
            active = null;
            restored = operation.Baseline with { Generation = operation.Generation };
            state = restored;
            operation.Completion.TrySetCanceled(operation.Cancellation.Token);
        }
    }

    private void Fail(
        ActiveRefresh operation,
        RemoteAuthorityHealthFailure failure) =>
        Publish(operation, Unavailable(operation.Generation, failure));

    private static RemoteAuthorityHealthState Idle(ulong generation) => new(
        generation,
        RemoteAuthorityHealthPhase.Idle,
        RemoteAuthorityHealthFailure.None,
        "AUTHORITY / awaiting check",
        "Remote authority health has not been checked",
        false,
        false);

    private static RemoteAuthorityHealthState Checking(ulong generation) => new(
        generation,
        RemoteAuthorityHealthPhase.Checking,
        RemoteAuthorityHealthFailure.None,
        "AUTHORITY / checking",
        "Checking remote authority health",
        false,
        false);

    private static RemoteAuthorityHealthState Unavailable(
        ulong generation,
        RemoteAuthorityHealthFailure failure)
    {
        var (label, automationName) = failure switch
        {
            RemoteAuthorityHealthFailure.AuthorityRejected => (
                "AUTHORITY / rejected",
                "Remote authority rejected the health query"),
            RemoteAuthorityHealthFailure.InvalidRequest => (
                "AUTHORITY / invalid request",
                "Remote authority health request is invalid"),
            RemoteAuthorityHealthFailure.InvalidResponse => (
                "AUTHORITY / invalid response",
                "Remote authority health response is invalid"),
            RemoteAuthorityHealthFailure.TransportUnavailable => (
                "AUTHORITY / unavailable",
                "Remote authority health transport is unavailable"),
            RemoteAuthorityHealthFailure.TimedOut => (
                "AUTHORITY / timeout",
                "Remote authority health query timed out"),
            RemoteAuthorityHealthFailure.Unexpected => (
                "AUTHORITY / unavailable",
                "Remote authority health failed safely"),
            _ => throw new ArgumentOutOfRangeException(nameof(failure)),
        };
        return new RemoteAuthorityHealthState(
            generation,
            RemoteAuthorityHealthPhase.Unavailable,
            failure,
            label,
            automationName,
            false,
            true);
    }

    private static RemoteAuthorityHealthState Stopped(ulong generation) => new(
        generation,
        RemoteAuthorityHealthPhase.Stopped,
        RemoteAuthorityHealthFailure.None,
        "AUTHORITY / stopped",
        "Remote authority health monitoring stopped",
        false,
        false);

    private static async Task RequireFailureAsync(
        RemoteAuthorityHealthFailure expected,
        Exception error)
    {
        using var coordinator = new RemoteAuthorityHealthCoordinator(
            _ => Task.FromException<RemoteHealth>(error));
        var result = await coordinator.RefreshAsync().ConfigureAwait(false);
        if (result.Phase != RemoteAuthorityHealthPhase.Unavailable
            || result.Failure != expected
            || !result.RequiresAttention
            || !result.IsRefreshEnabled
            || result.Label.Length > 64
            || result.AutomationName.Length > 256
            || result.AutomationName.Contains(error.Message, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                $"authority health coordinator misclassified {expected}");
        }
    }

    private static async Task RequireCancelledAsync(
        Task<RemoteAuthorityHealthState> operation,
        string message)
    {
        try
        {
            _ = await operation.ConfigureAwait(false);
            throw new InvalidDataException(message);
        }
        catch (OperationCanceledException)
        {
        }
    }
}
