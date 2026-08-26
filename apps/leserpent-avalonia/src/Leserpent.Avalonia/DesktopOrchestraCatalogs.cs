using System.Globalization;

internal static class DesktopOrchestraCatalogs
{
    private const string Prefix = "desktop.orchestra.";
    public const int KeyCount = 72;

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
        new("help.entry", 0, "Opens native plans, controls, and persisted history owned by this daemon.", "打开由此 daemon 管理的原生计划、控制与持久化历史。", "開啟由此 daemon 管理的原生計畫、控制與持久化歷史。", "この daemon が管理するネイティブプラン、制御、永続履歴を開きます。", "Abre los planes nativos, controles e historial persistente de este daemon.", "Öffnet native Pläne, Steuerungen und den dauerhaften Verlauf dieses Daemons.", "Ouvre les plans natifs, les contrôles et l’historique persistant de ce daemon.", "이 daemon이 관리하는 네이티브 계획, 제어 및 영구 기록을 엽니다."),
        new("title", 1, "Orchestra / {0}", "Orchestra 编排 / {0}", "Orchestra 編排 / {0}", "Orchestra / {0}", "Orchestra / {0}", "Orchestra / {0}", "Orchestra / {0}", "Orchestra / {0}"),
        new("heading", 0, "Orchestra control", "Orchestra 编排控制", "Orchestra 編排控制", "Orchestra 制御", "Control de Orchestra", "Orchestra-Steuerung", "Contrôle Orchestra", "Orchestra 제어"),
        new("body", 0, "Run Rust-authoritative automatic plans and inspect durable history. Guided plans remain review-only; cancellation safely stops queued work only.", "执行由 Rust 权威控制的自动计划并检查持久化历史。引导计划仍为只读审查；取消只安全停止仍在队列中的工作。", "執行由 Rust 權威控制的自動計畫並檢查持久化歷史。引導計畫仍為唯讀審查；取消只安全停止仍在佇列中的工作。", "Rust が権威を持つ自動プランを実行し、永続履歴を確認します。ガイド付きプランはレビュー専用で、キャンセルはキュー内の処理だけを安全に停止します。", "Ejecuta planes automáticos con autoridad Rust e inspecciona el historial duradero. Los planes guiados son solo de revisión; la cancelación solo detiene trabajo en cola.", "Führt Rust-autorisierte automatische Pläne aus und zeigt den dauerhaften Verlauf. Geführte Pläne bleiben schreibgeschützt; Abbruch stoppt nur wartende Arbeit.", "Exécute les plans automatiques sous autorité Rust et consulte l’historique durable. Les plans guidés restent en lecture seule ; l’annulation n’arrête que le travail en file.", "Rust 권한의 자동 계획을 실행하고 영구 기록을 확인합니다. 안내 계획은 검토 전용이며 취소는 대기 중인 작업만 안전하게 중지합니다."),
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
        new("plans.heading", 0, "Native plans", "原生计划", "原生計畫", "ネイティブプラン", "Planes nativos", "Native Pläne", "Plans natifs", "네이티브 계획"),
        new("catalog.detail", 5, "{0} / revision {1} / {2} / source {3} / {4}", "{0} / 修订 {1} / {2} / 来源 {3} / {4}", "{0} / 修訂 {1} / {2} / 來源 {3} / {4}", "{0} / リビジョン {1} / {2} / ソース {3} / {4}", "{0} / revisión {1} / {2} / fuente {3} / {4}", "{0} / Revision {1} / {2} / Quelle {3} / {4}", "{0} / révision {1} / {2} / source {3} / {4}", "{0} / 리비전 {1} / {2} / 소스 {3} / {4}"),
        new("plans.none", 0, "Select a run or enter an exact runtime ID to load native plans.", "选择一条运行或输入精确 runtime ID 以加载原生计划。", "選擇一筆執行或輸入精確 runtime ID 以載入原生計畫。", "実行を選択するか正確な runtime ID を入力してネイティブプランを読み込んでください。", "Selecciona una ejecución o introduce un ID de runtime exacto para cargar planes nativos.", "Wählen Sie eine Ausführung oder geben Sie eine exakte Runtime-ID ein, um native Pläne zu laden.", "Sélectionnez une exécution ou saisissez un ID de runtime exact pour charger les plans natifs.", "실행을 선택하거나 정확한 runtime ID를 입력해 네이티브 계획을 불러오세요."),
        new("plan.label", 4, "{0} / {1} risk / {2} / {3}", "{0} / {1} 风险 / {2} / {3}", "{0} / {1} 風險 / {2} / {3}", "{0} / リスク {1} / {2} / {3}", "{0} / riesgo {1} / {2} / {3}", "{0} / Risiko {1} / {2} / {3}", "{0} / risque {1} / {2} / {3}", "{0} / 위험 {1} / {2} / {3}"),
        new("plan.detail", 4, "{0} / {1} / approval {2} / revision {3}", "{0} / {1} / 审批 {2} / 修订 {3}", "{0} / {1} / 核准 {2} / 修訂 {3}", "{0} / {1} / 承認 {2} / リビジョン {3}", "{0} / {1} / aprobación {2} / revisión {3}", "{0} / {1} / Freigabe {2} / Revision {3}", "{0} / {1} / approbation {2} / révision {3}", "{0} / {1} / 승인 {2} / 리비전 {3}"),
        new("plan.step", 3, "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}", "{0} / {1} / {2}"),
        new("approval.placeholder", 0, "Approval note (required when indicated)", "审批说明（标明需要时必填）", "核准說明（標明需要時必填）", "承認メモ（必要と表示された場合は必須）", "Nota de aprobación (obligatoria cuando se indique)", "Freigabehinweis (falls angegeben erforderlich)", "Note d’approbation (requise si indiqué)", "승인 메모(필요하다고 표시된 경우 필수)"),
        new("a11y.plans", 0, "Native Orchestra plans for the selected runtime", "所选 runtime 的原生 Orchestra 计划", "所選 runtime 的原生 Orchestra 計畫", "選択した runtime のネイティブ Orchestra プラン", "Planes nativos de Orchestra para el runtime seleccionado", "Native Orchestra-Pläne für die ausgewählte Runtime", "Plans Orchestra natifs pour le runtime sélectionné", "선택한 runtime의 네이티브 Orchestra 계획"),
        new("a11y.plan_steps", 0, "Steps in the selected native plan", "所选原生计划的步骤", "所選原生計畫的步驟", "選択したネイティブプランのステップ", "Pasos del plan nativo seleccionado", "Schritte im ausgewählten nativen Plan", "Étapes du plan natif sélectionné", "선택한 네이티브 계획의 단계"),
        new("a11y.approval", 0, "Bounded Orchestra approval note", "有边界的 Orchestra 审批说明", "有界限的 Orchestra 核准說明", "長さ制限された Orchestra 承認メモ", "Nota de aprobación acotada de Orchestra", "Begrenzter Orchestra-Freigabehinweis", "Note d’approbation Orchestra bornée", "길이가 제한된 Orchestra 승인 메모"),
        new("help.approval", 0, "At most 500 characters. Required only for plans marked operator_confirmation; it is persisted in the audit trail.", "最多 500 个字符。仅标记为 operator_confirmation 的计划必填，并会持久化到审计记录。", "最多 500 個字元。僅標記為 operator_confirmation 的計畫必填，並會持久化到稽核記錄。", "最大 500 文字です。operator_confirmation のプランでのみ必須で、監査履歴に保存されます。", "Máximo 500 caracteres. Solo es obligatoria para planes operator_confirmation y se conserva en la auditoría.", "Höchstens 500 Zeichen. Nur für operator_confirmation-Pläne erforderlich und im Audit gespeichert.", "500 caractères maximum. Requise uniquement pour operator_confirmation et conservée dans l’audit.", "최대 500자입니다. operator_confirmation 계획에만 필요하며 감사 기록에 저장됩니다."),
        new("action.run", 0, "Run plan", "执行计划", "執行計畫", "プランを実行", "Ejecutar plan", "Plan ausführen", "Exécuter le plan", "계획 실행"),
        new("action.cancel", 0, "Cancel queued run", "取消排队运行", "取消佇列執行", "キュー内の実行をキャンセル", "Cancelar ejecución en cola", "Wartende Ausführung abbrechen", "Annuler l’exécution en file", "대기 실행 취소"),
        new("action.retry", 0, "Retry run", "重试运行", "重試執行", "実行を再試行", "Reintentar ejecución", "Ausführung wiederholen", "Relancer l’exécution", "실행 재시도"),
        new("help.run", 0, "Executes only a ready automatic plan at its exact revision through Rust authority.", "仅通过 Rust 权威执行精确修订版本的就绪自动计划。", "僅透過 Rust 權威執行精確修訂版本的就緒自動計畫。", "正確なリビジョンの実行可能な自動プランだけを Rust 権威で実行します。", "Solo ejecuta un plan automático listo en su revisión exacta mediante la autoridad Rust.", "Führt nur einen bereiten automatischen Plan in seiner exakten Revision über die Rust-Autorität aus.", "Exécute uniquement un plan automatique prêt à sa révision exacte via l’autorité Rust.", "정확한 리비전의 준비된 자동 계획만 Rust 권한으로 실행합니다."),
        new("help.cancel", 0, "Requires confirmation and safely cancels only work that is still queued; leased adapter work is not claimed as preempted.", "需要确认，且只安全取消仍在队列中的工作；不会声称已抢占被适配器租约的工作。", "需要確認，且只安全取消仍在佇列中的工作；不會聲稱已搶占被介面卡租約的工作。", "確認が必要で、キュー内の処理だけを安全にキャンセルします。アダプターが取得済みの処理を停止したとは表示しません。", "Requiere confirmación y solo cancela trabajo aún en cola; no afirma interrumpir trabajo ya arrendado.", "Erfordert Bestätigung und bricht nur wartende Arbeit sicher ab; bereits geleaste Adapterarbeit gilt nicht als unterbrochen.", "Nécessite une confirmation et n’annule que le travail encore en file ; le travail déjà loué n’est pas déclaré interrompu.", "확인이 필요하며 아직 대기 중인 작업만 안전하게 취소합니다. 이미 임대된 어댑터 작업을 중단했다고 주장하지 않습니다."),
        new("help.retry", 0, "Creates a new durable attempt from a terminal run using the current matching plan revision.", "使用当前匹配的计划修订，从终态运行创建新的持久化尝试。", "使用目前相符的計畫修訂，從終態執行建立新的持久化嘗試。", "終端状態の実行から、現在一致するプランリビジョンで新しい永続試行を作成します。", "Crea un nuevo intento duradero desde una ejecución terminal con la revisión actual coincidente.", "Erstellt aus einer terminalen Ausführung einen neuen dauerhaften Versuch mit der passenden aktuellen Planrevision.", "Crée une nouvelle tentative durable depuis une exécution terminale avec la révision actuelle correspondante.", "종료된 실행에서 현재 일치하는 계획 리비전으로 새 영구 시도를 만듭니다."),
        new("status.loading_plans", 1, "Loading native plans for runtime {0}...", "正在加载 runtime {0} 的原生计划...", "正在載入 runtime {0} 的原生計畫...", "runtime {0} のネイティブプランを読み込み中...", "Cargando planes nativos para el runtime {0}...", "Native Pläne für Runtime {0} werden geladen...", "Chargement des plans natifs du runtime {0}...", "runtime {0}의 네이티브 계획을 불러오는 중..."),
        new("status.plans_loaded", 1, "Loaded {0} native plan.", "已加载 {0} 个原生计划。", "已載入 {0} 個原生計畫。", "ネイティブプランを {0} 件読み込みました。", "Se cargaron {0} planes nativos.", "{0} native Pläne geladen.", "{0} plans natifs chargés.", "네이티브 계획 {0}개를 불러왔습니다."),
        new("status.no_plan_runtime", 0, "Choose an exact runtime before loading native plans.", "请先选择一个精确 runtime，再加载原生计划。", "請先選擇一個精確 runtime，再載入原生計畫。", "ネイティブプランを読み込む前に正確な runtime を選択してください。", "Elige un runtime exacto antes de cargar planes nativos.", "Wählen Sie vor dem Laden nativer Pläne eine exakte Runtime.", "Choisissez un runtime exact avant de charger les plans natifs.", "네이티브 계획을 불러오기 전에 정확한 runtime을 선택하세요."),
        new("status.running_plan", 1, "Submitting plan {0} through Rust authority...", "正在通过 Rust 权威提交计划 {0}...", "正在透過 Rust 權威提交計畫 {0}...", "プラン {0} を Rust 権威へ送信中...", "Enviando el plan {0} mediante la autoridad Rust...", "Plan {0} wird über die Rust-Autorität übermittelt...", "Envoi du plan {0} via l’autorité Rust...", "계획 {0}을 Rust 권한으로 제출하는 중..."),
        new("status.run_queued", 1, "Durable run accepted: {0}.", "持久化运行已接受：{0}。", "持久化執行已接受：{0}。", "永続実行を受け付けました: {0}。", "Ejecución duradera aceptada: {0}.", "Dauerhafte Ausführung angenommen: {0}.", "Exécution durable acceptée : {0}.", "영구 실행이 수락되었습니다: {0}."),
        new("status.cancelling", 1, "Requesting safe cancellation for run {0}...", "正在请求安全取消运行 {0}...", "正在請求安全取消執行 {0}...", "実行 {0} の安全なキャンセルを要求中...", "Solicitando cancelación segura para la ejecución {0}...", "Sicherer Abbruch für Ausführung {0} wird angefordert...", "Demande d’annulation sûre pour l’exécution {0}...", "실행 {0}의 안전한 취소를 요청하는 중..."),
        new("status.cancelled", 1, "Queued run cancelled: {0}.", "已取消排队运行：{0}。", "已取消佇列執行：{0}。", "キュー内の実行をキャンセルしました: {0}。", "Ejecución en cola cancelada: {0}.", "Wartende Ausführung abgebrochen: {0}.", "Exécution en file annulée : {0}.", "대기 실행이 취소되었습니다: {0}."),
        new("status.retrying", 1, "Creating a durable retry for run {0}...", "正在为运行 {0} 创建持久化重试...", "正在為執行 {0} 建立持久化重試...", "実行 {0} の永続再試行を作成中...", "Creando un reintento duradero para la ejecución {0}...", "Dauerhafte Wiederholung für Ausführung {0} wird erstellt...", "Création d’une relance durable pour l’exécution {0}...", "실행 {0}의 영구 재시도를 만드는 중..."),
        new("status.retry_queued", 1, "Durable retry accepted: {0}.", "持久化重试已接受：{0}。", "持久化重試已接受：{0}。", "永続再試行を受け付けました: {0}。", "Reintento duradero aceptado: {0}.", "Dauerhafte Wiederholung angenommen: {0}.", "Relance durable acceptée : {0}.", "영구 재시도가 수락되었습니다: {0}."),
        new("approval.required", 0, "Enter a bounded approval note before executing this plan.", "执行此计划前请输入有边界的审批说明。", "執行此計畫前請輸入有界限的核准說明。", "このプランを実行する前に長さ制限内の承認メモを入力してください。", "Introduce una nota de aprobación acotada antes de ejecutar este plan.", "Geben Sie vor der Ausführung einen begrenzten Freigabehinweis ein.", "Saisissez une note d’approbation bornée avant d’exécuter ce plan.", "이 계획을 실행하기 전에 길이가 제한된 승인 메모를 입력하세요."),
        new("cancel.title", 0, "Confirm queued-run cancellation", "确认取消排队运行", "確認取消佇列執行", "キュー内実行のキャンセルを確認", "Confirmar cancelación de ejecución en cola", "Abbruch der wartenden Ausführung bestätigen", "Confirmer l’annulation de l’exécution en file", "대기 실행 취소 확인"),
        new("cancel.body", 1, "Cancel run {0} only if its work is still safely queued?", "仅当运行 {0} 的工作仍安全排队时取消它？", "僅當執行 {0} 的工作仍安全排隊時取消它？", "実行 {0} の処理がまだ安全にキュー内にある場合のみキャンセルしますか？", "¿Cancelar la ejecución {0} solo si su trabajo sigue en cola de forma segura?", "Ausführung {0} nur abbrechen, wenn ihre Arbeit noch sicher wartet?", "Annuler l’exécution {0} uniquement si son travail est encore en file en toute sécurité ?", "실행 {0}의 작업이 아직 안전하게 대기 중인 경우에만 취소할까요?"),
        new("cancel.warning", 0, "Already leased adapter work will be rejected rather than falsely reported as cancelled.", "已被适配器租约的工作会被拒绝取消，而不会被错误报告为已取消。", "已被介面卡租約的工作會被拒絕取消，而不會被錯誤回報為已取消。", "アダプターが取得済みの処理は、キャンセル済みと誤表示せず拒否されます。", "El trabajo ya arrendado será rechazado en vez de declararse cancelado falsamente.", "Bereits geleaste Adapterarbeit wird abgelehnt statt fälschlich als abgebrochen gemeldet.", "Le travail déjà loué sera refusé plutôt que déclaré annulé à tort.", "이미 임대된 어댑터 작업은 취소되었다고 잘못 보고하지 않고 거부됩니다."),
        new("cancel.confirm", 0, "Cancel queued work", "取消排队工作", "取消佇列工作", "キュー内の処理をキャンセル", "Cancelar trabajo en cola", "Wartende Arbeit abbrechen", "Annuler le travail en file", "대기 작업 취소"),
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
