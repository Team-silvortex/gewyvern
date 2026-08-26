using System.Text.Json.Serialization;

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraPlanCatalogRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required OrchestraPlanCatalogRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraPlanCatalogRequest
{
    public string Kind { get; set; } = "orchestra_plan_catalog";
    public required OrchestraPlanCatalogRequestPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraPlanCatalogRequestPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string RuntimeId { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraRunCommandRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required OrchestraRunCommandRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraRunCommandRequest
{
    public string Kind { get; set; } = "orchestra_run_command";
    public required OrchestraRunCommandRequestPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraRunCommandRequestPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string CommandId { get; set; }
    public required string RuntimeId { get; set; }
    public required string PlanId { get; set; }
    public required string ExpectedPlanRevision { get; set; }
    public bool Confirmed { get; set; }

    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? ApprovedBy { get; set; }

    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? ApprovalNote { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraCancelCommandRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required OrchestraCancelCommandRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraCancelCommandRequest
{
    public string Kind { get; set; } = "orchestra_cancel_command";
    public required OrchestraCancelCommandRequestPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraCancelCommandRequestPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string CommandId { get; set; }
    public required string RuntimeId { get; set; }
    public required string RunId { get; set; }
    public bool Confirmed { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraRetryCommandRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required OrchestraRetryCommandRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraRetryCommandRequest
{
    public string Kind { get; set; } = "orchestra_retry_command";
    public required OrchestraRetryCommandRequestPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraRetryCommandRequestPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string CommandId { get; set; }
    public required string RuntimeId { get; set; }
    public required string RunId { get; set; }
    public required string ExpectedPlanRevision { get; set; }
    public bool Confirmed { get; set; }

    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? ApprovedBy { get; set; }

    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? ApprovalNote { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraHistoryRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required OrchestraHistoryRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraHistoryRequest
{
    public string Kind { get; set; } = "orchestra_history";
    public required OrchestraHistoryRequestPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraHistoryRequestPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public string? RuntimeId { get; set; }
    public string? RunId { get; set; }
    public uint Offset { get; set; }
    public ushort Limit { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraDeleteRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required OrchestraDeleteRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraDeleteRequest
{
    public string Kind { get; set; } = "orchestra_delete_command";
    public required OrchestraDeleteRequestPayload Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class OrchestraDeleteRequestPayload
{
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required string CommandId { get; set; }
    public required List<string> RuntimeIds { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraHistoryPage
{
    public required List<RemoteOrchestraRun> Runs { get; set; }
    public required List<RemoteOrchestraEvent> Events { get; set; }
    public uint? NextOffset { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraPlanCatalog
{
    public required string RuntimeId { get; set; }
    public required string RuntimeName { get; set; }
    public ulong RuntimeRevision { get; set; }
    public required string StatusSource { get; set; }
    public required string AttentionSeverity { get; set; }
    public bool NeedsAttention { get; set; }
    public required List<string> AttentionReasons { get; set; }
    public required List<RemoteOrchestraPlan> Plans { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraPlan
{
    public required string PlanId { get; set; }
    public required string Intent { get; set; }
    public required string Title { get; set; }
    public required string Summary { get; set; }
    public required string RiskLevel { get; set; }
    public required string ExecutionReadiness { get; set; }
    public required string ExecutionMode { get; set; }
    public required string ApprovalMode { get; set; }
    public required string Revision { get; set; }
    public required List<string> Reasons { get; set; }
    public required List<string> RequiredCapabilities { get; set; }
    public required List<RemoteOrchestraPlanStep> Steps { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraPlanStep
{
    public required string Key { get; set; }
    public required string Title { get; set; }
    public required string Detail { get; set; }
    public required string Kind { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraStep
{
    [JsonPropertyName("step")]
    public required string Step { get; set; }

    [JsonPropertyName("outcome")]
    public required string Outcome { get; set; }

    [JsonPropertyName("summary")]
    public required string Summary { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraRun
{
    [JsonPropertyName("runId")]
    public required string RunId { get; set; }

    [JsonPropertyName("runtimeId")]
    public required string RuntimeId { get; set; }

    [JsonPropertyName("planId")]
    public required string PlanId { get; set; }

    [JsonPropertyName("outcome")]
    public required string Outcome { get; set; }

    [JsonPropertyName("executedAt")]
    public required string ExecutedAt { get; set; }

    [JsonPropertyName("steps")]
    public required List<RemoteOrchestraStep> Steps { get; set; }

    [JsonPropertyName("completedAt")]
    public string? CompletedAt { get; set; }

    [JsonPropertyName("attempt")]
    public uint Attempt { get; set; }

    [JsonPropertyName("retriedFromRunId")]
    public string? RetriedFromRunId { get; set; }

    [JsonPropertyName("approvedBy")]
    public string? ApprovedBy { get; set; }

    [JsonPropertyName("approvalNote")]
    public string? ApprovalNote { get; set; }

    [JsonPropertyName("planRevision")]
    public string? PlanRevision { get; set; }

    [JsonPropertyName("requestId")]
    public string? RequestId { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraEvent
{
    [JsonPropertyName("eventId")]
    public ulong EventId { get; set; }

    [JsonPropertyName("runId")]
    public required string RunId { get; set; }

    [JsonPropertyName("runtimeId")]
    public required string RuntimeId { get; set; }

    [JsonPropertyName("eventType")]
    public required string EventType { get; set; }

    [JsonPropertyName("fromOutcome")]
    public string? FromOutcome { get; set; }

    [JsonPropertyName("toOutcome")]
    public required string ToOutcome { get; set; }

    [JsonPropertyName("summary")]
    public required string Summary { get; set; }

    [JsonPropertyName("recordedAt")]
    public required string RecordedAt { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraDeleteReceipt
{
    public required string CommandId { get; set; }
    public ulong OperationGeneration { get; set; }
    public required List<string> RuntimeIds { get; set; }
    public uint DeletedRuntimeCount { get; set; }
    public ulong DeletedRunCount { get; set; }
    public ulong DeletedEventCount { get; set; }
    public long CommittedAtUnixMs { get; set; }
    public bool Replayed { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteOrchestraRunReceipt
{
    public required string CommandId { get; set; }
    public required string Operation { get; set; }
    public required RemoteOrchestraRun Run { get; set; }
    public bool Replayed { get; set; }
}

public sealed class RemoteOrchestraException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(OrchestraPlanCatalogRequestEnvelope))]
[JsonSerializable(typeof(OrchestraRunCommandRequestEnvelope))]
[JsonSerializable(typeof(OrchestraCancelCommandRequestEnvelope))]
[JsonSerializable(typeof(OrchestraRetryCommandRequestEnvelope))]
[JsonSerializable(typeof(OrchestraHistoryRequestEnvelope))]
[JsonSerializable(typeof(OrchestraDeleteRequestEnvelope))]
[JsonSerializable(typeof(RemoteOrchestraPlanCatalog))]
[JsonSerializable(typeof(RemoteOrchestraRunReceipt))]
[JsonSerializable(typeof(RemoteOrchestraHistoryPage))]
[JsonSerializable(typeof(RemoteOrchestraDeleteReceipt))]
public partial class RemoteOrchestraJsonContext : JsonSerializerContext;
