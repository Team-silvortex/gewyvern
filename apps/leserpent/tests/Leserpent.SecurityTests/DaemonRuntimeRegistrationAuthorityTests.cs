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
        var request = frame.GetProperty("request");
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

    private static string CommandResponse(string runtimeId, string commandId) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"command\"," +
        "\"payload\":{\"command_id\":\"" +
        commandId +
        "\",\"status\":\"applied\"," +
        "\"runtime\":{\"id\":\"" +
        runtimeId +
        "\"},\"revision\":1}}}";

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
