using System.Globalization;

internal static class DesktopRemoteShellCatalogs
{
    private const string Prefix = "desktop.remote_shell.";
    public const int KeyCount = 56;

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
        new("title", 1, "Leserpent / {0}", "Leserpent / {0}", "Leserpent / {0}", "Leserpent / {0}", "Leserpent / {0}", "Leserpent / {0}", "Leserpent / {0}", "Leserpent / {0}"),
        new("a11y.dismiss", 0, "Dismiss remote operation status", "收起远程操作状态", "收起遠端操作狀態", "リモート操作の状態を閉じる", "Descartar el estado de la operación remota", "Status des Remotevorgangs schließen", "Masquer l’état de l’opération distante", "원격 작업 상태 닫기"),
        new("a11y.reconnect", 0, "Reconnect remote event stream", "重新连接远程事件流", "重新連線遠端事件串流", "リモートイベントストリームに再接続", "Volver a conectar el flujo de eventos remoto", "Remote-Ereignisstrom neu verbinden", "Reconnecter le flux d’événements distant", "원격 이벤트 스트림 다시 연결"),
        new("help.reconnect", 0, "Restarts the read-only event stream after automatic reconnect is exhausted. Shortcut: F5.", "自动重连耗尽后重新启动只读事件流。快捷键：F5。", "自動重連耗盡後重新啟動唯讀事件串流。快速鍵：F5。", "自動再接続が尽きた後に読み取り専用イベントストリームを再起動します。ショートカット: F5。", "Reinicia el flujo de eventos de solo lectura cuando se agota la reconexión automática. Atajo: F5.", "Startet den schreibgeschützten Ereignisstrom neu, wenn die automatische Wiederverbindung ausgeschöpft ist. Tastenkürzel: F5.", "Redémarre le flux d’événements en lecture seule après épuisement des reconnexions automatiques. Raccourci : F5.", "자동 재연결을 모두 시도한 뒤 읽기 전용 이벤트 스트림을 다시 시작합니다. 단축키: F5."),
        new("tooltip.reconnect", 0, "Reconnect event stream (F5)", "重新连接事件流（F5）", "重新連線事件串流（F5）", "イベントストリームに再接続 (F5)", "Volver a conectar el flujo de eventos (F5)", "Ereignisstrom neu verbinden (F5)", "Reconnecter le flux d’événements (F5)", "이벤트 스트림 다시 연결(F5)"),
        new("help.connection", 0, "Switch the remote authority or forget the saved profile and endpoint credential.", "切换远程权威端，或忘记已保存的配置与端点凭证。", "切換遠端權威端，或忘記已儲存的設定檔與端點憑證。", "リモート権限元を切り替えるか、保存済みプロファイルとエンドポイント資格情報を削除します。", "Cambia la autoridad remota u olvida el perfil y la credencial del endpoint guardados.", "Wechselt die Remote-Autorität oder entfernt das gespeicherte Profil und die Endpunkt-Anmeldedaten.", "Change l’autorité distante ou oublie le profil et l’accréditation de point de terminaison enregistrés.", "원격 권한 주체를 전환하거나 저장된 프로필과 엔드포인트 자격 증명을 삭제합니다."),
        new("a11y.filter", 0, "Filter remote runtimes", "筛选远程 runtime", "篩選遠端 runtime", "リモート runtime を絞り込む", "Filtrar runtimes remotos", "Remote-Runtimes filtern", "Filtrer les runtimes distants", "원격 runtime 필터링"),
        new("help.filter", 0, "Filters the local runtime projection without contacting the server. Shortcut: Control or Command plus F.", "仅筛选本地 runtime 投影，不会联系服务器。快捷键：Control 或 Command 加 F。", "僅篩選本機 runtime 投影，不會聯絡伺服器。快速鍵：Control 或 Command 加 F。", "サーバーへ接続せず、ローカルの runtime 投影だけを絞り込みます。ショートカット: Control または Command + F。", "Filtra la proyección local de runtimes sin contactar con el servidor. Atajo: Control o Command más F.", "Filtert die lokale Runtime-Projektion ohne Serverkontakt. Tastenkürzel: Strg oder Befehl plus F.", "Filtre la projection locale des runtimes sans contacter le serveur. Raccourci : Contrôle ou Commande plus F.", "서버에 연결하지 않고 로컬 runtime 투영만 필터링합니다. 단축키: Control 또는 Command와 F."),
        new("a11y.clear_filter", 0, "Clear runtime filter", "清除 runtime 筛选条件", "清除 runtime 篩選條件", "runtime フィルターを消去", "Borrar el filtro de runtimes", "Runtime-Filter leeren", "Effacer le filtre des runtimes", "runtime 필터 지우기"),
        new("a11y.health_refresh", 0, "Refresh remote authority health", "刷新远程权威端健康状态", "重新整理遠端權威端健康狀態", "リモート権限元の正常性を更新", "Actualizar el estado de la autoridad remota", "Zustand der Remote-Autorität aktualisieren", "Actualiser l’état de l’autorité distante", "원격 권한 주체 상태 새로 고침"),
        new("help.health_refresh", 0, "Checks authority ownership, protocol readiness, and effect queue pressure without changing remote state.", "检查权威归属、协议就绪状态和副作用队列压力，不会更改远程状态。", "檢查權威歸屬、協定就緒狀態和副作用佇列壓力，不會變更遠端狀態。", "リモート状態を変更せず、権限の所有、プロトコル準備状態、エフェクトキューの負荷を確認します。", "Comprueba la propiedad de la autoridad, la preparación del protocolo y la presión de la cola de efectos sin cambiar el estado remoto.", "Prüft Autoritätsbesitz, Protokollbereitschaft und Druck der Effektwarteschlange, ohne den Remotezustand zu ändern.", "Vérifie la propriété de l’autorité, l’état du protocole et la pression de la file d’effets sans modifier l’état distant.", "원격 상태를 변경하지 않고 권한 소유권, 프로토콜 준비 상태와 효과 큐 압력을 확인합니다."),
        new("identity.ca_short", 1, "CA / {0}", "CA / {0}", "CA / {0}", "CA / {0}", "AC / {0}", "CA / {0}", "AC / {0}", "CA / {0}"),
        new("a11y.origin", 1, "Remote HTTPS origin: {0}", "远程 HTTPS 源站：{0}", "遠端 HTTPS 來源：{0}", "リモート HTTPS オリジン: {0}", "Origen HTTPS remoto: {0}", "Remote-HTTPS-Ursprung: {0}", "Origine HTTPS distante : {0}", "원격 HTTPS 원본: {0}"),
        new("a11y.ca", 1, "Remote CA SHA-256 fingerprint: {0}", "远程 CA SHA-256 指纹：{0}", "遠端 CA SHA-256 指紋：{0}", "リモート CA SHA-256 フィンガープリント: {0}", "Huella SHA-256 de la AC remota: {0}", "SHA-256-Fingerabdruck der Remote-CA: {0}", "Empreinte SHA-256 de l’AC distante : {0}", "원격 CA SHA-256 지문: {0}"),
        new("help.ca", 0, "Compare this SHA-256 fingerprint with the expected remote authority CA.", "请将此 SHA-256 指纹与预期的远程权威端 CA 对比。", "請將此 SHA-256 指紋與預期的遠端權威端 CA 比對。", "この SHA-256 フィンガープリントを想定するリモート権限元 CA と比較してください。", "Compara esta huella SHA-256 con la AC esperada de la autoridad remota.", "Vergleichen Sie diesen SHA-256-Fingerabdruck mit der erwarteten CA der Remote-Autorität.", "Comparez cette empreinte SHA-256 avec l’AC attendue de l’autorité distante.", "이 SHA-256 지문을 예상한 원격 권한 주체 CA와 비교하세요."),
        new("tooltip.ca", 1, "CA SHA-256: {0}", "CA SHA-256：{0}", "CA SHA-256：{0}", "CA SHA-256: {0}", "SHA-256 de la AC: {0}", "CA SHA-256: {0}", "SHA-256 de l’AC : {0}", "CA SHA-256: {0}"),
        new("a11y.status", 1, "Remote connection status: {0}", "远程连接状态：{0}", "遠端連線狀態：{0}", "リモート接続状態: {0}", "Estado de la conexión remota: {0}", "Remote-Verbindungsstatus: {0}", "État de la connexion distante : {0}", "원격 연결 상태: {0}"),
        new("a11y.operation", 1, "Remote operation status: {0}", "远程操作状态：{0}", "遠端操作狀態：{0}", "リモート操作状態: {0}", "Estado de la operación remota: {0}", "Status des Remotevorgangs: {0}", "État de l’opération distante : {0}", "원격 작업 상태: {0}"),
        new("a11y.health", 1, "Remote authority health: {0}", "远程权威端健康状态：{0}", "遠端權威端健康狀態：{0}", "リモート権限元の正常性: {0}", "Estado de la autoridad remota: {0}", "Zustand der Remote-Autorität: {0}", "État de l’autorité distante : {0}", "원격 권한 주체 상태: {0}"),
        new("unavailable_value", 0, "Unavailable", "不可用", "無法使用", "利用不可", "No disponible", "Nicht verfügbar", "Indisponible", "사용할 수 없음"),
        new("credential.local.label", 0, "TOKEN / LOCAL PROCESS", "令牌 / 本地进程", "權杖 / 本機程序", "トークン / ローカルプロセス", "TOKEN / PROCESO LOCAL", "TOKEN / LOKALER PROZESS", "JETON / PROCESSUS LOCAL", "토큰 / 로컬 프로세스"),
        new("credential.local.a11y", 0, "Remote credential source: local process", "远程凭证来源：本地进程", "遠端憑證來源：本機程序", "リモート資格情報の取得元: ローカルプロセス", "Origen de la credencial remota: proceso local", "Quelle der Remote-Anmeldedaten: lokaler Prozess", "Source de l’accréditation distante : processus local", "원격 자격 증명 원본: 로컬 프로세스"),
        new("credential.local.help", 0, "The credential is ephemeral and scoped to the local Leserpent service process.", "该凭证是临时的，仅限本地 Leserpent 服务进程使用。", "該憑證是暫時的，僅限本機 Leserpent 服務程序使用。", "この資格情報は一時的で、ローカル Leserpent サービスプロセスに限定されます。", "La credencial es efímera y está limitada al proceso de servicio Leserpent local.", "Die Anmeldedaten sind flüchtig und auf den lokalen Leserpent-Dienstprozess beschränkt.", "L’accréditation est éphémère et limitée au processus de service Leserpent local.", "자격 증명은 임시이며 로컬 Leserpent 서비스 프로세스로 범위가 제한됩니다."),
        new("credential.environment.label", 0, "TOKEN / ENV FALLBACK", "令牌 / 环境变量回退", "權杖 / 環境變數後援", "トークン / 環境変数フォールバック", "TOKEN / ALTERNATIVA DE ENTORNO", "TOKEN / UMGEBUNGS-FALLBACK", "JETON / REPLI ENVIRONNEMENT", "토큰 / 환경 변수 대체"),
        new("credential.environment.a11y", 0, "Remote credential source: environment fallback", "远程凭证来源：环境变量回退", "遠端憑證來源：環境變數後援", "リモート資格情報の取得元: 環境変数フォールバック", "Origen de la credencial remota: alternativa de entorno", "Quelle der Remote-Anmeldedaten: Umgebungs-Fallback", "Source de l’accréditation distante : repli sur l’environnement", "원격 자격 증명 원본: 환경 변수 대체"),
        new("credential.environment.help", 0, "Remote token comes from LESERPENT_REMOTE_TOKEN. Store an endpoint-scoped token in the platform credential store for interactive use.", "远程令牌来自 LESERPENT_REMOTE_TOKEN。交互使用时，请在平台凭证库中保存限定端点的令牌。", "遠端權杖來自 LESERPENT_REMOTE_TOKEN。互動使用時，請在平台憑證庫中儲存限定端點的權杖。", "リモートトークンは LESERPENT_REMOTE_TOKEN から取得されます。対話操作には、エンドポイント限定トークンをプラットフォーム資格情報ストアへ保存してください。", "El token remoto procede de LESERPENT_REMOTE_TOKEN. Para uso interactivo, guarda un token limitado al endpoint en el almacén de credenciales de la plataforma.", "Das Remote-Token stammt aus LESERPENT_REMOTE_TOKEN. Speichern Sie für die interaktive Nutzung ein endpunktgebundenes Token im Anmeldedatenspeicher der Plattform.", "Le jeton distant provient de LESERPENT_REMOTE_TOKEN. Pour un usage interactif, stockez un jeton limité au point de terminaison dans le coffre d’accréditations de la plateforme.", "원격 토큰은 LESERPENT_REMOTE_TOKEN에서 가져옵니다. 대화형 사용을 위해 엔드포인트 범위 토큰을 플랫폼 자격 증명 저장소에 보관하세요."),
        new("credential.platform.label", 1, "TOKEN / {0}", "令牌 / {0}", "權杖 / {0}", "トークン / {0}", "TOKEN / {0}", "TOKEN / {0}", "JETON / {0}", "토큰 / {0}"),
        new("credential.platform.a11y", 1, "Remote credential source: {0}", "远程凭证来源：{0}", "遠端憑證來源：{0}", "リモート資格情報の取得元: {0}", "Origen de la credencial remota: {0}", "Quelle der Remote-Anmeldedaten: {0}", "Source de l’accréditation distante : {0}", "원격 자격 증명 원본: {0}"),
        new("credential.platform.help", 1, "Remote token comes from {0}.", "远程令牌来自 {0}。", "遠端權杖來自 {0}。", "リモートトークンは {0} から取得されます。", "El token remoto procede de {0}.", "Das Remote-Token stammt aus {0}.", "Le jeton distant provient de {0}.", "원격 토큰은 {0}에서 가져옵니다."),
        new("feed.connecting", 0, "Connecting", "正在连接", "正在連線", "接続中", "Conectando", "Verbindung wird hergestellt", "Connexion en cours", "연결 중"),
        new("feed.cached_connecting", 1, "Showing cached revision {0}; connecting", "正在显示缓存修订 {0}；连接中", "正在顯示快取修訂 {0}；連線中", "キャッシュ済みリビジョン {0} を表示中、接続しています", "Mostrando la revisión en caché {0}; conectando", "Zwischengespeicherte Revision {0} wird angezeigt; Verbindung wird hergestellt", "Affichage de la révision {0} en cache ; connexion en cours", "캐시된 리비전 {0} 표시 중; 연결 중"),
        new("feed.live", 1, "Live at revision {0}", "实时，修订 {0}", "即時，修訂 {0}", "リビジョン {0} でライブ", "En directo en la revisión {0}", "Live bei Revision {0}", "En direct à la révision {0}", "리비전 {0}에서 실시간"),
        new("feed.resynchronizing", 1, "Resynchronizing from revision {0}", "正在从修订 {0} 重新同步", "正在從修訂 {0} 重新同步", "リビジョン {0} から再同期中", "Resincronizando desde la revisión {0}", "Neusynchronisierung ab Revision {0}", "Resynchronisation depuis la révision {0}", "리비전 {0}부터 다시 동기화 중"),
        new("feed.reconnecting", 1, "Reconnect attempt {0}", "第 {0} 次重连", "第 {0} 次重新連線", "再接続試行 {0}", "Intento de reconexión {0}", "Wiederverbindungsversuch {0}", "Tentative de reconnexion {0}", "재연결 시도 {0}"),
        new("feed.refreshing_snapshot", 0, "Refreshing the complete remote snapshot", "正在刷新完整远程快照", "正在重新整理完整遠端快照", "完全なリモートスナップショットを更新中", "Actualizando la instantánea remota completa", "Vollständiger Remote-Snapshot wird aktualisiert", "Actualisation de l’instantané distant complet", "전체 원격 스냅샷 새로 고침 중"),
        new("feed.offline", 1, "Offline after {0} attempts", "尝试 {0} 次后离线", "嘗試 {0} 次後離線", "{0} 回の試行後にオフライン", "Sin conexión después de {0} intentos", "Nach {0} Versuchen offline", "Hors ligne après {0} tentatives", "{0}회 시도 후 오프라인"),
        new("feed.stopped", 0, "Stopped", "已停止", "已停止", "停止しました", "Detenido", "Beendet", "Arrêté", "중지됨"),
        new("feed.revision", 1, "EVENTS v1 / revision {0}", "EVENTS v1 / 修订 {0}", "EVENTS v1 / 修訂 {0}", "EVENTS v1 / リビジョン {0}", "EVENTS v1 / revisión {0}", "EVENTS v1 / Revision {0}", "EVENTS v1 / révision {0}", "EVENTS v1 / 리비전 {0}"),
        new("feed.awaiting_snapshot", 0, "EVENTS v1 / awaiting snapshot", "EVENTS v1 / 等待快照", "EVENTS v1 / 等待快照", "EVENTS v1 / スナップショット待機中", "EVENTS v1 / esperando instantánea", "EVENTS v1 / Snapshot ausstehend", "EVENTS v1 / en attente d’un instantané", "EVENTS v1 / 스냅샷 대기 중"),
        new("count.all", 1, "{0} runtimes", "{0} 个 runtime", "{0} 個 runtime", "runtime {0} 件", "{0} runtimes", "{0} Runtimes", "{0} runtimes", "runtime {0}개"),
        new("count.filtered", 2, "{0} of {1}", "{1} 个中显示 {0} 个", "{1} 個中顯示 {0} 個", "{1} 件中 {0} 件", "{0} de {1}", "{0} von {1}", "{0} sur {1}", "{1}개 중 {0}개"),
        new("a11y.count", 2, "Showing {0} of {1} remote runtimes", "正在显示 {1} 个远程 runtime 中的 {0} 个", "正在顯示 {1} 個遠端 runtime 中的 {0} 個", "{1} 件のリモート runtime のうち {0} 件を表示", "Mostrando {0} de {1} runtimes remotos", "{0} von {1} Remote-Runtimes werden angezeigt", "Affichage de {0} runtimes distants sur {1}", "원격 runtime {1}개 중 {0}개 표시 중"),
        new("health.idle", 0, "AUTHORITY / awaiting check", "权威端 / 等待检查", "權威端 / 等待檢查", "権限元 / 確認待ち", "AUTORIDAD / esperando comprobación", "AUTORITÄT / Prüfung ausstehend", "AUTORITÉ / en attente de vérification", "권한 주체 / 검사 대기 중"),
        new("health.checking", 0, "AUTHORITY / checking", "权威端 / 检查中", "權威端 / 檢查中", "権限元 / 確認中", "AUTORIDAD / comprobando", "AUTORITÄT / wird geprüft", "AUTORITÉ / vérification", "권한 주체 / 검사 중"),
        new("health.ready", 0, "AUTHORITY / ready", "权威端 / 就绪", "權威端 / 就緒", "権限元 / 準備完了", "AUTORIDAD / lista", "AUTORITÄT / bereit", "AUTORITÉ / prête", "권한 주체 / 준비됨"),
        new("health.queue", 2, "QUEUE / {0}/{1}", "队列 / {0}/{1}", "佇列 / {0}/{1}", "キュー / {0}/{1}", "COLA / {0}/{1}", "WARTESCHLANGE / {0}/{1}", "FILE / {0}/{1}", "큐 / {0}/{1}"),
        new("health.queue_saturated", 2, "QUEUE SATURATED / {0}/{1}", "队列饱和 / {0}/{1}", "佇列飽和 / {0}/{1}", "キュー飽和 / {0}/{1}", "COLA SATURADA / {0}/{1}", "WARTESCHLANGE GESÄTTIGT / {0}/{1}", "FILE SATURÉE / {0}/{1}", "큐 포화 / {0}/{1}"),
        new("health.replay_warning", 3, "REPLAY WARNING / {0}/{1} free / lag {2}", "重放警告 / 剩余 {0}/{1} / 滞后 {2}", "重播警告 / 剩餘 {0}/{1} / 落後 {2}", "リプレイ警告 / 空き {0}/{1} / 遅延 {2}", "AVISO DE REPRODUCCIÓN / {0}/{1} libres / retraso {2}", "REPLAY-WARNUNG / {0}/{1} frei / Rückstand {2}", "ALERTE DE RELECTURE / {0}/{1} libres / retard {2}", "재생 경고 / {0}/{1} 여유 / 지연 {2}"),
        new("health.replay_critical", 3, "REPLAY CRITICAL / {0}/{1} free / lag {2}", "重放严重 / 剩余 {0}/{1} / 滞后 {2}", "重播嚴重 / 剩餘 {0}/{1} / 落後 {2}", "リプレイ危険 / 空き {0}/{1} / 遅延 {2}", "REPRODUCCIÓN CRÍTICA / {0}/{1} libres / retraso {2}", "REPLAY KRITISCH / {0}/{1} frei / Rückstand {2}", "RELECTURE CRITIQUE / {0}/{1} libres / retard {2}", "재생 위험 / {0}/{1} 여유 / 지연 {2}"),
        new("health.replay_blocked", 3, "REPLAY BLOCKED / {0}/{1} free / lag {2}", "重放阻塞 / 剩余 {0}/{1} / 滞后 {2}", "重播阻塞 / 剩餘 {0}/{1} / 落後 {2}", "リプレイ停止 / 空き {0}/{1} / 遅延 {2}", "REPRODUCCIÓN BLOQUEADA / {0}/{1} libres / retraso {2}", "REPLAY BLOCKIERT / {0}/{1} frei / Rückstand {2}", "RELECTURE BLOQUÉE / {0}/{1} libres / retard {2}", "재생 차단 / {0}/{1} 여유 / 지연 {2}"),
        new("health.rejected", 0, "AUTHORITY / rejected", "权威端 / 已拒绝", "權威端 / 已拒絕", "権限元 / 拒否", "AUTORIDAD / rechazada", "AUTORITÄT / abgelehnt", "AUTORITÉ / rejetée", "권한 주체 / 거부됨"),
        new("health.invalid_request", 0, "AUTHORITY / invalid request", "权威端 / 请求无效", "權威端 / 要求無效", "権限元 / 無効な要求", "AUTORIDAD / solicitud no válida", "AUTORITÄT / ungültige Anfrage", "AUTORITÉ / requête invalide", "권한 주체 / 잘못된 요청"),
        new("health.invalid_response", 0, "AUTHORITY / invalid response", "权威端 / 响应无效", "權威端 / 回應無效", "権限元 / 無効な応答", "AUTORIDAD / respuesta no válida", "AUTORITÄT / ungültige Antwort", "AUTORITÉ / réponse invalide", "권한 주체 / 잘못된 응답"),
        new("health.unavailable", 0, "AUTHORITY / unavailable", "权威端 / 不可用", "權威端 / 無法使用", "権限元 / 利用不可", "AUTORIDAD / no disponible", "AUTORITÄT / nicht verfügbar", "AUTORITÉ / indisponible", "권한 주체 / 사용할 수 없음"),
        new("health.timeout", 0, "AUTHORITY / timeout", "权威端 / 超时", "權威端 / 逾時", "権限元 / タイムアウト", "AUTORIDAD / tiempo agotado", "AUTORITÄT / Zeitüberschreitung", "AUTORITÉ / délai dépassé", "권한 주체 / 시간 초과"),
        new("health.stopped", 0, "AUTHORITY / stopped", "权威端 / 已停止", "權威端 / 已停止", "権限元 / 停止", "AUTORIDAD / detenida", "AUTORITÄT / beendet", "AUTORITÉ / arrêtée", "권한 주체 / 중지됨"),
    ];

    public static IReadOnlyDictionary<string, string> English { get; } = Catalog(entry => entry.English);
    public static IReadOnlyDictionary<string, string> SimplifiedChinese { get; } = Catalog(entry => entry.SimplifiedChinese);
    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } = Catalog(entry => entry.TraditionalChinese);
    public static IReadOnlyDictionary<string, string> Japanese { get; } = Catalog(entry => entry.Japanese);
    public static IReadOnlyDictionary<string, string> Spanish { get; } = Catalog(entry => entry.Spanish);
    public static IReadOnlyDictionary<string, string> German { get; } = Catalog(entry => entry.German);
    public static IReadOnlyDictionary<string, string> French { get; } = Catalog(entry => entry.French);
    public static IReadOnlyDictionary<string, string> Korean { get; } = Catalog(entry => entry.Korean);

    public static string Resolve(DesktopLocalization localization, string key)
    {
        var fullKey = FullKey(key);
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
                "desktop remote shell localization entries drifted");
        }
        DesktopDomainCatalogContract.Verify(
            "remote shell",
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
