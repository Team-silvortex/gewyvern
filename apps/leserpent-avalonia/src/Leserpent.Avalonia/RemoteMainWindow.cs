using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class RemoteMainWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly RemoteEventClient eventClient;
    private readonly RemoteMutationClient mutationClient;
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private RemoteFeedState currentState;
    private bool mutationInFlight;
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

        renderer = new AvaloniaDocumentRenderer(OnActionInvoked);
        eventClient = new RemoteEventClient(options);
        mutationClient = new RemoteMutationClient(options);
        principal = Environment.GetEnvironmentVariable("LESERPENT_PRINCIPAL")
            ?? "avalonia-remote";
        currentState = eventClient.State;
        renderer.Mount(RemoteDocumentProjection.Project(currentState));
        ApplyState(currentState);
        eventClient.StateChanged += OnStateChanged;

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
        Opened += (_, _) => eventClient.Start();
        Closed += OnClosed;
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
        currentState = state;
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

    private async void OnActionInvoked(string nodeId)
    {
        if (mutationInFlight)
        {
            SetMutationStatus("A remote change is already in progress", LeserpentTheme.Primary);
            return;
        }
        if (currentState.Phase != RemoteFeedPhase.Live || currentState.IsStale)
        {
            SetMutationStatus("Refresh blocked: remote state is not live", LeserpentTheme.Destructive);
            return;
        }
        var runtime = currentState.Runtimes.FirstOrDefault(candidate =>
            nodeId == $"runtime:{candidate.Id}:refresh");
        if (runtime is null)
        {
            SetMutationStatus("Refresh blocked: action context is invalid", LeserpentTheme.Destructive);
            return;
        }
        mutationInFlight = true;
        var confirmed = await new RuntimeRefreshConfirmationWindow(runtime)
            .ShowDialog<bool>(this);
        if (!confirmed || lifetime.IsCancellationRequested)
        {
            mutationInFlight = false;
            return;
        }
        var confirmedRuntime = currentState.Runtimes.FirstOrDefault(candidate =>
            candidate.Id == runtime.Id);
        if (currentState.Phase != RemoteFeedPhase.Live
            || currentState.IsStale
            || confirmedRuntime?.Revision != runtime.Revision)
        {
            mutationInFlight = false;
            SetMutationStatus(
                "Refresh blocked: remote state changed during confirmation",
                LeserpentTheme.Destructive);
            return;
        }
        SetMutationStatus(
            $"Refreshing {SafeDisplay(runtime.Name)} at revision {runtime.Revision}...",
            LeserpentTheme.Primary);
        try
        {
            var result = await mutationClient.RefreshAsync(
                runtime.Id,
                runtime.Revision,
                principal,
                lifetime.Token);
            SetMutationStatus(
                $"Refresh applied to {SafeDisplay(runtime.Name)} at revision {result.Revision}",
                LeserpentTheme.Accent);
        }
        catch (RemoteMutationException error)
        {
            SetMutationStatus(
                $"Refresh rejected ({SafeDisplay(error.Code)}): {SafeDisplay(error.Message)}",
                LeserpentTheme.Destructive);
        }
        catch (InvalidDataException error)
        {
            SetMutationStatus(
                $"Refresh response rejected: {SafeDisplay(error.Message)}",
                LeserpentTheme.Destructive);
        }
        catch (ArgumentException error)
        {
            SetMutationStatus(
                $"Refresh blocked: {SafeDisplay(error.Message)}",
                LeserpentTheme.Destructive);
        }
        catch (OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            SetMutationStatus(
                "Refresh outcome unknown after timeout; wait for the event stream before retrying",
                LeserpentTheme.Destructive);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            // Window shutdown owns this cancellation.
        }
        catch (ObjectDisposedException) when (lifetime.IsCancellationRequested)
        {
            // The HTTP client may be disposed while the window is closing.
        }
        catch (HttpRequestException)
        {
            SetMutationStatus(
                "Refresh outcome unknown after a network failure; wait for the event stream before retrying",
                LeserpentTheme.Destructive);
        }
        finally
        {
            mutationInFlight = false;
        }
    }

    private async void OnClosed(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        lifetime.Cancel();
        mutationClient.Dispose();
        await eventClient.DisposeAsync();
        lifetime.Dispose();
    }

    private void SetMutationStatus(string text, IBrush foreground)
    {
        statusText.Text = text;
        statusText.Foreground = foreground;
    }

    private static string SafeDisplay(string value)
    {
        var sanitized = new string(value
            .Where(character => !char.IsControl(character))
            .Take(256)
            .ToArray());
        return string.IsNullOrWhiteSpace(sanitized) ? "Unavailable" : sanitized;
    }
}

internal sealed class RuntimeRefreshConfirmationWindow : Window
{
    public RuntimeRefreshConfirmationWindow(RemoteRuntimeProjection runtime)
    {
        Title = "Confirm remote refresh";
        Width = 480;
        MinWidth = 420;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        var cancel = new Button
        {
            Content = "Cancel",
            Padding = new Thickness(18, 9),
        };
        var confirm = new Button
        {
            Content = "Refresh runtime",
            Background = LeserpentTheme.Accent,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(18, 9),
        };
        AutomationProperties.SetAutomationId(cancel, "runtime-refresh-cancel");
        AutomationProperties.SetName(cancel, "Cancel runtime refresh");
        AutomationProperties.SetAutomationId(confirm, "runtime-refresh-confirm");
        AutomationProperties.SetName(confirm, "Confirm runtime refresh");
        cancel.Click += (_, _) => Close(false);
        confirm.Click += (_, _) => Close(true);

        var buttons = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            Spacing = 12,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Children = { cancel, confirm },
        };
        Content = new Border
        {
            Padding = new Thickness(28),
            Child = new StackPanel
            {
                Spacing = 14,
                Children =
                {
                    new TextBlock
                    {
                        Text = "Refresh this remote runtime?",
                        Foreground = LeserpentTheme.Primary,
                        FontSize = 22,
                        FontWeight = FontWeight.Bold,
                    },
                    new TextBlock
                    {
                        Text = $"{Safe(runtime.Name)}\nID: {runtime.Id}\nExpected revision: {runtime.Revision}",
                        Foreground = LeserpentTheme.Body,
                        FontSize = 14,
                        LineHeight = 22,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new TextBlock
                    {
                        Text = "This changes remote state. The request is revision-checked and is not retried automatically.",
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 13,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    buttons,
                },
            },
        };
    }

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
