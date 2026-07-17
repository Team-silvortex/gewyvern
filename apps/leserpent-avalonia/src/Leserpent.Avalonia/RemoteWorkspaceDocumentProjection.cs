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
                != "[WARNING] bounded warning")
        {
            throw new InvalidDataException(
                "remote workspace projection leaked transport identity");
        }
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

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
