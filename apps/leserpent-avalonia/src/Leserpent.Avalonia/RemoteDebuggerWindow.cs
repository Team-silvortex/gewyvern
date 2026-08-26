using System.Globalization;
using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;

internal sealed record RemoteDebuggerWindowOperations(
    Func<string, string, ulong?, ulong, string, CancellationToken,
        Task<RemoteDebuggerSession>> Start,
    Func<string, string?, CancellationToken,
        Task<IReadOnlyList<RemoteDebuggerSession>>> List,
    Func<RemoteDebuggerSession, string, CancellationToken,
        Task<RemoteDebuggerCancelPlan>> PlanCancel,
    Func<RemoteDebuggerCancelPlan, string, CancellationToken,
        Task<RemoteDebuggerCancelResult>> ApplyCancel);

internal sealed class RemoteDebuggerWindow : Window
{
    private const double CompactWidth = 860;
    private readonly RemoteDebuggerWindowOperations operations;
    private readonly RemoteDebuggerClient? ownedClient;
    private readonly DesktopLocalization localization;
    private readonly CancellationTokenSource lifetime = new();
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly List<Control> auditedControls = [];
    private readonly string principal;
    private readonly string daemonAuthority;
    private readonly Grid bodyGrid = new();
    private readonly Border sourcePanel = Panel();
    private readonly Border projectionPanel = Panel();
    private readonly TextBlock heading = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 27,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock body = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock sessionLabel = Label();
    private readonly TextBlock timeoutLabel = Label();
    private readonly TextBlock sourceLabel = Label();
    private readonly TextBox sessionId = new()
    {
        MaxLength = 128,
        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
    };
    private readonly TextBox timeout = new()
    {
        MaxLength = 6,
        Text = "300000",
        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
    };
    private readonly TextBox source = new()
    {
        AcceptsReturn = true,
        TextWrapping = TextWrapping.NoWrap,
        MaxLength = RemoteDebuggerClient.MaxSourceBytes,
        MinHeight = 180,
        Text = "fn main() = runtime.list()",
        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
        FontSize = 13,
    };
    private readonly Button startButton = PrimaryButton();
    private readonly Button newButton = SecondaryButton();
    private readonly Button refreshButton = SecondaryButton();
    private readonly TextBlock emptyProjection = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
        HorizontalAlignment = HorizontalAlignment.Center,
        VerticalAlignment = VerticalAlignment.Center,
        TextAlignment = TextAlignment.Center,
        Margin = new Thickness(24),
    };
    private readonly TextBlock status = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
    };
    private RemoteDebuggerSession? currentSession;
    private DebuggerCancelConfirmationWindow? confirmationWindow;
    private bool operationInFlight;
    private bool isClosed;
    private string statusKey = "status.ready";
    private object[] statusValues = [];

    public RemoteDebuggerWindow(
        RemoteClientOptions options,
        string principal,
        DesktopLocalization localization)
        : this(
            new RemoteDebuggerClient(options),
            options.Endpoint.Authority,
            principal,
            localization)
    {
    }

    private RemoteDebuggerWindow(
        RemoteDebuggerClient client,
        string daemonAuthority,
        string principal,
        DesktopLocalization localization)
        : this(
            new RemoteDebuggerWindowOperations(
                client.StartAsync,
                client.ListAsync,
                client.PlanCancelAsync,
                client.ApplyCancelAsync),
            daemonAuthority,
            principal,
            localization)
    {
        ownedClient = client;
    }

    internal RemoteDebuggerWindow(
        RemoteDebuggerWindowOperations operations,
        string daemonAuthority,
        string principal,
        DesktopLocalization localization)
    {
        this.operations = operations;
        this.daemonAuthority = daemonAuthority;
        this.principal = principal;
        this.localization = localization;
        sessionId.Text = NewSessionId();
        Width = 1040;
        Height = 740;
        MinWidth = 640;
        MinHeight = 520;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        renderer = new AvaloniaDocumentRenderer(OnActionInvoked, localization.Resolve);
        renderer.Surface.IsVisible = false;
        ConfigureControl(sessionId, "remote-debugger-session");
        ConfigureControl(timeout, "remote-debugger-timeout");
        ConfigureControl(source, "remote-debugger-source");
        ConfigureControl(startButton, "remote-debugger-start");
        ConfigureControl(newButton, "remote-debugger-new");
        ConfigureControl(refreshButton, "remote-debugger-refresh");
        ConfigureControl(emptyProjection, "remote-debugger-projection-empty");
        ConfigureControl(status, "remote-debugger-status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(renderer.Surface, "remote-debugger-projection");

        sourcePanel.Child = BuildSourcePanel();
        projectionPanel.Child = new Grid
        {
            Children = { emptyProjection, renderer.Surface },
        };
        bodyGrid.Children.Add(sourcePanel);
        bodyGrid.Children.Add(projectionPanel);
        var content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto"),
            Children =
            {
                new ScrollViewer
                {
                    VerticalScrollBarVisibility =
                        Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
                    Content = bodyGrid,
                },
                new Border
                {
                    Background = LeserpentTheme.Panel,
                    BorderBrush = LeserpentTheme.PanelBorder,
                    BorderThickness = new Thickness(0, 1, 0, 0),
                    Padding = new Thickness(24, 12),
                    Child = status,
                },
            },
        };
        Grid.SetRow(content.Children[1], 1);
        Content = content;

        startButton.Click += (_, _) => Observe(StartAsync());
        newButton.Click += (_, _) => ResetSession();
        refreshButton.Click += (_, _) => Observe(RefreshAsync());
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape && !operationInFlight)
            {
                eventArgs.Handled = true;
                Close();
            }
        };
        SizeChanged += (_, eventArgs) => ApplyResponsiveLayout(eventArgs.NewSize.Width);
        localization.Changed += OnLocalizationChanged;
        Closed += OnClosed;
        ApplyLocalization();
        ApplyResponsiveLayout(Width);
        UpdateAvailability();
    }

    public void VerifyAccessibility()
    {
        if (auditedControls.Count != 8
            || auditedControls.Select(AutomationProperties.GetAutomationId)
                .Distinct(StringComparer.Ordinal).Count() != auditedControls.Count
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control)))
            || string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(renderer.Surface))
            || string.IsNullOrWhiteSpace(AutomationProperties.GetName(renderer.Surface))
            || AutomationProperties.GetLiveSetting(status) != AutomationLiveSetting.Polite)
        {
            throw new InvalidDataException(
                "remote debugger accessibility contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("remote debugger window has no control root");
        }
        foreach (var (width, height) in new[]
        {
            (MinWidth, MinHeight),
            (Width, Height),
        })
        {
            ApplyResponsiveLayout(width);
            root.Measure(new Size(width, height));
            if (!double.IsFinite(root.DesiredSize.Width)
                || !double.IsFinite(root.DesiredSize.Height)
                || root.DesiredSize.Width <= 0
                || root.DesiredSize.Height <= 0
                || root.DesiredSize.Width > width
                || root.DesiredSize.Height > height)
            {
                throw new InvalidDataException(
                    "remote debugger controls exceeded their layout envelope");
            }
            var compact = width < CompactWidth;
            if (compact
                ? Grid.GetRow(projectionPanel) != 1
                    || Grid.GetColumn(projectionPanel) != 0
                : Grid.GetRow(projectionPanel) != 0
                    || Grid.GetColumn(projectionPanel) != 1)
            {
                throw new InvalidDataException(
                    "remote debugger responsive panels can overlap");
            }
        }
        ApplyResponsiveLayout(Width);
    }

    public void ProbeLocalizedPresentation()
    {
        var expected = new[]
        {
            DesktopDebuggerCatalogs.Format(localization, "title", daemonAuthority),
            DesktopDebuggerCatalogs.Resolve(localization, "heading"),
            DesktopDebuggerCatalogs.Resolve(localization, "label.session"),
            DesktopDebuggerCatalogs.Resolve(localization, "label.timeout"),
            DesktopDebuggerCatalogs.Resolve(localization, "label.source"),
            DesktopDebuggerCatalogs.Resolve(localization, "action.start"),
            DesktopDebuggerCatalogs.Resolve(localization, "action.new"),
            DesktopDebuggerCatalogs.Resolve(localization, "action.refresh"),
        };
        var actual = new[]
        {
            Title,
            heading.Text,
            sessionLabel.Text,
            timeoutLabel.Text,
            sourceLabel.Text,
            startButton.Content?.ToString(),
            newButton.Content?.ToString(),
            refreshButton.Content?.ToString(),
        };
        if (!actual.SequenceEqual(expected, StringComparer.Ordinal)
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "remote debugger localized presentation drifted");
        }
    }

    public async Task ProbeWorkflowAsync()
    {
        await StartAsync();
        var session = currentSession
            ?? throw new InvalidDataException("remote debugger probe did not mount a session");
        if (newButton.IsEnabled)
        {
            throw new InvalidDataException(
                "remote debugger can abandon a waiting session without cancellation");
        }
        var nodeId = FirstCancelNode(session.Document)
            ?? throw new InvalidDataException("remote debugger probe omitted cancellation");
        var route = RemoteUiActionRouter.ResolveDebuggerActivation(
            session.Document,
            nodeId,
            session.Projection.SessionId,
            session.Projection.Revision,
            mutationEnabled: true);
        if (!route.Accepted)
        {
            throw new InvalidDataException("remote debugger probe action did not route");
        }
        var plan = await operations.PlanCancel(session, principal, lifetime.Token);
        var result = await operations.ApplyCancel(plan, principal, lifetime.Token);
        Mount(result.Session);
        if (currentSession.Projection.State != RemoteDebuggerState.Cancelled
            || renderer.RealizedDebuggerCancelButtonCount != 0
            || !newButton.IsEnabled)
        {
            throw new InvalidDataException(
                "remote debugger probe did not reach terminal feedback");
        }
    }

    private Control BuildSourcePanel()
    {
        var coordinates = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("2*,*"),
            ColumnSpacing = 12,
            Children =
            {
                Field(sessionLabel, sessionId),
                Field(timeoutLabel, timeout),
            },
        };
        Grid.SetColumn(coordinates.Children[1], 1);
        var actions = new WrapPanel
        {
            Orientation = Orientation.Horizontal,
            ItemWidth = double.NaN,
            ItemHeight = double.NaN,
            Children = { startButton, refreshButton, newButton },
        };
        return new StackPanel
        {
            Spacing = 16,
            Children =
            {
                new StackPanel
                {
                    Spacing = 6,
                    Children = { heading, body },
                },
                coordinates,
                Field(sourceLabel, source),
                actions,
            },
        };
    }

    private async Task StartAsync()
    {
        if (operationInFlight || currentSession is not null)
        {
            return;
        }
        if (!ulong.TryParse(
                timeout.Text,
                NumberStyles.None,
                CultureInfo.InvariantCulture,
                out var timeoutMs))
        {
            SetFailure("invalid timeout");
            return;
        }
        SetBusy(true);
        SetStatus("status.starting");
        try
        {
            var session = await operations.Start(
                sessionId.Text ?? string.Empty,
                source.Text ?? string.Empty,
                null,
                timeoutMs,
                principal,
                lifetime.Token);
            Mount(session);
            SetStatus(
                "status.started",
                session.Projection.SessionId,
                session.Projection.Revision);
        }
        catch (Exception error) when (!isClosed)
        {
            SetFailure(SafeError(error));
        }
        finally
        {
            SetBusy(false);
        }
    }

    private async Task RefreshAsync()
    {
        if (operationInFlight || currentSession is null)
        {
            return;
        }
        var expectedSessionId = currentSession.Projection.SessionId;
        SetBusy(true);
        SetStatus("status.refreshing");
        try
        {
            var sessions = await operations.List(
                principal,
                expectedSessionId,
                lifetime.Token);
            if (sessions is not [var session]
                || session.Projection.SessionId != expectedSessionId)
            {
                throw new InvalidDataException(
                    "debugger refresh returned an inconsistent session");
            }
            Mount(session);
            SetStatus(
                "status.refreshed",
                session.Projection.SessionId,
                session.Projection.Revision);
        }
        catch (Exception error) when (!isClosed)
        {
            SetFailure(SafeError(error));
        }
        finally
        {
            SetBusy(false);
        }
    }

    private void OnActionInvoked(RenderedActionInvocation invocation)
    {
        if (!ReferenceEquals(invocation.Source, renderer)
            || currentSession is not { } session)
        {
            SetFailure("stale debugger action source");
            return;
        }
        var resolution = RemoteUiActionRouter.ResolveDebuggerActivation(
            session.Document,
            invocation.NodeId,
            session.Projection.SessionId,
            session.Projection.Revision,
            !operationInFlight,
            Text("availability.busy"));
        if (!resolution.Accepted)
        {
            SetFailure(resolution.Reason ?? "debugger action rejected");
            return;
        }
        Observe(CancelAsync(session));
    }

    private async Task CancelAsync(RemoteDebuggerSession session)
    {
        if (operationInFlight || !ReferenceEquals(currentSession, session))
        {
            return;
        }
        SetBusy(true);
        SetStatus("status.planning");
        try
        {
            var plan = await operations.PlanCancel(session, principal, lifetime.Token);
            SetStatus("status.plan_ready");
            var effectId = session.Projection.PendingEffect?.EffectId
                ?? throw new InvalidDataException("debugger effect is no longer pending");
            confirmationWindow = new DebuggerCancelConfirmationWindow(
                session.Projection.SessionId,
                effectId,
                localization);
            var confirmed = await confirmationWindow.ShowDialog<bool>(this);
            confirmationWindow = null;
            if (!confirmed)
            {
                SetStatus("status.cancel_dismissed");
                return;
            }
            var result = await operations.ApplyCancel(
                plan,
                principal,
                lifetime.Token);
            Mount(result.Session);
            SetStatus(
                "status.cancelled",
                result.Session.Projection.SessionId,
                result.AuditedAtMs);
        }
        catch (Exception error) when (!isClosed)
        {
            SetFailure(SafeError(error));
        }
        finally
        {
            SetBusy(false);
        }
    }

    private void Mount(RemoteDebuggerSession session)
    {
        currentSession = session;
        sessionId.Text = session.Projection.SessionId;
        renderer.Mount(session.Document);
        renderer.Surface.IsVisible = true;
        emptyProjection.IsVisible = false;
        sessionId.IsReadOnly = true;
        timeout.IsReadOnly = true;
        source.IsReadOnly = true;
        UpdateAvailability();
    }

    private void ResetSession()
    {
        if (operationInFlight
            || currentSession?.Projection.State == RemoteDebuggerState.WaitingEffect)
        {
            return;
        }
        currentSession = null;
        sessionId.Text = NewSessionId();
        sessionId.IsReadOnly = false;
        timeout.IsReadOnly = false;
        source.IsReadOnly = false;
        renderer.Surface.IsVisible = false;
        emptyProjection.IsVisible = true;
        SetStatus("status.ready");
        UpdateAvailability();
        source.Focus();
    }

    private void SetBusy(bool busy)
    {
        operationInFlight = busy;
        UpdateAvailability();
    }

    private void UpdateAvailability()
    {
        startButton.IsEnabled = !operationInFlight && currentSession is null;
        newButton.IsEnabled = !operationInFlight
            && currentSession?.Projection.State != RemoteDebuggerState.WaitingEffect;
        refreshButton.IsEnabled = !operationInFlight && currentSession is not null;
        renderer.SetActionAvailability(
            ActionKind.DebuggerCancel,
            !operationInFlight
                && currentSession?.Projection.State == RemoteDebuggerState.WaitingEffect,
            operationInFlight
                ? Text("availability.busy")
                : Text("availability.terminal"));
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        if (isClosed)
        {
            return;
        }
        ApplyLocalization();
        if (currentSession is { } session)
        {
            renderer.Mount(session.Document);
            UpdateAvailability();
        }
    }

    private void ApplyLocalization()
    {
        FlowDirection = localization.FlowDirection;
        Title = Text("title", daemonAuthority);
        heading.Text = Text("heading");
        body.Text = Text("body");
        sessionLabel.Text = Text("label.session");
        timeoutLabel.Text = Text("label.timeout");
        sourceLabel.Text = Text("label.source");
        startButton.Content = Text("action.start");
        newButton.Content = Text("action.new");
        refreshButton.Content = Text("action.refresh");
        emptyProjection.Text = Text("projection.empty");
        status.Text = Text(statusKey, statusValues);
        AutomationProperties.SetName(sessionId, sessionLabel.Text);
        AutomationProperties.SetName(timeout, timeoutLabel.Text);
        AutomationProperties.SetName(source, sourceLabel.Text);
        AutomationProperties.SetName(startButton, Text("action.start"));
        AutomationProperties.SetName(newButton, Text("action.new"));
        AutomationProperties.SetName(refreshButton, Text("action.refresh"));
        AutomationProperties.SetName(emptyProjection, Text("projection.empty"));
        AutomationProperties.SetName(status, status.Text);
        AutomationProperties.SetName(renderer.Surface, Text("heading"));
        AutomationProperties.SetHelpText(source, Text("body"));
        AutomationProperties.SetHelpText(startButton, Text("body"));
    }

    private void ApplyResponsiveLayout(double width)
    {
        var compact = width < CompactWidth;
        bodyGrid.Margin = compact
            ? new Thickness(18, 16)
            : new Thickness(28, 24);
        bodyGrid.ColumnDefinitions = ColumnDefinitions.Parse(compact ? "*" : "2*,3*");
        bodyGrid.RowDefinitions = RowDefinitions.Parse(compact ? "Auto,Auto" : "Auto");
        bodyGrid.ColumnSpacing = compact ? 0 : 18;
        bodyGrid.RowSpacing = compact ? 16 : 0;
        Grid.SetColumn(sourcePanel, 0);
        Grid.SetRow(sourcePanel, 0);
        Grid.SetColumn(projectionPanel, compact ? 0 : 1);
        Grid.SetRow(projectionPanel, compact ? 1 : 0);
        source.MinHeight = compact ? 180 : 300;
        projectionPanel.MinHeight = compact ? 300 : 520;
    }

    private void SetStatus(string key, params object[] values)
    {
        statusKey = key;
        statusValues = values;
        status.Text = Text(key, values);
        status.Foreground = LeserpentTheme.Muted;
        AutomationProperties.SetName(status, status.Text);
    }

    private void SetFailure(string reason)
    {
        statusKey = "status.failed";
        statusValues = [reason];
        status.Text = Text(statusKey, statusValues);
        status.Foreground = LeserpentTheme.Destructive;
        AutomationProperties.SetName(status, status.Text);
    }

    private async void Observe(Task operation)
    {
        try
        {
            await operation;
        }
        catch (Exception error) when (!isClosed)
        {
            SetBusy(false);
            SetFailure(SafeError(error));
        }
        catch (Exception) when (isClosed)
        {
        }
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
        localization.Changed -= OnLocalizationChanged;
        lifetime.Cancel();
        confirmationWindow?.Close(false);
        confirmationWindow = null;
        ownedClient?.Dispose();
        lifetime.Dispose();
    }

    private string Text(string key, params object[] values) => values.Length == 0
        ? DesktopDebuggerCatalogs.Resolve(localization, key)
        : DesktopDebuggerCatalogs.Format(localization, key, values);

    private void ConfigureControl(Control control, string automationId)
    {
        auditedControls.Add(control);
        AutomationProperties.SetAutomationId(control, automationId);
    }

    private static string NewSessionId() => $"avalonia-{Guid.NewGuid():N}";

    private static string SafeError(Exception error)
    {
        var value = error is RemoteDebuggerException remote
            ? remote.Code
            : error.Message;
        var bounded = new string(value
            .Where(character => !char.IsControl(character))
            .Take(240)
            .ToArray());
        return string.IsNullOrWhiteSpace(bounded)
            ? "debugger_operation_failed"
            : bounded;
    }

    private static string? FirstCancelNode(UiDocument document)
    {
        var stack = new Stack<UiNode>();
        stack.Push(document.Root);
        while (stack.Count > 0)
        {
            var node = stack.Pop();
            if (node.Action?.Kind == ActionKind.DebuggerCancel)
            {
                return node.Id;
            }
            for (var index = node.Children.Count - 1; index >= 0; index--)
            {
                stack.Push(node.Children[index]);
            }
        }
        return null;
    }

    private static Border Panel() => new()
    {
        Background = LeserpentTheme.Panel,
        BorderBrush = LeserpentTheme.PanelBorder,
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(10),
        Padding = new Thickness(20),
    };

    private static TextBlock Label() => new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 12,
        FontWeight = FontWeight.SemiBold,
    };

    private static Control Field(TextBlock label, Control control) => new StackPanel
    {
        Spacing = 6,
        Children = { label, control },
    };

    private static Button PrimaryButton() => new()
    {
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.Bold,
        Padding = new Thickness(16, 9),
        Margin = new Thickness(0, 0, 8, 8),
    };

    private static Button SecondaryButton() => new()
    {
        Padding = new Thickness(16, 9),
        Margin = new Thickness(0, 0, 8, 8),
    };
}

internal sealed class DebuggerCancelConfirmationWindow : Window
{
    private readonly string sessionId;
    private readonly string effectId;
    private readonly DesktopLocalization localization;
    private readonly TextBlock heading = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 21,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock body = new()
    {
        Foreground = LeserpentTheme.Body,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock warning = new()
    {
        Foreground = LeserpentTheme.Destructive,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button dismiss = new()
    {
        Padding = new Thickness(15, 8),
    };
    private readonly Button confirm = new()
    {
        Background = LeserpentTheme.Destructive,
        Foreground = Brushes.White,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(15, 8),
    };

    public DebuggerCancelConfirmationWindow(
        string sessionId,
        string effectId,
        DesktopLocalization localization)
    {
        this.sessionId = sessionId;
        this.effectId = effectId;
        this.localization = localization;
        Width = 540;
        MinWidth = 440;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        AutomationProperties.SetAutomationId(heading, "debugger-cancel-heading");
        AutomationProperties.SetAutomationId(body, "debugger-cancel-body");
        AutomationProperties.SetAutomationId(warning, "debugger-cancel-warning");
        AutomationProperties.SetAutomationId(dismiss, "debugger-cancel-dismiss");
        AutomationProperties.SetAutomationId(confirm, "debugger-cancel-confirm");
        dismiss.Click += (_, _) => Close(false);
        confirm.Click += (_, _) => Close(true);
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close(false);
            }
        };
        Content = new Border
        {
            Padding = new Thickness(28, 24),
            Child = new StackPanel
            {
                Spacing = 14,
                Children =
                {
                    heading,
                    body,
                    warning,
                    new StackPanel
                    {
                        Orientation = Orientation.Horizontal,
                        HorizontalAlignment = HorizontalAlignment.Right,
                        Spacing = 10,
                        Children = { dismiss, confirm },
                    },
                },
            },
        };
        localization.Changed += OnLocalizationChanged;
        Closed += (_, _) => localization.Changed -= OnLocalizationChanged;
        ApplyLocalization();
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException(
                "debugger cancellation dialog has no control root");
        }
        root.Measure(new Size(Width, 620));
        if (!double.IsFinite(root.DesiredSize.Width)
            || !double.IsFinite(root.DesiredSize.Height)
            || root.DesiredSize.Width <= 0
            || root.DesiredSize.Height <= 0
            || root.DesiredSize.Width > Width
            || root.DesiredSize.Height > 620)
        {
            throw new InvalidDataException(
                "debugger cancellation dialog exceeded its layout envelope");
        }
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        ApplyLocalization();
    }

    private void ApplyLocalization()
    {
        FlowDirection = localization.FlowDirection;
        Title = Text("confirm.title");
        heading.Text = Text("confirm.title");
        body.Text = Text("confirm.body", effectId, sessionId);
        warning.Text = Text("confirm.warning");
        dismiss.Content = localization.Text(DesktopTextKey.Cancel);
        confirm.Content = Text("confirm.apply");
        AutomationProperties.SetName(heading, heading.Text);
        AutomationProperties.SetName(body, body.Text);
        AutomationProperties.SetName(warning, warning.Text);
        AutomationProperties.SetName(dismiss, localization.Text(DesktopTextKey.Cancel));
        AutomationProperties.SetName(confirm, Text("confirm.apply"));
    }

    private string Text(string key, params object[] values) => values.Length == 0
        ? DesktopDebuggerCatalogs.Resolve(localization, key)
        : DesktopDebuggerCatalogs.Format(localization, key, values);
}
