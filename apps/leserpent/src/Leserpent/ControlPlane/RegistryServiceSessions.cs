namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    private static List<CapabilityRejection> EvaluateRequirements(
        IReadOnlyList<RuntimeCapability> capabilities,
        IReadOnlyList<SessionCapabilityRequirement> requirements)
    {
        var capabilityMap = capabilities.ToDictionary(
            capability => capability.Key,
            capability => capability,
            StringComparer.OrdinalIgnoreCase);
        var rejections = new List<CapabilityRejection>();

        foreach (var requirement in requirements)
        {
            if (!capabilityMap.TryGetValue(requirement.Key, out var capability))
            {
                rejections.Add(new CapabilityRejection(
                    requirement.Key,
                    "not_supported",
                    "runtime did not advertise this capability"));
                continue;
            }

            if (string.Equals(capability.Support, "not_supported", StringComparison.OrdinalIgnoreCase))
            {
                rejections.Add(new CapabilityRejection(
                    requirement.Key,
                    capability.Support,
                    "runtime explicitly marked this capability as unavailable"));
                continue;
            }

            if (string.Equals(capability.Support, "risky", StringComparison.OrdinalIgnoreCase))
            {
                rejections.Add(new CapabilityRejection(
                    requirement.Key,
                    capability.Support,
                    "runtime marked this capability as risky; leserpent should not remote-execute it"));
                continue;
            }
        }

        return rejections;
    }
}
