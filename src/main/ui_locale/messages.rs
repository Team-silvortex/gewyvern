use super::UiLocale;

impl UiLocale {
    pub(crate) fn msg(self, key: &'static str) -> &'static str {
        match (self, key) {
            (Self::Zh, "diagnostics_requires_dsl") => "--diagnostics 需要配合 --dsl",
            (Self::Zh, "summary_only_requires_json") => "--summary-only 需要配合 --json",
            (Self::Zh, "diagnostics_socket_conflict") => {
                "--diagnostics 不能和 socket 监听模式一起使用"
            }
            (Self::Zh, "diagnostics_serve_conflict") => "--diagnostics 不能和 --serve 一起使用",
            (Self::Zh, "dsl_demo_conflict") => "--dsl 不能和 --demo 一起使用",
            (Self::Zh, "demo_socket_conflict") => "--demo 不能和 socket 监听模式一起使用",
            (Self::Zh, "dsl_protocol_conflict") => "--dsl 不能和 --protocol 一起使用",
            (Self::Zh, "dsl_entry_conflict") => "--dsl 不能和 --entry 一起使用",
            (Self::Zh, "entry_requires_protocol") => "--entry 需要和 --protocol 一起使用",
            (Self::Zh, "list_conflict") => "--list-protocols 不能和 --list-entries 一起使用",
            (Self::Zh, "remote_socket_requires_flag") => {
                "远程 TCP 监听默认关闭。只有在你确认要接收未验证的远程事实流时，才应显式加上 --ingest-mode remote-advisory（或兼容的 --socket-trust unsafe-remote / --allow-remote-socket）"
            }
            (Self::Zh, "serve_requires_socket") => "--serve 需要 --unix-socket 或 --tcp-socket",
            (Self::Zh, "api_requires_serve") => "--api-socket 需要和 --serve 一起使用",
            (Self::Zh, "remote_api_requires_flag") => {
                "远程 API 监听默认关闭。只有在你确认要暴露只读分析接口时，才应显式加上 --allow-remote-api"
            }
            (Self::Zh, "unsupported_fragment_combo") => "不支持的片段组合",
            (Self::Zh, "unix_only") => "unix socket 服务仅支持 unix 平台",
            (Self::Zh, "findings_diagnostics_conflict") => {
                "--findings 不能和 --diagnostics 一起使用"
            }
            (Self::Ja, "diagnostics_requires_dsl") => "--diagnostics には --dsl が必要です",
            (Self::Ja, "summary_only_requires_json") => "--summary-only には --json が必要です",
            (Self::Ja, "diagnostics_socket_conflict") => {
                "--diagnostics はソケット待受モードと併用できません"
            }
            (Self::Ja, "diagnostics_serve_conflict") => "--diagnostics は --serve と併用できません",
            (Self::Ja, "dsl_demo_conflict") => "--dsl は --demo と併用できません",
            (Self::Ja, "demo_socket_conflict") => "--demo はソケット待受モードと併用できません",
            (Self::Ja, "dsl_protocol_conflict") => "--dsl は --protocol と併用できません",
            (Self::Ja, "dsl_entry_conflict") => "--dsl は --entry と併用できません",
            (Self::Ja, "entry_requires_protocol") => "--entry には --protocol が必要です",
            (Self::Ja, "serve_requires_socket") => {
                "--serve には --unix-socket または --tcp-socket が必要です"
            }
            (Self::Ja, "unsupported_fragment_combo") => "サポートされていないフラグメント構成です",
            (Self::Ja, "unix_only") => {
                "unix ソケットサービスは unix プラットフォームでのみ利用できます"
            }
            (Self::Ja, "findings_diagnostics_conflict") => {
                "--findings は --diagnostics と併用できません"
            }
            (Self::Ko, "diagnostics_requires_dsl") => "--diagnostics에는 --dsl이 필요합니다",
            (Self::Ko, "summary_only_requires_json") => "--summary-only에는 --json이 필요합니다",
            (Self::Ko, "diagnostics_socket_conflict") => {
                "--diagnostics는 소켓 리스너 모드와 함께 사용할 수 없습니다"
            }
            (Self::Ko, "diagnostics_serve_conflict") => {
                "--diagnostics는 --serve와 함께 사용할 수 없습니다"
            }
            (Self::Ko, "dsl_demo_conflict") => "--dsl은 --demo와 함께 사용할 수 없습니다",
            (Self::Ko, "demo_socket_conflict") => {
                "--demo는 소켓 리스너 모드와 함께 사용할 수 없습니다"
            }
            (Self::Ko, "dsl_protocol_conflict") => "--dsl은 --protocol과 함께 사용할 수 없습니다",
            (Self::Ko, "dsl_entry_conflict") => "--dsl은 --entry와 함께 사용할 수 없습니다",
            (Self::Ko, "entry_requires_protocol") => "--entry에는 --protocol이 필요합니다",
            (Self::Ko, "serve_requires_socket") => {
                "--serve에는 --unix-socket 또는 --tcp-socket이 필요합니다"
            }
            (Self::Ko, "unsupported_fragment_combo") => "지원되지 않는 프래그먼트 조합입니다",
            (Self::Ko, "unix_only") => "unix 소켓 서비스는 unix 플랫폼에서만 지원됩니다",
            (Self::Ko, "findings_diagnostics_conflict") => {
                "--findings는 --diagnostics와 함께 사용할 수 없습니다"
            }
            (Self::Fr, "diagnostics_requires_dsl") => "--diagnostics nécessite --dsl",
            (Self::Fr, "summary_only_requires_json") => "--summary-only nécessite --json",
            (Self::Fr, "diagnostics_socket_conflict") => {
                "--diagnostics ne peut pas être combiné avec le mode écoute socket"
            }
            (Self::Fr, "diagnostics_serve_conflict") => {
                "--diagnostics ne peut pas être combiné avec --serve"
            }
            (Self::Fr, "dsl_demo_conflict") => "--dsl ne peut pas être combiné avec --demo",
            (Self::Fr, "demo_socket_conflict") => {
                "--demo ne peut pas être combiné avec le mode écoute socket"
            }
            (Self::Fr, "dsl_protocol_conflict") => "--dsl ne peut pas être combiné avec --protocol",
            (Self::Fr, "dsl_entry_conflict") => "--dsl ne peut pas être combiné avec --entry",
            (Self::Fr, "entry_requires_protocol") => "--entry nécessite --protocol",
            (Self::Fr, "serve_requires_socket") => {
                "--serve nécessite --unix-socket ou --tcp-socket"
            }
            (Self::Fr, "unsupported_fragment_combo") => {
                "combinaison de fragments non prise en charge"
            }
            (Self::Fr, "unix_only") => {
                "le service socket unix n'est pris en charge que sur les plateformes unix"
            }
            (Self::Fr, "findings_diagnostics_conflict") => {
                "--findings ne peut pas être combiné avec --diagnostics"
            }
            (Self::De, "diagnostics_requires_dsl") => "--diagnostics erfordert --dsl",
            (Self::De, "summary_only_requires_json") => "--summary-only erfordert --json",
            (Self::De, "diagnostics_socket_conflict") => {
                "--diagnostics kann nicht mit dem Socket-Listener-Modus kombiniert werden"
            }
            (Self::De, "diagnostics_serve_conflict") => {
                "--diagnostics kann nicht mit --serve kombiniert werden"
            }
            (Self::De, "dsl_demo_conflict") => "--dsl kann nicht mit --demo kombiniert werden",
            (Self::De, "demo_socket_conflict") => {
                "--demo kann nicht mit dem Socket-Listener-Modus kombiniert werden"
            }
            (Self::De, "dsl_protocol_conflict") => {
                "--dsl kann nicht mit --protocol kombiniert werden"
            }
            (Self::De, "dsl_entry_conflict") => "--dsl kann nicht mit --entry kombiniert werden",
            (Self::De, "entry_requires_protocol") => "--entry erfordert --protocol",
            (Self::De, "serve_requires_socket") => {
                "--serve erfordert --unix-socket oder --tcp-socket"
            }
            (Self::De, "unsupported_fragment_combo") => "nicht unterstützte Fragment-Kombination",
            (Self::De, "unix_only") => {
                "Unix-Socket-Dienst wird nur auf Unix-Plattformen unterstützt"
            }
            (Self::De, "findings_diagnostics_conflict") => {
                "--findings kann nicht mit --diagnostics kombiniert werden"
            }
            (Self::Es, "diagnostics_requires_dsl") => "--diagnostics requiere --dsl",
            (Self::Es, "summary_only_requires_json") => "--summary-only requiere --json",
            (Self::Es, "diagnostics_socket_conflict") => {
                "--diagnostics no se puede combinar con el modo de escucha por socket"
            }
            (Self::Es, "diagnostics_serve_conflict") => {
                "--diagnostics no se puede combinar con --serve"
            }
            (Self::Es, "dsl_demo_conflict") => "--dsl no se puede combinar con --demo",
            (Self::Es, "demo_socket_conflict") => {
                "--demo no se puede combinar con el modo de escucha por socket"
            }
            (Self::Es, "dsl_protocol_conflict") => "--dsl no se puede combinar con --protocol",
            (Self::Es, "dsl_entry_conflict") => "--dsl no se puede combinar con --entry",
            (Self::Es, "entry_requires_protocol") => "--entry requiere --protocol",
            (Self::Es, "serve_requires_socket") => "--serve requiere --unix-socket o --tcp-socket",
            (Self::Es, "unsupported_fragment_combo") => "combinación de fragmentos no compatible",
            (Self::Es, "unix_only") => {
                "el servicio de socket unix solo es compatible en plataformas unix"
            }
            (Self::Es, "findings_diagnostics_conflict") => {
                "--findings no se puede combinar con --diagnostics"
            }
            (Self::Pt, "diagnostics_requires_dsl") => "--diagnostics requer --dsl",
            (Self::Pt, "summary_only_requires_json") => "--summary-only requer --json",
            (Self::Pt, "diagnostics_socket_conflict") => {
                "--diagnostics não pode ser combinado com o modo de escuta por socket"
            }
            (Self::Pt, "diagnostics_serve_conflict") => {
                "--diagnostics não pode ser combinado com --serve"
            }
            (Self::Pt, "dsl_demo_conflict") => "--dsl não pode ser combinado com --demo",
            (Self::Pt, "demo_socket_conflict") => {
                "--demo não pode ser combinado com o modo de escuta por socket"
            }
            (Self::Pt, "dsl_protocol_conflict") => "--dsl não pode ser combinado com --protocol",
            (Self::Pt, "dsl_entry_conflict") => "--dsl não pode ser combinado com --entry",
            (Self::Pt, "entry_requires_protocol") => "--entry requer --protocol",
            (Self::Pt, "serve_requires_socket") => "--serve requer --unix-socket ou --tcp-socket",
            (Self::Pt, "unsupported_fragment_combo") => "combinação de fragmentos não suportada",
            (Self::Pt, "unix_only") => {
                "o serviço de socket unix só é suportado em plataformas unix"
            }
            (Self::Pt, "findings_diagnostics_conflict") => {
                "--findings não pode ser combinado com --diagnostics"
            }
            (Self::Ru, "diagnostics_requires_dsl") => "для --diagnostics требуется --dsl",
            (Self::Ru, "summary_only_requires_json") => "для --summary-only требуется --json",
            (Self::Ru, "diagnostics_socket_conflict") => {
                "--diagnostics нельзя сочетать с режимом сокет-сервера"
            }
            (Self::Ru, "diagnostics_serve_conflict") => "--diagnostics нельзя сочетать с --serve",
            (Self::Ru, "dsl_demo_conflict") => "--dsl нельзя сочетать с --demo",
            (Self::Ru, "demo_socket_conflict") => "--demo нельзя сочетать с режимом сокет-сервера",
            (Self::Ru, "dsl_protocol_conflict") => "--dsl нельзя сочетать с --protocol",
            (Self::Ru, "dsl_entry_conflict") => "--dsl нельзя сочетать с --entry",
            (Self::Ru, "entry_requires_protocol") => "для --entry требуется --protocol",
            (Self::Ru, "serve_requires_socket") => {
                "для --serve требуется --unix-socket или --tcp-socket"
            }
            (Self::Ru, "unsupported_fragment_combo") => "неподдерживаемая комбинация фрагментов",
            (Self::Ru, "unix_only") => {
                "служба unix socket поддерживается только на unix-платформах"
            }
            (Self::Ru, "findings_diagnostics_conflict") => {
                "--findings нельзя сочетать с --diagnostics"
            }
            (_, "diagnostics_requires_dsl") => "--diagnostics requires --dsl",
            (_, "summary_only_requires_json") => "--summary-only requires --json",
            (_, "diagnostics_socket_conflict") => {
                "--diagnostics cannot be combined with socket listener mode"
            }
            (_, "diagnostics_serve_conflict") => "--diagnostics cannot be combined with --serve",
            (_, "dsl_demo_conflict") => "--dsl cannot be combined with --demo",
            (_, "demo_socket_conflict") => "--demo cannot be combined with socket listener mode",
            (_, "dsl_protocol_conflict") => "--dsl cannot be combined with --protocol",
            (_, "dsl_entry_conflict") => "--dsl cannot be combined with --entry",
            (_, "entry_requires_protocol") => "--entry requires --protocol",
            (_, "pid_not_yet_supported") => {
                "--pid is not wired to live process capture yet; the current CLI only supports synthetic demos or socket-ingested sessions, so use those paths instead of expecting direct pid inspection"
            }
            (_, "pid_socket_conflict") => {
                "--pid cannot be combined with socket ingest because incoming fact lineage is unverified; run a broader advisory scan first, then narrow down with a verified local source"
            }
            (_, "list_conflict") => "--list-protocols cannot be combined with --list-entries",
            (_, "remote_socket_requires_flag") => {
                "remote TCP listeners are off by default; only opt in with --ingest-mode remote-advisory (or legacy --socket-trust unsafe-remote / --allow-remote-socket) when you intentionally want unverified remote ingest"
            }
            (_, "serve_requires_socket") => "--serve requires --unix-socket or --tcp-socket",
            (_, "api_requires_serve") => "--api-socket requires --serve",
            (_, "remote_api_requires_flag") => {
                "remote API listeners are off by default; only opt in with --allow-remote-api when you intentionally want to expose the read-only analysis API"
            }
            (_, "unsupported_fragment_combo") => "unsupported fragment combination",
            (_, "unix_only") => "unix socket service is only supported on unix platforms",
            (_, "findings_diagnostics_conflict") => {
                "--findings cannot be combined with --diagnostics"
            }
            _ => key,
        }
    }

    pub(crate) fn msgf(self, key: &'static str, a: &str, b: Option<&str>) -> String {
        match (self, key) {
            (Self::Zh, "unsupported_demo") => {
                format!("不支持的 demo 模式 '{a}'，期望 tcp、udp 或 both")
            }
            (Self::Zh, "unsupported_template") => format!("不支持的模板 '{a}'，期望 tcp 或 udp"),
            (Self::Zh, "dsl_compile_failed") => format!("DSL 编译失败: {a}"),
            (Self::Zh, "binding_diagnostics_failed") => format!("binding 诊断失败: {a}"),
            (Self::Zh, "socket_session_failed") => format!("socket 会话失败: {a}"),
            (Self::Zh, "socket_service_failed") => format!("socket 服务失败: {a}"),
            (Self::Zh, "write_failed") => format!("写入输出到 {a} 失败: {}", b.unwrap_or("")),
            (Self::Zh, "unknown_argument") => format!("未知参数 '{a}'\n{}", self.usage()),
            (Self::Zh, "missing_demo") => "缺少 --demo 的值，期望 tcp、udp 或 both".into(),
            (Self::Zh, "missing_max_sessions") => "缺少 --max-sessions 的值，期望正整数".into(),
            (Self::Zh, "invalid_max_sessions") => "--max-sessions 必须是正整数".into(),
            (Self::Zh, "missing_template") => "缺少 --template 的值，期望 tcp 或 udp".into(),
            (Self::Zh, "missing_dsl") => "缺少 --dsl 的值，期望 DSL 文件路径".into(),
            (Self::Zh, "missing_protocol") => "缺少 --protocol 的值，期望协议名称".into(),
            (Self::Zh, "missing_entry") => "缺少 --entry 的值，期望 gewy 入口模式".into(),
            (Self::Zh, "missing_pid") => "缺少 --pid 的值，期望进程 PID".into(),
            (Self::Zh, "invalid_pid") => "--pid 必须是正整数".into(),
            (Self::Zh, "pid_not_yet_supported") => {
                "--pid 还没有接通真实活进程抓取；当前 CLI 只支持 synthetic demo 或 socket 导入的会话，不要把它当成直接进程诊断入口".into()
            }
            (Self::Zh, "missing_unix_socket") => "缺少 --unix-socket 的值，期望文件路径".into(),
            (Self::Zh, "missing_tcp_socket") => "缺少 --tcp-socket 的值，期望 host:port".into(),
            (Self::Zh, "missing_api_socket") => "缺少 --api-socket 的值，期望 host:port".into(),
            (Self::Zh, "missing_socket_trust") => {
                "缺少 --socket-trust 的值，期望 trusted-local 或 unsafe-remote".into()
            }
            (Self::Zh, "missing_ingest_mode") => {
                "缺少 --ingest-mode 的值，期望 local-advisory 或 remote-advisory".into()
            }
            (Self::Zh, "missing_out") => "缺少 --out 的值，期望可写文件路径".into(),
            (Self::Zh, "unsupported_protocol") => format!("不支持的协议 '{a}'"),
            (Self::Zh, "unsupported_socket_trust") => {
                format!("不支持的 socket 信任模式 '{a}'，期望 trusted-local 或 unsafe-remote")
            }
            (Self::Zh, "unsupported_ingest_mode") => {
                format!("不支持的采集模式 '{a}'，期望 local-advisory 或 remote-advisory")
            }
            (_, "unsupported_demo") => {
                format!("unsupported demo mode '{a}', expected tcp, udp, or both")
            }
            (_, "unsupported_template") => {
                format!("unsupported template '{a}', expected tcp or udp")
            }
            (_, "dsl_compile_failed") => format!("dsl compile failed: {a}"),
            (_, "binding_diagnostics_failed") => format!("binding diagnostics failed: {a}"),
            (_, "socket_session_failed") => format!("socket session failed: {a}"),
            (_, "socket_service_failed") => format!("socket service failed: {a}"),
            (_, "write_failed") => format!("failed to write output to {a}: {}", b.unwrap_or("")),
            (_, "unknown_argument") => format!("unknown argument '{a}'\n{}", self.usage()),
            (_, "missing_demo") => "missing value for --demo, expected tcp, udp, or both".into(),
            (_, "missing_max_sessions") => {
                "missing value for --max-sessions, expected a positive integer".into()
            }
            (_, "invalid_max_sessions") => "--max-sessions must be a positive integer".into(),
            (_, "missing_template") => "missing value for --template, expected tcp or udp".into(),
            (_, "missing_dsl") => "missing value for --dsl, expected a DSL file path".into(),
            (_, "missing_protocol") => {
                "missing value for --protocol, expected a built-in protocol name".into()
            }
            (_, "missing_entry") => "missing value for --entry, expected a gewy entry mode".into(),
            (_, "missing_pid") => "missing value for --pid, expected a process pid".into(),
            (_, "invalid_pid") => "--pid must be a positive integer".into(),
            (_, "missing_unix_socket") => {
                "missing value for --unix-socket, expected a filesystem path".into()
            }
            (_, "missing_tcp_socket") => {
                "missing value for --tcp-socket, expected host:port".into()
            }
            (_, "missing_api_socket") => {
                "missing value for --api-socket, expected host:port".into()
            }
            (_, "missing_socket_trust") => {
                "missing value for --socket-trust, expected trusted-local or unsafe-remote".into()
            }
            (_, "missing_ingest_mode") => {
                "missing value for --ingest-mode, expected local-advisory or remote-advisory".into()
            }
            (_, "missing_out") => "missing value for --out, expected a writable file path".into(),
            (_, "unsupported_protocol") => format!("unsupported protocol '{a}'"),
            (_, "unsupported_socket_trust") => format!(
                "unsupported socket trust mode '{a}', expected trusted-local or unsafe-remote"
            ),
            (_, "unsupported_ingest_mode") => {
                format!("unsupported ingest mode '{a}', expected local-advisory or remote-advisory")
            }
            _ => key.into(),
        }
    }
}
