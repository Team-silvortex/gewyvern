use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::gewyc::{
    RenderFormat, compile_diagnostics_report_file, render_diagnostics_report,
};
use gewyvern::http::{compose_http_transactions, HttpSuspectSide, HttpTransactionView};
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact,
    SessionId, SockLineageFact, TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::socket_input::{
    bind_unix_socket_listener, run_tcp_socket_session, run_tcp_socket_session_on_listener,
    run_tcp_socket_session_on_listener_with_binding, run_tcp_socket_session_with_binding,
    run_unix_socket_session, run_unix_socket_session_on_listener,
    run_unix_socket_session_on_listener_with_binding, run_unix_socket_session_with_binding,
};
use gewyvern::template::{handshake_debug_template, udp_debug_template, TemplateBinding};
use std::env;
use std::fs;
use std::net::TcpListener;
use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiLocale {
    En,
    Zh,
    Ja,
    Ko,
    Fr,
    De,
    Es,
    Pt,
    Ru,
}

impl UiLocale {
    fn detect() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = env::var(key) {
                let value = value.to_ascii_lowercase();
                if value.starts_with("zh") {
                    return Self::Zh;
                }
                if value.starts_with("ja") {
                    return Self::Ja;
                }
                if value.starts_with("ko") {
                    return Self::Ko;
                }
                if value.starts_with("fr") {
                    return Self::Fr;
                }
                if value.starts_with("de") {
                    return Self::De;
                }
                if value.starts_with("es") {
                    return Self::Es;
                }
                if value.starts_with("pt") {
                    return Self::Pt;
                }
                if value.starts_with("ru") {
                    return Self::Ru;
                }
            }
        }
        Self::En
    }

    fn none(self) -> &'static str {
        match self {
            Self::En => "none",
            Self::Zh => "无",
            Self::Ja => "なし",
            Self::Ko => "없음",
            Self::Fr => "aucun",
            Self::De => "keine",
            Self::Es => "ninguno",
            Self::Pt => "nenhum",
            Self::Ru => "нет",
        }
    }

    fn usage(self) -> &'static str {
        match self {
            Self::Zh => "用法: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::Ja => "使い方: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::Ko => "사용법: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::Fr => "Utilisation : gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::De => "Verwendung: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::Es => "Uso: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::Pt => "Uso: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::Ru => "Использование: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
            Self::En => "usage: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]",
        }
    }

    fn label(self, key: &'static str) -> &'static str {
        match (self, key) {
            (Self::Zh, "template") => "模板",
            (Self::Zh, "fragments") => "片段",
            (Self::Zh, "fragments_loaded") => "已加载片段",
            (Self::Zh, "hookpoints_failed") => "失败挂载点",
            (Self::Zh, "accepted_facts") => "接受事实",
            (Self::Zh, "rejected_facts") => "拒收事实",
            (Self::Zh, "flows") => "流",
            (Self::Zh, "program_findings") => "程序发现",
            (Self::Zh, "module_findings") => "模块发现",
            (Self::Zh, "reasons") => "原因链",
            (Self::Zh, "degraded") => "降级",
            (Self::Zh, "suspect_areas") => "可疑区域",
            (Self::Zh, "suspect_modules") => "可疑模块",
            (Self::Zh, "program_model") => "程序模型",
            (Self::Zh, "reason_model") => "原因模型",
            (Self::Zh, "program_rule") => "程序规则",
            (Self::Zh, "reason_rule") => "原因规则",
            (Self::Zh, "tier") => "层级",
            (Self::Zh, "supported") => "支持",
            (Self::Zh, "required") => "需要",
            (Self::Zh, "supporting") => "支撑片段",
            (Self::Zh, "missing") => "缺失",
            (Self::Zh, "severity") => "严重度",
            (Self::Zh, "process") => "进程",
            (Self::Zh, "operation") => "操作",
            (Self::Zh, "causes") => "原因",
            (Self::Zh, "module") => "模块",
            (Self::Zh, "trace") => "证据链",
            (Self::Zh, "summary") => "摘要",
            (Self::Ja, "template") => "テンプレート",
            (Self::Ja, "fragments") => "フラグメント",
            (Self::Ja, "fragments_loaded") => "ロード済みフラグメント",
            (Self::Ja, "hookpoints_failed") => "失敗したフックポイント",
            (Self::Ja, "accepted_facts") => "受理されたファクト",
            (Self::Ja, "rejected_facts") => "拒否されたファクト",
            (Self::Ja, "flows") => "フロー",
            (Self::Ja, "program_findings") => "プログラム所見",
            (Self::Ja, "module_findings") => "モジュール所見",
            (Self::Ja, "reasons") => "理由チェーン",
            (Self::Ja, "degraded") => "劣化",
            (Self::Ja, "suspect_areas") => "疑わしい領域",
            (Self::Ja, "suspect_modules") => "疑わしいモジュール",
            (Self::Ja, "program_model") => "プログラムモデル",
            (Self::Ja, "reason_model") => "理由モデル",
            (Self::Ja, "program_rule") => "プログラム規則",
            (Self::Ja, "reason_rule") => "理由規則",
            (Self::Ja, "tier") => "階層",
            (Self::Ja, "supported") => "サポート",
            (Self::Ja, "required") => "必要",
            (Self::Ja, "supporting") => "支援フラグメント",
            (Self::Ja, "missing") => "不足",
            (Self::Ja, "severity") => "重大度",
            (Self::Ja, "process") => "プロセス",
            (Self::Ja, "operation") => "操作",
            (Self::Ja, "causes") => "原因",
            (Self::Ja, "module") => "モジュール",
            (Self::Ja, "trace") => "証拠トレース",
            (Self::Ja, "summary") => "要約",
            (Self::Ko, "template") => "템플릿",
            (Self::Ko, "fragments") => "프래그먼트",
            (Self::Ko, "fragments_loaded") => "로드된 프래그먼트",
            (Self::Ko, "hookpoints_failed") => "실패한 훅포인트",
            (Self::Ko, "accepted_facts") => "수용된 사실",
            (Self::Ko, "rejected_facts") => "거부된 사실",
            (Self::Ko, "flows") => "플로우",
            (Self::Ko, "program_findings") => "프로그램 발견",
            (Self::Ko, "module_findings") => "모듈 발견",
            (Self::Ko, "reasons") => "이유 체인",
            (Self::Ko, "degraded") => "저하",
            (Self::Ko, "suspect_areas") => "의심 영역",
            (Self::Ko, "suspect_modules") => "의심 모듈",
            (Self::Ko, "program_model") => "프로그램 모델",
            (Self::Ko, "reason_model") => "이유 모델",
            (Self::Ko, "program_rule") => "프로그램 규칙",
            (Self::Ko, "reason_rule") => "이유 규칙",
            (Self::Ko, "tier") => "등급",
            (Self::Ko, "supported") => "지원 여부",
            (Self::Ko, "required") => "필요",
            (Self::Ko, "supporting") => "지원 프래그먼트",
            (Self::Ko, "missing") => "누락",
            (Self::Ko, "severity") => "심각도",
            (Self::Ko, "process") => "프로세스",
            (Self::Ko, "operation") => "작업",
            (Self::Ko, "causes") => "원인",
            (Self::Ko, "module") => "모듈",
            (Self::Ko, "trace") => "증거 추적",
            (Self::Ko, "summary") => "요약",
            (Self::Fr, "template") => "modèle",
            (Self::Fr, "fragments") => "fragments",
            (Self::Fr, "fragments_loaded") => "fragments_chargés",
            (Self::Fr, "hookpoints_failed") => "points_d_accroche_échoués",
            (Self::Fr, "accepted_facts") => "faits_acceptés",
            (Self::Fr, "rejected_facts") => "faits_rejetés",
            (Self::Fr, "flows") => "flux",
            (Self::Fr, "program_findings") => "résultats_programme",
            (Self::Fr, "module_findings") => "résultats_module",
            (Self::Fr, "reasons") => "raisons",
            (Self::Fr, "degraded") => "dégradé",
            (Self::Fr, "suspect_areas") => "zones_suspectes",
            (Self::Fr, "suspect_modules") => "modules_suspects",
            (Self::Fr, "program_model") => "modèle_programme",
            (Self::Fr, "reason_model") => "modèle_de_raison",
            (Self::Fr, "program_rule") => "règle_programme",
            (Self::Fr, "reason_rule") => "règle_de_raison",
            (Self::Fr, "tier") => "niveau",
            (Self::Fr, "supported") => "pris_en_charge",
            (Self::Fr, "required") => "requis",
            (Self::Fr, "supporting") => "fragments_support",
            (Self::Fr, "missing") => "manquant",
            (Self::Fr, "severity") => "gravité",
            (Self::Fr, "process") => "processus",
            (Self::Fr, "operation") => "opération",
            (Self::Fr, "causes") => "causes",
            (Self::Fr, "module") => "module",
            (Self::Fr, "trace") => "trace",
            (Self::Fr, "summary") => "résumé",
            (Self::De, "template") => "vorlage",
            (Self::De, "fragments") => "fragmente",
            (Self::De, "fragments_loaded") => "geladene_fragmente",
            (Self::De, "hookpoints_failed") => "fehlgeschlagene_hookpoints",
            (Self::De, "accepted_facts") => "akzeptierte_fakten",
            (Self::De, "rejected_facts") => "abgelehnte_fakten",
            (Self::De, "flows") => "flüsse",
            (Self::De, "program_findings") => "programm_befunde",
            (Self::De, "module_findings") => "modul_befunde",
            (Self::De, "reasons") => "gründe",
            (Self::De, "degraded") => "degradiert",
            (Self::De, "suspect_areas") => "verdächtige_bereiche",
            (Self::De, "suspect_modules") => "verdächtige_module",
            (Self::De, "program_model") => "programm_modell",
            (Self::De, "reason_model") => "begründungs_modell",
            (Self::De, "program_rule") => "programm_regel",
            (Self::De, "reason_rule") => "begründungs_regel",
            (Self::De, "tier") => "stufe",
            (Self::De, "supported") => "unterstützt",
            (Self::De, "required") => "erforderlich",
            (Self::De, "supporting") => "stützende_fragmente",
            (Self::De, "missing") => "fehlend",
            (Self::De, "severity") => "schweregrad",
            (Self::De, "process") => "prozess",
            (Self::De, "operation") => "operation",
            (Self::De, "causes") => "ursachen",
            (Self::De, "module") => "modul",
            (Self::De, "trace") => "spur",
            (Self::De, "summary") => "zusammenfassung",
            (Self::Es, "template") => "plantilla",
            (Self::Es, "fragments") => "fragmentos",
            (Self::Es, "fragments_loaded") => "fragmentos_cargados",
            (Self::Es, "hookpoints_failed") => "hookpoints_fallidos",
            (Self::Es, "accepted_facts") => "hechos_aceptados",
            (Self::Es, "rejected_facts") => "hechos_rechazados",
            (Self::Es, "flows") => "flujos",
            (Self::Es, "program_findings") => "hallazgos_programa",
            (Self::Es, "module_findings") => "hallazgos_módulo",
            (Self::Es, "reasons") => "razones",
            (Self::Es, "degraded") => "degradado",
            (Self::Es, "suspect_areas") => "áreas_sospechosas",
            (Self::Es, "suspect_modules") => "módulos_sospechosos",
            (Self::Es, "program_model") => "modelo_programa",
            (Self::Es, "reason_model") => "modelo_razón",
            (Self::Es, "program_rule") => "regla_programa",
            (Self::Es, "reason_rule") => "regla_razón",
            (Self::Es, "tier") => "nivel",
            (Self::Es, "supported") => "soportado",
            (Self::Es, "required") => "requerido",
            (Self::Es, "supporting") => "fragmentos_soporte",
            (Self::Es, "missing") => "faltante",
            (Self::Es, "severity") => "severidad",
            (Self::Es, "process") => "proceso",
            (Self::Es, "operation") => "operación",
            (Self::Es, "causes") => "causas",
            (Self::Es, "module") => "módulo",
            (Self::Es, "trace") => "traza",
            (Self::Es, "summary") => "resumen",
            (Self::Pt, "template") => "modelo",
            (Self::Pt, "fragments") => "fragmentos",
            (Self::Pt, "fragments_loaded") => "fragmentos_carregados",
            (Self::Pt, "hookpoints_failed") => "hookpoints_com_falha",
            (Self::Pt, "accepted_facts") => "fatos_aceitos",
            (Self::Pt, "rejected_facts") => "fatos_rejeitados",
            (Self::Pt, "flows") => "fluxos",
            (Self::Pt, "program_findings") => "achados_programa",
            (Self::Pt, "module_findings") => "achados_módulo",
            (Self::Pt, "reasons") => "razões",
            (Self::Pt, "degraded") => "degradado",
            (Self::Pt, "suspect_areas") => "áreas_suspeitas",
            (Self::Pt, "suspect_modules") => "módulos_suspeitos",
            (Self::Pt, "program_model") => "modelo_programa",
            (Self::Pt, "reason_model") => "modelo_de_razão",
            (Self::Pt, "program_rule") => "regra_programa",
            (Self::Pt, "reason_rule") => "regra_de_razão",
            (Self::Pt, "tier") => "nível",
            (Self::Pt, "supported") => "suportado",
            (Self::Pt, "required") => "necessário",
            (Self::Pt, "supporting") => "fragmentos_de_suporte",
            (Self::Pt, "missing") => "faltando",
            (Self::Pt, "severity") => "severidade",
            (Self::Pt, "process") => "processo",
            (Self::Pt, "operation") => "operação",
            (Self::Pt, "causes") => "causas",
            (Self::Pt, "module") => "módulo",
            (Self::Pt, "trace") => "rastreamento",
            (Self::Pt, "summary") => "resumo",
            (Self::Ru, "template") => "шаблон",
            (Self::Ru, "fragments") => "фрагменты",
            (Self::Ru, "fragments_loaded") => "загруженные_фрагменты",
            (Self::Ru, "hookpoints_failed") => "ошибки_hookpoint",
            (Self::Ru, "accepted_facts") => "принятые_факты",
            (Self::Ru, "rejected_facts") => "отклонённые_факты",
            (Self::Ru, "flows") => "потоки",
            (Self::Ru, "program_findings") => "находки_программы",
            (Self::Ru, "module_findings") => "находки_модуля",
            (Self::Ru, "reasons") => "цепочки_причин",
            (Self::Ru, "degraded") => "деградировано",
            (Self::Ru, "suspect_areas") => "подозрительные_области",
            (Self::Ru, "suspect_modules") => "подозрительные_модули",
            (Self::Ru, "program_model") => "модель_программы",
            (Self::Ru, "reason_model") => "модель_объяснения",
            (Self::Ru, "program_rule") => "правило_программы",
            (Self::Ru, "reason_rule") => "правило_объяснения",
            (Self::Ru, "tier") => "уровень",
            (Self::Ru, "supported") => "поддерживается",
            (Self::Ru, "required") => "требуется",
            (Self::Ru, "supporting") => "поддерживающие_фрагменты",
            (Self::Ru, "missing") => "отсутствует",
            (Self::Ru, "severity") => "серьёзность",
            (Self::Ru, "process") => "процесс",
            (Self::Ru, "operation") => "операция",
            (Self::Ru, "causes") => "причины",
            (Self::Ru, "module") => "модуль",
            (Self::Ru, "trace") => "трасса",
            (Self::Ru, "summary") => "сводка",
            (_, "template") => "template",
            (_, "fragments") => "fragments",
            (_, "fragments_loaded") => "fragments_loaded",
            (_, "hookpoints_failed") => "hookpoints_failed",
            (_, "accepted_facts") => "accepted_facts",
            (_, "rejected_facts") => "rejected_facts",
            (_, "flows") => "flows",
            (_, "program_findings") => "program_findings",
            (_, "module_findings") => "module_findings",
            (_, "reasons") => "reasons",
            (_, "degraded") => "degraded",
            (_, "suspect_areas") => "suspect_areas",
            (_, "suspect_modules") => "suspect_modules",
            (_, "program_model") => "program_model",
            (_, "reason_model") => "reason_model",
            (_, "program_rule") => "program_rule",
            (_, "reason_rule") => "reason_rule",
            (_, "tier") => "tier",
            (_, "supported") => "supported",
            (_, "required") => "required",
            (_, "supporting") => "supporting",
            (_, "missing") => "missing",
            (_, "severity") => "severity",
            (_, "process") => "process",
            (_, "operation") => "operation",
            (_, "causes") => "causes",
            (_, "module") => "module",
            (_, "trace") => "trace",
            (_, "summary") => "summary",
            (_, "phase") => "phase",
            (_, "phases") => "phases",
            (_, "phase_transition") => "phase_transition",
            (_, "phase_transitions") => "phase_transitions",
            _ => key,
        }
    }

    fn msg(self, key: &'static str) -> &'static str {
        match (self, key) {
            (Self::Zh, "diagnostics_requires_dsl") => "--diagnostics 需要配合 --dsl",
            (Self::Zh, "summary_only_requires_json") => "--summary-only 需要配合 --json",
            (Self::Zh, "diagnostics_socket_conflict") => "--diagnostics 不能和 socket 监听模式一起使用",
            (Self::Zh, "diagnostics_serve_conflict") => "--diagnostics 不能和 --serve 一起使用",
            (Self::Zh, "dsl_demo_conflict") => "--dsl 不能和 --demo 一起使用",
            (Self::Zh, "demo_socket_conflict") => "--demo 不能和 socket 监听模式一起使用",
            (Self::Zh, "serve_requires_socket") => "--serve 需要 --unix-socket 或 --tcp-socket",
            (Self::Zh, "unsupported_fragment_combo") => "不支持的片段组合",
            (Self::Zh, "unix_only") => "unix socket 服务仅支持 unix 平台",
            (Self::Zh, "findings_diagnostics_conflict") => "--findings 不能和 --diagnostics 一起使用",
            (Self::Ja, "diagnostics_requires_dsl") => "--diagnostics には --dsl が必要です",
            (Self::Ja, "summary_only_requires_json") => "--summary-only には --json が必要です",
            (Self::Ja, "diagnostics_socket_conflict") => "--diagnostics はソケット待受モードと併用できません",
            (Self::Ja, "diagnostics_serve_conflict") => "--diagnostics は --serve と併用できません",
            (Self::Ja, "dsl_demo_conflict") => "--dsl は --demo と併用できません",
            (Self::Ja, "demo_socket_conflict") => "--demo はソケット待受モードと併用できません",
            (Self::Ja, "serve_requires_socket") => "--serve には --unix-socket または --tcp-socket が必要です",
            (Self::Ja, "unsupported_fragment_combo") => "サポートされていないフラグメント構成です",
            (Self::Ja, "unix_only") => "unix ソケットサービスは unix プラットフォームでのみ利用できます",
            (Self::Ja, "findings_diagnostics_conflict") => "--findings は --diagnostics と併用できません",
            (Self::Ko, "diagnostics_requires_dsl") => "--diagnostics에는 --dsl이 필요합니다",
            (Self::Ko, "summary_only_requires_json") => "--summary-only에는 --json이 필요합니다",
            (Self::Ko, "diagnostics_socket_conflict") => "--diagnostics는 소켓 리스너 모드와 함께 사용할 수 없습니다",
            (Self::Ko, "diagnostics_serve_conflict") => "--diagnostics는 --serve와 함께 사용할 수 없습니다",
            (Self::Ko, "dsl_demo_conflict") => "--dsl은 --demo와 함께 사용할 수 없습니다",
            (Self::Ko, "demo_socket_conflict") => "--demo는 소켓 리스너 모드와 함께 사용할 수 없습니다",
            (Self::Ko, "serve_requires_socket") => "--serve에는 --unix-socket 또는 --tcp-socket이 필요합니다",
            (Self::Ko, "unsupported_fragment_combo") => "지원되지 않는 프래그먼트 조합입니다",
            (Self::Ko, "unix_only") => "unix 소켓 서비스는 unix 플랫폼에서만 지원됩니다",
            (Self::Ko, "findings_diagnostics_conflict") => "--findings는 --diagnostics와 함께 사용할 수 없습니다",
            (Self::Fr, "diagnostics_requires_dsl") => "--diagnostics nécessite --dsl",
            (Self::Fr, "summary_only_requires_json") => "--summary-only nécessite --json",
            (Self::Fr, "diagnostics_socket_conflict") => "--diagnostics ne peut pas être combiné avec le mode écoute socket",
            (Self::Fr, "diagnostics_serve_conflict") => "--diagnostics ne peut pas être combiné avec --serve",
            (Self::Fr, "dsl_demo_conflict") => "--dsl ne peut pas être combiné avec --demo",
            (Self::Fr, "demo_socket_conflict") => "--demo ne peut pas être combiné avec le mode écoute socket",
            (Self::Fr, "serve_requires_socket") => "--serve nécessite --unix-socket ou --tcp-socket",
            (Self::Fr, "unsupported_fragment_combo") => "combinaison de fragments non prise en charge",
            (Self::Fr, "unix_only") => "le service socket unix n'est pris en charge que sur les plateformes unix",
            (Self::Fr, "findings_diagnostics_conflict") => "--findings ne peut pas être combiné avec --diagnostics",
            (Self::De, "diagnostics_requires_dsl") => "--diagnostics erfordert --dsl",
            (Self::De, "summary_only_requires_json") => "--summary-only erfordert --json",
            (Self::De, "diagnostics_socket_conflict") => "--diagnostics kann nicht mit dem Socket-Listener-Modus kombiniert werden",
            (Self::De, "diagnostics_serve_conflict") => "--diagnostics kann nicht mit --serve kombiniert werden",
            (Self::De, "dsl_demo_conflict") => "--dsl kann nicht mit --demo kombiniert werden",
            (Self::De, "demo_socket_conflict") => "--demo kann nicht mit dem Socket-Listener-Modus kombiniert werden",
            (Self::De, "serve_requires_socket") => "--serve erfordert --unix-socket oder --tcp-socket",
            (Self::De, "unsupported_fragment_combo") => "nicht unterstützte Fragment-Kombination",
            (Self::De, "unix_only") => "Unix-Socket-Dienst wird nur auf Unix-Plattformen unterstützt",
            (Self::De, "findings_diagnostics_conflict") => "--findings kann nicht mit --diagnostics kombiniert werden",
            (Self::Es, "diagnostics_requires_dsl") => "--diagnostics requiere --dsl",
            (Self::Es, "summary_only_requires_json") => "--summary-only requiere --json",
            (Self::Es, "diagnostics_socket_conflict") => "--diagnostics no se puede combinar con el modo de escucha por socket",
            (Self::Es, "diagnostics_serve_conflict") => "--diagnostics no se puede combinar con --serve",
            (Self::Es, "dsl_demo_conflict") => "--dsl no se puede combinar con --demo",
            (Self::Es, "demo_socket_conflict") => "--demo no se puede combinar con el modo de escucha por socket",
            (Self::Es, "serve_requires_socket") => "--serve requiere --unix-socket o --tcp-socket",
            (Self::Es, "unsupported_fragment_combo") => "combinación de fragmentos no compatible",
            (Self::Es, "unix_only") => "el servicio de socket unix solo es compatible en plataformas unix",
            (Self::Es, "findings_diagnostics_conflict") => "--findings no se puede combinar con --diagnostics",
            (Self::Pt, "diagnostics_requires_dsl") => "--diagnostics requer --dsl",
            (Self::Pt, "summary_only_requires_json") => "--summary-only requer --json",
            (Self::Pt, "diagnostics_socket_conflict") => "--diagnostics não pode ser combinado com o modo de escuta por socket",
            (Self::Pt, "diagnostics_serve_conflict") => "--diagnostics não pode ser combinado com --serve",
            (Self::Pt, "dsl_demo_conflict") => "--dsl não pode ser combinado com --demo",
            (Self::Pt, "demo_socket_conflict") => "--demo não pode ser combinado com o modo de escuta por socket",
            (Self::Pt, "serve_requires_socket") => "--serve requer --unix-socket ou --tcp-socket",
            (Self::Pt, "unsupported_fragment_combo") => "combinação de fragmentos não suportada",
            (Self::Pt, "unix_only") => "o serviço de socket unix só é suportado em plataformas unix",
            (Self::Pt, "findings_diagnostics_conflict") => "--findings não pode ser combinado com --diagnostics",
            (Self::Ru, "diagnostics_requires_dsl") => "для --diagnostics требуется --dsl",
            (Self::Ru, "summary_only_requires_json") => "для --summary-only требуется --json",
            (Self::Ru, "diagnostics_socket_conflict") => "--diagnostics нельзя сочетать с режимом сокет-сервера",
            (Self::Ru, "diagnostics_serve_conflict") => "--diagnostics нельзя сочетать с --serve",
            (Self::Ru, "dsl_demo_conflict") => "--dsl нельзя сочетать с --demo",
            (Self::Ru, "demo_socket_conflict") => "--demo нельзя сочетать с режимом сокет-сервера",
            (Self::Ru, "serve_requires_socket") => "для --serve требуется --unix-socket или --tcp-socket",
            (Self::Ru, "unsupported_fragment_combo") => "неподдерживаемая комбинация фрагментов",
            (Self::Ru, "unix_only") => "служба unix socket поддерживается только на unix-платформах",
            (Self::Ru, "findings_diagnostics_conflict") => "--findings нельзя сочетать с --diagnostics",
            (_, "diagnostics_requires_dsl") => "--diagnostics requires --dsl",
            (_, "summary_only_requires_json") => "--summary-only requires --json",
            (_, "diagnostics_socket_conflict") => "--diagnostics cannot be combined with socket listener mode",
            (_, "diagnostics_serve_conflict") => "--diagnostics cannot be combined with --serve",
            (_, "dsl_demo_conflict") => "--dsl cannot be combined with --demo",
            (_, "demo_socket_conflict") => "--demo cannot be combined with socket listener mode",
            (_, "serve_requires_socket") => "--serve requires --unix-socket or --tcp-socket",
            (_, "unsupported_fragment_combo") => "unsupported fragment combination",
            (_, "unix_only") => "unix socket service is only supported on unix platforms",
            (_, "findings_diagnostics_conflict") => "--findings cannot be combined with --diagnostics",
            _ => key,
        }
    }

    fn msgf(self, key: &'static str, a: &str, b: Option<&str>) -> String {
        match (self, key) {
            (Self::Zh, "unsupported_demo") => format!("不支持的 demo 模式 '{a}'，期望 tcp、udp 或 both"),
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
            (Self::Zh, "missing_unix_socket") => "缺少 --unix-socket 的值，期望文件路径".into(),
            (Self::Zh, "missing_tcp_socket") => "缺少 --tcp-socket 的值，期望 host:port".into(),
            (Self::Zh, "missing_out") => "缺少 --out 的值，期望可写文件路径".into(),
            (_, "unsupported_demo") => format!("unsupported demo mode '{a}', expected tcp, udp, or both"),
            (_, "unsupported_template") => format!("unsupported template '{a}', expected tcp or udp"),
            (_, "dsl_compile_failed") => format!("dsl compile failed: {a}"),
            (_, "binding_diagnostics_failed") => format!("binding diagnostics failed: {a}"),
            (_, "socket_session_failed") => format!("socket session failed: {a}"),
            (_, "socket_service_failed") => format!("socket service failed: {a}"),
            (_, "write_failed") => format!("failed to write output to {a}: {}", b.unwrap_or("")),
            (_, "unknown_argument") => format!("unknown argument '{a}'\n{}", self.usage()),
            (_, "missing_demo") => "missing value for --demo, expected tcp, udp, or both".into(),
            (_, "missing_max_sessions") => "missing value for --max-sessions, expected a positive integer".into(),
            (_, "invalid_max_sessions") => "--max-sessions must be a positive integer".into(),
            (_, "missing_template") => "missing value for --template, expected tcp or udp".into(),
            (_, "missing_dsl") => "missing value for --dsl, expected a DSL file path".into(),
            (_, "missing_unix_socket") => "missing value for --unix-socket, expected a filesystem path".into(),
            (_, "missing_tcp_socket") => "missing value for --tcp-socket, expected host:port".into(),
            (_, "missing_out") => "missing value for --out, expected a writable file path".into(),
            _ => key.into(),
        }
    }
}

fn main() {
    let locale = UiLocale::detect();
    let cli = Cli::from_args(env::args().skip(1)).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let mut outputs = Vec::new();

    if cli.diagnostics {
        let path = cli.dsl_path.as_deref().unwrap_or_else(|| {
            eprintln!("{}", locale.msg("diagnostics_requires_dsl"));
            std::process::exit(2);
        });
        let report = compile_diagnostics_report_file(path).unwrap_or_else(|err| {
            eprintln!("{}", locale.msgf("binding_diagnostics_failed", &format!("{err:?}"), None));
            std::process::exit(2);
        });
        let rendered = if cli.json {
            render_diagnostics_report(&report, RenderFormat::Json)
        } else {
            render_diagnostics_report(&report, RenderFormat::Text)
        };
        if let Some(path) = cli.out_path.as_deref() {
            fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
                eprintln!("{}", locale.msgf("write_failed", path, Some(&err.to_string())));
                std::process::exit(1);
            });
        } else {
            println!("{rendered}");
        }
        return;
    }

    if let Some(socket_target) = cli.socket_target.as_ref() {
        if cli.serve {
            serve_socket_sessions(&cli, socket_target);
            return;
        }

        let export = match (socket_target, cli.dsl_binding()) {
            (SocketTarget::Unix(path), Some(binding)) => run_unix_socket_session_with_binding(path, binding),
            (SocketTarget::Tcp(addr), Some(binding)) => run_tcp_socket_session_with_binding(addr, binding),
            (SocketTarget::Unix(path), None) => run_unix_socket_session(path, cli.template_mode.template()),
            (SocketTarget::Tcp(addr), None) => run_tcp_socket_session(addr, cli.template_mode.template()),
        }
        .unwrap_or_else(|err| {
            eprintln!("{}", locale.msgf("socket_session_failed", &format!("{err:?}"), None));
            std::process::exit(1);
        });
        outputs.push(("socket_session", export));
    } else {
        if let Some(binding) = cli.dsl_binding() {
            outputs.push(("dsl_demo", run_binding_demo(binding)));
        } else {
            if cli.demo_mode.includes_tcp() {
                let tcp_export = run_session(
                    handshake_debug_template(),
                    vec![
                        FactEnvelope {
                            id: FactId(1),
                            ts: base,
                            cpu: CpuId(0),
                            ifindex: Some(2),
                            session: SessionId(1),
                            fragment_id: "tcp_state_fragment".into(),
                            kind: FactKind::TcpState(TcpStateFact {
                                netns: 1,
                                sk_cookie: 42,
                                saddr: [0; 16],
                                daddr: [0; 16],
                                sport: 42310,
                                dport: 443,
                                family: 2,
                                old: 1,
                                new: 2,
                            }),
                        },
                        FactEnvelope {
                            id: FactId(2),
                            ts: base + Duration::from_millis(10),
                            cpu: CpuId(0),
                            ifindex: Some(2),
                            session: SessionId(1),
                            fragment_id: "tcp_packet_meta_fragment".into(),
                            kind: FactKind::PacketMeta(PacketMetaFact {
                                netns: 1,
                                sk_cookie: Some(42),
                                dir: PacketDir::Egress,
                                local_port: None,
                                remote_port: None,
                                payload_byte0: None,
                                payload_prefix2: None,
                                payload_prefix4: None,
                                payload_byte4: None,
                                l3_proto: 0x0800,
                                l4_proto: 6,
                                tot_len: 60,
                                tcp_flags: 0x02,
                                seq: Some(1),
                                ack: None,
                                window: Some(65535),
                            }),
                        },
                        route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
                    ],
                );

                outputs.push(("tcp_demo", tcp_export));
            }

            if cli.demo_mode.includes_udp() {
                let udp_export = run_session(
                    udp_debug_template(),
                    vec![
                        FactEnvelope {
                            id: FactId(1),
                            ts: base,
                            cpu: CpuId(0),
                            ifindex: Some(3),
                            session: SessionId(2),
                            fragment_id: "udp_packet_meta_fragment".into(),
                            kind: FactKind::PacketMeta(PacketMetaFact {
                                netns: 1,
                                sk_cookie: Some(99),
                                dir: PacketDir::Egress,
                                local_port: None,
                                remote_port: None,
                                payload_byte0: None,
                                payload_prefix2: None,
                                payload_prefix4: None,
                                payload_byte4: None,
                                l3_proto: 0x0800,
                                l4_proto: 17,
                                tot_len: 72,
                                tcp_flags: 0,
                                seq: None,
                                ack: None,
                                window: None,
                            }),
                        },
                        route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
                    ],
                );

                outputs.push(("udp_demo", udp_export));
            }
        }
    }

    let rendered = if cli.http_transactions {
        let transactions = if cli.dsl_path.is_some() {
            let mut composed_exports = Vec::new();
            composed_exports.extend(outputs.iter().map(|(_, export)| export.clone()));
            composed_exports.push(run_binding_demo(
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy")
                    .expect("dns dsl should compile"),
            ));
            composed_exports.push(run_binding_demo(
                compile_file(
                    "/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy",
                )
                .expect("http server dsl should compile"),
            ));
            compose_http_transactions(&composed_exports)
        } else {
            compose_http_transactions(
                &outputs
                    .iter()
                    .map(|(_, export)| export.clone())
                    .collect::<Vec<_>>(),
            )
        };

        if cli.json {
            http_transactions_json(&transactions)
        } else {
            http_transactions_text(&transactions)
        }
    } else if cli.findings {
        outputs
            .into_iter()
            .map(|(name, export)| {
                if cli.json {
                    findings_json(name, &export)
                } else {
                    findings_text(name, &export)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else if cli.json {
        outputs
            .into_iter()
            .map(|(name, export)| {
                if cli.summary_only {
                    summary_json(name, &export)
                } else {
                    export.to_json()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        outputs
            .into_iter()
            .map(|(name, export)| summary_line(name, &export))
            .collect::<Vec<_>>()
            .join("\n")
    };

    if let Some(path) = cli.out_path.as_deref() {
        fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
            eprintln!("{}", locale.msgf("write_failed", path, Some(&err.to_string())));
            std::process::exit(1);
        });
    } else {
        println!("{rendered}");
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    demo_mode: DemoMode,
    template_mode: TemplateMode,
    dsl_path: Option<String>,
    diagnostics: bool,
    findings: bool,
    http_transactions: bool,
    serve: bool,
    max_sessions: Option<usize>,
    json: bool,
    summary_only: bool,
    out_path: Option<String>,
    socket_target: Option<SocketTarget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoMode {
    Tcp,
    Udp,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TemplateMode {
    Tcp,
    Udp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SocketTarget {
    Unix(String),
    Tcp(String),
}

impl DemoMode {
    fn from_str(value: &str) -> Result<Self, String>
    {
        let locale = UiLocale::detect();
        match value {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            "both" => Ok(Self::Both),
            other => Err(locale.msgf("unsupported_demo", other, None)),
        }
    }

    fn includes_tcp(self) -> bool {
        matches!(self, Self::Tcp | Self::Both)
    }

    fn includes_udp(self) -> bool {
        matches!(self, Self::Udp | Self::Both)
    }
}

impl TemplateMode {
    fn from_str(value: &str) -> Result<Self, String> {
        let locale = UiLocale::detect();
        match value {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            other => Err(locale.msgf("unsupported_template", other, None)),
        }
    }

    fn template(self) -> gewyvern::template::Template {
        match self {
            Self::Tcp => handshake_debug_template(),
            Self::Udp => udp_debug_template(),
        }
    }
}

impl Cli {
    fn dsl_binding(&self) -> Option<TemplateBinding> {
        let locale = UiLocale::detect();
        self.dsl_path
            .as_deref()
            .map(|path| compile_file(path).unwrap_or_else(|err| {
                eprintln!("{}", locale.msgf("dsl_compile_failed", &format!("{err:?}"), None));
                std::process::exit(2);
            }))
    }

    fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let locale = UiLocale::detect();
        let mut demo_mode = DemoMode::Both;
        let mut template_mode = TemplateMode::Tcp;
        let mut dsl_path = None;
        let mut diagnostics = false;
        let mut findings = false;
        let mut http_transactions = false;
        let mut serve = false;
        let mut max_sessions = None;
        let mut json = false;
        let mut summary_only = false;
        let mut out_path = None;
        let mut socket_target = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--demo" => {
                    let value = args.next().ok_or_else(|| locale.msgf("missing_demo", "", None))?;
                    demo_mode = DemoMode::from_str(&value)?;
                }
                "--json" => json = true,
                "--serve" => serve = true,
                "--findings" => findings = true,
                "--http-transactions" => http_transactions = true,
                "--max-sessions" => {
                    let value = args.next().ok_or_else(|| locale.msgf("missing_max_sessions", "", None))?;
                    max_sessions = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| locale.msgf("invalid_max_sessions", "", None))?,
                    );
                }
                "--summary-only" => summary_only = true,
                "--template" => {
                    let value = args.next().ok_or_else(|| locale.msgf("missing_template", "", None))?;
                    template_mode = TemplateMode::from_str(&value)?;
                }
                "--dsl" => {
                    dsl_path = Some(args.next().ok_or_else(|| locale.msgf("missing_dsl", "", None))?);
                }
                "--diagnostics" => diagnostics = true,
                "--unix-socket" => {
                    socket_target = Some(SocketTarget::Unix(args.next().ok_or_else(|| locale.msgf("missing_unix_socket", "", None))?));
                }
                "--tcp-socket" => {
                    socket_target = Some(SocketTarget::Tcp(args.next().ok_or_else(|| locale.msgf("missing_tcp_socket", "", None))?));
                }
                "--out" => {
                    out_path = Some(args.next().ok_or_else(|| locale.msgf("missing_out", "", None))?);
                }
                "--help" | "-h" => return Err(usage().into()),
                other => return Err(locale.msgf("unknown_argument", other, None)),
            }
        }

        if summary_only && !json {
            return Err(locale.msg("summary_only_requires_json").into());
        }
        if diagnostics && dsl_path.is_none() {
            return Err(locale.msg("diagnostics_requires_dsl").into());
        }
        if diagnostics && socket_target.is_some() {
            return Err(locale.msg("diagnostics_socket_conflict").into());
        }
        if diagnostics && serve {
            return Err(locale.msg("diagnostics_serve_conflict").into());
        }
        if diagnostics && findings {
            return Err(locale.msg("findings_diagnostics_conflict").into());
        }
        if diagnostics && http_transactions {
            return Err(locale.msg("findings_diagnostics_conflict").into());
        }
        if dsl_path.is_some() && demo_mode != DemoMode::Both {
            return Err(locale.msg("dsl_demo_conflict").into());
        }
        if socket_target.is_some() && demo_mode != DemoMode::Both {
            return Err(locale.msg("demo_socket_conflict").into());
        }
        if serve && socket_target.is_none() {
            return Err(locale.msg("serve_requires_socket").into());
        }

        Ok(Self {
            demo_mode,
            template_mode,
            dsl_path,
            diagnostics,
            findings,
            http_transactions,
            serve,
            max_sessions,
            json,
            summary_only,
            out_path,
            socket_target,
        })
    }
}

fn run_session(
    template: gewyvern::template::Template,
    facts: Vec<FactEnvelope>,
) -> ExportBundle {
    let config = SessionConfig::for_template(template).expect("builtin template should be valid");
    let mut session = RuntimeSession::start(config).expect("session startup should succeed");
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);

    let export = session.export_bundle();
    let replay = ExportBundle::from_json(&export.to_json())
        .expect("runtime should export replayable json")
        .replay()
        .expect("export should replay");

    assert_eq!(export.reasons, replay.reasons, "replay should stay deterministic");
    export
}

fn run_binding_demo(binding: TemplateBinding) -> ExportBundle {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let fragments = &binding.template.fragment_set;
    let tcp_demo_dport = binding
        .template
        .program_model
        .as_ref()
        .map(|model| match &model.operation {
            gewyvern::flow::ProgramOperation::Custom(value) if value == "postgres_connect" => 5432,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "redis_connect" => 6379,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "mysql_connect" => 3306,
            _ => 443,
        })
        .unwrap_or(443);
    let is_dns_lookup = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| matches!(
            &model.operation,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "dns_lookup"
        ));
    let is_http_request = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| matches!(
            &model.operation,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "http_request"
        ));
    let is_tls_client = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| matches!(
            &model.operation,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "tls_client"
        ));
    let is_http_server_response = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| matches!(
            &model.operation,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "http_server_response"
        ));
    let facts = if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
        && is_http_server_response
    {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "sock_lineage_fragment".into(),
                kind: FactKind::SockLineage(SockLineageFact {
                    netns: 1,
                    sk_cookie: 77,
                    pid: 8080,
                    tid: 8080,
                    cgroup_id: 8080,
                    comm: {
                        let mut comm = [0u8; 16];
                        comm[..5].copy_from_slice(b"nginx");
                        comm
                    },
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 77,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 80,
                    dport: 53000,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(3),
                ts: base + Duration::from_millis(20),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 77,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 80,
                    dport: 53000,
                    family: 2,
                    old: 2,
                    new: 3,
                }),
            },
            FactEnvelope {
                id: FactId(4),
                ts: base + Duration::from_millis(30),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(77),
                    dir: PacketDir::Ingress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 140,
                    tcp_flags: 0x18,
                    seq: Some(1),
                    ack: Some(1),
                    window: Some(65535),
                }),
            },
            FactEnvelope {
                id: FactId(5),
                ts: base + Duration::from_millis(40),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(77),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 220,
                    tcp_flags: 0x18,
                    seq: Some(2),
                    ack: Some(2),
                    window: Some(65535),
                }),
            },
        ]
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
        && is_http_request
    {
        let mut facts = vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "sock_lineage_fragment".into(),
                kind: FactKind::SockLineage(SockLineageFact {
                    netns: 1,
                    sk_cookie: 99,
                    pid: 4242,
                    tid: 4242,
                    cgroup_id: 4242,
                    comm: {
                        let mut comm = [0u8; 16];
                        comm[..4].copy_from_slice(b"curl");
                        comm
                    },
                }),
            },
        ];
        if fragments.contains(&"route_meta_fragment") {
            facts.push(route_fact(2, base + Duration::from_millis(10), 99, 2, SessionId(2)));
        }
        let offset = facts.len() as u64 + 1;
        facts.extend([
            FactEnvelope {
                id: FactId(offset),
                ts: base + Duration::from_millis(20),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 99,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: 443,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 1),
                ts: base + Duration::from_millis(30),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 99,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: 443,
                    family: 2,
                    old: 2,
                    new: 3,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 2),
                ts: base + Duration::from_millis(40),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 120,
                    tcp_flags: 0x18,
                    seq: Some(1),
                    ack: Some(1),
                    window: Some(65535),
                }),
            },
            FactEnvelope {
                id: FactId(offset + 3),
                ts: base + Duration::from_millis(50),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Ingress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 180,
                    tcp_flags: 0x18,
                    seq: Some(2),
                    ack: Some(2),
                    window: Some(65535),
                }),
            },
        ]);
        facts
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
        && is_tls_client
    {
        let mut facts = vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "sock_lineage_fragment".into(),
                kind: FactKind::SockLineage(SockLineageFact {
                    netns: 1,
                    sk_cookie: 88,
                    pid: 4242,
                    tid: 4242,
                    cgroup_id: 4242,
                    comm: {
                        let mut comm = [0u8; 16];
                        comm[..4].copy_from_slice(b"curl");
                        comm
                    },
                }),
            },
        ];
        if fragments.contains(&"route_meta_fragment") {
            facts.push(route_fact(2, base + Duration::from_millis(10), 88, 2, SessionId(2)));
        }
        let offset = facts.len() as u64 + 1;
        facts.extend([
            FactEnvelope {
                id: FactId(offset),
                ts: base + Duration::from_millis(20),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 88,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 1),
                ts: base + Duration::from_millis(30),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 88,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 2,
                    new: 3,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 2),
                ts: base + Duration::from_millis(40),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(88),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 96,
                    tcp_flags: 0x18,
                    seq: Some(1),
                    ack: Some(1),
                    window: Some(65535),
                }),
            },
        ]);
        facts
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
    {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 42,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(42),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 60,
                    tcp_flags: 0x02,
                    seq: Some(1),
                    ack: None,
                    window: Some(65535),
                }),
            },
            route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
        ]
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"sock_lineage_fragment")
    {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "sock_lineage_fragment".into(),
                kind: FactKind::SockLineage(SockLineageFact {
                    netns: 1,
                    sk_cookie: 42,
                    pid: 4242,
                    tid: 4242,
                    cgroup_id: 4242,
                    comm: {
                        let mut comm = [0u8; 16];
                        comm[..4].copy_from_slice(b"curl");
                        comm
                    },
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 42,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
        ]
    } else if fragments.contains(&"udp_packet_meta_fragment") && fragments.contains(&"sock_lineage_fragment") {
        if is_dns_lookup {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 99,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..4].copy_from_slice(b"curl");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: None,
                        remote_port: None,
                        payload_byte0: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 72,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Ingress,
                        local_port: None,
                        remote_port: None,
                        payload_byte0: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 96,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
            ]
        } else {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 99,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..4].copy_from_slice(b"curl");
                            comm
                        },
                    }),
                },
                FactEnvelope {
                    id: FactId(2),
                    ts: base + Duration::from_millis(10),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: None,
                        remote_port: None,
                        payload_byte0: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 72,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                route_fact(3, base + Duration::from_millis(20), 99, 3, SessionId(2)),
            ]
        }
    } else if fragments.contains(&"udp_packet_meta_fragment") {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(3),
                session: SessionId(2),
                fragment_id: "udp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    l3_proto: 0x0800,
                    l4_proto: 17,
                    tot_len: 72,
                    tcp_flags: 0,
                    seq: None,
                    ack: None,
                    window: None,
                }),
            },
            route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
        ]
    } else {
        eprintln!("{}", UiLocale::detect().msg("unsupported_fragment_combo"));
        std::process::exit(2);
    };

    let config = SessionConfig::for_binding(binding).expect("dsl binding should validate");
    let mut session = RuntimeSession::start(config).expect("dsl session startup should succeed");
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);

    let export = session.export_bundle();
    let replay = ExportBundle::from_json(&export.to_json())
        .expect("runtime should export replayable json")
        .replay()
        .expect("export should replay");

    assert_eq!(export.reasons, replay.reasons, "replay should stay deterministic");
    export
}

#[cfg(test)]
mod tests {
    use super::run_binding_demo;
    use gewyvern::dsl::compile_file;
    use gewyvern::flow::ProgramOperation;

    #[test]
    fn http_request_demo_produces_healthy_cross_transport_path() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                .expect("http_request_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.debug_summary.accepted_facts, 6);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert_eq!(bundle.program_flows.len(), 1);
        assert!(bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request")));
        assert!(bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response")));
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("http_request".into())
        );
    }

    #[test]
    fn tls_client_demo_produces_healthy_packet_phase() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy")
            .expect("tls_client_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert!(bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_client_hello")));
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("tls_client".into())
        );
    }

    #[test]
    fn http_server_response_demo_produces_healthy_server_path() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy")
                .expect("http_server_response_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert!(bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_request")));
        assert!(bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_response")));
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("http_server_response".into())
        );
    }
}

fn route_fact(id: u64, ts: SystemTime, cookie: u64, oif: u32, session: SessionId) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts,
        cpu: CpuId(1),
        ifindex: Some(oif),
        session,
        fragment_id: "route_meta_fragment".into(),
        kind: FactKind::RouteDecision(RouteDecisionFact {
            netns: 1,
            sk_cookie: Some(cookie),
            fib_table: Some(254),
            oif,
            gw: None,
        }),
    }
}

fn summary_line(name: &str, export: &ExportBundle) -> String {
    let locale = UiLocale::detect();
    let suspect_areas = if export.program_findings.is_empty() {
        locale.none().to_string()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>()
            .join(",")
    };
    let suspect_modules = if export.program_findings.is_empty() {
        locale.none().to_string()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.module_label.clone())
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "{name}: {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={}",
        locale.label("template"),
        export.template_id,
        locale.label("fragments_loaded"),
        export.debug_summary.fragments_loaded,
        locale.label("hookpoints_failed"),
        export.debug_summary.hookpoints_failed,
        locale.label("accepted_facts"),
        export.debug_summary.accepted_facts,
        locale.label("rejected_facts"),
        export.debug_summary.rejected_facts,
        locale.label("flows"),
        export.debug_summary.flows,
        locale.label("program_findings"),
        export.debug_summary.program_findings,
        locale.label("module_findings"),
        export.debug_summary.module_findings,
        locale.label("reasons"),
        export.debug_summary.reasons,
        locale.label("degraded"),
        export.debug_summary.degraded,
        locale.label("suspect_areas"),
        suspect_areas,
        locale.label("suspect_modules"),
        suspect_modules,
    )
}

fn usage() -> &'static str {
    UiLocale::detect().usage()
}

fn summary_json(name: &str, export: &ExportBundle) -> String {
    let suspect_modules = format!(
        "[{}]",
        export
            .program_findings
            .iter()
            .map(|finding| format!("\"{}\"", finding.module_label))
            .collect::<Vec<_>>()
            .join(",")
    );
    format!(
        "{{\"demo\":\"{name}\",\"template_id\":\"{}\",\"fragments_loaded\":{},\"hookpoints_failed\":{},\"accepted_facts\":{},\"rejected_facts\":{},\"flows\":{},\"program_findings\":{},\"module_findings\":{},\"reasons\":{},\"degraded\":{},\"suspect_modules\":{}}}",
        export.template_id,
        export.debug_summary.fragments_loaded,
        export.debug_summary.hookpoints_failed,
        export.debug_summary.accepted_facts,
        export.debug_summary.rejected_facts,
        export.debug_summary.flows,
        export.debug_summary.program_findings,
        export.debug_summary.module_findings,
        export.debug_summary.reasons,
        export.debug_summary.degraded,
        suspect_modules,
    )
}

fn findings_text(name: &str, export: &ExportBundle) -> String {
    let locale = UiLocale::detect();
    if export.module_findings.is_empty() {
        return format!("{name}: {}", locale.none());
    }

    let mut lines = vec![format!("{name}:")];
    for finding in &export.module_findings {
        let process = finding
            .process
            .as_ref()
            .map(|process| format!("{}(pid={})", process.comm, process.pid))
            .unwrap_or_else(|| locale.none().to_string());
        let traces = if finding.evidence_trace.is_empty() {
            locale.none().to_string()
        } else {
            finding.evidence_trace.join("|")
        };
        let phases = if finding.phases.is_empty() {
            locale.none().to_string()
        } else {
            finding.phases.join(",")
        };
        let transitions = if finding.phase_transitions.is_empty() {
            locale.none().to_string()
        } else {
            finding.phase_transitions.join(",")
        };
        let summaries = if finding.summaries.is_empty() {
            locale.none().to_string()
        } else {
            finding.summaries.join("|")
        };
        lines.push(format!(
            "  {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={}",
            locale.label("severity"),
            module_severity_label(&finding.severity),
            locale.label("module"),
            finding.module_label,
            locale.label("phases"),
            phases,
            locale.label("phase_transitions"),
            transitions,
            locale.label("process"),
            process,
            locale.label("operation"),
            operation_label(&finding.operation),
            locale.label("suspect_areas"),
            finding.suspect_areas.join(","),
            locale.label("causes"),
            finding
                .causes
                .iter()
                .map(finding_cause_label)
                .collect::<Vec<_>>()
                .join(","),
            locale.label("supporting"),
            finding.supporting_fragments.join(","),
            locale.label("trace"),
            traces,
        ));
        lines.push(format!("  {}={}", locale.label("summary"), summaries));
    }

    lines.join("\n")
}

fn findings_json(name: &str, export: &ExportBundle) -> String {
    format!(
        "{{\"demo\":\"{name}\",\"template_id\":\"{}\",\"module_findings\":[{}],\"program_findings\":[{}]}}",
        export.template_id,
        export
            .module_findings
            .iter()
            .map(module_finding_json)
            .collect::<Vec<_>>()
            .join(","),
        export
            .program_findings
            .iter()
            .map(program_finding_json)
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn http_transactions_text(transactions: &[HttpTransactionView]) -> String {
    let locale = UiLocale::detect();
    if transactions.is_empty() {
        return locale.none().into();
    }

    transactions
        .iter()
        .map(|tx| {
            format!(
                "http_transaction#{}: client={} server={} verdict={} severity={} degraded={} suspect_sides={} phases={} components={} summaries={}",
                tx.id.0,
                tx.client_process
                    .as_ref()
                    .map(|p| format!("{}(pid={})", p.comm, p.pid))
                    .unwrap_or_else(|| locale.none().to_string()),
                tx.server_process
                    .as_ref()
                    .map(|p| format!("{}(pid={})", p.comm, p.pid))
                    .unwrap_or_else(|| locale.none().to_string()),
                http_transaction_verdict_label(&tx.verdict),
                tx.severity
                    .as_ref()
                    .map(module_severity_label)
                    .unwrap_or_else(|| locale.none()),
                tx.degraded,
                if tx.suspect_sides.is_empty() {
                    locale.none().to_string()
                } else {
                    tx.suspect_sides
                        .iter()
                        .map(http_suspect_side_label)
                        .collect::<Vec<_>>()
                        .join(",")
                },
                if tx.phases.is_empty() {
                    locale.none().to_string()
                } else {
                    tx.phases.join(",")
                },
                tx.components
                    .iter()
                    .map(|component| format!("{}:{}", http_component_kind_label(&component.kind), operation_label(&component.operation)))
                    .collect::<Vec<_>>()
                    .join(","),
                if tx.finding_summaries.is_empty() {
                    tx.summaries.join("|")
                } else {
                    tx.finding_summaries.join("|")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn http_transactions_json(transactions: &[HttpTransactionView]) -> String {
    format!(
        "[{}]",
        transactions
            .iter()
            .map(http_transaction_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn http_transaction_json(transaction: &HttpTransactionView) -> String {
    format!(
        "{{\"id\":{},\"client_process\":{},\"server_process\":{},\"verdict\":\"{}\",\"severity\":{},\"degraded\":{},\"suspect_sides\":{},\"phases\":{},\"components\":{},\"finding_summaries\":{},\"summaries\":{}}}",
        transaction.id.0,
        process_json(transaction.client_process.as_ref()),
        process_json(transaction.server_process.as_ref()),
        http_transaction_verdict_label(&transaction.verdict),
        transaction
            .severity
            .as_ref()
            .map(|severity| format!("\"{}\"", module_severity_label(severity)))
            .unwrap_or_else(|| "null".into()),
        transaction.degraded,
        string_list_json(
            &transaction
                .suspect_sides
                .iter()
                .map(|side| http_suspect_side_label(side).to_string())
                .collect::<Vec<_>>()
        ),
        string_list_json(&transaction.phases),
        format!(
            "[{}]",
            transaction
                .components
                .iter()
                .map(http_component_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        string_list_json(&transaction.finding_summaries),
        string_list_json(&transaction.summaries),
    )
}

fn http_component_json(component: &gewyvern::http::HttpComponentRef) -> String {
    format!(
        "{{\"template_id\":\"{}\",\"kind\":\"{}\",\"operation\":\"{}\"}}",
        component.template_id,
        http_component_kind_label(&component.kind),
        operation_label(&component.operation),
    )
}

fn module_finding_json(finding: &gewyvern::flow::ModuleFinding) -> String {
    format!(
        "{{\"module_label\":\"{}\",\"severity\":\"{}\",\"process\":{},\"operation\":\"{}\",\"phases\":{},\"phase_transitions\":{},\"suspect_areas\":{},\"causes\":{},\"supporting_fragments\":{},\"program_flows\":{},\"summaries\":{},\"evidence_trace\":{}}}",
        finding.module_label,
        module_severity_label(&finding.severity),
        process_json(finding.process.as_ref()),
        operation_label(&finding.operation),
        string_list_json(&finding.phases),
        string_list_json(&finding.phase_transitions),
        string_list_json(&finding.suspect_areas),
        string_list_json(
            &finding
                .causes
                .iter()
                .map(finding_cause_label)
                .map(str::to_string)
                .collect::<Vec<_>>()
        ),
        string_list_json(&finding.supporting_fragments),
        format!(
            "[{}]",
            finding
                .program_flows
                .iter()
                .map(|flow| flow.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        ),
        string_list_json(&finding.summaries),
        string_list_json(&finding.evidence_trace),
    )
}

fn program_finding_json(finding: &gewyvern::flow::ProgramFinding) -> String {
    format!(
        "{{\"program_flow\":{},\"module_label\":\"{}\",\"phase\":{},\"phase_transition\":{},\"suspect_area\":\"{}\",\"cause\":\"{}\",\"process\":{},\"operation\":\"{}\",\"summary\":\"{}\",\"supporting_fragments\":{},\"evidence_trace\":{}}}",
        finding.program_flow.0,
        finding.module_label,
        finding.phase.as_ref().map_or("null".to_string(), |phase| format!("\"{}\"", phase)),
        finding.phase_transition.as_ref().map_or("null".to_string(), |transition| format!("\"{}\"", transition)),
        finding.suspect_area,
        finding_cause_label(&finding.cause),
        process_json(finding.process.as_ref()),
        operation_label(&finding.operation),
        finding.summary,
        string_list_json(&finding.supporting_fragments),
        string_list_json(&finding.evidence_trace),
    )
}

fn http_component_kind_label(kind: &gewyvern::http::HttpComponentKind) -> &'static str {
    match kind {
        gewyvern::http::HttpComponentKind::DnsLookup => "dns",
        gewyvern::http::HttpComponentKind::ClientRequest => "client",
        gewyvern::http::HttpComponentKind::ServerResponse => "server",
    }
}

fn http_suspect_side_label(side: &HttpSuspectSide) -> &'static str {
    match side {
        HttpSuspectSide::Dns => "dns",
        HttpSuspectSide::Client => "client",
        HttpSuspectSide::Server => "server",
    }
}

fn http_transaction_verdict_label(
    verdict: &gewyvern::http::HttpTransactionVerdict,
) -> &'static str {
    match verdict {
        gewyvern::http::HttpTransactionVerdict::HealthyRequestResponsePath => {
            "healthy_request_response_path"
        }
        gewyvern::http::HttpTransactionVerdict::SuspectDnsResolutionGap => {
            "suspect_dns_resolution_gap"
        }
        gewyvern::http::HttpTransactionVerdict::SuspectClientResponseGap => {
            "suspect_client_response_gap"
        }
        gewyvern::http::HttpTransactionVerdict::SuspectServerResponseGap => {
            "suspect_server_response_gap"
        }
        gewyvern::http::HttpTransactionVerdict::SuspectMultiSidedGap => {
            "suspect_multi_sided_gap"
        }
    }
}

fn process_json(process: Option<&gewyvern::flow::ProcessView>) -> String {
    match process {
        Some(process) => format!(
            "{{\"pid\":{},\"tid\":{},\"cgroup_id\":{},\"comm\":\"{}\"}}",
            process.pid, process.tid, process.cgroup_id, process.comm
        ),
        None => "null".into(),
    }
}

fn string_list_json(items: &[String]) -> String {
    format!(
        "[{}]",
        items.iter()
            .map(|item| format!("\"{}\"", item))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn operation_label(operation: &gewyvern::flow::ProgramOperation) -> String {
    match operation {
        gewyvern::flow::ProgramOperation::ConnectFlow => "connect_flow".into(),
        gewyvern::flow::ProgramOperation::DatagramExchange => "datagram_exchange".into(),
        gewyvern::flow::ProgramOperation::Custom(value) => value.clone(),
        gewyvern::flow::ProgramOperation::Unknown => "unknown".into(),
    }
}

fn finding_cause_label(cause: &gewyvern::flow::ProgramFindingCause) -> &'static str {
    match cause {
        gewyvern::flow::ProgramFindingCause::AttachFailure => "attach_failure",
        gewyvern::flow::ProgramFindingCause::RejectedEvidence => "rejected_evidence",
        gewyvern::flow::ProgramFindingCause::MissingCoreStage => "missing_core_stage",
    }
}

fn module_severity_label(severity: &gewyvern::flow::ModuleSeverity) -> &'static str {
    match severity {
        gewyvern::flow::ModuleSeverity::High => "high",
        gewyvern::flow::ModuleSeverity::Medium => "medium",
        gewyvern::flow::ModuleSeverity::Low => "low",
    }
}

fn serve_socket_sessions(cli: &Cli, socket_target: &SocketTarget) {
    match socket_target {
        SocketTarget::Unix(path) => serve_unix_socket_sessions(cli, path),
        SocketTarget::Tcp(addr) => serve_tcp_socket_sessions(cli, addr),
    }
}

fn serve_unix_socket_sessions(cli: &Cli, path: &str) {
    let locale = UiLocale::detect();
    #[cfg(target_family = "unix")]
    {
        let _ = fs::remove_file(path);
        let listener = bind_unix_socket_listener(path).unwrap_or_else(|err| {
            eprintln!("{}", locale.msgf("socket_service_failed", &format!("{err:?}"), None));
            std::process::exit(1);
        });
        let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

        for _ in 0..max_sessions {
            let export = if let Some(binding) = cli.dsl_binding() {
                run_unix_socket_session_on_listener_with_binding(&listener, binding)
            } else {
                run_unix_socket_session_on_listener(&listener, cli.template_mode.template())
            }
                .unwrap_or_else(|err| {
                    eprintln!("{}", locale.msgf("socket_service_failed", &format!("{err:?}"), None));
                    std::process::exit(1);
                });
            emit_rendered(cli, "socket_session", &export, true);
        }

        let _ = fs::remove_file(path);
        return;
    }

    #[cfg(not(target_family = "unix"))]
    {
        let _ = path;
        eprintln!("{}", locale.msg("unix_only"));
        std::process::exit(1);
    }
}

fn serve_tcp_socket_sessions(cli: &Cli, addr: &str) {
    let locale = UiLocale::detect();
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        eprintln!("{}", locale.msgf("socket_service_failed", &err.to_string(), None));
        std::process::exit(1);
    });
    let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

    for _ in 0..max_sessions {
        let export = if let Some(binding) = cli.dsl_binding() {
            run_tcp_socket_session_on_listener_with_binding(&listener, binding)
        } else {
            run_tcp_socket_session_on_listener(&listener, cli.template_mode.template())
        }
            .unwrap_or_else(|err| {
                eprintln!("{}", locale.msgf("socket_service_failed", &format!("{err:?}"), None));
                std::process::exit(1);
            });
        emit_rendered(cli, "socket_session", &export, true);
    }
}

fn emit_rendered(cli: &Cli, name: &str, export: &ExportBundle, append: bool) {
    let locale = UiLocale::detect();
    let rendered = if cli.findings {
        if cli.json {
            findings_json(name, export)
        } else {
            findings_text(name, export)
        }
    } else if cli.json {
        if cli.summary_only {
            summary_json(name, export)
        } else {
            export.to_json()
        }
    } else {
        summary_line(name, export)
    };

    if let Some(path) = cli.out_path.as_deref() {
        if append {
            let mut existing = fs::read_to_string(path).unwrap_or_default();
            existing.push_str(&rendered);
            existing.push('\n');
            fs::write(path, existing).unwrap_or_else(|err| {
                eprintln!("{}", locale.msgf("write_failed", path, Some(&err.to_string())));
                std::process::exit(1);
            });
        } else {
            fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
                eprintln!("{}", locale.msgf("write_failed", path, Some(&err.to_string())));
                std::process::exit(1);
            });
        }
    } else {
        println!("{rendered}");
    }
}
