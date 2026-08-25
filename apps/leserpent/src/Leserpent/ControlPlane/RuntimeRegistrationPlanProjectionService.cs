namespace Leserpent.ControlPlane;

internal sealed class RuntimeRegistrationPlanProjectionService(
    RegistryService registry,
    IDaemonRuntimeProjectionReader daemon)
{
    internal async Task<RuntimeRegistrationPlan> BuildAsync(
        RuntimeRegistrationPlanRequest request,
        CancellationToken cancellationToken)
    {
        if (!daemon.Enabled)
        {
            return registry.GetRuntimeRegistrationPlan(request);
        }

        var snapshot = await daemon.SnapshotAsync(cancellationToken);
        var plannedCreateRuntimeId = ResolvePlannedCreateRuntimeId(
            request,
            snapshot.Runtimes);
        var plan = RuntimeRegistrationPolicy.BuildAuthoritative(
            request,
            snapshot.Runtimes,
            plannedCreateRuntimeId);
        return plan.PlannedRuntimeId is not null
            && registry.IsRuntimeDeletionPending(plan.PlannedRuntimeId)
                ? RuntimeRegistrationPolicy.RejectAuthoritative(
                    request,
                    plan,
                    RuntimeRegistrationPolicy.RuntimeDeletionInProgressReason)
                : plan;
    }

    private string ResolvePlannedCreateRuntimeId(
        RuntimeRegistrationPlanRequest request,
        IReadOnlyList<DaemonRuntimeProjection> authoritativeRuntimes)
    {
        var managedRuntime = registry.ListRuntimes().FirstOrDefault(runtime =>
            string.Equals(
                runtime.Name,
                request.Name.Trim(),
                StringComparison.OrdinalIgnoreCase));
        if (managedRuntime is not null
            && !authoritativeRuntimes.Any(runtime =>
                string.Equals(
                    runtime.RuntimeId,
                    managedRuntime.RuntimeId,
                    StringComparison.OrdinalIgnoreCase)))
        {
            return managedRuntime.RuntimeId;
        }
        return RuntimeRegistrationPolicy.BuildProposedRuntimeId(
            request.Name,
            request.Endpoint);
    }
}
