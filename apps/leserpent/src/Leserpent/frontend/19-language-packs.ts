// @ts-nocheck
// Same-origin language-pack catalog, validation, installation, and export.

const languagePackSchema = "leserpent.language-pack/v1";
const languagePackCatalogUrl = "/language-packs/catalog.json";
const builtinLanguageLocales = new Set(["en", "zh-CN", "zh-TW", "ja", "es", "de", "fr", "ko"]);
const languagePackLimits = {
  bytes: 256 * 1024,
  catalogBytes: 128 * 1024,
  catalogPacks: 64,
  installedBytes: 512 * 1024,
  packs: 12,
  depth: 12,
  nodes: 2000,
  stringLength: 4000,
};

function languagePackError(message) {
  throw new Error(message);
}

function validLanguagePackText(value, field, maxLength = 120) {
  if (typeof value !== "string" || !value.trim() || value.length > maxLength || /[\u0000-\u001f\u007f]/.test(value)) {
    languagePackError(`${field} is invalid`);
  }
  return value.trim();
}

function validateLanguagePackTranslations(value, depth = 0, budget = { nodes: 0 }) {
  if (!value || typeof value !== "object" || Array.isArray(value) || depth > languagePackLimits.depth) {
    languagePackError("translations must be a bounded object tree");
  }
  const result = {};
  for (const [key, item] of Object.entries(value)) {
    budget.nodes += 1;
    if (budget.nodes > languagePackLimits.nodes || !/^[A-Za-z0-9_-]+$/.test(key) || ["__proto__", "prototype", "constructor"].includes(key)) {
      languagePackError("translations contains an invalid key or too many entries");
    }
    if (typeof item === "string") {
      if (item.length > languagePackLimits.stringLength || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(item)) {
        languagePackError(`translation '${key}' is invalid`);
      }
      result[key] = item;
    } else {
      result[key] = validateLanguagePackTranslations(item, depth + 1, budget);
    }
  }
  return result;
}

function validateLanguagePack(pack, { allowBuiltin = false } = {}) {
  if (!pack || typeof pack !== "object" || Array.isArray(pack) || pack.schema !== languagePackSchema) {
    languagePackError(`schema must be '${languagePackSchema}'`);
  }
  const locale = validLanguagePackText(pack.locale, "locale", 35);
  if (!/^[A-Za-z]{2,3}(?:-[A-Za-z0-9]{2,8})*$/.test(locale)) {
    languagePackError("locale must be a BCP 47-style language tag");
  }
  if (!allowBuiltin && builtinLanguageLocales.has(locale)) {
    languagePackError("built-in locales cannot be replaced by downloadable packs");
  }
  return {
    schema: languagePackSchema,
    locale,
    name: validLanguagePackText(pack.name, "name"),
    nativeName: validLanguagePackText(pack.nativeName, "nativeName"),
    version: validLanguagePackText(pack.version, "version", 40),
    author: pack.author ? validLanguagePackText(pack.author, "author") : "Leserpent community",
    direction: pack.direction === "rtl" ? "rtl" : "ltr",
    coverage: pack.coverage === "core-ui" ? "core-ui" : "partial",
    translations: validateLanguagePackTranslations(pack.translations),
  };
}

function serializeLanguagePack(pack) {
  return `${JSON.stringify(pack, null, 2)}\n`;
}

async function sha256Hex(text) {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function registerInstalledLanguagePack(pack) {
  translations[pack.locale] = mergeTranslations(translations.en, pack.translations);
}

function persistLanguagePacks() {
  const serialized = JSON.stringify(state.installedLanguagePacks);
  if (new TextEncoder().encode(serialized).byteLength > languagePackLimits.installedBytes) {
    languagePackError("installed language packs exceed the browser storage limit");
  }
  window.localStorage.setItem(storageKeys.languagePacks, serialized);
}

function safeCatalogPackUrl(value) {
  try {
    const url = new URL(value, window.location.origin);
    return url.origin === window.location.origin && url.pathname.startsWith("/language-packs/") && url.pathname !== "/language-packs/catalog.json"
      ? `${url.pathname}${url.search}`
      : null;
  } catch {
    return null;
  }
}

function restoreLanguagePacks() {
  state.installedLanguagePacks = {};
  try {
    const stored = JSON.parse(window.localStorage.getItem(storageKeys.languagePacks) || "{}");
    if (!stored || typeof stored !== "object" || Array.isArray(stored)) return;
    for (const value of Object.values(stored).slice(0, languagePackLimits.packs)) {
      try {
        const pack = validateLanguagePack(value);
        state.installedLanguagePacks[pack.locale] = pack;
        registerInstalledLanguagePack(pack);
      } catch {
        // Ignore a malformed browser-local entry without blocking dashboard startup.
      }
    }
  } catch {
    state.installedLanguagePacks = {};
  }
}

function syncLanguageOptions() {
  const selected = state.languagePreference;
  for (const option of Array.from(nodes.languageSelect.options)) {
    if (option.dataset.languagePack === "true") option.remove();
  }
  for (const pack of Object.values(state.installedLanguagePacks).sort((a, b) => a.nativeName.localeCompare(b.nativeName))) {
    const option = document.createElement("option");
    option.value = pack.locale;
    option.textContent = pack.nativeName;
    option.dataset.languagePack = "true";
    nodes.languageSelect.appendChild(option);
  }
  nodes.languageSelect.value = selected;
}

function setLanguagePackStatus(message, tone = "") {
  nodes.languagePackStatus.textContent = message;
  nodes.languagePackStatus.dataset.tone = tone;
}

async function boundedResponseText(response, maxBytes, label) {
  const declaredLength = Number(response.headers.get("content-length"));
  if (Number.isFinite(declaredLength) && declaredLength > maxBytes) {
    languagePackError(`${label} exceeds ${maxBytes} bytes`);
  }
  if (!response.body) languagePackError(`${label} response has no readable body`);

  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let bytes = 0;
  let text = "";
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      bytes += value.byteLength;
      if (bytes > maxBytes) {
        await reader.cancel("response exceeds configured limit");
        languagePackError(`${label} exceeds ${maxBytes} bytes`);
      }
      text += decoder.decode(value, { stream: true });
    }
    return text + decoder.decode();
  } catch (error) {
    if (error instanceof TypeError) languagePackError(`${label} is not valid UTF-8`);
    throw error;
  } finally {
    reader.releaseLock();
  }
}

async function fetchLanguagePackText(url) {
  const response = await fetch(url, { credentials: "same-origin", cache: "no-cache" });
  if (!response.ok) languagePackError(`${url} -> ${response.status}`);
  return boundedResponseText(response, languagePackLimits.bytes, "language pack");
}

async function loadLanguagePackCatalog() {
  setLanguagePackStatus(t("languagePacks.loading"));
  try {
    const response = await fetch(languagePackCatalogUrl, { credentials: "same-origin", cache: "no-cache" });
    if (!response.ok) languagePackError(`${languagePackCatalogUrl} -> ${response.status}`);
    const catalog = JSON.parse(await boundedResponseText(
      response,
      languagePackLimits.catalogBytes,
      "language-pack catalog",
    ));
    if (catalog?.schema !== "leserpent.language-pack-catalog/v1"
      || !Array.isArray(catalog.packs)
      || catalog.packs.length > languagePackLimits.catalogPacks) {
      languagePackError("language-pack catalog schema is invalid");
    }
    state.languagePackCatalog = catalog.packs.flatMap((entry) => {
      const safeUrl = safeCatalogPackUrl(entry?.url);
      return entry
        && typeof entry.locale === "string"
        && typeof entry.version === "string"
        && typeof entry.nativeName === "string"
        && (entry.direction === "ltr" || entry.direction === "rtl")
        && entry.coverage === "core-ui"
        && safeUrl
        && /^[a-f0-9]{64}$/.test(entry.sha256)
        ? [{ ...entry, url: safeUrl }]
        : [];
    });
    state.languagePackCatalogMeta = {
      official: Number(catalog.officialLocaleCount) || builtinLanguageLocales.size + state.languagePackCatalog.length,
      builtin: Number(catalog.builtinLocaleCount) || builtinLanguageLocales.size,
    };
    renderLanguagePackCenter();
    setLanguagePackStatus(t("languagePacks.catalogReady", {
      official: state.languagePackCatalogMeta.official,
      builtin: state.languagePackCatalogMeta.builtin,
      count: state.languagePackCatalog.length,
    }), "good");
  } catch (error) {
    console.error(error);
    state.languagePackCatalog = [];
    renderLanguagePackCenter();
    setLanguagePackStatus(t("languagePacks.catalogFailed", { message: error.message }), "bad");
  }
}

async function verifiedCatalogPack(entry) {
  const text = await fetchLanguagePackText(entry.url);
  const digest = await sha256Hex(text);
  if (digest !== entry.sha256) languagePackError("language-pack SHA-256 verification failed");
  const pack = validateLanguagePack(JSON.parse(text));
  if (pack.locale !== entry.locale || pack.version !== entry.version) {
    languagePackError("language-pack metadata does not match its catalog entry");
  }
  return pack;
}

async function installLanguagePack(pack) {
  const validated = validateLanguagePack(pack);
  const previous = state.installedLanguagePacks;
  const next = { ...state.installedLanguagePacks, [validated.locale]: validated };
  if (Object.keys(next).length > languagePackLimits.packs) languagePackError("at most 12 language packs can be installed");
  try {
    state.installedLanguagePacks = next;
    persistLanguagePacks();
    registerInstalledLanguagePack(validated);
  } catch (error) {
    state.installedLanguagePacks = previous;
    if (previous[validated.locale]) {
      registerInstalledLanguagePack(previous[validated.locale]);
    } else {
      delete translations[validated.locale];
    }
    throw error;
  }
  syncLanguageOptions();
  renderLanguagePackCenter();
  setLanguagePackStatus(t("languagePacks.installed", { name: validated.nativeName }), "good");
}

function downloadLanguagePack(pack) {
  const blob = new Blob([serializeLanguagePack(pack)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `leserpent-language-${pack.locale}-${pack.version}.json`;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

async function handleLanguagePackAction(button) {
  const locale = button.dataset.locale;
  const action = button.dataset.languagePackAction;
  const key = `language-pack:${locale}`;
  if (state.uiActions.has(key)) return;
  if (action === "remove") {
    const pack = state.installedLanguagePacks[locale];
    if (!pack || !window.confirm(t("languagePacks.removeConfirm", { name: pack.nativeName }))) return;
  }
  await runUiActionOnce(key, button, `${button.textContent}...`, async () => {
    try {
      if (action === "remove") {
        delete state.installedLanguagePacks[locale];
        delete translations[locale];
        persistLanguagePacks();
        if (state.languagePreference === locale) {
          state.languagePreference = "auto";
          state.language = resolveLanguage("auto");
          setStoredLanguagePreference("auto");
        }
        syncLanguageOptions();
        applyTranslations();
        renderDashboardFromCache();
        renderLanguagePackCenter();
        setLanguagePackStatus(t("languagePacks.removed"), "good");
        return;
      }
      if (action === "export") {
        downloadLanguagePack(state.installedLanguagePacks[locale]);
        return;
      }
      const entry = state.languagePackCatalog.find((item) => item.locale === locale);
      if (!entry) languagePackError("catalog entry not found");
      const pack = await verifiedCatalogPack(entry);
      if (action === "download") {
        downloadLanguagePack(pack);
        setLanguagePackStatus(t("languagePacks.downloaded", { name: pack.nativeName }), "good");
      } else {
        await installLanguagePack(pack);
      }
    } catch (error) {
      console.error(error);
      setLanguagePackStatus(t("languagePacks.operationFailed", { message: error.message }), "bad");
    }
  });
}

async function importLanguagePackFile(file) {
  if (!file) return;
  await runUiActionOnce("language-pack-import", nodes.languagePackImport, `${t("languagePacks.import")}...`, async () => {
    try {
      if (file.size > languagePackLimits.bytes) languagePackError("language pack exceeds 256 KiB");
      await installLanguagePack(JSON.parse(await file.text()));
    } catch (error) {
      console.error(error);
      setLanguagePackStatus(t("languagePacks.operationFailed", { message: error.message }), "bad");
    } finally {
      nodes.languagePackFile.value = "";
    }
  });
}

function renderLanguagePackCenter() {
  const installed = Object.values(state.installedLanguagePacks);
  nodes.languagePackInstalled.innerHTML = installed.length
    ? installed.map((pack) => `<div class="language-pack-row"><div><strong>${escapeHtml(pack.nativeName)}</strong><span>${escapeHtml(pack.locale)} · ${escapeHtml(pack.version)}</span></div><div><button type="button" data-language-pack-action="export" data-locale="${escapeHtml(pack.locale)}">${escapeHtml(t("languagePacks.export"))}</button><button type="button" class="quiet" data-language-pack-action="remove" data-locale="${escapeHtml(pack.locale)}">${escapeHtml(t("languagePacks.remove"))}</button></div></div>`).join("")
    : `<div class="hint-line">${escapeHtml(t("languagePacks.noneInstalled"))}</div>`;
  nodes.languagePackCatalog.innerHTML = state.languagePackCatalog.length
    ? state.languagePackCatalog.map((entry) => {
      const present = !!state.installedLanguagePacks[entry.locale];
      return `<div class="language-pack-row"><div><strong>${escapeHtml(entry.nativeName)}</strong><span>${escapeHtml(entry.locale)} · ${escapeHtml(entry.version)} · ${escapeHtml(t("languagePacks.coverageCore"))}</span></div><div><button type="button" data-language-pack-action="install" data-locale="${escapeHtml(entry.locale)}" ${present ? "disabled" : ""}>${escapeHtml(present ? t("languagePacks.installedLabel") : t("languagePacks.install"))}</button><button type="button" class="quiet" data-language-pack-action="download" data-locale="${escapeHtml(entry.locale)}">${escapeHtml(t("languagePacks.download"))}</button></div></div>`;
    }).join("")
    : `<div class="hint-line">${escapeHtml(t("languagePacks.catalogEmpty"))}</div>`;
}
