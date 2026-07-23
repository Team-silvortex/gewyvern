namespace Leserpent.ControlPlane;

public sealed class RuntimeDeletionReservation : IDisposable
{
    private RegistryService? owner;

    internal RuntimeDeletionReservation(RegistryService owner, IReadOnlyList<string> runtimeIds)
    {
        this.owner = owner;
        RuntimeIds = runtimeIds;
    }

    public IReadOnlyList<string> RuntimeIds { get; }

    public void Dispose()
    {
        Interlocked.Exchange(ref owner, null)?.ReleaseRuntimeDeletion(RuntimeIds);
    }
}

public sealed class RuntimeDeletionInProgressException(IReadOnlyCollection<string> runtimeIds)
    : InvalidOperationException("runtime deletion is already in progress")
{
    public IReadOnlyCollection<string> RuntimeIds { get; } = runtimeIds;
}
