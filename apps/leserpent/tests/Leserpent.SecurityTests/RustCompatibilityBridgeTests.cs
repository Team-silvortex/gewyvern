using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RustCompatibilityBridgeTests
{
    [Fact]
    public async Task UnconfiguredBridgeIsAnExplicitNoOp()
    {
        using var bridge = CreateBridge();
        Assert.False(bridge.Enabled);

        await bridge.ValidateRuntimeListAsync(
            new RuntimeCollectionResponse(
                new RuntimeListFilter(null, null, null),
                Array.Empty<RuntimeSummary>()),
            CancellationToken.None);
    }

    [Fact]
    public void ConfiguredBridgeRequiresAnExistingAbsoluteExecutable()
    {
        Assert.Throws<InvalidOperationException>(() => CreateBridge(("LESERPENT_RUST_BRIDGE_BIN", "relative/path")));
        Assert.Throws<FileNotFoundException>(() => CreateBridge((
            "LESERPENT_RUST_BRIDGE_BIN",
            Path.Combine(Path.GetTempPath(), $"missing-leserpent-bridge-{Guid.NewGuid():N}"))));
    }

    [Fact]
    public async Task OversizedPayloadIsRejectedBeforeStartingTheConfiguredProcess()
    {
        using var bridge = CreateBridge(("LESERPENT_RUST_BRIDGE_BIN", Environment.ProcessPath!));
        var status = new RuntimeStatusSnapshot(
            new string('x', 1024 * 1024),
            null,
            null,
            false,
            null,
            null,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false);

        var error = await Assert.ThrowsAsync<CompatibilityBridgeException>(() =>
            bridge.ValidateStatusRefreshAsync(
                new RuntimeStatusRefreshResponse("runtime-a", "A", "http://a", status),
                CancellationToken.None));
        Assert.Contains("exceeds 1 MiB", error.Message, StringComparison.Ordinal);
    }

    private static RustCompatibilityBridge CreateBridge(
        params (string Key, string Value)[] values)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(values.ToDictionary(
                item => item.Key,
                item => (string?)item.Value))
            .Build();
        return new RustCompatibilityBridge(
            configuration,
            NullLogger<RustCompatibilityBridge>.Instance);
    }
}
