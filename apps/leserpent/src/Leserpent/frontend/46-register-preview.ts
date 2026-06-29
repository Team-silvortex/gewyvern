// @ts-nocheck
// Split from app.ts to keep the control-plane shell maintainable.

function findDuplicateRuntime(name, endpoint) {
  const normalizedName = name.trim().toLowerCase();
  const normalizedEndpoint = endpoint.trim().toLowerCase();
  return state.latestRuntimes.find((runtime) =>
    runtime.name.toLowerCase() === normalizedName ||
    runtime.endpoint.toLowerCase() === normalizedEndpoint
  ) || null;
}

function isLikelyHttpEndpoint(endpoint) {
  if (!(endpoint.startsWith("http://") || endpoint.startsWith("https://"))) {
    return false;
  }

  try {
    const parsed = new URL(endpoint);
    return parsed.protocol === "http:" || parsed.protocol === "https:";
  } catch {
    return false;
  }
}

function suggestedRuntimeName(endpoint) {
  try {
    const parsed = new URL(endpoint);
    const hostBits = parsed.hostname
      .split(".")
      .filter(Boolean)
      .slice(0, 4)
      .map((bit) => bit.replace(/[^a-zA-Z0-9-]/g, "-"))
      .filter(Boolean);
    const portBit = parsed.port ? `-${parsed.port}` : "";
    const hostPart = hostBits.length ? hostBits.join("-").toLowerCase() : "runtime";
    return `gw-${hostPart}${portBit}`;
  } catch {
    return "";
  }
}

function maybePrefillRuntimeNameFromEndpoint() {
  if (state.registerNameTouched) {
    scheduleRenderRegisterPreview();
    return;
  }

  const endpoint = nodes.registerEndpoint.value.trim();
  if (!isLikelyHttpEndpoint(endpoint)) {
    scheduleRenderRegisterPreview();
    return;
  }

  const suggestion = suggestedRuntimeName(endpoint);
  if (suggestion) {
    nodes.registerName.value = suggestion;
  }
  scheduleRenderRegisterPreview();
}

function registerPreviewSignature() {
  return [
    state.language,
    nodes.registerName.value.trim(),
    nodes.registerEndpoint.value.trim(),
    nodes.registerSidecarEndpoint.value.trim(),
    nodes.registerSidecarAdminToken.value.trim() ? "protected" : "open",
    nodes.registerRuntimeEnvironment.value.trim(),
    nodes.registerRuntimeCluster.value.trim(),
    nodes.registerRuntimeRole.value.trim(),
    nodes.registerFetchCapabilities.checked ? "fetch" : "skip",
  ].join("::");
}

function scheduleRenderRegisterPreview() {
  if (state.pendingRegisterPreview) {
    return;
  }

  state.pendingRegisterPreview = window.requestAnimationFrame(() => {
    state.pendingRegisterPreview = 0;
    renderRegisterPreview();
  });
}

function renderRegisterPreview() {
  const signature = registerPreviewSignature();
  if (state.renderSignatures.registerPreview === signature) {
    return;
  }
  state.renderSignatures.registerPreview = signature;

  const endpoint = nodes.registerEndpoint.value.trim();
  const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
  const sidecarAdminToken = nodes.registerSidecarAdminToken.value.trim();
  const explicitName = nodes.registerName.value.trim();
  const endpointValid = endpoint.length > 0 && isLikelyHttpEndpoint(endpoint);
  const sidecarEndpointValid = sidecarEndpoint.length > 0 ? isLikelyHttpEndpoint(sidecarEndpoint) : true;
  const suggestedName = endpointValid ? suggestedRuntimeName(endpoint) : "";
  const effectiveName = explicitName || suggestedName || t("register.pendingRuntimeName");
  const endpointState = endpoint.length === 0
    ? t("register.endpointPending")
    : endpointValid ? t("register.endpointValid") : t("register.endpointInvalid");
  const sidecarState = sidecarEndpoint.length === 0
    ? t("register.sidecarUnpaired")
    : sidecarEndpointValid ? t("register.endpointValid") : t("register.endpointInvalid");
  const sidecarAccess = sidecarEndpoint.length === 0
    ? t("register.sidecarUnpaired")
    : sidecarAdminToken ? t("runtimeDetail.sidecarProtected") : t("runtimeDetail.sidecarOpen");
  const slice = [
    nodes.registerRuntimeEnvironment.value.trim(),
    nodes.registerRuntimeCluster.value.trim(),
    nodes.registerRuntimeRole.value.trim(),
  ].filter(Boolean).join(" / ") || t("register.allRuntimes");

  nodes.registerPreview.innerHTML = `
    <div class="register-preview-head">
      <strong>${escapeHtml(t("register.previewTitle"))}</strong>
      ${!explicitName && suggestedName ? `<span class="tag-pill">${escapeHtml(t("register.suggested"))}</span>` : ""}
    </div>
    <div class="register-preview-grid">
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewName"))}</span>
        <strong>${escapeHtml(effectiveName)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSlice"))}</span>
        <strong>${escapeHtml(slice)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewEndpoint"))}</span>
        <strong>${escapeHtml(endpointState)}</strong>
        ${endpoint ? `<div class="register-preview-meta">${escapeHtml(endpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecar"))}</span>
        <strong>${escapeHtml(sidecarState)}</strong>
        ${sidecarEndpoint ? `<div class="register-preview-meta">${escapeHtml(sidecarEndpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecarAccess"))}</span>
        <strong>${escapeHtml(sidecarAccess)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewCapabilityFetch"))}</span>
        <strong>${escapeHtml(nodes.registerFetchCapabilities.checked ? t("register.capabilityEnabled") : t("register.capabilityDisabled"))}</strong>
      </div>
    </div>
  `;
}
