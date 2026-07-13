using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class CapabilityDiscoveryCancellationTests
{
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
}
