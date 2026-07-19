using System.Net.Sockets;
using System.Diagnostics;
using System.Text;
using System.Text.Json;
using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class DaemonDeploymentAuthorityTests
{
    private const string Token = "0123456789abcdef0123456789abcdef";

    [Fact]
    public void ConfigurationIsExplicitAndFailClosed()
    {
        Assert.False(CreateAuthority().Enabled);
        Assert.Throws<InvalidOperationException>(() =>
            CreateAuthority(("LESERPENT_DAEMON_SOCKET", "/tmp/leserpent.sock")));
        Assert.Throws<InvalidOperationException>(() =>
            CreateAuthority(("LESERPENT_DAEMON_TOKEN", Token)));
        Assert.Throws<InvalidOperationException>(() =>
            CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", "relative.sock"),
                ("LESERPENT_DAEMON_TOKEN", Token)));
    }

    [Fact]
    public void OrchestraStoreConfigurationIsExplicitAndFailClosed()
    {
        Assert.False(CreateOrchestraStore().Enabled);
        Assert.Throws<InvalidOperationException>(() =>
            CreateOrchestraStore(("LESERPENT_DAEMON_SOCKET", "/tmp/leserpent.sock")));
        Assert.Throws<InvalidOperationException>(() =>
            CreateOrchestraStore(("LESERPENT_DAEMON_TOKEN", Token)));
    }

    [Fact]
    public async Task ConfiguredAuthoritySubmitsAndReadsTheBoundReceiptOverARealSocket()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        var server = ServeAsync(
            listener,
            requests,
            CommandResponse("deploy-1"),
            CompletedReceipt("deploy-1"));
        try
        {
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token));
            var result = await authority.DeployAsync(
                RuntimeAccess(),
                DeploymentRequest(),
                CancellationToken.None);

            await server;
            Assert.Equal("gdep-1", result.DeploymentId);
            Assert.Equal("runtime-a", result.RuntimeId);
            Assert.Equal("deploy-1", result.RequestId);
            Assert.False(result.Replayed);
            Assert.Equal(2, requests.Count);
            Assert.All(requests, frame => Assert.Equal(Token, frame.GetProperty("token").GetString()));

            var command = requests[0].GetProperty("request").GetProperty("request");
            Assert.Equal("command", command.GetProperty("kind").GetString());
            var commandPayload = command.GetProperty("payload");
            Assert.Equal("deploy-1", commandPayload.GetProperty("command_id").GetString());
            Assert.Equal("deploy-1", commandPayload.GetProperty("idempotency_key").GetString());
            Assert.Equal("compatibility_adapter", commandPayload.GetProperty("origin").GetString());
            Assert.Equal("confirmed", commandPayload.GetProperty("confirmation").GetString());
            Assert.Equal("runtime.deploy", commandPayload.GetProperty("capabilities")[0].GetString());
            Assert.Equal(
                "runtime-a",
                commandPayload.GetProperty("command").GetProperty("runtime_id").GetString());

            var receipt = requests[1].GetProperty("request").GetProperty("request");
            Assert.Equal("deployment_receipt", receipt.GetProperty("kind").GetString());
            Assert.Equal("deploy-1", receipt.GetProperty("payload").GetProperty("request_id").GetString());
        }
        finally
        {
            listener.Dispose();
            TryDelete(socketPath);
        }
    }

    [Fact]
    public async Task NonPrivateSocketIsRejectedBeforeSendingTheToken()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        File.SetUnixFileMode(socketPath, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.GroupRead);
        try
        {
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token));
            var error = await Assert.ThrowsAsync<DaemonDeploymentException>(() =>
                authority.DeployAsync(RuntimeAccess(), DeploymentRequest(), CancellationToken.None));
            Assert.Equal("daemon_socket_unsafe", error.Code);
        }
        finally
        {
            TryDelete(socketPath);
        }
    }

    [Fact]
    public async Task ConfiguredRustDaemonExecutesTheDeploymentEndToEnd()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        using var gewyvern = new TcpListener(System.Net.IPAddress.Loopback, 0);
        gewyvern.Start();
        var gewyvernTask = ServeGewyvernAsync(gewyvern);
        using var daemon = StartDaemon(
            daemonBinary,
            databasePath,
            socketPath,
            ((System.Net.IPEndPoint)gewyvern.LocalEndpoint).Port);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_DEPLOY_TIMEOUT_MS", "10000"));
            var result = await authority.DeployAsync(
                RuntimeAccess(),
                DeploymentRequest(),
                CancellationToken.None);
            Assert.Equal("gdep-real", result.DeploymentId);
            Assert.Equal("deploy-1", result.RequestId);
            await gewyvernTask;
        }
        finally
        {
            if (!daemon.HasExited)
            {
                daemon.Kill(entireProcessTree: true);
                daemon.WaitForExit(2000);
            }
            TryDelete(socketPath);
            TryDelete(databasePath);
            TryDelete(databasePath + "-journal");
            TryDelete(databasePath + "-wal");
            TryDelete(databasePath + "-shm");
        }
    }

    [Fact]
    public async Task ConfiguredRustDaemonOwnsOrchestraPersistenceEndToEnd()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        using var unusedTarget = new TcpListener(System.Net.IPAddress.Loopback, 0);
        unusedTarget.Start();
        using var daemon = StartDaemon(
            daemonBinary,
            databasePath,
            socketPath,
            ((System.Net.IPEndPoint)unusedTarget.LocalEndpoint).Port);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var store = CreateOrchestraStore(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_ORCHESTRA_TIMEOUT_MS", "10000"));
            var executedAt = DateTimeOffset.Parse("2026-07-19T00:00:00Z");
            var run = new OrchestraRunSummary(
                "orun-real",
                "runtime-a",
                "plan-a",
                "queued",
                executedAt,
                Array.Empty<OrchestraExecutionStepResult>(),
                RequestId: "request-a");
            var eventRecord = new OrchestraRunEvent(
                0,
                run.RunId,
                run.RuntimeId,
                "run_queued",
                null,
                run.Outcome,
                "Orchestra run queued",
                executedAt);

            Assert.True(store.Upsert(run, eventRecord));
            Assert.Equal(run.RunId, Assert.Single(store.LoadAll()).RunId);
            var restoredEvent = Assert.Single(store.LoadEvents(run.RuntimeId, run.RunId));
            Assert.Equal(1, restoredEvent.EventId);
            Assert.Equal(eventRecord.Summary, restoredEvent.Summary);
            Assert.True(store.DeleteRuntimes([run.RuntimeId]));
            Assert.Empty(store.LoadAll());
            Assert.Null(store.LastError);
        }
        finally
        {
            if (!daemon.HasExited)
            {
                daemon.Kill(entireProcessTree: true);
                daemon.WaitForExit(2000);
            }
            TryDelete(socketPath);
            TryDelete(databasePath);
            TryDelete(databasePath + "-journal");
            TryDelete(databasePath + "-wal");
            TryDelete(databasePath + "-shm");
        }
    }

    private static Socket BindPrivateSocket(string path)
    {
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException();
        }
        var listener = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        listener.Bind(new UnixDomainSocketEndPoint(path));
        File.SetUnixFileMode(path, UnixFileMode.UserRead | UnixFileMode.UserWrite);
        listener.Listen(2);
        return listener;
    }

    private static async Task ServeAsync(
        Socket listener,
        List<JsonElement> requests,
        params string[] responses)
    {
        foreach (var response in responses)
        {
            using var client = await listener.AcceptAsync();
            var request = await ReadFrameAsync(client);
            using var document = JsonDocument.Parse(request);
            requests.Add(document.RootElement.Clone());
            var encoded = Encoding.UTF8.GetBytes(response + "\n");
            using var stream = new NetworkStream(client, ownsSocket: false);
            await stream.WriteAsync(encoded);
            await stream.FlushAsync();
            client.Shutdown(SocketShutdown.Send);
        }
    }

    private static async Task<byte[]> ReadFrameAsync(Socket socket)
    {
        using var output = new MemoryStream();
        var buffer = new byte[1024];
        while (true)
        {
            var read = await socket.ReceiveAsync(buffer, SocketFlags.None);
            Assert.True(read > 0);
            var newline = Array.IndexOf(buffer, (byte)'\n', 0, read);
            output.Write(buffer, 0, newline < 0 ? read : newline);
            if (newline >= 0)
            {
                return output.ToArray();
            }
        }
    }

    private static Process StartDaemon(
        string executable,
        string databasePath,
        string socketPath,
        int gewyvernPort)
    {
        var start = new ProcessStartInfo
        {
            FileName = executable,
            UseShellExecute = false,
            RedirectStandardError = true,
            RedirectStandardOutput = true,
        };
        start.ArgumentList.Add("--database");
        start.ArgumentList.Add(databasePath);
        start.ArgumentList.Add("--socket");
        start.ArgumentList.Add(socketPath);
        start.ArgumentList.Add("--gewyvern-target");
        start.ArgumentList.Add($"runtime-a=127.0.0.1:{gewyvernPort}");
        start.Environment["LESERPENT_IPC_TOKEN"] = Token;
        start.Environment["GEWY_API_ADMIN_TOKEN"] = "test-gewyvern-admin-token";
        return Process.Start(start) ?? throw new InvalidOperationException("failed to start leserpentd");
    }

    private static async Task WaitForSocketAsync(Process daemon, string socketPath)
    {
        for (var attempt = 0; attempt < 200; attempt++)
        {
            try
            {
                _ = File.GetAttributes(socketPath);
                return;
            }
            catch (FileNotFoundException)
            {
            }
            if (daemon.HasExited)
            {
                throw new InvalidOperationException(
                    $"leserpentd exited during startup: {await daemon.StandardError.ReadToEndAsync()}");
            }
            await Task.Delay(10);
        }
        throw new TimeoutException("leserpentd socket was not created");
    }

    private static async Task ServeGewyvernAsync(TcpListener listener)
    {
        using var client = await listener.AcceptTcpClientAsync();
        using var stream = client.GetStream();
        var buffer = new byte[8192];
        var read = await stream.ReadAsync(buffer);
        var request = Encoding.UTF8.GetString(buffer, 0, read);
        Assert.StartsWith("POST /v1/deployments HTTP/1.1", request, StringComparison.Ordinal);
        Assert.Contains("X-Gewyvern-Admin-Token: test-gewyvern-admin-token", request, StringComparison.Ordinal);
        var body = """{"deployment_id":"gdep-real","request_id":"deploy-1","pipeline_kind":"capture/http","requested_by":"operator-a","status":"accepted","accepted_unix_ms":1700000000000,"target":"service-a","replayed":false}""";
        var response = Encoding.UTF8.GetBytes(
            $"HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: {Encoding.UTF8.GetByteCount(body)}\r\nConnection: close\r\n\r\n{body}");
        await stream.WriteAsync(response);
        await stream.FlushAsync();
    }

    private static string CommandResponse(string requestId) =>
        """{"schema_version":1,"response":{"kind":"command","payload":{"command_id":"REQUEST_ID","status":"applied"}}}"""
            .Replace("REQUEST_ID", requestId, StringComparison.Ordinal);

    private static string CompletedReceipt(string requestId) =>
        """{"schema_version":1,"response":{"kind":"deployment_receipt","payload":{"command_id":"REQUEST_ID","request_id":"REQUEST_ID","status":"completed","attempt":1,"outcome":{"deployment_id":"gdep-1","request_id":"REQUEST_ID","pipeline_kind":"capture/http","requested_by":"operator-a","status":"accepted","accepted_unix_ms":1700000000000,"target":"service-a","replayed":false}}}}"""
            .Replace("REQUEST_ID", requestId, StringComparison.Ordinal);

    private static RuntimeControlAccess RuntimeAccess() =>
        new("runtime-a", "Runtime A", "https://runtime.invalid", "secret", new RuntimeTags(null, null, null));

    private static RuntimeDeploymentRequest DeploymentRequest() =>
        new("capture/http", "operator-a", true, "deploy-1", "service-a");

    private static DaemonDeploymentAuthority CreateAuthority(
        params (string Key, string Value)[] values)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(values.ToDictionary(item => item.Key, item => (string?)item.Value))
            .Build();
        return new DaemonDeploymentAuthority(configuration);
    }

    private static DaemonOrchestraRunStore CreateOrchestraStore(
        params (string Key, string Value)[] values)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(values.ToDictionary(item => item.Key, item => (string?)item.Value))
            .Build();
        return new DaemonOrchestraRunStore(
            configuration,
            NullLogger<DaemonOrchestraRunStore>.Instance);
    }

    private static string TempSocket() =>
        $"/tmp/lese-{Guid.NewGuid():N}"[..32] + ".sock";

    private static void TryDelete(string path)
    {
        try
        {
            File.Delete(path);
        }
        catch (FileNotFoundException)
        {
        }
    }
}
