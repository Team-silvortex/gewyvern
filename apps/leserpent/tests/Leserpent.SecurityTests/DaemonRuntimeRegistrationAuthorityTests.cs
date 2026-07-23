using System.Diagnostics;
using System.Diagnostics.CodeAnalysis;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Leserpent.ControlPlane;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class DaemonRuntimeRegistrationAuthorityTests
{
    private const string Token = "0123456789abcdef0123456789abcdef";
    private const int RuntimeDeletionInterferenceRuntimeCount = 8;
    private static readonly string[] RuntimeDeletionCrashPhases =
    {
        "intent_persisted",
        "daemon_committed",
        "local_cleanup_persisted",
    };

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
        var commandId = BuildCommandId(
            runtimeId,
            "Runtime A",
            "https://runtime.example",
            "https://sidecar.example");
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
                Tags: new RuntimeTags("prod", "eu", "edge"),
                SidecarEndpoint: "https://sidecar.example"),
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
        Assert.Equal(
            "https://sidecar.example",
            payload.GetProperty("command").GetProperty("sidecar_endpoint").GetString());
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
        var sidecarDiscovery = RuntimeSidecarDiscoveryResult.Failed(
            "https://sidecar.example/v1/status",
            "raw upstream failure with secret-token");
        var statusDiscovery = RuntimeStatusDiscoveryResult.Failed(
            "https://runtime.example/v1/latest/status",
            "raw status failure with status-secret");
        var registeredId = await authority.RegisterAsync(
            new RuntimeRegistrationRequest(
                "Runtime A",
                "https://runtime.example",
                "pairing-token",
                Tags: new RuntimeTags("prod", "eu", "edge")),
            runtimeId,
            CancellationToken.None,
            update: true,
            capabilityDiscovery: discovery,
            statusDiscovery: statusDiscovery,
            sidecarDiscovery: sidecarDiscovery);

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
        Assert.Equal(
            "sidecar_fetch_failed",
            intake.GetProperty("command").GetProperty("sidecar_status").GetProperty("status_fetch_error").GetString());
        Assert.Equal(
            "runtime_status_fetch_failed",
            intake.GetProperty("command").GetProperty("status").GetProperty("status_fetch_error").GetString());
        Assert.DoesNotContain("pairing-token", requests.Select(request => request.GetRawText()));
        Assert.DoesNotContain("secret-token", requests.Select(request => request.GetRawText()));
        Assert.DoesNotContain("status-secret", requests.Select(request => request.GetRawText()));

        TryDelete(socketPath);
    }

    [Fact]
    public async Task ConfiguredAuthoritySubmitsRevisionFencedUnregistration()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        const string runtimeId = "runtime-delete-a";
        var server = ServeSequenceAsync(listener, requests, (request, index) =>
        {
            if (index == 0)
            {
                return QueryResponse(runtimeId, 9);
            }
            var payload = request
                .GetProperty("request")
                .GetProperty("request")
                .GetProperty("payload");
            return RuntimeUnregisteredResponse(
                payload.GetProperty("command_id").GetString()!,
                runtimeId,
                9);
        }, 2);

        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));
        await authority.UnregisterAsync(new[] { runtimeId }, CancellationToken.None);

        await server;
        Assert.Equal(2, requests.Count);
        var request = requests[1].GetProperty("request").GetProperty("request");
        Assert.Equal("runtime_unregister", request.GetProperty("kind").GetString());
        var payload = request.GetProperty("payload");
        Assert.True(payload.GetProperty("confirmed").GetBoolean());
        Assert.Equal("runtime.unregister", payload.GetProperty("capabilities")[0].GetString());
        var target = Assert.Single(payload.GetProperty("targets").EnumerateArray());
        Assert.Equal(runtimeId, target.GetProperty("runtime_id").GetString());
        Assert.Equal(9, target.GetProperty("expected_revision").GetInt64());
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
                    "pairing-token",
                    SidecarEndpoint: "https://sidecar.example/v2"),
                runtimeId,
                CancellationToken.None,
                update: true,
                capabilityDiscovery: discovery,
                sidecarDiscovery: AuthoritySidecarDiscovery());
            await authority.SubmitDiscoveryAsync(
                runtimeId,
                CancellationToken.None,
                statusDiscovery: RuntimeStatusDiscoveryResult.Failed(
                    "https://runtime.example/v1/latest/status",
                    "raw daemon refresh failure"));

            using var response = await InspectAsync(socketPath, runtimeId);
            var runtime = response.RootElement
                .GetProperty("response")
                .GetProperty("payload")
                .GetProperty("runtime");
            Assert.Equal("Runtime Updated", runtime.GetProperty("name").GetString());
            Assert.Equal("https://runtime.example/v2", runtime.GetProperty("endpoint").GetString());
            Assert.Equal(
                "https://sidecar.example/v2",
                runtime.GetProperty("sidecar_endpoint").GetString());
            Assert.True(runtime.GetProperty("registered_at_unix_ms").GetInt64() > 0);
            Assert.True(
                runtime.GetProperty("updated_at_unix_ms").GetInt64()
                    >= runtime.GetProperty("registered_at_unix_ms").GetInt64());
            Assert.Equal(5, runtime.GetProperty("revision").GetInt64());
            Assert.Equal("1.2.0", runtime.GetProperty("capabilities").GetProperty("version").GetString());
            Assert.Equal(3, runtime.GetProperty("capabilities_observed_for_revision").GetInt64());
            Assert.Equal(
                "runtime_status_fetch_failed",
                runtime.GetProperty("status").GetProperty("status_fetch_error").GetString());
            Assert.Equal(
                "etragon-api",
                runtime.GetProperty("sidecar_status").GetProperty("status_source").GetString());
            Assert.Equal(
                "slot-a",
                runtime.GetProperty("sidecar_status").GetProperty("memory").GetProperty("latest_slot").GetString());

            var typedList = await authority.ListAsync(
                new RuntimeListFilter(null, null, null),
                CancellationToken.None);
            var typedInspect = await authority.InspectAsync(runtimeId, CancellationToken.None);
            Assert.Equal("Runtime Updated", Assert.Single(typedList).Name);
            Assert.Equal((ulong)5, typedInspect?.Revision);
            Assert.Equal("https://sidecar.example/v2", typedInspect?.SidecarEndpoint);
            Assert.NotNull(typedInspect?.RegisteredAt);
            Assert.NotNull(typedInspect?.UpdatedAt);
            Assert.Equal("1.2.0", typedInspect?.Capabilities?.Version);
            Assert.Equal("runtime_status_fetch_failed", typedInspect?.Status.StatusFetchError);
            Assert.Equal("ready", typedInspect?.SidecarStatus?.DaemonStatus);
            Assert.Equal("slot-a", typedInspect?.SidecarStatus?.Memory?.LatestSlot);

            await authority.UnregisterAsync(new[] { runtimeId }, CancellationToken.None);
            Assert.Empty(await authority.ListAsync(
                new RuntimeListFilter(null, null, null),
                CancellationToken.None));
            Assert.Null(await authority.InspectAsync(runtimeId, CancellationToken.None));
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
    public async Task RuntimeDeletionRecoversAfterHostIsKilledAtDaemonCommitBoundary()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(File.Exists(harnessAssembly), $"crash harness was not built at {harnessAssembly}");
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
            await ExecuteRuntimeDeletionCrashScenarioAsync(
                harnessAssembly,
                socketPath,
                authority,
                "daemon_committed",
                0);
            WriteCrashEvidenceIfRequested();
        }
        finally
        {
            if (!daemon.HasExited)
            {
                daemon.Kill(entireProcessTree: true);
                daemon.WaitForExit(5000);
            }
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
            })
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionFaultCampaignRecoversAcrossEveryDurableTransition()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(File.Exists(harnessAssembly), $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable("LESERPENT_RUNTIME_DELETION_FAULT_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 20)
            : 3;
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
            foreach (var phase in RuntimeDeletionCrashPhases)
            {
                for (var iteration = 0; iteration < iterations; iteration += 1)
                {
                    await ExecuteRuntimeDeletionCrashScenarioAsync(
                        harnessAssembly,
                        socketPath,
                        authority,
                        phase,
                        iteration);
                }
            }
            WriteFaultCampaignEvidenceIfRequested(iterations);
        }
        finally
        {
            if (!daemon.HasExited)
            {
                daemon.Kill(entireProcessTree: true);
                daemon.WaitForExit(5000);
            }
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
            })
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionFaultCampaignPreservesConcurrentRegistrationAndStateSaves()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(File.Exists(harnessAssembly), $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable("LESERPENT_RUNTIME_DELETION_FAULT_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 20)
            : 3;
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
            foreach (var phase in RuntimeDeletionCrashPhases)
            {
                for (var iteration = 0; iteration < iterations; iteration += 1)
                {
                    await ExecuteRuntimeDeletionCrashScenarioAsync(
                        harnessAssembly,
                        socketPath,
                        authority,
                        phase,
                        iteration,
                        injectConcurrentTraffic: true);
                }
            }
            WriteConcurrentFaultCampaignEvidenceIfRequested(iterations);
        }
        finally
        {
            if (!daemon.HasExited)
            {
                daemon.Kill(entireProcessTree: true);
                daemon.WaitForExit(5000);
            }
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
            })
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionFaultCampaignRecoversAcrossDaemonRestarts()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(File.Exists(harnessAssembly), $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable("LESERPENT_RUNTIME_DELETION_FAULT_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 20)
            : 3;
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        using var daemon = new RestartableTestDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await daemon.StartAsync();
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            foreach (var phase in RuntimeDeletionCrashPhases)
            {
                for (var iteration = 0; iteration < iterations; iteration += 1)
                {
                    await ExecuteRuntimeDeletionCrashScenarioAsync(
                        harnessAssembly,
                        socketPath,
                        authority,
                        phase,
                        iteration,
                        injectConcurrentTraffic: true,
                        restartableDaemon: daemon);
                }
            }
            WriteDaemonRestartFaultCampaignEvidenceIfRequested(iterations);
        }
        finally
        {
            daemon.Stop();
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
            })
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionRecoversAfterUncleanDaemonLeaseTakeover()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(File.Exists(harnessAssembly), $"crash harness was not built at {harnessAssembly}");
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        var takeoverLatenciesMs = new List<long>();
        using var daemon = new RestartableTestDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await daemon.StartAsync();
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            foreach (var phase in RuntimeDeletionCrashPhases)
            {
                var result = await ExecuteRuntimeDeletionCrashScenarioAsync(
                    harnessAssembly,
                    socketPath,
                    authority,
                    phase,
                    0,
                    injectConcurrentTraffic: true,
                    restartableDaemon: daemon,
                    waitForExpiredOwnerLease: true);
                Assert.True(result.OwnerLeaseTakeoverMs.HasValue);
                takeoverLatenciesMs.Add(result.OwnerLeaseTakeoverMs.Value);
            }
            WriteUncleanDaemonTakeoverEvidenceIfRequested(takeoverLatenciesMs);
        }
        finally
        {
            daemon.Stop();
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
            })
            {
                TryDelete(path);
            }
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
        Assert.Equal("https://daemon-sidecar.invalid", runtime.SidecarEndpoint);
        Assert.Equal("1.2.0", runtime.Capabilities?.Version);
        Assert.Equal("gewyvern-api", runtime.Status.StatusSource);
        Assert.Equal("etragon-api", runtime.SidecarStatus?.StatusSource);
        Assert.Equal("slot-a", runtime.SidecarStatus?.Memory?.LatestSlot);
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
    public async Task ConfiguredAuthorityRejectsReversedAuthorityTimestamps()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        var response = RuntimeInspectResponse().Replace(
            "\"updated_at_unix_ms\":1784626200000",
            "\"updated_at_unix_ms\":1784620000000",
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
    public async Task ConfiguredAuthorityAcceptsLegacyProjectionWithoutAuthorityTimestamps()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        var response = RuntimeInspectResponse().Replace(
            "\"registered_at_unix_ms\":1784620800000,\"updated_at_unix_ms\":1784626200000,",
            string.Empty,
            StringComparison.Ordinal);
        var server = ServeSequenceAsync(listener, requests, (_, _) => response, 1);
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var projection = await authority.InspectAsync("runtime-a", CancellationToken.None);
        await server;
        Assert.NotNull(projection);
        Assert.Null(projection!.RegisteredAt);
        Assert.Null(projection.UpdatedAt);

        TryDelete(socketPath);
    }

    [Fact]
    public async Task ConfiguredAuthorityAcceptsLegacyProjectionWithoutSidecarStatus()
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
            (_, _) => RuntimeInspectResponse(includeSidecarStatus: false),
            1);
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var projection = await authority.InspectAsync("runtime-a", CancellationToken.None);
        await server;
        Assert.NotNull(projection);
        Assert.Null(projection!.SidecarStatus);

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

    private static async Task<RuntimeDeletionCrashScenarioResult> ExecuteRuntimeDeletionCrashScenarioAsync(
        string harnessAssembly,
        string socketPath,
        DaemonRuntimeRegistrationAuthority authority,
        string phase,
        int iteration,
        bool injectConcurrentTraffic = false,
        RestartableTestDaemon? restartableDaemon = null,
        bool waitForExpiredOwnerLease = false)
    {
        var phaseSlug = phase.Replace('_', '-');
        var runtimeId = $"runtime-crash-{phaseSlug}-{iteration}";
        var statePath = $"{socketPath}.{phase}.{iteration}.state.json";
        var markerPath = $"{socketPath}.{phase}.{iteration}.marker";
        var interferenceRuntimeIds = new List<string>();
        long? ownerLeaseTakeoverMs = null;
        Process? harness = null;
        int? harnessProcessId = null;
        RuntimeDeletionRecoveryService? recovery = null;
        try
        {
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                runtimeId,
                phase);
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            var daemonRuntime = await authority.InspectAsync(
                runtimeId,
                CancellationToken.None);
            Assert.Equal(
                string.Equals(phase, "intent_persisted", StringComparison.Ordinal),
                daemonRuntime is not null);
            Assert.False(harness.HasExited);

            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
                .Build();
            var stateStore = new ControlPlaneStateStore(
                configuration,
                new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                NullLogger<ControlPlaneStateStore>.Instance);
            var restarted = new RegistryService(
                stateStore,
                new InMemoryOrchestraRunStore());
            Assert.Single(restarted.ListPendingRuntimeDeletions());
            var localRuntimeShouldExist = !string.Equals(
                phase,
                "local_cleanup_persisted",
                StringComparison.Ordinal);
            Assert.Equal(
                localRuntimeShouldExist,
                restarted.GetRuntime(runtimeId) is not null);
            if (localRuntimeShouldExist)
            {
                Assert.Equal(
                    runtimeId,
                    restarted.CreateSession(new SessionCreateRequest(
                        runtimeId,
                        "diagnostic",
                        "crash-recovery-test",
                        Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);
            }

            if (restartableDaemon is not null)
            {
                if (waitForExpiredOwnerLease)
                {
                    restartableDaemon.Stop();
                }
                else
                {
                    restartableDaemon.StopGracefully();
                }
            }
            var restartCoordinatedAuthority = restartableDaemon is not null
                ? new RestartCoordinatedRuntimeDeletionAuthority(authority)
                : null;
            var coordinatedAuthority = injectConcurrentTraffic &&
                restartCoordinatedAuthority is null
                ? new CoordinatedRuntimeDeletionAuthority(authority)
                : null;
            IRuntimeRegistrationAuthority recoveryAuthority =
                restartCoordinatedAuthority is not null
                    ? restartCoordinatedAuthority
                    : coordinatedAuthority is not null
                        ? coordinatedAuthority
                        : authority;
            recovery = new RuntimeDeletionRecoveryService(
                restarted,
                recoveryAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            await recovery.StartAsync(CancellationToken.None);
            if (restartCoordinatedAuthority is not null)
            {
                if (waitForExpiredOwnerLease)
                {
                    var takeover = await DriveUncleanDaemonTakeoverAndConcurrentTrafficAsync(
                        restarted,
                        authority,
                        restartCoordinatedAuthority,
                        restartableDaemon!,
                        phaseSlug,
                        iteration);
                    interferenceRuntimeIds.AddRange(takeover.RuntimeIds);
                    ownerLeaseTakeoverMs = takeover.TakeoverLatencyMs;
                }
                else
                {
                    interferenceRuntimeIds.AddRange(
                        await DriveDaemonRestartAndConcurrentTrafficAsync(
                            restarted,
                            authority,
                            restartCoordinatedAuthority,
                            restartableDaemon!,
                            phaseSlug,
                            iteration));
                }
            }
            else if (coordinatedAuthority is not null)
            {
                interferenceRuntimeIds.AddRange(
                    await DriveConcurrentRegistrationAndSaveTrafficAsync(
                        restarted,
                        authority,
                        coordinatedAuthority,
                        phaseSlug,
                        iteration));
            }
            await WaitForDeletionRecoveryAsync(restarted, runtimeId);

            Assert.Empty(restarted.ListPendingRuntimeDeletions());
            Assert.Null(restarted.GetRuntime(runtimeId));
            Assert.Null(await authority.InspectAsync(runtimeId, CancellationToken.None));
            if (interferenceRuntimeIds.Count > 0)
            {
                await recovery.StopAsync(CancellationToken.None);
                recovery.Dispose();
                recovery = null;
                restarted.SaveNow();
                var diskReloaded = new RegistryService(
                    new ControlPlaneStateStore(
                        configuration,
                        new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                        NullLogger<ControlPlaneStateStore>.Instance),
                    new InMemoryOrchestraRunStore());
                foreach (var interferenceRuntimeId in interferenceRuntimeIds)
                {
                    Assert.NotNull(restarted.GetRuntime(interferenceRuntimeId));
                    Assert.NotNull(diskReloaded.GetRuntime(interferenceRuntimeId));
                    Assert.NotNull(await authority.InspectAsync(
                        interferenceRuntimeId,
                        CancellationToken.None));
                }
            }
            return new RuntimeDeletionCrashScenarioResult(ownerLeaseTakeoverMs);
        }
        finally
        {
            if (recovery is not null)
            {
                await recovery.StopAsync(CancellationToken.None);
                recovery.Dispose();
            }
            if (harness is not null)
            {
                if (!harness.HasExited)
                {
                    harness.Kill(entireProcessTree: true);
                    harness.WaitForExit(5000);
                }
                harness.Dispose();
            }
            if (interferenceRuntimeIds.Count > 0)
            {
                try
                {
                    await authority.UnregisterAsync(
                        interferenceRuntimeIds,
                        CancellationToken.None);
                }
                catch
                {
                    // The enclosing test tears down the isolated daemon database.
                }
            }
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                markerPath,
                markerPath + $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
        }
    }

    private static async Task<IReadOnlyList<string>> DriveConcurrentRegistrationAndSaveTrafficAsync(
        RegistryService registry,
        DaemonRuntimeRegistrationAuthority authority,
        CoordinatedRuntimeDeletionAuthority coordinatedAuthority,
        string phaseSlug,
        int iteration)
    {
        await coordinatedAuthority.UnregisterStarted.WaitAsync(TimeSpan.FromSeconds(5));
        var runtimeIds = Enumerable.Range(0, RuntimeDeletionInterferenceRuntimeCount)
            .Select(index => $"runtime-traffic-{phaseSlug}-{iteration}-{index}")
            .ToArray();

        await RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[..2]);
        coordinatedAuthority.AllowDaemonCommit();
        await coordinatedAuthority.DaemonCommitted.WaitAsync(TimeSpan.FromSeconds(5));

        await RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[2..4]);
        var racingRegistrations = RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[4..]);
        coordinatedAuthority.AllowLocalCleanup();
        await racingRegistrations;
        return runtimeIds;
    }

    private static async Task<IReadOnlyList<string>> DriveDaemonRestartAndConcurrentTrafficAsync(
        RegistryService registry,
        DaemonRuntimeRegistrationAuthority authority,
        RestartCoordinatedRuntimeDeletionAuthority coordinatedAuthority,
        RestartableTestDaemon restartableDaemon,
        string phaseSlug,
        int iteration)
    {
        await coordinatedAuthority.FirstAttemptFailed.WaitAsync(TimeSpan.FromSeconds(5));
        var runtimeIds = Enumerable.Range(0, RuntimeDeletionInterferenceRuntimeCount)
            .Select(index => $"runtime-restart-traffic-{phaseSlug}-{iteration}-{index}")
            .ToArray();

        RegisterLocalInterferenceBatch(registry, runtimeIds[..2]);
        await restartableDaemon.StartAsync();
        await coordinatedAuthority.RetryStarted.WaitAsync(TimeSpan.FromSeconds(5));
        await RegisterDaemonInterferenceBatchAsync(authority, runtimeIds[..2]);
        await RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[2..4]);
        coordinatedAuthority.AllowDaemonCommit();
        await coordinatedAuthority.DaemonCommitted.WaitAsync(TimeSpan.FromSeconds(5));

        await RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[4..6]);
        var racingRegistrations = RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[6..]);
        coordinatedAuthority.AllowLocalCleanup();
        await racingRegistrations;
        return runtimeIds;
    }

    private static async Task<UncleanDaemonTakeoverResult> DriveUncleanDaemonTakeoverAndConcurrentTrafficAsync(
        RegistryService registry,
        DaemonRuntimeRegistrationAuthority authority,
        RestartCoordinatedRuntimeDeletionAuthority coordinatedAuthority,
        RestartableTestDaemon restartableDaemon,
        string phaseSlug,
        int iteration)
    {
        await coordinatedAuthority.FirstAttemptFailed.WaitAsync(TimeSpan.FromSeconds(5));
        var runtimeIds = Enumerable.Range(0, RuntimeDeletionInterferenceRuntimeCount)
            .Select(index => $"runtime-unclean-traffic-{phaseSlug}-{iteration}-{index}")
            .ToArray();

        RegisterLocalInterferenceBatch(registry, runtimeIds[..2]);
        var takeoverLatencyMs = await restartableDaemon.StartAfterOwnerLeaseExpiryAsync();
        await coordinatedAuthority.RetryStarted.WaitAsync(TimeSpan.FromSeconds(5));
        await RegisterDaemonInterferenceBatchAsync(authority, runtimeIds[..2]);
        await RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[2..4]);
        coordinatedAuthority.AllowDaemonCommit();
        await coordinatedAuthority.DaemonCommitted.WaitAsync(TimeSpan.FromSeconds(5));

        await RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[4..6]);
        var racingRegistrations = RegisterInterferenceBatchAsync(
            registry,
            authority,
            runtimeIds[6..]);
        coordinatedAuthority.AllowLocalCleanup();
        await racingRegistrations;
        return new UncleanDaemonTakeoverResult(runtimeIds, takeoverLatencyMs);
    }

    private static void RegisterLocalInterferenceBatch(
        RegistryService registry,
        IReadOnlyCollection<string> runtimeIds)
    {
        foreach (var runtimeId in runtimeIds)
        {
            registry.RegisterRuntime(InterferenceRequest(runtimeId), runtimeId);
            registry.SaveNow();
        }
    }

    private static Task RegisterDaemonInterferenceBatchAsync(
        DaemonRuntimeRegistrationAuthority authority,
        IReadOnlyCollection<string> runtimeIds) =>
        Task.WhenAll(runtimeIds.Select(runtimeId => authority.RegisterAsync(
            InterferenceRequest(runtimeId),
            runtimeId,
            CancellationToken.None)));

    private static Task RegisterInterferenceBatchAsync(
        RegistryService registry,
        DaemonRuntimeRegistrationAuthority authority,
        IReadOnlyCollection<string> runtimeIds) =>
        Task.WhenAll(runtimeIds.Select(runtimeId => Task.Run(async () =>
        {
            var request = InterferenceRequest(runtimeId);
            await authority.RegisterAsync(
                request,
                runtimeId,
                CancellationToken.None);
            registry.RegisterRuntime(request, runtimeId);
            registry.SaveNow();
        })));

    private static RuntimeRegistrationRequest InterferenceRequest(string runtimeId) =>
        new(
            $"Concurrent Traffic {runtimeId}",
            $"https://{runtimeId}.example",
            "test-only-pairing-token");

    private static Process StartCrashHarness(
        string harnessAssembly,
        string statePath,
        string socketPath,
        string markerPath,
        string runtimeId,
        string phase)
    {
        var start = new ProcessStartInfo
        {
            FileName = Environment.GetEnvironmentVariable("DOTNET_HOST_PATH") ?? "dotnet",
            UseShellExecute = false,
            RedirectStandardError = true,
            RedirectStandardOutput = true,
        };
        start.ArgumentList.Add(harnessAssembly);
        start.ArgumentList.Add(statePath);
        start.ArgumentList.Add(socketPath);
        start.ArgumentList.Add(markerPath);
        start.ArgumentList.Add(runtimeId);
        start.ArgumentList.Add(phase);
        start.Environment["LESERPENT_DAEMON_TOKEN"] = Token;
        return Process.Start(start) ?? throw new InvalidOperationException(
            "failed to start runtime deletion crash harness");
    }

    private static string FindCrashHarnessAssembly()
    {
        var repositoryRoot = FindRepositoryRoot();
        var configuration = new DirectoryInfo(
            Path.TrimEndingDirectorySeparator(AppContext.BaseDirectory))
            .Parent?.Name ?? "Debug";
        return Path.Combine(
            repositoryRoot,
            "apps",
            "leserpent",
            "tests",
            "Leserpent.RuntimeDeletionCrashHarness",
            "bin",
            configuration,
            "net10.0",
            "Leserpent.RuntimeDeletionCrashHarness.dll");
    }

    private static string FindRepositoryRoot()
    {
        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            if (File.Exists(Path.Combine(directory.FullName, "Cargo.toml")))
            {
                return directory.FullName;
            }
        }
        throw new DirectoryNotFoundException("could not locate the gewyvern repository root");
    }

    private static async Task WaitForMarkerAsync(Process harness, string markerPath)
    {
        for (var attempt = 0; attempt < 500; attempt++)
        {
            if (File.Exists(markerPath))
            {
                return;
            }
            if (harness.HasExited)
            {
                throw new InvalidOperationException(
                    $"crash harness exited before the requested durable boundary: {await harness.StandardError.ReadToEndAsync()}");
            }
            await Task.Delay(10);
        }
        throw new TimeoutException("crash harness did not reach the requested durable boundary");
    }

    private static async Task WaitForDeletionRecoveryAsync(
        RegistryService registry,
        string runtimeId)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(5);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (registry.ListPendingRuntimeDeletions().Count == 0 &&
                registry.GetRuntime(runtimeId) is null)
            {
                return;
            }
            await Task.Delay(10);
        }
        throw new TimeoutException("runtime deletion intent did not converge after restart");
    }

    private static void WriteCrashEvidenceIfRequested()
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_CRASH_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            checks = new
            {
                real_leserpentd = true,
                daemon_unregistration_committed = true,
                host_process_force_killed = true,
                durable_intent_restored = true,
                protected_runtime_restored = true,
                background_recovery_converged = true,
                daemon_and_compatibility_state_absent = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteFaultCampaignEvidenceIfRequested(int iterations)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_CAMPAIGN_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_phase = iterations,
            total_forced_terminations = iterations * RuntimeDeletionCrashPhases.Length,
            phases = RuntimeDeletionCrashPhases,
            checks = new
            {
                real_leserpentd = true,
                every_durable_transition_covered = true,
                every_host_process_force_killed = true,
                every_intent_restored = true,
                every_protected_runtime_rejected_new_work = true,
                every_background_recovery_converged = true,
                every_daemon_and_compatibility_state_absent = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteConcurrentFaultCampaignEvidenceIfRequested(int iterations)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_CONCURRENCY_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var scenarioCount = iterations * RuntimeDeletionCrashPhases.Length;
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_phase = iterations,
            total_forced_terminations = scenarioCount,
            phases = RuntimeDeletionCrashPhases,
            interference_runtimes_per_scenario = RuntimeDeletionInterferenceRuntimeCount,
            total_interference_registrations =
                scenarioCount * RuntimeDeletionInterferenceRuntimeCount,
            checks = new
            {
                real_leserpentd = true,
                every_durable_transition_covered = true,
                concurrent_registration_and_state_save_traffic = true,
                traffic_before_and_after_daemon_commit = true,
                local_cleanup_raced_with_normal_writes = true,
                every_unrelated_runtime_survived_in_memory = true,
                every_unrelated_runtime_survived_disk_reload = true,
                every_unrelated_daemon_registration_survived = true,
                every_deletion_recovery_converged = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteDaemonRestartFaultCampaignEvidenceIfRequested(int iterations)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_DAEMON_RESTART_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var scenarioCount = iterations * RuntimeDeletionCrashPhases.Length;
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_phase = iterations,
            total_forced_host_terminations = scenarioCount,
            total_controlled_daemon_restarts = scenarioCount,
            observed_failed_recovery_attempts = scenarioCount,
            phases = RuntimeDeletionCrashPhases,
            interference_runtimes_per_scenario = RuntimeDeletionInterferenceRuntimeCount,
            total_interference_registrations =
                scenarioCount * RuntimeDeletionInterferenceRuntimeCount,
            checks = new
            {
                real_leserpentd = true,
                same_daemon_database_reopened = true,
                every_durable_transition_covered = true,
                every_daemon_stopped_with_sigterm = true,
                every_owner_lease_released_before_restart = true,
                every_offline_recovery_attempt_failed = true,
                every_failed_claim_was_released_for_retry = true,
                concurrent_registration_and_state_save_traffic = true,
                every_unrelated_runtime_survived_disk_reload = true,
                every_unrelated_daemon_registration_survived = true,
                every_post_restart_recovery_converged = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteUncleanDaemonTakeoverEvidenceIfRequested(
        IReadOnlyList<long> takeoverLatenciesMs)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_UNCLEAN_TAKEOVER_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        Assert.Equal(RuntimeDeletionCrashPhases.Length, takeoverLatenciesMs.Count);
        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var scenarioCount = RuntimeDeletionCrashPhases.Length;
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            owner_lease_duration_ms = 30_000,
            total_forced_host_terminations = scenarioCount,
            total_sigkill_daemon_terminations = scenarioCount,
            observed_owner_lease_rejections = scenarioCount,
            phases = RuntimeDeletionCrashPhases,
            interference_runtimes_per_scenario = RuntimeDeletionInterferenceRuntimeCount,
            total_interference_registrations =
                scenarioCount * RuntimeDeletionInterferenceRuntimeCount,
            takeover_latencies_ms = takeoverLatenciesMs,
            min_takeover_latency_ms = takeoverLatenciesMs.Min(),
            max_takeover_latency_ms = takeoverLatenciesMs.Max(),
            average_takeover_latency_ms = takeoverLatenciesMs.Average(),
            checks = new
            {
                real_leserpentd = true,
                every_durable_transition_covered = true,
                every_daemon_terminated_uncleanly = true,
                every_pre_expiry_start_rejected = true,
                every_takeover_waited_for_natural_owner_lease_expiry = true,
                same_daemon_database_reopened = true,
                every_failed_claim_was_released_for_retry = true,
                concurrent_registration_and_state_save_traffic = true,
                every_unrelated_runtime_survived_disk_reload = true,
                every_unrelated_daemon_registration_survived = true,
                every_post_takeover_recovery_converged = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static async Task WaitForSocketAsync(Process daemon, string socketPath)
    {
        for (var attempt = 0; attempt < 1000; attempt++)
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

    private static RuntimeSidecarDiscoveryResult AuthoritySidecarDiscovery() =>
        RuntimeSidecarDiscoveryResult.Succeeded(
            "https://sidecar.example/v1/status",
            new RuntimeSidecarStatusSnapshot(
                "etragon-api",
                DateTimeOffset.Parse("2026-07-20T12:01:00Z"),
                null,
                true,
                "ready",
                2,
                false,
                4,
                true,
                false,
                null,
                new RuntimeSidecarMemorySnapshot(
                    true,
                    1,
                    2,
                    "slot-a",
                    "baseline",
                    "manual",
                    new[]
                    {
                        new RuntimeSidecarMemorySlotSummary(
                            "slot-a",
                            "baseline",
                            null,
                            "manual",
                            DateTimeOffset.Parse("2026-07-20T11:00:00Z"),
                            3,
                            2),
                    })));

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

    private static string RuntimeUnregisteredResponse(
        string commandId,
        string runtimeId,
        int revision) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"runtime_unregistered\",\"payload\":{" +
        "\"command_id\":\"" + commandId + "\"," +
        "\"removed\":[{\"runtime_id\":\"" + runtimeId +
        "\",\"expected_revision\":" + revision + "}]," +
        "\"deleted_orchestra_runtime_count\":0," +
        "\"deleted_orchestra_run_count\":0," +
        "\"deleted_orchestra_event_count\":0," +
        "\"removed_at_unix_ms\":1784620800000,\"replayed\":false}}}";

    private static string RuntimeListResponse() =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"query\"," +
        "\"payload\":{\"kind\":\"runtime_list\",\"revision\":2,\"runtimes\":[" +
        RuntimeProjectionJson() + "]}}}";

    private static string RuntimeInspectResponse(bool includeSidecarStatus = true) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"query\"," +
        "\"payload\":{\"kind\":\"runtime_inspect\",\"revision\":2,\"runtime\":" +
        RuntimeProjectionJson(includeSidecarStatus) + "}}}";

    private static string RuntimeProjectionJson(bool includeSidecarStatus = true) =>
        "{" +
        "\"id\":\"runtime-a\",\"name\":\"Daemon Runtime\",\"endpoint\":\"https://daemon.invalid\"," +
        "\"sidecar_endpoint\":\"https://daemon-sidecar.invalid\"," +
        "\"registered_at_unix_ms\":1784620800000,\"updated_at_unix_ms\":1784626200000," +
        "\"revision\":2,\"refresh_count\":0,\"refresh_status\":\"ready\"," +
        "\"tags\":{\"environment\":\"prod\",\"cluster\":null,\"role\":\"edge\"}," +
        "\"status\":{\"status_source\":\"gewyvern-api\",\"status_fetched_at\":\"2026-07-20T12:00:00Z\",\"status_fetch_error\":null," +
        "\"has_latest_snapshot\":true,\"snapshot_kind\":\"capture\",\"target_count\":3," +
        "\"has_summary_json\":true,\"has_analysis_json\":true,\"has_training_example_json\":false," +
        "\"has_training_dataset_manifest\":false,\"has_export_json\":false,\"has_report_json\":false,\"has_report_html\":false," +
        "\"has_external_sidecar_context\":true,\"has_external_evidence_chain_enrichment\":false,\"has_external_diagnostic_opinion\":false," +
        "\"resilience_degraded\":false,\"resilience_status\":null,\"resilience_summary\":null,\"socket_service_status\":null," +
        "\"socket_consecutive_idle_timeouts\":null,\"socket_total_idle_timeouts\":null}," +
        (includeSidecarStatus
            ? "\"sidecar_status\":{\"status_source\":\"etragon-api\",\"status_fetched_at\":\"2026-07-20T12:01:00Z\",\"status_fetch_error\":null," +
                "\"healthy\":true,\"daemon_status\":\"ready\",\"target_count\":2,\"learning_active\":false,\"learned_routes\":4," +
                "\"has_evidence_chain_enrichment\":true,\"has_diagnostic_opinion\":false,\"last_error\":null," +
                "\"memory\":{\"versions_supported\":true,\"slot_count\":1,\"history_count\":2,\"latest_slot\":\"slot-a\"," +
                "\"latest_label\":\"baseline\",\"latest_source\":\"manual\",\"slots\":[{\"slot\":\"slot-a\",\"label\":\"baseline\"," +
                "\"note\":null,\"source\":\"manual\",\"saved_at\":\"2026-07-20T11:00:00Z\",\"pattern_count\":3,\"label_count\":2}]," +
                "\"fetch_error\":null}},"
            : string.Empty) +
        "\"capabilities\":{\"source\":\"gewyvern-api\",\"service\":\"gewyvern-api\",\"version\":\"1.2.0\"," +
        "\"latest_snapshot\":true,\"authenticated_deployment\":true,\"serve_required\":true,\"external_sidecar_context\":true," +
        "\"target_path_segment_encoding\":\"percent-encoding\",\"target_direct_path_chars\":\"A-Z a-z 0-9 . _ ~ :\"," +
        "\"endpoints\":[\"/v1/capabilities\",\"/v1/deployments\"],\"extensions\":{}}," +
        "\"capabilities_observed_for_revision\":1}";

    private static string BuildCommandId(
        string runtimeId,
        string name,
        string endpoint,
        string? sidecarEndpoint = null)
    {
        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes(
            $"{runtimeId}|{name.Trim()}|{endpoint.Trim()}|{sidecarEndpoint?.Trim() ?? string.Empty}"));
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

    private sealed record RuntimeDeletionCrashScenarioResult(long? OwnerLeaseTakeoverMs);

    private sealed record UncleanDaemonTakeoverResult(
        IReadOnlyList<string> RuntimeIds,
        long TakeoverLatencyMs);

    private sealed class CoordinatedRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner) : IRuntimeRegistrationAuthority
    {
        private readonly TaskCompletionSource unregisterStarted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowDaemonCommit =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource daemonCommitted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowLocalCleanup =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        public bool Enabled => inner.Enabled;
        public Task UnregisterStarted => unregisterStarted.Task;
        public Task DaemonCommitted => daemonCommitted.Task;

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            inner.RegisterAsync(
                request,
                runtimeId,
                cancellationToken,
                update,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            inner.SubmitDiscoveryAsync(
                runtimeId,
                cancellationToken,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);

        public async Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            unregisterStarted.TrySetResult();
            await allowDaemonCommit.Task.WaitAsync(cancellationToken);
            await inner.UnregisterAsync(runtimeIds, cancellationToken);
            daemonCommitted.TrySetResult();
            await allowLocalCleanup.Task.WaitAsync(cancellationToken);
        }

        public void AllowDaemonCommit() => allowDaemonCommit.TrySetResult();
        public void AllowLocalCleanup() => allowLocalCleanup.TrySetResult();
    }

    private sealed class RestartCoordinatedRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner) : IRuntimeRegistrationAuthority
    {
        private readonly TaskCompletionSource firstAttemptFailed =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource retryStarted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowDaemonCommit =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource daemonCommitted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowLocalCleanup =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int attemptCount;

        public bool Enabled => inner.Enabled;
        public Task FirstAttemptFailed => firstAttemptFailed.Task;
        public Task RetryStarted => retryStarted.Task;
        public Task DaemonCommitted => daemonCommitted.Task;

        public Task<string> RegisterAsync(
            RuntimeRegistrationRequest request,
            string runtimeId,
            CancellationToken cancellationToken,
            bool update = false,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            inner.RegisterAsync(
                request,
                runtimeId,
                cancellationToken,
                update,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);

        public Task SubmitDiscoveryAsync(
            string runtimeId,
            CancellationToken cancellationToken,
            CapabilityDiscoveryResult? capabilityDiscovery = null,
            RuntimeStatusDiscoveryResult? statusDiscovery = null,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
            inner.SubmitDiscoveryAsync(
                runtimeId,
                cancellationToken,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);

        public async Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            var attempt = Interlocked.Increment(ref attemptCount);
            if (attempt == 1)
            {
                try
                {
                    await inner.UnregisterAsync(runtimeIds, cancellationToken);
                }
                catch
                {
                    firstAttemptFailed.TrySetResult();
                    throw;
                }

                var error = new InvalidOperationException(
                    "runtime deletion unexpectedly reached an offline daemon");
                firstAttemptFailed.TrySetException(error);
                throw error;
            }

            retryStarted.TrySetResult();
            await allowDaemonCommit.Task.WaitAsync(cancellationToken);
            await inner.UnregisterAsync(runtimeIds, cancellationToken);
            daemonCommitted.TrySetResult();
            await allowLocalCleanup.Task.WaitAsync(cancellationToken);
        }

        public void AllowDaemonCommit() => allowDaemonCommit.TrySetResult();
        public void AllowLocalCleanup() => allowLocalCleanup.TrySetResult();
    }

    private sealed class RestartableTestDaemon(
        string executable,
        string databasePath,
        string socketPath) : IDisposable
    {
        private Process? process;

        public async Task StartAsync()
        {
            if (process is not null)
            {
                throw new InvalidOperationException("leserpentd is already running");
            }

            TryDelete(socketPath);
            process = StartDaemon(executable, databasePath, socketPath);
            try
            {
                await WaitForSocketAsync(process, socketPath);
            }
            catch
            {
                Stop();
                throw;
            }
        }

        public async Task<long> StartAfterOwnerLeaseExpiryAsync()
        {
            var timer = Stopwatch.StartNew();
            var rejectionCount = 0;
            while (timer.Elapsed < TimeSpan.FromSeconds(45))
            {
                try
                {
                    await StartAsync();
                    if (rejectionCount == 0)
                    {
                        Stop();
                        throw new InvalidOperationException(
                            "replacement leserpentd started before the stale owner lease was observed");
                    }
                    return timer.ElapsedMilliseconds;
                }
                catch (InvalidOperationException error) when (
                    error.Message.Contains(
                        "runtime journal is owned by another live process",
                        StringComparison.Ordinal))
                {
                    rejectionCount += 1;
                }
                await Task.Delay(250);
            }
            throw new TimeoutException(
                "replacement leserpentd did not acquire the expired owner lease");
        }

        public void Stop()
        {
            if (process is not null)
            {
                if (!process.HasExited)
                {
                    process.Kill(entireProcessTree: true);
                    process.WaitForExit(5000);
                }
                process.Dispose();
                process = null;
            }
            TryDelete(socketPath);
        }

        public void StopGracefully()
        {
            if (process is null)
            {
                return;
            }
            if (!process.HasExited)
            {
                if (SendSignal(process.Id, 15) != 0 ||
                    !process.WaitForExit(5000))
                {
                    throw new InvalidOperationException(
                        "leserpentd did not stop cleanly after SIGTERM");
                }
            }
            process.Dispose();
            process = null;
            TryDelete(socketPath);
        }

        public void Dispose() => Stop();

        [SuppressMessage(
            "Interoperability",
            "SYSLIB1054:Use LibraryImportAttribute instead of DllImportAttribute",
            Justification = "The test project intentionally remains safe-code only.")]
        [DllImport("libc", EntryPoint = "kill", SetLastError = true)]
        private static extern int SendSignal(int processId, int signal);
    }

    private sealed class CrashTestEnvironment(string contentRootPath) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = contentRootPath;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
