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
