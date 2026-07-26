namespace Leserpent.ControlPlane;

public sealed class RuntimeDeletionReservation : IDisposable
{
    private RegistryService? owner;

    internal RuntimeDeletionReservation(
        RegistryService owner,
        string intentId,
        string claimId,
        IReadOnlyList<string> runtimeIds,
        string unregistrationCommandId,
        ulong? unregistrationReplayHorizonFloor = null,
        bool unregistrationMutationMayHaveStarted = false)
    {
        this.owner = owner;
        IntentId = intentId;
        ClaimId = claimId;
        RuntimeIds = runtimeIds;
        UnregistrationCommandId = unregistrationCommandId;
        UnregistrationReplayHorizonFloor =
            unregistrationReplayHorizonFloor;
        UnregistrationMutationMayHaveStarted =
            unregistrationMutationMayHaveStarted;
    }

    public string IntentId { get; }
    internal string ClaimId { get; }
    public IReadOnlyList<string> RuntimeIds { get; }
    public string UnregistrationCommandId { get; }
    public ulong? UnregistrationReplayHorizonFloor { get; private set; }
    public bool UnregistrationMutationMayHaveStarted { get; private set; }

    internal void MarkUnregistrationMutationFenced(ulong replayHorizonFloor)
    {
        UnregistrationReplayHorizonFloor = replayHorizonFloor;
        UnregistrationMutationMayHaveStarted = true;
    }

    public void Dispose()
    {
        Interlocked.Exchange(ref owner, null)?.ReleaseRuntimeDeletionClaim(IntentId, ClaimId);
    }
}

public sealed class RuntimeDeletionInProgressException(IReadOnlyCollection<string> runtimeIds)
    : InvalidOperationException("runtime deletion is already in progress")
{
    public IReadOnlyCollection<string> RuntimeIds { get; } = runtimeIds;
}
