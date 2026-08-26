use std::fs;
use std::path::PathBuf;

fn source(relative: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn registration_client_is_secret_free_strict_and_revision_fenced() {
    let contracts =
        source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteRegistrationContracts.cs");
    let client =
        source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteRegistrationClient.cs");
    let program = source("apps/leserpent-avalonia/src/Leserpent.Avalonia/Program.cs");

    assert!(contracts.contains("JsonUnmappedMemberHandling.Disallow"));
    assert!(contracts.contains("RemoteRegistrationJsonContext : JsonSerializerContext"));
    assert!(contracts.contains("public ulong? ExpectedRevision { get; set; }"));
    assert!(contracts.contains("JsonIgnoreCondition.WhenWritingNull"));
    assert!(!contracts.contains("AdminToken"));
    assert!(!contracts.contains("PairingToken"));
    assert!(!contracts.contains("CredentialHandle"));
    assert!(client.contains("public async Task<RemoteRegistrationDetails> InspectAsync("));
    assert!(client.contains("public Task<RemoteRegistrationPlan> PlanRegisterAsync("));
    assert!(client.contains("public Task<RemoteRegistrationPlan> PlanUpdateAsync("));
    assert!(client.contains("public async Task<RemoteRegistrationResult> ApplyAsync("));
    assert!(client.contains("Capabilities = [\"runtime.register\"]"));
    assert!(client.contains("Confirmation = dryRun ? \"not_required\" : \"confirmed\""));
    assert!(client.contains("DryRun = dryRun"));
    assert!(client.contains("runtime_registration_update"));
    assert!(client.contains("result.Events is not [var domainEvent]"));
    assert!(client.contains("result.Runtime.Revision < plan.PlannedRevision"));
    assert!(client.contains("RemoteWorkspaceCodec.ValidateRuntime"));
    assert!(!client.contains("options.Token"));
    assert!(program.contains("--verify-registration-client"));
    assert!(program.contains("update_revision_fence=true"));
    assert!(program.contains("secret_free=true"));
}

#[test]
fn native_registration_editor_has_reachable_create_and_update_workflows() {
    let main = source("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteMainWindow.cs");
    let workspace =
        source("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let window =
        source("apps/leserpent-avalonia/src/Leserpent.Avalonia/RuntimeRegistrationWindow.cs");
    let catalog =
        source("apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopRegistrationCatalogs.cs");
    let localization =
        source("apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopLocalization.cs");
    let app = source("apps/leserpent-avalonia/src/Leserpent.Avalonia/LeserpentApp.cs");

    assert!(main.contains("remote-runtime-registration-open"));
    assert!(main.contains("private async Task OpenRuntimeRegistrationAsync()"));
    assert!(main.contains("registrationMutationEnabled = availability.MutationsEnabled"));
    assert!(main.contains("registrationWindow?.SetMutationAvailability"));
    assert!(main.contains("registrationWindow?.Close()"));
    assert!(workspace.contains("runtime-registration-edit"));
    assert!(workspace.contains("private async Task EditRegistrationAsync()"));
    assert!(workspace.contains("ReloadIfOlder(result.Revision)"));
    assert!(workspace.contains("registrationMutationEnabled = enabled"));
    assert!(workspace.contains("registrationWindow?.SetMutationAvailability(enabled)"));
    assert!(window.contains("private async Task LoadExistingAsync()"));
    assert!(window.contains("private async Task ReviewAsync()"));
    assert!(window.contains("private async Task ApplyAsync()"));
    assert!(window.contains("confirmation.IsChecked != true"));
    assert!(window.contains("SetFieldsReadOnly(true)"));
    assert!(window.contains("EditReviewedPlan()"));
    assert!(window.contains("plan.ExpectedRevision != details!.Revision"));
    assert!(window.contains("internal void SetMutationAvailability(bool enabled)"));
    assert!(window.contains("internal void ProbeMutationAvailabilityFence()"));
    assert!(window.contains("public void VerifyAccessibility()"));
    assert!(window.contains("public void VerifyLayoutEnvelope()"));
    assert!(catalog.contains("public const int KeyCount = 49"));
    assert!(
        catalog.contains("Desktop registration localization catalog is incomplete")
            || catalog.contains("desktop registration localization catalog is incomplete")
    );
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
            "missing registration locale {locale}"
        );
        assert!(
            localization.contains(&format!("DesktopRegistrationCatalogs.{locale}")),
            "registration locale {locale} is not installed"
        );
    }
    assert!(localization.contains("DesktopRegistrationCatalogs.VerifyContract();"));
    assert!(app.contains("localized_registration_layouts=16"));
    assert!(app.contains("registration_dry_run=true"));
    assert!(app.contains("registration_revision_fence=true"));
    assert!(app.contains("registration_confirmation=true"));
    assert!(app.contains("registration_mutation_fence=true"));
    assert!(app.contains("network_started=false"));
}

#[test]
fn function_chain_closes_existing_runtime_registration_without_hiding_other_gaps() {
    let matrix = source("project/release/leserpent-gui-function-chain.json");
    let docs = source("docs/leserpent-gui-function-chains.md");
    let status = source("project/status/catalog.json");

    let registration = matrix
        .split("\"id\": \"runtime-registration-lifecycle\"")
        .nth(1)
        .expect("registration function chain must exist")
        .split("\"id\": \"daemon-lifecycle\"")
        .next()
        .expect("registration function chain must be bounded");
    assert!(registration.contains("\"surface\": \"avalonia-desktop\""));
    assert!(registration.contains("\"state\": \"closed\""));
    for stage in [
        "entry",
        "semantic",
        "transport",
        "authority",
        "persistence",
        "projection",
        "feedback",
    ] {
        assert!(
            registration.contains(&format!("\"stage\": \"{stage}\"")),
            "registration chain lacks {stage} evidence"
        );
    }
    assert!(docs.contains("| Avalonia desktop | target | 86 | 7 | 1 | 1 | 0 |"));
    assert!(docs.contains("The combined target score is 78"));
    assert!(docs.contains("field edits invalidate it"));
    assert!(!status.contains("\"id\": \"avalonia-runtime-registration-editor\""));
    assert!(status.contains("\"id\": \"product-debugger-session-bridge\""));
    assert!(status.contains("\"id\": \"product-leselang-execution-host\""));
    assert!(status.contains("\"id\": \"rust-web-self-host\""));
}
