// @ts-nocheck
// Orchestra plan loading and rendering.

function orchestraReasonLabel(reason) {
  if (!reason) {
    return "clear";
  }
  const translated = t(`attention.${reason}`);
  return translated === `attention.${reason}` ? reason : translated;
}

function orchestraTagLabel(tags) {
  const parts = [tags?.environment, tags?.cluster, tags?.role].filter(Boolean);
  return parts.length ? parts.join(" / ") : "unscoped runtime";
}

function orchestraTimestamp(value) {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? `${value || "unknown"}` : parsed.toLocaleString();
}

function orchestraRequestId(key) {
  if (!state.orchestraRequestIds[key]) {
    const random = globalThis.crypto?.randomUUID?.()
      || `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    state.orchestraRequestIds[key] = `ui:${random}`;
  }
  return state.orchestraRequestIds[key];
}

function renderOrchestraPlan(payload) {
  state.orchestraPlan = payload;

  if (!payload) {
    state.renderSignatures.orchestraPanel = "empty";
    nodes.orchestraSummary.textContent = "Select a runtime to build an operator-facing orchestration plan.";
    nodes.orchestraPlans.innerHTML = `
      <div class="group-card">
        <div class="group-title">No runtime selected</div>
        <div class="hint-line">Use the runtimes workspace to choose a runtime first, then return here for an orchestration plan.</div>
      </div>
    `;
    return;
  }

  const signature = JSON.stringify(payload);
  if (state.renderSignatures.orchestraPanel === signature) {
    return;
  }
  state.renderSignatures.orchestraPanel = signature;

  nodes.orchestraSummary.textContent =
    `${payload.name} · ${payload.endpoint} · ${payload.statusSource} · ${payload.attentionSeverity}`;

  nodes.orchestraPlans.innerHTML = payload.plans.map((plan) => `
    <article class="group-card orchestra-plan-card">
      <div class="group-title">${escapeHtml(plan.title)}</div>
      <div class="hint-line">${escapeHtml(plan.summary)}</div>
      <div class="runtime-tags orchestra-plan-meta">
        <span class="tag-pill">intent: ${escapeHtml(plan.intent)}</span>
        <span class="tag-pill">risk: ${escapeHtml(plan.riskLevel)}</span>
        <span class="tag-pill">readiness: ${escapeHtml(plan.executionReadiness)}</span>
        <span class="tag-pill">mode: ${escapeHtml(plan.executionMode)}</span>
        <span class="tag-pill">approval: ${escapeHtml(plan.approvalMode)}</span>
        <span class="tag-pill">revision: ${escapeHtml(plan.revision)}</span>
        <span class="tag-pill">scope: ${escapeHtml(orchestraTagLabel(payload.tags))}</span>
      </div>
      ${plan.reasons?.length ? `
        <div class="hint-line"><strong>Attention reasons</strong>: ${(plan.reasons || []).map((reason) => `<span class="reason-pill">${escapeHtml(orchestraReasonLabel(reason))}</span>`).join(" ")}</div>
      ` : ""}
      ${plan.requiredCapabilities?.length ? `
        <div class="hint-line"><strong>Required capabilities</strong>: ${(plan.requiredCapabilities || []).map((capability) => `<span class="tag-pill">${escapeHtml(capability)}</span>`).join(" ")}</div>
      ` : ""}
      <ol class="orchestra-step-list">
        ${(plan.steps || []).map((step) => `
          <li class="orchestra-step">
            <strong>${escapeHtml(step.title)}</strong>
            <div class="item-meta">${escapeHtml(step.kind)}</div>
            <div class="hint-line">${escapeHtml(step.detail)}</div>
          </li>
        `).join("")}
      </ol>
      ${plan.executionMode === "automatic" && plan.approvalMode === "operator_confirmation" ? `
        <div class="orchestra-approval-form" data-orchestra-approval-form>
          <label>
            <span>Approved by <small>operator-provided attribution</small></span>
            <input type="text" data-orchestra-approved-by value="leserpent-operator" maxlength="80" autocomplete="off" />
          </label>
          <label>
            <span>Approval note</span>
            <textarea data-orchestra-approval-note maxlength="500" rows="2" placeholder="Why is this execution appropriate now?"></textarea>
          </label>
        </div>
      ` : ""}
      ${plan.executionMode === "guided" && plan.planId === "session_preparation" ? `
        <div class="orchestra-guided-form" data-orchestra-session-form>
          <label>
            <span>Pipeline kind</span>
            <input type="text" data-orchestra-pipeline-kind value="diagnostic" maxlength="128" autocomplete="off" />
          </label>
          <label>
            <span>Requested by</span>
            <input type="text" data-orchestra-requested-by value="leserpent-operator" maxlength="80" autocomplete="off" />
          </label>
          <button type="button" data-orchestra-create-session>Create session</button>
        </div>
      ` : ""}
      ${(plan.suggestedSurfaces || []).length ? `
        <div class="runtime-inline-actions orchestra-surface-links">
          ${plan.executionMode === "automatic" ? `
            <button type="button"
              data-orchestra-execute="${escapeHtml(plan.planId)}"
              data-orchestra-revision="${escapeHtml(plan.revision)}"
              data-orchestra-approval="${escapeHtml(plan.approvalMode)}">${plan.approvalMode === "operator_confirmation" ? "Review & run" : "Run plan"}</button>
          ` : ""}
          ${(plan.suggestedSurfaces || []).map((surface) => `
            <a class="quiet" href="${escapeHtml(surface.path)}">${escapeHtml(surface.label)}</a>
          `).join("")}
        </div>
      ` : ""}
    </article>
  `).join("");
}

function renderOrchestraHistory(runs) {
  const normalized = Array.isArray(runs) ? runs : [];
  const signature = JSON.stringify(normalized);
  if (state.renderSignatures.orchestraHistory === signature) {
    return;
  }
  state.renderSignatures.orchestraHistory = signature;
  nodes.orchestraHistorySummary.textContent = normalized.length
    ? `${normalized.length} retained run${normalized.length === 1 ? "" : "s"}`
    : "No runs recorded.";
  nodes.orchestraHistory.innerHTML = normalized.length
    ? normalized.map((run) => `
      <article class="orchestra-run" data-outcome="${escapeHtml(run.outcome)}">
        <div class="item-head">
          <strong>${escapeHtml(run.planId)}</strong>
          <span class="severity ${escapeHtml(["succeeded", "ok"].includes(run.outcome) ? "" : "warning")}">${escapeHtml(run.outcome)}</span>
        </div>
        <div class="item-meta">${escapeHtml(run.runId)} · attempt ${escapeHtml(run.attempt || 1)} · ${escapeHtml(orchestraTimestamp(run.executedAt))}</div>
        <div class="item-meta">actor: ${escapeHtml(run.approvedBy || "unattributed")} · revision: ${escapeHtml(run.planRevision || "legacy")}</div>
        <div class="item-meta">request: ${escapeHtml(run.requestId || "legacy")}</div>
        ${run.approvalNote ? `<div class="orchestra-approval-note">${escapeHtml(run.approvalNote)}</div>` : ""}
        <div class="orchestra-run-steps">
          ${(run.steps || []).map((step) => `
            <div class="orchestra-run-step" data-outcome="${escapeHtml(step.outcome)}">
              <span>${escapeHtml(step.step)}</span>
              <strong>${escapeHtml(step.outcome)}</strong>
              <span class="hint-line">${escapeHtml(step.summary)}</span>
            </div>
          `).join("")}
        </div>
        <div class="runtime-inline-actions orchestra-run-actions">
          <button type="button" class="quiet" data-orchestra-load-events="${escapeHtml(run.runId)}">Timeline</button>
          ${["queued", "running"].includes(run.outcome) ? `
            <button type="button" class="quiet" data-orchestra-cancel-run="${escapeHtml(run.runId)}">Cancel</button>
          ` : ""}
          ${!["queued", "running"].includes(run.outcome) && run.planId !== "session_preparation" ? `
            <button type="button" class="quiet" data-orchestra-retry-run="${escapeHtml(run.runId)}">Retry</button>
          ` : ""}
        </div>
        <div class="orchestra-run-events" data-orchestra-run-events="${escapeHtml(run.runId)}" hidden></div>
      </article>
    `).join("")
    : `<div class="hint-line">Executed automatic plans will appear here.</div>`;
}

async function loadOrchestraRunEvents(runId, button) {
  const runtimeId = state.selectedRuntimeId;
  const container = button?.closest(".orchestra-run")?.querySelector("[data-orchestra-run-events]");
  if (!runtimeId || !runId || !container) {
    return;
  }
  if (container.dataset.loaded === "true") {
    container.hidden = !container.hidden;
    button.textContent = container.hidden ? "Timeline" : "Hide timeline";
    return;
  }

  button.disabled = true;
  button.textContent = "Loading...";
  try {
    const payload = await getJson(
      `/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs/${encodeURIComponent(runId)}/events`,
    );
    const events = Array.isArray(payload.events) ? payload.events : [];
    container.innerHTML = events.length
      ? `<ol class="orchestra-event-list">${events.map((item) => `
          <li class="orchestra-event">
            <span class="orchestra-event-marker" aria-hidden="true"></span>
            <div>
              <strong>${escapeHtml(item.fromOutcome ? `${item.fromOutcome} → ${item.toOutcome}` : item.toOutcome)}</strong>
              <span class="item-meta">${escapeHtml(item.eventType)} · ${escapeHtml(orchestraTimestamp(item.recordedAt))}</span>
              <div class="hint-line">${escapeHtml(item.summary)}</div>
            </div>
          </li>
        `).join("")}</ol>`
      : `<div class="hint-line">No append-only events were recorded for this legacy run.</div>`;
    container.dataset.loaded = "true";
    container.hidden = false;
    button.textContent = "Hide timeline";
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = `Run timeline failed: ${error.message}`;
    button.textContent = "Retry timeline";
  } finally {
    button.disabled = false;
  }
}

function renderOrchestraFleetBoard(payload) {
  const signature = JSON.stringify(payload);
  if (state.renderSignatures.orchestraFleetBoard === signature) {
    return;
  }
  state.renderSignatures.orchestraFleetBoard = signature;
  nodes.orchestraFleetCount.textContent = `${payload.runCount} runs`;
  const metrics = [
    ["Active", payload.activeCount],
    ["Failed", payload.failedCount],
    ["Degraded", payload.degradedCount],
    ["Retryable", payload.retryableCount],
    ["Runtimes", payload.runtimeCount],
  ];
  nodes.orchestraFleetMetrics.innerHTML = metrics.map(([label, value]) => `
    <div class="metric">
      <div class="metric-label">${escapeHtml(label)}</div>
      <div class="metric-value">${escapeHtml(value)}</div>
    </div>
  `).join("");
  const recent = (payload.runs || []).slice(0, 20);
  nodes.orchestraFleetRuns.innerHTML = recent.length
    ? recent.map((item) => `
      <button type="button" class="orchestra-fleet-run" data-outcome="${escapeHtml(item.run.outcome)}" data-orchestra-runtime-id="${escapeHtml(item.runtimeId)}">
        <span class="orchestra-fleet-runtime">
          <strong>${escapeHtml(item.runtimeName)}</strong>
          <small>${escapeHtml(orchestraTagLabel(item.tags))}</small>
        </span>
        <span>${escapeHtml(item.run.planId)}</span>
        <span class="severity">${escapeHtml(item.run.outcome)}</span>
        <span class="item-meta">${escapeHtml(orchestraTimestamp(item.run.executedAt))}</span>
      </button>
    `).join("")
    : `<div class="hint-line">No Orchestra runs have been recorded yet.</div>`;
}

function scheduleOrchestraFleetPoll(payload) {
  if (state.orchestraFleetPollTimer) {
    window.clearTimeout(state.orchestraFleetPollTimer);
    state.orchestraFleetPollTimer = 0;
  }
  if (!payload.activeCount || state.activeTab !== "orchestra" || document.hidden) {
    return;
  }
  state.orchestraFleetPollTimer = window.setTimeout(() => {
    state.orchestraFleetPollTimer = 0;
    if (state.activeTab === "orchestra" && !document.hidden) {
      void loadOrchestraFleetBoard();
    }
  }, 1000);
}

async function loadOrchestraFleetBoard() {
  try {
    const payload = await getJson("/v1/orchestra/runs");
    renderOrchestraFleetBoard(payload);
    scheduleOrchestraFleetPoll(payload);
  } catch (error) {
    console.error(error);
    nodes.orchestraFleetCount.textContent = "Fleet board unavailable";
  }
}

function scheduleOrchestraHistoryPoll(runtimeId, runs) {
  if (state.orchestraPollTimer) {
    window.clearTimeout(state.orchestraPollTimer);
    state.orchestraPollTimer = 0;
  }
  if (state.activeTab !== "orchestra"
      || document.hidden
      || !runs.some((run) => ["queued", "running"].includes(run.outcome))) {
    return;
  }
  state.orchestraPollTimer = window.setTimeout(() => {
    state.orchestraPollTimer = 0;
    if (state.activeTab === "orchestra" && !document.hidden && runtimeId === state.selectedRuntimeId) {
      void loadOrchestraHistory(runtimeId);
    }
  }, 1000);
}

function clearOrchestraPollTimers() {
  if (state.orchestraPollTimer) {
    window.clearTimeout(state.orchestraPollTimer);
    state.orchestraPollTimer = 0;
  }
  if (state.orchestraFleetPollTimer) {
    window.clearTimeout(state.orchestraFleetPollTimer);
    state.orchestraFleetPollTimer = 0;
  }
}

async function loadOrchestraHistory(runtimeId = state.selectedRuntimeId) {
  if (!runtimeId) {
    renderOrchestraHistory([]);
    return;
  }

  try {
    const payload = await getJson(`/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs`);
    if (runtimeId === state.selectedRuntimeId) {
      renderOrchestraHistory(payload.runs);
      scheduleOrchestraHistoryPoll(runtimeId, payload.runs || []);
    }
  } catch (error) {
    console.error(error);
    if (runtimeId === state.selectedRuntimeId) {
      nodes.orchestraHistorySummary.textContent = "Run history unavailable.";
    }
  }
}

async function executeOrchestraPlan(planId, revision, approvalMode, approvedBy, approvalNote) {
  const runtimeId = state.selectedRuntimeId;
  if (!runtimeId || !planId) {
    return;
  }
  if (approvalMode === "operator_confirmation" && (!approvedBy || !approvalNote)) {
    nodes.statusLine.textContent = "Approved by and approval note are required for this plan.";
    return;
  }

  const confirmed = approvalMode !== "operator_confirmation" || window.confirm(
    `Approve Orchestra plan ${planId}?\n\nRisk-aware execution requires operator confirmation.\nRevision: ${revision}`,
  );
  if (!confirmed) {
    return;
  }

  const button = nodes.orchestraPlans.querySelector(`[data-orchestra-execute="${CSS.escape(planId)}"]`);
  if (button) {
    button.disabled = true;
    button.textContent = "Running...";
  }
  nodes.statusLine.textContent = `Running orchestra plan ${planId}...`;
  const requestKey = `${runtimeId}:${planId}:execute`;
  const requestId = orchestraRequestId(requestKey);

  try {
    const result = await postJsonBody(`/v1/orchestra/plans/${encodeURIComponent(runtimeId)}/${encodeURIComponent(planId)}/execute`, {
      confirmed,
      expectedRevision: revision,
      approvedBy: approvalMode === "operator_confirmation" ? approvedBy : "automatic",
      approvalNote: approvalMode === "operator_confirmation" ? approvalNote : null,
      requestId,
    });
    if (runtimeId !== state.selectedRuntimeId) {
      return;
    }
    await loadOrchestraHistory(runtimeId);
    delete state.orchestraRequestIds[requestKey];
    void loadOrchestraFleetBoard();
    nodes.statusLine.textContent = `Orchestra plan ${planId} started as ${result.run.runId}.`;
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = `Orchestra plan ${planId} failed: ${error.message}`;
    if (button) {
      button.disabled = false;
      button.textContent = approvalMode === "operator_confirmation" ? "Review & run" : "Run plan";
    }
    void loadOrchestraPlan(runtimeId);
  }
}

async function mutateOrchestraRun(runId, action, button) {
  const runtimeId = state.selectedRuntimeId;
  if (!runtimeId || !runId) {
    return;
  }
  button.disabled = true;
  const originalLabel = button.textContent;
  button.textContent = action === "cancel" ? "Cancelling..." : "Retrying...";
  try {
    const previousRun = (await getJson(`/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs`)).runs
      .find((run) => run.runId === runId);
    const currentPlan = state.orchestraPlan?.plans?.find((plan) => plan.planId === previousRun?.planId);
    let approvedBy = "automatic";
    let approvalNote = null;
    if (action === "retry" && currentPlan?.approvalMode === "operator_confirmation") {
      approvedBy = window.prompt("Approved by (operator-provided attribution)", previousRun?.approvedBy || "leserpent-operator");
      if (approvedBy === null) {
        button.disabled = false;
        button.textContent = originalLabel;
        return;
      }
      approvalNote = window.prompt("Approval note for this retry", "");
      if (approvalNote === null) {
        button.disabled = false;
        button.textContent = originalLabel;
        return;
      }
    }
    const confirmed = action !== "retry"
      || currentPlan?.approvalMode !== "operator_confirmation"
      || window.confirm(`Approve retry for ${previousRun?.planId || runId}?\n\nRisk: ${currentPlan?.riskLevel || "unknown"}`);
    if (!confirmed) {
      button.disabled = false;
      button.textContent = originalLabel;
      return;
    }
    const path = `/v1/orchestra/runtimes/${encodeURIComponent(runtimeId)}/runs/${encodeURIComponent(runId)}/${action}`;
    const requestKey = `${runtimeId}:${runId}:${action}`;
    const requestId = orchestraRequestId(requestKey);
    const result = action === "retry"
      ? await postJsonBody(path, { confirmed, approvedBy, approvalNote, requestId })
      : await postJson(path);
    if (runtimeId === state.selectedRuntimeId) {
      await loadOrchestraHistory(runtimeId);
      delete state.orchestraRequestIds[requestKey];
      void loadOrchestraFleetBoard();
      nodes.statusLine.textContent = action === "cancel"
        ? `Cancellation requested for ${runId}.`
        : `Retry started as ${result.run.runId}.`;
    }
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = `Orchestra ${action} failed: ${error.message}`;
    button.disabled = false;
    button.textContent = originalLabel;
  }
}

async function createOrchestraSession(button) {
  const runtimeId = state.selectedRuntimeId;
  const form = button?.closest("[data-orchestra-session-form]");
  const pipelineKind = form?.querySelector("[data-orchestra-pipeline-kind]")?.value?.trim();
  const requestedBy = form?.querySelector("[data-orchestra-requested-by]")?.value?.trim();
  if (!runtimeId || !pipelineKind || !requestedBy) {
    nodes.statusLine.textContent = "Pipeline kind and requested by are required.";
    return;
  }

  button.disabled = true;
  button.textContent = "Creating...";
  nodes.statusLine.textContent = `Creating ${pipelineKind} session through Orchestra...`;
  try {
    const result = await postJsonBody(`/v1/orchestra/plans/${encodeURIComponent(runtimeId)}/session`, {
      pipelineKind,
      requestedBy,
    });
    if (runtimeId !== state.selectedRuntimeId) {
      return;
    }
    renderOrchestraPlan(result.currentPlan);
    await loadDashboard();
    void loadOrchestraFleetBoard();
    nodes.statusLine.textContent = `Session ${result.session.sessionId} created through Orchestra.`;
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = `Orchestra session handoff failed: ${error.message}`;
    button.disabled = false;
    button.textContent = "Create session";
  }
}

async function loadOrchestraPlan(runtimeId = state.selectedRuntimeId) {
  const requestSeq = ++state.orchestraRequestSeq;
  if (!runtimeId) {
    renderOrchestraPlan(null);
    renderOrchestraHistory([]);
    return;
  }

  try {
    const payload = await getJson(`/v1/orchestra/plans/${encodeURIComponent(runtimeId)}`);
    if (requestSeq !== state.orchestraRequestSeq || runtimeId !== state.selectedRuntimeId) {
      return;
    }
    renderOrchestraPlan(payload);
    void loadOrchestraHistory(runtimeId);
  } catch (error) {
    if (requestSeq !== state.orchestraRequestSeq) {
      return;
    }
    console.error(error);
    nodes.orchestraSummary.textContent = "Failed to build orchestra plan.";
    nodes.orchestraPlans.innerHTML = `
      <div class="group-card">
        <div class="group-title">Orchestra plan unavailable</div>
        <div class="hint-line">${escapeHtml(error.message || "unknown error")}</div>
      </div>
    `;
  }
}
