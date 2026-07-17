use std::fs;
use std::path::PathBuf;

fn remote_main_window_source() -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteMainWindow.cs"),
    )
    .expect("RemoteMainWindow source must exist")
}

#[test]
fn remote_window_observes_async_ui_operations_and_fences_shutdown_updates() {
    let source = remote_main_window_source();

    assert!(!source.contains("private async void RequestReconnect()"));
    assert!(!source.contains("private async void OnActionInvoked(string nodeId)"));
    assert!(source.contains("ObserveUiOperation(RequestReconnectAsync())"));
    assert!(source.contains("ObserveUiOperation(OnActionInvokedAsync(nodeId))"));
    assert!(source.contains("eventClient.StateChanged -= OnStateChanged;"));
    assert!(source.contains("if (!isClosed)\n            {\n                ApplyState(state);"));
}
