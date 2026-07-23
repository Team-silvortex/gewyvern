using System.Text;
using System.Text.Json;

public sealed record RemoteProvisioningIntent(
    string ProvisioningId,
    string RuntimeId,
    string Host,
    ushort Port,
    string InstallCredentialHandle,
    string RequestedBy);

public sealed record RemoteProvisioningSnapshot(
    string ProvisioningId,
    string RuntimeId,
    string Phase,
    string Transport,
    string Host,
    ushort Port,
    bool InstallCredentialPresent,
    string? Endpoint,
    string? ApiCredentialHandle,
    string? TrustCredentialHandle,
    string? FaultCode,
    bool RuntimeRegistered)
{
    public bool IsTerminal => Phase is "runtime_registered" or "failed";
}

public sealed class RemoteProvisioningClient : IDisposable
{
    public const int MaxMessageBytes = 64 * 1024;
    private const string Capability = "runtime.provision";
    private readonly RemoteWireTransport transport;

    public RemoteProvisioningClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteProvisioningSnapshot> ReconcileAsync(
        RemoteProvisioningIntent intent,
        CancellationToken cancellationToken = default)
    {
        var payload = EncodeRequest(intent);
        var response = await transport.PostProvisioningAsync(
            payload,
            "gewyvern provisioning",
            cancellationToken).ConfigureAwait(false);
        return DecodeResponse(response, intent);
    }

    public void Dispose() => transport.Dispose();

    internal static byte[] EncodeRequest(RemoteProvisioningIntent intent)
    {
        ValidateIntent(intent);
        return JsonSerializer.SerializeToUtf8Bytes(
            new ProvisioningRequestEnvelope
            {
                Request = new ProvisioningRequest
                {
                    Principal = new RemotePrincipal { Id = intent.RequestedBy },
                    Capabilities = [Capability],
                    Intent = new ProvisioningIntent
                    {
                        ProvisioningId = intent.ProvisioningId,
                        RuntimeId = intent.RuntimeId,
                        Target = new ProvisioningTarget
                        {
                            Transport = "ssh",
                            Host = intent.Host,
                            Port = intent.Port,
                        },
                        InstallCredentialHandle = intent.InstallCredentialHandle,
                        RequestedBy = intent.RequestedBy,
                    },
                },
            },
            RemoteProvisioningJsonContext.Default.ProvisioningRequestEnvelope);
    }

    public static void VerifyContract()
    {
        var intent = new RemoteProvisioningIntent(
            "provision-ui-1",
            "runtime-ui-1",
            "runtime.example",
            22,
            "vault:ssh:runtime-example",
            "avalonia-hub");
        var encoded = Encoding.UTF8.GetString(EncodeRequest(intent));
        if (!encoded.Contains("\"capabilities\":[\"runtime.provision\"]", StringComparison.Ordinal)
            || !encoded.Contains("\"confirmed\":true", StringComparison.Ordinal)
            || !encoded.Contains("\"install_credential_handle\":\"vault:ssh:runtime-example\"", StringComparison.Ordinal)
            || encoded.Contains("password", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("private_key", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("runtime.deploy", StringComparison.Ordinal))
        {
            throw new InvalidDataException("provisioning request contract drifted");
        }

        const string planned = """
            {"schema_version":1,"response":{"kind":"state","payload":{"provisioning_id":"provision-ui-1","runtime_id":"runtime-ui-1","phase":"planned","target":{"transport":"ssh","host":"runtime.example","port":22},"install_credential_present":true,"endpoint":null,"api_credential_handle":null,"trust_credential_handle":null,"fault_code":null,"runtime_registered":false}}}
            """;
        var state = DecodeResponse(Encoding.UTF8.GetBytes(planned), intent);
        if (state.Phase != "planned" || state.IsTerminal)
        {
            throw new InvalidDataException("planned provisioning projection drifted");
        }

        const string registered = """
            {"schema_version":1,"response":{"kind":"state","payload":{"provisioning_id":"provision-ui-1","runtime_id":"runtime-ui-1","phase":"runtime_registered","target":{"transport":"ssh","host":"runtime.example","port":22},"install_credential_present":false,"endpoint":"https://runtime.example:9444","api_credential_handle":"vault:gewyvern:runtime-api","trust_credential_handle":"vault:gewyvern-ca:runtime-ca","fault_code":null,"runtime_registered":true}}}
            """;
        state = DecodeResponse(Encoding.UTF8.GetBytes(registered), intent);
        if (!state.IsTerminal || !state.RuntimeRegistered)
        {
            throw new InvalidDataException("registered provisioning projection drifted");
        }

        ExpectInvalid(
            () => DecodeResponse(
                Encoding.UTF8.GetBytes(planned.Replace("runtime-ui-1", "runtime-other", StringComparison.Ordinal)),
                intent),
            "provisioning response crossed its runtime identity fence");
        ExpectInvalid(
            () => DecodeResponse(
                Encoding.UTF8.GetBytes(planned.Replace("\"transport\":\"ssh\",", string.Empty, StringComparison.Ordinal)),
                intent),
            "provisioning response accepted a missing target transport");
        ExpectInvalid(
            () => ValidateIntent(intent with { InstallCredentialHandle = "ssh-password" }),
            "provisioning client accepted a raw credential source");
    }

    private static RemoteProvisioningSnapshot DecodeResponse(
        ReadOnlySpan<byte> payload,
        RemoteProvisioningIntent expected)
    {
        RequireBound(payload);
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteProvisioningJsonContext.Default.ProvisioningResponseEnvelope)
                ?? throw new InvalidDataException("provisioning response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported provisioning response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw ProtocolError(envelope.Response.Payload, expected.ProvisioningId);
            }
            if (envelope.Response.Kind != "state")
            {
                throw new InvalidDataException("provisioning returned an unexpected response kind");
            }
            var state = envelope.Response.Payload.Deserialize(
                RemoteProvisioningJsonContext.Default.ProvisioningSnapshotPayload)
                ?? throw new InvalidDataException("provisioning response state is empty");
            ValidateSnapshot(state, expected);
            return new RemoteProvisioningSnapshot(
                state.ProvisioningId,
                state.RuntimeId,
                state.Phase,
                state.Target.Transport,
                state.Target.Host,
                state.Target.Port,
                state.InstallCredentialPresent,
                state.Endpoint,
                state.ApiCredentialHandle,
                state.TrustCredentialHandle,
                state.FaultCode,
                state.RuntimeRegistered);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("provisioning response JSON is invalid", error);
        }
    }

    private static RemoteProvisioningException ProtocolError(
        JsonElement payload,
        string expectedProvisioningId)
    {
        var error = payload.Deserialize(RemoteProvisioningJsonContext.Default.ProvisioningErrorPayload)
            ?? throw new InvalidDataException("provisioning protocol error is empty");
        if (error.ProvisioningId is not null && error.ProvisioningId != expectedProvisioningId)
        {
            throw new InvalidDataException("provisioning error crossed its identity fence");
        }
        if (!ValidCode(error.Code)
            || string.IsNullOrWhiteSpace(error.Message)
            || error.Message != error.Message.Trim()
            || error.Message.Length > 512
            || error.Message.Any(char.IsControl))
        {
            throw new InvalidDataException("provisioning protocol error is invalid");
        }
        return new RemoteProvisioningException(error.Code, error.Message);
    }

    private static void ValidateIntent(RemoteProvisioningIntent intent)
    {
        RequireIdentifier(intent.ProvisioningId, "provisioning ID");
        RequireIdentifier(intent.RuntimeId, "runtime ID");
        RequireIdentifier(intent.RequestedBy, "principal");
        ValidateTarget(intent.Host, intent.Port);
        RequireHandle(intent.InstallCredentialHandle, "ssh");
    }

    private static void ValidateSnapshot(
        ProvisioningSnapshotPayload state,
        RemoteProvisioningIntent expected)
    {
        RequireIdentifier(state.ProvisioningId, "provisioning ID");
        RequireIdentifier(state.RuntimeId, "runtime ID");
        if (state.ProvisioningId != expected.ProvisioningId
            || state.RuntimeId != expected.RuntimeId
            || state.Target.Transport != "ssh"
            || state.Target.Host != expected.Host
            || state.Target.Port != expected.Port)
        {
            throw new InvalidDataException("provisioning response identity is invalid");
        }
        ValidateTarget(state.Target.Host, state.Target.Port);
        var hasService = state.Endpoint is not null
            && state.ApiCredentialHandle is not null
            && state.TrustCredentialHandle is not null;
        switch (state.Phase)
        {
            case "planned" or "installing" when state.InstallCredentialPresent
                && !hasService && state.Endpoint is null
                && state.ApiCredentialHandle is null && state.TrustCredentialHandle is null
                && state.FaultCode is null && !state.RuntimeRegistered:
                return;
            case "service_ready" when !state.InstallCredentialPresent && hasService
                && state.FaultCode is null && !state.RuntimeRegistered:
                ValidateServiceIdentity(state);
                return;
            case "runtime_registered" when !state.InstallCredentialPresent && hasService
                && state.FaultCode is null && state.RuntimeRegistered:
                ValidateServiceIdentity(state);
                return;
            case "failed" when !state.InstallCredentialPresent && !hasService
                && state.Endpoint is null && state.ApiCredentialHandle is null
                && state.TrustCredentialHandle is null && ValidCode(state.FaultCode)
                && !state.RuntimeRegistered:
                return;
            default:
                throw new InvalidDataException("provisioning response state is inconsistent");
        }
    }

    private static void ValidateServiceIdentity(ProvisioningSnapshotPayload state)
    {
        RequireHandle(state.ApiCredentialHandle!, "gewyvern");
        RequireHandle(state.TrustCredentialHandle!, "gewyvern-ca");
        var endpoint = RemoteClientOptions.ParseEndpoint(state.Endpoint!);
        if (endpoint.GetComponents(UriComponents.SchemeAndServer, UriFormat.UriEscaped)
            != state.Endpoint)
        {
            throw new InvalidDataException("provisioning endpoint is not canonical");
        }
    }

    private static void ValidateTarget(string host, ushort port)
    {
        if (port == 0 || host.Length is < 1 or > 253 || host != host.Trim()
            || !host.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '-' or ':' or '_'))
        {
            throw new ArgumentException("invalid provisioning target");
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
            throw new ArgumentException($"credential handle must use the {provider} vault provider");
        }
    }

    private static bool ValidCode(string? value) => value is { Length: > 0 and <= 64 }
        && value.All(character => character is >= 'a' and <= 'z'
            || char.IsAsciiDigit(character) || character == '_');

    private static void RequireBound(ReadOnlySpan<byte> payload)
    {
        if (payload.Length > MaxMessageBytes)
        {
            throw new InvalidDataException("provisioning response exceeds the message limit");
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

public sealed class RemoteProvisioningException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}
