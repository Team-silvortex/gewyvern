using System.Diagnostics;
using System.Security.Cryptography;
using System.Text;

namespace Leserpent.ControlPlane;

public sealed class OrchestraDeleteCheckpointWorkerLease :
    IDisposable
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
    private readonly string ownerToken = Guid.NewGuid().ToString("N");
    private bool owned;

    public OrchestraDeleteCheckpointWorkerLease(
        ControlPlaneStateStore stateStore)
    {
        LeasePath =
            $"{Path.GetFullPath(stateStore.StatePath)}.checkpoint-worker.lease";
        var digest = SHA256.HashData(
            Encoding.UTF8.GetBytes(LeasePath));
        mutexName =
            $"leserpent-checkpoint-{Convert.ToHexString(digest)}";
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
                            metadata.ProcessStartTicks ==
                                CurrentProcessStartTicks() &&
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
                        $"{Environment.ProcessId}|{CurrentProcessStartTicks()}|{ownerToken}\n");
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
                            "checkpoint worker lease must be owner-private");
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

    private static long CurrentProcessStartTicks()
    {
        using var process = Process.GetCurrentProcess();
        return process.StartTime.ToUniversalTime().Ticks;
    }

    private static bool IsOwnerAlive(LeaseMetadata metadata)
    {
        try
        {
            using var process =
                Process.GetProcessById(metadata.ProcessId);
            return process.StartTime.ToUniversalTime().Ticks ==
                metadata.ProcessStartTicks;
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

    private LeaseMetadata? ReadMetadata()
    {
        RejectSymbolicLink();
        if (!OperatingSystem.IsWindows() &&
            File.Exists(LeasePath) &&
            (File.GetUnixFileMode(LeasePath) & UnsafeMode) != 0)
        {
            throw new InvalidDataException(
                "checkpoint worker lease must be owner-private");
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
                System.Globalization.NumberStyles.None,
                System.Globalization.CultureInfo.InvariantCulture,
                out var processStartTicks) ||
            processStartTicks <= 0 ||
            fields[2].Length != 32 ||
            fields[2].Any(static value =>
                !char.IsAsciiHexDigit(value)))
        {
            return null;
        }
        return new LeaseMetadata(
            processId,
            processStartTicks,
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
                "checkpoint worker lease must not be a symbolic link");
        }
    }

    private sealed record LeaseMetadata(
        int ProcessId,
        long ProcessStartTicks,
        string OwnerToken);
}
