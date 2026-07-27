using System.Text;
using System.Text.Json;

public sealed record RemoteBootstrapIntent(
    string BootstrapId,
    string Host,
    ushort Port,
    string CredentialHandle,
    string RequestedBy);

public sealed record RemoteBootstrapSnapshot(
    string BootstrapId,
    string Phase,
    string Transport,
    string Host,
    ushort Port,
    bool BootstrapCredentialPresent,
    string? DaemonId,
    string? Endpoint,
    string? SessionCredentialHandle,
    string? TrustCredentialHandle,
    string? FaultCode,
    bool MutationAuthorized,
    string? Generation = null,
    string? InstallProfile = null)
{
    public bool IsTerminal => Phase is "session_bound" or "failed";
    public bool CanBind => Phase == "bootstrapped";
    public bool HasRetirementAuthority => Generation is not null && InstallProfile is not null;
}

public sealed class RemoteBootstrapClient : IDisposable
{
    public const int MaxMessageBytes = 64 * 1024;
    private const string Capability = "host.bootstrap";
    private readonly RemoteWireTransport transport;

    public RemoteBootstrapClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteBootstrapSnapshot> SubmitAsync(
        RemoteBootstrapIntent intent,
        CancellationToken cancellationToken = default)
    {
        ValidateIntent(intent);
        var envelope = new BootstrapRequestEnvelope
        {
            Request = new BootstrapRequest
            {
                Principal = new RemotePrincipal { Id = intent.RequestedBy },
                Capabilities = [Capability],
                Intent = new BootstrapIntent
                {
                    BootstrapId = intent.BootstrapId,
                    Target = new BootstrapTarget
                    {
                        Host = intent.Host,
                        Port = intent.Port,
                    },
                    CredentialHandle = intent.CredentialHandle,
                    RequestedBy = intent.RequestedBy,
                },
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteBootstrapJsonContext.Default.BootstrapRequestEnvelope);
        var response = await transport.PostBootstrapAsync(
            payload,
            "bootstrap submission",
            cancellationToken).ConfigureAwait(false);
        return DecodeBootstrapResponse(response, intent.BootstrapId);
    }

    public Task<RemoteBootstrapSnapshot> InspectAsync(
        string bootstrapId,
        string principal,
        CancellationToken cancellationToken = default) =>
        SendWireAsync(bootstrapId, principal, bind: false, cancellationToken);

    public Task<RemoteBootstrapSnapshot> BindAsync(
        string bootstrapId,
        string principal,
        CancellationToken cancellationToken = default) =>
        SendWireAsync(bootstrapId, principal, bind: true, cancellationToken);

    public void Dispose() => transport.Dispose();

    private async Task<RemoteBootstrapSnapshot> SendWireAsync(
        string bootstrapId,
        string principal,
        bool bind,
        CancellationToken cancellationToken)
    {
        RequireIdentifier(bootstrapId, "bootstrap ID");
        RequireIdentifier(principal, "principal");
        var payload = EncodeWireRequest(bootstrapId, principal, bind);
        var response = await transport.PostAsync(
            payload,
            bind ? "bootstrap session binding" : "bootstrap handoff inspection",
            cancellationToken).ConfigureAwait(false);
        return DecodeWireResponse(response, bootstrapId);
    }

    internal static byte[] EncodeSubmission(RemoteBootstrapIntent intent)
    {
        ValidateIntent(intent);
        return JsonSerializer.SerializeToUtf8Bytes(
            new BootstrapRequestEnvelope
            {
                Request = new BootstrapRequest
                {
                    Principal = new RemotePrincipal { Id = intent.RequestedBy },
                    Capabilities = [Capability],
                    Intent = new BootstrapIntent
                    {
                        BootstrapId = intent.BootstrapId,
                        Target = new BootstrapTarget { Host = intent.Host, Port = intent.Port },
                        CredentialHandle = intent.CredentialHandle,
                        RequestedBy = intent.RequestedBy,
                    },
                },
            },
            RemoteBootstrapJsonContext.Default.BootstrapRequestEnvelope);
    }

    internal static byte[] EncodeWireRequest(
        string bootstrapId,
        string principal,
        bool bind)
    {
        RequireIdentifier(bootstrapId, "bootstrap ID");
        RequireIdentifier(principal, "principal");
        return JsonSerializer.SerializeToUtf8Bytes(
            new BootstrapWireRequestEnvelope
            {
                Request = new BootstrapWireRequest
                {
                    Kind = bind ? "bootstrap_session_bind" : "bootstrap_handoff",
                    Payload = new BootstrapWirePayload
                    {
                        Principal = new RemotePrincipal { Id = principal },
                        Capabilities = [Capability],
                        BootstrapId = bootstrapId,
                        Confirmed = bind ? true : null,
                    },
                },
            },
            RemoteBootstrapJsonContext.Default.BootstrapWireRequestEnvelope);
    }

    public static void VerifyContract()
    {
        var intent = new RemoteBootstrapIntent(
            "bootstrap-ui-1",
            "target.example",
            22,
            "vault:ssh:target-example",
            "avalonia-hub");
        var encoded = Encoding.UTF8.GetString(EncodeSubmission(intent));
        if (!encoded.Contains("\"credential_handle\":\"vault:ssh:target-example\"", StringComparison.Ordinal)
            || !encoded.Contains("\"confirmed\":true", StringComparison.Ordinal)
            || encoded.Contains("password", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("private_key", StringComparison.OrdinalIgnoreCase)
            || encoded.Contains("session_token", StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException("bootstrap submission contract drifted");
        }
        var inspect = Encoding.UTF8.GetString(EncodeWireRequest(
            intent.BootstrapId,
            intent.RequestedBy,
            bind: false));
        var bind = Encoding.UTF8.GetString(EncodeWireRequest(
            intent.BootstrapId,
            intent.RequestedBy,
            bind: true));
        if (!inspect.Contains("\"kind\":\"bootstrap_handoff\"", StringComparison.Ordinal)
            || inspect.Contains("confirmed", StringComparison.Ordinal)
            || !bind.Contains("\"kind\":\"bootstrap_session_bind\"", StringComparison.Ordinal)
            || !bind.Contains("\"confirmed\":true", StringComparison.Ordinal))
        {
            throw new InvalidDataException("bootstrap inspect or bind request contract drifted");
        }

        const string planned = """
            {"schema_version":1,"response":{"kind":"state","payload":{"bootstrap_id":"bootstrap-ui-1","phase":"planned","target":{"transport":"ssh","host":"target.example","port":22},"bootstrap_credential_present":true,"daemon_id":null,"endpoint":null,"session_credential_handle":null,"trust_credential_handle":null,"fault_code":null,"mutation_authorized":false}}}
            """;
        var state = DecodeBootstrapResponse(Encoding.UTF8.GetBytes(planned), intent.BootstrapId);
        if (state.Phase != "planned" || state.IsTerminal || state.CanBind)
        {
            throw new InvalidDataException("bootstrap planned response projection drifted");
        }

        const string bound = """
            {"schema_version":1,"response":{"kind":"bootstrap_handoff","payload":{"bootstrap_id":"bootstrap-ui-1","phase":"session_bound","target":{"transport":"ssh","host":"target.example","port":22},"bootstrap_credential_present":false,"daemon_id":"daemon-target","endpoint":"https://target.example:9443","generation":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","install_profile":"system","session_credential_handle":"vault:leserpentd:target","trust_credential_handle":"vault:leserpent-ca:target","fault_code":null,"mutation_authorized":true}}}
            """;
        state = DecodeWireResponse(Encoding.UTF8.GetBytes(bound), intent.BootstrapId);
        if (!state.IsTerminal || state.CanBind || !state.MutationAuthorized
            || !state.HasRetirementAuthority || state.InstallProfile != "system")
        {
            throw new InvalidDataException("bootstrap bound response projection drifted");
        }
        var legacyBound = bound
            .Replace(
                "\"generation\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",",
                "",
                StringComparison.Ordinal)
            .Replace("\"install_profile\":\"system\",", "", StringComparison.Ordinal);
        state = DecodeWireResponse(Encoding.UTF8.GetBytes(legacyBound), intent.BootstrapId);
        if (state.HasRetirementAuthority)
        {
            throw new InvalidDataException("legacy bootstrap response invented retirement authority");
        }

        ExpectInvalid(
            () => DecodeBootstrapResponse(
                Encoding.UTF8.GetBytes(planned.Replace("planned", "bootstrapped", StringComparison.Ordinal)),
                intent.BootstrapId),
            "bootstrap client accepted an incomplete bootstrapped state");
        ExpectInvalid(
            () => DecodeWireResponse(
                Encoding.UTF8.GetBytes(bound.Replace(
                    "\"install_profile\":\"system\",",
                    "",
                    StringComparison.Ordinal)),
                intent.BootstrapId),
            "bootstrap client accepted partial retirement authority");
        ExpectInvalid(
            () => ValidateIntent(intent with { CredentialHandle = "ssh-password" }),
            "bootstrap client accepted a raw credential source");
    }

    private static RemoteBootstrapSnapshot DecodeBootstrapResponse(
        ReadOnlySpan<byte> payload,
        string expectedBootstrapId)
    {
        RequireBound(payload);
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteBootstrapJsonContext.Default.BootstrapResponseEnvelope)
                ?? throw new InvalidDataException("bootstrap submission response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported bootstrap response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw ProtocolError(envelope.Response.Payload);
            }
            if (envelope.Response.Kind != "state")
            {
                throw new InvalidDataException("bootstrap submission returned an unexpected response kind");
            }
            return Project(envelope.Response.Payload, expectedBootstrapId);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("bootstrap submission response JSON is invalid", error);
        }
    }

    private static RemoteBootstrapSnapshot DecodeWireResponse(
        ReadOnlySpan<byte> payload,
        string expectedBootstrapId)
    {
        RequireBound(payload);
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteBootstrapJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException("bootstrap wire response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported bootstrap wire response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                throw ProtocolError(envelope.Response.Payload);
            }
            if (envelope.Response.Kind != "bootstrap_handoff")
            {
                throw new InvalidDataException("bootstrap wire request returned an unexpected response kind");
            }
            return Project(envelope.Response.Payload, expectedBootstrapId);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("bootstrap wire response JSON is invalid", error);
        }
    }

    private static RemoteBootstrapSnapshot Project(
        JsonElement payload,
        string expectedBootstrapId)
    {
        var state = payload.Deserialize(RemoteBootstrapJsonContext.Default.BootstrapSnapshotPayload)
            ?? throw new InvalidDataException("bootstrap response state is empty");
        ValidateSnapshot(state, expectedBootstrapId);
        return new RemoteBootstrapSnapshot(
            state.BootstrapId,
            state.Phase,
            state.Target.Transport,
            state.Target.Host,
            state.Target.Port,
            state.BootstrapCredentialPresent,
            state.DaemonId,
            state.Endpoint,
            state.SessionCredentialHandle,
            state.TrustCredentialHandle,
            state.FaultCode,
            state.MutationAuthorized,
            state.Generation,
            state.InstallProfile);
    }

    private static RemoteBootstrapException ProtocolError(JsonElement payload)
    {
        var error = payload.Deserialize(RemoteBootstrapJsonContext.Default.BootstrapErrorPayload)
            ?? throw new InvalidDataException("bootstrap protocol error is empty");
        if (!ValidCode(error.Code)
            || string.IsNullOrWhiteSpace(error.Message)
            || error.Message.Length > 512
            || error.Message.Any(char.IsControl))
        {
            throw new InvalidDataException("bootstrap protocol error is invalid");
        }
        return new RemoteBootstrapException(error.Code, error.Message);
    }

    private static void ValidateIntent(RemoteBootstrapIntent intent)
    {
        RequireIdentifier(intent.BootstrapId, "bootstrap ID");
        RequireIdentifier(intent.RequestedBy, "principal");
        ValidateTarget(intent.Host, intent.Port);
        RequireHandle(intent.CredentialHandle, "ssh");
    }

    private static void ValidateSnapshot(BootstrapSnapshotPayload state, string expectedBootstrapId)
    {
        RequireIdentifier(state.BootstrapId, "bootstrap ID");
        if (state.BootstrapId != expectedBootstrapId || state.Target.Transport != "ssh")
        {
            throw new InvalidDataException("bootstrap response identity is invalid");
        }
        ValidateTarget(state.Target.Host, state.Target.Port);
        var hasReceipt = state.DaemonId is not null
            && state.Endpoint is not null
            && state.SessionCredentialHandle is not null
            && state.TrustCredentialHandle is not null;
        switch (state.Phase)
        {
            case "planned" or "deploying" when state.BootstrapCredentialPresent
                && !hasReceipt && state.DaemonId is null && state.Endpoint is null
                && state.Generation is null && state.InstallProfile is null
                && state.SessionCredentialHandle is null && state.TrustCredentialHandle is null
                && state.FaultCode is null && !state.MutationAuthorized:
                return;
            case "bootstrapped" when state.BootstrapCredentialPresent && hasReceipt
                && state.FaultCode is null && !state.MutationAuthorized:
                ValidateReceipt(state);
                return;
            case "session_bound" when !state.BootstrapCredentialPresent && hasReceipt
                && state.FaultCode is null && state.MutationAuthorized:
                ValidateReceipt(state);
                return;
            case "failed" when !state.BootstrapCredentialPresent && !hasReceipt
                && state.DaemonId is null && state.Endpoint is null
                && state.Generation is null && state.InstallProfile is null
                && state.SessionCredentialHandle is null && state.TrustCredentialHandle is null
                && ValidCode(state.FaultCode) && !state.MutationAuthorized:
                return;
            default:
                throw new InvalidDataException("bootstrap response state is inconsistent");
        }
    }

    private static void ValidateReceipt(BootstrapSnapshotPayload state)
    {
        RequireIdentifier(state.DaemonId!, "daemon ID");
        RequireHandle(state.SessionCredentialHandle!, "leserpentd");
        RequireHandle(state.TrustCredentialHandle!, "leserpent-ca");
        var endpoint = RemoteClientOptions.ParseEndpoint(state.Endpoint!);
        var canonicalOrigin = endpoint.GetComponents(
            UriComponents.SchemeAndServer,
            UriFormat.UriEscaped);
        if (canonicalOrigin != state.Endpoint)
        {
            throw new InvalidDataException("bootstrap response endpoint is not canonical");
        }
        if ((state.Generation is null) != (state.InstallProfile is null))
        {
            throw new InvalidDataException("bootstrap retirement authority is incomplete");
        }
        if (state.Generation is not null
            && (state.Generation.Length != 64
                || !state.Generation.All(character => character is >= '0' and <= '9'
                    or >= 'a' and <= 'f')))
        {
            throw new InvalidDataException("bootstrap generation is invalid");
        }
        if (state.InstallProfile is not null
            && state.InstallProfile is not ("system" or "user" or "test"))
        {
            throw new InvalidDataException("bootstrap install profile is invalid");
        }
    }

    private static void ValidateTarget(string host, ushort port)
    {
        if (port == 0 || host.Length is < 1 or > 253 || host != host.Trim()
            || !host.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '-' or ':' or '_'))
        {
            throw new ArgumentException("invalid bootstrap target");
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
            throw new InvalidDataException("bootstrap response exceeds the message limit");
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

public sealed class RemoteBootstrapException(string code, string message) : Exception(message)
{
    public string Code { get; } = code;
}
