using System.Text.Json;
using System.Text.Json.Serialization;

public static class RemoteEventCodec
{
    public const int SchemaVersion = 1;
    public const int MaxMessageBytes = 1024 * 1024;

    public static RemoteEvent Decode(ReadOnlySpan<byte> payload)
    {
        if (payload.Length > MaxMessageBytes)
        {
            throw new InvalidDataException("remote event exceeds the message limit");
        }
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteEventJsonContext.Default.RemoteEventEnvelope)
                ?? throw new InvalidDataException("remote event is empty");
            if (envelope.SchemaVersion != SchemaVersion)
            {
                throw new InvalidDataException("unsupported remote event schema");
            }
            return envelope.Event.Kind switch
            {
                "runtime_snapshot" => DeserializePayload(
                    envelope.Event.Payload,
                    RemoteEventJsonContext.Default.RuntimeSnapshotPayload,
                    ValidateSnapshot),
                "heartbeat" => DeserializePayload(
                    envelope.Event.Payload,
                    RemoteEventJsonContext.Default.HeartbeatPayload,
                    eventPayload => new RemoteEvent.Heartbeat(eventPayload.Revision)),
                "resync_required" => DeserializePayload(
                    envelope.Event.Payload,
                    RemoteEventJsonContext.Default.ResyncRequiredPayload,
                    eventPayload => new RemoteEvent.ResyncRequired(
                        eventPayload.RequestedAfter,
                        eventPayload.CurrentRevision)),
                _ => throw new InvalidDataException("unknown remote event kind"),
            };
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("remote event JSON is invalid", error);
        }
    }

    private static RemoteEvent DeserializePayload<TPayload>(
        JsonElement payload,
        System.Text.Json.Serialization.Metadata.JsonTypeInfo<TPayload> typeInfo,
        Func<TPayload, RemoteEvent> project)
        where TPayload : class
    {
        var value = payload.Deserialize(typeInfo)
            ?? throw new InvalidDataException("remote event payload is empty");
        return project(value);
    }

    private static RemoteEvent ValidateSnapshot(RuntimeSnapshotPayload payload)
    {
        if (payload.Runtimes.Count > 4096)
        {
            throw new InvalidDataException("remote snapshot exceeds the runtime limit");
        }
        var ids = new HashSet<string>(StringComparer.Ordinal);
        foreach (var runtime in payload.Runtimes)
        {
            if (!IsIdentifier(runtime.Id) || !ids.Add(runtime.Id))
            {
                throw new InvalidDataException("remote snapshot contains an invalid runtime ID");
            }
            ValidateCapabilities(
                runtime.Capabilities,
                runtime.CapabilitiesObservedForRevision,
                runtime.Revision);
        }
        return new RemoteEvent.Snapshot(
            payload.Revision,
            payload.ResumedAfter,
            payload.Runtimes);
    }

    private static bool IsIdentifier(string value) => value.Length is > 0 and <= 128
        && value.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '-' or '_' or '.' or ':');

    internal static void ValidateCapabilities(
        RuntimeCapabilitySnapshot? capabilities,
        ulong? observedForRevision = null,
        ulong? runtimeRevision = null)
    {
        if (capabilities is null)
        {
            if (observedForRevision is not null)
            {
                throw new InvalidDataException(
                    "missing runtime capabilities carry an observation revision");
            }
            return;
        }
        if (capabilities.Source is null
            || capabilities.Service is null
            || capabilities.Version is null
            || capabilities.TargetPathSegmentEncoding is null
            || capabilities.TargetDirectPathChars is null
            || capabilities.Endpoints is null
            || capabilities.Extensions is null)
        {
            throw new InvalidDataException(
                "remote snapshot contains incomplete runtime capabilities");
        }
        if (capabilities.IsUnobserved)
        {
            if (observedForRevision is not null)
            {
                throw new InvalidDataException(
                    "unobserved runtime capabilities carry an observation revision");
            }
            return;
        }
        if (observedForRevision is { } observed
            && (runtimeRevision is not { } current || observed >= current))
        {
            throw new InvalidDataException(
                "runtime capability observation revision is invalid");
        }
        var endpointsValid = capabilities.Endpoints.Count is > 0 and <= 128
            && capabilities.Endpoints.SequenceEqual(
                capabilities.Endpoints.Distinct(StringComparer.Ordinal)
                    .Order(StringComparer.Ordinal))
            && capabilities.Endpoints.All(endpoint => endpoint.Length <= 256
                && endpoint.StartsWith('/')
                && !endpoint.StartsWith("//", StringComparison.Ordinal)
                && !endpoint.Contains('?')
                && !endpoint.Contains('#')
                && endpoint.All(character => char.IsAscii(character)
                    && !char.IsControl(character)
                    && !char.IsWhiteSpace(character)));
        var extensionsValid = capabilities.Extensions.Count <= 64
            && capabilities.Extensions.Keys.All(key => key.Length is > 0 and <= 64
                && key.All(character => character is >= 'a' and <= 'z'
                    || char.IsAsciiDigit(character)
                    || character is '_' or '-'));
        var versionValid = capabilities.Version.Length is > 0 and <= 32
            && capabilities.Version.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '-' or '+');
        var directCharsValid = capabilities.TargetDirectPathChars.Length is > 0 and <= 64
            && capabilities.TargetDirectPathChars.All(character => char.IsAscii(character)
                && !char.IsControl(character));
        if (capabilities.Source != "gewyvern-api"
            || capabilities.Service != "gewyvern-api"
            || !versionValid
            || capabilities.TargetPathSegmentEncoding != "percent-encoding"
            || !directCharsValid
            || !endpointsValid
            || !extensionsValid
            || !capabilities.Endpoints.Contains("/v1/capabilities", StringComparer.Ordinal)
            || capabilities.AuthenticatedDeployment
                != capabilities.Endpoints.Contains("/v1/deployments", StringComparer.Ordinal))
        {
            throw new InvalidDataException(
                "remote snapshot contains invalid runtime capabilities");
        }
    }
}

public abstract record RemoteEvent
{
    public sealed record Snapshot(
        ulong Revision,
        ulong? ResumedAfter,
        IReadOnlyList<RemoteRuntimeProjection> Runtimes) : RemoteEvent;

    public sealed record Heartbeat(ulong Revision) : RemoteEvent;

    public sealed record ResyncRequired(
        ulong RequestedAfter,
        ulong CurrentRevision) : RemoteEvent;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteEventEnvelope
{
    public int SchemaVersion { get; set; }
    public required RemoteEventUnion Event { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteEventUnion
{
    public required string Kind { get; set; }
    public JsonElement Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeSnapshotPayload
{
    public ulong Revision { get; set; }
    public ulong? ResumedAfter { get; set; }
    public required List<RemoteRuntimeProjection> Runtimes { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class HeartbeatPayload
{
    public ulong Revision { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class ResyncRequiredPayload
{
    public ulong RequestedAfter { get; set; }
    public ulong CurrentRevision { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteSnapshotCache
{
    public int SchemaVersion { get; set; }
    public required string EndpointHash { get; set; }
    public ulong Revision { get; set; }
    public required List<RemoteRuntimeProjection> Runtimes { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RemoteRuntimeProjection
{
    public required string Id { get; set; }
    public required string Name { get; set; }
    public ulong Revision { get; set; }
    public ulong RefreshCount { get; set; }
    public RefreshStatus RefreshStatus { get; set; }
    public required RuntimeTags Tags { get; set; }
    public required RuntimeStatusSnapshot Status { get; set; }
    public RuntimeCapabilitySnapshot? Capabilities { get; set; }
    public ulong? CapabilitiesObservedForRevision { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeCapabilitySnapshot
{
    public string Source { get; set; } = string.Empty;
    public string Service { get; set; } = string.Empty;
    public string Version { get; set; } = string.Empty;
    public bool LatestSnapshot { get; set; }
    public bool AuthenticatedDeployment { get; set; }
    public bool ServeRequired { get; set; }
    public bool ExternalSidecarContext { get; set; }
    public string TargetPathSegmentEncoding { get; set; } = string.Empty;
    public string TargetDirectPathChars { get; set; } = string.Empty;
    public List<string> Endpoints { get; set; } = [];
    public Dictionary<string, bool> Extensions { get; set; } = new(StringComparer.Ordinal);

    [JsonIgnore]
    public bool IsUnobserved => Source.Length == 0
        && Service.Length == 0
        && Version.Length == 0
        && !LatestSnapshot
        && !AuthenticatedDeployment
        && !ServeRequired
        && !ExternalSidecarContext
        && TargetPathSegmentEncoding.Length == 0
        && TargetDirectPathChars.Length == 0
        && Endpoints.Count == 0
        && Extensions.Count == 0;
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeTags
{
    public string? Environment { get; set; }
    public string? Cluster { get; set; }
    public string? Role { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeStatusSnapshot
{
    public required string StatusSource { get; set; }
    public string? StatusFetchedAt { get; set; }
    public string? StatusFetchError { get; set; }
    public bool HasLatestSnapshot { get; set; }
    public string? SnapshotKind { get; set; }
    public ulong? TargetCount { get; set; }
    public bool HasSummaryJson { get; set; }
    public bool HasAnalysisJson { get; set; }
    public bool HasTrainingExampleJson { get; set; }
    public bool HasTrainingDatasetManifest { get; set; }
    public bool HasExportJson { get; set; }
    public bool HasReportJson { get; set; }
    public bool HasReportHtml { get; set; }
    public bool HasExternalSidecarContext { get; set; }
    public bool HasExternalEvidenceChainEnrichment { get; set; }
    public bool HasExternalDiagnosticOpinion { get; set; }
    public bool ResilienceDegraded { get; set; }
    public string? ResilienceStatus { get; set; }
    public string? ResilienceSummary { get; set; }
    public string? SocketServiceStatus { get; set; }
    public ulong? SocketConsecutiveIdleTimeouts { get; set; }
    public ulong? SocketTotalIdleTimeouts { get; set; }
}

[JsonConverter(typeof(JsonStringEnumConverter<RefreshStatus>))]
public enum RefreshStatus
{
    [JsonStringEnumMemberName("never_requested")] NeverRequested,
    [JsonStringEnumMemberName("pending")] Pending,
    [JsonStringEnumMemberName("ready")] Ready,
    [JsonStringEnumMemberName("failed")] Failed,
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(RemoteEventEnvelope))]
[JsonSerializable(typeof(RuntimeSnapshotPayload))]
[JsonSerializable(typeof(HeartbeatPayload))]
[JsonSerializable(typeof(ResyncRequiredPayload))]
[JsonSerializable(typeof(RemoteSnapshotCache))]
public partial class RemoteEventJsonContext : JsonSerializerContext;
