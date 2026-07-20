using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Leserpent.ControlPlane;

public interface IRuntimeRegistrationAuthority
{
    bool Enabled { get; }

    Task<string> RegisterAsync(
        RuntimeRegistrationRequest request,
        string runtimeId,
        CancellationToken cancellationToken);
}

public sealed class DaemonRuntimeRegistrationAuthority : IRuntimeRegistrationAuthority
{
    private const int MaxFrameBytes = 1024 * 1024 + 1024;
    private readonly string? socketPath;
    private readonly string? token;
    private readonly TimeSpan timeout;

    public DaemonRuntimeRegistrationAuthority(IConfiguration configuration)
    {
        var configuredSocket = configuration["LESERPENT_DAEMON_SOCKET"];
        var configuredToken = configuration["LESERPENT_DAEMON_TOKEN"];
        if (string.IsNullOrWhiteSpace(configuredSocket) != string.IsNullOrWhiteSpace(configuredToken))
        {
            throw new InvalidOperationException(
                "LESERPENT_DAEMON_SOCKET and LESERPENT_DAEMON_TOKEN must be configured together");
        }
        if (!string.IsNullOrWhiteSpace(configuredSocket))
        {
            if (!Path.IsPathFullyQualified(configuredSocket))
            {
                throw new InvalidOperationException("LESERPENT_DAEMON_SOCKET must be an absolute path");
            }
            if (Encoding.UTF8.GetByteCount(configuredSocket) > 100)
            {
                throw new InvalidOperationException(
                    "LESERPENT_DAEMON_SOCKET must not exceed 100 UTF-8 bytes");
            }
            var tokenBytes = Encoding.UTF8.GetBytes(configuredToken!);
            if (tokenBytes.Length is < 32 or > 256 || tokenBytes.Any(value => value <= 0x20))
            {
                throw new InvalidOperationException(
                    "LESERPENT_DAEMON_TOKEN must contain 32 to 256 non-whitespace characters");
            }
            socketPath = Path.GetFullPath(configuredSocket);
            token = configuredToken;
        }

        var configuredTimeout = configuration.GetValue<int?>("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS")
            ?? configuration.GetValue<int?>("LESERPENT_DAEMON_DEPLOY_TIMEOUT_MS")
            ?? 5000;
        timeout = TimeSpan.FromMilliseconds(Math.Clamp(configuredTimeout, 100, 30_000));
    }

    public bool Enabled => socketPath is not null;

    public async Task<string> RegisterAsync(
        RuntimeRegistrationRequest request,
        string runtimeId,
        CancellationToken cancellationToken)
    {
        if (!Enabled)
        {
            throw new InvalidOperationException("leserpentd runtime registration authority is not configured");
        }
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("leserpentd Unix socket registration is unavailable on Windows");
        }

        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            var command = BuildCommand(request, runtimeId);
            using var response = await ExchangeAsync(command, deadline.Token);
            return ParseRuntimeId(response.RootElement, runtimeId);
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_registration_timeout",
                "leserpentd runtime registration timed out",
                error);
        }
        catch (Exception error) when (
            error is KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd returned an invalid registration protocol response",
                error);
        }
    }

    private async Task<JsonDocument> ExchangeAsync(byte[] request, CancellationToken cancellationToken)
    {
        ValidateSocketBoundary();
        using var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        try
        {
            await socket.ConnectAsync(new UnixDomainSocketEndPoint(socketPath!), cancellationToken);
            using var stream = new NetworkStream(socket, ownsSocket: false);
            await stream.WriteAsync(request, cancellationToken);
            await stream.FlushAsync(cancellationToken);
            socket.Shutdown(SocketShutdown.Send);
            var response = await ReadFrameAsync(socket, cancellationToken);
            return JsonDocument.Parse(response);
        }
        catch (Exception error) when (error is SocketException or IOException or JsonException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_transport_failed",
                "leserpentd registration transport failed",
                error);
        }
    }

    private void ValidateSocketBoundary()
    {
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException(
                "leserpentd Unix socket registration is unavailable on Windows");
        }
        try
        {
            var attributes = File.GetAttributes(socketPath!);
            if ((attributes & FileAttributes.ReparsePoint) != 0)
            {
                throw new DaemonRuntimeRegistrationException(
                    "daemon_socket_unsafe",
                    "leserpentd socket must not be a symbolic link");
            }
            var mode = File.GetUnixFileMode(socketPath!);
            const UnixFileMode unsafePermissions = UnixFileMode.GroupRead
                | UnixFileMode.GroupWrite
                | UnixFileMode.GroupExecute
                | UnixFileMode.OtherRead
                | UnixFileMode.OtherWrite
                | UnixFileMode.OtherExecute;
            if ((mode & unsafePermissions) != 0)
            {
                throw new DaemonRuntimeRegistrationException(
                    "daemon_socket_unsafe",
                    "leserpentd socket must be owner-private");
            }
        }
        catch (DaemonRuntimeRegistrationException)
        {
            throw;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_transport_failed",
                "leserpentd socket is unavailable",
                error);
        }
    }

    private static async Task<byte[]> ReadFrameAsync(Socket socket, CancellationToken cancellationToken)
    {
        using var output = new MemoryStream();
        var buffer = new byte[4096];
        while (true)
        {
            var read = await socket.ReceiveAsync(buffer, SocketFlags.None, cancellationToken);
            if (read == 0)
            {
                throw new IOException("leserpentd closed the socket before a complete frame");
            }
            var newline = Array.IndexOf(buffer, (byte)'\n', 0, read);
            var bodyBytes = newline < 0 ? read : newline;
            if (output.Length + bodyBytes > MaxFrameBytes)
            {
                throw new IOException("leserpentd response exceeds the protocol limit");
            }
            output.Write(buffer, 0, bodyBytes);
            if (newline >= 0)
            {
                if (newline != read - 1)
                {
                    throw new IOException("leserpentd returned data after the protocol frame");
                }
                return output.ToArray();
            }
        }
    }

    private byte[] BuildCommand(RuntimeRegistrationRequest request, string runtimeId)
    {
        var tags = request.Tags ?? new RuntimeTags(null, null, null);
        var stableCommandId = BuildDeterministicCommandId(runtimeId, request.Name, request.Endpoint);
        return BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteString("kind", "command");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteString("command_id", stableCommandId);
            writer.WriteString("idempotency_key", stableCommandId);
            writer.WriteNull("expected_revision");
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer);
            writer.WriteString("origin", "compatibility_adapter");
            writer.WriteString("confirmation", "confirmed");
            writer.WriteBoolean("dry_run", false);
            writer.WritePropertyName("command");
            writer.WriteStartObject();
            writer.WriteString("kind", "runtime_register");
            writer.WriteString("runtime_id", runtimeId);
            writer.WriteString("name", request.Name.Trim());
            writer.WriteString("endpoint", request.Endpoint.Trim());
            writer.WritePropertyName("tags");
            writer.WriteStartObject();
            WriteOptionalString(writer, "environment", tags.Environment?.Trim());
            WriteOptionalString(writer, "cluster", tags.Cluster?.Trim());
            WriteOptionalString(writer, "role", tags.Role?.Trim());
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
    }

    private byte[] BuildFrame(Action<Utf8JsonWriter> writeRequest)
    {
        using var output = new MemoryStream();
        using (var writer = new Utf8JsonWriter(output))
        {
            writer.WriteStartObject();
            writer.WriteString("token", token);
            writer.WritePropertyName("request");
            writeRequest(writer);
            writer.WriteEndObject();
        }
        if (output.Length + 1 > MaxFrameBytes)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_request_oversized",
                "leserpentd registration request exceeds the protocol limit");
        }
        output.WriteByte((byte)'\n');
        return output.ToArray();
    }

    private static string ParseRuntimeId(JsonElement root, string expectedRuntimeId)
    {
        var payload = RequireResponse(root, "command");
        if (!string.Equals(payload.GetProperty("status").GetString(), "applied", StringComparison.Ordinal)
            || payload.GetProperty("command_id").ValueKind == JsonValueKind.Null)
        {
            throw new InvalidOperationException("leserpentd registration response is invalid");
        }
        var registeredRuntimeId = payload.GetProperty("runtime").GetProperty("id").GetString();
        if (!string.Equals(registeredRuntimeId, expectedRuntimeId, StringComparison.Ordinal))
        {
            throw new InvalidOperationException("leserpentd registration response did not echo the requested runtime ID");
        }
        return registeredRuntimeId!;
    }

    private static JsonElement RequireResponse(JsonElement root, string expectedKind)
    {
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unsupported protocol version");
        }
        var response = root.GetProperty("response");
        var kind = response.GetProperty("kind").GetString();
        if (string.Equals(kind, "error", StringComparison.Ordinal))
        {
            var error = response.GetProperty("payload");
            throw new DaemonRuntimeRegistrationException(
                error.GetProperty("code").GetString() ?? "daemon_request_failed",
                error.GetProperty("message").GetString() ?? "leserpentd rejected the registration request");
        }
        if (!string.Equals(kind, expectedKind, StringComparison.Ordinal))
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unexpected response kind");
        }
        return response.GetProperty("payload");
    }

    private static void WritePrincipal(Utf8JsonWriter writer, string principal)
    {
        writer.WritePropertyName("principal");
        writer.WriteStartObject();
        writer.WriteString("id", principal);
        writer.WriteEndObject();
    }

    private static void WriteCapabilities(Utf8JsonWriter writer)
    {
        writer.WritePropertyName("capabilities");
        writer.WriteStartArray();
        writer.WriteStringValue("runtime.register");
        writer.WriteEndArray();
    }

    private static void WriteOptionalString(Utf8JsonWriter writer, string name, string? value)
    {
        if (value is null)
        {
            writer.WriteNull(name);
        }
        else
        {
            writer.WriteString(name, value);
        }
    }

    private static string BuildDeterministicCommandId(string runtimeId, string name, string endpoint)
    {
        var normalizedName = name.Trim();
        var normalizedEndpoint = endpoint.Trim();
        var bytes = HashData($"{runtimeId}|{normalizedName}|{normalizedEndpoint}");
        return Convert.ToHexString(bytes).ToLowerInvariant().Substring(0, 32);
    }

    private static byte[] HashData(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        return SHA256.HashData(bytes);
    }
}

public sealed class DaemonRuntimeRegistrationException : Exception
{
    public DaemonRuntimeRegistrationException(string code, string message, Exception? innerException = null)
        : base(message, innerException)
    {
        Code = code;
    }

    public string Code { get; }
}
