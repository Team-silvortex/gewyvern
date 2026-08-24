namespace Leserpent.ControlPlane;

public sealed class FleetReadProjectionService(
    RuntimeReadProjectionService runtimeReads,
    RegistryService registry)
{
    public async Task<FleetSummary> GetSummaryAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken)
    {
        var runtimes = await runtimeReads.ListAsync(filter, cancellationToken);
        return RegistryService.ProjectFleetSummary(runtimes);
    }

    public async Task<IReadOnlyList<RuntimeAttentionItem>> GetRuntimesNeedingAttentionAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken)
    {
        var runtimes = await runtimeReads.ListAsync(filter, cancellationToken);
        return registry.ProjectRuntimesNeedingAttention(runtimes);
    }

    public async Task<FleetAttentionSummary> GetAttentionSummaryAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken)
    {
        var runtimes = await runtimeReads.ListAsync(filter, cancellationToken);
        var attention = registry.ProjectRuntimesNeedingAttention(runtimes);
        return RegistryService.ProjectFleetAttentionSummary(attention);
    }
}
