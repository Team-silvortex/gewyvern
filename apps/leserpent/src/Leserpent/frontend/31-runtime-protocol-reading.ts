// @ts-nocheck
// Compact protocol reading strip for runtime child panels.

function protocolReadingStore() {
  state.cache.protocolReadingByRuntimeId ||= {};
  state.cache.protocolReadingPendingByRuntimeId ||= {};
  return state.cache.protocolReadingByRuntimeId;
}

function protocolReadingPendingStore() {
  protocolReadingStore();
  return state.cache.protocolReadingPendingByRuntimeId;
}

function runtimeProtocolReadingContainer() {
  return document.getElementById("runtime-panel-reading");
}

function protocolReadingAbsoluteUrl(runtime, path) {
  if (!runtime?.endpoint || !path) {
    return "";
  }
  return `${runtime.endpoint.replace(/\/+$/, "")}${path}`;
}

function protocolReadingChipLabel(protocol, entry) {
  return `${protocol} / ${entry}`;
}

function protocolReadingOverlayLabel(viaOverlay) {
  return viaOverlay ? t("runtimePanel.reading.via", { overlay: viaOverlay }) : "";
}

function clearRuntimeProtocolReading() {
  const container = runtimeProtocolReadingContainer();
  if (!container) {
    return;
  }
  container.classList.add("hidden");
  container.innerHTML = "";
}

function renderRuntimeProtocolReading(runtime, reading) {
  const container = runtimeProtocolReadingContainer();
  if (!container) {
    return;
  }

  if (!runtime || !reading) {
    clearRuntimeProtocolReading();
    return;
  }

  const currentUrl = protocolReadingAbsoluteUrl(runtime, reading.currentSurfacePath);
  const currentOverlay = protocolReadingOverlayLabel(reading.selectedOverlay);
  const companionLinks = (reading.readingCompanions || []).map((companion) => {
    const surfaceUrl = protocolReadingAbsoluteUrl(runtime, companion.surfacePath);
    const overlayText = protocolReadingOverlayLabel(companion.viaOverlay);
    return `
      <a class="protocol-reading-link" href="${escapeHtml(surfaceUrl)}" target="_blank" rel="noreferrer">
        <span>${escapeHtml(protocolReadingChipLabel(companion.protocol, companion.entry))}</span>
        ${overlayText ? `<small>${escapeHtml(overlayText)}</small>` : ""}
      </a>
    `;
  }).join("");

  container.classList.remove("hidden");
  container.innerHTML = `
    <span class="protocol-reading-kicker">${escapeHtml(t("runtimePanel.reading.title"))}</span>
    <div class="protocol-reading-links">
      <a class="protocol-reading-link is-current" href="${escapeHtml(currentUrl)}" target="_blank" rel="noreferrer">
        <span>${escapeHtml(protocolReadingChipLabel(reading.protocol, reading.entry))}</span>
        ${currentOverlay ? `<small>${escapeHtml(currentOverlay)}</small>` : ""}
      </a>
      ${(reading.readingCompanions || []).length ? `<span class="protocol-reading-sep">${escapeHtml(t("runtimePanel.reading.next"))}</span>` : ""}
      ${companionLinks}
    </div>
    <span class="protocol-reading-meta">${escapeHtml(t("runtimePanel.reading.target", { target: reading.targetName }))}</span>
  `;
}

async function loadRuntimeProtocolReading(runtimeId) {
  if (!runtimeId) {
    return null;
  }

  const cache = protocolReadingStore();
  const pending = protocolReadingPendingStore();
  if (pending[runtimeId]) {
    return pending[runtimeId];
  }

  pending[runtimeId] = (async () => {
    try {
      const reading = await getJson(`/v1/runtimes/${runtimeId}/protocol-reading`);
      cache[runtimeId] = reading;
      const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
      if (selectedRuntime && state.selectedRuntimeId === runtimeId) {
        renderRuntimeProtocolReading(selectedRuntime, reading);
      }
      return reading;
    } catch (error) {
      cache[runtimeId] = null;
      const selectedRuntime = state.latestRuntimes.find((runtime) => runtime.runtimeId === runtimeId) || null;
      if (selectedRuntime && state.selectedRuntimeId === runtimeId) {
        clearRuntimeProtocolReading();
      }
      return null;
    } finally {
      delete pending[runtimeId];
    }
  })();

  return pending[runtimeId];
}

function ensureRuntimeProtocolReading(runtime) {
  if (!runtime?.runtimeId) {
    clearRuntimeProtocolReading();
    return;
  }

  const cache = protocolReadingStore();
  if (cache[runtime.runtimeId]) {
    renderRuntimeProtocolReading(runtime, cache[runtime.runtimeId]);
    return;
  }

  clearRuntimeProtocolReading();
  void loadRuntimeProtocolReading(runtime.runtimeId);
}
