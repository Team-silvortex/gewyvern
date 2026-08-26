using System.Globalization;

internal static class DesktopOrchestraCatalogs
{
    private const string Prefix = "desktop.orchestra.";
    public const int KeyCount = 41;

    private sealed record Entry(
        string Key,
        int Arity,
        string English,
        string SimplifiedChinese,
        string TraditionalChinese,
        string Japanese,
        string Spanish,
        string German,
        string French,
        string Korean);

    private static readonly Entry[] Entries =
    [
        new("entry.open", 0, "Orchestra", "Orchestra 编排", "Orchestra 編排", "Orchestra", "Orchestra", "Orchestra", "Orchestra", "Orchestra"),
        new("a11y.entry", 0, "Open this daemon's Orchestra workspace", "打开此 daemon 的 Orchestra 工作区", "開啟此 daemon 的 Orchestra 工作區", "この daemon の Orchestra ワークスペースを開く", "Abrir el espacio de trabajo Orchestra de este daemon", "Orchestra-Arbeitsbereich dieses Daemons öffnen", "Ouvrir l’espace Orchestra de ce daemon", "이 daemon의 Orchestra 작업 공간 열기"),
        new("help.entry", 0, "Opens persisted run and event history owned by this daemon.", "打开由此 daemon 管理的持久化运行与事件历史。", "開啟由此 daemon 管理的持久化執行與事件歷史。", "この daemon が管理する永続化済み実行履歴とイベント履歴を開きます。", "Abre el historial persistente de ejecuciones y eventos administrado por este daemon.", "Öffnet den von diesem Daemon verwalteten dauerhaften Ausführungs- und Ereignisverlauf.", "Ouvre l’historique persistant des exécutions et événements géré par ce daemon.", "이 daemon이 관리하는 영구 실행 및 이벤트 기록을 엽니다."),
        new("title", 1, "Orchestra / {0}", "Orchestra 编排 / {0}", "Orchestra 編排 / {0}", "Orchestra / {0}", "Orchestra / {0}", "Orchestra / {0}", "Orchestra / {0}", "Orchestra / {0}"),
        new("heading", 0, "Orchestra history", "Orchestra 编排历史", "Orchestra 編排歷史", "Orchestra 履歴", "Historial de Orchestra", "Orchestra-Verlauf", "Historique Orchestra", "Orchestra 기록"),
        new("body", 0, "Inspect persisted runs and events from this daemon. Plan execution, cancellation, and retry are not available in this desktop slice yet.", "检查此 daemon 持久化的运行与事件。桌面端当前尚未提供计划执行、取消和重试。", "檢查此 daemon 持久化的執行與事件。桌面端目前尚未提供計畫執行、取消和重試。", "この daemon の永続化済み実行とイベントを確認します。プラン実行、キャンセル、再試行はこのデスクトップ機能ではまだ利用できません。", "Inspecciona las ejecuciones y eventos persistentes de este daemon. La ejecución, cancelación y reintento de planes aún no están disponibles aquí.", "Zeigt dauerhafte Ausführungen und Ereignisse dieses Daemons. Planausführung, Abbruch und Wiederholung sind hier noch nicht verfügbar.", "Consulte les exécutions et événements persistants de ce daemon. L’exécution, l’annulation et la relance des plans ne sont pas encore disponibles ici.", "이 daemon의 영구 실행 및 이벤트를 확인합니다. 이 데스크톱 기능에서는 아직 계획 실행, 취소 및 재시도를 사용할 수 없습니다."),
        new("filter.placeholder", 0, "Optional runtime ID", "可选 runtime ID", "可選 runtime ID", "任意の runtime ID", "ID de runtime opcional", "Optionale Runtime-ID", "ID de runtime facultatif", "선택적 runtime ID"),
        new("filter.apply", 0, "Apply filter", "应用筛选", "套用篩選", "フィルターを適用", "Aplicar filtro", "Filter anwenden", "Appliquer le filtre", "필터 적용"),
        new("a11y.filter", 0, "Filter Orchestra runs by exact runtime ID", "按精确 runtime ID 筛选 Orchestra 运行", "依精確 runtime ID 篩選 Orchestra 執行", "正確な runtime ID で Orchestra 実行を絞り込む", "Filtrar ejecuciones de Orchestra por ID de runtime exacto", "Orchestra-Ausführungen nach exakter Runtime-ID filtern", "Filtrer les exécutions Orchestra par ID de runtime exact", "정확한 runtime ID로 Orchestra 실행 필터링"),
        new("help.filter", 0, "Uses an exact, bounded runtime ID. Leave empty to query every runtime owned by this daemon.", "使用精确且有边界的 runtime ID；留空则查询此 daemon 管理的全部 runtime。", "使用精確且有界限的 runtime ID；留空則查詢此 daemon 管理的全部 runtime。", "正確で長さ制限された runtime ID を使用します。空欄なら、この daemon が管理するすべての runtime を照会します。", "Usa un ID de runtime exacto y acotado. Déjalo vacío para consultar todos los runtimes de este daemon.", "Verwendet eine exakte, begrenzte Runtime-ID. Leer lassen, um alle Runtimes dieses Daemons abzufragen.", "Utilise un ID de runtime exact et borné. Laissez vide pour interroger tous les runtimes de ce daemon.", "정확하고 제한된 runtime ID를 사용합니다. 비워 두면 이 daemon의 모든 runtime을 조회합니다."),
        new("action.more_runs", 0, "Load more runs", "加载更多运行", "載入更多執行", "実行をさらに読み込む", "Cargar más ejecuciones", "Weitere Ausführungen laden", "Charger plus d’exécutions", "실행 더 불러오기"),
        new("action.more_events", 0, "Load more events", "加载更多事件", "載入更多事件", "イベントをさらに読み込む", "Cargar más eventos", "Weitere Ereignisse laden", "Charger plus d’événements", "이벤트 더 불러오기"),
        new("action.cleanup", 0, "Clear runtime history", "清理 runtime 历史", "清理 runtime 歷史", "runtime 履歴を消去", "Borrar historial del runtime", "Runtime-Verlauf löschen", "Effacer l’historique du runtime", "runtime 기록 지우기"),
        new("a11y.status", 0, "Orchestra workspace status", "Orchestra 工作区状态", "Orchestra 工作區狀態", "Orchestra ワークスペースの状態", "Estado del espacio de trabajo Orchestra", "Status des Orchestra-Arbeitsbereichs", "État de l’espace Orchestra", "Orchestra 작업 공간 상태"),
        new("a11y.runs", 0, "Persisted Orchestra runs", "持久化的 Orchestra 运行", "持久化的 Orchestra 執行", "永続化済み Orchestra 実行", "Ejecuciones persistentes de Orchestra", "Dauerhafte Orchestra-Ausführungen", "Exécutions Orchestra persistantes", "영구 Orchestra 실행"),
        new("a11y.steps", 0, "Persisted steps for the selected Orchestra run", "所选 Orchestra 运行的持久化步骤", "所選 Orchestra 執行的持久化步驟", "選択した Orchestra 実行の永続化済みステップ", "Pasos persistentes de la ejecución de Orchestra seleccionada", "Dauerhafte Schritte der ausgewählten Orchestra-Ausführung", "Étapes persistantes de l’exécution Orchestra sélectionnée", "선택한 Orchestra 실행의 영구 단계"),
        new("a11y.events", 0, "Events for the selected Orchestra run", "所选 Orchestra 运行的事件", "所選 Orchestra 執行的事件", "選択した Orchestra 実行のイベント", "Eventos de la ejecución de Orchestra seleccionada", "Ereignisse der ausgewählten Orchestra-Ausführung", "Événements de l’exécution Orchestra sélectionnée", "선택한 Orchestra 실행의 이벤트"),
        new("help.cleanup", 0, "Permanently deletes persisted Orchestra runs and events for the selected runtime after explicit confirmation.", "明确确认后，永久删除所选 runtime 的持久化 Orchestra 运行和事件。", "明確確認後，永久刪除所選 runtime 的持久化 Orchestra 執行和事件。", "明示的な確認後、選択した runtime の永続化済み Orchestra 実行とイベントを完全に削除します。", "Elimina permanentemente las ejecuciones y eventos persistentes del runtime seleccionado tras una confirmación explícita.", "Löscht nach ausdrücklicher Bestätigung dauerhaft die Orchestra-Ausführungen und -Ereignisse der ausgewählten Runtime.", "Supprime définitivement les exécutions et événements Orchestra du runtime sélectionné après confirmation explicite.", "명시적으로 확인한 후 선택한 runtime의 영구 Orchestra 실행과 이벤트를 완전히 삭제합니다."),
        new("status.ready", 0, "Ready to load authenticated Orchestra history.", "可以加载已认证的 Orchestra 历史。", "可以載入已驗證的 Orchestra 歷史。", "認証済み Orchestra 履歴を読み込めます。", "Listo para cargar el historial autenticado de Orchestra.", "Bereit zum Laden des authentifizierten Orchestra-Verlaufs.", "Prêt à charger l’historique Orchestra authentifié.", "인증된 Orchestra 기록을 불러올 준비가 되었습니다."),
        new("status.loading_runs", 0, "Loading authenticated Orchestra runs...", "正在加载已认证的 Orchestra 运行...", "正在載入已驗證的 Orchestra 執行...", "認証済み Orchestra 実行を読み込み中...", "Cargando ejecuciones autenticadas de Orchestra...", "Authentifizierte Orchestra-Ausführungen werden geladen...", "Chargement des exécutions Orchestra authentifiées...", "인증된 Orchestra 실행을 불러오는 중..."),
        new("status.runs_loaded", 1, "Loaded {0} persisted run", "已加载 {0} 条持久化运行", "已載入 {0} 筆持久化執行", "永続化済み実行を {0} 件読み込みました", "Se cargaron {0} ejecuciones persistentes", "{0} dauerhafte Ausführungen geladen", "{0} exécutions persistantes chargées", "영구 실행 {0}개를 불러왔습니다"),
        new("status.no_runs", 0, "No persisted Orchestra runs matched this daemon and filter.", "此 daemon 和筛选条件没有匹配的持久化 Orchestra 运行。", "此 daemon 和篩選條件沒有符合的持久化 Orchestra 執行。", "この daemon とフィルターに一致する永続化済み Orchestra 実行はありません。", "No hay ejecuciones persistentes de Orchestra que coincidan con este daemon y filtro.", "Keine dauerhaften Orchestra-Ausführungen entsprechen diesem Daemon und Filter.", "Aucune exécution Orchestra persistante ne correspond à ce daemon et à ce filtre.", "이 daemon 및 필터와 일치하는 영구 Orchestra 실행이 없습니다."),
        new("status.loading_events", 1, "Loading authenticated events for run {0}...", "正在加载运行 {0} 的已认证事件...", "正在載入執行 {0} 的已驗證事件...", "実行 {0} の認証済みイベントを読み込み中...", "Cargando eventos autenticados de la ejecución {0}...", "Authentifizierte Ereignisse für Ausführung {0} werden geladen...", "Chargement des événements authentifiés de l’exécution {0}...", "실행 {0}의 인증된 이벤트를 불러오는 중..."),
        new("status.events_loaded", 1, "Loaded {0} persisted event", "已加载 {0} 条持久化事件", "已載入 {0} 筆持久化事件", "永続化済みイベントを {0} 件読み込みました", "Se cargaron {0} eventos persistentes", "{0} dauerhafte Ereignisse geladen", "{0} événements persistants chargés", "영구 이벤트 {0}개를 불러왔습니다"),
        new("status.no_events", 0, "No persisted events are available for this run.", "此运行没有可用的持久化事件。", "此執行沒有可用的持久化事件。", "この実行には永続化済みイベントがありません。", "No hay eventos persistentes disponibles para esta ejecución.", "Für diese Ausführung sind keine dauerhaften Ereignisse verfügbar.", "Aucun événement persistant n’est disponible pour cette exécution.", "이 실행에는 영구 이벤트가 없습니다."),
        new("status.failed_rejected", 1, "Orchestra request rejected ({0}).", "Orchestra 请求被拒绝（{0}）。", "Orchestra 請求遭拒絕（{0}）。", "Orchestra リクエストが拒否されました ({0})。", "Solicitud de Orchestra rechazada ({0}).", "Orchestra-Anfrage abgelehnt ({0}).", "Requête Orchestra rejetée ({0}).", "Orchestra 요청이 거부되었습니다({0})."),
        new("status.failed_response", 0, "The Orchestra response failed strict validation; no partial data was retained.", "Orchestra 响应未通过严格校验；未保留任何部分数据。", "Orchestra 回應未通過嚴格驗證；未保留任何部分資料。", "Orchestra 応答が厳密な検証に失敗したため、部分データは保持されませんでした。", "La respuesta de Orchestra no superó la validación estricta; no se conservaron datos parciales.", "Die Orchestra-Antwort bestand die strenge Prüfung nicht; Teildaten wurden nicht übernommen.", "La réponse Orchestra a échoué à la validation stricte ; aucune donnée partielle n’a été conservée.", "Orchestra 응답이 엄격한 검증을 통과하지 못해 부분 데이터를 유지하지 않았습니다."),
        new("status.failed_transport", 0, "The authenticated Orchestra connection failed.", "已认证的 Orchestra 连接失败。", "已驗證的 Orchestra 連線失敗。", "認証済み Orchestra 接続に失敗しました。", "Falló la conexión autenticada de Orchestra.", "Die authentifizierte Orchestra-Verbindung ist fehlgeschlagen.", "La connexion Orchestra authentifiée a échoué.", "인증된 Orchestra 연결에 실패했습니다."),
        new("status.cleanup_loading", 1, "Clearing persisted Orchestra history for runtime {0}...", "正在清理 runtime {0} 的持久化 Orchestra 历史...", "正在清理 runtime {0} 的持久化 Orchestra 歷史...", "runtime {0} の永続化済み Orchestra 履歴を消去中...", "Borrando el historial persistente de Orchestra del runtime {0}...", "Dauerhafter Orchestra-Verlauf für Runtime {0} wird gelöscht...", "Effacement de l’historique Orchestra persistant du runtime {0}...", "runtime {0}의 영구 Orchestra 기록을 지우는 중..."),
        new("status.cleanup_completed", 2, "Cleanup committed: {0} run and {1} event deleted.", "清理已提交：删除 {0} 条运行和 {1} 条事件。", "清理已提交：刪除 {0} 筆執行和 {1} 筆事件。", "消去をコミットしました: 実行 {0} 件、イベント {1} 件を削除しました。", "Limpieza confirmada: se eliminaron {0} ejecuciones y {1} eventos.", "Bereinigung bestätigt: {0} Ausführungen und {1} Ereignisse gelöscht.", "Nettoyage validé : {0} exécutions et {1} événements supprimés.", "정리가 커밋되었습니다. 실행 {0}개와 이벤트 {1}개를 삭제했습니다."),
        new("run.label", 4, "{0} / {1} / {2} / attempt {3}", "{0} / {1} / {2} / 第 {3} 次尝试", "{0} / {1} / {2} / 第 {3} 次嘗試", "{0} / {1} / {2} / 試行 {3}", "{0} / {1} / {2} / intento {3}", "{0} / {1} / {2} / Versuch {3}", "{0} / {1} / {2} / tentative {3}", "{0} / {1} / {2} / 시도 {3}"),
        new("run.detail", 3, "Run {0} / executed {1} / completed {2}", "运行 {0} / 执行于 {1} / 完成于 {2}", "執行 {0} / 執行於 {1} / 完成於 {2}", "実行 {0} / 開始 {1} / 完了 {2}", "Ejecución {0} / iniciada {1} / completada {2}", "Ausführung {0} / gestartet {1} / abgeschlossen {2}", "Exécution {0} / lancée {1} / terminée {2}", "실행 {0} / 시작 {1} / 완료 {2}"),
        new("step.label", 3, "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}"),
        new("event.label", 5, "{0} / {1} / {2} -> {3} / {4}", "{0} / {1} / {2} -> {3} / {4}", "{0} / {1} / {2} -> {3} / {4}", "{0} / {1} / {2} -> {3} / {4}", "{0} / {1} / {2} -> {3} / {4}", "{0} / {1} / {2} -> {3} / {4}", "{0} / {1} / {2} -> {3} / {4}", "{0} / {1} / {2} -> {3} / {4}"),
        new("events.heading", 1, "Events / {0}", "事件 / {0}", "事件 / {0}", "イベント / {0}", "Eventos / {0}", "Ereignisse / {0}", "Événements / {0}", "이벤트 / {0}"),
        new("selection.none", 0, "Select a run to inspect its persisted steps and events.", "选择一条运行以检查其持久化步骤和事件。", "選擇一筆執行以檢查其持久化步驟和事件。", "実行を選択して、永続化済みステップとイベントを確認してください。", "Selecciona una ejecución para inspeccionar sus pasos y eventos persistentes.", "Wählen Sie eine Ausführung, um ihre dauerhaften Schritte und Ereignisse zu prüfen.", "Sélectionnez une exécution pour consulter ses étapes et événements persistants.", "실행을 선택하여 영구 단계와 이벤트를 확인하세요."),
        new("cleanup.title", 0, "Confirm Orchestra history cleanup", "确认清理 Orchestra 历史", "確認清理 Orchestra 歷史", "Orchestra 履歴の消去を確認", "Confirmar limpieza del historial de Orchestra", "Löschen des Orchestra-Verlaufs bestätigen", "Confirmer le nettoyage de l’historique Orchestra", "Orchestra 기록 정리 확인"),
        new("cleanup.body", 1, "Permanently delete every persisted Orchestra run and event for runtime {0}?", "永久删除 runtime {0} 的全部持久化 Orchestra 运行和事件？", "永久刪除 runtime {0} 的全部持久化 Orchestra 執行和事件？", "runtime {0} の永続化済み Orchestra 実行とイベントをすべて完全に削除しますか？", "¿Eliminar permanentemente todas las ejecuciones y eventos persistentes de Orchestra del runtime {0}?", "Alle dauerhaften Orchestra-Ausführungen und -Ereignisse für Runtime {0} endgültig löschen?", "Supprimer définitivement toutes les exécutions et tous les événements Orchestra persistants du runtime {0} ?", "runtime {0}의 모든 영구 Orchestra 실행과 이벤트를 완전히 삭제할까요?"),
        new("cleanup.warning", 0, "This deletion cannot be undone. Other runtimes are not affected.", "此删除无法撤销，其他 runtime 不受影响。", "此刪除無法復原，其他 runtime 不受影響。", "この削除は元に戻せません。他の runtime には影響しません。", "Esta eliminación no se puede deshacer. Los demás runtimes no se verán afectados.", "Diese Löschung kann nicht rückgängig gemacht werden. Andere Runtimes sind nicht betroffen.", "Cette suppression est irréversible. Les autres runtimes ne sont pas affectés.", "이 삭제는 실행 취소할 수 없으며 다른 runtime에는 영향을 주지 않습니다."),
        new("cleanup.confirm", 0, "Delete history", "删除历史", "刪除歷史", "履歴を削除", "Eliminar historial", "Verlauf löschen", "Supprimer l’historique", "기록 삭제"),
        new("filter.invalid", 0, "Enter a valid runtime ID using letters, numbers, dot, dash, underscore, or colon.", "请输入仅含字母、数字、点、短横线、下划线或冒号的有效 runtime ID。", "請輸入僅含字母、數字、點、連字號、底線或冒號的有效 runtime ID。", "英数字、ピリオド、ハイフン、アンダースコア、コロンを使った有効な runtime ID を入力してください。", "Introduce un ID de runtime válido con letras, números, punto, guion, guion bajo o dos puntos.", "Geben Sie eine gültige Runtime-ID mit Buchstaben, Zahlen, Punkt, Bindestrich, Unterstrich oder Doppelpunkt ein.", "Saisissez un ID de runtime valide avec lettres, chiffres, point, tiret, soulignement ou deux-points.", "문자, 숫자, 점, 대시, 밑줄 또는 콜론으로 된 올바른 runtime ID를 입력하세요."),
    ];

    private static readonly IReadOnlyDictionary<string, string> English =
        Catalog(entry => entry.English);
    public static readonly IReadOnlyDictionary<string, string> SimplifiedChinese =
        Catalog(entry => entry.SimplifiedChinese);
    public static readonly IReadOnlyDictionary<string, string> TraditionalChinese =
        Catalog(entry => entry.TraditionalChinese);
    public static readonly IReadOnlyDictionary<string, string> Japanese =
        Catalog(entry => entry.Japanese);
    public static readonly IReadOnlyDictionary<string, string> Spanish =
        Catalog(entry => entry.Spanish);
    public static readonly IReadOnlyDictionary<string, string> German =
        Catalog(entry => entry.German);
    public static readonly IReadOnlyDictionary<string, string> French =
        Catalog(entry => entry.French);
    public static readonly IReadOnlyDictionary<string, string> Korean =
        Catalog(entry => entry.Korean);

    public static string Resolve(DesktopLocalization localization, string key)
    {
        var fullKey = FullKey(key);
        if (!English.ContainsKey(fullKey))
        {
            throw new InvalidDataException(
                $"desktop Orchestra localization key is unknown: {key}");
        }
        return localization.Resolve(new LocalizedText
        {
            Key = fullKey,
            Fallback = English[fullKey],
        });
    }

    public static string Format(
        DesktopLocalization localization,
        string key,
        params object[] values) => string.Format(
            CultureInfo.InvariantCulture,
            Resolve(localization, key),
            values);

    public static void VerifyContract()
    {
        if (Entries.Length != KeyCount
            || Entries.Select(entry => entry.Key).Distinct(StringComparer.Ordinal).Count()
                != KeyCount)
        {
            throw new InvalidDataException(
                "desktop Orchestra localization entries drifted");
        }
        DesktopDomainCatalogContract.Verify(
            "Orchestra",
            KeyCount,
            All,
            Entries.Where(entry => entry.Arity > 0).ToDictionary(
                entry => FullKey(entry.Key),
                entry => entry.Arity,
                StringComparer.Ordinal));
    }

    private static IReadOnlyList<IReadOnlyDictionary<string, string>> All =>
        [English, SimplifiedChinese, TraditionalChinese, Japanese, Spanish, German, French, Korean];

    private static IReadOnlyDictionary<string, string> Catalog(
        Func<Entry, string> value) => Entries.ToDictionary(
            entry => FullKey(entry.Key),
            value,
            StringComparer.Ordinal);

    private static string FullKey(string key) => $"{Prefix}{key}";
}
