using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using System.ComponentModel;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Security;
using System.Text;
using System.Runtime.InteropServices;

internal sealed class LocalOrchestraServiceSupervisor : IDisposable
{
    private static readonly UTF8Encoding Utf8WithoutBom = new(false);
    private const string DaemonExecutable = "leserpentd";
    private const string CaFile = "orchestra-local-ca.pem";
    private const string CertFile = "orchestra-local-server.pem";
    private const string KeyFile = "orchestra-local-server-key.pem";
    private const int DaemonPortStart = 9443;
    private const int DaemonPortEnd = 9503;
    private const int HealthPollMilliseconds = 120;
    private const int HealthPollAttempts = 40;
    private const int MaxStartupOutputCharacters = 4096;
    private const int MaxTlsMaterialBytes = 64 * 1024;

    private readonly object lifecycleGate = new();
    private readonly string rootDirectory;
    private readonly string databasePath;
    private readonly string caCertificatePath;
    private readonly string serverCertificatePath;
    private readonly string serverKeyPath;
    private readonly string? configuredDaemonPath;
    private readonly string remoteToken;
    private Process? process;
    private string? managedAuthorityPath;
    private int remotePort;
    private bool disposed;

    public LocalOrchestraServiceSupervisor()
        : this(DefaultRootDirectory(), null)
    {
    }

    private LocalOrchestraServiceSupervisor(string rootDirectory, string? daemonPath)
    {
        this.rootDirectory = Path.GetFullPath(rootDirectory);
        configuredDaemonPath = daemonPath is null ? null : Path.GetFullPath(daemonPath);
        EnsurePrivateRootDirectory(this.rootDirectory);

        databasePath = Path.Combine(this.rootDirectory, "orchestra.sqlite");
        caCertificatePath = Path.Combine(this.rootDirectory, CaFile);
        serverCertificatePath = Path.Combine(this.rootDirectory, CertFile);
        serverKeyPath = Path.Combine(this.rootDirectory, KeyFile);
        remoteToken = Convert.ToHexString(RandomNumberGenerator.GetBytes(32));
    }

    public bool TryEnsureReady(
        DesktopCertificateAuthorityStore certificateStore,
        out DesktopProductStartupPlan? plan,
        out string? startupFailure)
    {
        plan = null;
        startupFailure = null;
        try
        {
            lock (lifecycleGate)
            {
                ObjectDisposedException.ThrowIf(disposed, this);
                EnsureRunning(certificateStore);
                if (process is null || process.HasExited || remotePort is 0)
                {
                    throw new InvalidDataException("local orchestra process did not stay alive");
                }

                var authorityPath = certificateStore.Import(caCertificatePath);
                managedAuthorityPath = authorityPath;
                var options = CreateRemoteOptions(authorityPath, remotePort, remoteToken);
                VerifyReadiness(options, process);
                var profile = new DesktopConnectionProfile
                {
                    SchemaVersion = 1,
                    Endpoint = options.Endpoint.ToString(),
                    CertificateAuthorityPath = authorityPath,
                };
                plan = new DesktopProductStartupPlan(profile, options, RemoteTokenSource.LocalProcess);
            }
            return true;
        }
        catch (Exception error) when (StartupFailure.IsExpected(error)
            || error is TimeoutException)
        {
            startupFailure = StartupFailure.Describe(
                error,
                remoteToken,
                Environment.GetEnvironmentVariable(RemoteTokenResolver.EnvironmentVariable));
            lock (lifecycleGate)
            {
                ShutdownProcessLocked();
            }
            return false;
        }
    }

    public string? ManagedAuthorityPath
    {
        get
        {
            lock (lifecycleGate)
            {
                return managedAuthorityPath;
            }
        }
    }

    public void Dispose()
    {
        lock (lifecycleGate)
        {
            if (disposed)
            {
                return;
            }
            disposed = true;
            ShutdownProcessLocked();
        }
    }

    private static Uri CreateRemoteUri(int port) => new($"https://127.0.0.1:{port}");

    private static RemoteClientOptions CreateRemoteOptions(
        string authorityPath,
        int port,
        string token) => RemoteClientOptions.Create(
            CreateRemoteUri(port).ToString(),
            authorityPath,
            token);

    private void EnsureRunning(DesktopCertificateAuthorityStore certificateStore)
    {
        if (process is not null && !process.HasExited)
        {
            return;
        }

        if (process is not null)
        {
            ShutdownProcessLocked();
        }

        var daemon = ResolveDaemonExecutable(configuredDaemonPath);
        EnsureCertificateMaterial();
        var authorityPath = certificateStore.Import(caCertificatePath);

        var port = SelectAvailablePort();
        if (TryStartOnPort(daemon, authorityPath, port, out var startupFault, out var startedProcess))
        {
            process = startedProcess;
            remotePort = port;
            return;
        }

        process = null;
        remotePort = 0;
        throw new InvalidDataException(
            $"local orchestra service could not start on loopback port {port}: {startupFault}");
    }

    private static int SelectAvailablePort()
    {
        for (var port = DaemonPortStart; port <= DaemonPortEnd; port++)
        {
            try
            {
                using var probe = new TcpListener(IPAddress.Loopback, port);
                probe.Start();
                return port;
            }
            catch (SocketException)
            {
                // Try the next bounded product port.
            }
        }
        throw new IOException(
            $"no local orchestra port is available in {DaemonPortStart}..{DaemonPortEnd}");
    }

    private bool TryStartOnPort(
        string daemon,
        string authorityPath,
        int port,
        out string startupFault,
        out Process startedProcess)
    {
        var options = CreateRemoteOptions(authorityPath, port, remoteToken);
        startupFault = string.Empty;
        startedProcess = null!;
        var info = new ProcessStartInfo
        {
            FileName = daemon,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
        };
        info.Environment.Clear();
        info.ArgumentList.Add("--database");
        info.ArgumentList.Add(databasePath);
        info.ArgumentList.Add("--remote-listen");
        info.ArgumentList.Add($"{IPAddress.Loopback}:{port}");
        info.ArgumentList.Add("--remote-cert");
        info.ArgumentList.Add(serverCertificatePath);
        info.ArgumentList.Add("--remote-key");
        info.ArgumentList.Add(serverKeyPath);
        info.Environment["LESERPENT_REMOTE_TOKEN"] = remoteToken;
        var candidate = new Process { StartInfo = info };
        var localOutput = new StringBuilder();
        candidate.OutputDataReceived += (_, args) => AppendStartupLine(localOutput, args?.Data);
        candidate.ErrorDataReceived += (_, args) => AppendStartupLine(localOutput, args?.Data);

        try
        {
            if (!candidate.Start())
            {
                startupFault = $"local orchestra process failed to start on port {port}";
                return false;
            }
            candidate.BeginOutputReadLine();
            candidate.BeginErrorReadLine();

            VerifyReadiness(options, candidate);
            startedProcess = candidate;
            return true;
        }
        catch (Exception error) when (error is Win32Exception or InvalidOperationException or InvalidDataException or IOException
            or UnauthorizedAccessException or SecurityException
            or PlatformNotSupportedException or TimeoutException
            or OperationCanceledException or DllNotFoundException or EntryPointNotFoundException)
        {
            ShutdownProcess(candidate);
            startupFault = StartupFailure.Describe(error, remoteToken)
                + StartupDiagnostic(localOutput);
            return false;
        }
        catch (Exception error)
        {
            ShutdownProcess(candidate);
            startupFault = $"local orchestra startup failed on port {port}: {error.GetType().Name}";
            return false;
        }
    }

    private void VerifyReadiness(RemoteClientOptions options, Process candidate)
    {
        Exception? last = null;
        for (var attempt = 0; attempt < HealthPollAttempts; attempt++)
        {
            if (candidate.HasExited)
            {
                throw new InvalidOperationException("local orchestra process exited before remote readiness");
            }

            try
            {
                using var client = new RemoteHealthClient(options);
                var healthTask = client.CheckAsync();
                var health = healthTask.GetAwaiter().GetResult();
                if (health.Status == "ready" && health.AuthorityOwned)
                {
                    return;
                }
                last = new InvalidDataException("local orchestra health response was not ready");
            }
            catch (Exception error) when (StartupFailure.IsExpected(error))
            {
                last = error;
            }
            catch (Exception error)
            {
                last = error;
            }

            Thread.Sleep(HealthPollMilliseconds);
        }

        throw last is null
            ? new TimeoutException("local orchestra readiness check timed out")
            : new InvalidDataException(
                $"local orchestra readiness check failed: {last.GetType().Name}: {last.Message}",
                last);
    }

    private void EnsureCertificateMaterial()
    {
        using var certAuthority = GenerateSelfSignedCertificateAuthority();
        var serverIdentity = GenerateSelfSignedServerCertificate(certAuthority);
        using var serverCertificate = serverIdentity.Certificate;
        try
        {
            var caPem = certAuthority.ExportCertificatePem();
            var serverPem = serverCertificate.ExportCertificatePem();
            var serverKeyPem = PemEncoding.Write("PRIVATE KEY", serverIdentity.PrivateKey);

            WriteBoundedPrivateFile(caCertificatePath, $"{caPem}\n");
            WriteBoundedPrivateFile(serverCertificatePath, $"{serverPem}\n");
            try
            {
                WriteBoundedPrivateFile(serverKeyPath, serverKeyPem);
            }
            finally
            {
                Array.Clear(serverKeyPem);
            }
        }
        finally
        {
            CryptographicOperations.ZeroMemory(serverIdentity.PrivateKey);
        }
    }

    private static X509Certificate2 GenerateSelfSignedCertificateAuthority()
    {
        using var key = RSA.Create(3072);
        var request = new CertificateRequest(
            "CN=Leserpent Local Orchestra CA",
            key,
            HashAlgorithmName.SHA256,
            RSASignaturePadding.Pkcs1);
        request.CertificateExtensions.Add(
            new X509BasicConstraintsExtension(
                true,
                false,
                0,
                true));
        request.CertificateExtensions.Add(
            new X509KeyUsageExtension(
                X509KeyUsageFlags.KeyCertSign | X509KeyUsageFlags.CrlSign,
                true));
        request.CertificateExtensions.Add(
            new X509SubjectKeyIdentifierExtension(request.PublicKey, false));
        return request.CreateSelfSigned(
            DateTimeOffset.UtcNow.AddMinutes(-1),
            DateTimeOffset.UtcNow.AddDays(180));
    }

    private static (X509Certificate2 Certificate, byte[] PrivateKey) GenerateSelfSignedServerCertificate(
        X509Certificate2 authority)
    {
        using var key = RSA.Create(3072);
        var request = new CertificateRequest(
            "CN=127.0.0.1",
            key,
            HashAlgorithmName.SHA256,
            RSASignaturePadding.Pkcs1);
        request.CertificateExtensions.Add(
            new X509BasicConstraintsExtension(
                false,
                false,
                0,
                true));
        request.CertificateExtensions.Add(
            new X509KeyUsageExtension(
                X509KeyUsageFlags.DigitalSignature | X509KeyUsageFlags.KeyEncipherment,
                true));
        request.CertificateExtensions.Add(
            new X509EnhancedKeyUsageExtension(
                new OidCollection
                {
                    new Oid("1.3.6.1.5.5.7.3.1"),
                },
                false));
        var san = new SubjectAlternativeNameBuilder();
        san.AddDnsName("localhost");
        san.AddIpAddress(IPAddress.Loopback);
        san.AddIpAddress(IPAddress.IPv6Loopback);
        request.CertificateExtensions.Add(san.Build());

        var serial = RandomNumberGenerator.GetBytes(20);
        var certificate = request.Create(
            authority,
            DateTimeOffset.UtcNow.AddMinutes(-1),
            DateTimeOffset.UtcNow.AddDays(30),
            serial);
        return (certificate, key.ExportPkcs8PrivateKey());
    }

    private static string ResolveDaemonExecutable(string? injectedPath = null)
    {
        var configured = injectedPath
            ?? Environment.GetEnvironmentVariable("LESERPENT_DAEMON_PATH")
            ?? Environment.GetEnvironmentVariable("LESERPENTD_PATH");
        if (!string.IsNullOrWhiteSpace(configured))
        {
            var resolved = Path.GetFullPath(configured);
            ValidateDaemonExecutable(resolved, "configured orchestra daemon");
            return resolved;
        }

        var baseDirectory = AppContext.BaseDirectory;
        var explicitCandidates = new[]
        {
            Path.Combine(baseDirectory, DaemonExecutable),
            Path.Combine(baseDirectory, $"{DaemonExecutable}.exe"),
        };
        foreach (var candidate in explicitCandidates)
        {
            var absolute = Path.GetFullPath(candidate);
            if (File.Exists(absolute))
            {
                ValidateDaemonExecutable(absolute, "app-bundled orchestra daemon");
                return absolute;
            }
        }

        throw new FileNotFoundException(
            $"app-bundled orchestra daemon was not found as '{DaemonExecutable}'");
    }

    private static string DefaultRootDirectory()
    {
        var localData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(localData))
        {
            throw new InvalidDataException("local app data directory is unavailable");
        }
        return Path.Combine(localData, "leserpent", "desktop-orchestra-self-host");
    }

    public static void VerifyContract(string daemonPath)
    {
        var root = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-local-orchestra-{Environment.ProcessId}-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            var trust = new DesktopCertificateAuthorityStore(Path.Combine(root, "trust"));
            using var supervisor = new LocalOrchestraServiceSupervisor(
                Path.Combine(root, "state"),
                daemonPath);
            if (!supervisor.TryEnsureReady(trust, out var plan, out var failure)
                || plan is null
                || failure is not null
                || plan.TokenSource != RemoteTokenSource.LocalProcess
                || !plan.Options.Endpoint.IsLoopback
                || plan.Options.Token.Length != 64
                || !File.Exists(plan.Options.CertificateAuthorityPath))
            {
                throw new InvalidDataException(
                    $"local orchestra verification did not produce a ready plan: {failure}");
            }

            using var client = new RemoteHealthClient(plan.Options);
            var health = client.CheckAsync().GetAwaiter().GetResult();
            if (health.Status != "ready" || !health.AuthorityOwned)
            {
                throw new InvalidDataException(
                    "local orchestra verification did not reach an owned authority");
            }
            using var topologyClient = new RemoteTopologyClient(plan.Options);
            var topology = topologyClient.LoadAsync("avalonia-hub").GetAwaiter().GetResult()
                with { Health = health };
            if (topology.Revision != 0 || topology.Runtimes.Count != 0 || topology.IsStale)
            {
                throw new InvalidDataException(
                    "local orchestra topology query did not return its empty owned fleet");
            }
            if (new RemoteTopologyStateMachine().Accept(topology).Phase
                != RemoteTopologyPhase.Live)
            {
                throw new InvalidDataException(
                    "local orchestra health and topology did not compose into live authority state");
            }
            if (!OperatingSystem.IsWindows())
            {
                var privateFileMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
                var privateDirectoryMode = privateFileMode | UnixFileMode.UserExecute;
                if (File.GetUnixFileMode(Path.Combine(root, "state")) != privateDirectoryMode
                    || File.GetUnixFileMode(supervisor.caCertificatePath) != privateFileMode
                    || File.GetUnixFileMode(supervisor.serverCertificatePath) != privateFileMode
                    || File.GetUnixFileMode(supervisor.serverKeyPath) != privateFileMode)
                {
                    throw new InvalidDataException(
                        "local orchestra TLS material is not owner-private");
                }
            }

            supervisor.Dispose();
            using var restarted = new LocalOrchestraServiceSupervisor(
                Path.Combine(root, "state"),
                daemonPath);
            if (!restarted.TryEnsureReady(trust, out var restartedPlan, out var restartFailure)
                || restartedPlan is null
                || restartFailure is not null
                || restartedPlan.TokenSource != RemoteTokenSource.LocalProcess)
            {
                throw new InvalidDataException(
                    $"local orchestra did not restart after graceful cleanup: {restartFailure}");
            }
            restarted.Dispose();

            if (!OperatingSystem.IsWindows())
            {
                var linkedState = Path.Combine(root, "linked-state");
                Directory.CreateSymbolicLink(linkedState, Path.Combine(root, "state"));
                try
                {
                    _ = new LocalOrchestraServiceSupervisor(linkedState, daemonPath);
                    throw new InvalidDataException(
                        "local orchestra accepted a symbolic-link state directory");
                }
                catch (IOException error) when (error.Message.Contains(
                    "symbolic link",
                    StringComparison.Ordinal))
                {
                    // Expected fail-closed boundary.
                }
                finally
                {
                    Directory.Delete(linkedState);
                }

                var linkedDaemon = Path.Combine(root, "linked-leserpentd");
                File.CreateSymbolicLink(linkedDaemon, Path.GetFullPath(daemonPath));
                using var rejected = new LocalOrchestraServiceSupervisor(
                    Path.Combine(root, "rejected-state"),
                    linkedDaemon);
                if (rejected.TryEnsureReady(trust, out var rejectedPlan, out var rejectedFailure)
                    || rejectedPlan is not null
                    || string.IsNullOrWhiteSpace(rejectedFailure))
                {
                    throw new InvalidDataException(
                        "local orchestra accepted a symbolic-link daemon");
                }
            }
        }
        finally
        {
            Directory.Delete(root, true);
        }
    }

    private static void EnsurePrivateRootDirectory(string path)
    {
        Directory.CreateDirectory(path);
        var attributes = File.GetAttributes(path);
        if ((attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new IOException("local orchestra state directory must not be a symbolic link");
        }
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(
                path,
                UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        }
    }

    private static void ValidateDaemonExecutable(string path, string label)
    {
        var attributes = File.GetAttributes(path);
        if ((attributes & FileAttributes.ReparsePoint) != 0
            || (attributes & FileAttributes.Directory) != 0)
        {
            throw new IOException($"{label} must be a regular non-symlink file");
        }
        if (!OperatingSystem.IsWindows()
            && (File.GetUnixFileMode(path)
                & (UnixFileMode.UserExecute
                    | UnixFileMode.GroupExecute
                    | UnixFileMode.OtherExecute)) == 0)
        {
            throw new UnauthorizedAccessException($"{label} is not executable");
        }
    }

    private static void WriteBoundedPrivateFile(string path, ReadOnlySpan<char> content)
    {
        if (Utf8WithoutBom.GetByteCount(content) > MaxTlsMaterialBytes)
        {
            throw new InvalidDataException("local orchestra TLS material exceeds its byte limit");
        }
        var directory = Path.GetDirectoryName(path) ?? string.Empty;
        Directory.CreateDirectory(directory);
        var temporary = Path.Combine(directory, $".{Guid.NewGuid():N}.tmp");
        try
        {
            var options = new FileStreamOptions
            {
                Access = FileAccess.Write,
                Mode = FileMode.CreateNew,
                Share = FileShare.None,
                Options = FileOptions.WriteThrough,
            };
            if (!OperatingSystem.IsWindows())
            {
                options.UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;
            }
            using (var stream = new FileStream(temporary, options))
            using (var writer = new StreamWriter(
                stream,
                Utf8WithoutBom,
                bufferSize: 4096,
                leaveOpen: true))
            {
                writer.Write(content);
                writer.Flush();
                stream.Flush(flushToDisk: true);
            }
            File.Move(temporary, path, true);
        }
        finally
        {
            File.Delete(temporary);
        }
    }

    private void ShutdownProcess(Process runningProcess)
    {
        if (!runningProcess.HasExited)
        {
            try
            {
                var graceful = !OperatingSystem.IsWindows()
                    && SendSignal(runningProcess.Id, TerminateSignal) == 0
                    && runningProcess.WaitForExit(3000);
                if (!graceful && !runningProcess.HasExited)
                {
                    runningProcess.Kill(entireProcessTree: true);
                    runningProcess.WaitForExit(3000);
                }
            }
            catch (Exception)
            {
                // ignore best-effort shutdown failures
            }
        }
        runningProcess.Dispose();
    }

    private const int TerminateSignal = 15;

    [DllImport("libc", EntryPoint = "kill", SetLastError = true)]
    private static extern int SendSignal(int processId, int signal);

    private void ShutdownProcessLocked()
    {
        if (process is null)
        {
            return;
        }

        var alive = process;
        process = null;
        remotePort = 0;
        ShutdownProcess(alive);
    }

    private static void AppendStartupLine(StringBuilder destination, string? data)
    {
        if (string.IsNullOrWhiteSpace(data))
        {
            return;
        }
        lock (destination)
        {
            var remaining = MaxStartupOutputCharacters - destination.Length;
            if (remaining <= 0)
            {
                return;
            }
            destination.AppendLine(data[..Math.Min(data.Length, remaining)]);
        }
    }

    private string StartupDiagnostic(StringBuilder output)
    {
        string value;
        lock (output)
        {
            value = output.ToString().Trim();
        }
        if (value.Length == 0)
        {
            return string.Empty;
        }
        value = value.Replace(remoteToken, "[REDACTED]", StringComparison.Ordinal);
        return $" ({value})";
    }
}
