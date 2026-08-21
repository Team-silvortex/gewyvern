using System.Globalization;

internal static class DesktopBootstrapDeploymentCatalogs
{
    private const string Prefix = "desktop.bootstrap.";
    public const int KeyCount = 46;

    public static IReadOnlyDictionary<string, string> English { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Deploy daemon",
            ["confirmation"] = "I confirm deployment changes on the selected target host",
            ["deploy"] = "Deploy leserpentd",
            ["refresh"] = "Refresh status",
            ["bind"] = "Verify & bind session",
            ["promote"] = "Add to Hub",
            ["close"] = "Close",
            ["status.initial"] = "Choose an existing daemon authority to perform the deployment.",
            ["status.name"] = "Bootstrap deployment status",
            ["kicker"] = "REVERSE DEPLOYMENT",
            ["heading"] = "Deploy a daemon authority",
            ["body"] = "An authenticated leserpentd authority performs native SSH deployment. The desktop sends only an opaque credential handle, never a password or private key.",
            ["authority.label"] = "Deployment authority",
            ["bootstrap_id.label"] = "Bootstrap ID",
            ["host.label"] = "Target host",
            ["port.label"] = "SSH port",
            ["credential.label"] = "SSH credential handle",
            ["a11y.authority"] = "Daemon authority performing bootstrap deployment",
            ["a11y.bootstrap_id"] = "Stable bootstrap operation ID",
            ["a11y.host"] = "Target host for leserpent daemon deployment",
            ["a11y.port"] = "Target SSH port",
            ["a11y.credential"] = "Opaque SSH credential handle",
            ["a11y.confirm"] = "Confirm target host deployment",
            ["a11y.deploy"] = "Deploy leserpent daemon to target host",
            ["a11y.refresh"] = "Refresh bootstrap deployment status",
            ["a11y.bind"] = "Verify and bind deployed daemon session",
            ["a11y.promote"] = "Add authenticated daemon connection to Hub",
            ["a11y.close"] = "Close daemon deployment window",
            ["a11y.status"] = "Bootstrap deployment status",
            ["phase.not_submitted"] = "NOT SUBMITTED",
            ["phase.planned"] = "PLANNED",
            ["phase.deploying"] = "DEPLOYING",
            ["phase.bootstrapped"] = "BOOTSTRAPPED",
            ["phase.session_bound"] = "SESSION BOUND",
            ["phase.failed"] = "FAILED",
            ["error.confirm_required"] = "Confirm target deployment before submitting.",
            ["error.authority_required"] = "Select a deployment authority first.",
            ["status.promoting"] = "Verifying target trust and session credential before saving...",
            ["status.promoted"] = "Daemon {0} was verified and added to the Hub.",
            ["status.waiting"] = "Waiting for the selected authority...",
            ["status.planned"] = "Deployment is durably queued. Status refresh will continue without resubmitting the effect.",
            ["status.deploying"] = "The authority is reconciling the target host.",
            ["status.bootstrapped"] = "Daemon {0} is reachable at {1}. Verify and bind its session authority before mutations.",
            ["status.session_bound"] = "Daemon {0} is authenticated and mutation authority is enabled.",
            ["status.failed"] = "Deployment failed with bounded fault {0}.",
            ["unavailable"] = "unavailable",
        });

    public static IReadOnlyDictionary<string, string> SimplifiedChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 部署 daemon",
            ["confirmation"] = "我确认将更改所选目标主机上的部署",
            ["deploy"] = "部署 leserpentd",
            ["refresh"] = "刷新状态",
            ["bind"] = "验证并绑定会话",
            ["promote"] = "添加到 Hub",
            ["close"] = "关闭",
            ["status.initial"] = "请选择一个现有 daemon 权威端来执行部署。",
            ["status.name"] = "引导部署状态",
            ["kicker"] = "反向部署",
            ["heading"] = "部署 daemon 权威端",
            ["body"] = "由已通过身份验证的 leserpentd 权威端执行原生 SSH 部署。桌面端只发送不透明凭证句柄，绝不发送密码或私钥。",
            ["authority.label"] = "部署权威端",
            ["bootstrap_id.label"] = "引导 ID",
            ["host.label"] = "目标主机",
            ["port.label"] = "SSH 端口",
            ["credential.label"] = "SSH 凭证句柄",
            ["a11y.authority"] = "执行引导部署的 daemon 权威端",
            ["a11y.bootstrap_id"] = "稳定的引导操作 ID",
            ["a11y.host"] = "leserpent daemon 的部署目标主机",
            ["a11y.port"] = "目标 SSH 端口",
            ["a11y.credential"] = "不透明 SSH 凭证句柄",
            ["a11y.confirm"] = "确认目标主机部署",
            ["a11y.deploy"] = "将 leserpent daemon 部署到目标主机",
            ["a11y.refresh"] = "刷新引导部署状态",
            ["a11y.bind"] = "验证并绑定已部署 daemon 的会话",
            ["a11y.promote"] = "将已验证的 daemon 连接添加到 Hub",
            ["a11y.close"] = "关闭 daemon 部署窗口",
            ["a11y.status"] = "引导部署状态",
            ["phase.not_submitted"] = "尚未提交",
            ["phase.planned"] = "已规划",
            ["phase.deploying"] = "部署中",
            ["phase.bootstrapped"] = "已引导",
            ["phase.session_bound"] = "会话已绑定",
            ["phase.failed"] = "失败",
            ["error.confirm_required"] = "提交前请确认目标部署。",
            ["error.authority_required"] = "请先选择部署权威端。",
            ["status.promoting"] = "正在验证目标信任和会话凭证，然后再保存...",
            ["status.promoted"] = "daemon {0} 已通过验证并添加到 Hub。",
            ["status.waiting"] = "正在等待所选权威端...",
            ["status.planned"] = "部署已持久排队。状态刷新会继续进行，不会重复提交副作用。",
            ["status.deploying"] = "权威端正在协调目标主机。",
            ["status.bootstrapped"] = "daemon {0} 已可通过 {1} 访问。执行变更前，请验证并绑定其会话权威。",
            ["status.session_bound"] = "daemon {0} 已通过身份验证，并已启用变更权威。",
            ["status.failed"] = "部署失败，受限故障代码为 {0}。",
            ["unavailable"] = "不可用",
        });

    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 部署 daemon",
            ["confirmation"] = "我確認將變更所選目標主機上的部署",
            ["deploy"] = "部署 leserpentd",
            ["refresh"] = "重新整理狀態",
            ["bind"] = "驗證並綁定工作階段",
            ["promote"] = "加入 Hub",
            ["close"] = "關閉",
            ["status.initial"] = "請選擇現有 daemon 權威端來執行部署。",
            ["status.name"] = "引導部署狀態",
            ["kicker"] = "反向部署",
            ["heading"] = "部署 daemon 權威端",
            ["body"] = "由已驗證身分的 leserpentd 權威端執行原生 SSH 部署。桌面端只傳送不透明憑證控制代碼，絕不傳送密碼或私密金鑰。",
            ["authority.label"] = "部署權威端",
            ["bootstrap_id.label"] = "引導 ID",
            ["host.label"] = "目標主機",
            ["port.label"] = "SSH 連接埠",
            ["credential.label"] = "SSH 憑證控制代碼",
            ["a11y.authority"] = "執行引導部署的 daemon 權威端",
            ["a11y.bootstrap_id"] = "穩定的引導操作 ID",
            ["a11y.host"] = "leserpent daemon 的部署目標主機",
            ["a11y.port"] = "目標 SSH 連接埠",
            ["a11y.credential"] = "不透明 SSH 憑證控制代碼",
            ["a11y.confirm"] = "確認目標主機部署",
            ["a11y.deploy"] = "將 leserpent daemon 部署到目標主機",
            ["a11y.refresh"] = "重新整理引導部署狀態",
            ["a11y.bind"] = "驗證並綁定已部署 daemon 的工作階段",
            ["a11y.promote"] = "將已驗證的 daemon 連線加入 Hub",
            ["a11y.close"] = "關閉 daemon 部署視窗",
            ["a11y.status"] = "引導部署狀態",
            ["phase.not_submitted"] = "尚未提交",
            ["phase.planned"] = "已規劃",
            ["phase.deploying"] = "部署中",
            ["phase.bootstrapped"] = "已引導",
            ["phase.session_bound"] = "工作階段已綁定",
            ["phase.failed"] = "失敗",
            ["error.confirm_required"] = "提交前請確認目標部署。",
            ["error.authority_required"] = "請先選擇部署權威端。",
            ["status.promoting"] = "正在驗證目標信任與工作階段憑證，然後再儲存...",
            ["status.promoted"] = "daemon {0} 已通過驗證並加入 Hub。",
            ["status.waiting"] = "正在等待所選權威端...",
            ["status.planned"] = "部署已持久排入佇列。狀態重新整理會繼續進行，不會重複提交副作用。",
            ["status.deploying"] = "權威端正在協調目標主機。",
            ["status.bootstrapped"] = "daemon {0} 已可透過 {1} 存取。執行變更前，請驗證並綁定其工作階段權威。",
            ["status.session_bound"] = "daemon {0} 已通過身分驗證，並已啟用變更權威。",
            ["status.failed"] = "部署失敗，受限故障代碼為 {0}。",
            ["unavailable"] = "無法使用",
        });

    public static IReadOnlyDictionary<string, string> Japanese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / daemon をデプロイ",
            ["confirmation"] = "選択した対象ホストのデプロイ変更を確認しました",
            ["deploy"] = "leserpentd をデプロイ",
            ["refresh"] = "状態を更新",
            ["bind"] = "セッションを検証してバインド",
            ["promote"] = "Hub に追加",
            ["close"] = "閉じる",
            ["status.initial"] = "デプロイを実行する既存の daemon authority を選択してください。",
            ["status.name"] = "ブートストラップデプロイ状態",
            ["kicker"] = "リバースデプロイ",
            ["heading"] = "daemon authority をデプロイ",
            ["body"] = "認証済みの leserpentd authority がネイティブ SSH デプロイを実行します。デスクトップが送信するのは不透明な認証情報ハンドルだけで、パスワードや秘密鍵は送信しません。",
            ["authority.label"] = "デプロイ authority",
            ["bootstrap_id.label"] = "ブートストラップ ID",
            ["host.label"] = "対象ホスト",
            ["port.label"] = "SSH ポート",
            ["credential.label"] = "SSH 認証情報ハンドル",
            ["a11y.authority"] = "ブートストラップデプロイを実行する daemon authority",
            ["a11y.bootstrap_id"] = "安定したブートストラップ操作 ID",
            ["a11y.host"] = "leserpent daemon のデプロイ対象ホスト",
            ["a11y.port"] = "対象 SSH ポート",
            ["a11y.credential"] = "不透明な SSH 認証情報ハンドル",
            ["a11y.confirm"] = "対象ホストへのデプロイを確認",
            ["a11y.deploy"] = "leserpent daemon を対象ホストへデプロイ",
            ["a11y.refresh"] = "ブートストラップデプロイ状態を更新",
            ["a11y.bind"] = "デプロイ済み daemon のセッションを検証してバインド",
            ["a11y.promote"] = "認証済み daemon 接続を Hub に追加",
            ["a11y.close"] = "daemon デプロイウィンドウを閉じる",
            ["a11y.status"] = "ブートストラップデプロイ状態",
            ["phase.not_submitted"] = "未送信",
            ["phase.planned"] = "計画済み",
            ["phase.deploying"] = "デプロイ中",
            ["phase.bootstrapped"] = "ブートストラップ済み",
            ["phase.session_bound"] = "セッションバインド済み",
            ["phase.failed"] = "失敗",
            ["error.confirm_required"] = "送信前に対象へのデプロイを確認してください。",
            ["error.authority_required"] = "先にデプロイ authority を選択してください。",
            ["status.promoting"] = "保存前に対象の信頼とセッション認証情報を検証しています...",
            ["status.promoted"] = "daemon {0} を検証して Hub に追加しました。",
            ["status.waiting"] = "選択した authority を待機しています...",
            ["status.planned"] = "デプロイは永続キューに登録されました。効果を再送信せずに状態更新を継続します。",
            ["status.deploying"] = "authority が対象ホストを調整しています。",
            ["status.bootstrapped"] = "daemon {0} は {1} で到達可能です。変更前にセッション authority を検証してバインドしてください。",
            ["status.session_bound"] = "daemon {0} は認証済みで、変更 authority が有効です。",
            ["status.failed"] = "デプロイは制限付き障害 {0} で失敗しました。",
            ["unavailable"] = "利用不可",
        });

    public static IReadOnlyDictionary<string, string> Spanish { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Desplegar daemon",
            ["confirmation"] = "Confirmo los cambios de despliegue en el host de destino seleccionado",
            ["deploy"] = "Desplegar leserpentd",
            ["refresh"] = "Actualizar estado",
            ["bind"] = "Verificar y vincular sesión",
            ["promote"] = "Añadir al Hub",
            ["close"] = "Cerrar",
            ["status.initial"] = "Elige una autoridad daemon existente para realizar el despliegue.",
            ["status.name"] = "Estado del despliegue bootstrap",
            ["kicker"] = "DESPLIEGUE INVERSO",
            ["heading"] = "Desplegar una autoridad daemon",
            ["body"] = "Una autoridad leserpentd autenticada realiza el despliegue SSH nativo. El escritorio solo envía un identificador opaco de credencial, nunca una contraseña ni una clave privada.",
            ["authority.label"] = "Autoridad de despliegue",
            ["bootstrap_id.label"] = "ID de bootstrap",
            ["host.label"] = "Host de destino",
            ["port.label"] = "Puerto SSH",
            ["credential.label"] = "Identificador de credencial SSH",
            ["a11y.authority"] = "Autoridad daemon que realiza el despliegue bootstrap",
            ["a11y.bootstrap_id"] = "ID estable de la operación bootstrap",
            ["a11y.host"] = "Host de destino para desplegar el daemon de leserpent",
            ["a11y.port"] = "Puerto SSH de destino",
            ["a11y.credential"] = "Identificador opaco de credencial SSH",
            ["a11y.confirm"] = "Confirmar el despliegue en el host de destino",
            ["a11y.deploy"] = "Desplegar el daemon de leserpent en el host de destino",
            ["a11y.refresh"] = "Actualizar el estado del despliegue bootstrap",
            ["a11y.bind"] = "Verificar y vincular la sesión del daemon desplegado",
            ["a11y.promote"] = "Añadir la conexión daemon autenticada al Hub",
            ["a11y.close"] = "Cerrar la ventana de despliegue del daemon",
            ["a11y.status"] = "Estado del despliegue bootstrap",
            ["phase.not_submitted"] = "NO ENVIADO",
            ["phase.planned"] = "PLANIFICADO",
            ["phase.deploying"] = "DESPLEGANDO",
            ["phase.bootstrapped"] = "INICIALIZADO",
            ["phase.session_bound"] = "SESIÓN VINCULADA",
            ["phase.failed"] = "FALLIDO",
            ["error.confirm_required"] = "Confirma el despliegue de destino antes de enviarlo.",
            ["error.authority_required"] = "Selecciona primero una autoridad de despliegue.",
            ["status.promoting"] = "Verificando la confianza del destino y la credencial de sesión antes de guardar...",
            ["status.promoted"] = "El daemon {0} se verificó y se añadió al Hub.",
            ["status.waiting"] = "Esperando a la autoridad seleccionada...",
            ["status.planned"] = "El despliegue está en una cola duradera. La actualización del estado continuará sin reenviar el efecto.",
            ["status.deploying"] = "La autoridad está conciliando el host de destino.",
            ["status.bootstrapped"] = "El daemon {0} está disponible en {1}. Verifica y vincula su autoridad de sesión antes de realizar cambios.",
            ["status.session_bound"] = "El daemon {0} está autenticado y la autoridad de cambios está habilitada.",
            ["status.failed"] = "El despliegue falló con el error acotado {0}.",
            ["unavailable"] = "no disponible",
        });

    public static IReadOnlyDictionary<string, string> German { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Daemon bereitstellen",
            ["confirmation"] = "Ich bestätige die Bereitstellungsänderungen auf dem ausgewählten Zielhost",
            ["deploy"] = "leserpentd bereitstellen",
            ["refresh"] = "Status aktualisieren",
            ["bind"] = "Sitzung prüfen und binden",
            ["promote"] = "Zum Hub hinzufügen",
            ["close"] = "Schließen",
            ["status.initial"] = "Wählen Sie eine vorhandene Daemon-Autorität für die Bereitstellung aus.",
            ["status.name"] = "Status der Bootstrap-Bereitstellung",
            ["kicker"] = "RÜCKWÄRTSBEREITSTELLUNG",
            ["heading"] = "Daemon-Autorität bereitstellen",
            ["body"] = "Eine authentifizierte leserpentd-Autorität führt die native SSH-Bereitstellung aus. Der Desktop sendet nur ein undurchsichtiges Anmeldeinformations-Handle, niemals ein Passwort oder einen privaten Schlüssel.",
            ["authority.label"] = "Bereitstellungsautorität",
            ["bootstrap_id.label"] = "Bootstrap-ID",
            ["host.label"] = "Zielhost",
            ["port.label"] = "SSH-Port",
            ["credential.label"] = "SSH-Anmeldeinformations-Handle",
            ["a11y.authority"] = "Daemon-Autorität für die Bootstrap-Bereitstellung",
            ["a11y.bootstrap_id"] = "Stabile ID des Bootstrap-Vorgangs",
            ["a11y.host"] = "Zielhost für die Bereitstellung des leserpent-Daemons",
            ["a11y.port"] = "Ziel-SSH-Port",
            ["a11y.credential"] = "Undurchsichtiges SSH-Anmeldeinformations-Handle",
            ["a11y.confirm"] = "Bereitstellung auf dem Zielhost bestätigen",
            ["a11y.deploy"] = "leserpent-Daemon auf dem Zielhost bereitstellen",
            ["a11y.refresh"] = "Status der Bootstrap-Bereitstellung aktualisieren",
            ["a11y.bind"] = "Sitzung des bereitgestellten Daemons prüfen und binden",
            ["a11y.promote"] = "Authentifizierte Daemon-Verbindung zum Hub hinzufügen",
            ["a11y.close"] = "Fenster zur Daemon-Bereitstellung schließen",
            ["a11y.status"] = "Status der Bootstrap-Bereitstellung",
            ["phase.not_submitted"] = "NICHT GESENDET",
            ["phase.planned"] = "GEPLANT",
            ["phase.deploying"] = "WIRD BEREITGESTELLT",
            ["phase.bootstrapped"] = "INITIALISIERT",
            ["phase.session_bound"] = "SITZUNG GEBUNDEN",
            ["phase.failed"] = "FEHLGESCHLAGEN",
            ["error.confirm_required"] = "Bestätigen Sie vor dem Senden die Zielbereitstellung.",
            ["error.authority_required"] = "Wählen Sie zuerst eine Bereitstellungsautorität aus.",
            ["status.promoting"] = "Zielvertrauen und Sitzungsanmeldedaten werden vor dem Speichern geprüft...",
            ["status.promoted"] = "Daemon {0} wurde geprüft und zum Hub hinzugefügt.",
            ["status.waiting"] = "Warten auf die ausgewählte Autorität...",
            ["status.planned"] = "Die Bereitstellung wurde dauerhaft eingereiht. Der Status wird weiter aktualisiert, ohne den Effekt erneut zu senden.",
            ["status.deploying"] = "Die Autorität gleicht den Zielhost ab.",
            ["status.bootstrapped"] = "Daemon {0} ist unter {1} erreichbar. Prüfen und binden Sie seine Sitzungsautorität vor Änderungen.",
            ["status.session_bound"] = "Daemon {0} ist authentifiziert und die Änderungsautorität ist aktiviert.",
            ["status.failed"] = "Die Bereitstellung ist mit dem begrenzten Fehler {0} fehlgeschlagen.",
            ["unavailable"] = "nicht verfügbar",
        });

    public static IReadOnlyDictionary<string, string> French { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Déployer un daemon",
            ["confirmation"] = "Je confirme les changements de déploiement sur l’hôte cible sélectionné",
            ["deploy"] = "Déployer leserpentd",
            ["refresh"] = "Actualiser l’état",
            ["bind"] = "Vérifier et lier la session",
            ["promote"] = "Ajouter au Hub",
            ["close"] = "Fermer",
            ["status.initial"] = "Choisissez une autorité daemon existante pour effectuer le déploiement.",
            ["status.name"] = "État du déploiement bootstrap",
            ["kicker"] = "DÉPLOIEMENT INVERSÉ",
            ["heading"] = "Déployer une autorité daemon",
            ["body"] = "Une autorité leserpentd authentifiée effectue le déploiement SSH natif. L’application de bureau envoie uniquement une référence opaque de justificatif, jamais un mot de passe ni une clé privée.",
            ["authority.label"] = "Autorité de déploiement",
            ["bootstrap_id.label"] = "ID de bootstrap",
            ["host.label"] = "Hôte cible",
            ["port.label"] = "Port SSH",
            ["credential.label"] = "Référence de justificatif SSH",
            ["a11y.authority"] = "Autorité daemon effectuant le déploiement bootstrap",
            ["a11y.bootstrap_id"] = "ID stable de l’opération bootstrap",
            ["a11y.host"] = "Hôte cible du déploiement du daemon leserpent",
            ["a11y.port"] = "Port SSH cible",
            ["a11y.credential"] = "Référence opaque de justificatif SSH",
            ["a11y.confirm"] = "Confirmer le déploiement sur l’hôte cible",
            ["a11y.deploy"] = "Déployer le daemon leserpent sur l’hôte cible",
            ["a11y.refresh"] = "Actualiser l’état du déploiement bootstrap",
            ["a11y.bind"] = "Vérifier et lier la session du daemon déployé",
            ["a11y.promote"] = "Ajouter la connexion daemon authentifiée au Hub",
            ["a11y.close"] = "Fermer la fenêtre de déploiement du daemon",
            ["a11y.status"] = "État du déploiement bootstrap",
            ["phase.not_submitted"] = "NON ENVOYÉ",
            ["phase.planned"] = "PLANIFIÉ",
            ["phase.deploying"] = "DÉPLOIEMENT EN COURS",
            ["phase.bootstrapped"] = "INITIALISÉ",
            ["phase.session_bound"] = "SESSION LIÉE",
            ["phase.failed"] = "ÉCHEC",
            ["error.confirm_required"] = "Confirmez le déploiement cible avant l’envoi.",
            ["error.authority_required"] = "Sélectionnez d’abord une autorité de déploiement.",
            ["status.promoting"] = "Vérification de la confiance cible et du justificatif de session avant l’enregistrement...",
            ["status.promoted"] = "Le daemon {0} a été vérifié et ajouté au Hub.",
            ["status.waiting"] = "En attente de l’autorité sélectionnée...",
            ["status.planned"] = "Le déploiement est placé dans une file durable. L’état continuera d’être actualisé sans renvoyer l’effet.",
            ["status.deploying"] = "L’autorité réconcilie l’hôte cible.",
            ["status.bootstrapped"] = "Le daemon {0} est accessible à l’adresse {1}. Vérifiez et liez son autorité de session avant toute modification.",
            ["status.session_bound"] = "Le daemon {0} est authentifié et l’autorité de modification est activée.",
            ["status.failed"] = "Le déploiement a échoué avec l’erreur bornée {0}.",
            ["unavailable"] = "indisponible",
        });

    public static IReadOnlyDictionary<string, string> Korean { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / daemon 배포",
            ["confirmation"] = "선택한 대상 호스트의 배포 변경을 확인합니다",
            ["deploy"] = "leserpentd 배포",
            ["refresh"] = "상태 새로고침",
            ["bind"] = "세션 확인 및 바인딩",
            ["promote"] = "Hub에 추가",
            ["close"] = "닫기",
            ["status.initial"] = "배포를 수행할 기존 daemon 권한 주체를 선택하세요.",
            ["status.name"] = "bootstrap 배포 상태",
            ["kicker"] = "역방향 배포",
            ["heading"] = "daemon 권한 주체 배포",
            ["body"] = "인증된 leserpentd 권한 주체가 네이티브 SSH 배포를 수행합니다. 데스크톱은 불투명한 자격 증명 핸들만 전송하며 비밀번호나 개인 키는 전송하지 않습니다.",
            ["authority.label"] = "배포 권한 주체",
            ["bootstrap_id.label"] = "bootstrap ID",
            ["host.label"] = "대상 호스트",
            ["port.label"] = "SSH 포트",
            ["credential.label"] = "SSH 자격 증명 핸들",
            ["a11y.authority"] = "bootstrap 배포를 수행하는 daemon 권한 주체",
            ["a11y.bootstrap_id"] = "안정적인 bootstrap 작업 ID",
            ["a11y.host"] = "leserpent daemon 배포 대상 호스트",
            ["a11y.port"] = "대상 SSH 포트",
            ["a11y.credential"] = "불투명한 SSH 자격 증명 핸들",
            ["a11y.confirm"] = "대상 호스트 배포 확인",
            ["a11y.deploy"] = "대상 호스트에 leserpent daemon 배포",
            ["a11y.refresh"] = "bootstrap 배포 상태 새로고침",
            ["a11y.bind"] = "배포된 daemon 세션 확인 및 바인딩",
            ["a11y.promote"] = "인증된 daemon 연결을 Hub에 추가",
            ["a11y.close"] = "daemon 배포 창 닫기",
            ["a11y.status"] = "bootstrap 배포 상태",
            ["phase.not_submitted"] = "제출되지 않음",
            ["phase.planned"] = "계획됨",
            ["phase.deploying"] = "배포 중",
            ["phase.bootstrapped"] = "bootstrap 완료",
            ["phase.session_bound"] = "세션 바인딩됨",
            ["phase.failed"] = "실패",
            ["error.confirm_required"] = "제출하기 전에 대상 배포를 확인하세요.",
            ["error.authority_required"] = "먼저 배포 권한 주체를 선택하세요.",
            ["status.promoting"] = "저장하기 전에 대상 신뢰와 세션 자격 증명을 확인하는 중...",
            ["status.promoted"] = "daemon {0}을(를) 확인하여 Hub에 추가했습니다.",
            ["status.waiting"] = "선택한 권한 주체를 기다리는 중...",
            ["status.planned"] = "배포가 영구 큐에 등록되었습니다. 효과를 다시 제출하지 않고 상태 새로고침을 계속합니다.",
            ["status.deploying"] = "권한 주체가 대상 호스트를 조정하고 있습니다.",
            ["status.bootstrapped"] = "daemon {0}에 {1}(으)로 연결할 수 있습니다. 변경 전에 세션 권한을 확인하고 바인딩하세요.",
            ["status.session_bound"] = "daemon {0}이(가) 인증되었으며 변경 권한이 활성화되었습니다.",
            ["status.failed"] = "제한된 오류 {0}(으)로 배포에 실패했습니다.",
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
            [FullKey("status.promoted")] = 1,
            [FullKey("status.bootstrapped")] = 2,
            [FullKey("status.session_bound")] = 1,
            [FullKey("status.failed")] = 1,
        };
        if (expected.Count != KeyCount)
        {
            throw new InvalidDataException(
                "desktop bootstrap localization key contract drifted");
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
                    "desktop bootstrap localization catalog is incomplete");
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
                    "desktop bootstrap localization produced invalid text");
            }
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                "desktop bootstrap localization format is invalid",
                error);
        }
    }
}
