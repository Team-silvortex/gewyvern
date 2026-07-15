using System.Text.Json;
using Leserpent.ControlPlane;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RustCompatibilityFixtureTests
{
    [Fact]
    public void RuntimeListFixtureMatchesCurrentAspNetContract()
    {
        var response = JsonSerializer.Deserialize(
            File.ReadAllText(FixturePath("legacy-runtime-list-response-v1.json")),
            LeserpentJsonContext.Default.RuntimeCollectionResponse);

        Assert.NotNull(response);
        Assert.Equal(" production ", response.Filter.Environment);
        Assert.Equal("EDGE", response.Filter.Role);
        Assert.Collection(
            response.Runtimes,
            runtime =>
            {
                Assert.Equal("runtime-alpha", runtime.RuntimeId);
                Assert.Equal("gewyvern-api", runtime.Status.StatusSource);
                Assert.Equal(1, runtime.Status.TargetCount);
            },
            runtime => Assert.Equal("runtime-bravo", runtime.RuntimeId));
    }

    [Fact]
    public void StatusRefreshAndNotFoundFixturesMatchCurrentAspNetContract()
    {
        var refresh = JsonSerializer.Deserialize(
            File.ReadAllText(FixturePath("legacy-runtime-status-refresh-response-v1.json")),
            LeserpentJsonContext.Default.RuntimeStatusRefreshResponse);
        var notFound = JsonSerializer.Deserialize(
            File.ReadAllText(FixturePath("legacy-runtime-not-found-v1.json")),
            LeserpentJsonContext.Default.ApiErrorResponse);

        Assert.NotNull(refresh);
        Assert.Equal("runtime-alpha", refresh.RuntimeId);
        Assert.Equal(2, refresh.Status.TargetCount);
        Assert.NotNull(notFound);
        Assert.Equal("runtime_not_found", notFound.Error);
        Assert.Equal("runtime-missing", notFound.RuntimeId);
    }

    private static string FixturePath(string fileName) =>
        Path.Combine(AppContext.BaseDirectory, "CompatibilityFixtures", fileName);
}
