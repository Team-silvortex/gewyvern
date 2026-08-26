using Avalonia;
using Avalonia.Automation;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Input.Platform;
using Avalonia.Media;
using Avalonia.Platform.Storage;
using Avalonia.Threading;

internal sealed class RemoteRuntimeWorkspaceWindow : Window
{
    private readonly AvaloniaDocumentRenderer renderer;
    private readonly RemoteWorkspaceClient client;
    private readonly RemoteLeselangClient leselangClient;
    private readonly RemoteClientOptions options;
    private readonly DesktopLocalization localization;
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private readonly string runtimeName;
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button reloadButton = new()
    {
        Padding = new Thickness(12, 6),
    };
    private readonly Button liveRefreshButton = new()
    {
        Padding = new Thickness(12, 6),
    };
    private readonly Button acknowledgeAlertButton = new()
    {
        Padding = new Thickness(12, 6),
        IsVisible = false,
    };
    private readonly TextBox logSearchBox = new()
    {
        MaxLength = RemoteWorkspaceLogFilter.MaxQueryLength,
        MinWidth = 180,
    };
    private readonly ComboBox logLevelBox = new()
    {
        MinWidth = 112,
    };
    private readonly Button clearLogFilterButton = new()
    {
        Padding = new Thickness(12, 6),
        IsVisible = false,
    };
    private readonly Button copyDiagnosticsButton = new()
    {
        Padding = new Thickness(12, 6),
        IsEnabled = false,
    };
    private readonly Button saveDiagnosticsButton = new()
    {
        Padding = new Thickness(12, 6),
        IsEnabled = false,
    };
    private readonly Button workspaceLeselangButton = new()
    {
        Padding = new Thickness(12, 6),
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left,
    };
    private readonly Button editRegistrationButton = new()
    {
        Padding = new Thickness(12, 6),
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Right,
        IsEnabled = false,
    };
    private readonly TextBlock logFilterSummary = new()
    {
        FontSize = 12,
        Foreground = LeserpentTheme.Muted,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly TextBlock diagnosticCopyStatus = new()
    {
        FontSize = 12,
        TextWrapping = TextWrapping.Wrap,
        IsVisible = false,
    };
    private readonly DispatcherTimer logFilterTimer = new()
    {
        Interval = TimeSpan.FromMilliseconds(120),
    };
    private readonly DispatcherTimer liveRefreshTimer = new()
    {
        Interval = RemoteWorkspaceLiveRefresh.Interval,
    };
    private readonly RemoteWorkspaceLiveRefresh liveRefresh = new();
    private readonly RemoteWorkspaceSeverityAlert severityAlert = new();
    private readonly RemoteWorkspaceLogRefreshPlan logRefreshPlan = new();
    private bool loadInFlight;
    private bool diagnosticSaveInFlight;
    private bool applyingLocalization;
    private bool registrationMutationEnabled;
    private Window? workspaceLeselangWindow;
    private RuntimeRegistrationWindow? registrationWindow;
    private ulong loadedRevision;
    private ulong desiredRevision;
    private RemoteWorkspaceSnapshot? latestSnapshot;
    private DesktopRuntimeWorkspaceText? currentStatus;
    private DesktopRuntimeWorkspaceText? diagnosticStatus;

    private sealed record LogLevelOption(string Value, string Label)
    {
        public override string ToString() => Label;
    }

    public RemoteRuntimeWorkspaceWindow(
        RemoteClientOptions options,
        RemoteRuntimeProjection runtime,
        string principal,
        Action<RenderedActionInvocation> actionInvoked,
        DesktopLocalization? localization = null)
    {
        RuntimeId = runtime.Id;
        this.options = options;
        this.principal = principal;
        this.localization = localization ?? DesktopLocalization.ForVerification();
        runtimeName = Safe(runtime.Name);
        client = new RemoteWorkspaceClient(options);
        leselangClient = new RemoteLeselangClient(options);
        renderer = new AvaloniaDocumentRenderer(
            actionInvoked,
            this.localization.Resolve);
        Width = 760;
        Height = 620;
        MinWidth = 520;
        MinHeight = 420;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        FlowDirection = this.localization.FlowDirection;
        Title = $"{runtimeName} / Leserpent";
        AutomationProperties.SetAutomationId(statusText, "runtime-workspace-status");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(reloadButton, "runtime-workspace-reload");
        reloadButton.Click += (_, _) => _ = ReloadAsync();
        ConfigureLiveRefresh();
        ConfigureSeverityAlert();
        ConfigureLogFilter();
        var queryControls = new StackPanel
        {
            Orientation = Avalonia.Layout.Orientation.Horizontal,
            Spacing = 8,
            Children = { acknowledgeAlertButton, liveRefreshButton, reloadButton },
        };
        var status = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(20, 10),
            Child = new Grid
            {
                ColumnDefinitions = ColumnDefinitions.Parse("*,Auto"),
                ColumnSpacing = 12,
                Children = { statusText, queryControls },
            },
        };
        Grid.SetColumn(queryControls, 1);
        var filterControls = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,Auto"),
            ColumnDefinitions = ColumnDefinitions.Parse("*,Auto,Auto"),
            ColumnSpacing = 8,
            RowSpacing = 6,
            Children =
            {
                logSearchBox,
                logLevelBox,
                clearLogFilterButton,
                copyDiagnosticsButton,
                saveDiagnosticsButton,
                workspaceLeselangButton,
                editRegistrationButton,
            },
        };
        Grid.SetColumnSpan(logSearchBox, 2);
        Grid.SetColumn(logLevelBox, 2);
        Grid.SetRow(clearLogFilterButton, 1);
        Grid.SetColumn(clearLogFilterButton, 1);
        Grid.SetRow(copyDiagnosticsButton, 1);
        Grid.SetColumn(copyDiagnosticsButton, 2);
        Grid.SetRow(saveDiagnosticsButton, 1);
        Grid.SetRow(workspaceLeselangButton, 2);
        Grid.SetColumnSpan(workspaceLeselangButton, 2);
        Grid.SetRow(editRegistrationButton, 2);
        Grid.SetColumn(editRegistrationButton, 2);
        var logFilter = new Border
        {
            Background = LeserpentTheme.Panel,
            BorderBrush = LeserpentTheme.PanelBorder,
            BorderThickness = new Thickness(0, 0, 0, 1),
            Padding = new Thickness(20, 10),
            Child = new Grid
            {
                RowDefinitions = RowDefinitions.Parse("Auto,Auto,Auto"),
                RowSpacing = 6,
                Children =
                {
                    filterControls,
                    logFilterSummary,
                    diagnosticCopyStatus,
                },
            },
        };
        Grid.SetRow(logFilterSummary, 1);
        Grid.SetRow(diagnosticCopyStatus, 2);
        Content = new Grid
        {
            RowDefinitions = RowDefinitions.Parse("Auto,Auto,*"),
            Children = { status, logFilter, renderer.Surface },
        };
        Grid.SetRow(logFilter, 1);
        Grid.SetRow(renderer.Surface, 2);
        ApplyLocalization();
        this.localization.Changed += OnLocalizationChanged;
        Opened += (_, _) => _ = ReloadAsync();
        Activated += (_, _) =>
        {
            liveRefresh.Activate();
            UpdateLiveRefreshPresentation();
            ScheduleLiveRefresh();
        };
        Deactivated += (_, _) =>
        {
            liveRefreshTimer.Stop();
            liveRefresh.Deactivate();
            UpdateLiveRefreshPresentation();
        };
        KeyDown += (_, eventArgs) =>
        {
            var findModifier = eventArgs.KeyModifiers
                & (KeyModifiers.Control | KeyModifiers.Meta);
            if (eventArgs.Key == Key.F && findModifier != KeyModifiers.None)
            {
                eventArgs.Handled = true;
                logSearchBox.Focus();
                logSearchBox.SelectAll();
            }
            else if (eventArgs.Key == Key.Escape
                && !string.IsNullOrEmpty(logSearchBox.Text))
            {
                eventArgs.Handled = true;
                ClearLogFilter();
            }
            else if (eventArgs.Key == Key.F5 && reloadButton.IsEnabled)
            {
                eventArgs.Handled = true;
                _ = ReloadAsync();
            }
        };
        Closed += (_, _) =>
        {
            this.localization.Changed -= OnLocalizationChanged;
            logFilterTimer.Stop();
            liveRefreshTimer.Stop();
            liveRefresh.Pause();
            workspaceLeselangWindow?.Close();
            registrationWindow?.Close();
            registrationWindow = null;
            lifetime.Cancel();
            client.Dispose();
            leselangClient.Dispose();
            lifetime.Dispose();
        };
    }

    private void OnLocalizationChanged(object? sender, EventArgs eventArgs) =>
        Dispatcher.UIThread.Post(() =>
        {
            _ = sender;
            _ = eventArgs;
            if (lifetime.IsCancellationRequested)
            {
                return;
            }
            ApplyLocalization();
        });

    private void ApplyLocalization()
    {
        applyingLocalization = true;
        try
        {
            FlowDirection = localization.FlowDirection;
            reloadButton.Content = localization.Text(DesktopTextKey.Reload);
            acknowledgeAlertButton.Content = localization.Text(DesktopTextKey.Acknowledge);
            logSearchBox.PlaceholderText =
                localization.Text(DesktopTextKey.SearchSanitizedLogs);
            clearLogFilterButton.Content = localization.Text(DesktopTextKey.Clear);
            copyDiagnosticsButton.Content =
                localization.Text(DesktopTextKey.CopyDiagnostics);
            saveDiagnosticsButton.Content =
                localization.Text(DesktopTextKey.SaveDiagnostics);
            workspaceLeselangButton.Content =
                localization.Text(DesktopTextKey.WorkspaceLeselang);
            editRegistrationButton.Content = DesktopRegistrationCatalogs.Resolve(
                localization,
                "workspace.edit");
            ApplyLogLevelOptions();

            SetAutomationText(
                statusText,
                "a11y.status");
            SetAutomationText(
                reloadButton,
                "a11y.reload",
                "help.reload");
            SetAutomationText(
                acknowledgeAlertButton,
                "a11y.alert_acknowledge",
                "help.alert_acknowledge");
            SetAutomationText(
                logSearchBox,
                "a11y.log_search",
                "help.log_search");
            SetAutomationText(logLevelBox, "a11y.log_level");
            SetAutomationText(clearLogFilterButton, "a11y.clear_filter");
            SetAutomationText(
                copyDiagnosticsButton,
                "a11y.diagnostics_copy",
                "help.diagnostics_copy");
            SetAutomationText(
                saveDiagnosticsButton,
                "a11y.diagnostics_save",
                "help.diagnostics_save");
            SetAutomationText(
                workspaceLeselangButton,
                "a11y.leselang",
                "help.leselang");
            AutomationProperties.SetName(
                editRegistrationButton,
                DesktopRegistrationCatalogs.Resolve(localization, "workspace.edit"));
            AutomationProperties.SetHelpText(
                editRegistrationButton,
                DesktopRegistrationCatalogs.Resolve(
                    localization,
                    "workspace.edit.help"));
            SetAutomationText(diagnosticCopyStatus, "a11y.diagnostic_status");

            if (latestSnapshot is null)
            {
                logFilterSummary.Text =
                    localization.Text(DesktopTextKey.LogsLoadWithSnapshot);
                AutomationProperties.SetName(
                    logFilterSummary,
                    logFilterSummary.Text);
            }
            else
            {
                ApplyLogFilter();
            }
            RefreshStatusText();
            RefreshDiagnosticStatus();
            UpdateLiveRefreshPresentation();
            UpdateSeverityAlertPresentation();
            if (workspaceLeselangWindow is { } preview)
            {
                preview.FlowDirection = localization.FlowDirection;
                preview.Title = DesktopRuntimeWorkspaceCatalogs.Format(
                    localization,
                    "title.leselang",
                    runtimeName);
            }
        }
        finally
        {
            applyingLocalization = false;
        }
    }

    private void ApplyLogLevelOptions()
    {
        var selected = SelectedLogLevel;
        var options = RemoteWorkspaceLogFilter.Levels.Select(level =>
            new LogLevelOption(
                level,
                DesktopRuntimeWorkspacePresentation.LogLevel(level)
                    .Resolve(localization)))
            .ToArray();
        logLevelBox.ItemsSource = options;
        logLevelBox.SelectedItem = options.Single(option => option.Value == selected);
    }

    private string SelectedLogLevel =>
        (logLevelBox.SelectedItem as LogLevelOption)?.Value
        ?? RemoteWorkspaceLogFilter.AllLevels;

    private void SetAutomationText(
        Control control,
        string nameKey,
        string? helpKey = null)
    {
        AutomationProperties.SetName(
            control,
            DesktopRuntimeWorkspaceCatalogs.Resolve(localization, nameKey));
        if (helpKey is not null)
        {
            AutomationProperties.SetHelpText(
                control,
                DesktopRuntimeWorkspaceCatalogs.Resolve(localization, helpKey));
        }
    }

    internal bool OwnsActionSource(AvaloniaDocumentRenderer source) =>
        ReferenceEquals(renderer, source);

    public void VerifyLayoutEnvelope()
    {
        if (Content is not Control root)
        {
            throw new InvalidDataException(
                "runtime workspace has no control root");
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
                    "runtime workspace controls exceeded their layout envelope");
            }
        }
    }

    public void ProbeLocalizedPresentation()
    {
        var snapshot = new RemoteWorkspaceSnapshot(
            42,
            new RemoteRuntimeProjection
            {
                Id = RuntimeId,
                Name = runtimeName,
                Revision = 42,
                RefreshStatus = RefreshStatus.Ready,
                Tags = new RuntimeTags { Environment = "verification" },
                Status = new RuntimeStatusSnapshot
                {
                    StatusSource = "verification",
                },
            },
            [new RemoteHistoryProjection("command-verification", 42, "applied")],
            [
                new RemoteLogProjection(1, "info", "listener ready"),
                new RemoteLogProjection(2, "warning", "bounded warning"),
            ]);
        var change = new RemoteWorkspaceSnapshotChange(
            false,
            1,
            1,
            0,
            1,
            0,
            0,
            1,
            0,
            false);
        _ = severityAlert.Acknowledge();
        _ = severityAlert.Observe(snapshot.Revision, change);
        latestSnapshot = snapshot;
        loadedRevision = snapshot.Revision;
        logSearchBox.Text = "warning";
        ApplyLogFilter();
        UpdateSeverityAlertPresentation();
        SetDiagnosticCopyStatus(
            DesktopRuntimeWorkspacePresentation.Text("diagnostic.copied"),
            failed: false);
        SetStatus(
            DesktopRuntimeWorkspacePresentation.Loaded(
                snapshot.Revision,
                liveRequested: true,
                incremental: true,
                change,
                severityAlert),
            LeserpentTheme.Primary);

        var selected = logLevelBox.SelectedItem as LogLevelOption;
        if (reloadButton.Content as string
                != localization.Text(DesktopTextKey.Reload)
            || logSearchBox.PlaceholderText
                != localization.Text(DesktopTextKey.SearchSanitizedLogs)
            || selected?.Label
                != DesktopRuntimeWorkspacePresentation
                    .LogLevel(RemoteWorkspaceLogFilter.AllLevels)
                    .Resolve(localization)
            || logFilterSummary.Text
                != DesktopRuntimeWorkspaceCatalogs.Format(
                    localization,
                    "filter.some",
                    1,
                    2)
            || diagnosticCopyStatus.Text
                != DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "diagnostic.copied")
            || editRegistrationButton.Content as string
                != DesktopRegistrationCatalogs.Resolve(
                    localization,
                    "workspace.edit")
            || statusText.Text?.Contains(
                DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "snapshot.incremental"),
                StringComparison.Ordinal) != true
            || AutomationProperties.GetName(reloadButton)
                != DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "a11y.reload")
            || AutomationProperties.GetHelpText(copyDiagnosticsButton)
                != DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "help.diagnostics_copy")
            || AutomationProperties.GetHelpText(editRegistrationButton)
                != DesktopRegistrationCatalogs.Resolve(
                    localization,
                    "workspace.edit.help"))
        {
            throw new InvalidDataException(
                "runtime workspace localization did not reach native controls");
        }
    }

    private void ConfigureLiveRefresh()
    {
        AutomationProperties.SetAutomationId(
            liveRefreshButton,
            "runtime-workspace-live-logs");
        liveRefreshButton.Click += (_, _) => ToggleLiveRefresh();
        liveRefreshTimer.Tick += (_, _) =>
        {
            liveRefreshTimer.Stop();
            _ = RunLiveRefreshAsync();
        };
        UpdateLiveRefreshPresentation();
    }

    private void ConfigureSeverityAlert()
    {
        AutomationProperties.SetAutomationId(
            acknowledgeAlertButton,
            "runtime-workspace-alert-acknowledge");
        acknowledgeAlertButton.Click += (_, _) => AcknowledgeSeverityAlert();
        UpdateSeverityAlertPresentation();
    }

    private void AcknowledgeSeverityAlert()
    {
        if (!severityAlert.Acknowledge())
        {
            return;
        }
        UpdateSeverityAlertPresentation();
        SetStatus(
            DesktopRuntimeWorkspacePresentation.Text(
                "status.alert_acknowledged",
                loadedRevision),
            LeserpentTheme.Accent);
    }

    private void UpdateSeverityAlertPresentation()
    {
        acknowledgeAlertButton.IsVisible = severityAlert.IsPending;
        acknowledgeAlertButton.IsEnabled = severityAlert.IsPending;
        ToolTip.SetTip(
            acknowledgeAlertButton,
            severityAlert.IsPending
                ? DesktopRuntimeWorkspacePresentation.Alert(severityAlert)
                    .Resolve(localization)
                : DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "alert.none"));
    }

    private void ToggleLiveRefresh()
    {
        liveRefreshTimer.Stop();
        if (liveRefresh.IsRequested)
        {
            liveRefresh.Pause();
            UpdateLiveRefreshPresentation();
            if (latestSnapshot is not null)
            {
                SetStatus(
                    DesktopRuntimeWorkspacePresentation.Text(
                        "status.live_paused",
                        loadedRevision),
                    LeserpentTheme.Muted);
            }
            return;
        }
        liveRefresh.Start(IsActive);
        UpdateLiveRefreshPresentation();
        if (liveRefresh.ShouldSchedule)
        {
            _ = RunLiveRefreshAsync();
        }
    }

    private async Task RunLiveRefreshAsync()
    {
        liveRefreshTimer.Stop();
        if (!liveRefresh.TryBegin())
        {
            return;
        }
        UpdateLiveRefreshPresentation();
        try
        {
            var outcome = await ReloadAsync(allowIncrementalLogs: true);
            if (outcome == WorkspaceReloadOutcome.Closed)
            {
                liveRefresh.Pause();
            }
            else if (outcome == WorkspaceReloadOutcome.Skipped)
            {
                liveRefresh.Defer(IsActive);
            }
            else
            {
                liveRefresh.Complete(
                    outcome == WorkspaceReloadOutcome.Loaded,
                    IsActive);
                if (outcome == WorkspaceReloadOutcome.Failed)
                {
                    ShowLiveRefreshFailure();
                }
            }
        }
        catch (Exception)
        {
            liveRefresh.Complete(succeeded: false, IsActive);
            if (!lifetime.IsCancellationRequested)
            {
                ShowLiveRefreshFailure(unexpected: true);
            }
        }
        UpdateLiveRefreshPresentation();
        ScheduleLiveRefresh();
    }

    private void ScheduleLiveRefresh()
    {
        liveRefreshTimer.Stop();
        if (liveRefresh.ShouldSchedule && !lifetime.IsCancellationRequested)
        {
            liveRefreshTimer.Interval = liveRefresh.NextInterval;
            liveRefreshTimer.Start();
        }
    }

    private void UpdateLiveRefreshPresentation()
    {
        liveRefreshButton.IsEnabled = liveRefresh.IsRequested || !loadInFlight;
        liveRefreshButton.Content = liveRefresh.IsRequested
            ? DesktopRuntimeWorkspaceCatalogs.Resolve(localization, "action.pause_live")
            : localization.Text(DesktopTextKey.LiveLogs);
        var description = DesktopRuntimeWorkspacePresentation
            .LiveDescription(liveRefresh)
            .Resolve(localization);
        AutomationProperties.SetName(
            liveRefreshButton,
            DesktopRuntimeWorkspaceCatalogs.Resolve(
                localization,
                liveRefresh.IsRequested ? "a11y.live_pause" : "a11y.live_start"));
        AutomationProperties.SetHelpText(liveRefreshButton, description);
        ToolTip.SetTip(liveRefreshButton, description);
    }

    private void ConfigureLogFilter()
    {
        AutomationProperties.SetAutomationId(logSearchBox, "runtime-log-search");
        AutomationProperties.SetAutomationId(logLevelBox, "runtime-log-level");
        AutomationProperties.SetAutomationId(clearLogFilterButton, "runtime-log-filter-clear");
        AutomationProperties.SetAutomationId(logFilterSummary, "runtime-log-filter-summary");
        AutomationProperties.SetName(logFilterSummary, logFilterSummary.Text);
        AutomationProperties.SetLiveSetting(logFilterSummary, AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(
            copyDiagnosticsButton,
            "runtime-diagnostics-copy");
        AutomationProperties.SetAutomationId(
            saveDiagnosticsButton,
            "runtime-diagnostics-save");
        AutomationProperties.SetAutomationId(
            workspaceLeselangButton,
            "runtime-workspace-leselang");
        AutomationProperties.SetAutomationId(
            editRegistrationButton,
            "runtime-registration-edit");
        AutomationProperties.SetAutomationId(
            diagnosticCopyStatus,
            "runtime-diagnostics-copy-status");
        AutomationProperties.SetLiveSetting(
            diagnosticCopyStatus,
            AutomationLiveSetting.Polite);
        logSearchBox.TextChanged += (_, _) =>
        {
            var raw = logSearchBox.Text ?? string.Empty;
            var sanitized = new string(raw
                .Where(character => !char.IsControl(character))
                .Take(RemoteWorkspaceLogFilter.MaxQueryLength)
                .ToArray());
            if (!string.Equals(raw, sanitized, StringComparison.Ordinal))
            {
                logSearchBox.Text = sanitized;
                logSearchBox.CaretIndex = sanitized.Length;
                return;
            }
            QueueLogFilter();
        };
        logLevelBox.SelectionChanged += (_, _) =>
        {
            if (!applyingLocalization)
            {
                QueueLogFilter();
            }
        };
        clearLogFilterButton.Click += (_, _) => ClearLogFilter();
        copyDiagnosticsButton.Click += (_, _) => _ = CopyDiagnosticsAsync();
        saveDiagnosticsButton.Click += (_, _) => _ = SaveDiagnosticsAsync();
        workspaceLeselangButton.Click += (_, _) => ShowWorkspaceLeselang();
        editRegistrationButton.Click += (_, _) => _ = EditRegistrationAsync();
        logFilterTimer.Tick += (_, _) =>
        {
            logFilterTimer.Stop();
            ApplyLogFilter();
        };
    }

    private void QueueLogFilter()
    {
        SetDiagnosticCopyStatus(null, failed: false);
        clearLogFilterButton.IsVisible =
            !string.IsNullOrWhiteSpace(logSearchBox.Text)
            || logLevelBox.SelectedIndex > 0;
        logFilterTimer.Stop();
        logFilterTimer.Start();
    }

    private void ClearLogFilter()
    {
        logFilterTimer.Stop();
        logSearchBox.Text = string.Empty;
        logLevelBox.SelectedIndex = 0;
        logFilterTimer.Stop();
        clearLogFilterButton.IsVisible = false;
        ApplyLogFilter();
        logSearchBox.Focus();
    }

    private void ApplyLogFilter()
    {
        if (latestSnapshot is null)
        {
            return;
        }
        var view = RemoteWorkspaceLogFilter.Apply(
            latestSnapshot,
            logSearchBox.Text,
            SelectedLogLevel);
        renderer.Mount(RemoteWorkspaceDocumentProjection.Project(
            view.Snapshot,
            view.IsActive));
        logFilterSummary.Text = DesktopRuntimeWorkspacePresentation
            .LogFilterSummary(view)
            .Resolve(localization);
        AutomationProperties.SetName(
            logFilterSummary,
            DesktopRuntimeWorkspaceCatalogs.Format(
                localization,
                "a11y.filter_summary",
                logFilterSummary.Text));
        copyDiagnosticsButton.IsEnabled = true;
        saveDiagnosticsButton.IsEnabled = !diagnosticSaveInFlight;
    }

    private async Task CopyDiagnosticsAsync()
    {
        var snapshot = latestSnapshot;
        var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
        if (snapshot is null || clipboard is null)
        {
            SetDiagnosticCopyStatus(
                DesktopRuntimeWorkspacePresentation.Text(
                    "diagnostic.clipboard_unavailable"),
                failed: true);
            return;
        }
        try
        {
            var view = RemoteWorkspaceLogFilter.Apply(
                snapshot,
                logSearchBox.Text,
                SelectedLogLevel);
            await clipboard.SetTextAsync(RemoteWorkspaceDiagnosticExport.Create(view));
            if (!lifetime.IsCancellationRequested)
            {
                SetDiagnosticCopyStatus(
                    DesktopRuntimeWorkspacePresentation.Text("diagnostic.copied"),
                    failed: false);
            }
        }
        catch (Exception)
        {
            if (!lifetime.IsCancellationRequested)
            {
                SetDiagnosticCopyStatus(
                    DesktopRuntimeWorkspacePresentation.Text(
                        "diagnostic.copy_failed"),
                    failed: true);
            }
        }
    }

    private async Task SaveDiagnosticsAsync()
    {
        var snapshot = latestSnapshot;
        var storage = TopLevel.GetTopLevel(this)?.StorageProvider;
        if (snapshot is null || storage is null || !storage.CanSave)
        {
            SetDiagnosticCopyStatus(
                DesktopRuntimeWorkspacePresentation.Text(
                    "diagnostic.save_unavailable"),
                failed: true);
            return;
        }
        if (diagnosticSaveInFlight)
        {
            return;
        }
        diagnosticSaveInFlight = true;
        saveDiagnosticsButton.IsEnabled = false;
        try
        {
            var view = RemoteWorkspaceLogFilter.Apply(
                snapshot,
                logSearchBox.Text,
                SelectedLogLevel);
            var content = RemoteWorkspaceDiagnosticExport.Encode(view);
            var fileType = new FilePickerFileType(
                DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "file.diagnostic_text"))
            {
                Patterns = ["*.txt"],
                MimeTypes = ["text/plain"],
                AppleUniformTypeIdentifiers = ["public.plain-text"],
            };
            var file = await storage.SaveFilePickerAsync(new FilePickerSaveOptions
            {
                Title = DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "file.save_title"),
                SuggestedFileName = RemoteWorkspaceDiagnosticExport.SuggestedFileName(snapshot),
                DefaultExtension = "txt",
                ShowOverwritePrompt = true,
                FileTypeChoices = [fileType],
            });
            if (file is null || lifetime.IsCancellationRequested)
            {
                if (file is null && !lifetime.IsCancellationRequested)
                {
                    SetDiagnosticCopyStatus(
                        DesktopRuntimeWorkspacePresentation.Text(
                            "diagnostic.save_cancelled"),
                        failed: false);
                }
                return;
            }
            await using var stream = await file.OpenWriteAsync();
            if (!stream.CanWrite || !stream.CanSeek)
            {
                throw new IOException("selected diagnostic destination is not replaceable");
            }
            stream.SetLength(0);
            stream.Position = 0;
            await stream.WriteAsync(content, lifetime.Token);
            await stream.FlushAsync(lifetime.Token);
            if (!lifetime.IsCancellationRequested)
            {
                SetDiagnosticCopyStatus(
                    DesktopRuntimeWorkspacePresentation.Text("diagnostic.saved"),
                    failed: false);
            }
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            // Window shutdown owns this cancellation.
        }
        catch (Exception)
        {
            if (!lifetime.IsCancellationRequested)
            {
                SetDiagnosticCopyStatus(
                    DesktopRuntimeWorkspacePresentation.Text(
                        "diagnostic.save_failed"),
                    failed: true);
            }
        }
        finally
        {
            diagnosticSaveInFlight = false;
            saveDiagnosticsButton.IsEnabled = latestSnapshot is not null
                && !lifetime.IsCancellationRequested;
        }
    }

    private async Task EditRegistrationAsync()
    {
        if (registrationWindow is not null)
        {
            registrationWindow.Activate();
            return;
        }
        if (latestSnapshot is null || lifetime.IsCancellationRequested)
        {
            return;
        }
        var window = new RuntimeRegistrationWindow(
            options,
            principal,
            localization,
            RuntimeId);
        window.SetMutationAvailability(registrationMutationEnabled);
        registrationWindow = window;
        UpdateRegistrationAvailability();
        window.Closed += (_, _) =>
        {
            if (ReferenceEquals(registrationWindow, window))
            {
                registrationWindow = null;
                UpdateRegistrationAvailability();
            }
        };
        try
        {
            var result = await window.ShowDialog<RemoteRegistrationResult?>(this);
            if (result is not null && !lifetime.IsCancellationRequested)
            {
                ReloadIfOlder(result.Revision);
            }
        }
        catch (Exception) when (lifetime.IsCancellationRequested)
        {
            // Closing the workspace invalidates the owned registration dialog.
        }
    }

    private void UpdateRegistrationAvailability() =>
        editRegistrationButton.IsEnabled = latestSnapshot is not null
            && !loadInFlight
            && registrationWindow is null
            && registrationMutationEnabled
            && !lifetime.IsCancellationRequested;

    private void ShowWorkspaceLeselang()
    {
        if (workspaceLeselangWindow is not null)
        {
            workspaceLeselangWindow.Activate();
            return;
        }
        var preview = new Window
        {
            Title = DesktopRuntimeWorkspaceCatalogs.Format(
                localization,
                "title.leselang",
                runtimeName),
            Width = 640,
            Height = 360,
            MinWidth = 480,
            MinHeight = 280,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = LeserpentTheme.Canvas,
            FlowDirection = localization.FlowDirection,
            Content = new Border
            {
                Padding = new Thickness(20),
                Child = new LeselangExportControl(
                    "runtime-workspace-query",
                    cancellationToken => leselangClient.ExportWorkspaceAsync(
                        RuntimeId,
                        cancellationToken),
                    localization),
            },
        };
        preview.Closed += (_, _) =>
        {
            if (ReferenceEquals(workspaceLeselangWindow, preview))
            {
                workspaceLeselangWindow = null;
            }
        };
        workspaceLeselangWindow = preview;
        preview.Show(this);
    }

    private void SetDiagnosticCopyStatus(
        DesktopRuntimeWorkspaceText? value,
        bool failed)
    {
        diagnosticStatus = value;
        RefreshDiagnosticStatus();
        diagnosticCopyStatus.IsVisible = value is not null;
        diagnosticCopyStatus.Foreground = failed
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Accent;
        AutomationProperties.SetLiveSetting(
            diagnosticCopyStatus,
            failed ? AutomationLiveSetting.Assertive : AutomationLiveSetting.Polite);
    }

    private void RefreshDiagnosticStatus()
    {
        diagnosticCopyStatus.Text = diagnosticStatus?.Resolve(localization)
            ?? string.Empty;
        AutomationProperties.SetName(
            diagnosticCopyStatus,
            diagnosticStatus is null
                ? DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "a11y.diagnostic_status")
                : diagnosticCopyStatus.Text);
    }

    public string RuntimeId { get; }

    public void SetRefreshAvailability(bool enabled, string? reason)
    {
        renderer.SetActionAvailability(ActionKind.RuntimeRefresh, enabled, reason);
        renderer.SetActionAvailability(
            ActionKind.RuntimeCapabilitiesRefresh,
            enabled,
            reason);
        renderer.SetActionAvailability(ActionKind.RuntimeDeploy, enabled, reason);
        registrationMutationEnabled = enabled;
        registrationWindow?.SetMutationAvailability(enabled);
        UpdateRegistrationAvailability();
    }

    public void ReloadIfOlder(ulong revision)
    {
        desiredRevision = Math.Max(desiredRevision, revision);
        if (revision > loadedRevision && !loadInFlight)
        {
            _ = ReloadAsync();
        }
    }

    private async Task<WorkspaceReloadOutcome> ReloadAsync(
        bool allowIncrementalLogs = false)
    {
        if (lifetime.IsCancellationRequested)
        {
            return WorkspaceReloadOutcome.Closed;
        }
        if (loadInFlight)
        {
            return WorkspaceReloadOutcome.Skipped;
        }
        liveRefreshTimer.Stop();
        loadInFlight = true;
        var loaded = false;
        var outcome = WorkspaceReloadOutcome.Failed;
        reloadButton.IsEnabled = false;
        UpdateRegistrationAvailability();
        SetStatus(
            DesktopRuntimeWorkspacePresentation.Text("status.loading"),
            LeserpentTheme.Primary);
        try
        {
            var previous = latestSnapshot;
            var afterLogSequence = logRefreshPlan.SelectCursor(
                allowIncrementalLogs,
                previous);
            var snapshot = await client.LoadAsync(
                RuntimeId,
                principal,
                afterLogSequence,
                lifetime.Token);
            var usedIncrementalLogs = afterLogSequence.HasValue;
            if (usedIncrementalLogs
                && RemoteWorkspaceLogRefreshPlan.RequiresFullFallback(
                    previous!,
                    snapshot))
            {
                snapshot = await client.LoadAsync(
                    RuntimeId,
                    principal,
                    cancellationToken: lifetime.Token);
                usedIncrementalLogs = false;
            }
            else if (usedIncrementalLogs)
            {
                snapshot = RemoteWorkspaceCodec.MergeIncrementalLogs(previous!, snapshot);
            }
            var change = RemoteWorkspaceSnapshotChanges.Compare(latestSnapshot, snapshot);
            latestSnapshot = snapshot;
            ApplyLogFilter();
            loadedRevision = snapshot.Revision;
            loaded = true;
            outcome = WorkspaceReloadOutcome.Loaded;
            logRefreshPlan.RecordSuccess(usedIncrementalLogs);
            if (!allowIncrementalLogs)
            {
                _ = liveRefresh.RecoverAfterExternalSuccess();
            }
            _ = severityAlert.Observe(snapshot.Revision, change);
            UpdateSeverityAlertPresentation();
            var statusBrush = severityAlert.Level == WorkspaceSeverityAlertLevel.Error
                ? LeserpentTheme.Destructive
                : severityAlert.Level == WorkspaceSeverityAlertLevel.Warning
                    ? LeserpentTheme.Primary
                    : LeserpentTheme.Accent;
            SetStatus(
                DesktopRuntimeWorkspacePresentation.Loaded(
                    snapshot.Revision,
                    liveRefresh.IsRequested,
                    usedIncrementalLogs,
                    change,
                    severityAlert),
                statusBrush,
                assertive: change.NewErrors > 0);
        }
        catch (RemoteQueryException error)
        {
            ShowFailure(DesktopRuntimeWorkspacePresentation.QueryFailure(
                DesktopRuntimeWorkspaceQueryFailure.RemoteRejected,
                Safe(error.Code),
                Safe(error.Message)));
        }
        catch (InvalidDataException error)
        {
            ShowFailure(DesktopRuntimeWorkspacePresentation.QueryFailure(
                DesktopRuntimeWorkspaceQueryFailure.InvalidResponse,
                Safe(error.Message)));
        }
        catch (ArgumentException error)
        {
            ShowFailure(DesktopRuntimeWorkspacePresentation.QueryFailure(
                DesktopRuntimeWorkspaceQueryFailure.Blocked,
                Safe(error.Message)));
        }
        catch (OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            ShowFailure(DesktopRuntimeWorkspacePresentation.QueryFailure(
                DesktopRuntimeWorkspaceQueryFailure.Timeout));
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            // Window shutdown owns this cancellation.
            outcome = WorkspaceReloadOutcome.Closed;
        }
        catch (HttpRequestException)
        {
            ShowFailure(DesktopRuntimeWorkspacePresentation.QueryFailure(
                DesktopRuntimeWorkspaceQueryFailure.Transport));
        }
        catch (ObjectDisposedException) when (lifetime.IsCancellationRequested)
        {
            // The HTTP client may be disposed while the window is closing.
            outcome = WorkspaceReloadOutcome.Closed;
        }
        finally
        {
            loadInFlight = false;
            reloadButton.IsEnabled = !lifetime.IsCancellationRequested;
            UpdateRegistrationAvailability();
            UpdateLiveRefreshPresentation();
            if (loaded && desiredRevision > loadedRevision)
            {
                _ = ReloadAsync();
            }
            else if (!allowIncrementalLogs)
            {
                ScheduleLiveRefresh();
            }
        }
        return outcome;
    }

    private void ShowLiveRefreshFailure(bool unexpected = false)
    {
        if (liveRefresh.IsRequested)
        {
            SetStatus(
                DesktopRuntimeWorkspacePresentation.LiveFailure(
                    liveRefresh,
                    unexpected),
                LeserpentTheme.Destructive,
                assertive: true);
            return;
        }
        SetStatus(
            DesktopRuntimeWorkspacePresentation.LiveFailure(
                liveRefresh,
                unexpected),
            LeserpentTheme.Destructive,
            assertive: true);
    }

    private void ShowFailure(DesktopRuntimeWorkspaceText message)
    {
        SetStatus(message, LeserpentTheme.Destructive, assertive: true);
    }

    private void SetStatus(
        DesktopRuntimeWorkspaceText text,
        IBrush foreground,
        bool assertive = false)
    {
        currentStatus = text;
        RefreshStatusText();
        statusText.Foreground = foreground;
        AutomationProperties.SetLiveSetting(
            statusText,
            assertive
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
    }

    private void RefreshStatusText()
    {
        statusText.Text = currentStatus?.Resolve(localization) ?? string.Empty;
        AutomationProperties.SetName(
            statusText,
            currentStatus is null
                ? DesktopRuntimeWorkspaceCatalogs.Resolve(
                    localization,
                    "a11y.status")
                : DesktopRuntimeWorkspaceCatalogs.Format(
                    localization,
                    "a11y.status_value",
                    statusText.Text));
    }

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
