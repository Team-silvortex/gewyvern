using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed record DaemonRetirementHubOperations(
    Func<string, RemoteDaemonRetirementIntent, CancellationToken,
        Task<RemoteDaemonRetirementSnapshot>> Reconcile);

internal sealed class DaemonRetirementWindow : Window
{
    private const string Principal = "avalonia-hub";
    private const int MaxAutomaticObservations = 30;
    private static readonly object UnavailableValue = new();
    private readonly DesktopLocalization localization;
    private readonly DaemonRetirementHubOperations operations;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<Control> auditedControls = [];
    private readonly ComboBox authority = new();
    private readonly TextBox retirementId = new() { MaxLength = 128 };
    private readonly TextBox bootstrapId = new()
    {
        MaxLength = 128,
        PlaceholderText = "desktop-20260728-bootstrap",
    };
    private readonly TextBox credentialHandle = new()
    {
        MaxLength = 128,
        PlaceholderText = "vault:ssh:daemon-example",
    };
    private readonly CheckBox confirmation = new()
    {
        Foreground = LeserpentTheme.Body,
    };
    private readonly Button submitButton = DestructiveButton();
    private readonly Button refreshButton = new()
    {
        Padding = new Thickness(16, 9),
        Margin = new Thickness(5),
        IsEnabled = false,
    };
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
        Foreground = LeserpentTheme.Destructive,
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
    private readonly TextBlock retirementIdLabel = CreateLabel();
    private readonly TextBlock bootstrapIdLabel = CreateLabel();
    private readonly TextBlock credentialLabel = CreateLabel();
    private readonly DispatcherTimer polling = new() { Interval = TimeSpan.FromSeconds(2) };
    private RemoteDaemonRetirementIntent? intent;
    private RemoteDaemonRetirementSnapshot? snapshot;
    private bool operationInFlight;
    private bool isClosed;
    private bool lifetimeDisposed;
    private int automaticObservations;
    private string phaseKey = "phase.not_submitted";
    private string? localizedStatusKey = "status.initial";
    private object[] localizedStatusValues = [];

    public DaemonRetirementWindow(
        IReadOnlyList<BootstrapAuthorityOption> authorities,
        DaemonRetirementHubOperations operations,
        DesktopLocalization localization)
    {
        this.operations = operations;
        this.localization = localization;
        Width = 720;
        MinWidth = 590;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        authority.ItemsSource = authorities;
        authority.SelectedIndex = authorities.Count > 0 ? 0 : -1;
        authority.IsEnabled = authorities.Count > 1;
        retirementId.Text =
            $"retire-daemon-{DateTimeOffset.UtcNow:yyyyMMddHHmmss}-{Guid.NewGuid():N}"[..54];

        closeButton.Click += (_, _) => Close();
        submitButton.Click += async (_, _) => await SubmitAsync();
        refreshButton.Click += async (_, _) => await ReconcileAsync(background: false);
        polling.Tick += async (_, _) => await ReconcileAsync(background: true);
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close();
            }
        };
        Closed += OnClosed;

        Audit(authority, "daemon-retirement-authority");
        Audit(retirementId, "daemon-retirement-id");
        Audit(bootstrapId, "daemon-retirement-bootstrap-id");
        Audit(credentialHandle, "daemon-retirement-credential-handle");
        Audit(confirmation, "daemon-retirement-confirm");
        Audit(submitButton, "daemon-retirement-submit");
        Audit(refreshButton, "daemon-retirement-refresh");
        Audit(closeButton, "daemon-retirement-close");
        Audit(status, "daemon-retirement-status");
        Audit(phase, "daemon-retirement-phase");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);

        Content = new ScrollViewer
        {
            VerticalScrollBarVisibility = Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
            Content = new StackPanel
            {
                Margin = new Thickness(30, 26),
                Spacing = 18,
                Children =
                {
                    new StackPanel
                    {
                        Spacing = 5,
                        Children =
                        {
                            kickerText,
                            headingText,
                            descriptionText,
                        },
                    },
                    new Border
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
                                Field(retirementIdLabel, retirementId),
                                Field(bootstrapIdLabel, bootstrapId),
                                Field(credentialLabel, credentialHandle),
                                confirmation,
                            },
                        },
                    },
                    new Border
                    {
                        Background = Brush.Parse("#17140E"),
                        BorderBrush = LeserpentTheme.PanelBorder,
                        BorderThickness = new Thickness(1),
                        CornerRadius = new CornerRadius(10),
                        Padding = new Thickness(18, 14),
                        Child = new StackPanel { Spacing = 6, Children = { phase, status } },
                    },
                    new WrapPanel
                    {
                        Orientation = Orientation.Horizontal,
                        HorizontalAlignment = HorizontalAlignment.Right,
                        Children = { closeButton, refreshButton, submitButton },
                    },
                },
            },
        };
        localization.Changed += OnLocalizationChanged;
        ApplyLocalization();
    }

    public void VerifyAccessibility()
    {
        if (auditedControls.Count != 10
            || auditedControls.Select(AutomationProperties.GetAutomationId)
                .Distinct(StringComparer.Ordinal).Count() != auditedControls.Count
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))))
        {
            throw new InvalidDataException("daemon retirement control contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("daemon retirement window has no control root");
        }
        root.Measure(new Size(Width, 1400));
        var desired = root.DesiredSize;
        if (!double.IsFinite(desired.Width)
            || !double.IsFinite(desired.Height)
            || desired.Width <= 0
            || desired.Height <= 0
            || desired.Width > Width
            || desired.Height > 1400)
        {
            throw new InvalidDataException(
                "daemon retirement controls exceeded their layout envelope");
        }
    }

    public void ProbeLocalizedPresentation(
        string expectedTitle,
        string expectedHeading,
        string expectedSubmit,
        string expectedPhase,
        string expectedStatus)
    {
        if (Title != expectedTitle
            || headingText.Text != expectedHeading
            || submitButton.Content as string != expectedSubmit
            || AutomationProperties.GetName(submitButton) != Text("a11y.submit")
            || phase.Text != expectedPhase
            || status.Text != expectedStatus
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "daemon retirement localized presentation drifted");
        }
    }

    public async Task ProbeWorkflowAsync(string reprojectLocale)
    {
        var originalRetirementId = retirementId.Text;
        bootstrapId.Text = "bootstrap-ui-1";
        credentialHandle.Text = "vault:ssh:daemon-example";
        confirmation.IsChecked = true;
        await SubmitAsync();
        localization.SetPreference(reprojectLocale);
        if (retirementId.Text != originalRetirementId
            || bootstrapId.Text != "bootstrap-ui-1"
            || credentialHandle.Text != "vault:ssh:daemon-example"
            || phase.Text != Text("phase.planned")
            || status.Text != Format(
                "status.planned",
                "bootstrap-ui-1",
                "daemon-target"))
        {
            throw new InvalidDataException(
                "daemon retirement language reprojection changed identity or stale status text");
        }
        await ReconcileAsync(background: false);
        if (snapshot is not { Phase: "service_retired", ServiceRetired: true }
            || intent is null
            || phase.Text != Text("phase.service_retired")
            || status.Text != Format(
                "status.service_retired",
                "daemon-target",
                "daemon.example",
                22))
        {
            throw new InvalidDataException(
                "daemon retirement controls did not retire the service");
        }
    }

    public async Task ProbeObservationLimitAsync(string reprojectLocale)
    {
        automaticObservations = MaxAutomaticObservations;
        await ReconcileAsync(background: true);
        localization.SetPreference(reprojectLocale);
        if (status.Text != Text("status.observation_limit"))
        {
            throw new InvalidDataException(
                "daemon retirement observation limit did not reproject its bounded guidance");
        }
    }

    public async Task ProbeConfirmationFenceAsync()
    {
        bootstrapId.Text = "bootstrap-ui-1";
        credentialHandle.Text = "vault:ssh:daemon-example";
        confirmation.IsChecked = false;
        await SubmitAsync();
        if (snapshot is not null || intent is not null || !submitButton.IsEnabled)
        {
            throw new InvalidDataException(
                "daemon retirement controls crossed the confirmation fence");
        }
    }

    public static async Task ProbeLateCompletionCloseFenceAsync(
        IReadOnlyList<BootstrapAuthorityOption> authorities)
    {
        RemoteDaemonRetirementIntent? submitted = null;
        var started = new TaskCompletionSource<bool>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var completion = new TaskCompletionSource<RemoteDaemonRetirementSnapshot>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var window = new DaemonRetirementWindow(
            authorities,
            new DaemonRetirementHubOperations((_, intent, _) =>
            {
                submitted = intent;
                started.TrySetResult(true);
                return completion.Task;
            }),
            DesktopLocalization.ForVerification());
        window.bootstrapId.Text = "bootstrap-late";
        window.credentialHandle.Text = "vault:ssh:late-example";
        window.confirmation.IsChecked = true;

        var pending = window.SubmitAsync();
        await started.Task;
        window.OnClosed(null, EventArgs.Empty);
        var intent = submitted
            ?? throw new InvalidDataException("daemon retirement close-fence probe did not submit");
        completion.SetResult(new RemoteDaemonRetirementSnapshot(
            intent.RetirementId,
            intent.BootstrapId,
            "daemon-late",
            "planned",
            "ssh",
            "late.example",
            22,
            new string('a', 64),
            "system",
            true,
            false,
            null));
        await pending;
        window.VerifyLateCompletionCloseFence();
    }

    private async Task SubmitAsync()
    {
        if (isClosed || operationInFlight || snapshot is not null)
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
            intent = new RemoteDaemonRetirementIntent(
                retirementId.Text ?? string.Empty,
                bootstrapId.Text ?? string.Empty,
                credentialHandle.Text ?? string.Empty,
                Principal);
            await RunAsync(() => operations.Reconcile(source.AuthorityId, intent, lifetime.Token));
            if (isClosed)
            {
                return;
            }
            LockIdentityFields();
            if (snapshot is { IsTerminal: false })
            {
                polling.Start();
            }
        }
        catch (Exception error) when (IsExpected(error))
        {
            intent = null;
            if (!isClosed)
            {
                ShowError(error);
            }
        }
    }

    private async Task ReconcileAsync(bool background)
    {
        if (isClosed || operationInFlight || intent is null || snapshot is null)
        {
            return;
        }
        if (background && automaticObservations >= MaxAutomaticObservations)
        {
            polling.Stop();
            ShowLocalizedStatus("status.observation_limit", LeserpentTheme.Primary);
            return;
        }
        try
        {
            if (background)
            {
                automaticObservations++;
            }
            var source = SelectedAuthority();
            await RunAsync(
                () => operations.Reconcile(source.AuthorityId, intent, lifetime.Token),
                background);
        }
        catch (Exception error) when (IsExpected(error))
        {
            if (!background && !isClosed)
            {
                ShowError(error);
            }
        }
    }

    private async Task RunAsync(
        Func<Task<RemoteDaemonRetirementSnapshot>> operation,
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
            || bootstrapId.IsReadOnly)
        {
            throw new InvalidDataException(
                "daemon retirement controls accepted a late completion after close");
        }
    }

    private void RenderSnapshot(RemoteDaemonRetirementSnapshot state)
    {
        phaseKey = state.Phase switch
        {
            "planned" => "phase.planned",
            "retiring_service" => "phase.retiring_service",
            "service_retired" => "phase.service_retired",
            "failed" => "phase.failed",
            _ => throw new InvalidDataException("unsupported daemon retirement phase"),
        };
        phase.Text = Text(phaseKey);
        phase.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : state.ServiceRetired ? LeserpentTheme.Accent : LeserpentTheme.Primary;
        var statusForeground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Body;
        var presentation = state.Phase switch
        {
            "planned" => (
                "status.planned",
                new object[] { SafeValue(state.BootstrapId), SafeValue(state.DaemonId) }),
            "retiring_service" => (
                "status.retiring_service",
                new object[] { SafeValue(state.DaemonId), SafeValue(state.InstallProfile) }),
            "service_retired" => (
                "status.service_retired",
                new object[] { SafeValue(state.DaemonId), SafeValue(state.Host), state.Port }),
            "failed" => (
                "status.failed",
                new object[] { SafeValue(state.FaultCode) }),
            _ => throw new InvalidDataException("unsupported daemon retirement phase"),
        };
        ShowLocalizedStatus(presentation.Item1, statusForeground, presentation.Item2);
        if (state.IsTerminal)
        {
            polling.Stop();
        }
    }

    private void UpdateActions()
    {
        submitButton.IsEnabled = !operationInFlight && snapshot is null;
        refreshButton.IsEnabled = !operationInFlight && snapshot is not null;
    }

    private void LockIdentityFields()
    {
        authority.IsEnabled = false;
        retirementId.IsReadOnly = true;
        bootstrapId.IsReadOnly = true;
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
        submitButton.Content = Text("submit");
        refreshButton.Content = Text("refresh");
        closeButton.Content = Text("close");
        kickerText.Text = Text("kicker");
        headingText.Text = Text("heading");
        descriptionText.Text = Text("body");
        authorityLabel.Text = Text("authority.label");
        retirementIdLabel.Text = Text("retirement_id.label");
        bootstrapIdLabel.Text = Text("bootstrap_id.label");
        credentialLabel.Text = Text("credential.label");
        phase.Text = Text(phaseKey);
        if (localizedStatusKey is { } statusKey)
        {
            status.Text = localizedStatusValues.Length == 0
                ? Text(statusKey)
                : Format(statusKey, localizedStatusValues);
        }

        AutomationProperties.SetName(authority, Text("a11y.authority"));
        AutomationProperties.SetName(retirementId, Text("a11y.retirement_id"));
        AutomationProperties.SetName(bootstrapId, Text("a11y.bootstrap_id"));
        AutomationProperties.SetName(credentialHandle, Text("a11y.credential"));
        AutomationProperties.SetName(confirmation, Text("a11y.confirm"));
        AutomationProperties.SetName(submitButton, Text("a11y.submit"));
        AutomationProperties.SetName(refreshButton, Text("a11y.refresh"));
        AutomationProperties.SetName(closeButton, Text("a11y.close"));
        AutomationProperties.SetName(status, Text("status.name"));
        AutomationProperties.SetName(phase, Text("phase.name"));
    }

    private void Audit(Control control, string id)
    {
        AutomationProperties.SetAutomationId(control, id);
        auditedControls.Add(control);
    }

    private string Text(string key) =>
        DesktopDaemonRetirementCatalogs.Resolve(localization, key);

    private string Format(string key, params object[] values) =>
        DesktopDaemonRetirementCatalogs.Format(
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

    private static Button DestructiveButton() => new()
    {
        Background = LeserpentTheme.Destructive,
        Foreground = Brushes.White,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(17, 9),
        Margin = new Thickness(5),
    };

    private static bool IsExpected(Exception error) => error is ArgumentException
        or InvalidDataException or IOException or HttpRequestException
        or RemoteDaemonRetirementException or OperationCanceledException;

    private static object SafeValue(string? value) => value is null
        ? UnavailableValue
        : new string(value.Where(character => !char.IsControl(character)).Take(512).ToArray());

    private static string SafeRaw(string value) =>
        new(value.Where(character => !char.IsControl(character)).Take(512).ToArray());
}
