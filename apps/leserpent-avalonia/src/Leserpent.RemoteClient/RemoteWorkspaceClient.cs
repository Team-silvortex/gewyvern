using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text;

public sealed class RemoteWorkspaceClient : IDisposable
{
    public const int MaxHistoryEntries = 32;
    public const int MaxLogEntries = 256;
    public const int MaxLogMessageBytes = 64 * 1024;
    public const int MaxLogDisplayBytes = 768;
    private readonly RemoteWireTransport transport;

    public RemoteWorkspaceClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteWorkspaceSnapshot> LoadAsync(
        string runtimeId,
        string principal,
        ulong? afterLogSequence = null,
        CancellationToken cancellationToken = default)
    {
        RemoteQueryValidation.RequireIdentifier(runtimeId, "runtime ID");
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        var inspect = SendAsync(
            "runtime_inspect", runtimeId, principal, null, cancellationToken);
        var history = SendAsync(
            "runtime_history", runtimeId, principal, null, cancellationToken);
        var logs = SendAsync(
            "runtime_logs", runtimeId, principal, afterLogSequence, cancellationToken);
        await Task.WhenAll(inspect, history, logs).ConfigureAwait(false);
        return RemoteWorkspaceCodec.Compose(
            inspect.Result,
            history.Result,
            logs.Result,
            runtimeId);
    }

    public void Dispose() => transport.Dispose();

    private Task<byte[]> SendAsync(
        string queryKind,
        string runtimeId,
        string principal,
        ulong? afterLogSequence,
        CancellationToken cancellationToken)
    {
        var envelope = new WireQueryRequestEnvelope
        {
            Request = new WireQueryRequest
            {
                Payload = new RuntimeQueryEnvelope
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["runtime.read"],
                    Query = new RuntimeQuery
                    {
                        Kind = queryKind,
                        RuntimeId = runtimeId,
                        Limit = queryKind == "runtime_logs" ? MaxLogEntries : 0,
                        AfterSequence = queryKind == "runtime_logs"
                            ? afterLogSequence
                            : null,
                    },
                },
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteWorkspaceJsonContext.Default.WireQueryRequestEnvelope);
        return transport.PostAsync(payload, queryKind, cancellationToken);
    }
}

public sealed record RemoteWorkspaceSnapshot(
    ulong Revision,
    RemoteRuntimeProjection Runtime,
    IReadOnlyList<RemoteHistoryProjection> History,
    IReadOnlyList<RemoteLogProjection> Logs);

public sealed record RemoteHistoryProjection(
    string CommandId,
    ulong Revision,
    string Status);

public sealed record RemoteLogProjection(
    ulong Sequence,
    string Level,
    string Display);

public static class RemoteWorkspaceCodec
{
    public static RemoteWorkspaceSnapshot MergeIncrementalLogs(
        RemoteWorkspaceSnapshot previous,
        RemoteWorkspaceSnapshot incremental)
    {
        if (!string.Equals(
                previous.Runtime.Id,
                incremental.Runtime.Id,
                StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "incremental workspace runtime identity changed");
        }
        if (incremental.Revision < previous.Revision)
        {
            throw new InvalidDataException(
                "incremental workspace revision regressed");
        }
        if (previous.Logs.Count == 0 || incremental.Logs.Count == 0)
        {
            return incremental with
            {
                Logs = incremental.Logs.Count == 0 ? previous.Logs : incremental.Logs,
            };
        }
        var cursor = previous.Logs[^1].Sequence;
        if (incremental.Logs[0].Sequence <= cursor)
        {
            throw new InvalidDataException(
                "incremental workspace logs did not advance their cursor");
        }
        var merged = previous.Logs
            .Concat(incremental.Logs)
            .TakeLast(RemoteWorkspaceClient.MaxLogEntries)
            .ToArray();
        return incremental with { Logs = merged };
    }

    public static void VerifyIncrementalContract()
    {
        var request = new WireQueryRequestEnvelope
        {
            Request = new WireQueryRequest
            {
                Payload = new RuntimeQueryEnvelope
                {
                    Principal = new RemotePrincipal { Id = "operator" },
                    Capabilities = ["runtime.read"],
                    Query = new RuntimeQuery
                    {
                        Kind = "runtime_logs",
                        RuntimeId = "runtime-a",
                        Limit = RemoteWorkspaceClient.MaxLogEntries,
                        AfterSequence = 42,
                    },
                },
            },
        };
        var encoded = JsonSerializer.Serialize(
            request,
            RemoteWorkspaceJsonContext.Default.WireQueryRequestEnvelope);
        if (!encoded.Contains("\"after_sequence\":42", StringComparison.Ordinal))
        {
            throw new InvalidDataException("incremental workspace cursor encoding drifted");
        }
        var prior = Snapshot(7,
            [new RemoteLogProjection(2, "info", "two")]);
        var incremental = Snapshot(7,
            [
                new RemoteLogProjection(3, "warning", "three"),
                new RemoteLogProjection(4, "error", "four"),
            ]);
        var merged = MergeIncrementalLogs(prior, incremental);
        if (!merged.Logs.Select(entry => entry.Sequence).SequenceEqual([2UL, 3UL, 4UL]))
        {
            throw new InvalidDataException("incremental workspace log merge drifted");
        }
        var empty = MergeIncrementalLogs(prior, Snapshot(7, []));
        if (!ReferenceEquals(empty.Logs, prior.Logs))
        {
            throw new InvalidDataException("empty incremental workspace lost retained logs");
        }
        try
        {
            _ = MergeIncrementalLogs(prior, Snapshot(7,
                [new RemoteLogProjection(2, "info", "duplicate")]));
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException("incremental workspace accepted a stale cursor");
    }

    public static RemoteWorkspaceSnapshot Compose(
        ReadOnlySpan<byte> inspectPayload,
        ReadOnlySpan<byte> historyPayload,
        ReadOnlySpan<byte> logsPayload,
        string expectedRuntimeId)
    {
        RemoteQueryValidation.RequireIdentifier(expectedRuntimeId, "runtime ID");
        var inspect = Decode<RuntimeInspectQueryResult>(
            inspectPayload,
            "runtime_inspect",
            RemoteWorkspaceJsonContext.Default.RuntimeInspectQueryResult);
        var history = Decode<RuntimeHistoryQueryResult>(
            historyPayload,
            "runtime_history",
            RemoteWorkspaceJsonContext.Default.RuntimeHistoryQueryResult);
        var logs = Decode<RuntimeLogsQueryResult>(
            logsPayload,
            "runtime_logs",
            RemoteWorkspaceJsonContext.Default.RuntimeLogsQueryResult);
        if (inspect.Runtime is null || history.Entries is null || logs.Entries is null)
        {
            throw new InvalidDataException(
                "remote workspace query result is missing required data");
        }
        if (inspect.Revision != history.Revision || inspect.Revision != logs.Revision)
        {
            throw new InvalidDataException(
                "remote workspace query revisions do not match");
        }
        ValidateRuntime(inspect.Runtime, expectedRuntimeId);
        if (logs.RuntimeId is null || logs.RuntimeName is null)
        {
            throw new InvalidDataException(
                "remote workspace log identity is missing required data");
        }
        RemoteQueryValidation.RequireIdentifier(logs.RuntimeId, "runtime ID");
        RemoteQueryValidation.RequireDisplay(logs.RuntimeName, "runtime name");
        if (logs.RuntimeId != expectedRuntimeId || logs.RuntimeName != inspect.Runtime.Name)
        {
            throw new InvalidDataException("remote workspace log identity is invalid");
        }
        if (inspect.Runtime.Revision > inspect.Revision)
        {
            throw new InvalidDataException(
                "remote workspace runtime is newer than its snapshot");
        }
        if (history.Entries.Count > RemoteWorkspaceClient.MaxHistoryEntries)
        {
            throw new InvalidDataException("remote workspace history exceeds its item limit");
        }
        var projectedHistory = new List<RemoteHistoryProjection>(history.Entries.Count);
        foreach (var entry in history.Entries)
        {
            if (entry is null
                || entry.CommandId is null
                || entry.Status is null
                || entry.Runtime is null
                || entry.Events is null)
            {
                throw new InvalidDataException(
                    "remote workspace history entry is missing required data");
            }
            RemoteQueryValidation.RequireIdentifier(entry.CommandId, "command ID");
            if (entry.Status is not ("planned" or "applied"))
            {
                throw new InvalidDataException("remote workspace history status is invalid");
            }
            ValidateRuntime(entry.Runtime, expectedRuntimeId);
            if (entry.Runtime.Revision > inspect.Revision)
            {
                throw new InvalidDataException(
                    "remote workspace history is newer than its snapshot");
            }
            projectedHistory.Add(new RemoteHistoryProjection(
                entry.CommandId,
                entry.Runtime.Revision,
                entry.Status));
        }
        if (logs.Entries.Count > RemoteWorkspaceClient.MaxLogEntries)
        {
            throw new InvalidDataException("remote workspace logs exceed their item limit");
        }
        var projectedLogs = new List<RemoteLogProjection>(logs.Entries.Count);
        ulong? previousSequence = null;
        foreach (var entry in logs.Entries)
        {
            if (entry is null || entry.Level is null || entry.Message is null)
            {
                throw new InvalidDataException(
                    "remote workspace log entry is missing required data");
            }
            if (previousSequence is { } previous && entry.Sequence <= previous)
            {
                throw new InvalidDataException(
                    "remote workspace log sequence is not strictly increasing");
            }
            if (entry.Level is not ("trace" or "debug" or "info" or "warning" or "error"))
            {
                throw new InvalidDataException("remote workspace log level is invalid");
            }
            if (Encoding.UTF8.GetByteCount(entry.Message)
                > RemoteWorkspaceClient.MaxLogMessageBytes)
            {
                throw new InvalidDataException("remote workspace log message is oversized");
            }
            previousSequence = entry.Sequence;
            projectedLogs.Add(new RemoteLogProjection(
                entry.Sequence,
                entry.Level,
                SanitizeLogDisplay(entry.Message)));
        }
        return new RemoteWorkspaceSnapshot(
            inspect.Revision,
            Project(inspect.Runtime),
            projectedHistory,
            projectedLogs);
    }

    private static T Decode<T>(
        ReadOnlySpan<byte> payload,
        string expectedKind,
        System.Text.Json.Serialization.Metadata.JsonTypeInfo<T> resultType)
    {
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteWorkspaceJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException("remote workspace response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException(
                    "unsupported remote workspace response schema");
            }
            if (envelope.Response is null || envelope.Response.Kind is null)
            {
                throw new InvalidDataException(
                    "remote workspace response is missing required data");
            }
            if (envelope.Response.Kind == "error")
            {
                var code = RequiredString(envelope.Response.Payload, "code");
                var message = RequiredString(envelope.Response.Payload, "message");
                throw new RemoteQueryException(code, message);
            }
            if (envelope.Response.Kind != "query")
            {
                throw new InvalidDataException(
                    "remote workspace returned an unexpected response kind");
            }
            var result = JsonSerializer.Deserialize(envelope.Response.Payload, resultType)
                ?? throw new InvalidDataException("remote workspace query result is empty");
            var kind = RequiredString(envelope.Response.Payload, "kind");
            if (kind != expectedKind)
            {
                throw new InvalidDataException(
                    "remote workspace returned an unexpected query kind");
            }
            return result;
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("remote workspace response JSON is invalid", error);
        }
        catch (KeyNotFoundException error)
        {
            throw new InvalidDataException(
                "remote workspace response is missing a required field",
                error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException(
                "remote workspace response has an invalid field type",
                error);
        }
    }

    internal static void ValidateRuntime(
        WireRuntimeProjection runtime,
        string? expectedRuntimeId = null)
    {
        if (runtime is null
            || runtime.Id is null
            || runtime.Name is null
            || runtime.Endpoint is null
            || runtime.Tags is null
            || runtime.Status is null
            || runtime.Status.StatusSource is null)
        {
            throw new InvalidDataException(
                "remote workspace runtime is missing required data");
        }
        RemoteQueryValidation.RequireIdentifier(runtime.Id, "runtime ID");
        if (expectedRuntimeId is not null && runtime.Id != expectedRuntimeId)
        {
            throw new InvalidDataException("remote workspace runtime identity is invalid");
        }
        RemoteQueryValidation.RequireDisplay(runtime.Name, "runtime name");
        RemoteQueryValidation.RequireDisplay(runtime.Endpoint, "runtime endpoint");
        RemoteQueryValidation.RequireDisplay(runtime.Status.StatusSource, "status source");
        RemoteEventCodec.ValidateCapabilities(
            runtime.Capabilities,
            runtime.CapabilitiesObservedForRevision,
            runtime.Revision);
    }

    internal static RemoteRuntimeProjection Project(WireRuntimeProjection runtime) => new()
    {
        Id = runtime.Id,
        Name = runtime.Name,
        Revision = runtime.Revision,
        RefreshCount = runtime.RefreshCount,
        RefreshStatus = runtime.RefreshStatus,
        Tags = runtime.Tags,
        Status = runtime.Status,
        Capabilities = runtime.Capabilities,
        CapabilitiesObservedForRevision = runtime.CapabilitiesObservedForRevision,
    };

    private static string SanitizeLogDisplay(string message)
    {
        var display = new StringBuilder(Math.Min(message.Length, 256));
        var bytes = 0;
        foreach (var rune in message.EnumerateRunes())
        {
            var safe = Rune.IsControl(rune) ? new Rune(' ') : rune;
            if (bytes + safe.Utf8SequenceLength > RemoteWorkspaceClient.MaxLogDisplayBytes)
            {
                break;
            }
            display.Append(safe.ToString());
            bytes += safe.Utf8SequenceLength;
        }
        return display.ToString();
    }

    private static RemoteWorkspaceSnapshot Snapshot(
        ulong revision,
        IReadOnlyList<RemoteLogProjection> logs) => new(
            revision,
            new RemoteRuntimeProjection
            {
                Id = "runtime-a",
                Name = "Runtime A",
                Revision = revision,
                Tags = new RuntimeTags(),
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            },
            [],
            logs);

    private static string RequiredString(JsonElement element, string property) =>
        element.GetProperty(property).GetString()
        ?? throw new InvalidDataException(
            $"remote workspace response field '{property}' is invalid");
}

public sealed class RemoteQueryException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}

internal static class RemoteQueryValidation
{
    public static void RequireIdentifier(string value, string label)
    {
        if (value.Length is < 1 or > 128
            || !value.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '-' or '_' or '.' or ':'))
        {
            throw new ArgumentException($"invalid {label}");
        }
    }

    public static void RequireDisplay(string value, string label)
    {
        if (value.Length is < 1 or > 4096 || value.Any(char.IsControl))
        {
            throw new InvalidDataException($"remote workspace {label} is invalid");
        }
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireQueryRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireQueryRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireQueryRequest
{
    public string Kind { get; set; } = "query";
    public required RuntimeQueryEnvelope Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeQueryEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required RuntimeQuery Query { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeQuery
{
    public required string Kind { get; set; }
    public required string RuntimeId { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingDefault)]
    public int Limit { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? AfterSequence { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeInspectQueryResult
{
    public required string Kind { get; set; }
    public ulong Revision { get; set; }
    public required WireRuntimeProjection Runtime { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeHistoryQueryResult
{
    public required string Kind { get; set; }
    public ulong Revision { get; set; }
    public required List<WireCommandResult> Entries { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeLogsQueryResult
{
    public required string Kind { get; set; }
    public ulong Revision { get; set; }
    public required string RuntimeId { get; set; }
    public required string RuntimeName { get; set; }
    public required List<WireRuntimeLogRecord> Entries { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRuntimeLogRecord
{
    public ulong Sequence { get; set; }
    public required string Level { get; set; }
    public required string Message { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireCommandResult
{
    public required string CommandId { get; set; }
    public required string Status { get; set; }
    public required WireRuntimeProjection Runtime { get; set; }
    public required List<JsonElement> Events { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRuntimeProjection
{
    public required string Id { get; set; }
    public required string Name { get; set; }
    public required string Endpoint { get; set; }
    public ulong Revision { get; set; }
    public ulong RefreshCount { get; set; }
    public RefreshStatus RefreshStatus { get; set; }
    public required RuntimeTags Tags { get; set; }
    public required RuntimeStatusSnapshot Status { get; set; }
    public RuntimeCapabilitySnapshot? Capabilities { get; set; }
    public ulong? CapabilitiesObservedForRevision { get; set; }
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireQueryRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(RuntimeInspectQueryResult))]
[JsonSerializable(typeof(RuntimeHistoryQueryResult))]
[JsonSerializable(typeof(RuntimeLogsQueryResult))]
public partial class RemoteWorkspaceJsonContext : JsonSerializerContext;
