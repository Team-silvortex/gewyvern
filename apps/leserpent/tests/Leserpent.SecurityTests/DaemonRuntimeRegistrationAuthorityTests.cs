using System.Diagnostics;
using System.Diagnostics.CodeAnalysis;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Runtime.CompilerServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Leserpent.ControlPlane;
using Microsoft.Data.Sqlite;
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
    private const int HighCardinalityRuntimeDeletionIntentCount = 32;
    private const int HighCardinalityRuntimeDeletionPoisonStride = 8;
    private const string RuntimeDeletionReconciliationCommitTarget =
        "runtime-reconciliation-atomic";
    private static readonly string[] RuntimeDeletionCrashPhases =
    {
        "intent_persisted",
        "daemon_committed",
        "local_cleanup_persisted",
    };
    private static readonly string[] RuntimeDeletionRetryCrashPhases =
    {
        "retry_acknowledged",
        "retry_daemon_committed",
    };
    private static readonly ConditionalWeakTable<
        Process,
        BoundedProcessOutput> DaemonOutput = new();

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
            CommandResponse(
                runtimeId,
                commandId,
                includeRuntimeProjection: true,
                runtimeName: "Runtime A",
                runtimeEndpoint: "https://runtime.example",
                runtimeSidecarEndpoint: "https://sidecar.example",
                runtimeEnvironment: "prod",
                runtimeCluster: "eu",
                runtimeRole: "edge"));

        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));
        var receipt = await authority.RegisterWithReceiptAsync(
            new RuntimeRegistrationRequest(
                "Runtime A",
                "https://runtime.example",
                "pairing-token",
                Tags: new RuntimeTags("prod", "eu", "edge"),
                SidecarEndpoint: "https://sidecar.example"),
            runtimeId,
            CancellationToken.None);

        await server;
        Assert.True(receipt.Applied);
        Assert.Equal(runtimeId, receipt.RuntimeId);
        Assert.Equal(1UL, receipt.RegistrationRevision);
        Assert.Equal(1UL, receipt.Revision);
        Assert.False(receipt.DiscoveryApplied);
        Assert.DoesNotContain("daemon.invalid", receipt.ToString());
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
    public async Task ConfiguredAuthorityRejectsRegistrationProjectionThatDoesNotMatchCommand()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        const string runtimeId = "runtime-mismatched-registration";
        var commandId = BuildCommandId(
            runtimeId,
            "Runtime A",
            "https://runtime.example",
            null);
        var server = ServeAsync(
            listener,
            requests,
            CommandResponse(
                runtimeId,
                commandId,
                includeRuntimeProjection: true,
                runtimeName: "Another Runtime",
                runtimeEndpoint: "https://runtime.example"));

        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));
        var error = await Assert.ThrowsAsync<DaemonRuntimeRegistrationException>(() =>
            authority.RegisterWithReceiptAsync(
                new RuntimeRegistrationRequest(
                    "Runtime A",
                    "https://runtime.example",
                    "pairing-token"),
                runtimeId,
                CancellationToken.None));

        await server;
        Assert.Equal("daemon_protocol_invalid", error.Code);
        Assert.Single(requests);
        TryDelete(socketPath);
    }

    [Fact]
    public async Task ConfiguredAuthorityRejectsRegistrationReceiptForAnotherCommand()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var server = ServeAsync(
            listener,
            new List<JsonElement>(),
            CommandResponse(
                "runtime-mismatched-command",
                "another-registration-command",
                includeRuntimeProjection: true,
                runtimeName: "Runtime A",
                runtimeEndpoint: "https://runtime.example",
                runtimeSidecarEndpoint: null,
                runtimeEnvironment: null,
                runtimeRole: null));
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var error = await Assert.ThrowsAsync<DaemonRuntimeRegistrationException>(() =>
            authority.RegisterWithReceiptAsync(
                new RuntimeRegistrationRequest(
                    "Runtime A",
                    "https://runtime.example",
                    "pairing-token"),
                "runtime-mismatched-command",
                CancellationToken.None));

        await server;
        Assert.Equal("daemon_protocol_invalid", error.Code);
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
                return QueryResponse(runtimeId, 4);
            }
            var commandId = protocol.GetProperty("payload").GetProperty("command_id").GetString()!;
            return CommandResponse(
                runtimeId,
                commandId,
                index == 1 ? 5 : 6,
                includeRuntimeProjection: true,
                runtimeName: "Runtime A",
                runtimeEndpoint: "https://runtime.example",
                runtimeSidecarEndpoint: null,
                runtimeEnvironment: "prod",
                runtimeCluster: "eu",
                runtimeRole: "edge");
        }, 3);

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
        var receipt = await authority.RegisterWithReceiptAsync(
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
        Assert.Equal(runtimeId, receipt.RuntimeId);
        Assert.Equal(5UL, receipt.RegistrationRevision);
        Assert.Equal(6UL, receipt.Revision);
        Assert.True(receipt.DiscoveryApplied);
        Assert.Equal(3, requests.Count);
        Assert.Equal("runtime_inspect", requests[0].GetProperty("request").GetProperty("request").GetProperty("payload").GetProperty("query").GetProperty("kind").GetString());
        var update = requests[1].GetProperty("request").GetProperty("request").GetProperty("payload");
        Assert.Equal(4, update.GetProperty("expected_revision").GetInt64());
        Assert.Equal("runtime_registration_update", update.GetProperty("command").GetProperty("kind").GetString());
        var intake = requests[2].GetProperty("request").GetProperty("request").GetProperty("payload");
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
        Assert.Single(requests, request =>
            request.GetProperty("request").GetProperty("request").GetProperty("kind").GetString()
                == "query");

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
        const string commandId =
            "runtime-unregister-durable-command-a";
        await authority.UnregisterAsync(
            new[] { runtimeId },
            commandId,
            CancellationToken.None);

        await server;
        Assert.Equal(2, requests.Count);
        var request = requests[1].GetProperty("request").GetProperty("request");
        Assert.Equal("runtime_unregister", request.GetProperty("kind").GetString());
        var payload = request.GetProperty("payload");
        Assert.True(payload.GetProperty("confirmed").GetBoolean());
        Assert.Equal("runtime.unregister", payload.GetProperty("capabilities")[0].GetString());
        Assert.Equal(
            commandId,
            payload.GetProperty("command_id").GetString());
        var target = Assert.Single(payload.GetProperty("targets").EnumerateArray());
        Assert.Equal(runtimeId, target.GetProperty("runtime_id").GetString());
        Assert.Equal(9, target.GetProperty("expected_revision").GetInt64());
        TryDelete(socketPath);
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public async Task ConfiguredAuthorityLooksUpTypedUnregistrationReceipt(
        bool receiptExists)
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        const string commandId =
            "runtime-unregister-durable-lookup-a";
        const string runtimeId = "runtime-delete-lookup-a";
        var server = ServeSequenceAsync(
            listener,
            requests,
            (_, _) => RuntimeUnregistrationReceiptResponse(
                commandId,
                runtimeId,
                receiptExists),
            1);

        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));
        var lookup = await authority
            .LookupUnregistrationReceiptAsync(
                commandId,
                CancellationToken.None);

        await server;
        Assert.Equal(commandId, lookup.CommandId);
        Assert.Equal(receiptExists, lookup.Found);
        if (receiptExists)
        {
            Assert.Equal(
                new[] { runtimeId },
                lookup.RuntimeIds);
            Assert.Equal((ulong)7, lookup.OperationGeneration);
        }
        else
        {
            Assert.Null(lookup.RuntimeIds);
            Assert.Null(lookup.OperationGeneration);
        }
        var request = Assert.Single(requests)
            .GetProperty("request")
            .GetProperty("request");
        Assert.Equal(
            "runtime_unregistration_receipt",
            request.GetProperty("kind").GetString());
        var payload = request.GetProperty("payload");
        Assert.Equal(
            commandId,
            payload.GetProperty("command_id").GetString());
        Assert.Equal(
            "runtime.read",
            payload.GetProperty("capabilities")[0].GetString());
        TryDelete(socketPath);
    }

    [Fact]
    public async Task ConfiguredAuthorityRejectsZeroUnregistrationGeneration()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        const string runtimeId = "runtime-delete-zero-generation";
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
                9,
                operationGeneration: 0);
        }, 2);

        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));
        var error = await Assert.ThrowsAsync<DaemonRuntimeRegistrationException>(
            () => authority.UnregisterAsync(new[] { runtimeId }, CancellationToken.None));

        await server;
        Assert.Equal("daemon_protocol_invalid", error.Code);
        TryDelete(socketPath);
    }

    [Fact]
    public async Task DiscoveryIntakeUsesSuppliedProjectionRevisionWithoutReinspection()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        var requests = new List<JsonElement>();
        const string runtimeId = "runtime-context";
        var server = ServeSequenceAsync(listener, requests, (request, _) =>
        {
            var payload = request
                .GetProperty("request")
                .GetProperty("request")
                .GetProperty("payload");
            return CommandResponse(
                runtimeId,
                payload.GetProperty("command_id").GetString()!,
                43,
                includeRuntimeProjection: true);
        }, 1);
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var receipt = await authority.SubmitDiscoveryAtRevisionAsync(
            runtimeId,
            42,
            CancellationToken.None,
            capabilityDiscovery: AuthorityDiscovery());

        await server;
        var request = Assert.Single(requests)
            .GetProperty("request")
            .GetProperty("request");
        Assert.Equal("command", request.GetProperty("kind").GetString());
        var payload = request.GetProperty("payload");
        Assert.Equal(42UL, payload.GetProperty("expected_revision").GetUInt64());
        Assert.Equal(
            "runtime_discovery_intake",
            payload.GetProperty("command").GetProperty("kind").GetString());
        Assert.True(receipt.Applied);
        Assert.Equal(runtimeId, receipt.RuntimeId);
        Assert.Equal(43UL, receipt.Revision);
        Assert.Equal("Daemon Runtime", receipt.Runtime?.Name);
        Assert.Equal("https://daemon.invalid", receipt.Runtime?.Endpoint);
        Assert.DoesNotContain("daemon.invalid", receipt.ToString());

        TryDelete(socketPath);
    }

    [Fact]
    public async Task DiscoveryIntakeRejectsReceiptThatDoesNotAdvanceExpectedRevision()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        const string runtimeId = "runtime-stale-receipt";
        var server = ServeSequenceAsync(listener, new List<JsonElement>(), (request, _) =>
        {
            var payload = request
                .GetProperty("request")
                .GetProperty("request")
                .GetProperty("payload");
            return CommandResponse(
                runtimeId,
                payload.GetProperty("command_id").GetString()!,
                42,
                includeRuntimeProjection: true);
        }, 1);
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var error = await Assert.ThrowsAsync<DaemonRuntimeRegistrationException>(
            () => authority.SubmitDiscoveryAtRevisionAsync(
                runtimeId,
                42,
                CancellationToken.None,
                capabilityDiscovery: AuthorityDiscovery()));

        await server;
        Assert.Equal("daemon_protocol_invalid", error.Code);
        TryDelete(socketPath);
    }

    [Fact]
    public async Task DiscoveryIntakeRejectsIncoherentEnvelopeAndProjectionRevisions()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        using var listener = BindPrivateSocket(socketPath);
        const string runtimeId = "runtime-incoherent-receipt";
        var server = ServeSequenceAsync(listener, new List<JsonElement>(), (request, _) =>
        {
            var payload = request
                .GetProperty("request")
                .GetProperty("request")
                .GetProperty("payload");
            return CommandResponse(
                runtimeId,
                payload.GetProperty("command_id").GetString()!,
                43,
                includeRuntimeProjection: true,
                responseRevision: 44);
        }, 1);
        var authority = CreateAuthority(
            ("LESERPENT_DAEMON_SOCKET", socketPath),
            ("LESERPENT_DAEMON_TOKEN", Token));

        var error = await Assert.ThrowsAsync<DaemonRuntimeRegistrationException>(
            () => authority.SubmitDiscoveryAtRevisionAsync(
                runtimeId,
                42,
                CancellationToken.None,
                capabilityDiscovery: AuthorityDiscovery()));

        await server;
        Assert.Equal("daemon_protocol_invalid", error.Code);
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
            var discoveryReceipt = await authority.SubmitDiscoveryAtRevisionAsync(
                runtimeId,
                4,
                CancellationToken.None,
                statusDiscovery: RuntimeStatusDiscoveryResult.Failed(
                    "https://runtime.example/v1/latest/status",
                    "raw daemon refresh failure"));
            Assert.True(discoveryReceipt.Applied);
            Assert.Equal(runtimeId, discoveryReceipt.RuntimeId);
            Assert.Equal(5UL, discoveryReceipt.Revision);
            Assert.Equal(
                RuntimeDiagnosticCodes.RuntimeStatusFetchFailed,
                discoveryReceipt.Runtime?.Status.StatusFetchError);

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
    public async Task RealDaemonRejectsReplacedWriterAndAcceptsFreshOwner()
    {
        var daemonBinary =
            Environment.GetEnvironmentVariable(
                "LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) ||
            OperatingSystem.IsWindows())
        {
            return;
        }
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        var statePath = socketPath + ".state.json";
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(
                new Dictionary<string, string?>
                {
                    ["LESERPENT_DAEMON_SOCKET"] = socketPath,
                    ["LESERPENT_DAEMON_TOKEN"] = Token,
                    ["LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS"] =
                        "10000",
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
            .Build();
        using var daemon =
            StartDaemon(daemonBinary, databasePath, socketPath);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var firstStore = new ControlPlaneStateStore(
                configuration,
                new CrashTestEnvironment(
                    Path.GetDirectoryName(statePath)!),
                NullLogger<ControlPlaneStateStore>.Instance);
            using var firstLease =
                new ControlPlaneWriterLease(firstStore);
            var firstSession =
                new DaemonAuthorityWriterSession(configuration);
            var firstFence = new ControlPlaneWriterFence(
                firstLease,
                NullLogger<ControlPlaneWriterFence>.Instance,
                firstSession);
            await firstFence.StartAsync(CancellationToken.None);
            Assert.Equal(
                1UL,
                firstFence.Snapshot().AuthorityGeneration);
            var firstAuthority =
                new DaemonRuntimeRegistrationAuthority(
                    configuration,
                    firstFence);
            await firstAuthority.RegisterAsync(
                new RuntimeRegistrationRequest(
                    "First Owner Runtime",
                    "https://first.example",
                    "pairing-token"),
                "runtime-first-owner",
                CancellationToken.None);

            var takeoverSession =
                new DaemonAuthorityWriterSession(configuration);
            var takeoverTicket = await takeoverSession.ClaimAsync(
                CancellationToken.None);
            Assert.Equal(2UL, takeoverTicket?.Generation);

            var rejected =
                await Assert.ThrowsAsync<
                    DaemonRuntimeRegistrationException>(
                    () => firstAuthority.RegisterAsync(
                        new RuntimeRegistrationRequest(
                            "Stale Owner Runtime",
                            "https://stale.example",
                            "pairing-token"),
                        "runtime-stale-owner",
                        CancellationToken.None));
            Assert.Equal(
                "authority_writer_fence_rejected",
                rejected.Code);
            Assert.Null(await firstAuthority.InspectAsync(
                "runtime-stale-owner",
                CancellationToken.None));

            firstLease.Dispose();
            var takeoverStore = new ControlPlaneStateStore(
                configuration,
                new CrashTestEnvironment(
                    Path.GetDirectoryName(statePath)!),
                NullLogger<ControlPlaneStateStore>.Instance);
            using var takeoverLease =
                new ControlPlaneWriterLease(takeoverStore);
            var takeoverFence = new ControlPlaneWriterFence(
                takeoverLease,
                NullLogger<ControlPlaneWriterFence>.Instance,
                takeoverSession);
            await takeoverFence.StartAsync(CancellationToken.None);
            var takeoverAuthority =
                new DaemonRuntimeRegistrationAuthority(
                    configuration,
                    takeoverFence);
            await takeoverAuthority.RegisterAsync(
                new RuntimeRegistrationRequest(
                    "Takeover Runtime",
                    "https://takeover.example",
                    "pairing-token"),
                "runtime-takeover-owner",
                CancellationToken.None);
            Assert.NotNull(await takeoverAuthority.InspectAsync(
                "runtime-takeover-owner",
                CancellationToken.None));
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
            TryDelete(statePath);
            TryDelete(statePath + ".control-writer.lease");
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
    public async Task RuntimeDeletionRetryAcknowledgementSurvivesHostTermination()
    {
        var daemonBinary = Environment.GetEnvironmentVariable(
            "LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) ||
            OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_RETRY_CRASH_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 20)
            : 3;
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        using var daemon = StartDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            var results =
                new List<RuntimeDeletionRetryCrashScenarioResult>();
            foreach (var phase in RuntimeDeletionRetryCrashPhases)
            {
                for (var iteration = 0;
                     iteration < iterations;
                     iteration += 1)
                {
                    results.Add(
                        await ExecuteRuntimeDeletionRetryCrashScenarioAsync(
                            harnessAssembly,
                            socketPath,
                            authority,
                            phase,
                            iteration));
                }
            }
            WriteRuntimeDeletionRetryCrashEvidenceIfRequested(
                iterations,
                results);
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
    public async Task RuntimeDeletionLostAcknowledgementRecoversByReceiptLookupAfterHostTermination()
    {
        var daemonBinary = Environment.GetEnvironmentVariable(
            "LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) ||
            OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_LOST_ACK_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 20)
            : 3;
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        using var daemon = StartDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            var results =
                new List<RuntimeDeletionLostAcknowledgementResult>();
            for (var iteration = 0;
                 iteration < iterations;
                 iteration += 1)
            {
                results.Add(
                    await ExecuteRuntimeDeletionLostAcknowledgementScenarioAsync(
                        harnessAssembly,
                        socketPath,
                        authority,
                        iteration));
            }
            WriteRuntimeDeletionLostAcknowledgementEvidenceIfRequested(
                iterations,
                results);
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
    public async Task RuntimeDeletionEvictedLostAcknowledgementFailsClosedAfterHostTermination()
    {
        var daemonBinary = Environment.GetEnvironmentVariable(
            "LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) ||
            OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(File.Exists(harnessAssembly));
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        using var daemon = StartDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            var result =
                await ExecuteRuntimeDeletionLostAcknowledgementScenarioAsync(
                    harnessAssembly,
                    socketPath,
                    authority,
                    iteration: 0,
                    evictReceipt: true);
            WriteRuntimeDeletionReplayHorizonEvidenceIfRequested(
                result);
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
                databasePath + "-shm",
                databasePath + "-wal",
            })
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionReconciliationCommitIsAtomicAcrossHostTermination()
    {
        var daemonBinary = Environment.GetEnvironmentVariable(
            "LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) ||
            OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_RECONCILIATION_COMMIT_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 10)
            : 3;
        var rootPath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-reconciliation-commit-{Guid.NewGuid():N}");
        var baselinePath = $"{rootPath}.baseline.json";
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        var results =
            new List<RuntimeDeletionReconciliationCommitResult>();
        using var daemon = StartDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            await authority.RegisterAsync(
                new RuntimeRegistrationRequest(
                    "Runtime reconciliation revision sentinel",
                    "http://127.0.0.1:19120",
                    "pairing-token"),
                "runtime-reconciliation-revision-sentinel",
                CancellationToken.None);
            var daemonSnapshot = await authority.SnapshotAsync(
                CancellationToken.None);
            Assert.True(daemonSnapshot.Revision > 0);
            Assert.DoesNotContain(
                daemonSnapshot.Runtimes,
                runtime => string.Equals(
                    runtime.RuntimeId,
                    RuntimeDeletionReconciliationCommitTarget,
                    StringComparison.Ordinal));

            CreateRuntimeDeletionReconciliationCommitBaseline(
                baselinePath);
            foreach (var strategy in Enum.GetValues<
                RuntimeDeletionReconciliationCommitStrategy>())
            {
                for (var iteration = 0;
                     iteration < iterations;
                     iteration += 1)
                {
                    results.Add(
                        await ExecuteRuntimeDeletionReconciliationCommitAsync(
                            harnessAssembly,
                            baselinePath,
                            socketPath,
                            rootPath,
                            daemonSnapshot,
                            strategy,
                            iteration));
                }
            }

            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionReconciliationCommitStrategy.BeforeWrite),
                result => Assert.Equal(
                    RuntimeDeletionReconciliationCommitWindow.Previous,
                    result.Window));
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionReconciliationCommitStrategy
                        .DuringTempWrite),
                result => Assert.True(result.TempArtifactObserved));
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionReconciliationCommitStrategy.AfterCommit),
                result => Assert.Equal(
                    RuntimeDeletionReconciliationCommitWindow.Replacement,
                    result.Window));
            Assert.All(results, result =>
            {
                Assert.Equal(256, result.RetryAuditCount);
                Assert.True(result.FinalStateConverged);
                Assert.True(result.ReconciliationAuditSurvivedReload);
                Assert.True(result.RequestReplayedAfterRestart);
            });
            WriteRuntimeDeletionReconciliationCommitEvidenceIfRequested(
                iterations,
                daemonSnapshot.Revision,
                results);
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
                baselinePath,
                baselinePath + ".bak",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(rootPath)!,
                $"{Path.GetFileName(rootPath)}*"))
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionReconciliationConvergesAcrossOrchestraAndControlStateCrash()
    {
        var daemonBinary = Environment.GetEnvironmentVariable(
            "LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) ||
            OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_CROSS_AUTHORITY_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 10)
            : 3;
        var rootPath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-cross-authority-{Guid.NewGuid():N}");
        var baselinePath = $"{rootPath}.baseline.json";
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        var results =
            new List<RuntimeDeletionCrossAuthorityResult>();
        var cleanupCheckpointRaces =
            new List<CleanupCheckpointRaceResult>();
        using var daemon = StartDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await WaitForSocketAsync(daemon, socketPath);
            var authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            await authority.RegisterAsync(
                new RuntimeRegistrationRequest(
                    "Cross-authority revision sentinel",
                    "http://127.0.0.1:19122",
                    "pairing-token"),
                "runtime-cross-authority-revision-sentinel",
                CancellationToken.None);
            var daemonSnapshot = await authority.SnapshotAsync(
                CancellationToken.None);
            Assert.True(daemonSnapshot.Revision > 0);
            Assert.DoesNotContain(
                daemonSnapshot.Runtimes,
                runtime => string.Equals(
                    runtime.RuntimeId,
                    RuntimeDeletionReconciliationCommitTarget,
                    StringComparison.Ordinal));

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_DAEMON_SOCKET"] = socketPath,
                        ["LESERPENT_DAEMON_TOKEN"] = Token,
                        ["LESERPENT_DAEMON_ORCHESTRA_TIMEOUT_MS"] =
                            "30000",
                    })
                .Build();
            var orchestraStore = new DaemonOrchestraRunStore(
                configuration,
                NullLogger<DaemonOrchestraRunStore>.Instance);
            var unrelatedRun =
                CreateCrossAuthorityOrchestraRun(
                    "orun-cross-authority-unrelated",
                    "runtime-cross-authority-unrelated",
                    "request-cross-authority-unrelated",
                    DateTimeOffset.UtcNow.AddMinutes(-5));
            Assert.True(orchestraStore.Upsert(
                unrelatedRun,
                ControlPlaneStateValidator
                    .CreateLegacyOrchestraImportEvent(
                        unrelatedRun)));
            CreateRuntimeDeletionReconciliationCommitBaseline(
                baselinePath);

            foreach (var strategy in Enum.GetValues<
                RuntimeDeletionCrossAuthorityStrategy>())
            {
                for (var iteration = 0;
                     iteration < iterations;
                     iteration += 1)
                {
                    var targetRun =
                        CreateCrossAuthorityOrchestraRun(
                            $"orun-cross-authority-{strategy.ToString().ToLowerInvariant()}-{iteration:D2}",
                            RuntimeDeletionReconciliationCommitTarget,
                            $"request-cross-authority-{strategy.ToString().ToLowerInvariant()}-{iteration:D2}",
                            DateTimeOffset.UtcNow.AddMinutes(-4)
                                .AddSeconds(iteration));
                    Assert.True(orchestraStore.Upsert(
                        targetRun,
                        ControlPlaneStateValidator
                            .CreateLegacyOrchestraImportEvent(
                                targetRun)));
                    results.Add(
                        await ExecuteRuntimeDeletionCrossAuthorityAsync(
                            harnessAssembly,
                            baselinePath,
                            socketPath,
                            rootPath,
                            orchestraStore,
                            daemonSnapshot,
                            unrelatedRun,
                            targetRun,
                            strategy,
                            iteration));
                }
            }

            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionCrossAuthorityStrategy
                        .AfterOrchestraCleanup),
                result => Assert.Equal(
                    RuntimeDeletionReconciliationCommitWindow.Previous,
                    result.Window));
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionCrossAuthorityStrategy
                        .DuringControlTempWrite),
                result => Assert.True(result.TempArtifactObserved));
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionCrossAuthorityStrategy
                        .AfterControlCommit),
                result => Assert.Equal(
                    RuntimeDeletionReconciliationCommitWindow.Replacement,
                    result.Window));
            Assert.All(results, result =>
            {
                Assert.True(
                    result.TargetHistoryAbsentBeforeTermination);
                Assert.True(result.UnrelatedHistoryPreserved);
                Assert.True(result.FinalStateConverged);
                Assert.True(result.SingleAuditSurvivedReload);
                Assert.True(result.RequestReplayedAfterRestart);
                Assert.True(
                    result.CleanupReceiptReplayedSameGeneration);
                Assert.True(
                    result.AuditCheckpointProtectedReplayHorizon);
            });
            foreach (var order in Enum.GetValues<
                CleanupCheckpointRaceOrder>())
            {
                cleanupCheckpointRaces.Add(
                    await ExecuteCleanupCheckpointRaceAsync(
                        databasePath,
                        orchestraStore,
                        order));
            }
            Assert.All(cleanupCheckpointRaces, result =>
            {
                Assert.True(result.PreSaturationCriticalVisible);
                Assert.True(result.CleanupCommitted);
                Assert.True(result.CheckpointCommitted);
                Assert.True(result.ExpectedCompletionOrderObserved);
                Assert.True(result.FinalHorizonAdmissionSafe);
            });
            var auditCheckpointDaemonRestart =
                await ExecuteAuditCheckpointDaemonRestartAsync(
                    daemonBinary);
            Assert.True(
                auditCheckpointDaemonRestart.DaemonRestarted);
            Assert.True(
                auditCheckpointDaemonRestart
                    .CheckpointLagBeforeDaemonRestart > 0);
            Assert.Equal(
                0UL,
                auditCheckpointDaemonRestart
                    .CheckpointLagAfterDaemonRestart);
            Assert.Equal(
                auditCheckpointDaemonRestart.AuditGeneration,
                auditCheckpointDaemonRestart
                    .CheckpointedThroughGeneration);
            Assert.True(
                auditCheckpointDaemonRestart
                    .AutomaticCheckpointStatusReported);
            WriteRuntimeDeletionCrossAuthorityEvidenceIfRequested(
                iterations,
                daemonSnapshot.Revision,
                results,
                cleanupCheckpointRaces,
                auditCheckpointDaemonRestart);
        }
        catch (Exception error)
        {
            throw new InvalidOperationException(
                $"cross-authority campaign failed; leserpentd output:{Environment.NewLine}{CapturedDaemonOutput(daemon)}",
                error);
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
                baselinePath,
                baselinePath + ".bak",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(rootPath)!,
                $"{Path.GetFileName(rootPath)}*"))
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionRetryAuditRolloverPersistenceIsAtomicAcrossHostTermination()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_RETRY_ATOMIC_ROLLOVER_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 10)
            : 3;
        var rootPath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-retry-atomic-rollover-{Guid.NewGuid():N}");
        var baselinePath = $"{rootPath}.baseline.json";
        var socketPath = TempSocket();
        var results =
            new List<RuntimeDeletionRetryAtomicRolloverResult>();
        try
        {
            CreateRuntimeDeletionRetryAtomicRolloverBaseline(
                baselinePath);
            foreach (var strategy in Enum.GetValues<
                RuntimeDeletionRetryAtomicRolloverStrategy>())
            {
                for (var iteration = 0;
                     iteration < iterations;
                     iteration += 1)
                {
                    results.Add(
                        await ExecuteRuntimeDeletionRetryAtomicRolloverAsync(
                            harnessAssembly,
                            baselinePath,
                            socketPath,
                            rootPath,
                            strategy,
                            iteration));
                }
            }

            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionRetryAtomicRolloverStrategy.BeforeWrite),
                result => Assert.Equal(
                    RuntimeDeletionRetryAtomicRolloverWindow.Previous,
                    result.Window));
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionRetryAtomicRolloverStrategy.DuringTempWrite),
                result => Assert.True(result.TempArtifactObserved));
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionRetryAtomicRolloverStrategy.AfterCommit),
                result => Assert.Equal(
                    RuntimeDeletionRetryAtomicRolloverWindow.Replacement,
                    result.Window));
            Assert.All(
                results,
                result => Assert.Equal(256, result.AuditCount));
            WriteRuntimeDeletionRetryAtomicRolloverEvidenceIfRequested(
                iterations,
                results);
        }
        finally
        {
            foreach (var path in new[]
            {
                baselinePath,
                baselinePath + ".bak",
                baselinePath + ".tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(rootPath)!,
                $"{Path.GetFileName(rootPath)}*"))
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionRetryAuditBackupRefreshIsAtomicAcrossHostTermination()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_RETRY_ATOMIC_BACKUP_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 10)
            : 3;
        var rootPath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-retry-atomic-backup-{Guid.NewGuid():N}");
        var baselinePath = $"{rootPath}.baseline.json";
        var socketPath = TempSocket();
        var results =
            new List<RuntimeDeletionRetryAtomicBackupResult>();
        try
        {
            CreateRuntimeDeletionRetryAtomicRolloverBaseline(
                baselinePath);
            foreach (var strategy in Enum.GetValues<
                RuntimeDeletionRetryAtomicBackupStrategy>())
            {
                for (var iteration = 0;
                     iteration < iterations;
                     iteration += 1)
                {
                    results.Add(
                        await ExecuteRuntimeDeletionRetryAtomicBackupAsync(
                            harnessAssembly,
                            baselinePath,
                            socketPath,
                            rootPath,
                            strategy,
                            iteration));
                }
            }

            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionRetryAtomicBackupStrategy
                        .DuringBackupTempWrite),
                result => Assert.True(result.TempArtifactObserved));
            Assert.All(
                results,
                result =>
                {
                    Assert.True(result.PrimaryWasCorrupted);
                    Assert.Equal(256, result.AuditCount);
                    Assert.True(result.CompletePreviousWindowRestored);
                });
            WriteRuntimeDeletionRetryAtomicBackupEvidenceIfRequested(
                iterations,
                results);
        }
        finally
        {
            foreach (var path in new[]
            {
                baselinePath,
                baselinePath + ".bak",
                baselinePath + ".tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(rootPath)!,
                $"{Path.GetFileName(rootPath)}*"))
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionRetryAuditPostRecoveryWritePreservesKnownGoodBackupAcrossHostTermination()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_RETRY_POST_RECOVERY_WRITE_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 10)
            : 3;
        var rootPath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-retry-post-recovery-write-{Guid.NewGuid():N}");
        var baselinePath = $"{rootPath}.baseline.json";
        var socketPath = TempSocket();
        var results =
            new List<RuntimeDeletionRetryPostRecoveryWriteResult>();
        try
        {
            CreateRuntimeDeletionRetryAtomicRolloverBaseline(
                baselinePath);
            foreach (var strategy in Enum.GetValues<
                RuntimeDeletionRetryPostRecoveryWriteStrategy>())
            {
                for (var iteration = 0;
                     iteration < iterations;
                     iteration += 1)
                {
                    results.Add(
                        await ExecuteRuntimeDeletionRetryPostRecoveryWriteAsync(
                            harnessAssembly,
                            baselinePath,
                            socketPath,
                            rootPath,
                            ControlPlaneStateLoadFailureCode.InvalidJson,
                            strategy,
                            iteration));
                }
            }

            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionRetryPostRecoveryWriteStrategy
                        .BeforeWrite),
                result => Assert.Equal(
                    RuntimeDeletionRetryAtomicRolloverWindow.Previous,
                    result.ActiveWindow));
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionRetryPostRecoveryWriteStrategy
                        .DuringPrimaryTempWrite),
                result =>
                {
                    Assert.True(result.PrimaryTempArtifactObserved);
                    Assert.NotEqual(
                        RuntimeDeletionRetryAtomicRolloverWindow.Torn,
                        result.ActiveWindow);
                });
            Assert.All(
                results.Where(result =>
                    result.Strategy ==
                    RuntimeDeletionRetryPostRecoveryWriteStrategy
                        .AfterCommit),
                result => Assert.Equal(
                    RuntimeDeletionRetryAtomicRolloverWindow.Replacement,
                    result.ActiveWindow));
            Assert.All(
                results,
                result =>
                {
                    Assert.True(result.BackupWindowPreserved);
                    Assert.True(result.BackupTempArtifactAbsent);
                    Assert.Equal(256, result.ActiveAuditCount);
                    Assert.Equal(256, result.BackupAuditCount);
                });
            WriteRuntimeDeletionRetryPostRecoveryWriteEvidenceIfRequested(
                "LESERPENT_RUNTIME_DELETION_RETRY_POST_RECOVERY_WRITE_EVIDENCE",
                ControlPlaneStateLoadFailureCode.InvalidJson,
                iterations,
                results);
        }
        finally
        {
            foreach (var path in new[]
            {
                baselinePath,
                baselinePath + ".bak",
                baselinePath + ".tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(rootPath)!,
                $"{Path.GetFileName(rootPath)}*"))
            {
                TryDelete(path);
            }
        }
    }

    [Fact]
    public async Task RuntimeDeletionRetryAuditSemanticInvalidGenerationNeverPromotesAcrossHostTermination()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }

        var harnessAssembly = FindCrashHarnessAssembly();
        Assert.True(
            File.Exists(harnessAssembly),
            $"crash harness was not built at {harnessAssembly}");
        var iterations = int.TryParse(
            Environment.GetEnvironmentVariable(
                "LESERPENT_RUNTIME_DELETION_RETRY_SEMANTIC_GENERATION_ITERATIONS"),
            out var configuredIterations)
            ? Math.Clamp(configuredIterations, 1, 10)
            : 3;
        var rootPath = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-retry-semantic-generation-{Guid.NewGuid():N}");
        var baselinePath = $"{rootPath}.baseline.json";
        var socketPath = TempSocket();
        var results =
            new List<RuntimeDeletionRetryPostRecoveryWriteResult>();
        try
        {
            CreateRuntimeDeletionRetryAtomicRolloverBaseline(
                baselinePath);
            foreach (var strategy in Enum.GetValues<
                RuntimeDeletionRetryPostRecoveryWriteStrategy>())
            {
                for (var iteration = 0;
                     iteration < iterations;
                     iteration += 1)
                {
                    results.Add(
                        await ExecuteRuntimeDeletionRetryPostRecoveryWriteAsync(
                            harnessAssembly,
                            baselinePath,
                            socketPath,
                            rootPath,
                            ControlPlaneStateLoadFailureCode
                                .SemanticInvalid,
                            strategy,
                            iteration));
                }
            }

            Assert.All(
                results,
                result =>
                {
                    Assert.NotEqual(
                        RuntimeDeletionRetryAtomicRolloverWindow.Torn,
                        result.ActiveWindow);
                    Assert.True(result.BackupWindowPreserved);
                    Assert.True(result.BackupTempArtifactAbsent);
                    Assert.Equal(256, result.ActiveAuditCount);
                    Assert.Equal(256, result.BackupAuditCount);
                });
            Assert.All(
                results.Where(result =>
                    result.ActiveWindow ==
                    RuntimeDeletionRetryAtomicRolloverWindow.Previous),
                result => Assert.Equal(
                    ControlPlaneStateLoadFailureCode.SemanticInvalid,
                    result.LoadProvenance.PrimaryFailureCode));
            WriteRuntimeDeletionRetryPostRecoveryWriteEvidenceIfRequested(
                "LESERPENT_RUNTIME_DELETION_RETRY_SEMANTIC_GENERATION_EVIDENCE",
                ControlPlaneStateLoadFailureCode.SemanticInvalid,
                iterations,
                results);
        }
        finally
        {
            foreach (var path in new[]
            {
                baselinePath,
                baselinePath + ".bak",
                baselinePath + ".tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(rootPath)!,
                $"{Path.GetFileName(rootPath)}*"))
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
    public async Task RuntimeDeletionRecoversOverlappingIntentsAfterSingleUncleanTakeover()
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
        var statePath = socketPath + ".overlapping.state.json";
        var markerPath = socketPath + ".overlapping.marker";
        const string runtimePrefix = "runtime-overlapping";
        var targetRuntimeIds = RuntimeDeletionCrashPhases
            .Select(phase => $"{runtimePrefix}-{phase.Replace('_', '-')}")
            .ToArray();
        var interferenceRuntimeIds = Enumerable.Range(0, RuntimeDeletionInterferenceRuntimeCount)
            .Select(index => $"runtime-overlapping-traffic-{index}")
            .ToArray();
        Process? harness = null;
        int? harnessProcessId = null;
        RuntimeDeletionRecoveryService? recovery = null;
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
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                runtimePrefix,
                "mixed_overlapping");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            Assert.NotNull(await authority.InspectAsync(
                targetRuntimeIds[0],
                CancellationToken.None));
            Assert.Null(await authority.InspectAsync(
                targetRuntimeIds[1],
                CancellationToken.None));
            Assert.Null(await authority.InspectAsync(
                targetRuntimeIds[2],
                CancellationToken.None));
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);
            daemon.Stop();

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
                .Build();
            var restarted = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            Assert.Equal(3, restarted.ListPendingRuntimeDeletions().Count);
            Assert.NotNull(restarted.GetRuntime(targetRuntimeIds[0]));
            Assert.NotNull(restarted.GetRuntime(targetRuntimeIds[1]));
            Assert.Null(restarted.GetRuntime(targetRuntimeIds[2]));

            var coordinatedAuthority = new MultiIntentTakeoverRuntimeDeletionAuthority(
                authority,
                targetRuntimeIds);
            recovery = new RuntimeDeletionRecoveryService(
                restarted,
                coordinatedAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            await recovery.StartAsync(CancellationToken.None);
            await coordinatedAuthority.AllInitialAttemptsFailed.WaitAsync(
                TimeSpan.FromSeconds(5));

            RegisterLocalInterferenceBatch(restarted, interferenceRuntimeIds[..2]);
            var takeoverLatencyMs = await daemon.StartAfterOwnerLeaseExpiryAsync();
            await RegisterDaemonInterferenceBatchAsync(
                authority,
                interferenceRuntimeIds[..2]);
            var racingRegistrations = RegisterInterferenceBatchAsync(
                restarted,
                authority,
                interferenceRuntimeIds[2..]);
            coordinatedAuthority.AllowRetries();
            await racingRegistrations;
            await coordinatedAuthority.AllRetriesSucceeded.WaitAsync(
                TimeSpan.FromSeconds(10));
            await WaitForDeletionRecoveryAsync(restarted, targetRuntimeIds);

            Assert.Empty(restarted.ListPendingRuntimeDeletions());
            foreach (var runtimeId in targetRuntimeIds)
            {
                Assert.Null(restarted.GetRuntime(runtimeId));
                Assert.Null(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }

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
            foreach (var runtimeId in interferenceRuntimeIds)
            {
                Assert.NotNull(restarted.GetRuntime(runtimeId));
                Assert.NotNull(diskReloaded.GetRuntime(runtimeId));
                Assert.NotNull(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }
            WriteOverlappingIntentTakeoverEvidenceIfRequested(takeoverLatencyMs);
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
            daemon.Stop();
            try
            {
                var authority = CreateAuthority(
                    ("LESERPENT_DAEMON_SOCKET", socketPath),
                    ("LESERPENT_DAEMON_TOKEN", Token),
                    ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
                await authority.UnregisterAsync(
                    interferenceRuntimeIds,
                    CancellationToken.None);
            }
            catch
            {
                // The isolated daemon database is removed below.
            }
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
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

    [Fact]
    public async Task RuntimeDeletionKeepsPartialProgressAcrossRepeatedUncleanTakeovers()
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
        var statePath = socketPath + ".repeated-takeover.state.json";
        var markerPath = socketPath + ".repeated-takeover.marker";
        const string runtimePrefix = "runtime-repeated-takeover";
        var targetRuntimeIds = RuntimeDeletionCrashPhases
            .Select(phase => $"{runtimePrefix}-{phase.Replace('_', '-')}")
            .ToArray();
        var interferenceRuntimeIds = Enumerable.Range(0, RuntimeDeletionInterferenceRuntimeCount)
            .Select(index => $"runtime-repeated-takeover-traffic-{index}")
            .ToArray();
        Process? harness = null;
        int? harnessProcessId = null;
        RuntimeDeletionRecoveryService? recovery = null;
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
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                runtimePrefix,
                "mixed_overlapping");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);
            daemon.Stop();

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
                .Build();
            var restarted = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            Assert.Equal(3, restarted.ListPendingRuntimeDeletions().Count);

            var coordinatedAuthority =
                new InterruptedMultiIntentTakeoverRuntimeDeletionAuthority(
                    authority,
                    targetRuntimeIds);
            recovery = new RuntimeDeletionRecoveryService(
                restarted,
                coordinatedAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            await recovery.StartAsync(CancellationToken.None);
            await coordinatedAuthority.AllInitialAttemptsFailed.WaitAsync(
                TimeSpan.FromSeconds(5));

            RegisterLocalInterferenceBatch(restarted, interferenceRuntimeIds[..2]);
            var firstTakeoverLatencyMs = await daemon.StartAfterOwnerLeaseExpiryAsync();
            await RegisterDaemonInterferenceBatchAsync(
                authority,
                interferenceRuntimeIds[..2]);
            await RegisterInterferenceBatchAsync(
                restarted,
                authority,
                interferenceRuntimeIds[2..4]);
            coordinatedAuthority.AllowFirstRetry();
            await coordinatedAuthority.FirstRetryDaemonCommitted.WaitAsync(
                TimeSpan.FromSeconds(10));

            daemon.Stop();
            coordinatedAuthority.CompleteFirstRetryAfterSecondTermination();
            RegisterLocalInterferenceBatch(restarted, interferenceRuntimeIds[4..6]);
            await coordinatedAuthority.RemainingAttemptsFailedAfterSecondTermination.WaitAsync(
                TimeSpan.FromSeconds(10));
            await WaitForPendingDeletionCountAsync(restarted, 2);
            var partiallyConvergedRuntimeId = coordinatedAuthority.FirstCommittedRuntimeId;
            Assert.NotNull(partiallyConvergedRuntimeId);
            Assert.Null(restarted.GetRuntime(partiallyConvergedRuntimeId));

            var secondTakeoverLatencyMs = await daemon.StartAfterOwnerLeaseExpiryAsync();
            await RegisterDaemonInterferenceBatchAsync(
                authority,
                interferenceRuntimeIds[4..6]);
            var racingRegistrations = RegisterInterferenceBatchAsync(
                restarted,
                authority,
                interferenceRuntimeIds[6..]);
            coordinatedAuthority.AllowFinalRetries();
            await racingRegistrations;
            await coordinatedAuthority.AllFinalRetriesSucceeded.WaitAsync(
                TimeSpan.FromSeconds(10));
            await WaitForDeletionRecoveryAsync(restarted, targetRuntimeIds);

            Assert.Empty(restarted.ListPendingRuntimeDeletions());
            foreach (var runtimeId in targetRuntimeIds)
            {
                Assert.Null(restarted.GetRuntime(runtimeId));
                Assert.Null(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }

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
            foreach (var runtimeId in interferenceRuntimeIds)
            {
                Assert.NotNull(restarted.GetRuntime(runtimeId));
                Assert.NotNull(diskReloaded.GetRuntime(runtimeId));
                Assert.NotNull(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }
            WriteRepeatedTakeoverEvidenceIfRequested(
                firstTakeoverLatencyMs,
                secondTakeoverLatencyMs);
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
            daemon.Stop();
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
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

    [Fact]
    public async Task RuntimeDeletionPoisonIntentDoesNotStarveIndependentRecovery()
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
        var statePath = socketPath + ".poison-intent.state.json";
        var markerPath = socketPath + ".poison-intent.marker";
        const string runtimePrefix = "runtime-poison-intent";
        var targetRuntimeIds = RuntimeDeletionCrashPhases
            .Select(phase => $"{runtimePrefix}-{phase.Replace('_', '-')}")
            .ToArray();
        var poisonRuntimeId = targetRuntimeIds[0];
        var healthyRuntimeIds = targetRuntimeIds[1..];
        var interferenceRuntimeIds = Enumerable.Range(0, RuntimeDeletionInterferenceRuntimeCount)
            .Select(index => $"runtime-poison-intent-traffic-{index}")
            .ToArray();
        Process? harness = null;
        int? harnessProcessId = null;
        RuntimeDeletionRecoveryService? recovery = null;
        RuntimeDeletionRecoveryService? repairRecovery = null;
        DaemonRuntimeRegistrationAuthority? authority = null;
        using var daemon = new RestartableTestDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await daemon.StartAsync();
            authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                runtimePrefix,
                "mixed_overlapping");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
                .Build();
            var restarted = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            var pendingIntents = restarted.ListPendingRuntimeDeletions();
            Assert.Equal(3, pendingIntents.Count);
            Assert.Equal(poisonRuntimeId, Assert.Single(pendingIntents[0].RuntimeIds));
            await RegisterInterferenceBatchAsync(
                restarted,
                authority,
                interferenceRuntimeIds);

            var poisonAuthority = new PoisonIntentRuntimeDeletionAuthority(
                authority,
                poisonRuntimeId,
                healthyRuntimeIds);
            recovery = new RuntimeDeletionRecoveryService(
                restarted,
                poisonAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            var healthyConvergenceTimer = Stopwatch.StartNew();
            await recovery.StartAsync(CancellationToken.None);
            await poisonAuthority.AllHealthyIntentsSucceeded.WaitAsync(
                TimeSpan.FromSeconds(5));
            await WaitForPendingDeletionCountAsync(restarted, 1);
            var healthyConvergenceLatencyMs = healthyConvergenceTimer.ElapsedMilliseconds;
            await poisonAuthority.PoisonRetriedThreeTimes.WaitAsync(
                TimeSpan.FromSeconds(5));

            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            recovery = null;
            var remainingIntent = Assert.Single(restarted.ListPendingRuntimeDeletions());
            Assert.Equal(poisonRuntimeId, Assert.Single(remainingIntent.RuntimeIds));
            Assert.NotNull(restarted.GetRuntime(poisonRuntimeId));
            Assert.NotNull(await authority.InspectAsync(
                poisonRuntimeId,
                CancellationToken.None));
            Assert.Equal(
                poisonRuntimeId,
                restarted.CreateSession(new SessionCreateRequest(
                    poisonRuntimeId,
                    "diagnostic",
                    "poison-isolation-test",
                    Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);

            restarted.SaveNow();
            var diskReloaded = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            Assert.Single(diskReloaded.ListPendingRuntimeDeletions());
            Assert.NotNull(diskReloaded.GetRuntime(poisonRuntimeId));
            Assert.Equal(
                poisonRuntimeId,
                diskReloaded.CreateSession(new SessionCreateRequest(
                    poisonRuntimeId,
                    "diagnostic",
                    "poison-reload-test",
                    Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);

            repairRecovery = new RuntimeDeletionRecoveryService(
                diskReloaded,
                authority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            await repairRecovery.StartAsync(CancellationToken.None);
            await WaitForDeletionRecoveryAsync(diskReloaded, poisonRuntimeId);
            await repairRecovery.StopAsync(CancellationToken.None);
            repairRecovery.Dispose();
            repairRecovery = null;
            Assert.Null(await authority.InspectAsync(
                poisonRuntimeId,
                CancellationToken.None));
            foreach (var runtimeId in interferenceRuntimeIds)
            {
                Assert.NotNull(diskReloaded.GetRuntime(runtimeId));
                Assert.NotNull(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }
            WritePoisonIntentIsolationEvidenceIfRequested(
                poisonAuthority.PoisonAttemptCount,
                healthyConvergenceLatencyMs);
        }
        finally
        {
            if (recovery is not null)
            {
                await recovery.StopAsync(CancellationToken.None);
                recovery.Dispose();
            }
            if (repairRecovery is not null)
            {
                await repairRecovery.StopAsync(CancellationToken.None);
                repairRecovery.Dispose();
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
            if (authority is not null)
            {
                try
                {
                    await authority.UnregisterAsync(
                        interferenceRuntimeIds,
                        CancellationToken.None);
                }
                catch
                {
                    // The isolated daemon database is removed below.
                }
            }
            daemon.Stop();
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
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

    [Fact]
    public async Task RuntimeDeletionHighCardinalityQueueMakesBoundedProgressWithSparsePoison()
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
        var statePath = socketPath + ".high-cardinality.state.json";
        var markerPath = socketPath + ".high-cardinality.marker";
        const string runtimePrefix = "runtime-high-cardinality";
        var queueRuntimeIds = Enumerable.Range(
            0,
            HighCardinalityRuntimeDeletionIntentCount)
            .Select(index => $"{runtimePrefix}-queue-{index:D2}")
            .ToArray();
        var poisonRuntimeIds = queueRuntimeIds
            .Where((_, index) =>
                index % HighCardinalityRuntimeDeletionPoisonStride == 0)
            .ToArray();
        var healthyRuntimeIds = queueRuntimeIds
            .Except(poisonRuntimeIds, StringComparer.Ordinal)
            .ToArray();
        var interferenceRuntimeIds = Enumerable.Range(0, RuntimeDeletionInterferenceRuntimeCount)
            .Select(index => $"runtime-high-cardinality-traffic-{index}")
            .ToArray();
        Process? harness = null;
        int? harnessProcessId = null;
        RuntimeDeletionRecoveryService? recovery = null;
        RuntimeDeletionRecoveryService? repairRecovery = null;
        DaemonRuntimeRegistrationAuthority? authority = null;
        using var daemon = new RestartableTestDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await daemon.StartAsync();
            authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                runtimePrefix,
                "high_cardinality");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
                .Build();
            var restarted = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            var pendingIntents = restarted.ListPendingRuntimeDeletions();
            Assert.Equal(HighCardinalityRuntimeDeletionIntentCount, pendingIntents.Count);
            Assert.Equal(
                queueRuntimeIds,
                pendingIntents
                    .Select(intent => Assert.Single(intent.RuntimeIds))
                    .ToArray());
            await RegisterInterferenceBatchAsync(
                restarted,
                authority,
                interferenceRuntimeIds);

            var sparsePoisonAuthority =
                new SparsePoisonRuntimeDeletionAuthority(
                    authority,
                    poisonRuntimeIds,
                    healthyRuntimeIds);
            recovery = new RuntimeDeletionRecoveryService(
                restarted,
                sparsePoisonAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            var firstPassTimer = Stopwatch.StartNew();
            await recovery.StartAsync(CancellationToken.None);
            await sparsePoisonAuthority.FirstPassCompleted.WaitAsync(
                TimeSpan.FromSeconds(10));
            var authorityPhaseLatencyMs = firstPassTimer.ElapsedMilliseconds;
            await WaitForPendingDeletionCountAsync(
                restarted,
                poisonRuntimeIds.Length);
            var firstPassLatencyMs = firstPassTimer.ElapsedMilliseconds;
            await sparsePoisonAuthority.EveryPoisonRetriedThreeTimes.WaitAsync(
                TimeSpan.FromSeconds(10));
            var poisonRetryWindowMs = firstPassTimer.ElapsedMilliseconds;

            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            recovery = null;
            Assert.Equal(
                poisonRuntimeIds,
                restarted.ListPendingRuntimeDeletions()
                    .Select(intent => Assert.Single(intent.RuntimeIds))
                    .ToArray());
            foreach (var runtimeId in healthyRuntimeIds)
            {
                Assert.Null(restarted.GetRuntime(runtimeId));
                Assert.Null(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }
            foreach (var runtimeId in poisonRuntimeIds)
            {
                Assert.NotNull(restarted.GetRuntime(runtimeId));
                Assert.NotNull(await authority.InspectAsync(runtimeId, CancellationToken.None));
                Assert.Equal(
                    runtimeId,
                    restarted.CreateSession(new SessionCreateRequest(
                        runtimeId,
                        "diagnostic",
                        "sparse-poison-test",
                        Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);
            }

            restarted.SaveNow();
            var diskReloaded = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            Assert.Equal(
                poisonRuntimeIds,
                diskReloaded.ListPendingRuntimeDeletions()
                    .Select(intent => Assert.Single(intent.RuntimeIds))
                    .ToArray());
            repairRecovery = new RuntimeDeletionRecoveryService(
                diskReloaded,
                authority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            await repairRecovery.StartAsync(CancellationToken.None);
            await WaitForDeletionRecoveryAsync(diskReloaded, poisonRuntimeIds);
            await repairRecovery.StopAsync(CancellationToken.None);
            repairRecovery.Dispose();
            repairRecovery = null;

            Assert.Empty(diskReloaded.ListPendingRuntimeDeletions());
            foreach (var runtimeId in queueRuntimeIds)
            {
                Assert.Null(diskReloaded.GetRuntime(runtimeId));
                Assert.Null(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }
            foreach (var runtimeId in interferenceRuntimeIds)
            {
                Assert.NotNull(diskReloaded.GetRuntime(runtimeId));
                Assert.NotNull(await authority.InspectAsync(runtimeId, CancellationToken.None));
            }
            WriteHighCardinalityPoisonEvidenceIfRequested(
                sparsePoisonAuthority.PoisonAttemptCounts,
                authorityPhaseLatencyMs,
                firstPassLatencyMs,
                poisonRetryWindowMs);
        }
        finally
        {
            if (recovery is not null)
            {
                await recovery.StopAsync(CancellationToken.None);
                recovery.Dispose();
            }
            if (repairRecovery is not null)
            {
                await repairRecovery.StopAsync(CancellationToken.None);
                repairRecovery.Dispose();
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
            if (authority is not null)
            {
                try
                {
                    await authority.UnregisterAsync(
                        interferenceRuntimeIds,
                        CancellationToken.None);
                }
                catch
                {
                    // The isolated daemon database is removed below.
                }
            }
            daemon.Stop();
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
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

    [Fact]
    public async Task RuntimeDeletionBatchPersistenceFailureRollsBackAndReplaysAgainstRealDaemon()
    {
        var daemonBinary = Environment.GetEnvironmentVariable("LESERPENT_TEST_DAEMON_BIN");
        if (string.IsNullOrWhiteSpace(daemonBinary) || OperatingSystem.IsWindows())
        {
            return;
        }

        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        var statePath = socketPath + ".batch-persistence.state.json";
        var backupPath = statePath + ".bak";
        var runtimeIds = new[]
        {
            "runtime-batch-persistence-a",
            "runtime-batch-persistence-b",
        };
        var sessionIds = new Dictionary<string, string>(StringComparer.Ordinal);
        var runIds = new Dictionary<string, string>(StringComparer.Ordinal);
        RuntimeDeletionRecoveryService? recovery = null;
        DaemonRuntimeRegistrationAuthority? authority = null;
        using var daemon = new RestartableTestDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await daemon.StartAsync();
            authority = CreateAuthority(
                ("LESERPENT_DAEMON_SOCKET", socketPath),
                ("LESERPENT_DAEMON_TOKEN", Token),
                ("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS", "10000"));
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
            var runStore = new ReplayCountingRunStore();
            var registry = new RegistryService(stateStore, runStore);
            foreach (var runtimeId in runtimeIds)
            {
                var request = InterferenceRequest(runtimeId);
                await authority.RegisterAsync(
                    request,
                    runtimeId,
                    CancellationToken.None);
                registry.RegisterRuntime(request, runtimeId);
                sessionIds[runtimeId] = registry.CreateSession(new SessionCreateRequest(
                    runtimeId,
                    "diagnostic",
                    "batch-persistence-test",
                    Array.Empty<SessionCapabilityRequirement>())).Session!.SessionId;
                runIds[runtimeId] = registry.RecordOrchestraRun(
                    runtimeId,
                    "session_preparation",
                    "ok",
                    Array.Empty<OrchestraExecutionStepResult>()).RunId;
                registry.RecordRecoveryActivity(
                    runtimeId,
                    "refresh_status",
                    "network_failed",
                    "retained real-daemon rollback marker");
                registry.ReserveRuntimeDeletion(new[] { runtimeId }).Dispose();
            }

            File.Delete(backupPath);
            Directory.CreateDirectory(backupPath);
            var replayAuthority = new ReplayGatedRuntimeDeletionAuthority(
                authority,
                runtimeIds);
            recovery = new RuntimeDeletionRecoveryService(
                registry,
                replayAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            var timer = Stopwatch.StartNew();
            await recovery.StartAsync(CancellationToken.None);
            await replayAuthority.FirstPassCompleted.WaitAsync(
                TimeSpan.FromSeconds(5));
            await WaitForStatePersistenceRollbackAsync(
                stateStore,
                registry,
                runtimeIds);
            var firstFailureLatencyMs = timer.ElapsedMilliseconds;

            Assert.True(stateStore.IsDirty);
            Assert.NotNull(stateStore.LastSaveError);
            Assert.Equal(1, runStore.DeleteCount);
            Assert.Empty(runStore.LoadAll());
            foreach (var runtimeId in runtimeIds)
            {
                Assert.Null(await authority.InspectAsync(
                    runtimeId,
                    CancellationToken.None));
                Assert.NotNull(registry.GetRuntime(runtimeId));
                Assert.NotNull(registry.GetSession(sessionIds[runtimeId]));
                Assert.NotNull(registry.GetOrchestraRun(runtimeId, runIds[runtimeId]));
                Assert.Contains(
                    registry.GetRuntimeAttention(runtimeId)!.RecentRecoveryActivities,
                    activity =>
                        activity.Summary == "retained real-daemon rollback marker");
                Assert.Equal(
                    runtimeId,
                    registry.CreateSession(new SessionCreateRequest(
                        runtimeId,
                        "diagnostic",
                        "batch-persistence-test",
                        Array.Empty<SessionCapabilityRequirement>())).RuntimeMissing);
            }

            var failedPassReload = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            Assert.Equal(runtimeIds.Length, failedPassReload.ListPendingRuntimeDeletions().Count);
            foreach (var runtimeId in runtimeIds)
            {
                Assert.NotNull(failedPassReload.GetRuntime(runtimeId));
                Assert.NotNull(failedPassReload.GetSession(sessionIds[runtimeId]));
                Assert.NotNull(failedPassReload.GetOrchestraRun(runtimeId, runIds[runtimeId]));
            }

            await replayAuthority.ReplayStarted.WaitAsync(TimeSpan.FromSeconds(3));
            var replayStartLatencyMs = timer.ElapsedMilliseconds;
            Assert.True(replayStartLatencyMs >= firstFailureLatencyMs + 750);
            Directory.Delete(backupPath);
            replayAuthority.AllowReplay();
            await replayAuthority.EveryRuntimeReplayed.WaitAsync(
                TimeSpan.FromSeconds(5));
            await WaitForDeletionRecoveryAsync(registry, runtimeIds);
            var convergenceLatencyMs = timer.ElapsedMilliseconds;
            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            recovery = null;

            Assert.False(stateStore.IsDirty);
            Assert.Null(stateStore.LastSaveError);
            Assert.Equal(2, runStore.DeleteCount);
            Assert.Equal(
                Enumerable.Repeat(2, runtimeIds.Length),
                replayAuthority.AttemptCounts);
            foreach (var runtimeId in runtimeIds)
            {
                Assert.Null(registry.GetRuntime(runtimeId));
                Assert.Null(registry.GetSession(sessionIds[runtimeId]));
                Assert.Null(registry.GetOrchestraRun(runtimeId, runIds[runtimeId]));
                Assert.Null(await authority.InspectAsync(
                    runtimeId,
                    CancellationToken.None));
            }

            var convergedReload = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            Assert.Empty(convergedReload.ListPendingRuntimeDeletions());
            foreach (var runtimeId in runtimeIds)
            {
                Assert.Null(convergedReload.GetRuntime(runtimeId));
                Assert.Null(convergedReload.GetSession(sessionIds[runtimeId]));
                Assert.Null(convergedReload.GetOrchestraRun(runtimeId, runIds[runtimeId]));
            }
            WriteBatchPersistenceFailureEvidenceIfRequested(
                runtimeIds.Length,
                replayAuthority.AttemptCounts,
                firstFailureLatencyMs,
                replayStartLatencyMs,
                convergenceLatencyMs);
        }
        finally
        {
            if (recovery is not null)
            {
                await recovery.StopAsync(CancellationToken.None);
                recovery.Dispose();
            }
            if (Directory.Exists(backupPath))
            {
                Directory.Delete(backupPath);
            }
            if (authority is not null)
            {
                try
                {
                    await authority.UnregisterAsync(
                        runtimeIds,
                        CancellationToken.None);
                }
                catch
                {
                    // The isolated daemon database is removed below.
                }
            }
            daemon.Stop();
            foreach (var path in new[]
            {
                socketPath,
                databasePath,
                databasePath + "-journal",
                databasePath + "-wal",
                databasePath + "-shm",
                statePath,
                backupPath,
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
        var process = Process.Start(start) ??
            throw new InvalidOperationException(
                "failed to start leserpentd");
        var output = DaemonOutput.GetValue(
            process,
            static _ => new BoundedProcessOutput());
        process.OutputDataReceived += (_, args) =>
            output.Append("stdout", args.Data);
        process.ErrorDataReceived += (_, args) =>
            output.Append("stderr", args.Data);
        process.BeginOutputReadLine();
        process.BeginErrorReadLine();
        return process;
    }

    private static string CapturedDaemonOutput(Process daemon) =>
        DaemonOutput.TryGetValue(daemon, out var output)
            ? output.Snapshot()
            : "no daemon output was captured";

    private static void CreateRuntimeDeletionRetryAtomicRolloverBaseline(
        string statePath)
    {
        var runtimeIds = Enumerable.Range(0, 128)
            .Select(index =>
                $"runtime-atomic-rollover-evidence-{index:D3}")
            .ToArray();
        var requestedAt = DateTimeOffset.UtcNow.AddMinutes(-1);
        var audit = Enumerable.Range(0, 256)
            .Select(index =>
                new PersistedRuntimeDeletionRetryAudit(
                    $"retry-atomic-rollover-{index:D3}",
                    $"rdel_atomic_rollover_{index:D3}",
                    runtimeIds,
                    2,
                    3,
                    "atomic-rollover",
                    requestedAt.AddTicks(index)))
            .ToArray();
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(
                new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
            .Build();
        var store = new ControlPlaneStateStore(
            configuration,
            new CrashTestEnvironment(
                Path.GetDirectoryName(statePath)!),
            NullLogger<ControlPlaneStateStore>.Instance);
        store.SaveStrict(
            Array.Empty<PersistedRuntimeState>(),
            Array.Empty<PersistedSessionState>(),
            Array.Empty<OrchestraRunSummary>(),
            Array.Empty<PersistedRuntimeDeletionIntent>(),
            audit);
        store.SaveStrict(
            Array.Empty<PersistedRuntimeState>(),
            Array.Empty<PersistedSessionState>(),
            Array.Empty<OrchestraRunSummary>(),
            Array.Empty<PersistedRuntimeDeletionIntent>(),
            audit);
    }

    private static void
        CreateRuntimeDeletionReconciliationCommitBaseline(
            string statePath)
    {
        CreateRuntimeDeletionRetryAtomicRolloverBaseline(statePath);
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(
                new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                })
            .Build();
        var store = new ControlPlaneStateStore(
            configuration,
            new CrashTestEnvironment(
                Path.GetDirectoryName(statePath)!),
            NullLogger<ControlPlaneStateStore>.Instance);
        var registry = new RegistryService(
            store,
            new InMemoryOrchestraRunStore());
        registry.RegisterRuntime(
            new RuntimeRegistrationRequest(
                "Runtime reconciliation atomic target",
                "http://127.0.0.1:19121",
                "pairing-token"),
            RuntimeDeletionReconciliationCommitTarget);
        var createdSession = registry.CreateSession(
            new SessionCreateRequest(
                RuntimeDeletionReconciliationCommitTarget,
                "diagnostic",
                "reconciliation-crash-campaign",
                Array.Empty<SessionCapabilityRequirement>()));
        Assert.NotNull(createdSession.Session);

        using (var reservation = registry.ReserveRuntimeDeletion(
            new[] { RuntimeDeletionReconciliationCommitTarget }))
        {
            registry.FenceRuntimeDeletionMutation(
                reservation,
                replayHorizonFloor: 1);
            registry.RecordRuntimeDeletionFailures(
                new[]
                {
                    new RuntimeDeletionFailure(
                        reservation,
                        RuntimeDeletionFailureCodes.ReplayAmbiguous,
                        DateTimeOffset.UtcNow),
                });
        }

        var intent = Assert.Single(
            registry.ListPendingRuntimeDeletions());
        Assert.Equal(3, intent.Revision);
        Assert.Equal(
            RuntimeDeletionFailureCodes.ReplayAmbiguous,
            intent.LastFailureCode);
        var state = registry.ExportState();
        for (var generation = 0; generation < 2; generation += 1)
        {
            store.SaveStrict(
                state.Runtimes,
                state.Sessions,
                state.OrchestraRuns,
                state.PendingRuntimeDeletions,
                state.RuntimeDeletionRetryAudit,
                state.RuntimeDeletionReconciliationAudit);
        }
    }

    private static OrchestraRunSummary
        CreateCrossAuthorityOrchestraRun(
            string runId,
            string runtimeId,
            string requestId,
            DateTimeOffset executedAt) =>
        new(
            runId,
            runtimeId,
            "cross-authority-reconciliation",
            "succeeded",
            executedAt,
            Array.Empty<OrchestraExecutionStepResult>(),
            CompletedAt: executedAt.AddSeconds(1),
            RequestId: requestId);

    private static void RewriteCrossAuthorityIntentIdentity(
        string statePath,
        string suffix)
    {
        var context = new LeserpentJsonContext(
            new JsonSerializerOptions());
        PersistedControlPlaneState Load(string path)
        {
            using var input = File.OpenRead(path);
            return JsonSerializer.Deserialize(
                    input,
                    context.PersistedControlPlaneState)
                ?? throw new InvalidDataException(
                    "cross-authority baseline was empty");
        }
        void Save(string path, PersistedControlPlaneState state)
        {
            using var output = File.Create(path);
            JsonSerializer.Serialize(
                output,
                state,
                context.PersistedControlPlaneState);
            output.Flush(flushToDisk: true);
        }

        var primary = Load(statePath);
        var intent = Assert.Single(primary.PendingRuntimeDeletions!);
        var replacementIntentId = $"{intent.IntentId}-{suffix}";
        var replacement = primary with
        {
            PendingRuntimeDeletions = new[]
            {
                intent with
                {
                    IntentId = replacementIntentId,
                    UnregistrationCommandId =
                        RuntimeDeletionCommandIdentity.ForIntent(
                            replacementIntentId),
                },
            },
        };
        Save(statePath, replacement);
        var backup = Load(statePath + ".bak");
        var backupIntent = Assert.Single(
            backup.PendingRuntimeDeletions!);
        Save(
            statePath + ".bak",
            backup with
            {
                PendingRuntimeDeletions = new[]
                {
                    backupIntent with
                    {
                        IntentId = replacementIntentId,
                        UnregistrationCommandId =
                            RuntimeDeletionCommandIdentity.ForIntent(
                                replacementIntentId),
                    },
                },
            });
    }

    private static void WritePostRecoveryInvalidPrimary(
        string statePath,
        string baselinePath,
        ControlPlaneStateLoadFailureCode failureCode)
    {
        if (failureCode ==
            ControlPlaneStateLoadFailureCode.InvalidJson)
        {
            File.WriteAllText(statePath, "{");
            return;
        }
        if (failureCode !=
            ControlPlaneStateLoadFailureCode.SemanticInvalid)
        {
            throw new ArgumentOutOfRangeException(
                nameof(failureCode));
        }

        var stateJsonContext = new LeserpentJsonContext(
            new JsonSerializerOptions());
        using var baselineStream = File.OpenRead(baselinePath);
        var baseline = JsonSerializer.Deserialize(
                baselineStream,
                stateJsonContext.PersistedControlPlaneState)
            ?? throw new InvalidDataException(
                "semantic-generation baseline was empty");
        var now = DateTimeOffset.UtcNow;
        var invalid = baseline with
        {
            PendingRuntimeDeletions = new[]
            {
                new PersistedRuntimeDeletionIntent(
                    "rdel_semantic_generation",
                    new[] { "runtime-semantic-generation" },
                    now.AddSeconds(-2),
                    AttemptCount: 1,
                    LastAttemptAt: now.AddSeconds(-1),
                    NextAttemptAt: now,
                    LastFailureCode:
                        "authority_failure\ncredential=secret",
                    Revision: 2),
            },
        };
        using var output = File.Create(statePath);
        JsonSerializer.Serialize(
            output,
            invalid,
            stateJsonContext.PersistedControlPlaneState);
        output.Flush(flushToDisk: true);
    }

    private static async Task<RuntimeDeletionCrossAuthorityResult>
        ExecuteRuntimeDeletionCrossAuthorityAsync(
            string harnessAssembly,
            string baselinePath,
            string socketPath,
            string rootPath,
            DaemonOrchestraRunStore orchestraStore,
            DaemonRuntimeProjectionSnapshot daemonSnapshot,
            OrchestraRunSummary unrelatedRun,
            OrchestraRunSummary targetRun,
            RuntimeDeletionCrossAuthorityStrategy strategy,
            int iteration)
    {
        var strategyName = strategy.ToString().ToLowerInvariant();
        var statePath =
            $"{rootPath}.{strategyName}.{iteration}.state.json";
        var markerPath =
            $"{rootPath}.{strategyName}.{iteration}.marker";
        var triggerPath = $"{markerPath}.trigger";
        var committedMarkerPath = $"{markerPath}.committed";
        var requestId =
            $"reconcile-cross-authority-{strategyName}-{iteration:D2}";
        Process? harness = null;
        int? harnessProcessId = null;
        var tempArtifactObserved = false;
        try
        {
            File.Copy(baselinePath, statePath, overwrite: true);
            File.Copy(
                baselinePath + ".bak",
                statePath + ".bak",
                overwrite: true);
            RewriteCrossAuthorityIntentIdentity(
                statePath,
                $"{strategyName}-{iteration:D2}");
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                requestId,
                "reconciliation_cross_authority");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            var historyAfterOrchestraCommit =
                orchestraStore.LoadAll();
            var targetHistoryAbsentBeforeTermination =
                historyAfterOrchestraCommit.All(run =>
                    !string.Equals(
                        run.RuntimeId,
                        RuntimeDeletionReconciliationCommitTarget,
                        StringComparison.Ordinal));
            Assert.True(
                targetHistoryAbsentBeforeTermination);
            Assert.Empty(orchestraStore.LoadEvents(
                targetRun.RuntimeId,
                targetRun.RunId));
            AssertCrossAuthorityUnrelatedHistory(
                orchestraStore,
                unrelatedRun);

            if (strategy ==
                RuntimeDeletionCrossAuthorityStrategy
                    .DuringControlTempWrite)
            {
                tempArtifactObserved =
                    await WaitForStateTempArtifactAsync(
                        harness,
                        statePath,
                        triggerPath);
            }
            else if (strategy ==
                RuntimeDeletionCrossAuthorityStrategy
                    .AfterControlCommit)
            {
                File.WriteAllText(triggerPath, "start\n");
                await WaitForMarkerAsync(
                    harness,
                    committedMarkerPath);
            }

            Assert.False(harness.HasExited);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                        ["LESERPENT_DAEMON_SOCKET"] = socketPath,
                        ["LESERPENT_DAEMON_TOKEN"] = Token,
                        ["LESERPENT_DAEMON_ORCHESTRA_TIMEOUT_MS"] =
                            "10000",
                    })
                .Build();
            var reloaded = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new DaemonOrchestraRunStore(
                    configuration,
                    NullLogger<DaemonOrchestraRunStore>.Instance));
            var runtimePresent = reloaded.GetRuntime(
                RuntimeDeletionReconciliationCommitTarget) is not null;
            var sessionPresent = reloaded.ListSessions().Any(session =>
                string.Equals(
                    session.RuntimeId,
                    RuntimeDeletionReconciliationCommitTarget,
                    StringComparison.Ordinal));
            var pending = reloaded.ListPendingRuntimeDeletions();
            var reconciliationAudit =
                reloaded.ListRuntimeDeletionReconciliationAudit();
            var previousGeneration =
                runtimePresent &&
                sessionPresent &&
                pending.Count == 1 &&
                string.Equals(
                    pending[0].LastFailureCode,
                    RuntimeDeletionFailureCodes.ReplayAmbiguous,
                    StringComparison.Ordinal) &&
                reconciliationAudit.Count == 0;
            var replacementGeneration =
                !runtimePresent &&
                !sessionPresent &&
                pending.Count == 0 &&
                reconciliationAudit.Count == 1 &&
                string.Equals(
                    reconciliationAudit[0].RequestId,
                    requestId,
                    StringComparison.Ordinal);
            var window = previousGeneration
                ? RuntimeDeletionReconciliationCommitWindow.Previous
                : replacementGeneration
                    ? RuntimeDeletionReconciliationCommitWindow.Replacement
                    : RuntimeDeletionReconciliationCommitWindow.Torn;
            Assert.NotEqual(
                RuntimeDeletionReconciliationCommitWindow.Torn,
                window);

            var request = new RuntimeDeletionReconcileRequest(
                3,
                daemonSnapshot.Revision,
                requestId,
                "reconciliation-cross-authority-campaign",
                true);
            if (window ==
                RuntimeDeletionReconciliationCommitWindow.Previous)
            {
                var start = reloaded
                    .BeginRuntimeDeletionReconciliation(
                        pending.Single().IntentId,
                        request);
                using var reservation = Assert.IsType<
                    RuntimeDeletionReservation>(start.Reservation);
                var completed =
                    reloaded.CompleteRuntimeDeletionReconciliation(
                        reservation,
                        request,
                        daemonSnapshot);
                Assert.True(completed.Accepted);
                Assert.False(completed.Replayed);
            }
            else
            {
                var replayed =
                    reloaded.BeginRuntimeDeletionReconciliation(
                        reconciliationAudit.Single().IntentId,
                        request);
                Assert.Null(replayed.Reservation);
                Assert.True(replayed.Replay?.Replayed);
            }

            AssertCrossAuthorityUnrelatedHistory(
                orchestraStore,
                unrelatedRun);
            Assert.DoesNotContain(
                orchestraStore.LoadAll(),
                run => string.Equals(
                    run.RuntimeId,
                    RuntimeDeletionReconciliationCommitTarget,
                    StringComparison.Ordinal));
            var finalReload = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new DaemonOrchestraRunStore(
                    configuration,
                    NullLogger<DaemonOrchestraRunStore>.Instance));
            var finalAudit = Assert.Single(
                finalReload.ListRuntimeDeletionReconciliationAudit());
            Assert.Equal(requestId, finalAudit.RequestId);
            Assert.NotNull(finalAudit.OrchestraCleanupCommandId);
            Assert.NotNull(finalAudit.OrchestraCleanupGeneration);
            var replayedCleanupReceipt =
                orchestraStore.DeleteRuntimes(
                    new OrchestraDeleteCommand(
                        finalAudit.OrchestraCleanupCommandId,
                        finalAudit.RuntimeIds));
            Assert.NotNull(replayedCleanupReceipt);
            Assert.True(replayedCleanupReceipt.Replayed);
            Assert.Equal(
                finalAudit.OrchestraCleanupGeneration,
                replayedCleanupReceipt.OperationGeneration);
            var cleanupHorizon =
                orchestraStore.GetDeleteReplayHorizon();
            Assert.NotNull(cleanupHorizon);
            Assert.Equal(
                finalAudit.OrchestraCleanupGeneration,
                cleanupHorizon.OldestGeneration);
            Assert.Equal(
                finalAudit.OrchestraCleanupGeneration,
                cleanupHorizon.ProtectedFromGeneration);
            Assert.True(
                cleanupHorizon.NewestGeneration >=
                    finalAudit.OrchestraCleanupGeneration);
            Assert.Equal(
                checked(finalAudit.OrchestraCleanupGeneration.Value - 1),
                cleanupHorizon.EvictedThroughGeneration);
            Assert.Null(finalReload.GetRuntime(
                RuntimeDeletionReconciliationCommitTarget));
            Assert.DoesNotContain(
                finalReload.ListSessions(),
                session => string.Equals(
                    session.RuntimeId,
                    RuntimeDeletionReconciliationCommitTarget,
                    StringComparison.Ordinal));
            Assert.Empty(
                finalReload.ListPendingRuntimeDeletions());
            var replayAfterRestart =
                finalReload.BeginRuntimeDeletionReconciliation(
                    finalAudit.IntentId,
                    request);
            Assert.Null(replayAfterRestart.Reservation);
            Assert.True(replayAfterRestart.Replay?.Replayed);
            AssertCrossAuthorityUnrelatedHistory(
                orchestraStore,
                unrelatedRun);

            return new RuntimeDeletionCrossAuthorityResult(
                strategy,
                window,
                tempArtifactObserved,
                targetHistoryAbsentBeforeTermination,
                UnrelatedHistoryPreserved: true,
                FinalStateConverged: true,
                SingleAuditSurvivedReload: true,
                RequestReplayedAfterRestart: true,
                CleanupReceiptReplayedSameGeneration: true,
                AuditCheckpointProtectedReplayHorizon: true);
        }
        finally
        {
            if (harness is not null)
            {
                if (!harness.HasExited)
                {
                    harness.Kill(entireProcessTree: true);
                    harness.WaitForExit(5000);
                }
                harness.Dispose();
            }
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                markerPath,
                triggerPath,
                committedMarkerPath,
                markerPath + $".{harnessProcessId}.tmp",
                committedMarkerPath +
                    $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}.*.tmp"))
            {
                TryDelete(path);
            }
        }
    }

    private static async Task<CleanupCheckpointRaceResult>
        ExecuteCleanupCheckpointRaceAsync(
            string databasePath,
            DaemonOrchestraRunStore orchestraStore,
            CleanupCheckpointRaceOrder order)
    {
        var initial = Assert.IsType<OrchestraDeleteReplayHorizon>(
            orchestraStore.GetDeleteReplayHorizon());
        SeedCleanupReplayHorizonToOneAvailableSlot(
            databasePath,
            initial,
            order);
        var critical = Assert.IsType<OrchestraDeleteReplayHorizon>(
            orchestraStore.GetDeleteReplayHorizon());
        Assert.Equal(1UL, critical.AvailableCapacity);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Critical,
            critical.AdmissionPressure);
        Assert.Equal(
            OrchestraDeleteReplayOperatorAction
                .PersistAuditAndAdvanceCheckpoint,
            critical.OperatorAction);
        var checkpointGeneration =
            Assert.IsType<ulong>(critical.NewestGeneration);
        using var start = new ManualResetEventSlim();
        var completionSequence = 0;
        var cleanupCompletion = 0;
        var checkpointCompletion = 0;
        var cleanup = Task.Run(() =>
        {
            start.Wait();
            if (order == CleanupCheckpointRaceOrder.CheckpointFirst)
            {
                Thread.Sleep(50);
            }
            var receipt = orchestraStore.DeleteRuntimes(
                new OrchestraDeleteCommand(
                    $"orchestra-cleanup-checkpoint-race-{order.ToString().ToLowerInvariant()}",
                    new[] { "runtime-cleanup-checkpoint-race" }));
            cleanupCompletion =
                Interlocked.Increment(ref completionSequence);
            return receipt;
        });
        var checkpoint = Task.Run(() =>
        {
            start.Wait();
            if (order == CleanupCheckpointRaceOrder.CleanupFirst)
            {
                Thread.Sleep(50);
            }
            var horizon = orchestraStore.CheckpointDeleteReplayHorizon(
                new OrchestraDeleteReplayCheckpoint(
                    checkpointGeneration,
                    checkpointGeneration));
            checkpointCompletion =
                Interlocked.Increment(ref completionSequence);
            return horizon;
        });
        start.Set();
        await Task.WhenAll(cleanup, checkpoint);

        var receipt = Assert.IsType<OrchestraDeleteReceipt>(
            cleanup.Result);
        Assert.False(receipt.Replayed);
        Assert.Equal(
            checked(checkpointGeneration + 1),
            receipt.OperationGeneration);
        Assert.NotNull(checkpoint.Result);
        var final = Assert.IsType<OrchestraDeleteReplayHorizon>(
            orchestraStore.GetDeleteReplayHorizon());
        Assert.Equal(2UL, final.Retained);
        Assert.Equal(checkpointGeneration, final.OldestGeneration);
        Assert.Equal(receipt.OperationGeneration, final.NewestGeneration);
        Assert.Equal(
            checked(receipt.OperationGeneration + 1),
            final.NextGeneration);
        Assert.Equal(
            checked(checkpointGeneration - 1),
            final.EvictedThroughGeneration);
        Assert.Equal(
            checkpointGeneration,
            final.ProtectedFromGeneration);
        Assert.Equal(4094UL, final.AvailableCapacity);
        Assert.Equal(
            OrchestraDeleteReplayAdmissionPressure.Healthy,
            final.AdmissionPressure);
        Assert.Null(final.OperatorAction);
        var expectedCompletionOrderObserved =
            order == CleanupCheckpointRaceOrder.CleanupFirst
                ? cleanupCompletion < checkpointCompletion
                : checkpointCompletion < cleanupCompletion;
        Assert.True(expectedCompletionOrderObserved);

        return new CleanupCheckpointRaceResult(
            order,
            critical.AvailableCapacity,
            checkpointGeneration,
            receipt.OperationGeneration,
            PreSaturationCriticalVisible: true,
            CleanupCommitted: true,
            CheckpointCommitted: true,
            ExpectedCompletionOrderObserved:
                expectedCompletionOrderObserved,
            FinalHorizonAdmissionSafe: true);
    }

    private static async Task<AuditCheckpointDaemonRestartResult>
        ExecuteAuditCheckpointDaemonRestartAsync(
            string daemonBinary)
    {
        var socketPath = TempSocket();
        var databasePath = socketPath + ".db";
        var statePath = socketPath + ".state.json";
        using var daemon = new RestartableTestDaemon(
            daemonBinary,
            databasePath,
            socketPath);
        try
        {
            await daemon.StartAsync();
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                        ["LESERPENT_DAEMON_SOCKET"] = socketPath,
                        ["LESERPENT_DAEMON_TOKEN"] = Token,
                        ["LESERPENT_DAEMON_ORCHESTRA_TIMEOUT_MS"] =
                            "10000",
                    })
                .Build();
            var orchestraStore = new DaemonOrchestraRunStore(
                configuration,
                NullLogger<DaemonOrchestraRunStore>.Instance);
            var first = Assert.IsType<OrchestraDeleteReceipt>(
                orchestraStore.DeleteRuntimes(
                    new OrchestraDeleteCommand(
                        "orchestra-cleanup-audit-restart-1",
                        new[] { "runtime-audit-restart-1" })));
            var second = Assert.IsType<OrchestraDeleteReceipt>(
                orchestraStore.DeleteRuntimes(
                    new OrchestraDeleteCommand(
                        "orchestra-cleanup-audit-restart-2",
                        new[] { "runtime-audit-restart-2" })));
            Assert.Equal(
                checked(first.OperationGeneration + 1),
                second.OperationGeneration);
            var before = Assert.IsType<
                OrchestraDeleteReplayHorizon>(
                    orchestraStore.GetDeleteReplayHorizon());
            Assert.Null(before.CheckpointedThroughGeneration);
            Assert.Equal(2UL, before.CheckpointLagGenerations);

            var stateStore = new ControlPlaneStateStore(
                configuration,
                new CrashTestEnvironment(
                    Path.GetDirectoryName(statePath)!),
                NullLogger<ControlPlaneStateStore>.Instance);
            stateStore.SaveStrict(
                Array.Empty<PersistedRuntimeState>(),
                Array.Empty<PersistedSessionState>(),
                runtimeDeletionReconciliationAudit:
                [
                    new PersistedRuntimeDeletionReconciliationAudit(
                        "reconcile-request-audit-restart",
                        "delete-intent-audit-restart",
                        second.RuntimeIds,
                        1,
                        2,
                        "operator-a",
                        DateTimeOffset.UtcNow,
                        second.CommandId,
                        second.OperationGeneration),
                ]);

            daemon.StopGracefully();
            await daemon.StartAsync();
            var restartedRegistry = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new DaemonOrchestraRunStore(
                    configuration,
                    NullLogger<DaemonOrchestraRunStore>.Instance));
            var status = Assert.IsType<
                OrchestraDeleteReplayCheckpointStatus>(
                    restartedRegistry
                        .GetOrchestraDeleteReplayCheckpointStatus());
            Assert.Equal(
                second.OperationGeneration,
                status.MinimumAuditedGeneration);
            Assert.Equal(
                second.OperationGeneration,
                status.ObservedThroughAuditedGeneration);
            Assert.Equal(
                second.OperationGeneration,
                status.Horizon.OldestGeneration);
            Assert.Equal(
                second.OperationGeneration,
                status.Horizon.ProtectedFromGeneration);
            Assert.Equal(
                second.OperationGeneration,
                status.Horizon.CheckpointedThroughGeneration);
            Assert.Equal(
                0UL,
                status.Horizon.CheckpointLagGenerations);
            Assert.True(status.LastAutomaticCheckpointAdvanced);
            Assert.NotNull(status.LastAutomaticCheckpointAt);

            return new AuditCheckpointDaemonRestartResult(
                before.CheckpointLagGenerations,
                status.Horizon.CheckpointLagGenerations,
                second.OperationGeneration,
                status.Horizon.CheckpointedThroughGeneration!.Value,
                DaemonRestarted: true,
                AutomaticCheckpointStatusReported: true);
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
                statePath,
                statePath + ".bak",
                statePath + ".tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}.*.tmp"))
            {
                TryDelete(path);
            }
        }
    }

    private static void SeedCleanupReplayHorizonToOneAvailableSlot(
        string databasePath,
        OrchestraDeleteReplayHorizon horizon,
        CleanupCheckpointRaceOrder order)
    {
        const ulong targetRetained = 4095;
        Assert.NotNull(horizon.ProtectedFromGeneration);
        Assert.True(horizon.Retained <= targetRetained);
        var seedCount = checked(targetRetained - horizon.Retained);
        if (seedCount == 0)
        {
            return;
        }
        using var connection = new SqliteConnection(
            $"Data Source={databasePath}");
        connection.Open();
        using var transaction = connection.BeginTransaction();
        using (var insert = connection.CreateCommand())
        {
            insert.Transaction = transaction;
            insert.CommandText = """
                INSERT INTO orchestra_delete_operations (
                    operation_id, generation, request,
                    deleted_runtime_count, deleted_run_count,
                    deleted_event_count, committed_at_unix_ms)
                VALUES (
                    $operation_id, $generation, $request,
                    0, 0, 0, $committed_at_unix_ms);
                """;
            var operationId = insert.Parameters.Add(
                "$operation_id",
                SqliteType.Text);
            var generation = insert.Parameters.Add(
                "$generation",
                SqliteType.Integer);
            insert.Parameters.Add(
                "$request",
                SqliteType.Blob).Value =
                Encoding.UTF8.GetBytes(
                    "[\"runtime-cleanup-checkpoint-race\"]");
            var committedAt = insert.Parameters.Add(
                "$committed_at_unix_ms",
                SqliteType.Integer);
            for (ulong offset = 0; offset < seedCount; offset++)
            {
                var value = checked(horizon.NextGeneration + offset);
                operationId.Value =
                    $"cleanup-checkpoint-{order.ToString().ToLowerInvariant()}-{value}";
                generation.Value = checked((long)value);
                committedAt.Value = checked((long)value);
                Assert.Equal(1, insert.ExecuteNonQuery());
            }
        }
        using (var advance = connection.CreateCommand())
        {
            advance.Transaction = transaction;
            advance.CommandText = """
                UPDATE orchestra_delete_generation
                SET next_generation = $replacement
                WHERE id = 1 AND next_generation = $expected;
                """;
            advance.Parameters.AddWithValue(
                "$replacement",
                checked((long)(horizon.NextGeneration + seedCount)));
            advance.Parameters.AddWithValue(
                "$expected",
                checked((long)horizon.NextGeneration));
            Assert.Equal(1, advance.ExecuteNonQuery());
        }
        transaction.Commit();
    }

    private static void AssertCrossAuthorityUnrelatedHistory(
        DaemonOrchestraRunStore orchestraStore,
        OrchestraRunSummary unrelatedRun)
    {
        var restoredRun = Assert.Single(
            orchestraStore.LoadAll(),
            run => string.Equals(
                run.RunId,
                unrelatedRun.RunId,
                StringComparison.Ordinal));
        Assert.Equal(unrelatedRun.RunId, restoredRun.RunId);
        Assert.Equal(unrelatedRun.RuntimeId, restoredRun.RuntimeId);
        Assert.Equal(unrelatedRun.PlanId, restoredRun.PlanId);
        Assert.Equal(unrelatedRun.Outcome, restoredRun.Outcome);
        Assert.Equal(unrelatedRun.ExecutedAt, restoredRun.ExecutedAt);
        Assert.Equal(unrelatedRun.CompletedAt, restoredRun.CompletedAt);
        Assert.Equal(unrelatedRun.Attempt, restoredRun.Attempt);
        Assert.Equal(
            unrelatedRun.RetriedFromRunId,
            restoredRun.RetriedFromRunId);
        Assert.Equal(unrelatedRun.ApprovedBy, restoredRun.ApprovedBy);
        Assert.Equal(
            unrelatedRun.ApprovalNote,
            restoredRun.ApprovalNote);
        Assert.Equal(
            unrelatedRun.PlanRevision,
            restoredRun.PlanRevision);
        Assert.Equal(unrelatedRun.RequestId, restoredRun.RequestId);
        Assert.Equal(unrelatedRun.Steps, restoredRun.Steps);
        var restoredEvent = Assert.Single(
            orchestraStore.LoadEvents(
                unrelatedRun.RuntimeId,
                unrelatedRun.RunId));
        Assert.Equal(unrelatedRun.RuntimeId, restoredEvent.RuntimeId);
        Assert.Equal(unrelatedRun.RunId, restoredEvent.RunId);
        Assert.Equal("legacy_import", restoredEvent.EventType);
        Assert.Equal("succeeded", restoredEvent.ToOutcome);
    }

    private static async Task<RuntimeDeletionReconciliationCommitResult>
        ExecuteRuntimeDeletionReconciliationCommitAsync(
            string harnessAssembly,
            string baselinePath,
            string socketPath,
            string rootPath,
            DaemonRuntimeProjectionSnapshot daemonSnapshot,
            RuntimeDeletionReconciliationCommitStrategy strategy,
            int iteration)
    {
        var strategyName = strategy.ToString().ToLowerInvariant();
        var statePath =
            $"{rootPath}.{strategyName}.{iteration}.state.json";
        var markerPath =
            $"{rootPath}.{strategyName}.{iteration}.marker";
        var triggerPath = $"{markerPath}.trigger";
        var committedMarkerPath = $"{markerPath}.committed";
        var requestId =
            $"reconcile-commit-{strategyName}-{iteration:D2}";
        Process? harness = null;
        int? harnessProcessId = null;
        var tempArtifactObserved = false;
        try
        {
            File.Copy(baselinePath, statePath, overwrite: true);
            File.Copy(
                baselinePath + ".bak",
                statePath + ".bak",
                overwrite: true);
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                requestId,
                "reconciliation_commit");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            if (strategy ==
                RuntimeDeletionReconciliationCommitStrategy.DuringTempWrite)
            {
                tempArtifactObserved =
                    await WaitForStateTempArtifactAsync(
                        harness,
                        statePath,
                        triggerPath);
            }
            else if (strategy ==
                RuntimeDeletionReconciliationCommitStrategy.AfterCommit)
            {
                File.WriteAllText(triggerPath, "start\n");
                await WaitForMarkerAsync(
                    harness,
                    committedMarkerPath);
            }

            Assert.False(harness.HasExited);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                    })
                .Build();
            var reloaded = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            var runtimePresent = reloaded.GetRuntime(
                RuntimeDeletionReconciliationCommitTarget) is not null;
            var sessionPresent = reloaded.ListSessions().Any(session =>
                string.Equals(
                    session.RuntimeId,
                    RuntimeDeletionReconciliationCommitTarget,
                    StringComparison.Ordinal));
            var pending = reloaded.ListPendingRuntimeDeletions();
            var reconciliationAudit =
                reloaded.ListRuntimeDeletionReconciliationAudit();
            var previousGeneration =
                runtimePresent &&
                sessionPresent &&
                pending.Count == 1 &&
                string.Equals(
                    pending[0].LastFailureCode,
                    RuntimeDeletionFailureCodes.ReplayAmbiguous,
                    StringComparison.Ordinal) &&
                reconciliationAudit.Count == 0;
            var replacementGeneration =
                !runtimePresent &&
                !sessionPresent &&
                pending.Count == 0 &&
                reconciliationAudit.Count == 1 &&
                string.Equals(
                    reconciliationAudit[0].RequestId,
                    requestId,
                    StringComparison.Ordinal);
            var window = previousGeneration
                ? RuntimeDeletionReconciliationCommitWindow.Previous
                : replacementGeneration
                    ? RuntimeDeletionReconciliationCommitWindow.Replacement
                    : RuntimeDeletionReconciliationCommitWindow.Torn;
            Assert.NotEqual(
                RuntimeDeletionReconciliationCommitWindow.Torn,
                window);
            var retryAuditCount =
                reloaded.ListRuntimeDeletionRetryAudit().Count;
            Assert.Equal(256, retryAuditCount);

            var request = new RuntimeDeletionReconcileRequest(
                ExpectedRevision: 3,
                daemonSnapshot.Revision,
                requestId,
                "reconciliation-crash-campaign",
                true);
            if (window ==
                RuntimeDeletionReconciliationCommitWindow.Previous)
            {
                var start = reloaded
                    .BeginRuntimeDeletionReconciliation(
                        pending.Single().IntentId,
                        request);
                using var reservation = Assert.IsType<
                    RuntimeDeletionReservation>(start.Reservation);
                var completed =
                    reloaded.CompleteRuntimeDeletionReconciliation(
                        reservation,
                        request,
                        daemonSnapshot);
                Assert.True(completed.Accepted);
                Assert.False(completed.Replayed);
            }
            else
            {
                var replayed =
                    reloaded.BeginRuntimeDeletionReconciliation(
                        reconciliationAudit.Single().IntentId,
                        request);
                Assert.Null(replayed.Reservation);
                Assert.True(replayed.Replay?.Replayed);
            }

            var finalReload = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            var finalAudit = Assert.Single(
                finalReload.ListRuntimeDeletionReconciliationAudit());
            Assert.Equal(requestId, finalAudit.RequestId);
            Assert.Equal(
                daemonSnapshot.Revision,
                finalAudit.DaemonRevision);
            Assert.Null(finalReload.GetRuntime(
                RuntimeDeletionReconciliationCommitTarget));
            Assert.DoesNotContain(
                finalReload.ListSessions(),
                session => string.Equals(
                    session.RuntimeId,
                    RuntimeDeletionReconciliationCommitTarget,
                    StringComparison.Ordinal));
            Assert.Empty(
                finalReload.ListPendingRuntimeDeletions());
            var replayAfterRestart =
                finalReload.BeginRuntimeDeletionReconciliation(
                    finalAudit.IntentId,
                    request);
            Assert.Null(replayAfterRestart.Reservation);
            Assert.True(replayAfterRestart.Replay?.Replayed);

            return new RuntimeDeletionReconciliationCommitResult(
                strategy,
                window,
                tempArtifactObserved,
                retryAuditCount,
                FinalStateConverged: true,
                ReconciliationAuditSurvivedReload: true,
                RequestReplayedAfterRestart: true);
        }
        finally
        {
            if (harness is not null)
            {
                if (!harness.HasExited)
                {
                    harness.Kill(entireProcessTree: true);
                    harness.WaitForExit(5000);
                }
                harness.Dispose();
            }
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                markerPath,
                triggerPath,
                committedMarkerPath,
                markerPath + $".{harnessProcessId}.tmp",
                committedMarkerPath +
                    $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}.*.tmp"))
            {
                TryDelete(path);
            }
        }
    }

    private static async Task<RuntimeDeletionRetryAtomicRolloverResult>
        ExecuteRuntimeDeletionRetryAtomicRolloverAsync(
            string harnessAssembly,
            string baselinePath,
            string socketPath,
            string rootPath,
            RuntimeDeletionRetryAtomicRolloverStrategy strategy,
            int iteration)
    {
        var strategyName = strategy.ToString().ToLowerInvariant();
        var statePath =
            $"{rootPath}.{strategyName}.{iteration}.state.json";
        var markerPath =
            $"{rootPath}.{strategyName}.{iteration}.marker";
        var triggerPath = $"{markerPath}.trigger";
        var committedMarkerPath = $"{markerPath}.committed";
        Process? harness = null;
        int? harnessProcessId = null;
        var tempArtifactObserved = false;
        try
        {
            File.Copy(baselinePath, statePath, overwrite: true);
            File.Copy(
                baselinePath + ".bak",
                statePath + ".bak",
                overwrite: true);
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                "runtime-atomic-rollover-unused",
                "retry_rollover_persist");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            if (strategy ==
                RuntimeDeletionRetryAtomicRolloverStrategy.DuringTempWrite)
            {
                tempArtifactObserved =
                    await WaitForStateTempArtifactAsync(
                        harness,
                        statePath,
                        triggerPath);
            }
            else if (strategy ==
                RuntimeDeletionRetryAtomicRolloverStrategy.AfterCommit)
            {
                File.WriteAllText(triggerPath, "start\n");
                await WaitForMarkerAsync(
                    harness,
                    committedMarkerPath);
            }

            Assert.False(harness.HasExited);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                    })
                .Build();
            var stateStore = new ControlPlaneStateStore(
                configuration,
                new CrashTestEnvironment(
                    Path.GetDirectoryName(statePath)!),
                NullLogger<ControlPlaneStateStore>.Instance);
            var reloaded = new RegistryService(
                stateStore,
                new InMemoryOrchestraRunStore());
            var requestIds = reloaded
                .ListRuntimeDeletionRetryAudit()
                .Select(static audit => audit.RequestId)
                .ToArray();
            var previousWindow = Enumerable.Range(0, 256)
                .Reverse()
                .Select(index =>
                    $"retry-atomic-rollover-{index:D3}")
                .ToArray();
            var replacementWindow = Enumerable.Range(1, 256)
                .Reverse()
                .Select(index =>
                    $"retry-atomic-rollover-{index:D3}")
                .ToArray();
            var window = requestIds.SequenceEqual(previousWindow)
                ? RuntimeDeletionRetryAtomicRolloverWindow.Previous
                : requestIds.SequenceEqual(replacementWindow)
                    ? RuntimeDeletionRetryAtomicRolloverWindow.Replacement
                    : RuntimeDeletionRetryAtomicRolloverWindow.Torn;
            Assert.NotEqual(
                RuntimeDeletionRetryAtomicRolloverWindow.Torn,
                window);
            Assert.Equal(256, requestIds.Distinct().Count());

            return new RuntimeDeletionRetryAtomicRolloverResult(
                strategy,
                window,
                tempArtifactObserved,
                requestIds.Length);
        }
        finally
        {
            if (harness is not null)
            {
                if (!harness.HasExited)
                {
                    harness.Kill(entireProcessTree: true);
                    harness.WaitForExit(5000);
                }
                harness.Dispose();
            }
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                markerPath,
                triggerPath,
                committedMarkerPath,
                markerPath + $".{harnessProcessId}.tmp",
                committedMarkerPath +
                    $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}.*.tmp"))
            {
                TryDelete(path);
            }
        }
    }

    private static async Task<RuntimeDeletionRetryAtomicBackupResult>
        ExecuteRuntimeDeletionRetryAtomicBackupAsync(
            string harnessAssembly,
            string baselinePath,
            string socketPath,
            string rootPath,
            RuntimeDeletionRetryAtomicBackupStrategy strategy,
            int iteration)
    {
        var strategyName = strategy.ToString().ToLowerInvariant();
        var statePath =
            $"{rootPath}.{strategyName}.{iteration}.state.json";
        var markerPath =
            $"{rootPath}.{strategyName}.{iteration}.marker";
        var triggerPath = $"{markerPath}.trigger";
        var committedMarkerPath = $"{markerPath}.committed";
        Process? harness = null;
        int? harnessProcessId = null;
        var tempArtifactObserved = false;
        try
        {
            File.Copy(baselinePath, statePath, overwrite: true);
            File.Copy(
                baselinePath + ".bak",
                statePath + ".bak",
                overwrite: true);
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                "runtime-atomic-backup-unused",
                "retry_rollover_persist");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            if (strategy ==
                RuntimeDeletionRetryAtomicBackupStrategy
                    .DuringBackupTempWrite)
            {
                tempArtifactObserved =
                    await WaitForBackupTempArtifactAsync(
                        harness,
                        statePath,
                        triggerPath);
            }
            else if (strategy ==
                RuntimeDeletionRetryAtomicBackupStrategy.AfterCommit)
            {
                File.WriteAllText(triggerPath, "start\n");
                await WaitForMarkerAsync(
                    harness,
                    committedMarkerPath);
            }

            Assert.False(harness.HasExited);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            File.WriteAllText(statePath, "{");
            var primaryWasCorrupted =
                File.ReadAllText(statePath) == "{";
            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                    })
                .Build();
            var stateStore = new ControlPlaneStateStore(
                configuration,
                new CrashTestEnvironment(
                    Path.GetDirectoryName(statePath)!),
                NullLogger<ControlPlaneStateStore>.Instance);
            var reloaded = new RegistryService(
                stateStore,
                new InMemoryOrchestraRunStore());
            var requestIds = reloaded
                .ListRuntimeDeletionRetryAudit()
                .Select(static audit => audit.RequestId)
                .ToArray();
            var previousWindow = Enumerable.Range(0, 256)
                .Reverse()
                .Select(index =>
                    $"retry-atomic-rollover-{index:D3}")
                .ToArray();
            var completePreviousWindowRestored =
                requestIds.SequenceEqual(previousWindow);
            Assert.True(completePreviousWindowRestored);
            Assert.Equal(256, requestIds.Distinct().Count());
            Assert.Equal(
                ControlPlaneStateLoadSource.Backup,
                stateStore.LoadProvenance.Source);
            Assert.Equal(
                ControlPlaneStateLoadOutcome.Recovered,
                stateStore.LoadProvenance.Outcome);
            Assert.True(stateStore.LoadProvenance.Degraded);
            Assert.Equal(
                ControlPlaneStateLoadFailureCode.InvalidJson,
                stateStore.LoadProvenance.PrimaryFailureCode);
            Assert.Null(
                stateStore.LoadProvenance.BackupFailureCode);

            return new RuntimeDeletionRetryAtomicBackupResult(
                strategy,
                tempArtifactObserved,
                primaryWasCorrupted,
                completePreviousWindowRestored,
                stateStore.LoadProvenance,
                requestIds.Length);
        }
        finally
        {
            if (harness is not null)
            {
                if (!harness.HasExited)
                {
                    harness.Kill(entireProcessTree: true);
                    harness.WaitForExit(5000);
                }
                harness.Dispose();
            }
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                markerPath,
                triggerPath,
                committedMarkerPath,
                markerPath + $".{harnessProcessId}.tmp",
                committedMarkerPath +
                    $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}*.tmp"))
            {
                TryDelete(path);
            }
        }
    }

    private static async Task<RuntimeDeletionRetryPostRecoveryWriteResult>
        ExecuteRuntimeDeletionRetryPostRecoveryWriteAsync(
            string harnessAssembly,
            string baselinePath,
            string socketPath,
            string rootPath,
            ControlPlaneStateLoadFailureCode primaryFailureCode,
            RuntimeDeletionRetryPostRecoveryWriteStrategy strategy,
            int iteration)
    {
        var strategyName = strategy.ToString().ToLowerInvariant();
        var statePath =
            $"{rootPath}.{strategyName}.{iteration}.state.json";
        var markerPath =
            $"{rootPath}.{strategyName}.{iteration}.marker";
        var triggerPath = $"{markerPath}.trigger";
        var committedMarkerPath = $"{markerPath}.committed";
        Process? harness = null;
        int? harnessProcessId = null;
        var primaryTempArtifactObserved = false;
        try
        {
            WritePostRecoveryInvalidPrimary(
                statePath,
                baselinePath,
                primaryFailureCode);
            File.Copy(
                baselinePath + ".bak",
                statePath + ".bak",
                overwrite: true);
            harness = StartCrashHarness(
                harnessAssembly,
                statePath,
                socketPath,
                markerPath,
                "runtime-post-recovery-write-unused",
                "retry_rollover_persist");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            if (strategy ==
                RuntimeDeletionRetryPostRecoveryWriteStrategy
                    .DuringPrimaryTempWrite)
            {
                primaryTempArtifactObserved =
                    await WaitForStateTempArtifactAsync(
                        harness,
                        statePath,
                        triggerPath);
            }
            else if (strategy ==
                RuntimeDeletionRetryPostRecoveryWriteStrategy
                    .AfterCommit)
            {
                File.WriteAllText(triggerPath, "start\n");
                await WaitForMarkerAsync(
                    harness,
                    committedMarkerPath);
            }

            Assert.False(harness.HasExited);
            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            var backupTempArtifactAbsent = !Directory.EnumerateFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}.bak.*.tmp").Any();
            var activeConfiguration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                    })
                .Build();
            var activeStateStore = new ControlPlaneStateStore(
                activeConfiguration,
                new CrashTestEnvironment(
                    Path.GetDirectoryName(statePath)!),
                NullLogger<ControlPlaneStateStore>.Instance);
            var activeRegistry = new RegistryService(
                activeStateStore,
                new InMemoryOrchestraRunStore());
            var activeRequestIds = activeRegistry
                .ListRuntimeDeletionRetryAudit()
                .Select(static audit => audit.RequestId)
                .ToArray();
            var previousWindow = Enumerable.Range(0, 256)
                .Reverse()
                .Select(index =>
                    $"retry-atomic-rollover-{index:D3}")
                .ToArray();
            var replacementWindow = Enumerable.Range(1, 256)
                .Reverse()
                .Select(index =>
                    $"retry-atomic-rollover-{index:D3}")
                .ToArray();
            var activeWindow = activeRequestIds.SequenceEqual(
                previousWindow)
                ? RuntimeDeletionRetryAtomicRolloverWindow.Previous
                : activeRequestIds.SequenceEqual(replacementWindow)
                    ? RuntimeDeletionRetryAtomicRolloverWindow.Replacement
                    : RuntimeDeletionRetryAtomicRolloverWindow.Torn;
            Assert.NotEqual(
                RuntimeDeletionRetryAtomicRolloverWindow.Torn,
                activeWindow);

            var backupPath = statePath + ".bak";
            var backupConfiguration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = backupPath,
                    })
                .Build();
            var backupRegistry = new RegistryService(
                new ControlPlaneStateStore(
                    backupConfiguration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(backupPath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            var backupRequestIds = backupRegistry
                .ListRuntimeDeletionRetryAudit()
                .Select(static audit => audit.RequestId)
                .ToArray();
            var backupWindowPreserved =
                backupRequestIds.SequenceEqual(previousWindow);

            var expectedActiveSource = activeWindow ==
                RuntimeDeletionRetryAtomicRolloverWindow.Previous
                ? ControlPlaneStateLoadSource.Backup
                : ControlPlaneStateLoadSource.Primary;
            var expectedActiveOutcome = activeWindow ==
                RuntimeDeletionRetryAtomicRolloverWindow.Previous
                ? ControlPlaneStateLoadOutcome.Recovered
                : ControlPlaneStateLoadOutcome.Clean;
            Assert.Equal(
                expectedActiveSource,
                activeStateStore.LoadProvenance.Source);
            Assert.Equal(
                expectedActiveOutcome,
                activeStateStore.LoadProvenance.Outcome);
            if (activeWindow ==
                RuntimeDeletionRetryAtomicRolloverWindow.Previous)
            {
                Assert.Equal(
                    primaryFailureCode,
                    activeStateStore.LoadProvenance
                        .PrimaryFailureCode);
            }
            Assert.True(backupWindowPreserved);

            return new RuntimeDeletionRetryPostRecoveryWriteResult(
                strategy,
                activeWindow,
                primaryTempArtifactObserved,
                backupTempArtifactAbsent,
                backupWindowPreserved,
                activeStateStore.LoadProvenance,
                activeRequestIds.Length,
                backupRequestIds.Length);
        }
        finally
        {
            if (harness is not null)
            {
                if (!harness.HasExited)
                {
                    harness.Kill(entireProcessTree: true);
                    harness.WaitForExit(5000);
                }
                harness.Dispose();
            }
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                markerPath,
                triggerPath,
                committedMarkerPath,
                markerPath + $".{harnessProcessId}.tmp",
                committedMarkerPath +
                    $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
            foreach (var path in Directory.GetFiles(
                Path.GetDirectoryName(statePath)!,
                $"{Path.GetFileName(statePath)}*.tmp"))
            {
                TryDelete(path);
            }
        }
    }

    private static async Task<bool> WaitForStateTempArtifactAsync(
        Process harness,
        string statePath,
        string triggerPath) =>
        await WaitForTempArtifactAsync(
            harness,
            Path.GetDirectoryName(statePath)!,
            $"{Path.GetFileName(statePath)}.*.tmp",
            triggerPath,
            "state",
            path => !path.StartsWith(
                statePath + ".bak.",
                StringComparison.Ordinal));

    private static async Task<bool> WaitForBackupTempArtifactAsync(
        Process harness,
        string statePath,
        string triggerPath) =>
        await WaitForTempArtifactAsync(
            harness,
            Path.GetDirectoryName(statePath)!,
            $"{Path.GetFileName(statePath)}.bak.*.tmp",
            triggerPath,
            "backup");

    private static async Task<bool> WaitForTempArtifactAsync(
        Process harness,
        string directory,
        string filter,
        string triggerPath,
        string artifactKind,
        Func<string, bool>? pathPredicate = null)
    {
        var observed = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        using var watcher = new FileSystemWatcher(
            directory,
            filter)
        {
            EnableRaisingEvents = true,
            NotifyFilter = NotifyFilters.FileName,
        };
        watcher.Created += (_, eventArgs) =>
        {
            if (pathPredicate?.Invoke(eventArgs.FullPath) is not false)
            {
                observed.TrySetResult();
            }
        };
        File.WriteAllText(triggerPath, "start\n");
        try
        {
            await observed.Task.WaitAsync(TimeSpan.FromSeconds(5));
            return true;
        }
        catch (TimeoutException) when (harness.HasExited)
        {
            throw new InvalidOperationException(
                $"atomic rollover harness exited before creating its {artifactKind} temp file: {await harness.StandardError.ReadToEndAsync()}");
        }
    }

    private static async Task<RuntimeDeletionRetryCrashScenarioResult>
        ExecuteRuntimeDeletionRetryCrashScenarioAsync(
            string harnessAssembly,
            string socketPath,
            DaemonRuntimeRegistrationAuthority authority,
            string phase,
            int iteration)
    {
        var phaseSlug = phase.Replace('_', '-');
        var runtimeId = $"runtime-retry-crash-{phaseSlug}-{iteration}";
        var statePath = $"{socketPath}.{phase}.{iteration}.retry.state.json";
        var markerPath = $"{socketPath}.{phase}.{iteration}.retry.marker";
        var requestId = $"retry-crash-{runtimeId}";
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
            var daemonCommitted = string.Equals(
                phase,
                "retry_daemon_committed",
                StringComparison.Ordinal);
            Assert.Equal(!daemonCommitted, daemonRuntime is not null);
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
            var pendingIntent = Assert.Single(
                restarted.ListPendingRuntimeDeletions());
            Assert.Equal(runtimeId, Assert.Single(pendingIntent.RuntimeIds));
            Assert.Equal(1, pendingIntent.AttemptCount);
            Assert.Equal(3, pendingIntent.Revision);
            Assert.Equal(
                "authority_unavailable",
                pendingIntent.LastFailureCode);
            Assert.NotNull(restarted.GetRuntime(runtimeId));
            Assert.Equal(
                runtimeId,
                restarted.CreateSession(new SessionCreateRequest(
                    runtimeId,
                    "diagnostic",
                    "retry-crash-recovery-test",
                    Array.Empty<SessionCapabilityRequirement>()))
                    .RuntimeMissing);

            var audit = Assert.Single(
                restarted.ListRuntimeDeletionRetryAudit());
            Assert.Equal(requestId, audit.RequestId);
            Assert.Equal(pendingIntent.IntentId, audit.IntentId);
            Assert.Equal(2, audit.ExpectedRevision);
            Assert.Equal(3, audit.ResultingRevision);
            Assert.Equal("crash-harness", audit.RequestedBy);

            var countingAuthority =
                new CountingRuntimeDeletionAuthority(authority);
            recovery = new RuntimeDeletionRecoveryService(
                restarted,
                countingAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            await recovery.StartAsync(CancellationToken.None);
            await WaitForDeletionRecoveryAsync(restarted, runtimeId);
            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            recovery = null;

            Assert.Equal(1, countingAuthority.UnregisterCallCount);
            Assert.Empty(restarted.ListPendingRuntimeDeletions());
            Assert.Null(restarted.GetRuntime(runtimeId));
            Assert.Null(await authority.InspectAsync(
                runtimeId,
                CancellationToken.None));
            Assert.Single(restarted.ListRuntimeDeletionRetryAudit());

            var replay = restarted.RetryRuntimeDeletionNow(
                pendingIntent.IntentId,
                new RuntimeDeletionRetryNowRequest(
                    2,
                    requestId,
                    "crash-harness"));
            Assert.True(replay.Accepted);
            Assert.True(replay.Replayed);
            Assert.Null(replay.PendingIntent);

            var diskReloaded = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            Assert.Empty(diskReloaded.ListPendingRuntimeDeletions());
            Assert.Null(diskReloaded.GetRuntime(runtimeId));
            Assert.Single(diskReloaded.ListRuntimeDeletionRetryAudit());

            return new RuntimeDeletionRetryCrashScenarioResult(
                daemonCommitted,
                countingAuthority.UnregisterCallCount);
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
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                statePath + ".tmp",
                markerPath,
                markerPath + $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
        }
    }

    private static async Task<RuntimeDeletionLostAcknowledgementResult>
        ExecuteRuntimeDeletionLostAcknowledgementScenarioAsync(
            string harnessAssembly,
            string socketPath,
            DaemonRuntimeRegistrationAuthority authority,
            int iteration,
            bool evictReceipt = false)
    {
        var runtimeId =
            $"runtime-lost-ack-{iteration}";
        var statePath =
            $"{socketPath}.lost-ack.{iteration}.state.json";
        var markerPath =
            $"{socketPath}.lost-ack.{iteration}.marker";
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
                "lost_ack_daemon_committed");
            harnessProcessId = harness.Id;
            await WaitForMarkerAsync(harness, markerPath);

            var marker = (await File.ReadAllTextAsync(markerPath))
                .Trim()
                .Split(
                    ' ',
                    StringSplitOptions.RemoveEmptyEntries);
            Assert.Equal(2, marker.Length);
            var intentId = marker[0];
            var commandId = marker[1];
            var receiptBeforeTermination = await authority
                .LookupUnregistrationReceiptAsync(
                    commandId,
                    CancellationToken.None);
            Assert.True(receiptBeforeTermination.Found);
            Assert.Equal(
                new[] { runtimeId },
                receiptBeforeTermination.RuntimeIds);
            Assert.NotNull(
                receiptBeforeTermination.OperationGeneration);
            Assert.Null(await authority.InspectAsync(
                runtimeId,
                CancellationToken.None));
            Assert.False(harness.HasExited);

            harness.Kill(entireProcessTree: true);
            Assert.True(harness.WaitForExit(5000));
            Assert.NotEqual(0, harness.ExitCode);

            RuntimeUnregistrationReceiptLookup? evictedLookup = null;
            if (evictReceipt)
            {
                await EvictRuntimeUnregistrationReceiptAsync(
                    authority,
                    iteration);
                evictedLookup = await authority
                    .LookupUnregistrationReceiptAsync(
                        commandId,
                        CancellationToken.None);
                Assert.False(evictedLookup.Found);
                Assert.NotNull(evictedLookup.ReplayHorizon);
                Assert.True(
                    evictedLookup.ReplayHorizon
                        .EvictedThroughGeneration >=
                    receiptBeforeTermination.OperationGeneration);
            }

            var configuration = new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                    })
                .Build();
            var restarted = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            var pendingIntent = Assert.Single(
                restarted.ListPendingRuntimeDeletions());
            Assert.Equal(intentId, pendingIntent.IntentId);
            Assert.Equal(
                commandId,
                pendingIntent.UnregistrationCommandId);
            Assert.Equal(0, pendingIntent.AttemptCount);
            Assert.Equal(2, pendingIntent.Revision);
            Assert.True(
                pendingIntent.UnregistrationMutationMayHaveStarted);
            Assert.Equal(
                receiptBeforeTermination.OperationGeneration,
                pendingIntent.UnregistrationReplayHorizonFloor);
            Assert.NotNull(restarted.GetRuntime(runtimeId));
            Assert.Equal(
                runtimeId,
                restarted.CreateSession(
                    new SessionCreateRequest(
                        runtimeId,
                        "diagnostic",
                        "lost-ack-recovery-test",
                        Array.Empty<
                            SessionCapabilityRequirement>()))
                    .RuntimeMissing);

            var countingAuthority =
                new ReceiptLookupCountingRuntimeDeletionAuthority(
                    authority);
            recovery = new RuntimeDeletionRecoveryService(
                restarted,
                countingAuthority,
                NullLogger<RuntimeDeletionRecoveryService>.Instance);
            await recovery.StartAsync(CancellationToken.None);
            if (evictReceipt)
            {
                var deadline = DateTimeOffset.UtcNow.AddSeconds(10);
                while (DateTimeOffset.UtcNow < deadline &&
                    restarted.ListPendingRuntimeDeletions()
                        .Single().AttemptCount == 0)
                {
                    await Task.Delay(10);
                }
            }
            else
            {
                await WaitForDeletionRecoveryAsync(
                    restarted,
                    runtimeId);
            }
            await recovery.StopAsync(CancellationToken.None);
            recovery.Dispose();
            recovery = null;

            Assert.Equal(1, countingAuthority.LookupCallCount);
            Assert.Equal(0, countingAuthority.MutationCallCount);
            Assert.Equal(
                commandId,
                countingAuthority.LastLookup?.CommandId);
            Assert.Equal(
                !evictReceipt,
                countingAuthority.LastLookup?.Found);
            if (evictReceipt)
            {
                var ambiguous = Assert.Single(
                    restarted.ListPendingRuntimeDeletions());
                Assert.Equal(1, ambiguous.AttemptCount);
                Assert.Equal(
                    RuntimeDeletionFailureCodes.ReplayAmbiguous,
                    ambiguous.LastFailureCode);
                Assert.Equal(3, ambiguous.Revision);
                Assert.NotNull(restarted.GetRuntime(runtimeId));
            }
            else
            {
                Assert.Equal(
                    receiptBeforeTermination.OperationGeneration,
                    countingAuthority.LastLookup?.OperationGeneration);
                Assert.Empty(
                    restarted.ListPendingRuntimeDeletions());
                Assert.Null(restarted.GetRuntime(runtimeId));
            }

            var receiptAfterRecovery = await authority
                .LookupUnregistrationReceiptAsync(
                    commandId,
                    CancellationToken.None);
            Assert.Equal(!evictReceipt, receiptAfterRecovery.Found);
            if (!evictReceipt)
            {
                Assert.Equal(
                    receiptBeforeTermination.OperationGeneration,
                    receiptAfterRecovery.OperationGeneration);
            }
            Assert.Null(await authority.InspectAsync(
                runtimeId,
                CancellationToken.None));

            var diskReloaded = new RegistryService(
                new ControlPlaneStateStore(
                    configuration,
                    new CrashTestEnvironment(
                        Path.GetDirectoryName(statePath)!),
                    NullLogger<ControlPlaneStateStore>.Instance),
                new InMemoryOrchestraRunStore());
            var reappearedIdentityBlockedReconciliation = false;
            var reconciliationDaemonRevision = 0UL;
            var reconciliationAuditSurvivedReload = false;
            var reconciliationReplayedAfterRestart = false;
            if (evictReceipt)
            {
                var persistedAmbiguous = Assert.Single(
                    diskReloaded.ListPendingRuntimeDeletions());
                Assert.Equal(
                    RuntimeDeletionFailureCodes.ReplayAmbiguous,
                    persistedAmbiguous.LastFailureCode);
                Assert.NotNull(
                    diskReloaded.GetRuntime(runtimeId));

                await authority.RegisterAsync(
                    new RuntimeRegistrationRequest(
                        "Reappeared lost-ack runtime",
                        "http://127.0.0.1:19091",
                        "pairing-token"),
                    runtimeId,
                    CancellationToken.None);
                var reappearedSnapshot =
                    await authority.SnapshotAsync(
                        CancellationToken.None);
                Assert.Contains(
                    reappearedSnapshot.Runtimes,
                    runtime => runtime.RuntimeId == runtimeId);
                var reconcileRequest =
                    new RuntimeDeletionReconcileRequest(
                        persistedAmbiguous.Revision,
                        reappearedSnapshot.Revision,
                        "reconcile-lost-ack-evicted",
                        "real-daemon-campaign",
                        true);
                var blockedStart =
                    diskReloaded
                        .BeginRuntimeDeletionReconciliation(
                            persistedAmbiguous.IntentId,
                            reconcileRequest);
                using (var blockedReservation =
                    blockedStart.Reservation!)
                {
                    var blocked = Assert.Throws<
                        RuntimeDeletionReconciliationException>(() =>
                            diskReloaded
                                .CompleteRuntimeDeletionReconciliation(
                                    blockedReservation,
                                    reconcileRequest,
                                    reappearedSnapshot));
                    Assert.Equal(
                        "runtime_deletion_reconciliation_target_reappeared",
                        blocked.Code);
                    reappearedIdentityBlockedReconciliation = true;
                }
                Assert.NotNull(
                    diskReloaded.GetRuntime(runtimeId));
                Assert.Single(
                    diskReloaded.ListPendingRuntimeDeletions());
                Assert.Empty(
                    diskReloaded
                        .ListRuntimeDeletionReconciliationAudit());

                await authority.UnregisterAsync(
                    new[] { runtimeId },
                    "reconcile-remove-reappeared-runtime",
                    CancellationToken.None);
                var absentSnapshot =
                    await authority.SnapshotAsync(
                        CancellationToken.None);
                Assert.DoesNotContain(
                    absentSnapshot.Runtimes,
                    runtime => runtime.RuntimeId == runtimeId);
                Assert.True(
                    absentSnapshot.Revision >
                    reappearedSnapshot.Revision);
                reconciliationDaemonRevision =
                    absentSnapshot.Revision;
                var convergingRequest = reconcileRequest with
                {
                    ExpectedDaemonRevision =
                        absentSnapshot.Revision,
                };
                var convergingStart =
                    diskReloaded
                        .BeginRuntimeDeletionReconciliation(
                            persistedAmbiguous.IntentId,
                            convergingRequest);
                using (var convergingReservation =
                    convergingStart.Reservation!)
                {
                    var reconciled =
                        diskReloaded
                            .CompleteRuntimeDeletionReconciliation(
                                convergingReservation,
                                convergingRequest,
                                absentSnapshot);
                    Assert.True(reconciled.Accepted);
                    Assert.False(reconciled.Replayed);
                }
                Assert.Null(
                    diskReloaded.GetRuntime(runtimeId));
                Assert.Empty(
                    diskReloaded.ListPendingRuntimeDeletions());
                Assert.Single(
                    diskReloaded
                        .ListRuntimeDeletionReconciliationAudit());

                var reconciledReload = new RegistryService(
                    new ControlPlaneStateStore(
                        configuration,
                        new CrashTestEnvironment(
                            Path.GetDirectoryName(statePath)!),
                        NullLogger<
                            ControlPlaneStateStore>.Instance),
                    new InMemoryOrchestraRunStore());
                Assert.Null(
                    reconciledReload.GetRuntime(runtimeId));
                Assert.Empty(
                    reconciledReload
                        .ListPendingRuntimeDeletions());
                var restoredAudit = Assert.Single(
                    reconciledReload
                        .ListRuntimeDeletionReconciliationAudit());
                Assert.Equal(
                    absentSnapshot.Revision,
                    restoredAudit.DaemonRevision);
                reconciliationAuditSurvivedReload = true;

                var replayed =
                    reconciledReload
                        .BeginRuntimeDeletionReconciliation(
                            persistedAmbiguous.IntentId,
                            convergingRequest);
                Assert.Null(replayed.Reservation);
                Assert.True(replayed.Replay?.Replayed);
                reconciliationReplayedAfterRestart = true;
            }
            else
            {
                Assert.Empty(
                    diskReloaded.ListPendingRuntimeDeletions());
                Assert.Null(diskReloaded.GetRuntime(runtimeId));
            }

            return new RuntimeDeletionLostAcknowledgementResult(
                countingAuthority.LookupCallCount,
                countingAuthority.MutationCallCount,
                receiptBeforeTermination.OperationGeneration!.Value,
                pendingIntent
                    .UnregistrationReplayHorizonFloor!.Value,
                evictedLookup?.ReplayHorizon
                    ?.EvictedThroughGeneration,
                evictReceipt,
                reappearedIdentityBlockedReconciliation,
                reconciliationDaemonRevision,
                reconciliationAuditSurvivedReload,
                reconciliationReplayedAfterRestart);
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
            foreach (var path in new[]
            {
                statePath,
                statePath + ".bak",
                statePath + ".tmp",
                markerPath,
                markerPath + $".{harnessProcessId}.tmp",
            })
            {
                TryDelete(path);
            }
        }
    }

    private static async Task EvictRuntimeUnregistrationReceiptAsync(
        DaemonRuntimeRegistrationAuthority authority,
        int iteration)
    {
        const int replayHorizonCapacity = 256;
        for (var index = 0;
             index < replayHorizonCapacity;
             index += 1)
        {
            var runtimeId =
                $"runtime-horizon-{iteration}-{index}";
            var request = new RuntimeRegistrationRequest(
                $"Horizon Runtime {index}",
                $"http://127.0.0.1:{20000 + index}",
                "pairing-token");
            await authority.RegisterAsync(
                request,
                runtimeId,
                CancellationToken.None);
            await authority.UnregisterAsync(
                new[] { runtimeId },
                $"horizon-eviction-{iteration}-{index}",
                CancellationToken.None);
        }
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

    private static string FindCrashHarnessAssembly() =>
        CrashHarnessAssemblyLocator.Find();

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
        string runtimeId) =>
        await WaitForDeletionRecoveryAsync(registry, new[] { runtimeId });

    private static async Task WaitForDeletionRecoveryAsync(
        RegistryService registry,
        IReadOnlyCollection<string> runtimeIds)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(10);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (registry.ListPendingRuntimeDeletions().Count == 0 &&
                runtimeIds.All(runtimeId => registry.GetRuntime(runtimeId) is null))
            {
                return;
            }
            await Task.Delay(10);
        }
        throw new TimeoutException("runtime deletion intent did not converge after restart");
    }

    private static async Task WaitForPendingDeletionCountAsync(
        RegistryService registry,
        int expectedCount)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(10);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (registry.ListPendingRuntimeDeletions().Count == expectedCount)
            {
                return;
            }
            await Task.Delay(10);
        }
        throw new TimeoutException(
            $"runtime deletion intent count did not converge to {expectedCount}");
    }

    private static async Task WaitForStatePersistenceRollbackAsync(
        ControlPlaneStateStore stateStore,
        RegistryService registry,
        IReadOnlyCollection<string> runtimeIds)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(5);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (stateStore.IsDirty &&
                stateStore.LastSaveError is not null &&
                registry.ListPendingRuntimeDeletions().Count == runtimeIds.Count &&
                runtimeIds.All(runtimeId => registry.GetRuntime(runtimeId) is not null))
            {
                return;
            }
            await Task.Delay(10);
        }
        throw new TimeoutException(
            "runtime deletion batch did not restore local state after strict persistence failure");
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

    private static void WriteRuntimeDeletionRetryCrashEvidenceIfRequested(
        int iterations,
        IReadOnlyList<RuntimeDeletionRetryCrashScenarioResult> results)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_RETRY_CRASH_EVIDENCE");
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
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_phase = iterations,
            phases = RuntimeDeletionRetryCrashPhases,
            total_forced_terminations = results.Count,
            daemon_committed_before_termination_count = results.Count(
                static result => result.DaemonCommittedBeforeTermination),
            recovery_authority_call_count = results.Sum(
                static result => result.RecoveryAuthorityCallCount),
            checks = new
            {
                real_leserpentd = true,
                retry_acknowledgement_boundary_covered = true,
                retry_daemon_commit_boundary_covered = true,
                every_host_process_force_killed = true,
                every_revision_and_audit_restored = true,
                every_pending_runtime_remained_protected = true,
                committed_mutation_replayed_idempotently = true,
                exactly_one_recovery_authority_call_per_scenario =
                    results.All(static result =>
                        result.RecoveryAuthorityCallCount == 1),
                every_retry_request_replayed_after_convergence = true,
                every_daemon_and_compatibility_state_converged = true,
                every_audit_survived_convergence_and_reload = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void
        WriteRuntimeDeletionRetryAtomicRolloverEvidenceIfRequested(
            int iterations,
            IReadOnlyList<RuntimeDeletionRetryAtomicRolloverResult> results)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_RETRY_ATOMIC_ROLLOVER_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var previousWindowCount = results.Count(result =>
            result.Window ==
            RuntimeDeletionRetryAtomicRolloverWindow.Previous);
        var replacementWindowCount = results.Count(result =>
            result.Window ==
            RuntimeDeletionRetryAtomicRolloverWindow.Replacement);
        var duringTempResults = results.Where(result =>
            result.Strategy ==
            RuntimeDeletionRetryAtomicRolloverStrategy.DuringTempWrite)
            .ToArray();
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_strategy = iterations,
            strategies = Enum.GetNames<
                RuntimeDeletionRetryAtomicRolloverStrategy>(),
            total_forced_terminations = results.Count,
            audit_retention_limit = 256,
            runtime_ids_per_audit_record = 128,
            previous_window_count = previousWindowCount,
            replacement_window_count = replacementWindowCount,
            temp_artifact_observed_count = duringTempResults.Count(
                static result => result.TempArtifactObserved),
            checks = new
            {
                before_write_restored_complete_previous_window =
                    results
                        .Where(result =>
                            result.Strategy ==
                            RuntimeDeletionRetryAtomicRolloverStrategy
                                .BeforeWrite)
                        .All(result =>
                            result.Window ==
                            RuntimeDeletionRetryAtomicRolloverWindow
                                .Previous),
                every_temp_write_was_observed =
                    duringTempResults.All(static result =>
                        result.TempArtifactObserved),
                after_commit_restored_complete_replacement_window =
                    results
                        .Where(result =>
                            result.Strategy ==
                            RuntimeDeletionRetryAtomicRolloverStrategy
                                .AfterCommit)
                        .All(result =>
                            result.Window ==
                            RuntimeDeletionRetryAtomicRolloverWindow
                                .Replacement),
                every_restart_loaded_exactly_256_records =
                    results.All(static result =>
                        result.AuditCount == 256),
                every_restart_observed_old_or_new_window =
                    previousWindowCount + replacementWindowCount ==
                    results.Count,
                no_torn_or_reordered_window = results.All(result =>
                    result.Window !=
                    RuntimeDeletionRetryAtomicRolloverWindow.Torn),
                both_atomic_outcomes_were_exercised =
                    previousWindowCount > 0 &&
                    replacementWindowCount > 0,
                every_host_process_force_killed = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void
        WriteRuntimeDeletionReconciliationCommitEvidenceIfRequested(
            int iterations,
            ulong daemonRevision,
            IReadOnlyList<
                RuntimeDeletionReconciliationCommitResult> results)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_RECONCILIATION_COMMIT_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(
            Path.GetDirectoryName(evidencePath)!);
        var previousGenerationCount = results.Count(result =>
            result.Window ==
            RuntimeDeletionReconciliationCommitWindow.Previous);
        var replacementGenerationCount = results.Count(result =>
            result.Window ==
            RuntimeDeletionReconciliationCommitWindow.Replacement);
        var duringTempResults = results.Where(result =>
            result.Strategy ==
            RuntimeDeletionReconciliationCommitStrategy.DuringTempWrite)
            .ToArray();
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture =
                RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_strategy = iterations,
            strategies = Enum.GetNames<
                RuntimeDeletionReconciliationCommitStrategy>(),
            total_forced_terminations = results.Count,
            daemon_revision = daemonRevision,
            retry_audit_retention_limit = 256,
            previous_generation_count = previousGenerationCount,
            replacement_generation_count =
                replacementGenerationCount,
            temp_artifact_observed_count = duringTempResults.Count(
                static result => result.TempArtifactObserved),
            checks = new
            {
                real_leserpentd_snapshot_used =
                    daemonRevision > 0,
                before_write_restored_complete_previous_generation =
                    results
                        .Where(result =>
                            result.Strategy ==
                            RuntimeDeletionReconciliationCommitStrategy
                                .BeforeWrite)
                        .All(result =>
                            result.Window ==
                            RuntimeDeletionReconciliationCommitWindow
                                .Previous),
                every_temp_write_was_observed =
                    duringTempResults.All(static result =>
                        result.TempArtifactObserved),
                after_commit_restored_complete_replacement_generation =
                    results
                        .Where(result =>
                            result.Strategy ==
                            RuntimeDeletionReconciliationCommitStrategy
                                .AfterCommit)
                        .All(result =>
                            result.Window ==
                            RuntimeDeletionReconciliationCommitWindow
                                .Replacement),
                every_restart_observed_old_or_new_generation =
                    previousGenerationCount +
                    replacementGenerationCount ==
                    results.Count,
                no_torn_runtime_session_intent_or_audit_generation =
                    results.All(result =>
                        result.Window !=
                        RuntimeDeletionReconciliationCommitWindow.Torn),
                every_previous_generation_retry_converged =
                    results
                        .Where(result =>
                            result.Window ==
                            RuntimeDeletionReconciliationCommitWindow
                                .Previous)
                        .All(static result =>
                            result.FinalStateConverged),
                every_reconciliation_audit_survived_reload =
                    results.All(static result =>
                        result.ReconciliationAuditSurvivedReload),
                every_request_replayed_after_restart =
                    results.All(static result =>
                        result.RequestReplayedAfterRestart),
                every_restart_preserved_retry_audit_window =
                    results.All(static result =>
                        result.RetryAuditCount == 256),
                every_final_state_converged =
                    results.All(static result =>
                        result.FinalStateConverged),
                both_atomic_outcomes_were_exercised =
                    previousGenerationCount > 0 &&
                    replacementGenerationCount > 0,
                every_host_process_force_killed = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) +
            "\n");
    }

    private static void
        WriteRuntimeDeletionCrossAuthorityEvidenceIfRequested(
            int iterations,
            ulong daemonRevision,
            IReadOnlyList<RuntimeDeletionCrossAuthorityResult> results,
            IReadOnlyList<CleanupCheckpointRaceResult>
                cleanupCheckpointRaces,
            AuditCheckpointDaemonRestartResult
                auditCheckpointDaemonRestart)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_CROSS_AUTHORITY_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(
            Path.GetDirectoryName(evidencePath)!);
        var previousGenerationCount = results.Count(result =>
            result.Window ==
            RuntimeDeletionReconciliationCommitWindow.Previous);
        var replacementGenerationCount = results.Count(result =>
            result.Window ==
            RuntimeDeletionReconciliationCommitWindow.Replacement);
        var duringTempResults = results.Where(result =>
            result.Strategy ==
            RuntimeDeletionCrossAuthorityStrategy
                .DuringControlTempWrite)
            .ToArray();
        var evidence = new
        {
            schema_version = 3,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture =
                RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_strategy = iterations,
            strategies = Enum.GetNames<
                RuntimeDeletionCrossAuthorityStrategy>(),
            total_forced_terminations = results.Count,
            daemon_revision = daemonRevision,
            previous_generation_count = previousGenerationCount,
            replacement_generation_count =
                replacementGenerationCount,
            control_temp_artifact_observed_count =
                duringTempResults.Count(
                    static result =>
                        result.TempArtifactObserved),
            cleanup_checkpoint_race_rounds =
                cleanupCheckpointRaces.Count,
            cleanup_checkpoint_races =
                cleanupCheckpointRaces.Select(result => new
                {
                    order = result.Order.ToString(),
                    available_before_race =
                        result.AvailableBeforeRace,
                    checkpoint_generation =
                        result.CheckpointGeneration,
                    cleanup_generation =
                        result.CleanupGeneration,
                }),
            audit_checkpoint_daemon_restart = new
            {
                checkpoint_lag_before_daemon_restart =
                    auditCheckpointDaemonRestart
                        .CheckpointLagBeforeDaemonRestart,
                checkpoint_lag_after_daemon_restart =
                    auditCheckpointDaemonRestart
                        .CheckpointLagAfterDaemonRestart,
                audit_generation =
                    auditCheckpointDaemonRestart.AuditGeneration,
                checkpointed_through_generation =
                    auditCheckpointDaemonRestart
                        .CheckpointedThroughGeneration,
            },
            checks = new
            {
                real_leserpentd_orchestra_authority_used = true,
                orchestra_cleanup_committed_before_every_termination =
                    results.All(static result =>
                        result
                            .TargetHistoryAbsentBeforeTermination),
                target_history_absent_before_every_termination =
                    results.All(static result =>
                        result
                            .TargetHistoryAbsentBeforeTermination),
                unrelated_run_and_event_preserved =
                    results.All(static result =>
                        result.UnrelatedHistoryPreserved),
                after_orchestra_cleanup_restored_previous_control_generation =
                    results
                        .Where(result =>
                            result.Strategy ==
                            RuntimeDeletionCrossAuthorityStrategy
                                .AfterOrchestraCleanup)
                        .All(result =>
                            result.Window ==
                            RuntimeDeletionReconciliationCommitWindow
                                .Previous),
                every_control_temp_write_was_observed =
                    duringTempResults.All(static result =>
                        result.TempArtifactObserved),
                after_control_commit_restored_replacement_generation =
                    results
                        .Where(result =>
                            result.Strategy ==
                            RuntimeDeletionCrossAuthorityStrategy
                                .AfterControlCommit)
                        .All(result =>
                            result.Window ==
                            RuntimeDeletionReconciliationCommitWindow
                                .Replacement),
                every_restart_observed_old_or_new_control_generation =
                    previousGenerationCount +
                    replacementGenerationCount ==
                    results.Count,
                no_torn_control_generation =
                    results.All(result =>
                        result.Window !=
                        RuntimeDeletionReconciliationCommitWindow.Torn),
                every_previous_generation_retried_absent_target_cleanup =
                    results
                        .Where(result =>
                            result.Window ==
                            RuntimeDeletionReconciliationCommitWindow
                                .Previous)
                        .All(static result =>
                            result.FinalStateConverged),
                every_final_state_converged =
                    results.All(static result =>
                        result.FinalStateConverged),
                every_final_state_retained_one_reconciliation_audit =
                    results.All(static result =>
                        result.SingleAuditSurvivedReload),
                every_request_replayed_after_restart =
                    results.All(static result =>
                        result.RequestReplayedAfterRestart),
                every_cleanup_receipt_replayed_same_generation =
                    results.All(static result =>
                        result
                            .CleanupReceiptReplayedSameGeneration),
                every_audit_checkpoint_protected_cleanup_replay_horizon =
                    results.All(static result =>
                        result
                            .AuditCheckpointProtectedReplayHorizon),
                every_pre_saturation_critical_warning_visible =
                    cleanupCheckpointRaces.All(static result =>
                        result.PreSaturationCriticalVisible),
                cleanup_first_race_exercised =
                    cleanupCheckpointRaces.Any(result =>
                        result.Order ==
                        CleanupCheckpointRaceOrder.CleanupFirst),
                checkpoint_first_race_exercised =
                    cleanupCheckpointRaces.Any(result =>
                        result.Order ==
                        CleanupCheckpointRaceOrder.CheckpointFirst),
                every_raced_cleanup_committed =
                    cleanupCheckpointRaces.All(static result =>
                        result.CleanupCommitted),
                every_raced_checkpoint_committed =
                    cleanupCheckpointRaces.All(static result =>
                        result.CheckpointCommitted),
                every_race_observed_expected_completion_order =
                    cleanupCheckpointRaces.All(static result =>
                        result.ExpectedCompletionOrderObserved),
                every_cleanup_checkpoint_race_admission_safe =
                    cleanupCheckpointRaces.All(static result =>
                        result.FinalHorizonAdmissionSafe),
                audit_driven_checkpoint_advanced_after_daemon_restart =
                    auditCheckpointDaemonRestart.DaemonRestarted &&
                    auditCheckpointDaemonRestart
                        .CheckpointedThroughGeneration ==
                    auditCheckpointDaemonRestart.AuditGeneration,
                checkpoint_lag_was_visible_before_daemon_restart =
                    auditCheckpointDaemonRestart
                        .CheckpointLagBeforeDaemonRestart > 0,
                checkpoint_lag_converged_to_zero_after_daemon_restart =
                    auditCheckpointDaemonRestart
                        .CheckpointLagAfterDaemonRestart == 0,
                automatic_checkpoint_status_reported =
                    auditCheckpointDaemonRestart
                        .AutomaticCheckpointStatusReported,
                both_control_generation_outcomes_were_exercised =
                    previousGenerationCount > 0 &&
                    replacementGenerationCount > 0,
                every_host_process_force_killed = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) +
            "\n");
    }

    private static void
        WriteRuntimeDeletionLostAcknowledgementEvidenceIfRequested(
            int iterations,
            IReadOnlyList<
                RuntimeDeletionLostAcknowledgementResult> results)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_LOST_ACK_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(
            Path.GetDirectoryName(evidencePath)!);
        var evidence = new
        {
            schema_version = 2,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture =
                RuntimeInformation.ProcessArchitecture.ToString(),
            iterations,
            total_forced_host_terminations = results.Count,
            receipt_lookup_call_count = results.Sum(
                static result => result.LookupCallCount),
            post_restart_unregistration_mutation_count = results.Sum(
                static result => result.MutationCallCount),
            minimum_operation_generation = results.Min(
                static result => result.OperationGeneration),
            maximum_operation_generation = results.Max(
                static result => result.OperationGeneration),
            minimum_replay_horizon_floor = results.Min(
                static result => result.ReplayHorizonFloor),
            maximum_replay_horizon_floor = results.Max(
                static result => result.ReplayHorizonFloor),
            checks = new
            {
                real_leserpentd = true,
                schema_v5_command_identity_and_replay_floor_restored =
                    results.All(static result =>
                        result.ReplayHorizonFloor ==
                        result.OperationGeneration),
                daemon_commit_preceded_host_termination = true,
                acknowledgement_withheld_from_recovery_worker = true,
                every_host_process_force_killed = true,
                every_restart_performed_receipt_lookup = results.All(
                    static result => result.LookupCallCount == 1),
                zero_post_restart_unregistration_mutations =
                    results.All(
                        static result =>
                            result.MutationCallCount == 0),
                receipt_generation_stable_across_recovery = true,
                every_daemon_and_compatibility_state_converged = true,
                every_converged_state_survived_disk_reload = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) +
                "\n");
    }

    private static void
        WriteRuntimeDeletionReplayHorizonEvidenceIfRequested(
            RuntimeDeletionLostAcknowledgementResult result)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_REPLAY_HORIZON_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(
            Path.GetDirectoryName(evidencePath)!);
        var evidence = new
        {
            schema_version = 2,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture =
                RuntimeInformation.ProcessArchitecture.ToString(),
            forced_host_termination_count = 1,
            replay_horizon_capacity = 256,
            persisted_replay_horizon_floor =
                result.ReplayHorizonFloor,
            evicted_through_generation =
                result.EvictedThroughGeneration,
            receipt_lookup_call_count = result.LookupCallCount,
            post_restart_unregistration_mutation_count =
                result.MutationCallCount,
            reconciliation_daemon_revision =
                result.ReconciliationDaemonRevision,
            checks = new
            {
                real_leserpentd = true,
                schema_v5_replay_floor_persisted_before_mutation =
                    result.ReplayHorizonFloor ==
                    result.OperationGeneration,
                daemon_commit_preceded_host_termination = true,
                acknowledgement_withheld_from_recovery_worker = true,
                complete_replay_horizon_rollover = true,
                original_receipt_was_evicted =
                    result.EvictedThroughGeneration >=
                    result.OperationGeneration,
                typed_miss_was_classified_ambiguous =
                    result.ReplayAmbiguous,
                zero_post_restart_unregistration_mutations =
                    result.MutationCallCount == 0,
                local_runtime_projection_was_preserved = true,
                ambiguous_intent_survived_disk_reload = true,
                reappeared_identity_blocked_reconciliation =
                    result
                        .ReappearedIdentityBlockedReconciliation,
                absence_snapshot_permitted_convergence =
                    result.ReconciliationDaemonRevision > 0,
                atomic_local_cleanup_and_audit_survived_reload =
                    result.ReconciliationAuditSurvivedReload,
                reconciliation_replayed_after_restart =
                    result.ReconciliationReplayedAfterRestart,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) +
                "\n");
    }

    private static void
        WriteRuntimeDeletionRetryAtomicBackupEvidenceIfRequested(
            int iterations,
            IReadOnlyList<RuntimeDeletionRetryAtomicBackupResult> results)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_RETRY_ATOMIC_BACKUP_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var duringBackupResults = results.Where(result =>
            result.Strategy ==
            RuntimeDeletionRetryAtomicBackupStrategy
                .DuringBackupTempWrite)
            .ToArray();
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_strategy = iterations,
            strategies = Enum.GetNames<
                RuntimeDeletionRetryAtomicBackupStrategy>(),
            total_forced_terminations = results.Count,
            audit_retention_limit = 256,
            runtime_ids_per_audit_record = 128,
            deliberately_corrupted_primary_count = results.Count(
                static result => result.PrimaryWasCorrupted),
            complete_previous_window_recovery_count = results.Count(
                static result =>
                    result.CompletePreviousWindowRestored),
            typed_backup_recovery_provenance_count = results.Count(
                static result =>
                    result.LoadProvenance.Source ==
                        ControlPlaneStateLoadSource.Backup &&
                    result.LoadProvenance.Outcome ==
                        ControlPlaneStateLoadOutcome.Recovered),
            backup_temp_artifact_observed_count =
                duringBackupResults.Count(static result =>
                    result.TempArtifactObserved),
            checks = new
            {
                backup_refresh_used_unique_temp_file = true,
                every_backup_temp_write_was_observed =
                    duringBackupResults.All(static result =>
                        result.TempArtifactObserved),
                every_primary_was_deliberately_corrupted =
                    results.All(static result =>
                        result.PrimaryWasCorrupted),
                every_fallback_loaded_exactly_256_records =
                    results.All(static result =>
                        result.AuditCount == 256),
                every_fallback_restored_complete_previous_window =
                    results.All(static result =>
                        result.CompletePreviousWindowRestored),
                every_fallback_reported_backup_source =
                    results.All(static result =>
                        result.LoadProvenance.Source ==
                        ControlPlaneStateLoadSource.Backup),
                every_fallback_reported_recovered_outcome =
                    results.All(static result =>
                        result.LoadProvenance.Outcome ==
                        ControlPlaneStateLoadOutcome.Recovered),
                every_primary_failure_reported_invalid_json =
                    results.All(static result =>
                        result.LoadProvenance.PrimaryFailureCode ==
                        ControlPlaneStateLoadFailureCode.InvalidJson),
                no_backup_failure_was_reported =
                    results.All(static result =>
                        result.LoadProvenance.BackupFailureCode is null),
                recovery_provenance_was_secret_free = true,
                no_truncated_or_mixed_backup_window = true,
                every_host_process_force_killed = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void
        WriteRuntimeDeletionRetryPostRecoveryWriteEvidenceIfRequested(
            string evidenceEnvironmentVariable,
            ControlPlaneStateLoadFailureCode primaryFailureCode,
            int iterations,
            IReadOnlyList<RuntimeDeletionRetryPostRecoveryWriteResult> results)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            evidenceEnvironmentVariable);
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var duringWriteResults = results.Where(result =>
            result.Strategy ==
            RuntimeDeletionRetryPostRecoveryWriteStrategy
                .DuringPrimaryTempWrite)
            .ToArray();
        var previousWindowCount = results.Count(result =>
            result.ActiveWindow ==
            RuntimeDeletionRetryAtomicRolloverWindow.Previous);
        var replacementWindowCount = results.Count(result =>
            result.ActiveWindow ==
            RuntimeDeletionRetryAtomicRolloverWindow.Replacement);
        var primaryFailureCodeValue = primaryFailureCode switch
        {
            ControlPlaneStateLoadFailureCode.InvalidJson =>
                "invalid_json",
            ControlPlaneStateLoadFailureCode.SemanticInvalid =>
                "semantic_invalid",
            _ => throw new ArgumentOutOfRangeException(
                nameof(primaryFailureCode)),
        };
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            iterations_per_strategy = iterations,
            strategies = Enum.GetNames<
                RuntimeDeletionRetryPostRecoveryWriteStrategy>(),
            total_forced_terminations = results.Count,
            audit_retention_limit = 256,
            runtime_ids_per_audit_record = 128,
            primary_failure_code = primaryFailureCodeValue,
            active_previous_window_count = previousWindowCount,
            active_replacement_window_count =
                replacementWindowCount,
            known_good_backup_preserved_count = results.Count(
                static result => result.BackupWindowPreserved),
            backup_temp_artifact_absent_count = results.Count(
                static result => result.BackupTempArtifactAbsent),
            primary_temp_artifact_observed_count =
                duringWriteResults.Count(static result =>
                    result.PrimaryTempArtifactObserved),
            checks = new
            {
                recovery_started_from_corrupted_primary = true,
                first_post_recovery_write_skipped_backup_refresh =
                    results.All(static result =>
                        result.BackupTempArtifactAbsent),
                every_primary_temp_write_was_observed =
                    duringWriteResults.All(static result =>
                        result.PrimaryTempArtifactObserved),
                every_restart_loaded_exactly_256_records =
                    results.All(static result =>
                        result.ActiveAuditCount == 256),
                every_backup_retained_exactly_256_records =
                    results.All(static result =>
                        result.BackupAuditCount == 256),
                every_backup_preserved_complete_previous_window =
                    results.All(static result =>
                        result.BackupWindowPreserved),
                precommit_restart_reported_backup_recovery =
                    results
                        .Where(result =>
                            result.ActiveWindow ==
                            RuntimeDeletionRetryAtomicRolloverWindow
                                .Previous)
                        .All(static result =>
                            result.LoadProvenance.Source ==
                                ControlPlaneStateLoadSource.Backup &&
                            result.LoadProvenance.Outcome ==
                                ControlPlaneStateLoadOutcome.Recovered),
                postcommit_restart_reported_clean_primary =
                    results
                        .Where(result =>
                            result.ActiveWindow ==
                            RuntimeDeletionRetryAtomicRolloverWindow
                                .Replacement)
                        .All(static result =>
                            result.LoadProvenance.Source ==
                                ControlPlaneStateLoadSource.Primary &&
                            result.LoadProvenance.Outcome ==
                                ControlPlaneStateLoadOutcome.Clean),
                every_precommit_failure_reported_expected_code =
                    results
                        .Where(result =>
                            result.ActiveWindow ==
                            RuntimeDeletionRetryAtomicRolloverWindow
                                .Previous)
                        .All(result =>
                            result.LoadProvenance
                                .PrimaryFailureCode ==
                            primaryFailureCode),
                every_restart_observed_complete_old_or_new_window =
                    previousWindowCount + replacementWindowCount ==
                    results.Count,
                no_corrupted_primary_was_copied_into_backup = true,
                every_host_process_force_killed = true,
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

    private static void WriteOverlappingIntentTakeoverEvidenceIfRequested(
        long takeoverLatencyMs)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_OVERLAPPING_TAKEOVER_EVIDENCE");
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
            owner_lease_duration_ms = 30_000,
            forced_host_terminations = 1,
            sigkill_daemon_terminations = 1,
            overlapping_intent_count = RuntimeDeletionCrashPhases.Length,
            intent_boundaries = RuntimeDeletionCrashPhases,
            interference_runtime_count = RuntimeDeletionInterferenceRuntimeCount,
            takeover_latency_ms = takeoverLatencyMs,
            checks = new
            {
                real_leserpentd = true,
                mixed_durable_boundaries_shared_one_state = true,
                all_intents_restored_independently = true,
                all_initial_offline_attempts_failed = true,
                all_failed_claims_released_for_retry = true,
                pre_expiry_replacement_rejected = true,
                takeover_waited_for_natural_owner_lease_expiry = true,
                same_daemon_database_reopened = true,
                all_retries_succeeded = true,
                all_intents_converged = true,
                concurrent_registration_and_state_save_traffic = true,
                every_unrelated_runtime_survived_disk_reload = true,
                every_unrelated_daemon_registration_survived = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteRepeatedTakeoverEvidenceIfRequested(
        long firstTakeoverLatencyMs,
        long secondTakeoverLatencyMs)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_REPEATED_TAKEOVER_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var takeoverLatenciesMs = new[]
        {
            firstTakeoverLatencyMs,
            secondTakeoverLatencyMs,
        };
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            owner_lease_duration_ms = 30_000,
            forced_host_terminations = 1,
            sigkill_daemon_terminations = 2,
            owner_lease_takeovers = 2,
            overlapping_intent_count = RuntimeDeletionCrashPhases.Length,
            partially_converged_intent_count = 1,
            pending_intents_after_second_termination = 2,
            intent_boundaries = RuntimeDeletionCrashPhases,
            interference_runtime_count = RuntimeDeletionInterferenceRuntimeCount,
            takeover_latencies_ms = takeoverLatenciesMs,
            checks = new
            {
                real_leserpentd = true,
                mixed_durable_boundaries_shared_one_state = true,
                all_initial_offline_attempts_failed = true,
                first_retry_committed_before_second_sigkill = true,
                first_local_cleanup_completed_after_second_sigkill = true,
                partial_progress_remained_durable = true,
                remaining_attempts_observed_second_outage = true,
                all_failed_claims_released_for_retry = true,
                both_pre_expiry_replacements_rejected = true,
                both_takeovers_waited_for_natural_owner_lease_expiry = true,
                same_daemon_database_reopened_twice = true,
                remaining_retries_succeeded_after_second_takeover = true,
                all_intents_converged = true,
                concurrent_registration_and_state_save_traffic = true,
                every_unrelated_runtime_survived_disk_reload = true,
                every_unrelated_daemon_registration_survived = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WritePoisonIntentIsolationEvidenceIfRequested(
        int poisonAttemptCount,
        long healthyConvergenceLatencyMs)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_POISON_ISOLATION_EVIDENCE");
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
            overlapping_intent_count = RuntimeDeletionCrashPhases.Length,
            poison_intent_count = 1,
            healthy_intent_count = RuntimeDeletionCrashPhases.Length - 1,
            poison_attempt_count = poisonAttemptCount,
            interference_runtime_count = RuntimeDeletionInterferenceRuntimeCount,
            healthy_convergence_latency_ms = healthyConvergenceLatencyMs,
            checks = new
            {
                real_leserpentd = true,
                poison_intent_was_queue_head = true,
                poison_failure_was_target_scoped = true,
                healthy_intents_converged_while_poison_remained_pending = true,
                poison_intent_retried_without_busy_loop = true,
                poison_runtime_remained_protected = true,
                poison_intent_survived_disk_reload = true,
                poison_runtime_remained_protected_after_reload = true,
                repaired_authority_converged_poison_intent = true,
                concurrent_registration_and_state_save_traffic = true,
                every_unrelated_runtime_survived_disk_reload = true,
                every_unrelated_daemon_registration_survived = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteHighCardinalityPoisonEvidenceIfRequested(
        IReadOnlyList<int> poisonAttemptCounts,
        long authorityPhaseLatencyMs,
        long firstPassLatencyMs,
        long poisonRetryWindowMs)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_HIGH_CARDINALITY_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        var poisonIntentCount =
            HighCardinalityRuntimeDeletionIntentCount /
            HighCardinalityRuntimeDeletionPoisonStride;
        Assert.Equal(poisonIntentCount, poisonAttemptCounts.Count);
        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var healthyIntentCount =
            HighCardinalityRuntimeDeletionIntentCount - poisonIntentCount;
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            queue_intent_count = HighCardinalityRuntimeDeletionIntentCount,
            poison_stride = HighCardinalityRuntimeDeletionPoisonStride,
            recovery_batch_size = 32,
            max_concurrent_authority_mutations = 8,
            max_ipc_connections_per_daemon_tick = 64,
            poison_intent_count = poisonIntentCount,
            healthy_intent_count = healthyIntentCount,
            first_pass_converged_intent_count = healthyIntentCount,
            first_pass_pending_intent_count = poisonIntentCount,
            recovery_passes_observed = 3,
            poison_attempt_counts = poisonAttemptCounts,
            interference_runtime_count = RuntimeDeletionInterferenceRuntimeCount,
            authority_phase_latency_ms = authorityPhaseLatencyMs,
            local_batch_latency_ms = firstPassLatencyMs - authorityPhaseLatencyMs,
            first_pass_latency_ms = firstPassLatencyMs,
            poison_retry_window_ms = poisonRetryWindowMs,
            checks = new
            {
                real_leserpentd = true,
                bounded_recovery_claim_batch = true,
                bounded_concurrent_authority_mutations = true,
                bounded_daemon_ipc_drain = true,
                deterministic_durable_queue_order = true,
                sparse_poison_failures_were_target_scoped = true,
                first_pass_made_bounded_healthy_progress = true,
                every_healthy_intent_converged_in_first_pass = true,
                first_pass_latency_under_3000_ms = firstPassLatencyMs < 3_000,
                successful_local_convergence_used_one_strict_batch = true,
                every_poison_intent_retried_without_busy_loop = true,
                poison_reservations_survived_disk_reload = true,
                poison_runtimes_remained_protected_after_reload = true,
                repaired_authority_converged_every_poison_intent = true,
                concurrent_registration_and_state_save_traffic = true,
                every_unrelated_runtime_survived_disk_reload = true,
                every_unrelated_daemon_registration_survived = true,
            },
        };
        File.WriteAllText(
            evidencePath,
            JsonSerializer.Serialize(
                evidence,
                new JsonSerializerOptions { WriteIndented = true }) + "\n");
    }

    private static void WriteBatchPersistenceFailureEvidenceIfRequested(
        int runtimeIntentCount,
        IReadOnlyList<int> attemptCounts,
        long firstFailureLatencyMs,
        long replayStartLatencyMs,
        long convergenceLatencyMs)
    {
        var evidencePath = Environment.GetEnvironmentVariable(
            "LESERPENT_RUNTIME_DELETION_BATCH_PERSISTENCE_EVIDENCE");
        if (string.IsNullOrWhiteSpace(evidencePath))
        {
            return;
        }

        Assert.Equal(runtimeIntentCount, attemptCounts.Count);
        Assert.All(attemptCounts, attemptCount => Assert.Equal(2, attemptCount));
        evidencePath = Path.GetFullPath(evidencePath);
        Directory.CreateDirectory(Path.GetDirectoryName(evidencePath)!);
        var retryDelayMs = replayStartLatencyMs - firstFailureLatencyMs;
        var evidence = new
        {
            schema_version = 1,
            observed_at = DateTimeOffset.UtcNow,
            platform = Environment.OSVersion.Platform.ToString(),
            architecture = RuntimeInformation.ProcessArchitecture.ToString(),
            runtime_intent_count = runtimeIntentCount,
            authority_attempt_counts = attemptCounts,
            orchestra_delete_batch_count = 2,
            first_failure_latency_ms = firstFailureLatencyMs,
            retry_delay_ms = retryDelayMs,
            convergence_latency_ms = convergenceLatencyMs,
            checks = new
            {
                real_leserpentd = true,
                daemon_mutations_committed_before_local_failure = true,
                strict_local_batch_save_failed = true,
                runtime_projection_rolled_back = true,
                session_projection_rolled_back = true,
                orchestra_projection_rolled_back = true,
                recovery_activity_projection_rolled_back = true,
                deletion_intents_rolled_back = true,
                deleting_reservations_remained_protected = true,
                failed_pass_state_survived_disk_reload = true,
                retries_were_paced = retryDelayMs >= 750,
                daemon_unregistration_replayed_idempotently = true,
                orchestra_cleanup_replayed_idempotently = true,
                next_pass_converged = true,
                converged_state_survived_disk_reload = true,
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
                daemon.WaitForExit();
                throw new InvalidOperationException(
                    $"leserpentd exited during startup: {CapturedDaemonOutput(daemon)}");
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

    private static string CommandResponse(
        string runtimeId,
        string commandId,
        int revision = 1,
        bool includeRuntimeProjection = false,
        string runtimeName = "Daemon Runtime",
        string runtimeEndpoint = "https://daemon.invalid",
        string? runtimeSidecarEndpoint = "https://daemon-sidecar.invalid",
        string? runtimeEnvironment = "prod",
        string? runtimeCluster = null,
        string? runtimeRole = "edge",
        int? responseRevision = null) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"command\"," +
        "\"payload\":{\"command_id\":\"" +
        commandId +
        "\",\"status\":\"applied\"," +
        "\"runtime\":" +
        (includeRuntimeProjection
            ? RuntimeProjectionJson(
                runtimeId: runtimeId,
                revision: revision,
                runtimeName: runtimeName,
                runtimeEndpoint: runtimeEndpoint,
                runtimeSidecarEndpoint: runtimeSidecarEndpoint,
                runtimeEnvironment: runtimeEnvironment,
                runtimeCluster: runtimeCluster,
                runtimeRole: runtimeRole)
            : "{\"id\":\"" + runtimeId + "\"}") +
        ",\"revision\":" + (responseRevision ?? revision) + "}}}";

    private static string QueryResponse(string runtimeId, int revision) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"query\"," +
        "\"payload\":{\"kind\":\"runtime_inspect\",\"revision\":" + revision + "," +
        "\"runtime\":{\"id\":\"" + runtimeId + "\",\"revision\":" + revision + "}}}}";

    private static string RuntimeUnregisteredResponse(
        string commandId,
        string runtimeId,
        int revision,
        ulong operationGeneration = 1) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"runtime_unregistered\",\"payload\":{" +
        "\"command_id\":\"" + commandId + "\"," +
        "\"operation_generation\":" + operationGeneration + "," +
        "\"removed\":[{\"runtime_id\":\"" + runtimeId +
        "\",\"expected_revision\":" + revision + "}]," +
        "\"deleted_orchestra_runtime_count\":0," +
        "\"deleted_orchestra_run_count\":0," +
        "\"deleted_orchestra_event_count\":0," +
        "\"removed_at_unix_ms\":1784620800000,\"replayed\":false}}}";

    private static string RuntimeUnregistrationReceiptResponse(
        string commandId,
        string runtimeId,
        bool receiptExists) =>
        "{" +
        "\"schema_version\":1," +
        "\"response\":{\"kind\":\"runtime_unregistration_receipt\",\"payload\":{" +
        "\"command_id\":\"" + commandId + "\"," +
        "\"receipt\":" +
        (receiptExists
            ? "{\"operation_generation\":7," +
                "\"removed\":[{\"runtime_id\":\"" + runtimeId +
                "\",\"expected_revision\":9}]," +
                "\"deleted_orchestra_runtime_count\":0," +
                "\"deleted_orchestra_run_count\":0," +
                "\"deleted_orchestra_event_count\":0," +
                "\"removed_at_unix_ms\":1784620800000}"
            : "null") +
        ",\"replay_horizon\":{" +
        "\"capacity\":256," +
        "\"retained\":" + (receiptExists ? "1" : "0") + "," +
        "\"oldest_generation\":" + (receiptExists ? "7" : "null") + "," +
        "\"newest_generation\":" + (receiptExists ? "7" : "null") + "," +
        "\"next_generation\":" + (receiptExists ? "8" : "1") + "," +
        "\"evicted_through_generation\":" +
        (receiptExists ? "6" : "0") + "}}}}";

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

    private static string RuntimeProjectionJson(
        bool includeSidecarStatus = true,
        string runtimeId = "runtime-a",
        int revision = 2,
        string runtimeName = "Daemon Runtime",
        string runtimeEndpoint = "https://daemon.invalid",
        string? runtimeSidecarEndpoint = "https://daemon-sidecar.invalid",
        string? runtimeEnvironment = "prod",
        string? runtimeCluster = null,
        string? runtimeRole = "edge") =>
        "{" +
        "\"id\":\"" + runtimeId +
        "\",\"name\":" + JsonSerializer.Serialize(runtimeName) +
        ",\"endpoint\":" + JsonSerializer.Serialize(runtimeEndpoint) + "," +
        "\"sidecar_endpoint\":" + JsonSerializer.Serialize(runtimeSidecarEndpoint) + "," +
        "\"registered_at_unix_ms\":1784620800000,\"updated_at_unix_ms\":1784626200000," +
        "\"revision\":" + revision +
        ",\"refresh_count\":0,\"refresh_status\":\"ready\"," +
        "\"tags\":{\"environment\":" + JsonSerializer.Serialize(runtimeEnvironment) +
        ",\"cluster\":" + JsonSerializer.Serialize(runtimeCluster) +
        ",\"role\":" + JsonSerializer.Serialize(runtimeRole) + "}," +
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

    private sealed record RuntimeDeletionRetryCrashScenarioResult(
        bool DaemonCommittedBeforeTermination,
        int RecoveryAuthorityCallCount);

    private sealed record RuntimeDeletionLostAcknowledgementResult(
        int LookupCallCount,
        int MutationCallCount,
        ulong OperationGeneration,
        ulong ReplayHorizonFloor,
        ulong? EvictedThroughGeneration,
        bool ReplayAmbiguous,
        bool ReappearedIdentityBlockedReconciliation,
        ulong ReconciliationDaemonRevision,
        bool ReconciliationAuditSurvivedReload,
        bool ReconciliationReplayedAfterRestart);

    private enum RuntimeDeletionReconciliationCommitStrategy
    {
        BeforeWrite,
        DuringTempWrite,
        AfterCommit,
    }

    private enum RuntimeDeletionReconciliationCommitWindow
    {
        Previous,
        Replacement,
        Torn,
    }

    private sealed record RuntimeDeletionReconciliationCommitResult(
        RuntimeDeletionReconciliationCommitStrategy Strategy,
        RuntimeDeletionReconciliationCommitWindow Window,
        bool TempArtifactObserved,
        int RetryAuditCount,
        bool FinalStateConverged,
        bool ReconciliationAuditSurvivedReload,
        bool RequestReplayedAfterRestart);

    private enum RuntimeDeletionCrossAuthorityStrategy
    {
        AfterOrchestraCleanup,
        DuringControlTempWrite,
        AfterControlCommit,
    }

    private enum CleanupCheckpointRaceOrder
    {
        CleanupFirst,
        CheckpointFirst,
    }

    private sealed record CleanupCheckpointRaceResult(
        CleanupCheckpointRaceOrder Order,
        ulong AvailableBeforeRace,
        ulong CheckpointGeneration,
        ulong CleanupGeneration,
        bool PreSaturationCriticalVisible,
        bool CleanupCommitted,
        bool CheckpointCommitted,
        bool ExpectedCompletionOrderObserved,
        bool FinalHorizonAdmissionSafe);

    private sealed record AuditCheckpointDaemonRestartResult(
        ulong CheckpointLagBeforeDaemonRestart,
        ulong CheckpointLagAfterDaemonRestart,
        ulong AuditGeneration,
        ulong CheckpointedThroughGeneration,
        bool DaemonRestarted,
        bool AutomaticCheckpointStatusReported);

    private sealed record RuntimeDeletionCrossAuthorityResult(
        RuntimeDeletionCrossAuthorityStrategy Strategy,
        RuntimeDeletionReconciliationCommitWindow Window,
        bool TempArtifactObserved,
        bool TargetHistoryAbsentBeforeTermination,
        bool UnrelatedHistoryPreserved,
        bool FinalStateConverged,
        bool SingleAuditSurvivedReload,
        bool RequestReplayedAfterRestart,
        bool CleanupReceiptReplayedSameGeneration,
        bool AuditCheckpointProtectedReplayHorizon);

    private enum RuntimeDeletionRetryAtomicRolloverStrategy
    {
        BeforeWrite,
        DuringTempWrite,
        AfterCommit,
    }

    private enum RuntimeDeletionRetryAtomicRolloverWindow
    {
        Previous,
        Replacement,
        Torn,
    }

    private sealed record RuntimeDeletionRetryAtomicRolloverResult(
        RuntimeDeletionRetryAtomicRolloverStrategy Strategy,
        RuntimeDeletionRetryAtomicRolloverWindow Window,
        bool TempArtifactObserved,
        int AuditCount);

    private enum RuntimeDeletionRetryAtomicBackupStrategy
    {
        BeforeWrite,
        DuringBackupTempWrite,
        AfterCommit,
    }

    private sealed record RuntimeDeletionRetryAtomicBackupResult(
        RuntimeDeletionRetryAtomicBackupStrategy Strategy,
        bool TempArtifactObserved,
        bool PrimaryWasCorrupted,
        bool CompletePreviousWindowRestored,
        ControlPlaneStateLoadProvenance LoadProvenance,
        int AuditCount);

    private enum RuntimeDeletionRetryPostRecoveryWriteStrategy
    {
        BeforeWrite,
        DuringPrimaryTempWrite,
        AfterCommit,
    }

    private sealed record RuntimeDeletionRetryPostRecoveryWriteResult(
        RuntimeDeletionRetryPostRecoveryWriteStrategy Strategy,
        RuntimeDeletionRetryAtomicRolloverWindow ActiveWindow,
        bool PrimaryTempArtifactObserved,
        bool BackupTempArtifactAbsent,
        bool BackupWindowPreserved,
        ControlPlaneStateLoadProvenance LoadProvenance,
        int ActiveAuditCount,
        int BackupAuditCount);

    private sealed record UncleanDaemonTakeoverResult(
        IReadOnlyList<string> RuntimeIds,
        long TakeoverLatencyMs);

    private sealed class ReplayCountingRunStore : IOrchestraRunStore
    {
        private readonly InMemoryOrchestraRunStore inner = new();

        public int DeleteCount { get; private set; }
        public string Provider => "replay-counting-test";
        public string Location => "test";
        public int SchemaVersion => 0;
        public string? LastError => null;
        public IReadOnlyList<OrchestraRunSummary> LoadAll() => inner.LoadAll();
        public IReadOnlyList<OrchestraRunEvent> LoadEvents(
            string runtimeId,
            string runId) =>
            inner.LoadEvents(runtimeId, runId);
        public bool Upsert(
            OrchestraRunSummary run,
            OrchestraRunEvent? eventRecord = null) =>
            inner.Upsert(run, eventRecord);
        public bool ReplaceAll(IReadOnlyList<OrchestraRunSummary> runs) =>
            inner.ReplaceAll(runs);

        public bool DeleteRuntimes(IReadOnlyCollection<string> runtimeIds)
        {
            DeleteCount += 1;
            return inner.DeleteRuntimes(runtimeIds);
        }
    }

    private sealed class CountingRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner) : IRuntimeRegistrationAuthority
    {
        private int unregisterCallCount;

        public bool Enabled => inner.Enabled;
        public int UnregisterCallCount =>
            Volatile.Read(ref unregisterCallCount);

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
            Interlocked.Increment(ref unregisterCallCount);
            await inner.UnregisterAsync(runtimeIds, cancellationToken);
        }
    }

    private sealed class ReceiptLookupCountingRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner) :
        IRuntimeRegistrationAuthority
    {
        private int lookupCallCount;
        private int mutationCallCount;

        public bool Enabled => inner.Enabled;
        public int LookupCallCount =>
            Volatile.Read(ref lookupCallCount);
        public int MutationCallCount =>
            Volatile.Read(ref mutationCallCount);
        public RuntimeUnregistrationReceiptLookup? LastLookup
        {
            get;
            private set;
        }

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

        public Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref mutationCallCount);
            return inner.UnregisterAsync(
                runtimeIds,
                cancellationToken);
        }

        public Task UnregisterAsync(
            IReadOnlyCollection<string> runtimeIds,
            string commandId,
            CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref mutationCallCount);
            return inner.UnregisterAsync(
                runtimeIds,
                commandId,
                cancellationToken);
        }

        public async Task<RuntimeUnregistrationReceiptLookup>
            LookupUnregistrationReceiptAsync(
                string commandId,
                CancellationToken cancellationToken)
        {
            Interlocked.Increment(ref lookupCallCount);
            var lookup = await inner
                .LookupUnregistrationReceiptAsync(
                    commandId,
                    cancellationToken);
            LastLookup = lookup;
            return lookup;
        }
    }

    private sealed class ReplayGatedRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner,
        IReadOnlyCollection<string> expectedRuntimeIds) : IRuntimeRegistrationAuthority
    {
        private readonly object sync = new();
        private readonly string[] expectedRuntimeIds =
            expectedRuntimeIds.OrderBy(static runtimeId => runtimeId, StringComparer.Ordinal).ToArray();
        private readonly Dictionary<string, int> attemptCounts =
            expectedRuntimeIds.ToDictionary(
                static runtimeId => runtimeId,
                static _ => 0,
                StringComparer.Ordinal);
        private readonly TaskCompletionSource firstPassCompleted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource replayStarted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource everyRuntimeReplayed =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowReplay =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        public bool Enabled => inner.Enabled;
        public Task FirstPassCompleted => firstPassCompleted.Task;
        public Task ReplayStarted => replayStarted.Task;
        public Task EveryRuntimeReplayed => everyRuntimeReplayed.Task;
        public IReadOnlyList<int> AttemptCounts
        {
            get
            {
                lock (sync)
                {
                    return expectedRuntimeIds
                        .Select(runtimeId => attemptCounts[runtimeId])
                        .ToArray();
                }
            }
        }

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
            var runtimeId = Assert.Single(runtimeIds);
            Assert.Contains(runtimeId, expectedRuntimeIds);
            int attempt;
            lock (sync)
            {
                attemptCounts[runtimeId] += 1;
                attempt = attemptCounts[runtimeId];
                if (attempt == 2)
                {
                    replayStarted.TrySetResult();
                }
            }
            if (attempt > 1)
            {
                await allowReplay.Task.WaitAsync(cancellationToken);
            }

            await inner.UnregisterAsync(runtimeIds, cancellationToken);
            lock (sync)
            {
                if (attempt == 1 &&
                    attemptCounts.Values.All(static count => count >= 1))
                {
                    firstPassCompleted.TrySetResult();
                }
                if (attempt == 2 &&
                    attemptCounts.Values.All(static count => count >= 2))
                {
                    everyRuntimeReplayed.TrySetResult();
                }
            }
        }

        public void AllowReplay() => allowReplay.TrySetResult();
    }

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

    private sealed class MultiIntentTakeoverRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner,
        IReadOnlyCollection<string> expectedRuntimeIds) : IRuntimeRegistrationAuthority
    {
        private readonly object sync = new();
        private readonly HashSet<string> expectedRuntimeIds =
            expectedRuntimeIds.ToHashSet(StringComparer.Ordinal);
        private readonly Dictionary<string, int> attemptCounts = new(StringComparer.Ordinal);
        private readonly HashSet<string> initialFailures = new(StringComparer.Ordinal);
        private readonly HashSet<string> successfulRetries = new(StringComparer.Ordinal);
        private readonly TaskCompletionSource allInitialAttemptsFailed =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allRetriesSucceeded =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowRetries =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        public bool Enabled => inner.Enabled;
        public Task AllInitialAttemptsFailed => allInitialAttemptsFailed.Task;
        public Task AllRetriesSucceeded => allRetriesSucceeded.Task;

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
            var runtimeId = Assert.Single(runtimeIds);
            Assert.Contains(runtimeId, expectedRuntimeIds);
            int attempt;
            lock (sync)
            {
                attemptCounts.TryGetValue(runtimeId, out attempt);
                attempt += 1;
                attemptCounts[runtimeId] = attempt;
            }

            if (attempt == 1)
            {
                try
                {
                    await inner.UnregisterAsync(runtimeIds, cancellationToken);
                }
                catch
                {
                    lock (sync)
                    {
                        initialFailures.Add(runtimeId);
                        if (initialFailures.SetEquals(expectedRuntimeIds))
                        {
                            allInitialAttemptsFailed.TrySetResult();
                        }
                    }
                    throw;
                }

                var error = new InvalidOperationException(
                    "runtime deletion unexpectedly reached an offline daemon");
                allInitialAttemptsFailed.TrySetException(error);
                throw error;
            }

            await allowRetries.Task.WaitAsync(cancellationToken);
            await inner.UnregisterAsync(runtimeIds, cancellationToken);
            lock (sync)
            {
                successfulRetries.Add(runtimeId);
                if (successfulRetries.SetEquals(expectedRuntimeIds))
                {
                    allRetriesSucceeded.TrySetResult();
                }
            }
        }

        public void AllowRetries() => allowRetries.TrySetResult();
    }

    private sealed class InterruptedMultiIntentTakeoverRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner,
        IReadOnlyCollection<string> expectedRuntimeIds) : IRuntimeRegistrationAuthority
    {
        private readonly object sync = new();
        private readonly HashSet<string> expectedRuntimeIds =
            expectedRuntimeIds.ToHashSet(StringComparer.Ordinal);
        private readonly Dictionary<string, int> attemptCounts = new(StringComparer.Ordinal);
        private readonly HashSet<string> initialFailures = new(StringComparer.Ordinal);
        private readonly HashSet<string> failuresAfterSecondTermination =
            new(StringComparer.Ordinal);
        private readonly HashSet<string> finalSuccesses = new(StringComparer.Ordinal);
        private readonly TaskCompletionSource allInitialAttemptsFailed =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource firstRetryDaemonCommitted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource remainingAttemptsFailedAfterSecondTermination =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allFinalRetriesSucceeded =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowFirstRetry =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource completeFirstRetry =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource allowFinalRetries =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private string? firstCommittedRuntimeId;

        public bool Enabled => inner.Enabled;
        public Task AllInitialAttemptsFailed => allInitialAttemptsFailed.Task;
        public Task FirstRetryDaemonCommitted => firstRetryDaemonCommitted.Task;
        public Task RemainingAttemptsFailedAfterSecondTermination =>
            remainingAttemptsFailedAfterSecondTermination.Task;
        public Task AllFinalRetriesSucceeded => allFinalRetriesSucceeded.Task;
        public string? FirstCommittedRuntimeId
        {
            get
            {
                lock (sync)
                {
                    return firstCommittedRuntimeId;
                }
            }
        }

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
            var runtimeId = Assert.Single(runtimeIds);
            Assert.Contains(runtimeId, expectedRuntimeIds);
            int attempt;
            lock (sync)
            {
                attemptCounts.TryGetValue(runtimeId, out attempt);
                attempt += 1;
                attemptCounts[runtimeId] = attempt;
            }

            if (attempt == 1)
            {
                try
                {
                    await inner.UnregisterAsync(runtimeIds, cancellationToken);
                }
                catch
                {
                    lock (sync)
                    {
                        initialFailures.Add(runtimeId);
                        if (initialFailures.SetEquals(expectedRuntimeIds))
                        {
                            allInitialAttemptsFailed.TrySetResult();
                        }
                    }
                    throw;
                }
                throw UnexpectedOnlineDaemon(allInitialAttemptsFailed);
            }

            bool isFirstRetry;
            lock (sync)
            {
                isFirstRetry = firstCommittedRuntimeId is null;
                if (isFirstRetry)
                {
                    firstCommittedRuntimeId = runtimeId;
                }
            }
            if (isFirstRetry)
            {
                await allowFirstRetry.Task.WaitAsync(cancellationToken);
                await inner.UnregisterAsync(runtimeIds, cancellationToken);
                firstRetryDaemonCommitted.TrySetResult();
                await completeFirstRetry.Task.WaitAsync(cancellationToken);
                return;
            }

            bool alreadyFailedAfterSecondTermination;
            lock (sync)
            {
                alreadyFailedAfterSecondTermination =
                    failuresAfterSecondTermination.Contains(runtimeId);
            }
            if (!alreadyFailedAfterSecondTermination)
            {
                await completeFirstRetry.Task.WaitAsync(cancellationToken);
                try
                {
                    await inner.UnregisterAsync(runtimeIds, cancellationToken);
                }
                catch
                {
                    lock (sync)
                    {
                        failuresAfterSecondTermination.Add(runtimeId);
                        if (failuresAfterSecondTermination.Count ==
                            expectedRuntimeIds.Count - 1)
                        {
                            remainingAttemptsFailedAfterSecondTermination.TrySetResult();
                        }
                    }
                    throw;
                }
                throw UnexpectedOnlineDaemon(
                    remainingAttemptsFailedAfterSecondTermination);
            }

            await allowFinalRetries.Task.WaitAsync(cancellationToken);
            await inner.UnregisterAsync(runtimeIds, cancellationToken);
            lock (sync)
            {
                finalSuccesses.Add(runtimeId);
                if (finalSuccesses.Count == expectedRuntimeIds.Count - 1)
                {
                    allFinalRetriesSucceeded.TrySetResult();
                }
            }
        }

        public void AllowFirstRetry() => allowFirstRetry.TrySetResult();

        public void CompleteFirstRetryAfterSecondTermination() =>
            completeFirstRetry.TrySetResult();

        public void AllowFinalRetries() => allowFinalRetries.TrySetResult();

        private static InvalidOperationException UnexpectedOnlineDaemon(
            TaskCompletionSource completion)
        {
            var error = new InvalidOperationException(
                "runtime deletion unexpectedly reached an online daemon");
            completion.TrySetException(error);
            return error;
        }
    }

    private sealed class PoisonIntentRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner,
        string poisonRuntimeId,
        IReadOnlyCollection<string> healthyRuntimeIds) : IRuntimeRegistrationAuthority
    {
        private readonly object sync = new();
        private readonly HashSet<string> healthyRuntimeIds =
            healthyRuntimeIds.ToHashSet(StringComparer.Ordinal);
        private readonly HashSet<string> healthySuccesses = new(StringComparer.Ordinal);
        private readonly TaskCompletionSource allHealthyIntentsSucceeded =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource poisonRetriedThreeTimes =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private int poisonAttemptCount;

        public bool Enabled => inner.Enabled;
        public Task AllHealthyIntentsSucceeded => allHealthyIntentsSucceeded.Task;
        public Task PoisonRetriedThreeTimes => poisonRetriedThreeTimes.Task;
        public int PoisonAttemptCount => Volatile.Read(ref poisonAttemptCount);

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
            var runtimeId = Assert.Single(runtimeIds);
            if (string.Equals(runtimeId, poisonRuntimeId, StringComparison.Ordinal))
            {
                var attempt = Interlocked.Increment(ref poisonAttemptCount);
                if (attempt >= 3)
                {
                    poisonRetriedThreeTimes.TrySetResult();
                }
                throw new InvalidOperationException(
                    "test-only poison runtime deletion failure");
            }

            Assert.Contains(runtimeId, healthyRuntimeIds);
            await inner.UnregisterAsync(runtimeIds, cancellationToken);
            lock (sync)
            {
                healthySuccesses.Add(runtimeId);
                if (healthySuccesses.SetEquals(healthyRuntimeIds))
                {
                    allHealthyIntentsSucceeded.TrySetResult();
                }
            }
        }
    }

    private sealed class SparsePoisonRuntimeDeletionAuthority(
        IRuntimeRegistrationAuthority inner,
        IReadOnlyCollection<string> poisonRuntimeIds,
        IReadOnlyCollection<string> healthyRuntimeIds) : IRuntimeRegistrationAuthority
    {
        private readonly object sync = new();
        private readonly string[] poisonRuntimeIds =
            poisonRuntimeIds.OrderBy(static runtimeId => runtimeId, StringComparer.Ordinal).ToArray();
        private readonly HashSet<string> poisonRuntimeIdSet =
            poisonRuntimeIds.ToHashSet(StringComparer.Ordinal);
        private readonly HashSet<string> healthyRuntimeIds =
            healthyRuntimeIds.ToHashSet(StringComparer.Ordinal);
        private readonly Dictionary<string, int> poisonAttemptCounts =
            poisonRuntimeIds.ToDictionary(
                static runtimeId => runtimeId,
                static _ => 0,
                StringComparer.Ordinal);
        private readonly HashSet<string> healthySuccesses = new(StringComparer.Ordinal);
        private readonly TaskCompletionSource firstPassCompleted =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource everyPoisonRetriedThreeTimes =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        public bool Enabled => inner.Enabled;
        public Task FirstPassCompleted => firstPassCompleted.Task;
        public Task EveryPoisonRetriedThreeTimes =>
            everyPoisonRetriedThreeTimes.Task;
        public IReadOnlyList<int> PoisonAttemptCounts
        {
            get
            {
                lock (sync)
                {
                    return poisonRuntimeIds
                        .Select(runtimeId => poisonAttemptCounts[runtimeId])
                        .ToArray();
                }
            }
        }

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
            var runtimeId = Assert.Single(runtimeIds);
            if (poisonRuntimeIdSet.Contains(runtimeId))
            {
                lock (sync)
                {
                    poisonAttemptCounts[runtimeId] += 1;
                    ObserveProgress();
                }
                throw new InvalidOperationException(
                    "test-only sparse poison runtime deletion failure");
            }

            Assert.Contains(runtimeId, healthyRuntimeIds);
            await inner.UnregisterAsync(runtimeIds, cancellationToken);
            lock (sync)
            {
                healthySuccesses.Add(runtimeId);
                ObserveProgress();
            }
        }

        private void ObserveProgress()
        {
            if (healthySuccesses.SetEquals(healthyRuntimeIds) &&
                poisonAttemptCounts.Values.All(static attempts => attempts >= 1))
            {
                firstPassCompleted.TrySetResult();
            }
            if (poisonAttemptCounts.Values.All(static attempts => attempts >= 3))
            {
                everyPoisonRetriedThreeTimes.TrySetResult();
            }
        }
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

    private sealed class BoundedProcessOutput
    {
        private const int MaximumLines = 32;
        private const int MaximumLineLength = 1024;
        private readonly Queue<string> lines = new();
        private readonly object sync = new();

        public void Append(string stream, string? line)
        {
            if (string.IsNullOrEmpty(line))
            {
                return;
            }
            var bounded = line.Length <= MaximumLineLength
                ? line
                : line[..MaximumLineLength];
            lock (sync)
            {
                lines.Enqueue($"{stream}: {bounded}");
                while (lines.Count > MaximumLines)
                {
                    lines.Dequeue();
                }
            }
        }

        public string Snapshot()
        {
            lock (sync)
            {
                return lines.Count == 0
                    ? "no daemon output was captured"
                    : string.Join(Environment.NewLine, lines);
            }
        }
    }

    private sealed class CrashTestEnvironment(string contentRootPath) : IHostEnvironment
    {
        public string EnvironmentName { get; set; } = Environments.Development;
        public string ApplicationName { get; set; } = "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = contentRootPath;
        public IFileProvider ContentRootFileProvider { get; set; } = new NullFileProvider();
    }
}
