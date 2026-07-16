using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class RemoteMainWindow : Window
{
    private const int MaxOpenWorkspaces = 8;
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly RemoteEventClient eventClient;
    private readonly RemoteMutationClient mutationClient;
    private readonly RemoteClientOptions options;
    private readonly Dictionary<string, RemoteRuntimeWorkspaceWindow> workspaceWindows =
        new(StringComparer.Ordinal);
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private RemoteFeedState currentState;
    private bool mutationInFlight;
    private long feedStateSerial;
    private long? mutationObservationFence;
    private MutationRevisionFence? mutationRevisionFence;
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
    private readonly TextBlock mutationStatusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button dismissMutationButton = new()
    {
        Content = "Dismiss",
        Padding = new Thickness(12, 6),
    };
    private readonly Border mutationStatusBar = new()
    {
        Background = LeserpentTheme.Panel,
        BorderBrush = LeserpentTheme.PanelBorder,
        BorderThickness = new Thickness(0, 1, 0, 0),
        IsVisible = false,
        Padding = new Thickness(24, 10),
    };
    private readonly Button reconnectButton = new()
    {
        Content = "Reconnect",
        IsEnabled = false,
        Padding = new Thickness(14, 7),
    };
    private readonly TextBlock credentialSourceText = new()
    {
        FontSize = 11,
        FontWeight = FontWeight.Bold,
    };
    private readonly Border credentialSourceBadge = new()
    {
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(6),
        Padding = new Thickness(9, 5),
    };
    private readonly TextBox runtimeFilterBox = new()
    {
        MaxLength = RemoteDocumentProjection.MaxFilterLength,
        PlaceholderText = "Filter runtimes by name, ID, tag, or status",
    };
    private readonly TextBlock remoteOriginText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextTrimming = TextTrimming.CharacterEllipsis,
    };
    private readonly TextBlock caFingerprintText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
        FontSize = 11,
        FontWeight = FontWeight.SemiBold,
    };
    private readonly TextBlock runtimeCountText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 12,
        FontWeight = FontWeight.SemiBold,
        VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center,
    };
    private readonly Button clearRuntimeFilterButton = new()
    {
        Content = "Clear",
        IsVisible = false,
        Padding = new Thickness(12, 7),
    };
    private readonly DispatcherTimer runtimeFilterTimer;
    private readonly Border remoteBodyBorder = new();
    private readonly Grid identityGrid = new();
    private readonly Grid runtimeToolbarGrid = new();
    private readonly Grid statusGrid = new();

    public RemoteMainWindow(RemoteClientOptions options, RemoteTokenSource tokenSource)
    {
        this.options = options;
        Width = 1080;
        Height = 760;
        MinWidth = 640;
        MinHeight = 480;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        Title = $"Leserpent / {options.Endpoint.Authority}";

        renderer = new AvaloniaDocumentRenderer(OnActionInvoked);
        eventClient = new RemoteEventClient(options);
        mutationClient = new RemoteMutationClient(options);
        ConfigureTrustIdentity(eventClient.TrustIdentity);
        principal = Environment.GetEnvironmentVariable("LESERPENT_PRINCIPAL")
            ?? "avalonia-remote";
        currentState = eventClient.State;
        runtimeFilterTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromMilliseconds(160),
        };
        runtimeFilterTimer.Tick += (_, _) => ApplyRuntimeFilter();
        runtimeFilterBox.TextChanged += OnRuntimeFilterChanged;
        runtimeFilterBox.KeyDown += OnRuntimeFilterKeyDown;
        clearRuntimeFilterButton.Click += (_, _) => ClearRuntimeFilter();
        ConfigureCredentialSource(tokenSource);

        AutomationProperties.SetAutomationId(statusText, "remote-connection-state");
        AutomationProperties.SetName(statusText, "Remote connection state");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Off);
        AutomationProperties.SetAutomationId(mutationStatusText, "remote-operation-status");
        AutomationProperties.SetName(mutationStatusText, "Remote operation status");
        AutomationProperties.SetLiveSetting(
            mutationStatusText,
            AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(
            dismissMutationButton,
            "remote-operation-dismiss");
        AutomationProperties.SetName(
            dismissMutationButton,
            "Dismiss remote operation status");
        dismissMutationButton.Click += (_, _) => DismissMutationStatus();
        AutomationProperties.SetAutomationId(reconnectButton, "remote-reconnect");
        AutomationProperties.SetName(reconnectButton, "Reconnect remote event stream");
        AutomationProperties.SetHelpText(
            reconnectButton,
            "Restarts the read-only event stream after automatic reconnect is exhausted. Shortcut: F5.");
        ToolTip.SetTip(reconnectButton, "Reconnect event stream (F5)");
        reconnectButton.Click += (_, _) => RequestReconnect();
        AutomationProperties.SetAutomationId(runtimeFilterBox, "remote-runtime-filter");
        AutomationProperties.SetName(runtimeFilterBox, "Filter remote runtimes");
        AutomationProperties.SetHelpText(
            runtimeFilterBox,
            "Filters the local runtime projection without contacting the server. Shortcut: Control or Command plus F.");
        AutomationProperties.SetAutomationId(runtimeCountText, "remote-runtime-count");
        AutomationProperties.SetName(runtimeCountText, "Remote runtime result count");
        AutomationProperties.SetAutomationId(
            clearRuntimeFilterButton,
            "remote-runtime-filter-clear");
        AutomationProperties.SetName(clearRuntimeFilterButton, "Clear runtime filter");
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto,Auto"),
            Children =
            {
                BuildRemoteBody(),
                BuildMutationStatusBar(),
                BuildStatusBar(),
            },
        };
        ApplyResponsiveLayout(RemoteResponsiveLayout.Select(Width));
        ApplyState(currentState);
        eventClient.StateChanged += OnStateChanged;
        Opened += (_, _) => eventClient.Start();
        KeyDown += OnKeyDown;
        SizeChanged += (_, eventArgs) => ApplyResponsiveLayout(
            RemoteResponsiveLayout.Select(eventArgs.NewSize.Width));
        Closed += OnClosed;
    }

    private Border BuildRemoteBody()
    {
        identityGrid.ColumnSpacing = 14;
        identityGrid.Children.Add(remoteOriginText);
        identityGrid.Children.Add(caFingerprintText);
        var identity = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(14, 10),
            Margin = new Thickness(0, 0, 0, 12),
            Child = identityGrid,
        };
        runtimeToolbarGrid.ColumnSpacing = 12;
        runtimeToolbarGrid.Margin = new Thickness(0, 0, 0, 16);
        runtimeToolbarGrid.Children.Add(runtimeFilterBox);
        runtimeToolbarGrid.Children.Add(clearRuntimeFilterButton);
        runtimeToolbarGrid.Children.Add(runtimeCountText);
        var body = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,*"),
            Children = { identity, runtimeToolbarGrid, renderer.Surface },
        };
        Grid.SetRow(runtimeToolbarGrid, 1);
        Grid.SetRow(renderer.Surface, 2);
        remoteBodyBorder.Child = body;
        return remoteBodyBorder;
    }

    private void ConfigureTrustIdentity(RemoteTrustIdentity identity)
    {
        remoteOriginText.Text = identity.Origin;
        caFingerprintText.Text = $"CA / {identity.ShortFingerprint}";
        AutomationProperties.SetAutomationId(remoteOriginText, "remote-origin");
        AutomationProperties.SetName(
            remoteOriginText,
            $"Remote HTTPS origin: {identity.Origin}");
        AutomationProperties.SetAutomationId(caFingerprintText, "remote-ca-fingerprint");
        AutomationProperties.SetName(
            caFingerprintText,
            $"Remote CA SHA-256 fingerprint: {identity.Sha256Fingerprint}");
        AutomationProperties.SetHelpText(
            caFingerprintText,
            "Compare this SHA-256 fingerprint with the expected remote authority CA.");
        ToolTip.SetTip(
            caFingerprintText,
            $"CA SHA-256\n{identity.Sha256Fingerprint}");
    }

    private Border BuildStatusBar()
    {
        var bar = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(24, 12),
            Child = statusGrid,
        };
        statusGrid.ColumnSpacing = 14;
        statusGrid.Children.Add(statusText);
        statusGrid.Children.Add(credentialSourceBadge);
        statusGrid.Children.Add(revisionText);
        statusGrid.Children.Add(reconnectButton);
        statusText.TextTrimming = TextTrimming.CharacterEllipsis;
        revisionText.TextTrimming = TextTrimming.CharacterEllipsis;
        Grid.SetRow(bar, 2);
        return bar;
    }

    private void ApplyResponsiveLayout(RemoteLayoutDensity density)
    {
        var compact = density == RemoteLayoutDensity.Compact;
        remoteBodyBorder.Padding = compact
            ? new Thickness(18, 16)
            : new Thickness(32, 28);

        identityGrid.ColumnDefinitions = ColumnDefinitions.Parse(compact ? "*" : "*,Auto");
        identityGrid.RowDefinitions = RowDefinitions.Parse(compact ? "Auto,Auto" : "Auto");
        Grid.SetColumn(remoteOriginText, 0);
        Grid.SetRow(remoteOriginText, 0);
        Grid.SetColumnSpan(remoteOriginText, 1);
        Grid.SetColumn(caFingerprintText, compact ? 0 : 1);
        Grid.SetRow(caFingerprintText, compact ? 1 : 0);
        caFingerprintText.Margin = compact
            ? new Thickness(0, 6, 0, 0)
            : new Thickness(0);

        runtimeToolbarGrid.ColumnDefinitions = ColumnDefinitions.Parse(
            compact ? "*,Auto" : "*,Auto,Auto");
        runtimeToolbarGrid.RowDefinitions = RowDefinitions.Parse(
            compact ? "Auto,Auto" : "Auto");
        Grid.SetColumn(runtimeFilterBox, 0);
        Grid.SetRow(runtimeFilterBox, 0);
        Grid.SetColumnSpan(runtimeFilterBox, compact ? 2 : 1);
        Grid.SetColumn(clearRuntimeFilterButton, compact ? 0 : 1);
        Grid.SetRow(clearRuntimeFilterButton, compact ? 1 : 0);
        Grid.SetColumn(runtimeCountText, compact ? 1 : 2);
        Grid.SetRow(runtimeCountText, compact ? 1 : 0);
        clearRuntimeFilterButton.Margin = compact
            ? new Thickness(0, 8, 0, 0)
            : new Thickness(0);
        runtimeCountText.Margin = compact
            ? new Thickness(0, 8, 0, 0)
            : new Thickness(0);

        statusGrid.ColumnDefinitions = ColumnDefinitions.Parse(
            compact ? "*,Auto,Auto" : "*,Auto,Auto,Auto");
        statusGrid.RowDefinitions = RowDefinitions.Parse(compact ? "Auto,Auto" : "Auto");
        Grid.SetColumn(statusText, 0);
        Grid.SetRow(statusText, 0);
        Grid.SetColumnSpan(statusText, compact ? 3 : 1);
        statusText.Margin = compact
            ? new Thickness(0, 0, 0, 8)
            : new Thickness(0);
        Grid.SetColumn(credentialSourceBadge, compact ? 0 : 1);
        Grid.SetRow(credentialSourceBadge, compact ? 1 : 0);
        Grid.SetColumn(revisionText, compact ? 1 : 2);
        Grid.SetRow(revisionText, compact ? 1 : 0);
        Grid.SetColumn(reconnectButton, compact ? 2 : 3);
        Grid.SetRow(reconnectButton, compact ? 1 : 0);
    }

    private void ConfigureCredentialSource(RemoteTokenSource source)
    {
        var presentation = RemoteCredentialPresentation.Create(source);
        credentialSourceText.Text = presentation.Label;
        credentialSourceText.Foreground = presentation.IsEnvironmentFallback
            ? LeserpentTheme.Primary
            : LeserpentTheme.Muted;
        credentialSourceBadge.BorderBrush = presentation.IsEnvironmentFallback
            ? LeserpentTheme.Accent
            : LeserpentTheme.PanelBorder;
        credentialSourceBadge.Background = LeserpentTheme.Panel;
        credentialSourceBadge.Child = credentialSourceText;
        AutomationProperties.SetAutomationId(
            credentialSourceBadge,
            "remote-credential-source");
        AutomationProperties.SetName(
            credentialSourceBadge,
            presentation.AutomationName);
        AutomationProperties.SetHelpText(
            credentialSourceBadge,
            presentation.Description);
        ToolTip.SetTip(credentialSourceBadge, presentation.Description);
    }

    private Border BuildMutationStatusBar()
    {
        mutationStatusBar.Child = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
            ColumnSpacing = 14,
            Children = { mutationStatusText, dismissMutationButton },
        };
        Grid.SetColumn(dismissMutationButton, 1);
        Grid.SetRow(mutationStatusBar, 1);
        return mutationStatusBar;
    }

    private void OnStateChanged(RemoteFeedState state) =>
        Dispatcher.UIThread.Post(() => ApplyState(state));

    private void ApplyState(RemoteFeedState state)
    {
        if (feedStateSerial < long.MaxValue)
        {
            feedStateSerial++;
        }
        currentState = state;
        ClearSatisfiedMutationFences(state);
        RenderProjection();
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
        reconnectButton.IsEnabled = state.Phase is RemoteFeedPhase.Stale
            or RemoteFeedPhase.Stopped;
        var live = state.Phase == RemoteFeedPhase.Live && !state.IsStale;
        foreach (var workspace in workspaceWindows.Values)
        {
            workspace.SetRefreshAvailability(
                live && !mutationInFlight,
                live && !mutationInFlight
                    ? null
                    : "Remote refresh requires a live, idle fleet window");
            var runtime = state.Runtimes.FirstOrDefault(candidate =>
                candidate.Id == workspace.RuntimeId);
            if (live && runtime is not null)
            {
                workspace.ReloadIfOlder(runtime.Revision);
            }
        }
    }

    private void OnKeyDown(object? sender, KeyEventArgs eventArgs)
    {
        _ = sender;
        var findModifier = eventArgs.KeyModifiers
            & (KeyModifiers.Control | KeyModifiers.Meta);
        if (eventArgs.Key == Key.F && findModifier != KeyModifiers.None)
        {
            eventArgs.Handled = true;
            runtimeFilterBox.Focus();
            runtimeFilterBox.SelectAll();
        }
        else if (eventArgs.Key == Key.F5 && reconnectButton.IsEnabled)
        {
            eventArgs.Handled = true;
            RequestReconnect();
        }
    }

    private void OnRuntimeFilterChanged(object? sender, TextChangedEventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        var raw = runtimeFilterBox.Text ?? string.Empty;
        var sanitized = new string(raw
            .Where(character => !char.IsControl(character))
            .Take(RemoteDocumentProjection.MaxFilterLength)
            .ToArray());
        if (!string.Equals(raw, sanitized, StringComparison.Ordinal))
        {
            runtimeFilterBox.Text = sanitized;
            runtimeFilterBox.CaretIndex = sanitized.Length;
            return;
        }
        clearRuntimeFilterButton.IsVisible = sanitized.Length > 0;
        runtimeFilterTimer.Stop();
        runtimeFilterTimer.Start();
    }

    private void OnRuntimeFilterKeyDown(object? sender, KeyEventArgs eventArgs)
    {
        _ = sender;
        if (eventArgs.Key == Key.Escape
            && !string.IsNullOrEmpty(runtimeFilterBox.Text))
        {
            eventArgs.Handled = true;
            ClearRuntimeFilter();
        }
    }

    private void ClearRuntimeFilter()
    {
        runtimeFilterTimer.Stop();
        runtimeFilterBox.Text = string.Empty;
        clearRuntimeFilterButton.IsVisible = false;
        RenderProjection();
        runtimeFilterBox.Focus();
    }

    private void ApplyRuntimeFilter()
    {
        runtimeFilterTimer.Stop();
        RenderProjection();
    }

    private void RenderProjection()
    {
        var projection = RemoteDocumentProjection.Project(
            currentState,
            runtimeFilterBox.Text);
        renderer.Mount(projection.Document);
        UpdateMutationAvailability();
        runtimeCountText.Text = projection.VisibleRuntimeCount == projection.TotalRuntimeCount
            ? $"{projection.TotalRuntimeCount} runtimes"
            : $"{projection.VisibleRuntimeCount} of {projection.TotalRuntimeCount}";
        AutomationProperties.SetName(
            runtimeCountText,
            $"Showing {projection.VisibleRuntimeCount} of {projection.TotalRuntimeCount} remote runtimes");
    }

    private async void RequestReconnect()
    {
        if (!reconnectButton.IsEnabled || lifetime.IsCancellationRequested)
        {
            return;
        }
        reconnectButton.IsEnabled = false;
        SetMutationStatus("Restarting the remote event stream...", LeserpentTheme.Primary);
        try
        {
            await eventClient.RestartAsync(lifetime.Token);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            // Window shutdown owns this cancellation.
        }
        catch (InvalidOperationException error)
        {
            SetMutationStatus(
                $"Reconnect blocked: {SafeDisplay(error.Message)}",
                LeserpentTheme.Destructive);
            reconnectButton.IsEnabled = currentState.Phase is RemoteFeedPhase.Stale
                or RemoteFeedPhase.Stopped;
        }
    }

    private async void OnActionInvoked(string nodeId)
    {
        var inspectedRuntime = currentState.Runtimes.FirstOrDefault(candidate =>
            nodeId == $"runtime:{candidate.Id}:inspect");
        if (inspectedRuntime is not null)
        {
            OpenWorkspace(inspectedRuntime);
            return;
        }
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
        UpdateMutationAvailability();
        var confirmed = await new RuntimeRefreshConfirmationWindow(runtime)
            .ShowDialog<bool>(this);
        if (!confirmed || lifetime.IsCancellationRequested)
        {
            mutationInFlight = false;
            UpdateMutationAvailability();
            return;
        }
        var confirmedRuntime = currentState.Runtimes.FirstOrDefault(candidate =>
            candidate.Id == runtime.Id);
        if (currentState.Phase != RemoteFeedPhase.Live
            || currentState.IsStale
            || confirmedRuntime?.Revision != runtime.Revision)
        {
            mutationInFlight = false;
            UpdateMutationAvailability();
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
            mutationRevisionFence = new MutationRevisionFence(runtime.Id, result.Revision);
            ClearSatisfiedMutationFences(currentState);
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
            FenceUntilNextLiveObservation();
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
            FenceUntilNextLiveObservation();
            SetMutationStatus(
                "Refresh outcome unknown after a network failure; wait for the event stream before retrying",
                LeserpentTheme.Destructive);
        }
        finally
        {
            mutationInFlight = false;
            UpdateMutationAvailability();
        }
    }

    private void FenceUntilNextLiveObservation()
    {
        mutationObservationFence = feedStateSerial == long.MaxValue
            ? long.MaxValue
            : feedStateSerial + 1;
    }

    private void ClearSatisfiedMutationFences(RemoteFeedState state)
    {
        if (mutationRevisionFence is { } revisionFence
            && state.Runtimes.Any(runtime => runtime.Id == revisionFence.RuntimeId
                && runtime.Revision >= revisionFence.Revision))
        {
            mutationRevisionFence = null;
        }
        if (mutationObservationFence is { } observationFence
            && feedStateSerial >= observationFence
            && state.Phase == RemoteFeedPhase.Live
            && !state.IsStale)
        {
            mutationObservationFence = null;
        }
    }

    private void UpdateMutationAvailability()
    {
        var live = currentState.Phase == RemoteFeedPhase.Live && !currentState.IsStale;
        var reason = mutationInFlight
            ? "A remote refresh is awaiting confirmation or completion"
            : mutationRevisionFence is { } revisionFence
                ? $"Waiting for event revision {revisionFence.Revision} before another refresh"
                : mutationObservationFence is not null
                    ? "Waiting for a live event after an unknown refresh outcome"
                    : live
                        ? null
                        : "Remote refresh is unavailable while the event stream is not live";
        renderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            reason is null,
            reason);
        renderer.SetActionAvailability(
            ActionKind.RuntimeInspect,
            live,
            live ? null : "Runtime inspection requires a live event stream");
        foreach (var workspace in workspaceWindows.Values)
        {
            workspace.SetRefreshAvailability(reason is null, reason);
        }
    }

    private void OpenWorkspace(RemoteRuntimeProjection runtime)
    {
        if (currentState.Phase != RemoteFeedPhase.Live || currentState.IsStale)
        {
            SetMutationStatus(
                "Inspect blocked: remote state is not live",
                LeserpentTheme.Destructive);
            return;
        }
        if (workspaceWindows.TryGetValue(runtime.Id, out var existing))
        {
            existing.Activate();
            return;
        }
        if (workspaceWindows.Count >= MaxOpenWorkspaces)
        {
            SetMutationStatus(
                $"Inspect blocked: close one of the {MaxOpenWorkspaces} open workspaces first",
                LeserpentTheme.Destructive);
            return;
        }
        var workspace = new RemoteRuntimeWorkspaceWindow(
            options,
            runtime,
            principal,
            OnActionInvoked);
        workspaceWindows.Add(runtime.Id, workspace);
        workspace.Closed += (_, _) => workspaceWindows.Remove(runtime.Id);
        workspace.SetRefreshAvailability(!mutationInFlight, mutationInFlight
            ? "A remote refresh is already in progress"
            : null);
        workspace.Show(this);
    }

    private async void OnClosed(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        lifetime.Cancel();
        runtimeFilterTimer.Stop();
        foreach (var workspace in workspaceWindows.Values.ToArray())
        {
            workspace.Close();
        }
        mutationClient.Dispose();
        await eventClient.DisposeAsync();
        lifetime.Dispose();
    }

    private void SetMutationStatus(string text, IBrush foreground)
    {
        mutationStatusText.Text = text;
        mutationStatusText.Foreground = foreground;
        AutomationProperties.SetName(
            mutationStatusText,
            $"Remote operation status: {text}");
        AutomationProperties.SetLiveSetting(
            mutationStatusText,
            ReferenceEquals(foreground, LeserpentTheme.Destructive)
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
        mutationStatusBar.IsVisible = true;
    }

    private void DismissMutationStatus()
    {
        mutationStatusBar.IsVisible = false;
        mutationStatusText.Text = string.Empty;
        AutomationProperties.SetName(
            mutationStatusText,
            "Remote operation status");
        AutomationProperties.SetLiveSetting(
            mutationStatusText,
            AutomationLiveSetting.Off);
    }

    private static string SafeDisplay(string value)
    {
        var sanitized = new string(value
            .Where(character => !char.IsControl(character))
            .Take(256)
            .ToArray());
        return string.IsNullOrWhiteSpace(sanitized) ? "Unavailable" : sanitized;
    }

    private sealed record MutationRevisionFence(string RuntimeId, ulong Revision);
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
        Opened += (_, _) => cancel.Focus();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close(false);
            }
        };

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
