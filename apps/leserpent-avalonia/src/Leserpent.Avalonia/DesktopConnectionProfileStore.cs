using System.Text.Json;
using System.Text.Json.Serialization;

internal sealed class DesktopConnectionProfileStore(string path)
{
    private const int SchemaVersion = 1;
    private const int MaxProfileBytes = 8 * 1024;

    public DesktopConnectionProfile? Load()
    {
        if (!File.Exists(path))
        {
            return null;
        }
        EnsureRegularFile(path);
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Length is <= 0 or > MaxProfileBytes)
        {
            throw new InvalidDataException("desktop connection profile has an invalid size");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        DesktopConnectionProfile profile;
        try
        {
            profile = JsonSerializer.Deserialize(
                payload,
                DesktopConnectionProfileJsonContext.Default.DesktopConnectionProfile)
                ?? throw new InvalidDataException("desktop connection profile is empty");
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("desktop connection profile JSON is invalid", error);
        }
        Validate(profile);
        return profile;
    }

    public void Save(DesktopConnectionProfile profile)
    {
        Validate(profile);
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            profile,
            DesktopConnectionProfileJsonContext.Default.DesktopConnectionProfile);
        if (payload.Length > MaxProfileBytes)
        {
            throw new InvalidDataException("desktop connection profile exceeds the size limit");
        }
        var directory = Path.GetDirectoryName(path)
            ?? throw new InvalidOperationException("desktop profile directory is unavailable");
        Directory.CreateDirectory(directory);
        if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException("desktop profile directory must not be a symbolic link");
        }
        if (File.Exists(path))
        {
            EnsureRegularFile(path);
        }
        var temporary = Path.Combine(directory, $".{Path.GetFileName(path)}.{Guid.NewGuid():N}.tmp");
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
                File.SetUnixFileMode(temporary, UnixFileMode.UserRead | UnixFileMode.UserWrite);
            }
            File.Move(temporary, path, true);
        }
        finally
        {
            File.Delete(temporary);
        }
    }

    public void Clear()
    {
        if (!File.Exists(path))
        {
            return;
        }
        EnsureRegularFile(path);
        File.Delete(path);
    }

    public static string DefaultPath()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(root))
        {
            throw new InvalidOperationException("local application data directory is unavailable");
        }
        return Path.Combine(root, "leserpent", "desktop-profile-v1.json");
    }

    public static void VerifyContract()
    {
        var root = Path.Combine(Path.GetTempPath(), $"leserpent-profile-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        var certificate = Path.Combine(root, "ca.pem");
        File.WriteAllText(certificate, "bounded certificate fixture");
        try
        {
            var store = new DesktopConnectionProfileStore(Path.Combine(root, "profile.json"));
            var profile = new DesktopConnectionProfile
            {
                SchemaVersion = SchemaVersion,
                Endpoint = "https://control.example:9443",
                CertificateAuthorityPath = certificate,
            };
            store.Save(profile);
            var loaded = store.Load();
            if (loaded != profile
                || JsonSerializer.Serialize(
                    loaded,
                    DesktopConnectionProfileJsonContext.Default.DesktopConnectionProfile)
                    .Contains("token", StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidDataException("desktop connection profile did not round-trip safely");
            }
            File.WriteAllText(Path.Combine(root, "profile.json"), "{\"schema_version\":1,\"unknown\":true}");
            ExpectInvalidData(
                () => _ = store.Load(),
                "desktop connection profile accepted an unknown field");

            var trustRoot = Path.Combine(root, "bootstrap-trust");
            Directory.CreateDirectory(trustRoot);
            if (!OperatingSystem.IsWindows())
            {
                File.SetUnixFileMode(
                    trustRoot,
                    UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
            }
            var trustProfile = new DesktopConnectionProfile
            {
                SchemaVersion = SchemaVersion,
                Endpoint = "https://control.example:9443",
                BootstrapTrustRoot = trustRoot,
                BootstrapTrustHandle = "vault:leserpent-ca:control-example",
            };
            store.Save(trustProfile);
            var trustJson = File.ReadAllText(Path.Combine(root, "profile.json"));
            if (store.Load() != trustProfile
                || trustJson.Contains("certificate_authority_path", StringComparison.Ordinal))
            {
                throw new InvalidDataException(
                    "desktop bootstrap trust profile did not round-trip safely");
            }
            ExpectInvalidData(
                () => store.Save(trustProfile with { BootstrapTrustHandle = null }),
                "desktop connection profile accepted an incomplete trust source");
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    internal static void Validate(DesktopConnectionProfile profile)
    {
        if (profile.SchemaVersion != SchemaVersion)
        {
            throw new InvalidDataException("unsupported desktop connection profile schema");
        }
        _ = RemoteClientOptions.ParseEndpoint(profile.Endpoint);
        var certificateAuthorityPath = profile.CertificateAuthorityPath;
        var hasCertificatePath = certificateAuthorityPath is not null;
        var hasTrustRoot = profile.BootstrapTrustRoot is not null;
        var hasTrustHandle = profile.BootstrapTrustHandle is not null;
        if (hasCertificatePath == (hasTrustRoot && hasTrustHandle)
            || hasTrustRoot != hasTrustHandle)
        {
            throw new InvalidDataException(
                "desktop profile must contain exactly one complete trust source");
        }
        if (!hasCertificatePath)
        {
            BootstrapTrustRecordStore.ValidateReference(
                profile.BootstrapTrustRoot!,
                profile.BootstrapTrustHandle!);
            return;
        }
        if (certificateAuthorityPath!.Length is <= 0 or > 4096
            || certificateAuthorityPath.Any(char.IsControl)
            || !Path.IsPathFullyQualified(certificateAuthorityPath))
        {
            throw new InvalidDataException("desktop profile CA path is invalid");
        }
        var info = new FileInfo(certificateAuthorityPath);
        if (!info.Exists
            || info.Length is <= 0 or > 1024 * 1024
            || (info.Attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("desktop profile CA must be a bounded regular file");
        }
    }

    private static void EnsureRegularFile(string candidate)
    {
        if ((File.GetAttributes(candidate)
            & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("desktop connection profile must be a regular file");
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
internal sealed record DesktopConnectionProfile
{
    public int SchemaVersion { get; set; }
    public required string Endpoint { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? CertificateAuthorityPath { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? BootstrapTrustRoot { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? BootstrapTrustHandle { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(DesktopConnectionProfile))]
internal partial class DesktopConnectionProfileJsonContext : JsonSerializerContext;
