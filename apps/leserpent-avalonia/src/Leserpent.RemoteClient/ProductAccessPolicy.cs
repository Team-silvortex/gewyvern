public enum ProductCapability
{
    FleetTopology,
    LocalOrchestra,
    DaemonConnection,
    ReverseDeployment,
    DaemonRetirement,
    GewyvernProvisioning,
    GewyvernRetirement,
    RuntimeWorkspace,
    RuntimeMutation,
    RuntimeDebugger,
    LeselangAutomation,
    DiagnosticExport,
    LanguageManagement,
    LearningCenter,
    ReservedHostedSubscriptionService,
}

public enum ProductAccessTier
{
    OpenSourceCore,
    HostedSubscriptionService,
}

public sealed record ProductAccessDecision(
    ProductCapability Capability,
    ProductAccessTier Tier,
    bool Allowed,
    bool RequiresPayment,
    string? LicenseSpdx,
    string Reason);

public static class ProductAccessPolicy
{
    public const string OpenSourceLicenseSpdx = "MIT";
    public const string SubscriptionEntitlement = "leserpent.subscription.active";
    public static bool OpenSourceCoreRequiresPayment => false;
    public static bool OpenSourceCoreMayBeSubscriptionGated => false;

    private static readonly IReadOnlyList<ProductCapability> OpenSourceCore =
        Array.AsReadOnly<ProductCapability>(
        [
            ProductCapability.FleetTopology,
            ProductCapability.LocalOrchestra,
            ProductCapability.DaemonConnection,
            ProductCapability.ReverseDeployment,
            ProductCapability.DaemonRetirement,
            ProductCapability.GewyvernProvisioning,
            ProductCapability.GewyvernRetirement,
            ProductCapability.RuntimeWorkspace,
            ProductCapability.RuntimeMutation,
            ProductCapability.RuntimeDebugger,
            ProductCapability.LeselangAutomation,
            ProductCapability.DiagnosticExport,
            ProductCapability.LanguageManagement,
            ProductCapability.LearningCenter,
        ]);

    private static readonly IReadOnlyList<ProductCapability> HostedSubscriptionServices =
        Array.AsReadOnly<ProductCapability>(
        [
            ProductCapability.ReservedHostedSubscriptionService,
        ]);

    public static IReadOnlyList<ProductCapability> OpenSourceCoreCapabilities => OpenSourceCore;

    public static IReadOnlyList<ProductCapability> SubscriptionServiceExtensions =>
        HostedSubscriptionServices;

    public static ProductAccessTier Tier(ProductCapability capability) => capability switch
    {
        ProductCapability.FleetTopology
            or ProductCapability.LocalOrchestra
            or ProductCapability.DaemonConnection
            or ProductCapability.ReverseDeployment
            or ProductCapability.DaemonRetirement
            or ProductCapability.GewyvernProvisioning
            or ProductCapability.GewyvernRetirement
            or ProductCapability.RuntimeWorkspace
            or ProductCapability.RuntimeMutation
            or ProductCapability.RuntimeDebugger
            or ProductCapability.LeselangAutomation
            or ProductCapability.DiagnosticExport
            or ProductCapability.LanguageManagement
            or ProductCapability.LearningCenter => ProductAccessTier.OpenSourceCore,
        ProductCapability.ReservedHostedSubscriptionService =>
            ProductAccessTier.HostedSubscriptionService,
        _ => throw new ArgumentOutOfRangeException(
            nameof(capability),
            capability,
            "Unknown Leserpent product capability."),
    };

    public static ProductAccessDecision Evaluate(
        ProductCapability capability,
        bool accountAuthenticated,
        IReadOnlyCollection<string>? entitlements = null)
    {
        var tier = Tier(capability);
        if (tier == ProductAccessTier.OpenSourceCore)
        {
            return new ProductAccessDecision(
                capability,
                tier,
                true,
                OpenSourceCoreRequiresPayment,
                OpenSourceLicenseSpdx,
                "open-source-free-core");
        }
        if (!accountAuthenticated)
        {
            return new ProductAccessDecision(
                capability,
                tier,
                false,
                true,
                null,
                "account-session-required");
        }
        if (entitlements is null
            || !entitlements.Any(value => string.Equals(
                value,
                SubscriptionEntitlement,
                StringComparison.Ordinal)))
        {
            return new ProductAccessDecision(
                capability,
                tier,
                false,
                true,
                null,
                "subscription-entitlement-required");
        }
        return new ProductAccessDecision(
            capability,
            tier,
            true,
            true,
            null,
            "subscription-entitlement");
    }

    public static void RequireCompleteOpenSourceCore(
        IReadOnlyCollection<ProductCapability> capabilities)
    {
        ArgumentNullException.ThrowIfNull(capabilities);
        var unique = new HashSet<ProductCapability>();
        foreach (var capability in capabilities)
        {
            if (!unique.Add(capability)
                || Tier(capability) != ProductAccessTier.OpenSourceCore)
            {
                throw new InvalidDataException(
                    "The Leserpent open-source core capability set is invalid.");
            }
        }
        if (unique.Count != OpenSourceCore.Count
            || OpenSourceCore.Any(capability => !unique.Contains(capability)))
        {
            throw new InvalidDataException(
                "The Leserpent open-source core capability set is incomplete.");
        }
    }

    public static void VerifyContract()
    {
        RequireCompleteOpenSourceCore(OpenSourceCore);
        if (Enum.GetValues<ProductCapability>().Length
                != OpenSourceCore.Count + HostedSubscriptionServices.Count
            || HostedSubscriptionServices.Count != 1
            || HostedSubscriptionServices.Any(capability =>
                Tier(capability) != ProductAccessTier.HostedSubscriptionService)
            || !string.Equals(OpenSourceLicenseSpdx, "MIT", StringComparison.Ordinal)
            || OpenSourceCoreRequiresPayment
            || OpenSourceCoreMayBeSubscriptionGated)
        {
            throw new InvalidDataException(
                "The Leserpent product capability classification is not exhaustive.");
        }
        foreach (var capability in OpenSourceCore)
        {
            var signedOut = Evaluate(capability, accountAuthenticated: false);
            var subscribed = Evaluate(
                capability,
                accountAuthenticated: true,
                [SubscriptionEntitlement]);
            if (!signedOut.Allowed
                || signedOut.RequiresPayment
                || signedOut.LicenseSpdx != OpenSourceLicenseSpdx
                || !subscribed.Allowed
                || subscribed.RequiresPayment
                || subscribed.LicenseSpdx != OpenSourceLicenseSpdx)
            {
                throw new InvalidDataException(
                    "A Leserpent open-source core capability became paid or account-dependent.");
            }
        }

        var reserved = ProductCapability.ReservedHostedSubscriptionService;
        if (Evaluate(reserved, accountAuthenticated: false).Allowed
            || Evaluate(reserved, accountAuthenticated: true).Allowed
            || Evaluate(
                reserved,
                accountAuthenticated: true,
                [SubscriptionEntitlement.ToUpperInvariant()]).Allowed
            || !Evaluate(
                reserved,
                accountAuthenticated: true,
                [SubscriptionEntitlement]).Allowed)
        {
            throw new InvalidDataException(
                "The Leserpent subscription entitlement boundary drifted.");
        }

        try
        {
            _ = Tier((ProductCapability)int.MaxValue);
        }
        catch (ArgumentOutOfRangeException)
        {
            return;
        }
        throw new InvalidDataException(
            "The Leserpent product access policy accepted an unknown capability.");
    }
}
