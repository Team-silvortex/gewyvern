public sealed record RemoteMutationRevisionFence(
    string RuntimeId,
    ulong Revision,
    bool RequiresLaterCapabilityObservation);

public sealed record RemoteMutationObservationFence(
    string RuntimeId,
    ulong Revision,
    ulong SnapshotGeneration,
    bool RequiresCapabilityChange);

public static class RemoteMutationFences
{
    public static bool SatisfiesRevision(
        RemoteRuntimeProjection runtime,
        RemoteMutationRevisionFence fence) => runtime.Id == fence.RuntimeId
        && runtime.Revision >= fence.Revision
        && (!fence.RequiresLaterCapabilityObservation
            || runtime.Capabilities is { IsUnobserved: false }
                && runtime.CapabilitiesObservedForRevision is { } observedFor
                && observedFor >= fence.Revision);

    public static bool SatisfiesObservation(
        RemoteFeedState state,
        RemoteMutationObservationFence fence)
    {
        if (state.Phase != RemoteFeedPhase.Live
            || state.IsStale
            || state.SnapshotGeneration <= fence.SnapshotGeneration)
        {
            return false;
        }
        var runtime = state.Runtimes.FirstOrDefault(candidate =>
            candidate.Id == fence.RuntimeId);
        if (runtime is null || !fence.RequiresCapabilityChange)
        {
            return runtime is not null;
        }
        if (runtime.Revision == fence.Revision)
        {
            return true;
        }
        return runtime.Revision > fence.Revision
            && runtime.Capabilities is { IsUnobserved: false }
            && runtime.CapabilitiesObservedForRevision is { } observedFor
            && observedFor > fence.Revision;
    }

    public static void VerifyContract()
    {
        var runtime = new RemoteRuntimeProjection
        {
            Id = "runtime-a",
            Name = "Runtime A",
            Revision = 7,
            RefreshStatus = RefreshStatus.Ready,
            Tags = new RuntimeTags(),
            Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
        };
        var ordinary = new RemoteMutationRevisionFence("runtime-a", 7, false);
        var capability = new RemoteMutationRevisionFence("runtime-a", 7, true);
        if (!SatisfiesRevision(runtime, ordinary)
            || SatisfiesRevision(runtime, capability))
        {
            throw new InvalidDataException(
                "mutation fence did not distinguish command and observation revisions");
        }
        runtime.Revision = 8;
        if (SatisfiesRevision(runtime, capability))
        {
            throw new InvalidDataException(
                "mutation fence accepted an unobserved capability revision");
        }
        runtime.Capabilities = new RuntimeCapabilitySnapshot
        {
            Source = "gewyvern-api",
        };
        runtime.CapabilitiesObservedForRevision = 7;
        if (!SatisfiesRevision(runtime, capability))
        {
            throw new InvalidDataException(
                "mutation fence rejected a later observed capability revision");
        }

        runtime.Revision = 7;
        runtime.Capabilities = null;
        var ordinaryUnknown = new RemoteMutationObservationFence(
            "runtime-a",
            7,
            4,
            false);
        var capabilityUnknown = new RemoteMutationObservationFence(
            "runtime-a",
            7,
            4,
            true);
        var heartbeat = new RemoteFeedState(
            RemoteFeedPhase.Live,
            7,
            [runtime],
            0,
            false,
            "heartbeat",
            4);
        if (SatisfiesObservation(heartbeat, ordinaryUnknown))
        {
            throw new InvalidDataException(
                "mutation observation fence was released by a heartbeat");
        }
        var snapshot = heartbeat with { SnapshotGeneration = 5 };
        if (!SatisfiesObservation(snapshot, ordinaryUnknown)
            || !SatisfiesObservation(snapshot, capabilityUnknown))
        {
            throw new InvalidDataException(
                "mutation observation fence rejected an unchanged authoritative snapshot");
        }
        runtime.Revision = 8;
        if (SatisfiesObservation(snapshot, capabilityUnknown))
        {
            throw new InvalidDataException(
                "mutation observation fence accepted a pending capability projection");
        }
        runtime.Capabilities = new RuntimeCapabilitySnapshot
        {
            Source = "gewyvern-api",
        };
        runtime.CapabilitiesObservedForRevision = 8;
        runtime.Revision = 9;
        if (!SatisfiesObservation(snapshot, capabilityUnknown))
        {
            throw new InvalidDataException(
                "mutation observation fence rejected a changed capability snapshot");
        }
    }
}
