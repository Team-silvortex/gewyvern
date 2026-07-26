using System.Text.Json;
using System.Text.Json.Serialization;

namespace Leserpent.ControlPlane;

public interface IDaemonRuntimeProjectionReader
{
    bool Enabled { get; }

    Task<IReadOnlyList<DaemonRuntimeProjection>> ListAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken);

    Task<DaemonRuntimeProjectionSnapshot> SnapshotAsync(
        CancellationToken cancellationToken);

    Task<DaemonRuntimeProjection?> InspectAsync(
        string runtimeId,
        CancellationToken cancellationToken);
}

public sealed record DaemonRuntimeProjectionSnapshot(
    ulong Revision,
    IReadOnlyList<DaemonRuntimeProjection> Runtimes);

public sealed record DaemonRuntimeProjection(
    string RuntimeId,
    string Name,
    string Endpoint,
    string? SidecarEndpoint,
    DateTimeOffset? RegisteredAt,
    DateTimeOffset? UpdatedAt,
    ulong Revision,
    RuntimeTags Tags,
    RuntimeStatusSnapshot Status,
    RuntimeCapabilityAuthoritySnapshot? Capabilities,
    RuntimeSidecarStatusSnapshot? SidecarStatus);

public sealed class RuntimeReadProjectionService(
    RegistryService registry,
    IDaemonRuntimeProjectionReader daemon)
{
    public async Task<IReadOnlyList<RuntimeSummary>> ListAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken)
    {
        var managed = registry.ListRuntimes(filter);
        if (!daemon.Enabled)
        {
            return managed;
        }
        var managedById = registry.ListRuntimes()
            .ToDictionary(runtime => runtime.RuntimeId, StringComparer.Ordinal);
        var authoritativeAll = await daemon.ListAsync(
            new RuntimeListFilter(null, null, null),
            cancellationToken);
        var authoritativeIds = authoritativeAll
            .Select(runtime => runtime.RuntimeId)
            .ToHashSet(StringComparer.Ordinal);
        var authoritative = authoritativeAll.Where(runtime => MatchesFilter(runtime.Tags, filter));
        var projected = new List<RuntimeSummary>(authoritativeAll.Count + managed.Count);
        foreach (var runtime in authoritative)
        {
            if (!managedById.TryGetValue(runtime.RuntimeId, out var compatibility))
            {
                throw new DaemonRuntimeProjectionException(
                    "daemon_projection_unmapped",
                    $"daemon runtime '{runtime.RuntimeId}' has no managed compatibility metadata");
            }
            projected.Add(Merge(runtime, compatibility));
        }
        projected.AddRange(managed.Where(runtime => !authoritativeIds.Contains(runtime.RuntimeId)));
        return projected
            .OrderBy(runtime => runtime.Name, StringComparer.OrdinalIgnoreCase)
            .ThenBy(runtime => runtime.RuntimeId, StringComparer.Ordinal)
            .ToArray();
    }

    public async Task<RuntimeSummary?> InspectAsync(
        string runtimeId,
        CancellationToken cancellationToken)
    {
        var managed = registry.GetRuntime(runtimeId);
        if (!daemon.Enabled)
        {
            return managed;
        }
        if (!ValidRuntimeId(runtimeId))
        {
            return managed;
        }
        var authoritative = await daemon.InspectAsync(runtimeId, cancellationToken);
        if (authoritative is null)
        {
            return managed;
        }
        if (managed is null)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_unmapped",
                $"daemon runtime '{runtimeId}' has no managed compatibility metadata");
        }
        return Merge(authoritative, managed);
    }

    private static RuntimeSummary Merge(
        DaemonRuntimeProjection authoritative,
        RuntimeSummary compatibility)
    {
        var observedCapabilities = authoritative.Capabilities is not null;
        return compatibility with
        {
            Name = authoritative.Name,
            Endpoint = authoritative.Endpoint,
            SidecarEndpoint = authoritative.SidecarEndpoint,
            RegisteredAt = authoritative.RegisteredAt ?? compatibility.RegisteredAt,
            UpdatedAt = authoritative.UpdatedAt ?? compatibility.UpdatedAt,
            Tags = authoritative.Tags,
            Status = authoritative.Status,
            Capabilities = observedCapabilities
                ? RuntimeCapabilityProjection.ToLegacy(authoritative.Capabilities!)
                : compatibility.Capabilities,
            CapabilitySource = observedCapabilities
                ? authoritative.Capabilities!.Source
                : compatibility.CapabilitySource,
            SidecarStatus = authoritative.SidecarStatus ?? compatibility.SidecarStatus,
        };
    }

    private static bool MatchesFilter(RuntimeTags tags, RuntimeListFilter filter) =>
        MatchesTag(tags.Environment, filter.Environment)
            && MatchesTag(tags.Cluster, filter.Cluster)
            && MatchesTag(tags.Role, filter.Role);

    private static bool MatchesTag(string? actual, string? expected) =>
        string.IsNullOrWhiteSpace(expected)
            || (!string.IsNullOrWhiteSpace(actual)
                && string.Equals(actual, expected.Trim(), StringComparison.OrdinalIgnoreCase));

    private static bool ValidRuntimeId(string value) =>
        !string.IsNullOrEmpty(value)
            && value.Length <= 128
            && value.All(character =>
                char.IsAsciiLetterOrDigit(character)
                    || character is '-' or '_' or ':' or '.');
}

public sealed class DaemonRuntimeProjectionException(
    string code,
    string message,
    Exception? innerException = null) : Exception(message, innerException)
{
    public string Code { get; } = code;
}

public sealed partial class DaemonRuntimeRegistrationAuthority
{
    public async Task<IReadOnlyList<DaemonRuntimeProjection>> ListAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken) =>
        (await ReadSnapshotAsync(filter, cancellationToken)).Runtimes;

    public Task<DaemonRuntimeProjectionSnapshot> SnapshotAsync(
        CancellationToken cancellationToken) =>
        ReadSnapshotAsync(
            new RuntimeListFilter(null, null, null),
            cancellationToken);

    private async Task<DaemonRuntimeProjectionSnapshot> ReadSnapshotAsync(
        RuntimeListFilter filter,
        CancellationToken cancellationToken)
    {
        EnsureProjectionEnabled();
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            using var response = await ExchangeAsync(
                BuildProjectionQuery(writer =>
                {
                    writer.WriteString("kind", "runtime_list");
                    writer.WritePropertyName("filter");
                    writer.WriteStartObject();
                    WriteOptionalString(writer, "environment", NormalizeFilter(filter.Environment));
                    WriteOptionalString(writer, "cluster", NormalizeFilter(filter.Cluster));
                    WriteOptionalString(writer, "role", NormalizeFilter(filter.Role));
                    writer.WriteEndObject();
                }),
                deadline.Token);
            var payload = RequireProjectionResponse(response.RootElement);
            var decoded = JsonSerializer.Deserialize(
                payload.GetRawText(),
                DaemonRuntimeProjectionJsonContext.Default.DaemonRuntimeListPayload)
                ?? throw new InvalidOperationException("leserpentd returned an empty runtime list");
            if (!string.Equals(decoded.Kind, "runtime_list", StringComparison.Ordinal))
            {
                throw new InvalidOperationException("leserpentd returned an unexpected runtime list payload");
            }
            if (decoded.Runtimes is null)
            {
                throw new InvalidOperationException("leserpentd runtime list omitted its projections");
            }
            return new DaemonRuntimeProjectionSnapshot(
                decoded.Revision,
                decoded.Runtimes.Select(ConvertProjection).ToArray());
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_timeout",
                "leserpentd runtime projection query timed out",
                error);
        }
        catch (DaemonRuntimeProjectionException)
        {
            throw;
        }
        catch (DaemonRuntimeRegistrationException error)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_unavailable",
                "leserpentd runtime projection transport failed",
                error);
        }
        catch (Exception error) when (
            error is JsonException
                or KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_invalid",
                "leserpentd returned an invalid runtime projection",
                error);
        }
    }

    public async Task<DaemonRuntimeProjection?> InspectAsync(
        string runtimeId,
        CancellationToken cancellationToken)
    {
        EnsureProjectionEnabled();
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            using var response = await ExchangeAsync(
                BuildProjectionQuery(writer =>
                {
                    writer.WriteString("kind", "runtime_inspect");
                    writer.WriteString("runtime_id", runtimeId);
                }),
                deadline.Token);
            var protocolResponse = RequireProtocolResponse(response.RootElement);
            if (string.Equals(protocolResponse.GetProperty("kind").GetString(), "error", StringComparison.Ordinal))
            {
                var error = protocolResponse.GetProperty("payload");
                if (string.Equals(error.GetProperty("code").GetString(), "runtime_not_found", StringComparison.Ordinal))
                {
                    return null;
                }
                throw new DaemonRuntimeProjectionException(
                    error.GetProperty("code").GetString() ?? "daemon_projection_failed",
                    error.GetProperty("message").GetString() ?? "leserpentd rejected the runtime projection query");
            }
            var payload = RequireProjectionResponse(response.RootElement);
            var decoded = JsonSerializer.Deserialize(
                payload.GetRawText(),
                DaemonRuntimeProjectionJsonContext.Default.DaemonRuntimeInspectPayload)
                ?? throw new InvalidOperationException("leserpentd returned an empty runtime projection");
            if (!string.Equals(decoded.Kind, "runtime_inspect", StringComparison.Ordinal))
            {
                throw new InvalidOperationException("leserpentd returned an unexpected runtime projection payload");
            }
            if (decoded.Runtime is null)
            {
                throw new InvalidOperationException("leserpentd runtime inspect omitted its projection");
            }
            return ConvertProjection(decoded.Runtime);
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_timeout",
                "leserpentd runtime projection query timed out",
                error);
        }
        catch (DaemonRuntimeProjectionException)
        {
            throw;
        }
        catch (DaemonRuntimeRegistrationException error)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_unavailable",
                "leserpentd runtime projection transport failed",
                error);
        }
        catch (Exception error) when (
            error is JsonException
                or KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_invalid",
                "leserpentd returned an invalid runtime projection",
                error);
        }
    }

    private byte[] BuildProjectionQuery(Action<Utf8JsonWriter> writeQuery) =>
        BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "query");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer, "runtime.read");
            writer.WritePropertyName("query");
            writer.WriteStartObject();
            writeQuery(writer);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });

    private static JsonElement RequireProjectionResponse(JsonElement root)
    {
        var response = RequireProtocolResponse(root);
        if (!string.Equals(response.GetProperty("kind").GetString(), "query", StringComparison.Ordinal))
        {
            if (string.Equals(response.GetProperty("kind").GetString(), "error", StringComparison.Ordinal))
            {
                var error = response.GetProperty("payload");
                throw new DaemonRuntimeProjectionException(
                    error.GetProperty("code").GetString() ?? "daemon_projection_failed",
                    error.GetProperty("message").GetString() ?? "leserpentd rejected the runtime projection query");
            }
            throw new InvalidOperationException("leserpentd returned an unexpected projection response kind");
        }
        return response.GetProperty("payload");
    }

    private static JsonElement RequireProtocolResponse(JsonElement root)
    {
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unsupported protocol version");
        }
        return root.GetProperty("response");
    }

    private void EnsureProjectionEnabled()
    {
        if (!Enabled)
        {
            throw new InvalidOperationException("leserpentd runtime projection authority is not configured");
        }
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("leserpentd Unix socket projection is unavailable on Windows");
        }
    }

    private static string? NormalizeFilter(string? value) =>
        string.IsNullOrWhiteSpace(value) ? null : value.Trim();

    private static DaemonRuntimeProjection ConvertProjection(DaemonRuntimeProjectionPayload runtime)
    {
        if (!ValidProjectionRuntimeId(runtime.Id)
            || string.IsNullOrWhiteSpace(runtime.Name)
            || string.IsNullOrWhiteSpace(runtime.Endpoint)
            || runtime.Revision == 0
            || runtime.Tags is null
            || runtime.Status is null
            || runtime.Capabilities is null
            || runtime.Capabilities.Endpoints is null
            || runtime.Capabilities.Extensions is null
            || (runtime.SidecarStatus is not null && !runtime.SidecarStatus.IsValid())
            || !ValidOptionalProjectionValue(runtime.SidecarEndpoint)
            || !ValidAuthorityTimestamps(
                runtime.RegisteredAtUnixMs,
                runtime.UpdatedAtUnixMs))
        {
            throw new InvalidOperationException("leserpentd runtime projection is incomplete");
        }
        var capabilities = string.IsNullOrEmpty(runtime.Capabilities.Source)
            ? null
            : new RuntimeCapabilityAuthoritySnapshot(
                runtime.Capabilities.Source,
                runtime.Capabilities.Service,
                runtime.Capabilities.Version,
                runtime.Capabilities.LatestSnapshot,
                runtime.Capabilities.AuthenticatedDeployment,
                runtime.Capabilities.ServeRequired,
                runtime.Capabilities.ExternalSidecarContext,
                runtime.Capabilities.TargetPathSegmentEncoding,
                runtime.Capabilities.TargetDirectPathChars,
                runtime.Capabilities.Endpoints,
                runtime.Capabilities.Extensions);
        return new DaemonRuntimeProjection(
            runtime.Id,
            runtime.Name,
            runtime.Endpoint,
            runtime.SidecarEndpoint,
            ToDateTimeOffset(runtime.RegisteredAtUnixMs),
            ToDateTimeOffset(runtime.UpdatedAtUnixMs),
            runtime.Revision,
            new RuntimeTags(runtime.Tags.Environment, runtime.Tags.Cluster, runtime.Tags.Role),
            runtime.Status.ToLegacy(),
            capabilities,
            runtime.SidecarStatus?.ToLegacy());
    }

    private static bool ValidProjectionRuntimeId(string? value) =>
        !string.IsNullOrEmpty(value)
            && value.Length <= 128
            && value.All(character =>
                char.IsAsciiLetterOrDigit(character)
                    || character is '-' or '_' or ':' or '.');

    private static bool ValidOptionalProjectionValue(string? value) =>
        value is null
            || (!string.IsNullOrWhiteSpace(value)
                && value.Length <= 2048
                && value == value.Trim()
                && !value.Any(char.IsControl));

    private static bool ValidAuthorityTimestamps(ulong? registeredAt, ulong? updatedAt) =>
        (registeredAt is null or > 0)
            && (updatedAt is null or > 0)
            && (registeredAt is null || updatedAt is null || registeredAt <= updatedAt)
            && (registeredAt is null or <= 253_402_300_799_999)
            && (updatedAt is null or <= 253_402_300_799_999);

    private static DateTimeOffset? ToDateTimeOffset(ulong? value) =>
        value is null ? null : DateTimeOffset.FromUnixTimeMilliseconds((long)value.Value);
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeListPayload(
    [property: JsonPropertyName("kind")] string Kind,
    [property: JsonPropertyName("revision")] ulong Revision,
    [property: JsonPropertyName("runtimes")] DaemonRuntimeProjectionPayload[] Runtimes);

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeInspectPayload(
    [property: JsonPropertyName("kind")] string Kind,
    [property: JsonPropertyName("revision")] ulong Revision,
    [property: JsonPropertyName("runtime")] DaemonRuntimeProjectionPayload Runtime);

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeProjectionPayload(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("endpoint")] string Endpoint,
    [property: JsonPropertyName("sidecar_endpoint")] string? SidecarEndpoint,
    [property: JsonPropertyName("registered_at_unix_ms")] ulong? RegisteredAtUnixMs,
    [property: JsonPropertyName("updated_at_unix_ms")] ulong? UpdatedAtUnixMs,
    [property: JsonPropertyName("revision")] ulong Revision,
    [property: JsonPropertyName("refresh_count")] ulong RefreshCount,
    [property: JsonPropertyName("refresh_status")] string RefreshStatus,
    [property: JsonPropertyName("tags")] DaemonRuntimeTagsPayload Tags,
    [property: JsonPropertyName("status")] DaemonRuntimeStatusPayload Status,
    [property: JsonPropertyName("sidecar_status")] DaemonRuntimeSidecarStatusPayload? SidecarStatus,
    [property: JsonPropertyName("capabilities")] DaemonRuntimeCapabilityPayload Capabilities,
    [property: JsonPropertyName("capabilities_observed_for_revision")] ulong? CapabilitiesObservedForRevision);

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeTagsPayload(
    [property: JsonPropertyName("environment")] string? Environment,
    [property: JsonPropertyName("cluster")] string? Cluster,
    [property: JsonPropertyName("role")] string? Role);

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeCapabilityPayload(
    [property: JsonPropertyName("source")] string Source,
    [property: JsonPropertyName("service")] string Service,
    [property: JsonPropertyName("version")] string Version,
    [property: JsonPropertyName("latest_snapshot")] bool LatestSnapshot,
    [property: JsonPropertyName("authenticated_deployment")] bool AuthenticatedDeployment,
    [property: JsonPropertyName("serve_required")] bool ServeRequired,
    [property: JsonPropertyName("external_sidecar_context")] bool ExternalSidecarContext,
    [property: JsonPropertyName("target_path_segment_encoding")] string TargetPathSegmentEncoding,
    [property: JsonPropertyName("target_direct_path_chars")] string TargetDirectPathChars,
    [property: JsonPropertyName("endpoints")] string[] Endpoints,
    [property: JsonPropertyName("extensions")] Dictionary<string, bool> Extensions);

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeStatusPayload(
    [property: JsonPropertyName("status_source")] string StatusSource,
    [property: JsonPropertyName("status_fetched_at")] DateTimeOffset? StatusFetchedAt,
    [property: JsonPropertyName("status_fetch_error")] string? StatusFetchError,
    [property: JsonPropertyName("has_latest_snapshot")] bool HasLatestSnapshot,
    [property: JsonPropertyName("snapshot_kind")] string? SnapshotKind,
    [property: JsonPropertyName("target_count")] ulong? TargetCount,
    [property: JsonPropertyName("has_summary_json")] bool HasSummaryJson,
    [property: JsonPropertyName("has_analysis_json")] bool HasAnalysisJson,
    [property: JsonPropertyName("has_training_example_json")] bool HasTrainingExampleJson,
    [property: JsonPropertyName("has_training_dataset_manifest")] bool HasTrainingDatasetManifest,
    [property: JsonPropertyName("has_export_json")] bool HasExportJson,
    [property: JsonPropertyName("has_report_json")] bool HasReportJson,
    [property: JsonPropertyName("has_report_html")] bool HasReportHtml,
    [property: JsonPropertyName("has_external_sidecar_context")] bool HasExternalSidecarContext,
    [property: JsonPropertyName("has_external_evidence_chain_enrichment")] bool HasExternalEvidenceChainEnrichment,
    [property: JsonPropertyName("has_external_diagnostic_opinion")] bool HasExternalDiagnosticOpinion,
    [property: JsonPropertyName("resilience_degraded")] bool ResilienceDegraded,
    [property: JsonPropertyName("resilience_status")] string? ResilienceStatus,
    [property: JsonPropertyName("resilience_summary")] string? ResilienceSummary,
    [property: JsonPropertyName("socket_service_status")] string? SocketServiceStatus,
    [property: JsonPropertyName("socket_consecutive_idle_timeouts")] ulong? SocketConsecutiveIdleTimeouts,
    [property: JsonPropertyName("socket_total_idle_timeouts")] ulong? SocketTotalIdleTimeouts)
{
    public RuntimeStatusSnapshot ToLegacy() =>
        new(
            StatusSource,
            StatusFetchedAt,
            StatusFetchError,
            HasLatestSnapshot,
            SnapshotKind,
            TargetCount is null ? null : checked((int)TargetCount.Value),
            HasSummaryJson,
            HasAnalysisJson,
            HasTrainingExampleJson,
            HasTrainingDatasetManifest,
            HasExportJson,
            HasReportJson,
            HasReportHtml,
            HasExternalSidecarContext,
            HasExternalEvidenceChainEnrichment,
            HasExternalDiagnosticOpinion,
            ResilienceDegraded,
            ResilienceStatus,
            ResilienceSummary,
            SocketServiceStatus,
            SocketConsecutiveIdleTimeouts is null ? null : checked((int)SocketConsecutiveIdleTimeouts.Value),
            SocketTotalIdleTimeouts is null ? null : checked((int)SocketTotalIdleTimeouts.Value));
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeSidecarMemorySlotPayload(
    [property: JsonPropertyName("slot")] string Slot,
    [property: JsonPropertyName("label")] string? Label,
    [property: JsonPropertyName("note")] string? Note,
    [property: JsonPropertyName("source")] string Source,
    [property: JsonPropertyName("saved_at")] DateTimeOffset? SavedAt,
    [property: JsonPropertyName("pattern_count")] ulong PatternCount,
    [property: JsonPropertyName("label_count")] ulong LabelCount)
{
    public bool IsValid() =>
        ValidValue(Slot, 128)
            && ValidOptionalValue(Label, 256)
            && ValidOptionalValue(Note, 1024)
            && ValidValue(Source, 128)
            && PatternCount <= 10_000_000
            && LabelCount <= 10_000_000;

    public RuntimeSidecarMemorySlotSummary ToLegacy() =>
        new(
            Slot,
            Label,
            Note,
            Source,
            SavedAt,
            checked((int)PatternCount),
            checked((int)LabelCount));

    private static bool ValidValue(string? value, int maximum) =>
        value is not null
            && value.Length is > 0
            && value.Length <= maximum
            && value == value.Trim()
            && !value.Any(char.IsControl);

    private static bool ValidOptionalValue(string? value, int maximum) =>
        value is null || ValidValue(value, maximum);
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeSidecarMemoryPayload(
    [property: JsonPropertyName("versions_supported")] bool VersionsSupported,
    [property: JsonPropertyName("slot_count")] ulong SlotCount,
    [property: JsonPropertyName("history_count")] ulong HistoryCount,
    [property: JsonPropertyName("latest_slot")] string? LatestSlot,
    [property: JsonPropertyName("latest_label")] string? LatestLabel,
    [property: JsonPropertyName("latest_source")] string? LatestSource,
    [property: JsonPropertyName("slots")] DaemonRuntimeSidecarMemorySlotPayload[] Slots,
    [property: JsonPropertyName("fetch_error")] string? FetchError)
{
    public bool IsValid() =>
        SlotCount <= 10_000
            && HistoryCount <= 1_000_000
            && Slots is not null
            && Slots.Length <= 128
            && ValidOptionalValue(LatestSlot, 128)
            && ValidOptionalValue(LatestLabel, 256)
            && ValidOptionalValue(LatestSource, 128)
            && FetchError is null or "sidecar_memory_fetch_failed"
            && Slots.All(slot => slot is not null && slot.IsValid());

    public RuntimeSidecarMemorySnapshot ToLegacy() =>
        new(
            VersionsSupported,
            checked((int)SlotCount),
            checked((int)HistoryCount),
            LatestSlot,
            LatestLabel,
            LatestSource,
            Slots.Select(slot => slot.ToLegacy()).ToArray(),
            FetchError);

    private static bool ValidOptionalValue(string? value, int maximum) =>
        value is null
            || (value.Length is > 0
                && value.Length <= maximum
                && value == value.Trim()
                && !value.Any(char.IsControl));
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
internal sealed record DaemonRuntimeSidecarStatusPayload(
    [property: JsonPropertyName("status_source")] string StatusSource,
    [property: JsonPropertyName("status_fetched_at")] DateTimeOffset? StatusFetchedAt,
    [property: JsonPropertyName("status_fetch_error")] string? StatusFetchError,
    [property: JsonPropertyName("healthy")] bool Healthy,
    [property: JsonPropertyName("daemon_status")] string DaemonStatus,
    [property: JsonPropertyName("target_count")] ulong? TargetCount,
    [property: JsonPropertyName("learning_active")] bool LearningActive,
    [property: JsonPropertyName("learned_routes")] ulong LearnedRoutes,
    [property: JsonPropertyName("has_evidence_chain_enrichment")] bool HasEvidenceChainEnrichment,
    [property: JsonPropertyName("has_diagnostic_opinion")] bool HasDiagnosticOpinion,
    [property: JsonPropertyName("last_error")] string? LastError,
    [property: JsonPropertyName("memory")] DaemonRuntimeSidecarMemoryPayload? Memory)
{
    public bool IsValid()
    {
        var posture = StatusSource == "etragon-api"
            ? StatusFetchedAt is not null
                && StatusFetchError is null
                && (LastError is null or "sidecar_reported_error")
            : StatusSource == "fetch_failed"
                && StatusFetchedAt is null
                && StatusFetchError == "sidecar_fetch_failed"
                && (LastError is null or "sidecar_fetch_failed")
                && !Healthy;
        return posture
            && ValidValue(DaemonStatus, 128)
            && ValidOptionalValue(StatusFetchError, 128)
            && ValidOptionalValue(LastError, 128)
            && TargetCount is null or <= 10_000_000
            && LearnedRoutes <= 10_000_000
            && (Memory is null || Memory.IsValid());
    }

    public RuntimeSidecarStatusSnapshot ToLegacy() =>
        new(
            StatusSource,
            StatusFetchedAt,
            StatusFetchError,
            Healthy,
            DaemonStatus,
            TargetCount is null ? null : checked((int)TargetCount.Value),
            LearningActive,
            checked((int)LearnedRoutes),
            HasEvidenceChainEnrichment,
            HasDiagnosticOpinion,
            LastError,
            Memory?.ToLegacy());

    private static bool ValidValue(string? value, int maximum) =>
        value is not null
            && value.Length is > 0
            && value.Length <= maximum
            && value == value.Trim()
            && !value.Any(char.IsControl);

    private static bool ValidOptionalValue(string? value, int maximum) =>
        value is null || ValidValue(value, maximum);
}

[JsonSourceGenerationOptions(GenerationMode = JsonSourceGenerationMode.Metadata)]
[JsonSerializable(typeof(DaemonRuntimeListPayload))]
[JsonSerializable(typeof(DaemonRuntimeInspectPayload))]
internal sealed partial class DaemonRuntimeProjectionJsonContext : JsonSerializerContext;
