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
    let remote = remote_main_window_source();

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
    assert!(core.contains("UiPresentationOperationKind.OpenWindow"));
    assert!(core.contains("UiPresentationOperationKind.CloseWindow"));
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
    assert!(core.contains("UiPresentationOperationKind.SetSelection"));
    assert!(core.contains("UiPresentationAtom.SetSelection"));
    assert!(core.contains("UiPresentationOperationKind.AssertSelection"));
    assert!(core.contains("UiPresentationOperationKind.WaitSelection"));
    assert!(core.contains("UiSelectionState"));
    assert!(core.contains("WaitSelectionTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.AssertChildCount"));
    assert!(core.contains("UiPresentationOperationKind.WaitChildCount"));
    assert!(core.contains("public int? Count { get; set; }"));
    assert!(core.contains("WaitChildCountTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationValidation.InvalidExpectedChildCount"));
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
    assert!(core.contains("UiPresentationOperationKind.WaitAutomationId"));
    assert!(core.contains("UiPresentationAtom.WaitAutomationId"));
    assert!(core.contains("WaitAutomationIdTimeoutMs = 2000"));
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
    assert!(core.contains("UiPresentationOperationKind.WaitFormField"));
    assert!(core.contains("WaitFormFieldTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitFormFieldInputKind"));
    assert!(core.contains("WaitFormFieldInputKindTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitFormFieldRequired"));
    assert!(core.contains("WaitFormFieldRequiredTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitFormFieldMaxLength"));
    assert!(core.contains("WaitFormFieldMaxLengthTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.WaitFormFieldPlaceholder"));
    assert!(core.contains("WaitFormFieldPlaceholderTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationOperationKind.SetFormValue"));
    assert!(core.contains("UiPresentationOperationKind.AssertFormValue"));
    assert!(core.contains("UiPresentationOperationKind.WaitFormValue"));
    assert!(core.contains("UiPresentationOperationKind.SubmitForm"));
    assert!(core.contains("UiPresentationOperationKind.CancelForm"));
    assert!(core.contains("UiPresentationAtom.SetFormValue"));
    assert!(core.contains("UiPresentationAtom.SubmitForm"));
    assert!(core.contains("UiPresentationAtom.CancelForm"));
    assert!(core.contains("UiPresentationAtomFamily.FormValue"));
    assert!(core.contains("UiPresentationAtomFamily.FormLifecycle"));
    assert!(core.contains("public string? Value { get; set; }"));
    assert!(core.contains("WaitFormValueTimeoutMs = 2000"));
    assert!(core.contains("UiPresentationValidation.InvalidFormValue"));
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
    assert!(renderer.contains("ShowActivated = false"));
    assert!(renderer.contains("private PresentationAutomationResult OpenPresentationWindow"));
    assert!(renderer.contains("private PresentationAutomationResult ClosePresentationWindow"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetTextMismatch"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetAutomationIdMismatch"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitAutomationId"));
    assert!(renderer.contains("SemanticRenderer.WaitAutomationIdTimeoutMs"));
    assert!(renderer.contains("SetAutomationIdForVerification"));
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
    assert!(renderer.contains("UiPresentationOperationKind.WaitFormField"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitFormFieldInputKind"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitFormFieldRequired"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitFormFieldMaxLength"));
    assert!(renderer.contains("IReadOnlyDictionary<string, string>? FormFieldLabels"));
    assert!(renderer.contains("IReadOnlyDictionary<string, int>? FormFieldMaxLengths"));
    assert!(renderer.contains("public IDisposable RegisterFormFields"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetFormFieldUnrealized"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetFormUnrealized"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetFormActionUnavailable"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetFormValueMismatch"));
    assert!(renderer.contains("input!.Text = operation.Value"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitFormValue"));
    assert!(renderer.contains("SemanticRenderer.WaitFormValueTimeoutMs"));
    assert!(renderer.contains("ApplyFormLifecyclePresentation"));
    assert!(renderer.contains("registration.SubmitButton"));
    assert!(renderer.contains("registration.CancelButton"));
    assert!(renderer.contains("var observedClicks = 0"));
    assert!(renderer.contains("observedClicks == 1"));
    assert!(renderer.contains("PresentationAutomationFailureCode.InvalidExpectedKind"));
    assert!(renderer.contains("PresentationAutomationFailureCode.InvalidExpectedActionKind"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetAccessibleNameMismatch"));
    assert!(
        renderer.contains("PresentationAutomationFailureCode.TargetAccessibleDescriptionMismatch")
    );
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetSelectionMismatch"));
    assert!(renderer.contains("operation.Kind == UiPresentationOperationKind.SetSelection"));
    assert!(renderer.contains("item.IsSelected = operation.State == UiSelectionState.Selected"));
    assert!(renderer.contains("NativeSelectionState"));
    assert!(renderer.contains("ListBoxItem"));
    assert!(renderer.contains("list.SelectedItem = item"));
    assert!(renderer.contains("control.IsEffectivelyVisible"));
    assert!(renderer.contains("control!.IsEffectivelyEnabled"));
    assert!(renderer.contains("UiPresentationOperationKind.Activate"));
    assert!(renderer.contains("button.RaiseEvent(new RoutedEventArgs(Button.ClickEvent))"));
    assert!(renderer.contains("!IsControlVisibleInSurface(button)"));
    assert!(renderer.contains("!button.IsEffectivelyEnabled"));
    assert!(renderer.contains("SharedSurfaceWindow(control) is { IsVisible: true }"));
    assert!(renderer.contains("PresentationWindowGenerationCount++"));
    assert!(renderer.contains("PresentationTreeRematerializationCount++"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetWindowStillOpen"));
    assert!(renderer.contains("RecreateDetachedSurface(window)"));
    assert!(renderer.contains("presenter.UpdateChild()"));
    assert!(renderer.contains("Mount(document)"));
    assert!(renderer.contains("StringComparer.Ordinal.Equals(actual, operation.Expected)"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitText"));
    assert!(renderer.contains("WaitTextTimeoutMs"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitAccessibleName"));
    assert!(renderer.contains("WaitAccessibleNameTimeoutMs"));
    assert!(renderer.contains("UiPresentationOperationKind.WaitAccessibleDescription"));
    assert!(renderer.contains("WaitAccessibleDescriptionTimeoutMs"));
    assert!(renderer.contains("node.Children.Count == operation.Count"));
    assert!(renderer.contains("PresentationAutomationFailureCode.TargetChildCountMismatch"));
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
    assert!(window.contains("ProbeAutomationIdWaitAsync"));
    assert!(window.contains("AutomationIdWaitCompleted"));
    assert!(window.contains("AutomationIdWaitTimedOut"));
    assert!(window.contains("AutomationIdWaitDidNotTransferFocus"));
    assert!(window.contains("AutomationIdWaitDidNotActivate"));
    assert!(window.contains("ChildCountAssertCompleted"));
    assert!(window.contains("InitialChildCountWaitCompleted"));
    assert!(window.contains("InitialChildCountWaitTimedOut"));
    assert!(window.contains("ChildCountObservationPreservedVirtualization"));
    assert!(window.contains("ActionActivationCompleted"));
    assert!(window.contains("ActionActivationExactlyOnce"));
    assert!(window.contains("UnavailableActionActivationRejected"));
    assert!(window.contains("HiddenActionActivationRejected"));
    assert!(window.contains("NonActionActivationRejected"));
    assert!(window.contains("MissingActionActivationRejected"));
    assert!(window.contains("WindowReopenMutationCompleted"));
    assert!(window.contains("WindowRecloseMutationCompleted"));
    assert!(window.contains("WindowLifecycleIdempotent"));
    assert!(window.contains("WindowLifecycleUsedFreshNativeWindow"));
    assert!(window.contains("WindowLifecycleRematerializedSemanticTree"));
    assert!(window.contains("generationCountAfterDuplicateOpen == 2"));
    assert!(window.contains("PresentationTreeRematerializationCount == 2"));
    assert!(window.contains("DispatcherPriority.Render"));
    assert!(window.contains("if (!presentationProbesEnabled)"));
    assert!(window.contains("ConfigureWindowContent();"));
    assert!(window.contains("RequirePresentationProbes();"));
    assert!(window.contains("() => childCountRenderer.Apply(childCountPatch)"));
    assert!(window.contains("InitialAccessibleNameWaitCompleted"));
    assert!(window.contains("InitialAccessibleNameWaitTimedOut"));
    assert!(window.contains("PatchTextFallback("));
    assert!(window.contains("var actionUnavailableReasonWait = renderer.ApplyPresentationAsync"));
    assert!(window.contains("ActionUnavailableReasonWaitCompleted"));
    assert!(window.contains("ActionUnavailableReasonWaitClearedCompleted"));
    assert!(window.contains("ActionUnavailableReasonWaitTimedOut"));
    assert!(window.contains("InitialFormFieldWaitCompleted"));
    assert!(window.contains("InitialFormFieldInputKindWaitCompleted"));
    assert!(window.contains("InitialFormFieldRequiredWaitCompleted"));
    assert!(window.contains("InitialFormFieldMaxLengthWaitCompleted"));
    assert!(window.contains("PatchFormFieldLabel("));
    assert!(window.contains("PatchFormFieldInputKind("));
    assert!(window.contains("PatchFormFieldRequired("));
    assert!(window.contains("PatchFormFieldMaxLength("));
    assert!(window.contains("var focusedWait = renderer.ApplyPresentationAsync"));
    assert!(window.contains("var unfocusedAssertBaseline = renderer.ApplyPresentation"));
    assert!(
        window
            .contains("Leselang unfocused assertion probe could not establish its focus baseline")
    );
    assert!(window.contains("var unfocusedTimeoutBaseline = renderer.ApplyPresentation"));
    assert!(window.contains("InitialUnfocusedWaitObservedExternalDeactivation"));
    assert!(window.contains("var focusedTimeoutResult = await renderer.ApplyPresentationAsync"));
    assert!(window.contains("renderer.ApplyPresentation(new UiPresentationOperation"));
    assert!(app.contains("leselang_presentation=true"));
    assert!(app.contains("Avalonia action activation valid:"));
    assert!(app.contains("presentation_activate="));
    assert!(app.contains("native_click_exactly_once="));
    assert!(app.contains("unavailable_action_rejected="));
    assert!(app.contains("hidden_action_rejected="));
    assert!(app.contains("non_action_rejected="));
    assert!(app.contains("missing_action_rejected="));
    assert!(app.contains("new MainWindow(fixture, verifyFocusRetention)"));
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
    assert!(app.contains("assert_child_count="));
    assert!(app.contains("wait_child_count_external_patch="));
    assert!(app.contains("wait_child_count_timeout="));
    assert!(app.contains("child_count_virtualization_preserved="));
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
    assert!(app.contains("open_window="));
    assert!(app.contains("close_window="));
    assert!(app.contains("reopen_window="));
    assert!(app.contains("reclose_window="));
    assert!(app.contains("window_lifecycle_idempotent="));
    assert!(app.contains("window_reopen_fresh_native_window="));
    assert!(app.contains("window_semantic_tree_rematerialized="));
    assert!(app.contains("window_lifecycle_state_observed="));
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
    assert!(app.contains("wait_unfocused_external_deactivation="));
    assert!(app.contains("Avalonia selection mutation valid:"));
    assert!(app.contains("set_selection_idempotent="));
    assert!(app.contains("set_selection_reversible="));
    assert!(app.contains("set_selection_no_activation="));
    assert!(app.contains("set_selection_focus_preserved="));
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
    assert!(app.contains("wait_form_field="));
    assert!(app.contains("wait_form_field_timeout="));
    assert!(app.contains("wait_form_field_input_kind="));
    assert!(app.contains("wait_form_field_input_kind_timeout="));
    assert!(app.contains("wait_form_field_required="));
    assert!(app.contains("wait_form_field_required_timeout="));
    assert!(app.contains("wait_form_field_max_length="));
    assert!(app.contains("wait_form_field_max_length_timeout="));
    assert!(app.contains("wait_form_field_placeholder="));
    assert!(app.contains("wait_form_field_placeholder_timeout="));
    assert!(app.contains("set_form_value="));
    assert!(app.contains("set_form_value_idempotent="));
    assert!(app.contains("set_form_value_no_activation="));
    assert!(app.contains("set_form_value_focus_preserved="));
    assert!(app.contains("assert_form_value="));
    assert!(app.contains("form_value_mismatch_rejected="));
    assert!(app.contains("wait_form_value="));
    assert!(app.contains("wait_form_value_external_transition="));
    assert!(app.contains("wait_form_value_timeout="));
    assert!(app.contains("form_value_unregistered_rejected="));
    assert!(app.contains("form_value_scope_disposed="));
    assert!(app.contains("assert_automation_id=true"));
    assert!(app.contains("wait_automation_id="));
    assert!(app.contains("wait_automation_id_external_transition="));
    assert!(app.contains("wait_automation_id_timeout="));
    assert!(app.contains("wait_automation_id_no_focus_transfer="));
    assert!(app.contains("wait_automation_id_no_activation="));
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
    assert!(conformance.contains("presentation_open_window=true"));
    assert!(conformance.contains("presentation_close_window=true"));
    assert!(conformance.contains("presentation_assert_window_open=true"));
    assert!(conformance.contains("presentation_wait_window_open=true"));
    assert!(conformance.contains("presentation_assert_window_closed=true"));
    assert!(conformance.contains("presentation_wait_window_closed=true"));
    assert!(conformance.contains("presentation_wait_focused=true"));
    assert!(conformance.contains("presentation_assert_unfocused=true"));
    assert!(conformance.contains("presentation_wait_unfocused=true"));
    assert!(conformance.contains("presentation_set_selection=true"));
    assert!(conformance.contains("presentation_assert_selection=true"));
    assert!(conformance.contains("presentation_wait_selection=true"));
    assert!(conformance.contains("presentation_assert_child_count=true"));
    assert!(conformance.contains("presentation_wait_child_count=true"));
    assert!(conformance.contains("presentation_assert_disabled=true"));
    assert!(conformance.contains("presentation_assert_automation_id=true"));
    assert!(conformance.contains("presentation_wait_automation_id=true"));
    assert!(conformance.contains("presentation_assert_node_kind=true"));
    assert!(conformance.contains("presentation_wait_node_kind=true"));
    assert!(conformance.contains("presentation_wait_text=true"));
    assert!(conformance.contains("presentation_wait_accessible_name=true"));
    assert!(conformance.contains("presentation_wait_accessible_description=true"));
    assert!(conformance.contains("presentation_wait_form_field=true"));
    assert!(conformance.contains("presentation_wait_form_field_input_kind=true"));
    assert!(conformance.contains("presentation_wait_form_field_required=true"));
    assert!(conformance.contains("presentation_wait_form_field_max_length=true"));
    assert!(conformance.contains("presentation_wait_form_field_placeholder=true"));
    assert!(conformance.contains("presentation_set_form_value=true"));
    assert!(conformance.contains("presentation_assert_form_value=true"));
    assert!(conformance.contains("presentation_wait_form_value=true"));
    assert!(conformance.contains("presentation_submit_form=true"));
    assert!(conformance.contains("presentation_cancel_form=true"));
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
    assert!(window.contains("ProbeFormValueAutomationAsync"));
    assert!(window.contains("duplicateRegistrationRejected"));
    assert!(window.contains("FormValueProbeDidNotActivate"));
    assert!(window.contains("FormValueProbePreservedFocus"));
    assert!(window.contains("FormSubmitExactlyOnce"));
    assert!(window.contains("FormSubmitDisabledRejected"));
    assert!(window.contains("FormSubmitClosedReplayRejected"));
    assert!(window.contains("FormCancelExactlyOnce"));
    assert!(window.contains("FormCancelClosedReplayRejected"));
    assert!(app.contains("Avalonia form lifecycle valid:"));
    assert!(app.contains("form_lifecycle_unregistered_rejected="));
    assert!(remote.contains("using var formRegistration = sourceRenderer.RegisterFormFields"));
    assert!(remote.contains("formWindow.FormFields"));
    assert!(remote.contains("formWindow.SubmitButton"));
    assert!(remote.contains("formWindow.CancelButton"));
}

#[test]
fn workspace_policies_are_renderer_independent_and_mobile_consumable() {
    let remote_project = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/Leserpent.RemoteClient.csproj",
    );
    let fleet_projection = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteDocumentProjection.cs",
    );
    let runtime_search =
        repo_source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteRuntimeSearch.cs");
    let topology_refresh = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteTopologyRefreshCoordinator.cs",
    );
    let workspace_projection = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteWorkspaceDocumentProjection.cs",
    );
    let mutation_fences =
        repo_source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteMutationFences.cs");
    let authority_policy = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteFeedAuthorityPolicy.cs",
    );
    let mutation_availability = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteMutationAvailability.cs",
    );
    let mutation_coordinator = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteMutationCoordinator.cs",
    );
    let mutation_failure =
        repo_source("apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteMutationFailure.cs");
    let health_coordinator = repo_source(
        "apps/leserpent-avalonia/src/Leserpent.RemoteClient/RemoteAuthorityHealthCoordinator.cs",
    );
    let remote_window =
        repo_source("apps/leserpent-avalonia/src/Leserpent.Avalonia/RemoteMainWindow.cs");
    let remote_conformance =
        repo_source("apps/leserpent-avalonia/src/Leserpent.RemoteConformance/Program.cs");
    let mobile_project =
        repo_source("apps/leserpent-mobile/src/Leserpent.MobileCore/Leserpent.MobileCore.csproj");
    let mobile_conformance =
        repo_source("apps/leserpent-mobile/src/Leserpent.MobileConformance/Program.cs");

    assert!(remote_project.contains("Leserpent.RendererCore.csproj"));
    assert!(fleet_projection.contains("public static class RemoteDocumentProjection"));
    assert!(fleet_projection.contains("public sealed record RemoteDocumentView"));
    assert!(fleet_projection.contains("RemoteRuntimeSearch.Matches"));
    assert!(!fleet_projection.contains("Avalonia"));
    assert!(runtime_search.contains("public static class RemoteRuntimeSearch"));
    assert!(runtime_search.contains("public const int MaxFilterLength = 128"));
    assert!(runtime_search.contains("public static RemoteTopologySearchResult FilterTopology"));
    assert!(runtime_search.contains("VisibleAuthorityIds"));
    assert!(!runtime_search.contains("Avalonia"));
    assert!(topology_refresh.contains("public sealed class RemoteTopologyRefreshCoordinator"));
    assert!(topology_refresh.contains("public const int DefaultMaxConcurrency = 4"));
    assert!(topology_refresh.contains("public const int MaxAuthorityCount = 65"));
    assert!(topology_refresh.contains("AuthorityCount: MaxAuthorityCount"));
    assert!(
        topology_refresh.contains("topology coordinator accepted an oversized authority fleet")
    );
    assert!(topology_refresh.contains("TaskCreationOptions.RunContinuationsAsynchronously"));
    assert!(topology_refresh.contains("topology refresh returned a non-terminal phase"));
    assert!(!topology_refresh.contains("Avalonia"));
    assert!(workspace_projection.contains("public static class RemoteWorkspaceDocumentProjection"));
    assert!(!workspace_projection.contains("Avalonia"));
    assert!(mutation_fences.contains("public static class RemoteMutationFences"));
    assert!(mutation_fences.contains("public sealed record RemoteMutationRevisionFence"));
    assert!(mutation_fences.contains("public sealed record RemoteMutationObservationFence"));
    assert!(!mutation_fences.contains("Avalonia"));
    assert!(authority_policy.contains("public static class RemoteFeedAuthorityPolicy"));
    assert!(authority_policy.contains("state.SnapshotGeneration > 0"));
    assert!(authority_policy.contains("revision >= snapshotRevision"));
    assert!(authority_policy.contains("heartbeatOnly"));
    assert!(!authority_policy.contains("Avalonia"));
    assert!(mutation_availability.contains("public static class RemoteMutationAvailabilityPolicy"));
    assert!(mutation_availability.contains("public sealed record RemoteMutationAvailability"));
    assert!(mutation_availability.contains("RemoteFeedAuthorityPolicy.HasAuthoritativeSnapshot"));
    assert!(
        mutation_availability.contains("Remote changes require a generated authoritative snapshot")
    );
    assert!(!mutation_availability.contains("Avalonia"));
    assert!(mutation_coordinator.contains("public sealed class RemoteMutationCoordinator"));
    assert!(mutation_coordinator.contains("public RemoteMutationAdmission Begin("));
    assert!(mutation_coordinator.contains("public RemoteMutationAdmission Confirm("));
    assert!(mutation_coordinator.contains("public RemoteMutationFailure CompleteFailure("));
    assert!(mutation_coordinator.contains("RemoteMutationFailurePolicy.Classify("));
    assert!(mutation_coordinator.contains("RemoteMutationFailurePolicy.VerifyContract();"));
    assert!(mutation_coordinator.contains("public void MarkUnknown("));
    assert!(
        mutation_coordinator
            .contains("malformed mutation response did not become an unknown outcome")
    );
    assert!(mutation_coordinator.contains("heartbeat-only state admitted a mutation"));
    assert!(
        mutation_coordinator.contains("retired mutation token cleared current operation ownership")
    );
    assert!(
        mutation_coordinator
            .contains("retired mutation failure disturbed current operation ownership")
    );
    assert!(!mutation_coordinator.contains("Avalonia"));
    assert!(mutation_failure.contains("public static class RemoteMutationFailurePolicy"));
    assert!(mutation_failure.contains("RemoteMutationFailureDisposition.UnknownOutcome"));
    assert!(mutation_failure.contains("RemoteMutationFailureDisposition.Ignored"));
    assert!(mutation_failure.contains("public const int MaxOperatorMessageLength = 320"));
    assert!(mutation_failure.contains("unexpected failure exposed exception detail"));
    assert!(!mutation_failure.contains("Avalonia"));
    assert!(health_coordinator.contains("public sealed class RemoteAuthorityHealthCoordinator"));
    assert!(health_coordinator.contains("public Task<RemoteAuthorityHealthState> RefreshAsync("));
    assert!(health_coordinator.contains("RemoteAuthorityHealthPhase.Checking"));
    assert!(health_coordinator.contains("RemoteAuthorityHealthFailure.InvalidResponse"));
    assert!(
        health_coordinator
            .contains("authority health coordinator did not preserve single-flight ownership")
    );
    assert!(
        health_coordinator.contains("retired authority health completion crossed the stop fence")
    );
    assert!(!health_coordinator.contains("Avalonia"));
    assert!(
        remote_window.contains("private readonly RemoteMutationCoordinator mutationCoordinator")
    );
    assert!(remote_window.contains("mutationCoordinator.Begin("));
    assert!(remote_window.contains("mutationCoordinator.Confirm(operation, currentState)"));
    assert!(remote_window.contains("mutationCoordinator.Accept(operation, result, currentState)"));
    assert!(remote_window.contains("mutationCoordinator.Abandon(operation, currentState)"));
    assert_eq!(
        remote_window
            .matches("ApplyMutationFailure(operation, error);")
            .count(),
        2,
        "refresh and deployment failures must enter one shared classifier"
    );
    assert!(remote_window.contains("mutationCoordinator.CompleteFailure("));
    assert!(!remote_window.contains("catch (RemoteMutationException"));
    assert!(!remote_window.contains("catch (HttpRequestException"));
    assert!(!remote_window.contains("mutationCoordinator.MarkUnknown(operation"));
    assert!(!remote_window.contains("mutationCoordinator.RejectKnown(operation"));
    assert!(!remote_window.contains("private bool mutationInFlight"));
    assert!(!remote_window.contains("mutationRevisionFence"));
    assert!(!remote_window.contains("mutationObservationFence"));
    assert!(!remote_window.contains("RemoteMutationFences.SatisfiesRevision"));
    assert!(!remote_window.contains("RemoteMutationFences.SatisfiesObservation"));
    assert!(!remote_window.contains("RemoteMutationAvailabilityPolicy.Evaluate"));
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
    assert!(remote_conformance.contains("RemoteRuntimeSearch.VerifyContract()"));
    assert!(remote_conformance.contains("runtime_search=true"));
    assert!(
        remote_conformance.contains("await RemoteTopologyRefreshCoordinator.VerifyContractAsync()")
    );
    assert!(remote_conformance.contains("topology_refresh_coordination=true"));
    assert!(remote_conformance.contains("RemoteMutationCoordinator.VerifyContract()"));
    assert!(remote_conformance.contains("mutation_coordination=true"));
    assert!(remote_conformance.contains("cached_heartbeat_mutation=false"));
    assert!(remote_conformance.contains("malformed_mutation_response_unknown=true"));
    assert!(remote_conformance.contains("shared_failure_classification=true"));
    assert!(remote_conformance.contains("stale_failure_ignored=true"));
    assert!(remote_conformance.contains("bounded_failure_diagnostics=true"));
    assert!(
        remote_conformance.contains("await RemoteAuthorityHealthCoordinator.VerifyContractAsync()")
    );
    assert!(remote_conformance.contains("authority_health_coordination=true"));
    assert!(remote_conformance.contains("health_single_flight=true"));
    assert!(remote_conformance.contains("health_stop_fence=true"));
    assert!(mobile_project.contains("Leserpent.RemoteClient.csproj"));
    assert!(mobile_conformance.contains("RemoteWorkspaceLogFilter.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceDiagnosticExport.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceLiveRefresh.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceLogRefreshPlan.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceSeverityAlert.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteWorkspaceSnapshotChanges.VerifyContract()"));
    assert!(mobile_conformance.contains("RemoteDocumentProjection.VerifyFilterContract()"));
    assert!(mobile_conformance.contains("RemoteRuntimeSearch.VerifyContract()"));
    assert!(mobile_conformance.contains("runtime_search=true"));
    assert!(
        mobile_conformance.contains("await RemoteTopologyRefreshCoordinator.VerifyContractAsync()")
    );
    assert!(mobile_conformance.contains("topology_refresh=true"));
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
    assert!(mobile_conformance.contains("RemoteMutationCoordinator.VerifyContract()"));
    assert!(mobile_conformance.contains("mutation_coordination=true"));
    assert!(mobile_conformance.contains("cached_heartbeat_mutation=false"));
    assert!(mobile_conformance.contains("shared_failure_classification=true"));
    assert!(mobile_conformance.contains("stale_failure_ignored=true"));
    assert!(mobile_conformance.contains("bounded_failure_diagnostics=true"));
    assert!(mobile_conformance.contains("action_availability=true"));
    assert!(mobile_conformance.contains("RemoteAuthorityHealthPresentation.VerifyContract()"));
    assert!(
        mobile_conformance.contains("await RemoteAuthorityHealthCoordinator.VerifyContractAsync()")
    );
    assert!(mobile_conformance.contains("authority_health=true"));
    assert!(mobile_conformance.contains("authority_health_coordination=true"));
    assert!(mobile_conformance.contains("health_single_flight=true"));
    assert!(mobile_conformance.contains("health_stop_fence=true"));
}

#[test]
fn remote_event_lifecycle_is_shared_idempotent_and_subscriber_isolated() {
    let client = avalonia_source("Leserpent.RemoteClient/RemoteEventClient.cs");
    let lifecycle = avalonia_source("Leserpent.RemoteClient/RemoteEventLifecycle.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let remote_conformance = avalonia_source("Leserpent.RemoteConformance/Program.cs");
    let mobile_conformance =
        repo_source("apps/leserpent-mobile/src/Leserpent.MobileConformance/Program.cs");

    assert!(lifecycle.contains("internal sealed class RemoteEventRun"));
    assert!(lifecycle.contains("private RemoteEventRun? activeRun"));
    assert!(lifecycle.contains("ReferenceEquals(activeRun, previous)"));
    assert!(lifecycle.contains("private Task? disposalTask"));
    assert!(lifecycle.contains("var shutdowns = new Task[16]"));
    assert!(lifecycle.contains("Parallel.For("));
    assert!(lifecycle.contains("releaseCount != 1"));
    assert!(lifecycle.contains("subscribers.GetInvocationList()"));
    assert!(lifecycle.contains("public int SubscriberFailureCount"));
    assert!(lifecycle.contains("current == int.MaxValue"));
    assert!(lifecycle.contains("publisher.Clear()"));
    assert!(!lifecycle.contains("Avalonia"));

    assert!(client.contains("private readonly RemoteEventLifecycle lifecycle"));
    assert!(client.contains("previous.Task.WaitAsync(cancellationToken)"));
    assert!(client.contains("lifecycle.Restart(previous"));
    assert!(client.contains("new(lifecycle.DisposeAsync())"));
    assert!(client.contains("publisher.Publish(state)"));
    assert!(client.contains("publisher.Clear()"));
    assert!(!client.contains("private readonly CancellationTokenSource shutdown"));
    assert!(!client.contains("private Task? runTask"));
    assert!(!client.contains("StateChanged?.Invoke"));

    assert!(program.contains("--verify-remote-event-lifecycle"));
    for marker in [
        "event_dispose_single_flight=true",
        "event_resource_release_once=true",
        "event_restart_identity=true",
        "subscriber_failure_isolated=true",
        "subscriber_failure_count_bounded=true",
    ] {
        assert!(
            remote_conformance.contains(marker),
            "remote conformance is missing {marker}"
        );
        assert!(
            mobile_conformance.contains(marker),
            "mobile conformance is missing {marker}"
        );
    }
}

#[test]
fn remote_ui_actions_route_by_typed_binding_and_preserve_workspace_source() {
    let router = avalonia_source("Leserpent.RemoteClient/RemoteUiActionRouter.cs");
    let renderer = avalonia_source("Leserpent.Avalonia/AvaloniaDocumentRenderer.cs");
    let window = remote_main_window_source();
    let workspace = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let remote_conformance = avalonia_source("Leserpent.RemoteConformance/Program.cs");
    let mobile_conformance =
        repo_source("apps/leserpent-mobile/src/Leserpent.MobileConformance/Program.cs");

    assert!(router.contains("public static class RemoteUiActionRouter"));
    assert!(router.contains("ResolveActivation("));
    assert!(router.contains("ResolveSubmission("));
    assert!(router.contains("opaque-action-control"));
    assert!(router.contains("opaque-deployment-control"));
    assert!(router.contains("StringComparer.Ordinal.Equals(action.RuntimeId, location.RuntimeId)"));
    assert!(router.contains("RemoteMutationAvailabilityPolicy.Evaluate("));
    assert!(router.contains("MaxOperatorReasonLength = 320"));
    assert!(!router.contains("Avalonia"));

    assert!(renderer.contains("internal sealed record RenderedActionInvocation("));
    assert!(renderer.contains("AvaloniaDocumentRenderer Source"));
    assert!(renderer.contains("actionInvoked(new(this, node.Id))"));
    assert!(workspace.contains("Action<RenderedActionInvocation> actionInvoked"));
    assert!(workspace.contains("internal bool OwnsActionSource("));
    assert!(window.contains("invocation.Source.Document"));
    assert!(window.contains("RemoteUiActionRouter.ResolveActivation("));
    assert!(window.contains("RemoteUiActionRouter.ResolveSubmission("));
    assert!(window.contains("sourceRenderer.RegisterFormFields("));
    assert!(window.contains("sourceRenderer.CreateFormSubmission("));
    assert!(window.contains("IsActiveActionSource(invocation.Source)"));
    assert!(window.contains("workspace.OwnsActionSource(source)"));
    assert!(!window.contains("nodeId == $\"runtime:{candidate.Id}:inspect\""));
    assert!(!window.contains("nodeId == $\"runtime:{candidate.Id}:refresh\""));
    assert!(!window.contains("FindNode(renderer.Document.Root, nodeId)"));
    assert!(!window.contains("submission.Values.TryGetValue(\"pipeline_kind\""));

    assert!(program.contains("--verify-remote-ui-action-routing"));
    for marker in [
        "typed_ui_action_routing=true",
        "opaque_action_node_ids=true",
        "deployment_submission_source_fence=true",
    ] {
        assert!(
            remote_conformance.contains(marker),
            "remote conformance is missing {marker}"
        );
        assert!(
            mobile_conformance.contains(marker),
            "mobile conformance is missing {marker}"
        );
    }
}

#[test]
fn hub_topology_filter_is_bounded_keyboard_accessible_and_renderer_neutral() {
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let search = avalonia_source("Leserpent.RemoteClient/RemoteRuntimeSearch.cs");

    for automation_id in [
        "hub-topology-filter",
        "hub-topology-filter-clear",
        "hub-topology-filter-summary",
    ] {
        assert!(
            hub.contains(automation_id),
            "missing Hub control {automation_id}"
        );
    }
    assert!(hub.contains("RemoteRuntimeSearch.FilterTopology"));
    assert!(hub.contains("KeyModifiers.Control | KeyModifiers.Meta"));
    assert!(hub.contains("eventArgs.Key == Key.F5"));
    assert!(hub.contains("VisibleAuthorityIds.Contains"));
    assert!(hub.contains("ProbeTopologyFilter"));
    assert!(!hub.contains("runtime.Name.Contains("));
    assert!(search.contains("public const int MaxFilterLength = 128"));
    assert!(search.contains("AuthorityValues(authority)"));
    assert!(search.contains("RuntimeValues(runtime)"));
    assert!(search.contains("unique non-empty identities"));
    assert!(!search.contains("Avalonia"));
    assert!(app.contains("topology_filter=true"));
    assert!(app.contains("cross_authority_runtime_filter=true"));
    assert!(app.contains("empty_filter_state=true"));
    assert!(app.contains("filter_focus_recovery=true"));
}

#[test]
fn desktop_tutorial_is_offline_accessible_and_ui_reachable() {
    let tutorial = avalonia_source("Leserpent.Avalonia/DesktopTutorialWindow.cs");
    let tutorial_catalog = avalonia_source("Leserpent.Avalonia/DesktopTutorialCatalogs.cs");
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let lifecycle = avalonia_source("Leserpent.Avalonia/DesktopApplicationLifecycle.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let readme = repo_source("apps/leserpent-avalonia/README.md");
    let first_run = repo_source("docs/book/tutorial-first-run.md");

    for automation_id in [
        "desktop-tutorial-progress",
        "desktop-tutorial-previous",
        "desktop-tutorial-next",
        "desktop-tutorial-close",
        "desktop-tutorial-step-",
    ] {
        assert!(
            tutorial.contains(automation_id),
            "missing tutorial control {automation_id}"
        );
    }
    assert!(!tutorial.contains("EnglishSteps"));
    assert!(!tutorial.contains("SimplifiedChineseSteps"));
    assert!(tutorial.contains("DesktopTutorialCatalogs.Steps("));
    assert!(tutorial_catalog.contains("public const int KeyCount = 61"));
    assert_eq!(tutorial_catalog.matches("new(\"").count(), 61);
    for marker in [
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
        "DesktopDomainCatalogContract.Verify(",
        "a11y.progress.current",
        "step.1.title",
        "step.6.point.4",
    ] {
        assert!(
            tutorial_catalog.contains(marker),
            "tutorial catalog is missing {marker}"
        );
    }
    assert!(tutorial.contains("steps.Length != 6"));
    assert!(tutorial.contains("VerifyAccessibility()"));
    assert!(tutorial.contains("VerifyLayoutEnvelope()"));
    assert!(tutorial.contains("ProbeLocalizedPresentation()"));
    assert!(tutorial.contains("ProbeNavigationContract()"));
    assert!(tutorial.contains("TutorialText(\"a11y.next.finish\")"));
    assert!(tutorial.contains("Key.Left"));
    assert!(tutorial.contains("Key.Right"));
    assert!(tutorial.contains("Key.Home"));
    assert!(tutorial.contains("Key.End"));
    assert!(tutorial.contains("Key.Escape"));
    assert!(tutorial.contains("LeserpentTheme.MinimumTextContrastRatio < 4.5"));
    assert!(!tutorial.contains("HttpClient"));
    assert!(!tutorial.contains("Process.Start"));
    assert!(!tutorial.contains("RemoteMutation"));
    assert!(!tutorial.contains("File."));

    assert!(hub.contains("hub-open-tutorial"));
    assert!(hub.contains("eventArgs.Key == Key.F1"));
    assert!(hub.contains("ProbeTutorialEntry()"));
    assert!(app.contains("--verify-desktop-tutorial"));
    assert!(app.contains("tutorial_entry=true"));
    assert!(app.contains("desktop tutorial valid:"));
    assert!(app.contains("localized_tutorial_catalogs=7"));
    assert!(app.contains("tutorial_semantic_keys=61"));
    assert!(app.contains("localized_layouts=8"));
    assert!(app.contains("localized_step_layouts=48"));
    assert!(app.contains("localized_accessibility=true"));
    assert!(app.contains("live_language_reprojection=true"));
    assert!(lifecycle.contains("Learning Center..."));
    assert!(lifecycle.contains("desktop.Windows.OfType<DesktopTutorialWindow>()"));
    assert!(lifecycle.contains("window is not DesktopTutorialWindow"));
    assert!(program.contains("offline_tutorial=true"));
    assert!(program.contains("builtin_tutorial_catalogs=7"));
    assert!(program.contains("builtin_semantic_keys=750"));
    assert!(program.contains("builtin_tutorial_complete=true"));
    assert!(readme.contains("`--verify-desktop-tutorial`"));
    assert!(first_run.contains("`Learning Center...`"));
}

#[test]
fn desktop_language_selection_is_persistent_bounded_and_ui_ir_aware() {
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
    let built_in = avalonia_source("Leserpent.Avalonia/DesktopBuiltInShellCatalogs.cs");
    let semantic = avalonia_source("Leserpent.Avalonia/DesktopBuiltInSemanticCatalogs.cs");
    let store = avalonia_source("Leserpent.Avalonia/DesktopLanguagePreferenceStore.cs");
    let pack_store = avalonia_source("Leserpent.Avalonia/DesktopLanguagePackStore.cs");
    let pack_catalog = avalonia_source("Leserpent.Avalonia/DesktopLanguagePackCatalogClient.cs");
    let window = avalonia_source("Leserpent.Avalonia/DesktopLanguageWindow.cs");
    let renderer = avalonia_source("Leserpent.Avalonia/AvaloniaDocumentRenderer.cs");
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let lifecycle = avalonia_source("Leserpent.Avalonia/DesktopApplicationLifecycle.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let pack_generator = repo_source("apps/leserpent/scripts/build-language-packs.mjs");
    let pack_coverage = repo_source("apps/leserpent/scripts/check-language-pack-coverage.mjs");
    let pack_artifact_tests =
        repo_source("apps/leserpent/tests/Leserpent.SecurityTests/LanguagePackArtifactTests.cs");
    let daemon_pack_assets = repo_source("crates/leserpentd/src/language_packs.rs");
    let web_catalog: serde_json::Value = serde_json::from_str(&repo_source(
        "apps/leserpent/src/Leserpent/wwwroot/language-packs/catalog.json",
    ))
    .expect("web language-pack catalog must decode");

    for marker in [
        "leserpent.desktop-localization/v1",
        "LocaleDefinitions.Length != 30",
        "Count(locale => locale.BuiltIn) != 8",
        "Count(locale => locale.IsRightToLeft) != 3",
        "SimplifiedChineseSemanticText",
        "EnglishText[key]",
        "FlowDirection.RightToLeft",
        "zh-Hans-CN",
        "zh-HK",
        "nb-NO",
    ] {
        assert!(
            localization.contains(marker),
            "desktop localization is missing {marker}"
        );
    }
    assert!(localization.contains("public string Resolve(LocalizedText text)"));
    for marker in [
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
        "catalog.Count != expected",
        "VerifyFormat(catalog[DesktopTextKey.StepProgress]",
        "built-in desktop shell catalog is incomplete",
    ] {
        assert!(
            built_in.contains(marker),
            "built-in catalog is missing {marker}"
        );
    }
    assert_eq!(built_in.matches("[DesktopTextKey.").count(), 488);
    assert!(built_in.contains("[DesktopTextKey.ControlTopology] = \"控制拓撲\""));
    assert!(built_in.contains("[DesktopTextKey.Close] = \"閉じる\""));
    assert!(built_in.contains("[DesktopTextKey.FollowSystem] = \"Seguir el sistema\""));
    assert!(built_in.contains("[DesktopTextKey.Reconnect] = \"Neu verbinden\""));
    assert!(built_in.contains("[DesktopTextKey.LearningCenter] = \"Centre d’apprentissage...\""));
    assert!(built_in.contains("[DesktopTextKey.RefreshAll] = \"모두 새로고침\""));
    assert!(!built_in.contains("HttpClient"));
    assert!(!built_in.contains("File."));
    assert!(!built_in.contains("Process."));
    for marker in [
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
        "public const int KeyCount = 26",
        "catalog.Count != KeyCount",
        "SetEquals(expected)",
        "built-in desktop semantic catalog is incomplete",
    ] {
        assert!(
            semantic.contains(marker),
            "built-in semantic catalog is missing {marker}"
        );
    }
    assert!(semantic.contains("[\"remote.title\"] = \"遠端 runtimes\""));
    assert!(semantic.contains("[\"runtime.deploy\"] = \"pipeline をデプロイ\""));
    assert!(semantic.contains("[\"runtime.logs.title\"] = \"Registros\""));
    assert!(semantic.contains("[\"runtime.capabilities.title\"] = \"Fähigkeiten\""));
    assert!(semantic.contains("[\"runtime.history.title\"] = \"Historique\""));
    assert!(semantic.contains("[\"runtime.deploy.form.title\"] = \"원격 배포 확인\""));
    assert!(!semantic.contains("HttpClient"));
    assert!(!semantic.contains("File."));
    assert!(!semantic.contains("Process."));
    assert!(localization.contains(
        "DesktopBuiltInSemanticCatalogs.VerifyContract(\n            SimplifiedChineseSemanticText.Keys)"
    ));
    assert!(localization.contains("built-in desktop semantic translation drifted"));
    assert_eq!(web_catalog["officialLocaleCount"], 30);
    assert_eq!(web_catalog["builtinLocaleCount"], 8);
    assert_eq!(web_catalog["downloadableLocaleCount"], 22);
    for pack in web_catalog["packs"]
        .as_array()
        .expect("web language-pack catalog must contain packs")
    {
        let locale = pack["locale"]
            .as_str()
            .expect("web language pack must have a locale");
        assert!(
            localization.contains(&format!("Core(\"{locale}\"")),
            "desktop locale roster drifted from web locale {locale}"
        );
    }
    assert!(pack_generator.contains("const expandedCoreUiFieldCount = 12"));
    assert!(pack_generator.contains("version: \"1.1.0\""));
    assert!(pack_generator.contains("expanded core UI locale roster drifted"));
    assert!(pack_coverage.contains("const officialPackKeys ="));
    assert!(pack_coverage.contains("publishedKeys.length !== total"));
    assert!(pack_coverage.contains("entry.version !== \"1.1.0\""));
    assert!(pack_artifact_tests.contains("OfficialPackVersion = \"1.1.0\""));
    assert!(pack_artifact_tests.contains("Assert.Equal(30, keys.Count)"));
    assert!(pack_artifact_tests.contains("OfficialPackKeys.SetEquals(keys)"));
    assert!(daemon_pack_assets.contains("string_leaf_count(&pack_json[\"translations\"]), 30"));
    assert!(renderer.contains("Func<LocalizedText, string>? localizedTextResolver"));
    assert!(renderer.contains("localizedTextResolver(text)"));
    assert!(!renderer.contains("node.Accessibility.Label?.Fallback"));
    assert!(!renderer.contains("field => field.Label.Fallback"));

    for marker in [
        "desktop-language-v1.json",
        "JsonUnmappedMemberHandling.Disallow",
        "FileOptions.WriteThrough",
        "stream.Flush(true)",
        "File.Move(temporary, path, true)",
        "UnixFileMode.UserRead | UnixFileMode.UserWrite",
        "desktop language preference must be a regular file",
    ] {
        assert!(store.contains(marker), "language store is missing {marker}");
    }
    assert!(!store.to_ascii_lowercase().contains("token"));
    assert!(!store.to_ascii_lowercase().contains("credential"));

    for marker in [
        "leserpent.language-pack/v1",
        "public const int CoreUiKeyCount = 18",
        "public const int OfficialPackKeyCount = 30",
        "public const string OfficialPackVersion = \"1.1.0\"",
        "InstallCatalogArtifact",
        "VerifyOfficialArtifact",
        "public const int MaxPackBytes = 256 * 1024",
        ".Take(MaxDirectoryEntries + 1)",
        "ReadBoundedAsync",
        "ReadAsync(buffer, cancellationToken)",
        "cancellationToken.ThrowIfCancellationRequested()",
        "RequiredCoreUiKeys.IsSubsetOf(translations.Keys)",
        "CryptographicOperations.FixedTimeEquals",
        "FileOptions.WriteThrough",
        "File.Move(temporary, target, true)",
        "UnixFileMode.UserRead | UnixFileMode.UserWrite",
        "JsonUnmappedMemberHandling.Disallow",
        "built-in locale",
        "malformed desktop language pack blocked a valid sibling",
        "per-key English fallback",
        "failed official language-pack install created persistent state",
        "failed official language-pack update changed persistent state",
    ] {
        assert!(
            pack_store.contains(marker),
            "desktop language-pack store is missing {marker}"
        );
    }
    assert!(!pack_store.contains("HttpClient"));
    assert!(!pack_store.contains("Process."));
    let pack_store_lower = pack_store.to_ascii_lowercase();
    assert!(!pack_store_lower.contains("bearer"));
    assert!(!pack_store_lower.contains("admin_token"));
    assert!(!pack_store_lower.contains("credential"));
    let install_core = pack_store
        .split("private DesktopInstalledLanguagePack InstallCore")
        .nth(1)
        .expect("desktop language-pack store must retain one install commit boundary");
    let official_validation = install_core
        .find("VerifyOfficialArtifact(installed)")
        .expect("official language-pack validation must remain inside the commit boundary");
    let directory_creation = install_core
        .find("EnsurePrivateDirectory(root, create: true)")
        .expect("language-pack commit must retain its private storage boundary");
    assert!(
        official_validation < directory_creation,
        "official language-pack validation must finish before persistent state is created"
    );
    assert_eq!(
        pack_store
            .matches("VerifyOfficialArtifact(installed)")
            .count(),
        1,
        "official artifact validation must only be called by the pre-commit store path"
    );

    for marker in [
        "leserpent.language-pack-catalog/v1",
        "public const int MaxCatalogBytes = 128 * 1024",
        "AllowAutoRedirect = false",
        "UseCookies = false",
        "RemoteTls.ValidateServerCertificate",
        "language-packs/catalog.json",
        "entry.Url != $\"/language-packs/{definition.Locale}.json\"",
        "JsonUnmappedMemberHandling.Disallow",
        "CryptographicOperations.FixedTimeEquals",
        "download must target an official downloadable locale",
        "request.Headers.Authorization is null",
        "X-Leserpent-Admin-Token",
        "VerifyPublishedArtifacts",
        "InstallCatalogArtifact",
        "entry.Version != DesktopLanguagePackStore.OfficialPackVersion",
        "published language-pack set did not round-trip through desktop storage",
    ] {
        assert!(
            pack_catalog.contains(marker),
            "desktop language-pack catalog client is missing {marker}"
        );
    }
    assert!(!pack_catalog.contains("AuthenticationHeaderValue"));
    assert!(!pack_catalog.contains("RemoteTokenResolver"));
    assert!(!pack_catalog.contains("options.Token"));

    for automation_id in [
        "desktop-language-choice",
        "desktop-language-coverage",
        "desktop-language-status",
        "desktop-language-cancel",
        "desktop-language-apply",
        "desktop-language-pack-status",
        "desktop-language-pack-install",
        "desktop-language-pack-remove",
        "desktop-language-pack-source",
        "desktop-language-pack-download",
    ] {
        assert!(window.contains(automation_id));
    }
    assert!(window.contains("choices.Count != 31"));
    assert!(window.contains("public void VerifyLayoutEnvelope()"));
    assert!(window.contains("desired.Width > Width"));
    assert!(window.contains("localization.SetPreference(choice.Preference)"));
    assert!(window.contains("OpenFilePickerAsync"));
    assert!(window.contains("ProbeLanguagePackContract"));
    assert!(window.contains("ProbeLanguagePackDownloadContractAsync"));
    assert!(window.contains("ProbeLanguagePackCancellationContractAsync"));
    assert!(window.contains("localization.InstallCatalogLanguagePack("));
    assert!(localization.contains("packStore.InstallCatalogArtifact("));
    assert!(window.contains("languagePackOperationInProgress"));
    assert!(!window.contains("HttpClient"));
    assert!(hub.contains("hub-open-language"));
    assert!(hub.contains("ProbeLanguageEntry()"));
    assert!(lifecycle.contains("DesktopLanguageWindow"));
    assert!(lifecycle.contains("Language..."));
    assert!(app.contains("--verify-desktop-language-controls"));
    assert!(app.contains("builtin_layouts=8"));
    assert!(app.contains("builtin_semantic_catalogs=7"));
    assert!(app.contains("builtin_ui_ir_controls=7"));
    assert!(app.contains("language_pack_install=true"));
    assert!(app.contains("language_pack_catalog_download=true"));
    assert!(app.contains("language_pack_close_cancellation=true"));
    assert!(app.contains("language_pack_applied_mutations=3"));
    assert!(app.contains("automation_ids=10"));
    assert!(app.contains("localized UI-IR did not reach its native control"));
    assert!(program.contains("--verify-desktop-localization"));
    assert!(program.contains("--verify-desktop-language-pack-artifacts"));
    assert!(program.contains("native_store_roundtrip=true"));
    assert!(program.contains("builtin_shell_catalogs=8"));
    assert!(program.contains("native_shell_keys=80"));
    assert!(program.contains("complete_builtin_locales=8"));
    assert!(program.contains("builtin_semantic_catalogs=7"));
    assert!(program.contains("semantic_keys=26"));
    assert!(program.contains("builtin_semantic_keys=750"));
    assert!(program.contains("language_pack_core_ui_keys=18"));
    assert!(program.contains("language_pack_official_version=1.1.0"));
    assert!(program.contains("language_pack_official_keys=30"));
    assert!(program.contains("compatibility_keys=18"));
    assert!(program.contains("official_version=1.1.0"));
    assert!(program.contains("official_keys=30"));
    assert!(program.contains("language_pack_sha256=true"));
    assert!(program.contains("language_pack_catalog_locale_binding=true"));
    assert!(program.contains("language_pack_catalog_version_binding=true"));
    assert!(program.contains("language_pack_same_origin=true"));
    assert!(program.contains("language_pack_bearer_sent=false"));
    assert!(program.contains("language_pack_cancellation_fenced=true"));
    assert!(program.contains("language_pack_official_precommit=true"));
    assert!(program.contains("language_pack_failed_update_preserves_previous=true"));
    assert!(program.contains("official_precommit=true"));
    assert!(program.contains("language_pack_bad_sibling_isolation=true"));
    assert!(program.contains("builtin_remote_shell_catalogs=7"));
    assert!(program.contains("remote_shell_semantic_keys=56"));
    assert!(program.contains("builtin_remote_operation_catalogs=7"));
    assert!(program.contains("remote_operation_semantic_keys=57"));
    assert!(program.contains("builtin_runtime_workspace_catalogs=7"));
    assert!(program.contains("runtime_workspace_semantic_keys=78"));
    assert!(program.contains("localized_runtime_workspace=true"));
    assert!(program.contains("builtin_orchestra_catalogs=7"));
    assert!(program.contains("orchestra_semantic_keys=72"));
    assert!(program.contains("localized_orchestra=true"));
    assert!(program.contains("builtin_hub_catalogs=7"));
    assert!(program.contains("hub_semantic_keys=69"));
    assert!(program.contains("localized_hub=true"));
    assert!(program.contains("typed_hub_cards=true"));
    assert!(localization.contains("DesktopHubCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopHubPresentation.VerifyContract();"));
    assert!(program.contains("builtin_daemon_retirement_catalogs=7"));
    assert!(program.contains("builtin_startup_recovery_catalogs=7"));
    assert!(program.contains("builtin_account_catalogs=7"));
    assert!(program.contains("localized_ui_ir=true"));
}

#[test]
fn remote_desktop_shell_is_strictly_localized_typed_and_layout_probed() {
    let shell_catalog = avalonia_source("Leserpent.Avalonia/DesktopRemoteShellCatalogs.cs");
    let operation_catalog = avalonia_source("Leserpent.Avalonia/DesktopRemoteOperationCatalogs.cs");
    let workspace_catalog =
        avalonia_source("Leserpent.Avalonia/DesktopRuntimeWorkspaceCatalogs.cs");
    let workspace_presentation =
        avalonia_source("Leserpent.Avalonia/DesktopRuntimeWorkspacePresentation.cs");
    let presentation = avalonia_source("Leserpent.Avalonia/DesktopRemotePresentation.cs");
    let window = avalonia_source("Leserpent.Avalonia/RemoteMainWindow.cs");
    let leselang = avalonia_source("Leserpent.Avalonia/LeselangExportControl.cs");
    let health = avalonia_source("Leserpent.RemoteClient/RemoteAuthorityHealthCoordinator.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");

    for marker in [
        "public const int KeyCount = 56",
        "SimplifiedChinese",
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
        "DesktopDomainCatalogContract.Verify(",
        "Entries.Length != KeyCount",
        "Distinct(StringComparer.Ordinal).Count()",
        "feed.cached_connecting",
        "health.replay_blocked",
        "credential.environment.help",
    ] {
        assert!(
            shell_catalog.contains(marker),
            "remote shell catalog is missing {marker}"
        );
    }
    assert_eq!(shell_catalog.matches("new(\"").count(), 56);
    for marker in [
        "public const int KeyCount = 57",
        "DesktopDomainCatalogContract.Verify(",
        "status.operation_blocked",
        "reason.observation_fence",
        "failure.invalid_response",
        "confirm.capabilities.heading",
        "leselang.status.generated",
    ] {
        assert!(
            operation_catalog.contains(marker),
            "remote operation catalog is missing {marker}"
        );
    }
    assert_eq!(operation_catalog.matches("new(\"").count(), 57);
    for marker in [
        "public const int KeyCount = 78",
        "DesktopDomainCatalogContract.Verify(",
        "a11y.diagnostics_save",
        "live.recovering",
        "change.log_sequence_reset",
        "failure.transport",
        "status.live_alert",
    ] {
        assert!(
            workspace_catalog.contains(marker),
            "runtime workspace catalog is missing {marker}"
        );
    }
    assert_eq!(workspace_catalog.matches("new(\"").count(), 78);
    assert!(workspace_presentation.contains("public static DesktopRuntimeWorkspaceText Change("));
    assert!(workspace_presentation.contains("RemoteWorkspaceSnapshotChange change"));
    assert!(workspace_presentation.contains("RemoteWorkspaceSeverityAlert alert"));
    assert!(presentation.contains("public static DesktopRemoteText Feed(RemoteFeedState state)"));
    assert!(presentation.contains("state.Health is { } health"));
    assert!(presentation.contains("RemoteMutationAdmissionFailure.ObservationFencePending"));
    assert!(presentation.contains("RemoteMutationFailureKind.InvalidResponse"));
    assert!(health.contains("RemoteHealth? Health = null"));
    assert!(health.contains("presentation.RequiresAttention,\n                    health"));
    assert!(window.contains("public void VerifyLayoutEnvelope()"));
    assert!(window.contains("public void ProbeTypedPresentation()"));
    assert!(window.contains("public void ProbeLocalizedPresentation("));
    assert!(!window.contains("statusText.Text = state.IsStale"));
    assert!(!window.contains("authorityHealthText.Text = state.Label"));
    assert!(window.contains("DesktopRemotePresentation.MutationFailure("));
    assert!(window.contains("RemoteLayoutDensity.Compact"));
    assert!(window.contains("RemoteLayoutDensity.Wide"));
    assert!(leselang.contains("DesktopRemoteOperationCatalogs.Resolve("));
    assert!(leselang.contains("localization.Changed += OnLocalizationChanged"));
    assert!(localization.contains("DesktopRemoteShellCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopRemoteOperationCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopRuntimeWorkspaceCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopRuntimeWorkspacePresentation.VerifyContract();"));
    assert!(localization.contains("DesktopOrchestraCatalogs.VerifyContract();"));
    assert!(app.contains("--verify-remote-shell-controls"));
    assert!(app.contains("startRemoteClients: false"));
    assert!(app.contains("localized_dialog_layouts=40"));
    assert!(app.contains("localized_workspace_layouts=8"));
    assert!(app.contains("localized_orchestra_layouts=8"));
    assert!(app.contains("workspace_instances=1"));
    assert!(app.contains("workspace_live_language_reprojection=true"));
    assert!(app.contains("orchestra_live_language_reprojection=true"));
    assert!(app.contains("wide_layout=true"));
    assert!(app.contains("live_language_reprojection=true"));
    assert!(app.contains("network_started=false"));
}

#[test]
fn hub_topology_refresh_is_discoverable_observed_and_single_flight() {
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let hub_catalog = avalonia_source("Leserpent.Avalonia/DesktopHubCatalogs.cs");
    let hub_presentation = avalonia_source("Leserpent.Avalonia/DesktopHubPresentation.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let coordinator = avalonia_source("Leserpent.RemoteClient/RemoteTopologyRefreshCoordinator.cs");

    assert!(hub.contains("hub-refresh-all"));
    assert!(hub.contains("HubText(\"tooltip.refresh_all\")"));
    assert!(hub_catalog.contains("public const int KeyCount = 69"));
    assert_eq!(hub_catalog.matches("new(\"").count(), 69);
    for marker in [
        "tooltip.refresh_all",
        "status.refresh_attention",
        "summary.filtered",
        "runtime.status.failed",
        "a11y.open_runtime",
    ] {
        assert!(
            hub_catalog.contains(marker),
            "Hub catalog is missing {marker}"
        );
    }
    assert!(hub_presentation.contains("public static DesktopHubText RefreshSummary("));
    assert!(hub_presentation.contains("public static DesktopHubText TopologySummary("));
    assert!(hub_presentation.contains("DesktopRemotePresentation.AuthorityHealth(state)"));
    assert!(hub.contains("private readonly RemoteTopologyRefreshCoordinator topologyRefresh"));
    assert!(hub.contains("refreshAllPresentationOperation"));
    assert!(hub.contains("private bool operatorRefreshRequested"));
    assert!(hub.contains("topologyRefresh.RefreshAuthorityAsync"));
    assert!(hub.contains("topologyRefresh.RefreshAllAsync"));
    assert!(hub.contains("summary.RequiresAttention"));
    assert!(!hub.contains("SemaphoreSlim topologyLoadGate"));
    assert!(!hub.contains("card.RefreshOperation"));
    assert!(hub.contains("ObserveTopologyOperation("));
    assert!(hub.contains(
        "catch (Exception)\n        {\n            if (lifetime.IsCancellationRequested)"
    ));
    assert!(!hub.contains("catch (Exception) when (!lifetime.IsCancellationRequested)"));
    assert!(hub.contains("refreshAllTopologyButton.RaiseEvent"));
    assert!(hub.contains("ReferenceEquals(cardRefresh, cardJoin)"));
    assert!(hub.contains("ReferenceEquals(refreshAll, refreshAllPresentationOperation)"));
    assert!(hub.contains("DesktopHubPresentation.RefreshSummary(summary)"));
    assert!(hub.contains("DesktopHubPresentation.RuntimeStatus(runtime)"));
    assert!(!hub.contains("state.Phase.ToString().ToUpperInvariant()"));
    assert!(!hub.contains("runtime.RefreshStatus.ToString().ToUpperInvariant()"));
    assert!(!hub.contains("_ = RefreshAllTopologiesAsync"));
    assert!(!hub.contains("_ = RefreshTopologyAsync"));
    assert!(app.contains("await window.ProbeRefreshAllControlAsync()"));
    assert!(app.contains("refresh_all_control=true"));
    assert!(app.contains("refresh_all_single_flight=true"));
    assert!(app.contains("card_refresh_join=true"));
    assert!(app.contains("shared_refresh_policy=true"));
    assert!(app.contains("refresh_busy_state=true"));
    assert!(app.contains("refresh_completion_status=true"));
    assert!(app.contains("localized_hub_catalogs=7"));
    assert!(app.contains("hub_semantic_keys=69"));
    assert!(app.contains("localized_hub_layouts=8"));
    assert!(app.contains("typed_hub_cards=true"));
    assert!(coordinator.contains("ReferenceEquals(alphaRefresh, alphaJoin)"));
    assert!(coordinator.contains("ReferenceEquals(all, allJoin)"));
    assert!(coordinator.contains("maximumActive != 2"));
    assert!(coordinator.contains("topology coordinator did not accept the complete bounded fleet"));
    assert!(coordinator.contains("cancelled topology refresh reached its authority loader"));
}

#[test]
fn runtime_workspace_launch_is_shared_revision_fenced_and_frontend_neutral() {
    let window = avalonia_source("Leserpent.Avalonia/RemoteMainWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let coordinator = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceLaunchCoordinator.cs");
    let authority = avalonia_source("Leserpent.RemoteClient/RemoteFeedAuthorityPolicy.cs");
    let remote_conformance = avalonia_source("Leserpent.RemoteConformance/Program.cs");
    let mobile_conformance =
        repo_source("apps/leserpent-mobile/src/Leserpent.MobileConformance/Program.cs");

    assert!(window.contains("private readonly RemoteWorkspaceLaunchCoordinator workspaceLaunch"));
    assert!(window.contains("workspaceLaunch.Request("));
    assert!(window.contains("workspaceLaunch.Observe(state)"));
    assert!(window.contains("ApplyWorkspaceLaunchDecision"));
    assert!(!window.contains("Dictionary<string, ulong> pendingWorkspaceRequests"));
    assert!(!window.contains("internal static class RemoteWorkspaceLaunchPolicy"));
    assert!(!window.contains("RemoteWorkspaceLaunchPolicy.CanResolve"));
    assert!(coordinator.contains("public sealed class RemoteWorkspaceLaunchCoordinator"));
    assert!(coordinator.contains("public static class RemoteWorkspaceLaunchPolicy"));
    assert!(coordinator.contains("RemoteFeedAuthorityPolicy.HasAuthoritativeSnapshot(state)"));
    assert!(authority.contains("state.SnapshotGeneration > 0"));
    assert!(coordinator.contains("Math.Max(previousRevision, minimumRevision)"));
    assert!(coordinator.contains("RemoteWorkspaceLaunchDisposition.RejectUnavailable"));
    assert!(coordinator.contains("combined active and pending capacity"));
    assert!(coordinator.contains("coalesced snapshot revision fence"));
    assert!(coordinator.contains("terminal remote state retained"));
    assert!(!coordinator.contains("Avalonia"));
    assert!(app.contains("RemoteWorkspaceLaunchCoordinator.VerifyContract()"));
    assert!(app.contains("shared_workspace_launch=true"));
    assert!(remote_conformance.contains("RemoteWorkspaceLaunchCoordinator.VerifyContract()"));
    assert!(remote_conformance.contains("workspace_launch_coordination=true"));
    assert!(mobile_conformance.contains("RemoteWorkspaceLaunchCoordinator.VerifyContract()"));
    assert!(mobile_conformance.contains("workspace_launch=true"));
}

#[test]
fn gewyvern_provisioning_is_authority_scoped_identity_locked_and_bounded() {
    let client = avalonia_source("Leserpent.RemoteClient/RemoteProvisioningClient.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let window = avalonia_source("Leserpent.Avalonia/GewyvernProvisioningWindow.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopProvisioningCatalogs.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
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
    assert!(window.contains("provisioning-credential-handle"));
    assert!(window.contains("AutomationLiveSetting.Assertive"));
    assert!(window.contains("DesktopProvisioningCatalogs.Resolve(localization, key)"));
    assert!(window.contains("localization.Changed += OnLocalizationChanged"));
    assert!(window.contains("localization.Changed -= OnLocalizationChanged"));
    assert!(window.contains("public void VerifyLayoutEnvelope()"));
    assert!(window.contains("public void ProbeLocalizedPresentation("));
    assert!(window.contains("SafeValue(state.RuntimeId)"));
    assert!(window.contains("localizedStatusKey = null"));
    assert!(window.contains("provisioningId.Text != originalProvisioningId"));
    assert!(!window.contains("Text = \"RUNTIME PROVISIONING\""));
    assert!(!window.contains("Content = \"Provision gewyvern\""));
    for marker in [
        "public const int KeyCount = 43",
        "SimplifiedChinese",
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
        "catalog.Count != KeyCount",
        "SetEquals(expected)",
        "HasExpectedPlaceholders",
        "VerifyFormat(entry.Value",
        "desktop provisioning localization catalog is incomplete",
        "new provisioning ID",
    ] {
        assert!(
            catalog.contains(marker),
            "provisioning localization catalog is missing {marker}"
        );
    }
    assert_eq!(catalog.matches("[\"").count(), 344);
    assert!(catalog.contains("[\"heading\"] = \"安装并注册 gewyvern\""));
    assert!(catalog.contains("[\"submit\"] = \"佈建 gewyvern\""));
    assert!(catalog.contains("[\"phase.runtime_registered\"] = \"RUNTIME 登録済み\""));
    assert!(catalog.contains("[\"kicker\"] = \"APROVISIONAMIENTO DEL RUNTIME\""));
    assert!(catalog.contains("[\"title\"] = \"Leserpent / Gewyvern bereitstellen\""));
    assert!(catalog.contains("[\"submit\"] = \"Provisionner gewyvern\""));
    assert!(
        catalog.contains("[\"status.waiting\"] = \"선택한 daemon 권한 주체를 기다리는 중...\"")
    );
    assert!(!catalog.contains("HttpClient"));
    assert!(!catalog.contains("Process."));
    assert!(!catalog.contains("File."));
    assert!(localization.contains("DesktopProvisioningCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopProvisioningCatalogs.KeyCount"));
    assert!(hub.contains("hub-provision-gewyvern"));
    assert!(app.contains("ExecuteProvisioningAsync"));
    assert!(app.contains("--verify-provisioning-controls"));
    assert!(app.contains("localized_provisioning_catalogs=7"));
    assert!(app.contains("localized_layouts=8"));
    assert!(app.contains("observation_limit_no_reconcile=true"));
    assert!(app.contains("await window.ProbeWorkflowAsync(\"zh-CN\")"));
    assert!(app.contains("await window.ProbeObservationLimitAsync(\"de\")"));
    assert!(program.contains("--verify-provisioning-client"));
    assert!(program.contains("builtin_provisioning_catalogs=7"));
    assert!(program.contains("provisioning_semantic_keys=43"));
    assert!(program.contains("builtin_semantic_keys=750"));
    assert!(program.contains("localized_gewyvern_provisioning=true"));
    assert!(promotion.contains("BootstrapPromotionJsonContext.Default"));
    assert!(!promotion.contains("JsonSerializer.Serialize(new\n"));
}

#[test]
fn gewyvern_retirement_is_confirmed_provisioning_bound_and_failure_safe() {
    let client = avalonia_source("Leserpent.RemoteClient/RemoteRetirementClient.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let window = avalonia_source("Leserpent.Avalonia/GewyvernRetirementWindow.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopRetirementCatalogs.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
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
    assert!(window.contains("retirement-credential-handle"));
    assert!(window.contains("AutomationLiveSetting.Assertive"));
    assert!(window.contains("DesktopRetirementCatalogs.Resolve(localization, key)"));
    assert!(window.contains("localization.Changed += OnLocalizationChanged"));
    assert!(window.contains("localizedStatusKey = null"));
    assert!(window.contains("VerifyLayoutEnvelope()"));
    assert!(window.contains("ProbeLocalizedPresentation("));
    assert!(window.contains("ProbeObservationLimitAsync"));
    assert!(window.contains("new WrapPanel"));
    assert!(!window.contains("phase.Text = state.Phase.Replace"));
    assert!(catalog.contains("public const int KeyCount = 45;"));
    assert!(catalog.contains("private const string Prefix = \"desktop.retirement.\";"));
    assert!(catalog.contains("public static IReadOnlyDictionary<string, string> Korean"));
    assert!(catalog.contains("SetEquals(expected)"));
    assert!(catalog.contains("HasExpectedPlaceholders"));
    assert!(catalog.contains("use a new retirement ID for a corrected attempt"));
    assert!(
        catalog
            .contains("[\"status.failed\"] = \"退役失败，受限故障代码为 {0}。runtime 仍保持注册")
    );
    assert!(
        catalog.contains("[\"status.waiting\"] = \"선택한 daemon 권한 주체를 기다리는 중...\"")
    );
    assert!(!catalog.contains("HttpClient"));
    assert!(!catalog.contains("Process."));
    assert!(!catalog.contains("File."));
    assert!(localization.contains("DesktopRetirementCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopRetirementCatalogs.KeyCount"));
    assert!(hub.contains("hub-retire-gewyvern"));
    assert!(app.contains("ExecuteRetirementAsync"));
    assert!(app.contains("--verify-retirement-controls"));
    assert!(app.contains("localized_retirement_catalogs=7"));
    assert!(app.contains("localized_layouts=8"));
    assert!(app.contains("observation_limit_no_reconcile=true"));
    assert!(app.contains("await window.ProbeWorkflowAsync(\"zh-CN\")"));
    assert!(app.contains("await window.ProbeObservationLimitAsync(\"de\")"));
    assert!(program.contains("--verify-retirement-client"));
    assert!(program.contains("builtin_retirement_catalogs=7"));
    assert!(program.contains("retirement_semantic_keys=45"));
    assert!(program.contains("builtin_semantic_keys=750"));
    assert!(program.contains("localized_gewyvern_retirement=true"));
}

#[test]
fn daemon_retirement_is_bootstrap_bound_authority_omitting_and_runtime_independent() {
    let client = avalonia_source("Leserpent.RemoteClient/RemoteDaemonRetirementClient.cs");
    let contracts = avalonia_source("Leserpent.RemoteClient/RemoteDaemonRetirementContracts.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let window = avalonia_source("Leserpent.Avalonia/DaemonRetirementWindow.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopDaemonRetirementCatalogs.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
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
    assert!(window.contains("daemon-retirement-credential-handle"));
    assert!(window.contains("AutomationLiveSetting.Assertive"));
    assert!(window.contains("DesktopDaemonRetirementCatalogs.Resolve(localization, key)"));
    assert!(window.contains("localization.Changed += OnLocalizationChanged"));
    assert!(window.contains("localization.Changed -= OnLocalizationChanged"));
    assert!(window.contains("VerifyLayoutEnvelope()"));
    assert!(window.contains("ProbeLocalizedPresentation("));
    assert!(window.contains("ProbeObservationLimitAsync"));
    assert!(window.contains("new WrapPanel"));
    assert!(!window.contains("phase.Text = state.Phase.Replace"));
    assert!(!window.contains("private readonly TextBox host"));
    assert!(catalog.contains("public const int KeyCount = 37;"));
    assert!(catalog.contains("private const string Prefix = \"desktop.daemon_retirement.\";"));
    assert!(catalog.contains("public static IReadOnlyDictionary<string, string> Korean"));
    assert!(catalog.contains("DesktopDomainCatalogContract.Verify("));
    assert!(catalog.contains("use a new retirement ID after remediation"));
    assert!(!catalog.contains("HttpClient"));
    assert!(!catalog.contains("Process."));
    assert!(!catalog.contains("File."));
    assert!(localization.contains("DesktopDaemonRetirementCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopDaemonRetirementCatalogs.KeyCount"));
    assert!(hub.contains("hub-retire-daemon"));
    assert!(app.contains("ExecuteDaemonRetirementAsync"));
    assert!(app.contains("--verify-daemon-retirement-controls"));
    assert!(app.contains("localized_daemon_retirement_catalogs=7"));
    assert!(app.contains("localized_layouts=8"));
    assert!(app.contains("await window.ProbeWorkflowAsync(\"zh-CN\")"));
    assert!(app.contains("await window.ProbeObservationLimitAsync(\"de\")"));
    assert!(program.contains("--verify-daemon-retirement-client"));
    assert!(program.contains("builtin_daemon_retirement_catalogs=7"));
    assert!(program.contains("daemon_retirement_semantic_keys=37"));
    assert!(program.contains("localized_daemon_retirement=true"));
}

#[test]
fn startup_recovery_is_redacted_strictly_localized_and_layout_bounded() {
    let window = avalonia_source("Leserpent.Avalonia/StartupErrorWindow.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopStartupRecoveryCatalogs.cs");
    let contract = avalonia_source("Leserpent.Avalonia/DesktopDomainCatalogContract.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("public const string DefaultDescription"));
    assert!(window.contains("DesktopStartupRecoveryCatalogs.Resolve(localization, key)"));
    assert!(window.contains("localization.Changed += OnLocalizationChanged"));
    assert!(window.contains("localization.Changed -= OnLocalizationChanged"));
    assert!(window.contains("description == StartupFailure.DefaultDescription"));
    assert!(window.contains("VerifyLayoutEnvelope()"));
    assert!(window.contains("ProbeLocalizedPresentation("));
    assert!(window.contains("Take(512)"));
    assert!(window.contains("redacted.Replace(secret, \"[redacted]\""));
    assert!(catalog.contains("public const int KeyCount = 9;"));
    assert!(catalog.contains("private const string Prefix = \"desktop.startup_recovery.\";"));
    assert!(catalog.contains("public static IReadOnlyDictionary<string, string> Korean"));
    assert!(catalog.contains("DesktopDomainCatalogContract.Verify("));
    assert!(!catalog.contains("HttpClient"));
    assert!(!catalog.contains("Process."));
    assert!(!catalog.contains("File."));
    assert!(contract.contains("SetEquals(expected)"));
    assert!(contract.contains("HasExpectedPlaceholders"));
    assert!(contract.contains("entry.Value.Length is > 0 and <= 1024"));
    assert!(localization.contains("DesktopStartupRecoveryCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopStartupRecoveryCatalogs.KeyCount"));
    assert!(app.contains("localized_startup_catalogs=7"));
    assert!(app.contains("startup recovery localized layout coverage drifted"));
    assert!(program.contains("builtin_startup_recovery_catalogs=7"));
    assert!(program.contains("startup_recovery_semantic_keys=9"));
    assert!(program.contains("localized_startup_recovery=true"));
}

#[test]
fn remote_window_observes_async_ui_operations_and_fences_shutdown_updates() {
    let source = remote_main_window_source();

    assert!(!source.contains("private async void RequestReconnect()"));
    assert!(!source.contains("private async void OnActionInvoked(string nodeId)"));
    assert!(source.contains("ObserveUiOperation(RequestReconnectAsync())"));
    assert!(source.contains("ObserveUiOperation(OnActionInvokedAsync(invocation))"));
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
    let coordinator = avalonia_source("Leserpent.RemoteClient/RemoteAuthorityHealthCoordinator.cs");

    assert!(source.contains("remote-authority-health"));
    assert!(source.contains("remote-authority-health-refresh"));
    assert!(source.contains("AutomationLiveSetting.Assertive"));
    assert!(
        source.contains(
            "private readonly RemoteAuthorityHealthCoordinator authorityHealthCoordinator"
        )
    );
    assert!(source.contains("authorityHealthCoordinator.RefreshAsync(lifetime.Token)"));
    assert!(source.contains("ApplyAuthorityHealth(authorityHealthCoordinator.State)"));
    assert!(source.contains("authorityHealthCoordinator.Stop();"));
    assert!(!source.contains("private bool healthInFlight"));
    assert!(!source.contains("await healthClient.CheckAsync"));
    assert!(!source.contains("RemoteAuthorityHealthPresentation.Create"));
    assert!(!source.contains("QUEUE SATURATED"));
    assert!(presentation.contains("public sealed record RemoteAuthorityHealthPresentation"));
    assert!(presentation.contains("QUEUE SATURATED"));
    assert!(presentation.contains("effect queue metrics unavailable"));
    assert!(!presentation.contains("Avalonia"));
    assert!(coordinator.contains("public sealed record RemoteAuthorityHealthState"));
    assert!(coordinator.contains("RemoteAuthorityHealthPhase.Stopped"));
    assert!(coordinator.contains("RemoteAuthorityHealthFailure.Unexpected"));
    assert!(coordinator.contains("TaskCreationOptions.RunContinuationsAsynchronously"));
    assert!(!coordinator.contains("Avalonia"));
    assert!(program.contains("--verify-authority-health-presentation"));
    assert!(program.contains("RemoteAuthorityHealthCoordinator.VerifyContractAsync()"));
    assert!(program.contains("saturation_visible=true"));
    assert!(program.contains("shared_lifecycle=true"));
    assert!(program.contains("single_flight=true"));
    assert!(program.contains("stop_fence=true"));
}

#[test]
fn gui_mutations_export_canonical_leselang_without_execution() {
    let window = remote_main_window_source();
    let workspace = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let exporter = avalonia_source("Leserpent.RemoteClient/RemoteLeselangExport.cs");
    let transport = avalonia_source("Leserpent.RemoteClient/RemoteWireTransport.cs");
    let control = avalonia_source("Leserpent.Avalonia/LeselangExportControl.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopRemoteOperationCatalogs.cs");
    let workspace_catalog =
        avalonia_source("Leserpent.Avalonia/DesktopRuntimeWorkspaceCatalogs.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("leselangClient.ExportRefreshAsync"));
    assert!(window.contains("leselangClient.ExportDeployAsync"));
    assert!(workspace.contains("leselangClient.ExportWorkspaceAsync"));
    assert!(workspace.contains("runtime-workspace-leselang"));
    assert!(workspace.contains("DesktopRuntimeWorkspaceCatalogs.Format("));
    assert!(workspace_catalog.contains("Preview equivalent workspace Leselang"));
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
    assert!(!control.contains("Content = \"Copy Leselang\""));
    assert!(control.contains("DesktopRemoteOperationCatalogs.Resolve("));
    assert!(catalog.contains("\"Copy Leselang\""));
    assert!(catalog.contains("No operation was executed."));
    assert!(control.contains("SetTextAsync(source)"));
    assert!(control.contains("ExportDebounce"));
    assert!(control.contains("leselang.status.unavailable"));
    assert!(catalog.contains("no local template was substituted"));
    assert!(program.contains("--verify-leselang-gui-export"));
    assert!(program.contains("rust_authority=true"));
}

#[test]
fn runtime_workspace_log_filter_is_local_bounded_and_accessible() {
    let window = avalonia_source("Leserpent.Avalonia/RemoteRuntimeWorkspaceWindow.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopRuntimeWorkspaceCatalogs.cs");
    let filter = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceLogFilter.cs");
    let export = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceDiagnosticExport.cs");
    let projection = avalonia_source("Leserpent.RemoteClient/RemoteWorkspaceDocumentProjection.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("runtime-log-search"));
    assert!(window.contains("runtime-log-level"));
    assert!(window.contains("runtime-log-filter-summary"));
    assert!(window.contains("runtime-diagnostics-copy"));
    assert!(window.contains("runtime-diagnostics-save"));
    assert!(catalog.contains("Save visible runtime diagnostics"));
    assert!(catalog.contains("Review it before sharing"));
    assert!(window.contains("SaveDiagnosticsAsync"));
    assert!(window.contains("storage is null || !storage.CanSave"));
    assert!(window.contains("SaveFilePickerAsync"));
    assert!(window.contains("ShowOverwritePrompt = true"));
    assert!(window.contains("!stream.CanWrite || !stream.CanSeek"));
    assert!(window.contains("stream.SetLength(0)"));
    assert!(window.contains("stream.Position = 0"));
    assert!(window.contains("RemoteWorkspaceDiagnosticExport.Encode(view)"));
    assert!(catalog.contains("Diagnostic save canceled."));
    assert!(catalog.contains("Diagnostic save failed safely."));
    assert!(window.contains(".LogFilterSummary(view)"));
    assert!(window.contains("SelectedLogLevel"));
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
    let presentation = avalonia_source("Leserpent.Avalonia/DesktopRuntimeWorkspacePresentation.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(window.contains("RemoteWorkspaceSnapshotChanges.Compare"));
    assert!(window.contains("DesktopRuntimeWorkspacePresentation.Loaded("));
    assert!(!window.contains("change.Describe()"));
    assert!(!window.contains("severityAlert.Describe()"));
    assert!(presentation.contains("public static DesktopRuntimeWorkspaceText Change("));
    assert!(presentation.contains("change.NewErrors"));
    assert!(presentation.contains("change.NewWarnings"));
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
fn desktop_ca_pruning_is_bounded_validated_and_hub_owned() {
    let store = avalonia_source("Leserpent.Avalonia/DesktopCertificateAuthorityStore.cs");
    let promotion = avalonia_source("Leserpent.Avalonia/DesktopBootstrapPromotion.cs");
    let startup = avalonia_source("Leserpent.Avalonia/DesktopProductStartup.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let prune_start = store
        .find("public void PruneExcept(IEnumerable<string> retainedPaths)")
        .expect("desktop CA store must expose set-based pruning");
    let prune_end = store[prune_start..]
        .find("public static DesktopCertificateAuthorityStore Default()")
        .expect("desktop CA pruning must have a bounded source region");
    let prune = &store[prune_start..prune_start + prune_end];

    for marker in [
        "private const int MaxDirectoryEntries = 128;",
        ".Take(MaxDirectoryEntries + 1)",
        "VerifyManagedCertificate(entry)",
        "var deletions = new List<string>();",
        "desktop CA entry-budget rejection mutated the trust directory",
        "desktop CA validation failure partially pruned the trust directory",
    ] {
        assert!(
            store.contains(marker),
            "desktop CA store is missing {marker}"
        );
    }
    let validation = prune
        .find("foreach (var entry in entries)")
        .expect("desktop CA pruning must validate its directory snapshot");
    let deletion = prune
        .find("foreach (var entry in deletions)")
        .expect("desktop CA pruning must retain a separate commit phase");
    assert!(
        validation < deletion,
        "desktop CA pruning must validate the complete snapshot before deletion"
    );
    assert!(!promotion.contains(".PruneExcept("));
    assert!(!startup.contains(".PruneExcept("));
    assert_eq!(app.matches("certificateStore.PruneExcept(").count(), 3);
    assert!(app.contains("localOrchestraService?.ManagedAuthorityPath"));
    assert!(promotion.contains("File.Exists(unrelatedAuthority)"));
    assert!(program.contains("global_ca_gc=false"));
    assert!(program.contains("unrelated_managed_ca_preserved=true"));
    assert!(program.contains("validation_before_prune=true"));
    assert!(program.contains("retained_content_revalidation=true"));
}

#[test]
fn desktop_connection_preflight_is_explicit_cancellable_and_side_effect_free() {
    let window = avalonia_source("Leserpent.Avalonia/DesktopConnectionWindow.cs");
    let forget = avalonia_source("Leserpent.Avalonia/DesktopForgetConnectionWindow.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopConnectionCatalogs.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
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
    assert!(window.contains("localization.Changed += OnLocalizationChanged"));
    assert!(window.contains("localization.Changed -= OnLocalizationChanged"));
    assert!(window.contains("DesktopConnectionCatalogs.Resolve(localization, key)"));
    assert!(window.contains("public void VerifyLayoutEnvelope()"));
    assert!(window.contains("public void ProbeLocalizedPresentation("));
    assert!(!window.contains("Text = \"Connect the desktop console\""));
    assert!(!window.contains("Content = \"Test connection\""));
    assert!(forget.contains("DesktopConnectionCatalogs.Resolve(localization, key)"));
    assert!(forget.contains("public void VerifyLayoutEnvelope()"));
    assert!(forget.contains("localization.Changed += OnLocalizationChanged"));
    assert!(!forget.contains("Text = \"Forget this connection?\""));
    for marker in [
        "public const int KeyCount = 33",
        "SimplifiedChinese",
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
        "catalog.Count != KeyCount",
        "SetEquals(expected)",
        "formattedKeys.Contains(entry.Key)",
        "VerifyFormat(value)",
        "desktop connection localization catalog is incomplete",
    ] {
        assert!(
            catalog.contains(marker),
            "connection catalog is missing {marker}"
        );
    }
    assert!(catalog.contains("[\"heading\"] = \"连接桌面控制台\""));
    assert!(catalog.contains("[\"connect\"] = \"連線\""));
    assert!(catalog.contains("[\"test\"] = \"接続をテスト\""));
    assert!(catalog.contains("[\"forget.action\"] = \"Olvidar conexión\""));
    assert!(catalog.contains("[\"title\"] = \"Leserpent / Verbinden\""));
    assert!(catalog.contains("[\"forget.heading\"] = \"Oublier cette connexion ?\""));
    assert!(catalog.contains(
        "[\"status.ready\"] = \"연결을 확인했습니다. 원격 권한 주체가 준비되었습니다.\""
    ));
    assert!(!catalog.contains("HttpClient"));
    assert!(!catalog.contains("Process."));
    assert!(!catalog.contains("File."));
    assert!(localization.contains("DesktopConnectionCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopConnectionCatalogs.KeyCount"));
    assert!(app.contains("localized_connection_catalogs=7"));
    assert!(app.contains("localized_layouts=8"));
    assert!(app.contains("localized_forget_layouts=8"));
    assert!(app.contains("live_language_reprojection=true"));
    assert!(program.contains("builtin_connection_catalogs=7"));
    assert!(program.contains("connection_semantic_keys=33"));
    assert!(program.contains("builtin_semantic_keys=750"));
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
fn desktop_reverse_deployment_is_strictly_localized_and_operator_data_preserving() {
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopBootstrapDeploymentCatalogs.cs");
    let window = avalonia_source("Leserpent.Avalonia/BootstrapDeploymentWindow.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    for marker in [
        "public const int KeyCount = 46",
        "SimplifiedChinese",
        "TraditionalChinese",
        "Japanese",
        "Spanish",
        "German",
        "French",
        "Korean",
        "catalog.Count != KeyCount",
        "SetEquals(expected)",
        "HasExpectedPlaceholders",
        "VerifyFormat(entry.Value",
        "desktop bootstrap localization catalog is incomplete",
    ] {
        assert!(
            catalog.contains(marker),
            "bootstrap localization catalog is missing {marker}"
        );
    }
    assert_eq!(catalog.matches("[\"").count(), 368);
    assert!(catalog.contains("[\"heading\"] = \"部署 daemon 权威端\""));
    assert!(catalog.contains("[\"phase.session_bound\"] = \"工作階段已綁定\""));
    assert!(catalog.contains("[\"deploy\"] = \"leserpentd をデプロイ\""));
    assert!(catalog.contains("[\"kicker\"] = \"DESPLIEGUE INVERSO\""));
    assert!(catalog.contains("[\"title\"] = \"Leserpent / Daemon bereitstellen\""));
    assert!(catalog.contains("[\"promote\"] = \"Ajouter au Hub\""));
    assert!(catalog.contains("[\"status.waiting\"] = \"선택한 권한 주체를 기다리는 중...\""));
    assert!(!catalog.contains("HttpClient"));
    assert!(!catalog.contains("Process."));
    assert!(!catalog.contains("File."));

    assert!(window.contains("DesktopBootstrapDeploymentCatalogs.Resolve(localization, key)"));
    assert!(window.contains("localization.Changed += OnLocalizationChanged"));
    assert!(window.contains("localization.Changed -= OnLocalizationChanged"));
    assert!(window.contains("public void VerifyLayoutEnvelope()"));
    assert!(window.contains("public void ProbeLocalizedPresentation("));
    assert!(window.contains("SafeValue(state.DaemonId)"));
    assert!(window.contains("localizedStatusKey = null"));
    assert!(window.contains("host.Text != \"target.example\""));
    assert!(!window.contains("Text = \"REVERSE DEPLOYMENT\""));
    assert!(!window.contains("Content = \"Deploy leserpentd\""));

    assert!(localization.contains("DesktopBootstrapDeploymentCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopBootstrapDeploymentCatalogs.KeyCount"));
    assert!(app.contains("localized_bootstrap_catalogs=7"));
    assert!(app.contains("localized_layouts=8"));
    assert!(app.contains("await window.ProbeWorkflowAsync(\"zh-CN\")"));
    assert!(program.contains("builtin_bootstrap_catalogs=7"));
    assert!(program.contains("bootstrap_semantic_keys=46"));
    assert!(program.contains("builtin_semantic_keys=750"));
    assert!(program.contains("localized_reverse_deployment=true"));
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
    assert!(supervisor.contains("DesktopLanguagePackSource.FromLocal(plan)"));
    assert!(supervisor.contains("catalogClient.DownloadAsync(\"pt-BR\")"));
    assert!(supervisor.contains("download.Sha256"));
    assert!(supervisor.contains("download.Version"));
    assert!(supervisor.contains("languagePackStore.InstallCatalogArtifact("));
    assert!(!supervisor.contains("VerifyOfficialArtifact"));
    assert!(supervisor.contains("Kill(entireProcessTree: true)"));
    assert!(supervisor.contains("ObjectDisposedException.ThrowIf(disposed, this)"));
    assert!(token_store.contains("LocalProcess"));
    assert!(presentation.contains("TOKEN / LOCAL PROCESS"));
    assert!(program.contains("--verify-local-orchestra"));
    assert!(program.contains("owned_authority=true"));
    assert!(program.contains("credential_free_language_pack_download=true"));
    assert!(program.contains("language_pack_digest_binding=true"));
    assert!(program.contains("language_pack_private_roundtrip=true"));
    assert!(program.contains("language_pack_official_version=1.1.0"));
    assert!(program.contains("language_pack_official_keys=30"));
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

#[test]
fn saved_daemon_language_packs_are_persisted_ca_bound_and_credential_free() {
    let verifier = avalonia_source("Leserpent.Avalonia/SavedDaemonLanguagePackVerifier.cs");
    let supervisor = avalonia_source("Leserpent.Avalonia/LocalOrchestraServiceSupervisor.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    for marker in [
        "DesktopConnectionCatalogStore",
        "DesktopProductStartup.PrepareSavedCatalog",
        "DesktopLanguagePackSource.FromConnection",
        "DesktopLanguagePackCatalogClient",
        "VerifyWrongCertificateRejected",
        "catch (HttpRequestException)",
        "CryptographicOperations.FixedTimeEquals",
        "download.Sha256",
        "download.Version",
        "languagePackStore.InstallCatalogArtifact(",
        "languagePackStore.Remove(download.Locale)",
        "saved daemon language-pack catalog leaked its live credential",
        "saved daemon language-pack proof mutated its persisted inputs",
    ] {
        assert!(
            verifier.contains(marker),
            "saved daemon language-pack verifier is missing {marker}"
        );
    }
    assert!(!verifier.contains("RemoteTokenResolver"));
    assert!(!verifier.contains("RemoteClientOptions.Create"));
    assert!(!verifier.contains("AuthenticationHeaderValue"));
    assert!(!verifier.contains("Authorization"));
    assert!(!verifier.contains("X-Leserpent-Admin-Token"));

    assert!(supervisor.contains("VerifySavedDaemonLanguagePackContract"));
    assert!(supervisor.contains("SavedDaemonLanguagePackVerifier.Verify"));
    assert!(program.contains("--verify-saved-daemon-language-pack"));
    assert!(program.contains("persisted_catalog=true"));
    assert!(program.contains("saved_connection_source=true"));
    assert!(program.contains("selected_ca_only=true"));
    assert!(program.contains("wrong_ca_rejected=true"));
    assert!(program.contains("bearer_sent=false"));
    assert!(program.contains("admin_token_sent=false"));
    assert!(program.contains("private_roundtrip=true"));
    assert!(program.contains("language_pack_official_version=1.1.0"));
    assert!(program.contains("language_pack_official_keys=30"));
    assert!(program.contains("input_immutable=true"));
}

#[test]
fn silvortex_account_is_native_pkce_bound_and_offline_optional() {
    let account = avalonia_source("Leserpent.Avalonia/SilvortexAccountSession.cs");
    let configuration = avalonia_source("Leserpent.Avalonia/SilvortexAccountConfiguration.cs");
    let control = avalonia_source("Leserpent.Avalonia/SilvortexAccountControl.cs");
    let catalog = avalonia_source("Leserpent.Avalonia/DesktopAccountCatalogs.cs");
    let localization = avalonia_source("Leserpent.Avalonia/DesktopLocalization.cs");
    let hub = avalonia_source("Leserpent.Avalonia/HubWindow.cs");
    let app = avalonia_source("Leserpent.Avalonia/LeserpentApp.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");
    let vault = avalonia_source("Leserpent.RemoteClient/RemoteTokenStore.cs");

    assert!(account.contains("code_challenge_method"));
    assert!(account.contains("ReviewedApplicationKey = \"leserpent\""));
    assert!(account.contains("ReviewedClientProfile = \"leserpent_desktop\""));
    assert!(account.contains("ReviewedClientId = \"svx_client_leserpent_desktop\""));
    assert!(account.contains("ReviewedScopes = \"openid profile email offline_access\""));
    assert!(account.contains("IsCanonicalIssuerHost"));
    assert!(account.contains("normalizedIssuer.AbsoluteUri"));
    assert!(account.contains("https://foo&bar/"));
    assert!(account.contains("SilvortexAccountOptions.ResolveClientId(null)"));
    assert!(account.contains("svx_client_self_hosted_fixture"));
    assert!(account.contains("SilvortexAccountConfigurationLoader.Load()"));
    assert!(account.contains("AuthorizationTransaction.Create()"));
    assert!(account.contains("FixedTimeEquals(state, expectedState)"));
    assert!(account.contains("VerifyIdTokenAsync"));
    assert!(account.contains("RSASignaturePadding.Pkcs1"));
    assert!(account.contains("urn:silvortex:assurance:mfa"));
    assert!(account.contains("PlatformCredentialVault.Store"));
    assert!(account.contains("CredentialService = \"org.gewyvern.leserpent.silvortex\""));
    assert!(account.contains("internal enum SilvortexAccountStatus"));
    assert!(account.contains("Status: SilvortexAccountStatus.SignInFailed"));
    assert!(account.contains("Status: SilvortexAccountStatus.RestoreFailed"));
    assert!(account.contains("private static void ValidateSnapshot("));
    assert!(account.contains("ExpectInvalidPresentationStatus();"));
    assert!(account.contains("Team Silvortex accepted an incompatible presentation status"));
    assert!(!account.contains("new(\"client_secret\""));
    assert!(control.contains("hub-silvortex-action"));
    assert!(control.contains("DesktopAccountCatalogs.Resolve(localization, key)"));
    assert!(control.contains("localization.Changed += OnLocalizationChanged"));
    assert!(control.contains("localization.Changed -= OnLocalizationChanged"));
    assert!(control.contains("VerifyLayoutEnvelope()"));
    assert!(control.contains("ProbeLocalizedPresentation("));
    assert!(catalog.contains("public const int KeyCount = 36;"));
    assert!(catalog.contains("Daemon credentials remain separate"));
    assert!(catalog.contains("DesktopDomainCatalogContract.Verify("));
    assert!(localization.contains("DesktopAccountCatalogs.VerifyContract();"));
    assert!(localization.contains("DesktopAccountCatalogs.KeyCount"));
    assert!(hub.contains("SilvortexAccountControl"));
    assert!(hub.contains("ProbeLocalizedAccountPresentation("));
    assert!(hub.contains("accountControl.VerifyLayoutEnvelope();"));
    assert!(app.contains("SilvortexAccountSession.FromRuntimeConfiguration()"));
    assert!(app.contains("localized_account_catalogs=7"));
    assert!(app.contains("localized_account_layouts=8"));
    assert!(configuration.contains("LeserpentSilvortexIssuer"));
    assert!(configuration.contains("MaxPlistBytes = 64 * 1024"));
    assert!(configuration.contains("DtdProcessing = DtdProcessing.Ignore"));
    assert!(configuration.contains("FileAttributes.ReparsePoint"));
    assert!(configuration.contains("PackagedBundle"));
    assert!(configuration.contains("refuses environment overrides"));
    assert!(configuration.contains("ResolvePackagedInfoPlist"));
    assert!(
        configuration.contains(
            "var clientId = SilvortexAccountOptions.ResolveClientId(environmentClientId)"
        )
    );
    assert!(program.contains("--verify-silvortex-account"));
    assert!(program.contains("reviewed_application=leserpent"));
    assert!(program.contains("reviewed_profile=leserpent_desktop"));
    assert!(program.contains("default_client_id=true"));
    assert!(program.contains("builtin_account_catalogs=7"));
    assert!(program.contains("account_semantic_keys=36"));
    assert!(program.contains("localized_account=true"));
    assert!(vault.contains("public static class PlatformCredentialVault"));
    assert!(vault.contains("LinuxSecretService.StoreAccount"));
}

#[test]
fn silvortex_account_proof_is_native_private_and_existing_credential_safe() {
    let proof = avalonia_source("Leserpent.Avalonia/SilvortexAccountProof.cs");
    let account = avalonia_source("Leserpent.Avalonia/SilvortexAccountSession.cs");
    let program = avalonia_source("Leserpent.Avalonia/Program.cs");

    assert!(proof.contains("ContractVersion = \"1.99.0\""));
    assert!(!proof.contains("--prove-silvortex-account"));
    assert!(proof.contains("RuntimeFeature.IsDynamicCodeSupported"));
    assert!(proof.contains("packaged-info-plist"));
    assert!(proof.contains("environment_override_accepted"));
    assert!(proof.contains("macOS desktop account proof requires the reviewed issuer embedded"));
    assert!(proof.contains("EnsureFreshCredential"));
    assert!(proof.contains("refuses to replace an existing Team Silvortex credential"));
    assert!(proof.contains("SilvortexAccountSession.CreateForProof(options)"));
    assert!(proof.contains("await activeSession.RestoreForProofAsync()"));
    assert!(proof.contains("CryptographicOperations.FixedTimeEquals"));
    assert!(proof.contains("await activeSession.SignOutAsync()"));
    assert!(proof.contains("AccessTokenRevocationAttempted"));
    assert!(proof.contains("RefreshTokenRevocationAttempted"));
    assert!(proof.contains("options.UnixCreateMode"));
    assert!(proof.contains("File.Move(temporary, outputPath, overwrite: false)"));
    assert!(proof.contains("account_identity_written"));
    assert!(proof.contains("credential_digest_written"));
    assert!(proof.contains("daemon_authority_touched"));
    assert!(proof.contains("preexisting_credential_overwritten"));
    assert!(account.contains("internal Task RestoreForProofAsync()"));
    assert!(account.contains("StoredRefreshTokenDigest"));
    assert!(program.contains("--verify-silvortex-account-proof"));
    assert!(program.contains("--prove-silvortex-account"));
    assert!(program.contains("packaged_macos_config=true"));
    assert!(program.contains("identity_retained=false"));
    assert!(program.contains("credential_retained=false"));
}
