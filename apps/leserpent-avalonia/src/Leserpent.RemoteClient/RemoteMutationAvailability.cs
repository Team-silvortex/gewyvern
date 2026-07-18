public sealed record RemoteMutationAvailability(
    bool MutationsEnabled,
    string? MutationUnavailableReason,
    bool InspectEnabled,
    string? InspectUnavailableReason);

public static class RemoteMutationAvailabilityPolicy
{
    public static RemoteMutationAvailability Evaluate(
        RemoteFeedState state,
        bool mutationInFlight,
        RemoteMutationRevisionFence? revisionFence,
        RemoteMutationObservationFence? observationFence)
    {
        var live = state.Phase == RemoteFeedPhase.Live && !state.IsStale;
        var mutationReason = mutationInFlight
            ? "A remote change is awaiting confirmation or completion"
            : revisionFence is not null
                ? revisionFence.RequiresLaterCapabilityObservation
                    ? $"Waiting for a capability observation after revision {revisionFence.Revision}"
                    : $"Waiting for event revision {revisionFence.Revision} before another remote change"
                : observationFence is not null
                    ? "Waiting for an authoritative snapshot after an unknown remote outcome"
                    : live
                        ? null
                        : "Remote changes are unavailable while the event stream is not live";
        var inspectReason = live
            ? null
            : "Runtime inspection requires a live event stream";
        return new RemoteMutationAvailability(
            mutationReason is null,
            mutationReason,
            live,
            inspectReason);
    }

    public static void VerifyContract()
    {
        var live = new RemoteFeedState(
            RemoteFeedPhase.Live,
            7,
            Array.Empty<RemoteRuntimeProjection>(),
            0,
            false,
            "live",
            3);
        Require(
            Evaluate(live, false, null, null),
            mutationsEnabled: true,
            inspectEnabled: true,
            expectedMutationReason: null,
            "live state");
        Require(
            Evaluate(
                live,
                true,
                new RemoteMutationRevisionFence("runtime-a", 8, false),
                new RemoteMutationObservationFence("runtime-a", 7, 3, false)),
            mutationsEnabled: false,
            inspectEnabled: true,
            expectedMutationReason: "A remote change is awaiting confirmation or completion",
            "in-flight precedence");
        Require(
            Evaluate(
                live,
                false,
                new RemoteMutationRevisionFence("runtime-a", 8, false),
                null),
            mutationsEnabled: false,
            inspectEnabled: true,
            expectedMutationReason: "Waiting for event revision 8 before another remote change",
            "revision fence");
        Require(
            Evaluate(
                live,
                false,
                new RemoteMutationRevisionFence("runtime-a", 8, true),
                null),
            mutationsEnabled: false,
            inspectEnabled: true,
            expectedMutationReason: "Waiting for a capability observation after revision 8",
            "capability fence");
        Require(
            Evaluate(
                live,
                false,
                null,
                new RemoteMutationObservationFence("runtime-a", 7, 3, false)),
            mutationsEnabled: false,
            inspectEnabled: true,
            expectedMutationReason: "Waiting for an authoritative snapshot after an unknown remote outcome",
            "unknown outcome fence");

        var stale = live with { Phase = RemoteFeedPhase.Stale, IsStale = true };
        var unavailable = Evaluate(stale, false, null, null);
        if (unavailable.MutationsEnabled
            || unavailable.InspectEnabled
            || unavailable.MutationUnavailableReason
                != "Remote changes are unavailable while the event stream is not live"
            || unavailable.InspectUnavailableReason
                != "Runtime inspection requires a live event stream")
        {
            throw new InvalidDataException(
                "stale remote state did not disable mutation and inspection consistently");
        }
    }

    private static void Require(
        RemoteMutationAvailability availability,
        bool mutationsEnabled,
        bool inspectEnabled,
        string? expectedMutationReason,
        string caseName)
    {
        if (availability.MutationsEnabled != mutationsEnabled
            || availability.InspectEnabled != inspectEnabled
            || availability.MutationUnavailableReason != expectedMutationReason
            || availability.InspectUnavailableReason is not null)
        {
            throw new InvalidDataException(
                $"remote mutation availability drifted for {caseName}");
        }
    }
}
