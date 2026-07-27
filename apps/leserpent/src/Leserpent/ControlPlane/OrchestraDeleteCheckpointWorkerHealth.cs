namespace Leserpent.ControlPlane;

public sealed record OrchestraDeleteCheckpointWorkerHealthSnapshot(
    int Version,
    string WorkerState,
    bool LeaseHeld,
    DateTimeOffset? LeaseAcquiredAt,
    DateTimeOffset? LeaseLostAt,
    string AlertSinkMode,
    bool ExternalAlertSinkConfigured,
    DateTimeOffset? LastAlertDeliveryAttemptAt,
    DateTimeOffset? LastAlertDeliverySucceededAt,
    uint ConsecutiveAlertDeliveryFailures,
    string? LastAlertDeliveryFailureCode);

public sealed class OrchestraDeleteCheckpointWorkerHealth
{
    private readonly object sync = new();
    private readonly OrchestraDeleteCheckpointWorkerLease workerLease;
    private readonly TimeProvider timeProvider;
    private readonly string alertSinkMode;
    private readonly bool externalAlertSinkConfigured;
    private string workerState = "starting";
    private DateTimeOffset? leaseAcquiredAt;
    private DateTimeOffset? leaseLostAt;
    private DateTimeOffset? lastAlertDeliveryAttemptAt;
    private DateTimeOffset? lastAlertDeliverySucceededAt;
    private uint consecutiveAlertDeliveryFailures;
    private string? lastAlertDeliveryFailureCode;

    public OrchestraDeleteCheckpointWorkerHealth(
        OrchestraDeleteCheckpointWorkerLease workerLease,
        IOrchestraDeleteCheckpointAlertSink alertSink)
        : this(workerLease, alertSink, TimeProvider.System)
    {
    }

    internal OrchestraDeleteCheckpointWorkerHealth(
        OrchestraDeleteCheckpointWorkerLease workerLease,
        IOrchestraDeleteCheckpointAlertSink alertSink,
        TimeProvider timeProvider)
    {
        this.workerLease = workerLease;
        this.timeProvider = timeProvider;
        (alertSinkMode, externalAlertSinkConfigured) =
            alertSink switch
            {
                AuthenticatedHttpOrchestraDeleteCheckpointAlertSink =>
                    ("authenticated_https", true),
                LoggingOrchestraDeleteCheckpointAlertSink =>
                    ("structured_logging", false),
                _ => ("custom", true),
            };
    }

    public OrchestraDeleteCheckpointWorkerHealthSnapshot Snapshot()
    {
        var leaseHeld = workerLease.IsHeld;
        lock (sync)
        {
            if (string.Equals(
                    workerState,
                    "owner",
                    StringComparison.Ordinal) &&
                !leaseHeld)
            {
                MarkLeaseLostUnsafe();
            }
            return new(
                Version: 1,
                workerState,
                leaseHeld,
                leaseAcquiredAt,
                leaseLostAt,
                alertSinkMode,
                externalAlertSinkConfigured,
                lastAlertDeliveryAttemptAt,
                lastAlertDeliverySucceededAt,
                consecutiveAlertDeliveryFailures,
                lastAlertDeliveryFailureCode);
        }
    }

    internal void MarkStarting()
    {
        lock (sync)
        {
            workerState = "starting";
        }
    }

    internal void MarkOwner()
    {
        lock (sync)
        {
            workerState = "owner";
            leaseAcquiredAt = timeProvider.GetUtcNow();
            leaseLostAt = null;
        }
    }

    internal void MarkStandby()
    {
        lock (sync)
        {
            workerState = "standby";
        }
    }

    internal void MarkLeaseLost()
    {
        lock (sync)
        {
            MarkLeaseLostUnsafe();
        }
    }

    internal void MarkStopped()
    {
        lock (sync)
        {
            if (!string.Equals(
                    workerState,
                    "lease_lost",
                    StringComparison.Ordinal))
            {
                workerState = "stopped";
            }
        }
    }

    internal void MarkAlertDeliveryAttempt()
    {
        lock (sync)
        {
            lastAlertDeliveryAttemptAt = timeProvider.GetUtcNow();
        }
    }

    internal void MarkAlertDeliverySucceeded()
    {
        lock (sync)
        {
            lastAlertDeliverySucceededAt = timeProvider.GetUtcNow();
            consecutiveAlertDeliveryFailures = 0;
            lastAlertDeliveryFailureCode = null;
        }
    }

    internal void MarkAlertDeliveryFailed(string failureCode)
    {
        lock (sync)
        {
            consecutiveAlertDeliveryFailures =
                consecutiveAlertDeliveryFailures == uint.MaxValue
                    ? uint.MaxValue
                    : consecutiveAlertDeliveryFailures + 1;
            lastAlertDeliveryFailureCode = failureCode;
        }
    }

    private void MarkLeaseLostUnsafe()
    {
        workerState = "lease_lost";
        leaseLostAt ??= timeProvider.GetUtcNow();
    }
}
