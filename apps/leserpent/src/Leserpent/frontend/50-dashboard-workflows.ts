// @ts-nocheck
// Dashboard loading, persistence, and registration workflows split from app.ts.

function syncFilterInputs() {
  nodes.environmentInput.value = state.filter.environment;
  nodes.clusterInput.value = state.filter.cluster;
  nodes.roleInput.value = state.filter.role;
  nodes.runtimeSearch.value = state.runtimeSearch;
  nodes.runtimeSort.value = state.runtimeSort;
  const parts = [state.filter.environment, state.filter.cluster, state.filter.role].filter(Boolean);
  nodes.fleetFilterChip.textContent = parts.length ? parts.join(" / ") : t("filters.allRuntimes");
  if (state.activeTab === "runtimes" && state.activeRuntimeMainTab === "register") {
    renderRegisterPreview();
  }
}

function clearRegisterForm() {
  state.registerNameTouched = false;
  nodes.registerName.value = "";
  nodes.registerEndpoint.value = "";
  nodes.registerSidecarEndpoint.value = "";
  nodes.registerSidecarAdminToken.value = "";
  nodes.registerToken.value = "";
  syncRegisterFormTagsFromFilter();
  nodes.registerFetchCapabilities.checked = true;
  renderRegisterPreview();
  nodes.registerResult.textContent = t("register.untouched");
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

function currentSliceRuntimeIds() {
  return new Set(currentSliceRuntimes().map((runtime) => runtime.runtimeId));
}

function currentFailedRuntimeCount() {
  return currentSliceRuntimes().filter((runtime) => runtime.status?.statusSource === "fetch_failed").length;
}

function currentSliceCount() {
  return currentSliceRuntimes().length;
}

function currentUnobservedRuntimeCount() {
  return currentSliceRuntimes().filter((runtime) =>
    runtime.status?.statusSource === "unobserved" && !isIdleReadyStatus(runtime.status)).length;
}

function currentSliceSessionCount() {
  const runtimeIds = currentSliceRuntimeIds();
  return (state.cache.sessions?.sessions || []).filter((session) => runtimeIds.has(session.runtimeId)).length;
}

function currentSliceRiskLevel() {
  const values = [state.filter.environment, state.filter.cluster, state.filter.role]
    .filter(Boolean)
    .map((value) => value.toLowerCase());
  return values.some((value) => value.includes("prod") || value.includes("live"));
}

function currentSliceRiskWarning() {
  return currentSliceRiskLevel() ? `\n\n${t("notifications.runtimeCleanupProtectedWarning")}` : "";
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
    } else if (state.activeOverviewTab === "attention") {
      renderMetricCards(nodes.attentionSummaryCards, [
        [t("metrics.critical"), attentionSummary.summary.criticalCount],
        [t("metrics.warning"), attentionSummary.summary.warningCount],
      ], "attentionSummaryCards");
      renderAttentionReasons(attentionSummary.summary);
    } else {
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
    const [capabilities, fleetSummary, attentionSummary, attentionList, runtimes, sessions] = await Promise.all([
      getJson("/v1/capabilities", abortController.signal),
      getJson(`/v1/fleet/summary${query}`, abortController.signal),
      getJson(`/v1/fleet/attention-summary${query}`, abortController.signal),
      getJson(`/v1/fleet/runtimes-needing-attention${query}`, abortController.signal),
      getJson(`/v1/runtimes${query}`, abortController.signal),
      getJson("/v1/sessions", abortController.signal),
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
  } catch (error) {
    if (error?.name === "AbortError") {
      return;
    }
    if (requestId !== state.dashboardRequestSeq) {
      return;
    }
    console.error(error);
    nodes.statusLine.textContent = t("notifications.dashboardLoadFailed", { message: error.message });
    if (looksLikeTokenDenied(error.message)) {
      renderSecurityState(null);
      nodes.adminTokenState.textContent = t("security.tokenRequired");
      nodes.securityDetails?.setAttribute("open", "open");
    }
  } finally {
    if (state.dashboardAbortController === abortController) {
      state.dashboardAbortController = null;
    }
  }
}

async function postAndReload(path, label) {
  nodes.statusLine.textContent = `${label}...`;
  try {
    await postJson(`${path}${buildQuery()}`);
    await loadDashboard();
    nodes.statusLine.textContent = t("notifications.fleetRefreshComplete", { label });
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.fleetRefreshFailed", { label, message: error.message });
  }
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
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.runtimeDeleteFailed", { message: error.message });
  }
}

async function deleteFailedRuntimes() {
  const slice = currentSliceLabel();
  const targets = currentSliceRuntimes().filter((runtime) => runtime.status?.statusSource === "fetch_failed");
  const count = targets.length;
  const confirmed = window.confirm(
    `${t("notifications.runtimeDeleteFailedSliceConfirm", { slice, count })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}`);
  if (!confirmed) {
    return;
  }

  nodes.statusLine.textContent = `${t("runtimes.actions.deleteFailed")}...`;
  try {
    const result = await postJson(`/v1/runtimes/delete-failed${buildQuery()}`);
    nodes.runtimeCleanupMenu?.removeAttribute("open");
    resetRuntimeSelectionAfterBulkDelete();
    await loadDashboard();
    nodes.statusLine.textContent = t("notifications.runtimeDeleteFailedSliceDone", {
      count: result.removedRuntimeCount ?? 0,
      sessions: result.removedSessionCount ?? 0,
      slice,
    });
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
  }
}

async function deleteUnobservedRuntimes() {
  const slice = currentSliceLabel();
  const targets = currentSliceRuntimes().filter((runtime) =>
    runtime.status?.statusSource === "unobserved" && !isIdleReadyStatus(runtime.status));
  const count = targets.length;
  const confirmed = window.confirm(
    `${t("notifications.runtimeDeleteUnobservedSliceConfirm", { slice, count })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}`);
  if (!confirmed) {
    return;
  }

  nodes.statusLine.textContent = `${t("runtimes.actions.deleteUnobserved")}...`;
  try {
    const result = await postJson(`/v1/runtimes/delete-unobserved${buildQuery()}`);
    nodes.runtimeCleanupMenu?.removeAttribute("open");
    resetRuntimeSelectionAfterBulkDelete();
    await loadDashboard();
    nodes.statusLine.textContent = t("notifications.runtimeDeleteUnobservedSliceDone", {
      count: result.removedRuntimeCount ?? 0,
      sessions: result.removedSessionCount ?? 0,
      slice,
    });
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
  }
}

async function clearRuntimeSlice() {
  const slice = currentSliceLabel();
  const targets = currentSliceRuntimes();
  const confirmed = window.confirm(
    `${t("notifications.runtimeClearSliceConfirm", { slice, count: currentSliceCount() })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}`);
  if (!confirmed) {
    return;
  }

  nodes.statusLine.textContent = `${t("runtimes.actions.clearSlice")}...`;
  try {
    const result = await postJson(`/v1/runtimes/delete-slice${buildQuery()}`);
    nodes.runtimeCleanupMenu?.removeAttribute("open");
    resetRuntimeSelectionAfterBulkDelete();
    await loadDashboard();
    nodes.statusLine.textContent = t("notifications.runtimeClearSliceDone", {
      count: result.removedRuntimeCount ?? 0,
      sessions: result.removedSessionCount ?? 0,
      slice,
    });
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
  }
}

async function savePersistenceNow() {
  nodes.statusLine.textContent = t("persistence.saving");
  try {
    await postJson("/v1/persistence/save");
    await loadDashboard();
    nodes.statusLine.textContent = t("persistence.saved");
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("persistence.saveFailed", { message: error.message });
  }
}

async function exportPersistenceState() {
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
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("persistence.exportFailed", { message: error.message });
  }
}

function triggerPersistenceImportPicker() {
  nodes.persistenceImportFile.value = "";
  nodes.persistenceImportFile.click();
}

async function importPersistenceState(file) {
  if (!file) {
    return;
  }

  nodes.statusLine.textContent = t("persistence.importing", { file: file.name });
  try {
    const text = await file.text();
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch {
      throw new Error(t("persistence.invalidJson"));
    }

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
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("persistence.importFailed", { message: error.message });
  }
}

async function submitRegisterForm(event) {
  event.preventDefault();
  const name = nodes.registerName.value.trim();
  const endpoint = nodes.registerEndpoint.value.trim();
  const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
  if (!isLikelyHttpEndpoint(endpoint)) {
    nodes.registerResult.textContent = t("register.blockedEndpoint");
    state.activeTab = "runtimes";
    state.activeRuntimeMainTab = "register";
    applyTabShell();
    return;
  }

  if (sidecarEndpoint && !isLikelyHttpEndpoint(sidecarEndpoint)) {
    nodes.registerResult.textContent = t("register.blockedSidecarEndpoint");
    state.activeTab = "runtimes";
    state.activeRuntimeMainTab = "register";
    applyTabShell();
    return;
  }

  const duplicate = findDuplicateRuntime(name, endpoint);
  if (duplicate) {
    const nameConflict = duplicate.name.toLowerCase() === name.toLowerCase();
    const endpointConflict = duplicate.endpoint.toLowerCase() === endpoint.toLowerCase();
    const conflictReason = nameConflict && endpointConflict
      ? t("register.duplicateNameAndEndpoint")
      : nameConflict
        ? t("register.duplicateName")
        : t("register.duplicateEndpoint");
    nodes.registerResult.textContent = t("register.blockedDuplicate", {
      reason: conflictReason,
      name: duplicate.name,
      endpoint: duplicate.endpoint,
    });
    state.activeTab = "runtimes";
    state.activeRuntimeMainTab = "register";
    applyTabShell();
    return;
  }

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
  };

  nodes.registerResult.textContent = t("register.registering");
  try {
    const result = await postJsonBody("/v1/runtimes/register", body);
    state.registerNameTouched = false;
    state.activeTab = "runtimes";
    state.activeRuntimeMainTab = "detail";
    state.selectedRuntimeId = result.runtimeId;
    nodes.registerResult.textContent = t("register.registered", {
      name: result.name,
      runtimeId: result.runtimeId,
      slice: currentSliceLabel(),
      status: runtimeStatusHint(result.status),
    });
    await loadDashboard();
    nodes.runtimeDetailPanel.scrollIntoView({ behavior: "smooth", block: "start" });
  } catch (error) {
    console.error(error);
    nodes.registerResult.textContent = t("register.failed", { message: error.message });
    state.activeTab = "runtimes";
    state.activeRuntimeMainTab = "register";
    applyTabShell();
  }
}
