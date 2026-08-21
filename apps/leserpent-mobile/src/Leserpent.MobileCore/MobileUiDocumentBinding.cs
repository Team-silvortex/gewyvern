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
