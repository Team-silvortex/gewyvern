namespace Leserpent.ControlPlane;

public sealed record ControlPlaneWriterHealthSnapshot(
    int Version,
    string State,
    bool LeaseHeld,
    DateTimeOffset? AcquiredAt,
    DateTimeOffset? LostAt,
    ulong? AuthorityGeneration);

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

public sealed class ControlPlaneWriterFence : IHostedService
{
    private readonly ControlPlaneWriterLease lease;
    private readonly ILogger<ControlPlaneWriterFence> logger;
    private readonly DaemonAuthorityWriterSession? authoritySession;
    private readonly object sync = new();
    private string state = "starting";
    private DateTimeOffset? acquiredAt;
    private DateTimeOffset? lostAt;

    public ControlPlaneWriterFence(
        ControlPlaneWriterLease lease,
        ILogger<ControlPlaneWriterFence> logger,
        DaemonAuthorityWriterSession? authoritySession = null)
    {
        this.lease = lease;
        this.logger = logger;
        this.authoritySession = authoritySession;
    }

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

    public async Task StartAsync(CancellationToken cancellationToken)
    {
        if (!lease.TryAcquire())
        {
            lock (sync)
            {
                state = "standby";
                logger.LogWarning(
                    "Control-plane writer lease {LeasePath} is already owned; this host is read-only until a fresh process starts.",
                    lease.LeasePath);
            }
            return;
        }
        AuthorityWriterTicket? authorityTicket;
        try
        {
            authorityTicket = authoritySession is null
                ? null
                : await authoritySession.ClaimAsync(cancellationToken);
        }
        catch
        {
            lease.Dispose();
            lock (sync)
            {
                state = "authority_claim_failed";
                lostAt = DateTimeOffset.UtcNow;
            }
            throw;
        }
        lock (sync)
        {
            state = "owner";
            acquiredAt = DateTimeOffset.UtcNow;
            logger.LogInformation(
                "This host owns the control-plane writer lease {LeasePath} at authority generation {AuthorityGeneration}.",
                lease.LeasePath,
                authorityTicket?.Generation);
        }
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

    public AuthorityWriterTicket? AuthorityTicket =>
        IsWriter ? authoritySession?.Ticket : null;

    public ControlPlaneWriterHealthSnapshot Snapshot()
    {
        var held = IsWriter;
        lock (sync)
        {
            return new ControlPlaneWriterHealthSnapshot(
                2,
                state,
                held,
                acquiredAt,
                lostAt,
                authoritySession?.Ticket?.Generation);
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
