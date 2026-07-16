using System.Net.Security;
using System.Security.Cryptography.X509Certificates;

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
