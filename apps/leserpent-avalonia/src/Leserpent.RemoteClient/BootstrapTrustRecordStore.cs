using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Text.Json;

public sealed record BootstrapTrustRecord(
    string Endpoint,
    string CertificateAuthorityPem,
    string CertificateAuthoritySha256);

public static class BootstrapTrustRecordStore
{
    private const int MaxRecordBytes = 64 * 1024;
    private const int MaxCertificateAuthorityBytes = 32 * 1024;
    private const string HandlePrefix = "vault:leserpent-ca:";
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    public static BootstrapTrustRecord Load(
        string expectedEndpoint,
        string trustRoot,
        string handle)
    {
        _ = RemoteClientOptions.ParseEndpoint(expectedEndpoint);
        var key = ParseHandle(handle);
        var root = ValidateRoot(trustRoot);
        var path = Path.Combine(root, $"{key}.json");
        var payload = ReadPrivateRecord(path);
        var record = Decode(payload);
        ValidateRecord(record);
        if (!string.Equals(record.Endpoint, expectedEndpoint, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "bootstrap trust record does not match the remote endpoint");
        }
        return record;
    }

    public static void ValidateReference(string trustRoot, string handle)
    {
        _ = ParseHandle(handle);
        _ = ValidateRoot(trustRoot);
    }

    public static void VerifyContract()
    {
        var root = Path.Combine(Path.GetTempPath(), $"leserpent-bootstrap-trust-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        SetPrivateDirectory(root);
        try
        {
            using var key = RSA.Create(2048);
            var request = new CertificateRequest(
                "CN=Leserpent Bootstrap Trust Verification",
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
            var pem = certificate.ExportCertificatePem().TrimEnd('\r', '\n') + "\n";
            const string endpoint = "https://control.example:9443";
            const string handle = "vault:leserpent-ca:control-example";
            var path = Path.Combine(root, "control-example.json");
            WriteRecord(path, endpoint, pem, Sha256(pem), false);

            var loaded = Load(endpoint, root, handle);
            if (loaded.Endpoint != endpoint || loaded.CertificateAuthorityPem != pem)
            {
                throw new InvalidDataException("bootstrap trust record did not round-trip");
            }
            ExpectInvalidData(
                () => _ = Load("https://other.example:9443", root, handle),
                "bootstrap trust record accepted the wrong endpoint");

            WriteRecord(path, endpoint, pem, new string('0', 64), false);
            ExpectInvalidData(
                () => _ = Load(endpoint, root, handle),
                "bootstrap trust record accepted a replaced digest");
            WriteRecord(path, endpoint, pem, Sha256(pem), true);
            ExpectInvalidData(
                () => _ = Load(endpoint, root, handle),
                "bootstrap trust record accepted an unknown field");
            WriteRecord(path, endpoint, pem, Sha256(pem), false);

            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(path, UnixFileMode.UserRead);
                ExpectInvalidData(
                    () => _ = Load(endpoint, root, handle),
                    "bootstrap trust record accepted the wrong file mode");
                File.SetUnixFileMode(path, UnixFileMode.UserRead | UnixFileMode.UserWrite);
                var linkedRoot = Path.Combine(Path.GetDirectoryName(root)!, $"{Path.GetFileName(root)}-link");
                Directory.CreateSymbolicLink(linkedRoot, root);
                try
                {
                    ExpectInvalidData(
                        () => _ = Load(endpoint, linkedRoot, handle),
                        "bootstrap trust record accepted a linked root");
                }
                finally
                {
                    Directory.Delete(linkedRoot);
                }
            }
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    private static string ParseHandle(string handle)
    {
        if (handle.Length is <= 0 or > 128
            || !handle.StartsWith(HandlePrefix, StringComparison.Ordinal))
        {
            throw new InvalidDataException("bootstrap trust handle is invalid");
        }
        var key = handle[HandlePrefix.Length..];
        if (key.Length == 0 || key.Any(character =>
                !char.IsAsciiLetterOrDigit(character)
                && character is not ('.' or '_' or '-')))
        {
            throw new InvalidDataException("bootstrap trust handle is invalid");
        }
        return key;
    }

    private static string ValidateRoot(string trustRoot)
    {
        if (trustRoot.Length is <= 0 or > 4096
            || trustRoot.Any(char.IsControl)
            || !Path.IsPathFullyQualified(trustRoot))
        {
            throw new InvalidDataException("bootstrap trust root is invalid");
        }
        var root = Path.GetFullPath(trustRoot).TrimEnd(Path.DirectorySeparatorChar);
        if (root.Length == 0 || root == Path.GetPathRoot(root))
        {
            throw new InvalidDataException("bootstrap trust root is unsafe");
        }
        var info = new DirectoryInfo(root);
        if (!info.Exists || (info.Attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                "bootstrap trust root must be a regular directory");
        }
        if (!OperatingSystem.IsWindows()
            && File.GetUnixFileMode(root)
                != (UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute))
        {
            throw new InvalidDataException("bootstrap trust root must be private");
        }
        return root;
    }

    private static byte[] ReadPrivateRecord(string path)
    {
        var info = new FileInfo(path);
        if (!info.Exists
            || info.Length is <= 0 or > MaxRecordBytes
            || (info.Attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException(
                "bootstrap trust record must be a bounded regular file");
        }
        if (!OperatingSystem.IsWindows()
            && File.GetUnixFileMode(path)
                != (UnixFileMode.UserRead | UnixFileMode.UserWrite))
        {
            throw new InvalidDataException("bootstrap trust record must be private");
        }
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Length is <= 0 or > MaxRecordBytes)
        {
            throw new InvalidDataException(
                "bootstrap trust record changed to an invalid size while opening");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        if ((File.GetAttributes(path)
                & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("bootstrap trust record changed while reading");
        }
        return payload;
    }

    private static BootstrapTrustRecord Decode(byte[] payload)
    {
        try
        {
            using var document = JsonDocument.Parse(payload, new JsonDocumentOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 4,
            });
            if (document.RootElement.ValueKind != JsonValueKind.Object)
            {
                throw new InvalidDataException("bootstrap trust record must be an object");
            }
            string? endpoint = null;
            string? caPem = null;
            string? caSha256 = null;
            var names = new HashSet<string>(StringComparer.Ordinal);
            foreach (var property in document.RootElement.EnumerateObject())
            {
                if (!names.Add(property.Name) || property.Value.ValueKind != JsonValueKind.String)
                {
                    throw new InvalidDataException(
                        "bootstrap trust record contains an invalid field");
                }
                switch (property.Name)
                {
                    case "endpoint":
                        endpoint = property.Value.GetString();
                        break;
                    case "ca_pem":
                        caPem = property.Value.GetString();
                        break;
                    case "ca_sha256":
                        caSha256 = property.Value.GetString();
                        break;
                    default:
                        throw new InvalidDataException(
                            "bootstrap trust record contains an unknown field");
                }
            }
            if (endpoint is null || caPem is null || caSha256 is null || names.Count != 3)
            {
                throw new InvalidDataException("bootstrap trust record is incomplete");
            }
            return new BootstrapTrustRecord(endpoint, caPem, caSha256);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("bootstrap trust record JSON is invalid", error);
        }
    }

    private static void ValidateRecord(BootstrapTrustRecord record)
    {
        _ = RemoteClientOptions.ParseEndpoint(record.Endpoint);
        byte[] pem;
        try
        {
            pem = StrictUtf8.GetBytes(record.CertificateAuthorityPem);
        }
        catch (EncoderFallbackException error)
        {
            throw new InvalidDataException("bootstrap trust CA is not valid UTF-8", error);
        }
        if (pem.Length > MaxCertificateAuthorityBytes
            || !record.CertificateAuthorityPem.StartsWith(
                "-----BEGIN CERTIFICATE-----\n",
                StringComparison.Ordinal)
            || !record.CertificateAuthorityPem.EndsWith(
                "-----END CERTIFICATE-----\n",
                StringComparison.Ordinal)
            || record.CertificateAuthoritySha256.Length != 64
            || record.CertificateAuthoritySha256.Any(character =>
                !char.IsAsciiDigit(character) && character is not (>= 'a' and <= 'f')))
        {
            throw new InvalidDataException("bootstrap trust record is invalid");
        }
        var expected = Encoding.ASCII.GetBytes(record.CertificateAuthoritySha256);
        var actual = Encoding.ASCII.GetBytes(
            Convert.ToHexString(SHA256.HashData(pem)).ToLowerInvariant());
        if (!CryptographicOperations.FixedTimeEquals(actual, expected))
        {
            throw new InvalidDataException("bootstrap trust CA digest does not match");
        }
        try
        {
            using var certificate = RemoteTls.LoadRootFromPem(record.CertificateAuthorityPem);
        }
        catch (CryptographicException error)
        {
            throw new InvalidDataException("bootstrap trust CA PEM is invalid", error);
        }
    }

    private static void WriteRecord(
        string path,
        string endpoint,
        string pem,
        string sha256,
        bool includeUnknown)
    {
        using (var stream = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.None))
        using (var writer = new Utf8JsonWriter(stream))
        {
            writer.WriteStartObject();
            writer.WriteString("endpoint", endpoint);
            writer.WriteString("ca_pem", pem);
            writer.WriteString("ca_sha256", sha256);
            if (includeUnknown)
            {
                writer.WriteBoolean("unknown", true);
            }
            writer.WriteEndObject();
        }
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(path, UnixFileMode.UserRead | UnixFileMode.UserWrite);
        }
    }

    private static string Sha256(string value) => Convert.ToHexString(
        SHA256.HashData(StrictUtf8.GetBytes(value))).ToLowerInvariant();

    private static void SetPrivateDirectory(string path)
    {
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(
                path,
                UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        }
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
