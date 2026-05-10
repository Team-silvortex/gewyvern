use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::{FlowId, ProcessView, ProgramFlowId};
use gewyvern::gewyc::{RenderFormat, compile_diagnostics_report_file, render_diagnostics_report};
use gewyvern::http::{HttpSuspectSide, HttpTransactionView, compose_http_transactions};
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, QuicFrameType,
    QuicPacketType, RouteDecisionFact, SessionId, SockLineageFact, TcpStateFact,
};
use gewyvern::protocol_profiles::{
    ResolvedProtocolProfile, default_protocol_scan_set, default_protocol_scan_set_from_dir,
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_names,
    resolve_protocol_profile,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::socket_input::{
    bind_unix_socket_listener, collect_tcp_socket_facts, collect_tcp_socket_facts_on_listener,
    collect_unix_socket_facts, collect_unix_socket_facts_on_listener, remove_unix_socket_file,
    run_tcp_socket_session, run_tcp_socket_session_on_listener,
    run_tcp_socket_session_on_listener_with_binding, run_tcp_socket_session_with_binding,
    run_unix_socket_session, run_unix_socket_session_on_listener,
    run_unix_socket_session_on_listener_with_binding, run_unix_socket_session_with_binding,
};
use gewyvern::template::{TemplateBinding, handshake_debug_template, udp_debug_template};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::net::{TcpListener, ToSocketAddrs};
use std::path::Path;
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
            Self::Zh => {
                "用法: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::Ja => {
                "使い方: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::Ko => {
                "사용법: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::Fr => {
                "Utilisation : gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::De => {
                "Verwendung: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::Es => {
                "Uso: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::Pt => {
                "Uso: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::Ru => {
                "Использование: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
            Self::En => {
                "usage: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
            }
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
                "远程 TCP 监听需要显式加上 --socket-trust unsafe-remote（或兼容的 --allow-remote-socket）"
            }
            (Self::Zh, "serve_requires_socket") => "--serve 需要 --unix-socket 或 --tcp-socket",
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
            (_, "list_conflict") => "--list-protocols cannot be combined with --list-entries",
            (_, "remote_socket_requires_flag") => {
                "remote TCP listeners require explicit --socket-trust unsafe-remote (or legacy --allow-remote-socket)"
            }
            (_, "serve_requires_socket") => "--serve requires --unix-socket or --tcp-socket",
            (_, "unsupported_fragment_combo") => "unsupported fragment combination",
            (_, "unix_only") => "unix socket service is only supported on unix platforms",
            (_, "findings_diagnostics_conflict") => {
                "--findings cannot be combined with --diagnostics"
            }
            _ => key,
        }
    }

    fn msgf(self, key: &'static str, a: &str, b: Option<&str>) -> String {
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
            (Self::Zh, "missing_unix_socket") => "缺少 --unix-socket 的值，期望文件路径".into(),
            (Self::Zh, "missing_tcp_socket") => "缺少 --tcp-socket 的值，期望 host:port".into(),
            (Self::Zh, "missing_socket_trust") => {
                "缺少 --socket-trust 的值，期望 trusted-local 或 unsafe-remote".into()
            }
            (Self::Zh, "missing_out") => "缺少 --out 的值，期望可写文件路径".into(),
            (Self::Zh, "unsupported_protocol") => format!("不支持的协议 '{a}'"),
            (Self::Zh, "unsupported_socket_trust") => {
                format!("不支持的 socket 信任模式 '{a}'，期望 trusted-local 或 unsafe-remote")
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
            (_, "missing_socket_trust") => {
                "missing value for --socket-trust, expected trusted-local or unsafe-remote".into()
            }
            (_, "missing_out") => "missing value for --out, expected a writable file path".into(),
            (_, "unsupported_protocol") => format!("unsupported protocol '{a}'"),
            (_, "unsupported_socket_trust") => format!(
                "unsupported socket trust mode '{a}', expected trusted-local or unsafe-remote"
            ),
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

    if cli.list_protocols {
        let rendered = if cli.json {
            list_protocols_json()
        } else {
            list_protocols_text()
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return;
    }

    if let Some(protocol) = cli.list_entries.as_deref() {
        let rendered = if cli.json {
            list_entries_json(protocol).unwrap_or_else(|| {
                eprintln!("{}", locale.msgf("unsupported_protocol", protocol, None));
                std::process::exit(2);
            })
        } else {
            list_entries_text(protocol).unwrap_or_else(|| {
                eprintln!("{}", locale.msgf("unsupported_protocol", protocol, None));
                std::process::exit(2);
            })
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return;
    }

    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let scan_targets = scan_targets_for_cli(&cli).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    let mut outputs: Vec<(String, ExportBundle)> = Vec::new();

    if cli.diagnostics {
        let path = cli.dsl_path.as_deref().unwrap_or_else(|| {
            eprintln!("{}", locale.msg("diagnostics_requires_dsl"));
            std::process::exit(2);
        });
        let report = compile_diagnostics_report_file(path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("binding_diagnostics_failed", &format!("{err:?}"), None)
            );
            std::process::exit(2);
        });
        let rendered = if cli.json {
            render_diagnostics_report(&report, RenderFormat::Json)
        } else {
            render_diagnostics_report(&report, RenderFormat::Text)
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return;
    }

    if let Some(socket_target) = cli.socket_target.as_ref() {
        if cli.serve {
            serve_socket_sessions(&cli, socket_target);
            return;
        }

        if cli.scan_all {
            let facts = match socket_target {
                SocketTarget::Unix(path) => collect_unix_socket_facts(path),
                SocketTarget::Tcp(addr) => collect_tcp_socket_facts(addr),
            }
            .unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("socket_session_failed", &format!("{err:?}"), None)
                );
                std::process::exit(1);
            });
            for target in &scan_targets {
                let export = run_binding_session(target.binding(), &facts);
                let export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&export, pid))
                    .unwrap_or(export);
                outputs.push((target.label(), annotate_export_trust(export, &cli)));
            }
        } else {
            let export = match (socket_target, cli.dsl_binding()) {
                (SocketTarget::Unix(path), Some(binding)) => {
                    run_unix_socket_session_with_binding(path, binding)
                }
                (SocketTarget::Tcp(addr), Some(binding)) => {
                    run_tcp_socket_session_with_binding(addr, binding)
                }
                (SocketTarget::Unix(path), None) => {
                    run_unix_socket_session(path, cli.template_mode.template())
                }
                (SocketTarget::Tcp(addr), None) => {
                    run_tcp_socket_session(addr, cli.template_mode.template())
                }
            }
            .unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("socket_session_failed", &format!("{err:?}"), None)
                );
                std::process::exit(1);
            });
            let export = cli
                .pid
                .map(|pid| filter_export_by_pid(&export, pid))
                .unwrap_or(export);
            outputs.push((
                "socket_session".to_string(),
                annotate_export_trust(export, &cli),
            ));
        }
    } else {
        if cli.scan_all {
            for target in &scan_targets {
                let export = run_binding_demo(target.binding());
                let export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&export, pid))
                    .unwrap_or(export);
                outputs.push((target.label(), annotate_export_trust(export, &cli)));
            }
        } else if let Some(binding) = cli.dsl_binding() {
            let export = run_binding_demo(binding);
            let export = cli
                .pid
                .map(|pid| filter_export_by_pid(&export, pid))
                .unwrap_or(export);
            outputs.push(("dsl_demo".to_string(), annotate_export_trust(export, &cli)));
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
                                payload_byte1: None,
                                payload_prefix2: None,
                                payload_prefix4: None,
                                payload_byte4: None,
                                payload_byte5: None,
                                payload_byte9: None,
                                payload_byte10: None,
                                payload_byte13: None,
                                payload_bytes: std::collections::BTreeMap::new(),
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

                let tcp_export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&tcp_export, pid))
                    .unwrap_or(tcp_export);
                outputs.push((
                    "tcp_demo".to_string(),
                    annotate_export_trust(tcp_export, &cli),
                ));
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
                                payload_byte1: None,
                                payload_prefix2: None,
                                payload_prefix4: None,
                                payload_byte4: None,
                                payload_byte5: None,
                                payload_byte9: None,
                                payload_byte10: None,
                                payload_byte13: None,
                                payload_bytes: std::collections::BTreeMap::new(),
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

                let udp_export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&udp_export, pid))
                    .unwrap_or(udp_export);
                outputs.push((
                    "udp_demo".to_string(),
                    annotate_export_trust(udp_export, &cli),
                ));
            }
        }
    }

    let rendered = if cli.http_transactions {
        let transactions = if cli.dsl_path.is_some() {
            let mut composed_exports = Vec::new();
            composed_exports.extend(outputs.iter().map(|(_, export)| export.clone()));
            if outputs
                .iter()
                .any(|(_, export)| export_has_operation(export, "http_request"))
            {
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
            }
            if outputs
                .iter()
                .any(|(_, export)| export_has_operation(export, "http3_request"))
            {
                composed_exports.push(run_binding_demo(
                    compile_file(
                        "/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy",
                    )
                    .expect("http3 server dsl should compile"),
                ));
            }
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
        if cli.scan_all {
            render_scan_outputs(&cli, &outputs)
        } else if cli.report_format.is_some() {
            render_report_outputs(&cli, &outputs)
        } else {
            outputs
                .into_iter()
                .map(|(name, export)| {
                    if cli.json {
                        findings_json(&name, &export)
                    } else {
                        findings_text(&name, &export)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else if cli.json {
        if cli.scan_all && cli.summary_only {
            render_scan_outputs(&cli, &outputs)
        } else if cli.report_format.is_some() {
            render_report_outputs(&cli, &outputs)
        } else {
            outputs
                .into_iter()
                .map(|(name, export)| {
                    if cli.summary_only {
                        summary_json(&name, &export)
                    } else {
                        export.to_json()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        if cli.scan_all {
            render_scan_outputs(&cli, &outputs)
        } else if cli.report_format.is_some() {
            render_report_outputs(&cli, &outputs)
        } else {
            outputs
                .into_iter()
                .map(|(name, export)| summary_line(&name, &export))
                .collect::<Vec<_>>()
                .join("\n")
        }
    };

    if let Some(path) = cli.out_path.as_deref() {
        fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("write_failed", path, Some(&err.to_string()))
            );
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
    protocol: Option<String>,
    entry: Option<String>,
    scan_all: bool,
    protocol_set_path: Option<String>,
    list_protocols: bool,
    list_entries: Option<String>,
    socket_trust: SocketTrustMode,
    pid: Option<u32>,
    diagnostics: bool,
    findings: bool,
    http_transactions: bool,
    serve: bool,
    max_sessions: Option<usize>,
    json: bool,
    report_format: Option<ReportFormat>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReportFormat {
    Json,
    Html,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SocketTrustMode {
    TrustedLocal,
    UnsafeRemote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SocketTarget {
    Unix(String),
    Tcp(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScanTarget {
    protocol: String,
    entry: String,
    dsl_path: String,
}

impl ScanTarget {
    fn from_resolved(profile: ResolvedProtocolProfile) -> Self {
        Self {
            protocol: profile.protocol.to_string(),
            entry: profile.entry.to_string(),
            dsl_path: profile.dsl_path.to_string(),
        }
    }

    fn label(&self) -> String {
        format!("scan:{}:{}", self.protocol, self.entry)
    }

    fn binding(&self) -> TemplateBinding {
        let locale = UiLocale::detect();
        compile_file(&self.dsl_path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("dsl_compile_failed", &format!("{err:?}"), None)
            );
            std::process::exit(2);
        })
    }
}

impl DemoMode {
    fn from_str(value: &str) -> Result<Self, String> {
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

impl SocketTrustMode {
    fn from_str(value: &str) -> Result<Self, String> {
        let locale = UiLocale::detect();
        match value {
            "trusted-local" | "local" => Ok(Self::TrustedLocal),
            "unsafe-remote" | "remote" => Ok(Self::UnsafeRemote),
            other => Err(locale.msgf("unsupported_socket_trust", other, None)),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::TrustedLocal => "trusted-local",
            Self::UnsafeRemote => "unsafe-remote",
        }
    }
}

impl ReportFormat {
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "json" => Ok(Self::Json),
            "html" => Ok(Self::Html),
            other => Err(format!("unsupported report format '{other}'")),
        }
    }
}

impl Cli {
    fn dsl_binding(&self) -> Option<TemplateBinding> {
        let locale = UiLocale::detect();
        self.dsl_path.as_deref().map(|path| {
            compile_file(path).unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("dsl_compile_failed", &format!("{err:?}"), None)
                );
                std::process::exit(2);
            })
        })
    }

    fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let locale = UiLocale::detect();
        let mut demo_mode = DemoMode::Both;
        let mut template_mode = TemplateMode::Tcp;
        let mut dsl_path = None;
        let mut protocol = None;
        let mut entry = None;
        let mut scan_all = false;
        let mut protocol_set_path = None;
        let mut list_protocols = false;
        let mut list_entries = None;
        let mut socket_trust = SocketTrustMode::TrustedLocal;
        let mut pid = None;
        let mut diagnostics = false;
        let mut findings = false;
        let mut http_transactions = false;
        let mut serve = false;
        let mut max_sessions = None;
        let mut json = false;
        let mut report_format = None;
        let mut summary_only = false;
        let mut out_path = None;
        let mut socket_target = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--demo" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_demo", "", None))?;
                    demo_mode = DemoMode::from_str(&value)?;
                }
                "--json" => json = true,
                "--report-format" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "missing report format".to_string())?;
                    report_format = Some(ReportFormat::from_str(&value)?);
                }
                "--serve" => serve = true,
                "--findings" => findings = true,
                "--http-transactions" => http_transactions = true,
                "--max-sessions" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_max_sessions", "", None))?;
                    max_sessions = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| locale.msgf("invalid_max_sessions", "", None))?,
                    );
                }
                "--summary-only" => summary_only = true,
                "--template" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_template", "", None))?;
                    template_mode = TemplateMode::from_str(&value)?;
                }
                "--dsl" => {
                    dsl_path = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_dsl", "", None))?,
                    );
                }
                "--protocol" => {
                    protocol = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_protocol", "", None))?,
                    );
                }
                "--scan-all" => scan_all = true,
                "--protocol-set" => {
                    protocol_set_path = Some(args.next().ok_or_else(|| {
                        "--protocol-set requires a protocol set file path".to_string()
                    })?);
                }
                "--entry" => {
                    entry = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_entry", "", None))?,
                    );
                }
                "--list-protocols" => list_protocols = true,
                "--allow-remote-socket" => socket_trust = SocketTrustMode::UnsafeRemote,
                "--list-entries" => {
                    list_entries = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_protocol", "", None))?,
                    );
                }
                "--socket-trust" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_socket_trust", "", None))?;
                    socket_trust = SocketTrustMode::from_str(&value)?;
                }
                "--pid" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_pid", "", None))?;
                    pid = Some(
                        value
                            .parse::<u32>()
                            .ok()
                            .filter(|pid| *pid > 0)
                            .ok_or_else(|| locale.msgf("invalid_pid", "", None))?,
                    );
                }
                "--diagnostics" => diagnostics = true,
                "--unix-socket" => {
                    socket_target =
                        Some(SocketTarget::Unix(args.next().ok_or_else(|| {
                            locale.msgf("missing_unix_socket", "", None)
                        })?));
                }
                "--tcp-socket" => {
                    socket_target =
                        Some(SocketTarget::Tcp(args.next().ok_or_else(|| {
                            locale.msgf("missing_tcp_socket", "", None)
                        })?));
                }
                "--out" => {
                    out_path = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_out", "", None))?,
                    );
                }
                "--help" | "-h" => return Err(usage().into()),
                other => return Err(locale.msgf("unknown_argument", other, None)),
            }
        }

        if summary_only && !json && report_format.is_none() {
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
        if report_format.is_some() && diagnostics {
            return Err("--report-format cannot be combined with --diagnostics".into());
        }
        if report_format.is_some() && http_transactions {
            return Err("--report-format cannot be combined with --http-transactions".into());
        }
        if scan_all && dsl_path.is_some() {
            return Err("--scan-all cannot be combined with --dsl".into());
        }
        if scan_all && protocol.is_some() {
            return Err("--scan-all cannot be combined with --protocol".into());
        }
        if scan_all && entry.is_some() {
            return Err("--scan-all cannot be combined with --entry".into());
        }
        if protocol_set_path.is_some() && !scan_all {
            return Err("--protocol-set requires --scan-all".into());
        }
        if dsl_path.is_some() && protocol.is_some() {
            return Err(locale.msg("dsl_protocol_conflict").into());
        }
        if dsl_path.is_some() && entry.is_some() {
            return Err(locale.msg("dsl_entry_conflict").into());
        }
        if list_protocols && list_entries.is_some() {
            return Err(locale.msg("list_conflict").into());
        }
        if entry.is_some() && protocol.is_none() {
            return Err(locale.msg("entry_requires_protocol").into());
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
        if matches!(socket_target, Some(SocketTarget::Tcp(_)))
            && socket_trust != SocketTrustMode::UnsafeRemote
            && socket_target
                .as_ref()
                .is_some_and(|target| !socket_target_is_local(target))
        {
            return Err(locale.msg("remote_socket_requires_flag").into());
        }

        if let Some(protocol_name) = protocol.as_deref() {
            let built_in_path = protocol_dsl_path(protocol_name, entry.as_deref())
                .ok_or_else(|| locale.msgf("unsupported_protocol", protocol_name, None))?;
            dsl_path = Some(built_in_path.to_string());
        }

        Ok(Self {
            demo_mode,
            template_mode,
            dsl_path,
            protocol,
            entry,
            scan_all,
            protocol_set_path,
            list_protocols,
            list_entries,
            socket_trust,
            pid,
            diagnostics,
            findings,
            http_transactions,
            serve,
            max_sessions,
            json,
            report_format,
            summary_only,
            out_path,
            socket_target,
        })
    }
}

fn process_matches_pid(process: Option<&ProcessView>, pid: u32) -> bool {
    process.is_some_and(|process| process.pid == pid)
}

fn ingest_trust_mode_for_cli(cli: &Cli) -> &'static str {
    match cli.socket_target {
        Some(_) => cli.socket_trust.as_str(),
        None => "synthetic-demo",
    }
}

fn annotate_export_trust(mut export: ExportBundle, cli: &Cli) -> ExportBundle {
    export.ingest_trust_mode = ingest_trust_mode_for_cli(cli).to_string();
    export
}

fn export_has_operation(export: &ExportBundle, operation: &str) -> bool {
    export.program_flows.iter().any(|flow| {
        matches!(
            &flow.operation,
            gewyvern::flow::ProgramOperation::Custom(value) if value == operation
        )
    })
}

fn socket_target_is_local(target: &SocketTarget) -> bool {
    match target {
        SocketTarget::Unix(_) => true,
        SocketTarget::Tcp(addr) => tcp_bind_addr_is_local(addr),
    }
}

fn tcp_bind_addr_is_local(addr: &str) -> bool {
    addr.to_socket_addrs()
        .map(|resolved| resolved.into_iter().all(|addr| addr.ip().is_loopback()))
        .unwrap_or_else(|_| addr.starts_with("localhost:"))
}

fn filter_export_by_pid(export: &ExportBundle, pid: u32) -> ExportBundle {
    let sessions = export
        .facts
        .iter()
        .filter_map(|fact| match &fact.kind {
            FactKind::SockLineage(lineage) if lineage.pid == pid => Some(fact.session),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let cookies = export
        .facts
        .iter()
        .filter_map(|fact| match &fact.kind {
            FactKind::SockLineage(lineage) if lineage.pid == pid => Some(lineage.sk_cookie),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let flow_ids = export
        .flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .map(|flow| flow.id)
        .collect::<HashSet<FlowId>>();
    let program_flow_ids = export
        .program_flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .map(|flow| flow.id)
        .collect::<HashSet<ProgramFlowId>>();
    let fact_matches = |fact: &FactEnvelope| {
        if sessions.contains(&fact.session) {
            return true;
        }
        match &fact.kind {
            FactKind::SockLineage(lineage) => lineage.pid == pid,
            FactKind::TcpState(state) => cookies.contains(&state.sk_cookie),
            FactKind::PacketMeta(packet) => packet
                .sk_cookie
                .is_some_and(|cookie| cookies.contains(&cookie)),
            FactKind::QuicMeta(quic) => quic
                .sk_cookie
                .is_some_and(|cookie| cookies.contains(&cookie)),
            FactKind::RouteDecision(route) => route
                .sk_cookie
                .is_some_and(|cookie| cookies.contains(&cookie)),
            FactKind::DropAction(_) | FactKind::AttachScope(_) => false,
        }
    };

    let mut filtered = export.clone();
    filtered.facts = export
        .facts
        .iter()
        .filter(|fact| fact_matches(fact))
        .cloned()
        .collect();
    let accepted_fact_ids = filtered
        .facts
        .iter()
        .map(|fact| fact.id)
        .collect::<HashSet<_>>();
    filtered.rejected_facts = export
        .rejected_facts
        .iter()
        .filter(|fact| accepted_fact_ids.contains(&fact.id))
        .cloned()
        .collect();
    filtered.rejected_fact_summary =
        gewyvern::runtime::summarize_rejected_facts(&filtered.rejected_facts);
    filtered.flows = export
        .flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .cloned()
        .collect();
    filtered.program_flows = export
        .program_flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .cloned()
        .collect();
    filtered.program_findings = export
        .program_findings
        .iter()
        .filter(|finding| {
            process_matches_pid(finding.process.as_ref(), pid)
                || program_flow_ids.contains(&finding.program_flow)
        })
        .cloned()
        .collect();
    filtered.module_findings = export
        .module_findings
        .iter()
        .filter(|finding| process_matches_pid(finding.process.as_ref(), pid))
        .cloned()
        .collect();
    filtered.reasons = export
        .reasons
        .iter()
        .filter(|reason| flow_ids.contains(&reason.flow))
        .cloned()
        .collect();
    filtered.debug_summary.accepted_facts = filtered.facts.len() as u64;
    filtered.debug_summary.rejected_facts = filtered.rejected_facts.len() as u64;
    filtered.debug_summary.flows = filtered.flows.len() as u64;
    filtered.debug_summary.program_flows = filtered.program_flows.len() as u64;
    filtered.debug_summary.program_findings = filtered.program_findings.len() as u64;
    filtered.debug_summary.module_findings = filtered.module_findings.len() as u64;
    filtered.debug_summary.reasons = filtered.reasons.len() as u64;
    filtered
}

fn run_session(template: gewyvern::template::Template, facts: Vec<FactEnvelope>) -> ExportBundle {
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

    assert_eq!(
        export.reasons, replay.reasons,
        "replay should stay deterministic"
    );
    export
}

fn run_binding_session(binding: TemplateBinding, facts: &[FactEnvelope]) -> ExportBundle {
    let config = SessionConfig::for_binding(binding).expect("dsl binding should be valid");
    let mut session = RuntimeSession::start(config).expect("dsl session startup should succeed");
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for fact in facts {
        session.ingest(fact.clone());
    }
    session.freeze(window_end);
    session.export_bundle()
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
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "dns_lookup"
            )
        });
    let is_http_request = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_request"
            )
        });
    let is_tls_client = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "tls_client"
            )
        });
    let is_http_server_response = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_server_response"
            )
        });
    let is_http3_request = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http3_request"
            )
        });
    let is_http3_server_response = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http3_server_response"
            )
        });
    let is_hy2_auth = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_auth"
            )
        });
    let is_hy2_udp_relay = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_udp_relay"
            )
        });
    let is_hy2_tcp_relay = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_tcp_relay"
            )
        });
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
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
        let mut facts = vec![FactEnvelope {
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
        }];
        if fragments.contains(&"route_meta_fragment") {
            facts.push(route_fact(
                2,
                base + Duration::from_millis(10),
                99,
                2,
                SessionId(2),
            ));
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
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
        let mut facts = vec![FactEnvelope {
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
        }];
        if fragments.contains(&"route_meta_fragment") {
            facts.push(route_fact(
                2,
                base + Duration::from_millis(10),
                88,
                2,
                SessionId(2),
            ));
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
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
    } else if fragments.contains(&"udp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
    {
        if is_http3_server_response {
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
                        sk_cookie: 177,
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
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(9),
                    ts: base + Duration::from_millis(80),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::ConnectionClose],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_http3_request {
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
                        local_port: Some(53000),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
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
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Ingress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Ingress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::ConnectionClose],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_hy2_tcp_relay {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 211,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..8].copy_from_slice(b"hysteria");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 211, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
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
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xe0),
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 220,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(9),
                    ts: base + Duration::from_millis(80),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x44),
                            (1u16, 0x01),
                        ]),
                    }),
                },
                FactEnvelope {
                    id: FactId(10),
                    ts: base + Duration::from_millis(90),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::from([(0u16, 0x00)]),
                    }),
                },
            ]
        } else if is_hy2_udp_relay {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 199,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..8].copy_from_slice(b"hysteria");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 199, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
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
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xe0),
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 220,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(9),
                    ts: base + Duration::from_millis(80),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Datagram],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(10),
                    ts: base + Duration::from_millis(90),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Datagram],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_hy2_auth {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 188,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..8].copy_from_slice(b"hysteria");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 188, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
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
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xe0),
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 220,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_dns_lookup {
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
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
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
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
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
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
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
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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

    assert_eq!(
        export.reasons, replay.reasons,
        "replay should stay deterministic"
    );
    export
}

#[cfg(test)]
mod tests {
    use super::{
        Cli, SocketTrustMode, annotate_export_trust, filter_export_by_pid, list_entries_json,
        list_entries_text, list_protocols_json, list_protocols_text, protocol_dsl_path,
        run_binding_demo, render_report_outputs, scan_report_html, scan_report_json,
        scan_targets_for_cli, scan_targets_from_set_file, summary_json, summary_line,
        findings_json, ReportFormat,
    };
    use gewyvern::dsl::compile_file;
    use gewyvern::flow::{ProgramFinding, ProgramFindingCause, ProgramOperation};
    use std::fs;
    use std::time::Instant;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn synthesize_large_protocol_flow_export() -> gewyvern::export::ExportBundle {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let base_flow = export.program_flows[0].clone();
        let base_process = base_flow.process.clone();
        let flow_count = 256u64;

        export.program_flows = (0..flow_count)
            .map(|offset| {
                let mut flow = base_flow.clone();
                flow.id = gewyvern::flow::ProgramFlowId(offset + 1);
                flow.process = base_process.clone();
                flow
            })
            .collect();
        export.program_findings = export
            .program_flows
            .iter()
            .map(|flow| ProgramFinding {
                program_flow: flow.id,
                process: flow.process.clone(),
                operation: flow.operation.clone(),
                module_label: "http_request_path".into(),
                network_module_kind: "http_request_response".into(),
                phase: Some("receive_response".into()),
                phase_kind: Some("receive_payload".into()),
                phase_transition: Some("send_request->receive_response".into()),
                phase_transition_kind: Some("emit_payload->receive_payload".into()),
                suspect_area: "transport_io".into(),
                cause: ProgramFindingCause::MissingCoreStage,
                summary: "synthetic missing response".into(),
                supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
                evidence_trace: vec!["missing_signal:packet_observed".into()],
            })
            .collect();
        export.debug_summary.program_flows = export.program_flows.len() as u64;
        export.debug_summary.program_findings = export.program_findings.len() as u64;
        export.debug_summary.module_findings = export.module_findings.len() as u64;
        export
    }

    #[test]
    fn http_request_demo_produces_healthy_cross_transport_path() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.debug_summary.accepted_facts, 6);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert_eq!(bundle.program_flows.len(), 1);
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_request"))
        );
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("receive_response"))
        );
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
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_client_hello"))
        );
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
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("receive_request"))
        );
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_response"))
        );
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("http_server_response".into())
        );
    }

    #[test]
    fn http3_request_demo_produces_healthy_quic_path() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy")
            .expect("http3_request_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_request_stream"))
        );
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("receive_response_stream"))
        );
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("http3_request".into())
        );
    }

    #[test]
    fn http3_server_response_demo_produces_healthy_quic_server_path() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy")
                .expect("http3_server_response_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("receive_request_stream"))
        );
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_response_stream"))
        );
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("http3_server_response".into())
        );
    }

    #[test]
    fn hy2_auth_demo_produces_healthy_quic_auth_path() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy")
            .expect("hy2_auth_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_auth_request_stream"))
        );
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("receive_auth_ok_stream"))
        );
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("hy2_auth".into())
        );
    }

    #[test]
    fn hy2_udp_relay_demo_produces_healthy_quic_datagram_path() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_relay_path.gewy")
            .expect("hy2_udp_relay_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_udp_relay_datagram"))
        );
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("receive_udp_relay_datagram"))
        );
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("hy2_udp_relay".into())
        );
    }

    #[test]
    fn hy2_tcp_relay_demo_produces_healthy_quic_tcp_path() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy")
            .expect("hy2_tcp_relay_path DSL should compile");
        let bundle = run_binding_demo(binding);
        assert_eq!(bundle.program_findings.len(), 0);
        assert_eq!(bundle.module_findings.len(), 0);
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("send_tcp_request_stream"))
        );
        assert!(
            bundle.program_flows[0]
                .stages
                .iter()
                .any(|stage| stage.phase.as_deref() == Some("receive_tcp_response_stream"))
        );
        assert_eq!(
            bundle.program_flows[0].operation,
            ProgramOperation::Custom("hy2_tcp_relay".into())
        );
    }

    #[test]
    fn cli_accepts_protocol_and_pid_and_resolves_built_in_dsl() {
        let cli = Cli::from_args([
            "--protocol".to_string(),
            "mysql".to_string(),
            "--entry".to_string(),
            "session".to_string(),
            "--pid".to_string(),
            "4242".to_string(),
            "--json".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.protocol.as_deref(), Some("mysql"));
        assert_eq!(cli.entry.as_deref(), Some("session"));
        assert_eq!(
            cli.dsl_path.as_deref(),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/session")
        );
        assert_eq!(cli.pid, Some(4242));
    }

    #[test]
    fn cli_rejects_combined_dsl_and_protocol() {
        let err = Cli::from_args([
            "--dsl".to_string(),
            "/tmp/demo.gewy".to_string(),
            "--protocol".to_string(),
            "mysql-query".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("--dsl"));
        assert!(err.contains("--protocol"));
    }

    #[test]
    fn protocol_lookup_covers_mysql_session() {
        assert_eq!(
            protocol_dsl_path("mysql", Some("session")),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/session".to_string())
        );
    }

    #[test]
    fn protocol_lookup_uses_default_entry_when_none_is_provided() {
        assert_eq!(
            protocol_dsl_path("mysql", None),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/session".to_string())
        );
        assert_eq!(
            protocol_dsl_path("amqp", None),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/amqp/session".to_string())
        );
    }

    #[test]
    fn cli_rejects_entry_without_protocol() {
        let err = Cli::from_args(["--entry".to_string(), "session".to_string()]).unwrap_err();
        assert!(err.contains("--entry"));
        assert!(err.contains("--protocol"));
    }

    #[test]
    fn legacy_protocol_alias_still_resolves() {
        assert_eq!(
            protocol_dsl_path("mysql-session", None),
            Some("/Users/Shared/chroot/dev/gewyvern/protocols/mysql/session".to_string())
        );
    }

    #[test]
    fn cli_accepts_list_protocols_mode() {
        let cli = Cli::from_args(["--list-protocols".to_string(), "--json".to_string()]).unwrap();
        assert!(cli.list_protocols);
        assert_eq!(cli.list_entries, None);
    }

    #[test]
    fn cli_accepts_list_entries_mode() {
        let cli = Cli::from_args(["--list-entries".to_string(), "mysql".to_string()]).unwrap();
        assert!(!cli.list_protocols);
        assert_eq!(cli.list_entries.as_deref(), Some("mysql"));
    }

    #[test]
    fn cli_accepts_scan_all_mode() {
        let cli = Cli::from_args(["--scan-all".to_string(), "--json".to_string()]).unwrap();
        assert!(cli.scan_all);
        assert_eq!(cli.protocol_set_path, None);
    }

    #[test]
    fn cli_accepts_html_report_format_for_scan_all() {
        let cli = Cli::from_args([
            "--scan-all".to_string(),
            "--report-format".to_string(),
            "html".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.report_format, Some(ReportFormat::Html));
    }

    #[test]
    fn cli_accepts_summary_only_html_report_without_json_flag() {
        let cli = Cli::from_args([
            "--scan-all".to_string(),
            "--summary-only".to_string(),
            "--report-format".to_string(),
            "html".to_string(),
        ])
        .unwrap();
        assert!(cli.summary_only);
        assert_eq!(cli.report_format, Some(ReportFormat::Html));
        assert!(!cli.json);
    }

    #[test]
    fn cli_accepts_protocol_html_report_without_scan_all() {
        let cli = Cli::from_args([
            "--protocol".to_string(),
            "mysql".to_string(),
            "--entry".to_string(),
            "session".to_string(),
            "--report-format".to_string(),
            "html".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.report_format, Some(ReportFormat::Html));
        assert_eq!(cli.protocol.as_deref(), Some("mysql"));
    }

    #[test]
    fn cli_rejects_protocol_set_without_scan_all() {
        let err = Cli::from_args([
            "--protocol-set".to_string(),
            "/tmp/protocols.txt".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("--protocol-set"));
        assert!(err.contains("--scan-all"));
    }

    #[test]
    fn cli_rejects_scan_all_with_protocol_selector() {
        let err = Cli::from_args([
            "--scan-all".to_string(),
            "--protocol".to_string(),
            "mysql".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("--scan-all"));
        assert!(err.contains("--protocol"));
    }

    #[test]
    fn cli_rejects_combined_list_modes() {
        let err = Cli::from_args([
            "--list-protocols".to_string(),
            "--list-entries".to_string(),
            "mysql".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("--list-protocols"));
        assert!(err.contains("--list-entries"));
    }

    #[test]
    fn list_protocols_output_includes_mysql_default_entry() {
        let text = list_protocols_text();
        assert!(text.contains("mysql (default: session)"));

        let json = list_protocols_json();
        assert!(json.contains("\"protocol\":\"mysql\""));
        assert!(json.contains("\"default_entry\":\"session\""));
    }

    #[test]
    fn list_entries_output_marks_default_entry() {
        let text = list_entries_text("ldap").expect("ldap should be present");
        assert!(text.contains("sync (default)"));
        assert!(text.contains("bind"));

        let json = list_entries_json("mysql").expect("mysql should be present");
        assert!(json.contains("\"protocol\":\"mysql\""));
        assert!(json.contains("\"mode\":\"session\",\"default\":true"));
    }

    #[test]
    fn default_scan_targets_include_protocol_defaults() {
        let cli = Cli::from_args(["--scan-all".to_string()]).unwrap();
        let targets = scan_targets_for_cli(&cli).unwrap();
        assert!(
            targets
                .iter()
                .any(|target| { target.protocol == "mysql" && target.entry == "session" })
        );
        assert!(
            targets
                .iter()
                .any(|target| { target.protocol == "amqp" && target.entry == "session" })
        );
        assert!(
            targets
                .iter()
                .any(|target| { target.protocol == "mysql" && target.entry == "connect" })
        );
        assert!(
            targets
                .iter()
                .any(|target| { target.protocol == "hy2" && target.entry == "tcp" })
        );
    }

    #[test]
    fn protocol_set_file_parses_comments_defaults_and_explicit_entries() {
        let path = std::env::temp_dir().join(format!(
            "gewyvern-protocol-set-{}.txt",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, "# comment\nmysql\namqp:publish\nldap bind\nmysql\n").unwrap();
        let targets = scan_targets_from_set_file(path.to_str().unwrap()).unwrap();
        fs::remove_file(&path).unwrap();

        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0].protocol, "mysql");
        assert_eq!(targets[0].entry, "session");
        assert_eq!(targets[1].protocol, "amqp");
        assert_eq!(targets[1].entry, "publish");
        assert_eq!(targets[2].protocol, "ldap");
        assert_eq!(targets[2].entry, "bind");
    }

    #[test]
    fn protocol_set_directory_scans_registered_gewy_projects() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-protocol-registry-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let package_dir = root.join("mysql").join("session");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("gewy.pkg"),
            "name=mysql_session\nversion=0.5.0\nentry=main.gewy\nregister.protocol=mysql\nregister.entry=session\nregister.default=true\n",
        )
        .unwrap();
        fs::write(
            package_dir.join("main.gewy"),
            "template(:mysql_session)\n|> window(:default_5s)\n|> reason(:udp_datagram_l1)\n|> fragment(:udp_packet_meta_fragment)\n|> fragment(:route_meta_fragment)\n",
        )
        .unwrap();

        let targets = scan_targets_from_set_file(root.to_str().unwrap()).unwrap();
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].protocol, "mysql");
        assert_eq!(targets[0].entry, "session");
        assert!(targets[0].dsl_path.ends_with("/mysql/session"));
    }

    #[test]
    fn cli_rejects_remote_tcp_socket_without_explicit_flag() {
        let err =
            Cli::from_args(["--tcp-socket".to_string(), "0.0.0.0:9000".to_string()]).unwrap_err();
        assert!(err.contains("--allow-remote-socket"));
    }

    #[test]
    fn cli_accepts_remote_tcp_socket_with_explicit_flag() {
        let cli = Cli::from_args([
            "--tcp-socket".to_string(),
            "0.0.0.0:9000".to_string(),
            "--allow-remote-socket".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.socket_trust, SocketTrustMode::UnsafeRemote);
    }

    #[test]
    fn cli_accepts_loopback_tcp_socket_without_remote_flag() {
        let cli =
            Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();
        assert_eq!(cli.socket_trust, SocketTrustMode::TrustedLocal);
    }

    #[test]
    fn cli_accepts_explicit_socket_trust_mode() {
        let cli = Cli::from_args([
            "--tcp-socket".to_string(),
            "0.0.0.0:9000".to_string(),
            "--socket-trust".to_string(),
            "unsafe-remote".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.socket_trust, SocketTrustMode::UnsafeRemote);
    }

    #[test]
    fn cli_rejects_unknown_socket_trust_mode() {
        let err =
            Cli::from_args(["--socket-trust".to_string(), "mystery".to_string()]).unwrap_err();
        assert!(err.contains("socket trust") || err.contains("信任模式"));
    }

    #[test]
    fn export_json_carries_ingest_trust_mode() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = export.to_json();
        assert!(json.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
    }

    #[test]
    fn summary_json_carries_ingest_trust_mode() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
    }

    #[test]
    fn summary_json_includes_protocol_flow_progress_for_healthy_export() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"protocol_flows\":["));
        assert!(json.contains("\"process_network_profiles\":["));
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"last_phase\":\"receive_response\""));
        assert!(json.contains("\"module_kinds\":[\"http_request_response\"]"));
    }

    #[test]
    fn scan_report_json_summarizes_all_targets() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let healthy_export = annotate_export_trust(
            run_binding_demo(binding.clone()),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let mut attention_export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = attention_export.program_flows[0].clone();
        attention_export.program_findings.push(ProgramFinding {
            program_flow: flow.id,
            process: flow.process.clone(),
            operation: flow.operation.clone(),
            module_label: "http_request_path".into(),
            network_module_kind: "http_request_response".into(),
            phase: Some("receive_response".into()),
            phase_kind: Some("receive_payload".into()),
            phase_transition: Some("send_request->receive_response".into()),
            phase_transition_kind: Some("emit_payload->receive_payload".into()),
            suspect_area: "transport_io".into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: "synthetic missing response".into(),
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            evidence_trace: vec!["missing_signal:packet_observed".into()],
        });
        let report = scan_report_json(&[
            ("scan:http:request".to_string(), healthy_export),
            ("scan:http:response".to_string(), attention_export),
        ]);
        assert!(report.contains("\"scan_all\":true"));
        assert!(report.contains("\"total_targets\":2"));
        assert!(report.contains("\"healthy_targets\":1"));
        assert!(report.contains("\"attention_targets\":1"));
        assert!(report.contains("\"target\":\"scan:http:request\""));
        assert!(report.contains("\"target\":\"scan:http:response\""));
    }

    #[test]
    fn scan_report_html_renders_visual_summary() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let report = scan_report_html(&[("scan:http:request".to_string(), export)]);
        assert!(report.contains("<!DOCTYPE html>"));
        assert!(report.contains("gewyvern Scan Report"));
        assert!(report.contains("scan:http:request"));
        assert!(report.contains("Process Profiles"));
        assert!(report.contains("primary module:"));
        assert!(report.contains("primary stage:"));
        assert!(report.contains("failure mode:"));
        assert!(report.contains("suspect modules:"));
        assert!(report.contains("family-request-response"));
        assert!(report.contains("stage-request-response"));
        assert!(report.contains("failure-none"));
        assert!(report.contains("last_phase=receive_response"));
        assert!(report.contains("request-response</span> 1"));
        assert!(report.contains("attention targets are shown first"));
        assert!(report.contains("<details class=\"card status-healthy\">"));
    }

    #[test]
    fn single_target_html_report_renders_visual_summary() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy")
            .expect("mysql_query_session DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let cli = Cli::from_args([
            "--protocol".to_string(),
            "mysql".to_string(),
            "--entry".to_string(),
            "session".to_string(),
            "--report-format".to_string(),
            "html".to_string(),
        ])
        .unwrap();
        let rendered = render_report_outputs(&cli, &[("scan:mysql:session".to_string(), export)]);
        assert!(rendered.contains("<!DOCTYPE html>"));
        assert!(rendered.contains("scan:mysql:session"));
    }

    #[test]
    fn scan_report_html_expands_attention_targets_by_default() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let mut attention_export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = attention_export.program_flows[0].clone();
        attention_export.program_findings.push(ProgramFinding {
            program_flow: flow.id,
            process: flow.process.clone(),
            operation: flow.operation.clone(),
            module_label: "http_request_path".into(),
            network_module_kind: "http_request_response".into(),
            phase: Some("receive_response".into()),
            phase_kind: Some("receive_payload".into()),
            phase_transition: Some("send_request->receive_response".into()),
            phase_transition_kind: Some("emit_payload->receive_payload".into()),
            suspect_area: "transport_io".into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: "synthetic missing response".into(),
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            evidence_trace: vec!["missing_signal:packet_observed".into()],
        });
        let report = scan_report_html(&[("scan:http:attention".to_string(), attention_export)]);
        assert!(report.contains("<details class=\"card status-attention\" open>"));
        assert!(report.contains("scan:http:attention"));
    }

    #[test]
    fn export_primary_conclusion_prefers_attention_process_profile() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );

        let healthy_flow = export.program_flows[0].clone();
        let mut attention_flow = healthy_flow.clone();
        attention_flow.id = gewyvern::flow::ProgramFlowId(healthy_flow.id.0 + 1000);
        if let Some(process) = &mut attention_flow.process {
            process.pid = 4242;
            process.comm = "apt".into();
        }
        export.program_flows.push(attention_flow.clone());
        export.program_findings.push(ProgramFinding {
            program_flow: attention_flow.id,
            process: attention_flow.process.clone(),
            operation: attention_flow.operation.clone(),
            module_label: "http_request_path".into(),
            network_module_kind: "http_request_response".into(),
            phase: Some("receive_response".into()),
            phase_kind: Some("receive_payload".into()),
            phase_transition: Some("send_request->receive_response".into()),
            phase_transition_kind: Some("emit_payload->receive_payload".into()),
            suspect_area: "transport_io".into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: "synthetic missing response".into(),
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            evidence_trace: vec!["missing_signal:packet_observed".into()],
        });

        assert_eq!(
            crate::primary_module_kind_for_export(&export),
            "http_request_response"
        );
        assert_eq!(
            crate::primary_failure_stage_for_export(&export),
            "send_request->receive_response"
        );
        assert_eq!(crate::primary_failure_mode_for_export(&export), "no_response");
        assert_eq!(crate::suspect_modules_for_export(&export), "http_request_path");
    }

    #[test]
    fn single_target_json_report_wraps_protocol_result() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy")
            .expect("mysql_query_session DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let cli = Cli::from_args([
            "--protocol".to_string(),
            "mysql".to_string(),
            "--entry".to_string(),
            "session".to_string(),
            "--report-format".to_string(),
            "json".to_string(),
        ])
        .unwrap();
        let rendered = render_report_outputs(&cli, &[("scan:mysql:session".to_string(), export)]);
        assert!(rendered.contains("\"scan_all\":true"));
        assert!(rendered.contains("\"target\":\"scan:mysql:session\""));
    }

    #[test]
    fn summary_json_marks_protocol_flow_attention_and_missing_transition() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        export.program_findings.push(ProgramFinding {
            program_flow: flow.id,
            process: flow.process.clone(),
            operation: flow.operation.clone(),
            module_label: "http_request_path".into(),
            network_module_kind: "http_request_response".into(),
            phase: Some("receive_response".into()),
            phase_kind: Some("receive_payload".into()),
            phase_transition: Some("send_request->receive_response".into()),
            phase_transition_kind: Some("emit_payload->receive_payload".into()),
            suspect_area: "transport_io".into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: "synthetic missing response".into(),
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            evidence_trace: vec!["missing_signal:packet_observed".into()],
        });
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"status\":\"attention\""));
        assert!(json.contains("\"network_module_kind\":\"http_request_response\""));
        assert!(json.contains("\"network_module_kinds\":[\"http_request_response\"]"));
        assert!(json.contains("\"process_network_profiles\":["));
        assert!(json.contains("\"missing_transitions\":[\"send_request->receive_response\"]"));
        assert!(json.contains("\"attention_flows\":1"));
        assert!(json.contains("\"missing_transitions\":[\"send_request->receive_response\"]"));
        assert!(json.contains("\"suspect_areas\":[\"transport_io\"]"));
        assert!(json.contains("\"primary_module_kind\":\"http_request_response\""));
        assert!(json.contains("\"primary_failure_stage\":\"send_request->receive_response\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"failure_mode\":\"no_response\""));
    }

    #[test]
    fn failure_mode_label_classifies_database_directory_and_quic_families() {
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "database_error_handling",
                "receive_error",
                &[],
            ),
            "semantic_error"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "directory_write",
                "receive_modify_denied",
                &[],
            ),
            "server_denied"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "quic_handshake",
                "send_initial->receive_handshake",
                &[],
            ),
            "setup_incomplete"
        );
    }

    #[test]
    fn findings_json_carries_network_module_classification() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        export.program_findings.push(ProgramFinding {
            program_flow: flow.id,
            process: flow.process.clone(),
            operation: flow.operation.clone(),
            module_label: "http_request_path".into(),
            network_module_kind: "http_request_response".into(),
            phase: Some("receive_response".into()),
            phase_kind: Some("receive_payload".into()),
            phase_transition: Some("send_request->receive_response".into()),
            phase_transition_kind: Some("emit_payload->receive_payload".into()),
            suspect_area: "transport_io".into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: "synthetic missing response".into(),
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            evidence_trace: vec!["missing_signal:packet_observed".into()],
        });
        export.module_findings = vec![gewyvern::flow::ModuleFinding {
            module_label: "http_request_path".into(),
            process: flow.process.clone(),
            operation: flow.operation.clone(),
            severity: gewyvern::flow::ModuleSeverity::Low,
            network_module_kinds: vec!["http_request_response".into()],
            phases: vec!["receive_response".into()],
            phase_kinds: vec!["receive_payload".into()],
            phase_transitions: vec!["send_request->receive_response".into()],
            phase_transition_kinds: vec!["emit_payload->receive_payload".into()],
            suspect_areas: vec!["transport_io".into()],
            causes: vec![ProgramFindingCause::MissingCoreStage],
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            program_flows: vec![flow.id],
            summaries: vec!["synthetic missing response".into()],
            evidence_trace: vec!["missing_signal:packet_observed".into()],
        }];
        let json = findings_json("dsl_demo", &export);
        assert!(json.contains("\"network_module_kind\":\"http_request_response\""));
        assert!(json.contains("\"network_module_kinds\":[\"http_request_response\"]"));
        assert!(json.contains("\"process_network_profiles\":["));
        assert!(json.contains("\"primary_module_kind\":\"http_request_response\""));
        assert!(json.contains("\"primary_failure_stage\":\"send_request->receive_response\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
    }

    #[test]
    #[ignore = "benchmark"]
    fn benchmark_summary_json_large_protocol_flow_export() {
        let export = synthesize_large_protocol_flow_export();
        let start = Instant::now();
        let mut total_len = 0usize;
        for _ in 0..200 {
            total_len += summary_json("bench", &export).len();
        }
        let elapsed = start.elapsed();
        assert!(total_len > 0);
        eprintln!(
            "benchmark_summary_json_large_protocol_flow_export: iterations=200 flows={} findings={} elapsed_ms={:.3}",
            export.program_flows.len(),
            export.program_findings.len(),
            elapsed.as_secs_f64() * 1000.0
        );
    }

    #[test]
    #[ignore = "benchmark"]
    fn benchmark_summary_line_large_protocol_flow_export() {
        let export = synthesize_large_protocol_flow_export();
        let start = Instant::now();
        let mut total_len = 0usize;
        for _ in 0..200 {
            total_len += summary_line("bench", &export).len();
        }
        let elapsed = start.elapsed();
        assert!(total_len > 0);
        eprintln!(
            "benchmark_summary_line_large_protocol_flow_export: iterations=200 flows={} findings={} elapsed_ms={:.3}",
            export.program_flows.len(),
            export.program_findings.len(),
            elapsed.as_secs_f64() * 1000.0
        );
    }

    #[test]
    fn pid_filter_keeps_only_target_process_view() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let bundle = run_binding_demo(binding);
        let pid = bundle.program_flows[0]
            .process
            .as_ref()
            .expect("demo should be process-bound")
            .pid;
        let filtered = filter_export_by_pid(&bundle, pid);
        assert_eq!(filtered.program_flows.len(), 1);
        assert_eq!(
            filtered.program_flows[0].process.as_ref().map(|p| p.pid),
            Some(pid)
        );
        let empty = filter_export_by_pid(&bundle, pid + 1);
        assert_eq!(empty.program_flows.len(), 0);
        assert_eq!(empty.program_findings.len(), 0);
        assert_eq!(empty.module_findings.len(), 0);
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
    let protocol_flows = protocol_flow_summaries_text(export);
    let process_profiles = process_network_profiles_text(export);
    format!(
        "{name}: {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} {}={} protocol_flows={} process_network_profiles={}",
        locale.label("template"),
        export.template_id,
        "ingest_trust_mode",
        export.ingest_trust_mode,
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
        protocol_flows,
        process_profiles,
    )
}

fn write_or_print(rendered: &str, out_path: Option<&str>, locale: UiLocale) {
    if let Some(path) = out_path {
        fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("write_failed", path, Some(&err.to_string()))
            );
            std::process::exit(1);
        });
    } else {
        println!("{rendered}");
    }
}

fn list_protocols_text() -> String {
    protocol_names()
        .into_iter()
        .filter_map(|protocol| {
            protocol_default_entry(&protocol)
                .map(|default_entry| format!("{protocol} (default: {default_entry})"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn list_protocols_json() -> String {
    let items = protocol_names()
        .into_iter()
        .filter_map(|protocol| {
            protocol_default_entry(&protocol).map(|default_entry| {
                format!("{{\"protocol\":\"{protocol}\",\"default_entry\":\"{default_entry}\"}}")
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{items}]")
}

fn list_entries_text(protocol: &str) -> Option<String> {
    let default_entry = protocol_default_entry(protocol)?;
    let lines = protocol_entries(protocol)?
        .into_iter()
        .map(|entry| {
            if entry == default_entry {
                format!("{entry} (default)")
            } else {
                entry.to_string()
            }
        })
        .collect::<Vec<_>>();
    Some(lines.join("\n"))
}

fn list_entries_json(protocol: &str) -> Option<String> {
    let default_entry = protocol_default_entry(protocol)?;
    let entries = protocol_entries(protocol)?
        .into_iter()
        .map(|entry| {
            format!(
                "{{\"mode\":\"{entry}\",\"default\":{}}}",
                if entry == default_entry {
                    "true"
                } else {
                    "false"
                }
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        "{{\"protocol\":\"{protocol}\",\"default_entry\":\"{default_entry}\",\"entries\":[{entries}]}}"
    ))
}

fn scan_targets_for_cli(cli: &Cli) -> Result<Vec<ScanTarget>, String> {
    if !cli.scan_all {
        return Ok(Vec::new());
    }
    match cli.protocol_set_path.as_deref() {
        Some(path) => scan_targets_from_set_file(path),
        None => Ok(default_protocol_scan_set()
            .into_iter()
            .map(ScanTarget::from_resolved)
            .collect()),
    }
}

fn scan_targets_from_set_file(path: &str) -> Result<Vec<ScanTarget>, String> {
    if Path::new(path).is_dir() {
        return default_protocol_scan_set_from_dir(path)
            .map(|targets| targets.into_iter().map(ScanTarget::from_resolved).collect())
            .ok_or_else(|| {
                format!(
                    "protocol registry directory '{}' did not resolve any scan targets",
                    path
                )
            });
    }
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read protocol set '{path}': {err}"))?;
    let mut targets = Vec::new();
    let mut seen = HashSet::new();

    for (index, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (protocol, entry) = parse_protocol_set_line(line)
            .map_err(|err| format!("invalid protocol set line {}: {err}", index + 1))?;
        let resolved = resolve_protocol_profile(protocol, entry).ok_or_else(|| {
            format!(
                "unsupported protocol target on line {}: {}",
                index + 1,
                line
            )
        })?;
        let key = format!("{}:{}", resolved.protocol, resolved.entry);
        if seen.insert(key) {
            targets.push(ScanTarget::from_resolved(resolved));
        }
    }

    if targets.is_empty() {
        return Err(format!(
            "protocol set '{}' did not resolve any scan targets",
            path
        ));
    }

    Ok(targets)
}

fn parse_protocol_set_line(line: &str) -> Result<(&str, Option<&str>), String> {
    if let Some((protocol, entry)) = line.split_once(':') {
        let protocol = protocol.trim();
        let entry = entry.trim();
        if protocol.is_empty() || entry.is_empty() {
            return Err(format!("expected '<protocol>:<entry>', got '{line}'"));
        }
        return Ok((protocol, Some(entry)));
    }

    let mut parts = line.split_whitespace();
    let protocol = parts
        .next()
        .ok_or_else(|| format!("expected '<protocol>' or '<protocol> <entry>', got '{line}'"))?;
    let entry = parts.next();
    if parts.next().is_some() {
        return Err(format!(
            "expected '<protocol>' or '<protocol> <entry>', got '{line}'"
        ));
    }
    Ok((protocol, entry))
}

fn usage() -> &'static str {
    UiLocale::detect().usage()
}

#[derive(Default)]
struct ProtocolFlowFindingSummary {
    missing_transitions: Vec<String>,
    network_module_kinds: Vec<String>,
    suspect_areas: Vec<String>,
    has_findings: bool,
}

#[derive(Default)]
struct ProcessNetworkProfileSummary {
    pid: u32,
    comm: String,
    status: String,
    primary_module_kind: String,
    primary_module_family: String,
    primary_failure_stage: String,
    primary_stage_family: String,
    primary_failure_mode: String,
    operations: Vec<String>,
    module_kinds: Vec<String>,
    phases: Vec<String>,
    missing_transitions: Vec<String>,
    suspect_areas: Vec<String>,
    suspect_modules: Vec<String>,
    attention_flows: u64,
    healthy_flows: u64,
}

fn first_or_none(items: &[String]) -> String {
    items.first().cloned().unwrap_or_else(|| "none".into())
}

fn first_non_none(items: &[String]) -> Option<String> {
    items.iter().find(|item| item.as_str() != "none").cloned()
}

fn bump_score(
    scores: &mut HashMap<(u32, String), HashMap<String, u32>>,
    key: &(u32, String),
    value: &str,
    delta: u32,
) {
    if value.is_empty() || value == "none" {
        return;
    }
    *scores
        .entry(key.clone())
        .or_default()
        .entry(value.to_string())
        .or_default() += delta;
}

fn best_scored_value(
    scores: &HashMap<(u32, String), HashMap<String, u32>>,
    key: &(u32, String),
) -> Option<String> {
    let values = scores.get(key)?;
    values
        .iter()
        .max_by(|(left_value, left_score), (right_value, right_score)| {
            left_score
                .cmp(right_score)
                .then_with(|| right_value.cmp(left_value))
        })
        .map(|(value, _)| value.clone())
}

fn module_family_label(module_kind: &str) -> &'static str {
    match module_kind {
        "name_resolution" => "dns",
        "route_resolution" => "route",
        "connection_establishment" | "database_connectivity" => "connect",
        "tls_handshake" | "quic_handshake" | "tunnel_handshake" => "handshake",
        "http_request_response" | "http3_request_response" | "iot_request_response" => {
            "request-response"
        }
        "database_query" | "database_error_handling" => "database",
        "database_authentication" | "directory_bind" | "authentication_exchange"
        | "proxy_authentication" => "auth",
        "directory_search" | "directory_write" | "directory_sync" => "directory",
        "message_publish" | "message_session" | "mail_session" | "signaling_session" => {
            "messaging"
        }
        "proxy_udp_relay" | "proxy_tcp_relay" | "quic_stream_session" | "transport_session" => {
            "relay"
        }
        "datagram_exchange" | "management_query" | "time_synchronization"
        | "address_configuration" | "tunnel_control" | "cache_access" => "service",
        _ => "general",
    }
}

fn stage_family_label(stage: &str) -> &'static str {
    if stage.contains("resolve") || stage.contains("dns") {
        "dns"
    } else if stage.contains("connect") || stage.contains("establish") || stage.contains("bind") {
        "connect"
    } else if stage.contains("tls")
        || stage.contains("hello")
        || stage.contains("crypto")
        || stage.contains("handshake")
    {
        "handshake"
    } else if stage.contains("request")
        || stage.contains("response")
        || stage.contains("query")
        || stage.contains("publish")
        || stage.contains("stream")
        || stage.contains("relay")
    {
        "request-response"
    } else if stage.contains("auth") || stage.contains("password") {
        "auth"
    } else if stage == "none" {
        "none"
    } else {
        "general"
    }
}

fn failure_mode_label(
    status: &str,
    module_kind: &str,
    primary_stage: &str,
    suspect_areas: &[String],
) -> &'static str {
    if status != "attention" {
        return "none";
    }

    let stage = primary_stage.to_ascii_lowercase();
    let module = module_kind.to_ascii_lowercase();

    if stage.contains("denied") {
        return "server_denied";
    }
    if stage.contains("constraint") || stage.contains("error") || module.contains("error") {
        return "semantic_error";
    }
    if stage.contains("close") {
        return "peer_closed";
    }
    if stage.contains("resolve") || stage.contains("dns") || stage.contains("connect")
        || stage.contains("establish") || stage.contains("handshake") || stage.contains("crypto")
    {
        return "setup_incomplete";
    }
    if let Some((_, right)) = stage.split_once("->") {
        if right.starts_with("receive")
            || right.contains("response")
            || right.contains("result")
            || right.contains("ack")
            || right.contains("accept")
            || right.contains("offer")
            || right.contains("ready")
            || right.contains("ok")
        {
            return "no_response";
        }
        if right.starts_with("send")
            || right.contains("request")
            || right.contains("query")
            || right.contains("publish")
            || right.contains("auth")
            || right.contains("password")
            || right.contains("relay")
            || right.contains("stream")
        {
            return "not_sent";
        }
    }
    if stage.starts_with("send_")
        || stage.contains("request")
        || stage.contains("query")
        || stage.contains("publish")
        || stage.contains("relay")
        || stage.contains("stream")
    {
        return "not_sent";
    }
    if stage.starts_with("receive_")
        || stage.contains("response")
        || stage.contains("result")
        || stage.contains("ack")
        || stage.contains("ready")
        || stage.contains("ok")
    {
        return "no_response";
    }
    if suspect_areas.iter().any(|area| area == "route_io" || area == "transport_io") {
        return "no_response";
    }
    "attention"
}

fn failure_mode_family_label(mode: &str) -> &'static str {
    match mode {
        "not_sent" => "blocked",
        "no_response" => "timeout",
        "setup_incomplete" => "setup",
        "semantic_error" => "semantic",
        "server_denied" => "denied",
        "peer_closed" => "peer",
        "none" => "none",
        _ => "general",
    }
}

#[derive(Clone, Copy)]
enum ScanTargetStatus {
    Idle,
    Healthy,
    Attention,
}

impl ScanTargetStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Healthy => "healthy",
            Self::Attention => "attention",
        }
    }
}

fn protocol_flow_phases(flow: &gewyvern::flow::ProgramFlow) -> Vec<String> {
    let mut phases = Vec::new();
    for phase in flow.stages.iter().filter_map(|stage| stage.phase.as_ref()) {
        if phases.last() != Some(phase) {
            phases.push(phase.clone());
        }
    }
    phases
}

fn protocol_flow_last_phase(flow: &gewyvern::flow::ProgramFlow) -> Option<String> {
    flow.stages
        .iter()
        .rev()
        .find_map(|stage| stage.phase.clone())
}

fn protocol_flow_finding_summaries(
    export: &ExportBundle,
) -> HashMap<ProgramFlowId, ProtocolFlowFindingSummary> {
    let mut summaries = HashMap::<ProgramFlowId, ProtocolFlowFindingSummary>::new();
    for finding in &export.program_findings {
        let entry = summaries.entry(finding.program_flow).or_default();
        entry.has_findings = true;
        if let Some(transition) = &finding.phase_transition {
            if !entry.missing_transitions.contains(transition) {
                entry.missing_transitions.push(transition.clone());
            }
        }
        if !entry
            .network_module_kinds
            .contains(&finding.network_module_kind)
        {
            entry
                .network_module_kinds
                .push(finding.network_module_kind.clone());
        }
        if !entry.suspect_areas.contains(&finding.suspect_area) {
            entry.suspect_areas.push(finding.suspect_area.clone());
        }
    }
    summaries
}

fn protocol_flow_status(
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> &'static str {
    if finding_summary.is_some_and(|summary| summary.has_findings) {
        "attention"
    } else {
        "healthy"
    }
}

fn protocol_flow_failure_mode(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> String {
    let status = protocol_flow_status(finding_summary);
    let last_phase = protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into());
    let module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        Some(&last_phase),
        None,
        "network_module",
    );
    let primary_stage = finding_summary
        .and_then(|summary| summary.missing_transitions.first().cloned())
        .unwrap_or(last_phase);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.as_slice())
        .unwrap_or(&[]);
    failure_mode_label(status, module_kind, &primary_stage, suspect_areas).to_string()
}

fn protocol_flow_summary_item_json(
    flow: &gewyvern::flow::ProgramFlow,
    finding_summary: Option<&ProtocolFlowFindingSummary>,
) -> String {
    let phases = protocol_flow_phases(flow);
    let network_module_kind = gewyvern::flow::infer_network_module_kind(
        &flow.operation,
        protocol_flow_last_phase(flow).as_deref(),
        None,
        "network_module",
    );
    let missing_transitions = finding_summary
        .map(|summary| summary.missing_transitions.as_slice())
        .unwrap_or(&[]);
    let network_module_kinds = finding_summary
        .map(|summary| summary.network_module_kinds.as_slice())
        .unwrap_or(&[]);
    let suspect_areas = finding_summary
        .map(|summary| summary.suspect_areas.as_slice())
        .unwrap_or(&[]);
    let failure_mode = protocol_flow_failure_mode(flow, finding_summary);
    format!(
        "{{\"program_flow\":{},\"process\":{},\"operation\":\"{}\",\"network_module_kind\":\"{}\",\"network_module_kinds\":{},\"status\":\"{}\",\"failure_mode\":\"{}\",\"failure_mode_family\":\"{}\",\"phases\":{},\"last_phase\":{},\"missing_transitions\":{},\"suspect_areas\":{}}}",
        flow.id.0,
        process_json(flow.process.as_ref()),
        operation_label(&flow.operation),
        network_module_kind,
        if network_module_kinds.is_empty() {
            format!("[\"{network_module_kind}\"]")
        } else {
            string_list_json(network_module_kinds)
        },
        protocol_flow_status(finding_summary),
        failure_mode,
        failure_mode_family_label(&failure_mode),
        string_list_json(&phases),
        protocol_flow_last_phase(flow)
            .map(|phase| format!("\"{}\"", phase))
            .unwrap_or_else(|| "null".into()),
        string_list_json(missing_transitions),
        string_list_json(suspect_areas),
    )
}

fn protocol_flow_summaries_json(export: &ExportBundle) -> String {
    let finding_summaries = protocol_flow_finding_summaries(export);
    format!(
        "[{}]",
        export
            .program_flows
            .iter()
            .map(|flow| protocol_flow_summary_item_json(flow, finding_summaries.get(&flow.id)))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn protocol_flow_summaries_text(export: &ExportBundle) -> String {
    let locale = UiLocale::detect();
    if export.program_flows.is_empty() {
        return locale.none().to_string();
    }
    let finding_summaries = protocol_flow_finding_summaries(export);
    export
        .program_flows
        .iter()
        .map(|flow| {
            let phases = protocol_flow_phases(flow);
            let finding_summary = finding_summaries.get(&flow.id);
            let network_module_kind = gewyvern::flow::infer_network_module_kind(
                &flow.operation,
                protocol_flow_last_phase(flow).as_deref(),
                None,
                "network_module",
            );
            let missing_transitions = finding_summary
                .map(|summary| summary.missing_transitions.as_slice())
                .unwrap_or(&[]);
            let phase_text = if phases.is_empty() {
                locale.none().to_string()
            } else {
                phases.join(">")
            };
            let missing_text = if missing_transitions.is_empty() {
                String::new()
            } else {
                format!(" missing={}", missing_transitions.join("|"))
            };
            let failure_mode = protocol_flow_failure_mode(flow, finding_summary);
            format!(
                "{}[kind={} status={} failure_mode={} phases={}{}]",
                operation_label(&flow.operation),
                network_module_kind,
                protocol_flow_status(finding_summary),
                failure_mode,
                phase_text,
                missing_text
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn process_network_profile_summaries(export: &ExportBundle) -> Vec<ProcessNetworkProfileSummary> {
    let finding_summaries = protocol_flow_finding_summaries(export);
    let mut profiles = HashMap::<(u32, String), ProcessNetworkProfileSummary>::new();
    let mut module_scores = HashMap::<(u32, String), HashMap<String, u32>>::new();
    let mut stage_scores = HashMap::<(u32, String), HashMap<String, u32>>::new();
    let mut suspect_module_scores = HashMap::<(u32, String), HashMap<String, u32>>::new();

    for flow in &export.program_flows {
        let Some(process) = flow.process.as_ref() else {
            continue;
        };
        let key = (process.pid, process.comm.clone());
        let entry = profiles
            .entry(key.clone())
            .or_insert_with(|| ProcessNetworkProfileSummary {
                pid: process.pid,
                comm: process.comm.clone(),
                status: "idle".into(),
                primary_module_kind: "none".into(),
                primary_module_family: "general".into(),
                primary_failure_stage: "none".into(),
                primary_stage_family: "none".into(),
                primary_failure_mode: "none".into(),
                ..Default::default()
            });

        let operation = operation_label(&flow.operation);
        if !entry.operations.contains(&operation) {
            entry.operations.push(operation);
        }

        let inferred_kind = gewyvern::flow::infer_network_module_kind(
            &flow.operation,
            protocol_flow_last_phase(flow).as_deref(),
            None,
            "network_module",
        )
        .to_string();
        let last_phase = protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into());
        if !entry.module_kinds.contains(&inferred_kind) {
            entry.module_kinds.push(inferred_kind.clone());
        }
        bump_score(&mut module_scores, &key, &inferred_kind, 1);
        bump_score(&mut stage_scores, &key, &last_phase, 1);

        for phase in protocol_flow_phases(flow) {
            if !entry.phases.contains(&phase) {
                entry.phases.push(phase);
            }
        }

        match finding_summaries.get(&flow.id) {
            Some(summary) if summary.has_findings => {
                entry.attention_flows += 1;
                entry.status = "attention".into();
                if summary.network_module_kinds.is_empty() {
                    bump_score(&mut module_scores, &key, &inferred_kind, 10);
                } else {
                    for module_kind in &summary.network_module_kinds {
                        bump_score(&mut module_scores, &key, module_kind, 10);
                    }
                }
                if summary.missing_transitions.is_empty() {
                    bump_score(&mut stage_scores, &key, &last_phase, 10);
                } else {
                    for transition in &summary.missing_transitions {
                        bump_score(&mut stage_scores, &key, transition, 10);
                    }
                }
                for module_kind in &summary.network_module_kinds {
                    if !entry.module_kinds.contains(module_kind) {
                        entry.module_kinds.push(module_kind.clone());
                    }
                }
                for transition in &summary.missing_transitions {
                    if !entry.missing_transitions.contains(transition) {
                        entry.missing_transitions.push(transition.clone());
                    }
                }
                for suspect_area in &summary.suspect_areas {
                    if !entry.suspect_areas.contains(suspect_area) {
                        entry.suspect_areas.push(suspect_area.clone());
                    }
                }
            }
            _ => {
                entry.healthy_flows += 1;
                if entry.status != "attention" {
                    entry.status = "healthy".into();
                }
            }
        }
    }

    for finding in &export.program_findings {
        let Some(process) = finding.process.as_ref() else {
            continue;
        };
        let key = (process.pid, process.comm.clone());
        let entry = profiles
            .entry(key.clone())
            .or_insert_with(|| ProcessNetworkProfileSummary {
                pid: process.pid,
                comm: process.comm.clone(),
                status: "idle".into(),
                primary_module_kind: "none".into(),
                primary_module_family: "general".into(),
                primary_failure_stage: "none".into(),
                primary_stage_family: "none".into(),
                primary_failure_mode: "none".into(),
                ..Default::default()
            });
        entry.status = "attention".into();
        if !entry.module_kinds.contains(&finding.network_module_kind) {
            entry
                .module_kinds
                .push(finding.network_module_kind.clone());
        }
        if !entry.suspect_areas.contains(&finding.suspect_area) {
            entry.suspect_areas.push(finding.suspect_area.clone());
        }
        if !entry.suspect_modules.contains(&finding.module_label) {
            entry.suspect_modules.push(finding.module_label.clone());
        }
        bump_score(&mut module_scores, &key, &finding.network_module_kind, 20);
        if let Some(phase) = &finding.phase {
            bump_score(&mut stage_scores, &key, phase, 20);
        }
        if let Some(transition) = &finding.phase_transition {
            bump_score(&mut stage_scores, &key, transition, 25);
        }
        bump_score(&mut suspect_module_scores, &key, &finding.module_label, 20);
    }

    let mut profiles = profiles.into_values().collect::<Vec<_>>();
    for profile in &mut profiles {
        let key = (profile.pid, profile.comm.clone());
        profile.operations.sort();
        profile.operations.dedup();
        profile.module_kinds.sort();
        profile.module_kinds.dedup();
        profile.phases.sort();
        profile.phases.dedup();
        profile.missing_transitions.sort();
        profile.missing_transitions.dedup();
        profile.suspect_areas.sort();
        profile.suspect_areas.dedup();
        profile.suspect_modules.sort();
        profile.suspect_modules.dedup();
        profile.primary_module_kind = best_scored_value(&module_scores, &key)
            .or_else(|| first_non_none(&profile.module_kinds))
            .unwrap_or_else(|| "none".into());
        profile.primary_module_family =
            module_family_label(&profile.primary_module_kind).to_string();
        profile.primary_failure_stage = best_scored_value(&stage_scores, &key)
            .or_else(|| first_non_none(&profile.missing_transitions))
            .or_else(|| first_non_none(&profile.phases))
            .unwrap_or_else(|| "none".into());
        profile.primary_stage_family =
            stage_family_label(&profile.primary_failure_stage).to_string();
        profile.primary_failure_mode = failure_mode_label(
            &profile.status,
            &profile.primary_module_kind,
            &profile.primary_failure_stage,
            &profile.suspect_areas,
        )
        .to_string();
        if let Some(primary_suspect_module) = best_scored_value(&suspect_module_scores, &key) {
            if let Some(index) = profile
                .suspect_modules
                .iter()
                .position(|module| module == &primary_suspect_module)
            {
                let module = profile.suspect_modules.remove(index);
                profile.suspect_modules.insert(0, module);
            }
        }
    }
    profiles.sort_by(|a, b| a.pid.cmp(&b.pid).then_with(|| a.comm.cmp(&b.comm)));
    profiles
}

fn process_network_profiles_json(export: &ExportBundle) -> String {
    format!(
        "[{}]",
        process_network_profile_summaries(export)
            .into_iter()
            .map(|profile| format!(
                "{{\"pid\":{},\"comm\":\"{}\",\"status\":\"{}\",\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"operations\":{},\"module_kinds\":{},\"phases\":{},\"missing_transitions\":{},\"suspect_areas\":{},\"suspect_modules\":{},\"healthy_flows\":{},\"attention_flows\":{}}}",
                profile.pid,
                profile.comm,
                profile.status,
                profile.primary_module_kind,
                profile.primary_module_family,
                profile.primary_failure_stage,
                profile.primary_stage_family,
                profile.primary_failure_mode,
                failure_mode_family_label(&profile.primary_failure_mode),
                string_list_json(&profile.operations),
                string_list_json(&profile.module_kinds),
                string_list_json(&profile.phases),
                string_list_json(&profile.missing_transitions),
                string_list_json(&profile.suspect_areas),
                string_list_json(&profile.suspect_modules),
                profile.healthy_flows,
                profile.attention_flows,
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn process_network_profiles_text(export: &ExportBundle) -> String {
    let locale = UiLocale::detect();
    let profiles = process_network_profile_summaries(export);
    if profiles.is_empty() {
        return locale.none().to_string();
    }
    profiles
        .into_iter()
        .map(|profile| {
            let kinds = if profile.module_kinds.is_empty() {
                locale.none().to_string()
            } else {
                profile.module_kinds.join("|")
            };
            let phases = if profile.phases.is_empty() {
                locale.none().to_string()
            } else {
                profile.phases.join(">")
            };
            let missing = if profile.missing_transitions.is_empty() {
                String::new()
            } else {
                format!(" missing={}", profile.missing_transitions.join("|"))
            };
            format!(
                "{}(pid={})[status={} primary_kind={} primary_stage={} failure_mode={} kinds={} healthy={} attention={} phases={}{}]",
                profile.comm,
                profile.pid,
                profile.status,
                profile.primary_module_kind,
                profile.primary_failure_stage,
                profile.primary_failure_mode,
                kinds,
                profile.healthy_flows,
                profile.attention_flows,
                phases,
                missing
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn primary_process_profile_for_export(export: &ExportBundle) -> Option<ProcessNetworkProfileSummary> {
    let mut profiles = process_network_profile_summaries(export);
    profiles.sort_by(|left, right| {
        let left_rank = match left.status.as_str() {
            "attention" => 0,
            "healthy" => 1,
            _ => 2,
        };
        let right_rank = match right.status.as_str() {
            "attention" => 0,
            "healthy" => 1,
            _ => 2,
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| right.attention_flows.cmp(&left.attention_flows))
            .then_with(|| right.healthy_flows.cmp(&left.healthy_flows))
            .then_with(|| left.pid.cmp(&right.pid))
            .then_with(|| left.comm.cmp(&right.comm))
    });
    profiles.into_iter().next()
}

fn primary_module_kind_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_module_kind;
    }
    if let Some(finding) = export.program_findings.first() {
        return finding.network_module_kind.clone();
    }
    export
        .program_flows
        .first()
        .map(|flow| {
            gewyvern::flow::infer_network_module_kind(
                &flow.operation,
                protocol_flow_last_phase(flow).as_deref(),
                None,
                "network_module",
            )
            .to_string()
        })
        .unwrap_or_else(|| "none".into())
}

fn primary_failure_stage_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_failure_stage;
    }
    if let Some(finding) = export.program_findings.first() {
        if let Some(phase) = &finding.phase {
            return phase.clone();
        }
        if let Some(transition) = &finding.phase_transition {
            return transition.clone();
        }
    }
    export
        .program_flows
        .iter()
        .filter_map(protocol_flow_last_phase)
        .next_back()
        .unwrap_or_else(|| "none".into())
}

fn primary_failure_mode_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        return profile.primary_failure_mode;
    }
    failure_mode_label(
        scan_target_status(export).label(),
        &primary_module_kind_for_export(export),
        &primary_failure_stage_for_export(export),
        &export
            .program_findings
            .iter()
            .map(|finding| finding.suspect_area.clone())
            .collect::<Vec<_>>(),
    )
    .to_string()
}

fn suspect_modules_for_export(export: &ExportBundle) -> String {
    if let Some(profile) = primary_process_profile_for_export(export) {
        if !profile.suspect_modules.is_empty() {
            return profile.suspect_modules.join(" | ");
        }
    }
    if export.program_findings.is_empty() {
        "none".into()
    } else {
        export
            .program_findings
            .iter()
            .map(|finding| finding.module_label.clone())
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

fn scan_target_status(export: &ExportBundle) -> ScanTargetStatus {
    if export.program_flows.is_empty() {
        ScanTargetStatus::Idle
    } else if export.program_findings.is_empty() {
        ScanTargetStatus::Healthy
    } else {
        ScanTargetStatus::Attention
    }
}

fn scan_report_json(outputs: &[(String, ExportBundle)]) -> String {
    let total_targets = outputs.len();
    let healthy_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Healthy))
        .count();
    let attention_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Attention))
        .count();
    let idle_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Idle))
        .count();
    let targets = outputs
        .iter()
        .map(|(name, export)| {
            let primary_module_kind = primary_module_kind_for_export(export);
            let primary_failure_stage = primary_failure_stage_for_export(export);
            let primary_failure_mode = primary_failure_mode_for_export(export);
            format!(
                "{{\"target\":\"{}\",\"template_id\":\"{}\",\"status\":\"{}\",\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"suspect_modules\":\"{}\",\"program_flows\":{},\"program_findings\":{},\"module_findings\":{},\"process_network_profiles\":{},\"protocol_flows\":{}}}",
                name,
                export.template_id,
                scan_target_status(export).label(),
                primary_module_kind,
                module_family_label(&primary_module_kind),
                primary_failure_stage,
                stage_family_label(&primary_failure_stage),
                primary_failure_mode,
                failure_mode_family_label(&primary_failure_mode),
                suspect_modules_for_export(export),
                export.program_flows.len(),
                export.program_findings.len(),
                export.module_findings.len(),
                process_network_profiles_json(export),
                protocol_flow_summaries_json(export),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"scan_all\":true,\"total_targets\":{},\"healthy_targets\":{},\"attention_targets\":{},\"idle_targets\":{},\"targets\":[{}]}}",
        total_targets,
        healthy_targets,
        attention_targets,
        idle_targets,
        targets
    )
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn scan_report_html(outputs: &[(String, ExportBundle)]) -> String {
    let total_targets = outputs.len();
    let healthy_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Healthy))
        .count();
    let attention_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Attention))
        .count();
    let idle_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Idle))
        .count();

    let mut family_counts = std::collections::BTreeMap::<String, usize>::new();
    for (_, export) in outputs {
        let family = module_family_label(&primary_module_kind_for_export(export)).to_string();
        *family_counts.entry(family).or_default() += 1;
    }
    let family_summary = family_counts
        .into_iter()
        .map(|(family, count)| {
            format!(
                "<div class=\"pill\"><span class=\"tag family-{}\">{}</span> {}</div>",
                family,
                family,
                count
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let mut sorted_outputs = outputs
        .iter()
        .map(|(name, export)| (name, export))
        .collect::<Vec<_>>();
    sorted_outputs.sort_by(|(left_name, left), (right_name, right)| {
        let left_rank = match scan_target_status(left) {
            ScanTargetStatus::Attention => 0,
            ScanTargetStatus::Healthy => 1,
            ScanTargetStatus::Idle => 2,
        };
        let right_rank = match scan_target_status(right) {
            ScanTargetStatus::Attention => 0,
            ScanTargetStatus::Healthy => 1,
            ScanTargetStatus::Idle => 2,
        };
        left_rank
            .cmp(&right_rank)
            .then_with(|| {
                primary_module_kind_for_export(left).cmp(&primary_module_kind_for_export(right))
            })
            .then_with(|| left_name.cmp(right_name))
    });

    let cards = sorted_outputs
        .into_iter()
        .map(|(name, export)| {
            let status = scan_target_status(export).label();
            let details_open = if matches!(scan_target_status(export), ScanTargetStatus::Attention) {
                " open"
            } else {
                ""
            };
            let profiles = process_network_profile_summaries(export)
                .into_iter()
                .map(|profile| {
                    let suspect_modules = first_or_none(&profile.suspect_modules);
                    format!(
                        "<li><strong>{}</strong> (pid={}): status={} <span class=\"tag family-{}\">{}</span> <span class=\"tag stage-{}\">{}</span> <span class=\"tag failure-{}\">{}</span> suspect_module={} kinds={} healthy_flows={} attention_flows={} phases={} missing={}</li>",
                        html_escape(&profile.comm),
                        profile.pid,
                        html_escape(&profile.status),
                        html_escape(&profile.primary_module_family),
                        html_escape(&profile.primary_module_kind),
                        html_escape(&profile.primary_stage_family),
                        html_escape(&profile.primary_failure_stage),
                        html_escape(failure_mode_family_label(&profile.primary_failure_mode)),
                        html_escape(&profile.primary_failure_mode),
                        html_escape(&suspect_modules),
                        html_escape(&profile.module_kinds.join(" | ")),
                        profile.healthy_flows,
                        profile.attention_flows,
                        html_escape(&profile.phases.join(" > ")),
                        html_escape(&profile.missing_transitions.join(" | ")),
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            let primary_module_kind = primary_module_kind_for_export(export);
            let primary_failure_stage = primary_failure_stage_for_export(export);
            let primary_failure_mode = primary_failure_mode_for_export(export);
            let suspect_modules = suspect_modules_for_export(export);
            let primary_module_family = module_family_label(&primary_module_kind);
            let primary_stage_family = stage_family_label(&primary_failure_stage);
            let primary_failure_mode_family = failure_mode_family_label(&primary_failure_mode);
            let flow_finding_summaries = protocol_flow_finding_summaries(export);
            let flow_lines = export
                .program_flows
                .iter()
                .map(|flow| {
                    let phase_text = protocol_flow_phases(flow).join(" > ");
                    let failure_mode =
                        protocol_flow_failure_mode(flow, flow_finding_summaries.get(&flow.id));
                    format!(
                        "<li>{}: last_phase={} <span class=\"tag failure-{}\">{}</span> phases={}</li>",
                        html_escape(&operation_label(&flow.operation)),
                        html_escape(&protocol_flow_last_phase(flow).unwrap_or_else(|| "none".into())),
                        html_escape(failure_mode_family_label(&failure_mode)),
                        html_escape(&failure_mode),
                        html_escape(&phase_text),
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                "<details class=\"card status-{status}\"{details_open}><summary><div class=\"card-title\"><h2>{}</h2><p><strong>status:</strong> {} | <strong>flows:</strong> {} | <strong>findings:</strong> {} | <strong>modules:</strong> {}</p></div><div class=\"conclusion\"><div class=\"pill\"><strong>primary module:</strong> <span class=\"tag family-{}\">{}</span></div><div class=\"pill\"><strong>primary stage:</strong> <span class=\"tag stage-{}\">{}</span></div><div class=\"pill\"><strong>failure mode:</strong> <span class=\"tag failure-{}\">{}</span></div><div class=\"pill\"><strong>suspect modules:</strong> {}</div></div></summary><div class=\"card-body\"><h3>Process Profiles</h3><ul>{}</ul><h3>Protocol Flows</h3><ul>{}</ul></div></details>",
                html_escape(name),
                status,
                export.program_flows.len(),
                export.program_findings.len(),
                export.module_findings.len(),
                primary_module_family,
                html_escape(&primary_module_kind),
                primary_stage_family,
                html_escape(&primary_failure_stage),
                primary_failure_mode_family,
                html_escape(&primary_failure_mode),
                html_escape(&suspect_modules),
                profiles,
                flow_lines,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>gewyvern scan report</title><style>body{{font-family:ui-sans-serif,system-ui,sans-serif;background:#f6f7fb;color:#18202a;margin:0;padding:24px}}h1,h2,h3{{margin:0 0 12px}}.summary{{display:flex;gap:12px;flex-wrap:wrap;margin:16px 0 24px}}.summary-note{{margin:-10px 0 24px;color:#475569;font-size:14px}}.pill{{background:#fff;border:1px solid #d8dee9;border-radius:999px;padding:10px 14px;font-size:14px}}.tag{{display:inline-flex;align-items:center;border-radius:999px;padding:2px 10px;font-size:12px;font-weight:600}}.family-dns{{background:#dbeafe;color:#1d4ed8}}.family-route{{background:#e0f2fe;color:#0369a1}}.family-connect{{background:#ede9fe;color:#6d28d9}}.family-handshake{{background:#fae8ff;color:#a21caf}}.family-request-response{{background:#dcfce7;color:#166534}}.family-database{{background:#fef3c7;color:#92400e}}.family-auth{{background:#fee2e2;color:#b91c1c}}.family-directory{{background:#ecfccb;color:#3f6212}}.family-messaging{{background:#ffedd5;color:#c2410c}}.family-relay{{background:#d1fae5;color:#047857}}.family-service{{background:#e2e8f0;color:#334155}}.family-general{{background:#f3f4f6;color:#374151}}.stage-dns{{background:#dbeafe;color:#1d4ed8}}.stage-connect{{background:#ede9fe;color:#6d28d9}}.stage-handshake{{background:#fae8ff;color:#a21caf}}.stage-request-response{{background:#dcfce7;color:#166534}}.stage-auth{{background:#fee2e2;color:#b91c1c}}.stage-general{{background:#f3f4f6;color:#374151}}.stage-none{{background:#e5e7eb;color:#6b7280}}.failure-blocked{{background:#fef3c7;color:#92400e}}.failure-timeout{{background:#fee2e2;color:#b91c1c}}.failure-setup{{background:#e0e7ff;color:#4338ca}}.failure-semantic{{background:#ffedd5;color:#c2410c}}.failure-denied{{background:#fce7f3;color:#be185d}}.failure-peer{{background:#d1fae5;color:#047857}}.failure-none{{background:#e5e7eb;color:#6b7280}}.failure-general{{background:#f3f4f6;color:#374151}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(320px,1fr));gap:16px}}.card{{background:#fff;border:1px solid #d8dee9;border-radius:16px;padding:0;box-shadow:0 6px 24px rgba(15,23,42,0.06);overflow:hidden}}.card summary{{list-style:none;cursor:pointer;padding:18px}}.card summary::-webkit-details-marker{{display:none}}.card-title p{{margin:0}}.card-body{{padding:0 18px 18px}}.conclusion{{display:flex;gap:10px;flex-wrap:wrap;margin:14px 0 0}}.status-attention{{border-color:#f0b429}}.status-healthy{{border-color:#68b984}}.status-idle{{border-color:#cbd5e1}}ul{{padding-left:18px}}li{{margin:6px 0}}</style></head><body><h1>gewyvern Scan Report</h1><div class=\"summary\"><div class=\"pill\">total targets: {}</div><div class=\"pill\">healthy: {}</div><div class=\"pill\">attention: {}</div><div class=\"pill\">idle: {}</div></div><p class=\"summary-note\">attention targets are shown first and expanded by default so the highest-risk paths are easier to inspect.</p><div class=\"summary\">{}</div><div class=\"grid\">{}</div></body></html>",
        total_targets,
        healthy_targets,
        attention_targets,
        idle_targets,
        family_summary,
        cards
    )
}

fn scan_report_text(outputs: &[(String, ExportBundle)]) -> String {
    let total_targets = outputs.len();
    let healthy_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Healthy))
        .count();
    let attention_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Attention))
        .count();
    let idle_targets = outputs
        .iter()
        .filter(|(_, export)| matches!(scan_target_status(export), ScanTargetStatus::Idle))
        .count();
    let mut lines = vec![format!(
        "scan_all_report: total_targets={} healthy_targets={} attention_targets={} idle_targets={}",
        total_targets, healthy_targets, attention_targets, idle_targets
    )];
    lines.extend(outputs.iter().map(|(name, export)| {
        format!(
            "{} status={} flows={} findings={} modules={} profiles={} protocol_flows={}",
            name,
            scan_target_status(export).label(),
            export.program_flows.len(),
            export.program_findings.len(),
            export.module_findings.len(),
            process_network_profiles_text(export),
            protocol_flow_summaries_text(export),
        )
    }));
    lines.join("\n")
}

fn render_scan_outputs(cli: &Cli, outputs: &[(String, ExportBundle)]) -> String {
    match cli.report_format {
        Some(ReportFormat::Html) => scan_report_html(outputs),
        Some(ReportFormat::Json) => scan_report_json(outputs),
        None if cli.json => scan_report_json(outputs),
        None => scan_report_text(outputs),
    }
}

fn render_report_outputs(cli: &Cli, outputs: &[(String, ExportBundle)]) -> String {
    match cli.report_format {
        Some(ReportFormat::Html) => scan_report_html(outputs),
        Some(ReportFormat::Json) => scan_report_json(outputs),
        None => scan_report_text(outputs),
    }
}

fn summary_json(name: &str, export: &ExportBundle) -> String {
    let primary_module_kind = primary_module_kind_for_export(export);
    let primary_failure_stage = primary_failure_stage_for_export(export);
    let primary_failure_mode = primary_failure_mode_for_export(export);
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
        "{{\"demo\":\"{name}\",\"template_id\":\"{}\",\"ingest_trust_mode\":\"{}\",\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"fragments_loaded\":{},\"hookpoints_failed\":{},\"accepted_facts\":{},\"rejected_facts\":{},\"flows\":{},\"program_findings\":{},\"module_findings\":{},\"reasons\":{},\"degraded\":{},\"suspect_modules\":{},\"protocol_flows\":{},\"process_network_profiles\":{}}}",
        export.template_id,
        export.ingest_trust_mode,
        primary_module_kind,
        module_family_label(&primary_module_kind),
        primary_failure_stage,
        stage_family_label(&primary_failure_stage),
        primary_failure_mode,
        failure_mode_family_label(&primary_failure_mode),
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
        protocol_flow_summaries_json(export),
        process_network_profiles_json(export),
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
    let primary_module_kind = primary_module_kind_for_export(export);
    let primary_failure_stage = primary_failure_stage_for_export(export);
    let primary_failure_mode = primary_failure_mode_for_export(export);
    format!(
        "{{\"demo\":\"{name}\",\"template_id\":\"{}\",\"primary_module_kind\":\"{}\",\"primary_module_family\":\"{}\",\"primary_failure_stage\":\"{}\",\"primary_stage_family\":\"{}\",\"primary_failure_mode\":\"{}\",\"primary_failure_mode_family\":\"{}\",\"module_findings\":[{}],\"program_findings\":[{}],\"process_network_profiles\":{}}}",
        export.template_id,
        primary_module_kind,
        module_family_label(&primary_module_kind),
        primary_failure_stage,
        stage_family_label(&primary_failure_stage),
        primary_failure_mode,
        failure_mode_family_label(&primary_failure_mode),
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
        process_network_profiles_json(export),
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
        "{{\"module_label\":\"{}\",\"severity\":\"{}\",\"process\":{},\"operation\":\"{}\",\"network_module_kinds\":{},\"phases\":{},\"phase_transitions\":{},\"suspect_areas\":{},\"causes\":{},\"supporting_fragments\":{},\"program_flows\":{},\"summaries\":{},\"evidence_trace\":{}}}",
        finding.module_label,
        module_severity_label(&finding.severity),
        process_json(finding.process.as_ref()),
        operation_label(&finding.operation),
        string_list_json(&finding.network_module_kinds),
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
        "{{\"program_flow\":{},\"module_label\":\"{}\",\"network_module_kind\":\"{}\",\"phase\":{},\"phase_transition\":{},\"suspect_area\":\"{}\",\"cause\":\"{}\",\"process\":{},\"operation\":\"{}\",\"summary\":\"{}\",\"supporting_fragments\":{},\"evidence_trace\":{}}}",
        finding.program_flow.0,
        finding.module_label,
        finding.network_module_kind,
        finding
            .phase
            .as_ref()
            .map_or("null".to_string(), |phase| format!("\"{}\"", phase)),
        finding
            .phase_transition
            .as_ref()
            .map_or("null".to_string(), |transition| format!(
                "\"{}\"",
                transition
            )),
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
        gewyvern::http::HttpTransactionVerdict::SuspectMultiSidedGap => "suspect_multi_sided_gap",
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
        items
            .iter()
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
    let scan_targets = scan_targets_for_cli(cli).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    #[cfg(target_family = "unix")]
    {
        remove_unix_socket_file(path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        let listener = bind_unix_socket_listener(path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

        for _ in 0..max_sessions {
            if cli.scan_all {
                let facts =
                    collect_unix_socket_facts_on_listener(&listener).unwrap_or_else(|err| {
                        eprintln!(
                            "{}",
                            locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                        );
                        std::process::exit(1);
                    });
                let mut outputs = Vec::new();
                for target in &scan_targets {
                    let export = run_binding_session(target.binding(), &facts);
                    let export = cli
                        .pid
                        .map(|pid| filter_export_by_pid(&export, pid))
                        .unwrap_or(export);
                    let export = annotate_export_trust(export, cli);
                    outputs.push((target.label(), export));
                }
                emit_scan_outputs(cli, &outputs, true);
                continue;
            }

            let export = if let Some(binding) = cli.dsl_binding() {
                run_unix_socket_session_on_listener_with_binding(&listener, binding)
            } else {
                run_unix_socket_session_on_listener(&listener, cli.template_mode.template())
            }
            .unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                );
                std::process::exit(1);
            });
            let export = cli
                .pid
                .map(|pid| filter_export_by_pid(&export, pid))
                .unwrap_or(export);
            let export = annotate_export_trust(export, cli);
            emit_rendered(cli, "socket_session", &export, true);
        }

        remove_unix_socket_file(path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
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
    let scan_targets = scan_targets_for_cli(cli).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        eprintln!(
            "{}",
            locale.msgf("socket_service_failed", &err.to_string(), None)
        );
        std::process::exit(1);
    });
    let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

    for _ in 0..max_sessions {
        if cli.scan_all {
            let facts = collect_tcp_socket_facts_on_listener(&listener).unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                );
                std::process::exit(1);
            });
            let mut outputs = Vec::new();
            for target in &scan_targets {
                let export = run_binding_session(target.binding(), &facts);
                let export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&export, pid))
                    .unwrap_or(export);
                let export = annotate_export_trust(export, cli);
                outputs.push((target.label(), export));
            }
            emit_scan_outputs(cli, &outputs, true);
            continue;
        }

        let export = if let Some(binding) = cli.dsl_binding() {
            run_tcp_socket_session_on_listener_with_binding(&listener, binding)
        } else {
            run_tcp_socket_session_on_listener(&listener, cli.template_mode.template())
        }
        .unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        let export = cli
            .pid
            .map(|pid| filter_export_by_pid(&export, pid))
            .unwrap_or(export);
        let export = annotate_export_trust(export, cli);
        emit_rendered(cli, "socket_session", &export, true);
    }
}

fn emit_rendered(cli: &Cli, name: &str, export: &ExportBundle, append: bool) {
    let locale = UiLocale::detect();
    let single = vec![(name.to_string(), export.clone())];
    let rendered = if cli.report_format.is_some() {
        render_report_outputs(cli, &single)
    } else if cli.findings {
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
                eprintln!(
                    "{}",
                    locale.msgf("write_failed", path, Some(&err.to_string()))
                );
                std::process::exit(1);
            });
        } else {
            fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("write_failed", path, Some(&err.to_string()))
                );
                std::process::exit(1);
            });
        }
    } else {
        println!("{rendered}");
    }
}

fn emit_scan_outputs(cli: &Cli, outputs: &[(String, ExportBundle)], append: bool) {
    let locale = UiLocale::detect();
    let rendered = render_scan_outputs(cli, outputs);
    if let Some(path) = cli.out_path.as_deref() {
        if append {
            let mut existing = fs::read_to_string(path).unwrap_or_default();
            existing.push_str(&rendered);
            existing.push('\n');
            fs::write(path, existing).unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("write_failed", path, Some(&err.to_string()))
                );
                std::process::exit(1);
            });
        } else {
            fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
                eprintln!(
                    "{}",
                    locale.msgf("write_failed", path, Some(&err.to_string()))
                );
                std::process::exit(1);
            });
        }
    } else {
        println!("{rendered}");
    }
}
