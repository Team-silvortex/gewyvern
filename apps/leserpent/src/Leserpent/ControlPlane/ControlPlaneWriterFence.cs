namespace Leserpent.ControlPlane;

public sealed record ControlPlaneWriterHealthSnapshot(
    int Version,
    string State,
    bool LeaseHeld,
    DateTimeOffset? AcquiredAt,
    DateTimeOffset? LostAt);

public sealed class ControlPlaneWriterUnavailableException :
    InvalidOperationException
{
    public const string ErrorCode = "control_plane_writer_standby";

    public ControlPlaneWriterUnavailableException()
        : base(
            "control-plane mutation requires active writer ownership")
    {
    }
}

public sealed class ControlPlaneWriterFence(
    ControlPlaneWriterLease lease,
    ILogger<ControlPlaneWriterFence> logger) : IHostedService
{
    private readonly object sync = new();
    private string state = "starting";
    private DateTimeOffset? acquiredAt;
    private DateTimeOffset? lostAt;

    public bool IsWriter
    {
        get
        {
            var held = lease.IsHeld;
            lock (sync)
            {
                if (!held && state == "owner")
                {
                    state = "lease_lost";
                    lostAt ??= DateTimeOffset.UtcNow;
                }
                return held && state == "owner";
            }
        }
    }

    public Task StartAsync(CancellationToken cancellationToken)
    {
        lock (sync)
        {
            if (lease.TryAcquire())
            {
                state = "owner";
                acquiredAt = DateTimeOffset.UtcNow;
                logger.LogInformation(
                    "This host owns the control-plane writer lease {LeasePath}.",
                    lease.LeasePath);
            }
            else
            {
                state = "standby";
                logger.LogWarning(
                    "Control-plane writer lease {LeasePath} is already owned; this host is read-only until a fresh process starts.",
                    lease.LeasePath);
            }
        }
        return Task.CompletedTask;
    }

    public Task StopAsync(CancellationToken cancellationToken)
    {
        lock (sync)
        {
            if (state != "lease_lost")
            {
                state = "stopped";
            }
        }
        return Task.CompletedTask;
    }

    public void RequireWriter()
    {
        if (!IsWriter)
        {
            throw new ControlPlaneWriterUnavailableException();
        }
    }

    public ControlPlaneWriterHealthSnapshot Snapshot()
    {
        var held = IsWriter;
        lock (sync)
        {
            return new ControlPlaneWriterHealthSnapshot(
                1,
                state,
                held,
                acquiredAt,
                lostAt);
        }
    }
}

public static class ControlPlaneMutationPolicy
{
    private static readonly HashSet<string> ReadOnlyPostPaths =
        new(StringComparer.OrdinalIgnoreCase)
        {
            "/v1/runtimes/registration-plan",
        };

    public static bool IsMutation(HttpRequest request)
    {
        if (!request.Path.StartsWithSegments(
                "/v1",
                StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }
        if (HttpMethods.IsGet(request.Method) ||
            HttpMethods.IsHead(request.Method) ||
            HttpMethods.IsOptions(request.Method))
        {
            return false;
        }
        return !HttpMethods.IsPost(request.Method) ||
            !ReadOnlyPostPaths.Contains(request.Path.Value ?? string.Empty);
    }
}
