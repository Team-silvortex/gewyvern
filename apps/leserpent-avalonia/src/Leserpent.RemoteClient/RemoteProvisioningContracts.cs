using System.Text.Json;
using System.Text.Json.Serialization;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required ProvisioningRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningRequest
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required ProvisioningIntent Intent { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningIntent
{
    public int SchemaVersion { get; set; } = 1;
    public required string ProvisioningId { get; set; }
    public required string RuntimeId { get; set; }
    public required ProvisioningTarget Target { get; set; }
    public required string InstallCredentialHandle { get; set; }
    public required string RequestedBy { get; set; }
    public bool Confirmed { get; set; } = true;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningTarget
{
    public required string Transport { get; set; }
    public required string Host { get; set; }
    public required ushort Port { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningResponseEnvelope
{
    public int SchemaVersion { get; set; }
    public required ProvisioningTaggedResponse Response { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningTaggedResponse
{
    public required string Kind { get; set; }
    public JsonElement Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningSnapshotPayload
{
    public required string ProvisioningId { get; set; }
    public required string RuntimeId { get; set; }
    public required string Phase { get; set; }
    public required ProvisioningTarget Target { get; set; }
    public required bool InstallCredentialPresent { get; set; }
    public string? Endpoint { get; set; }
    public string? ApiCredentialHandle { get; set; }
    public string? TrustCredentialHandle { get; set; }
    public string? FaultCode { get; set; }
    public required bool RuntimeRegistered { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class ProvisioningErrorPayload
{
    public string? ProvisioningId { get; set; }
    public required string Code { get; set; }
    public required string Message { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(ProvisioningRequestEnvelope))]
[JsonSerializable(typeof(ProvisioningResponseEnvelope))]
[JsonSerializable(typeof(ProvisioningSnapshotPayload))]
[JsonSerializable(typeof(ProvisioningErrorPayload))]
internal partial class RemoteProvisioningJsonContext : JsonSerializerContext;
