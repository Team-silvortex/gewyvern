using System.Text.Json;
using System.Text.Json.Serialization;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireCommandRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireCommandRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireCommandRequest
{
    public string Kind { get; set; } = "command";
    public required RuntimeRefreshEnvelope Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeRefreshEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required string CommandId { get; set; }
    public required string IdempotencyKey { get; set; }
    public ulong ExpectedRevision { get; set; }
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public string Origin { get; set; } = "gui";
    public string Confirmation { get; set; } = "confirmed";
    public bool DryRun { get; set; }
    public required RuntimeRefreshCommand Command { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemotePrincipal
{
    public required string Id { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeRefreshCommand
{
    public string Kind { get; set; } = "runtime_refresh";
    public required string RuntimeId { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? PipelineKind { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Target { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireResponseEnvelope
{
    public int SchemaVersion { get; set; }
    public required WireResponse Response { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireResponse
{
    public required string Kind { get; set; }
    public JsonElement Payload { get; set; }
}

public sealed record RemoteMutationResult(
    string CommandId,
    string RuntimeId,
    ulong Revision,
    string Status);

public sealed class RemoteMutationException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireCommandRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
public partial class RemoteMutationJsonContext : JsonSerializerContext;
