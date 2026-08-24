namespace Leserpent.ControlPlane;

internal sealed record RuntimeCleanupSelection(
    IReadOnlyList<string> RuntimeIds,
    IReadOnlyList<string> SessionIds);

public sealed class RuntimeCleanupProjectionService(
    RuntimeReadProjectionService runtimeReads,
    RegistryService registry)
{
    public async Task<RuntimeCleanupPlan> ReadAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken)
    {
        var runtimes = await runtimeReads.ListAsync(filter, cancellationToken);
        return RuntimeCleanupPolicy.Build(filter, runtimes, registry.ListSessions());
    }

    internal async Task<RuntimeCleanupSelection> SelectAsync(
        string kind,
        RuntimeListFilter filter,
        RuntimeCleanupRequest request,
        CancellationToken cancellationToken)
    {
        var runtimes = await runtimeReads.ListAsync(filter, cancellationToken);
        var sessions = registry.ListSessions();
        var plan = RuntimeCleanupPolicy.Build(filter, runtimes, sessions);
        var action = RuntimeCleanupPolicy.RequireMatchingAction(plan, kind, request);
        var runtimeIds = action.Targets
            .Select(target => target.RuntimeId)
            .ToArray();
        return new RuntimeCleanupSelection(
            runtimeIds,
            RuntimeCleanupPolicy.GetAffectedSessionIds(runtimeIds, sessions));
    }
}
