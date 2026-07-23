using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed record ProvisioningHubOperations(
    Func<string, RemoteProvisioningIntent, CancellationToken, Task<RemoteProvisioningSnapshot>>
        Reconcile);

internal sealed class GewyvernProvisioningWindow : Window
{
    private const string Principal = "avalonia-hub";
    private const int MaxAutomaticObservations = 30;
    private readonly ProvisioningHubOperations operations;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<Control> auditedControls = [];
    private readonly ComboBox authority = new();
    private readonly TextBox provisioningId = new() { MaxLength = 128 };
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
        Content = "I confirm gewyvern installation and runtime registration on this host",
        Foreground = LeserpentTheme.Body,
    };
    private readonly Button submitButton = PrimaryButton("Provision gewyvern");
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
        Text = "Choose the daemon authority that will own this gewyvern runtime.",
    };
    private readonly TextBlock phase = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 1,
        Text = "NOT SUBMITTED",
    };
    private readonly DispatcherTimer polling = new() { Interval = TimeSpan.FromSeconds(2) };
    private RemoteProvisioningIntent? intent;
    private RemoteProvisioningSnapshot? snapshot;
    private bool operationInFlight;
    private int automaticObservations;

    public GewyvernProvisioningWindow(
        IReadOnlyList<BootstrapAuthorityOption> authorities,
        ProvisioningHubOperations operations)
    {
        this.operations = operations;
        Title = "Leserpent / Provision gewyvern";
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
        provisioningId.Text = $"provision-{DateTimeOffset.UtcNow:yyyyMMddHHmmss}-{Guid.NewGuid():N}"[..50];

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

        Audit(authority, "provisioning-authority", "Daemon authority owning the gewyvern runtime");
        Audit(provisioningId, "provisioning-id", "Stable gewyvern provisioning operation ID");
        Audit(runtimeId, "provisioning-runtime-id", "Runtime ID registered by provisioning");
        Audit(host, "provisioning-host", "Target host for gewyvern installation");
        Audit(port, "provisioning-port", "Target SSH port");
        Audit(credentialHandle, "provisioning-credential-handle", "Opaque SSH credential handle");
        Audit(confirmation, "provisioning-confirm", "Confirm gewyvern installation and registration");
        Audit(submitButton, "provisioning-submit", "Provision and register gewyvern runtime");
        Audit(refreshButton, "provisioning-refresh", "Refresh the same provisioning attempt");
        Audit(close, "provisioning-close", "Close gewyvern provisioning window");
        Audit(status, "provisioning-status", "Gewyvern provisioning status");
        Audit(phase, "provisioning-phase", "Gewyvern provisioning phase");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);

        var target = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,130"),
            ColumnSpacing = 12,
            Children = { Field("Target host", host), Field("SSH port", port) },
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
                            new TextBlock
                            {
                                Text = "RUNTIME PROVISIONING",
                                Foreground = LeserpentTheme.Accent,
                                FontSize = 12,
                                FontWeight = FontWeight.Bold,
                                LetterSpacing = 2,
                            },
                            new TextBlock
                            {
                                Text = "Install and register gewyvern",
                                Foreground = LeserpentTheme.Primary,
                                FontSize = 27,
                                FontWeight = FontWeight.Bold,
                            },
                            new TextBlock
                            {
                                Text = "The selected leserpentd performs native SSH installation, proves service identity, and atomically registers the runtime. Only an opaque vault handle leaves this desktop.",
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
                                Field("Owning daemon authority", authority),
                                Field("Provisioning ID", provisioningId),
                                Field("Runtime ID", runtimeId),
                                target,
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
        if (auditedControls.Count != 12
            || auditedControls.Select(AutomationProperties.GetAutomationId)
                .Distinct(StringComparer.Ordinal).Count() != auditedControls.Count
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))))
        {
            throw new InvalidDataException("gewyvern provisioning control contract drifted");
        }
    }

    public async Task ProbeWorkflowAsync()
    {
        runtimeId.Text = "runtime-ui-1";
        host.Text = "runtime.example";
        credentialHandle.Text = "vault:ssh:runtime-example";
        confirmation.IsChecked = true;
        await SubmitAsync();
        await ReconcileAsync(background: false);
        if (snapshot is not { Phase: "runtime_registered", RuntimeRegistered: true }
            || intent is null)
        {
            throw new InvalidDataException("provisioning controls did not register the runtime");
        }
    }

    public async Task ProbeConfirmationFenceAsync()
    {
        runtimeId.Text = "runtime-ui-1";
        host.Text = "runtime.example";
        credentialHandle.Text = "vault:ssh:runtime-example";
        confirmation.IsChecked = false;
        await SubmitAsync();
        if (snapshot is not null || intent is not null || !submitButton.IsEnabled)
        {
            throw new InvalidDataException("provisioning controls crossed the confirmation fence");
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
                throw new ArgumentException("Confirm gewyvern installation before submitting.");
            }
            var source = SelectedAuthority();
            intent = new RemoteProvisioningIntent(
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
            status.Text = "Automatic observation reached its bounded limit. Use Refresh same attempt to inspect this exact provisioning ID without creating another installation.";
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
        Func<Task<RemoteProvisioningSnapshot>> operation,
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

    private void RenderSnapshot(RemoteProvisioningSnapshot state)
    {
        phase.Text = state.Phase.Replace('_', ' ').ToUpperInvariant();
        phase.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : state.RuntimeRegistered ? LeserpentTheme.Accent : LeserpentTheme.Primary;
        status.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Body;
        status.Text = state.Phase switch
        {
            "planned" => "Provisioning is durably queued. Observation reuses this exact identity and does not submit a second installation.",
            "installing" => "The daemon authority is installing and activating gewyvern on the target host.",
            "service_ready" => $"Gewyvern is verified at {Safe(state.Endpoint)}; atomic runtime registration is pending.",
            "runtime_registered" => $"Runtime {Safe(state.RuntimeId)} is registered and ready at {Safe(state.Endpoint)}.",
            "failed" => $"Provisioning failed with bounded fault {Safe(state.FaultCode)}. Correct the cause, then start a new attempt with a new provisioning ID; this failed identity remains immutable for audit.",
            _ => throw new InvalidDataException("unsupported provisioning phase"),
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
        provisioningId.IsReadOnly = true;
        runtimeId.IsReadOnly = true;
        host.IsReadOnly = true;
        port.IsReadOnly = true;
        credentialHandle.IsReadOnly = true;
        confirmation.IsEnabled = false;
    }

    private BootstrapAuthorityOption SelectedAuthority() =>
        authority.SelectedItem as BootstrapAuthorityOption
        ?? throw new ArgumentException("Select an owning daemon authority first.");

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

    private static Button PrimaryButton(string label) => new()
    {
        Content = label,
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(17, 9),
    };

    private static bool IsExpected(Exception error) => error is ArgumentException
        or InvalidDataException or IOException or HttpRequestException
        or RemoteProvisioningException or OperationCanceledException;

    private static string Safe(string? value) => value is null
        ? "unavailable"
        : new string(value.Where(character => !char.IsControl(character)).Take(512).ToArray());
}
