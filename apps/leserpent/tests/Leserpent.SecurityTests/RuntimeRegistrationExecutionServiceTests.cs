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
    private const string PairingToken = "runtime-pairing-secret";

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

    private static Fixture CreateFixture(
        bool enabled,
        DaemonRuntimeProjection? runtime,
        DiscoveryHandler discoveryHandler)
    {
        var statePath = Path.Combine(
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
        var authority = new FakeRegistrationAuthority(enabled, runtime);
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

        public void Dispose()
        {
            client.Dispose();
            File.Delete(statePath);
            File.Delete($"{statePath}.bak");
            File.Delete($"{statePath}.tmp");
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
        internal ulong? ExpectedRevision { get; private set; }
        internal bool Update { get; private set; }
        internal CapabilityDiscoveryResult? CapabilityDiscovery { get; private set; }
        internal RuntimeStatusDiscoveryResult? StatusDiscovery { get; private set; }

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

        public Task<RuntimeRegistrationCommitReceipt>
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
            Update = update;
            CapabilityDiscovery = capabilityDiscovery;
            StatusDiscovery = statusDiscovery;
            var registrationRevision = update
                ? (expectedRevision ?? throw new InvalidOperationException(
                    "update receipt requires the reviewed revision")) + 1
                : 1;
            var discoveryApplied = capabilityDiscovery?.AuthoritySnapshot is not null
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
                statusDiscovery?.Status ?? Runtime?.Status ?? Status("manual"),
                capabilityDiscovery?.AuthoritySnapshot ?? Runtime?.Capabilities,
                sidecarDiscovery?.SidecarStatus ?? Runtime?.SidecarStatus);
            return Task.FromResult(
                RuntimeRegistrationCommitReceipt.FromAuthoritativeCommit(
                    registrationRevision,
                    Runtime,
                    discoveryApplied));
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

        protected override Task<HttpResponseMessage> SendAsync(
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
            return Task.FromResult(new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(
                    payload,
                    Encoding.UTF8,
                    "application/json"),
            });
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
