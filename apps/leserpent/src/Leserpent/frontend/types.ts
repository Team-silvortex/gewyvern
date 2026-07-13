type Nullable<T> = T | null;

interface RuntimeTags {
  environment?: string | null;
  cluster?: string | null;
  role?: string | null;
}

interface RuntimeCapability {
  key: string;
  support: string;
  description: string;
}

interface RuntimeStatusSnapshot {
  statusSource: string;
  statusFetchedAt?: string | null;
  statusFetchError?: string | null;
  hasLatestSnapshot: boolean;
  snapshotKind?: string | null;
  targetCount?: number | null;
  resilienceDegraded?: boolean | null;
  resilienceStatus?: string | null;
  resilienceSummary?: string | null;
  socketServiceStatus?: string | null;
  socketConsecutiveIdleTimeouts?: number | null;
  socketTotalIdleTimeouts?: number | null;
  hasSummaryJson: boolean;
  hasAnalysisJson: boolean;
  hasTrainingExampleJson: boolean;
  hasTrainingDatasetManifest: boolean;
  hasExportJson: boolean;
  hasReportJson: boolean;
  hasReportHtml: boolean;
  hasExternalSidecarContext: boolean;
  hasExternalEvidenceChainEnrichment: boolean;
  hasExternalDiagnosticOpinion: boolean;
}

interface RuntimeSidecarMemorySnapshot {
  versionsSupported: boolean;
  slotCount: number;
  historyCount: number;
  latestSlot?: string | null;
  latestLabel?: string | null;
  latestSource?: string | null;
  fetchError?: string | null;
}

interface RuntimeSidecarStatusSnapshot {
  statusSource: string;
  statusFetchedAt?: string | null;
  statusFetchError?: string | null;
  healthy: boolean;
  daemonStatus: string;
  targetCount?: number | null;
  learningActive: boolean;
  learnedRoutes: number;
  hasEvidenceChainEnrichment: boolean;
  hasDiagnosticOpinion: boolean;
  lastError?: string | null;
  memory?: RuntimeSidecarMemorySnapshot | null;
}

interface RuntimeSummary {
  runtimeId: string;
  name: string;
  endpoint: string;
  sidecarEndpoint?: string | null;
  hasSidecarAdminToken?: boolean;
  registeredAt: string;
  updatedAt: string;
  capabilities: RuntimeCapability[];
  capabilitySource: string;
  capabilityFetchedAt?: string | null;
  capabilityFetchError?: string | null;
  tags: RuntimeTags;
  status: RuntimeStatusSnapshot;
  sidecarStatus?: RuntimeSidecarStatusSnapshot | null;
}

interface RuntimeRecoveryActivity {
  action: string;
  outcome: string;
  summary: string;
  recordedAt: string;
}

interface RuntimeSuggestedAction {
  action: string;
  priority: number;
  hint: string;
  coolingDown?: boolean;
  cooldownSecondsRemaining?: number;
}

interface RuntimeAttentionItem {
  runtimeId: string;
  name: string;
  endpoint: string;
  tags: RuntimeTags;
  status: RuntimeStatusSnapshot;
  severity: string;
  reasons: string[];
  suggestedActions: RuntimeSuggestedAction[];
  recentRecoveryActivities: RuntimeRecoveryActivity[];
}

interface RuntimeAttentionView extends RuntimeAttentionItem {
  needsAttention: boolean;
}

interface RuntimeProtocolReadingCompanion {
  protocol: string;
  entry: string;
  viaOverlay?: string | null;
  surfacePath: string;
}

interface RuntimeProtocolReadingSummary {
  runtimeId: string;
  name: string;
  endpoint: string;
  targetName: string;
  targetPathSegment: string;
  targetUrlPath: string;
  currentSurfacePath: string;
  protocol: string;
  entry: string;
  defaultEntry: string;
  selectedIsDefault: boolean;
  selectedOverlay?: string | null;
  readingCompanions: RuntimeProtocolReadingCompanion[];
}

interface OrchestraPlanStep {
  key: string;
  title: string;
  detail: string;
  kind: string;
}

interface OrchestraSuggestedSurface {
  label: string;
  path: string;
}

interface OrchestraPlan {
  planId: string;
  intent: string;
  title: string;
  summary: string;
  riskLevel: string;
  executionReadiness: string;
  executionMode: string;
  reasons: string[];
  requiredCapabilities: string[];
  steps: OrchestraPlanStep[];
  suggestedSurfaces: OrchestraSuggestedSurface[];
}

interface OrchestraRuntimePlanResponse {
  runtimeId: string;
  name: string;
  endpoint: string;
  tags: RuntimeTags;
  statusSource: string;
  attentionSeverity: string;
  needsAttention: boolean;
  attentionReasons: string[];
  plans: OrchestraPlan[];
}

interface OrchestraExecutionStepResult {
  step: string;
  outcome: string;
  summary: string;
}

interface OrchestraRunSummary {
  runId: string;
  runtimeId: string;
  planId: string;
  outcome: string;
  executedAt: string;
  steps: OrchestraExecutionStepResult[];
}
