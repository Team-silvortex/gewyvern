using System.Text.Json;
using System.Text.Json.Serialization;

namespace Leserpent.ControlPlane;

public sealed partial class CapabilityDiscoveryService
{
    private sealed record GewyvernCapabilityPayload(
        [property: JsonPropertyName("service")] string Service,
        [property: JsonPropertyName("version")] string Version,
        [property: JsonPropertyName("latest_snapshot")] bool LatestSnapshot,
        [property: JsonPropertyName("serve_required")] bool ServeRequired,
        [property: JsonPropertyName("external_sidecar_context")] bool ExternalSidecarContext,
        [property: JsonPropertyName("target_path_segment_encoding")] string TargetPathSegmentEncoding,
        [property: JsonPropertyName("endpoints")] string[] Endpoints
    );

    private sealed record GewyvernLatestMetaPayload(
        [property: JsonPropertyName("updated_unix_ms")] long UpdatedUnixMs,
        [property: JsonPropertyName("kind")] string? Kind,
        [property: JsonPropertyName("target_count")] int? TargetCount,
        [property: JsonPropertyName("has_summary_json")] bool HasSummaryJson,
        [property: JsonPropertyName("has_analysis_json")] bool HasAnalysisJson,
        [property: JsonPropertyName("has_training_example_json")] bool HasTrainingExampleJson,
        [property: JsonPropertyName("has_export_json")] bool HasExportJson,
        [property: JsonPropertyName("has_report_json")] bool HasReportJson,
        [property: JsonPropertyName("has_report_html")] bool HasReportHtml,
        [property: JsonPropertyName("has_external_sidecar_context")] bool HasExternalSidecarContext,
        [property: JsonPropertyName("has_external_evidence_chain_enrichment")] bool HasExternalEvidenceChainEnrichment,
        [property: JsonPropertyName("has_external_diagnostic_opinion")] bool HasExternalDiagnosticOpinion
    );

    private sealed record GewyvernRuntimeResiliencePayload(
        [property: JsonPropertyName("degraded")] bool Degraded,
        [property: JsonPropertyName("status")] string? Status,
        [property: JsonPropertyName("summary")] string? Summary,
        [property: JsonPropertyName("socket_service")] GewyvernRuntimeResilienceSocketPayload? SocketService
    );

    private sealed record GewyvernRuntimeResilienceSocketPayload(
        [property: JsonPropertyName("status")] string? Status,
        [property: JsonPropertyName("consecutive_idle_timeouts")] int? ConsecutiveIdleTimeouts,
        [property: JsonPropertyName("total_idle_timeouts")] int? TotalIdleTimeouts
    );

    private sealed record GewyvernLatestTargetsPayload(
        [property: JsonPropertyName("target_refs")] GewyvernLatestTargetRefPayload[]? TargetRefs
    );

    private sealed record GewyvernLatestTargetRefPayload(
        [property: JsonPropertyName("name")] string? Name,
        [property: JsonPropertyName("path_segment")] string? PathSegment,
        [property: JsonPropertyName("url_path")] string? UrlPath,
        [property: JsonPropertyName("has_protocol_surface")] bool HasProtocolSurface
    );

    private sealed record GewyvernProtocolSurfacePayload(
        [property: JsonPropertyName("protocol")] string? Protocol,
        [property: JsonPropertyName("entry")] string? Entry,
        [property: JsonPropertyName("default_entry")] string? DefaultEntry,
        [property: JsonPropertyName("selected_is_default")] bool SelectedIsDefault,
        [property: JsonPropertyName("selected_overlay")] string? SelectedOverlay,
        [property: JsonPropertyName("reading_companions")] GewyvernProtocolReadingCompanionPayload[]? ReadingCompanions
    );

    private sealed record GewyvernProtocolReadingCompanionPayload(
        [property: JsonPropertyName("protocol")] string? Protocol,
        [property: JsonPropertyName("entry")] string? Entry,
        [property: JsonPropertyName("via_overlay")] string? ViaOverlay
    );

    private sealed record EtragonHealthPayload(
        [property: JsonPropertyName("status")] string Status
    );

    private sealed record EtragonLatestStatusPayload(
        [property: JsonPropertyName("status")] string? Status,
        [property: JsonPropertyName("target_count")] int? TargetCount,
        [property: JsonPropertyName("learning_active")] bool LearningActive,
        [property: JsonPropertyName("learned_routes")] int LearnedRoutes,
        [property: JsonPropertyName("last_error")] string? LastError
    );

    private sealed record EtragonMemoryVersionsPayload(
        [property: JsonPropertyName("slot_count")] int? SlotCount,
        [property: JsonPropertyName("slots")] EtragonMemorySlotPayload[]? Slots,
        [property: JsonPropertyName("history")] JsonElement[]? History
    );

    private sealed record EtragonMemorySlotPayload(
        [property: JsonPropertyName("slot")] string? Slot,
        [property: JsonPropertyName("label")] string? Label,
        [property: JsonPropertyName("note")] string? Note,
        [property: JsonPropertyName("source")] string? Source,
        [property: JsonPropertyName("saved_unix_ms")] long? SavedUnixMs,
        [property: JsonPropertyName("pattern_count")] int? PatternCount,
        [property: JsonPropertyName("label_count")] int? LabelCount
    );
}
