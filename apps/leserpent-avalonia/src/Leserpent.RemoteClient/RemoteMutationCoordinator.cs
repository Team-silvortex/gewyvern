public enum RemoteMutationKind
{
    Refresh,
    CapabilityRefresh,
    Deployment,
}

public enum RemoteMutationAdmissionFailure
{
    None,
    InvalidRuntimeId,
    InFlight,
    RevisionFencePending,
    ObservationFencePending,
    AuthoritativeSnapshotRequired,
    RuntimeUnavailable,
    RuntimeRevisionChanged,
    AuthenticatedDeploymentRequired,
    OperationInactive,
}

public sealed record RemoteMutationRequest(
    string RuntimeId,
    ulong Revision,
    RemoteMutationKind Kind);

public sealed record RemoteMutationOperation
{
    internal RemoteMutationOperation(ulong sequence, RemoteMutationRequest request)
    {
        Sequence = sequence;
        Request = request;
    }

    internal ulong Sequence { get; }
    public RemoteMutationRequest Request { get; }
}

public sealed record RemoteMutationAdmission(
    RemoteMutationOperation? Operation,
    RemoteMutationAdmissionFailure Failure)
{
    public bool Accepted => Operation is not null
        && Failure == RemoteMutationAdmissionFailure.None;
}

public sealed class RemoteMutationCoordinator
{
    private sealed record ActiveMutation(
        RemoteMutationOperation Operation,
        ulong? SnapshotGeneration);

    private ulong nextSequence;
    private ActiveMutation? active;

    public bool IsInFlight => active is not null;
    public RemoteMutationRevisionFence? RevisionFence { get; private set; }
    public RemoteMutationObservationFence? ObservationFence { get; private set; }

    public RemoteMutationAvailability Availability(RemoteFeedState state) =>
        RemoteMutationAvailabilityPolicy.Evaluate(
            state,
            IsInFlight,
            RevisionFence,
            ObservationFence);

    public RemoteMutationAdmission Begin(
        RemoteMutationRequest request,
        RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(state);
        if (!Enum.IsDefined(request.Kind))
        {
            throw new ArgumentOutOfRangeException(nameof(request));
        }
        Observe(state);
        if (active is not null)
        {
            return Reject(RemoteMutationAdmissionFailure.InFlight);
        }
        if (RevisionFence is not null)
        {
            return Reject(RemoteMutationAdmissionFailure.RevisionFencePending);
        }
        if (ObservationFence is not null)
        {
            return Reject(RemoteMutationAdmissionFailure.ObservationFencePending);
        }
        var failure = Validate(request, state);
        if (failure != RemoteMutationAdmissionFailure.None)
        {
            return Reject(failure);
        }
        nextSequence = checked(nextSequence + 1);
        var operation = new RemoteMutationOperation(nextSequence, request);
        active = new ActiveMutation(operation, null);
        return new RemoteMutationAdmission(
            operation,
            RemoteMutationAdmissionFailure.None);
    }

    public RemoteMutationAdmission Confirm(
        RemoteMutationOperation operation,
        RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(operation);
        ArgumentNullException.ThrowIfNull(state);
        if (!IsActive(operation))
        {
            return Reject(RemoteMutationAdmissionFailure.OperationInactive);
        }
        var failure = Validate(operation.Request, state);
        if (failure != RemoteMutationAdmissionFailure.None)
        {
            active = null;
            return Reject(failure);
        }
        active = active! with { SnapshotGeneration = state.SnapshotGeneration };
        return new RemoteMutationAdmission(
            operation,
            RemoteMutationAdmissionFailure.None);
    }

    public void Accept(
        RemoteMutationOperation operation,
        RemoteMutationResult result,
        RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(result);
        ArgumentNullException.ThrowIfNull(state);
        var current = RequireConfirmed(operation);
        if (result.RuntimeId != current.Operation.Request.RuntimeId
            || result.Revision <= current.Operation.Request.Revision
            || result.Status != "applied")
        {
            throw new InvalidDataException(
                "mutation coordinator rejected a mismatched result");
        }
        RevisionFence = new RemoteMutationRevisionFence(
            result.RuntimeId,
            result.Revision,
            current.Operation.Request.Kind == RemoteMutationKind.CapabilityRefresh);
        active = null;
        Observe(state);
    }

    public void RejectKnown(RemoteMutationOperation operation)
    {
        RequireActive(operation);
        active = null;
    }

    public void MarkUnknown(
        RemoteMutationOperation operation,
        RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        var current = RequireConfirmed(operation);
        ObservationFence = new RemoteMutationObservationFence(
            current.Operation.Request.RuntimeId,
            current.Operation.Request.Revision,
            current.SnapshotGeneration!.Value,
            current.Operation.Request.Kind == RemoteMutationKind.CapabilityRefresh);
        active = null;
        Observe(state);
    }

    public void Cancel(RemoteMutationOperation operation)
    {
        RequireActive(operation);
        active = null;
    }

    public RemoteMutationFailure CompleteFailure(
        RemoteMutationOperation operation,
        Exception error,
        RemoteFeedState state,
        bool ownerCancellationRequested)
    {
        ArgumentNullException.ThrowIfNull(operation);
        ArgumentNullException.ThrowIfNull(error);
        ArgumentNullException.ThrowIfNull(state);
        if (!IsActive(operation))
        {
            return RemoteMutationFailurePolicy.StaleOperation();
        }
        _ = RequireConfirmed(operation);
        var failure = RemoteMutationFailurePolicy.Classify(
            operation.Request.Kind,
            error,
            ownerCancellationRequested);
        switch (failure.Disposition)
        {
            case RemoteMutationFailureDisposition.KnownRejection:
                RejectKnown(operation);
                break;
            case RemoteMutationFailureDisposition.UnknownOutcome:
                MarkUnknown(operation, state);
                break;
            case RemoteMutationFailureDisposition.Cancelled:
                Cancel(operation);
                break;
            default:
                throw new InvalidDataException(
                    "active mutation failure produced an invalid disposition");
        }
        return failure;
    }

    public void Abandon(
        RemoteMutationOperation operation,
        RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(operation);
        ArgumentNullException.ThrowIfNull(state);
        if (!IsActive(operation))
        {
            return;
        }
        if (active!.SnapshotGeneration is null)
        {
            active = null;
            return;
        }
        MarkUnknown(operation, state);
    }

    public void AbandonActive(RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        if (active is not { } current)
        {
            return;
        }
        Abandon(current.Operation, state);
    }

    public void CancelActive() => active = null;

    public bool Observe(RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(state);
        var changed = false;
        if (RevisionFence is { } revisionFence
            && state.Runtimes.Any(runtime => RemoteMutationFences.SatisfiesRevision(
                runtime,
                revisionFence)))
        {
            RevisionFence = null;
            changed = true;
        }
        if (ObservationFence is { } observationFence
            && RemoteMutationFences.SatisfiesObservation(state, observationFence))
        {
            ObservationFence = null;
            changed = true;
        }
        return changed;
    }

    public static string DescribeFailure(RemoteMutationAdmissionFailure failure) => failure switch
    {
        RemoteMutationAdmissionFailure.InvalidRuntimeId =>
            "the runtime ID is invalid",
        RemoteMutationAdmissionFailure.InFlight =>
            "another remote change is awaiting confirmation or completion",
        RemoteMutationAdmissionFailure.RevisionFencePending =>
            "a prior remote change is awaiting its event revision",
        RemoteMutationAdmissionFailure.ObservationFencePending =>
            "an unknown remote outcome is awaiting an authoritative snapshot",
        RemoteMutationAdmissionFailure.AuthoritativeSnapshotRequired =>
            "a generated authoritative snapshot is required",
        RemoteMutationAdmissionFailure.RuntimeUnavailable =>
            "the runtime is absent from the authoritative snapshot",
        RemoteMutationAdmissionFailure.RuntimeRevisionChanged =>
            "the runtime revision changed",
        RemoteMutationAdmissionFailure.AuthenticatedDeploymentRequired =>
            "the runtime has not advertised authenticated deployment",
        RemoteMutationAdmissionFailure.OperationInactive =>
            "the mutation operation is no longer active",
        RemoteMutationAdmissionFailure.None =>
            "the mutation operation is admitted",
        _ => throw new ArgumentOutOfRangeException(nameof(failure)),
    };

    public static void VerifyContract()
    {
        RemoteMutationFailurePolicy.VerifyContract();
        RemoteFeedAuthorityPolicy.VerifyContract();
        RemoteMutationAvailabilityPolicy.VerifyContract();
        var runtime = Runtime("runtime-a", 7);
        var authoritative = State(runtime, 7, 4, 7);
        var heartbeatOnly = authoritative with
        {
            Revision = 8,
            SnapshotGeneration = 0,
            SnapshotRevision = null,
        };

        var coordinator = new RemoteMutationCoordinator();
        RequireFailure(
            coordinator.Begin(
                new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
                heartbeatOnly),
            RemoteMutationAdmissionFailure.AuthoritativeSnapshotRequired,
            "heartbeat-only state admitted a mutation");
        if (coordinator.Availability(heartbeatOnly).MutationsEnabled
            || coordinator.Availability(heartbeatOnly).InspectEnabled)
        {
            throw new InvalidDataException(
                "heartbeat-only state exposed authoritative actions");
        }

        var first = RequireAccepted(coordinator.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
            authoritative));
        RequireFailure(
            coordinator.Begin(
                new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
                authoritative),
            RemoteMutationAdmissionFailure.InFlight,
            "mutation coordinator admitted concurrent work");
        var changedRuntime = Runtime("runtime-a", 8);
        RequireFailure(
            coordinator.Confirm(first, State(changedRuntime, 8, 5, 8)),
            RemoteMutationAdmissionFailure.RuntimeRevisionChanged,
            "confirmation accepted a changed runtime revision");
        if (coordinator.IsInFlight)
        {
            throw new InvalidDataException(
                "failed confirmation retained active mutation ownership");
        }

        var tokenFence = new RemoteMutationCoordinator();
        var retired = RequireAccepted(tokenFence.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
            authoritative));
        tokenFence.Cancel(retired);
        var current = RequireAccepted(tokenFence.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
            authoritative));
        RequireFailure(
            tokenFence.Confirm(retired, authoritative),
            RemoteMutationAdmissionFailure.OperationInactive,
            "retired mutation token was confirmed");
        if (!tokenFence.IsInFlight)
        {
            throw new InvalidDataException(
                "retired mutation token cleared current operation ownership");
        }
        tokenFence.Cancel(current);

        var unknown = RequireAccepted(coordinator.Begin(
            new RemoteMutationRequest(
                "runtime-a",
                7,
                RemoteMutationKind.CapabilityRefresh),
            authoritative));
        _ = RequireAccepted(coordinator.Confirm(unknown, authoritative));
        coordinator.MarkUnknown(unknown, authoritative);
        if (coordinator.ObservationFence is not
            {
                RuntimeId: "runtime-a",
                Revision: 7,
                SnapshotGeneration: 4,
                RequiresCapabilityChange: true,
            })
        {
            throw new InvalidDataException(
                "unknown capability mutation did not retain its snapshot fence");
        }
        coordinator.Observe(authoritative with { Revision = 8 });
        if (coordinator.ObservationFence is null)
        {
            throw new InvalidDataException(
                "heartbeat released an unknown mutation outcome");
        }
        coordinator.Observe(State(runtime, 8, 5, 8));
        if (coordinator.ObservationFence is not null)
        {
            throw new InvalidDataException(
                "authoritative unchanged snapshot did not release unknown outcome");
        }

        var capability = RequireAccepted(coordinator.Begin(
            new RemoteMutationRequest(
                "runtime-a",
                7,
                RemoteMutationKind.CapabilityRefresh),
            authoritative));
        _ = RequireAccepted(coordinator.Confirm(capability, authoritative));
        coordinator.Accept(
            capability,
            new RemoteMutationResult("command-a", "runtime-a", 8, "applied"),
            authoritative);
        var pendingCapability = Runtime("runtime-a", 8);
        coordinator.Observe(State(pendingCapability, 8, 5, 8));
        if (coordinator.RevisionFence is null)
        {
            throw new InvalidDataException(
                "capability mutation fence accepted an unobserved projection");
        }
        pendingCapability.Capabilities = new RuntimeCapabilitySnapshot
        {
            Source = "gewyvern-api",
        };
        pendingCapability.CapabilitiesObservedForRevision = 8;
        coordinator.Observe(State(pendingCapability, 8, 5, 8));
        if (coordinator.RevisionFence is not null)
        {
            throw new InvalidDataException(
                "capability mutation fence rejected its observed projection");
        }

        var malformed = RequireAccepted(coordinator.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
            authoritative));
        _ = RequireAccepted(coordinator.Confirm(malformed, authoritative));
        try
        {
            coordinator.Accept(
                malformed,
                new RemoteMutationResult("command-b", "runtime-b", 8, "applied"),
                authoritative);
            throw new InvalidDataException(
                "mutation coordinator accepted a mismatched result");
        }
        catch (InvalidDataException error) when (
            error.Message == "mutation coordinator rejected a mismatched result")
        {
        }
        coordinator.MarkUnknown(malformed, authoritative);
        if (coordinator.ObservationFence is null)
        {
            throw new InvalidDataException(
                "malformed mutation response did not become an unknown outcome");
        }
        coordinator.Observe(State(runtime, 8, 5, 8));

        RequireFailure(
            new RemoteMutationCoordinator().Begin(
                new RemoteMutationRequest(
                    "runtime-a",
                    7,
                    RemoteMutationKind.Deployment),
                authoritative),
            RemoteMutationAdmissionFailure.AuthenticatedDeploymentRequired,
            "deployment admission ignored runtime capability");
        var deployable = Runtime("runtime-a", 7, authenticatedDeployment: true);
        var abandon = new RemoteMutationCoordinator();
        var unconfirmed = RequireAccepted(abandon.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Deployment),
            State(deployable, 7, 4, 7)));
        abandon.Abandon(unconfirmed, State(deployable, 7, 4, 7));
        if (abandon.IsInFlight || abandon.ObservationFence is not null)
        {
            throw new InvalidDataException(
                "unconfirmed mutation abandonment fabricated an unknown outcome");
        }
        var confirmed = RequireAccepted(abandon.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Deployment),
            State(deployable, 7, 4, 7)));
        _ = RequireAccepted(abandon.Confirm(confirmed, State(deployable, 7, 4, 7)));
        abandon.Abandon(confirmed, State(deployable, 7, 4, 7));
        if (abandon.ObservationFence is null)
        {
            throw new InvalidDataException(
                "confirmed mutation abandonment lost its unknown outcome fence");
        }

        var failureCoordinator = new RemoteMutationCoordinator();
        var failed = RequireAccepted(failureCoordinator.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
            authoritative));
        _ = RequireAccepted(failureCoordinator.Confirm(failed, authoritative));
        var failure = failureCoordinator.CompleteFailure(
            failed,
            new HttpRequestException("endpoint detail must not escape"),
            authoritative,
            ownerCancellationRequested: false);
        if (failure.Disposition != RemoteMutationFailureDisposition.UnknownOutcome
            || failureCoordinator.IsInFlight
            || failureCoordinator.ObservationFence is null)
        {
            throw new InvalidDataException(
                "transport failure did not atomically install its observation fence");
        }
        failureCoordinator.Observe(State(runtime, 8, 5, 8));

        var lateRetired = RequireAccepted(failureCoordinator.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
            authoritative));
        _ = RequireAccepted(failureCoordinator.Confirm(lateRetired, authoritative));
        failureCoordinator.Cancel(lateRetired);
        var lateCurrent = RequireAccepted(failureCoordinator.Begin(
            new RemoteMutationRequest("runtime-a", 7, RemoteMutationKind.Refresh),
            authoritative));
        _ = RequireAccepted(failureCoordinator.Confirm(lateCurrent, authoritative));
        var staleFailure = failureCoordinator.CompleteFailure(
            lateRetired,
            new InvalidDataException("late response"),
            authoritative,
            ownerCancellationRequested: false);
        if (staleFailure.Disposition != RemoteMutationFailureDisposition.Ignored
            || !failureCoordinator.IsInFlight
            || failureCoordinator.ObservationFence is not null)
        {
            throw new InvalidDataException(
                "retired mutation failure disturbed current operation ownership");
        }
        failureCoordinator.Cancel(lateCurrent);
    }

    private static RemoteMutationAdmissionFailure Validate(
        RemoteMutationRequest request,
        RemoteFeedState state)
    {
        if (!RemoteWorkspaceLaunchPolicy.IsRuntimeId(request.RuntimeId))
        {
            return RemoteMutationAdmissionFailure.InvalidRuntimeId;
        }
        if (!RemoteFeedAuthorityPolicy.HasAuthoritativeSnapshot(state))
        {
            return RemoteMutationAdmissionFailure.AuthoritativeSnapshotRequired;
        }
        var runtime = state.Runtimes.FirstOrDefault(candidate =>
            candidate.Id == request.RuntimeId);
        if (runtime is null)
        {
            return RemoteMutationAdmissionFailure.RuntimeUnavailable;
        }
        if (runtime.Revision != request.Revision)
        {
            return RemoteMutationAdmissionFailure.RuntimeRevisionChanged;
        }
        if (request.Kind == RemoteMutationKind.Deployment
            && runtime.Capabilities is not { AuthenticatedDeployment: true })
        {
            return RemoteMutationAdmissionFailure.AuthenticatedDeploymentRequired;
        }
        return RemoteMutationAdmissionFailure.None;
    }

    private bool IsActive(RemoteMutationOperation operation) => active is { } current
        && current.Operation.Sequence == operation.Sequence
        && ReferenceEquals(current.Operation, operation);

    private ActiveMutation RequireActive(RemoteMutationOperation operation)
    {
        ArgumentNullException.ThrowIfNull(operation);
        if (!IsActive(operation))
        {
            throw new InvalidOperationException(
                "mutation operation is no longer active");
        }
        return active!;
    }

    private ActiveMutation RequireConfirmed(RemoteMutationOperation operation)
    {
        var current = RequireActive(operation);
        if (current.SnapshotGeneration is null)
        {
            throw new InvalidOperationException(
                "mutation operation has not been confirmed");
        }
        return current;
    }

    private static RemoteMutationAdmission Reject(RemoteMutationAdmissionFailure failure) =>
        new(null, failure);

    private static RemoteMutationOperation RequireAccepted(RemoteMutationAdmission admission)
    {
        if (!admission.Accepted || admission.Operation is null)
        {
            throw new InvalidDataException(
                $"mutation coordinator rejected contract fixture: {admission.Failure}");
        }
        return admission.Operation;
    }

    private static void RequireFailure(
        RemoteMutationAdmission admission,
        RemoteMutationAdmissionFailure expected,
        string message)
    {
        if (admission.Accepted || admission.Failure != expected)
        {
            throw new InvalidDataException(message);
        }
    }

    private static RemoteRuntimeProjection Runtime(
        string id,
        ulong revision,
        bool authenticatedDeployment = false) => new()
    {
        Id = id,
        Name = id,
        Revision = revision,
        Tags = new RuntimeTags(),
        Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
        Capabilities = authenticatedDeployment
            ? new RuntimeCapabilitySnapshot
            {
                Source = "gewyvern-api",
                AuthenticatedDeployment = true,
            }
            : null,
    };

    private static RemoteFeedState State(
        RemoteRuntimeProjection runtime,
        ulong revision,
        ulong snapshotGeneration,
        ulong snapshotRevision) => new(
        RemoteFeedPhase.Live,
        revision,
        [runtime],
        0,
        false,
        "authoritative",
        snapshotGeneration,
        snapshotRevision);
}
