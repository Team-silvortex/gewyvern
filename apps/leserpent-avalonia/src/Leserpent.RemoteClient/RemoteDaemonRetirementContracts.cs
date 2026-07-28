using System.Text.Json;
using System.Text.Json.Serialization;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required DaemonRetirementRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementRequest
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required DaemonRetirementIntent Intent { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementIntent
{
    public int SchemaVersion { get; set; } = 1;
    public required string RetirementId { get; set; }
    public required string BootstrapId { get; set; }
    public required string RetirementCredentialHandle { get; set; }
    public required string RequestedBy { get; set; }
    public bool Confirmed { get; set; } = true;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementResponseEnvelope
{
    public int SchemaVersion { get; set; }
    public required DaemonRetirementTaggedResponse Response { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementTaggedResponse
{
    public required string Kind { get; set; }
    public JsonElement Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementSnapshotPayload
{
    public required string RetirementId { get; set; }
    public required string BootstrapId { get; set; }
    public required string DaemonId { get; set; }
    public required string Phase { get; set; }
    public required DaemonRetirementTarget Target { get; set; }
    public required string Generation { get; set; }
    public required string InstallProfile { get; set; }
    public required bool RetirementCredentialPresent { get; set; }
    public required bool ServiceRetired { get; set; }
    public string? FaultCode { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementTarget
{
    public required string Transport { get; set; }
    public required string Host { get; set; }
    public required ushort Port { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed class DaemonRetirementErrorPayload
{
    public string? RetirementId { get; set; }
    public required string Code { get; set; }
    public required string Message { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(DaemonRetirementRequestEnvelope))]
[JsonSerializable(typeof(DaemonRetirementResponseEnvelope))]
[JsonSerializable(typeof(DaemonRetirementSnapshotPayload))]
[JsonSerializable(typeof(DaemonRetirementErrorPayload))]
internal partial class RemoteDaemonRetirementJsonContext : JsonSerializerContext;
