using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography.X509Certificates;

internal sealed class RemoteWireTransport : IDisposable
{
    private static readonly TimeSpan RequestTimeout = TimeSpan.FromSeconds(5);
    private readonly HttpClient client;
    private readonly X509Certificate2 trustedRoot;

    public RemoteWireTransport(RemoteClientOptions options)
    {
        trustedRoot = RemoteTls.LoadRoot(options.CertificateAuthorityPath);
        var handler = new SocketsHttpHandler
        {
            AllowAutoRedirect = false,
            AutomaticDecompression = DecompressionMethods.None,
            ConnectTimeout = RequestTimeout,
            MaxConnectionsPerServer = 2,
            PooledConnectionLifetime = TimeSpan.FromMinutes(2),
        };
        handler.SslOptions.RemoteCertificateValidationCallback =
            (_, certificate, _, errors) =>
                RemoteTls.ValidateServerCertificate(certificate, errors, trustedRoot);
        client = new HttpClient(handler)
        {
            BaseAddress = options.Endpoint,
            Timeout = RequestTimeout,
        };
        client.DefaultRequestHeaders.Authorization =
            new AuthenticationHeaderValue("Bearer", options.Token);
        client.DefaultRequestHeaders.Accept.Add(
            new MediaTypeWithQualityHeaderValue("application/json"));
    }

    public async Task<byte[]> PostAsync(
        ReadOnlyMemory<byte> payload,
        string operation,
        CancellationToken cancellationToken)
        => await PostAsync(
            payload,
            operation,
            "v1/wire",
            RemoteEventCodec.MaxMessageBytes,
            cancellationToken).ConfigureAwait(false);

    public async Task<byte[]> PostBootstrapAsync(
        ReadOnlyMemory<byte> payload,
        string operation,
        CancellationToken cancellationToken)
        => await PostAsync(
            payload,
            operation,
            "v1/bootstrap",
            RemoteBootstrapClient.MaxMessageBytes,
            cancellationToken).ConfigureAwait(false);

    private async Task<byte[]> PostAsync(
        ReadOnlyMemory<byte> payload,
        string operation,
        string route,
        int maxMessageBytes,
        CancellationToken cancellationToken)
    {
        if (payload.Length > maxMessageBytes)
        {
            throw new InvalidDataException($"remote {operation} exceeds the protocol limit");
        }
        using var content = new ByteArrayContent(payload.ToArray());
        content.Headers.ContentType = new MediaTypeHeaderValue("application/json");
        using var response = await client.PostAsync(route, content, cancellationToken)
            .ConfigureAwait(false);
        if (!string.Equals(
            response.Content.Headers.ContentType?.MediaType,
            "application/json",
            StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException(
                $"remote {operation} response is not application/json");
        }
        if (response.Content.Headers.ContentLength is long length && length > maxMessageBytes)
        {
            throw new InvalidDataException(
                $"remote {operation} response exceeds the protocol limit");
        }
        return await ReadBoundedAsync(response, operation, maxMessageBytes, cancellationToken)
            .ConfigureAwait(false);
    }

    public void Dispose()
    {
        client.Dispose();
        trustedRoot.Dispose();
    }

    private static async Task<byte[]> ReadBoundedAsync(
        HttpResponseMessage response,
        string operation,
        int maxMessageBytes,
        CancellationToken cancellationToken)
    {
        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken)
            .ConfigureAwait(false);
        using var payload = new MemoryStream();
        var buffer = new byte[16 * 1024];
        while (true)
        {
            var read = await stream.ReadAsync(buffer, cancellationToken).ConfigureAwait(false);
            if (read == 0)
            {
                return payload.ToArray();
            }
            if (payload.Length + read > maxMessageBytes)
            {
                throw new InvalidDataException(
                    $"remote {operation} response exceeds the protocol limit");
            }
            payload.Write(buffer, 0, read);
        }
    }
}
