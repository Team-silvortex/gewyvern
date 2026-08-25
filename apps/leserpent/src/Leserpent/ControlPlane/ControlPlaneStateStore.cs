using System.Text.Json;

namespace Leserpent.ControlPlane;

public sealed class ControlPlaneStateStore
{
    private const int CurrentSchemaVersion = 9;
    private const int OldestSupportedSchemaVersion = 1;

    private readonly string statePath;
    private readonly string backupStatePath;
    private readonly object saveSync = new();
    private readonly ILogger<ControlPlaneStateStore> logger;
    private readonly JsonSerializerOptions serializerOptions = new()
    {
        WriteIndented = true,
    };
    private readonly LeserpentJsonContext jsonContext;
    private bool primaryStateKnownGood;

    public ControlPlaneStateStore(IConfiguration configuration, IHostEnvironment environment, ILogger<ControlPlaneStateStore> logger)
    {
        statePath = configuration["LESERPENT_STATE_PATH"]
            ?? DefaultStatePath(environment);
        backupStatePath = $"{statePath}.bak";
        this.logger = logger;
        jsonContext = new LeserpentJsonContext(serializerOptions);
    }

    public string StatePath => statePath;
    public string BackupStatePath => backupStatePath;
    public int SchemaVersion => CurrentSchemaVersion;

    public DateTimeOffset? LastSavedAt { get; private set; }
    public bool IsDirty { get; private set; }
    public string? LastSaveError { get; private set; }
    public ControlPlaneStateLoadProvenance LoadProvenance { get; private set; } =
        new(
            ControlPlaneStateLoadSource.None,
            ControlPlaneStateLoadOutcome.NotAttempted,
            Degraded: false,
            PrimaryFailureCode: null,
            BackupFailureCode: null);

    public PersistedControlPlaneState? Load()
    {
        if (!File.Exists(statePath))
        {
            IsDirty = false;
            LastSaveError = null;
            return TryLoadBackupState(
                ControlPlaneStateLoadFailureCode.NotFound);
        }

        try
        {
            using var stream = File.OpenRead(statePath);
            var state = JsonSerializer.Deserialize(stream, jsonContext.PersistedControlPlaneState);
            if (state is null)
            {
                IsDirty = false;
                LastSaveError = null;
                logger.LogWarning("Control-plane state file at {StatePath} was empty or unreadable; starting from an empty registry.", statePath);
                return TryLoadBackupState(
                    ControlPlaneStateLoadFailureCode.Empty);
            }

            if (!IsCompatible(state))
            {
                IsDirty = false;
                LastSaveError = null;
                logger.LogWarning(
                    "Control-plane state file at {StatePath} used schema {SchemaVersion}; current schema is {CurrentSchemaVersion}. Starting from an empty registry.",
                    statePath,
                    state.SchemaVersion,
                    CurrentSchemaVersion);
                return TryLoadBackupState(
                    ControlPlaneStateLoadFailureCode.IncompatibleSchema);
            }

            state = UpgradeState(state);
            ControlPlaneStateValidator.Validate(state);
            LastSavedAt = state.SavedAt;
            IsDirty = false;
            LastSaveError = null;
            LoadProvenance = new(
                ControlPlaneStateLoadSource.Primary,
                ControlPlaneStateLoadOutcome.Clean,
                Degraded: false,
                PrimaryFailureCode: null,
                BackupFailureCode: null);
            primaryStateKnownGood = true;
            return state;
        }
        catch (JsonException ex)
        {
            IsDirty = false;
            LastSaveError = "control_plane_state_load_failed";
            logger.LogWarning(ex, "Failed to decode control-plane state from {StatePath}; attempting backup recovery.", statePath);
            return TryLoadBackupState(
                ControlPlaneStateLoadFailureCode.InvalidJson);
        }
        catch (InvalidDataException ex)
        {
            IsDirty = false;
            LastSaveError = "control_plane_state_load_failed";
            logger.LogWarning(ex, "Control-plane state at {StatePath} failed semantic validation; attempting backup recovery.", statePath);
            return TryLoadBackupState(
                ControlPlaneStateLoadFailureCode.SemanticInvalid);
        }
        catch (Exception ex)
        {
            IsDirty = false;
            LastSaveError = "control_plane_state_load_failed";
            logger.LogWarning(ex, "Failed to load control-plane state from {StatePath}; attempting backup recovery.", statePath);
            return TryLoadBackupState(
                ControlPlaneStateLoadFailureCode.ReadFailed);
        }
    }

    public PersistedControlPlaneState CreateState(
        IReadOnlyList<PersistedRuntimeState> runtimes,
        IReadOnlyList<PersistedSessionState> sessions,
        IReadOnlyList<OrchestraRunSummary>? orchestraRuns = null,
        IReadOnlyList<PersistedRuntimeDeletionIntent>? pendingRuntimeDeletions = null,
        IReadOnlyList<PersistedRuntimeDeletionRetryAudit>? runtimeDeletionRetryAudit = null,
        IReadOnlyList<PersistedRuntimeDeletionReconciliationAudit>?
            runtimeDeletionReconciliationAudit = null,
        PersistedOrchestraDeleteCheckpointMonitor?
            orchestraDeleteCheckpointMonitor = null,
        IReadOnlyList<PersistedOrchestraDeleteCheckpointAlertDelivery>?
            orchestraDeleteCheckpointAlertOutbox = null,
        IReadOnlyList<PersistedRuntimeRegistrationIntent>?
            pendingRuntimeRegistrations = null) =>
        new(
            CurrentSchemaVersion,
            DateTimeOffset.UtcNow,
            runtimes,
            sessions,
            orchestraRuns ?? Array.Empty<OrchestraRunSummary>(),
            pendingRuntimeDeletions ?? Array.Empty<PersistedRuntimeDeletionIntent>(),
            runtimeDeletionRetryAudit ?? Array.Empty<PersistedRuntimeDeletionRetryAudit>(),
            runtimeDeletionReconciliationAudit ??
                Array.Empty<PersistedRuntimeDeletionReconciliationAudit>(),
            orchestraDeleteCheckpointMonitor,
            orchestraDeleteCheckpointAlertOutbox ??
                Array.Empty<
                    PersistedOrchestraDeleteCheckpointAlertDelivery>(),
            pendingRuntimeRegistrations ??
                Array.Empty<PersistedRuntimeRegistrationIntent>());

    public bool IsCompatible(PersistedControlPlaneState? state) =>
        state is not null &&
        state.SchemaVersion is >= OldestSupportedSchemaVersion and <= CurrentSchemaVersion;

    public void Save(
        IReadOnlyList<PersistedRuntimeState> runtimes,
        IReadOnlyList<PersistedSessionState> sessions,
        IReadOnlyList<OrchestraRunSummary>? orchestraRuns = null,
        IReadOnlyList<PersistedRuntimeDeletionIntent>? pendingRuntimeDeletions = null,
        IReadOnlyList<PersistedRuntimeDeletionRetryAudit>? runtimeDeletionRetryAudit = null,
        IReadOnlyList<PersistedRuntimeDeletionReconciliationAudit>?
            runtimeDeletionReconciliationAudit = null,
        PersistedOrchestraDeleteCheckpointMonitor?
            orchestraDeleteCheckpointMonitor = null,
        IReadOnlyList<PersistedOrchestraDeleteCheckpointAlertDelivery>?
            orchestraDeleteCheckpointAlertOutbox = null,
        IReadOnlyList<PersistedRuntimeRegistrationIntent>?
            pendingRuntimeRegistrations = null) =>
        SaveCore(
            runtimes,
            sessions,
            orchestraRuns,
            pendingRuntimeDeletions,
            runtimeDeletionRetryAudit,
            runtimeDeletionReconciliationAudit,
            orchestraDeleteCheckpointMonitor,
            orchestraDeleteCheckpointAlertOutbox,
            pendingRuntimeRegistrations,
            throwOnFailure: false);

    public void SaveStrict(
        IReadOnlyList<PersistedRuntimeState> runtimes,
        IReadOnlyList<PersistedSessionState> sessions,
        IReadOnlyList<OrchestraRunSummary>? orchestraRuns = null,
        IReadOnlyList<PersistedRuntimeDeletionIntent>? pendingRuntimeDeletions = null,
        IReadOnlyList<PersistedRuntimeDeletionRetryAudit>? runtimeDeletionRetryAudit = null,
        IReadOnlyList<PersistedRuntimeDeletionReconciliationAudit>?
            runtimeDeletionReconciliationAudit = null,
        PersistedOrchestraDeleteCheckpointMonitor?
            orchestraDeleteCheckpointMonitor = null,
        IReadOnlyList<PersistedOrchestraDeleteCheckpointAlertDelivery>?
            orchestraDeleteCheckpointAlertOutbox = null,
        IReadOnlyList<PersistedRuntimeRegistrationIntent>?
            pendingRuntimeRegistrations = null) =>
        SaveCore(
            runtimes,
            sessions,
            orchestraRuns,
            pendingRuntimeDeletions,
            runtimeDeletionRetryAudit,
            runtimeDeletionReconciliationAudit,
            orchestraDeleteCheckpointMonitor,
            orchestraDeleteCheckpointAlertOutbox,
            pendingRuntimeRegistrations,
            throwOnFailure: true);

    private void SaveCore(
        IReadOnlyList<PersistedRuntimeState> runtimes,
        IReadOnlyList<PersistedSessionState> sessions,
        IReadOnlyList<OrchestraRunSummary>? orchestraRuns,
        IReadOnlyList<PersistedRuntimeDeletionIntent>? pendingRuntimeDeletions,
        IReadOnlyList<PersistedRuntimeDeletionRetryAudit>? runtimeDeletionRetryAudit,
        IReadOnlyList<PersistedRuntimeDeletionReconciliationAudit>?
            runtimeDeletionReconciliationAudit,
        PersistedOrchestraDeleteCheckpointMonitor?
            orchestraDeleteCheckpointMonitor,
        IReadOnlyList<PersistedOrchestraDeleteCheckpointAlertDelivery>?
            orchestraDeleteCheckpointAlertOutbox,
        IReadOnlyList<PersistedRuntimeRegistrationIntent>?
            pendingRuntimeRegistrations,
        bool throwOnFailure)
    {
        lock (saveSync)
        {
            IsDirty = true;
            var state = CreateState(
                runtimes,
                sessions,
                orchestraRuns,
                pendingRuntimeDeletions,
                runtimeDeletionRetryAudit,
                runtimeDeletionReconciliationAudit,
                orchestraDeleteCheckpointMonitor,
                orchestraDeleteCheckpointAlertOutbox,
                pendingRuntimeRegistrations);
            var tempPath = $"{statePath}.{Environment.ProcessId}.{Guid.NewGuid():N}.tmp";
            var backupTempPath =
                $"{backupStatePath}.{Environment.ProcessId}.{Guid.NewGuid():N}.tmp";

            try
            {
                ControlPlaneStateValidator.Validate(state);
                var directory = Path.GetDirectoryName(statePath);
                if (!string.IsNullOrWhiteSpace(directory))
                {
                    Directory.CreateDirectory(directory);
                }

                using (var stream = new FileStream(
                    tempPath,
                    FileMode.CreateNew,
                    FileAccess.Write,
                    FileShare.None))
                {
                    JsonSerializer.Serialize(stream, state, jsonContext.PersistedControlPlaneState);
                    stream.Flush(flushToDisk: true);
                }

                if (primaryStateKnownGood && File.Exists(statePath))
                {
                    using var source = new FileStream(
                        statePath,
                        FileMode.Open,
                        FileAccess.Read,
                        FileShare.Read);
                    using (var backup = new FileStream(
                        backupTempPath,
                        FileMode.CreateNew,
                        FileAccess.Write,
                        FileShare.None))
                    {
                        source.CopyTo(backup);
                        backup.Flush(flushToDisk: true);
                    }
                    File.Move(
                        backupTempPath,
                        backupStatePath,
                        overwrite: true);
                }

                File.Move(tempPath, statePath, overwrite: true);
                primaryStateKnownGood = true;
                LastSavedAt = state.SavedAt;
                IsDirty = false;
                LastSaveError = null;
            }
            catch (Exception ex)
            {
                LastSaveError = "control_plane_state_save_failed";
                logger.LogError(ex, "Failed to persist control-plane state to {StatePath}.", statePath);
                if (throwOnFailure)
                {
                    throw new ControlPlaneStatePersistenceException(
                        "failed to persist control-plane state",
                        ex);
                }
            }
            finally
            {
                foreach (var pendingTempPath in new[]
                {
                    tempPath,
                    backupTempPath,
                })
                {
                    try
                    {
                        File.Delete(pendingTempPath);
                    }
                    catch (Exception ex)
                    {
                        logger.LogWarning(
                            ex,
                            "Failed to clean temporary control-plane state file at {TempPath}.",
                            pendingTempPath);
                    }
                }
            }
        }
    }

    private PersistedControlPlaneState? TryLoadBackupState(
        ControlPlaneStateLoadFailureCode primaryFailureCode)
    {
        primaryStateKnownGood = false;
        if (!File.Exists(backupStatePath))
        {
            LoadProvenance = primaryFailureCode ==
                ControlPlaneStateLoadFailureCode.NotFound
                ? new(
                    ControlPlaneStateLoadSource.Empty,
                    ControlPlaneStateLoadOutcome.Empty,
                    Degraded: false,
                    PrimaryFailureCode: primaryFailureCode,
                    BackupFailureCode:
                        ControlPlaneStateLoadFailureCode.NotFound)
                : FailedLoad(
                    primaryFailureCode,
                    ControlPlaneStateLoadFailureCode.NotFound);
            return null;
        }

        try
        {
            using var stream = File.OpenRead(backupStatePath);
            var state = JsonSerializer.Deserialize(stream, jsonContext.PersistedControlPlaneState);
            if (state is null || !IsCompatible(state))
            {
                var backupFailureCode = state is null
                    ? ControlPlaneStateLoadFailureCode.Empty
                    : ControlPlaneStateLoadFailureCode.IncompatibleSchema;
                LoadProvenance = FailedLoad(
                    primaryFailureCode,
                    backupFailureCode);
                logger.LogWarning(
                    "Backup control-plane state file at {BackupStatePath} was empty or used an incompatible schema; starting from an empty registry.",
                    backupStatePath);
                return null;
            }

            state = UpgradeState(state);
            ControlPlaneStateValidator.Validate(state);
            LastSavedAt = state.SavedAt;
            LastSaveError = null;
            LoadProvenance = new(
                ControlPlaneStateLoadSource.Backup,
                ControlPlaneStateLoadOutcome.Recovered,
                Degraded: true,
                PrimaryFailureCode: primaryFailureCode,
                BackupFailureCode: null);
            logger.LogWarning("Recovered control-plane state from backup file at {BackupStatePath}.", backupStatePath);
            return state;
        }
        catch (JsonException ex)
        {
            LastSaveError = "control_plane_state_backup_load_failed";
            LoadProvenance = FailedLoad(
                primaryFailureCode,
                ControlPlaneStateLoadFailureCode.InvalidJson);
            logger.LogWarning(ex, "Failed to decode backup control-plane state from {BackupStatePath}; starting from an empty registry.", backupStatePath);
            return null;
        }
        catch (InvalidDataException ex)
        {
            LastSaveError = "control_plane_state_backup_load_failed";
            LoadProvenance = FailedLoad(
                primaryFailureCode,
                ControlPlaneStateLoadFailureCode.SemanticInvalid);
            logger.LogWarning(ex, "Backup control-plane state at {BackupStatePath} failed semantic validation; starting from an empty registry.", backupStatePath);
            return null;
        }
        catch (Exception ex)
        {
            LastSaveError = "control_plane_state_backup_load_failed";
            LoadProvenance = FailedLoad(
                primaryFailureCode,
                ControlPlaneStateLoadFailureCode.ReadFailed);
            logger.LogWarning(ex, "Failed to load backup control-plane state from {BackupStatePath}; starting from an empty registry.", backupStatePath);
            return null;
        }
    }

    private static ControlPlaneStateLoadProvenance FailedLoad(
        ControlPlaneStateLoadFailureCode primaryFailureCode,
        ControlPlaneStateLoadFailureCode backupFailureCode) =>
        new(
            ControlPlaneStateLoadSource.None,
            ControlPlaneStateLoadOutcome.Failed,
            Degraded: true,
            PrimaryFailureCode: primaryFailureCode,
            BackupFailureCode: backupFailureCode);

    private static PersistedControlPlaneState UpgradeState(
        PersistedControlPlaneState state)
    {
        var backfillUnregistrationCommandId =
            state.SchemaVersion < 4;
        var markLegacyUnregistrationMutation =
            state.SchemaVersion < 5;
        return state with
        {
            SchemaVersion = CurrentSchemaVersion,
            PendingRuntimeDeletions =
                (state.PendingRuntimeDeletions ??
                    Array.Empty<PersistedRuntimeDeletionIntent>())
                .Select(intent => intent with
                {
                    Revision = Math.Max(
                        intent.Revision,
                        (long)intent.AttemptCount + 1),
                    UnregistrationCommandId =
                        backfillUnregistrationCommandId &&
                        string.IsNullOrWhiteSpace(
                            intent.UnregistrationCommandId)
                            ? RuntimeDeletionCommandIdentity.ForIntent(
                                intent.IntentId ?? string.Empty)
                            : intent.UnregistrationCommandId?.Trim() ??
                                string.Empty,
                    UnregistrationMutationMayHaveStarted =
                        markLegacyUnregistrationMutation ||
                        intent.UnregistrationMutationMayHaveStarted,
                })
                .ToArray(),
            RuntimeDeletionRetryAudit =
                state.RuntimeDeletionRetryAudit ??
                    Array.Empty<PersistedRuntimeDeletionRetryAudit>(),
            RuntimeDeletionReconciliationAudit =
                state.RuntimeDeletionReconciliationAudit ??
                    Array.Empty<
                        PersistedRuntimeDeletionReconciliationAudit>(),
            OrchestraDeleteCheckpointMonitor =
                state.SchemaVersion < 7
                    ? null
                    : state.OrchestraDeleteCheckpointMonitor,
            OrchestraDeleteCheckpointAlertOutbox =
                state.SchemaVersion < 8
                    ? Array.Empty<
                        PersistedOrchestraDeleteCheckpointAlertDelivery>()
                    : state.OrchestraDeleteCheckpointAlertOutbox ??
                        Array.Empty<
                            PersistedOrchestraDeleteCheckpointAlertDelivery>(),
            PendingRuntimeRegistrations =
                state.SchemaVersion < 9
                    ? Array.Empty<PersistedRuntimeRegistrationIntent>()
                    : state.PendingRuntimeRegistrations ??
                        Array.Empty<PersistedRuntimeRegistrationIntent>(),
        };
    }

    private static string DefaultStatePath(IHostEnvironment environment)
    {
        var localData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (!string.IsNullOrWhiteSpace(localData))
        {
            return Path.Combine(localData, "leserpent", "control-plane-state.json");
        }

        return Path.Combine(environment.ContentRootPath, ".leserpent-state", "control-plane-state.json");
    }
}

public sealed class ControlPlaneStatePersistenceException(string message, Exception innerException)
    : IOException(message, innerException);
