using System.Collections.Concurrent;

namespace Leserpent.ControlPlane;

public sealed class RuntimeDeletionRecoveryService(
    RegistryService registry,
    IRuntimeRegistrationAuthority registrationAuthority,
    ILogger<RuntimeDeletionRecoveryService> logger) : BackgroundService
{
    private const int MaxRecoveryBatchSize = 32;
    private const int MaxConcurrentAuthorityMutations = 8;
    private static readonly TimeSpan IdleDelay = TimeSpan.FromSeconds(5);
    private static readonly TimeSpan RetryDelay = TimeSpan.FromSeconds(1);

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        while (!stoppingToken.IsCancellationRequested)
        {
            var reservations = registry.ClaimPendingRuntimeDeletions(
                MaxRecoveryBatchSize);
            var successfulReservations =
                new ConcurrentBag<RuntimeDeletionReservation>();
            try
            {
                try
                {
                    await Parallel.ForEachAsync(
                        reservations,
                        new ParallelOptions
                        {
                            CancellationToken = stoppingToken,
                            MaxDegreeOfParallelism =
                                MaxConcurrentAuthorityMutations,
                        },
                        async (reservation, cancellationToken) =>
                        {
                            try
                            {
                                await registrationAuthority.UnregisterAsync(
                                    reservation.RuntimeIds,
                                    cancellationToken);
                                successfulReservations.Add(reservation);
                            }
                            catch (OperationCanceledException) when (
                                cancellationToken.IsCancellationRequested)
                            {
                                throw;
                            }
                            catch (Exception ex)
                            {
                                logger.LogWarning(
                                    ex,
                                    "Pending runtime deletion intent {IntentId} did not converge; it will be retried.",
                                    reservation.IntentId);
                            }
                        });
                }
                catch (OperationCanceledException) when (
                    stoppingToken.IsCancellationRequested)
                {
                    return;
                }

                if (!successfulReservations.IsEmpty)
                {
                    try
                    {
                        registry.CompleteRecoveredRuntimeDeletions(
                            successfulReservations);
                        foreach (var reservation in successfulReservations)
                        {
                            logger.LogInformation(
                                "Recovered pending runtime deletion intent {IntentId} for {RuntimeCount} runtime(s).",
                                reservation.IntentId,
                                reservation.RuntimeIds.Count);
                        }
                    }
                    catch (Exception ex)
                    {
                        logger.LogWarning(
                            ex,
                            "Recovered daemon mutations for {IntentCount} runtime deletion intent(s), but local batch persistence did not converge; they will be retried.",
                            successfulReservations.Count);
                    }
                }
            }
            finally
            {
                foreach (var reservation in reservations)
                {
                    reservation.Dispose();
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
