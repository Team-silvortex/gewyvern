using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class ControlPlaneWriterFenceTests
{
    [Theory]
    [InlineData("GET", "/v1/runtimes", false)]
    [InlineData("HEAD", "/v1/runtimes", false)]
    [InlineData("OPTIONS", "/v1/runtimes", false)]
    [InlineData("POST", "/v1/runtimes/registration-plan", false)]
    [InlineData("POST", "/v1/persistence/save", true)]
    [InlineData("POST", "/v1/runtimes/register", true)]
    [InlineData("POST", "/v1/runtimes/abc/deployments", true)]
    [InlineData("POST", "/v1/orchestra/plans/abc/plan/execute", true)]
    [InlineData("POST", "/v1/sessions", true)]
    [InlineData("PUT", "/v1/future-resource", true)]
    [InlineData("PATCH", "/v1/future-resource", true)]
    [InlineData("DELETE", "/v1/future-resource", true)]
    [InlineData("POST", "/health", false)]
    public void MutationPolicyIsFailClosedForControlPlaneWrites(
        string method,
        string path,
        bool expected)
    {
        var context = new DefaultHttpContext();
        context.Request.Method = method;
        context.Request.Path = path;

        Assert.Equal(
            expected,
            ControlPlaneMutationPolicy.IsMutation(context.Request));
    }

    [Fact]
    public async Task OneWriterStandbyRefusesMutationAndFreshProcessTakesOver()
    {
        var directory = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-writer-fence-{Guid.NewGuid():N}");
        Directory.CreateDirectory(directory);
        var statePath = Path.Combine(directory, "state.json");

        var firstStore = CreateStateStore(statePath);
        var secondStore = CreateStateStore(statePath);
        using var firstLease = new ControlPlaneWriterLease(firstStore);
        using var secondLease = new ControlPlaneWriterLease(secondStore);
        var firstFence = CreateFence(firstLease);
        var secondFence = CreateFence(secondLease);
        using var firstCheckpointLease =
            new OrchestraDeleteCheckpointWorkerLease(firstStore);
        using var secondCheckpointLease =
            new OrchestraDeleteCheckpointWorkerLease(secondStore);

        try
        {
            await firstFence.StartAsync(CancellationToken.None);
            await secondFence.StartAsync(CancellationToken.None);

            Assert.True(firstFence.IsWriter);
            Assert.False(secondFence.IsWriter);
            Assert.Equal("owner", firstFence.Snapshot().State);
            Assert.Equal("standby", secondFence.Snapshot().State);

            var firstRegistry = new RegistryService(
                firstStore,
                new InMemoryOrchestraRunStore(),
                firstCheckpointLease,
                firstFence);
            var secondRegistry = new RegistryService(
                secondStore,
                new InMemoryOrchestraRunStore(),
                secondCheckpointLease,
                secondFence);

            firstRegistry.RegisterRuntime(new RuntimeRegistrationRequest(
                "writer-owned-runtime",
                "http://127.0.0.1:8080",
                "pairing-token"));
            var persistedBeforeStandbyAttempt =
                await File.ReadAllBytesAsync(statePath);

            var error = Assert.Throws<
                ControlPlaneWriterUnavailableException>(
                () => secondRegistry.SaveNow());
            Assert.Equal(
                "control-plane mutation requires active writer ownership",
                error.Message);
            Assert.Empty(secondRegistry.ListRuntimes());
            Assert.Equal(
                persistedBeforeStandbyAttempt,
                await File.ReadAllBytesAsync(statePath));

            firstLease.Dispose();
            Assert.False(firstFence.IsWriter);
            Assert.Equal("lease_lost", firstFence.Snapshot().State);
            Assert.False(secondFence.IsWriter);
            Assert.Throws<ControlPlaneWriterUnavailableException>(
                () => secondRegistry.SaveNow());

            var takeoverStore = CreateStateStore(statePath);
            using var takeoverLease =
                new ControlPlaneWriterLease(takeoverStore);
            var takeoverFence = CreateFence(takeoverLease);
            await takeoverFence.StartAsync(CancellationToken.None);
            Assert.True(takeoverFence.IsWriter);

            using var takeoverCheckpointLease =
                new OrchestraDeleteCheckpointWorkerLease(
                    takeoverStore);
            var takeoverRegistry = new RegistryService(
                takeoverStore,
                new InMemoryOrchestraRunStore(),
                takeoverCheckpointLease,
                takeoverFence);
            Assert.Single(takeoverRegistry.ListRuntimes());
            _ = takeoverRegistry.SaveNow();
        }
        finally
        {
            try
            {
                Directory.Delete(directory, recursive: true);
            }
            catch (IOException)
            {
            }
        }
    }

    private static ControlPlaneWriterFence CreateFence(
        ControlPlaneWriterLease lease) =>
        new(
            lease,
            NullLogger<ControlPlaneWriterFence>.Instance);

    private static ControlPlaneStateStore CreateStateStore(
        string statePath) =>
        new(
            new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                    })
                .Build(),
            new TestHostEnvironment
            {
                ContentRootPath =
                    Path.GetDirectoryName(statePath)!,
            },
            NullLogger<ControlPlaneStateStore>.Instance);

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } =
            Environments.Development;
        public string ApplicationName { get; set; } =
            "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = string.Empty;
        public IFileProvider ContentRootFileProvider { get; set; } =
            new NullFileProvider();
    }
}
