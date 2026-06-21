// @ts-nocheck
// Transitional TypeScript source: keep runtime behavior stable first, then tighten types incrementally.
const state = {
  filter: {
    environment: "",
    cluster: "",
    role: "",
  },
  languagePreference: "auto",
  language: "en",
  themePreference: "auto",
  theme: "light",
  layoutMode: "default",
  activeTab: "overview",
  activeOverviewTab: "summary",
  activeRuntimeMainTab: "select",
  activeRuntimeSideTab: "detail",
  activeRuntimeDetailTab: "identity",
  runtimePanelView: "root",
  runtimeSearch: "",
  runtimeSort: "name",
  selectedRuntimeId: null,
  dashboardRequestSeq: 0,
  pendingLocationSync: 0,
  lastSyncedLocation: "",
  pendingRegisterPreview: 0,
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
  runtimeWorkspace: document.getElementById("runtime-workspace"),
  runtimeMainTabButtons: Array.from(document.querySelectorAll(".runtime-main-tab-button")),
  runtimeMainPanels: Array.from(document.querySelectorAll(".runtime-main-panel")),
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
  statusLine: document.getElementById("status-line"),
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
  registerSidecarEndpoint: document.getElementById("register-sidecar-endpoint"),
  registerSidecarAdminToken: document.getElementById("register-sidecar-admin-token"),
  registerToken: document.getElementById("register-token"),
  registerRuntimeEnvironment: document.getElementById("register-runtime-environment"),
  registerRuntimeCluster: document.getElementById("register-runtime-cluster"),
  registerRuntimeRole: document.getElementById("register-runtime-role"),
  registerFetchCapabilities: document.getElementById("register-fetch-capabilities"),
  registerFormClear: document.getElementById("register-form-clear"),
  registerPreview: document.getElementById("register-preview"),
  registerResult: document.getElementById("register-result"),
  languageSelect: document.getElementById("language-select"),
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
    <div class="reason-line"><strong>${escapeHtml(t(`attention.${reason}`))}</strong> · ${escapeHtml(count)} ${escapeHtml(t("metrics.runtimes"))}</div>
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
        ${(item.reasons || []).map((reason) => `<span class="reason-pill">${escapeHtml(t(`attention.${reason}`) || reason)}</span>`).join("")}
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

function findDuplicateRuntime(name, endpoint) {
  const normalizedName = name.trim().toLowerCase();
  const normalizedEndpoint = endpoint.trim().toLowerCase();
  return state.latestRuntimes.find((runtime) =>
    runtime.name.toLowerCase() === normalizedName ||
    runtime.endpoint.toLowerCase() === normalizedEndpoint
  ) || null;
}

function isLikelyHttpEndpoint(endpoint) {
  if (!(endpoint.startsWith("http://") || endpoint.startsWith("https://"))) {
    return false;
  }

  try {
    const parsed = new URL(endpoint);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
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
  } catch {
    return "";
  }
}

function maybePrefillRuntimeNameFromEndpoint() {
  if (state.registerNameTouched) {
    scheduleRenderRegisterPreview();
    return;
  }

  const endpoint = nodes.registerEndpoint.value.trim();
  if (!isLikelyHttpEndpoint(endpoint)) {
    scheduleRenderRegisterPreview();
    return;
  }

  const suggestion = suggestedRuntimeName(endpoint);
  if (suggestion) {
    nodes.registerName.value = suggestion;
  }
  scheduleRenderRegisterPreview();
}

function registerPreviewSignature() {
  return [
    state.language,
    nodes.registerName.value.trim(),
    nodes.registerEndpoint.value.trim(),
    nodes.registerSidecarEndpoint.value.trim(),
    nodes.registerSidecarAdminToken.value.trim() ? "protected" : "open",
    nodes.registerRuntimeEnvironment.value.trim(),
    nodes.registerRuntimeCluster.value.trim(),
    nodes.registerRuntimeRole.value.trim(),
    nodes.registerFetchCapabilities.checked ? "fetch" : "skip",
  ].join("::");
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

function renderRegisterPreview() {
  const signature = registerPreviewSignature();
  if (state.renderSignatures.registerPreview === signature) {
    return;
  }
  state.renderSignatures.registerPreview = signature;

  const endpoint = nodes.registerEndpoint.value.trim();
  const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
  const sidecarAdminToken = nodes.registerSidecarAdminToken.value.trim();
  const explicitName = nodes.registerName.value.trim();
  const endpointValid = endpoint.length > 0 && isLikelyHttpEndpoint(endpoint);
  const sidecarEndpointValid = sidecarEndpoint.length > 0 ? isLikelyHttpEndpoint(sidecarEndpoint) : true;
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
        <strong>${escapeHtml(endpointState)}</strong>
        ${endpoint ? `<div class="register-preview-meta">${escapeHtml(endpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecar"))}</span>
        <strong>${escapeHtml(sidecarState)}</strong>
        ${sidecarEndpoint ? `<div class="register-preview-meta">${escapeHtml(sidecarEndpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecarAccess"))}</span>
        <strong>${escapeHtml(sidecarAccess)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewCapabilityFetch"))}</span>
        <strong>${escapeHtml(nodes.registerFetchCapabilities.checked ? t("register.capabilityEnabled") : t("register.capabilityDisabled"))}</strong>
      </div>
    </div>
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
  const previous = nodes.runtimeTableBody.querySelector("tr.selected");
  if (previous instanceof HTMLTableRowElement && previous.dataset.runtimeId !== selectedRuntimeId) {
    previous.classList.remove("selected");
  }

  if (!selectedRuntimeId) {
    return;
  }

  const next = nodes.runtimeTableBody.querySelector(`tr[data-runtime-id="${CSS.escape(selectedRuntimeId)}"]`);
  if (next instanceof HTMLTableRowElement) {
    next.classList.add("selected");
  }
}

function renderRuntimes(payload, attentionMap) {
  const allItems = payload.runtimes || [];
  state.latestRuntimes = allItems;
  const query = state.runtimeSearch.trim().toLowerCase();
  const filteredItems = query
    ? allItems.filter((runtime) =>
      runtime.name.toLowerCase().includes(query) ||
      runtime.endpoint.toLowerCase().includes(query))
    : allItems;
  const items = [...filteredItems].sort((left, right) => {
    if (state.runtimeSort === "status") {
      return (left.status.statusSource || "").localeCompare(right.status.statusSource || "") ||
        left.name.localeCompare(right.name);
    }
    if (state.runtimeSort === "snapshot") {
      return (left.status.snapshotKind || "").localeCompare(right.status.snapshotKind || "") ||
        left.name.localeCompare(right.name);
    }
    return left.name.localeCompare(right.name);
  });
  nodes.runtimeCount.textContent = `${items.length} ${t("metrics.runtimes")}`;
  if (!items.length) {
    state.selectedRuntimeId = null;
    const emptySignature = `empty::${state.language}::${state.runtimeSearch.trim().toLowerCase()}::${state.runtimeSort}`;
    if (state.renderSignatures.runtimeTable !== emptySignature) {
      state.renderSignatures.runtimeTable = emptySignature;
      nodes.runtimeTableBody.innerHTML = `<tr><td colspan="7">${escapeHtml(t("runtimes.noMatch"))}</td></tr>`;
    }
    renderRuntimeDetail(null, null);
    renderRuntimePanel(null);
    return;
  }

  if (!items.some((item) => item.runtimeId === state.selectedRuntimeId)) {
    state.selectedRuntimeId = items[0].runtimeId;
  }

  const tableSignature = runtimeTableSignature(items, attentionMap);
  if (state.renderSignatures.runtimeTable !== tableSignature) {
    state.renderSignatures.runtimeTable = tableSignature;
    nodes.runtimeTableBody.innerHTML = items.map((runtime) => {
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
        <tr class="${runtime.runtimeId === state.selectedRuntimeId ? "selected" : ""}" data-runtime-id="${escapeHtml(runtime.runtimeId)}">
          <td>
            <strong>${escapeHtml(runtime.name)}</strong>
            <div class="item-meta">${escapeHtml(runtime.endpoint)}</div>
          </td>
          <td>
            <div class="runtime-tags">
              <span class="tag-pill">${escapeHtml(runtime.tags.environment || t("runtimes.states.noEnv"))}</span>
              <span class="tag-pill">${escapeHtml(runtime.tags.cluster || t("runtimes.states.noCluster"))}</span>
              <span class="tag-pill">${escapeHtml(runtime.tags.role || t("runtimes.states.noRole"))}</span>
            </div>
          </td>
          <td>
            <span class="runtime-state ${escapeHtml(badge.tone)}">${escapeHtml(badge.text)}</span>
            <div class="item-meta">${escapeHtml(t("runtimeDetail.source"))}: ${escapeHtml(runtime.status.statusSource)}</div>
            ${runtime.status.resilienceStatus ? `<div class="item-meta">${escapeHtml(t("runtimeDetail.resilienceStatus"))}: ${escapeHtml(runtime.status.resilienceStatus)}</div>` : ""}
          </td>
          <td>
            <div class="runtime-surface">
              <div class="runtime-surface-compact item-meta">${escapeHtml(compactCapabilitySummary)}</div>
              <div class="runtime-surface-pills">
                ${capabilityKeys.length ? capabilityKeys.map((key) => `<span class="tag-pill">${escapeHtml(key)}</span>`).join("") : `<span class="item-meta">${escapeHtml(t("runtimes.states.noCapabilities"))}</span>`}
              </div>
            </div>
          </td>
          <td>
            <div class="runtime-sidecar">
              ${sidecarBits.length ? sidecarBits.map((bit) => `<span class="tag-pill">${escapeHtml(bit)}</span>`).join("") : `<span class="item-meta">${escapeHtml(t("runtimes.states.none"))}</span>`}
            </div>
          </td>
          <td>
            <div class="runtime-attention">
              ${attention
                ? `<span class="runtime-state ${attention.severity === "critical" ? "bad" : "warn"}">${escapeHtml(t(`attention.${attention.severity}`))}</span>
                   ${(attention.reasons || []).map((reason) => `<span class="tag-pill">${escapeHtml(t(`attention.${reason}`) || reason)}</span>`).join("")}`
                : `<span class="runtime-state good">${escapeHtml(t("runtimes.states.clear"))}</span>`}
            </div>
          </td>
          <td>
            <div class="inline-actions">
              <button type="button" data-action="show-attention" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimes.actions.attention"))}</button>
              <button type="button" data-action="refresh-status" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimes.actions.status"))}</button>
              ${runtime.sidecarEndpoint ? `<button type="button" data-action="refresh-sidecar" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimeDetail.refreshSidecar"))}</button>` : ""}
              <button type="button" data-action="refresh-all" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimes.actions.all"))}</button>
              <button type="button" data-action="delete-runtime" data-runtime-id="${escapeHtml(runtime.runtimeId)}" data-runtime-name="${escapeHtml(runtime.name)}">${escapeHtml(t("runtimes.actions.delete"))}</button>
            </div>
            <details class="runtime-row-menu">
              <summary class="quiet">${escapeHtml(t("runtimes.actions.menu"))}</summary>
              <div class="runtime-row-menu-panel">
                <button type="button" data-action="show-attention" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimes.actions.attention"))}</button>
                <button type="button" data-action="refresh-status" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimes.actions.status"))}</button>
                ${runtime.sidecarEndpoint ? `<button type="button" data-action="refresh-sidecar" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimeDetail.refreshSidecar"))}</button>` : ""}
                <button type="button" data-action="refresh-all" data-runtime-id="${escapeHtml(runtime.runtimeId)}">${escapeHtml(t("runtimes.actions.all"))}</button>
                <button type="button" data-action="delete-runtime" data-runtime-id="${escapeHtml(runtime.runtimeId)}" data-runtime-name="${escapeHtml(runtime.name)}">${escapeHtml(t("runtimes.actions.delete"))}</button>
              </div>
            </details>
          </td>
        </tr>
      `;
    }).join("");
  } else {
    updateRuntimeTableSelection(state.selectedRuntimeId);
  }

  const selectedRuntime = items.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
  const selectedAttention = selectedRuntime
    ? state.runtimeAttentionById.get(selectedRuntime.runtimeId) || attentionMap.get(selectedRuntime.runtimeId) || null
    : null;
  renderRuntimeDetail(selectedRuntime, selectedAttention);
  renderRuntimePanel(selectedRuntime);
}

bootstrapDashboard();
