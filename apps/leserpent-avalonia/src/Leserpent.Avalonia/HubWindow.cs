using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;

internal sealed class HubWindow : Window
{
    private const int MaxVisibleRuntimesPerDaemon = 6;
    private readonly List<Control> auditedControls = [];
    private readonly List<DaemonTopologyCard> topologyCards = [];
    private readonly SemaphoreSlim topologyLoadGate = new(4, 4);
    private readonly CancellationTokenSource lifetime = new();
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
        Func<CancellationToken, Task<RemoteTopologySnapshot>> loadLocalTopology,
        Func<DesktopDaemonConnection, CancellationToken, Task<RemoteTopologySnapshot>>
            loadRemoteTopology,
        Action addConnection,
        Action<DesktopDaemonConnection> manageConnection)
    {
        daemonCardCount = connections.Count + (localSupported ? 1 : 0);
        expectedAuditedControlCount = 2 + connections.Count * 3 + (localSupported ? 2 : 0);
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
        Grid.SetColumn(addButton, 1);
        heading.Children.Add(addButton);

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
                $"CA  {Path.GetFileName(connection.Profile.CertificateAuthorityPath)}",
                () => openRemote(captured),
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
        Opened += (_, _) => _ = RefreshAllTopologiesAsync();
        Closed += (_, _) => lifetime.Cancel();
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
            loadTopology,
            topologySummary,
            runtimeList,
            refreshButton);
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
                            Children = { topologySummary, runtimeList },
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
        card.Summary.Text = "RUNTIMES / loading";
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
                RenderTopology(card, snapshot);
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
                card.RuntimeList.Children.Clear();
                card.RenderedRuntimeCount = 0;
                card.RuntimeList.Children.Add(RuntimeMessage(
                    "Topology unavailable. The daemon session can still be opened manually."));
                card.Summary.Text = "RUNTIMES / unavailable";
                card.Summary.Foreground = LeserpentTheme.Destructive;
                AutomationProperties.SetName(
                    card.Summary,
                    $"Runtime topology for daemon {Safe(card.Name)} is unavailable");
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
        RemoteTopologySnapshot snapshot)
    {
        card.RuntimeList.Children.Clear();
        card.RenderedRuntimeCount = 0;
        var source = snapshot.IsStale ? "CACHED" : "LIVE";
        card.Summary.Text = $"RUNTIMES / {source} / REV {snapshot.Revision} / {snapshot.Runtimes.Count}";
        card.Summary.Foreground = snapshot.IsStale
            ? LeserpentTheme.Muted
            : LeserpentTheme.Accent;
        AutomationProperties.SetName(
            card.Summary,
            $"Daemon {Safe(card.Name)} has {snapshot.Runtimes.Count} runtimes at revision {snapshot.Revision}, {source.ToLowerInvariant()}");
        if (snapshot.Runtimes.Count == 0)
        {
            card.RuntimeList.Children.Add(RuntimeMessage(
                "No gewyvern runtimes are registered under this daemon."));
            return;
        }
        foreach (var runtime in snapshot.Runtimes.Take(MaxVisibleRuntimesPerDaemon))
        {
            card.RuntimeList.Children.Add(RuntimeRow(runtime));
            card.RenderedRuntimeCount++;
        }
        var hidden = snapshot.Runtimes.Count - MaxVisibleRuntimesPerDaemon;
        if (hidden > 0)
        {
            card.RuntimeList.Children.Add(RuntimeMessage(
                $"+ {hidden} more runtimes in the daemon session"));
        }
    }

    private static Border RuntimeRow(RemoteRuntimeProjection runtime)
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
        var row = new Border
        {
            Background = Brush.Parse("#16140F"),
            CornerRadius = new CornerRadius(7),
            Padding = new Thickness(10, 8),
            Child = new Grid
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
        AutomationProperties.SetName(
            row,
            $"Gewyvern runtime {Safe(runtime.Name)}, ID {Safe(runtime.Id)}, status {state}");
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
        Func<CancellationToken, Task<RemoteTopologySnapshot>> load,
        TextBlock summary,
        StackPanel runtimeList,
        Button refreshButton)
    {
        public string DaemonId { get; } = daemonId;
        public string Name { get; } = name;
        public Func<CancellationToken, Task<RemoteTopologySnapshot>> Load { get; } = load;
        public TextBlock Summary { get; } = summary;
        public StackPanel RuntimeList { get; } = runtimeList;
        public Button RefreshButton { get; } = refreshButton;
        public bool Refreshing { get; set; }
        public int RenderedRuntimeCount { get; set; }
    }
}
