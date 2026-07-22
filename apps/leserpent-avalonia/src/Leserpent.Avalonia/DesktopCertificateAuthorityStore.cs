using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Text.Json;

internal sealed class DesktopCertificateAuthorityStore(string directory)
{
    private const int MaxCertificateBytes = 1024 * 1024;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    public string Import(string sourcePath)
    {
        var source = Path.GetFullPath(sourcePath);
        var payload = ReadBoundedRegularFile(source);
        return ImportPayload(payload, source);
    }

    public string ImportPem(string pem)
    {
        byte[] payload;
        try
        {
            payload = StrictUtf8.GetBytes(pem);
        }
        catch (EncoderFallbackException error)
        {
            throw new InvalidDataException("desktop CA is not valid UTF-8 PEM", error);
        }
        if (payload.Length is <= 0 or > MaxCertificateBytes)
        {
            throw new InvalidDataException("desktop CA has an invalid size");
        }
        return ImportPayload(payload, null);
    }

    private string ImportPayload(byte[] payload, string? source)
    {
        using var certificate = ParseCertificate(payload);
        ValidateCertificateAuthority(certificate);

        var fingerprint = Convert.ToHexString(
            certificate.GetCertHash(HashAlgorithmName.SHA256));
        var canonicalPem = certificate.ExportCertificatePem().TrimEnd('\r', '\n') + "\n";
        var canonical = Encoding.ASCII.GetBytes(canonicalPem);
        EnsurePrivateDirectory();
        var trustDirectory = Path.GetFullPath(directory);
        var destination = Path.Combine(trustDirectory, $"{fingerprint}.pem");
        if (source is not null
            && IsDirectChild(source, trustDirectory)
            && !string.Equals(source, destination, PathComparison()))
        {
            throw new InvalidDataException(
                "desktop trust certificate content does not match its fingerprint path");
        }
        if (File.Exists(destination))
        {
            EnsureMatchingDestination(destination, canonical);
            return destination;
        }

        var temporary = Path.Combine(
            trustDirectory,
            $".{fingerprint}.{Guid.NewGuid():N}.tmp");
        try
        {
            using (var stream = new FileStream(
                temporary,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                4096,
                FileOptions.WriteThrough))
            {
                stream.Write(canonical);
                stream.Flush(true);
            }
            SetPrivateFileMode(temporary);
            File.Move(temporary, destination, false);
            return destination;
        }
        catch (IOException) when (File.Exists(destination))
        {
            EnsureMatchingDestination(destination, canonical);
            return destination;
        }
        finally
        {
            File.Delete(temporary);
        }
    }

    public void PruneExcept(string? retainedPath) =>
        PruneExcept(retainedPath is null ? [] : [retainedPath]);

    public void PruneExcept(IEnumerable<string> retainedPaths)
    {
        if (!Directory.Exists(directory))
        {
            return;
        }
        EnsurePrivateDirectory();
        var trustDirectory = Path.GetFullPath(directory);
        var retained = new HashSet<string>(PathComparer());
        foreach (var retainedPath in retainedPaths)
        {
            var fullPath = Path.GetFullPath(retainedPath);
            if (!IsDirectChild(fullPath, trustDirectory))
            {
                throw new InvalidDataException("retained desktop CA is outside the trust directory");
            }
            if (!File.Exists(fullPath)
                || !IsCanonicalCertificateName(Path.GetFileName(fullPath)))
            {
                throw new InvalidDataException("retained desktop CA is not a managed certificate");
            }
            retained.Add(fullPath);
        }

        foreach (var entry in Directory.EnumerateFileSystemEntries(trustDirectory))
        {
            var attributes = File.GetAttributes(entry);
            if ((attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
            {
                throw new InvalidDataException("desktop trust directory contains an unknown entry");
            }
            var name = Path.GetFileName(entry);
            if (IsTemporaryCertificateName(name))
            {
                File.Delete(entry);
                continue;
            }
            if (!IsCanonicalCertificateName(name))
            {
                throw new InvalidDataException("desktop trust directory contains an unknown entry");
            }
            if (!retained.Contains(entry))
            {
                File.Delete(entry);
            }
        }
    }

    public static DesktopCertificateAuthorityStore Default() =>
        new(DefaultDirectory());

    public static string DefaultDirectory()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(root))
        {
            throw new InvalidOperationException("local application data directory is unavailable");
        }
        return Path.Combine(root, "leserpent", "trust-v1");
    }

    public static void VerifyContract()
    {
        var root = Path.Combine(Path.GetTempPath(), $"leserpent-ca-store-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            var source = Path.Combine(root, "source.pem");
            using var key = RSA.Create(2048);
            var request = new CertificateRequest(
                "CN=Leserpent CA Store Verification",
                key,
                HashAlgorithmName.SHA256,
                RSASignaturePadding.Pkcs1);
            request.CertificateExtensions.Add(
                new X509BasicConstraintsExtension(true, false, 0, true));
            request.CertificateExtensions.Add(
                new X509KeyUsageExtension(X509KeyUsageFlags.KeyCertSign, true));
            using var certificate = request.CreateSelfSigned(
                DateTimeOffset.UtcNow.AddMinutes(-1),
                DateTimeOffset.UtcNow.AddDays(1));
            File.WriteAllText(source, certificate.ExportCertificatePem());

            var store = new DesktopCertificateAuthorityStore(Path.Combine(root, "trust"));
            var imported = store.Import(source);
            var repeated = store.Import(source);
            var expectedName = $"{certificate.GetCertHashString(HashAlgorithmName.SHA256)}.pem";
            if (imported != repeated
                || Path.GetFileName(imported) != expectedName
                || !File.Exists(imported)
                || File.ReadAllText(imported)
                    != certificate.ExportCertificatePem().TrimEnd('\r', '\n') + "\n")
            {
                throw new InvalidDataException("desktop CA import is not canonical and idempotent");
            }
            if (!OperatingSystem.IsWindows()
                && File.GetUnixFileMode(imported)
                    != (UnixFileMode.UserRead | UnixFileMode.UserWrite))
            {
                throw new InvalidDataException("desktop CA import is not private");
            }

            var bootstrapRoot = Path.Combine(root, "bootstrap-trust");
            Directory.CreateDirectory(bootstrapRoot);
            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(
                    bootstrapRoot,
                    UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
            }
            const string bootstrapEndpoint = "https://control.example:9443";
            var bootstrapPem = certificate.ExportCertificatePem().TrimEnd('\r', '\n') + "\n";
            var bootstrapRecord = Path.Combine(bootstrapRoot, "control-example.json");
            using (var stream = new FileStream(
                bootstrapRecord,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None))
            using (var writer = new Utf8JsonWriter(stream))
            {
                writer.WriteStartObject();
                writer.WriteString("endpoint", bootstrapEndpoint);
                writer.WriteString("ca_pem", bootstrapPem);
                writer.WriteString(
                    "ca_sha256",
                    Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(bootstrapPem)))
                        .ToLowerInvariant());
                writer.WriteEndObject();
            }
            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(
                    bootstrapRecord,
                    UnixFileMode.UserRead | UnixFileMode.UserWrite);
            }
            var bootstrapProfile = new DesktopConnectionProfile
            {
                SchemaVersion = 1,
                Endpoint = bootstrapEndpoint,
                BootstrapTrustRoot = bootstrapRoot,
                BootstrapTrustHandle = "vault:leserpent-ca:control-example",
            };
            var resolvedBootstrapCertificate =
                DesktopProductStartup.ResolveCertificateAuthorityPath(bootstrapProfile, store);
            if (resolvedBootstrapCertificate != imported)
            {
                throw new InvalidDataException(
                    "desktop bootstrap trust did not resolve to the managed CA");
            }

            var profileStore = new DesktopConnectionProfileStore(
                Path.Combine(root, "profile.json"));
            var externalProfile = new DesktopConnectionProfile
            {
                SchemaVersion = 1,
                Endpoint = "https://control.example:9443",
                CertificateAuthorityPath = source,
            };
            profileStore.Save(externalProfile);
            var migrated = DesktopProductStartup.PrepareSavedProfile(
                externalProfile,
                profileStore,
                store);
            if (migrated.CertificateAuthorityPath != imported
                || profileStore.Load() != migrated)
            {
                throw new InvalidDataException("desktop profile CA path was not migrated");
            }

            using var leafKey = RSA.Create(2048);
            var leafRequest = new CertificateRequest(
                "CN=Leserpent Leaf Verification",
                leafKey,
                HashAlgorithmName.SHA256,
                RSASignaturePadding.Pkcs1);
            leafRequest.CertificateExtensions.Add(
                new X509BasicConstraintsExtension(false, false, 0, true));
            using var leaf = leafRequest.CreateSelfSigned(
                DateTimeOffset.UtcNow.AddMinutes(-1),
                DateTimeOffset.UtcNow.AddDays(1));
            File.WriteAllText(source, leaf.ExportCertificatePem());
            ExpectInvalidData(
                () => store.Import(source),
                "desktop CA import accepted a non-CA certificate");

            File.WriteAllText(source, certificate.ExportCertificatePem());
            File.AppendAllText(source, "PRIVATE KEY");
            ExpectInvalidData(
                () => store.Import(source),
                "desktop CA import accepted trailing material");

            using var replacementKey = RSA.Create(2048);
            var replacementRequest = new CertificateRequest(
                "CN=Leserpent Replacement CA Verification",
                replacementKey,
                HashAlgorithmName.SHA256,
                RSASignaturePadding.Pkcs1);
            replacementRequest.CertificateExtensions.Add(
                new X509BasicConstraintsExtension(true, false, 0, true));
            replacementRequest.CertificateExtensions.Add(
                new X509KeyUsageExtension(X509KeyUsageFlags.KeyCertSign, true));
            using var replacement = replacementRequest.CreateSelfSigned(
                DateTimeOffset.UtcNow.AddMinutes(-1),
                DateTimeOffset.UtcNow.AddDays(1));
            File.WriteAllText(imported, replacement.ExportCertificatePem());
            ExpectInvalidData(
                () => store.Import(imported),
                "desktop CA import accepted a replaced managed certificate");

            if (!OperatingSystem.IsWindows())
            {
                File.WriteAllText(source, certificate.ExportCertificatePem());
                var symbolicLink = Path.Combine(root, "linked.pem");
                File.CreateSymbolicLink(symbolicLink, source);
                ExpectInvalidData(
                    () => store.Import(symbolicLink),
                    "desktop CA import accepted a symbolic link");
            }

            File.WriteAllText(imported, certificate.ExportCertificatePem());
            var secondSource = Path.Combine(root, "second.pem");
            File.WriteAllText(secondSource, replacement.ExportCertificatePem());
            var second = store.Import(secondSource);
            var staleTemporary = Path.Combine(
                Path.GetDirectoryName(second)!,
                $".{new string('A', 64)}.{new string('b', 32)}.tmp");
            File.WriteAllText(staleTemporary, "stale");
            store.PruneExcept([imported, second]);
            if (!File.Exists(imported)
                || !File.Exists(second)
                || File.Exists(staleTemporary))
            {
                throw new InvalidDataException("desktop CA pruning did not retain the active CA set");
            }
            store.PruneExcept(second);
            if (File.Exists(imported) || !File.Exists(second))
            {
                throw new InvalidDataException("desktop CA pruning did not remove an unused CA");
            }
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    private static byte[] ReadBoundedRegularFile(string path)
    {
        var info = new FileInfo(path);
        if (!info.Exists
            || info.Length is <= 0 or > MaxCertificateBytes
            || (info.Attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("desktop CA must be a bounded regular file");
        }
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Length is <= 0 or > MaxCertificateBytes)
        {
            throw new InvalidDataException("desktop CA changed to an invalid size while opening");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        return payload;
    }

    private static X509Certificate2 ParseCertificate(byte[] payload)
    {
        string pem;
        try
        {
            pem = StrictUtf8.GetString(payload);
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException("desktop CA is not valid UTF-8 PEM", error);
        }
        PemFields fields;
        try
        {
            fields = PemEncoding.Find(pem);
        }
        catch (ArgumentException error)
        {
            throw new InvalidDataException("desktop CA PEM framing is invalid", error);
        }
        if (!pem.AsSpan()[fields.Label].SequenceEqual("CERTIFICATE")
            || pem[..fields.Location.Start.GetOffset(pem.Length)].Any(character => !char.IsWhiteSpace(character))
            || pem[fields.Location.End.GetOffset(pem.Length)..].Any(character => !char.IsWhiteSpace(character)))
        {
            throw new InvalidDataException("desktop CA must contain exactly one PEM certificate");
        }
        try
        {
            return X509Certificate2.CreateFromPem(pem);
        }
        catch (CryptographicException error)
        {
            throw new InvalidDataException("desktop CA PEM is invalid", error);
        }
    }

    private static void ValidateCertificateAuthority(X509Certificate2 certificate)
    {
        var constraints = certificate.Extensions
            .OfType<X509BasicConstraintsExtension>()
            .SingleOrDefault();
        if (constraints is null || !constraints.CertificateAuthority)
        {
            throw new InvalidDataException("desktop trust certificate is not a certificate authority");
        }
        var keyUsage = certificate.Extensions.OfType<X509KeyUsageExtension>().SingleOrDefault();
        if (keyUsage is not null
            && (keyUsage.KeyUsages & X509KeyUsageFlags.KeyCertSign) == 0)
        {
            throw new InvalidDataException("desktop CA is not permitted to sign certificates");
        }
    }

    private void EnsurePrivateDirectory()
    {
        Directory.CreateDirectory(directory);
        if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException("desktop trust directory must not be a symbolic link");
        }
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(
                directory,
                UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        }
    }

    private static void EnsureMatchingDestination(string destination, byte[] expected)
    {
        var actual = ReadBoundedRegularFile(destination);
        if (!CryptographicOperations.FixedTimeEquals(actual, expected))
        {
            throw new InvalidDataException("desktop trust certificate content does not match its fingerprint");
        }
        SetPrivateFileMode(destination);
    }

    private static void SetPrivateFileMode(string path)
    {
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(path, UnixFileMode.UserRead | UnixFileMode.UserWrite);
        }
    }

    private static bool IsDirectChild(string path, string parent) => string.Equals(
        Path.GetDirectoryName(path),
        parent,
        PathComparison());

    private static StringComparison PathComparison() =>
        OperatingSystem.IsWindows() || OperatingSystem.IsMacOS()
            ? StringComparison.OrdinalIgnoreCase
            : StringComparison.Ordinal;

    private static StringComparer PathComparer() =>
        OperatingSystem.IsWindows() || OperatingSystem.IsMacOS()
            ? StringComparer.OrdinalIgnoreCase
            : StringComparer.Ordinal;

    private static bool IsCanonicalCertificateName(string name)
    {
        if (name.Length != 68 || !name.EndsWith(".pem", StringComparison.Ordinal))
        {
            return false;
        }
        return IsHex(name.AsSpan(0, 64), true);
    }

    private static bool IsTemporaryCertificateName(string name)
    {
        var parts = name.Split('.', StringSplitOptions.None);
        return parts is ["", var fingerprint, var nonce, "tmp"]
            && fingerprint.Length == 64
            && nonce.Length == 32
            && IsHex(fingerprint, true)
            && IsHex(nonce, false);
    }

    private static bool IsHex(ReadOnlySpan<char> value, bool uppercaseOnly)
    {
        foreach (var character in value)
        {
            var digit = character is >= '0' and <= '9';
            var uppercase = character is >= 'A' and <= 'F';
            var lowercase = character is >= 'a' and <= 'f';
            if (!digit && !uppercase && (uppercaseOnly || !lowercase))
            {
                return false;
            }
        }
        return true;
    }

    private static void ExpectInvalidData(Action action, string failure)
    {
        try
        {
            action();
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException(failure);
    }
}
