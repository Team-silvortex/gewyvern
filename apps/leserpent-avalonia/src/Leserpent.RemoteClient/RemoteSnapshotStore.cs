using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

public sealed class RemoteSnapshotStore
{
    private const int MaxCacheBytes = RemoteEventCodec.MaxMessageBytes;
    private readonly string path;
    private readonly string endpointHash;

    public RemoteSnapshotStore(Uri endpoint, string path)
    {
        if (!Path.IsPathFullyQualified(path))
        {
            throw new ArgumentException("remote cache path must be absolute", nameof(path));
        }
        this.path = path;
        endpointHash = HashEndpoint(endpoint);
    }

    public static string DefaultPath(Uri endpoint)
    {
        var root = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(root))
        {
            throw new InvalidOperationException("local application data directory is unavailable");
        }
        return Path.Combine(root, "leserpent", $"remote-{HashEndpoint(endpoint)}.json");
    }

    public RemoteSnapshotCache? Load()
    {
        if (!File.Exists(path))
        {
            return null;
        }
        EnsureRegularFile(path);
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        if (stream.Length is <= 0 or > MaxCacheBytes)
        {
            throw new InvalidDataException("remote snapshot cache has an invalid size");
        }
        var payload = new byte[checked((int)stream.Length)];
        stream.ReadExactly(payload);
        RemoteSnapshotCache cache;
        try
        {
            cache = JsonSerializer.Deserialize(
                payload,
                RemoteEventJsonContext.Default.RemoteSnapshotCache)
                ?? throw new InvalidDataException("remote snapshot cache is empty");
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("remote snapshot cache JSON is invalid", error);
        }
        if (cache.SchemaVersion != RemoteEventCodec.SchemaVersion
            || !CryptographicOperations.FixedTimeEquals(
                Encoding.ASCII.GetBytes(cache.EndpointHash),
                Encoding.ASCII.GetBytes(endpointHash)))
        {
            throw new InvalidDataException("remote snapshot cache identity is invalid");
        }
        return cache;
    }

    public void Save(RemoteEvent.Snapshot snapshot)
    {
        var cache = new RemoteSnapshotCache
        {
            SchemaVersion = RemoteEventCodec.SchemaVersion,
            EndpointHash = endpointHash,
            Revision = snapshot.Revision,
            Runtimes = snapshot.Runtimes.ToList(),
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            cache,
            RemoteEventJsonContext.Default.RemoteSnapshotCache);
        if (payload.Length > MaxCacheBytes)
        {
            throw new InvalidDataException("remote snapshot cache exceeds the size limit");
        }

        var directory = Path.GetDirectoryName(path)
            ?? throw new InvalidOperationException("remote cache directory is unavailable");
        Directory.CreateDirectory(directory);
        if ((File.GetAttributes(directory) & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException("remote cache directory must not be a symbolic link");
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
            SetPrivatePermissions(temporary);
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

    private static string HashEndpoint(Uri endpoint)
    {
        var canonical = endpoint.GetComponents(
            UriComponents.SchemeAndServer,
            UriFormat.UriEscaped).ToLowerInvariant();
        return Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(canonical)))
            .ToLowerInvariant();
    }

    private static void EnsureRegularFile(string candidate)
    {
        var attributes = File.GetAttributes(candidate);
        if ((attributes & (FileAttributes.Directory | FileAttributes.ReparsePoint)) != 0)
        {
            throw new InvalidDataException("remote snapshot cache must be a regular file");
        }
    }

    private static void SetPrivatePermissions(string candidate)
    {
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(candidate, UnixFileMode.UserRead | UnixFileMode.UserWrite);
        }
    }
}
