using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

internal sealed class DesktopConnectionCatalogStore(string path)
{
    private const int SchemaVersion = 1;
    private const int MaxConnections = 64;
    private const int MaxCatalogBytes = 256 * 1024;

    public DesktopConnectionCatalog Load()
    {
        if (!File.Exists(path))
        {
            return DesktopConnectionCatalog.Empty;
        }
        EnsureRegularFile(path);
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Length is <= 0 or > MaxCatalogBytes)
        {
            throw new InvalidDataException("desktop connection catalog has an invalid size");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        DesktopConnectionCatalog catalog;
        try
        {
            catalog = JsonSerializer.Deserialize(
                payload,
                DesktopConnectionCatalogJsonContext.Default.DesktopConnectionCatalog)
                ?? throw new InvalidDataException("desktop connection catalog is empty");
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("desktop connection catalog JSON is invalid", error);
        }
        Validate(catalog);
        return catalog;
    }

    public DesktopConnectionCatalog LoadOrMigrate(
        DesktopConnectionProfileStore legacyStore)
    {
        if (File.Exists(path))
        {
            return Load();
        }
        var legacy = legacyStore.Load();
        if (legacy is null)
        {
            return DesktopConnectionCatalog.Empty;
        }
        var catalog = new DesktopConnectionCatalog
        {
            SchemaVersion = SchemaVersion,
            Connections = [DesktopDaemonConnection.FromProfile(legacy)],
        };
        Save(catalog);
        legacyStore.Clear();
        return catalog;
    }

    public void Save(DesktopConnectionCatalog catalog)
    {
        Validate(catalog);
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            catalog,
            DesktopConnectionCatalogJsonContext.Default.DesktopConnectionCatalog);
        if (payload.Length > MaxCatalogBytes)
        {
            throw new InvalidDataException("desktop connection catalog exceeds the size limit");
        }
        var directory = Path.GetDirectoryName(path)
            ?? throw new InvalidOperationException("desktop catalog directory is unavailable");
        Directory.CreateDirectory(directory);
        if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException("desktop catalog directory must not be a symbolic link");
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

    public DesktopConnectionCatalog Upsert(
        DesktopDaemonConnection connection,
        string? expectedDaemonId = null)
    {
        var current = Load();
        var connections = current.Connections.ToList();
        var index = expectedDaemonId is null
            ? connections.FindIndex(item => item.DaemonId == connection.DaemonId)
            : connections.FindIndex(item => item.DaemonId == expectedDaemonId);
        if (expectedDaemonId is not null && index < 0)
        {
            throw new InvalidDataException(
                "the saved daemon connection changed; reopen the Hub before editing it");
        }
        if (index < 0)
        {
            connections.Add(connection);
        }
        else
        {
            connections[index] = connection;
        }
        var updated = current with { Connections = connections };
        Save(updated);
        return updated;
    }

    public DesktopConnectionCatalog Remove(DesktopDaemonConnection expected)
    {
        var current = Load();
        var connections = current.Connections.ToList();
        var index = connections.FindIndex(item => item.DaemonId == expected.DaemonId);
        if (index < 0 || connections[index] != expected)
        {
            throw new InvalidDataException(
                "the saved daemon connection changed; reopen the Hub before removing it");
        }
        connections.RemoveAt(index);
        var updated = current with { Connections = connections };
        Save(updated);
        return updated;
    }

    public static string DefaultPath()
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(root))
        {
            throw new InvalidOperationException("local application data directory is unavailable");
        }
        return Path.Combine(root, "leserpent", "desktop-connections-v1.json");
    }

    public static void VerifyContract()
    {
        var root = Path.Combine(Path.GetTempPath(), $"leserpent-catalog-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            var firstCa = Path.Combine(root, "first-ca.pem");
            var secondCa = Path.Combine(root, "second-ca.pem");
            File.WriteAllText(firstCa, "bounded first certificate fixture");
            File.WriteAllText(secondCa, "bounded second certificate fixture");
            var legacyStore = new DesktopConnectionProfileStore(Path.Combine(root, "legacy.json"));
            var first = Profile("https://alpha.example:9443", firstCa);
            legacyStore.Save(first);
            var store = new DesktopConnectionCatalogStore(Path.Combine(root, "catalog.json"));
            var migrated = store.LoadOrMigrate(legacyStore);
            if (migrated.Connections.Count != 1
                || migrated.Connections[0].Profile != first
                || legacyStore.Load() is not null)
            {
                throw new InvalidDataException("legacy desktop profile was not migrated atomically");
            }

            var second = DesktopDaemonConnection.FromProfile(
                Profile("https://beta.example:9443", secondCa));
            var updated = store.Upsert(second);
            if (updated.Connections.Count != 2
                || store.Load().Connections.Select(item => item.DaemonId).Distinct().Count() != 2)
            {
                throw new InvalidDataException("desktop catalog did not preserve multiple daemons");
            }
            store.Remove(migrated.Connections[0]);
            if (store.Load().Connections is not [var remaining]
                || remaining != second)
            {
                throw new InvalidDataException("desktop catalog removed the wrong daemon");
            }

            File.WriteAllText(
                Path.Combine(root, "catalog.json"),
                "{\"schema_version\":1,\"connections\":[],\"unknown\":true}");
            ExpectInvalidData(
                () => _ = store.Load(),
                "desktop catalog accepted an unknown field");
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    private static DesktopConnectionProfile Profile(string endpoint, string certificate) => new()
    {
        SchemaVersion = 1,
        Endpoint = endpoint,
        CertificateAuthorityPath = certificate,
    };

    private static void Validate(DesktopConnectionCatalog catalog)
    {
        if (catalog.SchemaVersion != SchemaVersion)
        {
            throw new InvalidDataException("unsupported desktop connection catalog schema");
        }
        if (catalog.Connections.Count > MaxConnections)
        {
            throw new InvalidDataException("desktop connection catalog is too large");
        }
        var daemonIds = new HashSet<string>(StringComparer.Ordinal);
        var endpoints = new HashSet<string>(StringComparer.Ordinal);
        foreach (var connection in catalog.Connections)
        {
            DesktopConnectionProfileStore.Validate(connection.Profile);
            var expectedId = DesktopDaemonConnection.DeriveDaemonId(connection.Profile.Endpoint);
            if (connection.DaemonId != expectedId || !daemonIds.Add(connection.DaemonId))
            {
                throw new InvalidDataException("desktop connection catalog contains an invalid daemon ID");
            }
            if (connection.DisplayName.Length is <= 0 or > 96
                || connection.DisplayName != connection.DisplayName.Trim()
                || connection.DisplayName.Any(char.IsControl))
            {
                throw new InvalidDataException("desktop connection display name is invalid");
            }
            var endpoint = RemoteClientOptions.ParseEndpoint(connection.Profile.Endpoint).ToString();
            if (!endpoints.Add(endpoint))
            {
                throw new InvalidDataException("desktop connection catalog contains a duplicate daemon");
            }
        }
    }

    private static void EnsureRegularFile(string candidate)
    {
        if ((File.GetAttributes(candidate)
            & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("desktop connection catalog must be a regular file");
        }
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
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DesktopConnectionCatalog
{
    public int SchemaVersion { get; set; }
    public required IReadOnlyList<DesktopDaemonConnection> Connections { get; set; }

    public static DesktopConnectionCatalog Empty => new()
    {
        SchemaVersion = 1,
        Connections = [],
    };
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DesktopDaemonConnection
{
    public required string DaemonId { get; set; }
    public required string DisplayName { get; set; }
    public required DesktopConnectionProfile Profile { get; set; }

    public static DesktopDaemonConnection FromProfile(DesktopConnectionProfile profile)
    {
        var endpoint = RemoteClientOptions.ParseEndpoint(profile.Endpoint);
        return new DesktopDaemonConnection
        {
            DaemonId = DeriveDaemonId(profile.Endpoint),
            DisplayName = endpoint.IsDefaultPort
                ? endpoint.Host
                : $"{endpoint.Host}:{endpoint.Port}",
            Profile = profile,
        };
    }

    public static string DeriveDaemonId(string endpoint)
    {
        var authority = RemoteClientOptions.ParseEndpoint(endpoint).ToString().ToLowerInvariant();
        var digest = SHA256.HashData(Encoding.UTF8.GetBytes(authority));
        return $"daemon-{Convert.ToHexString(digest.AsSpan(0, 12)).ToLowerInvariant()}";
    }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(DesktopConnectionCatalog))]
internal partial class DesktopConnectionCatalogJsonContext : JsonSerializerContext;
