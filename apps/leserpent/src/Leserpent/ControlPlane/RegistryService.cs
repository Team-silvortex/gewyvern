using System.Collections.Concurrent;
using System.Collections.Immutable;

namespace Leserpent.ControlPlane;

public sealed partial class RegistryService
{
    private const int MaxRecoveryActivitiesPerRuntime = 8;
    private const int MaxOrchestraRunsPerRuntime = 32;
    private const int MaxPendingRuntimeDeletionIntents =
        ControlPlaneStateValidator.MaxPendingRuntimeDeletionIntents;
    private const int MaxRuntimeDeletionAttempts =
        ControlPlaneStateValidator.MaxRuntimeDeletionAttempts;
    private const int MaxRuntimeDeletionRetryAuditEntries =
        ControlPlaneStateValidator.MaxRuntimeDeletionRetryAuditEntries;
    private const int MaxRuntimeDeletionReconciliationAuditEntries =
        ControlPlaneStateValidator
            .MaxRuntimeDeletionReconciliationAuditEntries;
    private const long MaxRuntimeDeletionRevision =
        ControlPlaneStateValidator.MaxRuntimeDeletionRevision;
    private static readonly TimeSpan MaxRuntimeDeletionRetryDelay =
        ControlPlaneStateValidator.MaxRuntimeDeletionRetryDelay;
    private static readonly TimeSpan GenericFailedRecoveryCooldown = TimeSpan.FromSeconds(30);
    private static readonly TimeSpan AuthFailedRecoveryCooldown = TimeSpan.FromSeconds(60);
    private static readonly TimeSpan NetworkFailedRecoveryCooldown = TimeSpan.FromSeconds(20);
    private static readonly TimeSpan IncompleteDataRecoveryCooldown = TimeSpan.FromSeconds(10);
    private static readonly TimeSpan MaxAutomaticCheckpointRetryDelay =
        TimeSpan.FromSeconds(30);
    private readonly ConcurrentDictionary<string, RuntimeRecord> runtimes = new();
    private readonly ConcurrentDictionary<string, SessionRecord> sessions = new();
    private readonly ConcurrentDictionary<string, ImmutableQueue<RuntimeRecoveryActivity>> recoveryActivities = new();
    private readonly ConcurrentDictionary<string, ImmutableQueue<OrchestraRunSummary>> orchestraRuns = new();
    private readonly ConcurrentDictionary<string, PersistedRuntimeDeletionIntent> pendingRuntimeDeletions = new(StringComparer.Ordinal);
    private ImmutableQueue<PersistedRuntimeDeletionRetryAudit>
        runtimeDeletionRetryAudit =
            ImmutableQueue<PersistedRuntimeDeletionRetryAudit>.Empty;
    private ImmutableQueue<PersistedRuntimeDeletionReconciliationAudit>
        runtimeDeletionReconciliationAudit =
            ImmutableQueue<
                PersistedRuntimeDeletionReconciliationAudit>.Empty;
    private readonly HashSet<string> deletingRuntimes = new(StringComparer.OrdinalIgnoreCase);
    private readonly Dictionary<string, string> activeRuntimeDeletionClaims = new(StringComparer.Ordinal);
    private readonly object orchestraRunSync = new();
    private readonly object runtimeRegistrationSync = new();
    private readonly object persistenceSync = new();
    private readonly ControlPlaneStateStore stateStore;
    private readonly IOrchestraRunStore orchestraRunStore;
    private readonly TimeProvider timeProvider;
    private readonly OrchestraDeleteCheckpointWorkerLease?
        checkpointWorkerLease;
    private readonly ControlPlaneWriterFence? controlPlaneWriterFence;
    private readonly DateTimeOffset? restoredFromSavedAt;
    private OrchestraDeleteReplayAdmissionPressure
        orchestraDeleteReplayAdmissionPressure =
            OrchestraDeleteReplayAdmissionPressure.Healthy;
    private bool lastAutomaticOrchestraDeleteCheckpointAdvanced;
    private DateTimeOffset? lastAutomaticOrchestraDeleteCheckpointAt;
    private PersistedOrchestraDeleteCheckpointMonitor?
        orchestraDeleteCheckpointMonitor;
    private ImmutableQueue<
        PersistedOrchestraDeleteCheckpointAlertDelivery>
        orchestraDeleteCheckpointAlertOutbox =
            ImmutableQueue<
                PersistedOrchestraDeleteCheckpointAlertDelivery>.Empty;

    public int RestoredRuntimeCount { get; }
    public int RestoredSessionCount { get; }
    public DateTimeOffset? RestoredFromSavedAt => restoredFromSavedAt;

    public RegistryService(ControlPlaneStateStore stateStore)
        : this(
            stateStore,
            new InMemoryOrchestraRunStore(),
            TimeProvider.System)
    {
    }

    public RegistryService(
        ControlPlaneStateStore stateStore,
        IOrchestraRunStore orchestraRunStore)
        : this(
            stateStore,
            orchestraRunStore,
            TimeProvider.System,
            checkpointWorkerLease: null,
            controlPlaneWriterFence: null)
    {
    }

    public RegistryService(
        ControlPlaneStateStore stateStore,
        IOrchestraRunStore orchestraRunStore,
        OrchestraDeleteCheckpointWorkerLease checkpointWorkerLease)
        : this(
            stateStore,
            orchestraRunStore,
            TimeProvider.System,
            checkpointWorkerLease,
            controlPlaneWriterFence: null)
    {
    }

    public RegistryService(
        ControlPlaneStateStore stateStore,
        IOrchestraRunStore orchestraRunStore,
        OrchestraDeleteCheckpointWorkerLease checkpointWorkerLease,
        ControlPlaneWriterFence controlPlaneWriterFence)
        : this(
            stateStore,
            orchestraRunStore,
            TimeProvider.System,
            checkpointWorkerLease,
            controlPlaneWriterFence)
    {
    }

    public RegistryService(
        ControlPlaneStateStore stateStore,
        IOrchestraRunStore orchestraRunStore,
        TimeProvider timeProvider)
        : this(
            stateStore,
            orchestraRunStore,
            timeProvider,
            checkpointWorkerLease: null,
            controlPlaneWriterFence: null)
    {
    }

    private RegistryService(
        ControlPlaneStateStore stateStore,
        IOrchestraRunStore orchestraRunStore,
        TimeProvider timeProvider,
        OrchestraDeleteCheckpointWorkerLease?
            checkpointWorkerLease,
        ControlPlaneWriterFence? controlPlaneWriterFence)
    {
        this.stateStore = stateStore;
        this.orchestraRunStore = orchestraRunStore;
        this.timeProvider = timeProvider;
        this.checkpointWorkerLease = checkpointWorkerLease;
        this.controlPlaneWriterFence = controlPlaneWriterFence;
        var loaded = stateStore.Load();
        restoredFromSavedAt = loaded?.SavedAt;
        (RestoredRuntimeCount, RestoredSessionCount) = RestorePersistedState(loaded);
        RestorePendingRuntimeDeletions(loaded);
        runtimeDeletionRetryAudit = NormalizeRuntimeDeletionRetryAudit(loaded);
        runtimeDeletionReconciliationAudit =
            NormalizeRuntimeDeletionReconciliationAudit(loaded);
        orchestraDeleteCheckpointMonitor =
            NormalizeOrchestraDeleteCheckpointMonitor(loaded);
        orchestraDeleteCheckpointAlertOutbox =
            NormalizeOrchestraDeleteCheckpointAlertOutbox(loaded);
        orchestraDeleteReplayAdmissionPressure =
            orchestraDeleteCheckpointMonitor?.AdmissionPressure ??
                OrchestraDeleteReplayAdmissionPressure.Healthy;
        RestoreOrMigrateOrchestraRuns(
            allowRepairs:
                controlPlaneWriterFence is null ||
                controlPlaneWriterFence.IsWriter);
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
        RequireControlPlaneWriter();
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
            return new RuntimeDeletionReservation(
                this,
                string.Empty,
                string.Empty,
                targets,
                string.Empty,
                null,
                false);
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
                    overlappingIntent.RuntimeIds,
                    overlappingIntent.UnregistrationCommandId,
                    overlappingIntent
                        .UnregistrationReplayHorizonFloor,
                    overlappingIntent
                        .UnregistrationMutationMayHaveStarted);
            }

            if (pendingRuntimeDeletions.Count >= MaxPendingRuntimeDeletionIntents ||
                targets.Any(deletingRuntimes.Contains))
            {
                throw new RuntimeDeletionInProgressException(targets);
            }

            lock (persistenceSync)
            {
                var intentId = $"rdel_{Guid.NewGuid():N}";
                var createdIntent = new PersistedRuntimeDeletionIntent(
                    intentId,
                    Array.AsReadOnly(targets),
                    DateTimeOffset.UtcNow,
                    UnregistrationCommandId:
                        RuntimeDeletionCommandIdentity.ForIntent(intentId));
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
                    createdIntent.RuntimeIds,
                    createdIntent.UnregistrationCommandId,
                    createdIntent.UnregistrationReplayHorizonFloor,
                    createdIntent
                        .UnregistrationMutationMayHaveStarted);
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

    public IReadOnlyList<RuntimeDeletionReservation> ClaimPendingRuntimeDeletions(
        int maxCount = MaxPendingRuntimeDeletionIntents,
        DateTimeOffset? eligibleAt = null)
    {
        RequireControlPlaneWriter();
        ArgumentOutOfRangeException.ThrowIfLessThan(maxCount, 1);
        var eligibilityBoundary = eligibleAt ?? DateTimeOffset.UtcNow;
        lock (orchestraRunSync)
        {
            var reservations = new List<RuntimeDeletionReservation>();
            foreach (var intent in pendingRuntimeDeletions.Values
                .Where(intent =>
                    intent.NextAttemptAt is null ||
                    intent.NextAttemptAt <= eligibilityBoundary)
                .OrderBy(static intent => intent.PreparedAt)
                .ThenBy(static intent => intent.IntentId, StringComparer.Ordinal)
                .Take(maxCount))
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
                    intent.RuntimeIds,
                    intent.UnregistrationCommandId,
                    intent.UnregistrationReplayHorizonFloor,
                    intent.UnregistrationMutationMayHaveStarted));
            }
            return reservations;
        }
    }

    internal void FenceRuntimeDeletionMutation(
        RuntimeDeletionReservation reservation,
        ulong replayHorizonFloor)
    {
        RequireControlPlaneWriter();
        if (replayHorizonFloor == 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(replayHorizonFloor));
        }

        lock (orchestraRunSync)
        {
            if (!pendingRuntimeDeletions.TryGetValue(
                    reservation.IntentId,
                    out var intent) ||
                !activeRuntimeDeletionClaims.TryGetValue(
                    reservation.IntentId,
                    out var activeClaimId) ||
                !string.Equals(
                    activeClaimId,
                    reservation.ClaimId,
                    StringComparison.Ordinal) ||
                !string.Equals(
                    reservation.UnregistrationCommandId,
                    intent.UnregistrationCommandId,
                    StringComparison.Ordinal) ||
                reservation.UnregistrationReplayHorizonFloor !=
                    intent.UnregistrationReplayHorizonFloor ||
                reservation.UnregistrationMutationMayHaveStarted !=
                    intent.UnregistrationMutationMayHaveStarted ||
                !reservation.RuntimeIds
                    .ToHashSet(StringComparer.OrdinalIgnoreCase)
                    .SetEquals(intent.RuntimeIds))
            {
                throw new InvalidOperationException(
                    "runtime deletion replay fence does not match a pending claim");
            }
            if (intent.UnregistrationMutationMayHaveStarted)
            {
                if (intent.UnregistrationReplayHorizonFloor !=
                    replayHorizonFloor)
                {
                    throw new InvalidOperationException(
                        "runtime deletion replay fence changed");
                }
                reservation.MarkUnregistrationMutationFenced(
                    replayHorizonFloor);
                return;
            }
            if (intent.Revision >= MaxRuntimeDeletionRevision)
            {
                throw new InvalidOperationException(
                    "runtime deletion replay fence exceeded the revision bound");
            }

            var updated = intent with
            {
                UnregistrationReplayHorizonFloor =
                    replayHorizonFloor,
                UnregistrationMutationMayHaveStarted = true,
                Revision = intent.Revision + 1,
            };
            lock (persistenceSync)
            {
                pendingRuntimeDeletions[intent.IntentId] = updated;
                try
                {
                    PersistStateStrict();
                    reservation.MarkUnregistrationMutationFenced(
                        replayHorizonFloor);
                }
                catch (ControlPlaneStatePersistenceException ex)
                {
                    pendingRuntimeDeletions[intent.IntentId] = intent;
                    throw new OrchestraPersistenceException(
                        "failed to persist runtime deletion replay fence",
                        ex);
                }
            }
        }
    }

    internal static TimeSpan CalculateRuntimeDeletionRetryDelay(
        int attemptCount)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(attemptCount, 1);
        var exponent = Math.Min(attemptCount - 1, 5);
        return TimeSpan.FromSeconds(Math.Min(1 << exponent, 30));
    }

    internal void RecordRuntimeDeletionFailures(
        IReadOnlyCollection<RuntimeDeletionFailure> failures)
    {
        RequireControlPlaneWriter();
        if (failures.Count == 0)
        {
            return;
        }

        lock (orchestraRunSync)
        {
            var previousIntents =
                new Dictionary<string, PersistedRuntimeDeletionIntent>(
                    StringComparer.Ordinal);
            var updatedIntents =
                new Dictionary<string, PersistedRuntimeDeletionIntent>(
                    StringComparer.Ordinal);
            foreach (var failure in failures)
            {
                var reservation = failure.Reservation;
                if (!RuntimeDeletionFailureCodes.IsValid(failure.FailureCode) ||
                    !pendingRuntimeDeletions.TryGetValue(
                        reservation.IntentId,
                        out var intent) ||
                    !previousIntents.TryAdd(reservation.IntentId, intent) ||
                    !activeRuntimeDeletionClaims.TryGetValue(
                        reservation.IntentId,
                        out var activeClaimId) ||
                    !string.Equals(
                        activeClaimId,
                        reservation.ClaimId,
                        StringComparison.Ordinal) ||
                    !string.Equals(
                        reservation.UnregistrationCommandId,
                        intent.UnregistrationCommandId,
                        StringComparison.Ordinal) ||
                    reservation.UnregistrationReplayHorizonFloor !=
                        intent.UnregistrationReplayHorizonFloor ||
                    reservation.UnregistrationMutationMayHaveStarted !=
                        intent.UnregistrationMutationMayHaveStarted ||
                    !reservation.RuntimeIds
                        .ToHashSet(StringComparer.OrdinalIgnoreCase)
                        .SetEquals(intent.RuntimeIds))
                {
                    throw new InvalidOperationException(
                        "runtime deletion failure does not match a pending claim");
                }
                if (failure.AttemptedAt == default ||
                    failure.AttemptedAt < intent.PreparedAt ||
                    failure.AttemptedAt > DateTimeOffset.UtcNow.AddMinutes(5) ||
                    intent.Revision >= MaxRuntimeDeletionRevision)
                {
                    throw new InvalidOperationException(
                        "runtime deletion failure has an invalid attempt timestamp");
                }

                var attemptCount = Math.Min(
                    intent.AttemptCount + 1,
                    MaxRuntimeDeletionAttempts);
                updatedIntents[intent.IntentId] = intent with
                {
                    AttemptCount = attemptCount,
                    LastAttemptAt = failure.AttemptedAt,
                    NextAttemptAt = failure.AttemptedAt.Add(
                        CalculateRuntimeDeletionRetryDelay(attemptCount)),
                    LastFailureCode = failure.FailureCode,
                    Revision = intent.Revision + 1,
                };
            }

            lock (persistenceSync)
            {
                foreach (var updated in updatedIntents.Values)
                {
                    pendingRuntimeDeletions[updated.IntentId] = updated;
                }
                try
                {
                    PersistStateStrict();
                }
                catch (ControlPlaneStatePersistenceException ex)
                {
                    foreach (var previous in previousIntents.Values)
                    {
                        pendingRuntimeDeletions[previous.IntentId] = previous;
                    }
                    throw new OrchestraPersistenceException(
                        "failed to persist runtime deletion retry metadata",
                        ex);
                }
            }
        }
    }

    internal OrchestraDeleteReceipt? CompleteRecoveredRuntimeDeletions(
        IReadOnlyCollection<RuntimeDeletionReservation> reservations,
        OrchestraDeleteCommand? cleanupCommand = null,
        Func<OrchestraDeleteReceipt,
            PersistedRuntimeDeletionReconciliationAudit>?
            reconciliationAuditFactory = null)
    {
        RequireControlPlaneWriter();
        if (reservations.Count == 0)
        {
            return null;
        }
        if ((cleanupCommand is null) !=
                (reconciliationAuditFactory is null) ||
            (cleanupCommand is not null && reservations.Count != 1))
        {
            throw new InvalidOperationException(
                "typed Orchestra cleanup must bind exactly one reconciliation intent");
        }

        lock (orchestraRunSync)
        {
            var intents = new List<PersistedRuntimeDeletionIntent>(reservations.Count);
            var intentIds = new HashSet<string>(StringComparer.Ordinal);
            foreach (var reservation in reservations)
            {
                if (!intentIds.Add(reservation.IntentId) ||
                    !pendingRuntimeDeletions.TryGetValue(
                        reservation.IntentId,
                        out var intent) ||
                    !activeRuntimeDeletionClaims.TryGetValue(
                        reservation.IntentId,
                        out var activeClaimId) ||
                    !string.Equals(
                        activeClaimId,
                        reservation.ClaimId,
                        StringComparison.Ordinal) ||
                    !string.Equals(
                        reservation.UnregistrationCommandId,
                        intent.UnregistrationCommandId,
                        StringComparison.Ordinal) ||
                    reservation.UnregistrationReplayHorizonFloor !=
                        intent.UnregistrationReplayHorizonFloor ||
                    reservation.UnregistrationMutationMayHaveStarted !=
                        intent.UnregistrationMutationMayHaveStarted ||
                    !reservation.RuntimeIds
                        .ToHashSet(StringComparer.OrdinalIgnoreCase)
                        .SetEquals(intent.RuntimeIds))
                {
                    throw new InvalidOperationException(
                        "runtime deletion intent is no longer pending");
                }
                intents.Add(intent);
            }

            var runtimeIds = intents
                .SelectMany(static intent => intent.RuntimeIds)
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToArray();
            var runtimeIdSet = runtimeIds.ToHashSet(StringComparer.OrdinalIgnoreCase);
            var activeRuns = FindActiveOrchestraRuns(runtimeIds);
            if (activeRuns.Count > 0)
            {
                throw new OrchestraRuntimeBusyException(activeRuns);
            }

            var removedRuntimes = runtimes.Values
                .Where(runtime => runtimeIdSet.Contains(runtime.RuntimeId))
                .ToArray();
            var removedSessions = sessions.Values
                .Where(session => runtimeIdSet.Contains(session.RuntimeId))
                .ToArray();
            var removedOrchestraRuns =
                new Dictionary<string, ImmutableQueue<OrchestraRunSummary>>(
                    StringComparer.OrdinalIgnoreCase);
            var removedRecoveryActivities =
                new Dictionary<string, ImmutableQueue<RuntimeRecoveryActivity>>(
                    StringComparer.OrdinalIgnoreCase);
            foreach (var runtimeId in runtimeIds)
            {
                if (orchestraRuns.TryGetValue(runtimeId, out var runs))
                {
                    removedOrchestraRuns[runtimeId] = runs;
                }
                if (recoveryActivities.TryGetValue(runtimeId, out var activities))
                {
                    removedRecoveryActivities[runtimeId] = activities;
                }
            }

            lock (persistenceSync)
            {
                OrchestraDeleteReceipt? cleanupReceipt = null;
                var previousReconciliationAudit =
                    runtimeDeletionReconciliationAudit;
                if (cleanupCommand is null)
                {
                    if (!orchestraRunStore.DeleteRuntimes(runtimeIds))
                    {
                        throw new OrchestraPersistenceException(
                            "failed to delete Orchestra history for recovered runtimes");
                    }
                }
                else
                {
                    SynchronizeOrchestraDeleteReplayCheckpoint();
                    cleanupReceipt =
                        orchestraRunStore.DeleteRuntimes(cleanupCommand);
                    if (cleanupReceipt is null)
                    {
                        throw new OrchestraPersistenceException(
                            "failed to obtain a durable Orchestra cleanup receipt");
                    }
                    ValidateOrchestraDeleteReceipt(
                        cleanupCommand,
                        cleanupReceipt);
                    runtimeDeletionReconciliationAudit =
                        TrimRuntimeDeletionReconciliationAudit(
                            runtimeDeletionReconciliationAudit.Enqueue(
                                reconciliationAuditFactory!(
                                    cleanupReceipt)));
                }

                foreach (var runtimeId in runtimeIds)
                {
                    runtimes.TryRemove(runtimeId, out _);
                    orchestraRuns.TryRemove(runtimeId, out _);
                    recoveryActivities.TryRemove(runtimeId, out _);
                }
                foreach (var session in removedSessions)
                {
                    sessions.TryRemove(session.SessionId, out _);
                }
                foreach (var intent in intents)
                {
                    pendingRuntimeDeletions.TryRemove(intent.IntentId, out _);
                    foreach (var runtimeId in intent.RuntimeIds)
                    {
                        deletingRuntimes.Remove(runtimeId);
                    }
                }

                try
                {
                    PersistStateStrict();
                    foreach (var intent in intents)
                    {
                        activeRuntimeDeletionClaims.Remove(intent.IntentId);
                    }
                    SynchronizeOrchestraDeleteReplayCheckpoint();
                }
                catch (ControlPlaneStatePersistenceException ex)
                {
                    runtimeDeletionReconciliationAudit =
                        previousReconciliationAudit;
                    foreach (var runtime in removedRuntimes)
                    {
                        runtimes[runtime.RuntimeId] = runtime;
                    }
                    foreach (var session in removedSessions)
                    {
                        sessions[session.SessionId] = session;
                    }
                    foreach (var (runtimeId, runs) in removedOrchestraRuns)
                    {
                        orchestraRuns[runtimeId] = runs;
                    }
                    foreach (var (runtimeId, activities) in removedRecoveryActivities)
                    {
                        recoveryActivities[runtimeId] = activities;
                    }
                    foreach (var intent in intents)
                    {
                        pendingRuntimeDeletions[intent.IntentId] = intent;
                        foreach (var runtimeId in intent.RuntimeIds)
                        {
                            deletingRuntimes.Add(runtimeId);
                        }
                    }
                    throw new OrchestraPersistenceException(
                        "failed to persist recovered runtime deletion batch",
                        ex);
                }
                return cleanupReceipt;
            }
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

    public IReadOnlyList<PersistedRuntimeDeletionRetryAudit>
        ListRuntimeDeletionRetryAudit() =>
            runtimeDeletionRetryAudit
                .Reverse()
                .Select(static audit => audit with
                {
                    RuntimeIds = audit.RuntimeIds.ToArray(),
                })
                .ToArray();

    public IReadOnlyList<PersistedRuntimeDeletionReconciliationAudit>
        ListRuntimeDeletionReconciliationAudit() =>
            runtimeDeletionReconciliationAudit
                .Reverse()
                .Select(CloneRuntimeDeletionReconciliationAudit)
                .ToArray();

    public PersistedRuntimeDeletionIntent
        GetRuntimeDeletionReconciliationIntent(string intentId)
    {
        var normalizedIntentId = intentId?.Trim() ?? string.Empty;
        if (!IsValidDeletionIdentifier(normalizedIntentId))
        {
            throw new RuntimeDeletionReconciliationException(
                "invalid_runtime_deletion_reconciliation",
                "runtime deletion reconciliation intent is invalid");
        }

        lock (orchestraRunSync)
        {
            if (!pendingRuntimeDeletions.TryGetValue(
                    normalizedIntentId,
                    out var intent))
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_intent_not_found",
                    "runtime deletion intent was not found");
            }
            EnsureRuntimeDeletionRequiresReconciliation(intent);
            return CloneRuntimeDeletionIntent(intent)!;
        }
    }

    internal RuntimeDeletionReconciliationStart
        BeginRuntimeDeletionReconciliation(
            string intentId,
            RuntimeDeletionReconcileRequest request)
    {
        RequireControlPlaneWriter();
        var normalizedIntentId = intentId?.Trim() ?? string.Empty;
        var normalizedRequestId = request.RequestId?.Trim() ?? string.Empty;
        var normalizedRequestedBy = request.RequestedBy?.Trim() ?? string.Empty;
        if (!request.Confirmed ||
            !IsValidDeletionIdentifier(normalizedIntentId) ||
            !IsValidDeletionIdentifier(normalizedRequestId) ||
            !IsValidRuntimeDeletionRetryActor(normalizedRequestedBy) ||
            request.ExpectedRevision < 1 ||
            request.ExpectedDaemonRevision == 0)
        {
            throw new RuntimeDeletionReconciliationException(
                "invalid_runtime_deletion_reconciliation",
                "runtime deletion reconciliation request is invalid");
        }

        lock (orchestraRunSync)
        {
            var replayedAudit =
                runtimeDeletionReconciliationAudit.FirstOrDefault(audit =>
                    string.Equals(
                        audit.RequestId,
                        normalizedRequestId,
                        StringComparison.Ordinal));
            if (replayedAudit is not null)
            {
                if (!string.Equals(
                        replayedAudit.IntentId,
                        normalizedIntentId,
                        StringComparison.Ordinal) ||
                    replayedAudit.ExpectedRevision !=
                        request.ExpectedRevision ||
                    replayedAudit.DaemonRevision !=
                        request.ExpectedDaemonRevision ||
                    !string.Equals(
                        replayedAudit.RequestedBy,
                        normalizedRequestedBy,
                        StringComparison.Ordinal))
                {
                    throw new RuntimeDeletionReconciliationException(
                        "runtime_deletion_reconciliation_request_conflict",
                        "reconciliation requestId was already used for a different operation");
                }

                SynchronizeOrchestraDeleteReplayCheckpoint();
                return new RuntimeDeletionReconciliationStart(
                    null,
                    new RuntimeDeletionReconcileResponse(
                        true,
                        true,
                        CloneRuntimeDeletionReconciliationAudit(
                            replayedAudit)));
            }

            if (!pendingRuntimeDeletions.TryGetValue(
                    normalizedIntentId,
                    out var intent))
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_intent_not_found",
                    "runtime deletion intent was not found");
            }
            EnsureRuntimeDeletionRequiresReconciliation(intent);
            if (activeRuntimeDeletionClaims.ContainsKey(intent.IntentId))
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_reconciliation_in_progress",
                    "runtime deletion intent is currently being recovered or reconciled");
            }
            if (intent.Revision != request.ExpectedRevision)
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_reconciliation_revision_changed",
                    "runtime deletion intent changed; inspect the reconciliation plan again");
            }

            var claimId = Guid.NewGuid().ToString("n");
            activeRuntimeDeletionClaims[intent.IntentId] = claimId;
            return new RuntimeDeletionReconciliationStart(
                new RuntimeDeletionReservation(
                    this,
                    intent.IntentId,
                    claimId,
                    intent.RuntimeIds,
                    intent.UnregistrationCommandId,
                    intent.UnregistrationReplayHorizonFloor,
                    intent.UnregistrationMutationMayHaveStarted),
                null);
        }
    }

    internal RuntimeDeletionReconcileResponse
        CompleteRuntimeDeletionReconciliation(
            RuntimeDeletionReservation reservation,
            RuntimeDeletionReconcileRequest request,
            DaemonRuntimeProjectionSnapshot daemonSnapshot,
            DateTimeOffset? reconciledAt = null)
    {
        RequireControlPlaneWriter();
        var normalizedRequestId = request.RequestId.Trim();
        var normalizedRequestedBy = request.RequestedBy.Trim();
        lock (orchestraRunSync)
        {
            if (daemonSnapshot.Revision == 0)
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_reconciliation_daemon_revision_invalid",
                    "daemon runtime projection revision is not valid for reconciliation");
            }
            if (daemonSnapshot.Revision !=
                request.ExpectedDaemonRevision)
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_reconciliation_daemon_revision_changed",
                    "daemon runtime projection changed; inspect the reconciliation plan again");
            }
            var targetIds = reservation.RuntimeIds.ToHashSet(
                StringComparer.OrdinalIgnoreCase);
            if (daemonSnapshot.Runtimes.Any(runtime =>
                    targetIds.Contains(runtime.RuntimeId)))
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_reconciliation_target_reappeared",
                    "one or more original runtime identities are present in the daemon projection");
            }
            if (!pendingRuntimeDeletions.TryGetValue(
                    reservation.IntentId,
                    out var intent) ||
                !activeRuntimeDeletionClaims.TryGetValue(
                    reservation.IntentId,
                    out var activeClaimId) ||
                !string.Equals(
                    activeClaimId,
                    reservation.ClaimId,
                    StringComparison.Ordinal) ||
                intent.Revision != request.ExpectedRevision ||
                !reservation.RuntimeIds
                    .ToHashSet(StringComparer.OrdinalIgnoreCase)
                    .SetEquals(intent.RuntimeIds))
            {
                throw new RuntimeDeletionReconciliationException(
                    "runtime_deletion_reconciliation_revision_changed",
                    "runtime deletion intent changed; inspect the reconciliation plan again");
            }
            EnsureRuntimeDeletionRequiresReconciliation(intent);

            var effectiveReconciledAt =
                reconciledAt ?? DateTimeOffset.UtcNow;
            if (effectiveReconciledAt < intent.PreparedAt ||
                effectiveReconciledAt >
                    DateTimeOffset.UtcNow.AddMinutes(5))
            {
                throw new RuntimeDeletionReconciliationException(
                    "invalid_runtime_deletion_reconciliation",
                    "runtime deletion reconciliation timestamp is invalid");
            }

            PersistedRuntimeDeletionReconciliationAudit? audit = null;
            var cleanupCommand = new OrchestraDeleteCommand(
                RuntimeDeletionCommandIdentity.ForOrchestraCleanup(
                    intent.IntentId,
                    intent.Revision),
                intent.RuntimeIds
                    .Order(StringComparer.Ordinal)
                    .ToArray());
            _ = CompleteRecoveredRuntimeDeletions(
                [reservation],
                cleanupCommand,
                receipt =>
                {
                    audit =
                        new PersistedRuntimeDeletionReconciliationAudit(
                            normalizedRequestId,
                            intent.IntentId,
                            intent.RuntimeIds.ToArray(),
                            intent.Revision,
                            daemonSnapshot.Revision,
                            normalizedRequestedBy,
                            effectiveReconciledAt,
                            receipt.CommandId,
                            receipt.OperationGeneration);
                    return audit;
                });

            return new RuntimeDeletionReconcileResponse(
                true,
                false,
                CloneRuntimeDeletionReconciliationAudit(
                    audit ??
                    throw new InvalidOperationException(
                        "Orchestra cleanup receipt was not audited")));
        }
    }

    public RuntimeDeletionRetryNowResponse RetryRuntimeDeletionNow(
        string intentId,
        RuntimeDeletionRetryNowRequest request,
        DateTimeOffset? requestedAt = null)
    {
        RequireControlPlaneWriter();
        var normalizedIntentId = intentId?.Trim() ?? string.Empty;
        var normalizedRequestId = request.RequestId?.Trim() ?? string.Empty;
        var normalizedRequestedBy = request.RequestedBy?.Trim() ?? string.Empty;
        if (!IsValidDeletionIdentifier(normalizedIntentId) ||
            !IsValidDeletionIdentifier(normalizedRequestId) ||
            !IsValidRuntimeDeletionRetryActor(normalizedRequestedBy) ||
            request.ExpectedRevision < 1)
        {
            throw new RuntimeDeletionRetryException(
                "invalid_runtime_deletion_retry",
                "runtime deletion retry request is invalid");
        }

        lock (orchestraRunSync)
        {
            var replayedAudit = runtimeDeletionRetryAudit.FirstOrDefault(audit =>
                string.Equals(
                    audit.RequestId,
                    normalizedRequestId,
                    StringComparison.Ordinal));
            if (replayedAudit is not null)
            {
                if (!string.Equals(
                        replayedAudit.IntentId,
                        normalizedIntentId,
                        StringComparison.Ordinal) ||
                    replayedAudit.ExpectedRevision != request.ExpectedRevision ||
                    !string.Equals(
                        replayedAudit.RequestedBy,
                        normalizedRequestedBy,
                        StringComparison.Ordinal))
                {
                    throw new RuntimeDeletionRetryException(
                        "runtime_deletion_retry_request_conflict",
                        "retry requestId was already used for a different operation");
                }

                pendingRuntimeDeletions.TryGetValue(
                    normalizedIntentId,
                    out var replayedIntent);
                return new RuntimeDeletionRetryNowResponse(
                    true,
                    true,
                    CloneRuntimeDeletionIntent(replayedIntent),
                    CloneRuntimeDeletionRetryAudit(replayedAudit));
            }

            if (!pendingRuntimeDeletions.TryGetValue(
                    normalizedIntentId,
                    out var intent))
            {
                throw new RuntimeDeletionRetryException(
                    "runtime_deletion_intent_not_found",
                    "runtime deletion intent was not found");
            }
            if (activeRuntimeDeletionClaims.ContainsKey(intent.IntentId))
            {
                throw new RuntimeDeletionRetryException(
                    "runtime_deletion_retry_in_progress",
                    "runtime deletion intent is currently being recovered");
            }
            if (intent.Revision != request.ExpectedRevision)
            {
                throw new RuntimeDeletionRetryException(
                    "runtime_deletion_retry_revision_changed",
                    "runtime deletion intent changed; inspect it before retrying");
            }
            if (intent.Revision >= MaxRuntimeDeletionRevision)
            {
                throw new RuntimeDeletionRetryException(
                    "runtime_deletion_retry_revision_exhausted",
                    "runtime deletion intent revision is exhausted");
            }
            var effectiveRequestedAt = requestedAt ?? DateTimeOffset.UtcNow;
            if (requestedAt is null)
            {
                var lastRequestedAt = runtimeDeletionRetryAudit
                    .LastOrDefault()
                    ?.RequestedAt;
                if (lastRequestedAt >= effectiveRequestedAt)
                {
                    effectiveRequestedAt =
                        lastRequestedAt.Value.AddTicks(1);
                }
            }
            if (intent.AttemptCount == 0 ||
                intent.NextAttemptAt is null ||
                intent.NextAttemptAt <= effectiveRequestedAt)
            {
                throw new RuntimeDeletionRetryException(
                    "runtime_deletion_retry_not_deferred",
                    "runtime deletion intent is already eligible for automatic recovery");
            }
            if (effectiveRequestedAt < intent.PreparedAt ||
                effectiveRequestedAt > DateTimeOffset.UtcNow.AddMinutes(5))
            {
                throw new RuntimeDeletionRetryException(
                    "invalid_runtime_deletion_retry",
                    "runtime deletion retry timestamp is invalid");
            }

            var updatedIntent = intent with
            {
                NextAttemptAt = effectiveRequestedAt,
                Revision = intent.Revision + 1,
            };
            var audit = new PersistedRuntimeDeletionRetryAudit(
                normalizedRequestId,
                intent.IntentId,
                intent.RuntimeIds.ToArray(),
                intent.Revision,
                updatedIntent.Revision,
                normalizedRequestedBy,
                effectiveRequestedAt);
            var previousAudit = runtimeDeletionRetryAudit;
            pendingRuntimeDeletions[intent.IntentId] = updatedIntent;
            runtimeDeletionRetryAudit = TrimRuntimeDeletionRetryAudit(
                runtimeDeletionRetryAudit.Enqueue(audit));
            try
            {
                PersistStateStrict();
            }
            catch (ControlPlaneStatePersistenceException ex)
            {
                pendingRuntimeDeletions[intent.IntentId] = intent;
                runtimeDeletionRetryAudit = previousAudit;
                throw new OrchestraPersistenceException(
                    "failed to persist runtime deletion retry request",
                    ex);
            }

            return new RuntimeDeletionRetryNowResponse(
                true,
                false,
                CloneRuntimeDeletionIntent(updatedIntent),
                CloneRuntimeDeletionRetryAudit(audit));
        }
    }

    private static PersistedRuntimeDeletionIntent? CloneRuntimeDeletionIntent(
        PersistedRuntimeDeletionIntent? intent) =>
            intent is null
                ? null
                : intent with { RuntimeIds = intent.RuntimeIds.ToArray() };

    private static PersistedRuntimeDeletionRetryAudit
        CloneRuntimeDeletionRetryAudit(
            PersistedRuntimeDeletionRetryAudit audit) =>
                audit with { RuntimeIds = audit.RuntimeIds.ToArray() };

    private static PersistedRuntimeDeletionReconciliationAudit
        CloneRuntimeDeletionReconciliationAudit(
            PersistedRuntimeDeletionReconciliationAudit audit) =>
                audit with { RuntimeIds = audit.RuntimeIds.ToArray() };

    private static void ValidateOrchestraDeleteReceipt(
        OrchestraDeleteCommand command,
        OrchestraDeleteReceipt receipt)
    {
        var expectedRuntimeIds = command.RuntimeIds
            .Order(StringComparer.Ordinal)
            .ToArray();
        var maximumRunCount =
            checked((ulong)expectedRuntimeIds.Length * 32);
        var maximumEventCount = checked(maximumRunCount * 3);
        if (!string.Equals(
                receipt.CommandId,
                command.CommandId,
                StringComparison.Ordinal) ||
            receipt.OperationGeneration == 0 ||
            !receipt.RuntimeIds.SequenceEqual(
                expectedRuntimeIds,
                StringComparer.Ordinal) ||
            receipt.DeletedRuntimeCount >
                checked((uint)expectedRuntimeIds.Length) ||
            receipt.DeletedRunCount > maximumRunCount ||
            receipt.DeletedEventCount > maximumEventCount ||
            receipt.CommittedAt == default ||
            receipt.CommittedAt >
                DateTimeOffset.UtcNow.AddMinutes(5))
        {
            throw new OrchestraPersistenceException(
                "Orchestra cleanup receipt does not match the reconciliation intent");
        }
    }

    private static void EnsureRuntimeDeletionRequiresReconciliation(
        PersistedRuntimeDeletionIntent intent)
    {
        if (!string.Equals(
                intent.LastFailureCode,
                RuntimeDeletionFailureCodes.ReplayAmbiguous,
                StringComparison.Ordinal) ||
            !intent.UnregistrationMutationMayHaveStarted)
        {
            throw new RuntimeDeletionReconciliationException(
                "runtime_deletion_reconciliation_not_required",
                "runtime deletion intent is not replay-ambiguous");
        }
    }

    private static ImmutableQueue<PersistedRuntimeDeletionRetryAudit>
        TrimRuntimeDeletionRetryAudit(
            ImmutableQueue<PersistedRuntimeDeletionRetryAudit> audit)
    {
        while (audit.Count() > MaxRuntimeDeletionRetryAuditEntries)
        {
            audit = audit.Dequeue();
        }
        return audit;
    }

    private static ImmutableQueue<
        PersistedRuntimeDeletionReconciliationAudit>
        TrimRuntimeDeletionReconciliationAudit(
            ImmutableQueue<
                PersistedRuntimeDeletionReconciliationAudit> audit)
    {
        while (audit.Count() >
            MaxRuntimeDeletionReconciliationAuditEntries)
        {
            audit = audit.Dequeue();
        }
        return audit;
    }

    public OrchestraDeleteReplayCheckpointStatus?
        GetOrchestraDeleteReplayCheckpointStatus()
    {
        if (!orchestraRunStore.SupportsDeleteReplayHorizon)
        {
            return null;
        }
        lock (persistenceSync)
        {
            if (controlPlaneWriterFence is null)
            {
                SynchronizeOrchestraDeleteReplayCheckpoint();
            }
            return BuildOrchestraDeleteReplayCheckpointStatus();
        }
    }

    public OrchestraDeleteCheckpointAlertAcknowledgeResponse
        AcknowledgeOrchestraDeleteCheckpointAlert(
            OrchestraDeleteCheckpointAlertAcknowledgeRequest request,
            DateTimeOffset? acknowledgedAt = null)
    {
        RequireControlPlaneWriter();
        var requestedBy = request.RequestedBy?.Trim() ?? string.Empty;
        var effectiveAcknowledgedAt =
            acknowledgedAt ?? timeProvider.GetUtcNow();
        if (!request.Confirmed ||
            request.AlertGeneration == 0 ||
            !IsValidRuntimeDeletionRetryActor(requestedBy) ||
            effectiveAcknowledgedAt == default ||
            effectiveAcknowledgedAt >
                timeProvider.GetUtcNow().AddMinutes(5))
        {
            throw new OrchestraDeleteCheckpointAlertException(
                "invalid_orchestra_checkpoint_alert_acknowledgement",
                "checkpoint alert acknowledgement is invalid");
        }

        lock (persistenceSync)
        {
            var monitor = orchestraDeleteCheckpointMonitor;
            if (monitor is null ||
                monitor.ConsecutiveFailureCount == 0)
            {
                throw new OrchestraDeleteCheckpointAlertException(
                    "orchestra_checkpoint_alert_not_active",
                    "checkpoint alert is not active");
            }
            if (monitor.AlertGeneration != request.AlertGeneration)
            {
                throw new OrchestraDeleteCheckpointAlertException(
                    "orchestra_checkpoint_alert_generation_changed",
                    "checkpoint alert generation changed; inspect the current status");
            }
            if (monitor.AcknowledgedAlertGeneration ==
                request.AlertGeneration)
            {
                if (!string.Equals(
                        monitor.AcknowledgedBy,
                        requestedBy,
                        StringComparison.Ordinal))
                {
                    throw new OrchestraDeleteCheckpointAlertException(
                        "orchestra_checkpoint_alert_acknowledgement_conflict",
                        "checkpoint alert was already acknowledged by another operator");
                }
                return new OrchestraDeleteCheckpointAlertAcknowledgeResponse(
                    true,
                    true,
                    RequireOrchestraDeleteReplayCheckpointStatus());
            }

            orchestraDeleteCheckpointMonitor = monitor with
            {
                AcknowledgedAlertGeneration =
                    request.AlertGeneration,
                AcknowledgedBy = requestedBy,
                AcknowledgedAt = effectiveAcknowledgedAt,
            };
            PersistStateStrict();
            return new OrchestraDeleteCheckpointAlertAcknowledgeResponse(
                true,
                false,
                RequireOrchestraDeleteReplayCheckpointStatus());
        }
    }

    private ulong[] RuntimeDeletionReconciliationAuditGenerations() =>
        runtimeDeletionReconciliationAudit
            .Where(static audit =>
                audit.OrchestraCleanupGeneration is not null)
            .Select(static audit =>
                audit.OrchestraCleanupGeneration!.Value)
            .Order()
            .ToArray();

    private void SynchronizeOrchestraDeleteReplayCheckpoint()
    {
        if (!orchestraRunStore.SupportsDeleteReplayHorizon)
        {
            return;
        }
        if (checkpointWorkerLease is not null &&
            !checkpointWorkerLease.IsHeld)
        {
            return;
        }
        var now = timeProvider.GetUtcNow();
        if (orchestraDeleteCheckpointMonitor is
            { ConsecutiveFailureCount: > 0, NextRetryAt: not null }
                monitor &&
            monitor.NextRetryAt > now)
        {
            return;
        }

        try
        {
            SynchronizeOrchestraDeleteReplayCheckpointCore(now);
        }
        catch (OrchestraPersistenceException) when (
            orchestraRunStore
                .DeleteReplayHorizonAvailabilityMayBeTransient &&
            !string.IsNullOrWhiteSpace(orchestraRunStore.LastError))
        {
            RecordOrchestraDeleteCheckpointFailure(now);
        }
        catch (Exception error) when (
            error is IOException or TimeoutException
                or OperationCanceledException)
        {
            RecordOrchestraDeleteCheckpointFailure(now);
        }
    }

    internal void RunOrchestraDeleteCheckpointMaintenance()
    {
        RequireControlPlaneWriter();
        lock (persistenceSync)
        {
            SynchronizeOrchestraDeleteReplayCheckpoint();
        }
    }

    internal PersistedOrchestraDeleteCheckpointAlertDelivery?
        ClaimDueOrchestraDeleteCheckpointAlertDelivery()
    {
        RequireControlPlaneWriter();
        lock (persistenceSync)
        {
            if (checkpointWorkerLease is not null &&
                !checkpointWorkerLease.IsHeld)
            {
                return null;
            }
            var now = timeProvider.GetUtcNow();
            var delivery = orchestraDeleteCheckpointAlertOutbox
                .FirstOrDefault(candidate =>
                    candidate.NextAttemptAt is null ||
                    candidate.NextAttemptAt <= now);
            if (delivery is null)
            {
                return null;
            }

            var attemptCount = delivery.AttemptCount >=
                ControlPlaneStateValidator
                    .MaxOrchestraDeleteCheckpointFailures
                ? ControlPlaneStateValidator
                    .MaxOrchestraDeleteCheckpointFailures
                : delivery.AttemptCount + 1;
            var retryDelaySeconds = Math.Min(
                1L << Math.Min(
                    checked((int)attemptCount - 1),
                    5),
                checked((long)
                    MaxAutomaticCheckpointRetryDelay.TotalSeconds));
            var claimed = delivery with
            {
                AttemptCount = attemptCount,
                LastAttemptAt = now,
                NextAttemptAt =
                    now.AddSeconds(retryDelaySeconds),
                LastDeliveryFailureCode = null,
            };
            ReplaceOrchestraDeleteCheckpointAlertDelivery(claimed);
            PersistStateStrict();
            return claimed;
        }
    }

    internal void CompleteOrchestraDeleteCheckpointAlertDelivery(
        string eventId)
    {
        RequireControlPlaneWriter();
        lock (persistenceSync)
        {
            if (checkpointWorkerLease is not null &&
                !checkpointWorkerLease.IsHeld)
            {
                return;
            }
            var remaining = orchestraDeleteCheckpointAlertOutbox
                .Where(delivery => !string.Equals(
                    delivery.EventId,
                    eventId,
                    StringComparison.Ordinal))
                .ToArray();
            if (remaining.Length ==
                orchestraDeleteCheckpointAlertOutbox.Count())
            {
                return;
            }
            orchestraDeleteCheckpointAlertOutbox =
                remaining.Aggregate(
                    ImmutableQueue<
                        PersistedOrchestraDeleteCheckpointAlertDelivery>
                        .Empty,
                    static (queue, delivery) =>
                        queue.Enqueue(delivery));
            PersistStateStrict();
        }
    }

    internal void RecordOrchestraDeleteCheckpointAlertDeliveryFailure(
        string eventId)
    {
        RequireControlPlaneWriter();
        lock (persistenceSync)
        {
            if (checkpointWorkerLease is not null &&
                !checkpointWorkerLease.IsHeld)
            {
                return;
            }
            var delivery = orchestraDeleteCheckpointAlertOutbox
                .FirstOrDefault(candidate => string.Equals(
                    candidate.EventId,
                    eventId,
                    StringComparison.Ordinal));
            if (delivery is null)
            {
                return;
            }
            ReplaceOrchestraDeleteCheckpointAlertDelivery(
                delivery with
                {
                    LastDeliveryFailureCode =
                        "checkpoint_alert_delivery_failed",
                });
            PersistStateStrict();
        }
    }

    internal TimeSpan GetNextOrchestraDeleteCheckpointMaintenanceDelay(
        TimeSpan idleDelay,
        TimeSpan readyDelay)
    {
        lock (persistenceSync)
        {
            var now = timeProvider.GetUtcNow();
            var candidates = orchestraDeleteCheckpointAlertOutbox
                .Select(static delivery =>
                    delivery.NextAttemptAt)
                .Append(
                    orchestraDeleteCheckpointMonitor is
                    { ConsecutiveFailureCount: > 0 } monitor
                        ? monitor.NextRetryAt
                        : null)
                .ToArray();
            if (orchestraDeleteCheckpointAlertOutbox.Any(
                    static delivery =>
                        delivery.NextAttemptAt is null))
            {
                return readyDelay;
            }

            var nextDueAt = candidates
                .Where(static dueAt => dueAt is not null)
                .Min();
            if (nextDueAt is null)
            {
                return idleDelay;
            }
            var untilDue = nextDueAt.Value - now;
            if (untilDue <= readyDelay)
            {
                return readyDelay;
            }
            return untilDue < idleDelay
                ? untilDue
                : idleDelay;
        }
    }

    private void ReplaceOrchestraDeleteCheckpointAlertDelivery(
        PersistedOrchestraDeleteCheckpointAlertDelivery replacement)
    {
        orchestraDeleteCheckpointAlertOutbox =
            orchestraDeleteCheckpointAlertOutbox
                .Select(delivery => string.Equals(
                    delivery.EventId,
                    replacement.EventId,
                    StringComparison.Ordinal)
                    ? replacement
                    : delivery)
                .Aggregate(
                    ImmutableQueue<
                        PersistedOrchestraDeleteCheckpointAlertDelivery>
                        .Empty,
                    static (queue, delivery) =>
                        queue.Enqueue(delivery));
    }

    private void SynchronizeOrchestraDeleteReplayCheckpointCore(
        DateTimeOffset now)
    {
        var before = orchestraRunStore.GetDeleteReplayHorizon();
        if (before is null)
        {
            if (orchestraRunStore
                .DeleteReplayHorizonAvailabilityMayBeTransient)
            {
                RecordOrchestraDeleteCheckpointFailure(now);
                return;
            }
            throw new OrchestraPersistenceException(
                "Orchestra cleanup replay horizon is unavailable");
        }
        var generations =
            RuntimeDeletionReconciliationAuditGenerations();
        var checkpointed = before;
        if (generations.Length > 0)
        {
            var minimum = generations[0];
            var observedThrough = generations[^1];
            var alreadyCheckpointed =
                before.ProtectedFromGeneration == minimum &&
                before.OldestGeneration == minimum &&
                before.NewestGeneration >= observedThrough &&
                before.CheckpointedThroughGeneration >=
                    observedThrough &&
                before.EvictedThroughGeneration < minimum;
            checkpointed = alreadyCheckpointed
                ? before
                : orchestraRunStore.CheckpointDeleteReplayHorizon(
                    new OrchestraDeleteReplayCheckpoint(
                        minimum,
                        observedThrough));
            if (checkpointed is null)
            {
                if (orchestraRunStore
                    .DeleteReplayHorizonAvailabilityMayBeTransient)
                {
                    RecordOrchestraDeleteCheckpointFailure(now);
                    return;
                }
                throw new OrchestraPersistenceException(
                    "Orchestra cleanup audit is outside the durable replay horizon");
            }
            if (checkpointed.ProtectedFromGeneration != minimum ||
                checkpointed.OldestGeneration != minimum ||
                checkpointed.NewestGeneration < observedThrough ||
                checkpointed.CheckpointedThroughGeneration is null ||
                checkpointed.CheckpointedThroughGeneration.Value <
                    observedThrough ||
                checkpointed.EvictedThroughGeneration >= minimum)
            {
                throw new OrchestraPersistenceException(
                    "Orchestra cleanup audit is outside the durable replay horizon");
            }
            var advanced =
                before.ProtectedFromGeneration !=
                    checkpointed.ProtectedFromGeneration ||
                before.CheckpointedThroughGeneration !=
                    checkpointed.CheckpointedThroughGeneration;
            if (advanced)
            {
                lastAutomaticOrchestraDeleteCheckpointAdvanced = true;
                lastAutomaticOrchestraDeleteCheckpointAt = now;
            }
        }

        orchestraDeleteReplayAdmissionPressure =
            checkpointed.AdmissionPressureWithHysteresis(
                orchestraDeleteReplayAdmissionPressure);
        var previous = orchestraDeleteCheckpointMonitor;
        orchestraDeleteCheckpointMonitor =
            new PersistedOrchestraDeleteCheckpointMonitor(
                checkpointed,
                orchestraDeleteReplayAdmissionPressure,
                ConsecutiveFailureCount: 0,
                LastAttemptAt: now,
                NextRetryAt: null,
                LastSucceededAt: now,
                LastFailureCode: null,
                AlertGeneration: previous?.AlertGeneration ?? 0,
                AlertRaisedAt: previous?.AlertRaisedAt,
                AcknowledgedAlertGeneration:
                    previous?.AcknowledgedAlertGeneration,
                AcknowledgedBy: previous?.AcknowledgedBy,
                AcknowledgedAt: previous?.AcknowledgedAt);
        if (previous is not null ||
            checkpointed.Retained > 0 ||
            generations.Length > 0 ||
            orchestraDeleteReplayAdmissionPressure !=
                OrchestraDeleteReplayAdmissionPressure.Healthy)
        {
            PersistStateStrict();
        }
    }

    private void RecordOrchestraDeleteCheckpointFailure(
        DateTimeOffset attemptedAt)
    {
        var previous = orchestraDeleteCheckpointMonitor;
        var previousFailureCount =
            previous?.ConsecutiveFailureCount ?? 0;
        var failureCount = Math.Min(
            previousFailureCount + 1,
            ControlPlaneStateValidator
                .MaxOrchestraDeleteCheckpointFailures);
        var retryDelaySeconds = Math.Min(
            1L << Math.Min(
                checked((int)failureCount - 1),
                5),
            checked((long)MaxAutomaticCheckpointRetryDelay.TotalSeconds));
        var newIncident = previousFailureCount == 0;
        var alertGeneration = newIncident
            ? checked((previous?.AlertGeneration ?? 0) + 1)
            : previous!.AlertGeneration;
        if (newIncident &&
            orchestraDeleteCheckpointAlertOutbox.Count() >=
                ControlPlaneStateValidator
                    .MaxOrchestraDeleteCheckpointAlertOutboxEntries)
        {
            throw new OrchestraPersistenceException(
                "checkpoint alert delivery outbox is full");
        }
        orchestraDeleteCheckpointMonitor =
            new PersistedOrchestraDeleteCheckpointMonitor(
                previous?.LastKnownHorizon,
                previous?.AdmissionPressure ??
                    orchestraDeleteReplayAdmissionPressure,
                failureCount,
                attemptedAt,
                attemptedAt.AddSeconds(retryDelaySeconds),
                previous?.LastSucceededAt,
                "orchestra_checkpoint_unavailable",
                alertGeneration,
                newIncident
                    ? attemptedAt
                    : previous!.AlertRaisedAt,
                newIncident
                    ? null
                    : previous!.AcknowledgedAlertGeneration,
                newIncident ? null : previous!.AcknowledgedBy,
                newIncident ? null : previous!.AcknowledgedAt);
        if (newIncident)
        {
            orchestraDeleteCheckpointAlertOutbox =
                orchestraDeleteCheckpointAlertOutbox.Enqueue(
                    new PersistedOrchestraDeleteCheckpointAlertDelivery(
                        $"orchestra-checkpoint-alert-{alertGeneration}",
                        alertGeneration,
                        attemptedAt,
                        orchestraDeleteCheckpointMonitor
                            .AdmissionPressure,
                        failureCount,
                        "orchestra_checkpoint_unavailable",
                        attemptedAt,
                        AttemptCount: 0,
                        LastAttemptAt: null,
                        NextAttemptAt: null,
                        LastDeliveryFailureCode: null));
        }
        PersistStateStrict();
    }

    private OrchestraDeleteReplayCheckpointStatus?
        BuildOrchestraDeleteReplayCheckpointStatus()
    {
        var monitor = orchestraDeleteCheckpointMonitor;
        if (monitor?.LastKnownHorizon is not { } horizon)
        {
            return null;
        }
        var generations =
            RuntimeDeletionReconciliationAuditGenerations();
        var alertActive = monitor.ConsecutiveFailureCount > 0;
        var alertAcknowledged =
            alertActive &&
            monitor.AcknowledgedAlertGeneration ==
                monitor.AlertGeneration;
        return new OrchestraDeleteReplayCheckpointStatus(
            horizon,
            generations.FirstOrDefault() is var minimum &&
                minimum > 0
                ? minimum
                : null,
            generations.LastOrDefault() is var observed &&
                observed > 0
                ? observed
                : null,
            monitor.AdmissionPressure,
            lastAutomaticOrchestraDeleteCheckpointAdvanced,
            lastAutomaticOrchestraDeleteCheckpointAt,
            ObservationStale: alertActive,
            monitor.ConsecutiveFailureCount,
            monitor.LastAttemptAt,
            monitor.NextRetryAt,
            monitor.LastSucceededAt,
            monitor.LastFailureCode,
            AlertActive: alertActive,
            monitor.AlertGeneration,
            monitor.AlertRaisedAt,
            AlertAcknowledged: alertAcknowledged,
            AcknowledgedBy:
                alertAcknowledged ? monitor.AcknowledgedBy : null,
            AcknowledgedAt:
                alertAcknowledged ? monitor.AcknowledgedAt : null);
    }

    private OrchestraDeleteReplayCheckpointStatus
        RequireOrchestraDeleteReplayCheckpointStatus() =>
        BuildOrchestraDeleteReplayCheckpointStatus() ??
        throw new OrchestraDeleteCheckpointAlertException(
            "orchestra_checkpoint_horizon_unavailable",
            "checkpoint replay horizon has not been observed");

    public void CompleteRuntimeDeletion(RuntimeDeletionReservation reservation)
    {
        RequireControlPlaneWriter();
        lock (orchestraRunSync)
        {
            PersistedRuntimeDeletionIntent intent;
            if (!pendingRuntimeDeletions.TryGetValue(reservation.IntentId, out intent!) ||
                !activeRuntimeDeletionClaims.TryGetValue(
                    reservation.IntentId,
                    out var activeClaimId) ||
                !string.Equals(activeClaimId, reservation.ClaimId, StringComparison.Ordinal) ||
                !string.Equals(
                    reservation.UnregistrationCommandId,
                    intent.UnregistrationCommandId,
                    StringComparison.Ordinal) ||
                reservation.UnregistrationReplayHorizonFloor !=
                    intent.UnregistrationReplayHorizonFloor ||
                reservation.UnregistrationMutationMayHaveStarted !=
                    intent.UnregistrationMutationMayHaveStarted ||
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

    public FleetSummary GetFleetSummary(RuntimeListFilter? filter = null) =>
        ProjectFleetSummary(ListRuntimes(filter));

    internal static FleetSummary ProjectFleetSummary(IReadOnlyList<RuntimeSummary> values)
    {
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
            values.Count,
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
        ProjectRuntimesNeedingAttention(ListRuntimes(filter));

    internal IReadOnlyList<RuntimeAttentionItem> ProjectRuntimesNeedingAttention(
        IReadOnlyList<RuntimeSummary> runtimes) =>
        runtimes
            .Select(runtime => GetRuntimeAttention(runtime.RuntimeId, runtime))
            .OfType<RuntimeAttentionView>()
            .Where(item => item.NeedsAttention)
            .Select(item => new RuntimeAttentionItem(
                item.RuntimeId,
                item.Name,
                item.Endpoint,
                item.Tags,
                item.Status,
                item.Severity,
                item.Reasons,
                item.SuggestedActions,
                item.RecentRecoveryActivities))
            .OrderByDescending(item => AttentionSeverityRank(item.Severity))
            .ThenBy(item => item.Name, StringComparer.OrdinalIgnoreCase)
            .ToArray();

    public FleetAttentionSummary GetFleetAttentionSummary(RuntimeListFilter? filter = null) =>
        ProjectFleetAttentionSummary(GetRuntimesNeedingAttention(filter));

    internal static FleetAttentionSummary ProjectFleetAttentionSummary(
        IReadOnlyList<RuntimeAttentionItem> items)
    {
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
        RequireControlPlaneWriter();
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
                .ToArray(),
            runtimeDeletionRetryAudit.ToArray(),
            runtimeDeletionReconciliationAudit.ToArray(),
            orchestraDeleteCheckpointMonitor,
            orchestraDeleteCheckpointAlertOutbox.ToArray());

    public PersistenceImportResponse ImportState(PersistedControlPlaneState state)
    {
        RequireControlPlaneWriter();
        if (!stateStore.IsCompatible(state))
        {
            throw new InvalidOperationException(
                $"imported state schema {state.SchemaVersion} is not compatible with schema {stateStore.SchemaVersion}");
        }
        ControlPlaneStateValidator.Validate(state);
        if ((state.PendingRuntimeDeletions?.Count ?? 0) > 0)
        {
            throw new InvalidOperationException(
                "pending runtime deletion intents cannot be imported");
        }
        var importedRetryAudit = NormalizeRuntimeDeletionRetryAudit(state);
        var importedReconciliationAudit =
            NormalizeRuntimeDeletionReconciliationAudit(state);
        var importedCheckpointMonitor =
            NormalizeOrchestraDeleteCheckpointMonitor(state);
        var importedCheckpointAlertOutbox =
            NormalizeOrchestraDeleteCheckpointAlertOutbox(state);

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
            runtimeDeletionRetryAudit = importedRetryAudit;
            runtimeDeletionReconciliationAudit =
                importedReconciliationAudit;
            orchestraDeleteCheckpointMonitor =
                importedCheckpointMonitor;
            orchestraDeleteCheckpointAlertOutbox =
                importedCheckpointAlertOutbox;
            var (runtimeCount, sessionCount) = RestorePersistedState(state);
            if (!orchestraRunStore.ReplaceAll(orchestraRuns.Values.SelectMany(static queue => queue).ToArray()))
            {
                runtimes.Clear();
                sessions.Clear();
                orchestraRuns.Clear();
                RestorePersistedState(previousState);
                runtimeDeletionRetryAudit =
                    NormalizeRuntimeDeletionRetryAudit(previousState);
                runtimeDeletionReconciliationAudit =
                    NormalizeRuntimeDeletionReconciliationAudit(
                        previousState);
                orchestraDeleteCheckpointMonitor =
                    NormalizeOrchestraDeleteCheckpointMonitor(
                        previousState);
                orchestraDeleteCheckpointAlertOutbox =
                    NormalizeOrchestraDeleteCheckpointAlertOutbox(
                        previousState);
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
        RequireControlPlaneWriter();
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
        RequireControlPlaneWriter();
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
        RequireControlPlaneWriter();
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
        RequireControlPlaneWriter();
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
        RequireControlPlaneWriter();
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
        RequireControlPlaneWriter();
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
        RequireControlPlaneWriter();
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
        RequireControlPlaneWriter();
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

    private void RequireControlPlaneWriter() =>
        controlPlaneWriterFence?.RequireWriter();
}
