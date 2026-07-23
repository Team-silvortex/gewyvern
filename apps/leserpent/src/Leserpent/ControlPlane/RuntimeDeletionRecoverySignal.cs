namespace Leserpent.ControlPlane;

public sealed class RuntimeDeletionRecoverySignal
{
    private readonly SemaphoreSlim signal = new(0, 1);

    public void Pulse()
    {
        try
        {
            signal.Release();
        }
        catch (SemaphoreFullException)
        {
        }
    }

    public async Task WaitAsync(
        TimeSpan timeout,
        CancellationToken cancellationToken)
    {
        _ = await signal.WaitAsync(timeout, cancellationToken);
    }
}
