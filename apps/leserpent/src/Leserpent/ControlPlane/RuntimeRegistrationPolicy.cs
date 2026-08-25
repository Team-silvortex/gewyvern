using System.Security.Cryptography;
using System.Text;

namespace Leserpent.ControlPlane;

public static class RuntimeRegistrationPolicy
{
    public const string CreateAction = "create";
    public const string UpdateAction = "update";
    public const string RejectAction = "reject";
    public const string EndpointConflictReason = "endpoint_conflict";
    public const string RuntimeDeletionInProgressReason =
        "runtime_deletion_in_progress";
    public const string RuntimeRegistrationRecoveryPendingReason =
        "runtime_registration_recovery_pending";

    public static RuntimeRegistrationPlan Build(
        RuntimeRegistrationPlanRequest request,
        IReadOnlyList<RuntimeSummary> runtimes) =>
        Build(
            request,
            runtimes.Select(runtime => new RegistrationCandidate(
                runtime.RuntimeId,
                runtime.Name,
                runtime.Endpoint,
                null)).ToArray(),
            null,
            authorityBound: false);

    internal static RuntimeRegistrationPlan BuildAuthoritative(
        RuntimeRegistrationPlanRequest request,
        IReadOnlyList<DaemonRuntimeProjection> runtimes,
        string plannedCreateRuntimeId) =>
        Build(
            request,
            runtimes.Select(runtime => new RegistrationCandidate(
                runtime.RuntimeId,
                runtime.Name,
                runtime.Endpoint,
                runtime.Revision)).ToArray(),
            plannedCreateRuntimeId,
            authorityBound: true);

    internal static string BuildProposedRuntimeId(string name, string endpoint)
    {
        var normalizedName = name.Trim().ToLowerInvariant();
        var normalizedEndpoint = NormalizeEndpointIdentity(endpoint);
        var bytes = SHA256.HashData(
            Encoding.UTF8.GetBytes($"{normalizedName}\u0000{normalizedEndpoint}"));
        return Convert.ToHexString(bytes).ToLowerInvariant()[..32];
    }

    internal static RuntimeRegistrationPlan RejectAuthoritative(
        RuntimeRegistrationPlanRequest request,
        RuntimeRegistrationPlan plan,
        string reason)
    {
        if (!plan.AuthorityBound)
        {
            throw new ArgumentException(
                "only an authority-bound registration plan can be rejected here",
                nameof(plan));
        }
        return Reject(request, plan, reason);
    }

    internal static RuntimeRegistrationPlan Reject(
        RuntimeRegistrationPlanRequest request,
        RuntimeRegistrationPlan plan,
        string reason)
    {
        return plan with
        {
            Allowed = false,
            Action = RejectAction,
            Reason = reason,
            PlanToken = BuildToken(
                request,
                RejectAction,
                plan.PlannedRuntimeId,
                plan.ExpectedRevision,
                plan.AuthorityBound),
        };
    }

    internal static RuntimeRegistrationPlan BuildRecovery(
        RuntimeRegistrationPlanRequest request,
        PersistedRuntimeRegistrationIntent intent,
        bool allowed)
    {
        var action = allowed ? intent.Action : RejectAction;
        return new RuntimeRegistrationPlan(
            allowed,
            action,
            RuntimeRegistrationRecoveryPendingReason,
            intent.Action == UpdateAction ? intent.RuntimeId : null,
            intent.Action == UpdateAction ? intent.Name : null,
            null,
            intent.RuntimeId,
            intent.ExpectedRevision,
            true,
            BuildToken(
                request,
                action,
                intent.RuntimeId,
                intent.ExpectedRevision,
                authorityBound: true));
    }

    private static RuntimeRegistrationPlan Build(
        RuntimeRegistrationPlanRequest request,
        IReadOnlyList<RegistrationCandidate> runtimes,
        string? plannedCreateRuntimeId,
        bool authorityBound)
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
        var plannedRuntimeId = existing?.RuntimeId ?? plannedCreateRuntimeId;
        var expectedRevision = existing?.Revision;
        var token = BuildToken(
            request,
            action,
            plannedRuntimeId,
            expectedRevision,
            authorityBound);
        return new RuntimeRegistrationPlan(
            !endpointConflict,
            action,
            endpointConflict ? EndpointConflictReason : null,
            existing?.RuntimeId,
            existing?.Name,
            existing?.Endpoint,
            plannedRuntimeId,
            expectedRevision,
            authorityBound,
            token);
    }

    private static string BuildToken(
        RuntimeRegistrationPlanRequest request,
        string action,
        string? plannedRuntimeId,
        ulong? expectedRevision,
        bool authorityBound)
    {
        var tokenParts = new[]
        {
            "runtime-registration-plan-v2",
            request.Name.Trim().ToLowerInvariant(),
            NormalizeEndpointIdentity(request.Endpoint),
            string.IsNullOrWhiteSpace(request.SidecarEndpoint)
                ? string.Empty
                : NormalizeEndpointIdentity(request.SidecarEndpoint),
            authorityBound ? "daemon" : "managed",
            action,
            plannedRuntimeId?.ToLowerInvariant() ?? string.Empty,
            expectedRevision?.ToString(
                System.Globalization.CultureInfo.InvariantCulture)
                ?? string.Empty,
        };
        return Convert.ToHexString(SHA256.HashData(
            Encoding.UTF8.GetBytes(string.Join('\n', tokenParts))))
            .ToLowerInvariant();
    }

    private sealed record RegistrationCandidate(
        string RuntimeId,
        string Name,
        string Endpoint,
        ulong? Revision);

    internal static bool EndpointIdentityEquals(
        string left,
        string right) =>
        string.Equals(
            NormalizeEndpointIdentity(left),
            NormalizeEndpointIdentity(right),
            StringComparison.Ordinal);

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
