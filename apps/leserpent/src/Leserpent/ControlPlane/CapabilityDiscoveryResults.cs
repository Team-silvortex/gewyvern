namespace Leserpent.ControlPlane;

public sealed record CapabilityDiscoveryResult(
    string CapabilityEndpoint,
    IReadOnlyList<RuntimeCapability> Capabilities,
    string CapabilitySource,
    DateTimeOffset? CapabilityFetchedAt,
    string? CapabilityFetchError)
{
    public static CapabilityDiscoveryResult Succeeded(string capabilityEndpoint, IReadOnlyList<RuntimeCapability> capabilities) =>
        new(capabilityEndpoint, capabilities, "gewyvern-api", DateTimeOffset.UtcNow, null);

    public static CapabilityDiscoveryResult Failed(string capabilityEndpoint, string error) =>
        new(capabilityEndpoint, Array.Empty<RuntimeCapability>(), "fetch_failed", null, error);
}

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
            error,
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
            error,
            false,
            "fetch_failed",
            null,
            false,
            0,
            false,
            false,
            error));
}
