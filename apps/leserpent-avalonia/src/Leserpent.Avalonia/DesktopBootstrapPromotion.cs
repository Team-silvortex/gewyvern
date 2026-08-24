using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

internal sealed class DesktopBootstrapPromotion(
    DesktopConnectionCatalogStore catalogStore,
    DesktopCertificateAuthorityStore certificateStore,
    string trustRoot,
    IRemoteTokenVault tokenVault,
    Func<string, string> resolveSessionCredential,
    Func<RemoteClientOptions, CancellationToken, Task> proveHealth)
{
    public async Task<DesktopDaemonConnection> PromoteAsync(
        RemoteBootstrapSnapshot state,
        CancellationToken cancellationToken)
    {
        if (state is not
            {
                Phase: "session_bound",
                MutationAuthorized: true,
                Endpoint: not null,
                SessionCredentialHandle: not null,
                TrustCredentialHandle: not null,
            })
        {
            throw new InvalidDataException(
                "connection promotion requires a verified SessionBound receipt");
        }

        var trust = BootstrapTrustRecordStore.Load(
            state.Endpoint,
            trustRoot,
            state.TrustCredentialHandle);
        var sessionToken = resolveSessionCredential(state.SessionCredentialHandle);
        RemoteClientOptions.ValidateToken(sessionToken);
        var endpoint = RemoteClientOptions.ParseEndpoint(state.Endpoint);
        var existingToken = tokenVault.Load(endpoint);
        if (existingToken is not null
            && !string.Equals(existingToken, sessionToken, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "the target endpoint already has a different platform credential");
        }

        var importedAuthority = certificateStore.ImportPem(trust.CertificateAuthorityPem);
        var options = RemoteClientOptions.Create(
            state.Endpoint,
            importedAuthority,
            sessionToken);
        await proveHealth(options, cancellationToken);

        var profile = new DesktopConnectionProfile
        {
            SchemaVersion = 1,
            Endpoint = state.Endpoint,
            BootstrapTrustRoot = trustRoot,
            BootstrapTrustHandle = state.TrustCredentialHandle,
        };
        var connection = DesktopDaemonConnection.FromProfile(profile);
        var storedToken = existingToken is null;
        if (storedToken)
        {
            tokenVault.Store(endpoint, sessionToken);
        }
        try
        {
            catalogStore.Upsert(connection);
        }
        catch
        {
            if (storedToken)
            {
                tokenVault.Delete(endpoint);
            }
            throw;
        }
        return connection;
    }

    public static void VerifyContract()
    {
        var root = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-bootstrap-promotion-{Environment.ProcessId}-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            var trustRoot = Path.Combine(root, "bootstrap-trust");
            Directory.CreateDirectory(trustRoot);
            SetPrivateDirectory(trustRoot);
            const string endpoint = "https://target.example:9443";
            const string trustHandle = "vault:leserpent-ca:target";
            var caPem = CreateCertificateAuthorityPem();
            var digest = Convert.ToHexString(
                SHA256.HashData(Encoding.UTF8.GetBytes(caPem))).ToLowerInvariant();
            var recordPath = Path.Combine(trustRoot, "target.json");
            File.WriteAllText(recordPath, JsonSerializer.Serialize(
                new BootstrapPromotionTrustFixture
                {
                    Endpoint = endpoint,
                    CaPem = caPem,
                    CaSha256 = digest,
                },
                BootstrapPromotionJsonContext.Default.BootstrapPromotionTrustFixture));
            SetPrivateFile(recordPath);

            var catalogPath = Path.Combine(root, "connections.json");
            var catalog = new DesktopConnectionCatalogStore(catalogPath);
            var certificates = new DesktopCertificateAuthorityStore(
                Path.Combine(root, "managed-trust"));
            var unrelatedAuthority = certificates.ImportPem(
                CreateCertificateAuthorityPem());
            var vault = new VerificationTokenVault();
            var healthProofs = 0;
            var promotion = new DesktopBootstrapPromotion(
                catalog,
                certificates,
                trustRoot,
                vault,
                handle => handle == "vault:leserpentd:target"
                    ? new string('s', 32)
                    : throw new InvalidDataException("unexpected session handle"),
                (options, _) =>
                {
                    if (options.Endpoint.GetComponents(
                            UriComponents.SchemeAndServer,
                            UriFormat.UriEscaped) != endpoint
                        || options.Token != new string('s', 32))
                    {
                        throw new InvalidDataException(
                            "promotion health proof received the wrong authority");
                    }
                    healthProofs++;
                    return Task.CompletedTask;
                });
            var state = BoundState(endpoint, trustHandle);
            var promoted = promotion.PromoteAsync(state, CancellationToken.None)
                .GetAwaiter().GetResult();
            if (healthProofs != 1
                || vault.Load(RemoteClientOptions.ParseEndpoint(endpoint)) != new string('s', 32)
                || catalog.Load().Connections is not [var saved]
                || saved != promoted
                || saved.Profile.BootstrapTrustRoot != trustRoot
                || saved.Profile.BootstrapTrustHandle != trustHandle
                || !File.Exists(unrelatedAuthority)
                || File.ReadAllText(catalogPath).Contains(
                    new string('s', 32),
                    StringComparison.Ordinal))
            {
                throw new InvalidDataException(
                    "bound bootstrap connection was not promoted safely");
            }

            ExpectInvalid(
                () => promotion.PromoteAsync(
                    state with { Phase = "bootstrapped", MutationAuthorized = false },
                    CancellationToken.None).GetAwaiter().GetResult(),
                "promotion accepted an unbound bootstrap receipt");
            vault.Store(
                RemoteClientOptions.ParseEndpoint(endpoint),
                new string('x', 32));
            ExpectInvalid(
                () => promotion.PromoteAsync(
                    state,
                    CancellationToken.None).GetAwaiter().GetResult(),
                "promotion accepted a conflicting endpoint credential");
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    private static RemoteBootstrapSnapshot BoundState(string endpoint, string trustHandle) => new(
        "bootstrap-target",
        "session_bound",
        "ssh",
        "target.example",
        22,
        false,
        "daemon-target",
        endpoint,
        "vault:leserpentd:target",
        trustHandle,
        null,
        true);

    private static string CreateCertificateAuthorityPem()
    {
        using var key = RSA.Create(2048);
        var request = new CertificateRequest(
            "CN=Leserpent Promotion Verification",
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
        return certificate.ExportCertificatePem().TrimEnd('\r', '\n') + "\n";
    }

    private static void SetPrivateDirectory(string path)
    {
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(
                path,
                UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        }
    }

    private static void SetPrivateFile(string path)
    {
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(path, UnixFileMode.UserRead | UnixFileMode.UserWrite);
        }
    }

    private static void ExpectInvalid(Action action, string failure)
    {
        try
        {
            action();
        }
        catch (Exception error) when (error is InvalidDataException or ArgumentException)
        {
            return;
        }
        throw new InvalidDataException(failure);
    }

    private sealed class VerificationTokenVault : IRemoteTokenVault
    {
        private readonly Dictionary<string, string> values = new(StringComparer.Ordinal);

        public string? Load(Uri endpoint) =>
            values.GetValueOrDefault(RemoteTokenResolver.Account(endpoint));

        public void Store(Uri endpoint, string token) =>
            values[RemoteTokenResolver.Account(endpoint)] = token;

        public void Delete(Uri endpoint) =>
            values.Remove(RemoteTokenResolver.Account(endpoint));
    }
}

internal sealed class BootstrapPromotionTrustFixture
{
    public required string Endpoint { get; set; }
    public required string CaPem { get; set; }
    public required string CaSha256 { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(BootstrapPromotionTrustFixture))]
internal partial class BootstrapPromotionJsonContext : JsonSerializerContext;
