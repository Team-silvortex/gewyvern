namespace Leserpent.ControlPlane;

internal static class RuntimeDiagnosticCodes
{
    internal const string CapabilityFetchFailed =
        "capability_fetch_failed";
    internal const string RuntimeStatusFetchFailed =
        "runtime_status_fetch_failed";
    internal const string SidecarFetchFailed =
        "sidecar_fetch_failed";
    internal const string SidecarReportedError =
        "sidecar_reported_error";
    internal const string SidecarMemoryFetchFailed =
        "sidecar_memory_fetch_failed";
}

public sealed record CapabilityDiscoveryResult(
    string CapabilityEndpoint,
    IReadOnlyList<RuntimeCapability> Capabilities,
    string CapabilitySource,
    DateTimeOffset? CapabilityFetchedAt,
    string? CapabilityFetchError,
    RuntimeCapabilityAuthoritySnapshot? AuthoritySnapshot)
{
    public static CapabilityDiscoveryResult Succeeded(
        string capabilityEndpoint,
        IReadOnlyList<RuntimeCapability> capabilities,
        RuntimeCapabilityAuthoritySnapshot? authoritySnapshot = null) =>
        new(capabilityEndpoint, capabilities, "gewyvern-api", DateTimeOffset.UtcNow, null, authoritySnapshot);

    public static CapabilityDiscoveryResult Failed(string capabilityEndpoint, string error) =>
        new(
            capabilityEndpoint,
            Array.Empty<RuntimeCapability>(),
            "fetch_failed",
            null,
            RuntimeDiagnosticCodes.CapabilityFetchFailed,
            null);
}

public sealed record RuntimeCapabilityAuthoritySnapshot(
    string Source,
    string Service,
    string Version,
    bool LatestSnapshot,
    bool AuthenticatedDeployment,
    bool ServeRequired,
    bool ExternalSidecarContext,
    string TargetPathSegmentEncoding,
    string TargetDirectPathChars,
    IReadOnlyList<string> Endpoints,
    IReadOnlyDictionary<string, bool> Extensions);

public sealed record RuntimeStatusDiscoveryResult(
    string StatusEndpoint,
    RuntimeStatusSnapshot Status)
{
    public static RuntimeStatusDiscoveryResult Succeeded(string statusEndpoint, RuntimeStatusSnapshot status) =>
        new(statusEndpoint, status);

    public static RuntimeStatusDiscoveryResult Failed(string statusEndpoint, string error) =>
        new(statusEndpoint, new RuntimeStatusSnapshot(
            "fetch_failed",
            null,
            RuntimeDiagnosticCodes.RuntimeStatusFetchFailed,
            false,
            null,
            null,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false));
}

public sealed record RuntimeSidecarDiscoveryResult(
    string StatusEndpoint,
    RuntimeSidecarStatusSnapshot? SidecarStatus)
{
    public static RuntimeSidecarDiscoveryResult Succeeded(string statusEndpoint, RuntimeSidecarStatusSnapshot status) =>
        new(statusEndpoint, status);

    public static RuntimeSidecarDiscoveryResult Failed(string statusEndpoint, string error) =>
        new(statusEndpoint, new RuntimeSidecarStatusSnapshot(
            "fetch_failed",
            null,
            RuntimeDiagnosticCodes.SidecarFetchFailed,
            false,
            "fetch_failed",
            null,
            false,
            0,
            false,
            false,
            RuntimeDiagnosticCodes.SidecarFetchFailed));
}
