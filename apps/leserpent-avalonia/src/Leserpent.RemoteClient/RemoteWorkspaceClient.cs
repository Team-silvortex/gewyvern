using System.Text.Json;
using System.Text.Json.Serialization;

public sealed class RemoteWorkspaceClient : IDisposable
{
    public const int MaxHistoryEntries = 32;
    private readonly RemoteWireTransport transport;

    public RemoteWorkspaceClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteWorkspaceSnapshot> LoadAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken = default)
    {
        RemoteQueryValidation.RequireIdentifier(runtimeId, "runtime ID");
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        var inspect = SendAsync("runtime_inspect", runtimeId, principal, cancellationToken);
        var history = SendAsync("runtime_history", runtimeId, principal, cancellationToken);
        await Task.WhenAll(inspect, history).ConfigureAwait(false);
        return RemoteWorkspaceCodec.Compose(inspect.Result, history.Result, runtimeId);
    }

    public void Dispose() => transport.Dispose();

    private Task<byte[]> SendAsync(
        string queryKind,
        string runtimeId,
        string principal,
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
    IReadOnlyList<RemoteHistoryProjection> History);

public sealed record RemoteHistoryProjection(
    string CommandId,
    ulong Revision,
    string Status);

public static class RemoteWorkspaceCodec
{
    public static RemoteWorkspaceSnapshot Compose(
        ReadOnlySpan<byte> inspectPayload,
        ReadOnlySpan<byte> historyPayload,
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
        if (inspect.Runtime is null || history.Entries is null)
        {
            throw new InvalidDataException(
                "remote workspace query result is missing required data");
        }
        if (inspect.Revision != history.Revision)
        {
            throw new InvalidDataException(
                "remote workspace query revisions do not match");
        }
        ValidateRuntime(inspect.Runtime, expectedRuntimeId);
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
        return new RemoteWorkspaceSnapshot(
            inspect.Revision,
            Project(inspect.Runtime),
            projectedHistory);
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

    private static void ValidateRuntime(WireRuntimeProjection runtime, string expectedRuntimeId)
    {
        if (runtime.Id is null
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
        if (runtime.Id != expectedRuntimeId)
        {
            throw new InvalidDataException("remote workspace runtime identity is invalid");
        }
        RemoteQueryValidation.RequireDisplay(runtime.Name, "runtime name");
        RemoteQueryValidation.RequireDisplay(runtime.Endpoint, "runtime endpoint");
        RemoteQueryValidation.RequireDisplay(runtime.Status.StatusSource, "status source");
    }

    private static RemoteRuntimeProjection Project(WireRuntimeProjection runtime) => new()
    {
        Id = runtime.Id,
        Name = runtime.Name,
        Revision = runtime.Revision,
        RefreshCount = runtime.RefreshCount,
        RefreshStatus = runtime.RefreshStatus,
        Tags = runtime.Tags,
        Status = runtime.Status,
    };

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
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireQueryRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(RuntimeInspectQueryResult))]
[JsonSerializable(typeof(RuntimeHistoryQueryResult))]
public partial class RemoteWorkspaceJsonContext : JsonSerializerContext;
