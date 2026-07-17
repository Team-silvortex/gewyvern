using System.Text.Json;

public sealed class RemoteMutationClient : IDisposable
{
    private readonly RemoteWireTransport transport;

    public RemoteMutationClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public async Task<RemoteMutationResult> RefreshAsync(
        string runtimeId,
        ulong expectedRevision,
        string principal,
        CancellationToken cancellationToken = default) => await ExecuteAsync(
            runtimeId,
            expectedRevision,
            principal,
            "runtime_refresh",
            "runtime.refresh",
            null,
            null,
            cancellationToken).ConfigureAwait(false);

    public async Task<RemoteMutationResult> RefreshCapabilitiesAsync(
        string runtimeId,
        ulong expectedRevision,
        string principal,
        CancellationToken cancellationToken = default) => await ExecuteAsync(
            runtimeId,
            expectedRevision,
            principal,
            "runtime_capabilities_refresh",
            "runtime.refresh",
            null,
            null,
            cancellationToken).ConfigureAwait(false);

    public async Task<RemoteMutationResult> DeployAsync(
        string runtimeId,
        ulong expectedRevision,
        string principal,
        string pipelineKind,
        string? target,
        CancellationToken cancellationToken = default)
    {
        RequireDeploymentToken(pipelineKind, "pipeline kind");
        if (target is not null)
        {
            RequireDeploymentTarget(target);
        }
        return await ExecuteAsync(
            runtimeId,
            expectedRevision,
            principal,
            "runtime_deploy",
            "runtime.deploy",
            pipelineKind,
            target,
            cancellationToken).ConfigureAwait(false);
    }

    private async Task<RemoteMutationResult> ExecuteAsync(
        string runtimeId,
        ulong expectedRevision,
        string principal,
        string commandKind,
        string capability,
        string? pipelineKind,
        string? target,
        CancellationToken cancellationToken)
    {
        RequireIdentifier(runtimeId, "runtime ID");
        RequireIdentifier(principal, "principal");
        var commandId = $"gui-{Guid.NewGuid():N}";
        var payload = EncodeRequest(
            commandId,
            runtimeId,
            expectedRevision,
            principal,
            commandKind,
            capability,
            pipelineKind,
            target);
        var responsePayload = await transport.PostAsync(
            payload,
            "mutation",
            cancellationToken)
            .ConfigureAwait(false);
        return DecodeResponse(responsePayload, commandId, runtimeId, expectedRevision);
    }

    private static byte[] EncodeRequest(
        string commandId,
        string runtimeId,
        ulong expectedRevision,
        string principal,
        string commandKind,
        string capability,
        string? pipelineKind,
        string? target)
    {
        var envelope = new WireCommandRequestEnvelope
        {
            Request = new WireCommandRequest
            {
                Payload = new RuntimeRefreshEnvelope
                {
                    CommandId = commandId,
                    IdempotencyKey = commandId,
                    ExpectedRevision = expectedRevision,
                    Principal = new RemotePrincipal { Id = principal },
                    Capabilities = [capability],
                    Command = new RuntimeRefreshCommand
                    {
                        Kind = commandKind,
                        RuntimeId = runtimeId,
                        PipelineKind = pipelineKind,
                        Target = target,
                    },
                },
            },
        };
        return JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteMutationJsonContext.Default.WireCommandRequestEnvelope);
    }

    public static void VerifyDeploymentContract()
    {
        var refresh = EncodeRequest(
            "command-refresh", "runtime-a", 7, "operator-a",
            "runtime_refresh", "runtime.refresh", null, null);
        using var refreshDocument = JsonDocument.Parse(refresh);
        var refreshCommand = refreshDocument.RootElement
            .GetProperty("request").GetProperty("payload").GetProperty("command");
        if (refreshCommand.TryGetProperty("pipeline_kind", out _)
            || refreshCommand.TryGetProperty("target", out _))
        {
            throw new InvalidDataException(
                "refresh mutation serialized deployment-only fields");
        }

        RequireDeploymentToken("http/request", "pipeline kind");
        RequireDeploymentTarget("pid:42");
        var deployment = EncodeRequest(
            "command-deploy", "runtime-a", 7, "operator-a",
            "runtime_deploy", "runtime.deploy", "http/request", "pid:42");
        using var deploymentDocument = JsonDocument.Parse(deployment);
        var payload = deploymentDocument.RootElement
            .GetProperty("request").GetProperty("payload");
        var command = payload.GetProperty("command");
        if (payload.GetProperty("confirmation").GetString() != "confirmed"
            || payload.GetProperty("capabilities")[0].GetString() != "runtime.deploy"
            || command.GetProperty("kind").GetString() != "runtime_deploy"
            || command.GetProperty("pipeline_kind").GetString() != "http/request"
            || command.GetProperty("target").GetString() != "pid:42")
        {
            throw new InvalidDataException("deployment mutation contract is invalid");
        }
        try
        {
            RequireDeploymentToken("bad kind", "pipeline kind");
            throw new InvalidDataException("invalid deployment token was accepted");
        }
        catch (ArgumentException)
        {
        }
        try
        {
            RequireDeploymentTarget("pid:42\nforged");
            throw new InvalidDataException("invalid deployment target was accepted");
        }
        catch (ArgumentException)
        {
        }
    }

    public void Dispose()
    {
        transport.Dispose();
    }

    private static RemoteMutationResult DecodeResponse(
        ReadOnlySpan<byte> payload,
        string commandId,
        string runtimeId,
        ulong expectedRevision)
    {
        try
        {
            var envelope = JsonSerializer.Deserialize(
                payload,
                RemoteMutationJsonContext.Default.WireResponseEnvelope)
                ?? throw new InvalidDataException("remote mutation response is empty");
            if (envelope.SchemaVersion != 1)
            {
                throw new InvalidDataException("unsupported remote mutation response schema");
            }
            if (envelope.Response.Kind == "error")
            {
                var code = RequireString(envelope.Response.Payload, "code");
                var message = RequireString(envelope.Response.Payload, "message");
                throw new RemoteMutationException(code, message);
            }
            if (envelope.Response.Kind != "command")
            {
                throw new InvalidDataException("remote mutation returned an unexpected response kind");
            }
            var resultCommandId = RequireString(envelope.Response.Payload, "command_id");
            var status = RequireString(envelope.Response.Payload, "status");
            var runtime = envelope.Response.Payload.GetProperty("runtime");
            var resultRuntimeId = RequireString(runtime, "id");
            var revision = runtime.GetProperty("revision").GetUInt64();
            if (resultCommandId != commandId
                || resultRuntimeId != runtimeId
                || status != "applied"
                || revision <= expectedRevision)
            {
                throw new InvalidDataException("remote mutation response identity is invalid");
            }
            return new RemoteMutationResult(resultCommandId, resultRuntimeId, revision, status);
        }
        catch (JsonException error)
        {
            throw new InvalidDataException("remote mutation response JSON is invalid", error);
        }
        catch (KeyNotFoundException error)
        {
            throw new InvalidDataException("remote mutation response is missing a required field", error);
        }
        catch (InvalidOperationException error)
        {
            throw new InvalidDataException("remote mutation response has an invalid field type", error);
        }
    }

    private static string RequireString(JsonElement element, string property) =>
        element.GetProperty(property).GetString()
        ?? throw new InvalidDataException($"remote mutation response field '{property}' is invalid");

    private static void RequireIdentifier(string value, string label)
    {
        if (value.Length is < 1 or > 128
            || !value.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '-' or '_' or '.' or ':'))
        {
            throw new ArgumentException($"invalid {label}");
        }
    }

    private static void RequireDeploymentToken(string value, string label)
    {
        if (value.Length is < 1 or > 128
            || value != value.Trim()
            || !value.All(character => char.IsAsciiLetterOrDigit(character)
                || character is '.' or '/' or '_' or '-'))
        {
            throw new ArgumentException($"invalid {label}");
        }
    }

    private static void RequireDeploymentTarget(string value)
    {
        if (value.Length is < 1 or > 256
            || value != value.Trim()
            || value.Any(char.IsControl))
        {
            throw new ArgumentException("invalid deployment target");
        }
    }
}
