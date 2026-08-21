using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;

public interface IMobileEndpointStore
{
    string? Load();
    void Save(string endpoint);
}

public sealed record MobileConnectionProfile(
    string Endpoint,
    string CertificateAuthorityPath);

public sealed class MobileConnectionProfileStore
{
    private const int MaximumCertificateBytes = 64 * 1024;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private readonly IMobileEndpointStore endpointStore;
    private readonly string privateDirectory;
    private readonly string cacheDirectory;

    public MobileConnectionProfileStore(
        IMobileEndpointStore endpointStore,
        string privateDirectory,
        string cacheDirectory)
    {
        this.endpointStore = endpointStore
            ?? throw new ArgumentNullException(nameof(endpointStore));
        this.privateDirectory = RequirePrivateDirectory(
            privateDirectory,
            nameof(privateDirectory));
        this.cacheDirectory = RequirePrivateDirectory(
            cacheDirectory,
            nameof(cacheDirectory));
    }

    public MobileConnectionProfile? Load()
    {
        try
        {
            var storedEndpoint = endpointStore.Load();
            if (string.IsNullOrWhiteSpace(storedEndpoint))
            {
                return null;
            }
            var endpoint = RemoteClientOptions.ParseEndpoint(storedEndpoint).AbsoluteUri;
            var certificatePath = CertificateAuthorityPath(endpoint);
            var information = new FileInfo(certificatePath);
            if (!information.Exists
                || information.Length is < 64 or > MaximumCertificateBytes)
            {
                return null;
            }
            ValidateCertificate(File.ReadAllText(certificatePath, StrictUtf8));
            return new MobileConnectionProfile(endpoint, certificatePath);
        }
        catch (Exception error) when (error is
            ArgumentException
            or CryptographicException
            or DecoderFallbackException
            or InvalidOperationException
            or IOException
            or UnauthorizedAccessException)
        {
            return null;
        }
    }

    public MobileConnectionProfile Save(string endpoint, string certificatePem)
    {
        ArgumentNullException.ThrowIfNull(endpoint);
        ArgumentNullException.ThrowIfNull(certificatePem);
        var canonicalEndpoint = RemoteClientOptions.ParseEndpoint(endpoint).AbsoluteUri;
        ValidateCertificate(certificatePem);
        var certificatePath = CertificateAuthorityPath(canonicalEndpoint);
        var temporaryPath = $"{certificatePath}.{Guid.NewGuid():N}.tmp";
        try
        {
            using (var stream = new FileStream(
                temporaryPath,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                4096,
                FileOptions.WriteThrough))
            {
                using (var writer = new StreamWriter(
                    stream,
                    StrictUtf8,
                    4096,
                    leaveOpen: true))
                {
                    writer.Write(certificatePem);
                    writer.Flush();
                }
                stream.Flush(flushToDisk: true);
            }
            File.Move(temporaryPath, certificatePath, true);
        }
        finally
        {
            if (File.Exists(temporaryPath))
            {
                File.Delete(temporaryPath);
            }
        }
        endpointStore.Save(canonicalEndpoint);
        return new MobileConnectionProfile(canonicalEndpoint, certificatePath);
    }

    public string CertificateAuthorityPath(string endpoint) => Path.Combine(
        privateDirectory,
        $"remote-ca-{EndpointDigest(endpoint)}.pem");

    public string CachePath(string endpoint) => Path.Combine(
        cacheDirectory,
        $"remote-snapshot-{EndpointDigest(endpoint)}.json");

    private static string EndpointDigest(string endpoint)
    {
        var canonical = RemoteTokenResolver.Account(RemoteClientOptions.ParseEndpoint(endpoint));
        return Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(canonical)))
            .ToLowerInvariant();
    }

    private static void ValidateCertificate(string certificatePem)
    {
        var byteCount = StrictUtf8.GetByteCount(certificatePem);
        if (byteCount is < 64 or > MaximumCertificateBytes
            || certificatePem.Any(character => character == '\0')
            || certificatePem.Contains("PRIVATE KEY", StringComparison.OrdinalIgnoreCase))
        {
            throw new ArgumentException(
                "CA certificate PEM is invalid.",
                nameof(certificatePem));
        }
        try
        {
            using var certificate = X509Certificate2.CreateFromPem(certificatePem);
            if (certificate.HasPrivateKey)
            {
                throw new ArgumentException(
                    "CA input must not contain a private key.",
                    nameof(certificatePem));
            }
        }
        catch (CryptographicException)
        {
            throw new ArgumentException(
                "CA certificate PEM is invalid.",
                nameof(certificatePem));
        }
    }

    private static string RequirePrivateDirectory(string path, string parameter)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(path, parameter);
        if (!Path.IsPathFullyQualified(path))
        {
            throw new ArgumentException(
                "Mobile profile directories must be absolute.",
                parameter);
        }
        var canonical = Path.GetFullPath(path);
        Directory.CreateDirectory(canonical);
        return canonical;
    }
}
