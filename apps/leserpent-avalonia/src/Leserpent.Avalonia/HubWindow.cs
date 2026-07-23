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
    private readonly SemaphoreSlim topologyLoadGate = new(4, 4);
    private readonly CancellationTokenSource lifetime = new();
    private readonly DispatcherTimer topologyRefreshTimer = new()
    {
        Interval = TimeSpan.FromSeconds(30),
    };
    private readonly int daemonCardCount;
    private readonly int expectedAuditedControlCount;
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
        Action provisionRuntime,
        Action retireRuntime,
        Action addConnection,
        Action<DesktopDaemonConnection> manageConnection)
    {
        daemonCardCount = connections.Count + (localSupported ? 1 : 0);
        expectedAuditedControlCount = 5 + connections.Count * 3 + (localSupported ? 2 : 0);
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

        var addButton = new Button
        {
            Content = "+ Add daemon",
            Background = LeserpentTheme.Accent,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(addButton, "hub-add-daemon");
        AutomationProperties.SetName(addButton, "Add a leserpent daemon connection");
        auditedControls.Add(addButton);
        addButton.Click += (_, _) => addConnection();

        var deployButton = new Button
        {
            Content = "Deploy daemon",
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(deployButton, "hub-deploy-daemon");
        AutomationProperties.SetName(deployButton, "Deploy a leserpent daemon to a target host");
        auditedControls.Add(deployButton);
        deployButton.Click += (_, _) => deployDaemon();

        var provisionButton = new Button
        {
            Content = "Provision gewyvern",
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(provisionButton, "hub-provision-gewyvern");
        AutomationProperties.SetName(provisionButton, "Provision a gewyvern runtime through a daemon authority");
        auditedControls.Add(provisionButton);
        provisionButton.Click += (_, _) => provisionRuntime();

        var retireButton = new Button
        {
            Content = "Retire gewyvern",
            Padding = new Thickness(17, 9),
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAutomationId(retireButton, "hub-retire-gewyvern");
        AutomationProperties.SetName(
            retireButton,
            "Retire a gewyvern runtime through its daemon authority");
        auditedControls.Add(retireButton);
        retireButton.Click += (_, _) => retireRuntime();

        var headingActions = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 9,
            VerticalAlignment = VerticalAlignment.Center,
            Children = { deployButton, provisionButton, retireButton, addButton },
        };

        var heading = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
            ColumnSpacing = 20,
        };
        heading.Children.Add(new StackPanel
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
                    Text = "Control topology",
                    Foreground = LeserpentTheme.Primary,
                    FontSize = 31,
                    FontWeight = FontWeight.Bold,
                },
                new TextBlock
                {
                    Text = "One client, multiple daemon authorities. Open a daemon to manage its gewyvern runtimes.",
                    Foreground = LeserpentTheme.Muted,
                    FontSize = 13,
                    TextWrapping = TextWrapping.Wrap,
                },
            },
        });
        Grid.SetColumn(headingActions, 1);
        heading.Children.Add(headingActions);

        var topology = new StackPanel { Spacing = 10 };
        topology.Children.Add(CreateClientRoot(connections.Count, localSupported));

        var branch = new Border
        {
            Width = 2,
            Height = 18,
            Background = LeserpentTheme.PanelBorder,
            HorizontalAlignment = HorizontalAlignment.Left,
            Margin = new Thickness(27, 0, 0, 0),
        };
        topology.Children.Add(branch);

        if (localSupported)
        {
            topology.Children.Add(CreateDaemonCard(
                "local-orchestra",
                "Local Orchestra",
                "Managed on this device",
                "LOCAL",
                "Ephemeral session authority",
                openLocal,
                openLocalRuntime,
                loadLocalTopology,
                null));
        }

        foreach (var connection in connections)
        {
            var captured = connection;
            topology.Children.Add(CreateDaemonCard(
                connection.DaemonId,
                connection.DisplayName,
                connection.Profile.Endpoint,
                "REMOTE",
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
            topology.Children.Add(new Border
            {
                Background = LeserpentTheme.Panel,
                BorderBrush = LeserpentTheme.PanelBorder,
                BorderThickness = new Thickness(1),
                CornerRadius = new CornerRadius(10),
                Padding = new Thickness(20),
                Child = new TextBlock
                {
                    Text = "No daemon authorities are configured. Add one to establish the first topology branch.",
                    Foreground = LeserpentTheme.Muted,
                    TextWrapping = TextWrapping.Wrap,
                },
            });
        }

        statusText.Foreground = string.IsNullOrWhiteSpace(initialError)
            ? LeserpentTheme.Muted
            : LeserpentTheme.Destructive;
        statusText.Text = string.IsNullOrWhiteSpace(initialError)
            ? $"Topology ready: {(localSupported ? 1 : 0) + connections.Count} daemon authorit{((localSupported ? 1 : 0) + connections.Count == 1 ? "y" : "ies")}."
            : Safe(initialError);
        statusText.IsVisible = true;
        AutomationProperties.SetAutomationId(statusText, "hub-status");
        AutomationProperties.SetName(statusText, "Hub topology status");
        auditedControls.Add(statusText);

        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,*,Auto"),
            RowSpacing = 18,
            Margin = new Thickness(34, 28),
            Children =
            {
                heading,
                new ScrollViewer
                {
                    Content = topology,
                    VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
                    HorizontalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Disabled,
                },
                statusText,
            },
        };
        Grid.SetRow(((Grid)Content).Children[1], 1);
        Grid.SetRow(statusText, 2);

        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close();
            }
        };
        topologyRefreshTimer.Tick += (_, _) => _ = RefreshAllTopologiesAsync();
        Opened += (_, _) =>
        {
            topologyRefreshTimer.Start();
            _ = RefreshAllTopologiesAsync();
        };
        Closed += (_, _) =>
        {
            topologyRefreshTimer.Stop();
            lifetime.Cancel();
        };
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

    public void ProbeFirstRuntimeAction()
    {
        var action = topologyCards
            .SelectMany(card => card.RuntimeList.Children.OfType<Button>())
            .FirstOrDefault()
            ?? throw new InvalidDataException("Hub topology has no runtime action to probe");
        action.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
    }

    private Border CreateClientRoot(int remoteCount, bool localSupported)
    {
        var count = remoteCount + (localSupported ? 1 : 0);
        var client = NodeText("Leserpent Desktop", "Topology root / operator session");
        var daemonCount = CountBadge($"{count} DAEMON{(count == 1 ? string.Empty : "S")}");
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
            Content = "Open",
            Background = LeserpentTheme.Primary,
            Foreground = Brushes.Black,
            FontWeight = FontWeight.SemiBold,
            Padding = new Thickness(16, 8),
        };
        AutomationProperties.SetAutomationId(openButton, $"hub-open-{daemonId}");
        AutomationProperties.SetName(openButton, $"Open daemon {name}");
        auditedControls.Add(openButton);
        openButton.Click += (_, _) => OpenDaemon(open, openButton, name);

        var refreshButton = new Button
        {
            Content = "Refresh",
            Padding = new Thickness(14, 8),
        };
        AutomationProperties.SetAutomationId(refreshButton, $"hub-refresh-{daemonId}");
        AutomationProperties.SetName(refreshButton, $"Refresh runtime topology for daemon {name}");
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
                Content = "Manage",
                Padding = new Thickness(14, 8),
            };
            AutomationProperties.SetAutomationId(manageButton, $"hub-manage-{daemonId}");
            AutomationProperties.SetName(manageButton, $"Manage daemon {name}");
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
            Text = "RUNTIMES / awaiting topology",
            Foreground = LeserpentTheme.Muted,
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            LetterSpacing = 0.8,
        };
        AutomationProperties.SetName(
            topologySummary,
            $"Runtime topology for daemon {name} is awaiting refresh");
        var authoritySummary = new TextBlock
        {
            Text = "AUTHORITY / awaiting proof",
            Foreground = LeserpentTheme.Muted,
            FontSize = 11,
            FontWeight = FontWeight.Bold,
            LetterSpacing = 0.8,
        };
        AutomationProperties.SetName(
            authoritySummary,
            $"Authority health for daemon {name} is awaiting proof");
        var runtimeList = new StackPanel
        {
            Spacing = 7,
            Children =
            {
                new TextBlock
                {
                    Text = "Loading bounded runtime summary...",
                    Foreground = LeserpentTheme.Muted,
                    FontSize = 12,
                },
            },
        };
        var topologyCard = new DaemonTopologyCard(
            daemonId,
            name,
            openRuntime,
            loadTopology,
            topologySummary,
            authoritySummary,
            runtimeList,
            refreshButton);
        topologyCard.ReportWorkspaceResult = (runtime, error) =>
            ReportWorkspaceOpen(name, runtime, error);
        topologyCards.Add(topologyCard);
        refreshButton.Click += (_, _) => _ = RefreshTopologyAsync(topologyCard);

        return new Border
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
    }

    private async Task RefreshAllTopologiesAsync()
    {
        await Task.WhenAll(topologyCards.Select(RefreshTopologyAsync));
    }

    private async Task RefreshTopologyAsync(DaemonTopologyCard card)
    {
        if (card.Refreshing || lifetime.IsCancellationRequested)
        {
            return;
        }
        card.Refreshing = true;
        card.RefreshButton.IsEnabled = false;
        var loading = card.State.BeginRefresh();
        card.Summary.Text = loading.Snapshot is null
            ? "RUNTIMES / loading"
            : $"RUNTIMES / refreshing / REV {loading.Snapshot.Revision}";
        card.Summary.Foreground = LeserpentTheme.Primary;
        AutomationProperties.SetName(
            card.Summary,
            $"Loading runtime topology for daemon {Safe(card.Name)}");
        var enteredGate = false;
        try
        {
            await topologyLoadGate.WaitAsync(lifetime.Token);
            enteredGate = true;
            var snapshot = await card.Load(lifetime.Token);
            if (!lifetime.IsCancellationRequested)
            {
                RenderTopology(card, card.State.Accept(snapshot));
            }
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error) when (StartupFailure.IsExpected(error)
            || error is TaskCanceledException or TimeoutException)
        {
            if (!lifetime.IsCancellationRequested)
            {
                RenderTopologyFailure(card, card.State.Reject());
            }
        }
        finally
        {
            if (enteredGate)
            {
                topologyLoadGate.Release();
            }
            card.Refreshing = false;
            if (!lifetime.IsCancellationRequested)
            {
                card.RefreshButton.IsEnabled = true;
            }
        }
    }

    private static void RenderTopology(
        DaemonTopologyCard card,
        RemoteTopologyState state)
    {
        var snapshot = state.Snapshot
            ?? throw new InvalidDataException("renderable topology state has no snapshot");
        card.RuntimeList.Children.Clear();
        card.RenderedRuntimeCount = 0;
        var source = state.Phase.ToString().ToUpperInvariant();
        card.Summary.Text = $"RUNTIMES / {source} / REV {snapshot.Revision} / {snapshot.Runtimes.Count}";
        card.Summary.Foreground = state.Phase is RemoteTopologyPhase.Cached
            or RemoteTopologyPhase.Retained
            ? LeserpentTheme.Muted
            : LeserpentTheme.Accent;
        AutomationProperties.SetName(
            card.Summary,
            $"Daemon {Safe(card.Name)} has {snapshot.Runtimes.Count} runtimes at revision {snapshot.Revision}, {source.ToLowerInvariant()}");
        RenderAuthority(card, state);
        if (snapshot.Runtimes.Count == 0)
        {
            card.RuntimeList.Children.Add(RuntimeMessage(
                "No gewyvern runtimes are registered under this daemon."));
            return;
        }
        foreach (var runtime in snapshot.Runtimes.Take(MaxVisibleRuntimesPerDaemon))
        {
            card.RuntimeList.Children.Add(RuntimeRow(card, runtime, snapshot.Revision));
            card.RenderedRuntimeCount++;
        }
        var hidden = snapshot.Runtimes.Count - MaxVisibleRuntimesPerDaemon;
        if (hidden > 0)
        {
            card.RuntimeList.Children.Add(RuntimeMessage(
                $"+ {hidden} more runtimes in the daemon session"));
        }
    }

    private static void RenderAuthority(
        DaemonTopologyCard card,
        RemoteTopologyState state)
    {
        if (state.Snapshot?.Health is not { } health)
        {
            card.AuthoritySummary.Text = "AUTHORITY / unverified cache";
            card.AuthoritySummary.Foreground = LeserpentTheme.Muted;
            AutomationProperties.SetName(
                card.AuthoritySummary,
                $"Authority health for daemon {Safe(card.Name)} is unavailable in the cached topology");
            return;
        }
        var presentation = RemoteAuthorityHealthPresentation.Create(health);
        var stale = state.Phase is RemoteTopologyPhase.Cached or RemoteTopologyPhase.Retained;
        card.AuthoritySummary.Text = stale
            ? $"{presentation.Label} / STALE"
            : presentation.Label;
        card.AuthoritySummary.Foreground = presentation.IsSaturated
            ? LeserpentTheme.Destructive
            : stale
                ? LeserpentTheme.Muted
                : LeserpentTheme.Primary;
        AutomationProperties.SetName(
            card.AuthoritySummary,
            stale
                ? $"{presentation.AutomationName}; stale topology evidence"
                : presentation.AutomationName);
    }

    private static void RenderTopologyFailure(
        DaemonTopologyCard card,
        RemoteTopologyState state)
    {
        if (state.Snapshot is not null)
        {
            RenderTopology(card, state);
            card.RuntimeList.Children.Insert(0, RuntimeMessage(
                $"Refresh failed {state.ConsecutiveFailures} time(s). Retaining the last known topology; workspace launch still requires a live daemon snapshot."));
            return;
        }
        card.RuntimeList.Children.Clear();
        card.RenderedRuntimeCount = 0;
        card.RuntimeList.Children.Add(RuntimeMessage(
            "Topology unavailable. The daemon session can still be opened manually."));
        card.Summary.Text = $"RUNTIMES / unavailable / failures {state.ConsecutiveFailures}";
        card.Summary.Foreground = LeserpentTheme.Destructive;
        card.AuthoritySummary.Text = "AUTHORITY / unavailable";
        card.AuthoritySummary.Foreground = LeserpentTheme.Destructive;
        AutomationProperties.SetName(
            card.AuthoritySummary,
            $"Authority health for daemon {Safe(card.Name)} is unavailable");
        AutomationProperties.SetName(
            card.Summary,
            $"Runtime topology for daemon {Safe(card.Name)} is unavailable after {state.ConsecutiveFailures} failures");
    }

    private static Button RuntimeRow(
        DaemonTopologyCard card,
        RemoteRuntimeProjection runtime,
        ulong topologyRevision)
    {
        var state = runtime.Status.StatusFetchError is { Length: > 0 }
            ? "FAILED"
            : runtime.RefreshStatus.ToString().ToUpperInvariant();
        var status = new TextBlock
        {
            Text = state,
            Foreground = state == "FAILED"
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
            $"Open gewyvern runtime {Safe(runtime.Name)}, ID {Safe(runtime.Id)}, status {state}");
        AutomationProperties.SetHelpText(
            row,
            "Opens this runtime through its owning daemon session after an authoritative revision check.");
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

    private void OpenDaemon(Func<string?> open, Button button, string name)
    {
        button.IsEnabled = false;
        statusText.Text = $"Opening {Safe(name)}...";
        statusText.Foreground = LeserpentTheme.Primary;
        var error = open();
        if (error is null)
        {
            statusText.Text = $"{Safe(name)} session is open.";
            statusText.Foreground = LeserpentTheme.Accent;
            button.IsEnabled = true;
            return;
        }
        statusText.Text = Safe(error);
        statusText.Foreground = LeserpentTheme.Destructive;
        button.IsEnabled = true;
    }

    private void ReportWorkspaceOpen(
        string daemonName,
        RemoteRuntimeProjection runtime,
        string? error)
    {
        if (error is null)
        {
            statusText.Text = $"Opening {Safe(runtime.Name)} through {Safe(daemonName)}...";
            statusText.Foreground = LeserpentTheme.Accent;
            return;
        }
        statusText.Text = Safe(error);
        statusText.Foreground = LeserpentTheme.Destructive;
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
        Func<RemoteRuntimeProjection, ulong, string?> openRuntime,
        Func<CancellationToken, Task<RemoteTopologySnapshot>> load,
        TextBlock summary,
        TextBlock authoritySummary,
        StackPanel runtimeList,
        Button refreshButton)
    {
        public string DaemonId { get; } = daemonId;
        public string Name { get; } = name;
        public Func<RemoteRuntimeProjection, ulong, string?> OpenRuntime { get; } = openRuntime;
        public Func<CancellationToken, Task<RemoteTopologySnapshot>> Load { get; } = load;
        public TextBlock Summary { get; } = summary;
        public TextBlock AuthoritySummary { get; } = authoritySummary;
        public StackPanel RuntimeList { get; } = runtimeList;
        public Button RefreshButton { get; } = refreshButton;
        public RemoteTopologyStateMachine State { get; } = new();
        public bool Refreshing { get; set; }
        public int RenderedRuntimeCount { get; set; }
        public Action<RemoteRuntimeProjection, string?> ReportWorkspaceResult { get; set; } =
            (_, _) => { };
    }
}
