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
        CancellationToken cancellationToken,
        bool update = false,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null);
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
        CancellationToken cancellationToken,
        bool update = false,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null)
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
            var expectedRevision = update
                ? await InspectRevisionAsync(runtimeId, deadline.Token)
                : null;
            var command = BuildCommand(request, runtimeId, expectedRevision);
            using var response = await ExchangeAsync(command, deadline.Token);
            _ = ParseCommandResult(response.RootElement, runtimeId);

            var capabilitySnapshot = capabilityDiscovery?.AuthoritySnapshot;
            var statusSnapshot = statusDiscovery?.Status.StatusSource == "gewyvern-api"
                && statusDiscovery.Status.StatusFetchError is null
                    ? statusDiscovery.Status
                    : null;
            if (capabilitySnapshot is not null || statusSnapshot is not null)
            {
                var intakeRevision = await InspectRevisionAsync(runtimeId, deadline.Token)
                    ?? throw new InvalidOperationException("leserpentd lost the registered runtime before discovery intake");
                var intake = BuildDiscoveryIntakeCommand(
                    runtimeId,
                    intakeRevision,
                    capabilitySnapshot,
                    statusSnapshot);
                using var intakeResponse = await ExchangeAsync(intake, deadline.Token);
                _ = ParseCommandResult(intakeResponse.RootElement, runtimeId);
            }
            return runtimeId;
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

    private byte[] BuildCommand(
        RuntimeRegistrationRequest request,
        string runtimeId,
        long? expectedRevision)
    {
        var tags = request.Tags ?? new RuntimeTags(null, null, null);
        var stableCommandId = BuildDeterministicCommandId(
            runtimeId,
            request.Name,
            request.Endpoint,
            expectedRevision is not null);
        return BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "command");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteString("command_id", stableCommandId);
            writer.WriteString("idempotency_key", stableCommandId);
            if (expectedRevision is null)
            {
                writer.WriteNull("expected_revision");
            }
            else
            {
                writer.WriteNumber("expected_revision", expectedRevision.Value);
            }
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer);
            writer.WriteString("origin", "compatibility_adapter");
            writer.WriteString("confirmation", "confirmed");
            writer.WriteBoolean("dry_run", false);
            writer.WritePropertyName("command");
            writer.WriteStartObject();
            writer.WriteString(
                "kind",
                expectedRevision is null ? "runtime_register" : "runtime_registration_update");
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
            writer.WriteEndObject();
        });
    }

    private async Task<long?> InspectRevisionAsync(
        string runtimeId,
        CancellationToken cancellationToken)
    {
        var query = BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "query");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer, "runtime.read");
            writer.WritePropertyName("query");
            writer.WriteStartObject();
            writer.WriteString("kind", "runtime_inspect");
            writer.WriteString("runtime_id", runtimeId);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
        using var response = await ExchangeAsync(query, cancellationToken);
        var root = response.RootElement;
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unsupported protocol version");
        }
        var protocolResponse = root.GetProperty("response");
        var kind = protocolResponse.GetProperty("kind").GetString();
        if (string.Equals(kind, "error", StringComparison.Ordinal))
        {
            var error = protocolResponse.GetProperty("payload");
            var code = error.GetProperty("code").GetString();
            if (string.Equals(code, "runtime_not_found", StringComparison.Ordinal))
            {
                return null;
            }
            throw new DaemonRuntimeRegistrationException(
                code ?? "daemon_request_failed",
                error.GetProperty("message").GetString() ?? "leserpentd rejected the registration query");
        }
        if (!string.Equals(kind, "query", StringComparison.Ordinal))
        {
            throw new InvalidOperationException("leserpentd returned an unexpected registration query response");
        }
        var payload = protocolResponse.GetProperty("payload");
        if (!string.Equals(payload.GetProperty("kind").GetString(), "runtime_inspect", StringComparison.Ordinal))
        {
            throw new InvalidOperationException("leserpentd returned an unexpected runtime query payload");
        }
        var runtime = payload.GetProperty("runtime");
        if (!string.Equals(runtime.GetProperty("id").GetString(), runtimeId, StringComparison.Ordinal))
        {
            throw new InvalidOperationException("leserpentd runtime query returned a different runtime");
        }
        return runtime.GetProperty("revision").GetInt64();
    }

    private byte[] BuildDiscoveryIntakeCommand(
        string runtimeId,
        long expectedRevision,
        RuntimeCapabilityAuthoritySnapshot? capabilities,
        RuntimeStatusSnapshot? status)
    {
        var commandId = BuildDiscoveryCommandId(runtimeId, expectedRevision, capabilities, status);
        return BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "command");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteString("command_id", commandId);
            writer.WriteString("idempotency_key", commandId);
            writer.WriteNumber("expected_revision", expectedRevision);
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer, "runtime.register");
            writer.WriteString("origin", "compatibility_adapter");
            writer.WriteString("confirmation", "confirmed");
            writer.WriteBoolean("dry_run", false);
            writer.WritePropertyName("command");
            writer.WriteStartObject();
            writer.WriteString("kind", "runtime_discovery_intake");
            writer.WriteString("runtime_id", runtimeId);
            WriteCapabilitySnapshot(writer, capabilities);
            WriteStatusSnapshot(writer, status);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
    }

    private static void WriteCapabilitySnapshot(
        Utf8JsonWriter writer,
        RuntimeCapabilityAuthoritySnapshot? snapshot)
    {
        if (snapshot is null)
        {
            writer.WriteNull("capabilities");
            return;
        }
        writer.WritePropertyName("capabilities");
        writer.WriteStartObject();
        writer.WriteString("source", snapshot.Source);
        writer.WriteString("service", snapshot.Service);
        writer.WriteString("version", snapshot.Version);
        writer.WriteBoolean("latest_snapshot", snapshot.LatestSnapshot);
        writer.WriteBoolean("authenticated_deployment", snapshot.AuthenticatedDeployment);
        writer.WriteBoolean("serve_required", snapshot.ServeRequired);
        writer.WriteBoolean("external_sidecar_context", snapshot.ExternalSidecarContext);
        writer.WriteString("target_path_segment_encoding", snapshot.TargetPathSegmentEncoding);
        writer.WriteString("target_direct_path_chars", snapshot.TargetDirectPathChars);
        writer.WritePropertyName("endpoints");
        writer.WriteStartArray();
        foreach (var endpoint in snapshot.Endpoints)
        {
            writer.WriteStringValue(endpoint);
        }
        writer.WriteEndArray();
        writer.WritePropertyName("extensions");
        writer.WriteStartObject();
        foreach (var extension in snapshot.Extensions.OrderBy(item => item.Key, StringComparer.Ordinal))
        {
            writer.WriteBoolean(extension.Key, extension.Value);
        }
        writer.WriteEndObject();
        writer.WriteEndObject();
    }

    private static void WriteStatusSnapshot(Utf8JsonWriter writer, RuntimeStatusSnapshot? status)
    {
        if (status is null)
        {
            writer.WriteNull("status");
            return;
        }
        writer.WritePropertyName("status");
        writer.WriteStartObject();
        writer.WriteString("status_source", status.StatusSource);
        WriteOptionalString(writer, "status_fetched_at", status.StatusFetchedAt?.ToString("O"));
        WriteOptionalString(writer, "status_fetch_error", status.StatusFetchError);
        writer.WriteBoolean("has_latest_snapshot", status.HasLatestSnapshot);
        WriteOptionalString(writer, "snapshot_kind", status.SnapshotKind);
        WriteOptionalNumber(writer, "target_count", status.TargetCount);
        writer.WriteBoolean("has_summary_json", status.HasSummaryJson);
        writer.WriteBoolean("has_analysis_json", status.HasAnalysisJson);
        writer.WriteBoolean("has_training_example_json", status.HasTrainingExampleJson);
        writer.WriteBoolean("has_training_dataset_manifest", status.HasTrainingDatasetManifest);
        writer.WriteBoolean("has_export_json", status.HasExportJson);
        writer.WriteBoolean("has_report_json", status.HasReportJson);
        writer.WriteBoolean("has_report_html", status.HasReportHtml);
        writer.WriteBoolean("has_external_sidecar_context", status.HasExternalSidecarContext);
        writer.WriteBoolean("has_external_evidence_chain_enrichment", status.HasExternalEvidenceChainEnrichment);
        writer.WriteBoolean("has_external_diagnostic_opinion", status.HasExternalDiagnosticOpinion);
        writer.WriteBoolean("resilience_degraded", status.ResilienceDegraded);
        WriteOptionalString(writer, "resilience_status", status.ResilienceStatus);
        WriteOptionalString(writer, "resilience_summary", status.ResilienceSummary);
        WriteOptionalString(writer, "socket_service_status", status.SocketServiceStatus);
        WriteOptionalNumber(writer, "socket_consecutive_idle_timeouts", status.SocketConsecutiveIdleTimeouts);
        WriteOptionalNumber(writer, "socket_total_idle_timeouts", status.SocketTotalIdleTimeouts);
        writer.WriteEndObject();
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

    private static long ParseCommandResult(JsonElement root, string expectedRuntimeId)
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
        var runtime = payload.GetProperty("runtime");
        return runtime.TryGetProperty("revision", out var runtimeRevision)
            ? runtimeRevision.GetInt64()
            : payload.GetProperty("revision").GetInt64();
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

    private static void WriteCapabilities(Utf8JsonWriter writer, string capability = "runtime.register")
    {
        writer.WritePropertyName("capabilities");
        writer.WriteStartArray();
        writer.WriteStringValue(capability);
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

    private static void WriteOptionalNumber(Utf8JsonWriter writer, string name, int? value)
    {
        if (value is null)
        {
            writer.WriteNull(name);
        }
        else
        {
            writer.WriteNumber(name, value.Value);
        }
    }

    private static string BuildDeterministicCommandId(
        string runtimeId,
        string name,
        string endpoint,
        bool update)
    {
        var normalizedName = name.Trim();
        var normalizedEndpoint = endpoint.Trim();
        var prefix = update ? "update|" : string.Empty;
        var bytes = HashData($"{prefix}{runtimeId}|{normalizedName}|{normalizedEndpoint}");
        return Convert.ToHexString(bytes).ToLowerInvariant().Substring(0, 32);
    }

    private static string BuildDiscoveryCommandId(
        string runtimeId,
        long expectedRevision,
        RuntimeCapabilityAuthoritySnapshot? capabilities,
        RuntimeStatusSnapshot? status)
    {
        var endpoints = capabilities is null
            ? string.Empty
            : string.Join(',', capabilities.Endpoints);
        var extensions = capabilities is null
            ? string.Empty
            : string.Join(',', capabilities.Extensions.OrderBy(item => item.Key, StringComparer.Ordinal)
                .Select(item => $"{item.Key}={item.Value}"));
        var value = string.Join('|',
            "discovery",
            runtimeId,
            expectedRevision,
            capabilities?.Version ?? string.Empty,
            endpoints,
            extensions,
            status?.StatusFetchedAt?.ToString("O") ?? string.Empty,
            status?.SnapshotKind ?? string.Empty,
            status?.TargetCount?.ToString() ?? string.Empty);
        return Convert.ToHexString(HashData(value)).ToLowerInvariant()[..32];
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
