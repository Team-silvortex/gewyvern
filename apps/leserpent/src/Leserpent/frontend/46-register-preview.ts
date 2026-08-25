// @ts-nocheck
// Split from app.ts to keep the control-plane shell maintainable.

function registrationPlanConflictMessage(plan) {
  if (!plan || plan.allowed) return "";
  if (plan.reason === "runtime_deletion_in_progress") {
    return t("register.deletionInProgress");
  }
  return t("register.blockedDuplicate", {
    reason: t("register.duplicateEndpoint"),
    name: plan.existingRuntimeName,
    endpoint: plan.existingRuntimeEndpoint,
  });
}

function registrationPlanDraft() {
  return {
    name: nodes.registerName.value.trim(),
    endpoint: nodes.registerEndpoint.value.trim(),
    sidecarEndpoint: nodes.registerSidecarEndpoint.value.trim() || null,
  };
}

function registrationPlanDraftKey(draft = registrationPlanDraft()) {
  return [draft.name, draft.endpoint, draft.sidecarEndpoint || ""].join("::");
}

function currentRegistrationPlan() {
  const plan = state.registrationPlan;
  return plan?.draftKey === registrationPlanDraftKey() ? plan : null;
}

function registrationReadiness(endpointValid, sidecarEndpointValid) {
  const name = nodes.registerName.value.trim();
  const endpoint = nodes.registerEndpoint.value.trim();
  const sidecarEndpoint = nodes.registerSidecarEndpoint.value.trim();
  const pairingToken = nodes.registerToken.value.trim();
  const plan = currentRegistrationPlan();

  if (!name) {
    return {
      plan,
      ready: false,
      field: nodes.registerName,
      tone: "pending",
      message: t("register.completeField", { field: t("register.name") }),
    };
  }
  if (!endpoint) {
    return {
      plan,
      ready: false,
      field: nodes.registerEndpoint,
      tone: "pending",
      message: t("register.completeField", { field: t("register.endpoint") }),
    };
  }
  if (!endpointValid) {
    return { plan, ready: false, field: nodes.registerEndpoint, tone: "bad", message: t("register.blockedEndpoint") };
  }
  if (!pairingToken) {
    return {
      plan,
      ready: false,
      field: nodes.registerToken,
      tone: "pending",
      message: t("register.completeField", { field: t("register.pairingToken") }),
    };
  }
  if (sidecarEndpoint && !sidecarEndpointValid) {
    return { plan, ready: false, field: nodes.registerSidecarEndpoint, tone: "bad", message: t("register.blockedSidecarEndpoint") };
  }
  if (state.registrationPlanError) {
    return {
      plan,
      ready: false,
      field: nodes.registerEndpoint,
      tone: "bad",
      message: t("register.planUnavailable", { message: state.registrationPlanError }),
    };
  }
  if (!plan) {
    return { plan, ready: false, field: null, tone: "pending", message: t("register.checkingPlan") };
  }
  if (!plan.allowed) {
    return {
      plan,
      ready: false,
      field: nodes.registerEndpoint,
      tone: "bad",
      message: registrationPlanConflictMessage(plan),
    };
  }
  return { plan, ready: true, field: null, tone: "good", message: t("register.ready") };
}

function setRegisterResult(message, tone = "neutral", focus = false) {
  nodes.registerResult.textContent = message;
  nodes.registerResult.dataset.tone = tone;
  if (focus) {
    window.requestAnimationFrame(() => nodes.registerResult.focus({ preventScroll: false }));
  }
}

function revealRegistrationField(field) {
  if (!field) return;
  if (nodes.registerSidecarDetails?.contains(field)) {
    nodes.registerSidecarDetails.open = true;
  }
  field.setAttribute("aria-invalid", "true");
  window.requestAnimationFrame(() => {
    field.focus({ preventScroll: true });
    field.scrollIntoView({ behavior: "smooth", block: "center" });
  });
}

function showRegistrationIssue(issue) {
  state.activeTab = "runtimes";
  state.activeRuntimeMainTab = "register";
  applyTabShell();
  setRegisterResult(issue.message, "bad");
  revealRegistrationField(issue.field);
}

function setRegistrationSecretVisibility(input, toggle, label, visible) {
  if (!input || !toggle || !label) return;
  input.type = visible ? "text" : "password";
  toggle.setAttribute("aria-pressed", String(visible));
  label.textContent = t(visible ? "register.hideToken" : "register.showToken");
}

function syncRegistrationSecretToggles() {
  setRegistrationSecretVisibility(
    nodes.registerToken,
    nodes.registerTokenToggle,
    nodes.registerTokenToggleLabel,
    nodes.registerToken.type === "text",
  );
  setRegistrationSecretVisibility(
    nodes.registerSidecarAdminToken,
    nodes.registerSidecarAdminTokenToggle,
    nodes.registerSidecarAdminTokenToggleLabel,
    nodes.registerSidecarAdminToken.type === "text",
  );
}

function maskRegistrationSecrets() {
  setRegistrationSecretVisibility(
    nodes.registerToken,
    nodes.registerTokenToggle,
    nodes.registerTokenToggleLabel,
    false,
  );
  setRegistrationSecretVisibility(
    nodes.registerSidecarAdminToken,
    nodes.registerSidecarAdminTokenToggle,
    nodes.registerSidecarAdminTokenToggleLabel,
    false,
  );
}

function clearRegistrationSecrets() {
  nodes.registerToken.value = "";
  nodes.registerSidecarAdminToken.value = "";
  maskRegistrationSecrets();
}

async function loadRegistrationPlan() {
  const draft = registrationPlanDraft();
  const draftKey = registrationPlanDraftKey(draft);
  if (!draft.name || !isLikelyHttpEndpoint(draft.endpoint) ||
      (draft.sidecarEndpoint && !isLikelyHttpEndpoint(draft.sidecarEndpoint))) {
    state.registrationPlan = null;
    renderRegisterPreview();
    return;
  }

  state.registrationPlanAbortController?.abort();
  const abortController = new AbortController();
  state.registrationPlanAbortController = abortController;
  try {
    const plan = await postJsonBody("/v1/runtimes/registration-plan", draft, abortController.signal);
    if (draftKey !== registrationPlanDraftKey()) return;
    state.registrationPlan = { ...plan, draftKey };
    state.registrationPlanError = "";
  } catch (error) {
    if (error?.name === "AbortError") return;
    if (draftKey !== registrationPlanDraftKey()) return;
    state.registrationPlan = null;
    state.registrationPlanError = error.message;
  } finally {
    if (state.registrationPlanAbortController === abortController) {
      state.registrationPlanAbortController = null;
    }
    renderRegisterPreview();
  }
}

function scheduleRegistrationPlan() {
  window.clearTimeout(state.registrationPlanTimer);
  state.registrationPlan = null;
  state.registrationPlanError = "";
  state.registrationPlanTimer = window.setTimeout(() => void loadRegistrationPlan(), 250);
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
    scheduleRegistrationPlanPreview();
    return;
  }

  const endpoint = nodes.registerEndpoint.value.trim();
  if (!isLikelyHttpEndpoint(endpoint)) {
    scheduleRegistrationPlanPreview();
    return;
  }

  const suggestion = suggestedRuntimeName(endpoint);
  if (suggestion) {
    nodes.registerName.value = suggestion;
  }
  scheduleRegistrationPlanPreview();
}

function registerPreviewSignature() {
  const plan = currentRegistrationPlan();
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
    plan?.planToken || state.registrationPlanError || "plan-pending",
  ].join("::");
}

function syncRegisterSubmitState(endpointValid, sidecarEndpointValid) {
  const endpoint = nodes.registerEndpoint.value.trim();
  const busy = state.uiActions.has("register-runtime");
  const readiness = registrationReadiness(endpointValid, sidecarEndpointValid);

  nodes.registerEndpoint.setAttribute("aria-invalid", endpoint && !endpointValid ? "true" : "false");
  nodes.registerSidecarEndpoint.setAttribute(
    "aria-invalid",
    nodes.registerSidecarEndpoint.value.trim() && !sidecarEndpointValid ? "true" : "false",
  );
  nodes.registerGuidance.textContent = readiness.message;
  nodes.registerGuidance.dataset.tone = busy ? "pending" : readiness.tone;
  nodes.registerSubmit.disabled = busy || !readiness.ready;
  nodes.registerForm.dataset.ready = readiness.ready ? "true" : "false";
  return { ...readiness, valid: readiness.ready };
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

function scheduleRegistrationPlanPreview() {
  scheduleRenderRegisterPreview();
  scheduleRegistrationPlan();
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
    ${submission.plan && !submission.plan.allowed ? `<div class="register-preview-warning">${escapeHtml(registrationPlanConflictMessage(submission.plan))}</div>` : ""}
    ${state.registrationPlanError ? `<div class="register-preview-warning">${escapeHtml(state.registrationPlanError)}</div>` : ""}
  `;
}
