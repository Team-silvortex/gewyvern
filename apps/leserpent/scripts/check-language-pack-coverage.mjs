import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const frontendRoot = join(scriptDir, "..", "src", "Leserpent", "frontend");
const languagePackRoot = join(scriptDir, "..", "src", "Leserpent", "wwwroot", "language-packs");

const localeFiles = [
  "10-i18n-en.ts",
  "11-i18n-zh-cn.ts",
  "12-i18n-zh-tw.ts",
  "13-i18n-de.ts",
  "14-i18n-fr.ts",
  "16-i18n-ko.ts",
  "17-i18n-ja.ts",
  "18-i18n-es.ts",
];

const builtinLocales = ["en", "zh-CN", "zh-TW", "ja", "es", "de", "fr", "ko"];

const coreUiPackKeys = [
  "hero.title",
  "hero.subcopy",
  "language.label",
  "languagePacks.title",
  "languagePacks.install",
  "languagePacks.installedLabel",
  "languagePacks.download",
  "languagePacks.remove",
  "theme.label",
  "tabs.overview",
  "tabs.runtimes",
  "tabs.register",
  "tabs.persistence",
  "tabs.sessions",
  "languagePacks.coverageCore",
  "runtimes.workspaceTabs.panel",
  "runtimePanel.windows.openAll",
  "runtimePanel.windows.closeAll",
];

const expandedOfficialPackKeys = [
  "language.auto",
  "languagePacks.subcopy",
  "languagePacks.refresh",
  "languagePacks.import",
  "languagePacks.installedTitle",
  "languagePacks.catalogTitle",
  "languagePacks.catalogEmpty",
  "languagePacks.noneInstalled",
  "languagePacks.export",
  "theme.auto",
  "theme.light",
  "theme.dark",
];

const fleetOfficialPackKeys = [
  "actions.refreshAll",
  "actions.refreshStatus",
  "actions.refreshCapabilities",
  "filters.title",
  "filters.apply",
  "filters.clear",
  "runtimes.title",
  "runtimes.quickSearch",
  "runtimes.sortBy",
  "runtimePanel.windows.openSelected",
  "runtimePanel.windows.close",
  "runtimePanel.windows.activate",
];

const officialPackKeys = [
  ...coreUiPackKeys,
  ...expandedOfficialPackKeys,
  ...fleetOfficialPackKeys,
];

function parseLocaleFile(path) {
  const text = readFileSync(path, "utf8");
  const match = /translations(?:\.([A-Za-z0-9_-]+)|\[[\'"]([^\'"]+)[\'"]\])\s*=\s*(?:mergeTranslations\([^,]+,\s*)?\{/.exec(
    text
  );
  if (!match) {
    throw new Error(`Cannot find locale assignment in ${path}`);
  }
  const locale = match[1] || match[2];
  const braceStart = text.indexOf("{", match.index);
  if (braceStart < 0) {
    throw new Error(`Cannot find locale object in ${path}`);
  }
  const isMerge = /mergeTranslations\s*\(/.test(match[0]);

  let depth = 0;
  let inSingle = false;
  let inDouble = false;
  let inTemplate = false;
  let inBlock = false;
  let inLine = false;
  let escaped = false;

  for (let i = braceStart; i < text.length; i++) {
    const ch = text[i];
    const prev = text[i - 1];

    if (inBlock) {
      if (prev !== "*" && ch === "/") inBlock = false;
      continue;
    }
    if (inLine) {
      if (ch === "\n") inLine = false;
      continue;
    }
    if (inSingle) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === "'") inSingle = false;
      continue;
    }
    if (inDouble) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === '"') inDouble = false;
      continue;
    }
    if (inTemplate) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === "`") inTemplate = false;
      continue;
    }

    if (ch === "/" && text[i + 1] === "/") {
      inLine = true;
      i++;
      continue;
    }
    if (ch === "/" && text[i + 1] === "*") {
      inBlock = true;
      i++;
      continue;
    }
    if (ch === '"') {
      inDouble = true;
      continue;
    }
    if (ch === "'") {
      inSingle = true;
      continue;
    }
    if (ch === "`") {
      inTemplate = true;
      continue;
    }

    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) {
        const literal = text.slice(braceStart, i + 1);
        const patch = vm.runInNewContext(`(${literal})`);
        return { locale, patch, merged: isMerge };
      }
    }
  }

  throw new Error(`Could not parse locale object in ${path}`);
}

function mergeTranslations(base, patch) {
  const merged = { ...(base && typeof base === "object" ? base : {}) };
  if (!patch || typeof patch !== "object" || Array.isArray(patch)) {
    return patch;
  }
  for (const [key, value] of Object.entries(patch)) {
    if (
      value &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      merged[key] &&
      typeof merged[key] === "object" &&
      !Array.isArray(merged[key])
    ) {
      merged[key] = mergeTranslations(merged[key], value);
    } else {
      merged[key] = value;
    }
  }
  return merged;
}

function flattenKeys(value, prefix = "") {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  const out = [];
  for (const [key, child] of Object.entries(value)) {
    const next = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === "object" && !Array.isArray(child)) {
      out.push(...flattenKeys(child, next));
    } else {
      out.push(next);
    }
  }
  return out;
}

function hasKey(obj, dotted) {
  let current = obj;
  for (const segment of dotted.split(".")) {
    if (!current || typeof current !== "object" || !Object.hasOwn(current, segment)) {
      return false;
    }
    current = current[segment];
  }
  return true;
}

const parsed = {};
for (const file of localeFiles) {
  const data = parseLocaleFile(join(frontendRoot, file));
  parsed[data.locale] = data;
}

const builtin = {};
for (const [locale, item] of Object.entries(parsed)) {
  builtin[locale] = item.merged ? mergeTranslations(builtin.en, item.patch) : item.patch;
}

const enKeys = new Set(flattenKeys(builtin.en));
let failures = 0;

console.log("locale alignment:");
for (const locale of builtinLocales) {
  const value = builtin[locale];
  if (!value) {
    console.log(`- ${locale}: missing locale module`);
    failures++;
    continue;
  }
  const missing = [...enKeys].filter((k) => !hasKey(value, k));
  const extra = [...flattenKeys(value)].filter((k) => !enKeys.has(k));
  if (missing.length === 0 && extra.length === 0) {
    console.log(`- ${locale}: aligned (${enKeys.size} keys)`);
  } else {
    console.log(`- ${locale}: missing=${missing.length}, extra=${extra.length}`);
    if (missing.length) failures++;
  }
}

const catalog = JSON.parse(readFileSync(join(languagePackRoot, "catalog.json"), "utf8"));
console.log("downloadable pack official coverage:");
for (const entry of catalog.packs) {
  const packPath = join(languagePackRoot, `${entry.locale}.json`);
  const pack = JSON.parse(readFileSync(packPath, "utf8"));
  const translations = pack.translations || {};
  const publishedKeys = flattenKeys(translations);
  const missingPackKeys = officialPackKeys.filter((k) => !hasKey(translations, k));
  const unexpectedPackKeys = publishedKeys.filter((k) => !officialPackKeys.includes(k));
  const total = officialPackKeys.length;
  const covered = total - missingPackKeys.length;
  console.log(`- ${entry.locale}: ${covered}/${total}`);
  if (entry.version !== "1.2.0" || pack.version !== entry.version) {
    failures++;
    console.log(`  version: catalog=${entry.version}, pack=${pack.version}`);
  }
  if (missingPackKeys.length || unexpectedPackKeys.length || publishedKeys.length !== total) {
    failures++;
    if (missingPackKeys.length) console.log(`  missing: ${missingPackKeys.join(", ")}`);
    if (unexpectedPackKeys.length) console.log(`  unexpected: ${unexpectedPackKeys.join(", ")}`);
  }
}

if (failures > 0) {
  console.log(`\nlocale coverage check failed with ${failures} issue(s).`);
  process.exit(1);
}
console.log("\nlocale coverage check passed.");
