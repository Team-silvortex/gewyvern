using System.Globalization;
using Avalonia.Media;

internal enum DesktopTextKey
{
    AboutLeserpent,
    LearningCenter,
    Connection,
    Language,
    ShowLeserpent,
    QuitLeserpent,
    Close,
    Cancel,
    Apply,
    Clear,
    Open,
    Refresh,
    Manage,
    RefreshAll,
    FollowSystem,
    QuickTour,
    AddDaemon,
    DeployDaemon,
    RetireDaemon,
    ProvisionGewyvern,
    RetireGewyvern,
    ControlTopology,
    HubSubcopy,
    FindDaemonOrRuntime,
    NoTopologyMatches,
    NoAuthorities,
    LocalOrchestra,
    ManagedOnDevice,
    Local,
    EphemeralSessionAuthority,
    LanguageSettingsTitle,
    LanguageSettingsKicker,
    LanguageSettingsHeading,
    LanguageSettingsBody,
    LanguagePreference,
    DesktopCoverage,
    CoverageComplete,
    CoverageCore,
    CoverageFallback,
    AppliesImmediately,
    LanguagePacks,
    LanguagePackSource,
    DownloadLanguagePack,
    LanguagePackSourceHint,
    LanguagePackDownloadSucceeded,
    InstallLanguagePack,
    RemoveLanguagePack,
    BuiltInLanguagePack,
    LanguagePackInstalled,
    LanguagePackNotInstalled,
    LanguagePackInstallSucceeded,
    LanguagePackRemoveSucceeded,
    LanguagePackOperationFailed,
    TutorialKicker,
    TutorialHeading,
    TutorialBody,
    Previous,
    Next,
    Finish,
    StepProgress,
    AboutDescription,
    Version,
    Dismiss,
    Reconnect,
    FilterRuntimes,
    AwaitingAuthorityCheck,
    RefreshHealth,
    RemoteConnectionState,
    RemoteOperationStatus,
    ManageConnection,
    RuntimeResultCount,
    Reload,
    LiveLogs,
    Acknowledge,
    SearchSanitizedLogs,
    CopyDiagnostics,
    SaveDiagnostics,
    WorkspaceLeselang,
    LogsLoadWithSnapshot,
    RequiredSuffix,
}

internal enum DesktopLocaleCoverage
{
    Complete,
    Core,
    EnglishFallback,
}

internal sealed record DesktopLocaleDefinition(
    string Locale,
    string Name,
    string NativeName,
    bool BuiltIn,
    bool IsRightToLeft,
    DesktopLocaleCoverage Coverage,
    IReadOnlyDictionary<DesktopTextKey, string> Text,
    IReadOnlyDictionary<string, string> SemanticText);

internal sealed class DesktopLocalization
{
    public const string Schema = "leserpent.desktop-localization/v1";
    public const string SystemPreference = "system";
    private static readonly IReadOnlyDictionary<DesktopTextKey, string> EnglishText =
        new Dictionary<DesktopTextKey, string>
        {
            [DesktopTextKey.AboutLeserpent] = "About Leserpent",
            [DesktopTextKey.LearningCenter] = "Learning Center...",
            [DesktopTextKey.Connection] = "Connection...",
            [DesktopTextKey.Language] = "Language...",
            [DesktopTextKey.ShowLeserpent] = "Show Leserpent",
            [DesktopTextKey.QuitLeserpent] = "Quit Leserpent",
            [DesktopTextKey.Close] = "Close",
            [DesktopTextKey.Cancel] = "Cancel",
            [DesktopTextKey.Apply] = "Apply",
            [DesktopTextKey.Clear] = "Clear",
            [DesktopTextKey.Open] = "Open",
            [DesktopTextKey.Refresh] = "Refresh",
            [DesktopTextKey.Manage] = "Manage",
            [DesktopTextKey.RefreshAll] = "Refresh all",
            [DesktopTextKey.FollowSystem] = "Follow System",
            [DesktopTextKey.QuickTour] = "Quick tour",
            [DesktopTextKey.AddDaemon] = "+ Add daemon",
            [DesktopTextKey.DeployDaemon] = "Deploy daemon",
            [DesktopTextKey.RetireDaemon] = "Retire daemon",
            [DesktopTextKey.ProvisionGewyvern] = "Provision gewyvern",
            [DesktopTextKey.RetireGewyvern] = "Retire gewyvern",
            [DesktopTextKey.ControlTopology] = "Control topology",
            [DesktopTextKey.HubSubcopy] = "One client, multiple daemon authorities. Open a daemon to manage its gewyvern runtimes.",
            [DesktopTextKey.FindDaemonOrRuntime] = "Find a daemon or runtime",
            [DesktopTextKey.NoTopologyMatches] = "No daemon authorities or runtimes match this filter.",
            [DesktopTextKey.NoAuthorities] = "No daemon authorities are configured. Add one to establish the first topology branch.",
            [DesktopTextKey.LocalOrchestra] = "Local Orchestra",
            [DesktopTextKey.ManagedOnDevice] = "Managed on this device",
            [DesktopTextKey.Local] = "LOCAL",
            [DesktopTextKey.EphemeralSessionAuthority] = "Ephemeral session authority",
            [DesktopTextKey.LanguageSettingsTitle] = "Leserpent / Language",
            [DesktopTextKey.LanguageSettingsKicker] = "INTERFACE LANGUAGE",
            [DesktopTextKey.LanguageSettingsHeading] = "Choose how Leserpent speaks",
            [DesktopTextKey.LanguageSettingsBody] = "The preference applies to the native shell, Learning Center, accessible names, and localized UI-IR text. Missing keys fall back to English without changing action identities.",
            [DesktopTextKey.LanguagePreference] = "Language preference",
            [DesktopTextKey.DesktopCoverage] = "Desktop coverage",
            [DesktopTextKey.CoverageComplete] = "Complete desktop shell and Learning Center",
            [DesktopTextKey.CoverageCore] = "Core desktop shell; extended views use English fallback",
            [DesktopTextKey.CoverageFallback] = "Official locale available; desktop-specific text currently uses English fallback",
            [DesktopTextKey.AppliesImmediately] = "Applies immediately and is stored only on this device.",
            [DesktopTextKey.LanguagePacks] = "Language packs",
            [DesktopTextKey.LanguagePackSource] = "Daemon catalog source",
            [DesktopTextKey.DownloadLanguagePack] = "Download selected",
            [DesktopTextKey.LanguagePackSourceHint] = "Downloads use the selected daemon's saved TLS trust and never send its admin credential. Install JSON remains available offline.",
            [DesktopTextKey.LanguagePackDownloadSucceeded] = "Downloaded and installed {0} from {1}.",
            [DesktopTextKey.InstallLanguagePack] = "Install JSON...",
            [DesktopTextKey.RemoveLanguagePack] = "Remove pack",
            [DesktopTextKey.BuiltInLanguagePack] = "Built in; no language pack is required.",
            [DesktopTextKey.LanguagePackInstalled] = "Installed language pack {0}.",
            [DesktopTextKey.LanguagePackNotInstalled] = "This downloadable language pack is not installed on this device.",
            [DesktopTextKey.LanguagePackInstallSucceeded] = "Installed {0}.",
            [DesktopTextKey.LanguagePackRemoveSucceeded] = "Removed {0}; missing text now falls back to English.",
            [DesktopTextKey.LanguagePackOperationFailed] = "Language-pack operation failed: {0}",
            [DesktopTextKey.TutorialKicker] = "LESERPENT LEARNING CENTER",
            [DesktopTextKey.TutorialHeading] = "A six-step operator tour",
            [DesktopTextKey.TutorialBody] = "Offline, read-only, and safe to revisit. No connection, deployment, or command starts from this window.",
            [DesktopTextKey.Previous] = "Previous",
            [DesktopTextKey.Next] = "Next",
            [DesktopTextKey.Finish] = "Finish",
            [DesktopTextKey.StepProgress] = "Step {0} of {1}",
            [DesktopTextKey.AboutDescription] = "Native orchestration and model-driven control for Gewyvern.",
            [DesktopTextKey.Version] = "Version {0}",
            [DesktopTextKey.Dismiss] = "Dismiss",
            [DesktopTextKey.Reconnect] = "Reconnect",
            [DesktopTextKey.FilterRuntimes] = "Filter runtimes by name, ID, tag, or status",
            [DesktopTextKey.AwaitingAuthorityCheck] = "AUTHORITY / awaiting check",
            [DesktopTextKey.RefreshHealth] = "Refresh health",
            [DesktopTextKey.RemoteConnectionState] = "Remote connection state",
            [DesktopTextKey.RemoteOperationStatus] = "Remote operation status",
            [DesktopTextKey.ManageConnection] = "Manage remote connection",
            [DesktopTextKey.RuntimeResultCount] = "Remote runtime result count",
            [DesktopTextKey.Reload] = "Reload",
            [DesktopTextKey.LiveLogs] = "Live logs",
            [DesktopTextKey.Acknowledge] = "Acknowledge",
            [DesktopTextKey.SearchSanitizedLogs] = "Search sanitized logs",
            [DesktopTextKey.CopyDiagnostics] = "Copy diagnostics",
            [DesktopTextKey.SaveDiagnostics] = "Save diagnostics",
            [DesktopTextKey.WorkspaceLeselang] = "Workspace Leselang",
            [DesktopTextKey.LogsLoadWithSnapshot] = "Logs load with the runtime snapshot",
            [DesktopTextKey.RequiredSuffix] = "{0} (required)",
        };

    private static readonly IReadOnlyDictionary<DesktopTextKey, string> SimplifiedChineseText =
        new Dictionary<DesktopTextKey, string>
        {
            [DesktopTextKey.AboutLeserpent] = "关于 Leserpent",
            [DesktopTextKey.LearningCenter] = "学习中心...",
            [DesktopTextKey.Connection] = "连接...",
            [DesktopTextKey.Language] = "语言...",
            [DesktopTextKey.ShowLeserpent] = "显示 Leserpent",
            [DesktopTextKey.QuitLeserpent] = "退出 Leserpent",
            [DesktopTextKey.Close] = "关闭",
            [DesktopTextKey.Cancel] = "取消",
            [DesktopTextKey.Apply] = "应用",
            [DesktopTextKey.Clear] = "清除",
            [DesktopTextKey.Open] = "打开",
            [DesktopTextKey.Refresh] = "刷新",
            [DesktopTextKey.Manage] = "管理",
            [DesktopTextKey.RefreshAll] = "全部刷新",
            [DesktopTextKey.FollowSystem] = "跟随系统",
            [DesktopTextKey.QuickTour] = "快速教学",
            [DesktopTextKey.AddDaemon] = "+ 添加 daemon",
            [DesktopTextKey.DeployDaemon] = "部署 daemon",
            [DesktopTextKey.RetireDaemon] = "退役 daemon",
            [DesktopTextKey.ProvisionGewyvern] = "部署 gewyvern",
            [DesktopTextKey.RetireGewyvern] = "退役 gewyvern",
            [DesktopTextKey.ControlTopology] = "控制拓扑",
            [DesktopTextKey.HubSubcopy] = "一个客户端，管理多个 daemon 权威端；打开 daemon 即可管理其下属的 gewyvern runtime。",
            [DesktopTextKey.FindDaemonOrRuntime] = "查找 daemon 或 runtime",
            [DesktopTextKey.NoTopologyMatches] = "没有符合当前筛选条件的 daemon 权威端或 runtime。",
            [DesktopTextKey.NoAuthorities] = "尚未配置 daemon 权威端。添加一个即可建立第一条拓扑分支。",
            [DesktopTextKey.LocalOrchestra] = "本地 Orchestra",
            [DesktopTextKey.ManagedOnDevice] = "由此设备管理",
            [DesktopTextKey.Local] = "本地",
            [DesktopTextKey.EphemeralSessionAuthority] = "临时会话权威端",
            [DesktopTextKey.LanguageSettingsTitle] = "Leserpent / 语言",
            [DesktopTextKey.LanguageSettingsKicker] = "界面语言",
            [DesktopTextKey.LanguageSettingsHeading] = "选择 Leserpent 的界面语言",
            [DesktopTextKey.LanguageSettingsBody] = "该偏好会应用到原生外壳、学习中心、无障碍名称和已本地化的 UI-IR 文本。缺失词条会确定性回退英文，动作标识不会改变。",
            [DesktopTextKey.LanguagePreference] = "语言偏好",
            [DesktopTextKey.DesktopCoverage] = "桌面端覆盖度",
            [DesktopTextKey.CoverageComplete] = "完整桌面外壳与学习中心",
            [DesktopTextKey.CoverageCore] = "核心桌面外壳；扩展视图回退英文",
            [DesktopTextKey.CoverageFallback] = "官方语言已可选；桌面专用文案目前回退英文",
            [DesktopTextKey.AppliesImmediately] = "立即生效，且仅保存在此设备上。",
            [DesktopTextKey.LanguagePacks] = "语言包",
            [DesktopTextKey.LanguagePackSource] = "Daemon 目录来源",
            [DesktopTextKey.DownloadLanguagePack] = "下载所选语言",
            [DesktopTextKey.LanguagePackSourceHint] = "下载仅使用所选 daemon 已保存的 TLS 信任，绝不会发送管理凭证；离线时仍可安装 JSON。",
            [DesktopTextKey.LanguagePackDownloadSucceeded] = "已从 {1} 下载并安装 {0}。",
            [DesktopTextKey.InstallLanguagePack] = "安装 JSON...",
            [DesktopTextKey.RemoveLanguagePack] = "移除语言包",
            [DesktopTextKey.BuiltInLanguagePack] = "内置语言，无需安装语言包。",
            [DesktopTextKey.LanguagePackInstalled] = "已安装语言包 {0}。",
            [DesktopTextKey.LanguagePackNotInstalled] = "此设备尚未安装这个可下载语言包。",
            [DesktopTextKey.LanguagePackInstallSucceeded] = "已安装 {0}。",
            [DesktopTextKey.LanguagePackRemoveSucceeded] = "已移除 {0}；缺失文案现已回退英文。",
            [DesktopTextKey.LanguagePackOperationFailed] = "语言包操作失败：{0}",
            [DesktopTextKey.TutorialKicker] = "LESERPENT 学习中心",
            [DesktopTextKey.TutorialHeading] = "六步操作员入门",
            [DesktopTextKey.TutorialBody] = "离线、只读，随时可以重新查看。此窗口不会发起连接、部署或命令。",
            [DesktopTextKey.Previous] = "上一步",
            [DesktopTextKey.Next] = "下一步",
            [DesktopTextKey.Finish] = "完成",
            [DesktopTextKey.StepProgress] = "第 {0} 步，共 {1} 步",
            [DesktopTextKey.AboutDescription] = "面向 Gewyvern 的原生编排与模型驱动控制。",
            [DesktopTextKey.Version] = "版本 {0}",
            [DesktopTextKey.Dismiss] = "收起",
            [DesktopTextKey.Reconnect] = "重新连接",
            [DesktopTextKey.FilterRuntimes] = "按名称、ID、标签或状态筛选 runtime",
            [DesktopTextKey.AwaitingAuthorityCheck] = "权威端 / 等待检查",
            [DesktopTextKey.RefreshHealth] = "刷新健康状态",
            [DesktopTextKey.RemoteConnectionState] = "远程连接状态",
            [DesktopTextKey.RemoteOperationStatus] = "远程操作状态",
            [DesktopTextKey.ManageConnection] = "管理远程连接",
            [DesktopTextKey.RuntimeResultCount] = "远程 runtime 结果数量",
            [DesktopTextKey.Reload] = "重新载入",
            [DesktopTextKey.LiveLogs] = "实时日志",
            [DesktopTextKey.Acknowledge] = "确认告警",
            [DesktopTextKey.SearchSanitizedLogs] = "搜索净化后的日志",
            [DesktopTextKey.CopyDiagnostics] = "复制诊断",
            [DesktopTextKey.SaveDiagnostics] = "保存诊断",
            [DesktopTextKey.WorkspaceLeselang] = "工作区 Leselang",
            [DesktopTextKey.LogsLoadWithSnapshot] = "日志会随 runtime 快照载入",
            [DesktopTextKey.RequiredSuffix] = "{0}（必填）",
        };

    private static readonly IReadOnlyDictionary<string, string> SimplifiedChineseSemanticText =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["remote.title"] = "远程 runtime",
            ["remote.fleet"] = "远程 runtime 集群",
            ["remote.filter.empty"] = "当前筛选条件没有匹配的 runtime",
            ["runtime.inspect"] = "检查 runtime",
            ["runtime.inspect.description"] = "打开只读 runtime 工作区",
            ["runtime.refresh"] = "刷新 runtime",
            ["runtime.refresh.description"] = "更改远程状态前需要明确确认",
            ["runtime.capabilities.refresh"] = "探测能力",
            ["runtime.capabilities.refresh.description"] = "查询远程 runtime 前需要明确确认",
            ["runtime.workspace.refresh"] = "刷新",
            ["runtime.workspace.refresh.description"] = "需要在集群窗口中确认",
            ["runtime.history.empty"] = "没有已应用的命令",
            ["runtime.history.title"] = "历史记录",
            ["runtime.logs.empty"] = "没有日志记录",
            ["runtime.logs.filtered_empty"] = "没有匹配的日志记录",
            ["runtime.logs.title"] = "日志",
            ["runtime.capabilities.unobserved"] = "尚未探测能力",
            ["runtime.capabilities.title"] = "能力",
            ["runtime.deploy"] = "部署管道",
            ["runtime.deploy.description"] = "打开有边界的部署表单，并要求明确确认",
            ["runtime.deploy.form.title"] = "确认远程部署",
            ["runtime.deploy.form.submit"] = "部署管道",
            ["runtime.deploy.form.pipeline_kind"] = "管道类型",
            ["runtime.deploy.form.pipeline_kind.placeholder"] = "http/request",
            ["runtime.deploy.form.target"] = "可选目标",
            ["runtime.deploy.form.target.placeholder"] = "例如 pid:42",
        };

    private static readonly DesktopLocaleDefinition[] LocaleDefinitions =
    [
        Complete(
            "en",
            "English",
            "English",
            EnglishText,
            new Dictionary<string, string>(StringComparer.Ordinal)),
        Complete("zh-CN", "Chinese (Simplified)", "简体中文", SimplifiedChineseText,
            MergeSemantic(
                SimplifiedChineseSemanticText,
                DesktopConnectionCatalogs.SimplifiedChinese,
                DesktopBootstrapDeploymentCatalogs.SimplifiedChinese,
                DesktopProvisioningCatalogs.SimplifiedChinese,
                DesktopRetirementCatalogs.SimplifiedChinese,
                DesktopDaemonRetirementCatalogs.SimplifiedChinese,
                DesktopStartupRecoveryCatalogs.SimplifiedChinese,
                DesktopAccountCatalogs.SimplifiedChinese,
                DesktopRemoteShellCatalogs.SimplifiedChinese,
                DesktopRemoteOperationCatalogs.SimplifiedChinese,
                DesktopRuntimeWorkspaceCatalogs.SimplifiedChinese,
                DesktopOrchestraCatalogs.SimplifiedChinese,
                DesktopHubCatalogs.SimplifiedChinese,
                DesktopTutorialCatalogs.SimplifiedChinese)),
        BuiltInShell(
            "zh-TW",
            "Chinese (Traditional)",
            "繁體中文",
            DesktopBuiltInShellCatalogs.TraditionalChinese,
            MergeSemantic(
                DesktopBuiltInSemanticCatalogs.TraditionalChinese,
                DesktopConnectionCatalogs.TraditionalChinese,
                DesktopBootstrapDeploymentCatalogs.TraditionalChinese,
                DesktopProvisioningCatalogs.TraditionalChinese,
                DesktopRetirementCatalogs.TraditionalChinese,
                DesktopDaemonRetirementCatalogs.TraditionalChinese,
                DesktopStartupRecoveryCatalogs.TraditionalChinese,
                DesktopAccountCatalogs.TraditionalChinese,
                DesktopRemoteShellCatalogs.TraditionalChinese,
                DesktopRemoteOperationCatalogs.TraditionalChinese,
                DesktopRuntimeWorkspaceCatalogs.TraditionalChinese,
                DesktopOrchestraCatalogs.TraditionalChinese,
                DesktopHubCatalogs.TraditionalChinese,
                DesktopTutorialCatalogs.TraditionalChinese)),
        BuiltInShell(
            "ja",
            "Japanese",
            "日本語",
            DesktopBuiltInShellCatalogs.Japanese,
            MergeSemantic(
                DesktopBuiltInSemanticCatalogs.Japanese,
                DesktopConnectionCatalogs.Japanese,
                DesktopBootstrapDeploymentCatalogs.Japanese,
                DesktopProvisioningCatalogs.Japanese,
                DesktopRetirementCatalogs.Japanese,
                DesktopDaemonRetirementCatalogs.Japanese,
                DesktopStartupRecoveryCatalogs.Japanese,
                DesktopAccountCatalogs.Japanese,
                DesktopRemoteShellCatalogs.Japanese,
                DesktopRemoteOperationCatalogs.Japanese,
                DesktopRuntimeWorkspaceCatalogs.Japanese,
                DesktopOrchestraCatalogs.Japanese,
                DesktopHubCatalogs.Japanese,
                DesktopTutorialCatalogs.Japanese)),
        BuiltInShell(
            "es",
            "Spanish",
            "Español",
            DesktopBuiltInShellCatalogs.Spanish,
            MergeSemantic(
                DesktopBuiltInSemanticCatalogs.Spanish,
                DesktopConnectionCatalogs.Spanish,
                DesktopBootstrapDeploymentCatalogs.Spanish,
                DesktopProvisioningCatalogs.Spanish,
                DesktopRetirementCatalogs.Spanish,
                DesktopDaemonRetirementCatalogs.Spanish,
                DesktopStartupRecoveryCatalogs.Spanish,
                DesktopAccountCatalogs.Spanish,
                DesktopRemoteShellCatalogs.Spanish,
                DesktopRemoteOperationCatalogs.Spanish,
                DesktopRuntimeWorkspaceCatalogs.Spanish,
                DesktopOrchestraCatalogs.Spanish,
                DesktopHubCatalogs.Spanish,
                DesktopTutorialCatalogs.Spanish)),
        BuiltInShell(
            "de",
            "German",
            "Deutsch",
            DesktopBuiltInShellCatalogs.German,
            MergeSemantic(
                DesktopBuiltInSemanticCatalogs.German,
                DesktopConnectionCatalogs.German,
                DesktopBootstrapDeploymentCatalogs.German,
                DesktopProvisioningCatalogs.German,
                DesktopRetirementCatalogs.German,
                DesktopDaemonRetirementCatalogs.German,
                DesktopStartupRecoveryCatalogs.German,
                DesktopAccountCatalogs.German,
                DesktopRemoteShellCatalogs.German,
                DesktopRemoteOperationCatalogs.German,
                DesktopRuntimeWorkspaceCatalogs.German,
                DesktopOrchestraCatalogs.German,
                DesktopHubCatalogs.German,
                DesktopTutorialCatalogs.German)),
        BuiltInShell(
            "fr",
            "French",
            "Français",
            DesktopBuiltInShellCatalogs.French,
            MergeSemantic(
                DesktopBuiltInSemanticCatalogs.French,
                DesktopConnectionCatalogs.French,
                DesktopBootstrapDeploymentCatalogs.French,
                DesktopProvisioningCatalogs.French,
                DesktopRetirementCatalogs.French,
                DesktopDaemonRetirementCatalogs.French,
                DesktopStartupRecoveryCatalogs.French,
                DesktopAccountCatalogs.French,
                DesktopRemoteShellCatalogs.French,
                DesktopRemoteOperationCatalogs.French,
                DesktopRuntimeWorkspaceCatalogs.French,
                DesktopOrchestraCatalogs.French,
                DesktopHubCatalogs.French,
                DesktopTutorialCatalogs.French)),
        BuiltInShell(
            "ko",
            "Korean",
            "한국어",
            DesktopBuiltInShellCatalogs.Korean,
            MergeSemantic(
                DesktopBuiltInSemanticCatalogs.Korean,
                DesktopConnectionCatalogs.Korean,
                DesktopBootstrapDeploymentCatalogs.Korean,
                DesktopProvisioningCatalogs.Korean,
                DesktopRetirementCatalogs.Korean,
                DesktopDaemonRetirementCatalogs.Korean,
                DesktopStartupRecoveryCatalogs.Korean,
                DesktopAccountCatalogs.Korean,
                DesktopRemoteShellCatalogs.Korean,
                DesktopRemoteOperationCatalogs.Korean,
                DesktopRuntimeWorkspaceCatalogs.Korean,
                DesktopOrchestraCatalogs.Korean,
                DesktopHubCatalogs.Korean,
                DesktopTutorialCatalogs.Korean)),
        Core("pt-BR", "Portuguese (Brazil)", "Português (Brasil)", "Idioma",
            "Painel do plano de controle", "Uma visão leve da frota para vários runtimes gewyvern próximos.", false),
        Core("it", "Italian", "Italiano", "Lingua", "Dashboard del piano di controllo",
            "Una vista leggera della flotta per più runtime gewyvern vicini.", false),
        Core("ru", "Russian", "Русский", "Язык", "Панель управления",
            "Лёгкое представление флота для нескольких ближайших runtime gewyvern.", false),
        Core("ar", "Arabic", "العربية", "اللغة", "لوحة مستوى التحكم",
            "عرض خفيف للأسطول لعدة بيئات تشغيل gewyvern قريبة.", false, true),
        Core("hi", "Hindi", "हिन्दी", "भाषा", "कंट्रोल प्लेन डैशबोर्ड",
            "कई निकटवर्ती gewyvern रनटाइम के लिए हल्का फ़्लीट दृश्य।", false),
        Core("bn", "Bengali", "বাংলা", "ভাষা", "কন্ট্রোল প্লেন ড্যাশবোর্ড",
            "একাধিক কাছাকাছি gewyvern রানটাইমের জন্য হালকা ফ্লিট ভিউ।", false),
        Core("id", "Indonesian", "Bahasa Indonesia", "Bahasa", "Dasbor bidang kontrol",
            "Tampilan armada ringan untuk beberapa runtime gewyvern terdekat.", false),
        Core("ms", "Malay", "Bahasa Melayu", "Bahasa", "Papan pemuka satah kawalan",
            "Paparan armada ringan untuk beberapa runtime gewyvern berdekatan.", false),
        Core("th", "Thai", "ไทย", "ภาษา", "แดชบอร์ดระนาบควบคุม",
            "มุมมองฟลีตแบบเบาสำหรับ gewyvern runtime หลายตัวที่อยู่ใกล้กัน", false),
        Core("vi", "Vietnamese", "Tiếng Việt", "Ngôn ngữ",
            "Bảng điều khiển mặt phẳng điều khiển",
            "Chế độ xem đội nhẹ cho nhiều runtime gewyvern ở gần.", false),
        Core("tr", "Turkish", "Türkçe", "Dil", "Kontrol düzlemi panosu",
            "Yakındaki birden çok gewyvern çalışma zamanı için hafif filo görünümü.", false),
        Core("pl", "Polish", "Polski", "Język", "Panel płaszczyzny sterowania",
            "Lekki widok floty dla wielu pobliskich środowisk wykonawczych gewyvern.", false),
        Core("nl", "Dutch", "Nederlands", "Taal", "Dashboard voor het besturingsvlak",
            "Een lichte vlootweergave voor meerdere gewyvern-runtimes in de buurt.", false),
        Core("uk", "Ukrainian", "Українська", "Мова", "Панель площини керування",
            "Легкий огляд флоту для кількох сусідніх runtime gewyvern.", false),
        Core("cs", "Czech", "Čeština", "Jazyk", "Řídicí panel",
            "Lehký pohled na flotilu několika blízkých runtime gewyvern.", false),
        Core("sv", "Swedish", "Svenska", "Språk", "Kontrollplanspanel",
            "En lätt flottvy för flera närliggande gewyvern-körmiljöer.", false),
        Core("da", "Danish", "Dansk", "Sprog", "Kontrolplanspanel",
            "En let flådevisning til flere nærliggende gewyvern-kørsler.", false),
        Core("no", "Norwegian", "Norsk", "Språk", "Kontrollplandashboard",
            "En lett flåtevisning for flere gewyvern-kjøretider i nærheten.", false),
        Core("fi", "Finnish", "Suomi", "Kieli", "Ohjaustason koontinäyttö",
            "Kevyt laivastonäkymä useille lähellä oleville gewyvern-ajoympäristöille.", false),
        Core("el", "Greek", "Ελληνικά", "Γλώσσα", "Πίνακας επιπέδου ελέγχου",
            "Μια ελαφριά προβολή στόλου για πολλά κοντινά runtime gewyvern.", false),
        Core("he", "Hebrew", "עברית", "שפה", "לוח מישור הבקרה",
            "תצוגת צי קלה עבור מספר סביבות gewyvern קרובות.", false, true),
        Core("fa", "Persian", "فارسی", "زبان", "داشبورد صفحه کنترل",
            "نمای سبک ناوگان برای چند محیط اجرای نزدیک gewyvern.", false, true),
    ];

    private static readonly IReadOnlyDictionary<string, DesktopLocaleDefinition> LocalesById =
        LocaleDefinitions.ToDictionary(locale => locale.Locale, StringComparer.OrdinalIgnoreCase);
    private readonly DesktopLanguagePreferenceStore? store;
    private readonly DesktopLanguagePackStore? languagePackStore;
    private readonly string systemLocale;
    private IReadOnlyDictionary<string, DesktopInstalledLanguagePack> installedLanguagePacks;

    private DesktopLocalization(
        DesktopLanguagePreferenceStore? store,
        string preference,
        string systemLocale,
        DesktopLanguagePackStore? languagePackStore,
        IReadOnlyDictionary<string, DesktopInstalledLanguagePack> installedLanguagePacks)
    {
        this.store = store;
        this.languagePackStore = languagePackStore;
        this.systemLocale = systemLocale;
        this.installedLanguagePacks = installedLanguagePacks;
        ValidatePreference(preference);
        Preference = CanonicalPreference(preference);
        Active = ResolveActive(Preference, systemLocale);
    }

    public event EventHandler? Changed;
    public string Preference { get; private set; }
    public DesktopLocaleDefinition Active { get; private set; }
    public FlowDirection FlowDirection => Active.IsRightToLeft
        ? FlowDirection.RightToLeft
        : FlowDirection.LeftToRight;
    public bool SupportsLanguagePackInstallation => languagePackStore is not null;
    public static IReadOnlyList<DesktopLocaleDefinition> OfficialLocales => LocaleDefinitions;

    public static DesktopLocalization CreateDefault(out string? warning)
    {
        var store = new DesktopLanguagePreferenceStore(
            DesktopLanguagePreferenceStore.DefaultPath());
        var packStore = new DesktopLanguagePackStore(DesktopLanguagePackStore.DefaultPath());
        var warnings = new List<string>();
        string preference;
        try
        {
            preference = store.Load();
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            preference = SystemPreference;
            warnings.Add(
                $"Language preference could not be loaded; following the system language. {StartupFailure.Describe(error)}");
        }
        DesktopLanguagePackSnapshot packs;
        try
        {
            packs = packStore.LoadAll();
            if (packs.RejectedFiles.Count > 0)
            {
                warnings.Add(
                    $"{packs.RejectedFiles.Count} invalid language-pack file(s) were ignored.");
            }
        }
        catch (Exception error) when (StartupFailure.IsExpected(error))
        {
            packs = new DesktopLanguagePackSnapshot(
                new Dictionary<string, DesktopInstalledLanguagePack>(
                    StringComparer.OrdinalIgnoreCase),
                []);
            warnings.Add(
                $"Installed language packs could not be loaded; safe fallback remains active. {StartupFailure.Describe(error)}");
        }
        warning = warnings.Count == 0 ? null : string.Join(' ', warnings);
        return new DesktopLocalization(
            store,
            preference,
            CultureInfo.CurrentUICulture.Name,
            packStore,
            packs.Packs);
    }

    public static DesktopLocalization ForVerification(
        string preference = "en",
        string systemLocale = "en-US") =>
        new(
            null,
            preference,
            systemLocale,
            null,
            new Dictionary<string, DesktopInstalledLanguagePack>(
                StringComparer.OrdinalIgnoreCase));

    internal static DesktopLocalization ForLanguagePackVerification(
        string root,
        string preference = "en",
        string systemLocale = "en-US")
    {
        var packStore = new DesktopLanguagePackStore(root);
        var snapshot = packStore.LoadAll();
        return new DesktopLocalization(
            null,
            preference,
            systemLocale,
            packStore,
            snapshot.Packs);
    }

    public string Text(DesktopTextKey key)
    {
        if (installedLanguagePacks.TryGetValue(Active.Locale, out var pack)
            && DesktopLanguagePackProjection.TryResolve(pack, key, out var packed))
        {
            return packed;
        }
        return Active.Text.TryGetValue(key, out var value) ? value : EnglishText[key];
    }

    public string Format(DesktopTextKey key, params object[] values) =>
        string.Format(CultureInfo.InvariantCulture, Text(key), values);

    public string Resolve(LocalizedText text)
    {
        var value = Active.SemanticText.TryGetValue(text.Key, out var translated)
            ? translated
            : text.Fallback;
        if (value.Length is <= 0 or > 1024 || value.Any(char.IsControl))
        {
            throw new InvalidDataException("desktop localization produced invalid UI text");
        }
        return value;
    }

    public void SetPreference(string preference)
    {
        ValidatePreference(preference);
        var canonical = CanonicalPreference(preference);
        store?.Save(canonical);
        if (Preference == canonical)
        {
            return;
        }
        Preference = canonical;
        Active = ResolveActive(canonical, systemLocale);
        Changed?.Invoke(this, EventArgs.Empty);
    }

    public DesktopInstalledLanguagePack InstallLanguagePack(Stream stream)
    {
        var packStore = languagePackStore
            ?? throw new InvalidOperationException(
                "desktop language-pack installation is unavailable");
        var installed = packStore.Install(stream);
        ReplaceInstalledPack(installed);
        return installed;
    }

    public async Task<DesktopInstalledLanguagePack> InstallLanguagePackAsync(
        Stream stream,
        CancellationToken cancellationToken = default)
    {
        var packStore = languagePackStore
            ?? throw new InvalidOperationException(
                "desktop language-pack installation is unavailable");
        var installed = await packStore.InstallAsync(
            stream,
            cancellationToken);
        ReplaceInstalledPack(installed);
        return installed;
    }

    internal DesktopInstalledLanguagePack InstallLanguagePack(ReadOnlySpan<byte> payload)
    {
        var packStore = languagePackStore
            ?? throw new InvalidOperationException(
                "desktop language-pack installation is unavailable");
        var installed = packStore.Install(payload);
        ReplaceInstalledPack(installed);
        return installed;
    }

    internal DesktopInstalledLanguagePack InstallCatalogLanguagePack(
        ReadOnlySpan<byte> payload,
        string expectedSha256,
        string expectedLocale,
        string expectedVersion,
        CancellationToken cancellationToken = default)
    {
        var packStore = languagePackStore
            ?? throw new InvalidOperationException(
                "desktop language-pack installation is unavailable");
        var installed = packStore.InstallCatalogArtifact(
            payload,
            expectedSha256,
            expectedLocale,
            expectedVersion,
            cancellationToken);
        ReplaceInstalledPack(installed);
        return installed;
    }

    public void RemoveLanguagePack(string locale)
    {
        var packStore = languagePackStore
            ?? throw new InvalidOperationException(
                "desktop language-pack installation is unavailable");
        packStore.Remove(locale);
        if (!installedLanguagePacks.ContainsKey(locale))
        {
            return;
        }
        var next = new Dictionary<string, DesktopInstalledLanguagePack>(
            installedLanguagePacks,
            StringComparer.OrdinalIgnoreCase);
        next.Remove(locale);
        installedLanguagePacks = next;
        if (Active.Locale.Equals(locale, StringComparison.OrdinalIgnoreCase))
        {
            Changed?.Invoke(this, EventArgs.Empty);
        }
    }

    public bool IsLanguagePackInstalled(string locale) =>
        installedLanguagePacks.ContainsKey(locale);

    public string? InstalledLanguagePackVersion(string locale) =>
        installedLanguagePacks.TryGetValue(locale, out var pack)
            ? pack.Manifest.Version
            : null;

    public static bool TryGetOfficialLocale(
        string locale,
        out DesktopLocaleDefinition definition) =>
        LocalesById.TryGetValue(locale, out definition!);

    public static void ValidatePreference(string preference)
    {
        if (preference != SystemPreference && !LocalesById.ContainsKey(preference))
        {
            throw new InvalidDataException("desktop language preference is not an official locale");
        }
    }

    private void ReplaceInstalledPack(DesktopInstalledLanguagePack installed)
    {
        var next = new Dictionary<string, DesktopInstalledLanguagePack>(
            installedLanguagePacks,
            StringComparer.OrdinalIgnoreCase)
        {
            [installed.Manifest.Locale] = installed,
        };
        installedLanguagePacks = next;
        if (Active.Locale.Equals(
            installed.Manifest.Locale,
            StringComparison.OrdinalIgnoreCase))
        {
            Changed?.Invoke(this, EventArgs.Empty);
        }
    }

    public static void VerifyContract()
    {
        DesktopBuiltInShellCatalogs.VerifyContract();
        DesktopBuiltInSemanticCatalogs.VerifyContract(
            SimplifiedChineseSemanticText.Keys);
        DesktopConnectionCatalogs.VerifyContract();
        DesktopBootstrapDeploymentCatalogs.VerifyContract();
        DesktopProvisioningCatalogs.VerifyContract();
        DesktopRetirementCatalogs.VerifyContract();
        DesktopDaemonRetirementCatalogs.VerifyContract();
        DesktopStartupRecoveryCatalogs.VerifyContract();
        DesktopAccountCatalogs.VerifyContract();
        DesktopRemoteShellCatalogs.VerifyContract();
        DesktopRemoteOperationCatalogs.VerifyContract();
        DesktopRuntimeWorkspaceCatalogs.VerifyContract();
        DesktopRuntimeWorkspacePresentation.VerifyContract();
        DesktopOrchestraCatalogs.VerifyContract();
        DesktopHubCatalogs.VerifyContract();
        DesktopHubPresentation.VerifyContract();
        DesktopTutorialCatalogs.VerifyContract();
        var ids = LocaleDefinitions.Select(locale => locale.Locale).ToArray();
        var desktopTextKeyCount = Enum.GetValues<DesktopTextKey>().Length;
        var builtInSemanticKeyCount = DesktopBuiltInSemanticCatalogs.KeyCount
            + DesktopConnectionCatalogs.KeyCount
            + DesktopBootstrapDeploymentCatalogs.KeyCount
            + DesktopProvisioningCatalogs.KeyCount
            + DesktopRetirementCatalogs.KeyCount
            + DesktopDaemonRetirementCatalogs.KeyCount
            + DesktopStartupRecoveryCatalogs.KeyCount
            + DesktopAccountCatalogs.KeyCount
            + DesktopRemoteShellCatalogs.KeyCount
            + DesktopRemoteOperationCatalogs.KeyCount
            + DesktopRuntimeWorkspaceCatalogs.KeyCount
            + DesktopOrchestraCatalogs.KeyCount
            + DesktopHubCatalogs.KeyCount
            + DesktopTutorialCatalogs.KeyCount;
        if (Schema != "leserpent.desktop-localization/v1"
            || desktopTextKeyCount != 80
            || LocaleDefinitions.Length != 30
            || LocaleDefinitions.Count(locale => locale.BuiltIn) != 8
            || LocaleDefinitions.Count(locale => locale.BuiltIn
                && locale.Coverage == DesktopLocaleCoverage.Complete) != 8
            || LocaleDefinitions.Count(locale => !locale.BuiltIn
                && locale.Coverage == DesktopLocaleCoverage.Core) != 22
            || LocaleDefinitions.Count(locale => locale.BuiltIn
                && locale.Text.Count == desktopTextKeyCount) != 8
            || LocaleDefinitions.Count(locale => locale.IsRightToLeft) != 3
            || ids.Distinct(StringComparer.OrdinalIgnoreCase).Count() != 30
            || EnglishText.Count != desktopTextKeyCount
            || SimplifiedChineseText.Count != desktopTextKeyCount
            || SimplifiedChineseSemanticText.Count
                != DesktopBuiltInSemanticCatalogs.KeyCount
            || LocaleDefinitions.Count(locale => locale.BuiltIn
                && locale.Locale != "en"
                && locale.SemanticText.Count == builtInSemanticKeyCount) != 7
            || LocaleDefinitions.Any(locale => !ValidLocale(locale)))
        {
            throw new InvalidDataException("desktop localization catalog contract drifted");
        }
        var simplified = ForVerification("zh-CN");
        if (simplified.Text(DesktopTextKey.ControlTopology) != "控制拓扑"
            || simplified.Resolve(new LocalizedText
            {
                Key = "runtime.deploy",
                Fallback = "Deploy pipeline",
            }) != "部署管道"
            || simplified.Resolve(new LocalizedText
            {
                Key = "unknown.key",
                Fallback = "Safe fallback",
            }) != "Safe fallback")
        {
            throw new InvalidDataException("desktop localized-text resolution drifted");
        }
        var builtInSamples = new Dictionary<string, (DesktopTextKey Key, string Value)>
        {
            ["zh-TW"] = (DesktopTextKey.ControlTopology, "控制拓撲"),
            ["ja"] = (DesktopTextKey.Close, "閉じる"),
            ["es"] = (DesktopTextKey.FollowSystem, "Seguir el sistema"),
            ["de"] = (DesktopTextKey.Reconnect, "Neu verbinden"),
            ["fr"] = (DesktopTextKey.LearningCenter, "Centre d’apprentissage..."),
            ["ko"] = (DesktopTextKey.RefreshAll, "모두 새로고침"),
        };
        if (builtInSamples.Any(sample =>
            ForVerification(sample.Key).Text(sample.Value.Key) != sample.Value.Value))
        {
            throw new InvalidDataException("built-in desktop shell translation drifted");
        }
        var semanticSamples = new Dictionary<string, string>
        {
            ["zh-CN"] = "部署管道",
            ["zh-TW"] = "部署 pipeline",
            ["ja"] = "pipeline をデプロイ",
            ["es"] = "Desplegar pipeline",
            ["de"] = "Pipeline bereitstellen",
            ["fr"] = "Déployer le pipeline",
            ["ko"] = "pipeline 배포",
        };
        if (semanticSamples.Any(sample => ForVerification(sample.Key).Resolve(
            new LocalizedText
            {
                Key = "runtime.deploy",
                Fallback = "Deploy pipeline",
            }) != sample.Value))
        {
            throw new InvalidDataException("built-in desktop semantic translation drifted");
        }
        var connectionSamples = new Dictionary<string, string>
        {
            ["en"] = "Leserpent / Connect",
            ["zh-CN"] = "Leserpent / 连接",
            ["zh-TW"] = "Leserpent / 連線",
            ["ja"] = "Leserpent / 接続",
            ["es"] = "Leserpent / Conectar",
            ["de"] = "Leserpent / Verbinden",
            ["fr"] = "Leserpent / Connexion",
            ["ko"] = "Leserpent / 연결",
        };
        if (connectionSamples.Any(sample => DesktopConnectionCatalogs.Resolve(
            ForVerification(sample.Key),
            "title") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop connection translation drifted");
        }
        var bootstrapSamples = new Dictionary<string, string>
        {
            ["en"] = "Leserpent / Deploy daemon",
            ["zh-CN"] = "Leserpent / 部署 daemon",
            ["zh-TW"] = "Leserpent / 部署 daemon",
            ["ja"] = "Leserpent / daemon をデプロイ",
            ["es"] = "Leserpent / Desplegar daemon",
            ["de"] = "Leserpent / Daemon bereitstellen",
            ["fr"] = "Leserpent / Déployer un daemon",
            ["ko"] = "Leserpent / daemon 배포",
        };
        if (bootstrapSamples.Any(sample => DesktopBootstrapDeploymentCatalogs.Resolve(
            ForVerification(sample.Key),
            "title") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop bootstrap translation drifted");
        }
        var provisioningSamples = new Dictionary<string, string>
        {
            ["en"] = "Leserpent / Provision gewyvern",
            ["zh-CN"] = "Leserpent / 部署 gewyvern",
            ["zh-TW"] = "Leserpent / 佈建 gewyvern",
            ["ja"] = "Leserpent / gewyvern をプロビジョニング",
            ["es"] = "Leserpent / Aprovisionar gewyvern",
            ["de"] = "Leserpent / Gewyvern bereitstellen",
            ["fr"] = "Leserpent / Provisionner gewyvern",
            ["ko"] = "Leserpent / gewyvern 프로비저닝",
        };
        if (provisioningSamples.Any(sample => DesktopProvisioningCatalogs.Resolve(
            ForVerification(sample.Key),
            "title") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop provisioning translation drifted");
        }
        var retirementSamples = new Dictionary<string, string>
        {
            ["en"] = "Leserpent / Retire gewyvern",
            ["zh-CN"] = "Leserpent / 退役 gewyvern",
            ["zh-TW"] = "Leserpent / 退役 gewyvern",
            ["ja"] = "Leserpent / gewyvern を廃止",
            ["es"] = "Leserpent / Retirar gewyvern",
            ["de"] = "Leserpent / Gewyvern stilllegen",
            ["fr"] = "Leserpent / Retirer gewyvern",
            ["ko"] = "Leserpent / gewyvern 폐기",
        };
        if (retirementSamples.Any(sample => DesktopRetirementCatalogs.Resolve(
            ForVerification(sample.Key),
            "title") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop retirement translation drifted");
        }
        var daemonRetirementSamples = new Dictionary<string, string>
        {
            ["en"] = "Leserpent / Retire daemon",
            ["zh-CN"] = "Leserpent / 退役 daemon",
            ["zh-TW"] = "Leserpent / 退役 daemon",
            ["ja"] = "Leserpent / daemon を廃止",
            ["es"] = "Leserpent / Retirar daemon",
            ["de"] = "Leserpent / Daemon stilllegen",
            ["fr"] = "Leserpent / Retirer le daemon",
            ["ko"] = "Leserpent / daemon 폐기",
        };
        if (daemonRetirementSamples.Any(sample =>
            DesktopDaemonRetirementCatalogs.Resolve(
                ForVerification(sample.Key),
                "title") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop daemon retirement translation drifted");
        }
        var startupRecoverySamples = new Dictionary<string, string>
        {
            ["en"] = "Remote console could not start",
            ["zh-CN"] = "远程控制台无法启动",
            ["zh-TW"] = "遠端主控台無法啟動",
            ["ja"] = "リモートコンソールを起動できませんでした",
            ["es"] = "No se pudo iniciar la consola remota",
            ["de"] = "Die Remotekonsole konnte nicht gestartet werden",
            ["fr"] = "La console distante n'a pas pu démarrer",
            ["ko"] = "원격 콘솔을 시작할 수 없습니다",
        };
        if (startupRecoverySamples.Any(sample =>
            DesktopStartupRecoveryCatalogs.Resolve(
                ForVerification(sample.Key),
                "heading") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop startup recovery translation drifted");
        }
        var accountSamples = new Dictionary<string, string>
        {
            ["en"] = "Sign in",
            ["zh-CN"] = "登录",
            ["zh-TW"] = "登入",
            ["ja"] = "サインイン",
            ["es"] = "Iniciar sesión",
            ["de"] = "Anmelden",
            ["fr"] = "Se connecter",
            ["ko"] = "로그인",
        };
        if (accountSamples.Any(sample => DesktopAccountCatalogs.Resolve(
            ForVerification(sample.Key),
            "action.signed_out") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop account translation drifted");
        }
        var remoteShellSamples = new Dictionary<string, string>
        {
            ["en"] = "Connecting",
            ["zh-CN"] = "正在连接",
            ["zh-TW"] = "正在連線",
            ["ja"] = "接続中",
            ["es"] = "Conectando",
            ["de"] = "Verbindung wird hergestellt",
            ["fr"] = "Connexion en cours",
            ["ko"] = "연결 중",
        };
        if (remoteShellSamples.Any(sample => DesktopRemoteShellCatalogs.Resolve(
            ForVerification(sample.Key),
            "feed.connecting") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop remote shell translation drifted");
        }
        var remoteOperationSamples = new Dictionary<string, string>
        {
            ["en"] = "Remote operation failed safely: fixture",
            ["zh-CN"] = "远程操作已安全失败：fixture",
            ["zh-TW"] = "遠端操作已安全失敗：fixture",
            ["ja"] = "リモート操作は安全に失敗しました: fixture",
            ["es"] = "La operación remota falló de forma segura: fixture",
            ["de"] = "Remotevorgang ist sicher fehlgeschlagen: fixture",
            ["fr"] = "L’opération distante a échoué de manière sûre : fixture",
            ["ko"] = "원격 작업이 안전하게 실패했습니다: fixture",
        };
        if (remoteOperationSamples.Any(sample =>
            DesktopRemoteOperationCatalogs.Format(
                ForVerification(sample.Key),
                "status.operation_failed",
                "fixture") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop remote operation translation drifted");
        }
        var runtimeWorkspaceSamples = new Dictionary<string, string>
        {
            ["en"] = "Showing 2 of 7 logs",
            ["zh-CN"] = "显示 7 条日志中的 2 条",
            ["zh-TW"] = "顯示 7 筆日誌中的 2 筆",
            ["ja"] = "7 件中 2 件のログを表示",
            ["es"] = "Mostrando 2 de 7 registros",
            ["de"] = "2 von 7 Protokollen werden angezeigt",
            ["fr"] = "Affichage de 2 journaux sur 7",
            ["ko"] = "로그 7개 중 2개 표시",
        };
        if (runtimeWorkspaceSamples.Any(sample =>
            DesktopRuntimeWorkspaceCatalogs.Format(
                ForVerification(sample.Key),
                "filter.some",
                2,
                7) != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop runtime workspace translation drifted");
        }
        var hubSamples = new Dictionary<string, string>
        {
            ["en"] = "Topology refresh complete: 2 daemon authorities live.",
            ["zh-CN"] = "拓扑刷新完成：2 个 daemon 权威端实时可用。",
            ["zh-TW"] = "拓撲重新整理完成：2 個 daemon 權威端即時可用。",
            ["ja"] = "トポロジ更新完了: 2 件の daemon 権限元がライブです。",
            ["es"] = "Actualización de topología completada: 2 autoridades daemon activas.",
            ["de"] = "Topologieaktualisierung abgeschlossen: 2 Daemon-Autoritäten live.",
            ["fr"] = "Actualisation de la topologie terminée : 2 autorités daemon actives.",
            ["ko"] = "토폴로지 새로 고침 완료: daemon 권한 주체 2개 실시간.",
        };
        if (hubSamples.Any(sample => DesktopHubCatalogs.Format(
            ForVerification(sample.Key),
            "status.refresh_complete",
            2) != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop Hub translation drifted");
        }
        var tutorialSamples = new Dictionary<string, string>
        {
            ["en"] = "Read the topology",
            ["zh-CN"] = "读懂拓扑",
            ["zh-TW"] = "讀懂拓撲",
            ["ja"] = "トポロジを読み解く",
            ["es"] = "Leer la topología",
            ["de"] = "Topologie lesen",
            ["fr"] = "Lire la topologie",
            ["ko"] = "토폴로지 이해하기",
        };
        if (tutorialSamples.Any(sample => DesktopTutorialCatalogs.Resolve(
            ForVerification(sample.Key),
            "step.1.title") != sample.Value))
        {
            throw new InvalidDataException(
                "built-in desktop tutorial translation drifted");
        }
        var system = ForVerification(SystemPreference, "zh-Hans-CN");
        if (system.Active.Locale != "zh-CN"
            || ForVerification(SystemPreference, "zh-HK").Active.Locale != "zh-TW"
            || ForVerification(SystemPreference, "nb-NO").Active.Locale != "no"
            || ForVerification(SystemPreference, "unknown").Active.Locale != "en"
            || ForVerification("ar").FlowDirection != FlowDirection.RightToLeft)
        {
            throw new InvalidDataException("desktop system-locale resolution drifted");
        }
        try
        {
            _ = ForVerification("not-official");
        }
        catch (InvalidDataException)
        {
            return;
        }
        throw new InvalidDataException("desktop localization accepted an unsupported locale");
    }

    private static DesktopLocaleDefinition Complete(
        string locale,
        string name,
        string nativeName,
        IReadOnlyDictionary<DesktopTextKey, string> text,
        IReadOnlyDictionary<string, string> semanticText,
        DesktopLocaleCoverage coverage = DesktopLocaleCoverage.Complete) => new(
            locale,
            name,
            nativeName,
            true,
            false,
            coverage,
            text,
            semanticText);

    private static DesktopLocaleDefinition BuiltInShell(
        string locale,
        string name,
        string nativeName,
        IReadOnlyDictionary<DesktopTextKey, string> text,
        IReadOnlyDictionary<string, string> semanticText) => new(
            locale,
            name,
            nativeName,
            true,
            false,
            DesktopLocaleCoverage.Complete,
            text,
            semanticText);

    private static DesktopLocaleDefinition Core(
        string locale,
        string name,
        string nativeName,
        string language,
        string title,
        string subcopy,
        bool builtIn = true,
        bool rightToLeft = false) => new(
            locale,
            name,
            nativeName,
            builtIn,
            rightToLeft,
            DesktopLocaleCoverage.Core,
            new Dictionary<DesktopTextKey, string>
            {
                [DesktopTextKey.Language] = $"{language}...",
                [DesktopTextKey.ControlTopology] = title,
                [DesktopTextKey.HubSubcopy] = subcopy,
            },
            new Dictionary<string, string>(StringComparer.Ordinal));

    private static IReadOnlyDictionary<string, string> MergeSemantic(
        params IReadOnlyDictionary<string, string>[] catalogs)
    {
        var merged = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var catalog in catalogs)
        {
            foreach (var entry in catalog)
            {
                if (!merged.TryAdd(entry.Key, entry.Value))
                {
                    throw new InvalidDataException(
                        $"desktop localization semantic key is duplicated: {entry.Key}");
                }
            }
        }
        return merged;
    }

    private static string CanonicalPreference(string preference) =>
        preference == SystemPreference
            ? SystemPreference
            : LocalesById[preference].Locale;

    private static DesktopLocaleDefinition ResolveActive(string preference, string systemLocale)
    {
        if (preference != SystemPreference)
        {
            return LocalesById[preference];
        }
        foreach (var candidate in LocaleCandidates(systemLocale))
        {
            if (LocalesById.TryGetValue(candidate, out var locale))
            {
                return locale;
            }
        }
        return LocalesById["en"];
    }

    private static IEnumerable<string> LocaleCandidates(string locale)
    {
        var normalized = locale.Trim().Replace('_', '-');
        if (normalized.StartsWith("zh-Hant", StringComparison.OrdinalIgnoreCase)
            || normalized.StartsWith("zh-HK", StringComparison.OrdinalIgnoreCase)
            || normalized.StartsWith("zh-MO", StringComparison.OrdinalIgnoreCase))
        {
            yield return "zh-TW";
            yield break;
        }
        if (normalized.StartsWith("zh", StringComparison.OrdinalIgnoreCase))
        {
            yield return "zh-CN";
            yield break;
        }
        if (normalized.StartsWith("pt", StringComparison.OrdinalIgnoreCase))
        {
            yield return "pt-BR";
        }
        if (normalized.StartsWith("nb", StringComparison.OrdinalIgnoreCase)
            || normalized.StartsWith("nn", StringComparison.OrdinalIgnoreCase))
        {
            yield return "no";
        }
        if (normalized.Length > 0)
        {
            yield return normalized;
            var separator = normalized.IndexOf('-');
            if (separator > 0)
            {
                yield return normalized[..separator];
            }
        }
    }

    private static bool ValidLocale(DesktopLocaleDefinition locale) =>
        locale.Locale.Length is > 0 and <= 35
        && locale.Locale.All(character => char.IsAsciiLetterOrDigit(character) || character == '-')
        && locale.Name.Length is > 0 and <= 96
        && locale.NativeName.Length is > 0 and <= 96
        && !locale.Name.Any(char.IsControl)
        && !locale.NativeName.Any(char.IsControl)
        && locale.Text.All(entry => entry.Value.Length is > 0 and <= 1024
            && !entry.Value.Any(char.IsControl))
        && locale.SemanticText.All(entry => entry.Key.Length is > 0 and <= 128
            && entry.Value.Length is > 0 and <= 1024
            && !entry.Value.Any(char.IsControl));
}
