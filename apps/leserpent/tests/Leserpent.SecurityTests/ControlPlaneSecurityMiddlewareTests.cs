using System.Net;
using System.Net.Http.Json;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.HttpOverrides;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class ControlPlaneSecurityMiddlewareTests
{
    private const string StrongAdminToken = "0123456789abcdef0123456789abcdef";

    [Fact]
    public async Task RemoteHealthRequestWithoutTokenReturns403()
    {
        await using var app = await BuildTestAppAsync();
        var client = app.GetTestClient();

        var request = new HttpRequestMessage(HttpMethod.Get, "https://localhost/health");
        request.Headers.Add("X-Test-Remote-IP", "10.0.0.8");
        var response = await client.SendAsync(request);

        Assert.Equal(HttpStatusCode.Forbidden, response.StatusCode);
        var body = await response.Content.ReadAsStringAsync();
        Assert.Contains("api_access_denied", body);
    }

    [Fact]
    public async Task RemoteHealthRequestWithTokenReturns200()
    {
        await using var app = await BuildTestAppAsync(("LESERPENT_ADMIN_TOKEN", StrongAdminToken));
        var client = app.GetTestClient();

        var request = new HttpRequestMessage(HttpMethod.Get, "https://localhost/health");
        request.Headers.Add("X-Test-Remote-IP", "10.0.0.8");
        request.Headers.Add(ControlPlaneSecurityPolicy.AdminTokenHeader, StrongAdminToken);
        var response = await client.SendAsync(request);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    [Fact]
    public async Task RemoteHealthRequestWithTokenOverHttpReturns426()
    {
        await using var app = await BuildTestAppAsync(("LESERPENT_ADMIN_TOKEN", StrongAdminToken));
        var client = app.GetTestClient();

        var request = new HttpRequestMessage(HttpMethod.Get, "/health");
        request.Headers.Add("X-Test-Remote-IP", "10.0.0.8");
        request.Headers.Add(ControlPlaneSecurityPolicy.AdminTokenHeader, StrongAdminToken);
        var response = await client.SendAsync(request);

        Assert.Equal(HttpStatusCode.UpgradeRequired, response.StatusCode);
        var body = await response.Content.ReadAsStringAsync();
        Assert.Contains("https_required", body);
    }

    [Fact]
    public async Task ForwardedRemoteRequestThroughLoopbackProxyRequiresToken()
    {
        await using var app = await BuildTestAppAsync(("LESERPENT_ADMIN_TOKEN", StrongAdminToken));
        var client = app.GetTestClient();

        var request = new HttpRequestMessage(HttpMethod.Get, "/health");
        request.Headers.Add("X-Forwarded-For", "10.0.0.8");
        request.Headers.Add("X-Forwarded-Proto", "https");
        var response = await client.SendAsync(request);

        Assert.Equal(HttpStatusCode.Forbidden, response.StatusCode);
        var body = await response.Content.ReadAsStringAsync();
        Assert.Contains("api_access_denied", body);
    }

    [Fact]
    public async Task LoopbackMutationWithoutIntentReturns400()
    {
        await using var app = await BuildTestAppAsync();
        var client = app.GetTestClient();

        var response = await client.PostAsJsonAsync("/v1/persistence/save", new { });

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        var body = await response.Content.ReadAsStringAsync();
        Assert.Contains("missing_control_plane_intent", body);
    }

    [Fact]
    public async Task LoopbackMutationWithIntentReturns200()
    {
        await using var app = await BuildTestAppAsync();
        var client = app.GetTestClient();

        var request = new HttpRequestMessage(HttpMethod.Post, "/v1/persistence/save")
        {
            Content = JsonContent.Create(new { }),
        };
        request.Headers.Add(
            ControlPlaneSecurityPolicy.IntentHeader,
            ControlPlaneSecurityPolicy.MutateIntent);
        var response = await client.SendAsync(request);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    [Fact]
    public async Task RemoteMutationWithTokenDoesNotRequireIntentHeader()
    {
        await using var app = await BuildTestAppAsync(("LESERPENT_ADMIN_TOKEN", StrongAdminToken));
        var client = app.GetTestClient();

        var request = new HttpRequestMessage(HttpMethod.Post, "https://localhost/v1/persistence/save")
        {
            Content = JsonContent.Create(new { }),
        };
        request.Headers.Add("X-Test-Remote-IP", "10.0.0.8");
        request.Headers.Add(ControlPlaneSecurityPolicy.AdminTokenHeader, StrongAdminToken);
        var response = await client.SendAsync(request);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    private static async Task<WebApplication> BuildTestAppAsync(
        params (string Key, string Value)[] configValues)
    {
        var builder = WebApplication.CreateBuilder();
        builder.WebHost.UseTestServer();
        builder.Configuration.AddInMemoryCollection(configValues.ToDictionary(
            item => item.Key,
            item => (string?)item.Value));
        builder.Services.Configure<ForwardedHeadersOptions>(options =>
        {
            options.ForwardedHeaders = ForwardedHeaders.XForwardedFor
                | ForwardedHeaders.XForwardedProto;
            options.ForwardLimit = 1;
            options.KnownIPNetworks.Clear();
            options.KnownProxies.Clear();
            options.KnownProxies.Add(IPAddress.Loopback);
            options.KnownProxies.Add(IPAddress.IPv6Loopback);
        });
        builder.Services.AddSingleton<ControlPlaneSecurityPolicy>();

        var app = builder.Build();
        app.Use(async (context, next) =>
        {
            if (context.Request.Headers.TryGetValue("X-Test-Remote-IP", out var value)
                && IPAddress.TryParse(value.ToString(), out var parsed))
            {
                context.Connection.RemoteIpAddress = parsed;
            }
            else if (context.Connection.RemoteIpAddress is null)
            {
                context.Connection.RemoteIpAddress = IPAddress.Loopback;
            }

            await next();
        });
        app.UseForwardedHeaders();
        app.Use(async (context, next) =>
        {
            var security = context.RequestServices.GetRequiredService<ControlPlaneSecurityPolicy>();
            if (!security.TryAuthorize(context, out var statusCode, out var payload))
            {
                context.Response.StatusCode = statusCode;
                await context.Response.WriteAsJsonAsync(payload);
                return;
            }

            await next();
        });
        app.MapGet("/health", () => Results.Ok(new { ok = true }));
        app.MapPost("/v1/persistence/save", () => Results.Ok(new { ok = true }));
        await app.StartAsync();
        return app;
    }
}
