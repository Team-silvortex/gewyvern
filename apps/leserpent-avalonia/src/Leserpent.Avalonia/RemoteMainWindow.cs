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
    private readonly RemoteHealthClient healthClient;
    private readonly RemoteAuthorityHealthCoordinator authorityHealthCoordinator;
    private readonly RemoteLeselangClient leselangClient;
    private readonly RemoteMutationClient mutationClient;
    private readonly RemoteClientOptions options;
    private readonly Dictionary<string, RemoteRuntimeWorkspaceWindow> workspaceWindows =
        new(StringComparer.Ordinal);
    private readonly RemoteWorkspaceLaunchCoordinator workspaceLaunch =
        new(MaxOpenWorkspaces);
    private readonly RemoteMutationCoordinator mutationCoordinator = new();
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private RemoteFeedState currentState;
    private bool isClosed;
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
    private readonly Button connectionButton = new()
    {
        Content = "Connection...",
        IsVisible = false,
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
    private readonly TextBlock authorityHealthText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 11,
        FontWeight = FontWeight.Bold,
        Text = "AUTHORITY / awaiting check",
    };
    private readonly Button authorityHealthButton = new()
    {
        Content = "Refresh health",
        Padding = new Thickness(12, 6),
    };
    private readonly StackPanel authorityHealthPanel = new()
    {
        Orientation = Avalonia.Layout.Orientation.Horizontal,
        Spacing = 10,
        VerticalAlignment = Avalonia.Layout.VerticalAlignment.Center,
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

    public RemoteMainWindow(
        RemoteClientOptions options,
        RemoteTokenSource tokenSource,
        Action? manageConnection = null)
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
        healthClient = new RemoteHealthClient(options);
        authorityHealthCoordinator = new RemoteAuthorityHealthCoordinator(
            healthClient.CheckAsync);
        leselangClient = new RemoteLeselangClient(options);
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
        authorityHealthButton.Click += (_, _) => RefreshAuthorityHealth();
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
        connectionButton.IsVisible = manageConnection is not null;
        connectionButton.Click += (_, _) => manageConnection?.Invoke();
        AutomationProperties.SetAutomationId(connectionButton, "remote-manage-connection");
        AutomationProperties.SetName(connectionButton, "Manage remote connection");
        AutomationProperties.SetHelpText(
            connectionButton,
            "Switch the remote authority or forget the saved profile and endpoint credential.");
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
        AutomationProperties.SetAutomationId(
            authorityHealthText,
            "remote-authority-health");
        AutomationProperties.SetName(
            authorityHealthText,
            "Remote authority health has not been checked");
        AutomationProperties.SetLiveSetting(
            authorityHealthText,
            AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(
            authorityHealthButton,
            "remote-authority-health-refresh");
        AutomationProperties.SetName(
            authorityHealthButton,
            "Refresh remote authority health");
        AutomationProperties.SetHelpText(
            authorityHealthButton,
            "Checks authority ownership, protocol readiness, and effect queue pressure without changing remote state.");
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
        ApplyAuthorityHealth(authorityHealthCoordinator.State);
        eventClient.StateChanged += OnStateChanged;
        Opened += (_, _) =>
        {
            eventClient.Start();
            RefreshAuthorityHealth();
        };
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
        authorityHealthPanel.Children.Add(authorityHealthText);
        authorityHealthPanel.Children.Add(authorityHealthButton);
        identityGrid.Children.Add(authorityHealthPanel);
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
        statusGrid.Children.Add(connectionButton);
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

        identityGrid.ColumnDefinitions = ColumnDefinitions.Parse(
            compact ? "*" : "*,Auto,Auto");
        identityGrid.RowDefinitions = RowDefinitions.Parse(
            compact ? "Auto,Auto,Auto" : "Auto");
        Grid.SetColumn(remoteOriginText, 0);
        Grid.SetRow(remoteOriginText, 0);
        Grid.SetColumnSpan(remoteOriginText, 1);
        Grid.SetColumn(caFingerprintText, compact ? 0 : 1);
        Grid.SetRow(caFingerprintText, compact ? 1 : 0);
        caFingerprintText.Margin = compact
            ? new Thickness(0, 6, 0, 0)
            : new Thickness(0);
        Grid.SetColumn(authorityHealthPanel, compact ? 0 : 2);
        Grid.SetRow(authorityHealthPanel, compact ? 2 : 0);
        authorityHealthPanel.Margin = compact
            ? new Thickness(0, 8, 0, 0)
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
            compact ? "*,Auto,Auto,Auto" : "*,Auto,Auto,Auto,Auto");
        statusGrid.RowDefinitions = RowDefinitions.Parse(compact ? "Auto,Auto" : "Auto");
        Grid.SetColumn(statusText, 0);
        Grid.SetRow(statusText, 0);
        Grid.SetColumnSpan(statusText, compact ? 4 : 1);
        statusText.Margin = compact
            ? new Thickness(0, 0, 0, 8)
            : new Thickness(0);
        Grid.SetColumn(credentialSourceBadge, compact ? 0 : 1);
        Grid.SetRow(credentialSourceBadge, compact ? 1 : 0);
        Grid.SetColumn(revisionText, compact ? 1 : 2);
        Grid.SetRow(revisionText, compact ? 1 : 0);
        Grid.SetColumn(connectionButton, compact ? 2 : 3);
        Grid.SetRow(connectionButton, compact ? 1 : 0);
        Grid.SetColumn(reconnectButton, compact ? 3 : 4);
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
        Dispatcher.UIThread.Post(() =>
        {
            if (!isClosed)
            {
                ApplyState(state);
            }
        });

    private void ApplyState(RemoteFeedState state)
    {
        currentState = state;
        mutationCoordinator.Observe(state);
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
            var runtime = state.Runtimes.FirstOrDefault(candidate =>
                candidate.Id == workspace.RuntimeId);
            if (live && runtime is not null)
            {
                workspace.ReloadIfOlder(runtime.Revision);
            }
        }
        ResolvePendingWorkspaces(state);
    }

    internal string? RequestRuntimeWorkspace(string runtimeId, ulong topologyRevision)
    {
        if (isClosed)
        {
            return "The daemon session is already closed.";
        }
        return ApplyWorkspaceLaunchDecision(workspaceLaunch.Request(
            runtimeId,
            topologyRevision,
            currentState,
            workspaceWindows.Keys));
    }

    private void ResolvePendingWorkspaces(RemoteFeedState state)
    {
        var decisions = workspaceLaunch.Observe(state);
        if (decisions.Any(decision =>
                decision.Disposition == RemoteWorkspaceLaunchDisposition.RejectUnavailable))
        {
            SetMutationStatus(
                "Pending workspaces were not opened because no authoritative daemon snapshot is available",
                LeserpentTheme.Destructive);
            return;
        }
        foreach (var decision in decisions)
        {
            if (decision.Disposition == RemoteWorkspaceLaunchDisposition.Open
                && decision.Runtime is { } runtime)
            {
                OpenWorkspace(runtime);
                continue;
            }
            if (decision.Disposition == RemoteWorkspaceLaunchDisposition.RejectRemoved)
            {
                SetMutationStatus(
                    $"Workspace not opened: {SafeDisplay(decision.RuntimeId)} is absent from the authoritative topology",
                    LeserpentTheme.Destructive);
            }
        }
    }

    private string? ApplyWorkspaceLaunchDecision(RemoteWorkspaceLaunchDecision decision)
    {
        switch (decision.Disposition)
        {
            case RemoteWorkspaceLaunchDisposition.FocusExisting:
                var existing = workspaceWindows[decision.RuntimeId];
                existing.Show();
                existing.Activate();
                return null;
            case RemoteWorkspaceLaunchDisposition.Open when decision.Runtime is { } runtime:
                OpenWorkspace(runtime);
                return null;
            case RemoteWorkspaceLaunchDisposition.Wait:
                SetMutationStatus(
                    $"Waiting for an authoritative daemon snapshot before opening {SafeDisplay(decision.RuntimeId)}...",
                    LeserpentTheme.Primary);
                return null;
            case RemoteWorkspaceLaunchDisposition.RejectInvalidRuntimeId:
                return "Workspace request contains an invalid runtime ID.";
            case RemoteWorkspaceLaunchDisposition.RejectCapacity:
                return $"Close one of the {MaxOpenWorkspaces} open or pending workspaces first.";
            case RemoteWorkspaceLaunchDisposition.RejectRemoved:
                return "The runtime is no longer present in the daemon's authoritative topology.";
            case RemoteWorkspaceLaunchDisposition.RejectUnavailable:
                return "No authoritative daemon snapshot is available.";
            default:
                return "The workspace launch decision is incomplete.";
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
        var sanitized = RemoteRuntimeSearch.SanitizeInput(raw);
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

    private void RequestReconnect() => ObserveUiOperation(RequestReconnectAsync());

    private void RefreshAuthorityHealth() =>
        ObserveHealthOperation(RefreshAuthorityHealthAsync());

    private async Task RefreshAuthorityHealthAsync()
    {
        if (isClosed)
        {
            return;
        }
        var refresh = authorityHealthCoordinator.RefreshAsync(lifetime.Token);
        ApplyAuthorityHealth(authorityHealthCoordinator.State);
        var state = await refresh;
        if (!isClosed)
        {
            ApplyAuthorityHealth(state);
        }
    }

    private void ApplyAuthorityHealth(RemoteAuthorityHealthState state)
    {
        authorityHealthText.Text = state.Label;
        authorityHealthText.Foreground = state.RequiresAttention
            ? LeserpentTheme.Destructive
            : state.Phase == RemoteAuthorityHealthPhase.Ready
                ? LeserpentTheme.Accent
                : LeserpentTheme.Muted;
        AutomationProperties.SetName(authorityHealthText, state.AutomationName);
        AutomationProperties.SetLiveSetting(
            authorityHealthText,
            state.RequiresAttention
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
        authorityHealthButton.IsEnabled = state.IsRefreshEnabled && !isClosed;
    }

    private async Task RequestReconnectAsync()
    {
        if (!reconnectButton.IsEnabled || isClosed)
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

    private void OnActionInvoked(RenderedActionInvocation invocation) =>
        ObserveUiOperation(OnActionInvokedAsync(invocation));

    private async Task OnActionInvokedAsync(RenderedActionInvocation invocation)
    {
        if (isClosed)
        {
            return;
        }
        if (!IsActiveActionSource(invocation.Source))
        {
            SetMutationStatus(
                "Remote action blocked: its workspace is already closed",
                LeserpentTheme.Destructive);
            return;
        }
        var resolution = RemoteUiActionRouter.ResolveActivation(
            invocation.Source.Document,
            invocation.NodeId,
            currentState,
            mutationCoordinator.Availability(currentState));
        if (!resolution.Accepted || resolution.Intent is not { } intent)
        {
            SetMutationStatus(
                $"Remote action blocked: {resolution.Reason}",
                LeserpentTheme.Destructive);
            return;
        }
        var runtime = intent.Runtime;
        if (intent.Kind == ActionKind.RuntimeInspect)
        {
            var workspaceError = RequestRuntimeWorkspace(
                runtime.Id,
                currentState.SnapshotRevision ?? runtime.Revision);
            if (workspaceError is not null)
            {
                SetMutationStatus(workspaceError, LeserpentTheme.Destructive);
            }
            return;
        }
        if (intent.Kind == ActionKind.RuntimeDeploy)
        {
            await DeployRuntimeAsync(invocation.Source, intent);
            return;
        }
        if (intent.Kind is not (
            ActionKind.RuntimeRefresh or ActionKind.RuntimeCapabilitiesRefresh))
        {
            SetMutationStatus(
                "Remote action blocked: unsupported typed action",
                LeserpentTheme.Destructive);
            return;
        }
        var refreshCapabilities = intent.Kind == ActionKind.RuntimeCapabilitiesRefresh;
        var admission = mutationCoordinator.Begin(
            new RemoteMutationRequest(
                runtime.Id,
                runtime.Revision,
                refreshCapabilities
                    ? RemoteMutationKind.CapabilityRefresh
                    : RemoteMutationKind.Refresh),
            currentState);
        if (!admission.Accepted || admission.Operation is not { } operation)
        {
            SetMutationStatus(
                $"Refresh blocked: {RemoteMutationCoordinator.DescribeFailure(admission.Failure)}",
                LeserpentTheme.Destructive);
            return;
        }
        UpdateMutationAvailability();
        var confirmed = await new RuntimeRefreshConfirmationWindow(
                runtime,
                refreshCapabilities,
                cancellationToken => leselangClient.ExportRefreshAsync(
                    runtime.Id,
                    refreshCapabilities,
                    cancellationToken))
            .ShowDialog<bool>(this);
        if (!confirmed || lifetime.IsCancellationRequested)
        {
            mutationCoordinator.Cancel(operation);
            UpdateMutationAvailability();
            return;
        }
        var confirmation = mutationCoordinator.Confirm(operation, currentState);
        if (!confirmation.Accepted)
        {
            UpdateMutationAvailability();
            SetMutationStatus(
                $"Refresh blocked: {RemoteMutationCoordinator.DescribeFailure(confirmation.Failure)} during confirmation",
                LeserpentTheme.Destructive);
            return;
        }
        SetMutationStatus(
            refreshCapabilities
                ? $"Discovering capabilities for {SafeDisplay(runtime.Name)} at revision {runtime.Revision}..."
                : $"Refreshing {SafeDisplay(runtime.Name)} at revision {runtime.Revision}...",
            LeserpentTheme.Primary);
        try
        {
            var result = refreshCapabilities
                ? await mutationClient.RefreshCapabilitiesAsync(
                    runtime.Id,
                    runtime.Revision,
                    principal,
                    lifetime.Token)
                : await mutationClient.RefreshAsync(
                    runtime.Id,
                    runtime.Revision,
                    principal,
                    lifetime.Token);
            mutationCoordinator.Accept(operation, result, currentState);
            SetMutationStatus(
                refreshCapabilities
                    ? $"Capability discovery requested for {SafeDisplay(runtime.Name)} at revision {result.Revision}"
                    : $"Refresh applied to {SafeDisplay(runtime.Name)} at revision {result.Revision}",
                LeserpentTheme.Accent);
        }
        catch (Exception error)
        {
            ApplyMutationFailure(operation, error);
        }
        finally
        {
            mutationCoordinator.Abandon(operation, currentState);
            UpdateMutationAvailability();
        }
    }

    private async Task DeployRuntimeAsync(
        AvaloniaDocumentRenderer sourceRenderer,
        RemoteUiActionIntent actionIntent)
    {
        if (actionIntent.Form is not { } form)
        {
            SetMutationStatus(
                "Deployment blocked: typed action has no form",
                LeserpentTheme.Destructive);
            return;
        }
        var runtime = actionIntent.Runtime;
        var nodeId = actionIntent.NodeId;
        var admission = mutationCoordinator.Begin(
            new RemoteMutationRequest(
                runtime.Id,
                runtime.Revision,
                RemoteMutationKind.Deployment),
            currentState);
        if (!admission.Accepted || admission.Operation is not { } operation)
        {
            SetMutationStatus(
                $"Deployment blocked: {RemoteMutationCoordinator.DescribeFailure(admission.Failure)}",
                LeserpentTheme.Destructive);
            return;
        }
        UpdateMutationAvailability();
        var formWindow = new ParameterizedActionFormWindow(
            form,
            $"{SafeDisplay(runtime.Name)}\nID: {runtime.Id}\nExpected revision: {runtime.Revision}",
            "This submits an authenticated, revision-checked deployment and is not retried automatically.",
            (values, cancellationToken) =>
            {
                var preview = RemoteUiActionRouter.ResolveSubmission(
                    sourceRenderer.Document,
                    new UiEvent
                    {
                        NodeId = nodeId,
                        Kind = UiEventKind.Submit,
                        Values = values.ToDictionary(
                            entry => entry.Key,
                            entry => entry.Value,
                            StringComparer.Ordinal),
                    },
                    currentState,
                    nodeId);
                if (!preview.Accepted
                    || preview.Intent is not
                    { PipelineKind: { } pipelineKind } previewIntent)
                {
                    throw new ArgumentException(preview.Reason);
                }
                return leselangClient.ExportDeployAsync(
                    runtime.Id,
                    pipelineKind,
                    previewIntent.Target,
                    cancellationToken);
            });
        using var formRegistration = sourceRenderer.RegisterFormFields(
            nodeId,
            formWindow.FormFields,
            formWindow,
            formWindow.SubmitButton,
            formWindow.CancelButton);
        var intent = await formWindow.ShowDialog<ParameterizedFormIntent?>(this);
        if (intent is null || lifetime.IsCancellationRequested)
        {
            mutationCoordinator.Cancel(operation);
            UpdateMutationAvailability();
            return;
        }
        UiEvent submission;
        try
        {
            submission = sourceRenderer.CreateFormSubmission(nodeId, intent.Values);
        }
        catch (InvalidDataException error)
        {
            mutationCoordinator.Cancel(operation);
            UpdateMutationAvailability();
            SetMutationStatus(
                $"Deployment blocked: {SafeDisplay(error.Message)}",
                LeserpentTheme.Destructive);
            return;
        }
        if (!IsActiveActionSource(sourceRenderer))
        {
            mutationCoordinator.Cancel(operation);
            UpdateMutationAvailability();
            SetMutationStatus(
                "Deployment blocked: its workspace was closed before submission",
                LeserpentTheme.Destructive);
            return;
        }
        var submitted = RemoteUiActionRouter.ResolveSubmission(
            sourceRenderer.Document,
            submission,
            currentState,
            nodeId);
        if (!submitted.Accepted
            || submitted.Intent is not
            { PipelineKind: { } pipelineKind } submittedIntent)
        {
            mutationCoordinator.Cancel(operation);
            UpdateMutationAvailability();
            SetMutationStatus(
                $"Deployment blocked: {submitted.Reason}",
                LeserpentTheme.Destructive);
            return;
        }
        var target = submittedIntent.Target;
        var confirmation = mutationCoordinator.Confirm(operation, currentState);
        if (!confirmation.Accepted)
        {
            UpdateMutationAvailability();
            SetMutationStatus(
                $"Deployment blocked: {RemoteMutationCoordinator.DescribeFailure(confirmation.Failure)} during confirmation",
                LeserpentTheme.Destructive);
            return;
        }
        SetMutationStatus(
            $"Deploying {SafeDisplay(pipelineKind)} to {SafeDisplay(runtime.Name)} at revision {runtime.Revision}...",
            LeserpentTheme.Primary);
        try
        {
            var result = await mutationClient.DeployAsync(
                runtime.Id,
                runtime.Revision,
                principal,
                pipelineKind,
                target,
                lifetime.Token);
            mutationCoordinator.Accept(operation, result, currentState);
            SetMutationStatus(
                $"Deployment accepted for {SafeDisplay(runtime.Name)} at revision {result.Revision}",
                LeserpentTheme.Accent);
        }
        catch (Exception error)
        {
            ApplyMutationFailure(operation, error);
        }
        finally
        {
            mutationCoordinator.Abandon(operation, currentState);
            UpdateMutationAvailability();
        }
    }

    private void UpdateMutationAvailability()
    {
        var availability = mutationCoordinator.Availability(currentState);
        renderer.SetActionAvailability(
            ActionKind.RuntimeRefresh,
            availability.MutationsEnabled,
            availability.MutationUnavailableReason);
        renderer.SetActionAvailability(
            ActionKind.RuntimeCapabilitiesRefresh,
            availability.MutationsEnabled,
            availability.MutationUnavailableReason);
        renderer.SetActionAvailability(
            ActionKind.RuntimeDeploy,
            availability.MutationsEnabled,
            availability.MutationUnavailableReason);
        renderer.SetActionAvailability(
            ActionKind.RuntimeInspect,
            availability.InspectEnabled,
            availability.InspectUnavailableReason);
        foreach (var workspace in workspaceWindows.Values)
        {
            SetWorkspaceMutationAvailability(workspace, availability);
        }
    }

    private void ApplyMutationFailure(
        RemoteMutationOperation operation,
        Exception error)
    {
        var failure = mutationCoordinator.CompleteFailure(
            operation,
            error,
            currentState,
            lifetime.IsCancellationRequested);
        if (!isClosed
            && failure.RequiresOperatorAttention
            && failure.OperatorMessage is { } message)
        {
            SetMutationStatus(message, LeserpentTheme.Destructive);
        }
    }

    private static void SetWorkspaceMutationAvailability(
        RemoteRuntimeWorkspaceWindow workspace,
        RemoteMutationAvailability availability) => workspace.SetRefreshAvailability(
            availability.MutationsEnabled,
            availability.MutationUnavailableReason);

    private bool IsActiveActionSource(AvaloniaDocumentRenderer source) =>
        ReferenceEquals(source, renderer)
        || workspaceWindows.Values.Any(workspace => workspace.OwnsActionSource(source));

    private void OpenWorkspace(RemoteRuntimeProjection runtime)
    {
        if (currentState.Phase != RemoteFeedPhase.Live
            || currentState.IsStale
            || currentState.SnapshotGeneration == 0)
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
        SetWorkspaceMutationAvailability(
            workspace,
            mutationCoordinator.Availability(currentState));
        workspace.Show(this);
    }

    private void OnClosed(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        if (isClosed)
        {
            return;
        }
        isClosed = true;
        eventClient.StateChanged -= OnStateChanged;
        authorityHealthCoordinator.Stop();
        lifetime.Cancel();
        runtimeFilterTimer.Stop();
        foreach (var workspace in workspaceWindows.Values.ToArray())
        {
            workspace.Close();
        }
        workspaceLaunch.ClearPending();
        mutationCoordinator.CancelActive();
        healthClient.Dispose();
        leselangClient.Dispose();
        mutationClient.Dispose();
        ObserveShutdown(eventClient.DisposeAsync());
    }

    private async void ObserveUiOperation(Task operation)
    {
        try
        {
            await operation;
        }
        catch (Exception error) when (!isClosed)
        {
            mutationCoordinator.AbandonActive(currentState);
            UpdateMutationAvailability();
            SetMutationStatus(
                $"Remote operation failed safely: {SafeDisplay(error.Message)}",
                LeserpentTheme.Destructive);
        }
        catch (Exception) when (isClosed)
        {
            // Closing the window invalidates dialogs, controls, and clients together.
        }
    }

    private async void ObserveHealthOperation(Task operation)
    {
        try
        {
            await operation;
        }
        catch (Exception) when (!isClosed)
        {
            ApplyAuthorityHealth(authorityHealthCoordinator.State);
        }
        catch (Exception) when (isClosed)
        {
            // Closing the window invalidates the health client and controls together.
        }
    }

    private async void ObserveShutdown(ValueTask shutdown)
    {
        try
        {
            await shutdown;
        }
        catch (Exception)
        {
            // Shutdown is best-effort after cancellation and client disposal.
        }
        finally
        {
            lifetime.Dispose();
        }
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

}

internal sealed class RuntimeRefreshConfirmationWindow : Window
{
    public RuntimeRefreshConfirmationWindow(
        RemoteRuntimeProjection runtime,
        bool refreshCapabilities,
        Func<CancellationToken, Task<string>> exportLeselang)
    {
        var operation = refreshCapabilities ? "capability discovery" : "runtime refresh";
        var action = refreshCapabilities ? "Discover capabilities" : "Refresh runtime";
        Title = $"Confirm remote {operation}";
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
            Content = action,
            Background = LeserpentTheme.Accent,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(18, 9),
        };
        var leselang = new LeselangExportControl(
            refreshCapabilities ? "runtime-capabilities-refresh" : "runtime-refresh",
            exportLeselang);
        AutomationProperties.SetAutomationId(
            cancel,
            refreshCapabilities
                ? "runtime-capabilities-refresh-cancel"
                : "runtime-refresh-cancel");
        AutomationProperties.SetName(cancel, $"Cancel {operation}");
        AutomationProperties.SetAutomationId(
            confirm,
            refreshCapabilities
                ? "runtime-capabilities-refresh-confirm"
                : "runtime-refresh-confirm");
        AutomationProperties.SetName(confirm, $"Confirm {operation}");
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
                        Text = refreshCapabilities
                            ? "Discover this runtime's capabilities?"
                            : "Refresh this remote runtime?",
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
                    leselang,
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

internal sealed record ParameterizedFormIntent(IReadOnlyDictionary<string, string> Values);

internal sealed class ParameterizedActionFormWindow : Window
{
    public IReadOnlyDictionary<string, TextBox> FormFields { get; }
    public Button SubmitButton { get; }
    public Button CancelButton { get; }

    public ParameterizedActionFormWindow(
        UiForm form,
        string context,
        string warning,
        Func<IReadOnlyDictionary<string, string>, CancellationToken, Task<string>> exportLeselang)
    {
        Title = form.Title.Fallback;
        Width = 520;
        MinWidth = 440;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        var inputs = new Dictionary<string, (UiFormField Field, TextBox Input)>(
            StringComparer.Ordinal);
        var fields = new StackPanel { Spacing = 8 };
        foreach (var field in form.Fields)
        {
            var input = new TextBox
            {
                PlaceholderText = field.Placeholder?.Fallback,
                MaxLength = field.MaxLength,
            };
            AutomationProperties.SetAutomationId(input, $"parameter-form-{field.Key}");
            AutomationProperties.SetName(input, field.Label.Fallback);
            fields.Children.Add(new TextBlock
            {
                Text = field.Required
                    ? $"{field.Label.Fallback} (required)"
                    : field.Label.Fallback,
                Foreground = LeserpentTheme.Body,
                FontWeight = FontWeight.SemiBold,
            });
            fields.Children.Add(input);
            if (!inputs.TryAdd(field.Key, (field, input)))
            {
                throw new InvalidDataException("parameterized form contains duplicate fields");
            }
        }
        FormFields = inputs.ToDictionary(
            entry => entry.Key,
            entry => entry.Value.Input,
            StringComparer.Ordinal);
        var validation = new TextBlock
        {
            Foreground = LeserpentTheme.Destructive,
            FontSize = 13,
            TextWrapping = TextWrapping.Wrap,
        };
        CancelButton = new Button
        {
            Content = "Cancel",
            Padding = new Thickness(18, 9),
        };
        SubmitButton = new Button
        {
            Content = form.SubmitLabel.Fallback,
            Background = LeserpentTheme.Accent,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(18, 9),
            IsEnabled = false,
        };
        var cancel = CancelButton;
        var submit = SubmitButton;
        AutomationProperties.SetAutomationId(cancel, "parameter-form-cancel");
        AutomationProperties.SetName(cancel, "Cancel form submission");
        AutomationProperties.SetAutomationId(submit, "parameter-form-submit");
        AutomationProperties.SetName(submit, form.SubmitLabel.Fallback);
        var leselang = new LeselangExportControl("parameter-form");

        IReadOnlyDictionary<string, string> Values() => inputs
            .Where(entry => !string.IsNullOrEmpty(entry.Value.Input.Text))
            .ToDictionary(
                entry => entry.Key,
                entry => entry.Value.Input.Text!,
                StringComparer.Ordinal);

        void Validate()
        {
            var invalid = inputs.Values.FirstOrDefault(entry =>
                !ValidValue(entry.Input.Text ?? string.Empty, entry.Field));
            submit.IsEnabled = invalid.Field is null;
            validation.Text = invalid.Field is null
                ? string.Empty
                : $"{invalid.Field.Label.Fallback}: {ValidationMessage(invalid.Field)}";
            leselang.Update(invalid.Field is null
                ? cancellationToken => exportLeselang(Values(), cancellationToken)
                : null);
        }
        foreach (var (_, input) in inputs.Values)
        {
            input.TextChanged += (_, _) => Validate();
        }
        cancel.Click += (_, _) => Close(null);
        submit.Click += (_, _) => Close(new ParameterizedFormIntent(Values()));
        Opened += (_, _) => inputs.Values.First().Input.Focus();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close(null);
            }
        };

        var buttons = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            Spacing = 12,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Children = { cancel, submit },
        };
        Content = new Border
        {
            Padding = new Thickness(28),
            Child = new StackPanel
            {
                Spacing = 12,
                Children =
                {
                    new TextBlock
                    {
                        Text = form.Title.Fallback,
                        Foreground = LeserpentTheme.Primary,
                        FontSize = 22,
                        FontWeight = FontWeight.Bold,
                    },
                    new TextBlock
                    {
                        Text = Safe(context),
                        Foreground = LeserpentTheme.Body,
                        FontSize = 14,
                        LineHeight = 22,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    fields,
                    validation,
                    leselang,
                    new TextBlock
                    {
                        Text = Safe(warning),
                        Foreground = LeserpentTheme.Muted,
                        FontSize = 13,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    buttons,
                },
            },
        };
        Validate();
    }

    private static bool ValidValue(string value, UiFormField field)
    {
        if ((field.Required && value.Length == 0) || value.Length > field.MaxLength)
        {
            return false;
        }
        return field.InputKind switch
        {
            UiFormInputKind.PathToken => value.Length > 0
                && value.All(character => char.IsAsciiLetterOrDigit(character)
                    || character is '.' or '/' or '_' or '-'),
            UiFormInputKind.TrimmedText => value == value.Trim()
                && !value.Any(char.IsControl),
            _ => false,
        };
    }

    private static string ValidationMessage(UiFormField field) => field.InputKind switch
    {
        UiFormInputKind.PathToken =>
            "use only letters, digits, '.', '/', '_' and '-' within the declared limit",
        UiFormInputKind.TrimmedText =>
            "use trimmed text without control characters within the declared limit",
        _ => "unsupported input constraint",
    };

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
