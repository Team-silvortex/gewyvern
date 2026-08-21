using System.Globalization;

internal static class DesktopStartupRecoveryCatalogs
{
    private const string Prefix = "desktop.startup_recovery.";
    public const int KeyCount = 9;

    public static IReadOnlyDictionary<string, string> English { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent startup problem",
            ["close"] = "Close",
            ["heading"] = "Remote console could not start",
            ["guidance"] = "Check the HTTPS origin, CA file, and the endpoint-scoped token in Keychain or Secret Service. Tokens are never shown here.",
            ["detail.fallback"] = "The desktop configuration could not be validated.",
            ["a11y.close"] = "Close startup error",
            ["a11y.detail"] = "Startup error: {0}",
            ["a11y.heading"] = "Remote console could not start",
            ["a11y.guidance"] = "Check the HTTPS origin, CA file, and endpoint-scoped platform credential",
        });

    public static IReadOnlyDictionary<string, string> SimplifiedChinese { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent 启动问题",
            ["close"] = "关闭",
            ["heading"] = "远程控制台无法启动",
            ["guidance"] = "请检查 HTTPS 源站、CA 文件，以及 Keychain 或 Secret Service 中限定端点的令牌。令牌绝不会显示在这里。",
            ["detail.fallback"] = "无法验证桌面配置。",
            ["a11y.close"] = "关闭启动错误",
            ["a11y.detail"] = "启动错误：{0}",
            ["a11y.heading"] = "远程控制台无法启动",
            ["a11y.guidance"] = "检查 HTTPS 源站、CA 文件和限定端点的平台凭证",
        });

    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent 啟動問題",
            ["close"] = "關閉",
            ["heading"] = "遠端主控台無法啟動",
            ["guidance"] = "請檢查 HTTPS 來源、CA 檔案，以及 Keychain 或 Secret Service 中限定端點的權杖。權杖絕不會顯示在這裡。",
            ["detail.fallback"] = "無法驗證桌面設定。",
            ["a11y.close"] = "關閉啟動錯誤",
            ["a11y.detail"] = "啟動錯誤：{0}",
            ["a11y.heading"] = "遠端主控台無法啟動",
            ["a11y.guidance"] = "檢查 HTTPS 來源、CA 檔案和限定端點的平台憑證",
        });

    public static IReadOnlyDictionary<string, string> Japanese { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent の起動問題",
            ["close"] = "閉じる",
            ["heading"] = "リモートコンソールを起動できませんでした",
            ["guidance"] = "HTTPS オリジン、CA ファイル、Keychain または Secret Service のエンドポイント限定トークンを確認してください。トークンがここに表示されることはありません。",
            ["detail.fallback"] = "デスクトップ設定を検証できませんでした。",
            ["a11y.close"] = "起動エラーを閉じる",
            ["a11y.detail"] = "起動エラー: {0}",
            ["a11y.heading"] = "リモートコンソールを起動できませんでした",
            ["a11y.guidance"] = "HTTPS オリジン、CA ファイル、エンドポイント限定のプラットフォーム認証情報を確認",
        });

    public static IReadOnlyDictionary<string, string> Spanish { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Problema de inicio de Leserpent",
            ["close"] = "Cerrar",
            ["heading"] = "No se pudo iniciar la consola remota",
            ["guidance"] = "Comprueba el origen HTTPS, el archivo de CA y el token limitado al endpoint en Keychain o Secret Service. Los tokens nunca se muestran aquí.",
            ["detail.fallback"] = "No se pudo validar la configuración de escritorio.",
            ["a11y.close"] = "Cerrar el error de inicio",
            ["a11y.detail"] = "Error de inicio: {0}",
            ["a11y.heading"] = "No se pudo iniciar la consola remota",
            ["a11y.guidance"] = "Comprobar el origen HTTPS, el archivo de CA y la credencial de plataforma limitada al endpoint",
        });

    public static IReadOnlyDictionary<string, string> German { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent-Startproblem",
            ["close"] = "Schließen",
            ["heading"] = "Die Remotekonsole konnte nicht gestartet werden",
            ["guidance"] = "Prüfen Sie den HTTPS-Ursprung, die CA-Datei und das endpunktgebundene Token in Keychain oder Secret Service. Token werden hier nie angezeigt.",
            ["detail.fallback"] = "Die Desktopkonfiguration konnte nicht überprüft werden.",
            ["a11y.close"] = "Startfehler schließen",
            ["a11y.detail"] = "Startfehler: {0}",
            ["a11y.heading"] = "Die Remotekonsole konnte nicht gestartet werden",
            ["a11y.guidance"] = "HTTPS-Ursprung, CA-Datei und endpunktgebundene Plattformanmeldeinformationen prüfen",
        });

    public static IReadOnlyDictionary<string, string> French { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Problème de démarrage de Leserpent",
            ["close"] = "Fermer",
            ["heading"] = "La console distante n'a pas pu démarrer",
            ["guidance"] = "Vérifiez l'origine HTTPS, le fichier d'AC et le jeton limité au point de terminaison dans Keychain ou Secret Service. Les jetons ne sont jamais affichés ici.",
            ["detail.fallback"] = "La configuration du bureau n'a pas pu être validée.",
            ["a11y.close"] = "Fermer l'erreur de démarrage",
            ["a11y.detail"] = "Erreur de démarrage : {0}",
            ["a11y.heading"] = "La console distante n'a pas pu démarrer",
            ["a11y.guidance"] = "Vérifier l'origine HTTPS, le fichier d'AC et l'accréditation de plateforme limitée au point de terminaison",
        });

    public static IReadOnlyDictionary<string, string> Korean { get; } = Catalog(
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent 시작 문제",
            ["close"] = "닫기",
            ["heading"] = "원격 콘솔을 시작할 수 없습니다",
            ["guidance"] = "HTTPS 원본, CA 파일, Keychain 또는 Secret Service의 엔드포인트 범위 토큰을 확인하세요. 토큰은 여기에 표시되지 않습니다.",
            ["detail.fallback"] = "데스크톱 구성을 검증할 수 없습니다.",
            ["a11y.close"] = "시작 오류 닫기",
            ["a11y.detail"] = "시작 오류: {0}",
            ["a11y.heading"] = "원격 콘솔을 시작할 수 없습니다",
            ["a11y.guidance"] = "HTTPS 원본, CA 파일 및 엔드포인트 범위 플랫폼 자격 증명 확인",
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

    public static void VerifyContract() => DesktopDomainCatalogContract.Verify(
        "startup recovery",
        KeyCount,
        All,
        new Dictionary<string, int>(StringComparer.Ordinal)
        {
            [FullKey("a11y.detail")] = 1,
        });

    private static IReadOnlyList<IReadOnlyDictionary<string, string>> All =>
        [English, SimplifiedChinese, TraditionalChinese, Japanese, Spanish, German, French, Korean];

    private static IReadOnlyDictionary<string, string> Catalog(
        Dictionary<string, string> values) =>
        DesktopDomainCatalogContract.Catalog(Prefix, values);

    private static string FullKey(string key) => $"{Prefix}{key}";
}
