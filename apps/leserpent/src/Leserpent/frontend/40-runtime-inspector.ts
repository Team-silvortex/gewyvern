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

function recoveryActionKind(action) {
  return action === "refresh_all"
    ? "all"
    : action === "refresh_status"
      ? "status"
      : action === "refresh_sidecar"
        ? "sidecar"
        : null;
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
  if (hint) {
    return hint;
  }

  if (action === "refresh_status") {
    return t("attention.hints.refreshStatus");
  }

  if (action === "refresh_all") {
    return t("attention.hints.refreshAll");
  }

  if (action === "refresh_sidecar") {
    return t("attention.hints.refreshSidecar");
  }

  return "";
}

function runtimeDetailSignature(runtime, attention) {
  if (!runtime) {
    return `empty:${state.language}`;
  }

  const capabilityKeys = (runtime.capabilities || [])
    .map((item) => `${item.key}:${item.support}`)
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
    runtime.tags.environment || "",
    runtime.tags.cluster || "",
    runtime.tags.role || "",
    runtime.status.statusSource,
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

function runtimePanelSignature(runtime) {
  if (!runtime) {
    return `empty:${state.language}:${state.runtimePanelView}`;
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
  ].join("::");
}

function renderRuntimeDetail(runtime, attention) {
  const signature = runtimeDetailSignature(runtime, attention);
  if (state.renderSignatures.runtimeDetail === signature) {
    return;
  }
  state.renderSignatures.runtimeDetail = signature;

  if (!runtime) {
    nodes.runtimeDetailChip.textContent = t("runtimeDetail.nothingSelected");
    nodes.runtimeDetailActions.classList.add("hidden");
    nodes.runtimeDetailRefreshSidecar.disabled = true;
    nodes.runtimeDetailEmpty.classList.remove("hidden");
    nodes.runtimeDetailPanel.classList.add("hidden");
    nodes.runtimeDetailIdentity.innerHTML = "";
    nodes.runtimeDetailStatus.innerHTML = "";
    nodes.runtimeDetailCapabilities.innerHTML = "";
    nodes.runtimeDetailAttention.innerHTML = "";
    return;
  }

  const badge = statusBadge(runtime.status);
  const sidecarBadge = sidecarStatusBadge(runtime.sidecarStatus);
  nodes.runtimeDetailChip.textContent = runtime.name;
  nodes.runtimeDetailActions.classList.remove("hidden");
  nodes.runtimeDetailRefreshSidecar.disabled = !runtime.sidecarEndpoint;
  nodes.runtimeDetailEmpty.classList.add("hidden");
  nodes.runtimeDetailPanel.classList.remove("hidden");
  nodes.runtimeDetailIdentity.innerHTML = `
    <div><strong>${escapeHtml(runtime.name)}</strong></div>
    <div class="item-meta">${escapeHtml(runtime.endpoint)}</div>
    ${runtime.sidecarEndpoint ? `<div class="item-meta">${escapeHtml(t("register.sidecarEndpoint"))}: ${escapeHtml(runtime.sidecarEndpoint)}</div>` : ""}
    <div class="hint-line">${escapeHtml(t("runtimeDetail.registered"))}: ${escapeHtml(runtime.registeredAt)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.updated"))}: ${escapeHtml(runtime.updatedAt)}</div>
    <div class="group-list">
      <span class="tag-pill">${escapeHtml(runtime.tags.environment || t("runtimes.states.noEnv"))}</span>
      <span class="tag-pill">${escapeHtml(runtime.tags.cluster || t("runtimes.states.noCluster"))}</span>
      <span class="tag-pill">${escapeHtml(runtime.tags.role || t("runtimes.states.noRole"))}</span>
    </div>
  `;
  nodes.runtimeDetailStatus.innerHTML = `
    <div><span class="runtime-state ${escapeHtml(badge.tone)}">${escapeHtml(badge.text)}</span></div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.source"))}: ${escapeHtml(runtime.status.statusSource)}</div>
    ${runtime.status.resilienceStatus ? `<div class="hint-line">${escapeHtml(t("runtimeDetail.resilienceStatus"))}: ${escapeHtml(runtime.status.resilienceStatus)}</div>` : ""}
    ${runtime.status.resilienceSummary ? `<div class="hint-line">${escapeHtml(t("runtimeDetail.resilienceSummary"))}: ${escapeHtml(runtime.status.resilienceSummary)}</div>` : ""}
    ${runtime.status.socketServiceStatus ? `<div class="hint-line">${escapeHtml(t("runtimeDetail.socketServiceStatus"))}: ${escapeHtml(runtime.status.socketServiceStatus)}</div>` : ""}
    ${runtime.status.socketTotalIdleTimeouts != null ? `<div class="hint-line">${escapeHtml(t("runtimeDetail.idleTimeouts"))}: ${escapeHtml(`${runtime.status.socketConsecutiveIdleTimeouts ?? 0} / ${runtime.status.socketTotalIdleTimeouts ?? 0}`)}</div>` : ""}
    <div class="hint-line">${escapeHtml(t("runtimeDetail.snapshotKind"))}: ${escapeHtml(runtime.status.snapshotKind || t("runtimeDetail.none"))}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.targetCount"))}: ${escapeHtml(runtime.status.targetCount ?? t("runtimeDetail.na"))}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.summaryJson"))}: ${escapeHtml(runtime.status.hasSummaryJson)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.analysisJson"))}: ${escapeHtml(runtime.status.hasAnalysisJson)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.trainingExampleJson"))}: ${escapeHtml(runtime.status.hasTrainingExampleJson)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.trainingDatasetManifest"))}: ${escapeHtml(runtime.status.hasTrainingDatasetManifest)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.exportJson"))}: ${escapeHtml(runtime.status.hasExportJson)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.reportJson"))}: ${escapeHtml(runtime.status.hasReportJson)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.reportHtml"))}: ${escapeHtml(runtime.status.hasReportHtml)}</div>
    <div class="hint-line">${escapeHtml(t("register.sidecarEndpoint"))}: <strong>${escapeHtml(runtime.sidecarEndpoint || t("register.sidecarUnpaired"))}</strong></div>
    ${runtime.sidecarEndpoint ? `<div class="hint-line">${escapeHtml(t("runtimeDetail.sidecarAccess"))}: <strong>${escapeHtml(runtime.hasSidecarAdminToken ? t("runtimeDetail.sidecarProtected") : t("runtimeDetail.sidecarOpen"))}</strong></div>` : ""}
    <div class="hint-line">${escapeHtml(t("runtimes.columns.sidecar"))}: <span class="runtime-state ${escapeHtml(sidecarBadge.tone)}">${escapeHtml(sidecarBadge.text)}</span></div>
    ${runtime.sidecarStatus ? `<div class="hint-line">${escapeHtml(t("runtimeDetail.sidecarSource"))}: ${escapeHtml(runtime.sidecarStatus.statusSource)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.sidecarLearning"))}: ${escapeHtml(runtime.sidecarStatus.learningActive)} · ${escapeHtml(runtime.sidecarStatus.learnedRoutes)}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.sidecarMemory"))}: ${escapeHtml(runtime.sidecarStatus.memory?.versionsSupported ? `${runtime.sidecarStatus.memory.slotCount} slots · ${runtime.sidecarStatus.memory.historyCount} history` : t("runtimeDetail.none"))}</div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.sidecarMemoryLatest"))}: ${escapeHtml(latestSidecarMemoryText(runtime.sidecarStatus))}</div>` : ""}
  `;
  const capabilityKeys = (runtime.capabilities || [])
    .map((item) => [item.key, item.support])
    .sort((a, b) => a[0].localeCompare(b[0]));
  nodes.runtimeDetailCapabilities.innerHTML = capabilityKeys.length
    ? capabilityKeys.map(([key, support]) => `<span class="tag-pill">${escapeHtml(key)} · ${escapeHtml(support)}</span>`).join("")
    : `<span class="item-meta">${escapeHtml(t("runtimeDetail.noCapabilities"))}</span>`;

  if (!attention) {
    nodes.runtimeDetailAttention.innerHTML = `
      <div><span class="runtime-state good">${escapeHtml(t("runtimeDetail.clear"))}</span></div>
      <div class="hint-line">${escapeHtml(t("runtimeDetail.noAttention"))}</div>
    `;
    return;
  }

  nodes.runtimeDetailAttention.innerHTML = `
    <div><span class="runtime-state ${attention.severity === "critical" ? "bad" : "warn"}">${escapeHtml(t(`attention.${attention.severity}`))}</span></div>
    <div class="hint-line">${escapeHtml(t("runtimeDetail.needsAttention"))}: ${escapeHtml(attention.needsAttention)}</div>
    <div class="reason-list">
      ${(attention.reasons || []).map((reason) => `<span class="reason-pill">${escapeHtml(t(`attention.${reason}`) || reason)}</span>`).join("")}
    </div>
    <div class="hint-line"><strong>${escapeHtml(t("attention.suggestedActions"))}</strong></div>
    <div class="inline-actions">
      ${(attention.suggestedActions || []).length
        ? attention.suggestedActions.map((action) => {
          const kind = recoveryActionKind(action.action);
          return kind
            ? `<button type="button" data-recovery-action="${escapeHtml(kind)}" ${action.coolingDown ? "disabled" : ""}>${escapeHtml(recoveryActionLabel(action.action))} · #${escapeHtml(action.priority)}${action.coolingDown ? ` · ${escapeHtml(t("attention.cooldownRemaining", { seconds: action.cooldownSecondsRemaining }))}` : ""}</button>`
            : `<span class="tag-pill">${escapeHtml(recoveryActionLabel(action.action))}</span>`;
        }).join("")
        : `<span class="item-meta">${escapeHtml(t("attention.noReasons"))}</span>`}
    </div>
    ${(attention.suggestedActions || []).length ? `
      <div class="stack">
        ${(attention.suggestedActions || []).map((action) => `
          <div class="hint-line">${escapeHtml(t("attention.actionHint"))}: ${escapeHtml(recoveryHintLabel(action.action, action.hint))}${action.coolingDown ? ` · ${escapeHtml(t("attention.cooldownRemaining", { seconds: action.cooldownSecondsRemaining }))}` : ""}</div>
        `).join("")}
      </div>
    ` : ""}
    <div class="hint-line"><strong>${escapeHtml(t("attention.recentRecovery"))}</strong></div>
    <div class="stack">
      ${(attention.recentRecoveryActivities || []).length
        ? attention.recentRecoveryActivities.map((item) => `
          <div class="hint-line">${escapeHtml(recoveryActionLabel(item.action))} · <strong>${escapeHtml(recoveryOutcomeLabel(item.outcome))}</strong> · ${escapeHtml(item.recordedAt)}${item.summary ? ` · ${escapeHtml(item.summary)}` : ""}</div>
        `).join("")
        : `<div class="hint-line">${escapeHtml(t("attention.noRecoveryHistory"))}</div>`}
    </div>
  `;
}

function renderRuntimePanel(runtime) {
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
    return;
  }

  if (!runtimeSupportsPanelView(runtime, state.runtimePanelView)) {
    state.runtimePanelView = isSidecarView(state.runtimePanelView) ? "sidecar-root" : "root";
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
}

async function refreshRuntimeById(runtimeId, kind) {
  if (!runtimeId) {
    nodes.statusLine.textContent = t("notifications.noRuntimeSelected");
    return;
  }

  const label = refreshLabel(kind);
  nodes.statusLine.textContent = `${label}...`;

  try {
    if (kind === "all") {
      await postJson(`/v1/runtimes/${runtimeId}/refresh-capabilities`);
      await postJson(`/v1/runtimes/${runtimeId}/refresh-status`);
      const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
      if (selectedRuntime?.sidecarEndpoint) {
        await postJson(`/v1/runtimes/${runtimeId}/refresh-sidecar`);
        markBadgeRefresh("sidecar");
      }
      markBadgeRefresh("runtime");
    } else if (kind === "status") {
      await postJson(`/v1/runtimes/${runtimeId}/refresh-status`);
      markBadgeRefresh("runtime");
    } else if (kind === "sidecar") {
      await postJson(`/v1/runtimes/${runtimeId}/refresh-sidecar`);
      markBadgeRefresh("sidecar");
    } else {
      await postJson(`/v1/runtimes/${runtimeId}/refresh-capabilities`);
    }

    state.activeTab = "runtimes";
    state.selectedRuntimeId = runtimeId;
    await loadDashboard();
    const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
    if (selectedRuntime) {
      renderRuntimePanel(selectedRuntime);
      window.setTimeout(() => {
        const latestSelected = state.latestRuntimes.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
        if (latestSelected) {
          renderRuntimePanel(latestSelected);
        }
      }, 2500);
    }
    nodes.statusLine.textContent = t("notifications.runtimeRefreshComplete", { label });
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.runtimeRefreshFailed", { label, message: error.message });
  }
}

async function refreshSelectedRuntime(kind) {
  await refreshRuntimeById(state.selectedRuntimeId, kind);
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
