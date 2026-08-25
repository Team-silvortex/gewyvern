using System.Net;
using System.Text;
using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeRegistrationExecutionServiceTests
{
    private const string RuntimeEndpoint = "http://127.0.0.1:49152";
    private const string SidecarEndpoint = "http://127.0.0.1:49154";
    private const string PairingToken = "runtime-pairing-secret";
    private const string SidecarAdminToken = "sidecar-admin-secret";

    [Fact]
    public async Task AuthorityUpdateUsesReviewedRevisionAndCredentialBoundDiscovery()
    {
        using var fixture = CreateFixture(
            enabled: true,
            Projection(7),
            new DiscoveryHandler());
        fixture.Registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Managed Stale",
                "http://127.0.0.1:49153",
                "old-secret"),
            "runtime-coordinator");
        var previewRequest = new RuntimeRegistrationPlanRequest(
            "Runtime A",
            RuntimeEndpoint);
        var plan = await fixture.Plans.BuildAsync(
            previewRequest,
            CancellationToken.None);
        var request = new RuntimeRegistrationRequest(
            previewRequest.Name,
            previewRequest.Endpoint,
            PairingToken,
            FetchCapabilities: true,
            RegistrationPlanToken: plan.PlanToken);

        var registered = await fixture.Registrations.ExecuteAsync(
            request,
            CancellationToken.None);

        Assert.Equal("runtime-coordinator", registered.RuntimeId);
        Assert.Equal(7UL, fixture.Authority.ExpectedRevision);
        Assert.True(fixture.Authority.Update);
        Assert.Equal(1, fixture.Authority.RegisterCalls);
        Assert.NotNull(fixture.Authority.CapabilityDiscovery?.AuthoritySnapshot);
        Assert.NotNull(fixture.Authority.StatusDiscovery);
        Assert.Equal(3, fixture.Discovery.Requests.Count);
        Assert.All(
            fixture.Discovery.AdminTokens,
            token => Assert.Equal(PairingToken, token));
        Assert.Equal(9UL, fixture.Authority.Runtime?.Revision);
        Assert.Single(fixture.Registry.ListRuntimes());
        Assert.Equal(
            PairingToken,
            fixture.Registry.GetRuntimeControlAccess(
                registered.RuntimeId)?.AdminToken);
    }

    [Fact]
    public async Task MissingAuthorityPlanFailsBeforeDiscoveryOrDaemonMutation()
    {
        using var fixture = CreateFixture(
            enabled: true,
            Projection(11),
            new DiscoveryHandler());
        var request = new RuntimeRegistrationRequest(
            "Runtime A",
            RuntimeEndpoint,
            PairingToken,
            FetchCapabilities: true);

        var error = await Assert.ThrowsAsync<
            RuntimeRegistrationExecutionException>(() =>
                fixture.Registrations.ExecuteAsync(
                    request,
                    CancellationToken.None));

        Assert.Equal(
            RuntimeRegistrationExecutionFailureKind.Conflict,
            error.Kind);
        Assert.Equal("runtime_registration_plan_required", error.Code);
        Assert.Equal("runtime-coordinator", error.RuntimeId);
        Assert.Empty(fixture.Discovery.Requests);
        Assert.Equal(0, fixture.Authority.RegisterCalls);
        Assert.Empty(fixture.Registry.ListRuntimes());
        Assert.DoesNotContain(PairingToken, error.ToString());
    }

    [Fact]
    public async Task AdvancedDaemonRevisionRejectsStalePlanBeforeEffects()
    {
        using var fixture = CreateFixture(
            enabled: true,
            Projection(13),
            new DiscoveryHandler());
        var previewRequest = new RuntimeRegistrationPlanRequest(
            "Runtime A",
            RuntimeEndpoint);
        var plan = await fixture.Plans.BuildAsync(
            previewRequest,
            CancellationToken.None);
        fixture.Authority.Runtime = fixture.Authority.Runtime! with
        {
            Revision = 14,
        };
        var request = new RuntimeRegistrationRequest(
            previewRequest.Name,
            previewRequest.Endpoint,
            PairingToken,
            FetchCapabilities: true,
            RegistrationPlanToken: plan.PlanToken);

        var error = await Assert.ThrowsAsync<
            RuntimeRegistrationExecutionException>(() =>
                fixture.Registrations.ExecuteAsync(
                    request,
                    CancellationToken.None));

        Assert.Equal("runtime_registration_plan_changed", error.Code);
        Assert.Empty(fixture.Discovery.Requests);
        Assert.Equal(0, fixture.Authority.RegisterCalls);
        Assert.Empty(fixture.Registry.ListRuntimes());
    }

    [Fact]
    public async Task UnconfiguredAuthorityPreservesManagedRegistrationFallback()
    {
        using var fixture = CreateFixture(
            enabled: false,
            runtime: null,
            new DiscoveryHandler());

        var registered = await fixture.Registrations.ExecuteAsync(
            new RuntimeRegistrationRequest(
                "Managed Runtime",
                RuntimeEndpoint,
                PairingToken),
            CancellationToken.None);

        Assert.NotEmpty(registered.RuntimeId);
        Assert.Equal("Managed Runtime", registered.Name);
        Assert.Equal(0, fixture.Authority.SnapshotCalls);
        Assert.Equal(0, fixture.Authority.RegisterCalls);
        Assert.Empty(fixture.Discovery.Requests);
        Assert.Single(fixture.Registry.ListRuntimes());
    }

    [Fact]
    public async Task ConcurrentManagedRegistrationHasOneCredentialOwner()
    {
        var discovery = new DiscoveryHandler
        {
            RequestEntered = new TaskCompletionSource<bool>(
                TaskCreationOptions.RunContinuationsAsynchronously),
            RequestRelease = new TaskCompletionSource<bool>(
                TaskCreationOptions.RunContinuationsAsynchronously),
        };
        using var fixture = CreateFixture(
            enabled: false,
            runtime: null,
            discovery);
        const string winningCredential = "managed-winning-secret";
        const string losingCredential = "managed-losing-secret";
        var winningTask = fixture.Registrations.ExecuteAsync(
            new RuntimeRegistrationRequest(
                "Managed Runtime",
                RuntimeEndpoint,
                winningCredential,
                FetchCapabilities: true),
            CancellationToken.None);
        await discovery.RequestEntered.Task.WaitAsync(
            TimeSpan.FromSeconds(5));

        var losingTask = fixture.Registrations.ExecuteAsync(
            new RuntimeRegistrationRequest(
                "managed runtime",
                RuntimeEndpoint,
                losingCredential,
                FetchCapabilities: true),
            CancellationToken.None);
        RuntimeRegistrationExecutionException conflict;
        try
        {
            var losingCompletion = await Task.WhenAny(
                losingTask,
                Task.Delay(TimeSpan.FromSeconds(5)));
            Assert.Same(losingTask, losingCompletion);
            conflict = await Assert.ThrowsAsync<
                RuntimeRegistrationExecutionException>(() => losingTask);
        }
        finally
        {
            discovery.RequestRelease.TrySetResult(true);
        }

        Assert.Equal("runtime_registration_in_progress", conflict.Code);
        var registered = await winningTask;
        Assert.Equal(3, discovery.Requests.Count);
        Assert.Single(fixture.Registry.ListRuntimes());
        Assert.Equal(
            winningCredential,
            fixture.Registry.GetRuntimeControlAccess(
                registered.RuntimeId)?.AdminToken);
        Assert.DoesNotContain(
            losingCredential,
            await File.ReadAllTextAsync(fixture.StatePath),
            StringComparison.Ordinal);
    }

    [Fact]
    public async Task ManagedRegistrationAndDeletionAreMutuallyExclusive()
    {
        var entered = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var release = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var discovery = new DiscoveryHandler
        {
            RequestEntered = entered,
            RequestRelease = release,
        };
        using var fixture = CreateFixture(
            enabled: false,
            runtime: null,
            discovery);
        var existing = fixture.Registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Managed Runtime",
                RuntimeEndpoint,
                "managed-old-secret"));
        const string winningCredential = "managed-updated-secret";
        var registrationTask = fixture.Registrations.ExecuteAsync(
            new RuntimeRegistrationRequest(
                existing.Name,
                existing.Endpoint,
                winningCredential,
                FetchCapabilities: true),
            CancellationToken.None);
        await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));

        try
        {
            var conflict = Assert.Throws<
                RuntimeRegistrationInProgressException>(() =>
                    fixture.Registry.ReserveRuntimeDeletion(
                        new[] { existing.RuntimeId }));
            Assert.Equal(new[] { existing.RuntimeId }, conflict.RuntimeIds);
            Assert.Empty(fixture.Registry.ListPendingRuntimeDeletions());
        }
        finally
        {
            release.TrySetResult(true);
        }

        var registered = await registrationTask;
        Assert.Equal(3, discovery.Requests.Count);
        Assert.Equal(
            winningCredential,
            fixture.Registry.GetRuntimeControlAccess(
                registered.RuntimeId)?.AdminToken);

        using var deletion = fixture.Registry.ReserveRuntimeDeletion(
            new[] { registered.RuntimeId });
        var blocked = await Assert.ThrowsAsync<
            RuntimeRegistrationExecutionException>(() =>
                fixture.Registrations.ExecuteAsync(
                    new RuntimeRegistrationRequest(
                        registered.Name,
                        registered.Endpoint,
                        "managed-blocked-secret",
                        FetchCapabilities: true),
                    CancellationToken.None));
        Assert.Equal("runtime_registration_plan_changed", blocked.Code);
        Assert.False(blocked.Plan?.Allowed);
        Assert.Equal(
            RuntimeRegistrationPolicy.RuntimeDeletionInProgressReason,
            blocked.Plan?.Reason);
        Assert.Equal(3, discovery.Requests.Count);
        Assert.Equal(0, fixture.Authority.RegisterCalls);
        Assert.Equal(
            winningCredential,
            fixture.Registry.GetRuntimeControlAccess(
                registered.RuntimeId)?.AdminToken);
    }

    [Fact]
    public async Task AmbiguousAuthorityResponseReplaysExactPersistedIntent()
    {
        using var fixture = CreateFixture(
            enabled: true,
            Projection(7),
            new DiscoveryHandler());
        var preview = new RuntimeRegistrationPlanRequest(
            "Runtime A",
            RuntimeEndpoint);
        var plan = await fixture.Plans.BuildAsync(
            preview,
            CancellationToken.None);
        fixture.Authority.AmbiguousFailuresRemaining = 1;

        var registered = await fixture.Registrations.ExecuteAsync(
            new RuntimeRegistrationRequest(
                preview.Name,
                preview.Endpoint,
                PairingToken,
                FetchCapabilities: true,
                RegistrationPlanToken: plan.PlanToken),
            CancellationToken.None);

        Assert.Equal("runtime-coordinator", registered.RuntimeId);
        Assert.Equal(2, fixture.Authority.RegisterCalls);
        Assert.All(
            fixture.Authority.ExpectedRevisions,
            revision => Assert.Equal(7UL, revision));
        Assert.Single(fixture.Authority.CommandIds.Distinct());
        Assert.Equal(3, fixture.Discovery.Requests.Count);
        Assert.Empty(fixture.Registry.ListPendingRuntimeRegistrations());
        Assert.Equal(9UL, fixture.Authority.Runtime?.Revision);
    }

    [Fact]
    public async Task RepeatedAmbiguityPersistsSecretFreeIntentAndBlocksChanges()
    {
        using var fixture = CreateFixture(
            enabled: true,
            Projection(17),
            new DiscoveryHandler());
        fixture.Registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Runtime A",
                RuntimeEndpoint,
                "managed-existing-secret"),
            "runtime-coordinator");
        var preview = new RuntimeRegistrationPlanRequest(
            "Runtime A",
            RuntimeEndpoint,
            SidecarEndpoint);
        var plan = await fixture.Plans.BuildAsync(
            preview,
            CancellationToken.None);
        var request = new RuntimeRegistrationRequest(
            preview.Name,
            preview.Endpoint,
            PairingToken,
            Tags: new RuntimeTags("prod", "alpha", "capture"),
            FetchCapabilities: true,
            SidecarEndpoint: SidecarEndpoint,
            SidecarAdminToken: SidecarAdminToken,
            RegistrationPlanToken: plan.PlanToken);
        fixture.Authority.AmbiguousFailuresRemaining = 2;

        var error = await Assert.ThrowsAsync<
            RuntimeRegistrationExecutionException>(() =>
                fixture.Registrations.ExecuteAsync(
                    request,
                    CancellationToken.None));

        Assert.Equal("runtime_registration_outcome_ambiguous", error.Code);
        var intent = Assert.Single(
            fixture.Registry.ListPendingRuntimeRegistrations());
        Assert.Equal(2, intent.AttemptCount);
        Assert.Equal("daemon_transport_failed", intent.LastFailureCode);
        Assert.Equal(2, fixture.Authority.RegisterCalls);
        Assert.Single(fixture.Authority.CommandIds.Distinct());
        var persisted = await File.ReadAllTextAsync(fixture.StatePath);
        Assert.Contains(intent.CommandId, persisted, StringComparison.Ordinal);
        Assert.DoesNotContain(PairingToken, persisted, StringComparison.Ordinal);
        Assert.DoesNotContain(
            SidecarAdminToken,
            persisted,
            StringComparison.Ordinal);
        Assert.DoesNotContain(
            plan.PlanToken,
            persisted,
            StringComparison.Ordinal);

        var deletionConflict = Assert.Throws<
            RuntimeRegistrationInProgressException>(() =>
                fixture.Registry.ReserveRuntimeDeletion(
                    new[] { intent.RuntimeId }));
        Assert.Equal(new[] { intent.RuntimeId }, deletionConflict.RuntimeIds);
        Assert.Empty(fixture.Registry.ListPendingRuntimeDeletions());

        var changed = request with
        {
            Tags = new RuntimeTags("prod", "beta", "capture"),
        };
        var conflict = await Assert.ThrowsAsync<
            RuntimeRegistrationExecutionException>(() =>
                fixture.Registrations.ExecuteAsync(
                    changed,
                    CancellationToken.None));
        Assert.Equal("runtime_registration_recovery_pending", conflict.Code);
        Assert.Equal(2, fixture.Authority.RegisterCalls);

        var tamperedState = new PersistedControlPlaneState(
            9,
            DateTimeOffset.UtcNow,
            Array.Empty<PersistedRuntimeState>(),
            Array.Empty<PersistedSessionState>(),
            PendingRuntimeRegistrations:
            [
                intent with { CommandId = new string('0', 32) },
            ]);
        Assert.Throws<InvalidDataException>(() =>
            ControlPlaneStateValidator.Validate(tamperedState));
    }

    [Fact]
    public async Task RestartRecoversPersistedIntentWithoutRediscovery()
    {
        using var fixture = CreateFixture(
            enabled: true,
            Projection(23),
            new DiscoveryHandler());
        var preview = new RuntimeRegistrationPlanRequest(
            "Runtime A",
            RuntimeEndpoint);
        var originalPlan = await fixture.Plans.BuildAsync(
            preview,
            CancellationToken.None);
        var originalRequest = new RuntimeRegistrationRequest(
            preview.Name,
            preview.Endpoint,
            PairingToken,
            FetchCapabilities: true,
            RegistrationPlanToken: originalPlan.PlanToken);
        fixture.Authority.AmbiguousFailuresRemaining = 2;
        _ = await Assert.ThrowsAsync<RuntimeRegistrationExecutionException>(
            () => fixture.Registrations.ExecuteAsync(
                originalRequest,
                CancellationToken.None));
        var snapshotCallsBeforeRestart = fixture.Authority.SnapshotCalls;
        var commandId = Assert.Single(
            fixture.Registry.ListPendingRuntimeRegistrations()).CommandId;

        var restartDiscovery = new DiscoveryHandler();
        using var restarted = CreateFixture(
            enabled: true,
            fixture.Authority.Runtime,
            restartDiscovery,
            fixture.StatePath,
            fixture.Authority);
        var recoveryPlan = await restarted.Plans.BuildAsync(
            preview,
            CancellationToken.None);

        Assert.True(recoveryPlan.Allowed);
        Assert.Equal(
            RuntimeRegistrationPolicy
                .RuntimeRegistrationRecoveryPendingReason,
            recoveryPlan.Reason);
        Assert.Equal(23UL, recoveryPlan.ExpectedRevision);
        Assert.Equal(originalPlan.PlanToken, recoveryPlan.PlanToken);
        Assert.Equal(
            snapshotCallsBeforeRestart,
            fixture.Authority.SnapshotCalls);

        fixture.Authority.AmbiguousFailuresRemaining = 0;
        const string refreshedCredential = "runtime-refreshed-secret";
        var registered = await restarted.Registrations.ExecuteAsync(
            originalRequest with
            {
                PairingToken = refreshedCredential,
                RegistrationPlanToken = recoveryPlan.PlanToken,
            },
            CancellationToken.None);

        Assert.Equal("runtime-coordinator", registered.RuntimeId);
        Assert.Equal(3, fixture.Authority.RegisterCalls);
        Assert.All(
            fixture.Authority.ExpectedRevisions,
            revision => Assert.Equal(23UL, revision));
        Assert.All(
            fixture.Authority.CommandIds,
            replayed => Assert.Equal(commandId, replayed));
        Assert.Empty(restartDiscovery.Requests);
        Assert.Empty(restarted.Registry.ListPendingRuntimeRegistrations());
        Assert.Equal(
            refreshedCredential,
            restarted.Registry.GetRuntimeControlAccess(
                registered.RuntimeId)?.AdminToken);
    }

    [Fact]
    public async Task ConcurrentRecoveryHasOneCredentialAndMutationOwner()
    {
        using var fixture = CreateFixture(
            enabled: true,
            Projection(29),
            new DiscoveryHandler());
        var preview = new RuntimeRegistrationPlanRequest(
            "Runtime A",
            RuntimeEndpoint);
        var originalPlan = await fixture.Plans.BuildAsync(
            preview,
            CancellationToken.None);
        var originalRequest = new RuntimeRegistrationRequest(
            preview.Name,
            preview.Endpoint,
            PairingToken,
            FetchCapabilities: true,
            RegistrationPlanToken: originalPlan.PlanToken);
        fixture.Authority.AmbiguousFailuresRemaining = 2;
        _ = await Assert.ThrowsAsync<RuntimeRegistrationExecutionException>(
            () => fixture.Registrations.ExecuteAsync(
                originalRequest,
                CancellationToken.None));
        var recoveryPlan = await fixture.Plans.BuildAsync(
            preview,
            CancellationToken.None);
        var entered = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var release = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        fixture.Authority.RegistrationEntered = entered;
        fixture.Authority.RegistrationRelease = release;
        const string winningCredential = "runtime-winning-secret";
        const string losingCredential = "runtime-losing-secret";
        var winningTask = fixture.Registrations.ExecuteAsync(
            originalRequest with
            {
                PairingToken = winningCredential,
                RegistrationPlanToken = recoveryPlan.PlanToken,
            },
            CancellationToken.None);
        await entered.Task.WaitAsync(TimeSpan.FromSeconds(5));

        var losingTask = fixture.Registrations.ExecuteAsync(
            originalRequest with
            {
                PairingToken = losingCredential,
                RegistrationPlanToken = recoveryPlan.PlanToken,
            },
            CancellationToken.None);
        RuntimeRegistrationExecutionException conflict;
        RuntimeRegistrationExecutionException competingConflict;
        try
        {
            var losingCompletion = await Task.WhenAny(
                losingTask,
                Task.Delay(TimeSpan.FromSeconds(5)));
            Assert.Same(losingTask, losingCompletion);
            conflict = await Assert.ThrowsAsync<
                RuntimeRegistrationExecutionException>(() => losingTask);

            var competingTask = fixture.Registrations.ExecuteAsync(
                originalRequest with
                {
                    PairingToken = losingCredential,
                    Tags = new RuntimeTags("prod", "competing", "capture"),
                    RegistrationPlanToken = recoveryPlan.PlanToken,
                },
                CancellationToken.None);
            var competingCompletion = await Task.WhenAny(
                competingTask,
                Task.Delay(TimeSpan.FromSeconds(5)));
            Assert.Same(competingTask, competingCompletion);
            competingConflict = await Assert.ThrowsAsync<
                RuntimeRegistrationExecutionException>(() => competingTask);
        }
        finally
        {
            release.TrySetResult(true);
        }

        Assert.Equal("runtime_registration_in_progress", conflict.Code);
        Assert.Equal(
            "runtime_registration_in_progress",
            competingConflict.Code);
        Assert.Equal(3, fixture.Authority.RegisterCalls);
        Assert.Equal(3, fixture.Discovery.Requests.Count);
        var registered = await winningTask;

        Assert.Equal("runtime-coordinator", registered.RuntimeId);
        Assert.Empty(fixture.Registry.ListPendingRuntimeRegistrations());
        Assert.Equal(
            winningCredential,
            fixture.Registry.GetRuntimeControlAccess(
                registered.RuntimeId)?.AdminToken);
        Assert.DoesNotContain(
            losingCredential,
            await File.ReadAllTextAsync(fixture.StatePath),
            StringComparison.Ordinal);

        var stale = await Assert.ThrowsAsync<
            RuntimeRegistrationExecutionException>(() =>
                fixture.Registrations.ExecuteAsync(
                    originalRequest with
                    {
                        PairingToken = losingCredential,
                        RegistrationPlanToken = recoveryPlan.PlanToken,
                    },
                    CancellationToken.None));
        Assert.Equal("runtime_registration_plan_changed", stale.Code);
        Assert.Equal(3, fixture.Authority.RegisterCalls);
        Assert.Equal(
            winningCredential,
            fixture.Registry.GetRuntimeControlAccess(
                registered.RuntimeId)?.AdminToken);
    }

    private static Fixture CreateFixture(
        bool enabled,
        DaemonRuntimeProjection? runtime,
        DiscoveryHandler discoveryHandler,
        string? statePath = null,
        FakeRegistrationAuthority? authority = null)
    {
        statePath ??= Path.Combine(
            Path.GetTempPath(),
            $"leserpent-registration-execution-{Guid.NewGuid():N}.json");
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_STATE_PATH"] = statePath,
                ["LESERPENT_ALLOW_PUBLIC_ENDPOINTS"] = "true",
            })
            .Build();
        var environment = new TestEnvironment(
            Path.GetDirectoryName(statePath)!);
        var store = new ControlPlaneStateStore(
            configuration,
            environment,
            NullLogger<ControlPlaneStateStore>.Instance);
        var registry = new RegistryService(
            store,
            new InMemoryOrchestraRunStore());
        var security = new ControlPlaneSecurityPolicy(configuration);
        var client = new HttpClient(discoveryHandler);
        var discovery = new CapabilityDiscoveryService(client, security);
        authority ??= new FakeRegistrationAuthority(enabled, runtime);
        var plans = new RuntimeRegistrationPlanProjectionService(
            registry,
            authority);
        var registrations = new RuntimeRegistrationExecutionService(
            registry,
            discovery,
            authority,
            plans,
            new RuntimeRegistrationCommitProjectionService(),
            security);
        return new Fixture(
            statePath,
            registry,
            authority,
            plans,
            registrations,
            discoveryHandler,
            client);
    }

    private static DaemonRuntimeProjection Projection(ulong revision)
    {
        var observedAt = DateTimeOffset.Parse("2026-08-25T08:00:00Z");
        return new DaemonRuntimeProjection(
            "runtime-coordinator",
            "Runtime A",
            RuntimeEndpoint,
            null,
            observedAt,
            observedAt,
            revision,
            new RuntimeTags(null, null, null),
            Status("daemon"),
            null,
            null);
    }

    private static RuntimeStatusSnapshot Status(string source) =>
        new(
            source,
            DateTimeOffset.Parse("2026-08-25T08:00:00Z"),
            null,
            true,
            "capture",
            1,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            false,
            false,
            false);

    private sealed class Fixture(
        string statePath,
        RegistryService registry,
        FakeRegistrationAuthority authority,
        RuntimeRegistrationPlanProjectionService plans,
        RuntimeRegistrationExecutionService registrations,
        DiscoveryHandler discovery,
        HttpClient client) : IDisposable
    {
        internal RegistryService Registry { get; } = registry;
        internal FakeRegistrationAuthority Authority { get; } = authority;
        internal RuntimeRegistrationPlanProjectionService Plans { get; } = plans;
        internal RuntimeRegistrationExecutionService Registrations { get; } = registrations;
        internal DiscoveryHandler Discovery { get; } = discovery;
        internal string StatePath { get; } = statePath;

        public void Dispose()
        {
            client.Dispose();
            File.Delete(StatePath);
            File.Delete($"{StatePath}.bak");
            File.Delete($"{StatePath}.tmp");
        }
    }

    private sealed class FakeRegistrationAuthority(
        bool enabled,
        DaemonRuntimeProjection? runtime) :
        IRuntimeRegistrationAuthority,
        IDaemonRuntimeProjectionReader
    {
        public bool Enabled => enabled;
        internal DaemonRuntimeProjection? Runtime { get; set; } = runtime;
        internal int SnapshotCalls { get; private set; }
        internal int RegisterCalls { get; private set; }
        internal int AmbiguousFailuresRemaining { get; set; }
        internal ulong? ExpectedRevision { get; private set; }
        internal List<ulong?> ExpectedRevisions { get; } = [];
        internal List<string> CommandIds { get; } = [];
        internal bool Update { get; private set; }
        internal CapabilityDiscoveryResult? CapabilityDiscovery { get; private set; }
        internal RuntimeStatusDiscoveryResult? StatusDiscovery { get; private set; }
        internal TaskCompletionSource<bool>? RegistrationEntered { get; set; }
        internal TaskCompletionSource<bool>? RegistrationRelease { get; set; }
        private readonly Dictionary<
            string,
            RuntimeRegistrationCommitReceipt> receipts =
                new(StringComparer.Ordinal);

        public Task<IReadOnlyList<DaemonRuntimeProjection>> ListAsync(
            RuntimeListFilter filter,
            CancellationToken cancellationToken) =>
            Task.FromResult<IReadOnlyList<DaemonRuntimeProjection>>(
                Runtime is null ? Array.Empty<DaemonRuntimeProjection>() : [Runtime]);

        public Task<DaemonRuntimeProjectionSnapshot> SnapshotAsync(
            CancellationToken cancellationToken)
        {
            SnapshotCalls += 1;
            return Task.FromResult(new DaemonRuntimeProjectionSnapshot(
                Runtime?.Revision ?? 1,
                Runtime is null
                    ? Array.Empty<DaemonRuntimeProjection>()
                    : [Runtime]));
        }

        public Task<DaemonRuntimeProjection?> InspectAsync(
            string runtimeId,
            CancellationToken cancellationToken) =>
            Task.FromResult(Runtime is not null
                && string.Equals(
                    Runtime.RuntimeId,
                    runtimeId,
                    StringComparison.Ordinal)
                    ? Runtime
                    : null);

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            throw new InvalidOperationException(
                "registration coordinator must use the typed receipt path");

        public async Task<RuntimeRegistrationCommitReceipt>
            RegisterWithReceiptAsync(
                RuntimeRegistrationRequest request,
                string runtimeId,
                CancellationToken cancellationToken,
                bool update = false,
                CapabilityDiscoveryResult? capabilityDiscovery = null,
                RuntimeStatusDiscoveryResult? statusDiscovery = null,
                RuntimeSidecarDiscoveryResult? sidecarDiscovery = null,
                ulong? expectedRevision = null)
        {
            RegisterCalls += 1;
            ExpectedRevision = expectedRevision;
            ExpectedRevisions.Add(expectedRevision);
            Update = update;
            CapabilityDiscovery = capabilityDiscovery;
            StatusDiscovery = statusDiscovery;
            var commandId = RuntimeRegistrationCommandIdentity.ForIntent(
                runtimeId,
                request.Name,
                request.Endpoint,
                request.SidecarEndpoint,
                request.Tags,
                expectedRevision);
            CommandIds.Add(commandId);
            RegistrationEntered?.TrySetResult(true);
            if (RegistrationRelease is not null)
            {
                await RegistrationRelease.Task.WaitAsync(cancellationToken);
            }
            if (!receipts.TryGetValue(commandId, out var receipt))
            {
                var registrationRevision = update
                    ? (expectedRevision ?? throw new InvalidOperationException(
                        "update receipt requires the reviewed revision")) + 1
                    : 1;
                var discoveryApplied =
                    capabilityDiscovery?.AuthoritySnapshot is not null
                    || statusDiscovery is not null
                    || sidecarDiscovery?.SidecarStatus is not null;
                var finalRevision = registrationRevision
                    + (discoveryApplied ? 1UL : 0UL);
                var observedAt = DateTimeOffset.Parse(
                    "2026-08-25T08:01:00Z");
                Runtime = new DaemonRuntimeProjection(
                    runtimeId,
                    request.Name.Trim(),
                    request.Endpoint.Trim(),
                    string.IsNullOrWhiteSpace(request.SidecarEndpoint)
                        ? null
                        : request.SidecarEndpoint.Trim(),
                    Runtime?.RegisteredAt ?? observedAt,
                    observedAt,
                    finalRevision,
                    request.Tags ?? new RuntimeTags(null, null, null),
                    statusDiscovery?.Status ?? Runtime?.Status ??
                        Status("manual"),
                    capabilityDiscovery?.AuthoritySnapshot ??
                        Runtime?.Capabilities,
                    sidecarDiscovery?.SidecarStatus ??
                        Runtime?.SidecarStatus);
                receipt = RuntimeRegistrationCommitReceipt
                    .FromAuthoritativeCommit(
                        registrationRevision,
                        Runtime,
                        discoveryApplied);
                receipts.Add(commandId, receipt);
            }
            if (AmbiguousFailuresRemaining > 0)
            {
                AmbiguousFailuresRemaining -= 1;
                throw new DaemonRuntimeRegistrationException(
                    "daemon_transport_failed",
                    "daemon applied registration before transport loss");
            }
            return receipt;
        }

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            Task.CompletedTask;

        public Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken) =>
            Task.CompletedTask;
    }

    private sealed class DiscoveryHandler : HttpMessageHandler
    {
        internal List<string> Requests { get; } = [];
        internal List<string?> AdminTokens { get; } = [];
        internal TaskCompletionSource<bool>? RequestEntered { get; init; }
        internal TaskCompletionSource<bool>? RequestRelease { get; init; }

        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            Requests.Add(request.RequestUri?.AbsolutePath ?? string.Empty);
            AdminTokens.Add(
                request.Headers.TryGetValues(
                    CapabilityDiscoveryService.GewyvernAdminTokenHeader,
                    out var values)
                        ? values.SingleOrDefault()
                        : null);
            RequestEntered?.TrySetResult(true);
            if (RequestRelease is not null)
            {
                await RequestRelease.Task.WaitAsync(cancellationToken);
            }
            var payload = request.RequestUri?.AbsolutePath switch
            {
                "/v1/capabilities" =>
                    """{"service":"gewyvern-api","version":"1.17.4","latest_snapshot":true,"authenticated_deployment":true,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/capabilities"]}""",
                "/v1/latest/meta" =>
                    """{"updated_unix_ms":1,"kind":"capture","target_count":2,"has_summary_json":true,"has_analysis_json":true,"has_training_example_json":true,"has_export_json":true,"has_report_json":true,"has_report_html":true,"has_external_sidecar_context":false,"has_external_evidence_chain_enrichment":false,"has_external_diagnostic_opinion":false}""",
                "/v1/runtime/resilience.json" =>
                    """{"degraded":false,"status":"ready","summary":"healthy","socket_service":{"status":"ready","consecutive_idle_timeouts":0,"total_idle_timeouts":0}}""",
                _ => "{}",
            };
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(
                    payload,
                    Encoding.UTF8,
                    "application/json"),
            };
        }
    }

    private sealed class TestEnvironment(string contentRootPath) :
        IHostEnvironment
    {
        public string EnvironmentName { get; set; } =
            Environments.Development;
        public string ApplicationName { get; set; } =
            "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = contentRootPath;
        public IFileProvider ContentRootFileProvider { get; set; } =
            new NullFileProvider();
    }
}
