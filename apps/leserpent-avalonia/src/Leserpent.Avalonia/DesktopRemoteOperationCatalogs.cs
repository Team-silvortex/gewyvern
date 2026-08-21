using System.Globalization;

internal static class DesktopRemoteOperationCatalogs
{
    private const string Prefix = "desktop.remote_operation.";
    public const int KeyCount = 57;

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
        new("status.pending_unavailable", 0, "Pending workspaces were not opened because no authoritative daemon snapshot is available", "由于没有权威 daemon 快照，待处理工作区未打开", "由於沒有權威 daemon 快照，待處理工作區未開啟", "権威ある daemon スナップショットがないため、保留中のワークスペースは開かれませんでした", "Los espacios de trabajo pendientes no se abrieron porque no hay una instantánea autoritativa del daemon", "Ausstehende Arbeitsbereiche wurden nicht geöffnet, da kein autoritativer Daemon-Snapshot verfügbar ist", "Les espaces de travail en attente n’ont pas été ouverts faute d’instantané de daemon faisant autorité", "권한 있는 daemon 스냅샷이 없어 대기 중인 작업 공간을 열지 않았습니다"),
        new("status.workspace_removed", 1, "Workspace not opened: {0} is absent from the authoritative topology", "工作区未打开：权威拓扑中不存在 {0}", "工作區未開啟：權威拓撲中不存在 {0}", "ワークスペースを開けません: {0} は権威あるトポロジに存在しません", "No se abrió el espacio de trabajo: {0} no está en la topología autoritativa", "Arbeitsbereich nicht geöffnet: {0} fehlt in der autoritativen Topologie", "Espace de travail non ouvert : {0} est absent de la topologie faisant autorité", "작업 공간을 열지 않았습니다: 권한 있는 토폴로지에 {0}이(가) 없습니다"),
        new("status.workspace_waiting", 1, "Waiting for an authoritative daemon snapshot before opening {0}...", "正在等待权威 daemon 快照，之后再打开 {0}...", "正在等待權威 daemon 快照，之後再開啟 {0}...", "{0} を開く前に権威ある daemon スナップショットを待機しています...", "Esperando una instantánea autoritativa del daemon antes de abrir {0}...", "Vor dem Öffnen von {0} wird auf einen autoritativen Daemon-Snapshot gewartet...", "Attente d’un instantané de daemon faisant autorité avant l’ouverture de {0}...", "{0}을(를) 열기 전에 권한 있는 daemon 스냅샷을 기다리는 중..."),
        new("status.reconnect_starting", 0, "Restarting the remote event stream...", "正在重新启动远程事件流...", "正在重新啟動遠端事件串流...", "リモートイベントストリームを再起動しています...", "Reiniciando el flujo de eventos remoto...", "Remote-Ereignisstrom wird neu gestartet...", "Redémarrage du flux d’événements distant...", "원격 이벤트 스트림 다시 시작 중..."),
        new("status.reconnect_blocked", 1, "Reconnect blocked: {0}", "重连被阻止：{0}", "重新連線遭阻止：{0}", "再接続がブロックされました: {0}", "Reconexión bloqueada: {0}", "Wiederverbindung blockiert: {0}", "Reconnexion bloquée : {0}", "재연결 차단됨: {0}"),
        new("status.operation_blocked", 2, "{0} blocked: {1}", "{0} 被阻止：{1}", "{0} 遭阻止：{1}", "{0} がブロックされました: {1}", "{0} bloqueada: {1}", "{0} blockiert: {1}", "{0} bloquée : {1}", "{0} 차단됨: {1}"),
        new("status.confirmation_blocked", 2, "{0} blocked during confirmation: {1}", "确认期间 {0} 被阻止：{1}", "確認期間 {0} 遭阻止：{1}", "確認中に {0} がブロックされました: {1}", "{0} bloqueada durante la confirmación: {1}", "{0} während der Bestätigung blockiert: {1}", "{0} bloquée pendant la confirmation : {1}", "확인 중 {0} 차단됨: {1}"),
        new("status.operation_starting", 3, "{0} for {1} at revision {2}...", "正在对 {1} 执行{0}，修订 {2}...", "正在對 {1} 執行{0}，修訂 {2}...", "{1} に対して {0} を実行中、リビジョン {2}...", "{0} para {1} en la revisión {2}...", "{0} für {1} bei Revision {2}...", "{0} pour {1} à la révision {2}...", "{1}에 {0} 수행 중, 리비전 {2}..."),
        new("status.operation_accepted", 3, "{0} accepted for {1} at revision {2}", "已接受对 {1} 的{0}，修订 {2}", "已接受對 {1} 的{0}，修訂 {2}", "{1} に対する {0} を受理しました、リビジョン {2}", "{0} aceptada para {1} en la revisión {2}", "{0} für {1} bei Revision {2} angenommen", "{0} acceptée pour {1} à la révision {2}", "{1}에 대한 {0} 수락됨, 리비전 {2}"),
        new("status.operation_failed", 1, "Remote operation failed safely: {0}", "远程操作已安全失败：{0}", "遠端操作已安全失敗：{0}", "リモート操作は安全に失敗しました: {0}", "La operación remota falló de forma segura: {0}", "Remotevorgang ist sicher fehlgeschlagen: {0}", "L’opération distante a échoué de manière sûre : {0}", "원격 작업이 안전하게 실패했습니다: {0}"),
        new("reason.invalid_runtime_id", 0, "the runtime ID is invalid", "runtime ID 无效", "runtime ID 無效", "runtime ID が無効です", "el ID del runtime no es válido", "die Runtime-ID ist ungültig", "l’identifiant du runtime est invalide", "runtime ID가 잘못되었습니다"),
        new("reason.in_flight", 0, "another remote change is awaiting confirmation or completion", "另一个远程变更正在等待确认或完成", "另一個遠端變更正在等待確認或完成", "別のリモート変更が確認または完了を待っています", "otro cambio remoto está esperando confirmación o finalización", "eine andere Remoteänderung wartet auf Bestätigung oder Abschluss", "une autre modification distante attend une confirmation ou son achèvement", "다른 원격 변경이 확인 또는 완료를 기다리고 있습니다"),
        new("reason.revision_fence", 0, "a prior remote change is awaiting its event revision", "先前的远程变更正在等待对应事件修订", "先前的遠端變更正在等待對應事件修訂", "以前のリモート変更がイベントリビジョンを待っています", "un cambio remoto anterior está esperando su revisión de evento", "eine frühere Remoteänderung wartet auf ihre Ereignisrevision", "une modification distante antérieure attend sa révision d’événement", "이전 원격 변경이 이벤트 리비전을 기다리고 있습니다"),
        new("reason.observation_fence", 0, "an unknown remote outcome is awaiting an authoritative snapshot", "结果未知的远程操作正在等待权威快照", "結果未知的遠端操作正在等待權威快照", "結果不明のリモート操作が権威あるスナップショットを待っています", "un resultado remoto desconocido está esperando una instantánea autoritativa", "ein unbekanntes Remoteergebnis wartet auf einen autoritativen Snapshot", "un résultat distant inconnu attend un instantané faisant autorité", "결과를 알 수 없는 원격 작업이 권한 있는 스냅샷을 기다리고 있습니다"),
        new("reason.authoritative_snapshot", 0, "a generated authoritative snapshot is required", "需要已生成的权威快照", "需要已產生的權威快照", "生成済みの権威あるスナップショットが必要です", "se requiere una instantánea autoritativa generada", "ein erzeugter autoritativer Snapshot ist erforderlich", "un instantané généré faisant autorité est requis", "생성된 권한 있는 스냅샷이 필요합니다"),
        new("reason.runtime_unavailable", 0, "the runtime is absent from the authoritative snapshot", "权威快照中不存在该 runtime", "權威快照中不存在該 runtime", "権威あるスナップショットに runtime がありません", "el runtime no está en la instantánea autoritativa", "die Runtime fehlt im autoritativen Snapshot", "le runtime est absent de l’instantané faisant autorité", "권한 있는 스냅샷에 runtime이 없습니다"),
        new("reason.runtime_revision_changed", 0, "the runtime revision changed", "runtime 修订已变更", "runtime 修訂已變更", "runtime のリビジョンが変更されました", "la revisión del runtime cambió", "die Runtime-Revision hat sich geändert", "la révision du runtime a changé", "runtime 리비전이 변경되었습니다"),
        new("reason.authenticated_deployment", 0, "the runtime has not advertised authenticated deployment", "runtime 尚未声明支持已认证部署", "runtime 尚未宣告支援已驗證部署", "runtime は認証済みデプロイを通知していません", "el runtime no ha anunciado despliegue autenticado", "die Runtime hat keine authentifizierte Bereitstellung angekündigt", "le runtime n’a pas annoncé le déploiement authentifié", "runtime이 인증된 배포를 알리지 않았습니다"),
        new("reason.operation_inactive", 0, "the mutation operation is no longer active", "变更操作已不再活动", "變更操作已不再作用中", "変更操作はすでにアクティブではありません", "la operación de cambio ya no está activa", "der Änderungsvorgang ist nicht mehr aktiv", "l’opération de modification n’est plus active", "변경 작업이 더 이상 활성 상태가 아닙니다"),
        new("reason.source_closed", 0, "its workspace is already closed", "其工作区已经关闭", "其工作區已經關閉", "そのワークスペースはすでに閉じられています", "su espacio de trabajo ya está cerrado", "der zugehörige Arbeitsbereich ist bereits geschlossen", "son espace de travail est déjà fermé", "해당 작업 공간이 이미 닫혔습니다"),
        new("reason.unsupported_action", 0, "the typed action is not supported", "不支持该类型化动作", "不支援該型別化動作", "この型付きアクションはサポートされていません", "la acción tipada no es compatible", "die typisierte Aktion wird nicht unterstützt", "l’action typée n’est pas prise en charge", "형식화된 작업이 지원되지 않습니다"),
        new("reason.missing_form", 0, "the typed action has no form", "类型化动作没有表单", "型別化動作沒有表單", "型付きアクションにフォームがありません", "la acción tipada no tiene formulario", "die typisierte Aktion hat kein Formular", "l’action typée ne contient aucun formulaire", "형식화된 작업에 폼이 없습니다"),
        new("reason.not_live", 0, "the remote state is not live", "远程状态不是实时状态", "遠端狀態不是即時狀態", "リモート状態がライブではありません", "el estado remoto no está en directo", "der Remotezustand ist nicht live", "l’état distant n’est pas en direct", "원격 상태가 실시간이 아닙니다"),
        new("reason.workspace_capacity", 1, "close one of the {0} open or pending workspaces first", "请先关闭 {0} 个已打开或待处理工作区中的一个", "請先關閉 {0} 個已開啟或待處理工作區中的一個", "開いている、または保留中の {0} 個のワークスペースのいずれかを先に閉じてください", "cierra primero uno de los {0} espacios de trabajo abiertos o pendientes", "schließen Sie zuerst einen der {0} offenen oder ausstehenden Arbeitsbereiche", "fermez d’abord l’un des {0} espaces de travail ouverts ou en attente", "열려 있거나 대기 중인 작업 공간 {0}개 중 하나를 먼저 닫으세요"),
        new("reason.workspace_removed", 0, "the runtime is no longer present in the authoritative topology", "权威拓扑中已不存在该 runtime", "權威拓撲中已不存在該 runtime", "権威あるトポロジに runtime が存在しなくなりました", "el runtime ya no está en la topología autoritativa", "die Runtime ist nicht mehr in der autoritativen Topologie vorhanden", "le runtime n’est plus présent dans la topologie faisant autorité", "권한 있는 토폴로지에 runtime이 더 이상 없습니다"),
        new("reason.workspace_unavailable", 0, "no authoritative daemon snapshot is available", "没有可用的权威 daemon 快照", "沒有可用的權威 daemon 快照", "権威ある daemon スナップショットがありません", "no hay una instantánea autoritativa del daemon disponible", "kein autoritativer Daemon-Snapshot ist verfügbar", "aucun instantané de daemon faisant autorité n’est disponible", "사용 가능한 권한 있는 daemon 스냅샷이 없습니다"),
        new("reason.workspace_incomplete", 0, "the workspace launch decision is incomplete", "工作区启动决策不完整", "工作區啟動決策不完整", "ワークスペース起動判断が不完全です", "la decisión de abrir el espacio de trabajo está incompleta", "die Entscheidung zum Öffnen des Arbeitsbereichs ist unvollständig", "la décision d’ouverture de l’espace de travail est incomplète", "작업 공간 실행 결정이 불완전합니다"),
        new("failure.remote_rejected", 3, "{0} rejected ({1}): {2}", "{0} 被拒绝（{1}）：{2}", "{0} 遭拒絕（{1}）：{2}", "{0} が拒否されました ({1}): {2}", "{0} rechazada ({1}): {2}", "{0} abgelehnt ({1}): {2}", "{0} rejetée ({1}) : {2}", "{0} 거부됨({1}): {2}"),
        new("failure.invalid_request", 2, "{0} blocked: {1}", "{0} 被阻止：{1}", "{0} 遭阻止：{1}", "{0} がブロックされました: {1}", "{0} bloqueada: {1}", "{0} blockiert: {1}", "{0} bloquée : {1}", "{0} 차단됨: {1}"),
        new("failure.invalid_response", 2, "{0} outcome unknown after an invalid response ({1}); wait for an authoritative snapshot before retrying", "{0} 收到无效响应后结果未知（{1}）；请等待权威快照后再重试", "{0} 收到無效回應後結果未知（{1}）；請等待權威快照後再重試", "無効な応答 ({1}) の後、{0} の結果は不明です。権威あるスナップショットを待ってから再試行してください", "El resultado de {0} es desconocido tras una respuesta no válida ({1}); espera una instantánea autoritativa antes de reintentar", "Das Ergebnis von {0} ist nach einer ungültigen Antwort ({1}) unbekannt; warten Sie vor einem erneuten Versuch auf einen autoritativen Snapshot", "Le résultat de {0} est inconnu après une réponse invalide ({1}) ; attendez un instantané faisant autorité avant de réessayer", "잘못된 응답({1}) 후 {0} 결과를 알 수 없습니다. 다시 시도하기 전에 권한 있는 스냅샷을 기다리세요"),
        new("failure.timeout", 1, "{0} outcome unknown after timeout; wait for an authoritative snapshot before retrying", "{0} 超时后结果未知；请等待权威快照后再重试", "{0} 逾時後結果未知；請等待權威快照後再重試", "タイムアウト後の {0} の結果は不明です。権威あるスナップショットを待ってから再試行してください", "El resultado de {0} es desconocido tras agotarse el tiempo; espera una instantánea autoritativa antes de reintentar", "Das Ergebnis von {0} ist nach einer Zeitüberschreitung unbekannt; warten Sie vor einem erneuten Versuch auf einen autoritativen Snapshot", "Le résultat de {0} est inconnu après expiration du délai ; attendez un instantané faisant autorité avant de réessayer", "시간 초과 후 {0} 결과를 알 수 없습니다. 다시 시도하기 전에 권한 있는 스냅샷을 기다리세요"),
        new("failure.transport", 1, "{0} outcome unknown after a network failure; wait for an authoritative snapshot before retrying", "{0} 发生网络故障后结果未知；请等待权威快照后再重试", "{0} 發生網路故障後結果未知；請等待權威快照後再重試", "ネットワーク障害後の {0} の結果は不明です。権威あるスナップショットを待ってから再試行してください", "El resultado de {0} es desconocido tras un fallo de red; espera una instantánea autoritativa antes de reintentar", "Das Ergebnis von {0} ist nach einem Netzwerkfehler unbekannt; warten Sie vor einem erneuten Versuch auf einen autoritativen Snapshot", "Le résultat de {0} est inconnu après une panne réseau ; attendez un instantané faisant autorité avant de réessayer", "네트워크 오류 후 {0} 결과를 알 수 없습니다. 다시 시도하기 전에 권한 있는 스냅샷을 기다리세요"),
        new("failure.unexpected", 1, "{0} outcome unknown after an unexpected local failure; wait for an authoritative snapshot before retrying", "{0} 发生意外本地故障后结果未知；请等待权威快照后再重试", "{0} 發生意外本機故障後結果未知；請等待權威快照後再重試", "予期しないローカル障害後の {0} の結果は不明です。権威あるスナップショットを待ってから再試行してください", "El resultado de {0} es desconocido tras un fallo local inesperado; espera una instantánea autoritativa antes de reintentar", "Das Ergebnis von {0} ist nach einem unerwarteten lokalen Fehler unbekannt; warten Sie vor einem erneuten Versuch auf einen autoritativen Snapshot", "Le résultat de {0} est inconnu après une défaillance locale inattendue ; attendez un instantané faisant autorité avant de réessayer", "예기치 않은 로컬 오류 후 {0} 결과를 알 수 없습니다. 다시 시도하기 전에 권한 있는 스냅샷을 기다리세요"),
        new("confirm.refresh.title", 0, "Confirm remote runtime refresh", "确认远程 runtime 刷新", "確認遠端 runtime 重新整理", "リモート runtime の更新を確認", "Confirmar la actualización del runtime remoto", "Aktualisierung der Remote-Runtime bestätigen", "Confirmer l’actualisation du runtime distant", "원격 runtime 새로 고침 확인"),
        new("confirm.capabilities.title", 0, "Confirm remote capability discovery", "确认远程能力探测", "確認遠端能力探測", "リモート機能検出を確認", "Confirmar la detección remota de capacidades", "Remote-Funktionserkennung bestätigen", "Confirmer la détection des capacités distantes", "원격 기능 검색 확인"),
        new("confirm.refresh.heading", 0, "Refresh this remote runtime?", "刷新此远程 runtime？", "重新整理此遠端 runtime？", "このリモート runtime を更新しますか？", "¿Actualizar este runtime remoto?", "Diese Remote-Runtime aktualisieren?", "Actualiser ce runtime distant ?", "이 원격 runtime을 새로 고칠까요?"),
        new("confirm.capabilities.heading", 0, "Discover this runtime's capabilities?", "探测此 runtime 的能力？", "探測此 runtime 的能力？", "この runtime の機能を検出しますか？", "¿Detectar las capacidades de este runtime?", "Funktionen dieser Runtime erkennen?", "Détecter les capacités de ce runtime ?", "이 runtime의 기능을 검색할까요?"),
        new("confirm.context", 3, "{0} / ID {1} / expected revision {2}", "{0} / ID {1} / 预期修订 {2}", "{0} / ID {1} / 預期修訂 {2}", "{0} / ID {1} / 想定リビジョン {2}", "{0} / ID {1} / revisión esperada {2}", "{0} / ID {1} / erwartete Revision {2}", "{0} / ID {1} / révision attendue {2}", "{0} / ID {1} / 예상 리비전 {2}"),
        new("confirm.warning", 0, "This changes remote state. The request is revision-checked and is not retried automatically.", "这会更改远程状态。请求会校验修订，且不会自动重试。", "這會變更遠端狀態。要求會檢查修訂，且不會自動重試。", "これはリモート状態を変更します。要求はリビジョン検査され、自動では再試行されません。", "Esto cambia el estado remoto. La solicitud comprueba la revisión y no se reintenta automáticamente.", "Dies ändert den Remotezustand. Die Anfrage wird gegen die Revision geprüft und nicht automatisch wiederholt.", "Cette action modifie l’état distant. La requête vérifie la révision et n’est pas relancée automatiquement.", "원격 상태가 변경됩니다. 요청은 리비전을 확인하며 자동으로 다시 시도하지 않습니다."),
        new("deployment.context", 3, "{0} / ID {1} / expected revision {2}", "{0} / ID {1} / 预期修订 {2}", "{0} / ID {1} / 預期修訂 {2}", "{0} / ID {1} / 想定リビジョン {2}", "{0} / ID {1} / revisión esperada {2}", "{0} / ID {1} / erwartete Revision {2}", "{0} / ID {1} / révision attendue {2}", "{0} / ID {1} / 예상 리비전 {2}"),
        new("deployment.warning", 0, "This submits an authenticated, revision-checked deployment and is not retried automatically.", "这会提交经过认证并校验修订的部署，且不会自动重试。", "這會提交經過驗證並檢查修訂的部署，且不會自動重試。", "認証済みでリビジョン検査されるデプロイを送信します。自動では再試行されません。", "Esto envía un despliegue autenticado y comprobado por revisión, sin reintentos automáticos.", "Dies sendet eine authentifizierte, revisionsgeprüfte Bereitstellung, die nicht automatisch wiederholt wird.", "Cette action soumet un déploiement authentifié et vérifié par révision, sans nouvelle tentative automatique.", "인증되고 리비전이 확인된 배포를 제출하며 자동으로 다시 시도하지 않습니다."),
        new("validation.path", 0, "use only letters, digits, '.', '/', '_' and '-' within the declared limit", "请在声明的长度限制内仅使用字母、数字、'.'、'/'、'_' 和 '-'", "請在宣告的長度限制內僅使用字母、數字、'.'、'/'、'_' 和 '-'", "宣言された上限内で、英字、数字、'.'、'/'、'_'、'-' のみを使用してください", "usa solo letras, dígitos, '.', '/', '_' y '-' dentro del límite declarado", "verwenden Sie innerhalb des festgelegten Limits nur Buchstaben, Ziffern, '.', '/', '_' und '-'", "utilisez uniquement des lettres, chiffres, '.', '/', '_' et '-' dans la limite déclarée", "선언된 제한 내에서 문자, 숫자, '.', '/', '_', '-'만 사용하세요"),
        new("validation.trimmed", 0, "use trimmed text without control characters within the declared limit", "请在声明的长度限制内使用无首尾空格且不含控制字符的文本", "請在宣告的長度限制內使用無首尾空白且不含控制字元的文字", "宣言された上限内で、前後空白と制御文字を含まないテキストを使用してください", "usa texto sin espacios exteriores ni caracteres de control dentro del límite declarado", "verwenden Sie innerhalb des festgelegten Limits Text ohne Randabstände und Steuerzeichen", "utilisez dans la limite déclarée un texte sans espaces périphériques ni caractères de contrôle", "선언된 제한 내에서 앞뒤 공백과 제어 문자가 없는 텍스트를 사용하세요"),
        new("validation.unsupported", 0, "unsupported input constraint", "不支持的输入约束", "不支援的輸入限制", "サポートされていない入力制約", "restricción de entrada no compatible", "nicht unterstützte Eingabebeschränkung", "contrainte de saisie non prise en charge", "지원되지 않는 입력 제약"),
        new("leselang.label", 0, "Equivalent Leselang", "等价 Leselang", "等價 Leselang", "等価な Leselang", "Leselang equivalente", "Äquivalentes Leselang", "Leselang équivalent", "동등한 Leselang"),
        new("leselang.copy", 0, "Copy Leselang", "复制 Leselang", "複製 Leselang", "Leselang をコピー", "Copiar Leselang", "Leselang kopieren", "Copier Leselang", "Leselang 복사"),
        new("leselang.a11y.preview", 0, "Equivalent Leselang source", "等价 Leselang 源代码", "等價 Leselang 原始碼", "等価な Leselang ソース", "Código Leselang equivalente", "Äquivalenter Leselang-Quelltext", "Source Leselang équivalente", "동등한 Leselang 소스"),
        new("leselang.a11y.copy", 0, "Copy equivalent Leselang source", "复制等价 Leselang 源代码", "複製等價 Leselang 原始碼", "等価な Leselang ソースをコピー", "Copiar el código Leselang equivalente", "Äquivalenten Leselang-Quelltext kopieren", "Copier la source Leselang équivalente", "동등한 Leselang 소스 복사"),
        new("leselang.help.copy", 0, "Copies the code-equivalent operation without executing it.", "复制代码等价操作，但不执行。", "複製程式碼等價操作，但不執行。", "コード上等価な操作を、実行せずにコピーします。", "Copia la operación equivalente en código sin ejecutarla.", "Kopiert den codeäquivalenten Vorgang, ohne ihn auszuführen.", "Copie l’opération équivalente en code sans l’exécuter.", "코드와 동등한 작업을 실행하지 않고 복사합니다."),
        new("leselang.a11y.status", 0, "Leselang copy status", "Leselang 复制状态", "Leselang 複製狀態", "Leselang コピー状態", "Estado de copia de Leselang", "Leselang-Kopierstatus", "État de copie Leselang", "Leselang 복사 상태"),
        new("leselang.status.invalid", 0, "Enter valid values to generate equivalent Leselang.", "请输入有效值以生成等价 Leselang。", "請輸入有效值以產生等價 Leselang。", "等価な Leselang を生成するには有効な値を入力してください。", "Introduce valores válidos para generar Leselang equivalente.", "Geben Sie gültige Werte ein, um äquivalentes Leselang zu erzeugen.", "Saisissez des valeurs valides pour générer le Leselang équivalent.", "동등한 Leselang을 생성하려면 올바른 값을 입력하세요."),
        new("leselang.status.generating", 0, "Generating canonical Leselang from the Rust core...", "正在从 Rust 核心生成规范 Leselang...", "正在從 Rust 核心產生規範 Leselang...", "Rust コアから正規 Leselang を生成しています...", "Generando Leselang canónico desde el núcleo Rust...", "Kanonisches Leselang wird vom Rust-Kern erzeugt...", "Génération du Leselang canonique depuis le cœur Rust...", "Rust 코어에서 정규 Leselang 생성 중..."),
        new("leselang.status.generated", 0, "Canonical source generated by the connected Rust core.", "已由连接的 Rust 核心生成规范源代码。", "已由連線的 Rust 核心產生規範原始碼。", "接続中の Rust コアが正規ソースを生成しました。", "El núcleo Rust conectado generó el código canónico.", "Der verbundene Rust-Kern hat den kanonischen Quelltext erzeugt.", "Le cœur Rust connecté a généré la source canonique.", "연결된 Rust 코어가 정규 소스를 생성했습니다."),
        new("leselang.status.unavailable", 0, "Canonical Leselang unavailable; no local template was substituted.", "规范 Leselang 不可用；未使用本地模板替代。", "規範 Leselang 無法使用；未使用本機範本替代。", "正規 Leselang を利用できません。ローカルテンプレートへの置き換えは行われませんでした。", "Leselang canónico no disponible; no se sustituyó por una plantilla local.", "Kanonisches Leselang ist nicht verfügbar; es wurde keine lokale Vorlage eingesetzt.", "Leselang canonique indisponible ; aucun modèle local n’a été substitué.", "정규 Leselang을 사용할 수 없습니다. 로컬 템플릿으로 대체하지 않았습니다."),
        new("leselang.status.clipboard_unavailable", 0, "Clipboard unavailable.", "剪贴板不可用。", "剪貼簿無法使用。", "クリップボードを利用できません。", "Portapapeles no disponible.", "Zwischenablage nicht verfügbar.", "Presse-papiers indisponible.", "클립보드를 사용할 수 없습니다."),
        new("leselang.status.copied", 0, "Leselang copied. No operation was executed.", "已复制 Leselang，未执行任何操作。", "已複製 Leselang，未執行任何操作。", "Leselang をコピーしました。操作は実行されていません。", "Leselang copiado. No se ejecutó ninguna operación.", "Leselang kopiert. Es wurde kein Vorgang ausgeführt.", "Leselang copié. Aucune opération n’a été exécutée.", "Leselang을 복사했습니다. 작업은 실행되지 않았습니다."),
        new("leselang.status.copy_failed", 0, "Leselang copy failed safely.", "Leselang 复制已安全失败。", "Leselang 複製已安全失敗。", "Leselang のコピーは安全に失敗しました。", "La copia de Leselang falló de forma segura.", "Das Kopieren von Leselang ist sicher fehlgeschlagen.", "La copie de Leselang a échoué de manière sûre.", "Leselang 복사가 안전하게 실패했습니다."),
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
                "desktop remote operation localization entries drifted");
        }
        DesktopDomainCatalogContract.Verify(
            "remote operation",
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
