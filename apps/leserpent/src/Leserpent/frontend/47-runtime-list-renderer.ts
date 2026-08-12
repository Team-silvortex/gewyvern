// @ts-nocheck
// Split from app.ts to keep the control-plane shell maintainable.

function runtimeTableSignature(items, attentionMap) {
  return [
    state.language,
    state.runtimeSearch.trim().toLowerCase(),
    state.runtimeSort,
    ...items.map((runtime) => {
      const attention = attentionMap.get(runtime.runtimeId);
      const capabilityKeys = (runtime.capabilities || [])
        .map((item) => `${item.key}:${item.support}`)
        .sort()
        .join("|");
      const sidecarBits = [
        runtime.sidecarEndpoint || "",
        runtime.hasSidecarAdminToken ? "protected" : "open",
        runtime.sidecarStatus?.Healthy ? "healthy" : "",
        runtime.sidecarStatus?.HasEvidenceChainEnrichment ? "enrichment" : "",
        runtime.sidecarStatus?.HasDiagnosticOpinion ? "opinion" : "",
        runtime.status.hasExternalSidecarContext ? "context" : "",
        runtime.status.hasExternalDiagnosticOpinion ? "merged-opinion" : "",
      ].join("|");
      const attentionBits = attention
        ? `${attention.severity}:${attention.needsAttention}:${(attention.reasons || []).join("|")}`
        : "clear";
      return [
        runtime.runtimeId,
        runtime.name,
        runtime.endpoint,
        runtime.tags.environment || "",
        runtime.tags.cluster || "",
        runtime.tags.role || "",
        runtime.status.statusSource || "",
        runtime.status.resilienceStatus || "",
        runtime.status.socketServiceStatus || "",
        runtime.status.snapshotKind || "",
        capabilityKeys,
        sidecarBits,
        attentionBits,
      ].join("::");
    }),
  ].join("##");
}

function updateRuntimeTableSelection(selectedRuntimeId) {
  for (const row of nodes.runtimeTableBody.querySelectorAll("tr[data-runtime-id]")) {
    if (!(row instanceof HTMLTableRowElement)) continue;
    const isSelected = row.dataset.runtimeId === selectedRuntimeId;
    row.classList.toggle("selected", isSelected);
    row.setAttribute("aria-selected", String(isSelected));
    row.tabIndex = isSelected ? 0 : -1;
  }
}

function runtimeActionLabel(actionKey, runtimeName) {
  return `${t(actionKey)}: ${runtimeName}`;
}

function renderRuntimes(payload, attentionMap) {
  const allItems = payload.runtimes || [];
  state.latestRuntimes = allItems;
  if (state.activeRuntimeMainTab === "register") {
    return;
  }

  const listVisible = state.activeRuntimeMainTab === "select";
  const query = listVisible ? state.runtimeSearch.trim().toLowerCase() : "";
  const filteredItems = query
    ? allItems.filter((runtime) =>
      runtime.name.toLowerCase().includes(query) ||
      runtime.endpoint.toLowerCase().includes(query))
    : allItems;
  const items = listVisible
    ? [...filteredItems].sort((left, right) => {
      if (state.runtimeSort === "status") {
        return (left.status.statusSource || "").localeCompare(right.status.statusSource || "") ||
          left.name.localeCompare(right.name);
      }
      if (state.runtimeSort === "snapshot") {
        return (left.status.snapshotKind || "").localeCompare(right.status.snapshotKind || "") ||
          left.name.localeCompare(right.name);
      }
      return left.name.localeCompare(right.name);
    })
    : allItems;
  if (listVisible) {
    nodes.runtimeCount.textContent = `${items.length} ${t("metrics.runtimes")}`;
  }
  if (!items.length) {
    state.selectedRuntimeId = null;
    if (listVisible) {
      const emptySignature = `empty::${state.language}::${state.runtimeSearch.trim().toLowerCase()}::${state.runtimeSort}`;
      if (state.renderSignatures.runtimeTable !== emptySignature) {
        state.renderSignatures.runtimeTable = emptySignature;
        nodes.runtimeTableBody.innerHTML = `<tr class="runtime-empty-row"><td colspan="7">${escapeHtml(t("runtimes.noMatch"))}</td></tr>`;
      }
    }
    if (state.activeRuntimeMainTab === "detail") {
      renderRuntimeDetail(null, null);
    } else if (state.activeRuntimeMainTab === "panel") {
      renderRuntimePanel(null);
    }
    return;
  }

  if (!items.some((item) => item.runtimeId === state.selectedRuntimeId)) {
    state.selectedRuntimeId = items[0].runtimeId;
  }

  if (listVisible) {
    const tableSignature = runtimeTableSignature(items, attentionMap);
    if (state.renderSignatures.runtimeTable !== tableSignature) {
      state.renderSignatures.runtimeTable = tableSignature;
      nodes.runtimeTableBody.innerHTML = items.map((runtime) => {
      const isSelected = runtime.runtimeId === state.selectedRuntimeId;
      const badge = statusBadge(runtime.status);
      const attention = attentionMap.get(runtime.runtimeId);
      const capabilityKeys = (runtime.capabilities || [])
        .filter((item) => item.support === "fully_supported")
        .map((item) => item.key);
      const compactCapabilitySummary = capabilityKeys.length
        ? t("runtimes.states.capabilitiesCount", { count: capabilityKeys.length })
        : t("runtimes.states.noCapabilities");
      const sidecarBits = [
        runtime.sidecarEndpoint ? "paired" : null,
        runtime.hasSidecarAdminToken ? t("runtimes.states.protected") : null,
        runtime.sidecarStatus?.Healthy ? "healthy" : null,
        runtime.sidecarStatus?.HasEvidenceChainEnrichment ? "enrichment" : null,
        runtime.sidecarStatus?.HasDiagnosticOpinion ? "opinion" : null,
        runtime.status.hasExternalSidecarContext ? "context" : null,
        runtime.status.hasExternalDiagnosticOpinion ? "merged-opinion" : null,
      ].filter(Boolean);

      return `
        <tr class="${isSelected ? "selected" : ""}"
            data-runtime-id="${escapeHtml(runtime.runtimeId)}"
            aria-selected="${String(isSelected)}"
            tabindex="${isSelected ? 0 : -1}">
          <td data-runtime-cell="identity" data-label="${escapeHtml(t("runtimes.columns.name"))}">
            <strong>${escapeHtml(runtime.name)}</strong>
            <div class="item-meta">${escapeHtml(runtime.endpoint)}</div>
          </td>
          <td data-runtime-cell="tags" data-label="${escapeHtml(t("runtimes.columns.tags"))}">
            <div class="runtime-tags">
              <span class="tag-pill">${escapeHtml(runtime.tags.environment || t("runtimes.states.noEnv"))}</span>
              <span class="tag-pill">${escapeHtml(runtime.tags.cluster || t("runtimes.states.noCluster"))}</span>
              <span class="tag-pill">${escapeHtml(runtime.tags.role || t("runtimes.states.noRole"))}</span>
            </div>
          </td>
          <td data-runtime-cell="status" data-label="${escapeHtml(t("runtimes.columns.status"))}">
            <span class="runtime-state ${escapeHtml(badge.tone)}">${escapeHtml(badge.text)}</span>
            <div class="item-meta">${escapeHtml(t("runtimeDetail.source"))}: ${escapeHtml(runtime.status.statusSource)}</div>
            ${runtime.status.resilienceStatus ? `<div class="item-meta">${escapeHtml(t("runtimeDetail.resilienceStatus"))}: ${escapeHtml(runtime.status.resilienceStatus)}</div>` : ""}
          </td>
          <td data-runtime-cell="capabilities" data-label="${escapeHtml(t("runtimes.columns.capabilitySurface"))}">
            <div class="runtime-surface">
              <div class="runtime-surface-compact item-meta">${escapeHtml(compactCapabilitySummary)}</div>
              <div class="runtime-surface-pills">
                ${capabilityKeys.length ? capabilityKeys.map((key) => `<span class="tag-pill">${escapeHtml(key)}</span>`).join("") : `<span class="item-meta">${escapeHtml(t("runtimes.states.noCapabilities"))}</span>`}
              </div>
            </div>
          </td>
          <td data-runtime-cell="sidecar" data-label="${escapeHtml(t("runtimes.columns.sidecar"))}">
            <div class="runtime-sidecar">
              ${sidecarBits.length ? sidecarBits.map((bit) => `<span class="tag-pill">${escapeHtml(bit)}</span>`).join("") : `<span class="item-meta">${escapeHtml(t("runtimes.states.none"))}</span>`}
            </div>
          </td>
          <td data-runtime-cell="attention" data-label="${escapeHtml(t("runtimes.columns.attention"))}">
            <div class="runtime-attention">
              ${attention
                ? `<span class="runtime-state ${attention.severity === "critical" ? "bad" : "warn"}">${escapeHtml(t(`attention.${attention.severity}`))}</span>
                   ${(attention.reasons || []).map((reason) => `<span class="tag-pill">${escapeHtml(attentionReasonLabel(reason))}</span>`).join("")}`
                : `<span class="runtime-state good">${escapeHtml(t("runtimes.states.clear"))}</span>`}
            </div>
          </td>
          <td data-runtime-cell="actions" data-label="${escapeHtml(t("runtimes.columns.actions"))}">
            <div class="inline-actions">
              <button type="button" data-action="open-panel" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.openPanel", runtime.name))}">${escapeHtml(t("runtimes.actions.openPanel"))}</button>
              <button type="button" data-action="show-attention" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.attention", runtime.name))}">${escapeHtml(t("runtimes.actions.attention"))}</button>
              <button type="button" data-action="refresh-status" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.status", runtime.name))}">${escapeHtml(t("runtimes.actions.status"))}</button>
              ${runtime.sidecarEndpoint ? `<button type="button" data-action="refresh-sidecar" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimeDetail.refreshSidecar", runtime.name))}">${escapeHtml(t("runtimeDetail.refreshSidecar"))}</button>` : ""}
              <button type="button" data-action="refresh-all" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.all", runtime.name))}">${escapeHtml(t("runtimes.actions.all"))}</button>
              <button type="button" data-action="delete-runtime" data-runtime-id="${escapeHtml(runtime.runtimeId)}" data-runtime-name="${escapeHtml(runtime.name)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.delete", runtime.name))}">${escapeHtml(t("runtimes.actions.delete"))}</button>
            </div>
            <details class="runtime-row-menu">
              <summary class="quiet" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.menu", runtime.name))}">${escapeHtml(t("runtimes.actions.menu"))}</summary>
              <div class="runtime-row-menu-panel">
                <button type="button" data-action="open-panel" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.openPanel", runtime.name))}">${escapeHtml(t("runtimes.actions.openPanel"))}</button>
                <button type="button" data-action="show-attention" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.attention", runtime.name))}">${escapeHtml(t("runtimes.actions.attention"))}</button>
                <button type="button" data-action="refresh-status" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.status", runtime.name))}">${escapeHtml(t("runtimes.actions.status"))}</button>
                ${runtime.sidecarEndpoint ? `<button type="button" data-action="refresh-sidecar" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimeDetail.refreshSidecar", runtime.name))}">${escapeHtml(t("runtimeDetail.refreshSidecar"))}</button>` : ""}
                <button type="button" data-action="refresh-all" data-runtime-id="${escapeHtml(runtime.runtimeId)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.all", runtime.name))}">${escapeHtml(t("runtimes.actions.all"))}</button>
                <button type="button" data-action="delete-runtime" data-runtime-id="${escapeHtml(runtime.runtimeId)}" data-runtime-name="${escapeHtml(runtime.name)}" aria-label="${escapeHtml(runtimeActionLabel("runtimes.actions.delete", runtime.name))}">${escapeHtml(t("runtimes.actions.delete"))}</button>
              </div>
            </details>
          </td>
        </tr>
      `;
      }).join("");
    } else {
      updateRuntimeTableSelection(state.selectedRuntimeId);
    }
  }

  const selectedRuntime = items.find((runtime) => runtime.runtimeId === state.selectedRuntimeId) || null;
  const selectedAttention = selectedRuntime
    ? state.runtimeAttentionById.get(selectedRuntime.runtimeId) || attentionMap.get(selectedRuntime.runtimeId) || null
    : null;
  if (state.activeRuntimeMainTab === "detail") {
    renderRuntimeDetail(selectedRuntime, selectedAttention);
  } else if (state.activeRuntimeMainTab === "panel") {
    renderRuntimePanel(selectedRuntime);
  }
}
