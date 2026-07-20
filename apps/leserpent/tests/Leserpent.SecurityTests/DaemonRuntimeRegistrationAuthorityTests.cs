using System.Diagnostics;
using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class DaemonRuntimeRegistrationAuthorityTests
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
    public async Task ConfiguredAuthoritySubmitsTypedCreateOverARealSocket()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        var runtimeId = "runtime-1a2b3c4d";
        var commandId = BuildCommandId(runtimeId, "Runtime A", "https://runtime.example");
        var server = ServeAsync(
            listener,
            requests,
            CommandResponse(runtimeId, commandId));

        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));
        var registeredId = await authority.RegisterAsync(
            new RuntimeRegistrationRequest(
                "Runtime A",
                "https://runtime.example",
                "pairing-token",
                Tags: new RuntimeTags("prod", "eu", "edge")),
            runtimeId,
            CancellationToken.None);

        await server;
        Assert.Equal(runtimeId, registeredId);
        Assert.Single(requests);
        var frame = requests[0];
        Assert.Equal(Token, frame.GetProperty("token").GetString());
        var request = frame.GetProperty("request").GetProperty("request");
        Assert.Equal("command", request.GetProperty("kind").GetString());
        var payload = request.GetProperty("payload");
        Assert.Equal(1, payload.GetProperty("schema_version").GetInt32());
        Assert.Equal("runtime_register", payload.GetProperty("command").GetProperty("kind").GetString());
        Assert.Equal(commandId, payload.GetProperty("command_id").GetString());
        Assert.Equal(commandId, payload.GetProperty("idempotency_key").GetString());
        Assert.Equal("confirmed", payload.GetProperty("confirmation").GetString());
        Assert.Equal(runtimeId, payload.GetProperty("command").GetProperty("runtime_id").GetString());
        Assert.Equal("prod", payload.GetProperty("command").GetProperty("tags").GetProperty("environment").GetString());
        Assert.Equal("eu", payload.GetProperty("command").GetProperty("tags").GetProperty("cluster").GetString());
        Assert.Equal("edge", payload.GetProperty("command").GetProperty("tags").GetProperty("role").GetString());

        TryDelete(socketPath);
    }

    [Fact]
    public async Task ConfiguredAuthorityReconcilesUpdateAndTypedDiscoveryIntake()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        const string runtimeId = "runtime-existing";
        var server = ServeSequenceAsync(listener, requests, (request, index) =>
        {
            var protocol = request.GetProperty("request").GetProperty("request");
            if (protocol.GetProperty("kind").GetString() == "query")
            {
                return QueryResponse(runtimeId, index == 0 ? 4 : 5);
            }
            var commandId = protocol.GetProperty("payload").GetProperty("command_id").GetString()!;
            return CommandResponse(runtimeId, commandId, index == 1 ? 5 : 6);
        }, 4);

        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));
        var discovery = AuthorityDiscovery();
        var registeredId = await authority.RegisterAsync(
            new RuntimeRegistrationRequest(
                "Runtime A",
                "https://runtime.example",
                "pairing-token",
                Tags: new RuntimeTags("prod", "eu", "edge")),
            runtimeId,
            CancellationToken.None,
            update: true,
            capabilityDiscovery: discovery);

        await server;
        Assert.Equal(runtimeId, registeredId);
        Assert.Equal(4, requests.Count);
        Assert.Equal("runtime_inspect", requests[0].GetProperty("request").GetProperty("request").GetProperty("payload").GetProperty("query").GetProperty("kind").GetString());
        var update = requests[1].GetProperty("request").GetProperty("request").GetProperty("payload");
        Assert.Equal(4, update.GetProperty("expected_revision").GetInt64());
        Assert.Equal("runtime_registration_update", update.GetProperty("command").GetProperty("kind").GetString());
        var intake = requests[3].GetProperty("request").GetProperty("request").GetProperty("payload");
        Assert.Equal(5, intake.GetProperty("expected_revision").GetInt64());
        Assert.Equal("runtime_discovery_intake", intake.GetProperty("command").GetProperty("kind").GetString());
        Assert.Equal("1.2.0", intake.GetProperty("command").GetProperty("capabilities").GetProperty("version").GetString());
        Assert.DoesNotContain("pairing-token", requests.Select(request => request.GetRawText()));

        TryDelete(socketPath);
    }

    [Fact]
    public async Task ConfiguredRustDaemonOwnsRegistrationDiscoveryAndUpdateEndToEnd()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        using var daemon = StartDaemon(daemonBinary, databasePath, socketPath);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            var discovery = AuthorityDiscovery();
            const string runtimeId = "runtime-real";
            _ = await authority.RegisterAsync(
                new RuntimeRegistrationRequest(
                    "Runtime Real",
                    "https://runtime.example",
                    "pairing-token"),
                runtimeId,
                CancellationToken.None,
                capabilityDiscovery: discovery);
            _ = await authority.RegisterAsync(
                new RuntimeRegistrationRequest(
                    "Runtime Updated",
                    "https://runtime.example/v2",
                    "pairing-token"),
                runtimeId,
                CancellationToken.None,
                update: true,
                capabilityDiscovery: discovery);

            using var response = await InspectAsync(socketPath, runtimeId);
            var runtime = response.RootElement
                .GetProperty("response")
                .GetProperty("payload")
                .GetProperty("runtime");
            Assert.Equal("Runtime Updated", runtime.GetProperty("name").GetString());
            Assert.Equal("https://runtime.example/v2", runtime.GetProperty("endpoint").GetString());
            Assert.Equal(4, runtime.GetProperty("revision").GetInt64());
            Assert.Equal("1.2.0", runtime.GetProperty("capabilities").GetProperty("version").GetString());
            Assert.Equal(3, runtime.GetProperty("capabilities_observed_for_revision").GetInt64());

            var typedList = await authority.ListAsync(
                new RuntimeListFilter(null, null, null),
                CancellationToken.None);
            var typedInspect = await authority.InspectAsync(runtimeId, CancellationToken.None);
            Assert.Equal("Runtime Updated", Assert.Single(typedList).Name);
            Assert.Equal((ulong)4, typedInspect?.Revision);
            Assert.Equal("1.2.0", typedInspect?.Capabilities?.Version);
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
    public async Task ConfiguredAuthorityReadsStrictTypedRuntimeProjections()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        var server = ServeSequenceAsync(
            listener,
            requests,
            (_, index) => index == 0 ? RuntimeListResponse() : RuntimeInspectResponse(),
            2);
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var listed = await authority.ListAsync(
            new RuntimeListFilter(" prod ", null, "edge"),
            CancellationToken.None);
        var inspected = await authority.InspectAsync("runtime-a", CancellationToken.None);
        await server;

        var runtime = Assert.Single(listed);
        Assert.Equal("runtime-a", runtime.RuntimeId);
        Assert.Equal("Daemon Runtime", runtime.Name);
        Assert.Equal("1.2.0", runtime.Capabilities?.Version);
        Assert.Equal("gewyvern-api", runtime.Status.StatusSource);
        Assert.Equal(runtime.RuntimeId, inspected?.RuntimeId);
        Assert.Equal(runtime.Revision, inspected?.Revision);
        Assert.Equal(runtime.Status, inspected?.Status);
        Assert.Equal(runtime.Capabilities?.Version, inspected?.Capabilities?.Version);
        var filter = requests[0]
            .GetProperty("request")
            .GetProperty("request")
            .GetProperty("payload")
            .GetProperty("query")
            .GetProperty("filter");
        Assert.Equal("prod", filter.GetProperty("environment").GetString());
        Assert.Equal("edge", filter.GetProperty("role").GetString());
        Assert.DoesNotContain("token", requests[0]
            .GetProperty("request")
            .GetProperty("request")
            .GetRawText(), StringComparison.OrdinalIgnoreCase);

        TryDelete(socketPath);
    }

    [Fact]
    public async Task ConfiguredAuthorityRejectsUnknownProjectionFields()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        var response = RuntimeInspectResponse().Replace(
            "\"refresh_count\":0",
            "\"refresh_count\":0,\"pairing_token\":\"secret\"",
            StringComparison.Ordinal);
        var server = ServeSequenceAsync(listener, requests, (_, _) => response, 1);
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var error = await Assert.ThrowsAsync<DaemonRuntimeProjectionException>(() =>
            authority.InspectAsync("runtime-a", CancellationToken.None));
        await server;
        Assert.Equal("daemon_projection_invalid", error.Code);

        TryDelete(socketPath);
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
            var error = await Assert.ThrowsAsync<DaemonRuntimeRegistrationException>(() => authority.RegisterAsync(
                new RuntimeRegistrationRequest("Runtime A", "https://runtime.example", "pairing-token"),
                "runtime-1",
                CancellationToken.None));
            Assert.Equal("daemon_socket_unsafe", error.Code);
        }
        finally
        {
            TryDelete(socketPath);
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

    private static async Task ServeAsync(Socket listener, List<JsonElement> requests, string response)
    {
        using var client = await listener.AcceptAsync();
        var request = await ReadFrameAsync(client);
        using var document = JsonDocument.Parse(request);
        requests.Add(document.RootElement.Clone());
        using var stream = new NetworkStream(client, ownsSocket: false);
        var encoded = Encoding.UTF8.GetBytes(response + "\n");
        await stream.WriteAsync(encoded);
        await stream.FlushAsync();
        client.Shutdown(SocketShutdown.Send);
    }

    private static async Task ServeSequenceAsync(
        Socket listener,
        List<JsonElement> requests,
        Func<JsonElement, int, string> response,
        int count)
    {
        for (var index = 0; index < count; index++)
        {
            using var client = await listener.AcceptAsync();
            var request = await ReadFrameAsync(client);
            using var document = JsonDocument.Parse(request);
            var frame = document.RootElement.Clone();
            requests.Add(frame);
            using var stream = new NetworkStream(client, ownsSocket: false);
            var encoded = Encoding.UTF8.GetBytes(response(frame, index) + "\n");
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

    private static Process StartDaemon(string executable, string databasePath, string socketPath)
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
        start.Environment["LESERPENT_IPC_TOKEN"] = Token;
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

    private static async Task<JsonDocument> InspectAsync(string socketPath, string runtimeId)
    {
        var request = JsonSerializer.SerializeToUtf8Bytes(new
        {
            token = Token,
            request = new
            {
                schema_version = 1,
                request = new
                {
                    kind = "query",
                    payload = new
                    {
                        schema_version = 1,
                        principal = new { id = "operator" },
                        capabilities = new[] { "runtime.read" },
                        query = new { kind = "runtime_inspect", runtime_id = runtimeId },
                    },
                },
            },
        });
        using var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        await socket.ConnectAsync(new UnixDomainSocketEndPoint(socketPath));
        using var stream = new NetworkStream(socket, ownsSocket: false);
        await stream.WriteAsync(request);
        await stream.WriteAsync(new byte[] { (byte)'\n' });
        await stream.FlushAsync();
        socket.Shutdown(SocketShutdown.Send);
        return JsonDocument.Parse(await ReadFrameAsync(socket));
    }

    private static CapabilityDiscoveryResult AuthorityDiscovery() =>
        CapabilityDiscoveryResult.Succeeded(
            "https://runtime.example/v1/capabilities",
            Array.Empty<RuntimeCapability>(),
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
                new Dictionary<string, bool> { ["protocol_catalog"] = true }));

    private static string CommandResponse(string runtimeId, string commandId, int revision = 1) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"command\"," +
        "\"payload\":{\"command_id\":\"" +
        commandId +
        "\",\"status\":\"applied\"," +
        "\"runtime\":{\"id\":\"" +
        runtimeId +
        "\"},\"revision\":" + revision + "}}}";

    private static string QueryResponse(string runtimeId, int revision) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"query\"," +
        "\"payload\":{\"kind\":\"runtime_inspect\",\"revision\":" + revision + "," +
        "\"runtime\":{\"id\":\"" + runtimeId + "\",\"revision\":" + revision + "}}}}";

    private static string RuntimeListResponse() =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"query\"," +
        "\"payload\":{\"kind\":\"runtime_list\",\"revision\":2,\"runtimes\":[" +
        RuntimeProjectionJson() + "]}}}";

    private static string RuntimeInspectResponse() =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"query\"," +
        "\"payload\":{\"kind\":\"runtime_inspect\",\"revision\":2,\"runtime\":" +
        RuntimeProjectionJson() + "}}}";

    private static string RuntimeProjectionJson() =>
        "{" +
        "\"id\":\"runtime-a\",\"name\":\"Daemon Runtime\",\"endpoint\":\"https://daemon.invalid\"," +
        "\"revision\":2,\"refresh_count\":0,\"refresh_status\":\"ready\"," +
        "\"tags\":{\"environment\":\"prod\",\"cluster\":null,\"role\":\"edge\"}," +
        "\"status\":{\"status_source\":\"gewyvern-api\",\"status_fetched_at\":\"2026-07-20T12:00:00Z\",\"status_fetch_error\":null," +
        "\"has_latest_snapshot\":true,\"snapshot_kind\":\"capture\",\"target_count\":3," +
        "\"has_summary_json\":true,\"has_analysis_json\":true,\"has_training_example_json\":false," +
        "\"has_training_dataset_manifest\":false,\"has_export_json\":false,\"has_report_json\":false,\"has_report_html\":false," +
        "\"has_external_sidecar_context\":true,\"has_external_evidence_chain_enrichment\":false,\"has_external_diagnostic_opinion\":false," +
        "\"resilience_degraded\":false,\"resilience_status\":null,\"resilience_summary\":null,\"socket_service_status\":null," +
        "\"socket_consecutive_idle_timeouts\":null,\"socket_total_idle_timeouts\":null}," +
        "\"capabilities\":{\"source\":\"gewyvern-api\",\"service\":\"gewyvern-api\",\"version\":\"1.2.0\"," +
        "\"latest_snapshot\":true,\"authenticated_deployment\":true,\"serve_required\":true,\"external_sidecar_context\":true," +
        "\"target_path_segment_encoding\":\"percent-encoding\",\"target_direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"," +
        "\"endpoints\":[\"/v1/capabilities\",\"/v1/deployments\"],\"extensions\":{}}," +
        "\"capabilities_observed_for_revision\":1}";

    private static string BuildCommandId(string runtimeId, string name, string endpoint)
    {
        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes($"{runtimeId}|{name.Trim()}|{endpoint.Trim()}"));
        return Convert.ToHexString(bytes).ToLowerInvariant()[..32];
    }

    private static DaemonRuntimeRegistrationAuthority CreateAuthority(params (string Key, string Value)[] values)
    {
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(values.ToDictionary(item => item.Key, item => (string?)item.Value))
            .Build();
        return new DaemonRuntimeRegistrationAuthority(configuration);
    }

    private static string TempSocket() =>
        $"/tmp/lese-runtime-reg-{Guid.NewGuid():N}"[..32] + ".sock";

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
