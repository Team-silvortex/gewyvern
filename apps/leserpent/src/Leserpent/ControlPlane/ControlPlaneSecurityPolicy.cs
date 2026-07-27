using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Http.Features;

namespace Leserpent.ControlPlane;

public sealed class ControlPlaneSecurityPolicy
{
    public const string AdminTokenHeader = "X-Leserpent-Admin-Token";
    public const string IntentHeader = "X-Leserpent-Intent";
    public const string MutateIntent = "mutate";
    public const string ExportIntent = "export";
    public const long PersistenceImportBodyLimitBytes = 1_048_576;

    private readonly string? adminToken;

    public ControlPlaneSecurityPolicy(IConfiguration configuration)
    {
        adminToken = NormalizeToken(
            configuration["LESERPENT_ADMIN_TOKEN"]
            ?? configuration["Leserpent:AdminToken"]);
        PublicEndpointDiscoveryAllowed = ParseBoolean(
            configuration["LESERPENT_ALLOW_PUBLIC_ENDPOINTS"]
            ?? configuration["Leserpent:AllowPublicEndpoints"]);
    }

    public bool AdminTokenConfigured => !string.IsNullOrWhiteSpace(adminToken);

    public bool PublicEndpointDiscoveryAllowed { get; }

    public string ApiMode => AdminTokenConfigured ? "loopback_or_token" : "loopback_only";

    public bool TryAuthorize(HttpContext context, out int statusCode, out ApiErrorResponse payload)
    {
        statusCode = StatusCodes.Status200OK;
        payload = new ApiErrorResponse("none");

        var path = context.Request.Path;
        if (!path.StartsWithSegments("/v1") && !path.StartsWithSegments("/health"))
        {
            return true;
        }

        if (IsPersistenceImport(path))
        {
            ApplyPersistenceImportLimit(context);
            if (context.Request.ContentLength > PersistenceImportBodyLimitBytes)
            {
                statusCode = StatusCodes.Status413PayloadTooLarge;
                payload = new ApiErrorResponse(
                    "persistence_import_too_large",
                    MaxBytes: PersistenceImportBodyLimitBytes);
                return false;
            }
        }

        var isLoopback = IsLoopbackRequest(context);
        var hasValidToken = HasValidAdminToken(context.Request);
        if (!isLoopback && !hasValidToken)
        {
            statusCode = StatusCodes.Status403Forbidden;
            payload = new ApiErrorResponse(
                "api_access_denied",
                "leserpent API access is limited to loopback clients unless a valid admin token is supplied");
            return false;
        }

        var requiresIntentHeader = RequiresIntentHeader(context.Request);
        if (!requiresIntentHeader || hasValidToken)
        {
            return true;
        }

        var expectedIntent = string.Equals(context.Request.Path, "/v1/persistence/export", StringComparison.OrdinalIgnoreCase)
            ? ExportIntent
            : MutateIntent;

        if (!isLoopback || !HasIntent(context.Request, expectedIntent))
        {
            statusCode = StatusCodes.Status400BadRequest;
            payload = new ApiErrorResponse(
                "missing_control_plane_intent",
                $"{IntentHeader}: {expectedIntent} is required for this operation when no admin token is supplied");
            return false;
        }

        return true;
    }

    public async Task<string?> ValidateRegistrationAsync(RuntimeRegistrationRequest request, CancellationToken cancellationToken)
    {
        var runtimeError = await ValidateEndpointUrlAsync(request.Endpoint, "runtime endpoint", cancellationToken);
        if (runtimeError is not null)
        {
            return runtimeError;
        }

        var sidecarError = await ValidateOptionalEndpointUrlAsync(request.SidecarEndpoint, "sidecar endpoint", cancellationToken);
        if (sidecarError is not null)
        {
            return sidecarError;
        }

        var capabilityError = await ValidateOptionalEndpointUrlAsync(request.CapabilityEndpoint, "capability endpoint", cancellationToken);
        if (capabilityError is not null)
        {
            return capabilityError;
        }

        var statusError = await ValidateOptionalEndpointUrlAsync(request.StatusEndpoint, "status endpoint", cancellationToken);
        if (statusError is not null)
        {
            return statusError;
        }

        var sidecarStatusError = await ValidateOptionalEndpointUrlAsync(request.SidecarStatusEndpoint, "sidecar status endpoint", cancellationToken);
        return sidecarStatusError;
    }

    public async Task<string?> ValidateRegistrationPlanAsync(RuntimeRegistrationPlanRequest request, CancellationToken cancellationToken)
    {
        var runtimeError = await ValidateEndpointUrlAsync(request.Endpoint, "runtime endpoint", cancellationToken);
        return runtimeError ?? await ValidateOptionalEndpointUrlAsync(request.SidecarEndpoint, "sidecar endpoint", cancellationToken);
    }

    public async Task<string?> ValidateImportAsync(PersistedControlPlaneState state, CancellationToken cancellationToken)
    {
        if ((state.PendingRuntimeDeletions?.Count ?? 0) > 0)
        {
            return "pending runtime deletion intents cannot be imported";
        }

        foreach (var runtime in state.Runtimes)
        {
            var runtimeError = await ValidateEndpointUrlAsync(runtime.Endpoint, $"runtime endpoint for {runtime.Name}", cancellationToken);
            if (runtimeError is not null)
            {
                return runtimeError;
            }

            var sidecarError = await ValidateOptionalEndpointUrlAsync(runtime.SidecarEndpoint, $"sidecar endpoint for {runtime.Name}", cancellationToken);
            if (sidecarError is not null)
            {
                return sidecarError;
            }
        }

        return null;
    }

    public async Task<string?> ValidateEndpointUrlAsync(string url, string label, CancellationToken cancellationToken)
    {
        var result = await BuildEndpointAccessPlanAsync(url, label, cancellationToken);
        return result.Error;
    }

    public async Task<EndpointAccessPlanResult> BuildEndpointAccessPlanAsync(string url, string label, CancellationToken cancellationToken)
    {
        if (!Uri.TryCreate(url.Trim(), UriKind.Absolute, out var uri))
        {
            return EndpointAccessPlanResult.Failed($"{label} is not a valid absolute URL");
        }

        if (!string.Equals(uri.Scheme, Uri.UriSchemeHttp, StringComparison.OrdinalIgnoreCase)
            && !string.Equals(uri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase))
        {
            return EndpointAccessPlanResult.Failed($"{label} must use http or https");
        }

        if (!string.IsNullOrWhiteSpace(uri.UserInfo))
        {
            return EndpointAccessPlanResult.Failed($"{label} may not embed user credentials");
        }

        if (PublicEndpointDiscoveryAllowed)
        {
            return EndpointAccessPlanResult.Succeeded(new EndpointAccessPlan(uri, null));
        }

        var addresses = await ResolveAddressesAsync(uri.Host, cancellationToken);
        if (addresses.Length == 0)
        {
            return EndpointAccessPlanResult.Failed($"{label} host could not be resolved");
        }

        if (addresses.Any(address => !IsAllowedDiscoveryAddress(address)))
        {
            return EndpointAccessPlanResult.Failed($"{label} must resolve only to loopback or private-network addresses unless LESERPENT_ALLOW_PUBLIC_ENDPOINTS=true");
        }

        return EndpointAccessPlanResult.Succeeded(new EndpointAccessPlan(uri, SelectPinnedAddress(addresses)));
    }

    private Task<string?> ValidateOptionalEndpointUrlAsync(string? url, string label, CancellationToken cancellationToken) =>
        string.IsNullOrWhiteSpace(url)
            ? Task.FromResult<string?>(null)
            : ValidateEndpointUrlAsync(url, label, cancellationToken);

    private static bool RequiresIntentHeader(HttpRequest request)
    {
        if (string.Equals(request.Path, "/v1/persistence/export", StringComparison.OrdinalIgnoreCase))
        {
            return true;
        }

        return ControlPlaneMutationPolicy.IsMutation(request);
    }

    private bool HasValidAdminToken(HttpRequest request)
    {
        if (!AdminTokenConfigured)
        {
            return false;
        }

        var supplied = request.Headers[AdminTokenHeader].ToString().Trim();
        return TokenEquals(supplied, adminToken);
    }

    private static bool TokenEquals(string supplied, string? expected)
    {
        if (string.IsNullOrWhiteSpace(supplied) || string.IsNullOrWhiteSpace(expected))
        {
            return false;
        }

        var suppliedBytes = Encoding.UTF8.GetBytes(supplied);
        var expectedBytes = Encoding.UTF8.GetBytes(expected);
        return suppliedBytes.Length == expectedBytes.Length
            && CryptographicOperations.FixedTimeEquals(suppliedBytes, expectedBytes);
    }

    private static bool HasIntent(HttpRequest request, string expectedIntent) =>
        string.Equals(request.Headers[IntentHeader].ToString().Trim(), expectedIntent, StringComparison.OrdinalIgnoreCase);

    private static bool IsPersistenceImport(PathString path) =>
        string.Equals(path, "/v1/persistence/import", StringComparison.OrdinalIgnoreCase);

    private static void ApplyPersistenceImportLimit(HttpContext context)
    {
        var maxBodySizeFeature = context.Features.Get<IHttpMaxRequestBodySizeFeature>();
        if (maxBodySizeFeature is { IsReadOnly: false })
        {
            maxBodySizeFeature.MaxRequestBodySize = PersistenceImportBodyLimitBytes;
        }
    }

    private static bool IsLoopbackRequest(HttpContext context)
    {
        var address = context.Connection.RemoteIpAddress;
        if (address is null)
        {
            return false;
        }

        if (address.IsIPv4MappedToIPv6)
        {
            address = address.MapToIPv4();
        }

        return IPAddress.IsLoopback(address);
    }

    private static async Task<IPAddress[]> ResolveAddressesAsync(string host, CancellationToken cancellationToken)
    {
        if (IPAddress.TryParse(host, out var parsed))
        {
            return [NormalizeAddress(parsed)];
        }

        try
        {
            return (await Dns.GetHostAddressesAsync(host, cancellationToken))
                .Select(NormalizeAddress)
                .Distinct()
                .ToArray();
        }
        catch
        {
            return [];
        }
    }

    private static IPAddress NormalizeAddress(IPAddress address) =>
        address.IsIPv4MappedToIPv6 ? address.MapToIPv4() : address;

    private static IPAddress SelectPinnedAddress(IEnumerable<IPAddress> addresses) =>
        addresses
            .OrderByDescending(IPAddress.IsLoopback)
            .ThenBy(address => address.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork ? 0 : 1)
            .First();

    private static bool IsAllowedDiscoveryAddress(IPAddress address)
    {
        address = NormalizeAddress(address);
        if (IPAddress.IsLoopback(address))
        {
            return true;
        }

        if (address.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork)
        {
            var bytes = address.GetAddressBytes();
            return bytes[0] == 10
                || (bytes[0] == 172 && bytes[1] >= 16 && bytes[1] <= 31)
                || (bytes[0] == 192 && bytes[1] == 168);
        }

        if (address.AddressFamily == System.Net.Sockets.AddressFamily.InterNetworkV6)
        {
            var bytes = address.GetAddressBytes();
            return (bytes[0] & 0xfe) == 0xfc;
        }

        return false;
    }

    private static bool ParseBoolean(string? value) =>
        string.Equals(value?.Trim(), "1", StringComparison.OrdinalIgnoreCase)
        || string.Equals(value?.Trim(), "true", StringComparison.OrdinalIgnoreCase)
        || string.Equals(value?.Trim(), "yes", StringComparison.OrdinalIgnoreCase)
        || string.Equals(value?.Trim(), "on", StringComparison.OrdinalIgnoreCase);

    private static string? NormalizeToken(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }

        return value.Trim();
    }
}

public sealed record EndpointAccessPlan(
    Uri RequestUri,
    IPAddress? PinnedAddress);

public sealed record EndpointAccessPlanResult(
    EndpointAccessPlan? Plan,
    string? Error)
{
    public static EndpointAccessPlanResult Succeeded(EndpointAccessPlan plan) => new(plan, null);

    public static EndpointAccessPlanResult Failed(string error) => new(null, error);
}
