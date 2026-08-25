namespace Leserpent.ControlPlane;

internal enum RuntimeRegistrationExecutionFailureKind
{
    InvalidRequest,
    Conflict,
    NotFound,
    Gateway,
}

internal sealed class RuntimeRegistrationExecutionException :
    InvalidOperationException
{
    private RuntimeRegistrationExecutionException(
        RuntimeRegistrationExecutionFailureKind kind,
        string code,
        string message,
        string? runtimeId,
        RuntimeRegistrationPlan? plan = null,
        Exception? innerException = null)
        : base(message, innerException)
    {
        Kind = kind;
        Code = code;
        RuntimeId = runtimeId;
        Plan = plan;
    }

    internal RuntimeRegistrationExecutionFailureKind Kind { get; }
    internal string Code { get; }
    internal string? RuntimeId { get; }
    internal RuntimeRegistrationPlan? Plan { get; }

    internal static RuntimeRegistrationExecutionException Invalid(
        string message) =>
        new(
            RuntimeRegistrationExecutionFailureKind.InvalidRequest,
            "invalid_runtime_registration",
            message,
            null);

    internal static RuntimeRegistrationExecutionException PlanRequired(
        RuntimeRegistrationPlan plan) =>
        new(
            RuntimeRegistrationExecutionFailureKind.Conflict,
            "runtime_registration_plan_required",
            "review the current daemon registration plan before registering",
            PlanRuntimeId(plan),
            plan);

    internal static RuntimeRegistrationExecutionException PlanChanged(
        RuntimeRegistrationPlan plan,
        string message,
        Exception? innerException = null) =>
        new(
            RuntimeRegistrationExecutionFailureKind.Conflict,
            "runtime_registration_plan_changed",
            message,
            PlanRuntimeId(plan),
            plan,
            innerException);

    internal static RuntimeRegistrationExecutionException FromAuthority(
        DaemonRuntimeRegistrationException error,
        string runtimeId) =>
        error.Code switch
        {
            "runtime_already_exists" or "idempotency_conflict" or
                "revision_conflict" => new(
                    RuntimeRegistrationExecutionFailureKind.Conflict,
                    "runtime_registration_conflict",
                    error.Message,
                    runtimeId,
                    innerException: error),
            "runtime_not_found" => new(
                RuntimeRegistrationExecutionFailureKind.NotFound,
                "runtime_not_found",
                error.Message,
                runtimeId,
                innerException: error),
            _ => new(
                RuntimeRegistrationExecutionFailureKind.Gateway,
                "runtime_registration_rejected",
                error.Message,
                runtimeId,
                innerException: error),
        };

    private static string? PlanRuntimeId(RuntimeRegistrationPlan plan) =>
        plan.PlannedRuntimeId ?? plan.ExistingRuntimeId;
}

internal sealed class RuntimeRegistrationExecutionService(
    RegistryService registry,
    CapabilityDiscoveryService discovery,
    IRuntimeRegistrationAuthority registrationAuthority,
    RuntimeRegistrationPlanProjectionService registrationPlans,
    RuntimeRegistrationCommitProjectionService registrationCommits,
    ControlPlaneSecurityPolicy security)
{
    internal async Task<RuntimeRegistrationResponse> ExecuteAsync(
        RuntimeRegistrationRequest request,
        CancellationToken cancellationToken)
    {
        request = await NormalizeAndValidateAsync(request, cancellationToken);
        var plan = await registrationPlans.BuildAsync(
            new RuntimeRegistrationPlanRequest(
                request.Name,
                request.Endpoint,
                request.SidecarEndpoint),
            cancellationToken);
        ValidateReviewedPlan(request, plan);
        ValidateAuthorityConfiguration(plan);
        var runtimeId = ResolveAuthorityRuntimeId(plan);

        try
        {
            return request.FetchCapabilities
                ? await ExecuteWithDiscoveryAsync(
                    request,
                    plan,
                    runtimeId,
                    cancellationToken)
                : await ExecuteManualAsync(
                    request,
                    plan,
                    runtimeId,
                    cancellationToken);
        }
        catch (RuntimeRegistrationPlanException error)
        {
            throw RuntimeRegistrationExecutionException.PlanChanged(
                error.Plan,
                error.Message,
                error);
        }
        catch (DaemonRuntimeRegistrationException error)
            when (runtimeId is not null)
        {
            throw RuntimeRegistrationExecutionException.FromAuthority(
                error,
                runtimeId);
        }
    }

    private async Task<RuntimeRegistrationRequest> NormalizeAndValidateAsync(
        RuntimeRegistrationRequest request,
        CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(request.Name)
            || string.IsNullOrWhiteSpace(request.Endpoint))
        {
            throw RuntimeRegistrationExecutionException.Invalid(
                "name and endpoint are required");
        }

        var normalized = request with
        {
            Name = request.Name.Trim(),
            Endpoint = request.Endpoint.Trim(),
        };
        var validation = await security.ValidateRegistrationAsync(
            normalized,
            cancellationToken);
        return validation is null
            ? normalized
            : throw RuntimeRegistrationExecutionException.Invalid(validation);
    }

    private static void ValidateReviewedPlan(
        RuntimeRegistrationRequest request,
        RuntimeRegistrationPlan plan)
    {
        if (!plan.Allowed)
        {
            var message = plan.Reason ==
                RuntimeRegistrationPolicy.RuntimeDeletionInProgressReason
                    ? "runtime deletion is in progress; review the plan after cleanup completes"
                    : "runtime endpoint is already registered to another runtime";
            throw RuntimeRegistrationExecutionException.PlanChanged(
                plan,
                message);
        }
        if (plan.AuthorityBound
            && string.IsNullOrWhiteSpace(request.RegistrationPlanToken))
        {
            throw RuntimeRegistrationExecutionException.PlanRequired(plan);
        }
        if (!string.IsNullOrWhiteSpace(request.RegistrationPlanToken)
            && !string.Equals(
                request.RegistrationPlanToken,
                plan.PlanToken,
                StringComparison.Ordinal))
        {
            throw RuntimeRegistrationExecutionException.PlanChanged(
                plan,
                "runtime registration plan changed; review the current target before retrying");
        }
    }

    private void ValidateAuthorityConfiguration(
        RuntimeRegistrationPlan plan)
    {
        if (registrationAuthority.Enabled != plan.AuthorityBound)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_configuration_mismatch",
                "runtime registration authority and projection authority are inconsistent");
        }
    }

    private static string? ResolveAuthorityRuntimeId(
        RuntimeRegistrationPlan plan)
    {
        if (!plan.AuthorityBound)
        {
            return null;
        }

        var runtimeId = plan.PlannedRuntimeId;
        var updatePlanInvalid = plan.Action ==
                RuntimeRegistrationPolicy.UpdateAction
            && (plan.ExpectedRevision is null
                || !string.Equals(
                    plan.ExistingRuntimeId,
                    runtimeId,
                    StringComparison.Ordinal));
        var createPlanInvalid = plan.Action ==
                RuntimeRegistrationPolicy.CreateAction
            && plan.ExpectedRevision is not null;
        if (string.IsNullOrWhiteSpace(runtimeId)
            || updatePlanInvalid
            || createPlanInvalid)
        {
            throw new DaemonRuntimeProjectionException(
                "daemon_projection_invalid_registration_plan",
                "daemon registration plan omitted or confused its runtime authority");
        }
        return runtimeId;
    }

    private async Task<RuntimeRegistrationResponse>
        ExecuteWithDiscoveryAsync(
            RuntimeRegistrationRequest request,
            RuntimeRegistrationPlan plan,
            string? runtimeId,
            CancellationToken cancellationToken)
    {
        var capabilityDiscovery = await discovery.DiscoverAsync(
            request.Endpoint,
            request.CapabilityEndpoint,
            cancellationToken,
            request.PairingToken);
        var statusDiscovery = await discovery.DiscoverStatusAsync(
            request.Endpoint,
            request.StatusEndpoint,
            cancellationToken,
            request.PairingToken);
        var sidecarDiscovery = string.IsNullOrWhiteSpace(
            request.SidecarEndpoint)
                ? null
                : await discovery.DiscoverSidecarStatusAsync(
                    request.SidecarEndpoint,
                    request.SidecarStatusEndpoint,
                    request.SidecarAdminToken,
                    cancellationToken);

        RuntimeRegistrationResponse registered;
        if (runtimeId is null)
        {
            registered = registry.RegisterRuntimeFromDiscovery(
                request,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);
        }
        else
        {
            var receipt = await registrationAuthority.RegisterWithReceiptAsync(
                request,
                runtimeId,
                cancellationToken,
                update: plan.Action == RuntimeRegistrationPolicy.UpdateAction,
                capabilityDiscovery: capabilityDiscovery,
                statusDiscovery: statusDiscovery,
                sidecarDiscovery: sidecarDiscovery,
                expectedRevision: plan.ExpectedRevision);
            var authorityCommit = registrationCommits.Bind(
                runtimeId,
                request,
                receipt,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery);
            registered = registry.RegisterRuntimeFromAuthority(
                authorityCommit.Request,
                authorityCommit.Runtime,
                authorityCommit.CapabilityDiscovery);
        }

        registry.RecordRecoveryActivity(
            registered.RuntimeId,
            "register_runtime",
            RuntimeRefreshOutcomePolicy.Determine(
                registered.Status.StatusSource,
                registered.Status.StatusFetchError,
                registered.SidecarStatus?.StatusSource,
                registered.SidecarStatus?.StatusFetchError),
            "runtime registered through discovery");
        return registered;
    }

    private async Task<RuntimeRegistrationResponse> ExecuteManualAsync(
        RuntimeRegistrationRequest request,
        RuntimeRegistrationPlan plan,
        string? runtimeId,
        CancellationToken cancellationToken)
    {
        RuntimeRegistrationResponse registered;
        if (runtimeId is null)
        {
            registered = registry.RegisterRuntime(request);
        }
        else
        {
            var receipt = await registrationAuthority.RegisterWithReceiptAsync(
                request,
                runtimeId,
                cancellationToken,
                update: plan.Action == RuntimeRegistrationPolicy.UpdateAction,
                expectedRevision: plan.ExpectedRevision);
            var authorityCommit = registrationCommits.Bind(
                runtimeId,
                request,
                receipt);
            registered = registry.RegisterRuntimeFromAuthority(
                authorityCommit.Request,
                authorityCommit.Runtime);
        }

        registry.RecordRecoveryActivity(
            registered.RuntimeId,
            "register_runtime",
            "ok",
            "runtime registered with manual capability intake");
        return registered;
    }
}
