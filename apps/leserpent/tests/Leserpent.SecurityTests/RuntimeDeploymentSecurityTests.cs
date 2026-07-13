using System.Net;
using System.Text;
using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeDeploymentSecurityTests
{
    [Fact]
    public async Task DeploymentUsesGewyvernTokenHeaderWithoutPuttingSecretInBody()
    {
        string? observedToken = null;
        string? observedBody = null;
        var handler = new RecordingHandler(async request =>
        {
            observedToken = request.Headers.GetValues(CapabilityDiscoveryService.GewyvernAdminTokenHeader).Single();
            observedBody = await request.Content!.ReadAsStringAsync();
            return new HttpResponseMessage(HttpStatusCode.Accepted)
            {
                Content = new StringContent(
                    "{\"deployment_id\":\"gdep_1\",\"request_id\":\"req-1\",\"pipeline_kind\":\"http/request\",\"requested_by\":\"operator\",\"status\":\"accepted\",\"accepted_unix_ms\":1700000000000,\"target\":\"pid:42\",\"replayed\":false}",
                    Encoding.UTF8,
                    "application/json"),
            };
        });
        using var client = new HttpClient(handler);
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_ALLOW_PUBLIC_ENDPOINTS"] = "true",
            })
            .Build();
        var discovery = new CapabilityDiscoveryService(client, new ControlPlaneSecurityPolicy(configuration));

        var result = await discovery.DeployAsync(
            new RuntimeControlAccess("runtime-1", "runtime", "http://runtime.test", "runtime-secret", new RuntimeTags(null, null, null)),
            new RuntimeDeploymentRequest("http/request", "operator", true, "req-1", "pid:42"),
            CancellationToken.None);

        Assert.Equal("runtime-secret", observedToken);
        Assert.DoesNotContain("runtime-secret", observedBody);
        Assert.Equal("accepted", result.Status);
        Assert.Equal("runtime-1", result.RuntimeId);
    }

    [Fact]
    public void RuntimeAdminTokenIsMemoryOnlyAndIsNotRestoredFromState()
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-deployment-test-{Guid.NewGuid():N}.json");
        try
        {
            var registry = CreateRegistry(statePath);
            var registered = registry.RegisterRuntime(new RuntimeRegistrationRequest(
                "runtime",
                "http://127.0.0.1:49152",
                "runtime-secret"));

            Assert.True(registered.HasRuntimeAdminToken);
            Assert.Equal("runtime-secret", registry.GetRuntimeControlAccess(registered.RuntimeId)!.AdminToken);
            Assert.DoesNotContain("runtime-secret", File.ReadAllText(statePath));

            var restored = CreateRegistry(statePath);
            Assert.False(restored.GetRuntime(registered.RuntimeId)!.HasRuntimeAdminToken);
            Assert.Null(restored.GetRuntimeControlAccess(registered.RuntimeId)!.AdminToken);
        }
        finally
        {
            File.Delete(statePath);
            File.Delete($"{statePath}.bak");
            File.Delete($"{statePath}.tmp");
        }
    }

    private static RegistryService CreateRegistry(string statePath)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_STATE_PATH"] = statePath,
            })
            .Build();
        var environment = new TestEnvironment(Path.GetDirectoryName(statePath)!);
        var store = new ControlPlaneStateStore(
            configuration,
            environment,
            NullLogger<ControlPlaneStateStore>.Instance);
        return new RegistryService(store, new InMemoryOrchestraRunStore());
    }

    private sealed class RecordingHandler(Func<HttpRequestMessage, Task<HttpResponseMessage>> handle)
        : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) => handle(request);
    }

    private sealed class TestEnvironment(string contentRootPath) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = contentRootPath;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
