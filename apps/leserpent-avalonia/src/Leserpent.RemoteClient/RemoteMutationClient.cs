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
        CancellationToken cancellationToken = default)
    {
        RequireIdentifier(runtimeId, "runtime ID");
        RequireIdentifier(principal, "principal");
        var commandId = $"gui-{Guid.NewGuid():N}";
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
                    Capabilities = ["runtime.refresh"],
                    Command = new RuntimeRefreshCommand { RuntimeId = runtimeId },
                },
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            envelope,
            RemoteMutationJsonContext.Default.WireCommandRequestEnvelope);
        var responsePayload = await transport.PostAsync(
            payload,
            "mutation",
            cancellationToken)
            .ConfigureAwait(false);
        return DecodeResponse(responsePayload, commandId, runtimeId, expectedRevision);
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
}
