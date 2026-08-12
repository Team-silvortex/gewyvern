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

    assert_eq!(cell["contract"]["version"], "1.4.5");
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
