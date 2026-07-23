namespace Leserpent.ControlPlane;

public sealed class RuntimeDeletionReservation : IDisposable
{
    private RegistryService? owner;

    internal RuntimeDeletionReservation(
        RegistryService owner,
        string intentId,
        string claimId,
        IReadOnlyList<string> runtimeIds)
    {
        this.owner = owner;
        IntentId = intentId;
        ClaimId = claimId;
        RuntimeIds = runtimeIds;
    }

    public string IntentId { get; }
    internal string ClaimId { get; }
    public IReadOnlyList<string> RuntimeIds { get; }

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
