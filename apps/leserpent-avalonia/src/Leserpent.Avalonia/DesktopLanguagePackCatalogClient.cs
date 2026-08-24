using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

internal sealed record DesktopLanguagePackSource(
    string SourceId,
    string DisplayName,
    Uri Endpoint,
    string CertificateAuthorityPath)
{
    public override string ToString() =>
        $"{DisplayName} · {Endpoint.GetLeftPart(UriPartial.Authority)}";

    public static DesktopLanguagePackSource FromConnection(
        DesktopDaemonConnection connection,
        string certificateAuthorityPath) => new(
            connection.DaemonId,
            connection.DisplayName,
            RemoteClientOptions.ParseEndpoint(connection.Profile.Endpoint),
            certificateAuthorityPath);

    public static DesktopLanguagePackSource FromLocal(
        DesktopProductStartupPlan plan) => new(
            "local-orchestra",
            "Local Orchestra",
            plan.Options.Endpoint,
            plan.Options.CertificateAuthorityPath);
}

internal sealed record DesktopLanguagePackDownload(
    string SourceId,
    string Locale,
    string Version,
    string Sha256,
    byte[] Payload);

internal sealed class DesktopLanguagePackCatalogClient : IDisposable
{
    public const string Schema = "leserpent.language-pack-catalog/v1";
    public const int MaxCatalogBytes = 128 * 1024;
    private const int DownloadableLocaleCount = 22;
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(10);
    private readonly DesktopLanguagePackSource source;
    private readonly HttpClient http;
    private readonly X509Certificate2? trustedRoot;

    public DesktopLanguagePackCatalogClient(DesktopLanguagePackSource source)
    {
        ValidateSource(source, requireCertificate: true);
        this.source = source;
        trustedRoot = RemoteTls.LoadRoot(source.CertificateAuthorityPath);
        var handler = new SocketsHttpHandler
        {
            AllowAutoRedirect = false,
            AutomaticDecompression = DecompressionMethods.None,
            ConnectTimeout = RequestTimeout,
            MaxConnectionsPerServer = 2,
            PooledConnectionLifetime = TimeSpan.FromMinutes(2),
            UseCookies = false,
        };
        handler.SslOptions.RemoteCertificateValidationCallback =
            (_, certificate, _, errors) =>
                RemoteTls.ValidateServerCertificate(certificate, errors, trustedRoot);
        http = CreateHttpClient(source.Endpoint, handler);
    }

    private DesktopLanguagePackCatalogClient(
        DesktopLanguagePackSource source,
        HttpMessageHandler handler)
    {
        ValidateSource(source, requireCertificate: false);
        this.source = source;
        http = CreateHttpClient(source.Endpoint, handler);
    }

    public async Task<DesktopLanguagePackDownload> DownloadAsync(
        string locale,
        CancellationToken cancellationToken = default)
    {
        var canonical = DownloadableLocale(locale).Locale;
        var catalogPayload = await GetJsonAsync(
            "language-packs/catalog.json",
            MaxCatalogBytes,
            cancellationToken).ConfigureAwait(false);
        var catalog = DecodeCatalog(catalogPayload);
        var entry = catalog.Packs.Single(pack => pack.Locale == canonical);
        var payload = await GetJsonAsync(
            entry.Url.TrimStart('/'),
            DesktopLanguagePackStore.MaxPackBytes,
            cancellationToken).ConfigureAwait(false);
        VerifyDownloadedDigest(payload, entry.Sha256);
        return new DesktopLanguagePackDownload(
            source.SourceId,
            entry.Locale,
            entry.Version,
            entry.Sha256,
            payload);
    }

    public void Dispose()
    {
        http.Dispose();
        trustedRoot?.Dispose();
    }

    public static async Task VerifyContractAsync()
    {
        var packPayload = DesktopLanguagePackStore.VerificationPayload("pt-BR");
        var digest = Convert.ToHexString(SHA256.HashData(packPayload)).ToLowerInvariant();
        var catalog = VerificationCatalog(digest);
        var catalogPayload = JsonSerializer.SerializeToUtf8Bytes(
            catalog,
            DesktopLanguagePackCatalogJsonContext.Default.DesktopLanguagePackCatalog);
        var source = new DesktopLanguagePackSource(
            "daemon-verification",
            "Verification authority",
            new Uri("https://catalog.example:9443/"),
            "/verification/ca.pem");
        var handler = new VerificationHandler(catalogPayload, packPayload);
        using var client = new DesktopLanguagePackCatalogClient(source, handler);
        var downloaded = await client.DownloadAsync("pt-BR").ConfigureAwait(false);
        if (downloaded.SourceId != source.SourceId
            || downloaded.Locale != "pt-BR"
            || downloaded.Version != "1.0.0"
            || downloaded.Sha256 != digest
            || !downloaded.Payload.AsSpan().SequenceEqual(packPayload)
            || handler.RequestCount != 2
            || !handler.CredentialFree)
        {
            throw new InvalidDataException(
                "desktop language-pack catalog download contract drifted");
        }

        ExpectInvalidData(
            () => DecodeCatalog(JsonSerializer.SerializeToUtf8Bytes(
                catalog with { DownloadableLocaleCount = 21 },
                DesktopLanguagePackCatalogJsonContext.Default.DesktopLanguagePackCatalog)),
            "desktop language-pack catalog accepted a false locale count");
        ExpectInvalidData(
            () => DecodeCatalog(JsonSerializer.SerializeToUtf8Bytes(
                catalog with { Packs = catalog.Packs.Skip(1).ToArray() },
                DesktopLanguagePackCatalogJsonContext.Default.DesktopLanguagePackCatalog)),
            "desktop language-pack catalog accepted a missing locale");
        var unsafePacks = catalog.Packs.ToArray();
        unsafePacks[0] = unsafePacks[0] with { Url = "https://example.invalid/pt-BR.json" };
        ExpectInvalidData(
            () => DecodeCatalog(JsonSerializer.SerializeToUtf8Bytes(
                catalog with { Packs = unsafePacks },
                DesktopLanguagePackCatalogJsonContext.Default.DesktopLanguagePackCatalog)),
            "desktop language-pack catalog accepted a cross-origin pack URL");
        ExpectInvalidData(
            () => DecodeCatalog(
                Encoding.UTF8.GetBytes(
                    "{\"schema\":\"leserpent.language-pack-catalog/v1\","
                    + "\"schema\":\"leserpent.language-pack-catalog/v1\"}")),
            "desktop language-pack catalog accepted a duplicate property");
        ExpectInvalidData(
            () => VerifyDownloadedDigest(packPayload, new string('0', 64)),
            "desktop language-pack download accepted a mismatched digest");
    }

    public static void VerifyPublishedArtifacts(
        string catalogPath,
        string languagePackRoot)
    {
        var fullCatalogPath = Path.GetFullPath(catalogPath);
        var fullPackRoot = Path.GetFullPath(languagePackRoot);
        if (!Directory.Exists(fullPackRoot)
            || (File.GetAttributes(fullPackRoot) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                "published language-pack root must be a regular directory");
        }
        var catalog = DecodeCatalog(ReadBoundedRegularFile(
            fullCatalogPath,
            MaxCatalogBytes));
        var verificationRoot = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-published-language-packs-{Guid.NewGuid():N}");
        try
        {
            var store = new DesktopLanguagePackStore(verificationRoot);
            foreach (var entry in catalog.Packs)
            {
                var payload = ReadBoundedRegularFile(
                    Path.Combine(fullPackRoot, $"{entry.Locale}.json"),
                    DesktopLanguagePackStore.MaxPackBytes);
                VerifyDownloadedDigest(payload, entry.Sha256);
                var installed = store.Install(
                    payload,
                    entry.Sha256,
                    entry.Locale,
                    entry.Version);
                if (installed.Manifest.Locale != entry.Locale
                    || installed.Manifest.Version != entry.Version)
                {
                    throw new InvalidDataException(
                        "published language pack drifted from its catalog entry");
                }
            }
            var snapshot = store.LoadAll();
            if (snapshot.Packs.Count != DownloadableLocaleCount
                || snapshot.RejectedFiles.Count != 0)
            {
                throw new InvalidDataException(
                    "published language-pack set did not round-trip through desktop storage");
            }
        }
        finally
        {
            if (Directory.Exists(verificationRoot))
            {
                Directory.Delete(verificationRoot, true);
            }
        }
    }

    private async Task<byte[]> GetJsonAsync(
        string path,
        int maxBytes,
        CancellationToken cancellationToken)
    {
        ValidateContentPath(path);
        using var request = new HttpRequestMessage(HttpMethod.Get, path);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        request.Headers.CacheControl = new CacheControlHeaderValue { NoCache = true };
        using var response = await http.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken).ConfigureAwait(false);
        if (!response.IsSuccessStatusCode)
        {
            throw new InvalidDataException(
                $"language-pack source returned HTTP {(int)response.StatusCode}");
        }
        if (!string.Equals(
                response.Content.Headers.ContentType?.MediaType,
                "application/json",
                StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException(
                "language-pack source returned a non-JSON response");
        }
        if (response.Content.Headers.ContentLength is long length
            && (length <= 0 || length > maxBytes))
        {
            throw new InvalidDataException(
                "language-pack source response exceeds its byte budget");
        }
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken)
            .ConfigureAwait(false);
        using var output = new MemoryStream();
        var buffer = new byte[8192];
        while (true)
        {
            var read = await stream.ReadAsync(buffer, cancellationToken).ConfigureAwait(false);
            if (read == 0)
            {
                break;
            }
            if (output.Length + read > maxBytes)
            {
                throw new InvalidDataException(
                    "language-pack source response exceeds its byte budget");
            }
            output.Write(buffer, 0, read);
        }
        if (output.Length == 0)
        {
            throw new InvalidDataException("language-pack source returned an empty response");
        }
        cancellationToken.ThrowIfCancellationRequested();
        return output.ToArray();
    }

    private static DesktopLanguagePackCatalog DecodeCatalog(ReadOnlySpan<byte> payload)
    {
        if (payload.Length is <= 0 or > MaxCatalogBytes)
        {
            throw new InvalidDataException("desktop language-pack catalog has an invalid size");
        }
        try
        {
            using var document = JsonDocument.Parse(payload.ToArray(), new JsonDocumentOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = 8,
            });
            VerifyNoDuplicateProperties(document.RootElement);
            var catalog = JsonSerializer.Deserialize(
                payload,
                DesktopLanguagePackCatalogJsonContext.Default.DesktopLanguagePackCatalog)
                ?? throw new InvalidDataException(
                    "desktop language-pack catalog is empty");
            ValidateCatalog(catalog);
            return catalog;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException(
                "desktop language-pack catalog JSON is invalid",
                error);
        }
    }

    private static void ValidateCatalog(DesktopLanguagePackCatalog catalog)
    {
        if (catalog.Schema != Schema
            || catalog.OfficialLocaleCount != DesktopLocalization.OfficialLocales.Count
            || catalog.BuiltinLocaleCount != DesktopLocalization.OfficialLocales.Count(
                locale => locale.BuiltIn)
            || catalog.DownloadableLocaleCount != DownloadableLocaleCount
            || catalog.Packs is null
            || catalog.Packs.Count != DownloadableLocaleCount
            || !DateTimeOffset.TryParseExact(
                catalog.GeneratedAt,
                "yyyy-MM-dd'T'HH:mm:ss'Z'",
                CultureInfo.InvariantCulture,
                DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal,
                out _))
        {
            throw new InvalidDataException(
                "desktop language-pack catalog metadata is invalid");
        }
        var locales = new HashSet<string>(StringComparer.Ordinal);
        foreach (var entry in catalog.Packs)
        {
            if (entry is null
                || string.IsNullOrEmpty(entry.Locale)
                || string.IsNullOrEmpty(entry.Url))
            {
                throw new InvalidDataException(
                    "desktop language-pack catalog contains a null entry");
            }
            var definition = DownloadableLocale(entry.Locale);
            var expectedDirection = definition.IsRightToLeft ? "rtl" : "ltr";
            if (!locales.Add(entry.Locale)
                || entry.Locale != definition.Locale
                || entry.Name != definition.Name
                || entry.NativeName != definition.NativeName
                || entry.Direction != expectedDirection
                || entry.Coverage != "core-ui"
                || entry.Url != $"/language-packs/{definition.Locale}.json"
                || !ValidVersion(entry.Version)
                || !ValidSha256(entry.Sha256))
            {
                throw new InvalidDataException(
                    "desktop language-pack catalog contains an invalid entry");
            }
        }
        var expected = DesktopLocalization.OfficialLocales
            .Where(locale => !locale.BuiltIn)
            .Select(locale => locale.Locale);
        if (!locales.SetEquals(expected))
        {
            throw new InvalidDataException(
                "desktop language-pack catalog does not cover the official download set");
        }
    }

    private static void ValidateSource(
        DesktopLanguagePackSource source,
        bool requireCertificate)
    {
        var endpoint = RemoteClientOptions.ParseEndpoint(source.Endpoint.ToString());
        if (endpoint != source.Endpoint
            || source.SourceId.Length is <= 0 or > 128
            || source.SourceId.Any(char.IsControl)
            || source.DisplayName.Length is <= 0 or > 160
            || source.DisplayName.Any(char.IsControl))
        {
            throw new InvalidDataException("desktop language-pack source is invalid");
        }
        if (!requireCertificate)
        {
            return;
        }
        var authority = new FileInfo(source.CertificateAuthorityPath);
        if (!Path.IsPathFullyQualified(source.CertificateAuthorityPath)
            || !authority.Exists
            || authority.Length is <= 0 or > 1024 * 1024
            || (authority.Attributes
                & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException(
                "desktop language-pack source CA must be a bounded regular file");
        }
    }

    private static void ValidateContentPath(string path)
    {
        if (path.Length is <= 0 or > 160
            || path.Any(char.IsControl)
            || path.Contains('\\')
            || path.Contains("..", StringComparison.Ordinal)
            || path.Contains('?')
            || path.Contains('#')
            || Uri.TryCreate(path, UriKind.Absolute, out _)
            || !path.StartsWith("language-packs/", StringComparison.Ordinal)
            || !path.EndsWith(".json", StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "desktop language-pack content path is invalid");
        }
    }

    private static byte[] ReadBoundedRegularFile(string path, int maxBytes)
    {
        var info = new FileInfo(path);
        if (!info.Exists
            || info.Length is <= 0
            || info.Length > maxBytes
            || (info.Attributes
                & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException(
                "published language-pack artifact must be a bounded regular file");
        }
        using var stream = new FileStream(
            path,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read);
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        return payload;
    }

    private static DesktopLocaleDefinition DownloadableLocale(string locale)
    {
        if (!DesktopLocalization.TryGetOfficialLocale(locale, out var definition)
            || definition.BuiltIn)
        {
            throw new InvalidDataException(
                "desktop language-pack download must target an official downloadable locale");
        }
        return definition;
    }

    private static bool ValidVersion(string? version)
    {
        if (version is null)
        {
            return false;
        }
        var core = version.Split('-', 2, StringSplitOptions.None)[0];
        var components = core.Split('.', StringSplitOptions.None);
        return version.Length is > 0 and <= 40
            && version.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '-' or '+')
            && components.Length == 3
            && components.All(component => uint.TryParse(
                component,
                NumberStyles.None,
                CultureInfo.InvariantCulture,
                out _));
    }

    private static bool ValidSha256(string? digest)
    {
        if (digest is null
            || digest.Length != 64
            || digest.Any(character =>
                !(character is >= '0' and <= '9')
                && !(character is >= 'a' and <= 'f')))
        {
            return false;
        }
        return Convert.FromHexString(digest).Length == 32;
    }

    private static void VerifyDownloadedDigest(
        ReadOnlySpan<byte> payload,
        string expectedSha256)
    {
        if (!ValidSha256(expectedSha256))
        {
            throw new InvalidDataException(
                "desktop language-pack catalog digest is invalid");
        }
        var expected = Convert.FromHexString(expectedSha256);
        var actual = SHA256.HashData(payload);
        if (!CryptographicOperations.FixedTimeEquals(expected, actual))
        {
            throw new InvalidDataException(
                "desktop language-pack download failed SHA-256 verification");
        }
    }

    private static void VerifyNoDuplicateProperties(JsonElement value)
    {
        if (value.ValueKind == JsonValueKind.Object)
        {
            var names = new HashSet<string>(StringComparer.Ordinal);
            foreach (var property in value.EnumerateObject())
            {
                if (!names.Add(property.Name))
                {
                    throw new InvalidDataException(
                        "desktop language-pack catalog contains a duplicate property");
                }
                VerifyNoDuplicateProperties(property.Value);
            }
            return;
        }
        if (value.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in value.EnumerateArray())
            {
                VerifyNoDuplicateProperties(item);
            }
        }
    }

    private static HttpClient CreateHttpClient(Uri endpoint, HttpMessageHandler handler)
    {
        var client = new HttpClient(handler, disposeHandler: true)
        {
            BaseAddress = endpoint,
            Timeout = RequestTimeout,
        };
        client.DefaultRequestHeaders.UserAgent.ParseAdd("Leserpent-Desktop/1.0");
        return client;
    }

    private static DesktopLanguagePackCatalog VerificationCatalog(string digest) => new()
    {
        Schema = Schema,
        GeneratedAt = "2026-07-13T00:00:00Z",
        OfficialLocaleCount = DesktopLocalization.OfficialLocales.Count,
        BuiltinLocaleCount = DesktopLocalization.OfficialLocales.Count(locale => locale.BuiltIn),
        DownloadableLocaleCount = DownloadableLocaleCount,
        Packs = DesktopLocalization.OfficialLocales
            .Where(locale => !locale.BuiltIn)
            .Select(locale => new DesktopLanguagePackCatalogEntry
            {
                Locale = locale.Locale,
                Name = locale.Name,
                NativeName = locale.NativeName,
                Version = "1.0.0",
                Direction = locale.IsRightToLeft ? "rtl" : "ltr",
                Coverage = "core-ui",
                Url = $"/language-packs/{locale.Locale}.json",
                Sha256 = locale.Locale == "pt-BR" ? digest : new string('0', 64),
            })
            .ToArray(),
    };

    private static void ExpectInvalidData(Action action, string message)
    {
        try
        {
            action();
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException(message);
    }

    private sealed class VerificationHandler(byte[] catalog, byte[] pack)
        : HttpMessageHandler
    {
        public int RequestCount { get; private set; }
        public bool CredentialFree { get; private set; } = true;

        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            RequestCount++;
            CredentialFree &= request.Headers.Authorization is null
                && !request.Headers.Contains("X-Leserpent-Admin-Token")
                && request.RequestUri?.GetLeftPart(UriPartial.Authority)
                    == "https://catalog.example:9443";
            var payload = request.RequestUri?.AbsolutePath switch
            {
                "/language-packs/catalog.json" => catalog,
                "/language-packs/pt-BR.json" => pack,
                _ => throw new InvalidDataException(
                    "desktop language-pack client requested an unexpected path"),
            };
            var response = new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new ByteArrayContent(payload),
                RequestMessage = request,
            };
            response.Content.Headers.ContentType =
                new MediaTypeHeaderValue("application/json");
            return Task.FromResult(response);
        }
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DesktopLanguagePackCatalog
{
    public required string Schema { get; init; }
    public required string GeneratedAt { get; init; }
    public int OfficialLocaleCount { get; init; }
    public int BuiltinLocaleCount { get; init; }
    public int DownloadableLocaleCount { get; init; }
    public required IReadOnlyList<DesktopLanguagePackCatalogEntry> Packs { get; init; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DesktopLanguagePackCatalogEntry
{
    public required string Locale { get; init; }
    public required string Name { get; init; }
    public required string NativeName { get; init; }
    public required string Version { get; init; }
    public required string Direction { get; init; }
    public required string Coverage { get; init; }
    public required string Url { get; init; }
    public required string Sha256 { get; init; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(DesktopLanguagePackCatalog))]
internal partial class DesktopLanguagePackCatalogJsonContext : JsonSerializerContext;
