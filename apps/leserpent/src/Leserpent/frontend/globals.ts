interface DashboardState {
  filter: {
    environment: string;
    cluster: string;
    role: string;
  };
  languagePreference: string;
  language: string;
  installedLanguagePacks: Record<string, any>;
  languagePackCatalog: any[];
  languagePackCatalogMeta: { official: number; builtin: number };
  themePreference: string;
  theme: string;
  layoutMode: string;
  activeTab: string;
  activeOverviewTab: string;
  activeRuntimeMainTab: string;
  activeRuntimeSideTab: string;
  activeRuntimeDetailTab: string;
  runtimePanelView: string;
  runtimeWindowIds: string[];
  activeRuntimeWindowId: string | null;
  runtimeWindowViews: Record<string, string>;
  runtimeSearch: string;
  runtimeSort: string;
  selectedRuntimeId: string | null;
  orchestraPlan?: OrchestraRuntimePlanResponse | null;
  orchestraRequestSeq?: number;
  runtimeAttentionById: Map<string, RuntimeAttentionView>;
  recentBadgeRefresh: {
    runtime: Nullable<number>;
    sidecar: Nullable<number>;
  };
  latestRuntimes: RuntimeSummary[];
  registerNameTouched: boolean;
  adminToken: string;
  adminTokenVisible: boolean;
  adminTokenTestState: string;
  cache?: Record<string, any>;
}
