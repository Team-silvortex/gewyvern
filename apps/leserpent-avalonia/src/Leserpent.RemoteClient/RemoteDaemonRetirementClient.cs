using System.Text;
using System.Text.Json;

public sealed record RemoteDaemonRetirementIntent(
    string RetirementId,
    string BootstrapId,
    string RetirementCredentialHandle,
    string RequestedBy);

public sealed record RemoteDaemonRetirementSnapshot(
    string RetirementId,
    string BootstrapId,
    string DaemonId,
    string Phase,
    string Transport,
    string Host,
    ushort Port,
    string Generation,
    string InstallProfile,
    bool RetirementCredentialPresent,
    bool ServiceRetired,
    string? FaultCode)
{
    public bool IsTerminal => Phase is "service_retired" or "failed";
}

public sealed class RemoteDaemonRetirementClient : IDisposable
{
    public const int MaxMessageBytes = 64 * 1024;
    private const string Capability = "host.retire";
    private readonly RemoteWireTransport transport;

    public RemoteDaemonRetirementClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteDaemonRetirementSnapshot> ReconcileAsync(
        RemoteDaemonRetirementIntent intent,
        CancellationToken cancellationToken = default)
    {
        var payload = EncodeRequest(intent);
        var response = await transport.PostDaemonRetirementAsync(
            payload,
            "daemon retirement",
            cancellationToken).ConfigureAwait(false);
        return DecodeResponse(response, intent);
    }

    public void Dispose() => transport.Dispose();

    internal static byte[] EncodeRequest(RemoteDaemonRetirementIntent intent)
    {
        ValidateIntent(intent);
        return JsonSerializer.SerializeToUtf8Bytes(
            new DaemonRetirementRequestEnvelope
            {
                Request = new DaemonRetirementRequest
                {
                    Principal = new RemotePrincipal { Id = intent.RequestedBy },
                    Capabilities = [Capability],
                    Intent = new DaemonRetirementIntent
                    {
                        RetirementId = intent.RetirementId,
                        BootstrapId = intent.BootstrapId,
                        RetirementCredentialHandle = intent.RetirementCredentialHandle,
                        RequestedBy = intent.RequestedBy,
                    },
                },
            },
            RemoteDaemonRetirementJsonContext.Default.DaemonRetirementRequestEnvelope);
    }

    public static void VerifyContract()
    {
        var intent = new RemoteDaemonRetirementIntent(
            "retire-daemon-ui-1",
            "bootstrap-ui-1",
            "vault:ssh:daemon-example",
            "avalonia-hub");
        var encoded = Encoding.UTF8.GetString(EncodeRequest(intent));
        if (!encoded.Contains("\"capabilities\":[\"host.retire\"]", StringComparison.Ordinal)
            || !encoded.Contains("\"confirmed\":true", StringComparison.Ordinal)
            || !encoded.Contains(
                "\"retirement_credential_handle\":\"vault:ssh:daemon-example\"",
                StringComparison.Ordinal)
            || encoded.Contains("\"target\":", StringComparison.Ordinal)
            || encoded.Contains("\"daemon_id\":", StringComparison.Ordinal)
            || encoded.Contains("\"generation\":", StringComparison.Ordinal)
            || encoded.Contains("\"install_profile\":", StringComparison.Ordinal)
            || encoded.Contains("password", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("private_key", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("runtime.retire", StringComparison.Ordinal))
        {
            throw new InvalidDataException("daemon retirement request contract drifted");
        }

        const string planned = """
            {"schema_version":1,"response":{"kind":"state","payload":{"retirement_id":"retire-daemon-ui-1","bootstrap_id":"bootstrap-ui-1","daemon_id":"daemon-target","phase":"planned","target":{"transport":"ssh","host":"daemon.example","port":22},"generation":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","install_profile":"system","retirement_credential_present":true,"service_retired":false,"fault_code":null}}}
            """;
        var state = DecodeResponse(Encoding.UTF8.GetBytes(planned), intent);
        if (state.Phase != "planned" || state.IsTerminal || state.ServiceRetired)
        {
            throw new InvalidDataException("planned daemon retirement projection drifted");
        }

        const string completed = """
            {"schema_version":1,"response":{"kind":"state","payload":{"retirement_id":"retire-daemon-ui-1","bootstrap_id":"bootstrap-ui-1","daemon_id":"daemon-target","phase":"service_retired","target":{"transport":"ssh","host":"daemon.example","port":22},"generation":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","install_profile":"system","retirement_credential_present":false,"service_retired":true,"fault_code":null}}}
            """;
        state = DecodeResponse(Encoding.UTF8.GetBytes(completed), intent);
        if (!state.IsTerminal || !state.ServiceRetired)
        {
            throw new InvalidDataException("completed daemon retirement projection drifted");
        }

        const string failed = """
            {"schema_version":1,"response":{"kind":"state","payload":{"retirement_id":"retire-daemon-ui-1","bootstrap_id":"bootstrap-ui-1","daemon_id":"daemon-target","phase":"failed","target":{"transport":"ssh","host":"daemon.example","port":22},"generation":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","install_profile":"system","retirement_credential_present":false,"service_retired":false,"fault_code":"service_stop_failed"}}}
            """;
        state = DecodeResponse(Encoding.UTF8.GetBytes(failed), intent);
        if (!state.IsTerminal || state.ServiceRetired || state.FaultCode != "service_stop_failed")
        {
            throw new InvalidDataException("failed daemon retirement projection drifted");
        }

        ExpectInvalid(
            () => DecodeResponse(
                Encoding.UTF8.GetBytes(
                    planned.Replace("bootstrap-ui-1", "bootstrap-other", StringComparison.Ordinal)),
                intent),
            "daemon retirement response crossed its bootstrap identity fence");
        ExpectInvalid(
            () => DecodeResponse(
                Encoding.UTF8.GetBytes(
                    planned.Replace(
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "not-a-generation",
                        StringComparison.Ordinal)),
                intent),
            "daemon retirement response accepted a malformed derived generation");
        ExpectInvalid(
            () => ValidateIntent(intent with { RetirementCredentialHandle = "ssh-password" }),
            "daemon retirement client accepted a raw credential source");
    }

    private static RemoteDaemonRetirementSnapshot DecodeResponse(
        ReadOnlySpan<byte> payload,
        RemoteDaemonRetirementIntent expected)
    {
        RequireBound(payload);
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteDaemonRetirementJsonContext.Default.DaemonRetirementResponseEnvelope)
                ?? throw new InvalidDataException("daemon retirement response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported daemon retirement response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw ProtocolError(envelope.Response.Payload, expected.RetirementId);
            }
            if (envelope.Response.Kind != "state")
            {
                throw new InvalidDataException(
                    "daemon retirement returned an unexpected response kind");
            }
            var state = envelope.Response.Payload.Deserialize(
                RemoteDaemonRetirementJsonContext.Default.DaemonRetirementSnapshotPayload)
                ?? throw new InvalidDataException("daemon retirement response state is empty");
            ValidateSnapshot(state, expected);
            return new RemoteDaemonRetirementSnapshot(
                state.RetirementId,
                state.BootstrapId,
                state.DaemonId,
                state.Phase,
                state.Target.Transport,
                state.Target.Host,
                state.Target.Port,
                state.Generation,
                state.InstallProfile,
                state.RetirementCredentialPresent,
                state.ServiceRetired,
                state.FaultCode);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("daemon retirement response JSON is invalid", error);
        }
    }

    private static RemoteDaemonRetirementException ProtocolError(
        JsonElement payload,
        string expectedRetirementId)
    {
        var error = payload.Deserialize(
            RemoteDaemonRetirementJsonContext.Default.DaemonRetirementErrorPayload)
            ?? throw new InvalidDataException("daemon retirement protocol error is empty");
        if (error.RetirementId is not null && error.RetirementId != expectedRetirementId)
        {
            throw new InvalidDataException("daemon retirement error crossed its identity fence");
        }
        if (!ValidCode(error.Code)
            || string.IsNullOrWhiteSpace(error.Message)
            || error.Message != error.Message.Trim()
            || error.Message.Length > 512
            || error.Message.Any(char.IsControl))
        {
            throw new InvalidDataException("daemon retirement protocol error is invalid");
        }
        return new RemoteDaemonRetirementException(error.Code, error.Message);
    }

    private static void ValidateIntent(RemoteDaemonRetirementIntent intent)
    {
        RequireIdentifier(intent.RetirementId, "retirement ID");
        RequireIdentifier(intent.BootstrapId, "bootstrap ID");
        RequireIdentifier(intent.RequestedBy, "principal");
        RequireHandle(intent.RetirementCredentialHandle, "ssh");
    }

    private static void ValidateSnapshot(
        DaemonRetirementSnapshotPayload state,
        RemoteDaemonRetirementIntent expected)
    {
        RequireIdentifier(state.RetirementId, "retirement ID");
        RequireIdentifier(state.BootstrapId, "bootstrap ID");
        RequireIdentifier(state.DaemonId, "daemon ID");
        if (state.RetirementId != expected.RetirementId
            || state.BootstrapId != expected.BootstrapId
            || state.Target.Transport != "ssh")
        {
            throw new InvalidDataException("daemon retirement response identity is invalid");
        }
        ValidateTarget(state.Target.Host, state.Target.Port);
        if (state.Generation.Length != 64
            || !state.Generation.All(character =>
                char.IsAsciiDigit(character) || character is >= 'a' and <= 'f'))
        {
            throw new InvalidDataException("daemon retirement generation is invalid");
        }
        if (state.InstallProfile is not ("system" or "user" or "test"))
        {
            throw new InvalidDataException("daemon retirement install profile is invalid");
        }
        switch (state.Phase)
        {
            case "planned" or "retiring_service"
                when state.RetirementCredentialPresent && !state.ServiceRetired
                    && state.FaultCode is null:
                return;
            case "service_retired"
                when !state.RetirementCredentialPresent && state.ServiceRetired
                    && state.FaultCode is null:
                return;
            case "failed"
                when !state.RetirementCredentialPresent && !state.ServiceRetired
                    && ValidCode(state.FaultCode):
                return;
            default:
                throw new InvalidDataException("daemon retirement response state is inconsistent");
        }
    }

    private static void ValidateTarget(string host, ushort port)
    {
        if (port == 0 || host.Length is < 1 or > 253 || host != host.Trim()
            || !host.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '-' or ':' or '_'))
        {
            throw new InvalidDataException("invalid daemon retirement target");
        }
    }

    private static void RequireIdentifier(string value, string label)
    {
        if (value.Length is < 1 or > 128 || value != value.Trim()
            || !value.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '-' or '_' or '.' or ':'))
        {
            throw new ArgumentException($"invalid {label}");
        }
    }

    private static void RequireHandle(string value, string provider)
    {
        RequireIdentifier(value, "credential handle");
        if (!value.StartsWith($"vault:{provider}:", StringComparison.Ordinal)
            || value.Length == $"vault:{provider}:".Length)
        {
            throw new ArgumentException(
                $"credential handle must use the {provider} vault provider");
        }
    }

    private static bool ValidCode(string? value) => value is { Length: > 0 and <= 64 }
        && value.All(character => character is >= 'a' and <= 'z'
            || char.IsAsciiDigit(character) || character == '_');

    private static void RequireBound(ReadOnlySpan<byte> payload)
    {
        if (payload.Length > MaxMessageBytes)
        {
            throw new InvalidDataException(
                "daemon retirement response exceeds the message limit");
        }
    }

    private static void ExpectInvalid(Action action, string message)
    {
        try
        {
            action();
        }
        catch (Exception error) when (error is ArgumentException or InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException(message);
    }
}

public sealed class RemoteDaemonRetirementException(string code, string message)
    : Exception(message)
{
    public string Code { get; } = code;
}
