using System.Net;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.TestHost;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class LanguagePackRequestPolicyTests
{
    [Fact]
    public async Task PublicLanguagePackRequestWithoutCredentialsIsAccepted()
    {
        await using var app = await BuildTestAppAsync();

        var response = await app.GetTestClient().GetAsync("/language-packs/catalog.json");

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    [Theory]
    [InlineData("Authorization", "Bearer secret")]
    [InlineData(ControlPlaneSecurityPolicy.AdminTokenHeader, "secret")]
    public async Task PublicLanguagePackRequestWithCredentialsIsRejected(
        string header,
        string value)
    {
        await using var app = await BuildTestAppAsync();
        using var request = new HttpRequestMessage(
            HttpMethod.Get,
            "/language-packs/catalog.json");
        request.Headers.TryAddWithoutValidation(header, value);

        var response = await app.GetTestClient().SendAsync(request);

        Assert.Equal(HttpStatusCode.BadRequest, response.StatusCode);
        Assert.Contains(
            LanguagePackRequestPolicy.ErrorCode,
            await response.Content.ReadAsStringAsync(),
            StringComparison.Ordinal);
    }

    [Fact]
    public async Task PolicyDoesNotChangeAuthenticatedApiRequests()
    {
        await using var app = await BuildTestAppAsync();
        using var request = new HttpRequestMessage(HttpMethod.Get, "/v1/example");
        request.Headers.Authorization = new("Bearer", "secret");

        var response = await app.GetTestClient().SendAsync(request);

        Assert.Equal(HttpStatusCode.OK, response.StatusCode);
    }

    private static async Task<WebApplication> BuildTestAppAsync()
    {
        var builder = WebApplication.CreateBuilder();
        builder.WebHost.UseTestServer();
        var app = builder.Build();
        app.Use(async (context, next) =>
        {
            if (!LanguagePackRequestPolicy.TryAccept(context.Request, out var payload))
            {
                context.Response.StatusCode = StatusCodes.Status400BadRequest;
                await context.Response.WriteAsJsonAsync(payload);
                return;
            }

            await next();
        });
        app.MapGet("/language-packs/catalog.json", () => Results.Json(new { ok = true }));
        app.MapGet("/v1/example", () => Results.Json(new { ok = true }));
        await app.StartAsync();
        return app;
    }
}
