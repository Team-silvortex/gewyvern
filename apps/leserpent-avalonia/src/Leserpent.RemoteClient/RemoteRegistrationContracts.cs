using System.Text.Json.Serialization;

public enum RemoteRegistrationMode
{
    Register,
    Update,
}

public sealed record RemoteRegistrationIntent(
    string RuntimeId,
    string Name,
    string Endpoint,
    string? SidecarEndpoint,
    string? Environment,
    string? Cluster,
    string? Role);

public sealed record RemoteRegistrationDetails(
    RemoteRegistrationIntent Intent,
    ulong Revision);

public sealed class RemoteRegistrationPlan
{
    public RemoteRegistrationPlan(
        string commandId,
        RemoteRegistrationMode mode,
        RemoteRegistrationIntent intent,
        ulong? expectedRevision,
        ulong plannedRevision)
    {
        CommandId = commandId;
        Mode = mode;
        Intent = intent;
        ExpectedRevision = expectedRevision;
        PlannedRevision = plannedRevision;
    }

    public string CommandId { get; }
    public RemoteRegistrationMode Mode { get; }
    public RemoteRegistrationIntent Intent { get; }
    public ulong? ExpectedRevision { get; }
    public ulong PlannedRevision { get; }
}

public sealed record RemoteRegistrationResult(
    string CommandId,
    RemoteRegistrationMode Mode,
    string RuntimeId,
    ulong Revision);

public sealed class RemoteRegistrationException(string code, string message)
    : Exception(message)
{
    public string Code { get; } = code;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRegistrationCommandRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireRegistrationCommandRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRegistrationCommandRequest
{
    public string Kind { get; set; } = "command";
    public required WireRegistrationCommandEnvelope Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRegistrationCommandEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required string CommandId { get; set; }
    public required string IdempotencyKey { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? ExpectedRevision { get; set; }
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public string Origin { get; set; } = "gui";
    public required string Confirmation { get; set; }
    public bool DryRun { get; set; }
    public required WireRegistrationCommand Command { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRegistrationCommand
{
    public required string Kind { get; set; }
    public required string RuntimeId { get; set; }
    public required string Name { get; set; }
    public required string Endpoint { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? SidecarEndpoint { get; set; }
    public required RuntimeTags Tags { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRegistrationCommandResult
{
    public required string CommandId { get; set; }
    public required string Status { get; set; }
    public required WireRuntimeProjection Runtime { get; set; }
    public required List<WireRegistrationEvent> Events { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRegistrationEvent
{
    public required string Kind { get; set; }
    public required string RuntimeId { get; set; }
    public ulong Revision { get; set; }
    public required string CommandId { get; set; }
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireRegistrationCommandRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(WireRegistrationCommandResult))]
[JsonSerializable(typeof(RuntimeInspectQueryResult))]
public partial class RemoteRegistrationJsonContext : JsonSerializerContext;
