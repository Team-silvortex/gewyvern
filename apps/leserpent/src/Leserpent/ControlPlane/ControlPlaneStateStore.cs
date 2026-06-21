using System.Text.Json;

namespace Leserpent.ControlPlane;

public sealed class ControlPlaneStateStore
{
    private const int CurrentSchemaVersion = 1;

    private readonly string statePath;
    private readonly string backupStatePath;
    private readonly ILogger<ControlPlaneStateStore> logger;
    private readonly JsonSerializerOptions serializerOptions = new()
    {
        WriteIndented = true,
    };

    public ControlPlaneStateStore(IConfiguration configuration, IHostEnvironment environment, ILogger<ControlPlaneStateStore> logger)
    {
        statePath = configuration["LESERPENT_STATE_PATH"]
            ?? Path.Combine(environment.ContentRootPath, "data", "control-plane-state.json");
        backupStatePath = $"{statePath}.bak";
        this.logger = logger;
    }

    public string StatePath => statePath;
    public string BackupStatePath => backupStatePath;
    public int SchemaVersion => CurrentSchemaVersion;

    public DateTimeOffset? LastSavedAt { get; private set; }
    public bool IsDirty { get; private set; }
    public string? LastSaveError { get; private set; }

    public PersistedControlPlaneState? Load()
    {
        if (!File.Exists(statePath))
        {
            IsDirty = false;
            LastSaveError = null;
            return TryLoadBackupState();
        }

        try
        {
            using var stream = File.OpenRead(statePath);
            var state = JsonSerializer.Deserialize<PersistedControlPlaneState>(stream, serializerOptions);
            if (state is null)
            {
                IsDirty = false;
                LastSaveError = null;
                logger.LogWarning("Control-plane state file at {StatePath} was empty or unreadable; starting from an empty registry.", statePath);
                return TryLoadBackupState();
            }

            if (state.SchemaVersion != CurrentSchemaVersion)
            {
                IsDirty = false;
                LastSaveError = null;
                logger.LogWarning(
                    "Control-plane state file at {StatePath} used schema {SchemaVersion}; current schema is {CurrentSchemaVersion}. Starting from an empty registry.",
                    statePath,
                    state.SchemaVersion,
                    CurrentSchemaVersion);
                return TryLoadBackupState();
            }

            LastSavedAt = state.SavedAt;
            IsDirty = false;
            LastSaveError = null;
            return state;
        }
        catch (Exception ex)
        {
            IsDirty = false;
            LastSaveError = ex.Message;
            logger.LogWarning(ex, "Failed to load control-plane state from {StatePath}; starting from an empty registry.", statePath);
            return TryLoadBackupState();
        }
    }

    public PersistedControlPlaneState CreateState(
        IReadOnlyList<PersistedRuntimeState> runtimes,
        IReadOnlyList<PersistedSessionState> sessions) =>
        new(
            CurrentSchemaVersion,
            DateTimeOffset.UtcNow,
            runtimes,
            sessions);

    public bool IsCompatible(PersistedControlPlaneState? state) =>
        state is not null && state.SchemaVersion == CurrentSchemaVersion;

    public void Save(IReadOnlyList<PersistedRuntimeState> runtimes, IReadOnlyList<PersistedSessionState> sessions)
    {
        IsDirty = true;
        var state = CreateState(runtimes, sessions);

        try
        {
            var directory = Path.GetDirectoryName(statePath);
            if (!string.IsNullOrWhiteSpace(directory))
            {
                Directory.CreateDirectory(directory);
            }

            var tempPath = $"{statePath}.tmp";
            using (var stream = File.Create(tempPath))
            {
                JsonSerializer.Serialize(stream, state, serializerOptions);
            }

            if (File.Exists(statePath))
            {
                File.Copy(statePath, backupStatePath, overwrite: true);
            }

            File.Move(tempPath, statePath, overwrite: true);
            LastSavedAt = state.SavedAt;
            IsDirty = false;
            LastSaveError = null;
        }
        catch (Exception ex)
        {
            LastSaveError = ex.Message;
            logger.LogError(ex, "Failed to persist control-plane state to {StatePath}.", statePath);
        }
    }

    private PersistedControlPlaneState? TryLoadBackupState()
    {
        if (!File.Exists(backupStatePath))
        {
            return null;
        }

        try
        {
            using var stream = File.OpenRead(backupStatePath);
            var state = JsonSerializer.Deserialize<PersistedControlPlaneState>(stream, serializerOptions);
            if (state is null || state.SchemaVersion != CurrentSchemaVersion)
            {
                logger.LogWarning(
                    "Backup control-plane state file at {BackupStatePath} was empty or used an incompatible schema; starting from an empty registry.",
                    backupStatePath);
                return null;
            }

            LastSavedAt = state.SavedAt;
            LastSaveError = null;
            logger.LogWarning("Recovered control-plane state from backup file at {BackupStatePath}.", backupStatePath);
            return state;
        }
        catch (Exception ex)
        {
            LastSaveError = ex.Message;
            logger.LogWarning(ex, "Failed to load backup control-plane state from {BackupStatePath}; starting from an empty registry.", backupStatePath);
            return null;
        }
    }
}
