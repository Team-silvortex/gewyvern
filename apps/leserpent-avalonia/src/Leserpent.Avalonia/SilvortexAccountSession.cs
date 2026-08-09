using System.ComponentModel;
using System.Diagnostics;
using System.Net;
using System.Net.Http.Headers;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

internal enum SilvortexAccountPhase
{
    Disabled,
    SignedOut,
    Working,
    SignedIn,
    Error,
}

internal sealed record SilvortexAccountSnapshot(
    SilvortexAccountPhase Phase,
    string Message,
    string? Subject = null,
    string? DisplayName = null,
    string? Email = null)
{
    public bool IsSignedIn => Phase == SilvortexAccountPhase.SignedIn;
}

internal sealed record SilvortexAccountOptions(
    Uri Issuer,
    string ClientId,
    int CallbackPort)
{
    public const string ReviewedApplicationKey = "leserpent";
    public const string ReviewedClientProfile = "leserpent_desktop";
    public const string ReviewedClientId = "svx_client_leserpent_desktop";
    public const string ReviewedScopes = "openid profile email offline_access";
    public const string CallbackPath = "/oidc/callback";
    public const string IssuerEnvironmentVariable = "LESERPENT_SILVORTEX_ISSUER";
    public const string ClientIdEnvironmentVariable = "LESERPENT_SILVORTEX_CLIENT_ID";
    public const string CallbackPortEnvironmentVariable = "LESERPENT_SILVORTEX_CALLBACK_PORT";
    public const string AllowInsecureEnvironmentVariable =
        "LESERPENT_SILVORTEX_ALLOW_INSECURE_HTTP";
    public const int DefaultCallbackPort = 43817;
    public const int MaxIssuerLength = 2048;

    public Uri RedirectUri => new($"http://127.0.0.1:{CallbackPort}{CallbackPath}");

    public string CredentialAccount => $"{Issuer.AbsoluteUri}|{ClientId}";

    internal static SilvortexAccountOptions Create(
        string issuer,
        string clientId,
        int callbackPort,
        bool allowInsecure = false)
    {
        if (issuer.Length is <= 0 or > MaxIssuerLength
            || issuer.Any(character => character > 0x7f
                || char.IsControl(character)
                || char.IsWhiteSpace(character))
            || !Uri.TryCreate(issuer, UriKind.Absolute, out var issuerUri)
            || !string.IsNullOrEmpty(issuerUri.UserInfo)
            || !string.IsNullOrEmpty(issuerUri.Query)
            || !string.IsNullOrEmpty(issuerUri.Fragment)
            || issuerUri.AbsolutePath != "/"
            || issuerUri.Port == 0
            || !IsCanonicalIssuerHost(issuerUri))
        {
            throw new InvalidDataException("Team Silvortex issuer must be an absolute origin URL.");
        }
        var loopback = issuerUri.IsLoopback;
        if (issuerUri.Scheme != Uri.UriSchemeHttps
            && !(allowInsecure && loopback && issuerUri.Scheme == Uri.UriSchemeHttp))
        {
            throw new InvalidDataException(
                "Team Silvortex issuer must use HTTPS; HTTP is limited to explicitly enabled loopback development.");
        }
        if (clientId.Length is < 16 or > 80 || clientId.Any(char.IsControl))
        {
            throw new InvalidDataException("Team Silvortex client ID is invalid.");
        }
        if (callbackPort is < 1024 or > 65535)
        {
            throw new InvalidDataException("Team Silvortex callback port is invalid.");
        }
        var normalizedIssuer = new UriBuilder(issuerUri) { Path = "/" }.Uri;
        if (!string.Equals(normalizedIssuer.AbsoluteUri, issuer, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "Team Silvortex issuer must use its canonical origin spelling.");
        }
        return new SilvortexAccountOptions(normalizedIssuer, clientId, callbackPort);
    }

    private static bool IsCanonicalIssuerHost(Uri issuer)
    {
        var host = issuer.IdnHost;
        return Uri.CheckHostName(host) switch
        {
            UriHostNameType.IPv4 or UriHostNameType.IPv6 => true,
            UriHostNameType.Dns => host.Length <= 253
                && !(host.Contains('.')
                    && host.All(character => char.IsAsciiDigit(character) || character == '.'))
                && host.Split('.').All(label => label.Length is > 0 and <= 63
                    && label.All(character => char.IsAsciiLetterLower(character)
                        || char.IsAsciiDigit(character)
                        || character == '-')
                    && label[0] != '-'
                    && label[^1] != '-'),
            _ => false,
        };
    }

    internal static string ResolveClientId(string? configured) =>
        string.IsNullOrEmpty(configured) ? ReviewedClientId : configured;
}

internal sealed class SilvortexAccountSession : IDisposable
{
    internal const string CredentialService = "org.gewyvern.leserpent.silvortex";
    private const int MaxJsonBytes = 64 * 1024;
    private const int MaxCallbackBytes = 8 * 1024;
    private const int MaxCallbackAttempts = 4;
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(20);
    private static readonly TimeSpan CallbackTimeout = TimeSpan.FromMinutes(3);

    private readonly SilvortexAccountOptions? options;
    private readonly HttpClient http;
    private readonly SemaphoreSlim operationGate = new(1, 1);
    private readonly CancellationTokenSource lifetime = new();
    private readonly object restoreSync = new();
    private OidcMetadata? metadata;
    private Task? restoreTask;
    private string? accessToken;
    private bool disposed;

    private SilvortexAccountSnapshot snapshot;

    private SilvortexAccountSession(
        SilvortexAccountOptions? options,
        string initialMessage,
        HttpMessageHandler? handler = null)
    {
        this.options = options;
        snapshot = new SilvortexAccountSnapshot(
            options is null ? SilvortexAccountPhase.Disabled : SilvortexAccountPhase.SignedOut,
            initialMessage);
        http = handler is null
            ? new HttpClient(new SocketsHttpHandler
            {
                AllowAutoRedirect = false,
                AutomaticDecompression = DecompressionMethods.Brotli
                    | DecompressionMethods.Deflate
                    | DecompressionMethods.GZip,
                ConnectTimeout = RequestTimeout,
            }, disposeHandler: true)
            : new HttpClient(handler, disposeHandler: true);
        http.Timeout = RequestTimeout;
        http.DefaultRequestHeaders.UserAgent.ParseAdd("Leserpent-Desktop/1.0");
    }

    public event Action<SilvortexAccountSnapshot>? SnapshotChanged;

    public SilvortexAccountSnapshot Snapshot => snapshot;

    internal bool SystemBrowserLaunched { get; private set; }

    internal bool AccessTokenRevocationAttempted { get; private set; }

    internal bool RefreshTokenRevocationAttempted { get; private set; }

    public static SilvortexAccountSession FromRuntimeConfiguration()
    {
        var configuration = SilvortexAccountConfigurationLoader.Load();
        return new SilvortexAccountSession(configuration.Options, configuration.Message);
    }

    internal static SilvortexAccountSession DisabledForVerification() =>
        new(null, "Team Silvortex sign-in is not configured for verification.");

    internal static SilvortexAccountSession CreateForProof(SilvortexAccountOptions options) =>
        new(options, "Team Silvortex desktop proof is ready.");

    public void BeginRestore() => _ = StartRestore();

    internal Task RestoreForProofAsync()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (options is null)
        {
            throw new InvalidOperationException(
                "Team Silvortex account restore proof requires configured options.");
        }
        return StartRestore();
    }

    public async Task SignInAsync()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (options is null || !await operationGate.WaitAsync(0, lifetime.Token))
        {
            return;
        }
        try
        {
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.Working,
                "Opening the secure Team Silvortex sign-in page..."));
            var discovered = await DiscoverAsync(lifetime.Token);
            var transaction = AuthorizationTransaction.Create();
            using var callback = new LoopbackCallbackServer(options);
            callback.Start();
            OpenSystemBrowser(BuildAuthorizationUri(discovered, options, transaction));
            SystemBrowserLaunched = true;
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.Working,
                "Complete sign-in in your browser. Leserpent is waiting for the local callback."));
            var code = await callback.ReceiveCodeAsync(transaction.State, lifetime.Token);
            var tokens = await ExchangeCodeAsync(discovered, transaction, code, lifetime.Token);
            await AcceptTokensAsync(discovered, tokens, transaction.Nonce, lifetime.Token);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error) when (IsExpectedFailure(error))
        {
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.Error,
                SafeFailure("Sign-in failed", error)));
        }
        finally
        {
            operationGate.Release();
        }
    }

    public async Task SignOutAsync()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (options is null || !await operationGate.WaitAsync(0, lifetime.Token))
        {
            return;
        }
        try
        {
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.Working,
                "Signing out of Leserpent..."));
            var refreshToken = LoadRefreshToken(options);
            if (metadata is not null)
            {
                AccessTokenRevocationAttempted = accessToken is not null;
                await TryRevokeAsync(metadata, accessToken, "access_token", lifetime.Token);
                RefreshTokenRevocationAttempted = refreshToken is not null;
                await TryRevokeAsync(metadata, refreshToken, "refresh_token", lifetime.Token);
            }
            accessToken = null;
            PlatformCredentialVault.Delete(
                CredentialService,
                options.CredentialAccount);
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.SignedOut,
                "Signed out locally. Daemon connections and offline control remain available."));
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error) when (IsExpectedFailure(error))
        {
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.Error,
                SafeFailure("Sign-out could not clear the secure session", error)));
        }
        finally
        {
            operationGate.Release();
        }
    }

    public void Dispose()
    {
        if (disposed)
        {
            return;
        }
        disposed = true;
        accessToken = null;
        lifetime.Cancel();
        http.Dispose();
        operationGate.Dispose();
        lifetime.Dispose();
    }

    public static void VerifyContract()
    {
        var options = SilvortexAccountOptions.Create(
            "https://id.example.invalid/",
            SilvortexAccountOptions.ReviewedClientId,
            SilvortexAccountOptions.DefaultCallbackPort);
        foreach (var validIssuer in new[]
        {
            "https://127.0.0.1:8443/",
            "https://[2001:db8::1]:8443/",
        })
        {
            if (SilvortexAccountOptions.Create(
                    validIssuer,
                    SilvortexAccountOptions.ReviewedClientId,
                    SilvortexAccountOptions.DefaultCallbackPort).Issuer.AbsoluteUri
                != validIssuer)
            {
                throw new InvalidDataException(
                    $"Silvortex changed a canonical issuer origin: {validIssuer}");
            }
        }
        var transaction = AuthorizationTransaction.Create();
        var metadata = new OidcMetadata(
            options.Issuer,
            new Uri(options.Issuer, "account"),
            new Uri(options.Issuer, "v1/oidc/token"),
            new Uri(options.Issuer, "v1/oidc/userinfo"),
            new Uri(options.Issuer, "v1/oidc/jwks"),
            new Uri(options.Issuer, "v1/oidc/revoke"));
        var authorizationUri = BuildAuthorizationUri(metadata, options, transaction);
        var authorization = authorizationUri.AbsoluteUri;
        var authorizationFields = ParseCallbackQuery(authorizationUri.Query);
        if (!authorizationFields.TryGetValue("response_type", out var responseType)
            || responseType != "code"
            || !authorizationFields.TryGetValue("code_challenge_method", out var challengeMethod)
            || challengeMethod != "S256"
            || !authorizationFields.TryGetValue("nonce", out var nonce)
            || nonce != transaction.Nonce
            || !authorizationFields.TryGetValue("scope", out var scope)
            || scope != SilvortexAccountOptions.ReviewedScopes
            || authorizationFields.ContainsKey("client_secret")
            || options.RedirectUri.AbsolutePath != SilvortexAccountOptions.CallbackPath
            || options.ClientId != SilvortexAccountOptions.ReviewedClientId
            || SilvortexAccountOptions.ResolveClientId(null)
                != SilvortexAccountOptions.ReviewedClientId
            || SilvortexAccountOptions.ResolveClientId("svx_client_self_hosted_fixture")
                != "svx_client_self_hosted_fixture"
            || transaction.CodeVerifier.Length != 43
            || transaction.State.Length != 43
            || transaction.Nonce.Length != 43)
        {
            throw new InvalidDataException("Silvortex native authorization contract drifted.");
        }
        var fields = CodeExchangeFields(options, transaction, "bounded-code");
        if (fields.Any(field => field.Key == "client_secret")
            || fields.Single(field => field.Key == "grant_type").Value != "authorization_code")
        {
            throw new InvalidDataException("Silvortex public token exchange contract drifted.");
        }
        ExpectInvalidCallback("?code=one&code=two&state=state");
        ExpectInvalidCallback("?code=one&state=one&state=two");
        foreach (var invalidIssuer in new[]
        {
            "https://foo&bar/",
            "https://foo=bar/",
            "https://foo;bar/",
            "https://UPPER.example.invalid/",
            "https://under_score.example.invalid/",
            "https://-prefix.example.invalid/",
            "https://suffix-.example.invalid/",
            "https://999.0.0.1/",
            "https://123/",
            "https://2130706433/",
            "https://0x7f000001/",
            "https://0x7f.0.0.1/",
            "https://0177.0.0.1/",
            "https://id.example.invalid:0/",
            "https://id.example.invalid:0443/",
            "https://id.example.invalid:443/",
            "https://[2001:0db8::1]/",
        })
        {
            ExpectInvalidIssuer(invalidIssuer);
        }
        var callbackHeader = $"GET /oidc/callback?code=bounded-code&state={transaction.State}"
            + $"&iss=https%3A%2F%2Fid.example.invalid HTTP/1.1\r\nHost: 127.0.0.1:{options.CallbackPort}";
        if (LoopbackCallbackServer.ParseRequestHeader(callbackHeader, options).AbsolutePath
            != options.RedirectUri.AbsolutePath)
        {
            throw new InvalidDataException("Silvortex loopback callback projection drifted.");
        }
        ExpectInvalidCallbackHeader(
            "GET /oidc/callback?code=bounded-code HTTP/1.1\r\nHost: attacker.invalid",
            options);
        SilvortexAccountConfigurationLoader.VerifyContract();
        VerifyCryptographicContractAsync(options, metadata).GetAwaiter().GetResult();
    }

    internal static bool HasStoredRefreshToken(SilvortexAccountOptions configured) =>
        LoadRefreshToken(configured) is not null;

    internal static byte[] StoredRefreshTokenDigest(SilvortexAccountOptions configured)
    {
        var token = LoadRefreshToken(configured)
            ?? throw new InvalidDataException(
                "The Team Silvortex refresh credential is absent from the platform vault.");
        var bytes = Encoding.UTF8.GetBytes(token);
        try
        {
            return SHA256.HashData(bytes);
        }
        finally
        {
            CryptographicOperations.ZeroMemory(bytes);
        }
    }

    internal static void DeleteStoredRefreshToken(SilvortexAccountOptions configured) =>
        PlatformCredentialVault.Delete(CredentialService, configured.CredentialAccount);

    private Task StartRestore()
    {
        lock (restoreSync)
        {
            if (options is null || disposed)
            {
                return Task.CompletedTask;
            }
            return restoreTask ??= RestoreAsync();
        }
    }

    private async Task RestoreAsync()
    {
        if (options is null)
        {
            return;
        }
        await operationGate.WaitAsync(lifetime.Token);
        try
        {
            var refreshToken = LoadRefreshToken(options);
            if (refreshToken is null)
            {
                SetSnapshot(new SilvortexAccountSnapshot(
                    SilvortexAccountPhase.SignedOut,
                    "Sign in to connect this desktop client to your Team Silvortex account."));
                return;
            }
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.Working,
                "Restoring the protected Team Silvortex session..."));
            var discovered = await DiscoverAsync(lifetime.Token);
            var tokens = await RefreshAsync(discovered, refreshToken, lifetime.Token);
            await AcceptTokensAsync(discovered, tokens, null, lifetime.Token);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error) when (IsExpectedFailure(error))
        {
            SetSnapshot(new SilvortexAccountSnapshot(
                SilvortexAccountPhase.Error,
                SafeFailure("The saved account session could not be restored", error)));
        }
        finally
        {
            operationGate.Release();
        }
    }

    private async Task<OidcMetadata> DiscoverAsync(CancellationToken cancellationToken)
    {
        if (metadata is not null)
        {
            return metadata;
        }
        var configured = options ?? throw new InvalidOperationException("Silvortex is not configured.");
        var discoveryUri = new Uri(configured.Issuer, ".well-known/openid-configuration");
        using var request = new HttpRequestMessage(HttpMethod.Get, discoveryUri);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        using var response = await http.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        response.EnsureSuccessStatusCode();
        using var document = await ReadJsonAsync(response, cancellationToken);
        RequireUniqueObject(document.RootElement, "OIDC discovery");
        var issuer = RequiredUri(document.RootElement, "issuer");
        if (issuer != configured.Issuer)
        {
            throw new InvalidDataException("OIDC discovery issuer does not match the configured issuer.");
        }
        RequireStringArrayContains(document.RootElement, "response_types_supported", "code");
        RequireStringArrayContains(
            document.RootElement,
            "grant_types_supported",
            "authorization_code",
            "refresh_token");
        RequireStringArrayContains(
            document.RootElement,
            "code_challenge_methods_supported",
            "S256");
        RequireStringArrayContains(
            document.RootElement,
            "id_token_signing_alg_values_supported",
            "RS256");
        RequireStringArrayContains(
            document.RootElement,
            "token_endpoint_auth_methods_supported",
            "none");
        RequireStringArrayContains(
            document.RootElement,
            "scopes_supported",
            "openid",
            "profile",
            "email",
            "offline_access");
        if (!document.RootElement.TryGetProperty(
                "authorization_response_iss_parameter_supported",
                out var responseIssuer)
            || responseIssuer.ValueKind != JsonValueKind.True)
        {
            throw new InvalidDataException("OIDC authorization response issuer binding is unavailable.");
        }
        var discovered = new OidcMetadata(
            issuer,
            RequiredUri(document.RootElement, "authorization_endpoint"),
            RequiredUri(document.RootElement, "token_endpoint"),
            RequiredUri(document.RootElement, "userinfo_endpoint"),
            RequiredUri(document.RootElement, "jwks_uri"),
            RequiredUri(document.RootElement, "revocation_endpoint"));
        foreach (var endpoint in discovered.Endpoints)
        {
            RequireSameOrigin(configured.Issuer, endpoint);
        }
        metadata = discovered;
        return discovered;
    }

    private async Task<TokenSet> ExchangeCodeAsync(
        OidcMetadata discovered,
        AuthorizationTransaction transaction,
        string code,
        CancellationToken cancellationToken) =>
        await RequestTokensAsync(
            discovered.TokenEndpoint,
            CodeExchangeFields(options!, transaction, code),
            cancellationToken);

    private async Task<TokenSet> RefreshAsync(
        OidcMetadata discovered,
        string refreshToken,
        CancellationToken cancellationToken) =>
        await RequestTokensAsync(
            discovered.TokenEndpoint,
            [
                new("grant_type", "refresh_token"),
                new("client_id", options!.ClientId),
                new("refresh_token", refreshToken),
            ],
            cancellationToken);

    private async Task<TokenSet> RequestTokensAsync(
        Uri endpoint,
        IReadOnlyList<KeyValuePair<string, string>> fields,
        CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(HttpMethod.Post, endpoint)
        {
            Content = new FormUrlEncodedContent(fields),
        };
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        using var response = await http.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        using var document = await ReadJsonAsync(response, cancellationToken);
        RequireUniqueObject(document.RootElement, "OIDC token response");
        if (!response.IsSuccessStatusCode)
        {
            var code = OptionalString(document.RootElement, "error", 64) ?? "token_request_rejected";
            throw new InvalidDataException($"Team Silvortex rejected the token request ({code}).");
        }
        var tokenType = RequiredString(document.RootElement, "token_type", 32);
        if (!string.Equals(tokenType, "Bearer", StringComparison.Ordinal))
        {
            throw new InvalidDataException("OIDC token type is not Bearer.");
        }
        var access = RequiredString(document.RootElement, "access_token", 512, minimum: 32);
        var identity = RequiredString(document.RootElement, "id_token", 8192, minimum: 64);
        var refresh = RequiredString(document.RootElement, "refresh_token", 512, minimum: 32);
        return new TokenSet(access, identity, refresh);
    }

    private async Task AcceptTokensAsync(
        OidcMetadata discovered,
        TokenSet tokens,
        string? expectedNonce,
        CancellationToken cancellationToken)
    {
        var identity = await VerifyIdTokenAsync(
            discovered,
            tokens.IdToken,
            expectedNonce,
            cancellationToken);
        var profile = await LoadUserInfoAsync(
            discovered.UserInfoEndpoint,
            tokens.AccessToken,
            cancellationToken);
        if (!FixedTimeEquals(identity.Subject, profile.Subject))
        {
            throw new InvalidDataException("OIDC UserInfo subject does not match the ID token.");
        }
        PlatformCredentialVault.Store(
            CredentialService,
            options!.CredentialAccount,
            tokens.RefreshToken);
        accessToken = tokens.AccessToken;
        SetSnapshot(new SilvortexAccountSnapshot(
            SilvortexAccountPhase.SignedIn,
            "Authenticated with MFA. Daemon credentials remain independently scoped.",
            profile.Subject,
            profile.DisplayName,
            profile.Email));
    }

    private async Task<VerifiedIdentity> VerifyIdTokenAsync(
        OidcMetadata discovered,
        string token,
        string? expectedNonce,
        CancellationToken cancellationToken)
    {
        var segments = token.Split('.');
        if (segments.Length != 3 || segments.Any(segment => segment.Length == 0))
        {
            throw new InvalidDataException("OIDC ID token is malformed.");
        }
        using var header = ParseJwtSegment(segments[0], 4096, "ID token header");
        using var claims = ParseJwtSegment(segments[1], 8192, "ID token claims");
        RequireUniqueObject(header.RootElement, "ID token header");
        RequireUniqueObject(claims.RootElement, "ID token claims");
        if (RequiredString(header.RootElement, "alg", 16) != "RS256")
        {
            throw new InvalidDataException("OIDC ID token algorithm is not RS256.");
        }
        var keyId = RequiredString(header.RootElement, "kid", 128);
        using var key = await LoadSigningKeyAsync(discovered.JwksUri, keyId, cancellationToken);
        var signed = Encoding.ASCII.GetBytes($"{segments[0]}.{segments[1]}");
        var signature = DecodeBase64Url(segments[2], 1024, "ID token signature");
        if (!key.VerifyData(signed, signature, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1))
        {
            throw new CryptographicException("OIDC ID token signature is invalid.");
        }
        var issuer = RequiredString(claims.RootElement, "iss", 2048);
        var subject = RequiredString(claims.RootElement, "sub", 64);
        var audience = RequiredString(claims.RootElement, "aud", 80);
        var nonce = OptionalString(claims.RootElement, "nonce", 512);
        if (!FixedTimeEquals(issuer, discovered.Issuer.AbsoluteUri.TrimEnd('/'))
            || !FixedTimeEquals(audience, options!.ClientId)
            || !ValidSubject(subject)
            || expectedNonce is not null && !FixedTimeEquals(nonce, expectedNonce)
            || expectedNonce is null && nonce is not null)
        {
            throw new InvalidDataException("OIDC ID token binding is invalid.");
        }
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var issuedAt = RequiredInt64(claims.RootElement, "iat");
        var expiresAt = RequiredInt64(claims.RootElement, "exp");
        if (issuedAt > now + 60 || expiresAt <= now - 60 || expiresAt - issuedAt != 15 * 60)
        {
            throw new InvalidDataException("OIDC ID token lifetime is invalid.");
        }
        if (RequiredString(claims.RootElement, "acr", 128)
            != "urn:silvortex:assurance:mfa")
        {
            throw new InvalidDataException("OIDC session does not carry the required MFA assurance.");
        }
        return new VerifiedIdentity(subject);
    }

    private async Task<RSA> LoadSigningKeyAsync(
        Uri jwksUri,
        string keyId,
        CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get, jwksUri);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        using var response = await http.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        response.EnsureSuccessStatusCode();
        using var document = await ReadJsonAsync(response, cancellationToken);
        RequireUniqueObject(document.RootElement, "OIDC JWKS");
        if (!document.RootElement.TryGetProperty("keys", out var keys)
            || keys.ValueKind != JsonValueKind.Array
            || keys.GetArrayLength() is <= 0 or > 5)
        {
            throw new InvalidDataException("OIDC JWKS key set is invalid.");
        }
        JsonElement? match = null;
        foreach (var candidate in keys.EnumerateArray())
        {
            RequireUniqueObject(candidate, "OIDC JWK");
            if (OptionalString(candidate, "kid", 128) == keyId)
            {
                if (match is not null)
                {
                    throw new InvalidDataException("OIDC JWKS contains a duplicate signing key ID.");
                }
                match = candidate;
            }
        }
        var jwk = match ?? throw new InvalidDataException("OIDC signing key is unavailable.");
        if (RequiredString(jwk, "kty", 16) != "RSA"
            || RequiredString(jwk, "alg", 16) != "RS256"
            || OptionalString(jwk, "use", 16) is { } use && use != "sig")
        {
            throw new InvalidDataException("OIDC signing key contract is invalid.");
        }
        var modulus = DecodeBase64Url(RequiredString(jwk, "n", 2048), 1024, "JWK modulus");
        var exponent = DecodeBase64Url(RequiredString(jwk, "e", 16), 8, "JWK exponent");
        if (modulus.Length is < 256 or > 1024 || exponent.Length is <= 0 or > 8)
        {
            throw new InvalidDataException("OIDC signing key size is invalid.");
        }
        var rsa = RSA.Create();
        try
        {
            rsa.ImportParameters(new RSAParameters { Modulus = modulus, Exponent = exponent });
            return rsa;
        }
        catch
        {
            rsa.Dispose();
            throw;
        }
    }

    private async Task<AccountProfile> LoadUserInfoAsync(
        Uri endpoint,
        string token,
        CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(HttpMethod.Get, endpoint);
        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        using var response = await http.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        response.EnsureSuccessStatusCode();
        using var document = await ReadJsonAsync(response, cancellationToken);
        RequireUniqueObject(document.RootElement, "OIDC UserInfo");
        var subject = RequiredString(document.RootElement, "sub", 64);
        if (!ValidSubject(subject))
        {
            throw new InvalidDataException("OIDC UserInfo subject is invalid.");
        }
        var displayName = OptionalString(document.RootElement, "name", 120);
        var email = OptionalString(document.RootElement, "email", 320);
        if (email is not null
            && (!document.RootElement.TryGetProperty("email_verified", out var verified)
                || verified.ValueKind != JsonValueKind.True))
        {
            throw new InvalidDataException("OIDC UserInfo email is not verified.");
        }
        return new AccountProfile(subject, displayName, email);
    }

    private async Task TryRevokeAsync(
        OidcMetadata discovered,
        string? token,
        string hint,
        CancellationToken cancellationToken)
    {
        if (string.IsNullOrEmpty(token))
        {
            return;
        }
        try
        {
            using var request = new HttpRequestMessage(HttpMethod.Post, discovered.RevocationEndpoint)
            {
                Content = new FormUrlEncodedContent([
                    new("token", token),
                    new("token_type_hint", hint),
                    new("client_id", options!.ClientId),
                ]),
            };
            using var response = await http.SendAsync(
                request,
                HttpCompletionOption.ResponseHeadersRead,
                cancellationToken);
            _ = response.IsSuccessStatusCode;
        }
        catch (Exception error) when (IsExpectedFailure(error))
        {
            // Local sign-out must remain available when the identity service is offline.
        }
    }

    private static Uri BuildAuthorizationUri(
        OidcMetadata discovered,
        SilvortexAccountOptions configured,
        AuthorizationTransaction transaction)
    {
        var fields = new[]
        {
            new KeyValuePair<string, string>("response_type", "code"),
            new KeyValuePair<string, string>("client_id", configured.ClientId),
            new KeyValuePair<string, string>("redirect_uri", configured.RedirectUri.AbsoluteUri),
            new KeyValuePair<string, string>("scope", SilvortexAccountOptions.ReviewedScopes),
            new KeyValuePair<string, string>("state", transaction.State),
            new KeyValuePair<string, string>("nonce", transaction.Nonce),
            new KeyValuePair<string, string>("code_challenge", transaction.CodeChallenge),
            new KeyValuePair<string, string>("code_challenge_method", "S256"),
        };
        var builder = new UriBuilder(discovered.AuthorizationEndpoint)
        {
            Query = FormQuery(fields),
        };
        return builder.Uri;
    }

    private static IReadOnlyList<KeyValuePair<string, string>> CodeExchangeFields(
        SilvortexAccountOptions configured,
        AuthorizationTransaction transaction,
        string code) =>
        [
            new("grant_type", "authorization_code"),
            new("client_id", configured.ClientId),
            new("code", code),
            new("redirect_uri", configured.RedirectUri.AbsoluteUri),
            new("code_verifier", transaction.CodeVerifier),
        ];

    private static string FormQuery(IEnumerable<KeyValuePair<string, string>> fields) =>
        string.Join("&", fields.Select(field =>
            $"{Uri.EscapeDataString(field.Key)}={Uri.EscapeDataString(field.Value)}"));

    private static void OpenSystemBrowser(Uri authorizationUri)
    {
        try
        {
            _ = Process.Start(new ProcessStartInfo
            {
                FileName = authorizationUri.AbsoluteUri,
                UseShellExecute = true,
            }) ?? throw new InvalidOperationException("The system browser did not start.");
        }
        catch (Win32Exception error)
        {
            throw new InvalidOperationException("The system browser could not be opened.", error);
        }
    }

    private static string? LoadRefreshToken(SilvortexAccountOptions configured)
    {
        var token = PlatformCredentialVault.Load(
            CredentialService,
            configured.CredentialAccount);
        if (token is not null
            && (token.Length is < 32 or > 512 || token.Any(char.IsWhiteSpace)))
        {
            throw new InvalidDataException("The stored Team Silvortex refresh token is invalid.");
        }
        return token;
    }

    private async Task<JsonDocument> ReadJsonAsync(
        HttpResponseMessage response,
        CancellationToken cancellationToken)
    {
        if (response.Content.Headers.ContentLength is > MaxJsonBytes)
        {
            throw new InvalidDataException("Team Silvortex response exceeds the size limit.");
        }
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken);
        using var buffer = new MemoryStream();
        var chunk = new byte[4096];
        while (true)
        {
            var read = await stream.ReadAsync(chunk, cancellationToken);
            if (read == 0)
            {
                break;
            }
            if (buffer.Length + read > MaxJsonBytes)
            {
                throw new InvalidDataException("Team Silvortex response exceeds the size limit.");
            }
            buffer.Write(chunk, 0, read);
        }
        return JsonDocument.Parse(buffer.ToArray(), new JsonDocumentOptions
        {
            AllowTrailingCommas = false,
            CommentHandling = JsonCommentHandling.Disallow,
            MaxDepth = 16,
        });
    }

    private static Uri RequiredUri(JsonElement value, string name)
    {
        var text = RequiredString(value, name, 2048);
        if (!Uri.TryCreate(text, UriKind.Absolute, out var uri)
            || !string.IsNullOrEmpty(uri.UserInfo)
            || !string.IsNullOrEmpty(uri.Query)
            || !string.IsNullOrEmpty(uri.Fragment))
        {
            throw new InvalidDataException($"OIDC {name} is invalid.");
        }
        return uri;
    }

    private static void RequireSameOrigin(Uri issuer, Uri endpoint)
    {
        if (issuer.Scheme != endpoint.Scheme
            || issuer.Host != endpoint.Host
            || issuer.Port != endpoint.Port)
        {
            throw new InvalidDataException("OIDC endpoint is outside the configured issuer origin.");
        }
    }

    private static JsonDocument ParseJwtSegment(string segment, int maximum, string label)
    {
        var bytes = DecodeBase64Url(segment, maximum, label);
        return JsonDocument.Parse(bytes, new JsonDocumentOptions
        {
            AllowTrailingCommas = false,
            CommentHandling = JsonCommentHandling.Disallow,
            MaxDepth = 8,
        });
    }

    private static byte[] DecodeBase64Url(string value, int maximum, string label)
    {
        if (value.Length is <= 0 or > 4 * 1024
            || value.Any(character => !char.IsAsciiLetterOrDigit(character)
                && character is not ('-' or '_')))
        {
            throw new InvalidDataException($"{label} is not canonical Base64URL.");
        }
        var padded = value.Replace('-', '+').Replace('_', '/');
        padded += (padded.Length % 4) switch
        {
            0 => string.Empty,
            2 => "==",
            3 => "=",
            _ => throw new InvalidDataException($"{label} is not canonical Base64URL."),
        };
        byte[] bytes;
        try
        {
            bytes = Convert.FromBase64String(padded);
        }
        catch (FormatException error)
        {
            throw new InvalidDataException($"{label} is not canonical Base64URL.", error);
        }
        if (bytes.Length is <= 0 || bytes.Length > maximum || Base64Url(bytes) != value)
        {
            throw new InvalidDataException($"{label} is not canonical Base64URL.");
        }
        return bytes;
    }

    private static string Base64Url(ReadOnlySpan<byte> value) =>
        Convert.ToBase64String(value).TrimEnd('=').Replace('+', '-').Replace('/', '_');

    private static void RequireUniqueObject(JsonElement value, string label)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new InvalidDataException($"{label} must be a JSON object.");
        }
        var names = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!names.Add(property.Name))
            {
                throw new InvalidDataException($"{label} contains duplicate members.");
            }
        }
    }

    private static string RequiredString(
        JsonElement value,
        string name,
        int maximum,
        int minimum = 1) =>
        OptionalString(value, name, maximum, minimum)
            ?? throw new InvalidDataException($"OIDC {name} is required.");

    private static string? OptionalString(
        JsonElement value,
        string name,
        int maximum,
        int minimum = 1)
    {
        if (!value.TryGetProperty(name, out var property))
        {
            return null;
        }
        if (property.ValueKind != JsonValueKind.String)
        {
            throw new InvalidDataException($"OIDC {name} must be a string.");
        }
        var text = property.GetString()!;
        if (text.Length < minimum || text.Length > maximum || text.Any(char.IsControl))
        {
            throw new InvalidDataException($"OIDC {name} is invalid.");
        }
        return text;
    }

    private static long RequiredInt64(JsonElement value, string name)
    {
        if (!value.TryGetProperty(name, out var property)
            || property.ValueKind != JsonValueKind.Number
            || !property.TryGetInt64(out var number))
        {
            throw new InvalidDataException($"OIDC {name} must be an integer.");
        }
        return number;
    }

    private static void RequireStringArrayContains(
        JsonElement value,
        string name,
        params string[] required)
    {
        if (!value.TryGetProperty(name, out var property)
            || property.ValueKind != JsonValueKind.Array
            || property.GetArrayLength() is <= 0 or > 16)
        {
            throw new InvalidDataException($"OIDC {name} is invalid.");
        }
        var values = new HashSet<string>(StringComparer.Ordinal);
        foreach (var item in property.EnumerateArray())
        {
            if (item.ValueKind != JsonValueKind.String
                || item.GetString() is not { Length: > 0 and <= 128 } text
                || !values.Add(text))
            {
                throw new InvalidDataException($"OIDC {name} is invalid.");
            }
        }
        if (required.Any(item => !values.Contains(item)))
        {
            throw new InvalidDataException($"OIDC {name} does not satisfy the native client contract.");
        }
    }

    private static bool ValidSubject(string value) =>
        value.Length == 24
        && value.StartsWith("svx_", StringComparison.Ordinal)
        && value.AsSpan(4).IndexOfAnyExcept("0123456789abcdef") < 0;

    private static bool FixedTimeEquals(string? left, string? right)
    {
        if (left is null || right is null)
        {
            return left is null && right is null;
        }
        var leftBytes = Encoding.UTF8.GetBytes(left);
        var rightBytes = Encoding.UTF8.GetBytes(right);
        return leftBytes.Length == rightBytes.Length
            && CryptographicOperations.FixedTimeEquals(leftBytes, rightBytes);
    }

    private static Dictionary<string, string> ParseCallbackQuery(string query)
    {
        if (query.Length is <= 1 or > 4096 || query[0] != '?')
        {
            throw new InvalidDataException("OIDC callback query is invalid.");
        }
        var values = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var pair in query[1..].Split('&'))
        {
            if (pair.Length == 0)
            {
                throw new InvalidDataException("OIDC callback query is invalid.");
            }
            var split = pair.Split('=', 2);
            var name = DecodeQueryComponent(split[0]);
            var value = DecodeQueryComponent(split.Length == 2 ? split[1] : string.Empty);
            if (string.IsNullOrEmpty(name)
                || name.Length > 64
                || value.Length > 2048
                || !values.TryAdd(name, value))
            {
                throw new InvalidDataException("OIDC callback contains invalid or duplicate parameters.");
            }
        }
        return values;
    }

    private static string DecodeQueryComponent(string value)
    {
        if (value.Any(character => character > 0x7f || char.IsControl(character)))
        {
            throw new InvalidDataException("OIDC callback query encoding is invalid.");
        }
        for (var index = 0; index < value.Length; index++)
        {
            if (value[index] == '%'
                && (index + 2 >= value.Length
                    || !Uri.IsHexDigit(value[index + 1])
                    || !Uri.IsHexDigit(value[index + 2])))
            {
                throw new InvalidDataException("OIDC callback query encoding is invalid.");
            }
        }
        var decoded = WebUtility.UrlDecode(value);
        if (decoded.Any(char.IsControl))
        {
            throw new InvalidDataException("OIDC callback query encoding is invalid.");
        }
        return decoded;
    }

    private static void ExpectInvalidCallback(string query)
    {
        try
        {
            _ = ParseCallbackQuery(query);
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException("Silvortex callback accepted duplicate parameters.");
    }

    private static void ExpectInvalidIssuer(string issuer)
    {
        try
        {
            _ = SilvortexAccountOptions.Create(
                issuer,
                SilvortexAccountOptions.ReviewedClientId,
                SilvortexAccountOptions.DefaultCallbackPort);
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException(
            $"Silvortex accepted a non-canonical issuer origin: {issuer}");
    }

    private static void ExpectInvalidCallbackHeader(
        string header,
        SilvortexAccountOptions options)
    {
        try
        {
            _ = LoopbackCallbackServer.ParseRequestHeader(header, options);
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException("Silvortex callback accepted an invalid HTTP authority.");
    }

    private static async Task VerifyCryptographicContractAsync(
        SilvortexAccountOptions options,
        OidcMetadata metadata)
    {
        using var signingKey = RSA.Create(2048);
        var publicKey = signingKey.ExportParameters(false);
        const string keyId = "silvortex-contract-key";
        const string nonce = "0123456789abcdef0123456789abcdef";
        const string subject = "svx_0123456789abcdef0123";
        var now = DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        var header = Base64Url(Encoding.UTF8.GetBytes(
            $"{{\"alg\":\"RS256\",\"kid\":\"{keyId}\"}}"));
        var claims = Base64Url(Encoding.UTF8.GetBytes(
            $"{{\"iss\":\"{options.Issuer.AbsoluteUri.TrimEnd('/')}\","
            + $"\"sub\":\"{subject}\",\"aud\":\"{options.ClientId}\","
            + $"\"iat\":{now},\"exp\":{now + 15 * 60},\"nonce\":\"{nonce}\","
            + "\"acr\":\"urn:silvortex:assurance:mfa\"}"));
        var signed = Encoding.ASCII.GetBytes($"{header}.{claims}");
        var signature = signingKey.SignData(
            signed,
            HashAlgorithmName.SHA256,
            RSASignaturePadding.Pkcs1);
        var token = $"{header}.{claims}.{Base64Url(signature)}";
        var jwks = "{\"keys\":[{\"kty\":\"RSA\",\"use\":\"sig\","
            + $"\"alg\":\"RS256\",\"kid\":\"{keyId}\","
            + $"\"n\":\"{Base64Url(publicKey.Modulus!)}\","
            + $"\"e\":\"{Base64Url(publicKey.Exponent!)}\"}}]}}";
        using var session = new SilvortexAccountSession(
            options,
            "verification",
            new FixtureOidcHandler(jwks));
        var identity = await session.VerifyIdTokenAsync(
            metadata,
            token,
            nonce,
            CancellationToken.None);
        if (identity.Subject != subject)
        {
            throw new InvalidDataException("Silvortex ID token subject projection drifted.");
        }
        signature[0] ^= 0x01;
        try
        {
            _ = await session.VerifyIdTokenAsync(
                metadata,
                $"{header}.{claims}.{Base64Url(signature)}",
                nonce,
                CancellationToken.None);
        }
        catch (CryptographicException)
        {
            return;
        }
        throw new InvalidDataException("Silvortex ID token accepted a modified signature.");
    }

    private void SetSnapshot(SilvortexAccountSnapshot value)
    {
        snapshot = value;
        SnapshotChanged?.Invoke(value);
    }

    private static bool IsExpectedFailure(Exception error) => error is
        HttpRequestException
        or IOException
        or JsonException
        or CryptographicException
        or InvalidDataException
        or InvalidOperationException
        or OperationCanceledException
        or PlatformNotSupportedException
        or SocketException
        or Win32Exception;

    private static string SafeFailure(string prefix, Exception error)
    {
        var message = error.Message.Replace('\r', ' ').Replace('\n', ' ').Trim();
        if (message.Length > 240)
        {
            message = message[..240];
        }
        return string.IsNullOrEmpty(message) ? prefix : $"{prefix}: {message}";
    }

    private sealed record OidcMetadata(
        Uri Issuer,
        Uri AuthorizationEndpoint,
        Uri TokenEndpoint,
        Uri UserInfoEndpoint,
        Uri JwksUri,
        Uri RevocationEndpoint)
    {
        public IEnumerable<Uri> Endpoints
        {
            get
            {
                yield return AuthorizationEndpoint;
                yield return TokenEndpoint;
                yield return UserInfoEndpoint;
                yield return JwksUri;
                yield return RevocationEndpoint;
            }
        }
    }

    private sealed record AuthorizationTransaction(
        string State,
        string Nonce,
        string CodeVerifier,
        string CodeChallenge)
    {
        public static AuthorizationTransaction Create()
        {
            var verifier = Base64Url(RandomNumberGenerator.GetBytes(32));
            return new AuthorizationTransaction(
                Base64Url(RandomNumberGenerator.GetBytes(32)),
                Base64Url(RandomNumberGenerator.GetBytes(32)),
                verifier,
                Base64Url(SHA256.HashData(Encoding.ASCII.GetBytes(verifier))));
        }
    }

    private sealed record TokenSet(string AccessToken, string IdToken, string RefreshToken);
    private sealed record VerifiedIdentity(string Subject);
    private sealed record AccountProfile(string Subject, string? DisplayName, string? Email);

    private sealed class FixtureOidcHandler(string jwks) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) =>
            Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
            {
                RequestMessage = request,
                Content = new StringContent(jwks, Encoding.UTF8, "application/json"),
            });
    }

    private sealed class LoopbackCallbackServer(SilvortexAccountOptions options) : IDisposable
    {
        private readonly TcpListener listener = new(IPAddress.Loopback, options.CallbackPort);
        private bool started;

        public void Start()
        {
            listener.Server.ExclusiveAddressUse = true;
            listener.Start(1);
            started = true;
        }

        public async Task<string> ReceiveCodeAsync(
            string expectedState,
            CancellationToken cancellationToken)
        {
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
            timeout.CancelAfter(CallbackTimeout);
            for (var attempt = 0; attempt < MaxCallbackAttempts; attempt++)
            {
                using var client = await listener.AcceptTcpClientAsync(timeout.Token);
                client.ReceiveTimeout = checked((int)RequestTimeout.TotalMilliseconds);
                client.SendTimeout = checked((int)RequestTimeout.TotalMilliseconds);
                try
                {
                    var callback = await ReadCallbackAsync(client, timeout.Token);
                    var values = ParseCallbackQuery(callback.Query);
                    if (!values.TryGetValue("state", out var state)
                        || !FixedTimeEquals(state, expectedState))
                    {
                        throw new InvalidDataException("OIDC callback state is invalid.");
                    }
                    if (!values.TryGetValue("iss", out var issuer)
                        || !FixedTimeEquals(
                            issuer,
                            options.Issuer.AbsoluteUri.TrimEnd('/')))
                    {
                        throw new InvalidDataException("OIDC callback issuer is invalid.");
                    }
                    if (values.TryGetValue("error", out var error))
                    {
                        await RespondAsync(client, false, timeout.Token);
                        throw new InvalidOperationException(
                            $"Team Silvortex denied authorization ({BoundedCode(error)}).");
                    }
                    if (!values.TryGetValue("code", out var code)
                        || code.Length is < 16 or > 512
                        || code.Any(char.IsWhiteSpace))
                    {
                        throw new InvalidDataException("OIDC callback code is invalid.");
                    }
                    await RespondAsync(client, true, timeout.Token);
                    return code;
                }
                catch (InvalidDataException)
                {
                    await RespondAsync(client, false, timeout.Token);
                    if (attempt + 1 >= MaxCallbackAttempts)
                    {
                        throw;
                    }
                }
            }
            throw new InvalidDataException("OIDC callback did not contain a valid authorization response.");
        }

        public void Dispose()
        {
            if (started)
            {
                listener.Stop();
            }
        }

        private async Task<Uri> ReadCallbackAsync(
            TcpClient client,
            CancellationToken cancellationToken)
        {
            var stream = client.GetStream();
            var bytes = new byte[MaxCallbackBytes + 1];
            var length = 0;
            var headerEnd = -1;
            while (length < bytes.Length)
            {
                var read = await stream.ReadAsync(bytes.AsMemory(length), cancellationToken);
                if (read == 0)
                {
                    break;
                }
                length += read;
                if (length >= 4
                    && (headerEnd = bytes.AsSpan(0, length).IndexOf("\r\n\r\n"u8)) >= 0)
                {
                    break;
                }
            }
            if (length == 0 || length > MaxCallbackBytes || headerEnd < 0)
            {
                throw new InvalidDataException("OIDC callback request exceeds the size limit.");
            }
            return ParseRequestHeader(Encoding.ASCII.GetString(bytes, 0, headerEnd), options);
        }

        public static Uri ParseRequestHeader(
            string request,
            SilvortexAccountOptions options)
        {
            var lines = request.Split("\r\n", StringSplitOptions.None);
            if (lines.Length < 2)
            {
                throw new InvalidDataException("OIDC callback request line is invalid.");
            }
            var parts = lines[0].Split(' ');
            if (parts is not ["GET", var target, "HTTP/1.1"]
                || !target.StartsWith("/", StringComparison.Ordinal)
                || !Uri.TryCreate($"http://127.0.0.1:{options.CallbackPort}{target}", UriKind.Absolute, out var uri)
                || uri.AbsolutePath != options.RedirectUri.AbsolutePath)
            {
                throw new InvalidDataException("OIDC callback target is invalid.");
            }
            var headers = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (var line in lines.Skip(1))
            {
                var separator = line.IndexOf(':');
                if (separator <= 0)
                {
                    throw new InvalidDataException("OIDC callback header is invalid.");
                }
                var name = line[..separator];
                var value = line[(separator + 1)..].Trim();
                if (!name.All(character => char.IsAsciiLetterOrDigit(character) || character == '-')
                    || value.Any(char.IsControl)
                    || !headers.TryAdd(name, value))
                {
                    throw new InvalidDataException("OIDC callback header is invalid.");
                }
            }
            if (!headers.TryGetValue("Host", out var host)
                || host != $"127.0.0.1:{options.CallbackPort}"
                || headers.ContainsKey("Transfer-Encoding")
                || headers.TryGetValue("Content-Length", out var contentLength)
                    && contentLength != "0")
            {
                throw new InvalidDataException("OIDC callback authority or body is invalid.");
            }
            return uri;
        }

        private static async Task RespondAsync(
            TcpClient client,
            bool success,
            CancellationToken cancellationToken)
        {
            if (!client.Connected)
            {
                return;
            }
            var title = success ? "Sign-in complete" : "Invalid callback";
            var body = Encoding.UTF8.GetBytes(
                $"<!doctype html><meta charset=utf-8><title>{title}</title>"
                + "<style>body{font:16px sans-serif;background:#11100d;color:#e9e1d0;padding:3rem}"
                + "strong{color:#f4c95d}</style>"
                + $"<strong>{title}</strong><p>{(success ? "Return to Leserpent." : "This callback was not accepted.")}</p>");
            var status = success ? "200 OK" : "400 Bad Request";
            var headers = Encoding.ASCII.GetBytes(
                $"HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n"
                + "Cache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n"
                + $"Content-Length: {body.Length}\r\nConnection: close\r\n\r\n");
            await using var stream = client.GetStream();
            await stream.WriteAsync(headers, cancellationToken);
            await stream.WriteAsync(body, cancellationToken);
            await stream.FlushAsync(cancellationToken);
        }

        private static string BoundedCode(string value) =>
            value.Length is > 0 and <= 64
                && value.All(character => char.IsAsciiLetterOrDigit(character)
                    || character is '_' or '-')
                ? value
                : "authorization_error";
    }
}
