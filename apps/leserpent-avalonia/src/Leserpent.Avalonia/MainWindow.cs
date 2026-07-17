using System.Text.Json;
using System.Text.Json.Nodes;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Media;

internal sealed class MainWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly TextBlock statusText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 13,
        Text = "No action selected",
    };

    public int RenderedNodeCount { get; }
    public int AppliedPatchOperations { get; }
    public int ReusedNodeCount { get; }
    public int VirtualizedHostCount { get; }
    public int ActiveVirtualizedHostCount => renderer.ActiveVirtualizedHostCount;
    public int InitialUnrealizedVirtualItemCount { get; }
    public int UnrealizedVirtualItemCount => renderer.UnrealizedVirtualItemCount;
    public int InitialUnrealizedNodeCount { get; }
    public int UnrealizedNodeCount => renderer.UnrealizedNodeCount;
    public int InitialDebuggerCancelButtonCount { get; }
    public int DebuggerCancelButtonCount => renderer.RealizedDebuggerCancelButtonCount;
    public int DisabledActionProbeCount { get; private set; }
    public AccessibilityAudit InitialAccessibility { get; }
    public AccessibilityAudit Accessibility => renderer.AuditAccessibility();
    public ulong Revision { get; }

    public MainWindow(RendererFixture fixture)
    {
        Width = 1080;
        Height = 760;
        MinWidth = 640;
        MinHeight = 480;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        renderer = new AvaloniaDocumentRenderer(OnActionInvoked);
        renderer.Mount(fixture.Previous);
        InitialDebuggerCancelButtonCount = renderer.RealizedDebuggerCancelButtonCount;
        InitialAccessibility = renderer.AuditAccessibility();
        renderer.Apply(fixture.Patch);
        RequireExpectedDocument(renderer.Document, fixture.Next);
        _ = renderer.AuditAccessibility();

        Revision = renderer.Document.Revision;
        RenderedNodeCount = renderer.NodeCount;
        AppliedPatchOperations = renderer.LastAppliedOperationCount;
        ReusedNodeCount = renderer.LastReusedNodeCount;
        VirtualizedHostCount = renderer.VirtualizedHostCount;
        InitialUnrealizedVirtualItemCount = renderer.UnrealizedVirtualItemCount;
        InitialUnrealizedNodeCount = renderer.UnrealizedNodeCount;
        Title = $"Leserpent / revision {Revision}";
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto"),
            Children =
            {
                new Border
                {
                    Padding = new Thickness(32, 28),
                    Child = renderer.Surface,
                },
                BuildStatusBar(),
            },
        };
    }

    public void ProbeActionAvailability()
    {
        ActionKind[] mutationKinds =
        [
            ActionKind.RuntimeRefresh,
            ActionKind.RuntimeCapabilitiesRefresh,
            ActionKind.RuntimeDeploy,
        ];
        var realizedActions = mutationKinds.Sum(renderer.RealizedActionCount);
        if (realizedActions == 0)
        {
            DisabledActionProbeCount = 0;
            _ = renderer.AuditAccessibility();
            return;
        }
        foreach (var kind in mutationKinds)
        {
            renderer.SetActionAvailability(
                kind,
                false,
                "Verification action is temporarily unavailable");
        }
        DisabledActionProbeCount = mutationKinds.Sum(
            renderer.RealizedDisabledActionCount);
        if (DisabledActionProbeCount != realizedActions)
        {
            throw new InvalidDataException(
                "Avalonia action availability did not disable every realized mutation action");
        }
        foreach (var kind in mutationKinds)
        {
            renderer.SetActionAvailability(kind, true, null);
        }
        if (mutationKinds.Sum(renderer.RealizedDisabledActionCount) != 0)
        {
            throw new InvalidDataException(
                "Avalonia action availability did not restore every realized mutation action");
        }
        _ = renderer.AuditAccessibility();
    }

    public string BeginFocusRetentionProbe()
    {
        var nodeId = renderer.FirstRealizedActionNodeId
            ?? throw new InvalidDataException("focus probe requires a realized action");
        if (!renderer.TryFocusNode(nodeId) || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException("focus probe could not focus its action node");
        }
        renderer.Mount(renderer.Document);
        if (!renderer.IsFocusRestorePending)
        {
            throw new InvalidDataException("focus restoration was not scheduled after mount");
        }
        return nodeId;
    }

    public void CompleteFocusRetentionProbe(string nodeId)
    {
        if (renderer.IsFocusRestorePending || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException(
                "stable action focus was not restored after document mount");
        }
    }

    public void BeginPatchedFocusRetentionProbe(string nodeId)
    {
        if (!renderer.TryFocusNode(nodeId) || renderer.FocusedNodeId != nodeId)
        {
            throw new InvalidDataException("patch focus probe could not focus its action node");
        }
        var source = FindNode(renderer.Document.Root, nodeId)
            ?? throw new InvalidDataException("patch focus probe action was not found");
        var revision = renderer.Document.Revision;
        renderer.Apply(new UiPatch
        {
            SchemaVersion = 1,
            FromRevision = revision,
            ToRevision = checked(revision + 1),
            Operations =
            [
                new UiPatchOperation
                {
                    Kind = PatchKind.Update,
                    Node = CloneShallow(source),
                },
            ],
        });
        if (!renderer.IsFocusRestorePending)
        {
            throw new InvalidDataException("focus restoration was not scheduled after patch");
        }
    }

    public void ProbeRemovedFocusTarget(string nodeId)
    {
        var payload = JsonSerializer.SerializeToUtf8Bytes(
            renderer.Document,
            RendererJsonContext.Default.UiDocument);
        var withoutFocusedAction = JsonSerializer.Deserialize(
            payload,
            RendererJsonContext.Default.UiDocument)
            ?? throw new InvalidDataException("focus probe document clone failed");
        if (!RemoveNode(withoutFocusedAction.Root, nodeId))
        {
            throw new InvalidDataException("focus probe could not remove its action node");
        }
        renderer.Mount(withoutFocusedAction);
        if (renderer.IsFocusRestorePending || renderer.FocusedNodeId is not null)
        {
            throw new InvalidDataException(
                "removed action focus was transferred to another control");
        }
    }

    private Border BuildStatusBar()
    {
        var revisionText = new TextBlock
        {
            Foreground = LeserpentTheme.Primary,
            FontSize = 12,
            FontWeight = FontWeight.SemiBold,
            Text = $"UI IR v1  /  rev {Revision}",
        };
        var bar = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(24, 12),
            Child = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
                Children = { statusText, revisionText },
            },
        };
        Grid.SetColumn(revisionText, 1);
        Grid.SetRow(bar, 1);
        return bar;
    }

    private void OnActionInvoked(string nodeId)
    {
        statusText.Text = $"Action node emitted: {nodeId}";
        statusText.Foreground = LeserpentTheme.Accent;
    }

    private static void RequireExpectedDocument(UiDocument actual, UiDocument expected)
    {
        if (!JsonNode.DeepEquals(
            JsonSerializer.SerializeToNode(actual, RendererJsonContext.Default.UiDocument),
            JsonSerializer.SerializeToNode(expected, RendererJsonContext.Default.UiDocument)))
        {
            throw new InvalidDataException("Avalonia patch result does not match the fixture");
        }
    }

    private static bool RemoveNode(UiNode parent, string nodeId)
    {
        var index = parent.Children.FindIndex(child => child.Id == nodeId);
        if (index >= 0)
        {
            parent.Children.RemoveAt(index);
            return true;
        }
        return parent.Children.Any(child => RemoveNode(child, nodeId));
    }

    private static UiNode? FindNode(UiNode node, string nodeId)
    {
        if (node.Id == nodeId)
        {
            return node;
        }
        return node.Children.Select(FindChild).FirstOrDefault(found => found is not null);

        UiNode? FindChild(UiNode child) => FindNode(child, nodeId);
    }

    private static UiNode CloneShallow(UiNode node) => new()
    {
        Id = node.Id,
        Kind = node.Kind,
        RuntimeId = node.RuntimeId,
        DebuggerSessionId = node.DebuggerSessionId,
        Text = node.Text is null ? null : new LocalizedText
        {
            Key = node.Text.Key,
            Fallback = node.Text.Fallback,
        },
        Accessibility = new Accessibility
        {
            Label = node.Accessibility.Label is null ? null : new LocalizedText
            {
                Key = node.Accessibility.Label.Key,
                Fallback = node.Accessibility.Label.Fallback,
            },
            Description = node.Accessibility.Description is null ? null : new LocalizedText
            {
                Key = node.Accessibility.Description.Key,
                Fallback = node.Accessibility.Description.Fallback,
            },
        },
        Action = node.Action is null ? null : new UiAction
        {
            Kind = node.Action.Kind,
            RuntimeId = node.Action.RuntimeId,
            SessionId = node.Action.SessionId,
        },
        Children = [],
    };
}
