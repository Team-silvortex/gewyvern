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
            <button type="button" data-orchestra-execute="${escapeHtml(plan.planId)}">Run plan</button>
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
          <span class="severity ${escapeHtml(run.outcome === "ok" ? "" : "warning")}">${escapeHtml(run.outcome)}</span>
        </div>
        <div class="item-meta">${escapeHtml(run.runId)} · ${escapeHtml(orchestraTimestamp(run.executedAt))}</div>
        <div class="orchestra-run-steps">
          ${(run.steps || []).map((step) => `
            <div class="orchestra-run-step" data-outcome="${escapeHtml(step.outcome)}">
              <span>${escapeHtml(step.step)}</span>
              <strong>${escapeHtml(step.outcome)}</strong>
              <span class="hint-line">${escapeHtml(step.summary)}</span>
            </div>
          `).join("")}
        </div>
      </article>
    `).join("")
    : `<div class="hint-line">Executed automatic plans will appear here.</div>`;
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
    }
  } catch (error) {
    console.error(error);
    if (runtimeId === state.selectedRuntimeId) {
      nodes.orchestraHistorySummary.textContent = "Run history unavailable.";
    }
  }
}

async function executeOrchestraPlan(planId) {
  const runtimeId = state.selectedRuntimeId;
  if (!runtimeId || !planId) {
    return;
  }

  const button = nodes.orchestraPlans.querySelector(`[data-orchestra-execute="${CSS.escape(planId)}"]`);
  if (button) {
    button.disabled = true;
    button.textContent = "Running...";
  }
  nodes.statusLine.textContent = `Running orchestra plan ${planId}...`;

  try {
    const result = await postJson(`/v1/orchestra/plans/${encodeURIComponent(runtimeId)}/${encodeURIComponent(planId)}/execute`);
    if (runtimeId !== state.selectedRuntimeId) {
      return;
    }
    renderOrchestraPlan(result.currentPlan);
    await loadDashboard();
    nodes.statusLine.textContent = `Orchestra plan ${planId} completed: ${result.outcome}.`;
  } catch (error) {
    console.error(error);
    nodes.statusLine.textContent = `Orchestra plan ${planId} failed: ${error.message}`;
    if (button) {
      button.disabled = false;
      button.textContent = "Run plan";
    }
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
