using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeReadProjectionTests
{
    [Fact]
    public async Task ConfiguredDaemonProjectionOverridesAuthorityFieldsAndRetainsCompatibilityMetadata()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret",
                    Capabilities: new[] { new RuntimeCapability("manual", "fully_supported", "manual") },
                    Tags: new RuntimeTags("managed", null, null),
                    SidecarEndpoint: "https://sidecar.invalid",
                    SidecarAdminToken: "sidecar-secret"),
                "runtime-a");
            var authoritative = Projection("runtime-a");
            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[] { authoritative }));

            var inspected = Assert.IsType<RuntimeSummary>(
                await reads.InspectAsync("runtime-a", CancellationToken.None));
            Assert.Equal("Daemon Name", inspected.Name);
            Assert.Equal("https://daemon.invalid", inspected.Endpoint);
            Assert.Equal("daemon", inspected.Tags.Environment);
            Assert.Equal("gewyvern-api", inspected.Status.StatusSource);
            Assert.Equal("gewyvern-api", inspected.CapabilitySource);
            Assert.Equal("etragon-api", inspected.SidecarStatus?.StatusSource);
            Assert.Equal("ready", inspected.SidecarStatus?.DaemonStatus);
            Assert.Contains(inspected.Capabilities, item => item.Key == "api.latest_snapshot" && item.Support == "fully_supported");
            Assert.Equal("https://daemon-sidecar.invalid", inspected.SidecarEndpoint);
            Assert.Equal(
                DateTimeOffset.Parse("2026-07-21T08:00:00Z"),
                inspected.RegisteredAt);
            Assert.Equal(
                DateTimeOffset.Parse("2026-07-21T09:30:00Z"),
                inspected.UpdatedAt);
            Assert.True(inspected.HasSidecarAdminToken);
            Assert.True(inspected.HasRuntimeAdminToken);

            var listed = await reads.ListAsync(
                new RuntimeListFilter("daemon", null, null),
                CancellationToken.None);
            var listedRuntime = Assert.Single(listed);
            Assert.Equal(inspected.RuntimeId, listedRuntime.RuntimeId);
            Assert.Equal(inspected.Name, listedRuntime.Name);
            Assert.Equal(inspected.Endpoint, listedRuntime.Endpoint);
            Assert.Equal(inspected.Status, listedRuntime.Status);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task AttentionUsesDaemonProjectionForIdentityAndStatusWhileRetainingManagedRecoveryHistory()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret",
                    Tags: new RuntimeTags("managed", "west", "edge"),
                    SidecarEndpoint: "https://sidecar.invalid",
                    SidecarAdminToken: "sidecar-secret"),
                "runtime-attention");

            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[]
                {
                    Projection("runtime-attention") with
                    {
                        Name = "Daemon Name",
                        Endpoint = "https://daemon.invalid",
                        Tags = new RuntimeTags("daemon", "east", "edge"),
                        Status = Projection("runtime-attention").Status with
                        {
                            StatusSource = "fetch_failed"
                        }
                    }
                }));

            var runtime = Assert.IsType<RuntimeSummary>(
                await reads.InspectAsync("runtime-attention", CancellationToken.None));

            var attention = registry.GetRuntimeAttention("runtime-attention", runtime);
            Assert.NotNull(attention);
            Assert.Equal("Daemon Name", attention!.Name);
            Assert.Equal("https://daemon.invalid", attention.Endpoint);
            Assert.Equal("daemon", attention.Tags.Environment);
            Assert.Equal("fetch_failed", attention.Status.StatusSource);
            Assert.True(attention.NeedsAttention);
            Assert.Equal("critical", attention.Severity);
            Assert.Contains("status_fetch_failed", attention.Reasons);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task UnconfiguredOrManagedOnlyRuntimeRetainsManagedFallback()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var registered = registry.RegisterRuntime(
                new RuntimeRegistrationRequest("Managed", "https://managed.invalid", "secret"),
                "runtime-managed");
            var disabled = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(false, Array.Empty<DaemonRuntimeProjection>()));
            Assert.Equal(
                registered.RuntimeId,
                (await disabled.InspectAsync("runtime-managed", CancellationToken.None))?.RuntimeId);

            var transitional = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, Array.Empty<DaemonRuntimeProjection>()));
            Assert.Equal(
                registered.RuntimeId,
                (await transitional.InspectAsync("runtime-managed", CancellationToken.None))?.RuntimeId);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task LegacyDaemonProjectionRetainsManagedAuthorityTimestamps()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var registered = registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed",
                    "https://managed.invalid",
                    "secret"),
                "runtime-legacy-time");
            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[]
                {
                    Projection("runtime-legacy-time") with
                    {
                        RegisteredAt = null,
                        UpdatedAt = null
                    }
                }));

            var projected = Assert.IsType<RuntimeSummary>(
                await reads.InspectAsync("runtime-legacy-time", CancellationToken.None));
            Assert.Equal(registered.RegisteredAt, projected.RegisteredAt);
            Assert.Equal(
                registry.GetRuntime("runtime-legacy-time")?.UpdatedAt,
                projected.UpdatedAt);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task DaemonOnlyRuntimeFailsClosedWithoutCompatibilityMetadata()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[] { Projection("runtime-orphan") }));
            var error = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                reads.InspectAsync("runtime-orphan", CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", error.Code);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    private static DaemonRuntimeProjection Projection(string runtimeId) =>
        new(
            runtimeId,
            "Daemon Name",
            "https://daemon.invalid",
            "https://daemon-sidecar.invalid",
            DateTimeOffset.Parse("2026-07-21T08:00:00Z"),
            DateTimeOffset.Parse("2026-07-21T09:30:00Z"),
            7,
            new RuntimeTags("daemon", "east", "edge"),
            new RuntimeStatusSnapshot(
                "gewyvern-api",
                DateTimeOffset.Parse("2026-07-20T12:00:00Z"),
                null,
                true,
                "capture",
                3,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                false),
            new RuntimeCapabilityAuthoritySnapshot(
                "gewyvern-api",
                "gewyvern-api",
                "1.2.0",
                true,
                true,
                true,
                true,
                "percent-encoding",
                "A-Z a-z 0-9 . _ ~ :",
                new[] { "/v1/capabilities", "/v1/deployments" },
                new Dictionary<string, bool>()),
            new RuntimeSidecarStatusSnapshot(
                "etragon-api",
                DateTimeOffset.Parse("2026-07-21T09:25:00Z"),
                null,
                true,
                "ready",
                2,
                false,
                4,
                true,
                false));

    private static (RegistryService Registry, string StatePath) CreateRegistry()
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-read-projection-{Guid.NewGuid():N}.json");
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?> { ["LESERPENT_STATE_PATH"] = statePath })
            .Build();
        var environment = new TestEnvironment(Path.GetDirectoryName(statePath)!);
        var store = new ControlPlaneStateStore(
            configuration,
            environment,
            NullLogger<ControlPlaneStateStore>.Instance);
        return (new RegistryService(store, new InMemoryOrchestraRunStore()), statePath);
    }

    private static void DeleteStateFiles(string statePath)
    {
        File.Delete(statePath);
        File.Delete($"{statePath}.bak");
        File.Delete($"{statePath}.tmp");
    }

    private sealed class FakeDaemonReader(
        bool enabled,
        IReadOnlyList<DaemonRuntimeProjection> runtimes) : IDaemonRuntimeProjectionReader
    {
        public bool Enabled => enabled;

        public Task<IReadOnlyList<DaemonRuntimeProjection>> ListAsync(
            RuntimeListFilter filter,
            CancellationToken cancellationToken) =>
            Task.FromResult<IReadOnlyList<DaemonRuntimeProjection>>(runtimes
                .Where(runtime => string.IsNullOrWhiteSpace(filter.Environment)
                    || string.Equals(runtime.Tags.Environment, filter.Environment, StringComparison.OrdinalIgnoreCase))
                .ToArray());

        public Task<DaemonRuntimeProjection?> InspectAsync(
            string runtimeId,
            CancellationToken cancellationToken) =>
            Task.FromResult(runtimes.FirstOrDefault(runtime => runtime.RuntimeId == runtimeId));
    }

    private sealed class TestEnvironment(string contentRootPath) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = contentRootPath;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
