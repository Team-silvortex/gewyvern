namespace Leserpent.ControlPlane;

internal sealed class RuntimeRegistrationPlanProjectionService(
    RegistryService registry,
    IDaemonRuntimeProjectionReader daemon)
{
    internal async Task<RuntimeRegistrationPlan> BuildAsync(
        RuntimeRegistrationPlanRequest request,
        CancellationToken cancellationToken)
    {
        var recoveryPlan =
            registry.GetRuntimeRegistrationRecoveryPlan(request);
        if (recoveryPlan is not null)
        {
            return RejectIfDeleting(request, recoveryPlan);
        }
        if (!daemon.Enabled)
        {
            return RejectIfDeleting(
                request,
                registry.GetRuntimeRegistrationPlan(request));
        }

        var snapshot = await daemon.SnapshotAsync(cancellationToken);
        var plannedCreateRuntimeId = ResolvePlannedCreateRuntimeId(
            request,
            snapshot.Runtimes);
        var plan = RuntimeRegistrationPolicy.BuildAuthoritative(
            request,
            snapshot.Runtimes,
            plannedCreateRuntimeId);
        return RejectIfDeleting(request, plan);
    }

    private RuntimeRegistrationPlan RejectIfDeleting(
        RuntimeRegistrationPlanRequest request,
        RuntimeRegistrationPlan plan)
    {
        if (plan.PlannedRuntimeId is null ||
            !registry.IsRuntimeDeletionPending(plan.PlannedRuntimeId))
        {
            return plan;
        }
        return plan.AuthorityBound
            ? RuntimeRegistrationPolicy.RejectAuthoritative(
                request,
                plan,
                RuntimeRegistrationPolicy.RuntimeDeletionInProgressReason)
            : RuntimeRegistrationPolicy.Reject(
                request,
                plan,
                RuntimeRegistrationPolicy.RuntimeDeletionInProgressReason);
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
