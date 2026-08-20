public enum RemoteWorkspaceLaunchDisposition
{
    Wait,
    Open,
    FocusExisting,
    RejectInvalidRuntimeId,
    RejectCapacity,
    RejectRemoved,
    RejectUnavailable,
}

public sealed record RemoteWorkspaceLaunchDecision(
    RemoteWorkspaceLaunchDisposition Disposition,
    string RuntimeId,
    RemoteRuntimeProjection? Runtime = null);

public static class RemoteWorkspaceLaunchPolicy
{
    public static bool IsRuntimeId(string? runtimeId) => runtimeId is not null
        && runtimeId.Length is >= 1 and <= 128
        && runtimeId.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '-' or '_' or '.' or ':');

    public static bool CanResolve(RemoteFeedState state, ulong minimumRevision)
    {
        ArgumentNullException.ThrowIfNull(state);
        return RemoteFeedAuthorityPolicy.HasAuthoritativeSnapshot(state)
            && state.SnapshotRevision >= minimumRevision;
    }

    public static void VerifyContract()
    {
        var runtimes = new[] { Runtime("runtime-a", 9) };
        var cached = new RemoteFeedState(
            RemoteFeedPhase.Live,
            9,
            runtimes,
            0,
            false,
            "cached heartbeat");
        var stale = cached with
        {
            IsStale = true,
            SnapshotGeneration = 1,
            SnapshotRevision = 9,
        };
        var heartbeatOnly = cached with { Revision = 10, SnapshotGeneration = 1 };
        var ungenerated = cached with { SnapshotRevision = 9 };
        var older = cached with { SnapshotGeneration = 1, SnapshotRevision = 8 };
        var inconsistent = cached with
        {
            Revision = 8,
            SnapshotGeneration = 1,
            SnapshotRevision = 9,
        };
        var authoritative = cached with
        {
            SnapshotGeneration = 1,
            SnapshotRevision = 9,
        };
        if (CanResolve(cached, 9)
            || CanResolve(stale, 9)
            || CanResolve(heartbeatOnly, 9)
            || CanResolve(ungenerated, 9)
            || CanResolve(older, 9)
            || CanResolve(inconsistent, 9)
            || !CanResolve(authoritative, 9)
            || !IsRuntimeId("runtime-a")
            || !IsRuntimeId(new string('a', 128))
            || IsRuntimeId(new string('a', 129))
            || IsRuntimeId("runtime/a")
            || IsRuntimeId(null))
        {
            throw new InvalidDataException(
                "Runtime workspace launch policy drifted");
        }
    }

    internal static RemoteRuntimeProjection Runtime(string id, ulong revision) => new()
    {
        Id = id,
        Name = id,
        Revision = revision,
        Tags = new RuntimeTags(),
        Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
    };
}

public sealed class RemoteWorkspaceLaunchCoordinator
{
    public const int DefaultMaxWorkspaces = 8;
    public const int MaximumWorkspaceLimit = 64;

    private readonly int maxWorkspaces;
    private readonly Dictionary<string, ulong> pending = new(StringComparer.Ordinal);

    public RemoteWorkspaceLaunchCoordinator(int maxWorkspaces = DefaultMaxWorkspaces)
    {
        if (maxWorkspaces is < 1 or > MaximumWorkspaceLimit)
        {
            throw new ArgumentOutOfRangeException(nameof(maxWorkspaces));
        }
        this.maxWorkspaces = maxWorkspaces;
    }

    public int PendingCount => pending.Count;

    public RemoteWorkspaceLaunchDecision Request(
        string runtimeId,
        ulong minimumRevision,
        RemoteFeedState state,
        IReadOnlyCollection<string> activeRuntimeIds)
    {
        ArgumentNullException.ThrowIfNull(state);
        ArgumentNullException.ThrowIfNull(activeRuntimeIds);
        if (!RemoteWorkspaceLaunchPolicy.IsRuntimeId(runtimeId))
        {
            return new(
                RemoteWorkspaceLaunchDisposition.RejectInvalidRuntimeId,
                runtimeId ?? string.Empty);
        }
        if (activeRuntimeIds.Contains(runtimeId, StringComparer.Ordinal))
        {
            pending.Remove(runtimeId);
            return new(RemoteWorkspaceLaunchDisposition.FocusExisting, runtimeId);
        }
        if (!pending.ContainsKey(runtimeId)
            && activeRuntimeIds.Count >= maxWorkspaces - pending.Count)
        {
            return new(RemoteWorkspaceLaunchDisposition.RejectCapacity, runtimeId);
        }
        if (RemoteWorkspaceLaunchPolicy.CanResolve(state, minimumRevision))
        {
            pending.Remove(runtimeId);
            var runtime = state.Runtimes.FirstOrDefault(candidate =>
                candidate.Id == runtimeId);
            return runtime is null
                ? new(RemoteWorkspaceLaunchDisposition.RejectRemoved, runtimeId)
                : new(RemoteWorkspaceLaunchDisposition.Open, runtimeId, runtime);
        }
        pending[runtimeId] = pending.TryGetValue(runtimeId, out var previousRevision)
            ? Math.Max(previousRevision, minimumRevision)
            : minimumRevision;
        return new(RemoteWorkspaceLaunchDisposition.Wait, runtimeId);
    }

    public IReadOnlyList<RemoteWorkspaceLaunchDecision> Observe(RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        if (pending.Count == 0)
        {
            return Array.Empty<RemoteWorkspaceLaunchDecision>();
        }
        if (state.Phase is RemoteFeedPhase.Stale or RemoteFeedPhase.Stopped)
        {
            var rejected = pending.Keys
                .Order(StringComparer.Ordinal)
                .Select(runtimeId => new RemoteWorkspaceLaunchDecision(
                    RemoteWorkspaceLaunchDisposition.RejectUnavailable,
                    runtimeId))
                .ToArray();
            pending.Clear();
            return rejected;
        }

        var decisions = new List<RemoteWorkspaceLaunchDecision>();
        foreach (var request in pending.OrderBy(item => item.Key, StringComparer.Ordinal).ToArray())
        {
            if (!RemoteWorkspaceLaunchPolicy.CanResolve(state, request.Value))
            {
                continue;
            }
            pending.Remove(request.Key);
            var runtime = state.Runtimes.FirstOrDefault(candidate =>
                candidate.Id == request.Key);
            decisions.Add(runtime is null
                ? new(RemoteWorkspaceLaunchDisposition.RejectRemoved, request.Key)
                : new(RemoteWorkspaceLaunchDisposition.Open, request.Key, runtime));
        }
        return decisions;
    }

    public int ClearPending()
    {
        var count = pending.Count;
        pending.Clear();
        return count;
    }

    public static void VerifyContract()
    {
        RemoteWorkspaceLaunchPolicy.VerifyContract();
        var runtimeA = RemoteWorkspaceLaunchPolicy.Runtime("runtime-a", 10);
        var runtimeB = RemoteWorkspaceLaunchPolicy.Runtime("runtime-b", 9);
        var heartbeatOnly = new RemoteFeedState(
            RemoteFeedPhase.Live,
            10,
            [runtimeA, runtimeB],
            0,
            false,
            "heartbeat without a new snapshot",
            1,
            8);
        var revisionNine = heartbeatOnly with
        {
            Revision = 9,
            SnapshotGeneration = 2,
            SnapshotRevision = 9,
        };
        var revisionTen = heartbeatOnly with
        {
            SnapshotGeneration = 3,
            SnapshotRevision = 10,
        };

        var coordinator = new RemoteWorkspaceLaunchCoordinator(2);
        Require(
            coordinator.Request("runtime/a", 9, revisionNine, Array.Empty<string>()).Disposition
                == RemoteWorkspaceLaunchDisposition.RejectInvalidRuntimeId,
            "workspace launch accepted an invalid runtime ID");
        Require(
            coordinator.Request("runtime-a", 9, heartbeatOnly, Array.Empty<string>()).Disposition
                == RemoteWorkspaceLaunchDisposition.Wait,
            "heartbeat-only state opened a workspace");
        Require(
            coordinator.Request("runtime-a", 10, heartbeatOnly, Array.Empty<string>()).Disposition
                == RemoteWorkspaceLaunchDisposition.Wait
                && coordinator.PendingCount == 1,
            "duplicate workspace request did not coalesce its revision fence");
        Require(
            coordinator.Request("runtime-b", 9, heartbeatOnly, Array.Empty<string>()).Disposition
                == RemoteWorkspaceLaunchDisposition.Wait
                && coordinator.PendingCount == 2,
            "workspace launch did not retain a bounded second request");
        Require(
            coordinator.Request("runtime-c", 9, heartbeatOnly, Array.Empty<string>()).Disposition
                == RemoteWorkspaceLaunchDisposition.RejectCapacity,
            "workspace launch exceeded its combined active and pending capacity");

        var revisionNineDecisions = coordinator.Observe(revisionNine);
        Require(
            revisionNineDecisions is
            [
                {
                    Disposition: RemoteWorkspaceLaunchDisposition.Open,
                    RuntimeId: "runtime-b",
                    Runtime.Id: "runtime-b",
                },
            ]
                && coordinator.PendingCount == 1,
            "workspace launch ignored its coalesced snapshot revision fence");
        var revisionTenDecisions = coordinator.Observe(revisionTen);
        Require(
            revisionTenDecisions is
            [
                {
                    Disposition: RemoteWorkspaceLaunchDisposition.Open,
                    RuntimeId: "runtime-a",
                    Runtime.Id: "runtime-a",
                },
            ]
                && coordinator.PendingCount == 0,
            "authoritative workspace launch did not resolve exactly once");

        var removed = new RemoteWorkspaceLaunchCoordinator();
        Require(
            removed.Request("runtime-c", 10, heartbeatOnly, Array.Empty<string>()).Disposition
                == RemoteWorkspaceLaunchDisposition.Wait,
            "removed runtime fixture did not enter the pending state");
        Require(
            removed.Observe(revisionTen) is
            [
                {
                    Disposition: RemoteWorkspaceLaunchDisposition.RejectRemoved,
                    RuntimeId: "runtime-c",
                },
            ],
            "authoritative workspace launch fabricated a removed runtime");

        var unavailable = new RemoteWorkspaceLaunchCoordinator();
        _ = unavailable.Request("runtime-a", 11, revisionTen, Array.Empty<string>());
        var rejected = unavailable.Observe(revisionTen with
        {
            Phase = RemoteFeedPhase.Stale,
            IsStale = true,
        });
        Require(
            rejected is
            [
                {
                    Disposition: RemoteWorkspaceLaunchDisposition.RejectUnavailable,
                    RuntimeId: "runtime-a",
                },
            ]
                && unavailable.PendingCount == 0,
            "terminal remote state retained a pending workspace request");

        var existing = new RemoteWorkspaceLaunchCoordinator();
        _ = existing.Request("runtime-a", 11, revisionTen, Array.Empty<string>());
        Require(
            existing.Request("runtime-a", 11, revisionTen, ["runtime-a"]).Disposition
                == RemoteWorkspaceLaunchDisposition.FocusExisting
                && existing.PendingCount == 0,
            "existing workspace did not supersede its pending request");
        RequireThrows<ArgumentOutOfRangeException>(
            () => new RemoteWorkspaceLaunchCoordinator(0),
            "workspace coordinator accepted a zero workspace limit");
        RequireThrows<ArgumentOutOfRangeException>(
            () => new RemoteWorkspaceLaunchCoordinator(MaximumWorkspaceLimit + 1),
            "workspace coordinator accepted an unbounded workspace limit");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidDataException(message);
        }
    }

    private static void RequireThrows<TException>(Action action, string message)
        where TException : Exception
    {
        try
        {
            action();
        }
        catch (TException)
        {
            return;
        }
        throw new InvalidDataException(message);
    }
}
