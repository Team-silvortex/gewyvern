using System.Globalization;

internal static class DesktopConnectionCatalogs
{
    private const string Prefix = "desktop.connection.";
    public const int KeyCount = 33;

    public static IReadOnlyDictionary<string, string> English { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Connect",
            ["token.placeholder"] = "Leave blank to use the existing platform credential",
            ["remember"] = "Remember this non-secret connection profile",
            ["connect"] = "Connect",
            ["test"] = "Test connection",
            ["choose_ca"] = "Choose CA...",
            ["cancel"] = "Cancel",
            ["quit"] = "Quit",
            ["forget_saved"] = "Forget saved connection...",
            ["status.name"] = "Connection setup status",
            ["test.help"] = "Checks TLS, authentication, protocol version, and authority readiness without saving the connection.",
            ["heading"] = "Connect the desktop console",
            ["body.bootstrap"] = "This connection retains its endpoint-bound bootstrap trust handle. Enter a token only to replace the endpoint credential; CA material stays managed outside the UI.",
            ["body.standard"] = "Enter a token once to store it in macOS Keychain or Linux Secret Service. Remembered CA certificates are copied into private application storage; the profile stores only the HTTPS origin and managed CA path.",
            ["endpoint.label"] = "HTTPS authority",
            ["ca.label"] = "CA certificate",
            ["token.label"] = "Endpoint-scoped token (optional)",
            ["managed_ca"] = "Managed by {0}",
            ["picker.title"] = "Choose the trusted CA certificate",
            ["picker.pem"] = "PEM certificate",
            ["status.removed"] = "Saved profile and endpoint credential removed.",
            ["status.testing"] = "Testing TLS, authentication, and authority readiness...",
            ["status.ready"] = "Connection verified. The remote authority is ready.",
            ["status.test_failed"] = "Connection test failed safely.",
            ["account"] = "Credential account: {0}",
            ["account.pending"] = "Credential account appears after a valid HTTPS origin is entered.",
            ["status.failed"] = "Connection setup failed: {0}",
            ["forget.title"] = "Leserpent / Forget Connection",
            ["forget.action"] = "Forget connection",
            ["forget.status.name"] = "Forget connection status",
            ["forget.heading"] = "Forget this connection?",
            ["forget.body"] = "This removes the saved non-secret profile and this endpoint's Keychain or Secret Service credential. Environment variables and credentials for other endpoints are not changed.",
            ["forget.failed"] = "Forget connection failed: {0}",
        });

    public static IReadOnlyDictionary<string, string> SimplifiedChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 连接",
            ["token.placeholder"] = "留空以使用现有平台凭证",
            ["remember"] = "记住此非机密连接配置",
            ["connect"] = "连接",
            ["test"] = "测试连接",
            ["choose_ca"] = "选择 CA...",
            ["cancel"] = "取消",
            ["quit"] = "退出",
            ["forget_saved"] = "忘记已保存的连接...",
            ["status.name"] = "连接设置状态",
            ["test.help"] = "在不保存连接的情况下检查 TLS、身份验证、协议版本和权威端就绪状态。",
            ["heading"] = "连接桌面控制台",
            ["body.bootstrap"] = "此连接保留其端点绑定的引导信任句柄。仅在替换端点凭证时输入令牌；CA 材料仍由界面外部管理。",
            ["body.standard"] = "输入一次令牌即可将其存入 macOS 钥匙串或 Linux Secret Service。记住的 CA 证书会复制到应用私有存储；配置仅保存 HTTPS 源和托管 CA 路径。",
            ["endpoint.label"] = "HTTPS 权威端",
            ["ca.label"] = "CA 证书",
            ["token.label"] = "端点范围令牌（可选）",
            ["managed_ca"] = "由 {0} 管理",
            ["picker.title"] = "选择可信 CA 证书",
            ["picker.pem"] = "PEM 证书",
            ["status.removed"] = "已删除保存的配置和端点凭证。",
            ["status.testing"] = "正在检查 TLS、身份验证和权威端就绪状态...",
            ["status.ready"] = "连接已验证，远程权威端已就绪。",
            ["status.test_failed"] = "连接测试已安全失败。",
            ["account"] = "凭证账户：{0}",
            ["account.pending"] = "输入有效的 HTTPS 源后将显示凭证账户。",
            ["status.failed"] = "连接设置失败：{0}",
            ["forget.title"] = "Leserpent / 忘记连接",
            ["forget.action"] = "忘记连接",
            ["forget.status.name"] = "忘记连接状态",
            ["forget.heading"] = "忘记此连接？",
            ["forget.body"] = "这将删除保存的非机密配置和此端点的钥匙串或 Secret Service 凭证。环境变量及其他端点的凭证不会更改。",
            ["forget.failed"] = "忘记连接失败：{0}",
        });

    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 連線",
            ["token.placeholder"] = "留空以使用現有平台憑證",
            ["remember"] = "記住此非機密連線設定檔",
            ["connect"] = "連線",
            ["test"] = "測試連線",
            ["choose_ca"] = "選擇 CA...",
            ["cancel"] = "取消",
            ["quit"] = "結束",
            ["forget_saved"] = "忘記已儲存的連線...",
            ["status.name"] = "連線設定狀態",
            ["test.help"] = "在不儲存連線的情況下檢查 TLS、身分驗證、協定版本和權威端就緒狀態。",
            ["heading"] = "連線桌面控制台",
            ["body.bootstrap"] = "此連線保留其端點綁定的引導信任控制代碼。僅在取代端點憑證時輸入令牌；CA 資料仍由介面外部管理。",
            ["body.standard"] = "輸入一次令牌即可將其存入 macOS 鑰匙圈或 Linux Secret Service。記住的 CA 憑證會複製到應用程式私有儲存空間；設定檔只儲存 HTTPS 來源和受管理的 CA 路徑。",
            ["endpoint.label"] = "HTTPS 權威端",
            ["ca.label"] = "CA 憑證",
            ["token.label"] = "端點範圍令牌（選用）",
            ["managed_ca"] = "由 {0} 管理",
            ["picker.title"] = "選擇受信任的 CA 憑證",
            ["picker.pem"] = "PEM 憑證",
            ["status.removed"] = "已移除儲存的設定檔和端點憑證。",
            ["status.testing"] = "正在檢查 TLS、身分驗證和權威端就緒狀態...",
            ["status.ready"] = "連線已驗證，遠端權威端已就緒。",
            ["status.test_failed"] = "連線測試已安全失敗。",
            ["account"] = "憑證帳戶：{0}",
            ["account.pending"] = "輸入有效的 HTTPS 來源後將顯示憑證帳戶。",
            ["status.failed"] = "連線設定失敗：{0}",
            ["forget.title"] = "Leserpent / 忘記連線",
            ["forget.action"] = "忘記連線",
            ["forget.status.name"] = "忘記連線狀態",
            ["forget.heading"] = "忘記此連線？",
            ["forget.body"] = "這會移除儲存的非機密設定檔及此端點的鑰匙圈或 Secret Service 憑證。環境變數和其他端點的憑證不會變更。",
            ["forget.failed"] = "忘記連線失敗：{0}",
        });

    public static IReadOnlyDictionary<string, string> Japanese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 接続",
            ["token.placeholder"] = "既存のプラットフォーム資格情報を使う場合は空欄にします",
            ["remember"] = "機密情報を含まない接続プロファイルを保存",
            ["connect"] = "接続",
            ["test"] = "接続をテスト",
            ["choose_ca"] = "CA を選択...",
            ["cancel"] = "キャンセル",
            ["quit"] = "終了",
            ["forget_saved"] = "保存済み接続を消去...",
            ["status.name"] = "接続設定の状態",
            ["test.help"] = "接続を保存せずに TLS、認証、プロトコルバージョン、権威の準備状態を確認します。",
            ["heading"] = "デスクトップコンソールを接続",
            ["body.bootstrap"] = "この接続はエンドポイントに結び付いたブートストラップ信頼ハンドルを保持します。トークンはエンドポイント資格情報を置き換える場合にのみ入力し、CA 資料は UI の外部で管理されます。",
            ["body.standard"] = "トークンを一度入力すると macOS キーチェーンまたは Linux Secret Service に保存されます。記憶した CA 証明書はアプリ専用領域へコピーされ、プロファイルには HTTPS オリジンと管理対象 CA パスだけが保存されます。",
            ["endpoint.label"] = "HTTPS 権威",
            ["ca.label"] = "CA 証明書",
            ["token.label"] = "エンドポイント専用トークン（任意）",
            ["managed_ca"] = "{0} が管理",
            ["picker.title"] = "信頼する CA 証明書を選択",
            ["picker.pem"] = "PEM 証明書",
            ["status.removed"] = "保存済みプロファイルとエンドポイント資格情報を削除しました。",
            ["status.testing"] = "TLS、認証、権威の準備状態を確認しています...",
            ["status.ready"] = "接続を確認しました。リモート権威は準備完了です。",
            ["status.test_failed"] = "接続テストは安全に失敗しました。",
            ["account"] = "資格情報アカウント: {0}",
            ["account.pending"] = "有効な HTTPS オリジンを入力すると資格情報アカウントが表示されます。",
            ["status.failed"] = "接続設定に失敗しました: {0}",
            ["forget.title"] = "Leserpent / 接続を消去",
            ["forget.action"] = "接続を消去",
            ["forget.status.name"] = "接続消去の状態",
            ["forget.heading"] = "この接続を消去しますか？",
            ["forget.body"] = "保存済みの非機密プロファイルと、このエンドポイントのキーチェーンまたは Secret Service 資格情報を削除します。環境変数と他のエンドポイントの資格情報は変更されません。",
            ["forget.failed"] = "接続の消去に失敗しました: {0}",
        });

    public static IReadOnlyDictionary<string, string> Spanish { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Conectar",
            ["token.placeholder"] = "Déjalo vacío para usar la credencial de plataforma existente",
            ["remember"] = "Recordar este perfil de conexión sin secretos",
            ["connect"] = "Conectar",
            ["test"] = "Probar conexión",
            ["choose_ca"] = "Elegir CA...",
            ["cancel"] = "Cancelar",
            ["quit"] = "Salir",
            ["forget_saved"] = "Olvidar conexión guardada...",
            ["status.name"] = "Estado de configuración de la conexión",
            ["test.help"] = "Comprueba TLS, autenticación, versión del protocolo y disponibilidad de la autoridad sin guardar la conexión.",
            ["heading"] = "Conectar la consola de escritorio",
            ["body.bootstrap"] = "Esta conexión conserva su identificador de confianza de bootstrap vinculado al endpoint. Introduce un token solo para sustituir la credencial del endpoint; el material de CA permanece administrado fuera de la interfaz.",
            ["body.standard"] = "Introduce el token una vez para guardarlo en el llavero de macOS o en Secret Service de Linux. Los certificados CA recordados se copian al almacenamiento privado de la aplicación; el perfil solo guarda el origen HTTPS y la ruta CA administrada.",
            ["endpoint.label"] = "Autoridad HTTPS",
            ["ca.label"] = "Certificado CA",
            ["token.label"] = "Token limitado al endpoint (opcional)",
            ["managed_ca"] = "Administrado por {0}",
            ["picker.title"] = "Elegir el certificado CA de confianza",
            ["picker.pem"] = "Certificado PEM",
            ["status.removed"] = "Se eliminaron el perfil guardado y la credencial del endpoint.",
            ["status.testing"] = "Comprobando TLS, autenticación y disponibilidad de la autoridad...",
            ["status.ready"] = "Conexión verificada. La autoridad remota está lista.",
            ["status.test_failed"] = "La prueba de conexión falló de forma segura.",
            ["account"] = "Cuenta de credenciales: {0}",
            ["account.pending"] = "La cuenta de credenciales aparecerá tras introducir un origen HTTPS válido.",
            ["status.failed"] = "Error al configurar la conexión: {0}",
            ["forget.title"] = "Leserpent / Olvidar conexión",
            ["forget.action"] = "Olvidar conexión",
            ["forget.status.name"] = "Estado de olvido de la conexión",
            ["forget.heading"] = "¿Olvidar esta conexión?",
            ["forget.body"] = "Esto elimina el perfil guardado sin secretos y la credencial de este endpoint del llavero o Secret Service. Las variables de entorno y las credenciales de otros endpoints no cambian.",
            ["forget.failed"] = "No se pudo olvidar la conexión: {0}",
        });

    public static IReadOnlyDictionary<string, string> German { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Verbinden",
            ["token.placeholder"] = "Leer lassen, um die vorhandenen Plattform-Zugangsdaten zu verwenden",
            ["remember"] = "Dieses geheimnisfreie Verbindungsprofil speichern",
            ["connect"] = "Verbinden",
            ["test"] = "Verbindung testen",
            ["choose_ca"] = "CA auswählen...",
            ["cancel"] = "Abbrechen",
            ["quit"] = "Beenden",
            ["forget_saved"] = "Gespeicherte Verbindung vergessen...",
            ["status.name"] = "Status der Verbindungseinrichtung",
            ["test.help"] = "Prüft TLS, Authentifizierung, Protokollversion und Bereitschaft der Autorität, ohne die Verbindung zu speichern.",
            ["heading"] = "Desktop-Konsole verbinden",
            ["body.bootstrap"] = "Diese Verbindung behält ihren endpointgebundenen Bootstrap-Vertrauenshandle. Gib nur dann ein Token ein, wenn die Endpoint-Zugangsdaten ersetzt werden sollen; CA-Material bleibt außerhalb der Oberfläche verwaltet.",
            ["body.standard"] = "Gib das Token einmal ein, um es im macOS-Schlüsselbund oder Linux Secret Service zu speichern. Gemerkte CA-Zertifikate werden in den privaten Anwendungsspeicher kopiert; das Profil speichert nur den HTTPS-Ursprung und den verwalteten CA-Pfad.",
            ["endpoint.label"] = "HTTPS-Autorität",
            ["ca.label"] = "CA-Zertifikat",
            ["token.label"] = "Endpointgebundenes Token (optional)",
            ["managed_ca"] = "Verwaltet von {0}",
            ["picker.title"] = "Vertrauenswürdiges CA-Zertifikat auswählen",
            ["picker.pem"] = "PEM-Zertifikat",
            ["status.removed"] = "Gespeichertes Profil und Endpoint-Zugangsdaten wurden entfernt.",
            ["status.testing"] = "TLS, Authentifizierung und Bereitschaft der Autorität werden geprüft...",
            ["status.ready"] = "Verbindung bestätigt. Die Remote-Autorität ist bereit.",
            ["status.test_failed"] = "Der Verbindungstest ist sicher fehlgeschlagen.",
            ["account"] = "Zugangsdatenkonto: {0}",
            ["account.pending"] = "Das Zugangsdatenkonto erscheint nach Eingabe eines gültigen HTTPS-Ursprungs.",
            ["status.failed"] = "Verbindungseinrichtung fehlgeschlagen: {0}",
            ["forget.title"] = "Leserpent / Verbindung vergessen",
            ["forget.action"] = "Verbindung vergessen",
            ["forget.status.name"] = "Status zum Vergessen der Verbindung",
            ["forget.heading"] = "Diese Verbindung vergessen?",
            ["forget.body"] = "Dadurch werden das gespeicherte geheimnisfreie Profil und die Zugangsdaten dieses Endpoints aus Schlüsselbund oder Secret Service entfernt. Umgebungsvariablen und Zugangsdaten anderer Endpoints bleiben unverändert.",
            ["forget.failed"] = "Verbindung konnte nicht vergessen werden: {0}",
        });

    public static IReadOnlyDictionary<string, string> French { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Connexion",
            ["token.placeholder"] = "Laisser vide pour utiliser l’identifiant de plateforme existant",
            ["remember"] = "Mémoriser ce profil de connexion sans secret",
            ["connect"] = "Se connecter",
            ["test"] = "Tester la connexion",
            ["choose_ca"] = "Choisir la CA...",
            ["cancel"] = "Annuler",
            ["quit"] = "Quitter",
            ["forget_saved"] = "Oublier la connexion enregistrée...",
            ["status.name"] = "État de configuration de la connexion",
            ["test.help"] = "Vérifie TLS, l’authentification, la version du protocole et la disponibilité de l’autorité sans enregistrer la connexion.",
            ["heading"] = "Connecter la console de bureau",
            ["body.bootstrap"] = "Cette connexion conserve son handle de confiance bootstrap lié à l’endpoint. Saisissez un token uniquement pour remplacer l’identifiant de l’endpoint ; le matériel CA reste géré hors de l’interface.",
            ["body.standard"] = "Saisissez le token une fois pour le stocker dans le trousseau macOS ou Linux Secret Service. Les certificats CA mémorisés sont copiés dans le stockage privé de l’application ; le profil ne conserve que l’origine HTTPS et le chemin CA géré.",
            ["endpoint.label"] = "Autorité HTTPS",
            ["ca.label"] = "Certificat CA",
            ["token.label"] = "Token limité à l’endpoint (facultatif)",
            ["managed_ca"] = "Géré par {0}",
            ["picker.title"] = "Choisir le certificat CA de confiance",
            ["picker.pem"] = "Certificat PEM",
            ["status.removed"] = "Le profil enregistré et l’identifiant de l’endpoint ont été supprimés.",
            ["status.testing"] = "Vérification de TLS, de l’authentification et de la disponibilité de l’autorité...",
            ["status.ready"] = "Connexion vérifiée. L’autorité distante est prête.",
            ["status.test_failed"] = "Le test de connexion a échoué sans risque.",
            ["account"] = "Compte d’identifiants : {0}",
            ["account.pending"] = "Le compte d’identifiants apparaîtra après la saisie d’une origine HTTPS valide.",
            ["status.failed"] = "Échec de la configuration de la connexion : {0}",
            ["forget.title"] = "Leserpent / Oublier la connexion",
            ["forget.action"] = "Oublier la connexion",
            ["forget.status.name"] = "État de suppression de la connexion",
            ["forget.heading"] = "Oublier cette connexion ?",
            ["forget.body"] = "Cette action supprime le profil enregistré sans secret et l’identifiant de cet endpoint du trousseau ou de Secret Service. Les variables d’environnement et les identifiants des autres endpoints restent inchangés.",
            ["forget.failed"] = "Impossible d’oublier la connexion : {0}",
        });

    public static IReadOnlyDictionary<string, string> Korean { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 연결",
            ["token.placeholder"] = "기존 플랫폼 자격 증명을 사용하려면 비워 두세요",
            ["remember"] = "비밀 정보가 없는 연결 프로필 기억",
            ["connect"] = "연결",
            ["test"] = "연결 테스트",
            ["choose_ca"] = "CA 선택...",
            ["cancel"] = "취소",
            ["quit"] = "종료",
            ["forget_saved"] = "저장된 연결 지우기...",
            ["status.name"] = "연결 설정 상태",
            ["test.help"] = "연결을 저장하지 않고 TLS, 인증, 프로토콜 버전과 권한 주체 준비 상태를 확인합니다.",
            ["heading"] = "데스크톱 콘솔 연결",
            ["body.bootstrap"] = "이 연결은 endpoint에 바인딩된 bootstrap 신뢰 핸들을 유지합니다. endpoint 자격 증명을 교체할 때만 토큰을 입력하며 CA 자료는 UI 외부에서 계속 관리됩니다.",
            ["body.standard"] = "토큰을 한 번 입력하면 macOS 키체인 또는 Linux Secret Service에 저장됩니다. 기억한 CA 인증서는 애플리케이션 전용 저장소로 복사되며 프로필에는 HTTPS origin과 관리되는 CA 경로만 저장됩니다.",
            ["endpoint.label"] = "HTTPS 권한 주체",
            ["ca.label"] = "CA 인증서",
            ["token.label"] = "endpoint 범위 토큰(선택 사항)",
            ["managed_ca"] = "{0}에서 관리",
            ["picker.title"] = "신뢰할 CA 인증서 선택",
            ["picker.pem"] = "PEM 인증서",
            ["status.removed"] = "저장된 프로필과 endpoint 자격 증명을 제거했습니다.",
            ["status.testing"] = "TLS, 인증과 권한 주체 준비 상태를 확인하는 중...",
            ["status.ready"] = "연결을 확인했습니다. 원격 권한 주체가 준비되었습니다.",
            ["status.test_failed"] = "연결 테스트가 안전하게 실패했습니다.",
            ["account"] = "자격 증명 계정: {0}",
            ["account.pending"] = "유효한 HTTPS origin을 입력하면 자격 증명 계정이 표시됩니다.",
            ["status.failed"] = "연결 설정 실패: {0}",
            ["forget.title"] = "Leserpent / 연결 지우기",
            ["forget.action"] = "연결 지우기",
            ["forget.status.name"] = "연결 지우기 상태",
            ["forget.heading"] = "이 연결을 지울까요?",
            ["forget.body"] = "저장된 비밀 정보 없는 프로필과 이 endpoint의 키체인 또는 Secret Service 자격 증명을 제거합니다. 환경 변수와 다른 endpoint의 자격 증명은 변경되지 않습니다.",
            ["forget.failed"] = "연결을 지우지 못했습니다: {0}",
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
        var formattedKeys = new HashSet<string>(StringComparer.Ordinal)
        {
            FullKey("managed_ca"),
            FullKey("account"),
            FullKey("status.failed"),
            FullKey("forget.failed"),
        };
        if (expected.Count != KeyCount)
        {
            throw new InvalidDataException("desktop connection localization key contract drifted");
        }
        foreach (var catalog in All)
        {
            if (catalog.Count != KeyCount
                || !catalog.Keys.ToHashSet(StringComparer.Ordinal).SetEquals(expected)
                || catalog.Any(entry => entry.Key.Length is <= 0 or > 128
                    || entry.Value.Length is <= 0 or > 1024
                    || entry.Key.Any(char.IsControl)
                    || entry.Value.Any(char.IsControl)
                    || entry.Value.Contains("{0}", StringComparison.Ordinal)
                        != formattedKeys.Contains(entry.Key)))
            {
                throw new InvalidDataException(
                    "desktop connection localization catalog is incomplete");
            }
            foreach (var value in catalog.Values)
            {
                VerifyFormat(value);
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

    private static void VerifyFormat(string format)
    {
        try
        {
            var value = string.Format(CultureInfo.InvariantCulture, format, "fixture");
            if (string.IsNullOrWhiteSpace(value) || value.Any(char.IsControl))
            {
                throw new InvalidDataException(
                    "desktop connection localization produced invalid text");
            }
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                "desktop connection localization format is invalid",
                error);
        }
    }
}
