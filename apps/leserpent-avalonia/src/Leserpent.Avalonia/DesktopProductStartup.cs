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
        var importedCertificate = ResolveCertificateAuthorityPath(profile, certificateStore);
        if (profile.CertificateAuthorityPath is not null
            && !string.Equals(
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
        certificateStore.PruneExcept(importedCertificate);
        return profile;
    }

    public static DesktopConnectionCatalog PrepareSavedCatalog(
        DesktopConnectionCatalog catalog,
        DesktopConnectionCatalogStore catalogStore,
        DesktopCertificateAuthorityStore certificateStore)
    {
        var changed = false;
        var connections = new List<DesktopDaemonConnection>(catalog.Connections.Count);
        foreach (var connection in catalog.Connections)
        {
            var importedCertificate = ResolveCertificateAuthorityPath(
                connection.Profile,
                certificateStore);
            if (connection.Profile.CertificateAuthorityPath is null)
            {
                connections.Add(connection);
                continue;
            }
            if (string.Equals(
                importedCertificate,
                connection.Profile.CertificateAuthorityPath,
                StringComparison.Ordinal))
            {
                connections.Add(connection);
                continue;
            }
            changed = true;
            connections.Add(connection with
            {
                Profile = connection.Profile with
                {
                    CertificateAuthorityPath = importedCertificate,
                },
            });
        }
        var prepared = changed ? catalog with { Connections = connections } : catalog;
        if (changed)
        {
            catalogStore.Save(prepared);
        }
        return prepared;
    }

    public static DesktopProductStartupPlan Resolve(
        DesktopConnectionProfile profile,
        string? submittedToken = null)
    {
        DesktopConnectionProfileStore.Validate(profile);
        var certificateAuthorityPath = profile.CertificateAuthorityPath
            ?? throw new InvalidDataException(
                "bootstrap trust profiles require a desktop certificate store");
        return Resolve(profile, certificateAuthorityPath, submittedToken);
    }

    public static DesktopProductStartupPlan Resolve(
        DesktopConnectionProfile profile,
        DesktopCertificateAuthorityStore certificateStore,
        string? submittedToken = null)
    {
        DesktopConnectionProfileStore.Validate(profile);
        var certificateAuthorityPath = ResolveCertificateAuthorityPath(
            profile,
            certificateStore);
        return Resolve(profile, certificateAuthorityPath, submittedToken);
    }

    public static string ResolveCertificateAuthorityPath(
        DesktopConnectionProfile profile,
        DesktopCertificateAuthorityStore certificateStore)
    {
        DesktopConnectionProfileStore.Validate(profile);
        if (profile.CertificateAuthorityPath is { } certificateAuthorityPath)
        {
            return certificateStore.Import(certificateAuthorityPath);
        }
        var record = BootstrapTrustRecordStore.Load(
            profile.Endpoint,
            profile.BootstrapTrustRoot!,
            profile.BootstrapTrustHandle!);
        return certificateStore.ImportPem(record.CertificateAuthorityPem);
    }

    private static DesktopProductStartupPlan Resolve(
        DesktopConnectionProfile profile,
        string certificateAuthorityPath,
        string? submittedToken)
    {
        var endpoint = RemoteClientOptions.ParseEndpoint(profile.Endpoint);
        if (submittedToken is null)
        {
            var token = RemoteTokenResolver.Resolve(endpoint);
            return new DesktopProductStartupPlan(
                profile,
                RemoteClientOptions.Create(
                    profile.Endpoint,
                    certificateAuthorityPath,
                    token.Value),
                token.Source);
        }

        var options = RemoteClientOptions.Create(
            profile.Endpoint,
            certificateAuthorityPath,
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
        var certificatePath = Path.GetFullPath(profile.CertificateAuthorityPath!);
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
