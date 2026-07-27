using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace Leserpent.ControlPlane;

public interface IOrchestraDeleteCheckpointAlertSink
{
    Task DeliverAsync(
        PersistedOrchestraDeleteCheckpointAlertDelivery delivery,
        CancellationToken cancellationToken);
}

public sealed record OrchestraDeleteCheckpointAlertEnvelope(
    int Version,
    string EventId,
    string Kind,
    ulong AlertGeneration,
    DateTimeOffset RaisedAt,
    string AdmissionPressure,
    uint FailureCount,
    string FailureCode);

public sealed class LoggingOrchestraDeleteCheckpointAlertSink(
    ILogger<LoggingOrchestraDeleteCheckpointAlertSink> logger) :
    IOrchestraDeleteCheckpointAlertSink
{
    public Task DeliverAsync(
        PersistedOrchestraDeleteCheckpointAlertDelivery delivery,
        CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        logger.LogCritical(
            "Orchestra delete replay checkpoint alert {EventId} generation {AlertGeneration}: {FailureCode}, pressure {AdmissionPressure}.",
            delivery.EventId,
            delivery.AlertGeneration,
            delivery.FailureCode,
            delivery.AdmissionPressure);
        return Task.CompletedTask;
    }
}

public sealed class AuthenticatedHttpOrchestraDeleteCheckpointAlertSink(
    HttpClient httpClient,
    Uri endpoint,
    string bearerToken) :
    IOrchestraDeleteCheckpointAlertSink
{
    public async Task DeliverAsync(
        PersistedOrchestraDeleteCheckpointAlertDelivery delivery,
        CancellationToken cancellationToken)
    {
        var envelope = new OrchestraDeleteCheckpointAlertEnvelope(
            Version: 1,
            delivery.EventId,
            Kind: "orchestra_delete_checkpoint_unavailable",
            delivery.AlertGeneration,
            delivery.RaisedAt,
            delivery.AdmissionPressure.ToString().ToLowerInvariant(),
            delivery.FailureCount,
            delivery.FailureCode);
        using var request = new HttpRequestMessage(
            HttpMethod.Post,
            endpoint)
        {
            Content = new StringContent(
                JsonSerializer.Serialize(
                    envelope,
                    LeserpentJsonContext.Default
                        .OrchestraDeleteCheckpointAlertEnvelope),
                Encoding.UTF8,
                "application/json"),
        };
        request.Headers.Authorization =
            new AuthenticationHeaderValue(
                "Bearer",
                bearerToken);
        request.Headers.TryAddWithoutValidation(
            "Idempotency-Key",
            delivery.EventId);
        request.Headers.TryAddWithoutValidation(
            "X-Leserpent-Alert-Generation",
            delivery.AlertGeneration.ToString(
                System.Globalization.CultureInfo.InvariantCulture));
        using var response = await httpClient.SendAsync(
            request,
            HttpCompletionOption.ResponseHeadersRead,
            cancellationToken);
        response.EnsureSuccessStatusCode();
    }
}

internal static class OrchestraDeleteCheckpointAlertSinkFactory
{
    internal const string HttpClientName =
        "leserpent-checkpoint-alert";
    private const int MaxTokenFileBytes = 258;

    public static IOrchestraDeleteCheckpointAlertSink Create(
        IConfiguration configuration,
        IHttpClientFactory httpClientFactory,
        LoggingOrchestraDeleteCheckpointAlertSink loggingSink)
    {
        var endpointValue =
            configuration[
                "LESERPENT_CHECKPOINT_ALERT_ENDPOINT"]?.Trim();
        var tokenPath =
            configuration[
                "LESERPENT_CHECKPOINT_ALERT_TOKEN_FILE"]?.Trim();
        var inlineToken =
            configuration[
                "LESERPENT_CHECKPOINT_ALERT_TOKEN"];
        if (!string.IsNullOrWhiteSpace(inlineToken))
        {
            throw new InvalidOperationException(
                "LESERPENT_CHECKPOINT_ALERT_TOKEN is forbidden; use LESERPENT_CHECKPOINT_ALERT_TOKEN_FILE");
        }
        if (string.IsNullOrWhiteSpace(endpointValue) &&
            string.IsNullOrWhiteSpace(tokenPath))
        {
            return loggingSink;
        }
        if (string.IsNullOrWhiteSpace(endpointValue) ||
            string.IsNullOrWhiteSpace(tokenPath))
        {
            throw new InvalidOperationException(
                "checkpoint alert endpoint and token file must be configured together");
        }
        if (!Uri.TryCreate(
                endpointValue,
                UriKind.Absolute,
                out var endpoint) ||
            !string.Equals(
                endpoint.Scheme,
                Uri.UriSchemeHttps,
                StringComparison.OrdinalIgnoreCase) ||
            !string.IsNullOrWhiteSpace(endpoint.UserInfo) ||
            !string.IsNullOrWhiteSpace(endpoint.Fragment))
        {
            throw new InvalidOperationException(
                "checkpoint alert endpoint must be an absolute HTTPS URL without credentials or fragments");
        }

        var token = ReadPrivateToken(tokenPath);
        return new AuthenticatedHttpOrchestraDeleteCheckpointAlertSink(
            httpClientFactory.CreateClient(HttpClientName),
            endpoint,
            token);
    }

    private static string ReadPrivateToken(string tokenPath)
    {
        if (!Path.IsPathFullyQualified(tokenPath))
        {
            throw new InvalidOperationException(
                "checkpoint alert token file must be an absolute path");
        }
        var path = Path.GetFullPath(tokenPath);
        RejectSymbolicLink(path);
        if (!OperatingSystem.IsWindows())
        {
            var mode = File.GetUnixFileMode(path);
            const UnixFileMode unsafeMode =
                UnixFileMode.UserExecute |
                UnixFileMode.GroupRead |
                UnixFileMode.GroupWrite |
                UnixFileMode.GroupExecute |
                UnixFileMode.OtherRead |
                UnixFileMode.OtherWrite |
                UnixFileMode.OtherExecute;
            if ((mode & unsafeMode) != 0 ||
                (mode & UnixFileMode.UserRead) == 0)
            {
                throw new InvalidDataException(
                    "checkpoint alert token file must be owner-readable and owner-private");
            }
        }

        byte[] bytes;
        using (var stream = new FileStream(
            path,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read))
        {
            if (stream.Length is < 32 or > MaxTokenFileBytes)
            {
                throw new InvalidDataException(
                    "checkpoint alert token file has an invalid length");
            }
            bytes = new byte[checked((int)stream.Length)];
            stream.ReadExactly(bytes);
            if (stream.ReadByte() != -1)
            {
                CryptographicOperations.ZeroMemory(bytes);
                throw new InvalidDataException(
                    "checkpoint alert token file changed while being read");
            }
        }
        RejectSymbolicLink(path);

        try
        {
            var length = bytes.Length;
            while (length > 0 &&
                bytes[length - 1] is (byte)'\r' or (byte)'\n')
            {
                length -= 1;
            }
            if (length is < 32 or > 256 ||
                bytes.AsSpan(0, length).IndexOfAnyInRange(
                    (byte)0,
                    (byte)0x20) >= 0 ||
                bytes.AsSpan(0, length).IndexOfAnyInRange(
                    (byte)0x7f,
                    byte.MaxValue) >= 0)
            {
                throw new InvalidDataException(
                    "checkpoint alert token must contain 32 to 256 visible ASCII characters");
            }
            return Encoding.ASCII.GetString(bytes, 0, length);
        }
        finally
        {
            CryptographicOperations.ZeroMemory(bytes);
        }
    }

    private static void RejectSymbolicLink(string path)
    {
        var file = new FileInfo(path);
        if (file.LinkTarget is not null ||
            (file.Exists &&
             (file.Attributes & FileAttributes.ReparsePoint) != 0))
        {
            throw new InvalidDataException(
                "checkpoint alert token file must not be a symbolic link");
        }
    }
}
