use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn source(path: &str) -> String {
    std::fs::read_to_string(repository_root().join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

#[test]
fn mobile_shell_is_width_first_touch_safe_and_filter_disclosed() {
    let html = source("apps/leserpent/src/Leserpent/wwwroot/index.html");
    let styles = source("apps/leserpent/src/Leserpent/wwwroot/styles.css");
    let state = source("apps/leserpent/src/Leserpent/frontend/app.ts");
    let transport = source("apps/leserpent/src/Leserpent/frontend/20-security-transport.ts");
    let bootstrap = source("apps/leserpent/src/Leserpent/frontend/15-preferences-bootstrap.ts");

    for contract in [
        "id=\"mobile-filter-toggle\"",
        "aria-controls=\"app-toolbar\"",
        "aria-expanded=\"false\"",
        "id=\"mobile-filter-count\"",
        "id=\"app-toolbar\"",
    ] {
        assert!(
            html.contains(contract),
            "missing mobile shell contract {contract}"
        );
    }

    assert!(state.contains("mobileFiltersOpen: false"));
    assert!(transport.contains("if (width <= 600)"));
    assert!(transport.contains("return \"mobile\""));
    assert!(transport.contains("document.documentElement.dataset.mobileFiltersOpen"));
    assert!(transport.contains("setAttribute(\"aria-selected\""));
    assert!(bootstrap.contains("nodes.mobileFilterToggle?.addEventListener(\"click\""));
    assert!(bootstrap.contains("event.key === \"Escape\""));
    assert!(bootstrap.contains("event.key === \"ArrowRight\""));

    for contract in [
        ":root[data-layout-mode=\"mobile\"]",
        "min-height: 44px",
        "env(safe-area-inset-bottom)",
        "position: fixed",
        "grid-template-columns: repeat(5, minmax(0, 1fr))",
        ":root[data-mobile-filters-open=\"false\"] .app-toolbar",
    ] {
        assert!(
            styles.contains(contract),
            "missing adaptive CSS contract {contract}"
        );
    }

    assert!(styles.contains(
        ":root[data-layout-mode=\"safe-compact\"] body {\n  font-size: 14px;\n  overflow-x: hidden;\n  overflow-y: auto;"
    ));
}

#[test]
fn runtime_list_is_mobile_card_safe_and_keyboard_operable() {
    let html = source("apps/leserpent/src/Leserpent/wwwroot/index.html");
    let styles = source("apps/leserpent/src/Leserpent/wwwroot/styles.css");
    let renderer = source("apps/leserpent/src/Leserpent/frontend/47-runtime-list-renderer.ts");
    let overview = source("apps/leserpent/src/Leserpent/frontend/45-overview-renderers.ts");
    let bootstrap = source("apps/leserpent/src/Leserpent/frontend/15-preferences-bootstrap.ts");

    for contract in [
        "id=\"runtime-list-heading\"",
        "aria-labelledby=\"runtime-list-heading\"",
        "aria-describedby=\"runtime-count\"",
        "aria-live=\"polite\"",
    ] {
        assert!(
            html.contains(contract),
            "missing runtime list contract {contract}"
        );
    }

    for contract in [
        "data-runtime-cell=\"identity\"",
        "data-runtime-cell=\"capabilities\"",
        "data-runtime-cell=\"actions\"",
        "aria-selected=",
        "tabindex=",
        "runtimeActionLabel",
    ] {
        assert!(
            renderer.contains(contract),
            "missing runtime row contract {contract}"
        );
    }

    for contract in [
        "function selectRuntimeTableRow",
        "function closeOpenRuntimeRowMenu",
        "function protocolKeyToTranslationSegment",
        "function attentionReasonLabel",
        "event.key === \"ArrowDown\"",
        "event.key === \"ArrowUp\"",
        "event.key === \"Home\"",
        "event.key === \"End\"",
        "event.key === \"Enter\"",
    ] {
        assert!(
            bootstrap.contains(contract),
            "missing keyboard contract {contract}"
        );
    }

    assert!(renderer.contains("attentionReasonLabel(reason)"));
    assert!(overview.contains("attentionReasonLabel(reason)"));

    for contract in [
        ":root[data-runtime-list-layout=\"cards\"] .runtime-table-wrap table",
        "data-runtime-cell=\"identity\"",
        "grid-template-columns: repeat(2, minmax(0, 1fr))",
        "content: attr(data-label)",
        ":root[data-runtime-list-layout=\"cards\"] .runtime-row-menu",
    ] {
        assert!(
            styles.contains(contract),
            "missing runtime card CSS contract {contract}"
        );
    }
    assert!(bootstrap.contains("new ResizeObserver"));
    assert!(bootstrap.contains("entry.contentRect.width"));
}

#[test]
fn runtime_registration_is_progressive_secret_safe_and_error_recoverable() {
    let html = source("apps/leserpent/src/Leserpent/wwwroot/index.html");
    let styles = source("apps/leserpent/src/Leserpent/wwwroot/styles.css");
    let preview = source("apps/leserpent/src/Leserpent/frontend/46-register-preview.ts");
    let workflows = source("apps/leserpent/src/Leserpent/frontend/50-dashboard-workflows.ts");
    let bootstrap = source("apps/leserpent/src/Leserpent/frontend/15-preferences-bootstrap.ts");
    let transport = source("apps/leserpent/src/Leserpent/frontend/20-security-transport.ts");
    let simplified_chinese = source("apps/leserpent/src/Leserpent/frontend/11-i18n-zh-cn.ts");

    for contract in [
        "<fieldset class=\"register-section register-section-target\">",
        "<fieldset class=\"register-section register-section-access\">",
        "<fieldset class=\"register-section register-section-placement\">",
        "id=\"register-sidecar-details\"",
        "id=\"register-guidance\"",
        "aria-live=\"polite\"",
        "aria-controls=\"register-token\"",
        "aria-pressed=\"false\"",
        "aria-describedby=\"register-guidance register-result\"",
    ] {
        assert!(
            html.contains(contract),
            "missing registration UX contract {contract}"
        );
    }

    for contract in [
        "function registrationReadiness",
        "function revealRegistrationField",
        "function setRegistrationSecretVisibility",
        "function maskRegistrationSecrets",
        "function clearRegistrationSecrets",
        "field.scrollIntoView",
        "nodes.registerGuidance.dataset.tone",
    ] {
        assert!(
            preview.contains(contract),
            "missing registration behavior {contract}"
        );
    }

    assert!(workflows.contains("clearRegistrationSecrets();\n      renderRegisterPreview();"));
    assert!(workflows.contains("}), \"good\");\n      applyTabShell();"));
    assert!(workflows.contains("showRegistrationIssue(readiness);"));
    assert!(bootstrap.contains("document.addEventListener(\"visibilitychange\""));
    assert!(bootstrap.contains("nodes.registerForm.addEventListener(\"invalid\""));
    assert!(transport.contains("state.activeRuntimeMainTab !== \"register\""));
    assert!(transport.contains("syncRegistrationSecretToggles();\n  renderRegisterPreview();"));

    for contract in [
        ".register-section-target .register-section-fields",
        ".register-secret-toggle[aria-pressed=\"true\"]",
        ".register-guidance[data-tone=\"bad\"]",
        "position: sticky",
        "bottom: calc(76px + env(safe-area-inset-bottom))",
        ".register-action-buttons",
    ] {
        assert!(
            styles.contains(contract),
            "missing registration CSS contract {contract}"
        );
    }

    for key in [
        "targetSection:",
        "accessSection:",
        "sidecarSection:",
        "placementSection:",
        "checkingPlan:",
        "ready:",
        "fixHighlighted:",
    ] {
        assert!(
            simplified_chinese.contains(key),
            "missing localized registration guidance {key}"
        );
    }
}

#[test]
fn cleanup_controls_follow_daemon_mutation_capability() {
    let workflows = source("apps/leserpent/src/Leserpent/frontend/50-dashboard-workflows.ts");
    for contract in [
        "function cleanupMutationAvailable()",
        "webConsole?.cleanupAvailable !== false",
        "const cleanupUnavailable = !cleanupMutationAvailable();",
        "if (!cleanupMutationAvailable() || !count",
        "if (!cleanupMutationAvailable() || !plan?.runtimeCount",
    ] {
        assert!(
            workflows.contains(contract),
            "missing cleanup capability fence {contract}"
        );
    }
}

#[test]
fn orchestra_session_handoff_follows_host_capability_without_fake_success() {
    let renderer = source("apps/leserpent/src/Leserpent/frontend/48-orchestra-renderer.ts");
    let rust_host = source("crates/leserpentd/src/web_console.rs");
    let bridge = source("apps/leserpent/src/Leserpent/ProgramHealthEndpoints.cs");
    for contract in [
        "function orchestraSessionHandoffAvailable()",
        "webConsole?.orchestraSessionHandoffAvailable",
        "capabilities.routes.includes(\"/v1/orchestra/plans/{id}/session\")",
        "function orchestraMutationAvailable()",
        "webConsole?.orchestraMutationAvailable",
        "JSON.stringify([payload, sessionHandoffAvailable, mutationAvailable])",
        "JSON.stringify([normalized, mutationAvailable])",
        "plan.executionMode === \"automatic\" && mutationAvailable",
        "mutationAvailable && [\"queued\", \"running\"].includes(run.outcome)",
        "Session handoff requires durable Rust writer authority",
    ] {
        assert!(
            renderer.contains(contract),
            "missing Orchestra session capability fence {contract}"
        );
    }
    assert!(
        rust_host.contains(
            "\"orchestraSessionHandoffAvailable\": writer_enabled && persistence_enabled"
        )
    );
    assert!(
        rust_host.contains("\"orchestraMutationAvailable\": writer_enabled && persistence_enabled")
    );
    assert!(bridge.contains("\"/v1/orchestra/plans/{id}/session\""));
}

#[test]
fn runtime_detail_is_operator_first_localized_and_actionable() {
    let html = source("apps/leserpent/src/Leserpent/wwwroot/index.html");
    let styles = source("apps/leserpent/src/Leserpent/wwwroot/styles.css");
    let inspector = source("apps/leserpent/src/Leserpent/frontend/40-runtime-inspector.ts");
    let bootstrap = source("apps/leserpent/src/Leserpent/frontend/15-preferences-bootstrap.ts");
    let transport = source("apps/leserpent/src/Leserpent/frontend/20-security-transport.ts");
    let simplified_chinese = source("apps/leserpent/src/Leserpent/frontend/11-i18n-zh-cn.ts");

    for contract in [
        "id=\"runtime-detail-summary\"",
        "role=\"region\"",
        "aria-labelledby=\"runtime-detail-summary-heading\"",
        "data-i18n-aria-label=\"runtimeDetail.sectionsLabel\"",
        "data-runtime-detail-tab=\"identity\" type=\"button\" data-i18n=\"runtimeDetail.identity\"",
        "data-runtime-detail-tab=\"attention\" type=\"button\" data-i18n=\"runtimeDetail.attention\"",
    ] {
        assert!(
            html.contains(contract),
            "missing runtime detail semantic contract {contract}"
        );
    }

    for contract in [
        "function runtimeDetailTimestamp",
        "new Intl.DateTimeFormat(state.language",
        "function runtimeEvidenceItems",
        "function capabilitySupportLabel",
        "function runtimeDetailPosture",
        "function runtimeNeedsAttention",
        "function renderRuntimeDetailSummary",
        "function renderRuntimeCapabilities",
        "function renderRuntimeAttention",
        "return t(\"attention.hints.refreshStatus\")",
        "runtime.status.statusFetchedAt;",
        "data-recovery-action=\"${escapeHtml(kind)}\"",
    ] {
        assert!(
            inspector.contains(contract),
            "missing operator-first runtime detail behavior {contract}"
        );
    }

    assert!(bootstrap.contains("function normalizeRuntimeDetailTab"));
    assert!(bootstrap.contains("state.activeRuntimeDetailTab = \"attention\";"));
    assert!(bootstrap.contains("button[data-runtime-detail-target]"));
    assert!(transport.contains("normalizeRuntimeDetailTab(params.get(\"runtimeDetail\"))"));
    assert!(transport.contains("[data-i18n-aria-label]"));

    for contract in [
        ".runtime-detail-posture",
        ".runtime-detail-facts",
        ".runtime-detail-definition-grid",
        ".runtime-evidence-grid",
        ".runtime-capability-grid",
        ".runtime-recovery-grid",
        ".runtime-detail-subtab-button.has-attention",
        "@media (max-width: 600px)",
    ] {
        assert!(
            styles.contains(contract),
            "missing runtime detail CSS contract {contract}"
        );
    }

    for key in [
        "sectionsLabel:",
        "liveSummary:",
        "reviewAttention:",
        "evidenceAvailability:",
        "capabilitySource:",
        "fullySupported:",
    ] {
        assert!(
            simplified_chinese.contains(key),
            "missing localized runtime detail key {key}"
        );
    }
}

#[test]
fn runtime_window_workspace_is_bounded_lazy_keyboard_operable_and_deep_link_safe() {
    let html = source("apps/leserpent/src/Leserpent/wwwroot/index.html");
    let styles = source("apps/leserpent/src/Leserpent/wwwroot/styles.css");
    let inspector = source("apps/leserpent/src/Leserpent/frontend/40-runtime-inspector.ts");
    let bootstrap = source("apps/leserpent/src/Leserpent/frontend/15-preferences-bootstrap.ts");
    let transport = source("apps/leserpent/src/Leserpent/frontend/20-security-transport.ts");
    let simplified_chinese = source("apps/leserpent/src/Leserpent/frontend/11-i18n-zh-cn.ts");
    let guide = source("apps/leserpent/docs/runtime-window-workspace.md");

    for contract in [
        "id=\"runtime-window-toolbar\" class=\"runtime-window-toolbar hidden\" role=\"toolbar\"",
        "id=\"runtime-window-policy\"",
        "id=\"runtime-window-count\" class=\"chip\" aria-live=\"polite\"",
        "id=\"runtime-window-grid\" class=\"runtime-window-grid hidden\" role=\"list\"",
        "data-i18n-aria-label=\"runtimePanel.windows.workspaceLabel\"",
    ] {
        assert!(
            html.contains(contract),
            "missing runtime workspace semantic {contract}"
        );
    }

    for contract in [
        "const MAX_RUNTIME_WINDOWS = 8;",
        "const MAX_RUNTIME_WINDOW_STATE_BYTES = 64 * 1024;",
        "function sanitizeRuntimeWindowIds",
        "function sanitizeRuntimeWindowViews",
        "function runtimeWindowStateWithinLimit",
        "new TextEncoder().encode(value).byteLength <= MAX_RUNTIME_WINDOW_STATE_BYTES",
        "Object.create(null)",
        "function applyRuntimeWindowDeepLink",
        "state.runtimeWindowIntentPending = true;",
        "|| intentPending",
        "function runtimeWindowSuspendedMarkup",
        "if (!isActive)",
        "frame.src = \"about:blank\";",
        "function handleRuntimeWindowGridKeydown",
        "event.key === \"ArrowDown\"",
        "event.key === \"Home\"",
        "Math.min(Math.max(closedIndex, 0)",
        "function focusRuntimeWindowAfterClose",
        "focusRuntimeWindowAfterClose(state.activeRuntimeWindowId);",
        "nodes.runtimeWindowOpenSelected?.focus();",
        "nodes.runtimeWindowOpenAll.disabled",
    ] {
        assert!(
            inspector.contains(contract),
            "missing bounded runtime workspace behavior {contract}"
        );
    }

    let runtime_id_hydration = transport
        .find("state.selectedRuntimeId = params.get(\"runtimeId\") || null;")
        .expect("runtime ID must be hydrated");
    let deep_link_application = transport
        .find("applyRuntimeWindowDeepLink(state.selectedRuntimeId, state.runtimePanelView);")
        .expect("runtime deep link must be applied");
    assert!(runtime_id_hydration < deep_link_application);
    assert!(!transport.contains("state.runtimeWindowIds.push(state.selectedRuntimeId)"));
    assert!(bootstrap.contains(
        "nodes.runtimeWindowGrid?.addEventListener(\"keydown\", handleRuntimeWindowGridKeydown)"
    ));
    assert!(
        source("apps/leserpent/src/Leserpent/frontend/app.ts")
            .contains("runtimeWindowViews: Object.create(null)")
    );

    for contract in [
        ".runtime-window-toolbar-status",
        ".runtime-child-window.is-suspended",
        ".runtime-window-suspended",
        ".runtime-window-grid:has(.runtime-child-window:nth-child(2))",
        "grid-template-columns: minmax(0, 1fr);",
    ] {
        assert!(
            styles.contains(contract),
            "missing runtime workspace CSS contract {contract}"
        );
    }

    for key in [
        "capacity:",
        "policy:",
        "pausedTitle:",
        "pausedBody:",
        "limitReached:",
        "openAllLimited:",
        "workspaceLabel:",
    ] {
        assert!(
            simplified_chinese.contains(key),
            "missing localized runtime workspace key {key}"
        );
    }

    for contract in [
        "工作区硬上限为 8 个窗口",
        "只有活动窗口加载远端 iframe",
        "roving keyboard navigation",
        "无原型对象重建 view map",
    ] {
        assert!(
            guide.contains(contract),
            "missing runtime workspace guide contract {contract}"
        );
    }
}

#[test]
fn mobile_adaptation_is_protocolized_in_the_status_tensor() {
    let catalog: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repository_root().join("project/status/catalog.json")).unwrap(),
    )
    .unwrap();
    let cell = catalog["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["id"] == "leserpent-1x/web-console/browser-operations")
        .expect("web console status cell must exist");

    assert_eq!(cell["contract"]["version"], "1.4.10");
    for surface in [
        "width-first-mobile-layout",
        "mobile-filter-disclosure",
        "safe-area-bottom-navigation",
        "minimum-44px-touch-targets",
        "roving-keyboard-tab-semantics",
        "low-height-document-scroll-fallback",
        "mobile-runtime-card-table",
        "narrow-runtime-card-table",
        "container-aware-runtime-card-table",
        "runtime-row-roving-keyboard-selection",
        "contextual-runtime-action-labels",
        "single-open-runtime-action-menu",
        "polite-runtime-list-status",
        "protocol-reason-i18n-normalization",
        "semantic-registration-sections",
        "optional-sidecar-disclosure",
        "live-registration-readiness",
        "field-focused-registration-recovery",
        "operator-controlled-secret-visibility",
        "secret-remask-and-dom-clear",
        "mobile-sticky-registration-actions",
        "localized-registration-guidance",
        "deletion-aware-registration-guidance",
        "registration-success-detail-handoff",
        "live-registration-language-switch",
        "operator-first-runtime-detail",
        "persistent-runtime-posture-summary",
        "localized-runtime-detail-tabs",
        "structured-evidence-availability",
        "capability-support-cards",
        "server-command-recovery-cards",
        "attention-targeted-navigation",
        "validated-runtime-detail-route",
        "mobile-runtime-diagnostic-workbench",
        "bounded-runtime-window-workspace",
        "single-live-runtime-iframe",
        "sanitized-runtime-window-persistence",
        "deep-link-window-intent-priority",
        "runtime-window-keyboard-navigation",
        "runtime-window-close-focus-recovery",
    ] {
        assert!(
            cell["contract"]["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .any(|candidate| candidate == surface),
            "missing UX adaptation surface {surface}"
        );
    }
}
