using System.Net.Sockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Leserpent.ControlPlane;

public interface IRuntimeRegistrationAuthority
{
    bool Enabled { get; }

    Task<string> RegisterAsync(
        RuntimeRegistrationRequest request,
        string runtimeId,
        CancellationToken cancellationToken,
        bool update = false,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null);

    Task<RuntimeRegistrationCommitReceipt> RegisterWithReceiptAsync(
        RuntimeRegistrationRequest request,
        string runtimeId,
        CancellationToken cancellationToken,
        bool update = false,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null,
        ulong? expectedRevision = null) =>
        Task.FromResult(
            RuntimeRegistrationCommitReceipt.WithoutAuthoritativeCommit(
                runtimeId));

    Task SubmitDiscoveryAsync(
        string runtimeId,
        CancellationToken cancellationToken,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null);

    async Task<RuntimeDiscoveryIntakeReceipt> SubmitDiscoveryAtRevisionAsync(
        string runtimeId,
        ulong? expectedRevision,
        CancellationToken cancellationToken,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null)
    {
        await SubmitDiscoveryAsync(
            runtimeId,
            cancellationToken,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery);
        return RuntimeDiscoveryIntakeReceipt.WithoutAuthoritativeCommit(runtimeId);
    }

    Task UnregisterAsync(
        IReadOnlyCollection<string> runtimeIds,
        CancellationToken cancellationToken);

    Task UnregisterAsync(
        IReadOnlyCollection<string> runtimeIds,
        string commandId,
        CancellationToken cancellationToken) =>
        UnregisterAsync(runtimeIds, cancellationToken);

    Task<RuntimeUnregistrationReceiptLookup>
        LookupUnregistrationReceiptAsync(
            string commandId,
            CancellationToken cancellationToken) =>
        Task.FromResult(
            RuntimeUnregistrationReceiptLookup.Missing(commandId));
}

public sealed class RuntimeRegistrationCommitReceipt
{
    private RuntimeRegistrationCommitReceipt(
        string runtimeId,
        ulong? registrationRevision,
        DaemonRuntimeProjection? runtime,
        bool discoveryApplied)
    {
        RuntimeId = runtimeId;
        RegistrationRevision = registrationRevision;
        Runtime = runtime;
        DiscoveryApplied = discoveryApplied;
    }

    public string RuntimeId { get; }
    public ulong? RegistrationRevision { get; }
    public ulong? Revision => Runtime?.Revision;
    public DaemonRuntimeProjection? Runtime { get; }
    public bool DiscoveryApplied { get; }
    public bool Applied => Runtime is not null;

    internal static RuntimeRegistrationCommitReceipt WithoutAuthoritativeCommit(
        string runtimeId) =>
        new(runtimeId, null, null, false);

    public static RuntimeRegistrationCommitReceipt FromAuthoritativeCommit(
        ulong registrationRevision,
        DaemonRuntimeProjection runtime,
        bool discoveryApplied)
    {
        ArgumentNullException.ThrowIfNull(runtime);
        if (registrationRevision == 0
            || (discoveryApplied && runtime.Revision <= registrationRevision)
            || (!discoveryApplied && runtime.Revision != registrationRevision))
        {
            throw new ArgumentException(
                "registration receipt revisions are incoherent",
                nameof(runtime));
        }
        return new(
            runtime.RuntimeId,
            registrationRevision,
            runtime,
            discoveryApplied);
    }

    public override string ToString() =>
        $"RuntimeRegistrationCommitReceipt {{ RuntimeId = {RuntimeId}, RegistrationRevision = {RegistrationRevision?.ToString() ?? "managed"}, Revision = {Revision?.ToString() ?? "managed"}, DiscoveryApplied = {DiscoveryApplied}, Applied = {Applied} }}";
}

public sealed class RuntimeDiscoveryIntakeReceipt
{
    private RuntimeDiscoveryIntakeReceipt(
        string runtimeId,
        DaemonRuntimeProjection? runtime)
    {
        RuntimeId = runtimeId;
        Runtime = runtime;
    }

    public string RuntimeId { get; }
    public ulong? Revision => Runtime?.Revision;
    public DaemonRuntimeProjection? Runtime { get; }
    public bool Applied => Runtime is not null;

    internal static RuntimeDiscoveryIntakeReceipt WithoutAuthoritativeCommit(
        string runtimeId) =>
        new(runtimeId, null);

    public static RuntimeDiscoveryIntakeReceipt FromAuthoritativeCommit(
        DaemonRuntimeProjection runtime)
    {
        ArgumentNullException.ThrowIfNull(runtime);
        return new(runtime.RuntimeId, runtime);
    }

    public override string ToString() =>
        $"RuntimeDiscoveryIntakeReceipt {{ RuntimeId = {RuntimeId}, Revision = {Revision?.ToString() ?? "managed"}, Applied = {Applied} }}";
}

public sealed partial class DaemonRuntimeRegistrationAuthority :
    IRuntimeRegistrationAuthority,
    IDaemonRuntimeProjectionReader
{
    private const int MaxFrameBytes = 1024 * 1024 + 1024;
    private readonly string? socketPath;
    private readonly string? token;
    private readonly TimeSpan timeout;
    private readonly ControlPlaneWriterFence? writerFence;

    public DaemonRuntimeRegistrationAuthority(IConfiguration configuration)
        : this(configuration, null)
    {
    }

    public DaemonRuntimeRegistrationAuthority(
        IConfiguration configuration,
        ControlPlaneWriterFence? writerFence)
    {
        this.writerFence = writerFence;
        var configuredSocket = configuration["LESERPENT_DAEMON_SOCKET"];
        var configuredToken = configuration["LESERPENT_DAEMON_TOKEN"];
        if (string.IsNullOrWhiteSpace(configuredSocket) != string.IsNullOrWhiteSpace(configuredToken))
        {
            throw new InvalidOperationException(
                "LESERPENT_DAEMON_SOCKET and LESERPENT_DAEMON_TOKEN must be configured together");
        }
        if (!string.IsNullOrWhiteSpace(configuredSocket))
        {
            if (!Path.IsPathFullyQualified(configuredSocket))
            {
                throw new InvalidOperationException("LESERPENT_DAEMON_SOCKET must be an absolute path");
            }
            if (Encoding.UTF8.GetByteCount(configuredSocket) > 100)
            {
                throw new InvalidOperationException(
                    "LESERPENT_DAEMON_SOCKET must not exceed 100 UTF-8 bytes");
            }
            var tokenBytes = Encoding.UTF8.GetBytes(configuredToken!);
            if (tokenBytes.Length is < 32 or > 256 || tokenBytes.Any(value => value <= 0x20))
            {
                throw new InvalidOperationException(
                    "LESERPENT_DAEMON_TOKEN must contain 32 to 256 non-whitespace characters");
            }
            socketPath = Path.GetFullPath(configuredSocket);
            token = configuredToken;
        }

        var configuredTimeout = configuration.GetValue<int?>("LESERPENT_DAEMON_REGISTRATION_TIMEOUT_MS")
            ?? configuration.GetValue<int?>("LESERPENT_DAEMON_DEPLOY_TIMEOUT_MS")
            ?? 5000;
        timeout = TimeSpan.FromMilliseconds(Math.Clamp(configuredTimeout, 100, 30_000));
    }

    public bool Enabled => socketPath is not null;

    public async Task<string> RegisterAsync(
        RuntimeRegistrationRequest request,
        string runtimeId,
        CancellationToken cancellationToken,
        bool update = false,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
        (await RegisterWithReceiptAsync(
            request,
            runtimeId,
            cancellationToken,
            update,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery)).RuntimeId;

    public async Task<RuntimeRegistrationCommitReceipt> RegisterWithReceiptAsync(
        RuntimeRegistrationRequest request,
        string runtimeId,
        CancellationToken cancellationToken,
        bool update = false,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null,
        ulong? expectedRevision = null)
    {
        if (!Enabled)
        {
            throw new InvalidOperationException("leserpentd runtime registration authority is not configured");
        }
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("leserpentd Unix socket registration is unavailable on Windows");
        }

        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            if (!update && expectedRevision is not null)
            {
                throw new InvalidOperationException(
                    "a create registration cannot carry an expected revision");
            }
            if (expectedRevision == 0)
            {
                throw new InvalidOperationException(
                    "a registration update requires a non-zero expected revision");
            }
            var commandRevision = update
                ? expectedRevision
                    ?? await InspectRevisionAsync(runtimeId, deadline.Token)
                : null;
            var command = BuildCommand(request, runtimeId, commandRevision);
            using var response = await ExchangeAsync(command.Frame, deadline.Token);
            var registeredRuntime = ParseAppliedRuntimeProjection(
                response.RootElement,
                runtimeId,
                command.CommandId,
                "registration");
            ValidateRegistrationProjection(request, registeredRuntime);
            if (commandRevision is { } previousRevision
                && registeredRuntime.Revision <= previousRevision)
            {
                throw new InvalidOperationException(
                    "leserpentd registration receipt did not advance the runtime revision");
            }

            var (capabilitySnapshot, statusSnapshot, sidecarSnapshot) =
                BuildDiscoverySnapshots(capabilityDiscovery, statusDiscovery, sidecarDiscovery);
            var finalRuntime = registeredRuntime;
            var discoveryApplied = false;
            if (capabilitySnapshot is not null || statusSnapshot is not null || sidecarSnapshot is not null)
            {
                var intake = BuildDiscoveryIntakeCommand(
                    runtimeId,
                    registeredRuntime.Revision,
                    capabilitySnapshot,
                    statusSnapshot,
                    sidecarSnapshot);
                using var intakeResponse = await ExchangeAsync(
                    intake.Frame,
                    deadline.Token);
                var intakeReceipt = ParseDiscoveryIntakeReceipt(
                    intakeResponse.RootElement,
                    runtimeId,
                    intake.CommandId,
                    registeredRuntime.Revision);
                finalRuntime = intakeReceipt.Runtime
                    ?? throw new InvalidOperationException(
                        "leserpentd discovery intake receipt omitted its runtime projection");
                discoveryApplied = true;
            }
            return RuntimeRegistrationCommitReceipt.FromAuthoritativeCommit(
                registeredRuntime.Revision,
                finalRuntime,
                discoveryApplied);
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_registration_timeout",
                "leserpentd runtime registration timed out",
                error);
        }
        catch (Exception error) when (
            error is JsonException
                or KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd returned an invalid registration protocol response",
                error);
        }
    }

    public async Task SubmitDiscoveryAsync(
        string runtimeId,
        CancellationToken cancellationToken,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null) =>
        _ = await SubmitDiscoveryAtRevisionAsync(
            runtimeId,
            null,
            cancellationToken,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery);

    public async Task<RuntimeDiscoveryIntakeReceipt> SubmitDiscoveryAtRevisionAsync(
        string runtimeId,
        ulong? expectedRevision,
        CancellationToken cancellationToken,
        CapabilityDiscoveryResult? capabilityDiscovery = null,
        RuntimeStatusDiscoveryResult? statusDiscovery = null,
        RuntimeSidecarDiscoveryResult? sidecarDiscovery = null)
    {
        if (!Enabled)
        {
            return RuntimeDiscoveryIntakeReceipt.WithoutAuthoritativeCommit(runtimeId);
        }
        var (capabilitySnapshot, statusSnapshot, sidecarSnapshot) =
            BuildDiscoverySnapshots(capabilityDiscovery, statusDiscovery, sidecarDiscovery);
        if (capabilitySnapshot is null && statusSnapshot is null && sidecarSnapshot is null)
        {
            return RuntimeDiscoveryIntakeReceipt.WithoutAuthoritativeCommit(runtimeId);
        }
        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            var revision = expectedRevision
                ?? await InspectRevisionAsync(runtimeId, deadline.Token)
                ?? throw new InvalidOperationException("leserpentd lost the runtime before discovery intake");
            var command = BuildDiscoveryIntakeCommand(
                runtimeId,
                revision,
                capabilitySnapshot,
                statusSnapshot,
                sidecarSnapshot);
            using var response = await ExchangeAsync(command.Frame, deadline.Token);
            return ParseDiscoveryIntakeReceipt(
                response.RootElement,
                runtimeId,
                command.CommandId,
                revision);
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_registration_timeout",
                "leserpentd runtime discovery intake timed out",
                error);
        }
        catch (Exception error) when (
            error is JsonException
                or KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd returned an invalid runtime discovery intake response",
                error);
        }
    }

    public async Task UnregisterAsync(
        IReadOnlyCollection<string> runtimeIds,
        CancellationToken cancellationToken) =>
        await UnregisterAsync(
            runtimeIds,
            $"runtime-unregister-{Guid.NewGuid():N}",
            cancellationToken);

    public async Task UnregisterAsync(
        IReadOnlyCollection<string> runtimeIds,
        string commandId,
        CancellationToken cancellationToken)
    {
        if (!Enabled || runtimeIds.Count == 0)
        {
            return;
        }
        if (!ControlPlaneStateValidator.IsValidDeletionIdentifier(commandId))
        {
            throw new ArgumentException(
                "runtime unregistration command ID is invalid",
                nameof(commandId));
        }
        var uniqueRuntimeIds = runtimeIds
            .Select(runtimeId => runtimeId.Trim())
            .Where(runtimeId => runtimeId.Length > 0)
            .Distinct(StringComparer.Ordinal)
            .ToArray();
        if (uniqueRuntimeIds.Length != runtimeIds.Count || uniqueRuntimeIds.Length > 128)
        {
            throw new ArgumentException(
                "runtime unregistration requires between 1 and 128 unique runtime IDs",
                nameof(runtimeIds));
        }

        using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            var targets = new List<(string RuntimeId, ulong Revision)>(uniqueRuntimeIds.Length);
            foreach (var runtimeId in uniqueRuntimeIds)
            {
                var revision = await InspectRevisionAsync(runtimeId, deadline.Token);
                if (revision is not null)
                {
                    targets.Add((runtimeId, revision.Value));
                }
            }
            if (targets.Count == 0)
            {
                return;
            }

            using var response = await ExchangeAsync(
                BuildUnregisterRequest(commandId, targets),
                deadline.Token);
            var payload = RequireResponse(response.RootElement, "runtime_unregistered");
            if (!string.Equals(
                    payload.GetProperty("command_id").GetString(),
                    commandId,
                    StringComparison.Ordinal))
            {
                throw new InvalidOperationException(
                    "leserpentd returned a mismatched runtime unregistration result");
            }
            if (payload.TryGetProperty("operation_generation", out var generation)
                && generation.GetUInt64() == 0)
            {
                throw new InvalidOperationException(
                    "leserpentd returned an invalid runtime unregistration generation");
            }
            var removed = payload.GetProperty("removed")
                .EnumerateArray()
                .Select(target => target.GetProperty("runtime_id").GetString())
                .ToArray();
            if (removed.Length != targets.Count ||
                !removed.SequenceEqual(targets.Select(target => target.RuntimeId), StringComparer.Ordinal))
            {
                throw new InvalidOperationException(
                    "leserpentd returned mismatched runtime unregistration targets");
            }
        }
        catch (OperationCanceledException error) when (!cancellationToken.IsCancellationRequested)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_unregistration_timeout",
                "leserpentd runtime unregistration timed out",
                error);
        }
        catch (Exception error) when (
            error is KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd returned an invalid runtime unregistration response",
                error);
        }
    }

    public async Task<RuntimeUnregistrationReceiptLookup>
        LookupUnregistrationReceiptAsync(
            string commandId,
            CancellationToken cancellationToken)
    {
        if (!ControlPlaneStateValidator.IsValidDeletionIdentifier(commandId))
        {
            throw new ArgumentException(
                "runtime unregistration command ID is invalid",
                nameof(commandId));
        }
        if (!Enabled)
        {
            return RuntimeUnregistrationReceiptLookup.Missing(commandId);
        }

        using var deadline =
            CancellationTokenSource.CreateLinkedTokenSource(
                cancellationToken);
        deadline.CancelAfter(timeout);
        try
        {
            using var response = await ExchangeAsync(
                BuildUnregistrationReceiptRequest(commandId),
                deadline.Token);
            var payload = RequireResponse(
                response.RootElement,
                "runtime_unregistration_receipt");
            if (!string.Equals(
                    payload.GetProperty("command_id").GetString(),
                    commandId,
                    StringComparison.Ordinal))
            {
                throw new InvalidOperationException(
                    "leserpentd returned a mismatched unregistration receipt identity");
            }
            var replayHorizon = ValidateUnregistrationReplayHorizon(
                payload.GetProperty("replay_horizon"));
            var receipt = payload.GetProperty("receipt");
            if (receipt.ValueKind == JsonValueKind.Null)
            {
                return RuntimeUnregistrationReceiptLookup.Missing(
                    commandId,
                    replayHorizon);
            }
            if (receipt.ValueKind != JsonValueKind.Object)
            {
                throw new InvalidOperationException(
                    "leserpentd returned an invalid unregistration receipt");
            }

            var operationGeneration = receipt
                .GetProperty("operation_generation")
                .GetUInt64();
            var removed = receipt.GetProperty("removed")
                .EnumerateArray()
                .Select(target =>
                {
                    if (target.GetProperty("expected_revision").GetUInt64() == 0)
                    {
                        throw new InvalidOperationException(
                            "leserpentd returned an invalid receipt target revision");
                    }
                    return target.GetProperty("runtime_id").GetString()
                        ?? throw new InvalidOperationException(
                            "leserpentd returned an empty receipt target identity");
                })
                .ToArray();
            var deletedOrchestraRuntimeCount = receipt
                .GetProperty("deleted_orchestra_runtime_count")
                .GetUInt64();
            var deletedOrchestraRunCount = receipt
                .GetProperty("deleted_orchestra_run_count")
                .GetUInt64();
            var deletedOrchestraEventCount = receipt
                .GetProperty("deleted_orchestra_event_count")
                .GetUInt64();
            if (operationGeneration == 0 ||
                replayHorizon.OldestGeneration is null ||
                replayHorizon.NewestGeneration is null ||
                operationGeneration <
                    replayHorizon.OldestGeneration.Value ||
                operationGeneration >
                    replayHorizon.NewestGeneration.Value ||
                removed.Length is < 1 or > 128 ||
                removed.Distinct(StringComparer.Ordinal).Count() !=
                    removed.Length ||
                removed.Any(static runtimeId =>
                    !ControlPlaneStateValidator
                        .IsValidDeletionIdentifier(runtimeId)) ||
                deletedOrchestraRuntimeCount >
                    (ulong)removed.Length ||
                deletedOrchestraRunCount >
                    (ulong)removed.Length * 32 ||
                deletedOrchestraEventCount >
                    deletedOrchestraRunCount * 3 ||
                receipt.GetProperty("removed_at_unix_ms").GetInt64() < 0)
            {
                throw new InvalidOperationException(
                    "leserpentd returned inconsistent unregistration receipt bounds");
            }
            return new RuntimeUnregistrationReceiptLookup(
                commandId,
                Array.AsReadOnly(removed),
                operationGeneration,
                replayHorizon);
        }
        catch (OperationCanceledException error) when (
            !cancellationToken.IsCancellationRequested)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_unregistration_timeout",
                "leserpentd runtime unregistration receipt lookup timed out",
                error);
        }
        catch (Exception error) when (
            error is KeyNotFoundException
                or InvalidOperationException
                or FormatException
                or OverflowException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_invalid",
                "leserpentd returned an invalid runtime unregistration receipt",
                error);
        }
    }

    private static (
        RuntimeCapabilityAuthoritySnapshot? Capabilities,
        RuntimeStatusSnapshot? Status,
        RuntimeSidecarStatusSnapshot? SidecarStatus) BuildDiscoverySnapshots(
            CapabilityDiscoveryResult? capabilityDiscovery,
            RuntimeStatusDiscoveryResult? statusDiscovery,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery) =>
        (
            capabilityDiscovery?.AuthoritySnapshot,
            SanitizeRuntimeStatus(statusDiscovery?.Status),
            SanitizeSidecarStatus(sidecarDiscovery?.SidecarStatus)
        );

    private async Task<JsonDocument> ExchangeAsync(byte[] request, CancellationToken cancellationToken)
    {
        ValidateSocketBoundary();
        using var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        try
        {
            await socket.ConnectAsync(new UnixDomainSocketEndPoint(socketPath!), cancellationToken);
            using var stream = new NetworkStream(socket, ownsSocket: false);
            await stream.WriteAsync(request, cancellationToken);
            await stream.FlushAsync(cancellationToken);
            socket.Shutdown(SocketShutdown.Send);
            var response = await ReadFrameAsync(socket, cancellationToken);
            return JsonDocument.Parse(response);
        }
        catch (Exception error) when (error is SocketException or IOException or JsonException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_transport_failed",
                "leserpentd registration transport failed",
                error);
        }
    }

    private void ValidateSocketBoundary()
    {
        if (OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException(
                "leserpentd Unix socket registration is unavailable on Windows");
        }
        try
        {
            var attributes = File.GetAttributes(socketPath!);
            if ((attributes & FileAttributes.ReparsePoint) != 0)
            {
                throw new DaemonRuntimeRegistrationException(
                    "daemon_socket_unsafe",
                    "leserpentd socket must not be a symbolic link");
            }
            var mode = File.GetUnixFileMode(socketPath!);
            const UnixFileMode unsafePermissions = UnixFileMode.GroupRead
                | UnixFileMode.GroupWrite
                | UnixFileMode.GroupExecute
                | UnixFileMode.OtherRead
                | UnixFileMode.OtherWrite
                | UnixFileMode.OtherExecute;
            if ((mode & unsafePermissions) != 0)
            {
                throw new DaemonRuntimeRegistrationException(
                    "daemon_socket_unsafe",
                    "leserpentd socket must be owner-private");
            }
        }
        catch (DaemonRuntimeRegistrationException)
        {
            throw;
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_transport_failed",
                "leserpentd socket is unavailable",
                error);
        }
    }

    private static async Task<byte[]> ReadFrameAsync(Socket socket, CancellationToken cancellationToken)
    {
        using var output = new MemoryStream();
        var buffer = new byte[4096];
        while (true)
        {
            var read = await socket.ReceiveAsync(buffer, SocketFlags.None, cancellationToken);
            if (read == 0)
            {
                throw new IOException("leserpentd closed the socket before a complete frame");
            }
            var newline = Array.IndexOf(buffer, (byte)'\n', 0, read);
            var bodyBytes = newline < 0 ? read : newline;
            if (output.Length + bodyBytes > MaxFrameBytes)
            {
                throw new IOException("leserpentd response exceeds the protocol limit");
            }
            output.Write(buffer, 0, bodyBytes);
            if (newline >= 0)
            {
                if (newline != read - 1)
                {
                    throw new IOException("leserpentd returned data after the protocol frame");
                }
                return output.ToArray();
            }
        }
    }

    private (byte[] Frame, string CommandId) BuildCommand(
        RuntimeRegistrationRequest request,
        string runtimeId,
        ulong? expectedRevision)
    {
        var tags = request.Tags ?? new RuntimeTags(null, null, null);
        var stableCommandId = BuildDeterministicCommandId(
            runtimeId,
            request.Name,
            request.Endpoint,
            request.SidecarEndpoint,
            expectedRevision is not null);
        var frame = BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "command");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteString("command_id", stableCommandId);
            writer.WriteString("idempotency_key", stableCommandId);
            if (expectedRevision is null)
            {
                writer.WriteNull("expected_revision");
            }
            else
            {
                writer.WriteNumber("expected_revision", expectedRevision.Value);
            }
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer);
            writer.WriteString("origin", "compatibility_adapter");
            writer.WriteString("confirmation", "confirmed");
            writer.WriteBoolean("dry_run", false);
            writer.WritePropertyName("command");
            writer.WriteStartObject();
            writer.WriteString(
                "kind",
                expectedRevision is null ? "runtime_register" : "runtime_registration_update");
            writer.WriteString("runtime_id", runtimeId);
            writer.WriteString("name", request.Name.Trim());
            writer.WriteString("endpoint", request.Endpoint.Trim());
            WriteOptionalString(writer, "sidecar_endpoint", request.SidecarEndpoint?.Trim());
            writer.WritePropertyName("tags");
            writer.WriteStartObject();
            WriteOptionalString(writer, "environment", tags.Environment?.Trim());
            WriteOptionalString(writer, "cluster", tags.Cluster?.Trim());
            WriteOptionalString(writer, "role", tags.Role?.Trim());
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
        return (frame, stableCommandId);
    }

    private async Task<ulong?> InspectRevisionAsync(
        string runtimeId,
        CancellationToken cancellationToken)
    {
        var query = BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "query");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer, "runtime.read");
            writer.WritePropertyName("query");
            writer.WriteStartObject();
            writer.WriteString("kind", "runtime_inspect");
            writer.WriteString("runtime_id", runtimeId);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
        using var response = await ExchangeAsync(query, cancellationToken);
        var root = response.RootElement;
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unsupported protocol version");
        }
        var protocolResponse = root.GetProperty("response");
        var kind = protocolResponse.GetProperty("kind").GetString();
        if (string.Equals(kind, "error", StringComparison.Ordinal))
        {
            var error = protocolResponse.GetProperty("payload");
            var code = error.GetProperty("code").GetString();
            if (string.Equals(code, "runtime_not_found", StringComparison.Ordinal))
            {
                return null;
            }
            throw new DaemonRuntimeRegistrationException(
                code ?? "daemon_request_failed",
                error.GetProperty("message").GetString() ?? "leserpentd rejected the registration query");
        }
        if (!string.Equals(kind, "query", StringComparison.Ordinal))
        {
            throw new InvalidOperationException("leserpentd returned an unexpected registration query response");
        }
        var payload = protocolResponse.GetProperty("payload");
        if (!string.Equals(payload.GetProperty("kind").GetString(), "runtime_inspect", StringComparison.Ordinal))
        {
            throw new InvalidOperationException("leserpentd returned an unexpected runtime query payload");
        }
        var runtime = payload.GetProperty("runtime");
        if (!string.Equals(runtime.GetProperty("id").GetString(), runtimeId, StringComparison.Ordinal))
        {
            throw new InvalidOperationException("leserpentd runtime query returned a different runtime");
        }
        return runtime.GetProperty("revision").GetUInt64();
    }

    private (byte[] Frame, string CommandId) BuildDiscoveryIntakeCommand(
        string runtimeId,
        ulong expectedRevision,
        RuntimeCapabilityAuthoritySnapshot? capabilities,
        RuntimeStatusSnapshot? status,
        RuntimeSidecarStatusSnapshot? sidecarStatus)
    {
        var commandId = BuildDiscoveryCommandId(
            runtimeId,
            expectedRevision,
            capabilities,
            status,
            sidecarStatus);
        var frame = BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "command");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteString("command_id", commandId);
            writer.WriteString("idempotency_key", commandId);
            writer.WriteNumber("expected_revision", expectedRevision);
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer, "runtime.register");
            writer.WriteString("origin", "compatibility_adapter");
            writer.WriteString("confirmation", "confirmed");
            writer.WriteBoolean("dry_run", false);
            writer.WritePropertyName("command");
            writer.WriteStartObject();
            writer.WriteString("kind", "runtime_discovery_intake");
            writer.WriteString("runtime_id", runtimeId);
            WriteCapabilitySnapshot(writer, capabilities);
            WriteStatusSnapshot(writer, status);
            WriteSidecarStatusSnapshot(writer, sidecarStatus);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
        return (frame, commandId);
    }

    private byte[] BuildUnregisterRequest(
        string commandId,
        IReadOnlyList<(string RuntimeId, ulong Revision)> targets)
    {
        return BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString("kind", "runtime_unregister");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer, "runtime.unregister");
            writer.WriteString("command_id", commandId);
            writer.WritePropertyName("targets");
            writer.WriteStartArray();
            foreach (var target in targets)
            {
                writer.WriteStartObject();
                writer.WriteString("runtime_id", target.RuntimeId);
                writer.WriteNumber("expected_revision", target.Revision);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteBoolean("confirmed", true);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
    }

    private byte[] BuildUnregistrationReceiptRequest(string commandId)
    {
        return BuildFrame(writer =>
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WritePropertyName("request");
            writer.WriteStartObject();
            writer.WriteString(
                "kind",
                "runtime_unregistration_receipt");
            writer.WritePropertyName("payload");
            writer.WriteStartObject();
            WritePrincipal(writer, "operator");
            WriteCapabilities(writer, "runtime.read");
            writer.WriteString("command_id", commandId);
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndObject();
        });
    }

    private static RuntimeUnregistrationReplayHorizon
        ValidateUnregistrationReplayHorizon(
        JsonElement horizon)
    {
        var capacity = horizon.GetProperty("capacity").GetUInt64();
        var retained = horizon.GetProperty("retained").GetUInt64();
        var nextGeneration = horizon
            .GetProperty("next_generation")
            .GetUInt64();
        var oldest = OptionalUInt64(
            horizon.GetProperty("oldest_generation"));
        var newest = OptionalUInt64(
            horizon.GetProperty("newest_generation"));
        var evictedThrough = horizon
            .GetProperty("evicted_through_generation")
            .GetUInt64();
        var contiguous = oldest is { } oldestGeneration &&
            newest is { } newestGeneration
                ? retained > 0 &&
                    evictedThrough < ulong.MaxValue &&
                    oldestGeneration == evictedThrough + 1 &&
                    newestGeneration < ulong.MaxValue &&
                    nextGeneration == newestGeneration + 1 &&
                    newestGeneration >= oldestGeneration &&
                    retained ==
                        newestGeneration - oldestGeneration + 1
                : oldest is null &&
                    newest is null &&
                    retained == 0 &&
                    evictedThrough < ulong.MaxValue &&
                    nextGeneration == evictedThrough + 1;
        if (capacity == 0 ||
            retained > capacity ||
            nextGeneration == 0 ||
            evictedThrough >= nextGeneration ||
            !contiguous)
        {
            throw new InvalidOperationException(
                "leserpentd returned an invalid unregistration replay horizon");
        }
        return new RuntimeUnregistrationReplayHorizon(
            capacity,
            retained,
            oldest,
            newest,
            nextGeneration,
            evictedThrough);
    }

    private static ulong? OptionalUInt64(JsonElement value) =>
        value.ValueKind == JsonValueKind.Null
            ? null
            : value.GetUInt64();

    private static void WriteCapabilitySnapshot(
        Utf8JsonWriter writer,
        RuntimeCapabilityAuthoritySnapshot? snapshot)
    {
        if (snapshot is null)
        {
            writer.WriteNull("capabilities");
            return;
        }
        writer.WritePropertyName("capabilities");
        writer.WriteStartObject();
        writer.WriteString("source", snapshot.Source);
        writer.WriteString("service", snapshot.Service);
        writer.WriteString("version", snapshot.Version);
        writer.WriteBoolean("latest_snapshot", snapshot.LatestSnapshot);
        writer.WriteBoolean("authenticated_deployment", snapshot.AuthenticatedDeployment);
        writer.WriteBoolean("serve_required", snapshot.ServeRequired);
        writer.WriteBoolean("external_sidecar_context", snapshot.ExternalSidecarContext);
        writer.WriteString("target_path_segment_encoding", snapshot.TargetPathSegmentEncoding);
        writer.WriteString("target_direct_path_chars", snapshot.TargetDirectPathChars);
        writer.WritePropertyName("endpoints");
        writer.WriteStartArray();
        foreach (var endpoint in snapshot.Endpoints)
        {
            writer.WriteStringValue(endpoint);
        }
        writer.WriteEndArray();
        writer.WritePropertyName("extensions");
        writer.WriteStartObject();
        foreach (var extension in snapshot.Extensions.OrderBy(item => item.Key, StringComparer.Ordinal))
        {
            writer.WriteBoolean(extension.Key, extension.Value);
        }
        writer.WriteEndObject();
        writer.WriteEndObject();
    }

    private static void WriteStatusSnapshot(Utf8JsonWriter writer, RuntimeStatusSnapshot? status)
    {
        if (status is null)
        {
            writer.WriteNull("status");
            return;
        }
        writer.WritePropertyName("status");
        writer.WriteStartObject();
        writer.WriteString("status_source", status.StatusSource);
        WriteOptionalString(writer, "status_fetched_at", status.StatusFetchedAt?.ToString("O"));
        WriteOptionalString(writer, "status_fetch_error", status.StatusFetchError);
        writer.WriteBoolean("has_latest_snapshot", status.HasLatestSnapshot);
        WriteOptionalString(writer, "snapshot_kind", status.SnapshotKind);
        WriteOptionalNumber(writer, "target_count", status.TargetCount);
        writer.WriteBoolean("has_summary_json", status.HasSummaryJson);
        writer.WriteBoolean("has_analysis_json", status.HasAnalysisJson);
        writer.WriteBoolean("has_training_example_json", status.HasTrainingExampleJson);
        writer.WriteBoolean("has_training_dataset_manifest", status.HasTrainingDatasetManifest);
        writer.WriteBoolean("has_export_json", status.HasExportJson);
        writer.WriteBoolean("has_report_json", status.HasReportJson);
        writer.WriteBoolean("has_report_html", status.HasReportHtml);
        writer.WriteBoolean("has_external_sidecar_context", status.HasExternalSidecarContext);
        writer.WriteBoolean("has_external_evidence_chain_enrichment", status.HasExternalEvidenceChainEnrichment);
        writer.WriteBoolean("has_external_diagnostic_opinion", status.HasExternalDiagnosticOpinion);
        writer.WriteBoolean("resilience_degraded", status.ResilienceDegraded);
        WriteOptionalString(writer, "resilience_status", status.ResilienceStatus);
        WriteOptionalString(writer, "resilience_summary", status.ResilienceSummary);
        WriteOptionalString(writer, "socket_service_status", status.SocketServiceStatus);
        WriteOptionalNumber(writer, "socket_consecutive_idle_timeouts", status.SocketConsecutiveIdleTimeouts);
        WriteOptionalNumber(writer, "socket_total_idle_timeouts", status.SocketTotalIdleTimeouts);
        writer.WriteEndObject();
    }

    private static void WriteSidecarStatusSnapshot(
        Utf8JsonWriter writer,
        RuntimeSidecarStatusSnapshot? status)
    {
        if (status is null)
        {
            writer.WriteNull("sidecar_status");
            return;
        }
        writer.WritePropertyName("sidecar_status");
        writer.WriteStartObject();
        writer.WriteString("status_source", status.StatusSource);
        WriteOptionalString(writer, "status_fetched_at", status.StatusFetchedAt?.ToString("O"));
        WriteOptionalString(writer, "status_fetch_error", status.StatusFetchError);
        writer.WriteBoolean("healthy", status.Healthy);
        writer.WriteString("daemon_status", status.DaemonStatus);
        WriteOptionalNumber(writer, "target_count", status.TargetCount);
        writer.WriteBoolean("learning_active", status.LearningActive);
        writer.WriteNumber("learned_routes", status.LearnedRoutes);
        writer.WriteBoolean("has_evidence_chain_enrichment", status.HasEvidenceChainEnrichment);
        writer.WriteBoolean("has_diagnostic_opinion", status.HasDiagnosticOpinion);
        WriteOptionalString(writer, "last_error", status.LastError);
        WriteSidecarMemorySnapshot(writer, status.Memory);
        writer.WriteEndObject();
    }

    private static void WriteSidecarMemorySnapshot(
        Utf8JsonWriter writer,
        RuntimeSidecarMemorySnapshot? memory)
    {
        if (memory is null)
        {
            writer.WriteNull("memory");
            return;
        }
        writer.WritePropertyName("memory");
        writer.WriteStartObject();
        writer.WriteBoolean("versions_supported", memory.VersionsSupported);
        writer.WriteNumber("slot_count", memory.SlotCount);
        writer.WriteNumber("history_count", memory.HistoryCount);
        WriteOptionalString(writer, "latest_slot", memory.LatestSlot);
        WriteOptionalString(writer, "latest_label", memory.LatestLabel);
        WriteOptionalString(writer, "latest_source", memory.LatestSource);
        writer.WritePropertyName("slots");
        writer.WriteStartArray();
        foreach (var slot in memory.Slots.Take(128))
        {
            writer.WriteStartObject();
            writer.WriteString("slot", slot.Slot);
            WriteOptionalString(writer, "label", slot.Label);
            WriteOptionalString(writer, "note", slot.Note);
            writer.WriteString("source", slot.Source);
            WriteOptionalString(writer, "saved_at", slot.SavedAt?.ToString("O"));
            writer.WriteNumber("pattern_count", slot.PatternCount);
            writer.WriteNumber("label_count", slot.LabelCount);
            writer.WriteEndObject();
        }
        writer.WriteEndArray();
        WriteOptionalString(
            writer,
            "fetch_error",
            memory.FetchError is null ? null : "sidecar_memory_fetch_failed");
        writer.WriteEndObject();
    }

    private static RuntimeStatusSnapshot? SanitizeRuntimeStatus(RuntimeStatusSnapshot? status)
    {
        if (status is null)
        {
            return null;
        }
        if (status.StatusSource == "gewyvern-api"
            && status.StatusFetchedAt is not null
            && status.StatusFetchError is null)
        {
            return status;
        }
        return new RuntimeStatusSnapshot(
            "fetch_failed",
            null,
            "runtime_status_fetch_failed",
            false,
            null,
            null,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false);
    }

    private static RuntimeSidecarStatusSnapshot? SanitizeSidecarStatus(
        RuntimeSidecarStatusSnapshot? status)
    {
        if (status is null)
        {
            return null;
        }
        if (status.StatusSource == "etragon-api" && status.StatusFetchError is null)
        {
            return status with
            {
                LastError = string.IsNullOrWhiteSpace(status.LastError)
                    ? null
                    : "sidecar_reported_error"
            };
        }
        return new RuntimeSidecarStatusSnapshot(
            "fetch_failed",
            null,
            "sidecar_fetch_failed",
            false,
            "fetch_failed",
            null,
            false,
            0,
            false,
            false,
            "sidecar_fetch_failed");
    }

    private byte[] BuildFrame(Action<Utf8JsonWriter> writeRequest)
    {
        using var output = new MemoryStream();
        using (var writer = new Utf8JsonWriter(output))
        {
            writer.WriteStartObject();
            writer.WriteString("token", token);
            AuthorityWriterFrame.Write(
                writer,
                writerFence?.AuthorityTicket);
            writer.WritePropertyName("request");
            writeRequest(writer);
            writer.WriteEndObject();
        }
        if (output.Length + 1 > MaxFrameBytes)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_request_oversized",
                "leserpentd registration request exceeds the protocol limit");
        }
        output.WriteByte((byte)'\n');
        return output.ToArray();
    }

    private static DaemonRuntimeProjection ParseAppliedRuntimeProjection(
        JsonElement root,
        string expectedRuntimeId,
        string expectedCommandId,
        string operation)
    {
        var payload = RequireResponse(root, "command");
        if (!string.Equals(
                payload.GetProperty("status").GetString(),
                "applied",
                StringComparison.Ordinal)
            || !string.Equals(
                payload.GetProperty("command_id").GetString(),
                expectedCommandId,
                StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                $"leserpentd {operation} response is invalid");
        }
        var decoded = JsonSerializer.Deserialize(
            payload.GetProperty("runtime").GetRawText(),
            DaemonRuntimeProjectionJsonContext.Default.DaemonRuntimeProjectionPayload)
            ?? throw new InvalidOperationException(
                $"leserpentd {operation} response omitted its runtime projection");
        var runtime = ConvertProjection(decoded);
        if (!string.Equals(
                runtime.RuntimeId,
                expectedRuntimeId,
                StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                $"leserpentd {operation} response returned another runtime");
        }
        if (payload.TryGetProperty("revision", out var envelopeRevision)
            && envelopeRevision.GetUInt64() != runtime.Revision)
        {
            throw new InvalidOperationException(
                $"leserpentd {operation} response returned an incoherent revision");
        }
        return runtime;
    }

    private static void ValidateRegistrationProjection(
        RuntimeRegistrationRequest request,
        DaemonRuntimeProjection runtime)
    {
        var tags = request.Tags ?? new RuntimeTags(null, null, null);
        if (!string.Equals(runtime.Name, request.Name.Trim(), StringComparison.Ordinal)
            || !string.Equals(runtime.Endpoint, request.Endpoint.Trim(), StringComparison.Ordinal)
            || !string.Equals(
                runtime.SidecarEndpoint,
                NormalizeOptionalRegistrationValue(request.SidecarEndpoint),
                StringComparison.Ordinal)
            || !string.Equals(
                runtime.Tags.Environment,
                NormalizeOptionalRegistrationValue(tags.Environment),
                StringComparison.Ordinal)
            || !string.Equals(
                runtime.Tags.Cluster,
                NormalizeOptionalRegistrationValue(tags.Cluster),
                StringComparison.Ordinal)
            || !string.Equals(
                runtime.Tags.Role,
                NormalizeOptionalRegistrationValue(tags.Role),
                StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                "leserpentd registration response does not match the requested identity");
        }
    }

    private static string? NormalizeOptionalRegistrationValue(string? value) =>
        string.IsNullOrWhiteSpace(value) ? null : value.Trim();

    private static RuntimeDiscoveryIntakeReceipt ParseDiscoveryIntakeReceipt(
        JsonElement root,
        string expectedRuntimeId,
        string expectedCommandId,
        ulong expectedRevision)
    {
        var runtime = ParseAppliedRuntimeProjection(
            root,
            expectedRuntimeId,
            expectedCommandId,
            "discovery intake");
        if (runtime.Revision <= expectedRevision)
        {
            throw new InvalidOperationException(
                "leserpentd discovery intake receipt did not advance the runtime revision");
        }
        return RuntimeDiscoveryIntakeReceipt.FromAuthoritativeCommit(runtime);
    }

    private static JsonElement RequireResponse(JsonElement root, string expectedKind)
    {
        if (root.GetProperty("schema_version").GetInt32() != 1)
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unsupported protocol version");
        }
        var response = root.GetProperty("response");
        var kind = response.GetProperty("kind").GetString();
        if (string.Equals(kind, "error", StringComparison.Ordinal))
        {
            var error = response.GetProperty("payload");
            throw new DaemonRuntimeRegistrationException(
                error.GetProperty("code").GetString() ?? "daemon_request_failed",
                error.GetProperty("message").GetString() ?? "leserpentd rejected the registration request");
        }
        if (!string.Equals(kind, expectedKind, StringComparison.Ordinal))
        {
            throw new DaemonRuntimeRegistrationException(
                "daemon_protocol_mismatch",
                "leserpentd returned an unexpected response kind");
        }
        return response.GetProperty("payload");
    }

    private static void WritePrincipal(Utf8JsonWriter writer, string principal)
    {
        writer.WritePropertyName("principal");
        writer.WriteStartObject();
        writer.WriteString("id", principal);
        writer.WriteEndObject();
    }

    private static void WriteCapabilities(Utf8JsonWriter writer, string capability = "runtime.register")
    {
        writer.WritePropertyName("capabilities");
        writer.WriteStartArray();
        writer.WriteStringValue(capability);
        writer.WriteEndArray();
    }

    private static void WriteOptionalString(Utf8JsonWriter writer, string name, string? value)
    {
        if (value is null)
        {
            writer.WriteNull(name);
        }
        else
        {
            writer.WriteString(name, value);
        }
    }

    private static void WriteOptionalNumber(Utf8JsonWriter writer, string name, int? value)
    {
        if (value is null)
        {
            writer.WriteNull(name);
        }
        else
        {
            writer.WriteNumber(name, value.Value);
        }
    }

    private static string BuildDeterministicCommandId(
        string runtimeId,
        string name,
        string endpoint,
        string? sidecarEndpoint,
        bool update)
    {
        var normalizedName = name.Trim();
        var normalizedEndpoint = endpoint.Trim();
        var normalizedSidecarEndpoint = sidecarEndpoint?.Trim() ?? string.Empty;
        var prefix = update ? "update|" : string.Empty;
        var bytes = HashData(
            $"{prefix}{runtimeId}|{normalizedName}|{normalizedEndpoint}|{normalizedSidecarEndpoint}");
        return Convert.ToHexString(bytes).ToLowerInvariant().Substring(0, 32);
    }

    private static string BuildDiscoveryCommandId(
        string runtimeId,
        ulong expectedRevision,
        RuntimeCapabilityAuthoritySnapshot? capabilities,
        RuntimeStatusSnapshot? status,
        RuntimeSidecarStatusSnapshot? sidecarStatus)
    {
        var value = string.Join(
            '|',
            "discovery",
            runtimeId,
            expectedRevision,
            BuildSnapshotDigest(
                capabilities,
                static (writer, snapshot) => WriteCapabilitySnapshot(writer, snapshot)),
            BuildSnapshotDigest(
                status,
                static (writer, snapshot) => WriteStatusSnapshot(writer, snapshot)),
            BuildSnapshotDigest(
                sidecarStatus,
                static (writer, snapshot) => WriteSidecarStatusSnapshot(writer, snapshot)));
        return Convert.ToHexString(HashData(value)).ToLowerInvariant()[..32];
    }

    private static string BuildSnapshotDigest<T>(
        T? snapshot,
        Action<Utf8JsonWriter, T?> writeSnapshot)
        where T : class
    {
        if (snapshot is null)
        {
            return string.Empty;
        }
        using var output = new MemoryStream();
        using (var writer = new Utf8JsonWriter(output))
        {
            writer.WriteStartObject();
            writeSnapshot(writer, snapshot);
            writer.WriteEndObject();
        }
        return Convert.ToHexString(SHA256.HashData(output.ToArray())).ToLowerInvariant();
    }

    private static byte[] HashData(string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        return SHA256.HashData(bytes);
    }
}

public sealed class DaemonRuntimeRegistrationException : Exception
{
    public DaemonRuntimeRegistrationException(string code, string message, Exception? innerException = null)
        : base(message, innerException)
    {
        Code = code;
    }

    public string Code { get; }
}
