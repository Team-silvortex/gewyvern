using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class RemoteRuntimeWorkspaceWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly RemoteWorkspaceClient client;
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button reloadButton = new()
    {
        Content = "Reload",
        Padding = new Thickness(12, 6),
    };
    private readonly TextBox logSearchBox = new()
    {
        PlaceholderText = "Search sanitized logs",
        MaxLength = RemoteWorkspaceLogFilter.MaxQueryLength,
        MinWidth = 180,
    };
    private readonly ComboBox logLevelBox = new()
    {
        ItemsSource = RemoteWorkspaceLogFilter.Levels,
        SelectedIndex = 0,
        MinWidth = 112,
    };
    private readonly Button clearLogFilterButton = new()
    {
        Content = "Clear",
        Padding = new Thickness(12, 6),
        IsVisible = false,
    };
    private readonly TextBlock logFilterSummary = new()
    {
        FontSize = 12,
        Foreground = LeserpentTheme.Muted,
        Text = "Logs load with the runtime snapshot",
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly DispatcherTimer logFilterTimer = new()
    {
        Interval = TimeSpan.FromMilliseconds(120),
    };
    private bool loadInFlight;
    private ulong loadedRevision;
    private ulong desiredRevision;
    private RemoteWorkspaceSnapshot? latestSnapshot;

    public RemoteRuntimeWorkspaceWindow(
        RemoteClientOptions options,
        RemoteRuntimeProjection runtime,
        string principal,
        Action<string> actionInvoked)
    {
        RuntimeId = runtime.Id;
        this.principal = principal;
        client = new RemoteWorkspaceClient(options);
        renderer = new AvaloniaDocumentRenderer(actionInvoked);
        Width = 760;
        Height = 620;
        MinWidth = 520;
        MinHeight = 420;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        Title = $"{Safe(runtime.Name)} / Leserpent";
        AutomationProperties.SetAutomationId(statusText, "runtime-workspace-status");
        AutomationProperties.SetName(statusText, "Runtime workspace query status");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(reloadButton, "runtime-workspace-reload");
        AutomationProperties.SetName(reloadButton, "Reload runtime workspace");
        AutomationProperties.SetHelpText(
            reloadButton,
            "Reloads status, history, and logs through one revision-consistent query group. Shortcut: F5.");
        reloadButton.Click += (_, _) => _ = ReloadAsync();
        ConfigureLogFilter();
        var status = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(20, 10),
            Child = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
                ColumnSpacing = 12,
                Children = { statusText, reloadButton },
            },
        };
        Grid.SetColumn(reloadButton, 1);
        var filterControls = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto,Auto"),
            ColumnSpacing = 8,
            Children = { logSearchBox, logLevelBox, clearLogFilterButton },
        };
        Grid.SetColumn(logLevelBox, 1);
        Grid.SetColumn(clearLogFilterButton, 2);
        var logFilter = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(20, 10),
            Child = new Grid
            {
                RowDefinitions = RowDefinitions.Parse("Auto,Auto"),
                RowSpacing = 6,
                Children = { filterControls, logFilterSummary },
            },
        };
        Grid.SetRow(logFilterSummary, 1);
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,*"),
            Children = { status, logFilter, renderer.Surface },
        };
        Grid.SetRow(logFilter, 1);
        Grid.SetRow(renderer.Surface, 2);
        Opened += (_, _) => _ = ReloadAsync();
        KeyDown += (_, eventArgs) =>
        {
            var findModifier = eventArgs.KeyModifiers
                & (KeyModifiers.Control | KeyModifiers.Meta);
            if (eventArgs.Key == Key.F && findModifier != KeyModifiers.None)
            {
                eventArgs.Handled = true;
                logSearchBox.Focus();
                logSearchBox.SelectAll();
            }
            else if (eventArgs.Key == Key.Escape
                && !string.IsNullOrEmpty(logSearchBox.Text))
            {
                eventArgs.Handled = true;
                ClearLogFilter();
            }
            else if (eventArgs.Key == Key.F5 && reloadButton.IsEnabled)
            {
                eventArgs.Handled = true;
                _ = ReloadAsync();
            }
        };
        Closed += (_, _) =>
        {
            logFilterTimer.Stop();
            lifetime.Cancel();
            client.Dispose();
            lifetime.Dispose();
        };
    }

    private void ConfigureLogFilter()
    {
        AutomationProperties.SetAutomationId(logSearchBox, "runtime-log-search");
        AutomationProperties.SetName(logSearchBox, "Search runtime logs");
        AutomationProperties.SetHelpText(
            logSearchBox,
            "Filters the loaded sanitized log display locally. Shortcut: Control or Command plus F.");
        AutomationProperties.SetAutomationId(logLevelBox, "runtime-log-level");
        AutomationProperties.SetName(logLevelBox, "Runtime log level filter");
        AutomationProperties.SetAutomationId(clearLogFilterButton, "runtime-log-filter-clear");
        AutomationProperties.SetName(clearLogFilterButton, "Clear runtime log filters");
        AutomationProperties.SetAutomationId(logFilterSummary, "runtime-log-filter-summary");
        AutomationProperties.SetName(logFilterSummary, logFilterSummary.Text);
        AutomationProperties.SetLiveSetting(logFilterSummary, AutomationLiveSetting.Polite);
        logSearchBox.TextChanged += (_, _) =>
        {
            var raw = logSearchBox.Text ?? string.Empty;
            var sanitized = new string(raw
                .Where(character => !char.IsControl(character))
                .Take(RemoteWorkspaceLogFilter.MaxQueryLength)
                .ToArray());
            if (!string.Equals(raw, sanitized, StringComparison.Ordinal))
            {
                logSearchBox.Text = sanitized;
                logSearchBox.CaretIndex = sanitized.Length;
                return;
            }
            QueueLogFilter();
        };
        logLevelBox.SelectionChanged += (_, _) => QueueLogFilter();
        clearLogFilterButton.Click += (_, _) => ClearLogFilter();
        logFilterTimer.Tick += (_, _) =>
        {
            logFilterTimer.Stop();
            ApplyLogFilter();
        };
    }

    private void QueueLogFilter()
    {
        clearLogFilterButton.IsVisible =
            !string.IsNullOrWhiteSpace(logSearchBox.Text)
            || logLevelBox.SelectedIndex > 0;
        logFilterTimer.Stop();
        logFilterTimer.Start();
    }

    private void ClearLogFilter()
    {
        logFilterTimer.Stop();
        logSearchBox.Text = string.Empty;
        logLevelBox.SelectedIndex = 0;
        logFilterTimer.Stop();
        clearLogFilterButton.IsVisible = false;
        ApplyLogFilter();
        logSearchBox.Focus();
    }

    private void ApplyLogFilter()
    {
        if (latestSnapshot is null)
        {
            return;
        }
        var view = RemoteWorkspaceLogFilter.Apply(
            latestSnapshot,
            logSearchBox.Text,
            logLevelBox.SelectedItem as string);
        renderer.Mount(RemoteWorkspaceDocumentProjection.Project(
            view.Snapshot,
            view.IsActive));
        logFilterSummary.Text = view.IsActive
            ? $"Showing {view.VisibleLogCount} of {view.TotalLogCount} logs"
            : $"Showing all {view.TotalLogCount} logs";
        AutomationProperties.SetName(
            logFilterSummary,
            $"Runtime log filter: {logFilterSummary.Text}");
    }

    public string RuntimeId { get; }

    public void SetRefreshAvailability(bool enabled, string? reason)
    {
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, enabled, reason);
        renderer.SetActionAvailability(
            ActionKind.RuntimeCapabilitiesRefresh,
            enabled,
            reason);
        renderer.SetActionAvailability(ActionKind.RuntimeDeploy, enabled, reason);
    }

    public void ReloadIfOlder(ulong revision)
    {
        desiredRevision = Math.Max(desiredRevision, revision);
        if (revision > loadedRevision && !loadInFlight)
        {
            _ = ReloadAsync();
        }
    }

    private async Task ReloadAsync()
    {
        if (loadInFlight || lifetime.IsCancellationRequested)
        {
            return;
        }
        loadInFlight = true;
        var loaded = false;
        reloadButton.IsEnabled = false;
        SetStatus("Loading authenticated runtime snapshot...", LeserpentTheme.Primary);
        try
        {
            var snapshot = await client.LoadAsync(RuntimeId, principal, lifetime.Token);
            latestSnapshot = snapshot;
            ApplyLogFilter();
            loadedRevision = snapshot.Revision;
            loaded = true;
            SetStatus(
                $"Live workspace at revision {snapshot.Revision}",
                LeserpentTheme.Accent);
        }
        catch (RemoteQueryException error)
        {
            ShowFailure($"Query rejected ({Safe(error.Code)}): {Safe(error.Message)}");
        }
        catch (InvalidDataException error)
        {
            ShowFailure($"Query response rejected: {Safe(error.Message)}");
        }
        catch (ArgumentException error)
        {
            ShowFailure($"Query blocked: {Safe(error.Message)}");
        }
        catch (OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            ShowFailure("Query timed out; no partial workspace was retained");
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            // Window shutdown owns this cancellation.
        }
        catch (HttpRequestException)
        {
            ShowFailure("Query failed over the authenticated HTTPS connection");
        }
        catch (ObjectDisposedException) when (lifetime.IsCancellationRequested)
        {
            // The HTTP client may be disposed while the window is closing.
        }
        finally
        {
            loadInFlight = false;
            reloadButton.IsEnabled = !lifetime.IsCancellationRequested;
            if (loaded && desiredRevision > loadedRevision)
            {
                _ = ReloadAsync();
            }
        }
    }

    private void ShowFailure(string message)
    {
        SetStatus(message, LeserpentTheme.Destructive);
    }

    private void SetStatus(string text, IBrush foreground)
    {
        statusText.Text = text;
        statusText.Foreground = foreground;
        AutomationProperties.SetName(statusText, $"Runtime workspace query status: {text}");
        AutomationProperties.SetLiveSetting(
            statusText,
            ReferenceEquals(foreground, LeserpentTheme.Destructive)
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
    }

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
