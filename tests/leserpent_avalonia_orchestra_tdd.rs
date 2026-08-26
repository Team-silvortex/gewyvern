use std::fs;
use std::path::PathBuf;

fn source(relative: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn orchestra_remote_client_is_strict_bounded_and_receipt_driven() {
    let contracts =
        source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteOrchestraContracts.cs");
    let client =
        source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteOrchestraClient.cs");
    let program = source("apps/leserpent-avalonia/src/Leserpent.Avalonia/Program.cs");

    assert!(contracts.contains("JsonUnmappedMemberHandling.Disallow"));
    assert!(contracts.contains("public string Kind { get; set; } = \"orchestra_history\""));
    assert!(contracts.contains("public string Kind { get; set; } = \"orchestra_delete_command\""));
    assert!(contracts.contains("RemoteOrchestraJsonContext : JsonSerializerContext"));
    assert!(contracts.contains("RemoteOrchestraDeleteReceipt"));
    assert!(client.contains("private const string OrchestraCapability = \"orchestra.write\""));
    assert!(client.contains("public const ushort MaxPageSize = 64"));
    assert!(client.contains("public const uint MaxOffset = 10_000"));
    assert!(client.contains("transport.PostAsync("));
    assert!(client.contains("DecodeHistoryResponse(response, runtimeId, runId, offset, limit)"));
    assert!(client.contains("DecodeDeleteResponse(response, runtimeId, commandId)"));
    assert!(client.contains("receipt.CommandId != commandId"));
    assert!(client.contains("orchestraEvent.RuntimeId != runtimeId"));
    assert!(client.contains("orchestraEvent.RunId != runId"));
    assert!(client.contains("page.NextOffset is uint nextOffset"));
    assert!(client.contains("ExpectInvalid(() => DecodeHistoryResponse("));
    assert!(client.contains("ExpectInvalid(() => DecodeDeleteResponse("));
    assert!(!client.contains("HttpClient"));
    assert!(program.contains("--verify-orchestra-client"));
    assert!(program.contains("idempotent_cleanup=true"));
}

#[test]
fn daemon_window_owns_one_localized_orchestra_history_workspace() {
    let main = source("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteMainWindow.cs");
    let workspace =
        source("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteOrchestraWorkspaceWindow.cs");
    let catalog =
        source("apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopOrchestraCatalogs.cs");
    let localization =
        source("apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopLocalization.cs");
    let app = source("apps/leserpent-avalonia/src/Leserpent.Avalonia/LeserpentApp.cs");

    assert!(main.contains("private RemoteOrchestraWorkspaceWindow? orchestraWorkspace"));
    assert!(main.contains("private void OpenOrchestraWorkspace()"));
    assert!(main.contains("orchestraWorkspace.Activate()"));
    assert!(main.contains("workspace.Show(this)"));
    assert!(main.contains("orchestraWorkspace?.Close()"));
    assert!(main.contains("remote-orchestra-open"));
    assert!(workspace.contains("client.LoadRunsAsync("));
    assert!(workspace.contains("client.LoadEventsAsync("));
    assert!(workspace.contains("client.DeleteRuntimeHistoryAsync("));
    assert!(workspace.contains("ShowDialog<bool>(this)"));
    assert!(workspace.contains("status.cleanup_completed"));
    assert!(workspace.contains("CompactBreakpoint = 840"));
    assert!(workspace.contains("MaxRetainedRuns = 256"));
    assert!(workspace.contains("MaxRetainedEvents = 256"));
    assert!(workspace.contains("when (lifetime.IsCancellationRequested)"));
    assert!(workspace.contains("public void VerifyLayoutEnvelope()"));
    assert!(workspace.contains("public void VerifyAccessibility()"));
    assert!(workspace.contains("public void ProbeProjection()"));
    assert!(workspace.contains("startLoading: false") || app.contains("startLoading: false"));
    assert!(!workspace.contains("options.Token"));
    assert_eq!(catalog.matches("new(\"").count(), 41);
    assert!(catalog.contains("public const int KeyCount = 41"));
    for locale in [
        "SimplifiedChinese",
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
    ] {
        assert!(
            catalog.contains(locale),
            "missing Orchestra locale {locale}"
        );
        assert!(localization.contains(&format!("DesktopOrchestraCatalogs.{locale}")));
    }
    assert!(localization.contains("DesktopOrchestraCatalogs.VerifyContract();"));
    assert!(app.contains("localized_orchestra_layouts=8"));
    assert!(app.contains("localized_dialog_layouts=24"));
    assert!(app.contains("network_started=false"));
}

#[test]
fn function_chain_claims_only_the_orchestra_slice_that_exists() {
    let matrix = source("project/release/leserpent-gui-function-chain.json");
    let docs = source("docs/leserpent-gui-function-chains.md");

    assert!(matrix.contains("\"id\": \"orchestra-control\""));
    assert!(
        matrix.contains("\"surface\": \"avalonia-desktop\",\n          \"state\": \"partial\"")
    );
    assert!(matrix.contains("authenticated persisted run history"));
    assert!(matrix.contains("Plan execution, cancellation, and retry remain"));
    assert!(matrix.contains("RemoteOrchestraWorkspaceWindow.cs"));
    assert!(docs.contains("| Avalonia desktop | target | 75 | 5 | 3 | 1 | 0 |"));
    assert!(docs.contains("The combined target score is 68"));
    assert!(docs.contains("Rust-authoritative Orchestra plan execution"));
}
