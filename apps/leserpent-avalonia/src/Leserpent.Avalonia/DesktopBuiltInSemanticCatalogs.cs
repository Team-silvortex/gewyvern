internal static class DesktopBuiltInSemanticCatalogs
{
    public const int KeyCount = 26;

    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["remote.title"] = "遠端 runtimes",
            ["remote.fleet"] = "遠端 runtime fleet",
            ["remote.filter.empty"] = "目前篩選條件沒有匹配的 runtime",
            ["runtime.inspect"] = "檢查 runtime",
            ["runtime.inspect.description"] = "開啟唯讀的 runtime 工作區",
            ["runtime.refresh"] = "刷新 runtime",
            ["runtime.refresh.description"] = "變更遠端狀態前需要明確確認",
            ["runtime.capabilities.refresh"] = "刷新能力",
            ["runtime.capabilities.refresh.description"] = "查詢遠端 runtime 前需要明確確認",
            ["runtime.workspace.refresh"] = "刷新",
            ["runtime.workspace.refresh.description"] = "需要在 fleet 視窗中確認",
            ["runtime.history.empty"] = "沒有已套用的命令",
            ["runtime.history.title"] = "歷史記錄",
            ["runtime.logs.empty"] = "沒有日誌記錄",
            ["runtime.logs.filtered_empty"] = "沒有匹配的日誌記錄",
            ["runtime.logs.title"] = "日誌",
            ["runtime.capabilities.unobserved"] = "尚未探測能力",
            ["runtime.capabilities.title"] = "能力",
            ["runtime.deploy"] = "部署 pipeline",
            ["runtime.deploy.description"] = "開啟受限的部署表單，並要求明確確認",
            ["runtime.deploy.form.title"] = "確認遠端部署",
            ["runtime.deploy.form.submit"] = "部署 pipeline",
            ["runtime.deploy.form.pipeline_kind"] = "Pipeline 類型",
            ["runtime.deploy.form.pipeline_kind.placeholder"] = "http/request",
            ["runtime.deploy.form.target"] = "選用 target",
            ["runtime.deploy.form.target.placeholder"] = "例如 pid:42",
        });

    public static IReadOnlyDictionary<string, string> Japanese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["remote.title"] = "リモート runtimes",
            ["remote.fleet"] = "リモート runtime fleet",
            ["remote.filter.empty"] = "現在のフィルターに一致する runtime はありません",
            ["runtime.inspect"] = "runtime を確認",
            ["runtime.inspect.description"] = "読み取り専用の runtime ワークスペースを開く",
            ["runtime.refresh"] = "runtime を更新",
            ["runtime.refresh.description"] = "リモート状態を変更する前に明示的な確認が必要です",
            ["runtime.capabilities.refresh"] = "能力を更新",
            ["runtime.capabilities.refresh.description"] = "リモート runtime を照会する前に明示的な確認が必要です",
            ["runtime.workspace.refresh"] = "更新",
            ["runtime.workspace.refresh.description"] = "fleet ウィンドウでの確認が必要です",
            ["runtime.history.empty"] = "適用済みコマンドはありません",
            ["runtime.history.title"] = "履歴",
            ["runtime.logs.empty"] = "ログ記録はありません",
            ["runtime.logs.filtered_empty"] = "一致するログ記録はありません",
            ["runtime.logs.title"] = "ログ",
            ["runtime.capabilities.unobserved"] = "能力はまだ取得されていません",
            ["runtime.capabilities.title"] = "能力",
            ["runtime.deploy"] = "pipeline をデプロイ",
            ["runtime.deploy.description"] = "制限付きのデプロイフォームを開き、明示的な確認を求めます",
            ["runtime.deploy.form.title"] = "リモートデプロイを確認",
            ["runtime.deploy.form.submit"] = "pipeline をデプロイ",
            ["runtime.deploy.form.pipeline_kind"] = "Pipeline の種類",
            ["runtime.deploy.form.pipeline_kind.placeholder"] = "http/request",
            ["runtime.deploy.form.target"] = "任意の target",
            ["runtime.deploy.form.target.placeholder"] = "例: pid:42",
        });

    public static IReadOnlyDictionary<string, string> Spanish { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["remote.title"] = "Runtimes remotos",
            ["remote.fleet"] = "Flota de runtimes remotos",
            ["remote.filter.empty"] = "Ningún runtime coincide con el filtro actual",
            ["runtime.inspect"] = "Inspeccionar runtime",
            ["runtime.inspect.description"] = "Abrir el espacio de trabajo del runtime en modo de solo lectura",
            ["runtime.refresh"] = "Actualizar runtime",
            ["runtime.refresh.description"] = "Se requiere confirmación explícita antes de cambiar el estado remoto",
            ["runtime.capabilities.refresh"] = "Actualizar capacidades",
            ["runtime.capabilities.refresh.description"] = "Se requiere confirmación explícita antes de consultar el runtime remoto",
            ["runtime.workspace.refresh"] = "Actualizar",
            ["runtime.workspace.refresh.description"] = "Requiere confirmación en la ventana del fleet",
            ["runtime.history.empty"] = "No hay comandos aplicados",
            ["runtime.history.title"] = "Historial",
            ["runtime.logs.empty"] = "No hay registros",
            ["runtime.logs.filtered_empty"] = "No hay registros que coincidan",
            ["runtime.logs.title"] = "Registros",
            ["runtime.capabilities.unobserved"] = "Las capacidades aún no se han consultado",
            ["runtime.capabilities.title"] = "Capacidades",
            ["runtime.deploy"] = "Desplegar pipeline",
            ["runtime.deploy.description"] = "Abrir un formulario de despliegue acotado y solicitar confirmación explícita",
            ["runtime.deploy.form.title"] = "Confirmar despliegue remoto",
            ["runtime.deploy.form.submit"] = "Desplegar pipeline",
            ["runtime.deploy.form.pipeline_kind"] = "Tipo de pipeline",
            ["runtime.deploy.form.pipeline_kind.placeholder"] = "http/request",
            ["runtime.deploy.form.target"] = "Destino opcional",
            ["runtime.deploy.form.target.placeholder"] = "p. ej., pid:42",
        });

    public static IReadOnlyDictionary<string, string> German { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["remote.title"] = "Remote-Runtimes",
            ["remote.fleet"] = "Remote-Runtime-Flotte",
            ["remote.filter.empty"] = "Keine Runtime entspricht dem aktuellen Filter",
            ["runtime.inspect"] = "Runtime prüfen",
            ["runtime.inspect.description"] = "Schreibgeschützten Runtime-Arbeitsbereich öffnen",
            ["runtime.refresh"] = "Runtime aktualisieren",
            ["runtime.refresh.description"] = "Vor Änderungen am Remote-Status ist eine ausdrückliche Bestätigung erforderlich",
            ["runtime.capabilities.refresh"] = "Fähigkeiten aktualisieren",
            ["runtime.capabilities.refresh.description"] = "Vor der Abfrage der Remote-Runtime ist eine ausdrückliche Bestätigung erforderlich",
            ["runtime.workspace.refresh"] = "Aktualisieren",
            ["runtime.workspace.refresh.description"] = "Bestätigung im Flottenfenster erforderlich",
            ["runtime.history.empty"] = "Keine angewendeten Befehle",
            ["runtime.history.title"] = "Verlauf",
            ["runtime.logs.empty"] = "Keine Protokolleinträge",
            ["runtime.logs.filtered_empty"] = "Keine passenden Protokolleinträge",
            ["runtime.logs.title"] = "Protokolle",
            ["runtime.capabilities.unobserved"] = "Fähigkeiten noch nicht abgefragt",
            ["runtime.capabilities.title"] = "Fähigkeiten",
            ["runtime.deploy"] = "Pipeline bereitstellen",
            ["runtime.deploy.description"] = "Begrenztes Bereitstellungsformular öffnen und ausdrückliche Bestätigung anfordern",
            ["runtime.deploy.form.title"] = "Remote-Bereitstellung bestätigen",
            ["runtime.deploy.form.submit"] = "Pipeline bereitstellen",
            ["runtime.deploy.form.pipeline_kind"] = "Pipeline-Typ",
            ["runtime.deploy.form.pipeline_kind.placeholder"] = "http/request",
            ["runtime.deploy.form.target"] = "Optionales Ziel",
            ["runtime.deploy.form.target.placeholder"] = "z. B. pid:42",
        });

    public static IReadOnlyDictionary<string, string> French { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["remote.title"] = "Runtimes distants",
            ["remote.fleet"] = "Flotte de runtimes distants",
            ["remote.filter.empty"] = "Aucun runtime ne correspond au filtre actuel",
            ["runtime.inspect"] = "Inspecter le runtime",
            ["runtime.inspect.description"] = "Ouvrir l’espace de travail runtime en lecture seule",
            ["runtime.refresh"] = "Rafraîchir le runtime",
            ["runtime.refresh.description"] = "Une confirmation explicite est requise avant de modifier l’état distant",
            ["runtime.capabilities.refresh"] = "Rafraîchir les capacités",
            ["runtime.capabilities.refresh.description"] = "Une confirmation explicite est requise avant d’interroger le runtime distant",
            ["runtime.workspace.refresh"] = "Rafraîchir",
            ["runtime.workspace.refresh.description"] = "Une confirmation est requise dans la fenêtre du fleet",
            ["runtime.history.empty"] = "Aucune commande appliquée",
            ["runtime.history.title"] = "Historique",
            ["runtime.logs.empty"] = "Aucune entrée de journal",
            ["runtime.logs.filtered_empty"] = "Aucune entrée de journal correspondante",
            ["runtime.logs.title"] = "Journaux",
            ["runtime.capabilities.unobserved"] = "Les capacités n’ont pas encore été interrogées",
            ["runtime.capabilities.title"] = "Capacités",
            ["runtime.deploy"] = "Déployer le pipeline",
            ["runtime.deploy.description"] = "Ouvrir un formulaire de déploiement encadré et demander une confirmation explicite",
            ["runtime.deploy.form.title"] = "Confirmer le déploiement distant",
            ["runtime.deploy.form.submit"] = "Déployer le pipeline",
            ["runtime.deploy.form.pipeline_kind"] = "Type de pipeline",
            ["runtime.deploy.form.pipeline_kind.placeholder"] = "http/request",
            ["runtime.deploy.form.target"] = "Cible facultative",
            ["runtime.deploy.form.target.placeholder"] = "p. ex. pid:42",
        });

    public static IReadOnlyDictionary<string, string> Korean { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["remote.title"] = "원격 runtime",
            ["remote.fleet"] = "원격 runtime fleet",
            ["remote.filter.empty"] = "현재 필터와 일치하는 runtime이 없습니다",
            ["runtime.inspect"] = "runtime 검사",
            ["runtime.inspect.description"] = "읽기 전용 runtime 작업 영역 열기",
            ["runtime.refresh"] = "runtime 새로고침",
            ["runtime.refresh.description"] = "원격 상태를 변경하려면 명시적 확인이 필요합니다",
            ["runtime.capabilities.refresh"] = "기능 새로고침",
            ["runtime.capabilities.refresh.description"] = "원격 runtime을 조회하려면 명시적 확인이 필요합니다",
            ["runtime.workspace.refresh"] = "새로고침",
            ["runtime.workspace.refresh.description"] = "fleet 창에서 확인해야 합니다",
            ["runtime.history.empty"] = "적용된 명령이 없습니다",
            ["runtime.history.title"] = "기록",
            ["runtime.logs.empty"] = "로그 기록이 없습니다",
            ["runtime.logs.filtered_empty"] = "일치하는 로그 기록이 없습니다",
            ["runtime.logs.title"] = "로그",
            ["runtime.capabilities.unobserved"] = "기능을 아직 조회하지 않았습니다",
            ["runtime.capabilities.title"] = "기능",
            ["runtime.deploy"] = "pipeline 배포",
            ["runtime.deploy.description"] = "범위가 제한된 배포 양식을 열고 명시적 확인 요청",
            ["runtime.deploy.form.title"] = "원격 배포 확인",
            ["runtime.deploy.form.submit"] = "pipeline 배포",
            ["runtime.deploy.form.pipeline_kind"] = "Pipeline 유형",
            ["runtime.deploy.form.pipeline_kind.placeholder"] = "http/request",
            ["runtime.deploy.form.target"] = "선택적 target",
            ["runtime.deploy.form.target.placeholder"] = "예: pid:42",
        });

    public static void VerifyContract(IEnumerable<string> expectedKeys)
    {
        var expected = expectedKeys.ToHashSet(StringComparer.Ordinal);
        if (expected.Count != KeyCount)
        {
            throw new InvalidDataException("desktop semantic key contract drifted");
        }
        foreach (var catalog in All)
        {
            if (catalog.Count != KeyCount
                || !catalog.Keys.ToHashSet(StringComparer.Ordinal).SetEquals(expected)
                || catalog.Any(entry => entry.Key.Length is <= 0 or > 128
                    || entry.Value.Length is <= 0 or > 1024
                    || entry.Key.Any(char.IsControl)
                    || entry.Value.Any(char.IsControl)))
            {
                throw new InvalidDataException(
                    "built-in desktop semantic catalog is incomplete");
            }
        }
    }

    private static IReadOnlyList<IReadOnlyDictionary<string, string>> All =>
        [TraditionalChinese, Japanese, Spanish, German, French, Korean];

    private static IReadOnlyDictionary<string, string> Catalog(
        Dictionary<string, string> values) => values;
}
