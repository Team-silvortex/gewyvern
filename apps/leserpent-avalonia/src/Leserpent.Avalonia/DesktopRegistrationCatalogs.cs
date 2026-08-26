using System.Globalization;

internal static class DesktopRegistrationCatalogs
{
    private const string Prefix = "desktop.registration.";
    public const int KeyCount = 49;

    private static readonly IReadOnlyDictionary<string, string> EnglishValues =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "Register existing runtime",
            ["entry.help"] = "Attach an already-running gewyvern service to this daemon authority without deploying it.",
            ["workspace.edit"] = "Edit registration",
            ["workspace.edit.help"] = "Review and update this runtime's authority-owned registration metadata.",
            ["title.register"] = "Leserpent / Register runtime",
            ["title.update"] = "Leserpent / Update registration",
            ["kicker"] = "RUNTIME AUTHORITY",
            ["heading.register"] = "Attach an existing runtime",
            ["heading.update"] = "Review registration metadata",
            ["body.register"] = "Register a running gewyvern endpoint with this leserpentd. No deployment credential or service secret crosses this boundary.",
            ["body.update"] = "Inspect the daemon-owned record, preview the exact replacement, then commit through a runtime revision fence.",
            ["runtime_id.label"] = "Runtime ID",
            ["name.label"] = "Display name",
            ["endpoint.label"] = "Gewyvern endpoint",
            ["sidecar.label"] = "Optional sidecar endpoint",
            ["environment.label"] = "Environment tag",
            ["cluster.label"] = "Cluster tag",
            ["role.label"] = "Role tag",
            ["status.name"] = "Runtime registration status",
            ["status.initial.register"] = "Enter the existing service coordinates, then ask the daemon authority to review them.",
            ["status.initial.update"] = "Authority record loaded at runtime revision {0}. Edit the metadata, then review a fenced update.",
            ["status.loading"] = "Loading the authority-owned registration record...",
            ["status.reviewing"] = "Requesting a side-effect-free registration plan...",
            ["status.plan_ready.register"] = "The daemon accepted a registration plan predicting authority revision {0}. Review it before confirming.",
            ["status.plan_ready.update"] = "The daemon accepted a revision-fenced update plan predicting authority revision {0}.",
            ["status.applying"] = "Applying the reviewed registration plan...",
            ["status.applied.register"] = "Runtime {0} was registered at authority revision {1}.",
            ["status.applied.update"] = "Runtime {0} registration was updated at authority revision {1}.",
            ["status.failed"] = "Registration operation failed: {0}",
            ["plan.heading"] = "REVIEWED AUTHORITY PLAN",
            ["plan.summary.name"] = "Reviewed runtime registration plan",
            ["plan.empty"] = "No reviewed plan is active.",
            ["plan.kind"] = "Operation: {0}",
            ["plan.kind.register"] = "register existing runtime",
            ["plan.kind.update"] = "update runtime registration",
            ["plan.identity"] = "Identity: {0} / {1}",
            ["plan.endpoint"] = "Endpoint: {0}",
            ["plan.sidecar"] = "Sidecar: {0}",
            ["plan.tags"] = "Tags: environment={0}, cluster={1}, role={2}",
            ["plan.revision.register"] = "Predicted authority revision: {0}",
            ["plan.revision.update"] = "Fence: runtime revision {0}; predicted authority revision: {1}",
            ["confirmation"] = "I reviewed this exact daemon plan and authorize its registration change",
            ["optional.none"] = "none",
            ["error.confirm"] = "Review the plan and explicitly confirm it before applying.",
            ["action.review"] = "Review plan",
            ["action.apply.register"] = "Register runtime",
            ["action.apply.update"] = "Update registration",
            ["action.edit"] = "Continue editing",
            ["action.close"] = "Close",
        };

    public static IReadOnlyDictionary<string, string> English { get; } =
        Catalog(EnglishValues);

    public static IReadOnlyDictionary<string, string> SimplifiedChinese { get; } =
        Catalog(Localized(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "注册已有 runtime",
            ["entry.help"] = "把已经运行的 gewyvern 服务接入此 daemon 权威端，不执行部署。",
            ["workspace.edit"] = "编辑注册信息",
            ["workspace.edit.help"] = "审查并更新此 runtime 由权威端持有的注册元数据。",
            ["title.register"] = "Leserpent / 注册 runtime",
            ["title.update"] = "Leserpent / 更新注册信息",
            ["kicker"] = "RUNTIME 权威",
            ["heading.register"] = "接入已有 runtime",
            ["heading.update"] = "审查注册元数据",
            ["body.register"] = "把正在运行的 gewyvern 端点注册到此 leserpentd。此边界不会传输部署凭证或服务密钥。",
            ["body.update"] = "读取 daemon 权威记录，预演精确替换内容，再通过 runtime revision 栅栏提交。",
            ["runtime_id.label"] = "Runtime ID",
            ["name.label"] = "显示名称",
            ["endpoint.label"] = "Gewyvern 端点",
            ["sidecar.label"] = "可选 sidecar 端点",
            ["environment.label"] = "环境标签",
            ["cluster.label"] = "集群标签",
            ["role.label"] = "角色标签",
            ["status.name"] = "Runtime 注册状态",
            ["status.initial.register"] = "填写已有服务坐标，然后交由 daemon 权威端审查。",
            ["status.initial.update"] = "已读取 runtime revision {0} 的权威记录。编辑元数据后审查带栅栏的更新。",
            ["status.loading"] = "正在读取权威端持有的注册记录...",
            ["status.reviewing"] = "正在请求无副作用的注册预演...",
            ["status.plan_ready.register"] = "daemon 已接受注册预演，预计权威 revision 为 {0}。请确认后再授权。",
            ["status.plan_ready.update"] = "daemon 已接受带 revision 栅栏的更新预演，预计权威 revision 为 {0}。",
            ["status.applying"] = "正在应用已审查的注册预演...",
            ["status.applied.register"] = "Runtime {0} 已在权威 revision {1} 注册。",
            ["status.applied.update"] = "Runtime {0} 的注册信息已在权威 revision {1} 更新。",
            ["status.failed"] = "注册操作失败：{0}",
            ["plan.heading"] = "已审查的权威预演",
            ["plan.summary.name"] = "已审查的 runtime 注册预演",
            ["plan.empty"] = "当前没有已审查的预演。",
            ["plan.kind"] = "操作：{0}",
            ["plan.kind.register"] = "注册已有 runtime",
            ["plan.kind.update"] = "更新 runtime 注册信息",
            ["plan.identity"] = "身份：{0} / {1}",
            ["plan.endpoint"] = "端点：{0}",
            ["plan.sidecar"] = "Sidecar：{0}",
            ["plan.tags"] = "标签：环境={0}，集群={1}，角色={2}",
            ["plan.revision.register"] = "预计权威 revision：{0}",
            ["plan.revision.update"] = "栅栏：runtime revision {0}；预计权威 revision：{1}",
            ["confirmation"] = "我已审查此 daemon 预演的确切内容，并授权修改注册信息",
            ["optional.none"] = "无",
            ["error.confirm"] = "应用前请审查预演并明确确认。",
            ["action.review"] = "审查预演",
            ["action.apply.register"] = "注册 runtime",
            ["action.apply.update"] = "更新注册信息",
            ["action.edit"] = "继续编辑",
            ["action.close"] = "关闭",
        }));

    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } =
        Catalog(Localized(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "註冊既有 runtime",
            ["entry.help"] = "將已執行的 gewyvern 服務接入此 daemon 權威端，不執行部署。",
            ["workspace.edit"] = "編輯註冊資訊",
            ["workspace.edit.help"] = "檢閱並更新此 runtime 由權威端持有的註冊中繼資料。",
            ["title.register"] = "Leserpent / 註冊 runtime",
            ["title.update"] = "Leserpent / 更新註冊資訊",
            ["kicker"] = "RUNTIME 權威",
            ["heading.register"] = "接入既有 runtime",
            ["heading.update"] = "檢閱註冊中繼資料",
            ["body.register"] = "將正在執行的 gewyvern 端點註冊至此 leserpentd。此邊界不會傳輸部署憑證或服務密鑰。",
            ["body.update"] = "讀取 daemon 權威記錄、預覽精確替換內容，再透過 runtime revision 柵欄提交。",
            ["name.label"] = "顯示名稱",
            ["endpoint.label"] = "Gewyvern 端點",
            ["sidecar.label"] = "選用 sidecar 端點",
            ["environment.label"] = "環境標籤",
            ["cluster.label"] = "叢集標籤",
            ["role.label"] = "角色標籤",
            ["status.name"] = "Runtime 註冊狀態",
            ["status.initial.register"] = "填寫既有服務座標，再交由 daemon 權威端檢閱。",
            ["status.initial.update"] = "已讀取 runtime revision {0} 的權威記錄。編輯中繼資料後檢閱帶柵欄的更新。",
            ["status.loading"] = "正在讀取權威端持有的註冊記錄...",
            ["status.reviewing"] = "正在要求無副作用的註冊預覽...",
            ["status.plan_ready.register"] = "daemon 已接受註冊預覽，預計權威 revision 為 {0}。請確認後再授權。",
            ["status.plan_ready.update"] = "daemon 已接受帶 revision 柵欄的更新預覽，預計權威 revision 為 {0}。",
            ["status.applying"] = "正在套用已檢閱的註冊預覽...",
            ["status.applied.register"] = "Runtime {0} 已在權威 revision {1} 註冊。",
            ["status.applied.update"] = "Runtime {0} 的註冊資訊已在權威 revision {1} 更新。",
            ["status.failed"] = "註冊操作失敗：{0}",
            ["plan.heading"] = "已檢閱的權威預覽",
            ["plan.summary.name"] = "已檢閱的 runtime 註冊預覽",
            ["plan.empty"] = "目前沒有已檢閱的預覽。",
            ["plan.kind"] = "操作：{0}",
            ["plan.kind.register"] = "註冊既有 runtime",
            ["plan.kind.update"] = "更新 runtime 註冊資訊",
            ["plan.identity"] = "身分：{0} / {1}",
            ["plan.endpoint"] = "端點：{0}",
            ["plan.sidecar"] = "Sidecar：{0}",
            ["plan.tags"] = "標籤：環境={0}，叢集={1}，角色={2}",
            ["plan.revision.register"] = "預計權威 revision：{0}",
            ["plan.revision.update"] = "柵欄：runtime revision {0}；預計權威 revision：{1}",
            ["confirmation"] = "我已檢閱此 daemon 預覽的確切內容，並授權修改註冊資訊",
            ["optional.none"] = "無",
            ["error.confirm"] = "套用前請檢閱預覽並明確確認。",
            ["action.review"] = "檢閱預覽",
            ["action.apply.register"] = "註冊 runtime",
            ["action.apply.update"] = "更新註冊資訊",
            ["action.edit"] = "繼續編輯",
            ["action.close"] = "關閉",
        }));

    public static IReadOnlyDictionary<string, string> Japanese { get; } =
        Catalog(Localized(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "既存 runtime を登録",
            ["entry.help"] = "実行中の gewyvern サービスを、デプロイせずにこの daemon authority へ接続します。",
            ["workspace.edit"] = "登録情報を編集",
            ["workspace.edit.help"] = "authority が所有する runtime 登録メタデータを確認して更新します。",
            ["title.register"] = "Leserpent / runtime を登録",
            ["title.update"] = "Leserpent / 登録情報を更新",
            ["kicker"] = "RUNTIME AUTHORITY",
            ["heading.register"] = "既存 runtime を接続",
            ["heading.update"] = "登録メタデータを確認",
            ["body.register"] = "実行中の gewyvern endpoint をこの leserpentd に登録します。デプロイ資格情報やサービス秘密は送信されません。",
            ["body.update"] = "daemon 所有の記録を読み、置換内容をプレビューしてから runtime revision fence で確定します。",
            ["name.label"] = "表示名",
            ["endpoint.label"] = "Gewyvern endpoint",
            ["sidecar.label"] = "任意の sidecar endpoint",
            ["environment.label"] = "環境タグ",
            ["cluster.label"] = "クラスタタグ",
            ["role.label"] = "役割タグ",
            ["status.name"] = "Runtime 登録状態",
            ["status.initial.register"] = "既存サービスの座標を入力し、daemon authority に確認を依頼してください。",
            ["status.initial.update"] = "runtime revision {0} の authority 記録を読み込みました。編集後、fence 付き更新を確認してください。",
            ["status.loading"] = "authority 所有の登録記録を読み込んでいます...",
            ["status.reviewing"] = "副作用のない登録プランを要求しています...",
            ["status.plan_ready.register"] = "daemon が登録プランを受理しました。予測 authority revision は {0} です。",
            ["status.plan_ready.update"] = "daemon が revision fence 付き更新プランを受理しました。予測 authority revision は {0} です。",
            ["status.applying"] = "確認済み登録プランを適用しています...",
            ["status.applied.register"] = "Runtime {0} を authority revision {1} で登録しました。",
            ["status.applied.update"] = "Runtime {0} の登録を authority revision {1} で更新しました。",
            ["status.failed"] = "登録操作に失敗しました: {0}",
            ["plan.heading"] = "確認済み AUTHORITY プラン",
            ["plan.summary.name"] = "確認済み runtime 登録プラン",
            ["plan.empty"] = "確認済みプランはありません。",
            ["plan.kind"] = "操作: {0}",
            ["plan.kind.register"] = "既存 runtime の登録",
            ["plan.kind.update"] = "runtime 登録の更新",
            ["plan.identity"] = "ID: {0} / {1}",
            ["plan.endpoint"] = "Endpoint: {0}",
            ["plan.sidecar"] = "Sidecar: {0}",
            ["plan.tags"] = "タグ: 環境={0}、クラスタ={1}、役割={2}",
            ["plan.revision.register"] = "予測 authority revision: {0}",
            ["plan.revision.update"] = "Fence: runtime revision {0}、予測 authority revision: {1}",
            ["confirmation"] = "この daemon プランの内容を確認し、登録変更を許可します",
            ["optional.none"] = "なし",
            ["error.confirm"] = "適用前にプランを確認し、明示的に許可してください。",
            ["action.review"] = "プランを確認",
            ["action.apply.register"] = "Runtime を登録",
            ["action.apply.update"] = "登録情報を更新",
            ["action.edit"] = "編集を続ける",
            ["action.close"] = "閉じる",
        }));

    public static IReadOnlyDictionary<string, string> Spanish { get; } =
        Catalog(Localized(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "Registrar runtime existente",
            ["entry.help"] = "Conecta un servicio gewyvern activo a esta autoridad daemon sin desplegarlo.",
            ["workspace.edit"] = "Editar registro",
            ["workspace.edit.help"] = "Revisa y actualiza los metadatos de registro propiedad de la autoridad.",
            ["title.register"] = "Leserpent / Registrar runtime",
            ["title.update"] = "Leserpent / Actualizar registro",
            ["kicker"] = "AUTORIDAD DEL RUNTIME",
            ["heading.register"] = "Conectar un runtime existente",
            ["heading.update"] = "Revisar metadatos de registro",
            ["body.register"] = "Registra un endpoint gewyvern activo en este leserpentd. No se transmiten credenciales de despliegue ni secretos del servicio.",
            ["body.update"] = "Lee el registro de la autoridad, previsualiza el reemplazo exacto y confirma con una barrera de revision del runtime.",
            ["name.label"] = "Nombre visible",
            ["endpoint.label"] = "Endpoint de gewyvern",
            ["sidecar.label"] = "Endpoint sidecar opcional",
            ["environment.label"] = "Etiqueta de entorno",
            ["cluster.label"] = "Etiqueta de clúster",
            ["role.label"] = "Etiqueta de rol",
            ["status.name"] = "Estado del registro del runtime",
            ["status.initial.register"] = "Introduce las coordenadas del servicio y solicita su revisión a la autoridad daemon.",
            ["status.initial.update"] = "Registro de autoridad cargado en la revision {0}. Edita y revisa una actualización protegida.",
            ["status.loading"] = "Cargando el registro propiedad de la autoridad...",
            ["status.reviewing"] = "Solicitando un plan de registro sin efectos secundarios...",
            ["status.plan_ready.register"] = "El daemon aceptó el plan; la revision de autoridad prevista es {0}. Revísalo antes de confirmar.",
            ["status.plan_ready.update"] = "El daemon aceptó el plan protegido; la revision de autoridad prevista es {0}.",
            ["status.applying"] = "Aplicando el plan de registro revisado...",
            ["status.applied.register"] = "El runtime {0} se registró en la revision de autoridad {1}.",
            ["status.applied.update"] = "El registro del runtime {0} se actualizó en la revision de autoridad {1}.",
            ["status.failed"] = "La operación de registro falló: {0}",
            ["plan.heading"] = "PLAN DE AUTORIDAD REVISADO",
            ["plan.summary.name"] = "Plan de registro del runtime revisado",
            ["plan.empty"] = "No hay ningún plan revisado activo.",
            ["plan.kind"] = "Operación: {0}",
            ["plan.kind.register"] = "registrar runtime existente",
            ["plan.kind.update"] = "actualizar registro del runtime",
            ["plan.identity"] = "Identidad: {0} / {1}",
            ["plan.endpoint"] = "Endpoint: {0}",
            ["plan.sidecar"] = "Sidecar: {0}",
            ["plan.tags"] = "Etiquetas: entorno={0}, clúster={1}, rol={2}",
            ["plan.revision.register"] = "Revision de autoridad prevista: {0}",
            ["plan.revision.update"] = "Barrera: revision del runtime {0}; revision de autoridad prevista: {1}",
            ["confirmation"] = "He revisado este plan exacto del daemon y autorizo el cambio de registro",
            ["optional.none"] = "ninguno",
            ["error.confirm"] = "Revisa el plan y confírmalo explícitamente antes de aplicarlo.",
            ["action.review"] = "Revisar plan",
            ["action.apply.register"] = "Registrar runtime",
            ["action.apply.update"] = "Actualizar registro",
            ["action.edit"] = "Seguir editando",
            ["action.close"] = "Cerrar",
        }));

    public static IReadOnlyDictionary<string, string> German { get; } =
        Catalog(Localized(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "Vorhandene Runtime registrieren",
            ["entry.help"] = "Bindet einen laufenden Gewyvern-Dienst ohne Bereitstellung an diese Daemon-Autorität.",
            ["workspace.edit"] = "Registrierung bearbeiten",
            ["workspace.edit.help"] = "Prüft und aktualisiert die autoritätseigenen Registrierungsmetadaten.",
            ["title.register"] = "Leserpent / Runtime registrieren",
            ["title.update"] = "Leserpent / Registrierung aktualisieren",
            ["kicker"] = "RUNTIME-AUTORITÄT",
            ["heading.register"] = "Vorhandene Runtime anbinden",
            ["heading.update"] = "Registrierungsmetadaten prüfen",
            ["body.register"] = "Registriert einen laufenden Gewyvern-Endpunkt bei diesem leserpentd. Bereitstellungsdaten und Dienstgeheimnisse werden nicht übertragen.",
            ["body.update"] = "Liest den Daemon-Datensatz, zeigt den exakten Ersatz und bestätigt ihn über eine Runtime-Revisionssperre.",
            ["name.label"] = "Anzeigename",
            ["endpoint.label"] = "Gewyvern-Endpunkt",
            ["sidecar.label"] = "Optionaler Sidecar-Endpunkt",
            ["environment.label"] = "Umgebungs-Tag",
            ["cluster.label"] = "Cluster-Tag",
            ["role.label"] = "Rollen-Tag",
            ["status.name"] = "Status der Runtime-Registrierung",
            ["status.initial.register"] = "Dienstkoordinaten eingeben und von der Daemon-Autorität prüfen lassen.",
            ["status.initial.update"] = "Autoritätsdatensatz bei Runtime-Revision {0} geladen. Metadaten bearbeiten und gesperrte Aktualisierung prüfen.",
            ["status.loading"] = "Autoritätseigenen Registrierungsdatensatz laden...",
            ["status.reviewing"] = "Nebenwirkungsfreien Registrierungsplan anfordern...",
            ["status.plan_ready.register"] = "Der Daemon hat den Plan mit erwarteter Autoritätsrevision {0} akzeptiert. Vor Bestätigung prüfen.",
            ["status.plan_ready.update"] = "Der Daemon hat den revisionsgesperrten Plan mit erwarteter Autoritätsrevision {0} akzeptiert.",
            ["status.applying"] = "Geprüften Registrierungsplan anwenden...",
            ["status.applied.register"] = "Runtime {0} wurde bei Autoritätsrevision {1} registriert.",
            ["status.applied.update"] = "Registrierung von Runtime {0} wurde bei Autoritätsrevision {1} aktualisiert.",
            ["status.failed"] = "Registrierungsvorgang fehlgeschlagen: {0}",
            ["plan.heading"] = "GEPRÜFTER AUTORITÄTSPLAN",
            ["plan.summary.name"] = "Geprüfter Runtime-Registrierungsplan",
            ["plan.empty"] = "Kein geprüfter Plan ist aktiv.",
            ["plan.kind"] = "Vorgang: {0}",
            ["plan.kind.register"] = "vorhandene Runtime registrieren",
            ["plan.kind.update"] = "Runtime-Registrierung aktualisieren",
            ["plan.identity"] = "Identität: {0} / {1}",
            ["plan.endpoint"] = "Endpunkt: {0}",
            ["plan.sidecar"] = "Sidecar: {0}",
            ["plan.tags"] = "Tags: Umgebung={0}, Cluster={1}, Rolle={2}",
            ["plan.revision.register"] = "Erwartete Autoritätsrevision: {0}",
            ["plan.revision.update"] = "Sperre: Runtime-Revision {0}; erwartete Autoritätsrevision: {1}",
            ["confirmation"] = "Ich habe diesen exakten Daemon-Plan geprüft und autorisiere die Registrierungsänderung",
            ["optional.none"] = "keine",
            ["error.confirm"] = "Plan vor dem Anwenden prüfen und ausdrücklich bestätigen.",
            ["action.review"] = "Plan prüfen",
            ["action.apply.register"] = "Runtime registrieren",
            ["action.apply.update"] = "Registrierung aktualisieren",
            ["action.edit"] = "Weiter bearbeiten",
            ["action.close"] = "Schließen",
        }));

    public static IReadOnlyDictionary<string, string> French { get; } =
        Catalog(Localized(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "Enregistrer un runtime existant",
            ["entry.help"] = "Rattache un service gewyvern actif à cette autorité daemon sans le déployer.",
            ["workspace.edit"] = "Modifier l’enregistrement",
            ["workspace.edit.help"] = "Vérifie et met à jour les métadonnées d’enregistrement détenues par l’autorité.",
            ["title.register"] = "Leserpent / Enregistrer un runtime",
            ["title.update"] = "Leserpent / Mettre à jour l’enregistrement",
            ["kicker"] = "AUTORITÉ DU RUNTIME",
            ["heading.register"] = "Rattacher un runtime existant",
            ["heading.update"] = "Vérifier les métadonnées d’enregistrement",
            ["body.register"] = "Enregistre un endpoint gewyvern actif auprès de ce leserpentd. Aucun justificatif de déploiement ni secret de service n’est transmis.",
            ["body.update"] = "Lit l’enregistrement du daemon, prévisualise son remplacement exact, puis valide avec une barrière de révision du runtime.",
            ["name.label"] = "Nom affiché",
            ["endpoint.label"] = "Endpoint gewyvern",
            ["sidecar.label"] = "Endpoint sidecar facultatif",
            ["environment.label"] = "Étiquette d’environnement",
            ["cluster.label"] = "Étiquette de cluster",
            ["role.label"] = "Étiquette de rôle",
            ["status.name"] = "État de l’enregistrement du runtime",
            ["status.initial.register"] = "Saisissez les coordonnées du service puis demandez leur vérification à l’autorité daemon.",
            ["status.initial.update"] = "Enregistrement d’autorité chargé à la révision {0}. Modifiez puis vérifiez la mise à jour protégée.",
            ["status.loading"] = "Chargement de l’enregistrement détenu par l’autorité...",
            ["status.reviewing"] = "Demande d’un plan d’enregistrement sans effet de bord...",
            ["status.plan_ready.register"] = "Le daemon a accepté le plan prévoyant la révision d’autorité {0}. Vérifiez-le avant confirmation.",
            ["status.plan_ready.update"] = "Le daemon a accepté le plan protégé prévoyant la révision d’autorité {0}.",
            ["status.applying"] = "Application du plan d’enregistrement vérifié...",
            ["status.applied.register"] = "Le runtime {0} a été enregistré à la révision d’autorité {1}.",
            ["status.applied.update"] = "L’enregistrement du runtime {0} a été mis à jour à la révision d’autorité {1}.",
            ["status.failed"] = "Échec de l’opération d’enregistrement : {0}",
            ["plan.heading"] = "PLAN D’AUTORITÉ VÉRIFIÉ",
            ["plan.summary.name"] = "Plan d’enregistrement du runtime vérifié",
            ["plan.empty"] = "Aucun plan vérifié n’est actif.",
            ["plan.kind"] = "Opération : {0}",
            ["plan.kind.register"] = "enregistrer un runtime existant",
            ["plan.kind.update"] = "mettre à jour l’enregistrement du runtime",
            ["plan.identity"] = "Identité : {0} / {1}",
            ["plan.endpoint"] = "Endpoint : {0}",
            ["plan.sidecar"] = "Sidecar : {0}",
            ["plan.tags"] = "Étiquettes : environnement={0}, cluster={1}, rôle={2}",
            ["plan.revision.register"] = "Révision d’autorité prévue : {0}",
            ["plan.revision.update"] = "Barrière : révision du runtime {0} ; révision d’autorité prévue : {1}",
            ["confirmation"] = "J’ai vérifié ce plan daemon exact et j’autorise la modification de l’enregistrement",
            ["optional.none"] = "aucun",
            ["error.confirm"] = "Vérifiez le plan et confirmez-le explicitement avant application.",
            ["action.review"] = "Vérifier le plan",
            ["action.apply.register"] = "Enregistrer le runtime",
            ["action.apply.update"] = "Mettre à jour l’enregistrement",
            ["action.edit"] = "Continuer la modification",
            ["action.close"] = "Fermer",
        }));

    public static IReadOnlyDictionary<string, string> Korean { get; } =
        Catalog(Localized(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["entry.open"] = "기존 runtime 등록",
            ["entry.help"] = "실행 중인 gewyvern 서비스를 배포 없이 이 daemon authority에 연결합니다.",
            ["workspace.edit"] = "등록 정보 편집",
            ["workspace.edit.help"] = "authority가 소유한 runtime 등록 메타데이터를 검토하고 갱신합니다.",
            ["title.register"] = "Leserpent / runtime 등록",
            ["title.update"] = "Leserpent / 등록 정보 갱신",
            ["kicker"] = "RUNTIME AUTHORITY",
            ["heading.register"] = "기존 runtime 연결",
            ["heading.update"] = "등록 메타데이터 검토",
            ["body.register"] = "실행 중인 gewyvern endpoint를 이 leserpentd에 등록합니다. 배포 자격 증명이나 서비스 비밀은 전송하지 않습니다.",
            ["body.update"] = "daemon 소유 레코드를 읽고 정확한 교체 내용을 미리 본 뒤 runtime revision fence로 확정합니다.",
            ["name.label"] = "표시 이름",
            ["endpoint.label"] = "Gewyvern endpoint",
            ["sidecar.label"] = "선택적 sidecar endpoint",
            ["environment.label"] = "환경 태그",
            ["cluster.label"] = "클러스터 태그",
            ["role.label"] = "역할 태그",
            ["status.name"] = "Runtime 등록 상태",
            ["status.initial.register"] = "기존 서비스 좌표를 입력한 뒤 daemon authority에 검토를 요청하세요.",
            ["status.initial.update"] = "runtime revision {0}의 authority 레코드를 불러왔습니다. 편집 후 fence가 적용된 갱신을 검토하세요.",
            ["status.loading"] = "authority 소유 등록 레코드를 불러오는 중...",
            ["status.reviewing"] = "부작용 없는 등록 계획을 요청하는 중...",
            ["status.plan_ready.register"] = "daemon이 등록 계획을 승인했습니다. 예상 authority revision은 {0}입니다.",
            ["status.plan_ready.update"] = "daemon이 revision fence 갱신 계획을 승인했습니다. 예상 authority revision은 {0}입니다.",
            ["status.applying"] = "검토한 등록 계획을 적용하는 중...",
            ["status.applied.register"] = "Runtime {0}이 authority revision {1}에 등록되었습니다.",
            ["status.applied.update"] = "Runtime {0} 등록이 authority revision {1}에 갱신되었습니다.",
            ["status.failed"] = "등록 작업 실패: {0}",
            ["plan.heading"] = "검토된 AUTHORITY 계획",
            ["plan.summary.name"] = "검토된 runtime 등록 계획",
            ["plan.empty"] = "활성화된 검토 계획이 없습니다.",
            ["plan.kind"] = "작업: {0}",
            ["plan.kind.register"] = "기존 runtime 등록",
            ["plan.kind.update"] = "runtime 등록 갱신",
            ["plan.identity"] = "ID: {0} / {1}",
            ["plan.endpoint"] = "Endpoint: {0}",
            ["plan.sidecar"] = "Sidecar: {0}",
            ["plan.tags"] = "태그: 환경={0}, 클러스터={1}, 역할={2}",
            ["plan.revision.register"] = "예상 authority revision: {0}",
            ["plan.revision.update"] = "Fence: runtime revision {0}; 예상 authority revision: {1}",
            ["confirmation"] = "이 daemon 계획의 정확한 내용을 검토했으며 등록 변경을 승인합니다",
            ["optional.none"] = "없음",
            ["error.confirm"] = "적용 전에 계획을 검토하고 명시적으로 확인하세요.",
            ["action.review"] = "계획 검토",
            ["action.apply.register"] = "Runtime 등록",
            ["action.apply.update"] = "등록 정보 갱신",
            ["action.edit"] = "계속 편집",
            ["action.close"] = "닫기",
        }));

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
            [FullKey("status.initial.update")] = 1,
            [FullKey("status.plan_ready.register")] = 1,
            [FullKey("status.plan_ready.update")] = 1,
            [FullKey("status.applied.register")] = 2,
            [FullKey("status.applied.update")] = 2,
            [FullKey("status.failed")] = 1,
            [FullKey("plan.kind")] = 1,
            [FullKey("plan.identity")] = 2,
            [FullKey("plan.endpoint")] = 1,
            [FullKey("plan.sidecar")] = 1,
            [FullKey("plan.tags")] = 3,
            [FullKey("plan.revision.register")] = 1,
            [FullKey("plan.revision.update")] = 2,
        };
        if (expected.Count != KeyCount)
        {
            throw new InvalidDataException(
                "desktop registration localization key contract drifted");
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
                    "desktop registration localization catalog is incomplete");
            }
            foreach (var entry in catalog)
            {
                VerifyFormat(entry.Value, formattedKeys.GetValueOrDefault(entry.Key));
            }
        }
    }

    private static IReadOnlyList<IReadOnlyDictionary<string, string>> All =>
        [English, SimplifiedChinese, TraditionalChinese, Japanese, Spanish, German, French, Korean];

    private static IReadOnlyDictionary<string, string> Localized(
        Dictionary<string, string> values)
    {
        var result = new Dictionary<string, string>(EnglishValues, StringComparer.Ordinal);
        foreach (var entry in values)
        {
            result[entry.Key] = entry.Value;
        }
        return result;
    }

    private static IReadOnlyDictionary<string, string> Catalog(
        IReadOnlyDictionary<string, string> values) => values.ToDictionary(
            entry => FullKey(entry.Key),
            entry => entry.Value,
            StringComparer.Ordinal);

    private static string FullKey(string key) => $"{Prefix}{key}";

    private static bool HasExpectedPlaceholders(string value, int arity)
    {
        for (var index = 0; index < 4; index++)
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
                    "desktop registration localization produced invalid text");
            }
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                "desktop registration localization format is invalid",
                error);
        }
    }
}
