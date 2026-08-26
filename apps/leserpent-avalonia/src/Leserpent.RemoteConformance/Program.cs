using System.Diagnostics;
using System.Text;
using System.Text.Json;

if (args is ["--benchmark-workspace-logs"])
{
    Console.WriteLine(JsonSerializer.Serialize(RunWorkspaceLogBenchmark()));
    return 0;
}

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
    using (var healthClient = new RemoteHealthClient(options))
    {
        var preflightHealth = await healthClient.CheckAsync();
        Console.WriteLine(
            $"remote health valid: status={preflightHealth.Status}, authority_owned={preflightHealth.AuthorityOwned.ToString().ToLowerInvariant()}, protocol_schema_version={preflightHealth.ProtocolSchemaVersion}, queue_present={(preflightHealth.EffectQueue is not null).ToString().ToLowerInvariant()}");
    }
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
var fixtureHealth = RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(Fixtures.HealthJson));
var fixtureReceipt = RemoteUnregistrationReceiptCodec.Decode(
    Encoding.UTF8.GetBytes(Fixtures.UnregistrationReceiptJson));
RemoteLeselangClient.VerifyContract();
RemoteDebuggerClient.VerifyContract();
RemoteRegistrationClient.VerifyContract();
RemoteTopologyStateMachine.VerifyContract();
RemoteRuntimeSearch.VerifyContract();
await RemoteTopologyRefreshCoordinator.VerifyContractAsync();
RemoteWorkspaceLaunchCoordinator.VerifyContract();
RemoteMutationFences.VerifyContract();
RemoteMutationCoordinator.VerifyContract();
RemoteUiActionRouter.VerifyContract();
await RemoteEventClient.VerifyLifecycleContractAsync();
await RemoteAuthorityHealthCoordinator.VerifyContractAsync();
Require(fixtureHealth is
{
    Status: "ready",
    AuthorityOwned: true,
    ProtocolSchemaVersion: 1,
    EffectQueue.Active: 1,
    EffectQueue.Terminal: 5,
    RuntimeUnregistrationReplayHorizon:
    {
        Capacity: 256,
        Retained: 12,
        OldestGeneration: 4,
        NewestGeneration: 15,
        NextGeneration: 16,
        EvictedThroughGeneration: 3,
    },
    OrchestraDeleteReplayHorizon:
    {
        Capacity: 4096,
        Retained: 2,
        AvailableCapacity: 4094,
        AdmissionState: RemoteOrchestraDeleteReplayAdmissionState.Ready,
        AdmissionPressure: RemoteOrchestraDeleteReplayAdmissionPressure.Healthy,
        OperatorAction: null,
        CheckpointLagGenerations: 0,
    },
}, "health codec did not preserve authority and queue state");
var fixtureReplayHorizon = fixtureHealth.RuntimeUnregistrationReplayHorizon
    ?? throw new InvalidOperationException("fixture health omitted its replay horizon");
Require(
    fixtureReplayHorizon.Classify(15)
        == RemoteUnregistrationGenerationState.Retained,
    "health replay horizon did not retain its newest receipt generation");
Require(
    fixtureReplayHorizon.Classify(3)
        == RemoteUnregistrationGenerationState.Evicted,
    "health replay horizon did not classify its eviction highwater");
Require(
    fixtureReplayHorizon.Classify(16)
        == RemoteUnregistrationGenerationState.Future,
    "health replay horizon did not classify its next generation as future");
RequireThrows<ArgumentOutOfRangeException>(
    () => fixtureReplayHorizon.Classify(0),
    "health replay horizon accepted generation zero");
Require(fixtureReceipt is
{
    CommandId: "runtime-unregister-a",
    Receipt:
    {
        OperationGeneration: 15,
        Removed.Count: 1,
        DeletedOrchestraRuntimeCount: 1,
        DeletedOrchestraRunCount: 2,
        DeletedOrchestraEventCount: 3,
    },
    ReplayHorizon:
    {
        OldestGeneration: 4,
        NewestGeneration: 15,
    },
}, "unregistration receipt codec did not preserve its atomic lookup");
var missingReceipt = RemoteUnregistrationReceiptCodec.Decode(
    Encoding.UTF8.GetBytes(Fixtures.MissingUnregistrationReceiptJson));
Require(missingReceipt.Receipt is null,
    "unregistration receipt codec fabricated a missing receipt");
RequireThrows<InvalidDataException>(() => RemoteUnregistrationReceiptCodec.Decode(
    Encoding.UTF8.GetBytes(Fixtures.UnregistrationReceiptJson.Replace(
        "\"operation_generation\": 15",
        "\"operation_generation\": 16",
        StringComparison.Ordinal))),
    "unregistration receipt codec accepted a future generation");
RequireThrows<InvalidDataException>(() => RemoteUnregistrationReceiptCodec.Decode(
    Encoding.UTF8.GetBytes(Fixtures.UnregistrationReceiptJson.Replace(
        "\"expected_revision\": 4",
        "\"expected_revision\": 0",
        StringComparison.Ordinal))),
    "unregistration receipt codec accepted a zero target revision");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.HealthJson.Replace(
        "\"active\": 1",
        "\"active\": 2",
        StringComparison.Ordinal))),
    "health codec accepted inconsistent active counters");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.HealthJson.Replace(
        "\"newest_generation\": 15",
        "\"newest_generation\": 14",
        StringComparison.Ordinal))),
    "health codec accepted a non-contiguous replay horizon");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.HealthJson.Replace(
        "\"available_capacity\": 4094",
        "\"available_capacity\": 4093",
        StringComparison.Ordinal))),
    "health codec accepted inconsistent Orchestra replay capacity");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.HealthJson.Replace(
        "\"admission_pressure\": \"healthy\"",
        "\"admission_pressure\": \"warning\"",
        StringComparison.Ordinal))),
    "health codec accepted inconsistent Orchestra replay pressure");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.HealthJson.Replace(
        "\"checkpoint_lag_generations\": 0",
        "\"checkpoint_lag_generations\": 1",
        StringComparison.Ordinal))),
    "health codec accepted inconsistent Orchestra checkpoint lag");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.HealthJson.Replace(
        "\"authority_owned\": true",
        "\"authority_owned\": false",
        StringComparison.Ordinal))),
    "health codec accepted an unowned authority");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(Encoding.UTF8.GetBytes(
    Fixtures.HealthJson.Replace(
        "\"effect_queue\": {",
        "\"unexpected\": true, \"effect_queue\": {",
        StringComparison.Ordinal))),
    "health codec accepted an unknown field");
RequireThrows<InvalidDataException>(() => RemoteHealthCodec.Decode(
    new byte[RemoteEventCodec.MaxMessageBytes + 1]),
    "health codec accepted an oversized response");
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
Require(liveState.SnapshotRevision == 7,
    "snapshot did not bind the projection revision");
var heartbeatState = stateMachine.Accept(new RemoteEvent.Heartbeat(7));
Require(heartbeatState.SnapshotGeneration == 1,
    "heartbeat incorrectly advanced the projection generation");
Require(heartbeatState.SnapshotRevision == 7,
    "heartbeat incorrectly advanced the projection revision");
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
    Encoding.UTF8.GetBytes(Encoding.UTF8.GetString(
        Fixtures.InspectResponse(7, "runtime-a")).Replace(
            "\"registered_at_unix_ms\": 1784620800000",
            "\"registered_at_unix_ms\": 1784620800002",
            StringComparison.Ordinal)),
    Fixtures.HistoryResponse(7, "runtime-a"),
    Fixtures.LogsResponse(7, "runtime-a"),
    "runtime-a"),
    "workspace accepted reversed authority timestamps");
RequireThrows<InvalidDataException>(() => RemoteWorkspaceCodec.Compose(
    Encoding.UTF8.GetBytes(Encoding.UTF8.GetString(
        Fixtures.InspectResponse(7, "runtime-a")).Replace(
            "\"registered_at_unix_ms\": 1784620800000",
            "\"sidecar_status\": {\"status_source\": null, \"daemon_status\": \"ready\"}, \"registered_at_unix_ms\": 1784620800000",
            StringComparison.Ordinal)),
    Fixtures.HistoryResponse(7, "runtime-a"),
    Fixtures.LogsResponse(7, "runtime-a"),
    "runtime-a"),
    "workspace accepted an incomplete sidecar status");
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
RemoteWorkspaceCodec.VerifyIncrementalContract();

Console.WriteLine(
    "remote health conformance valid: codec=true, fail_closed=true, queue_consistent=true, orchestra_replay_horizon=true");
Console.WriteLine(
    "remote GUI Leselang export conformance valid: refresh=true, capabilities=true, deployment=true, workspace_queries=true, canonical=true, execution=false");
Console.WriteLine("remote state conformance valid: codec=true, stale=true, snapshot_revision=true, heartbeat_snapshot_fence=true, topology_state=true, authority_bound_topology=true, unproved_live_rejection=true, retained_topology=true, topology_regression_fence=true, runtime_search=true, topology_refresh_coordination=true, workspace_launch_coordination=true, mutation_coordination=true, cached_heartbeat_mutation=false, malformed_mutation_response_unknown=true, shared_failure_classification=true, stale_failure_ignored=true, bounded_failure_diagnostics=true, typed_ui_action_routing=true, opaque_action_node_ids=true, deployment_submission_source_fence=true, event_dispose_single_flight=true, event_resource_release_once=true, event_restart_identity=true, subscriber_failure_isolated=true, subscriber_failure_count_bounded=true, authority_health_coordination=true, health_single_flight=true, health_stop_fence=true, reconnect_attempts=8, manual_resume=true, endpoint_cache=true, credential_resolution=true, trust_identity=true, workspace_atomic=true, logs_bounded=true, endpoint_retained=false, incremental_logs=true");
return 0;

static object RunWorkspaceLogBenchmark()
{
    const int iterations = 500;
    const int fullLogCount = RemoteWorkspaceClient.MaxLogEntries;
    const int incrementalLogCount = 8;
    var inspect = Fixtures.InspectResponse(7, "runtime-a");
    var history = Fixtures.HistoryResponse(7, "runtime-a");
    var fullLogs = Fixtures.LogsResponse(7, "runtime-a", 1, fullLogCount);
    var incrementalLogs = Fixtures.LogsResponse(
        7,
        "runtime-a",
        (ulong)fullLogCount + 1,
        incrementalLogCount);
    var previous = RemoteWorkspaceCodec.Compose(
        inspect,
        history,
        fullLogs,
        "runtime-a");

    for (var warmup = 0; warmup < 50; warmup++)
    {
        _ = RemoteWorkspaceCodec.Compose(inspect, history, fullLogs, "runtime-a");
        _ = RemoteWorkspaceCodec.MergeIncrementalLogs(
            previous,
            RemoteWorkspaceCodec.Compose(
                inspect,
                history,
                incrementalLogs,
                "runtime-a"));
    }

    var full = Measure(iterations, () =>
        RemoteWorkspaceCodec.Compose(inspect, history, fullLogs, "runtime-a"));
    var incremental = Measure(iterations, () =>
        RemoteWorkspaceCodec.MergeIncrementalLogs(
            previous,
            RemoteWorkspaceCodec.Compose(
                inspect,
                history,
                incrementalLogs,
                "runtime-a")));
    if (incremental.LastLogCount != fullLogCount)
    {
        throw new InvalidDataException("incremental benchmark lost its bounded log window");
    }
    return new
    {
        schema_version = 1,
        workload = new
        {
            iterations,
            full_log_count = fullLogCount,
            incremental_log_count = incrementalLogCount,
        },
        metrics = new
        {
            full_snapshot_p50_ms = full.P50Milliseconds,
            incremental_snapshot_p50_ms = incremental.P50Milliseconds,
            incremental_to_full_ratio = incremental.P50Milliseconds / full.P50Milliseconds,
            full_allocated_bytes_per_iteration = full.AllocatedBytesPerIteration,
            incremental_allocated_bytes_per_iteration = incremental.AllocatedBytesPerIteration,
            incremental_allocation_ratio =
                incremental.AllocatedBytesPerIteration / full.AllocatedBytesPerIteration,
            merged_log_count = incremental.LastLogCount,
        },
    };
}

static BenchmarkMeasurement Measure(
    int iterations,
    Func<RemoteWorkspaceSnapshot> operation)
{
    GC.Collect();
    GC.WaitForPendingFinalizers();
    GC.Collect();
    var samples = new double[iterations];
    var allocatedBefore = GC.GetAllocatedBytesForCurrentThread();
    var lastLogCount = 0;
    for (var index = 0; index < iterations; index++)
    {
        var started = Stopwatch.GetTimestamp();
        var snapshot = operation();
        samples[index] = Stopwatch.GetElapsedTime(started).TotalMilliseconds;
        lastLogCount = snapshot.Logs.Count;
    }
    var allocated = GC.GetAllocatedBytesForCurrentThread() - allocatedBefore;
    Array.Sort(samples);
    return new BenchmarkMeasurement(
        samples[iterations / 2],
        (double)allocated / iterations,
        lastLogCount);
}

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

readonly record struct BenchmarkMeasurement(
    double P50Milliseconds,
    double AllocatedBytesPerIteration,
    int LastLogCount);

static class Fixtures
{
public const string UnregistrationReceiptJson = """
{
  "schema_version": 1,
  "response": {
    "kind": "runtime_unregistration_receipt",
    "payload": {
      "command_id": "runtime-unregister-a",
      "receipt": {
        "operation_generation": 15,
        "removed": [
          {
            "runtime_id": "runtime-a",
            "expected_revision": 4
          }
        ],
        "deleted_orchestra_runtime_count": 1,
        "deleted_orchestra_run_count": 2,
        "deleted_orchestra_event_count": 3,
        "removed_at_unix_ms": 1784620800000
      },
      "replay_horizon": {
        "capacity": 256,
        "retained": 12,
        "oldest_generation": 4,
        "newest_generation": 15,
        "next_generation": 16,
        "evicted_through_generation": 3
      }
    }
  }
}
""";

public const string MissingUnregistrationReceiptJson = """
{
  "schema_version": 1,
  "response": {
    "kind": "runtime_unregistration_receipt",
    "payload": {
      "command_id": "runtime-unregister-missing",
      "receipt": null,
      "replay_horizon": {
        "capacity": 256,
        "retained": 12,
        "oldest_generation": 4,
        "newest_generation": 15,
        "next_generation": 16,
        "evicted_through_generation": 3
      }
    }
  }
}
""";

public const string HealthJson = """
{
  "schema_version": 1,
  "response": {
    "kind": "health",
    "payload": {
      "status": "ready",
      "authority_owned": true,
      "protocol_schema_version": 1,
      "effect_queue": {
        "ready": 1,
        "leased": 0,
        "completed": 3,
        "failed": 2,
        "active": 1,
        "terminal": 5,
        "capacity": 10,
        "saturated": false
      },
      "runtime_unregistration_replay_horizon": {
        "capacity": 256,
        "retained": 12,
        "oldest_generation": 4,
        "newest_generation": 15,
        "next_generation": 16,
        "evicted_through_generation": 3
      },
      "orchestra_delete_replay_horizon": {
        "capacity": 4096,
        "retained": 2,
        "available_capacity": 4094,
        "warning_available_capacity": 512,
        "critical_available_capacity": 128,
        "warning_recovery_available_capacity": 768,
        "critical_recovery_available_capacity": 256,
        "checkpoint_lag_generations": 0,
        "saturated": false,
        "admission_state": "ready",
        "admission_pressure": "healthy",
        "oldest_generation": 5,
        "newest_generation": 6,
        "next_generation": 7,
        "evicted_through_generation": 4,
        "protected_from_generation": 5,
        "checkpointed_through_generation": 6
      }
    }
  }
}
""";

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
      "runtime": {{RuntimeJson(
          revision,
          runtimeId,
          extraRuntimeField,
          includeAuthorityTimestamps: true)}}
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

public static byte[] LogsResponse(
    ulong revision,
    string runtimeId,
    ulong firstSequence,
    int count)
{
    var entries = string.Join(",", Enumerable.Range(0, count).Select(index => $$"""
        {
          "sequence": {{firstSequence + (ulong)index}},
          "level": "info",
          "message": "bounded benchmark log"
        }
        """));
    return Encoding.UTF8.GetBytes($$"""
    {
      "schema_version": 1,
      "response": {
        "kind": "query",
        "payload": {
          "kind": "runtime_logs",
          "revision": {{revision}},
          "runtime_id": "{{runtimeId}}",
          "runtime_name": "Runtime A",
          "entries": [{{entries}}]
        }
      }
    }
    """);
}

private static string RuntimeJson(
    ulong revision,
    string runtimeId,
    bool extraRuntimeField = false,
    bool includeAuthorityTimestamps = false) => $$"""
{
  "id": "{{runtimeId}}",
  "name": "Runtime A",
  "endpoint": "unix:///private/runtime-a.sock",
{{(includeAuthorityTimestamps ? "  \"registered_at_unix_ms\": 1784620800000,\n  \"updated_at_unix_ms\": 1784620800001,\n" : string.Empty)}}  "revision": {{revision}},
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
