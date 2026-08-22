using System.Globalization;

internal static class DesktopRuntimeWorkspaceCatalogs
{
    private const string Prefix = "desktop.runtime_workspace.";
    public const int KeyCount = 78;

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
        new("a11y.status", 0, "Runtime workspace query status", "runtime 工作区查询状态", "runtime 工作區查詢狀態", "runtime ワークスペースのクエリ状態", "Estado de consulta del espacio de trabajo del runtime", "Abfragestatus des Runtime-Arbeitsbereichs", "État de requête de l’espace runtime", "runtime 작업 공간 쿼리 상태"),
        new("a11y.status_value", 1, "Runtime workspace query status: {0}", "runtime 工作区查询状态：{0}", "runtime 工作區查詢狀態：{0}", "runtime ワークスペースのクエリ状態: {0}", "Estado de consulta del espacio de trabajo del runtime: {0}", "Abfragestatus des Runtime-Arbeitsbereichs: {0}", "État de requête de l’espace runtime : {0}", "runtime 작업 공간 쿼리 상태: {0}"),
        new("a11y.reload", 0, "Reload runtime workspace", "重新载入 runtime 工作区", "重新載入 runtime 工作區", "runtime ワークスペースを再読み込み", "Recargar el espacio de trabajo del runtime", "Runtime-Arbeitsbereich neu laden", "Recharger l’espace runtime", "runtime 작업 공간 다시 로드"),
        new("help.reload", 0, "Reloads status, history, and logs through one revision-consistent query group. Shortcut: F5.", "通过一个修订一致的查询组重新载入状态、历史记录和日志。快捷键：F5。", "透過一個修訂一致的查詢群組重新載入狀態、歷史記錄和日誌。快速鍵：F5。", "リビジョン整合性のある単一クエリグループで状態、履歴、ログを再読み込みします。ショートカット: F5。", "Recarga estado, historial y registros mediante un grupo de consultas coherente por revisión. Atajo: F5.", "Lädt Status, Verlauf und Protokolle über eine revisionskonsistente Abfragegruppe neu. Tastenkürzel: F5.", "Recharge l’état, l’historique et les journaux via un groupe de requêtes cohérent avec la révision. Raccourci : F5.", "리비전 일관성이 있는 단일 쿼리 그룹으로 상태, 기록, 로그를 다시 로드합니다. 단축키: F5."),
        new("a11y.alert_acknowledge", 0, "Acknowledge runtime workspace alert", "确认 runtime 工作区告警", "確認 runtime 工作區警示", "runtime ワークスペースのアラートを確認", "Confirmar la alerta del espacio de trabajo del runtime", "Warnung des Runtime-Arbeitsbereichs bestätigen", "Acquitter l’alerte de l’espace runtime", "runtime 작업 공간 경고 확인"),
        new("help.alert_acknowledge", 0, "Clears the retained severity alert without changing logs, filters, or live refresh.", "清除保留的严重级别告警，不更改日志、筛选条件或实时刷新。", "清除保留的嚴重性警示，不變更日誌、篩選條件或即時重新整理。", "保持されている重大度アラートを消去します。ログ、フィルター、ライブ更新は変更しません。", "Borra la alerta de gravedad retenida sin cambiar los registros, filtros ni la actualización en vivo.", "Löscht die gespeicherte Schweregradwarnung, ohne Protokolle, Filter oder Live-Aktualisierung zu ändern.", "Efface l’alerte de gravité conservée sans modifier les journaux, les filtres ni l’actualisation en direct.", "로그, 필터 또는 실시간 새로 고침을 변경하지 않고 유지된 심각도 경고를 지웁니다."),
        new("alert.none", 0, "No runtime workspace alert is awaiting acknowledgement.", "没有等待确认的 runtime 工作区告警。", "沒有等待確認的 runtime 工作區警示。", "確認待ちの runtime ワークスペースアラートはありません。", "No hay ninguna alerta del espacio de trabajo del runtime pendiente de confirmación.", "Keine Warnung des Runtime-Arbeitsbereichs wartet auf Bestätigung.", "Aucune alerte de l’espace runtime n’attend d’être acquittée.", "확인을 기다리는 runtime 작업 공간 경고가 없습니다."),
        new("status.alert_acknowledged", 1, "Workspace alert acknowledged at revision {0}", "已确认工作区告警，修订 {0}", "已確認工作區警示，修訂 {0}", "リビジョン {0} のワークスペースアラートを確認しました", "Alerta del espacio de trabajo confirmada en la revisión {0}", "Warnung des Arbeitsbereichs bei Revision {0} bestätigt", "Alerte de l’espace acquittée à la révision {0}", "리비전 {0}에서 작업 공간 경고를 확인했습니다"),
        new("status.live_paused", 1, "Live logs paused at revision {0}", "实时日志已在修订 {0} 暂停", "即時日誌已在修訂 {0} 暫停", "ライブログをリビジョン {0} で一時停止しました", "Registros en vivo pausados en la revisión {0}", "Live-Protokolle bei Revision {0} pausiert", "Journaux en direct suspendus à la révision {0}", "리비전 {0}에서 실시간 로그를 일시 중지했습니다"),
        new("action.pause_live", 0, "Pause live", "暂停实时刷新", "暫停即時重新整理", "ライブ更新を一時停止", "Pausar en vivo", "Live pausieren", "Suspendre le direct", "실시간 일시 중지"),
        new("live.waiting", 1, "Live logs enabled; the next revision-consistent query runs within {0} seconds.", "实时日志已启用；下一次修订一致查询将在 {0} 秒内运行。", "即時日誌已啟用；下一次修訂一致查詢將在 {0} 秒內執行。", "ライブログは有効です。次のリビジョン整合クエリは {0} 秒以内に実行されます。", "Los registros en vivo están activados; la siguiente consulta coherente por revisión se ejecutará en {0} segundos.", "Live-Protokolle sind aktiviert; die nächste revisionskonsistente Abfrage läuft innerhalb von {0} Sekunden.", "Les journaux en direct sont activés ; la prochaine requête cohérente avec la révision s’exécute dans les {0} secondes.", "실시간 로그가 활성화되었습니다. 다음 리비전 일관성 쿼리는 {0}초 이내에 실행됩니다."),
        new("live.recovering", 2, "Live logs recovering after {0} failed query; the next attempt runs within {1} seconds.", "实时日志正在从 {0} 次查询失败中恢复；下一次尝试将在 {1} 秒内运行。", "即時日誌正在從 {0} 次查詢失敗中復原；下一次嘗試將在 {1} 秒內執行。", "ライブログは {0} 回のクエリ失敗から復旧中です。次の試行は {1} 秒以内に実行されます。", "Los registros en vivo se recuperan tras {0} consulta fallida; el siguiente intento se ejecutará en {1} segundos.", "Live-Protokolle werden nach {0} fehlgeschlagener Abfrage wiederhergestellt; der nächste Versuch läuft innerhalb von {1} Sekunden.", "Les journaux en direct récupèrent après {0} requête échouée ; la prochaine tentative s’exécute dans les {1} secondes.", "실시간 로그가 {0}회의 쿼리 실패 후 복구 중입니다. 다음 시도는 {1}초 이내에 실행됩니다."),
        new("live.refreshing", 0, "Live logs enabled; one authenticated query group is in progress.", "实时日志已启用；一个已认证查询组正在运行。", "即時日誌已啟用；一個已驗證查詢群組正在執行。", "ライブログは有効です。認証済みクエリグループを実行中です。", "Los registros en vivo están activados; hay un grupo de consultas autenticado en curso.", "Live-Protokolle sind aktiviert; eine authentifizierte Abfragegruppe wird ausgeführt.", "Les journaux en direct sont activés ; un groupe de requêtes authentifié est en cours.", "실시간 로그가 활성화되었고 인증된 쿼리 그룹 하나가 실행 중입니다."),
        new("live.suspended", 0, "Live logs paused while this window is inactive.", "此窗口处于非活动状态时，实时日志会暂停。", "此視窗處於非作用中狀態時，即時日誌會暫停。", "このウィンドウが非アクティブな間、ライブログは一時停止します。", "Los registros en vivo están pausados mientras esta ventana está inactiva.", "Live-Protokolle sind pausiert, solange dieses Fenster inaktiv ist.", "Les journaux en direct sont suspendus tant que cette fenêtre est inactive.", "이 창이 비활성 상태인 동안 실시간 로그가 일시 중지됩니다."),
        new("live.idle", 1, "Starts explicit {0}-second live log refresh. No overlapping query is allowed.", "启动显式的 {0} 秒实时日志刷新。不允许查询重叠。", "啟動明確的 {0} 秒即時日誌重新整理。不允許查詢重疊。", "明示的な {0} 秒間隔のライブログ更新を開始します。クエリの重複実行は許可されません。", "Inicia una actualización explícita de registros en vivo cada {0} segundos. No se permiten consultas superpuestas.", "Startet eine explizite Live-Protokollaktualisierung im Abstand von {0} Sekunden. Überlappende Abfragen sind nicht zulässig.", "Démarre une actualisation explicite des journaux en direct toutes les {0} secondes. Aucune requête concurrente n’est autorisée.", "명시적인 {0}초 실시간 로그 새로 고침을 시작합니다. 쿼리 중첩은 허용되지 않습니다."),
        new("a11y.live_pause", 0, "Pause live runtime logs", "暂停 runtime 实时日志", "暫停 runtime 即時日誌", "runtime のライブログを一時停止", "Pausar los registros en vivo del runtime", "Live-Protokolle der Runtime pausieren", "Suspendre les journaux en direct du runtime", "runtime 실시간 로그 일시 중지"),
        new("a11y.live_start", 0, "Start live runtime logs", "启动 runtime 实时日志", "啟動 runtime 即時日誌", "runtime のライブログを開始", "Iniciar los registros en vivo del runtime", "Live-Protokolle der Runtime starten", "Démarrer les journaux en direct du runtime", "runtime 실시간 로그 시작"),
        new("a11y.log_search", 0, "Search runtime logs", "搜索 runtime 日志", "搜尋 runtime 日誌", "runtime ログを検索", "Buscar en los registros del runtime", "Runtime-Protokolle durchsuchen", "Rechercher dans les journaux du runtime", "runtime 로그 검색"),
        new("help.log_search", 0, "Filters the loaded sanitized log display locally. Shortcut: Control or Command plus F.", "在本地筛选已载入且净化后的日志显示。快捷键：Control 或 Command + F。", "在本機篩選已載入且淨化後的日誌顯示。快速鍵：Control 或 Command + F。", "読み込み済みのサニタイズ済みログ表示をローカルで絞り込みます。ショートカット: Control または Command + F。", "Filtra localmente la vista de registros saneados cargados. Atajo: Control o Command más F.", "Filtert die geladenen bereinigten Protokolle lokal. Tastenkürzel: Strg oder Command plus F.", "Filtre localement l’affichage des journaux assainis chargés. Raccourci : Contrôle ou Commande plus F.", "로드된 정제 로그 표시를 로컬에서 필터링합니다. 단축키: Control 또는 Command + F."),
        new("a11y.log_level", 0, "Runtime log level filter", "runtime 日志级别筛选", "runtime 日誌層級篩選", "runtime ログレベルフィルター", "Filtro de nivel de registro del runtime", "Filter für Runtime-Protokollstufen", "Filtre de niveau des journaux du runtime", "runtime 로그 수준 필터"),
        new("a11y.clear_filter", 0, "Clear runtime log filters", "清除 runtime 日志筛选条件", "清除 runtime 日誌篩選條件", "runtime ログフィルターを消去", "Borrar los filtros de registro del runtime", "Runtime-Protokollfilter löschen", "Effacer les filtres des journaux du runtime", "runtime 로그 필터 지우기"),
        new("a11y.filter_summary", 1, "Runtime log filter: {0}", "runtime 日志筛选：{0}", "runtime 日誌篩選：{0}", "runtime ログフィルター: {0}", "Filtro de registro del runtime: {0}", "Runtime-Protokollfilter: {0}", "Filtre des journaux du runtime : {0}", "runtime 로그 필터: {0}"),
        new("a11y.diagnostics_copy", 0, "Copy visible runtime diagnostics", "复制可见的 runtime 诊断", "複製可見的 runtime 診斷", "表示中の runtime 診断をコピー", "Copiar los diagnósticos visibles del runtime", "Sichtbare Runtime-Diagnose kopieren", "Copier le diagnostic runtime visible", "표시된 runtime 진단 복사"),
        new("help.diagnostics_copy", 0, "Copies endpoint-free workspace metadata, command history, and currently visible sanitized logs. Review before sharing.", "复制不含端点的工作区元数据、命令历史和当前可见的净化日志。分享前请检查。", "複製不含端點的工作區中繼資料、命令歷史和目前可見的淨化日誌。分享前請檢查。", "エンドポイントを含まないワークスペースメタデータ、コマンド履歴、現在表示中のサニタイズ済みログをコピーします。共有前に確認してください。", "Copia metadatos del espacio de trabajo sin endpoint, historial de comandos y registros saneados visibles. Revísalos antes de compartir.", "Kopiert endpunktfreie Arbeitsbereichsmetadaten, Befehlsverlauf und aktuell sichtbare bereinigte Protokolle. Vor dem Teilen prüfen.", "Copie les métadonnées de l’espace sans endpoint, l’historique des commandes et les journaux assainis visibles. Vérifiez avant de partager.", "엔드포인트가 없는 작업 공간 메타데이터, 명령 기록, 현재 표시된 정제 로그를 복사합니다. 공유 전에 검토하세요."),
        new("a11y.diagnostics_save", 0, "Save visible runtime diagnostics", "保存可见的 runtime 诊断", "儲存可見的 runtime 診斷", "表示中の runtime 診断を保存", "Guardar los diagnósticos visibles del runtime", "Sichtbare Runtime-Diagnose speichern", "Enregistrer le diagnostic runtime visible", "표시된 runtime 진단 저장"),
        new("help.diagnostics_save", 0, "Opens the system save panel for an endpoint-free bounded text export. Review the selected destination and file before sharing.", "打开系统保存面板，以导出不含端点且有边界的文本。分享前请检查所选目标位置和文件。", "開啟系統儲存面板，以匯出不含端點且有界限的文字。分享前請檢查所選目的地和檔案。", "エンドポイントを含まない有界テキストを書き出すシステム保存パネルを開きます。共有前に保存先とファイルを確認してください。", "Abre el panel de guardado del sistema para una exportación de texto acotada y sin endpoint. Revisa el destino y el archivo antes de compartir.", "Öffnet den Systemdialog zum Speichern eines begrenzten, endpunktfreien Textexports. Ziel und Datei vor dem Teilen prüfen.", "Ouvre le panneau système pour enregistrer un export texte borné et sans endpoint. Vérifiez la destination et le fichier avant de partager.", "엔드포인트가 없는 제한된 텍스트 내보내기를 위해 시스템 저장 패널을 엽니다. 공유 전에 대상과 파일을 검토하세요."),
        new("a11y.leselang", 0, "Preview equivalent workspace Leselang", "预览等价的工作区 Leselang", "預覽等價的工作區 Leselang", "同等のワークスペース Leselang をプレビュー", "Previsualizar el Leselang equivalente del espacio de trabajo", "Äquivalentes Arbeitsbereich-Leselang anzeigen", "Prévisualiser le Leselang équivalent de l’espace", "동등한 작업 공간 Leselang 미리 보기"),
        new("help.leselang", 0, "Opens canonical Leselang for the same inspect, history, and logs query group without executing it.", "打开同一检查、历史和日志查询组的规范 Leselang，但不执行。", "開啟同一檢查、歷史和日誌查詢群組的規範 Leselang，但不執行。", "同じ検査、履歴、ログのクエリグループに対応する正規 Leselang を、実行せずに開きます。", "Abre el Leselang canónico para el mismo grupo de consultas de inspección, historial y registros sin ejecutarlo.", "Öffnet kanonisches Leselang für dieselbe Inspektions-, Verlaufs- und Protokollabfragegruppe, ohne es auszuführen.", "Ouvre le Leselang canonique du même groupe de requêtes d’inspection, d’historique et de journaux sans l’exécuter.", "동일한 검사, 기록, 로그 쿼리 그룹의 정규 Leselang을 실행하지 않고 엽니다."),
        new("a11y.diagnostic_status", 0, "Diagnostic export status", "诊断导出状态", "診斷匯出狀態", "診断エクスポート状態", "Estado de exportación de diagnósticos", "Status des Diagnoseexports", "État de l’export du diagnostic", "진단 내보내기 상태"),
        new("level.all", 0, "All levels", "全部级别", "全部層級", "すべてのレベル", "Todos los niveles", "Alle Stufen", "Tous les niveaux", "모든 수준"),
        new("level.trace", 0, "Trace", "跟踪", "追蹤", "トレース", "Traza", "Ablaufverfolgung", "Trace", "추적"),
        new("level.debug", 0, "Debug", "调试", "偵錯", "デバッグ", "Depuración", "Debug", "Débogage", "디버그"),
        new("level.info", 0, "Info", "信息", "資訊", "情報", "Información", "Info", "Information", "정보"),
        new("level.warning", 0, "Warning", "警告", "警告", "警告", "Advertencia", "Warnung", "Avertissement", "경고"),
        new("level.error", 0, "Error", "错误", "錯誤", "エラー", "Error", "Fehler", "Erreur", "오류"),
        new("filter.some", 2, "Showing {0} of {1} logs", "显示 {1} 条日志中的 {0} 条", "顯示 {1} 筆日誌中的 {0} 筆", "{1} 件中 {0} 件のログを表示", "Mostrando {0} de {1} registros", "{0} von {1} Protokollen werden angezeigt", "Affichage de {0} journaux sur {1}", "로그 {1}개 중 {0}개 표시"),
        new("filter.all", 1, "Showing all {0} logs", "显示全部 {0} 条日志", "顯示全部 {0} 筆日誌", "全 {0} 件のログを表示", "Mostrando los {0} registros", "Alle {0} Protokolle werden angezeigt", "Affichage des {0} journaux", "로그 {0}개 모두 표시"),
        new("diagnostic.clipboard_unavailable", 0, "Clipboard unavailable.", "剪贴板不可用。", "剪貼簿不可用。", "クリップボードを利用できません。", "El portapapeles no está disponible.", "Zwischenablage nicht verfügbar.", "Le presse-papiers n’est pas disponible.", "클립보드를 사용할 수 없습니다."),
        new("diagnostic.copied", 0, "Visible diagnostic snapshot copied. Review it before sharing.", "已复制可见诊断快照。分享前请检查。", "已複製可見診斷快照。分享前請檢查。", "表示中の診断スナップショットをコピーしました。共有前に確認してください。", "Se copió la instantánea de diagnóstico visible. Revísala antes de compartir.", "Sichtbarer Diagnose-Snapshot kopiert. Vor dem Teilen prüfen.", "L’instantané de diagnostic visible a été copié. Vérifiez-le avant de partager.", "표시된 진단 스냅샷을 복사했습니다. 공유 전에 검토하세요."),
        new("diagnostic.copy_failed", 0, "Diagnostic copy failed safely.", "诊断复制已安全失败。", "診斷複製已安全失敗。", "診断のコピーは安全に失敗しました。", "La copia del diagnóstico falló de forma segura.", "Diagnosekopie ist sicher fehlgeschlagen.", "La copie du diagnostic a échoué de manière sûre.", "진단 복사가 안전하게 실패했습니다."),
        new("diagnostic.save_unavailable", 0, "System file saving is unavailable.", "系统文件保存不可用。", "系統檔案儲存不可用。", "システムのファイル保存を利用できません。", "El guardado de archivos del sistema no está disponible.", "Speichern von Systemdateien ist nicht verfügbar.", "L’enregistrement de fichiers système n’est pas disponible.", "시스템 파일 저장을 사용할 수 없습니다."),
        new("file.diagnostic_text", 0, "Leserpent diagnostic text", "Leserpent 诊断文本", "Leserpent 診斷文字", "Leserpent 診断テキスト", "Texto de diagnóstico de Leserpent", "Leserpent-Diagnosetext", "Texte de diagnostic Leserpent", "Leserpent 진단 텍스트"),
        new("file.save_title", 0, "Save runtime diagnostics", "保存 runtime 诊断", "儲存 runtime 診斷", "runtime 診断を保存", "Guardar diagnósticos del runtime", "Runtime-Diagnose speichern", "Enregistrer le diagnostic runtime", "runtime 진단 저장"),
        new("diagnostic.save_cancelled", 0, "Diagnostic save canceled.", "已取消保存诊断。", "已取消儲存診斷。", "診断の保存をキャンセルしました。", "Se canceló el guardado del diagnóstico.", "Speichern der Diagnose abgebrochen.", "L’enregistrement du diagnostic a été annulé.", "진단 저장을 취소했습니다."),
        new("diagnostic.saved", 0, "Diagnostic snapshot saved. Review the file before sharing.", "诊断快照已保存。分享前请检查文件。", "診斷快照已儲存。分享前請檢查檔案。", "診断スナップショットを保存しました。共有前にファイルを確認してください。", "Se guardó la instantánea de diagnóstico. Revisa el archivo antes de compartir.", "Diagnose-Snapshot gespeichert. Datei vor dem Teilen prüfen.", "L’instantané de diagnostic a été enregistré. Vérifiez le fichier avant de partager.", "진단 스냅샷을 저장했습니다. 공유 전에 파일을 검토하세요."),
        new("diagnostic.save_failed", 0, "Diagnostic save failed safely.", "诊断保存已安全失败。", "診斷儲存已安全失敗。", "診断の保存は安全に失敗しました。", "El guardado del diagnóstico falló de forma segura.", "Speichern der Diagnose ist sicher fehlgeschlagen.", "L’enregistrement du diagnostic a échoué de manière sûre.", "진단 저장이 안전하게 실패했습니다."),
        new("title.leselang", 1, "Workspace Leselang / {0}", "工作区 Leselang / {0}", "工作區 Leselang / {0}", "ワークスペース Leselang / {0}", "Leselang del espacio de trabajo / {0}", "Arbeitsbereich-Leselang / {0}", "Leselang de l’espace / {0}", "작업 공간 Leselang / {0}"),
        new("status.loading", 0, "Loading authenticated runtime snapshot...", "正在载入已认证的 runtime 快照...", "正在載入已驗證的 runtime 快照...", "認証済み runtime スナップショットを読み込み中...", "Cargando la instantánea autenticada del runtime...", "Authentifizierter Runtime-Snapshot wird geladen...", "Chargement de l’instantané runtime authentifié...", "인증된 runtime 스냅샷 로드 중..."),
        new("snapshot.incremental", 0, "incremental", "增量", "增量", "増分", "incremental", "inkrementell", "incrémental", "증분"),
        new("snapshot.full", 0, "full", "完整", "完整", "完全", "completa", "vollständig", "complet", "전체"),
        new("change.initial", 0, "initial snapshot", "初始快照", "初始快照", "初期スナップショット", "instantánea inicial", "erster Snapshot", "instantané initial", "초기 스냅샷"),
        new("change.revision", 1, "revision +{0}", "修订 +{0}", "修訂 +{0}", "リビジョン +{0}", "revisión +{0}", "Revision +{0}", "révision +{0}", "리비전 +{0}"),
        new("change.log_sequence_reset", 0, "log sequence reset", "日志序列已重置", "日誌序列已重設", "ログシーケンスをリセット", "se reinició la secuencia de registros", "Protokollsequenz zurückgesetzt", "séquence des journaux réinitialisée", "로그 시퀀스 재설정"),
        new("change.logs_added", 1, "+{0} logs", "+{0} 条日志", "+{0} 筆日誌", "+{0} 件のログ", "+{0} registros", "+{0} Protokolle", "+{0} journaux", "로그 +{0}개"),
        new("change.errors_new", 1, "{0} new error", "{0} 个新错误", "{0} 個新錯誤", "新しいエラー {0} 件", "{0} error nuevo", "{0} neuer Fehler", "{0} nouvelle erreur", "새 오류 {0}개"),
        new("change.warnings_new", 1, "{0} new warning", "{0} 个新警告", "{0} 個新警告", "新しい警告 {0} 件", "{0} advertencia nueva", "{0} neue Warnung", "{0} nouvel avertissement", "새 경고 {0}개"),
        new("change.logs_expired", 1, "{0} logs expired", "{0} 条日志已过期", "{0} 筆日誌已過期", "{0} 件のログが期限切れ", "{0} registros caducaron", "{0} Protokolle abgelaufen", "{0} journaux expirés", "로그 {0}개 만료"),
        new("change.logs_changed", 1, "{0} logs changed", "{0} 条日志已变更", "{0} 筆日誌已變更", "{0} 件のログが変更", "{0} registros cambiaron", "{0} Protokolle geändert", "{0} journaux modifiés", "로그 {0}개 변경"),
        new("change.commands_added", 1, "+{0} commands", "+{0} 条命令", "+{0} 筆命令", "+{0} 件のコマンド", "+{0} comandos", "+{0} Befehle", "+{0} commandes", "명령 +{0}개"),
        new("change.commands_updated", 1, "{0} commands updated", "{0} 条命令已更新", "{0} 筆命令已更新", "{0} 件のコマンドを更新", "{0} comandos actualizados", "{0} Befehle aktualisiert", "{0} commandes mises à jour", "명령 {0}개 업데이트"),
        new("change.none", 0, "no changes", "无变更", "無變更", "変更なし", "sin cambios", "keine Änderungen", "aucune modification", "변경 없음"),
        new("alert.error", 1, "unacknowledged error signal from revision {0}", "修订 {0} 有未确认的错误信号", "修訂 {0} 有未確認的錯誤訊號", "リビジョン {0} の未確認エラーシグナル", "señal de error sin confirmar de la revisión {0}", "unbestätigtes Fehlersignal aus Revision {0}", "signal d’erreur non acquitté de la révision {0}", "리비전 {0}의 미확인 오류 신호"),
        new("alert.warning", 1, "unacknowledged warning signal from revision {0}", "修订 {0} 有未确认的警告信号", "修訂 {0} 有未確認的警告訊號", "リビジョン {0} の未確認警告シグナル", "señal de advertencia sin confirmar de la revisión {0}", "unbestätigtes Warnsignal aus Revision {0}", "signal d’avertissement non acquitté de la révision {0}", "리비전 {0}의 미확인 경고 신호"),
        new("status.live", 4, "Live logs at revision {0} / {1} snapshot / {2} / refresh every {3} seconds", "实时日志，修订 {0} / {1}快照 / {2} / 每 {3} 秒刷新", "即時日誌，修訂 {0} / {1}快照 / {2} / 每 {3} 秒重新整理", "ライブログ、リビジョン {0} / {1}スナップショット / {2} / {3} 秒ごとに更新", "Registros en vivo en la revisión {0} / instantánea {1} / {2} / actualización cada {3} segundos", "Live-Protokolle bei Revision {0} / {1}er Snapshot / {2} / Aktualisierung alle {3} Sekunden", "Journaux en direct à la révision {0} / instantané {1} / {2} / actualisation toutes les {3} secondes", "실시간 로그 리비전 {0} / {1} 스냅샷 / {2} / {3}초마다 새로 고침"),
        new("status.live_alert", 5, "Live logs at revision {0} / {1} snapshot / {2} / {3} / refresh every {4} seconds", "实时日志，修订 {0} / {1}快照 / {2} / {3} / 每 {4} 秒刷新", "即時日誌，修訂 {0} / {1}快照 / {2} / {3} / 每 {4} 秒重新整理", "ライブログ、リビジョン {0} / {1}スナップショット / {2} / {3} / {4} 秒ごとに更新", "Registros en vivo en la revisión {0} / instantánea {1} / {2} / {3} / actualización cada {4} segundos", "Live-Protokolle bei Revision {0} / {1}er Snapshot / {2} / {3} / Aktualisierung alle {4} Sekunden", "Journaux en direct à la révision {0} / instantané {1} / {2} / {3} / actualisation toutes les {4} secondes", "실시간 로그 리비전 {0} / {1} 스냅샷 / {2} / {3} / {4}초마다 새로 고침"),
        new("status.workspace", 2, "Live workspace at revision {0} / {1}", "实时工作区，修订 {0} / {1}", "即時工作區，修訂 {0} / {1}", "ライブワークスペース、リビジョン {0} / {1}", "Espacio de trabajo en vivo en la revisión {0} / {1}", "Live-Arbeitsbereich bei Revision {0} / {1}", "Espace en direct à la révision {0} / {1}", "실시간 작업 공간 리비전 {0} / {1}"),
        new("status.workspace_alert", 3, "Live workspace at revision {0} / {1} / {2}", "实时工作区，修订 {0} / {1} / {2}", "即時工作區，修訂 {0} / {1} / {2}", "ライブワークスペース、リビジョン {0} / {1} / {2}", "Espacio de trabajo en vivo en la revisión {0} / {1} / {2}", "Live-Arbeitsbereich bei Revision {0} / {1} / {2}", "Espace en direct à la révision {0} / {1} / {2}", "실시간 작업 공간 리비전 {0} / {1} / {2}"),
        new("failure.rejected", 2, "Query rejected ({0}): {1}", "查询被拒绝（{0}）：{1}", "查詢遭拒絕（{0}）：{1}", "クエリが拒否されました ({0}): {1}", "Consulta rechazada ({0}): {1}", "Abfrage abgelehnt ({0}): {1}", "Requête rejetée ({0}) : {1}", "쿼리 거부됨({0}): {1}"),
        new("failure.response", 1, "Query response rejected: {0}", "查询响应被拒绝：{0}", "查詢回應遭拒絕：{0}", "クエリ応答が拒否されました: {0}", "Respuesta de consulta rechazada: {0}", "Abfrageantwort abgelehnt: {0}", "Réponse de requête rejetée : {0}", "쿼리 응답 거부됨: {0}"),
        new("failure.blocked", 1, "Query blocked: {0}", "查询被阻止：{0}", "查詢遭阻止：{0}", "クエリがブロックされました: {0}", "Consulta bloqueada: {0}", "Abfrage blockiert: {0}", "Requête bloquée : {0}", "쿼리 차단됨: {0}"),
        new("failure.timeout", 0, "Query timed out; no partial workspace was retained", "查询超时；未保留部分工作区", "查詢逾時；未保留部分工作區", "クエリがタイムアウトしました。部分的なワークスペースは保持されていません", "La consulta agotó el tiempo; no se conservó ningún espacio de trabajo parcial", "Zeitüberschreitung der Abfrage; kein teilweiser Arbeitsbereich wurde beibehalten", "La requête a expiré ; aucun espace partiel n’a été conservé", "쿼리 시간이 초과되어 부분 작업 공간을 유지하지 않았습니다"),
        new("failure.transport", 0, "Query failed over the authenticated HTTPS connection", "查询通过已认证 HTTPS 连接失败", "查詢透過已驗證 HTTPS 連線失敗", "認証済み HTTPS 接続でクエリが失敗しました", "La consulta falló en la conexión HTTPS autenticada", "Abfrage über die authentifizierte HTTPS-Verbindung fehlgeschlagen", "La requête a échoué sur la connexion HTTPS authentifiée", "인증된 HTTPS 연결에서 쿼리가 실패했습니다"),
        new("live.reason.unexpected", 0, "unexpected query failure", "意外查询故障", "非預期查詢故障", "予期しないクエリ障害", "fallo inesperado de consulta", "unerwarteter Abfragefehler", "échec inattendu de requête", "예기치 않은 쿼리 실패"),
        new("live.reason.authenticated", 0, "authenticated query failure", "已认证查询故障", "已驗證查詢故障", "認証済みクエリの障害", "fallo de consulta autenticada", "Fehler der authentifizierten Abfrage", "échec de requête authentifiée", "인증된 쿼리 실패"),
        new("live.recovery.active", 0, "retry when this window becomes active", "此窗口恢复活动时重试", "此視窗恢復作用中時重試", "このウィンドウがアクティブになったときに再試行", "reintentar cuando esta ventana vuelva a estar activa", "erneut versuchen, wenn dieses Fenster aktiv wird", "réessayer lorsque cette fenêtre redevient active", "이 창이 활성화되면 다시 시도"),
        new("live.recovery.delay", 1, "retry in {0} seconds", "在 {0} 秒后重试", "在 {0} 秒後重試", "{0} 秒後に再試行", "reintentar en {0} segundos", "in {0} Sekunden erneut versuchen", "réessayer dans {0} secondes", "{0}초 후 다시 시도"),
        new("status.live_recovering", 4, "Live logs recovering from {0} ({1}/{2}); {3}", "实时日志正在从{0}中恢复（{1}/{2}）；{3}", "即時日誌正在從{0}中復原（{1}/{2}）；{3}", "ライブログは {0} から復旧中です ({1}/{2})。{3}", "Los registros en vivo se recuperan de {0} ({1}/{2}); {3}", "Live-Protokolle werden nach {0} wiederhergestellt ({1}/{2}); {3}", "Les journaux en direct récupèrent après {0} ({1}/{2}) ; {3}", "실시간 로그가 {0}에서 복구 중입니다({1}/{2}). {3}"),
        new("status.live_stopped", 1, "Live logs stopped after {0} consecutive failures", "实时日志在连续 {0} 次失败后停止", "即時日誌在連續 {0} 次失敗後停止", "ライブログは {0} 回連続の失敗後に停止しました", "Los registros en vivo se detuvieron tras {0} fallos consecutivos", "Live-Protokolle nach {0} aufeinanderfolgenden Fehlern gestoppt", "Les journaux en direct se sont arrêtés après {0} échecs consécutifs", "실시간 로그가 {0}회 연속 실패 후 중지되었습니다"),
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
                $"desktop runtime workspace localization key is unknown: {key}");
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
                "desktop runtime workspace localization entries drifted");
        }
        DesktopDomainCatalogContract.Verify(
            "runtime workspace",
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
