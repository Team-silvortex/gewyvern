using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeRegistrationPolicyTests
{
    [Fact]
    public void SameNameProducesIdempotentUpdatePlanAndPreservesRuntimeIdentity()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var original = Register(registry, "runtime", "http://runtime-a.test");
            var plan = registry.GetRuntimeRegistrationPlan(new RuntimeRegistrationPlanRequest(
                "RUNTIME",
                "http://runtime-b.test"));

            Assert.True(plan.Allowed);
            Assert.Equal(RuntimeRegistrationPolicy.UpdateAction, plan.Action);
            Assert.Equal(original.RuntimeId, plan.ExistingRuntimeId);

            var updated = Register(
                registry,
                "RUNTIME",
                "http://runtime-b.test",
                plan.PlanToken);
            Assert.Equal(original.RuntimeId, updated.RuntimeId);
            Assert.Equal("http://runtime-b.test", updated.Endpoint);
            Assert.Single(registry.ListRuntimes());
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void EndpointOwnedByAnotherNameIsRejectedAtTheRegistryBoundary()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var original = Register(registry, "first", "http://runtime.test");
            var plan = registry.GetRuntimeRegistrationPlan(new RuntimeRegistrationPlanRequest(
                "second",
                "http://runtime.test"));

            Assert.False(plan.Allowed);
            Assert.Equal(RuntimeRegistrationPolicy.EndpointConflictReason, plan.Reason);
            Assert.Equal(original.RuntimeId, plan.ExistingRuntimeId);
            Assert.Throws<RuntimeRegistrationPlanException>(() =>
                Register(registry, "second", "http://runtime.test"));
            Assert.Single(registry.ListRuntimes());
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void CanonicallyEquivalentEndpointCannotBypassUniqueness()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            Register(registry, "first", "http://runtime.test");
            var plan = registry.GetRuntimeRegistrationPlan(new RuntimeRegistrationPlanRequest(
                "second",
                "HTTP://RUNTIME.TEST:80/"));

            Assert.False(plan.Allowed);
            Assert.Equal(RuntimeRegistrationPolicy.EndpointConflictReason, plan.Reason);
            Assert.Throws<RuntimeRegistrationPlanException>(() =>
                Register(registry, "second", "HTTP://RUNTIME.TEST:80/"));
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void StaleCreatePlanCannotBecomeAnUnreviewedUpdate()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var stalePlan = registry.GetRuntimeRegistrationPlan(new RuntimeRegistrationPlanRequest(
                "runtime",
                "http://runtime-b.test"));
            Register(registry, "runtime", "http://runtime-a.test");

            Assert.Throws<RuntimeRegistrationPlanException>(() =>
                Register(registry, "runtime", "http://runtime-b.test", stalePlan.PlanToken));
            Assert.Equal("http://runtime-a.test", registry.ListRuntimes().Single().Endpoint);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    private static RuntimeRegistrationResponse Register(
        RegistryService registry,
        string name,
        string endpoint,
        string? planToken = null) =>
        registry.RegisterRuntime(new RuntimeRegistrationRequest(
            name,
            endpoint,
            "pairing-token",
            RegistrationPlanToken: planToken));

    private static (RegistryService Registry, string StatePath) CreateRegistry()
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-registration-test-{Guid.NewGuid():N}.json");
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
