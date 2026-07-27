namespace Leserpent.ControlPlane;

public sealed record OrchestraDeleteCheckpointWorkerOptions(
    TimeSpan IdleDelay,
    TimeSpan ReadyDelay)
{
    public static OrchestraDeleteCheckpointWorkerOptions Default { get; } =
        new(
            TimeSpan.FromSeconds(30),
            TimeSpan.FromMilliseconds(25));
}

public sealed class OrchestraDeleteCheckpointService(
    RegistryService registry,
    IOrchestraDeleteCheckpointAlertSink alertSink,
    ILogger<OrchestraDeleteCheckpointService> logger,
    OrchestraDeleteCheckpointWorkerOptions? options = null,
    OrchestraDeleteCheckpointWorkerLease? workerLease = null,
    OrchestraDeleteCheckpointWorkerHealth? workerHealth = null,
    ControlPlaneWriterFence? writerFence = null) :
    BackgroundService
{
    private const int MaxDeliveryBatchSize = 8;
    private readonly OrchestraDeleteCheckpointWorkerOptions options =
        ValidateOptions(
            options ?? OrchestraDeleteCheckpointWorkerOptions.Default);

    protected override async Task ExecuteAsync(
        CancellationToken stoppingToken)
    {
        workerHealth?.MarkStarting();
        if (writerFence is not null &&
            !writerFence.IsWriter)
        {
            workerHealth?.MarkStandby();
            logger.LogInformation(
                "Checkpoint worker is idle because this host is a control-plane standby.");
            try
            {
                await Task.Delay(
                    Timeout.InfiniteTimeSpan,
                    stoppingToken);
            }
            catch (OperationCanceledException) when (
                stoppingToken.IsCancellationRequested)
            {
            }
            return;
        }
        if (workerLease is not null &&
            !workerLease.TryAcquire())
        {
            workerHealth?.MarkStandby();
            logger.LogWarning(
                "Checkpoint worker lease {LeasePath} is already owned; this service host will not run checkpoint or alert-delivery work.",
                workerLease.LeasePath);
            try
            {
                await Task.Delay(
                    Timeout.InfiniteTimeSpan,
                    stoppingToken);
            }
            catch (OperationCanceledException) when (
                stoppingToken.IsCancellationRequested)
            {
            }
            return;
        }
        workerHealth?.MarkOwner();

        try
        {
            while (!stoppingToken.IsCancellationRequested)
            {
                if (writerFence is not null &&
                    !writerFence.IsWriter)
                {
                    workerHealth?.MarkLeaseLost();
                    logger.LogWarning(
                        "Control-plane writer ownership was lost; checkpoint and alert-delivery work is stopping.");
                    return;
                }
                if (workerLease is not null &&
                    !workerLease.IsHeld)
                {
                    workerHealth?.MarkLeaseLost();
                    logger.LogWarning(
                        "Checkpoint worker lease {LeasePath} was lost; this service host is stopping checkpoint and alert-delivery work.",
                        workerLease.LeasePath);
                    return;
                }
                try
                {
                    registry.RunOrchestraDeleteCheckpointMaintenance();
                }
                catch (Exception ex)
                {
                    logger.LogWarning(
                        ex,
                        "Automatic Orchestra delete replay checkpoint maintenance did not converge.");
                }

                for (var index = 0;
                    index < MaxDeliveryBatchSize;
                    index += 1)
                {
                    PersistedOrchestraDeleteCheckpointAlertDelivery?
                        delivery;
                    try
                    {
                        delivery = registry
                            .ClaimDueOrchestraDeleteCheckpointAlertDelivery();
                    }
                    catch (Exception ex)
                    {
                        logger.LogWarning(
                            ex,
                            "Checkpoint alert delivery claim could not be persisted.");
                        break;
                    }
                    if (delivery is null)
                    {
                        break;
                    }
                    if (workerLease is not null &&
                        !workerLease.IsHeld)
                    {
                        workerHealth?.MarkLeaseLost();
                        logger.LogWarning(
                            "Checkpoint worker lease {LeasePath} was lost before alert {EventId} delivery; this service host is stopping.",
                            workerLease.LeasePath,
                            delivery.EventId);
                        return;
                    }

                    var delivered = false;
                    workerHealth?.MarkAlertDeliveryAttempt();
                    try
                    {
                        await alertSink.DeliverAsync(
                            delivery,
                            stoppingToken);
                        delivered = true;
                        workerHealth?.MarkAlertDeliverySucceeded();
                        registry
                            .CompleteOrchestraDeleteCheckpointAlertDelivery(
                                delivery.EventId);
                    }
                    catch (OperationCanceledException) when (
                        stoppingToken.IsCancellationRequested)
                    {
                        return;
                    }
                    catch (Exception ex)
                    {
                        workerHealth?.MarkAlertDeliveryFailed(
                            delivered
                                ? "delivery_ack_persistence_failed"
                                : "sink_delivery_failed");
                        logger.LogWarning(
                            ex,
                            "Checkpoint alert {EventId} delivery failed; the durable outbox will retry it.",
                            delivery.EventId);
                        try
                        {
                            registry
                                .RecordOrchestraDeleteCheckpointAlertDeliveryFailure(
                                    delivery.EventId);
                        }
                        catch (Exception persistenceError)
                        {
                            logger.LogWarning(
                                persistenceError,
                                "Checkpoint alert {EventId} failure metadata could not be persisted.",
                                delivery.EventId);
                        }
                    }
                }

                var delay = registry
                    .GetNextOrchestraDeleteCheckpointMaintenanceDelay(
                        options.IdleDelay,
                        options.ReadyDelay);
                try
                {
                    await Task.Delay(delay, stoppingToken);
                }
                catch (OperationCanceledException) when (
                    stoppingToken.IsCancellationRequested)
                {
                    return;
                }
            }
        }
        finally
        {
            workerHealth?.MarkStopped();
        }
    }

    private static OrchestraDeleteCheckpointWorkerOptions
        ValidateOptions(
            OrchestraDeleteCheckpointWorkerOptions value)
    {
        if (value.ReadyDelay <= TimeSpan.Zero ||
            value.IdleDelay < value.ReadyDelay ||
            value.IdleDelay > TimeSpan.FromMinutes(5))
        {
            throw new ArgumentOutOfRangeException(
                nameof(value),
                "checkpoint worker delays are invalid");
        }
        return value;
    }
}
