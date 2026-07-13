// @ts-nocheck
// Preference storage, shared translation lookup, and bootstrap wiring split from app.ts.

function t(key, params = {}) {
  const parts = key.split(".");
  const localeTree = translations[state.language] || translations.en;
  let value = localeTree;
  for (const part of parts) value = value?.[part];
  if (typeof value !== "string") {
    value = translations.en;
    for (const part of parts) value = value?.[part];
  }
  if (typeof value !== "string") {
    value = key;
  }

  return value.replace(/\{(\w+)\}/g, (_, name) => String(params[name] ?? `{${name}}`));
}

function getStoredLanguagePreference() {
  try {
    return window.localStorage.getItem(storageKeys.languagePreference);
  } catch {
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
  } catch {
    // ignore storage failures
  }
}

function getStoredThemePreference() {
  try {
    return window.localStorage.getItem(storageKeys.themePreference);
  } catch {
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
  } catch {
    // ignore storage failures
  }
}

function activateTab(tab) {
  state.activeTab = tab;
  applyTabShell();
  syncLocation();
  if (tab === "orchestra") {
    void loadOrchestraPlan();
    void loadOrchestraFleetBoard();
  }
}

function activateOverviewSubtab(tab) {
  state.activeOverviewTab = tab;
  applyTabShell();
  syncLocation();
}

function activateRuntimeMainTab(tab) {
  state.activeRuntimeMainTab = ["register", "detail", "panel"].includes(tab) ? tab : "select";
  state.activeRuntimeSideTab = state.activeRuntimeMainTab === "panel" ? "panel" : "detail";
  state.activeTab = "runtimes";
  applyTabShell();
  syncLocation();
}

function activateRuntimeDetailTab(tab) {
  state.activeRuntimeDetailTab = tab;
  applyTabShell();
  syncLocation();
}

async function handleRuntimeTableAction(button) {
  const runtimeId = button.dataset.runtimeId;
  if (!runtimeId) {
    return;
  }

  if (button.dataset.action === "show-attention") {
    state.activeTab = "runtimes";
    state.activeRuntimeMainTab = "detail";
    state.selectedRuntimeId = runtimeId;
    applyTabShell();
    renderRuntimeSliceFromCache();
    syncLocation();
    nodes.runtimeDetailPanel.scrollIntoView({ behavior: "smooth", block: "start" });
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
  await refreshRuntimeById(runtimeId, kind);
}

function bootstrapDashboard() {
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

  nodes.runtimePanelTabs.forEach((button) => {
    button.addEventListener("click", () => {
      state.runtimePanelView = button.dataset.runtimePanelView;
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

  nodes.languageSelect.addEventListener("change", () => {
    state.languagePreference = nodes.languageSelect.value;
    state.language = resolveLanguage(state.languagePreference);
    setStoredLanguagePreference(state.languagePreference);
    applyTranslations();
    renderDashboardFromCache();
    syncLocation();
  });

  nodes.themeSelect.addEventListener("change", () => {
    state.themePreference = nodes.themeSelect.value;
    state.theme = resolveTheme(state.themePreference);
    setStoredThemePreference(state.themePreference);
    applyTheme();
    syncLocation();
  });

  nodes.applyFiltersButton.addEventListener("click", () => {
    state.filter.environment = nodes.environmentInput.value.trim();
    state.filter.cluster = nodes.clusterInput.value.trim();
    state.filter.role = nodes.roleInput.value.trim();
    loadDashboard();
  });

  nodes.clearFiltersButton.addEventListener("click", () => {
    state.filter.environment = "";
    state.filter.cluster = "";
    state.filter.role = "";
    state.runtimeSearch = "";
    state.selectedRuntimeId = null;
    loadDashboard();
  });

  nodes.runtimeSearch.addEventListener("input", () => {
    state.runtimeSearch = nodes.runtimeSearch.value.trim();
    renderRuntimeSliceFromCache();
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
      await handleRuntimeTableAction(actionButton);
      return;
    }

    if (target.closest(".runtime-row-menu")) {
      event.stopPropagation();
      return;
    }

    const row = target.closest("tr[data-runtime-id]");
    if (!(row instanceof HTMLTableRowElement)) {
      return;
    }

    state.selectedRuntimeId = row.dataset.runtimeId;
    renderRuntimeSliceFromCache();
    syncLocation();
    void loadRuntimeAttention(state.selectedRuntimeId);
    void loadOrchestraPlan(state.selectedRuntimeId);
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

    await refreshSelectedRuntime(button.dataset.recoveryAction);
  });
  nodes.runtimeDeleteFailed?.addEventListener("click", deleteFailedRuntimes);
  nodes.runtimeDeleteUnobserved?.addEventListener("click", deleteUnobservedRuntimes);
  nodes.runtimeClearSlice?.addEventListener("click", clearRuntimeSlice);
  nodes.runtimeCleanupMenu?.addEventListener("toggle", syncCleanupMenuState);

  nodes.refreshAllButton.addEventListener("click", () => postAndReload("/v1/fleet/refresh-all", t("notifications.fleetRefreshAll")));
  nodes.refreshStatusButton.addEventListener("click", () => postAndReload("/v1/fleet/refresh-status", t("notifications.fleetStatusRefresh")));
  nodes.refreshCapabilitiesButton.addEventListener("click", () => postAndReload("/v1/fleet/refresh-capabilities", t("notifications.fleetCapabilityRefresh")));
  nodes.orchestraRefresh?.addEventListener("click", () => loadOrchestraPlan());
  nodes.orchestraPlans?.addEventListener("click", (event) => {
    const button = event.target.closest("[data-orchestra-execute]");
    if (button) {
      const card = button.closest(".orchestra-plan-card");
      void executeOrchestraPlan(
        button.dataset.orchestraExecute,
        button.dataset.orchestraRevision,
        button.dataset.orchestraApproval,
        card?.querySelector("[data-orchestra-approved-by]")?.value?.trim(),
        card?.querySelector("[data-orchestra-approval-note]")?.value?.trim(),
      );
      return;
    }
    const sessionButton = event.target.closest("[data-orchestra-create-session]");
    if (sessionButton) {
      void createOrchestraSession(sessionButton);
    }
  });
  nodes.orchestraHistory?.addEventListener("click", (event) => {
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
  nodes.runtimeDetailRefreshAll.addEventListener("click", () => refreshSelectedRuntime("all"));
  nodes.runtimeDetailRefreshStatus.addEventListener("click", () => refreshSelectedRuntime("status"));
  nodes.runtimeDetailRefreshCapabilities.addEventListener("click", () => refreshSelectedRuntime("capabilities"));
  nodes.runtimeDetailRefreshSidecar.addEventListener("click", () => refreshSelectedRuntime("sidecar"));
  nodes.runtimeDetailCopyLink.addEventListener("click", copySelectedRuntimeLink);
  nodes.registerName.addEventListener("input", () => {
    state.registerNameTouched = nodes.registerName.value.trim().length > 0;
    scheduleRenderRegisterPreview();
  });
  nodes.registerEndpoint.addEventListener("input", maybePrefillRuntimeNameFromEndpoint);
  nodes.registerSidecarEndpoint.addEventListener("input", scheduleRenderRegisterPreview);
  nodes.registerSidecarAdminToken.addEventListener("input", scheduleRenderRegisterPreview);
  nodes.registerRuntimeEnvironment.addEventListener("input", scheduleRenderRegisterPreview);
  nodes.registerRuntimeCluster.addEventListener("input", scheduleRenderRegisterPreview);
  nodes.registerRuntimeRole.addEventListener("input", scheduleRenderRegisterPreview);
  nodes.registerFetchCapabilities.addEventListener("change", scheduleRenderRegisterPreview);
  nodes.registerForm.addEventListener("submit", submitRegisterForm);
  nodes.registerFormClear.addEventListener("click", clearRegisterForm);

  document.addEventListener("click", (event) => {
    if (nodes.runtimeCleanupMenu?.open) {
      if (!(event.target instanceof Node) || !nodes.runtimeCleanupMenu.contains(event.target)) {
        nodes.runtimeCleanupMenu.open = false;
      }
    }
    if (!nodes.securityDetails?.open) {
      return;
    }
    if (!(event.target instanceof Node)) {
      return;
    }
    if (!nodes.securityDetails.contains(event.target)) {
      closeSecurityDetails();
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && nodes.securityDetails?.open) {
      closeSecurityDetails();
    }
  });

  nodes.securityDetails?.addEventListener("toggle", () => {
    syncSecurityDetailsState();
    if (nodes.securityDetails.open) {
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
    applyLayoutMode();
  });

  hydrateStateFromLocation();
  applyTheme();
  applyLayoutMode();
  applyTranslations();
  applyTabShell();
  clearRegisterForm();
  loadDashboard();
}
