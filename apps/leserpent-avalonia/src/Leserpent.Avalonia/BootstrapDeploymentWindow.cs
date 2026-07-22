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
        Content = "I confirm deployment changes on the selected target host",
        Foreground = LeserpentTheme.Body,
    };
    private readonly Button submitButton = PrimaryButton("Deploy leserpentd");
    private readonly Button refreshButton = new()
    {
        Content = "Refresh status",
        Padding = new Thickness(16, 9),
        IsEnabled = false,
    };
    private readonly Button bindButton = new()
    {
        Content = "Verify & bind session",
        Padding = new Thickness(16, 9),
        IsEnabled = false,
    };
    private readonly Button promoteButton = PrimaryButton("Add to Hub");
    private readonly TextBlock status = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
        Text = "Choose an existing daemon authority to perform the deployment.",
    };
    private readonly TextBlock phase = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 1,
        Text = "NOT SUBMITTED",
    };
    private readonly DispatcherTimer polling = new() { Interval = TimeSpan.FromSeconds(2) };
    private RemoteBootstrapSnapshot? snapshot;
    private bool operationInFlight;
    private bool promotionCompleted;

    public BootstrapDeploymentWindow(
        IReadOnlyList<BootstrapAuthorityOption> authorities,
        BootstrapHubOperations operations)
    {
        this.operations = operations;
        Title = "Leserpent / Deploy daemon";
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

        var close = new Button { Content = "Close", Padding = new Thickness(16, 9) };
        close.Click += (_, _) => Close();
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
        Closed += (_, _) =>
        {
            polling.Stop();
            lifetime.Cancel();
            lifetime.Dispose();
        };

        Audit(authority, "bootstrap-authority", "Daemon authority performing bootstrap deployment");
        Audit(bootstrapId, "bootstrap-id", "Stable bootstrap operation ID");
        Audit(host, "bootstrap-host", "Target host for leserpent daemon deployment");
        Audit(port, "bootstrap-port", "Target SSH port");
        Audit(credentialHandle, "bootstrap-credential-handle", "Opaque SSH credential handle");
        Audit(confirmation, "bootstrap-confirm", "Confirm target host deployment");
        Audit(submitButton, "bootstrap-submit", "Deploy leserpent daemon to target host");
        Audit(refreshButton, "bootstrap-refresh", "Refresh bootstrap deployment status");
        Audit(bindButton, "bootstrap-bind", "Verify and bind deployed daemon session");
        Audit(promoteButton, "bootstrap-promote", "Add authenticated daemon connection to Hub");
        Audit(close, "bootstrap-close", "Close daemon deployment window");
        Audit(status, "bootstrap-status", "Bootstrap deployment status");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Assertive);

        var heading = new StackPanel
        {
            Spacing = 5,
            Children =
            {
                new TextBlock
                {
                    Text = "REVERSE DEPLOYMENT",
                    Foreground = LeserpentTheme.Accent,
                    FontSize = 12,
                    FontWeight = FontWeight.Bold,
                    LetterSpacing = 2,
                },
                new TextBlock
                {
                    Text = "Deploy a daemon authority",
                    Foreground = LeserpentTheme.Primary,
                    FontSize = 27,
                    FontWeight = FontWeight.Bold,
                },
                new TextBlock
                {
                    Text = "An authenticated leserpentd authority performs native SSH deployment. The desktop sends only an opaque credential handle, never a password or private key.",
                    Foreground = LeserpentTheme.Muted,
                    TextWrapping = TextWrapping.Wrap,
                },
            },
        };

        var target = new Grid
        {
            ColumnDefinitions = ColumnDefinitions.Parse("*,130"),
            ColumnSpacing = 12,
            Children = { Field("Target host", host), Field("SSH port", port) },
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
                    Field("Deployment authority", authority),
                    Field("Bootstrap ID", bootstrapId),
                    target,
                    Field("SSH credential handle", credentialHandle),
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
                    new StackPanel
                    {
                        Orientation = Orientation.Horizontal,
                        Spacing = 10,
                        HorizontalAlignment = HorizontalAlignment.Right,
                        Children =
                        {
                            close,
                            refreshButton,
                            bindButton,
                            promoteButton,
                            submitButton,
                        },
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
            throw new InvalidDataException("bootstrap deployment control contract drifted");
        }
    }

    public async Task ProbeWorkflowAsync()
    {
        host.Text = "target.example";
        credentialHandle.Text = "vault:ssh:target-example";
        confirmation.IsChecked = true;
        await SubmitAsync();
        await RefreshAsync();
        await BindAsync();
        await PromoteAsync();
        if (snapshot is not { Phase: "session_bound", MutationAuthorized: true }
            || !promotionCompleted)
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

    private async Task SubmitAsync()
    {
        if (operationInFlight)
        {
            return;
        }
        try
        {
            if (confirmation.IsChecked != true)
            {
                throw new ArgumentException("Confirm target deployment before submitting.");
            }
            var source = SelectedAuthority();
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
            LockIdentityFields();
            if (snapshot is { IsTerminal: false, CanBind: false })
            {
                polling.Start();
            }
        }
        catch (Exception error) when (IsExpected(error))
        {
            ShowError(error);
        }
    }

    private async Task RefreshAsync(bool background = false)
    {
        if (operationInFlight || snapshot is null)
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
            if (!background)
            {
                ShowError(error);
            }
        }
    }

    private async Task BindAsync()
    {
        if (operationInFlight || snapshot is not { CanBind: true })
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
            ShowError(error);
        }
    }

    private async Task PromoteAsync()
    {
        var source = SelectedAuthority();
        if (operationInFlight
            || promotionCompleted
            || !source.CanPromote
            || snapshot is not { Phase: "session_bound", MutationAuthorized: true } state)
        {
            return;
        }
        operationInFlight = true;
        UpdateActions();
        status.Text = "Verifying target trust and session credential before saving...";
        status.Foreground = LeserpentTheme.Muted;
        try
        {
            await operations.Promote(source.AuthorityId, state, lifetime.Token);
            promotionCompleted = true;
            status.Text = $"Daemon {Safe(state.DaemonId)} was verified and added to the Hub.";
            status.Foreground = LeserpentTheme.Accent;
        }
        catch (Exception error) when (IsExpected(error))
        {
            ShowError(error);
        }
        finally
        {
            operationInFlight = false;
            UpdateActions();
        }
    }

    private async Task RunAsync(
        Func<Task<RemoteBootstrapSnapshot>> operation,
        bool background = false)
    {
        operationInFlight = true;
        UpdateActions();
        if (!background)
        {
            status.Text = "Waiting for the selected authority...";
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

    private void RenderSnapshot(RemoteBootstrapSnapshot state)
    {
        phase.Text = state.Phase.Replace('_', ' ').ToUpperInvariant();
        phase.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : state.MutationAuthorized ? LeserpentTheme.Accent : LeserpentTheme.Primary;
        status.Foreground = state.Phase == "failed"
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Body;
        status.Text = state.Phase switch
        {
            "planned" => "Deployment is durably queued. Status refresh will continue without resubmitting the effect.",
            "deploying" => "The authority is reconciling the target host.",
            "bootstrapped" => $"Daemon {Safe(state.DaemonId)} is reachable at {Safe(state.Endpoint)}. Verify and bind its session authority before mutations.",
            "session_bound" => $"Daemon {Safe(state.DaemonId)} is authenticated and mutation authority is enabled.",
            "failed" => $"Deployment failed with bounded fault {Safe(state.FaultCode)}.",
            _ => throw new InvalidDataException("unsupported bootstrap phase"),
        };
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
        ?? throw new ArgumentException("Select a deployment authority first.");

    private BootstrapAuthorityOption? SelectedAuthorityOrNull() =>
        authority.SelectedItem as BootstrapAuthorityOption;

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
        or RemoteBootstrapException or OperationCanceledException;

    private static string Safe(string? value) => value is null
        ? "unavailable"
        : new string(value.Where(character => !char.IsControl(character)).Take(512).ToArray());
}
