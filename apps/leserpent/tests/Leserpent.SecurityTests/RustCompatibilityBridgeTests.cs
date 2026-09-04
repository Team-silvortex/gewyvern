using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging.Abstractions;
using System.Text;
using System.Text.Json;
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
        var normalized = await bridge.NormalizeRuntimeDeploymentRequestAsync(
            new RuntimeDeploymentCompatibilityEnvelope(
                "runtime-a",
                new RuntimeDeploymentRequest(
                    "capture/http",
                    "operator-a",
                    true,
                    "deploy-001",
                    "service-a")),
            CancellationToken.None);
        Assert.Equal("deploy-001", normalized.Request.RequestId);
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
    public void DeploymentEnvelopeMatchesTheStrictRustFixtureShape()
    {
        var payload = new RuntimeDeploymentCompatibilityEnvelope(
            "runtime-alpha",
            new RuntimeDeploymentRequest(
                "capture/http",
                "operator-a",
                true,
                "deploy-001",
                "service-a"));
        var json = JsonSerializer.SerializeToElement(
            payload,
            global::Leserpent.LeserpentJsonContext.Default.RuntimeDeploymentCompatibilityEnvelope);

        Assert.Equal(
            new[] { "request", "runtimeId" }.OrderBy(name => name, StringComparer.Ordinal),
            json.EnumerateObject().Select(property => property.Name).OrderBy(name => name, StringComparer.Ordinal));
        var request = json.GetProperty("request");
        Assert.Equal(
            new[] { "confirmed", "pipelineKind", "requestId", "requestedBy", "target" }
                .OrderBy(name => name, StringComparer.Ordinal),
            request.EnumerateObject().Select(property => property.Name)
                .OrderBy(name => name, StringComparer.Ordinal));
        Assert.Equal("runtime-alpha", json.GetProperty("runtimeId").GetString());
        Assert.Equal("deploy-001", request.GetProperty("requestId").GetString());
        Assert.True(request.GetProperty("confirmed").GetBoolean());
    }

    [Fact]
    public async Task ConfiguredRustProcessReturnsTheCanonicalDeploymentAuthority()
    {
        var executable = Environment.GetEnvironmentVariable("LESERPENT_TEST_RUST_BRIDGE_BIN");
        if (string.IsNullOrWhiteSpace(executable))
        {
            return;
        }
        using var bridge = CreateBridge(("LESERPENT_RUST_BRIDGE_BIN", executable));
        var normalized = await bridge.NormalizeRuntimeDeploymentRequestAsync(
            new RuntimeDeploymentCompatibilityEnvelope(
                "runtime-alpha",
                new RuntimeDeploymentRequest(
                    " capture/http ",
                    " operator-a ",
                    true,
                    " deploy-001 ",
                    "  ")),
            CancellationToken.None);

        Assert.Equal("capture/http", normalized.Request.PipelineKind);
        Assert.Equal("operator-a", normalized.Request.RequestedBy);
        Assert.Equal("deploy-001", normalized.Request.RequestId);
        Assert.Null(normalized.Request.Target);
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

    [Fact]
    public async Task BridgeResponseReaderBoundsAndStrictlyDecodesStdout()
    {
        var valid = new MemoryStream(Encoding.UTF8.GetBytes("{\"label\":\"羽蛇\"}\r\n"));
        Assert.Equal(
            "{\"label\":\"羽蛇\"}",
            await RustCompatibilityBridge.ReadBoundedResponseLineAsync(
                valid,
                CancellationToken.None));

        var oversized = new MemoryStream(
            Enumerable.Repeat((byte)'x', 1024 * 1024 + 1).ToArray());
        var sizeError = await Assert.ThrowsAsync<InvalidDataException>(() =>
            RustCompatibilityBridge.ReadBoundedResponseLineAsync(
                oversized,
                CancellationToken.None));
        Assert.Contains("exceeds 1 MiB", sizeError.Message, StringComparison.Ordinal);

        var malformed = new MemoryStream([0xff, (byte)'\n']);
        var encodingError = await Assert.ThrowsAsync<InvalidDataException>(() =>
            RustCompatibilityBridge.ReadBoundedResponseLineAsync(
                malformed,
                CancellationToken.None));
        Assert.Contains("valid UTF-8", encodingError.Message, StringComparison.Ordinal);
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
