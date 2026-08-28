const translations = {};
function mergeTranslations(base, patch) {
    const result = Array.isArray(base) ? [...base] : { ...base };
    for (const [key, value] of Object.entries(patch || {})) {
        if (value
            && typeof value === "object"
            && !Array.isArray(value)
            && base?.[key]
            && typeof base[key] === "object"
            && !Array.isArray(base[key])) {
            result[key] = mergeTranslations(base[key], value);
        }
        else {
            result[key] = value;
        }
    }
    return result;
}
translations.en = {
    hero: {
        title: "Control Plane Dashboard",
        subcopy: "A very-light fleet view for many nearby gewyvern runtimes.",
    },
    language: {
        label: "Language",
        auto: "Follow Browser",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    languagePacks: {
        title: "Language Packs",
        subcopy: "Install verified same-origin packs or import a local JSON pack.",
        refresh: "Refresh Catalog",
        import: "Import JSON",
        installedTitle: "Installed",
        catalogTitle: "Available Downloads",
        ready: "Open the catalog to discover language packs.",
        loading: "Loading language-pack catalog...",
        catalogReady: "{official} official locales: {builtin} built-in and {count} downloadable.",
        catalogFailed: "Catalog unavailable: {message}",
        catalogEmpty: "No downloadable packs are currently published.",
        noneInstalled: "No additional language packs installed.",
        install: "Install",
        installedLabel: "Installed",
        download: "Download",
        export: "Export",
        remove: "Remove",
        removeConfirm: "Remove {name}? The built-in English fallback will be used if this is the active language.",
        installed: "Installed {name}.",
        downloaded: "Downloaded {name}.",
        removed: "Language pack removed.",
        operationFailed: "Language-pack operation failed: {message}",
        coverageCore: "core UI",
    },
    theme: {
        label: "Theme",
        auto: "Follow System",
        light: "Day",
        dark: "Night",
    },
    security: {
        title: "Security",
        adminToken: "Admin Token",
        adminTokenPlaceholder: "optional for remote access",
        clearToken: "Clear Token",
        showToken: "Show Token",
        hideToken: "Hide Token",
        testToken: "Test Token",
        consoleSubcopy: "remote access / token tools",
        commandSet: "command set",
        localModeHint: "Loopback mode is active locally. Add a token only when you are intentionally connecting through a protected remote endpoint.",
        tokenStored: "Admin token stored in this browser session.",
        tokenCleared: "Admin token cleared.",
        tokenRequired: "This control plane denied the request. If you are connecting remotely, add the configured admin token.",
        tokenTestRunning: "Testing admin token...",
        tokenTestOk: "Admin token accepted for the current control-plane endpoint.",
        tokenTestFailed: "Admin token test failed: {message}",
        tokenMissing: "Add an admin token first, then test it.",
        lastTokenTest: "Last token test",
        neverTested: "never tested",
        testStateOk: "ok",
        testStateFailed: "failed",
        testStateRunning: "running",
        mode: "security mode",
        tokenConfigured: "admin token",
        publicDiscovery: "public discovery",
        configured: "configured",
        notConfigured: "not configured",
        enabled: "enabled",
        disabled: "disabled",
        loopbackOnly: "loopback only",
        tokenMode: "loopback or token",
        panelLocal: "Local",
        panelStored: "Token Ready",
        panelRunning: "Testing",
        panelOk: "Verified",
        panelNeedsToken: "Needs Token",
    },
    actions: {
        refreshAll: "Fleet Refresh All",
        refreshStatus: "Refresh Status",
        refreshCapabilities: "Refresh Capabilities",
    },
    filters: {
        title: "Fleet Filters",
        environment: "Environment",
        environmentPlaceholder: "prod",
        cluster: "Cluster",
        clusterPlaceholder: "alpha",
        role: "Role",
        rolePlaceholder: "edge",
        apply: "Apply Filters",
        clear: "Clear",
        allRuntimes: "all runtimes",
    },
    tabs: {
        overview: "Overview",
        runtimes: "Runtimes",
        register: "Register",
        persistence: "Persistence",
        sessions: "Sessions",
    },
    overview: {
        fleetSummary: "Fleet Summary",
        attentionSummary: "Attention Summary",
        triage: "triage",
        runtimesNeedingAttention: "Runtimes Needing Attention",
        kicker: "Fleet posture",
        spotlightTitle: "Stay oriented before you drill into individual runtimes.",
        spotlightBody: "Use this shell to move between fleet posture, runtime detail, intake, persistence, and sessions without falling into one long scrolling page.",
        spotlightRail1: "fleet slice",
        spotlightRail2: "current mode",
        summaryChip: "live counts",
    },
    runtimes: {
        title: "Runtimes",
        workspaceTabs: {
            select: "Select",
            register: "Register",
            detail: "Detail",
            panel: "Child Panel",
        },
        quickSearch: "Quick Search",
        quickSearchPlaceholder: "name or endpoint",
        sortBy: "Sort By",
        noMatch: "No runtimes match the current filter or search.",
        columns: {
            name: "Name",
            tags: "Tags",
            status: "Status",
            capabilitySurface: "Capability Surface",
            sidecar: "Sidecar",
            attention: "Attention",
            actions: "Actions",
        },
        sort: {
            name: "Name",
            status: "Status Source",
            snapshot: "Snapshot Kind",
        },
        actions: {
            openPanel: "Open Panel",
            attention: "Attention",
            status: "Status",
            all: "All",
            cleanup: "Cleanup",
            menu: "More",
            delete: "Delete",
            deleteFailed: "Delete Failed",
            deleteUnobserved: "Delete Unobserved",
            clearSlice: "Clear Slice",
        },
        cleanupHint: "Compact cleanup tools for the current slice.",
        cleanupHintProtected: "Protected slice detected. Cleanup actions are shown with extra caution.",
        states: {
            clear: "clear",
            none: "none",
            protected: "protected",
            noEnv: "no-env",
            noCluster: "no-cluster",
            noRole: "no-role",
            noCapabilities: "No fully-supported capabilities",
            capabilitiesCount: "{count} capabilities",
        },
    },
    runtimeDetail: {
        title: "Runtime Detail",
        nothingSelected: "nothing selected",
        empty: "Select a runtime from the table to inspect its capability, status, and attention state.",
        identity: "Identity",
        status: "Status",
        capabilities: "Capabilities",
        attention: "Attention",
        refreshAll: "Refresh This Runtime",
        refreshStatus: "Refresh Status",
        refreshCapabilities: "Refresh Capabilities",
        refreshSidecar: "Refresh Sidecar",
        copyLink: "Copy Runtime Link",
        registered: "registered",
        updated: "updated",
        source: "source",
        sidecarAccess: "sidecar access",
        sidecarProtected: "protected",
        sidecarOpen: "open",
        sidecarSource: "sidecar source",
        sidecarLearning: "sidecar learning",
        sidecarMemory: "sidecar memory",
        sidecarMemoryLatest: "latest checkpoint",
        resilienceStatus: "resilience status",
        resilienceSummary: "resilience summary",
        socketServiceStatus: "socket service",
        idleTimeouts: "idle timeouts (current / total)",
        snapshotKind: "snapshot kind",
        targetCount: "target count",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        trainingExampleJson: "training example json",
        trainingDatasetManifest: "training dataset manifest",
        exportJson: "export json",
        reportJson: "report json",
        reportHtml: "report html",
        noCapabilities: "No capabilities recorded.",
        clear: "clear",
        noAttention: "No attention reasons for this runtime.",
        needsAttention: "needs attention",
        sectionsLabel: "Runtime detail sections",
        liveSummary: "Live runtime posture",
        operational: "The latest control-plane snapshot is available and no action is currently required.",
        requiresReview: "This runtime needs operator review.",
        refreshRecommended: "The latest runtime refresh failed. Refresh before relying on this snapshot.",
        reviewAttention: "Review attention",
        inspectStatus: "Inspect status",
        lastObserved: "Last observed",
        notObserved: "Not observed",
        runtimeId: "Runtime ID",
        supportedCapabilities: "Supported capabilities",
        fullySupportedCount: "{count} fully supported",
        availableEvidence: "Available evidence",
        availableCount: "{count} available",
        attentionReasonCount: "{count} active reasons",
        statusOverview: "Runtime status",
        evidenceAvailability: "Evidence availability",
        available: "Available",
        missing: "Missing",
        sidecarOverview: "Sidecar status",
        capabilitySource: "Capability source",
        lastCapabilityRefresh: "Last capability refresh",
        support: {
            fullySupported: "Fully supported",
            partiallySupported: "Partially supported",
            notSupported: "Not supported",
            unsupported: "Unsupported",
            unknown: "Unknown",
        },
        none: "none",
        na: "n/a",
    },
    runtimePanel: {
        windows: {
            openSelected: "Open Selected",
            openAll: "Open All",
            closeAll: "Close All",
            close: "Close",
            activate: "Activate",
            external: "New Tab",
            count: "{count} windows",
            one: "1 window",
            capacity: "{count} / {limit} windows",
            policy: "Only the active panel is live; inactive panels are paused.",
            pausedTitle: "Panel paused",
            pausedBody: "Activate this window to load its remote panel. Inactive windows stay paused to protect browser and runtime resources.",
            pausedAction: "Activate panel",
            limitReached: "The workspace has reached its {limit}-window limit. Close a window before opening another.",
            openAllLimited: "Opened {count} of {total} runtimes. The workspace is capped at {limit} windows.",
            openAllComplete: "Prepared {count} runtime windows.",
            workspaceLabel: "Runtime window workspace",
            windowLabel: "{name} · {view}",
        },
        title: "Runtime Child Panel",
        notReady: "no runtime selected",
        empty: "Select a runtime to load its child control panel.",
        compactTrustObserved: "Live runtime snapshot is available.",
        compactTrustIdleReady: "Runtime is idle but healthy and ready.",
        compactTrustUnobserved: "Waiting for the first runtime snapshot.",
        compactTrustFetchFailed: "Last runtime refresh failed; treat data as stale.",
        compactTrustSidecarObserved: "Live sidecar snapshot is available.",
        compactTrustSidecarUnobserved: "Waiting for the first sidecar snapshot.",
        compactTrustSidecarFetchFailed: "Last sidecar refresh failed; treat data as stale.",
        compactTrustNoSidecar: "No paired sidecar is configured.",
        blankRuntimeTitle: "This runtime panel is not ready yet",
        blankRuntimeBody: "We know where this runtime lives, but it has not published panel-ready data yet. Use the control-plane summary above as your source of truth first.",
        blankSidecarTitle: "This sidecar panel is not ready yet",
        blankSidecarBody: "The paired diagnostic sidecar is configured, but it has not produced a panel-ready status snapshot yet. Start with the control-plane view, then refresh when needed.",
        blankFetchFailedTitle: "We could not safely open this panel yet",
        blankFetchFailedBody: "The latest status refresh failed, so showing the raw endpoint would be more confusing than helpful right now. Refresh first, then try again.",
        blankHintRefreshRuntime: "Try refreshing runtime status first.",
        blankHintRefreshSidecar: "Try refreshing sidecar status first.",
        blankHintEndpoint: "endpoint",
        breadcrumbFleet: "runtime panel",
        breadcrumbSource: "source",
        breadcrumbView: "view",
        currentView: "current view",
        sourceUrl: "source url",
        sourceStatus: "source status",
        trustTitle: "runtime trust",
        trustObserved: "Observed runtime state",
        trustObservedMessage: "This child panel is backed by an observed gewyvern snapshot and is generally safe to read as live runtime context.",
        trustIdleReady: "Idle-ready runtime state",
        trustIdleReadyMessage: "This runtime is currently idle but healthy. It has not produced a fresh payload snapshot yet, but the socket service is alive and ready for the next connection.",
        trustUnobserved: "Unobserved runtime state",
        trustUnobservedMessage: "This runtime has not published a latest snapshot yet, so treat the child panel as a thin endpoint shell until status arrives.",
        trustFetchFailed: "Status fetch failed",
        trustFetchFailedMessage: "The control plane could not refresh this runtime's status, so the child panel may be stale or unreachable right now.",
        trustSidecarObserved: "Observed sidecar state",
        trustSidecarObservedMessage: "This child panel is backed by a reachable paired etragon sidecar and is safe to read as nearby diagnostic-partner context.",
        trustSidecarUnobserved: "Sidecar not observed yet",
        trustSidecarUnobservedMessage: "A paired etragon endpoint is configured, but the control plane has not observed a sidecar status snapshot yet.",
        trustSidecarFetchFailed: "Sidecar status fetch failed",
        trustSidecarFetchFailedMessage: "The control plane could not refresh this paired etragon sidecar, so the sidecar panel may be stale or unreachable right now.",
        trustNoSidecar: "No paired sidecar",
        trustNoSidecarMessage: "This runtime does not currently advertise a paired etragon endpoint.",
        trustMeta: "status source: {source} · snapshot: {snapshot}",
        trustRefreshStatus: "Refresh Status Now",
        trustRefreshSidecar: "Refresh Sidecar Now",
        openExternal: "Open in New Tab",
        reading: {
            title: "Reading path",
            next: "next",
            via: "via {overlay}",
            target: "target {target}",
        },
        sources: {
            runtime: "Runtime",
            sidecar: "Sidecar",
        },
        views: {
            root: "Home",
            health: "Health",
            meta: "Latest Meta",
            summary: "Summary",
            analysis: "Analysis",
            training: "Training Example",
            dataset: "Training Dataset",
            export: "Export",
            reportJson: "Report JSON",
            reportHtml: "Report HTML",
            targets: "Targets",
            sidecarRoot: "Sidecar Home",
            sidecarHealth: "Sidecar Health",
            sidecarStatus: "Sidecar Status",
            sidecarMemory: "Memory",
            sidecarEnrichment: "Enrichment",
            sidecarOpinion: "Opinion",
        },
    },
    register: {
        title: "Register Runtime",
        intake: "very-light intake",
        targetSection: "Runtime target",
        targetSectionCopy: "Identify the gewyvern service that this control plane will manage.",
        accessSection: "Control access",
        accessSectionCopy: "Pair through a short-lived secret; it is never included in the preview.",
        sidecarSection: "Optional sidecar",
        sidecarSectionCopy: "Add this only when the runtime exposes a companion service.",
        placementSection: "Placement and discovery",
        placementSectionCopy: "Optional tags keep this runtime discoverable without changing its identity.",
        showToken: "Show",
        hideToken: "Hide",
        completeField: "Complete {field} to continue.",
        checkingPlan: "Checking the registration plan with the control plane...",
        planUnavailable: "The registration plan could not be verified: {message}",
        ready: "Registration plan verified. This runtime is ready to register.",
        fixHighlighted: "Review the highlighted field before registering this runtime.",
        name: "Name",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "Endpoint",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "Sidecar Endpoint",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        sidecarAdminToken: "Sidecar Admin Token",
        sidecarAdminTokenPlaceholder: "optional for protected etragon sidecars",
        pairingToken: "Pairing Token",
        tokenPlaceholder: "token-123",
        fetchCapabilities: "Fetch capability and latest-meta from gewyvern",
        submit: "Register Runtime",
        clear: "Clear Form",
        clearConfirm: "Clear the registration form? Endpoint and token values will be discarded.",
        untouched: "No runtime submitted yet.",
        previewTitle: "Live Preview",
        previewName: "name",
        previewSlice: "slice",
        previewEndpoint: "endpoint",
        previewSidecar: "sidecar",
        previewSidecarAccess: "sidecar access",
        previewPairing: "pairing token",
        previewCapabilityFetch: "capability fetch",
        suggested: "suggested",
        pendingRuntimeName: "pending runtime name",
        endpointPending: "pending",
        endpointValid: "valid",
        endpointInvalid: "invalid",
        sidecarUnpaired: "not paired",
        capabilityEnabled: "enabled",
        capabilityDisabled: "disabled",
        pairingReady: "provided",
        pairingMissing: "required",
        blockedEndpoint: "Registration blocked: endpoint must start with http:// or https:// and be a valid URL.",
        blockedSidecarEndpoint: "Registration blocked: sidecar endpoint must start with http:// or https:// and be a valid URL.",
        blockedDuplicate: "Registration blocked: {reason} already exists on {name} ({endpoint}).",
        deletionInProgress: "Registration is paused while this runtime is being deleted. Wait for cleanup to finish, then review the plan again.",
        duplicateNameAndEndpoint: "name and endpoint",
        duplicateName: "name",
        duplicateEndpoint: "endpoint",
        registering: "Registering runtime...",
        registeringShort: "Registering...",
        registered: "Registered {name} ({runtimeId}) into {slice} with status {status}.",
        failed: "Registration failed: {message}. If this runtime already exists, try selecting it from the table instead of registering again.",
        allRuntimes: "all runtimes",
    },
    persistence: {
        title: "Persistence",
        chip: "control-plane state",
        yes: "yes",
        no: "no",
        saveNow: "Save Now",
        exportState: "Export State",
        importState: "Import State",
        enabled: "enabled",
        schema: "schema",
        state: "state",
        stateFile: "state file",
        lastSaved: "last saved",
        restoredRuntimes: "restored runtimes",
        restoredSessions: "restored sessions",
        statePath: "state path",
        backupPath: "backup path",
        schemaVersion: "schema version",
        lastSavedAt: "last saved at",
        lastSaveError: "last save error",
        restoredFromSave: "restored from save",
        configured: "configured",
        missing: "missing",
        never: "never",
        unknown: "unknown",
        none: "none",
        clean: "clean",
        dirty: "dirty",
        saving: "Saving control-plane state...",
        saved: "Control-plane state saved.",
        saveFailed: "Save now failed: {message}",
        exporting: "Exporting control-plane state...",
        exported: "Control-plane state exported.",
        exportFailed: "Export failed: {message}",
        importing: "Importing control-plane state from {file}...",
        invalidJson: "selected file is not valid JSON",
        invalidStructure: "selected file is not a Leserpent control-plane state document",
        incompatibleSchema: "selected file uses unsupported schema version {schema}",
        importTooLarge: "selected file exceeds the 1 MiB import limit",
        importConfirm: "Import {file}? This replaces the current state ({currentRuntimes} runtimes, {currentSessions} sessions) with {runtimes} runtimes and {sessions} sessions.",
        importCancelled: "State import cancelled; the current state was not changed.",
        importingShort: "Importing...",
        imported: "Imported {runtimes} runtimes and {sessions} sessions.",
        importFailed: "Import failed: {message}",
    },
    attention: {
        noReasons: "No active attention reasons.",
        reasonLine: "{reason} · {count} runtimes",
        noRuntimes: "No runtimes need attention in this slice.",
        suggestedActions: "Suggested actions",
        actionHint: "Why this first",
        coolingDown: "cooling down",
        cooldownRemaining: "{seconds}s remaining",
        recentRecovery: "Recent recovery",
        noRecoveryHistory: "No recovery activity recorded yet.",
        critical: "critical",
        warning: "warning",
        statusFetchFailed: "status_fetch_failed",
        sidecarStatusFetchFailed: "sidecar_status_fetch_failed",
        noLatestSnapshot: "no current snapshot",
        noAnalysisJson: "no analysis JSON",
        actions: {
            refreshAll: "Refresh all",
            refreshStatus: "Refresh status",
            refreshSidecar: "Refresh sidecar",
            registerRuntime: "Register runtime",
        },
        hints: {
            refreshStatus: "Retry the runtime snapshot path first.",
            refreshAll: "Run the full refresh path when partial state is missing.",
            refreshSidecar: "Retry diagnostics separately when the sidecar path is stale.",
        },
        outcomes: {
            ok: "ok",
            authFailed: "auth failed",
            networkFailed: "network failed",
            incompleteData: "incomplete data",
            degraded: "degraded",
        },
    },
    sessions: {
        title: "Sessions",
        none: "No sessions yet.",
        runtime: "runtime",
    },
    metrics: {
        runtimes: "runtimes",
        latestSnapshots: "latest snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "sidecar context",
        diagnosticOpinions: "diagnostic opinions",
        pairedSidecars: "paired sidecars",
        healthySidecars: "healthy sidecars",
        critical: "critical",
        warning: "warning",
    },
    groups: {
        snapshotKinds: "snapshot kinds",
        statusSources: "status sources",
        sidecarStatusSources: "sidecar status sources",
        environments: "environments",
        clusters: "clusters",
        roles: "roles",
        empty: "No grouped data yet.",
    },
    statuses: {
        fetchFailed: "fetch failed",
        idleReady: "idle ready",
        unobserved: "unobserved",
        observedSnapshot: "{kind} snapshot",
        observed: "observed",
        sidecarObserved: "sidecar observed",
        sidecarStarting: "sidecar starting",
        sidecarDegraded: "sidecar degraded",
        sidecarFetchFailed: "sidecar fetch failed",
    },
    notifications: {
        noRuntimeSelected: "No runtime selected.",
        runtimeLinkCopied: "Runtime link copied.",
        runtimeLinkFailed: "Copy link failed: {message}",
        runtimeDeleteConfirm: "Delete runtime {name}? This also removes linked sessions and clears saved state for this entry.",
        runtimeDeleted: "Deleted runtime {name}. Removed {sessions} linked sessions.",
        runtimeDeleteFailed: "Delete runtime failed: {message}",
        runtimeDeleteFailedSliceConfirm: "Delete {count} fetch-failed runtimes in the current slice ({slice})? This also removes their linked sessions.",
        runtimeDeleteFailedSliceDone: "Deleted {count} failed runtimes from {slice}. Removed {sessions} linked sessions.",
        runtimeDeleteUnobservedSliceConfirm: "Delete {count} unobserved runtimes in the current slice ({slice})? This also removes their linked sessions.",
        runtimeDeleteUnobservedSliceDone: "Deleted {count} unobserved runtimes from {slice}. Removed {sessions} linked sessions.",
        runtimeClearSliceConfirm: "Delete all {count} runtimes in the current slice ({slice})? This also removes all linked sessions for that slice.",
        runtimeClearSliceChallenge: "Type {challenge} to confirm this destructive operation.",
        runtimeClearSliceChallengeFailed: "Slice cleanup cancelled because the confirmation text did not match.",
        runtimeClearSliceDone: "Cleared {count} runtimes from {slice}. Removed {sessions} linked sessions.",
        runtimeDeleteBatchFailed: "Bulk runtime cleanup failed: {message}",
        runtimeCleanupProtectedWarning: "Caution: this slice looks like live or prod. Double-check before continuing.",
        runtimeCleanupFailedCount: "{count} failed",
        runtimeCleanupRuntimeCount: "{count} runtimes",
        runtimeCleanupSessionCount: "{count} sessions",
        runtimeCleanupUnobservedCount: "{count} unobserved",
        runtimeCleanupPreviewLabel: "Targets",
        runtimeCleanupPreviewNone: "none",
        runtimeCleanupPreviewMore: " and {count} more",
        runtimeRefreshAll: "Runtime refresh-all",
        runtimeRefreshStatus: "Runtime status refresh",
        runtimeRefreshCapabilities: "Runtime capability refresh",
        runtimeRefreshComplete: "{label} complete.",
        runtimeRefreshFailed: "{label} failed: {message}",
        loading: "Loading control-plane state...",
        loaded: "Loaded {count} runtimes.",
        dashboardLoadFailed: "Dashboard load failed: {message}",
        fleetRefreshAll: "Fleet refresh-all",
        fleetStatusRefresh: "Fleet status refresh",
        fleetCapabilityRefresh: "Fleet capability refresh",
        fleetRefreshComplete: "{label} complete.",
        fleetRefreshFailed: "{label} failed: {message}",
        badgeUpdated: "updated",
    },
};
translations["zh-CN"] = {
    hero: {
        title: "控制平面面板",
        subcopy: "面向多台邻近 gewyvern runtime 的轻量级 fleet 视图。",
    },
    language: {
        label: "语言",
        auto: "跟随浏览器",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    languagePacks: {
        title: "语言包",
        subcopy: "安装经过校验的同源语言包，或导入本地 JSON 语言包。",
        refresh: "刷新目录",
        import: "导入 JSON",
        installedTitle: "已安装",
        catalogTitle: "可下载语言包",
        ready: "打开目录以发现可用语言包。",
        loading: "正在加载语言包目录...",
        catalogReady: "官方支持 {official} 种语言：内置 {builtin} 种，可下载 {count} 种。",
        catalogFailed: "语言包目录不可用：{message}",
        catalogEmpty: "当前没有已发布的可下载语言包。",
        noneInstalled: "尚未安装附加语言包。",
        install: "安装",
        installedLabel: "已安装",
        download: "下载",
        export: "导出",
        remove: "卸载",
        removeConfirm: "要卸载 {name} 吗？如果它是当前语言，界面会回退到内置语言。",
        installed: "已安装 {name}。",
        downloaded: "已下载 {name}。",
        removed: "语言包已卸载。",
        operationFailed: "语言包操作失败：{message}",
        coverageCore: "核心界面",
    },
    theme: {
        label: "主题",
        auto: "跟随系统",
        light: "白天",
        dark: "夜晚",
    },
    security: {
        title: "安全",
        adminToken: "管理令牌",
        adminTokenPlaceholder: "远程接入时可选",
        clearToken: "清除令牌",
        showToken: "显示令牌",
        hideToken: "隐藏令牌",
        testToken: "测试令牌",
        consoleSubcopy: "远程接入 / 令牌工具",
        commandSet: "命令组",
        localModeHint: "当前本地是 loopback 模式。只有在你明确通过受保护的远程入口访问时，才需要填管理令牌。",
        tokenStored: "管理令牌已保存在当前浏览器。",
        tokenCleared: "管理令牌已清除。",
        tokenRequired: "这次请求被控制面拒绝了。如果你现在是远程接入，请填入已配置的管理令牌。",
        tokenTestRunning: "正在测试管理令牌...",
        tokenTestOk: "这枚管理令牌已被当前控制面接受。",
        tokenTestFailed: "管理令牌测试失败：{message}",
        tokenMissing: "先填一枚管理令牌，再测试。",
        lastTokenTest: "最近一次令牌测试",
        neverTested: "从未测试",
        testStateOk: "成功",
        testStateFailed: "失败",
        testStateRunning: "测试中",
        mode: "安全模式",
        tokenConfigured: "管理令牌",
        publicDiscovery: "公网发现",
        configured: "已配置",
        notConfigured: "未配置",
        enabled: "开启",
        disabled: "关闭",
        loopbackOnly: "仅本机",
        tokenMode: "本机或令牌",
        panelLocal: "本地",
        panelStored: "令牌就绪",
        panelRunning: "测试中",
        panelOk: "已验证",
        panelNeedsToken: "需要令牌",
    },
    actions: {
        refreshAll: "整组刷新",
        refreshStatus: "刷新状态",
        refreshCapabilities: "刷新能力",
    },
    filters: {
        title: "Fleet 筛选",
        environment: "环境",
        environmentPlaceholder: "prod",
        cluster: "集群",
        clusterPlaceholder: "alpha",
        role: "角色",
        rolePlaceholder: "edge",
        apply: "应用筛选",
        clear: "清空",
        allRuntimes: "全部 runtimes",
    },
    tabs: {
        overview: "总览",
        runtimes: "节点",
        register: "注册",
        persistence: "持久化",
        sessions: "会话",
    },
    overview: {
        fleetSummary: "Fleet 总览",
        attentionSummary: "告警总览",
        triage: "分诊",
        runtimesNeedingAttention: "需要关注的 runtimes",
        kicker: "Fleet 姿态",
        spotlightTitle: "先建立全局方向感，再深入到单个 runtime。",
        spotlightBody: "现在你可以在同一个 shell 里切换 fleet 姿态、runtime 详情、注册 intake、持久化和 sessions，不再需要一条很长的滚动页面。",
        spotlightRail1: "当前切片",
        spotlightRail2: "当前模式",
        summaryChip: "实时计数",
    },
    runtimes: {
        title: "Runtimes",
        workspaceTabs: {
            select: "选取",
            register: "注册",
            detail: "详情",
            panel: "子面板",
        },
        quickSearch: "快速搜索",
        quickSearchPlaceholder: "名称或 endpoint",
        sortBy: "排序方式",
        noMatch: "当前筛选或搜索下没有匹配的 runtime。",
        columns: {
            name: "名称",
            tags: "标签",
            status: "状态",
            capabilitySurface: "能力面",
            sidecar: "Sidecar",
            attention: "关注项",
            actions: "操作",
        },
        sort: {
            name: "名称",
            status: "状态来源",
            snapshot: "快照类型",
        },
        actions: {
            openPanel: "打开子窗口",
            attention: "查看关注",
            status: "刷新状态",
            all: "全部刷新",
            cleanup: "清理",
            menu: "更多",
            delete: "删除",
            deleteFailed: "删除失败项",
            deleteUnobserved: "删除未观测项",
            clearSlice: "清空切片",
        },
        cleanupHint: "当前切片的紧凑清理工具。",
        cleanupHintProtected: "检测到受保护切片，下面的清理动作会按更高风险来对待。",
        states: {
            clear: "正常",
            none: "无",
            protected: "受保护",
            noEnv: "无环境",
            noCluster: "无集群",
            noRole: "无角色",
            noCapabilities: "没有 fully-supported 的能力",
            capabilitiesCount: "{count} 个能力",
        },
    },
    runtimeDetail: {
        title: "节点详情",
        nothingSelected: "未选择节点",
        empty: "从表格中选择一个 runtime，就能查看它的 capability、status 和 attention 状态。",
        identity: "身份信息",
        status: "状态",
        capabilities: "能力",
        attention: "关注项",
        refreshAll: "刷新该节点",
        refreshStatus: "刷新状态",
        refreshCapabilities: "刷新能力",
        refreshSidecar: "刷新 Sidecar",
        copyLink: "复制节点链接",
        registered: "注册时间",
        updated: "更新时间",
        source: "来源",
        sidecarAccess: "sidecar 访问",
        sidecarProtected: "受保护",
        sidecarOpen: "开放",
        sidecarSource: "sidecar 来源",
        sidecarLearning: "sidecar 学习态",
        sidecarMemory: "sidecar 记忆",
        sidecarMemoryLatest: "最新检查点",
        resilienceStatus: "韧性状态",
        resilienceSummary: "韧性摘要",
        socketServiceStatus: "socket 服务",
        idleTimeouts: "空闲超时（当前 / 累计）",
        snapshotKind: "快照类型",
        targetCount: "target 数量",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        trainingExampleJson: "training example json",
        trainingDatasetManifest: "training dataset manifest",
        exportJson: "export json",
        reportJson: "report json",
        reportHtml: "report html",
        noCapabilities: "当前没有记录 capability。",
        clear: "正常",
        noAttention: "该 runtime 当前没有 attention reason。",
        needsAttention: "是否需要关注",
        sectionsLabel: "节点详情分区",
        liveSummary: "实时节点态势",
        operational: "控制平面已有最新快照，当前无需操作。",
        requiresReview: "该节点需要操作者检查。",
        refreshRecommended: "最近一次节点刷新失败，请先刷新再依赖这份快照。",
        reviewAttention: "检查关注项",
        inspectStatus: "查看状态",
        lastObserved: "最近观测",
        notObserved: "尚未观测",
        runtimeId: "Runtime ID",
        supportedCapabilities: "已支持能力",
        fullySupportedCount: "{count} 项完全支持",
        availableEvidence: "可用证据",
        availableCount: "{count} 项可用",
        attentionReasonCount: "{count} 个活跃原因",
        statusOverview: "节点状态",
        evidenceAvailability: "证据可用性",
        available: "可用",
        missing: "缺失",
        sidecarOverview: "Sidecar 状态",
        capabilitySource: "能力来源",
        lastCapabilityRefresh: "最近能力刷新",
        support: {
            fullySupported: "完全支持",
            partiallySupported: "部分支持",
            notSupported: "不支持",
            unsupported: "不支持",
            unknown: "未知",
        },
        none: "无",
        na: "暂无",
    },
    runtimePanel: {
        windows: {
            openSelected: "打开所选实例",
            openAll: "全部打开",
            closeAll: "全部关闭",
            close: "关闭",
            activate: "激活",
            external: "新标签页",
            count: "{count} 个窗口",
            one: "1 个窗口",
            capacity: "{count} / {limit} 个窗口",
            policy: "仅活动面板保持在线，其他面板会暂停以保护资源。",
            pausedTitle: "面板已暂停",
            pausedBody: "激活这个窗口后才会加载远端面板；非活动窗口保持暂停，避免占满浏览器和 runtime 资源。",
            pausedAction: "激活面板",
            limitReached: "工作区已达到 {limit} 个窗口上限，请先关闭一个窗口。",
            openAllLimited: "已打开 {total} 个 runtime 中的 {count} 个；工作区上限为 {limit} 个窗口。",
            openAllComplete: "已准备好 {count} 个 runtime 窗口。",
            workspaceLabel: "Runtime 窗口工作区",
            windowLabel: "{name} · {view}",
        },
        title: "Runtime 子面板",
        notReady: "尚未选择 runtime",
        empty: "选择一个 runtime 后，就能加载它的子控制页面。",
        compactTrustObserved: "runtime 实时快照已可用。",
        compactTrustIdleReady: "runtime 当前空闲，但健康可接入。",
        compactTrustUnobserved: "正在等待第一份 runtime 快照。",
        compactTrustFetchFailed: "最近一次 runtime 刷新失败，请把当前数据当作可能过期。",
        compactTrustSidecarObserved: "sidecar 实时快照已可用。",
        compactTrustSidecarUnobserved: "正在等待第一份 sidecar 快照。",
        compactTrustSidecarFetchFailed: "最近一次 sidecar 刷新失败，请把当前数据当作可能过期。",
        compactTrustNoSidecar: "当前没有配置 paired sidecar。",
        blankRuntimeTitle: "这个 runtime 面板还没准备好",
        blankRuntimeBody: "我们已经知道这台 runtime 在哪，但它还没有发布适合直接展示的面板数据。现在先把上面的控制平面摘要当作准绳更合适。",
        blankSidecarTitle: "这个 sidecar 面板还没准备好",
        blankSidecarBody: "配对的诊断 sidecar 已经配置好了，但还没有产出可稳定展示的状态快照。现在先看控制平面视图，再按需刷新。",
        blankFetchFailedTitle: "这个面板暂时不适合直接打开",
        blankFetchFailedBody: "最近一次状态刷新失败了，这时直接展示原始 endpoint 往往比帮助更容易误导。先刷新，再回来查看会更稳。",
        blankHintRefreshRuntime: "先刷新一下 runtime 状态。",
        blankHintRefreshSidecar: "先刷新一下 sidecar 状态。",
        blankHintEndpoint: "endpoint",
        breadcrumbFleet: "runtime 面板",
        breadcrumbSource: "来源",
        breadcrumbView: "当前视图",
        currentView: "当前视图",
        sourceUrl: "来源地址",
        sourceStatus: "来源状态",
        trustTitle: "可信度提示",
        trustObserved: "已观测到 runtime 状态",
        trustObservedMessage: "这块子面板背后已经有可用的 gewyvern latest snapshot，一般可以把它当成实时 runtime 上下文来读。",
        trustIdleReady: "runtime 空闲就绪",
        trustIdleReadyMessage: "这条 runtime 当前处于空闲但健康的状态。它还没有产出新的 payload 快照，但 socket 服务是活的，下一次连接随时可以接进来。",
        trustUnobserved: "runtime 尚未观测",
        trustUnobservedMessage: "这条 runtime 还没有发布 latest snapshot，所以在状态到达前，更适合把子面板理解成一个薄的 endpoint 壳。",
        trustFetchFailed: "状态抓取失败",
        trustFetchFailedMessage: "控制面这次没能刷新这条 runtime 的状态，所以右侧子面板当前可能是陈旧的，或者已经不可达。",
        trustSidecarObserved: "Sidecar 状态已观测",
        trustSidecarObservedMessage: "这块子面板背后已经能连到 paired etragon sidecar，可以把它当成邻近诊断搭档的实时上下文来读。",
        trustSidecarUnobserved: "Sidecar 尚未观测",
        trustSidecarUnobservedMessage: "这条 runtime 已经配置了 paired etragon endpoint，但控制面还没有拿到 sidecar 状态快照。",
        trustSidecarFetchFailed: "Sidecar 状态抓取失败",
        trustSidecarFetchFailedMessage: "控制面这次没能刷新 paired etragon sidecar，所以右侧 sidecar 子面板当前可能是陈旧的，或者已经不可达。",
        trustNoSidecar: "没有 paired sidecar",
        trustNoSidecarMessage: "这条 runtime 当前还没有配置 etragon sidecar endpoint。",
        trustMeta: "状态来源：{source} · 快照：{snapshot}",
        trustRefreshStatus: "立即刷新状态",
        trustRefreshSidecar: "立即刷新 Sidecar",
        openExternal: "新标签打开",
        reading: {
            title: "阅读跳转",
            next: "下一跳",
            via: "经由 {overlay}",
            target: "目标 {target}",
        },
        sources: {
            runtime: "Runtime",
            sidecar: "Sidecar",
        },
        views: {
            root: "主页",
            health: "健康",
            meta: "最新 Meta",
            summary: "摘要",
            analysis: "分析",
            training: "训练样本",
            dataset: "训练数据集",
            export: "导出",
            reportJson: "报告 JSON",
            reportHtml: "报告 HTML",
            targets: "Targets",
            sidecarRoot: "Sidecar 主页",
            sidecarHealth: "Sidecar 健康",
            sidecarStatus: "Sidecar 状态",
            sidecarMemory: "记忆",
            sidecarEnrichment: "补强",
            sidecarOpinion: "诊断意见",
        },
    },
    register: {
        title: "注册 Runtime",
        intake: "轻量 intake",
        targetSection: "Runtime 目标",
        targetSectionCopy: "指定由当前控制面管理的 gewyvern 服务。",
        accessSection: "控制访问",
        accessSectionCopy: "使用短期令牌完成配对；令牌绝不会出现在预览中。",
        sidecarSection: "可选 Sidecar",
        sidecarSectionCopy: "仅在 runtime 暴露配套服务时添加。",
        placementSection: "归属与发现",
        placementSectionCopy: "可选标签用于发现 runtime，不会改变它的身份。",
        showToken: "显示",
        hideToken: "隐藏",
        completeField: "请填写{field}后继续。",
        checkingPlan: "正在通过控制面校验注册计划...",
        planUnavailable: "无法校验注册计划：{message}",
        ready: "注册计划已通过校验，可以注册该 runtime。",
        fixHighlighted: "注册前请检查高亮字段。",
        name: "名称",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "Endpoint",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "Sidecar Endpoint",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        sidecarAdminToken: "Sidecar 管理令牌",
        sidecarAdminTokenPlaceholder: "需要访问受保护的 etragon sidecar 时再填写",
        pairingToken: "配对令牌",
        tokenPlaceholder: "token-123",
        fetchCapabilities: "注册时从 gewyvern 拉取 capability 和 latest-meta",
        submit: "注册 Runtime",
        clear: "清空表单",
        clearConfirm: "要清空注册表单吗？Endpoint 和令牌内容将被丢弃。",
        untouched: "还没有提交 runtime。",
        previewTitle: "实时预览",
        previewName: "名称",
        previewSlice: "归属切片",
        previewEndpoint: "endpoint",
        previewSidecar: "sidecar",
        previewSidecarAccess: "sidecar 访问",
        previewPairing: "配对令牌",
        previewCapabilityFetch: "能力抓取",
        suggested: "建议",
        pendingRuntimeName: "等待生成 runtime 名称",
        endpointPending: "待输入",
        endpointValid: "有效",
        endpointInvalid: "无效",
        sidecarUnpaired: "未配对",
        capabilityEnabled: "开启",
        capabilityDisabled: "关闭",
        pairingReady: "已填写",
        pairingMissing: "必填",
        blockedEndpoint: "注册已拦截：endpoint 必须以 http:// 或 https:// 开头，并且是合法 URL。",
        blockedSidecarEndpoint: "注册已拦截：sidecar endpoint 必须以 http:// 或 https:// 开头，并且是合法 URL。",
        blockedDuplicate: "注册已拦截：{reason} 已存在于 {name} ({endpoint})。",
        deletionInProgress: "该 runtime 正在删除，注册已暂停。请等待清理完成后重新检查计划。",
        duplicateNameAndEndpoint: "名称和 endpoint",
        duplicateName: "名称",
        duplicateEndpoint: "endpoint",
        registering: "正在注册 runtime...",
        registeringShort: "注册中...",
        registered: "已将 {name} ({runtimeId}) 注册到 {slice}，当前状态为 {status}。",
        failed: "注册失败：{message}。如果这条 runtime 已经存在，建议直接在表格里选中它。",
        allRuntimes: "全部 runtimes",
    },
    persistence: {
        title: "持久化",
        chip: "控制面状态",
        yes: "是",
        no: "否",
        saveNow: "立即保存",
        exportState: "导出状态",
        importState: "导入状态",
        enabled: "已启用",
        schema: "schema",
        state: "状态",
        stateFile: "状态文件",
        lastSaved: "最近保存",
        restoredRuntimes: "恢复 runtimes",
        restoredSessions: "恢复 sessions",
        statePath: "状态文件路径",
        backupPath: "备份路径",
        schemaVersion: "schema 版本",
        lastSavedAt: "最近保存时间",
        lastSaveError: "最近保存错误",
        restoredFromSave: "恢复来源",
        configured: "已配置",
        missing: "缺失",
        never: "从未",
        unknown: "未知",
        none: "无",
        clean: "已同步",
        dirty: "有变更",
        saving: "正在保存控制面状态...",
        saved: "控制面状态已保存。",
        saveFailed: "保存失败：{message}",
        exporting: "正在导出控制面状态...",
        exported: "控制面状态已导出。",
        exportFailed: "导出失败：{message}",
        importing: "正在从 {file} 导入控制面状态...",
        invalidJson: "所选文件不是合法 JSON",
        invalidStructure: "所选文件不是 Leserpent 控制面状态文档",
        incompatibleSchema: "所选文件使用了不受支持的 schema 版本 {schema}",
        importTooLarge: "所选文件超过 1 MiB 导入上限",
        importConfirm: "要导入 {file} 吗？这会用 {runtimes} 个 runtime 和 {sessions} 个 session 覆盖当前的 {currentRuntimes} 个 runtime 与 {currentSessions} 个 session。",
        importCancelled: "已取消状态导入，当前状态没有改变。",
        importingShort: "导入中...",
        imported: "已导入 {runtimes} 个 runtime 和 {sessions} 个 session。",
        importFailed: "导入失败：{message}",
    },
    attention: {
        noReasons: "当前没有 active attention reasons。",
        reasonLine: "{reason} · {count} 个 runtime",
        noRuntimes: "当前切片里没有需要关注的 runtime。",
        suggestedActions: "建议动作",
        actionHint: "为什么先做这个",
        coolingDown: "冷却中",
        cooldownRemaining: "剩余 {seconds} 秒",
        recentRecovery: "最近恢复记录",
        noRecoveryHistory: "还没有恢复动作记录。",
        critical: "严重",
        warning: "警告",
        statusFetchFailed: "状态抓取失败",
        sidecarStatusFetchFailed: "sidecar 状态抓取失败",
        noLatestSnapshot: "缺少最新快照",
        noAnalysisJson: "缺少分析 JSON",
        actions: {
            refreshAll: "全部刷新",
            refreshStatus: "刷新状态",
            refreshSidecar: "刷新 Sidecar",
            registerRuntime: "注册 runtime",
        },
        hints: {
            refreshStatus: "先重试 runtime 快照链路。",
            refreshAll: "当局部状态缺失时，走完整刷新更稳。",
            refreshSidecar: "当诊断链路过期时，单独重试 sidecar。",
        },
        outcomes: {
            ok: "正常",
            authFailed: "鉴权失败",
            networkFailed: "网络失败",
            incompleteData: "数据不完整",
            degraded: "降级",
        },
    },
    sessions: {
        title: "Sessions",
        none: "当前还没有 session。",
        runtime: "runtime",
    },
    metrics: {
        runtimes: "runtimes",
        latestSnapshots: "latest snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "sidecar context",
        diagnosticOpinions: "diagnostic opinions",
        pairedSidecars: "paired sidecars",
        healthySidecars: "healthy sidecars",
        critical: "严重",
        warning: "警告",
    },
    groups: {
        snapshotKinds: "快照类型",
        statusSources: "状态来源",
        sidecarStatusSources: "sidecar 状态来源",
        environments: "环境",
        clusters: "集群",
        roles: "角色",
        empty: "当前还没有分组数据。",
    },
    statuses: {
        fetchFailed: "抓取失败",
        idleReady: "空闲就绪",
        unobserved: "未观测",
        observedSnapshot: "{kind} 快照",
        observed: "已观测",
        sidecarObserved: "sidecar 已观测",
        sidecarStarting: "sidecar 启动中",
        sidecarDegraded: "sidecar 已降级",
        sidecarFetchFailed: "sidecar 抓取失败",
    },
    notifications: {
        noRuntimeSelected: "当前没有选中的 runtime。",
        runtimeLinkCopied: "已复制 runtime 链接。",
        runtimeLinkFailed: "复制链接失败：{message}",
        runtimeDeleteConfirm: "要删除 runtime {name} 吗？这会同时移除关联 session，并从持久化状态里清掉这条记录。",
        runtimeDeleted: "已删除 runtime {name}，并移除了 {sessions} 个关联 session。",
        runtimeDeleteFailed: "删除 runtime 失败：{message}",
        runtimeDeleteFailedSliceConfirm: "要删除当前切片（{slice}）里这 {count} 个抓取失败的 runtime 吗？这也会移除它们关联的 session。",
        runtimeDeleteFailedSliceDone: "已从 {slice} 删除 {count} 个失败 runtime，并移除了 {sessions} 个关联 session。",
        runtimeDeleteUnobservedSliceConfirm: "要删除当前切片（{slice}）里这 {count} 个未观测 runtime 吗？这也会移除它们关联的 session。",
        runtimeDeleteUnobservedSliceDone: "已从 {slice} 删除 {count} 个未观测 runtime，并移除了 {sessions} 个关联 session。",
        runtimeClearSliceConfirm: "要清空当前切片（{slice}）里的全部 {count} 个 runtime 吗？这也会移除这个切片下的所有关联 session。",
        runtimeClearSliceChallenge: "请输入 {challenge} 以确认这项破坏性操作。",
        runtimeClearSliceChallengeFailed: "确认文字不匹配，已取消清空切片。",
        runtimeClearSliceDone: "已清空 {slice}，删除了 {count} 个 runtime，并移除了 {sessions} 个关联 session。",
        runtimeDeleteBatchFailed: "批量清理 runtime 失败：{message}",
        runtimeCleanupProtectedWarning: "注意：这个切片看起来像 live 或 prod，继续前请再确认一次。",
        runtimeCleanupFailedCount: "{count} 个失败项",
        runtimeCleanupRuntimeCount: "{count} 个 runtime",
        runtimeCleanupSessionCount: "{count} 个 session",
        runtimeCleanupUnobservedCount: "{count} 个未观测项",
        runtimeCleanupPreviewLabel: "将影响",
        runtimeCleanupPreviewNone: "无",
        runtimeCleanupPreviewMore: " 等另外 {count} 个",
        runtimeRefreshAll: "刷新该节点的全部信息",
        runtimeRefreshStatus: "刷新该节点状态",
        runtimeRefreshCapabilities: "刷新该节点能力",
        runtimeRefreshComplete: "{label} 完成。",
        runtimeRefreshFailed: "{label} 失败：{message}",
        loading: "正在加载控制面状态...",
        loaded: "已加载 {count} 个 runtime。",
        dashboardLoadFailed: "面板加载失败：{message}",
        fleetRefreshAll: "整组全部刷新",
        fleetStatusRefresh: "整组状态刷新",
        fleetCapabilityRefresh: "整组能力刷新",
        fleetRefreshComplete: "{label} 完成。",
        fleetRefreshFailed: "{label} 失败：{message}",
        badgeUpdated: "已更新",
    },
};
translations["zh-TW"] = mergeTranslations(translations.en, {
    hero: {
        title: "控制平面面板",
        subcopy: "面向多台鄰近 gewyvern runtime 的輕量 fleet 視圖。",
    },
    language: {
        label: "語言",
        auto: "跟隨瀏覽器",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    theme: {
        label: "主題",
        auto: "跟隨系統",
        light: "白天",
        dark: "夜晚",
    },
    security: {
        title: "安全",
        adminToken: "管理令牌",
        adminTokenPlaceholder: "遠端接入時可選",
        clearToken: "清除令牌",
        showToken: "顯示令牌",
        hideToken: "隱藏令牌",
        testToken: "測試令牌",
        localModeHint: "目前本地為 loopback 模式。只有在你明確透過受保護的遠端入口連線時，才需要填管理令牌。",
        tokenStored: "管理令牌已保存在目前瀏覽器。",
        tokenCleared: "管理令牌已清除。",
        tokenRequired: "這次請求被控制面拒絕了。如果你現在是遠端接入，請填入已配置的管理令牌。",
        tokenTestRunning: "正在測試管理令牌...",
        tokenTestOk: "這枚管理令牌已被目前控制面接受。",
        tokenTestFailed: "管理令牌測試失敗：{message}",
        tokenMissing: "先填一枚管理令牌，再測試。",
        lastTokenTest: "最近一次令牌測試",
        neverTested: "從未測試",
        testStateOk: "成功",
        testStateFailed: "失敗",
        testStateRunning: "測試中",
        mode: "安全模式",
        tokenConfigured: "管理令牌",
        publicDiscovery: "公網發現",
        configured: "已配置",
        notConfigured: "未配置",
        enabled: "開啟",
        disabled: "關閉",
        loopbackOnly: "僅本機",
        tokenMode: "本機或令牌",
    },
    actions: {
        refreshAll: "整組刷新",
        refreshStatus: "刷新狀態",
        refreshCapabilities: "刷新能力",
    },
    filters: {
        title: "Fleet 篩選",
        environment: "環境",
        environmentPlaceholder: "prod",
        cluster: "叢集",
        clusterPlaceholder: "alpha",
        role: "角色",
        rolePlaceholder: "edge",
        apply: "套用篩選",
        clear: "清空",
        allRuntimes: "全部 runtimes",
    },
    tabs: {
        overview: "總覽",
        runtimes: "節點",
        register: "註冊",
        persistence: "持久化",
        sessions: "工作階段",
    },
    overview: {
        fleetSummary: "Fleet 總覽",
        attentionSummary: "告警總覽",
        triage: "分診",
        runtimesNeedingAttention: "需要關注的 runtimes",
        kicker: "Fleet 姿態",
        spotlightTitle: "先建立全局方向感，再深入到單個 runtime。",
        spotlightBody: "現在你可以在同一個 shell 裡切換 fleet 姿態、runtime 詳情、註冊 intake、持久化和 sessions，不再需要一條很長的滾動頁面。",
        spotlightRail1: "目前切片",
        spotlightRail2: "目前模式",
        summaryChip: "即時計數",
    },
    runtimes: {
        title: "Runtimes",
        workspaceTabs: {
            select: "選取",
            register: "註冊",
            detail: "詳情",
            panel: "子面板",
        },
        quickSearch: "快速搜尋",
        quickSearchPlaceholder: "名稱或 endpoint",
        sortBy: "排序方式",
        noMatch: "目前篩選或搜尋下沒有匹配的 runtime。",
        columns: {
            name: "名稱",
            tags: "標籤",
            status: "狀態",
            capabilitySurface: "能力面",
            sidecar: "Sidecar",
            attention: "關注項",
            actions: "操作",
        },
        sort: {
            name: "名稱",
            status: "狀態來源",
            snapshot: "快照類型",
        },
        actions: {
            attention: "查看關注",
            status: "刷新狀態",
            all: "全部刷新",
        },
        states: {
            clear: "正常",
            none: "無",
            noEnv: "無環境",
            noCluster: "無叢集",
            noRole: "無角色",
            noCapabilities: "沒有 fully-supported 的能力",
        },
    },
    runtimeDetail: {
        title: "節點詳情",
        nothingSelected: "未選擇節點",
        empty: "從表格中選擇一個 runtime，就能查看它的 capability、status 和 attention 狀態。",
        identity: "身份資訊",
        status: "狀態",
        capabilities: "能力",
        attention: "關注項",
        refreshAll: "刷新該節點",
        refreshStatus: "刷新狀態",
        refreshCapabilities: "刷新能力",
        refreshSidecar: "刷新 Sidecar",
        copyLink: "複製節點連結",
        registered: "註冊時間",
        updated: "更新時間",
        source: "來源",
        sidecarSource: "sidecar 來源",
        sidecarLearning: "sidecar 學習態",
        resilienceStatus: "韌性狀態",
        resilienceSummary: "韌性摘要",
        socketServiceStatus: "socket 服務",
        idleTimeouts: "空閒逾時（目前 / 累計）",
        snapshotKind: "快照類型",
        targetCount: "target 數量",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        noCapabilities: "目前沒有記錄 capability。",
        clear: "正常",
        noAttention: "這條 runtime 目前沒有 attention reason。",
        needsAttention: "是否需要關注",
        sectionsLabel: "節點詳情分區",
        liveSummary: "即時節點態勢",
        operational: "控制平面已有最新快照，目前不需要操作。",
        requiresReview: "此節點需要操作者檢查。",
        refreshRecommended: "最近一次節點重新整理失敗，請先重新整理再依賴這份快照。",
        reviewAttention: "檢查關注項",
        inspectStatus: "查看狀態",
        lastObserved: "最近觀測",
        notObserved: "尚未觀測",
        runtimeId: "Runtime ID",
        supportedCapabilities: "已支援能力",
        fullySupportedCount: "{count} 項完全支援",
        availableEvidence: "可用證據",
        availableCount: "{count} 項可用",
        attentionReasonCount: "{count} 個活躍原因",
        statusOverview: "節點狀態",
        evidenceAvailability: "證據可用性",
        available: "可用",
        missing: "缺失",
        sidecarOverview: "Sidecar 狀態",
        capabilitySource: "能力來源",
        lastCapabilityRefresh: "最近能力重新整理",
        support: {
            fullySupported: "完全支援",
            partiallySupported: "部分支援",
            notSupported: "不支援",
            unsupported: "不支援",
            unknown: "未知",
        },
        none: "無",
        na: "暫無",
    },
    runtimePanel: {
        windows: {
            openSelected: "打開所選實例",
            openAll: "全部打開",
            closeAll: "全部關閉",
            close: "關閉",
            activate: "啟用",
            external: "新分頁",
            count: "{count} 個視窗",
            one: "1 個視窗",
            capacity: "{count} / {limit} 個視窗",
            policy: "只有目前面板保持連線，其他面板會暫停以保護資源。",
            pausedTitle: "面板已暫停",
            pausedBody: "啟用這個視窗後才會載入遠端面板；非活動視窗會保持暫停，避免佔滿瀏覽器和 runtime 資源。",
            pausedAction: "啟用面板",
            limitReached: "工作區已達到 {limit} 個視窗上限，請先關閉一個視窗。",
            openAllLimited: "已打開 {total} 個 runtime 中的 {count} 個；工作區上限為 {limit} 個視窗。",
            openAllComplete: "已準備好 {count} 個 runtime 視窗。",
            workspaceLabel: "Runtime 視窗工作區",
            windowLabel: "{name} · {view}",
        },
        title: "Runtime 子面板",
        notReady: "尚未選擇 runtime",
        empty: "選擇一個 runtime 後，就能載入它的子控制頁面。",
        compactTrustObserved: "runtime 即時快照已可用。",
        compactTrustIdleReady: "runtime 目前空閒，但健康可接入。",
        compactTrustUnobserved: "正在等待第一份 runtime 快照。",
        compactTrustFetchFailed: "最近一次 runtime 刷新失敗，請把目前資料視為可能過期。",
        compactTrustSidecarObserved: "sidecar 即時快照已可用。",
        compactTrustSidecarUnobserved: "正在等待第一份 sidecar 快照。",
        compactTrustSidecarFetchFailed: "最近一次 sidecar 刷新失敗，請把目前資料視為可能過期。",
        compactTrustNoSidecar: "目前沒有配置 paired sidecar。",
        blankRuntimeTitle: "這個 runtime 面板還沒準備好",
        blankRuntimeBody: "我們已經知道這台 runtime 在哪，但它還沒有發布適合直接展示的面板資料。現在先以上面的控制平面摘要為準更合適。",
        blankSidecarTitle: "這個 sidecar 面板還沒準備好",
        blankSidecarBody: "配對的診斷 sidecar 已經配置好了，但還沒有產出可穩定展示的狀態快照。現在先看控制平面視圖，再按需刷新。",
        blankFetchFailedTitle: "這個面板暫時不適合直接打開",
        blankFetchFailedBody: "最近一次狀態刷新失敗了，這時直接展示原始 endpoint 往往比幫助更容易誤導。先刷新，再回來查看會更穩。",
        blankHintRefreshRuntime: "先刷新一下 runtime 狀態。",
        blankHintRefreshSidecar: "先刷新一下 sidecar 狀態。",
        blankHintEndpoint: "endpoint",
        breadcrumbFleet: "runtime 面板",
        breadcrumbSource: "來源",
        breadcrumbView: "目前視圖",
        currentView: "目前視圖",
        sourceUrl: "來源位址",
        sourceStatus: "來源狀態",
        trustTitle: "可信度提示",
        trustObserved: "已觀測到 runtime 狀態",
        trustObservedMessage: "這塊子面板背後已經有可用的 gewyvern latest snapshot，一般可以把它當成即時 runtime 上下文來讀。",
        trustIdleReady: "runtime 空閒就緒",
        trustIdleReadyMessage: "這條 runtime 目前處於空閒但健康的狀態。它還沒有產出新的 payload 快照，但 socket 服務仍然活著，下一次連線隨時可以接進來。",
        trustUnobserved: "runtime 尚未觀測",
        trustUnobservedMessage: "這條 runtime 還沒有發布 latest snapshot，所以在狀態到達前，更適合把子面板理解成一個薄的 endpoint 殼。",
        trustFetchFailed: "狀態抓取失敗",
        trustFetchFailedMessage: "控制面這次沒能刷新這條 runtime 的狀態，所以右側子面板目前可能是陳舊的，或者已經不可達。",
        trustSidecarObserved: "Sidecar 狀態已觀測",
        trustSidecarObservedMessage: "這塊子面板背後已經能連到 paired etragon sidecar，可以把它當成鄰近診斷搭檔的即時上下文來讀。",
        trustSidecarUnobserved: "Sidecar 尚未觀測",
        trustSidecarUnobservedMessage: "這條 runtime 已經配置了 paired etragon endpoint，但控制面還沒有拿到 sidecar 狀態快照。",
        trustSidecarFetchFailed: "Sidecar 狀態抓取失敗",
        trustSidecarFetchFailedMessage: "控制面這次沒能刷新 paired etragon sidecar，所以右側 sidecar 子面板目前可能是陳舊的，或者已經不可達。",
        trustNoSidecar: "沒有 paired sidecar",
        trustNoSidecarMessage: "這條 runtime 目前還沒有配置 etragon sidecar endpoint。",
        trustMeta: "狀態來源：{source} · 快照：{snapshot}",
        trustRefreshStatus: "立即刷新狀態",
        trustRefreshSidecar: "立即刷新 Sidecar",
        openExternal: "新分頁打開",
        sources: {
            runtime: "Runtime",
            sidecar: "Sidecar",
        },
        views: {
            root: "主頁",
            health: "健康",
            meta: "最新 Meta",
            summary: "摘要",
            analysis: "分析",
            targets: "Targets",
            sidecarRoot: "Sidecar 主頁",
            sidecarHealth: "Sidecar 健康",
            sidecarStatus: "Sidecar 狀態",
            sidecarEnrichment: "補強",
            sidecarOpinion: "診斷意見",
        },
    },
    register: {
        title: "註冊 Runtime",
        intake: "輕量 intake",
        targetSection: "Runtime 目標",
        targetSectionCopy: "指定由目前控制面管理的 gewyvern 服務。",
        accessSection: "控制存取",
        accessSectionCopy: "使用短期令牌完成配對；令牌絕不會出現在預覽中。",
        sidecarSection: "選用 Sidecar",
        sidecarSectionCopy: "僅在 runtime 提供配套服務時加入。",
        placementSection: "歸屬與探索",
        placementSectionCopy: "選用標籤便於探索 runtime，不會改變其身分。",
        showToken: "顯示",
        hideToken: "隱藏",
        completeField: "請填寫{field}後繼續。",
        checkingPlan: "正在透過控制面驗證註冊計畫...",
        planUnavailable: "無法驗證註冊計畫：{message}",
        ready: "註冊計畫已通過驗證，可以註冊此 runtime。",
        fixHighlighted: "註冊前請檢查醒目提示的欄位。",
        name: "名稱",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "Endpoint",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "Sidecar Endpoint",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        pairingToken: "配對令牌",
        tokenPlaceholder: "token-123",
        fetchCapabilities: "註冊時從 gewyvern 拉取 capability 和 latest-meta",
        submit: "註冊 Runtime",
        clear: "清空表單",
        untouched: "還沒有提交 runtime。",
        previewTitle: "即時預覽",
        previewName: "名稱",
        previewSlice: "歸屬切片",
        previewEndpoint: "endpoint",
        previewSidecar: "sidecar",
        previewCapabilityFetch: "能力抓取",
        suggested: "建議",
        pendingRuntimeName: "等待產生 runtime 名稱",
        endpointPending: "待輸入",
        endpointValid: "有效",
        endpointInvalid: "無效",
        sidecarUnpaired: "未配對",
        capabilityEnabled: "開啟",
        capabilityDisabled: "關閉",
        blockedEndpoint: "註冊已攔截：endpoint 必須以 http:// 或 https:// 開頭，並且是合法 URL。",
        blockedSidecarEndpoint: "註冊已攔截：sidecar endpoint 必須以 http:// 或 https:// 開頭，並且是合法 URL。",
        blockedDuplicate: "註冊已攔截：{reason} 已存在於 {name} ({endpoint})。",
        deletionInProgress: "此 runtime 正在刪除，註冊已暫停。請等待清理完成後重新檢查計畫。",
        duplicateNameAndEndpoint: "名稱和 endpoint",
        duplicateName: "名稱",
        duplicateEndpoint: "endpoint",
        registering: "正在註冊 runtime...",
        registered: "已將 {name} ({runtimeId}) 註冊到 {slice}，目前狀態為 {status}。",
        failed: "註冊失敗：{message}。如果這條 runtime 已經存在，建議直接在表格裡選中它。",
        allRuntimes: "全部 runtimes",
    },
    persistence: {
        title: "持久化",
        chip: "控制面狀態",
        yes: "是",
        no: "否",
        saveNow: "立即保存",
        exportState: "匯出狀態",
        importState: "匯入狀態",
        stateFile: "狀態檔案",
        lastSaved: "最近保存",
        enabled: "已啟用",
        schema: "schema",
        state: "狀態",
        restoredRuntimes: "恢復 runtimes",
        restoredSessions: "恢復 sessions",
        statePath: "狀態檔案路徑",
        backupPath: "備份路徑",
        schemaVersion: "schema 版本",
        lastSavedAt: "最近保存時間",
        lastSaveError: "最近保存錯誤",
        restoredFromSave: "恢復來源",
        configured: "已配置",
        missing: "缺失",
        never: "從未",
        unknown: "未知",
        none: "無",
        clean: "已同步",
        dirty: "有變更",
        saving: "正在保存控制面狀態...",
        saved: "控制面狀態已保存。",
        saveFailed: "保存失敗：{message}",
        exporting: "正在匯出控制面狀態...",
        exported: "控制面狀態已匯出。",
        exportFailed: "匯出失敗：{message}",
        importing: "正在從 {file} 匯入控制面狀態...",
        invalidJson: "所選檔案不是合法 JSON",
        imported: "已匯入 {runtimes} 個 runtime 和 {sessions} 個 session。",
        importFailed: "匯入失敗：{message}",
    },
    attention: {
        noReasons: "目前沒有 active attention reasons。",
        reasonLine: "{reason} · {count} 個 runtime",
        noRuntimes: "目前切片裡沒有需要關注的 runtime。",
        critical: "嚴重",
        warning: "警告",
        statusFetchFailed: "狀態抓取失敗",
        sidecarStatusFetchFailed: "sidecar 狀態抓取失敗",
        noLatestSnapshot: "缺少最新快照",
        noAnalysisJson: "缺少分析 JSON",
    },
    sessions: {
        title: "工作階段",
        none: "目前還沒有 session。",
        runtime: "runtime",
    },
    metrics: {
        runtimes: "runtimes",
        latestSnapshots: "latest snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "sidecar context",
        diagnosticOpinions: "diagnostic opinions",
        pairedSidecars: "paired sidecars",
        healthySidecars: "healthy sidecars",
        critical: "嚴重",
        warning: "警告",
    },
    groups: {
        snapshotKinds: "快照類型",
        statusSources: "狀態來源",
        sidecarStatusSources: "sidecar 狀態來源",
        environments: "環境",
        clusters: "叢集",
        roles: "角色",
        empty: "目前還沒有分組資料。",
    },
    statuses: {
        fetchFailed: "抓取失敗",
        idleReady: "空閒就緒",
        unobserved: "未觀測",
        observedSnapshot: "{kind} 快照",
        observed: "已觀測",
        sidecarObserved: "sidecar 已觀測",
        sidecarStarting: "sidecar 啟動中",
        sidecarDegraded: "sidecar 已降級",
        sidecarFetchFailed: "sidecar 抓取失敗",
    },
    notifications: {
        loading: "正在載入控制面狀態...",
        loaded: "已載入 {count} 個 runtime。",
        dashboardLoadFailed: "面板載入失敗：{message}",
        noRuntimeSelected: "目前沒有選中的 runtime。",
        runtimeLinkCopied: "已複製 runtime 連結。",
        runtimeLinkFailed: "複製連結失敗：{message}",
        runtimeRefreshAll: "刷新這個節點的全部資訊",
        runtimeRefreshStatus: "刷新這個節點狀態",
        runtimeRefreshCapabilities: "刷新這個節點能力",
        runtimeRefreshComplete: "{label} 完成。",
        runtimeRefreshFailed: "{label} 失敗：{message}",
        fleetRefreshAll: "整組全部刷新",
        fleetStatusRefresh: "整組狀態刷新",
        fleetCapabilityRefresh: "整組能力刷新",
        fleetRefreshComplete: "{label} 完成。",
        fleetRefreshFailed: "{label} 失敗：{message}",
        badgeUpdated: "已更新",
    },
});
translations.de = mergeTranslations(translations.en, {
    hero: {
        title: "Control Plane Dashboard",
        subcopy: "Eine leichtgewichtige Fleet-Ansicht für viele nahe gewyvern-Runtimes.",
    },
    language: {
        label: "Sprache",
        auto: "Browser folgen",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    theme: {
        label: "Design",
        auto: "System folgen",
        light: "Tag",
        dark: "Nacht",
    },
    security: {
        title: "Sicherheit",
        adminToken: "Admin-Token",
        adminTokenPlaceholder: "optional für Remote-Zugriff",
        clearToken: "Token löschen",
        showToken: "Token anzeigen",
        hideToken: "Token verbergen",
        testToken: "Token testen",
        localModeHint: "Lokal ist der Loopback-Modus aktiv. Trage ein Token nur ein, wenn du absichtlich über einen geschützten Remote-Endpunkt arbeitest.",
        tokenStored: "Admin-Token in diesem Browser gespeichert.",
        tokenCleared: "Admin-Token gelöscht.",
        tokenRequired: "Diese Anfrage wurde vom Control Plane abgelehnt. Wenn du remote verbunden bist, trage das konfigurierte Admin-Token ein.",
        tokenTestRunning: "Admin-Token wird getestet...",
        tokenTestOk: "Das Admin-Token wurde vom aktuellen Control Plane akzeptiert.",
        tokenTestFailed: "Admin-Token-Test fehlgeschlagen: {message}",
        tokenMissing: "Bitte zuerst ein Admin-Token eintragen und dann testen.",
        lastTokenTest: "Letzter Token-Test",
        neverTested: "nie getestet",
        testStateOk: "ok",
        testStateFailed: "fehlgeschlagen",
        testStateRunning: "läuft",
        mode: "Sicherheitsmodus",
        tokenConfigured: "Admin-Token",
        publicDiscovery: "öffentliche Discovery",
        configured: "konfiguriert",
        notConfigured: "nicht konfiguriert",
        enabled: "aktiviert",
        disabled: "deaktiviert",
        loopbackOnly: "nur Loopback",
        tokenMode: "Loopback oder Token",
    },
    actions: {
        refreshAll: "Fleet vollständig aktualisieren",
        refreshStatus: "Status aktualisieren",
        refreshCapabilities: "Fähigkeiten aktualisieren",
    },
    filters: {
        title: "Flottenfilter",
        environment: "Umgebung",
        environmentPlaceholder: "prod",
        cluster: "Cluster",
        clusterPlaceholder: "alpha",
        role: "Rolle",
        rolePlaceholder: "edge",
        apply: "Filter anwenden",
        clear: "Zurücksetzen",
        allRuntimes: "alle Runtimes",
    },
    tabs: {
        overview: "Überblick",
        runtimes: "Runtimes",
        register: "Registrieren",
        persistence: "Persistenz",
        sessions: "Sitzungen",
    },
    overview: {
        fleetSummary: "Fleet-Zusammenfassung",
        attentionSummary: "Aufmerksamkeitsübersicht",
        triage: "Triage",
        runtimesNeedingAttention: "Runtimes mit Handlungsbedarf",
        kicker: "Fleet-Lage",
        spotlightTitle: "Erst die Gesamtlage sehen, dann in einzelne Runtimes einsteigen.",
        spotlightBody: "Mit dieser Shell wechselst du zwischen Fleet-Lage, Runtime-Details, Registrierung, Persistenz und Sitzungen, ohne in einer langen Scroll-Seite zu landen.",
        spotlightRail1: "aktueller Slice",
        spotlightRail2: "aktueller Modus",
        summaryChip: "Live-Zähler",
    },
    runtimes: {
        title: "Runtimes",
        workspaceTabs: { select: "Auswahl", register: "Registrieren", detail: "Detail", panel: "Kind-Panel" },
        quickSearch: "Schnellsuche",
        quickSearchPlaceholder: "Name oder Endpunkt",
        sortBy: "Sortieren nach",
        noMatch: "Keine Runtime passt zu Filter oder Suche.",
        columns: { name: "Name", tags: "Tags", status: "Status", capabilitySurface: "Fähigkeitsfläche", sidecar: "Sidecar", attention: "Hinweise", actions: "Aktionen" },
        sort: { name: "Name", status: "Statusquelle", snapshot: "Snapshot-Typ" },
        actions: { attention: "Hinweise", status: "Status", all: "Alles" },
        states: { clear: "ok", none: "keine", noEnv: "keine-umgebung", noCluster: "kein-cluster", noRole: "keine-rolle", noCapabilities: "Keine vollständig unterstützten Fähigkeiten" },
    },
    runtimeDetail: {
        title: "Runtime-Details",
        nothingSelected: "nichts ausgewählt",
        empty: "Wähle eine Runtime aus der Tabelle, um Fähigkeit, Status und Hinweise zu sehen.",
        identity: "Identität",
        status: "Status",
        capabilities: "Fähigkeiten",
        attention: "Hinweise",
        refreshAll: "Diese Runtime aktualisieren",
        refreshStatus: "Status aktualisieren",
        refreshCapabilities: "Fähigkeiten aktualisieren",
        refreshSidecar: "Sidecar aktualisieren",
        copyLink: "Runtime-Link kopieren",
        registered: "registriert",
        updated: "aktualisiert",
        source: "Quelle",
        sidecarSource: "Sidecar-Quelle",
        sidecarLearning: "Sidecar-Lernstatus",
        resilienceStatus: "Resilienzstatus",
        resilienceSummary: "Resilienzzusammenfassung",
        socketServiceStatus: "Socket-Dienst",
        idleTimeouts: "Leerlauf-Timeouts (aktuell / gesamt)",
        snapshotKind: "Snapshot-Typ",
        targetCount: "Target-Anzahl",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        noCapabilities: "Keine Fähigkeiten aufgezeichnet.",
        clear: "ok",
        noAttention: "Für diese Runtime gibt es aktuell keine Attention-Reasons.",
        needsAttention: "braucht Aufmerksamkeit",
        sectionsLabel: "Bereiche der Runtime-Details",
        liveSummary: "Aktueller Runtime-Zustand",
        operational: "Der neueste Control-Plane-Snapshot ist verfügbar; derzeit ist keine Aktion erforderlich.",
        requiresReview: "Diese Runtime muss geprüft werden.",
        refreshRecommended: "Die letzte Aktualisierung ist fehlgeschlagen. Vor Verwendung dieses Snapshots erneut aktualisieren.",
        reviewAttention: "Hinweise prüfen",
        inspectStatus: "Status ansehen",
        lastObserved: "Zuletzt beobachtet",
        notObserved: "Nicht beobachtet",
        runtimeId: "Runtime-ID",
        supportedCapabilities: "Unterstützte Fähigkeiten",
        fullySupportedCount: "{count} vollständig unterstützt",
        availableEvidence: "Verfügbare Evidenz",
        availableCount: "{count} verfügbar",
        attentionReasonCount: "{count} aktive Gründe",
        statusOverview: "Runtime-Status",
        evidenceAvailability: "Evidenzverfügbarkeit",
        available: "Verfügbar",
        missing: "Fehlt",
        sidecarOverview: "Sidecar-Status",
        capabilitySource: "Fähigkeitsquelle",
        lastCapabilityRefresh: "Letzte Fähigkeitsaktualisierung",
        support: {
            fullySupported: "Vollständig unterstützt",
            partiallySupported: "Teilweise unterstützt",
            notSupported: "Nicht unterstützt",
            unsupported: "Nicht unterstützt",
            unknown: "Unbekannt",
        },
        none: "keine",
        na: "k. A.",
    },
    runtimePanel: {
        windows: {
            openSelected: "Ausgewählte öffnen",
            openAll: "Alle öffnen",
            closeAll: "Alle schließen",
            close: "Schließen",
            activate: "Aktivieren",
            external: "Neuer Tab",
            count: "{count} Fenster",
            one: "1 Fenster",
            capacity: "{count} / {limit} Fenster",
            policy: "Nur das aktive Panel ist live; inaktive Panels sind pausiert.",
            pausedTitle: "Panel pausiert",
            pausedBody: "Aktiviere dieses Fenster, um sein Remote-Panel zu laden. Inaktive Fenster bleiben zum Schutz der Browser- und Runtime-Ressourcen pausiert.",
            pausedAction: "Panel aktivieren",
            limitReached: "Das Arbeitsfenster hat sein Limit von {limit} Fenstern erreicht. Schließe zuerst ein Fenster.",
            openAllLimited: "{count} von {total} Runtimes geöffnet. Das Arbeitsfenster ist auf {limit} Fenster begrenzt.",
            openAllComplete: "{count} Runtime-Fenster vorbereitet.",
            workspaceLabel: "Runtime-Fensterarbeitsbereich",
            windowLabel: "{name} · {view}",
        },
        title: "Runtime-Kind-Panel",
        notReady: "keine Runtime ausgewählt",
        empty: "Wähle eine Runtime aus, um ihr Kind-Panel zu laden.",
        compactTrustObserved: "Live-Runtime-Snapshot verfügbar.",
        compactTrustIdleReady: "Runtime ist im Leerlauf, aber gesund und bereit.",
        compactTrustUnobserved: "Warte auf den ersten Runtime-Snapshot.",
        compactTrustFetchFailed: "Die letzte Runtime-Aktualisierung ist fehlgeschlagen; diese Daten können veraltet sein.",
        compactTrustSidecarObserved: "Live-Sidecar-Snapshot verfügbar.",
        compactTrustSidecarUnobserved: "Warte auf den ersten Sidecar-Snapshot.",
        compactTrustSidecarFetchFailed: "Die letzte Sidecar-Aktualisierung ist fehlgeschlagen; diese Daten können veraltet sein.",
        compactTrustNoSidecar: "Kein gekoppelter Sidecar konfiguriert.",
        blankRuntimeTitle: "Dieses Runtime-Panel ist noch nicht bereit",
        blankRuntimeBody: "Wir wissen, wo diese Runtime lebt, aber sie hat noch keine paneltauglichen Daten veröffentlicht. Nutze vorerst die Control-Plane-Zusammenfassung oben.",
        blankSidecarTitle: "Dieses Sidecar-Panel ist noch nicht bereit",
        blankSidecarBody: "Der gekoppelte diagnostische Sidecar ist konfiguriert, hat aber noch keinen paneltauglichen Snapshot geliefert. Starte mit der Control-Plane-Ansicht und aktualisiere dann.",
        blankFetchFailedTitle: "Dieses Panel kann gerade nicht sicher geöffnet werden",
        blankFetchFailedBody: "Die letzte Statusaktualisierung ist fehlgeschlagen. Den rohen Endpunkt jetzt direkt zu zeigen wäre eher verwirrend als hilfreich. Erst aktualisieren, dann erneut versuchen.",
        blankHintRefreshRuntime: "Versuche zuerst, den Runtime-Status zu aktualisieren.",
        blankHintRefreshSidecar: "Versuche zuerst, den Sidecar-Status zu aktualisieren.",
        blankHintEndpoint: "Endpunkt",
        breadcrumbFleet: "Runtime-Panel",
        breadcrumbSource: "Quelle",
        breadcrumbView: "Ansicht",
        currentView: "aktuelle Ansicht",
        sourceUrl: "Quell-URL",
        sourceStatus: "Quellstatus",
        trustTitle: "Vertrauen",
        trustObserved: "Runtime-Status beobachtet",
        trustObservedMessage: "Dieses Kind-Panel wird von einem beobachteten gewyvern-Snapshot getragen und kann normalerweise als Live-Kontext gelesen werden.",
        trustIdleReady: "Leerlaufbereite Runtime",
        trustIdleReadyMessage: "Diese Runtime ist derzeit im Leerlauf, aber gesund. Sie hat noch keinen frischen Payload-Snapshot erzeugt, aber der Socket-Dienst lebt und ist für die nächste Verbindung bereit.",
        trustUnobserved: "Runtime noch nicht beobachtet",
        trustUnobservedMessage: "Diese Runtime hat noch keinen Latest-Snapshot veröffentlicht. Bis dahin ist das Kind-Panel eher eine dünne Endpunkt-Hülle.",
        trustFetchFailed: "Statusabruf fehlgeschlagen",
        trustFetchFailedMessage: "Die Control Plane konnte den Status dieser Runtime nicht aktualisieren. Das Kind-Panel kann also veraltet oder unerreichbar sein.",
        trustSidecarObserved: "Sidecar-Status beobachtet",
        trustSidecarObservedMessage: "Dieses Kind-Panel wird von einem erreichbaren etragon-Sidecar gestützt und kann als naher Diagnosekontext gelesen werden.",
        trustSidecarUnobserved: "Sidecar noch nicht beobachtet",
        trustSidecarUnobservedMessage: "Ein gekoppelter etragon-Endpunkt ist konfiguriert, aber die Control Plane hat noch keinen Sidecar-Status-Snapshot gesehen.",
        trustSidecarFetchFailed: "Sidecar-Abruf fehlgeschlagen",
        trustSidecarFetchFailedMessage: "Die Control Plane konnte den gekoppelten etragon-Sidecar nicht aktualisieren. Das Sidecar-Panel kann veraltet oder unerreichbar sein.",
        trustNoSidecar: "Kein gekoppelter Sidecar",
        trustNoSidecarMessage: "Diese Runtime meldet derzeit keinen gekoppelten etragon-Endpunkt.",
        trustMeta: "Statusquelle: {source} · Snapshot: {snapshot}",
        trustRefreshStatus: "Status jetzt aktualisieren",
        trustRefreshSidecar: "Sidecar jetzt aktualisieren",
        openExternal: "In neuem Tab öffnen",
        sources: { runtime: "Runtime", sidecar: "Sidecar" },
        views: { root: "Start", health: "Health", meta: "Latest Meta", summary: "Zusammenfassung", analysis: "Analyse", targets: "Targets", sidecarRoot: "Sidecar Start", sidecarHealth: "Sidecar Health", sidecarStatus: "Sidecar-Status", sidecarEnrichment: "Anreicherung", sidecarOpinion: "Meinung" },
    },
    register: {
        title: "Runtime registrieren",
        intake: "sehr leichter Intake",
        targetSection: "Runtime-Ziel",
        targetSectionCopy: "Bestimme den gewyvern-Dienst, den diese Control Plane verwaltet.",
        accessSection: "Steuerungszugriff",
        accessSectionCopy: "Kopplung mit einem kurzlebigen Token; es erscheint nie in der Vorschau.",
        sidecarSection: "Optionaler Sidecar",
        sidecarSectionCopy: "Nur hinzufügen, wenn die Runtime einen Begleitdienst bereitstellt.",
        placementSection: "Zuordnung und Erkennung",
        placementSectionCopy: "Optionale Tags erleichtern die Erkennung, ohne die Identität zu ändern.",
        showToken: "Anzeigen",
        hideToken: "Ausblenden",
        completeField: "Fülle {field} aus, um fortzufahren.",
        checkingPlan: "Registrierungsplan wird mit der Control Plane geprüft...",
        planUnavailable: "Der Registrierungsplan konnte nicht geprüft werden: {message}",
        ready: "Registrierungsplan bestätigt. Die Runtime kann registriert werden.",
        fixHighlighted: "Prüfe vor der Registrierung das hervorgehobene Feld.",
        name: "Name",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "Endpunkt",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "Sidecar-Endpunkt",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        pairingToken: "Kopplungstoken",
        tokenPlaceholder: "token-123",
        fetchCapabilities: "Beim Registrieren Capability und Latest-Meta von gewyvern abrufen",
        submit: "Runtime registrieren",
        clear: "Formular leeren",
        untouched: "Noch keine Runtime eingereicht.",
        previewTitle: "Live-Vorschau",
        previewName: "Name",
        previewSlice: "Slice",
        previewEndpoint: "Endpunkt",
        previewSidecar: "Sidecar",
        previewCapabilityFetch: "Capability-Abruf",
        suggested: "vorgeschlagen",
        pendingRuntimeName: "ausstehender Runtime-Name",
        endpointPending: "ausstehend",
        endpointValid: "gültig",
        endpointInvalid: "ungültig",
        sidecarUnpaired: "nicht gekoppelt",
        capabilityEnabled: "aktiviert",
        capabilityDisabled: "deaktiviert",
        blockedEndpoint: "Registrierung blockiert: Der Endpunkt muss mit http:// oder https:// beginnen und eine gültige URL sein.",
        blockedSidecarEndpoint: "Registrierung blockiert: Der Sidecar-Endpunkt muss mit http:// oder https:// beginnen und eine gültige URL sein.",
        blockedDuplicate: "Registrierung blockiert: {reason} existiert bereits bei {name} ({endpoint}).",
        deletionInProgress: "Die Registrierung ist pausiert, während diese Runtime gelöscht wird. Warten Sie auf den Abschluss der Bereinigung und prüfen Sie den Plan erneut.",
        duplicateNameAndEndpoint: "Name und Endpunkt",
        duplicateName: "Name",
        duplicateEndpoint: "Endpunkt",
        registering: "Runtime wird registriert...",
        registered: "{name} ({runtimeId}) wurde in {slice} mit Status {status} registriert.",
        failed: "Registrierung fehlgeschlagen: {message}. Wenn diese Runtime schon existiert, wähle sie direkt aus der Tabelle.",
        allRuntimes: "alle Runtimes",
    },
    persistence: {
        title: "Persistenz",
        chip: "Control-Plane-Status",
        yes: "ja",
        no: "nein",
        saveNow: "Jetzt speichern",
        exportState: "Status exportieren",
        importState: "Status importieren",
        stateFile: "Statusdatei",
        lastSaved: "zuletzt gespeichert",
        enabled: "aktiviert",
        schema: "Schema",
        state: "Status",
        restoredRuntimes: "wiederhergestellte Runtimes",
        restoredSessions: "wiederhergestellte Sitzungen",
        statePath: "Pfad der Statusdatei",
        backupPath: "Pfad der Sicherung",
        schemaVersion: "Schema-Version",
        lastSavedAt: "zuletzt gespeichert um",
        lastSaveError: "letzter Speicherfehler",
        restoredFromSave: "wiederhergestellt aus",
        configured: "konfiguriert",
        missing: "fehlt",
        never: "nie",
        unknown: "unbekannt",
        none: "keine",
        clean: "sauber",
        dirty: "mit Änderungen",
        saving: "Control-Plane-Status wird gespeichert...",
        saved: "Control-Plane-Status gespeichert.",
        saveFailed: "Speichern fehlgeschlagen: {message}",
        exporting: "Control-Plane-Status wird exportiert...",
        exported: "Control-Plane-Status exportiert.",
        exportFailed: "Export fehlgeschlagen: {message}",
        importing: "Control-Plane-Status aus {file} wird importiert...",
        invalidJson: "Die ausgewählte Datei ist kein gültiges JSON",
        imported: "{runtimes} Runtimes und {sessions} Sitzungen importiert.",
        importFailed: "Import fehlgeschlagen: {message}",
    },
    attention: {
        noReasons: "Aktuell keine aktiven Attention-Reasons.",
        reasonLine: "{reason} · {count} Runtimes",
        noRuntimes: "In diesem Slice brauchen aktuell keine Runtimes Aufmerksamkeit.",
        critical: "kritisch",
        warning: "Warnung",
        statusFetchFailed: "status_fetch_failed",
        sidecarStatusFetchFailed: "sidecar_status_fetch_failed",
        noLatestSnapshot: "kein aktueller Snapshot",
        noAnalysisJson: "kein Analyse-JSON",
    },
    sessions: { title: "Sitzungen", none: "Noch keine Sitzungen.", runtime: "Runtime" },
    metrics: {
        runtimes: "Runtimes",
        latestSnapshots: "Latest Snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "Sidecar-Kontext",
        diagnosticOpinions: "Diagnosemeinungen",
        pairedSidecars: "gekoppelte Sidecars",
        healthySidecars: "gesunde Sidecars",
        critical: "kritisch",
        warning: "Warnung",
    },
    groups: {
        snapshotKinds: "Snapshot-Typen",
        statusSources: "Statusquellen",
        sidecarStatusSources: "Sidecar-Statusquellen",
        environments: "Umgebungen",
        clusters: "Cluster",
        roles: "Rollen",
        empty: "Noch keine Gruppierungsdaten.",
    },
    statuses: {
        fetchFailed: "Abruf fehlgeschlagen",
        idleReady: "leerlaufbereit",
        unobserved: "unbeobachtet",
        observedSnapshot: "{kind}-Snapshot",
        observed: "beobachtet",
        sidecarObserved: "Sidecar beobachtet",
        sidecarStarting: "Sidecar startet",
        sidecarDegraded: "Sidecar degradiert",
        sidecarFetchFailed: "Sidecar-Abruf fehlgeschlagen",
    },
    notifications: {
        loading: "Control-Plane-Status wird geladen...",
        loaded: "{count} Runtimes geladen.",
        dashboardLoadFailed: "Dashboard-Laden fehlgeschlagen: {message}",
        noRuntimeSelected: "Keine Runtime ausgewählt.",
        runtimeLinkCopied: "Runtime-Link kopiert.",
        runtimeLinkFailed: "Link kopieren fehlgeschlagen: {message}",
        runtimeRefreshAll: "Runtime vollständig aktualisieren",
        runtimeRefreshStatus: "Runtime-Status aktualisieren",
        runtimeRefreshCapabilities: "Runtime-Fähigkeiten aktualisieren",
        runtimeRefreshComplete: "{label} abgeschlossen.",
        runtimeRefreshFailed: "{label} fehlgeschlagen: {message}",
        fleetRefreshAll: "Fleet vollständig aktualisieren",
        fleetStatusRefresh: "Fleet-Status aktualisieren",
        fleetCapabilityRefresh: "Fleet-Fähigkeiten aktualisieren",
        fleetRefreshComplete: "{label} abgeschlossen.",
        fleetRefreshFailed: "{label} fehlgeschlagen: {message}",
        badgeUpdated: "aktualisiert",
    },
});
translations.fr = mergeTranslations(translations.en, {
    hero: {
        title: "Tableau de bord du plan de contrôle",
        subcopy: "Une vue légère du fleet pour de nombreux runtimes gewyvern proches.",
    },
    language: {
        label: "Langue",
        auto: "Suivre le navigateur",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    theme: {
        label: "Thème",
        auto: "Suivre le système",
        light: "Jour",
        dark: "Nuit",
    },
    security: {
        title: "Sécurité",
        adminToken: "Jeton administrateur",
        adminTokenPlaceholder: "optionnel pour un accès distant",
        clearToken: "Effacer le jeton",
        showToken: "Afficher le jeton",
        hideToken: "Masquer le jeton",
        testToken: "Tester le jeton",
        localModeHint: "Le mode loopback local est actif. Ajoute un jeton seulement si tu te connectes volontairement via un endpoint distant protégé.",
        tokenStored: "Jeton administrateur enregistré dans ce navigateur.",
        tokenCleared: "Jeton administrateur effacé.",
        tokenRequired: "Cette requête a été refusée par le control plane. Si tu es connecté à distance, saisis le jeton administrateur configuré.",
        tokenTestRunning: "Test du jeton administrateur...",
        tokenTestOk: "Ce jeton administrateur a été accepté par le control plane actuel.",
        tokenTestFailed: "Échec du test du jeton administrateur : {message}",
        tokenMissing: "Ajoute d'abord un jeton administrateur, puis teste-le.",
        lastTokenTest: "Dernier test du jeton",
        neverTested: "jamais testé",
        testStateOk: "ok",
        testStateFailed: "échec",
        testStateRunning: "test en cours",
        mode: "mode de sécurité",
        tokenConfigured: "jeton admin",
        publicDiscovery: "découverte publique",
        configured: "configuré",
        notConfigured: "non configuré",
        enabled: "activé",
        disabled: "désactivé",
        loopbackOnly: "loopback uniquement",
        tokenMode: "loopback ou jeton",
    },
    actions: {
        refreshAll: "Rafraîchir tout le fleet",
        refreshStatus: "Rafraîchir l'état",
        refreshCapabilities: "Rafraîchir les capacités",
    },
    filters: {
        title: "Filtres de flotte",
        environment: "Environnement",
        environmentPlaceholder: "prod",
        cluster: "Cluster",
        clusterPlaceholder: "alpha",
        role: "Rôle",
        rolePlaceholder: "edge",
        apply: "Appliquer les filtres",
        clear: "Effacer",
        allRuntimes: "tous les runtimes",
    },
    tabs: {
        overview: "Vue d'ensemble",
        runtimes: "Runtimes",
        register: "Enregistrement",
        persistence: "Persistance",
        sessions: "Sessions",
    },
    overview: {
        fleetSummary: "Résumé du fleet",
        attentionSummary: "Résumé d'attention",
        triage: "triage",
        runtimesNeedingAttention: "Runtimes nécessitant une attention",
        kicker: "Posture du fleet",
        spotlightTitle: "Commence par voir l'ensemble avant d'entrer dans chaque runtime.",
        spotlightBody: "Ce shell te laisse passer entre la posture du fleet, le détail du runtime, l'intake d'enregistrement, la persistance et les sessions sans retomber dans une page interminable.",
        spotlightRail1: "slice actuel",
        spotlightRail2: "mode actuel",
        summaryChip: "compteurs en direct",
    },
    runtimes: {
        title: "Runtimes",
        workspaceTabs: { select: "Sélection", register: "Enregistrer", detail: "Détail", panel: "Panneau enfant" },
        quickSearch: "Recherche rapide",
        quickSearchPlaceholder: "nom ou endpoint",
        sortBy: "Trier par",
        noMatch: "Aucun runtime ne correspond au filtre ou à la recherche actuelle.",
        columns: { name: "Nom", tags: "Tags", status: "État", capabilitySurface: "Surface de capacité", sidecar: "Sidecar", attention: "Attention", actions: "Actions" },
        sort: { name: "Nom", status: "Source d'état", snapshot: "Type de snapshot" },
        actions: { attention: "Attention", status: "État", all: "Tout" },
        states: { clear: "ok", none: "aucun", noEnv: "sans-env", noCluster: "sans-cluster", noRole: "sans-rôle", noCapabilities: "Aucune capacité fully-supported" },
    },
    runtimeDetail: {
        title: "Détail du runtime",
        nothingSelected: "rien de sélectionné",
        empty: "Sélectionne un runtime dans le tableau pour inspecter sa capacité, son état et ses alertes.",
        identity: "Identité",
        status: "État",
        capabilities: "Capacités",
        attention: "Attention",
        refreshAll: "Rafraîchir ce runtime",
        refreshStatus: "Rafraîchir l'état",
        refreshCapabilities: "Rafraîchir les capacités",
        refreshSidecar: "Rafraîchir le sidecar",
        copyLink: "Copier le lien",
        registered: "enregistré",
        updated: "mis à jour",
        source: "source",
        sidecarSource: "source du sidecar",
        sidecarLearning: "apprentissage du sidecar",
        resilienceStatus: "état de résilience",
        resilienceSummary: "résumé de résilience",
        socketServiceStatus: "service socket",
        idleTimeouts: "timeouts d'inactivité (courant / total)",
        snapshotKind: "type de snapshot",
        targetCount: "nombre de targets",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        noCapabilities: "Aucune capacité enregistrée.",
        clear: "ok",
        noAttention: "Ce runtime n'a actuellement aucune attention reason.",
        needsAttention: "requiert une attention",
        sectionsLabel: "Sections de détail du runtime",
        liveSummary: "État actuel du runtime",
        operational: "Le dernier instantané du plan de contrôle est disponible et aucune action n'est requise.",
        requiresReview: "Ce runtime nécessite une vérification.",
        refreshRecommended: "La dernière actualisation a échoué. Actualisez avant de vous fier à cet instantané.",
        reviewAttention: "Examiner les alertes",
        inspectStatus: "Voir le statut",
        lastObserved: "Dernière observation",
        notObserved: "Non observé",
        runtimeId: "ID du runtime",
        supportedCapabilities: "Capacités prises en charge",
        fullySupportedCount: "{count} entièrement prises en charge",
        availableEvidence: "Preuves disponibles",
        availableCount: "{count} disponibles",
        attentionReasonCount: "{count} raisons actives",
        statusOverview: "Statut du runtime",
        evidenceAvailability: "Disponibilité des preuves",
        available: "Disponible",
        missing: "Manquant",
        sidecarOverview: "Statut du sidecar",
        capabilitySource: "Source des capacités",
        lastCapabilityRefresh: "Dernière actualisation des capacités",
        support: {
            fullySupported: "Entièrement pris en charge",
            partiallySupported: "Partiellement pris en charge",
            notSupported: "Non pris en charge",
            unsupported: "Non pris en charge",
            unknown: "Inconnu",
        },
        none: "aucun",
        na: "n/a",
    },
    runtimePanel: {
        windows: {
            openSelected: "Ouvrir la sélection",
            openAll: "Tout ouvrir",
            closeAll: "Tout fermer",
            close: "Fermer",
            activate: "Activer",
            external: "Nouvel onglet",
            count: "{count} fenêtres",
            one: "1 fenêtre",
            capacity: "{count} / {limit} fenêtres",
            policy: "Seul le panneau actif reste en direct ; les autres sont suspendus.",
            pausedTitle: "Panneau suspendu",
            pausedBody: "Activez cette fenêtre pour charger son panneau distant. Les fenêtres inactives restent suspendues afin de préserver les ressources du navigateur et du runtime.",
            pausedAction: "Activer le panneau",
            limitReached: "L’espace de travail a atteint sa limite de {limit} fenêtres. Fermez d’abord une fenêtre.",
            openAllLimited: "{count} runtimes ouverts sur {total}. L’espace de travail est limité à {limit} fenêtres.",
            openAllComplete: "{count} fenêtres de runtime sont prêtes.",
            workspaceLabel: "Espace de travail des fenêtres runtime",
            windowLabel: "{name} · {view}",
        },
        title: "Panneau enfant du runtime",
        notReady: "aucun runtime sélectionné",
        empty: "Sélectionne un runtime pour charger son panneau enfant.",
        compactTrustObserved: "Snapshot runtime en direct disponible.",
        compactTrustIdleReady: "La runtime est inactive mais saine et prête.",
        compactTrustUnobserved: "En attente du premier snapshot runtime.",
        compactTrustFetchFailed: "Le dernier rafraîchissement du runtime a échoué ; considère ces données comme potentiellement obsolètes.",
        compactTrustSidecarObserved: "Snapshot sidecar en direct disponible.",
        compactTrustSidecarUnobserved: "En attente du premier snapshot sidecar.",
        compactTrustSidecarFetchFailed: "Le dernier rafraîchissement du sidecar a échoué ; considère ces données comme potentiellement obsolètes.",
        compactTrustNoSidecar: "Aucun sidecar apparié configuré.",
        blankRuntimeTitle: "Ce panneau runtime n'est pas encore prêt",
        blankRuntimeBody: "Nous savons où vit ce runtime, mais il n'a pas encore publié de données prêtes pour le panneau. Pour l'instant, appuie-toi surtout sur le résumé du control plane au-dessus.",
        blankSidecarTitle: "Ce panneau sidecar n'est pas encore prêt",
        blankSidecarBody: "Le sidecar de diagnostic apparié est configuré, mais n'a pas encore produit de snapshot stable pour l'affichage. Commence par la vue control plane puis rafraîchis si nécessaire.",
        blankFetchFailedTitle: "Nous ne pouvons pas encore ouvrir ce panneau en toute sécurité",
        blankFetchFailedBody: "Le dernier rafraîchissement d'état a échoué, donc montrer l'endpoint brut maintenant serait plus trompeur qu'utile. Rafraîchis d'abord, puis réessaie.",
        blankHintRefreshRuntime: "Essaie d'abord de rafraîchir l'état du runtime.",
        blankHintRefreshSidecar: "Essaie d'abord de rafraîchir l'état du sidecar.",
        blankHintEndpoint: "endpoint",
        breadcrumbFleet: "panneau runtime",
        breadcrumbSource: "source",
        breadcrumbView: "vue",
        currentView: "vue actuelle",
        sourceUrl: "URL source",
        sourceStatus: "état source",
        trustTitle: "niveau de confiance",
        trustObserved: "état runtime observé",
        trustObservedMessage: "Ce panneau enfant est soutenu par un snapshot gewyvern observé et peut généralement être lu comme un contexte runtime vivant.",
        trustIdleReady: "Runtime prête en veille",
        trustIdleReadyMessage: "Cette runtime est actuellement inactive mais saine. Elle n'a pas encore produit de nouveau snapshot de charge utile, mais le service socket est vivant et prêt pour la prochaine connexion.",
        trustUnobserved: "runtime non observé",
        trustUnobservedMessage: "Ce runtime n'a pas encore publié de latest snapshot, donc tant que l'état n'arrive pas il vaut mieux traiter ce panneau comme une fine coque d'endpoint.",
        trustFetchFailed: "échec du rafraîchissement d'état",
        trustFetchFailedMessage: "Le control plane n'a pas réussi à rafraîchir l'état de ce runtime ; le panneau enfant peut donc être obsolète ou inaccessible.",
        trustSidecarObserved: "état sidecar observé",
        trustSidecarObservedMessage: "Derrière ce panneau enfant, le sidecar etragon apparié est joignable ; tu peux le lire comme un contexte vivant du partenaire de diagnostic.",
        trustSidecarUnobserved: "sidecar non observé",
        trustSidecarUnobservedMessage: "Un endpoint etragon apparié est configuré, mais le control plane n'a pas encore observé de snapshot d'état sidecar.",
        trustSidecarFetchFailed: "échec du rafraîchissement sidecar",
        trustSidecarFetchFailedMessage: "Le control plane n'a pas réussi à rafraîchir le sidecar etragon apparié ; le panneau sidecar peut donc être obsolète ou inaccessible.",
        trustNoSidecar: "aucun sidecar apparié",
        trustNoSidecarMessage: "Ce runtime n'annonce actuellement aucun endpoint sidecar etragon apparié.",
        trustMeta: "source d'état : {source} · snapshot : {snapshot}",
        trustRefreshStatus: "Rafraîchir l'état maintenant",
        trustRefreshSidecar: "Rafraîchir le sidecar maintenant",
        openExternal: "Ouvrir dans un nouvel onglet",
        sources: { runtime: "Runtime", sidecar: "Sidecar" },
        views: { root: "Accueil", health: "Santé", meta: "Latest Meta", summary: "Résumé", analysis: "Analyse", targets: "Targets", sidecarRoot: "Accueil Sidecar", sidecarHealth: "Santé Sidecar", sidecarStatus: "État Sidecar", sidecarEnrichment: "Enrichissement", sidecarOpinion: "Opinion" },
    },
    register: {
        title: "Enregistrer un runtime",
        intake: "intake léger",
        targetSection: "Runtime cible",
        targetSectionCopy: "Identifiez le service gewyvern que ce control plane va gérer.",
        accessSection: "Accès de contrôle",
        accessSectionCopy: "Appairez avec un jeton de courte durée, jamais affiché dans l'aperçu.",
        sidecarSection: "Sidecar facultatif",
        sidecarSectionCopy: "Ajoutez-le uniquement si le runtime expose un service compagnon.",
        placementSection: "Placement et découverte",
        placementSectionCopy: "Les tags facultatifs facilitent la découverte sans changer l'identité.",
        showToken: "Afficher",
        hideToken: "Masquer",
        completeField: "Renseignez {field} pour continuer.",
        checkingPlan: "Vérification du plan d'enregistrement avec le control plane...",
        planUnavailable: "Impossible de vérifier le plan d'enregistrement : {message}",
        ready: "Plan d'enregistrement vérifié. Le runtime peut être enregistré.",
        fixHighlighted: "Vérifiez le champ mis en évidence avant l'enregistrement.",
        name: "Nom",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "Endpoint",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "Endpoint du sidecar",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        pairingToken: "Jeton d'appairage",
        tokenPlaceholder: "token-123",
        fetchCapabilities: "Récupérer capability et latest-meta depuis gewyvern lors de l'enregistrement",
        submit: "Enregistrer le runtime",
        clear: "Effacer le formulaire",
        untouched: "Aucun runtime envoyé pour l'instant.",
        previewTitle: "Aperçu en direct",
        previewName: "nom",
        previewSlice: "slice",
        previewEndpoint: "endpoint",
        previewSidecar: "sidecar",
        previewCapabilityFetch: "récupération de capability",
        suggested: "suggéré",
        pendingRuntimeName: "nom de runtime en attente",
        endpointPending: "en attente",
        endpointValid: "valide",
        endpointInvalid: "invalide",
        sidecarUnpaired: "non apparié",
        capabilityEnabled: "activé",
        capabilityDisabled: "désactivé",
        blockedEndpoint: "Enregistrement bloqué : l'endpoint doit commencer par http:// ou https:// et être une URL valide.",
        blockedSidecarEndpoint: "Enregistrement bloqué : l'endpoint sidecar doit commencer par http:// ou https:// et être une URL valide.",
        blockedDuplicate: "Enregistrement bloqué : {reason} existe déjà sur {name} ({endpoint}).",
        deletionInProgress: "L'enregistrement est suspendu pendant la suppression de ce runtime. Attendez la fin du nettoyage, puis vérifiez à nouveau le plan.",
        duplicateNameAndEndpoint: "nom et endpoint",
        duplicateName: "nom",
        duplicateEndpoint: "endpoint",
        registering: "Enregistrement du runtime...",
        registered: "{name} ({runtimeId}) a été enregistré dans {slice} avec l'état {status}.",
        failed: "Échec de l'enregistrement : {message}. Si ce runtime existe déjà, sélectionne-le directement dans le tableau.",
        allRuntimes: "tous les runtimes",
    },
    persistence: {
        title: "Persistance",
        chip: "état du control plane",
        yes: "oui",
        no: "non",
        saveNow: "Enregistrer maintenant",
        exportState: "Exporter l'état",
        importState: "Importer l'état",
        stateFile: "fichier d'état",
        lastSaved: "dernier enregistrement",
        enabled: "activé",
        schema: "schema",
        state: "état",
        restoredRuntimes: "runtimes restaurés",
        restoredSessions: "sessions restaurées",
        statePath: "chemin du fichier d'état",
        backupPath: "chemin de sauvegarde",
        schemaVersion: "version du schema",
        lastSavedAt: "dernier enregistrement à",
        lastSaveError: "dernière erreur d'enregistrement",
        restoredFromSave: "restauré depuis",
        configured: "configuré",
        missing: "manquant",
        never: "jamais",
        unknown: "inconnu",
        none: "aucun",
        clean: "propre",
        dirty: "avec modifications",
        saving: "Enregistrement de l'état du control plane...",
        saved: "État du control plane enregistré.",
        saveFailed: "Échec de l'enregistrement : {message}",
        exporting: "Export de l'état du control plane...",
        exported: "État du control plane exporté.",
        exportFailed: "Échec de l'export : {message}",
        importing: "Import de l'état du control plane depuis {file}...",
        invalidJson: "le fichier choisi n'est pas un JSON valide",
        imported: "{runtimes} runtimes et {sessions} sessions importés.",
        importFailed: "Échec de l'import : {message}",
    },
    attention: {
        noReasons: "Aucune attention reason active.",
        reasonLine: "{reason} · {count} runtimes",
        noRuntimes: "Aucun runtime n'a besoin d'attention dans ce slice.",
        critical: "critique",
        warning: "avertissement",
        statusFetchFailed: "status_fetch_failed",
        sidecarStatusFetchFailed: "sidecar_status_fetch_failed",
        noLatestSnapshot: "aucun instantané récent",
        noAnalysisJson: "aucun JSON d’analyse",
    },
    sessions: { title: "Sessions", none: "Pas encore de sessions.", runtime: "runtime" },
    metrics: {
        runtimes: "runtimes",
        latestSnapshots: "latest snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "sidecar context",
        diagnosticOpinions: "diagnostic opinions",
        pairedSidecars: "paired sidecars",
        healthySidecars: "healthy sidecars",
        critical: "critique",
        warning: "avertissement",
    },
    groups: {
        snapshotKinds: "types de snapshot",
        statusSources: "sources d'état",
        sidecarStatusSources: "sources d'état du sidecar",
        environments: "environnements",
        clusters: "clusters",
        roles: "rôles",
        empty: "Pas encore de données groupées.",
    },
    statuses: {
        fetchFailed: "échec de récupération",
        idleReady: "veille prête",
        unobserved: "non observé",
        observedSnapshot: "snapshot {kind}",
        observed: "observé",
        sidecarObserved: "sidecar observé",
        sidecarStarting: "sidecar en démarrage",
        sidecarDegraded: "sidecar dégradé",
        sidecarFetchFailed: "échec sidecar",
    },
    notifications: {
        loading: "Chargement de l'état du control plane...",
        loaded: "{count} runtimes chargés.",
        dashboardLoadFailed: "Échec du chargement du tableau de bord : {message}",
        noRuntimeSelected: "Aucun runtime sélectionné.",
        runtimeLinkCopied: "Lien du runtime copié.",
        runtimeLinkFailed: "Échec de la copie du lien : {message}",
        runtimeRefreshAll: "rafraîchissement complet du runtime",
        runtimeRefreshStatus: "rafraîchissement de l'état du runtime",
        runtimeRefreshCapabilities: "rafraîchissement des capacités du runtime",
        runtimeRefreshComplete: "{label} terminé.",
        runtimeRefreshFailed: "{label} a échoué : {message}",
        fleetRefreshAll: "rafraîchissement complet du fleet",
        fleetStatusRefresh: "rafraîchissement de l'état du fleet",
        fleetCapabilityRefresh: "rafraîchissement des capacités du fleet",
        fleetRefreshComplete: "{label} terminé.",
        fleetRefreshFailed: "{label} a échoué : {message}",
        badgeUpdated: "mis à jour",
    },
});
function t(key, params = {}) {
    const parts = key.split(".");
    const localeTree = translations[state.language] || translations.en;
    let value = localeTree;
    for (const part of parts)
        value = value?.[part];
    if (typeof value !== "string") {
        value = translations.en;
        for (const part of parts)
            value = value?.[part];
    }
    if (typeof value !== "string") {
        value = key;
    }
    return value.replace(/\{(\w+)\}/g, (_, name) => String(params[name] ?? `{${name}}`));
}
function protocolKeyToTranslationSegment(value) {
    return String(value || "").replace(/[_-]([a-z0-9])/gi, (_, character) => character.toUpperCase());
}
function attentionReasonLabel(reason) {
    const key = `attention.${protocolKeyToTranslationSegment(reason)}`;
    const translated = t(key);
    return translated === key ? String(reason || "") : translated;
}
function getStoredLanguagePreference() {
    try {
        return window.localStorage.getItem(storageKeys.languagePreference);
    }
    catch {
        return null;
    }
}
function setStoredLanguagePreference(value) {
    try {
        if (!value || value === "auto") {
            window.localStorage.removeItem(storageKeys.languagePreference);
            return;
        }
        window.localStorage.setItem(storageKeys.languagePreference, value);
    }
    catch {
    }
}
function getStoredThemePreference() {
    try {
        return window.localStorage.getItem(storageKeys.themePreference);
    }
    catch {
        return null;
    }
}
function setStoredThemePreference(value) {
    try {
        if (!value || value === "auto") {
            window.localStorage.removeItem(storageKeys.themePreference);
            return;
        }
        window.localStorage.setItem(storageKeys.themePreference, value);
    }
    catch {
    }
}
function activateTab(tab) {
    state.activeTab = tab;
    if (tab !== "orchestra") {
        clearOrchestraPollTimers();
    }
    applyTabShell();
    renderDashboardFromCache();
    if (tab === "runtimes") {
        syncCleanupMenuState();
        if (state.activeRuntimeMainTab === "detail" && state.selectedRuntimeId) {
            void loadRuntimeAttention(state.selectedRuntimeId);
        }
    }
    syncLocation();
    if (tab === "orchestra") {
        ensureRuntimeSelectionFromCache();
        void loadOrchestraPlan(state.selectedRuntimeId);
        void loadOrchestraFleetBoard();
    }
}
function activateOverviewSubtab(tab) {
    state.activeOverviewTab = tab;
    applyTabShell();
    renderDashboardFromCache();
    syncLocation();
}
function activateRuntimeMainTab(tab) {
    state.activeRuntimeMainTab = ["register", "detail", "panel"].includes(tab) ? tab : "select";
    state.activeRuntimeSideTab = state.activeRuntimeMainTab === "panel" ? "panel" : "detail";
    state.activeTab = "runtimes";
    applyTabShell();
    renderDashboardFromCache();
    syncCleanupMenuState();
    if (state.activeRuntimeMainTab === "register") {
        renderRegisterPreview();
    }
    else if (state.activeRuntimeMainTab === "detail" && state.selectedRuntimeId) {
        void loadRuntimeAttention(state.selectedRuntimeId);
    }
    else if (state.activeRuntimeMainTab === "panel" && state.selectedRuntimeId) {
        openRuntimeWindow(state.selectedRuntimeId);
    }
    syncLocation();
}
function normalizeRuntimeDetailTab(tab) {
    return ["identity", "status", "capabilities", "attention"].includes(tab) ? tab : "identity";
}
function activateRuntimeDetailTab(tab) {
    state.activeRuntimeDetailTab = normalizeRuntimeDetailTab(tab);
    applyTabShell();
    syncLocation();
}
function bindRovingTabs(buttons, dataKey, activate) {
    buttons.forEach((button, index) => {
        button.addEventListener("keydown", (event) => {
            let nextIndex = null;
            const direction = document.documentElement.dir === "rtl" ? -1 : 1;
            if (event.key === "ArrowRight")
                nextIndex = index + direction;
            if (event.key === "ArrowLeft")
                nextIndex = index - direction;
            if (event.key === "Home")
                nextIndex = 0;
            if (event.key === "End")
                nextIndex = buttons.length - 1;
            if (nextIndex === null) {
                return;
            }
            event.preventDefault();
            const target = buttons[(nextIndex + buttons.length) % buttons.length];
            activate(target.dataset[dataKey]);
            target.focus();
        });
    });
}
function runtimeTableRows() {
    return Array.from(nodes.runtimeTableBody.querySelectorAll("tr[data-runtime-id]"))
        .filter((row) => row instanceof HTMLTableRowElement);
}
function selectRuntimeTableRow(row, restoreFocus = false) {
    if (!(row instanceof HTMLTableRowElement) || !row.dataset.runtimeId) {
        return;
    }
    state.selectedRuntimeId = row.dataset.runtimeId;
    renderRuntimeSliceFromCache();
    syncLocation();
    if (restoreFocus) {
        window.requestAnimationFrame(() => {
            const selected = nodes.runtimeTableBody.querySelector(`tr[data-runtime-id="${CSS.escape(state.selectedRuntimeId)}"]`);
            if (selected instanceof HTMLTableRowElement)
                selected.focus();
        });
    }
}
function closeOpenRuntimeRowMenu(restoreFocus = false, except = null) {
    let focusTarget = null;
    let closed = false;
    for (const menu of nodes.runtimeTableBody.querySelectorAll(".runtime-row-menu[open]")) {
        if (!(menu instanceof HTMLDetailsElement) || menu === except)
            continue;
        if (!focusTarget)
            focusTarget = menu.querySelector("summary");
        menu.open = false;
        closed = true;
    }
    if (restoreFocus && focusTarget instanceof HTMLElement) {
        window.requestAnimationFrame(() => focusTarget.focus());
    }
    return closed;
}
async function handleRuntimeTableAction(button) {
    const runtimeId = button.dataset.runtimeId;
    if (!runtimeId) {
        return;
    }
    if (button.dataset.action === "show-attention") {
        state.activeTab = "runtimes";
        state.activeRuntimeMainTab = "detail";
        state.activeRuntimeDetailTab = "attention";
        state.selectedRuntimeId = runtimeId;
        applyTabShell();
        renderRuntimeSliceFromCache();
        syncLocation();
        nodes.runtimeDetailPanel.scrollIntoView({ behavior: "smooth", block: "start" });
        return;
    }
    if (button.dataset.action === "open-panel") {
        state.activeTab = "runtimes";
        state.activeRuntimeMainTab = "panel";
        openRuntimeWindow(runtimeId);
        applyTabShell();
        return;
    }
    if (button.dataset.action === "delete-runtime") {
        await deleteRuntime(runtimeId, button.dataset.runtimeName || runtimeId);
        return;
    }
    const kind = button.dataset.action === "refresh-status"
        ? "status"
        : button.dataset.action === "refresh-sidecar"
            ? "sidecar"
            : "all";
    await refreshRuntimeById(runtimeId, kind, button);
}
function bootstrapDashboard() {
    restoreLanguagePacks();
    restoreRuntimeWindows();
    nodes.mobileFilterToggle?.addEventListener("click", () => {
        setMobileFiltersOpen(!state.mobileFiltersOpen);
    });
    nodes.tabButtons.forEach((button) => {
        button.addEventListener("click", () => activateTab(button.dataset.tab));
    });
    nodes.overviewSubtabButtons.forEach((button) => {
        button.addEventListener("click", () => activateOverviewSubtab(button.dataset.overviewTab));
    });
    nodes.runtimeMainTabButtons.forEach((button) => {
        button.addEventListener("click", () => activateRuntimeMainTab(button.dataset.runtimeMainTab));
    });
    nodes.runtimeDetailSubtabButtons.forEach((button) => {
        button.addEventListener("click", () => activateRuntimeDetailTab(button.dataset.runtimeDetailTab));
    });
    bindRovingTabs(nodes.overviewSubtabButtons, "overviewTab", activateOverviewSubtab);
    bindRovingTabs(nodes.runtimeMainTabButtons, "runtimeMainTab", activateRuntimeMainTab);
    bindRovingTabs(nodes.runtimeDetailSubtabButtons, "runtimeDetailTab", activateRuntimeDetailTab);
    nodes.runtimePanelTabs.forEach((button) => {
        button.addEventListener("click", () => {
            state.runtimePanelView = button.dataset.runtimePanelView;
            if (state.activeRuntimeWindowId) {
                state.runtimeWindowViews[state.activeRuntimeWindowId] = state.runtimePanelView;
                persistRuntimeWindows();
            }
            const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
            renderRuntimePanel(selectedRuntime);
            syncLocation();
        });
    });
    nodes.runtimePanelSourceButtons.forEach((button) => {
        button.addEventListener("click", () => {
            const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
            switchRuntimePanelSource(button.dataset.runtimePanelSource, selectedRuntime);
        });
    });
    nodes.runtimePanelOpenExternal.addEventListener("click", () => {
        const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
        const targetUrl = runtimePanelUrl(selectedRuntime);
        if (!targetUrl) {
            nodes.statusLine.textContent = t("notifications.noRuntimeSelected");
            return;
        }
        window.open(targetUrl, "_blank", "noopener,noreferrer");
    });
    nodes.runtimeWindowOpenSelected?.addEventListener("click", () => {
        if (state.selectedRuntimeId) {
            openRuntimeWindow(state.selectedRuntimeId);
        }
    });
    nodes.runtimeWindowOpenAll?.addEventListener("click", openAllRuntimeWindows);
    nodes.runtimeWindowCloseAll?.addEventListener("click", closeAllRuntimeWindows);
    nodes.runtimeWindowGrid?.addEventListener("click", handleRuntimeWindowGridClick);
    nodes.runtimeWindowGrid?.addEventListener("keydown", handleRuntimeWindowGridKeydown);
    nodes.languageSelect.addEventListener("change", () => {
        state.languagePreference = nodes.languageSelect.value;
        state.language = resolveLanguage(state.languagePreference);
        setStoredLanguagePreference(state.languagePreference);
        applyTranslations();
        renderDashboardFromCache();
        syncLocation();
    });
    nodes.languagePackDetails?.addEventListener("toggle", () => {
        if (nodes.languagePackDetails.open) {
            nodes.securityDetails?.removeAttribute("open");
            renderLanguagePackCenter();
            if (!state.languagePackCatalog.length)
                void loadLanguagePackCatalog();
        }
    });
    nodes.languagePackRefresh?.addEventListener("click", loadLanguagePackCatalog);
    nodes.languagePackImport?.addEventListener("click", () => nodes.languagePackFile.click());
    nodes.languagePackFile?.addEventListener("change", (event) => importLanguagePackFile(event.target.files?.[0]));
    nodes.languagePackDetails?.addEventListener("click", (event) => {
        const button = event.target.closest("[data-language-pack-action][data-locale]");
        if (button)
            void handleLanguagePackAction(button);
    });
    nodes.themeSelect.addEventListener("change", () => {
        state.themePreference = nodes.themeSelect.value;
        state.theme = resolveTheme(state.themePreference);
        setStoredThemePreference(state.themePreference);
        applyTheme();
        syncLocation();
    });
    nodes.applyFiltersButton.addEventListener("click", applyFleetFilters);
    for (const input of [nodes.environmentInput, nodes.clusterInput, nodes.roleInput]) {
        input.addEventListener("input", syncFilterActionState);
        input.addEventListener("keydown", (event) => {
            if (event.key === "Enter" && !nodes.applyFiltersButton.disabled) {
                event.preventDefault();
                applyFleetFilters();
            }
        });
    }
    nodes.clearFiltersButton.addEventListener("click", () => {
        state.filter.environment = "";
        state.filter.cluster = "";
        state.filter.role = "";
        state.runtimeSearch = "";
        state.selectedRuntimeId = null;
        syncFilterActionState();
        if (window.innerWidth <= 920) {
            setMobileFiltersOpen(false, true);
        }
        void loadDashboard();
    });
    nodes.runtimeSearch.addEventListener("input", () => {
        state.runtimeSearch = nodes.runtimeSearch.value.trim();
        syncFilterActionState();
        scheduleRuntimeSliceRender();
        syncLocation();
    });
    nodes.runtimeSort.addEventListener("change", () => {
        state.runtimeSort = nodes.runtimeSort.value;
        renderRuntimeSliceFromCache();
        syncLocation();
    });
    nodes.runtimeTableBody.addEventListener("click", async (event) => {
        const target = event.target;
        if (!(target instanceof Element)) {
            return;
        }
        const actionButton = target.closest("button[data-action][data-runtime-id]");
        if (actionButton instanceof HTMLButtonElement) {
            event.stopPropagation();
            closeOpenRuntimeRowMenu();
            await handleRuntimeTableAction(actionButton);
            return;
        }
        const rowMenu = target.closest(".runtime-row-menu");
        if (rowMenu instanceof HTMLDetailsElement) {
            if (target.closest("summary"))
                closeOpenRuntimeRowMenu(false, rowMenu);
            event.stopPropagation();
            return;
        }
        const row = target.closest("tr[data-runtime-id]");
        if (!(row instanceof HTMLTableRowElement)) {
            return;
        }
        selectRuntimeTableRow(row);
    });
    nodes.runtimeTableBody.addEventListener("keydown", (event) => {
        const target = event.target;
        if (!(target instanceof HTMLElement))
            return;
        const row = target.closest("tr[data-runtime-id]");
        if (!(row instanceof HTMLTableRowElement) || target !== row)
            return;
        if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            selectRuntimeTableRow(row, true);
            return;
        }
        const rows = runtimeTableRows();
        const index = rows.indexOf(row);
        let nextIndex = null;
        if (event.key === "ArrowDown")
            nextIndex = Math.min(index + 1, rows.length - 1);
        if (event.key === "ArrowUp")
            nextIndex = Math.max(index - 1, 0);
        if (event.key === "Home")
            nextIndex = 0;
        if (event.key === "End")
            nextIndex = rows.length - 1;
        if (nextIndex === null || nextIndex === index || !rows[nextIndex])
            return;
        event.preventDefault();
        selectRuntimeTableRow(rows[nextIndex], true);
    });
    nodes.runtimeDetailAttention.addEventListener("click", async (event) => {
        const target = event.target;
        if (!(target instanceof Element)) {
            return;
        }
        const button = target.closest("button[data-recovery-action]");
        if (!(button instanceof HTMLButtonElement) || button.disabled) {
            return;
        }
        await refreshSelectedRuntime(button.dataset.recoveryAction, button);
    });
    nodes.runtimeDetailSummary.addEventListener("click", (event) => {
        const target = event.target;
        if (!(target instanceof Element))
            return;
        const button = target.closest("button[data-runtime-detail-target]");
        if (!(button instanceof HTMLButtonElement))
            return;
        activateRuntimeDetailTab(button.dataset.runtimeDetailTarget);
        const activeTab = nodes.runtimeDetailSubtabButtons.find((candidate) => candidate.dataset.runtimeDetailTab === state.activeRuntimeDetailTab);
        if (activeTab instanceof HTMLButtonElement)
            activeTab.focus();
    });
    nodes.runtimeDeleteFailed?.addEventListener("click", deleteFailedRuntimes);
    nodes.runtimeDeleteUnobserved?.addEventListener("click", deleteUnobservedRuntimes);
    nodes.runtimeClearSlice?.addEventListener("click", clearRuntimeSlice);
    nodes.runtimeCleanupMenu?.addEventListener("toggle", syncCleanupMenuState);
    nodes.refreshAllButton.addEventListener("click", () => postAndReload("/v1/fleet/refresh-all", t("notifications.fleetRefreshAll"), nodes.refreshAllButton));
    nodes.refreshStatusButton.addEventListener("click", () => postAndReload("/v1/fleet/refresh-status", t("notifications.fleetStatusRefresh"), nodes.refreshStatusButton));
    nodes.refreshCapabilitiesButton.addEventListener("click", () => postAndReload("/v1/fleet/refresh-capabilities", t("notifications.fleetCapabilityRefresh"), nodes.refreshCapabilitiesButton));
    nodes.orchestraRefresh?.addEventListener("click", () => loadOrchestraPlan());
    nodes.orchestraPlans?.addEventListener("click", (event) => {
        const button = event.target.closest("[data-orchestra-execute]");
        if (button) {
            const card = button.closest(".orchestra-plan-card");
            void executeOrchestraPlan(button.dataset.orchestraExecute, button.dataset.orchestraRevision, button.dataset.orchestraApproval, card?.querySelector("[data-orchestra-approved-by]")?.value?.trim(), card?.querySelector("[data-orchestra-approval-note]")?.value?.trim());
            return;
        }
        const sessionButton = event.target.closest("[data-orchestra-create-session]");
        if (sessionButton) {
            void createOrchestraSession(sessionButton);
        }
    });
    nodes.orchestraHistory?.addEventListener("click", (event) => {
        const eventsButton = event.target.closest("[data-orchestra-load-events]");
        if (eventsButton) {
            void loadOrchestraRunEvents(eventsButton.dataset.orchestraLoadEvents, eventsButton);
            return;
        }
        const cancelButton = event.target.closest("[data-orchestra-cancel-run]");
        if (cancelButton) {
            void mutateOrchestraRun(cancelButton.dataset.orchestraCancelRun, "cancel", cancelButton);
            return;
        }
        const retryButton = event.target.closest("[data-orchestra-retry-run]");
        if (retryButton) {
            void mutateOrchestraRun(retryButton.dataset.orchestraRetryRun, "retry", retryButton);
        }
    });
    nodes.orchestraFleetRuns?.addEventListener("click", (event) => {
        const target = event.target.closest("[data-orchestra-runtime-id]");
        if (!target) {
            return;
        }
        state.selectedRuntimeId = target.dataset.orchestraRuntimeId;
        renderRuntimeSliceFromCache();
        syncLocation();
        void loadOrchestraPlan(state.selectedRuntimeId);
    });
    nodes.persistenceSaveNow.addEventListener("click", savePersistenceNow);
    nodes.persistenceExportState.addEventListener("click", exportPersistenceState);
    nodes.persistenceImportState.addEventListener("click", triggerPersistenceImportPicker);
    nodes.persistenceImportFile.addEventListener("change", (event) => {
        const [file] = event.target.files || [];
        importPersistenceState(file);
    });
    nodes.runtimeDetailRefreshAll.addEventListener("click", () => refreshSelectedRuntime("all", nodes.runtimeDetailRefreshAll));
    nodes.runtimeDetailRefreshStatus.addEventListener("click", () => refreshSelectedRuntime("status", nodes.runtimeDetailRefreshStatus));
    nodes.runtimeDetailRefreshCapabilities.addEventListener("click", () => refreshSelectedRuntime("capabilities", nodes.runtimeDetailRefreshCapabilities));
    nodes.runtimeDetailRefreshSidecar.addEventListener("click", () => refreshSelectedRuntime("sidecar", nodes.runtimeDetailRefreshSidecar));
    nodes.runtimeDetailCopyLink.addEventListener("click", copySelectedRuntimeLink);
    nodes.registerName.addEventListener("input", () => {
        state.registerNameTouched = nodes.registerName.value.trim().length > 0;
        nodes.registerName.setAttribute("aria-invalid", "false");
        scheduleRegistrationPlanPreview();
    });
    nodes.registerEndpoint.addEventListener("input", maybePrefillRuntimeNameFromEndpoint);
    nodes.registerSidecarEndpoint.addEventListener("input", scheduleRegistrationPlanPreview);
    nodes.registerSidecarAdminToken.addEventListener("input", scheduleRenderRegisterPreview);
    nodes.registerToken.addEventListener("input", () => {
        nodes.registerToken.setAttribute("aria-invalid", "false");
        scheduleRenderRegisterPreview();
    });
    nodes.registerTokenToggle.addEventListener("click", () => {
        setRegistrationSecretVisibility(nodes.registerToken, nodes.registerTokenToggle, nodes.registerTokenToggleLabel, nodes.registerToken.type === "password");
        nodes.registerToken.focus();
    });
    nodes.registerSidecarAdminTokenToggle.addEventListener("click", () => {
        setRegistrationSecretVisibility(nodes.registerSidecarAdminToken, nodes.registerSidecarAdminTokenToggle, nodes.registerSidecarAdminTokenToggleLabel, nodes.registerSidecarAdminToken.type === "password");
        nodes.registerSidecarAdminToken.focus();
    });
    nodes.registerSidecarDetails.addEventListener("toggle", () => {
        if (!nodes.registerSidecarDetails.open) {
            setRegistrationSecretVisibility(nodes.registerSidecarAdminToken, nodes.registerSidecarAdminTokenToggle, nodes.registerSidecarAdminTokenToggleLabel, false);
        }
    });
    nodes.registerRuntimeEnvironment.addEventListener("input", scheduleRenderRegisterPreview);
    nodes.registerRuntimeCluster.addEventListener("input", scheduleRenderRegisterPreview);
    nodes.registerRuntimeRole.addEventListener("input", scheduleRenderRegisterPreview);
    nodes.registerFetchCapabilities.addEventListener("change", scheduleRenderRegisterPreview);
    nodes.registerForm.addEventListener("submit", submitRegisterForm);
    nodes.registerForm.addEventListener("invalid", (event) => {
        const field = event.target;
        field.setAttribute("aria-invalid", "true");
        if (nodes.registerSidecarDetails.contains(field)) {
            nodes.registerSidecarDetails.open = true;
        }
        setRegisterResult(t("register.fixHighlighted"), "bad");
    }, true);
    nodes.registerFormClear.addEventListener("click", clearRegisterForm);
    document.addEventListener("visibilitychange", () => {
        if (document.hidden) {
            maskRegistrationSecrets();
        }
    });
    if (nodes.adminTokenInput) {
        nodes.adminTokenInput.value = state.adminToken;
        nodes.adminTokenInput.addEventListener("input", (event) => syncAdminTokenFromInput(event.currentTarget.value));
        nodes.adminTokenInput.addEventListener("keydown", (event) => {
            if (event.key !== "Enter") {
                return;
            }
            event.preventDefault();
            void testAdminToken();
        });
    }
    nodes.adminTokenToggleVisibility?.addEventListener("click", () => {
        state.adminTokenVisible = !state.adminTokenVisible;
        updateAdminTokenVisibilityButton();
    });
    nodes.adminTokenTest?.addEventListener("click", () => {
        void testAdminToken();
    });
    nodes.adminTokenClear?.addEventListener("click", clearAdminToken);
    document.addEventListener("click", (event) => {
        if (nodes.runtimeCleanupMenu?.open) {
            if (!(event.target instanceof Node) || !nodes.runtimeCleanupMenu.contains(event.target)) {
                nodes.runtimeCleanupMenu.open = false;
            }
        }
        if (!(event.target instanceof Node)) {
            return;
        }
        if (nodes.securityDetails?.open && !nodes.securityDetails.contains(event.target)) {
            closeSecurityDetails();
        }
        if (nodes.languagePackDetails?.open && !nodes.languagePackDetails.contains(event.target)) {
            nodes.languagePackDetails.open = false;
        }
    });
    document.addEventListener("keydown", (event) => {
        if (event.key === "Escape") {
            if (closeOpenRuntimeRowMenu(true))
                event.preventDefault();
            if (state.mobileFiltersOpen)
                setMobileFiltersOpen(false, true);
            if (nodes.securityDetails?.open)
                closeSecurityDetails();
            if (nodes.languagePackDetails?.open)
                nodes.languagePackDetails.open = false;
        }
    });
    document.addEventListener("click", (event) => {
        const target = event.target;
        if (target instanceof Element && !target.closest(".runtime-row-menu")) {
            closeOpenRuntimeRowMenu();
        }
    });
    nodes.securityDetails?.addEventListener("toggle", () => {
        syncSecurityDetailsState();
        if (nodes.securityDetails.open) {
            nodes.languagePackDetails?.removeAttribute("open");
            window.setTimeout(() => {
                nodes.adminTokenInput?.focus();
                nodes.adminTokenInput?.select();
            }, 0);
        }
    });
    syncSecurityDetailsState();
    window.matchMedia?.("(prefers-color-scheme: dark)")?.addEventListener("change", () => {
        if (state.themePreference !== "auto") {
            return;
        }
        state.theme = resolveTheme(state.themePreference);
        applyTheme();
    });
    window.addEventListener("resize", () => {
        if (state.pendingLayoutFrame) {
            return;
        }
        state.pendingLayoutFrame = window.requestAnimationFrame(() => {
            state.pendingLayoutFrame = 0;
            applyLayoutMode();
        });
    });
    if (nodes.runtimeListCard && typeof ResizeObserver === "function") {
        state.runtimeListLayoutObserver?.disconnect();
        state.runtimeListLayoutObserver = new ResizeObserver((entries) => {
            const entry = entries.find((candidate) => candidate.target === nodes.runtimeListCard);
            if (entry)
                syncRuntimeListLayout(entry.contentRect.width);
        });
        state.runtimeListLayoutObserver.observe(nodes.runtimeListCard);
    }
    document.addEventListener("visibilitychange", () => {
        if (document.hidden) {
            clearOrchestraPollTimers();
            return;
        }
        if (state.activeTab === "orchestra") {
            void loadOrchestraFleetBoard();
            void loadOrchestraHistory();
        }
    });
    hydrateStateFromLocation();
    applyTheme();
    applyLayoutMode();
    applyTranslations();
    renderSecurityState();
    applyTabShell();
    clearRegisterForm();
    loadDashboard();
}
translations.ko = mergeTranslations(translations.en, {
    hero: {
        title: "컨트롤 플레인 대시보드",
        subcopy: "가까이에 있는 여러 gewyvern 런타임을 위한 가벼운 fleet 보기입니다.",
    },
    language: {
        label: "언어",
        auto: "브라우저 따르기",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    theme: {
        label: "테마",
        auto: "시스템 따르기",
        light: "주간",
        dark: "야간",
    },
    security: {
        title: "보안",
        adminToken: "관리자 토큰",
        adminTokenPlaceholder: "원격 접속 시에만 선택적으로 사용",
        clearToken: "토큰 지우기",
        showToken: "토큰 보기",
        hideToken: "토큰 숨기기",
        testToken: "토큰 테스트",
        localModeHint: "현재 로컬 loopback 모드가 활성화되어 있습니다. 보호된 원격 엔드포인트를 통해 의도적으로 접속할 때만 토큰을 입력하세요.",
        tokenStored: "이 브라우저에 관리자 토큰을 저장했습니다.",
        tokenCleared: "관리자 토큰을 지웠습니다.",
        tokenRequired: "컨트롤 플레인이 이 요청을 거부했습니다. 원격으로 연결 중이라면 설정된 관리자 토큰을 입력하세요.",
        tokenTestRunning: "관리자 토큰을 테스트하는 중...",
        tokenTestOk: "현재 컨트롤 플레인이 이 관리자 토큰을 받아들였습니다.",
        tokenTestFailed: "관리자 토큰 테스트 실패: {message}",
        tokenMissing: "먼저 관리자 토큰을 입력한 뒤 테스트하세요.",
        lastTokenTest: "마지막 토큰 테스트",
        neverTested: "테스트한 적 없음",
        testStateOk: "성공",
        testStateFailed: "실패",
        testStateRunning: "테스트 중",
        mode: "보안 모드",
        tokenConfigured: "관리자 토큰",
        publicDiscovery: "공개 엔드포인트 발견",
        configured: "설정됨",
        notConfigured: "설정 안 됨",
        enabled: "활성화",
        disabled: "비활성화",
        loopbackOnly: "루프백 전용",
        tokenMode: "루프백 또는 토큰",
    },
    actions: {
        refreshAll: "Fleet 전체 새로고침",
        refreshStatus: "상태 새로고침",
        refreshCapabilities: "기능 새로고침",
    },
    filters: {
        title: "Fleet 필터",
        environment: "환경",
        environmentPlaceholder: "prod",
        cluster: "클러스터",
        clusterPlaceholder: "alpha",
        role: "역할",
        rolePlaceholder: "edge",
        apply: "필터 적용",
        clear: "지우기",
        allRuntimes: "모든 런타임",
    },
    tabs: {
        overview: "개요",
        runtimes: "런타임",
        register: "등록",
        persistence: "영속성",
        sessions: "세션",
    },
    overview: {
        fleetSummary: "Fleet 요약",
        attentionSummary: "주의 요약",
        triage: "트리아지",
        runtimesNeedingAttention: "주의가 필요한 런타임",
        kicker: "Fleet 상태",
        spotlightTitle: "개별 런타임으로 들어가기 전에 먼저 전체 상태를 보세요.",
        spotlightBody: "이 셸 안에서 fleet 상태, runtime 상세, 등록 intake, 영속성, 세션을 긴 스크롤 페이지 없이 오갈 수 있습니다.",
        spotlightRail1: "현재 슬라이스",
        spotlightRail2: "현재 모드",
        summaryChip: "실시간 집계",
    },
    runtimes: {
        title: "런타임",
        workspaceTabs: { select: "선택", register: "등록", detail: "상세", panel: "자식 패널" },
        quickSearch: "빠른 검색",
        quickSearchPlaceholder: "이름 또는 엔드포인트",
        sortBy: "정렬 기준",
        noMatch: "현재 필터나 검색과 일치하는 런타임이 없습니다.",
        columns: { name: "이름", tags: "태그", status: "상태", capabilitySurface: "기능 표면", sidecar: "사이드카", attention: "주의", actions: "동작" },
        sort: { name: "이름", status: "상태 출처", snapshot: "스냅샷 종류" },
        actions: { attention: "주의", status: "상태", all: "전체" },
        states: { clear: "정상", none: "없음", noEnv: "환경 없음", noCluster: "클러스터 없음", noRole: "역할 없음", noCapabilities: "fully-supported 기능 없음" },
    },
    runtimeDetail: {
        title: "런타임 상세",
        nothingSelected: "선택 없음",
        empty: "표에서 런타임을 선택해 capability, status, attention 상태를 확인하세요.",
        identity: "식별",
        status: "상태",
        capabilities: "기능",
        attention: "주의",
        refreshAll: "이 런타임 새로고침",
        refreshStatus: "상태 새로고침",
        refreshCapabilities: "기능 새로고침",
        refreshSidecar: "사이드카 새로고침",
        copyLink: "링크 복사",
        registered: "등록 시각",
        updated: "업데이트 시각",
        source: "출처",
        sidecarSource: "사이드카 출처",
        sidecarLearning: "사이드카 학습 상태",
        resilienceStatus: "복원력 상태",
        resilienceSummary: "복원력 요약",
        socketServiceStatus: "socket 서비스",
        idleTimeouts: "유휴 타임아웃 (현재 / 누적)",
        snapshotKind: "스냅샷 종류",
        targetCount: "타깃 수",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        noCapabilities: "기록된 기능이 없습니다.",
        clear: "정상",
        noAttention: "이 런타임에는 현재 attention reason 이 없습니다.",
        needsAttention: "주의 필요",
        sectionsLabel: "런타임 상세 섹션",
        liveSummary: "실시간 런타임 상태",
        operational: "최신 제어 평면 스냅샷을 사용할 수 있으며 현재 필요한 작업이 없습니다.",
        requiresReview: "이 런타임은 운영자 확인이 필요합니다.",
        refreshRecommended: "최근 런타임 새로 고침에 실패했습니다. 이 스냅샷을 사용하기 전에 다시 시도하세요.",
        reviewAttention: "주의 항목 확인",
        inspectStatus: "상태 보기",
        lastObserved: "최근 관측",
        notObserved: "관측되지 않음",
        runtimeId: "런타임 ID",
        supportedCapabilities: "지원되는 기능",
        fullySupportedCount: "{count}개 완전 지원",
        availableEvidence: "사용 가능한 증거",
        availableCount: "{count}개 사용 가능",
        attentionReasonCount: "활성 원인 {count}개",
        statusOverview: "런타임 상태",
        evidenceAvailability: "증거 가용성",
        available: "사용 가능",
        missing: "누락",
        sidecarOverview: "Sidecar 상태",
        capabilitySource: "기능 출처",
        lastCapabilityRefresh: "최근 기능 새로 고침",
        support: {
            fullySupported: "완전 지원",
            partiallySupported: "부분 지원",
            notSupported: "지원하지 않음",
            unsupported: "지원하지 않음",
            unknown: "알 수 없음",
        },
        none: "없음",
        na: "해당 없음",
    },
    runtimePanel: {
        windows: {
            openSelected: "선택 항목 열기",
            openAll: "모두 열기",
            closeAll: "모두 닫기",
            close: "닫기",
            activate: "활성화",
            external: "새 탭",
            count: "창 {count}개",
            one: "창 1개",
            capacity: "창 {count} / {limit}개",
            policy: "활성 패널만 연결되며 비활성 패널은 일시 중지됩니다.",
            pausedTitle: "패널 일시 중지됨",
            pausedBody: "이 창을 활성화하면 원격 패널을 불러옵니다. 비활성 창은 브라우저와 런타임 리소스를 보호하기 위해 일시 중지됩니다.",
            pausedAction: "패널 활성화",
            limitReached: "작업 공간이 창 {limit}개 제한에 도달했습니다. 먼저 창 하나를 닫으세요.",
            openAllLimited: "런타임 {total}개 중 {count}개를 열었습니다. 작업 공간은 창 {limit}개로 제한됩니다.",
            openAllComplete: "런타임 창 {count}개를 준비했습니다.",
            workspaceLabel: "런타임 창 작업 공간",
            windowLabel: "{name} · {view}",
        },
        title: "런타임 자식 패널",
        notReady: "선택된 런타임 없음",
        empty: "런타임을 선택하면 자식 제어 패널을 불러올 수 있습니다.",
        compactTrustObserved: "실시간 런타임 스냅샷을 사용할 수 있습니다.",
        compactTrustIdleReady: "런타임이 유휴 상태지만 건강하며 연결 준비가 되어 있습니다.",
        compactTrustUnobserved: "첫 런타임 스냅샷을 기다리는 중입니다.",
        compactTrustFetchFailed: "마지막 런타임 새로고침이 실패했습니다. 현재 데이터가 오래됐을 수 있습니다.",
        compactTrustSidecarObserved: "실시간 사이드카 스냅샷을 사용할 수 있습니다.",
        compactTrustSidecarUnobserved: "첫 사이드카 스냅샷을 기다리는 중입니다.",
        compactTrustSidecarFetchFailed: "마지막 사이드카 새로고침이 실패했습니다. 현재 데이터가 오래됐을 수 있습니다.",
        compactTrustNoSidecar: "연결된 사이드카가 구성되어 있지 않습니다.",
        blankRuntimeTitle: "이 런타임 패널은 아직 준비되지 않았습니다",
        blankRuntimeBody: "이 런타임이 어디 있는지는 알고 있지만, 아직 패널에 바로 보여줄 수 있는 데이터를 게시하지 않았습니다. 우선 위의 control plane 요약을 기준으로 보세요.",
        blankSidecarTitle: "이 사이드카 패널은 아직 준비되지 않았습니다",
        blankSidecarBody: "연결된 진단 사이드카는 구성되어 있지만, 아직 패널에 안정적으로 보여줄 상태 스냅샷을 만들지 못했습니다. 먼저 control plane 뷰를 보고 필요할 때 새로고침하세요.",
        blankFetchFailedTitle: "아직 이 패널을 안전하게 열 수 없습니다",
        blankFetchFailedBody: "가장 최근 상태 새로고침이 실패했기 때문에 지금 원시 엔드포인트를 그대로 보여주면 도움이 되기보다 더 혼란스러울 수 있습니다. 먼저 새로고침한 뒤 다시 시도하세요.",
        blankHintRefreshRuntime: "먼저 런타임 상태를 새로고침해 보세요.",
        blankHintRefreshSidecar: "먼저 사이드카 상태를 새로고침해 보세요.",
        blankHintEndpoint: "엔드포인트",
        breadcrumbFleet: "런타임 패널",
        breadcrumbSource: "출처",
        breadcrumbView: "보기",
        currentView: "현재 보기",
        sourceUrl: "출처 URL",
        sourceStatus: "출처 상태",
        trustTitle: "신뢰 상태",
        trustObserved: "런타임 상태 관측됨",
        trustObservedMessage: "이 자식 패널은 관측된 gewyvern 스냅샷을 기반으로 하므로 일반적으로 실시간 런타임 컨텍스트로 읽어도 됩니다.",
        trustIdleReady: "유휴 준비 완료 런타임",
        trustIdleReadyMessage: "이 런타임은 현재 유휴 상태지만 건강합니다. 아직 새 payload 스냅샷은 만들지 않았지만 socket 서비스는 살아 있고 다음 연결을 받을 준비가 되어 있습니다.",
        trustUnobserved: "런타임 미관측",
        trustUnobservedMessage: "이 런타임은 아직 latest snapshot 을 게시하지 않았으므로 상태가 도착하기 전까지는 자식 패널을 얇은 엔드포인트 껍데기로 보는 편이 맞습니다.",
        trustFetchFailed: "상태 가져오기 실패",
        trustFetchFailedMessage: "컨트롤 플레인이 이 런타임 상태를 새로고침하지 못했습니다. 그래서 자식 패널이 오래됐거나 닿지 않을 수 있습니다.",
        trustSidecarObserved: "사이드카 상태 관측됨",
        trustSidecarObservedMessage: "이 자식 패널 뒤의 연결된 etragon 사이드카에 접근할 수 있으므로 근처 진단 파트너 컨텍스트로 읽어도 됩니다.",
        trustSidecarUnobserved: "사이드카 미관측",
        trustSidecarUnobservedMessage: "연결된 etragon 엔드포인트는 설정되어 있지만 control plane 이 아직 사이드카 상태 스냅샷을 관측하지 못했습니다.",
        trustSidecarFetchFailed: "사이드카 가져오기 실패",
        trustSidecarFetchFailedMessage: "컨트롤 플레인이 연결된 etragon 사이드카를 새로고침하지 못했습니다. 그래서 사이드카 패널이 오래됐거나 닿지 않을 수 있습니다.",
        trustNoSidecar: "연결된 사이드카 없음",
        trustNoSidecarMessage: "이 런타임은 현재 연결된 etragon 사이드카 엔드포인트를 알리지 않습니다.",
        trustMeta: "상태 출처: {source} · 스냅샷: {snapshot}",
        trustRefreshStatus: "지금 상태 새로고침",
        trustRefreshSidecar: "지금 사이드카 새로고침",
        openExternal: "새 탭에서 열기",
        sources: { runtime: "Runtime", sidecar: "Sidecar" },
        views: { root: "홈", health: "상태", meta: "Latest Meta", summary: "요약", analysis: "분석", targets: "Targets", sidecarRoot: "Sidecar 홈", sidecarHealth: "Sidecar 상태", sidecarStatus: "Sidecar 상태", sidecarEnrichment: "보강", sidecarOpinion: "의견" },
    },
    register: {
        title: "런타임 등록",
        intake: "가벼운 intake",
        targetSection: "런타임 대상",
        targetSectionCopy: "이 컨트롤 플레인이 관리할 gewyvern 서비스를 지정합니다.",
        accessSection: "제어 접근",
        accessSectionCopy: "단기 토큰으로 페어링하며 토큰은 미리보기에 표시되지 않습니다.",
        sidecarSection: "선택적 사이드카",
        sidecarSectionCopy: "런타임이 보조 서비스를 제공할 때만 추가합니다.",
        placementSection: "배치 및 검색",
        placementSectionCopy: "선택적 태그는 ID를 바꾸지 않고 런타임 검색을 돕습니다.",
        showToken: "표시",
        hideToken: "숨기기",
        completeField: "계속하려면 {field}을(를) 입력하세요.",
        checkingPlan: "컨트롤 플레인에서 등록 계획을 확인하는 중...",
        planUnavailable: "등록 계획을 확인할 수 없습니다: {message}",
        ready: "등록 계획이 확인되었습니다. 이 런타임을 등록할 수 있습니다.",
        fixHighlighted: "등록하기 전에 강조된 필드를 확인하세요.",
        name: "이름",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "엔드포인트",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "사이드카 엔드포인트",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        pairingToken: "페어링 토큰",
        tokenPlaceholder: "token-123",
        fetchCapabilities: "등록 시 gewyvern 에서 capability 와 latest-meta 가져오기",
        submit: "런타임 등록",
        clear: "양식 지우기",
        untouched: "아직 제출된 런타임이 없습니다.",
        previewTitle: "실시간 미리보기",
        previewName: "이름",
        previewSlice: "슬라이스",
        previewEndpoint: "엔드포인트",
        previewSidecar: "사이드카",
        previewCapabilityFetch: "capability 가져오기",
        suggested: "추천",
        pendingRuntimeName: "대기 중인 런타임 이름",
        endpointPending: "대기 중",
        endpointValid: "유효",
        endpointInvalid: "무효",
        sidecarUnpaired: "연결 안 됨",
        capabilityEnabled: "활성화",
        capabilityDisabled: "비활성화",
        blockedEndpoint: "등록 차단: 엔드포인트는 http:// 또는 https:// 로 시작하고 유효한 URL 이어야 합니다.",
        blockedSidecarEndpoint: "등록 차단: 사이드카 엔드포인트는 http:// 또는 https:// 로 시작하고 유효한 URL 이어야 합니다.",
        blockedDuplicate: "등록 차단: {reason} 이(가) {name} ({endpoint}) 에 이미 존재합니다.",
        deletionInProgress: "이 런타임을 삭제하는 동안 등록이 일시 중지됩니다. 정리가 끝난 후 계획을 다시 확인하세요.",
        duplicateNameAndEndpoint: "이름과 엔드포인트",
        duplicateName: "이름",
        duplicateEndpoint: "엔드포인트",
        registering: "런타임 등록 중...",
        registered: "{name} ({runtimeId}) 를 {slice} 에 상태 {status} 로 등록했습니다.",
        failed: "등록 실패: {message}. 이 런타임이 이미 존재한다면 표에서 직접 선택하세요.",
        allRuntimes: "모든 런타임",
    },
    persistence: {
        title: "영속성",
        chip: "컨트롤 플레인 상태",
        yes: "예",
        no: "아니오",
        saveNow: "지금 저장",
        exportState: "상태 내보내기",
        importState: "상태 가져오기",
        stateFile: "상태 파일",
        lastSaved: "마지막 저장",
        enabled: "활성화",
        schema: "스키마",
        state: "상태",
        restoredRuntimes: "복구된 런타임",
        restoredSessions: "복구된 세션",
        statePath: "상태 파일 경로",
        backupPath: "백업 경로",
        schemaVersion: "스키마 버전",
        lastSavedAt: "마지막 저장 시각",
        lastSaveError: "마지막 저장 오류",
        restoredFromSave: "복구 원본",
        configured: "설정됨",
        missing: "없음",
        never: "없음",
        unknown: "알 수 없음",
        none: "없음",
        clean: "동기화됨",
        dirty: "변경 있음",
        saving: "컨트롤 플레인 상태 저장 중...",
        saved: "컨트롤 플레인 상태를 저장했습니다.",
        saveFailed: "저장 실패: {message}",
        exporting: "컨트롤 플레인 상태 내보내는 중...",
        exported: "컨트롤 플레인 상태를 내보냈습니다.",
        exportFailed: "내보내기 실패: {message}",
        importing: "{file} 에서 컨트롤 플레인 상태 가져오는 중...",
        invalidJson: "선택한 파일이 유효한 JSON 이 아닙니다",
        imported: "{runtimes} 개의 런타임과 {sessions} 개의 세션을 가져왔습니다.",
        importFailed: "가져오기 실패: {message}",
    },
    attention: {
        noReasons: "현재 활성 attention reason 이 없습니다.",
        reasonLine: "{reason} · {count} 런타임",
        noRuntimes: "이 슬라이스에는 지금 주의가 필요한 런타임이 없습니다.",
        critical: "심각",
        warning: "경고",
        statusFetchFailed: "status_fetch_failed",
        sidecarStatusFetchFailed: "sidecar_status_fetch_failed",
        noLatestSnapshot: "최신 스냅샷 없음",
        noAnalysisJson: "분석 JSON 없음",
    },
    sessions: { title: "세션", none: "아직 세션이 없습니다.", runtime: "runtime" },
    metrics: {
        runtimes: "런타임",
        latestSnapshots: "latest snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "sidecar context",
        diagnosticOpinions: "diagnostic opinions",
        pairedSidecars: "paired sidecars",
        healthySidecars: "healthy sidecars",
        critical: "심각",
        warning: "경고",
    },
    groups: {
        snapshotKinds: "스냅샷 종류",
        statusSources: "상태 출처",
        sidecarStatusSources: "사이드카 상태 출처",
        environments: "환경",
        clusters: "클러스터",
        roles: "역할",
        empty: "아직 그룹 데이터가 없습니다.",
    },
    statuses: {
        fetchFailed: "가져오기 실패",
        idleReady: "유휴 준비 완료",
        unobserved: "미관측",
        observedSnapshot: "{kind} 스냅샷",
        observed: "관측됨",
        sidecarObserved: "사이드카 관측됨",
        sidecarStarting: "사이드카 시작 중",
        sidecarDegraded: "사이드카 저하",
        sidecarFetchFailed: "사이드카 가져오기 실패",
    },
    notifications: {
        loading: "컨트롤 플레인 상태를 불러오는 중...",
        loaded: "{count} 개의 런타임을 불러왔습니다.",
        dashboardLoadFailed: "대시보드 로드 실패: {message}",
        noRuntimeSelected: "선택된 런타임이 없습니다.",
        runtimeLinkCopied: "런타임 링크를 복사했습니다.",
        runtimeLinkFailed: "링크 복사 실패: {message}",
        runtimeRefreshAll: "런타임 전체 새로고침",
        runtimeRefreshStatus: "런타임 상태 새로고침",
        runtimeRefreshCapabilities: "런타임 기능 새로고침",
        runtimeRefreshComplete: "{label} 완료.",
        runtimeRefreshFailed: "{label} 실패: {message}",
        fleetRefreshAll: "Fleet 전체 새로고침",
        fleetStatusRefresh: "Fleet 상태 새로고침",
        fleetCapabilityRefresh: "Fleet 기능 새로고침",
        fleetRefreshComplete: "{label} 완료.",
        fleetRefreshFailed: "{label} 실패: {message}",
        badgeUpdated: "업데이트됨",
    },
});
translations.ja = mergeTranslations(translations.en, {
    hero: {
        title: "コントロールプレーン ダッシュボード",
        subcopy: "近くにある複数の gewyvern runtime を見るための軽量 fleet ビューです。",
    },
    language: {
        label: "言語",
        auto: "ブラウザーに従う",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    theme: {
        label: "テーマ",
        auto: "システムに従う",
        light: "昼",
        dark: "夜",
    },
    security: {
        title: "セキュリティ",
        adminToken: "管理トークン",
        adminTokenPlaceholder: "リモート接続時のみ必要",
        clearToken: "トークンを消去",
        showToken: "表示",
        hideToken: "非表示",
        testToken: "トークンを確認",
        localModeHint: "現在はローカル loopback モードです。保護されたリモート入口から接続するときだけ管理トークンを入れてください。",
        tokenStored: "管理トークンをこのブラウザーに保存しました。",
        tokenCleared: "管理トークンを消去しました。",
        tokenRequired: "この操作はコントロールプレーンに拒否されました。リモート接続中なら設定済みの管理トークンを入れてください。",
        tokenTestRunning: "管理トークンを確認しています...",
        tokenTestOk: "この管理トークンは現在のコントロールプレーンで受け入れられました。",
        tokenTestFailed: "管理トークンの確認に失敗しました: {message}",
        tokenMissing: "先に管理トークンを入れてから確認してください。",
        lastTokenTest: "最後のトークン確認",
        neverTested: "未確認",
        testStateOk: "成功",
        testStateFailed: "失敗",
        testStateRunning: "確認中",
        mode: "セキュリティモード",
        tokenConfigured: "管理トークン",
        publicDiscovery: "公開 endpoint 発見",
        configured: "設定済み",
        notConfigured: "未設定",
        enabled: "有効",
        disabled: "無効",
        loopbackOnly: "ローカルのみ",
        tokenMode: "ローカルまたはトークン",
    },
    actions: {
        refreshAll: "Fleet 全体を更新",
        refreshStatus: "状態を更新",
        refreshCapabilities: "能力を更新",
    },
    filters: {
        title: "Fleet フィルター",
        environment: "環境",
        environmentPlaceholder: "prod",
        cluster: "クラスター",
        clusterPlaceholder: "alpha",
        role: "ロール",
        rolePlaceholder: "edge",
        apply: "フィルターを適用",
        clear: "クリア",
        allRuntimes: "すべての runtimes",
    },
    tabs: {
        overview: "概要",
        runtimes: "Runtimes",
        register: "登録",
        persistence: "永続化",
        sessions: "セッション",
    },
    overview: {
        fleetSummary: "Fleet サマリー",
        attentionSummary: "注意サマリー",
        triage: "トリアージ",
        runtimesNeedingAttention: "注意が必要な runtimes",
        kicker: "Fleet 状態",
        spotlightTitle: "まず全体像をつかんでから、個々の runtime に入っていきましょう。",
        spotlightBody: "この shell では fleet 状態、runtime 詳細、登録 intake、永続化、sessions を行き来できます。長い 1 ページスクロールにはなりません。",
        spotlightRail1: "現在のスライス",
        spotlightRail2: "現在のモード",
        summaryChip: "ライブ集計",
    },
    runtimes: {
        title: "Runtimes",
        workspaceTabs: {
            select: "選択",
            register: "登録",
            detail: "詳細",
            panel: "子パネル",
        },
        quickSearch: "クイック検索",
        quickSearchPlaceholder: "名前または endpoint",
        sortBy: "並び順",
        noMatch: "現在の絞り込みや検索に一致する runtime はありません。",
        columns: {
            name: "名前",
            tags: "タグ",
            status: "状態",
            capabilitySurface: "能力面",
            sidecar: "Sidecar",
            attention: "注意項目",
            actions: "操作",
        },
        sort: {
            name: "名前",
            status: "状態ソース",
            snapshot: "スナップショット種類",
        },
        actions: {
            attention: "注意を見る",
            status: "状態更新",
            all: "すべて更新",
        },
        states: {
            clear: "正常",
            none: "なし",
            noEnv: "環境なし",
            noCluster: "クラスターなし",
            noRole: "ロールなし",
            noCapabilities: "fully-supported な capability はありません",
        },
    },
    runtimeDetail: {
        title: "Runtime 詳細",
        nothingSelected: "未選択",
        empty: "表から runtime を選ぶと、capability、status、attention 状態を確認できます。",
        identity: "識別情報",
        status: "状態",
        capabilities: "能力",
        attention: "注意項目",
        refreshAll: "この runtime を更新",
        refreshStatus: "状態を更新",
        refreshCapabilities: "能力を更新",
        refreshSidecar: "Sidecar を更新",
        copyLink: "ランタイムリンクをコピー",
        registered: "登録時刻",
        updated: "更新時刻",
        source: "ソース",
        sidecarSource: "sidecar ソース",
        sidecarLearning: "sidecar 学習状態",
        resilienceStatus: "レジリエンス状態",
        resilienceSummary: "レジリエンス要約",
        socketServiceStatus: "socket サービス",
        idleTimeouts: "アイドルタイムアウト（現在 / 累計）",
        snapshotKind: "スナップショット種類",
        targetCount: "target 数",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        noCapabilities: "現在記録されている capability はありません。",
        clear: "正常",
        noAttention: "この runtime には現在 attention reason がありません。",
        needsAttention: "要注意かどうか",
        sectionsLabel: "ランタイム詳細セクション",
        liveSummary: "現在のランタイム状態",
        operational: "最新のコントロールプレーンスナップショットが利用でき、現在必要な操作はありません。",
        requiresReview: "このランタイムはオペレーターの確認が必要です。",
        refreshRecommended: "直近の更新に失敗しました。このスナップショットを利用する前に再更新してください。",
        reviewAttention: "注意項目を確認",
        inspectStatus: "状態を確認",
        lastObserved: "最終観測",
        notObserved: "未観測",
        runtimeId: "ランタイム ID",
        supportedCapabilities: "対応機能",
        fullySupportedCount: "{count} 件を完全対応",
        availableEvidence: "利用可能な証拠",
        availableCount: "{count} 件利用可能",
        attentionReasonCount: "アクティブな理由 {count} 件",
        statusOverview: "ランタイム状態",
        evidenceAvailability: "証拠の可用性",
        available: "利用可能",
        missing: "不足",
        sidecarOverview: "Sidecar 状態",
        capabilitySource: "機能の取得元",
        lastCapabilityRefresh: "最終機能更新",
        support: {
            fullySupported: "完全対応",
            partiallySupported: "一部対応",
            notSupported: "未対応",
            unsupported: "未対応",
            unknown: "不明",
        },
        none: "なし",
        na: "n/a",
    },
    runtimePanel: {
        windows: {
            openSelected: "選択項目を開く",
            openAll: "すべて開く",
            closeAll: "すべて閉じる",
            close: "閉じる",
            activate: "有効化",
            external: "新しいタブ",
            count: "{count} ウィンドウ",
            one: "1 ウィンドウ",
            capacity: "{count} / {limit} ウィンドウ",
            policy: "アクティブなパネルだけを接続し、非アクティブなパネルは一時停止します。",
            pausedTitle: "パネルは一時停止中です",
            pausedBody: "このウィンドウを有効化するとリモートパネルを読み込みます。非アクティブなウィンドウはブラウザと runtime のリソースを守るため一時停止します。",
            pausedAction: "パネルを有効化",
            limitReached: "ワークスペースは {limit} ウィンドウの上限に達しました。先にウィンドウを閉じてください。",
            openAllLimited: "{total} runtime のうち {count} 件を開きました。ワークスペースの上限は {limit} ウィンドウです。",
            openAllComplete: "{count} 件の runtime ウィンドウを準備しました。",
            workspaceLabel: "Runtime ウィンドウワークスペース",
            windowLabel: "{name} · {view}",
        },
        title: "Runtime 子パネル",
        notReady: "runtime が未選択です",
        empty: "runtime を選択すると、その子コントロール画面を読み込めます。",
        compactTrustObserved: "runtime のライブスナップショットを利用できます。",
        compactTrustIdleReady: "runtime は現在アイドルですが健全で、接続を受けられます。",
        compactTrustUnobserved: "最初の runtime スナップショットを待っています。",
        compactTrustFetchFailed: "直近の runtime 更新に失敗したため、現在の情報は古い可能性があります。",
        compactTrustSidecarObserved: "sidecar のライブスナップショットを利用できます。",
        compactTrustSidecarUnobserved: "最初の sidecar スナップショットを待っています。",
        compactTrustSidecarFetchFailed: "直近の sidecar 更新に失敗したため、現在の情報は古い可能性があります。",
        compactTrustNoSidecar: "paired sidecar は設定されていません。",
        blankRuntimeTitle: "この runtime パネルはまだ準備できていません",
        blankRuntimeBody: "この runtime の場所は分かっていますが、パネル表示に適したデータがまだ公開されていません。いまは上のコントロールプレーン要約を基準にする方が安全です。",
        blankSidecarTitle: "この sidecar パネルはまだ準備できていません",
        blankSidecarBody: "診断 sidecar は設定されていますが、安定して表示できる状態スナップショットがまだありません。まずはコントロールプレーン表示を見て、必要に応じて更新してください。",
        blankFetchFailedTitle: "このパネルはまだ安全に開けません",
        blankFetchFailedBody: "最新の状態更新に失敗したため、今ここで生の endpoint を見せると、役に立つより混乱を招く可能性があります。先に更新してください。",
        blankHintRefreshRuntime: "まず runtime 状態を更新してください。",
        blankHintRefreshSidecar: "まず sidecar 状態を更新してください。",
        blankHintEndpoint: "endpoint",
        breadcrumbFleet: "runtime パネル",
        breadcrumbSource: "ソース",
        breadcrumbView: "ビュー",
        currentView: "現在のビュー",
        sourceUrl: "ソース URL",
        sourceStatus: "ソース状態",
        trustTitle: "信頼状態",
        trustObserved: "runtime 状態を観測済み",
        trustObservedMessage: "この子パネルの背後には利用可能な gewyvern latest snapshot があり、通常はライブの runtime コンテキストとして読めます。",
        trustIdleReady: "runtime はアイドル待機中",
        trustIdleReadyMessage: "この runtime は現在アイドルですが健全です。まだ新しい payload snapshot は生成していませんが、socket サービスは生きており次の接続を受ける準備ができています。",
        trustUnobserved: "runtime はまだ未観測",
        trustUnobservedMessage: "この runtime はまだ latest snapshot を公開していないため、状態が届くまでは子パネルを薄い endpoint シェルとして扱うのが適切です。",
        trustFetchFailed: "状態取得に失敗",
        trustFetchFailedMessage: "コントロールプレーンがこの runtime の状態更新に失敗したため、右側の子パネルは古いか、到達不能の可能性があります。",
        trustSidecarObserved: "Sidecar 状態を観測済み",
        trustSidecarObservedMessage: "この子パネルの背後では paired etragon sidecar に到達できており、近接診断パートナーのライブコンテキストとして読めます。",
        trustSidecarUnobserved: "Sidecar はまだ未観測",
        trustSidecarUnobservedMessage: "この runtime には paired etragon endpoint が設定されていますが、コントロールプレーンは sidecar の状態スナップショットをまだ取得していません。",
        trustSidecarFetchFailed: "Sidecar 状態取得に失敗",
        trustSidecarFetchFailedMessage: "コントロールプレーンが paired etragon sidecar の更新に失敗したため、sidecar 子パネルは古いか、到達不能の可能性があります。",
        trustNoSidecar: "paired sidecar なし",
        trustNoSidecarMessage: "この runtime には現在 etragon sidecar endpoint が設定されていません。",
        trustMeta: "状態ソース: {source} · スナップショット: {snapshot}",
        trustRefreshStatus: "今すぐ状態を更新",
        trustRefreshSidecar: "今すぐ Sidecar を更新",
        openExternal: "新しいタブで開く",
        sources: {
            runtime: "Runtime",
            sidecar: "Sidecar",
        },
        views: {
            root: "ホーム",
            health: "Health",
            meta: "Latest Meta",
            summary: "Summary",
            analysis: "Analysis",
            targets: "Targets",
            sidecarRoot: "Sidecar ホーム",
            sidecarHealth: "Sidecar Health",
            sidecarStatus: "Sidecar Status",
            sidecarEnrichment: "Enrichment",
            sidecarOpinion: "Opinion",
        },
    },
    register: {
        title: "Runtime を登録",
        intake: "軽量 intake",
        targetSection: "Runtime の接続先",
        targetSectionCopy: "このコントロールプレーンが管理する gewyvern サービスを指定します。",
        accessSection: "制御アクセス",
        accessSectionCopy: "短期トークンでペアリングします。トークンはプレビューに表示されません。",
        sidecarSection: "任意の Sidecar",
        sidecarSectionCopy: "runtime が関連サービスを公開している場合のみ追加します。",
        placementSection: "配置と検出",
        placementSectionCopy: "任意のタグで、ID を変えずに runtime を見つけやすくします。",
        showToken: "表示",
        hideToken: "隠す",
        completeField: "続行するには {field} を入力してください。",
        checkingPlan: "コントロールプレーンで登録プランを確認しています...",
        planUnavailable: "登録プランを確認できませんでした: {message}",
        ready: "登録プランを確認しました。この runtime を登録できます。",
        fixHighlighted: "登録前に強調表示された項目を確認してください。",
        name: "名前",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "Endpoint",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "Sidecar Endpoint",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        pairingToken: "ペアリングトークン",
        tokenPlaceholder: "token-123",
        fetchCapabilities: "登録時に gewyvern から capability と latest-meta を取得する",
        submit: "Runtime を登録",
        clear: "フォームをクリア",
        untouched: "まだ runtime は送信されていません。",
        previewTitle: "ライブプレビュー",
        previewName: "名前",
        previewSlice: "対象スライス",
        previewEndpoint: "endpoint",
        previewSidecar: "sidecar",
        previewCapabilityFetch: "能力取得",
        suggested: "提案",
        pendingRuntimeName: "runtime 名の提案待ち",
        endpointPending: "未入力",
        endpointValid: "有効",
        endpointInvalid: "無効",
        sidecarUnpaired: "未ペア",
        capabilityEnabled: "有効",
        capabilityDisabled: "無効",
        blockedDuplicate: "登録はブロックされました: {reason} はすでに {name} ({endpoint}) に存在します。",
        deletionInProgress: "このランタイムの削除中は登録が一時停止されます。クリーンアップ完了後にプランを再確認してください。",
        duplicateNameAndEndpoint: "名前と endpoint",
        duplicateName: "名前",
        duplicateEndpoint: "endpoint",
        registering: "runtime を登録しています...",
        blockedEndpoint: "登録はブロックされました: endpoint は http:// または https:// で始まる有効な URL である必要があります。",
        blockedSidecarEndpoint: "登録はブロックされました: sidecar endpoint は http:// または https:// で始まる有効な URL である必要があります。",
        registered: "{name} ({runtimeId}) を {slice} に登録しました。現在の状態は {status} です。",
        failed: "登録に失敗しました: {message}。すでに存在する runtime なら、表から選んでください。",
        allRuntimes: "すべての runtimes",
    },
    persistence: {
        title: "永続化",
        chip: "コントロールプレーン状態",
        yes: "はい",
        no: "いいえ",
        saveNow: "今すぐ保存",
        exportState: "状態をエクスポート",
        importState: "状態をインポート",
        stateFile: "状態ファイル",
        lastSaved: "最後の保存",
        enabled: "有効",
        schema: "schema",
        state: "状態",
        restoredRuntimes: "復元 runtimes",
        restoredSessions: "復元 sessions",
        statePath: "状態ファイルのパス",
        backupPath: "バックアップパス",
        schemaVersion: "schema バージョン",
        lastSavedAt: "最後の保存時刻",
        lastSaveError: "最後の保存エラー",
        restoredFromSave: "復元元",
        configured: "設定済み",
        missing: "不足",
        never: "なし",
        unknown: "不明",
        none: "なし",
        clean: "同期済み",
        dirty: "変更あり",
        saving: "コントロールプレーン状態を保存しています...",
        saved: "コントロールプレーン状態を保存しました。",
        saveFailed: "保存に失敗しました: {message}",
        exporting: "コントロールプレーン状態をエクスポートしています...",
        exported: "コントロールプレーン状態をエクスポートしました。",
        exportFailed: "エクスポートに失敗しました: {message}",
        importing: "{file} からコントロールプレーン状態をインポートしています...",
        invalidJson: "選択したファイルは有効な JSON ではありません",
        imported: "{runtimes} 件の runtime と {sessions} 件の session をインポートしました。",
        importFailed: "インポートに失敗しました: {message}",
    },
    attention: {
        noReasons: "有効な attention reason はありません。",
        reasonLine: "{reason} · {count} runtimes",
        noRuntimes: "このスライスには注意が必要な runtime はありません。",
        critical: "重大",
        warning: "警告",
        statusFetchFailed: "状態取得失敗",
        sidecarStatusFetchFailed: "sidecar 状態取得失敗",
        noLatestSnapshot: "最新スナップショットなし",
        noAnalysisJson: "分析 JSON なし",
    },
    sessions: {
        title: "セッション",
        none: "まだ session はありません。",
        runtime: "runtime",
    },
    metrics: {
        runtimes: "runtimes",
        latestSnapshots: "latest snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "sidecar context",
        diagnosticOpinions: "diagnostic opinions",
        pairedSidecars: "paired sidecars",
        healthySidecars: "healthy sidecars",
        critical: "重大",
        warning: "警告",
    },
    groups: {
        snapshotKinds: "スナップショット種類",
        statusSources: "状態ソース",
        sidecarStatusSources: "sidecar 状態ソース",
        environments: "環境",
        clusters: "クラスター",
        roles: "ロール",
        empty: "まだグループ化データはありません。",
    },
    statuses: {
        fetchFailed: "取得失敗",
        idleReady: "アイドル待機中",
        unobserved: "未観測",
        observedSnapshot: "{kind} スナップショット",
        observed: "観測済み",
        sidecarObserved: "sidecar 観測済み",
        sidecarStarting: "sidecar 起動中",
        sidecarDegraded: "sidecar 劣化中",
        sidecarFetchFailed: "sidecar 取得失敗",
    },
    notifications: {
        loading: "コントロールプレーン状態を読み込んでいます...",
        loaded: "{count} 件の runtime を読み込みました。",
        dashboardLoadFailed: "ダッシュボードの読み込みに失敗しました: {message}",
        noRuntimeSelected: "runtime が選択されていません。",
        runtimeLinkCopied: "runtime リンクをコピーしました。",
        runtimeLinkFailed: "リンクのコピーに失敗しました: {message}",
        runtimeRefreshAll: "この runtime の全更新",
        runtimeRefreshStatus: "この runtime の状態更新",
        runtimeRefreshCapabilities: "この runtime の能力更新",
        runtimeRefreshComplete: "{label} が完了しました。",
        runtimeRefreshFailed: "{label} に失敗しました: {message}",
        fleetRefreshAll: "fleet 全体更新",
        fleetStatusRefresh: "fleet 状態更新",
        fleetCapabilityRefresh: "fleet 能力更新",
        fleetRefreshComplete: "{label} が完了しました。",
        fleetRefreshFailed: "{label} に失敗しました: {message}",
        badgeUpdated: "更新済み",
    },
});
translations.es = mergeTranslations(translations.en, {
    hero: {
        title: "Panel de Control",
        subcopy: "Una vista ligera del fleet para muchos runtimes gewyvern cercanos.",
    },
    language: {
        label: "Idioma",
        auto: "Seguir navegador",
        english: "English",
        simplifiedChinese: "简体中文",
        traditionalChinese: "繁體中文",
        japanese: "日本語",
        spanish: "Español",
        german: "Deutsch",
        french: "Français",
        korean: "한국어",
    },
    theme: {
        label: "Tema",
        auto: "Seguir sistema",
        light: "Día",
        dark: "Noche",
    },
    security: {
        title: "Seguridad",
        adminToken: "Token de administrador",
        adminTokenPlaceholder: "opcional para acceso remoto",
        clearToken: "Borrar token",
        showToken: "Mostrar token",
        hideToken: "Ocultar token",
        testToken: "Probar token",
        localModeHint: "El modo loopback local está activo. Agrega un token solo si te conectas intencionalmente mediante un endpoint remoto protegido.",
        tokenStored: "El token de administrador se guardó en este navegador.",
        tokenCleared: "El token de administrador se borró.",
        tokenRequired: "Este control plane rechazó la solicitud. Si te conectas en remoto, agrega el token de administrador configurado.",
        tokenTestRunning: "Probando token de administrador...",
        tokenTestOk: "El token fue aceptado por el control plane actual.",
        tokenTestFailed: "La prueba del token falló: {message}",
        tokenMissing: "Agrega primero un token y luego pruébalo.",
        lastTokenTest: "Última prueba del token",
        neverTested: "nunca probado",
        testStateOk: "ok",
        testStateFailed: "falló",
        testStateRunning: "probando",
        mode: "modo de seguridad",
        tokenConfigured: "token admin",
        publicDiscovery: "descubrimiento público",
        configured: "configurado",
        notConfigured: "no configurado",
        enabled: "habilitado",
        disabled: "deshabilitado",
        loopbackOnly: "solo loopback",
        tokenMode: "loopback o token",
    },
    actions: {
        refreshAll: "Actualizar todo el fleet",
        refreshStatus: "Actualizar estado",
        refreshCapabilities: "Actualizar capacidades",
    },
    filters: {
        title: "Filtros de flota",
        environment: "Entorno",
        environmentPlaceholder: "prod",
        cluster: "Clúster",
        clusterPlaceholder: "alpha",
        role: "Rol",
        rolePlaceholder: "edge",
        apply: "Aplicar filtros",
        clear: "Limpiar",
        allRuntimes: "todos los runtimes",
    },
    tabs: {
        overview: "Resumen",
        runtimes: "Runtimes",
        register: "Registro",
        persistence: "Persistencia",
        sessions: "Sesiones",
    },
    overview: {
        fleetSummary: "Resumen del fleet",
        attentionSummary: "Resumen de atención",
        triage: "triaje",
        runtimesNeedingAttention: "Runtimes que necesitan atención",
        kicker: "Postura del fleet",
        spotlightTitle: "Primero entiende el panorama general y luego entra en cada runtime.",
        spotlightBody: "Este shell te deja moverte entre la postura del fleet, el detalle del runtime, el intake de registro, la persistencia y las sesiones sin caer en una página larguísima.",
        spotlightRail1: "slice actual",
        spotlightRail2: "modo actual",
        summaryChip: "conteos en vivo",
    },
    runtimes: {
        title: "Runtimes",
        workspaceTabs: {
            select: "Seleccionar",
            register: "Registrar",
            detail: "Detalle",
            panel: "Panel hijo",
        },
        quickSearch: "Búsqueda rápida",
        quickSearchPlaceholder: "nombre o endpoint",
        sortBy: "Ordenar por",
        noMatch: "No hay runtimes que coincidan con el filtro o la búsqueda actual.",
        columns: {
            name: "Nombre",
            tags: "Etiquetas",
            status: "Estado",
            capabilitySurface: "Superficie de capacidad",
            sidecar: "Sidecar",
            attention: "Atención",
            actions: "Acciones",
        },
        sort: {
            name: "Nombre",
            status: "Fuente de estado",
            snapshot: "Tipo de snapshot",
        },
        actions: {
            attention: "Atención",
            status: "Estado",
            all: "Todo",
        },
        states: {
            clear: "ok",
            none: "ninguno",
            noEnv: "sin entorno",
            noCluster: "sin clúster",
            noRole: "sin rol",
            noCapabilities: "Sin capacidades fully-supported",
        },
    },
    runtimeDetail: {
        title: "Detalle del runtime",
        nothingSelected: "nada seleccionado",
        empty: "Selecciona un runtime de la tabla para inspeccionar su capacidad, estado y atención.",
        identity: "Identidad",
        status: "Estado",
        capabilities: "Capacidades",
        attention: "Atención",
        refreshAll: "Actualizar este runtime",
        refreshStatus: "Actualizar estado",
        refreshCapabilities: "Actualizar capacidades",
        refreshSidecar: "Actualizar Sidecar",
        copyLink: "Copiar enlace",
        registered: "registrado",
        updated: "actualizado",
        source: "fuente",
        sidecarSource: "fuente del sidecar",
        sidecarLearning: "aprendizaje del sidecar",
        resilienceStatus: "estado de resiliencia",
        resilienceSummary: "resumen de resiliencia",
        socketServiceStatus: "servicio socket",
        idleTimeouts: "timeouts en reposo (actual / total)",
        snapshotKind: "tipo de snapshot",
        targetCount: "cantidad de targets",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        noCapabilities: "No hay capacidades registradas.",
        clear: "ok",
        noAttention: "Este runtime no tiene attention reasons en este momento.",
        needsAttention: "necesita atención",
        sectionsLabel: "Secciones de detalle del runtime",
        liveSummary: "Estado actual del runtime",
        operational: "La instantánea más reciente del plano de control está disponible y no requiere acciones.",
        requiresReview: "Este runtime requiere revisión del operador.",
        refreshRecommended: "La última actualización falló. Actualiza antes de confiar en esta instantánea.",
        reviewAttention: "Revisar alertas",
        inspectStatus: "Ver estado",
        lastObserved: "Última observación",
        notObserved: "No observado",
        runtimeId: "ID del runtime",
        supportedCapabilities: "Capacidades compatibles",
        fullySupportedCount: "{count} totalmente compatibles",
        availableEvidence: "Evidencia disponible",
        availableCount: "{count} disponibles",
        attentionReasonCount: "{count} motivos activos",
        statusOverview: "Estado del runtime",
        evidenceAvailability: "Disponibilidad de evidencia",
        available: "Disponible",
        missing: "Falta",
        sidecarOverview: "Estado del sidecar",
        capabilitySource: "Origen de capacidades",
        lastCapabilityRefresh: "Última actualización de capacidades",
        support: {
            fullySupported: "Totalmente compatible",
            partiallySupported: "Parcialmente compatible",
            notSupported: "No compatible",
            unsupported: "No compatible",
            unknown: "Desconocido",
        },
        none: "ninguno",
        na: "n/a",
    },
    runtimePanel: {
        windows: {
            openSelected: "Abrir selección",
            openAll: "Abrir todo",
            closeAll: "Cerrar todo",
            close: "Cerrar",
            activate: "Activar",
            external: "Nueva pestaña",
            count: "{count} ventanas",
            one: "1 ventana",
            capacity: "{count} / {limit} ventanas",
            policy: "Solo el panel activo permanece en vivo; los demás quedan en pausa.",
            pausedTitle: "Panel en pausa",
            pausedBody: "Activa esta ventana para cargar su panel remoto. Las ventanas inactivas permanecen en pausa para proteger los recursos del navegador y del runtime.",
            pausedAction: "Activar panel",
            limitReached: "El espacio de trabajo alcanzó su límite de {limit} ventanas. Cierra una antes de abrir otra.",
            openAllLimited: "Se abrieron {count} de {total} runtimes. El espacio de trabajo está limitado a {limit} ventanas.",
            openAllComplete: "Se prepararon {count} ventanas de runtime.",
            workspaceLabel: "Espacio de trabajo de ventanas runtime",
            windowLabel: "{name} · {view}",
        },
        title: "Panel hijo del runtime",
        notReady: "ningún runtime seleccionado",
        empty: "Selecciona un runtime para cargar su panel hijo.",
        compactTrustObserved: "Hay un snapshot en vivo del runtime.",
        compactTrustIdleReady: "La runtime está inactiva pero sana y lista.",
        compactTrustUnobserved: "Esperando el primer snapshot del runtime.",
        compactTrustFetchFailed: "La última actualización del runtime falló; trata estos datos como posiblemente obsoletos.",
        compactTrustSidecarObserved: "Hay un snapshot en vivo del sidecar.",
        compactTrustSidecarUnobserved: "Esperando el primer snapshot del sidecar.",
        compactTrustSidecarFetchFailed: "La última actualización del sidecar falló; trata estos datos como posiblemente obsoletos.",
        compactTrustNoSidecar: "No hay un sidecar emparejado configurado.",
        blankRuntimeTitle: "Este panel de runtime todavía no está listo",
        blankRuntimeBody: "Sabemos dónde vive este runtime, pero aún no ha publicado datos listos para mostrar en el panel. Por ahora es más seguro usar el resumen del control plane de arriba.",
        blankSidecarTitle: "Este panel de sidecar todavía no está listo",
        blankSidecarBody: "El sidecar de diagnóstico está configurado, pero todavía no ha producido un snapshot estable para mostrar. Primero mira la vista del control plane y actualiza cuando haga falta.",
        blankFetchFailedTitle: "Todavía no podemos abrir este panel con seguridad",
        blankFetchFailedBody: "La actualización más reciente de estado falló, así que mostrar el endpoint crudo ahora mismo podría confundir más de lo que ayuda. Primero actualiza y luego vuelve a intentarlo.",
        blankHintRefreshRuntime: "Prueba primero a actualizar el estado del runtime.",
        blankHintRefreshSidecar: "Prueba primero a actualizar el estado del sidecar.",
        blankHintEndpoint: "endpoint",
        breadcrumbFleet: "panel del runtime",
        breadcrumbSource: "fuente",
        breadcrumbView: "vista",
        currentView: "vista actual",
        sourceUrl: "URL de origen",
        sourceStatus: "estado de origen",
        trustTitle: "nivel de confianza",
        trustObserved: "estado del runtime observado",
        trustObservedMessage: "Este panel hijo ya está respaldado por un latest snapshot utilizable de gewyvern, así que normalmente puedes leerlo como contexto vivo del runtime.",
        trustIdleReady: "Runtime lista en reposo",
        trustIdleReadyMessage: "Esta runtime está actualmente inactiva pero sana. Aún no produjo un nuevo snapshot de payload, pero el servicio socket está vivo y listo para la próxima conexión.",
        trustUnobserved: "runtime sin observar",
        trustUnobservedMessage: "Este runtime todavía no ha publicado latest snapshot, así que hasta que llegue estado es mejor tratar el panel hijo como una cáscara ligera del endpoint.",
        trustFetchFailed: "falló la obtención del estado",
        trustFetchFailedMessage: "El control plane no pudo actualizar el estado de este runtime, así que el panel hijo de la derecha puede estar obsoleto o incluso inaccesible.",
        trustSidecarObserved: "estado del sidecar observado",
        trustSidecarObservedMessage: "Detrás de este panel hijo ya se puede alcanzar el sidecar emparejado de etragon, así que puede leerse como contexto vivo del compañero de diagnóstico.",
        trustSidecarUnobserved: "sidecar sin observar",
        trustSidecarUnobservedMessage: "Este runtime tiene configurado un endpoint de etragon emparejado, pero el control plane todavía no ha obtenido un snapshot de estado del sidecar.",
        trustSidecarFetchFailed: "falló la obtención del sidecar",
        trustSidecarFetchFailedMessage: "El control plane no pudo actualizar el sidecar emparejado de etragon, así que el panel hijo del sidecar puede estar obsoleto o inaccesible.",
        trustNoSidecar: "sin sidecar emparejado",
        trustNoSidecarMessage: "Este runtime actualmente no tiene configurado un endpoint de sidecar etragon.",
        trustMeta: "fuente del estado: {source} · snapshot: {snapshot}",
        trustRefreshStatus: "Actualizar estado ahora",
        trustRefreshSidecar: "Actualizar Sidecar ahora",
        openExternal: "Abrir en nueva pestaña",
        sources: {
            runtime: "Runtime",
            sidecar: "Sidecar",
        },
        views: {
            root: "Inicio",
            health: "Salud",
            meta: "Latest Meta",
            summary: "Resumen",
            analysis: "Análisis",
            targets: "Targets",
            sidecarRoot: "Inicio Sidecar",
            sidecarHealth: "Salud Sidecar",
            sidecarStatus: "Estado Sidecar",
            sidecarEnrichment: "Enriquecimiento",
            sidecarOpinion: "Opinión",
        },
    },
    register: {
        title: "Registrar runtime",
        intake: "intake ligero",
        targetSection: "Runtime de destino",
        targetSectionCopy: "Identifica el servicio gewyvern que gestionará este control plane.",
        accessSection: "Acceso de control",
        accessSectionCopy: "Empareja con un token temporal; nunca aparece en la vista previa.",
        sidecarSection: "Sidecar opcional",
        sidecarSectionCopy: "Añádelo solo cuando el runtime exponga un servicio complementario.",
        placementSection: "Ubicación y descubrimiento",
        placementSectionCopy: "Las etiquetas opcionales facilitan encontrarlo sin cambiar su identidad.",
        showToken: "Mostrar",
        hideToken: "Ocultar",
        completeField: "Completa {field} para continuar.",
        checkingPlan: "Comprobando el plan de registro con el control plane...",
        planUnavailable: "No se pudo verificar el plan de registro: {message}",
        ready: "Plan de registro verificado. El runtime está listo para registrarse.",
        fixHighlighted: "Revisa el campo resaltado antes de registrar el runtime.",
        name: "Nombre",
        namePlaceholder: "gw-prod-edge-01",
        endpoint: "Endpoint",
        endpointPlaceholder: "http://127.0.0.1:9910",
        sidecarEndpoint: "Endpoint del sidecar",
        sidecarEndpointPlaceholder: "http://127.0.0.1:4321",
        pairingToken: "Token de emparejamiento",
        tokenPlaceholder: "token-123",
        submit: "Registrar runtime",
        clear: "Limpiar formulario",
        previewTitle: "Vista previa en vivo",
        untouched: "Todavía no se ha enviado ningún runtime.",
        previewName: "nombre",
        previewSlice: "slice",
        previewEndpoint: "endpoint",
        previewSidecar: "sidecar",
        previewCapabilityFetch: "obtención de capability",
        suggested: "sugerido",
        pendingRuntimeName: "nombre del runtime pendiente",
        endpointPending: "pendiente",
        endpointValid: "válido",
        endpointInvalid: "inválido",
        sidecarUnpaired: "sin emparejar",
        capabilityEnabled: "habilitado",
        capabilityDisabled: "deshabilitado",
        fetchCapabilities: "Obtener capability y latest-meta desde gewyvern al registrar",
        blockedEndpoint: "Registro bloqueado: el endpoint debe empezar con http:// o https:// y ser una URL válida.",
        blockedSidecarEndpoint: "Registro bloqueado: el endpoint del sidecar debe empezar con http:// o https:// y ser una URL válida.",
        blockedDuplicate: "Registro bloqueado: {reason} ya existe en {name} ({endpoint}).",
        deletionInProgress: "El registro está en pausa mientras se elimina este runtime. Espere a que termine la limpieza y vuelva a revisar el plan.",
        duplicateNameAndEndpoint: "nombre y endpoint",
        duplicateName: "nombre",
        duplicateEndpoint: "endpoint",
        registering: "Registrando runtime...",
        registered: "Se registró {name} ({runtimeId}) en {slice} con estado {status}.",
        failed: "Error al registrar: {message}. Si este runtime ya existe, selecciónalo desde la tabla.",
        allRuntimes: "todos los runtimes",
    },
    persistence: {
        title: "Persistencia",
        chip: "estado del control plane",
        yes: "sí",
        no: "no",
        saveNow: "Guardar ahora",
        exportState: "Exportar estado",
        importState: "Importar estado",
        stateFile: "archivo de estado",
        lastSaved: "último guardado",
        enabled: "habilitado",
        schema: "schema",
        state: "estado",
        restoredRuntimes: "runtimes restaurados",
        restoredSessions: "sesiones restauradas",
        statePath: "ruta del archivo de estado",
        backupPath: "ruta de respaldo",
        schemaVersion: "versión de schema",
        lastSavedAt: "último guardado en",
        lastSaveError: "último error de guardado",
        restoredFromSave: "restaurado desde",
        configured: "configurado",
        missing: "faltante",
        never: "nunca",
        unknown: "desconocido",
        none: "ninguno",
        clean: "sin cambios",
        dirty: "con cambios",
        saving: "Guardando el estado del control plane...",
        saved: "Estado del control plane guardado.",
        saveFailed: "Falló Guardar ahora: {message}",
        exporting: "Exportando el estado del control plane...",
        exported: "Estado del control plane exportado.",
        exportFailed: "Falló la exportación: {message}",
        importing: "Importando el estado del control plane desde {file}...",
        invalidJson: "el archivo seleccionado no es JSON válido",
        imported: "Se importaron {runtimes} runtimes y {sessions} sesiones.",
        importFailed: "Falló la importación: {message}",
    },
    attention: {
        noReasons: "No hay attention reasons activas.",
        reasonLine: "{reason} · {count} runtimes",
        noRuntimes: "No hay runtimes que requieran atención en este slice.",
        critical: "crítico",
        warning: "advertencia",
        statusFetchFailed: "status_fetch_failed",
        sidecarStatusFetchFailed: "sidecar_status_fetch_failed",
        noLatestSnapshot: "sin instantánea reciente",
        noAnalysisJson: "sin JSON de análisis",
    },
    sessions: {
        title: "Sesiones",
        none: "Todavía no hay sesiones.",
        runtime: "runtime",
    },
    metrics: {
        runtimes: "runtimes",
        latestSnapshots: "latest snapshots",
        summaryJson: "summary json",
        analysisJson: "analysis json",
        sidecarContext: "sidecar context",
        diagnosticOpinions: "diagnostic opinions",
        pairedSidecars: "paired sidecars",
        healthySidecars: "healthy sidecars",
        critical: "crítico",
        warning: "advertencia",
    },
    groups: {
        snapshotKinds: "tipos de snapshot",
        statusSources: "fuentes de estado",
        sidecarStatusSources: "fuentes de estado del sidecar",
        environments: "entornos",
        clusters: "clústeres",
        roles: "roles",
        empty: "Todavía no hay datos agrupados.",
    },
    statuses: {
        fetchFailed: "falló la obtención",
        idleReady: "lista en reposo",
        unobserved: "sin observar",
        observedSnapshot: "snapshot {kind}",
        observed: "observado",
        sidecarObserved: "sidecar observado",
        sidecarStarting: "sidecar iniciando",
        sidecarDegraded: "sidecar degradado",
        sidecarFetchFailed: "falló sidecar",
    },
    notifications: {
        loading: "Cargando el estado del control plane...",
        loaded: "Se cargaron {count} runtimes.",
        dashboardLoadFailed: "Falló la carga del panel: {message}",
        noRuntimeSelected: "No hay runtime seleccionado.",
        runtimeLinkCopied: "Enlace del runtime copiado.",
        runtimeLinkFailed: "Falló al copiar el enlace: {message}",
        runtimeRefreshAll: "actualización completa del runtime",
        runtimeRefreshStatus: "actualización de estado del runtime",
        runtimeRefreshCapabilities: "actualización de capacidades del runtime",
        runtimeRefreshComplete: "{label} completada.",
        runtimeRefreshFailed: "{label} falló: {message}",
        fleetRefreshAll: "actualización total del fleet",
        fleetStatusRefresh: "actualización de estado del fleet",
        fleetCapabilityRefresh: "actualización de capacidades del fleet",
        fleetRefreshComplete: "{label} completada.",
        fleetRefreshFailed: "{label} falló: {message}",
        badgeUpdated: "actualizado",
    },
});
const languagePackSchema = "leserpent.language-pack/v1";
const languagePackCatalogUrl = "/language-packs/catalog.json";
const builtinLanguageLocales = new Set(["en", "zh-CN", "zh-TW", "ja", "es", "de", "fr", "ko"]);
const languagePackLimits = {
    bytes: 256 * 1024,
    catalogBytes: 128 * 1024,
    catalogPacks: 64,
    installedBytes: 512 * 1024,
    packs: 12,
    depth: 12,
    nodes: 2000,
    stringLength: 4000,
};
function languagePackError(message) {
    throw new Error(message);
}
function validLanguagePackText(value, field, maxLength = 120) {
    if (typeof value !== "string" || !value.trim() || value.length > maxLength || /[\u0000-\u001f\u007f]/.test(value)) {
        languagePackError(`${field} is invalid`);
    }
    return value.trim();
}
function validateLanguagePackTranslations(value, depth = 0, budget = { nodes: 0 }) {
    if (!value || typeof value !== "object" || Array.isArray(value) || depth > languagePackLimits.depth) {
        languagePackError("translations must be a bounded object tree");
    }
    const result = {};
    for (const [key, item] of Object.entries(value)) {
        budget.nodes += 1;
        if (budget.nodes > languagePackLimits.nodes || !/^[A-Za-z0-9_-]+$/.test(key) || ["__proto__", "prototype", "constructor"].includes(key)) {
            languagePackError("translations contains an invalid key or too many entries");
        }
        if (typeof item === "string") {
            if (item.length > languagePackLimits.stringLength || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(item)) {
                languagePackError(`translation '${key}' is invalid`);
            }
            result[key] = item;
        }
        else {
            result[key] = validateLanguagePackTranslations(item, depth + 1, budget);
        }
    }
    return result;
}
function validateLanguagePack(pack, { allowBuiltin = false } = {}) {
    if (!pack || typeof pack !== "object" || Array.isArray(pack) || pack.schema !== languagePackSchema) {
        languagePackError(`schema must be '${languagePackSchema}'`);
    }
    const locale = validLanguagePackText(pack.locale, "locale", 35);
    if (!/^[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*$/.test(locale)) {
        languagePackError("locale must be a BCP 47-style language tag");
    }
    if (!allowBuiltin && builtinLanguageLocales.has(locale)) {
        languagePackError("built-in locales cannot be replaced by downloadable packs");
    }
    return {
        schema: languagePackSchema,
        locale,
        name: validLanguagePackText(pack.name, "name"),
        nativeName: validLanguagePackText(pack.nativeName, "nativeName"),
        version: validLanguagePackText(pack.version, "version", 40),
        author: pack.author ? validLanguagePackText(pack.author, "author") : "Leserpent community",
        direction: pack.direction === "rtl" ? "rtl" : "ltr",
        coverage: pack.coverage === "core-ui" ? "core-ui" : "partial",
        translations: validateLanguagePackTranslations(pack.translations),
    };
}
function serializeLanguagePack(pack) {
    return `${JSON.stringify(pack, null, 2)}\n`;
}
async function sha256Hex(text) {
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}
function registerInstalledLanguagePack(pack) {
    translations[pack.locale] = mergeTranslations(translations.en, pack.translations);
}
function persistLanguagePacks() {
    const serialized = JSON.stringify(state.installedLanguagePacks);
    if (new TextEncoder().encode(serialized).byteLength > languagePackLimits.installedBytes) {
        languagePackError("installed language packs exceed the browser storage limit");
    }
    window.localStorage.setItem(storageKeys.languagePacks, serialized);
}
function safeCatalogPackUrl(value) {
    try {
        const url = new URL(value, window.location.origin);
        return url.origin === window.location.origin && url.pathname.startsWith("/language-packs/") && url.pathname !== "/language-packs/catalog.json"
            ? `${url.pathname}${url.search}`
            : null;
    }
    catch {
        return null;
    }
}
function restoreLanguagePacks() {
    state.installedLanguagePacks = {};
    try {
        const stored = JSON.parse(window.localStorage.getItem(storageKeys.languagePacks) || "{}");
        if (!stored || typeof stored !== "object" || Array.isArray(stored))
            return;
        for (const value of Object.values(stored).slice(0, languagePackLimits.packs)) {
            try {
                const pack = validateLanguagePack(value);
                state.installedLanguagePacks[pack.locale] = pack;
                registerInstalledLanguagePack(pack);
            }
            catch {
            }
        }
    }
    catch {
        state.installedLanguagePacks = {};
    }
}
function syncLanguageOptions() {
    const selected = state.languagePreference;
    for (const option of Array.from(nodes.languageSelect.options)) {
        if (option.dataset.languagePack === "true")
            option.remove();
    }
    for (const pack of Object.values(state.installedLanguagePacks).sort((a, b) => a.nativeName.localeCompare(b.nativeName))) {
        const option = document.createElement("option");
        option.value = pack.locale;
        option.textContent = pack.nativeName;
        option.dataset.languagePack = "true";
        nodes.languageSelect.appendChild(option);
    }
    nodes.languageSelect.value = selected;
}
function setLanguagePackStatus(message, tone = "") {
    nodes.languagePackStatus.textContent = message;
    nodes.languagePackStatus.dataset.tone = tone;
}
async function boundedResponseText(response, maxBytes, label) {
    const declaredLength = Number(response.headers.get("content-length"));
    if (Number.isFinite(declaredLength) && declaredLength > maxBytes) {
        languagePackError(`${label} exceeds ${maxBytes} bytes`);
    }
    if (!response.body)
        languagePackError(`${label} response has no readable body`);
    const reader = response.body.getReader();
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let bytes = 0;
    let text = "";
    try {
        while (true) {
            const { done, value } = await reader.read();
            if (done)
                break;
            bytes += value.byteLength;
            if (bytes > maxBytes) {
                await reader.cancel("response exceeds configured limit");
                languagePackError(`${label} exceeds ${maxBytes} bytes`);
            }
            text += decoder.decode(value, { stream: true });
        }
        return text + decoder.decode();
    }
    catch (error) {
        if (error instanceof TypeError)
            languagePackError(`${label} is not valid UTF-8`);
        throw error;
    }
    finally {
        reader.releaseLock();
    }
}
async function fetchLanguagePackText(url) {
    const response = await fetch(url, { credentials: "same-origin", cache: "no-cache" });
    if (!response.ok)
        languagePackError(`${url} -> ${response.status}`);
    return boundedResponseText(response, languagePackLimits.bytes, "language pack");
}
async function loadLanguagePackCatalog() {
    setLanguagePackStatus(t("languagePacks.loading"));
    try {
        const response = await fetch(languagePackCatalogUrl, { credentials: "same-origin", cache: "no-cache" });
        if (!response.ok)
            languagePackError(`${languagePackCatalogUrl} -> ${response.status}`);
        const catalog = JSON.parse(await boundedResponseText(response, languagePackLimits.catalogBytes, "language-pack catalog"));
        if (catalog?.schema !== "leserpent.language-pack-catalog/v1"
            || !Array.isArray(catalog.packs)
            || catalog.packs.length > languagePackLimits.catalogPacks) {
            languagePackError("language-pack catalog schema is invalid");
        }
        state.languagePackCatalog = catalog.packs.flatMap((entry) => {
            const safeUrl = safeCatalogPackUrl(entry?.url);
            return entry
                && typeof entry.locale === "string"
                && typeof entry.version === "string"
                && typeof entry.nativeName === "string"
                && (entry.direction === "ltr" || entry.direction === "rtl")
                && entry.coverage === "core-ui"
                && safeUrl
                && /^[a-f0-9]{64}$/.test(entry.sha256)
                ? [{ ...entry, url: safeUrl }]
                : [];
        });
        state.languagePackCatalogMeta = {
            official: Number(catalog.officialLocaleCount) || builtinLanguageLocales.size + state.languagePackCatalog.length,
            builtin: Number(catalog.builtinLocaleCount) || builtinLanguageLocales.size,
        };
        renderLanguagePackCenter();
        setLanguagePackStatus(t("languagePacks.catalogReady", {
            official: state.languagePackCatalogMeta.official,
            builtin: state.languagePackCatalogMeta.builtin,
            count: state.languagePackCatalog.length,
        }), "good");
    }
    catch (error) {
        console.error(error);
        state.languagePackCatalog = [];
        renderLanguagePackCenter();
        setLanguagePackStatus(t("languagePacks.catalogFailed", { message: error.message }), "bad");
    }
}
async function verifiedCatalogPack(entry) {
    const text = await fetchLanguagePackText(entry.url);
    const digest = await sha256Hex(text);
    if (digest !== entry.sha256)
        languagePackError("language-pack SHA-256 verification failed");
    const pack = validateLanguagePack(JSON.parse(text));
    if (pack.locale !== entry.locale || pack.version !== entry.version) {
        languagePackError("language-pack metadata does not match its catalog entry");
    }
    return pack;
}
async function installLanguagePack(pack) {
    const validated = validateLanguagePack(pack);
    const previous = state.installedLanguagePacks;
    const next = { ...state.installedLanguagePacks, [validated.locale]: validated };
    if (Object.keys(next).length > languagePackLimits.packs)
        languagePackError("at most 12 language packs can be installed");
    try {
        state.installedLanguagePacks = next;
        persistLanguagePacks();
        registerInstalledLanguagePack(validated);
    }
    catch (error) {
        state.installedLanguagePacks = previous;
        if (previous[validated.locale]) {
            registerInstalledLanguagePack(previous[validated.locale]);
        }
        else {
            delete translations[validated.locale];
        }
        throw error;
    }
    syncLanguageOptions();
    renderLanguagePackCenter();
    setLanguagePackStatus(t("languagePacks.installed", { name: validated.nativeName }), "good");
}
function downloadLanguagePack(pack) {
    const blob = new Blob([serializeLanguagePack(pack)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `leserpent-language-${pack.locale}-${pack.version}.json`;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
}
async function handleLanguagePackAction(button) {
    const locale = button.dataset.locale;
    const action = button.dataset.languagePackAction;
    const key = `language-pack:${locale}`;
    if (state.uiActions.has(key))
        return;
    if (action === "remove") {
        const pack = state.installedLanguagePacks[locale];
        if (!pack || !window.confirm(t("languagePacks.removeConfirm", { name: pack.nativeName })))
            return;
    }
    await runUiActionOnce(key, button, `${button.textContent}...`, async () => {
        try {
            if (action === "remove") {
                delete state.installedLanguagePacks[locale];
                delete translations[locale];
                persistLanguagePacks();
                if (state.languagePreference === locale) {
                    state.languagePreference = "auto";
                    state.language = resolveLanguage("auto");
                    setStoredLanguagePreference("auto");
                }
                syncLanguageOptions();
                applyTranslations();
                renderDashboardFromCache();
                renderLanguagePackCenter();
                setLanguagePackStatus(t("languagePacks.removed"), "good");
                return;
            }
            if (action === "export") {
                downloadLanguagePack(state.installedLanguagePacks[locale]);
                return;
            }
            const entry = state.languagePackCatalog.find((item) => item.locale === locale);
            if (!entry)
                languagePackError("catalog entry not found");
            const pack = await verifiedCatalogPack(entry);
            if (action === "download") {
                downloadLanguagePack(pack);
                setLanguagePackStatus(t("languagePacks.downloaded", { name: pack.nativeName }), "good");
            }
            else {
                await installLanguagePack(pack);
            }
        }
        catch (error) {
            console.error(error);
            setLanguagePackStatus(t("languagePacks.operationFailed", { message: error.message }), "bad");
        }
    });
}
async function importLanguagePackFile(file) {
    if (!file)
        return;
    await runUiActionOnce("language-pack-import", nodes.languagePackImport, `${t("languagePacks.import")}...`, async () => {
        try {
            if (file.size > languagePackLimits.bytes)
                languagePackError("language pack exceeds 256 KiB");
            await installLanguagePack(JSON.parse(await file.text()));
        }
        catch (error) {
            console.error(error);
            setLanguagePackStatus(t("languagePacks.operationFailed", { message: error.message }), "bad");
        }
        finally {
            nodes.languagePackFile.value = "";
        }
    });
}
function renderLanguagePackCenter() {
    const installed = Object.values(state.installedLanguagePacks);
    nodes.languagePackInstalled.innerHTML = installed.length
        ? installed.map((pack) => `<div class="language-pack-row"><div><strong>${escapeHtml(pack.nativeName)}</strong><span>${escapeHtml(pack.locale)} · ${escapeHtml(pack.version)}</span></div><div><button type="button" data-language-pack-action="export" data-locale="${escapeHtml(pack.locale)}">${escapeHtml(t("languagePacks.export"))}</button><button type="button" class="quiet" data-language-pack-action="remove" data-locale="${escapeHtml(pack.locale)}">${escapeHtml(t("languagePacks.remove"))}</button></div></div>`).join("")
        : `<div class="hint-line">${escapeHtml(t("languagePacks.noneInstalled"))}</div>`;
    nodes.languagePackCatalog.innerHTML = state.languagePackCatalog.length
        ? state.languagePackCatalog.map((entry) => {
            const present = !!state.installedLanguagePacks[entry.locale];
            return `<div class="language-pack-row"><div><strong>${escapeHtml(entry.nativeName)}</strong><span>${escapeHtml(entry.locale)} · ${escapeHtml(entry.version)} · ${escapeHtml(t("languagePacks.coverageCore"))}</span></div><div><button type="button" data-language-pack-action="install" data-locale="${escapeHtml(entry.locale)}" ${present ? "disabled" : ""}>${escapeHtml(present ? t("languagePacks.installedLabel") : t("languagePacks.install"))}</button><button type="button" class="quiet" data-language-pack-action="download" data-locale="${escapeHtml(entry.locale)}">${escapeHtml(t("languagePacks.download"))}</button></div></div>`;
        }).join("")
        : `<div class="hint-line">${escapeHtml(t("languagePacks.catalogEmpty"))}</div>`;
}
function getStoredAdminToken() {
    try {
        window.localStorage.removeItem(storageKeys.adminToken);
        return window.sessionStorage.getItem(storageKeys.adminToken) || "";
    }
    catch {
        return "";
    }
}
function setStoredAdminToken(value) {
    try {
        const normalized = value?.trim() || "";
        window.localStorage.removeItem(storageKeys.adminToken);
        if (!normalized) {
            window.sessionStorage.removeItem(storageKeys.adminToken);
            return;
        }
        window.sessionStorage.setItem(storageKeys.adminToken, normalized);
    }
    catch {
    }
}
function getStoredAdminTokenTestState() {
    try {
        window.localStorage.removeItem(storageKeys.adminTokenTestState);
        return window.sessionStorage.getItem(storageKeys.adminTokenTestState) || "never";
    }
    catch {
        return "never";
    }
}
function getStoredAdminTokenTestAt() {
    try {
        window.localStorage.removeItem(storageKeys.adminTokenTestAt);
        return window.sessionStorage.getItem(storageKeys.adminTokenTestAt) || null;
    }
    catch {
        return null;
    }
}
function setStoredAdminTokenTest(stateValue, atValue) {
    try {
        window.localStorage.removeItem(storageKeys.adminTokenTestState);
        window.localStorage.removeItem(storageKeys.adminTokenTestAt);
        window.sessionStorage.setItem(storageKeys.adminTokenTestState, stateValue || "never");
        if (atValue) {
            window.sessionStorage.setItem(storageKeys.adminTokenTestAt, atValue);
        }
        else {
            window.sessionStorage.removeItem(storageKeys.adminTokenTestAt);
        }
    }
    catch {
    }
}
function syncAdminTokenFromInput(rawValue) {
    const normalized = (rawValue || "").trim();
    const previousToken = state.adminToken || "";
    state.adminToken = normalized;
    setStoredAdminToken(state.adminToken);
    if (normalized !== previousToken) {
        state.adminTokenTestState = "never";
        state.adminTokenTestAt = null;
        setStoredAdminTokenTest(state.adminTokenTestState, state.adminTokenTestAt);
        if (!normalized && nodes.adminTokenInput) {
            nodes.adminTokenInput.value = "";
        }
    }
    renderSecurityState();
}
function clearAdminToken() {
    if (nodes.adminTokenInput) {
        nodes.adminTokenInput.value = "";
    }
    state.adminToken = "";
    setStoredAdminToken("");
    state.adminTokenTestState = "never";
    state.adminTokenTestAt = null;
    setStoredAdminTokenTest(state.adminTokenTestState, state.adminTokenTestAt);
    state.adminTokenVisible = false;
    updateAdminTokenVisibilityButton();
    renderSecurityState();
    nodes.adminTokenState.textContent = t("security.tokenCleared");
}
function updateAdminTokenVisibilityButton() {
    if (!nodes.adminTokenToggleVisibility || !nodes.adminTokenInput) {
        return;
    }
    nodes.adminTokenInput.type = state.adminTokenVisible ? "text" : "password";
    nodes.adminTokenToggleVisibility.textContent = state.adminTokenVisible
        ? t("security.hideToken")
        : t("security.showToken");
}
function closeSecurityDetails() {
    if (!nodes.securityDetails) {
        return;
    }
    nodes.securityDetails.open = false;
    syncSecurityDetailsState();
}
function syncSecurityDetailsState() {
    document.documentElement.dataset.securityOpen = nodes.securityDetails?.open ? "true" : "false";
}
function looksLikeTokenDenied(message) {
    const normalized = `${message || ""}`.toLowerCase();
    return normalized.includes("token")
        || normalized.includes("unauthorized")
        || normalized.includes("forbidden")
        || normalized.includes("401")
        || normalized.includes("403");
}
function renderSecurityState(capabilities = null) {
    let securityVisualState = "local";
    if (nodes.adminTokenState) {
        if (state.adminTokenTestState === "running") {
            nodes.adminTokenState.textContent = t("security.tokenTestRunning");
            securityVisualState = "running";
        }
        else if (state.adminTokenTestState === "ok") {
            nodes.adminTokenState.textContent = t("security.tokenTestOk");
            securityVisualState = "ok";
        }
        else if (state.adminTokenTestState === "failed") {
            nodes.adminTokenState.textContent = state.adminToken?.trim()
                ? t("security.tokenStored")
                : t("security.tokenRequired");
            securityVisualState = state.adminToken?.trim() ? "stored" : "required";
        }
        else if (state.adminToken?.trim()) {
            nodes.adminTokenState.textContent = t("security.tokenStored");
            securityVisualState = "stored";
        }
        else {
            nodes.adminTokenState.textContent = t("security.localModeHint");
            securityVisualState = "local";
        }
        if (capabilities?.security?.adminTokenConfigured && !state.adminToken?.trim()) {
            nodes.adminTokenState.textContent = t("security.tokenRequired");
            securityVisualState = "required";
        }
        nodes.adminTokenState.dataset.state = securityVisualState;
    }
    if (nodes.adminTokenLastTest) {
        const stateLabel = state.adminTokenTestState === "ok"
            ? t("security.testStateOk")
            : state.adminTokenTestState === "failed"
                ? t("security.testStateFailed")
                : state.adminTokenTestState === "running"
                    ? t("security.testStateRunning")
                    : t("security.neverTested");
        const atValue = state.adminTokenTestAt || t("security.neverTested");
        nodes.adminTokenLastTest.textContent = `${t("security.lastTokenTest")}: ${stateLabel} · ${atValue}`;
    }
    if (nodes.securityPanelBadge) {
        nodes.securityPanelBadge.dataset.state = securityVisualState;
        nodes.securityPanelBadge.textContent = securityVisualState === "ok"
            ? t("security.panelOk")
            : securityVisualState === "running"
                ? t("security.panelRunning")
                : securityVisualState === "required"
                    ? t("security.panelNeedsToken")
                    : securityVisualState === "stored"
                        ? t("security.panelStored")
                        : t("security.panelLocal");
    }
    if (nodes.securityDetails) {
        nodes.securityDetails.dataset.state = securityVisualState;
    }
    updateAdminTokenVisibilityButton();
}
async function decodeApiError(response, path) {
    const payload = await response.json().catch(() => null);
    return payload?.reason
        || payload?.error
        || `${response.status} ${response.statusText || ""}`.trim()
        || `request failed for ${path}`;
}
function browserPreferredLanguage() {
    const browserLanguage = navigator.language || navigator.languages?.[0] || "en";
    const normalized = browserLanguage.toLowerCase();
    const installedMatch = Object.keys(state.installedLanguagePacks || {}).find((locale) => {
        const candidate = locale.toLowerCase();
        return normalized === candidate || normalized.startsWith(`${candidate}-`) || candidate.startsWith(`${normalized}-`);
    });
    if (installedMatch)
        return installedMatch;
    if (normalized.startsWith("zh")) {
        if (normalized.includes("hant")
            || normalized.includes("tw")
            || normalized.includes("hk")
            || normalized.includes("mo")) {
            return "zh-TW";
        }
        return "zh-CN";
    }
    if (normalized.startsWith("ja"))
        return "ja";
    if (normalized.startsWith("es"))
        return "es";
    if (normalized.startsWith("de"))
        return "de";
    if (normalized.startsWith("fr"))
        return "fr";
    if (normalized.startsWith("ko"))
        return "ko";
    return "en";
}
function browserPreferredTheme() {
    return window.matchMedia?.("(prefers-color-scheme: dark)")?.matches ? "dark" : "light";
}
function resolveLanguage(preference) {
    if (preference && preference !== "auto" && translations[preference]) {
        return preference;
    }
    return browserPreferredLanguage();
}
function resolveTheme(preference) {
    if (preference === "light" || preference === "dark") {
        return preference;
    }
    return browserPreferredTheme();
}
function applyTheme() {
    document.documentElement.dataset.theme = state.theme;
    if (nodes.themeSelect) {
        nodes.themeSelect.value = state.themePreference;
    }
}
function resolveLayoutMode(width = window.innerWidth, height = window.innerHeight) {
    if (width <= 600) {
        return "mobile";
    }
    if (width <= 980 && height <= 700) {
        return "emergency";
    }
    if (width <= 1180 && height <= 820) {
        return "safe-compact";
    }
    if (width <= 1366 || height <= 860) {
        return "compact";
    }
    return "default";
}
function syncMobileFilterDisclosure() {
    document.documentElement.dataset.mobileFiltersOpen = String(state.mobileFiltersOpen);
    nodes.mobileFilterToggle?.setAttribute("aria-expanded", String(state.mobileFiltersOpen));
    const activeCount = [state.filter.environment, state.filter.cluster, state.filter.role]
        .filter(Boolean)
        .length;
    if (nodes.mobileFilterCount) {
        nodes.mobileFilterCount.textContent = String(activeCount);
        nodes.mobileFilterCount.classList.toggle("hidden", activeCount === 0);
    }
}
function setMobileFiltersOpen(open, restoreFocus = false) {
    state.mobileFiltersOpen = Boolean(open);
    syncMobileFilterDisclosure();
    if (!state.mobileFiltersOpen && restoreFocus) {
        window.requestAnimationFrame(() => nodes.mobileFilterToggle?.focus());
    }
}
function syncRuntimeListLayout(width = nodes.runtimeListCard?.getBoundingClientRect().width || 0) {
    if (!(width > 0))
        return;
    state.runtimeListLayout = width <= 920 ? "cards" : "table";
    document.documentElement.dataset.runtimeListLayout = state.runtimeListLayout;
}
function applyLayoutMode() {
    state.layoutMode = resolveLayoutMode();
    document.documentElement.dataset.layoutMode = state.layoutMode;
    syncMobileFilterDisclosure();
    syncRuntimeListLayout();
}
function buildQuery() {
    const params = new URLSearchParams();
    if (state.languagePreference && state.languagePreference !== "auto")
        params.set("lang", state.languagePreference);
    if (state.themePreference && state.themePreference !== "auto")
        params.set("theme", state.themePreference);
    if (state.activeTab && state.activeTab !== "overview")
        params.set("tab", state.activeTab);
    if (state.activeOverviewTab && state.activeOverviewTab !== "summary")
        params.set("overview", state.activeOverviewTab);
    if (state.activeRuntimeMainTab && state.activeRuntimeMainTab !== "select")
        params.set("runtimePane", state.activeRuntimeMainTab);
    if (state.activeRuntimeDetailTab && state.activeRuntimeDetailTab !== "identity")
        params.set("runtimeDetail", state.activeRuntimeDetailTab);
    if (state.runtimePanelView && state.runtimePanelView !== "root")
        params.set("runtimeView", state.runtimePanelView);
    if (state.filter.environment)
        params.set("environment", state.filter.environment);
    if (state.filter.cluster)
        params.set("cluster", state.filter.cluster);
    if (state.filter.role)
        params.set("role", state.filter.role);
    if (state.runtimeSearch)
        params.set("search", state.runtimeSearch);
    if (state.runtimeSort && state.runtimeSort !== "name")
        params.set("sort", state.runtimeSort);
    if (state.selectedRuntimeId)
        params.set("runtimeId", state.selectedRuntimeId);
    const qs = params.toString();
    return qs ? `?${qs}` : "";
}
function hydrateStateFromLocation() {
    const params = new URLSearchParams(window.location.search);
    const lang = params.get("lang");
    const theme = params.get("theme");
    const storedPreference = getStoredLanguagePreference();
    const storedThemePreference = getStoredThemePreference();
    state.languagePreference =
        (lang && (lang === "auto" || translations[lang])) ? lang :
            (storedPreference && (storedPreference === "auto" || translations[storedPreference])) ? storedPreference :
                "auto";
    state.language = resolveLanguage(state.languagePreference);
    state.themePreference =
        (theme && (theme === "auto" || theme === "light" || theme === "dark")) ? theme :
            (storedThemePreference && (storedThemePreference === "auto" || storedThemePreference === "light" || storedThemePreference === "dark")) ? storedThemePreference :
                "auto";
    state.theme = resolveTheme(state.themePreference);
    state.adminToken = getStoredAdminToken();
    state.adminTokenVisible = false;
    state.adminTokenTestState = getStoredAdminTokenTestState();
    state.adminTokenTestAt = getStoredAdminTokenTestAt();
    state.activeTab = params.get("tab") || "overview";
    if (state.activeTab === "register") {
        state.activeTab = "runtimes";
        state.activeRuntimeMainTab = "register";
    }
    state.activeOverviewTab = params.get("overview") || "summary";
    state.activeRuntimeMainTab =
        params.get("runtimePane") ||
            params.get("runtimeMode") ||
            params.get("runtimeSide") ||
            state.activeRuntimeMainTab ||
            "select";
    state.activeRuntimeSideTab = state.activeRuntimeMainTab === "panel" ? "panel" : "detail";
    state.activeRuntimeDetailTab = normalizeRuntimeDetailTab(params.get("runtimeDetail"));
    state.runtimePanelView = params.get("runtimeView") || "root";
    state.selectedRuntimeId = params.get("runtimeId") || null;
    if (state.activeRuntimeMainTab === "panel" && state.selectedRuntimeId) {
        applyRuntimeWindowDeepLink(state.selectedRuntimeId, state.runtimePanelView);
    }
    state.filter.environment = params.get("environment") || "";
    state.filter.cluster = params.get("cluster") || "";
    state.filter.role = params.get("role") || "";
    state.runtimeSearch = params.get("search") || "";
    state.runtimeSort = params.get("sort") || "name";
}
function syncLocation() {
    const next = `${window.location.pathname}${buildQuery()}`;
    if (state.lastSyncedLocation === next) {
        return;
    }
    if (state.pendingLocationSync) {
        window.cancelAnimationFrame(state.pendingLocationSync);
    }
    state.pendingLocationSync = window.requestAnimationFrame(() => {
        state.pendingLocationSync = 0;
        const latest = `${window.location.pathname}${buildQuery()}`;
        if (state.lastSyncedLocation === latest) {
            return;
        }
        window.history.replaceState(null, "", latest);
        state.lastSyncedLocation = latest;
    });
}
function escapeHtml(value) {
    return String(value ?? "")
        .replaceAll("&", "&amp;")
        .replaceAll("<", "&lt;")
        .replaceAll(">", "&gt;")
        .replaceAll('"', "&quot;")
        .replaceAll("'", "&#39;");
}
function applyTranslations() {
    syncLanguageOptions();
    document.documentElement.lang = state.language;
    document.documentElement.dir = state.installedLanguagePacks[state.language]?.direction === "rtl" ? "rtl" : "ltr";
    document.title = `leserpent · ${t("hero.title")}`;
    nodes.languageSelect.value = state.languagePreference;
    if (nodes.themeSelect) {
        nodes.themeSelect.value = state.themePreference;
    }
    for (const node of document.querySelectorAll("[data-i18n]")) {
        node.textContent = t(node.dataset.i18n);
    }
    for (const node of document.querySelectorAll("[data-i18n-placeholder]")) {
        node.placeholder = t(node.dataset.i18nPlaceholder);
    }
    for (const node of document.querySelectorAll("[data-i18n-aria-label]")) {
        node.setAttribute("aria-label", t(node.dataset.i18nAriaLabel));
    }
    syncRegistrationSecretToggles();
    renderRegisterPreview();
    const options = Array.from(nodes.languageSelect.options);
    const optionLabels = {
        auto: "language.auto",
        en: "language.english",
        "zh-CN": "language.simplifiedChinese",
        "zh-TW": "language.traditionalChinese",
        ja: "language.japanese",
        es: "language.spanish",
    };
    for (const option of options) {
        const labelKey = optionLabels[option.value];
        if (labelKey) {
            option.textContent = t(labelKey);
        }
    }
    if (nodes.themeSelect) {
        const themeOptions = Array.from(nodes.themeSelect.options);
        for (const option of themeOptions) {
            if (option.value === "auto") {
                option.textContent = t("theme.auto");
            }
            else if (option.value === "light") {
                option.textContent = t("theme.light");
            }
            else if (option.value === "dark") {
                option.textContent = t("theme.dark");
            }
        }
    }
    if (nodes.languagePackInstalled && nodes.languagePackCatalog) {
        renderLanguagePackCenter();
    }
}
function syncTabSet(buttons, panels, activeValue, buttonKey, panelKey, prefix) {
    for (const button of buttons) {
        const value = button.dataset[buttonKey];
        const isActive = value === activeValue;
        const panel = panels.find((candidate) => candidate.dataset[panelKey] === value);
        const buttonId = `${prefix}-tab-${value}`;
        const panelId = `${prefix}-panel-${value}`;
        button.id = buttonId;
        button.setAttribute("role", "tab");
        button.setAttribute("aria-selected", String(isActive));
        button.setAttribute("aria-controls", panelId);
        button.tabIndex = isActive ? 0 : -1;
        button.classList.toggle("active", isActive);
        if (panel) {
            panel.id = panelId;
            panel.setAttribute("role", "tabpanel");
            panel.setAttribute("aria-labelledby", buttonId);
            panel.classList.toggle("active", isActive);
            panel.hidden = !isActive;
        }
    }
}
function applyTabShell() {
    if (state.activeTab !== "runtimes" || state.activeRuntimeMainTab !== "register") {
        maskRegistrationSecrets();
    }
    for (const button of nodes.tabButtons) {
        const isActive = button.dataset.tab === state.activeTab;
        button.classList.toggle("active", isActive);
        if (isActive) {
            button.setAttribute("aria-current", "page");
        }
        else {
            button.removeAttribute("aria-current");
        }
    }
    for (const panel of nodes.tabPanels) {
        const isActive = panel.dataset.tabPanel === state.activeTab;
        panel.classList.toggle("active", isActive);
        panel.hidden = !isActive;
    }
    syncTabSet(nodes.runtimeMainTabButtons, nodes.runtimeMainPanels, state.activeRuntimeMainTab, "runtimeMainTab", "runtimeMainPanel", "runtime-main");
    syncTabSet(nodes.overviewSubtabButtons, nodes.overviewSubpanels, state.activeOverviewTab, "overviewTab", "overviewPanel", "overview");
    if (nodes.runtimeWorkspace) {
        nodes.runtimeWorkspace.classList.toggle("register-focus", state.activeRuntimeMainTab === "register");
        nodes.runtimeWorkspace.classList.toggle("panel-focus", state.activeRuntimeMainTab === "panel");
        nodes.runtimeWorkspace.classList.toggle("detail-focus", state.activeRuntimeMainTab === "detail");
        nodes.runtimeWorkspace.dataset.mainTab = state.activeRuntimeMainTab;
    }
    syncTabSet(nodes.runtimeDetailSubtabButtons, nodes.runtimeDetailSections, state.activeRuntimeDetailTab, "runtimeDetailTab", "runtimeDetailPanel", "runtime-detail");
    window.requestAnimationFrame(() => syncRuntimeListLayout());
}
async function testAdminToken() {
    const token = state.adminToken?.trim();
    if (!token) {
        nodes.statusLine.textContent = t("security.tokenMissing");
        nodes.securityDetails?.setAttribute("open", "open");
        return;
    }
    state.adminTokenTestState = "running";
    state.adminTokenTestAt = null;
    setStoredAdminTokenTest(state.adminTokenTestState, state.adminTokenTestAt);
    renderSecurityState();
    nodes.statusLine.textContent = t("security.tokenTestRunning");
    try {
        const capabilities = await getJson("/v1/capabilities");
        state.cache.capabilities = capabilities;
        state.adminTokenTestState = "ok";
        state.adminTokenTestAt = new Date().toLocaleString();
        setStoredAdminTokenTest(state.adminTokenTestState, state.adminTokenTestAt);
        nodes.statusLine.textContent = t("security.tokenTestOk");
        await loadDashboard();
        renderSecurityState(capabilities);
    }
    catch (error) {
        console.error(error);
        state.adminTokenTestState = "failed";
        state.adminTokenTestAt = new Date().toLocaleString();
        setStoredAdminTokenTest(state.adminTokenTestState, state.adminTokenTestAt);
        const message = looksLikeTokenDenied(error.message)
            ? t("security.tokenRequired")
            : error.message;
        nodes.statusLine.textContent = t("security.tokenTestFailed", { message });
        renderSecurityState();
        nodes.securityDetails?.setAttribute("open", "open");
    }
}
function apiHeaders({ contentType = null, intent = null } = {}) {
    const headers = {};
    if (contentType) {
        headers["Content-Type"] = contentType;
    }
    if (intent) {
        headers["X-Leserpent-Intent"] = intent;
    }
    const token = state.adminToken?.trim();
    if (token) {
        headers["X-Leserpent-Admin-Token"] = token;
        headers.Authorization = `Bearer ${token}`;
    }
    return headers;
}
async function getJson(path, signal = null) {
    const response = await fetch(path, { headers: apiHeaders(), signal: signal || undefined });
    if (!response.ok) {
        throw new Error(await decodeApiError(response, path));
    }
    return response.json();
}
async function postJson(path) {
    const response = await fetch(path, {
        method: "POST",
        headers: apiHeaders({ intent: "mutate" }),
    });
    if (!response.ok) {
        throw new Error(await decodeApiError(response, path));
    }
    return response.json();
}
async function postJsonBody(path, body, signal = null) {
    const response = await fetch(path, {
        method: "POST",
        headers: apiHeaders({ contentType: "application/json", intent: "mutate" }),
        body: JSON.stringify(body),
        signal: signal || undefined,
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) {
        const reason = payload?.reason || payload?.error || `${response.status}`;
        throw new Error(reason);
    }
    return payload;
}
function isSidecarView(view = state.runtimePanelView) {
    return typeof view === "string" && view.startsWith("sidecar-");
}
function runtimePanelSource(view = state.runtimePanelView) {
    return isSidecarView(view) ? "sidecar" : "runtime";
}
function shouldRenderRuntimePanelBlank(runtime, trust, view = state.runtimePanelView) {
    const source = runtimePanelSource(view);
    if (source === "sidecar") {
        return !runtime.sidecarEndpoint || runtime.sidecarStatus?.statusSource === "fetch_failed" || runtime.sidecarStatus?.statusSource === "unobserved";
    }
    if (isIdleReadyStatus(runtime?.status)) {
        return false;
    }
    return runtime.status.statusSource === "fetch_failed"
        || runtime.status.statusSource === "unobserved"
        || !runtime.status.hasLatestSnapshot
        || runtime.status.snapshotKind === "none";
}
function runtimePanelBlankMarkup(runtime, trust, url, view = state.runtimePanelView) {
    const source = runtimePanelSource(view);
    const isFetchFailed = trust.source === "fetch_failed";
    const title = isFetchFailed
        ? t("runtimePanel.blankFetchFailedTitle")
        : source === "sidecar"
            ? t("runtimePanel.blankSidecarTitle")
            : t("runtimePanel.blankRuntimeTitle");
    const body = isFetchFailed
        ? t("runtimePanel.blankFetchFailedBody")
        : source === "sidecar"
            ? t("runtimePanel.blankSidecarBody")
            : t("runtimePanel.blankRuntimeBody");
    const hint = source === "sidecar"
        ? t("runtimePanel.blankHintRefreshSidecar")
        : t("runtimePanel.blankHintRefreshRuntime");
    const sourceLabel = source === "sidecar"
        ? t("runtimePanel.sources.sidecar")
        : t("runtimePanel.sources.runtime");
    const viewLabel = t(`runtimePanel.views.${view}`);
    const targetText = url || t("runtimePanel.notReady");
    const stateText = isFetchFailed
        ? t("statuses.fetchFailed")
        : trust.source === "unobserved"
            ? t("statuses.unobserved")
            : t("statuses.observed");
    return `
    <div class="runtime-panel-console-head">
      <span class="runtime-panel-console-badge">${escapeHtml(sourceLabel)}</span>
      <span class="runtime-panel-console-sep">/</span>
      <span class="runtime-panel-console-badge">${escapeHtml(viewLabel)}</span>
      <span class="runtime-panel-console-sep">/</span>
      <span class="runtime-panel-console-state">${escapeHtml(stateText)}</span>
    </div>
    <div class="runtime-panel-blank-copy">
      <strong>${escapeHtml(title)}</strong>
      <p>${escapeHtml(body)}</p>
      <div class="runtime-panel-console-target">${escapeHtml(targetText)}</div>
      <div class="runtime-panel-blank-hints">
        <span class="tag-pill">${escapeHtml(hint)}</span>
      </div>
    </div>
  `;
}
function renderRuntimePanelBlank(runtime, trust, url, view = state.runtimePanelView) {
    nodes.runtimePanelBlank.classList.remove("hidden");
    nodes.runtimePanelBlank.innerHTML = runtimePanelBlankMarkup(runtime, trust, url, view);
}
function compactTrustMessage(trust, view = state.runtimePanelView) {
    const source = runtimePanelSource(view);
    if (source === "sidecar") {
        if (trust.source === "fetch_failed")
            return t("runtimePanel.compactTrustSidecarFetchFailed");
        if (trust.source === "unobserved")
            return t("runtimePanel.compactTrustSidecarUnobserved");
        if (trust.label === t("runtimePanel.trustNoSidecar"))
            return t("runtimePanel.compactTrustNoSidecar");
        return t("runtimePanel.compactTrustSidecarObserved");
    }
    if (trust.source === "idle_ready")
        return t("runtimePanel.compactTrustIdleReady");
    if (trust.source === "fetch_failed")
        return t("runtimePanel.compactTrustFetchFailed");
    if (trust.source === "unobserved")
        return t("runtimePanel.compactTrustUnobserved");
    return t("runtimePanel.compactTrustObserved");
}
function defaultRuntimePanelViewForSource(source) {
    return source === "sidecar" ? "sidecar-root" : "root";
}
function markBadgeRefresh(kind) {
    if (kind !== "runtime" && kind !== "sidecar") {
        return;
    }
    state.recentBadgeRefresh[kind] = Date.now();
}
function badgeRecentlyUpdated(kind) {
    const value = state.recentBadgeRefresh[kind];
    return typeof value === "number" && Date.now() - value < 2400;
}
function switchRuntimePanelSource(source, runtime) {
    if (source === "sidecar" && !runtime?.sidecarEndpoint) {
        return;
    }
    state.runtimePanelView = defaultRuntimePanelViewForSource(source);
    if (state.activeRuntimeWindowId) {
        state.runtimeWindowViews[state.activeRuntimeWindowId] = state.runtimePanelView;
        persistRuntimeWindows();
    }
    renderRuntimePanel(runtime);
    syncLocation();
}
function runtimeSourceBadge(status) {
    if (isIdleReadyStatus(status)) {
        return { tone: "good", text: t("statuses.idleReady"), refreshKind: null };
    }
    if (!status || status.statusSource === "fetch_failed") {
        return { tone: "bad", text: t("statuses.fetchFailed"), refreshKind: "status" };
    }
    if (!status.hasLatestSnapshot) {
        return { tone: "warn", text: t("statuses.unobserved"), refreshKind: "status" };
    }
    return {
        tone: "good",
        text: t("statuses.observedSnapshot", { kind: status.snapshotKind || t("statuses.observed") }),
        refreshKind: null,
    };
}
function runtimePanelUrl(runtime, view = state.runtimePanelView) {
    if (!runtime) {
        return "";
    }
    if (isSidecarView(view)) {
        if (!runtime.sidecarEndpoint) {
            return "";
        }
        const sidecarBase = runtime.sidecarEndpoint.replace(/\/+$/, "");
        switch (view) {
            case "sidecar-health":
                return `${sidecarBase}/health`;
            case "sidecar-status":
                return `${sidecarBase}/v1/latest/status`;
            case "sidecar-memory":
                return `${sidecarBase}/v1/memory-versions.json`;
            case "sidecar-enrichment":
                return `${sidecarBase}/v1/latest/evidence-chain-enrichment.json`;
            case "sidecar-opinion":
                return `${sidecarBase}/v1/latest/diagnostic-opinion.json`;
            case "sidecar-root":
            default:
                return sidecarBase;
        }
    }
    if (!runtime.endpoint) {
        return "";
    }
    const base = runtime.endpoint.replace(/\/+$/, "");
    switch (view) {
        case "health":
            return `${base}/health`;
        case "meta":
            return `${base}/v1/latest/meta`;
        case "summary":
            return `${base}/v1/latest/summary.json`;
        case "analysis":
            return `${base}/v1/latest/analysis.json`;
        case "training":
            return `${base}/v1/latest/training-example.json`;
        case "dataset":
            return `${base}/v1/latest/training-dataset.json`;
        case "export":
            return `${base}/v1/latest/export.json`;
        case "report-json":
            return `${base}/v1/latest/report.json`;
        case "report-html":
            return `${base}/v1/latest/report.html`;
        case "targets":
            return `${base}/v1/latest/targets`;
        case "root":
        default:
            return base;
    }
}
function runtimeHasSidecarMemory(runtime) {
    return !!runtime?.sidecarStatus?.memory?.versionsSupported;
}
function runtimeSupportsPanelView(runtime, view = state.runtimePanelView) {
    if (!runtime) {
        return false;
    }
    if (isSidecarView(view)) {
        if (!runtime.sidecarEndpoint) {
            return false;
        }
        if (view === "sidecar-memory") {
            return runtimeHasSidecarMemory(runtime);
        }
        return true;
    }
    switch (view) {
        case "root":
        case "health":
        case "meta":
            return !!runtime.endpoint;
        case "summary":
            return !!runtime.status?.hasSummaryJson;
        case "analysis":
            return !!runtime.status?.hasAnalysisJson;
        case "training":
            return !!runtime.status?.hasTrainingExampleJson;
        case "dataset":
            return !!runtime.status?.hasTrainingDatasetManifest;
        case "export":
            return !!runtime.status?.hasExportJson;
        case "report-json":
            return !!runtime.status?.hasReportJson;
        case "report-html":
            return !!runtime.status?.hasReportHtml;
        case "targets":
            return !!runtime.status?.hasLatestSnapshot;
        default:
            return !!runtime.endpoint;
    }
}
function latestSidecarMemoryText(sidecarStatus) {
    const memory = sidecarStatus?.memory;
    if (!memory?.versionsSupported || !memory.latestSlot) {
        return t("runtimeDetail.none");
    }
    const label = memory.latestLabel ? `${memory.latestSlot} · ${memory.latestLabel}` : memory.latestSlot;
    return memory.latestSource ? `${label} · ${memory.latestSource}` : label;
}
function runtimePanelTrustState(runtime, view = state.runtimePanelView) {
    if (isSidecarView(view)) {
        if (!runtime?.sidecarEndpoint) {
            return {
                tone: "warn",
                label: t("runtimePanel.trustNoSidecar"),
                message: t("runtimePanel.trustNoSidecarMessage"),
                source: "none",
                snapshot: "sidecar",
                refreshKind: null,
            };
        }
        const sidecarStatus = runtime.sidecarStatus;
        if (!sidecarStatus) {
            return {
                tone: "warn",
                label: t("runtimePanel.trustSidecarUnobserved"),
                message: t("runtimePanel.trustSidecarUnobservedMessage"),
                source: "unobserved",
                snapshot: "starting",
                refreshKind: "sidecar",
            };
        }
        if (sidecarStatus.statusSource === "fetch_failed") {
            return {
                tone: "bad",
                label: t("runtimePanel.trustSidecarFetchFailed"),
                message: t("runtimePanel.trustSidecarFetchFailedMessage"),
                source: sidecarStatus.statusSource,
                snapshot: sidecarStatus.daemonStatus || "unknown",
                refreshKind: "sidecar",
            };
        }
        if (sidecarStatus.statusSource === "unobserved" || sidecarStatus.daemonStatus === "starting") {
            return {
                tone: "warn",
                label: t("runtimePanel.trustSidecarUnobserved"),
                message: t("runtimePanel.trustSidecarUnobservedMessage"),
                source: sidecarStatus.statusSource,
                snapshot: sidecarStatus.daemonStatus || "starting",
                refreshKind: "sidecar",
            };
        }
        return {
            tone: sidecarStatus.daemonStatus === "degraded" ? "warn" : "good",
            label: t("runtimePanel.trustSidecarObserved"),
            message: t("runtimePanel.trustSidecarObservedMessage"),
            source: sidecarStatus.statusSource,
            snapshot: sidecarStatus.daemonStatus || "ready",
            refreshKind: null,
        };
    }
    const status = runtime?.status;
    if (!status || status.statusSource === "fetch_failed") {
        return {
            tone: "bad",
            label: t("runtimePanel.trustFetchFailed"),
            message: t("runtimePanel.trustFetchFailedMessage"),
            source: status?.statusSource || "fetch_failed",
            snapshot: status?.snapshotKind || t("runtimeDetail.none"),
            refreshKind: "status",
        };
    }
    if (isIdleReadyStatus(status)) {
        return {
            tone: "good",
            label: t("runtimePanel.trustIdleReady"),
            message: t("runtimePanel.trustIdleReadyMessage"),
            source: "idle_ready",
            snapshot: status.socketServiceStatus || status.snapshotKind || t("runtimeDetail.none"),
            refreshKind: null,
        };
    }
    if (!status.hasLatestSnapshot) {
        return {
            tone: "warn",
            label: t("runtimePanel.trustUnobserved"),
            message: t("runtimePanel.trustUnobservedMessage"),
            source: status.statusSource,
            snapshot: status.snapshotKind || t("runtimeDetail.none"),
            refreshKind: null,
        };
    }
    return {
        tone: "good",
        label: t("runtimePanel.trustObserved"),
        message: t("runtimePanel.trustObservedMessage"),
        source: status.statusSource,
        snapshot: status.snapshotKind || t("runtimeDetail.none"),
        refreshKind: null,
    };
}
function protocolReadingStore() {
    state.cache.protocolReadingByRuntimeId ||= {};
    state.cache.protocolReadingPendingByRuntimeId ||= {};
    return state.cache.protocolReadingByRuntimeId;
}
function protocolReadingPendingStore() {
    protocolReadingStore();
    return state.cache.protocolReadingPendingByRuntimeId;
}
function runtimeProtocolReadingContainer() {
    return document.getElementById("runtime-panel-reading");
}
function protocolReadingAbsoluteUrl(runtime, path) {
    if (!runtime?.endpoint || !path) {
        return "";
    }
    return `${runtime.endpoint.replace(/\/+$/, "")}${path}`;
}
function protocolReadingChipLabel(protocol, entry) {
    return `${protocol} / ${entry}`;
}
function protocolReadingOverlayLabel(viaOverlay) {
    return viaOverlay ? t("runtimePanel.reading.via", { overlay: viaOverlay }) : "";
}
function clearRuntimeProtocolReading() {
    const container = runtimeProtocolReadingContainer();
    if (!container) {
        return;
    }
    container.classList.add("hidden");
    container.innerHTML = "";
}
function renderRuntimeProtocolReading(runtime, reading) {
    const container = runtimeProtocolReadingContainer();
    if (!container) {
        return;
    }
    if (!runtime || !reading) {
        clearRuntimeProtocolReading();
        return;
    }
    const currentUrl = protocolReadingAbsoluteUrl(runtime, reading.currentSurfacePath);
    const currentOverlay = protocolReadingOverlayLabel(reading.selectedOverlay);
    const companionLinks = (reading.readingCompanions || []).map((companion) => {
        const surfaceUrl = protocolReadingAbsoluteUrl(runtime, companion.surfacePath);
        const overlayText = protocolReadingOverlayLabel(companion.viaOverlay);
        return `
      <a class="protocol-reading-link" href="${escapeHtml(surfaceUrl)}" target="_blank" rel="noreferrer">
        <span>${escapeHtml(protocolReadingChipLabel(companion.protocol, companion.entry))}</span>
        ${overlayText ? `<small>${escapeHtml(overlayText)}</small>` : ""}
      </a>
    `;
    }).join("");
    container.classList.remove("hidden");
    container.innerHTML = `
    <span class="protocol-reading-kicker">${escapeHtml(t("runtimePanel.reading.title"))}</span>
    <div class="protocol-reading-links">
      <a class="protocol-reading-link is-current" href="${escapeHtml(currentUrl)}" target="_blank" rel="noreferrer">
        <span>${escapeHtml(protocolReadingChipLabel(reading.protocol, reading.entry))}</span>
        ${currentOverlay ? `<small>${escapeHtml(currentOverlay)}</small>` : ""}
      </a>
      ${(reading.readingCompanions || []).length ? `<span class="protocol-reading-sep">${escapeHtml(t("runtimePanel.reading.next"))}</span>` : ""}
      ${companionLinks}
    </div>
    <span class="protocol-reading-meta">${escapeHtml(t("runtimePanel.reading.target", { target: reading.targetName }))}</span>
  `;
}
async function loadRuntimeProtocolReading(runtimeId) {
    if (!runtimeId) {
        return null;
    }
    const cache = protocolReadingStore();
    const pending = protocolReadingPendingStore();
    if (pending[runtimeId]) {
        return pending[runtimeId];
    }
    pending[runtimeId] = (async () => {
        try {
            const reading = await getJson(`/v1/runtimes/${runtimeId}/protocol-reading`);
            cache[runtimeId] = reading;
            const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
            if (selectedRuntime && state.selectedRuntimeId === runtimeId) {
                renderRuntimeProtocolReading(selectedRuntime, reading);
            }
            return reading;
        }
        catch (error) {
            cache[runtimeId] = null;
            const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
            if (selectedRuntime && state.selectedRuntimeId === runtimeId) {
                clearRuntimeProtocolReading();
            }
            return null;
        }
        finally {
            delete pending[runtimeId];
        }
    })();
    return pending[runtimeId];
}
function ensureRuntimeProtocolReading(runtime) {
    if (!runtime?.runtimeId) {
        clearRuntimeProtocolReading();
        return;
    }
    const cache = protocolReadingStore();
    if (cache[runtime.runtimeId]) {
        renderRuntimeProtocolReading(runtime, cache[runtime.runtimeId]);
        return;
    }
    clearRuntimeProtocolReading();
    void loadRuntimeProtocolReading(runtime.runtimeId);
}
function refreshLabel(kind) {
    return kind === "all"
        ? t("notifications.runtimeRefreshAll")
        : kind === "status"
            ? t("notifications.runtimeRefreshStatus")
            : kind === "sidecar"
                ? t("runtimeDetail.refreshSidecar")
                : t("notifications.runtimeRefreshCapabilities");
}
function recoveryActionLabel(action) {
    return action === "refresh_all"
        ? t("attention.actions.refreshAll")
        : action === "refresh_status"
            ? t("attention.actions.refreshStatus")
            : action === "refresh_sidecar"
                ? t("attention.actions.refreshSidecar")
                : action === "register_runtime"
                    ? t("attention.actions.registerRuntime")
                    : action;
}
function recoveryOutcomeLabel(outcome) {
    return outcome === "ok"
        ? t("attention.outcomes.ok")
        : outcome === "auth_failed"
            ? t("attention.outcomes.authFailed")
            : outcome === "network_failed"
                ? t("attention.outcomes.networkFailed")
                : outcome === "incomplete_data"
                    ? t("attention.outcomes.incompleteData")
                    : outcome === "degraded"
                        ? t("attention.outcomes.degraded")
                        : outcome;
}
function recoveryHintLabel(action, hint) {
    if (action === "refresh_status") {
        return t("attention.hints.refreshStatus");
    }
    if (action === "refresh_all") {
        return t("attention.hints.refreshAll");
    }
    if (action === "refresh_sidecar") {
        return t("attention.hints.refreshSidecar");
    }
    return hint || "";
}
function runtimeDetailSignature(runtime, attention) {
    if (!runtime) {
        return `empty:${state.language}`;
    }
    const capabilityKeys = (runtime.capabilities || [])
        .map((item) => `${item.key}:${item.support}:${item.description || ""}`)
        .sort()
        .join("|");
    const attentionReasons = (attention?.reasons || []).join("|");
    const attentionActions = (attention?.suggestedActions || [])
        .map((item) => `${item.action}:${item.priority}:${item.coolingDown}:${item.cooldownSecondsRemaining ?? 0}`)
        .join("|");
    const recoveryHistory = (attention?.recentRecoveryActivities || [])
        .map((item) => `${item.action}:${item.outcome}:${item.recordedAt}:${item.summary || ""}`)
        .join("|");
    return [
        state.language,
        runtime.runtimeId,
        runtime.name,
        runtime.endpoint,
        runtime.sidecarEndpoint || "",
        runtime.registeredAt,
        runtime.updatedAt,
        runtime.capabilitySource || "",
        runtime.capabilityFetchedAt || "",
        runtime.capabilityFetchError || "",
        runtime.tags.environment || "",
        runtime.tags.cluster || "",
        runtime.tags.role || "",
        runtime.status.statusSource,
        runtime.status.statusFetchedAt || "",
        runtime.status.statusFetchError || "",
        runtime.status.resilienceStatus || "",
        runtime.status.resilienceSummary || "",
        runtime.status.socketServiceStatus || "",
        runtime.status.socketConsecutiveIdleTimeouts ?? "",
        runtime.status.socketTotalIdleTimeouts ?? "",
        runtime.status.snapshotKind || "",
        runtime.status.targetCount ?? "",
        runtime.status.hasSummaryJson,
        runtime.status.hasAnalysisJson,
        runtime.status.hasTrainingExampleJson,
        runtime.status.hasTrainingDatasetManifest,
        runtime.status.hasExportJson,
        runtime.status.hasReportJson,
        runtime.status.hasReportHtml,
        runtime.hasSidecarAdminToken,
        runtime.sidecarStatus?.statusSource || "",
        runtime.sidecarStatus?.statusFetchedAt || "",
        runtime.sidecarStatus?.statusFetchError || "",
        runtime.sidecarStatus?.daemonStatus || "",
        runtime.sidecarStatus?.learningActive ?? "",
        runtime.sidecarStatus?.learnedRoutes ?? "",
        runtime.sidecarStatus?.memory?.versionsSupported ?? "",
        runtime.sidecarStatus?.memory?.slotCount ?? "",
        runtime.sidecarStatus?.memory?.historyCount ?? "",
        capabilityKeys,
        attention?.severity || "",
        attention?.needsAttention || "",
        attentionReasons,
        attentionActions,
        recoveryHistory,
    ].join("::");
}
const MAX_RUNTIME_WINDOWS = 8;
const MAX_RUNTIME_WINDOW_STATE_BYTES = 64 * 1024;
const runtimePanelViews = new Set([
    "root",
    "health",
    "meta",
    "summary",
    "analysis",
    "training",
    "dataset",
    "export",
    "report-json",
    "report-html",
    "targets",
    "sidecar-root",
    "sidecar-health",
    "sidecar-status",
    "sidecar-memory",
    "sidecar-enrichment",
    "sidecar-opinion",
]);
function normalizeRuntimeWindowId(value) {
    return typeof value === "string" && value.length > 0 && value.length <= 256 ? value : null;
}
function normalizeRuntimeWindowView(value) {
    return typeof value === "string" && runtimePanelViews.has(value) ? value : "root";
}
function sanitizeRuntimeWindowIds(values, limit = MAX_RUNTIME_WINDOWS) {
    const ids = [];
    const seen = new Set();
    for (const value of Array.isArray(values) ? values : []) {
        const id = normalizeRuntimeWindowId(value);
        if (!id || seen.has(id))
            continue;
        ids.push(id);
        seen.add(id);
        if (ids.length >= limit)
            break;
    }
    return ids;
}
function sanitizeRuntimeWindowViews(ids, values) {
    const views = Object.create(null);
    const source = values && typeof values === "object" && !Array.isArray(values) ? values : {};
    for (const id of ids) {
        views[id] = normalizeRuntimeWindowView(source[id]);
    }
    return views;
}
function runtimeWindowStateWithinLimit(value) {
    return typeof value === "string"
        && value.length <= MAX_RUNTIME_WINDOW_STATE_BYTES
        && new TextEncoder().encode(value).byteLength <= MAX_RUNTIME_WINDOW_STATE_BYTES;
}
function persistRuntimeWindows() {
    try {
        const ids = sanitizeRuntimeWindowIds(state.runtimeWindowIds);
        window.localStorage.setItem(storageKeys.runtimeWindows, JSON.stringify({
            ids,
            activeId: ids.includes(state.activeRuntimeWindowId) ? state.activeRuntimeWindowId : ids[0] || null,
            views: sanitizeRuntimeWindowViews(ids, state.runtimeWindowViews),
        }));
    }
    catch {
    }
}
function restoreRuntimeWindows() {
    try {
        const stored = window.localStorage.getItem(storageKeys.runtimeWindows);
        const value = runtimeWindowStateWithinLimit(stored)
            ? JSON.parse(stored)
            : null;
        state.runtimeWindowIds = sanitizeRuntimeWindowIds(value?.ids);
        state.activeRuntimeWindowId = state.runtimeWindowIds.includes(value?.activeId)
            ? value.activeId
            : state.runtimeWindowIds[0] || null;
        state.runtimeWindowViews = sanitizeRuntimeWindowViews(state.runtimeWindowIds, value?.views);
    }
    catch {
        state.runtimeWindowIds = [];
        state.activeRuntimeWindowId = null;
        state.runtimeWindowViews = Object.create(null);
    }
}
function applyRuntimeWindowDeepLink(runtimeId, view) {
    const id = normalizeRuntimeWindowId(runtimeId);
    if (!id)
        return;
    const restoredIds = sanitizeRuntimeWindowIds(state.runtimeWindowIds);
    state.runtimeWindowIds = [id, ...restoredIds.filter((candidate) => candidate !== id)];
    state.activeRuntimeWindowId = id;
    state.runtimeWindowViews = sanitizeRuntimeWindowViews(state.runtimeWindowIds, state.runtimeWindowViews);
    state.runtimeWindowViews[id] = normalizeRuntimeWindowView(view);
    state.runtimeWindowIntentPending = true;
}
function reconcileRuntimeWindows() {
    const available = new Set(state.latestRuntimes.map((runtime) => runtime.runtimeId));
    const previousIds = state.runtimeWindowIds.join("\u0000");
    const previousActiveId = state.activeRuntimeWindowId;
    const intentPending = state.runtimeWindowIntentPending;
    state.runtimeWindowIds = sanitizeRuntimeWindowIds(sanitizeRuntimeWindowIds(state.runtimeWindowIds, MAX_RUNTIME_WINDOWS + 1)
        .filter((id) => available.has(id)));
    if (!state.runtimeWindowIds.includes(state.activeRuntimeWindowId)) {
        state.activeRuntimeWindowId = state.runtimeWindowIds[0] || null;
    }
    const sanitizedViews = sanitizeRuntimeWindowViews(state.runtimeWindowIds, state.runtimeWindowViews);
    const viewsChanged = JSON.stringify(sanitizedViews) !== JSON.stringify(state.runtimeWindowViews);
    state.runtimeWindowViews = sanitizedViews;
    if (previousIds !== state.runtimeWindowIds.join("\u0000")
        || previousActiveId !== state.activeRuntimeWindowId
        || viewsChanged
        || intentPending) {
        state.runtimeWindowIntentPending = false;
        persistRuntimeWindows();
    }
}
function openRuntimeWindow(runtimeId) {
    runtimeId = normalizeRuntimeWindowId(runtimeId);
    if (!runtimeId)
        return false;
    if (!state.runtimeWindowIds.includes(runtimeId)) {
        if (state.runtimeWindowIds.length >= MAX_RUNTIME_WINDOWS) {
            nodes.statusLine.textContent = t("runtimePanel.windows.limitReached", { limit: MAX_RUNTIME_WINDOWS });
            return false;
        }
        state.runtimeWindowIds.push(runtimeId);
    }
    state.activeRuntimeWindowId = runtimeId;
    state.selectedRuntimeId = runtimeId;
    state.runtimePanelView = normalizeRuntimeWindowView(state.runtimeWindowViews[runtimeId] || state.runtimePanelView);
    state.runtimeWindowViews[runtimeId] = state.runtimePanelView;
    state.renderSignatures.runtimePanel = "";
    persistRuntimeWindows();
    renderRuntimeSliceFromCache();
    syncLocation();
    return true;
}
function openAllRuntimeWindows() {
    const selected = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId);
    const candidates = selected
        ? [selected, ...state.latestRuntimes.filter((runtime) => runtime.runtimeId !== selected.runtimeId)]
        : state.latestRuntimes;
    for (const runtime of candidates) {
        if (state.runtimeWindowIds.length >= MAX_RUNTIME_WINDOWS)
            break;
        if (!state.runtimeWindowIds.includes(runtime.runtimeId)) {
            state.runtimeWindowIds.push(runtime.runtimeId);
        }
        state.runtimeWindowViews[runtime.runtimeId] ||= "root";
    }
    state.activeRuntimeWindowId ||= state.runtimeWindowIds[0] || null;
    if (state.activeRuntimeWindowId)
        state.selectedRuntimeId = state.activeRuntimeWindowId;
    state.renderSignatures.runtimePanel = "";
    persistRuntimeWindows();
    renderRuntimeSliceFromCache();
    const count = state.runtimeWindowIds.length;
    nodes.statusLine.textContent = count < state.latestRuntimes.length
        ? t("runtimePanel.windows.openAllLimited", {
            count,
            total: state.latestRuntimes.length,
            limit: MAX_RUNTIME_WINDOWS,
        })
        : t("runtimePanel.windows.openAllComplete", { count });
}
function focusRuntimeWindowAfterClose(runtimeId) {
    window.requestAnimationFrame(() => {
        if (runtimeId) {
            const identity = nodes.runtimeWindowGrid.querySelector(`.runtime-child-window-identity[data-runtime-id="${CSS.escape(runtimeId)}"]`);
            if (identity instanceof HTMLButtonElement) {
                identity.focus();
                return;
            }
        }
        nodes.runtimeWindowOpenSelected?.focus();
    });
}
function closeRuntimeWindow(runtimeId) {
    const closedIndex = state.runtimeWindowIds.indexOf(runtimeId);
    state.runtimeWindowIds = state.runtimeWindowIds.filter((id) => id !== runtimeId);
    delete state.runtimeWindowViews[runtimeId];
    if (state.activeRuntimeWindowId === runtimeId) {
        state.activeRuntimeWindowId = state.runtimeWindowIds[Math.min(Math.max(closedIndex, 0), state.runtimeWindowIds.length - 1)] || null;
    }
    if (state.activeRuntimeWindowId) {
        state.selectedRuntimeId = state.activeRuntimeWindowId;
        state.runtimePanelView = state.runtimeWindowViews[state.activeRuntimeWindowId] || "root";
    }
    state.renderSignatures.runtimePanel = "";
    persistRuntimeWindows();
    renderRuntimeSliceFromCache();
    syncLocation();
    focusRuntimeWindowAfterClose(state.activeRuntimeWindowId);
}
function closeAllRuntimeWindows() {
    state.runtimeWindowIds = [];
    state.activeRuntimeWindowId = null;
    state.runtimeWindowViews = Object.create(null);
    state.renderSignatures.runtimePanel = "";
    persistRuntimeWindows();
    renderRuntimeSliceFromCache();
}
function activateRuntimeWindow(runtimeId) {
    if (!state.runtimeWindowIds.includes(runtimeId))
        return;
    state.activeRuntimeWindowId = runtimeId;
    state.selectedRuntimeId = runtimeId;
    state.runtimePanelView = normalizeRuntimeWindowView(state.runtimeWindowViews[runtimeId]);
    state.renderSignatures.runtimePanel = "";
    persistRuntimeWindows();
    renderRuntimeSliceFromCache();
    syncLocation();
}
function handleRuntimeWindowGridClick(event) {
    const button = event.target.closest("[data-runtime-window-action][data-runtime-id]");
    if (!(button instanceof HTMLElement))
        return;
    const runtimeId = button.dataset.runtimeId;
    const action = button.dataset.runtimeWindowAction;
    if (action === "close") {
        closeRuntimeWindow(runtimeId);
    }
    else if (action === "external") {
        const runtime = state.latestRuntimes.find((item) => item.runtimeId === runtimeId);
        const url = runtimePanelUrl(runtime, state.runtimeWindowViews[runtimeId] || "root");
        if (url)
            window.open(url, "_blank", "noopener,noreferrer");
    }
    else {
        activateRuntimeWindow(runtimeId);
    }
}
function handleRuntimeWindowGridKeydown(event) {
    const identity = event.target.closest(".runtime-child-window-identity[data-runtime-id]");
    if (!(identity instanceof HTMLButtonElement))
        return;
    const ids = state.runtimeWindowIds;
    const index = ids.indexOf(identity.dataset.runtimeId);
    if (index < 0)
        return;
    const direction = document.documentElement.dir === "rtl" ? -1 : 1;
    let nextIndex = null;
    if (event.key === "ArrowRight")
        nextIndex = index + direction;
    if (event.key === "ArrowLeft")
        nextIndex = index - direction;
    if (event.key === "ArrowDown")
        nextIndex = index + 1;
    if (event.key === "ArrowUp")
        nextIndex = index - 1;
    if (event.key === "Home")
        nextIndex = 0;
    if (event.key === "End")
        nextIndex = ids.length - 1;
    if (nextIndex === null || !ids.length)
        return;
    event.preventDefault();
    const nextId = ids[(nextIndex + ids.length) % ids.length];
    activateRuntimeWindow(nextId);
    window.requestAnimationFrame(() => {
        nodes.runtimeWindowGrid
            .querySelector(`.runtime-child-window-identity[data-runtime-id="${CSS.escape(nextId)}"]`)
            ?.focus();
    });
}
function runtimeWindowSuspendedMarkup(runtime, view) {
    return `
    <div class="runtime-window-suspended">
      <div class="runtime-window-suspended-mark" aria-hidden="true">II</div>
      <div class="runtime-window-suspended-copy">
        <strong>${escapeHtml(t("runtimePanel.windows.pausedTitle"))}</strong>
        <p>${escapeHtml(t("runtimePanel.windows.pausedBody"))}</p>
      </div>
      <button type="button" data-runtime-window-action="activate" data-runtime-id="${escapeHtml(runtime.runtimeId)}">
        ${escapeHtml(t("runtimePanel.windows.pausedAction"))} · ${escapeHtml(t(`runtimePanel.views.${view}`))}
      </button>
    </div>`;
}
function runtimePanelSignature(runtime) {
    const windowBits = state.runtimeWindowIds.map((id) => {
        const item = state.latestRuntimes.find((candidate) => candidate.runtimeId === id);
        return item
            ? `${id}:${state.runtimeWindowViews[id] || "root"}:${item.updatedAt}:${item.status?.statusSource}:${item.status?.snapshotKind || ""}`
            : id;
    }).join("|");
    if (!runtime) {
        return `empty:${state.language}:${state.runtimePanelView}:${state.activeRuntimeWindowId || ""}:${windowBits}`;
    }
    const trust = runtimePanelTrustState(runtime, state.runtimePanelView);
    const source = runtimePanelSource(state.runtimePanelView);
    const url = runtimePanelUrl(runtime) || "";
    return [
        state.language,
        runtime.runtimeId,
        runtime.name,
        state.runtimePanelView,
        source,
        url,
        trust.tone,
        trust.label,
        trust.reason,
        runtime.endpoint,
        runtime.sidecarEndpoint || "",
        runtime.status.statusSource,
        runtime.status.snapshotKind || "",
        runtime.sidecarStatus?.statusSource || "",
        state.activeRuntimeWindowId || "",
        windowBits,
    ].join("::");
}
function renderRuntimeWindowGrid() {
    const wanted = new Set(state.runtimeWindowIds);
    for (const card of nodes.runtimeWindowGrid.querySelectorAll("[data-runtime-window-id]")) {
        if (!wanted.has(card.dataset.runtimeWindowId))
            card.remove();
    }
    for (const runtimeId of state.runtimeWindowIds) {
        const runtime = state.latestRuntimes.find((item) => item.runtimeId === runtimeId);
        if (!runtime)
            continue;
        const view = state.runtimeWindowViews[runtimeId] || "root";
        const trust = runtimePanelTrustState(runtime, view);
        const url = runtimePanelUrl(runtime, view) || "";
        const blank = shouldRenderRuntimePanelBlank(runtime, trust, view);
        const isActive = runtimeId === state.activeRuntimeWindowId;
        let card = nodes.runtimeWindowGrid.querySelector(`[data-runtime-window-id="${CSS.escape(runtimeId)}"]`);
        if (!card) {
            card = document.createElement("article");
            card.className = "runtime-child-window";
            card.dataset.runtimeWindowId = runtimeId;
            card.setAttribute("role", "listitem");
            card.innerHTML = `
        <header class="runtime-child-window-head">
          <button type="button" class="runtime-child-window-identity" data-runtime-window-action="activate" data-runtime-id="${escapeHtml(runtimeId)}">
            <strong data-runtime-window-name></strong>
            <span data-runtime-window-view></span>
          </button>
          <span class="runtime-state" data-runtime-window-status></span>
          <button type="button" class="quiet" data-runtime-window-action="external" data-runtime-id="${escapeHtml(runtimeId)}"></button>
          <button type="button" class="quiet" data-runtime-window-action="close" data-runtime-id="${escapeHtml(runtimeId)}"></button>
        </header>
        <div class="runtime-child-window-target" data-runtime-window-target></div>
        <div class="runtime-child-window-body">
          <div class="runtime-panel-blank hidden" data-runtime-window-blank></div>
          <iframe loading="lazy" referrerpolicy="no-referrer" sandbox data-runtime-window-frame></iframe>
        </div>`;
        }
        card.classList.toggle("is-active", isActive);
        card.classList.toggle("is-suspended", !isActive);
        card.setAttribute("aria-label", t("runtimePanel.windows.windowLabel", {
            name: runtime.name,
            view: t(`runtimePanel.views.${view}`),
        }));
        card.querySelector("[data-runtime-window-name]").textContent = runtime.name;
        card.querySelector("[data-runtime-window-view]").textContent = t(`runtimePanel.views.${view}`);
        const identity = card.querySelector(".runtime-child-window-identity");
        identity.tabIndex = isActive ? 0 : -1;
        identity.setAttribute("aria-pressed", String(isActive));
        identity.setAttribute("aria-label", `${t("runtimePanel.windows.activate")}: ${runtime.name}`);
        const status = card.querySelector("[data-runtime-window-status]");
        status.className = `runtime-state ${trust.tone}`;
        status.textContent = trust.label;
        const external = card.querySelector('[data-runtime-window-action="external"]');
        external.textContent = t("runtimePanel.windows.external");
        external.disabled = !url;
        external.setAttribute("aria-label", `${t("runtimePanel.windows.external")}: ${runtime.name}`);
        const close = card.querySelector('[data-runtime-window-action="close"]');
        close.textContent = t("runtimePanel.windows.close");
        close.setAttribute("aria-label", `${t("runtimePanel.windows.close")}: ${runtime.name}`);
        card.querySelector("[data-runtime-window-target]").textContent = url || runtime.endpoint;
        const blankNode = card.querySelector("[data-runtime-window-blank]");
        const frame = card.querySelector("[data-runtime-window-frame]");
        if (!isActive) {
            blankNode.innerHTML = runtimeWindowSuspendedMarkup(runtime, view);
            blankNode.classList.remove("hidden");
            frame.classList.add("hidden");
            frame.src = "about:blank";
            delete frame.dataset.src;
        }
        else if (blank) {
            blankNode.innerHTML = runtimePanelBlankMarkup(runtime, trust, url, view);
            blankNode.classList.remove("hidden");
            frame.classList.add("hidden");
            frame.src = "about:blank";
            delete frame.dataset.src;
        }
        else {
            blankNode.classList.add("hidden");
            frame.classList.remove("hidden");
            frame.title = `${runtime.name} ${t(`runtimePanel.views.${view}`)}`;
            if (url && frame.dataset.src !== url) {
                frame.src = url;
                frame.dataset.src = url;
            }
        }
        nodes.runtimeWindowGrid.appendChild(card);
    }
    const count = state.runtimeWindowIds.length;
    nodes.runtimeWindowCount.textContent = t("runtimePanel.windows.capacity", {
        count,
        limit: MAX_RUNTIME_WINDOWS,
    });
    nodes.runtimeWindowPolicy.textContent = t("runtimePanel.windows.policy");
    nodes.runtimeWindowToolbar.classList.remove("hidden");
    const selectedIsOpen = state.runtimeWindowIds.includes(state.selectedRuntimeId);
    nodes.runtimeWindowOpenSelected.disabled = !state.selectedRuntimeId
        || (!selectedIsOpen && count >= MAX_RUNTIME_WINDOWS);
    nodes.runtimeWindowOpenAll.disabled = state.latestRuntimes.length === 0
        || count >= Math.min(state.latestRuntimes.length, MAX_RUNTIME_WINDOWS);
    nodes.runtimeWindowCloseAll.disabled = count === 0;
    nodes.runtimeWindowGrid.classList.toggle("hidden", count === 0);
}
function finalizeRuntimeWindowWorkspace() {
    nodes.runtimePanelFrameWrap.classList.add("hidden");
    nodes.runtimePanelFrame.src = "about:blank";
    renderRuntimeWindowGrid();
}
function runtimeDetailTimestamp(value) {
    if (!value)
        return t("runtimeDetail.notObserved");
    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime()))
        return String(value);
    try {
        return new Intl.DateTimeFormat(state.language, {
            dateStyle: "medium",
            timeStyle: "short",
        }).format(parsed);
    }
    catch {
        return parsed.toLocaleString();
    }
}
function runtimeDetailTimeMarkup(value) {
    if (!value)
        return escapeHtml(t("runtimeDetail.notObserved"));
    return `<time datetime="${escapeHtml(value)}">${escapeHtml(runtimeDetailTimestamp(value))}</time>`;
}
function runtimeEvidenceItems(status) {
    return [
        { label: t("runtimeDetail.summaryJson"), available: !!status.hasSummaryJson },
        { label: t("runtimeDetail.analysisJson"), available: !!status.hasAnalysisJson },
        { label: t("runtimeDetail.trainingExampleJson"), available: !!status.hasTrainingExampleJson },
        { label: t("runtimeDetail.trainingDatasetManifest"), available: !!status.hasTrainingDatasetManifest },
        { label: t("runtimeDetail.exportJson"), available: !!status.hasExportJson },
        { label: t("runtimeDetail.reportJson"), available: !!status.hasReportJson },
        { label: t("runtimeDetail.reportHtml"), available: !!status.hasReportHtml },
    ];
}
function capabilitySupportLabel(support) {
    const normalized = protocolKeyToTranslationSegment(support || "unknown");
    const key = `runtimeDetail.support.${normalized}`;
    const translated = t(key);
    return translated === key ? String(support || "unknown").replaceAll("_", " ") : translated;
}
function capabilitySupportTone(support) {
    if (support === "fully_supported")
        return "good";
    if (support === "unsupported" || support === "not_supported")
        return "bad";
    return "warn";
}
function attentionSeverityLabel(attention) {
    const severity = attention?.severity || "warning";
    const key = `attention.${severity}`;
    const translated = t(key);
    return translated === key ? severity : translated;
}
function runtimeNeedsAttention(attention) {
    if (!attention)
        return false;
    if (typeof attention.needsAttention === "boolean")
        return attention.needsAttention;
    return (attention.reasons || []).length > 0;
}
function runtimeDetailPosture(runtime, attention) {
    if (runtimeNeedsAttention(attention)) {
        return {
            tone: attention.severity === "critical" ? "bad" : "warn",
            label: attentionSeverityLabel(attention),
            message: attention.reasons?.length
                ? attentionReasonLabel(attention.reasons[0])
                : t("runtimeDetail.requiresReview"),
            target: "attention",
            action: t("runtimeDetail.reviewAttention"),
        };
    }
    const badge = statusBadge(runtime.status);
    return {
        tone: badge.tone,
        label: badge.text,
        message: badge.tone === "bad"
            ? t("runtimeDetail.refreshRecommended")
            : badge.tone === "warn"
                ? t("statuses.unobserved")
                : t("runtimeDetail.operational"),
        target: "status",
        action: t("runtimeDetail.inspectStatus"),
    };
}
function renderRuntimeDetailSummary(runtime, attention) {
    const posture = runtimeDetailPosture(runtime, attention);
    const evidence = runtimeEvidenceItems(runtime.status);
    const availableEvidence = evidence.filter((item) => item.available).length;
    const capabilities = runtime.capabilities || [];
    const supportedCapabilities = capabilities.filter((item) => item.support === "fully_supported").length;
    const reasonCount = runtimeNeedsAttention(attention) ? (attention.reasons || []).length : 0;
    const observedAt = runtime.status.statusFetchedAt;
    nodes.runtimeDetailSummary.classList.remove("hidden");
    nodes.runtimeDetailSummary.innerHTML = `
    <div class="runtime-detail-posture" data-tone="${escapeHtml(posture.tone)}">
      <div class="runtime-detail-posture-copy">
        <div class="runtime-detail-kicker">${escapeHtml(t("runtimeDetail.liveSummary"))}</div>
        <div class="runtime-detail-posture-title">
          <span class="runtime-state ${escapeHtml(posture.tone)}">${escapeHtml(posture.label)}</span>
          <h3 id="runtime-detail-summary-heading">${escapeHtml(runtime.name)}</h3>
        </div>
        <p>${escapeHtml(posture.message)}</p>
      </div>
      <button type="button" class="quiet" data-runtime-detail-target="${escapeHtml(posture.target)}">${escapeHtml(posture.action)}</button>
    </div>
    <div class="runtime-detail-facts">
      <div class="runtime-detail-fact">
        <span>${escapeHtml(t("runtimeDetail.lastObserved"))}</span>
        <strong>${runtimeDetailTimeMarkup(observedAt)}</strong>
        <small>${escapeHtml(t("runtimeDetail.source"))}: ${escapeHtml(runtime.status.statusSource)}</small>
      </div>
      <div class="runtime-detail-fact">
        <span>${escapeHtml(t("runtimeDetail.attention"))}</span>
        <strong>${escapeHtml(reasonCount ? attentionSeverityLabel(attention) : t("runtimeDetail.clear"))}</strong>
        <small>${escapeHtml(t("runtimeDetail.attentionReasonCount", { count: reasonCount }))}</small>
      </div>
      <div class="runtime-detail-fact">
        <span>${escapeHtml(t("runtimeDetail.supportedCapabilities"))}</span>
        <strong>${escapeHtml(`${supportedCapabilities} / ${capabilities.length}`)}</strong>
        <small>${escapeHtml(t("runtimeDetail.fullySupportedCount", { count: supportedCapabilities }))}</small>
      </div>
      <div class="runtime-detail-fact">
        <span>${escapeHtml(t("runtimeDetail.availableEvidence"))}</span>
        <strong>${escapeHtml(`${availableEvidence} / ${evidence.length}`)}</strong>
        <small>${escapeHtml(t("runtimeDetail.availableCount", { count: availableEvidence }))}</small>
      </div>
    </div>
  `;
}
function renderRuntimeIdentity(runtime) {
    nodes.runtimeDetailIdentity.innerHTML = `
    <dl class="runtime-detail-definition-grid">
      <div>
        <dt>${escapeHtml(t("register.name"))}</dt>
        <dd><strong>${escapeHtml(runtime.name)}</strong></dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.runtimeId"))}</dt>
        <dd><code>${escapeHtml(runtime.runtimeId)}</code></dd>
      </div>
      <div>
        <dt>${escapeHtml(t("register.endpoint"))}</dt>
        <dd><code>${escapeHtml(runtime.endpoint)}</code></dd>
      </div>
      <div>
        <dt>${escapeHtml(t("register.sidecarEndpoint"))}</dt>
        <dd><code>${escapeHtml(runtime.sidecarEndpoint || t("register.sidecarUnpaired"))}</code></dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.registered"))}</dt>
        <dd>${runtimeDetailTimeMarkup(runtime.registeredAt)}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.updated"))}</dt>
        <dd>${runtimeDetailTimeMarkup(runtime.updatedAt)}</dd>
      </div>
    </dl>
    <div class="runtime-detail-tags" aria-label="${escapeHtml(t("runtimes.columns.tags"))}">
      <span class="tag-pill">${escapeHtml(runtime.tags.environment || t("runtimes.states.noEnv"))}</span>
      <span class="tag-pill">${escapeHtml(runtime.tags.cluster || t("runtimes.states.noCluster"))}</span>
      <span class="tag-pill">${escapeHtml(runtime.tags.role || t("runtimes.states.noRole"))}</span>
    </div>
  `;
}
function renderRuntimeStatus(runtime) {
    const badge = statusBadge(runtime.status);
    const sidecarBadge = sidecarStatusBadge(runtime.sidecarStatus);
    const evidence = runtimeEvidenceItems(runtime.status);
    const availableEvidence = evidence.filter((item) => item.available).length;
    const statusSummary = runtime.status.resilienceSummary || runtimeStatusHint(runtime.status);
    nodes.runtimeDetailStatus.innerHTML = `
    <div class="runtime-detail-section-lead">
      <span class="runtime-state ${escapeHtml(badge.tone)}">${escapeHtml(badge.text)}</span>
      <div>
        <strong>${escapeHtml(t("runtimeDetail.statusOverview"))}</strong>
        <p>${escapeHtml(statusSummary)}</p>
      </div>
    </div>
    <dl class="runtime-detail-definition-grid runtime-detail-status-grid">
      <div>
        <dt>${escapeHtml(t("runtimeDetail.source"))}</dt>
        <dd>${escapeHtml(runtime.status.statusSource)}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.lastObserved"))}</dt>
        <dd>${runtimeDetailTimeMarkup(runtime.status.statusFetchedAt)}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.snapshotKind"))}</dt>
        <dd>${escapeHtml(runtime.status.snapshotKind || t("runtimeDetail.none"))}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.targetCount"))}</dt>
        <dd>${escapeHtml(runtime.status.targetCount ?? t("runtimeDetail.na"))}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.resilienceStatus"))}</dt>
        <dd>${escapeHtml(runtime.status.resilienceStatus || t("runtimeDetail.none"))}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.socketServiceStatus"))}</dt>
        <dd>${escapeHtml(runtime.status.socketServiceStatus || t("runtimeDetail.none"))}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.idleTimeouts"))}</dt>
        <dd>${escapeHtml(runtime.status.socketTotalIdleTimeouts != null ? `${runtime.status.socketConsecutiveIdleTimeouts ?? 0} / ${runtime.status.socketTotalIdleTimeouts}` : t("runtimeDetail.na"))}</dd>
      </div>
    </dl>
    <div class="runtime-detail-section-heading">
      <strong>${escapeHtml(t("runtimeDetail.evidenceAvailability"))}</strong>
      <span>${escapeHtml(`${availableEvidence} / ${evidence.length}`)}</span>
    </div>
    <div class="runtime-evidence-grid">
      ${evidence.map((item) => `
        <div class="runtime-evidence-item ${item.available ? "available" : "missing"}">
          <span class="runtime-evidence-dot" aria-hidden="true"></span>
          <span>${escapeHtml(item.label)}</span>
          <strong>${escapeHtml(item.available ? t("runtimeDetail.available") : t("runtimeDetail.missing"))}</strong>
        </div>
      `).join("")}
    </div>
    <div class="runtime-sidecar-overview">
      <div class="runtime-detail-section-heading">
        <strong>${escapeHtml(t("runtimeDetail.sidecarOverview"))}</strong>
        <span class="runtime-state ${escapeHtml(sidecarBadge.tone)}">${escapeHtml(sidecarBadge.text)}</span>
      </div>
      <dl class="runtime-detail-definition-grid">
        <div>
          <dt>${escapeHtml(t("register.sidecarEndpoint"))}</dt>
          <dd><code>${escapeHtml(runtime.sidecarEndpoint || t("register.sidecarUnpaired"))}</code></dd>
        </div>
        <div>
          <dt>${escapeHtml(t("runtimeDetail.sidecarAccess"))}</dt>
          <dd>${escapeHtml(runtime.sidecarEndpoint ? (runtime.hasSidecarAdminToken ? t("runtimeDetail.sidecarProtected") : t("runtimeDetail.sidecarOpen")) : t("runtimeDetail.none"))}</dd>
        </div>
        ${runtime.sidecarStatus ? `
          <div>
            <dt>${escapeHtml(t("runtimeDetail.sidecarSource"))}</dt>
            <dd>${escapeHtml(runtime.sidecarStatus.statusSource)}</dd>
          </div>
          <div>
            <dt>${escapeHtml(t("runtimeDetail.lastObserved"))}</dt>
            <dd>${runtimeDetailTimeMarkup(runtime.sidecarStatus.statusFetchedAt)}</dd>
          </div>
          <div>
            <dt>${escapeHtml(t("runtimeDetail.sidecarLearning"))}</dt>
            <dd>${escapeHtml(runtime.sidecarStatus.learningActive ? t("security.enabled") : t("security.disabled"))} · ${escapeHtml(runtime.sidecarStatus.learnedRoutes)}</dd>
          </div>
          <div>
            <dt>${escapeHtml(t("runtimeDetail.sidecarMemory"))}</dt>
            <dd>${escapeHtml(runtime.sidecarStatus.memory?.versionsSupported ? `${runtime.sidecarStatus.memory.slotCount} / ${runtime.sidecarStatus.memory.historyCount}` : t("runtimeDetail.none"))}</dd>
          </div>
          <div>
            <dt>${escapeHtml(t("runtimeDetail.sidecarMemoryLatest"))}</dt>
            <dd>${escapeHtml(latestSidecarMemoryText(runtime.sidecarStatus))}</dd>
          </div>
        ` : ""}
      </dl>
    </div>
  `;
}
function renderRuntimeCapabilities(runtime) {
    const capabilities = [...(runtime.capabilities || [])]
        .sort((left, right) => left.key.localeCompare(right.key));
    const supported = capabilities.filter((item) => item.support === "fully_supported").length;
    nodes.runtimeDetailCapabilities.innerHTML = `
    <div class="runtime-detail-section-lead">
      <span class="runtime-state ${supported === capabilities.length && capabilities.length ? "good" : "warn"}">${escapeHtml(`${supported} / ${capabilities.length}`)}</span>
      <div>
        <strong>${escapeHtml(t("runtimeDetail.supportedCapabilities"))}</strong>
        <p>${escapeHtml(t("runtimeDetail.fullySupportedCount", { count: supported }))}</p>
      </div>
    </div>
    <dl class="runtime-detail-definition-grid runtime-capability-provenance">
      <div>
        <dt>${escapeHtml(t("runtimeDetail.capabilitySource"))}</dt>
        <dd>${escapeHtml(runtime.capabilitySource || t("runtimeDetail.none"))}</dd>
      </div>
      <div>
        <dt>${escapeHtml(t("runtimeDetail.lastCapabilityRefresh"))}</dt>
        <dd>${runtimeDetailTimeMarkup(runtime.capabilityFetchedAt)}</dd>
      </div>
    </dl>
    ${capabilities.length ? `
      <div class="runtime-capability-grid">
        ${capabilities.map((item) => `
          <article class="runtime-capability-card" data-tone="${escapeHtml(capabilitySupportTone(item.support))}">
            <div class="runtime-capability-head">
              <strong>${escapeHtml(item.key)}</strong>
              <span class="runtime-state ${escapeHtml(capabilitySupportTone(item.support))}">${escapeHtml(capabilitySupportLabel(item.support))}</span>
            </div>
            ${item.description ? `<p>${escapeHtml(item.description)}</p>` : ""}
          </article>
        `).join("")}
      </div>
    ` : `<div class="runtime-detail-empty-state">${escapeHtml(t("runtimeDetail.noCapabilities"))}</div>`}
  `;
}
function renderRuntimeAttention(attention) {
    if (!runtimeNeedsAttention(attention)) {
        nodes.runtimeDetailAttention.innerHTML = `
      <div class="runtime-detail-clear-state">
        <span class="runtime-state good">${escapeHtml(t("runtimeDetail.clear"))}</span>
        <strong>${escapeHtml(t("runtimeDetail.noAttention"))}</strong>
      </div>
    `;
        return;
    }
    const actions = [...(attention.suggestedActions || [])]
        .sort((left, right) => (left.priority ?? Number.MAX_SAFE_INTEGER) - (right.priority ?? Number.MAX_SAFE_INTEGER));
    const history = attention.recentRecoveryActivities || [];
    const tone = attention.severity === "critical" ? "bad" : "warn";
    nodes.runtimeDetailAttention.innerHTML = `
    <div class="runtime-detail-section-lead attention">
      <span class="runtime-state ${tone}">${escapeHtml(attentionSeverityLabel(attention))}</span>
      <div>
        <strong>${escapeHtml(t("runtimeDetail.requiresReview"))}</strong>
        <p>${escapeHtml(t("runtimeDetail.attentionReasonCount", { count: (attention.reasons || []).length }))}</p>
      </div>
    </div>
    <div class="reason-list">
      ${(attention.reasons || []).map((reason) => `<span class="reason-pill attention">${escapeHtml(attentionReasonLabel(reason))}</span>`).join("")}
    </div>
    <div class="runtime-detail-section-heading">
      <strong>${escapeHtml(t("attention.suggestedActions"))}</strong>
      <span>${escapeHtml(actions.length)}</span>
    </div>
    ${actions.length ? `
      <div class="runtime-recovery-grid">
        ${actions.map((action) => {
        const kind = action.commandKind;
        return `
            <article class="runtime-recovery-action ${action.coolingDown ? "cooling-down" : ""}">
              <div class="runtime-recovery-action-head">
                <strong>${escapeHtml(recoveryActionLabel(action.action))}</strong>
                <span class="tag-pill">#${escapeHtml(action.priority)}</span>
              </div>
              <p>${escapeHtml(recoveryHintLabel(action.action, action.hint))}</p>
              ${action.coolingDown ? `<div class="hint-line">${escapeHtml(t("attention.cooldownRemaining", { seconds: action.cooldownSecondsRemaining }))}</div>` : ""}
              ${kind
            ? `<button type="button" data-recovery-action="${escapeHtml(kind)}" ${action.coolingDown ? "disabled" : ""}>${escapeHtml(recoveryActionLabel(action.action))}</button>`
            : `<span class="item-meta">${escapeHtml(recoveryActionLabel(action.action))}</span>`}
            </article>
          `;
    }).join("")}
      </div>
    ` : `<div class="runtime-detail-empty-state">${escapeHtml(t("attention.noReasons"))}</div>`}
    <div class="runtime-detail-section-heading">
      <strong>${escapeHtml(t("attention.recentRecovery"))}</strong>
      <span>${escapeHtml(history.length)}</span>
    </div>
    ${history.length ? `
      <div class="runtime-recovery-history">
        ${history.map((item) => `
          <article>
            <div>
              <strong>${escapeHtml(recoveryActionLabel(item.action))}</strong>
              <span class="runtime-state ${item.outcome === "ok" ? "good" : "warn"}">${escapeHtml(recoveryOutcomeLabel(item.outcome))}</span>
            </div>
            <time datetime="${escapeHtml(item.recordedAt)}">${escapeHtml(runtimeDetailTimestamp(item.recordedAt))}</time>
            ${item.summary ? `<p>${escapeHtml(item.summary)}</p>` : ""}
          </article>
        `).join("")}
      </div>
    ` : `<div class="runtime-detail-empty-state">${escapeHtml(t("attention.noRecoveryHistory"))}</div>`}
  `;
}
function renderRuntimeDetail(runtime, attention) {
    const signature = runtimeDetailSignature(runtime, attention);
    if (state.renderSignatures.runtimeDetail === signature)
        return;
    state.renderSignatures.runtimeDetail = signature;
    if (!runtime) {
        nodes.runtimeDetailChip.textContent = t("runtimeDetail.nothingSelected");
        nodes.runtimeDetailActions.classList.add("hidden");
        nodes.runtimeDetailRefreshSidecar.disabled = true;
        nodes.runtimeDetailEmpty.classList.remove("hidden");
        nodes.runtimeDetailPanel.classList.add("hidden");
        nodes.runtimeDetailSummary.classList.add("hidden");
        nodes.runtimeDetailSummary.innerHTML = "";
        nodes.runtimeDetailIdentity.innerHTML = "";
        nodes.runtimeDetailStatus.innerHTML = "";
        nodes.runtimeDetailCapabilities.innerHTML = "";
        nodes.runtimeDetailAttention.innerHTML = "";
        for (const button of nodes.runtimeDetailSubtabButtons) {
            button.classList.remove("has-attention", "has-status-warning");
            button.removeAttribute("data-tone");
        }
        return;
    }
    const badge = statusBadge(runtime.status);
    const attentionButton = nodes.runtimeDetailSubtabButtons.find((button) => button.dataset.runtimeDetailTab === "attention");
    const statusButton = nodes.runtimeDetailSubtabButtons.find((button) => button.dataset.runtimeDetailTab === "status");
    const needsAttention = runtimeNeedsAttention(attention);
    attentionButton?.classList.toggle("has-attention", needsAttention);
    if (attentionButton)
        attentionButton.dataset.tone = needsAttention ? attention.severity : "clear";
    statusButton?.classList.toggle("has-status-warning", badge.tone !== "good");
    if (statusButton)
        statusButton.dataset.tone = badge.tone;
    nodes.runtimeDetailChip.textContent = runtime.name;
    nodes.runtimeDetailActions.classList.remove("hidden");
    nodes.runtimeDetailRefreshSidecar.disabled = !runtime.sidecarEndpoint;
    nodes.runtimeDetailEmpty.classList.add("hidden");
    nodes.runtimeDetailPanel.classList.remove("hidden");
    renderRuntimeDetailSummary(runtime, attention);
    renderRuntimeIdentity(runtime);
    renderRuntimeStatus(runtime);
    renderRuntimeCapabilities(runtime);
    renderRuntimeAttention(attention);
}
function renderRuntimePanel(runtime) {
    reconcileRuntimeWindows();
    runtime = state.latestRuntimes.find((item) => item.runtimeId === state.activeRuntimeWindowId) || null;
    if (runtime) {
        state.runtimePanelView = state.runtimeWindowViews[runtime.runtimeId] || "root";
    }
    const signature = runtimePanelSignature(runtime);
    if (state.renderSignatures.runtimePanel === signature) {
        return;
    }
    state.renderSignatures.runtimePanel = signature;
    if (!runtime) {
        nodes.runtimePanelChip.textContent = t("runtimePanel.notReady");
        nodes.runtimePanelChip.classList.remove("hidden");
        nodes.runtimePanelBreadcrumb.classList.add("hidden");
        nodes.runtimePanelTrust.className = "runtime-panel-trust hidden";
        nodes.runtimePanelTrust.innerHTML = "";
        nodes.runtimePanelSourceSwitch.classList.add("hidden");
        nodes.runtimePanelSourceBadges.classList.add("hidden");
        nodes.runtimePanelSourceBadges.innerHTML = "";
        nodes.runtimePanelActions.classList.add("hidden");
        nodes.runtimePanelEmpty.classList.remove("hidden");
        nodes.runtimePanelFrameWrap.classList.add("hidden");
        nodes.runtimePanelBlank.classList.add("hidden");
        nodes.runtimePanelBlank.innerHTML = "";
        nodes.runtimePanelFrame.src = "about:blank";
        nodes.runtimePanelUrl.textContent = "";
        nodes.runtimePanelOpenExternal.removeAttribute("href");
        clearRuntimeProtocolReading();
        finalizeRuntimeWindowWorkspace();
        return;
    }
    if (!runtimeSupportsPanelView(runtime, state.runtimePanelView)) {
        state.runtimePanelView = isSidecarView(state.runtimePanelView) ? "sidecar-root" : "root";
        state.runtimeWindowViews[runtime.runtimeId] = state.runtimePanelView;
        persistRuntimeWindows();
    }
    const url = runtimePanelUrl(runtime);
    const viewLabel = t(`runtimePanel.views.${state.runtimePanelView}`);
    const trust = runtimePanelTrustState(runtime, state.runtimePanelView);
    const source = runtimePanelSource(state.runtimePanelView);
    const sourceLabel = t(`runtimePanel.sources.${source}`);
    nodes.runtimePanelChip.textContent = runtime.name;
    nodes.runtimePanelChip.classList.add("hidden");
    ensureRuntimeProtocolReading(runtime);
    nodes.runtimePanelBreadcrumb.classList.remove("hidden");
    nodes.runtimePanelBreadcrumb.innerHTML = `
    <span class="crumb-block">
      <span class="crumb-label">source</span>
      <span class="crumb-value">${escapeHtml(sourceLabel)}</span>
    </span>
    <span class="crumb-block">
      <span class="crumb-label">view</span>
      <span class="crumb-value">${escapeHtml(viewLabel)}</span>
    </span>
    <span class="crumb-block crumb-block-target">
      <span class="crumb-label">target</span>
      <span class="crumb-value">${escapeHtml(runtime.name)}</span>
      <span class="crumb-status ${escapeHtml(trust.tone)}">${escapeHtml(trust.label)}</span>
    </span>
  `;
    nodes.runtimePanelTrust.className = "runtime-panel-trust hidden";
    nodes.runtimePanelTrust.innerHTML = "";
    nodes.runtimePanelSourceSwitch.classList.remove("hidden");
    for (const button of nodes.runtimePanelSourceButtons) {
        const isSidecar = button.dataset.runtimePanelSource === "sidecar";
        button.disabled = isSidecar && !runtime.sidecarEndpoint;
        button.classList.toggle("is-active", button.dataset.runtimePanelSource === source);
    }
    nodes.runtimePanelSourceBadges.classList.add("hidden");
    nodes.runtimePanelSourceBadges.innerHTML = "";
    nodes.runtimePanelActions.classList.remove("hidden");
    let overflowVisibleCount = 0;
    let overflowHasActiveTab = false;
    for (const tab of nodes.runtimePanelTabs) {
        const wantsSidecar = isSidecarView(tab.dataset.runtimePanelView);
        const hidden = tab.dataset.runtimePanelSource !== source || !runtimeSupportsPanelView(runtime, tab.dataset.runtimePanelView);
        tab.classList.toggle("hidden", hidden);
        tab.disabled = wantsSidecar && !runtime.sidecarEndpoint;
        const insideOverflow = !!tab.closest(".runtime-panel-overflow-menu");
        const isActive = tab.dataset.runtimePanelView === state.runtimePanelView;
        if (insideOverflow && !hidden) {
            overflowVisibleCount += 1;
            overflowHasActiveTab = overflowHasActiveTab || isActive;
        }
    }
    if (nodes.runtimePanelOverflow) {
        nodes.runtimePanelOverflow.classList.toggle("hidden", overflowVisibleCount === 0);
        nodes.runtimePanelOverflow.open = overflowVisibleCount > 0 && overflowHasActiveTab;
    }
    const sidecarViewWithoutEndpoint = isSidecarView(state.runtimePanelView) && !runtime.sidecarEndpoint;
    nodes.runtimePanelEmpty.classList.add("hidden");
    nodes.runtimePanelFrameWrap.classList.remove("hidden");
    if (sidecarViewWithoutEndpoint) {
        renderRuntimePanelBlank(runtime, trust, "", state.runtimePanelView);
        nodes.runtimePanelFrame.classList.add("hidden");
        nodes.runtimePanelFrame.src = "about:blank";
        if (url) {
            nodes.runtimePanelUrl.textContent = url;
            nodes.runtimePanelUrl.classList.remove("hidden");
        }
        else {
            nodes.runtimePanelUrl.textContent = "";
            nodes.runtimePanelUrl.classList.add("hidden");
        }
        nodes.runtimePanelOpenExternal.removeAttribute("href");
        finalizeRuntimeWindowWorkspace();
        return;
    }
    const useBlankShell = shouldRenderRuntimePanelBlank(runtime, trust, state.runtimePanelView);
    if (useBlankShell) {
        renderRuntimePanelBlank(runtime, trust, url, state.runtimePanelView);
        nodes.runtimePanelFrame.classList.add("hidden");
        nodes.runtimePanelFrame.src = "about:blank";
        if (url) {
            nodes.runtimePanelUrl.textContent = url;
            nodes.runtimePanelUrl.classList.remove("hidden");
        }
        else {
            nodes.runtimePanelUrl.textContent = "";
            nodes.runtimePanelUrl.classList.add("hidden");
        }
    }
    else {
        nodes.runtimePanelBlank.classList.add("hidden");
        nodes.runtimePanelBlank.innerHTML = "";
        nodes.runtimePanelFrame.classList.remove("hidden");
        nodes.runtimePanelFrame.src = url;
        if (url) {
            nodes.runtimePanelUrl.textContent = url;
            nodes.runtimePanelUrl.classList.remove("hidden");
        }
        else {
            nodes.runtimePanelUrl.textContent = "";
            nodes.runtimePanelUrl.classList.add("hidden");
        }
    }
    nodes.runtimePanelOpenExternal.href = url;
    nodes.runtimePanelOpenExternal.target = "_blank";
    nodes.runtimePanelOpenExternal.rel = "noreferrer";
    for (const tab of nodes.runtimePanelTabs) {
        tab.classList.toggle("is-active", tab.dataset.runtimePanelView === state.runtimePanelView);
    }
    finalizeRuntimeWindowWorkspace();
}
async function refreshRuntimeById(runtimeId, kind, button = null) {
    if (!runtimeId) {
        nodes.statusLine.textContent = t("notifications.noRuntimeSelected");
        return;
    }
    const label = refreshLabel(kind);
    await runUiActionOnce(`runtime-refresh:${runtimeId}`, button, `${label}...`, async () => {
        const detailControls = [
            nodes.runtimeDetailRefreshAll,
            nodes.runtimeDetailRefreshStatus,
            nodes.runtimeDetailRefreshCapabilities,
            nodes.runtimeDetailRefreshSidecar,
        ];
        for (const control of detailControls)
            control.disabled = true;
        nodes.statusLine.textContent = `${label}...`;
        try {
            const recovery = await postJsonBody(`/v1/runtimes/${runtimeId}/recovery`, { kind });
            if ((recovery.steps || []).some((step) => step.kind === "status"))
                markBadgeRefresh("runtime");
            if ((recovery.steps || []).some((step) => step.kind === "sidecar"))
                markBadgeRefresh("sidecar");
            state.activeTab = "runtimes";
            state.selectedRuntimeId = runtimeId;
            await loadDashboard();
            const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
            if (selectedRuntime) {
                renderRuntimePanel(selectedRuntime);
                window.setTimeout(() => {
                    const latestSelected = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
                    if (latestSelected)
                        renderRuntimePanel(latestSelected);
                }, 2500);
            }
            nodes.statusLine.textContent = t("notifications.runtimeRefreshComplete", { label });
        }
        catch (error) {
            console.error(error);
            nodes.statusLine.textContent = t("notifications.runtimeRefreshFailed", { label, message: error.message });
        }
        finally {
            for (const control of detailControls)
                control.disabled = false;
            const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId);
            nodes.runtimeDetailRefreshSidecar.disabled = !selectedRuntime?.sidecarEndpoint;
        }
    });
}
async function refreshSelectedRuntime(kind, button = null) {
    await refreshRuntimeById(state.selectedRuntimeId, kind, button);
}
async function loadRuntimeAttention(runtimeId) {
    if (!runtimeId) {
        return null;
    }
    try {
        const attention = await getJson(`/v1/runtimes/${runtimeId}/attention`);
        state.runtimeAttentionById.set(runtimeId, attention);
        const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
        if (selectedRuntime && state.selectedRuntimeId === runtimeId) {
            renderRuntimeDetail(selectedRuntime, attention);
        }
        return attention;
    }
    catch (error) {
        console.error(error);
        return null;
    }
}
async function copySelectedRuntimeLink() {
    if (!state.selectedRuntimeId) {
        nodes.statusLine.textContent = t("notifications.noRuntimeSelected");
        return;
    }
    const url = `${window.location.origin}${window.location.pathname}${buildQuery()}`;
    try {
        await navigator.clipboard.writeText(url);
        nodes.statusLine.textContent = t("notifications.runtimeLinkCopied");
    }
    catch (error) {
        console.error(error);
        nodes.statusLine.textContent = t("notifications.runtimeLinkFailed", { message: error.message });
    }
}
function renderMetricCards(target, items, signatureKey = "") {
    const signature = [state.language, ...items.map(([label, value]) => `${label}:${value}`)].join("::");
    if (signatureKey && state.renderSignatures[signatureKey] === signature) {
        return;
    }
    if (signatureKey) {
        state.renderSignatures[signatureKey] = signature;
    }
    target.innerHTML = items.map(([label, value]) => `
    <div class="metric">
      <div class="metric-label">${escapeHtml(label)}</div>
      <div class="metric-value">${escapeHtml(value)}</div>
    </div>
  `).join("");
}
function renderGroupCards(target, groups, signatureKey = "") {
    const entries = Object.entries(groups);
    const signature = [
        state.language,
        ...entries.map(([title, values]) => `${title}:${Object.entries(values).map(([key, count]) => `${key}:${count}`).join("|")}`),
    ].join("::");
    if (signatureKey && state.renderSignatures[signatureKey] === signature) {
        return;
    }
    if (signatureKey) {
        state.renderSignatures[signatureKey] = signature;
    }
    if (!entries.length) {
        target.innerHTML = `<div class="group-card"><div class="group-title">${escapeHtml(t("groups.empty"))}</div></div>`;
        return;
    }
    target.innerHTML = entries.map(([title, values]) => `
    <div class="group-card">
      <div class="group-title">${escapeHtml(title)}</div>
      <div class="group-list">
        ${Object.entries(values).map(([key, count]) => `
          <span class="tag-pill">${escapeHtml(key)}: ${escapeHtml(count)}</span>
        `).join("")}
      </div>
    </div>
  `).join("");
}
function renderAttentionReasons(summary) {
    const entries = Object.entries(summary.reasonCounts || {});
    const signature = [state.language, ...entries.map(([reason, count]) => `${reason}:${count}`)].join("::");
    if (state.renderSignatures.attentionReasons === signature) {
        return;
    }
    state.renderSignatures.attentionReasons = signature;
    if (!entries.length) {
        nodes.attentionReasons.innerHTML = `<div class="reason-line">${escapeHtml(t("attention.noReasons"))}</div>`;
        return;
    }
    nodes.attentionReasons.innerHTML = entries.map(([reason, count]) => `
    <div class="reason-line"><strong>${escapeHtml(attentionReasonLabel(reason))}</strong> · ${escapeHtml(count)} ${escapeHtml(t("metrics.runtimes"))}</div>
  `).join("");
}
function renderPersistence(capabilities) {
    const persistence = capabilities.persistence || {
        enabled: false,
        schemaVersion: null,
        statePath: t("persistence.unknown"),
        backupStatePath: t("persistence.unknown"),
        lastSavedAt: null,
        isDirty: false,
        lastSaveError: null,
        restoredRuntimeCount: 0,
        restoredSessionCount: 0,
        restoredFromSavedAt: null,
    };
    const cards = [
        [t("persistence.enabled"), persistence.enabled ? t("persistence.yes") : t("persistence.no")],
        [t("persistence.schema"), persistence.schemaVersion ?? t("persistence.unknown")],
        [t("persistence.state"), persistence.isDirty ? t("persistence.dirty") : t("persistence.clean")],
        [t("persistence.stateFile"), persistence.statePath ? t("persistence.configured") : t("persistence.missing")],
        [t("persistence.lastSaved"), persistence.lastSavedAt || t("persistence.never")],
        [t("persistence.restoredRuntimes"), persistence.restoredRuntimeCount ?? 0],
        [t("persistence.restoredSessions"), persistence.restoredSessionCount ?? 0],
    ];
    const detailSignature = [
        state.language,
        persistence.statePath || "",
        persistence.backupStatePath || "",
        persistence.schemaVersion ?? "",
        persistence.isDirty,
        persistence.lastSavedAt || "",
        persistence.lastSaveError || "",
        persistence.restoredFromSavedAt || "",
    ].join("::");
    renderMetricCards(nodes.persistenceCards, cards, "persistenceCards");
    if (state.renderSignatures.persistenceDetails !== detailSignature) {
        state.renderSignatures.persistenceDetails = detailSignature;
        nodes.persistenceDetails.innerHTML = `
      <div class="hint-line">${escapeHtml(t("persistence.statePath"))}: <strong>${escapeHtml(persistence.statePath || t("persistence.unknown"))}</strong></div>
      <div class="hint-line">${escapeHtml(t("persistence.backupPath"))}: <strong>${escapeHtml(persistence.backupStatePath || t("persistence.unknown"))}</strong></div>
      <div class="hint-line">${escapeHtml(t("persistence.schemaVersion"))}: <strong>${escapeHtml(persistence.schemaVersion ?? t("persistence.unknown"))}</strong></div>
      <div class="hint-line">${escapeHtml(t("persistence.state"))}: <strong>${escapeHtml(persistence.isDirty ? t("persistence.dirty") : t("persistence.clean"))}</strong></div>
      <div class="hint-line">${escapeHtml(t("persistence.lastSavedAt"))}: <strong>${escapeHtml(persistence.lastSavedAt || t("persistence.never"))}</strong></div>
      <div class="hint-line">${escapeHtml(t("persistence.lastSaveError"))}: <strong>${escapeHtml(persistence.lastSaveError || t("persistence.none"))}</strong></div>
      <div class="hint-line">${escapeHtml(t("persistence.restoredFromSave"))}: <strong>${escapeHtml(persistence.restoredFromSavedAt || t("persistence.none"))}</strong></div>
    `;
    }
}
function renderAttentionList(payload) {
    const items = payload.runtimes || [];
    nodes.attentionCount.textContent = `${items.length} ${t("metrics.runtimes")}`;
    const signature = [
        state.language,
        ...items.map((item) => [
            item.runtimeId,
            item.name,
            item.endpoint,
            item.severity,
            item.tags.environment || "",
            item.tags.cluster || "",
            item.tags.role || "",
            (item.reasons || []).join("|"),
            (item.suggestedActions || []).map((action) => `${action.action}:${action.priority}:${action.coolingDown}`).join("|"),
            (item.recentRecoveryActivities || []).map((activity) => `${activity.action}:${activity.outcome}:${activity.recordedAt}`).join("|"),
        ].join("::")),
    ].join("##");
    if (state.renderSignatures.attentionList === signature) {
        return;
    }
    state.renderSignatures.attentionList = signature;
    if (!items.length) {
        nodes.attentionList.innerHTML = `<div class="attention-item"><div class="item-meta">${escapeHtml(t("attention.noRuntimes"))}</div></div>`;
        return;
    }
    nodes.attentionList.innerHTML = items.map((item) => `
    <div class="attention-item ${escapeHtml(item.severity)}">
      <div class="item-head">
        <div>
          <h3>${escapeHtml(item.name)}</h3>
          <div class="item-meta">${escapeHtml(item.endpoint)}</div>
        </div>
        <div class="severity ${escapeHtml(item.severity)}">${escapeHtml(t(`attention.${item.severity}`))}</div>
      </div>
      <div class="item-meta">
        ${escapeHtml(item.tags.environment || t("runtimes.states.noEnv"))} · ${escapeHtml(item.tags.cluster || t("runtimes.states.noCluster"))} · ${escapeHtml(item.tags.role || t("runtimes.states.noRole"))}
      </div>
      <div class="reason-list">
        ${(item.reasons || []).map((reason) => `<span class="reason-pill">${escapeHtml(attentionReasonLabel(reason))}</span>`).join("")}
      </div>
      ${(item.suggestedActions || []).length ? `
        <div class="hint-line"><strong>${escapeHtml(t("attention.suggestedActions"))}</strong>: ${(item.suggestedActions || []).map((action) => `${escapeHtml(recoveryActionLabel(action.action))} (#${escapeHtml(action.priority)})${action.coolingDown ? ` · ${escapeHtml(t("attention.coolingDown"))}` : ""}`).join(" · ")}</div>
      ` : ""}
      ${(item.recentRecoveryActivities || []).length ? `
        <div class="hint-line"><strong>${escapeHtml(t("attention.recentRecovery"))}</strong>: ${escapeHtml(recoveryActionLabel(item.recentRecoveryActivities[0].action))} · ${escapeHtml(recoveryOutcomeLabel(item.recentRecoveryActivities[0].outcome))} · ${escapeHtml(item.recentRecoveryActivities[0].recordedAt)}</div>
      ` : ""}
    </div>
  `).join("");
}
function renderSessions(payload) {
    const items = payload.sessions || [];
    nodes.sessionCount.textContent = `${items.length} ${t("tabs.sessions").toLowerCase()}`;
    const signature = [
        state.language,
        ...items.map((item) => `${item.sessionId || ""}:${item.pipelineKind}:${item.requestedBy}:${item.status}:${item.runtimeId}`),
    ].join("##");
    if (state.renderSignatures.sessions === signature) {
        return;
    }
    state.renderSignatures.sessions = signature;
    if (!items.length) {
        nodes.sessionList.innerHTML = `<div class="session-item"><div class="item-meta">${escapeHtml(t("sessions.none"))}</div></div>`;
        return;
    }
    nodes.sessionList.innerHTML = items.map((item) => `
    <div class="session-item">
      <div class="item-head">
        <div>
          <h3>${escapeHtml(item.pipelineKind)}</h3>
          <div class="item-meta">${escapeHtml(item.requestedBy)}</div>
        </div>
        <div class="chip">${escapeHtml(item.status)}</div>
      </div>
      <div class="hint-line">${escapeHtml(t("sessions.runtime"))}: ${escapeHtml(item.runtimeId)}</div>
    </div>
  `).join("");
}
function attentionMapFromCache() {
    return new Map((state.cache.attentionList?.runtimes || []).map((item) => [item.runtimeId, item]));
}
function renderRuntimeSliceFromCache() {
    if (!state.cache.runtimes) {
        return;
    }
    renderRuntimes(state.cache.runtimes, attentionMapFromCache());
}
function ensureRuntimeSelectionFromCache() {
    const runtimes = state.cache.runtimes?.runtimes || [];
    if (runtimes.some((runtime) => runtime.runtimeId === state.selectedRuntimeId)) {
        return;
    }
    state.selectedRuntimeId = runtimes[0]?.runtimeId || null;
}
function scheduleRuntimeSliceRender() {
    if (state.pendingRuntimeRender) {
        return;
    }
    state.pendingRuntimeRender = window.requestAnimationFrame(() => {
        state.pendingRuntimeRender = 0;
        if (state.activeTab === "runtimes") {
            renderRuntimeSliceFromCache();
        }
    });
}
function isIdleReadyStatus(status) {
    return !!status
        && status.resilienceStatus === "idle_ready"
        && status.resilienceDegraded === false;
}
function runtimeSnapshotLabel(status) {
    if (isIdleReadyStatus(status)) {
        return t("statuses.idleReady");
    }
    if (status.statusSource === "fetch_failed") {
        return t("statuses.fetchFailed");
    }
    if (!status.hasLatestSnapshot) {
        return t("statuses.unobserved");
    }
    return t("statuses.observedSnapshot", { kind: status.snapshotKind || t("statuses.observed") });
}
function statusBadge(status) {
    if (isIdleReadyStatus(status)) {
        return { text: t("statuses.idleReady"), tone: "good" };
    }
    if (status.statusSource === "fetch_failed") {
        return { text: t("statuses.fetchFailed"), tone: "bad" };
    }
    if (!status.hasLatestSnapshot) {
        return { text: t("statuses.unobserved"), tone: "warn" };
    }
    return { text: t("statuses.observedSnapshot", { kind: status.snapshotKind || t("statuses.observed") }), tone: "good" };
}
function sidecarStatusBadge(sidecarStatus) {
    if (!sidecarStatus) {
        return { text: t("register.sidecarUnpaired"), tone: "warn", refreshKind: null };
    }
    if (sidecarStatus.statusSource === "fetch_failed") {
        return { text: t("statuses.sidecarFetchFailed"), tone: "bad", refreshKind: "sidecar" };
    }
    if (sidecarStatus.daemonStatus === "starting") {
        return { text: t("statuses.sidecarStarting"), tone: "warn", refreshKind: "sidecar" };
    }
    if (sidecarStatus.daemonStatus === "degraded") {
        return { text: t("statuses.sidecarDegraded"), tone: "warn", refreshKind: "sidecar" };
    }
    return { text: t("statuses.sidecarObserved"), tone: "good", refreshKind: null };
}
function runtimeStatusHint(status) {
    if (!status) {
        return t("statuses.unobserved");
    }
    if (isIdleReadyStatus(status)) {
        return t("statuses.idleReady");
    }
    if (status.statusSource === "fetch_failed") {
        return t("statuses.fetchFailed");
    }
    if (!status.hasLatestSnapshot) {
        return t("statuses.unobserved");
    }
    return t("statuses.observed");
}
function registrationPlanConflictMessage(plan) {
    if (!plan || plan.allowed)
        return "";
    if (plan.reason === "runtime_deletion_in_progress") {
        return t("register.deletionInProgress");
    }
    if (plan.reason && plan.reason !== "endpoint_conflict") {
        return t("register.planUnavailable", {
            message: plan.reasonMessage || plan.reason,
        });
    }
    return t("register.blockedDuplicate", {
        reason: t("register.duplicateEndpoint"),
        name: plan.existingRuntimeName,
        endpoint: plan.existingRuntimeEndpoint,
    });
}
function registrationPlanDraft() {
    return {
        name: nodes.registerName.value.trim(),
        endpoint: nodes.registerEndpoint.value.trim(),
        sidecarEndpoint: nodes.registerSidecarEndpoint.value.trim() || null,
    };
}
function registrationPlanDraftKey(draft = registrationPlanDraft()) {
    return [draft.name, draft.endpoint, draft.sidecarEndpoint || ""].join("::");
}
function currentRegistrationPlan() {
    const plan = state.registrationPlan;
    return plan?.draftKey === registrationPlanDraftKey() ? plan : null;
}
function registrationReadiness(endpointValid, sidecarEndpointValid) {
    const name = nodes.registerName.value.trim();
    const endpoint = nodes.registerEndpoint.value.trim();
    const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
    const pairingToken = nodes.registerToken.value.trim();
    const plan = currentRegistrationPlan();
    if (!name) {
        return {
            plan,
            ready: false,
            field: nodes.registerName,
            tone: "pending",
            message: t("register.completeField", { field: t("register.name") }),
        };
    }
    if (!endpoint) {
        return {
            plan,
            ready: false,
            field: nodes.registerEndpoint,
            tone: "pending",
            message: t("register.completeField", { field: t("register.endpoint") }),
        };
    }
    if (!endpointValid) {
        return { plan, ready: false, field: nodes.registerEndpoint, tone: "bad", message: t("register.blockedEndpoint") };
    }
    if (!pairingToken) {
        return {
            plan,
            ready: false,
            field: nodes.registerToken,
            tone: "pending",
            message: t("register.completeField", { field: t("register.pairingToken") }),
        };
    }
    if (sidecarEndpoint && !sidecarEndpointValid) {
        return { plan, ready: false, field: nodes.registerSidecarEndpoint, tone: "bad", message: t("register.blockedSidecarEndpoint") };
    }
    if (state.registrationPlanError) {
        return {
            plan,
            ready: false,
            field: nodes.registerEndpoint,
            tone: "bad",
            message: t("register.planUnavailable", { message: state.registrationPlanError }),
        };
    }
    if (!plan) {
        return { plan, ready: false, field: null, tone: "pending", message: t("register.checkingPlan") };
    }
    if (!plan.allowed) {
        return {
            plan,
            ready: false,
            field: nodes.registerEndpoint,
            tone: "bad",
            message: registrationPlanConflictMessage(plan),
        };
    }
    return { plan, ready: true, field: null, tone: "good", message: t("register.ready") };
}
function setRegisterResult(message, tone = "neutral", focus = false) {
    nodes.registerResult.textContent = message;
    nodes.registerResult.dataset.tone = tone;
    if (focus) {
        window.requestAnimationFrame(() => nodes.registerResult.focus({ preventScroll: false }));
    }
}
function revealRegistrationField(field) {
    if (!field)
        return;
    if (nodes.registerSidecarDetails?.contains(field)) {
        nodes.registerSidecarDetails.open = true;
    }
    field.setAttribute("aria-invalid", "true");
    window.requestAnimationFrame(() => {
        field.focus({ preventScroll: true });
        field.scrollIntoView({ behavior: "smooth", block: "center" });
    });
}
function showRegistrationIssue(issue) {
    state.activeTab = "runtimes";
    state.activeRuntimeMainTab = "register";
    applyTabShell();
    setRegisterResult(issue.message, "bad");
    revealRegistrationField(issue.field);
}
function setRegistrationSecretVisibility(input, toggle, label, visible) {
    if (!input || !toggle || !label)
        return;
    input.type = visible ? "text" : "password";
    toggle.setAttribute("aria-pressed", String(visible));
    label.textContent = t(visible ? "register.hideToken" : "register.showToken");
}
function syncRegistrationSecretToggles() {
    setRegistrationSecretVisibility(nodes.registerToken, nodes.registerTokenToggle, nodes.registerTokenToggleLabel, nodes.registerToken.type === "text");
    setRegistrationSecretVisibility(nodes.registerSidecarAdminToken, nodes.registerSidecarAdminTokenToggle, nodes.registerSidecarAdminTokenToggleLabel, nodes.registerSidecarAdminToken.type === "text");
}
function maskRegistrationSecrets() {
    setRegistrationSecretVisibility(nodes.registerToken, nodes.registerTokenToggle, nodes.registerTokenToggleLabel, false);
    setRegistrationSecretVisibility(nodes.registerSidecarAdminToken, nodes.registerSidecarAdminTokenToggle, nodes.registerSidecarAdminTokenToggleLabel, false);
}
function clearRegistrationSecrets() {
    nodes.registerToken.value = "";
    nodes.registerSidecarAdminToken.value = "";
    maskRegistrationSecrets();
}
async function loadRegistrationPlan() {
    const draft = registrationPlanDraft();
    const draftKey = registrationPlanDraftKey(draft);
    if (!draft.name || !isLikelyHttpEndpoint(draft.endpoint) ||
        (draft.sidecarEndpoint && !isLikelyHttpEndpoint(draft.sidecarEndpoint))) {
        state.registrationPlan = null;
        renderRegisterPreview();
        return;
    }
    state.registrationPlanAbortController?.abort();
    const abortController = new AbortController();
    state.registrationPlanAbortController = abortController;
    try {
        const plan = await postJsonBody("/v1/runtimes/registration-plan", draft, abortController.signal);
        if (draftKey !== registrationPlanDraftKey())
            return;
        state.registrationPlan = { ...plan, draftKey };
        state.registrationPlanError = "";
    }
    catch (error) {
        if (error?.name === "AbortError")
            return;
        if (draftKey !== registrationPlanDraftKey())
            return;
        state.registrationPlan = null;
        state.registrationPlanError = error.message;
    }
    finally {
        if (state.registrationPlanAbortController === abortController) {
            state.registrationPlanAbortController = null;
        }
        renderRegisterPreview();
    }
}
function scheduleRegistrationPlan() {
    window.clearTimeout(state.registrationPlanTimer);
    state.registrationPlan = null;
    state.registrationPlanError = "";
    state.registrationPlanTimer = window.setTimeout(() => void loadRegistrationPlan(), 250);
}
function isLikelyHttpEndpoint(endpoint) {
    if (!(endpoint.startsWith("http://") || endpoint.startsWith("https://"))) {
        return false;
    }
    try {
        const parsed = new URL(endpoint);
        return (parsed.protocol === "http:" || parsed.protocol === "https:")
            && !!parsed.hostname
            && !parsed.username
            && !parsed.password;
    }
    catch {
        return false;
    }
}
function suggestedRuntimeName(endpoint) {
    try {
        const parsed = new URL(endpoint);
        const hostBits = parsed.hostname
            .split(".")
            .filter(Boolean)
            .slice(0, 4)
            .map((bit) => bit.replace(/[^a-zA-Z0-9-]/g, "-"))
            .filter(Boolean);
        const portBit = parsed.port ? `-${parsed.port}` : "";
        const hostPart = hostBits.length ? hostBits.join("-").toLowerCase() : "runtime";
        return `gw-${hostPart}${portBit}`;
    }
    catch {
        return "";
    }
}
function maybePrefillRuntimeNameFromEndpoint() {
    if (state.registerNameTouched) {
        scheduleRegistrationPlanPreview();
        return;
    }
    const endpoint = nodes.registerEndpoint.value.trim();
    if (!isLikelyHttpEndpoint(endpoint)) {
        scheduleRegistrationPlanPreview();
        return;
    }
    const suggestion = suggestedRuntimeName(endpoint);
    if (suggestion) {
        nodes.registerName.value = suggestion;
    }
    scheduleRegistrationPlanPreview();
}
function registerPreviewSignature() {
    const plan = currentRegistrationPlan();
    return [
        state.language,
        nodes.registerName.value.trim(),
        nodes.registerEndpoint.value.trim(),
        nodes.registerSidecarEndpoint.value.trim(),
        nodes.registerSidecarAdminToken.value.trim() ? "protected" : "open",
        nodes.registerToken.value.trim() ? "paired" : "missing-token",
        nodes.registerRuntimeEnvironment.value.trim(),
        nodes.registerRuntimeCluster.value.trim(),
        nodes.registerRuntimeRole.value.trim(),
        nodes.registerFetchCapabilities.checked ? "fetch" : "skip",
        plan?.planToken || state.registrationPlanError || "plan-pending",
    ].join("::");
}
function syncRegisterSubmitState(endpointValid, sidecarEndpointValid) {
    const endpoint = nodes.registerEndpoint.value.trim();
    const busy = state.uiActions.has("register-runtime");
    const readiness = registrationReadiness(endpointValid, sidecarEndpointValid);
    nodes.registerEndpoint.setAttribute("aria-invalid", endpoint && !endpointValid ? "true" : "false");
    nodes.registerSidecarEndpoint.setAttribute("aria-invalid", nodes.registerSidecarEndpoint.value.trim() && !sidecarEndpointValid ? "true" : "false");
    nodes.registerGuidance.textContent = readiness.message;
    nodes.registerGuidance.dataset.tone = busy ? "pending" : readiness.tone;
    nodes.registerSubmit.disabled = busy || !readiness.ready;
    nodes.registerForm.dataset.ready = readiness.ready ? "true" : "false";
    return { ...readiness, valid: readiness.ready };
}
function scheduleRenderRegisterPreview() {
    if (state.pendingRegisterPreview) {
        return;
    }
    state.pendingRegisterPreview = window.requestAnimationFrame(() => {
        state.pendingRegisterPreview = 0;
        renderRegisterPreview();
    });
}
function scheduleRegistrationPlanPreview() {
    scheduleRenderRegisterPreview();
    scheduleRegistrationPlan();
}
function renderRegisterPreview() {
    const endpoint = nodes.registerEndpoint.value.trim();
    const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
    const endpointValid = endpoint.length > 0 && isLikelyHttpEndpoint(endpoint);
    const sidecarEndpointValid = sidecarEndpoint.length > 0 ? isLikelyHttpEndpoint(sidecarEndpoint) : true;
    const submission = syncRegisterSubmitState(endpointValid, sidecarEndpointValid);
    const signature = registerPreviewSignature();
    if (state.renderSignatures.registerPreview === signature) {
        return;
    }
    state.renderSignatures.registerPreview = signature;
    const sidecarAdminToken = nodes.registerSidecarAdminToken.value.trim();
    const pairingTokenReady = !!nodes.registerToken.value.trim();
    const explicitName = nodes.registerName.value.trim();
    const suggestedName = endpointValid ? suggestedRuntimeName(endpoint) : "";
    const effectiveName = explicitName || suggestedName || t("register.pendingRuntimeName");
    const endpointState = endpoint.length === 0
        ? t("register.endpointPending")
        : endpointValid ? t("register.endpointValid") : t("register.endpointInvalid");
    const sidecarState = sidecarEndpoint.length === 0
        ? t("register.sidecarUnpaired")
        : sidecarEndpointValid ? t("register.endpointValid") : t("register.endpointInvalid");
    const sidecarAccess = sidecarEndpoint.length === 0
        ? t("register.sidecarUnpaired")
        : sidecarAdminToken ? t("runtimeDetail.sidecarProtected") : t("runtimeDetail.sidecarOpen");
    const slice = [
        nodes.registerRuntimeEnvironment.value.trim(),
        nodes.registerRuntimeCluster.value.trim(),
        nodes.registerRuntimeRole.value.trim(),
    ].filter(Boolean).join(" / ") || t("register.allRuntimes");
    nodes.registerPreview.innerHTML = `
    <div class="register-preview-head">
      <strong>${escapeHtml(t("register.previewTitle"))}</strong>
      ${!explicitName && suggestedName ? `<span class="tag-pill">${escapeHtml(t("register.suggested"))}</span>` : ""}
    </div>
    <div class="register-preview-grid">
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewName"))}</span>
        <strong>${escapeHtml(effectiveName)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSlice"))}</span>
        <strong>${escapeHtml(slice)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewEndpoint"))}</span>
        <strong class="register-preview-state ${endpointValid ? "good" : endpoint ? "bad" : "pending"}">${escapeHtml(endpointState)}</strong>
        ${endpoint ? `<div class="register-preview-meta">${escapeHtml(endpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecar"))}</span>
        <strong class="register-preview-state ${sidecarEndpoint ? sidecarEndpointValid ? "good" : "bad" : "pending"}">${escapeHtml(sidecarState)}</strong>
        ${sidecarEndpoint ? `<div class="register-preview-meta">${escapeHtml(sidecarEndpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecarAccess"))}</span>
        <strong>${escapeHtml(sidecarAccess)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewPairing"))}</span>
        <strong class="register-preview-state ${pairingTokenReady ? "good" : "bad"}">${escapeHtml(t(pairingTokenReady ? "register.pairingReady" : "register.pairingMissing"))}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewCapabilityFetch"))}</span>
        <strong>${escapeHtml(nodes.registerFetchCapabilities.checked ? t("register.capabilityEnabled") : t("register.capabilityDisabled"))}</strong>
      </div>
    </div>
    ${submission.plan && !submission.plan.allowed ? `<div class="register-preview-warning">${escapeHtml(registrationPlanConflictMessage(submission.plan))}</div>` : ""}
    ${state.registrationPlanError ? `<div class="register-preview-warning">${escapeHtml(state.registrationPlanError)}</div>` : ""}
  `;
}
function runtimeTableSignature(items, attentionMap) {
    return [
        state.language,
        state.runtimeSearch.trim().toLowerCase(),
        state.runtimeSort,
        ...items.map((runtime) => {
            const attention = attentionMap.get(runtime.runtimeId);
            const capabilityKeys = (runtime.capabilities || [])
                .map((item) => `${item.key}:${item.support}`)
                .sort()
                .join("|");
            const sidecarBits = [
                runtime.sidecarEndpoint || "",
                runtime.hasSidecarAdminToken ? "protected" : "open",
                runtime.sidecarStatus?.Healthy ? "healthy" : "",
                runtime.sidecarStatus?.HasEvidenceChainEnrichment ? "enrichment" : "",
                runtime.sidecarStatus?.HasDiagnosticOpinion ? "opinion" : "",
                runtime.status.hasExternalSidecarContext ? "context" : "",
                runtime.status.hasExternalDiagnosticOpinion ? "merged-opinion" : "",
            ].join("|");
            const attentionBits = attention
                ? `${attention.severity}:${attention.needsAttention}:${(attention.reasons || []).join("|")}`
                : "clear";
            return [
                runtime.runtimeId,
                runtime.name,
                runtime.endpoint,
                runtime.tags.environment || "",
                runtime.tags.cluster || "",
                runtime.tags.role || "",
                runtime.status.statusSource || "",
                runtime.status.resilienceStatus || "",
                runtime.status.socketServiceStatus || "",
                runtime.status.snapshotKind || "",
                capabilityKeys,
                sidecarBits,
                attentionBits,
            ].join("::");
        }),
    ].join("##");
}
function updateRuntimeTableSelection(selectedRuntimeId) {
    for (const row of nodes.runtimeTableBody.querySelectorAll("tr[data-runtime-id]")) {
        if (!(row instanceof HTMLTableRowElement))
            continue;
        const isSelected = row.dataset.runtimeId === selectedRuntimeId;
        row.classList.toggle("selected", isSelected);
        row.setAttribute("aria-selected", String(isSelected));
        row.tabIndex = isSelected ? 0 : -1;
    }
}
function runtimeActionLabel(actionKey, runtimeName) {
    return `${t(actionKey)}: ${runtimeName}`;
}
function renderRuntimes(payload, attentionMap) {
    const allItems = payload.runtimes || [];
    state.latestRuntimes = allItems;
    if (state.activeRuntimeMainTab === "register") {
        return;
    }
    const listVisible = state.activeRuntimeMainTab === "select";
    const query = listVisible ? state.runtimeSearch.trim().toLowerCase() : "";
    const filteredItems = query
        ? allItems.filter((runtime) => runtime.name.toLowerCase().includes(query) ||
            runtime.endpoint.toLowerCase().includes(query))
        : allItems;
    const items = listVisible
        ? [...filteredItems].sort((left, right) => {
            if (state.runtimeSort === "status") {
                return (left.status.statusSource || "").localeCompare(right.status.statusSource || "") ||
                    left.name.localeCompare(right.name);
            }
            if (state.runtimeSort === "snapshot") {
                return (left.status.snapshotKind || "").localeCompare(right.status.snapshotKind || "") ||
                    left.name.localeCompare(right.name);
            }
            return left.name.localeCompare(right.name);
        })
        : allItems;
    if (listVisible) {
        nodes.runtimeCount.textContent = `${items.length} ${t("metrics.runtimes")}`;
    }
    if (!items.length) {
        state.selectedRuntimeId = null;
        if (listVisible) {
            const emptySignature = `empty::${state.language}::${state.runtimeSearch.trim().toLowerCase()}::${state.runtimeSort}`;
            if (state.renderSignatures.runtimeTable !== emptySignature) {
                state.renderSignatures.runtimeTable = emptySignature;
                nodes.runtimeTableBody.innerHTML = `<tr class="runtime-empty-row"><td colspan="7">${escapeHtml(t("runtimes.noMatch"))}</td></tr>`;
            }
        }
        if (state.activeRuntimeMainTab === "detail") {
            renderRuntimeDetail(null, null);
        }
        else if (state.activeRuntimeMainTab === "panel") {
            renderRuntimePanel(null);
        }
        return;
    }
    if (!items.some((item) => item.runtimeId === state.selectedRuntimeId)) {
        state.selectedRuntimeId = items[0].runtimeId;
    }
    if (listVisible) {
        const tableSignature = runtimeTableSignature(items, attentionMap);
        if (state.renderSignatures.runtimeTable !== tableSignature) {
            state.renderSignatures.runtimeTable = tableSignature;
            nodes.runtimeTableBody.innerHTML = items.map((runtime) => {
                const isSelected = runtime.runtimeId === state.selectedRuntimeId;
                const badge = statusBadge(runtime.status);
                const attention = attentionMap.get(runtime.runtimeId);
                const capabilityKeys = (runtime.capabilities || [])
                    .filter((item) => item.support === "fully_supported")
                    .map((item) => item.key);
                const compactCapabilitySummary = capabilityKeys.length
                    ? t("runtimes.states.capabilitiesCount", { count: capabilityKeys.length })
                    : t("runtimes.states.noCapabilities");
                const sidecarBits = [
                    runtime.sidecarEndpoint ? "paired" : null,
                    runtime.hasSidecarAdminToken ? t("runtimes.states.protected") : null,
                    runtime.sidecarStatus?.Healthy ? "healthy" : null,
                    runtime.sidecarStatus?.HasEvidenceChainEnrichment ? "enrichment" : null,
                    runtime.sidecarStatus?.HasDiagnosticOpinion ? "opinion" : null,
                    runtime.status.hasExternalSidecarContext ? "context" : null,
                    runtime.status.hasExternalDiagnosticOpinion ? "merged-opinion" : null,
                ].filter(Boolean);
                return `
        <tr class="${isSelected ? "selected" : ""}"
            data-runtime-id="${escapeHtml(runtime.runtimeId)}"
            aria-selected="${String(isSelected)}"
            tabindex="${isSelected ? 0 : -1}">
          <td data-runtime-cell="identity" data-label="${escapeHtml(t("runtimes.columns.name"))}">
            <strong>${escapeHtml(runtime.name)}</strong>
            <div class="item-meta">${escapeHtml(runtime.endpoint)}</div>
          </td>
          <td data-runtime-cell="tags" data-label="${escapeHtml(t("runtimes.columns.tags"))}">
            <div class="runtime-tags">
              <span class="tag-pill">${escapeHtml(runtime.tags.environment || t("runtimes.states.noEnv"))}</span>
              <span class="tag-pill">${escapeHtml(runtime.tags.cluster || t("runtimes.states.noCluster"))}</span>
              <span class="tag-pill">${escapeHtml(runtime.tags.role || t("runtimes.states.noRole"))}</span>
            </div>
          </td>
          <td data-runtime-cell="status" data-label="${escapeHtml(t("runtimes.columns.status"))}">
            <span class="runtime-state ${escapeHtml(badge.tone)}">${escapeHtml(badge.text)}</span>
            <div class="item-meta">${escapeHtml(t("runtimeDetail.source"))}: ${escapeHtml(runtime.status.statusSource)}</div>
            ${runtime.status.resilienceStatus ? `<div class="item-meta">${escapeHtml(t("runtimeDetail.resilienceStatus"))}: ${escapeHtml(runtime.status.resilienceStatus)}</div>` : ""}
          </td>
          <td data-runtime-cell="capabilities" data-label="${escapeHtml(t("runtimes.columns.capabilitySurface"))}">
            <div class="runtime-surface">
              <div class="runtime-surface-compact item-meta">${escapeHtml(compactCapabilitySummary)}</div>
              <div class="runtime-surface-pills">
                ${capabilityKeys.length ? capabilityKeys.map((key) => `<span class="tag-pill">${escapeHtml(key)}</span>`).join("") : `<span class="item-meta">${escapeHtml(t("runtimes.states.noCapabilities"))}</span>`}
              </div>
            </div>
          </td>
          <td data-runtime-cell="sidecar" data-label="${escapeHtml(t("runtimes.columns.sidecar"))}">
            <div class="runtime-sidecar">
              ${sidecarBits.length ? sidecarBits.map((bit) => `<span class="tag-pill">${escapeHtml(bit)}</span>`).join("") : `<span class="item-meta">${escapeHtml(t("runtimes.states.none"))}</span>`}
            </div>
          </td>
          <td data-runtime-cell="attention" data-label="${escapeHtml(t("runtimes.columns.attention"))}">
            <div class="runtime-attention">
              ${attention
                    ? `<span class="runtime-state ${attention.severity === "critical" ? "bad" : "warn"}">${escapeHtml(t(`attention.${attention.severity}`))}</span>
                   ${(attention.reasons || []).map((reason) => `<span class="tag-pill">${escapeHtml(attentionReasonLabel(reason))}</span>`).join("")}`
                    : `<span class="runtime-state good">${escapeHtml(t("runtimes.states.clear"))}</span>`}
            </div>
          </td>
          <td data-runtime-cell="actions" data-label="${escapeHtml(t("runtimes.columns.actions"))}">
            <div class="inline-actions">
              <button type="button" data-action="open-panel" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.openPanel", runtime.name))}">${escapeHtml(t("runtimes.actions.openPanel"))}</button>
              <button type="button" data-action="show-attention" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.attention", runtime.name))}">${escapeHtml(t("runtimes.actions.attention"))}</button>
              <button type="button" data-action="refresh-status" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.status", runtime.name))}">${escapeHtml(t("runtimes.actions.status"))}</button>
              ${runtime.sidecarEndpoint ? `<button type="button" data-action="refresh-sidecar" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimeDetail.refreshSidecar", runtime.name))}">${escapeHtml(t("runtimeDetail.refreshSidecar"))}</button>` : ""}
              <button type="button" data-action="refresh-all" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.all", runtime.name))}">${escapeHtml(t("runtimes.actions.all"))}</button>
              <button type="button" data-action="delete-runtime" data-runtime-id="${escapeHtml(runtime.runtimeId)}" data-runtime-name="${escapeHtml(runtime.name)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.delete", runtime.name))}">${escapeHtml(t("runtimes.actions.delete"))}</button>
            </div>
            <details class="runtime-row-menu">
              <summary class="quiet" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.menu", runtime.name))}">${escapeHtml(t("runtimes.actions.menu"))}</summary>
              <div class="runtime-row-menu-panel">
                <button type="button" data-action="open-panel" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.openPanel", runtime.name))}">${escapeHtml(t("runtimes.actions.openPanel"))}</button>
                <button type="button" data-action="show-attention" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.attention", runtime.name))}">${escapeHtml(t("runtimes.actions.attention"))}</button>
                <button type="button" data-action="refresh-status" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.status", runtime.name))}">${escapeHtml(t("runtimes.actions.status"))}</button>
                ${runtime.sidecarEndpoint ? `<button type="button" data-action="refresh-sidecar" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimeDetail.refreshSidecar", runtime.name))}">${escapeHtml(t("runtimeDetail.refreshSidecar"))}</button>` : ""}
                <button type="button" data-action="refresh-all" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.all", runtime.name))}">${escapeHtml(t("runtimes.actions.all"))}</button>
                <button type="button" data-action="delete-runtime" data-runtime-id="${escapeHtml(runtime.runtimeId)}" data-runtime-name="${escapeHtml(runtime.name)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.delete", runtime.name))}">${escapeHtml(t("runtimes.actions.delete"))}</button>
              </div>
            </details>
          </td>
        </tr>
      `;
            }).join("");
        }
        else {
            updateRuntimeTableSelection(state.selectedRuntimeId);
        }
    }
    const selectedRuntime = items.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
    const selectedAttention = selectedRuntime
        ? state.runtimeAttentionById.get(selectedRuntime.runtimeId) || attentionMap.get(selectedRuntime.runtimeId) || null
        : null;
    if (state.activeRuntimeMainTab === "detail") {
        renderRuntimeDetail(selectedRuntime, selectedAttention);
    }
    else if (state.activeRuntimeMainTab === "panel") {
        renderRuntimePanel(selectedRuntime);
    }
}
function orchestraReasonLabel(reason) {
    if (!reason) {
        return "clear";
    }
    return attentionReasonLabel(reason);
}
function orchestraTagLabel(tags) {
    const parts = [tags?.environment, tags?.cluster, tags?.role].filter(Boolean);
    return parts.length ? parts.join(" / ") : "unscoped runtime";
}
function orchestraTimestamp(value) {
    const parsed = new Date(value);
    return Number.isNaN(parsed.getTime()) ? `${value || "unknown"}` : parsed.toLocaleString();
}
function orchestraRequestId(key) {
    if (!state.orchestraRequestIds[key]) {
        const random = globalThis.crypto?.randomUUID?.()
            || `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
        state.orchestraRequestIds[key] = `ui:${random}`;
    }
    return state.orchestraRequestIds[key];
}
function renderOrchestraPlan(payload) {
    state.orchestraPlan = payload;
    if (!payload) {
        state.renderSignatures.orchestraPanel = "empty";
        nodes.orchestraSummary.textContent = "Select a runtime to build an operator-facing orchestration plan.";
        nodes.orchestraPlans.innerHTML = `
      <div class="group-card">
        <div class="group-title">No runtime selected</div>
        <div class="hint-line">Use the runtimes workspace to choose a runtime first, then return here for an orchestration plan.</div>
      </div>
    `;
        return;
    }
    const signature = JSON.stringify(payload);
    if (state.renderSignatures.orchestraPanel === signature) {
        return;
    }
    state.renderSignatures.orchestraPanel = signature;
    nodes.orchestraSummary.textContent =
        `${payload.name} · ${payload.endpoint} · ${payload.statusSource} · ${payload.attentionSeverity}`;
    nodes.orchestraPlans.innerHTML = payload.plans.map((plan) => `
    <article class="group-card orchestra-plan-card">
      <div class="group-title">${escapeHtml(plan.title)}</div>
      <div class="hint-line">${escapeHtml(plan.summary)}</div>
      <div class="runtime-tags orchestra-plan-meta">
        <span class="tag-pill">intent: ${escapeHtml(plan.intent)}</span>
        <span class="tag-pill">risk: ${escapeHtml(plan.riskLevel)}</span>
        <span class="tag-pill">readiness: ${escapeHtml(plan.executionReadiness)}</span>
        <span class="tag-pill">mode: ${escapeHtml(plan.executionMode)}</span>
        <span class="tag-pill">approval: ${escapeHtml(plan.approvalMode)}</span>
        <span class="tag-pill">revision: ${escapeHtml(plan.revision)}</span>
        <span class="tag-pill">scope: ${escapeHtml(orchestraTagLabel(payload.tags))}</span>
      </div>
      ${plan.reasons?.length ? `
        <div class="hint-line"><strong>Attention reasons</strong>: ${(plan.reasons || []).map((reason) => `<span class="reason-pill">${escapeHtml(orchestraReasonLabel(reason))}</span>`).join(" ")}</div>
      ` : ""}
      ${plan.requiredCapabilities?.length ? `
        <div class="hint-line"><strong>Required capabilities</strong>: ${(plan.requiredCapabilities || []).map((capability) => `<span class="tag-pill">${escapeHtml(capability)}</span>`).join(" ")}</div>
      ` : ""}
      <ol class="orchestra-step-list">
        ${(plan.steps || []).map((step) => `
          <li class="orchestra-step">
            <strong>${escapeHtml(step.title)}</strong>
            <div class="item-meta">${escapeHtml(step.kind)}</div>
            <div class="hint-line">${escapeHtml(step.detail)}</div>
          </li>
        `).join("")}
      </ol>
      ${plan.executionMode === "automatic" && plan.approvalMode === "operator_confirmation" ? `
        <div class="orchestra-approval-form" data-orchestra-approval-form>
          <label>
            <span>Approved by <small>operator-provided attribution</small></span>
            <input type="text" data-orchestra-approved-by value="leserpent-operator" maxlength="80" autocomplete="off" />
          </label>
          <label>
            <span>Approval note</span>
            <textarea data-orchestra-approval-note maxlength="500" rows="2" placeholder="Why is this execution appropriate now?"></textarea>
          </label>
        </div>
      ` : ""}
      ${plan.executionMode === "guided" && plan.planId === "session_preparation" ? `
        <div class="orchestra-guided-form" data-orchestra-session-form>
          <label>
            <span>Pipeline kind</span>
            <input type="text" data-orchestra-pipeline-kind value="diagnostic" maxlength="128" autocomplete="off" />
          </label>
          <label>
            <span>Requested by</span>
            <input type="text" data-orchestra-requested-by value="leserpent-operator" maxlength="80" autocomplete="off" />
          </label>
          <button type="button" data-orchestra-create-session>Create session</button>
        </div>
      ` : ""}
      ${(plan.suggestedSurfaces || []).length ? `
        <div class="runtime-inline-actions orchestra-surface-links">
          ${plan.executionMode === "automatic" ? `
            <button type="button"
              data-orchestra-execute="${escapeHtml(plan.planId)}"
              data-orchestra-revision="${escapeHtml(plan.revision)}"
              data-orchestra-approval="${escapeHtml(plan.approvalMode)}">${plan.approvalMode === "operator_confirmation" ? "Review & run" : "Run plan"}</button>
          ` : ""}
          ${(plan.suggestedSurfaces || []).map((surface) => `
            <a class="quiet" href="${escapeHtml(surface.path)}">${escapeHtml(surface.label)}</a>
          `).join("")}
        </div>
      ` : ""}
    </article>
  `).join("");
}
function renderOrchestraHistory(runs) {
    const normalized = Array.isArray(runs) ? runs : [];
    const signature = JSON.stringify(normalized);
    if (state.renderSignatures.orchestraHistory === signature) {
        return;
    }
    state.renderSignatures.orchestraHistory = signature;
    nodes.orchestraHistorySummary.textContent = normalized.length
        ? `${normalized.length} retained run${normalized.length === 1 ? "" : "s"}`
        : "No runs recorded.";
    nodes.orchestraHistory.innerHTML = normalized.length
        ? normalized.map((run) => `
      <article class="orchestra-run" data-outcome="${escapeHtml(run.outcome)}">
        <div class="item-head">
          <strong>${escapeHtml(run.planId)}</strong>
          <span class="severity ${escapeHtml(["succeeded", "ok"].includes(run.outcome) ? "" : "warning")}">${escapeHtml(run.outcome)}</span>
        </div>
        <div class="item-meta">${escapeHtml(run.runId)} · attempt ${escapeHtml(run.attempt || 1)} · ${escapeHtml(orchestraTimestamp(run.executedAt))}</div>
        <div class="item-meta">actor: ${escapeHtml(run.approvedBy || "unattributed")} · revision: ${escapeHtml(run.planRevision || "legacy")}</div>
        <div class="item-meta">request: ${escapeHtml(run.requestId || "legacy")}</div>
        ${run.approvalNote ? `<div class="orchestra-approval-note">${escapeHtml(run.approvalNote)}</div>` : ""}
        <div class="orchestra-run-steps">
          ${(run.steps || []).map((step) => `
            <div class="orchestra-run-step" data-outcome="${escapeHtml(step.outcome)}">
              <span>${escapeHtml(step.step)}</span>
              <strong>${escapeHtml(step.outcome)}</strong>
              <span class="hint-line">${escapeHtml(step.summary)}</span>
            </div>
          `).join("")}
        </div>
        <div class="runtime-inline-actions orchestra-run-actions">
          <button type="button" class="quiet" data-orchestra-load-events="${escapeHtml(run.runId)}">Timeline</button>
          ${["queued", "running"].includes(run.outcome) ? `
            <button type="button" class="quiet" data-orchestra-cancel-run="${escapeHtml(run.runId)}">Cancel</button>
          ` : ""}
          ${!["queued", "running"].includes(run.outcome) && run.planId !== "session_preparation" ? `
            <button type="button" class="quiet" data-orchestra-retry-run="${escapeHtml(run.runId)}">Retry</button>
          ` : ""}
        </div>
        <div class="orchestra-run-events" data-orchestra-run-events="${escapeHtml(run.runId)}" hidden></div>
      </article>
    `).join("")
        : `<div class="hint-line">Executed automatic plans will appear here.</div>`;
}
async function loadOrchestraRunEvents(runId, button) {
    const runtimeId = state.selectedRuntimeId;
    const container = button?.closest(".orchestra-run")?.querySelector("[data-orchestra-run-events]");
    if (!runtimeId || !runId || !container) {
        return;
    }
    if (container.dataset.loaded === "true") {
        container.hidden = !container.hidden;
        button.textContent = container.hidden ? "Timeline" : "Hide timeline";
        return;
    }
    button.disabled = true;
    button.textContent = "Loading...";
    try {
        const payload = await getJson(`/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs/${encodeURIComponent(runId)}/events`);
        const events = Array.isArray(payload.events) ? payload.events : [];
        container.innerHTML = events.length
            ? `<ol class="orchestra-event-list">${events.map((item) => `
          <li class="orchestra-event">
            <span class="orchestra-event-marker" aria-hidden="true"></span>
            <div>
              <strong>${escapeHtml(item.fromOutcome ? `${item.fromOutcome} → ${item.toOutcome}` : item.toOutcome)}</strong>
              <span class="item-meta">${escapeHtml(item.eventType)} · ${escapeHtml(orchestraTimestamp(item.recordedAt))}</span>
              <div class="hint-line">${escapeHtml(item.summary)}</div>
            </div>
          </li>
        `).join("")}</ol>`
            : `<div class="hint-line">No append-only events were recorded for this legacy run.</div>`;
        container.dataset.loaded = "true";
        container.hidden = false;
        button.textContent = "Hide timeline";
    }
    catch (error) {
        console.error(error);
        nodes.statusLine.textContent = `Run timeline failed: ${error.message}`;
        button.textContent = "Retry timeline";
    }
    finally {
        button.disabled = false;
    }
}
function renderOrchestraFleetBoard(payload) {
    const signature = JSON.stringify(payload);
    if (state.renderSignatures.orchestraFleetBoard === signature) {
        return;
    }
    state.renderSignatures.orchestraFleetBoard = signature;
    nodes.orchestraFleetCount.textContent = `${payload.runCount} runs`;
    const metrics = [
        ["Active", payload.activeCount],
        ["Failed", payload.failedCount],
        ["Degraded", payload.degradedCount],
        ["Retryable", payload.retryableCount],
        ["Runtimes", payload.runtimeCount],
    ];
    nodes.orchestraFleetMetrics.innerHTML = metrics.map(([label, value]) => `
    <div class="metric">
      <div class="metric-label">${escapeHtml(label)}</div>
      <div class="metric-value">${escapeHtml(value)}</div>
    </div>
  `).join("");
    const recent = (payload.runs || []).slice(0, 20);
    nodes.orchestraFleetRuns.innerHTML = recent.length
        ? recent.map((item) => `
      <button type="button" class="orchestra-fleet-run" data-outcome="${escapeHtml(item.run.outcome)}" data-orchestra-runtime-id="${escapeHtml(item.runtimeId)}">
        <span class="orchestra-fleet-runtime">
          <strong>${escapeHtml(item.runtimeName)}</strong>
          <small>${escapeHtml(orchestraTagLabel(item.tags))}</small>
        </span>
        <span>${escapeHtml(item.run.planId)}</span>
        <span class="severity">${escapeHtml(item.run.outcome)}</span>
        <span class="item-meta">${escapeHtml(orchestraTimestamp(item.run.executedAt))}</span>
      </button>
    `).join("")
        : `<div class="hint-line">No Orchestra runs have been recorded yet.</div>`;
}
function scheduleOrchestraFleetPoll(payload) {
    if (state.orchestraFleetPollTimer) {
        window.clearTimeout(state.orchestraFleetPollTimer);
        state.orchestraFleetPollTimer = 0;
    }
    if (!payload.activeCount || state.activeTab !== "orchestra" || document.hidden) {
        return;
    }
    state.orchestraFleetPollTimer = window.setTimeout(() => {
        state.orchestraFleetPollTimer = 0;
        if (state.activeTab === "orchestra" && !document.hidden) {
            void loadOrchestraFleetBoard();
        }
    }, 1000);
}
async function loadOrchestraFleetBoard() {
    try {
        const payload = await getJson("/v1/orchestra/runs");
        renderOrchestraFleetBoard(payload);
        scheduleOrchestraFleetPoll(payload);
    }
    catch (error) {
        console.error(error);
        nodes.orchestraFleetCount.textContent = "Fleet board unavailable";
    }
}
function scheduleOrchestraHistoryPoll(runtimeId, runs) {
    if (state.orchestraPollTimer) {
        window.clearTimeout(state.orchestraPollTimer);
        state.orchestraPollTimer = 0;
    }
    if (state.activeTab !== "orchestra"
        || document.hidden
        || !runs.some((run) => ["queued", "running"].includes(run.outcome))) {
        return;
    }
    state.orchestraPollTimer = window.setTimeout(() => {
        state.orchestraPollTimer = 0;
        if (state.activeTab === "orchestra" && !document.hidden && runtimeId === state.selectedRuntimeId) {
            void loadOrchestraHistory(runtimeId);
        }
    }, 1000);
}
function clearOrchestraPollTimers() {
    if (state.orchestraPollTimer) {
        window.clearTimeout(state.orchestraPollTimer);
        state.orchestraPollTimer = 0;
    }
    if (state.orchestraFleetPollTimer) {
        window.clearTimeout(state.orchestraFleetPollTimer);
        state.orchestraFleetPollTimer = 0;
    }
}
async function loadOrchestraHistory(runtimeId = state.selectedRuntimeId) {
    if (!runtimeId) {
        renderOrchestraHistory([]);
        return;
    }
    try {
        const payload = await getJson(`/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs`);
        if (runtimeId === state.selectedRuntimeId) {
            renderOrchestraHistory(payload.runs);
            scheduleOrchestraHistoryPoll(runtimeId, payload.runs || []);
        }
    }
    catch (error) {
        console.error(error);
        if (runtimeId === state.selectedRuntimeId) {
            nodes.orchestraHistorySummary.textContent = "Run history unavailable.";
        }
    }
}
async function executeOrchestraPlan(planId, revision, approvalMode, approvedBy, approvalNote) {
    const runtimeId = state.selectedRuntimeId;
    if (!runtimeId || !planId) {
        return;
    }
    if (approvalMode === "operator_confirmation" && (!approvedBy || !approvalNote)) {
        nodes.statusLine.textContent = "Approved by and approval note are required for this plan.";
        return;
    }
    const confirmed = approvalMode !== "operator_confirmation" || window.confirm(`Approve Orchestra plan ${planId}?\n\nRisk-aware execution requires operator confirmation.\nRevision: ${revision}`);
    if (!confirmed) {
        return;
    }
    const button = nodes.orchestraPlans.querySelector(`[data-orchestra-execute="${CSS.escape(planId)}"]`);
    if (button) {
        button.disabled = true;
        button.textContent = "Running...";
    }
    nodes.statusLine.textContent = `Running orchestra plan ${planId}...`;
    const requestKey = `${runtimeId}:${planId}:execute`;
    const requestId = orchestraRequestId(requestKey);
    try {
        const result = await postJsonBody(`/v1/orchestra/plans/${encodeURIComponent(runtimeId)}/${encodeURIComponent(planId)}/execute`, {
            confirmed,
            expectedRevision: revision,
            approvedBy: approvalMode === "operator_confirmation" ? approvedBy : "automatic",
            approvalNote: approvalMode === "operator_confirmation" ? approvalNote : null,
            requestId,
        });
        if (runtimeId !== state.selectedRuntimeId) {
            return;
        }
        await loadOrchestraHistory(runtimeId);
        delete state.orchestraRequestIds[requestKey];
        void loadOrchestraFleetBoard();
        nodes.statusLine.textContent = `Orchestra plan ${planId} started as ${result.run.runId}.`;
    }
    catch (error) {
        console.error(error);
        nodes.statusLine.textContent = `Orchestra plan ${planId} failed: ${error.message}`;
        if (button) {
            button.disabled = false;
            button.textContent = approvalMode === "operator_confirmation" ? "Review & run" : "Run plan";
        }
        void loadOrchestraPlan(runtimeId);
    }
}
async function mutateOrchestraRun(runId, action, button) {
    const runtimeId = state.selectedRuntimeId;
    if (!runtimeId || !runId) {
        return;
    }
    if (action === "cancel" && !window.confirm(`Cancel Orchestra run ${runId}?\n\nAlready completed steps cannot be rolled back.`)) {
        return;
    }
    button.disabled = true;
    const originalLabel = button.textContent;
    button.textContent = action === "cancel" ? "Cancelling..." : "Retrying...";
    try {
        const previousRun = (await getJson(`/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs`)).runs
            .find((run) => run.runId === runId);
        const currentPlan = state.orchestraPlan?.plans?.find((plan) => plan.planId === previousRun?.planId);
        let approvedBy = "automatic";
        let approvalNote = null;
        if (action === "retry" && currentPlan?.approvalMode === "operator_confirmation") {
            approvedBy = window.prompt("Approved by (operator-provided attribution)", previousRun?.approvedBy || "leserpent-operator");
            if (approvedBy === null) {
                button.disabled = false;
                button.textContent = originalLabel;
                return;
            }
            approvalNote = window.prompt("Approval note for this retry", "");
            if (approvalNote === null) {
                button.disabled = false;
                button.textContent = originalLabel;
                return;
            }
        }
        const confirmed = action !== "retry"
            || currentPlan?.approvalMode !== "operator_confirmation"
            || window.confirm(`Approve retry for ${previousRun?.planId || runId}?\n\nRisk: ${currentPlan?.riskLevel || "unknown"}`);
        if (!confirmed) {
            button.disabled = false;
            button.textContent = originalLabel;
            return;
        }
        const path = `/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs/${encodeURIComponent(runId)}/${action}`;
        const requestKey = `${runtimeId}:${runId}:${action}`;
        const requestId = orchestraRequestId(requestKey);
        const result = action === "retry"
            ? await postJsonBody(path, { confirmed, approvedBy, approvalNote, requestId })
            : await postJson(path);
        if (runtimeId === state.selectedRuntimeId) {
            await loadOrchestraHistory(runtimeId);
            delete state.orchestraRequestIds[requestKey];
            void loadOrchestraFleetBoard();
            nodes.statusLine.textContent = action === "cancel"
                ? `Cancellation requested for ${runId}.`
                : `Retry started as ${result.run.runId}.`;
        }
    }
    catch (error) {
        console.error(error);
        nodes.statusLine.textContent = `Orchestra ${action} failed: ${error.message}`;
        button.disabled = false;
        button.textContent = originalLabel;
    }
}
async function createOrchestraSession(button) {
    const runtimeId = state.selectedRuntimeId;
    const form = button?.closest("[data-orchestra-session-form]");
    const pipelineKind = form?.querySelector("[data-orchestra-pipeline-kind]")?.value?.trim();
    const requestedBy = form?.querySelector("[data-orchestra-requested-by]")?.value?.trim();
    if (!runtimeId || !pipelineKind || !requestedBy) {
        nodes.statusLine.textContent = "Pipeline kind and requested by are required.";
        return;
    }
    button.disabled = true;
    button.textContent = "Creating...";
    nodes.statusLine.textContent = `Creating ${pipelineKind} session through Orchestra...`;
    try {
        const result = await postJsonBody(`/v1/orchestra/plans/${encodeURIComponent(runtimeId)}/session`, {
            pipelineKind,
            requestedBy,
        });
        if (runtimeId !== state.selectedRuntimeId) {
            return;
        }
        renderOrchestraPlan(result.currentPlan);
        await loadDashboard();
        void loadOrchestraFleetBoard();
        nodes.statusLine.textContent = `Session ${result.session.sessionId} created through Orchestra.`;
    }
    catch (error) {
        console.error(error);
        nodes.statusLine.textContent = `Orchestra session handoff failed: ${error.message}`;
        button.disabled = false;
        button.textContent = "Create session";
    }
}
async function loadOrchestraPlan(runtimeId = state.selectedRuntimeId) {
    const requestSeq = ++state.orchestraRequestSeq;
    if (!runtimeId) {
        renderOrchestraPlan(null);
        renderOrchestraHistory([]);
        return;
    }
    try {
        const payload = await getJson(`/v1/orchestra/plans/${encodeURIComponent(runtimeId)}`);
        if (requestSeq !== state.orchestraRequestSeq || runtimeId !== state.selectedRuntimeId) {
            return;
        }
        renderOrchestraPlan(payload);
        void loadOrchestraHistory(runtimeId);
    }
    catch (error) {
        if (requestSeq !== state.orchestraRequestSeq) {
            return;
        }
        console.error(error);
        nodes.orchestraSummary.textContent = "Failed to build orchestra plan.";
        nodes.orchestraPlans.innerHTML = `
      <div class="group-card">
        <div class="group-title">Orchestra plan unavailable</div>
        <div class="hint-line">${escapeHtml(error.message || "unknown error")}</div>
      </div>
    `;
    }
}
function syncFilterInputs() {
    nodes.environmentInput.value = state.filter.environment;
    nodes.clusterInput.value = state.filter.cluster;
    nodes.roleInput.value = state.filter.role;
    nodes.runtimeSearch.value = state.runtimeSearch;
    nodes.runtimeSort.value = state.runtimeSort;
    const parts = [state.filter.environment, state.filter.cluster, state.filter.role].filter(Boolean);
    nodes.fleetFilterChip.textContent = parts.length ? parts.join(" / ") : t("filters.allRuntimes");
    syncFilterActionState();
    if (state.activeTab === "runtimes" && state.activeRuntimeMainTab === "register") {
        renderRegisterPreview();
    }
}
function syncFilterActionState() {
    const draft = [nodes.environmentInput, nodes.clusterInput, nodes.roleInput]
        .map((input) => input.value.trim());
    const applied = [state.filter.environment, state.filter.cluster, state.filter.role];
    nodes.applyFiltersButton.disabled = draft.every((value, index) => value === applied[index]);
    nodes.clearFiltersButton.disabled = !draft.some(Boolean)
        && !applied.some(Boolean)
        && !state.runtimeSearch;
    syncMobileFilterDisclosure();
}
function applyFleetFilters() {
    state.filter.environment = nodes.environmentInput.value.trim();
    state.filter.cluster = nodes.clusterInput.value.trim();
    state.filter.role = nodes.roleInput.value.trim();
    syncFilterActionState();
    if (window.innerWidth <= 920) {
        setMobileFiltersOpen(false, true);
    }
    void loadDashboard();
}
function clearRegisterForm() {
    const hasOperatorInput = [
        nodes.registerName,
        nodes.registerEndpoint,
        nodes.registerSidecarEndpoint,
        nodes.registerSidecarAdminToken,
        nodes.registerToken,
    ].some((input) => input.value.trim());
    if (hasOperatorInput && !window.confirm(t("register.clearConfirm"))) {
        return;
    }
    state.registerNameTouched = false;
    state.registrationPlanAbortController?.abort();
    state.registrationPlan = null;
    state.registrationPlanError = "";
    nodes.registerName.value = "";
    nodes.registerEndpoint.value = "";
    nodes.registerSidecarEndpoint.value = "";
    clearRegistrationSecrets();
    nodes.registerSidecarDetails.open = false;
    for (const field of [
        nodes.registerName,
        nodes.registerEndpoint,
        nodes.registerSidecarEndpoint,
        nodes.registerSidecarAdminToken,
        nodes.registerToken,
    ]) {
        field.setAttribute("aria-invalid", "false");
    }
    syncRegisterFormTagsFromFilter();
    nodes.registerFetchCapabilities.checked = true;
    renderRegisterPreview();
    setRegisterResult(t("register.untouched"));
}
function syncRegisterFormTagsFromFilter() {
    nodes.registerRuntimeEnvironment.value = state.filter.environment;
    nodes.registerRuntimeCluster.value = state.filter.cluster;
    nodes.registerRuntimeRole.value = state.filter.role;
}
function currentSliceLabel() {
    const parts = [state.filter.environment, state.filter.cluster, state.filter.role].filter(Boolean);
    return parts.length ? parts.join(" / ") : t("register.allRuntimes");
}
function currentSliceRuntimes() {
    return state.cache.runtimes?.runtimes || [];
}
function currentFailedRuntimeCount() {
    return state.cache.cleanupPlan?.failed?.runtimeCount ?? 0;
}
function currentSliceCount() {
    return state.cache.cleanupPlan?.slice?.runtimeCount ?? 0;
}
function currentUnobservedRuntimeCount() {
    return state.cache.cleanupPlan?.unobserved?.runtimeCount ?? 0;
}
function currentSliceSessionCount() {
    return state.cache.cleanupPlan?.slice?.sessionCount ?? 0;
}
function currentSliceRiskLevel() {
    return state.cache.cleanupPlan?.riskLevel === "protected";
}
function cleanupMutationAvailable() {
    return state.cache.capabilities?.webConsole?.cleanupAvailable !== false;
}
function currentSliceRiskWarning() {
    return currentSliceRiskLevel() ? `\n\n${t("notifications.runtimeCleanupProtectedWarning")}` : "";
}
async function runUiActionOnce(key, button, busyLabel, action) {
    if (state.uiActions.has(key)) {
        return;
    }
    state.uiActions.add(key);
    const previousLabel = button?.textContent || "";
    if (button) {
        button.disabled = true;
        button.setAttribute("aria-busy", "true");
        button.dataset.busy = "true";
        if (busyLabel)
            button.textContent = busyLabel;
    }
    try {
        return await action();
    }
    finally {
        state.uiActions.delete(key);
        if (button) {
            button.removeAttribute("aria-busy");
            delete button.dataset.busy;
            button.textContent = previousLabel;
            button.disabled = false;
        }
        if (key === "runtime-cleanup")
            syncCleanupMenuState();
        if (key === "register-runtime")
            renderRegisterPreview();
    }
}
function setCleanupControlsBusy(busy) {
    for (const button of [nodes.runtimeDeleteFailed, nodes.runtimeDeleteUnobserved, nodes.runtimeClearSlice]) {
        if (!button)
            continue;
        button.disabled = busy;
        button.toggleAttribute("aria-busy", busy);
    }
}
function runtimeNamesPreview(runtimes) {
    const names = runtimes.map((runtime) => runtime.name);
    if (!names.length) {
        return t("notifications.runtimeCleanupPreviewNone");
    }
    const preview = names.slice(0, 5).join(", ");
    if (names.length <= 5) {
        return preview;
    }
    return `${preview}${t("notifications.runtimeCleanupPreviewMore", { count: names.length - 5 })}`;
}
function describeCleanupTargets(runtimes) {
    return `\n\n${t("notifications.runtimeCleanupPreviewLabel")}: ${runtimeNamesPreview(runtimes)}`;
}
function cleanupAction(kind) {
    return state.cache.cleanupPlan?.[kind] || null;
}
function syncCleanupMenuState() {
    const menu = nodes.runtimeCleanupMenu;
    if (!menu) {
        return;
    }
    menu.dataset.risk = currentSliceRiskLevel() ? "protected" : "normal";
    if (nodes.runtimeCleanupHint) {
        nodes.runtimeCleanupHint.textContent = currentSliceRiskLevel()
            ? t("runtimes.cleanupHintProtected")
            : t("runtimes.cleanupHint");
    }
    if (nodes.runtimeCleanupFailedCount) {
        nodes.runtimeCleanupFailedCount.textContent = t("notifications.runtimeCleanupFailedCount", {
            count: currentFailedRuntimeCount(),
        });
    }
    if (nodes.runtimeCleanupUnobservedCount) {
        nodes.runtimeCleanupUnobservedCount.textContent = t("notifications.runtimeCleanupUnobservedCount", {
            count: currentUnobservedRuntimeCount(),
        });
    }
    if (nodes.runtimeCleanupRuntimeCount) {
        nodes.runtimeCleanupRuntimeCount.textContent = t("notifications.runtimeCleanupRuntimeCount", {
            count: currentSliceCount(),
        });
    }
    if (nodes.runtimeCleanupSessionCount) {
        nodes.runtimeCleanupSessionCount.textContent = t("notifications.runtimeCleanupSessionCount", {
            count: currentSliceSessionCount(),
        });
    }
    const cleanupBusy = state.uiActions.has("runtime-cleanup");
    const cleanupUnavailable = !cleanupMutationAvailable();
    if (nodes.runtimeDeleteFailed) {
        nodes.runtimeDeleteFailed.disabled = cleanupUnavailable || cleanupBusy || currentFailedRuntimeCount() === 0;
        nodes.runtimeDeleteFailed.toggleAttribute("aria-busy", cleanupBusy);
    }
    if (nodes.runtimeDeleteUnobserved) {
        nodes.runtimeDeleteUnobserved.disabled = cleanupUnavailable || cleanupBusy || currentUnobservedRuntimeCount() === 0;
        nodes.runtimeDeleteUnobserved.toggleAttribute("aria-busy", cleanupBusy);
    }
    if (nodes.runtimeClearSlice) {
        nodes.runtimeClearSlice.disabled = cleanupUnavailable || cleanupBusy || currentSliceCount() === 0;
        nodes.runtimeClearSlice.toggleAttribute("aria-busy", cleanupBusy);
    }
}
function resetRuntimeSelectionAfterBulkDelete() {
    state.selectedRuntimeId = null;
    if (state.activeRuntimeMainTab === "detail" || state.activeRuntimeMainTab === "panel") {
        state.activeRuntimeMainTab = "select";
        state.activeRuntimeSideTab = "detail";
    }
}
function renderDashboardFromCache() {
    const { capabilities, fleetSummary, attentionSummary, attentionList, runtimes, sessions } = state.cache;
    if (!capabilities || !fleetSummary || !attentionSummary || !attentionList || !runtimes || !sessions) {
        return;
    }
    if (state.activeTab === "overview") {
        if (state.activeOverviewTab === "summary") {
            renderMetricCards(nodes.fleetSummaryCards, [
                [t("metrics.runtimes"), fleetSummary.summary.runtimeCount],
                [t("metrics.latestSnapshots"), fleetSummary.summary.runtimesWithLatestSnapshot],
                [t("metrics.summaryJson"), fleetSummary.summary.runtimesWithSummaryJson],
                [t("metrics.analysisJson"), fleetSummary.summary.runtimesWithAnalysisJson],
                [t("metrics.pairedSidecars"), fleetSummary.summary.runtimesWithPairedSidecar],
                [t("metrics.healthySidecars"), fleetSummary.summary.runtimesWithHealthySidecar],
                [t("metrics.sidecarContext"), fleetSummary.summary.runtimesWithExternalSidecarContext],
                [t("metrics.diagnosticOpinions"), fleetSummary.summary.runtimesWithExternalDiagnosticOpinion],
            ], "fleetSummaryCards");
            renderGroupCards(nodes.fleetSummaryGroups, {
                [t("groups.snapshotKinds")]: fleetSummary.summary.snapshotKindCounts,
                [t("groups.statusSources")]: fleetSummary.summary.statusSourceCounts,
                [t("groups.sidecarStatusSources")]: fleetSummary.summary.sidecarStatusSourceCounts,
                [t("groups.environments")]: fleetSummary.summary.environmentCounts,
                [t("groups.clusters")]: fleetSummary.summary.clusterCounts,
                [t("groups.roles")]: fleetSummary.summary.roleCounts,
            }, "fleetSummaryGroups");
        }
        else if (state.activeOverviewTab === "attention") {
            renderMetricCards(nodes.attentionSummaryCards, [
                [t("metrics.critical"), attentionSummary.summary.criticalCount],
                [t("metrics.warning"), attentionSummary.summary.warningCount],
            ], "attentionSummaryCards");
            renderAttentionReasons(attentionSummary.summary);
        }
        else {
            renderAttentionList(attentionList);
        }
        return;
    }
    if (state.activeTab === "persistence") {
        renderPersistence(capabilities);
        return;
    }
    if (state.activeTab === "sessions") {
        renderSessions(sessions);
        return;
    }
    if (state.activeTab === "runtimes") {
        renderRuntimes(runtimes, state.runtimeAttentionById);
    }
}
async function loadDashboard() {
    state.dashboardAbortController?.abort();
    const abortController = new AbortController();
    state.dashboardAbortController = abortController;
    const requestId = ++state.dashboardRequestSeq;
    syncLocation();
    syncFilterInputs();
    syncRegisterFormTagsFromFilter();
    const query = buildQuery();
    nodes.statusLine.textContent = t("notifications.loading");
    try {
        const [capabilities, fleetSummary, attentionSummary, attentionList, runtimes, sessions, cleanupPlan] = await Promise.all([
            getJson("/v1/capabilities", abortController.signal),
            getJson(`/v1/fleet/summary${query}`, abortController.signal),
            getJson(`/v1/fleet/attention-summary${query}`, abortController.signal),
            getJson(`/v1/fleet/runtimes-needing-attention${query}`, abortController.signal),
            getJson(`/v1/runtimes${query}`, abortController.signal),
            getJson("/v1/sessions", abortController.signal),
            getJson(`/v1/runtimes/cleanup-plan${query}`, abortController.signal),
        ]);
        if (requestId !== state.dashboardRequestSeq) {
            return;
        }
        state.cache = {
            capabilities,
            fleetSummary,
            attentionSummary,
            attentionList,
            runtimes,
            sessions,
            cleanupPlan,
        };
        state.runtimeAttentionById = new Map((attentionList.runtimes || []).map((item) => [item.runtimeId, item]));
        state.latestRuntimes = runtimes.runtimes || [];
        renderDashboardFromCache();
        if (state.activeTab === "runtimes") {
            syncCleanupMenuState();
        }
        if (state.activeTab === "runtimes" && state.selectedRuntimeId) {
            void loadRuntimeAttention(state.selectedRuntimeId);
        }
        if (state.activeTab === "orchestra") {
            ensureRuntimeSelectionFromCache();
            void loadOrchestraPlan(state.selectedRuntimeId);
            void loadOrchestraFleetBoard();
        }
        nodes.statusLine.textContent = t("notifications.loaded", { count: runtimes.runtimes.length });
    }
    catch (error) {
        if (error?.name === "AbortError") {
            return;
        }
        if (requestId !== state.dashboardRequestSeq) {
            return;
        }
        console.error(error);
        if (looksLikeTokenDenied(error.message)) {
            state.adminTokenTestState = "failed";
            state.adminTokenTestAt = new Date().toLocaleString();
            setStoredAdminTokenTest(state.adminTokenTestState, state.adminTokenTestAt);
            renderSecurityState();
            if (state.adminToken?.trim()) {
                nodes.securityDetails?.setAttribute("open", "open");
                nodes.statusLine.textContent = t("security.tokenTestFailed", { message: t("security.tokenRequired") });
            }
            else {
                nodes.securityDetails?.removeAttribute("open");
                nodes.statusLine.textContent = t("security.tokenMissing");
            }
            return;
        }
        nodes.statusLine.textContent = t("notifications.dashboardLoadFailed", { message: error.message });
    }
    finally {
        if (state.dashboardAbortController === abortController) {
            state.dashboardAbortController = null;
        }
    }
}
async function postAndReload(path, label, button) {
    await runUiActionOnce("fleet-refresh", button, `${label}...`, async () => {
        const controls = [nodes.refreshAllButton, nodes.refreshStatusButton, nodes.refreshCapabilitiesButton];
        for (const control of controls) {
            control.disabled = true;
            control.setAttribute("aria-busy", "true");
        }
        nodes.statusLine.textContent = `${label}...`;
        try {
            await postJson(`${path}${buildQuery()}`);
            await loadDashboard();
            nodes.statusLine.textContent = t("notifications.fleetRefreshComplete", { label });
        }
        catch (error) {
            console.error(error);
            nodes.statusLine.textContent = t("notifications.fleetRefreshFailed", { label, message: error.message });
        }
        finally {
            for (const control of controls) {
                control.disabled = false;
                control.removeAttribute("aria-busy");
            }
        }
    });
}
async function deleteRuntime(runtimeId, runtimeName) {
    const confirmed = window.confirm(t("notifications.runtimeDeleteConfirm", { name: runtimeName }));
    if (!confirmed) {
        return;
    }
    nodes.statusLine.textContent = `${t("runtimes.actions.delete")}...`;
    try {
        const result = await postJson(`/v1/runtimes/${runtimeId}/delete`);
        if (state.selectedRuntimeId === runtimeId) {
            state.selectedRuntimeId = null;
            if (state.activeRuntimeMainTab === "detail" || state.activeRuntimeMainTab === "panel") {
                state.activeRuntimeMainTab = "select";
                state.activeRuntimeSideTab = "detail";
            }
        }
        await loadDashboard();
        nodes.statusLine.textContent = t("notifications.runtimeDeleted", {
            name: result.name || runtimeName,
            sessions: result.removedSessionCount ?? 0,
        });
    }
    catch (error) {
        console.error(error);
        nodes.statusLine.textContent = t("notifications.runtimeDeleteFailed", { message: error.message });
    }
}
async function deleteFailedRuntimes() {
    const slice = currentSliceLabel();
    const plan = cleanupAction("failed");
    const targets = plan?.targets || [];
    const count = plan?.runtimeCount ?? 0;
    if (!cleanupMutationAvailable() || !count || state.uiActions.has("runtime-cleanup")) {
        syncCleanupMenuState();
        return;
    }
    const confirmed = window.confirm(`${t("notifications.runtimeDeleteFailedSliceConfirm", { slice, count })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}`);
    if (!confirmed) {
        return;
    }
    await runUiActionOnce("runtime-cleanup", nodes.runtimeDeleteFailed, t("runtimes.actions.deleteFailed"), async () => {
        setCleanupControlsBusy(true);
        nodes.statusLine.textContent = `${t("runtimes.actions.deleteFailed")}...`;
        try {
            const result = await postJsonBody(`/v1/runtimes/delete-failed${buildQuery()}`, {
                planToken: plan.planToken,
            });
            nodes.runtimeCleanupMenu?.removeAttribute("open");
            resetRuntimeSelectionAfterBulkDelete();
            await loadDashboard();
            nodes.statusLine.textContent = t("notifications.runtimeDeleteFailedSliceDone", {
                count: result.removedRuntimeCount ?? 0,
                sessions: result.removedSessionCount ?? 0,
                slice,
            });
        }
        catch (error) {
            console.error(error);
            nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
        }
        finally {
            syncCleanupMenuState();
        }
    });
}
async function deleteUnobservedRuntimes() {
    const slice = currentSliceLabel();
    const plan = cleanupAction("unobserved");
    const targets = plan?.targets || [];
    const count = plan?.runtimeCount ?? 0;
    if (!cleanupMutationAvailable() || !count || state.uiActions.has("runtime-cleanup")) {
        syncCleanupMenuState();
        return;
    }
    const confirmed = window.confirm(`${t("notifications.runtimeDeleteUnobservedSliceConfirm", { slice, count })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}`);
    if (!confirmed) {
        return;
    }
    await runUiActionOnce("runtime-cleanup", nodes.runtimeDeleteUnobserved, t("runtimes.actions.deleteUnobserved"), async () => {
        setCleanupControlsBusy(true);
        nodes.statusLine.textContent = `${t("runtimes.actions.deleteUnobserved")}...`;
        try {
            const result = await postJsonBody(`/v1/runtimes/delete-unobserved${buildQuery()}`, {
                planToken: plan.planToken,
            });
            nodes.runtimeCleanupMenu?.removeAttribute("open");
            resetRuntimeSelectionAfterBulkDelete();
            await loadDashboard();
            nodes.statusLine.textContent = t("notifications.runtimeDeleteUnobservedSliceDone", {
                count: result.removedRuntimeCount ?? 0,
                sessions: result.removedSessionCount ?? 0,
                slice,
            });
        }
        catch (error) {
            console.error(error);
            nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
        }
        finally {
            syncCleanupMenuState();
        }
    });
}
async function clearRuntimeSlice() {
    const slice = currentSliceLabel();
    const plan = cleanupAction("slice");
    const targets = plan?.targets || [];
    if (!cleanupMutationAvailable() || !plan?.runtimeCount || state.uiActions.has("runtime-cleanup")) {
        syncCleanupMenuState();
        return;
    }
    const challenge = plan.challenge;
    const entered = window.prompt(`${t("notifications.runtimeClearSliceConfirm", { slice, count: currentSliceCount() })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}\n\n${t("notifications.runtimeClearSliceChallenge", { challenge })}`, "");
    if (entered === null) {
        return;
    }
    if (entered.trim() !== challenge) {
        nodes.statusLine.textContent = t("notifications.runtimeClearSliceChallengeFailed");
        return;
    }
    await runUiActionOnce("runtime-cleanup", nodes.runtimeClearSlice, t("runtimes.actions.clearSlice"), async () => {
        setCleanupControlsBusy(true);
        nodes.statusLine.textContent = `${t("runtimes.actions.clearSlice")}...`;
        try {
            const result = await postJsonBody(`/v1/runtimes/delete-slice${buildQuery()}`, {
                planToken: plan.planToken,
                challenge: entered.trim(),
            });
            nodes.runtimeCleanupMenu?.removeAttribute("open");
            resetRuntimeSelectionAfterBulkDelete();
            await loadDashboard();
            nodes.statusLine.textContent = t("notifications.runtimeClearSliceDone", {
                count: result.removedRuntimeCount ?? 0,
                sessions: result.removedSessionCount ?? 0,
                slice,
            });
        }
        catch (error) {
            console.error(error);
            nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
        }
        finally {
            syncCleanupMenuState();
        }
    });
}
async function savePersistenceNow() {
    await runUiActionOnce("persistence-save", nodes.persistenceSaveNow, `${t("persistence.saveNow")}...`, async () => {
        nodes.statusLine.textContent = t("persistence.saving");
        try {
            await postJson("/v1/persistence/save");
            await loadDashboard();
            nodes.statusLine.textContent = t("persistence.saved");
        }
        catch (error) {
            console.error(error);
            nodes.statusLine.textContent = t("persistence.saveFailed", { message: error.message });
        }
    });
}
async function exportPersistenceState() {
    await runUiActionOnce("persistence-export", nodes.persistenceExportState, `${t("persistence.exportState")}...`, async () => {
        nodes.statusLine.textContent = t("persistence.exporting");
        try {
            const response = await fetch("/v1/persistence/export", {
                headers: apiHeaders({ intent: "export" }),
            });
            if (!response.ok) {
                throw new Error(`/v1/persistence/export -> ${response.status}`);
            }
            const blob = await response.blob();
            const downloadUrl = URL.createObjectURL(blob);
            const anchor = document.createElement("a");
            const disposition = response.headers.get("content-disposition") || "";
            const match = disposition.match(/filename=\"?([^\";]+)\"?/i);
            anchor.href = downloadUrl;
            anchor.download = match?.[1] || "leserpent-control-plane-state.json";
            document.body.appendChild(anchor);
            anchor.click();
            anchor.remove();
            URL.revokeObjectURL(downloadUrl);
            nodes.statusLine.textContent = t("persistence.exported");
        }
        catch (error) {
            console.error(error);
            nodes.statusLine.textContent = t("persistence.exportFailed", { message: error.message });
        }
    });
}
function triggerPersistenceImportPicker() {
    nodes.persistenceImportFile.value = "";
    nodes.persistenceImportFile.click();
}
async function importPersistenceState(file) {
    if (!file) {
        return;
    }
    if (state.uiActions.has("persistence-import"))
        return;
    try {
        if (file.size > 1_048_576)
            throw new Error(t("persistence.importTooLarge"));
        const text = await file.text();
        let parsed;
        try {
            parsed = JSON.parse(text);
        }
        catch {
            throw new Error(t("persistence.invalidJson"));
        }
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)
            || !Array.isArray(parsed.runtimes) || !Array.isArray(parsed.sessions)) {
            throw new Error(t("persistence.invalidStructure"));
        }
        const advertisedSchema = state.cache.capabilities?.persistence?.schemaVersion;
        const maximumSchema = Number.isInteger(advertisedSchema) && advertisedSchema > 0
            ? advertisedSchema
            : 1;
        if (!Number.isInteger(parsed.schemaVersion)
            || parsed.schemaVersion < 1
            || parsed.schemaVersion > maximumSchema) {
            throw new Error(t("persistence.incompatibleSchema", { schema: parsed.schemaVersion ?? "?" }));
        }
        const confirmed = window.confirm(t("persistence.importConfirm", {
            file: file.name,
            runtimes: parsed.runtimes.length,
            sessions: parsed.sessions.length,
            currentRuntimes: state.latestRuntimes.length,
            currentSessions: state.cache.sessions?.sessions?.length || 0,
        }));
        if (!confirmed) {
            nodes.statusLine.textContent = t("persistence.importCancelled");
            return;
        }
        await runUiActionOnce("persistence-import", nodes.persistenceImportState, t("persistence.importingShort"), async () => {
            nodes.statusLine.textContent = t("persistence.importing", { file: file.name });
            const response = await fetch("/v1/persistence/import", {
                method: "POST",
                headers: apiHeaders({ contentType: "application/json", intent: "mutate" }),
                body: JSON.stringify(parsed),
            });
            const payload = await response.json().catch(() => null);
            if (!response.ok) {
                throw new Error(payload?.reason || payload?.error || `${response.status}`);
            }
            state.selectedRuntimeId = null;
            await loadDashboard();
            nodes.statusLine.textContent = t("persistence.imported", {
                runtimes: payload.importedRuntimeCount,
                sessions: payload.importedSessionCount,
            });
        });
    }
    catch (error) {
        console.error(error);
        nodes.statusLine.textContent = t("persistence.importFailed", { message: error.message });
    }
}
async function submitRegisterForm(event) {
    event.preventDefault();
    const name = nodes.registerName.value.trim();
    const endpoint = nodes.registerEndpoint.value.trim();
    const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
    if (state.uiActions.has("register-runtime"))
        return;
    const readiness = registrationReadiness(isLikelyHttpEndpoint(endpoint), sidecarEndpoint ? isLikelyHttpEndpoint(sidecarEndpoint) : true);
    if (!readiness.ready) {
        showRegistrationIssue(readiness);
        return;
    }
    const registrationPlan = readiness.plan;
    const body = {
        name,
        endpoint,
        sidecarEndpoint: sidecarEndpoint || null,
        sidecarAdminToken: nodes.registerSidecarAdminToken.value.trim() || null,
        pairingToken: nodes.registerToken.value.trim(),
        capabilities: [],
        tags: {
            environment: nodes.registerRuntimeEnvironment.value.trim() || null,
            cluster: nodes.registerRuntimeCluster.value.trim() || null,
            role: nodes.registerRuntimeRole.value.trim() || null,
        },
        fetchCapabilities: nodes.registerFetchCapabilities.checked,
        registrationPlanToken: registrationPlan.planToken,
    };
    await runUiActionOnce("register-runtime", nodes.registerSubmit, t("register.registeringShort"), async () => {
        setRegisterResult(t("register.registering"), "pending");
        try {
            const result = await postJsonBody("/v1/runtimes/register", body);
            state.registrationPlan = null;
            state.registerNameTouched = false;
            clearRegistrationSecrets();
            renderRegisterPreview();
            state.activeTab = "runtimes";
            state.activeRuntimeMainTab = "detail";
            state.selectedRuntimeId = result.runtimeId;
            setRegisterResult(t("register.registered", {
                name: result.name,
                runtimeId: result.runtimeId,
                slice: currentSliceLabel(),
                status: runtimeStatusHint(result.status),
            }), "good");
            applyTabShell();
            await loadDashboard();
            nodes.runtimeDetailPanel.scrollIntoView({ behavior: "smooth", block: "start" });
        }
        catch (error) {
            console.error(error);
            setRegisterResult(t("register.failed", { message: error.message }), "bad", true);
            state.activeTab = "runtimes";
            state.activeRuntimeMainTab = "register";
            applyTabShell();
        }
    });
}
const state = {
    filter: {
        environment: "",
        cluster: "",
        role: "",
    },
    languagePreference: "auto",
    language: "en",
    installedLanguagePacks: {},
    languagePackCatalog: [],
    languagePackCatalogMeta: { official: 8, builtin: 8 },
    themePreference: "auto",
    theme: "light",
    layoutMode: "default",
    runtimeListLayout: "cards",
    runtimeListLayoutObserver: null,
    mobileFiltersOpen: false,
    activeTab: "overview",
    activeOverviewTab: "summary",
    activeRuntimeMainTab: "select",
    activeRuntimeSideTab: "detail",
    activeRuntimeDetailTab: "identity",
    runtimePanelView: "root",
    runtimeWindowIds: [],
    activeRuntimeWindowId: null,
    runtimeWindowViews: Object.create(null),
    runtimeWindowIntentPending: false,
    runtimeSearch: "",
    runtimeSort: "name",
    selectedRuntimeId: null,
    orchestraPlan: null,
    orchestraRequestSeq: 0,
    orchestraPollTimer: 0,
    orchestraFleetPollTimer: 0,
    orchestraRequestIds: {},
    dashboardRequestSeq: 0,
    dashboardAbortController: null,
    pendingLocationSync: 0,
    pendingLayoutFrame: 0,
    lastSyncedLocation: "",
    pendingRegisterPreview: 0,
    pendingRuntimeRender: 0,
    uiActions: new Set(),
    runtimeAttentionById: new Map(),
    recentBadgeRefresh: {
        runtime: null,
        sidecar: null,
    },
    latestRuntimes: [],
    renderSignatures: {
        runtimeDetail: "",
        runtimePanel: "",
        registerPreview: "",
        runtimeTable: "",
        fleetSummaryCards: "",
        fleetSummaryGroups: "",
        persistenceCards: "",
        persistenceDetails: "",
        attentionSummaryCards: "",
        attentionReasons: "",
        attentionList: "",
        sessions: "",
        orchestraPanel: "",
        orchestraHistory: "",
        orchestraFleetBoard: "",
    },
    registerNameTouched: false,
    adminToken: "",
    adminTokenVisible: false,
    adminTokenTestState: "never",
    adminTokenTestAt: null,
    cache: {
        capabilities: null,
        fleetSummary: null,
        attentionSummary: null,
        attentionList: null,
        runtimes: null,
        sessions: null,
    },
};
const storageKeys = {
    languagePreference: "leserpent.languagePreference",
    themePreference: "leserpent.themePreference",
    adminToken: "leserpent.adminToken",
    adminTokenTestState: "leserpent.adminTokenTestState",
    adminTokenTestAt: "leserpent.adminTokenTestAt",
    runtimeWindows: "leserpent.runtimeWindows",
    languagePacks: "leserpent.languagePacks",
};
const nodes = {
    fleetSummaryCards: document.getElementById("fleet-summary-cards"),
    fleetSummaryGroups: document.getElementById("fleet-summary-groups"),
    persistenceCards: document.getElementById("persistence-cards"),
    persistenceDetails: document.getElementById("persistence-details"),
    persistenceSaveNow: document.getElementById("persistence-save-now"),
    persistenceExportState: document.getElementById("persistence-export-state"),
    persistenceImportState: document.getElementById("persistence-import-state"),
    persistenceImportFile: document.getElementById("persistence-import-file"),
    attentionSummaryCards: document.getElementById("attention-summary-cards"),
    attentionReasons: document.getElementById("attention-reasons"),
    attentionList: document.getElementById("attention-list"),
    sessionList: document.getElementById("session-list"),
    runtimeTableBody: document.getElementById("runtime-table-body"),
    fleetFilterChip: document.getElementById("fleet-filter-chip"),
    attentionCount: document.getElementById("attention-count"),
    sessionCount: document.getElementById("session-count"),
    runtimeCount: document.getElementById("runtime-count"),
    orchestraSummary: document.getElementById("orchestra-summary"),
    orchestraRefresh: document.getElementById("orchestra-refresh"),
    orchestraPlans: document.getElementById("orchestra-plans"),
    orchestraHistorySummary: document.getElementById("orchestra-history-summary"),
    orchestraHistory: document.getElementById("orchestra-history"),
    orchestraFleetCount: document.getElementById("orchestra-fleet-count"),
    orchestraFleetMetrics: document.getElementById("orchestra-fleet-metrics"),
    orchestraFleetRuns: document.getElementById("orchestra-fleet-runs"),
    runtimeWorkspace: document.getElementById("runtime-workspace"),
    runtimeMainTabButtons: Array.from(document.querySelectorAll(".runtime-main-tab-button")),
    runtimeMainPanels: Array.from(document.querySelectorAll(".runtime-main-panel")),
    runtimeListCard: document.querySelector(".runtime-list-card"),
    runtimeSearch: document.getElementById("runtime-search"),
    runtimeSort: document.getElementById("runtime-sort"),
    runtimeCleanupMenu: document.getElementById("runtime-cleanup-menu"),
    runtimeCleanupSummary: document.getElementById("runtime-cleanup-summary"),
    runtimeCleanupHint: document.getElementById("runtime-cleanup-hint"),
    runtimeCleanupFailedCount: document.getElementById("runtime-cleanup-failed-count"),
    runtimeCleanupUnobservedCount: document.getElementById("runtime-cleanup-unobserved-count"),
    runtimeCleanupRuntimeCount: document.getElementById("runtime-cleanup-runtime-count"),
    runtimeCleanupSessionCount: document.getElementById("runtime-cleanup-session-count"),
    runtimeDeleteFailed: document.getElementById("runtime-delete-failed"),
    runtimeDeleteUnobserved: document.getElementById("runtime-delete-unobserved"),
    runtimeClearSlice: document.getElementById("runtime-clear-slice"),
    runtimeDetailChip: document.getElementById("runtime-detail-chip"),
    runtimeDetailActions: document.getElementById("runtime-detail-actions"),
    runtimeDetailEmpty: document.getElementById("runtime-detail-empty"),
    runtimeDetailPanel: document.getElementById("runtime-detail-panel"),
    runtimeDetailSummary: document.getElementById("runtime-detail-summary"),
    runtimeDetailIdentity: document.getElementById("runtime-detail-identity"),
    runtimeDetailStatus: document.getElementById("runtime-detail-status"),
    runtimeDetailCapabilities: document.getElementById("runtime-detail-capabilities"),
    runtimeDetailAttention: document.getElementById("runtime-detail-attention"),
    runtimeDetailRefreshAll: document.getElementById("runtime-detail-refresh-all"),
    runtimeDetailRefreshStatus: document.getElementById("runtime-detail-refresh-status"),
    runtimeDetailRefreshCapabilities: document.getElementById("runtime-detail-refresh-capabilities"),
    runtimeDetailRefreshSidecar: document.getElementById("runtime-detail-refresh-sidecar"),
    runtimeDetailCopyLink: document.getElementById("runtime-detail-copy-link"),
    runtimeDetailSubtabButtons: Array.from(document.querySelectorAll(".runtime-detail-subtab-button")),
    runtimeDetailSections: Array.from(document.querySelectorAll(".runtime-detail-section")),
    runtimePanelChip: document.getElementById("runtime-panel-chip"),
    runtimePanelBreadcrumb: document.getElementById("runtime-panel-breadcrumb"),
    runtimePanelTrust: document.getElementById("runtime-panel-trust"),
    runtimePanelSourceSwitch: document.getElementById("runtime-panel-source-switch"),
    runtimePanelSourceButtons: Array.from(document.querySelectorAll(".runtime-panel-source-button")),
    runtimePanelSourceBadges: document.getElementById("runtime-panel-source-badges"),
    runtimePanelActions: document.getElementById("runtime-panel-actions"),
    runtimePanelOverflow: document.getElementById("runtime-panel-overflow"),
    runtimePanelEmpty: document.getElementById("runtime-panel-empty"),
    runtimePanelFrameWrap: document.getElementById("runtime-panel-frame-wrap"),
    runtimePanelBlank: document.getElementById("runtime-panel-blank"),
    runtimePanelFrame: document.getElementById("runtime-panel-frame"),
    runtimePanelUrl: document.getElementById("runtime-panel-url"),
    runtimePanelTabs: Array.from(document.querySelectorAll(".runtime-panel-tab")),
    runtimePanelOpenExternal: document.getElementById("runtime-panel-open-external"),
    runtimeWindowToolbar: document.getElementById("runtime-window-toolbar"),
    runtimeWindowOpenSelected: document.getElementById("runtime-window-open-selected"),
    runtimeWindowOpenAll: document.getElementById("runtime-window-open-all"),
    runtimeWindowCloseAll: document.getElementById("runtime-window-close-all"),
    runtimeWindowCount: document.getElementById("runtime-window-count"),
    runtimeWindowPolicy: document.getElementById("runtime-window-policy"),
    runtimeWindowGrid: document.getElementById("runtime-window-grid"),
    statusLine: document.getElementById("status-line"),
    mobileFilterToggle: document.getElementById("mobile-filter-toggle"),
    mobileFilterCount: document.getElementById("mobile-filter-count"),
    environmentInput: document.getElementById("filter-environment"),
    clusterInput: document.getElementById("filter-cluster"),
    roleInput: document.getElementById("filter-role"),
    applyFiltersButton: document.getElementById("apply-filters"),
    clearFiltersButton: document.getElementById("clear-filters"),
    refreshAllButton: document.getElementById("refresh-all"),
    refreshStatusButton: document.getElementById("refresh-status"),
    refreshCapabilitiesButton: document.getElementById("refresh-capabilities"),
    registerForm: document.getElementById("register-form"),
    registerName: document.getElementById("register-name"),
    registerEndpoint: document.getElementById("register-endpoint"),
    registerSidecarDetails: document.getElementById("register-sidecar-details"),
    registerSidecarEndpoint: document.getElementById("register-sidecar-endpoint"),
    registerSidecarAdminToken: document.getElementById("register-sidecar-admin-token"),
    registerSidecarAdminTokenToggle: document.getElementById("register-sidecar-admin-token-toggle"),
    registerSidecarAdminTokenToggleLabel: document.getElementById("register-sidecar-admin-token-toggle-label"),
    registerToken: document.getElementById("register-token"),
    registerTokenToggle: document.getElementById("register-token-toggle"),
    registerTokenToggleLabel: document.getElementById("register-token-toggle-label"),
    registerRuntimeEnvironment: document.getElementById("register-runtime-environment"),
    registerRuntimeCluster: document.getElementById("register-runtime-cluster"),
    registerRuntimeRole: document.getElementById("register-runtime-role"),
    registerFetchCapabilities: document.getElementById("register-fetch-capabilities"),
    registerSubmit: document.getElementById("register-submit"),
    registerFormClear: document.getElementById("register-form-clear"),
    registerGuidance: document.getElementById("register-guidance"),
    registerPreview: document.getElementById("register-preview"),
    registerResult: document.getElementById("register-result"),
    languageSelect: document.getElementById("language-select"),
    languagePackDetails: document.getElementById("language-pack-details"),
    languagePackRefresh: document.getElementById("language-pack-refresh"),
    languagePackImport: document.getElementById("language-pack-import"),
    languagePackFile: document.getElementById("language-pack-file"),
    languagePackInstalled: document.getElementById("language-pack-installed"),
    languagePackCatalog: document.getElementById("language-pack-catalog"),
    languagePackStatus: document.getElementById("language-pack-status"),
    themeSelect: document.getElementById("theme-select"),
    securityDetails: document.getElementById("security-details"),
    securityPanelBadge: document.getElementById("security-panel-badge"),
    adminTokenInput: document.getElementById("admin-token-input"),
    adminTokenToggleVisibility: document.getElementById("admin-token-toggle-visibility"),
    adminTokenTest: document.getElementById("admin-token-test"),
    adminTokenClear: document.getElementById("admin-token-clear"),
    adminTokenState: document.getElementById("admin-token-state"),
    adminTokenLastTest: document.getElementById("admin-token-last-test"),
    tabButtons: Array.from(document.querySelectorAll(".tab-button")),
    tabPanels: Array.from(document.querySelectorAll(".tab-panel")),
    overviewSubtabButtons: Array.from(document.querySelectorAll(".overview-subtab-button")),
    overviewSubpanels: Array.from(document.querySelectorAll(".overview-subpanel")),
};
bootstrapDashboard();
