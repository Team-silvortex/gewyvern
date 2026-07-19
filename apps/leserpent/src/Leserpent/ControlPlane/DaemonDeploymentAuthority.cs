using System.Net.Sockets;
using System.Text;
using System.Text.Json;

namespace Leserpent.ControlPlane;

public interface IDeploymentAuthority
{
    bool Enabled { get; }

    Task<RuntimeDeploymentResponse> DeployAsync(
        RuntimeControlAccess runtime,
        RuntimeDeploymentRequest request,
        CancellationToken cancellationToken);
}

public sealed class DaemonDeploymentAuthority : IDeploymentAuthority
{
    private const int MaxFrameBytes = 1024 * 1024 + 1024;
    private readonly string? socketPath;
    private readonly string? token;
    private readonly TimeSpan timeout;

    public DaemonDeploymentAuthority(IConfiguration configuration)
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
            if (tokenBytes.Length is < 32 or > 256
                || tokenBytes.Any(value => value <= 0x20))
            {
                throw new InvalidOperationException(
                    "LESERPENT_DAEMON_TOKEN must contain 32 to 256 non-whitespace characters");
            }
            socketPath = Path.GetFullPath(configuredSocket);
            token = configuredToken;
        }

        var configuredTimeout = configuration.GetValue<int?>("LESERPENT_DAEMON_DEPLOY_TIMEOUT_MS") ?? 5000;
        timeout = TimeSpan.FromMilliseconds(Math.Clamp(configuredTimeout, 100, 30_000));
    }

    public bool Enabled => socketPath is not null;

    public async Task<RuntimeDeploymentResponse> DeployAsync(
        RuntimeControlAccess runtime,
        RuntimeDeploymentRequest request,
        CancellationToken cancellationToken)
    {
        if (!Enabled)
        {
            throw new InvalidOperationException("leserpentd deployment authority is not configured");
        }
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("leserpentd Unix socket deployment is unavailable on Windows");
        }

        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            var command = BuildCommand(runtime.RuntimeId, request);
            using (var response = await ExchangeAsync(command, deadline.Token))
            {
                ValidateCommandResponse(response.RootElement, request.RequestId);
            }

            var delay = TimeSpan.FromMilliseconds(20);
            while (true)
            {
                using var response = await ExchangeAsync(BuildReceipt(request), deadline.Token);
                var receipt = ParseReceipt(response.RootElement, runtime.RuntimeId, request);
                if (receipt is not null)
                {
                    return receipt;
                }
                await Task.Delay(delay, deadline.Token);
                delay = TimeSpan.FromMilliseconds(Math.Min(delay.TotalMilliseconds * 2, 200));
            }
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new DaemonDeploymentException(
                "daemon_deployment_timeout",
                "leserpentd deployment timed out",
                error);
        }
        catch (Exception error) when (
            error is KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException
                or ArgumentOutOfRangeException)
        {
            throw new DaemonDeploymentException(
                "daemon_protocol_invalid",
                "leserpentd returned an invalid deployment protocol response",
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
            throw new DaemonDeploymentException(
                "daemon_transport_failed",
                "leserpentd deployment transport failed",
                error);
        }
    }

    private void ValidateSocketBoundary()
    {
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException(
                "leserpentd Unix socket deployment is unavailable on Windows");
        }
        try
        {
            var attributes = File.GetAttributes(socketPath!);
            if ((attributes & FileAttributes.ReparsePoint) != 0)
            {
                throw new DaemonDeploymentException(
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
                throw new DaemonDeploymentException(
                    "daemon_socket_unsafe",
                    "leserpentd socket must be owner-private");
            }
        }
        catch (DaemonDeploymentException)
        {
            throw;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            throw new DaemonDeploymentException(
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

    private byte[] BuildCommand(string runtimeId, RuntimeDeploymentRequest request) =>
        BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "command");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteString("command_id", request.RequestId);
            writer.WriteString("idempotency_key", request.RequestId);
            writer.WriteNull("expected_revision");
            WritePrincipal(writer, request.RequestedBy);
            WriteCapabilities(writer);
            writer.WriteString("origin", "compatibility_adapter");
            writer.WriteString("confirmation", "confirmed");
            writer.WriteBoolean("dry_run", false);
            writer.WritePropertyName("command");
            writer.WriteStartObject();
            writer.WriteString("kind", "runtime_deploy");
            writer.WriteString("runtime_id", runtimeId);
            writer.WriteString("pipeline_kind", request.PipelineKind);
            WriteOptionalString(writer, "target", request.Target);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });

    private byte[] BuildReceipt(RuntimeDeploymentRequest request) =>
        BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "deployment_receipt");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            WritePrincipal(writer, request.RequestedBy);
            WriteCapabilities(writer);
            writer.WriteString("command_id", request.RequestId);
            writer.WriteString("request_id", request.RequestId);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });

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
            throw new DaemonDeploymentException(
                "daemon_request_oversized",
                "leserpentd deployment request exceeds the protocol limit");
        }
        output.WriteByte((byte)'\n');
        return output.ToArray();
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
        writer.WriteStringValue("runtime.deploy");
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

    private static void ValidateCommandResponse(JsonElement root, string requestId)
    {
        var payload = RequireResponse(root, "command");
        if (!string.Equals(payload.GetProperty("command_id").GetString(), requestId, StringComparison.Ordinal)
            || !string.Equals(payload.GetProperty("status").GetString(), "applied", StringComparison.Ordinal))
        {
            throw new DaemonDeploymentException(
                "daemon_command_mismatch",
                "leserpentd returned a mismatched deployment command result");
        }
    }

    private static RuntimeDeploymentResponse? ParseReceipt(
        JsonElement root,
        string runtimeId,
        RuntimeDeploymentRequest request)
    {
        var payload = RequireResponse(root, "deployment_receipt");
        if (!string.Equals(payload.GetProperty("command_id").GetString(), request.RequestId, StringComparison.Ordinal)
            || !string.Equals(payload.GetProperty("request_id").GetString(), request.RequestId, StringComparison.Ordinal))
        {
            throw new DaemonDeploymentException(
                "daemon_receipt_mismatch",
                "leserpentd returned a mismatched deployment receipt");
        }
        var status = payload.GetProperty("status").GetString();
        if (string.Equals(status, "pending", StringComparison.Ordinal))
        {
            return null;
        }
        if (string.Equals(status, "failed", StringComparison.Ordinal))
        {
            var message = payload.TryGetProperty("error", out var error)
                ? error.GetString()
                : "leserpentd deployment failed";
            throw new DaemonDeploymentException(
                message?.Contains("conflicts with an existing request", StringComparison.Ordinal) == true
                    ? "runtime_deployment_request_conflict"
                    : "runtime_deployment_rejected",
                message ?? "leserpentd deployment failed");
        }
        if (!string.Equals(status, "completed", StringComparison.Ordinal)
            || !payload.TryGetProperty("outcome", out var outcome))
        {
            throw new DaemonDeploymentException(
                "daemon_receipt_invalid",
                "leserpentd returned an invalid deployment receipt state");
        }
        var requestId = outcome.GetProperty("request_id").GetString();
        var pipelineKind = outcome.GetProperty("pipeline_kind").GetString();
        var requestedBy = outcome.GetProperty("requested_by").GetString();
        var target = outcome.TryGetProperty("target", out var targetElement)
            && targetElement.ValueKind != JsonValueKind.Null
            ? targetElement.GetString()
            : null;
        if (!string.Equals(requestId, request.RequestId, StringComparison.Ordinal)
            || !string.Equals(pipelineKind, request.PipelineKind, StringComparison.Ordinal)
            || !string.Equals(requestedBy, request.RequestedBy, StringComparison.Ordinal)
            || !string.Equals(target, request.Target, StringComparison.Ordinal)
            || !string.Equals(outcome.GetProperty("status").GetString(), "accepted", StringComparison.Ordinal))
        {
            throw new DaemonDeploymentException(
                "daemon_outcome_mismatch",
                "leserpentd returned a deployment outcome that does not match the request");
        }
        long acceptedUnixMs;
        try
        {
            acceptedUnixMs = outcome.GetProperty("accepted_unix_ms").GetInt64();
        }
        catch (Exception error) when (error is InvalidOperationException or FormatException or OverflowException)
        {
            throw new DaemonDeploymentException(
                "daemon_outcome_invalid",
                "leserpentd returned an invalid deployment timestamp",
                error);
        }
        return new RuntimeDeploymentResponse(
            outcome.GetProperty("deployment_id").GetString()!,
            requestId!,
            runtimeId,
            pipelineKind!,
            requestedBy!,
            "accepted",
            DateTimeOffset.FromUnixTimeMilliseconds(acceptedUnixMs),
            target,
            outcome.GetProperty("replayed").GetBoolean());
    }

    private static JsonElement RequireResponse(JsonElement root, string expectedKind)
    {
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new DaemonDeploymentException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unsupported protocol version");
        }
        var response = root.GetProperty("response");
        var kind = response.GetProperty("kind").GetString();
        if (string.Equals(kind, "error", StringComparison.Ordinal))
        {
            var error = response.GetProperty("payload");
            throw new DaemonDeploymentException(
                error.GetProperty("code").GetString() ?? "daemon_request_failed",
                error.GetProperty("message").GetString() ?? "leserpentd rejected the request");
        }
        if (!string.Equals(kind, expectedKind, StringComparison.Ordinal))
        {
            throw new DaemonDeploymentException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unexpected response kind");
        }
        return response.GetProperty("payload");
    }
}

public sealed class DaemonDeploymentException : Exception
{
    public DaemonDeploymentException(string code, string message, Exception? innerException = null)
        : base(message, innerException)
    {
        Code = code;
    }

    public string Code { get; }
}
