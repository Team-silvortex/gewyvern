// @ts-nocheck
// Split from app.ts to keep the control-plane shell maintainable.

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
