using System.Text;

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
    if (args.Length == 6 && args[4] != "--refresh")
    {
        Console.Error.WriteLine("usage: Leserpent.RemoteConformance --connect HTTPS_ORIGIN CA_PATH CACHE_PATH [--refresh RUNTIME_ID]");
        return 2;
    }
    var endpoint = args[1];
    var certificate = args[2];
    var cache = args[3];
    var refreshRuntimeId = args.Length == 6 ? args[5] : null;
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
    if (refreshRuntimeId is not null)
    {
        var runtime = live.Runtimes.Single(candidate => candidate.Id == refreshRuntimeId);
        var updated = new TaskCompletionSource<RemoteFeedState>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        client.StateChanged += state =>
        {
            var changedRuntime = state.Runtimes.FirstOrDefault(candidate =>
                candidate.Id == refreshRuntimeId);
            if (state.Phase == RemoteFeedPhase.Live
                && changedRuntime is not null
                && changedRuntime.Revision > runtime.Revision)
            {
                updated.TrySetResult(state);
            }
        };
        using var mutation = new RemoteMutationClient(options);
        var result = await mutation.RefreshAsync(
            refreshRuntimeId,
            runtime.Revision,
            "dotnet-conformance");
        var eventState = await updated.Task.WaitAsync(TimeSpan.FromSeconds(15));
        var eventRuntime = eventState.Runtimes.Single(candidate =>
            candidate.Id == refreshRuntimeId);
        Require(result.Revision == eventRuntime.Revision,
            "mutation response and event stream revisions diverged");
        Require(eventRuntime.RefreshStatus == RefreshStatus.Pending,
            "mutation event did not expose the pending refresh state");
        Console.WriteLine(
            $"remote mutation conformance valid: initial_revision={runtime.Revision}, applied_revision={result.Revision}, event_revision={eventRuntime.Revision}, runtime={eventRuntime.Id}, stale={eventState.IsStale.ToString().ToLowerInvariant()}");
    }
    return 0;
}

if (args.Length != 0)
{
    Console.Error.WriteLine("usage: Leserpent.RemoteConformance [--connect HTTPS_ORIGIN CA_PATH CACHE_PATH [--refresh RUNTIME_ID] | --credential-resolve HTTPS_ORIGIN]");
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

var stateMachine = new RemoteFeedStateMachine();
var liveState = stateMachine.Accept(decodedSnapshot);
Require(!liveState.IsStale && liveState.Phase == RemoteFeedPhase.Live,
    "snapshot did not establish a live state");
var reconnecting = stateMachine.ConnectionLost("test disconnect");
Require(reconnecting.IsStale && reconnecting.Phase == RemoteFeedPhase.Reconnecting,
    "disconnect did not mark cached data stale");
for (var attempt = 2; attempt <= 8; attempt++)
{
    reconnecting = stateMachine.ConnectionLost("test disconnect");
}
Require(reconnecting.Phase == RemoteFeedPhase.Stale && reconnecting.ConsecutiveFailures == 8,
    "reconnect policy did not stop at its attempt bound");

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

Console.WriteLine("remote state conformance valid: codec=true, stale=true, reconnect_attempts=8, endpoint_cache=true, credential_resolution=true");
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
