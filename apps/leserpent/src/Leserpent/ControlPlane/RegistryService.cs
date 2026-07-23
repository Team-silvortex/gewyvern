using System.Collections.Concurrent;
using System.Collections.Immutable;

namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    private const int MaxRecoveryActivitiesPerRuntime = 8;
    private const int MaxOrchestraRunsPerRuntime = 32;
    private const int MaxPendingRuntimeDeletionIntents = 256;
    private static readonly TimeSpan GenericFailedRecoveryCooldown = TimeSpan.FromSeconds(30);
    private static readonly TimeSpan AuthFailedRecoveryCooldown = TimeSpan.FromSeconds(60);
    private static readonly TimeSpan NetworkFailedRecoveryCooldown = TimeSpan.FromSeconds(20);
    private static readonly TimeSpan IncompleteDataRecoveryCooldown = TimeSpan.FromSeconds(10);
    private readonly ConcurrentDictionary<string, RuntimeRecord> runtimes = new();
    private readonly ConcurrentDictionary<string, SessionRecord> sessions = new();
    private readonly ConcurrentDictionary<string, ImmutableQueue<RuntimeRecoveryActivity>> recoveryActivities = new();
    private readonly ConcurrentDictionary<string, ImmutableQueue<OrchestraRunSummary>> orchestraRuns = new();
    private readonly ConcurrentDictionary<string, PersistedRuntimeDeletionIntent> pendingRuntimeDeletions = new(StringComparer.Ordinal);
    private readonly HashSet<string> deletingRuntimes = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, string> activeRuntimeDeletionClaims = new(StringComparer.Ordinal);
    private readonly object orchestraRunSync = new();
    private readonly object runtimeRegistrationSync = new();
    private readonly object persistenceSync = new();
    private readonly ControlPlaneStateStore stateStore;
    private readonly IOrchestraRunStore orchestraRunStore;
    private readonly DateTimeOffset? restoredFromSavedAt;

    public int RestoredRuntimeCount { get; }
    public int RestoredSessionCount { get; }
    public DateTimeOffset? RestoredFromSavedAt => restoredFromSavedAt;

    public RegistryService(ControlPlaneStateStore stateStore)
        : this(stateStore, new InMemoryOrchestraRunStore())
    {
    }

    public RegistryService(ControlPlaneStateStore stateStore, IOrchestraRunStore orchestraRunStore)
    {
        this.stateStore = stateStore;
        this.orchestraRunStore = orchestraRunStore;
        var loaded = stateStore.Load();
        restoredFromSavedAt = loaded?.SavedAt;
        (RestoredRuntimeCount, RestoredSessionCount) = RestorePersistedState(loaded);
        RestorePendingRuntimeDeletions(loaded);
        RestoreOrMigrateOrchestraRuns();
    }

    public RuntimeRegistrationResponse RegisterRuntime(
        RuntimeRegistrationRequest request,
        string? runtimeId = null)
    {
        var capabilities = NormalizeCapabilities(request.Capabilities);
        var capabilitySource = request.FetchCapabilities ? "gewyvern-api" : "manual";
        DateTimeOffset? capabilityFetchedAt = request.FetchCapabilities ? DateTimeOffset.UtcNow : null;
        var status = new RuntimeStatusSnapshot(
            "unobserved",
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
        return RegisterRuntimeInternal(
            request,
            capabilities,
            capabilitySource,
            capabilityFetchedAt,
            null,
            status,
            null,
            runtimeId);
    }

    public RuntimeRegistrationResponse RegisterRuntimeFromDiscovery(
        RuntimeRegistrationRequest request,
        CapabilityDiscoveryResult capabilityDiscovery,
        RuntimeStatusDiscoveryResult statusDiscovery,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null,
        string? runtimeId = null)
    {
        var capabilities = capabilityDiscovery.Capabilities.Count > 0
            ? NormalizeCapabilities(capabilityDiscovery.Capabilities)
            : NormalizeCapabilities(request.Capabilities);
        var capabilitySource = capabilityDiscovery.Capabilities.Count > 0
            ? capabilityDiscovery.CapabilitySource
            : "manual";
        return RegisterRuntimeInternal(
            request,
            capabilities,
            capabilitySource,
            capabilityDiscovery.CapabilityFetchedAt,
            capabilityDiscovery.CapabilityFetchError,
            statusDiscovery.Status,
            sidecarDiscovery?.SidecarStatus,
            runtimeId);
    }

    public IReadOnlyList<RuntimeSummary> ListRuntimes(RuntimeListFilter? filter = null) =>
        runtimes.Values
            .Where(runtime => MatchesFilter(runtime, filter))
            .OrderBy(runtime => runtime.Name, StringComparer.OrdinalIgnoreCase)
            .Select(runtime => runtime.ToSummary())
            .ToArray();

    public RuntimeCleanupPlan GetRuntimeCleanupPlan(RuntimeListFilter? filter = null)
    {
        var effectiveFilter = filter ?? new RuntimeListFilter(null, null, null);
        return RuntimeCleanupPolicy.Build(effectiveFilter, ListRuntimes(effectiveFilter), ListSessions());
    }

    public IReadOnlyList<string> GetPlannedRuntimeCleanupTargetIds(
        string kind,
        RuntimeListFilter filter,
        RuntimeCleanupRequest request)
    {
        var plan = GetRuntimeCleanupPlan(filter);
        var action = kind switch
        {
            RuntimeCleanupPolicy.FailedKind => plan.Failed,
            RuntimeCleanupPolicy.UnobservedKind => plan.Unobserved,
            RuntimeCleanupPolicy.SliceKind => plan.Slice,
            _ => throw new ArgumentOutOfRangeException(nameof(kind), kind, "unknown runtime cleanup kind"),
        };
        if (string.IsNullOrWhiteSpace(request.PlanToken) ||
            !string.Equals(request.PlanToken, action.PlanToken, StringComparison.Ordinal))
        {
            throw new RuntimeCleanupPlanMismatchException(
                "runtime cleanup plan changed; review the current targets before retrying");
        }
        if (action.Challenge is not null &&
            !string.Equals(request.Challenge?.Trim(), action.Challenge, StringComparison.Ordinal))
        {
            throw new RuntimeCleanupPlanMismatchException(
                "runtime cleanup challenge does not match the current plan");
        }
        return action.Targets.Select(target => target.RuntimeId).ToArray();
    }

    public RuntimeDeletionReservation ReserveRuntimeDeletion(
        IReadOnlyCollection<string> runtimeIds,
        bool requireAllTargets = false)
    {
        var requestedTargets = runtimeIds
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .OrderBy(static runtimeId => runtimeId, StringComparer.OrdinalIgnoreCase)
            .ToArray();
        var targets = requestedTargets
            .Where(runtimes.ContainsKey)
            .ToArray();
        if (requireAllTargets && targets.Length != requestedTargets.Length)
        {
            throw new RuntimeCleanupPlanMismatchException(
                "runtime cleanup targets changed before deletion reservation");
        }
        if (targets.Length == 0)
        {
            return new RuntimeDeletionReservation(this, string.Empty, string.Empty, targets);
        }

        lock (orchestraRunSync)
        {
            var activeRuns = FindActiveOrchestraRuns(targets);
            if (activeRuns.Count > 0)
            {
                throw new OrchestraRuntimeBusyException(activeRuns);
            }

            var targetSet = targets.ToHashSet(StringComparer.OrdinalIgnoreCase);
            var overlappingIntent = pendingRuntimeDeletions.Values.FirstOrDefault(intent =>
                intent.RuntimeIds.Any(targetSet.Contains));
            if (overlappingIntent is not null)
            {
                var exactMatch = targetSet.SetEquals(overlappingIntent.RuntimeIds);
                if (!exactMatch ||
                    activeRuntimeDeletionClaims.ContainsKey(overlappingIntent.IntentId))
                {
                    throw new RuntimeDeletionInProgressException(targets);
                }

                var claimId = Guid.NewGuid().ToString("n");
                activeRuntimeDeletionClaims[overlappingIntent.IntentId] = claimId;
                return new RuntimeDeletionReservation(
                    this,
                    overlappingIntent.IntentId,
                    claimId,
                    overlappingIntent.RuntimeIds);
            }

            if (pendingRuntimeDeletions.Count >= MaxPendingRuntimeDeletionIntents ||
                targets.Any(deletingRuntimes.Contains))
            {
                throw new RuntimeDeletionInProgressException(targets);
            }

            lock (persistenceSync)
            {
                var createdIntent = new PersistedRuntimeDeletionIntent(
                    $"rdel_{Guid.NewGuid():N}",
                    Array.AsReadOnly(targets),
                    DateTimeOffset.UtcNow);
                pendingRuntimeDeletions[createdIntent.IntentId] = createdIntent;
                var claimId = Guid.NewGuid().ToString("n");
                activeRuntimeDeletionClaims[createdIntent.IntentId] = claimId;
                foreach (var runtimeId in createdIntent.RuntimeIds)
                {
                    deletingRuntimes.Add(runtimeId);
                }
                var reservation = new RuntimeDeletionReservation(
                    this,
                    createdIntent.IntentId,
                    claimId,
                    createdIntent.RuntimeIds);
                try
                {
                    PersistStateStrict();
                    return reservation;
                }
                catch (ControlPlaneStatePersistenceException ex)
                {
                    pendingRuntimeDeletions.TryRemove(createdIntent.IntentId, out _);
                    activeRuntimeDeletionClaims.Remove(createdIntent.IntentId);
                    foreach (var runtimeId in createdIntent.RuntimeIds)
                    {
                        deletingRuntimes.Remove(runtimeId);
                    }
                    reservation.Dispose();
                    throw new OrchestraPersistenceException(
                        "failed to persist runtime deletion intent",
                        ex);
                }
            }
        }
    }

    internal void ReleaseRuntimeDeletionClaim(string intentId, string claimId)
    {
        lock (orchestraRunSync)
        {
            if (activeRuntimeDeletionClaims.TryGetValue(intentId, out var activeClaimId) &&
                string.Equals(activeClaimId, claimId, StringComparison.Ordinal))
            {
                activeRuntimeDeletionClaims.Remove(intentId);
            }
        }
    }

    public IReadOnlyList<RuntimeDeletionReservation> ClaimPendingRuntimeDeletions()
    {
        lock (orchestraRunSync)
        {
            var reservations = new List<RuntimeDeletionReservation>();
            foreach (var intent in pendingRuntimeDeletions.Values
                .OrderBy(static intent => intent.PreparedAt)
                .ThenBy(static intent => intent.IntentId, StringComparer.Ordinal))
            {
                if (activeRuntimeDeletionClaims.ContainsKey(intent.IntentId))
                {
                    continue;
                }

                var claimId = Guid.NewGuid().ToString("n");
                activeRuntimeDeletionClaims[intent.IntentId] = claimId;
                reservations.Add(new RuntimeDeletionReservation(
                    this,
                    intent.IntentId,
                    claimId,
                    intent.RuntimeIds));
            }
            return reservations;
        }
    }

    public IReadOnlyList<PersistedRuntimeDeletionIntent> ListPendingRuntimeDeletions() =>
        pendingRuntimeDeletions.Values
            .OrderBy(static intent => intent.PreparedAt)
            .ThenBy(static intent => intent.IntentId, StringComparer.Ordinal)
            .Select(static intent => intent with
            {
                RuntimeIds = intent.RuntimeIds.ToArray(),
            })
            .ToArray();

    public void CompleteRuntimeDeletion(RuntimeDeletionReservation reservation)
    {
        lock (orchestraRunSync)
        {
            PersistedRuntimeDeletionIntent intent;
            if (!pendingRuntimeDeletions.TryGetValue(reservation.IntentId, out intent!) ||
                !activeRuntimeDeletionClaims.TryGetValue(
                    reservation.IntentId,
                    out var activeClaimId) ||
                !string.Equals(activeClaimId, reservation.ClaimId, StringComparison.Ordinal) ||
                !reservation.RuntimeIds.ToHashSet(StringComparer.OrdinalIgnoreCase)
                    .SetEquals(intent.RuntimeIds))
            {
                throw new InvalidOperationException("runtime deletion intent is no longer pending");
            }

            lock (persistenceSync)
            {
                pendingRuntimeDeletions.TryRemove(intent.IntentId, out _);
                foreach (var runtimeId in intent.RuntimeIds)
                {
                    deletingRuntimes.Remove(runtimeId);
                }

                try
                {
                    PersistStateStrict();
                    activeRuntimeDeletionClaims.Remove(intent.IntentId);
                }
                catch (ControlPlaneStatePersistenceException ex)
                {
                    pendingRuntimeDeletions[intent.IntentId] = intent;
                    foreach (var runtimeId in intent.RuntimeIds)
                    {
                        deletingRuntimes.Add(runtimeId);
                    }
                    throw new OrchestraPersistenceException(
                        "failed to complete runtime deletion intent",
                        ex);
                }
            }
        }
    }

    public RuntimeRegistrationPlan GetRuntimeRegistrationPlan(RuntimeRegistrationPlanRequest request) =>
        RuntimeRegistrationPolicy.Build(request, ListRuntimes());

    public RuntimeSummary? GetRuntime(string runtimeId) =>
        runtimes.TryGetValue(runtimeId, out var runtime) ? runtime.ToSummary() : null;

    public RuntimeAttentionView? GetRuntimeAttention(string runtimeId)
    {
        if (!runtimes.TryGetValue(runtimeId, out var runtime))
        {
            return null;
        }
        return BuildRuntimeAttention(runtime, runtime.ToSummary());
    }

    public RuntimeAttentionView? GetRuntimeAttention(
        string runtimeId,
        RuntimeSummary authoritativeRuntime)
    {
        if (!string.Equals(runtimeId, authoritativeRuntime.RuntimeId, StringComparison.Ordinal))
        {
            return null;
        }
        if (!runtimes.TryGetValue(runtimeId, out var runtime))
        {
            return null;
        }
        return BuildRuntimeAttention(runtime, authoritativeRuntime);
    }

    public RuntimeControlAccess? GetRuntimeControlAccess(string runtimeId) =>
        runtimes.TryGetValue(runtimeId, out var runtime)
            ? new RuntimeControlAccess(
                runtime.RuntimeId,
                runtime.Name,
                runtime.Endpoint,
                runtime.RuntimeAdminToken,
                runtime.Tags)
            : null;

    public FleetSummary GetFleetSummary(RuntimeListFilter? filter = null)
    {
        var values = runtimes.Values
            .Where(runtime => MatchesFilter(runtime, filter))
            .ToArray();
        var snapshotKindCounts = values
            .Where(runtime => runtime.Status.HasLatestSnapshot && !string.IsNullOrWhiteSpace(runtime.Status.SnapshotKind))
            .GroupBy(runtime => runtime.Status.SnapshotKind!, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => group.Count(), StringComparer.OrdinalIgnoreCase);
        var statusSourceCounts = values
            .Where(runtime => !string.IsNullOrWhiteSpace(runtime.Status.StatusSource))
            .GroupBy(runtime => runtime.Status.StatusSource, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => group.Count(), StringComparer.OrdinalIgnoreCase);
        var sidecarStatusSourceCounts = values
            .Where(runtime => runtime.SidecarStatus is not null && !string.IsNullOrWhiteSpace(runtime.SidecarStatus.StatusSource))
            .GroupBy(runtime => runtime.SidecarStatus!.StatusSource, StringComparer.OrdinalIgnoreCase)
            .OrderBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => group.Count(), StringComparer.OrdinalIgnoreCase);
        var environmentCounts = BuildTagCounts(values, runtime => runtime.Tags.Environment);
        var clusterCounts = BuildTagCounts(values, runtime => runtime.Tags.Cluster);
        var roleCounts = BuildTagCounts(values, runtime => runtime.Tags.Role);

        return new FleetSummary(
            values.Length,
            values.Count(runtime => runtime.Status.HasLatestSnapshot),
            values.Count(runtime => runtime.Status.HasSummaryJson),
            values.Count(runtime => runtime.Status.HasAnalysisJson),
            values.Count(runtime => runtime.Status.HasExternalSidecarContext),
            values.Count(runtime => runtime.Status.HasExternalEvidenceChainEnrichment),
            values.Count(runtime => runtime.Status.HasExternalDiagnosticOpinion),
            values.Count(runtime => !string.Equals(runtime.Status.StatusSource, "unobserved", StringComparison.OrdinalIgnoreCase)),
            values.Count(runtime => string.Equals(runtime.Status.StatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase)),
            values.Count(runtime => !string.IsNullOrWhiteSpace(runtime.SidecarEndpoint)),
            values.Count(runtime => runtime.SidecarStatus?.Healthy == true),
            values.Count(runtime => runtime.SidecarStatus is not null && !string.Equals(runtime.SidecarStatus.StatusSource, "unobserved", StringComparison.OrdinalIgnoreCase)),
            values.Count(runtime => string.Equals(runtime.SidecarStatus?.StatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase)),
            values.Count(runtime => runtime.SidecarStatus?.HasEvidenceChainEnrichment == true),
            values.Count(runtime => runtime.SidecarStatus?.HasDiagnosticOpinion == true),
            snapshotKindCounts,
            statusSourceCounts,
            sidecarStatusSourceCounts,
            environmentCounts,
            clusterCounts,
            roleCounts);
    }

    public IReadOnlyList<RuntimeAttentionItem> GetRuntimesNeedingAttention(RuntimeListFilter? filter = null) =>
        runtimes.Values
            .Where(runtime => MatchesFilter(runtime, filter))
            .Select(runtime =>
            {
                var reasons = GetAttentionReasons(runtime.Status, runtime.SidecarStatus);
                var recentActivities = GetRecentRecoveryActivities(runtime.RuntimeId);
                var suggestedActions = GetSuggestedActions(reasons, recentActivities);
                return new RuntimeAttentionItem(
                    runtime.RuntimeId,
                    runtime.Name,
                    runtime.Endpoint,
                    runtime.Tags,
                    runtime.Status,
                    GetAttentionSeverity(reasons),
                    reasons,
                    suggestedActions,
                    recentActivities);
            })
            .Where(item => item.Reasons.Count > 0)
            .OrderByDescending(item => AttentionSeverityRank(item.Severity))
            .ThenBy(item => item.Name, StringComparer.OrdinalIgnoreCase)
            .ToArray();

    public FleetAttentionSummary GetFleetAttentionSummary(RuntimeListFilter? filter = null)
    {
        var items = GetRuntimesNeedingAttention(filter);
        var reasonCounts = items
            .SelectMany(item => item.Reasons)
            .GroupBy(reason => reason, StringComparer.OrdinalIgnoreCase)
            .OrderByDescending(group => group.Count())
            .ThenBy(group => group.Key, StringComparer.OrdinalIgnoreCase)
            .ToDictionary(group => group.Key, group => group.Count(), StringComparer.OrdinalIgnoreCase);

        return new FleetAttentionSummary(
            items.Count(item => string.Equals(item.Severity, "critical", StringComparison.OrdinalIgnoreCase)),
            items.Count(item => string.Equals(item.Severity, "warning", StringComparison.OrdinalIgnoreCase)),
            reasonCounts);
    }

    public DateTimeOffset SaveNow()
    {
        PersistState();
        return stateStore.LastSavedAt ?? DateTimeOffset.UtcNow;
    }

    public PersistedControlPlaneState ExportState() =>
        stateStore.CreateState(
            runtimes.Values
                .OrderBy(runtime => runtime.Name, StringComparer.OrdinalIgnoreCase)
                .Select(runtime => runtime.ToPersistedState())
                .ToArray(),
            sessions.Values
                .OrderByDescending(session => session.CreatedAt)
                .Select(session => session.ToPersistedState())
                .ToArray(),
            orchestraRuns.Values
                .SelectMany(static queue => queue)
                .OrderByDescending(static run => run.ExecutedAt)
                .ToArray(),
            pendingRuntimeDeletions.Values
                .OrderBy(static intent => intent.PreparedAt)
                .ThenBy(static intent => intent.IntentId, StringComparer.Ordinal)
                .ToArray());

    public PersistenceImportResponse ImportState(PersistedControlPlaneState state)
    {
        if (!stateStore.IsCompatible(state))
        {
            throw new InvalidOperationException(
                $"imported state schema {state.SchemaVersion} is not compatible with schema {stateStore.SchemaVersion}");
        }
        if ((state.PendingRuntimeDeletions?.Count ?? 0) > 0)
        {
            throw new InvalidOperationException(
                "pending runtime deletion intents cannot be imported");
        }

        lock (orchestraRunSync)
        {
            if (!pendingRuntimeDeletions.IsEmpty)
            {
                throw new RuntimeDeletionInProgressException(
                    pendingRuntimeDeletions.Values
                        .SelectMany(static intent => intent.RuntimeIds)
                        .ToArray());
            }

            var previousState = ExportState();
            runtimes.Clear();
            sessions.Clear();
            orchestraRuns.Clear();
            var (runtimeCount, sessionCount) = RestorePersistedState(state);
            if (!orchestraRunStore.ReplaceAll(orchestraRuns.Values.SelectMany(static queue => queue).ToArray()))
            {
                runtimes.Clear();
                sessions.Clear();
                orchestraRuns.Clear();
                RestorePersistedState(previousState);
                throw new OrchestraPersistenceException("failed to replace Orchestra database during state import");
            }
            PersistState();
            return new PersistenceImportResponse(
                true,
                runtimeCount,
                sessionCount,
                stateStore.LastSavedAt ?? DateTimeOffset.UtcNow,
                state.SavedAt);
        }
    }

    public RuntimeCapabilityRefreshResponse? RefreshRuntimeCapabilities(string runtimeId, CapabilityDiscoveryResult discovery)
    {
        if (!runtimes.TryGetValue(runtimeId, out var runtime))
        {
            return null;
        }

        var updated = runtime with
        {
            Capabilities = discovery.Capabilities.Count > 0
                ? NormalizeCapabilities(discovery.Capabilities)
                : runtime.Capabilities,
            CapabilitySource = discovery.Capabilities.Count > 0 ? discovery.CapabilitySource : runtime.CapabilitySource,
            CapabilityFetchedAt = discovery.CapabilityFetchedAt,
            CapabilityFetchError = discovery.CapabilityFetchError,
            UpdatedAt = DateTimeOffset.UtcNow,
        };
        runtimes[runtimeId] = updated;
        PersistState();
        return new RuntimeCapabilityRefreshResponse(
            updated.RuntimeId,
            updated.Name,
            updated.Endpoint,
            updated.Capabilities,
            updated.CapabilitySource,
            updated.CapabilityFetchedAt,
            updated.CapabilityFetchError);
    }

    public RuntimeStatusRefreshResponse? RefreshRuntimeStatus(string runtimeId, RuntimeStatusDiscoveryResult discovery)
    {
        if (!runtimes.TryGetValue(runtimeId, out var runtime))
        {
            return null;
        }

        var updated = runtime with
        {
            Status = discovery.Status,
            UpdatedAt = DateTimeOffset.UtcNow,
        };
        runtimes[runtimeId] = updated;
        PersistState();
        return new RuntimeStatusRefreshResponse(
            updated.RuntimeId,
            updated.Name,
            updated.Endpoint,
            updated.Status);
    }

    public RuntimeSidecarRefreshResponse? RefreshRuntimeSidecar(string runtimeId, RuntimeSidecarDiscoveryResult discovery)
    {
        if (!runtimes.TryGetValue(runtimeId, out var runtime))
        {
            return null;
        }

        var updated = runtime with
        {
            SidecarStatus = discovery.SidecarStatus,
            UpdatedAt = DateTimeOffset.UtcNow,
        };
        runtimes[runtimeId] = updated;
        PersistState();
        return new RuntimeSidecarRefreshResponse(
            updated.RuntimeId,
            updated.Name,
            updated.Endpoint,
            updated.SidecarEndpoint,
            !string.IsNullOrWhiteSpace(updated.SidecarAdminToken),
            updated.SidecarStatus);
    }

    public RuntimeSidecarAccess? GetRuntimeSidecarAccess(string runtimeId)
    {
        if (!runtimes.TryGetValue(runtimeId, out var runtime) || string.IsNullOrWhiteSpace(runtime.SidecarEndpoint))
        {
            return null;
        }

        return new RuntimeSidecarAccess(
            runtime.RuntimeId,
            runtime.Name,
            runtime.SidecarEndpoint!,
            runtime.SidecarAdminToken,
            runtime.Tags);
    }

    public (RuntimeSummary? RemovedRuntime, int RemovedSessionCount) DeleteRuntime(string runtimeId)
    {
        if (!runtimes.TryGetValue(runtimeId, out var runtime))
        {
            return (null, 0);
        }

        lock (orchestraRunSync)
        {
            var activeRuns = FindActiveOrchestraRuns(new[] { runtimeId });
            if (activeRuns.Count > 0)
            {
                throw new OrchestraRuntimeBusyException(activeRuns);
            }
            if (!orchestraRunStore.DeleteRuntimes(new[] { runtimeId }))
            {
                throw new OrchestraPersistenceException($"failed to delete Orchestra history for runtime {runtimeId}");
            }
            runtimes.TryRemove(runtimeId, out _);
            orchestraRuns.TryRemove(runtimeId, out _);
        }

        var removedSessionIds = sessions.Values
            .Where(session => string.Equals(session.RuntimeId, runtimeId, StringComparison.OrdinalIgnoreCase))
            .Select(session => session.SessionId)
            .ToArray();

        foreach (var sessionId in removedSessionIds)
        {
            sessions.TryRemove(sessionId, out _);
        }

        recoveryActivities.TryRemove(runtimeId, out _);
        PersistState();
        return (runtime.ToSummary(), removedSessionIds.Length);
    }

    public (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames)
        DeleteRuntimes(RuntimeListFilter? filter = null) =>
        DeleteRuntimesWhere(runtime => MatchesFilter(runtime, filter));

    public (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames)
        DeleteFailedRuntimes(RuntimeListFilter? filter = null) =>
        DeleteRuntimesWhere(runtime =>
            MatchesFilter(runtime, filter) &&
            string.Equals(runtime.Status.StatusSource, "fetch_failed", StringComparison.OrdinalIgnoreCase));

    public (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames)
        DeleteUnobservedRuntimes(RuntimeListFilter? filter = null) =>
        DeleteRuntimesWhere(runtime =>
            MatchesFilter(runtime, filter) &&
            RuntimeCleanupPolicy.IsDeletableUnobserved(runtime.ToSummary()));

    public (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames)
        DeletePlannedRuntimes(string kind, RuntimeListFilter filter, RuntimeCleanupRequest request)
    {
        var targetIds = GetPlannedRuntimeCleanupTargetIds(kind, filter, request)
            .ToHashSet(StringComparer.OrdinalIgnoreCase);
        return DeleteRuntimesWhere(runtime => targetIds.Contains(runtime.RuntimeId));
    }

    internal (int RemovedRuntimeCount, int RemovedSessionCount, IReadOnlyList<string> RemovedRuntimeNames)
        DeleteRuntimesById(IReadOnlyCollection<string> runtimeIds)
    {
        var targetIds = runtimeIds.ToHashSet(StringComparer.OrdinalIgnoreCase);
        return DeleteRuntimesWhere(runtime => targetIds.Contains(runtime.RuntimeId));
    }

    public void RecordRecoveryActivity(string runtimeId, string action, string outcome, string summary)
    {
        var activity = new RuntimeRecoveryActivity(
            action,
            outcome,
            summary,
            DateTimeOffset.UtcNow);
        recoveryActivities.AddOrUpdate(
            runtimeId,
            _ => ImmutableQueue<RuntimeRecoveryActivity>.Empty.Enqueue(activity),
            (_, existing) => TrimRecoveryQueue(existing.Enqueue(activity)));
    }

    public (SessionSummary? Session, IReadOnlyList<CapabilityRejection> Rejections, string? RuntimeMissing)
        CreateSession(SessionCreateRequest request)
    {
        lock (orchestraRunSync)
        {
            if (deletingRuntimes.Contains(request.RuntimeId) ||
                !runtimes.TryGetValue(request.RuntimeId, out var runtime))
            {
                return (null, Array.Empty<CapabilityRejection>(), request.RuntimeId);
            }

            var normalizedRequirements = NormalizeRequirements(request.Requirements);
            var rejections = EvaluateRequirements(runtime.Capabilities, normalizedRequirements);
            if (rejections.Count > 0)
            {
                return (null, rejections, null);
            }

            var now = DateTimeOffset.UtcNow;
            var created = new SessionRecord(
                Guid.NewGuid().ToString("n"),
                runtime.RuntimeId,
                request.PipelineKind.Trim(),
                request.RequestedBy.Trim(),
                "running",
                now,
                now,
                normalizedRequirements);
            sessions[created.SessionId] = created;
            PersistState();
            return (created.ToSummary(), Array.Empty<CapabilityRejection>(), null);
        }
    }

    public IReadOnlyList<SessionSummary> ListSessions() =>
        sessions.Values
            .OrderByDescending(session => session.CreatedAt)
            .Select(session => session.ToSummary())
            .ToArray();

    public SessionSummary? GetSession(string sessionId) =>
        sessions.TryGetValue(sessionId, out var session) ? session.ToSummary() : null;

    public SessionSummary? StopSession(string sessionId)
    {
        if (!sessions.TryGetValue(sessionId, out var session))
        {
            return null;
        }

        var updated = session with
        {
            Status = "stopped",
            UpdatedAt = DateTimeOffset.UtcNow,
        };
        sessions[sessionId] = updated;
        PersistState();
        return updated.ToSummary();
    }

    private RuntimeRegistrationResponse RegisterRuntimeInternal(
        RuntimeRegistrationRequest request,
        IReadOnlyList<RuntimeCapability> capabilities,
        string capabilitySource,
        DateTimeOffset? capabilityFetchedAt,
        string? capabilityFetchError,
        RuntimeStatusSnapshot status,
        RuntimeSidecarStatusSnapshot? sidecarStatus,
        string? runtimeId = null)
    {
        lock (runtimeRegistrationSync)
        {
            return RegisterRuntimeLocked(
                request,
                capabilities,
                capabilitySource,
                capabilityFetchedAt,
                capabilityFetchError,
                status,
                sidecarStatus,
                runtimeId);
        }
    }

    private RuntimeRegistrationResponse RegisterRuntimeLocked(
        RuntimeRegistrationRequest request,
        IReadOnlyList<RuntimeCapability> capabilities,
        string capabilitySource,
        DateTimeOffset? capabilityFetchedAt,
        string? capabilityFetchError,
        RuntimeStatusSnapshot status,
        RuntimeSidecarStatusSnapshot? sidecarStatus,
        string? runtimeId)
    {
        var plan = GetRuntimeRegistrationPlan(new RuntimeRegistrationPlanRequest(
            request.Name,
            request.Endpoint,
            request.SidecarEndpoint));
        if (!plan.Allowed)
        {
            throw new RuntimeRegistrationPlanException(
                "runtime endpoint is already registered to another runtime",
                plan);
        }
        if (!string.IsNullOrWhiteSpace(request.RegistrationPlanToken) &&
            !string.Equals(request.RegistrationPlanToken, plan.PlanToken, StringComparison.Ordinal))
        {
            throw new RuntimeRegistrationPlanException(
                "runtime registration plan changed; review the current target before retrying",
                plan);
        }

        var now = DateTimeOffset.UtcNow;
        var tags = NormalizeTags(request.Tags);
        var existing = plan.ExistingRuntimeId is null
            ? null
            : runtimes.GetValueOrDefault(plan.ExistingRuntimeId);

        if (existing is not null)
        {
            var updated = existing with
            {
                Endpoint = request.Endpoint.Trim(),
                RuntimeAdminToken = NormalizeOptionalSecret(request.PairingToken),
                SidecarEndpoint = NormalizeOptionalEndpoint(request.SidecarEndpoint),
                SidecarAdminToken = NormalizeOptionalSecret(request.SidecarAdminToken),
                Capabilities = capabilities,
                CapabilitySource = capabilitySource,
                CapabilityFetchedAt = capabilityFetchedAt,
                CapabilityFetchError = capabilityFetchError,
                Tags = tags,
                Status = status,
                SidecarStatus = sidecarStatus,
                UpdatedAt = now,
            };
            runtimes[existing.RuntimeId] = updated;
            PersistState();
            return updated.ToRegistrationResponse();
        }

        var created = new RuntimeRecord(
            runtimeId ?? Guid.NewGuid().ToString("n"),
            request.Name.Trim(),
            request.Endpoint.Trim(),
            NormalizeOptionalSecret(request.PairingToken),
            NormalizeOptionalEndpoint(request.SidecarEndpoint),
            NormalizeOptionalSecret(request.SidecarAdminToken),
            now,
            now,
            capabilities,
            capabilitySource,
            capabilityFetchedAt,
            capabilityFetchError,
            tags,
            status,
            sidecarStatus);
        runtimes[created.RuntimeId] = created;
        PersistState();
        return created.ToRegistrationResponse();
    }
}
