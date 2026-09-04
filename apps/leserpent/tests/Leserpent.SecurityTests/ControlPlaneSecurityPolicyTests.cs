using System.Net;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class ControlPlaneSecurityPolicyTests
{
    private const string StrongAdminToken = "0123456789abcdef0123456789abcdef";

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
        var policy = BuildPolicy(("LESERPENT_ADMIN_TOKEN", StrongAdminToken));
        var context = BuildContext("GET", "/health", IPAddress.Parse("10.0.0.8"));
        context.Request.Headers[ControlPlaneSecurityPolicy.AdminTokenHeader] = $" {StrongAdminToken} ";

        var allowed = policy.TryAuthorize(context, out var statusCode, out _);

        Assert.True(allowed);
        Assert.Equal(StatusCodes.Status200OK, statusCode);
    }

    [Fact]
    public void RemoteHttpRequestWithValidTokenIsDenied()
    {
        var policy = BuildPolicy(("LESERPENT_ADMIN_TOKEN", StrongAdminToken));
        var context = BuildContext("GET", "/health", IPAddress.Parse("10.0.0.8"));
        context.Request.Scheme = "http";
        context.Request.Headers[ControlPlaneSecurityPolicy.AdminTokenHeader] = StrongAdminToken;

        var allowed = policy.TryAuthorize(context, out var statusCode, out var payload);

        Assert.False(allowed);
        Assert.Equal(StatusCodes.Status426UpgradeRequired, statusCode);
        Assert.Contains("https_required", payload.ToString());
    }

    [Fact]
    public void RemoteHttpUiRequestIsDeniedBeforeStaticContent()
    {
        var policy = BuildPolicy(("LESERPENT_ADMIN_TOKEN", StrongAdminToken));
        var context = BuildContext("GET", "/", IPAddress.Parse("10.0.0.8"));
        context.Request.Scheme = "http";

        var allowed = policy.TryAuthorize(context, out var statusCode, out var payload);

        Assert.False(allowed);
        Assert.Equal(StatusCodes.Status426UpgradeRequired, statusCode);
        Assert.Contains("https_required", payload.ToString());
    }

    [Fact]
    public void InvalidConfiguredAdminTokensAreRejected()
    {
        var invalidTokens = new[]
        {
            new string('a', ControlPlaneSecurityPolicy.MinimumAdminTokenLength - 1),
            new string('a', ControlPlaneSecurityPolicy.MaximumAdminTokenLength + 1),
            $"{new string('a', 16)} {new string('b', 16)}",
        };

        foreach (var token in invalidTokens)
        {
            Assert.Throws<InvalidOperationException>(() =>
                BuildPolicy(("LESERPENT_ADMIN_TOKEN", token)));
        }
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
    public void OtherControlPlaneRequestsAreAlsoBoundedBeforeBodyRead()
    {
        var policy = BuildPolicy();
        var context = BuildContext(
            "POST",
            "/v1/runtimes",
            IPAddress.Loopback);
        context.Request.ContentLength =
            ControlPlaneSecurityPolicy.ControlPlaneRequestBodyLimitBytes + 1;

        var allowed = policy.TryAuthorize(
            context,
            out var statusCode,
            out var payload);

        Assert.False(allowed);
        Assert.Equal(StatusCodes.Status413PayloadTooLarge, statusCode);
        Assert.Contains("control_plane_request_too_large", payload.ToString());
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
    public async Task RuntimeEndpointBasesRejectAmbiguousCompositionSyntax()
    {
        var policy = BuildPolicy();
        var endpoints = new[]
        {
            " http://127.0.0.1:49152/",
            "http://127.0.0.1:49152/?mode=full",
            "http://127.0.0.1:49152/#client-fragment",
            "http://127.0.0.1:49152\\ignored",
            "http://127.0.0.1:0/",
            $"http://127.0.0.1/{new string('a', ControlPlaneSecurityPolicy.MaximumEndpointUrlLength)}",
        };

        foreach (var endpoint in endpoints)
        {
            var error = await policy.ValidateRegistrationPlanAsync(
                new RuntimeRegistrationPlanRequest("Runtime", endpoint, null),
                CancellationToken.None);

            Assert.NotNull(error);
        }
    }

    [Fact]
    public async Task RegistrationRejectsUnsafeOrOrphanedOutboundCredentials()
    {
        var policy = BuildPolicy();
        var invalidCredentials = new[]
        {
            "token\r\nInjected: value",
            "token with space",
            "token-é",
            new string('a', ControlPlaneSecurityPolicy.MaximumAdminTokenLength + 1),
        };

        foreach (var credential in invalidCredentials)
        {
            var error = await policy.ValidateRegistrationAsync(
                new RuntimeRegistrationRequest(
                    "Runtime",
                    "http://127.0.0.1:49152",
                    credential),
                CancellationToken.None);

            Assert.Equal(
                $"runtime pairing token must contain at most {ControlPlaneSecurityPolicy.MaximumAdminTokenLength} visible ASCII characters",
                error);
        }

        var orphanedSidecarCredential = await policy.ValidateRegistrationAsync(
            new RuntimeRegistrationRequest(
                "Runtime",
                "http://127.0.0.1:49152",
                "",
                SidecarAdminToken: "sidecar-token"),
            CancellationToken.None);

        Assert.Equal(
            "sidecar admin token requires a sidecar endpoint",
            orphanedSidecarCredential);
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
        context.Request.Scheme = "https";
        context.Connection.RemoteIpAddress = remoteIp;
        return context;
    }
}
