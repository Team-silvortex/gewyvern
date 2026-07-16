using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class RemoteMainWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly RemoteEventClient client;
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
    };
    private readonly TextBlock revisionText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 12,
        FontWeight = FontWeight.SemiBold,
    };

    public RemoteMainWindow(RemoteClientOptions options)
    {
        Width = 1080;
        Height = 760;
        MinWidth = 640;
        MinHeight = 480;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        Title = $"Leserpent / {options.Endpoint.Host}";

        renderer = new AvaloniaDocumentRenderer(_ => { });
        client = new RemoteEventClient(options);
        renderer.Mount(RemoteDocumentProjection.Project(client.State));
        ApplyState(client.State);
        client.StateChanged += OnStateChanged;

        AutomationProperties.SetAutomationId(statusText, "remote-connection-state");
        AutomationProperties.SetName(statusText, "Remote connection state");
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
        Opened += (_, _) => client.Start();
        Closed += async (_, _) => await client.DisposeAsync();
    }

    private Border BuildStatusBar()
    {
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

    private void OnStateChanged(RemoteFeedState state) =>
        Dispatcher.UIThread.Post(() => ApplyState(state));

    private void ApplyState(RemoteFeedState state)
    {
        renderer.Mount(RemoteDocumentProjection.Project(state));
        statusText.Text = state.IsStale ? $"STALE / {state.Detail}" : state.Detail;
        statusText.Foreground = state.Phase switch
        {
            RemoteFeedPhase.Live => LeserpentTheme.Accent,
            RemoteFeedPhase.Stale => LeserpentTheme.Destructive,
            RemoteFeedPhase.Reconnecting => LeserpentTheme.Primary,
            _ => LeserpentTheme.Muted,
        };
        revisionText.Text = state.Revision is { } revision
            ? $"EVENTS v1  /  rev {revision}"
            : "EVENTS v1  /  awaiting snapshot";
    }
}
