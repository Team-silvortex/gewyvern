using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed record RetirementHubOperations(
    Func<string, RemoteRetirementIntent, CancellationToken, Task<RemoteRetirementSnapshot>>
        Reconcile);

internal sealed class GewyvernRetirementWindow : Window
{
    private const string Principal = "avalonia-hub";
    private const int MaxAutomaticObservations = 30;
    private static readonly object UnavailableValue = new();
    private readonly DesktopLocalization localization;
    private readonly RetirementHubOperations operations;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<Control> auditedControls = [];
    private readonly ComboBox authority = new();
    private readonly TextBox retirementId = new() { MaxLength = 128 };
    private readonly TextBox provisioningId = new()
    {
        MaxLength = 128,
        PlaceholderText = "provision-production-a",
    };
    private readonly TextBox runtimeId = new()
    {
        MaxLength = 128,
        PlaceholderText = "runtime-production-a",
    };
    private readonly TextBox host = new()
    {
        MaxLength = 253,
        PlaceholderText = "runtime.example",
    };
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
        PlaceholderText = "vault:ssh:runtime-example",
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
    private readonly TextBlock provisioningIdLabel = CreateLabel();
    private readonly TextBlock runtimeIdLabel = CreateLabel();
    private readonly TextBlock hostLabel = CreateLabel();
    private readonly TextBlock portLabel = CreateLabel();
    private readonly TextBlock credentialLabel = CreateLabel();
    private readonly DispatcherTimer polling = new() { Interval = TimeSpan.FromSeconds(2) };
    private RemoteRetirementIntent? intent;
    private RemoteRetirementSnapshot? snapshot;
    private bool operationInFlight;
    private int automaticObservations;
    private string phaseKey = "phase.not_submitted";
    private string? localizedStatusKey = "status.initial";
    private object[] localizedStatusValues = [];

    public GewyvernRetirementWindow(
        IReadOnlyList<BootstrapAuthorityOption> authorities,
        RetirementHubOperations operations,
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
            $"retire-{DateTimeOffset.UtcNow:yyyyMMddHHmmss}-{Guid.NewGuid():N}"[..47];

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
        Closed += (_, _) =>
        {
            polling.Stop();
            localization.Changed -= OnLocalizationChanged;
            lifetime.Cancel();
            lifetime.Dispose();
        };

        Audit(authority, "retirement-authority");
        Audit(retirementId, "retirement-id");
        Audit(provisioningId, "retirement-provisioning-id");
        Audit(runtimeId, "retirement-runtime-id");
        Audit(host, "retirement-host");
        Audit(port, "retirement-port");
        Audit(credentialHandle, "retirement-credential-handle");
        Audit(confirmation, "retirement-confirm");
        Audit(submitButton, "retirement-submit");
        Audit(refreshButton, "retirement-refresh");
        Audit(closeButton, "retirement-close");
        Audit(status, "retirement-status");
        Audit(phase, "retirement-phase");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);

        var target = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,130"),
            ColumnSpacing = 12,
            Children = { Field(hostLabel, host), Field(portLabel, port) },
        };
        Grid.SetColumn(target.Children[1], 1);

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
                                Field(provisioningIdLabel, provisioningId),
                                Field(runtimeIdLabel, runtimeId),
                                target,
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
        if (auditedControls.Count != 13
            || auditedControls.Select(AutomationProperties.GetAutomationId)
                .Distinct(StringComparer.Ordinal).Count() != auditedControls.Count
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control)))
            || AutomationProperties.GetLiveSetting(status) != AutomationLiveSetting.Assertive)
        {
            throw new InvalidDataException("gewyvern retirement control contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("gewyvern retirement window has no control root");
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
                "gewyvern retirement controls exceeded their layout envelope");
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
                "gewyvern retirement localized presentation drifted");
        }
    }

    public async Task ProbeWorkflowAsync(string reprojectLocale)
    {
        var originalRetirementId = retirementId.Text;
        provisioningId.Text = "provision-ui-1";
        runtimeId.Text = "runtime-ui-1";
        host.Text = "runtime.example";
        credentialHandle.Text = "vault:ssh:runtime-example";
        confirmation.IsChecked = true;
        await SubmitAsync();
        localization.SetPreference(reprojectLocale);
        if (retirementId.Text != originalRetirementId
            || provisioningId.Text != "provision-ui-1"
            || runtimeId.Text != "runtime-ui-1"
            || host.Text != "runtime.example"
            || credentialHandle.Text != "vault:ssh:runtime-example"
            || phase.Text != Text("phase.planned")
            || status.Text != Text("status.planned"))
        {
            throw new InvalidDataException(
                "retirement language reprojection changed identity or stale status text");
        }
        await ReconcileAsync(background: false);
        if (snapshot is not
            {
                Phase: "runtime_unregistered",
                ServiceRetired: true,
                RuntimeRegistered: false,
            }
            || intent is null
            || phase.Text != Text("phase.runtime_unregistered")
            || status.Text != Format("status.runtime_unregistered", "runtime-ui-1"))
        {
            throw new InvalidDataException("retirement controls did not unregister the runtime");
        }
    }

    public async Task ProbeConfirmationFenceAsync()
    {
        provisioningId.Text = "provision-ui-1";
        runtimeId.Text = "runtime-ui-1";
        host.Text = "runtime.example";
        credentialHandle.Text = "vault:ssh:runtime-example";
        confirmation.IsChecked = false;
        await SubmitAsync();
        if (snapshot is not null || intent is not null || !submitButton.IsEnabled)
        {
            throw new InvalidDataException("retirement controls crossed the confirmation fence");
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
                "retirement observation limit did not reproject its bounded guidance");
        }
    }

    private async Task SubmitAsync()
    {
        if (operationInFlight || snapshot is not null)
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
            intent = new RemoteRetirementIntent(
                retirementId.Text ?? string.Empty,
                provisioningId.Text ?? string.Empty,
                runtimeId.Text ?? string.Empty,
                host.Text ?? string.Empty,
                checked((ushort)(port.Value ?? 0)),
                credentialHandle.Text ?? string.Empty,
                Principal);
            await RunAsync(() => operations.Reconcile(source.AuthorityId, intent, lifetime.Token));
            LockIdentityFields();
            if (snapshot is { IsTerminal: false })
            {
                polling.Start();
            }
        }
        catch (Exception error) when (IsExpected(error))
        {
            intent = null;
            ShowError(error);
        }
    }

    private async Task ReconcileAsync(bool background)
    {
        if (operationInFlight || intent is null || snapshot is null)
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
            if (!background)
            {
                ShowError(error);
            }
        }
    }

    private async Task RunAsync(
        Func<Task<RemoteRetirementSnapshot>> operation,
        bool background = false)
    {
        operationInFlight = true;
        UpdateActions();
        if (!background)
        {
            ShowLocalizedStatus("status.waiting", LeserpentTheme.Muted);
        }
        try
        {
            snapshot = await operation();
            RenderSnapshot(snapshot);
        }
        finally
        {
            operationInFlight = false;
            UpdateActions();
        }
    }

    private void RenderSnapshot(RemoteRetirementSnapshot state)
    {
        phaseKey = state.Phase switch
        {
            "planned" => "phase.planned",
            "retiring_service" => "phase.retiring_service",
            "service_retired" => "phase.service_retired",
            "runtime_unregistered" => "phase.runtime_unregistered",
            "failed" => "phase.failed",
            _ => throw new InvalidDataException("unsupported retirement phase"),
        };
        phase.Text = Text(phaseKey);
        phase.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : state.RuntimeRegistered ? LeserpentTheme.Primary : LeserpentTheme.Accent;
        var statusForeground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Body;
        var presentation = state.Phase switch
        {
            "planned" => ("status.planned", Array.Empty<object>()),
            "retiring_service" => ("status.retiring_service", Array.Empty<object>()),
            "service_retired" => ("status.service_retired", Array.Empty<object>()),
            "runtime_unregistered" => (
                "status.runtime_unregistered",
                new object[] { SafeValue(state.RuntimeId) }),
            "failed" => (
                "status.failed",
                new object[] { SafeValue(state.FaultCode) }),
            _ => throw new InvalidDataException("unsupported retirement phase"),
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
        provisioningId.IsReadOnly = true;
        runtimeId.IsReadOnly = true;
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

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs) =>
        ApplyLocalization();

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
        provisioningIdLabel.Text = Text("provisioning_id.label");
        runtimeIdLabel.Text = Text("runtime_id.label");
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
        AutomationProperties.SetName(retirementId, Text("a11y.retirement_id"));
        AutomationProperties.SetName(provisioningId, Text("a11y.provisioning_id"));
        AutomationProperties.SetName(runtimeId, Text("a11y.runtime_id"));
        AutomationProperties.SetName(host, Text("a11y.host"));
        AutomationProperties.SetName(port, Text("a11y.port"));
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
        DesktopRetirementCatalogs.Resolve(localization, key);

    private string Format(string key, params object[] values) =>
        DesktopRetirementCatalogs.Format(
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
        or RemoteRetirementException or OperationCanceledException;

    private static object SafeValue(string? value) => value is null
        ? UnavailableValue
        : new string(value.Where(character => !char.IsControl(character)).Take(512).ToArray());

    private static string SafeRaw(string value) =>
        new(value.Where(character => !char.IsControl(character)).Take(512).ToArray());
}
