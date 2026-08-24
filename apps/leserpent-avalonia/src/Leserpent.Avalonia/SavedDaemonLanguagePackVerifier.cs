using System.Net.Http;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;

internal static class SavedDaemonLanguagePackVerifier
{
    public static void Verify(
        DesktopConnectionProfile liveProfile,
        string rootDirectory,
        string forbiddenCredential)
    {
        if (string.IsNullOrWhiteSpace(forbiddenCredential))
        {
            throw new ArgumentException(
                "saved daemon verification requires a non-empty leak sentinel",
                nameof(forbiddenCredential));
        }
        DesktopConnectionProfileStore.Validate(liveProfile);
        var sourceAuthorityPath = Path.GetFullPath(
            liveProfile.CertificateAuthorityPath
                ?? throw new InvalidDataException(
                    "saved daemon language-pack verification requires explicit CA trust"));
        var sourceAuthorityDigest = DigestFile(sourceAuthorityPath);

        var root = Path.GetFullPath(rootDirectory);
        EnsurePrivateDirectory(root);
        var catalogPath = Path.Combine(root, "desktop-connections-v1.json");
        var catalogStore = new DesktopConnectionCatalogStore(catalogPath);
        var expectedConnection = DesktopDaemonConnection.FromProfile(liveProfile with
        {
            CertificateAuthorityPath = sourceAuthorityPath,
        });
        catalogStore.Save(new DesktopConnectionCatalog
        {
            SchemaVersion = 1,
            Connections = [expectedConnection],
        });

        var trustRoot = Path.Combine(root, "trust-v1");
        var trustStore = new DesktopCertificateAuthorityStore(trustRoot);
        var prepared = DesktopProductStartup.PrepareSavedCatalog(
            catalogStore.Load(),
            catalogStore,
            trustStore);
        var persisted = catalogStore.Load();
        if (prepared.Connections.Count != 1
            || persisted.Connections is not [var selected]
            || selected != prepared.Connections[0]
            || selected.DaemonId != expectedConnection.DaemonId
            || selected.Profile.CertificateAuthorityPath is not { } selectedAuthorityPath)
        {
            throw new InvalidDataException(
                "saved daemon language-pack connection did not survive persistence");
        }

        var managedAuthorities = Directory.EnumerateFiles(
                trustRoot,
                "*.pem",
                SearchOption.TopDirectoryOnly)
            .ToArray();
        if (managedAuthorities is not [var managedAuthority]
            || !PathEquals(managedAuthority, selectedAuthorityPath))
        {
            throw new InvalidDataException(
                "saved daemon language-pack source did not retain exactly one selected CA");
        }

        var catalogPayload = File.ReadAllBytes(catalogPath);
        if (Encoding.UTF8.GetString(catalogPayload).Contains(
            forbiddenCredential,
            StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "saved daemon language-pack catalog leaked its live credential");
        }
        var catalogDigest = SHA256.HashData(catalogPayload);
        var selectedAuthorityDigest = DigestFile(selectedAuthorityPath);

        var source = DesktopLanguagePackSource.FromConnection(
            selected,
            selectedAuthorityPath);
        if (source.SourceId != selected.DaemonId
            || source.Endpoint != RemoteClientOptions.ParseEndpoint(liveProfile.Endpoint)
            || !PathEquals(source.CertificateAuthorityPath, selectedAuthorityPath)
            || source.SourceId == "local-orchestra")
        {
            throw new InvalidDataException(
                "saved daemon language-pack source lost its persisted authority identity");
        }

        VerifyWrongCertificateRejected(source, Path.Combine(root, "decoy-trust"));

        var languagePackRoot = Path.Combine(root, "language-packs");
        var languagePackStore = new DesktopLanguagePackStore(languagePackRoot);
        DesktopLanguagePackDownload download;
        using (var client = new DesktopLanguagePackCatalogClient(source))
        {
            download = client.DownloadAsync("pt-BR").GetAwaiter().GetResult();
            var installed = languagePackStore.InstallCatalogArtifact(
                download.Payload,
                download.Sha256,
                download.Locale,
                download.Version);
            var snapshot = languagePackStore.LoadAll();
            if (download.SourceId != selected.DaemonId
                || installed.Manifest.Locale != "pt-BR"
                || snapshot.Packs.Count != 1
                || !snapshot.Packs.ContainsKey("pt-BR")
                || snapshot.RejectedFiles.Count != 0)
            {
                throw new InvalidDataException(
                    "saved daemon language-pack download did not round-trip privately");
            }
        }

        var installedPath = Path.Combine(languagePackRoot, $"{download.Locale}.json");
        VerifyPrivateFiles(catalogPath, selectedAuthorityPath, installedPath);
        languagePackStore.Remove(download.Locale);
        if (languagePackStore.LoadAll().Packs.Count != 0
            || Directory.EnumerateFileSystemEntries(languagePackRoot).Any())
        {
            throw new InvalidDataException(
                "saved daemon language-pack verification did not clean its store");
        }

        if (!CryptographicOperations.FixedTimeEquals(
                sourceAuthorityDigest,
                DigestFile(sourceAuthorityPath))
            || !CryptographicOperations.FixedTimeEquals(
                catalogDigest,
                DigestFile(catalogPath))
            || !CryptographicOperations.FixedTimeEquals(
                selectedAuthorityDigest,
                DigestFile(selectedAuthorityPath)))
        {
            throw new InvalidDataException(
                "saved daemon language-pack proof mutated its persisted inputs");
        }
    }

    private static void VerifyWrongCertificateRejected(
        DesktopLanguagePackSource source,
        string decoyTrustRoot)
    {
        using var key = RSA.Create(2048);
        var request = new CertificateRequest(
            "CN=Leserpent Saved Daemon Rejection CA",
            key,
            HashAlgorithmName.SHA256,
            RSASignaturePadding.Pkcs1);
        request.CertificateExtensions.Add(
            new X509BasicConstraintsExtension(true, false, 0, true));
        request.CertificateExtensions.Add(
            new X509KeyUsageExtension(
                X509KeyUsageFlags.KeyCertSign | X509KeyUsageFlags.CrlSign,
                true));
        request.CertificateExtensions.Add(
            new X509SubjectKeyIdentifierExtension(request.PublicKey, false));
        using var certificate = request.CreateSelfSigned(
            DateTimeOffset.UtcNow.AddMinutes(-1),
            DateTimeOffset.UtcNow.AddDays(1));
        var decoyAuthorityPath = new DesktopCertificateAuthorityStore(decoyTrustRoot)
            .ImportPem(certificate.ExportCertificatePem());

        try
        {
            using var rejectedClient = new DesktopLanguagePackCatalogClient(source with
            {
                CertificateAuthorityPath = decoyAuthorityPath,
            });
            _ = rejectedClient.DownloadAsync("pt-BR").GetAwaiter().GetResult();
        }
        catch (HttpRequestException)
        {
            return;
        }
        throw new InvalidDataException(
            "saved daemon language-pack endpoint accepted an unselected CA");
    }

    private static void VerifyPrivateFiles(params string[] paths)
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var expected = UnixFileMode.UserRead | UnixFileMode.UserWrite;
        foreach (var path in paths)
        {
            if (File.GetUnixFileMode(path) != expected)
            {
                throw new InvalidDataException(
                    "saved daemon language-pack proof produced a non-private file");
            }
        }
    }

    private static byte[] DigestFile(string path) =>
        SHA256.HashData(File.ReadAllBytes(path));

    private static bool PathEquals(string left, string right) =>
        string.Equals(
            Path.GetFullPath(left),
            Path.GetFullPath(right),
            OperatingSystem.IsWindows()
                ? StringComparison.OrdinalIgnoreCase
                : StringComparison.Ordinal);

    private static void EnsurePrivateDirectory(string path)
    {
        Directory.CreateDirectory(path);
        if ((File.GetAttributes(path) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                "saved daemon language-pack proof root must not be a symbolic link");
        }
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(
                path,
                UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        }
    }
}
