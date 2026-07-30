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

fn repo_source(relative: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn leselang_presentation_atoms_are_typed_native_operations() {
    let core = avalonia_source("Leserpent.RendererCore/Program.cs");
    let renderer = avalonia_source("Leserpent.Avalonia/AvaloniaDocumentRenderer.cs");
    let window = avalonia_source("Leserpent.Avalonia/MainWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let conformance = avalonia_source("Leserpent.RendererConformance/Program.cs");

    assert!(core.contains("public sealed class UiPresentationOperation"));
    assert!(core.contains("UiPresentationOperationKind.Focus"));
    assert!(core.contains("UiPresentationOperationKind.ScrollIntoView"));
    assert!(core.contains("UiPresentationOperationKind.AssertVisible"));
    assert!(core.contains("UiPresentationOperationKind.AssertRealized"));
    assert!(core.contains("UiPresentationOperationKind.WaitRealized"));
    assert!(core.contains("WaitRealizedTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitVisible"));
    assert!(core.contains("WaitVisibleTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitEnabled"));
    assert!(core.contains("UiPresentationOperationKind.WaitDisabled"));
    assert!(core.contains("UiPresentationOperationKind.AssertWindowOpen"));
    assert!(core.contains("UiPresentationOperationKind.WaitWindowOpen"));
    assert!(core.contains("UiPresentationOperationKind.AssertWindowClosed"));
    assert!(core.contains("UiPresentationOperationKind.WaitWindowClosed"));
    assert!(core.contains("WaitEnabledTimeoutMs = 2000"));
    assert!(core.contains("WaitWindowOpenTimeoutMs = 2000"));
    assert!(core.contains("WaitWindowClosedTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitFocused"));
    assert!(core.contains("WaitFocusedTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitUnfocused"));
    assert!(core.contains("UiPresentationOperationKind.AssertUnfocused"));
    assert!(core.contains("WaitUnfocusedTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.NavigateFocus"));
    assert!(core.contains("UiFocusNavigationDirection"));
    assert!(core.contains("[JsonStringEnumMemberName(\"first\")] First"));
    assert!(core.contains("[JsonStringEnumMemberName(\"last\")] Last"));
    assert!(core.contains("UiPresentationOperationKind.AssertSelection"));
    assert!(core.contains("UiPresentationOperationKind.WaitSelection"));
    assert!(core.contains("UiSelectionState"));
    assert!(core.contains("WaitSelectionTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertFocused"));
    assert!(core.contains("UiPresentationOperationKind.AssertEnabled"));
    assert!(core.contains("UiPresentationOperationKind.AssertDisabled"));
    assert!(core.contains("UiPresentationOperationKind.AssertHidden"));
    assert!(core.contains("UiPresentationOperationKind.WaitHidden"));
    assert!(core.contains("UiPresentationOperationKind.AssertText"));
    assert!(core.contains("UiPresentationOperationKind.WaitText"));
    assert!(core.contains("WaitTextTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitAccessibleName"));
    assert!(core.contains("WaitAccessibleNameTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitAccessibleDescription"));
    assert!(core.contains("WaitAccessibleDescriptionTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertAutomationId"));
    assert!(core.contains("UiPresentationOperationKind.AssertNodeKind"));
    assert!(core.contains("UiPresentationOperationKind.WaitNodeKind"));
    assert!(core.contains("WaitNodeKindTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertActionKind"));
    assert!(core.contains("UiPresentationOperationKind.WaitActionKind"));
    assert!(core.contains("WaitActionKindTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertActionLabel"));
    assert!(core.contains("UiPresentationOperationKind.WaitActionLabel"));
    assert!(core.contains("WaitActionLabelTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertActionAvailable"));
    assert!(core.contains("UiPresentationOperationKind.WaitActionAvailable"));
    assert!(core.contains("WaitActionAvailableTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertActionUnavailableReason"));
    assert!(core.contains("UiPresentationOperationKind.WaitActionUnavailableReason"));
    assert!(core.contains("WaitActionUnavailableReasonTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertFormField"));
    assert!(core.contains("public string? Field { get; set; }"));
    assert!(core.contains("UiPresentationOperationKind.AssertFormFieldMaxLength"));
    assert!(core.contains("public int? MaxLength { get; set; }"));
    assert!(core.contains("UiPresentationValidation.InvalidExpectedMaxLength"));
    assert!(core.contains("UiPresentationOperationKind.AssertFormFieldPlaceholder"));
    assert!(core.contains("UiPresentationOperationKind.WaitFormFieldPlaceholder"));
    assert!(core.contains("WaitFormFieldPlaceholderTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertAccessibleName"));
    assert!(core.contains("UiPresentationOperationKind.AssertAccessibleDescription"));
    assert!(core.contains("UiPresentationValidation.UnknownTarget"));
    assert!(core.contains("UiPresentationValidation.UnfocusableTarget"));
    assert!(core.contains("UiPresentationValidation.SelectionlessTarget"));
    assert!(core.contains("UiPresentationValidation.InvalidExpectedKind"));
    assert!(core.contains("UiPresentationValidation.InvalidExpectedActionKind"));
    assert!(renderer.contains("PresentationAutomationResult ApplyPresentation"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetUnrealized"));
    assert!(renderer.contains("PresentationAutomationFailureCode.WaitTimedOut"));
    assert!(renderer.contains("ApplyPresentationAsync"));
    assert!(renderer.contains("Task.Delay("));
    assert!(renderer.contains("PresentationAutomationFailureCode.FocusRejected"));
    assert!(renderer.contains("PresentationAutomationFailureCode.NavigationRejected"));
    assert!(renderer.contains("focusManager.TryMoveFocus("));
    assert!(renderer.contains("UiFocusNavigationDirection.First => FocusBoundaryAction"));
    assert!(renderer.contains("UiFocusNavigationDirection.Last => FocusBoundaryAction"));
    assert!(renderer.contains("private RenderedNode? FocusBoundaryAction"));
    assert!(renderer.contains("nodes.Values.Reverse()"));
    assert!(renderer.contains("control!.BringIntoView()"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetNotVisible"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetStillVisible"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetNotFocused"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetStillFocused"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetNotEnabled"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetStillEnabled"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetWindowUnavailable"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetWindowStillOpen"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetTextMismatch"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetAutomationIdMismatch"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetNodeKindMismatch"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetActionKindMismatch"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetActionLabelMismatch"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetActionUnavailable"));
    assert!(
        renderer
            .contains("PresentationAutomationFailureCode.TargetActionUnavailableReasonMismatch")
    );
    assert!(renderer.contains("UiPresentationOperationKind.WaitActionLabel"));
    assert!(renderer.contains("WaitActionLabelTimeoutMs"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitActionAvailable"));
    assert!(renderer.contains("WaitActionAvailableTimeoutMs"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitActionUnavailableReason"));
    assert!(renderer.contains("WaitActionUnavailableReasonTimeoutMs"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetFormFieldMismatch"));
    assert!(
        renderer.contains("PresentationAutomationFailureCode.TargetFormFieldMaxLengthMismatch")
    );
    assert!(renderer.contains("IReadOnlyDictionary<string, string>? FormFieldLabels"));
    assert!(renderer.contains("IReadOnlyDictionary<string, int>? FormFieldMaxLengths"));
    assert!(renderer.contains("PresentationAutomationFailureCode.InvalidExpectedKind"));
    assert!(renderer.contains("PresentationAutomationFailureCode.InvalidExpectedActionKind"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetAccessibleNameMismatch"));
    assert!(
        renderer.contains("PresentationAutomationFailureCode.TargetAccessibleDescriptionMismatch")
    );
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetSelectionMismatch"));
    assert!(renderer.contains("NativeSelectionState"));
    assert!(renderer.contains("ListBoxItem"));
    assert!(renderer.contains("list.SelectedItem = item"));
    assert!(renderer.contains("control.IsEffectivelyVisible"));
    assert!(renderer.contains("control!.IsEffectivelyEnabled"));
    assert!(renderer.contains("StringComparer.Ordinal.Equals(actual, operation.Expected)"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitText"));
    assert!(renderer.contains("WaitTextTimeoutMs"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitAccessibleName"));
    assert!(renderer.contains("WaitAccessibleNameTimeoutMs"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitAccessibleDescription"));
    assert!(renderer.contains("WaitAccessibleDescriptionTimeoutMs"));
    assert!(window.contains("private readonly string initialEnabledWaitNodeId"));
    assert!(
        !window.contains("private readonly Task<PresentationAutomationResult> initialEnabledWait;")
    );
    assert!(
        !window.contains("private readonly Task<PresentationAutomationResult> initialFocusedWait;")
    );
    assert!(!window.contains(
        "private readonly Task<PresentationAutomationResult> initialFocusedWaitTimeout;"
    ));
    assert!(window.contains("var initiallyDisabled = renderer.ApplyPresentation"));
    assert!(window.contains("var enabledWait = renderer.ApplyPresentationAsync"));
    assert!(window.contains("InitialTextWaitCompleted"));
    assert!(window.contains("InitialTextWaitTimedOut"));
    assert!(window.contains("InitialAccessibleNameWaitCompleted"));
    assert!(window.contains("InitialAccessibleNameWaitTimedOut"));
    assert!(window.contains("PatchTextFallback("));
    assert!(window.contains("var actionUnavailableReasonWait = renderer.ApplyPresentationAsync"));
    assert!(window.contains("ActionUnavailableReasonWaitCompleted"));
    assert!(window.contains("ActionUnavailableReasonWaitClearedCompleted"));
    assert!(window.contains("ActionUnavailableReasonWaitTimedOut"));
    assert!(window.contains("var focusedWait = renderer.ApplyPresentationAsync"));
    assert!(window.contains("var focusedTimeoutResult = await renderer.ApplyPresentationAsync"));
    assert!(window.contains("renderer.ApplyPresentation(new UiPresentationOperation"));
    assert!(app.contains("leselang_presentation=true"));
    assert!(app.contains("navigate_focus=true"));
    assert!(app.contains("navigate_focus_forward="));
    assert!(app.contains("navigate_focus_backward="));
    assert!(app.contains("navigate_focus_first="));
    assert!(app.contains("navigate_focus_last="));
    assert!(app.contains("navigate_focus_stable_destination=true"));
    assert!(app.contains("navigate_focus_failure_preserved_focus="));
    assert!(app.contains("navigate_focus_no_activation="));
    assert!(app.contains("scroll_into_view=true"));
    assert!(app.contains("assert_visible=true"));
    assert!(app.contains("assert_hidden="));
    assert!(app.contains("wait_hidden="));
    assert!(app.contains("wait_hidden_external_transition="));
    assert!(app.contains("wait_hidden_timeout="));
    assert!(app.contains("visible_target_hidden_assertion_rejected="));
    assert!(app.contains("assert_realized=true"));
    assert!(app.contains("wait_realized=true"));
    assert!(app.contains("wait_realized_natural_layout=true"));
    assert!(app.contains("wait_realized_timeout=true"));
    assert!(app.contains("wait_visible=true"));
    assert!(app.contains("wait_visible_natural_layout=true"));
    assert!(app.contains("wait_visible_timeout=true"));
    assert!(app.contains("wait_enabled=true"));
    assert!(app.contains("wait_enabled_external_transition=true"));
    assert!(app.contains("wait_enabled_timeout=true"));
    assert!(app.contains("wait_disabled="));
    assert!(app.contains("wait_disabled_external_transition="));
    assert!(app.contains("wait_disabled_timeout="));
    assert!(app.contains("assert_window_open="));
    assert!(app.contains("wait_window_open="));
    assert!(app.contains("assert_window_closed="));
    assert!(app.contains("wait_window_closed="));
    assert!(app.contains("wait_window_closed_timeout="));
    assert!(app.contains("wait_focused=true"));
    assert!(app.contains("wait_focused_external_transition=true"));
    assert!(app.contains("wait_focused_timeout=true"));
    assert!(app.contains("wait_focused_no_focus_mutation=true"));
    assert!(app.contains("assert_unfocused="));
    assert!(app.contains("wait_unfocused="));
    assert!(app.contains("wait_unfocused_timeout="));
    assert!(app.contains("assert_selection="));
    assert!(app.contains("wait_selection="));
    assert!(app.contains("wait_selection_timeout="));
    assert!(app.contains("selection_mismatch_rejected="));
    assert!(app.contains("selectionless_target_rejected="));
    assert!(app.contains("selection_focus_preserved="));
    assert!(app.contains("unrealized_target_rejected=true"));
    assert!(app.contains("assert_focused=true"));
    assert!(app.contains("assert_enabled=true"));
    assert!(app.contains("assert_disabled="));
    assert!(app.contains("enabled_target_disabled_assertion_rejected="));
    assert!(app.contains("disabled_target_rejected=true"));
    assert!(app.contains("assert_text=true"));
    assert!(app.contains("wait_text="));
    assert!(app.contains("wait_text_timeout="));
    assert!(app.contains("wait_accessible_name="));
    assert!(app.contains("wait_accessible_name_timeout="));
    assert!(app.contains("wait_accessible_description="));
    assert!(app.contains("wait_accessible_description_timeout="));
    assert!(app.contains("wait_form_field_placeholder="));
    assert!(app.contains("wait_form_field_placeholder_timeout="));
    assert!(app.contains("assert_automation_id=true"));
    assert!(app.contains("automation_id_mismatch_rejected=true"));
    assert!(app.contains("assert_node_kind=true"));
    assert!(app.contains("wait_node_kind="));
    assert!(app.contains("wait_node_kind_timeout="));
    assert!(app.contains("node_kind_mismatch_rejected=true"));
    assert!(app.contains("assert_action_kind="));
    assert!(app.contains("wait_action_kind="));
    assert!(app.contains("wait_action_kind_timeout="));
    assert!(app.contains("action_kind_mismatch_rejected="));
    assert!(app.contains("assert_action_label="));
    assert!(app.contains("wait_action_label="));
    assert!(app.contains("wait_action_label_timeout="));
    assert!(app.contains("action_label_mismatch_rejected="));
    assert!(app.contains("assert_action_available="));
    assert!(app.contains("wait_action_available="));
    assert!(app.contains("wait_action_available_timeout="));
    assert!(app.contains("assert_action_unavailable_reason="));
    assert!(app.contains("wait_action_unavailable_reason="));
    assert!(app.contains("wait_action_unavailable_reason_cleared="));
    assert!(app.contains("wait_action_unavailable_reason_timeout="));
    assert!(app.contains("action_unavailable_reason_mismatch_rejected="));
    assert!(app.contains("assert_form_field="));
    assert!(app.contains("form_field_mismatch_rejected="));
    assert!(app.contains("assert_form_field_input_kind="));
    assert!(app.contains("form_field_input_kind_mismatch_rejected="));
    assert!(app.contains("assert_form_field_required="));
    assert!(app.contains("form_field_required_mismatch_rejected="));
    assert!(app.contains("assert_form_field_max_length="));
    assert!(app.contains("form_field_max_length_mismatch_rejected="));
    assert!(app.contains("assert_form_field_placeholder="));
    assert!(app.contains("form_field_placeholder_mismatch_rejected="));
    assert!(app.contains("assert_accessible_name=true"));
    assert!(app.contains("accessible_name_mismatch_rejected=true"));
    assert!(app.contains("assert_accessible_description=true"));
    assert!(app.contains("accessible_description_mismatch_rejected=true"));
    assert!(app.contains("text_mismatch_rejected=true"));
    assert!(conformance.contains("presentation_focus=true"));
    assert!(conformance.contains("presentation_navigate_focus=true"));
    assert!(conformance.contains("presentation_navigate_focus_first_last=true"));
    assert!(conformance.contains("presentation_assert_hidden=true"));
    assert!(conformance.contains("presentation_wait_hidden=true"));
    assert!(conformance.contains("presentation_wait_visible=true"));
    assert!(conformance.contains("presentation_wait_enabled=true"));
    assert!(conformance.contains("presentation_wait_disabled=true"));
    assert!(conformance.contains("presentation_assert_window_open=true"));
    assert!(conformance.contains("presentation_wait_window_open=true"));
    assert!(conformance.contains("presentation_assert_window_closed=true"));
    assert!(conformance.contains("presentation_wait_window_closed=true"));
    assert!(conformance.contains("presentation_wait_focused=true"));
    assert!(conformance.contains("presentation_assert_unfocused=true"));
    assert!(conformance.contains("presentation_wait_unfocused=true"));
    assert!(conformance.contains("presentation_assert_selection=true"));
    assert!(conformance.contains("presentation_wait_selection=true"));
    assert!(conformance.contains("presentation_assert_disabled=true"));
    assert!(conformance.contains("presentation_assert_automation_id=true"));
    assert!(conformance.contains("presentation_assert_node_kind=true"));
    assert!(conformance.contains("presentation_wait_node_kind=true"));
    assert!(conformance.contains("presentation_wait_text=true"));
    assert!(conformance.contains("presentation_wait_accessible_name=true"));
    assert!(conformance.contains("presentation_wait_accessible_description=true"));
    assert!(conformance.contains("presentation_wait_form_field_placeholder=true"));
    assert!(conformance.contains("presentation_assert_action_kind=true"));
    assert!(conformance.contains("presentation_wait_action_kind=true"));
    assert!(conformance.contains("presentation_assert_action_label=true"));
    assert!(conformance.contains("presentation_wait_action_label=true"));
    assert!(conformance.contains("presentation_assert_action_available=true"));
    assert!(conformance.contains("presentation_wait_action_available=true"));
    assert!(conformance.contains("presentation_assert_action_unavailable_reason=true"));
    assert!(conformance.contains("presentation_wait_action_unavailable_reason=true"));
    assert!(conformance.contains("presentation_assert_form_field=true"));
    assert!(conformance.contains("presentation_assert_form_field_input_kind=true"));
    assert!(conformance.contains("presentation_assert_form_field_required=true"));
    assert!(conformance.contains("presentation_assert_form_field_max_length=true"));
    assert!(conformance.contains("presentation_assert_form_field_placeholder=true"));
}

#[test]
fn workspace_policies_are_renderer_independent_and_mobile_consumable() {
    let remote_project = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/Leserpent.RemoteClient.csproj",
    );
    let fleet_projection = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteDocumentProjection.cs",
    );
    let workspace_projection = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteWorkspaceDocumentProjection.cs",
    );
    let mutation_fences =
        repo_source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteMutationFences.cs");
    let mutation_availability = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteMutationAvailability.cs",
    );
    let remote_window =
        repo_source("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteMainWindow.cs");
    let mobile_project =
        repo_source("apps/leserpent-mobile/src/Leserpent.MobileCore/Leserpent.MobileCore.csproj");
    let mobile_conformance =
        repo_source("apps/leserpent-mobile/src/Leserpent.MobileConformance/Program.cs");

    assert!(remote_project.contains("Leserpent.RendererCore.csproj"));
    assert!(fleet_projection.contains("public static class RemoteDocumentProjection"));
    assert!(fleet_projection.contains("public sealed record RemoteDocumentView"));
    assert!(!fleet_projection.contains("Avalonia"));
    assert!(workspace_projection.contains("public static class RemoteWorkspaceDocumentProjection"));
    assert!(!workspace_projection.contains("Avalonia"));
    assert!(mutation_fences.contains("public static class RemoteMutationFences"));
    assert!(mutation_fences.contains("public sealed record RemoteMutationRevisionFence"));
    assert!(mutation_fences.contains("public sealed record RemoteMutationObservationFence"));
    assert!(!mutation_fences.contains("Avalonia"));
    assert!(mutation_availability.contains("public static class RemoteMutationAvailabilityPolicy"));
    assert!(mutation_availability.contains("public sealed record RemoteMutationAvailability"));
    assert!(!mutation_availability.contains("Avalonia"));
    assert!(remote_window.contains("RemoteMutationFences.SatisfiesRevision"));
    assert!(remote_window.contains("RemoteMutationFences.SatisfiesObservation"));
    assert!(remote_window.contains("RemoteMutationAvailabilityPolicy.Evaluate"));
    assert!(
        !remote_window
            .contains("Remote changes are unavailable while the event stream is not live")
    );
    assert!(!remote_window.contains("Remote refresh requires a live, idle fleet window"));
    assert_eq!(
        remote_window
            .matches("workspace.SetRefreshAvailability(")
            .count(),
        1,
        "workspace availability must only be written from the shared policy"
    );
    assert!(!remote_window.contains("SatisfiesMutationFence"));
    assert!(!remote_window.contains("SatisfiesObservationFence"));
    assert!(mobile_project.contains("Leserpent.RemoteClient.csproj"));
    assert!(mobile_conformance.contains("RemoteWorkspaceLogFilter.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceDiagnosticExport.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceLiveRefresh.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceLogRefreshPlan.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceSeverityAlert.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceSnapshotChanges.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteDocumentProjection.VerifyFilterContract()"));
    assert!(
        mobile_conformance.contains("RemoteWorkspaceDocumentProjection.VerifyEndpointIsolation()")
    );
    assert!(
        mobile_conformance
            .contains("RemoteWorkspaceDocumentProjection.VerifyParameterizedFormContract()")
    );
    assert!(mobile_conformance.contains("workspace_policy=true"));
    assert!(mobile_conformance.contains("ui_projection=true"));
    assert!(mobile_conformance.contains("RemoteMutationFences.VerifyContract()"));
    assert!(mobile_conformance.contains("mutation_fence=true"));
    assert!(mobile_conformance.contains("RemoteMutationAvailabilityPolicy.VerifyContract()"));
    assert!(mobile_conformance.contains("action_availability=true"));
    assert!(mobile_conformance.contains("RemoteAuthorityHealthPresentation.VerifyContract()"));
    assert!(mobile_conformance.contains("authority_health=true"));
}

#[test]
fn gewyvern_provisioning_is_authority_scoped_identity_locked_and_bounded() {
    let client = avalonia_source("Leserpent.RemoteClient/RemoteProvisioningClient.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let window = avalonia_source("Leserpent.Avalonia/GewyvernProvisioningWindow.cs");
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let promotion = avalonia_source("Leserpent.Avalonia/DesktopBootstrapPromotion.cs");

    assert!(client.contains("public sealed class RemoteProvisioningClient"));
    assert!(client.contains("Capability = \"runtime.provision\""));
    assert!(client.contains("InstallCredentialHandle"));
    assert!(client.contains("state.ProvisioningId != expected.ProvisioningId"));
    assert!(client.contains("state.RuntimeId != expected.RuntimeId"));
    assert!(!client.contains("Capability = \"runtime.deploy\""));
    assert!(transport.contains("\"v1/provisioning\""));
    assert!(window.contains("MaxAutomaticObservations = 30"));
    assert!(window.contains("LockIdentityFields()"));
    assert!(window.contains("new provisioning ID"));
    assert!(window.contains("provisioning-credential-handle"));
    assert!(window.contains("AutomationLiveSetting.Assertive"));
    assert!(hub.contains("hub-provision-gewyvern"));
    assert!(app.contains("ExecuteProvisioningAsync"));
    assert!(app.contains("--verify-provisioning-controls"));
    assert!(program.contains("--verify-provisioning-client"));
    assert!(promotion.contains("BootstrapPromotionJsonContext.Default"));
    assert!(!promotion.contains("JsonSerializer.Serialize(new\n"));
}

#[test]
fn gewyvern_retirement_is_confirmed_provisioning_bound_and_failure_safe() {
    let client = avalonia_source("Leserpent.RemoteClient/RemoteRetirementClient.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let window = avalonia_source("Leserpent.Avalonia/GewyvernRetirementWindow.cs");
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(client.contains("public sealed class RemoteRetirementClient"));
    assert!(client.contains("Capability = \"runtime.retire\""));
    assert!(client.contains("RetirementCredentialHandle"));
    assert!(client.contains("state.RetirementId != expected.RetirementId"));
    assert!(client.contains("state.ProvisioningId != expected.ProvisioningId"));
    assert!(client.contains("state.RuntimeId != expected.RuntimeId"));
    assert!(client.contains("\"failed\""));
    assert!(client.contains("state.RuntimeRegistered"));
    assert!(!client.contains("Capability = \"runtime.deploy\""));
    assert!(transport.contains("\"v1/retirement\""));
    assert!(window.contains("MaxAutomaticObservations = 30"));
    assert!(window.contains("LockIdentityFields()"));
    assert!(window.contains("new retirement ID"));
    assert!(window.contains("retirement-credential-handle"));
    assert!(window.contains("AutomationLiveSetting.Assertive"));
    assert!(hub.contains("hub-retire-gewyvern"));
    assert!(app.contains("ExecuteRetirementAsync"));
    assert!(app.contains("--verify-retirement-controls"));
    assert!(program.contains("--verify-retirement-client"));
}

#[test]
fn daemon_retirement_is_bootstrap_bound_authority_omitting_and_runtime_independent() {
    let client = avalonia_source("Leserpent.RemoteClient/RemoteDaemonRetirementClient.cs");
    let contracts = avalonia_source("Leserpent.RemoteClient/RemoteDaemonRetirementContracts.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let window = avalonia_source("Leserpent.Avalonia/DaemonRetirementWindow.cs");
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(client.contains("public sealed class RemoteDaemonRetirementClient"));
    assert!(client.contains("Capability = \"host.retire\""));
    assert!(client.contains("state.BootstrapId != expected.BootstrapId"));
    assert!(client.contains("state.RetirementId != expected.RetirementId"));
    assert!(client.contains("encoded.Contains(\"\\\"target\\\":\""));
    assert!(client.contains("encoded.Contains(\"\\\"daemon_id\\\":\""));
    assert!(client.contains("encoded.Contains(\"\\\"generation\\\":\""));
    assert!(client.contains("encoded.Contains(\"\\\"install_profile\\\":\""));
    assert!(!client.contains("Capability = \"runtime.retire\""));
    assert!(contracts.contains("JsonUnmappedMemberHandling.Disallow"));
    let request_intent = contracts
        .split("internal sealed class DaemonRetirementIntent")
        .nth(1)
        .expect("daemon retirement request intent must exist")
        .split("internal sealed class DaemonRetirementResponseEnvelope")
        .next()
        .expect("daemon retirement request intent must be bounded");
    assert!(!request_intent.contains("DaemonRetirementTarget"));
    assert!(!request_intent.contains("DaemonId"));
    assert!(!request_intent.contains("Generation"));
    assert!(!request_intent.contains("InstallProfile"));
    assert!(transport.contains("\"v1/daemon-retirement\""));
    assert!(window.contains("MaxAutomaticObservations = 30"));
    assert!(window.contains("LockIdentityFields()"));
    assert!(window.contains("new retirement ID"));
    assert!(window.contains("daemon-retirement-credential-handle"));
    assert!(window.contains("AutomationLiveSetting.Assertive"));
    assert!(!window.contains("private readonly TextBox host"));
    assert!(hub.contains("hub-retire-daemon"));
    assert!(app.contains("ExecuteDaemonRetirementAsync"));
    assert!(app.contains("--verify-daemon-retirement-controls"));
    assert!(program.contains("--verify-daemon-retirement-client"));
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
    let presentation =
        avalonia_source("Leserpent.RemoteClient/RemoteAuthorityHealthPresentation.cs");

    assert!(source.contains("remote-authority-health"));
    assert!(source.contains("remote-authority-health-refresh"));
    assert!(source.contains("AutomationLiveSetting.Assertive"));
    assert!(source.contains("RemoteAuthorityHealthPresentation.Create"));
    assert!(!source.contains("QUEUE SATURATED"));
    assert!(presentation.contains("public sealed record RemoteAuthorityHealthPresentation"));
    assert!(presentation.contains("QUEUE SATURATED"));
    assert!(presentation.contains("effect queue metrics unavailable"));
    assert!(!presentation.contains("Avalonia"));
    assert!(program.contains("--verify-authority-health-presentation"));
    assert!(program.contains("saturation_visible=true"));
}

#[test]
fn gui_mutations_export_canonical_leselang_without_execution() {
    let window = remote_main_window_source();
    let workspace = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let exporter = avalonia_source("Leserpent.RemoteClient/RemoteLeselangExport.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let control = avalonia_source("Leserpent.Avalonia/LeselangExportControl.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("leselangClient.ExportRefreshAsync"));
    assert!(window.contains("leselangClient.ExportDeployAsync"));
    assert!(workspace.contains("leselangClient.ExportWorkspaceAsync"));
    assert!(workspace.contains("runtime-workspace-leselang"));
    assert!(workspace.contains("Preview equivalent workspace Leselang"));
    assert!(workspace.contains("workspaceLeselangWindow.Activate()"));
    assert!(workspace.contains("new LeselangExportControl"));
    assert!(window.contains("new LeselangExportControl"));
    assert!(exporter.contains("public sealed class RemoteLeselangClient"));
    assert!(exporter.contains("RemoteLeselangExportException"));
    assert!(exporter.contains("JsonUnmappedMemberHandling.Disallow"));
    assert!(exporter.contains("PostLeselangExportAsync"));
    assert!(!exporter.contains("fn main()"));
    assert!(!exporter.contains("runtime.inspect("));
    assert!(!exporter.contains("runtime.deploy("));
    assert!(!exporter.contains("RemoteMutationClient(options"));
    assert!(transport.contains("\"v1/leselang-export\""));
    assert!(control.contains("Copy Leselang"));
    assert!(control.contains("No operation was executed."));
    assert!(control.contains("SetTextAsync(source)"));
    assert!(control.contains("ExportDebounce"));
    assert!(control.contains("no local template was substituted"));
    assert!(program.contains("--verify-leselang-gui-export"));
    assert!(program.contains("rust_authority=true"));
}

#[test]
fn runtime_workspace_log_filter_is_local_bounded_and_accessible() {
    let window = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let filter = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceLogFilter.cs");
    let export = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceDiagnosticExport.cs");
    let projection = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceDocumentProjection.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("runtime-log-search"));
    assert!(window.contains("runtime-log-level"));
    assert!(window.contains("runtime-log-filter-summary"));
    assert!(window.contains("runtime-diagnostics-copy"));
    assert!(window.contains("runtime-diagnostics-save"));
    assert!(window.contains("Save visible runtime diagnostics"));
    assert!(window.contains("Review it before sharing"));
    assert!(window.contains("SaveDiagnosticsAsync"));
    assert!(window.contains("storage is null || !storage.CanSave"));
    assert!(window.contains("SaveFilePickerAsync"));
    assert!(window.contains("ShowOverwritePrompt = true"));
    assert!(window.contains("!stream.CanWrite || !stream.CanSeek"));
    assert!(window.contains("stream.SetLength(0)"));
    assert!(window.contains("stream.Position = 0"));
    assert!(window.contains("RemoteWorkspaceDiagnosticExport.Encode(view)"));
    assert!(window.contains("Diagnostic save canceled."));
    assert!(window.contains("Diagnostic save failed safely."));
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
    assert!(filter.contains("public static class RemoteWorkspaceLogFilter"));
    assert!(!filter.contains("Avalonia"));
    assert!(export.contains("leserpent.workspace-diagnostic/v1"));
    assert!(export.contains("MaxUtf8Bytes = 512 * 1024"));
    assert!(export.contains("SuggestedFileName"));
    assert!(export.contains("char.IsAsciiLetterOrDigit"));
    assert!(export.contains("Encoding.UTF8.Preamble"));
    assert!(export.contains("MaxLogEntries"));
    assert!(export.contains("MaxLogDisplayBytes"));
    assert!(export.contains("command_id = "));
    assert!(!export.contains("new RemoteWorkspaceClient"));
    assert!(!export.contains("LoadAsync"));
    assert!(!export.contains("RemoteWireTransport"));
    assert!(!export.contains("RemoteMutationClient"));
    assert!(export.contains("public static class RemoteWorkspaceDiagnosticExport"));
    assert!(!export.contains("Avalonia"));
    assert!(projection.contains("No matching log entries"));
    assert!(projection.contains("Safe(entry.CommandId)"));
    assert!(projection.contains("public static class RemoteWorkspaceDocumentProjection"));
    assert!(!projection.contains("Avalonia"));
    assert!(program.contains("--verify-workspace-diagnostics"));
    assert!(program.contains("--verify-workspace-log-filter"));
    assert!(program.contains("local_only=true"));
    assert!(program.contains("explicit_export=true"));
    assert!(program.contains("file_export=true"));
    assert!(program.contains("maximal_escape=true"));
}

#[test]
fn runtime_workspace_live_refresh_is_explicit_single_flight_and_suspendable() {
    let window = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let policy = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceLiveRefresh.cs");
    let plan = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceLogRefreshPlan.cs");
    let client = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceClient.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("runtime-workspace-live-logs"));
    assert!(window.contains("Activated +="));
    assert!(window.contains("Deactivated +="));
    assert!(window.contains("if (!liveRefresh.TryBegin())"));
    assert!(window.contains("outcome == WorkspaceReloadOutcome.Skipped"));
    assert!(window.contains("liveRefresh.Defer(IsActive)"));
    assert!(window.contains("outcome == WorkspaceReloadOutcome.Loaded"));
    assert!(window.contains("liveRefresh.Pause();"));
    assert!(window.contains("liveRefreshTimer.Stop();"));
    assert!(
        window.contains("liveRefreshButton.IsEnabled = liveRefresh.IsRequested || !loadInFlight")
    );
    assert!(policy.contains("TimeSpan.FromSeconds(5)"));
    assert!(policy.contains("MaxConsecutiveFailures = 3"));
    assert!(policy.contains("TimeSpan.FromSeconds(10)"));
    assert!(policy.contains("TimeSpan.FromSeconds(20)"));
    assert!(policy.contains("State != WorkspaceLiveRefreshState.Waiting"));
    assert!(policy.contains("State == WorkspaceLiveRefreshState.Suspended"));
    assert!(policy.contains("live refresh lost its first bounded retry"));
    assert!(policy.contains("live refresh exceeded its bounded retry limit"));
    assert!(policy.contains("RecoverAfterExternalSuccess"));
    assert!(policy.contains("external query success did not reset live backoff"));
    assert!(policy.contains("deferred live query changed its backoff state"));
    assert!(window.contains("liveRefreshTimer.Interval = liveRefresh.NextInterval"));
    assert!(window.contains("liveRefreshTimer.Stop();\n        loadInFlight = true;"));
    assert!(window.contains("_ = liveRefresh.RecoverAfterExternalSuccess()"));
    assert!(window.contains("else if (!allowIncrementalLogs)"));
    assert!(window.contains("ShowLiveRefreshFailure"));
    assert!(!policy.contains("RemoteWorkspaceClient"));
    assert!(!policy.contains("RemoteWireTransport"));
    assert!(policy.contains("public sealed class RemoteWorkspaceLiveRefresh"));
    assert!(!policy.contains("Avalonia"));
    assert!(plan.contains("public sealed class RemoteWorkspaceLogRefreshPlan"));
    assert!(!plan.contains("Avalonia"));
    assert!(program.contains("live_refresh=true"));
    assert!(program.contains("bounded_retry=true"));
    assert!(program.contains("manual_recovery=true"));
    assert!(program.contains("skip_neutral=true"));
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
    let changes = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceSnapshotChange.cs");
    let alert = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceSeverityAlert.cs");
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
    assert!(changes.contains("public static class RemoteWorkspaceSnapshotChanges"));
    assert!(!changes.contains("Avalonia"));
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
    assert!(alert.contains("public sealed class RemoteWorkspaceSeverityAlert"));
    assert!(!alert.contains("Avalonia"));
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
        .find("private static DesktopConnectionProfile RequestedProfile")
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
    assert!(test_body.contains("manageCertificate: false"));
    assert!(test_body.contains("ResolveCertificateAuthorityPath"));
    assert!(!test_body.contains("profileStore"));
    assert!(!test_body.contains("RemoteTokenResolver.Store"));
    assert!(!test_body.contains(".Save("));
    assert!(!test_body.contains(".Import("));
}

#[test]
fn local_orchestra_is_a_bounded_rust_owned_desktop_session() {
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let supervisor = avalonia_source("Leserpent.Avalonia/LocalOrchestraServiceSupervisor.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let token_store = avalonia_source("Leserpent.RemoteClient/RemoteTokenStore.cs");
    let presentation = avalonia_source("Leserpent.Avalonia/RemoteCredentialPresentation.cs");

    assert!(app.contains("OpenLocalOrchestra"));
    assert!(app.contains("OpenLocalRuntimeWorkspace"));
    assert!(app.contains("LoadLocalTopologyAsync"));
    assert!(app.contains("new LocalOrchestraServiceSupervisor()"));
    assert!(supervisor.contains("DaemonExecutable = \"leserpentd\""));
    assert!(supervisor.contains("LESERPENT_REMOTE_TOKEN"));
    assert!(supervisor.contains("info.Environment.Clear()"));
    assert!(!supervisor.contains("GetEnvironmentVariable(\"PATH\")"));
    assert!(supervisor.contains("FileAttributes.ReparsePoint"));
    assert!(supervisor.contains("options.UnixCreateMode"));
    assert!(supervisor.contains("CryptographicOperations.ZeroMemory"));
    assert!(supervisor.contains("RemoteTokenSource.LocalProcess"));
    assert!(supervisor.contains("health.Status == \"ready\" && health.AuthorityOwned"));
    assert!(supervisor.contains("Kill(entireProcessTree: true)"));
    assert!(supervisor.contains("ObjectDisposedException.ThrowIf(disposed, this)"));
    assert!(token_store.contains("LocalProcess"));
    assert!(presentation.contains("TOKEN / LOCAL PROCESS"));
    assert!(program.contains("--verify-local-orchestra"));
    assert!(program.contains("owned_authority=true"));
    assert!(program.contains("private_files=true"));
    assert!(program.contains("minimal_child_environment=true"));
    assert!(program.contains("optional_bootstrap_origin=true"));
    assert!(program.contains("optional_gewyvern_provisioning_origin=true"));
    assert!(supervisor.contains("LESERPENT_GEWYVERN_PROVISIONING_CONFIG"));
    assert!(supervisor.contains("--gewyvern-provisioning-config"));
    assert!(app.contains("GewyvernProvisioningEnabled: true"));
    assert!(program.contains("package_local_daemon=true"));
    assert!(program.contains("symlink_rejection=true"));
    assert!(supervisor.contains("Directory.CreateSymbolicLink"));
    assert!(supervisor.contains("File.CreateSymbolicLink"));
}
