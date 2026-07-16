internal static class RemoteDocumentProjection
{
    public static UiDocument Project(RemoteFeedState state) => new()
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
                .. state.Runtimes.Select(RuntimeCard),
            ],
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
