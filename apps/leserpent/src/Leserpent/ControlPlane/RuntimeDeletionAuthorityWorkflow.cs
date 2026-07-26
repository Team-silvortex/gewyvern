namespace Leserpent.ControlPlane;

internal static class RuntimeDeletionAuthorityWorkflow
{
    public static async Task ExecuteAsync(
        RegistryService registry,
        RuntimeDeletionReservation reservation,
        IRuntimeRegistrationAuthority authority,
        CancellationToken cancellationToken)
    {
        var lookup = await authority.LookupUnregistrationReceiptAsync(
            reservation.UnregistrationCommandId,
            cancellationToken);
        if (!string.Equals(
                lookup.CommandId,
                reservation.UnregistrationCommandId,
                StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "runtime unregistration receipt changed the command identity");
        }
        if (lookup.Found)
        {
            if (reservation.UnregistrationReplayHorizonFloor is { } floor &&
                lookup.OperationGeneration < floor)
            {
                throw new InvalidDataException(
                    "runtime unregistration receipt predates the persisted replay floor");
            }
            if (!lookup.RuntimeIds!
                .ToHashSet(StringComparer.OrdinalIgnoreCase)
                .SetEquals(reservation.RuntimeIds))
            {
                throw new InvalidDataException(
                    "runtime unregistration receipt targets do not match the deletion intent");
            }
            return;
        }

        var horizon = lookup.ReplayHorizon;
        if (horizon is not null)
        {
            if (reservation.UnregistrationMutationMayHaveStarted)
            {
                var floor =
                    reservation.UnregistrationReplayHorizonFloor;
                if (floor is null ||
                    horizon.NextGeneration < floor.Value ||
                    horizon.EvictedThroughGeneration >= floor.Value)
                {
                    throw new RuntimeUnregistrationReplayAmbiguousException();
                }
            }
            else
            {
                registry.FenceRuntimeDeletionMutation(
                    reservation,
                    horizon.NextGeneration);
            }
        }

        await authority.UnregisterAsync(
            reservation.RuntimeIds,
            reservation.UnregistrationCommandId,
            cancellationToken);
    }
}

internal sealed class RuntimeUnregistrationReplayAmbiguousException :
    InvalidOperationException
{
    public RuntimeUnregistrationReplayAmbiguousException()
        : base(
            "runtime unregistration receipt may have been evicted; automatic replay was rejected")
    {
    }
}
