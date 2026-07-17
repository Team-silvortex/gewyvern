using System.Security.Cryptography;

internal sealed record DesktopProductStartupPlan(
    DesktopConnectionProfile Profile,
    RemoteClientOptions Options,
    RemoteTokenSource TokenSource);

internal static class DesktopProductStartup
{
    private const int VerificationMinimumPort = 49152;

    public static DesktopProductStartupPlan? TryResolve(
        DesktopConnectionProfileStore profileStore)
    {
        var profile = profileStore.Load();
        return profile is null ? null : Resolve(profile);
    }

    public static DesktopConnectionProfile PrepareSavedProfile(
        DesktopConnectionProfile profile,
        DesktopConnectionProfileStore profileStore,
        DesktopCertificateAuthorityStore certificateStore)
    {
        var importedCertificate = certificateStore.Import(
            profile.CertificateAuthorityPath);
        if (!string.Equals(
            importedCertificate,
            profile.CertificateAuthorityPath,
            StringComparison.Ordinal))
        {
            profile = profile with
            {
                CertificateAuthorityPath = importedCertificate,
            };
            profileStore.Save(profile);
        }
        certificateStore.PruneExcept(profile.CertificateAuthorityPath);
        return profile;
    }

    public static DesktopProductStartupPlan Resolve(
        DesktopConnectionProfile profile,
        string? submittedToken = null)
    {
        var endpoint = RemoteClientOptions.ParseEndpoint(profile.Endpoint);
        if (submittedToken is null)
        {
            var token = RemoteTokenResolver.Resolve(endpoint);
            return new DesktopProductStartupPlan(
                profile,
                RemoteClientOptions.Create(
                    profile.Endpoint,
                    profile.CertificateAuthorityPath,
                    token.Value),
                token.Source);
        }

        var options = RemoteClientOptions.Create(
            profile.Endpoint,
            profile.CertificateAuthorityPath,
            submittedToken);
        RemoteTokenResolver.Store(endpoint, submittedToken);
        return new DesktopProductStartupPlan(
            profile,
            options,
            RemoteTokenSource.PlatformStore);
    }

    public static void VerifyPackagedProfile(string profilePath)
    {
        var fullPath = Path.GetFullPath(profilePath);
        var temporaryRoot = Path.GetFullPath(Path.GetTempPath())
            .TrimEnd(Path.DirectorySeparatorChar)
            + Path.DirectorySeparatorChar;
        if (!fullPath.StartsWith(temporaryRoot, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "packaged startup verification profile must be inside the temporary directory");
        }
        var profileDirectory = Path.GetDirectoryName(fullPath)
            ?? throw new InvalidDataException("packaged startup profile directory is unavailable");
        if ((File.GetAttributes(profileDirectory) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                "packaged startup verification directory must not be a symbolic link");
        }

        var store = new DesktopConnectionProfileStore(fullPath);
        var profile = store.Load()
            ?? throw new InvalidDataException("packaged startup verification profile is absent");
        var endpoint = RemoteClientOptions.ParseEndpoint(profile.Endpoint);
        var certificatePath = Path.GetFullPath(profile.CertificateAuthorityPath);
        if (!certificatePath.StartsWith(temporaryRoot, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "packaged startup verification CA must be inside the temporary directory");
        }
        if (!endpoint.IsLoopback || endpoint.Port < VerificationMinimumPort)
        {
            throw new InvalidDataException(
                "packaged startup verification requires a high-port loopback endpoint");
        }
        if (PlatformRemoteTokenStore.Instance.Load(endpoint) is not null)
        {
            throw new InvalidDataException(
                "packaged startup verification refuses to replace an existing credential");
        }

        var token = Convert.ToHexString(RandomNumberGenerator.GetBytes(32));
        var stored = false;
        try
        {
            RemoteTokenResolver.Store(endpoint, token);
            stored = true;
            var plan = TryResolve(store)
                ?? throw new InvalidDataException("packaged startup plan was not created");
            if (plan.TokenSource != RemoteTokenSource.PlatformStore
                || plan.Options.Endpoint != endpoint
                || plan.Options.Token != token
                || plan.Profile != profile)
            {
                throw new InvalidDataException("packaged startup plan did not use Keychain state");
            }
        }
        finally
        {
            if (stored)
            {
                RemoteTokenResolver.Delete(endpoint);
            }
        }
    }
}
