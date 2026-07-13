using Leserpent;
using Leserpent.ControlPlane;
using System.Collections.Immutable;
using System.Text.Json;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class OrchestraPlannerTests
{
    [Fact]
    public void BuildAddsRecoveryAndSidecarPlansForDegradedRuntime()
    {
        var runtime = CreateRuntime(sidecarEndpoint: "http://sidecar.test", sidecarHealthy: false);
        var reasons = new[] { "no_latest_snapshot", "sidecar_status_fetch_failed" };

        var plans = OrchestraPlanner.Build(runtime, reasons, "critical", needsAttention: true);

        Assert.Collection(
            plans,
            plan =>
            {
                Assert.Equal("runtime_triage", plan.PlanId);
                Assert.Equal("automatic", plan.ExecutionMode);
            },
            plan =>
            {
                Assert.Equal("analysis_recovery", plan.PlanId);
                Assert.Equal("medium", plan.RiskLevel);
                Assert.Contains("no_latest_snapshot", plan.Reasons);
            },
            plan =>
            {
                Assert.Equal("sidecar_coordination", plan.PlanId);
                Assert.Equal("review_first", plan.ExecutionReadiness);
                Assert.Equal("automatic", plan.ExecutionMode);
            },
            plan =>
            {
                Assert.Equal("session_preparation", plan.PlanId);
                Assert.Equal("guided", plan.ExecutionMode);
            });
    }

    [Fact]
    public void BuildOmitsSidecarPlanAndUsesSupportedCapabilitiesForSessionHandoff()
    {
        var runtime = CreateRuntime(sidecarEndpoint: null, sidecarHealthy: false);

        var plans = OrchestraPlanner.Build(runtime, Array.Empty<string>(), "none", needsAttention: false);
        var sessionPlan = Assert.Single(plans, plan => plan.PlanId == "session_preparation");

        Assert.DoesNotContain(plans, plan => plan.PlanId == "sidecar_coordination");
        Assert.Equal("ready_now", sessionPlan.ExecutionReadiness);
        Assert.Equal("guided", sessionPlan.ExecutionMode);
        Assert.Equal(new[] { "capture" }, sessionPlan.RequiredCapabilities);
    }

    [Fact]
    public void LegacySchemaOneStateWithoutOrchestraRunsRemainsReadable()
    {
        const string json = """
            {"SchemaVersion":1,"SavedAt":"2026-01-01T00:00:00Z","Runtimes":[],"Sessions":[]}
            """;

        var state = JsonSerializer.Deserialize<PersistedControlPlaneState>(json);

        Assert.NotNull(state);
        Assert.Null(state.OrchestraRuns);
    }

    [Fact]
    public void OrchestraRunHistoryRetainsOnlyNewestThirtyTwoRuns()
    {
        var queue = ImmutableQueue<OrchestraRunSummary>.Empty;
        for (var index = 0; index < 40; index += 1)
        {
            queue = queue.Enqueue(new OrchestraRunSummary(
                $"run-{index}",
                "runtime-1",
                "runtime_triage",
                "ok",
                DateTimeOffset.UnixEpoch.AddMinutes(index),
                Array.Empty<OrchestraExecutionStepResult>()));
        }

        var retained = RegistryService.TrimOrchestraRuns(queue).ToArray();

        Assert.Equal(32, retained.Length);
        Assert.Equal("run-8", retained[0].RunId);
        Assert.Equal("run-39", retained[^1].RunId);
    }

    private static RuntimeSummary CreateRuntime(string? sidecarEndpoint, bool sidecarHealthy)
    {
        var now = DateTimeOffset.UtcNow;
        return new RuntimeSummary(
            "runtime-1",
            "test runtime",
            "http://runtime.test",
            sidecarEndpoint,
            false,
            now,
            now,
            new[]
            {
                new RuntimeCapability("capture", "fully_supported", "capture traffic"),
                new RuntimeCapability("mutate", "unsupported", "mutation disabled"),
            },
            "test",
            now,
            null,
            new RuntimeTags("test", "local", "debug"),
            new RuntimeStatusSnapshot(
                "test", now, null, false, null, 1, true, false, false, false,
                false, false, false, false, false, false),
            sidecarEndpoint is null
                ? null
                : new RuntimeSidecarStatusSnapshot(
                    "test", now, sidecarHealthy ? null : "offline", sidecarHealthy,
                    sidecarHealthy ? "ready" : "offline", 1, false, 0, false, false));
    }
}
