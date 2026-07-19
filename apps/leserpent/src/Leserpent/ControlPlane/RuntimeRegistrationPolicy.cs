using System.Security.Cryptography;
using System.Text;

namespace Leserpent.ControlPlane;

public static class RuntimeRegistrationPolicy
{
    public const string CreateAction = "create";
    public const string UpdateAction = "update";
    public const string RejectAction = "reject";
    public const string EndpointConflictReason = "endpoint_conflict";

    public static RuntimeRegistrationPlan Build(
        RuntimeRegistrationPlanRequest request,
        IReadOnlyList<RuntimeSummary> runtimes)
    {
        var name = request.Name.Trim();
        var endpoint = request.Endpoint.Trim();
        var endpointIdentity = NormalizeEndpointIdentity(endpoint);
        var sameName = runtimes.FirstOrDefault(runtime =>
            string.Equals(runtime.Name, name, StringComparison.OrdinalIgnoreCase));
        var sameEndpoint = runtimes.FirstOrDefault(runtime =>
            string.Equals(
                NormalizeEndpointIdentity(runtime.Endpoint),
                endpointIdentity,
                StringComparison.Ordinal));
        var endpointConflict = sameEndpoint is not null &&
            !string.Equals(sameEndpoint.RuntimeId, sameName?.RuntimeId, StringComparison.OrdinalIgnoreCase);

        var action = endpointConflict ? RejectAction : sameName is null ? CreateAction : UpdateAction;
        var existing = endpointConflict ? sameEndpoint : sameName;
        var tokenParts = new[]
        {
            name.ToLowerInvariant(),
            endpointIdentity,
            action,
            existing?.RuntimeId.ToLowerInvariant() ?? string.Empty,
        };
        var token = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(string.Join('\n', tokenParts))))
            .ToLowerInvariant();
        return new RuntimeRegistrationPlan(
            !endpointConflict,
            action,
            endpointConflict ? EndpointConflictReason : null,
            existing?.RuntimeId,
            existing?.Name,
            existing?.Endpoint,
            token);
    }

    private static string NormalizeEndpointIdentity(string endpoint)
    {
        if (!Uri.TryCreate(endpoint.Trim(), UriKind.Absolute, out var uri))
        {
            return endpoint.Trim();
        }
        return uri.GetComponents(
            UriComponents.SchemeAndServer | UriComponents.PathAndQuery,
            UriFormat.SafeUnescaped);
    }
}

public sealed class RuntimeRegistrationPlanException(string reason, RuntimeRegistrationPlan plan)
    : InvalidOperationException(reason)
{
    public RuntimeRegistrationPlan Plan { get; } = plan;
}
