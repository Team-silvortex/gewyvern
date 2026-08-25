using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class RuntimeReadProjectionTests
{
    [Fact]
    public async Task ConfiguredDaemonProjectionOverridesAuthorityFieldsAndRetainsCompatibilityMetadata()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret",
                    Capabilities: new[] { new RuntimeCapability("manual", "fully_supported", "manual") },
                    Tags: new RuntimeTags("managed", null, null),
                    SidecarEndpoint: "https://sidecar.invalid",
                    SidecarAdminToken: "sidecar-secret"),
                "runtime-a");
            var authoritative = Projection("runtime-a");
            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[] { authoritative }));

            var inspected = Assert.IsType<RuntimeSummary>(
                await reads.InspectAsync("runtime-a", CancellationToken.None));
            Assert.Equal("Daemon Name", inspected.Name);
            Assert.Equal("https://daemon.invalid", inspected.Endpoint);
            Assert.Equal("daemon", inspected.Tags.Environment);
            Assert.Equal("gewyvern-api", inspected.Status.StatusSource);
            Assert.Equal("gewyvern-api", inspected.CapabilitySource);
            Assert.Equal("etragon-api", inspected.SidecarStatus?.StatusSource);
            Assert.Equal("ready", inspected.SidecarStatus?.DaemonStatus);
            Assert.Contains(inspected.Capabilities, item => item.Key == "api.latest_snapshot" && item.Support == "fully_supported");
            Assert.Equal("https://daemon-sidecar.invalid", inspected.SidecarEndpoint);
            Assert.Equal(
                DateTimeOffset.Parse("2026-07-21T08:00:00Z"),
                inspected.RegisteredAt);
            Assert.Equal(
                DateTimeOffset.Parse("2026-07-21T09:30:00Z"),
                inspected.UpdatedAt);
            Assert.True(inspected.HasSidecarAdminToken);
            Assert.True(inspected.HasRuntimeAdminToken);

            var listed = await reads.ListAsync(
                new RuntimeListFilter("daemon", null, null),
                CancellationToken.None);
            var listedRuntime = Assert.Single(listed);
            Assert.Equal(inspected.RuntimeId, listedRuntime.RuntimeId);
            Assert.Equal(inspected.Name, listedRuntime.Name);
            Assert.Equal(inspected.Endpoint, listedRuntime.Endpoint);
            Assert.Equal(inspected.Status, listedRuntime.Status);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task AttentionUsesDaemonProjectionForIdentityAndStatusWhileRetainingManagedRecoveryHistory()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret",
                    Tags: new RuntimeTags("managed", "west", "edge"),
                    SidecarEndpoint: "https://sidecar.invalid",
                    SidecarAdminToken: "sidecar-secret"),
                "runtime-attention");

            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[]
                {
                    Projection("runtime-attention") with
                    {
                        Name = "Daemon Name",
                        Endpoint = "https://daemon.invalid",
                        Tags = new RuntimeTags("daemon", "east", "edge"),
                        Status = Projection("runtime-attention").Status with
                        {
                            StatusSource = "fetch_failed"
                        }
                    }
                }));

            var runtime = Assert.IsType<RuntimeSummary>(
                await reads.InspectAsync("runtime-attention", CancellationToken.None));

            var attention = registry.GetRuntimeAttention("runtime-attention", runtime);
            Assert.NotNull(attention);
            Assert.Equal("Daemon Name", attention!.Name);
            Assert.Equal("https://daemon.invalid", attention.Endpoint);
            Assert.Equal("daemon", attention.Tags.Environment);
            Assert.Equal("fetch_failed", attention.Status.StatusSource);
            Assert.True(attention.NeedsAttention);
            Assert.Equal("critical", attention.Severity);
            Assert.Contains("status_fetch_failed", attention.Reasons);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task UnconfiguredRuntimeRetainsFallbackButConfiguredDaemonOwnsPresence()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var registered = registry.RegisterRuntime(
                new RuntimeRegistrationRequest("Managed", "https://managed.invalid", "secret"),
                "runtime-managed");
            var disabled = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(false, Array.Empty<DaemonRuntimeProjection>()));
            Assert.Equal(
                registered.RuntimeId,
                (await disabled.InspectAsync("runtime-managed", CancellationToken.None))?.RuntimeId);
            Assert.Single(await disabled.ListAsync(
                new RuntimeListFilter(null, null, null),
                CancellationToken.None));

            var authoritative = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, Array.Empty<DaemonRuntimeProjection>()));
            Assert.Null(await authoritative.InspectAsync(
                "runtime-managed",
                CancellationToken.None));
            Assert.Empty(await authoritative.ListAsync(
                new RuntimeListFilter(null, null, null),
                CancellationToken.None));
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task FleetAggregatesUseDaemonMembershipAndAuthorityFields()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Active",
                    "https://managed-active.invalid",
                    "secret",
                    Tags: new RuntimeTags("managed", "west", "edge"),
                    SidecarEndpoint: "https://managed-sidecar.invalid"),
                "runtime-authoritative");
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Only",
                    "https://managed-only.invalid",
                    "secret",
                    Tags: new RuntimeTags("managed", "west", "edge")),
                "runtime-managed-only");

            var filter = new RuntimeListFilter(null, null, null);
            var localFleet = new FleetReadProjectionService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(false, Array.Empty<DaemonRuntimeProjection>())),
                registry);
            Assert.Equal(
                2,
                (await localFleet.GetSummaryAsync(filter, CancellationToken.None)).RuntimeCount);

            var projection = Projection("runtime-authoritative");
            projection = projection with
            {
                Status = projection.Status with
                {
                    StatusSource = "fetch_failed",
                    HasLatestSnapshot = false,
                    HasAnalysisJson = false
                },
                SidecarStatus = projection.SidecarStatus! with
                {
                    StatusSource = "fetch_failed",
                    Healthy = false
                }
            };
            var fleet = new FleetReadProjectionService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(true, new[] { projection })),
                registry);

            var summary = await fleet.GetSummaryAsync(filter, CancellationToken.None);
            Assert.Equal(1, summary.RuntimeCount);
            Assert.Equal(1, summary.RuntimesWithStatusFetchFailed);
            Assert.Equal(1, summary.RuntimesWithSidecarStatusFetchFailed);
            Assert.Equal(1, summary.EnvironmentCounts["daemon"]);
            Assert.False(summary.EnvironmentCounts.ContainsKey("managed"));

            var attention = Assert.Single(
                await fleet.GetRuntimesNeedingAttentionAsync(filter, CancellationToken.None));
            Assert.Equal("runtime-authoritative", attention.RuntimeId);
            Assert.Equal("Daemon Name", attention.Name);
            Assert.Equal("daemon", attention.Tags.Environment);
            Assert.Equal("critical", attention.Severity);
            Assert.Contains("status_fetch_failed", attention.Reasons);
            Assert.Contains("sidecar_status_fetch_failed", attention.Reasons);

            var attentionSummary = await fleet.GetAttentionSummaryAsync(
                filter,
                CancellationToken.None);
            Assert.Equal(1, attentionSummary.CriticalCount);
            Assert.Equal(0, attentionSummary.WarningCount);
            Assert.Equal(1, attentionSummary.ReasonCounts["status_fetch_failed"]);
            Assert.Equal(1, attentionSummary.ReasonCounts["sidecar_status_fetch_failed"]);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task FleetAggregatesPropagateDaemonProjectionFailures()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var fleet = new FleetReadProjectionService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(
                        true,
                        new[] { Projection("runtime-orphan") })),
                registry);
            var filter = new RuntimeListFilter(null, null, null);

            var summaryError = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                fleet.GetSummaryAsync(filter, CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", summaryError.Code);

            var attentionError = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                fleet.GetRuntimesNeedingAttentionAsync(filter, CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", attentionError.Code);

            var attentionSummaryError = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                fleet.GetAttentionSummaryAsync(filter, CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", attentionSummaryError.Code);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task OrchestraPlansUseDaemonAuthorityAndExcludeManagedOnlyRuntimes()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret",
                    Capabilities: new[] { new RuntimeCapability("manual", "fully_supported", "manual") },
                    Tags: new RuntimeTags("managed", "west", "edge"),
                    SidecarEndpoint: "https://managed-sidecar.invalid",
                    SidecarAdminToken: "sidecar-secret"),
                "runtime-authoritative");
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Only",
                    "https://managed-only.invalid",
                    "runtime-secret"),
                "runtime-managed-only");

            var managedRuntime = Assert.IsType<RuntimeSummary>(
                registry.GetRuntime("runtime-authoritative"));
            var managedAttention = Assert.IsType<RuntimeAttentionView>(
                registry.GetRuntimeAttention("runtime-authoritative", managedRuntime));
            var managedTriage = OrchestraPlanner.Build(
                    managedRuntime,
                    managedAttention.Reasons,
                    managedAttention.Severity,
                    managedAttention.NeedsAttention)
                .Single(plan => plan.PlanId == "runtime_triage");

            var daemonRuntime = Projection("runtime-authoritative");
            daemonRuntime = daemonRuntime with
            {
                Status = daemonRuntime.Status with { StatusSource = "fetch_failed" },
                SidecarStatus = daemonRuntime.SidecarStatus! with
                {
                    StatusSource = "fetch_failed",
                    Healthy = false
                }
            };
            var orchestra = new OrchestraRuntimeProjectionService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(true, new[] { daemonRuntime })),
                registry);

            var projected = Assert.IsType<OrchestraRuntimeProjection>(
                await orchestra.ReadAsync("runtime-authoritative", CancellationToken.None));
            Assert.Equal("Daemon Name", projected.Runtime.Name);
            Assert.Equal("https://daemon.invalid", projected.Runtime.Endpoint);
            Assert.Equal("https://daemon-sidecar.invalid", projected.Runtime.SidecarEndpoint);
            Assert.Equal("daemon", projected.Runtime.Tags.Environment);
            Assert.Equal("critical", projected.Attention.Severity);
            Assert.Contains("status_fetch_failed", projected.Attention.Reasons);
            Assert.Contains("sidecar_status_fetch_failed", projected.Attention.Reasons);
            Assert.Contains(projected.Plans, plan => plan.PlanId == "sidecar_coordination");
            Assert.NotEqual(
                managedTriage.Revision,
                projected.Plans.Single(plan => plan.PlanId == "runtime_triage").Revision);
            Assert.Null(await orchestra.ReadAsync(
                "runtime-managed-only",
                CancellationToken.None));
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task OrchestraPlansPropagateDaemonProjectionFailures()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var orchestra = new OrchestraRuntimeProjectionService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(
                        true,
                        new[] { Projection("runtime-orphan") })),
                registry);

            var error = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                orchestra.ReadAsync("runtime-orphan", CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", error.Code);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task CleanupPlansUseDaemonAuthorityAndBindManagedSessions()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret",
                    Tags: new RuntimeTags("managed", "west", "edge")),
                "runtime-authoritative");
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Only",
                    "https://managed-only.invalid",
                    "runtime-secret",
                    Tags: new RuntimeTags("daemon", "east", "edge")),
                "runtime-managed-only");
            var firstSession = Assert.IsType<SessionSummary>(registry.CreateSession(
                new SessionCreateRequest(
                    "runtime-authoritative",
                    "diagnostic",
                    "operator",
                    Array.Empty<SessionCapabilityRequirement>())).Session);

            var daemonRuntime = Projection("runtime-authoritative");
            daemonRuntime = daemonRuntime with
            {
                Status = daemonRuntime.Status with { StatusSource = "fetch_failed" }
            };
            var cleanup = new RuntimeCleanupProjectionService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(true, new[] { daemonRuntime })),
                registry);
            var filter = new RuntimeListFilter("daemon", null, null);

            var plan = await cleanup.ReadAsync(filter, CancellationToken.None);
            var failedTarget = Assert.Single(plan.Failed.Targets);
            Assert.Equal("runtime-authoritative", failedTarget.RuntimeId);
            Assert.Equal("Daemon Name", failedTarget.Name);
            Assert.Equal(1, plan.Failed.SessionCount);
            Assert.Single(plan.Slice.Targets);
            Assert.Equal("CLEAR 1", plan.Slice.Challenge);
            Assert.DoesNotContain(
                plan.Slice.Targets,
                target => target.RuntimeId == "runtime-managed-only");

            var selection = await cleanup.SelectAsync(
                RuntimeCleanupPolicy.FailedKind,
                filter,
                new RuntimeCleanupRequest(plan.Failed.PlanToken),
                CancellationToken.None);
            Assert.Equal(new[] { "runtime-authoritative" }, selection.RuntimeIds);
            Assert.Equal(new[] { firstSession.SessionId }, selection.SessionIds);

            Assert.NotNull(registry.CreateSession(new SessionCreateRequest(
                "runtime-authoritative",
                "follow-up",
                "operator",
                Array.Empty<SessionCapabilityRequirement>())).Session);
            await Assert.ThrowsAsync<RuntimeCleanupPlanMismatchException>(() =>
                cleanup.SelectAsync(
                    RuntimeCleanupPolicy.FailedKind,
                    filter,
                    new RuntimeCleanupRequest(plan.Failed.PlanToken),
                    CancellationToken.None));
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task CleanupPlansPropagateDaemonProjectionFailures()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var cleanup = new RuntimeCleanupProjectionService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(
                        true,
                        new[] { Projection("runtime-orphan") })),
                registry);

            var error = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                cleanup.ReadAsync(
                    new RuntimeListFilter(null, null, null),
                    CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", error.Code);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task LegacyDaemonProjectionRetainsManagedAuthorityTimestamps()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var registered = registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed",
                    "https://managed.invalid",
                    "secret"),
                "runtime-legacy-time");
            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[]
                {
                    Projection("runtime-legacy-time") with
                    {
                        RegisteredAt = null,
                        UpdatedAt = null
                    }
                }));

            var projected = Assert.IsType<RuntimeSummary>(
                await reads.InspectAsync("runtime-legacy-time", CancellationToken.None));
            Assert.Equal(registered.RegisteredAt, projected.RegisteredAt);
            Assert.Equal(
                registry.GetRuntime("runtime-legacy-time")?.UpdatedAt,
                projected.UpdatedAt);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task DaemonOnlyRuntimeFailsClosedWithoutCompatibilityMetadata()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var reads = new RuntimeReadProjectionService(
                registry,
                new FakeDaemonReader(true, new[] { Projection("runtime-orphan") }));
            var error = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                reads.InspectAsync("runtime-orphan", CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", error.Code);

            var listError = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
                reads.ListAsync(
                    new RuntimeListFilter(null, null, null),
                    CancellationToken.None));
            Assert.Equal("daemon_projection_unmapped", listError.Code);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task CommandContextComposesDaemonTargetAndRevisionWithManagedCredentials()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret",
                    Tags: new RuntimeTags("managed", "west", "legacy"),
                    SidecarEndpoint: "https://managed-sidecar.invalid",
                    SidecarAdminToken: "sidecar-secret"),
                "runtime-context");
            var projection = Projection("runtime-context") with
            {
                Revision = 42,
            };
            var contexts = new RuntimeCommandExecutionContextService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(true, new[] { projection })),
                registry);

            var context = Assert.IsType<RuntimeCommandExecutionContext>(
                await contexts.InspectAsync(
                    "runtime-context",
                    CancellationToken.None));
            Assert.Equal(42UL, context.AuthorityRevision);
            Assert.Equal("Daemon Name", context.Runtime.Name);
            Assert.Equal("https://daemon.invalid", context.Runtime.Endpoint);
            Assert.Equal("https://daemon.invalid", context.ControlAccess.Endpoint);
            Assert.Equal("runtime-secret", context.ControlAccess.AdminToken);
            Assert.Equal("daemon", context.ControlAccess.Tags.Environment);
            Assert.Equal(
                "https://daemon-sidecar.invalid",
                context.SidecarAccess?.SidecarEndpoint);
            Assert.Equal("sidecar-secret", context.SidecarAccess?.SidecarAdminToken);
            Assert.DoesNotContain("runtime-secret", context.ToString());
            Assert.DoesNotContain("sidecar-secret", context.ToString());
            Assert.DoesNotContain("daemon.invalid", context.ToString());
            Assert.All(
                new[]
                {
                    context.ControlAccess.Endpoint,
                    context.SidecarAccess!.SidecarEndpoint,
                },
                endpoint => Assert.DoesNotContain("managed.invalid", endpoint));
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task DiscoveryReceiptBindsCommittedProjectionWithoutMovingCredentials()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret"),
                "runtime-context");
            var initialProjection = Projection("runtime-context") with
            {
                Revision = 42,
            };
            var contexts = new RuntimeCommandExecutionContextService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(true, new[] { initialProjection })),
                registry);
            var context = Assert.IsType<RuntimeCommandExecutionContext>(
                await contexts.InspectAsync(
                    "runtime-context",
                    CancellationToken.None));
            var committedStatus = initialProjection.Status with
            {
                SnapshotKind = "receipt-bound",
                TargetCount = 9,
            };
            var receipt = RuntimeDiscoveryIntakeReceipt.FromAuthoritativeCommit(
                initialProjection with
                {
                    Name = "Committed Name",
                    Endpoint = "https://committed.invalid",
                    Revision = 43,
                    Status = committedStatus,
                });
            var observedStatus = RuntimeStatusDiscoveryResult.Failed(
                "https://managed.invalid/v1/latest/status",
                "raw transport detail");

            var commit = contexts.BindDiscoveryReceipt(
                context,
                receipt,
                statusDiscovery: observedStatus);

            Assert.Equal(43UL, commit.Context.AuthorityRevision);
            Assert.Equal("Committed Name", commit.Context.Runtime.Name);
            Assert.Equal("https://committed.invalid", commit.Context.Runtime.Endpoint);
            Assert.Equal(committedStatus, commit.StatusDiscovery?.Status);
            Assert.Equal("runtime-secret", commit.Context.ControlAccess.AdminToken);
            Assert.DoesNotContain("runtime-secret", commit.ToString());
            Assert.DoesNotContain("committed.invalid", commit.ToString());
            Assert.DoesNotContain("committed.invalid", receipt.ToString());
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task AuthoritativeDiscoveryCannotFallBackToAReceiptlessCompatibilityWrite()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Name",
                    "https://managed.invalid",
                    "runtime-secret"),
                "runtime-context");
            var projection = Projection("runtime-context") with
            {
                Revision = 42,
            };
            var contexts = new RuntimeCommandExecutionContextService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(true, new[] { projection })),
                registry);
            var context = Assert.IsType<RuntimeCommandExecutionContext>(
                await contexts.InspectAsync(
                    "runtime-context",
                    CancellationToken.None));
            var managedStatus = registry.GetRuntime("runtime-context")!.Status;

            var error = Assert.Throws<DaemonRuntimeRegistrationException>(() =>
                contexts.BindDiscoveryReceipt(
                    context,
                    RuntimeDiscoveryIntakeReceipt.WithoutAuthoritativeCommit(
                        "runtime-context"),
                    statusDiscovery: RuntimeStatusDiscoveryResult.Failed(
                        "https://managed.invalid/v1/latest/status",
                        "raw transport detail")));

            Assert.Equal("daemon_protocol_invalid", error.Code);
            Assert.Equal(
                managedStatus,
                registry.GetRuntime("runtime-context")?.Status);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RegistrationPlanUsesDaemonIdentityRevisionAndSidecarIntent()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var runtime = Projection("runtime-registration-plan") with
            {
                Revision = 41,
            };
            var daemon = new FakeDaemonReader(true, new[] { runtime });
            var plans = new RuntimeRegistrationPlanProjectionService(
                registry,
                daemon);
            var request = new RuntimeRegistrationPlanRequest(
                runtime.Name.ToUpperInvariant(),
                "https://replacement.invalid",
                "https://sidecar-a.invalid");

            var plan = await plans.BuildAsync(request, CancellationToken.None);

            Assert.True(plan.Allowed);
            Assert.Equal(RuntimeRegistrationPolicy.UpdateAction, plan.Action);
            Assert.Equal(runtime.RuntimeId, plan.ExistingRuntimeId);
            Assert.Equal(runtime.RuntimeId, plan.PlannedRuntimeId);
            Assert.Equal(runtime.Endpoint, plan.ExistingRuntimeEndpoint);
            Assert.Equal(41UL, plan.ExpectedRevision);
            Assert.True(plan.AuthorityBound);
            Assert.Equal(1, daemon.SnapshotCalls);
            Assert.Equal(0, daemon.ListCalls);
            Assert.Equal(0, daemon.InspectCalls);

            var advancedPlan = await new RuntimeRegistrationPlanProjectionService(
                registry,
                new FakeDaemonReader(true, new[]
                {
                    runtime with { Revision = 42 },
                })).BuildAsync(request, CancellationToken.None);
            var changedSidecarPlan = await plans.BuildAsync(
                request with { SidecarEndpoint = "https://sidecar-b.invalid" },
                CancellationToken.None);

            Assert.NotEqual(plan.PlanToken, advancedPlan.PlanToken);
            Assert.NotEqual(plan.PlanToken, changedSidecarPlan.PlanToken);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RegistrationCreatePlanReusesUnmigratedManagedIdWithoutClaimingAnUpdate()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var managed = registry.RegisterRuntime(new RuntimeRegistrationRequest(
                "Legacy Runtime",
                "https://managed.invalid",
                "managed-secret"));
            var daemon = new FakeDaemonReader(
                true,
                Array.Empty<DaemonRuntimeProjection>());
            var plans = new RuntimeRegistrationPlanProjectionService(
                registry,
                daemon);
            var request = new RuntimeRegistrationPlanRequest(
                "LEGACY RUNTIME",
                "https://authority.invalid");

            var first = await plans.BuildAsync(request, CancellationToken.None);
            var second = await plans.BuildAsync(request, CancellationToken.None);

            Assert.True(first.Allowed);
            Assert.Equal(RuntimeRegistrationPolicy.CreateAction, first.Action);
            Assert.Null(first.ExistingRuntimeId);
            Assert.Equal(managed.RuntimeId, first.PlannedRuntimeId);
            Assert.Null(first.ExpectedRevision);
            Assert.True(first.AuthorityBound);
            Assert.Equal(first.PlanToken, second.PlanToken);
            Assert.Equal(2, daemon.SnapshotCalls);
            Assert.Equal(0, daemon.ListCalls);
            Assert.Equal(0, daemon.InspectCalls);
            Assert.DoesNotContain("managed-secret", first.ToString());
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task RegistrationPlanRejectsManagedIdReservedForDeletion()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            var managed = registry.RegisterRuntime(new RuntimeRegistrationRequest(
                "Deleting Runtime",
                "https://managed.invalid",
                "managed-secret"));
            using var deletion = registry.ReserveRuntimeDeletion(
                new[] { managed.RuntimeId });
            var daemon = new FakeDaemonReader(
                true,
                Array.Empty<DaemonRuntimeProjection>());
            var plans = new RuntimeRegistrationPlanProjectionService(
                registry,
                daemon);

            var plan = await plans.BuildAsync(
                new RuntimeRegistrationPlanRequest(
                    "Deleting Runtime",
                    "https://authority.invalid"),
                CancellationToken.None);

            Assert.False(plan.Allowed);
            Assert.Equal(RuntimeRegistrationPolicy.RejectAction, plan.Action);
            Assert.Equal(
                RuntimeRegistrationPolicy.RuntimeDeletionInProgressReason,
                plan.Reason);
            Assert.Equal(managed.RuntimeId, plan.PlannedRuntimeId);
            Assert.Null(plan.ExpectedRevision);
            Assert.True(plan.AuthorityBound);
            Assert.Equal(1, daemon.SnapshotCalls);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void RegistrationReceiptBindsAuthorityProjectionAndKeepsCredentialsLocal()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            const string runtimeId = "runtime-registration-commit";
            var runtime = Projection(runtimeId) with
            {
                Revision = 8,
            };
            var request = new RuntimeRegistrationRequest(
                runtime.Name,
                runtime.Endpoint,
                "runtime-secret",
                Tags: runtime.Tags,
                FetchCapabilities: true,
                SidecarEndpoint: runtime.SidecarEndpoint,
                SidecarAdminToken: "sidecar-secret");
            var capabilityDiscovery = CapabilityDiscoveryResult.Succeeded(
                "https://managed.invalid/v1/capabilities",
                new[]
                {
                    new RuntimeCapability(
                        "stale-local-capability",
                        "unknown",
                        "must be replaced by the daemon receipt"),
                },
                runtime.Capabilities);
            var statusDiscovery = RuntimeStatusDiscoveryResult.Failed(
                "https://managed.invalid/v1/latest/status",
                "raw status detail");
            var sidecarDiscovery = RuntimeSidecarDiscoveryResult.Failed(
                "https://managed-sidecar.invalid/v1/status",
                "raw sidecar detail");
            var service = new RuntimeRegistrationCommitProjectionService();

            var commit = service.Bind(
                runtimeId,
                request,
                RuntimeRegistrationCommitReceipt.FromAuthoritativeCommit(
                    7,
                    runtime,
                    discoveryApplied: true),
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);
            var registered = registry.RegisterRuntimeFromAuthority(
                commit.Request,
                commit.Runtime,
                commit.CapabilityDiscovery);

            Assert.Equal(runtime.Name, registered.Name);
            Assert.Equal(runtime.Endpoint, registered.Endpoint);
            Assert.Equal(runtime.Tags, registered.Tags);
            Assert.Equal(runtime.Status, registered.Status);
            Assert.Equal(runtime.SidecarStatus, registered.SidecarStatus);
            Assert.Equal(
                RuntimeCapabilityProjection.ToLegacy(runtime.Capabilities!),
                registered.Capabilities);
            Assert.True(registered.HasRuntimeAdminToken);
            Assert.True(registered.HasSidecarAdminToken);
            Assert.Equal(runtime.Status, commit.StatusDiscovery?.Status);
            Assert.Equal(runtime.SidecarStatus, commit.SidecarDiscovery?.SidecarStatus);
            Assert.Equal(
                "runtime-secret",
                registry.GetRuntimeControlAccess(runtimeId)?.AdminToken);
            Assert.Equal(
                "sidecar-secret",
                registry.GetRuntimeSidecarAccess(runtimeId)?.SidecarAdminToken);
            Assert.DoesNotContain("runtime-secret", commit.ToString());
            Assert.DoesNotContain("sidecar-secret", commit.ToString());
            Assert.DoesNotContain("daemon.invalid", commit.ToString());

            var fetchedAt = registered.CapabilityFetchedAt;
            var updatedStatus = runtime.Status with
            {
                SnapshotKind = "registration-update",
                TargetCount = 11,
            };
            var updatedRuntime = runtime with
            {
                Revision = 9,
                Status = updatedStatus,
            };
            var updateRequest = request with
            {
                PairingToken = "rotated-runtime-secret",
                SidecarAdminToken = "rotated-sidecar-secret",
                FetchCapabilities = false,
            };
            var updateCommit = service.Bind(
                runtimeId,
                updateRequest,
                RuntimeRegistrationCommitReceipt.FromAuthoritativeCommit(
                    9,
                    updatedRuntime,
                    discoveryApplied: false));
            var updated = registry.RegisterRuntimeFromAuthority(
                updateCommit.Request,
                updateCommit.Runtime);

            Assert.Equal(updatedStatus, updated.Status);
            Assert.Equal(fetchedAt, updated.CapabilityFetchedAt);
            Assert.Equal(
                "rotated-runtime-secret",
                registry.GetRuntimeControlAccess(runtimeId)?.AdminToken);
            Assert.Equal(
                "rotated-sidecar-secret",
                registry.GetRuntimeSidecarAccess(runtimeId)?.SidecarAdminToken);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void AuthorityBoundRegistrationWriteUsesReceiptRuntimeIdOverManagedTopology()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            const string runtimeId = "runtime-authority-write";
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Stale",
                    "https://managed.invalid",
                    "old-secret"),
                runtimeId);
            var runtime = Projection(runtimeId) with
            {
                Revision = 17,
            };
            var request = new RuntimeRegistrationRequest(
                runtime.Name,
                runtime.Endpoint,
                "new-secret",
                Tags: runtime.Tags,
                SidecarEndpoint: runtime.SidecarEndpoint,
                SidecarAdminToken: "new-sidecar-secret",
                RegistrationPlanToken: "daemon-authoritative-plan-token");

            var registered = registry.RegisterRuntimeFromAuthority(
                request,
                runtime);

            Assert.Equal(runtimeId, registered.RuntimeId);
            Assert.Equal(runtime.Name, registered.Name);
            Assert.Equal(runtime.Endpoint, registered.Endpoint);
            Assert.Single(registry.ListRuntimes());
            Assert.Equal(
                "new-secret",
                registry.GetRuntimeControlAccess(runtimeId)?.AdminToken);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public void AuthoritativeRegistrationCannotFallBackToAReceiptlessWrite()
    {
        var service = new RuntimeRegistrationCommitProjectionService();
        var request = new RuntimeRegistrationRequest(
            "Runtime",
            "https://runtime.invalid",
            "runtime-secret");

        var error = Assert.Throws<DaemonRuntimeRegistrationException>(() =>
            service.Bind(
                "runtime-registration",
                request,
                RuntimeRegistrationCommitReceipt.WithoutAuthoritativeCommit(
                    "runtime-registration")));

        Assert.Equal("daemon_protocol_invalid", error.Code);
    }

    [Fact]
    public async Task CommandContextListUsesDaemonMembershipAndDoesNotResurrectManagedSidecar()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Authoritative",
                    "https://managed-authoritative.invalid",
                    "runtime-secret",
                    SidecarEndpoint: "https://managed-sidecar.invalid",
                    SidecarAdminToken: "sidecar-secret"),
                "runtime-authoritative");
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed Only",
                    "https://managed-only.invalid",
                    "managed-only-secret"),
                "runtime-managed-only");
            var projection = Projection("runtime-authoritative") with
            {
                SidecarEndpoint = null,
            };
            var contexts = new RuntimeCommandExecutionContextService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(true, new[] { projection })),
                registry);

            var context = Assert.Single(await contexts.ListAsync(
                new RuntimeListFilter(null, null, null),
                CancellationToken.None));

            Assert.Equal("runtime-authoritative", context.Runtime.RuntimeId);
            Assert.Null(context.SidecarAccess);
            Assert.DoesNotContain(
                "runtime-managed-only",
                (await contexts.ListAsync(
                    new RuntimeListFilter(null, null, null),
                    CancellationToken.None))
                    .Select(value => value.Runtime.RuntimeId));
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    [Fact]
    public async Task CommandContextUsesAuthoritativeSidecarWithoutInventingCredential()
    {
        var (registry, statePath) = CreateRegistry();
        try
        {
            registry.RegisterRuntime(
                new RuntimeRegistrationRequest(
                    "Managed",
                    "https://managed.invalid",
                    "runtime-secret"),
                "runtime-sidecar-context");
            var contexts = new RuntimeCommandExecutionContextService(
                new RuntimeReadProjectionService(
                    registry,
                    new FakeDaemonReader(
                        true,
                        new[] { Projection("runtime-sidecar-context") })),
                registry);

            var context = Assert.IsType<RuntimeCommandExecutionContext>(
                await contexts.InspectAsync(
                    "runtime-sidecar-context",
                    CancellationToken.None));

            Assert.Equal(
                "https://daemon-sidecar.invalid",
                context.SidecarAccess?.SidecarEndpoint);
            Assert.Null(context.SidecarAccess?.SidecarAdminToken);
        }
        finally
        {
            DeleteStateFiles(statePath);
        }
    }

    private static DaemonRuntimeProjection Projection(string runtimeId) =>
        new(
            runtimeId,
            "Daemon Name",
            "https://daemon.invalid",
            "https://daemon-sidecar.invalid",
            DateTimeOffset.Parse("2026-07-21T08:00:00Z"),
            DateTimeOffset.Parse("2026-07-21T09:30:00Z"),
            7,
            new RuntimeTags("daemon", "east", "edge"),
            new RuntimeStatusSnapshot(
                "gewyvern-api",
                DateTimeOffset.Parse("2026-07-20T12:00:00Z"),
                null,
                true,
                "capture",
                3,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                false),
            new RuntimeCapabilityAuthoritySnapshot(
                "gewyvern-api",
                "gewyvern-api",
                "1.2.0",
                true,
                true,
                true,
                true,
                "percent-encoding",
                "A-Z a-z 0-9 . _ ~ :",
                new[] { "/v1/capabilities", "/v1/deployments" },
                new Dictionary<string, bool>()),
            new RuntimeSidecarStatusSnapshot(
                "etragon-api",
                DateTimeOffset.Parse("2026-07-21T09:25:00Z"),
                null,
                true,
                "ready",
                2,
                false,
                4,
                true,
                false));

    private static (RegistryService Registry, string StatePath) CreateRegistry()
    {
        var statePath = Path.Combine(Path.GetTempPath(), $"leserpent-read-projection-{Guid.NewGuid():N}.json");
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(new Dictionary<string, string?> { ["LESERPENT_STATE_PATH"] = statePath })
            .Build();
        var environment = new TestEnvironment(Path.GetDirectoryName(statePath)!);
        var store = new ControlPlaneStateStore(
            configuration,
            environment,
            NullLogger<ControlPlaneStateStore>.Instance);
        return (new RegistryService(store, new InMemoryOrchestraRunStore()), statePath);
    }

    private static void DeleteStateFiles(string statePath)
    {
        File.Delete(statePath);
        File.Delete($"{statePath}.bak");
        File.Delete($"{statePath}.tmp");
    }

    private sealed class FakeDaemonReader(
        bool enabled,
        IReadOnlyList<DaemonRuntimeProjection> runtimes) : IDaemonRuntimeProjectionReader
    {
        public bool Enabled => enabled;
        public int ListCalls { get; private set; }
        public int SnapshotCalls { get; private set; }
        public int InspectCalls { get; private set; }

        public Task<IReadOnlyList<DaemonRuntimeProjection>> ListAsync(
            RuntimeListFilter filter,
            CancellationToken cancellationToken)
        {
            ListCalls += 1;
            return Task.FromResult<IReadOnlyList<DaemonRuntimeProjection>>(runtimes
                .Where(runtime => string.IsNullOrWhiteSpace(filter.Environment)
                    || string.Equals(runtime.Tags.Environment, filter.Environment, StringComparison.OrdinalIgnoreCase))
                .ToArray());
        }

        public Task<DaemonRuntimeProjectionSnapshot> SnapshotAsync(
            CancellationToken cancellationToken)
        {
            SnapshotCalls += 1;
            return Task.FromResult(new DaemonRuntimeProjectionSnapshot(
                1,
                runtimes));
        }

        public Task<DaemonRuntimeProjection?> InspectAsync(
            string runtimeId,
            CancellationToken cancellationToken)
        {
            InspectCalls += 1;
            return Task.FromResult(runtimes.FirstOrDefault(runtime => runtime.RuntimeId == runtimeId));
        }
    }

    private sealed class TestEnvironment(string contentRootPath) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = contentRootPath;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
