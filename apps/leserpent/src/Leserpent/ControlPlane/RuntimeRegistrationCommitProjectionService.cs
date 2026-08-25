namespace Leserpent.ControlPlane;

internal sealed class RuntimeRegistrationCompatibilityCommit(
    RuntimeRegistrationRequest request,
    DaemonRuntimeProjection runtime,
    CapabilityDiscoveryResult? capabilityDiscovery,
    RuntimeStatusDiscoveryResult? statusDiscovery,
    RuntimeSidecarDiscoveryResult? sidecarDiscovery)
{
    internal RuntimeRegistrationRequest Request { get; } = request;
    internal DaemonRuntimeProjection Runtime { get; } = runtime;
    internal CapabilityDiscoveryResult? CapabilityDiscovery { get; } = capabilityDiscovery;
    internal RuntimeStatusDiscoveryResult? StatusDiscovery { get; } = statusDiscovery;
    internal RuntimeSidecarDiscoveryResult? SidecarDiscovery { get; } = sidecarDiscovery;

    public override string ToString() =>
        $"RuntimeRegistrationCompatibilityCommit {{ RuntimeId = {Runtime.RuntimeId}, Revision = {Runtime.Revision}, HasRuntimeCredential = {!string.IsNullOrWhiteSpace(Request.PairingToken)}, HasSidecarCredential = {!string.IsNullOrWhiteSpace(Request.SidecarAdminToken)}, HasCapabilities = {CapabilityDiscovery is not null}, HasStatus = {StatusDiscovery is not null}, HasSidecarStatus = {SidecarDiscovery?.SidecarStatus is not null} }}";
}

internal sealed class RuntimeRegistrationCommitProjectionService
{
    internal RuntimeRegistrationCompatibilityCommit Bind(
        string expectedRuntimeId,
        RuntimeRegistrationRequest request,
        RuntimeRegistrationCommitReceipt receipt,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null)
    {
        if (!receipt.Applied || receipt.Runtime is null)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd registration omitted its authoritative receipt");
        }
        var runtime = receipt.Runtime;
        if (!string.Equals(receipt.RuntimeId, expectedRuntimeId, StringComparison.Ordinal)
            || !string.Equals(runtime.RuntimeId, expectedRuntimeId, StringComparison.Ordinal)
            || receipt.RegistrationRevision is null or 0
            || receipt.Revision != runtime.Revision)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd registration receipt is incoherent");
        }
        var requestedTags = request.Tags ?? new RuntimeTags(null, null, null);
        if (!string.Equals(runtime.Name, request.Name.Trim(), StringComparison.Ordinal)
            || !string.Equals(runtime.Endpoint, request.Endpoint.Trim(), StringComparison.Ordinal)
            || !string.Equals(
                runtime.SidecarEndpoint,
                NormalizeOptional(request.SidecarEndpoint),
                StringComparison.Ordinal)
            || !string.Equals(
                runtime.Tags.Environment,
                NormalizeOptional(requestedTags.Environment),
                StringComparison.Ordinal)
            || !string.Equals(
                runtime.Tags.Cluster,
                NormalizeOptional(requestedTags.Cluster),
                StringComparison.Ordinal)
            || !string.Equals(
                runtime.Tags.Role,
                NormalizeOptional(requestedTags.Role),
                StringComparison.Ordinal))
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd registration receipt does not match the requested identity");
        }

        var expectedDiscovery = capabilityDiscovery?.AuthoritySnapshot is not null
            || statusDiscovery is not null
            || sidecarDiscovery?.SidecarStatus is not null;
        if (receipt.DiscoveryApplied != expectedDiscovery
            || (receipt.DiscoveryApplied
                && runtime.Revision <= receipt.RegistrationRevision.Value)
            || (!receipt.DiscoveryApplied
                && runtime.Revision != receipt.RegistrationRevision.Value))
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd registration receipt does not match its discovery intake");
        }

        var committedCapabilities = capabilityDiscovery?.AuthoritySnapshot is null
            ? capabilityDiscovery
            : runtime.Capabilities is null
                ? throw new DaemonRuntimeRegistrationException(
                    "daemon_protocol_invalid",
                    "leserpentd registration receipt omitted committed capabilities")
                : capabilityDiscovery with
                {
                    Capabilities = RuntimeCapabilityProjection.ToLegacy(
                        runtime.Capabilities),
                    CapabilitySource = runtime.Capabilities.Source,
                    AuthoritySnapshot = runtime.Capabilities,
                };
        var committedStatus = statusDiscovery is null
            ? null
            : statusDiscovery with { Status = runtime.Status };
        var committedSidecar = sidecarDiscovery?.SidecarStatus is null
            ? sidecarDiscovery
            : sidecarDiscovery with
            {
                SidecarStatus = runtime.SidecarStatus
                    ?? throw new DaemonRuntimeRegistrationException(
                        "daemon_protocol_invalid",
                        "leserpentd registration receipt omitted committed sidecar status"),
            };
        var committedRequest = request with
        {
            Name = runtime.Name,
            Endpoint = runtime.Endpoint,
            SidecarEndpoint = runtime.SidecarEndpoint,
            Tags = runtime.Tags,
            Capabilities = runtime.Capabilities is null
                ? request.Capabilities
                : RuntimeCapabilityProjection.ToLegacy(runtime.Capabilities),
        };
        return new RuntimeRegistrationCompatibilityCommit(
            committedRequest,
            runtime,
            committedCapabilities,
            committedStatus,
            committedSidecar);
    }

    private static string? NormalizeOptional(string? value) =>
        string.IsNullOrWhiteSpace(value) ? null : value.Trim();
}
