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

function duplicateRuntimeMessage(duplicate, name, endpoint) {
  if (!duplicate) return "";
  const nameConflict = duplicate.name.toLowerCase() === name.toLowerCase();
  const endpointConflict = duplicate.endpoint.toLowerCase() === endpoint.toLowerCase();
  const reason = nameConflict && endpointConflict
    ? t("register.duplicateNameAndEndpoint")
    : nameConflict
      ? t("register.duplicateName")
      : t("register.duplicateEndpoint");
  return t("register.blockedDuplicate", {
    reason,
    name: duplicate.name,
    endpoint: duplicate.endpoint,
  });
}

function isLikelyHttpEndpoint(endpoint) {
  if (!(endpoint.startsWith("http://") || endpoint.startsWith("https://"))) {
    return false;
  }

  try {
    const parsed = new URL(endpoint);
    return (parsed.protocol === "http:" || parsed.protocol === "https:")
      && !!parsed.hostname
      && !parsed.username
      && !parsed.password;
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
  const duplicate = findDuplicateRuntime(
    nodes.registerName.value.trim(),
    nodes.registerEndpoint.value.trim(),
  );
  return [
    state.language,
    nodes.registerName.value.trim(),
    nodes.registerEndpoint.value.trim(),
    nodes.registerSidecarEndpoint.value.trim(),
    nodes.registerSidecarAdminToken.value.trim() ? "protected" : "open",
    nodes.registerToken.value.trim() ? "paired" : "missing-token",
    nodes.registerRuntimeEnvironment.value.trim(),
    nodes.registerRuntimeCluster.value.trim(),
    nodes.registerRuntimeRole.value.trim(),
    nodes.registerFetchCapabilities.checked ? "fetch" : "skip",
    duplicate?.runtimeId || "unique",
  ].join("::");
}

function syncRegisterSubmitState(endpointValid, sidecarEndpointValid) {
  const name = nodes.registerName.value.trim();
  const endpoint = nodes.registerEndpoint.value.trim();
  const pairingToken = nodes.registerToken.value.trim();
  const duplicate = name && endpointValid ? findDuplicateRuntime(name, endpoint) : null;
  const busy = state.uiActions.has("register-runtime");
  const valid = !!name && endpointValid && sidecarEndpointValid && !!pairingToken && !duplicate;

  nodes.registerEndpoint.setAttribute("aria-invalid", endpoint && !endpointValid ? "true" : "false");
  nodes.registerSidecarEndpoint.setAttribute(
    "aria-invalid",
    nodes.registerSidecarEndpoint.value.trim() && !sidecarEndpointValid ? "true" : "false",
  );
  nodes.registerSubmit.disabled = busy || !valid;
  nodes.registerForm.dataset.ready = valid ? "true" : "false";
  return { duplicate, valid };
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
  const endpoint = nodes.registerEndpoint.value.trim();
  const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
  const endpointValid = endpoint.length > 0 && isLikelyHttpEndpoint(endpoint);
  const sidecarEndpointValid = sidecarEndpoint.length > 0 ? isLikelyHttpEndpoint(sidecarEndpoint) : true;
  const submission = syncRegisterSubmitState(endpointValid, sidecarEndpointValid);
  const signature = registerPreviewSignature();
  if (state.renderSignatures.registerPreview === signature) {
    return;
  }
  state.renderSignatures.registerPreview = signature;

  const sidecarAdminToken = nodes.registerSidecarAdminToken.value.trim();
  const pairingTokenReady = !!nodes.registerToken.value.trim();
  const explicitName = nodes.registerName.value.trim();
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
        <strong class="register-preview-state ${endpointValid ? "good" : endpoint ? "bad" : "pending"}">${escapeHtml(endpointState)}</strong>
        ${endpoint ? `<div class="register-preview-meta">${escapeHtml(endpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecar"))}</span>
        <strong class="register-preview-state ${sidecarEndpoint ? sidecarEndpointValid ? "good" : "bad" : "pending"}">${escapeHtml(sidecarState)}</strong>
        ${sidecarEndpoint ? `<div class="register-preview-meta">${escapeHtml(sidecarEndpoint)}</div>` : ""}
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewSidecarAccess"))}</span>
        <strong>${escapeHtml(sidecarAccess)}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewPairing"))}</span>
        <strong class="register-preview-state ${pairingTokenReady ? "good" : "bad"}">${escapeHtml(t(pairingTokenReady ? "register.pairingReady" : "register.pairingMissing"))}</strong>
      </div>
      <div class="register-preview-row">
        <span>${escapeHtml(t("register.previewCapabilityFetch"))}</span>
        <strong>${escapeHtml(nodes.registerFetchCapabilities.checked ? t("register.capabilityEnabled") : t("register.capabilityDisabled"))}</strong>
      </div>
    </div>
    ${submission.duplicate ? `<div class="register-preview-warning">${escapeHtml(duplicateRuntimeMessage(submission.duplicate, explicitName, endpoint))}</div>` : ""}
  `;
}
