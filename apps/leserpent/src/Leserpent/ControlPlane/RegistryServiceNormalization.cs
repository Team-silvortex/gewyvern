namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    private static IReadOnlyList<RuntimeCapability> NormalizeCapabilities(
        IReadOnlyList<RuntimeCapability>? capabilities) =>
        (capabilities ?? Array.Empty<RuntimeCapability>())
            .Where(capability => !string.IsNullOrWhiteSpace(capability.Key))
            .Select(capability => capability with
            {
                Key = capability.Key.Trim(),
                Support = NormalizeSupport(capability.Support),
                Description = capability.Description.Trim(),
            })
            .OrderBy(capability => capability.Key, StringComparer.OrdinalIgnoreCase)
            .ToArray();

    private static IReadOnlyList<SessionCapabilityRequirement> NormalizeRequirements(
        IReadOnlyList<SessionCapabilityRequirement>? requirements) =>
        (requirements ?? Array.Empty<SessionCapabilityRequirement>())
            .Where(requirement => !string.IsNullOrWhiteSpace(requirement.Key))
            .Select(requirement => requirement with
            {
                Key = requirement.Key.Trim(),
                MinimumSupport = NormalizeSupport(requirement.MinimumSupport),
            })
            .OrderBy(requirement => requirement.Key, StringComparer.OrdinalIgnoreCase)
            .ToArray();

    private static RuntimeTags NormalizeTags(RuntimeTags? tags) =>
        new(
            NormalizeTagValue(tags?.Environment),
            NormalizeTagValue(tags?.Cluster),
            NormalizeTagValue(tags?.Role));

    private static string? NormalizeTagValue(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }

        return value.Trim();
    }

    private static bool MatchesFilter(RuntimeRecord runtime, RuntimeListFilter? filter)
    {
        if (filter is null)
        {
            return true;
        }

        return MatchesTag(runtime.Tags.Environment, filter.Environment)
            && MatchesTag(runtime.Tags.Cluster, filter.Cluster)
            && MatchesTag(runtime.Tags.Role, filter.Role);
    }

    private static bool MatchesTag(string? actual, string? expected)
    {
        if (string.IsNullOrWhiteSpace(expected))
        {
            return true;
        }

        if (string.IsNullOrWhiteSpace(actual))
        {
            return false;
        }

        return string.Equals(actual, expected.Trim(), StringComparison.OrdinalIgnoreCase);
    }

    private static IReadOnlyDictionary<string, int> BuildTagCounts<T>(
        IEnumerable<T> runtimes,
        Func<T, string?> selector) =>
        runtimes
            .Select(selector)
            .Where(value => !string.IsNullOrWhiteSpace(value))
            .GroupBy(value => value!, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => group.Count(), StringComparer.OrdinalIgnoreCase);

    private static string NormalizeSupport(string support)
    {
        if (string.IsNullOrWhiteSpace(support))
        {
            return "not_supported";
        }

        return support.Trim().ToLowerInvariant() switch
        {
            "fully_supported" => "fully_supported",
            "risky" => "risky",
            _ => "not_supported",
        };
    }

    private static string? NormalizeOptionalEndpoint(string? endpoint)
    {
        if (string.IsNullOrWhiteSpace(endpoint))
        {
            return null;
        }

        return endpoint.Trim();
    }

    private static string? NormalizeOptionalSecret(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }

        return value.Trim();
    }
}
