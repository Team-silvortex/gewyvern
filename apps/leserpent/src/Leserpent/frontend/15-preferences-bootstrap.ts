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
  } else if (state.activeRuntimeMainTab === "detail" && state.selectedRuntimeId) {
    void loadRuntimeAttention(state.selectedRuntimeId);
  } else if (state.activeRuntimeMainTab === "panel" && state.selectedRuntimeId) {
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
      if (event.key === "ArrowRight") nextIndex = index + direction;
      if (event.key === "ArrowLeft") nextIndex = index - direction;
      if (event.key === "Home") nextIndex = 0;
      if (event.key === "End") nextIndex = buttons.length - 1;
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
      const selected = nodes.runtimeTableBody.querySelector(
        `tr[data-runtime-id="${CSS.escape(state.selectedRuntimeId)}"]`,
      );
      if (selected instanceof HTMLTableRowElement) selected.focus();
    });
  }
}

function closeOpenRuntimeRowMenu(restoreFocus = false, except = null) {
  let focusTarget = null;
  let closed = false;
  for (const menu of nodes.runtimeTableBody.querySelectorAll(".runtime-row-menu[open]")) {
    if (!(menu instanceof HTMLDetailsElement) || menu === except) continue;
    if (!focusTarget) focusTarget = menu.querySelector("summary");
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
      if (!state.languagePackCatalog.length) void loadLanguagePackCatalog();
    }
  });
  nodes.languagePackRefresh?.addEventListener("click", loadLanguagePackCatalog);
  nodes.languagePackImport?.addEventListener("click", () => nodes.languagePackFile.click());
  nodes.languagePackFile?.addEventListener("change", (event) => importLanguagePackFile(event.target.files?.[0]));
  nodes.languagePackDetails?.addEventListener("click", (event) => {
    const button = event.target.closest("[data-language-pack-action][data-locale]");
    if (button) void handleLanguagePackAction(button);
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
      if (target.closest("summary")) closeOpenRuntimeRowMenu(false, rowMenu);
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
    if (!(target instanceof HTMLElement)) return;
    const row = target.closest("tr[data-runtime-id]");
    if (!(row instanceof HTMLTableRowElement) || target !== row) return;

    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      selectRuntimeTableRow(row, true);
      return;
    }

    const rows = runtimeTableRows();
    const index = rows.indexOf(row);
    let nextIndex = null;
    if (event.key === "ArrowDown") nextIndex = Math.min(index + 1, rows.length - 1);
    if (event.key === "ArrowUp") nextIndex = Math.max(index - 1, 0);
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = rows.length - 1;
    if (nextIndex === null || nextIndex === index || !rows[nextIndex]) return;
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
    if (!(target instanceof Element)) return;
    const button = target.closest("button[data-runtime-detail-target]");
    if (!(button instanceof HTMLButtonElement)) return;
    activateRuntimeDetailTab(button.dataset.runtimeDetailTarget);
    const activeTab = nodes.runtimeDetailSubtabButtons.find(
      (candidate) => candidate.dataset.runtimeDetailTab === state.activeRuntimeDetailTab,
    );
    if (activeTab instanceof HTMLButtonElement) activeTab.focus();
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
    setRegistrationSecretVisibility(
      nodes.registerToken,
      nodes.registerTokenToggle,
      nodes.registerTokenToggleLabel,
      nodes.registerToken.type === "password",
    );
    nodes.registerToken.focus();
  });
  nodes.registerSidecarAdminTokenToggle.addEventListener("click", () => {
    setRegistrationSecretVisibility(
      nodes.registerSidecarAdminToken,
      nodes.registerSidecarAdminTokenToggle,
      nodes.registerSidecarAdminTokenToggleLabel,
      nodes.registerSidecarAdminToken.type === "password",
    );
    nodes.registerSidecarAdminToken.focus();
  });
  nodes.registerSidecarDetails.addEventListener("toggle", () => {
    if (!nodes.registerSidecarDetails.open) {
      setRegistrationSecretVisibility(
        nodes.registerSidecarAdminToken,
        nodes.registerSidecarAdminTokenToggle,
        nodes.registerSidecarAdminTokenToggleLabel,
        false,
      );
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
      if (closeOpenRuntimeRowMenu(true)) event.preventDefault();
      if (state.mobileFiltersOpen) setMobileFiltersOpen(false, true);
      if (nodes.securityDetails?.open) closeSecurityDetails();
      if (nodes.languagePackDetails?.open) nodes.languagePackDetails.open = false;
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
      if (entry) syncRuntimeListLayout(entry.contentRect.width);
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
