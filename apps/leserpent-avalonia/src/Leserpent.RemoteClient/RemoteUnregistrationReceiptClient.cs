using System.Text.Json;
using System.Text.Json.Serialization;

public sealed record RemoteUnregistrationTarget(string RuntimeId, ulong ExpectedRevision);

public sealed record RemoteUnregistrationReceipt(
    ulong OperationGeneration,
    IReadOnlyList<RemoteUnregistrationTarget> Removed,
    uint DeletedOrchestraRuntimeCount,
    ulong DeletedOrchestraRunCount,
    ulong DeletedOrchestraEventCount,
    long RemovedAtUnixMs);

public sealed record RemoteUnregistrationReceiptLookup(
    string CommandId,
    RemoteUnregistrationReceipt? Receipt,
    RemoteUnregistrationReplayHorizon ReplayHorizon);

public sealed class RemoteUnregistrationReceiptClient : IDisposable
{
    private readonly RemoteWireTransport transport;

    public RemoteUnregistrationReceiptClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteUnregistrationReceiptLookup> LookupAsync(
        string commandId,
        string principal,
        CancellationToken cancellationToken = default)
    {
        RemoteQueryValidation.RequireIdentifier(commandId, "command ID");
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        var request = new WireUnregistrationReceiptRequestEnvelope
        {
            Request = new WireUnregistrationReceiptRequest
            {
                Payload = new WireUnregistrationReceiptRequestPayload
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["runtime.read"],
                    CommandId = commandId,
                },
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            request,
            RemoteUnregistrationReceiptJsonContext.Default
                .WireUnregistrationReceiptRequestEnvelope);
        var response = await transport.PostAsync(
            payload,
            "runtime_unregistration_receipt",
            cancellationToken).ConfigureAwait(false);
        var lookup = RemoteUnregistrationReceiptCodec.Decode(response);
        if (!string.Equals(lookup.CommandId, commandId, StringComparison.Ordinal))
        {
            throw new InvalidDataException(
                "remote unregistration receipt changed the command identity");
        }
        return lookup;
    }

    public void Dispose() => transport.Dispose();
}

public static class RemoteUnregistrationReceiptCodec
{
    private const int MaxTargets = 128;
    private const ulong MaxRunsPerRuntime = 32;
    private const ulong MaxEventsPerRun = 3;

    public static RemoteUnregistrationReceiptLookup Decode(ReadOnlySpan<byte> payload)
    {
        if (payload.Length > RemoteEventCodec.MaxMessageBytes)
        {
            throw new InvalidDataException(
                "remote unregistration receipt response exceeds the message limit");
        }
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteUnregistrationReceiptJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException(
                    "remote unregistration receipt response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException(
                    "unsupported remote unregistration receipt response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw new RemoteMutationException(
                    RequiredString(envelope.Response.Payload, "code"),
                    RequiredString(envelope.Response.Payload, "message"));
            }
            if (envelope.Response.Kind != "runtime_unregistration_receipt")
            {
                throw new InvalidDataException(
                    "remote unregistration receipt returned an unexpected response kind");
            }
            var wire = envelope.Response.Payload.Deserialize(
                RemoteUnregistrationReceiptJsonContext.Default
                    .WireUnregistrationReceiptLookupPayload)
                ?? throw new InvalidDataException(
                    "remote unregistration receipt payload is empty");
            RemoteQueryValidation.RequireIdentifier(wire.CommandId, "command ID");
            var horizon = RemoteHealthCodec.ValidateReplayHorizon(wire.ReplayHorizon);
            var receipt = wire.Receipt is null
                ? null
                : ValidateReceipt(wire.Receipt, horizon);
            return new RemoteUnregistrationReceiptLookup(wire.CommandId, receipt, horizon);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException(
                "remote unregistration receipt response JSON is invalid",
                error);
        }
        catch (ArgumentException error)
        {
            throw new InvalidDataException(
                "remote unregistration receipt identity is invalid",
                error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException(
                "remote unregistration receipt field type is invalid",
                error);
        }
        catch (OverflowException error)
        {
            throw new InvalidDataException(
                "remote unregistration receipt counters overflow",
                error);
        }
    }

    private static RemoteUnregistrationReceipt ValidateReceipt(
        WireUnregistrationReceipt receipt,
        RemoteUnregistrationReplayHorizon horizon)
    {
        if (receipt.OperationGeneration == 0
            || horizon.Classify(receipt.OperationGeneration)
                != RemoteUnregistrationGenerationState.Retained
            || receipt.Removed.Count is < 1 or > MaxTargets
            || receipt.DeletedOrchestraRuntimeCount > receipt.Removed.Count
            || receipt.RemovedAtUnixMs < 0)
        {
            throw new InvalidDataException(
                "remote unregistration receipt bounds are inconsistent");
        }
        var targets = new List<RemoteUnregistrationTarget>(receipt.Removed.Count);
        var identities = new HashSet<string>(StringComparer.Ordinal);
        foreach (var target in receipt.Removed)
        {
            RemoteQueryValidation.RequireIdentifier(target.RuntimeId, "runtime ID");
            if (target.ExpectedRevision == 0 || !identities.Add(target.RuntimeId))
            {
                throw new InvalidDataException(
                    "remote unregistration receipt targets are inconsistent");
            }
            targets.Add(new RemoteUnregistrationTarget(
                target.RuntimeId,
                target.ExpectedRevision));
        }
        var maximumRuns = checked((ulong)targets.Count * MaxRunsPerRuntime);
        var maximumEvents = checked(receipt.DeletedOrchestraRunCount * MaxEventsPerRun);
        if (receipt.DeletedOrchestraRunCount > maximumRuns
            || receipt.DeletedOrchestraEventCount > maximumEvents)
        {
            throw new InvalidDataException(
                "remote unregistration receipt cleanup counts are inconsistent");
        }
        return new RemoteUnregistrationReceipt(
            receipt.OperationGeneration,
            targets,
            receipt.DeletedOrchestraRuntimeCount,
            receipt.DeletedOrchestraRunCount,
            receipt.DeletedOrchestraEventCount,
            receipt.RemovedAtUnixMs);
    }

    private static string RequiredString(JsonElement value, string name)
    {
        var result = value.GetProperty(name).GetString();
        if (string.IsNullOrWhiteSpace(result) || result.Length > 4096)
        {
            throw new InvalidDataException(
                $"remote unregistration receipt {name} is invalid");
        }
        return result;
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireUnregistrationReceiptRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireUnregistrationReceiptRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireUnregistrationReceiptRequest
{
    public string Kind { get; set; } = "runtime_unregistration_receipt";
    public required WireUnregistrationReceiptRequestPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireUnregistrationReceiptRequestPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string CommandId { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireUnregistrationReceiptLookupPayload
{
    public required string CommandId { get; set; }
    public WireUnregistrationReceipt? Receipt { get; set; }
    public required WireUnregistrationReplayHorizon ReplayHorizon { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireUnregistrationReceipt
{
    public ulong OperationGeneration { get; set; }
    public required List<WireUnregistrationTarget> Removed { get; set; }
    public uint DeletedOrchestraRuntimeCount { get; set; }
    public ulong DeletedOrchestraRunCount { get; set; }
    public ulong DeletedOrchestraEventCount { get; set; }
    public long RemovedAtUnixMs { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireUnregistrationTarget
{
    public required string RuntimeId { get; set; }
    public ulong ExpectedRevision { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireUnregistrationReceiptRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(WireUnregistrationReceiptLookupPayload))]
public partial class RemoteUnregistrationReceiptJsonContext : JsonSerializerContext;
