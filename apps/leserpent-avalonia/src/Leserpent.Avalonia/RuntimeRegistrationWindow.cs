using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Layout;
using Avalonia.Media;
using Avalonia.Threading;

internal sealed record RuntimeRegistrationWindowOperations(
    Func<string, string, CancellationToken, Task<RemoteRegistrationDetails>> Inspect,
    Func<RemoteRegistrationIntent, string, CancellationToken, Task<RemoteRegistrationPlan>>
        PlanRegister,
    Func<RemoteRegistrationIntent, ulong, string, CancellationToken,
        Task<RemoteRegistrationPlan>> PlanUpdate,
    Func<RemoteRegistrationPlan, string, CancellationToken,
        Task<RemoteRegistrationResult>> Apply);

internal sealed class RuntimeRegistrationWindow : Window
{
    private readonly RuntimeRegistrationWindowOperations operations;
    private readonly RemoteRegistrationClient? ownedClient;
    private readonly DesktopLocalization localization;
    private readonly CancellationTokenSource lifetime = new();
    private readonly List<Control> auditedControls = [];
    private readonly string principal;
    private readonly RemoteRegistrationMode mode;
    private readonly string? runtimeIdToEdit;
    private readonly bool closeAfterApply;
    private readonly TextBox runtimeId = new()
    {
        MaxLength = 128,
        PlaceholderText = "runtime-production-a",
    };
    private readonly TextBox runtimeName = new()
    {
        MaxLength = 128,
        PlaceholderText = "Production capture runtime",
    };
    private readonly TextBox endpoint = new()
    {
        MaxLength = 2048,
        PlaceholderText = "https://runtime.example:9443",
    };
    private readonly TextBox sidecarEndpoint = new()
    {
        MaxLength = 2048,
        PlaceholderText = "https://runtime.example:9444",
    };
    private readonly TextBox environment = new()
    {
        MaxLength = 128,
        PlaceholderText = "production",
    };
    private readonly TextBox cluster = new()
    {
        MaxLength = 128,
        PlaceholderText = "edge-a",
    };
    private readonly TextBox role = new()
    {
        MaxLength = 128,
        PlaceholderText = "capture",
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
    private readonly TextBlock runtimeIdLabel = CreateLabel();
    private readonly TextBlock runtimeNameLabel = CreateLabel();
    private readonly TextBlock endpointLabel = CreateLabel();
    private readonly TextBlock sidecarEndpointLabel = CreateLabel();
    private readonly TextBlock environmentLabel = CreateLabel();
    private readonly TextBlock clusterLabel = CreateLabel();
    private readonly TextBlock roleLabel = CreateLabel();
    private readonly TextBlock status = new()
    {
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock planHeading = new()
    {
        Foreground = LeserpentTheme.Accent,
        FontSize = 12,
        FontWeight = FontWeight.Bold,
        LetterSpacing = 1.5,
    };
    private readonly TextBlock planSummary = new()
    {
        Foreground = LeserpentTheme.Body,
        FontFamily = new FontFamily("JetBrains Mono, Menlo, monospace"),
        FontSize = 12,
        LineHeight = 19,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Border planPanel = new()
    {
        Background = Brush.Parse("#17140E"),
        BorderBrush = LeserpentTheme.Accent,
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(10),
        Padding = new Thickness(18, 14),
        IsVisible = false,
    };
    private readonly CheckBox confirmation = new()
    {
        Foreground = LeserpentTheme.Body,
        IsEnabled = false,
    };
    private readonly Button reviewButton = PrimaryButton();
    private readonly Button applyButton = PrimaryButton();
    private readonly Button editButton = new()
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
    private RemoteRegistrationDetails? details;
    private RemoteRegistrationPlan? plan;
    private RemoteRegistrationResult? completed;
    private bool operationInFlight;
    private bool mutationEnabled = true;
    private bool suppressFieldChanges;
    private string localizedStatusKey;
    private object[] localizedStatusValues = [];

    public RuntimeRegistrationWindow(
        RemoteClientOptions options,
        string principal,
        DesktopLocalization localization,
        string? runtimeIdToEdit = null)
        : this(
            new RemoteRegistrationClient(options),
            principal,
            localization,
            runtimeIdToEdit)
    {
    }

    private RuntimeRegistrationWindow(
        RemoteRegistrationClient client,
        string principal,
        DesktopLocalization localization,
        string? runtimeIdToEdit)
        : this(
            new RuntimeRegistrationWindowOperations(
                client.InspectAsync,
                client.PlanRegisterAsync,
                client.PlanUpdateAsync,
                client.ApplyAsync),
            principal,
            localization,
            runtimeIdToEdit)
    {
        ownedClient = client;
    }

    internal RuntimeRegistrationWindow(
        RuntimeRegistrationWindowOperations operations,
        string principal,
        DesktopLocalization localization,
        string? runtimeIdToEdit = null,
        bool closeAfterApply = true)
    {
        this.operations = operations;
        this.principal = principal;
        this.localization = localization;
        this.runtimeIdToEdit = runtimeIdToEdit;
        this.closeAfterApply = closeAfterApply;
        mode = runtimeIdToEdit is null
            ? RemoteRegistrationMode.Register
            : RemoteRegistrationMode.Update;
        localizedStatusKey = mode == RemoteRegistrationMode.Register
            ? "status.initial.register"
            : "status.loading";

        Width = 680;
        Height = 760;
        MinWidth = 420;
        MinHeight = 520;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        ConfigureField(runtimeId, "runtime-registration-runtime-id");
        ConfigureField(runtimeName, "runtime-registration-name");
        ConfigureField(endpoint, "runtime-registration-endpoint");
        ConfigureField(sidecarEndpoint, "runtime-registration-sidecar-endpoint");
        ConfigureField(environment, "runtime-registration-environment");
        ConfigureField(cluster, "runtime-registration-cluster");
        ConfigureField(role, "runtime-registration-role");
        Audit(status, "runtime-registration-status");
        Audit(planSummary, "runtime-registration-plan-summary");
        Audit(confirmation, "runtime-registration-confirm");
        Audit(reviewButton, "runtime-registration-review");
        Audit(applyButton, "runtime-registration-apply");
        Audit(editButton, "runtime-registration-edit");
        Audit(closeButton, "runtime-registration-close");
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);

        planPanel.Child = new StackPanel
        {
            Spacing = 8,
            Children = { planHeading, planSummary, confirmation },
        };
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto"),
            Children =
            {
                new ScrollViewer
                {
                    VerticalScrollBarVisibility =
                        Avalonia.Controls.Primitives.ScrollBarVisibility.Auto,
                    Content = new StackPanel
                    {
                        Margin = new Thickness(30, 26, 30, 20),
                        Spacing = 18,
                        Children =
                        {
                            new StackPanel
                            {
                                Spacing = 5,
                                Children = { kickerText, headingText, descriptionText },
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
                                    Spacing = 13,
                                    Children =
                                    {
                                        Field(runtimeIdLabel, runtimeId),
                                        Field(runtimeNameLabel, runtimeName),
                                        Field(endpointLabel, endpoint),
                                        Field(sidecarEndpointLabel, sidecarEndpoint),
                                        Field(environmentLabel, environment),
                                        Field(clusterLabel, cluster),
                                        Field(roleLabel, role),
                                    },
                                },
                            },
                            new Border
                            {
                                Background = LeserpentTheme.Panel,
                                BorderBrush = LeserpentTheme.PanelBorder,
                                BorderThickness = new Thickness(1),
                                CornerRadius = new CornerRadius(10),
                                Padding = new Thickness(18, 14),
                                Child = status,
                            },
                            planPanel,
                        },
                    },
                },
                BuildActions(),
            },
        };
        Grid.SetRow(((Grid)Content).Children[1], 1);

        reviewButton.Click += async (_, _) => await ReviewAsync();
        applyButton.Click += async (_, _) => await ApplyAsync();
        editButton.Click += (_, _) => EditReviewedPlan();
        closeButton.Click += (_, _) => Close(completed);
        confirmation.IsCheckedChanged += (_, _) => UpdateActions();
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape && !operationInFlight)
            {
                eventArgs.Handled = true;
                Close(completed);
            }
        };
        Opened += async (_, _) =>
        {
            if (mode == RemoteRegistrationMode.Update)
            {
                await LoadExistingAsync();
            }
            else
            {
                runtimeId.Focus();
            }
        };
        Closed += (_, _) =>
        {
            localization.Changed -= OnLocalizationChanged;
            lifetime.Cancel();
            ownedClient?.Dispose();
            lifetime.Dispose();
        };
        localization.Changed += OnLocalizationChanged;
        ApplyLocalization();
        UpdateActions();
    }

    public void VerifyAccessibility()
    {
        if (auditedControls.Count != 14
            || auditedControls.Select(AutomationProperties.GetAutomationId)
                .Distinct(StringComparer.Ordinal).Count() != auditedControls.Count
            || auditedControls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control)))
            || AutomationProperties.GetLiveSetting(status)
                is not (AutomationLiveSetting.Polite or AutomationLiveSetting.Assertive))
        {
            throw new InvalidDataException(
                "runtime registration accessibility contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException(
                "runtime registration window has no control root");
        }
        foreach (var (width, height) in new[]
        {
            (MinWidth, MinHeight),
            (Width, Height),
        })
        {
            root.Measure(new Size(width, height));
            var desired = root.DesiredSize;
            if (!double.IsFinite(desired.Width)
                || !double.IsFinite(desired.Height)
                || desired.Width <= 0
                || desired.Height <= 0
                || desired.Width > width
                || desired.Height > height)
            {
                throw new InvalidDataException(
                    "runtime registration controls exceeded their layout envelope");
            }
        }
    }

    public void ProbeLocalizedPresentation()
    {
        if (Title != Text(mode == RemoteRegistrationMode.Register
                ? "title.register"
                : "title.update")
            || headingText.Text != Text(mode == RemoteRegistrationMode.Register
                ? "heading.register"
                : "heading.update")
            || reviewButton.Content as string != Text("action.review")
            || applyButton.Content as string != Text(mode == RemoteRegistrationMode.Register
                ? "action.apply.register"
                : "action.apply.update")
            || runtimeIdLabel.Text != Text("runtime_id.label")
            || AutomationProperties.GetName(reviewButton) != Text("action.review")
            || FlowDirection != localization.FlowDirection)
        {
            throw new InvalidDataException(
                "runtime registration localized presentation drifted");
        }
    }

    internal async Task ProbeRegisterWorkflowAsync(string reprojectLocale)
    {
        if (mode != RemoteRegistrationMode.Register || closeAfterApply)
        {
            throw new InvalidOperationException(
                "registration workflow probe requires a retained register window");
        }
        runtimeId.Text = "runtime-registration-probe";
        runtimeName.Text = "Registration probe";
        endpoint.Text = "https://registration.invalid:9443";
        sidecarEndpoint.Text = "https://registration.invalid:9444";
        environment.Text = "verification";
        cluster.Text = "probe-a";
        role.Text = "capture";
        await ReviewAsync();
        if (plan is null
            || plan.Mode != RemoteRegistrationMode.Register
            || plan.ExpectedRevision is not null
            || !planPanel.IsVisible
            || !runtimeName.IsReadOnly
            || confirmation.IsEnabled is not true
            || applyButton.IsEnabled)
        {
            throw new InvalidDataException(
                "runtime registration review did not establish its confirmation fence");
        }
        confirmation.IsChecked = true;
        await ApplyAsync();
        localization.SetPreference(reprojectLocale);
        await Dispatcher.UIThread.InvokeAsync(
            () => { },
            DispatcherPriority.Background);
        if (completed is not
            {
                Mode: RemoteRegistrationMode.Register,
                RuntimeId: "runtime-registration-probe",
            }
            || status.Text != Format(
                "status.applied.register",
                "runtime-registration-probe",
                completed.Revision)
            || runtimeName.Text != "Registration probe")
        {
            throw new InvalidDataException(
                "runtime registration workflow did not preserve reviewed intent");
        }
    }

    internal async Task InitializeUpdateForVerificationAsync() =>
        await LoadExistingAsync();

    internal void SetMutationAvailability(bool enabled)
    {
        mutationEnabled = enabled;
        UpdateActions();
    }

    internal void ProbeMutationAvailabilityFence()
    {
        if (mode != RemoteRegistrationMode.Register
            || plan is not null
            || completed is not null)
        {
            throw new InvalidOperationException(
                "registration mutation probe requires an idle register window");
        }
        SetMutationAvailability(false);
        if (reviewButton.IsEnabled
            || editButton.IsEnabled
            || confirmation.IsEnabled
            || applyButton.IsEnabled)
        {
            throw new InvalidDataException(
                "runtime registration controls bypassed the shared mutation fence");
        }
        SetMutationAvailability(true);
        if (!reviewButton.IsEnabled)
        {
            throw new InvalidDataException(
                "runtime registration controls did not recover with the mutation fence");
        }
    }

    private Border BuildActions()
    {
        var actions = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(20, 10),
            Child = new WrapPanel
            {
                Orientation = Orientation.Horizontal,
                HorizontalAlignment = HorizontalAlignment.Right,
                Children = { closeButton, editButton, reviewButton, applyButton },
            },
        };
        return actions;
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs) =>
        Dispatcher.UIThread.Post(() =>
        {
            _ = sender;
            _ = eventArgs;
            if (!lifetime.IsCancellationRequested)
            {
                ApplyLocalization();
            }
        });

    private void ApplyLocalization()
    {
        FlowDirection = localization.FlowDirection;
        Title = Text(mode == RemoteRegistrationMode.Register
            ? "title.register"
            : "title.update");
        kickerText.Text = Text("kicker");
        headingText.Text = Text(mode == RemoteRegistrationMode.Register
            ? "heading.register"
            : "heading.update");
        descriptionText.Text = Text(mode == RemoteRegistrationMode.Register
            ? "body.register"
            : "body.update");
        runtimeIdLabel.Text = Text("runtime_id.label");
        runtimeNameLabel.Text = Text("name.label");
        endpointLabel.Text = Text("endpoint.label");
        sidecarEndpointLabel.Text = Text("sidecar.label");
        environmentLabel.Text = Text("environment.label");
        clusterLabel.Text = Text("cluster.label");
        roleLabel.Text = Text("role.label");
        planHeading.Text = Text("plan.heading");
        confirmation.Content = Text("confirmation");
        reviewButton.Content = Text("action.review");
        applyButton.Content = Text(mode == RemoteRegistrationMode.Register
            ? "action.apply.register"
            : "action.apply.update");
        editButton.Content = Text("action.edit");
        closeButton.Content = Text("action.close");

        SetName(runtimeId, runtimeIdLabel.Text);
        SetName(runtimeName, runtimeNameLabel.Text);
        SetName(endpoint, endpointLabel.Text);
        SetName(sidecarEndpoint, sidecarEndpointLabel.Text);
        SetName(environment, environmentLabel.Text);
        SetName(cluster, clusterLabel.Text);
        SetName(role, roleLabel.Text);
        SetName(status, Text("status.name"));
        SetName(planSummary, Text("plan.summary.name"));
        SetName(confirmation, Text("confirmation"));
        SetName(reviewButton, Text("action.review"));
        SetName(applyButton, Text(mode == RemoteRegistrationMode.Register
            ? "action.apply.register"
            : "action.apply.update"));
        SetName(editButton, Text("action.edit"));
        SetName(closeButton, Text("action.close"));
        RefreshStatus();
        RenderPlan();
    }

    private async Task LoadExistingAsync()
    {
        if (mode != RemoteRegistrationMode.Update
            || operationInFlight
            || details is not null
            || runtimeIdToEdit is null
            || lifetime.IsCancellationRequested)
        {
            return;
        }
        operationInFlight = true;
        ShowStatus("status.loading", LeserpentTheme.Primary);
        UpdateActions();
        try
        {
            var loaded = await operations.Inspect(
                runtimeIdToEdit,
                principal,
                lifetime.Token);
            if (loaded.Intent.RuntimeId != runtimeIdToEdit || loaded.Revision == 0)
            {
                throw new InvalidDataException(
                    "runtime registration inspection changed the requested identity");
            }
            details = loaded;
            Populate(loaded.Intent);
            ShowStatus(
                "status.initial.update",
                LeserpentTheme.Muted,
                loaded.Revision);
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

    private async Task ReviewAsync()
    {
        if (operationInFlight
            || !mutationEnabled
            || completed is not null
            || plan is not null
            || mode == RemoteRegistrationMode.Update && details is null)
        {
            return;
        }
        operationInFlight = true;
        ShowStatus("status.reviewing", LeserpentTheme.Primary);
        UpdateActions();
        try
        {
            var intent = ReadIntent();
            plan = mode == RemoteRegistrationMode.Register
                ? await operations.PlanRegister(intent, principal, lifetime.Token)
                : await operations.PlanUpdate(
                    intent,
                    details!.Revision,
                    principal,
                    lifetime.Token);
            if (plan.Mode != mode
                || plan.Intent != intent
                || mode == RemoteRegistrationMode.Update
                    && plan.ExpectedRevision != details!.Revision)
            {
                throw new InvalidDataException(
                    "runtime registration plan changed the submitted intent");
            }
            Populate(intent);
            SetFieldsReadOnly(true);
            planPanel.IsVisible = true;
            confirmation.IsChecked = false;
            ShowStatus(
                mode == RemoteRegistrationMode.Register
                    ? "status.plan_ready.register"
                    : "status.plan_ready.update",
                LeserpentTheme.Accent,
                plan.PlannedRevision);
            RenderPlan();
        }
        catch (Exception error) when (IsExpected(error))
        {
            plan = null;
            planPanel.IsVisible = false;
            SetFieldsReadOnly(false);
            ShowError(error);
        }
        finally
        {
            operationInFlight = false;
            UpdateActions();
        }
    }

    private async Task ApplyAsync()
    {
        if (operationInFlight
            || !mutationEnabled
            || completed is not null
            || plan is null)
        {
            return;
        }
        if (confirmation.IsChecked != true)
        {
            ShowStatus("error.confirm", LeserpentTheme.Destructive, assertive: true);
            return;
        }
        operationInFlight = true;
        ShowStatus("status.applying", LeserpentTheme.Primary);
        UpdateActions();
        try
        {
            completed = await operations.Apply(plan, principal, lifetime.Token);
            if (completed.CommandId != plan.CommandId
                || completed.Mode != plan.Mode
                || completed.RuntimeId != plan.Intent.RuntimeId
                || completed.Revision < plan.PlannedRevision)
            {
                throw new InvalidDataException(
                    "runtime registration apply result changed its reviewed identity");
            }
            ShowStatus(
                mode == RemoteRegistrationMode.Register
                    ? "status.applied.register"
                    : "status.applied.update",
                LeserpentTheme.Accent,
                completed.RuntimeId,
                completed.Revision);
            if (closeAfterApply)
            {
                Close(completed);
            }
        }
        catch (Exception error) when (IsExpected(error))
        {
            completed = null;
            ShowError(error);
        }
        finally
        {
            operationInFlight = false;
            UpdateActions();
        }
    }

    private void EditReviewedPlan()
    {
        if (operationInFlight || completed is not null || plan is null)
        {
            return;
        }
        plan = null;
        confirmation.IsChecked = false;
        planPanel.IsVisible = false;
        SetFieldsReadOnly(false);
        ShowStatus(
            mode == RemoteRegistrationMode.Register
                ? "status.initial.register"
                : "status.initial.update",
            LeserpentTheme.Muted,
            details?.Revision ?? 0);
        UpdateActions();
        runtimeName.Focus();
    }

    private void ConfigureField(TextBox field, string automationId)
    {
        Audit(field, automationId);
        field.TextChanged += (_, _) =>
        {
            if (!suppressFieldChanges && plan is not null)
            {
                EditReviewedPlan();
            }
        };
    }

    private void Populate(RemoteRegistrationIntent intent)
    {
        suppressFieldChanges = true;
        try
        {
            runtimeId.Text = intent.RuntimeId;
            runtimeName.Text = intent.Name;
            endpoint.Text = intent.Endpoint;
            sidecarEndpoint.Text = intent.SidecarEndpoint ?? string.Empty;
            environment.Text = intent.Environment ?? string.Empty;
            cluster.Text = intent.Cluster ?? string.Empty;
            role.Text = intent.Role ?? string.Empty;
        }
        finally
        {
            suppressFieldChanges = false;
        }
    }

    private RemoteRegistrationIntent ReadIntent() => new(
        Required(runtimeId.Text, "runtime ID"),
        Required(runtimeName.Text, "runtime name"),
        Required(endpoint.Text, "runtime endpoint"),
        Optional(sidecarEndpoint.Text),
        Optional(environment.Text),
        Optional(cluster.Text),
        Optional(role.Text));

    private void RenderPlan()
    {
        if (plan is null)
        {
            planSummary.Text = Text("plan.empty");
            return;
        }
        var intent = plan.Intent;
        var none = Text("optional.none");
        var lines = new[]
        {
            Format("plan.kind", Text(plan.Mode == RemoteRegistrationMode.Register
                ? "plan.kind.register"
                : "plan.kind.update")),
            Format("plan.identity", intent.RuntimeId, intent.Name),
            Format("plan.endpoint", intent.Endpoint),
            Format("plan.sidecar", intent.SidecarEndpoint ?? none),
            Format(
                "plan.tags",
                intent.Environment ?? none,
                intent.Cluster ?? none,
                intent.Role ?? none),
            plan.Mode == RemoteRegistrationMode.Register
                ? Format("plan.revision.register", plan.PlannedRevision)
                : Format(
                    "plan.revision.update",
                    plan.ExpectedRevision ?? 0,
                    plan.PlannedRevision),
        };
        planSummary.Text = string.Join(Environment.NewLine, lines);
    }

    private void SetFieldsReadOnly(bool reviewed)
    {
        runtimeId.IsReadOnly = reviewed || mode == RemoteRegistrationMode.Update;
        runtimeName.IsReadOnly = reviewed;
        endpoint.IsReadOnly = reviewed;
        sidecarEndpoint.IsReadOnly = reviewed;
        environment.IsReadOnly = reviewed;
        cluster.IsReadOnly = reviewed;
        role.IsReadOnly = reviewed;
    }

    private void UpdateActions()
    {
        var updateReady = mode == RemoteRegistrationMode.Register || details is not null;
        reviewButton.IsEnabled = !operationInFlight
            && mutationEnabled
            && completed is null
            && plan is null
            && updateReady;
        editButton.IsEnabled = !operationInFlight
            && mutationEnabled
            && completed is null
            && plan is not null;
        confirmation.IsEnabled = !operationInFlight
            && mutationEnabled
            && completed is null
            && plan is not null;
        applyButton.IsEnabled = !operationInFlight
            && mutationEnabled
            && completed is null
            && plan is not null
            && confirmation.IsChecked == true;
        closeButton.IsEnabled = !operationInFlight;
    }

    private void ShowError(Exception error)
    {
        var detail = error switch
        {
            RemoteRegistrationException remote => $"{remote.Code}: {remote.Message}",
            RemoteQueryException remote => $"{remote.Code}: {remote.Message}",
            _ => error.Message,
        };
        ShowStatus("status.failed", LeserpentTheme.Destructive, assertive: true, Safe(detail));
    }

    private void ShowStatus(
        string key,
        IBrush foreground,
        bool assertive = false,
        params object[] values)
    {
        localizedStatusKey = key;
        localizedStatusValues = [.. values];
        status.Foreground = foreground;
        AutomationProperties.SetLiveSetting(
            status,
            assertive ? AutomationLiveSetting.Assertive : AutomationLiveSetting.Polite);
        RefreshStatus();
    }

    private void ShowStatus(string key, IBrush foreground, params object[] values) =>
        ShowStatus(key, foreground, assertive: false, values);

    private void RefreshStatus()
    {
        status.Text = localizedStatusValues.Length == 0
            ? Text(localizedStatusKey)
            : Format(localizedStatusKey, localizedStatusValues);
        AutomationProperties.SetName(status, status.Text);
    }

    private string Text(string key) =>
        DesktopRegistrationCatalogs.Resolve(localization, key);

    private string Format(string key, params object[] values) =>
        DesktopRegistrationCatalogs.Format(localization, key, values);

    private void Audit(Control control, string automationId)
    {
        auditedControls.Add(control);
        AutomationProperties.SetAutomationId(control, automationId);
    }

    private static TextBlock CreateLabel() => new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 12,
        FontWeight = FontWeight.SemiBold,
    };

    private static Control Field(TextBlock label, Control input) => new StackPanel
    {
        Spacing = 6,
        Children = { label, input },
    };

    private static Button PrimaryButton() => new()
    {
        Background = LeserpentTheme.Accent,
        Foreground = Brushes.Black,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(16, 9),
        Margin = new Thickness(5),
    };

    private static void SetName(Control control, string value) =>
        AutomationProperties.SetName(control, value);

    private static string Required(string? value, string label)
    {
        var trimmed = value?.Trim() ?? string.Empty;
        if (trimmed.Length == 0)
        {
            throw new ArgumentException($"{label} is required");
        }
        return trimmed;
    }

    private static string? Optional(string? value)
    {
        var trimmed = value?.Trim();
        return string.IsNullOrEmpty(trimmed) ? null : trimmed;
    }

    private static bool IsExpected(Exception error) => error is
        RemoteRegistrationException
        or RemoteQueryException
        or InvalidDataException
        or ArgumentException
        or HttpRequestException
        or OperationCanceledException;

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
