using System.Text.Json.Serialization;

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

public sealed class RemoteOrchestraException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(OrchestraHistoryRequestEnvelope))]
[JsonSerializable(typeof(OrchestraDeleteRequestEnvelope))]
[JsonSerializable(typeof(RemoteOrchestraHistoryPage))]
[JsonSerializable(typeof(RemoteOrchestraDeleteReceipt))]
public partial class RemoteOrchestraJsonContext : JsonSerializerContext;
