namespace Leserpent.ControlPlane;

public sealed class RuntimeDeletionRecoveryService(
    RegistryService registry,
    IRuntimeRegistrationAuthority registrationAuthority,
    ILogger<RuntimeDeletionRecoveryService> logger) : BackgroundService
{
    private static readonly TimeSpan IdleDelay = TimeSpan.FromSeconds(5);
    private static readonly TimeSpan RetryDelay = TimeSpan.FromSeconds(1);

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        while (!stoppingToken.IsCancellationRequested)
        {
            var reservations = registry.ClaimPendingRuntimeDeletions();
            foreach (var reservation in reservations)
            {
                using (reservation)
                {
                    try
                    {
                        await registrationAuthority.UnregisterAsync(
                            reservation.RuntimeIds,
                            stoppingToken);
                        registry.DeleteRuntimesById(reservation.RuntimeIds);
                        registry.CompleteRuntimeDeletion(reservation);
                        logger.LogInformation(
                            "Recovered pending runtime deletion intent {IntentId} for {RuntimeCount} runtime(s).",
                            reservation.IntentId,
                            reservation.RuntimeIds.Count);
                    }
                    catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested)
                    {
                        return;
                    }
                    catch (Exception ex)
                    {
                        logger.LogWarning(
                            ex,
                            "Pending runtime deletion intent {IntentId} did not converge; it will be retried.",
                            reservation.IntentId);
                    }
                }
            }

            try
            {
                await Task.Delay(
                    reservations.Count == 0 ? IdleDelay : RetryDelay,
                    stoppingToken);
            }
            catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested)
            {
                return;
            }
        }
    }
}
