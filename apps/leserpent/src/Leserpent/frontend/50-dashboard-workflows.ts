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
    if (busyLabel) button.textContent = busyLabel;
  }
  try {
    return await action();
  } finally {
    state.uiActions.delete(key);
    if (button) {
      button.removeAttribute("aria-busy");
      delete button.dataset.busy;
      button.textContent = previousLabel;
      button.disabled = false;
    }
    if (key === "runtime-cleanup") syncCleanupMenuState();
    if (key === "register-runtime") renderRegisterPreview();
  }
}

function setCleanupControlsBusy(busy) {
  for (const button of [nodes.runtimeDeleteFailed, nodes.runtimeDeleteUnobserved, nodes.runtimeClearSlice]) {
    if (!button) continue;
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
  if (nodes.runtimeDeleteFailed) {
    nodes.runtimeDeleteFailed.disabled = cleanupBusy || currentFailedRuntimeCount() === 0;
    nodes.runtimeDeleteFailed.toggleAttribute("aria-busy", cleanupBusy);
  }
  if (nodes.runtimeDeleteUnobserved) {
    nodes.runtimeDeleteUnobserved.disabled = cleanupBusy || currentUnobservedRuntimeCount() === 0;
    nodes.runtimeDeleteUnobserved.toggleAttribute("aria-busy", cleanupBusy);
  }
  if (nodes.runtimeClearSlice) {
    nodes.runtimeClearSlice.disabled = cleanupBusy || currentSliceCount() === 0;
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
  } catch (error) {
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
      } else {
        nodes.securityDetails?.removeAttribute("open");
        nodes.statusLine.textContent = t("security.tokenMissing");
      }
      return;
    }
    nodes.statusLine.textContent = t("notifications.dashboardLoadFailed", { message: error.message });
  } finally {
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
    } catch (error) {
      console.error(error);
      nodes.statusLine.textContent = t("notifications.fleetRefreshFailed", { label, message: error.message });
    } finally {
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
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = t("notifications.runtimeDeleteFailed", { message: error.message });
  }
}

async function deleteFailedRuntimes() {
  const slice = currentSliceLabel();
  const plan = cleanupAction("failed");
  const targets = plan?.targets || [];
  const count = plan?.runtimeCount ?? 0;
  if (!count || state.uiActions.has("runtime-cleanup")) {
    syncCleanupMenuState();
    return;
  }
  const confirmed = window.confirm(
    `${t("notifications.runtimeDeleteFailedSliceConfirm", { slice, count })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}`);
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
    } catch (error) {
      console.error(error);
      nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
    } finally {
      syncCleanupMenuState();
    }
  });
}

async function deleteUnobservedRuntimes() {
  const slice = currentSliceLabel();
  const plan = cleanupAction("unobserved");
  const targets = plan?.targets || [];
  const count = plan?.runtimeCount ?? 0;
  if (!count || state.uiActions.has("runtime-cleanup")) {
    syncCleanupMenuState();
    return;
  }
  const confirmed = window.confirm(
    `${t("notifications.runtimeDeleteUnobservedSliceConfirm", { slice, count })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}`);
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
    } catch (error) {
      console.error(error);
      nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
    } finally {
      syncCleanupMenuState();
    }
  });
}

async function clearRuntimeSlice() {
  const slice = currentSliceLabel();
  const plan = cleanupAction("slice");
  const targets = plan?.targets || [];
  if (!plan?.runtimeCount || state.uiActions.has("runtime-cleanup")) {
    syncCleanupMenuState();
    return;
  }
  const challenge = plan.challenge;
  const entered = window.prompt(
    `${t("notifications.runtimeClearSliceConfirm", { slice, count: currentSliceCount() })}${describeCleanupTargets(targets)}${currentSliceRiskWarning()}\n\n${t("notifications.runtimeClearSliceChallenge", { challenge })}`,
    "",
  );
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
    } catch (error) {
      console.error(error);
      nodes.statusLine.textContent = t("notifications.runtimeDeleteBatchFailed", { message: error.message });
    } finally {
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
    } catch (error) {
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
    } catch (error) {
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

  if (state.uiActions.has("persistence-import")) return;
  try {
    if (file.size > 1_048_576) throw new Error(t("persistence.importTooLarge"));
    const text = await file.text();
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch {
      throw new Error(t("persistence.invalidJson"));
    }

    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)
      || !Array.isArray(parsed.runtimes) || !Array.isArray(parsed.sessions)) {
      throw new Error(t("persistence.invalidStructure"));
    }
    if (parsed.schemaVersion !== 1) {
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
  if (state.uiActions.has("register-runtime")) return;
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

  const registrationPlan = currentRegistrationPlan();
  if (!registrationPlan?.allowed) {
    nodes.registerResult.textContent = registrationPlanConflictMessage(registrationPlan)
      || state.registrationPlanError
      || t("register.blockedEndpoint");
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
    registrationPlanToken: registrationPlan.planToken,
  };

  await runUiActionOnce("register-runtime", nodes.registerSubmit, t("register.registeringShort"), async () => {
    nodes.registerResult.textContent = t("register.registering");
    try {
      const result = await postJsonBody("/v1/runtimes/register", body);
      state.registrationPlan = null;
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
  });
}
