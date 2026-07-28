using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

public sealed class RemoteLeselangClient : IDisposable
{
    public const int SchemaVersion = 1;
    public const int MaxMessageBytes = 8 * 1024;
    public const int MaxSourceBytes = 4 * 1024;
    private readonly RemoteWireTransport transport;

    public RemoteLeselangClient(RemoteClientOptions options)
    {
        transport = new RemoteWireTransport(options);
    }

    public Task<string> ExportRefreshAsync(
        string runtimeId,
        bool capabilities,
        CancellationToken cancellationToken = default) => ExportAsync(
            capabilities
                ? "runtime_capabilities_refresh"
                : "runtime_refresh",
            runtimeId,
            null,
            null,
            cancellationToken);

    public Task<string> ExportWorkspaceAsync(
        string runtimeId,
        CancellationToken cancellationToken = default) => ExportAsync(
            "runtime_workspace",
            runtimeId,
            null,
            null,
            cancellationToken);

    public Task<string> ExportDeployAsync(
        string runtimeId,
        string pipelineKind,
        string? target,
        CancellationToken cancellationToken = default) => ExportAsync(
            "runtime_deploy",
            runtimeId,
            pipelineKind,
            target,
            cancellationToken);

    private async Task<string> ExportAsync(
        string kind,
        string runtimeId,
        string? pipelineKind,
        string? target,
        CancellationToken cancellationToken)
    {
        RemoteQueryValidation.RequireIdentifier(runtimeId, "runtime ID");
        var request = new LeselangExportRequest
        {
            SchemaVersion = SchemaVersion,
            Intent = new LeselangExportIntent
            {
                Kind = kind,
                RuntimeId = runtimeId,
                PipelineKind = pipelineKind,
                Target = target,
            },
        };
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            request,
            RemoteLeselangJsonContext.Default.LeselangExportRequest);
        var responsePayload = await transport.PostLeselangExportAsync(
            payload,
            cancellationToken).ConfigureAwait(false);
        return DecodeResponse(responsePayload);
    }

    public void Dispose() => transport.Dispose();

    private static string DecodeResponse(ReadOnlySpan<byte> payload)
    {
        if (payload.Length > MaxMessageBytes)
        {
            throw new InvalidDataException(
                "remote Leselang export response exceeds the protocol limit");
        }
        var response = JsonSerializer.Deserialize(
                payload,
                RemoteLeselangJsonContext.Default.LeselangExportResponse)
            ?? throw new InvalidDataException(
                "remote Leselang export response is empty");
        if (response.SchemaVersion != SchemaVersion)
        {
            throw new InvalidDataException(
                "remote Leselang export schema is unsupported");
        }
        if (response.Source is { } source && response.Error is null)
        {
            if (string.IsNullOrEmpty(source)
                || Encoding.UTF8.GetByteCount(source) > MaxSourceBytes)
            {
                throw new InvalidDataException(
                    "remote Leselang export source is invalid");
            }
            return source;
        }
        if (response.Source is null && response.Error is { } error
            && ValidFailure(error))
        {
            throw new RemoteLeselangExportException(error.Code, error.Message);
        }
        throw new InvalidDataException(
            "remote Leselang export response has an invalid result shape");
    }

    private static bool ValidFailure(LeselangExportFailure failure) =>
        !string.IsNullOrWhiteSpace(failure.Code)
        && failure.Code.Length <= 64
        && failure.Code.All(character =>
            char.IsAsciiLetterOrDigit(character) || character == '_')
        && !string.IsNullOrWhiteSpace(failure.Message)
        && failure.Message.Length <= 256
        && !failure.Message.Any(char.IsControl);

    public static void VerifyContract()
    {
        var success = Encoding.UTF8.GetBytes(
            "{\"schema_version\":1,\"source\":\"canonical-source\",\"error\":null}");
        if (DecodeResponse(success) != "canonical-source")
        {
            throw new InvalidDataException(
                "GUI Leselang export response contract diverged");
        }
        var rejected = Encoding.UTF8.GetBytes(
            "{\"schema_version\":1,\"source\":null,\"error\":{\"code\":\"invalid_intent\",\"message\":\"Leselang export intent is invalid\"}}");
        try
        {
            DecodeResponse(rejected);
            throw new InvalidDataException(
                "GUI Leselang export accepted a rejected Rust intent");
        }
        catch (RemoteLeselangExportException error)
            when (error.Code == "invalid_intent")
        {
        }
    }
}

public sealed class RemoteLeselangExportException : Exception
{
    public RemoteLeselangExportException(string code, string message)
        : base(message)
    {
        Code = code;
    }

    public string Code { get; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class LeselangExportRequest
{
    public int SchemaVersion { get; set; }
    public required LeselangExportIntent Intent { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class LeselangExportIntent
{
    public required string Kind { get; set; }
    public required string RuntimeId { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? PipelineKind { get; set; }
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Target { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class LeselangExportResponse
{
    public int SchemaVersion { get; set; }
    public string? Source { get; set; }
    public LeselangExportFailure? Error { get; set; }
}

[JsonUnmappedMemberHandling(JsonUnmappedMemberHandling.Disallow)]
public sealed class LeselangExportFailure
{
    public required string Code { get; set; }
    public required string Message { get; set; }
}

[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(LeselangExportRequest))]
[JsonSerializable(typeof(LeselangExportResponse))]
public partial class RemoteLeselangJsonContext : JsonSerializerContext;
