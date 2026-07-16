using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Controls.Primitives;
using Avalonia.Media;

internal sealed class MainWindow : Window
{
    private static readonly IBrush CanvasBrush = Brush.Parse("#11100D");
    private static readonly IBrush PanelBrush = Brush.Parse("#1C1913");
    private static readonly IBrush PanelBorderBrush = Brush.Parse("#514224");
    private static readonly IBrush PrimaryBrush = Brush.Parse("#F4C95D");
    private static readonly IBrush AccentBrush = Brush.Parse("#FF9418");
    private static readonly IBrush BodyBrush = Brush.Parse("#E9E1D0");
    private static readonly IBrush MutedBrush = Brush.Parse("#B9AA8A");

    private readonly TextBlock statusText = new()
    {
        Foreground = MutedBrush,
        FontSize = 13,
        Text = "No action selected",
    };

    public int RenderedNodeCount { get; }

    public MainWindow(UiDocument document)
    {
        Title = $"Leserpent / revision {document.Revision}";
        Width = 1080;
        Height = 760;
        MinWidth = 640;
        MinHeight = 480;
        Background = CanvasBrush;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        var renderer = new AvaloniaControlRenderer(OnActionInvoked);
        var root = renderer.Render(document.Root);
        RenderedNodeCount = renderer.RenderedNodeCount;
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto"),
            Children =
            {
                new ScrollViewer
                {
                    Content = root,
                    Padding = new Thickness(32, 28),
                    HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
                },
                BuildStatusBar(document.Revision),
            },
        };
    }

    private Border BuildStatusBar(ulong revision)
    {
        var revisionText = new TextBlock
        {
            Foreground = PrimaryBrush,
            FontSize = 12,
            FontWeight = FontWeight.SemiBold,
            Text = $"UI IR v1  /  rev {revision}",
        };
        var bar = new Border
        {
            Background = PanelBrush,
            BorderBrush = PanelBorderBrush,
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
        statusText.Foreground = AccentBrush;
    }

    private sealed class AvaloniaControlRenderer(Action<string> actionInvoked)
    {
        public int RenderedNodeCount { get; private set; }

        public Control Render(UiNode node)
        {
            var control = node.Kind switch
            {
                UiNodeKind.Heading => BuildHeading(node),
                UiNodeKind.Text or UiNodeKind.HistoryEntry => BuildText(node),
                UiNodeKind.Action => BuildAction(node),
                UiNodeKind.RuntimeCard => BuildContainer(node, true),
                UiNodeKind.Section => BuildContainer(node, false),
                UiNodeKind.Column or UiNodeKind.RuntimeWorkspace => BuildColumn(node),
                _ => throw new InvalidDataException($"unsupported UI node kind: {node.Kind}"),
            };
            control.Tag = node.Id;
            AutomationProperties.SetAutomationId(control, node.Id);
            AutomationProperties.SetName(
                control,
                node.Accessibility.Label?.Fallback ?? node.Text?.Fallback ?? node.Id);
            if (node.Accessibility.Description is { } description)
            {
                AutomationProperties.SetHelpText(control, description.Fallback);
            }
            RenderedNodeCount++;
            return control;
        }

        private TextBlock BuildHeading(UiNode node) => new()
        {
            Text = RequiredText(node),
            Foreground = PrimaryBrush,
            FontSize = node.Id.EndsWith("title", StringComparison.Ordinal) ? 30 : 19,
            FontWeight = FontWeight.Bold,
            Margin = new Thickness(0, 0, 0, 6),
            TextWrapping = TextWrapping.Wrap,
        };

        private static TextBlock BuildText(UiNode node) => new()
        {
            Text = RequiredText(node),
            Foreground = node.Kind == UiNodeKind.HistoryEntry ? MutedBrush : BodyBrush,
            FontSize = 14,
            LineHeight = 21,
            TextWrapping = TextWrapping.Wrap,
        };

        private Button BuildAction(UiNode node)
        {
            var button = new Button
            {
                Content = RequiredText(node),
                Background = AccentBrush,
                Foreground = Brushes.Black,
                FontWeight = FontWeight.SemiBold,
                Padding = new Thickness(18, 9),
                HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left,
                CornerRadius = new CornerRadius(8),
            };
            button.Click += (_, _) => actionInvoked(node.Id);
            return button;
        }

        private Border BuildContainer(UiNode node, bool emphasized) => new()
        {
            Background = emphasized ? PanelBrush : Brushes.Transparent,
            BorderBrush = emphasized ? PanelBorderBrush : Brush.Parse("#332B1E"),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(14),
            Padding = new Thickness(20),
            Margin = new Thickness(0, 4, 0, 10),
            Child = BuildChildren(node),
        };

        private Control BuildColumn(UiNode node) => BuildChildren(node);

        private StackPanel BuildChildren(UiNode node)
        {
            var panel = new StackPanel { Spacing = 10 };
            foreach (var child in node.Children)
            {
                panel.Children.Add(Render(child));
            }
            return panel;
        }

        private static string RequiredText(UiNode node) => node.Text?.Fallback
            ?? node.Accessibility.Label?.Fallback
            ?? throw new InvalidDataException($"node '{node.Id}' has no display text");
    }
}
