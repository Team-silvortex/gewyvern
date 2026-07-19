using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeRecoveryContractTests
{
    [Fact]
    public void SuggestedRecoveryActionsCarryServerExecutableCommandKinds()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var runtime = registry.RegisterRuntime(new RuntimeRegistrationRequest(
                "runtime",
                "http://127.0.0.1:49152",
                "pairing-token"));

            var attention = registry.GetRuntimeAttention(runtime.RuntimeId)!;

            Assert.NotEmpty(attention.SuggestedActions);
            Assert.All(attention.SuggestedActions, action => Assert.False(string.IsNullOrWhiteSpace(action.CommandKind)));
            Assert.Contains(attention.SuggestedActions, action => action.Action == "refresh_status" && action.CommandKind == "status");
            Assert.Contains(attention.SuggestedActions, action => action.Action == "refresh_all" && action.CommandKind == "all");
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void CooldownAdaptationPreservesExecutableCommandKind()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var runtime = registry.RegisterRuntime(new RuntimeRegistrationRequest(
                "runtime",
                "http://127.0.0.1:49152",
                "pairing-token"));
            registry.RecordRecoveryActivity(runtime.RuntimeId, "refresh_status", "auth_failed", "test failure");

            var action = registry.GetRuntimeAttention(runtime.RuntimeId)!.SuggestedActions
                .Single(item => item.Action == "refresh_status");

            Assert.True(action.CoolingDown);
            Assert.True(action.CooldownSecondsRemaining > 0);
            Assert.Equal("status", action.CommandKind);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    private static (RegistryService Registry, string StatePath) CreateRegistry()
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-recovery-test-{Guid.NewGuid():N}.json");
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
