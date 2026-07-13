namespace Leserpent.ControlPlane;

public sealed partial class CapabilityDiscoveryService
{
    private async Task<GewyvernRuntimeResiliencePayload?> TryDiscoverResilienceAsync(
        string endpoint,
        CancellationToken cancellationToken)
    {
        var resilienceUrl = BuildResilienceUrl(endpoint);
        var resiliencePlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(
            resilienceUrl,
            "runtime resilience endpoint",
            cancellationToken);
        if (resiliencePlanResult.Error is not null)
        {
            return null;
        }

        try
        {
            return await GetFromJsonAsync<GewyvernRuntimeResiliencePayload>(
                resiliencePlanResult.Plan!,
                cancellationToken);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch
        {
            return null;
        }
    }
}
