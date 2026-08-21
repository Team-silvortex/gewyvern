public sealed record MobileUiFormFieldBinding(
    string Key,
    string Label,
    string? Placeholder,
    bool Required,
    int MaxLength,
    UiFormInputKind InputKind);

public sealed record MobileUiFormBinding(
    string Title,
    string SubmitLabel,
    IReadOnlyList<MobileUiFormFieldBinding> Fields);

public sealed record MobileUiNodeBinding(
    string Id,
    UiNodeKind Kind,
    string? RuntimeId,
    string? Text,
    string? AccessibleName,
    string? AccessibleDescription,
    ActionKind? ActionKind,
    MobileUiFormBinding? Form,
    IReadOnlyList<MobileUiNodeBinding> Children);

public sealed class MobileUiDocumentBinding
{
    private readonly SemanticRenderer renderer;
    private readonly IReadOnlyDictionary<string, MobileUiNodeBinding> nodes;

    private MobileUiDocumentBinding(
        SemanticRenderer renderer,
        MobileUiNodeBinding root,
        IReadOnlyDictionary<string, MobileUiNodeBinding> nodes)
    {
        this.renderer = renderer;
        Root = root;
        this.nodes = nodes;
    }

    public ulong Revision => renderer.Document.Revision;
    public MobileUiNodeBinding Root { get; }

    public static MobileUiDocumentBinding Project(UiDocument document)
    {
        ArgumentNullException.ThrowIfNull(document);
        var renderer = new SemanticRenderer();
        renderer.Mount(document);
        var nodes = new Dictionary<string, MobileUiNodeBinding>(StringComparer.Ordinal);
        var root = ProjectNode(renderer.Document.Root, nodes);
        return new MobileUiDocumentBinding(renderer, root, nodes);
    }

    public MobileUiNodeBinding? Find(string nodeId)
    {
        if (nodeId is null)
        {
            return null;
        }
        return nodes.GetValueOrDefault(nodeId);
    }

    public bool HasSameNativePresentation(
        MobileUiDocumentBinding? other,
        string? ignoredNodeId = null) =>
        other is not null && SameNode(Root, other.Root, ignoredNodeId);

    public RemoteUiActionResolution ResolveActivation(
        string nodeId,
        RemoteFeedState state,
        RemoteMutationAvailability availability) =>
        RemoteUiActionRouter.ResolveActivation(
            renderer.Document,
            nodeId,
            state,
            availability);

    public RemoteUiActionResolution ResolveSubmission(
        string nodeId,
        IReadOnlyDictionary<string, string> values,
        RemoteFeedState state)
    {
        ArgumentNullException.ThrowIfNull(values);
        UiEvent submission;
        try
        {
            submission = renderer.CreateFormSubmission(nodeId, values);
        }
        catch (InvalidDataException)
        {
            return new RemoteUiActionResolution(
                null,
                RemoteUiActionFailure.InvalidFormValues,
                "The deployment form values are invalid");
        }
        return RemoteUiActionRouter.ResolveSubmission(
            renderer.Document,
            submission,
            state,
            nodeId);
    }

    public static void VerifyContract()
    {
        var runtime = new RemoteRuntimeProjection
        {
            Id = "runtime-a",
            Name = "Runtime A",
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
            "Live at revision 7",
            1,
            7);
        var source = RemoteWorkspaceDocumentProjection.Project(
            new RemoteWorkspaceSnapshot(7, runtime, [], []));
        var binding = Project(source);
        source.Root.Children.Clear();

        var deploy = binding.nodes.Values.Single(node =>
            node.ActionKind == ActionKind.RuntimeDeploy);
        if (deploy.Form is not
            {
                Fields:
                [
                { Key: "pipeline_kind", Required: true, InputKind: UiFormInputKind.PathToken },
                { Key: "target", Required: false, InputKind: UiFormInputKind.TrimmedText },
                ],
            })
        {
            throw new InvalidDataException(
                "mobile UI binding did not preserve the shared deployment form");
        }
        var availability = RemoteMutationAvailabilityPolicy.Evaluate(
            state,
            false,
            null,
            null);
        var activation = binding.ResolveActivation(deploy.Id, state, availability);
        if (!activation.Accepted
            || activation.Intent is not { Kind: ActionKind.RuntimeDeploy, Form: not null })
        {
            throw new InvalidDataException(
                "mobile UI binding did not route typed deployment activation");
        }
        var submitted = binding.ResolveSubmission(
            deploy.Id,
            new Dictionary<string, string>(StringComparer.Ordinal)
            {
                ["pipeline_kind"] = "http/request",
                ["target"] = "pid:42",
            },
            state);
        if (!submitted.Accepted
            || submitted.Intent is not
            {
                Kind: ActionKind.RuntimeDeploy,
                PipelineKind: "http/request",
                Target: "pid:42",
            })
        {
            throw new InvalidDataException(
                "mobile UI binding lost parameterized form-event values");
        }
        var rejected = binding.ResolveSubmission(
            deploy.Id,
            new Dictionary<string, string>(StringComparer.Ordinal)
            {
                ["pipeline_kind"] = "unsafe value",
            },
            state);
        if (rejected.Failure != RemoteUiActionFailure.InvalidFormValues)
        {
            throw new InvalidDataException(
                "mobile UI binding accepted an invalid form event");
        }
        if (binding.Root.Children.Count == 0)
        {
            throw new InvalidDataException(
                "mobile UI binding did not isolate itself from source mutation");
        }

        var fleet = Project(RemoteDocumentProjection.Project(state).Document);
        var heartbeat = Project(RemoteDocumentProjection.Project(state with
        {
            Revision = 8,
            Detail = "Live at revision 8",
        }).Document);
        if (!fleet.HasSameNativePresentation(heartbeat, "remote-state")
            || fleet.HasSameNativePresentation(heartbeat))
        {
            throw new InvalidDataException(
                "mobile native presentation did not isolate transient heartbeat status");
        }
        if (!ReferenceEquals(
                fleet,
                MobileNativeRenderGate.RetainEquivalentPresentation(
                    fleet,
                    heartbeat,
                    "remote-state")))
        {
            throw new InvalidDataException(
                "mobile native presentation did not retain its active action source");
        }
        var renderGate = new MobileNativeRenderGate();
        if (!renderGate.ShouldRender(fleet, availability, false, 1)
            || renderGate.ShouldRender(fleet, availability with { }, false, 1)
            || !renderGate.ShouldRender(fleet, availability, true, 1)
            || !renderGate.ShouldRender(fleet, availability, false, 2))
        {
            throw new InvalidDataException(
                "mobile native render gate did not suppress only an equivalent frame");
        }
        renderGate.Invalidate();
        if (!renderGate.ShouldRender(fleet, availability, false, 1))
        {
            throw new InvalidDataException(
                "mobile native render gate did not recover after invalidation");
        }
        runtime.Name = "Runtime B";
        var changed = Project(RemoteDocumentProjection.Project(state).Document);
        if (fleet.HasSameNativePresentation(changed, "remote-state")
            || fleet.HasSameNativePresentation(null, "remote-state")
            || ReferenceEquals(
                fleet,
                MobileNativeRenderGate.RetainEquivalentPresentation(
                    fleet,
                    changed,
                    "remote-state")))
        {
            throw new InvalidDataException(
                "mobile native presentation suppressed a visible document change");
        }
    }

    private static bool SameNode(
        MobileUiNodeBinding left,
        MobileUiNodeBinding right,
        string? ignoredNodeId)
    {
        if (!StringComparer.Ordinal.Equals(left.Id, right.Id)
            || left.Kind != right.Kind
            || !StringComparer.Ordinal.Equals(left.RuntimeId, right.RuntimeId)
            || !StringComparer.Ordinal.Equals(left.Text, right.Text)
            || !StringComparer.Ordinal.Equals(left.AccessibleName, right.AccessibleName)
            || !StringComparer.Ordinal.Equals(
                left.AccessibleDescription,
                right.AccessibleDescription)
            || left.ActionKind != right.ActionKind
            || !SameForm(left.Form, right.Form))
        {
            return false;
        }

        var leftIndex = 0;
        var rightIndex = 0;
        while (true)
        {
            SkipIgnored(left.Children, ignoredNodeId, ref leftIndex);
            SkipIgnored(right.Children, ignoredNodeId, ref rightIndex);
            var leftEnded = leftIndex == left.Children.Count;
            var rightEnded = rightIndex == right.Children.Count;
            if (leftEnded || rightEnded)
            {
                return leftEnded && rightEnded;
            }
            if (!SameNode(
                    left.Children[leftIndex],
                    right.Children[rightIndex],
                    ignoredNodeId))
            {
                return false;
            }
            leftIndex++;
            rightIndex++;
        }
    }

    private static bool SameForm(
        MobileUiFormBinding? left,
        MobileUiFormBinding? right)
    {
        if (ReferenceEquals(left, right))
        {
            return true;
        }
        if (left is null
            || right is null
            || !StringComparer.Ordinal.Equals(left.Title, right.Title)
            || !StringComparer.Ordinal.Equals(left.SubmitLabel, right.SubmitLabel)
            || left.Fields.Count != right.Fields.Count)
        {
            return false;
        }
        for (var index = 0; index < left.Fields.Count; index++)
        {
            if (left.Fields[index] != right.Fields[index])
            {
                return false;
            }
        }
        return true;
    }

    private static void SkipIgnored(
        IReadOnlyList<MobileUiNodeBinding> children,
        string? ignoredNodeId,
        ref int index)
    {
        while (index < children.Count
            && StringComparer.Ordinal.Equals(children[index].Id, ignoredNodeId))
        {
            index++;
        }
    }

    private static MobileUiNodeBinding ProjectNode(
        UiNode node,
        Dictionary<string, MobileUiNodeBinding> nodes)
    {
        var children = node.Children
            .Select(child => ProjectNode(child, nodes))
            .ToArray();
        var form = node.Action?.Form is { } sourceForm
            ? new MobileUiFormBinding(
                sourceForm.Title.Fallback,
                sourceForm.SubmitLabel.Fallback,
                sourceForm.Fields.Select(field => new MobileUiFormFieldBinding(
                    field.Key,
                    field.Label.Fallback,
                    field.Placeholder?.Fallback,
                    field.Required,
                    field.MaxLength,
                    field.InputKind)).ToArray())
            : null;
        var projected = new MobileUiNodeBinding(
            node.Id,
            node.Kind,
            node.RuntimeId,
            node.Text?.Fallback,
            node.Accessibility.Label?.Fallback,
            node.Accessibility.Description?.Fallback,
            node.Action?.Kind,
            form,
            children);
        if (!nodes.TryAdd(projected.Id, projected))
        {
            throw new InvalidDataException("mobile UI binding contains a duplicate node ID");
        }
        return projected;
    }
}
