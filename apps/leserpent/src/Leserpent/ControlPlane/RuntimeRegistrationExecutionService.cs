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

    internal static RuntimeRegistrationExecutionException RecoveryPending(
        RuntimeRegistrationPlan plan,
        string message = "an earlier runtime registration has an uncertain outcome; retry its exact reviewed intent first") =>
        new(
            RuntimeRegistrationExecutionFailureKind.Conflict,
            "runtime_registration_recovery_pending",
            message,
            PlanRuntimeId(plan),
            plan);

    internal static RuntimeRegistrationExecutionException InProgress() =>
        new(
            RuntimeRegistrationExecutionFailureKind.Conflict,
            "runtime_registration_in_progress",
            "another runtime registration already owns this target; retry after it completes",
            null);

    internal static RuntimeRegistrationExecutionException Ambiguous(
        RuntimeRegistrationPlan plan,
        Exception innerException) =>
        new(
            RuntimeRegistrationExecutionFailureKind.Gateway,
            "runtime_registration_outcome_ambiguous",
            "runtime registration may have reached the daemon; retry the same reviewed intent",
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
    private readonly object activeRegistrationSync = new();
    private readonly List<RuntimeRegistrationPlanRequest>
        activeRegistrations = [];

    internal async Task<RuntimeRegistrationResponse> ExecuteAsync(
        RuntimeRegistrationRequest request,
        CancellationToken cancellationToken)
    {
        request = await NormalizeAndValidateAsync(request, cancellationToken);
        using var executionClaim = ClaimRuntimeRegistrationExecution(request);
        var coordinates = new RuntimeRegistrationPlanRequest(
            request.Name,
            request.Endpoint,
            request.SidecarEndpoint);
        var plan = await registrationPlans.BuildAsync(
            coordinates,
            cancellationToken);
        ValidateReviewedPlan(request, plan);
        request = request with
        {
            RegistrationPlanToken = plan.PlanToken,
        };
        ValidateAuthorityConfiguration(plan);
        var runtimeId = ResolveAuthorityRuntimeId(plan);
        using var lifecycleClaim = ClaimRuntimeRegistrationLifecycle(
            coordinates,
            plan,
            runtimeId ?? plan.ExistingRuntimeId);
        var pending = registry.ResolveRuntimeRegistrationIntent(request);

        try
        {
            if (pending.Kind ==
                RuntimeRegistrationIntentResolutionKind.Conflict)
            {
                throw RuntimeRegistrationExecutionException.RecoveryPending(
                    plan);
            }
            if (pending.Kind ==
                RuntimeRegistrationIntentResolutionKind.Exact)
            {
                if (!string.Equals(
                        plan.Reason,
                        RuntimeRegistrationPolicy
                            .RuntimeRegistrationRecoveryPendingReason,
                        StringComparison.Ordinal) ||
                    runtimeId is null ||
                    pending.Intent is null)
                {
                    throw RuntimeRegistrationExecutionException.RecoveryPending(
                        plan,
                        "runtime registration recovery state changed; review the recovery plan before retrying");
                }
                return await ExecuteAuthorityIntentAsync(
                    request,
                    plan,
                    pending.Intent,
                    cancellationToken);
            }
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
        catch (RuntimeRegistrationIntentConflictException)
        {
            throw RuntimeRegistrationExecutionException.RecoveryPending(
                plan);
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
            SidecarEndpoint = NormalizeOptional(request.SidecarEndpoint),
            Tags = NormalizeTags(request.Tags),
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
            if (plan.Reason == RuntimeRegistrationPolicy
                .RuntimeRegistrationRecoveryPendingReason)
            {
                throw RuntimeRegistrationExecutionException.RecoveryPending(
                    plan);
            }
            var message = plan.Reason == RuntimeRegistrationPolicy
                .RuntimeDeletionInProgressReason
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
            return await PrepareAndExecuteAuthorityIntentAsync(
                request,
                plan,
                runtimeId,
                capabilityDiscovery,
                statusDiscovery,
                sidecarDiscovery,
                cancellationToken);
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
            return await PrepareAndExecuteAuthorityIntentAsync(
                request,
                plan,
                runtimeId,
                null,
                null,
                null,
                cancellationToken);
        }

        registry.RecordRecoveryActivity(
            registered.RuntimeId,
            "register_runtime",
            "ok",
            "runtime registered with manual capability intake");
        return registered;
    }

    private async Task<RuntimeRegistrationResponse>
        PrepareAndExecuteAuthorityIntentAsync(
            RuntimeRegistrationRequest request,
            RuntimeRegistrationPlan plan,
            string runtimeId,
            CapabilityDiscoveryResult? capabilityDiscovery,
            RuntimeStatusDiscoveryResult? statusDiscovery,
            RuntimeSidecarDiscoveryResult? sidecarDiscovery,
            CancellationToken cancellationToken)
    {
        var proposed = RuntimeRegistrationIntentPolicy.Build(
            request,
            plan,
            runtimeId,
            capabilityDiscovery,
            statusDiscovery,
            sidecarDiscovery,
            DateTimeOffset.UtcNow);
        var prepared = registry.PrepareRuntimeRegistrationIntent(proposed);
        return await ExecuteAuthorityIntentAsync(
            request,
            plan,
            prepared,
            cancellationToken);
    }

    private async Task<RuntimeRegistrationResponse> ExecuteAuthorityIntentAsync(
        RuntimeRegistrationRequest credentialSource,
        RuntimeRegistrationPlan plan,
        PersistedRuntimeRegistrationIntent intent,
        CancellationToken cancellationToken)
    {
        var request = RuntimeRegistrationIntentPolicy.RestoreRequest(
            intent,
            credentialSource,
            plan.PlanToken);
        var receipt = await CommitRegistrationIntentAsync(
            request,
            plan,
            intent,
            cancellationToken);
        var authorityCommit = registrationCommits.Bind(
            intent.RuntimeId,
            request,
            receipt,
            intent.CapabilityDiscovery,
            intent.StatusDiscovery,
            intent.SidecarDiscovery);
        var registered = registry.RegisterRuntimeFromAuthority(
            authorityCommit.Request,
            authorityCommit.Runtime,
            authorityCommit.CapabilityDiscovery);
        registry.CompleteRuntimeRegistrationIntent(intent.CommandId);
        registry.RecordRecoveryActivity(
            registered.RuntimeId,
            "register_runtime",
            RuntimeRefreshOutcomePolicy.Determine(
                registered.Status.StatusSource,
                registered.Status.StatusFetchError,
                registered.SidecarStatus?.StatusSource,
                registered.SidecarStatus?.StatusFetchError),
            intent.FetchCapabilities
                ? "runtime registered through discovery"
                : "runtime registered with manual capability intake");
        return registered;
    }

    private async Task<RuntimeRegistrationCommitReceipt>
        CommitRegistrationIntentAsync(
            RuntimeRegistrationRequest request,
            RuntimeRegistrationPlan plan,
            PersistedRuntimeRegistrationIntent intent,
            CancellationToken cancellationToken)
    {
        for (var attempt = 0; attempt < 2; attempt++)
        {
            _ = registry.BeginRuntimeRegistrationAttempt(intent.CommandId);
            try
            {
                return await registrationAuthority.RegisterWithReceiptAsync(
                    request,
                    intent.RuntimeId,
                    cancellationToken,
                    update: intent.Action ==
                        RuntimeRegistrationPolicy.UpdateAction,
                    capabilityDiscovery: intent.CapabilityDiscovery,
                    statusDiscovery: intent.StatusDiscovery,
                    sidecarDiscovery: intent.SidecarDiscovery,
                    expectedRevision: intent.ExpectedRevision);
            }
            catch (DaemonRuntimeRegistrationException error)
                when (IsAmbiguousAuthorityFailure(error.Code))
            {
                registry.RecordRuntimeRegistrationFailure(
                    intent.CommandId,
                    error.Code);
                if (attempt == 0)
                {
                    continue;
                }
                throw RuntimeRegistrationExecutionException.Ambiguous(
                    plan,
                    error);
            }
            catch (DaemonRuntimeRegistrationException)
            {
                registry.CompleteRuntimeRegistrationIntent(intent.CommandId);
                throw;
            }
        }
        throw new InvalidOperationException(
            "runtime registration retry loop did not return");
    }

    private static bool IsAmbiguousAuthorityFailure(string code) =>
        code is "daemon_transport_failed"
            or "daemon_registration_timeout"
            or "daemon_protocol_invalid"
            or "daemon_protocol_mismatch";

    private static RuntimeTags NormalizeTags(RuntimeTags? tags) =>
        new(
            NormalizeOptional(tags?.Environment),
            NormalizeOptional(tags?.Cluster),
            NormalizeOptional(tags?.Role));

    private static string? NormalizeOptional(string? value) =>
        string.IsNullOrWhiteSpace(value) ? null : value.Trim();

    private IDisposable ClaimRuntimeRegistrationExecution(
        RuntimeRegistrationRequest request)
    {
        var coordinates = new RuntimeRegistrationPlanRequest(
            request.Name,
            request.Endpoint,
            request.SidecarEndpoint);
        lock (activeRegistrationSync)
        {
            if (activeRegistrations.Count >=
                    ControlPlaneStateValidator
                        .MaxPendingRuntimeRegistrationIntents ||
                activeRegistrations.Any(existing =>
                    RuntimeRegistrationIntentPolicy.Overlaps(
                        existing,
                        coordinates)))
            {
                throw RuntimeRegistrationExecutionException.InProgress();
            }
            activeRegistrations.Add(coordinates);
        }
        return new RuntimeRegistrationExecutionClaim(
            () => ReleaseRuntimeRegistrationExecution(coordinates));
    }

    private IDisposable? ClaimRuntimeRegistrationLifecycle(
        RuntimeRegistrationPlanRequest coordinates,
        RuntimeRegistrationPlan plan,
        string? runtimeId)
    {
        if (runtimeId is null)
        {
            return null;
        }
        try
        {
            return registry.ClaimRuntimeRegistrationLifecycle(runtimeId);
        }
        catch (RuntimeRegistrationInProgressException)
        {
            throw RuntimeRegistrationExecutionException.InProgress();
        }
        catch (RuntimeDeletionInProgressException)
        {
            throw RuntimeRegistrationExecutionException.PlanChanged(
                RuntimeRegistrationPolicy.Reject(
                    coordinates,
                    plan,
                    RuntimeRegistrationPolicy
                        .RuntimeDeletionInProgressReason),
                "runtime deletion started after plan review; review the current target before retrying");
        }
    }

    private void ReleaseRuntimeRegistrationExecution(
        RuntimeRegistrationPlanRequest coordinates)
    {
        lock (activeRegistrationSync)
        {
            _ = activeRegistrations.Remove(coordinates);
        }
    }

    private sealed class RuntimeRegistrationExecutionClaim(
        Action release) : IDisposable
    {
        private Action? releaseAction = release;

        public void Dispose() =>
            Interlocked.Exchange(ref releaseAction, null)?.Invoke();
    }
}
