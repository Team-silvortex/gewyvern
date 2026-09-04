using System.Text.Json.Serialization;

namespace Leserpent.ControlPlane;

public sealed record ApiErrorResponse(
    string Error,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? Reason = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? RuntimeId = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? PlanId = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? RunId = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? RequestId = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? SessionId = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] string? Outcome = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] long? MaxBytes = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] int? SchemaVersion = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] int? ExpectedSchemaVersion = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] IReadOnlyList<CapabilityRejection>? Rejections = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] IReadOnlyList<OrchestraActiveRunConflict>? ActiveRuns = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] OrchestraRunSummary? ActiveRun = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] OrchestraPlan? CurrentPlan = null,
    [property: JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)] IReadOnlyList<OrchestraSuggestedSurface>? SuggestedSurfaces = null);

public sealed record HealthResponse(
    bool Ok,
    string Service,
    string Role,
    string Version,
    HealthSecurityResponse Security,
    ServiceRuntimePosture RuntimePosture,
    HealthPersistenceResponse Persistence,
    HealthOrchestraPersistenceResponse OrchestraPersistence);

public sealed record HealthSecurityResponse(
    string ApiMode,
    bool AdminTokenConfigured,
    bool PublicEndpointDiscoveryAllowed);

public sealed record HealthPersistenceResponse(
    string StatePath,
    string BackupStatePath,
    DateTimeOffset? LastSavedAt,
    int SchemaVersion,
    bool IsDirty,
    string? LastSaveError,
    ControlPlaneStateLoadProvenance Load,
    int RestoredRuntimeCount,
    int RestoredSessionCount,
    DateTimeOffset? RestoredFromSavedAt);

public sealed record ControlPlaneStateLoadProvenance(
    ControlPlaneStateLoadSource Source,
    ControlPlaneStateLoadOutcome Outcome,
    bool Degraded,
    ControlPlaneStateLoadFailureCode? PrimaryFailureCode,
    ControlPlaneStateLoadFailureCode? BackupFailureCode);

[JsonConverter(typeof(JsonStringEnumConverter<ControlPlaneStateLoadSource>))]
public enum ControlPlaneStateLoadSource
{
    [JsonStringEnumMemberName("none")]
    None,
    [JsonStringEnumMemberName("empty")]
    Empty,
    [JsonStringEnumMemberName("primary")]
    Primary,
    [JsonStringEnumMemberName("backup")]
    Backup,
}

[JsonConverter(typeof(JsonStringEnumConverter<ControlPlaneStateLoadOutcome>))]
public enum ControlPlaneStateLoadOutcome
{
    [JsonStringEnumMemberName("not_attempted")]
    NotAttempted,
    [JsonStringEnumMemberName("empty")]
    Empty,
    [JsonStringEnumMemberName("clean")]
    Clean,
    [JsonStringEnumMemberName("recovered")]
    Recovered,
    [JsonStringEnumMemberName("failed")]
    Failed,
}

[JsonConverter(typeof(JsonStringEnumConverter<ControlPlaneStateLoadFailureCode>))]
public enum ControlPlaneStateLoadFailureCode
{
    [JsonStringEnumMemberName("not_found")]
    NotFound,
    [JsonStringEnumMemberName("empty")]
    Empty,
    [JsonStringEnumMemberName("incompatible_schema")]
    IncompatibleSchema,
    [JsonStringEnumMemberName("invalid_json")]
    InvalidJson,
    [JsonStringEnumMemberName("semantic_invalid")]
    SemanticInvalid,
    [JsonStringEnumMemberName("too_large")]
    TooLarge,
    [JsonStringEnumMemberName("unsafe_file")]
    UnsafeFile,
    [JsonStringEnumMemberName("read_failed")]
    ReadFailed,
}

public sealed record HealthOrchestraPersistenceResponse(
    string Provider,
    string Location,
    int SchemaVersion,
    string? LastError,
    bool Ready);

public sealed record RuntimeCollectionResponse(RuntimeListFilter Filter, IReadOnlyList<RuntimeSummary> Runtimes);
public sealed record FleetSummaryResponse(RuntimeListFilter Filter, FleetSummary Summary);
public sealed record FleetAttentionListResponse(RuntimeListFilter Filter, IReadOnlyList<RuntimeAttentionItem> Runtimes);
public sealed record FleetAttentionSummaryResponse(RuntimeListFilter Filter, FleetAttentionSummary Summary);
public sealed record FleetRefreshAllEnvelope(RuntimeListFilter Filter, FleetRefreshAllResponse Refresh);
public sealed record FleetCapabilityRefreshEnvelope(RuntimeListFilter Filter, FleetCapabilityRefreshResponse Refresh);
public sealed record FleetSidecarRefreshEnvelope(RuntimeListFilter Filter, FleetSidecarRefreshResponse Refresh);
public sealed record FleetStatusRefreshEnvelope(RuntimeListFilter Filter, FleetStatusRefreshResponse Refresh);
public sealed record SessionCollectionResponse(IReadOnlyList<SessionSummary> Sessions);
public sealed record OrchestraRunCollectionResponse(string RuntimeId, IReadOnlyList<OrchestraRunSummary> Runs);
public sealed record OrchestraRunEventsResponse(string RuntimeId, string RunId, IReadOnlyList<OrchestraRunEvent> Events);
public sealed record OrchestraRunAcceptedResponse(OrchestraRunSummary Run, bool? Replayed = null);
public sealed record RuntimeDeploymentCompatibilityEnvelope(
    string RuntimeId,
    RuntimeDeploymentRequest Request);
public sealed record RuntimeDeleteResponse(bool Deleted, string RuntimeId, string Name, string Endpoint, int RemovedSessionCount);
public sealed record RuntimeBulkDeleteResponse(
    bool Deleted,
    RuntimeListFilter Filter,
    int RemovedRuntimeCount,
    int RemovedSessionCount,
    IReadOnlyList<string> RemovedRuntimeNames);

public sealed record RuntimeCleanupRequest(string PlanToken, string? Challenge = null);
public sealed record RuntimeCleanupTarget(string RuntimeId, string Name);
public sealed record RuntimeCleanupActionPlan(
    string Kind,
    int RuntimeCount,
    int SessionCount,
    IReadOnlyList<RuntimeCleanupTarget> Targets,
    string PlanToken,
    string? Challenge = null);
public sealed record RuntimeCleanupPlan(
    RuntimeListFilter Filter,
    string RiskLevel,
    RuntimeCleanupActionPlan Failed,
    RuntimeCleanupActionPlan Unobserved,
    RuntimeCleanupActionPlan Slice);

public sealed record RuntimeDeletionRetryNowRequest(
    long ExpectedRevision,
    string RequestId,
    string RequestedBy);
public sealed record RuntimeDeletionRetryNowResponse(
    bool Accepted,
    bool Replayed,
    PersistedRuntimeDeletionIntent? PendingIntent,
    PersistedRuntimeDeletionRetryAudit Audit);
public sealed record RuntimeDeletionReconciliationPlan(
    string IntentId,
    long IntentRevision,
    ulong DaemonRevision,
    IReadOnlyList<string> RuntimeIds,
    IReadOnlyList<string> ReappearedRuntimeIds,
    bool CanReconcile);
public sealed record RuntimeDeletionReconcileRequest(
    long ExpectedRevision,
    ulong ExpectedDaemonRevision,
    string RequestId,
    string RequestedBy,
    bool Confirmed);
public sealed record RuntimeDeletionReconcileResponse(
    bool Accepted,
    bool Replayed,
    PersistedRuntimeDeletionReconciliationAudit Audit);

public sealed record OrchestraDeleteCheckpointAlertAcknowledgeRequest(
    ulong AlertGeneration,
    string RequestedBy,
    bool Confirmed);
public sealed record OrchestraDeleteCheckpointAlertAcknowledgeResponse(
    bool Acknowledged,
    bool Replayed,
    OrchestraDeleteReplayCheckpointStatus Status);

internal sealed record RuntimeDeletionReconciliationStart(
    RuntimeDeletionReservation? Reservation,
    RuntimeDeletionReconcileResponse? Replay);

public sealed record RuntimeRegistrationPlanRequest(
    string Name,
    string Endpoint,
    string? SidecarEndpoint = null);
public sealed record RuntimeRegistrationPlan(
    bool Allowed,
    string Action,
    string? Reason,
    string? ExistingRuntimeId,
    string? ExistingRuntimeName,
    string? ExistingRuntimeEndpoint,
    string? PlannedRuntimeId,
    ulong? ExpectedRevision,
    bool AuthorityBound,
    string PlanToken);

public sealed record RuntimeRecoveryCommandRequest(string Kind);
public sealed record RuntimeRecoveryStepResult(string Kind, string Outcome, string Summary);
public sealed record RuntimeRecoveryCommandResponse(
    string RuntimeId,
    string Kind,
    string Outcome,
    IReadOnlyList<RuntimeRecoveryStepResult> Steps);

public sealed record OrchestraRevisionStep(string Key, string Kind);
public sealed record OrchestraRevisionPayload(
    string RuntimeId,
    string Endpoint,
    string? SidecarEndpoint,
    string PlanId,
    string Intent,
    string RiskLevel,
    string ExecutionReadiness,
    string ExecutionMode,
    string[] Reasons,
    string[] Capabilities,
    OrchestraRevisionStep[] Steps);
