using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

var root = Path.Combine(Path.GetTempPath(), $"leserpent-mobile-{Guid.NewGuid():N}");
Directory.CreateDirectory(root);
var certificate = Path.Combine(root, "ca.pem");
var cache = Path.Combine(root, "snapshot.json");
using var certificateKey = RSA.Create(2048);
var certificateRequest = new CertificateRequest(
    "CN=Leserpent Mobile Conformance",
    certificateKey,
    HashAlgorithmName.SHA256,
    RSASignaturePadding.Pkcs1);
certificateRequest.CertificateExtensions.Add(new X509BasicConstraintsExtension(
    certificateAuthority: true,
    hasPathLengthConstraint: false,
    pathLengthConstraint: 0,
    critical: true));
using var fixtureCertificate = CreatePublicCertificate(certificateRequest, certificateKey);
var certificatePem = fixtureCertificate.ExportCertificatePem();
File.WriteAllText(certificate, certificatePem);
try
{
    var firstToken = new string('a', 32);
    var secondToken = new string('b', 32);
    var rawStore = new FixtureSecretStore();
    var validatedVault = new MobileCredentialVault(rawStore);
    var firstEndpoint = new Uri("https://mobile.example:9443");
    var secondEndpoint = new Uri("https://other-mobile.example:9443");
    await validatedVault.StoreAsync(firstEndpoint, firstToken, CancellationToken.None);
    Require(await validatedVault.LoadAsync(firstEndpoint, CancellationToken.None) == firstToken,
        "mobile credential did not round-trip through the validating vault");
    Require(await validatedVault.LoadAsync(secondEndpoint, CancellationToken.None) is null,
        "mobile credential alias was not endpoint-isolated");
    Require(!MobileCredentialVault.Alias(firstEndpoint).Contains("mobile.example", StringComparison.Ordinal),
        "mobile credential alias leaked the remote endpoint");
    await RequireThrowsAsync<ArgumentException>(
        () => validatedVault.StoreAsync(firstEndpoint, "invalid token", CancellationToken.None).AsTask(),
        "invalid mobile credential reached the platform store");
    Require(rawStore.StoreCount == 1,
        "invalid mobile credential changed the platform store");
    rawStore.Corrupt(MobileCredentialVault.Alias(firstEndpoint), "corrupt token");
    await RequireThrowsAsync<ArgumentException>(
        () => validatedVault.LoadAsync(firstEndpoint, CancellationToken.None).AsTask(),
        "corrupt platform credential was accepted");
    await validatedVault.DeleteAsync(firstEndpoint, CancellationToken.None);
    Require(await validatedVault.LoadAsync(firstEndpoint, CancellationToken.None) is null,
        "mobile credential delete did not remove the endpoint alias");
    using var cancelled = new CancellationTokenSource();
    cancelled.Cancel();
    var loadCount = rawStore.LoadCount;
    await RequireThrowsAsync<OperationCanceledException>(
        () => validatedVault.LoadAsync(firstEndpoint, cancelled.Token).AsTask(),
        "cancelled mobile credential load was accepted");
    Require(rawStore.LoadCount == loadCount,
        "cancelled mobile credential load reached the platform store");

    var endpointStore = new FixtureEndpointStore();
    var profileStore = new MobileConnectionProfileStore(
        endpointStore,
        Path.Combine(root, "profile"),
        Path.Combine(root, "profile-cache"));
    var savedProfile = profileStore.Save(firstEndpoint.AbsoluteUri, certificatePem);
    Require(savedProfile.Endpoint == "https://mobile.example:9443/"
        && endpointStore.Endpoint == savedProfile.Endpoint
        && File.Exists(savedProfile.CertificateAuthorityPath)
        && !Path.GetFileName(savedProfile.CertificateAuthorityPath).Contains(
            "mobile.example",
            StringComparison.Ordinal)
        && profileStore.CachePath(savedProfile.Endpoint).EndsWith(
            ".json",
            StringComparison.Ordinal),
        "shared mobile connection profile was not canonical and endpoint opaque");
    Require(profileStore.Load() == savedProfile,
        "shared mobile connection profile did not round-trip");
    RequireThrows<ArgumentException>(
        () => profileStore.Save(
            firstEndpoint.AbsoluteUri,
            certificatePem + certificateKey.ExportPkcs8PrivateKeyPem()),
        "mobile connection profile accepted private key material");
    RequireThrows<ArgumentException>(
        () => profileStore.Save(
            firstEndpoint.AbsoluteUri,
            certificatePem + certificateKey.ExportPkcs8PrivateKeyPem().ToLowerInvariant()),
        "mobile connection profile accepted case-shifted private key material");
    endpointStore.Endpoint = "http://mobile.example:9443/";
    Require(profileStore.Load() is null,
        "malformed stored mobile endpoint was accepted");
    endpointStore.Endpoint = savedProfile.Endpoint;
    File.WriteAllText(savedProfile.CertificateAuthorityPath, "corrupt certificate");
    Require(profileStore.Load() is null,
        "corrupt stored mobile certificate was accepted");
    endpointStore.ThrowOnLoad = true;
    Require(profileStore.Load() is null,
        "unavailable native endpoint storage escaped the fail-closed profile boundary");
    endpointStore.ThrowOnLoad = false;
    savedProfile = profileStore.Save(firstEndpoint.AbsoluteUri, certificatePem);
    Require(!Directory.EnumerateFiles(
            Path.GetDirectoryName(savedProfile.CertificateAuthorityPath)!,
            "*.tmp")
        .Any(),
        "mobile connection profile left temporary certificate state");

    var vault = new FixtureVault([firstToken, secondToken]);
    var factory = new FixtureSessionFactory();
    await using var lifecycle = new MobileRemoteLifecycle(
        "https://mobile.example:9443",
        certificate,
        cache,
        vault,
        factory);
    var observed = new List<MobileRemoteLifecycleSnapshot>();
    lifecycle.StateChanged += observed.Add;
    Require(lifecycle.State is { Phase: MobileLifecyclePhase.Inactive, Generation: 0 },
        "mobile lifecycle did not start inactive");

    await lifecycle.EnterForegroundAsync();
    Require(lifecycle.State is
    {
        Phase: MobileLifecyclePhase.Foreground,
        Generation: 1,
        Feed.Phase: RemoteFeedPhase.Live,
        Feed.Revision: 1,
    }, "first foreground session did not become live");
    Require(observed.Any(state => state is
    {
        Phase: MobileLifecyclePhase.Foreground,
        Feed.IsStale: true,
        Feed.Revision: 0,
    }), "foreground transition did not publish hydrated cache state before live state");
    Require(factory.Tokens.SequenceEqual([firstToken]),
        "first foreground session did not receive the first vault token");
    var firstSession = factory.Sessions.Single();

    await lifecycle.EnterBackgroundAsync();
    Require(lifecycle.State is
    {
        Phase: MobileLifecyclePhase.Background,
        Generation: 2,
        Feed.Phase: RemoteFeedPhase.Stale,
    }, "background transition did not suspend the remote feed");
    Require(firstSession.DisposeCount == 1,
        "background transition did not dispose the active session exactly once");
    firstSession.EmitCaptured(Live(99));
    Require(lifecycle.State.Feed.Revision == 1,
        "retired session crossed the generation fence");

    await lifecycle.EnterForegroundAsync();
    Require(lifecycle.State is
    {
        Phase: MobileLifecyclePhase.Foreground,
        Generation: 3,
        Feed.Phase: RemoteFeedPhase.Live,
        Feed.Revision: 2,
    }, "foreground reentry did not establish a fresh session");
    Require(vault.LoadCount == 2 && factory.Tokens.SequenceEqual([firstToken, secondToken]),
        "foreground reentry did not reload the credential");
    RequireThrows<InvalidOperationException>(
        () => lifecycle.EnterForegroundAsync().AsTask().GetAwaiter().GetResult(),
        "duplicate foreground transition was accepted");

    await lifecycle.DisposeAsync();
    Require(lifecycle.State is { Phase: MobileLifecyclePhase.Stopped, Generation: 4 },
        "mobile lifecycle did not stop with a terminal generation");
    Require(factory.Sessions[1].DisposeCount == 1,
        "terminal disposal did not release the resumed session exactly once");
    await lifecycle.DisposeAsync();
    Require(factory.Sessions[1].DisposeCount == 1,
        "terminal disposal was not idempotent");

    var emptyVault = new FixtureVault([null]);
    await using var missing = new MobileRemoteLifecycle(
        "https://mobile.example:9443",
        certificate,
        cache,
        emptyVault,
        new FixtureSessionFactory());
    await RequireThrowsAsync<InvalidDataException>(
        () => missing.EnterForegroundAsync().AsTask(),
        "missing mobile credential was accepted");

    var failingFactory = new FixtureSessionFactory(failOnStart: true);
    await using var failing = new MobileRemoteLifecycle(
        "https://mobile.example:9443",
        certificate,
        cache,
        new FixtureVault([firstToken]),
        failingFactory);
    RequireThrows<InvalidOperationException>(
        () => failing.EnterForegroundAsync().AsTask().GetAwaiter().GetResult(),
        "session startup failure was hidden");
    Require(failing.State.Phase == MobileLifecyclePhase.Background
        && failingFactory.Sessions.Single().DisposeCount == 1,
        "session startup failure did not return to a cleaned background state");

    var coordinatorStore = new FixtureSecretStore();
    var coordinatorFactory = new FixtureSessionFactory();
    var coordinator = new MobileApplicationCoordinator(
        new MobileCredentialVault(coordinatorStore),
        coordinatorFactory);
    var applicationStates = new List<MobileApplicationSnapshot>();
    coordinator.StateChanged += applicationStates.Add;
    await coordinator.ConfigureAsync(
        firstEndpoint.AbsoluteUri,
        certificate,
        cache,
        firstToken);
    Require(coordinator.State.Phase == MobileApplicationPhase.Inactive
        && coordinatorStore.StoreCount == 1,
        "application coordinator did not securely configure the endpoint");
    await coordinator.EnterForegroundAsync();
    await coordinator.EnterForegroundAsync();
    Require(coordinator.State is
    {
        Phase: MobileApplicationPhase.Foreground,
        Remote.Feed.Phase: RemoteFeedPhase.Live,
    } && coordinatorFactory.Sessions.Count == 1,
        "duplicate application foreground callback created another session");
    await coordinator.EnterBackgroundAsync();
    await coordinator.EnterBackgroundAsync();
    Require(coordinator.State.Phase == MobileApplicationPhase.Background
        && coordinatorFactory.Sessions.Single().DisposeCount == 1,
        "duplicate application background callback changed session ownership");
    await coordinator.ConfigureAsync(
        secondEndpoint.AbsoluteUri,
        certificate,
        cache,
        secondToken);
    await coordinator.EnterForegroundAsync();
    Require(coordinatorFactory.Sessions.Count == 2
        && coordinatorFactory.Tokens.SequenceEqual([firstToken, secondToken])
        && applicationStates.Any(state => state.Phase == MobileApplicationPhase.Inactive),
        "application reconfiguration did not replace endpoint-bound ownership");

    MobileUiDocumentBinding.VerifyContract();
    var currentFeed = coordinator.State.Remote?.Feed
        ?? throw new InvalidDataException("mobile application lost its remote feed");
    var fleetBinding = MobileUiDocumentBinding.Project(
        RemoteDocumentProjection.Project(currentFeed).Document);
    var inspectNode = Descendants(fleetBinding.Root).Single(node =>
        node.ActionKind == ActionKind.RuntimeInspect);
    var inspect = fleetBinding.ResolveActivation(
        inspectNode.Id,
        currentFeed,
        coordinator.MutationAvailability);
    Require(inspect is { Accepted: true, Intent.Kind: ActionKind.RuntimeInspect },
        "mobile native document binding did not admit typed inspect");
    var workspace = await coordinator.LoadWorkspaceAsync(inspect.Intent!.Runtime.Id);
    Require(workspace.Runtime.Id == "runtime-a"
        && coordinatorFactory.Sessions[1].WorkspaceLoadCount == 1
        && coordinatorFactory.Sessions[1].Principals.SequenceEqual(["leserpent-mobile"]),
        "mobile workspace query did not stay inside the foreground session");
    var workspaceBinding = MobileUiDocumentBinding.Project(
        RemoteWorkspaceDocumentProjection.Project(workspace));
    var deployNode = Descendants(workspaceBinding.Root).Single(node =>
        node.ActionKind == ActionKind.RuntimeDeploy);
    var deployment = workspaceBinding.ResolveSubmission(
        deployNode.Id,
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["pipeline_kind"] = "http/request",
            ["target"] = "pid:42",
        },
        currentFeed);
    Require(deployment is
    {
        Accepted: true,
        Intent:
        {
            Kind: ActionKind.RuntimeDeploy,
            PipelineKind: "http/request",
            Target: "pid:42",
        },
    }, "mobile parameterized form event did not resolve to a typed deployment");
    var mutation = await coordinator.ExecuteMutationAsync(deployment.Intent!);
    Require(mutation is { RuntimeId: "runtime-a", Revision: 3, Status: "applied" }
        && coordinatorFactory.Sessions[1].Mutations is
        [
        {
            Intent.Kind: ActionKind.RuntimeDeploy,
            Intent.PipelineKind: "http/request",
            Intent.Target: "pid:42",
            Principal: "leserpent-mobile",
        },
        ]
        && coordinator.MutationAvailability.MutationsEnabled,
        "mobile typed deployment did not complete and release its observed revision fence");

    var activeSession = coordinatorFactory.Sessions[1];
    activeSession.DeferNextWorkspace = true;
    var retiredWorkspace = coordinator.LoadWorkspaceAsync("runtime-a");
    await coordinator.EnterBackgroundAsync();
    activeSession.CompleteDeferredWorkspace();
    await RequireThrowsAsync<MobileRemoteGenerationRetiredException>(
        () => retiredWorkspace,
        "workspace result crossed a retired mobile foreground generation");
    await coordinator.DisposeAsync();
    await coordinator.DisposeAsync();
    Require(coordinator.State.Phase == MobileApplicationPhase.Stopped
        && coordinatorFactory.Sessions[1].DisposeCount == 1,
        "application coordinator disposal was not terminal and idempotent");
    await RequireThrowsAsync<ObjectDisposedException>(
        () => coordinator.ConfigureAsync(
            firstEndpoint.AbsoluteUri,
            certificate,
            cache,
            firstToken).AsTask(),
        "stopped application coordinator accepted reconfiguration");

    RemoteWorkspaceLogFilter.VerifyContract();
    MobileLayoutPolicy.VerifyContract();
    RemoteWorkspaceDiagnosticExport.VerifyContract();
    RemoteWorkspaceLiveRefresh.VerifyContract();
    RemoteWorkspaceLogRefreshPlan.VerifyContract();
    RemoteWorkspaceSeverityAlert.VerifyContract();
    RemoteWorkspaceSnapshotChanges.VerifyContract();
    RemoteRuntimeSearch.VerifyContract();
    await RemoteTopologyRefreshCoordinator.VerifyContractAsync();
    RemoteWorkspaceLaunchCoordinator.VerifyContract();
    RemoteDocumentProjection.VerifyFilterContract();
    RemoteWorkspaceDocumentProjection.VerifyEndpointIsolation();
    RemoteWorkspaceDocumentProjection.VerifyParameterizedFormContract();
    RemoteMutationFences.VerifyContract();
    RemoteMutationCoordinator.VerifyContract();
    RemoteUiActionRouter.VerifyContract();
    await RemoteEventClient.VerifyLifecycleContractAsync();
    RemoteAuthorityHealthPresentation.VerifyContract();
    await RemoteAuthorityHealthCoordinator.VerifyContractAsync();
}
finally
{
    Directory.Delete(root, recursive: true);
}

Console.WriteLine("mobile lifecycle conformance valid: foreground=true, background_disconnect=true, credential_reload=true, generation_fence=true, failure_cleanup=true, application_entry=true, duplicate_callbacks=true, reconfigure=true, shared_connection_profile=true, atomic_ca_profile=true, malformed_profile_fail_closed=true, unavailable_profile_storage_fail_closed=true, keychain_independent_certificate_fixture=true, workspace_policy=true, runtime_search=true, topology_refresh=true, workspace_launch=true, ui_projection=true, mobile_ui_document_binding=true, immutable_native_projection=true, exact_native_presentation_equivalence=true, heartbeat_stable_native_render=true, native_render_state_fence=true, native_parameterized_form=true, native_form_event_routing=true, native_workspace_query=true, native_typed_deployment=true, mobile_operation_generation_fence=true, mutation_fence=true, mutation_coordination=true, cached_heartbeat_mutation=false, shared_failure_classification=true, stale_failure_ignored=true, bounded_failure_diagnostics=true, typed_ui_action_routing=true, opaque_action_node_ids=true, deployment_submission_source_fence=true, event_dispose_single_flight=true, event_resource_release_once=true, event_restart_identity=true, subscriber_failure_isolated=true, subscriber_failure_count_bounded=true, action_availability=true, authority_health=true, authority_health_coordination=true, health_single_flight=true, health_stop_fence=true, mobile_layout_policy=true, value_layout_plan=true, width_classes=3, safe_area=true, font_scale_fence=true, minimum_touch_dp=48, expanded_two_pane=true, runtime_columns=2");
return 0;

static X509Certificate2 CreatePublicCertificate(
    CertificateRequest request,
    RSA key)
{
    var serialNumber = RandomNumberGenerator.GetBytes(16);
    serialNumber[0] &= 0x7F;
    serialNumber[^1] |= 1;
    try
    {
        return request.Create(
            request.SubjectName,
            X509SignatureGenerator.CreateForRSA(key, RSASignaturePadding.Pkcs1),
            DateTimeOffset.UtcNow.AddMinutes(-1),
            DateTimeOffset.UtcNow.AddDays(1),
            serialNumber);
    }
    finally
    {
        CryptographicOperations.ZeroMemory(serialNumber);
    }
}

static IEnumerable<MobileUiNodeBinding> Descendants(MobileUiNodeBinding node)
{
    yield return node;
    foreach (var child in node.Children)
    {
        foreach (var descendant in Descendants(child))
        {
            yield return descendant;
        }
    }
}

static RemoteFeedState Live(ulong revision) => new(
    RemoteFeedPhase.Live,
    revision,
    Array.Empty<RemoteRuntimeProjection>(),
    0,
    false,
    $"Live at revision {revision}");

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

static async Task RequireThrowsAsync<TException>(Func<Task> action, string message)
    where TException : Exception
{
    try
    {
        await action();
    }
    catch (TException)
    {
        return;
    }
    throw new InvalidDataException(message);
}

sealed class FixtureVault(IReadOnlyList<string?> tokens) : IMobileCredentialVault
{
    public int LoadCount { get; private set; }

    public ValueTask<string?> LoadAsync(Uri endpoint, CancellationToken cancellationToken)
    {
        _ = endpoint;
        cancellationToken.ThrowIfCancellationRequested();
        var index = LoadCount++;
        return ValueTask.FromResult(index < tokens.Count ? tokens[index] : tokens[^1]);
    }

    public ValueTask StoreAsync(
        Uri endpoint,
        string token,
        CancellationToken cancellationToken) =>
        throw new NotSupportedException();

    public ValueTask DeleteAsync(Uri endpoint, CancellationToken cancellationToken) =>
        throw new NotSupportedException();
}

sealed class FixtureSecretStore : IMobileSecretStore
{
    private readonly Dictionary<string, string> values = new(StringComparer.Ordinal);
    public int LoadCount { get; private set; }
    public int StoreCount { get; private set; }

    public ValueTask<string?> LoadAsync(string alias, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        LoadCount++;
        return ValueTask.FromResult(values.GetValueOrDefault(alias));
    }

    public ValueTask StoreAsync(
        string alias,
        string secret,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        values[alias] = secret;
        StoreCount++;
        return ValueTask.CompletedTask;
    }

    public ValueTask DeleteAsync(string alias, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        values.Remove(alias);
        return ValueTask.CompletedTask;
    }

    public void Corrupt(string alias, string value) => values[alias] = value;
}

sealed class FixtureEndpointStore : IMobileEndpointStore
{
    public string? Endpoint { get; set; }
    public bool ThrowOnLoad { get; set; }

    public string? Load() => ThrowOnLoad
        ? throw new InvalidOperationException("fixture endpoint storage unavailable")
        : Endpoint;

    public void Save(string endpoint) => Endpoint = endpoint;
}

sealed class FixtureSessionFactory(bool failOnStart = false) : IMobileRemoteSessionFactory
{
    public List<string> Tokens { get; } = [];
    public List<FixtureSession> Sessions { get; } = [];

    public IMobileRemoteSession Create(RemoteClientOptions options)
    {
        Tokens.Add(options.Token);
        var session = new FixtureSession((ulong)Sessions.Count + 1, failOnStart);
        Sessions.Add(session);
        return session;
    }
}

sealed class FixtureSession(ulong revision, bool failOnStart) : IMobileRemoteSession
{
    private Action<RemoteFeedState>? stateChanged;
    private readonly List<Action<RemoteFeedState>> captured = [];
    private TaskCompletionSource<RemoteWorkspaceSnapshot>? deferredWorkspace;

    public sealed record MutationInvocation(
        RemoteUiActionIntent Intent,
        string Principal);

    public event Action<RemoteFeedState>? StateChanged
    {
        add
        {
            stateChanged += value;
            if (value is not null)
            {
                captured.Add(value);
            }
        }
        remove => stateChanged -= value;
    }

    public RemoteFeedState State { get; private set; } = new(
        RemoteFeedPhase.Connecting,
        0,
        Array.Empty<RemoteRuntimeProjection>(),
        0,
        true,
        "Showing cached revision 0; connecting");
    public int DisposeCount { get; private set; }
    public int WorkspaceLoadCount { get; private set; }
    public bool DeferNextWorkspace { get; set; }
    public List<string> Principals { get; } = [];
    public List<MutationInvocation> Mutations { get; } = [];

    public void Start()
    {
        if (failOnStart)
        {
            throw new InvalidOperationException("fixture start failure");
        }
        State = Live(revision);
        stateChanged?.Invoke(State);
    }

    public void EmitCaptured(RemoteFeedState state) => captured.Single().Invoke(state);

    public Task<RemoteWorkspaceSnapshot> LoadWorkspaceAsync(
        string runtimeId,
        string principal,
        CancellationToken cancellationToken)
    {
        _ = cancellationToken;
        if (runtimeId != "runtime-a")
        {
            throw new ArgumentException("fixture runtime is invalid", nameof(runtimeId));
        }
        WorkspaceLoadCount++;
        Principals.Add(principal);
        var workspace = Workspace(State.Runtimes.Single());
        if (!DeferNextWorkspace)
        {
            return Task.FromResult(workspace);
        }
        DeferNextWorkspace = false;
        deferredWorkspace = new TaskCompletionSource<RemoteWorkspaceSnapshot>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        return deferredWorkspace.Task;
    }

    public Task<RemoteMutationResult> ExecuteMutationAsync(
        RemoteUiActionIntent intent,
        string principal,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        Mutations.Add(new MutationInvocation(intent, principal));
        var nextRevision = checked(intent.Runtime.Revision + 1);
        State = Live(nextRevision);
        stateChanged?.Invoke(State);
        return Task.FromResult(new RemoteMutationResult(
            "mobile-fixture-command",
            intent.Runtime.Id,
            nextRevision,
            "applied"));
    }

    public void CompleteDeferredWorkspace()
    {
        var completion = deferredWorkspace
            ?? throw new InvalidOperationException("no deferred workspace is pending");
        deferredWorkspace = null;
        completion.SetResult(Workspace(Runtime(revision)));
    }

    public ValueTask DisposeAsync()
    {
        DisposeCount++;
        return ValueTask.CompletedTask;
    }

    private static RemoteFeedState Live(ulong value) => new(
        RemoteFeedPhase.Live,
        value,
        [Runtime(value)],
        0,
        false,
        $"Live at revision {value}",
        value,
        value);

    private static RemoteRuntimeProjection Runtime(ulong value) => new()
    {
        Id = "runtime-a",
        Name = "Runtime A",
        Revision = value,
        Tags = new RuntimeTags { Environment = "test" },
        Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
        Capabilities = new RuntimeCapabilitySnapshot
        {
            Source = "gewyvern-api",
            Service = "gewyvern",
            Version = "1.16.0",
            AuthenticatedDeployment = true,
        },
        CapabilitiesObservedForRevision = value,
    };

    private static RemoteWorkspaceSnapshot Workspace(RemoteRuntimeProjection runtime) => new(
        runtime.Revision,
        runtime,
        [new RemoteHistoryProjection("fixture-command", runtime.Revision, "applied")],
        [new RemoteLogProjection(1, "info", "fixture workspace ready")]);
}
