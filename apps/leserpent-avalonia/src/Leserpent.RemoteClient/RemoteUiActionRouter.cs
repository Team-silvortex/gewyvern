using System.Text;

public enum RemoteUiActionFailure
{
    None,
    InvalidDocument,
    InvalidNodeId,
    UnknownTarget,
    InvalidActionBinding,
    RuntimeUnavailable,
    DeploymentCapabilityRequired,
    ActionUnavailable,
    UnsupportedAction,
    InvalidEvent,
    InvalidFormValues,
}

public sealed record RemoteUiActionIntent(
    ActionKind Kind,
    string NodeId,
    RemoteRuntimeProjection Runtime,
    UiForm? Form = null,
    string? PipelineKind = null,
    string? Target = null);

public sealed record RemoteDebuggerActionIntent(
    ActionKind Kind,
    string NodeId,
    string SessionId,
    ulong Revision);

public sealed record RemoteDebuggerActionResolution(
    RemoteDebuggerActionIntent? Intent,
    RemoteUiActionFailure Failure,
    string? Reason)
{
    public bool Accepted => Intent is not null
        && Failure == RemoteUiActionFailure.None;
}

public sealed record RemoteUiActionResolution(
    RemoteUiActionIntent? Intent,
    RemoteUiActionFailure Failure,
    string? Reason)
{
    public bool Accepted => Intent is not null
        && Failure == RemoteUiActionFailure.None;
}

public static class RemoteUiActionRouter
{
    public const int MaxOperatorReasonLength = 320;
    private const int MaxNodes = 4096;
    private const int MaxDepth = 32;
    private const int MaxFormValueBytes = 4096;

    public static RemoteUiActionResolution ResolveActivation(
        UiDocument document,
        string nodeId,
        RemoteFeedState state,
        RemoteMutationAvailability availability)
    {
        ArgumentNullException.ThrowIfNull(document);
        ArgumentNullException.ThrowIfNull(state);
        ArgumentNullException.ThrowIfNull(availability);
        var resolved = ResolveTarget(document, nodeId, state);
        if (resolved is not { Accepted: true, Intent: { } intent })
        {
            return resolved;
        }

        if (intent.Kind == ActionKind.RuntimeInspect)
        {
            return availability.InspectEnabled
                ? Accept(intent)
                : Reject(
                    RemoteUiActionFailure.ActionUnavailable,
                    availability.InspectUnavailableReason);
        }
        return availability.MutationsEnabled
            ? Accept(intent)
            : Reject(
                RemoteUiActionFailure.ActionUnavailable,
                availability.MutationUnavailableReason);
    }

    public static RemoteUiActionResolution ResolveSubmission(
        UiDocument document,
        UiEvent submission,
        RemoteFeedState state,
        string expectedNodeId)
    {
        ArgumentNullException.ThrowIfNull(document);
        ArgumentNullException.ThrowIfNull(submission);
        ArgumentNullException.ThrowIfNull(state);
        if (!IsIdentifier(expectedNodeId)
            || !StringComparer.Ordinal.Equals(submission.NodeId, expectedNodeId)
            || submission.Kind != UiEventKind.Submit
            || submission.Values is null)
        {
            return Reject(RemoteUiActionFailure.InvalidEvent);
        }
        var resolved = ResolveTarget(document, submission.NodeId, state);
        if (!resolved.Accepted || resolved.Intent is not { } intent)
        {
            return resolved;
        }
        if (intent.Kind != ActionKind.RuntimeDeploy || intent.Form is not { } form)
        {
            return Reject(RemoteUiActionFailure.InvalidEvent);
        }
        if (!ValidFormValues(form, submission.Values)
            || !submission.Values.TryGetValue("pipeline_kind", out var pipelineKind)
            || pipelineKind.Length == 0)
        {
            return Reject(RemoteUiActionFailure.InvalidFormValues);
        }
        submission.Values.TryGetValue("target", out var target);
        return Accept(intent with
        {
            PipelineKind = pipelineKind,
            Target = string.IsNullOrEmpty(target) ? null : target,
        });
    }

    public static RemoteDebuggerActionResolution ResolveDebuggerActivation(
        UiDocument document,
        string nodeId,
        string expectedSessionId,
        ulong expectedRevision,
        bool mutationEnabled,
        string? mutationUnavailableReason = null)
    {
        ArgumentNullException.ThrowIfNull(document);
        if (!IsIdentifier(nodeId)
            || !IsIdentifier(expectedSessionId)
            || expectedRevision == 0
            || document.Revision != expectedRevision)
        {
            return RejectDebugger(RemoteUiActionFailure.InvalidNodeId);
        }
        NodeLocation? location;
        try
        {
            location = Find(document.Root, nodeId);
        }
        catch (InvalidDataException)
        {
            return RejectDebugger(RemoteUiActionFailure.InvalidDocument);
        }
        if (location is null)
        {
            return RejectDebugger(RemoteUiActionFailure.UnknownTarget);
        }
        if (location.Node is not
            {
                Kind: UiNodeKind.Action,
                Action:
                {
                    Kind: ActionKind.DebuggerCancel,
                    RuntimeId: null,
                    Form: null,
                } action,
            }
            || action.SessionId != expectedSessionId
            || location.DebuggerSessionId != expectedSessionId
            || location.RuntimeId is not null)
        {
            return RejectDebugger(RemoteUiActionFailure.InvalidActionBinding);
        }
        if (!mutationEnabled)
        {
            return RejectDebugger(
                RemoteUiActionFailure.ActionUnavailable,
                mutationUnavailableReason);
        }
        return new RemoteDebuggerActionResolution(
            new RemoteDebuggerActionIntent(
                action.Kind,
                nodeId,
                expectedSessionId,
                expectedRevision),
            RemoteUiActionFailure.None,
            null);
    }

    public static void VerifyContract()
    {
        var runtime = new RemoteRuntimeProjection
        {
            Id = "runtime-a",
            Name = "Payments",
            Revision = 7,
            Tags = new RuntimeTags(),
            Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
            Capabilities = new RuntimeCapabilitySnapshot
            {
                Source = "gewyvern-api",
                AuthenticatedDeployment = true,
            },
        };
        var state = new RemoteFeedState(
            RemoteFeedPhase.Live,
            7,
            [runtime],
            0,
            false,
            "authoritative",
            3,
            7);
        var availability = RemoteMutationAvailabilityPolicy.Evaluate(
            state,
            false,
            null,
            null);
        var fleet = RemoteDocumentProjection.Project(state).Document;
        var refresh = FindAction(fleet.Root, ActionKind.RuntimeRefresh);
        refresh.Id = "opaque-action-control";
        RequireAccepted(
            ResolveActivation(fleet, "opaque-action-control", state, availability),
            ActionKind.RuntimeRefresh,
            "opaque action ID did not route by its typed binding");
        RequireAccepted(
            ResolveActivation(
                fleet,
                "runtime:runtime-a:inspect",
                state,
                availability),
            ActionKind.RuntimeInspect,
            "inspect action did not route by its typed binding");
        RequireAccepted(
            ResolveActivation(
                fleet,
                "runtime:runtime-a:capabilities-refresh",
                state,
                availability),
            ActionKind.RuntimeCapabilitiesRefresh,
            "capability action did not route by its typed binding");

        var workspace = DeploymentDocument();
        var semantic = new SemanticRenderer();
        semantic.Mount(workspace);
        var prepared = RequireAccepted(
            ResolveActivation(
                semantic.Document,
                "opaque-deployment-control",
                state,
                availability),
            ActionKind.RuntimeDeploy,
            "deployment action did not preserve its source document form");
        if (prepared.Form is null || prepared.PipelineKind is not null)
        {
            throw new InvalidDataException(
                "deployment preparation did not remain side-effect free");
        }
        var submission = semantic.CreateFormSubmission(
            "opaque-deployment-control",
            new Dictionary<string, string>(StringComparer.Ordinal)
            {
                ["pipeline_kind"] = "http/request",
                ["target"] = "pid:42",
            });
        var submitted = RequireAccepted(
            ResolveSubmission(
                semantic.Document,
                submission,
                state,
                "opaque-deployment-control"),
            ActionKind.RuntimeDeploy,
            "deployment submission did not route by its typed event");
        if (submitted.PipelineKind != "http/request" || submitted.Target != "pid:42")
        {
            throw new InvalidDataException(
                "deployment submission lost its validated typed values");
        }

        RequireFailure(
            ResolveActivation(fleet, "missing-action", state, availability),
            RemoteUiActionFailure.UnknownTarget,
            "unknown action target was accepted");
        var unavailable = RemoteMutationAvailabilityPolicy.Evaluate(
            state,
            true,
            null,
            null);
        RequireFailure(
            ResolveActivation(fleet, "opaque-action-control", state, unavailable),
            RemoteUiActionFailure.ActionUnavailable,
            "in-flight mutation did not disable another typed action");
        var forged = new UiEvent
        {
            NodeId = "opaque-deployment-control",
            Kind = UiEventKind.Submit,
            Values = new Dictionary<string, string>(StringComparer.Ordinal)
            {
                ["pipeline_kind"] = "unsafe value",
            },
        };
        RequireFailure(
            ResolveSubmission(
                semantic.Document,
                forged,
                state,
                "opaque-deployment-control"),
            RemoteUiActionFailure.InvalidFormValues,
            "forged deployment form values were accepted");
        RequireFailure(
            ResolveSubmission(
                semantic.Document,
                submission,
                state,
                "another-action"),
            RemoteUiActionFailure.InvalidEvent,
            "deployment submission crossed its source-node fence");
        var capability = FindAction(fleet.Root, ActionKind.RuntimeCapabilitiesRefresh);
        capability.Action!.RuntimeId = "runtime-b";
        RequireFailure(
            ResolveActivation(
                fleet,
                capability.Id,
                state,
                availability),
            RemoteUiActionFailure.InvalidActionBinding,
            "action escaped its runtime container binding");

        var debugger = DebuggerDocument();
        var cancel = ResolveDebuggerActivation(
            debugger,
            "opaque-debugger-control",
            "session-a",
            9,
            mutationEnabled: true);
        if (!cancel.Accepted
            || cancel.Intent is not
            {
                Kind: ActionKind.DebuggerCancel,
                SessionId: "session-a",
                Revision: 9,
            })
        {
            throw new InvalidDataException(
                "debugger cancellation did not route through its typed binding");
        }
        var unavailableDebugger = ResolveDebuggerActivation(
            debugger,
            "opaque-debugger-control",
            "session-a",
            9,
            mutationEnabled: false,
            "A debugger mutation is already in flight");
        if (unavailableDebugger.Accepted
            || unavailableDebugger.Failure != RemoteUiActionFailure.ActionUnavailable
            || unavailableDebugger.Reason is not { Length: > 0 })
        {
            throw new InvalidDataException(
                "debugger mutation availability did not fail closed");
        }
        debugger.Root.DebuggerSessionId = "session-b";
        if (ResolveDebuggerActivation(
                debugger,
                "opaque-debugger-control",
                "session-a",
                9,
                mutationEnabled: true).Failure
            != RemoteUiActionFailure.InvalidActionBinding)
        {
            throw new InvalidDataException(
                "debugger action escaped its session-container binding");
        }
    }

    private static RemoteUiActionResolution ResolveTarget(
        UiDocument document,
        string nodeId,
        RemoteFeedState state)
    {
        if (!IsIdentifier(nodeId))
        {
            return Reject(RemoteUiActionFailure.InvalidNodeId);
        }
        NodeLocation? location;
        try
        {
            location = Find(document.Root, nodeId);
        }
        catch (InvalidDataException)
        {
            return Reject(RemoteUiActionFailure.InvalidDocument);
        }
        if (location is null)
        {
            return Reject(RemoteUiActionFailure.UnknownTarget);
        }
        var node = location.Node;
        if (node.Kind != UiNodeKind.Action || node.Action is not { } action)
        {
            return Reject(RemoteUiActionFailure.InvalidActionBinding);
        }
        if (action.Kind == ActionKind.DebuggerCancel)
        {
            return Reject(RemoteUiActionFailure.UnsupportedAction);
        }
        if (!Enum.IsDefined(action.Kind)
            || !IsIdentifier(action.RuntimeId)
            || action.SessionId is not null
            || !StringComparer.Ordinal.Equals(action.RuntimeId, location.RuntimeId)
            || location.DebuggerSessionId is not null
            || (action.Kind == ActionKind.RuntimeDeploy) != (action.Form is not null))
        {
            return Reject(RemoteUiActionFailure.InvalidActionBinding);
        }
        RemoteRuntimeProjection? runtime = null;
        foreach (var candidate in state.Runtimes)
        {
            if (!StringComparer.Ordinal.Equals(candidate.Id, action.RuntimeId))
            {
                continue;
            }
            if (runtime is not null)
            {
                return Reject(RemoteUiActionFailure.InvalidDocument);
            }
            runtime = candidate;
        }
        if (runtime is null)
        {
            return Reject(RemoteUiActionFailure.RuntimeUnavailable);
        }
        if (action.Kind == ActionKind.RuntimeDeploy
            && runtime.Capabilities is not { AuthenticatedDeployment: true })
        {
            return Reject(RemoteUiActionFailure.DeploymentCapabilityRequired);
        }
        return Accept(new RemoteUiActionIntent(
            action.Kind,
            nodeId,
            runtime,
            action.Form));
    }

    private static NodeLocation? Find(UiNode root, string nodeId)
    {
        var stack = new Stack<(
            UiNode Node,
            string? RuntimeId,
            string? DebuggerSessionId,
            int Depth)>();
        stack.Push((root, null, null, 1));
        var visited = 0;
        while (stack.Count > 0)
        {
            var (node, inheritedRuntimeId, inheritedDebuggerSessionId, depth) = stack.Pop();
            if (checked(++visited) > MaxNodes || depth > MaxDepth)
            {
                throw new InvalidDataException("remote UI document bounds are invalid");
            }
            var runtimeId = inheritedRuntimeId;
            var debuggerSessionId = inheritedDebuggerSessionId;
            if (node.Kind is UiNodeKind.RuntimeCard or UiNodeKind.RuntimeWorkspace)
            {
                if (!IsIdentifier(node.RuntimeId) || debuggerSessionId is not null)
                {
                    throw new InvalidDataException("remote UI runtime binding is invalid");
                }
                runtimeId = node.RuntimeId;
            }
            else if (node.RuntimeId is not null)
            {
                throw new InvalidDataException("remote UI runtime context is invalid");
            }
            if (node.Kind == UiNodeKind.DebuggerWorkspace)
            {
                if (!IsIdentifier(node.DebuggerSessionId) || runtimeId is not null)
                {
                    throw new InvalidDataException("remote UI debugger binding is invalid");
                }
                debuggerSessionId = node.DebuggerSessionId;
            }
            else if (node.DebuggerSessionId is not null)
            {
                throw new InvalidDataException("remote UI debugger context is invalid");
            }
            if (StringComparer.Ordinal.Equals(node.Id, nodeId))
            {
                return new NodeLocation(node, runtimeId, debuggerSessionId);
            }
            for (var index = node.Children.Count - 1; index >= 0; index--)
            {
                stack.Push((
                    node.Children[index],
                    runtimeId,
                    debuggerSessionId,
                    checked(depth + 1)));
            }
        }
        return null;
    }

    private static bool ValidFormValues(
        UiForm form,
        IReadOnlyDictionary<string, string> values)
    {
        if (form.Fields.Count is < 1 or > 16
            || values.Keys.Any(key => form.Fields.All(field => field.Key != key)))
        {
            return false;
        }
        foreach (var field in form.Fields)
        {
            var value = values.TryGetValue(field.Key, out var provided)
                ? provided
                : string.Empty;
            if ((field.Required && value.Length == 0)
                || Encoding.UTF8.GetByteCount(value) > field.MaxLength
                || Encoding.UTF8.GetByteCount(value) > MaxFormValueBytes)
            {
                return false;
            }
            var valid = field.InputKind switch
            {
                UiFormInputKind.PathToken => value.Length > 0
                    && value.All(character => char.IsAsciiLetterOrDigit(character)
                        || character is '.' or '/' or '_' or '-'),
                UiFormInputKind.TrimmedText => value == value.Trim()
                    && !value.Any(char.IsControl),
                _ => false,
            };
            if (!valid)
            {
                return false;
            }
        }
        return true;
    }

    private static UiDocument DeploymentDocument() => new()
    {
        SchemaVersion = 1,
        Revision = 7,
        Root = new UiNode
        {
            Id = "opaque-runtime-workspace",
            Kind = UiNodeKind.RuntimeWorkspace,
            RuntimeId = "runtime-a",
            Accessibility = new Accessibility(),
            Children =
            [
                new UiNode
                {
                    Id = "opaque-deployment-control",
                    Kind = UiNodeKind.Action,
                    Accessibility = new Accessibility
                    {
                        Label = Text("runtime.deploy", "Deploy pipeline"),
                    },
                    Action = new UiAction
                    {
                        Kind = ActionKind.RuntimeDeploy,
                        RuntimeId = "runtime-a",
                        Form = new UiForm
                        {
                            Title = Text("runtime.deploy.form", "Deploy pipeline"),
                            SubmitLabel = Text("runtime.deploy.submit", "Deploy"),
                            Fields =
                            [
                                new UiFormField
                                {
                                    Key = "pipeline_kind",
                                    Label = Text("runtime.deploy.pipeline", "Pipeline kind"),
                                    Required = true,
                                    MaxLength = 128,
                                    InputKind = UiFormInputKind.PathToken,
                                },
                                new UiFormField
                                {
                                    Key = "target",
                                    Label = Text("runtime.deploy.target", "Target"),
                                    MaxLength = 256,
                                    InputKind = UiFormInputKind.TrimmedText,
                                },
                            ],
                        },
                    },
                    Children = [],
                },
            ],
        },
    };

    private static UiDocument DebuggerDocument() => new()
    {
        SchemaVersion = 1,
        Revision = 9,
        Root = new UiNode
        {
            Id = "opaque-debugger-workspace",
            Kind = UiNodeKind.DebuggerWorkspace,
            DebuggerSessionId = "session-a",
            Accessibility = new Accessibility(),
            Children =
            [
                new UiNode
                {
                    Id = "opaque-debugger-control",
                    Kind = UiNodeKind.Action,
                    Accessibility = new Accessibility
                    {
                        Label = Text("debugger.cancel", "Cancel effect"),
                    },
                    Action = new UiAction
                    {
                        Kind = ActionKind.DebuggerCancel,
                        SessionId = "session-a",
                    },
                    Children = [],
                },
            ],
        },
    };

    private static LocalizedText Text(string key, string fallback) => new()
    {
        Key = key,
        Fallback = fallback,
    };

    private static UiNode FindAction(UiNode node, ActionKind kind)
    {
        if (node.Action?.Kind == kind)
        {
            return node;
        }
        foreach (var child in node.Children)
        {
            if (FindActionOrNull(child, kind) is { } found)
            {
                return found;
            }
        }
        throw new InvalidDataException($"remote UI fixture omitted {kind}");
    }

    private static UiNode? FindActionOrNull(UiNode node, ActionKind kind)
    {
        if (node.Action?.Kind == kind)
        {
            return node;
        }
        foreach (var child in node.Children)
        {
            if (FindActionOrNull(child, kind) is { } found)
            {
                return found;
            }
        }
        return null;
    }

    private static RemoteUiActionIntent RequireAccepted(
        RemoteUiActionResolution resolution,
        ActionKind expected,
        string message)
    {
        if (!resolution.Accepted || resolution.Intent is not { } intent || intent.Kind != expected)
        {
            throw new InvalidDataException(message);
        }
        return intent;
    }

    private static void RequireFailure(
        RemoteUiActionResolution resolution,
        RemoteUiActionFailure expected,
        string message)
    {
        if (resolution.Accepted
            || resolution.Intent is not null
            || resolution.Failure != expected
            || resolution.Reason is not { Length: > 0 and <= MaxOperatorReasonLength }
            || resolution.Reason.Any(char.IsControl))
        {
            throw new InvalidDataException(message);
        }
    }

    private static RemoteUiActionResolution Accept(RemoteUiActionIntent intent) =>
        new(intent, RemoteUiActionFailure.None, null);

    private static RemoteUiActionResolution Reject(
        RemoteUiActionFailure failure,
        string? reason = null)
    {
        var fallback = failure switch
        {
            RemoteUiActionFailure.InvalidDocument => "The UI document is invalid",
            RemoteUiActionFailure.InvalidNodeId => "The action target is invalid",
            RemoteUiActionFailure.UnknownTarget => "The action target is no longer present",
            RemoteUiActionFailure.InvalidActionBinding => "The action binding is invalid",
            RemoteUiActionFailure.RuntimeUnavailable =>
                "The runtime is absent from the current projection",
            RemoteUiActionFailure.DeploymentCapabilityRequired =>
                "The runtime has not advertised authenticated deployment",
            RemoteUiActionFailure.ActionUnavailable => "The action is currently unavailable",
            RemoteUiActionFailure.UnsupportedAction =>
                "The action is not supported by the remote runtime console",
            RemoteUiActionFailure.InvalidEvent => "The action event is invalid",
            RemoteUiActionFailure.InvalidFormValues => "The deployment form values are invalid",
            _ => "The action was rejected",
        };
        var bounded = new string((reason ?? fallback)
            .Where(character => !char.IsControl(character))
            .Take(MaxOperatorReasonLength)
            .ToArray());
        if (string.IsNullOrWhiteSpace(bounded))
        {
            bounded = fallback;
        }
        return new RemoteUiActionResolution(null, failure, bounded);
    }

    private static RemoteDebuggerActionResolution RejectDebugger(
        RemoteUiActionFailure failure,
        string? reason = null)
    {
        var rejected = Reject(failure, reason);
        return new RemoteDebuggerActionResolution(
            null,
            rejected.Failure,
            rejected.Reason);
    }

    private static bool IsIdentifier(string? value) => value is not null
        && value.Length is > 0 and <= 128
        && value.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '-' or '_' or '.' or ':');

    private sealed record NodeLocation(
        UiNode Node,
        string? RuntimeId,
        string? DebuggerSessionId);
}
