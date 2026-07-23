using System.Text;
using System.Text.Json;

public sealed record RemoteRetirementIntent(
    string RetirementId,
    string ProvisioningId,
    string RuntimeId,
    string Host,
    ushort Port,
    string RetirementCredentialHandle,
    string RequestedBy);

public sealed record RemoteRetirementSnapshot(
    string RetirementId,
    string ProvisioningId,
    string RuntimeId,
    string Phase,
    string Transport,
    string Host,
    ushort Port,
    bool RetirementCredentialPresent,
    bool ServiceRetired,
    bool RuntimeRegistered,
    string? FaultCode)
{
    public bool IsTerminal => Phase is "runtime_unregistered" or "failed";
}

public sealed class RemoteRetirementClient : IDisposable
{
    public const int MaxMessageBytes = 64 * 1024;
    private const string Capability = "runtime.retire";
    private readonly RemoteWireTransport transport;

    public RemoteRetirementClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteRetirementSnapshot> ReconcileAsync(
        RemoteRetirementIntent intent,
        CancellationToken cancellationToken = default)
    {
        var payload = EncodeRequest(intent);
        var response = await transport.PostRetirementAsync(
            payload,
            "gewyvern retirement",
            cancellationToken).ConfigureAwait(false);
        return DecodeResponse(response, intent);
    }

    public void Dispose() => transport.Dispose();

    internal static byte[] EncodeRequest(RemoteRetirementIntent intent)
    {
        ValidateIntent(intent);
        return JsonSerializer.SerializeToUtf8Bytes(
            new RetirementRequestEnvelope
            {
                Request = new RetirementRequest
                {
                    Principal = new RemotePrincipal { Id = intent.RequestedBy },
                    Capabilities = [Capability],
                    Intent = new RetirementIntent
                    {
                        RetirementId = intent.RetirementId,
                        ProvisioningId = intent.ProvisioningId,
                        RuntimeId = intent.RuntimeId,
                        Target = new RetirementTarget
                        {
                            Transport = "ssh",
                            Host = intent.Host,
                            Port = intent.Port,
                        },
                        RetirementCredentialHandle = intent.RetirementCredentialHandle,
                        RequestedBy = intent.RequestedBy,
                    },
                },
            },
            RemoteRetirementJsonContext.Default.RetirementRequestEnvelope);
    }

    public static void VerifyContract()
    {
        var intent = new RemoteRetirementIntent(
            "retire-ui-1",
            "provision-ui-1",
            "runtime-ui-1",
            "runtime.example",
            22,
            "vault:ssh:runtime-example",
            "avalonia-hub");
        var encoded = Encoding.UTF8.GetString(EncodeRequest(intent));
        if (!encoded.Contains("\"capabilities\":[\"runtime.retire\"]", StringComparison.Ordinal)
            || !encoded.Contains("\"confirmed\":true", StringComparison.Ordinal)
            || !encoded.Contains(
                "\"retirement_credential_handle\":\"vault:ssh:runtime-example\"",
                StringComparison.Ordinal)
            || encoded.Contains("password", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("private_key", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("runtime.deploy", StringComparison.Ordinal))
        {
            throw new InvalidDataException("retirement request contract drifted");
        }

        const string planned = """
            {"schema_version":1,"response":{"kind":"state","payload":{"retirement_id":"retire-ui-1","provisioning_id":"provision-ui-1","runtime_id":"runtime-ui-1","phase":"planned","target":{"transport":"ssh","host":"runtime.example","port":22},"retirement_credential_present":true,"service_retired":false,"runtime_registered":true,"fault_code":null}}}
            """;
        var state = DecodeResponse(Encoding.UTF8.GetBytes(planned), intent);
        if (state.Phase != "planned" || state.IsTerminal || !state.RuntimeRegistered)
        {
            throw new InvalidDataException("planned retirement projection drifted");
        }

        const string completed = """
            {"schema_version":1,"response":{"kind":"state","payload":{"retirement_id":"retire-ui-1","provisioning_id":"provision-ui-1","runtime_id":"runtime-ui-1","phase":"runtime_unregistered","target":{"transport":"ssh","host":"runtime.example","port":22},"retirement_credential_present":false,"service_retired":true,"runtime_registered":false,"fault_code":null}}}
            """;
        state = DecodeResponse(Encoding.UTF8.GetBytes(completed), intent);
        if (!state.IsTerminal || !state.ServiceRetired || state.RuntimeRegistered)
        {
            throw new InvalidDataException("completed retirement projection drifted");
        }

        const string failed = """
            {"schema_version":1,"response":{"kind":"state","payload":{"retirement_id":"retire-ui-1","provisioning_id":"provision-ui-1","runtime_id":"runtime-ui-1","phase":"failed","target":{"transport":"ssh","host":"runtime.example","port":22},"retirement_credential_present":false,"service_retired":false,"runtime_registered":true,"fault_code":"service_stop_failed"}}}
            """;
        state = DecodeResponse(Encoding.UTF8.GetBytes(failed), intent);
        if (!state.IsTerminal || !state.RuntimeRegistered)
        {
            throw new InvalidDataException("failed retirement did not preserve registration");
        }

        ExpectInvalid(
            () => DecodeResponse(
                Encoding.UTF8.GetBytes(
                    planned.Replace("provision-ui-1", "provision-other", StringComparison.Ordinal)),
                intent),
            "retirement response crossed its provisioning identity fence");
        ExpectInvalid(
            () => ValidateIntent(intent with { RetirementCredentialHandle = "ssh-password" }),
            "retirement client accepted a raw credential source");
    }

    private static RemoteRetirementSnapshot DecodeResponse(
        ReadOnlySpan<byte> payload,
        RemoteRetirementIntent expected)
    {
        RequireBound(payload);
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteRetirementJsonContext.Default.RetirementResponseEnvelope)
                ?? throw new InvalidDataException("retirement response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported retirement response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw ProtocolError(envelope.Response.Payload, expected.RetirementId);
            }
            if (envelope.Response.Kind != "state")
            {
                throw new InvalidDataException("retirement returned an unexpected response kind");
            }
            var state = envelope.Response.Payload.Deserialize(
                RemoteRetirementJsonContext.Default.RetirementSnapshotPayload)
                ?? throw new InvalidDataException("retirement response state is empty");
            ValidateSnapshot(state, expected);
            return new RemoteRetirementSnapshot(
                state.RetirementId,
                state.ProvisioningId,
                state.RuntimeId,
                state.Phase,
                state.Target.Transport,
                state.Target.Host,
                state.Target.Port,
                state.RetirementCredentialPresent,
                state.ServiceRetired,
                state.RuntimeRegistered,
                state.FaultCode);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("retirement response JSON is invalid", error);
        }
    }

    private static RemoteRetirementException ProtocolError(
        JsonElement payload,
        string expectedRetirementId)
    {
        var error = payload.Deserialize(RemoteRetirementJsonContext.Default.RetirementErrorPayload)
            ?? throw new InvalidDataException("retirement protocol error is empty");
        if (error.RetirementId is not null && error.RetirementId != expectedRetirementId)
        {
            throw new InvalidDataException("retirement error crossed its identity fence");
        }
        if (!ValidCode(error.Code)
            || string.IsNullOrWhiteSpace(error.Message)
            || error.Message != error.Message.Trim()
            || error.Message.Length > 512
            || error.Message.Any(char.IsControl))
        {
            throw new InvalidDataException("retirement protocol error is invalid");
        }
        return new RemoteRetirementException(error.Code, error.Message);
    }

    private static void ValidateIntent(RemoteRetirementIntent intent)
    {
        RequireIdentifier(intent.RetirementId, "retirement ID");
        RequireIdentifier(intent.ProvisioningId, "provisioning ID");
        RequireIdentifier(intent.RuntimeId, "runtime ID");
        RequireIdentifier(intent.RequestedBy, "principal");
        ValidateTarget(intent.Host, intent.Port);
        RequireHandle(intent.RetirementCredentialHandle, "ssh");
    }

    private static void ValidateSnapshot(
        RetirementSnapshotPayload state,
        RemoteRetirementIntent expected)
    {
        RequireIdentifier(state.RetirementId, "retirement ID");
        RequireIdentifier(state.ProvisioningId, "provisioning ID");
        RequireIdentifier(state.RuntimeId, "runtime ID");
        if (state.RetirementId != expected.RetirementId
            || state.ProvisioningId != expected.ProvisioningId
            || state.RuntimeId != expected.RuntimeId
            || state.Target.Transport != "ssh"
            || state.Target.Host != expected.Host
            || state.Target.Port != expected.Port)
        {
            throw new InvalidDataException("retirement response identity is invalid");
        }
        ValidateTarget(state.Target.Host, state.Target.Port);
        switch (state.Phase)
        {
            case "planned" or "retiring_service"
                when state.RetirementCredentialPresent && !state.ServiceRetired
                    && state.RuntimeRegistered && state.FaultCode is null:
                return;
            case "service_retired"
                when !state.RetirementCredentialPresent && state.ServiceRetired
                    && state.RuntimeRegistered && state.FaultCode is null:
                return;
            case "runtime_unregistered"
                when !state.RetirementCredentialPresent && state.ServiceRetired
                    && !state.RuntimeRegistered && state.FaultCode is null:
                return;
            case "failed"
                when !state.RetirementCredentialPresent && !state.ServiceRetired
                    && state.RuntimeRegistered && ValidCode(state.FaultCode):
                return;
            default:
                throw new InvalidDataException("retirement response state is inconsistent");
        }
    }

    private static void ValidateTarget(string host, ushort port)
    {
        if (port == 0 || host.Length is < 1 or > 253 || host != host.Trim()
            || !host.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '-' or ':' or '_'))
        {
            throw new ArgumentException("invalid retirement target");
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
            throw new InvalidDataException("retirement response exceeds the message limit");
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

public sealed class RemoteRetirementException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}
