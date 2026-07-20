internal sealed record RemoteCredentialPresentation(
    string Label,
    string AutomationName,
    string Description,
    bool IsEnvironmentFallback)
{
    public static RemoteCredentialPresentation Create(RemoteTokenSource source)
    {
        if (source == RemoteTokenSource.LocalProcess)
        {
            return new RemoteCredentialPresentation(
                "TOKEN / LOCAL PROCESS",
                "Remote credential source: local process",
                "The credential is ephemeral and scoped to the local Leserpent service process.",
                false);
        }

        var platformName = OperatingSystem.IsMacOS()
            ? "KEYCHAIN"
            : OperatingSystem.IsLinux()
                ? "SECRET SERVICE"
                : "PLATFORM STORE";
        return source == RemoteTokenSource.Environment
            ? new RemoteCredentialPresentation(
                "TOKEN / ENV FALLBACK",
                "Remote credential source: environment fallback",
                "Remote token comes from LESERPENT_REMOTE_TOKEN. Store an endpoint-scoped token in the platform credential store for interactive use.",
                true)
            : new RemoteCredentialPresentation(
                $"TOKEN / {platformName}",
                $"Remote credential source: {platformName.ToLowerInvariant()}",
                $"Remote token comes from {platformName.ToLowerInvariant()}.",
                false);
    }

    public static void VerifyContract()
    {
        var platform = Create(RemoteTokenSource.PlatformStore);
        var fallback = Create(RemoteTokenSource.Environment);
        var local = Create(RemoteTokenSource.LocalProcess);
        if (platform.IsEnvironmentFallback
            || platform.Label.Contains("FALLBACK", StringComparison.Ordinal)
            || fallback is not
            {
                Label: "TOKEN / ENV FALLBACK",
                IsEnvironmentFallback: true,
            }
            || !fallback.Description.Contains(
                RemoteTokenResolver.EnvironmentVariable,
                StringComparison.Ordinal)
            || local is not
            {
                Label: "TOKEN / LOCAL PROCESS",
                IsEnvironmentFallback: false,
            }
            || !local.Description.Contains("ephemeral", StringComparison.Ordinal))
        {
            throw new InvalidDataException("remote credential presentation is invalid");
        }
    }
}
