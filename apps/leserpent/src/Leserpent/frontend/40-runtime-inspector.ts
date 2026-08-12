// @ts-nocheck
// Runtime detail, recovery, and child-panel rendering split from app.ts.

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
    if (!id || seen.has(id)) continue;
    ids.push(id);
    seen.add(id);
    if (ids.length >= limit) break;
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
  } catch {
    // Window persistence is a convenience; the workspace still works without storage.
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
  } catch {
    state.runtimeWindowIds = [];
    state.activeRuntimeWindowId = null;
    state.runtimeWindowViews = Object.create(null);
  }
}

function applyRuntimeWindowDeepLink(runtimeId, view) {
  const id = normalizeRuntimeWindowId(runtimeId);
  if (!id) return;
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
  state.runtimeWindowIds = sanitizeRuntimeWindowIds(
    sanitizeRuntimeWindowIds(state.runtimeWindowIds, MAX_RUNTIME_WINDOWS + 1)
      .filter((id) => available.has(id)),
  );
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
  if (!runtimeId) return false;
  if (!state.runtimeWindowIds.includes(runtimeId)) {
    if (state.runtimeWindowIds.length >= MAX_RUNTIME_WINDOWS) {
      nodes.statusLine.textContent = t("runtimePanel.windows.limitReached", { limit: MAX_RUNTIME_WINDOWS });
      return false;
    }
    state.runtimeWindowIds.push(runtimeId);
  }
  state.activeRuntimeWindowId = runtimeId;
  state.selectedRuntimeId = runtimeId;
  state.runtimePanelView = normalizeRuntimeWindowView(
    state.runtimeWindowViews[runtimeId] || state.runtimePanelView,
  );
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
    if (state.runtimeWindowIds.length >= MAX_RUNTIME_WINDOWS) break;
    if (!state.runtimeWindowIds.includes(runtime.runtimeId)) {
      state.runtimeWindowIds.push(runtime.runtimeId);
    }
    state.runtimeWindowViews[runtime.runtimeId] ||= "root";
  }
  state.activeRuntimeWindowId ||= state.runtimeWindowIds[0] || null;
  if (state.activeRuntimeWindowId) state.selectedRuntimeId = state.activeRuntimeWindowId;
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

function closeRuntimeWindow(runtimeId) {
  const closedIndex = state.runtimeWindowIds.indexOf(runtimeId);
  state.runtimeWindowIds = state.runtimeWindowIds.filter((id) => id !== runtimeId);
  delete state.runtimeWindowViews[runtimeId];
  if (state.activeRuntimeWindowId === runtimeId) {
    state.activeRuntimeWindowId = state.runtimeWindowIds[
      Math.min(Math.max(closedIndex, 0), state.runtimeWindowIds.length - 1)
    ] || null;
  }
  if (state.activeRuntimeWindowId) {
    state.selectedRuntimeId = state.activeRuntimeWindowId;
    state.runtimePanelView = state.runtimeWindowViews[state.activeRuntimeWindowId] || "root";
  }
  state.renderSignatures.runtimePanel = "";
  persistRuntimeWindows();
  renderRuntimeSliceFromCache();
  syncLocation();
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
  if (!state.runtimeWindowIds.includes(runtimeId)) return;
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
  if (!(button instanceof HTMLElement)) return;
  const runtimeId = button.dataset.runtimeId;
  const action = button.dataset.runtimeWindowAction;
  if (action === "close") {
    closeRuntimeWindow(runtimeId);
  } else if (action === "external") {
    const runtime = state.latestRuntimes.find((item) => item.runtimeId === runtimeId);
    const url = runtimePanelUrl(runtime, state.runtimeWindowViews[runtimeId] || "root");
    if (url) window.open(url, "_blank", "noopener,noreferrer");
  } else {
    activateRuntimeWindow(runtimeId);
  }
}

function handleRuntimeWindowGridKeydown(event) {
  const identity = event.target.closest(".runtime-child-window-identity[data-runtime-id]");
  if (!(identity instanceof HTMLButtonElement)) return;
  const ids = state.runtimeWindowIds;
  const index = ids.indexOf(identity.dataset.runtimeId);
  if (index < 0) return;

  const direction = document.documentElement.dir === "rtl" ? -1 : 1;
  let nextIndex = null;
  if (event.key === "ArrowRight") nextIndex = index + direction;
  if (event.key === "ArrowLeft") nextIndex = index - direction;
  if (event.key === "ArrowDown") nextIndex = index + 1;
  if (event.key === "ArrowUp") nextIndex = index - 1;
  if (event.key === "Home") nextIndex = 0;
  if (event.key === "End") nextIndex = ids.length - 1;
  if (nextIndex === null || !ids.length) return;

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
    if (!wanted.has(card.dataset.runtimeWindowId)) card.remove();
  }

  for (const runtimeId of state.runtimeWindowIds) {
    const runtime = state.latestRuntimes.find((item) => item.runtimeId === runtimeId);
    if (!runtime) continue;
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
    } else if (blank) {
      blankNode.innerHTML = runtimePanelBlankMarkup(runtime, trust, url, view);
      blankNode.classList.remove("hidden");
      frame.classList.add("hidden");
      frame.src = "about:blank";
      delete frame.dataset.src;
    } else {
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
  if (!value) return t("runtimeDetail.notObserved");
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return String(value);
  try {
    return new Intl.DateTimeFormat(state.language, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(parsed);
  } catch {
    return parsed.toLocaleString();
  }
}

function runtimeDetailTimeMarkup(value) {
  if (!value) return escapeHtml(t("runtimeDetail.notObserved"));
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
  if (support === "fully_supported") return "good";
  if (support === "unsupported" || support === "not_supported") return "bad";
  return "warn";
}

function attentionSeverityLabel(attention) {
  const severity = attention?.severity || "warning";
  const key = `attention.${severity}`;
  const translated = t(key);
  return translated === key ? severity : translated;
}

function runtimeNeedsAttention(attention) {
  if (!attention) return false;
  if (typeof attention.needsAttention === "boolean") return attention.needsAttention;
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
  if (state.renderSignatures.runtimeDetail === signature) return;
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
  const attentionButton = nodes.runtimeDetailSubtabButtons.find(
    (button) => button.dataset.runtimeDetailTab === "attention",
  );
  const statusButton = nodes.runtimeDetailSubtabButtons.find(
    (button) => button.dataset.runtimeDetailTab === "status",
  );
  const needsAttention = runtimeNeedsAttention(attention);
  attentionButton?.classList.toggle("has-attention", needsAttention);
  if (attentionButton) attentionButton.dataset.tone = needsAttention ? attention.severity : "clear";
  statusButton?.classList.toggle("has-status-warning", badge.tone !== "good");
  if (statusButton) statusButton.dataset.tone = badge.tone;

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
    } else {
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
    } else {
      nodes.runtimePanelUrl.textContent = "";
      nodes.runtimePanelUrl.classList.add("hidden");
    }
  } else {
    nodes.runtimePanelBlank.classList.add("hidden");
    nodes.runtimePanelBlank.innerHTML = "";
    nodes.runtimePanelFrame.classList.remove("hidden");
    nodes.runtimePanelFrame.src = url;
    if (url) {
      nodes.runtimePanelUrl.textContent = url;
      nodes.runtimePanelUrl.classList.remove("hidden");
    } else {
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
    for (const control of detailControls) control.disabled = true;
    nodes.statusLine.textContent = `${label}...`;

    try {
      const recovery = await postJsonBody(`/v1/runtimes/${runtimeId}/recovery`, { kind });
      if ((recovery.steps || []).some((step) => step.kind === "status")) markBadgeRefresh("runtime");
      if ((recovery.steps || []).some((step) => step.kind === "sidecar")) markBadgeRefresh("sidecar");

      state.activeTab = "runtimes";
      state.selectedRuntimeId = runtimeId;
      await loadDashboard();
      const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
      if (selectedRuntime) {
        renderRuntimePanel(selectedRuntime);
        window.setTimeout(() => {
          const latestSelected = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
          if (latestSelected) renderRuntimePanel(latestSelected);
        }, 2500);
      }
      nodes.statusLine.textContent = t("notifications.runtimeRefreshComplete", { label });
    } catch (error) {
      console.error(error);
      nodes.statusLine.textContent = t("notifications.runtimeRefreshFailed", { label, message: error.message });
    } finally {
      for (const control of detailControls) control.disabled = false;
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
  } catch (error) {
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
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.runtimeLinkFailed", { message: error.message });
  }
}
