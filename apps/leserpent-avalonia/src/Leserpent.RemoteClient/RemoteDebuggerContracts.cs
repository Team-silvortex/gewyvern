using System.Text.Json;
using System.Text.Json.Serialization;

public enum RemoteDebuggerState
{
    Running,
    WaitingEffect,
    Yielded,
    Completed,
    Failed,
    Cancelled,
}

public sealed record RemoteDebuggerPendingEffect(
    string EffectId,
    string Kind,
    string? RuntimeId);

public sealed record RemoteDebuggerFrame(
    string FrameId,
    uint Instruction,
    string Display);

public sealed record RemoteDebuggerFault(string Code, string Display);

public sealed record RemoteDebuggerProjection(
    ulong Revision,
    string SessionId,
    RemoteDebuggerState State,
    uint ProgramCounter,
    ulong FuelRemaining,
    ulong? DeadlineRemainingMs,
    RemoteDebuggerPendingEffect? PendingEffect,
    IReadOnlyList<RemoteDebuggerFrame> Frames,
    RemoteDebuggerFault? Fault);

public sealed record RemoteDebuggerSession(
    RemoteDebuggerProjection Projection,
    UiDocument Document);

public sealed class RemoteDebuggerCancelPlan
{
    public RemoteDebuggerCancelPlan(
        string commandId,
        string sessionId,
        ulong expectedRevision,
        RemoteDebuggerSession reviewedSession)
        : this(commandId, sessionId, expectedRevision, reviewedSession, null, null)
    {
    }

    internal RemoteDebuggerCancelPlan(
        string commandId,
        string sessionId,
        ulong expectedRevision,
        RemoteDebuggerSession reviewedSession,
        object? authority,
        string? principal)
    {
        CommandId = commandId;
        SessionId = sessionId;
        ExpectedRevision = expectedRevision;
        ReviewedSession = reviewedSession;
        Authority = authority;
        Principal = principal;
    }

    public string CommandId { get; }
    public string SessionId { get; }
    public ulong ExpectedRevision { get; }
    public RemoteDebuggerSession ReviewedSession { get; }
    internal object? Authority { get; }
    internal string? Principal { get; }
}

public sealed record RemoteDebuggerCancelResult(
    string CommandId,
    RemoteDebuggerSession Session,
    ulong AuditedAtMs);

public sealed class RemoteDebuggerException(string code, string message)
    : Exception(message)
{
    public string Code { get; } = code;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionsRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireDebuggerSessionsRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionsRequest
{
    public string Kind { get; set; } = "debugger_sessions";
    public required WireDebuggerSessionsPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionsPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? SessionId { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionStartRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireDebuggerSessionStartRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionStartRequest
{
    public string Kind { get; set; } = "debugger_session_start";
    public required WireDebuggerSessionStartPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionStartPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string SessionId { get; set; }
    public required string Source { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? ExpectedRevision { get; set; }
    public ulong TimeoutMs { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerCommandRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireDebuggerCommandRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerCommandRequest
{
    public string Kind { get; set; } = "command";
    public required WireDebuggerCommandEnvelope Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerCommandEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required string CommandId { get; set; }
    public required string IdempotencyKey { get; set; }
    public ulong ExpectedRevision { get; set; }
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public string Origin { get; set; } = "gui";
    public required string Confirmation { get; set; }
    public bool DryRun { get; set; }
    public required WireDebuggerCancelCommand Command { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerCancelCommand
{
    public string Kind { get; set; } = "debugger_cancel";
    public required string SessionId { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionsResponse
{
    public required List<WireDebuggerSessionView> Sessions { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionResponse
{
    public required WireDebuggerSessionView Session { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerCancelResponse
{
    public required string CommandId { get; set; }
    public required string Status { get; set; }
    public required WireDebuggerSessionView Session { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public ulong? AuditedAtMs { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerSessionView
{
    public required WireDebuggerProjection Projection { get; set; }
    public JsonElement Document { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerProjection
{
    public ulong Revision { get; set; }
    public required string SessionId { get; set; }
    public required string State { get; set; }
    public uint ProgramCounter { get; set; }
    public ulong FuelRemaining { get; set; }
    public ulong? DeadlineRemainingMs { get; set; }
    public WireDebuggerPendingEffect? PendingEffect { get; set; }
    public required List<WireDebuggerFrame> Frames { get; set; }
    public WireDebuggerFault? Fault { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerPendingEffect
{
    public required string EffectId { get; set; }
    public required string Kind { get; set; }
    public string? RuntimeId { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerFrame
{
    public required string FrameId { get; set; }
    public uint Instruction { get; set; }
    public required string Display { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireDebuggerFault
{
    public required string Code { get; set; }
    public required string Display { get; set; }
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireDebuggerSessionsRequestEnvelope))]
[JsonSerializable(typeof(WireDebuggerSessionStartRequestEnvelope))]
[JsonSerializable(typeof(WireDebuggerCommandRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(WireDebuggerSessionsResponse))]
[JsonSerializable(typeof(WireDebuggerSessionResponse))]
[JsonSerializable(typeof(WireDebuggerCancelResponse))]
public partial class RemoteDebuggerJsonContext : JsonSerializerContext;
