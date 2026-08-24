using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed class HubWindow : Window
{
    private const int MaxVisibleRuntimesPerDaemon = 6;
    private readonly List<Control> auditedControls = [];
    private readonly List<DaemonTopologyCard> topologyCards = [];
    private readonly RemoteTopologyRefreshCoordinator topologyRefresh = new();
    private readonly CancellationTokenSource lifetime = new();
    private Task<RemoteTopologyRefreshSummary>? refreshAllPresentationOperation;
    private bool operatorRefreshRequested;
    private readonly DispatcherTimer topologyRefreshTimer = new()
    {
        Interval = TimeSpan.FromSeconds(30),
    };
    private readonly DispatcherTimer topologyFilterTimer = new()
    {
        Interval = TimeSpan.FromMilliseconds(160),
    };
    private readonly int daemonCardCount;
    private readonly int expectedAuditedControlCount;
    private readonly DesktopLocalization localization;
    private readonly SilvortexAccountControl accountControl;
    private readonly StackPanel topologyRoot = new() { Spacing = 10 };
    private readonly TextBox topologyFilterBox = new()
    {
        MaxLength = RemoteRuntimeSearch.MaxFilterLength,
        PlaceholderText = string.Empty,
    };
    private readonly Button clearTopologyFilterButton = new()
    {
        Content = string.Empty,
        IsVisible = false,
        Padding = new Thickness(12, 7),
    };
    private readonly Button refreshAllTopologyButton = new()
    {
        Content = string.Empty,
        Padding = new Thickness(12, 7),
    };
    private readonly Button tutorialButton = new()
    {
        Content = string.Empty,
        HorizontalAlignment = HorizontalAlignment.Left,
        Padding = new Thickness(14, 7),
    };
    private readonly Button languageButton = new()
    {
        HorizontalAlignment = HorizontalAlignment.Left,
        Padding = new Thickness(14, 7),
    };
    private readonly TextBlock topologyFilterSummary = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 12,
        FontWeight = FontWeight.SemiBold,
        VerticalAlignment = VerticalAlignment.Center,
    };
    private readonly Border topologyFilterEmpty = new()
    {
        Background = LeserpentTheme.Panel,
        BorderBrush = LeserpentTheme.PanelBorder,
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(10),
        IsVisible = false,
        Padding = new Thickness(18, 15),
        Child = new TextBlock
        {
            Text = string.Empty,
            Foreground = LeserpentTheme.Muted,
            TextWrapping = TextWrapping.Wrap,
        },
    };
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
        IsVisible = false,
    };

    public HubWindow(
        IReadOnlyList<DesktopDaemonConnection> connections,
        bool localSupported,
        string? initialError,
        Func<string?> openLocal,
        Func<DesktopDaemonConnection, string?> openRemote,
        Func<RemoteRuntimeProjection, ulong, string?> openLocalRuntime,
        Func<DesktopDaemonConnection, RemoteRuntimeProjection, ulong, string?>
            openRemoteRuntime,
        Func<CancellationToken, Task<RemoteTopologySnapshot>> loadLocalTopology,
        Func<DesktopDaemonConnection, CancellationToken, Task<RemoteTopologySnapshot>>
            loadRemoteTopology,
        Action deployDaemon,
        Action retireDaemon,
        Action provisionRuntime,
        Action retireRuntime,
        Action addConnection,
        Action<DesktopDaemonConnection> manageConnection,
        Action openTutorial,
        Action openLanguage,
        DesktopLocalization localization,
        SilvortexAccountSession accountSession)
    {
        this.localization = localization;
        daemonCardCount = connections.Count + (localSupported ? 1 : 0);
        expectedAuditedControlCount = 14 + connections.Count * 3 + (localSupported ? 2 : 0);
        topologyFilterBox.PlaceholderText = localization.Text(DesktopTextKey.FindDaemonOrRuntime);
        clearTopologyFilterButton.Content = localization.Text(DesktopTextKey.Clear);
        refreshAllTopologyButton.Content = localization.Text(DesktopTextKey.RefreshAll);
        tutorialButton.Content = localization.Text(DesktopTextKey.QuickTour);
        languageButton.Content = localization.Text(DesktopTextKey.Language);
        ((TextBlock)topologyFilterEmpty.Child!).Text =
            localization.Text(DesktopTextKey.NoTopologyMatches);
        refreshAllTopologyButton.IsEnabled = daemonCardCount > 0;
        Title = "Leserpent / Hub";
        Width = 900;
        Height = 680;
        MinWidth = 680;
        MinHeight = 520;
        SizeToContent = SizeToContent.Manual;
        WindowStartupLocation = WindowStartupLocation.CenterScreen;
        CanResize = true;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        FlowDirection = localization.FlowDirection;

        var addButton = new Button
        {
            Content = localization.Text(DesktopTextKey.AddDaemon),
            Background = LeserpentTheme.Accent,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(addButton, "hub-add-daemon");
        AutomationProperties.SetName(addButton, HubText("a11y.add_daemon"));
        auditedControls.Add(addButton);
        addButton.Click += (_, _) => addConnection();

        var deployButton = new Button
        {
            Content = localization.Text(DesktopTextKey.DeployDaemon),
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(deployButton, "hub-deploy-daemon");
        AutomationProperties.SetName(deployButton, HubText("a11y.deploy_daemon"));
        auditedControls.Add(deployButton);
        deployButton.Click += (_, _) => deployDaemon();

        var retireDaemonButton = new Button
        {
            Content = localization.Text(DesktopTextKey.RetireDaemon),
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(retireDaemonButton, "hub-retire-daemon");
        AutomationProperties.SetName(
            retireDaemonButton,
            HubText("a11y.retire_daemon"));
        auditedControls.Add(retireDaemonButton);
        retireDaemonButton.Click += (_, _) => retireDaemon();

        var provisionButton = new Button
        {
            Content = localization.Text(DesktopTextKey.ProvisionGewyvern),
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(provisionButton, "hub-provision-gewyvern");
        AutomationProperties.SetName(provisionButton, HubText("a11y.provision_runtime"));
        auditedControls.Add(provisionButton);
        provisionButton.Click += (_, _) => provisionRuntime();

        var retireButton = new Button
        {
            Content = localization.Text(DesktopTextKey.RetireGewyvern),
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(retireButton, "hub-retire-gewyvern");
        AutomationProperties.SetName(
            retireButton,
            HubText("a11y.retire_runtime"));
        auditedControls.Add(retireButton);
        retireButton.Click += (_, _) => retireRuntime();

        AutomationProperties.SetAutomationId(tutorialButton, "hub-open-tutorial");
        AutomationProperties.SetName(tutorialButton, HubText("a11y.tutorial"));
        AutomationProperties.SetHelpText(
            tutorialButton,
            HubText("help.tutorial"));
        ToolTip.SetTip(tutorialButton, HubText("tooltip.tutorial"));
        auditedControls.Add(tutorialButton);
        tutorialButton.Click += (_, _) => openTutorial();

        AutomationProperties.SetAutomationId(languageButton, "hub-open-language");
        AutomationProperties.SetName(
            languageButton,
            localization.Text(DesktopTextKey.LanguagePreference));
        AutomationProperties.SetHelpText(
            languageButton,
            localization.Text(DesktopTextKey.AppliesImmediately));
        auditedControls.Add(languageButton);
        languageButton.Click += (_, _) => openLanguage();

        var headingActions = new StackPanel
        {
            Spacing = 8,
            HorizontalAlignment = HorizontalAlignment.Right,
            Children =
            {
                new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 9,
                    HorizontalAlignment = HorizontalAlignment.Right,
                    Children = { deployButton, retireDaemonButton },
                },
                new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 9,
                    HorizontalAlignment = HorizontalAlignment.Right,
                    Children = { provisionButton, retireButton, addButton },
                },
            },
        };

        var heading = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto"),
            RowSpacing = 14,
        };
        accountControl = new SilvortexAccountControl(accountSession, localization);
        auditedControls.AddRange(accountControl.AuditedControls);
        ConfigureTopologyFilter();
        var headingIdentity = new StackPanel
        {
            Spacing = 5,
            Children =
            {
                new TextBlock
                {
                    Text = "LESERPENT",
                    Foreground = LeserpentTheme.Accent,
                    FontSize = 13,
                    FontWeight = FontWeight.Bold,
                    LetterSpacing = 2.2,
                },
                new TextBlock
                {
                    Text = localization.Text(DesktopTextKey.ControlTopology),
                    Foreground = LeserpentTheme.Primary,
                    FontSize = 31,
                    FontWeight = FontWeight.Bold,
                },
                new TextBlock
                {
                    Text = localization.Text(DesktopTextKey.HubSubcopy),
                    Foreground = LeserpentTheme.Muted,
                    FontSize = 13,
                    TextWrapping = TextWrapping.Wrap,
                },
                new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 8,
                    Children = { tutorialButton, languageButton },
                },
            },
        };
        var headingTop = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
            ColumnSpacing = 20,
            Children = { headingIdentity, accountControl },
        };
        Grid.SetColumn(accountControl, 1);
        heading.Children.Add(headingTop);
        Grid.SetRow(headingActions, 1);
        heading.Children.Add(headingActions);

        topologyRoot.Children.Add(CreateClientRoot(connections.Count, localSupported));

        var branch = new Border
        {
            Width = 2,
            Height = 18,
            Background = LeserpentTheme.PanelBorder,
            HorizontalAlignment = HorizontalAlignment.Left,
            Margin = new Thickness(27, 0, 0, 0),
        };
        topologyRoot.Children.Add(branch);
        topologyRoot.Children.Add(topologyFilterEmpty);

        if (localSupported)
        {
            topologyRoot.Children.Add(CreateDaemonCard(
                "local-orchestra",
                localization.Text(DesktopTextKey.LocalOrchestra),
                localization.Text(DesktopTextKey.ManagedOnDevice),
                localization.Text(DesktopTextKey.Local),
                localization.Text(DesktopTextKey.EphemeralSessionAuthority),
                openLocal,
                openLocalRuntime,
                loadLocalTopology,
                null));
        }

        foreach (var connection in connections)
        {
            var captured = connection;
            topologyRoot.Children.Add(CreateDaemonCard(
                connection.DaemonId,
                connection.DisplayName,
                connection.Profile.Endpoint,
                HubText("kind.remote"),
                connection.Profile.BootstrapTrustHandle is { } trustHandle
                    ? $"TRUST  {trustHandle}"
                    : $"CA  {Path.GetFileName(connection.Profile.CertificateAuthorityPath)}",
                () => openRemote(captured),
                (runtime, revision) => openRemoteRuntime(captured, runtime, revision),
                cancellationToken => loadRemoteTopology(captured, cancellationToken),
                () => manageConnection(captured)));
        }

        if (!localSupported && connections.Count == 0)
        {
            topologyRoot.Children.Add(new Border
            {
                Background = LeserpentTheme.Panel,
                BorderBrush = LeserpentTheme.PanelBorder,
                BorderThickness = new Thickness(1),
                CornerRadius = new CornerRadius(10),
                Padding = new Thickness(20),
                Child = new TextBlock
                {
                    Text = localization.Text(DesktopTextKey.NoAuthorities),
                    Foreground = LeserpentTheme.Muted,
                    TextWrapping = TextWrapping.Wrap,
                },
            });
        }

        AutomationProperties.SetAutomationId(statusText, "hub-status");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Polite);
        if (string.IsNullOrWhiteSpace(initialError))
        {
            SetStatus(
                DesktopHubPresentation.Text("status.ready", daemonCardCount),
                LeserpentTheme.Muted);
        }
        else
        {
            SetOpaqueStatus(initialError, LeserpentTheme.Destructive);
        }
        auditedControls.Add(statusText);

        var topologyFilterActions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { clearTopologyFilterButton, refreshAllTopologyButton },
        };
        var topologyFilter = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto"),
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
            RowSpacing = 6,
            ColumnSpacing = 10,
            Children =
            {
                topologyFilterBox,
                topologyFilterActions,
                topologyFilterSummary,
            },
        };
        Grid.SetColumn(topologyFilterActions, 1);
        Grid.SetRow(topologyFilterSummary, 1);
        Grid.SetColumnSpan(topologyFilterSummary, 2);

        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,*,Auto"),
            RowSpacing = 18,
            Margin = new Thickness(34, 28),
            Children =
            {
                heading,
                topologyFilter,
                new ScrollViewer
                {
                    Content = topologyRoot,
                    VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
                    HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
                },
                statusText,
            },
        };
        Grid.SetRow(topologyFilter, 1);
        Grid.SetRow(((Grid)Content).Children[2], 2);
        Grid.SetRow(statusText, 3);
        ApplyTopologyFilter();

        KeyDown += OnHubKeyDown;
        topologyRefreshTimer.Tick += (_, _) => ObserveTopologyOperation(
            RefreshAllTopologiesAsync(TopologyRefreshTrigger.Periodic));
        topologyFilterTimer.Tick += (_, _) => ApplyTopologyFilter();
        Opened += (_, _) =>
        {
            topologyRefreshTimer.Start();
            ObserveTopologyOperation(
                RefreshAllTopologiesAsync(TopologyRefreshTrigger.Startup));
        };
        Closed += (_, _) =>
        {
            topologyRefreshTimer.Stop();
            topologyFilterTimer.Stop();
            lifetime.Cancel();
            accountControl.Dispose();
        };
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("Hub window has no control root");
        }
        root.Measure(new Size(MinWidth, MinHeight));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > MinWidth
            || desired.Height > MinHeight)
        {
            throw new InvalidDataException("Hub controls exceeded their minimum layout envelope");
        }
        accountControl.VerifyLayoutEnvelope();
    }

    public void ProbeLocalizedAccountPresentation(
        string expectedLabel,
        string expectedIdentity,
        string expectedAction,
        string expectedStatus) => accountControl.ProbeLocalizedPresentation(
            expectedLabel,
            expectedIdentity,
            expectedAction,
            expectedStatus);

    public void ProbeLocalizedDynamicPresentation(
        RemoteTopologySnapshot snapshot)
    {
        ArgumentNullException.ThrowIfNull(snapshot);
        foreach (var card in topologyCards)
        {
            card.State.Accept(snapshot);
        }
        ApplyTopologyFilter();
        SetStatus(
            DesktopHubPresentation.Text("status.ready", daemonCardCount),
            LeserpentTheme.Muted);

        var expectedFilter = HubFormat(
            "filter.all",
            daemonCardCount,
            daemonCardCount * snapshot.Runtimes.Count);
        var expectedAuthority = snapshot.Health is { } health
            ? DesktopHubPresentation.AuthorityHealth(health).Resolve(localization)
            : throw new InvalidDataException(
                "localized Hub probe requires authority health proof");
        if (topologyFilterSummary.Text != expectedFilter
            || statusText.Text != HubFormat("status.ready", daemonCardCount))
        {
            throw new InvalidDataException(
                "localized Hub summary presentation drifted");
        }

        foreach (var card in topologyCards)
        {
            var state = card.State.State;
            var expectedSummary = DesktopHubPresentation.TopologySummary(
                state,
                snapshot.Runtimes.Count,
                automationName: false,
                card.Name).Resolve(localization);
            var expectedSummaryName = DesktopHubPresentation.TopologySummary(
                state,
                snapshot.Runtimes.Count,
                automationName: true,
                Safe(card.Name)).Resolve(localization);
            var expectedAuthorityName = HubFormat(
                "a11y.authority",
                Safe(card.Name),
                expectedAuthority);
            var rows = card.RuntimeList.Children.OfType<Button>().ToArray();
            if (card.Summary.Text != expectedSummary
                || AutomationProperties.GetName(card.Summary) != expectedSummaryName
                || card.AuthoritySummary.Text != expectedAuthority
                || AutomationProperties.GetName(card.AuthoritySummary)
                    != expectedAuthorityName
                || rows.Length != snapshot.Runtimes.Count)
            {
                throw new InvalidDataException(
                    "localized Hub daemon-card presentation drifted");
            }

            for (var index = 0; index < rows.Length; index++)
            {
                var runtime = snapshot.Runtimes[index];
                var expectedStatus = DesktopHubPresentation.RuntimeStatus(runtime)
                    .Resolve(localization);
                var status = (rows[index].Content as Grid)?.Children
                    .OfType<TextBlock>()
                    .SingleOrDefault();
                if (status?.Text != expectedStatus
                    || AutomationProperties.GetName(rows[index]) != HubFormat(
                        "a11y.open_runtime",
                        Safe(runtime.Name),
                        Safe(runtime.Id),
                        expectedStatus))
                {
                    throw new InvalidDataException(
                        "localized Hub runtime-row presentation drifted");
                }
            }
        }
    }

    private void ConfigureTopologyFilter()
    {
        AutomationProperties.SetAutomationId(topologyFilterBox, "hub-topology-filter");
        AutomationProperties.SetName(
            topologyFilterBox,
            localization.Text(DesktopTextKey.FindDaemonOrRuntime));
        AutomationProperties.SetHelpText(
            topologyFilterBox,
            HubText("help.filter"));
        AutomationProperties.SetAutomationId(
            clearTopologyFilterButton,
            "hub-topology-filter-clear");
        AutomationProperties.SetName(
            clearTopologyFilterButton,
            localization.Text(DesktopTextKey.Clear));
        AutomationProperties.SetAutomationId(topologyFilterSummary, "hub-topology-filter-summary");
        AutomationProperties.SetName(
            topologyFilterSummary,
            HubText("a11y.filter_summary"));
        AutomationProperties.SetLiveSetting(
            topologyFilterSummary,
            AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(refreshAllTopologyButton, "hub-refresh-all");
        AutomationProperties.SetName(
            refreshAllTopologyButton,
            localization.Text(DesktopTextKey.RefreshAll));
        AutomationProperties.SetHelpText(
            refreshAllTopologyButton,
            HubText("help.refresh_all"));
        ToolTip.SetTip(refreshAllTopologyButton, HubText("tooltip.refresh_all"));
        auditedControls.Add(topologyFilterBox);
        auditedControls.Add(clearTopologyFilterButton);
        auditedControls.Add(topologyFilterSummary);
        auditedControls.Add(refreshAllTopologyButton);
        topologyFilterBox.TextChanged += OnTopologyFilterChanged;
        topologyFilterBox.KeyDown += OnTopologyFilterKeyDown;
        clearTopologyFilterButton.Click += (_, _) => ClearTopologyFilter();
        refreshAllTopologyButton.Click += (_, _) => ObserveTopologyOperation(
            RefreshAllTopologiesAsync(TopologyRefreshTrigger.Operator));
    }

    private void OnHubKeyDown(object? sender, KeyEventArgs eventArgs)
    {
        _ = sender;
        var findModifier = eventArgs.KeyModifiers
            & (KeyModifiers.Control | KeyModifiers.Meta);
        if (eventArgs.Key == Key.F && findModifier != KeyModifiers.None)
        {
            eventArgs.Handled = true;
            topologyFilterBox.Focus();
            topologyFilterBox.SelectAll();
        }
        else if (eventArgs.Key == Key.F5)
        {
            eventArgs.Handled = true;
            ObserveTopologyOperation(
                RefreshAllTopologiesAsync(TopologyRefreshTrigger.Operator));
        }
        else if (eventArgs.Key == Key.F1)
        {
            eventArgs.Handled = true;
            tutorialButton.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        }
        else if (eventArgs.Key == Key.Escape)
        {
            eventArgs.Handled = true;
            if (RemoteRuntimeSearch.Normalize(topologyFilterBox.Text).Length > 0)
            {
                ClearTopologyFilter();
            }
            else
            {
                Close();
            }
        }
    }

    private void OnTopologyFilterChanged(object? sender, TextChangedEventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        var raw = topologyFilterBox.Text ?? string.Empty;
        var sanitized = RemoteRuntimeSearch.SanitizeInput(raw);
        if (!string.Equals(raw, sanitized, StringComparison.Ordinal))
        {
            topologyFilterBox.Text = sanitized;
            topologyFilterBox.CaretIndex = sanitized.Length;
            return;
        }
        clearTopologyFilterButton.IsVisible = RemoteRuntimeSearch.Normalize(sanitized).Length > 0;
        topologyFilterTimer.Stop();
        topologyFilterTimer.Start();
    }

    private void OnTopologyFilterKeyDown(object? sender, KeyEventArgs eventArgs)
    {
        _ = sender;
        if (eventArgs.Key == Key.Escape
            && RemoteRuntimeSearch.Normalize(topologyFilterBox.Text).Length > 0)
        {
            eventArgs.Handled = true;
            ClearTopologyFilter();
        }
    }

    private void ClearTopologyFilter()
    {
        topologyFilterTimer.Stop();
        topologyFilterBox.Text = string.Empty;
        clearTopologyFilterButton.IsVisible = false;
        ApplyTopologyFilter();
        topologyFilterBox.Focus();
    }

    private void ApplyTopologyFilter()
    {
        topologyFilterTimer.Stop();
        var result = RemoteRuntimeSearch.FilterTopology(
            topologyCards.Select(card => new RemoteTopologySearchItem(
                card.DaemonId,
                card.Name,
                card.Kind,
                card.Detail,
                card.State.State.Snapshot?.Runtimes
                    ?? Array.Empty<RemoteRuntimeProjection>())),
            topologyFilterBox.Text);
        var filterActive = result.Filter.Length > 0;
        foreach (var card in topologyCards)
        {
            card.Root.IsVisible = !filterActive
                || result.VisibleAuthorityIds.Contains(card.DaemonId);
            if (card.State.State.Snapshot is not null)
            {
                RenderTopology(
                    card,
                    card.State.State,
                    result.RuntimesByAuthority[card.DaemonId]);
                if (card.State.State.Phase == RemoteTopologyPhase.Retained)
                {
                    card.RuntimeList.Children.Insert(0, RuntimeMessage(
                        HubFormat(
                            "retained.message",
                            card.State.State.ConsecutiveFailures)));
                }
            }
            else if (card.State.State.Phase == RemoteTopologyPhase.Unavailable)
            {
                RenderTopologyFailure(card, card.State.State);
            }
        }

        topologyFilterEmpty.IsVisible = filterActive
            && result.TotalAuthorityCount > 0
            && result.VisibleAuthorityCount == 0;
        var topologyLoading = topologyCards.Any(card =>
            card.State.State.Phase is RemoteTopologyPhase.Awaiting
                or RemoteTopologyPhase.Loading);
        topologyFilterSummary.Text = DesktopHubPresentation.FilterSummary(
            result,
            automationName: false,
            topologyLoading: topologyLoading).Resolve(localization);
        AutomationProperties.SetName(
            topologyFilterSummary,
            DesktopHubPresentation.FilterSummary(
                result,
                automationName: true,
                topologyLoading: topologyLoading).Resolve(localization));
    }

    public void VerifyTopologyContract()
    {
        var ids = new HashSet<string>(StringComparer.Ordinal);
        if (daemonCardCount <= 0
            || auditedControls.Count != expectedAuditedControlCount
            || topologyCards.Count != daemonCardCount
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))
                || !ids.Add(AutomationProperties.GetAutomationId(control)!))
            || !ids.Contains("hub-open-tutorial")
            || !ids.Contains("hub-open-language")
            || ids.Contains("hub-open-remote"))
        {
            throw new InvalidDataException("Hub topology control contract drifted");
        }
    }

    public int RenderedRuntimeCount => topologyCards.Sum(card => card.RenderedRuntimeCount);
    public int LiveTopologyCount => topologyCards.Count(card =>
        card.State.State.Phase == RemoteTopologyPhase.Live);
    public int VerifiedAuthorityCount => topologyCards.Count(card =>
        card.State.State is
        {
            Phase: RemoteTopologyPhase.Live,
            Snapshot.Health: not null,
        });
    public int RenderedRuntimeActionCount => topologyCards.Sum(card =>
        card.RuntimeList.Children.OfType<Button>().Count(button =>
            !string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(button))
            && !string.IsNullOrWhiteSpace(AutomationProperties.GetName(button))));
    public int VisibleDaemonCardCount => topologyCards.Count(card => card.Root.IsVisible);

    public void ProbeTopologyFilter()
    {
        topologyFilterBox.Text = "alpha.example";
        ApplyTopologyFilter();
        if (VisibleDaemonCardCount != 1 || RenderedRuntimeCount != 2)
        {
            throw new InvalidDataException(
                "Hub authority filter did not retain the matching daemon topology");
        }
        topologyFilterBox.Text = "runtime-b";
        ApplyTopologyFilter();
        if (VisibleDaemonCardCount != daemonCardCount
            || RenderedRuntimeCount != daemonCardCount)
        {
            throw new InvalidDataException(
                $"Hub runtime filter did not project matching children across authorities: visible_daemons={VisibleDaemonCardCount}, rendered_runtimes={RenderedRuntimeCount}, expected={daemonCardCount}");
        }
        topologyFilterBox.Text = "does-not-exist";
        ApplyTopologyFilter();
        if (VisibleDaemonCardCount != 0 || !topologyFilterEmpty.IsVisible)
        {
            throw new InvalidDataException("Hub topology filter omitted its empty state");
        }
        ClearTopologyFilter();
        if (VisibleDaemonCardCount != daemonCardCount
            || RenderedRuntimeCount != daemonCardCount * 2
            || topologyFilterEmpty.IsVisible
            || !topologyFilterBox.IsFocused)
        {
            throw new InvalidDataException(
                "Hub topology filter did not restore the complete keyboard workflow");
        }
    }

    public async Task ProbeRefreshAllControlAsync()
    {
        var card = topologyCards.FirstOrDefault()
            ?? throw new InvalidDataException("Hub topology has no daemon card to refresh");
        var cardRefresh = RefreshTopologyAsync(card);
        var cardJoin = RefreshTopologyAsync(card);
        if (!ReferenceEquals(cardRefresh, cardJoin))
        {
            throw new InvalidDataException(
                "Hub daemon refresh did not join its active operation");
        }

        var generation = topologyRefresh.Generation;
        var refreshAll = RefreshAllTopologiesAsync(TopologyRefreshTrigger.Periodic);
        refreshAllTopologyButton.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
        if (!ReferenceEquals(refreshAll, refreshAllPresentationOperation)
            || !topologyRefresh.IsRefreshingAll
            || refreshAllTopologyButton.IsEnabled
            || refreshAllTopologyButton.Content as string
                != HubText("action.refreshing")
            || statusText.Text
                != HubFormat("status.refreshing_all", topologyCards.Count))
        {
            throw new InvalidDataException(
                "Hub refresh-all control did not expose its single-flight busy state");
        }

        await Task.WhenAll(cardRefresh, refreshAll);
        var completedSummary = await refreshAll;
        if (topologyRefresh.Generation != generation + 1
            || refreshAllPresentationOperation is not null
            || topologyRefresh.IsRefreshingAll
            || topologyRefresh.IsAuthorityRefreshing(card.DaemonId)
            || !refreshAllTopologyButton.IsEnabled
            || refreshAllTopologyButton.Content as string
                != localization.Text(DesktopTextKey.RefreshAll)
            || statusText.Text
                != DesktopHubPresentation.RefreshSummary(completedSummary)
                    .Resolve(localization))
        {
            throw new InvalidDataException(
                "Hub refresh-all control did not restore its completed state");
        }
    }

    public void ProbeFirstRuntimeAction()
    {
        var action = topologyCards
            .SelectMany(card => card.RuntimeList.Children.OfType<Button>())
            .FirstOrDefault()
            ?? throw new InvalidDataException("Hub topology has no runtime action to probe");
        action.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
    }

    public void ProbeTutorialEntry() =>
        tutorialButton.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));

    public void ProbeLanguageEntry() =>
        languageButton.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));

    private Border CreateClientRoot(int remoteCount, bool localSupported)
    {
        var count = remoteCount + (localSupported ? 1 : 0);
        var client = NodeText(HubText("client.title"), HubText("client.subtitle"));
        var daemonCount = CountBadge(HubFormat("client.daemon_count", count));
        Grid.SetColumn(client, 1);
        Grid.SetColumn(daemonCount, 2);
        return new Border
        {
            Background = Brush.Parse("#2A2113"),
            BorderBrush = LeserpentTheme.Accent,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(10),
            Padding = new Thickness(18, 15),
            Child = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("Auto,*,Auto"),
                ColumnSpacing = 14,
                Children =
                {
                    NodeGlyph("L", LeserpentTheme.Accent, Brushes.Black),
                    client,
                    daemonCount,
                },
            },
        };
    }

    private Border CreateDaemonCard(
        string daemonId,
        string name,
        string endpoint,
        string kind,
        string detail,
        Func<string?> open,
        Func<RemoteRuntimeProjection, ulong, string?> openRuntime,
        Func<CancellationToken, Task<RemoteTopologySnapshot>> loadTopology,
        Action? manage)
    {
        var openButton = new Button
        {
            Content = localization.Text(DesktopTextKey.Open),
            Background = LeserpentTheme.Primary,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(16, 8),
        };
        AutomationProperties.SetAutomationId(openButton, $"hub-open-{daemonId}");
        AutomationProperties.SetName(
            openButton,
            HubFormat("a11y.open_daemon", Safe(name)));
        auditedControls.Add(openButton);
        openButton.Click += (_, _) => OpenDaemon(open, openButton, name);

        var refreshButton = new Button
        {
            Content = localization.Text(DesktopTextKey.Refresh),
            Padding = new Thickness(14, 8),
        };
        AutomationProperties.SetAutomationId(refreshButton, $"hub-refresh-{daemonId}");
        AutomationProperties.SetName(
            refreshButton,
            HubFormat("a11y.refresh_daemon", Safe(name)));
        auditedControls.Add(refreshButton);

        var actions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { refreshButton, openButton },
        };
        if (manage is not null)
        {
            var manageButton = new Button
            {
                Content = localization.Text(DesktopTextKey.Manage),
                Padding = new Thickness(14, 8),
            };
            AutomationProperties.SetAutomationId(manageButton, $"hub-manage-{daemonId}");
            AutomationProperties.SetName(
                manageButton,
                HubFormat("a11y.manage_daemon", Safe(name)));
            auditedControls.Add(manageButton);
            manageButton.Click += (_, _) => manage();
            actions.Children.Add(manageButton);
        }

        var body = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("Auto,*,Auto"),
            ColumnSpacing = 14,
            Children =
            {
                NodeGlyph("D", LeserpentTheme.PanelBorder, LeserpentTheme.Primary),
                NodeText(name, endpoint, detail),
                actions,
            },
        };
        Grid.SetColumn(body.Children[1], 1);
        Grid.SetColumn(actions, 2);

        var topologySummary = new TextBlock
        {
            Text = HubText("summary.awaiting"),
            Foreground = LeserpentTheme.Muted,
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            LetterSpacing = 0.8,
        };
        AutomationProperties.SetName(
            topologySummary,
            HubFormat("a11y.summary.awaiting", Safe(name)));
        var authoritySummary = new TextBlock
        {
            Text = HubText("authority.awaiting"),
            Foreground = LeserpentTheme.Muted,
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            LetterSpacing = 0.8,
        };
        AutomationProperties.SetName(
            authoritySummary,
            HubFormat("a11y.authority.awaiting", Safe(name)));
        var runtimeList = new StackPanel
        {
            Spacing = 7,
            Children =
            {
                new TextBlock
                {
                    Text = HubText("runtime.loading"),
                    Foreground = LeserpentTheme.Muted,
                    FontSize = 12,
                },
            },
        };
        var root = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(10),
            Padding = new Thickness(18, 15),
            Margin = new Thickness(18, 0, 0, 0),
            Child = new StackPanel
            {
                Spacing = 10,
                Children =
                {
                    new TextBlock
                    {
                        Text = kind,
                        Foreground = LeserpentTheme.Accent,
                        FontSize = 10,
                        FontWeight = FontWeight.Bold,
                        LetterSpacing = 1.5,
                    },
                    body,
                    new Border
                    {
                        BorderBrush = LeserpentTheme.PanelBorder,
                        BorderThickness = new Thickness(2, 0, 0, 0),
                        Margin = new Thickness(20, 2, 0, 0),
                        Padding = new Thickness(18, 4, 0, 2),
                        Child = new StackPanel
                        {
                            Spacing = 9,
                            Children = { topologySummary, authoritySummary, runtimeList },
                        },
                    },
                },
            },
        };
        var topologyCard = new DaemonTopologyCard(
            daemonId,
            name,
            kind,
            detail,
            openRuntime,
            loadTopology,
            topologySummary,
            authoritySummary,
            runtimeList,
            refreshButton,
            root);
        topologyCard.ReportWorkspaceResult = (runtime, error) =>
            ReportWorkspaceOpen(name, runtime, error);
        topologyCards.Add(topologyCard);
        refreshButton.Click += (_, _) => ObserveTopologyOperation(
            RefreshTopologyAsync(topologyCard));
        return root;
    }

    private Task<RemoteTopologyRefreshSummary> RefreshAllTopologiesAsync(
        TopologyRefreshTrigger trigger)
    {
        if (lifetime.IsCancellationRequested || topologyCards.Count == 0)
        {
            return Task.FromResult(new RemoteTopologyRefreshSummary(
                topologyRefresh.Generation,
                0,
                0,
                0,
                0));
        }
        if (trigger == TopologyRefreshTrigger.Operator)
        {
            operatorRefreshRequested = true;
            SetStatus(
                DesktopHubPresentation.Text(
                    "status.refreshing_all",
                    topologyCards.Count),
                LeserpentTheme.Primary);
        }
        var coordinated = topologyRefresh.RefreshAllAsync(
            topologyCards.Select(RefreshAuthority),
            lifetime.Token);
        if (refreshAllPresentationOperation is { IsCompleted: false } active)
        {
            return active;
        }
        refreshAllPresentationOperation = PresentRefreshAllAsync(coordinated);
        return refreshAllPresentationOperation;
    }

    private async Task<RemoteTopologyRefreshSummary> PresentRefreshAllAsync(
        Task<RemoteTopologyRefreshSummary> coordinated)
    {
        refreshAllTopologyButton.Content = HubText("action.refreshing");
        refreshAllTopologyButton.IsEnabled = false;
        AutomationProperties.SetName(
            refreshAllTopologyButton,
            HubText("a11y.refreshing_all"));
        await Task.Yield();
        try
        {
            var summary = await coordinated;
            if (operatorRefreshRequested
                && !lifetime.IsCancellationRequested)
            {
                SetStatus(
                    DesktopHubPresentation.RefreshSummary(summary),
                    summary.RequiresAttention
                        ? LeserpentTheme.Destructive
                        : LeserpentTheme.Accent);
            }
            return summary;
        }
        finally
        {
            operatorRefreshRequested = false;
            refreshAllPresentationOperation = null;
            refreshAllTopologyButton.Content = localization.Text(DesktopTextKey.RefreshAll);
            AutomationProperties.SetName(
                refreshAllTopologyButton,
                localization.Text(DesktopTextKey.RefreshAll));
            refreshAllTopologyButton.IsEnabled = daemonCardCount > 0
                && !lifetime.IsCancellationRequested;
        }
    }

    private Task<RemoteTopologyPhase> RefreshTopologyAsync(DaemonTopologyCard card)
    {
        if (lifetime.IsCancellationRequested)
        {
            return Task.FromResult(card.State.State.Phase);
        }
        return topologyRefresh.RefreshAuthorityAsync(
            RefreshAuthority(card),
            lifetime.Token);
    }

    private RemoteTopologyRefreshAuthority RefreshAuthority(
        DaemonTopologyCard card) => new(
            card.DaemonId,
            cancellationToken => RefreshTopologyCoreAsync(card, cancellationToken));

    private async Task<RemoteTopologyPhase> RefreshTopologyCoreAsync(
        DaemonTopologyCard card,
        CancellationToken cancellationToken)
    {
        card.RefreshButton.IsEnabled = false;
        var loading = card.State.BeginRefresh();
        card.Summary.Text = DesktopHubPresentation.LoadingSummary(loading)
            .Resolve(localization);
        card.Summary.Foreground = LeserpentTheme.Primary;
        AutomationProperties.SetName(
            card.Summary,
            HubFormat("a11y.summary.loading", Safe(card.Name)));
        await Task.Yield();
        try
        {
            var snapshot = await card.Load(cancellationToken);
            if (!lifetime.IsCancellationRequested)
            {
                card.State.Accept(snapshot);
                ApplyTopologyFilter();
            }
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            throw;
        }
        catch (Exception error) when (StartupFailure.IsExpected(error)
            || error is TaskCanceledException or TimeoutException)
        {
            if (!lifetime.IsCancellationRequested)
            {
                card.State.Reject();
                ApplyTopologyFilter();
            }
        }
        finally
        {
            if (!lifetime.IsCancellationRequested)
            {
                card.RefreshButton.IsEnabled = true;
            }
        }
        return card.State.State.Phase;
    }

    private async void ObserveTopologyOperation(Task operation)
    {
        try
        {
            await operation;
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception)
        {
            if (lifetime.IsCancellationRequested)
            {
                return;
            }
            SetStatus(
                DesktopHubPresentation.Text("status.refresh_unexpected"),
                LeserpentTheme.Destructive);
        }
    }

    private void RenderTopology(
        DaemonTopologyCard card,
        RemoteTopologyState state,
        IReadOnlyList<RemoteRuntimeProjection> runtimes)
    {
        var snapshot = state.Snapshot
            ?? throw new InvalidDataException("renderable topology state has no snapshot");
        card.RuntimeList.Children.Clear();
        card.RenderedRuntimeCount = 0;
        if (state.Phase == RemoteTopologyPhase.Loading)
        {
            card.Summary.Text = DesktopHubPresentation.LoadingSummary(state)
                .Resolve(localization);
            AutomationProperties.SetName(
                card.Summary,
                HubFormat("a11y.summary.loading", Safe(card.Name)));
        }
        else
        {
            card.Summary.Text = DesktopHubPresentation.TopologySummary(
                state,
                runtimes.Count,
                automationName: false,
                card.Name).Resolve(localization);
            AutomationProperties.SetName(
                card.Summary,
                DesktopHubPresentation.TopologySummary(
                    state,
                    runtimes.Count,
                    automationName: true,
                    Safe(card.Name)).Resolve(localization));
        }
        card.Summary.Foreground = state.Phase is RemoteTopologyPhase.Cached
            or RemoteTopologyPhase.Retained
            ? LeserpentTheme.Muted
            : LeserpentTheme.Accent;
        RenderAuthority(card, state);
        if (runtimes.Count == 0)
        {
            card.RuntimeList.Children.Add(RuntimeMessage(
                HubText("runtime.empty")));
            return;
        }
        foreach (var runtime in runtimes.Take(MaxVisibleRuntimesPerDaemon))
        {
            card.RuntimeList.Children.Add(RuntimeRow(card, runtime, snapshot.Revision));
            card.RenderedRuntimeCount++;
        }
        var hidden = runtimes.Count - MaxVisibleRuntimesPerDaemon;
        if (hidden > 0)
        {
            card.RuntimeList.Children.Add(RuntimeMessage(
                HubFormat("runtime.more", hidden)));
        }
    }

    private void RenderAuthority(
        DaemonTopologyCard card,
        RemoteTopologyState state)
    {
        if (state.Snapshot?.Health is not { } health)
        {
            card.AuthoritySummary.Text = HubText("authority.unverified");
            card.AuthoritySummary.Foreground = LeserpentTheme.Muted;
            AutomationProperties.SetName(
                card.AuthoritySummary,
                HubFormat("a11y.authority.unverified", Safe(card.Name)));
            return;
        }
        var presentation = DesktopHubPresentation.AuthorityHealth(health);
        var healthText = presentation.Resolve(localization);
        var stale = state.Phase is RemoteTopologyPhase.Cached or RemoteTopologyPhase.Retained;
        card.AuthoritySummary.Text = stale
            ? HubFormat("authority.stale", healthText)
            : healthText;
        card.AuthoritySummary.Foreground = presentation.RequiresAttention
            ? LeserpentTheme.Destructive
            : stale
                ? LeserpentTheme.Muted
                : LeserpentTheme.Primary;
        AutomationProperties.SetName(
            card.AuthoritySummary,
            stale
                ? HubFormat(
                    "a11y.authority.stale",
                    Safe(card.Name),
                    healthText)
                : HubFormat(
                    "a11y.authority",
                    Safe(card.Name),
                    healthText));
        AutomationProperties.SetLiveSetting(
            card.AuthoritySummary,
            presentation.RequiresAttention
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
    }

    private void RenderTopologyFailure(
        DaemonTopologyCard card,
        RemoteTopologyState state)
    {
        card.RuntimeList.Children.Clear();
        card.RenderedRuntimeCount = 0;
        card.RuntimeList.Children.Add(RuntimeMessage(
            HubText("runtime.topology_unavailable")));
        card.Summary.Text = HubFormat(
            "summary.unavailable",
            state.ConsecutiveFailures);
        card.Summary.Foreground = LeserpentTheme.Destructive;
        card.AuthoritySummary.Text = HubText("authority.unavailable");
        card.AuthoritySummary.Foreground = LeserpentTheme.Destructive;
        AutomationProperties.SetName(
            card.AuthoritySummary,
            HubFormat("a11y.authority.unavailable", Safe(card.Name)));
        AutomationProperties.SetName(
            card.Summary,
            HubFormat(
                "a11y.summary.unavailable",
                Safe(card.Name),
                state.ConsecutiveFailures));
    }

    private Button RuntimeRow(
        DaemonTopologyCard card,
        RemoteRuntimeProjection runtime,
        ulong topologyRevision)
    {
        var failed = runtime.Status.StatusFetchError is { Length: > 0 }
            || runtime.RefreshStatus == RefreshStatus.Failed;
        var state = DesktopHubPresentation.RuntimeStatus(runtime)
            .Resolve(localization);
        var status = new TextBlock
        {
            Text = state,
            Foreground = failed
                ? LeserpentTheme.Destructive
                : runtime.RefreshStatus == RefreshStatus.Pending
                    ? LeserpentTheme.Accent
                    : LeserpentTheme.Primary,
            FontSize = 10,
            FontWeight = FontWeight.Bold,
            VerticalAlignment = VerticalAlignment.Center,
        };
        Grid.SetColumn(status, 2);
        var identity = new StackPanel
        {
            Spacing = 2,
            Children =
            {
                new TextBlock
                {
                    Text = Safe(runtime.Name),
                    Foreground = LeserpentTheme.Body,
                    FontSize = 12,
                    FontWeight = FontWeight.SemiBold,
                },
                new TextBlock
                {
                    Text = Safe(runtime.Id),
                    Foreground = LeserpentTheme.Muted,
                    FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
                    FontSize = 10,
                },
            },
        };
        Grid.SetColumn(identity, 1);
        var row = new Button
        {
            Background = Brush.Parse("#16140F"),
            Padding = new Thickness(10, 8),
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Content = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("Auto,*,Auto"),
                ColumnSpacing = 10,
                Children =
                {
                    NodeGlyph("G", LeserpentTheme.PanelBorder, LeserpentTheme.Body, 26),
                    identity,
                    status,
                },
            },
        };
        AutomationProperties.SetAutomationId(
            row,
            $"hub-runtime-{card.DaemonId}-{runtime.Id}");
        AutomationProperties.SetName(
            row,
            HubFormat(
                "a11y.open_runtime",
                Safe(runtime.Name),
                Safe(runtime.Id),
                state));
        AutomationProperties.SetHelpText(
            row,
            HubText("help.open_runtime"));
        row.Click += (_, _) =>
        {
            var error = card.OpenRuntime(runtime, topologyRevision);
            card.ReportWorkspaceResult(runtime, error);
        };
        return row;
    }

    private static TextBlock RuntimeMessage(string value) => new()
    {
        Text = value,
        Foreground = LeserpentTheme.Muted,
        FontSize = 12,
        TextWrapping = TextWrapping.Wrap,
    };

    private string HubText(string key) =>
        DesktopHubCatalogs.Resolve(localization, key);

    private string HubFormat(string key, params object[] values) =>
        DesktopHubCatalogs.Format(localization, key, values);

    private void SetStatus(DesktopHubText presentation, IBrush foreground) =>
        SetStatusValue(presentation.Resolve(localization), foreground);

    private void SetOpaqueStatus(string value, IBrush foreground) =>
        SetStatusValue(Safe(value), foreground);

    private void SetStatusValue(string value, IBrush foreground)
    {
        statusText.Text = value;
        statusText.Foreground = foreground;
        statusText.IsVisible = true;
        AutomationProperties.SetName(
            statusText,
            HubFormat("a11y.status_value", value));
    }

    private void OpenDaemon(Func<string?> open, Button button, string name)
    {
        button.IsEnabled = false;
        SetStatus(
            DesktopHubPresentation.Text("status.opening_daemon", Safe(name)),
            LeserpentTheme.Primary);
        var error = open();
        if (error is null)
        {
            SetStatus(
                DesktopHubPresentation.Text("status.daemon_open", Safe(name)),
                LeserpentTheme.Accent);
            button.IsEnabled = true;
            return;
        }
        SetOpaqueStatus(error, LeserpentTheme.Destructive);
        button.IsEnabled = true;
    }

    private void ReportWorkspaceOpen(
        string daemonName,
        RemoteRuntimeProjection runtime,
        string? error)
    {
        if (error is null)
        {
            SetStatus(
                DesktopHubPresentation.Text(
                    "status.opening_runtime",
                    Safe(runtime.Name),
                    Safe(daemonName)),
                LeserpentTheme.Accent);
            return;
        }
        SetOpaqueStatus(error, LeserpentTheme.Destructive);
    }

    private static Border NodeGlyph(
        string text,
        IBrush background,
        IBrush foreground,
        double size = 40) => new()
        {
            Width = size,
            Height = size,
            Background = background,
            CornerRadius = new CornerRadius(size / 2),
            Child = new TextBlock
            {
                Text = text,
                Foreground = foreground,
                FontSize = 14,
                FontWeight = FontWeight.Bold,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };

    private static StackPanel NodeText(string title, string subtitle, string? detail = null)
    {
        var panel = new StackPanel
        {
            Spacing = 3,
            VerticalAlignment = VerticalAlignment.Center,
            Children =
            {
                new TextBlock
                {
                    Text = Safe(title),
                    Foreground = LeserpentTheme.Body,
                    FontSize = 15,
                    FontWeight = FontWeight.SemiBold,
                },
                new TextBlock
                {
                    Text = Safe(subtitle),
                    Foreground = LeserpentTheme.Muted,
                    FontSize = 12,
                    TextWrapping = TextWrapping.Wrap,
                },
            },
        };
        if (!string.IsNullOrWhiteSpace(detail))
        {
            panel.Children.Add(new TextBlock
            {
                Text = Safe(detail),
                Foreground = LeserpentTheme.Muted,
                FontSize = 11,
            });
        }
        Grid.SetColumn(panel, 1);
        return panel;
    }

    private static Border CountBadge(string text) => new()
    {
        Background = LeserpentTheme.Panel,
        CornerRadius = new CornerRadius(12),
        Padding = new Thickness(10, 5),
        VerticalAlignment = VerticalAlignment.Center,
        Child = new TextBlock
        {
            Text = text,
            Foreground = LeserpentTheme.Primary,
            FontSize = 10,
            FontWeight = FontWeight.Bold,
            LetterSpacing = 1,
        },
    };

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(512)
        .ToArray());

    private sealed class DaemonTopologyCard(
        string daemonId,
        string name,
        string kind,
        string detail,
        Func<RemoteRuntimeProjection, ulong, string?> openRuntime,
        Func<CancellationToken, Task<RemoteTopologySnapshot>> load,
        TextBlock summary,
        TextBlock authoritySummary,
        StackPanel runtimeList,
        Button refreshButton,
        Border root)
    {
        public string DaemonId { get; } = daemonId;
        public string Name { get; } = name;
        public string Kind { get; } = kind;
        public string Detail { get; } = detail;
        public Func<RemoteRuntimeProjection, ulong, string?> OpenRuntime { get; } = openRuntime;
        public Func<CancellationToken, Task<RemoteTopologySnapshot>> Load { get; } = load;
        public TextBlock Summary { get; } = summary;
        public TextBlock AuthoritySummary { get; } = authoritySummary;
        public StackPanel RuntimeList { get; } = runtimeList;
        public Button RefreshButton { get; } = refreshButton;
        public Border Root { get; } = root;
        public RemoteTopologyStateMachine State { get; } = new();
        public int RenderedRuntimeCount { get; set; }
        public Action<RemoteRuntimeProjection, string?> ReportWorkspaceResult { get; set; } =
            (_, _) => { };
    }

    private enum TopologyRefreshTrigger
    {
        Startup,
        Periodic,
        Operator,
    }
}
