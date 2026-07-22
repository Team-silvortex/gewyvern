using System.Text.Json;
using System.Text.Json.Serialization;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required BootstrapRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapRequest
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required BootstrapIntent Intent { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapIntent
{
    public int SchemaVersion { get; set; } = 1;
    public required string BootstrapId { get; set; }
    public required BootstrapTarget Target { get; set; }
    public required string CredentialHandle { get; set; }
    public required string RequestedBy { get; set; }
    public bool Confirmed { get; set; } = true;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapTarget
{
    public string Transport { get; set; } = "ssh";
    public required string Host { get; set; }
    public ushort Port { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapResponseEnvelope
{
    public int SchemaVersion { get; set; }
    public required BootstrapTaggedResponse Response { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapTaggedResponse
{
    public required string Kind { get; set; }
    public JsonElement Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapWireRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required BootstrapWireRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapWireRequest
{
    public required string Kind { get; set; }
    public required BootstrapWirePayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapWirePayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string BootstrapId { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public bool? Confirmed { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapSnapshotPayload
{
    public required string BootstrapId { get; set; }
    public required string Phase { get; set; }
    public required BootstrapTarget Target { get; set; }
    public bool BootstrapCredentialPresent { get; set; }
    public string? DaemonId { get; set; }
    public string? Endpoint { get; set; }
    public string? SessionCredentialHandle { get; set; }
    public string? TrustCredentialHandle { get; set; }
    public string? FaultCode { get; set; }
    public bool MutationAuthorized { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class BootstrapErrorPayload
{
    public string? BootstrapId { get; set; }
    public required string Code { get; set; }
    public required string Message { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(BootstrapRequestEnvelope))]
[JsonSerializable(typeof(BootstrapResponseEnvelope))]
[JsonSerializable(typeof(BootstrapWireRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(BootstrapSnapshotPayload))]
[JsonSerializable(typeof(BootstrapErrorPayload))]
internal partial class RemoteBootstrapJsonContext : JsonSerializerContext;
