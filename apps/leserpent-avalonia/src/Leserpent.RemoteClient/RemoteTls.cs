using System.Net.Security;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;

public sealed record RemoteTrustIdentity(
    string Origin,
    string ShortFingerprint,
    string Sha256Fingerprint)
{
    public static RemoteTrustIdentity Create(Uri endpoint, X509Certificate2 trustedRoot) =>
        FromSha256(endpoint, trustedRoot.GetCertHash(HashAlgorithmName.SHA256));

    public static RemoteTrustIdentity FromSha256(Uri endpoint, ReadOnlySpan<byte> fingerprint)
    {
        if (fingerprint.Length != 32)
        {
            throw new ArgumentException(
                "CA SHA-256 fingerprint must contain 32 bytes",
                nameof(fingerprint));
        }
        var hexadecimal = Convert.ToHexString(fingerprint);
        var formatted = string.Join(':', Enumerable.Range(0, hexadecimal.Length / 2)
            .Select(index => hexadecimal.Substring(index * 2, 2)));
        return new RemoteTrustIdentity(
            RemoteTokenResolver.Account(endpoint),
            hexadecimal[..16],
            formatted);
    }
}

public static class RemoteTls
{
    private const int MaxCertificateBytes = 1024 * 1024;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);

    public static X509Certificate2 LoadRoot(string path)
    {
        if (!Path.IsPathFullyQualified(path))
        {
            throw new InvalidDataException("remote CA path must be absolute");
        }
        var attributes = File.GetAttributes(path);
        if ((attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("remote CA must be a regular file");
        }
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Length is <= 0 or > MaxCertificateBytes)
        {
            throw new InvalidDataException("remote CA has an invalid size");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        try
        {
            return LoadRootFromPem(StrictUtf8.GetString(payload));
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException("remote CA is not valid UTF-8 PEM", error);
        }
    }

    public static X509Certificate2 LoadRootFromPem(string pem)
    {
        if (string.IsNullOrWhiteSpace(pem)
            || StrictUtf8.GetByteCount(pem) > MaxCertificateBytes
            || pem.Contains('\0'))
        {
            throw new InvalidDataException("remote CA PEM has an invalid size or encoding");
        }
        return X509Certificate2.CreateFromPem(pem);
    }

    public static bool ValidateServerCertificate(
        X509Certificate? certificate,
        SslPolicyErrors errors,
        X509Certificate2 trustedRoot)
    {
        if (certificate is null
            || (errors & (SslPolicyErrors.RemoteCertificateNameMismatch
                | SslPolicyErrors.RemoteCertificateNotAvailable)) != 0)
        {
            return false;
        }
        using var leaf = new X509Certificate2(certificate);
        using var customChain = new X509Chain();
        customChain.ChainPolicy.TrustMode = X509ChainTrustMode.CustomRootTrust;
        customChain.ChainPolicy.CustomTrustStore.Add(trustedRoot);
        customChain.ChainPolicy.RevocationMode = X509RevocationMode.NoCheck;
        customChain.ChainPolicy.DisableCertificateDownloads = true;
        return customChain.Build(leaf);
    }
}
