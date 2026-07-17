using Android.Content;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;

public sealed record AndroidConnectionProfile(string Endpoint, string CertificateAuthorityPath);

public sealed class AndroidConnectionProfileStore(Context context)
{
    private const string PreferencesName = "leserpent.remote.profile.v1";
    private const string EndpointKey = "endpoint";
    private readonly Context applicationContext = context.ApplicationContext
        ?? throw new InvalidOperationException("Android application context is unavailable.");

    public AndroidConnectionProfile? Load()
    {
        var preferences = applicationContext.GetSharedPreferences(
            PreferencesName,
            FileCreationMode.Private)
            ?? throw new InvalidOperationException("Android profile storage is unavailable.");
        var endpoint = preferences.GetString(EndpointKey, null);
        var certificatePath = endpoint is null ? null : CertificateAuthorityPath(endpoint);
        return endpoint is null || certificatePath is null || !File.Exists(certificatePath)
            ? null
            : new AndroidConnectionProfile(endpoint, certificatePath);
    }

    public AndroidConnectionProfile Save(string endpoint, string certificatePem)
    {
        var validatedEndpoint = RemoteClientOptions.ParseEndpoint(endpoint);
        ValidateCertificate(certificatePem);
        var canonicalEndpoint = validatedEndpoint.AbsoluteUri;
        var certificatePath = CertificateAuthorityPath(canonicalEndpoint);
        var temporaryPath = $"{certificatePath}.{Guid.NewGuid():N}.tmp";
        try
        {
            File.WriteAllText(temporaryPath, certificatePem);
            File.Move(temporaryPath, certificatePath, true);
        }
        finally
        {
            if (File.Exists(temporaryPath))
            {
                File.Delete(temporaryPath);
            }
        }
        var preferences = applicationContext.GetSharedPreferences(
            PreferencesName,
            FileCreationMode.Private)
            ?? throw new InvalidOperationException("Android profile storage is unavailable.");
        using var editor = preferences.Edit()
            ?? throw new InvalidOperationException("Android profile storage is unavailable.");
        if (!editor.PutString(EndpointKey, canonicalEndpoint)!.Commit())
        {
            throw new InvalidOperationException("Android profile storage rejected the write.");
        }
        return new AndroidConnectionProfile(canonicalEndpoint, certificatePath);
    }

    public string CertificateAuthorityPath(string endpoint) => Path.Combine(
        PrivateDirectory,
        $"remote-ca-{EndpointDigest(endpoint)}.pem");

    public string CachePath(string endpoint) => Path.Combine(
        applicationContext.CacheDir?.AbsolutePath
            ?? throw new InvalidOperationException("Android cache storage is unavailable."),
        $"remote-snapshot-{EndpointDigest(endpoint)}.json");

    private string PrivateDirectory => applicationContext.FilesDir?.AbsolutePath
        ?? throw new InvalidOperationException("Android private storage is unavailable.");

    private static string EndpointDigest(string endpoint)
    {
        var canonical = RemoteTokenResolver.Account(RemoteClientOptions.ParseEndpoint(endpoint));
        return Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(canonical)))
            .ToLowerInvariant();
    }

    private static void ValidateCertificate(string certificatePem)
    {
        if (certificatePem.Length is < 64 or > 64 * 1024
            || certificatePem.Any(character => character == '\0')
            || certificatePem.Contains("PRIVATE KEY", StringComparison.Ordinal))
        {
            throw new ArgumentException("CA certificate PEM is invalid.", nameof(certificatePem));
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
            throw new ArgumentException("CA certificate PEM is invalid.", nameof(certificatePem));
        }
    }
}
