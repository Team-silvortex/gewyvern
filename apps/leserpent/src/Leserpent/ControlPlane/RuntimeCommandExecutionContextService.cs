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

internal sealed class RuntimeDiscoveryCommit(
    RuntimeCommandExecutionContext context,
    CapabilityDiscoveryResult? capabilityDiscovery,
    RuntimeStatusDiscoveryResult? statusDiscovery,
    RuntimeSidecarDiscoveryResult? sidecarDiscovery)
{
    internal RuntimeCommandExecutionContext Context { get; } = context;
    internal CapabilityDiscoveryResult? CapabilityDiscovery { get; } = capabilityDiscovery;
    internal RuntimeStatusDiscoveryResult? StatusDiscovery { get; } = statusDiscovery;
    internal RuntimeSidecarDiscoveryResult? SidecarDiscovery { get; } = sidecarDiscovery;

    public override string ToString() =>
        $"RuntimeDiscoveryCommit {{ RuntimeId = {Context.Runtime.RuntimeId}, AuthorityRevision = {Context.AuthorityRevision?.ToString() ?? "managed"}, HasCapabilities = {CapabilityDiscovery is not null}, HasStatus = {StatusDiscovery is not null}, HasSidecarStatus = {SidecarDiscovery?.SidecarStatus is not null} }}";
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

    internal async Task<RuntimeDiscoveryCommit> CommitDiscoveryAsync(
        RuntimeCommandExecutionContext context,
        IRuntimeRegistrationAuthority registrationAuthority,
        CancellationToken cancellationToken,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null)
    {
        var receipt = await registrationAuthority.SubmitDiscoveryAtRevisionAsync(
            context.Runtime.RuntimeId,
            context.AuthorityRevision,
            cancellationToken,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery);
        return BindDiscoveryReceipt(
            context,
            receipt,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery);
    }

    internal RuntimeDiscoveryCommit BindDiscoveryReceipt(
        RuntimeCommandExecutionContext context,
        RuntimeDiscoveryIntakeReceipt receipt,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null)
    {
        if (!string.Equals(
                context.Runtime.RuntimeId,
                receipt.RuntimeId,
                StringComparison.Ordinal))
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd discovery receipt references another runtime");
        }
        if (!receipt.Applied)
        {
            var expectedAuthoritativeCommit = context.AuthorityRevision is not null
                && (capabilityDiscovery?.AuthoritySnapshot is not null
                    || statusDiscovery is not null
                    || sidecarDiscovery?.SidecarStatus is not null);
            if (expectedAuthoritativeCommit)
            {
                throw new DaemonRuntimeRegistrationException(
                    "daemon_protocol_invalid",
                    "leserpentd discovery intake omitted its authoritative receipt");
            }
            return new RuntimeDiscoveryCommit(
                context,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);
        }

        var authoritative = receipt.Runtime
            ?? throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd discovery receipt omitted its runtime projection");
        if (authoritative.Revision == 0
            || receipt.Revision != authoritative.Revision
            || (context.AuthorityRevision is { } previousRevision
                && authoritative.Revision <= previousRevision))
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd discovery receipt has an incoherent runtime revision");
        }

        var runtime = RuntimeReadProjectionService.Merge(
            authoritative,
            context.Runtime);
        var committedContext = Compose(new RuntimeAuthorityProjection(
            runtime,
            authoritative.Revision));
        var committedCapabilities = capabilityDiscovery?.AuthoritySnapshot is null
            ? capabilityDiscovery
            : authoritative.Capabilities is null
                ? throw new DaemonRuntimeRegistrationException(
                    "daemon_protocol_invalid",
                    "leserpentd discovery receipt omitted committed capabilities")
                : capabilityDiscovery with
                {
                    Capabilities = RuntimeCapabilityProjection.ToLegacy(
                        authoritative.Capabilities),
                    CapabilitySource = authoritative.Capabilities.Source,
                    AuthoritySnapshot = authoritative.Capabilities,
                };
        var committedStatus = statusDiscovery is null
            ? null
            : statusDiscovery with { Status = authoritative.Status };
        var committedSidecar = sidecarDiscovery?.SidecarStatus is null
            ? sidecarDiscovery
            : sidecarDiscovery with
            {
                SidecarStatus = authoritative.SidecarStatus
                    ?? throw new DaemonRuntimeRegistrationException(
                        "daemon_protocol_invalid",
                        "leserpentd discovery receipt omitted committed sidecar status"),
            };
        return new RuntimeDiscoveryCommit(
            committedContext,
            committedCapabilities,
            committedStatus,
            committedSidecar);
    }

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
