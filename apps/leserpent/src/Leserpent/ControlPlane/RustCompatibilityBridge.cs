using System.Buffers;
using System.Diagnostics;
using System.ComponentModel;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;

namespace Leserpent.ControlPlane;

public interface ICompatibilityBridge
{
    bool Enabled { get; }

    Task ValidateRuntimeListAsync(RuntimeCollectionResponse response, CancellationToken cancellationToken);

    Task<RuntimeDeploymentCompatibilityEnvelope> NormalizeRuntimeDeploymentRequestAsync(
        RuntimeDeploymentCompatibilityEnvelope request,
        CancellationToken cancellationToken);

    Task ValidateStatusRefreshAsync(RuntimeStatusRefreshResponse response, CancellationToken cancellationToken);
}

public sealed class RustCompatibilityBridge : ICompatibilityBridge, IDisposable
{
    private const int MaxMessageBytes = 1024 * 1024;
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private readonly string? executablePath;
    private readonly TimeSpan timeout;
    private readonly ILogger<RustCompatibilityBridge> logger;
    private readonly SemaphoreSlim requestLock = new(1, 1);
    private Process? process;

    public RustCompatibilityBridge(
        IConfiguration configuration,
        ILogger<RustCompatibilityBridge> logger)
    {
        this.logger = logger;
        var configuredPath = configuration["LESERPENT_RUST_BRIDGE_BIN"];
        if (!string.IsNullOrWhiteSpace(configuredPath))
        {
            if (!Path.IsPathFullyQualified(configuredPath))
            {
                throw new InvalidOperationException("LESERPENT_RUST_BRIDGE_BIN must be an absolute path");
            }
            executablePath = Path.GetFullPath(configuredPath);
            if (!File.Exists(executablePath))
            {
                throw new FileNotFoundException("LESERPENT_RUST_BRIDGE_BIN does not exist", executablePath);
            }
        }

        var configuredTimeout = configuration.GetValue<int?>("LESERPENT_RUST_BRIDGE_TIMEOUT_MS") ?? 2000;
        timeout = TimeSpan.FromMilliseconds(Math.Clamp(configuredTimeout, 100, 30_000));
    }

    public bool Enabled => executablePath is not null;

    public Task ValidateRuntimeListAsync(
        RuntimeCollectionResponse response,
        CancellationToken cancellationToken) =>
        ValidateAsync(
            "validate_runtime_list",
            response,
            global::Leserpent.LeserpentJsonContext.Default.RuntimeCollectionResponse,
            cancellationToken);

    public Task ValidateStatusRefreshAsync(
        RuntimeStatusRefreshResponse response,
        CancellationToken cancellationToken) =>
        ValidateAsync(
            "validate_status_refresh",
            response,
            global::Leserpent.LeserpentJsonContext.Default.RuntimeStatusRefreshResponse,
            cancellationToken);

    public Task<RuntimeDeploymentCompatibilityEnvelope> NormalizeRuntimeDeploymentRequestAsync(
        RuntimeDeploymentCompatibilityEnvelope request,
        CancellationToken cancellationToken) =>
        NormalizeAsync(
            "normalize_runtime_deployment_request",
            request,
            global::Leserpent.LeserpentJsonContext.Default.RuntimeDeploymentCompatibilityEnvelope,
            cancellationToken);

    public void Dispose()
    {
        ResetProcess();
        requestLock.Dispose();
    }

    private async Task ValidateAsync<T>(
        string operation,
        T payload,
        JsonTypeInfo<T> payloadType,
        CancellationToken cancellationToken)
    {
        _ = await ExchangeAsync(operation, payload, payloadType, cancellationToken);
    }

    private async Task<T> NormalizeAsync<T>(
        string operation,
        T payload,
        JsonTypeInfo<T> payloadType,
        CancellationToken cancellationToken)
    {
        if (!Enabled)
        {
            return payload;
        }
        var normalized = await ExchangeAsync(operation, payload, payloadType, cancellationToken);
        if (normalized is null)
        {
            throw new CompatibilityBridgeException("Rust compatibility bridge omitted normalized payload");
        }
        return normalized.Value.Deserialize(payloadType)
            ?? throw new CompatibilityBridgeException("Rust compatibility bridge returned an empty normalized payload");
    }

    private async Task<JsonElement?> ExchangeAsync<T>(
        string operation,
        T payload,
        JsonTypeInfo<T> payloadType,
        CancellationToken cancellationToken)
    {
        if (!Enabled)
        {
            return null;
        }

        var requestId = Guid.NewGuid().ToString("N");
        var request = BuildRequest(requestId, operation, payload, payloadType);
        if (Encoding.UTF8.GetByteCount(request) > MaxMessageBytes)
        {
            throw new CompatibilityBridgeException("compatibility request exceeds 1 MiB");
        }

        await requestLock.WaitAsync(cancellationToken);
        try
        {
            for (var attempt = 0; attempt < 2; attempt++)
            {
                using var deadline = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                deadline.CancelAfter(timeout);
                try
                {
                    var activeProcess = EnsureProcess();
                    await activeProcess.StandardInput.WriteLineAsync(request.AsMemory(), deadline.Token);
                    await activeProcess.StandardInput.FlushAsync(deadline.Token);
                    var line = await ReadBoundedResponseLineAsync(
                        activeProcess.StandardOutput.BaseStream,
                        deadline.Token);
                    return ValidateResponse(requestId, line);
                }
                catch (Exception error) when (
                    !cancellationToken.IsCancellationRequested
                    && error is IOException or InvalidDataException or JsonException or Win32Exception or OperationCanceledException)
                {
                    ResetProcess();
                    if (attempt == 0)
                    {
                        logger.LogWarning(error, "Restarting the Rust compatibility bridge after a failed request");
                        continue;
                    }
                    var message = error is OperationCanceledException
                        ? "Rust compatibility bridge timed out"
                        : "Rust compatibility bridge transport failed";
                    throw new CompatibilityBridgeException(message);
                }
            }
        }
        finally
        {
            requestLock.Release();
        }

        throw new CompatibilityBridgeException("Rust compatibility bridge request failed");
    }

    internal static async Task<string?> ReadBoundedResponseLineAsync(
        Stream output,
        CancellationToken cancellationToken)
    {
        using var message = new MemoryStream();
        var buffer = new byte[4096];
        while (true)
        {
            var read = await output.ReadAsync(buffer.AsMemory(), cancellationToken);
            if (read == 0)
            {
                return message.Length == 0 ? null : DecodeResponseLine(message);
            }

            var newline = Array.IndexOf(buffer, (byte)'\n', 0, read);
            var bytesToAppend = newline >= 0 ? newline : read;
            if (message.Length + bytesToAppend > MaxMessageBytes)
            {
                throw new InvalidDataException("Rust compatibility bridge response exceeds 1 MiB");
            }
            message.Write(buffer, 0, bytesToAppend);

            if (newline < 0)
            {
                continue;
            }
            if (newline + 1 != read)
            {
                throw new InvalidDataException("Rust compatibility bridge emitted trailing response data");
            }
            return DecodeResponseLine(message);
        }
    }

    private static string DecodeResponseLine(MemoryStream message)
    {
        var length = checked((int)message.Length);
        var bytes = message.GetBuffer();
        if (length > 0 && bytes[length - 1] == (byte)'\r')
        {
            length--;
        }
        try
        {
            return StrictUtf8.GetString(bytes, 0, length);
        }
        catch (DecoderFallbackException error)
        {
            throw new InvalidDataException("Rust compatibility bridge response is not valid UTF-8", error);
        }
    }

    private Process EnsureProcess()
    {
        if (process is { HasExited: false })
        {
            return process;
        }
        ResetProcess();
        var startInfo = new ProcessStartInfo
        {
            FileName = executablePath!,
            WorkingDirectory = Path.GetDirectoryName(executablePath!)!,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
        };
        process = Process.Start(startInfo)
            ?? throw new CompatibilityBridgeException("failed to start Rust compatibility bridge");
        return process;
    }

    private void ResetProcess()
    {
        if (process is null)
        {
            return;
        }
        try
        {
            if (!process.HasExited)
            {
                process.Kill(entireProcessTree: true);
                process.WaitForExit(1000);
            }
        }
        catch (InvalidOperationException)
        {
            // The process exited between the state check and cleanup.
        }
        finally
        {
            process.Dispose();
            process = null;
        }
    }

    private static string BuildRequest<T>(
        string requestId,
        string operation,
        T payload,
        JsonTypeInfo<T> payloadType)
    {
        var buffer = new CappedBufferWriter(MaxMessageBytes);
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartObject();
            writer.WriteString("request_id", requestId);
            writer.WriteString("operation", operation);
            writer.WritePropertyName("payload");
            JsonSerializer.Serialize(writer, payload, payloadType);
            writer.WriteEndObject();
        }
        return StrictUtf8.GetString(buffer.WrittenSpan);
    }

    private sealed class CappedBufferWriter(int maximumBytes) : IBufferWriter<byte>
    {
        private readonly ArrayBufferWriter<byte> buffer = new(Math.Min(maximumBytes, 4096));

        public ReadOnlySpan<byte> WrittenSpan => buffer.WrittenSpan;

        public void Advance(int count)
        {
            if (count < 0 || count > maximumBytes - buffer.WrittenCount)
            {
                throw new CompatibilityBridgeException("compatibility request exceeds 1 MiB");
            }
            buffer.Advance(count);
        }

        public Memory<byte> GetMemory(int sizeHint = 0)
        {
            EnsureCapacity(sizeHint);
            return buffer.GetMemory(sizeHint);
        }

        public Span<byte> GetSpan(int sizeHint = 0)
        {
            EnsureCapacity(sizeHint);
            return buffer.GetSpan(sizeHint);
        }

        private void EnsureCapacity(int sizeHint)
        {
            if (sizeHint < 0)
            {
                throw new ArgumentOutOfRangeException(nameof(sizeHint));
            }
            var required = Math.Max(sizeHint, 1);
            if (required > maximumBytes - buffer.WrittenCount)
            {
                throw new CompatibilityBridgeException("compatibility request exceeds 1 MiB");
            }
        }
    }

    private static JsonElement? ValidateResponse(string requestId, string? line)
    {
        if (line is null)
        {
            throw new IOException("Rust compatibility bridge closed stdout");
        }
        if (Encoding.UTF8.GetByteCount(line) > MaxMessageBytes)
        {
            throw new InvalidDataException("Rust compatibility bridge response exceeds 1 MiB");
        }
        using var document = JsonDocument.Parse(line);
        var root = document.RootElement;
        if (!root.TryGetProperty("request_id", out var responseId)
            || !string.Equals(responseId.GetString(), requestId, StringComparison.Ordinal))
        {
            throw new InvalidDataException("Rust compatibility bridge response ID mismatch");
        }
        if (!root.TryGetProperty("ok", out var ok) || !ok.GetBoolean())
        {
            var message = root.TryGetProperty("error", out var error)
                && error.TryGetProperty("message", out var detail)
                ? detail.GetString()
                : "Rust compatibility bridge rejected the payload";
            throw new CompatibilityBridgeException(message ?? "Rust compatibility bridge rejected the payload");
        }
        return root.TryGetProperty("payload", out var payload) ? payload.Clone() : null;
    }
}

public sealed class CompatibilityBridgeException(string message) : Exception(message);
