using System.Text.Json;
using System.Text.Json.Serialization;

internal sealed class DesktopLanguagePreferenceStore(string path)
{
    private const int SchemaVersion = 1;
    private const int MaxPreferenceBytes = 2 * 1024;

    public string Load()
    {
        if (!File.Exists(path))
        {
            return DesktopLocalization.SystemPreference;
        }
        EnsureRegularFile(path);
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Length is <= 0 or > MaxPreferenceBytes)
        {
            throw new InvalidDataException("desktop language preference has an invalid size");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        DesktopLanguagePreference preference;
        try
        {
            preference = JsonSerializer.Deserialize(
                payload,
                DesktopLanguagePreferenceJsonContext.Default.DesktopLanguagePreference)
                ?? throw new InvalidDataException("desktop language preference is empty");
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("desktop language preference JSON is invalid", error);
        }
        Validate(preference);
        return preference.Locale;
    }

    public void Save(string locale)
    {
        DesktopLocalization.ValidatePreference(locale);
        var preference = new DesktopLanguagePreference
        {
            SchemaVersion = SchemaVersion,
            Locale = locale,
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            preference,
            DesktopLanguagePreferenceJsonContext.Default.DesktopLanguagePreference);
        if (payload.Length > MaxPreferenceBytes)
        {
            throw new InvalidDataException("desktop language preference exceeds the size limit");
        }
        var directory = Path.GetDirectoryName(path)
            ?? throw new InvalidOperationException("desktop language preference directory is unavailable");
        Directory.CreateDirectory(directory);
        if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                "desktop language preference directory must not be a symbolic link");
        }
        if (File.Exists(path))
        {
            EnsureRegularFile(path);
        }
        var temporary = Path.Combine(
            directory,
            $".{Path.GetFileName(path)}.{Guid.NewGuid():N}.tmp");
        try
        {
            using (var stream = new FileStream(
                temporary,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                4096,
                FileOptions.WriteThrough))
            {
                stream.Write(payload);
                stream.Flush(true);
            }
            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(
                    temporary,
                    UnixFileMode.UserRead | UnixFileMode.UserWrite);
            }
            File.Move(temporary, path, true);
        }
        finally
        {
            File.Delete(temporary);
        }
    }

    public static string DefaultPath()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(root))
        {
            throw new InvalidOperationException("local application data directory is unavailable");
        }
        return Path.Combine(root, "leserpent", "desktop-language-v1.json");
    }

    public static void VerifyContract()
    {
        var root = Path.Combine(Path.GetTempPath(), $"leserpent-language-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            var preferencePath = Path.Combine(root, "language.json");
            var store = new DesktopLanguagePreferenceStore(preferencePath);
            if (store.Load() != DesktopLocalization.SystemPreference)
            {
                throw new InvalidDataException("missing desktop language preference did not follow the system");
            }
            store.Save("zh-CN");
            if (store.Load() != "zh-CN")
            {
                throw new InvalidDataException("desktop language preference did not round-trip");
            }
            if (!OperatingSystem.IsWindows()
                && File.GetUnixFileMode(preferencePath)
                    != (UnixFileMode.UserRead | UnixFileMode.UserWrite))
            {
                throw new InvalidDataException("desktop language preference is not private");
            }
            File.WriteAllText(
                preferencePath,
                "{\"schema_version\":1,\"locale\":\"en\",\"unknown\":true}");
            ExpectInvalidData(
                () => _ = store.Load(),
                "desktop language preference accepted an unknown field");
            File.WriteAllText(
                preferencePath,
                "{\"schema_version\":1,\"locale\":\"not-an-official-locale\"}");
            ExpectInvalidData(
                () => _ = store.Load(),
                "desktop language preference accepted an unsupported locale");
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    private static void Validate(DesktopLanguagePreference preference)
    {
        if (preference.SchemaVersion != SchemaVersion)
        {
            throw new InvalidDataException("unsupported desktop language preference schema");
        }
        DesktopLocalization.ValidatePreference(preference.Locale);
    }

    private static void EnsureRegularFile(string candidate)
    {
        if ((File.GetAttributes(candidate)
            & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("desktop language preference must be a regular file");
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

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DesktopLanguagePreference
{
    public int SchemaVersion { get; set; }
    public required string Locale { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(DesktopLanguagePreference))]
internal partial class DesktopLanguagePreferenceJsonContext : JsonSerializerContext;
