using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeCleanupPolicyTests
{
    [Fact]
    public void CleanupPlanPreservesIdleReadyRuntimeAndClassifiesProtectedSlice()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var idle = Register(registry, "idle", "production");
            var unseen = Register(registry, "unseen", "production");
            var idleStatus = registry.GetRuntime(idle.RuntimeId)!.Status with
            {
                ResilienceStatus = "idle_ready",
                ResilienceDegraded = false,
            };
            registry.RefreshRuntimeStatus(
                idle.RuntimeId,
                RuntimeStatusDiscoveryResult.Succeeded("http://runtime.test/v1/latest-meta", idleStatus));

            var plan = registry.GetRuntimeCleanupPlan(new RuntimeListFilter("production", null, null));

            Assert.Equal("protected", plan.RiskLevel);
            Assert.Single(plan.Unobserved.Targets);
            Assert.Equal(unseen.RuntimeId, plan.Unobserved.Targets[0].RuntimeId);
            Assert.Equal(2, plan.Slice.RuntimeCount);
            Assert.Equal("CLEAR 2", plan.Slice.Challenge);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void PlannedDeleteRejectsStaleTokenInsteadOfDeletingNewTargets()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            Register(registry, "first", "dev");
            var filter = new RuntimeListFilter("dev", null, null);
            var stalePlan = registry.GetRuntimeCleanupPlan(filter);
            Register(registry, "second", "dev");

            Assert.Throws<RuntimeCleanupPlanMismatchException>(() =>
                registry.DeletePlannedRuntimes(
                    RuntimeCleanupPolicy.UnobservedKind,
                    filter,
                    new RuntimeCleanupRequest(stalePlan.Unobserved.PlanToken)));
            Assert.Equal(2, registry.ListRuntimes(filter).Count);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void SliceDeleteRequiresServerIssuedChallenge()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            Register(registry, "runtime", "dev");
            var filter = new RuntimeListFilter("dev", null, null);
            var plan = registry.GetRuntimeCleanupPlan(filter);

            Assert.Throws<RuntimeCleanupPlanMismatchException>(() =>
                registry.DeletePlannedRuntimes(
                    RuntimeCleanupPolicy.SliceKind,
                    filter,
                    new RuntimeCleanupRequest(plan.Slice.PlanToken, "CLEAR 0")));
            Assert.Single(registry.ListRuntimes(filter));
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    private static RuntimeRegistrationResponse Register(RegistryService registry, string name, string environment) =>
        registry.RegisterRuntime(new RuntimeRegistrationRequest(
            name,
            $"http://{name}.test",
            "pairing-token",
            Tags: new RuntimeTags(environment, null, null)));

    private static (RegistryService Registry, string StatePath) CreateRegistry()
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-cleanup-test-{Guid.NewGuid():N}.json");
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?> { ["LESERPENT_STATE_PATH"] = statePath })
            .Build();
        var environment = new TestEnvironment(Path.GetDirectoryName(statePath)!);
        var store = new ControlPlaneStateStore(configuration, environment, NullLogger<ControlPlaneStateStore>.Instance);
        return (new RegistryService(store, new InMemoryOrchestraRunStore()), statePath);
    }

    private static void DeleteStateFiles(string statePath)
    {
        File.Delete(statePath);
        File.Delete($"{statePath}.bak");
        File.Delete($"{statePath}.tmp");
    }

    private sealed class TestEnvironment(string contentRootPath) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = contentRootPath;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
