using System.Net.Sockets;
using System.Text;
using System.Text.Json;

namespace Leserpent.ControlPlane;

public sealed record AuthorityWriterTicket(
    ulong Generation,
    string WriterId);

internal static class AuthorityWriterFrame
{
    public static void Write(
        Utf8JsonWriter writer,
        AuthorityWriterTicket? ticket)
    {
        if (ticket is null)
        {
            return;
        }
        writer.WritePropertyName("writer_fence");
        writer.WriteStartObject();
        writer.WriteNumber("generation", ticket.Generation);
        writer.WriteString("writer_id", ticket.WriterId);
        writer.WriteEndObject();
    }
}

public sealed class DaemonAuthorityWriterSession
{
    private const int MaxFrameBytes = 1024 * 1024 + 1024;
    private readonly string? socketPath;
    private readonly string? token;
    private readonly string writerId = Guid.NewGuid().ToString("N");
    private readonly TimeSpan timeout;
    private readonly object sync = new();
    private AuthorityWriterTicket? ticket;

    public DaemonAuthorityWriterSession(IConfiguration configuration)
    {
        var configuredTimeout = configuration.GetValue<int?>(
                "LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS")
            ?? configuration.GetValue<int?>(
                "LESERPENT_DAEMON_DEPLOY_TIMEOUT_MS")
            ?? 5000;
        timeout = TimeSpan.FromMilliseconds(
            Math.Clamp(configuredTimeout, 100, 30_000));
        var configuredSocket = configuration["LESERPENT_DAEMON_SOCKET"];
        var configuredToken = configuration["LESERPENT_DAEMON_TOKEN"];
        if (string.IsNullOrWhiteSpace(configuredSocket) !=
            string.IsNullOrWhiteSpace(configuredToken))
        {
            throw new InvalidOperationException(
                "LESERPENT_DAEMON_SOCKET and LESERPENT_DAEMON_TOKEN must be configured together");
        }
        if (string.IsNullOrWhiteSpace(configuredSocket))
        {
            return;
        }
        if (!Path.IsPathFullyQualified(configuredSocket))
        {
            throw new InvalidOperationException(
                "LESERPENT_DAEMON_SOCKET must be an absolute path");
        }
        if (Encoding.UTF8.GetByteCount(configuredSocket) > 100)
        {
            throw new InvalidOperationException(
                "LESERPENT_DAEMON_SOCKET must not exceed 100 UTF-8 bytes");
        }
        var tokenBytes = Encoding.UTF8.GetBytes(configuredToken!);
        if (tokenBytes.Length is < 32 or > 256 ||
            tokenBytes.Any(value => value <= 0x20))
        {
            throw new InvalidOperationException(
                "LESERPENT_DAEMON_TOKEN must contain 32 to 256 non-whitespace characters");
        }
        socketPath = Path.GetFullPath(configuredSocket);
        token = configuredToken;
    }

    public bool Enabled => socketPath is not null;

    public AuthorityWriterTicket? Ticket
    {
        get
        {
            lock (sync)
            {
                return ticket;
            }
        }
    }

    public async Task<AuthorityWriterTicket?> ClaimAsync(
        CancellationToken cancellationToken)
    {
        if (!Enabled)
        {
            return null;
        }
        var retained = Ticket;
        if (retained is not null)
        {
            return retained;
        }

        using var deadline =
            CancellationTokenSource.CreateLinkedTokenSource(
                cancellationToken);
        deadline.CancelAfter(timeout);
        byte[] response;
        try
        {
            ValidateSocketBoundary();
            using var socket = new Socket(
                AddressFamily.Unix,
                SocketType.Stream,
                ProtocolType.Unspecified);
            await socket.ConnectAsync(
                new UnixDomainSocketEndPoint(socketPath!),
                deadline.Token);
            using var stream =
                new NetworkStream(socket, ownsSocket: false);
            var request = BuildClaimFrame();
            await stream.WriteAsync(request, deadline.Token);
            await stream.FlushAsync(deadline.Token);
            socket.Shutdown(SocketShutdown.Send);
            response = await ReadFrameAsync(
                socket,
                deadline.Token);
        }
        catch (OperationCanceledException error)
            when (!cancellationToken.IsCancellationRequested)
        {
            throw new TimeoutException(
                "leserpentd authority writer claim timed out",
                error);
        }
        using var document = JsonDocument.Parse(response);
        var root = document.RootElement;
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new InvalidOperationException(
                "leserpentd returned an unsupported authority writer protocol version");
        }
        var envelope = root.GetProperty("response");
        var kind = envelope.GetProperty("kind").GetString();
        if (string.Equals(kind, "error", StringComparison.Ordinal))
        {
            var error = envelope.GetProperty("payload");
            throw new InvalidOperationException(
                error.GetProperty("message").GetString() ??
                "leserpentd rejected the authority writer claim");
        }
        if (!string.Equals(
                kind,
                "authority_writer_claimed",
                StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "leserpentd returned an unexpected authority writer response");
        }
        var payload = envelope.GetProperty("payload");
        var generation = payload.GetProperty("generation").GetUInt64();
        var claimedWriterId =
            payload.GetProperty("writer_id").GetString();
        if (generation == 0 ||
            !string.Equals(
                claimedWriterId,
                writerId,
                StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "leserpentd returned an invalid authority writer claim");
        }
        var claimed = new AuthorityWriterTicket(
            generation,
            writerId);
        lock (sync)
        {
            ticket ??= claimed;
            return ticket;
        }
    }

    private byte[] BuildClaimFrame()
    {
        using var output = new MemoryStream();
        using (var writer = new Utf8JsonWriter(output))
        {
            writer.WriteStartObject();
            writer.WriteString("token", token);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "authority_writer_claim");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WritePropertyName("principal");
            writer.WriteStartObject();
            writer.WriteString("id", "operator");
            writer.WriteEndObject();
            writer.WritePropertyName("capabilities");
            writer.WriteStartArray();
            writer.WriteStringValue("authority.writer");
            writer.WriteEndArray();
            writer.WriteString("writer_id", writerId);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        }
        output.WriteByte((byte)'\n');
        return output.ToArray();
    }

    private void ValidateSocketBoundary()
    {
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException(
                "leserpentd Unix socket authority writer claim is unavailable on Windows");
        }
        var attributes = File.GetAttributes(socketPath!);
        if ((attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidOperationException(
                "leserpentd socket must not be a symbolic link");
        }
        var mode = File.GetUnixFileMode(socketPath!);
        const UnixFileMode unsafePermissions =
            UnixFileMode.GroupRead |
            UnixFileMode.GroupWrite |
            UnixFileMode.GroupExecute |
            UnixFileMode.OtherRead |
            UnixFileMode.OtherWrite |
            UnixFileMode.OtherExecute;
        if ((mode & unsafePermissions) != 0)
        {
            throw new InvalidOperationException(
                "leserpentd socket must be owner-private");
        }
    }

    private static async Task<byte[]> ReadFrameAsync(
        Socket socket,
        CancellationToken cancellationToken)
    {
        using var output = new MemoryStream();
        var buffer = new byte[4096];
        while (true)
        {
            var read = await socket.ReceiveAsync(
                buffer,
                SocketFlags.None,
                cancellationToken);
            if (read == 0)
            {
                throw new IOException(
                    "leserpentd closed the authority writer claim before a complete frame");
            }
            var newline = Array.IndexOf(buffer, (byte)'\n', 0, read);
            var bodyBytes = newline < 0 ? read : newline;
            if (output.Length + bodyBytes > MaxFrameBytes)
            {
                throw new IOException(
                    "leserpentd authority writer response exceeds the protocol limit");
            }
            output.Write(buffer, 0, bodyBytes);
            if (newline < 0)
            {
                continue;
            }
            if (newline != read - 1)
            {
                throw new IOException(
                    "leserpentd returned data after the authority writer frame");
            }
            return output.ToArray();
        }
    }
}
