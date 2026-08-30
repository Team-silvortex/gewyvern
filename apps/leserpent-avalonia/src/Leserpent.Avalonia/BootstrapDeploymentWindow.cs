using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed record BootstrapAuthorityOption(
    string AuthorityId,
    string DisplayName,
    string Endpoint,
    bool CanPromote)
{
    public override string ToString() => $"{DisplayName}  /  {Endpoint}";
}

internal sealed record BootstrapHubOperations(
    Func<string, RemoteBootstrapIntent, CancellationToken, Task<RemoteBootstrapSnapshot>> Submit,
    Func<string, string, string, CancellationToken, Task<RemoteBootstrapSnapshot>> Inspect,
    Func<string, string, string, CancellationToken, Task<RemoteBootstrapSnapshot>> Bind,
    Func<string, RemoteBootstrapSnapshot, CancellationToken, Task> Promote);

internal sealed class BootstrapDeploymentWindow : Window
{
    private const string Principal = "avalonia-hub";
    private static readonly object UnavailableValue = new();
    private readonly DesktopLocalization localization;
    private readonly BootstrapHubOperations operations;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<Control> auditedControls = [];
    private readonly ComboBox authority = new();
    private readonly TextBox bootstrapId = new() { MaxLength = 128 };
    private readonly TextBox host = new() { MaxLength = 253, PlaceholderText = "host.example" };
    private readonly NumericUpDown port = new()
    {
        Minimum = 1,
        Maximum = 65535,
        Value = 22,
        FormatString = "0",
    };
    private readonly TextBox credentialHandle = new()
    {
        MaxLength = 128,
        PlaceholderText = "vault:ssh:host-example",
    };
    private readonly CheckBox confirmation = new()
    {
        Foreground = LeserpentTheme.Body,
    };
    private readonly Button submitButton = PrimaryButton();
    private readonly Button refreshButton = new()
    {
        Padding = new Thickness(16, 9),
        Margin = new Thickness(5),
        IsEnabled = false,
    };
    private readonly Button bindButton = new()
    {
        Padding = new Thickness(16, 9),
        Margin = new Thickness(5),
        IsEnabled = false,
    };
    private readonly Button promoteButton = PrimaryButton();
    private readonly Button closeButton = new()
    {
        Padding = new Thickness(16, 9),
        Margin = new Thickness(5),
    };
    private readonly TextBlock status = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock phase = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 1,
    };
    private readonly TextBlock kickerText = new()
    {
        Foreground = LeserpentTheme.Accent,
        FontSize = 12,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 2,
    };
    private readonly TextBlock headingText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 27,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock descriptionText = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock authorityLabel = CreateLabel();
    private readonly TextBlock bootstrapIdLabel = CreateLabel();
    private readonly TextBlock hostLabel = CreateLabel();
    private readonly TextBlock portLabel = CreateLabel();
    private readonly TextBlock credentialLabel = CreateLabel();
    private readonly DispatcherTimer polling = new() { Interval = TimeSpan.FromSeconds(2) };
    private RemoteBootstrapSnapshot? snapshot;
    private bool operationInFlight;
    private bool isClosed;
    private bool lifetimeDisposed;
    private bool promotionCompleted;
    private string phaseKey = "phase.not_submitted";
    private string? localizedStatusKey = "status.initial";
    private object[] localizedStatusValues = [];

    public BootstrapDeploymentWindow(
        IReadOnlyList<BootstrapAuthorityOption> authorities,
        BootstrapHubOperations operations,
        DesktopLocalization localization)
    {
        this.operations = operations;
        this.localization = localization;
        Width = 700;
        MinWidth = 580;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        authority.ItemsSource = authorities;
        authority.SelectedIndex = authorities.Count > 0 ? 0 : -1;
        authority.IsEnabled = authorities.Count > 1;
        promoteButton.IsEnabled = false;
        bootstrapId.Text = $"desktop-{DateTimeOffset.UtcNow:yyyyMMddHHmmss}-{Guid.NewGuid():N}"[..48];

        closeButton.Click += (_, _) => Close();
        submitButton.Click += async (_, _) => await SubmitAsync();
        refreshButton.Click += async (_, _) => await RefreshAsync();
        bindButton.Click += async (_, _) => await BindAsync();
        promoteButton.Click += async (_, _) => await PromoteAsync();
        polling.Tick += async (_, _) => await RefreshAsync(background: true);
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close();
            }
        };
        Closed += OnClosed;

        Audit(authority, "bootstrap-authority");
        Audit(bootstrapId, "bootstrap-id");
        Audit(host, "bootstrap-host");
        Audit(port, "bootstrap-port");
        Audit(credentialHandle, "bootstrap-credential-handle");
        Audit(confirmation, "bootstrap-confirm");
        Audit(submitButton, "bootstrap-submit");
        Audit(refreshButton, "bootstrap-refresh");
        Audit(bindButton, "bootstrap-bind");
        Audit(promoteButton, "bootstrap-promote");
        Audit(closeButton, "bootstrap-close");
        Audit(status, "bootstrap-status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);

        var heading = new StackPanel
        {
            Spacing = 5,
            Children =
            {
                kickerText,
                headingText,
                descriptionText,
            },
        };

        var target = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,130"),
            ColumnSpacing = 12,
            Children = { Field(hostLabel, host), Field(portLabel, port) },
        };
        Grid.SetColumn(target.Children[1], 1);

        var form = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(10),
            Padding = new Thickness(20),
            Child = new StackPanel
            {
                Spacing = 14,
                Children =
                {
                    Field(authorityLabel, authority),
                    Field(bootstrapIdLabel, bootstrapId),
                    target,
                    Field(credentialLabel, credentialHandle),
                    confirmation,
                },
            },
        };

        var state = new Border
        {
            Background = Brush.Parse("#17140E"),
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(10),
            Padding = new Thickness(18, 14),
            Child = new StackPanel { Spacing = 6, Children = { phase, status } },
        };

        Content = new ScrollViewer
        {
            VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
            Content = new StackPanel
            {
                Margin = new Thickness(30, 26),
                Spacing = 18,
                Children =
                {
                    heading,
                    form,
                    state,
                    new WrapPanel
                    {
                        Orientation = Orientation.Horizontal,
                        HorizontalAlignment = HorizontalAlignment.Right,
                        Children =
                        {
                            closeButton,
                            refreshButton,
                            bindButton,
                            promoteButton,
                            submitButton,
                        },
                    },
                },
            },
        };
        localization.Changed += OnLocalizationChanged;
        ApplyLocalization();
    }

    public void VerifyAccessibility()
    {
        if (auditedControls.Count != 12
            || auditedControls.Select(AutomationProperties.GetAutomationId)
                .Distinct(StringComparer.Ordinal).Count() != auditedControls.Count
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control)))
            || AutomationProperties.GetLiveSetting(status) != AutomationLiveSetting.Assertive)
        {
            throw new InvalidDataException("bootstrap deployment control contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("bootstrap deployment window has no control root");
        }
        root.Measure(new Size(Width, 1600));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 1600)
        {
            throw new InvalidDataException(
                "bootstrap deployment controls exceeded their layout envelope");
        }
    }

    public void ProbeLocalizedPresentation(
        string expectedTitle,
        string expectedHeading,
        string expectedDeploy,
        string expectedPhase,
        string expectedStatus)
    {
        if (Title != expectedTitle
            || headingText.Text != expectedHeading
            || submitButton.Content as string != expectedDeploy
            || AutomationProperties.GetName(submitButton) != Text("a11y.deploy")
            || phase.Text != expectedPhase
            || status.Text != expectedStatus
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "bootstrap deployment localized presentation drifted");
        }
    }

    public async Task ProbeWorkflowAsync(string reprojectLocale)
    {
        host.Text = "target.example";
        credentialHandle.Text = "vault:ssh:target-example";
        confirmation.IsChecked = true;
        await SubmitAsync();
        localization.SetPreference(reprojectLocale);
        if (host.Text != "target.example"
            || credentialHandle.Text != "vault:ssh:target-example"
            || phase.Text != Text("phase.planned")
            || status.Text != Text("status.planned"))
        {
            throw new InvalidDataException(
                "bootstrap language reprojection changed operator input or stale status text");
        }
        await RefreshAsync();
        await BindAsync();
        await PromoteAsync();
        if (snapshot is not { Phase: "session_bound", MutationAuthorized: true }
            || !promotionCompleted
            || phase.Text != Text("phase.session_bound")
            || status.Text != Format("status.promoted", "daemon-target"))
        {
            throw new InvalidDataException("bootstrap deployment controls did not complete binding");
        }
    }

    public async Task ProbeConfirmationFenceAsync()
    {
        host.Text = "target.example";
        credentialHandle.Text = "vault:ssh:target-example";
        confirmation.IsChecked = false;
        await SubmitAsync();
        if (snapshot is not null || !submitButton.IsEnabled)
        {
            throw new InvalidDataException(
                "bootstrap controls crossed the explicit confirmation fence");
        }
    }

    public static async Task ProbeLateCompletionCloseFenceAsync(
        IReadOnlyList<BootstrapAuthorityOption> authorities)
    {
        RemoteBootstrapIntent? submitted = null;
        var started = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var completion = new TaskCompletionSource<RemoteBootstrapSnapshot>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var window = new BootstrapDeploymentWindow(
            authorities,
            new BootstrapHubOperations(
                (_, intent, _) =>
                {
                    submitted = intent;
                    started.TrySetResult(true);
                    return completion.Task;
                },
                static (_, _, _, _) => throw new InvalidOperationException(),
                static (_, _, _, _) => throw new InvalidOperationException(),
                static (_, _, _) => throw new InvalidOperationException()),
            DesktopLocalization.ForVerification());
        window.host.Text = "late.example";
        window.credentialHandle.Text = "vault:ssh:late-example";
        window.confirmation.IsChecked = true;

        var pending = window.SubmitAsync();
        await started.Task;
        window.OnClosed(null, EventArgs.Empty);
        var intent = submitted
            ?? throw new InvalidDataException("bootstrap close-fence probe did not submit");
        completion.SetResult(new RemoteBootstrapSnapshot(
            intent.BootstrapId,
            "planned",
            "ssh",
            intent.Host,
            intent.Port,
            true,
            null,
            null,
            null,
            null,
            null,
            false));
        await pending;
        window.VerifyLateCompletionCloseFence();
    }

    private async Task SubmitAsync()
    {
        if (isClosed || operationInFlight)
        {
            return;
        }
        try
        {
            if (confirmation.IsChecked != true)
            {
                ShowLocalizedStatus(
                    "error.confirm_required",
                    LeserpentTheme.Destructive);
                return;
            }
            if (SelectedAuthorityOrNull() is not { } source)
            {
                ShowLocalizedStatus(
                    "error.authority_required",
                    LeserpentTheme.Destructive);
                return;
            }
            var intent = new RemoteBootstrapIntent(
                bootstrapId.Text ?? string.Empty,
                host.Text ?? string.Empty,
                checked((ushort)(port.Value ?? 0)),
                credentialHandle.Text ?? string.Empty,
                Principal);
            await RunAsync(() => operations.Submit(
                source.AuthorityId,
                intent,
                lifetime.Token));
            if (isClosed)
            {
                return;
            }
            LockIdentityFields();
            if (snapshot is { IsTerminal: false, CanBind: false })
            {
                polling.Start();
            }
        }
        catch (Exception error) when (IsExpected(error))
        {
            if (!isClosed)
            {
                ShowError(error);
            }
        }
    }

    private async Task RefreshAsync(bool background = false)
    {
        if (isClosed || operationInFlight || snapshot is null)
        {
            return;
        }
        try
        {
            var source = SelectedAuthority();
            await RunAsync(() => operations.Inspect(
                source.AuthorityId,
                snapshot.BootstrapId,
                Principal,
                lifetime.Token), background);
        }
        catch (Exception error) when (IsExpected(error))
        {
            if (!background && !isClosed)
            {
                ShowError(error);
            }
        }
    }

    private async Task BindAsync()
    {
        if (isClosed || operationInFlight || snapshot is not { CanBind: true })
        {
            return;
        }
        try
        {
            var source = SelectedAuthority();
            await RunAsync(() => operations.Bind(
                source.AuthorityId,
                snapshot.BootstrapId,
                Principal,
                lifetime.Token));
        }
        catch (Exception error) when (IsExpected(error))
        {
            if (!isClosed)
            {
                ShowError(error);
            }
        }
    }

    private async Task PromoteAsync()
    {
        if (isClosed)
        {
            return;
        }
        if (SelectedAuthorityOrNull() is not { } source)
        {
            ShowLocalizedStatus("error.authority_required", LeserpentTheme.Destructive);
            return;
        }
        if (operationInFlight
            || promotionCompleted
            || !source.CanPromote
            || snapshot is not { Phase: "session_bound", MutationAuthorized: true } state)
        {
            return;
        }
        operationInFlight = true;
        UpdateActions();
        ShowLocalizedStatus("status.promoting", LeserpentTheme.Muted);
        try
        {
            await operations.Promote(source.AuthorityId, state, lifetime.Token);
            if (isClosed)
            {
                return;
            }
            promotionCompleted = true;
            ShowLocalizedStatus(
                "status.promoted",
                LeserpentTheme.Accent,
                SafeValue(state.DaemonId));
        }
        catch (Exception error) when (IsExpected(error))
        {
            if (!isClosed)
            {
                ShowError(error);
            }
        }
        finally
        {
            FinishOperation();
        }
    }

    private async Task RunAsync(
        Func<Task<RemoteBootstrapSnapshot>> operation,
        bool background = false)
    {
        if (isClosed)
        {
            return;
        }
        operationInFlight = true;
        UpdateActions();
        if (!background)
        {
            ShowLocalizedStatus("status.waiting", LeserpentTheme.Muted);
        }
        try
        {
            var completed = await operation();
            if (!isClosed)
            {
                snapshot = completed;
                RenderSnapshot(completed);
            }
        }
        finally
        {
            FinishOperation();
        }
    }

    private void FinishOperation()
    {
        operationInFlight = false;
        if (!isClosed)
        {
            UpdateActions();
        }
        DisposeLifetimeIfIdle();
    }

    private void OnClosed(object? sender, EventArgs eventArgs)
    {
        if (isClosed)
        {
            return;
        }
        isClosed = true;
        polling.Stop();
        localization.Changed -= OnLocalizationChanged;
        lifetime.Cancel();
        DisposeLifetimeIfIdle();
    }

    private void DisposeLifetimeIfIdle()
    {
        if (!isClosed || operationInFlight || lifetimeDisposed)
        {
            return;
        }
        lifetime.Dispose();
        lifetimeDisposed = true;
    }

    private void VerifyLateCompletionCloseFence()
    {
        if (!isClosed
            || snapshot is not null
            || operationInFlight
            || polling.IsEnabled
            || !lifetimeDisposed
            || host.IsReadOnly)
        {
            throw new InvalidDataException(
                "bootstrap controls accepted a late completion after close");
        }
    }

    private void RenderSnapshot(RemoteBootstrapSnapshot state)
    {
        phaseKey = state.Phase switch
        {
            "planned" => "phase.planned",
            "deploying" => "phase.deploying",
            "bootstrapped" => "phase.bootstrapped",
            "session_bound" => "phase.session_bound",
            "failed" => "phase.failed",
            _ => throw new InvalidDataException("unsupported bootstrap phase"),
        };
        phase.Text = Text(phaseKey);
        phase.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : state.MutationAuthorized ? LeserpentTheme.Accent : LeserpentTheme.Primary;
        var statusForeground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Body;
        var presentation = state.Phase switch
        {
            "planned" => ("status.planned", Array.Empty<object>()),
            "deploying" => ("status.deploying", Array.Empty<object>()),
            "bootstrapped" => (
                "status.bootstrapped",
                new object[] { SafeValue(state.DaemonId), SafeValue(state.Endpoint) }),
            "session_bound" => (
                "status.session_bound",
                new object[] { SafeValue(state.DaemonId) }),
            "failed" => (
                "status.failed",
                new object[] { SafeValue(state.FaultCode) }),
            _ => throw new InvalidDataException("unsupported bootstrap phase"),
        };
        ShowLocalizedStatus(presentation.Item1, statusForeground, presentation.Item2);
        if (state.IsTerminal || state.CanBind)
        {
            polling.Stop();
        }
    }

    private void UpdateActions()
    {
        submitButton.IsEnabled = !operationInFlight && snapshot is null;
        refreshButton.IsEnabled = !operationInFlight && snapshot is not null;
        bindButton.IsEnabled = !operationInFlight && snapshot is { CanBind: true };
        promoteButton.IsEnabled = !operationInFlight
            && !promotionCompleted
            && SelectedAuthorityOrNull() is { CanPromote: true }
            && snapshot is { Phase: "session_bound", MutationAuthorized: true };
    }

    private void LockIdentityFields()
    {
        authority.IsEnabled = false;
        bootstrapId.IsReadOnly = true;
        host.IsReadOnly = true;
        port.IsReadOnly = true;
        credentialHandle.IsReadOnly = true;
        confirmation.IsEnabled = false;
    }

    private BootstrapAuthorityOption SelectedAuthority() =>
        authority.SelectedItem as BootstrapAuthorityOption
        ?? throw new ArgumentException(Text("error.authority_required"));

    private BootstrapAuthorityOption? SelectedAuthorityOrNull() =>
        authority.SelectedItem as BootstrapAuthorityOption;

    private void ShowError(Exception error)
    {
        localizedStatusKey = null;
        localizedStatusValues = [];
        status.Text = SafeRaw(error.Message);
        status.Foreground = LeserpentTheme.Destructive;
    }

    private void ShowLocalizedStatus(
        string key,
        IBrush foreground,
        params object[] values)
    {
        localizedStatusKey = key;
        localizedStatusValues = [.. values];
        status.Text = values.Length == 0 ? Text(key) : Format(key, values);
        status.Foreground = foreground;
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs)
    {
        if (!isClosed)
        {
            ApplyLocalization();
        }
    }

    private void ApplyLocalization()
    {
        Title = Text("title");
        FlowDirection = localization.FlowDirection;
        confirmation.Content = Text("confirmation");
        submitButton.Content = Text("deploy");
        refreshButton.Content = Text("refresh");
        bindButton.Content = Text("bind");
        promoteButton.Content = Text("promote");
        closeButton.Content = Text("close");
        kickerText.Text = Text("kicker");
        headingText.Text = Text("heading");
        descriptionText.Text = Text("body");
        authorityLabel.Text = Text("authority.label");
        bootstrapIdLabel.Text = Text("bootstrap_id.label");
        hostLabel.Text = Text("host.label");
        portLabel.Text = Text("port.label");
        credentialLabel.Text = Text("credential.label");
        phase.Text = Text(phaseKey);
        if (localizedStatusKey is { } statusKey)
        {
            status.Text = localizedStatusValues.Length == 0
                ? Text(statusKey)
                : Format(statusKey, localizedStatusValues);
        }

        AutomationProperties.SetName(authority, Text("a11y.authority"));
        AutomationProperties.SetName(bootstrapId, Text("a11y.bootstrap_id"));
        AutomationProperties.SetName(host, Text("a11y.host"));
        AutomationProperties.SetName(port, Text("a11y.port"));
        AutomationProperties.SetName(credentialHandle, Text("a11y.credential"));
        AutomationProperties.SetName(confirmation, Text("a11y.confirm"));
        AutomationProperties.SetName(submitButton, Text("a11y.deploy"));
        AutomationProperties.SetName(refreshButton, Text("a11y.refresh"));
        AutomationProperties.SetName(bindButton, Text("a11y.bind"));
        AutomationProperties.SetName(promoteButton, Text("a11y.promote"));
        AutomationProperties.SetName(closeButton, Text("a11y.close"));
        AutomationProperties.SetName(status, Text("status.name"));
        AutomationProperties.SetHelpText(status, Text("a11y.status"));
    }

    private void Audit(Control control, string id)
    {
        AutomationProperties.SetAutomationId(control, id);
        auditedControls.Add(control);
    }

    private string Text(string key) =>
        DesktopBootstrapDeploymentCatalogs.Resolve(localization, key);

    private string Format(string key, params object[] values) =>
        DesktopBootstrapDeploymentCatalogs.Format(
            localization,
            key,
            values.Select(value => ReferenceEquals(value, UnavailableValue)
                ? Text("unavailable")
                : value).ToArray());

    private static StackPanel Field(TextBlock label, Control control) => new()
    {
        Spacing = 6,
        Children = { label, control },
    };

    private static TextBlock CreateLabel() => new()
    {
        Foreground = LeserpentTheme.Body,
        FontWeight = FontWeight.SemiBold,
        FontSize = 12,
    };

    private static Button PrimaryButton() => new()
    {
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(17, 9),
        Margin = new Thickness(5),
    };

    private static bool IsExpected(Exception error) => error is ArgumentException
        or InvalidDataException or IOException or HttpRequestException
        or RemoteBootstrapException or OperationCanceledException;

    private static object SafeValue(string? value) => value is null
        ? UnavailableValue
        : new string(value.Where(character => !char.IsControl(character)).Take(512).ToArray());

    private static string SafeRaw(string value) =>
        new(value.Where(character => !char.IsControl(character)).Take(512).ToArray());
}
