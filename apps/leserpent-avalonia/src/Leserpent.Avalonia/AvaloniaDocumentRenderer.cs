using System.Collections.ObjectModel;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Templates;
using Avalonia.Media;
using Avalonia.Threading;
using Leserpent.Avalonia;

internal static class LeserpentTheme
{
    private static readonly Color CanvasColor = Color.Parse("#11100D");
    private static readonly Color PanelColor = Color.Parse("#1C1913");
    private static readonly Color PrimaryColor = Color.Parse("#F4C95D");
    private static readonly Color AccentColor = Color.Parse("#FF9418");
    private static readonly Color BodyColor = Color.Parse("#E9E1D0");
    private static readonly Color MutedColor = Color.Parse("#B9AA8A");
    private static readonly Color DestructiveColor = Color.Parse("#C44D2D");

    public static readonly IBrush Canvas = new SolidColorBrush(CanvasColor);
    public static readonly IBrush Panel = new SolidColorBrush(PanelColor);
    public static readonly IBrush PanelBorder = Brush.Parse("#514224");
    public static readonly IBrush Primary = new SolidColorBrush(PrimaryColor);
    public static readonly IBrush Accent = new SolidColorBrush(AccentColor);
    public static readonly IBrush Body = new SolidColorBrush(BodyColor);
    public static readonly IBrush Muted = new SolidColorBrush(MutedColor);
    public static readonly IBrush Destructive = new SolidColorBrush(DestructiveColor);

    public static double MinimumTextContrastRatio => new[]
    {
        ContrastRatio(BodyColor, CanvasColor),
        ContrastRatio(MutedColor, PanelColor),
        ContrastRatio(PrimaryColor, CanvasColor),
        ContrastRatio(Colors.Black, AccentColor),
        ContrastRatio(Colors.White, DestructiveColor),
    }.Min();

    private static double ContrastRatio(Color foreground, Color background)
    {
        var light = Math.Max(Luminance(foreground), Luminance(background));
        var dark = Math.Min(Luminance(foreground), Luminance(background));
        return (light + 0.05) / (dark + 0.05);
    }

    private static double Luminance(Color color) =>
        0.2126 * Linear(color.R) + 0.7152 * Linear(color.G) + 0.0722 * Linear(color.B);

    private static double Linear(byte channel)
    {
        var value = channel / 255.0;
        return value <= 0.04045
            ? value / 12.92
            : Math.Pow((value + 0.055) / 1.055, 2.4);
    }
}

internal sealed record AccessibilityAudit(
    int RealizedControls,
    int AutomationNames,
    int ExplicitLabels,
    int ActionControls,
    int HelpTexts,
    double MinimumContrastRatio);

internal enum PresentationAutomationFailureCode
{
    None,
    UnknownTarget,
    UnfocusableTarget,
    TargetUnrealized,
    FocusRejected,
}

internal sealed record PresentationAutomationResult(
    bool Applied,
    string NodeId,
    PresentationAutomationFailureCode FailureCode);

internal sealed class AvaloniaDocumentRenderer(Action<string> actionInvoked)
{
    private readonly Dictionary<string, RenderedNode> nodes = new(StringComparer.Ordinal);
    private readonly Dictionary<ActionKind, ActionAvailability> actionAvailability = [];
    private SemanticRenderer semanticRenderer = new();
    private string? pendingFocusNodeId;

    public ContentControl Surface { get; } = new();
    public UiDocument Document => semanticRenderer.Document;
    public int NodeCount => nodes.Count;
    public int LastAppliedOperationCount { get; private set; }
    public int LastReusedNodeCount { get; private set; }
    public int VirtualizedHostCount => nodes.Values.Count(node => node.UsesVirtualizedHost);
    public int ActiveVirtualizedHostCount => nodes.Values.Count(
        node => node.RealizedChildrenHost?.IsVirtualizationActive == true);
    public int UnrealizedVirtualItemCount => nodes.Values.Sum(
        node => node.RealizedChildrenHost?.UnrealizedCount ?? 0);
    public int UnrealizedNodeCount => nodes.Values.Count(node => !node.IsRealized);
    public int RealizedDebuggerCancelButtonCount => nodes.Values.Count(node =>
        node.ActionKind is ActionKind.DebuggerCancel
        && node.TryGetRealizedControl(out var control)
        && control is Button);
    public int RealizedDisabledActionCount(ActionKind kind) => nodes.Values.Count(node =>
        node.ActionKind == kind
        && node.TryGetRealizedControl(out var control)
        && control is Button { IsEnabled: false });
    public int RealizedActionCount(ActionKind kind) => nodes.Values.Count(node =>
        node.ActionKind == kind
        && node.TryGetRealizedControl(out var control)
        && control is Button);
    public string? FirstRealizedActionNodeId => nodes.Values.FirstOrDefault(node =>
        node.ActionKind is not null
        && node.TryGetRealizedControl(out var control)
        && control is Button)?.Id;
    public string? FocusedNodeId => nodes.Values.FirstOrDefault(node =>
        node.TryGetRealizedControl(out var control)
        && control!.IsFocused)?.Id;
    public bool IsFocusRestorePending => pendingFocusNodeId is not null;

    public bool TryFocusNode(string nodeId) =>
        nodes.TryGetValue(nodeId, out var node)
        && node.TryGetRealizedControl(out var control)
        && control!.Focus();

    public PresentationAutomationResult ApplyPresentation(
        UiPresentationOperation operation)
    {
        var validation = semanticRenderer.ValidatePresentationOperation(operation);
        if (validation != UiPresentationValidation.Valid)
        {
            return new PresentationAutomationResult(
                false,
                operation.NodeId,
                validation == UiPresentationValidation.UnknownTarget
                    ? PresentationAutomationFailureCode.UnknownTarget
                    : PresentationAutomationFailureCode.UnfocusableTarget);
        }
        if (!nodes.TryGetValue(operation.NodeId, out var node)
            || !node.TryGetRealizedControl(out var control))
        {
            return new PresentationAutomationResult(
                false,
                operation.NodeId,
                PresentationAutomationFailureCode.TargetUnrealized);
        }
        if (!control!.Focus())
        {
            return new PresentationAutomationResult(
                false,
                operation.NodeId,
                PresentationAutomationFailureCode.FocusRejected);
        }
        return new PresentationAutomationResult(
            true,
            operation.NodeId,
            PresentationAutomationFailureCode.None);
    }

    public UiEvent CreateFormSubmission(
        string nodeId,
        IReadOnlyDictionary<string, string> values) =>
        semanticRenderer.CreateFormSubmission(nodeId, values);

    public void SetActionAvailability(ActionKind kind, bool enabled, string? unavailableReason)
    {
        if (!enabled && string.IsNullOrWhiteSpace(unavailableReason))
        {
            throw new ArgumentException(
                "disabled actions require an unavailable reason",
                nameof(unavailableReason));
        }
        var availability = new ActionAvailability(enabled, unavailableReason);
        actionAvailability[kind] = availability;
        foreach (var node in nodes.Values)
        {
            if (node.ActionKind == kind
                && node.TryGetRealizedControl(out var control)
                && control is Button button)
            {
                ApplyActionAvailability(
                    button,
                    availability,
                    node.AutomationDescription);
            }
        }
    }

    public AccessibilityAudit AuditAccessibility()
    {
        var automationIds = new HashSet<string>(StringComparer.Ordinal);
        var realized = 0;
        var names = 0;
        var labels = 0;
        var actions = 0;
        var helpTexts = 0;
        foreach (var node in nodes.Values)
        {
            if (!node.TryGetRealizedControl(out var control))
            {
                continue;
            }
            realized++;
            var automationId = AutomationProperties.GetAutomationId(control!);
            if (automationId != node.Id || !automationIds.Add(automationId))
            {
                throw new InvalidDataException("realized control has an invalid or duplicate AutomationId");
            }
            var name = AutomationProperties.GetName(control!);
            if (name != node.AutomationName)
            {
                throw new InvalidDataException($"control '{node.Id}' has an invalid Automation Name");
            }
            names++;
            if (node.HasExplicitLabel)
            {
                labels++;
            }
            var helpText = AutomationProperties.GetHelpText(control!);
            if (helpText != node.AutomationDescription)
            {
                throw new InvalidDataException($"control '{node.Id}' has an invalid Automation HelpText");
            }
            if (node.AutomationDescription is not null)
            {
                helpTexts++;
            }
            if (node.ActionKind is not null)
            {
                if (!node.HasExplicitLabel || control is not Button)
                {
                    throw new InvalidDataException($"action '{node.Id}' is not an explicitly named button");
                }
                actions++;
            }
        }
        var minimumContrast = LeserpentTheme.MinimumTextContrastRatio;
        if (realized == 0 || names != realized || minimumContrast < 4.5)
        {
            throw new InvalidDataException("accessibility audit did not meet the control or contrast floor");
        }
        return new AccessibilityAudit(realized, names, labels, actions, helpTexts, minimumContrast);
    }

    public void Mount(UiDocument document)
    {
        var focusedNodeId = FocusedNodeId;
        pendingFocusNodeId = null;
        var candidate = new SemanticRenderer();
        candidate.Mount(document);
        EnsureRenderable(candidate.Document.Root);

        nodes.Clear();
        var root = BuildSubtree(candidate.Document.Root, null);
        Surface.Content = root.Control;
        semanticRenderer = candidate;
        LastAppliedOperationCount = 0;
        LastReusedNodeCount = 0;
        VerifyIndex();
        ScheduleFocusRestore(focusedNodeId);
    }

    public void Apply(UiPatch patch)
    {
        var focusedNodeId = FocusedNodeId;
        pendingFocusNodeId = null;
        var candidate = new SemanticRenderer();
        candidate.Mount(semanticRenderer.Document);
        candidate.Apply(patch);
        EnsureRenderable(candidate.Document.Root);
        var previousControls = new Dictionary<string, Control>(StringComparer.Ordinal);
        foreach (var pair in nodes)
        {
            if (pair.Value.TryGetRealizedControl(out var control))
            {
                previousControls.Add(pair.Key, control!);
            }
        }

        foreach (var operation in patch.Operations)
        {
            ApplyVisualOperation(operation);
        }
        semanticRenderer = candidate;
        LastAppliedOperationCount = patch.Operations.Count;
        LastReusedNodeCount = previousControls.Count(pair =>
            nodes.TryGetValue(pair.Key, out var node)
            && node.TryGetRealizedControl(out var control)
            && ReferenceEquals(pair.Value, control));
        VerifyIndex();
        ScheduleFocusRestore(focusedNodeId);
    }

    private void ApplyVisualOperation(UiPatchOperation operation)
    {
        switch (operation.Kind)
        {
            case PatchKind.Remove:
                Remove(operation.NodeId!);
                break;
            case PatchKind.Insert:
                Insert(operation.ParentId!, operation.Index!.Value, operation.Node!);
                break;
            case PatchKind.Move:
                Move(operation.NodeId!, operation.ParentId!, operation.Index!.Value);
                break;
            case PatchKind.Update:
                Update(operation.Node!);
                break;
            default:
                throw new InvalidDataException("unknown visual patch operation");
        }
    }

    private void Remove(string nodeId)
    {
        var node = RequireNode(nodeId);
        var parent = node.Parent ?? throw new InvalidDataException("visual root cannot be removed");
        var index = parent.Children.IndexOf(node);
        parent.Children.RemoveAt(index);
        parent.RealizedChildrenHost?.RemoveAt(index);
        Unregister(node);
    }

    private void Insert(string parentId, int index, UiNode node)
    {
        var parent = RequireContainer(parentId);
        var inserted = BuildSubtree(node, parent);
        parent.Children.Insert(index, inserted);
        parent.RealizedChildrenHost?.Insert(index, Hosted(inserted));
    }

    private void Move(string nodeId, string parentId, int index)
    {
        var moving = RequireNode(nodeId);
        var oldParent = moving.Parent
            ?? throw new InvalidDataException("visual root cannot be moved");
        var oldIndex = oldParent.Children.IndexOf(moving);
        oldParent.Children.RemoveAt(oldIndex);
        var hosted = oldParent.RealizedChildrenHost?.RemoveAt(oldIndex);

        var newParent = RequireContainer(parentId);
        moving.Parent = newParent;
        newParent.Children.Insert(index, moving);
        if (newParent.RealizedChildrenHost is { } newHost)
        {
            newHost.Insert(index, hosted ?? Hosted(moving));
        }
    }

    private void Update(UiNode node)
    {
        var previous = RequireNode(node.Id);
        var replacement = CreateShell(node, previous.Parent);
        if (previous.Children.Count > 0 && !replacement.CanContainChildren)
        {
            throw new InvalidDataException("updated visual node cannot contain children");
        }

        previous.RealizedChildrenHost?.Clear();
        foreach (var child in previous.Children)
        {
            child.Parent = replacement;
            replacement.Children.Add(child);
        }

        if (previous.Parent is { } parent)
        {
            var index = parent.Children.IndexOf(previous);
            parent.Children[index] = replacement;
            if (parent.RealizedChildrenHost is { } parentHost)
            {
                parentHost.RemoveAt(index);
                parentHost.Insert(index, Hosted(replacement));
            }
        }
        else
        {
            Surface.Content = replacement.Control;
        }
        nodes[node.Id] = replacement;
    }

    private RenderedNode BuildSubtree(UiNode node, RenderedNode? parent)
    {
        var rendered = CreateShell(node, parent);
        nodes.Add(node.Id, rendered);
        foreach (var child in node.Children)
        {
            var renderedChild = BuildSubtree(child, rendered);
            rendered.Children.Add(renderedChild);
        }
        return rendered;
    }

    private RenderedNode CreateShell(UiNode node, RenderedNode? parent)
    {
        return node.Kind switch
        {
            UiNodeKind.Heading => LazyLeaf(node, parent, () => BuildHeading(node)),
            UiNodeKind.Text or UiNodeKind.HistoryEntry =>
                LazyLeaf(node, parent, () => BuildText(node)),
            UiNodeKind.LogEntry => LazyLeaf(node, parent, () => BuildLogText(node)),
            UiNodeKind.DebuggerFrame =>
                LazyLeaf(node, parent, () => BuildDebuggerFrameText(node)),
            UiNodeKind.Action => LazyLeaf(node, parent, () => BuildAction(node)),
            UiNodeKind.RuntimeCard =>
                LazyContainer(node, parent, false, () => BuildContainer(true, false)),
            UiNodeKind.Section =>
                LazyContainer(node, parent, true, () => BuildContainer(false, true)),
            UiNodeKind.Column => LazyContainer(node, parent, true, () => BuildColumn(true)),
            UiNodeKind.RuntimeWorkspace =>
                LazyContainer(node, parent, false, () => BuildColumn(false)),
            UiNodeKind.DebuggerWorkspace =>
                LazyContainer(node, parent, false, () => BuildColumn(false)),
            _ => throw new InvalidDataException($"unsupported UI node kind: {node.Kind}"),
        };
    }

    private RenderedNode LazyLeaf(
        UiNode node,
        RenderedNode? parent,
        Func<Control> factory) => new(
            node.Id,
            () => (InitializeControl(factory(), node), null),
            parent,
            false,
            false,
            node.Action?.Kind,
            AutomationName(node),
            node.Accessibility.Label is not null,
            node.Accessibility.Description?.Fallback);

    private RenderedNode LazyContainer(
        UiNode node,
        RenderedNode? parent,
        bool virtualized,
        Func<(Control Control, IChildrenHost ChildrenHost)> factory) => new(
            node.Id,
            () =>
            {
                var shell = factory();
                return (InitializeControl(shell.Control, node), shell.ChildrenHost);
            },
            parent,
            true,
            virtualized,
            node.Action?.Kind,
            AutomationName(node),
            node.Accessibility.Label is not null,
            node.Accessibility.Description?.Fallback);

    private static TextBlock BuildHeading(UiNode node) => new()
    {
        Text = RequiredText(node),
        Foreground = LeserpentTheme.Primary,
        FontSize = node.Id.EndsWith("title", StringComparison.Ordinal) ? 30 : 19,
        FontWeight = FontWeight.Bold,
        Margin = new Thickness(0, 0, 0, 6),
        TextWrapping = TextWrapping.Wrap,
    };

    private static TextBlock BuildText(UiNode node) => new()
    {
        Text = RequiredText(node),
        Foreground = node.Kind == UiNodeKind.HistoryEntry
            ? LeserpentTheme.Muted
            : LeserpentTheme.Body,
        FontSize = 14,
        LineHeight = 21,
        TextWrapping = TextWrapping.Wrap,
    };

    private static TextBlock BuildLogText(UiNode node) => new()
    {
        Text = RequiredText(node),
        Foreground = LeserpentTheme.Body,
        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
        FontSize = 13,
        LineHeight = 20,
        TextWrapping = TextWrapping.Wrap,
    };

    private static TextBlock BuildDebuggerFrameText(UiNode node) => new()
    {
        Text = RequiredText(node),
        Foreground = LeserpentTheme.Muted,
        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
        FontSize = 13,
        LineHeight = 20,
        TextWrapping = TextWrapping.Wrap,
    };

    private Button BuildAction(UiNode node)
    {
        var destructive = node.Action?.Kind is ActionKind.DebuggerCancel;
        var button = new Button
        {
            Content = RequiredText(node),
            Background = destructive ? LeserpentTheme.Destructive : LeserpentTheme.Accent,
            Foreground = destructive ? Brushes.White : Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(18, 9),
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left,
            CornerRadius = new CornerRadius(8),
        };
        button.Click += (_, _) => actionInvoked(node.Id);
        return button;
    }

    private void ApplyActionAvailability(Button button, UiNode node)
    {
        var availability = node.Action is { } action
            && actionAvailability.TryGetValue(action.Kind, out var configured)
                ? configured
                : ActionAvailability.Enabled;
        ApplyActionAvailability(
            button,
            availability,
            node.Accessibility.Description?.Fallback);
    }

    private static void ApplyActionAvailability(
        Button button,
        ActionAvailability availability,
        string? defaultDescription)
    {
        button.IsEnabled = availability.IsEnabled;
        var description = availability.IsEnabled
            ? defaultDescription
            : availability.UnavailableReason;
        AutomationProperties.SetHelpText(button, description);
        ToolTip.SetTip(button, availability.IsEnabled ? null : description);
    }

    private static (Control, IChildrenHost) BuildContainer(bool emphasized, bool virtualized)
    {
        var children = BuildChildrenHost(virtualized);
        if (virtualized)
        {
            children.Control.MaxHeight = 360;
        }
        return (new Border
        {
            Background = emphasized ? LeserpentTheme.Panel : Brushes.Transparent,
            BorderBrush = emphasized ? LeserpentTheme.PanelBorder : Brush.Parse("#332B1E"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(14),
            Padding = new Thickness(20),
            Margin = new Thickness(0, 4, 0, 10),
            Child = children.Control,
        }, children);
    }

    private static (Control, IChildrenHost) BuildColumn(bool virtualized)
    {
        var children = BuildChildrenHost(virtualized);
        return (children.Control, children);
    }

    private static IChildrenHost BuildChildrenHost(bool virtualized) => virtualized
        ? new VirtualizedChildrenHost()
        : new StackChildrenHost();

    private static void ApplyAutomation(Control control, UiNode node)
    {
        control.Tag = node.Id;
        AutomationProperties.SetAutomationId(control, node.Id);
        AutomationProperties.SetName(
            control,
            node.Accessibility.Label?.Fallback ?? node.Text?.Fallback ?? node.Id);
        if (node.Accessibility.Description is { } description)
        {
            AutomationProperties.SetHelpText(control, description.Fallback);
        }
    }

    private Control InitializeControl(Control control, UiNode node)
    {
        ApplyAutomation(control, node);
        if (control is Button button && node.Action is not null)
        {
            ApplyActionAvailability(button, node);
        }
        if (pendingFocusNodeId == node.Id)
        {
            Dispatcher.UIThread.Post(
                () => RestorePendingFocus(node.Id),
                DispatcherPriority.Loaded);
        }
        return control;
    }

    private void ScheduleFocusRestore(string? nodeId)
    {
        pendingFocusNodeId = nodeId is not null && nodes.ContainsKey(nodeId)
            ? nodeId
            : null;
        if (pendingFocusNodeId is not null
            && nodes[pendingFocusNodeId].TryGetRealizedControl(out var control))
        {
            if (control!.IsFocused)
            {
                pendingFocusNodeId = null;
                return;
            }
            Dispatcher.UIThread.Post(
                () => RestorePendingFocus(pendingFocusNodeId),
                DispatcherPriority.Loaded);
        }
    }

    private void RestorePendingFocus(string? nodeId)
    {
        if (nodeId is null
            || pendingFocusNodeId != nodeId
            || !nodes.TryGetValue(nodeId, out var node)
            || !node.TryGetRealizedControl(out var control))
        {
            return;
        }
        if (control!.Focus())
        {
            pendingFocusNodeId = null;
        }
    }

    private static string AutomationName(UiNode node) =>
        node.Accessibility.Label?.Fallback ?? node.Text?.Fallback ?? node.Id;

    private static void EnsureRenderable(UiNode node)
    {
        if (node.Kind is UiNodeKind.Heading or UiNodeKind.Text or UiNodeKind.HistoryEntry
            or UiNodeKind.LogEntry or UiNodeKind.DebuggerFrame or UiNodeKind.Action
            && node.Children.Count > 0)
        {
            throw new InvalidDataException($"leaf UI node '{node.Id}' contains children");
        }
        foreach (var child in node.Children)
        {
            EnsureRenderable(child);
        }
    }

    private void VerifyIndex()
    {
        var visited = new HashSet<string>(StringComparer.Ordinal);
        var root = RequireNode(Document.Root.Id);
        if (!ReferenceEquals(Surface.Content, root.Control))
        {
            throw new InvalidDataException("visual surface does not contain the UI IR root");
        }
        VerifyNode(Document.Root, null, visited);
        if (visited.Count != nodes.Count)
        {
            throw new InvalidDataException("visual node index contains stale entries");
        }
    }

    private void VerifyNode(UiNode semantic, RenderedNode? parent, HashSet<string> visited)
    {
        var rendered = RequireNode(semantic.Id);
        if (!ReferenceEquals(rendered.Parent, parent)
            || rendered.Children.Count != semantic.Children.Count
            || (!rendered.CanContainChildren && semantic.Children.Count != 0)
            || (rendered.RealizedChildrenHost is { } host
                && host.Count != semantic.Children.Count)
            || !visited.Add(semantic.Id))
        {
            throw new InvalidDataException("visual node index diverged from UI IR");
        }
        for (var index = 0; index < semantic.Children.Count; index++)
        {
            if (rendered.Children[index].Id != semantic.Children[index].Id)
            {
                throw new InvalidDataException("visual child order diverged from UI IR");
            }
            if (rendered.RealizedChildrenHost is { } realizedHost)
            {
                var hosted = realizedHost[index];
                if (hosted.NodeId != rendered.Children[index].Id)
                {
                    throw new InvalidDataException("visual child item diverged from its index");
                }
                if (hosted.TryGetContent(out var content)
                    && (!rendered.Children[index].TryGetRealizedControl(out var control)
                        || !ReferenceEquals(content, control)))
                {
                    throw new InvalidDataException("realized child control diverged from its index");
                }
            }
            VerifyNode(semantic.Children[index], rendered, visited);
        }
    }

    private RenderedNode RequireNode(string nodeId) => nodes.TryGetValue(nodeId, out var node)
        ? node
        : throw new InvalidDataException($"visual node '{nodeId}' was not found");

    private RenderedNode RequireContainer(string nodeId)
    {
        var node = RequireNode(nodeId);
        return node.CanContainChildren
            ? node
            : throw new InvalidDataException($"visual parent '{nodeId}' cannot contain children");
    }

    private void Unregister(RenderedNode node)
    {
        foreach (var child in node.Children)
        {
            Unregister(child);
        }
        nodes.Remove(node.Id);
    }

    private static string RequiredText(UiNode node) => node.Text?.Fallback
        ?? node.Accessibility.Label?.Fallback
        ?? throw new InvalidDataException($"node '{node.Id}' has no display text");

    private sealed record ActionAvailability(bool IsEnabled, string? UnavailableReason)
    {
        public static ActionAvailability Enabled { get; } = new(true, null);
    }

    private static VirtualizedItemViewModel Hosted(RenderedNode node) => new(
        node.Id,
        () => node.Control);

    private sealed class RenderedNode(
        string id,
        Func<(Control Control, IChildrenHost? ChildrenHost)> shellFactory,
        RenderedNode? parent,
        bool canContainChildren,
        bool usesVirtualizedHost,
        ActionKind? actionKind,
        string automationName,
        bool hasExplicitLabel,
        string? automationDescription)
    {
        private Control? control;
        private IChildrenHost? childrenHost;

        public string Id { get; } = id;
        public Control Control
        {
            get
            {
                if (control is null)
                {
                    var shell = shellFactory();
                    control = shell.Control;
                    childrenHost = shell.ChildrenHost;
                    if (childrenHost is { } host)
                    {
                        foreach (var child in Children)
                        {
                            host.Add(Hosted(child));
                        }
                    }
                }
                return control;
            }
        }
        public IChildrenHost? RealizedChildrenHost => childrenHost;
        public bool CanContainChildren { get; } = canContainChildren;
        public bool UsesVirtualizedHost { get; } = usesVirtualizedHost;
        public ActionKind? ActionKind { get; } = actionKind;
        public string AutomationName { get; } = automationName;
        public bool HasExplicitLabel { get; } = hasExplicitLabel;
        public string? AutomationDescription { get; } = automationDescription;
        public bool IsRealized => control is not null;
        public RenderedNode? Parent { get; set; } = parent;
        public List<RenderedNode> Children { get; } = [];

        public bool TryGetRealizedControl(out Control? realized)
        {
            realized = control;
            return realized is not null;
        }
    }

    private interface IChildrenHost
    {
        Control Control { get; }
        int Count { get; }
        int UnrealizedCount { get; }
        bool IsVirtualized { get; }
        bool IsVirtualizationActive { get; }
        VirtualizedItemViewModel this[int index] { get; }
        void Add(VirtualizedItemViewModel item);
        void Insert(int index, VirtualizedItemViewModel item);
        VirtualizedItemViewModel RemoveAt(int index);
        void Clear();
    }

    private sealed class StackChildrenHost : IChildrenHost
    {
        private readonly StackPanel panel = new() { Spacing = 10 };
        private readonly List<VirtualizedItemViewModel> items = [];

        public Control Control => panel;
        public int Count => items.Count;
        public int UnrealizedCount => 0;
        public bool IsVirtualized => false;
        public bool IsVirtualizationActive => false;
        public VirtualizedItemViewModel this[int index] => items[index];

        public void Add(VirtualizedItemViewModel item)
        {
            items.Add(item);
            panel.Children.Add(item.Content);
        }

        public void Insert(int index, VirtualizedItemViewModel item)
        {
            items.Insert(index, item);
            panel.Children.Insert(index, item.Content);
        }

        public VirtualizedItemViewModel RemoveAt(int index)
        {
            var item = items[index];
            items.RemoveAt(index);
            panel.Children.RemoveAt(index);
            return item;
        }

        public void Clear()
        {
            items.Clear();
            panel.Children.Clear();
        }
    }

    private sealed class VirtualizedChildrenHost : IChildrenHost
    {
        private readonly ObservableCollection<VirtualizedItemViewModel> items = [];
        private readonly ListBox list;

        public VirtualizedChildrenHost()
        {
            list = new ListBox
            {
                ItemsSource = items,
                ItemTemplate = new FuncDataTemplate<VirtualizedItemViewModel>(
                    (item, _) => new VirtualizedItemView { DataContext = item }),
                ItemsPanel = new FuncTemplate<Panel?>(() => new VirtualizingStackPanel
                {
                    CacheLength = 0.75,
                }),
                Background = Brushes.Transparent,
                BorderThickness = new Thickness(0),
            };
        }

        public Control Control => list;
        public int Count => items.Count;
        public int UnrealizedCount => items.Count(item => !item.IsRealized);
        public bool IsVirtualized => true;
        public bool IsVirtualizationActive => list.ItemsPanelRoot is VirtualizingStackPanel;
        public VirtualizedItemViewModel this[int index] => items[index];
        public void Add(VirtualizedItemViewModel item) => items.Add(item);
        public void Insert(int index, VirtualizedItemViewModel item) => items.Insert(index, item);

        public VirtualizedItemViewModel RemoveAt(int index)
        {
            var item = items[index];
            items.RemoveAt(index);
            return item;
        }

        public void Clear() => items.Clear();
    }
}
