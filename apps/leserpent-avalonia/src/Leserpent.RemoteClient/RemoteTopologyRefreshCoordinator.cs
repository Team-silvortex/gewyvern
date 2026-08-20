using System.Collections.Concurrent;

public sealed record RemoteTopologyRefreshAuthority(
    string AuthorityId,
    Func<CancellationToken, Task<RemoteTopologyPhase>> RefreshAsync);

public sealed record RemoteTopologyRefreshSummary(
    ulong Generation,
    int AuthorityCount,
    int LiveCount,
    int StaleCount,
    int UnavailableCount)
{
    public bool RequiresAttention => StaleCount + UnavailableCount > 0;
}

public sealed class RemoteTopologyRefreshCoordinator
{
    public const int DefaultMaxConcurrency = 4;
    public const int MaxAuthorityCount = 65;
    public const int MaxAuthorityIdLength = 256;

    private readonly object sync = new();
    private readonly SemaphoreSlim gate;
    private readonly Dictionary<string, Task<RemoteTopologyPhase>> authorityOperations =
        new(StringComparer.Ordinal);
    private Task<RemoteTopologyRefreshSummary>? refreshAllOperation;
    private ulong generation;

    public RemoteTopologyRefreshCoordinator(int maxConcurrency = DefaultMaxConcurrency)
    {
        if (maxConcurrency is <= 0 or > MaxAuthorityCount)
        {
            throw new ArgumentOutOfRangeException(nameof(maxConcurrency));
        }
        gate = new SemaphoreSlim(maxConcurrency, maxConcurrency);
    }

    public ulong Generation
    {
        get
        {
            lock (sync)
            {
                return generation;
            }
        }
    }

    public bool IsRefreshingAll
    {
        get
        {
            lock (sync)
            {
                return refreshAllOperation is { IsCompleted: false };
            }
        }
    }

    public bool IsAuthorityRefreshing(string authorityId)
    {
        ValidateAuthorityId(authorityId);
        lock (sync)
        {
            return authorityOperations.TryGetValue(authorityId, out var operation)
                && !operation.IsCompleted;
        }
    }

    public Task<RemoteTopologyPhase> RefreshAuthorityAsync(
        RemoteTopologyRefreshAuthority authority,
        CancellationToken cancellationToken)
    {
        ValidateAuthority(authority);
        TaskCompletionSource<RemoteTopologyPhase> completion;
        lock (sync)
        {
            if (authorityOperations.TryGetValue(authority.AuthorityId, out var active)
                && !active.IsCompleted)
            {
                return active;
            }
            authorityOperations.Remove(authority.AuthorityId);
            completion = new TaskCompletionSource<RemoteTopologyPhase>(
                TaskCreationOptions.RunContinuationsAsynchronously);
            authorityOperations.Add(authority.AuthorityId, completion.Task);
        }
        _ = RunAuthorityRefreshAsync(authority, cancellationToken, completion);
        return completion.Task;
    }

    public Task<RemoteTopologyRefreshSummary> RefreshAllAsync(
        IEnumerable<RemoteTopologyRefreshAuthority> authorities,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(authorities);
        var bounded = authorities.ToArray();
        ValidateAuthorities(bounded);

        TaskCompletionSource<RemoteTopologyRefreshSummary> completion;
        ulong refreshGeneration;
        lock (sync)
        {
            if (refreshAllOperation is { IsCompleted: false } active)
            {
                return active;
            }
            refreshGeneration = generation = checked(generation + 1);
            completion = new TaskCompletionSource<RemoteTopologyRefreshSummary>(
                TaskCreationOptions.RunContinuationsAsynchronously);
            refreshAllOperation = completion.Task;
        }
        _ = RunRefreshAllAsync(
            bounded,
            refreshGeneration,
            cancellationToken,
            completion);
        return completion.Task;
    }

    public static async Task VerifyContractAsync()
    {
        var release = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var starts = new ConcurrentDictionary<string, int>(StringComparer.Ordinal);
        var active = 0;
        var maximumActive = 0;
        RemoteTopologyRefreshAuthority Authority(
            string authorityId,
            RemoteTopologyPhase phase) => new(
                authorityId,
                async cancellationToken =>
                {
                    starts.AddOrUpdate(authorityId, 1, (_, count) => checked(count + 1));
                    var current = Interlocked.Increment(ref active);
                    UpdateMaximum(ref maximumActive, current);
                    try
                    {
                        await release.Task.WaitAsync(cancellationToken);
                        return phase;
                    }
                    finally
                    {
                        Interlocked.Decrement(ref active);
                    }
                });

        var coordinator = new RemoteTopologyRefreshCoordinator(maxConcurrency: 2);
        var alpha = Authority("alpha", RemoteTopologyPhase.Live);
        var beta = Authority("beta", RemoteTopologyPhase.Cached);
        var gamma = Authority("gamma", RemoteTopologyPhase.Unavailable);
        var alphaRefresh = coordinator.RefreshAuthorityAsync(alpha, CancellationToken.None);
        var alphaJoin = coordinator.RefreshAuthorityAsync(alpha, CancellationToken.None);
        if (!ReferenceEquals(alphaRefresh, alphaJoin))
        {
            throw new InvalidDataException(
                "topology coordinator did not join an authority refresh");
        }
        var all = coordinator.RefreshAllAsync([alpha, beta, gamma], CancellationToken.None);
        var allJoin = coordinator.RefreshAllAsync([alpha, beta, gamma], CancellationToken.None);
        if (!ReferenceEquals(all, allJoin)
            || !coordinator.IsRefreshingAll
            || !coordinator.IsAuthorityRefreshing("alpha")
            || starts.GetValueOrDefault("alpha") != 1
            || starts.GetValueOrDefault("beta") != 1
            || starts.GetValueOrDefault("gamma") != 0
            || maximumActive != 2)
        {
            throw new InvalidDataException(
                "topology coordinator did not preserve global single-flight and bounded fanout");
        }

        release.SetResult();
        var alphaPhase = await alphaRefresh;
        var summary = await all;
        if (alphaPhase != RemoteTopologyPhase.Live
            || summary is not
            {
                Generation: 1,
                AuthorityCount: 3,
                LiveCount: 1,
                StaleCount: 1,
                UnavailableCount: 1,
                RequiresAttention: true,
            }
            || starts.GetValueOrDefault("gamma") != 1
            || coordinator.IsRefreshingAll
            || coordinator.IsAuthorityRefreshing("alpha"))
        {
            throw new InvalidDataException(
                "topology coordinator completion summary drifted");
        }

        _ = await coordinator.RefreshAuthorityAsync(alpha, CancellationToken.None);
        if (starts.GetValueOrDefault("alpha") != 2)
        {
            throw new InvalidDataException(
                "topology coordinator retained a completed authority operation");
        }

        try
        {
            _ = coordinator.RefreshAllAsync([alpha, alpha], CancellationToken.None);
            throw new InvalidDataException(
                "topology coordinator accepted duplicate authority IDs");
        }
        catch (InvalidDataException error) when (
            error.Message == "topology refresh authorities require unique identities")
        {
        }

        var boundary = new RemoteTopologyRefreshCoordinator();
        var maximumFleet = Enumerable.Range(0, MaxAuthorityCount)
            .Select(index => new RemoteTopologyRefreshAuthority(
                $"boundary-{index}",
                _ => Task.FromResult(RemoteTopologyPhase.Live)))
            .ToArray();
        var boundarySummary = await boundary.RefreshAllAsync(
            maximumFleet,
            CancellationToken.None);
        if (boundarySummary is not
            {
                AuthorityCount: MaxAuthorityCount,
                LiveCount: MaxAuthorityCount,
                StaleCount: 0,
                UnavailableCount: 0,
            })
        {
            throw new InvalidDataException(
                "topology coordinator did not accept the complete bounded fleet");
        }
        try
        {
            _ = boundary.RefreshAllAsync(
                [.. maximumFleet, new RemoteTopologyRefreshAuthority(
                    "boundary-overflow",
                    _ => Task.FromResult(RemoteTopologyPhase.Live))],
                CancellationToken.None);
            throw new InvalidDataException(
                "topology coordinator accepted an oversized authority fleet");
        }
        catch (InvalidDataException error) when (
            error.Message == "topology refresh authority count exceeds the bounded catalog")
        {
        }

        var invalid = new RemoteTopologyRefreshAuthority(
            "invalid",
            _ => Task.FromResult(RemoteTopologyPhase.Loading));
        try
        {
            _ = await coordinator.RefreshAuthorityAsync(invalid, CancellationToken.None);
            throw new InvalidDataException(
                "topology coordinator accepted a non-terminal refresh phase");
        }
        catch (InvalidDataException error) when (
            error.Message == "topology refresh returned a non-terminal phase")
        {
        }

        var queuedRelease = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var queuedStarts = 0;
        var serial = new RemoteTopologyRefreshCoordinator(maxConcurrency: 1);
        var first = serial.RefreshAuthorityAsync(
            new RemoteTopologyRefreshAuthority(
                "first",
                async cancellationToken =>
                {
                    await queuedRelease.Task.WaitAsync(cancellationToken);
                    return RemoteTopologyPhase.Live;
                }),
            CancellationToken.None);
        using var cancelled = new CancellationTokenSource();
        cancelled.Cancel();
        var queued = serial.RefreshAuthorityAsync(
            new RemoteTopologyRefreshAuthority(
                "queued",
                _ =>
                {
                    Interlocked.Increment(ref queuedStarts);
                    return Task.FromResult(RemoteTopologyPhase.Live);
                }),
            cancelled.Token);
        try
        {
            _ = await queued;
            throw new InvalidDataException(
                "topology coordinator admitted a cancelled queued refresh");
        }
        catch (OperationCanceledException)
        {
        }
        if (queuedStarts != 0)
        {
            throw new InvalidDataException(
                "cancelled topology refresh reached its authority loader");
        }
        queuedRelease.SetResult();
        _ = await first;
    }

    private async Task RunAuthorityRefreshAsync(
        RemoteTopologyRefreshAuthority authority,
        CancellationToken cancellationToken,
        TaskCompletionSource<RemoteTopologyPhase> completion)
    {
        var enteredGate = false;
        try
        {
            await gate.WaitAsync(cancellationToken);
            enteredGate = true;
            var phase = await authority.RefreshAsync(cancellationToken);
            ValidateTerminalPhase(phase);
            completion.TrySetResult(phase);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            completion.TrySetCanceled(cancellationToken);
        }
        catch (Exception error)
        {
            completion.TrySetException(error);
        }
        finally
        {
            if (enteredGate)
            {
                gate.Release();
            }
            lock (sync)
            {
                if (authorityOperations.TryGetValue(authority.AuthorityId, out var current)
                    && ReferenceEquals(current, completion.Task))
                {
                    authorityOperations.Remove(authority.AuthorityId);
                }
            }
        }
    }

    private async Task RunRefreshAllAsync(
        IReadOnlyList<RemoteTopologyRefreshAuthority> authorities,
        ulong refreshGeneration,
        CancellationToken cancellationToken,
        TaskCompletionSource<RemoteTopologyRefreshSummary> completion)
    {
        try
        {
            var phases = await Task.WhenAll(authorities.Select(authority =>
                RefreshAuthorityAsync(authority, cancellationToken)));
            completion.TrySetResult(new RemoteTopologyRefreshSummary(
                refreshGeneration,
                phases.Length,
                phases.Count(phase => phase == RemoteTopologyPhase.Live),
                phases.Count(phase => phase is
                    RemoteTopologyPhase.Cached or RemoteTopologyPhase.Retained),
                phases.Count(phase => phase == RemoteTopologyPhase.Unavailable)));
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            completion.TrySetCanceled(cancellationToken);
        }
        catch (Exception error)
        {
            completion.TrySetException(error);
        }
        finally
        {
            lock (sync)
            {
                if (ReferenceEquals(refreshAllOperation, completion.Task))
                {
                    refreshAllOperation = null;
                }
            }
        }
    }

    private static void ValidateAuthorities(
        IReadOnlyList<RemoteTopologyRefreshAuthority> authorities)
    {
        if (authorities.Count > MaxAuthorityCount)
        {
            throw new InvalidDataException(
                "topology refresh authority count exceeds the bounded catalog");
        }
        foreach (var authority in authorities)
        {
            ValidateAuthority(authority);
        }
        if (authorities.Select(authority => authority.AuthorityId)
            .Distinct(StringComparer.Ordinal).Count() != authorities.Count)
        {
            throw new InvalidDataException(
                "topology refresh authorities require unique identities");
        }
    }

    private static void ValidateAuthority(RemoteTopologyRefreshAuthority authority)
    {
        ArgumentNullException.ThrowIfNull(authority);
        ValidateAuthorityId(authority.AuthorityId);
        ArgumentNullException.ThrowIfNull(authority.RefreshAsync);
    }

    private static void ValidateAuthorityId(string authorityId)
    {
        if (string.IsNullOrWhiteSpace(authorityId)
            || authorityId.Length > MaxAuthorityIdLength
            || authorityId.Any(char.IsControl))
        {
            throw new InvalidDataException(
                "topology refresh authority identity is invalid");
        }
    }

    private static void ValidateTerminalPhase(RemoteTopologyPhase phase)
    {
        if (phase is not (RemoteTopologyPhase.Live
            or RemoteTopologyPhase.Cached
            or RemoteTopologyPhase.Retained
            or RemoteTopologyPhase.Unavailable))
        {
            throw new InvalidDataException(
                "topology refresh returned a non-terminal phase");
        }
    }

    private static void UpdateMaximum(ref int maximum, int current)
    {
        var observed = Volatile.Read(ref maximum);
        while (current > observed)
        {
            var previous = Interlocked.CompareExchange(ref maximum, current, observed);
            if (previous == observed)
            {
                return;
            }
            observed = previous;
        }
    }
}
