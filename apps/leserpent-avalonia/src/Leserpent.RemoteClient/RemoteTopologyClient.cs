using System.Text.Json;
using System.Text.Json.Serialization;

public sealed record RemoteTopologySnapshot(
    ulong Revision,
    IReadOnlyList<RemoteRuntimeProjection> Runtimes,
    bool IsStale = false);

public sealed class RemoteTopologyClient : IDisposable
{
    public const int MaxRuntimes = 4096;
    private readonly RemoteWireTransport transport;

    public RemoteTopologyClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteTopologySnapshot> LoadAsync(
        string principal,
        CancellationToken cancellationToken = default)
    {
        RemoteQueryValidation.RequireIdentifier(principal, "principal");
        var envelope = new WireRuntimeListRequestEnvelope
        {
            Request = new WireRuntimeListRequest
            {
                Payload = new RuntimeListQueryEnvelope
                {
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = ["runtime.read"],
                    Query = new RuntimeListQuery
                    {
                        Filter = new RuntimeListFilter(),
                    },
                },
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteTopologyJsonContext.Default.WireRuntimeListRequestEnvelope);
        var response = await transport.PostAsync(
            payload,
            "runtime_list",
            cancellationToken).ConfigureAwait(false);
        return RemoteTopologyCodec.Decode(response);
    }

    public void Dispose() => transport.Dispose();
}

public static class RemoteTopologyCodec
{
    public static RemoteTopologySnapshot FromCache(RemoteSnapshotCache cache)
    {
        if (cache.Runtimes is null || cache.Runtimes.Count > RemoteTopologyClient.MaxRuntimes)
        {
            throw new InvalidDataException("cached remote topology exceeds the runtime limit");
        }
        var ids = new HashSet<string>(StringComparer.Ordinal);
        foreach (var runtime in cache.Runtimes)
        {
            if (runtime is null
                || runtime.Id is null
                || runtime.Name is null
                || runtime.Tags is null
                || runtime.Status?.StatusSource is null)
            {
                throw new InvalidDataException("cached remote topology is incomplete");
            }
            try
            {
                RemoteQueryValidation.RequireIdentifier(runtime.Id, "runtime ID");
            }
            catch (ArgumentException error)
            {
                throw new InvalidDataException("cached remote topology runtime is invalid", error);
            }
            RemoteQueryValidation.RequireDisplay(runtime.Name, "runtime name");
            RemoteQueryValidation.RequireDisplay(runtime.Status.StatusSource, "status source");
            RemoteEventCodec.ValidateCapabilities(
                runtime.Capabilities,
                runtime.CapabilitiesObservedForRevision,
                runtime.Revision);
            if (!ids.Add(runtime.Id) || runtime.Revision > cache.Revision)
            {
                throw new InvalidDataException(
                    "cached remote topology contains an invalid identity or revision");
            }
        }
        return new RemoteTopologySnapshot(cache.Revision, cache.Runtimes, true);
    }

    public static RemoteTopologySnapshot Decode(ReadOnlySpan<byte> payload)
    {
        if (payload.Length > RemoteEventCodec.MaxMessageBytes)
        {
            throw new InvalidDataException("remote topology response exceeds the message limit");
        }
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteTopologyJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException("remote topology response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported remote topology response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw new RemoteQueryException(
                    RequiredString(envelope.Response.Payload, "code"),
                    RequiredString(envelope.Response.Payload, "message"));
            }
            if (envelope.Response.Kind != "query")
            {
                throw new InvalidDataException(
                    "remote topology returned an unexpected response kind");
            }
            var result = envelope.Response.Payload.Deserialize(
                RemoteTopologyJsonContext.Default.RuntimeListQueryResult)
                ?? throw new InvalidDataException("remote topology query result is empty");
            if (result.Kind != "runtime_list" || result.Runtimes is null)
            {
                throw new InvalidDataException(
                    "remote topology returned an unexpected query kind");
            }
            if (result.Runtimes.Count > RemoteTopologyClient.MaxRuntimes)
            {
                throw new InvalidDataException("remote topology exceeds the runtime limit");
            }
            var ids = new HashSet<string>(StringComparer.Ordinal);
            var runtimes = new List<RemoteRuntimeProjection>(result.Runtimes.Count);
            foreach (var runtime in result.Runtimes)
            {
                if (runtime is null)
                {
                    throw new InvalidDataException(
                        "remote topology contains an empty runtime");
                }
                RemoteWorkspaceCodec.ValidateRuntime(runtime);
                if (!ids.Add(runtime.Id) || runtime.Revision > result.Revision)
                {
                    throw new InvalidDataException(
                        "remote topology contains an invalid runtime identity or revision");
                }
                runtimes.Add(RemoteWorkspaceCodec.Project(runtime));
            }
            return new RemoteTopologySnapshot(result.Revision, runtimes);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("remote topology response JSON is invalid", error);
        }
        catch (KeyNotFoundException error)
        {
            throw new InvalidDataException(
                "remote topology response is missing a required field",
                error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException(
                "remote topology response has an invalid field type",
                error);
        }
        catch (ArgumentException error)
        {
            throw new InvalidDataException("remote topology runtime is invalid", error);
        }
    }

    public static void VerifyContract()
    {
        var request = new WireRuntimeListRequestEnvelope
        {
            Request = new WireRuntimeListRequest
            {
                Payload = new RuntimeListQueryEnvelope
                {
                    Principal = new RemotePrincipal { Id = "avalonia-hub" },
                    Capabilities = ["runtime.read"],
                    Query = new RuntimeListQuery { Filter = new RuntimeListFilter() },
                },
            },
        };
        var encoded = JsonSerializer.Serialize(
            request,
            RemoteTopologyJsonContext.Default.WireRuntimeListRequestEnvelope);
        if (!encoded.Contains("\"kind\":\"runtime_list\"", StringComparison.Ordinal)
            || !encoded.Contains("\"capabilities\":[\"runtime.read\"]", StringComparison.Ordinal)
            || encoded.Contains("token", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException("remote topology request contract drifted");
        }

        const string response = """
            {"schema_version":1,"response":{"kind":"query","payload":{"kind":"runtime_list","revision":9,"runtimes":[{"id":"runtime-a","name":"Runtime A","endpoint":"https://runtime.internal:9411/","revision":8,"refresh_count":2,"refresh_status":"ready","tags":{"environment":"production","cluster":null,"role":null},"status":{"status_source":"gewyvern","status_fetched_at":null,"status_fetch_error":null,"has_latest_snapshot":true,"snapshot_kind":null,"target_count":null,"has_summary_json":false,"has_analysis_json":false,"has_training_example_json":false,"has_training_dataset_manifest":false,"has_export_json":false,"has_report_json":false,"has_report_html":false,"has_external_sidecar_context":false,"has_external_evidence_chain_enrichment":false,"has_external_diagnostic_opinion":false,"resilience_degraded":false,"resilience_status":null,"resilience_summary":null,"socket_service_status":null,"socket_consecutive_idle_timeouts":null,"socket_total_idle_timeouts":null},"capabilities":null,"capabilities_observed_for_revision":null}]}}}
            """;
        var snapshot = Decode(System.Text.Encoding.UTF8.GetBytes(response));
        if (snapshot.Revision != 9
            || snapshot.IsStale
            || snapshot.Runtimes is not [{ Id: "runtime-a", Name: "Runtime A" }])
        {
            throw new InvalidDataException("remote topology response projection drifted");
        }
        var cached = FromCache(new RemoteSnapshotCache
        {
            SchemaVersion = 1,
            EndpointHash = new string('a', 64),
            Revision = snapshot.Revision,
            Runtimes = snapshot.Runtimes.ToList(),
        });
        if (!cached.IsStale || cached.Runtimes.Count != 1)
        {
            throw new InvalidDataException("cached remote topology projection drifted");
        }
        var projected = JsonSerializer.Serialize(
            snapshot.Runtimes[0],
            RemoteEventJsonContext.Default.RemoteRuntimeProjection);
        if (projected.Contains("runtime.internal", StringComparison.Ordinal))
        {
            throw new InvalidDataException("remote topology retained a runtime endpoint");
        }
        const string nullRuntime = """
            {"schema_version":1,"response":{"kind":"query","payload":{"kind":"runtime_list","revision":9,"runtimes":[null]}}}
            """;
        ExpectInvalidData(
            () => Decode(System.Text.Encoding.UTF8.GetBytes(nullRuntime)),
            "remote topology accepted an empty runtime");
        const string errorResponse = """
            {"schema_version":1,"response":{"kind":"error","payload":{"code":"unauthorized","message":"runtime.read is required"}}}
            """;
        try
        {
            _ = Decode(System.Text.Encoding.UTF8.GetBytes(errorResponse));
        }
        catch (RemoteQueryException error) when (error.Code == "unauthorized")
        {
            return;
        }
        throw new InvalidDataException("remote topology did not preserve a typed query error");
    }

    private static string RequiredString(JsonElement element, string property)
    {
        var value = element.GetProperty(property).GetString();
        if (string.IsNullOrWhiteSpace(value)
            || value.Length > 4096
            || value.Any(char.IsControl))
        {
            throw new InvalidDataException(
                $"remote topology response field '{property}' is invalid");
        }
        return value;
    }

    private static void ExpectInvalidData(Action action, string failure)
    {
        try
        {
            action();
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException(failure);
    }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRuntimeListRequestEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required WireRuntimeListRequest Request { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class WireRuntimeListRequest
{
    public string Kind { get; set; } = "query";
    public required RuntimeListQueryEnvelope Payload { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeListQueryEnvelope
{
    public int SchemaVersion { get; set; } = 1;
    public required RemotePrincipal Principal { get; set; }
    public required List<string> Capabilities { get; set; }
    public required RuntimeListQuery Query { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeListQuery
{
    public string Kind { get; set; } = "runtime_list";
    public required RuntimeListFilter Filter { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeListFilter
{
    public string? Environment { get; set; }
    public string? Cluster { get; set; }
    public string? Role { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class RuntimeListQueryResult
{
    public required string Kind { get; set; }
    public ulong Revision { get; set; }
    public required List<WireRuntimeProjection> Runtimes { get; set; }
}

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(WireRuntimeListRequestEnvelope))]
[JsonSerializable(typeof(WireResponseEnvelope))]
[JsonSerializable(typeof(RuntimeListQueryResult))]
public partial class RemoteTopologyJsonContext : JsonSerializerContext;
