internal sealed class RemoteEventRun(Task task)
{
    public Task Task { get; } = task
        ?? throw new ArgumentNullException(nameof(task));
}

internal sealed class RemoteEventLifecycle(Action releaseResources)
{
    private readonly object sync = new();
    private readonly CancellationTokenSource shutdown = new();
    private readonly Action releaseResources = releaseResources
        ?? throw new ArgumentNullException(nameof(releaseResources));
    private RemoteEventRun? activeRun;
    private Task? disposalTask;

    public RemoteEventRun Start(Func<CancellationToken, Task> start)
    {
        ArgumentNullException.ThrowIfNull(start);
        lock (sync)
        {
            ThrowIfStopping();
            if (activeRun is not null)
            {
                throw new InvalidOperationException(
                    "remote event lifecycle is already started");
            }
            activeRun = new RemoteEventRun(
                start(shutdown.Token)
                    ?? throw new InvalidOperationException(
                        "remote event lifecycle returned no run task"));
            return activeRun;
        }
    }

    public RemoteEventRun RunningOrThrow()
    {
        lock (sync)
        {
            ThrowIfStopping();
            return activeRun
                ?? throw new InvalidOperationException(
                    "remote event lifecycle is not started");
        }
    }

    public RemoteEventRun Restart(
        RemoteEventRun previous,
        Func<CancellationToken, Task> restart)
    {
        ArgumentNullException.ThrowIfNull(previous);
        ArgumentNullException.ThrowIfNull(restart);
        lock (sync)
        {
            ThrowIfStopping();
            if (!ReferenceEquals(activeRun, previous))
            {
                throw new InvalidOperationException(
                    "remote event lifecycle was already restarted");
            }
            activeRun = new RemoteEventRun(
                restart(shutdown.Token)
                    ?? throw new InvalidOperationException(
                        "remote event lifecycle returned no restart task"));
            return activeRun;
        }
    }

    public Task DisposeAsync()
    {
        Task? running;
        TaskCompletionSource completion;
        lock (sync)
        {
            if (disposalTask is not null)
            {
                return disposalTask;
            }
            completion = new TaskCompletionSource(
                TaskCreationOptions.RunContinuationsAsynchronously);
            disposalTask = completion.Task;
            running = activeRun?.Task;
        }
        _ = CompleteDisposalAsync(running, completion);
        return completion.Task;
    }

    public static async Task VerifyContractAsync()
    {
        var releaseCount = 0;
        var entered = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var lifecycle = new RemoteEventLifecycle(
            () => Interlocked.Increment(ref releaseCount));
        _ = lifecycle.Start(async cancellationToken =>
        {
            entered.TrySetResult();
            try
            {
                await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken)
                    .ConfigureAwait(false);
            }
            catch (OperationCanceledException) when (
                cancellationToken.IsCancellationRequested)
            {
            }
        });
        await entered.Task.ConfigureAwait(false);

        var shutdowns = new Task[16];
        Parallel.For(
            0,
            shutdowns.Length,
            index => shutdowns[index] = lifecycle.DisposeAsync());
        if (shutdowns.Any(task => !ReferenceEquals(task, shutdowns[0])))
        {
            throw new InvalidDataException(
                "remote event disposal did not preserve single-flight identity");
        }
        await Task.WhenAll(shutdowns).ConfigureAwait(false);
        if (releaseCount != 1)
        {
            throw new InvalidDataException(
                "remote event disposal released resources more than once");
        }
        RequireThrows<ObjectDisposedException>(
            () => lifecycle.Start(_ => Task.CompletedTask),
            "stopped remote event lifecycle accepted a new start");

        var restartedReleaseCount = 0;
        var restarted = new RemoteEventLifecycle(
            () => Interlocked.Increment(ref restartedReleaseCount));
        var previous = restarted.Start(_ => Task.CompletedTask);
        if (!ReferenceEquals(previous, restarted.RunningOrThrow()))
        {
            throw new InvalidDataException(
                "remote event lifecycle lost its running task identity");
        }
        var replacement = restarted.Restart(previous, _ => Task.CompletedTask);
        RequireThrows<InvalidOperationException>(
            () => restarted.Restart(previous, _ => Task.CompletedTask),
            "retired remote event task replaced the current run");
        if (!ReferenceEquals(replacement, restarted.RunningOrThrow()))
        {
            throw new InvalidDataException(
                "remote event restart lost its replacement task identity");
        }
        await restarted.DisposeAsync().ConfigureAwait(false);
        await restarted.DisposeAsync().ConfigureAwait(false);
        if (restartedReleaseCount != 1)
        {
            throw new InvalidDataException(
                "restarted remote event lifecycle released resources more than once");
        }
    }

    private async Task CompleteDisposalAsync(
        Task? running,
        TaskCompletionSource completion)
    {
        Exception? failure = null;
        try
        {
            shutdown.Cancel();
        }
        catch (Exception error)
        {
            failure = error;
        }
        if (running is not null)
        {
            try
            {
                await running.ConfigureAwait(false);
            }
            catch (Exception error)
            {
                failure ??= error;
            }
        }
        try
        {
            releaseResources();
        }
        catch (Exception error)
        {
            failure ??= error;
        }
        try
        {
            shutdown.Dispose();
        }
        catch (Exception error)
        {
            failure ??= error;
        }

        if (failure is null)
        {
            completion.TrySetResult();
        }
        else
        {
            completion.TrySetException(failure);
        }
    }

    private void ThrowIfStopping() =>
        ObjectDisposedException.ThrowIf(disposalTask is not null, this);

    private static void RequireThrows<TError>(Action action, string message)
        where TError : Exception
    {
        try
        {
            action();
            throw new InvalidDataException(message);
        }
        catch (TError)
        {
        }
    }
}

internal sealed class RemoteFeedPublisher
{
    private readonly object sync = new();
    private Action<RemoteFeedState>? stateChanged;
    private int subscriberFailureCount;

    public event Action<RemoteFeedState>? StateChanged
    {
        add
        {
            lock (sync)
            {
                stateChanged += value;
            }
        }
        remove
        {
            lock (sync)
            {
                stateChanged -= value;
            }
        }
    }

    public int SubscriberFailureCount => Volatile.Read(ref subscriberFailureCount);

    public void Publish(RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        Action<RemoteFeedState>? subscribers;
        lock (sync)
        {
            subscribers = stateChanged;
        }
        if (subscribers is null)
        {
            return;
        }
        foreach (var candidate in subscribers.GetInvocationList())
        {
            try
            {
                ((Action<RemoteFeedState>)candidate)(state);
            }
            catch (Exception error) when (error is not (
                OutOfMemoryException or AccessViolationException))
            {
                IncrementFailureCount();
            }
        }
    }

    public void Clear()
    {
        lock (sync)
        {
            stateChanged = null;
        }
    }

    public static void VerifyContract()
    {
        var publisher = new RemoteFeedPublisher();
        var delivered = 0;
        Action<RemoteFeedState> hostile = _ => throw new InvalidOperationException(
            "subscriber detail must not escape");
        publisher.StateChanged += hostile;
        publisher.StateChanged += _ => delivered += 1;
        publisher.Publish(RemoteFeedState.Initial);
        publisher.Publish(RemoteFeedState.Initial);
        if (delivered != 2 || publisher.SubscriberFailureCount != 2)
        {
            throw new InvalidDataException(
                "remote feed subscriber failure interrupted healthy delivery");
        }
        publisher.StateChanged -= hostile;
        publisher.Publish(RemoteFeedState.Initial);
        if (delivered != 3 || publisher.SubscriberFailureCount != 2)
        {
            throw new InvalidDataException(
                "remote feed subscriber removal changed failure telemetry");
        }
        publisher.Clear();
        publisher.Publish(RemoteFeedState.Initial);
        if (delivered != 3)
        {
            throw new InvalidDataException(
                "cleared remote feed publisher retained a subscriber");
        }
    }

    private void IncrementFailureCount()
    {
        while (true)
        {
            var current = Volatile.Read(ref subscriberFailureCount);
            if (current == int.MaxValue)
            {
                return;
            }
            if (Interlocked.CompareExchange(
                    ref subscriberFailureCount,
                    current + 1,
                    current) == current)
            {
                return;
            }
        }
    }
}
