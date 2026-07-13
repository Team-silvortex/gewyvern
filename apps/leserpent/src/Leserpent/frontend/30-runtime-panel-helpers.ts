// @ts-nocheck
// Runtime panel navigation and trust helpers split from security/transport helpers.

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
    if (trust.source === "fetch_failed") return t("runtimePanel.compactTrustSidecarFetchFailed");
    if (trust.source === "unobserved") return t("runtimePanel.compactTrustSidecarUnobserved");
    if (trust.label === t("runtimePanel.trustNoSidecar")) return t("runtimePanel.compactTrustNoSidecar");
    return t("runtimePanel.compactTrustSidecarObserved");
  }

  if (trust.source === "idle_ready") return t("runtimePanel.compactTrustIdleReady");
  if (trust.source === "fetch_failed") return t("runtimePanel.compactTrustFetchFailed");
  if (trust.source === "unobserved") return t("runtimePanel.compactTrustUnobserved");
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
