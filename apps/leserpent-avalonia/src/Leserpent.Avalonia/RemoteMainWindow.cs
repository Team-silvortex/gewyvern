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
    private readonly DesktopLocalization localization;
    private readonly RemoteTrustIdentity trustIdentity;
    private readonly RemoteTokenSource credentialSource;
    private readonly bool startRemoteClients;
    private readonly Dictionary<string, RemoteRuntimeWorkspaceWindow> workspaceWindows =
        new(StringComparer.Ordinal);
    private readonly RemoteWorkspaceLaunchCoordinator workspaceLaunch =
        new(MaxOpenWorkspaces);
    private readonly RemoteMutationCoordinator mutationCoordinator = new();
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private RemoteFeedState currentState;
    private RemoteAuthorityHealthState currentAuthorityHealthState;
    private DesktopRemoteText? mutationNotice;
    private IBrush mutationNoticeForeground = LeserpentTheme.Muted;
    private bool registrationMutationEnabled;
    private string? registrationMutationUnavailableReason;
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
        IsEnabled = false,
        Padding = new Thickness(14, 7),
    };
    private readonly Button connectionButton = new()
    {
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
    };
    private readonly Button authorityHealthButton = new()
    {
        Padding = new Thickness(12, 6),
    };
    private readonly Button orchestraButton = new()
    {
        Padding = new Thickness(12, 7),
    };
    private readonly Button debuggerButton = new()
    {
        Padding = new Thickness(12, 7),
    };
    private readonly Button registrationButton = new()
    {
        Padding = new Thickness(12, 7),
        IsEnabled = false,
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
        IsVisible = false,
        Padding = new Thickness(12, 7),
    };
    private readonly DispatcherTimer runtimeFilterTimer;
    private readonly Border remoteBodyBorder = new();
    private readonly Grid identityGrid = new();
    private readonly Grid runtimeToolbarGrid = new();
    private readonly Grid statusGrid = new();
    private RemoteOrchestraWorkspaceWindow? orchestraWorkspace;
    private RemoteDebuggerWindow? debuggerWorkspace;
    private RuntimeRegistrationWindow? registrationWindow;

    public RemoteMainWindow(
        RemoteClientOptions options,
        RemoteTokenSource tokenSource,
        Action? manageConnection = null,
        DesktopLocalization? localization = null,
        bool startRemoteClients = true)
    {
        this.options = options;
        this.localization = localization ?? DesktopLocalization.ForVerification();
        this.startRemoteClients = startRemoteClients;
        credentialSource = tokenSource;
        Width = 1080;
        Height = 760;
        MinWidth = 640;
        MinHeight = 480;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        renderer = new AvaloniaDocumentRenderer(
            OnActionInvoked,
            this.localization.Resolve);
        eventClient = new RemoteEventClient(options);
        healthClient = new RemoteHealthClient(options);
        authorityHealthCoordinator = new RemoteAuthorityHealthCoordinator(
            healthClient.CheckAsync);
        currentAuthorityHealthState = authorityHealthCoordinator.State;
        leselangClient = new RemoteLeselangClient(options);
        mutationClient = new RemoteMutationClient(options);
        trustIdentity = eventClient.TrustIdentity;
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
        orchestraButton.Click += (_, _) => OpenOrchestraWorkspace();
        debuggerButton.Click += (_, _) => OpenDebuggerWorkspace();
        registrationButton.Click += (_, _) =>
            ObserveUiOperation(OpenRuntimeRegistrationAsync());

        AutomationProperties.SetAutomationId(statusText, "remote-connection-state");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Off);
        AutomationProperties.SetAutomationId(mutationStatusText, "remote-operation-status");
        AutomationProperties.SetLiveSetting(
            mutationStatusText,
            AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(
            dismissMutationButton,
            "remote-operation-dismiss");
        dismissMutationButton.Click += (_, _) => DismissMutationStatus();
        AutomationProperties.SetAutomationId(reconnectButton, "remote-reconnect");
        reconnectButton.Click += (_, _) => RequestReconnect();
        connectionButton.IsVisible = manageConnection is not null;
        connectionButton.Click += (_, _) => manageConnection?.Invoke();
        AutomationProperties.SetAutomationId(connectionButton, "remote-manage-connection");
        AutomationProperties.SetAutomationId(runtimeFilterBox, "remote-runtime-filter");
        AutomationProperties.SetAutomationId(runtimeCountText, "remote-runtime-count");
        AutomationProperties.SetAutomationId(
            clearRuntimeFilterButton,
            "remote-runtime-filter-clear");
        AutomationProperties.SetAutomationId(
            authorityHealthText,
            "remote-authority-health");
        AutomationProperties.SetLiveSetting(
            authorityHealthText,
            AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(
            authorityHealthButton,
            "remote-authority-health-refresh");
        AutomationProperties.SetAutomationId(
            orchestraButton,
            "remote-orchestra-open");
        AutomationProperties.SetAutomationId(
            debuggerButton,
            "remote-debugger-open");
        AutomationProperties.SetAutomationId(
            registrationButton,
            "remote-runtime-registration-open");
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
        ApplyLocalization();
        ApplyResponsiveLayout(RemoteResponsiveLayout.Select(Width));
        ApplyState(currentState);
        ApplyAuthorityHealth(currentAuthorityHealthState);
        eventClient.StateChanged += OnStateChanged;
        this.localization.Changed += OnLocalizationChanged;
        Opened += (_, _) =>
        {
            if (this.startRemoteClients)
            {
                eventClient.Start();
                RefreshAuthorityHealth();
            }
        };
        KeyDown += OnKeyDown;
        SizeChanged += (_, eventArgs) => ApplyResponsiveLayout(
            RemoteResponsiveLayout.Select(eventArgs.NewSize.Width));
        Closed += OnClosed;
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs) =>
        Dispatcher.UIThread.Post(() =>
        {
            _ = sender;
            _ = eventArgs;
            if (isClosed)
            {
                return;
            }
            ApplyLocalization();
            RenderProjection();
        });

    private void ApplyLocalization()
    {
        FlowDirection = localization.FlowDirection;
        Title = DesktopRemoteShellCatalogs.Format(
            localization,
            "title",
            options.Endpoint.Authority);
        dismissMutationButton.Content = localization.Text(DesktopTextKey.Dismiss);
        reconnectButton.Content = localization.Text(DesktopTextKey.Reconnect);
        connectionButton.Content = localization.Text(DesktopTextKey.Connection);
        runtimeFilterBox.PlaceholderText = localization.Text(DesktopTextKey.FilterRuntimes);
        authorityHealthText.Text = localization.Text(DesktopTextKey.AwaitingAuthorityCheck);
        authorityHealthButton.Content = localization.Text(DesktopTextKey.RefreshHealth);
        orchestraButton.Content = DesktopOrchestraCatalogs.Resolve(
            localization,
            "entry.open");
        debuggerButton.Content = DesktopDebuggerCatalogs.Resolve(
            localization,
            "entry.open");
        registrationButton.Content = DesktopRegistrationCatalogs.Resolve(
            localization,
            "entry.open");
        clearRuntimeFilterButton.Content = localization.Text(DesktopTextKey.Clear);
        AutomationProperties.SetName(
            statusText,
            localization.Text(DesktopTextKey.RemoteConnectionState));
        AutomationProperties.SetName(
            mutationStatusText,
            localization.Text(DesktopTextKey.RemoteOperationStatus));
        AutomationProperties.SetName(
            connectionButton,
            localization.Text(DesktopTextKey.ManageConnection));
        AutomationProperties.SetName(
            runtimeFilterBox,
            localization.Text(DesktopTextKey.FilterRuntimes));
        AutomationProperties.SetName(
            runtimeCountText,
            localization.Text(DesktopTextKey.RuntimeResultCount));
        AutomationProperties.SetName(
            clearRuntimeFilterButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "a11y.clear_filter"));
        AutomationProperties.SetName(
            dismissMutationButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "a11y.dismiss"));
        AutomationProperties.SetName(
            reconnectButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "a11y.reconnect"));
        AutomationProperties.SetHelpText(
            reconnectButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "help.reconnect"));
        ToolTip.SetTip(
            reconnectButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "tooltip.reconnect"));
        AutomationProperties.SetHelpText(
            connectionButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "help.connection"));
        AutomationProperties.SetName(
            runtimeFilterBox,
            DesktopRemoteShellCatalogs.Resolve(localization, "a11y.filter"));
        AutomationProperties.SetHelpText(
            runtimeFilterBox,
            DesktopRemoteShellCatalogs.Resolve(localization, "help.filter"));
        AutomationProperties.SetName(
            authorityHealthButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "a11y.health_refresh"));
        AutomationProperties.SetHelpText(
            authorityHealthButton,
            DesktopRemoteShellCatalogs.Resolve(localization, "help.health_refresh"));
        AutomationProperties.SetName(
            orchestraButton,
            DesktopOrchestraCatalogs.Resolve(localization, "a11y.entry"));
        AutomationProperties.SetHelpText(
            orchestraButton,
            DesktopOrchestraCatalogs.Resolve(localization, "help.entry"));
        AutomationProperties.SetName(
            debuggerButton,
            DesktopDebuggerCatalogs.Resolve(localization, "a11y.entry"));
        AutomationProperties.SetHelpText(
            debuggerButton,
            DesktopDebuggerCatalogs.Resolve(localization, "help.entry"));
        AutomationProperties.SetName(
            registrationButton,
            DesktopRegistrationCatalogs.Resolve(localization, "entry.open"));
        AutomationProperties.SetHelpText(
            registrationButton,
            DesktopRegistrationCatalogs.Resolve(localization, "entry.help"));
        ConfigureTrustIdentity(trustIdentity);
        ConfigureCredentialSource(credentialSource);
        ApplyFeedPresentation(currentState);
        ApplyAuthorityHealth(currentAuthorityHealthState);
        ApplyMutationStatus();
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
        runtimeToolbarGrid.Children.Add(registrationButton);
        runtimeToolbarGrid.Children.Add(orchestraButton);
        runtimeToolbarGrid.Children.Add(debuggerButton);
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
        caFingerprintText.Text = DesktopRemoteShellCatalogs.Format(
            localization,
            "identity.ca_short",
            identity.ShortFingerprint);
        AutomationProperties.SetAutomationId(remoteOriginText, "remote-origin");
        AutomationProperties.SetName(
            remoteOriginText,
            DesktopRemoteShellCatalogs.Format(
                localization,
                "a11y.origin",
                identity.Origin));
        AutomationProperties.SetAutomationId(caFingerprintText, "remote-ca-fingerprint");
        AutomationProperties.SetName(
            caFingerprintText,
            DesktopRemoteShellCatalogs.Format(
                localization,
                "a11y.ca",
                identity.Sha256Fingerprint));
        AutomationProperties.SetHelpText(
            caFingerprintText,
            DesktopRemoteShellCatalogs.Resolve(localization, "help.ca"));
        ToolTip.SetTip(
            caFingerprintText,
            DesktopRemoteShellCatalogs.Format(
                localization,
                "tooltip.ca",
                identity.Sha256Fingerprint));
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
            compact ? "*,Auto" : "*,Auto,Auto,Auto,Auto,Auto");
        runtimeToolbarGrid.RowDefinitions = RowDefinitions.Parse(
            compact ? "Auto,Auto,Auto,Auto,Auto" : "Auto");
        Grid.SetColumn(runtimeFilterBox, 0);
        Grid.SetRow(runtimeFilterBox, 0);
        Grid.SetColumnSpan(runtimeFilterBox, compact ? 2 : 1);
        Grid.SetColumn(clearRuntimeFilterButton, compact ? 0 : 1);
        Grid.SetRow(clearRuntimeFilterButton, compact ? 1 : 0);
        Grid.SetColumn(runtimeCountText, compact ? 1 : 2);
        Grid.SetRow(runtimeCountText, compact ? 1 : 0);
        Grid.SetColumn(registrationButton, compact ? 0 : 3);
        Grid.SetRow(registrationButton, compact ? 2 : 0);
        Grid.SetColumnSpan(registrationButton, compact ? 2 : 1);
        Grid.SetColumn(orchestraButton, compact ? 0 : 4);
        Grid.SetRow(orchestraButton, compact ? 3 : 0);
        Grid.SetColumnSpan(orchestraButton, compact ? 2 : 1);
        Grid.SetColumn(debuggerButton, compact ? 0 : 5);
        Grid.SetRow(debuggerButton, compact ? 4 : 0);
        Grid.SetColumnSpan(debuggerButton, compact ? 2 : 1);
        clearRuntimeFilterButton.Margin = compact
            ? new Thickness(0, 8, 0, 0)
            : new Thickness(0);
        runtimeCountText.Margin = compact
            ? new Thickness(0, 8, 0, 0)
            : new Thickness(0);
        registrationButton.Margin = compact
            ? new Thickness(0, 8, 0, 0)
            : new Thickness(0);
        orchestraButton.Margin = compact
            ? new Thickness(0, 6, 0, 0)
            : new Thickness(0);
        debuggerButton.Margin = compact
            ? new Thickness(0, 6, 0, 0)
            : new Thickness(0);

        statusGrid.ColumnDefinitions = ColumnDefinitions.Parse(
            compact ? "*,Auto" : "*,Auto,Auto,Auto,Auto");
        statusGrid.RowDefinitions = RowDefinitions.Parse(
            compact ? "Auto,Auto,Auto" : "Auto");
        Grid.SetColumn(statusText, 0);
        Grid.SetRow(statusText, 0);
        Grid.SetColumnSpan(statusText, compact ? 2 : 1);
        statusText.Margin = compact
            ? new Thickness(0, 0, 0, 8)
            : new Thickness(0);
        Grid.SetColumn(credentialSourceBadge, compact ? 0 : 1);
        Grid.SetRow(credentialSourceBadge, compact ? 1 : 0);
        Grid.SetColumn(revisionText, compact ? 1 : 2);
        Grid.SetRow(revisionText, compact ? 1 : 0);
        Grid.SetColumn(connectionButton, compact ? 0 : 3);
        Grid.SetRow(connectionButton, compact ? 2 : 0);
        Grid.SetColumn(reconnectButton, compact ? 1 : 4);
        Grid.SetRow(reconnectButton, compact ? 2 : 0);
        connectionButton.Margin = compact
            ? new Thickness(0, 8, 8, 0)
            : new Thickness(0);
        reconnectButton.Margin = compact
            ? new Thickness(0, 8, 0, 0)
            : new Thickness(0);
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("remote shell has no control root");
        }
        foreach (var (width, height, density) in new[]
        {
            (MinWidth, MinHeight, RemoteLayoutDensity.Compact),
            (1080d, 760d, RemoteLayoutDensity.Wide),
        })
        {
            ApplyResponsiveLayout(density);
            root.Measure(new Size(width, height));
            var desired = root.DesiredSize;
            if (!double.IsFinite(desired.Width)
                || !double.IsFinite(desired.Height)
                || desired.Width <= 0
                || desired.Height <= 0
                || desired.Width > width
                || desired.Height > height)
            {
                throw new InvalidDataException(
                    "remote shell controls exceeded their layout envelope");
            }
            if (density == RemoteLayoutDensity.Compact
                && (Grid.GetRow(connectionButton) != 2
                    || Grid.GetRow(reconnectButton) != 2
                    || Grid.GetColumnSpan(statusText) != 2
                    || Grid.GetRow(registrationButton) != 2
                    || Grid.GetColumnSpan(registrationButton) != 2
                    || Grid.GetRow(orchestraButton) != 3
                    || Grid.GetColumnSpan(orchestraButton) != 2
                    || Grid.GetRow(debuggerButton) != 4
                    || Grid.GetColumnSpan(debuggerButton) != 2))
            {
                throw new InvalidDataException(
                    "remote shell compact status controls can overlap");
            }
            if (density == RemoteLayoutDensity.Wide
                && (Grid.GetRow(connectionButton) != 0
                    || Grid.GetRow(reconnectButton) != 0
                    || Grid.GetColumnSpan(statusText) != 1
                    || Grid.GetColumn(registrationButton) != 3
                    || Grid.GetColumnSpan(registrationButton) != 1
                    || Grid.GetColumn(orchestraButton) != 4
                    || Grid.GetColumnSpan(orchestraButton) != 1
                    || Grid.GetColumn(debuggerButton) != 5
                    || Grid.GetColumnSpan(debuggerButton) != 1))
            {
                throw new InvalidDataException(
                    "remote shell wide status layout drifted");
            }
        }
        ApplyResponsiveLayout(RemoteResponsiveLayout.Select(Width));
    }

    public void ProbeTypedPresentation()
    {
        ApplyState(new RemoteFeedState(
            RemoteFeedPhase.Live,
            42,
            Array.Empty<RemoteRuntimeProjection>(),
            0,
            false,
            "opaque feed detail must not be presented",
            4,
            42));
        var health = new RemoteHealth(
            "ready",
            true,
            1,
            new RemoteEffectQueueHealth(2, 1, 4, 0, 3, 4, 16, false));
        const string compatibilityLabel = "core compatibility label must not render";
        ApplyAuthorityHealth(new RemoteAuthorityHealthState(
            1,
            RemoteAuthorityHealthPhase.Ready,
            RemoteAuthorityHealthFailure.None,
            compatibilityLabel,
            "core compatibility automation name must not render",
            false,
            false,
            health));
        SetMutationStatus(
            DesktopRemoteText.Operation("status.operation_failed", "fixture"),
            LeserpentTheme.Destructive);
        if (statusText.Text?.Contains("opaque feed detail", StringComparison.Ordinal)
                == true
            || authorityHealthText.Text == compatibilityLabel)
        {
            throw new InvalidDataException(
                "remote shell presentation bypassed its typed localization boundary");
        }
    }

    public void ProbeLocalizedPresentation(
        string expectedTitle,
        string expectedFeed,
        string expectedRevision,
        string expectedHealth,
        string expectedCredential,
        string expectedOperation)
    {
        var failures = new List<string>();
        if (Title != expectedTitle) failures.Add("title");
        if (statusText.Text != expectedFeed) failures.Add("feed");
        if (revisionText.Text != expectedRevision) failures.Add("revision");
        if (authorityHealthText.Text != expectedHealth) failures.Add("health");
        if (credentialSourceText.Text != expectedCredential) failures.Add("credential");
        if (mutationStatusText.Text != expectedOperation) failures.Add("operation");
        if (AutomationProperties.GetName(statusText)
            != DesktopRemoteShellCatalogs.Format(
                localization,
                "a11y.status",
                expectedFeed))
        {
            failures.Add("feed-a11y");
        }
        if (AutomationProperties.GetName(authorityHealthText)
            != DesktopRemoteShellCatalogs.Format(
                localization,
                "a11y.health",
                expectedHealth))
        {
            failures.Add("health-a11y");
        }
        if (FlowDirection != localization.FlowDirection) failures.Add("direction");
        if (failures.Count > 0)
        {
            throw new InvalidDataException(
                $"remote shell localized presentation drifted: {string.Join(',', failures)}");
        }
    }

    private void ConfigureCredentialSource(RemoteTokenSource source)
    {
        var presentation = DesktopRemotePresentation.Credential(source);
        credentialSourceText.Text = presentation.Label.Resolve(localization);
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
            presentation.AutomationName.Resolve(localization));
        AutomationProperties.SetHelpText(
            credentialSourceBadge,
            presentation.Help.Resolve(localization));
        ToolTip.SetTip(credentialSourceBadge, presentation.Help.Resolve(localization));
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
        ApplyFeedPresentation(state);
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

    private void ApplyFeedPresentation(RemoteFeedState state)
    {
        statusText.Text = DesktopRemotePresentation.Feed(state).Resolve(localization);
        statusText.Foreground = state.Phase switch
        {
            RemoteFeedPhase.Live => LeserpentTheme.Accent,
            RemoteFeedPhase.Stale => LeserpentTheme.Destructive,
            RemoteFeedPhase.Reconnecting => LeserpentTheme.Primary,
            _ => LeserpentTheme.Muted,
        };
        AutomationProperties.SetName(
            statusText,
            DesktopRemoteShellCatalogs.Format(
                localization,
                "a11y.status",
                statusText.Text));
        revisionText.Text = DesktopRemotePresentation.Revision(state).Resolve(localization);
        reconnectButton.IsEnabled = state.Phase is RemoteFeedPhase.Stale
            or RemoteFeedPhase.Stopped;
        UpdateRegistrationAvailability();
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
                DesktopRemoteText.Operation("status.pending_unavailable"),
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
                    DesktopRemoteText.Operation(
                        "status.workspace_removed",
                        SafeDisplay(decision.RuntimeId)),
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
                    DesktopRemoteText.Operation(
                        "status.workspace_waiting",
                        SafeDisplay(decision.RuntimeId)),
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
        runtimeCountText.Text = DesktopRemotePresentation.RuntimeCount(
            projection.VisibleRuntimeCount,
            projection.TotalRuntimeCount).Resolve(localization);
        AutomationProperties.SetName(
            runtimeCountText,
            DesktopRemotePresentation.RuntimeCount(
                projection.VisibleRuntimeCount,
                projection.TotalRuntimeCount,
                automationName: true).Resolve(localization));
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
        currentAuthorityHealthState = state;
        authorityHealthText.Text = DesktopRemotePresentation.AuthorityHealth(state)
            .Resolve(localization);
        authorityHealthText.Foreground = state.RequiresAttention
            ? LeserpentTheme.Destructive
            : state.Phase == RemoteAuthorityHealthPhase.Ready
                ? LeserpentTheme.Accent
                : LeserpentTheme.Muted;
        AutomationProperties.SetName(
            authorityHealthText,
            DesktopRemoteShellCatalogs.Format(
                localization,
                "a11y.health",
                authorityHealthText.Text));
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
        SetMutationStatus(
            DesktopRemoteText.Operation("status.reconnect_starting"),
            LeserpentTheme.Primary);
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
                DesktopRemoteText.Operation(
                    "status.reconnect_blocked",
                    SafeDisplay(error.Message)),
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.RemoteActionLabel,
                    DesktopRemoteText.Operation("reason.source_closed")),
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.RemoteActionLabel,
                    resolution.Reason is null
                        ? DesktopRemoteText.Shell("unavailable_value")
                        : SafeDisplay(resolution.Reason)),
                LeserpentTheme.Destructive);
            return;
        }
        var runtime = intent.Runtime;
        if (intent.Kind == ActionKind.RuntimeInspect)
        {
            var workspaceDecision = workspaceLaunch.Request(
                runtime.Id,
                currentState.SnapshotRevision ?? runtime.Revision,
                currentState,
                workspaceWindows.Keys);
            var workspaceError = ApplyWorkspaceLaunchDecision(workspaceDecision);
            if (workspaceError is not null)
            {
                SetMutationStatus(
                    DesktopRemoteText.Operation(
                        "status.operation_blocked",
                        DesktopRemotePresentation.WorkspaceLabel,
                        DesktopRemotePresentation.WorkspaceReason(
                            workspaceDecision.Disposition,
                            MaxOpenWorkspaces)),
                    LeserpentTheme.Destructive);
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.RemoteActionLabel,
                    DesktopRemoteText.Operation("reason.unsupported_action")),
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.MutationLabel(
                        refreshCapabilities
                            ? RemoteMutationKind.CapabilityRefresh
                            : RemoteMutationKind.Refresh),
                    DesktopRemotePresentation.AdmissionReason(admission.Failure)),
                LeserpentTheme.Destructive);
            return;
        }
        UpdateMutationAvailability();
        var confirmed = await new RuntimeRefreshConfirmationWindow(
                runtime,
                refreshCapabilities,
                localization,
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
                DesktopRemoteText.Operation(
                    "status.confirmation_blocked",
                    DesktopRemotePresentation.MutationLabel(operation.Request.Kind),
                    DesktopRemotePresentation.AdmissionReason(confirmation.Failure)),
                LeserpentTheme.Destructive);
            return;
        }
        SetMutationStatus(
            DesktopRemoteText.Operation(
                "status.operation_starting",
                DesktopRemotePresentation.MutationLabel(operation.Request.Kind),
                SafeDisplay(runtime.Name),
                runtime.Revision),
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
                DesktopRemoteText.Operation(
                    "status.operation_accepted",
                    DesktopRemotePresentation.MutationLabel(operation.Request.Kind),
                    SafeDisplay(runtime.Name),
                    result.Revision),
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                    DesktopRemoteText.Operation("reason.missing_form")),
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                    DesktopRemotePresentation.AdmissionReason(admission.Failure)),
                LeserpentTheme.Destructive);
            return;
        }
        UpdateMutationAvailability();
        var formWindow = new ParameterizedActionFormWindow(
            form,
            DesktopRemoteOperationCatalogs.Format(
                localization,
                "deployment.context",
                SafeDisplay(runtime.Name),
                SafeDisplay(runtime.Id),
                runtime.Revision),
            DesktopRemoteOperationCatalogs.Resolve(
                localization,
                "deployment.warning"),
            localization,
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                    SafeDisplay(error.Message)),
                LeserpentTheme.Destructive);
            return;
        }
        if (!IsActiveActionSource(sourceRenderer))
        {
            mutationCoordinator.Cancel(operation);
            UpdateMutationAvailability();
            SetMutationStatus(
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                    DesktopRemoteText.Operation("reason.source_closed")),
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                    submitted.Reason is null
                        ? DesktopRemoteText.Shell("unavailable_value")
                        : SafeDisplay(submitted.Reason)),
                LeserpentTheme.Destructive);
            return;
        }
        var target = submittedIntent.Target;
        var confirmation = mutationCoordinator.Confirm(operation, currentState);
        if (!confirmation.Accepted)
        {
            UpdateMutationAvailability();
            SetMutationStatus(
                DesktopRemoteText.Operation(
                    "status.confirmation_blocked",
                    DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                    DesktopRemotePresentation.AdmissionReason(confirmation.Failure)),
                LeserpentTheme.Destructive);
            return;
        }
        SetMutationStatus(
            DesktopRemoteText.Operation(
                "status.operation_starting",
                DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                SafeDisplay(runtime.Name),
                runtime.Revision),
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
                DesktopRemoteText.Operation(
                    "status.operation_accepted",
                    DesktopRemotePresentation.MutationLabel(RemoteMutationKind.Deployment),
                    SafeDisplay(runtime.Name),
                    result.Revision),
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
        registrationMutationEnabled = availability.MutationsEnabled;
        registrationMutationUnavailableReason = availability.MutationUnavailableReason;
        registrationWindow?.SetMutationAvailability(registrationMutationEnabled);
        UpdateRegistrationAvailability();
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
            && failure.Kind is not (
                RemoteMutationFailureKind.OwnerCancelled
                or RemoteMutationFailureKind.StaleOperation))
        {
            var unavailable = DesktopRemoteShellCatalogs.Resolve(
                localization,
                "unavailable_value");
            SetMutationStatus(
                DesktopRemotePresentation.MutationFailure(
                    failure,
                    DesktopRemotePresentation.MutationLabel(operation.Request.Kind),
                    unavailable),
                LeserpentTheme.Destructive);
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.WorkspaceLabel,
                    DesktopRemoteText.Operation("reason.not_live")),
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
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    DesktopRemotePresentation.WorkspaceLabel,
                    DesktopRemoteText.Operation(
                        "reason.workspace_capacity",
                        MaxOpenWorkspaces)),
                LeserpentTheme.Destructive);
            return;
        }
        var workspace = new RemoteRuntimeWorkspaceWindow(
            options,
            runtime,
            principal,
            OnActionInvoked,
            localization);
        workspaceWindows.Add(runtime.Id, workspace);
        workspace.Closed += (_, _) => workspaceWindows.Remove(runtime.Id);
        SetWorkspaceMutationAvailability(
            workspace,
            mutationCoordinator.Availability(currentState));
        workspace.Show(this);
    }

    private void OpenOrchestraWorkspace()
    {
        if (orchestraWorkspace is not null)
        {
            orchestraWorkspace.Activate();
            return;
        }
        var workspace = new RemoteOrchestraWorkspaceWindow(
            options,
            principal,
            localization);
        orchestraWorkspace = workspace;
        workspace.Closed += (_, _) =>
        {
            if (ReferenceEquals(orchestraWorkspace, workspace))
            {
                orchestraWorkspace = null;
            }
        };
        workspace.Show(this);
    }

    private void OpenDebuggerWorkspace()
    {
        if (debuggerWorkspace is not null)
        {
            debuggerWorkspace.Activate();
            return;
        }
        var workspace = new RemoteDebuggerWindow(
            options,
            principal,
            localization,
            renderer.ApplyPresentationAsync);
        debuggerWorkspace = workspace;
        workspace.Closed += (_, _) =>
        {
            if (ReferenceEquals(debuggerWorkspace, workspace))
            {
                debuggerWorkspace = null;
            }
        };
        workspace.Show(this);
    }

    private async Task OpenRuntimeRegistrationAsync()
    {
        if (registrationWindow is not null)
        {
            registrationWindow.Activate();
            return;
        }
        if (currentState.Phase != RemoteFeedPhase.Live
            || currentState.IsStale
            || !registrationMutationEnabled
            || isClosed)
        {
            object reason = string.IsNullOrWhiteSpace(
                registrationMutationUnavailableReason)
                ? DesktopRemoteText.Operation("reason.not_live")
                : SafeDisplay(registrationMutationUnavailableReason);
            SetMutationStatus(
                DesktopRemoteText.Operation(
                    "status.operation_blocked",
                    new DesktopRemoteSemanticValue(
                        "desktop.registration.entry.open",
                        "Register existing runtime"),
                    reason),
                LeserpentTheme.Destructive);
            return;
        }
        var window = new RuntimeRegistrationWindow(
            options,
            principal,
            localization);
        window.SetMutationAvailability(registrationMutationEnabled);
        registrationWindow = window;
        registrationButton.IsEnabled = false;
        window.Closed += (_, _) =>
        {
            if (ReferenceEquals(registrationWindow, window))
            {
                registrationWindow = null;
                ApplyFeedPresentation(currentState);
            }
        };
        var result = await window.ShowDialog<RemoteRegistrationResult?>(this);
        if (result is not null && !isClosed)
        {
            SetMutationStatus(
                DesktopRemoteText.Operation(
                    "status.operation_accepted",
                    new DesktopRemoteSemanticValue(
                        "desktop.registration.action.apply.register",
                        "Register runtime"),
                    SafeDisplay(result.RuntimeId),
                    result.Revision),
                LeserpentTheme.Accent);
        }
    }

    private void UpdateRegistrationAvailability() =>
        registrationButton.IsEnabled = currentState.Phase == RemoteFeedPhase.Live
            && !currentState.IsStale
            && registrationMutationEnabled
            && registrationWindow is null
            && !isClosed;

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
        localization.Changed -= OnLocalizationChanged;
        authorityHealthCoordinator.Stop();
        lifetime.Cancel();
        runtimeFilterTimer.Stop();
        foreach (var workspace in workspaceWindows.Values.ToArray())
        {
            workspace.Close();
        }
        orchestraWorkspace?.Close();
        orchestraWorkspace = null;
        debuggerWorkspace?.Close();
        debuggerWorkspace = null;
        registrationWindow?.Close();
        registrationWindow = null;
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
                DesktopRemoteText.Operation(
                    "status.operation_failed",
                    SafeDisplay(error.Message)),
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

    private void SetMutationStatus(DesktopRemoteText notice, IBrush foreground)
    {
        mutationNotice = notice;
        mutationNoticeForeground = foreground;
        ApplyMutationStatus();
        mutationStatusBar.IsVisible = true;
    }

    private void ApplyMutationStatus()
    {
        if (mutationNotice is null)
        {
            AutomationProperties.SetName(
                mutationStatusText,
                localization.Text(DesktopTextKey.RemoteOperationStatus));
            return;
        }
        var text = mutationNotice.Resolve(localization);
        mutationStatusText.Text = text;
        var foreground = mutationNoticeForeground;
        mutationStatusText.Foreground = foreground;
        AutomationProperties.SetName(
            mutationStatusText,
            DesktopRemoteShellCatalogs.Format(
                localization,
                "a11y.operation",
                text));
        AutomationProperties.SetLiveSetting(
            mutationStatusText,
            ReferenceEquals(foreground, LeserpentTheme.Destructive)
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
    }

    private void DismissMutationStatus()
    {
        mutationStatusBar.IsVisible = false;
        mutationStatusText.Text = string.Empty;
        mutationNotice = null;
        AutomationProperties.SetName(
            mutationStatusText,
            localization.Text(DesktopTextKey.RemoteOperationStatus));
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
        DesktopLocalization localization,
        Func<CancellationToken, Task<string>> exportLeselang)
    {
        var action = localization.Resolve(new LocalizedText
        {
            Key = refreshCapabilities
                ? "runtime.capabilities.refresh"
                : "runtime.refresh",
            Fallback = refreshCapabilities ? "Discover capabilities" : "Refresh runtime",
        });
        Title = DesktopRemoteOperationCatalogs.Resolve(
            localization,
            refreshCapabilities
                ? "confirm.capabilities.title"
                : "confirm.refresh.title");
        Width = 480;
        MinWidth = 420;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        FlowDirection = localization.FlowDirection;

        var cancel = new Button
        {
            Content = localization.Text(DesktopTextKey.Cancel),
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
            exportLeselang,
            localization);
        AutomationProperties.SetAutomationId(
            cancel,
            refreshCapabilities
                ? "runtime-capabilities-refresh-cancel"
                : "runtime-refresh-cancel");
        AutomationProperties.SetName(cancel, localization.Text(DesktopTextKey.Cancel));
        AutomationProperties.SetAutomationId(
            confirm,
            refreshCapabilities
                ? "runtime-capabilities-refresh-confirm"
                : "runtime-refresh-confirm");
        AutomationProperties.SetName(confirm, action);
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

        var buttons = new WrapPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            ItemWidth = double.NaN,
            ItemHeight = double.NaN,
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
                            ? DesktopRemoteOperationCatalogs.Resolve(
                                localization,
                                "confirm.capabilities.heading")
                            : DesktopRemoteOperationCatalogs.Resolve(
                                localization,
                                "confirm.refresh.heading"),
                        Foreground = LeserpentTheme.Primary,
                        FontSize = 22,
                        FontWeight = FontWeight.Bold,
                    },
                    new TextBlock
                    {
                        Text = DesktopRemoteOperationCatalogs.Format(
                            localization,
                            "confirm.context",
                            Safe(runtime.Name),
                            Safe(runtime.Id),
                            runtime.Revision),
                        Foreground = LeserpentTheme.Body,
                        FontSize = 14,
                        LineHeight = 22,
                        TextWrapping = TextWrapping.Wrap,
                    },
                    new TextBlock
                    {
                        Text = DesktopRemoteOperationCatalogs.Resolve(
                            localization,
                            "confirm.warning"),
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

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException(
                "runtime refresh confirmation has no control root");
        }
        root.Measure(new Size(Width, 900));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 900)
        {
            throw new InvalidDataException(
                "runtime refresh confirmation exceeded its layout envelope");
        }
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
        DesktopLocalization localization,
        Func<IReadOnlyDictionary<string, string>, CancellationToken, Task<string>> exportLeselang)
    {
        Title = localization.Resolve(form.Title);
        Width = 520;
        MinWidth = 440;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        FlowDirection = localization.FlowDirection;

        var inputs = new Dictionary<string, (UiFormField Field, TextBox Input)>(
            StringComparer.Ordinal);
        var fields = new StackPanel { Spacing = 8 };
        foreach (var field in form.Fields)
        {
            var label = localization.Resolve(field.Label);
            var input = new TextBox
            {
                PlaceholderText = field.Placeholder is null
                    ? null
                    : localization.Resolve(field.Placeholder),
                MaxLength = field.MaxLength,
            };
            AutomationProperties.SetAutomationId(input, $"parameter-form-{field.Key}");
            AutomationProperties.SetName(input, label);
            fields.Children.Add(new TextBlock
            {
                Text = field.Required
                    ? localization.Format(DesktopTextKey.RequiredSuffix, label)
                    : label,
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
            Content = localization.Text(DesktopTextKey.Cancel),
            Padding = new Thickness(18, 9),
        };
        SubmitButton = new Button
        {
            Content = localization.Resolve(form.SubmitLabel),
            Background = LeserpentTheme.Accent,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(18, 9),
            IsEnabled = false,
        };
        var cancel = CancelButton;
        var submit = SubmitButton;
        AutomationProperties.SetAutomationId(cancel, "parameter-form-cancel");
        AutomationProperties.SetName(cancel, localization.Text(DesktopTextKey.Cancel));
        AutomationProperties.SetAutomationId(submit, "parameter-form-submit");
        AutomationProperties.SetName(submit, localization.Resolve(form.SubmitLabel));
        var leselang = new LeselangExportControl(
            "parameter-form",
            localization: localization);

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
                : $"{localization.Resolve(invalid.Field.Label)}: {ValidationMessage(invalid.Field, localization)}";
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

        var buttons = new WrapPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            ItemWidth = double.NaN,
            ItemHeight = double.NaN,
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
                        Text = localization.Resolve(form.Title),
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

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException(
                "parameterized remote form has no control root");
        }
        root.Measure(new Size(Width, 1200));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 1200)
        {
            throw new InvalidDataException(
                "parameterized remote form exceeded its layout envelope");
        }
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

    private static string ValidationMessage(
        UiFormField field,
        DesktopLocalization localization) => field.InputKind switch
    {
        UiFormInputKind.PathToken =>
            DesktopRemoteOperationCatalogs.Resolve(localization, "validation.path"),
        UiFormInputKind.TrimmedText =>
            DesktopRemoteOperationCatalogs.Resolve(localization, "validation.trimmed"),
        _ => DesktopRemoteOperationCatalogs.Resolve(
            localization,
            "validation.unsupported"),
    };

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
