// @ts-nocheck
// Translation catalog base and merge helper split by locale to keep files small.

const translations = {};

function mergeTranslations(base, patch) {
  const result = Array.isArray(base) ? [...base] : { ...base };
  for (const [key, value] of Object.entries(patch || {})) {
    if (
      value
      && typeof value === "object"
      && !Array.isArray(value)
      && base?.[key]
      && typeof base[key] === "object"
      && !Array.isArray(base[key])
    ) {
      result[key] = mergeTranslations(base[key], value);
    } else {
      result[key] = value;
    }
  }
  return result;
}
