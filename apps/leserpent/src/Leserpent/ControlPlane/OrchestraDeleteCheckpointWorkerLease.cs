using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;

namespace Leserpent.ControlPlane;

public abstract class ControlPlaneProcessLease : IDisposable
{
    private const UnixFileMode OwnerPrivateMode =
        UnixFileMode.UserRead | UnixFileMode.UserWrite;
    private const UnixFileMode UnsafeMode =
        UnixFileMode.UserExecute |
        UnixFileMode.GroupRead |
        UnixFileMode.GroupWrite |
        UnixFileMode.GroupExecute |
        UnixFileMode.OtherRead |
        UnixFileMode.OtherWrite |
        UnixFileMode.OtherExecute;
    private const int MaxMetadataBytes = 256;
    private readonly object sync = new();
    private readonly string mutexName;
    private readonly string leaseDescription;
    private readonly string ownerToken = Guid.NewGuid().ToString("N");
    private bool owned;

    protected ControlPlaneProcessLease(
        ControlPlaneStateStore stateStore,
        string pathSuffix,
        string mutexPrefix,
        string leaseDescription)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(pathSuffix);
        ArgumentException.ThrowIfNullOrWhiteSpace(mutexPrefix);
        ArgumentException.ThrowIfNullOrWhiteSpace(leaseDescription);
        LeasePath = $"{Path.GetFullPath(stateStore.StatePath)}{pathSuffix}";
        this.leaseDescription = leaseDescription;
        var digest = SHA256.HashData(
            Encoding.UTF8.GetBytes(LeasePath));
        mutexName = $"{mutexPrefix}-{Convert.ToHexString(digest)}";
    }

    public string LeasePath { get; }

    public bool IsHeld
    {
        get
        {
            lock (sync)
            {
                if (!owned)
                {
                    return false;
                }
                using var mutex = new Mutex(
                    initiallyOwned: false,
                    mutexName);
                if (!TryEnter(mutex))
                {
                    return false;
                }
                try
                {
                    bool valid;
                    try
                    {
                        var metadata = ReadMetadata();
                        valid = metadata is not null &&
                            metadata.ProcessId ==
                                Environment.ProcessId &&
                            metadata.ProcessStartIdentity ==
                                CurrentProcessStartIdentity() &&
                            string.Equals(
                                metadata.OwnerToken,
                                ownerToken,
                                StringComparison.Ordinal);
                    }
                    catch (Exception ex) when (
                        ex is IOException or
                            UnauthorizedAccessException or
                            InvalidDataException)
                    {
                        valid = false;
                    }
                    if (!valid)
                    {
                        owned = false;
                    }
                    return valid;
                }
                finally
                {
                    mutex.ReleaseMutex();
                }
            }
        }
    }

    public bool TryAcquire()
    {
        lock (sync)
        {
            if (owned)
            {
                return true;
            }

            using var mutex = new Mutex(
                initiallyOwned: false,
                mutexName);
            if (!TryEnter(mutex))
            {
                return false;
            }
            try
            {
                var directory = Path.GetDirectoryName(LeasePath);
                if (!string.IsNullOrWhiteSpace(directory))
                {
                    Directory.CreateDirectory(directory);
                }
                RejectSymbolicLink();
                if (File.Exists(LeasePath))
                {
                    var existing = ReadMetadata();
                    if (existing is null ||
                        IsOwnerAlive(existing))
                    {
                        return false;
                    }
                    File.Delete(LeasePath);
                }

                try
                {
                    using var stream = new FileStream(
                        LeasePath,
                        FileMode.CreateNew,
                        FileAccess.Write,
                        FileShare.Read);
                    var metadata = Encoding.ASCII.GetBytes(
                        $"{Environment.ProcessId}|{CurrentProcessStartIdentity()}|{ownerToken}\n");
                    stream.Write(metadata);
                    stream.Flush(flushToDisk: true);
                }
                catch (IOException)
                {
                    return false;
                }
                if (!OperatingSystem.IsWindows())
                {
                    File.SetUnixFileMode(
                        LeasePath,
                        OwnerPrivateMode);
                    var mode = File.GetUnixFileMode(LeasePath);
                    if ((mode & UnsafeMode) != 0)
                    {
                        throw new InvalidDataException(
                            $"{leaseDescription} must be owner-private");
                    }
                }
                owned = true;
                return true;
            }
            finally
            {
                mutex.ReleaseMutex();
            }
        }
    }

    public void Dispose()
    {
        lock (sync)
        {
            if (!owned)
            {
                return;
            }

            using var mutex = new Mutex(
                initiallyOwned: false,
                mutexName);
            if (!TryEnter(mutex))
            {
                return;
            }
            try
            {
                try
                {
                    var metadata = ReadMetadata();
                    if (metadata is not null &&
                        string.Equals(
                            metadata.OwnerToken,
                            ownerToken,
                            StringComparison.Ordinal))
                    {
                        File.Delete(LeasePath);
                    }
                }
                catch (Exception ex) when (
                    ex is IOException or
                        UnauthorizedAccessException or
                        InvalidDataException)
                {
                    // A replaced owner record must never be removed here.
                }
                owned = false;
            }
            finally
            {
                mutex.ReleaseMutex();
            }
        }
    }

    private static bool TryEnter(Mutex mutex)
    {
        try
        {
            return mutex.WaitOne(TimeSpan.FromSeconds(1));
        }
        catch (AbandonedMutexException)
        {
            return true;
        }
    }

    private static long CurrentProcessStartIdentity()
    {
        var linuxStartTicks =
            ReadLinuxProcessStartTicks(Environment.ProcessId);
        if (linuxStartTicks is not null)
        {
            return -linuxStartTicks.Value;
        }
        using var process = Process.GetCurrentProcess();
        return process.StartTime.ToUniversalTime().Ticks;
    }

    private static bool IsOwnerAlive(LeaseMetadata metadata)
    {
        try
        {
            using var process =
                Process.GetProcessById(metadata.ProcessId);
            if (OperatingSystem.IsLinux() &&
                metadata.ProcessStartIdentity < 0)
            {
                var linuxStartTicks =
                    ReadLinuxProcessStartTicks(
                        metadata.ProcessId);
                return linuxStartTicks is null ||
                    -linuxStartTicks.Value ==
                        metadata.ProcessStartIdentity;
            }
            var observed =
                process.StartTime.ToUniversalTime().Ticks;
            if (OperatingSystem.IsLinux())
            {
                return Math.Abs(
                        observed -
                        metadata.ProcessStartIdentity) <=
                    TimeSpan.TicksPerSecond * 2;
            }
            return observed ==
                metadata.ProcessStartIdentity;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (System.ComponentModel.Win32Exception)
        {
            return true;
        }
        catch (UnauthorizedAccessException)
        {
            return true;
        }
    }

    private static long? ReadLinuxProcessStartTicks(
        int processId)
    {
        if (!OperatingSystem.IsLinux())
        {
            return null;
        }
        try
        {
            var stat = File.ReadAllText(
                $"/proc/{processId}/stat");
            if (stat.Length is < 4 or > 4096)
            {
                return null;
            }
            var commandEnd = stat.LastIndexOf(')');
            if (commandEnd < 1 ||
                commandEnd + 2 >= stat.Length)
            {
                return null;
            }
            var fields = stat[(commandEnd + 2)..]
                .Split(
                    ' ',
                    StringSplitOptions.RemoveEmptyEntries);
            return fields.Length > 19 &&
                long.TryParse(
                    fields[19],
                    System.Globalization.NumberStyles.None,
                    System.Globalization.CultureInfo.InvariantCulture,
                    out var startTicks) &&
                startTicks > 0
                    ? startTicks
                    : null;
        }
        catch (IOException)
        {
            return null;
        }
        catch (UnauthorizedAccessException)
        {
            return null;
        }
    }

    private LeaseMetadata? ReadMetadata()
    {
        RejectSymbolicLink();
        if (!OperatingSystem.IsWindows() &&
            File.Exists(LeasePath) &&
            (File.GetUnixFileMode(LeasePath) & UnsafeMode) != 0)
        {
            throw new InvalidDataException(
                $"{leaseDescription} must be owner-private");
        }
        byte[] bytes;
        try
        {
            using var stream = new FileStream(
                LeasePath,
                FileMode.Open,
                FileAccess.Read,
                FileShare.ReadWrite | FileShare.Delete);
            if (stream.Length is < 1 or > MaxMetadataBytes)
            {
                return null;
            }
            bytes = new byte[checked((int)stream.Length)];
            stream.ReadExactly(bytes);
        }
        catch (FileNotFoundException)
        {
            return null;
        }
        var fields = Encoding.ASCII
            .GetString(bytes)
            .TrimEnd('\r', '\n')
            .Split('|');
        if (fields.Length != 3 ||
            !int.TryParse(
                fields[0],
                System.Globalization.NumberStyles.None,
                System.Globalization.CultureInfo.InvariantCulture,
                out var processId) ||
            processId <= 0 ||
            !long.TryParse(
                fields[1],
                System.Globalization.NumberStyles.AllowLeadingSign,
                System.Globalization.CultureInfo.InvariantCulture,
            out var processStartIdentity) ||
            processStartIdentity == 0 ||
            !OperatingSystem.IsLinux() &&
            processStartIdentity < 0 ||
            fields[2].Length != 32 ||
            fields[2].Any(static value =>
                !char.IsAsciiHexDigit(value)))
        {
            return null;
        }
        return new LeaseMetadata(
            processId,
            processStartIdentity,
            fields[2]);
    }

    private void RejectSymbolicLink()
    {
        var file = new FileInfo(LeasePath);
        if (file.LinkTarget is not null ||
            file.Exists &&
            (file.Attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new InvalidDataException(
                $"{leaseDescription} must not be a symbolic link");
        }
    }

    private sealed record LeaseMetadata(
        int ProcessId,
        long ProcessStartIdentity,
        string OwnerToken);
}

public sealed class OrchestraDeleteCheckpointWorkerLease :
    ControlPlaneProcessLease
{
    public OrchestraDeleteCheckpointWorkerLease(
        ControlPlaneStateStore stateStore)
        : base(
            stateStore,
            ".checkpoint-worker.lease",
            "leserpent-checkpoint",
            "checkpoint worker lease")
    {
    }
}

public sealed class ControlPlaneWriterLease : ControlPlaneProcessLease
{
    public ControlPlaneWriterLease(ControlPlaneStateStore stateStore)
        : base(
            stateStore,
            ".control-writer.lease",
            "leserpent-control-writer",
            "control-plane writer lease")
    {
    }
}
