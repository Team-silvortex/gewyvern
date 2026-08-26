using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;
using System.Text.Json;

internal sealed class RemoteOrchestraWorkspaceWindow : Window
{
    private const double CompactBreakpoint = 1060;
    private const int MaxRetainedRuns = 256;
    private const int MaxRetainedEvents = 256;
    private readonly RemoteOrchestraClient client;
    private readonly DesktopLocalization localization;
    private readonly CancellationTokenSource lifetime = new();
    private CancellationTokenSource? eventRequest;
    private CancellationTokenSource? planRequest;
    private readonly string authority;
    private readonly string principal;
    private readonly bool startLoading;
    private readonly List<RemoteOrchestraRun> runs = [];
    private readonly List<RemoteOrchestraEvent> events = [];
    private readonly List<RemoteOrchestraPlan> plans = [];
    private readonly TextBlock headingText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 25,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock bodyText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 13,
        MaxWidth = 820,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBox runtimeFilterBox = new()
    {
        MaxLength = 128,
        MinWidth = 220,
    };
    private readonly Button applyFilterButton = StandardButton();
    private readonly Button clearFilterButton = StandardButton();
    private readonly Button reloadButton = StandardButton();
    private readonly Button moreRunsButton = StandardButton();
    private readonly Button moreEventsButton = StandardButton();
    private readonly Button runPlanButton = StandardButton();
    private readonly Button cancelRunButton = StandardButton();
    private readonly Button retryRunButton = StandardButton();
    private readonly Button cleanupButton = new()
    {
        Background = LeserpentTheme.Destructive,
        Foreground = Brushes.White,
        FontWeight = FontWeight.SemiBold,
        IsEnabled = false,
        Padding = new Thickness(14, 7),
    };
    private readonly ListBox runsList = new();
    private readonly TextBlock plansHeadingText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 15,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock planCatalogText = new()
    {
        Foreground = LeserpentTheme.Muted,
        FontSize = 12,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly ListBox plansList = new();
    private readonly TextBlock planDetailText = new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 12,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly ListBox planStepsList = new()
    {
        MaxHeight = 120,
    };
    private readonly TextBox approvalNoteBox = new()
    {
        AcceptsReturn = true,
        MaxLength = 500,
        MinHeight = 58,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly ListBox stepsList = new()
    {
        MaxHeight = 128,
    };
    private readonly ListBox eventsList = new();
    private readonly TextBlock runDetailText = new()
    {
        Foreground = LeserpentTheme.Body,
        FontSize = 13,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock eventsHeadingText = new()
    {
        Foreground = LeserpentTheme.Primary,
        FontSize = 15,
        FontWeight = FontWeight.Bold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Grid toolbarGrid = new();
    private readonly Grid workspaceGrid = new();
    private string? runtimeFilter;
    private RemoteOrchestraRun? selectedRun;
    private RemoteOrchestraPlanCatalog? planCatalog;
    private RemoteOrchestraPlan? selectedPlan;
    private uint? nextRunOffset;
    private uint? nextEventOffset;
    private bool runsLoading;
    private bool eventsLoading;
    private bool cleanupLoading;
    private bool plansLoading;
    private bool controlLoading;
    private int runLoadGeneration;
    private int eventLoadGeneration;
    private int planLoadGeneration;
    private string statusKey = "status.ready";
    private object[] statusValues = [];
    private IBrush statusBrush = LeserpentTheme.Muted;
    private OrchestraConfirmationWindow? cleanupConfirmation;
    private OrchestraConfirmationWindow? cancelConfirmation;

    private sealed record RunListItem(RemoteOrchestraRun Run, string Label)
    {
        public override string ToString() => Label;
    }

    private sealed record PlanListItem(RemoteOrchestraPlan Plan, string Label)
    {
        public override string ToString() => Label;
    }

    private sealed record TextListItem(string Label)
    {
        public override string ToString() => Label;
    }

    public RemoteOrchestraWorkspaceWindow(
        RemoteClientOptions options,
        string principal,
        DesktopLocalization? localization = null,
        bool startLoading = true)
    {
        this.principal = principal;
        this.localization = localization ?? DesktopLocalization.ForVerification();
        this.startLoading = startLoading;
        client = new RemoteOrchestraClient(options);
        authority = options.Endpoint.Authority;
        Width = 1240;
        Height = 780;
        MinWidth = 680;
        MinHeight = 720;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");

        ConfigureAutomation();
        runPlanButton.Background = LeserpentTheme.Accent;
        runPlanButton.Foreground = Brushes.Black;
        runPlanButton.FontWeight = FontWeight.SemiBold;
        cancelRunButton.Foreground = LeserpentTheme.Destructive;
        applyFilterButton.Click += (_, _) => ApplyFilter();
        clearFilterButton.Click += (_, _) => ClearFilter();
        reloadButton.Click += (_, _) => Observe(ReloadWorkspaceAsync());
        moreRunsButton.Click += (_, _) => Observe(LoadRunsAsync(reset: false));
        moreEventsButton.Click += (_, _) => Observe(LoadEventsAsync(reset: false));
        cleanupButton.Click += (_, _) => Observe(CleanupSelectedRuntimeAsync());
        runPlanButton.Click += (_, _) => Observe(RunSelectedPlanAsync());
        cancelRunButton.Click += (_, _) => Observe(CancelSelectedRunAsync());
        retryRunButton.Click += (_, _) => Observe(RetrySelectedRunAsync());
        runsList.SelectionChanged += (_, _) => OnRunSelectionChanged();
        plansList.SelectionChanged += (_, _) => OnPlanSelectionChanged();
        approvalNoteBox.TextChanged += (_, _) => UpdateAvailability();
        runtimeFilterBox.KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Enter)
            {
                eventArgs.Handled = true;
                ApplyFilter();
            }
        };

        Content = BuildContent();
        ApplyLocalization();
        ApplyResponsiveLayout(Width < CompactBreakpoint);
        this.localization.Changed += OnLocalizationChanged;
        Opened += (_, _) =>
        {
            if (this.startLoading)
            {
                Observe(ReloadWorkspaceAsync());
            }
        };
        SizeChanged += (_, eventArgs) =>
            ApplyResponsiveLayout(eventArgs.NewSize.Width < CompactBreakpoint);
        KeyDown += OnKeyDown;
        Closed += OnClosed;
    }

    private Control BuildContent()
    {
        var header = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(28, 22, 28, 18),
            Child = new StackPanel
            {
                Spacing = 7,
                Children = { headingText, bodyText },
            },
        };

        toolbarGrid.ColumnSpacing = 9;
        toolbarGrid.RowSpacing = 9;
        toolbarGrid.Margin = new Thickness(24, 14, 24, 14);
        toolbarGrid.Children.Add(runtimeFilterBox);
        toolbarGrid.Children.Add(applyFilterButton);
        toolbarGrid.Children.Add(clearFilterButton);
        toolbarGrid.Children.Add(reloadButton);

        var planPanel = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,*,Auto,Auto,Auto,Auto"),
            RowSpacing = 9,
            Children =
            {
                plansHeadingText,
                planCatalogText,
                plansList,
                planDetailText,
                planStepsList,
                approvalNoteBox,
                runPlanButton,
            },
        };
        Grid.SetRow(planCatalogText, 1);
        Grid.SetRow(plansList, 2);
        Grid.SetRow(planDetailText, 3);
        Grid.SetRow(planStepsList, 4);
        Grid.SetRow(approvalNoteBox, 5);
        Grid.SetRow(runPlanButton, 6);
        runPlanButton.HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left;

        var runPanel = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("*,Auto"),
            RowSpacing = 10,
            Children = { runsList, moreRunsButton },
        };
        Grid.SetRow(moreRunsButton, 1);
        moreRunsButton.HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left;

        var runControls = new WrapPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left,
            ItemHeight = double.NaN,
            ItemWidth = double.NaN,
            Children = { cancelRunButton, retryRunButton, cleanupButton },
        };
        cancelRunButton.Margin = new Thickness(0, 0, 8, 0);
        retryRunButton.Margin = new Thickness(0, 0, 8, 0);

        var detailPanel = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,Auto,*,Auto,Auto"),
            RowSpacing = 9,
            Children =
            {
                runDetailText,
                stepsList,
                eventsHeadingText,
                eventsList,
                moreEventsButton,
                runControls,
            },
        };
        Grid.SetRow(stepsList, 1);
        Grid.SetRow(eventsHeadingText, 2);
        Grid.SetRow(eventsList, 3);
        Grid.SetRow(moreEventsButton, 4);
        Grid.SetRow(runControls, 5);
        moreEventsButton.HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left;

        var planBorder = WorkspacePanel(planPanel);
        var runBorder = WorkspacePanel(runPanel);
        var detailBorder = WorkspacePanel(detailPanel);
        workspaceGrid.ColumnSpacing = 14;
        workspaceGrid.RowSpacing = 14;
        workspaceGrid.Margin = new Thickness(24, 0, 24, 16);
        workspaceGrid.Children.Add(planBorder);
        workspaceGrid.Children.Add(runBorder);
        workspaceGrid.Children.Add(detailBorder);
        Grid.SetColumn(runBorder, 1);
        Grid.SetColumn(detailBorder, 2);

        var status = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(24, 11),
            Child = statusText,
        };
        var root = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,*,Auto"),
            Children = { header, toolbarGrid, workspaceGrid, status },
        };
        Grid.SetRow(toolbarGrid, 1);
        Grid.SetRow(workspaceGrid, 2);
        Grid.SetRow(status, 3);
        return root;
    }

    private static Border WorkspacePanel(Control child) => new()
    {
        Background = LeserpentTheme.Panel,
        BorderBrush = LeserpentTheme.PanelBorder,
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(8),
        Padding = new Thickness(14),
        Child = child,
    };

    private void ConfigureAutomation()
    {
        SetAutomation(headingText, "orchestra-heading");
        SetAutomation(bodyText, "orchestra-scope");
        SetAutomation(statusText, "orchestra-status");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Polite);
        SetAutomation(runtimeFilterBox, "orchestra-runtime-filter");
        SetAutomation(applyFilterButton, "orchestra-filter-apply");
        SetAutomation(clearFilterButton, "orchestra-filter-clear");
        SetAutomation(reloadButton, "orchestra-reload");
        SetAutomation(plansHeadingText, "orchestra-plans-heading");
        SetAutomation(planCatalogText, "orchestra-plan-catalog-detail");
        SetAutomation(plansList, "orchestra-plans");
        SetAutomation(planDetailText, "orchestra-plan-detail");
        SetAutomation(planStepsList, "orchestra-plan-steps");
        SetAutomation(approvalNoteBox, "orchestra-approval-note");
        SetAutomation(runPlanButton, "orchestra-plan-run");
        SetAutomation(runsList, "orchestra-runs");
        SetAutomation(moreRunsButton, "orchestra-runs-more");
        SetAutomation(runDetailText, "orchestra-run-detail");
        SetAutomation(stepsList, "orchestra-steps");
        SetAutomation(eventsHeadingText, "orchestra-events-heading");
        SetAutomation(eventsList, "orchestra-events");
        SetAutomation(moreEventsButton, "orchestra-events-more");
        SetAutomation(cancelRunButton, "orchestra-run-cancel");
        SetAutomation(retryRunButton, "orchestra-run-retry");
        SetAutomation(cleanupButton, "orchestra-cleanup");
    }

    private static void SetAutomation(Control control, string id) =>
        AutomationProperties.SetAutomationId(control, id);

    private void ApplyLocalization()
    {
        FlowDirection = localization.FlowDirection;
        Title = Text("title", authority);
        headingText.Text = Text("heading");
        bodyText.Text = Text("body");
        runtimeFilterBox.PlaceholderText = Text("filter.placeholder");
        applyFilterButton.Content = Text("filter.apply");
        clearFilterButton.Content = localization.Text(DesktopTextKey.Clear);
        reloadButton.Content = localization.Text(DesktopTextKey.Reload);
        moreRunsButton.Content = Text("action.more_runs");
        moreEventsButton.Content = Text("action.more_events");
        plansHeadingText.Text = Text("plans.heading");
        approvalNoteBox.PlaceholderText = Text("approval.placeholder");
        runPlanButton.Content = Text("action.run");
        cancelRunButton.Content = Text("action.cancel");
        retryRunButton.Content = Text("action.retry");
        cleanupButton.Content = Text("action.cleanup");
        AutomationProperties.SetName(headingText, Text("heading"));
        AutomationProperties.SetName(bodyText, Text("body"));
        AutomationProperties.SetName(runtimeFilterBox, Text("a11y.filter"));
        AutomationProperties.SetHelpText(runtimeFilterBox, Text("help.filter"));
        AutomationProperties.SetName(applyFilterButton, Text("filter.apply"));
        AutomationProperties.SetName(
            clearFilterButton,
            localization.Text(DesktopTextKey.Clear));
        AutomationProperties.SetName(
            reloadButton,
            localization.Text(DesktopTextKey.Reload));
        AutomationProperties.SetName(moreRunsButton, Text("action.more_runs"));
        AutomationProperties.SetName(moreEventsButton, Text("action.more_events"));
        AutomationProperties.SetName(plansHeadingText, Text("plans.heading"));
        AutomationProperties.SetName(planCatalogText, Text("plans.none"));
        AutomationProperties.SetName(plansList, Text("a11y.plans"));
        AutomationProperties.SetName(planDetailText, Text("plans.none"));
        AutomationProperties.SetName(planStepsList, Text("a11y.plan_steps"));
        AutomationProperties.SetName(approvalNoteBox, Text("a11y.approval"));
        AutomationProperties.SetHelpText(approvalNoteBox, Text("help.approval"));
        AutomationProperties.SetName(runPlanButton, Text("action.run"));
        AutomationProperties.SetHelpText(runPlanButton, Text("help.run"));
        AutomationProperties.SetName(runsList, Text("a11y.runs"));
        AutomationProperties.SetName(stepsList, Text("a11y.steps"));
        AutomationProperties.SetName(eventsList, Text("a11y.events"));
        AutomationProperties.SetName(cancelRunButton, Text("action.cancel"));
        AutomationProperties.SetHelpText(cancelRunButton, Text("help.cancel"));
        AutomationProperties.SetName(retryRunButton, Text("action.retry"));
        AutomationProperties.SetHelpText(retryRunButton, Text("help.retry"));
        AutomationProperties.SetName(cleanupButton, Text("action.cleanup"));
        AutomationProperties.SetHelpText(cleanupButton, Text("help.cleanup"));
        RefreshPlanProjection(selectedPlan?.PlanId);
        RefreshRunProjection(selectedRun?.RunId);
        RefreshSelectedProjection();
        ApplyStatus();
    }

    private void ApplyResponsiveLayout(bool compact)
    {
        toolbarGrid.ColumnDefinitions = ColumnDefinitions.Parse(
            compact ? "*,Auto" : "*,Auto,Auto,Auto");
        toolbarGrid.RowDefinitions = RowDefinitions.Parse(compact ? "Auto,Auto" : "Auto");
        Grid.SetColumn(runtimeFilterBox, 0);
        Grid.SetRow(runtimeFilterBox, 0);
        Grid.SetColumn(applyFilterButton, 1);
        Grid.SetRow(applyFilterButton, 0);
        Grid.SetColumn(clearFilterButton, compact ? 0 : 2);
        Grid.SetRow(clearFilterButton, compact ? 1 : 0);
        Grid.SetColumn(reloadButton, compact ? 1 : 3);
        Grid.SetRow(reloadButton, compact ? 1 : 0);
        clearFilterButton.HorizontalAlignment = compact
            ? Avalonia.Layout.HorizontalAlignment.Left
            : Avalonia.Layout.HorizontalAlignment.Stretch;

        workspaceGrid.ColumnDefinitions = ColumnDefinitions.Parse(
            compact ? "*" : "2*,2*,3*");
        workspaceGrid.RowDefinitions = RowDefinitions.Parse(
            compact ? "*,*,*" : "*");
        var planPanel = workspaceGrid.Children[0];
        var runPanel = workspaceGrid.Children[1];
        var detailPanel = workspaceGrid.Children[2];
        Grid.SetColumn(planPanel, 0);
        Grid.SetRow(planPanel, 0);
        Grid.SetColumn(runPanel, compact ? 0 : 1);
        Grid.SetRow(runPanel, compact ? 1 : 0);
        Grid.SetColumn(detailPanel, compact ? 0 : 2);
        Grid.SetRow(detailPanel, compact ? 2 : 0);
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

    private void OnKeyDown(object? sender, KeyEventArgs eventArgs)
    {
        _ = sender;
        var modifier = eventArgs.KeyModifiers & (KeyModifiers.Control | KeyModifiers.Meta);
        if (eventArgs.Key == Key.F && modifier != KeyModifiers.None)
        {
            eventArgs.Handled = true;
            runtimeFilterBox.Focus();
            runtimeFilterBox.SelectAll();
        }
        else if (eventArgs.Key == Key.F5 && reloadButton.IsEnabled)
        {
            eventArgs.Handled = true;
            Observe(ReloadWorkspaceAsync());
        }
        else if (eventArgs.Key == Key.Escape
            && !string.IsNullOrWhiteSpace(runtimeFilterBox.Text))
        {
            eventArgs.Handled = true;
            ClearFilter();
        }
    }

    private void ApplyFilter()
    {
        var candidate = runtimeFilterBox.Text?.Trim();
        if (string.IsNullOrEmpty(candidate))
        {
            candidate = null;
        }
        if (candidate is not null && !ValidIdentifier(candidate))
        {
            SetStatus("filter.invalid", LeserpentTheme.Destructive);
            return;
        }
        runtimeFilterBox.Text = candidate ?? string.Empty;
        runtimeFilter = candidate;
        ClearPlans();
        Observe(ReloadWorkspaceAsync());
    }

    private void ClearFilter()
    {
        runtimeFilterBox.Text = string.Empty;
        runtimeFilter = null;
        ClearPlans();
        Observe(ReloadWorkspaceAsync());
    }

    private async Task ReloadWorkspaceAsync()
    {
        await LoadRunsAsync(reset: true);
        if (runs.Count == 0 && runtimeFilter is { } exactRuntime)
        {
            await LoadPlansAsync(exactRuntime, null, force: true);
        }
    }

    private async Task LoadRunsAsync(bool reset)
    {
        if (runsLoading || cleanupLoading || lifetime.IsCancellationRequested)
        {
            return;
        }
        var offset = reset ? 0 : nextRunOffset;
        if (offset is null || offset > RemoteOrchestraClient.MaxOffset)
        {
            return;
        }
        runsLoading = true;
        var generation = ++runLoadGeneration;
        var filter = runtimeFilter;
        UpdateAvailability();
        SetStatus("status.loading_runs", LeserpentTheme.Muted);
        try
        {
            var page = await client.LoadRunsAsync(
                filter,
                offset.Value,
                RemoteOrchestraClient.DefaultPageSize,
                principal,
                lifetime.Token);
            if (generation != runLoadGeneration
                || lifetime.IsCancellationRequested
                || filter != runtimeFilter)
            {
                return;
            }
            var previousRunId = reset ? null : selectedRun?.RunId;
            if (reset)
            {
                runs.Clear();
                selectedRun = null;
                events.Clear();
                nextEventOffset = null;
            }
            if (runs.Count + page.Runs.Count > MaxRetainedRuns
                || page.Runs.Any(pageRun => runs.Any(
                    retained => retained.RunId == pageRun.RunId)))
            {
                throw new InvalidDataException(
                    "Orchestra run pagination exceeded its projection bounds");
            }
            runs.AddRange(page.Runs);
            nextRunOffset = runs.Count < MaxRetainedRuns
                && page.NextOffset is <= RemoteOrchestraClient.MaxOffset
                ? page.NextOffset
                : null;
            RefreshRunProjection(previousRunId);
            if (runs.Count == 0)
            {
                if (filter is null)
                {
                    ClearPlans();
                }
                SetStatus("status.no_runs", LeserpentTheme.Muted);
            }
            else
            {
                SetStatus("status.runs_loaded", LeserpentTheme.Body, runs.Count);
                if (runsList.SelectedItem is null)
                {
                    runsList.SelectedIndex = 0;
                }
            }
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowFailure(error);
        }
        finally
        {
            runsLoading = false;
            UpdateAvailability();
        }
    }

    private void OnRunSelectionChanged()
    {
        if (runsList.SelectedItem is not RunListItem item
            || selectedRun?.RunId == item.Run.RunId)
        {
            return;
        }
        selectedRun = item.Run;
        events.Clear();
        nextEventOffset = 0;
        RefreshSelectedProjection();
        Observe(LoadPlansAsync(item.Run.RuntimeId, item.Run.PlanId));
        Observe(LoadEventsAsync(reset: true));
    }

    private void OnPlanSelectionChanged()
    {
        if (plansList.SelectedItem is not PlanListItem item
            || selectedPlan?.PlanId == item.Plan.PlanId)
        {
            return;
        }
        selectedPlan = item.Plan;
        approvalNoteBox.Text = string.Empty;
        RefreshSelectedPlanProjection();
    }

    private async Task LoadPlansAsync(
        string runtimeId,
        string? preferredPlanId,
        bool force = false)
    {
        if (lifetime.IsCancellationRequested)
        {
            return;
        }
        if (!force && planCatalog?.RuntimeId == runtimeId)
        {
            RefreshPlanProjection(preferredPlanId ?? selectedPlan?.PlanId);
            return;
        }

        var generation = ++planLoadGeneration;
        planRequest?.Cancel();
        planRequest?.Dispose();
        var request = CancellationTokenSource.CreateLinkedTokenSource(lifetime.Token);
        planRequest = request;
        plansLoading = true;
        UpdateAvailability();
        SetStatus("status.loading_plans", LeserpentTheme.Muted, runtimeId);
        try
        {
            var catalog = await client.LoadPlansAsync(
                runtimeId,
                principal,
                request.Token);
            if (generation != planLoadGeneration
                || request.IsCancellationRequested
                || lifetime.IsCancellationRequested)
            {
                return;
            }
            planCatalog = catalog;
            plans.Clear();
            plans.AddRange(catalog.Plans);
            selectedPlan = null;
            approvalNoteBox.Text = string.Empty;
            RefreshPlanProjection(preferredPlanId);
            SetStatus(
                "status.plans_loaded",
                LeserpentTheme.Body,
                plans.Count);
        }
        catch (OperationCanceledException) when (request.IsCancellationRequested)
        {
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            if (generation == planLoadGeneration)
            {
                ShowFailure(error);
            }
        }
        finally
        {
            if (generation == planLoadGeneration)
            {
                plansLoading = false;
                if (ReferenceEquals(planRequest, request))
                {
                    planRequest = null;
                }
                UpdateAvailability();
            }
            request.Dispose();
        }
    }

    private async Task LoadEventsAsync(bool reset)
    {
        var run = selectedRun;
        if (run is null
            || cleanupLoading
            || lifetime.IsCancellationRequested)
        {
            return;
        }
        var offset = reset ? 0 : nextEventOffset;
        if (offset is null || offset > RemoteOrchestraClient.MaxOffset)
        {
            return;
        }
        var generation = ++eventLoadGeneration;
        eventRequest?.Cancel();
        eventRequest?.Dispose();
        var request = CancellationTokenSource.CreateLinkedTokenSource(lifetime.Token);
        eventRequest = request;
        eventsLoading = true;
        UpdateAvailability();
        SetStatus("status.loading_events", LeserpentTheme.Muted, run.RunId);
        try
        {
            var page = await client.LoadEventsAsync(
                run.RuntimeId,
                run.RunId,
                offset.Value,
                RemoteOrchestraClient.DefaultPageSize,
                principal,
                request.Token);
            if (generation != eventLoadGeneration
                || selectedRun?.RunId != run.RunId
                || request.IsCancellationRequested
                || lifetime.IsCancellationRequested)
            {
                return;
            }
            if (reset)
            {
                events.Clear();
            }
            if (events.Count + page.Events.Count > MaxRetainedEvents
                || page.Events.Any(pageEvent => events.Any(
                    retained => retained.EventId == pageEvent.EventId)))
            {
                throw new InvalidDataException(
                    "Orchestra event pagination exceeded its projection bounds");
            }
            events.AddRange(page.Events);
            nextEventOffset = events.Count < MaxRetainedEvents
                && page.NextOffset is <= RemoteOrchestraClient.MaxOffset
                ? page.NextOffset
                : null;
            RefreshEventProjection();
            SetStatus(
                events.Count == 0 ? "status.no_events" : "status.events_loaded",
                LeserpentTheme.Body,
                events.Count);
        }
        catch (OperationCanceledException) when (request.IsCancellationRequested)
        {
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowFailure(error);
        }
        finally
        {
            if (generation == eventLoadGeneration)
            {
                eventsLoading = false;
                if (ReferenceEquals(eventRequest, request))
                {
                    eventRequest = null;
                }
                UpdateAvailability();
            }
            request.Dispose();
        }
    }

    private async Task CleanupSelectedRuntimeAsync()
    {
        var run = selectedRun;
        if (run is null
            || cleanupLoading
            || runs.Any(candidate => candidate.RuntimeId == run.RuntimeId
                && candidate.Outcome is "queued" or "running")
            || lifetime.IsCancellationRequested)
        {
            return;
        }
        cleanupConfirmation = new OrchestraConfirmationWindow(
            run.RuntimeId,
            OrchestraConfirmationKind.Cleanup,
            localization);
        var confirmed = await cleanupConfirmation.ShowDialog<bool>(this);
        cleanupConfirmation = null;
        if (!confirmed || lifetime.IsCancellationRequested)
        {
            return;
        }
        cleanupLoading = true;
        ++eventLoadGeneration;
        UpdateAvailability();
        SetStatus(
            "status.cleanup_loading",
            LeserpentTheme.Destructive,
            run.RuntimeId);
        try
        {
            var receipt = await client.DeleteRuntimeHistoryAsync(
                run.RuntimeId,
                principal,
                lifetime.Token);
            runs.RemoveAll(candidate => candidate.RuntimeId == run.RuntimeId);
            selectedRun = null;
            events.Clear();
            nextEventOffset = null;
            nextRunOffset = null;
            RefreshRunProjection();
            RefreshSelectedProjection();
            SetStatus(
                "status.cleanup_completed",
                LeserpentTheme.Body,
                receipt.DeletedRunCount,
                receipt.DeletedEventCount);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowFailure(error);
        }
        finally
        {
            cleanupLoading = false;
            UpdateAvailability();
        }
    }

    private async Task RunSelectedPlanAsync()
    {
        var plan = selectedPlan;
        var catalog = planCatalog;
        if (plan is null
            || catalog is null
            || controlLoading
            || lifetime.IsCancellationRequested
            || !TryGetApproval(plan, out var approvedBy, out var approvalNote))
        {
            return;
        }
        controlLoading = true;
        UpdateAvailability();
        SetStatus("status.running_plan", LeserpentTheme.Accent, plan.PlanId);
        try
        {
            var receipt = await client.RunPlanAsync(
                catalog.RuntimeId,
                plan,
                principal,
                approvedBy,
                approvalNote,
                lifetime.Token);
            ApplyRunReceipt(receipt.Run);
            await LoadPlansAsync(catalog.RuntimeId, plan.PlanId, force: true);
            SetStatus(
                "status.run_queued",
                LeserpentTheme.Body,
                receipt.Run.RunId);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowFailure(error);
        }
        finally
        {
            controlLoading = false;
            UpdateAvailability();
        }
    }

    private async Task CancelSelectedRunAsync()
    {
        var run = selectedRun;
        if (run is null
            || run.Outcome != "queued"
            || controlLoading
            || lifetime.IsCancellationRequested)
        {
            return;
        }
        cancelConfirmation = new OrchestraConfirmationWindow(
            run.RunId,
            OrchestraConfirmationKind.Cancel,
            localization);
        var confirmed = await cancelConfirmation.ShowDialog<bool>(this);
        cancelConfirmation = null;
        if (!confirmed || lifetime.IsCancellationRequested)
        {
            return;
        }
        controlLoading = true;
        ++eventLoadGeneration;
        eventRequest?.Cancel();
        UpdateAvailability();
        SetStatus("status.cancelling", LeserpentTheme.Destructive, run.RunId);
        try
        {
            var receipt = await client.CancelRunAsync(
                run,
                principal,
                lifetime.Token);
            ApplyRunReceipt(receipt.Run);
            SetStatus(
                "status.cancelled",
                LeserpentTheme.Body,
                receipt.Run.RunId);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowFailure(error);
        }
        finally
        {
            controlLoading = false;
            UpdateAvailability();
        }
    }

    private async Task RetrySelectedRunAsync()
    {
        var run = selectedRun;
        var plan = selectedPlan;
        if (run is null
            || plan is null
            || run.Outcome is "queued" or "running"
            || run.PlanId != plan.PlanId
            || controlLoading
            || lifetime.IsCancellationRequested
            || !TryGetApproval(plan, out var approvedBy, out var approvalNote))
        {
            return;
        }
        controlLoading = true;
        UpdateAvailability();
        SetStatus("status.retrying", LeserpentTheme.Accent, run.RunId);
        try
        {
            var receipt = await client.RetryRunAsync(
                run,
                plan,
                principal,
                approvedBy,
                approvalNote,
                lifetime.Token);
            ApplyRunReceipt(receipt.Run);
            await LoadPlansAsync(run.RuntimeId, plan.PlanId, force: true);
            SetStatus(
                "status.retry_queued",
                LeserpentTheme.Body,
                receipt.Run.RunId);
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
        catch (Exception error)
        {
            ShowFailure(error);
        }
        finally
        {
            controlLoading = false;
            UpdateAvailability();
        }
    }

    private bool TryGetApproval(
        RemoteOrchestraPlan plan,
        out string? approvedBy,
        out string? approvalNote)
    {
        approvedBy = null;
        approvalNote = null;
        if (plan.ApprovalMode != "operator_confirmation")
        {
            return true;
        }
        var note = approvalNoteBox.Text?.Trim();
        if (string.IsNullOrEmpty(note))
        {
            SetStatus("approval.required", LeserpentTheme.Destructive);
            return false;
        }
        approvedBy = principal;
        approvalNote = note;
        return true;
    }

    private void ApplyRunReceipt(RemoteOrchestraRun run)
    {
        runs.RemoveAll(candidate => candidate.RunId == run.RunId);
        runs.Insert(0, run);
        if (runs.Count > MaxRetainedRuns)
        {
            runs.RemoveRange(MaxRetainedRuns, runs.Count - MaxRetainedRuns);
            nextRunOffset = null;
        }
        selectedRun = run;
        events.Clear();
        nextEventOffset = 0;
        RefreshRunProjection(run.RunId);
        RefreshSelectedProjection();
        Observe(LoadEventsAsync(reset: true));
    }

    private void ClearPlans()
    {
        ++planLoadGeneration;
        planRequest?.Cancel();
        planRequest?.Dispose();
        planRequest = null;
        plansLoading = false;
        plans.Clear();
        planCatalog = null;
        selectedPlan = null;
        approvalNoteBox.Text = string.Empty;
        RefreshPlanProjection();
    }

    private void RefreshPlanProjection(string? preferredPlanId = null)
    {
        var items = plans.Select(plan => new PlanListItem(
            plan,
            Text(
                "plan.label",
                plan.PlanId,
                plan.RiskLevel,
                plan.ExecutionMode,
                plan.ExecutionReadiness)))
            .ToArray();
        var preferred = preferredPlanId is null
            ? null
            : items.FirstOrDefault(item => item.Plan.PlanId == preferredPlanId);
        var selected = preferred
            ?? items.FirstOrDefault(item => item.Plan.PlanId == selectedPlan?.PlanId)
            ?? items.FirstOrDefault(item => item.Plan.ExecutionMode == "automatic")
            ?? items.FirstOrDefault();
        selectedPlan = selected?.Plan;
        plansList.ItemsSource = items;
        plansList.SelectedItem = selected;
        if (planCatalog is null)
        {
            planCatalogText.Text = Text("plans.none");
        }
        else
        {
            var reasons = planCatalog.AttentionReasons.Count == 0
                ? "-"
                : string.Join(", ", planCatalog.AttentionReasons);
            planCatalogText.Text = Text(
                "catalog.detail",
                planCatalog.RuntimeName,
                planCatalog.RuntimeRevision,
                planCatalog.AttentionSeverity,
                planCatalog.StatusSource,
                reasons);
        }
        AutomationProperties.SetName(planCatalogText, planCatalogText.Text);
        RefreshSelectedPlanProjection();
    }

    private void RefreshSelectedPlanProjection()
    {
        if (selectedPlan is null)
        {
            planDetailText.Text = Text("plans.none");
            planStepsList.ItemsSource = Array.Empty<TextListItem>();
        }
        else
        {
            planDetailText.Text = Text(
                "plan.detail",
                selectedPlan.Title,
                selectedPlan.Summary,
                selectedPlan.ApprovalMode,
                selectedPlan.Revision);
            planStepsList.ItemsSource = selectedPlan.Steps.Select(step =>
                new TextListItem(Text(
                    "plan.step",
                    step.Title,
                    step.Kind,
                    step.Detail))).ToArray();
        }
        AutomationProperties.SetName(planDetailText, planDetailText.Text);
        UpdateAvailability();
    }

    private void RefreshRunProjection(string? preferredRunId = null)
    {
        var items = runs.Select(run => new RunListItem(
            run,
            Text(
                "run.label",
                run.RuntimeId,
                run.Outcome,
                run.PlanId,
                run.Attempt)))
            .ToArray();
        runsList.ItemsSource = items;
        if (preferredRunId is not null)
        {
            runsList.SelectedItem = items.FirstOrDefault(
                item => item.Run.RunId == preferredRunId);
        }
        UpdateAvailability();
    }

    private void RefreshSelectedProjection()
    {
        if (selectedRun is null)
        {
            runDetailText.Text = Text("selection.none");
            eventsHeadingText.Text = Text("heading");
            stepsList.ItemsSource = Array.Empty<TextListItem>();
            eventsList.ItemsSource = Array.Empty<TextListItem>();
            AutomationProperties.SetName(runDetailText, runDetailText.Text);
            AutomationProperties.SetName(eventsHeadingText, eventsHeadingText.Text);
            UpdateAvailability();
            return;
        }
        runDetailText.Text = Text(
            "run.detail",
            selectedRun.RunId,
            selectedRun.ExecutedAt,
            selectedRun.CompletedAt ?? "-");
        eventsHeadingText.Text = Text("events.heading", selectedRun.RunId);
        stepsList.ItemsSource = selectedRun.Steps.Select(step => new TextListItem(
            Text("step.label", step.Step, step.Outcome, step.Summary))).ToArray();
        AutomationProperties.SetName(runDetailText, runDetailText.Text);
        AutomationProperties.SetName(eventsHeadingText, eventsHeadingText.Text);
        RefreshEventProjection();
        UpdateAvailability();
    }

    private void RefreshEventProjection()
    {
        eventsList.ItemsSource = events.Select(orchestraEvent => new TextListItem(
            Text(
                "event.label",
                orchestraEvent.RecordedAt,
                orchestraEvent.EventType,
                orchestraEvent.FromOutcome ?? "-",
                orchestraEvent.ToOutcome,
                orchestraEvent.Summary)))
            .ToArray();
    }

    private void UpdateAvailability()
    {
        var busy = runsLoading
            || eventsLoading
            || cleanupLoading
            || plansLoading
            || controlLoading;
        var plan = selectedPlan;
        var planIsExecutable = plan is
        {
            ExecutionMode: "automatic",
            ExecutionReadiness: "ready_now",
        };
        var approvalReady = plan?.ApprovalMode != "operator_confirmation"
            || !string.IsNullOrWhiteSpace(approvalNoteBox.Text);
        var activeForPlanRuntime = planCatalog is { } catalog
            && runs.Any(run => run.RuntimeId == catalog.RuntimeId
                && run.Outcome is "queued" or "running");
        var selectedIsTerminal = selectedRun is { } run
            && run.Outcome is not ("queued" or "running");
        runtimeFilterBox.IsEnabled = !busy;
        applyFilterButton.IsEnabled = !busy;
        clearFilterButton.IsEnabled = !busy && !string.IsNullOrEmpty(runtimeFilter);
        reloadButton.IsEnabled = !busy;
        moreRunsButton.IsEnabled = !busy && nextRunOffset is <= RemoteOrchestraClient.MaxOffset;
        moreEventsButton.IsEnabled = !busy
            && selectedRun is not null
            && nextEventOffset is <= RemoteOrchestraClient.MaxOffset;
        plansList.IsEnabled = !busy;
        approvalNoteBox.IsEnabled = !busy
            && planIsExecutable
            && plan?.ApprovalMode == "operator_confirmation";
        runPlanButton.IsEnabled = !busy
            && planIsExecutable
            && approvalReady
            && planCatalog is not null
            && !activeForPlanRuntime;
        cancelRunButton.IsEnabled = !busy && selectedRun?.Outcome == "queued";
        retryRunButton.IsEnabled = !busy
            && selectedIsTerminal
            && planIsExecutable
            && approvalReady
            && selectedRun?.PlanId == plan?.PlanId
            && selectedRun?.RuntimeId == planCatalog?.RuntimeId;
        cleanupButton.IsEnabled = !busy
            && selectedRun is { } cleanupRun
            && !runs.Any(run => run.RuntimeId == cleanupRun.RuntimeId
                && run.Outcome is "queued" or "running");
    }

    private void SetStatus(string key, IBrush brush, params object[] values)
    {
        statusKey = key;
        statusValues = values;
        statusBrush = brush;
        ApplyStatus();
    }

    private void ApplyStatus()
    {
        statusText.Text = Text(statusKey, statusValues);
        statusText.Foreground = statusBrush;
        AutomationProperties.SetName(
            statusText,
            $"{Text("a11y.status")}: {statusText.Text}");
    }

    private void ShowFailure(Exception error)
    {
        switch (error)
        {
            case RemoteOrchestraException rejected:
                SetStatus(
                    "status.failed_rejected",
                    LeserpentTheme.Destructive,
                    rejected.Code);
                break;
            case InvalidDataException or JsonException:
                SetStatus("status.failed_response", LeserpentTheme.Destructive);
                break;
            default:
                SetStatus("status.failed_transport", LeserpentTheme.Destructive);
                break;
        }
    }

    private string Text(string key, params object[] values) => values.Length == 0
        ? DesktopOrchestraCatalogs.Resolve(localization, key)
        : DesktopOrchestraCatalogs.Format(localization, key, values);

    private static bool ValidIdentifier(string value) => value.Length is >= 1 and <= 128
        && value.All(character => char.IsAsciiLetterOrDigit(character)
            || character is '-' or '_' or '.' or ':');

    private static Button StandardButton() => new()
    {
        Padding = new Thickness(12, 7),
    };

    private async void Observe(Task operation)
    {
        try
        {
            await operation;
        }
        catch (Exception error) when (!lifetime.IsCancellationRequested)
        {
            ShowFailure(error);
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
        }
    }

    private void OnClosed(object? sender, EventArgs eventArgs)
    {
        _ = sender;
        _ = eventArgs;
        localization.Changed -= OnLocalizationChanged;
        ++runLoadGeneration;
        ++eventLoadGeneration;
        ++planLoadGeneration;
        lifetime.Cancel();
        eventRequest?.Cancel();
        eventRequest?.Dispose();
        eventRequest = null;
        planRequest?.Cancel();
        planRequest?.Dispose();
        planRequest = null;
        cleanupConfirmation?.Close(false);
        cleanupConfirmation = null;
        cancelConfirmation?.Close(false);
        cancelConfirmation = null;
        client.Dispose();
        lifetime.Dispose();
    }

    public void ProbeProjection()
    {
        plans.Clear();
        runs.Clear();
        events.Clear();
        var plan = new RemoteOrchestraPlan
        {
            PlanId = "runtime_triage",
            Intent = "triage",
            Title = "Refresh and verify runtime posture",
            Summary = "Refresh the authoritative runtime status.",
            RiskLevel = "low",
            ExecutionReadiness = "ready_now",
            ExecutionMode = "automatic",
            ApprovalMode = "none",
            Revision = "orchestra-v1-42-runtime_triage",
            Reasons = [],
            RequiredCapabilities = [],
            Steps =
            [
                new RemoteOrchestraPlanStep
                {
                    Key = "refresh_status",
                    Title = "Refresh runtime status",
                    Detail = "Run the bounded native status adapter.",
                    Kind = "refresh",
                },
            ],
        };
        plans.Add(plan);
        planCatalog = new RemoteOrchestraPlanCatalog
        {
            RuntimeId = "runtime-verification",
            RuntimeName = "Verification runtime",
            RuntimeRevision = 42,
            StatusSource = "verification",
            AttentionSeverity = "healthy",
            NeedsAttention = false,
            AttentionReasons = [],
            Plans = [plan],
        };
        selectedPlan = plan;
        runs.Add(new RemoteOrchestraRun
        {
            RunId = "orun-verification",
            RuntimeId = "runtime-verification",
            PlanId = "runtime_triage",
            Outcome = "succeeded",
            ExecutedAt = "2026-08-26T08:00:00Z",
            CompletedAt = "2026-08-26T08:00:01Z",
            Attempt = 1,
            PlanRevision = plan.Revision,
            RequestId = "verification-request",
            Steps =
            [
                new RemoteOrchestraStep
                {
                    Step = "deploy",
                    Outcome = "succeeded",
                    Summary = "deployment accepted",
                },
            ],
        });
        selectedRun = runs[0];
        events.Add(new RemoteOrchestraEvent
        {
            EventId = 1,
            RunId = selectedRun.RunId,
            RuntimeId = selectedRun.RuntimeId,
            EventType = "guided_completion",
            FromOutcome = "running",
            ToOutcome = "succeeded",
            Summary = "deployment accepted",
            RecordedAt = "2026-08-26T08:00:01Z",
        });
        nextRunOffset = null;
        nextEventOffset = null;
        RefreshPlanProjection(plan.PlanId);
        RefreshRunProjection(selectedRun.RunId);
        RefreshSelectedProjection();
        SetStatus("status.events_loaded", LeserpentTheme.Body, events.Count);
        if (plansList.ItemCount != 1
            || planStepsList.ItemCount != 1
            || runsList.ItemCount != 1
            || stepsList.ItemCount != 1
            || eventsList.ItemCount != 1
            || !runPlanButton.IsEnabled
            || !retryRunButton.IsEnabled
            || cancelRunButton.IsEnabled
            || !cleanupButton.IsEnabled)
        {
            throw new InvalidDataException(
                "Orchestra workspace projection contract drifted");
        }
    }

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("Orchestra workspace has no control root");
        }
        foreach (var (width, height, compact) in new[]
        {
            (MinWidth, MinHeight, true),
            (1240d, 780d, false),
        })
        {
            ApplyResponsiveLayout(compact);
            root.Measure(new Size(width, height));
            var desired = root.DesiredSize;
            if (!double.IsFinite(desired.Width)
                || !double.IsFinite(desired.Height)
                || desired.Width <= 0
                || desired.Height <= 0
                || desired.Width > width
                || desired.Height > height
                || (compact && Grid.GetRow(workspaceGrid.Children[1]) != 1)
                || (compact && Grid.GetRow(workspaceGrid.Children[2]) != 2)
                || (!compact && Grid.GetColumn(workspaceGrid.Children[1]) != 1)
                || (!compact && Grid.GetColumn(workspaceGrid.Children[2]) != 2))
            {
                throw new InvalidDataException(
                    "Orchestra workspace controls exceeded their layout envelope");
            }
        }
        ApplyResponsiveLayout(Width < CompactBreakpoint);
    }

    public void VerifyAccessibility()
    {
        var controls = new Control[]
        {
            headingText,
            bodyText,
            statusText,
            runtimeFilterBox,
            applyFilterButton,
            clearFilterButton,
            reloadButton,
            plansHeadingText,
            planCatalogText,
            plansList,
            planDetailText,
            planStepsList,
            approvalNoteBox,
            runPlanButton,
            runsList,
            moreRunsButton,
            runDetailText,
            stepsList,
            eventsHeadingText,
            eventsList,
            moreEventsButton,
            cancelRunButton,
            retryRunButton,
            cleanupButton,
        };
        var ids = new HashSet<string>(StringComparer.Ordinal);
        if (controls.Any(control =>
                string.IsNullOrWhiteSpace(AutomationProperties.GetAutomationId(control))
                || string.IsNullOrWhiteSpace(AutomationProperties.GetName(control))
                || !ids.Add(AutomationProperties.GetAutomationId(control)!))
            || AutomationProperties.GetLiveSetting(statusText)
                != AutomationLiveSetting.Polite)
        {
            throw new InvalidDataException(
                "Orchestra workspace accessibility contract drifted");
        }
    }
}

internal enum OrchestraConfirmationKind
{
    Cleanup,
    Cancel,
}

internal sealed class OrchestraConfirmationWindow : Window
{
    private readonly DesktopLocalization localization;
    private readonly string targetId;
    private readonly string keyPrefix;
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
        FontSize = 14,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock warning = new()
    {
        Foreground = LeserpentTheme.Destructive,
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button cancelButton = new()
    {
        Padding = new Thickness(15, 8),
    };
    private readonly Button confirmButton = new()
    {
        Background = LeserpentTheme.Destructive,
        Foreground = Brushes.White,
        FontWeight = FontWeight.SemiBold,
        Padding = new Thickness(15, 8),
    };

    public OrchestraConfirmationWindow(
        string targetId,
        OrchestraConfirmationKind kind,
        DesktopLocalization localization)
    {
        this.targetId = targetId;
        this.localization = localization;
        keyPrefix = kind == OrchestraConfirmationKind.Cleanup
            ? "cleanup"
            : "cancel";
        Width = 520;
        MinWidth = 440;
        SizeToContent = SizeToContent.Height;
        CanResize = false;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        AutomationProperties.SetAutomationId(
            heading,
            $"orchestra-{keyPrefix}-heading");
        AutomationProperties.SetAutomationId(
            body,
            $"orchestra-{keyPrefix}-body");
        AutomationProperties.SetAutomationId(
            warning,
            $"orchestra-{keyPrefix}-warning");
        AutomationProperties.SetAutomationId(
            cancelButton,
            $"orchestra-{keyPrefix}-dismiss");
        AutomationProperties.SetAutomationId(
            confirmButton,
            $"orchestra-{keyPrefix}-confirm");
        cancelButton.Click += (_, _) => Close(false);
        confirmButton.Click += (_, _) => Close(true);
        KeyDown += (_, eventArgs) =>
        {
            if (eventArgs.Key == Key.Escape)
            {
                eventArgs.Handled = true;
                Close(false);
            }
        };
        var buttons = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
            Spacing = 10,
            Children = { cancelButton, confirmButton },
        };
        Content = new Border
        {
            Padding = new Thickness(28, 24),
            Child = new StackPanel
            {
                Spacing = 14,
                Children = { heading, body, warning, buttons },
            },
        };
        localization.Changed += OnLocalizationChanged;
        Closed += (_, _) => localization.Changed -= OnLocalizationChanged;
        ApplyLocalization();
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
        Title = Text($"{keyPrefix}.title");
        heading.Text = Text($"{keyPrefix}.title");
        body.Text = Text($"{keyPrefix}.body", targetId);
        warning.Text = Text($"{keyPrefix}.warning");
        cancelButton.Content = localization.Text(DesktopTextKey.Cancel);
        confirmButton.Content = Text($"{keyPrefix}.confirm");
        AutomationProperties.SetName(heading, heading.Text);
        AutomationProperties.SetName(body, body.Text);
        AutomationProperties.SetName(warning, warning.Text);
        AutomationProperties.SetName(
            cancelButton,
            localization.Text(DesktopTextKey.Cancel));
        AutomationProperties.SetName(
            confirmButton,
            Text($"{keyPrefix}.confirm"));
    }

    private string Text(string key, params object[] values) => values.Length == 0
        ? DesktopOrchestraCatalogs.Resolve(localization, key)
        : DesktopOrchestraCatalogs.Format(localization, key, values);

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException("Orchestra cleanup dialog has no control root");
        }
        root.Measure(new Size(Width, 600));
        if (!double.IsFinite(root.DesiredSize.Width)
            || !double.IsFinite(root.DesiredSize.Height)
            || root.DesiredSize.Width <= 0
            || root.DesiredSize.Height <= 0
            || root.DesiredSize.Width > Width
            || root.DesiredSize.Height > 600)
        {
            throw new InvalidDataException(
                "Orchestra cleanup dialog exceeded its layout envelope");
        }
    }
}
