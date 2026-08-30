using System.Globalization;

internal static class DesktopHubCatalogs
{
    private const string Prefix = "desktop.hub.";
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
        new("a11y.add_daemon", 0, "Add a leserpent daemon connection", "添加 leserpent daemon 连接", "新增 leserpent daemon 連線", "leserpent daemon 接続を追加", "Añadir una conexión daemon de leserpent", "Leserpent-Daemon-Verbindung hinzufügen", "Ajouter une connexion daemon leserpent", "leserpent daemon 연결 추가"),
        new("a11y.deploy_daemon", 0, "Deploy a leserpent daemon to a target host", "将 leserpent daemon 部署到目标主机", "將 leserpent daemon 部署到目標主機", "対象ホストに leserpent daemon をデプロイ", "Desplegar un daemon de leserpent en un host de destino", "Leserpent-Daemon auf einem Zielhost bereitstellen", "Déployer un daemon leserpent sur un hôte cible", "대상 호스트에 leserpent daemon 배포"),
        new("a11y.retire_daemon", 0, "Retire a daemon service through its original bootstrap authority", "通过原始 bootstrap 权威端退役 daemon 服务", "透過原始 bootstrap 權威端退役 daemon 服務", "元の bootstrap 権限元を通じて daemon サービスを廃止", "Retirar un servicio daemon mediante su autoridad bootstrap original", "Daemon-Dienst über seine ursprüngliche Bootstrap-Autorität stilllegen", "Retirer un service daemon via son autorité bootstrap d’origine", "원래 bootstrap 권한 주체를 통해 daemon 서비스 폐기"),
        new("a11y.provision_runtime", 0, "Provision a gewyvern runtime through a daemon authority", "通过 daemon 权威端部署 gewyvern runtime", "透過 daemon 權威端佈建 gewyvern runtime", "daemon 権限元を通じて gewyvern runtime をプロビジョニング", "Aprovisionar un runtime gewyvern mediante una autoridad daemon", "Gewyvern-Runtime über eine Daemon-Autorität bereitstellen", "Provisionner un runtime gewyvern via une autorité daemon", "daemon 권한 주체를 통해 gewyvern runtime 프로비저닝"),
        new("a11y.retire_runtime", 0, "Retire a gewyvern runtime through its daemon authority", "通过所属 daemon 权威端退役 gewyvern runtime", "透過所屬 daemon 權威端退役 gewyvern runtime", "所属する daemon 権限元を通じて gewyvern runtime を廃止", "Retirar un runtime gewyvern mediante su autoridad daemon", "Gewyvern-Runtime über ihre Daemon-Autorität stilllegen", "Retirer un runtime gewyvern via son autorité daemon", "소속 daemon 권한 주체를 통해 gewyvern runtime 폐기"),
        new("a11y.tutorial", 0, "Open the Leserpent quick tour", "打开 Leserpent 快速教学", "開啟 Leserpent 快速教學", "Leserpent クイックツアーを開く", "Abrir el recorrido rápido de Leserpent", "Leserpent-Kurztour öffnen", "Ouvrir la visite rapide de Leserpent", "Leserpent 빠른 둘러보기 열기"),
        new("help.tutorial", 0, "Opens the offline, read-only Learning Center. Shortcut: F1.", "打开离线只读的学习中心。快捷键：F1。", "開啟離線唯讀的學習中心。快速鍵：F1。", "オフラインで読み取り専用のラーニングセンターを開きます。ショートカット: F1。", "Abre el Centro de aprendizaje sin conexión y de solo lectura. Atajo: F1.", "Öffnet das offline verfügbare, schreibgeschützte Lerncenter. Tastenkürzel: F1.", "Ouvre le centre d’apprentissage hors ligne et en lecture seule. Raccourci : F1.", "오프라인 읽기 전용 학습 센터를 엽니다. 단축키: F1."),
        new("tooltip.tutorial", 0, "Open Learning Center (F1)", "打开学习中心（F1）", "開啟學習中心（F1）", "ラーニングセンターを開く (F1)", "Abrir el Centro de aprendizaje (F1)", "Lerncenter öffnen (F1)", "Ouvrir le centre d’apprentissage (F1)", "학습 센터 열기(F1)"),
        new("onboarding.title", 0, "Choose where to begin", "选择从哪里开始", "選擇從哪裡開始", "どこから始めるか選択", "Elige por dónde empezar", "Wählen Sie Ihren Einstieg", "Choisissez par où commencer", "시작할 위치 선택"),
        new("onboarding.body", 0, "New here? Take the quick tour, then open Local Orchestra when available. Add daemon connects an existing service; Deploy daemon installs one on another host. Local and self-hosted use needs no account.", "第一次使用？先看快速教学，然后在可用时打开本地 Orchestra。添加 daemon 用于连接已有服务；部署 daemon 用于在另一台主机安装服务。本地与自托管功能无需登录账号。", "第一次使用？先看快速導覽，然後在可用時開啟本機 Orchestra。新增 daemon 用於連接既有服務；部署 daemon 用於在另一台主機安裝服務。本機與自架功能無需登入帳號。", "初めての場合はクイックツアーを確認し、利用可能ならローカル Orchestra を開いてください。daemon の追加は既存サービスへの接続、daemon のデプロイは別ホストへのインストールです。ローカルとセルフホストの利用にアカウントは不要です。", "¿Es tu primera vez? Consulta el recorrido rápido y abre Orchestra local cuando esté disponible. Añadir daemon conecta un servicio existente; Desplegar daemon instala uno en otro host. El uso local y autohospedado no requiere cuenta.", "Neu hier? Sehen Sie sich die Kurztour an und öffnen Sie anschließend die lokale Orchestra, sofern verfügbar. Daemon hinzufügen verbindet einen vorhandenen Dienst; Daemon bereitstellen installiert ihn auf einem anderen Host. Für lokale und selbst gehostete Nutzung ist kein Konto erforderlich.", "Première utilisation ? Consultez la visite rapide, puis ouvrez Orchestra locale lorsqu’elle est disponible. Ajouter un daemon connecte un service existant ; Déployer un daemon en installe un sur un autre hôte. L’usage local et auto-hébergé ne nécessite aucun compte.", "처음이라면 빠른 둘러보기를 확인한 다음 사용 가능한 경우 로컬 Orchestra를 여세요. daemon 추가는 기존 서비스에 연결하고 daemon 배포는 다른 호스트에 서비스를 설치합니다. 로컬 및 자체 호스팅 사용에는 계정이 필요하지 않습니다."),
        new("onboarding.advanced", 0, "Advanced lifecycle actions", "进阶生命周期操作", "進階生命週期操作", "高度なライフサイクル操作", "Acciones avanzadas del ciclo de vida", "Erweiterte Lebenszyklusaktionen", "Actions avancées du cycle de vie", "고급 수명 주기 작업"),
        new("help.filter", 0, "Filters the in-memory authority and runtime topology without contacting a daemon. Shortcut: Control or Command plus F.", "筛选内存中的权威端与 runtime 拓扑，不会联系 daemon。快捷键：Control 或 Command + F。", "篩選記憶體中的權威端與 runtime 拓撲，不會聯絡 daemon。快速鍵：Control 或 Command + F。", "メモリ内の権限元と runtime トポロジを、daemon に接続せず絞り込みます。ショートカット: Control または Command + F。", "Filtra la topología de autoridades y runtimes en memoria sin contactar con un daemon. Atajo: Control o Command más F.", "Filtert die Autoritäts- und Runtime-Topologie im Speicher, ohne einen Daemon zu kontaktieren. Tastenkürzel: Strg oder Command plus F.", "Filtre la topologie des autorités et runtimes en mémoire sans contacter de daemon. Raccourci : Contrôle ou Commande plus F.", "daemon에 연결하지 않고 메모리의 권한 주체 및 runtime 토폴로지를 필터링합니다. 단축키: Control 또는 Command + F."),
        new("a11y.filter_summary", 0, "Topology filter result count", "拓扑筛选结果数量", "拓撲篩選結果數量", "トポロジフィルターの結果数", "Cantidad de resultados del filtro de topología", "Ergebnisanzahl des Topologiefilters", "Nombre de résultats du filtre de topologie", "토폴로지 필터 결과 수"),
        new("help.refresh_all", 0, "Refreshes every daemon authority and joins an existing refresh instead of starting duplicate work. Shortcut: F5.", "刷新每个 daemon 权威端；如已有刷新则加入，而不会启动重复任务。快捷键：F5。", "重新整理每個 daemon 權威端；如已有重新整理則加入，而不會啟動重複工作。快速鍵：F5。", "すべての daemon 権限元を更新し、重複処理を開始せず既存の更新に参加します。ショートカット: F5。", "Actualiza cada autoridad daemon y se une a una actualización existente en vez de iniciar trabajo duplicado. Atajo: F5.", "Aktualisiert jede Daemon-Autorität und schließt sich einer laufenden Aktualisierung an, statt doppelte Arbeit zu starten. Tastenkürzel: F5.", "Actualise chaque autorité daemon et rejoint une actualisation existante au lieu de dupliquer le travail. Raccourci : F5.", "모든 daemon 권한 주체를 새로 고치며 중복 작업 대신 기존 새로 고침에 참여합니다. 단축키: F5."),
        new("tooltip.refresh_all", 0, "Refresh all daemon topologies (F5)", "刷新所有 daemon 拓扑（F5）", "重新整理所有 daemon 拓撲（F5）", "すべての daemon トポロジを更新 (F5)", "Actualizar todas las topologías daemon (F5)", "Alle Daemon-Topologien aktualisieren (F5)", "Actualiser toutes les topologies daemon (F5)", "모든 daemon 토폴로지 새로 고침(F5)"),
        new("status.ready", 1, "Topology ready / daemon authorities: {0}.", "拓扑就绪 / daemon 权威端：{0}。", "拓撲就緒 / daemon 權威端：{0}。", "トポロジ準備完了 / daemon 権限元: {0}。", "Topología lista / autoridades daemon: {0}.", "Topologie bereit / Daemon-Autoritäten: {0}.", "Topologie prête / autorités daemon : {0}.", "토폴로지 준비됨 / daemon 권한 주체: {0}."),
        new("a11y.status", 0, "Hub topology status", "Hub 拓扑状态", "Hub 拓撲狀態", "Hub トポロジ状態", "Estado de la topología del Hub", "Status der Hub-Topologie", "État de la topologie du Hub", "Hub 토폴로지 상태"),
        new("a11y.status_value", 1, "Hub topology status: {0}", "Hub 拓扑状态：{0}", "Hub 拓撲狀態：{0}", "Hub トポロジ状態: {0}", "Estado de la topología del Hub: {0}", "Status der Hub-Topologie: {0}", "État de la topologie du Hub : {0}", "Hub 토폴로지 상태: {0}"),
        new("client.title", 0, "Leserpent Desktop", "Leserpent 桌面端", "Leserpent 桌面端", "Leserpent デスクトップ", "Escritorio Leserpent", "Leserpent Desktop", "Bureau Leserpent", "Leserpent 데스크톱"),
        new("client.subtitle", 0, "Topology root / operator session", "拓扑根节点 / 操作员会话", "拓撲根節點 / 操作員工作階段", "トポロジルート / オペレーターセッション", "Raíz de topología / sesión del operador", "Topologiewurzel / Operatorsitzung", "Racine de topologie / session opérateur", "토폴로지 루트 / 운영자 세션"),
        new("client.daemon_count", 1, "DAEMONS {0}", "DAEMON {0}", "DAEMON {0}", "DAEMON {0}", "DAEMONS {0}", "DAEMONS {0}", "DAEMONS {0}", "DAEMON {0}"),
        new("kind.remote", 0, "REMOTE", "远程", "遠端", "リモート", "REMOTO", "REMOTE", "DISTANT", "원격"),
        new("a11y.open_daemon", 1, "Open daemon {0}", "打开 daemon {0}", "開啟 daemon {0}", "daemon {0} を開く", "Abrir el daemon {0}", "Daemon {0} öffnen", "Ouvrir le daemon {0}", "daemon {0} 열기"),
        new("a11y.refresh_daemon", 1, "Refresh runtime topology for daemon {0}", "刷新 daemon {0} 的 runtime 拓扑", "重新整理 daemon {0} 的 runtime 拓撲", "daemon {0} の runtime トポロジを更新", "Actualizar la topología de runtimes del daemon {0}", "Runtime-Topologie für Daemon {0} aktualisieren", "Actualiser la topologie des runtimes du daemon {0}", "daemon {0}의 runtime 토폴로지 새로 고침"),
        new("a11y.manage_daemon", 1, "Manage daemon {0}", "管理 daemon {0}", "管理 daemon {0}", "daemon {0} を管理", "Administrar el daemon {0}", "Daemon {0} verwalten", "Gérer le daemon {0}", "daemon {0} 관리"),
        new("summary.awaiting", 0, "RUNTIMES / awaiting topology", "RUNTIME / 等待拓扑", "RUNTIME / 等待拓撲", "RUNTIME / トポロジ待機中", "RUNTIMES / esperando topología", "RUNTIMES / Topologie ausstehend", "RUNTIMES / topologie en attente", "RUNTIME / 토폴로지 대기 중"),
        new("a11y.summary.awaiting", 1, "Runtime topology for daemon {0} is awaiting refresh", "daemon {0} 的 runtime 拓扑正在等待刷新", "daemon {0} 的 runtime 拓撲正在等待重新整理", "daemon {0} の runtime トポロジは更新待ちです", "La topología de runtimes del daemon {0} está esperando actualización", "Runtime-Topologie für Daemon {0} wartet auf Aktualisierung", "La topologie des runtimes du daemon {0} attend une actualisation", "daemon {0}의 runtime 토폴로지가 새로 고침을 기다리는 중입니다"),
        new("authority.awaiting", 0, "AUTHORITY / awaiting proof", "权威端 / 等待证明", "權威端 / 等待證明", "権限元 / 証明待ち", "AUTORIDAD / esperando prueba", "AUTORITÄT / Nachweis ausstehend", "AUTORITÉ / preuve en attente", "권한 주체 / 증명 대기 중"),
        new("a11y.authority.awaiting", 1, "Authority health for daemon {0} is awaiting proof", "daemon {0} 的权威端健康状态正在等待证明", "daemon {0} 的權威端健康狀態正在等待證明", "daemon {0} の権限元ヘルスは証明待ちです", "La salud de la autoridad del daemon {0} está esperando prueba", "Autoritätszustand für Daemon {0} wartet auf Nachweis", "La santé de l’autorité du daemon {0} attend une preuve", "daemon {0}의 권한 주체 상태가 증명을 기다리는 중입니다"),
        new("runtime.loading", 0, "Loading bounded runtime summary...", "正在载入有边界的 runtime 摘要...", "正在載入有界限的 runtime 摘要...", "有界な runtime サマリーを読み込み中...", "Cargando el resumen acotado de runtimes...", "Begrenzte Runtime-Zusammenfassung wird geladen...", "Chargement du résumé borné des runtimes...", "제한된 runtime 요약 로드 중..."),
        new("retained.message", 1, "Refresh failed {0} time(s). Retaining the last known topology; workspace launch still requires a live daemon snapshot.", "刷新失败 {0} 次。保留最后已知拓扑；打开工作区仍需要实时 daemon 快照。", "重新整理失敗 {0} 次。保留最後已知拓撲；開啟工作區仍需要即時 daemon 快照。", "更新に {0} 回失敗しました。最後の既知トポロジを保持します。ワークスペース起動には引き続きライブ daemon スナップショットが必要です。", "La actualización falló {0} vez/veces. Se conserva la última topología conocida; abrir un espacio aún requiere una instantánea daemon en vivo.", "Aktualisierung {0}-mal fehlgeschlagen. Die letzte bekannte Topologie bleibt erhalten; zum Öffnen eines Arbeitsbereichs ist weiterhin ein Live-Daemon-Snapshot erforderlich.", "L’actualisation a échoué {0} fois. La dernière topologie connue est conservée ; l’ouverture d’un espace exige toujours un instantané daemon actif.", "새로 고침이 {0}회 실패했습니다. 마지막으로 알려진 토폴로지를 유지하며 작업 공간 실행에는 여전히 실시간 daemon 스냅샷이 필요합니다."),
        new("filter.active", 4, "Daemons: {0}/{1} / runtimes: {2}/{3}", "daemon：{0}/{1} / runtime：{2}/{3}", "daemon：{0}/{1} / runtime：{2}/{3}", "daemon: {0}/{1} / runtime: {2}/{3}", "Daemons: {0}/{1} / runtimes: {2}/{3}", "Daemons: {0}/{1} / Runtimes: {2}/{3}", "Daemons : {0}/{1} / runtimes : {2}/{3}", "daemon: {0}/{1} / runtime: {2}/{3}"),
        new("filter.loading", 1, "Daemon authorities: {0} / topology loading", "daemon 权威端：{0} / 正在载入拓扑", "daemon 權威端：{0} / 正在載入拓撲", "daemon 権限元: {0} / トポロジ読み込み中", "Autoridades daemon: {0} / cargando topología", "Daemon-Autoritäten: {0} / Topologie wird geladen", "Autorités daemon : {0} / chargement de la topologie", "daemon 권한 주체: {0} / 토폴로지 로드 중"),
        new("filter.all", 2, "Daemon authorities: {0} / runtimes: {1}", "daemon 权威端：{0} / runtime：{1}", "daemon 權威端：{0} / runtime：{1}", "daemon 権限元: {0} / runtime: {1}", "Autoridades daemon: {0} / runtimes: {1}", "Daemon-Autoritäten: {0} / Runtimes: {1}", "Autorités daemon : {0} / runtimes : {1}", "daemon 권한 주체: {0} / runtime: {1}"),
        new("a11y.filter.active", 4, "Showing {0} of {1} daemon authorities and {2} of {3} runtimes", "显示 {1} 个 daemon 权威端中的 {0} 个，以及 {3} 个 runtime 中的 {2} 个", "顯示 {1} 個 daemon 權威端中的 {0} 個，以及 {3} 個 runtime 中的 {2} 個", "daemon 権限元 {1} 件中 {0} 件、runtime {3} 件中 {2} 件を表示", "Mostrando {0} de {1} autoridades daemon y {2} de {3} runtimes", "{0} von {1} Daemon-Autoritäten und {2} von {3} Runtimes werden angezeigt", "Affichage de {0} autorités daemon sur {1} et de {2} runtimes sur {3}", "daemon 권한 주체 {1}개 중 {0}개와 runtime {3}개 중 {2}개 표시"),
        new("a11y.filter.all", 2, "Showing all {0} daemon authorities and {1} runtimes", "显示全部 {0} 个 daemon 权威端和 {1} 个 runtime", "顯示全部 {0} 個 daemon 權威端和 {1} 個 runtime", "全 {0} 件の daemon 権限元と {1} 件の runtime を表示", "Mostrando las {0} autoridades daemon y los {1} runtimes", "Alle {0} Daemon-Autoritäten und {1} Runtimes werden angezeigt", "Affichage des {0} autorités daemon et des {1} runtimes", "daemon 권한 주체 {0}개와 runtime {1}개 모두 표시"),
        new("status.refreshing_all", 1, "Refreshing {0} daemon topologies...", "正在刷新 {0} 个 daemon 拓扑...", "正在重新整理 {0} 個 daemon 拓撲...", "{0} 件の daemon トポロジを更新中...", "Actualizando {0} topologías daemon...", "{0} Daemon-Topologien werden aktualisiert...", "Actualisation de {0} topologies daemon...", "daemon 토폴로지 {0}개 새로 고침 중..."),
        new("action.refreshing", 0, "Refreshing...", "刷新中...", "重新整理中...", "更新中...", "Actualizando...", "Wird aktualisiert...", "Actualisation...", "새로 고침 중..."),
        new("a11y.refreshing_all", 0, "Refreshing all daemon topologies", "正在刷新所有 daemon 拓扑", "正在重新整理所有 daemon 拓撲", "すべての daemon トポロジを更新中", "Actualizando todas las topologías daemon", "Alle Daemon-Topologien werden aktualisiert", "Actualisation de toutes les topologies daemon", "모든 daemon 토폴로지 새로 고침 중"),
        new("status.refresh_attention", 3, "Topology refresh complete with attention: {0} live, {1} stale, {2} unavailable.", "拓扑刷新完成，需要关注：{0} 个实时、{1} 个陈旧、{2} 个不可用。", "拓撲重新整理完成，需要注意：{0} 個即時、{1} 個過時、{2} 個不可用。", "トポロジ更新が要確認で完了: ライブ {0}、古い {1}、利用不可 {2}。", "Actualización de topología completada con atención: {0} activas, {1} obsoletas, {2} no disponibles.", "Topologieaktualisierung mit Handlungsbedarf abgeschlossen: {0} live, {1} veraltet, {2} nicht verfügbar.", "Actualisation de la topologie terminée avec attention : {0} actives, {1} obsolètes, {2} indisponibles.", "토폴로지 새로 고침 완료, 확인 필요: 실시간 {0}, 오래됨 {1}, 사용 불가 {2}."),
        new("status.refresh_complete", 1, "Topology refresh complete: {0} daemon authorities live.", "拓扑刷新完成：{0} 个 daemon 权威端实时可用。", "拓撲重新整理完成：{0} 個 daemon 權威端即時可用。", "トポロジ更新完了: {0} 件の daemon 権限元がライブです。", "Actualización de topología completada: {0} autoridades daemon activas.", "Topologieaktualisierung abgeschlossen: {0} Daemon-Autoritäten live.", "Actualisation de la topologie terminée : {0} autorités daemon actives.", "토폴로지 새로 고침 완료: daemon 권한 주체 {0}개 실시간."),
        new("summary.loading", 0, "RUNTIMES / loading", "RUNTIME / 载入中", "RUNTIME / 載入中", "RUNTIME / 読み込み中", "RUNTIMES / cargando", "RUNTIMES / Laden", "RUNTIMES / chargement", "RUNTIME / 로드 중"),
        new("summary.refreshing", 1, "RUNTIMES / refreshing / REV {0}", "RUNTIME / 刷新中 / 修订 {0}", "RUNTIME / 重新整理中 / 修訂 {0}", "RUNTIME / 更新中 / REV {0}", "RUNTIMES / actualizando / REV {0}", "RUNTIMES / Aktualisierung / REV {0}", "RUNTIMES / actualisation / RÉV {0}", "RUNTIME / 새로 고침 중 / REV {0}"),
        new("a11y.summary.loading", 1, "Loading runtime topology for daemon {0}", "正在载入 daemon {0} 的 runtime 拓扑", "正在載入 daemon {0} 的 runtime 拓撲", "daemon {0} の runtime トポロジを読み込み中", "Cargando la topología de runtimes del daemon {0}", "Runtime-Topologie für Daemon {0} wird geladen", "Chargement de la topologie des runtimes du daemon {0}", "daemon {0}의 runtime 토폴로지 로드 중"),
        new("status.refresh_unexpected", 0, "Topology refresh stopped unexpectedly. Retry or open the daemon session for diagnostics.", "拓扑刷新意外停止。请重试，或打开 daemon 会话查看诊断。", "拓撲重新整理非預期停止。請重試，或開啟 daemon 工作階段查看診斷。", "トポロジ更新が予期せず停止しました。再試行するか、診断のため daemon セッションを開いてください。", "La actualización de topología se detuvo inesperadamente. Reintenta o abre la sesión daemon para obtener diagnósticos.", "Topologieaktualisierung wurde unerwartet beendet. Erneut versuchen oder die Daemon-Sitzung zur Diagnose öffnen.", "L’actualisation de la topologie s’est arrêtée de manière inattendue. Réessayez ou ouvrez la session daemon pour le diagnostic.", "토폴로지 새로 고침이 예기치 않게 중지되었습니다. 다시 시도하거나 진단을 위해 daemon 세션을 여세요."),
        new("phase.live", 0, "LIVE", "实时", "即時", "ライブ", "ACTIVA", "LIVE", "ACTIVE", "실시간"),
        new("phase.cached", 0, "CACHED", "缓存", "快取", "キャッシュ", "EN CACHÉ", "ZWISCHENGESPEICHERT", "EN CACHE", "캐시됨"),
        new("phase.retained", 0, "RETAINED", "保留", "保留", "保持", "CONSERVADA", "BEIBEHALTEN", "CONSERVÉE", "유지됨"),
        new("summary.full", 3, "RUNTIMES / {0} / REV {1} / {2}", "RUNTIME / {0} / 修订 {1} / {2}", "RUNTIME / {0} / 修訂 {1} / {2}", "RUNTIME / {0} / REV {1} / {2}", "RUNTIMES / {0} / REV {1} / {2}", "RUNTIMES / {0} / REV {1} / {2}", "RUNTIMES / {0} / RÉV {1} / {2}", "RUNTIME / {0} / REV {1} / {2}"),
        new("summary.filtered", 4, "RUNTIMES / {0} / REV {1} / {2} OF {3}", "RUNTIME / {0} / 修订 {1} / {2}/{3}", "RUNTIME / {0} / 修訂 {1} / {2}/{3}", "RUNTIME / {0} / REV {1} / {2}/{3}", "RUNTIMES / {0} / REV {1} / {2} DE {3}", "RUNTIMES / {0} / REV {1} / {2} VON {3}", "RUNTIMES / {0} / RÉV {1} / {2} SUR {3}", "RUNTIME / {0} / REV {1} / {2}/{3}"),
        new("a11y.summary", 4, "Daemon {0} has {1} runtimes at revision {2}, phase {3}", "daemon {0} 在修订 {2} 有 {1} 个 runtime，阶段 {3}", "daemon {0} 在修訂 {2} 有 {1} 個 runtime，階段 {3}", "daemon {0} はリビジョン {2} に {1} 件の runtime、フェーズ {3}", "El daemon {0} tiene {1} runtimes en la revisión {2}, fase {3}", "Daemon {0} hat {1} Runtimes bei Revision {2}, Phase {3}", "Le daemon {0} possède {1} runtimes à la révision {2}, phase {3}", "daemon {0}은(는) 리비전 {2}에 runtime {1}개, 단계 {3}"),
        new("runtime.empty", 0, "No gewyvern runtimes are registered under this daemon.", "此 daemon 下没有注册 gewyvern runtime。", "此 daemon 下沒有註冊 gewyvern runtime。", "この daemon 配下に登録された gewyvern runtime はありません。", "No hay runtimes gewyvern registrados en este daemon.", "Unter diesem Daemon sind keine gewyvern-Runtimes registriert.", "Aucun runtime gewyvern n’est enregistré sous ce daemon.", "이 daemon 아래에 등록된 gewyvern runtime이 없습니다."),
        new("runtime.more", 1, "+ {0} more runtimes in the daemon session", "+ daemon 会话中还有 {0} 个 runtime", "+ daemon 工作階段中還有 {0} 個 runtime", "+ daemon セッションにさらに {0} 件の runtime", "+ {0} runtimes más en la sesión daemon", "+ {0} weitere Runtimes in der Daemon-Sitzung", "+ {0} runtimes supplémentaires dans la session daemon", "+ daemon 세션에 runtime {0}개 더 있음"),
        new("authority.unverified", 0, "AUTHORITY / unverified cache", "权威端 / 未验证缓存", "權威端 / 未驗證快取", "権限元 / 未検証キャッシュ", "AUTORIDAD / caché sin verificar", "AUTORITÄT / ungeprüfter Cache", "AUTORITÉ / cache non vérifié", "권한 주체 / 미검증 캐시"),
        new("a11y.authority.unverified", 1, "Authority health for daemon {0} is unavailable in the cached topology", "缓存拓扑中没有 daemon {0} 的权威端健康状态", "快取拓撲中沒有 daemon {0} 的權威端健康狀態", "キャッシュ済みトポロジでは daemon {0} の権限元ヘルスを利用できません", "La salud de la autoridad del daemon {0} no está disponible en la topología en caché", "Autoritätszustand für Daemon {0} ist in der zwischengespeicherten Topologie nicht verfügbar", "La santé de l’autorité du daemon {0} n’est pas disponible dans la topologie en cache", "캐시된 토폴로지에서 daemon {0}의 권한 주체 상태를 사용할 수 없습니다"),
        new("authority.stale", 1, "{0} / STALE", "{0} / 陈旧", "{0} / 過時", "{0} / 古い", "{0} / OBSOLETA", "{0} / VERALTET", "{0} / OBSOLÈTE", "{0} / 오래됨"),
        new("a11y.authority", 2, "Authority health for daemon {0}: {1}", "daemon {0} 的权威端健康状态：{1}", "daemon {0} 的權威端健康狀態：{1}", "daemon {0} の権限元ヘルス: {1}", "Salud de la autoridad del daemon {0}: {1}", "Autoritätszustand für Daemon {0}: {1}", "Santé de l’autorité du daemon {0} : {1}", "daemon {0}의 권한 주체 상태: {1}"),
        new("a11y.authority.stale", 2, "Authority health for daemon {0}: {1}; stale topology evidence", "daemon {0} 的权威端健康状态：{1}；拓扑证据陈旧", "daemon {0} 的權威端健康狀態：{1}；拓撲證據過時", "daemon {0} の権限元ヘルス: {1}。古いトポロジ証拠", "Salud de la autoridad del daemon {0}: {1}; evidencia de topología obsoleta", "Autoritätszustand für Daemon {0}: {1}; veralteter Topologienachweis", "Santé de l’autorité du daemon {0} : {1} ; preuve de topologie obsolète", "daemon {0}의 권한 주체 상태: {1}; 오래된 토폴로지 증거"),
        new("runtime.topology_unavailable", 0, "Topology unavailable. The daemon session can still be opened manually.", "拓扑不可用。仍可手动打开 daemon 会话。", "拓撲不可用。仍可手動開啟 daemon 工作階段。", "トポロジを利用できません。daemon セッションは手動で開けます。", "La topología no está disponible. La sesión daemon aún puede abrirse manualmente.", "Topologie nicht verfügbar. Die Daemon-Sitzung kann weiterhin manuell geöffnet werden.", "Topologie indisponible. La session daemon peut toujours être ouverte manuellement.", "토폴로지를 사용할 수 없습니다. daemon 세션은 수동으로 열 수 있습니다."),
        new("summary.unavailable", 1, "RUNTIMES / unavailable / failures {0}", "RUNTIME / 不可用 / 失败 {0}", "RUNTIME / 不可用 / 失敗 {0}", "RUNTIME / 利用不可 / 失敗 {0}", "RUNTIMES / no disponibles / fallos {0}", "RUNTIMES / nicht verfügbar / Fehler {0}", "RUNTIMES / indisponibles / échecs {0}", "RUNTIME / 사용 불가 / 실패 {0}"),
        new("authority.unavailable", 0, "AUTHORITY / unavailable", "权威端 / 不可用", "權威端 / 不可用", "権限元 / 利用不可", "AUTORIDAD / no disponible", "AUTORITÄT / nicht verfügbar", "AUTORITÉ / indisponible", "권한 주체 / 사용 불가"),
        new("a11y.summary.unavailable", 2, "Runtime topology for daemon {0} is unavailable after {1} failures", "daemon {0} 的 runtime 拓扑在 {1} 次失败后不可用", "daemon {0} 的 runtime 拓撲在 {1} 次失敗後不可用", "daemon {0} の runtime トポロジは {1} 回の失敗後に利用できません", "La topología de runtimes del daemon {0} no está disponible tras {1} fallos", "Runtime-Topologie für Daemon {0} ist nach {1} Fehlern nicht verfügbar", "La topologie des runtimes du daemon {0} est indisponible après {1} échecs", "daemon {0}의 runtime 토폴로지는 {1}회 실패 후 사용할 수 없습니다"),
        new("a11y.authority.unavailable", 1, "Authority health for daemon {0} is unavailable", "daemon {0} 的权威端健康状态不可用", "daemon {0} 的權威端健康狀態不可用", "daemon {0} の権限元ヘルスを利用できません", "La salud de la autoridad del daemon {0} no está disponible", "Autoritätszustand für Daemon {0} ist nicht verfügbar", "La santé de l’autorité du daemon {0} est indisponible", "daemon {0}의 권한 주체 상태를 사용할 수 없습니다"),
        new("runtime.status.never_requested", 0, "NOT REQUESTED", "未请求", "未要求", "未要求", "NO SOLICITADO", "NICHT ANGEFORDERT", "NON DEMANDÉ", "요청 안 됨"),
        new("runtime.status.pending", 0, "PENDING", "等待中", "等待中", "保留中", "PENDIENTE", "AUSSTEHEND", "EN ATTENTE", "대기 중"),
        new("runtime.status.ready", 0, "READY", "就绪", "就緒", "準備完了", "LISTO", "BEREIT", "PRÊT", "준비됨"),
        new("runtime.status.failed", 0, "FAILED", "失败", "失敗", "失敗", "FALLIDO", "FEHLGESCHLAGEN", "ÉCHEC", "실패"),
        new("a11y.open_runtime", 3, "Open gewyvern runtime {0}, ID {1}, status {2}", "打开 gewyvern runtime {0}，ID {1}，状态 {2}", "開啟 gewyvern runtime {0}，ID {1}，狀態 {2}", "gewyvern runtime {0}、ID {1}、状態 {2} を開く", "Abrir runtime gewyvern {0}, ID {1}, estado {2}", "Gewyvern-Runtime {0}, ID {1}, Status {2} öffnen", "Ouvrir le runtime gewyvern {0}, ID {1}, état {2}", "gewyvern runtime {0}, ID {1}, 상태 {2} 열기"),
        new("help.open_runtime", 0, "Opens this runtime through its owning daemon session after an authoritative revision check.", "在权威修订检查后，通过所属 daemon 会话打开此 runtime。", "在權威修訂檢查後，透過所屬 daemon 工作階段開啟此 runtime。", "権威あるリビジョン検査後、所属する daemon セッションを通じてこの runtime を開きます。", "Abre este runtime mediante su sesión daemon propietaria tras comprobar la revisión autoritativa.", "Öffnet diese Runtime nach einer autoritativen Revisionsprüfung über ihre zugehörige Daemon-Sitzung.", "Ouvre ce runtime via sa session daemon propriétaire après vérification de la révision faisant autorité.", "권한 있는 리비전 검사 후 소유 daemon 세션을 통해 이 runtime을 엽니다."),
        new("status.opening_daemon", 1, "Opening {0}...", "正在打开 {0}...", "正在開啟 {0}...", "{0} を開いています...", "Abriendo {0}...", "{0} wird geöffnet...", "Ouverture de {0}...", "{0} 여는 중..."),
        new("status.daemon_open", 1, "{0} session is open.", "{0} 会话已打开。", "{0} 工作階段已開啟。", "{0} セッションを開きました。", "La sesión {0} está abierta.", "Sitzung {0} ist geöffnet.", "La session {0} est ouverte.", "{0} 세션이 열렸습니다."),
        new("status.opening_runtime", 2, "Opening {0} through {1}...", "正在通过 {1} 打开 {0}...", "正在透過 {1} 開啟 {0}...", "{1} を通じて {0} を開いています...", "Abriendo {0} mediante {1}...", "{0} wird über {1} geöffnet...", "Ouverture de {0} via {1}...", "{1}을(를) 통해 {0} 여는 중..."),
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
                $"desktop Hub localization key is unknown: {key}");
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
            throw new InvalidDataException("desktop Hub localization entries drifted");
        }
        DesktopDomainCatalogContract.Verify(
            "Hub",
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
