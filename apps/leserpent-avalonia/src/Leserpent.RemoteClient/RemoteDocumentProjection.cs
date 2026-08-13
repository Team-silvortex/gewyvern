public static class RemoteDocumentProjection
{
    public const int MaxFilterLength = RemoteRuntimeSearch.MaxFilterLength;

    public static RemoteDocumentView Project(RemoteFeedState state, string? filter = null)
    {
        var normalizedFilter = RemoteRuntimeSearch.Normalize(filter);
        var runtimes = normalizedFilter.Length == 0
            ? state.Runtimes
            : state.Runtimes.Where(runtime => RemoteRuntimeSearch.Matches(runtime, normalizedFilter))
                .ToArray();
        var filterNodes = normalizedFilter.Length == 0
            ? Array.Empty<UiNode>()
            :
            [
                TextNode(
                    "remote-filter-summary",
                    UiNodeKind.Text,
                    "remote.filter.summary",
                    $"Showing {runtimes.Count} of {state.Runtimes.Count} runtimes"),
                .. runtimes.Count == 0
                    ?
                    [
                        TextNode(
                            "remote-filter-empty",
                            UiNodeKind.Text,
                            "remote.filter.empty",
                            "No runtimes match the current filter"),
                    ]
                    : Array.Empty<UiNode>(),
            ];
        var document = new UiDocument
        {
            SchemaVersion = 1,
            Revision = state.Revision ?? 0,
            Root = new UiNode
            {
                Id = "remote-fleet",
                Kind = UiNodeKind.Column,
                Accessibility = Accessibility("remote.fleet", "Remote runtime fleet"),
                Children =
                [
                    TextNode("remote-title", UiNodeKind.Heading, "remote.title", "Remote runtimes"),
                    TextNode("remote-state", UiNodeKind.Text, "remote.state", Safe(state.Detail)),
                    .. filterNodes,
                    .. runtimes.Select(RuntimeCard),
                ],
            },
        };
        return new RemoteDocumentView(document, runtimes.Count, state.Runtimes.Count);
    }

    public static void VerifyFilterContract()
    {
        RemoteRuntimeSearch.VerifyContract();
        var state = new RemoteFeedState(
            RemoteFeedPhase.Live,
            9,
            new[]
            {
                Runtime("runtime-a", "Payments API", "production", RefreshStatus.Ready),
                Runtime("runtime-b", "Queue Worker", "staging", RefreshStatus.Failed),
            },
            0,
            false,
            "Live at revision 9");
        RequireSingle(Project(state, "payments"), "runtime-a", "name filter");
        RequireSingle(Project(state, "RUNTIME-B"), "runtime-b", "ID filter");
        RequireSingle(Project(state, "PRODUCTION"), "runtime-a", "tag filter");
        RequireSingle(Project(state, "failed"), "runtime-b", "status filter");
        RequireSingle(Project(state, "prod\0uction"), "runtime-a", "sanitized filter");

        var all = Project(state, "  ");
        if (all.VisibleRuntimeCount != 2 || all.TotalRuntimeCount != 2)
        {
            throw new InvalidDataException("empty runtime filter did not restore all runtimes");
        }
        if (all.Document.Root.Children
            .Where(node => node.Kind == UiNodeKind.RuntimeCard)
            .Any(card => card.Children.All(node =>
                node.Action?.Kind != ActionKind.RuntimeCapabilitiesRefresh)))
        {
            throw new InvalidDataException(
                "runtime card omitted the capability discovery action");
        }
        var empty = Project(state, "does-not-exist");
        if (empty.VisibleRuntimeCount != 0
            || empty.Document.Root.Children.All(node => node.Id != "remote-filter-empty"))
        {
            throw new InvalidDataException("runtime filter did not expose its empty state");
        }
    }

    private static void RequireSingle(
        RemoteDocumentView view,
        string expectedRuntimeId,
        string caseName)
    {
        var cards = view.Document.Root.Children
            .Where(node => node.Kind == UiNodeKind.RuntimeCard)
            .ToArray();
        if (view.VisibleRuntimeCount != 1
            || view.TotalRuntimeCount != 2
            || cards is not [{ RuntimeId: var runtimeId }]
            || runtimeId != expectedRuntimeId)
        {
            throw new InvalidDataException($"{caseName} did not select the expected runtime");
        }
    }

    private static RemoteRuntimeProjection Runtime(
        string id,
        string name,
        string environment,
        RefreshStatus refreshStatus) => new()
    {
        Id = id,
        Name = name,
        Revision = 9,
        RefreshStatus = refreshStatus,
        Tags = new RuntimeTags
        {
            Environment = environment,
        },
        Status = new RuntimeStatusSnapshot
        {
            StatusSource = "gewyvern",
        },
    };

    private static UiNode RuntimeCard(RemoteRuntimeProjection runtime)
    {
        var status = runtime.Status.StatusFetchError is { Length: > 0 } error
            ? $"Refresh failed: {error}"
            : $"{runtime.RefreshStatus} / source {runtime.Status.StatusSource}";
        var tags = new[]
        {
            runtime.Tags.Environment,
            runtime.Tags.Cluster,
            runtime.Tags.Role,
        }.Where(value => !string.IsNullOrWhiteSpace(value));
        var tagText = string.Join(" / ", tags);
        return new UiNode
        {
            Id = $"runtime:{runtime.Id}",
            Kind = UiNodeKind.RuntimeCard,
            RuntimeId = runtime.Id,
            Accessibility = Accessibility(
                $"runtime.{runtime.Id}",
                $"Runtime {Safe(runtime.Name)}"),
            Children =
            [
                TextNode(
                    $"runtime:{runtime.Id}:name",
                    UiNodeKind.Heading,
                    "runtime.name",
                    Safe(runtime.Name)),
                TextNode(
                    $"runtime:{runtime.Id}:status",
                    UiNodeKind.Text,
                    "runtime.status",
                    Safe(status)),
                TextNode(
                    $"runtime:{runtime.Id}:revision",
                    UiNodeKind.Text,
                    "runtime.revision",
                    $"Revision {runtime.Revision} / refreshes {runtime.RefreshCount}"),
                TextNode(
                    $"runtime:{runtime.Id}:tags",
                    UiNodeKind.Text,
                    "runtime.tags",
                    string.IsNullOrEmpty(tagText) ? "No deployment tags" : Safe(tagText)),
                new UiNode
                {
                    Id = $"runtime:{runtime.Id}:inspect",
                    Kind = UiNodeKind.Action,
                    Text = new LocalizedText
                    {
                        Key = "runtime.inspect",
                        Fallback = "Inspect runtime",
                    },
                    Accessibility = new Accessibility
                    {
                        Label = new LocalizedText
                        {
                            Key = "runtime.inspect",
                            Fallback = $"Inspect runtime {Safe(runtime.Name)}",
                        },
                        Description = new LocalizedText
                        {
                            Key = "runtime.inspect.description",
                            Fallback = "Open the read-only runtime workspace",
                        },
                    },
                    Action = new UiAction
                    {
                        Kind = ActionKind.RuntimeInspect,
                        RuntimeId = runtime.Id,
                    },
                    Children = [],
                },
                new UiNode
                {
                    Id = $"runtime:{runtime.Id}:refresh",
                    Kind = UiNodeKind.Action,
                    Text = new LocalizedText
                    {
                        Key = "runtime.refresh",
                        Fallback = "Refresh runtime",
                    },
                    Accessibility = new Accessibility
                    {
                        Label = new LocalizedText
                        {
                            Key = "runtime.refresh",
                            Fallback = $"Refresh runtime {Safe(runtime.Name)}",
                        },
                        Description = new LocalizedText
                        {
                            Key = "runtime.refresh.description",
                            Fallback = "Requires explicit confirmation before changing remote state",
                        },
                    },
                    Action = new UiAction
                    {
                        Kind = ActionKind.RuntimeRefresh,
                        RuntimeId = runtime.Id,
                    },
                    Children = [],
                },
                new UiNode
                {
                    Id = $"runtime:{runtime.Id}:capabilities-refresh",
                    Kind = UiNodeKind.Action,
                    Text = new LocalizedText
                    {
                        Key = "runtime.capabilities.refresh",
                        Fallback = "Discover capabilities",
                    },
                    Accessibility = new Accessibility
                    {
                        Label = new LocalizedText
                        {
                            Key = "runtime.capabilities.refresh",
                            Fallback = $"Discover capabilities for {Safe(runtime.Name)}",
                        },
                        Description = new LocalizedText
                        {
                            Key = "runtime.capabilities.refresh.description",
                            Fallback = "Requires explicit confirmation before querying the remote runtime",
                        },
                    },
                    Action = new UiAction
                    {
                        Kind = ActionKind.RuntimeCapabilitiesRefresh,
                        RuntimeId = runtime.Id,
                    },
                    Children = [],
                },
            ],
        };
    }

    private static UiNode TextNode(
        string id,
        UiNodeKind kind,
        string key,
        string fallback) => new()
    {
        Id = id,
        Kind = kind,
        Text = new LocalizedText { Key = key, Fallback = fallback },
        Accessibility = Accessibility(key, fallback),
        Children = [],
    };

    private static Accessibility Accessibility(string key, string fallback) => new()
    {
        Label = new LocalizedText { Key = key, Fallback = fallback },
    };

    private static string Safe(string value)
    {
        var sanitized = new string(value
            .Where(character => !char.IsControl(character))
            .Take(1024)
            .ToArray());
        return string.IsNullOrWhiteSpace(sanitized) ? "Unavailable" : sanitized;
    }
}

public sealed record RemoteDocumentView(
    UiDocument Document,
    int VisibleRuntimeCount,
    int TotalRuntimeCount);
