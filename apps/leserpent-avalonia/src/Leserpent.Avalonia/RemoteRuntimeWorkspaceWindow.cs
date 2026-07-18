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
    private readonly CancellationTokenSource lifetime = new();
    private readonly string principal;
    private readonly TextBlock statusText = new()
    {
        FontSize = 13,
        FontWeight = FontWeight.SemiBold,
        TextWrapping = TextWrapping.Wrap,
    };
    private readonly Button reloadButton = new()
    {
        Content = "Reload",
        Padding = new Thickness(12, 6),
    };
    private readonly Button liveRefreshButton = new()
    {
        Content = "Live logs",
        Padding = new Thickness(12, 6),
    };
    private readonly Button acknowledgeAlertButton = new()
    {
        Content = "Acknowledge",
        Padding = new Thickness(12, 6),
        IsVisible = false,
    };
    private readonly TextBox logSearchBox = new()
    {
        PlaceholderText = "Search sanitized logs",
        MaxLength = RemoteWorkspaceLogFilter.MaxQueryLength,
        MinWidth = 180,
    };
    private readonly ComboBox logLevelBox = new()
    {
        ItemsSource = RemoteWorkspaceLogFilter.Levels,
        SelectedIndex = 0,
        MinWidth = 112,
    };
    private readonly Button clearLogFilterButton = new()
    {
        Content = "Clear",
        Padding = new Thickness(12, 6),
        IsVisible = false,
    };
    private readonly Button copyDiagnosticsButton = new()
    {
        Content = "Copy diagnostics",
        Padding = new Thickness(12, 6),
        IsEnabled = false,
    };
    private readonly Button saveDiagnosticsButton = new()
    {
        Content = "Save diagnostics",
        Padding = new Thickness(12, 6),
        IsEnabled = false,
    };
    private readonly Button workspaceLeselangButton = new()
    {
        Content = "Workspace Leselang",
        Padding = new Thickness(12, 6),
        HorizontalAlignment = Avalonia.Layout.HorizontalAlignment.Left,
    };
    private readonly TextBlock logFilterSummary = new()
    {
        FontSize = 12,
        Foreground = LeserpentTheme.Muted,
        Text = "Logs load with the runtime snapshot",
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
    private Window? workspaceLeselangWindow;
    private ulong loadedRevision;
    private ulong desiredRevision;
    private RemoteWorkspaceSnapshot? latestSnapshot;

    public RemoteRuntimeWorkspaceWindow(
        RemoteClientOptions options,
        RemoteRuntimeProjection runtime,
        string principal,
        Action<string> actionInvoked)
    {
        RuntimeId = runtime.Id;
        this.principal = principal;
        client = new RemoteWorkspaceClient(options);
        renderer = new AvaloniaDocumentRenderer(actionInvoked);
        Width = 760;
        Height = 620;
        MinWidth = 520;
        MinHeight = 420;
        Background = LeserpentTheme.Canvas;
        FontFamily = new FontFamily("Avenir Next, Segoe UI, sans-serif");
        Title = $"{Safe(runtime.Name)} / Leserpent";
        AutomationProperties.SetAutomationId(statusText, "runtime-workspace-status");
        AutomationProperties.SetName(statusText, "Runtime workspace query status");
        AutomationProperties.SetLiveSetting(statusText, AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(reloadButton, "runtime-workspace-reload");
        AutomationProperties.SetName(reloadButton, "Reload runtime workspace");
        AutomationProperties.SetHelpText(
            reloadButton,
            "Reloads status, history, and logs through one revision-consistent query group. Shortcut: F5.");
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
        Grid.SetColumnSpan(workspaceLeselangButton, 3);
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
            logFilterTimer.Stop();
            liveRefreshTimer.Stop();
            liveRefresh.Pause();
            workspaceLeselangWindow?.Close();
            lifetime.Cancel();
            client.Dispose();
            lifetime.Dispose();
        };
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
        AutomationProperties.SetName(
            acknowledgeAlertButton,
            "Acknowledge runtime workspace alert");
        AutomationProperties.SetHelpText(
            acknowledgeAlertButton,
            "Clears the retained severity alert without changing logs, filters, or live refresh.");
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
            $"Workspace alert acknowledged at revision {loadedRevision}",
            LeserpentTheme.Accent);
    }

    private void UpdateSeverityAlertPresentation()
    {
        acknowledgeAlertButton.IsVisible = severityAlert.IsPending;
        acknowledgeAlertButton.IsEnabled = severityAlert.IsPending;
        ToolTip.SetTip(
            acknowledgeAlertButton,
            severityAlert.IsPending
                ? severityAlert.Describe()
                : "No runtime workspace alert is awaiting acknowledgement.");
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
                    $"Live logs paused at revision {loadedRevision}",
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
            ? "Pause live"
            : "Live logs";
        var description = liveRefresh.State switch
        {
            WorkspaceLiveRefreshState.Waiting =>
                liveRefresh.ConsecutiveFailures == 0
                    ? "Live logs enabled; the next revision-consistent query runs within five seconds."
                    : $"Live logs recovering after {liveRefresh.ConsecutiveFailures} failed query; the next attempt runs within {liveRefresh.NextInterval.TotalSeconds:0} seconds.",
            WorkspaceLiveRefreshState.Refreshing =>
                "Live logs enabled; one authenticated query group is in progress.",
            WorkspaceLiveRefreshState.Suspended =>
                "Live logs paused while this window is inactive.",
            _ => "Starts explicit five-second live log refresh. No overlapping query is allowed.",
        };
        AutomationProperties.SetName(
            liveRefreshButton,
            liveRefresh.IsRequested ? "Pause live runtime logs" : "Start live runtime logs");
        AutomationProperties.SetHelpText(liveRefreshButton, description);
        ToolTip.SetTip(liveRefreshButton, description);
    }

    private void ConfigureLogFilter()
    {
        AutomationProperties.SetAutomationId(logSearchBox, "runtime-log-search");
        AutomationProperties.SetName(logSearchBox, "Search runtime logs");
        AutomationProperties.SetHelpText(
            logSearchBox,
            "Filters the loaded sanitized log display locally. Shortcut: Control or Command plus F.");
        AutomationProperties.SetAutomationId(logLevelBox, "runtime-log-level");
        AutomationProperties.SetName(logLevelBox, "Runtime log level filter");
        AutomationProperties.SetAutomationId(clearLogFilterButton, "runtime-log-filter-clear");
        AutomationProperties.SetName(clearLogFilterButton, "Clear runtime log filters");
        AutomationProperties.SetAutomationId(logFilterSummary, "runtime-log-filter-summary");
        AutomationProperties.SetName(logFilterSummary, logFilterSummary.Text);
        AutomationProperties.SetLiveSetting(logFilterSummary, AutomationLiveSetting.Polite);
        AutomationProperties.SetAutomationId(
            copyDiagnosticsButton,
            "runtime-diagnostics-copy");
        AutomationProperties.SetName(
            copyDiagnosticsButton,
            "Copy visible runtime diagnostics");
        AutomationProperties.SetHelpText(
            copyDiagnosticsButton,
            "Copies endpoint-free workspace metadata, command history, and currently visible sanitized logs. Review before sharing.");
        AutomationProperties.SetAutomationId(
            saveDiagnosticsButton,
            "runtime-diagnostics-save");
        AutomationProperties.SetName(
            saveDiagnosticsButton,
            "Save visible runtime diagnostics");
        AutomationProperties.SetHelpText(
            saveDiagnosticsButton,
            "Opens the system save panel for an endpoint-free bounded text export. Review the selected destination and file before sharing.");
        AutomationProperties.SetAutomationId(
            workspaceLeselangButton,
            "runtime-workspace-leselang");
        AutomationProperties.SetName(
            workspaceLeselangButton,
            "Preview equivalent workspace Leselang");
        AutomationProperties.SetHelpText(
            workspaceLeselangButton,
            "Opens canonical Leselang for the same inspect, history, and logs query group without executing it.");
        AutomationProperties.SetAutomationId(
            diagnosticCopyStatus,
            "runtime-diagnostics-copy-status");
        AutomationProperties.SetName(diagnosticCopyStatus, "Diagnostic copy status");
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
        logLevelBox.SelectionChanged += (_, _) => QueueLogFilter();
        clearLogFilterButton.Click += (_, _) => ClearLogFilter();
        copyDiagnosticsButton.Click += (_, _) => _ = CopyDiagnosticsAsync();
        saveDiagnosticsButton.Click += (_, _) => _ = SaveDiagnosticsAsync();
        workspaceLeselangButton.Click += (_, _) => ShowWorkspaceLeselang();
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
            logLevelBox.SelectedItem as string);
        renderer.Mount(RemoteWorkspaceDocumentProjection.Project(
            view.Snapshot,
            view.IsActive));
        logFilterSummary.Text = view.IsActive
            ? $"Showing {view.VisibleLogCount} of {view.TotalLogCount} logs"
            : $"Showing all {view.TotalLogCount} logs";
        AutomationProperties.SetName(
            logFilterSummary,
            $"Runtime log filter: {logFilterSummary.Text}");
        copyDiagnosticsButton.IsEnabled = true;
        saveDiagnosticsButton.IsEnabled = !diagnosticSaveInFlight;
    }

    private async Task CopyDiagnosticsAsync()
    {
        var snapshot = latestSnapshot;
        var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
        if (snapshot is null || clipboard is null)
        {
            SetDiagnosticCopyStatus("Clipboard unavailable.", failed: true);
            return;
        }
        try
        {
            var view = RemoteWorkspaceLogFilter.Apply(
                snapshot,
                logSearchBox.Text,
                logLevelBox.SelectedItem as string);
            await clipboard.SetTextAsync(RemoteWorkspaceDiagnosticExport.Create(view));
            if (!lifetime.IsCancellationRequested)
            {
                SetDiagnosticCopyStatus(
                    "Visible diagnostic snapshot copied. Review it before sharing.",
                    failed: false);
            }
        }
        catch (Exception)
        {
            if (!lifetime.IsCancellationRequested)
            {
                SetDiagnosticCopyStatus("Diagnostic copy failed safely.", failed: true);
            }
        }
    }

    private async Task SaveDiagnosticsAsync()
    {
        var snapshot = latestSnapshot;
        var storage = TopLevel.GetTopLevel(this)?.StorageProvider;
        if (snapshot is null || storage is null || !storage.CanSave)
        {
            SetDiagnosticCopyStatus("System file saving is unavailable.", failed: true);
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
                logLevelBox.SelectedItem as string);
            var content = RemoteWorkspaceDiagnosticExport.Encode(view);
            var fileType = new FilePickerFileType("Leserpent diagnostic text")
            {
                Patterns = ["*.txt"],
                MimeTypes = ["text/plain"],
                AppleUniformTypeIdentifiers = ["public.plain-text"],
            };
            var file = await storage.SaveFilePickerAsync(new FilePickerSaveOptions
            {
                Title = "Save runtime diagnostics",
                SuggestedFileName = RemoteWorkspaceDiagnosticExport.SuggestedFileName(snapshot),
                DefaultExtension = "txt",
                ShowOverwritePrompt = true,
                FileTypeChoices = [fileType],
            });
            if (file is null || lifetime.IsCancellationRequested)
            {
                if (file is null && !lifetime.IsCancellationRequested)
                {
                    SetDiagnosticCopyStatus("Diagnostic save canceled.", failed: false);
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
                    "Diagnostic snapshot saved. Review the file before sharing.",
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
                SetDiagnosticCopyStatus("Diagnostic save failed safely.", failed: true);
            }
        }
        finally
        {
            diagnosticSaveInFlight = false;
            saveDiagnosticsButton.IsEnabled = latestSnapshot is not null
                && !lifetime.IsCancellationRequested;
        }
    }

    private void ShowWorkspaceLeselang()
    {
        if (workspaceLeselangWindow is not null)
        {
            workspaceLeselangWindow.Activate();
            return;
        }
        var preview = new Window
        {
            Title = $"Workspace Leselang / {Title}",
            Width = 640,
            Height = 360,
            MinWidth = 480,
            MinHeight = 280,
            WindowStartupLocation = WindowStartupLocation.CenterOwner,
            Background = LeserpentTheme.Canvas,
            Content = new Border
            {
                Padding = new Thickness(20),
                Child = new LeselangExportControl(
                    "runtime-workspace-query",
                    RemoteLeselangExport.Workspace(RuntimeId)),
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

    private void SetDiagnosticCopyStatus(string? value, bool failed)
    {
        diagnosticCopyStatus.Text = value ?? string.Empty;
        diagnosticCopyStatus.IsVisible = value is not null;
        diagnosticCopyStatus.Foreground = failed
            ? LeserpentTheme.Destructive
            : LeserpentTheme.Accent;
        AutomationProperties.SetName(
            diagnosticCopyStatus,
            value is null ? "Diagnostic copy status" : value);
        AutomationProperties.SetLiveSetting(
            diagnosticCopyStatus,
            failed ? AutomationLiveSetting.Assertive : AutomationLiveSetting.Polite);
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
        SetStatus("Loading authenticated runtime snapshot...", LeserpentTheme.Primary);
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
            var alertSuffix = severityAlert.IsPending
                ? $" / {severityAlert.Describe()}"
                : string.Empty;
            SetStatus(
                liveRefresh.IsRequested
                    ? $"Live logs at revision {snapshot.Revision} / {(usedIncrementalLogs ? "incremental" : "full")} snapshot / {change.Describe()}{alertSuffix} / refresh every 5 seconds"
                    : $"Live workspace at revision {snapshot.Revision} / {change.Describe()}{alertSuffix}",
                statusBrush,
                assertive: change.NewErrors > 0);
        }
        catch (RemoteQueryException error)
        {
            ShowFailure($"Query rejected ({Safe(error.Code)}): {Safe(error.Message)}");
        }
        catch (InvalidDataException error)
        {
            ShowFailure($"Query response rejected: {Safe(error.Message)}");
        }
        catch (ArgumentException error)
        {
            ShowFailure($"Query blocked: {Safe(error.Message)}");
        }
        catch (OperationCanceledException) when (!lifetime.IsCancellationRequested)
        {
            ShowFailure("Query timed out; no partial workspace was retained");
        }
        catch (OperationCanceledException) when (lifetime.IsCancellationRequested)
        {
            // Window shutdown owns this cancellation.
            outcome = WorkspaceReloadOutcome.Closed;
        }
        catch (HttpRequestException)
        {
            ShowFailure("Query failed over the authenticated HTTPS connection");
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
        var reason = unexpected
            ? "unexpected query failure"
            : "authenticated query failure";
        if (liveRefresh.IsRequested)
        {
            var recovery = liveRefresh.State == WorkspaceLiveRefreshState.Suspended
                ? "retry when this window becomes active"
                : $"retry in {liveRefresh.NextInterval.TotalSeconds:0} seconds";
            SetStatus(
                $"Live logs recovering from {reason} ({liveRefresh.ConsecutiveFailures}/{RemoteWorkspaceLiveRefresh.MaxConsecutiveFailures}); {recovery}",
                LeserpentTheme.Destructive,
                assertive: true);
            return;
        }
        SetStatus(
            $"Live logs stopped after {RemoteWorkspaceLiveRefresh.MaxConsecutiveFailures} consecutive failures",
            LeserpentTheme.Destructive,
            assertive: true);
    }

    private void ShowFailure(string message)
    {
        SetStatus(message, LeserpentTheme.Destructive, assertive: true);
    }

    private void SetStatus(string text, IBrush foreground, bool assertive = false)
    {
        statusText.Text = text;
        statusText.Foreground = foreground;
        AutomationProperties.SetName(statusText, $"Runtime workspace query status: {text}");
        AutomationProperties.SetLiveSetting(
            statusText,
            assertive
                ? AutomationLiveSetting.Assertive
                : AutomationLiveSetting.Polite);
    }

    private static string Safe(string value) => new(value
        .Where(character => !char.IsControl(character))
        .Take(256)
        .ToArray());
}
