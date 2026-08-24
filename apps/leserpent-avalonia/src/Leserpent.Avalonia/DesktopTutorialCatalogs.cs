using System.Globalization;

internal static class DesktopTutorialCatalogs
{
    private const string Prefix = "desktop.tutorial.";
    public const int KeyCount = 61;

    private static readonly int[] PointCounts = [3, 4, 4, 4, 4, 4];

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
        new("a11y.progress", 0, "Tutorial progress", "教学进度", "教學進度", "チュートリアルの進行状況", "Progreso del tutorial", "Tutorialfortschritt", "Progression du tutoriel", "튜토리얼 진행 상황"),
        new("help.progress", 0, "Announces the current tutorial step.", "播报当前教学步骤。", "宣告目前教學步驟。", "現在のチュートリアル手順を読み上げます。", "Anuncia el paso actual del tutorial.", "Gibt den aktuellen Tutorialschritt aus.", "Annonce l’étape actuelle du tutoriel.", "현재 튜토리얼 단계를 알립니다."),
        new("a11y.previous", 0, "Open the previous tutorial step", "打开上一个教学步骤", "開啟上一個教學步驟", "前のチュートリアル手順を開く", "Abrir el paso anterior del tutorial", "Vorherigen Tutorialschritt öffnen", "Ouvrir l’étape précédente du tutoriel", "이전 튜토리얼 단계 열기"),
        new("help.previous", 0, "Shortcut: Left Arrow.", "快捷键：向左键。", "快速鍵：向左鍵。", "ショートカット: 左矢印キー。", "Atajo: flecha izquierda.", "Tastenkürzel: Pfeil nach links.", "Raccourci : flèche gauche.", "단축키: 왼쪽 화살표."),
        new("a11y.next", 0, "Open the next tutorial step", "打开下一个教学步骤", "開啟下一個教學步驟", "次のチュートリアル手順を開く", "Abrir el paso siguiente del tutorial", "Nächsten Tutorialschritt öffnen", "Ouvrir l’étape suivante du tutoriel", "다음 튜토리얼 단계 열기"),
        new("help.next", 0, "Advances until the final step. Shortcut: Right Arrow.", "前进到最后一步。快捷键：向右键。", "前進至最後一步。快速鍵：向右鍵。", "最後の手順まで進みます。ショートカット: 右矢印キー。", "Avanza hasta el último paso. Atajo: flecha derecha.", "Geht bis zum letzten Schritt weiter. Tastenkürzel: Pfeil nach rechts.", "Avance jusqu’à la dernière étape. Raccourci : flèche droite.", "마지막 단계까지 진행합니다. 단축키: 오른쪽 화살표."),
        new("a11y.close", 0, "Close the Leserpent tutorial", "关闭 Leserpent 教学", "關閉 Leserpent 教學", "Leserpent チュートリアルを閉じる", "Cerrar el tutorial de Leserpent", "Leserpent-Tutorial schließen", "Fermer le tutoriel Leserpent", "Leserpent 튜토리얼 닫기"),
        new("help.close", 0, "Returns to the Hub without starting an operation. Shortcut: Escape.", "返回 Hub，不会启动任何操作。快捷键：Escape。", "返回 Hub，且不會啟動任何操作。快速鍵：Escape。", "操作を開始せず Hub に戻ります。ショートカット: Escape。", "Vuelve al Hub sin iniciar ninguna operación. Atajo: Escape.", "Kehrt zum Hub zurück, ohne einen Vorgang zu starten. Tastenkürzel: Escape.", "Revient au Hub sans lancer d’opération. Raccourci : Échap.", "작업을 시작하지 않고 Hub로 돌아갑니다. 단축키: Escape."),
        new("a11y.step.open", 2, "Open tutorial step {0}: {1}", "打开教学步骤 {0}：{1}", "開啟教學步驟 {0}：{1}", "チュートリアル手順 {0} を開く: {1}", "Abrir el paso {0} del tutorial: {1}", "Tutorialschritt {0} öffnen: {1}", "Ouvrir l’étape {0} du tutoriel : {1}", "튜토리얼 {0}단계 열기: {1}"),
        new("help.step.jump", 1, "Jumps directly to {0}.", "直接跳转到{0}。", "直接跳至{0}。", "{0} に直接移動します。", "Salta directamente a {0}.", "Springt direkt zu {0}.", "Accède directement à {0}.", "{0}(으)로 바로 이동합니다."),
        new("a11y.step.current", 2, "Current tutorial step {0}: {1}", "当前教学步骤 {0}：{1}", "目前教學步驟 {0}：{1}", "現在のチュートリアル手順 {0}: {1}", "Paso actual {0} del tutorial: {1}", "Aktueller Tutorialschritt {0}: {1}", "Étape actuelle {0} du tutoriel : {1}", "현재 튜토리얼 {0}단계: {1}"),
        new("a11y.next.finish", 0, "Finish and close the Leserpent tutorial", "完成并关闭 Leserpent 教学", "完成並關閉 Leserpent 教學", "Leserpent チュートリアルを完了して閉じる", "Finalizar y cerrar el tutorial de Leserpent", "Leserpent-Tutorial abschließen und schließen", "Terminer et fermer le tutoriel Leserpent", "Leserpent 튜토리얼 완료 후 닫기"),
        new("help.next.finish", 0, "Closes the tutorial and returns to the Hub without starting an operation.", "关闭教学并返回 Hub，不会启动任何操作。", "關閉教學並返回 Hub，且不會啟動任何操作。", "操作を開始せずチュートリアルを閉じて Hub に戻ります。", "Cierra el tutorial y vuelve al Hub sin iniciar ninguna operación.", "Schließt das Tutorial und kehrt zum Hub zurück, ohne einen Vorgang zu starten.", "Ferme le tutoriel et revient au Hub sans lancer d’opération.", "작업을 시작하지 않고 튜토리얼을 닫은 뒤 Hub로 돌아갑니다."),
        new("a11y.progress.current", 3, "Tutorial step {0} of {1}: {2}", "教学步骤 {0}/{1}：{2}", "教學步驟 {0}/{1}：{2}", "チュートリアル手順 {0}/{1}: {2}", "Paso {0} de {1} del tutorial: {2}", "Tutorialschritt {0} von {1}: {2}", "Étape {0} sur {1} du tutoriel : {2}", "튜토리얼 {1}단계 중 {0}단계: {2}"),

        new("step.1.label", 0, "SYSTEM MAP", "系统地图", "系統地圖", "システムマップ", "MAPA DEL SISTEMA", "SYSTEMKARTE", "CARTE SYSTÈME", "시스템 맵"),
        new("step.1.title", 0, "Read the topology", "读懂拓扑", "讀懂拓撲", "トポロジを読み解く", "Leer la topología", "Topologie lesen", "Lire la topologie", "토폴로지 이해하기"),
        new("step.1.summary", 0, "Leserpent Desktop is a client, not the authority. It can manage many leserpentd authorities, and each daemon can own many Gewyvern runtime services.", "Leserpent Desktop 是客户端，而不是权威端。它可以管理多个 leserpentd 权威端，每个 daemon 又可以拥有多个 Gewyvern runtime 服务。", "Leserpent Desktop 是用戶端，而不是權威端。它可以管理多個 leserpentd 權威端，每個 daemon 又可以擁有多個 Gewyvern runtime 服務。", "Leserpent Desktop はクライアントであり、権限元ではありません。複数の leserpentd 権限元を管理でき、各 daemon は複数の Gewyvern runtime サービスを所有できます。", "Leserpent Desktop es un cliente, no la autoridad. Puede administrar muchas autoridades leserpentd y cada daemon puede poseer varios servicios runtime de Gewyvern.", "Leserpent Desktop ist ein Client, nicht die Autorität. Es kann viele leserpentd-Autoritäten verwalten, und jeder Daemon kann mehrere Gewyvern-Runtime-Dienste besitzen.", "Leserpent Desktop est un client, pas l’autorité. Il peut gérer plusieurs autorités leserpentd, et chaque daemon peut posséder plusieurs services runtime Gewyvern.", "Leserpent Desktop은 권한 주체가 아니라 클라이언트입니다. 여러 leserpentd 권한 주체를 관리할 수 있으며 각 daemon은 여러 Gewyvern runtime 서비스를 소유할 수 있습니다."),
        new("step.1.point.1", 0, "The Hub root is this desktop operator session.", "Hub 根节点代表当前桌面操作员会话。", "Hub 根節點代表目前桌面操作員工作階段。", "Hub ルートはこのデスクトップのオペレーターセッションです。", "La raíz del Hub es esta sesión del operador de escritorio.", "Die Hub-Wurzel ist diese Desktop-Operatorsitzung.", "La racine du Hub correspond à cette session opérateur de bureau.", "Hub 루트는 이 데스크톱 운영자 세션입니다."),
        new("step.1.point.2", 0, "Every daemon card is an independent local or remote authority with its own web service.", "每张 daemon 卡片都是独立的本地或远程权威端，并拥有自己的 Web 服务。", "每張 daemon 卡片都是獨立的本機或遠端權威端，並擁有自己的 Web 服務。", "各 daemon カードは独立したローカルまたはリモートの権限元で、固有の Web サービスを持ちます。", "Cada tarjeta daemon es una autoridad local o remota independiente con su propio servicio web.", "Jede Daemon-Karte ist eine unabhängige lokale oder entfernte Autorität mit eigenem Webdienst.", "Chaque carte daemon est une autorité locale ou distante indépendante avec son propre service Web.", "각 daemon 카드는 자체 웹 서비스를 가진 독립적인 로컬 또는 원격 권한 주체입니다."),
        new("step.1.point.3", 0, "Every runtime child stays routed through the daemon that owns it.", "每个 runtime 子节点始终通过拥有它的 daemon 完成路由。", "每個 runtime 子節點始終透過擁有它的 daemon 完成路由。", "各 runtime 子ノードは、所有する daemon を通じて常にルーティングされます。", "Cada runtime hijo siempre se enruta mediante el daemon que lo posee.", "Jedes Runtime-Kind bleibt über den besitzenden Daemon geroutet.", "Chaque runtime enfant reste acheminé via le daemon qui le possède.", "각 runtime 하위 항목은 항상 이를 소유한 daemon을 통해 라우팅됩니다."),
        new("step.1.model", 0, "Leserpent client -> leserpentd authority -> Gewyvern runtime", "Leserpent 客户端 -> leserpentd 权威端 -> Gewyvern runtime", "Leserpent 用戶端 -> leserpentd 權威端 -> Gewyvern runtime", "Leserpent クライアント -> leserpentd 権限元 -> Gewyvern runtime", "Cliente Leserpent -> autoridad leserpentd -> runtime Gewyvern", "Leserpent-Client -> leserpentd-Autorität -> Gewyvern-Runtime", "Client Leserpent -> autorité leserpentd -> runtime Gewyvern", "Leserpent 클라이언트 -> leserpentd 권한 주체 -> Gewyvern runtime"),

        new("step.2.label", 0, "FIRST AUTHORITY", "首个权威端", "首個權威端", "最初の権限元", "PRIMERA AUTORIDAD", "ERSTE AUTORITÄT", "PREMIÈRE AUTORITÉ", "첫 권한 주체"),
        new("step.2.title", 0, "Establish a daemon path", "建立 daemon 路径", "建立 daemon 路徑", "daemon 経路を確立する", "Establecer una ruta daemon", "Daemon-Pfad herstellen", "Établir un chemin vers le daemon", "daemon 경로 설정"),
        new("step.2.summary", 0, "Start locally or attach a remote machine without turning credentials into permanent application state.", "可以从本机启动，也可以接入远程机器；主机凭证不会因此变成应用的永久状态。", "可以從本機啟動，也可以連接遠端機器；主機憑證不會因此成為應用程式的永久狀態。", "ローカルで開始するか、認証情報をアプリの永続状態にせずリモートマシンを接続します。", "Inicia localmente o conecta una máquina remota sin convertir las credenciales en estado permanente de la aplicación.", "Lokal starten oder einen entfernten Rechner anbinden, ohne Anmeldedaten zum dauerhaften Anwendungszustand zu machen.", "Démarrez localement ou rattachez une machine distante sans transformer les identifiants en état permanent de l’application.", "자격 증명을 애플리케이션의 영구 상태로 만들지 않고 로컬에서 시작하거나 원격 시스템을 연결합니다."),
        new("step.2.point.1", 0, "Local Orchestra starts an app-owned loopback daemon on desktop systems.", "在桌面系统上，本地 Orchestra 会启动由应用托管的回环 daemon。", "在桌面系統上，本機 Orchestra 會啟動由應用程式管理的回送 daemon。", "デスクトップ環境では、Local Orchestra がアプリ所有のループバック daemon を起動します。", "Local Orchestra inicia un daemon de bucle local propiedad de la aplicación en sistemas de escritorio.", "Local Orchestra startet auf Desktopsystemen einen anwendungseigenen Loopback-Daemon.", "Local Orchestra démarre sur les systèmes de bureau un daemon de bouclage appartenant à l’application.", "데스크톱 시스템에서 Local Orchestra는 앱이 소유한 루프백 daemon을 시작합니다."),
        new("step.2.point.2", 0, "Deploy daemon uses supplied host credentials to install leserpentd remotely.", "部署 daemon 会使用临时提供的主机凭证，在远程安装 leserpentd。", "部署 daemon 會使用暫時提供的主機憑證，在遠端安裝 leserpentd。", "Deploy daemon は提供されたホスト認証情報を使い、リモートに leserpentd をインストールします。", "Desplegar daemon usa las credenciales de host proporcionadas para instalar leserpentd de forma remota.", "Daemon bereitstellen verwendet bereitgestellte Host-Anmeldedaten, um leserpentd entfernt zu installieren.", "Déployer le daemon utilise les identifiants d’hôte fournis pour installer leserpentd à distance.", "daemon 배포는 제공된 호스트 자격 증명으로 원격에 leserpentd를 설치합니다."),
        new("step.2.point.3", 0, "Add daemon attaches an existing service using endpoint-bound trust and a runtime credential.", "添加 daemon 会通过端点绑定的信任与 runtime 凭证接入现有服务。", "新增 daemon 會透過端點綁定的信任與 runtime 憑證連接現有服務。", "Add daemon はエンドポイントに結び付いた信頼と runtime 認証情報で既存サービスを接続します。", "Añadir daemon conecta un servicio existente mediante confianza ligada al endpoint y una credencial runtime.", "Daemon hinzufügen bindet einen vorhandenen Dienst mit endpunktgebundenem Vertrauen und einer Runtime-Anmeldung an.", "Ajouter un daemon rattache un service existant avec une confiance liée à l’endpoint et un identifiant runtime.", "daemon 추가는 엔드포인트에 바인딩된 신뢰와 runtime 자격 증명으로 기존 서비스를 연결합니다."),
        new("step.2.point.4", 0, "Closing one daemon session does not close the Hub or another authority.", "关闭一个 daemon 会话不会关闭 Hub，也不会影响其他权威端。", "關閉一個 daemon 工作階段不會關閉 Hub，也不會影響其他權威端。", "1 つの daemon セッションを閉じても、Hub や別の権限元は閉じません。", "Cerrar una sesión daemon no cierra el Hub ni otra autoridad.", "Das Schließen einer Daemon-Sitzung schließt weder den Hub noch eine andere Autorität.", "Fermer une session daemon ne ferme ni le Hub ni une autre autorité.", "하나의 daemon 세션을 닫아도 Hub나 다른 권한 주체는 닫히지 않습니다."),
        new("step.2.model", 0, "Local Orchestra | Deploy daemon | + Add daemon", "本地 Orchestra | 部署 daemon | + 添加 daemon", "本機 Orchestra | 部署 daemon | + 新增 daemon", "Local Orchestra | Deploy daemon | + Add daemon", "Local Orchestra | Desplegar daemon | + Añadir daemon", "Local Orchestra | Daemon bereitstellen | + Daemon hinzufügen", "Local Orchestra | Déployer le daemon | + Ajouter un daemon", "Local Orchestra | daemon 배포 | + daemon 추가"),

        new("step.3.label", 0, "WORKSPACE", "工作区", "工作區", "ワークスペース", "ESPACIO DE TRABAJO", "ARBEITSBEREICH", "ESPACE DE TRAVAIL", "작업 공간"),
        new("step.3.title", 0, "Reach the right runtime", "抵达正确的 runtime", "抵達正確的 runtime", "正しい runtime に到達する", "Llegar al runtime correcto", "Die richtige Runtime erreichen", "Atteindre le bon runtime", "올바른 runtime에 도달하기"),
        new("step.3.summary", 0, "Refresh topology before acting, then open a runtime beneath its owning daemon. A workspace never silently changes authority.", "执行操作前先刷新拓扑，再从所属 daemon 下打开 runtime。工作区绝不会在后台悄悄切换权威端。", "執行操作前先重新整理拓撲，再從所屬 daemon 下開啟 runtime。工作區絕不會在背景悄悄切換權威端。", "操作前にトポロジを更新し、所有 daemon の配下から runtime を開きます。ワークスペースが暗黙に権限元を切り替えることはありません。", "Actualiza la topología antes de actuar y abre un runtime bajo el daemon que lo posee. Un espacio de trabajo nunca cambia de autoridad silenciosamente.", "Vor einer Aktion die Topologie aktualisieren und dann eine Runtime unter ihrem besitzenden Daemon öffnen. Ein Arbeitsbereich wechselt niemals unbemerkt die Autorität.", "Actualisez la topologie avant d’agir, puis ouvrez un runtime sous le daemon qui le possède. Un espace de travail ne change jamais silencieusement d’autorité.", "작업 전에 토폴로지를 새로 고친 뒤 소유 daemon 아래의 runtime을 엽니다. 작업 공간은 권한 주체를 몰래 바꾸지 않습니다."),
        new("step.3.point.1", 0, "Refresh all joins existing work instead of starting duplicate requests.", "全部刷新会加入已有任务，而不是重复发起请求。", "全部重新整理會加入現有工作，而不是重複發出要求。", "Refresh all は重複要求を開始せず、既存の処理に参加します。", "Actualizar todo se une al trabajo existente en vez de iniciar solicitudes duplicadas.", "Alle aktualisieren schließt sich vorhandener Arbeit an, statt doppelte Anfragen zu starten.", "Tout actualiser rejoint le travail existant au lieu de lancer des requêtes en double.", "모두 새로 고침은 중복 요청을 시작하지 않고 기존 작업에 참여합니다."),
        new("step.3.point.2", 0, "LIVE topology may open a workspace; retained or cached topology remains visibly stale.", "只有 LIVE 拓扑可以打开工作区；保留或缓存拓扑会持续显示为陈旧状态。", "只有 LIVE 拓撲可以開啟工作區；保留或快取拓撲會持續顯示為過時狀態。", "LIVE トポロジだけがワークスペースを開けます。保持またはキャッシュされたトポロジは古い状態として明示されます。", "Una topología LIVE puede abrir un espacio; una topología conservada o en caché permanece visiblemente obsoleta.", "Eine LIVE-Topologie darf einen Arbeitsbereich öffnen; beibehaltene oder zwischengespeicherte Topologie bleibt sichtbar veraltet.", "Une topologie LIVE peut ouvrir un espace ; une topologie conservée ou en cache reste visiblement obsolète.", "LIVE 토폴로지만 작업 공간을 열 수 있으며 유지되거나 캐시된 토폴로지는 오래된 상태로 표시됩니다."),
        new("step.3.point.3", 0, "Provision gewyvern installs and registers a runtime through a selected daemon authority.", "部署 gewyvern 会通过选定的 daemon 权威端安装并注册 runtime。", "佈建 gewyvern 會透過選定的 daemon 權威端安裝並註冊 runtime。", "Provision gewyvern は選択した daemon 権限元を通じて runtime をインストールし登録します。", "Aprovisionar gewyvern instala y registra un runtime mediante la autoridad daemon seleccionada.", "Gewyvern bereitstellen installiert und registriert eine Runtime über die ausgewählte Daemon-Autorität.", "Provisionner gewyvern installe et enregistre un runtime via l’autorité daemon sélectionnée.", "gewyvern 프로비저닝은 선택한 daemon 권한 주체를 통해 runtime을 설치하고 등록합니다."),
        new("step.3.point.4", 0, "A runtime button opens or focuses that runtime's native child window.", "runtime 按钮会打开或聚焦该 runtime 的原生子窗口。", "runtime 按鈕會開啟或聚焦該 runtime 的原生子視窗。", "runtime ボタンは、その runtime のネイティブ子ウィンドウを開くかフォーカスします。", "Un botón runtime abre o enfoca la ventana secundaria nativa de ese runtime.", "Eine Runtime-Schaltfläche öffnet oder fokussiert das native Unterfenster dieser Runtime.", "Un bouton runtime ouvre ou cible la fenêtre enfant native de ce runtime.", "runtime 버튼은 해당 runtime의 네이티브 하위 창을 열거나 포커스합니다."),
        new("step.3.model", 0, "Refresh topology -> choose authority -> open runtime", "刷新拓扑 -> 选择权威端 -> 打开 runtime", "重新整理拓撲 -> 選擇權威端 -> 開啟 runtime", "トポロジを更新 -> 権限元を選択 -> runtime を開く", "Actualizar topología -> elegir autoridad -> abrir runtime", "Topologie aktualisieren -> Autorität wählen -> Runtime öffnen", "Actualiser la topologie -> choisir l’autorité -> ouvrir le runtime", "토폴로지 새로 고침 -> 권한 주체 선택 -> runtime 열기"),

        new("step.4.label", 0, "FIRST DIAGNOSIS", "首次诊断", "首次診斷", "最初の診断", "PRIMER DIAGNÓSTICO", "ERSTE DIAGNOSE", "PREMIER DIAGNOSTIC", "첫 진단"),
        new("step.4.title", 0, "Run a focused diagnostic", "执行聚焦诊断", "執行聚焦診斷", "焦点を絞った診断を実行する", "Ejecutar un diagnóstico enfocado", "Gezielte Diagnose ausführen", "Exécuter un diagnostic ciblé", "집중 진단 실행"),
        new("step.4.summary", 0, "Use a runtime workspace to deploy a typed pipeline, observe its bounded logs, and export diagnostics without losing daemon identity.", "在 runtime 工作区部署类型化管道、观察有边界的日志并导出诊断，同时始终保留 daemon 身份。", "在 runtime 工作區部署型別化 pipeline、觀察有界限的日誌並匯出診斷，同時始終保留 daemon 身分。", "runtime ワークスペースで型付き pipeline をデプロイし、有界なログを観察して、daemon の識別情報を失わずに診断をエクスポートします。", "Usa un espacio runtime para desplegar una pipeline tipada, observar sus registros acotados y exportar diagnósticos sin perder la identidad del daemon.", "In einem Runtime-Arbeitsbereich eine typisierte Pipeline bereitstellen, begrenzte Protokolle beobachten und Diagnosen exportieren, ohne die Daemon-Identität zu verlieren.", "Utilisez un espace runtime pour déployer une pipeline typée, observer ses journaux bornés et exporter les diagnostics sans perdre l’identité du daemon.", "runtime 작업 공간에서 타입화된 pipeline을 배포하고 제한된 로그를 관찰하며 daemon ID를 잃지 않고 진단을 내보냅니다."),
        new("step.4.point.1", 0, "Choose a pipeline kind such as http/request instead of relying on an opaque button name.", "选择 http/request 等管道类型，不要依赖含义不透明的按钮名称。", "選擇 http/request 等 pipeline 類型，不要依賴含義不透明的按鈕名稱。", "不透明なボタン名に頼らず、http/request などの pipeline 種別を選びます。", "Elige un tipo de pipeline como http/request en vez de depender del nombre opaco de un botón.", "Einen Pipeline-Typ wie http/request wählen, statt sich auf einen undurchsichtigen Schaltflächennamen zu verlassen.", "Choisissez un type de pipeline comme http/request au lieu de dépendre d’un nom de bouton opaque.", "불투명한 버튼 이름에 의존하지 말고 http/request 같은 pipeline 종류를 선택합니다."),
        new("step.4.point.2", 0, "Add a target such as pid:4242 only when process scope is known.", "只有明确进程范围时，才添加 pid:4242 这样的目标。", "只有明確處理程序範圍時，才新增 pid:4242 這類目標。", "プロセス範囲が分かっている場合に限り、pid:4242 のような対象を追加します。", "Añade un objetivo como pid:4242 solo cuando se conozca el alcance del proceso.", "Ein Ziel wie pid:4242 nur hinzufügen, wenn der Prozessumfang bekannt ist.", "Ajoutez une cible comme pid:4242 uniquement lorsque la portée du processus est connue.", "프로세스 범위를 아는 경우에만 pid:4242 같은 대상을 추가합니다."),
        new("step.4.point.3", 0, "Inspect status, capabilities, snapshot changes, and severity before drawing a conclusion.", "得出结论前，检查状态、能力、快照变化和严重等级。", "得出結論前，檢查狀態、能力、快照變更和嚴重等級。", "結論を出す前に、状態、能力、スナップショット変更、重大度を確認します。", "Inspecciona el estado, las capacidades, los cambios de instantánea y la gravedad antes de sacar una conclusión.", "Vor einer Schlussfolgerung Status, Fähigkeiten, Snapshot-Änderungen und Schweregrad prüfen.", "Examinez l’état, les capacités, les changements d’instantané et la gravité avant de conclure.", "결론을 내리기 전에 상태, 기능, 스냅샷 변경 및 심각도를 검사합니다."),
        new("step.4.point.4", 0, "Use explicit diagnostic export when another engineer or tool needs the evidence.", "其他工程师或工具需要证据时，使用显式诊断导出。", "其他工程師或工具需要證據時，使用明確的診斷匯出。", "別のエンジニアやツールが証拠を必要とする場合は、明示的な診断エクスポートを使います。", "Usa la exportación explícita de diagnósticos cuando otro ingeniero o herramienta necesite la evidencia.", "Expliziten Diagnoseexport verwenden, wenn ein anderer Techniker oder ein Werkzeug den Nachweis benötigt.", "Utilisez l’export explicite des diagnostics lorsqu’un autre ingénieur ou outil a besoin des preuves.", "다른 엔지니어나 도구에 증거가 필요하면 명시적 진단 내보내기를 사용합니다."),
        new("step.4.model", 0, "pipeline=http/request  target=pid:4242", "pipeline=http/request  target=pid:4242", "pipeline=http/request  target=pid:4242", "pipeline=http/request  target=pid:4242", "pipeline=http/request  target=pid:4242", "pipeline=http/request  target=pid:4242", "pipeline=http/request  target=pid:4242", "pipeline=http/request  target=pid:4242"),

        new("step.5.label", 0, "SAFETY FENCES", "安全围栏", "安全圍欄", "安全フェンス", "BARRERAS DE SEGURIDAD", "SICHERHEITSSCHRANKEN", "GARDE-FOUS", "안전 경계"),
        new("step.5.title", 0, "Know when a change is blocked", "理解操作为何被阻止", "理解操作為何被阻止", "変更がブロックされる理由を知る", "Saber cuándo se bloquea un cambio", "Erkennen, wann eine Änderung blockiert ist", "Comprendre quand un changement est bloqué", "변경이 차단되는 경우 이해하기"),
        new("step.5.summary", 0, "Leserpent fails closed when authority, freshness, capability, or revision evidence is missing. A disabled action is information, not friction to bypass.", "缺少权威性、新鲜度、能力或 revision 证据时，Leserpent 会安全拒绝。禁用动作是在提供信息，不是应该绕过的阻力。", "缺少權威性、新鮮度、能力或 revision 證據時，Leserpent 會安全拒絕。停用動作是在提供資訊，不是應該繞過的阻力。", "権限、新鮮性、能力、または revision の証拠が欠けると、Leserpent は安全側に閉じます。無効な操作は回避すべき障害ではなく情報です。", "Leserpent se cierra de forma segura cuando faltan pruebas de autoridad, vigencia, capacidad o revision. Una acción deshabilitada aporta información, no es una fricción que deba eludirse.", "Leserpent verweigert sicher, wenn Nachweise zu Autorität, Aktualität, Fähigkeit oder Revision fehlen. Eine deaktivierte Aktion ist Information und kein zu umgehendes Hindernis.", "Leserpent échoue de manière fermée lorsqu’une preuve d’autorité, de fraîcheur, de capacité ou de revision manque. Une action désactivée informe ; ce n’est pas un obstacle à contourner.", "권한, 최신성, 기능 또는 revision 증거가 없으면 Leserpent는 안전하게 거부합니다. 비활성화된 작업은 우회할 장애물이 아니라 정보입니다."),
        new("step.5.point.1", 0, "Inspection and mutation availability come from shared policy, not frontend guesses.", "检查与变更可用性来自共享策略，而不是前端猜测。", "檢查與變更可用性來自共享策略，而不是前端猜測。", "検査と変更の可用性は、フロントエンドの推測ではなく共有ポリシーから決まります。", "La disponibilidad de inspección y mutación procede de una política compartida, no de suposiciones del frontend.", "Die Verfügbarkeit von Inspektion und Mutation stammt aus gemeinsamer Richtlinie, nicht aus Vermutungen des Frontends.", "La disponibilité de l’inspection et des mutations vient d’une politique partagée, pas de suppositions du frontend.", "검사 및 변경 가능 여부는 프런트엔드의 추측이 아니라 공유 정책에서 결정됩니다."),
        new("step.5.point.2", 0, "Deployment requires an authenticated capability and explicit confirmation.", "部署要求 authenticated capability（已认证能力）和明确确认。", "部署需要 authenticated capability（已驗證能力）和明確確認。", "デプロイには authenticated capability と明示的な確認が必要です。", "El despliegue requiere una authenticated capability y confirmación explícita.", "Eine Bereitstellung erfordert eine authenticated capability und eine ausdrückliche Bestätigung.", "Le déploiement exige une authenticated capability et une confirmation explicite.", "배포에는 authenticated capability와 명시적 확인이 필요합니다."),
        new("step.5.point.3", 0, "Revision drift or a closed workspace invalidates an in-progress submission.", "revision 漂移或工作区关闭会使正在提交的操作失效。", "revision 漂移或工作區關閉會使正在提交的操作失效。", "revision のずれやワークスペースの終了は、進行中の送信を無効にします。", "La deriva de revision o el cierre del espacio invalida un envío en curso.", "Eine abweichende Revision oder ein geschlossener Arbeitsbereich macht eine laufende Übermittlung ungültig.", "Une dérive de revision ou la fermeture de l’espace invalide une soumission en cours.", "revision 드리프트 또는 닫힌 작업 공간은 진행 중인 제출을 무효화합니다."),
        new("step.5.point.4", 0, "Unknown mutation outcomes require operator review and are never retried invisibly.", "未知变更结果必须由操作员复核，绝不会在后台隐式重试。", "未知變更結果必須由操作員複核，絕不會在背景隱式重試。", "不明な変更結果はオペレーターの確認が必要で、見えないまま再試行されることはありません。", "Los resultados de mutación desconocidos requieren revisión del operador y nunca se reintentan de forma invisible.", "Unbekannte Mutationsergebnisse erfordern eine Prüfung durch den Operator und werden nie unsichtbar erneut versucht.", "Les résultats de mutation inconnus exigent une vérification de l’opérateur et ne sont jamais retentés de façon invisible.", "알 수 없는 변경 결과는 운영자 검토가 필요하며 보이지 않게 재시도되지 않습니다."),
        new("step.5.model", 0, "live + authoritative + capable + confirmed -> mutate", "live + authoritative + capable + confirmed -> mutate", "live + authoritative + capable + confirmed -> mutate", "live + authoritative + capable + confirmed -> mutate", "live + authoritative + capable + confirmed -> mutate", "live + authoritative + capable + confirmed -> mutate", "live + authoritative + capable + confirmed -> mutate", "live + authoritative + capable + confirmed -> mutate"),

        new("step.6.label", 0, "LESELANG", "LESELANG", "LESELANG", "LESELANG", "LESELANG", "LESELANG", "LESELANG", "LESELANG"),
        new("step.6.title", 0, "Automate the same interface", "自动化同一套界面", "自動化同一套介面", "同じインターフェースを自動化する", "Automatizar la misma interfaz", "Dieselbe Oberfläche automatisieren", "Automatiser la même interface", "동일한 인터페이스 자동화"),
        new("step.6.summary", 0, "Native controls and Leselang operations are two views of the same typed UI contract. Automation should not gain a hidden control plane.", "原生控件与 Leselang 操作是同一份类型化 UI 协议的两种视图；自动化不会获得隐藏的控制平面。", "原生控制項與 Leselang 操作是同一份型別化 UI 協定的兩種視圖；自動化不會取得隱藏的控制平面。", "ネイティブコントロールと Leselang 操作は、同じ型付き UI 契約の 2 つの見方です。自動化が隠れたコントロールプレーンを得ることはありません。", "Los controles nativos y las operaciones Leselang son dos vistas del mismo contrato de UI tipado. La automatización no debe obtener un plano de control oculto.", "Native Steuerelemente und Leselang-Operationen sind zwei Ansichten desselben typisierten UI-Vertrags. Automatisierung darf keine verborgene Steuerungsebene erhalten.", "Les contrôles natifs et les opérations Leselang sont deux vues du même contrat UI typé. L’automatisation ne doit pas obtenir de plan de contrôle caché.", "네이티브 컨트롤과 Leselang 작업은 동일한 타입화 UI 계약을 보는 두 방식입니다. 자동화에 숨겨진 제어 플레인이 생겨서는 안 됩니다."),
        new("step.6.point.1", 0, "Stable Automation IDs identify controls; business behavior comes from typed actions.", "稳定的 Automation ID 标识控件，业务行为来自类型化动作。", "穩定的 Automation ID 識別控制項，業務行為來自型別化動作。", "安定した Automation ID がコントロールを識別し、業務動作は型付きアクションから得られます。", "Los Automation ID estables identifican controles; el comportamiento de negocio procede de acciones tipadas.", "Stabile Automation IDs identifizieren Steuerelemente; das Geschäftsverhalten stammt aus typisierten Aktionen.", "Des Automation ID stables identifient les contrôles ; le comportement métier vient d’actions typées.", "안정적인 Automation ID가 컨트롤을 식별하며 비즈니스 동작은 타입화된 작업에서 나옵니다."),
        new("step.6.point.2", 0, "Node IDs are opaque and must never be parsed as protocol commands.", "节点 ID 是不透明标识，绝不能解析成协议命令。", "節點 ID 是不透明識別碼，絕不能解析成協定命令。", "ノード ID は不透明であり、プロトコルコマンドとして解析してはいけません。", "Los ID de nodo son opacos y nunca deben analizarse como comandos de protocolo.", "Knoten-IDs sind undurchsichtig und dürfen nie als Protokollbefehle interpretiert werden.", "Les ID de nœud sont opaques et ne doivent jamais être analysés comme des commandes de protocole.", "노드 ID는 불투명하며 프로토콜 명령으로 파싱해서는 안 됩니다."),
        new("step.6.point.3", 0, "Leselang can focus, inspect, fill, submit, wait, and assert the same states a person sees.", "Leselang 可以聚焦、检查、填写、提交、等待和断言人与机器看到的同一状态。", "Leselang 可以聚焦、檢查、填寫、提交、等待和斷言人與機器看到的相同狀態。", "Leselang は、人が見るのと同じ状態をフォーカス、検査、入力、送信、待機、アサートできます。", "Leselang puede enfocar, inspeccionar, rellenar, enviar, esperar y afirmar los mismos estados que ve una persona.", "Leselang kann dieselben für Menschen sichtbaren Zustände fokussieren, prüfen, ausfüllen, absenden, abwarten und bestätigen.", "Leselang peut cibler, inspecter, remplir, soumettre, attendre et vérifier les mêmes états qu’une personne voit.", "Leselang은 사람이 보는 것과 동일한 상태를 포커스, 검사, 입력, 제출, 대기 및 단언할 수 있습니다."),
        new("step.6.point.4", 0, "Export canonical Leselang before execution when reviewing or sharing an automated workflow.", "复核或分享自动化流程时，在执行前导出规范化 Leselang。", "複核或分享自動化流程時，在執行前匯出規範化 Leselang。", "自動化ワークフローを確認または共有する際は、実行前に正規 Leselang をエクスポートします。", "Exporta Leselang canónico antes de ejecutar al revisar o compartir un flujo automatizado.", "Beim Prüfen oder Teilen eines automatisierten Ablaufs vor der Ausführung kanonisches Leselang exportieren.", "Exportez le Leselang canonique avant l’exécution lors de la révision ou du partage d’un flux automatisé.", "자동화 워크플로를 검토하거나 공유할 때 실행 전에 정규 Leselang을 내보냅니다."),
        new("step.6.model", 0, "native control <-> typed UI action <-> Leselang", "原生控件 <-> 类型化 UI 动作 <-> Leselang", "原生控制項 <-> 型別化 UI 動作 <-> Leselang", "ネイティブコントロール <-> 型付き UI アクション <-> Leselang", "control nativo <-> acción UI tipada <-> Leselang", "natives Steuerelement <-> typisierte UI-Aktion <-> Leselang", "contrôle natif <-> action UI typée <-> Leselang", "네이티브 컨트롤 <-> 타입화 UI 작업 <-> Leselang"),
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
                $"desktop tutorial localization key is unknown: {key}");
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

    public static DesktopTutorialStep[] Steps(DesktopLocalization localization) =>
        Enumerable.Range(1, PointCounts.Length)
            .Select(index => new DesktopTutorialStep(
                Resolve(localization, $"step.{index}.label"),
                Resolve(localization, $"step.{index}.title"),
                Resolve(localization, $"step.{index}.summary"),
                Enumerable.Range(1, PointCounts[index - 1])
                    .Select(point => Resolve(
                        localization,
                        $"step.{index}.point.{point}"))
                    .ToArray(),
                Resolve(localization, $"step.{index}.model")))
            .ToArray();

    public static void VerifyContract()
    {
        if (Entries.Length != KeyCount
            || Entries.Select(entry => entry.Key).Distinct(StringComparer.Ordinal).Count()
                != KeyCount)
        {
            throw new InvalidDataException("desktop tutorial localization entries drifted");
        }
        DesktopDomainCatalogContract.Verify(
            "tutorial",
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
