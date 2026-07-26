using System.Text.Json;
using System.Text.Json.Serialization;

public sealed record RemoteHealth(
    string Status,
    bool AuthorityOwned,
    uint ProtocolSchemaVersion,
    RemoteEffectQueueHealth? EffectQueue,
    RemoteUnregistrationReplayHorizon? RuntimeUnregistrationReplayHorizon = null);

public sealed record RemoteEffectQueueHealth(
    ulong Ready,
    ulong Leased,
    ulong Completed,
    ulong Failed,
    ulong Active,
    ulong Terminal,
    ulong Capacity,
    bool Saturated);

public sealed record RemoteUnregistrationReplayHorizon(
    ulong Capacity,
    ulong Retained,
    ulong? OldestGeneration,
    ulong? NewestGeneration,
    ulong NextGeneration,
    ulong EvictedThroughGeneration);

public sealed class RemoteHealthClient : IDisposable
{
    private readonly RemoteWireTransport transport;

    public RemoteHealthClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteHealth> CheckAsync(
        CancellationToken cancellationToken = default)
    {
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            new WireHealthRequestEnvelope(),
            RemoteHealthJsonContext.Default.WireHealthRequestEnvelope);
        var response = await transport.PostAsync(payload, "health", cancellationToken)
            .ConfigureAwait(false);
        return RemoteHealthCodec.Decode(response);
    }

    public void Dispose() => transport.Dispose();
}

public static class RemoteHealthCodec
{
    public static RemoteHealth Decode(ReadOnlySpan<byte> payload)
    {
        if (payload.Length > RemoteEventCodec.MaxMessageBytes)
        {
            throw new InvalidDataException("remote health response exceeds the message limit");
        }
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteHealthJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException("remote health response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported remote health response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw new RemoteHealthException(
                    RequiredString(envelope.Response.Payload, "code"),
                    RequiredString(envelope.Response.Payload, "message"));
            }
            if (envelope.Response.Kind != "health")
            {
                throw new InvalidDataException(
                    "remote health returned an unexpected response kind");
            }
            var health = envelope.Response.Payload.Deserialize(
                RemoteHealthJsonContext.Default.WireHealthPayload)
                ?? throw new InvalidDataException("remote health payload is empty");
            if (health.Status != "ready"
                || !health.AuthorityOwned
                || health.ProtocolSchemaVersion != 1)
            {
                throw new InvalidDataException(
                    "remote health did not prove a ready protocol-v1 authority");
            }
            var queue = health.EffectQueue is null
                ? null
                : ValidateQueue(health.EffectQueue);
            var replayHorizon = health.RuntimeUnregistrationReplayHorizon is null
                ? null
                : ValidateReplayHorizon(health.RuntimeUnregistrationReplayHorizon);
            return new RemoteHealth(
                health.Status,
                health.AuthorityOwned,
                health.ProtocolSchemaVersion,
                queue,
                replayHorizon);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("remote health response JSON is invalid", error);
        }
        catch (KeyNotFoundException error)
        {
            throw new InvalidDataException(
                "remote health response is missing a required field",
                error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException(
                "remote health response has an invalid field type",
                error);
        }
    }

    private static RemoteEffectQueueHealth ValidateQueue(WireEffectQueueHealth queue)
    {
        ulong active;
        ulong terminal;
        try
        {
            active = checked(queue.Ready + queue.Leased);
            terminal = checked(queue.Completed + queue.Failed);
        }
        catch (OverflowException error)
        {
            throw new InvalidDataException("remote health queue counters overflow", error);
        }
        if (queue.Active != active
            || queue.Terminal != terminal
            || queue.Active > queue.Capacity
            || queue.Saturated != (queue.Active >= queue.Capacity))
        {
            throw new InvalidDataException("remote health queue counters are inconsistent");
        }
        return new RemoteEffectQueueHealth(
            queue.Ready,
            queue.Leased,
            queue.Completed,
            queue.Failed,
            queue.Active,
            queue.Terminal,
            queue.Capacity,
            queue.Saturated);
    }

    private static RemoteUnregistrationReplayHorizon ValidateReplayHorizon(
        WireUnregistrationReplayHorizon horizon)
    {
        var contiguous = horizon.OldestGeneration is { } oldest
            && horizon.NewestGeneration is { } newest
            ? horizon.Retained > 0
                && horizon.EvictedThroughGeneration < ulong.MaxValue
                && oldest == horizon.EvictedThroughGeneration + 1
                && newest < ulong.MaxValue
                && horizon.NextGeneration == newest + 1
                && newest >= oldest
                && horizon.Retained == newest - oldest + 1
            : !horizon.OldestGeneration.HasValue
                && !horizon.NewestGeneration.HasValue
                && horizon.Retained == 0
                && horizon.EvictedThroughGeneration < ulong.MaxValue
                && horizon.NextGeneration == horizon.EvictedThroughGeneration + 1;
        if (horizon.Capacity == 0
            || horizon.Retained > horizon.Capacity
            || horizon.NextGeneration == 0
            || horizon.EvictedThroughGeneration >= horizon.NextGeneration
            || !contiguous)
        {
            throw new InvalidDataException(
                "remote health unregistration replay horizon is inconsistent");
        }
        return new RemoteUnregistrationReplayHorizon(
            horizon.Capacity,
            horizon.Retained,
            horizon.OldestGeneration,
            horizon.NewestGeneration,
            horizon.NextGeneration,
            horizon.EvictedThroughGeneration);
    }

    private static string RequiredString(JsonElement value, string name)
    {
        var result = value.GetProperty(name).GetString();
        if (string.IsNullOrWhiteSpace(result) || result.Length > 4096)
        {
            throw new InvalidDataException($"remote health {name} is invalid");
        }
        return result;
    }
}

public sealed class RemoteHealthException(string code, string message)
    : IOException(message)
{
    public string Code { get; } = code;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireHealthRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public WireHealthRequest Request { get; set; } = new();
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireHealthRequest
{
    public string Kind { get; set; } = "health";
    public WireHealthRequestPayload Payload { get; set; } = new();
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireHealthRequestPayload;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireHealthPayload
{
    public required string Status { get; set; }
    public bool AuthorityOwned { get; set; }
    public uint ProtocolSchemaVersion { get; set; }
    public WireEffectQueueHealth? EffectQueue { get; set; }
    public WireUnregistrationReplayHorizon? RuntimeUnregistrationReplayHorizon { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireEffectQueueHealth
{
    public ulong Ready { get; set; }
    public ulong Leased { get; set; }
    public ulong Completed { get; set; }
    public ulong Failed { get; set; }
    public ulong Active { get; set; }
    public ulong Terminal { get; set; }
    public ulong Capacity { get; set; }
    public bool Saturated { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireUnregistrationReplayHorizon
{
    public ulong Capacity { get; set; }
    public ulong Retained { get; set; }
    public ulong? OldestGeneration { get; set; }
    public ulong? NewestGeneration { get; set; }
    public ulong NextGeneration { get; set; }
    public ulong EvictedThroughGeneration { get; set; }
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireHealthRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(WireHealthPayload))]
public partial class RemoteHealthJsonContext : JsonSerializerContext;
