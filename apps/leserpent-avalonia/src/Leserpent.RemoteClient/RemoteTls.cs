using System.Net.Security;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

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

internal static class RemoteTls
{
    public static X509Certificate2 LoadRoot(string path) =>
        X509Certificate2.CreateFromPem(File.ReadAllText(path));

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
