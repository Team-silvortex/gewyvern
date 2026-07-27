namespace Leserpent.ControlPlane;

public interface IOrchestraDeleteCheckpointAlertSink
{
    Task DeliverAsync(
        PersistedOrchestraDeleteCheckpointAlertDelivery delivery,
        CancellationToken cancellationToken);
}

public sealed class LoggingOrchestraDeleteCheckpointAlertSink(
    ILogger<LoggingOrchestraDeleteCheckpointAlertSink> logger) :
    IOrchestraDeleteCheckpointAlertSink
{
    public Task DeliverAsync(
        PersistedOrchestraDeleteCheckpointAlertDelivery delivery,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        logger.LogCritical(
            "Orchestra delete replay checkpoint alert {EventId} generation {AlertGeneration}: {FailureCode}, pressure {AdmissionPressure}.",
            delivery.EventId,
            delivery.AlertGeneration,
            delivery.FailureCode,
            delivery.AdmissionPressure);
        return Task.CompletedTask;
    }
}

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
    OrchestraDeleteCheckpointWorkerOptions? options = null) :
    BackgroundService
{
    private const int MaxDeliveryBatchSize = 8;
    private readonly OrchestraDeleteCheckpointWorkerOptions options =
        ValidateOptions(
            options ?? OrchestraDeleteCheckpointWorkerOptions.Default);

    protected override async Task ExecuteAsync(
        CancellationToken stoppingToken)
    {
        while (!stoppingToken.IsCancellationRequested)
        {
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

                try
                {
                    await alertSink.DeliverAsync(
                        delivery,
                        stoppingToken);
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
