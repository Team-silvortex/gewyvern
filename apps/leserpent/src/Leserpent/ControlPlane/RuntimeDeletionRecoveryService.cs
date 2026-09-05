using System.Collections.Concurrent;

namespace Leserpent.ControlPlane;

public sealed class RuntimeDeletionRecoveryService(
    RegistryService registry,
    IRuntimeRegistrationAuthority registrationAuthority,
    ILogger<RuntimeDeletionRecoveryService> logger,
    RuntimeDeletionRecoverySignal? recoverySignal = null,
    ControlPlaneWriterFence? writerFence = null,
    TimeProvider? timeProvider = null) : BackgroundService
{
    private const int MaxRecoveryBatchSize = 32;
    private const int MaxConcurrentAuthorityMutations = 8;
    private static readonly TimeSpan IdleDelay = TimeSpan.FromSeconds(5);
    private static readonly TimeSpan PersistenceFailureDelay =
        TimeSpan.FromSeconds(1);
    private static readonly TimeSpan ReadyDelay = TimeSpan.FromMilliseconds(25);
    private readonly RuntimeDeletionRecoverySignal recoverySignal =
        recoverySignal ?? new RuntimeDeletionRecoverySignal();
    private readonly TimeProvider timeProvider = timeProvider ?? TimeProvider.System;

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        if (writerFence is not null && !writerFence.IsWriter)
        {
            logger.LogInformation(
                "Runtime deletion recovery is idle because this host is a control-plane standby.");
            await WaitForShutdown(stoppingToken);
            return;
        }

        while (!stoppingToken.IsCancellationRequested)
        {
            if (writerFence is not null && !writerFence.IsWriter)
            {
                logger.LogWarning(
                    "Control-plane writer ownership was lost; runtime deletion recovery is stopping.");
                return;
            }
            var reservations = registry.ClaimPendingRuntimeDeletions(
                MaxRecoveryBatchSize,
                timeProvider.GetUtcNow());
            var successfulReservations =
                new ConcurrentBag<RuntimeDeletionReservation>();
            var failedReservations =
                new ConcurrentBag<RuntimeDeletionFailure>();
            var persistenceFailed = false;
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
                                await RuntimeDeletionAuthorityWorkflow
                                    .ExecuteAsync(
                                        registry,
                                        reservation,
                                        registrationAuthority,
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
                                failedReservations.Add(new RuntimeDeletionFailure(
                                    reservation,
                                    ClassifyFailure(ex),
                                    timeProvider.GetUtcNow()));
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

                if (!failedReservations.IsEmpty)
                {
                    try
                    {
                        registry.RecordRuntimeDeletionFailures(
                            failedReservations);
                    }
                    catch (Exception ex)
                    {
                        persistenceFailed = true;
                        logger.LogWarning(
                            ex,
                            "Runtime deletion retry metadata for {IntentCount} intent(s) did not persist; the claims will be released without advancing their retry schedule.",
                            failedReservations.Count);
                    }
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
                        persistenceFailed = true;
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
                await recoverySignal.WaitAsync(
                    persistenceFailed
                        ? PersistenceFailureDelay
                        : GetNextDelay(),
                    stoppingToken);
            }
            catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested)
            {
                return;
            }
        }
    }

    private static async Task WaitForShutdown(
        CancellationToken stoppingToken)
    {
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
    }

    private TimeSpan GetNextDelay()
    {
        var pending = registry.ListPendingRuntimeDeletions();
        if (pending.Count == 0)
        {
            return IdleDelay;
        }

        var now = timeProvider.GetUtcNow();
        var nextAttemptAt = pending
            .Select(static intent => intent.NextAttemptAt)
            .Where(static value => value is not null)
            .Min();
        if (pending.Any(intent =>
                intent.NextAttemptAt is null ||
                intent.NextAttemptAt <= now))
        {
            return ReadyDelay;
        }
        if (nextAttemptAt is null)
        {
            return ReadyDelay;
        }

        var untilNextAttempt = nextAttemptAt.Value - now;
        return untilNextAttempt < IdleDelay
            ? TimeSpan.FromMilliseconds(Math.Max(
                ReadyDelay.TotalMilliseconds,
                untilNextAttempt.TotalMilliseconds))
            : IdleDelay;
    }

    private static string ClassifyFailure(Exception error) =>
        error switch
        {
            DaemonRuntimeRegistrationException daemonError
                when daemonError.Code.Contains(
                    "timeout",
                    StringComparison.OrdinalIgnoreCase) =>
                RuntimeDeletionFailureCodes.AuthorityTimeout,
            TimeoutException =>
                RuntimeDeletionFailureCodes.AuthorityTimeout,
            IOException or System.Net.Sockets.SocketException =>
                RuntimeDeletionFailureCodes.AuthorityUnavailable,
            UnauthorizedAccessException =>
                RuntimeDeletionFailureCodes.AuthorityRejected,
            RuntimeUnregistrationReplayAmbiguousException =>
                RuntimeDeletionFailureCodes.ReplayAmbiguous,
            _ => RuntimeDeletionFailureCodes.AuthorityFailure,
        };
}

internal sealed record RuntimeDeletionFailure(
    RuntimeDeletionReservation Reservation,
    string FailureCode,
    DateTimeOffset AttemptedAt);

internal static class RuntimeDeletionFailureCodes
{
    public const string AuthorityFailure = "authority_failure";
    public const string AuthorityRejected = "authority_rejected";
    public const string AuthorityTimeout = "authority_timeout";
    public const string AuthorityUnavailable = "authority_unavailable";
    public const string ReplayAmbiguous = "replay_ambiguous";

    public static bool IsValid(string? code) =>
        code is AuthorityFailure or AuthorityRejected
            or AuthorityTimeout or AuthorityUnavailable
            or ReplayAmbiguous;
}
