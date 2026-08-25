namespace Leserpent.ControlPlane;

internal static class RuntimeRegistrationIntentPolicy
{
    internal static PersistedRuntimeRegistrationIntent Build(
        RuntimeRegistrationRequest request,
        RuntimeRegistrationPlan plan,
        string runtimeId,
        CapabilityDiscoveryResult? capabilityDiscovery,
        RuntimeStatusDiscoveryResult? statusDiscovery,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery,
        DateTimeOffset preparedAt)
    {
        var tags = NormalizeTags(request.Tags);
        var sidecarEndpoint = NormalizeOptional(request.SidecarEndpoint);
        var name = request.Name.Trim();
        var endpoint = request.Endpoint.Trim();
        return new PersistedRuntimeRegistrationIntent(
            RuntimeRegistrationCommandIdentity.ForIntent(
                runtimeId,
                name,
                endpoint,
                sidecarEndpoint,
                tags,
                plan.ExpectedRevision),
            runtimeId,
            plan.Action,
            plan.ExpectedRevision,
            name,
            endpoint,
            sidecarEndpoint,
            tags,
            NormalizeCapabilities(request.Capabilities),
            request.FetchCapabilities,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery,
            preparedAt);
    }

    internal static bool MatchesCommand(
        PersistedRuntimeRegistrationIntent intent,
        RuntimeRegistrationRequest request) =>
        string.Equals(
            intent.CommandId,
            RuntimeRegistrationCommandIdentity.ForIntent(
                intent.RuntimeId,
                request.Name,
                request.Endpoint,
                request.SidecarEndpoint,
                request.Tags,
                intent.ExpectedRevision),
            StringComparison.Ordinal);

    internal static bool CoordinatesMatch(
        PersistedRuntimeRegistrationIntent intent,
        RuntimeRegistrationPlanRequest request) =>
        string.Equals(
            intent.Name,
            request.Name.Trim(),
            StringComparison.Ordinal) &&
        string.Equals(
            intent.Endpoint,
            request.Endpoint.Trim(),
            StringComparison.Ordinal) &&
        string.Equals(
            intent.SidecarEndpoint,
            NormalizeOptional(request.SidecarEndpoint),
            StringComparison.Ordinal);

    internal static bool Overlaps(
        PersistedRuntimeRegistrationIntent intent,
        RuntimeRegistrationPlanRequest request) =>
        string.Equals(
            intent.Name,
            request.Name.Trim(),
            StringComparison.OrdinalIgnoreCase) ||
        RuntimeRegistrationPolicy.EndpointIdentityEquals(
            intent.Endpoint,
            request.Endpoint);

    internal static RuntimeRegistrationRequest RestoreRequest(
        PersistedRuntimeRegistrationIntent intent,
        RuntimeRegistrationRequest credentialSource,
        string reviewedPlanToken) =>
        credentialSource with
        {
            Name = intent.Name,
            Endpoint = intent.Endpoint,
            Capabilities = intent.ManualCapabilities,
            Tags = intent.Tags,
            FetchCapabilities = intent.FetchCapabilities,
            CapabilityEndpoint = null,
            StatusEndpoint = null,
            SidecarEndpoint = intent.SidecarEndpoint,
            SidecarStatusEndpoint = null,
            RegistrationPlanToken = reviewedPlanToken,
        };

    private static IReadOnlyList<RuntimeCapability> NormalizeCapabilities(
        IReadOnlyList<RuntimeCapability>? capabilities) =>
        (capabilities ?? Array.Empty<RuntimeCapability>())
            .Where(static capability =>
                !string.IsNullOrWhiteSpace(capability.Key))
            .Select(static capability => capability with
            {
                Key = capability.Key.Trim(),
                Support = NormalizeSupport(capability.Support),
                Description = capability.Description.Trim(),
            })
            .OrderBy(
                static capability => capability.Key,
                StringComparer.OrdinalIgnoreCase)
            .ToArray();

    private static RuntimeTags NormalizeTags(RuntimeTags? tags) =>
        new(
            NormalizeOptional(tags?.Environment),
            NormalizeOptional(tags?.Cluster),
            NormalizeOptional(tags?.Role));

    private static string? NormalizeOptional(string? value) =>
        string.IsNullOrWhiteSpace(value) ? null : value.Trim();

    private static string NormalizeSupport(string? support) =>
        support?.Trim().ToLowerInvariant() switch
        {
            "fully_supported" => "fully_supported",
            "risky" => "risky",
            _ => "not_supported",
        };
}
