namespace Leserpent.ControlPlane;

internal sealed class RuntimeCommandExecutionContext(
    RuntimeSummary runtime,
    ulong? authorityRevision,
    RuntimeControlAccess controlAccess,
    RuntimeSidecarAccess? sidecarAccess)
{
    internal RuntimeSummary Runtime { get; } = runtime;
    internal ulong? AuthorityRevision { get; } = authorityRevision;
    internal RuntimeControlAccess ControlAccess { get; } = controlAccess;
    internal RuntimeSidecarAccess? SidecarAccess { get; } = sidecarAccess;

    public override string ToString() =>
        $"RuntimeCommandExecutionContext {{ RuntimeId = {Runtime.RuntimeId}, AuthorityRevision = {AuthorityRevision?.ToString() ?? "managed"}, HasRuntimeCredential = {!string.IsNullOrWhiteSpace(ControlAccess.AdminToken)}, HasSidecarCredential = {!string.IsNullOrWhiteSpace(SidecarAccess?.SidecarAdminToken)} }}";
}

internal sealed class RuntimeCommandExecutionContextService(
    RuntimeReadProjectionService runtimeReads,
    RegistryService registry)
{
    internal async Task<RuntimeCommandExecutionContext?> InspectAsync(
        string runtimeId,
        CancellationToken cancellationToken)
    {
        var projection = await runtimeReads.InspectWithAuthorityAsync(
            runtimeId,
            cancellationToken);
        return projection is null ? null : Compose(projection);
    }

    internal async Task<IReadOnlyList<RuntimeCommandExecutionContext>> ListAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken) =>
        (await runtimeReads.ListWithAuthorityAsync(filter, cancellationToken))
            .Select(Compose)
            .ToArray();

    private RuntimeCommandExecutionContext Compose(
        RuntimeAuthorityProjection projection)
    {
        var runtime = projection.Runtime;
        var managedControl = registry.GetRuntimeControlAccess(runtime.RuntimeId)
            ?? throw new DaemonRuntimeProjectionException(
                "runtime_command_context_unmapped",
                $"runtime '{runtime.RuntimeId}' lost its managed credential metadata");
        var managedSidecar = registry.GetRuntimeSidecarAccess(runtime.RuntimeId);
        var control = new RuntimeControlAccess(
            runtime.RuntimeId,
            runtime.Name,
            runtime.Endpoint,
            managedControl.AdminToken,
            runtime.Tags);
        var sidecar = string.IsNullOrWhiteSpace(runtime.SidecarEndpoint)
            ? null
            : new RuntimeSidecarAccess(
                runtime.RuntimeId,
                runtime.Name,
                runtime.SidecarEndpoint,
                managedSidecar?.SidecarAdminToken,
                runtime.Tags);
        return new RuntimeCommandExecutionContext(
            runtime,
            projection.Revision,
            control,
            sidecar);
    }
}
