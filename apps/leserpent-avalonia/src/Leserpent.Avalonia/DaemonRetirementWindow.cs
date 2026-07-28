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
        Content = "I confirm removal of the leserpentd service created by this bootstrap",
        Foreground = LeserpentTheme.Body,
    };
    private readonly Button submitButton = DestructiveButton("Retire daemon");
    private readonly Button refreshButton = new()
    {
        Content = "Refresh same attempt",
        Padding = new Thickness(16, 9),
        IsEnabled = false,
    };
    private readonly TextBlock status = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
        Text = "Choose the daemon authority that performed the original bootstrap.",
    };
    private readonly TextBlock phase = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 1,
        Text = "NOT SUBMITTED",
    };
    private readonly DispatcherTimer polling = new() { Interval = TimeSpan.FromSeconds(2) };
    private RemoteDaemonRetirementIntent? intent;
    private RemoteDaemonRetirementSnapshot? snapshot;
    private bool operationInFlight;
    private int automaticObservations;

    public DaemonRetirementWindow(
        IReadOnlyList<BootstrapAuthorityOption> authorities,
        DaemonRetirementHubOperations operations)
    {
        this.operations = operations;
        Title = "Leserpent / Retire daemon";
        Width = 680;
        MinWidth = 560;
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

        var close = new Button { Content = "Close", Padding = new Thickness(16, 9) };
        close.Click += (_, _) => Close();
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
            lifetime.Cancel();
            lifetime.Dispose();
        };

        Audit(authority, "daemon-retirement-authority",
            "Daemon authority owning the original bootstrap");
        Audit(retirementId, "daemon-retirement-id", "Stable daemon retirement operation ID");
        Audit(bootstrapId, "daemon-retirement-bootstrap-id",
            "Original bootstrap authority ID");
        Audit(credentialHandle, "daemon-retirement-credential-handle",
            "Opaque SSH credential handle");
        Audit(confirmation, "daemon-retirement-confirm", "Confirm leserpent daemon removal");
        Audit(submitButton, "daemon-retirement-submit", "Retire the bootstrapped daemon service");
        Audit(refreshButton, "daemon-retirement-refresh",
            "Refresh the same daemon retirement attempt");
        Audit(close, "daemon-retirement-close", "Close daemon retirement window");
        Audit(status, "daemon-retirement-status", "Daemon retirement status");
        Audit(phase, "daemon-retirement-phase", "Daemon retirement phase");
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
                            new TextBlock
                            {
                                Text = "DAEMON RETIREMENT",
                                Foreground = LeserpentTheme.Destructive,
                                FontSize = 12,
                                FontWeight = FontWeight.Bold,
                                LetterSpacing = 2,
                            },
                            new TextBlock
                            {
                                Text = "Remove a bootstrapped authority",
                                Foreground = LeserpentTheme.Primary,
                                FontSize = 27,
                                FontWeight = FontWeight.Bold,
                            },
                            new TextBlock
                            {
                                Text = "The controller derives host, daemon, generation, and install profile from the bound bootstrap checkpoint. This form cannot override that authority.",
                                Foreground = LeserpentTheme.Muted,
                                TextWrapping = TextWrapping.Wrap,
                            },
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
                                Field("Controlling daemon authority", authority),
                                Field("Retirement ID", retirementId),
                                Field("Original bootstrap ID", bootstrapId),
                                Field("SSH credential handle", credentialHandle),
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
                    new StackPanel
                    {
                        Orientation = Orientation.Horizontal,
                        Spacing = 10,
                        HorizontalAlignment = HorizontalAlignment.Right,
                        Children = { close, refreshButton, submitButton },
                    },
                },
            },
        };
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

    public async Task ProbeWorkflowAsync()
    {
        bootstrapId.Text = "bootstrap-ui-1";
        credentialHandle.Text = "vault:ssh:daemon-example";
        confirmation.IsChecked = true;
        await SubmitAsync();
        await ReconcileAsync(background: false);
        if (snapshot is not { Phase: "service_retired", ServiceRetired: true }
            || intent is null)
        {
            throw new InvalidDataException(
                "daemon retirement controls did not retire the service");
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
                throw new ArgumentException("Confirm daemon retirement before submitting.");
            }
            var source = SelectedAuthority();
            intent = new RemoteDaemonRetirementIntent(
                retirementId.Text ?? string.Empty,
                bootstrapId.Text ?? string.Empty,
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
            status.Text = "Automatic observation reached its bounded limit. Use Refresh same attempt to inspect this exact retirement ID without creating another removal.";
            status.Foreground = LeserpentTheme.Primary;
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
        Func<Task<RemoteDaemonRetirementSnapshot>> operation,
        bool background = false)
    {
        operationInFlight = true;
        UpdateActions();
        if (!background)
        {
            status.Text = "Waiting for the selected daemon authority...";
            status.Foreground = LeserpentTheme.Muted;
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

    private void RenderSnapshot(RemoteDaemonRetirementSnapshot state)
    {
        phase.Text = state.Phase.Replace('_', ' ').ToUpperInvariant();
        phase.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : state.ServiceRetired ? LeserpentTheme.Accent : LeserpentTheme.Primary;
        status.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Body;
        status.Text = state.Phase switch
        {
            "planned" => $"Retirement is durably queued for bootstrap {Safe(state.BootstrapId)}. Derived target authority is locked to daemon {Safe(state.DaemonId)}.",
            "retiring_service" => $"The controller is removing daemon {Safe(state.DaemonId)} on its checkpoint-derived {Safe(state.InstallProfile)} profile.",
            "service_retired" => $"Daemon {Safe(state.DaemonId)} was retired from {Safe(state.Host)}:{state.Port}. Remove any now-offline Hub connection separately.",
            "failed" => $"Daemon retirement failed with bounded fault {Safe(state.FaultCode)}. The service was not marked retired; inspect the controller and use a new retirement ID after remediation.",
            _ => throw new InvalidDataException("unsupported daemon retirement phase"),
        };
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
        ?? throw new ArgumentException("Select the controlling daemon authority first.");

    private void ShowError(Exception error)
    {
        status.Text = Safe(error.Message);
        status.Foreground = LeserpentTheme.Destructive;
    }

    private void Audit(Control control, string id, string name)
    {
        AutomationProperties.SetAutomationId(control, id);
        AutomationProperties.SetName(control, name);
        auditedControls.Add(control);
    }

    private static StackPanel Field(string label, Control control) => new()
    {
        Spacing = 6,
        Children =
        {
            new TextBlock
            {
                Text = label,
                Foreground = LeserpentTheme.Body,
                FontWeight = FontWeight.SemiBold,
                FontSize = 12,
            },
            control,
        },
    };

    private static Button DestructiveButton(string label) => new()
    {
        Content = label,
        Background = LeserpentTheme.Destructive,
        Foreground = Brushes.White,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(17, 9),
    };

    private static bool IsExpected(Exception error) => error is ArgumentException
        or InvalidDataException or IOException or HttpRequestException
        or RemoteDaemonRetirementException or OperationCanceledException;

    private static string Safe(string? value) => value is null
        ? "unavailable"
        : new string(value.Where(character => !char.IsControl(character)).Take(512).ToArray());
}
