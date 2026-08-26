using System.Globalization;

internal static class DesktopDebuggerCatalogs
{
    private const string Prefix = "desktop.debugger.";
    public const int KeyCount = 29;

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
        new("entry.open", 0, "Debugger", "调试器", "偵錯器", "デバッガー", "Depurador", "Debugger", "Débogueur", "디버거"),
        new("a11y.entry", 0, "Open this daemon's Leselang debugger", "打开此 daemon 的 Leselang 调试器", "開啟此 daemon 的 Leselang 偵錯器", "この daemon の Leselang デバッガーを開く", "Abrir el depurador Leselang de este daemon", "Leselang-Debugger dieses Daemons öffnen", "Ouvrir le débogueur Leselang de ce daemon", "이 daemon의 Leselang 디버거 열기"),
        new("help.entry", 0, "Starts and observes bounded Leselang VM sessions owned by this daemon.", "启动并观察由此 daemon 管理的有界 Leselang VM 会话。", "啟動並觀察由此 daemon 管理的有界 Leselang VM 工作階段。", "この daemon が管理する制限付き Leselang VM セッションを開始して監視します。", "Inicia y observa sesiones acotadas de la VM Leselang gestionadas por este daemon.", "Startet und beobachtet begrenzte Leselang-VM-Sitzungen dieses Daemons.", "Démarre et observe les sessions bornées de la VM Leselang gérées par ce daemon.", "이 daemon이 관리하는 제한된 Leselang VM 세션을 시작하고 관찰합니다."),
        new("title", 1, "Debugger / {0}", "调试器 / {0}", "偵錯器 / {0}", "デバッガー / {0}", "Depurador / {0}", "Debugger / {0}", "Débogueur / {0}", "디버거 / {0}"),
        new("heading", 0, "Leselang VM session", "Leselang VM 会话", "Leselang VM 工作階段", "Leselang VM セッション", "Sesión de VM Leselang", "Leselang-VM-Sitzung", "Session de VM Leselang", "Leselang VM 세션"),
        new("body", 0, "Run only to the first effect, inspect the Rust projection, then explicitly review cancellation. Effects are never executed here.", "仅运行到第一个副作用，检查 Rust 投影，再明确审查取消操作；此处绝不会执行副作用。", "僅執行到第一個副作用，檢查 Rust 投影，再明確審查取消操作；此處絕不會執行副作用。", "最初のエフェクトまでだけ実行し、Rust 投影を確認してからキャンセルを明示的にレビューします。ここではエフェクトを実行しません。", "Ejecuta solo hasta el primer efecto, inspecciona la proyección Rust y revisa explícitamente la cancelación. Aquí nunca se ejecutan efectos.", "Führt nur bis zum ersten Effekt aus, prüft die Rust-Projektion und bestätigt den Abbruch ausdrücklich. Effekte werden hier nie ausgeführt.", "S’exécute uniquement jusqu’au premier effet, inspecte la projection Rust puis exige une validation explicite de l’annulation. Aucun effet n’est exécuté ici.", "첫 번째 효과까지만 실행하고 Rust 프로젝션을 확인한 뒤 취소를 명시적으로 검토합니다. 여기서는 효과를 실행하지 않습니다."),
        new("label.session", 0, "Session ID", "会话 ID", "工作階段 ID", "セッション ID", "ID de sesión", "Sitzungs-ID", "ID de session", "세션 ID"),
        new("label.timeout", 0, "Timeout (ms)", "超时（毫秒）", "逾時（毫秒）", "タイムアウト (ms)", "Tiempo límite (ms)", "Zeitlimit (ms)", "Délai (ms)", "시간 제한(ms)"),
        new("label.source", 0, "Leselang source", "Leselang 源码", "Leselang 原始碼", "Leselang ソース", "Código Leselang", "Leselang-Quelltext", "Source Leselang", "Leselang 소스"),
        new("action.start", 0, "Start suspended session", "启动挂起会话", "啟動暫停工作階段", "中断セッションを開始", "Iniciar sesión suspendida", "Angehaltene Sitzung starten", "Démarrer la session suspendue", "일시 중단 세션 시작"),
        new("action.new", 0, "New session", "新建会话", "新增工作階段", "新しいセッション", "Nueva sesión", "Neue Sitzung", "Nouvelle session", "새 세션"),
        new("action.refresh", 0, "Refresh projection", "刷新投影", "重新整理投影", "投影を更新", "Actualizar proyección", "Projektion aktualisieren", "Actualiser la projection", "프로젝션 새로 고침"),
        new("status.ready", 0, "Ready. The daemon will stop before executing the first effect.", "就绪。daemon 会在执行第一个副作用前停止。", "就緒。daemon 會在執行第一個副作用前停止。", "準備完了。daemon は最初のエフェクトを実行する前に停止します。", "Listo. El daemon se detendrá antes de ejecutar el primer efecto.", "Bereit. Der Daemon stoppt vor der Ausführung des ersten Effekts.", "Prêt. Le daemon s’arrêtera avant d’exécuter le premier effet.", "준비되었습니다. daemon은 첫 번째 효과를 실행하기 전에 중지합니다."),
        new("status.starting", 0, "Starting a bounded VM session...", "正在启动有界 VM 会话...", "正在啟動有界 VM 工作階段...", "制限付き VM セッションを開始中...", "Iniciando una sesión de VM acotada...", "Begrenzte VM-Sitzung wird gestartet...", "Démarrage d’une session de VM bornée...", "제한된 VM 세션을 시작하는 중..."),
        new("status.started", 2, "Session {0} suspended at revision {1}.", "会话 {0} 已在修订 {1} 处挂起。", "工作階段 {0} 已在修訂 {1} 處暫停。", "セッション {0} はリビジョン {1} で中断しました。", "La sesión {0} se suspendió en la revisión {1}.", "Sitzung {0} wurde bei Revision {1} angehalten.", "La session {0} est suspendue à la révision {1}.", "세션 {0}이 리비전 {1}에서 일시 중단되었습니다."),
        new("status.refreshing", 0, "Refreshing the authoritative projection...", "正在刷新权威投影...", "正在重新整理權威投影...", "権威投影を更新中...", "Actualizando la proyección autoritativa...", "Autoritative Projektion wird aktualisiert...", "Actualisation de la projection d’autorité...", "권한 프로젝션을 새로 고치는 중..."),
        new("status.refreshed", 2, "Session {0} refreshed at revision {1}.", "会话 {0} 已刷新到修订 {1}。", "工作階段 {0} 已重新整理至修訂 {1}。", "セッション {0} をリビジョン {1} に更新しました。", "La sesión {0} se actualizó en la revisión {1}.", "Sitzung {0} wurde auf Revision {1} aktualisiert.", "La session {0} a été actualisée à la révision {1}.", "세션 {0}을 리비전 {1}로 새로 고쳤습니다."),
        new("status.planning", 0, "Requesting a side-effect-free cancellation plan...", "正在请求无副作用的取消预演...", "正在請求無副作用的取消預演...", "副作用のないキャンセル計画を要求中...", "Solicitando un plan de cancelación sin efectos...", "Nebenwirkungsfreier Abbruchplan wird angefordert...", "Demande d’un plan d’annulation sans effet...", "부작용 없는 취소 계획을 요청하는 중..."),
        new("status.plan_ready", 0, "Cancellation was planned without changing VM state.", "取消已完成预演，VM 状态未改变。", "取消已完成預演，VM 狀態未變更。", "VM 状態を変更せずキャンセルを計画しました。", "La cancelación se planificó sin cambiar el estado de la VM.", "Der Abbruch wurde ohne Änderung des VM-Zustands geplant.", "L’annulation a été planifiée sans modifier l’état de la VM.", "VM 상태를 변경하지 않고 취소를 계획했습니다."),
        new("status.cancelled", 2, "Session {0} cancelled and audited at {1}.", "会话 {0} 已取消，并于 {1} 写入审计。", "工作階段 {0} 已取消，並於 {1} 寫入稽核。", "セッション {0} をキャンセルし、{1} に監査記録を保存しました。", "La sesión {0} se canceló y auditó en {1}.", "Sitzung {0} wurde abgebrochen und um {1} auditiert.", "La session {0} a été annulée et auditée à {1}.", "세션 {0}이 취소되었고 {1}에 감사 기록이 저장되었습니다."),
        new("status.cancel_dismissed", 0, "Cancellation review dismissed; VM state is unchanged.", "已关闭取消审查；VM 状态未改变。", "已關閉取消審查；VM 狀態未變更。", "キャンセルのレビューを閉じました。VM 状態は変更されていません。", "Se cerró la revisión de cancelación; el estado de la VM no cambió.", "Abbruchprüfung geschlossen; der VM-Zustand blieb unverändert.", "La validation de l’annulation a été fermée ; l’état de la VM est inchangé.", "취소 검토를 닫았습니다. VM 상태는 변경되지 않았습니다."),
        new("status.failed", 1, "Debugger operation failed: {0}", "调试器操作失败：{0}", "偵錯器操作失敗：{0}", "デバッガー操作に失敗しました: {0}", "Falló la operación del depurador: {0}", "Debugger-Vorgang fehlgeschlagen: {0}", "Échec de l’opération du débogueur : {0}", "디버거 작업 실패: {0}"),
        new("confirm.title", 0, "Confirm VM cancellation", "确认取消 VM", "確認取消 VM", "VM キャンセルを確認", "Confirmar cancelación de VM", "VM-Abbruch bestätigen", "Confirmer l’annulation de la VM", "VM 취소 확인"),
        new("confirm.body", 2, "Cancel pending effect {0} in session {1}?", "取消会话 {1} 中待处理的副作用 {0}？", "取消工作階段 {1} 中待處理的副作用 {0}？", "セッション {1} の保留中エフェクト {0} をキャンセルしますか？", "¿Cancelar el efecto pendiente {0} de la sesión {1}?", "Ausstehenden Effekt {0} in Sitzung {1} abbrechen?", "Annuler l’effet en attente {0} de la session {1} ?", "세션 {1}의 대기 중인 효과 {0}을 취소할까요?"),
        new("confirm.warning", 0, "This consumes the suspended continuation. The daemon will write a command-correlated VM audit record.", "这会消耗挂起的 continuation，daemon 将写入与命令关联的 VM 审计记录。", "這會消耗暫停的 continuation，daemon 將寫入與命令關聯的 VM 稽核記錄。", "中断中の continuation を消費し、daemon はコマンドに関連付けた VM 監査記録を書き込みます。", "Esto consume la continuación suspendida. El daemon escribirá un registro de auditoría de VM correlacionado con el comando.", "Dies verbraucht die angehaltene Fortsetzung. Der Daemon schreibt einen befehlsbezogenen VM-Auditdatensatz.", "Cela consomme la continuation suspendue. Le daemon écrira un audit VM corrélé à la commande.", "일시 중단된 continuation을 소비하며 daemon은 명령과 연계된 VM 감사 기록을 씁니다."),
        new("confirm.apply", 0, "Cancel and audit", "取消并审计", "取消並稽核", "キャンセルして監査", "Cancelar y auditar", "Abbrechen und auditieren", "Annuler et auditer", "취소 및 감사"),
        new("projection.empty", 0, "Start or refresh a session to mount its Rust-authored debugger document.", "启动或刷新会话，以挂载由 Rust 生成的调试器文档。", "啟動或重新整理工作階段，以掛載由 Rust 產生的偵錯器文件。", "セッションを開始または更新して、Rust が生成したデバッガー文書を表示します。", "Inicia o actualiza una sesión para montar su documento de depuración generado por Rust.", "Starten oder aktualisieren Sie eine Sitzung, um ihr Rust-erzeugtes Debugger-Dokument anzuzeigen.", "Démarrez ou actualisez une session pour afficher son document de débogage produit par Rust.", "세션을 시작하거나 새로 고쳐 Rust가 생성한 디버거 문서를 표시합니다."),
        new("availability.busy", 0, "Another debugger operation is in progress.", "另一个调试器操作正在进行。", "另一個偵錯器操作正在進行。", "別のデバッガー操作が進行中です。", "Hay otra operación del depurador en curso.", "Ein anderer Debugger-Vorgang läuft bereits.", "Une autre opération du débogueur est en cours.", "다른 디버거 작업이 진행 중입니다."),
        new("availability.terminal", 0, "This debugger session is no longer waiting for an effect.", "此调试器会话已不再等待副作用。", "此偵錯器工作階段已不再等待副作用。", "このデバッガーセッションはエフェクトを待機していません。", "Esta sesión de depuración ya no espera un efecto.", "Diese Debugger-Sitzung wartet nicht mehr auf einen Effekt.", "Cette session de débogage n’attend plus d’effet.", "이 디버거 세션은 더 이상 효과를 기다리지 않습니다."),
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
        var fullKey = $"{Prefix}{key}";
        if (!English.ContainsKey(fullKey))
        {
            throw new InvalidDataException(
                $"desktop debugger localization key is unknown: {key}");
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
                "desktop debugger localization entries drifted");
        }
        DesktopDomainCatalogContract.Verify(
            "debugger",
            KeyCount,
            All,
            Entries.Where(entry => entry.Arity > 0).ToDictionary(
                entry => $"{Prefix}{entry.Key}",
                entry => entry.Arity,
                StringComparer.Ordinal));
    }

    private static IReadOnlyList<IReadOnlyDictionary<string, string>> All =>
        [English, SimplifiedChinese, TraditionalChinese, Japanese, Spanish, German, French, Korean];

    private static IReadOnlyDictionary<string, string> Catalog(
        Func<Entry, string> value) => Entries.ToDictionary(
            entry => $"{Prefix}{entry.Key}",
            value,
            StringComparer.Ordinal);
}
