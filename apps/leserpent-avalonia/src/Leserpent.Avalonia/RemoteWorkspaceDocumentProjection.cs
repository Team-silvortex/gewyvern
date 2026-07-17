internal static class RemoteWorkspaceDocumentProjection
{
    public static UiDocument Project(RemoteWorkspaceSnapshot snapshot)
    {
        var runtime = snapshot.Runtime;
        var prefix = $"workspace:{runtime.Id}";
        var history = snapshot.History.Count == 0
            ?
            [
                TextNode(
                    $"{prefix}:history:empty",
                    UiNodeKind.Text,
                    "runtime.history.empty",
                    "No applied commands"),
            ]
            : snapshot.History.Select(entry => new UiNode
            {
                Id = $"{prefix}:history:{entry.Revision}:{entry.CommandId}",
                Kind = UiNodeKind.HistoryEntry,
                Text = new LocalizedText
                {
                    Key = "runtime.history.entry",
                    Fallback = $"Revision {entry.Revision}: {entry.Status}",
                },
                Accessibility = new Accessibility(),
                Children = [],
            }).ToArray();
        var logs = snapshot.Logs.Count == 0
            ?
            [
                TextNode(
                    $"{prefix}:logs:empty",
                    UiNodeKind.Text,
                    "runtime.logs.empty",
                    "No log entries"),
            ]
            : snapshot.Logs.Select(entry => new UiNode
            {
                Id = $"{prefix}:logs:{entry.Sequence}",
                Kind = UiNodeKind.LogEntry,
                Text = new LocalizedText
                {
                    Key = "runtime.logs.entry",
                    Fallback = $"[{entry.Level.ToUpperInvariant()}] {entry.Display}",
                },
                Accessibility = new Accessibility(),
                Children = [],
            }).ToArray();
        return new UiDocument
        {
            SchemaVersion = 1,
            Revision = snapshot.Revision,
            Root = new UiNode
            {
                Id = prefix,
                Kind = UiNodeKind.RuntimeWorkspace,
                RuntimeId = runtime.Id,
                Accessibility = new Accessibility
                {
                    Label = Localized("runtime.workspace", Safe(runtime.Name)),
                },
                Children =
                [
                    TextNode(
                        $"{prefix}:title",
                        UiNodeKind.Heading,
                        "runtime.workspace.title",
                        Safe(runtime.Name)),
                    TextNode(
                        $"{prefix}:revision",
                        UiNodeKind.Text,
                        "runtime.workspace.revision",
                        $"Revision {snapshot.Revision}"),
                    TextNode(
                        $"{prefix}:status",
                        UiNodeKind.Text,
                        "runtime.workspace.status",
                        RefreshStatusText(runtime.RefreshStatus)),
                    TextNode(
                        $"{prefix}:snapshot",
                        UiNodeKind.Text,
                        "runtime.workspace.snapshot",
                        runtime.Status.HasLatestSnapshot
                            ? "Latest snapshot available"
                            : "No runtime snapshot"),
                    new UiNode
                    {
                        Id = $"runtime:{runtime.Id}:refresh",
                        Kind = UiNodeKind.Action,
                        Text = Localized("runtime.workspace.refresh", "Refresh"),
                        Accessibility = new Accessibility
                        {
                            Label = Localized(
                                "runtime.workspace.refresh",
                                $"Refresh runtime {Safe(runtime.Name)}"),
                            Description = Localized(
                                "runtime.workspace.refresh.description",
                                "Requires confirmation in the fleet window"),
                        },
                        Action = new UiAction
                        {
                            Kind = ActionKind.RuntimeRefresh,
                            RuntimeId = runtime.Id,
                        },
                        Children = [],
                    },
                    CapabilitySection(prefix, runtime),
                    new UiNode
                    {
                        Id = $"{prefix}:history",
                        Kind = UiNodeKind.Section,
                        Text = Localized("runtime.history.title", "History"),
                        Accessibility = new Accessibility
                        {
                            Label = Localized("runtime.history.title", "Runtime history"),
                        },
                        Children = [.. history],
                    },
                    new UiNode
                    {
                        Id = $"{prefix}:logs",
                        Kind = UiNodeKind.Section,
                        Text = Localized("runtime.logs.title", "Logs"),
                        Accessibility = new Accessibility
                        {
                            Label = Localized("runtime.logs.title", "Runtime logs"),
                        },
                        Children = [.. logs],
                    },
                ],
            },
        };
    }

    private static UiNode CapabilitySection(
        string prefix,
        RemoteRuntimeProjection runtime)
    {
        var capabilities = runtime.Capabilities;
        UiNode[] details = capabilities is null || capabilities.IsUnobserved
            ?
            [
                TextNode(
                    $"{prefix}:capabilities:unobserved",
                    UiNodeKind.Text,
                    "runtime.capabilities.unobserved",
                    "Capabilities have not been observed"),
            ]
            :
            [
                TextNode(
                    $"{prefix}:capabilities:summary",
                    UiNodeKind.Text,
                    "runtime.capabilities.summary",
                    $"{Safe(capabilities.Service)} {Safe(capabilities.Version)} / latest snapshot {YesNo(capabilities.LatestSnapshot)} / authenticated deployment {YesNo(capabilities.AuthenticatedDeployment)} / serve required {YesNo(capabilities.ServeRequired)} / sidecar context {YesNo(capabilities.ExternalSidecarContext)}"),
                TextNode(
                    $"{prefix}:capabilities:binding",
                    UiNodeKind.Text,
                    "runtime.capabilities.binding",
                    runtime.CapabilitiesObservedForRevision is { } observedFor
                        ? $"Observed for command revision {observedFor}"
                        : "Observation command revision unavailable (legacy projection)"),
                TextNode(
                    $"{prefix}:capabilities:encoding",
                    UiNodeKind.Text,
                    "runtime.capabilities.encoding",
                    $"Target encoding: {Safe(capabilities.TargetPathSegmentEncoding)} / direct characters: {Safe(capabilities.TargetDirectPathChars)}"),
                .. capabilities.Endpoints.Select((endpoint, index) => TextNode(
                    $"{prefix}:capabilities:endpoint:{index}",
                    UiNodeKind.Text,
                    "runtime.capabilities.endpoint",
                    $"Endpoint: {Safe(endpoint)}")),
                .. capabilities.Extensions.OrderBy(entry => entry.Key, StringComparer.Ordinal)
                    .Select((entry, index) => TextNode(
                        $"{prefix}:capabilities:extension:{index}",
                        UiNodeKind.Text,
                        "runtime.capabilities.extension",
                        $"Extension {Safe(entry.Key)}: {YesNo(entry.Value)}")),
            ];
        var children = new List<UiNode>(details)
        {
            new UiNode
            {
                Id = $"runtime:{runtime.Id}:capabilities-refresh",
                Kind = UiNodeKind.Action,
                Text = Localized(
                    "runtime.capabilities.refresh",
                    "Discover capabilities"),
                Accessibility = new Accessibility
                {
                    Label = Localized(
                        "runtime.capabilities.refresh",
                        $"Discover capabilities for {Safe(runtime.Name)}"),
                    Description = Localized(
                        "runtime.capabilities.refresh.description",
                        "Requires confirmation in the fleet window"),
                },
                Action = new UiAction
                {
                    Kind = ActionKind.RuntimeCapabilitiesRefresh,
                    RuntimeId = runtime.Id,
                },
                Children = [],
            },
        };
        if (capabilities is { AuthenticatedDeployment: true })
        {
            children.Add(new UiNode
            {
                Id = $"runtime:{runtime.Id}:deploy",
                Kind = UiNodeKind.Action,
                Text = Localized("runtime.deploy", "Deploy pipeline"),
                Accessibility = new Accessibility
                {
                    Label = Localized(
                        "runtime.deploy",
                        $"Deploy a pipeline to {Safe(runtime.Name)}"),
                    Description = Localized(
                        "runtime.deploy.description",
                        "Opens a bounded deployment form and requires explicit confirmation"),
                },
                Action = new UiAction
                {
                    Kind = ActionKind.RuntimeDeploy,
                    RuntimeId = runtime.Id,
                    Form = DeploymentForm(),
                },
                Children = [],
            });
        }
        return new UiNode
        {
            Id = $"{prefix}:capabilities",
            Kind = UiNodeKind.Section,
            Text = Localized("runtime.capabilities.title", "Capabilities"),
            Accessibility = new Accessibility
            {
                Label = Localized("runtime.capabilities.title", "Runtime capabilities"),
            },
            Children = children,
        };
    }

    private static UiForm DeploymentForm() => new()
    {
        Title = Localized("runtime.deploy.form.title", "Confirm remote deployment"),
        SubmitLabel = Localized("runtime.deploy.form.submit", "Deploy pipeline"),
        Fields =
        [
            new UiFormField
            {
                Key = "pipeline_kind",
                Label = Localized("runtime.deploy.form.pipeline_kind", "Pipeline kind"),
                Placeholder = Localized(
                    "runtime.deploy.form.pipeline_kind.placeholder",
                    "http/request"),
                Required = true,
                MaxLength = 128,
                InputKind = UiFormInputKind.PathToken,
            },
            new UiFormField
            {
                Key = "target",
                Label = Localized("runtime.deploy.form.target", "Optional target"),
                Placeholder = Localized(
                    "runtime.deploy.form.target.placeholder",
                    "For example pid:42"),
                Required = false,
                MaxLength = 256,
                InputKind = UiFormInputKind.TrimmedText,
            },
        ],
    };

    public static void VerifyEndpointIsolation()
    {
        var snapshot = new RemoteWorkspaceSnapshot(
            7,
            new RemoteRuntimeProjection
            {
                Id = "runtime-a",
                Name = "Payments",
                Revision = 7,
                RefreshStatus = RefreshStatus.Ready,
                Tags = new RuntimeTags(),
                Status = new RuntimeStatusSnapshot { StatusSource = "gewyvern" },
                CapabilitiesObservedForRevision = 6,
                Capabilities = new RuntimeCapabilitySnapshot
                {
                    Source = "gewyvern-api",
                    Service = "gewyvern-api",
                    Version = "1.2.0",
                    LatestSnapshot = true,
                    AuthenticatedDeployment = true,
                    ServeRequired = true,
                    ExternalSidecarContext = true,
                    TargetPathSegmentEncoding = "percent-encoding",
                    TargetDirectPathChars = "A-Z a-z 0-9 . _ ~ :",
                    Endpoints = ["/v1/capabilities", "/v1/deployments"],
                    Extensions = new Dictionary<string, bool>(StringComparer.Ordinal)
                    {
                        ["protocol_catalog"] = true,
                    },
                },
            },
            [new RemoteHistoryProjection("command-a", 7, "applied")],
            [new RemoteLogProjection(1, "warning", "bounded warning")]);
        var document = Project(snapshot);
        var renderer = new SemanticRenderer();
        renderer.Mount(document);
        if (document.Root.RuntimeId != "runtime-a"
            || Descendants(document.Root).Any(node =>
                node.Text?.Fallback.Contains("https://", StringComparison.Ordinal) == true)
            || Descendants(document.Root).Single(node =>
                node.Id == "workspace:runtime-a:logs:1").Text?.Fallback
                != "[WARNING] bounded warning"
            || Descendants(document.Root).All(node =>
                node.Action?.Kind != ActionKind.RuntimeCapabilitiesRefresh)
            || Descendants(document.Root).All(node =>
                node.Action?.Kind != ActionKind.RuntimeDeploy)
            || Descendants(document.Root).All(node =>
                node.Text?.Fallback == "Endpoint: /v1/capabilities")
            || Descendants(document.Root).All(node =>
                node.Text?.Fallback == "Observed for command revision 6"))
        {
            throw new InvalidDataException(
                "remote workspace projection leaked transport identity");
        }

        var capabilities = snapshot.Runtime.Capabilities
            ?? throw new InvalidDataException("verification snapshot lost capabilities");
        capabilities.AuthenticatedDeployment = false;
        capabilities.Endpoints = ["/v1/capabilities"];
        var restrictedDocument = Project(snapshot);
        if (Descendants(restrictedDocument.Root).Any(node =>
                node.Action?.Kind == ActionKind.RuntimeDeploy))
        {
            throw new InvalidDataException(
                "remote workspace exposed deployment without an authenticated capability");
        }
    }

    public static void VerifyParameterizedFormContract()
    {
        var document = new UiDocument
        {
            SchemaVersion = 1,
            Revision = 7,
            Root = new UiNode
            {
                Id = "runtime:runtime-a",
                Kind = UiNodeKind.RuntimeWorkspace,
                RuntimeId = "runtime-a",
                Accessibility = new Accessibility
                {
                    Label = Localized("runtime.workspace", "Runtime A"),
                },
                Children =
                [
                    new UiNode
                    {
                        Id = "runtime:runtime-a:deploy",
                        Kind = UiNodeKind.Action,
                        Text = Localized("runtime.deploy", "Deploy pipeline"),
                        Accessibility = new Accessibility
                        {
                            Label = Localized("runtime.deploy", "Deploy pipeline"),
                        },
                        Action = new UiAction
                        {
                            Kind = ActionKind.RuntimeDeploy,
                            RuntimeId = "runtime-a",
                            Form = DeploymentForm(),
                        },
                        Children = [],
                    },
                ],
            },
        };
        var renderer = new SemanticRenderer();
        renderer.Mount(document);
        var submission = renderer.CreateFormSubmission(
            "runtime:runtime-a:deploy",
            new Dictionary<string, string>(StringComparer.Ordinal)
            {
                ["pipeline_kind"] = "http/request",
                ["target"] = "pid:42",
            });
        var json = System.Text.Json.JsonSerializer.Serialize(
            submission,
            RendererJsonContext.Default.UiEvent);
        if (submission.Kind != UiEventKind.Submit
            || submission.Values["pipeline_kind"] != "http/request"
            || !json.Contains("\"kind\":\"submit\"", StringComparison.Ordinal))
        {
            throw new InvalidDataException("parameterized form event was not preserved");
        }
        try
        {
            renderer.CreateFormSubmission(
                "runtime:runtime-a:deploy",
                new Dictionary<string, string>(StringComparer.Ordinal)
                {
                    ["pipeline_kind"] = "unsafe value",
                });
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException("parameterized form accepted an invalid value");
    }

    private static IEnumerable<UiNode> Descendants(UiNode node)
    {
        yield return node;
        foreach (var child in node.Children)
        {
            foreach (var descendant in Descendants(child))
            {
                yield return descendant;
            }
        }
    }

    private static string RefreshStatusText(RefreshStatus status) => status switch
    {
        RefreshStatus.NeverRequested => "Never requested",
        RefreshStatus.Pending => "Refresh pending",
        RefreshStatus.Ready => "Ready",
        RefreshStatus.Failed => "Refresh failed",
        _ => throw new InvalidDataException("unknown runtime refresh status"),
    };

    private static UiNode TextNode(
        string id,
        UiNodeKind kind,
        string key,
        string fallback) => new()
    {
        Id = id,
        Kind = kind,
        Text = Localized(key, fallback),
        Accessibility = new Accessibility { Label = Localized(key, fallback) },
        Children = [],
    };

    private static LocalizedText Localized(string key, string fallback) => new()
    {
        Key = key,
        Fallback = fallback,
    };

    private static string YesNo(bool value) => value ? "yes" : "no";

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
