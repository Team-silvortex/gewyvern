using System.Net.Sockets;
using System.Text;
using System.Text.Json;

namespace Leserpent.ControlPlane;

public sealed class DaemonOrchestraRunStore : IOrchestraRunStore
{
    private const int MaxFrameBytes = 1024 * 1024 + 1024;
    private const int PageSize = 64;
    private readonly string? socketPath;
    private readonly string? token;
    private readonly TimeSpan timeout;
    private readonly ILogger<DaemonOrchestraRunStore> logger;

    public DaemonOrchestraRunStore(
        IConfiguration configuration,
        ILogger<DaemonOrchestraRunStore> logger)
    {
        this.logger = logger;
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
                throw new InvalidOperationException("LESERPENT_DAEMON_SOCKET must not exceed 100 UTF-8 bytes");
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
        var configuredTimeout = configuration.GetValue<int?>("LESERPENT_DAEMON_ORCHESTRA_TIMEOUT_MS") ?? 5000;
        timeout = TimeSpan.FromMilliseconds(Math.Clamp(configuredTimeout, 100, 30_000));
    }

    public bool Enabled => socketPath is not null;
    public string Provider => Enabled ? "leserpentd" : "disabled";
    public string Location => socketPath ?? "unconfigured";
    public int SchemaVersion => Enabled ? 16 : 0;
    public string? LastError { get; private set; }

    public IReadOnlyList<OrchestraRunSummary> LoadAll() =>
        Execute("load Orchestra runs", () => LoadPages<OrchestraRunSummary>(null, null, "runs"))
        ?? Array.Empty<OrchestraRunSummary>();

    public IReadOnlyList<OrchestraRunEvent> LoadEvents(
        string runtimeId,
        string runId) =>
        Execute(
            $"load Orchestra events for run {runId}",
            () =>
            {
                var events = LoadPages<OrchestraRunEvent>(
                    runtimeId,
                    runId,
                    "events");
                ControlPlaneStateValidator
                    .ValidateOrchestraEventSequence(
                        null,
                        events,
                        runtimeId,
                        runId);
                return events;
            })
        ?? Array.Empty<OrchestraRunEvent>();

    public bool Upsert(OrchestraRunSummary run, OrchestraRunEvent? eventRecord = null)
    {
        try
        {
            ControlPlaneStateValidator.ValidateOrchestraStoreEnvelope(
                run,
                eventRecord);
        }
        catch (InvalidDataException error)
        {
            LastError = "orchestra_store_operation_failed";
            logger.LogError(
                error,
                "Rejected an invalid Orchestra persistence envelope");
            return false;
        }

        if (eventRecord is null)
        {
            var existing = LoadAll().FirstOrDefault(item =>
                string.Equals(item.RunId, run.RunId, StringComparison.Ordinal));
            return existing is not null && RunsEqual(existing, run);
        }
        return Execute("persist Orchestra run", () =>
        {
            using var response = Exchange(BuildFrame("orchestra_persist", writer =>
            {
                WriteAuthority(writer);
                writer.WritePropertyName("envelope");
                writer.WriteStartObject();
                writer.WritePropertyName("run");
                JsonSerializer.Serialize(writer, run, global::Leserpent.LeserpentJsonContext.Default.OrchestraRunSummary);
                writer.WritePropertyName("event");
                JsonSerializer.Serialize(writer, eventRecord, global::Leserpent.LeserpentJsonContext.Default.OrchestraRunEvent);
                writer.WriteEndObject();
            }));
            var payload = RequireResponse(response.RootElement, "orchestra_persisted");
            var returnedRun = payload.GetProperty("envelope").GetProperty("run")
                .Deserialize(global::Leserpent.LeserpentJsonContext.Default.OrchestraRunSummary);
            var returnedEvent = payload.GetProperty("envelope").GetProperty("event")
                .Deserialize(global::Leserpent.LeserpentJsonContext.Default.OrchestraRunEvent);
            if (returnedRun is null || returnedEvent != eventRecord || !RunsEqual(returnedRun, run))
            {
                throw new InvalidDataException("leserpentd returned mismatched Orchestra persistence data");
            }
            return true;
        });
    }

    public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs)
    {
        try
        {
            foreach (var run in runs)
            {
                ControlPlaneStateValidator
                    .ValidateOrchestraStoreEnvelope(run, null);
            }
        }
        catch (InvalidDataException error)
        {
            LastError = "orchestra_store_operation_failed";
            logger.LogError(
                error,
                "Rejected invalid legacy Orchestra history");
            return false;
        }

        foreach (var run in runs.OrderBy(item => item.ExecutedAt))
        {
            var imported = ControlPlaneStateValidator
                .CreateLegacyOrchestraImportEvent(run);
            if (!Upsert(run, imported))
            {
                return false;
            }
        }
        return true;
    }

    public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds)
    {
        if (runtimeIds.Count == 0)
        {
            return true;
        }
        return Execute("delete Orchestra history", () =>
        {
            using var response = Exchange(BuildFrame("orchestra_delete", writer =>
            {
                WriteAuthority(writer);
                writer.WritePropertyName("runtime_ids");
                writer.WriteStartArray();
                foreach (var runtimeId in runtimeIds)
                {
                    writer.WriteStringValue(runtimeId);
                }
                writer.WriteEndArray();
            }));
            _ = RequireResponse(response.RootElement, "orchestra_deleted");
            return true;
        });
    }

    public OrchestraDeleteReceipt? DeleteRuntimes(
        OrchestraDeleteCommand command)
    {
        var expectedRuntimeIds = command.RuntimeIds
            .Order(StringComparer.Ordinal)
            .ToArray();
        if (string.IsNullOrWhiteSpace(command.CommandId) ||
            expectedRuntimeIds.Length is < 1 or > 128 ||
            expectedRuntimeIds.Distinct(StringComparer.Ordinal).Count() !=
                expectedRuntimeIds.Length)
        {
            LastError = "orchestra_store_operation_failed";
            return null;
        }
        return Execute("idempotently delete Orchestra history", () =>
        {
            using var response = Exchange(BuildFrame(
                "orchestra_delete_command",
                writer =>
                {
                    WriteAuthority(writer);
                    writer.WriteString("command_id", command.CommandId);
                    writer.WritePropertyName("runtime_ids");
                    writer.WriteStartArray();
                    foreach (var runtimeId in expectedRuntimeIds)
                    {
                        writer.WriteStringValue(runtimeId);
                    }
                    writer.WriteEndArray();
                }));
            var payload = RequireResponse(
                response.RootElement,
                "orchestra_delete_receipt");
            var returnedCommandId =
                payload.GetProperty("command_id").GetString();
            var generation = payload
                .GetProperty("operation_generation")
                .GetUInt64();
            var returnedRuntimeIds = payload
                .GetProperty("runtime_ids")
                .EnumerateArray()
                .Select(element =>
                    element.GetString() ??
                    throw new InvalidDataException(
                        "leserpentd returned a blank Orchestra delete target"))
                .ToArray();
            var deletedRuntimeCount = payload
                .GetProperty("deleted_runtime_count")
                .GetUInt32();
            var deletedRunCount = payload
                .GetProperty("deleted_run_count")
                .GetUInt64();
            var deletedEventCount = payload
                .GetProperty("deleted_event_count")
                .GetUInt64();
            var committedAtUnixMs = payload
                .GetProperty("committed_at_unix_ms")
                .GetInt64();
            var replayed = payload.GetProperty("replayed").GetBoolean();
            var maximumRunCount = checked(
                (ulong)expectedRuntimeIds.Length * 32);
            var maximumEventCount = checked(maximumRunCount * 3);
            if (!string.Equals(
                    returnedCommandId,
                    command.CommandId,
                    StringComparison.Ordinal) ||
                generation == 0 ||
                !returnedRuntimeIds.SequenceEqual(
                    expectedRuntimeIds,
                    StringComparer.Ordinal) ||
                deletedRuntimeCount >
                    checked((uint)expectedRuntimeIds.Length) ||
                deletedRunCount > maximumRunCount ||
                deletedEventCount > maximumEventCount ||
                committedAtUnixMs < 0)
            {
                throw new InvalidDataException(
                    "leserpentd returned a mismatched Orchestra delete receipt");
            }
            return new OrchestraDeleteReceipt(
                command.CommandId,
                generation,
                returnedRuntimeIds,
                deletedRuntimeCount,
                deletedRunCount,
                deletedEventCount,
                DateTimeOffset.FromUnixTimeMilliseconds(
                    committedAtUnixMs),
                replayed);
        });
    }

    private IReadOnlyList<T> LoadPages<T>(string? runtimeId, string? runId, string property)
    {
        var records = new List<T>();
        uint offset = 0;
        while (true)
        {
            using var response = Exchange(BuildFrame("orchestra_history", writer =>
            {
                WriteAuthority(writer);
                WriteOptionalString(writer, "runtime_id", runtimeId);
                WriteOptionalString(writer, "run_id", runId);
                writer.WriteNumber("offset", offset);
                writer.WriteNumber("limit", PageSize);
            }));
            var payload = RequireResponse(response.RootElement, "orchestra_history");
            foreach (var element in payload.GetProperty(property).EnumerateArray())
            {
                records.Add(DeserializeRecord<T>(element));
            }
            if (payload.GetProperty("next_offset").ValueKind == JsonValueKind.Null)
            {
                return records;
            }
            var next = payload.GetProperty("next_offset").GetUInt32();
            if (next <= offset)
            {
                throw new InvalidDataException("leserpentd returned a non-advancing history cursor");
            }
            offset = next;
        }
    }

    private static T DeserializeRecord<T>(JsonElement element)
    {
        if (typeof(T) == typeof(OrchestraRunSummary))
        {
            var run = element.Deserialize(
                global::Leserpent.LeserpentJsonContext.Default
                    .OrchestraRunSummary)
                ?? throw new InvalidDataException(
                    "leserpentd returned an empty Orchestra run");
            ControlPlaneStateValidator.ValidateOrchestraStoreEnvelope(
                run,
                null);
            return (T)(object)run;
        }
        if (typeof(T) == typeof(OrchestraRunEvent))
        {
            var eventRecord = element.Deserialize(
                global::Leserpent.LeserpentJsonContext.Default
                    .OrchestraRunEvent)
                ?? throw new InvalidDataException(
                    "leserpentd returned an empty Orchestra event");
            ControlPlaneStateValidator.ValidateOrchestraEventPayload(
                eventRecord);
            return (T)(object)eventRecord;
        }
        throw new InvalidOperationException(
            "unsupported Orchestra history record type");
    }

    private static bool RunsEqual(OrchestraRunSummary left, OrchestraRunSummary right) =>
        JsonSerializer.SerializeToUtf8Bytes(
                left,
                global::Leserpent.LeserpentJsonContext.Default.OrchestraRunSummary)
            .AsSpan()
            .SequenceEqual(JsonSerializer.SerializeToUtf8Bytes(
                right,
                global::Leserpent.LeserpentJsonContext.Default.OrchestraRunSummary));

    private T? Execute<T>(string operation, Func<T> action)
    {
        if (!Enabled)
        {
            return default;
        }
        try
        {
            var result = action();
            LastError = null;
            return result;
        }
        catch (Exception error) when (
            error is IOException or SocketException or JsonException or InvalidDataException
                or OperationCanceledException or UnauthorizedAccessException)
        {
            LastError = "orchestra_store_operation_failed";
            logger.LogError(error, "Failed to {Operation} through leserpentd", operation);
            return default;
        }
    }

    private JsonDocument Exchange(byte[] request)
    {
        ValidateSocketBoundary();
        using var deadline = new CancellationTokenSource(timeout);
        using var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        socket.ConnectAsync(new UnixDomainSocketEndPoint(socketPath!), deadline.Token).AsTask().GetAwaiter().GetResult();
        using var stream = new NetworkStream(socket, ownsSocket: false);
        stream.WriteAsync(request, deadline.Token).AsTask().GetAwaiter().GetResult();
        stream.FlushAsync(deadline.Token).GetAwaiter().GetResult();
        socket.Shutdown(SocketShutdown.Send);
        return JsonDocument.Parse(ReadFrame(socket, deadline.Token));
    }

    private byte[] BuildFrame(string kind, Action<Utf8JsonWriter> writePayload)
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
            writer.WriteString("kind", kind);
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writePayload(writer);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        }
        if (output.Length + 1 > MaxFrameBytes)
        {
            throw new InvalidDataException("leserpentd Orchestra request exceeds the protocol limit");
        }
        output.WriteByte((byte)'\n');
        return output.ToArray();
    }

    private static JsonElement RequireResponse(JsonElement root, string expectedKind)
    {
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new InvalidDataException("leserpentd returned an unsupported protocol version");
        }
        var response = root.GetProperty("response");
        var kind = response.GetProperty("kind").GetString();
        if (string.Equals(kind, "error", StringComparison.Ordinal))
        {
            throw new InvalidDataException("leserpentd rejected the Orchestra operation");
        }
        if (!string.Equals(kind, expectedKind, StringComparison.Ordinal))
        {
            throw new InvalidDataException("leserpentd returned an unexpected Orchestra response");
        }
        return response.GetProperty("payload");
    }

    private void WriteAuthority(Utf8JsonWriter writer)
    {
        writer.WritePropertyName("principal");
        writer.WriteStartObject();
        writer.WriteString("id", "compatibility_adapter");
        writer.WriteEndObject();
        writer.WritePropertyName("capabilities");
        writer.WriteStartArray();
        writer.WriteStringValue("orchestra.write");
        writer.WriteEndArray();
    }

    private void ValidateSocketBoundary()
    {
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("leserpentd Unix socket Orchestra storage is unavailable on Windows");
        }
        var attributes = File.GetAttributes(socketPath!);
        if ((attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException("leserpentd socket must not be a symbolic link");
        }
        const UnixFileMode unsafePermissions = UnixFileMode.GroupRead | UnixFileMode.GroupWrite
            | UnixFileMode.GroupExecute | UnixFileMode.OtherRead | UnixFileMode.OtherWrite
            | UnixFileMode.OtherExecute;
        if ((File.GetUnixFileMode(socketPath!) & unsafePermissions) != 0)
        {
            throw new InvalidDataException("leserpentd socket must be owner-private");
        }
    }

    private static byte[] ReadFrame(Socket socket, CancellationToken cancellationToken)
    {
        using var output = new MemoryStream();
        var buffer = new byte[4096];
        while (true)
        {
            var read = socket.ReceiveAsync(buffer, SocketFlags.None, cancellationToken).AsTask().GetAwaiter().GetResult();
            if (read == 0)
            {
                throw new IOException("leserpentd closed before returning a complete frame");
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
                    throw new IOException("leserpentd returned trailing frame data");
                }
                return output.ToArray();
            }
        }
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
}
