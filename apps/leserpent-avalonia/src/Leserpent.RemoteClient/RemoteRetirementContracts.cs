using System.Text.Json;
using System.Text.Json.Serialization;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required RetirementRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementRequest
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required RetirementIntent Intent { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementIntent
{
    public int SchemaVersion { get; set; } = 1;
    public required string RetirementId { get; set; }
    public required string ProvisioningId { get; set; }
    public required string RuntimeId { get; set; }
    public required RetirementTarget Target { get; set; }
    public required string RetirementCredentialHandle { get; set; }
    public required string RequestedBy { get; set; }
    public bool Confirmed { get; set; } = true;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementTarget
{
    public required string Transport { get; set; }
    public required string Host { get; set; }
    public required ushort Port { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementResponseEnvelope
{
    public int SchemaVersion { get; set; }
    public required RetirementTaggedResponse Response { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementTaggedResponse
{
    public required string Kind { get; set; }
    public JsonElement Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementSnapshotPayload
{
    public required string RetirementId { get; set; }
    public required string ProvisioningId { get; set; }
    public required string RuntimeId { get; set; }
    public required string Phase { get; set; }
    public required RetirementTarget Target { get; set; }
    public required bool RetirementCredentialPresent { get; set; }
    public required bool ServiceRetired { get; set; }
    public required bool RuntimeRegistered { get; set; }
    public string? FaultCode { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class RetirementErrorPayload
{
    public string? RetirementId { get; set; }
    public required string Code { get; set; }
    public required string Message { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(RetirementRequestEnvelope))]
[JsonSerializable(typeof(RetirementResponseEnvelope))]
[JsonSerializable(typeof(RetirementSnapshotPayload))]
[JsonSerializable(typeof(RetirementErrorPayload))]
internal partial class RemoteRetirementJsonContext : JsonSerializerContext;
