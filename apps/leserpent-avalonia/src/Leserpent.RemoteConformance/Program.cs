using System.Text;
using System.Text.Json;

if (args is ["--credential-resolve", var credentialOrigin])
{
    var endpoint = RemoteClientOptions.ParseEndpoint(credentialOrigin);
    var resolved = RemoteTokenResolver.Resolve(endpoint);
    Console.WriteLine(
        $"remote credential valid: source={resolved.Source.ToString().ToLowerInvariant()}, account={RemoteTokenResolver.Account(endpoint)}");
    return 0;
}

if (args.Length is 4 or 6 && args[0] == "--connect")
{
    if (args.Length == 6
        && args[4] is not ("--refresh" or "--refresh-capabilities" or "--inspect"))
    {
        Console.Error.WriteLine("usage: Leserpent.RemoteConformance --connect HTTPS_ORIGIN CA_PATH CACHE_PATH [--refresh RUNTIME_ID | --refresh-capabilities RUNTIME_ID | --inspect RUNTIME_ID]");
        return 2;
    }
    var endpoint = args[1];
    var certificate = args[2];
    var cache = args[3];
    var refreshRuntimeId = args.Length == 6 && args[4] == "--refresh" ? args[5] : null;
    var capabilityRuntimeId = args.Length == 6 && args[4] == "--refresh-capabilities"
        ? args[5]
        : null;
    var inspectRuntimeId = args.Length == 6 && args[4] == "--inspect" ? args[5] : null;
    var token = Environment.GetEnvironmentVariable("LESERPENT_REMOTE_TOKEN")
        ?? throw new InvalidOperationException("LESERPENT_REMOTE_TOKEN is required");
    var options = RemoteClientOptions.Create(endpoint, certificate, token, cache);
    await using var client = new RemoteEventClient(options);
    var completed = new TaskCompletionSource<RemoteFeedState>(
        TaskCreationOptions.RunContinuationsAsynchronously);
    client.StateChanged += state =>
    {
        Console.WriteLine(
            $"remote state: phase={state.Phase}, revision={state.Revision}, runtimes={state.Runtimes.Count}, detail={state.Detail}");
        if (state.Phase == RemoteFeedPhase.Live)
        {
            completed.TrySetResult(state);
        }
        else if (state.Phase == RemoteFeedPhase.Stale)
        {
            completed.TrySetException(new InvalidOperationException(state.Detail));
        }
    };
    client.Start();
    var live = await completed.Task.WaitAsync(TimeSpan.FromSeconds(15));
    Console.WriteLine(
        $"remote conformance valid: revision={live.Revision}, runtimes={live.Runtimes.Count}, stale={live.IsStale.ToString().ToLowerInvariant()}");
    var mutationRuntimeId = refreshRuntimeId ?? capabilityRuntimeId;
    if (mutationRuntimeId is not null)
    {
        var runtime = live.Runtimes.Single(candidate => candidate.Id == mutationRuntimeId);
        var updated = new TaskCompletionSource<RemoteFeedState>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var updateGate = new object();
        ulong? appliedRevision = null;
        var latestMutationState = live;
        bool MutationSettled(RemoteFeedState state)
        {
            var changedRuntime = state.Runtimes.FirstOrDefault(candidate =>
                candidate.Id == mutationRuntimeId);
            return state.Phase == RemoteFeedPhase.Live
                && changedRuntime is not null
                && (capabilityRuntimeId is null
                    ? changedRuntime.Revision > runtime.Revision
                    : appliedRevision is { } revision
                        && changedRuntime.CapabilitiesObservedForRevision is { } observedFor
                        && observedFor >= revision);
        }
        client.StateChanged += state =>
        {
            lock (updateGate)
            {
                latestMutationState = state;
                if (MutationSettled(state))
                {
                    updated.TrySetResult(state);
                }
            }
        };
        using var mutation = new RemoteMutationClient(options);
        var result = capabilityRuntimeId is not null
            ? await mutation.RefreshCapabilitiesAsync(
                mutationRuntimeId,
                runtime.Revision,
                "dotnet-conformance")
            : await mutation.RefreshAsync(
                mutationRuntimeId,
                runtime.Revision,
                "dotnet-conformance");
        lock (updateGate)
        {
            appliedRevision = result.Revision;
            if (MutationSettled(latestMutationState))
            {
                updated.TrySetResult(latestMutationState);
            }
        }
        var eventState = await updated.Task.WaitAsync(TimeSpan.FromSeconds(15));
        var eventRuntime = eventState.Runtimes.Single(candidate =>
            candidate.Id == mutationRuntimeId);
        if (capabilityRuntimeId is not null)
        {
            Require(eventRuntime.Revision > result.Revision
                && eventRuntime.Capabilities is { IsUnobserved: false }
                && eventRuntime.CapabilitiesObservedForRevision is { } observedFor
                && observedFor >= result.Revision,
                "capability mutation did not settle to an observed projection");
        }
        else
        {
            Require(result.Revision == eventRuntime.Revision,
                "mutation response and event stream revisions diverged");
            Require(eventRuntime.RefreshStatus == RefreshStatus.Pending,
                "mutation event did not expose the pending refresh state");
        }
        Console.WriteLine(
            $"remote mutation conformance valid: kind={(capabilityRuntimeId is null ? "runtime_refresh" : "runtime_capabilities_refresh")}, initial_revision={runtime.Revision}, applied_revision={result.Revision}, event_revision={eventRuntime.Revision}, capabilities_observed={(eventRuntime.Capabilities is { IsUnobserved: false }).ToString().ToLowerInvariant()}, capabilities_observed_for_revision={eventRuntime.CapabilitiesObservedForRevision?.ToString() ?? "none"}, runtime={eventRuntime.Id}, stale={eventState.IsStale.ToString().ToLowerInvariant()}");
    }
    if (inspectRuntimeId is not null)
    {
        using var workspaceClient = new RemoteWorkspaceClient(options);
        var liveWorkspace = await workspaceClient.LoadAsync(
            inspectRuntimeId,
            "dotnet-conformance");
        Require(liveWorkspace.Runtime.Id == inspectRuntimeId,
            "workspace query changed runtime identity");
        Require(liveWorkspace.History.Count <= RemoteWorkspaceClient.MaxHistoryEntries,
            "workspace query exceeded its history bound");
        Require(liveWorkspace.Logs.Count <= RemoteWorkspaceClient.MaxLogEntries,
            "workspace query exceeded its log bound");
        var workspaceCapabilities = liveWorkspace.Runtime.Capabilities;
        Console.WriteLine(
            $"remote workspace conformance valid: revision={liveWorkspace.Revision}, runtime={liveWorkspace.Runtime.Id}, history={liveWorkspace.History.Count}, logs={liveWorkspace.Logs.Count}, endpoint_retained=false, capabilities_observed={(workspaceCapabilities is { IsUnobserved: false }).ToString().ToLowerInvariant()}, capabilities_observed_for_revision={liveWorkspace.Runtime.CapabilitiesObservedForRevision?.ToString() ?? "none"}, capability_version={workspaceCapabilities?.Version ?? "unobserved"}");
    }
    return 0;
}

if (args.Length != 0)
{
    Console.Error.WriteLine("usage: Leserpent.RemoteConformance [--connect HTTPS_ORIGIN CA_PATH CACHE_PATH [--refresh RUNTIME_ID | --refresh-capabilities RUNTIME_ID | --inspect RUNTIME_ID] | --credential-resolve HTTPS_ORIGIN]");
    return 2;
}

var snapshot = RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(Fixtures.SnapshotJson));
Require(snapshot is RemoteEvent.Snapshot
{
    Revision: 7,
    ResumedAfter: 6,
    Runtimes.Count: 1,
}, "snapshot codec did not preserve revisions or runtimes");
var decodedSnapshot = (RemoteEvent.Snapshot)snapshot;
Require(decodedSnapshot.Runtimes[0].Id == "runtime-a", "snapshot codec changed runtime identity");
Require(decodedSnapshot.Runtimes[0].Capabilities is
    {
        Service: "gewyvern-api",
        Version: "1.2.0",
        AuthenticatedDeployment: true,
        Endpoints.Count: 2,
    }, "snapshot codec did not preserve runtime capabilities");
Require(decodedSnapshot.Runtimes[0].CapabilitiesObservedForRevision == 6,
    "snapshot codec did not preserve the capability observation binding");
RequireThrows<InvalidDataException>(() => RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.SnapshotJson.Replace(
        "\"source\": \"gewyvern-api\"",
        "\"source\": \"untrusted\"",
        StringComparison.Ordinal))),
    "snapshot accepted an untrusted capability source");
RequireThrows<InvalidDataException>(() => RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.SnapshotJson.Replace(
        "\"/v1/capabilities\"",
        "\"/v1/capabilities?unsafe=true\"",
        StringComparison.Ordinal))),
    "snapshot accepted a non-canonical capability endpoint");
RequireThrows<InvalidDataException>(() => RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.SnapshotJson.Replace(
        "\"authenticated_deployment\": true",
        "\"authenticated_deployment\": false",
        StringComparison.Ordinal))),
    "snapshot accepted an inconsistent authenticated deployment capability");
RequireThrows<InvalidDataException>(() => RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.SnapshotJson.Replace(
        "\"endpoints\": [\"/v1/capabilities\", \"/v1/deployments\"]",
        "\"endpoints\": null",
        StringComparison.Ordinal))),
    "snapshot accepted null capability endpoints");
RequireThrows<InvalidDataException>(() => RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.SnapshotJson.Replace(
        "\"capabilities_observed_for_revision\": 6",
        "\"capabilities_observed_for_revision\": 7",
        StringComparison.Ordinal))),
    "snapshot accepted a non-prior capability observation revision");

var stateMachine = new RemoteFeedStateMachine();
var liveState = stateMachine.Accept(decodedSnapshot);
Require(!liveState.IsStale && liveState.Phase == RemoteFeedPhase.Live,
    "snapshot did not establish a live state");
Require(liveState.SnapshotGeneration == 1,
    "snapshot did not advance the projection generation");
var heartbeatState = stateMachine.Accept(new RemoteEvent.Heartbeat(7));
Require(heartbeatState.SnapshotGeneration == 1,
    "heartbeat incorrectly advanced the projection generation");
var reconnecting = stateMachine.ConnectionLost("test disconnect");
Require(reconnecting.IsStale && reconnecting.Phase == RemoteFeedPhase.Reconnecting,
    "disconnect did not mark cached data stale");
for (var attempt = 2; attempt <= 8; attempt++)
{
    reconnecting = stateMachine.ConnectionLost("test disconnect");
}
Require(reconnecting.Phase == RemoteFeedPhase.Stale && reconnecting.ConsecutiveFailures == 8,
    "reconnect policy did not stop at its attempt bound");
var resumed = stateMachine.Resume();
Require(resumed.Phase == RemoteFeedPhase.Connecting
    && resumed.ConsecutiveFailures == 0
    && resumed.Revision == 7
    && resumed.IsStale,
    "manual reconnect did not preserve the cursor and stale projection");
RequireThrows<InvalidOperationException>(() => stateMachine.Resume(),
    "manual reconnect was accepted while already connecting");

var cacheRoot = Path.Combine(Path.GetTempPath(), $"leserpent-remote-{Guid.NewGuid():N}");
Directory.CreateDirectory(cacheRoot);
var cachePath = Path.Combine(cacheRoot, "snapshot.json");
try
{
    var cacheEndpoint = new Uri("https://localhost:9443");
    var store = new RemoteSnapshotStore(cacheEndpoint, cachePath);
    store.Save(decodedSnapshot);
    var restored = store.Load();
    Require(restored is { Revision: 7, Runtimes.Count: 1 },
        "snapshot cache did not round-trip");
    var wrongEndpoint = new RemoteSnapshotStore(new Uri("https://localhost:9444"), cachePath);
    RequireThrows<InvalidDataException>(() => wrongEndpoint.Load(),
        "snapshot cache was accepted for another endpoint");
    File.WriteAllText(cachePath, "{not-json");
    RequireThrows<InvalidDataException>(() => store.Load(),
        "malformed snapshot cache was accepted");
}
finally
{
    Directory.Delete(cacheRoot, recursive: true);
}

var resync = RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(
    "{\"schema_version\":1,\"event\":{\"kind\":\"resync_required\",\"payload\":{\"requested_after\":99,\"current_revision\":7}}}"));
var resyncMachine = new RemoteFeedStateMachine();
resyncMachine.Accept(resync);
Require(resyncMachine.ResyncRequested, "resync event did not request cursor reset");
var reset = resyncMachine.ResetForResync();
Require(reset.Revision is null && !resyncMachine.ResyncRequested,
    "resync did not clear the reconnect cursor");
RequireThrows<InvalidDataException>(() => RemoteEventCodec.Decode(Encoding.UTF8.GetBytes(
    "{\"schema_version\":2,\"event\":{\"kind\":\"heartbeat\",\"payload\":{\"revision\":7}}}")),
    "unknown event schema was accepted");

var credentialEndpoint = RemoteClientOptions.ParseEndpoint("https://EXAMPLE.com:9443");
Require(RemoteTokenResolver.Account(credentialEndpoint) == "https://example.com:9443",
    "credential account did not canonicalize the HTTPS origin");
var storedToken = new string('s', 32);
var environmentToken = new string('e', 32);
var stored = RemoteTokenResolver.Resolve(
    credentialEndpoint,
    environmentToken,
    new FixtureTokenStore(storedToken));
Require(stored.Source == RemoteTokenSource.PlatformStore && stored.Value == storedToken,
    "platform credential did not take precedence");
var fallback = RemoteTokenResolver.Resolve(
    credentialEndpoint,
    environmentToken,
    new FixtureTokenStore(null));
Require(fallback.Source == RemoteTokenSource.Environment && fallback.Value == environmentToken,
    "environment credential fallback failed");
RequireThrows<ArgumentException>(() => RemoteTokenResolver.Resolve(
    credentialEndpoint,
    environmentToken,
    new FixtureTokenStore("invalid token")),
    "invalid platform credential silently fell back to the environment");
RequireThrows<ArgumentException>(() => RemoteClientOptions.ValidateToken(new string('x', 4097)),
    "oversized remote token was accepted");
var fixtureVault = new FixtureTokenVault();
RemoteTokenResolver.Store(credentialEndpoint, storedToken, fixtureVault);
Require(fixtureVault.Load(credentialEndpoint) == storedToken,
    "credential vault did not retain the endpoint-scoped token");
Require(fixtureVault.Load(new Uri("https://example.com:9444")) is null,
    "credential vault leaked a token across endpoints");
RequireThrows<ArgumentException>(() => RemoteTokenResolver.Store(
    credentialEndpoint,
    "invalid token",
    fixtureVault),
    "credential vault accepted an invalid token");
Require(fixtureVault.StoreCount == 1,
    "invalid token validation happened after credential mutation");
RemoteTokenResolver.Delete(credentialEndpoint, fixtureVault);
Require(fixtureVault.Load(credentialEndpoint) is null && fixtureVault.DeleteCount == 1,
    "credential vault deletion failed");
var trustIdentity = RemoteTrustIdentity.FromSha256(
    new Uri("https://EXAMPLE.com:9443"),
    Enumerable.Range(0, 32).Select(value => checked((byte)value)).ToArray());
Require(trustIdentity.Origin == "https://example.com:9443"
    && trustIdentity.ShortFingerprint == "0001020304050607"
    && trustIdentity.Sha256Fingerprint.StartsWith("00:01:02:03", StringComparison.Ordinal),
    "remote trust identity did not canonicalize origin and CA fingerprint");

var decodedWorkspace = RemoteWorkspaceCodec.Compose(
    Fixtures.InspectResponse(7, "runtime-a"),
    Fixtures.HistoryResponse(7, "runtime-a"),
    Fixtures.LogsResponse(7, "runtime-a", "bounded warning\ncontinued"),
    "runtime-a");
Require(decodedWorkspace.Revision == 7
    && decodedWorkspace.Runtime.Id == "runtime-a"
    && decodedWorkspace.History is [{ CommandId: "command-a", Revision: 7, Status: "applied" }]
    && decodedWorkspace.Logs is [{ Sequence: 41, Level: "warning", Display: "bounded warning continued" }],
    "workspace query did not preserve the bounded safe projection");
Require(decodedWorkspace.Runtime.GetType().GetProperty("Endpoint") is null,
    "workspace safe projection retained the remote endpoint");
RequireThrows<InvalidDataException>(() => RemoteWorkspaceCodec.Compose(
    Fixtures.InspectResponse(7, "runtime-a"),
    Fixtures.HistoryResponse(8, "runtime-a"),
    Fixtures.LogsResponse(7, "runtime-a"),
    "runtime-a"),
    "workspace accepted torn query revisions");
RequireThrows<InvalidDataException>(() => RemoteWorkspaceCodec.Compose(
    Fixtures.InspectResponse(7, "runtime-a"),
    Fixtures.HistoryResponse(7, "runtime-b"),
    Fixtures.LogsResponse(7, "runtime-a"),
    "runtime-a"),
    "workspace accepted history from another runtime");
RequireThrows<InvalidDataException>(() => RemoteWorkspaceCodec.Compose(
    Fixtures.InspectResponse(7, "runtime-a", extraRuntimeField: true),
    Fixtures.HistoryResponse(7, "runtime-a"),
    Fixtures.LogsResponse(7, "runtime-a"),
    "runtime-a"),
    "workspace accepted an unknown runtime field");
RequireThrows<InvalidDataException>(() => RemoteWorkspaceCodec.Compose(
    Fixtures.InspectResponse(7, "runtime-a"),
    Fixtures.HistoryResponse(7, "runtime-a"),
    Fixtures.LogsResponse(8, "runtime-a"),
    "runtime-a"),
    "workspace accepted torn log revision");
RequireThrows<InvalidDataException>(() => RemoteWorkspaceCodec.Compose(
    Fixtures.InspectResponse(7, "runtime-a"),
    Fixtures.HistoryResponse(7, "runtime-a"),
    Fixtures.LogsResponse(7, "runtime-b"),
    "runtime-a"),
    "workspace accepted logs from another runtime");
RequireThrows<InvalidDataException>(() => RemoteWorkspaceCodec.Compose(
    Fixtures.InspectResponse(7, "runtime-a"),
    Fixtures.HistoryResponse(7, "runtime-a"),
    Fixtures.LogsResponse(
        7,
        "runtime-a",
        new string('x', RemoteWorkspaceClient.MaxLogMessageBytes + 1)),
    "runtime-a"),
    "workspace accepted an oversized log message");

Console.WriteLine("remote state conformance valid: codec=true, stale=true, reconnect_attempts=8, manual_resume=true, endpoint_cache=true, credential_resolution=true, credential_mutation=true, trust_identity=true, workspace_atomic=true, logs_bounded=true, endpoint_retained=false");
return 0;

static void Require(bool condition, string message)
{
    if (!condition)
    {
        throw new InvalidDataException(message);
    }
}

static void RequireThrows<TException>(Action action, string message)
    where TException : Exception
{
    try
    {
        action();
    }
    catch (TException)
    {
        return;
    }
    throw new InvalidDataException(message);
}

static class Fixtures
{
public static byte[] InspectResponse(
    ulong revision,
    string runtimeId,
    bool extraRuntimeField = false) => Encoding.UTF8.GetBytes($$"""
{
  "schema_version": 1,
  "response": {
    "kind": "query",
    "payload": {
      "kind": "runtime_inspect",
      "revision": {{revision}},
      "runtime": {{RuntimeJson(revision, runtimeId, extraRuntimeField)}}
    }
  }
}
""");

public static byte[] HistoryResponse(ulong revision, string runtimeId) =>
    Encoding.UTF8.GetBytes($$"""
{
  "schema_version": 1,
  "response": {
    "kind": "query",
    "payload": {
      "kind": "runtime_history",
      "revision": {{revision}},
      "entries": [{
        "command_id": "command-a",
        "status": "applied",
        "runtime": {{RuntimeJson(revision, runtimeId)}},
        "events": []
      }]
    }
  }
}
""");

public static byte[] LogsResponse(
    ulong revision,
    string runtimeId,
    string message = "bounded warning") => Encoding.UTF8.GetBytes($$"""
{
  "schema_version": 1,
  "response": {
    "kind": "query",
    "payload": {
      "kind": "runtime_logs",
      "revision": {{revision}},
      "runtime_id": "{{runtimeId}}",
      "runtime_name": "Runtime A",
      "entries": [{
        "sequence": 41,
        "level": "warning",
        "message": {{JsonSerializer.Serialize(message)}}
      }]
    }
  }
}
""");

private static string RuntimeJson(
    ulong revision,
    string runtimeId,
    bool extraRuntimeField = false) => $$"""
{
  "id": "{{runtimeId}}",
  "name": "Runtime A",
  "endpoint": "unix:///private/runtime-a.sock",
  "revision": {{revision}},
  "refresh_count": 2,
  "refresh_status": "ready",
  "tags": {"environment": "test", "cluster": null, "role": null},
  "status": {
    "status_source": "gewyvern",
    "status_fetched_at": null,
    "status_fetch_error": null,
    "has_latest_snapshot": false,
    "snapshot_kind": null,
    "target_count": null,
    "has_summary_json": false,
    "has_analysis_json": false,
    "has_training_example_json": false,
    "has_training_dataset_manifest": false,
    "has_export_json": false,
    "has_report_json": false,
    "has_report_html": false,
    "has_external_sidecar_context": false,
    "has_external_evidence_chain_enrichment": false,
    "has_external_diagnostic_opinion": false,
    "resilience_degraded": false,
    "resilience_status": null,
    "resilience_summary": null,
    "socket_service_status": null,
    "socket_consecutive_idle_timeouts": null,
    "socket_total_idle_timeouts": null
  },
  "capabilities_observed_for_revision": {{revision - 1}},
  "capabilities": {
    "source": "gewyvern-api",
    "service": "gewyvern-api",
    "version": "1.2.0",
    "latest_snapshot": true,
    "authenticated_deployment": true,
    "serve_required": true,
    "external_sidecar_context": true,
    "target_path_segment_encoding": "percent-encoding",
    "target_direct_path_chars": "A-Z a-z 0-9 . _ ~ :",
    "endpoints": ["/v1/capabilities", "/v1/deployments"],
    "extensions": {"protocol_catalog": true}
  }{{(extraRuntimeField ? ",\n  \"unexpected\": true" : string.Empty)}}
}
""";

public const string SnapshotJson = """
{
  "schema_version": 1,
  "event": {
    "kind": "runtime_snapshot",
    "payload": {
      "revision": 7,
      "resumed_after": 6,
      "runtimes": [{
        "id": "runtime-a",
        "name": "Runtime A",
        "revision": 7,
        "refresh_count": 2,
        "refresh_status": "ready",
        "tags": {"environment": "test", "cluster": null, "role": null},
        "status": {
          "status_source": "gewyvern",
          "status_fetched_at": null,
          "status_fetch_error": null,
          "has_latest_snapshot": false,
          "snapshot_kind": null,
          "target_count": null,
          "has_summary_json": false,
          "has_analysis_json": false,
          "has_training_example_json": false,
          "has_training_dataset_manifest": false,
          "has_export_json": false,
          "has_report_json": false,
          "has_report_html": false,
          "has_external_sidecar_context": false,
          "has_external_evidence_chain_enrichment": false,
          "has_external_diagnostic_opinion": false,
          "resilience_degraded": false,
          "resilience_status": null,
          "resilience_summary": null,
          "socket_service_status": null,
          "socket_consecutive_idle_timeouts": null,
          "socket_total_idle_timeouts": null
        },
        "capabilities_observed_for_revision": 6,
        "capabilities": {
          "source": "gewyvern-api",
          "service": "gewyvern-api",
          "version": "1.2.0",
          "latest_snapshot": true,
          "authenticated_deployment": true,
          "serve_required": true,
          "external_sidecar_context": true,
          "target_path_segment_encoding": "percent-encoding",
          "target_direct_path_chars": "A-Z a-z 0-9 . _ ~ :",
          "endpoints": ["/v1/capabilities", "/v1/deployments"],
          "extensions": {"protocol_catalog": true}
        }
      }]
    }
  }
}
""";
}

sealed class FixtureTokenStore(string? token) : IRemoteTokenStore
{
    public string? Load(Uri endpoint)
    {
        _ = endpoint;
        return token;
    }
}

sealed class FixtureTokenVault : IRemoteTokenVault
{
    private readonly Dictionary<string, string> tokens = new(StringComparer.Ordinal);

    public int StoreCount { get; private set; }
    public int DeleteCount { get; private set; }

    public string? Load(Uri endpoint) => tokens.GetValueOrDefault(
        RemoteTokenResolver.Account(endpoint));

    public void Store(Uri endpoint, string token)
    {
        StoreCount++;
        tokens[RemoteTokenResolver.Account(endpoint)] = token;
    }

    public void Delete(Uri endpoint)
    {
        DeleteCount++;
        tokens.Remove(RemoteTokenResolver.Account(endpoint));
    }
}
