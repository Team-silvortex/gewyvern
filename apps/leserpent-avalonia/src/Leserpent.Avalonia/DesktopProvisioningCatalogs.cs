using System.Globalization;

internal static class DesktopProvisioningCatalogs
{
    private const string Prefix = "desktop.provisioning.";
    public const int KeyCount = 43;

    public static IReadOnlyDictionary<string, string> English { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Provision gewyvern",
            ["confirmation"] = "I confirm gewyvern installation and runtime registration on this host",
            ["submit"] = "Provision gewyvern",
            ["refresh"] = "Refresh same attempt",
            ["close"] = "Close",
            ["status.initial"] = "Choose the daemon authority that will own this gewyvern runtime.",
            ["status.name"] = "Gewyvern provisioning status",
            ["phase.name"] = "Gewyvern provisioning phase",
            ["kicker"] = "RUNTIME PROVISIONING",
            ["heading"] = "Install and register gewyvern",
            ["body"] = "The selected leserpentd performs native SSH installation, proves service identity, and atomically registers the runtime. Only an opaque vault handle leaves this desktop.",
            ["authority.label"] = "Owning daemon authority",
            ["provisioning_id.label"] = "Provisioning ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "Target host",
            ["port.label"] = "SSH port",
            ["credential.label"] = "SSH credential handle",
            ["a11y.authority"] = "Daemon authority owning the gewyvern runtime",
            ["a11y.provisioning_id"] = "Stable gewyvern provisioning operation ID",
            ["a11y.runtime_id"] = "Runtime ID registered by provisioning",
            ["a11y.host"] = "Target host for gewyvern installation",
            ["a11y.port"] = "Target SSH port",
            ["a11y.credential"] = "Opaque SSH credential handle",
            ["a11y.confirm"] = "Confirm gewyvern installation and registration",
            ["a11y.submit"] = "Provision and register gewyvern runtime",
            ["a11y.refresh"] = "Refresh the same provisioning attempt",
            ["a11y.close"] = "Close gewyvern provisioning window",
            ["phase.not_submitted"] = "NOT SUBMITTED",
            ["phase.planned"] = "PLANNED",
            ["phase.installing"] = "INSTALLING",
            ["phase.service_ready"] = "SERVICE READY",
            ["phase.runtime_registered"] = "RUNTIME REGISTERED",
            ["phase.failed"] = "FAILED",
            ["error.confirm_required"] = "Confirm gewyvern installation before submitting.",
            ["error.authority_required"] = "Select an owning daemon authority first.",
            ["status.observation_limit"] = "Automatic observation reached its bounded limit. Use Refresh same attempt to inspect this exact provisioning ID without creating another installation.",
            ["status.waiting"] = "Waiting for the selected daemon authority...",
            ["status.planned"] = "Provisioning is durably queued. Observation reuses this exact identity and does not submit a second installation.",
            ["status.installing"] = "The daemon authority is installing and activating gewyvern on the target host.",
            ["status.service_ready"] = "Gewyvern is verified at {0}; atomic runtime registration is pending.",
            ["status.runtime_registered"] = "Runtime {0} is registered and ready at {1}.",
            ["status.failed"] = "Provisioning failed with bounded fault {0}. Correct the cause, then start a new attempt with a new provisioning ID; this failed identity remains immutable for audit.",
            ["unavailable"] = "unavailable",
        });

    public static IReadOnlyDictionary<string, string> SimplifiedChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 部署 gewyvern",
            ["confirmation"] = "我确认在此主机上安装 gewyvern 并注册 runtime",
            ["submit"] = "部署 gewyvern",
            ["refresh"] = "刷新同一尝试",
            ["close"] = "关闭",
            ["status.initial"] = "请选择将拥有此 gewyvern runtime 的 daemon 权威端。",
            ["status.name"] = "Gewyvern 部署状态",
            ["phase.name"] = "Gewyvern 部署阶段",
            ["kicker"] = "RUNTIME 部署",
            ["heading"] = "安装并注册 gewyvern",
            ["body"] = "所选 leserpentd 会执行原生 SSH 安装、证明服务身份，并原子注册 runtime。离开此桌面端的只有不透明 vault 句柄。",
            ["authority.label"] = "所属 daemon 权威端",
            ["provisioning_id.label"] = "部署 ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "目标主机",
            ["port.label"] = "SSH 端口",
            ["credential.label"] = "SSH 凭证句柄",
            ["a11y.authority"] = "拥有此 gewyvern runtime 的 daemon 权威端",
            ["a11y.provisioning_id"] = "稳定的 gewyvern 部署操作 ID",
            ["a11y.runtime_id"] = "部署流程注册的 runtime ID",
            ["a11y.host"] = "gewyvern 的安装目标主机",
            ["a11y.port"] = "目标 SSH 端口",
            ["a11y.credential"] = "不透明 SSH 凭证句柄",
            ["a11y.confirm"] = "确认安装并注册 gewyvern",
            ["a11y.submit"] = "部署并注册 gewyvern runtime",
            ["a11y.refresh"] = "刷新同一次部署尝试",
            ["a11y.close"] = "关闭 gewyvern 部署窗口",
            ["phase.not_submitted"] = "尚未提交",
            ["phase.planned"] = "已规划",
            ["phase.installing"] = "安装中",
            ["phase.service_ready"] = "服务已就绪",
            ["phase.runtime_registered"] = "RUNTIME 已注册",
            ["phase.failed"] = "失败",
            ["error.confirm_required"] = "提交前请确认安装 gewyvern。",
            ["error.authority_required"] = "请先选择所属 daemon 权威端。",
            ["status.observation_limit"] = "自动观察已达到受限上限。请使用“刷新同一尝试”检查这个确切的部署 ID，不会创建另一次安装。",
            ["status.waiting"] = "正在等待所选 daemon 权威端...",
            ["status.planned"] = "部署已持久排队。观察会复用这个确切身份，不会提交第二次安装。",
            ["status.installing"] = "daemon 权威端正在目标主机上安装并启动 gewyvern。",
            ["status.service_ready"] = "已在 {0} 验证 gewyvern；正在等待原子注册 runtime。",
            ["status.runtime_registered"] = "runtime {0} 已注册，并在 {1} 就绪。",
            ["status.failed"] = "部署失败，受限故障代码为 {0}。请修正原因，再使用新的部署 ID 开始新尝试；此失败身份会保持不可变以供审计。",
            ["unavailable"] = "不可用",
        });

    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 佈建 gewyvern",
            ["confirmation"] = "我確認在此主機上安裝 gewyvern 並註冊 runtime",
            ["submit"] = "佈建 gewyvern",
            ["refresh"] = "重新整理同一次嘗試",
            ["close"] = "關閉",
            ["status.initial"] = "請選擇將擁有此 gewyvern runtime 的 daemon 權威端。",
            ["status.name"] = "Gewyvern 佈建狀態",
            ["phase.name"] = "Gewyvern 佈建階段",
            ["kicker"] = "RUNTIME 佈建",
            ["heading"] = "安裝並註冊 gewyvern",
            ["body"] = "所選 leserpentd 會執行原生 SSH 安裝、證明服務身分，並以原子方式註冊 runtime。離開此桌面端的只有不透明 vault 控制代碼。",
            ["authority.label"] = "所屬 daemon 權威端",
            ["provisioning_id.label"] = "佈建 ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "目標主機",
            ["port.label"] = "SSH 連接埠",
            ["credential.label"] = "SSH 憑證控制代碼",
            ["a11y.authority"] = "擁有此 gewyvern runtime 的 daemon 權威端",
            ["a11y.provisioning_id"] = "穩定的 gewyvern 佈建操作 ID",
            ["a11y.runtime_id"] = "佈建流程註冊的 runtime ID",
            ["a11y.host"] = "gewyvern 的安裝目標主機",
            ["a11y.port"] = "目標 SSH 連接埠",
            ["a11y.credential"] = "不透明 SSH 憑證控制代碼",
            ["a11y.confirm"] = "確認安裝並註冊 gewyvern",
            ["a11y.submit"] = "佈建並註冊 gewyvern runtime",
            ["a11y.refresh"] = "重新整理同一次佈建嘗試",
            ["a11y.close"] = "關閉 gewyvern 佈建視窗",
            ["phase.not_submitted"] = "尚未提交",
            ["phase.planned"] = "已規劃",
            ["phase.installing"] = "安裝中",
            ["phase.service_ready"] = "服務已就緒",
            ["phase.runtime_registered"] = "RUNTIME 已註冊",
            ["phase.failed"] = "失敗",
            ["error.confirm_required"] = "提交前請確認安裝 gewyvern。",
            ["error.authority_required"] = "請先選擇所屬 daemon 權威端。",
            ["status.observation_limit"] = "自動觀察已達受限上限。請使用「重新整理同一次嘗試」檢查這個確切的佈建 ID，不會建立另一次安裝。",
            ["status.waiting"] = "正在等待所選 daemon 權威端...",
            ["status.planned"] = "佈建已持久排入佇列。觀察會重複使用這個確切身分，不會提交第二次安裝。",
            ["status.installing"] = "daemon 權威端正在目標主機上安裝並啟動 gewyvern。",
            ["status.service_ready"] = "已在 {0} 驗證 gewyvern；正在等待以原子方式註冊 runtime。",
            ["status.runtime_registered"] = "runtime {0} 已註冊，並在 {1} 就緒。",
            ["status.failed"] = "佈建失敗，受限故障代碼為 {0}。請修正原因，再使用新的佈建 ID 開始新嘗試；此失敗身分會保持不可變以供稽核。",
            ["unavailable"] = "無法使用",
        });

    public static IReadOnlyDictionary<string, string> Japanese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / gewyvern をプロビジョニング",
            ["confirmation"] = "このホストへの gewyvern のインストールと runtime 登録を確認しました",
            ["submit"] = "gewyvern をプロビジョニング",
            ["refresh"] = "同じ試行を更新",
            ["close"] = "閉じる",
            ["status.initial"] = "この gewyvern runtime を所有する daemon authority を選択してください。",
            ["status.name"] = "Gewyvern プロビジョニング状態",
            ["phase.name"] = "Gewyvern プロビジョニング段階",
            ["kicker"] = "RUNTIME プロビジョニング",
            ["heading"] = "gewyvern をインストールして登録",
            ["body"] = "選択した leserpentd がネイティブ SSH インストールを実行し、サービス ID を証明して runtime をアトミックに登録します。このデスクトップから送信されるのは不透明な vault ハンドルだけです。",
            ["authority.label"] = "所有 daemon authority",
            ["provisioning_id.label"] = "プロビジョニング ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "対象ホスト",
            ["port.label"] = "SSH ポート",
            ["credential.label"] = "SSH 認証情報ハンドル",
            ["a11y.authority"] = "gewyvern runtime を所有する daemon authority",
            ["a11y.provisioning_id"] = "安定した gewyvern プロビジョニング操作 ID",
            ["a11y.runtime_id"] = "プロビジョニングで登録する runtime ID",
            ["a11y.host"] = "gewyvern のインストール対象ホスト",
            ["a11y.port"] = "対象 SSH ポート",
            ["a11y.credential"] = "不透明な SSH 認証情報ハンドル",
            ["a11y.confirm"] = "gewyvern のインストールと登録を確認",
            ["a11y.submit"] = "gewyvern runtime をプロビジョニングして登録",
            ["a11y.refresh"] = "同じプロビジョニング試行を更新",
            ["a11y.close"] = "gewyvern プロビジョニングウィンドウを閉じる",
            ["phase.not_submitted"] = "未送信",
            ["phase.planned"] = "計画済み",
            ["phase.installing"] = "インストール中",
            ["phase.service_ready"] = "サービス準備完了",
            ["phase.runtime_registered"] = "RUNTIME 登録済み",
            ["phase.failed"] = "失敗",
            ["error.confirm_required"] = "送信前に gewyvern のインストールを確認してください。",
            ["error.authority_required"] = "先に所有 daemon authority を選択してください。",
            ["status.observation_limit"] = "自動観察が上限に達しました。「同じ試行を更新」でこのプロビジョニング ID を調べてください。別のインストールは作成されません。",
            ["status.waiting"] = "選択した daemon authority を待機しています...",
            ["status.planned"] = "プロビジョニングは永続キューに登録されました。観察は同じ ID を再利用し、2 回目のインストールを送信しません。",
            ["status.installing"] = "daemon authority が対象ホストへ gewyvern をインストールして起動しています。",
            ["status.service_ready"] = "{0} で gewyvern を検証しました。runtime のアトミック登録を待機しています。",
            ["status.runtime_registered"] = "runtime {0} を登録し、{1} で準備が完了しました。",
            ["status.failed"] = "プロビジョニングは制限付き障害 {0} で失敗しました。原因を修正し、新しいプロビジョニング ID で新規試行を開始してください。この失敗 ID は監査用に不変のまま保持されます。",
            ["unavailable"] = "利用不可",
        });

    public static IReadOnlyDictionary<string, string> Spanish { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Aprovisionar gewyvern",
            ["confirmation"] = "Confirmo la instalación de gewyvern y el registro del runtime en este host",
            ["submit"] = "Aprovisionar gewyvern",
            ["refresh"] = "Actualizar el mismo intento",
            ["close"] = "Cerrar",
            ["status.initial"] = "Elige la autoridad daemon que será propietaria de este runtime gewyvern.",
            ["status.name"] = "Estado del aprovisionamiento de gewyvern",
            ["phase.name"] = "Fase del aprovisionamiento de gewyvern",
            ["kicker"] = "APROVISIONAMIENTO DEL RUNTIME",
            ["heading"] = "Instalar y registrar gewyvern",
            ["body"] = "El leserpentd seleccionado realiza la instalación SSH nativa, demuestra la identidad del servicio y registra el runtime de forma atómica. Solo un identificador opaco de vault sale de este escritorio.",
            ["authority.label"] = "Autoridad daemon propietaria",
            ["provisioning_id.label"] = "ID de aprovisionamiento",
            ["runtime_id.label"] = "ID del runtime",
            ["host.label"] = "Host de destino",
            ["port.label"] = "Puerto SSH",
            ["credential.label"] = "Identificador de credencial SSH",
            ["a11y.authority"] = "Autoridad daemon propietaria del runtime gewyvern",
            ["a11y.provisioning_id"] = "ID estable de la operación de aprovisionamiento de gewyvern",
            ["a11y.runtime_id"] = "ID del runtime registrado por el aprovisionamiento",
            ["a11y.host"] = "Host de destino para instalar gewyvern",
            ["a11y.port"] = "Puerto SSH de destino",
            ["a11y.credential"] = "Identificador opaco de credencial SSH",
            ["a11y.confirm"] = "Confirmar la instalación y el registro de gewyvern",
            ["a11y.submit"] = "Aprovisionar y registrar el runtime gewyvern",
            ["a11y.refresh"] = "Actualizar el mismo intento de aprovisionamiento",
            ["a11y.close"] = "Cerrar la ventana de aprovisionamiento de gewyvern",
            ["phase.not_submitted"] = "NO ENVIADO",
            ["phase.planned"] = "PLANIFICADO",
            ["phase.installing"] = "INSTALANDO",
            ["phase.service_ready"] = "SERVICIO LISTO",
            ["phase.runtime_registered"] = "RUNTIME REGISTRADO",
            ["phase.failed"] = "FALLIDO",
            ["error.confirm_required"] = "Confirma la instalación de gewyvern antes de enviarla.",
            ["error.authority_required"] = "Selecciona primero una autoridad daemon propietaria.",
            ["status.observation_limit"] = "La observación automática alcanzó su límite. Usa «Actualizar el mismo intento» para inspeccionar este ID de aprovisionamiento sin crear otra instalación.",
            ["status.waiting"] = "Esperando a la autoridad daemon seleccionada...",
            ["status.planned"] = "El aprovisionamiento está en una cola duradera. La observación reutiliza esta identidad y no envía una segunda instalación.",
            ["status.installing"] = "La autoridad daemon está instalando y activando gewyvern en el host de destino.",
            ["status.service_ready"] = "Gewyvern está verificado en {0}; el registro atómico del runtime está pendiente.",
            ["status.runtime_registered"] = "El runtime {0} está registrado y listo en {1}.",
            ["status.failed"] = "El aprovisionamiento falló con el error acotado {0}. Corrige la causa e inicia otro intento con un nuevo ID de aprovisionamiento; esta identidad fallida permanece inmutable para auditoría.",
            ["unavailable"] = "no disponible",
        });

    public static IReadOnlyDictionary<string, string> German { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Gewyvern bereitstellen",
            ["confirmation"] = "Ich bestätige die Installation von gewyvern und die Runtime-Registrierung auf diesem Host",
            ["submit"] = "Gewyvern bereitstellen",
            ["refresh"] = "Denselben Versuch aktualisieren",
            ["close"] = "Schließen",
            ["status.initial"] = "Wählen Sie die Daemon-Autorität aus, die diese Gewyvern-Runtime besitzt.",
            ["status.name"] = "Status der Gewyvern-Bereitstellung",
            ["phase.name"] = "Phase der Gewyvern-Bereitstellung",
            ["kicker"] = "RUNTIME-BEREITSTELLUNG",
            ["heading"] = "Gewyvern installieren und registrieren",
            ["body"] = "Das ausgewählte leserpentd führt die native SSH-Installation aus, weist die Dienstidentität nach und registriert die Runtime atomar. Nur ein undurchsichtiges Vault-Handle verlässt diesen Desktop.",
            ["authority.label"] = "Besitzende Daemon-Autorität",
            ["provisioning_id.label"] = "Bereitstellungs-ID",
            ["runtime_id.label"] = "Runtime-ID",
            ["host.label"] = "Zielhost",
            ["port.label"] = "SSH-Port",
            ["credential.label"] = "SSH-Anmeldeinformations-Handle",
            ["a11y.authority"] = "Daemon-Autorität, der die Gewyvern-Runtime gehört",
            ["a11y.provisioning_id"] = "Stabile ID des Gewyvern-Bereitstellungsvorgangs",
            ["a11y.runtime_id"] = "Durch die Bereitstellung registrierte Runtime-ID",
            ["a11y.host"] = "Zielhost für die Gewyvern-Installation",
            ["a11y.port"] = "Ziel-SSH-Port",
            ["a11y.credential"] = "Undurchsichtiges SSH-Anmeldeinformations-Handle",
            ["a11y.confirm"] = "Gewyvern-Installation und Registrierung bestätigen",
            ["a11y.submit"] = "Gewyvern-Runtime bereitstellen und registrieren",
            ["a11y.refresh"] = "Denselben Bereitstellungsversuch aktualisieren",
            ["a11y.close"] = "Fenster zur Gewyvern-Bereitstellung schließen",
            ["phase.not_submitted"] = "NICHT GESENDET",
            ["phase.planned"] = "GEPLANT",
            ["phase.installing"] = "WIRD INSTALLIERT",
            ["phase.service_ready"] = "DIENST BEREIT",
            ["phase.runtime_registered"] = "RUNTIME REGISTRIERT",
            ["phase.failed"] = "FEHLGESCHLAGEN",
            ["error.confirm_required"] = "Bestätigen Sie vor dem Senden die Gewyvern-Installation.",
            ["error.authority_required"] = "Wählen Sie zuerst eine besitzende Daemon-Autorität aus.",
            ["status.observation_limit"] = "Die automatische Beobachtung hat ihr Limit erreicht. Prüfen Sie mit „Denselben Versuch aktualisieren“ diese Bereitstellungs-ID, ohne eine weitere Installation zu erstellen.",
            ["status.waiting"] = "Warten auf die ausgewählte Daemon-Autorität...",
            ["status.planned"] = "Die Bereitstellung wurde dauerhaft eingereiht. Die Beobachtung verwendet dieselbe Identität und sendet keine zweite Installation.",
            ["status.installing"] = "Die Daemon-Autorität installiert und aktiviert gewyvern auf dem Zielhost.",
            ["status.service_ready"] = "Gewyvern wurde unter {0} geprüft; die atomare Runtime-Registrierung steht noch aus.",
            ["status.runtime_registered"] = "Runtime {0} ist registriert und unter {1} bereit.",
            ["status.failed"] = "Die Bereitstellung ist mit dem begrenzten Fehler {0} fehlgeschlagen. Beheben Sie die Ursache und starten Sie einen neuen Versuch mit einer neuen Bereitstellungs-ID; diese fehlgeschlagene Identität bleibt für die Prüfung unveränderlich.",
            ["unavailable"] = "nicht verfügbar",
        });

    public static IReadOnlyDictionary<string, string> French { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Provisionner gewyvern",
            ["confirmation"] = "Je confirme l’installation de gewyvern et l’enregistrement du runtime sur cet hôte",
            ["submit"] = "Provisionner gewyvern",
            ["refresh"] = "Actualiser la même tentative",
            ["close"] = "Fermer",
            ["status.initial"] = "Choisissez l’autorité daemon propriétaire de ce runtime gewyvern.",
            ["status.name"] = "État du provisionnement de gewyvern",
            ["phase.name"] = "Phase du provisionnement de gewyvern",
            ["kicker"] = "PROVISIONNEMENT DU RUNTIME",
            ["heading"] = "Installer et enregistrer gewyvern",
            ["body"] = "Le leserpentd sélectionné effectue l’installation SSH native, prouve l’identité du service et enregistre le runtime de façon atomique. Seule une référence vault opaque quitte cette application.",
            ["authority.label"] = "Autorité daemon propriétaire",
            ["provisioning_id.label"] = "ID de provisionnement",
            ["runtime_id.label"] = "ID du runtime",
            ["host.label"] = "Hôte cible",
            ["port.label"] = "Port SSH",
            ["credential.label"] = "Référence de justificatif SSH",
            ["a11y.authority"] = "Autorité daemon propriétaire du runtime gewyvern",
            ["a11y.provisioning_id"] = "ID stable de l’opération de provisionnement de gewyvern",
            ["a11y.runtime_id"] = "ID du runtime enregistré par le provisionnement",
            ["a11y.host"] = "Hôte cible de l’installation de gewyvern",
            ["a11y.port"] = "Port SSH cible",
            ["a11y.credential"] = "Référence opaque de justificatif SSH",
            ["a11y.confirm"] = "Confirmer l’installation et l’enregistrement de gewyvern",
            ["a11y.submit"] = "Provisionner et enregistrer le runtime gewyvern",
            ["a11y.refresh"] = "Actualiser la même tentative de provisionnement",
            ["a11y.close"] = "Fermer la fenêtre de provisionnement de gewyvern",
            ["phase.not_submitted"] = "NON ENVOYÉ",
            ["phase.planned"] = "PLANIFIÉ",
            ["phase.installing"] = "INSTALLATION EN COURS",
            ["phase.service_ready"] = "SERVICE PRÊT",
            ["phase.runtime_registered"] = "RUNTIME ENREGISTRÉ",
            ["phase.failed"] = "ÉCHEC",
            ["error.confirm_required"] = "Confirmez l’installation de gewyvern avant l’envoi.",
            ["error.authority_required"] = "Sélectionnez d’abord une autorité daemon propriétaire.",
            ["status.observation_limit"] = "L’observation automatique a atteint sa limite. Utilisez « Actualiser la même tentative » pour inspecter cet ID de provisionnement sans créer une autre installation.",
            ["status.waiting"] = "En attente de l’autorité daemon sélectionnée...",
            ["status.planned"] = "Le provisionnement est placé dans une file durable. L’observation réutilise cette identité et n’envoie pas une seconde installation.",
            ["status.installing"] = "L’autorité daemon installe et active gewyvern sur l’hôte cible.",
            ["status.service_ready"] = "Gewyvern est vérifié à l’adresse {0} ; l’enregistrement atomique du runtime est en attente.",
            ["status.runtime_registered"] = "Le runtime {0} est enregistré et prêt à l’adresse {1}.",
            ["status.failed"] = "Le provisionnement a échoué avec l’erreur bornée {0}. Corrigez la cause, puis démarrez une nouvelle tentative avec un nouvel ID de provisionnement ; cette identité en échec reste immuable pour l’audit.",
            ["unavailable"] = "indisponible",
        });

    public static IReadOnlyDictionary<string, string> Korean { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / gewyvern 프로비저닝",
            ["confirmation"] = "이 호스트에 gewyvern을 설치하고 runtime을 등록하는 작업을 확인합니다",
            ["submit"] = "gewyvern 프로비저닝",
            ["refresh"] = "같은 시도 새로고침",
            ["close"] = "닫기",
            ["status.initial"] = "이 gewyvern runtime을 소유할 daemon 권한 주체를 선택하세요.",
            ["status.name"] = "Gewyvern 프로비저닝 상태",
            ["phase.name"] = "Gewyvern 프로비저닝 단계",
            ["kicker"] = "RUNTIME 프로비저닝",
            ["heading"] = "gewyvern 설치 및 등록",
            ["body"] = "선택한 leserpentd가 네이티브 SSH 설치를 수행하고 서비스 ID를 증명한 뒤 runtime을 원자적으로 등록합니다. 이 데스크톱에서는 불투명한 vault 핸들만 전송됩니다.",
            ["authority.label"] = "소유 daemon 권한 주체",
            ["provisioning_id.label"] = "프로비저닝 ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "대상 호스트",
            ["port.label"] = "SSH 포트",
            ["credential.label"] = "SSH 자격 증명 핸들",
            ["a11y.authority"] = "gewyvern runtime을 소유하는 daemon 권한 주체",
            ["a11y.provisioning_id"] = "안정적인 gewyvern 프로비저닝 작업 ID",
            ["a11y.runtime_id"] = "프로비저닝으로 등록되는 runtime ID",
            ["a11y.host"] = "gewyvern 설치 대상 호스트",
            ["a11y.port"] = "대상 SSH 포트",
            ["a11y.credential"] = "불투명한 SSH 자격 증명 핸들",
            ["a11y.confirm"] = "gewyvern 설치 및 등록 확인",
            ["a11y.submit"] = "gewyvern runtime 프로비저닝 및 등록",
            ["a11y.refresh"] = "같은 프로비저닝 시도 새로고침",
            ["a11y.close"] = "gewyvern 프로비저닝 창 닫기",
            ["phase.not_submitted"] = "제출되지 않음",
            ["phase.planned"] = "계획됨",
            ["phase.installing"] = "설치 중",
            ["phase.service_ready"] = "서비스 준비됨",
            ["phase.runtime_registered"] = "RUNTIME 등록됨",
            ["phase.failed"] = "실패",
            ["error.confirm_required"] = "제출하기 전에 gewyvern 설치를 확인하세요.",
            ["error.authority_required"] = "먼저 소유 daemon 권한 주체를 선택하세요.",
            ["status.observation_limit"] = "자동 관찰이 제한에 도달했습니다. 다른 설치를 만들지 않고 이 프로비저닝 ID를 확인하려면 ‘같은 시도 새로고침’을 사용하세요.",
            ["status.waiting"] = "선택한 daemon 권한 주체를 기다리는 중...",
            ["status.planned"] = "프로비저닝이 영구 큐에 등록되었습니다. 관찰은 이 ID를 재사용하며 두 번째 설치를 제출하지 않습니다.",
            ["status.installing"] = "daemon 권한 주체가 대상 호스트에 gewyvern을 설치하고 활성화하고 있습니다.",
            ["status.service_ready"] = "{0}에서 gewyvern을 확인했습니다. runtime의 원자적 등록을 기다리고 있습니다.",
            ["status.runtime_registered"] = "runtime {0}이(가) 등록되었으며 {1}에서 준비되었습니다.",
            ["status.failed"] = "제한된 오류 {0}(으)로 프로비저닝에 실패했습니다. 원인을 해결하고 새 프로비저닝 ID로 새 시도를 시작하세요. 실패한 ID는 감사를 위해 변경되지 않습니다.",
            ["unavailable"] = "사용할 수 없음",
        });

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
        var expected = English.Keys.ToHashSet(StringComparer.Ordinal);
        var formattedKeys = new Dictionary<string, int>(StringComparer.Ordinal)
        {
            [FullKey("status.service_ready")] = 1,
            [FullKey("status.runtime_registered")] = 2,
            [FullKey("status.failed")] = 1,
        };
        if (expected.Count != KeyCount)
        {
            throw new InvalidDataException(
                "desktop provisioning localization key contract drifted");
        }
        foreach (var catalog in All)
        {
            if (catalog.Count != KeyCount
                || !catalog.Keys.ToHashSet(StringComparer.Ordinal).SetEquals(expected)
                || catalog.Any(entry => entry.Key.Length is <= 0 or > 128
                    || entry.Value.Length is <= 0 or > 1024
                    || entry.Key.Any(char.IsControl)
                    || entry.Value.Any(char.IsControl)
                    || !HasExpectedPlaceholders(
                        entry.Value,
                        formattedKeys.GetValueOrDefault(entry.Key))))
            {
                throw new InvalidDataException(
                    "desktop provisioning localization catalog is incomplete");
            }
            foreach (var entry in catalog)
            {
                VerifyFormat(entry.Value, formattedKeys.GetValueOrDefault(entry.Key));
            }
        }
    }

    private static IReadOnlyList<IReadOnlyDictionary<string, string>> All =>
        [English, SimplifiedChinese, TraditionalChinese, Japanese, Spanish, German, French, Korean];

    private static IReadOnlyDictionary<string, string> Catalog(
        Dictionary<string, string> values) => values.ToDictionary(
            entry => FullKey(entry.Key),
            entry => entry.Value,
            StringComparer.Ordinal);

    private static string FullKey(string key) => $"{Prefix}{key}";

    private static bool HasExpectedPlaceholders(string value, int arity)
    {
        for (var index = 0; index < 3; index++)
        {
            if (value.Contains($"{{{index}}}", StringComparison.Ordinal) != (index < arity))
            {
                return false;
            }
        }
        return !value.Contains('{') || arity > 0;
    }

    private static void VerifyFormat(string format, int arity)
    {
        try
        {
            var values = Enumerable.Repeat<object>("fixture", arity).ToArray();
            var value = string.Format(CultureInfo.InvariantCulture, format, values);
            if (string.IsNullOrWhiteSpace(value) || value.Any(char.IsControl))
            {
                throw new InvalidDataException(
                    "desktop provisioning localization produced invalid text");
            }
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                "desktop provisioning localization format is invalid",
                error);
        }
    }
}
