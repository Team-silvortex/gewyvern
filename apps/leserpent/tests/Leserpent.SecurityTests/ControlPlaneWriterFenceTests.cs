using Leserpent.ControlPlane;
using System.Net.Sockets;
using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Http;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.FileProviders;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Leserpent.SecurityTests;

public sealed class ControlPlaneWriterFenceTests
{
    [Theory]
    [InlineData("GET", "/v1/runtimes", false)]
    [InlineData("HEAD", "/v1/runtimes", false)]
    [InlineData("OPTIONS", "/v1/runtimes", false)]
    [InlineData("POST", "/v1/runtimes/registration-plan", false)]
    [InlineData("POST", "/v1/persistence/save", true)]
    [InlineData("POST", "/v1/runtimes/register", true)]
    [InlineData("POST", "/v1/runtimes/abc/deployments", true)]
    [InlineData("POST", "/v1/orchestra/plans/abc/plan/execute", true)]
    [InlineData("POST", "/v1/sessions", true)]
    [InlineData("PUT", "/v1/future-resource", true)]
    [InlineData("PATCH", "/v1/future-resource", true)]
    [InlineData("DELETE", "/v1/future-resource", true)]
    [InlineData("POST", "/health", false)]
    public void MutationPolicyIsFailClosedForControlPlaneWrites(
        string method,
        string path,
        bool expected)
    {
        var context = new DefaultHttpContext();
        context.Request.Method = method;
        context.Request.Path = path;

        Assert.Equal(
            expected,
            ControlPlaneMutationPolicy.IsMutation(context.Request));
    }

    [Fact]
    public async Task OneWriterStandbyRefusesMutationAndFreshProcessTakesOver()
    {
        var directory = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-writer-fence-{Guid.NewGuid():N}");
        Directory.CreateDirectory(directory);
        var statePath = Path.Combine(directory, "state.json");

        var firstStore = CreateStateStore(statePath);
        var secondStore = CreateStateStore(statePath);
        using var firstLease = new ControlPlaneWriterLease(firstStore);
        using var secondLease = new ControlPlaneWriterLease(secondStore);
        var firstFence = CreateFence(firstLease);
        var secondFence = CreateFence(secondLease);
        using var firstCheckpointLease =
            new OrchestraDeleteCheckpointWorkerLease(firstStore);
        using var secondCheckpointLease =
            new OrchestraDeleteCheckpointWorkerLease(secondStore);

        try
        {
            await firstFence.StartAsync(CancellationToken.None);
            await secondFence.StartAsync(CancellationToken.None);

            Assert.True(firstFence.IsWriter);
            Assert.False(secondFence.IsWriter);
            Assert.Equal("owner", firstFence.Snapshot().State);
            Assert.Equal("standby", secondFence.Snapshot().State);

            var firstRegistry = new RegistryService(
                firstStore,
                new InMemoryOrchestraRunStore(),
                firstCheckpointLease,
                firstFence);
            var secondRegistry = new RegistryService(
                secondStore,
                new InMemoryOrchestraRunStore(),
                secondCheckpointLease,
                secondFence);

            firstRegistry.RegisterRuntime(new RuntimeRegistrationRequest(
                "writer-owned-runtime",
                "http://127.0.0.1:8080",
                "pairing-token"));
            var persistedBeforeStandbyAttempt =
                await File.ReadAllBytesAsync(statePath);

            var error = Assert.Throws<
                ControlPlaneWriterUnavailableException>(
                () => secondRegistry.SaveNow());
            Assert.Equal(
                "control-plane mutation requires active writer ownership",
                error.Message);
            Assert.Empty(secondRegistry.ListRuntimes());
            Assert.Equal(
                persistedBeforeStandbyAttempt,
                await File.ReadAllBytesAsync(statePath));

            firstLease.Dispose();
            Assert.False(firstFence.IsWriter);
            Assert.Equal("lease_lost", firstFence.Snapshot().State);
            Assert.False(secondFence.IsWriter);
            Assert.Throws<ControlPlaneWriterUnavailableException>(
                () => secondRegistry.SaveNow());

            var takeoverStore = CreateStateStore(statePath);
            using var takeoverLease =
                new ControlPlaneWriterLease(takeoverStore);
            var takeoverFence = CreateFence(takeoverLease);
            await takeoverFence.StartAsync(CancellationToken.None);
            Assert.True(takeoverFence.IsWriter);

            using var takeoverCheckpointLease =
                new OrchestraDeleteCheckpointWorkerLease(
                    takeoverStore);
            var takeoverRegistry = new RegistryService(
                takeoverStore,
                new InMemoryOrchestraRunStore(),
                takeoverCheckpointLease,
                takeoverFence);
            Assert.Single(takeoverRegistry.ListRuntimes());
            _ = takeoverRegistry.SaveNow();
        }
        finally
        {
            try
            {
                Directory.Delete(directory, recursive: true);
            }
            catch (IOException)
            {
            }
        }
    }

    [Fact]
    public async Task ActiveWriterClaimsDaemonGenerationAndFencesAuthorityMutations()
    {
        if (OperatingSystem.IsWindows())
        {
            return;
        }
        var directory = Path.Combine(
            Path.GetTempPath(),
            $"leserpent-writer-authority-{Guid.NewGuid():N}");
        Directory.CreateDirectory(directory);
        var statePath = Path.Combine(directory, "state.json");
        var socketPath =
            $"/tmp/lese-writer-{Guid.NewGuid():N}.sock";
        const string token =
            "0123456789abcdef0123456789abcdef";
        var configuration = new ConfigurationBuilder()
            .AddInMemoryCollection(
                new Dictionary<string, string?>
                {
                    ["LESERPENT_STATE_PATH"] = statePath,
                    ["LESERPENT_DAEMON_SOCKET"] = socketPath,
                    ["LESERPENT_DAEMON_TOKEN"] = token,
                })
            .Build();
        var store = new ControlPlaneStateStore(
            configuration,
            new TestHostEnvironment
            {
                ContentRootPath = directory,
            },
            NullLogger<ControlPlaneStateStore>.Instance);
        using var lease = new ControlPlaneWriterLease(store);
        var session = new DaemonAuthorityWriterSession(configuration);
        var fence = new ControlPlaneWriterFence(
            lease,
            NullLogger<ControlPlaneWriterFence>.Instance,
            session);
        using var listener = new Socket(
            AddressFamily.Unix,
            SocketType.Stream,
            ProtocolType.Unspecified);
        listener.Bind(new UnixDomainSocketEndPoint(socketPath));
        File.SetUnixFileMode(
            socketPath,
            UnixFileMode.UserRead | UnixFileMode.UserWrite);
        listener.Listen(5);
        var requests = new List<JsonElement>();
        var server = ServeWriterClaimAndAuthorityMutationsAsync(
            listener,
            requests);

        try
        {
            await fence.StartAsync(CancellationToken.None);
            Assert.True(fence.IsWriter);
            Assert.Equal(
                7UL,
                fence.Snapshot().AuthorityGeneration);

            var authority =
                new DaemonRuntimeRegistrationAuthority(
                    configuration,
                    fence);
            await authority.RegisterAsync(
                new RuntimeRegistrationRequest(
                    "Runtime Fenced",
                    "https://runtime.example",
                    "pairing-token"),
                "runtime-fenced",
                CancellationToken.None);

            var deployment = new DaemonDeploymentAuthority(
                configuration,
                fence);
            var deploymentResult = await deployment.DeployAsync(
                new RuntimeControlAccess(
                    "runtime-fenced",
                    "Runtime Fenced",
                    "https://runtime.example",
                    "pairing-token",
                    new RuntimeTags(null, null, null)),
                1,
                new RuntimeDeploymentRequest(
                    "capture/http",
                    "operator",
                    true,
                    "writer-fenced-deploy",
                    "service-a"),
                CancellationToken.None);
            Assert.Equal(
                "writer-fenced-deployment",
                deploymentResult.DeploymentId);

            var orchestraStore = new DaemonOrchestraRunStore(
                configuration,
                NullLogger<DaemonOrchestraRunStore>.Instance,
                fence);
            var executedAt = DateTimeOffset.Parse(
                "2026-08-01T00:00:00Z");
            var run = new OrchestraRunSummary(
                "writer-fenced-run",
                "runtime-fenced",
                "plan-a",
                "queued",
                executedAt,
                Array.Empty<OrchestraExecutionStepResult>(),
                RequestId: "writer-fenced-orchestra");
            var eventRecord = new OrchestraRunEvent(
                0,
                run.RunId,
                run.RuntimeId,
                "run_queued",
                null,
                run.Outcome,
                "Orchestra run queued",
                executedAt);
            Assert.True(orchestraStore.Upsert(run, eventRecord));
            await server;

            Assert.Equal(5, requests.Count);
            var claim = requests[0]
                .GetProperty("request")
                .GetProperty("request");
            Assert.Equal(
                "authority_writer_claim",
                claim.GetProperty("kind").GetString());
            var writerId = claim
                .GetProperty("payload")
                .GetProperty("writer_id")
                .GetString();
            foreach (var frame in requests.Skip(1))
            {
                var ticket = frame.GetProperty("writer_fence");
                Assert.Equal(
                    7UL,
                    ticket.GetProperty("generation").GetUInt64());
                Assert.Equal(
                    writerId,
                    ticket.GetProperty("writer_id").GetString());
            }
        }
        finally
        {
            try
            {
                File.Delete(socketPath);
            }
            catch (IOException)
            {
            }
            try
            {
                Directory.Delete(directory, recursive: true);
            }
            catch (IOException)
            {
            }
        }
    }

    private static async Task ServeWriterClaimAndAuthorityMutationsAsync(
        Socket listener,
        List<JsonElement> requests)
    {
        for (var index = 0; index < 5; index++)
        {
            using var client = await listener.AcceptAsync();
            var request = await ReadFrameAsync(client);
            using var document = JsonDocument.Parse(request);
            var frame = document.RootElement.Clone();
            requests.Add(frame);
            string response;
            if (index == 0)
            {
                var writerId = frame
                    .GetProperty("request")
                    .GetProperty("request")
                    .GetProperty("payload")
                    .GetProperty("writer_id")
                    .GetString();
                response = JsonSerializer.Serialize(new
                {
                    schema_version = 1,
                    response = new
                    {
                        kind = "authority_writer_claimed",
                        payload = new
                        {
                            generation = 7,
                            writer_id = writerId,
                            replayed = false,
                        },
                    },
                });
            }
            else if (index == 1)
            {
                var command = frame
                    .GetProperty("request")
                    .GetProperty("request")
                    .GetProperty("payload");
                response = JsonSerializer.Serialize(new
                {
                    schema_version = 1,
                    response = new
                    {
                        kind = "command",
                        payload = new
                        {
                            status = "applied",
                            command_id =
                                command.GetProperty("command_id").GetString(),
                            revision = 1,
                            runtime = new
                            {
                                id = "runtime-fenced",
                                name = "Runtime Fenced",
                                endpoint = "https://runtime.example",
                                sidecar_endpoint = (string?)null,
                                registered_at_unix_ms = 1,
                                updated_at_unix_ms = 1,
                                revision = 1,
                                refresh_count = 0,
                                refresh_status = "never",
                                tags = new
                                {
                                    environment = (string?)null,
                                    cluster = (string?)null,
                                    role = (string?)null,
                                },
                                status = new
                                {
                                    status_source = "registration",
                                },
                                sidecar_status = (object?)null,
                                capabilities = new
                                {
                                    source = "",
                                    service = "",
                                    version = "",
                                    latest_snapshot = false,
                                    authenticated_deployment = false,
                                    serve_required = false,
                                    external_sidecar_context = false,
                                    target_path_segment_encoding = "",
                                    target_direct_path_chars = "",
                                    endpoints = Array.Empty<string>(),
                                    extensions = new Dictionary<string, bool>(),
                                },
                                capabilities_observed_for_revision = (ulong?)null,
                            },
                        },
                    },
                });
            }
            else if (index == 2)
            {
                response = JsonSerializer.Serialize(new
                {
                    schema_version = 1,
                    response = new
                    {
                        kind = "command",
                        payload = new
                        {
                            command_id = "writer-fenced-deploy",
                            status = "applied",
                        },
                    },
                });
            }
            else if (index == 3)
            {
                response = JsonSerializer.Serialize(new
                {
                    schema_version = 1,
                    response = new
                    {
                        kind = "deployment_receipt",
                        payload = new
                        {
                            command_id = "writer-fenced-deploy",
                            request_id = "writer-fenced-deploy",
                            status = "completed",
                            attempt = 1,
                            outcome = new
                            {
                                deployment_id =
                                    "writer-fenced-deployment",
                                request_id = "writer-fenced-deploy",
                                pipeline_kind = "capture/http",
                                requested_by = "operator",
                                status = "accepted",
                                accepted_unix_ms = 1700000000000,
                                target = "service-a",
                                replayed = false,
                            },
                        },
                    },
                });
            }
            else
            {
                var envelope = frame
                    .GetProperty("request")
                    .GetProperty("request")
                    .GetProperty("payload")
                    .GetProperty("envelope");
                response = JsonSerializer.Serialize(new
                {
                    schema_version = 1,
                    response = new
                    {
                        kind = "orchestra_persisted",
                        payload = new
                        {
                            envelope,
                            event_count = 1,
                        },
                    },
                });
            }
            var encoded = Encoding.UTF8.GetBytes(response + "\n");
            await client.SendAsync(encoded, SocketFlags.None);
            client.Shutdown(SocketShutdown.Send);
        }
    }

    private static async Task<byte[]> ReadFrameAsync(Socket socket)
    {
        using var output = new MemoryStream();
        var buffer = new byte[1024];
        while (true)
        {
            var read = await socket.ReceiveAsync(
                buffer,
                SocketFlags.None);
            Assert.True(read > 0);
            var newline = Array.IndexOf(buffer, (byte)'\n', 0, read);
            output.Write(
                buffer,
                0,
                newline < 0 ? read : newline);
            if (newline >= 0)
            {
                return output.ToArray();
            }
        }
    }

    private static ControlPlaneWriterFence CreateFence(
        ControlPlaneWriterLease lease) =>
        new(
            lease,
            NullLogger<ControlPlaneWriterFence>.Instance);

    private static ControlPlaneStateStore CreateStateStore(
        string statePath) =>
        new(
            new ConfigurationBuilder()
                .AddInMemoryCollection(
                    new Dictionary<string, string?>
                    {
                        ["LESERPENT_STATE_PATH"] = statePath,
                    })
                .Build(),
            new TestHostEnvironment
            {
                ContentRootPath =
                    Path.GetDirectoryName(statePath)!,
            },
            NullLogger<ControlPlaneStateStore>.Instance);

    private sealed class TestHostEnvironment : IHostEnvironment
    {
        public string EnvironmentName { get; set; } =
            Environments.Development;
        public string ApplicationName { get; set; } =
            "Leserpent.SecurityTests";
        public string ContentRootPath { get; set; } = string.Empty;
        public IFileProvider ContentRootFileProvider { get; set; } =
            new NullFileProvider();
    }
}
