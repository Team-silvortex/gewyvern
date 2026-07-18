use std::fs;
use std::path::PathBuf;

fn remote_main_window_source() -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteMainWindow.cs"),
    )
    .expect("RemoteMainWindow source must exist")
}

fn avalonia_source(relative: &str) -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("apps/leserpent-avalonia/src")
            .join(relative),
    )
    .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn remote_window_observes_async_ui_operations_and_fences_shutdown_updates() {
    let source = remote_main_window_source();

    assert!(!source.contains("private async void RequestReconnect()"));
    assert!(!source.contains("private async void OnActionInvoked(string nodeId)"));
    assert!(source.contains("ObserveUiOperation(RequestReconnectAsync())"));
    assert!(source.contains("ObserveUiOperation(OnActionInvokedAsync(nodeId))"));
    assert!(source.contains("ObserveHealthOperation(RefreshAuthorityHealthAsync())"));
    assert!(!source.contains("ObserveUiOperation(RefreshAuthorityHealthAsync())"));
    assert!(source.contains("healthClient.Dispose();"));
    assert!(source.contains("eventClient.StateChanged -= OnStateChanged;"));
    assert!(source.contains("if (!isClosed)\n            {\n                ApplyState(state);"));
}

#[test]
fn connected_authority_health_is_visible_bounded_and_mutation_independent() {
    let source = remote_main_window_source();
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(source.contains("remote-authority-health"));
    assert!(source.contains("remote-authority-health-refresh"));
    assert!(source.contains("QUEUE SATURATED"));
    assert!(source.contains("AutomationLiveSetting.Assertive"));
    assert!(source.contains("AuthorityHealthPresentation.Create"));
    assert!(program.contains("--verify-authority-health-presentation"));
    assert!(program.contains("saturation_visible=true"));
}

#[test]
fn gui_mutations_export_canonical_leselang_without_execution() {
    let window = remote_main_window_source();
    let exporter = avalonia_source("Leserpent.RemoteClient/RemoteLeselangExport.cs");
    let control = avalonia_source("Leserpent.Avalonia/LeselangExportControl.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("RemoteLeselangExport.Refresh"));
    assert!(window.contains("RemoteLeselangExport.Deploy"));
    assert!(window.contains("new LeselangExportControl"));
    assert!(exporter.contains("runtime.refresh_capabilities"));
    assert!(exporter.contains("target: none"));
    assert!(exporter.contains("GUI Leselang export diverged"));
    assert!(!exporter.contains("RemoteWireTransport"));
    assert!(!exporter.contains("RemoteMutationClient(options"));
    assert!(control.contains("Copy Leselang"));
    assert!(control.contains("No operation was executed."));
    assert!(control.contains("SetTextAsync(source)"));
    assert!(program.contains("--verify-leselang-gui-export"));
}

#[test]
fn runtime_workspace_log_filter_is_local_bounded_and_accessible() {
    let window = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let filter = avalonia_source("Leserpent.Avalonia/RemoteWorkspaceLogFilter.cs");
    let export = avalonia_source("Leserpent.Avalonia/RemoteWorkspaceDiagnosticExport.cs");
    let projection = avalonia_source("Leserpent.Avalonia/RemoteWorkspaceDocumentProjection.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("runtime-log-search"));
    assert!(window.contains("runtime-log-level"));
    assert!(window.contains("runtime-log-filter-summary"));
    assert!(window.contains("runtime-diagnostics-copy"));
    assert!(window.contains("Review it before sharing"));
    assert!(window.contains(
        "catch (Exception)\n        {\n            if (!lifetime.IsCancellationRequested)"
    ));
    assert!(window.contains("KeyModifiers.Control | KeyModifiers.Meta"));
    assert!(window.contains("latestSnapshot = snapshot"));
    assert!(window.contains("RemoteWorkspaceLogFilter.Apply"));
    assert!(filter.contains("MaxQueryLength = 128"));
    assert!(filter.contains("StringComparison.OrdinalIgnoreCase"));
    assert!(filter.contains("log level filter is invalid"));
    assert!(!filter.contains("RemoteWorkspaceClient"));
    assert!(!filter.contains("RemoteWireTransport"));
    assert!(export.contains("leserpent.workspace-diagnostic/v1"));
    assert!(export.contains("MaxUtf8Bytes = 512 * 1024"));
    assert!(export.contains("MaxLogEntries"));
    assert!(export.contains("MaxLogDisplayBytes"));
    assert!(export.contains("command_id = "));
    assert!(!export.contains("new RemoteWorkspaceClient"));
    assert!(!export.contains("LoadAsync"));
    assert!(!export.contains("RemoteWireTransport"));
    assert!(!export.contains("RemoteMutationClient"));
    assert!(projection.contains("No matching log entries"));
    assert!(projection.contains("Safe(entry.CommandId)"));
    assert!(program.contains("--verify-workspace-diagnostics"));
    assert!(program.contains("--verify-workspace-log-filter"));
    assert!(program.contains("local_only=true"));
    assert!(program.contains("explicit_export=true"));
    assert!(program.contains("maximal_escape=true"));
}

#[test]
fn runtime_workspace_live_refresh_is_explicit_single_flight_and_suspendable() {
    let window = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let policy = avalonia_source("Leserpent.Avalonia/RemoteWorkspaceLiveRefresh.cs");
    let plan = avalonia_source("Leserpent.Avalonia/RemoteWorkspaceLogRefreshPlan.cs");
    let client = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceClient.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("runtime-workspace-live-logs"));
    assert!(window.contains("Activated +="));
    assert!(window.contains("Deactivated +="));
    assert!(window.contains("if (!liveRefresh.TryBegin())"));
    assert!(window.contains("outcome != WorkspaceReloadOutcome.Failed"));
    assert!(window.contains("liveRefresh.Pause();"));
    assert!(window.contains("liveRefreshTimer.Stop();"));
    assert!(
        window.contains("liveRefreshButton.IsEnabled = liveRefresh.IsRequested || !loadInFlight")
    );
    assert!(policy.contains("TimeSpan.FromSeconds(5)"));
    assert!(policy.contains("State != WorkspaceLiveRefreshState.Waiting"));
    assert!(policy.contains("State == WorkspaceLiveRefreshState.Suspended"));
    assert!(policy.contains("live refresh retried after a failed query"));
    assert!(!policy.contains("RemoteWorkspaceClient"));
    assert!(!policy.contains("RemoteWireTransport"));
    assert!(program.contains("live_refresh=true"));
    assert!(program.contains("incremental_logs=true"));
    assert!(window.contains("ReloadAsync(allowIncrementalLogs: true)"));
    assert!(window.contains("logRefreshPlan.SelectCursor"));
    assert!(window.contains("RemoteWorkspaceCodec.MergeIncrementalLogs"));
    assert!(window.contains("RequiresFullFallback"));
    assert!(plan.contains("IncrementalPollsBeforeFullSnapshot = 11"));
    assert!(plan.contains("manual workspace reload selected a log cursor"));
    assert!(plan.contains("periodic full resync"));
    assert!(plan.contains("incremental fallback policy drifted"));
    assert!(client.contains("public ulong? AfterSequence"));
    assert!(client.contains("\\\"after_sequence\\\":42"));
    assert!(client.contains("incremental workspace logs did not advance their cursor"));
    assert!(client.contains("TakeLast(RemoteWorkspaceClient.MaxLogEntries)"));
}

#[test]
fn runtime_workspace_refresh_reports_bounded_snapshot_changes() {
    let window = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let changes = avalonia_source("Leserpent.Avalonia/RemoteWorkspaceSnapshotChange.cs");
    let alert = avalonia_source("Leserpent.Avalonia/RemoteWorkspaceSeverityAlert.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("RemoteWorkspaceSnapshotChanges.Compare"));
    assert!(window.contains("change.Describe()"));
    assert!(changes.contains("initial snapshot"));
    assert!(changes.contains("no changes"));
    assert!(changes.contains("logs expired"));
    assert!(changes.contains("logs changed"));
    assert!(changes.contains("commands updated"));
    assert!(changes.contains("int NewErrors"));
    assert!(changes.contains("int NewWarnings"));
    assert!(changes.contains("CountNewLevel(priorLogs, currentLogs, \"error\")"));
    assert!(changes.contains("Compare(null, initial).NewErrors != 0"));
    assert!(changes.contains("log sequence reset"));
    assert!(changes.contains("workspace snapshot revision regressed"));
    assert!(changes.contains("workspace history contains a duplicate command ID"));
    assert!(changes.contains("workspace log sequence is not strictly increasing"));
    assert!(changes.contains("workspace log level is invalid"));
    assert!(changes.contains("workspace logs exceed their retained item limit"));
    assert!(changes.contains("workspace history exceeds its retained item limit"));
    assert!(changes.contains("var currentLogs = LogIndex(current.Logs)"));
    assert!(!changes.contains("RemoteWorkspaceClient"));
    assert!(!changes.contains("RemoteWireTransport"));
    assert!(program.contains("delta_summary=true"));
    assert!(program.contains("severity_signal=true"));
    assert!(program.contains("snapshot_fence=true"));
    assert!(program.contains("severity_ack=true"));
    assert!(window.contains("runtime-workspace-alert-acknowledge"));
    assert!(window.contains("severityAlert.Observe(snapshot.Revision, change)"));
    assert!(window.contains("assertive: change.NewErrors > 0"));
    assert!(window.contains("LeserpentTheme.Destructive"));
    assert!(window.contains("LeserpentTheme.Primary"));
    assert!(window.contains("assertive: true"));
    assert!(alert.contains("WorkspaceSeverityAlertLevel.Error"));
    assert!(alert.contains("workspace warning downgraded a pending error"));
    assert!(alert.contains("unchanged refresh discarded a pending alert"));
    assert!(alert.contains("alert.Acknowledge()"));
    assert!(!alert.contains("RemoteWorkspaceClient"));
    assert!(!alert.contains("RemoteWireTransport"));
}

#[test]
fn desktop_connection_preflight_is_explicit_cancellable_and_side_effect_free() {
    let window = avalonia_source("Leserpent.Avalonia/DesktopConnectionWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let health = avalonia_source("Leserpent.RemoteClient/RemoteHealthClient.cs");
    let test_start = app
        .find("private static async Task<string?> TestConnectionAsync")
        .expect("connection test callback must exist");
    let test_end = app[test_start..]
        .find("private static string? ForgetSavedConnection")
        .expect("connection test callback must have a bounded source region");
    let test_body = &app[test_start..test_start + test_end];

    assert!(window.contains("desktop-connect-test"));
    assert!(window.contains("TestConnectionAsync()"));
    assert!(window.contains("lifetime.Cancel();"));
    assert!(window.contains("if (operationInFlight || isClosed)"));
    assert!(health.contains("remote health did not prove a ready protocol-v1 authority"));
    assert!(health.contains("remote health queue counters are inconsistent"));
    assert!(health.contains("remote health response exceeds the message limit"));
    assert!(health.contains("JsonUnmappedMemberHandling.Disallow"));
    assert!(test_body.contains("RemoteHealthClient"));
    assert!(!test_body.contains("profileStore"));
    assert!(!test_body.contains("certificateStore"));
    assert!(!test_body.contains("RemoteTokenResolver.Store"));
    assert!(!test_body.contains(".Save("));
    assert!(!test_body.contains(".Import("));
}
