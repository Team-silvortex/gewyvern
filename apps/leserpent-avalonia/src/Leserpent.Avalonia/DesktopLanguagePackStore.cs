using System.Buffers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

internal sealed record DesktopInstalledLanguagePack(
    DesktopLanguagePack Manifest,
    IReadOnlyDictionary<string, string> Translations,
    string Sha256);

internal sealed record DesktopLanguagePackSnapshot(
    IReadOnlyDictionary<string, DesktopInstalledLanguagePack> Packs,
    IReadOnlyList<string> RejectedFiles);

internal sealed class DesktopLanguagePackStore(string root)
{
    public const string Schema = "leserpent.language-pack/v1";
    public const int CoreUiKeyCount = 18;
    public const int OfficialPackKeyCount = 30;
    public const string OfficialPackVersion = "1.1.0";
    public const int MaxPackBytes = 256 * 1024;
    private const int MaxDirectoryEntries = 64;
    private const int MaxDepth = 12;
    private const int MaxNodes = 2000;
    private const int MaxStringLength = 4000;
    private static readonly HashSet<string> RequiredCoreUiKeys =
    [
        "hero.title",
        "hero.subcopy",
        "language.label",
        "languagePacks.title",
        "languagePacks.install",
        "languagePacks.installedLabel",
        "languagePacks.download",
        "languagePacks.remove",
        "languagePacks.coverageCore",
        "theme.label",
        "tabs.overview",
        "tabs.runtimes",
        "tabs.register",
        "tabs.persistence",
        "tabs.sessions",
        "runtimes.workspaceTabs.panel",
        "runtimePanel.windows.openAll",
        "runtimePanel.windows.closeAll",
    ];
    private static readonly HashSet<string> RequiredOfficialPackKeys =
    [
        .. RequiredCoreUiKeys,
        "language.auto",
        "languagePacks.subcopy",
        "languagePacks.refresh",
        "languagePacks.import",
        "languagePacks.installedTitle",
        "languagePacks.catalogTitle",
        "languagePacks.catalogEmpty",
        "languagePacks.noneInstalled",
        "languagePacks.export",
        "theme.auto",
        "theme.light",
        "theme.dark",
    ];

    public DesktopLanguagePackSnapshot LoadAll()
    {
        if (!Directory.Exists(root))
        {
            return EmptySnapshot();
        }
        EnsurePrivateDirectory(root);
        var files = Directory.EnumerateFiles(root, "*", SearchOption.TopDirectoryOnly)
            .Take(MaxDirectoryEntries + 1)
            .ToArray();
        if (files.Length > MaxDirectoryEntries)
        {
            return new DesktopLanguagePackSnapshot(
                new Dictionary<string, DesktopInstalledLanguagePack>(
                    StringComparer.OrdinalIgnoreCase),
                ["language-pack directory exceeds its entry budget"]);
        }
        Array.Sort(files, StringComparer.Ordinal);

        var packs = new Dictionary<string, DesktopInstalledLanguagePack>(
            StringComparer.OrdinalIgnoreCase);
        var rejected = new List<string>();
        foreach (var file in files)
        {
            var name = Path.GetFileName(file);
            if (!name.EndsWith(".json", StringComparison.Ordinal))
            {
                rejected.Add($"{BoundedName(name)}: unsupported entry");
                continue;
            }
            try
            {
                EnsureRegularFile(file);
                var installed = Decode(ReadBounded(file), expectedSha256: null);
                var expectedName = $"{installed.Manifest.Locale}.json";
                if (name != expectedName || !packs.TryAdd(installed.Manifest.Locale, installed))
                {
                    throw new InvalidDataException(
                        "desktop language-pack filename or locale is duplicated");
                }
            }
            catch (Exception error) when (StartupFailure.IsExpected(error))
            {
                rejected.Add($"{BoundedName(name)}: rejected");
            }
        }
        return new DesktopLanguagePackSnapshot(packs, rejected);
    }

    public DesktopInstalledLanguagePack Install(
        ReadOnlySpan<byte> payload,
        CancellationToken cancellationToken = default) => InstallCore(
            payload,
            expectedSha256: null,
            expectedLocale: null,
            expectedVersion: null,
            requireOfficialArtifact: false,
            cancellationToken);

    public DesktopInstalledLanguagePack InstallCatalogArtifact(
        ReadOnlySpan<byte> payload,
        string expectedSha256,
        string expectedLocale,
        string expectedVersion,
        CancellationToken cancellationToken = default)
    {
        if (expectedSha256 is null
            || expectedLocale is null
            || expectedVersion is null)
        {
            throw new InvalidDataException(
                "desktop catalog language pack requires complete catalog bindings");
        }
        return InstallCore(
            payload,
            expectedSha256,
            expectedLocale,
            expectedVersion,
            requireOfficialArtifact: true,
            cancellationToken);
    }

    private DesktopInstalledLanguagePack InstallCore(
        ReadOnlySpan<byte> payload,
        string? expectedSha256,
        string? expectedLocale,
        string? expectedVersion,
        bool requireOfficialArtifact,
        CancellationToken cancellationToken)
    {
        if (payload.Length is <= 0 or > MaxPackBytes)
        {
            throw new InvalidDataException("desktop language pack has an invalid size");
        }
        var ownedPayload = payload.ToArray();
        var installed = Decode(ownedPayload, expectedSha256);
        VerifyCatalogBinding(installed, expectedLocale, expectedVersion);
        if (requireOfficialArtifact)
        {
            VerifyOfficialArtifact(installed);
        }
        cancellationToken.ThrowIfCancellationRequested();
        EnsurePrivateDirectory(root, create: true);
        var target = Path.Combine(root, $"{installed.Manifest.Locale}.json");
        if (File.Exists(target))
        {
            EnsureRegularFile(target);
        }
        var temporary = Path.Combine(
            root,
            $".{installed.Manifest.Locale}.{Guid.NewGuid():N}.tmp");
        try
        {
            using (var stream = OpenPrivateTemporary(temporary))
            {
                stream.Write(ownedPayload);
                stream.Flush(true);
            }
            cancellationToken.ThrowIfCancellationRequested();
            File.Move(temporary, target, true);
        }
        finally
        {
            File.Delete(temporary);
        }
        return installed;
    }

    public DesktopInstalledLanguagePack Install(
        Stream stream) => Install(ReadBounded(stream));

    public async Task<DesktopInstalledLanguagePack> InstallAsync(
        Stream stream,
        CancellationToken cancellationToken = default)
    {
        var payload = await ReadBoundedAsync(stream, cancellationToken)
            .ConfigureAwait(false);
        cancellationToken.ThrowIfCancellationRequested();
        return Install(payload, cancellationToken);
    }

    public void Remove(string locale)
    {
        var definition = DownloadableLocale(locale);
        var target = Path.Combine(root, $"{definition.Locale}.json");
        if (!File.Exists(target))
        {
            return;
        }
        EnsurePrivateDirectory(root);
        EnsureRegularFile(target);
        File.Delete(target);
    }

    public static string DefaultPath()
    {
        var localData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(localData))
        {
            throw new InvalidOperationException("local application data directory is unavailable");
        }
        return Path.Combine(localData, "leserpent", "language-packs-v1");
    }

    public static void VerifyContract()
    {
        if (RequiredCoreUiKeys.Count != CoreUiKeyCount)
        {
            throw new InvalidDataException("desktop language-pack core key contract drifted");
        }
        if (RequiredOfficialPackKeys.Count != OfficialPackKeyCount)
        {
            throw new InvalidDataException("desktop official language-pack key contract drifted");
        }
        var verificationRoot = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-language-packs-{Guid.NewGuid():N}");
        try
        {
            var store = new DesktopLanguagePackStore(verificationRoot);
            var payload = VerificationPayload("pt-BR");
            var digest = Sha256(payload);
            var installed = store.Install(payload);
            var snapshot = store.LoadAll();
            if (installed.Manifest.Locale != "pt-BR"
                || installed.Translations.Count != CoreUiKeyCount
                || snapshot.Packs.Count != 1
                || snapshot.RejectedFiles.Count != 0
                || snapshot.Packs["pt-BR"].Sha256 != digest)
            {
                throw new InvalidDataException("desktop language pack did not round-trip");
            }
            using (var asyncPayload = new MemoryStream(payload))
            {
                var asyncInstalled = store.InstallAsync(asyncPayload)
                    .GetAwaiter()
                    .GetResult();
                if (asyncInstalled.Sha256 != digest)
                {
                    throw new InvalidDataException(
                        "desktop language-pack async reader changed the payload");
                }
            }
            using (var cancelled = new CancellationTokenSource())
            {
                cancelled.Cancel();
                ExpectCancellation(
                    () => store.InstallAsync(
                            new MemoryStream(VerificationPayload("it")),
                            cancellationToken: cancelled.Token)
                        .GetAwaiter()
                        .GetResult(),
                    "desktop language-pack async install ignored cancellation");
                if (File.Exists(Path.Combine(verificationRoot, "it.json")))
                {
                    throw new InvalidDataException(
                        "cancelled desktop language-pack install committed a file");
                }
            }
            if (!OperatingSystem.IsWindows()
                && (File.GetUnixFileMode(verificationRoot)
                        != (UnixFileMode.UserRead
                            | UnixFileMode.UserWrite
                            | UnixFileMode.UserExecute)
                    || File.GetUnixFileMode(Path.Combine(verificationRoot, "pt-BR.json"))
                        != (UnixFileMode.UserRead | UnixFileMode.UserWrite)))
            {
                throw new InvalidDataException("desktop language-pack storage is not private");
            }

            var overflowEntries = Enumerable.Range(0, MaxDirectoryEntries)
                .Select(index => Path.Combine(verificationRoot, $"overflow-{index:D2}.tmp"))
                .ToArray();
            foreach (var entry in overflowEntries)
            {
                File.WriteAllText(entry, "bounded fixture");
            }
            snapshot = store.LoadAll();
            if (snapshot.Packs.Count != 0
                || snapshot.RejectedFiles is not ["language-pack directory exceeds its entry budget"])
            {
                throw new InvalidDataException(
                    "desktop language-pack directory budget was not fail closed");
            }
            foreach (var entry in overflowEntries)
            {
                File.Delete(entry);
            }

            var rejectedOfficialPayload = VerificationPayload(
                "it",
                version: OfficialPackVersion);
            var rejectedOfficialDigest = Sha256(rejectedOfficialPayload);
            var rejectedOfficialRoot = Path.Combine(
                verificationRoot,
                "rejected-official");
            ExpectInvalidData(
                () => new DesktopLanguagePackStore(rejectedOfficialRoot)
                    .InstallCatalogArtifact(
                        rejectedOfficialPayload,
                        rejectedOfficialDigest,
                        "it",
                        OfficialPackVersion),
                "incomplete official language pack committed its first install");
            if (Directory.Exists(rejectedOfficialRoot))
            {
                throw new InvalidDataException(
                    "failed official language-pack install created persistent state");
            }

            var installedPath = Path.Combine(verificationRoot, "pt-BR.json");
            var previousPayload = File.ReadAllBytes(installedPath);
            var rejectedUpgradePayload = VerificationPayload(
                "pt-BR",
                version: OfficialPackVersion);
            ExpectInvalidData(
                () => store.InstallCatalogArtifact(
                    rejectedUpgradePayload,
                    Sha256(rejectedUpgradePayload),
                    "pt-BR",
                    OfficialPackVersion),
                "incomplete official language pack replaced an installed pack");
            if (!previousPayload.AsSpan().SequenceEqual(File.ReadAllBytes(installedPath))
                || Directory.EnumerateFiles(verificationRoot).Any(path =>
                    path.EndsWith(".tmp", StringComparison.Ordinal)))
            {
                throw new InvalidDataException(
                    "failed official language-pack update changed persistent state");
            }

            var officialPayload = OfficialVerificationPayload("it");
            var officialInstalled = store.InstallCatalogArtifact(
                officialPayload,
                Sha256(officialPayload),
                "it",
                OfficialPackVersion);
            snapshot = store.LoadAll();
            if (officialInstalled.Manifest.Version != OfficialPackVersion
                || officialInstalled.Translations.Count != OfficialPackKeyCount
                || snapshot.Packs.Count != 2
                || !snapshot.Packs.ContainsKey("it")
                || snapshot.RejectedFiles.Count != 0)
            {
                throw new InvalidDataException(
                    "official language pack did not pass its pre-commit contract");
            }
            store.Remove("it");

            ExpectInvalidData(
                () => store.InstallCatalogArtifact(
                    officialPayload,
                    new string('0', 64),
                    "it",
                    OfficialPackVersion),
                "desktop language pack accepted a mismatched digest");
            ExpectInvalidData(
                () => store.InstallCatalogArtifact(
                    officialPayload,
                    Sha256(officialPayload),
                    "pt-BR",
                    OfficialPackVersion),
                "desktop language pack accepted a mismatched catalog locale");
            ExpectInvalidData(
                () => store.InstallCatalogArtifact(
                    officialPayload,
                    Sha256(officialPayload),
                    "it",
                    "2.0.0"),
                "desktop language pack accepted a mismatched catalog version");
            ExpectInvalidData(
                () => store.Install(VerificationPayload("en")),
                "desktop language pack replaced a built-in locale");
            ExpectInvalidData(
                () => store.Install(VerificationPayload("pt-BR", omitLastCoreKey: true)),
                "desktop language pack accepted incomplete core-ui coverage");
            ExpectInvalidData(
                () => store.Install(VerificationPayload("pt-BR", includeUnknownField: true)),
                "desktop language pack accepted an unknown manifest field");
            ExpectInvalidData(
                () => store.Install(Encoding.UTF8.GetBytes(
                    "{\"schema\":\"leserpent.language-pack/v1\",\"locale\":null,"
                    + "\"name\":\"Portuguese (Brazil)\",\"nativeName\":\"Português (Brasil)\","
                    + "\"version\":\"1.0.0\",\"direction\":\"ltr\",\"coverage\":\"core-ui\","
                    + "\"translations\":{}}")),
                "desktop language pack accepted null metadata");

            File.WriteAllText(Path.Combine(verificationRoot, "it.json"), "{broken");
            snapshot = store.LoadAll();
            if (snapshot.Packs.Count != 1
                || snapshot.RejectedFiles.Count != 1
                || !snapshot.Packs.ContainsKey("pt-BR"))
            {
                throw new InvalidDataException(
                    "a malformed desktop language pack blocked a valid sibling");
            }
            var localization = DesktopLocalization.ForLanguagePackVerification(
                verificationRoot,
                "pt-BR");
            if (localization.Text(DesktopTextKey.ControlTopology) != "verified hero.title"
                || localization.Text(DesktopTextKey.Language) != "verified language.label..."
                || localization.Text(DesktopTextKey.InstallLanguagePack)
                    != "verified languagePacks.install JSON..."
                || localization.Text(DesktopTextKey.Close) != "Close")
            {
                throw new InvalidDataException(
                    "desktop language pack did not project with per-key English fallback");
            }
            store.Remove("pt-BR");
            if (File.Exists(Path.Combine(verificationRoot, "pt-BR.json")))
            {
                throw new InvalidDataException("desktop language pack was not removed");
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

    internal static byte[] VerificationPayload(
        string locale,
        bool omitLastCoreKey = false,
        bool includeUnknownField = false,
        string version = "1.0.0",
        bool includeOfficialKeys = false)
    {
        var definition = DesktopLocalization.OfficialLocales.FirstOrDefault(candidate =>
            candidate.Locale.Equals(locale, StringComparison.OrdinalIgnoreCase));
        var name = definition?.Name ?? "English";
        var nativeName = definition?.NativeName ?? "English";
        var direction = definition?.IsRightToLeft == true ? "rtl" : "ltr";
        var requiredKeys = includeOfficialKeys
            ? RequiredOfficialPackKeys
            : RequiredCoreUiKeys;
        var values = requiredKeys
            .Order(StringComparer.Ordinal)
            .Where((_, index) => !omitLastCoreKey || index < requiredKeys.Count - 1)
            .ToDictionary(key => key, key => $"verified {key}", StringComparer.Ordinal);
        var tree = new Dictionary<string, object>(StringComparer.Ordinal);
        foreach (var entry in values)
        {
            AddDotted(tree, entry.Key.Split('.'), entry.Value);
        }
        var buffer = new ArrayBufferWriter<byte>();
        using var writer = new Utf8JsonWriter(buffer);
        writer.WriteStartObject();
        writer.WriteString("schema", Schema);
        writer.WriteString("locale", locale);
        writer.WriteString("name", name);
        writer.WriteString("nativeName", nativeName);
        writer.WriteString("version", version);
        writer.WriteString("author", "Leserpent verification");
        writer.WriteString("direction", direction);
        writer.WriteString("coverage", "core-ui");
        writer.WritePropertyName("translations");
        WriteObject(writer, tree);
        if (includeUnknownField)
        {
            writer.WriteBoolean("unknown", true);
        }
        writer.WriteEndObject();
        writer.Flush();
        return buffer.WrittenSpan.ToArray();
    }

    internal static byte[] OfficialVerificationPayload(string locale) => VerificationPayload(
        locale,
        version: OfficialPackVersion,
        includeOfficialKeys: true);

    private static void VerifyOfficialArtifact(DesktopInstalledLanguagePack installed)
    {
        if (installed.Manifest.Version != OfficialPackVersion
            || installed.Translations.Count != OfficialPackKeyCount
            || !RequiredOfficialPackKeys.SetEquals(installed.Translations.Keys))
        {
            throw new InvalidDataException(
                "desktop official language pack does not match its published key contract");
        }
    }

    private static DesktopInstalledLanguagePack Decode(
        ReadOnlySpan<byte> payload,
        string? expectedSha256)
    {
        if (payload.Length is <= 0 or > MaxPackBytes)
        {
            throw new InvalidDataException("desktop language pack has an invalid size");
        }
        var digest = Sha256(payload);
        if (expectedSha256 is not null)
        {
            VerifyDigest(expectedSha256, digest);
        }
        try
        {
            using var document = JsonDocument.Parse(payload.ToArray(), new JsonDocumentOptions
            {
                AllowTrailingCommas = false,
                CommentHandling = JsonCommentHandling.Disallow,
                MaxDepth = MaxDepth + 4,
            });
            VerifyNoDuplicateProperties(document.RootElement);
            var manifest = JsonSerializer.Deserialize(
                payload,
                DesktopLanguagePackJsonContext.Default.DesktopLanguagePack)
                ?? throw new InvalidDataException("desktop language pack is empty");
            var translations = Validate(manifest);
            return new DesktopInstalledLanguagePack(manifest, translations, digest);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("desktop language-pack JSON is invalid", error);
        }
    }

    private static IReadOnlyDictionary<string, string> Validate(DesktopLanguagePack manifest)
    {
        if (manifest.Schema != Schema
            || manifest.Coverage != "core-ui"
            || manifest.Direction is not ("ltr" or "rtl")
            || !ValidText(manifest.Locale, 35)
            || !ValidText(manifest.Name, 120)
            || !ValidText(manifest.NativeName, 120)
            || !ValidText(manifest.Version, 40)
            || (manifest.Author is not null && !ValidText(manifest.Author, 120)))
        {
            throw new InvalidDataException("desktop language-pack metadata is invalid");
        }
        var definition = DownloadableLocale(manifest.Locale);
        var expectedDirection = definition.IsRightToLeft ? "rtl" : "ltr";
        if (manifest.Locale != definition.Locale
            || manifest.Name != definition.Name
            || manifest.NativeName != definition.NativeName
            || manifest.Direction != expectedDirection)
        {
            throw new InvalidDataException(
                "desktop language-pack metadata does not match the official locale");
        }
        var translations = new Dictionary<string, string>(StringComparer.Ordinal);
        var budget = 0;
        Flatten(manifest.Translations, string.Empty, 0, translations, ref budget);
        if (!RequiredCoreUiKeys.IsSubsetOf(translations.Keys))
        {
            throw new InvalidDataException(
                "desktop language pack does not cover the core-ui contract");
        }
        return translations;
    }

    private static void VerifyCatalogBinding(
        DesktopInstalledLanguagePack installed,
        string? expectedLocale,
        string? expectedVersion)
    {
        if ((expectedLocale is not null
                && installed.Manifest.Locale != expectedLocale)
            || (expectedVersion is not null
                && installed.Manifest.Version != expectedVersion))
        {
            throw new InvalidDataException(
                "desktop language pack does not match its catalog entry");
        }
    }

    private static void Flatten(
        JsonElement value,
        string prefix,
        int depth,
        Dictionary<string, string> output,
        ref int budget)
    {
        if (value.ValueKind != JsonValueKind.Object || depth > MaxDepth)
        {
            throw new InvalidDataException(
                "desktop language-pack translations must be a bounded object tree");
        }
        foreach (var property in value.EnumerateObject())
        {
            budget++;
            if (budget > MaxNodes || !ValidTranslationKey(property.Name))
            {
                throw new InvalidDataException(
                    "desktop language-pack translations exceed their key budget");
            }
            var path = prefix.Length == 0 ? property.Name : $"{prefix}.{property.Name}";
            if (property.Value.ValueKind == JsonValueKind.String)
            {
                var text = property.Value.GetString()
                    ?? throw new InvalidDataException("desktop language-pack text is null");
                if (text.Length > MaxStringLength || text.Any(InvalidTranslationCharacter)
                    || !output.TryAdd(path, text))
                {
                    throw new InvalidDataException("desktop language-pack text is invalid");
                }
            }
            else
            {
                Flatten(property.Value, path, depth + 1, output, ref budget);
            }
        }
    }

    private static void VerifyNoDuplicateProperties(JsonElement value)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new InvalidDataException("desktop language pack must be a JSON object");
        }
        var names = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!names.Add(property.Name))
            {
                throw new InvalidDataException(
                    "desktop language pack contains a duplicate JSON property");
            }
            if (property.Value.ValueKind == JsonValueKind.Object)
            {
                VerifyNoDuplicateProperties(property.Value);
            }
            else if (property.Value.ValueKind == JsonValueKind.Array)
            {
                throw new InvalidDataException("desktop language pack contains an array");
            }
        }
    }

    private static DesktopLocaleDefinition DownloadableLocale(string locale)
    {
        if (locale.Length is <= 0 or > 35
            || !DesktopLocalization.TryGetOfficialLocale(locale, out var definition)
            || definition.BuiltIn)
        {
            throw new InvalidDataException(
                "desktop language pack must target an official downloadable locale");
        }
        return definition;
    }

    private static byte[] ReadBounded(string path)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        return ReadBounded(stream);
    }

    private static byte[] ReadBounded(Stream stream)
    {
        using var output = new MemoryStream();
        Span<byte> buffer = stackalloc byte[8192];
        while (true)
        {
            var read = stream.Read(buffer);
            if (read == 0)
            {
                break;
            }
            if (output.Length + read > MaxPackBytes)
            {
                throw new InvalidDataException("desktop language pack exceeds 256 KiB");
            }
            output.Write(buffer[..read]);
        }
        if (output.Length == 0)
        {
            throw new InvalidDataException("desktop language pack is empty");
        }
        return output.ToArray();
    }

    private static async Task<byte[]> ReadBoundedAsync(
        Stream stream,
        CancellationToken cancellationToken)
    {
        using var output = new MemoryStream();
        var buffer = new byte[8192];
        while (true)
        {
            var read = await stream.ReadAsync(buffer, cancellationToken).ConfigureAwait(false);
            if (read == 0)
            {
                break;
            }
            if (output.Length + read > MaxPackBytes)
            {
                throw new InvalidDataException("desktop language pack exceeds 256 KiB");
            }
            output.Write(buffer, 0, read);
        }
        if (output.Length == 0)
        {
            throw new InvalidDataException("desktop language pack is empty");
        }
        return output.ToArray();
    }

    private static FileStream OpenPrivateTemporary(string path)
    {
        var options = new FileStreamOptions
        {
            Mode = FileMode.CreateNew,
            Access = FileAccess.Write,
            Share = FileShare.None,
            BufferSize = 4096,
            Options = FileOptions.WriteThrough,
        };
        if (!OperatingSystem.IsWindows())
        {
            options.UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
        }
        return new FileStream(path, options);
    }

    private static void EnsurePrivateDirectory(string path, bool create = false)
    {
        if (create)
        {
            Directory.CreateDirectory(path);
        }
        var attributes = File.GetAttributes(path);
        if ((attributes & FileAttributes.Directory) == 0
            || (attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                "desktop language-pack root must be a regular directory");
        }
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(
                path,
                UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        }
    }

    private static void EnsureRegularFile(string path)
    {
        if ((File.GetAttributes(path)
            & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException(
                "desktop language pack must be a regular file");
        }
    }

    private static bool ValidText(string? value, int maximum) =>
        value is not null
        && value.Length is > 0 && value.Length <= maximum
        && value == value.Trim()
        && !value.Any(char.IsControl);

    private static bool ValidTranslationKey(string value) =>
        value.Length is > 0 and <= 128
        && value is not ("__proto__" or "prototype" or "constructor")
        && value.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '_' or '-');

    private static bool InvalidTranslationCharacter(char character) =>
        char.IsControl(character) && character is not ('\n' or '\r' or '\t');

    private static string Sha256(ReadOnlySpan<byte> payload) =>
        Convert.ToHexString(SHA256.HashData(payload)).ToLowerInvariant();

    private static void VerifyDigest(string expected, string actual)
    {
        if (expected.Length != 64
            || expected.Any(character => !char.IsAsciiHexDigit(character)))
        {
            throw new InvalidDataException("desktop language-pack digest is invalid");
        }
        var expectedBytes = Convert.FromHexString(expected);
        var actualBytes = Convert.FromHexString(actual);
        if (!CryptographicOperations.FixedTimeEquals(expectedBytes, actualBytes))
        {
            throw new InvalidDataException("desktop language-pack SHA-256 verification failed");
        }
    }

    private static DesktopLanguagePackSnapshot EmptySnapshot() => new(
        new Dictionary<string, DesktopInstalledLanguagePack>(StringComparer.OrdinalIgnoreCase),
        []);

    private static string BoundedName(string value) => new(
        value.Where(character => !char.IsControl(character)).Take(96).ToArray());

    private static void AddDotted(
        Dictionary<string, object> target,
        ReadOnlySpan<string> segments,
        string value)
    {
        if (segments.Length == 1)
        {
            target[segments[0]] = value;
            return;
        }
        if (!target.TryGetValue(segments[0], out var child))
        {
            child = new Dictionary<string, object>(StringComparer.Ordinal);
            target[segments[0]] = child;
        }
        AddDotted((Dictionary<string, object>)child, segments[1..], value);
    }

    private static void WriteObject(
        Utf8JsonWriter writer,
        Dictionary<string, object> value)
    {
        writer.WriteStartObject();
        foreach (var entry in value.OrderBy(entry => entry.Key, StringComparer.Ordinal))
        {
            if (entry.Value is string text)
            {
                writer.WriteString(entry.Key, text);
            }
            else
            {
                writer.WritePropertyName(entry.Key);
                WriteObject(writer, (Dictionary<string, object>)entry.Value);
            }
        }
        writer.WriteEndObject();
    }

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

    private static void ExpectCancellation(Action action, string message)
    {
        try
        {
            action();
        }
        catch (OperationCanceledException)
        {
            return;
        }
        throw new InvalidDataException(message);
    }
}

internal static class DesktopLanguagePackProjection
{
    private static readonly IReadOnlyDictionary<DesktopTextKey, string> Paths =
        new Dictionary<DesktopTextKey, string>
        {
            [DesktopTextKey.Language] = "language.label",
            [DesktopTextKey.ControlTopology] = "hero.title",
            [DesktopTextKey.HubSubcopy] = "hero.subcopy",
            [DesktopTextKey.LanguagePacks] = "languagePacks.title",
            [DesktopTextKey.InstallLanguagePack] = "languagePacks.install",
            [DesktopTextKey.RemoveLanguagePack] = "languagePacks.remove",
        };

    public static bool TryResolve(
        DesktopInstalledLanguagePack pack,
        DesktopTextKey key,
        out string value)
    {
        value = string.Empty;
        if (!Paths.TryGetValue(key, out var path)
            || !pack.Translations.TryGetValue(path, out var translated)
            || string.IsNullOrWhiteSpace(translated))
        {
            return false;
        }
        value = key switch
        {
            DesktopTextKey.Language => $"{translated}...",
            DesktopTextKey.InstallLanguagePack => $"{translated} JSON...",
            _ => translated,
        };
        return true;
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DesktopLanguagePack
{
    [JsonPropertyName("schema")]
    public required string Schema { get; init; }

    [JsonPropertyName("locale")]
    public required string Locale { get; init; }

    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("nativeName")]
    public required string NativeName { get; init; }

    [JsonPropertyName("version")]
    public required string Version { get; init; }

    [JsonPropertyName("author")]
    public string? Author { get; init; }

    [JsonPropertyName("direction")]
    public required string Direction { get; init; }

    [JsonPropertyName("coverage")]
    public required string Coverage { get; init; }

    [JsonPropertyName("translations")]
    public required JsonElement Translations { get; init; }
}

[JsonSourceGenerationOptions(GenerationMode = JsonSourceGenerationMode.Metadata)]
[JsonSerializable(typeof(DesktopLanguagePack))]
internal partial class DesktopLanguagePackJsonContext : JsonSerializerContext;
