using System.Net;
using System.Net.Http.Json;
using Leserpent.ControlPlane;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeDeletionRetryEndpointTests
{
    [Fact]
    public async Task RetryNowEndpointRequiresIntentAndPublishesAudit()
    {
        var statePath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-retry-endpoint-{Guid.NewGuid():N}.json");
        var registry = CreateRegistry(statePath);
        const string runtimeId = "runtime-retry-endpoint";
        registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Retry endpoint runtime",
                "https://retry-endpoint.example",
                "pairing-token"),
            runtimeId);
        registry.ReserveRuntimeDeletion(new[] { runtimeId }).Dispose();
        var attemptedAt = DateTimeOffset.UtcNow;
        using (var reservation = Assert.Single(
            registry.ClaimPendingRuntimeDeletions(1, attemptedAt)))
        {
            registry.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        reservation,
                        RuntimeDeletionFailureCodes.AuthorityUnavailable,
                        attemptedAt),
                });
        }
        var intent = Assert.Single(
            registry.ListPendingRuntimeDeletions());

        await using var app = await BuildTestAppAsync(registry);
        var client = app.GetTestClient();
        var path =
            $"/v1/persistence/runtime-deletions/{intent.IntentId}/retry-now";
        var request = new RuntimeDeletionRetryNowRequest(
            intent.Revision,
            "retry-endpoint-request",
            "operator-a");

        var rejected = await client.PostAsJsonAsync(path, request);
        Assert.Equal(HttpStatusCode.BadRequest, rejected.StatusCode);

        var acceptedRequest = new HttpRequestMessage(HttpMethod.Post, path)
        {
            Content = JsonContent.Create(request),
        };
        acceptedRequest.Headers.Add(
            ControlPlaneSecurityPolicy.IntentHeader,
            ControlPlaneSecurityPolicy.MutateIntent);
        var accepted = await client.SendAsync(acceptedRequest);
        Assert.Equal(HttpStatusCode.OK, accepted.StatusCode);
        var response = await accepted.Content.ReadFromJsonAsync(
            LeserpentJsonContext.Default.RuntimeDeletionRetryNowResponse);
        Assert.NotNull(response);
        Assert.False(response.Replayed);
        Assert.Equal(intent.Revision + 1, response.PendingIntent!.Revision);

        var auditResponse = await client.GetAsync(
            "/v1/persistence/runtime-deletion-retry-audit");
        Assert.Equal(HttpStatusCode.OK, auditResponse.StatusCode);
        var audit = await auditResponse.Content.ReadFromJsonAsync(
            LeserpentJsonContext.Default
                .PersistedRuntimeDeletionRetryAuditArray);
        Assert.Single(audit!);
        Assert.Equal(request.RequestId, audit![0].RequestId);

        File.Delete(statePath);
        File.Delete($"{statePath}.bak");
    }

    private static RegistryService CreateRegistry(string statePath)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?>
            {
                ["LESERPENT_STATE_PATH"] = statePath,
            })
            .Build();
        return new RegistryService(
            new ControlPlaneStateStore(
                configuration,
                new TestHostEnvironment
                {
                    ContentRootPath =
                        Path.GetDirectoryName(statePath)!,
                },
                NullLogger<ControlPlaneStateStore>.Instance),
            new InMemoryOrchestraRunStore());
    }

    private static async Task<WebApplication> BuildTestAppAsync(
        RegistryService registry)
    {
        var builder = WebApplication.CreateBuilder();
        builder.WebHost.UseTestServer();
        builder.Services.AddSingleton(registry);
        builder.Services.AddSingleton<RuntimeDeletionRecoverySignal>();
        builder.Services.AddSingleton<ControlPlaneSecurityPolicy>();
        var app = builder.Build();
        app.Use(async (context, next) =>
        {
            context.Connection.RemoteIpAddress = IPAddress.Loopback;
            await next();
        });
        app.Use(async (context, next) =>
        {
            var security = context.RequestServices
                .GetRequiredService<ControlPlaneSecurityPolicy>();
            if (!security.TryAuthorize(
                    context,
                    out var statusCode,
                    out var payload))
            {
                context.Response.StatusCode = statusCode;
                await context.Response.WriteAsJsonAsync(payload);
                return;
            }
            await next();
        });
        Leserpent.Program.MapPersistenceEndpoints(app);
        await app.StartAsync();
        return app;
    }

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
