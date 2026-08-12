// @ts-nocheck
// Security, token, and transport helpers split from app.ts during the TypeScript migration.

function getStoredAdminToken() {
  try {
    window.localStorage.removeItem(storageKeys.adminToken);
    return window.sessionStorage.getItem(storageKeys.adminToken) || "";
  } catch {
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
  } catch {
    // ignore storage failures
  }
}

function getStoredAdminTokenTestState() {
  try {
    window.localStorage.removeItem(storageKeys.adminTokenTestState);
    return window.sessionStorage.getItem(storageKeys.adminTokenTestState) || "never";
  } catch {
    return "never";
  }
}

function getStoredAdminTokenTestAt() {
  try {
    window.localStorage.removeItem(storageKeys.adminTokenTestAt);
    return window.sessionStorage.getItem(storageKeys.adminTokenTestAt) || null;
  } catch {
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
    } else {
      window.sessionStorage.removeItem(storageKeys.adminTokenTestAt);
    }
  } catch {
    // ignore storage failures
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
    } else if (state.adminTokenTestState === "ok") {
      nodes.adminTokenState.textContent = t("security.tokenTestOk");
      securityVisualState = "ok";
    } else if (state.adminTokenTestState === "failed") {
      nodes.adminTokenState.textContent = state.adminToken?.trim()
        ? t("security.tokenStored")
        : t("security.tokenRequired");
      securityVisualState = state.adminToken?.trim() ? "stored" : "required";
    } else if (state.adminToken?.trim()) {
      nodes.adminTokenState.textContent = t("security.tokenStored");
      securityVisualState = "stored";
    } else {
      nodes.adminTokenState.textContent = t("security.localModeHint");
      securityVisualState = "local";
    }

    if (capabilities?.security?.apiMode === "loopback_or_token" && !state.adminToken?.trim()) {
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
  if (installedMatch) return installedMatch;
  if (normalized.startsWith("zh")) {
    if (
      normalized.includes("hant")
      || normalized.includes("tw")
      || normalized.includes("hk")
      || normalized.includes("mo")
    ) {
      return "zh-TW";
    }
    return "zh-CN";
  }
  if (normalized.startsWith("ja")) return "ja";
  if (normalized.startsWith("es")) return "es";
  if (normalized.startsWith("de")) return "de";
  if (normalized.startsWith("fr")) return "fr";
  if (normalized.startsWith("ko")) return "ko";
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
  if (!(width > 0)) return;
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
  if (state.languagePreference && state.languagePreference !== "auto") params.set("lang", state.languagePreference);
  if (state.themePreference && state.themePreference !== "auto") params.set("theme", state.themePreference);
  if (state.activeTab && state.activeTab !== "overview") params.set("tab", state.activeTab);
  if (state.activeOverviewTab && state.activeOverviewTab !== "summary") params.set("overview", state.activeOverviewTab);
  if (state.activeRuntimeMainTab && state.activeRuntimeMainTab !== "select") params.set("runtimePane", state.activeRuntimeMainTab);
  if (state.activeRuntimeDetailTab && state.activeRuntimeDetailTab !== "identity") params.set("runtimeDetail", state.activeRuntimeDetailTab);
  if (state.runtimePanelView && state.runtimePanelView !== "root") params.set("runtimeView", state.runtimePanelView);
  if (state.filter.environment) params.set("environment", state.filter.environment);
  if (state.filter.cluster) params.set("cluster", state.filter.cluster);
  if (state.filter.role) params.set("role", state.filter.role);
  if (state.runtimeSearch) params.set("search", state.runtimeSearch);
  if (state.runtimeSort && state.runtimeSort !== "name") params.set("sort", state.runtimeSort);
  if (state.selectedRuntimeId) params.set("runtimeId", state.selectedRuntimeId);
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
  if (state.activeRuntimeMainTab === "panel" && state.selectedRuntimeId) {
    if (!state.runtimeWindowIds.includes(state.selectedRuntimeId)) {
      state.runtimeWindowIds.push(state.selectedRuntimeId);
    }
    state.activeRuntimeWindowId = state.selectedRuntimeId;
    state.runtimeWindowViews[state.selectedRuntimeId] = state.runtimePanelView;
    persistRuntimeWindows();
  }
  state.filter.environment = params.get("environment") || "";
  state.filter.cluster = params.get("cluster") || "";
  state.filter.role = params.get("role") || "";
  state.runtimeSearch = params.get("search") || "";
  state.runtimeSort = params.get("sort") || "name";
  state.selectedRuntimeId = params.get("runtimeId") || null;
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
      } else if (option.value === "light") {
        option.textContent = t("theme.light");
      } else if (option.value === "dark") {
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
    } else {
      button.removeAttribute("aria-current");
    }
  }
  for (const panel of nodes.tabPanels) {
    const isActive = panel.dataset.tabPanel === state.activeTab;
    panel.classList.toggle("active", isActive);
    panel.hidden = !isActive;
  }
  syncTabSet(
    nodes.runtimeMainTabButtons,
    nodes.runtimeMainPanels,
    state.activeRuntimeMainTab,
    "runtimeMainTab",
    "runtimeMainPanel",
    "runtime-main",
  );
  syncTabSet(
    nodes.overviewSubtabButtons,
    nodes.overviewSubpanels,
    state.activeOverviewTab,
    "overviewTab",
    "overviewPanel",
    "overview",
  );
  if (nodes.runtimeWorkspace) {
    nodes.runtimeWorkspace.classList.toggle("register-focus", state.activeRuntimeMainTab === "register");
    nodes.runtimeWorkspace.classList.toggle("panel-focus", state.activeRuntimeMainTab === "panel");
    nodes.runtimeWorkspace.classList.toggle("detail-focus", state.activeRuntimeMainTab === "detail");
    nodes.runtimeWorkspace.dataset.mainTab = state.activeRuntimeMainTab;
  }
  syncTabSet(
    nodes.runtimeDetailSubtabButtons,
    nodes.runtimeDetailSections,
    state.activeRuntimeDetailTab,
    "runtimeDetailTab",
    "runtimeDetailPanel",
    "runtime-detail",
  );
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
  } catch (error) {
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
