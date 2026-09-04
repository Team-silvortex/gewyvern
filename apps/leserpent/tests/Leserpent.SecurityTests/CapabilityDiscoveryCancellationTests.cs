using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class CapabilityDiscoveryCancellationTests
{
    [Fact]
    public async Task DiscoverAsyncPreservesTypedBooleanAuthorityExtensions()
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_ALLOW_PUBLIC_ENDPOINTS"] = "true",
            })
            .Build();
        var security = new ControlPlaneSecurityPolicy(configuration);
        const string body = """{"service":"gewyvern-api","version":"1.2.0","latest_snapshot":true,"authenticated_deployment":true,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/deployments","/v1/capabilities","/v1/capabilities"],"protocol_catalog":true}""";
        using var client = new HttpClient(new StaticHandler(body));
        var discovery = new CapabilityDiscoveryService(client, security);

        var result = await discovery.DiscoverAsync(
            "http://127.0.0.1:49152",
            null,
            CancellationToken.None);

        var snapshot = Assert.IsType<RuntimeCapabilityAuthoritySnapshot>(result.AuthoritySnapshot);
        Assert.Equal(new[] { "/v1/capabilities", "/v1/deployments" }, snapshot.Endpoints);
        Assert.True(snapshot.Extensions["protocol_catalog"]);
    }

    [Fact]
    public async Task DiscoverAsyncPropagatesOperatorCancellation()
    {
        // Disable address pinning so this test exercises the injected blocking handler.
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_ALLOW_PUBLIC_ENDPOINTS"] = "true",
            })
            .Build();
        var security = new ControlPlaneSecurityPolicy(configuration);
        using var client = new HttpClient(new BlockingHandler());
        var discovery = new CapabilityDiscoveryService(client, security);
        using var cancellation = new CancellationTokenSource(TimeSpan.FromMilliseconds(50));

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
            discovery.DiscoverAsync("http://127.0.0.1:49152", null, cancellation.Token));
    }

    [Fact]
    public async Task DiscoverAsyncRejectsOversizedCapabilityResponse()
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_ALLOW_PUBLIC_ENDPOINTS"] = "true",
            })
            .Build();
        var security = new ControlPlaneSecurityPolicy(configuration);
        using var client = new HttpClient(new StaticHandler(new string('x', 1_048_577)));
        var discovery = new CapabilityDiscoveryService(client, security);

        var result = await discovery.DiscoverAsync(
            "http://127.0.0.1:49152",
            null,
            CancellationToken.None);

        Assert.Equal("capability_fetch_failed", result.CapabilityFetchError);
        Assert.Null(result.AuthoritySnapshot);
    }

    private sealed class BlockingHandler : HttpMessageHandler
    {
        protected override async Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken)
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken);
            throw new InvalidOperationException("unreachable");
        }
    }

    private sealed class StaticHandler(string body) : HttpMessageHandler
    {
        protected override Task<HttpResponseMessage> SendAsync(
            HttpRequestMessage request,
            CancellationToken cancellationToken) =>
            Task.FromResult(new HttpResponseMessage(System.Net.HttpStatusCode.OK)
            {
                Content = new StringContent(body),
            });
    }
}
