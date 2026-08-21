using System.Globalization;

internal static class DesktopRetirementCatalogs
{
    private const string Prefix = "desktop.retirement.";
    public const int KeyCount = 45;

    public static IReadOnlyDictionary<string, string> English { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Retire gewyvern",
            ["confirmation"] = "I confirm service removal and runtime unregistration on this host",
            ["submit"] = "Retire gewyvern",
            ["refresh"] = "Refresh same attempt",
            ["close"] = "Close",
            ["status.initial"] = "Choose the daemon authority that owns this registered runtime.",
            ["status.name"] = "Gewyvern retirement status",
            ["phase.name"] = "Gewyvern retirement phase",
            ["kicker"] = "RUNTIME RETIREMENT",
            ["heading"] = "Stop, remove, then unregister",
            ["body"] = "The selected leserpentd must prove that the bound gewyvern service was retired before it atomically unregisters the runtime. Failure preserves the registration for recovery.",
            ["authority.label"] = "Owning daemon authority",
            ["retirement_id.label"] = "Retirement ID",
            ["provisioning_id.label"] = "Original provisioning ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "Target host",
            ["port.label"] = "SSH port",
            ["credential.label"] = "SSH credential handle",
            ["a11y.authority"] = "Daemon authority owning the gewyvern runtime",
            ["a11y.retirement_id"] = "Stable gewyvern retirement operation ID",
            ["a11y.provisioning_id"] = "Original provisioning authority ID",
            ["a11y.runtime_id"] = "Registered runtime ID to retire",
            ["a11y.host"] = "Target host for gewyvern retirement",
            ["a11y.port"] = "Target SSH port",
            ["a11y.credential"] = "Opaque SSH credential handle",
            ["a11y.confirm"] = "Confirm service removal and unregistration",
            ["a11y.submit"] = "Retire and unregister gewyvern runtime",
            ["a11y.refresh"] = "Refresh the same retirement attempt",
            ["a11y.close"] = "Close gewyvern retirement window",
            ["phase.not_submitted"] = "NOT SUBMITTED",
            ["phase.planned"] = "PLANNED",
            ["phase.retiring_service"] = "RETIRING SERVICE",
            ["phase.service_retired"] = "SERVICE RETIRED",
            ["phase.runtime_unregistered"] = "RUNTIME UNREGISTERED",
            ["phase.failed"] = "FAILED",
            ["error.confirm_required"] = "Confirm gewyvern retirement before submitting.",
            ["error.authority_required"] = "Select an owning daemon authority first.",
            ["status.observation_limit"] = "Automatic observation reached its bounded limit. Use Refresh same attempt to inspect this exact retirement ID without creating another removal.",
            ["status.waiting"] = "Waiting for the selected daemon authority...",
            ["status.planned"] = "Retirement is durably queued. Observation reuses this exact identity and does not submit a second removal.",
            ["status.retiring_service"] = "The daemon authority is stopping and removing the bound gewyvern service.",
            ["status.service_retired"] = "The service retirement proof is committed; atomic runtime unregistration is pending.",
            ["status.runtime_unregistered"] = "Runtime {0} was safely unregistered after service retirement.",
            ["status.failed"] = "Retirement failed with bounded fault {0}. The runtime remains registered for inspection and recovery; use a new retirement ID for a corrected attempt.",
            ["unavailable"] = "unavailable",
        });

    public static IReadOnlyDictionary<string, string> SimplifiedChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 退役 gewyvern",
            ["confirmation"] = "我确认在此主机上移除服务并注销 runtime",
            ["submit"] = "退役 gewyvern",
            ["refresh"] = "刷新同一尝试",
            ["close"] = "关闭",
            ["status.initial"] = "请选择拥有此已注册 runtime 的 daemon 权威端。",
            ["status.name"] = "Gewyvern 退役状态",
            ["phase.name"] = "Gewyvern 退役阶段",
            ["kicker"] = "RUNTIME 退役",
            ["heading"] = "停止、移除并注销",
            ["body"] = "所选 leserpentd 必须先证明绑定的 gewyvern 服务已退役，随后才会原子注销 runtime。失败时会保留注册以便恢复。",
            ["authority.label"] = "所属 daemon 权威端",
            ["retirement_id.label"] = "退役 ID",
            ["provisioning_id.label"] = "原部署 ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "目标主机",
            ["port.label"] = "SSH 端口",
            ["credential.label"] = "SSH 凭证句柄",
            ["a11y.authority"] = "拥有此 gewyvern runtime 的 daemon 权威端",
            ["a11y.retirement_id"] = "稳定的 gewyvern 退役操作 ID",
            ["a11y.provisioning_id"] = "原部署权威 ID",
            ["a11y.runtime_id"] = "要退役的已注册 runtime ID",
            ["a11y.host"] = "gewyvern 的退役目标主机",
            ["a11y.port"] = "目标 SSH 端口",
            ["a11y.credential"] = "不透明 SSH 凭证句柄",
            ["a11y.confirm"] = "确认移除服务并注销 runtime",
            ["a11y.submit"] = "退役并注销 gewyvern runtime",
            ["a11y.refresh"] = "刷新同一次退役尝试",
            ["a11y.close"] = "关闭 gewyvern 退役窗口",
            ["phase.not_submitted"] = "尚未提交",
            ["phase.planned"] = "已规划",
            ["phase.retiring_service"] = "正在退役服务",
            ["phase.service_retired"] = "服务已退役",
            ["phase.runtime_unregistered"] = "RUNTIME 已注销",
            ["phase.failed"] = "失败",
            ["error.confirm_required"] = "提交前请确认退役 gewyvern。",
            ["error.authority_required"] = "请先选择所属 daemon 权威端。",
            ["status.observation_limit"] = "自动观察已达到受限上限。请使用“刷新同一尝试”检查这个确切的退役 ID，不会创建另一次移除。",
            ["status.waiting"] = "正在等待所选 daemon 权威端...",
            ["status.planned"] = "退役已持久排队。观察会复用这个确切身份，不会提交第二次移除。",
            ["status.retiring_service"] = "daemon 权威端正在停止并移除绑定的 gewyvern 服务。",
            ["status.service_retired"] = "服务退役证明已提交；正在等待原子注销 runtime。",
            ["status.runtime_unregistered"] = "runtime {0} 已在服务退役后安全注销。",
            ["status.failed"] = "退役失败，受限故障代码为 {0}。runtime 仍保持注册，以供检查和恢复；请使用新的退役 ID 发起修正后的尝试。",
            ["unavailable"] = "不可用",
        });

    public static IReadOnlyDictionary<string, string> TraditionalChinese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / 退役 gewyvern",
            ["confirmation"] = "我確認在此主機上移除服務並取消註冊 runtime",
            ["submit"] = "退役 gewyvern",
            ["refresh"] = "重新整理同一次嘗試",
            ["close"] = "關閉",
            ["status.initial"] = "請選擇擁有此已註冊 runtime 的 daemon 權威端。",
            ["status.name"] = "Gewyvern 退役狀態",
            ["phase.name"] = "Gewyvern 退役階段",
            ["kicker"] = "RUNTIME 退役",
            ["heading"] = "停止、移除，再取消註冊",
            ["body"] = "所選 leserpentd 必須先證明綁定的 gewyvern 服務已退役，隨後才會以原子方式取消註冊 runtime。失敗時會保留註冊以便復原。",
            ["authority.label"] = "所屬 daemon 權威端",
            ["retirement_id.label"] = "退役 ID",
            ["provisioning_id.label"] = "原佈建 ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "目標主機",
            ["port.label"] = "SSH 連接埠",
            ["credential.label"] = "SSH 憑證控制代碼",
            ["a11y.authority"] = "擁有此 gewyvern runtime 的 daemon 權威端",
            ["a11y.retirement_id"] = "穩定的 gewyvern 退役操作 ID",
            ["a11y.provisioning_id"] = "原佈建權威 ID",
            ["a11y.runtime_id"] = "要退役的已註冊 runtime ID",
            ["a11y.host"] = "gewyvern 的退役目標主機",
            ["a11y.port"] = "目標 SSH 連接埠",
            ["a11y.credential"] = "不透明 SSH 憑證控制代碼",
            ["a11y.confirm"] = "確認移除服務並取消註冊 runtime",
            ["a11y.submit"] = "退役並取消註冊 gewyvern runtime",
            ["a11y.refresh"] = "重新整理同一次退役嘗試",
            ["a11y.close"] = "關閉 gewyvern 退役視窗",
            ["phase.not_submitted"] = "尚未提交",
            ["phase.planned"] = "已規劃",
            ["phase.retiring_service"] = "正在退役服務",
            ["phase.service_retired"] = "服務已退役",
            ["phase.runtime_unregistered"] = "RUNTIME 已取消註冊",
            ["phase.failed"] = "失敗",
            ["error.confirm_required"] = "提交前請確認退役 gewyvern。",
            ["error.authority_required"] = "請先選擇所屬 daemon 權威端。",
            ["status.observation_limit"] = "自動觀察已達受限上限。請使用「重新整理同一次嘗試」檢查這個確切的退役 ID，不會建立另一次移除。",
            ["status.waiting"] = "正在等待所選 daemon 權威端...",
            ["status.planned"] = "退役已持久排入佇列。觀察會重複使用這個確切身分，不會提交第二次移除。",
            ["status.retiring_service"] = "daemon 權威端正在停止並移除綁定的 gewyvern 服務。",
            ["status.service_retired"] = "服務退役證明已提交；正在等待以原子方式取消註冊 runtime。",
            ["status.runtime_unregistered"] = "runtime {0} 已在服務退役後安全取消註冊。",
            ["status.failed"] = "退役失敗，受限故障代碼為 {0}。runtime 仍保持註冊，以供檢查與復原；請使用新的退役 ID 發起修正後的嘗試。",
            ["unavailable"] = "無法使用",
        });

    public static IReadOnlyDictionary<string, string> Japanese { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / gewyvern を廃止",
            ["confirmation"] = "このホストでのサービス削除と runtime 登録解除を確認しました",
            ["submit"] = "gewyvern を廃止",
            ["refresh"] = "同じ試行を更新",
            ["close"] = "閉じる",
            ["status.initial"] = "この登録済み runtime を所有する daemon authority を選択してください。",
            ["status.name"] = "Gewyvern 廃止状態",
            ["phase.name"] = "Gewyvern 廃止段階",
            ["kicker"] = "RUNTIME 廃止",
            ["heading"] = "停止、削除、登録解除",
            ["body"] = "選択した leserpentd は、runtime をアトミックに登録解除する前に、紐付いた gewyvern サービスの廃止を証明する必要があります。失敗時は復旧のため登録が保持されます。",
            ["authority.label"] = "所有 daemon authority",
            ["retirement_id.label"] = "廃止 ID",
            ["provisioning_id.label"] = "元のプロビジョニング ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "対象ホスト",
            ["port.label"] = "SSH ポート",
            ["credential.label"] = "SSH 認証情報ハンドル",
            ["a11y.authority"] = "gewyvern runtime を所有する daemon authority",
            ["a11y.retirement_id"] = "安定した gewyvern 廃止操作 ID",
            ["a11y.provisioning_id"] = "元のプロビジョニング authority ID",
            ["a11y.runtime_id"] = "廃止する登録済み runtime ID",
            ["a11y.host"] = "gewyvern の廃止対象ホスト",
            ["a11y.port"] = "対象 SSH ポート",
            ["a11y.credential"] = "不透明な SSH 認証情報ハンドル",
            ["a11y.confirm"] = "サービス削除と登録解除を確認",
            ["a11y.submit"] = "gewyvern runtime を廃止して登録解除",
            ["a11y.refresh"] = "同じ廃止試行を更新",
            ["a11y.close"] = "gewyvern 廃止ウィンドウを閉じる",
            ["phase.not_submitted"] = "未送信",
            ["phase.planned"] = "計画済み",
            ["phase.retiring_service"] = "サービス廃止中",
            ["phase.service_retired"] = "サービス廃止済み",
            ["phase.runtime_unregistered"] = "RUNTIME 登録解除済み",
            ["phase.failed"] = "失敗",
            ["error.confirm_required"] = "送信前に gewyvern の廃止を確認してください。",
            ["error.authority_required"] = "先に所有 daemon authority を選択してください。",
            ["status.observation_limit"] = "自動観察が上限に達しました。「同じ試行を更新」でこの廃止 ID を調べてください。別の削除は作成されません。",
            ["status.waiting"] = "選択した daemon authority を待機しています...",
            ["status.planned"] = "廃止は永続キューに登録されました。観察は同じ ID を再利用し、2 回目の削除を送信しません。",
            ["status.retiring_service"] = "daemon authority が紐付いた gewyvern サービスを停止して削除しています。",
            ["status.service_retired"] = "サービス廃止の証明が確定しました。runtime のアトミック登録解除を待機しています。",
            ["status.runtime_unregistered"] = "runtime {0} はサービス廃止後に安全に登録解除されました。",
            ["status.failed"] = "廃止は制限付き障害 {0} で失敗しました。runtime は検査と復旧のため登録されたままです。修正後は新しい廃止 ID で試行してください。",
            ["unavailable"] = "利用不可",
        });

    public static IReadOnlyDictionary<string, string> Spanish { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Retirar gewyvern",
            ["confirmation"] = "Confirmo la eliminación del servicio y la baja del runtime en este host",
            ["submit"] = "Retirar gewyvern",
            ["refresh"] = "Actualizar el mismo intento",
            ["close"] = "Cerrar",
            ["status.initial"] = "Elige la autoridad daemon propietaria de este runtime registrado.",
            ["status.name"] = "Estado de retirada de Gewyvern",
            ["phase.name"] = "Fase de retirada de Gewyvern",
            ["kicker"] = "RETIRADA DEL RUNTIME",
            ["heading"] = "Detener, eliminar y dar de baja",
            ["body"] = "El leserpentd seleccionado debe demostrar que el servicio gewyvern vinculado se retiró antes de dar de baja el runtime de forma atómica. Si falla, conserva el registro para recuperarlo.",
            ["authority.label"] = "Autoridad daemon propietaria",
            ["retirement_id.label"] = "ID de retirada",
            ["provisioning_id.label"] = "ID de aprovisionamiento original",
            ["runtime_id.label"] = "ID del runtime",
            ["host.label"] = "Host de destino",
            ["port.label"] = "Puerto SSH",
            ["credential.label"] = "Identificador de credencial SSH",
            ["a11y.authority"] = "Autoridad daemon propietaria del runtime gewyvern",
            ["a11y.retirement_id"] = "ID estable de la operación de retirada de gewyvern",
            ["a11y.provisioning_id"] = "ID de autoridad del aprovisionamiento original",
            ["a11y.runtime_id"] = "ID del runtime registrado que se retirará",
            ["a11y.host"] = "Host de destino para retirar gewyvern",
            ["a11y.port"] = "Puerto SSH de destino",
            ["a11y.credential"] = "Identificador opaco de credencial SSH",
            ["a11y.confirm"] = "Confirmar la eliminación del servicio y la baja",
            ["a11y.submit"] = "Retirar y dar de baja el runtime gewyvern",
            ["a11y.refresh"] = "Actualizar el mismo intento de retirada",
            ["a11y.close"] = "Cerrar la ventana de retirada de gewyvern",
            ["phase.not_submitted"] = "NO ENVIADO",
            ["phase.planned"] = "PLANIFICADO",
            ["phase.retiring_service"] = "RETIRANDO SERVICIO",
            ["phase.service_retired"] = "SERVICIO RETIRADO",
            ["phase.runtime_unregistered"] = "RUNTIME DADO DE BAJA",
            ["phase.failed"] = "FALLIDO",
            ["error.confirm_required"] = "Confirma la retirada de gewyvern antes de enviarla.",
            ["error.authority_required"] = "Selecciona primero una autoridad daemon propietaria.",
            ["status.observation_limit"] = "La observación automática alcanzó su límite. Usa Actualizar el mismo intento para inspeccionar este ID de retirada exacto sin crear otra eliminación.",
            ["status.waiting"] = "Esperando a la autoridad daemon seleccionada...",
            ["status.planned"] = "La retirada está en la cola duradera. La observación reutiliza esta identidad exacta y no envía una segunda eliminación.",
            ["status.retiring_service"] = "La autoridad daemon está deteniendo y eliminando el servicio gewyvern vinculado.",
            ["status.service_retired"] = "La prueba de retirada del servicio está confirmada; queda pendiente la baja atómica del runtime.",
            ["status.runtime_unregistered"] = "El runtime {0} se dio de baja de forma segura después de retirar el servicio.",
            ["status.failed"] = "La retirada falló con el error acotado {0}. El runtime sigue registrado para inspección y recuperación; usa un nuevo ID de retirada para un intento corregido.",
            ["unavailable"] = "no disponible",
        });

    public static IReadOnlyDictionary<string, string> German { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Gewyvern stilllegen",
            ["confirmation"] = "Ich bestätige das Entfernen des Dienstes und die Abmeldung der Runtime auf diesem Host",
            ["submit"] = "Gewyvern stilllegen",
            ["refresh"] = "Denselben Versuch aktualisieren",
            ["close"] = "Schließen",
            ["status.initial"] = "Wählen Sie die Daemon-Autorität aus, der diese registrierte Runtime gehört.",
            ["status.name"] = "Status der Gewyvern-Stilllegung",
            ["phase.name"] = "Phase der Gewyvern-Stilllegung",
            ["kicker"] = "RUNTIME-STILLLEGUNG",
            ["heading"] = "Stoppen, entfernen und abmelden",
            ["body"] = "Der ausgewählte leserpentd muss die Stilllegung des gebundenen Gewyvern-Dienstes nachweisen, bevor er die Runtime atomar abmeldet. Bei einem Fehler bleibt die Registrierung zur Wiederherstellung erhalten.",
            ["authority.label"] = "Zuständige Daemon-Autorität",
            ["retirement_id.label"] = "Stilllegungs-ID",
            ["provisioning_id.label"] = "Ursprüngliche Bereitstellungs-ID",
            ["runtime_id.label"] = "Runtime-ID",
            ["host.label"] = "Zielhost",
            ["port.label"] = "SSH-Port",
            ["credential.label"] = "SSH-Zugangsdaten-Handle",
            ["a11y.authority"] = "Daemon-Autorität der Gewyvern-Runtime",
            ["a11y.retirement_id"] = "Stabile Vorgangs-ID der Gewyvern-Stilllegung",
            ["a11y.provisioning_id"] = "Autoritäts-ID der ursprünglichen Bereitstellung",
            ["a11y.runtime_id"] = "Registrierte Runtime-ID für die Stilllegung",
            ["a11y.host"] = "Zielhost für die Gewyvern-Stilllegung",
            ["a11y.port"] = "Ziel-SSH-Port",
            ["a11y.credential"] = "Opakes SSH-Zugangsdaten-Handle",
            ["a11y.confirm"] = "Entfernen des Dienstes und Abmeldung bestätigen",
            ["a11y.submit"] = "Gewyvern-Runtime stilllegen und abmelden",
            ["a11y.refresh"] = "Denselben Stilllegungsversuch aktualisieren",
            ["a11y.close"] = "Fenster zur Gewyvern-Stilllegung schließen",
            ["phase.not_submitted"] = "NICHT GESENDET",
            ["phase.planned"] = "GEPLANT",
            ["phase.retiring_service"] = "DIENST WIRD STILLGELEGT",
            ["phase.service_retired"] = "DIENST STILLGELEGT",
            ["phase.runtime_unregistered"] = "RUNTIME ABGEMELDET",
            ["phase.failed"] = "FEHLGESCHLAGEN",
            ["error.confirm_required"] = "Bestätigen Sie die Gewyvern-Stilllegung vor dem Senden.",
            ["error.authority_required"] = "Wählen Sie zuerst eine zuständige Daemon-Autorität aus.",
            ["status.observation_limit"] = "Die automatische Beobachtung hat ihr Limit erreicht. Verwenden Sie Denselben Versuch aktualisieren, um diese Stilllegungs-ID zu prüfen, ohne eine weitere Entfernung zu erstellen.",
            ["status.waiting"] = "Warten auf die ausgewählte Daemon-Autorität...",
            ["status.planned"] = "Die Stilllegung ist dauerhaft eingereiht. Die Beobachtung verwendet exakt diese Identität und sendet keine zweite Entfernung.",
            ["status.retiring_service"] = "Die Daemon-Autorität stoppt und entfernt den gebundenen Gewyvern-Dienst.",
            ["status.service_retired"] = "Der Nachweis der Dienststilllegung ist gespeichert; die atomare Runtime-Abmeldung steht noch aus.",
            ["status.runtime_unregistered"] = "Runtime {0} wurde nach der Dienststilllegung sicher abgemeldet.",
            ["status.failed"] = "Die Stilllegung ist mit dem begrenzten Fehler {0} fehlgeschlagen. Die Runtime bleibt zur Prüfung und Wiederherstellung registriert; verwenden Sie für den korrigierten Versuch eine neue Stilllegungs-ID.",
            ["unavailable"] = "nicht verfügbar",
        });

    public static IReadOnlyDictionary<string, string> French { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / Retirer gewyvern",
            ["confirmation"] = "Je confirme la suppression du service et la désinscription du runtime sur cet hôte",
            ["submit"] = "Retirer gewyvern",
            ["refresh"] = "Actualiser la même tentative",
            ["close"] = "Fermer",
            ["status.initial"] = "Choisissez l’autorité daemon propriétaire de ce runtime enregistré.",
            ["status.name"] = "État du retrait de Gewyvern",
            ["phase.name"] = "Phase du retrait de Gewyvern",
            ["kicker"] = "RETRAIT DU RUNTIME",
            ["heading"] = "Arrêter, supprimer, puis désinscrire",
            ["body"] = "Le leserpentd sélectionné doit prouver que le service gewyvern associé a été retiré avant de désinscrire le runtime de manière atomique. En cas d’échec, l’enregistrement est conservé pour la récupération.",
            ["authority.label"] = "Autorité daemon propriétaire",
            ["retirement_id.label"] = "ID de retrait",
            ["provisioning_id.label"] = "ID de provisionnement d’origine",
            ["runtime_id.label"] = "ID du runtime",
            ["host.label"] = "Hôte cible",
            ["port.label"] = "Port SSH",
            ["credential.label"] = "Identifiant d’accès SSH",
            ["a11y.authority"] = "Autorité daemon propriétaire du runtime gewyvern",
            ["a11y.retirement_id"] = "ID stable de l’opération de retrait de gewyvern",
            ["a11y.provisioning_id"] = "ID d’autorité du provisionnement d’origine",
            ["a11y.runtime_id"] = "ID du runtime enregistré à retirer",
            ["a11y.host"] = "Hôte cible pour le retrait de gewyvern",
            ["a11y.port"] = "Port SSH cible",
            ["a11y.credential"] = "Identifiant opaque d’accès SSH",
            ["a11y.confirm"] = "Confirmer la suppression du service et la désinscription",
            ["a11y.submit"] = "Retirer et désinscrire le runtime gewyvern",
            ["a11y.refresh"] = "Actualiser la même tentative de retrait",
            ["a11y.close"] = "Fermer la fenêtre de retrait de gewyvern",
            ["phase.not_submitted"] = "NON ENVOYÉ",
            ["phase.planned"] = "PLANIFIÉ",
            ["phase.retiring_service"] = "RETRAIT DU SERVICE",
            ["phase.service_retired"] = "SERVICE RETIRÉ",
            ["phase.runtime_unregistered"] = "RUNTIME DÉSINSCRIT",
            ["phase.failed"] = "ÉCHEC",
            ["error.confirm_required"] = "Confirmez le retrait de gewyvern avant l’envoi.",
            ["error.authority_required"] = "Sélectionnez d’abord une autorité daemon propriétaire.",
            ["status.observation_limit"] = "L’observation automatique a atteint sa limite. Utilisez Actualiser la même tentative pour examiner cet ID de retrait exact sans créer une autre suppression.",
            ["status.waiting"] = "En attente de l’autorité daemon sélectionnée...",
            ["status.planned"] = "Le retrait est durablement mis en file. L’observation réutilise exactement cette identité et n’envoie pas une seconde suppression.",
            ["status.retiring_service"] = "L’autorité daemon arrête et supprime le service gewyvern associé.",
            ["status.service_retired"] = "La preuve du retrait du service est enregistrée ; la désinscription atomique du runtime est en attente.",
            ["status.runtime_unregistered"] = "Le runtime {0} a été désinscrit en toute sécurité après le retrait du service.",
            ["status.failed"] = "Le retrait a échoué avec l’erreur bornée {0}. Le runtime reste enregistré pour inspection et récupération ; utilisez un nouvel ID de retrait pour une tentative corrigée.",
            ["unavailable"] = "indisponible",
        });

    public static IReadOnlyDictionary<string, string> Korean { get; } =
        Catalog(new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["title"] = "Leserpent / gewyvern 폐기",
            ["confirmation"] = "이 호스트에서 서비스 제거 및 runtime 등록 해제를 확인합니다",
            ["submit"] = "gewyvern 폐기",
            ["refresh"] = "같은 시도 새로고침",
            ["close"] = "닫기",
            ["status.initial"] = "이 등록된 runtime을 소유하는 daemon 권한 주체를 선택하세요.",
            ["status.name"] = "Gewyvern 폐기 상태",
            ["phase.name"] = "Gewyvern 폐기 단계",
            ["kicker"] = "RUNTIME 폐기",
            ["heading"] = "중지, 제거 후 등록 해제",
            ["body"] = "선택한 leserpentd는 runtime을 원자적으로 등록 해제하기 전에 연결된 gewyvern 서비스가 폐기되었음을 증명해야 합니다. 실패하면 복구를 위해 등록이 유지됩니다.",
            ["authority.label"] = "소유 daemon 권한 주체",
            ["retirement_id.label"] = "폐기 ID",
            ["provisioning_id.label"] = "원래 프로비저닝 ID",
            ["runtime_id.label"] = "Runtime ID",
            ["host.label"] = "대상 호스트",
            ["port.label"] = "SSH 포트",
            ["credential.label"] = "SSH 자격 증명 핸들",
            ["a11y.authority"] = "gewyvern runtime을 소유하는 daemon 권한 주체",
            ["a11y.retirement_id"] = "안정적인 gewyvern 폐기 작업 ID",
            ["a11y.provisioning_id"] = "원래 프로비저닝 권한 ID",
            ["a11y.runtime_id"] = "폐기할 등록된 runtime ID",
            ["a11y.host"] = "gewyvern 폐기 대상 호스트",
            ["a11y.port"] = "대상 SSH 포트",
            ["a11y.credential"] = "불투명한 SSH 자격 증명 핸들",
            ["a11y.confirm"] = "서비스 제거 및 등록 해제 확인",
            ["a11y.submit"] = "gewyvern runtime 폐기 및 등록 해제",
            ["a11y.refresh"] = "같은 폐기 시도 새로고침",
            ["a11y.close"] = "gewyvern 폐기 창 닫기",
            ["phase.not_submitted"] = "제출되지 않음",
            ["phase.planned"] = "계획됨",
            ["phase.retiring_service"] = "서비스 폐기 중",
            ["phase.service_retired"] = "서비스 폐기됨",
            ["phase.runtime_unregistered"] = "RUNTIME 등록 해제됨",
            ["phase.failed"] = "실패",
            ["error.confirm_required"] = "제출하기 전에 gewyvern 폐기를 확인하세요.",
            ["error.authority_required"] = "먼저 소유 daemon 권한 주체를 선택하세요.",
            ["status.observation_limit"] = "자동 관찰이 제한에 도달했습니다. 다른 제거를 만들지 않고 이 폐기 ID를 확인하려면 ‘같은 시도 새로고침’을 사용하세요.",
            ["status.waiting"] = "선택한 daemon 권한 주체를 기다리는 중...",
            ["status.planned"] = "폐기 작업이 영구 큐에 등록되었습니다. 관찰은 이 ID를 재사용하며 두 번째 제거를 제출하지 않습니다.",
            ["status.retiring_service"] = "daemon 권한 주체가 연결된 gewyvern 서비스를 중지하고 제거하고 있습니다.",
            ["status.service_retired"] = "서비스 폐기 증명이 커밋되었습니다. runtime의 원자적 등록 해제를 기다리고 있습니다.",
            ["status.runtime_unregistered"] = "runtime {0}이(가) 서비스 폐기 후 안전하게 등록 해제되었습니다.",
            ["status.failed"] = "제한된 오류 {0}(으)로 폐기에 실패했습니다. 검사와 복구를 위해 runtime 등록은 유지됩니다. 수정된 시도에는 새 폐기 ID를 사용하세요.",
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
            [FullKey("status.runtime_unregistered")] = 1,
            [FullKey("status.failed")] = 1,
        };
        if (expected.Count != KeyCount)
        {
            throw new InvalidDataException(
                "desktop retirement localization key contract drifted");
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
                    "desktop retirement localization catalog is incomplete");
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
                    "desktop retirement localization produced invalid text");
            }
        }
        catch (FormatException error)
        {
            throw new InvalidDataException(
                "desktop retirement localization format is invalid",
                error);
        }
    }
}
