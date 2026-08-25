using System.Net;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class ControlPlaneSecurityPolicyTests
{
    [Fact]
    public void RemoteRequestWithoutTokenIsDenied()
    {
        var policy = BuildPolicy();
        var context = BuildContext("GET", "/health", IPAddress.Parse("10.0.0.8"));

        var allowed = policy.TryAuthorize(context, out var statusCode, out var payload);

        Assert.False(allowed);
        Assert.Equal(StatusCodes.Status403Forbidden, statusCode);
        Assert.Contains("api_access_denied", payload.ToString());
    }

    [Fact]
    public void RemoteRequestWithValidTokenIsAllowed()
    {
        var policy = BuildPolicy(("LESERPENT_ADMIN_TOKEN", "secret-token"));
        var context = BuildContext("GET", "/health", IPAddress.Parse("10.0.0.8"));
        context.Request.Headers[ControlPlaneSecurityPolicy.AdminTokenHeader] = " secret-token ";

        var allowed = policy.TryAuthorize(context, out var statusCode, out _);

        Assert.True(allowed);
        Assert.Equal(StatusCodes.Status200OK, statusCode);
    }

    [Fact]
    public void LoopbackMutationWithoutIntentHeaderIsRejected()
    {
        var policy = BuildPolicy();
        var context = BuildContext("POST", "/v1/persistence/save", IPAddress.Loopback);

        var allowed = policy.TryAuthorize(context, out var statusCode, out var payload);

        Assert.False(allowed);
        Assert.Equal(StatusCodes.Status400BadRequest, statusCode);
        Assert.Contains("missing_control_plane_intent", payload.ToString());
    }

    [Fact]
    public void LoopbackMutationWithIntentHeaderIsAllowed()
    {
        var policy = BuildPolicy();
        var context = BuildContext("POST", "/v1/persistence/save", IPAddress.Loopback);
        context.Request.Headers[ControlPlaneSecurityPolicy.IntentHeader] =
            ControlPlaneSecurityPolicy.MutateIntent;

        var allowed = policy.TryAuthorize(context, out var statusCode, out _);

        Assert.True(allowed);
        Assert.Equal(StatusCodes.Status200OK, statusCode);
    }

    [Fact]
    public void ReadOnlyRegistrationPlanDoesNotRequireMutationIntent()
    {
        var policy = BuildPolicy();
        var context = BuildContext(
            "POST",
            "/v1/runtimes/registration-plan",
            IPAddress.Loopback);

        var allowed = policy.TryAuthorize(
            context,
            out var statusCode,
            out _);

        Assert.True(allowed);
        Assert.Equal(StatusCodes.Status200OK, statusCode);
    }

    [Fact]
    public void PersistenceImportOverLimitIsRejectedBeforeBodyRead()
    {
        var policy = BuildPolicy();
        var context = BuildContext("POST", "/v1/persistence/import", IPAddress.Loopback);
        context.Request.ContentLength = ControlPlaneSecurityPolicy.PersistenceImportBodyLimitBytes + 1;

        var allowed = policy.TryAuthorize(context, out var statusCode, out var payload);

        Assert.False(allowed);
        Assert.Equal(StatusCodes.Status413PayloadTooLarge, statusCode);
        Assert.Contains("persistence_import_too_large", payload.ToString());
    }

    [Fact]
    public async Task PersistenceImportRejectsPendingRegistrationIntent()
    {
        var policy = BuildPolicy();
        var now = DateTimeOffset.UtcNow;
        var tags = new RuntimeTags(null, null, null);
        var runtimeId = "runtime-import";
        var name = "Runtime Import";
        var endpoint = "http://127.0.0.1:49152";
        var intent = new PersistedRuntimeRegistrationIntent(
            RuntimeRegistrationCommandIdentity.ForIntent(
                runtimeId,
                name,
                endpoint,
                null,
                tags,
                null),
            runtimeId,
            RuntimeRegistrationPolicy.CreateAction,
            null,
            name,
            endpoint,
            null,
            tags,
            Array.Empty<RuntimeCapability>(),
            false,
            null,
            null,
            null,
            now);
        var state = new PersistedControlPlaneState(
            9,
            now,
            Array.Empty<PersistedRuntimeState>(),
            Array.Empty<PersistedSessionState>(),
            PendingRuntimeRegistrations: [intent]);

        var error = await policy.ValidateImportAsync(
            state,
            CancellationToken.None);

        Assert.Equal(
            "pending runtime registration intents cannot be imported",
            error);
    }

    [Fact]
    public async Task PublicRuntimeEndpointsAreRejectedByDefault()
    {
        var policy = BuildPolicy();

        var error = await policy.ValidateEndpointUrlAsync(
            "http://8.8.8.8/health",
            "runtime endpoint",
            CancellationToken.None);

        Assert.Equal(
            "runtime endpoint must resolve only to loopback or private-network addresses unless LESERPENT_ALLOW_PUBLIC_ENDPOINTS=true",
            error);
    }

    [Fact]
    public async Task EmbeddedCredentialsAreRejected()
    {
        var policy = BuildPolicy();

        var error = await policy.ValidateEndpointUrlAsync(
            "http://user:pass@127.0.0.1/health",
            "runtime endpoint",
            CancellationToken.None);

        Assert.Equal("runtime endpoint may not embed user credentials", error);
    }

    [Fact]
    public async Task PrivateRuntimeEndpointReturnsPinnedAddress()
    {
        var policy = BuildPolicy();

        var result = await policy.BuildEndpointAccessPlanAsync(
            "http://10.0.0.7:4200/health",
            "runtime endpoint",
            CancellationToken.None);

        Assert.Null(result.Error);
        Assert.NotNull(result.Plan);
        Assert.Equal(IPAddress.Parse("10.0.0.7"), result.Plan!.PinnedAddress);
        Assert.Equal("http://10.0.0.7:4200/health", result.Plan.RequestUri.ToString());
    }

    private static ControlPlaneSecurityPolicy BuildPolicy(
        params (string Key, string Value)[] values)
    {
        var config = new ConfigurationBuilder()
            .AddInMemoryCollection(values.ToDictionary(
                item => item.Key,
                item => (string?)item.Value))
            .Build();
        return new ControlPlaneSecurityPolicy(config);
    }

    private static DefaultHttpContext BuildContext(
        string method,
        string path,
        IPAddress remoteIp)
    {
        var context = new DefaultHttpContext();
        context.Request.Method = method;
        context.Request.Path = path;
        context.Connection.RemoteIpAddress = remoteIp;
        return context;
    }
}
