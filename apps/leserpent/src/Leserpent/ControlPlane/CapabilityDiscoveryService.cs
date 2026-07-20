using System.Net.Http.Json;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;

namespace Leserpent.ControlPlane;

public sealed partial class CapabilityDiscoveryService(HttpClient httpClient, ControlPlaneSecurityPolicy securityPolicy)
{
    private const string EtragonAdminTokenHeader = "X-Etragon-Admin-Token";
    public const string GewyvernAdminTokenHeader = "X-Gewyvern-Admin-Token";

    public async Task<CapabilityDiscoveryResult> DiscoverAsync(string endpoint, string? capabilityEndpoint, CancellationToken cancellationToken, string? runtimeAdminToken = null)
    {
        var capabilityUrl = BuildCapabilityUrl(endpoint, capabilityEndpoint);
        var capabilityPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(capabilityUrl, "capability endpoint", cancellationToken);
        if (capabilityPlanResult.Error is not null)
        {
            return CapabilityDiscoveryResult.Failed(capabilityUrl, capabilityPlanResult.Error);
        }
        var capabilityPlan = capabilityPlanResult.Plan!;

        try
        {
            var discoveryPayload = await GetCapabilityPayloadAsync(
                capabilityPlan,
                cancellationToken,
                runtimeAdminToken,
                GewyvernAdminTokenHeader);
            if (discoveryPayload is null)
            {
                return CapabilityDiscoveryResult.Failed(capabilityUrl, "failed to decode gewyvern capability payload");
            }
            var payload = discoveryPayload.Payload;

            var endpointSet = payload.Endpoints ?? Array.Empty<string>();
            var authoritySnapshot = new RuntimeCapabilityAuthoritySnapshot(
                "gewyvern-api",
                payload.Service,
                payload.Version,
                payload.LatestSnapshot,
                payload.AuthenticatedDeployment,
                payload.ServeRequired,
                payload.ExternalSidecarContext,
                payload.TargetPathSegmentEncoding,
                payload.TargetDirectPathChars,
                endpointSet.Distinct(StringComparer.Ordinal).OrderBy(value => value, StringComparer.Ordinal).ToArray(),
                discoveryPayload.Extensions);

            return CapabilityDiscoveryResult.Succeeded(
                capabilityUrl,
                RuntimeCapabilityProjection.ToLegacy(authoritySnapshot),
                authoritySnapshot);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex)
        {
            return CapabilityDiscoveryResult.Failed(capabilityUrl, ex.Message);
        }
    }

    public async Task<RuntimeStatusDiscoveryResult> DiscoverStatusAsync(string endpoint, string? statusEndpoint, CancellationToken cancellationToken, string? runtimeAdminToken = null)
    {
        var statusUrl = BuildStatusUrl(endpoint, statusEndpoint);
        var statusPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(statusUrl, "status endpoint", cancellationToken);
        if (statusPlanResult.Error is not null)
        {
            return RuntimeStatusDiscoveryResult.Failed(statusUrl, statusPlanResult.Error);
        }
        var statusPlan = statusPlanResult.Plan!;

        try
        {
            var payload = await GetFromJsonAsync(
                statusPlan,
                DiscoveryJsonContext.Default.GewyvernLatestMetaPayload,
                cancellationToken,
                runtimeAdminToken,
                GewyvernAdminTokenHeader);
            if (payload is null)
            {
                return RuntimeStatusDiscoveryResult.Failed(statusUrl, "failed to decode gewyvern latest-meta payload");
            }

            var resilience = await TryDiscoverResilienceAsync(endpoint, cancellationToken, runtimeAdminToken);

            return RuntimeStatusDiscoveryResult.Succeeded(
                statusUrl,
                new RuntimeStatusSnapshot(
                    "gewyvern-api",
                    DateTimeOffset.UtcNow,
                    null,
                    payload.UpdatedUnixMs > 0 && !string.IsNullOrWhiteSpace(payload.Kind),
                    string.IsNullOrWhiteSpace(payload.Kind) ? null : payload.Kind,
                    payload.TargetCount,
                    payload.HasSummaryJson,
                    payload.HasAnalysisJson,
                    payload.HasTrainingExampleJson,
                    payload.HasTrainingExampleJson,
                    payload.HasExportJson,
                    payload.HasReportJson,
                    payload.HasReportHtml,
                    payload.HasExternalSidecarContext,
                    payload.HasExternalEvidenceChainEnrichment,
                    payload.HasExternalDiagnosticOpinion,
                    resilience?.Degraded ?? false,
                    string.IsNullOrWhiteSpace(resilience?.Status) ? null : resilience!.Status!.Trim(),
                    string.IsNullOrWhiteSpace(resilience?.Summary) ? null : resilience!.Summary!.Trim(),
                    string.IsNullOrWhiteSpace(resilience?.SocketService?.Status) ? null : resilience!.SocketService!.Status!.Trim(),
                    resilience?.SocketService?.ConsecutiveIdleTimeouts,
                    resilience?.SocketService?.TotalIdleTimeouts));
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex)
        {
            return RuntimeStatusDiscoveryResult.Failed(statusUrl, ex.Message);
        }
    }

    public async Task<RuntimeSidecarDiscoveryResult> DiscoverSidecarStatusAsync(
        string sidecarEndpoint,
        string? sidecarStatusEndpoint,
        string? sidecarAdminToken,
        CancellationToken cancellationToken)
    {
        var healthUrl = BuildSidecarHealthUrl(sidecarEndpoint);
        var statusUrl = BuildSidecarStatusUrl(sidecarEndpoint, sidecarStatusEndpoint);
        var enrichmentUrl = BuildSidecarEnrichmentUrl(sidecarEndpoint);
        var opinionUrl = BuildSidecarOpinionUrl(sidecarEndpoint);
        var memoryVersionsUrl = BuildSidecarMemoryVersionsUrl(sidecarEndpoint);

        var healthPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(healthUrl, "sidecar health endpoint", cancellationToken);
        if (healthPlanResult.Error is not null)
        {
            return RuntimeSidecarDiscoveryResult.Failed(statusUrl, healthPlanResult.Error);
        }
        var healthPlan = healthPlanResult.Plan!;

        var statusPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(statusUrl, "sidecar status endpoint", cancellationToken);
        if (statusPlanResult.Error is not null)
        {
            return RuntimeSidecarDiscoveryResult.Failed(statusUrl, statusPlanResult.Error);
        }
        var statusPlan = statusPlanResult.Plan!;

        var enrichmentPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(enrichmentUrl, "sidecar enrichment endpoint", cancellationToken);
        if (enrichmentPlanResult.Error is not null)
        {
            return RuntimeSidecarDiscoveryResult.Failed(statusUrl, enrichmentPlanResult.Error);
        }
        var enrichmentPlan = enrichmentPlanResult.Plan!;

        var opinionPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(opinionUrl, "sidecar opinion endpoint", cancellationToken);
        if (opinionPlanResult.Error is not null)
        {
            return RuntimeSidecarDiscoveryResult.Failed(statusUrl, opinionPlanResult.Error);
        }
        var opinionPlan = opinionPlanResult.Plan!;
        var memoryVersionsPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(memoryVersionsUrl, "sidecar memory versions endpoint", cancellationToken);
        if (memoryVersionsPlanResult.Error is not null)
        {
            return RuntimeSidecarDiscoveryResult.Failed(statusUrl, memoryVersionsPlanResult.Error);
        }
        var memoryVersionsPlan = memoryVersionsPlanResult.Plan!;

        try
        {
            var healthPayload = await GetFromJsonAsync(
                healthPlan,
                DiscoveryJsonContext.Default.EtragonHealthPayload,
                cancellationToken,
                sidecarAdminToken);
            if (healthPayload is null || !string.Equals(healthPayload.Status, "ok", StringComparison.OrdinalIgnoreCase))
            {
                return RuntimeSidecarDiscoveryResult.Failed(statusUrl, "failed to decode etragon health payload");
            }

            var statusPayload = await GetFromJsonAsync(
                statusPlan,
                DiscoveryJsonContext.Default.EtragonLatestStatusPayload,
                cancellationToken,
                sidecarAdminToken);
            if (statusPayload is null)
            {
                return RuntimeSidecarDiscoveryResult.Failed(statusUrl, "failed to decode etragon latest-status payload");
            }

            var hasEnrichment = await EndpointReturnsUsefulBodyAsync(enrichmentPlan, cancellationToken, sidecarAdminToken);
            var hasOpinion = await EndpointReturnsUsefulBodyAsync(opinionPlan, cancellationToken, sidecarAdminToken);
            var memorySnapshot = await TryGetSidecarMemorySnapshotAsync(memoryVersionsPlan, cancellationToken, sidecarAdminToken);

            return RuntimeSidecarDiscoveryResult.Succeeded(
                statusUrl,
                new RuntimeSidecarStatusSnapshot(
                    "etragon-api",
                    DateTimeOffset.UtcNow,
                    null,
                    true,
                    NormalizeSidecarDaemonStatus(statusPayload.Status),
                    statusPayload.TargetCount,
                    statusPayload.LearningActive,
                    statusPayload.LearnedRoutes,
                    hasEnrichment,
                    hasOpinion,
                    statusPayload.LastError,
                    memorySnapshot));
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex)
        {
            return RuntimeSidecarDiscoveryResult.Failed(statusUrl, ex.Message);
        }
    }

    public async Task<RuntimeDeploymentResponse> DeployAsync(
        RuntimeControlAccess runtime,
        RuntimeDeploymentRequest request,
        CancellationToken cancellationToken)
    {
        var deploymentUrl = runtime.Endpoint.TrimEnd('/') + "/v1/deployments";
        var planResult = await securityPolicy.BuildEndpointAccessPlanAsync(
            deploymentUrl,
            "runtime deployment endpoint",
            cancellationToken);
        if (planResult.Error is not null)
        {
            throw new InvalidOperationException(planResult.Error);
        }

        var payload = new GewyvernDeploymentRequestPayload(
            request.RequestId.Trim(),
            request.PipelineKind.Trim(),
            request.RequestedBy.Trim(),
            request.Confirmed,
            string.IsNullOrWhiteSpace(request.Target) ? null : request.Target.Trim());
        var plan = planResult.Plan!;
        using var response = plan.PinnedAddress is null
            ? await SendDeploymentAsync(httpClient, plan.RequestUri, payload, runtime.AdminToken, cancellationToken)
            : await SendPinnedDeploymentAsync(plan, payload, runtime.AdminToken, cancellationToken);
        response.EnsureSuccessStatusCode();
        var result = await response.Content.ReadFromJsonAsync(
            DiscoveryJsonContext.Default.GewyvernDeploymentResponsePayload,
            cancellationToken);
        if (result is null)
        {
            throw new InvalidOperationException("failed to decode gewyvern deployment response");
        }

        return new RuntimeDeploymentResponse(
            result.DeploymentId,
            result.RequestId,
            runtime.RuntimeId,
            result.PipelineKind,
            result.RequestedBy,
            result.Status,
            DateTimeOffset.FromUnixTimeMilliseconds(result.AcceptedUnixMs),
            result.Target,
            result.Replayed);
    }

    public async Task<RuntimeProtocolReadingSummary?> DiscoverProtocolReadingAsync(
        string runtimeId,
        string runtimeName,
        string endpoint,
        CancellationToken cancellationToken,
        string? runtimeAdminToken = null)
    {
        var targetsUrl = endpoint.TrimEnd('/') + "/v1/latest/targets";
        var targetsPlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(targetsUrl, "latest target index endpoint", cancellationToken);
        if (targetsPlanResult.Error is not null)
        {
            throw new InvalidOperationException(targetsPlanResult.Error);
        }

        var targetsPayload = await GetFromJsonAsync(
            targetsPlanResult.Plan!,
            DiscoveryJsonContext.Default.GewyvernLatestTargetsPayload,
            cancellationToken,
            runtimeAdminToken,
            GewyvernAdminTokenHeader);
        var target = targetsPayload?.TargetRefs?
            .FirstOrDefault(item => item.HasProtocolSurface && !string.IsNullOrWhiteSpace(item.PathSegment));
        if (target is null)
        {
            return null;
        }

        var surfacePath = $"{target.UrlPath?.TrimEnd('/') ?? $"/v1/latest/targets/{target.PathSegment}"}/protocol-surface.json";
        var surfaceUrl = endpoint.TrimEnd('/') + surfacePath;
        var surfacePlanResult = await securityPolicy.BuildEndpointAccessPlanAsync(surfaceUrl, "target protocol surface endpoint", cancellationToken);
        if (surfacePlanResult.Error is not null)
        {
            throw new InvalidOperationException(surfacePlanResult.Error);
        }

        var surfacePayload = await GetFromJsonAsync(
            surfacePlanResult.Plan!,
            DiscoveryJsonContext.Default.GewyvernProtocolSurfacePayload,
            cancellationToken,
            runtimeAdminToken,
            GewyvernAdminTokenHeader);
        if (surfacePayload is null
            || string.IsNullOrWhiteSpace(surfacePayload.Protocol)
            || string.IsNullOrWhiteSpace(surfacePayload.Entry))
        {
            return null;
        }

        var companions = (surfacePayload.ReadingCompanions ?? Array.Empty<GewyvernProtocolReadingCompanionPayload>())
            .Where(item => !string.IsNullOrWhiteSpace(item.Protocol) && !string.IsNullOrWhiteSpace(item.Entry))
            .Select(item => new RuntimeProtocolReadingCompanion(
                item.Protocol!.Trim(),
                item.Entry!.Trim(),
                string.IsNullOrWhiteSpace(item.ViaOverlay) ? null : item.ViaOverlay.Trim(),
                BuildProtocolEntrySurfacePath(item.Protocol!, item.Entry!)))
            .ToArray();

        return new RuntimeProtocolReadingSummary(
            runtimeId,
            runtimeName,
            endpoint,
            target.Name?.Trim() ?? runtimeName,
            target.PathSegment!.Trim(),
            string.IsNullOrWhiteSpace(target.UrlPath) ? $"/v1/latest/targets/{target.PathSegment}" : target.UrlPath.Trim(),
            surfacePath,
            surfacePayload.Protocol.Trim(),
            surfacePayload.Entry.Trim(),
            string.IsNullOrWhiteSpace(surfacePayload.DefaultEntry) ? surfacePayload.Entry.Trim() : surfacePayload.DefaultEntry.Trim(),
            surfacePayload.SelectedIsDefault,
            string.IsNullOrWhiteSpace(surfacePayload.SelectedOverlay) ? null : surfacePayload.SelectedOverlay.Trim(),
            companions);
    }

    private static string BuildCapabilityUrl(string endpoint, string? capabilityEndpoint)
    {
        if (!string.IsNullOrWhiteSpace(capabilityEndpoint))
        {
            return capabilityEndpoint.Trim();
        }

        return endpoint.TrimEnd('/') + "/v1/capabilities";
    }

    private static string BuildStatusUrl(string endpoint, string? statusEndpoint)
    {
        if (!string.IsNullOrWhiteSpace(statusEndpoint))
        {
            return statusEndpoint.Trim();
        }

        return endpoint.TrimEnd('/') + "/v1/latest/meta";
    }

    private static string BuildResilienceUrl(string endpoint) =>
        endpoint.TrimEnd('/') + "/v1/runtime/resilience.json";

    private static string BuildSidecarHealthUrl(string sidecarEndpoint) =>
        sidecarEndpoint.TrimEnd('/') + "/health";

    private static string BuildSidecarStatusUrl(string sidecarEndpoint, string? sidecarStatusEndpoint)
    {
        if (!string.IsNullOrWhiteSpace(sidecarStatusEndpoint))
        {
            return sidecarStatusEndpoint.Trim();
        }

        return sidecarEndpoint.TrimEnd('/') + "/v1/latest/status";
    }

    private static string BuildSidecarEnrichmentUrl(string sidecarEndpoint) =>
        sidecarEndpoint.TrimEnd('/') + "/v1/latest/evidence-chain-enrichment.json";

    private static string BuildSidecarOpinionUrl(string sidecarEndpoint) =>
        sidecarEndpoint.TrimEnd('/') + "/v1/latest/diagnostic-opinion.json";

    private static string BuildSidecarMemoryVersionsUrl(string sidecarEndpoint) =>
        sidecarEndpoint.TrimEnd('/') + "/v1/memory-versions.json";

    private static string BuildProtocolEntrySurfacePath(string protocol, string entry) =>
        $"/v1/protocols/{Uri.EscapeDataString(protocol.Trim())}/entries/{Uri.EscapeDataString(entry.Trim())}/surface.json";

    private async Task<bool> EndpointReturnsUsefulBodyAsync(EndpointAccessPlan plan, CancellationToken cancellationToken, string? sidecarAdminToken = null)
    {
        try
        {
            if (plan.PinnedAddress is null)
            {
                using var response = await SendAsync(httpClient, plan.RequestUri, cancellationToken, sidecarAdminToken);
                if (!response.IsSuccessStatusCode)
                {
                    return false;
                }

                var body = (await response.Content.ReadAsStringAsync(cancellationToken)).Trim();
                return !string.IsNullOrWhiteSpace(body)
                    && !string.Equals(body, "null", StringComparison.OrdinalIgnoreCase)
                    && !string.Equals(body, "{}", StringComparison.OrdinalIgnoreCase);
            }

            using var client = CreatePinnedHttpClient(plan);
            using var pinnedResponse = await SendAsync(client, plan.RequestUri, cancellationToken, sidecarAdminToken);
            if (!pinnedResponse.IsSuccessStatusCode)
            {
                return false;
            }

            var pinnedBody = (await pinnedResponse.Content.ReadAsStringAsync(cancellationToken)).Trim();
            return !string.IsNullOrWhiteSpace(pinnedBody)
                && !string.Equals(pinnedBody, "null", StringComparison.OrdinalIgnoreCase)
                && !string.Equals(pinnedBody, "{}", StringComparison.OrdinalIgnoreCase);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch
        {
            return false;
        }
    }

    private static string NormalizeSidecarDaemonStatus(string? status)
    {
        if (string.IsNullOrWhiteSpace(status))
        {
            return "unknown";
        }

        return status.Trim().ToLowerInvariant() switch
        {
            "ready" => "ready",
            "degraded" => "degraded",
            "starting" => "starting",
            _ => "unknown",
        };
    }

    private async Task<RuntimeSidecarMemorySnapshot> TryGetSidecarMemorySnapshotAsync(EndpointAccessPlan plan, CancellationToken cancellationToken, string? sidecarAdminToken = null)
    {
        try
        {
            if (plan.PinnedAddress is null)
            {
                using var response = await SendAsync(httpClient, plan.RequestUri, cancellationToken, sidecarAdminToken);
                if (!response.IsSuccessStatusCode)
                {
                    return new RuntimeSidecarMemorySnapshot(false, 0, 0, null, null, null, Array.Empty<RuntimeSidecarMemorySlotSummary>(), $"{(int)response.StatusCode} {response.ReasonPhrase}".Trim());
                }

                var payload = await response.Content.ReadFromJsonAsync(
                    DiscoveryJsonContext.Default.EtragonMemoryVersionsPayload,
                    cancellationToken);
                if (payload is null)
                {
                    return new RuntimeSidecarMemorySnapshot(false, 0, 0, null, null, null, Array.Empty<RuntimeSidecarMemorySlotSummary>(), "failed to decode etragon memory-versions payload");
                }

                return BuildMemorySnapshot(payload);
            }

            using var client = CreatePinnedHttpClient(plan);
            using var pinnedResponse = await SendAsync(client, plan.RequestUri, cancellationToken, sidecarAdminToken);
            if (!pinnedResponse.IsSuccessStatusCode)
            {
                return new RuntimeSidecarMemorySnapshot(false, 0, 0, null, null, null, Array.Empty<RuntimeSidecarMemorySlotSummary>(), $"{(int)pinnedResponse.StatusCode} {pinnedResponse.ReasonPhrase}".Trim());
            }

            var pinnedPayload = await pinnedResponse.Content.ReadFromJsonAsync(
                DiscoveryJsonContext.Default.EtragonMemoryVersionsPayload,
                cancellationToken);
            if (pinnedPayload is null)
            {
                return new RuntimeSidecarMemorySnapshot(false, 0, 0, null, null, null, Array.Empty<RuntimeSidecarMemorySlotSummary>(), "failed to decode etragon memory-versions payload");
            }

            return BuildMemorySnapshot(pinnedPayload);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception ex)
        {
            return new RuntimeSidecarMemorySnapshot(false, 0, 0, null, null, null, Array.Empty<RuntimeSidecarMemorySlotSummary>(), ex.Message);
        }
    }

    private async Task<CapabilityPayloadResult?> GetCapabilityPayloadAsync(
        EndpointAccessPlan plan,
        CancellationToken cancellationToken,
        string? adminToken,
        string adminTokenHeader)
    {
        using var pinnedClient = plan.PinnedAddress is null ? null : CreatePinnedHttpClient(plan);
        var client = pinnedClient ?? httpClient;
        using var response = await SendAsync(
            client,
            plan.RequestUri,
            cancellationToken,
            adminToken,
            adminTokenHeader);
        response.EnsureSuccessStatusCode();
        var body = await ReadBoundedBodyAsync(response.Content, 1024 * 1024, cancellationToken);
        if (body is null)
        {
            return null;
        }
        using var document = JsonDocument.Parse(body);
        var payload = JsonSerializer.Deserialize(
            document.RootElement.GetRawText(),
            DiscoveryJsonContext.Default.GewyvernCapabilityPayload);
        if (payload is null)
        {
            return null;
        }
        var knownFields = new HashSet<string>(StringComparer.Ordinal)
        {
            "service",
            "version",
            "latest_snapshot",
            "authenticated_deployment",
            "serve_required",
            "external_sidecar_context",
            "target_path_segment_encoding",
            "target_direct_path_chars",
            "endpoints",
        };
        var extensions = new Dictionary<string, bool>(StringComparer.Ordinal);
        foreach (var property in document.RootElement.EnumerateObject())
        {
            if (knownFields.Contains(property.Name))
            {
                continue;
            }
            if (property.Value.ValueKind is not (JsonValueKind.True or JsonValueKind.False))
            {
                return null;
            }
            extensions[property.Name] = property.Value.GetBoolean();
        }
        return new CapabilityPayloadResult(payload, extensions);
    }

    private static async Task<byte[]?> ReadBoundedBodyAsync(
        HttpContent content,
        int maximumBytes,
        CancellationToken cancellationToken)
    {
        if (content.Headers.ContentLength > maximumBytes)
        {
            return null;
        }
        await using var input = await content.ReadAsStreamAsync(cancellationToken);
        using var output = new MemoryStream(Math.Min(maximumBytes, 16 * 1024));
        var buffer = new byte[8192];
        while (true)
        {
            var read = await input.ReadAsync(buffer, cancellationToken);
            if (read == 0)
            {
                return output.ToArray();
            }
            if (output.Length + read > maximumBytes)
            {
                return null;
            }
            output.Write(buffer, 0, read);
        }
    }

    private async Task<T?> GetFromJsonAsync<T>(
        EndpointAccessPlan plan,
        JsonTypeInfo<T> jsonTypeInfo,
        CancellationToken cancellationToken,
        string? adminToken = null,
        string adminTokenHeader = EtragonAdminTokenHeader)
    {
        if (plan.PinnedAddress is null)
        {
            using var response = await SendAsync(httpClient, plan.RequestUri, cancellationToken, adminToken, adminTokenHeader);
            response.EnsureSuccessStatusCode();
            return await response.Content.ReadFromJsonAsync(jsonTypeInfo, cancellationToken);
        }

        using var client = CreatePinnedHttpClient(plan);
        using var pinnedResponse = await SendAsync(client, plan.RequestUri, cancellationToken, adminToken, adminTokenHeader);
        pinnedResponse.EnsureSuccessStatusCode();
        return await pinnedResponse.Content.ReadFromJsonAsync(jsonTypeInfo, cancellationToken);
    }

    private static async Task<HttpResponseMessage> SendAsync(HttpClient client, Uri requestUri, CancellationToken cancellationToken, string? adminToken, string adminTokenHeader = EtragonAdminTokenHeader)
    {
        if (string.IsNullOrWhiteSpace(adminToken))
        {
            return await client.GetAsync(requestUri, cancellationToken);
        }

        using var request = new HttpRequestMessage(HttpMethod.Get, requestUri);
        request.Headers.TryAddWithoutValidation(adminTokenHeader, adminToken.Trim());
        return await client.SendAsync(request, cancellationToken);
    }

    private async Task<HttpResponseMessage> SendPinnedDeploymentAsync(
        EndpointAccessPlan plan,
        GewyvernDeploymentRequestPayload payload,
        string? adminToken,
        CancellationToken cancellationToken)
    {
        using var client = CreatePinnedHttpClient(plan);
        return await SendDeploymentAsync(client, plan.RequestUri, payload, adminToken, cancellationToken);
    }

    private static async Task<HttpResponseMessage> SendDeploymentAsync(
        HttpClient client,
        Uri requestUri,
        GewyvernDeploymentRequestPayload payload,
        string? adminToken,
        CancellationToken cancellationToken)
    {
        using var request = new HttpRequestMessage(HttpMethod.Post, requestUri)
        {
            Content = new StringContent(
                JsonSerializer.Serialize(payload, DiscoveryJsonContext.Default.GewyvernDeploymentRequestPayload),
                Encoding.UTF8,
                "application/json"),
        };
        if (!string.IsNullOrWhiteSpace(adminToken))
        {
            request.Headers.TryAddWithoutValidation(GewyvernAdminTokenHeader, adminToken.Trim());
        }
        return await client.SendAsync(request, cancellationToken);
    }

    private HttpClient CreatePinnedHttpClient(EndpointAccessPlan plan)
    {
        var handler = new SocketsHttpHandler
        {
            ConnectCallback = async (context, cancellationToken) =>
            {
                var socket = new Socket(plan.PinnedAddress!.AddressFamily, SocketType.Stream, ProtocolType.Tcp);
                try
                {
                    await socket.ConnectAsync(plan.PinnedAddress, plan.RequestUri.Port, cancellationToken);
                    return new NetworkStream(socket, ownsSocket: true);
                }
                catch
                {
                    socket.Dispose();
                    throw;
                }
            },
        };
        var client = new HttpClient(handler, disposeHandler: true)
        {
            Timeout = httpClient.Timeout,
        };
        foreach (var header in httpClient.DefaultRequestHeaders)
        {
            client.DefaultRequestHeaders.TryAddWithoutValidation(header.Key, header.Value);
        }
        return client;
    }

    private static RuntimeSidecarMemorySnapshot BuildMemorySnapshot(EtragonMemoryVersionsPayload payload)
    {
        var slots = (payload.Slots ?? Array.Empty<EtragonMemorySlotPayload>())
            .Select(slot => new RuntimeSidecarMemorySlotSummary(
                slot.Slot ?? "unnamed",
                slot.Label,
                slot.Note,
                string.IsNullOrWhiteSpace(slot.Source) ? "manual" : slot.Source.Trim(),
                slot.SavedUnixMs is > 0
                    ? DateTimeOffset.FromUnixTimeMilliseconds(slot.SavedUnixMs.Value)
                    : null,
                slot.PatternCount ?? 0,
                slot.LabelCount ?? 0))
            .ToArray();
        var latest = slots.FirstOrDefault();
        return new RuntimeSidecarMemorySnapshot(
            true,
            payload.SlotCount ?? slots.Length,
            payload.History?.Length ?? 0,
            latest?.Slot,
            latest?.Label,
            latest?.Source,
            slots,
            null);
    }

}
