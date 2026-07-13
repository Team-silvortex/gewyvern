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
                Assert.Equal("operator_confirmation", plan.ApprovalMode);
                Assert.Contains("no_latest_snapshot", plan.Reasons);
            },
            plan =>
            {
                Assert.Equal("sidecar_coordination", plan.PlanId);
                Assert.Equal("review_first", plan.ExecutionReadiness);
                Assert.Equal("automatic", plan.ExecutionMode);
                Assert.Equal("operator_confirmation", plan.ApprovalMode);
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
    public void LegacyOrchestraRunWithoutApprovalAuditRemainsReadable()
    {
        const string json = """
            {"RunId":"run-legacy","RuntimeId":"runtime-1","PlanId":"runtime_triage","Outcome":"ok","ExecutedAt":"2026-01-01T00:00:00Z","Steps":[]}
            """;

        var run = JsonSerializer.Deserialize<OrchestraRunSummary>(json);

        Assert.NotNull(run);
        Assert.Null(run.ApprovedBy);
        Assert.Null(run.ApprovalNote);
        Assert.Null(run.PlanRevision);
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

    [Theory]
    [InlineData("succeeded", true)]
    [InlineData("degraded", true)]
    [InlineData("failed", true)]
    [InlineData("cancelled", true)]
    [InlineData("queued", false)]
    [InlineData("running", false)]
    public void OrchestraTerminalOutcomeClassificationIsStable(string outcome, bool expected)
    {
        Assert.Equal(expected, RegistryService.IsTerminalOrchestraOutcome(outcome));
    }

    [Fact]
    public void RestoredRunningRunIsMarkedFailedAndRetryable()
    {
        var run = new OrchestraRunSummary(
            "run-active",
            "runtime-1",
            "analysis_recovery",
            "running",
            DateTimeOffset.UtcNow,
            Array.Empty<OrchestraExecutionStepResult>());

        var restored = RegistryService.NormalizeRestoredOrchestraRun(run);

        Assert.Equal("failed", restored.Outcome);
        Assert.NotNull(restored.CompletedAt);
        Assert.Contains(restored.Steps, step => step.Step == "service_restart");
    }

    [Theory]
    [InlineData("queued", "running", true)]
    [InlineData("queued", "cancelled", true)]
    [InlineData("running", "succeeded", true)]
    [InlineData("running", "failed", true)]
    [InlineData("running", "running", false)]
    [InlineData("succeeded", "running", false)]
    [InlineData("cancelled", "queued", false)]
    public void OrchestraStateTransitionsRejectTerminalRegression(string current, string next, bool expected)
    {
        Assert.Equal(expected, RegistryService.CanTransitionOrchestraOutcome(current, next));
    }

    [Fact]
    public void PlanRevisionIsStableAndChangesWithOperatorRelevantInputs()
    {
        var runtime = CreateRuntime(sidecarEndpoint: null, sidecarHealthy: false);

        var first = OrchestraPlanner.Build(runtime, Array.Empty<string>(), "none", false);
        var second = OrchestraPlanner.Build(runtime, Array.Empty<string>(), "none", false);
        var changed = OrchestraPlanner.Build(runtime, new[] { "no_latest_snapshot" }, "warning", true);
        var changedEndpoint = OrchestraPlanner.Build(
            runtime with { Endpoint = "http://runtime-new.test" },
            Array.Empty<string>(),
            "none",
            false);

        Assert.Equal(first.Select(plan => plan.Revision), second.Select(plan => plan.Revision));
        Assert.NotEqual(
            first.Single(plan => plan.PlanId == "analysis_recovery").Revision,
            changed.Single(plan => plan.PlanId == "analysis_recovery").Revision);
        Assert.NotEqual(
            first.Single(plan => plan.PlanId == "runtime_triage").Revision,
            changedEndpoint.Single(plan => plan.PlanId == "runtime_triage").Revision);
        Assert.Equal(
            "operator_confirmation",
            changed.Single(plan => plan.PlanId == "analysis_recovery").ApprovalMode);
    }

    [Fact]
    public void OperatorConfirmedPlanRequiresAttributionAndReason()
    {
        var runtime = CreateRuntime(sidecarEndpoint: null, sidecarHealthy: false);
        var plan = OrchestraPlanner.Build(runtime, new[] { "no_latest_snapshot" }, "warning", true)
            .Single(candidate => candidate.PlanId == "analysis_recovery");

        Assert.Contains("approvedBy", Program.ValidateOrchestraApproval(plan, null, "needed"));
        Assert.Contains("approvalNote", Program.ValidateOrchestraApproval(plan, "operator", null));
        Assert.Null(Program.ValidateOrchestraApproval(plan, "operator", "fresh evidence required"));
        Assert.Contains("80", Program.ValidateOrchestraApproval(plan, new string('a', 81), "needed"));
        Assert.Contains("500", Program.ValidateOrchestraApproval(plan, "operator", new string('n', 501)));
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
