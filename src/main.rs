mod data_api;
mod diagnosis_runtime;
mod external_analysis;
mod render_utils;
mod report_runtime;
mod serve_runtime;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::{FlowId, ProcessView, ProgramFlowId};
use gewyvern::gewyc::{RenderFormat, compile_diagnostics_report_file, render_diagnostics_report};
use gewyvern::http::compose_http_transactions;
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
use std::collections::HashSet;
use std::env;
use std::fs;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::diagnosis_runtime::*;
use crate::external_analysis::{ExternalAnalysisConfig, set_external_analysis_config};
use crate::report_runtime::{
    findings_json, findings_text, http_transactions_json, http_transactions_text,
    render_report_outputs, render_scan_outputs, scan_report_html, scan_report_json,
    scan_report_text, summary_json, summary_line,
};
use crate::serve_runtime::serve_socket_sessions;

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
                "用法: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::Ja => {
                "使い方: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::Ko => {
                "사용법: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::Fr => {
                "Utilisation : gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::De => {
                "Verwendung: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::Es => {
                "Uso: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::Pt => {
                "Uso: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::Ru => {
                "Использование: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
            }
            Self::En => {
                "usage: gewyvern [--demo tcp|udp|both] [--dsl path|--protocol name [--entry mode]|--scan-all [--protocol-set path] [--report-format html|json]] [--list-protocols|--list-entries protocol] [--pid n] [--diagnostics] [--findings] [--http-transactions] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--ingest-mode local-advisory|remote-advisory|--socket-trust trusted-local|unsafe-remote|--allow-remote-socket] [--serve] [--api-socket host:port] [--allow-remote-api] [--max-sessions n] [--json] [--summary-only] [--out path] [--external-engine-bin path [--external-engine-worker path] [--external-engine-python-bin path]]"
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
                format!("不支持的 ingest 运行模式 '{a}'，期望 local-advisory 或 remote-advisory")
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

fn main() {
    let locale = UiLocale::detect();
    let cli = Cli::from_args(env::args().skip(1)).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    set_external_analysis_config(cli.external_analysis_config());

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
    ingest_mode: IngestMode,
    pid: Option<u32>,
    diagnostics: bool,
    findings: bool,
    http_transactions: bool,
    serve: bool,
    api_socket: Option<String>,
    allow_remote_api: bool,
    max_sessions: Option<usize>,
    json: bool,
    report_format: Option<ReportFormat>,
    summary_only: bool,
    out_path: Option<String>,
    socket_target: Option<SocketTarget>,
    external_engine_bin: Option<String>,
    external_engine_worker: Option<String>,
    external_engine_python_bin: Option<String>,
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
enum IngestMode {
    LocalAdvisory,
    RemoteAdvisory,
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

impl IngestMode {
    fn from_str(value: &str) -> Result<Self, String> {
        let locale = UiLocale::detect();
        match value {
            "local-advisory" | "local" => Ok(Self::LocalAdvisory),
            "remote-advisory" | "remote" => Ok(Self::RemoteAdvisory),
            other => Err(locale.msgf("unsupported_ingest_mode", other, None)),
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
        let mut ingest_mode = IngestMode::LocalAdvisory;
        let mut pid = None;
        let mut diagnostics = false;
        let mut findings = false;
        let mut http_transactions = false;
        let mut serve = false;
        let mut api_socket = None;
        let mut allow_remote_api = false;
        let mut max_sessions = None;
        let mut json = false;
        let mut report_format = None;
        let mut summary_only = false;
        let mut out_path = None;
        let mut socket_target = None;
        let mut external_engine_bin = None;
        let mut external_engine_worker = None;
        let mut external_engine_python_bin = None;
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
                "--allow-remote-api" => allow_remote_api = true,
                "--api-socket" => {
                    api_socket = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_api_socket", "", None))?,
                    );
                }
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
                "--allow-remote-socket" => ingest_mode = IngestMode::RemoteAdvisory,
                "--ingest-mode" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_ingest_mode", "", None))?;
                    ingest_mode = IngestMode::from_str(&value)?;
                }
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
                    ingest_mode = match value.as_str() {
                        "trusted-local" | "local" => IngestMode::LocalAdvisory,
                        "unsafe-remote" | "remote" => IngestMode::RemoteAdvisory,
                        other => {
                            return Err(locale.msgf("unsupported_socket_trust", other, None));
                        }
                    };
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
                "--external-engine-bin" | "--etragon-bin" => {
                    external_engine_bin =
                        Some(args.next().ok_or_else(|| {
                            "missing value for --external-engine-bin".to_string()
                        })?);
                }
                "--external-engine-worker" | "--etragon-python-worker" => {
                    external_engine_worker =
                        Some(args.next().ok_or_else(|| {
                            "missing value for --external-engine-worker".to_string()
                        })?);
                }
                "--external-engine-python-bin" | "--etragon-python-bin" => {
                    external_engine_python_bin = Some(args.next().ok_or_else(|| {
                        "missing value for --external-engine-python-bin".to_string()
                    })?);
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
        if socket_target.is_some() && pid.is_some() {
            return Err(locale.msg("pid_socket_conflict").into());
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
        if api_socket.is_some() && !serve {
            return Err(locale.msg("api_requires_serve").into());
        }
        if api_socket
            .as_deref()
            .is_some_and(|addr| !allow_remote_api && !api_socket_addr_is_local(addr))
        {
            return Err(locale.msg("remote_api_requires_flag").into());
        }
        if matches!(socket_target, Some(SocketTarget::Tcp(_)))
            && ingest_mode != IngestMode::RemoteAdvisory
            && socket_target
                .as_ref()
                .is_some_and(|target| !socket_target_is_local(target))
        {
            return Err(locale.msg("remote_socket_requires_flag").into());
        }
        if external_engine_worker.is_some() && external_engine_bin.is_none() {
            return Err("--external-engine-worker requires --external-engine-bin".into());
        }
        if external_engine_python_bin.is_some() && external_engine_worker.is_none() {
            return Err("--external-engine-python-bin requires --external-engine-worker".into());
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
            ingest_mode,
            pid,
            diagnostics,
            findings,
            http_transactions,
            serve,
            api_socket,
            allow_remote_api,
            max_sessions,
            json,
            report_format,
            summary_only,
            out_path,
            socket_target,
            external_engine_bin,
            external_engine_worker,
            external_engine_python_bin,
        })
    }

    fn external_analysis_config(&self) -> Option<ExternalAnalysisConfig> {
        self.external_engine_bin
            .as_ref()
            .map(|engine_bin| ExternalAnalysisConfig {
                engine_bin: engine_bin.clone(),
                python_worker: self.external_engine_worker.clone(),
                python_bin: self.external_engine_python_bin.clone(),
            })
    }
}

fn process_matches_pid(process: Option<&ProcessView>, pid: u32) -> bool {
    process.is_some_and(|process| process.pid == pid)
}

fn ingest_trust_mode_for_cli(cli: &Cli) -> &'static str {
    match cli.socket_target {
        Some(_) => match cli.ingest_mode {
            IngestMode::LocalAdvisory => "unverified-local",
            IngestMode::RemoteAdvisory => "unverified-remote",
        },
        None => "synthetic-demo",
    }
}

fn annotate_export_trust(mut export: ExportBundle, cli: &Cli) -> ExportBundle {
    export.ingest_trust_mode = ingest_trust_mode_for_cli(cli).to_string();
    export
}

fn ingest_mode_for_export(export: &ExportBundle) -> &'static str {
    match export.ingest_trust_mode.as_str() {
        "synthetic-demo" => "demo",
        "unverified-local" => "local-advisory",
        "unverified-remote" => "remote-advisory",
        _ => "unknown",
    }
}

fn ingest_mode_note_for_export(export: &ExportBundle) -> &'static str {
    match ingest_mode_for_export(export) {
        "demo" => {
            "synthetic demo mode: useful for exercising flows and reports, not for real process attribution"
        }
        "local-advisory" => {
            "local advisory mode: facts come from a local socket source, but lineage is still unverified"
        }
        "remote-advisory" => {
            "remote advisory mode: facts come from an explicitly enabled remote socket source and should be treated as unverified"
        }
        _ => "ingest mode could not be classified; treat process-level conclusions conservatively",
    }
}

fn pid_attribution_status_for_export(export: &ExportBundle) -> &'static str {
    match export.ingest_trust_mode.as_str() {
        "synthetic-demo" => "synthetic",
        "unverified-local" | "unverified-remote" => "unverified",
        _ => "unknown",
    }
}

fn pid_attribution_note_for_export(export: &ExportBundle) -> &'static str {
    match export.ingest_trust_mode.as_str() {
        "synthetic-demo" => "pid-scoped conclusions come from synthetic demo lineage",
        "unverified-local" | "unverified-remote" => {
            "pid-scoped conclusions are advisory only because ingest lineage is unverified"
        }
        _ => "pid attribution status is unknown",
    }
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

fn api_socket_addr_is_local(addr: &str) -> bool {
    tcp_bind_addr_is_local(addr)
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
    let is_socks5_session = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "socks5_session"
            )
        });
    let is_http_connect_tunnel = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_connect_tunnel"
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
        } else if is_socks5_session {
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
                        sk_cookie: 155,
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
                route_fact(2, base + Duration::from_millis(10), 155, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 155,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54000,
                        dport: 1080,
                        family: 2,
                        old: 1,
                        new: 2,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 155,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54000,
                        dport: 1080,
                        family: 2,
                        old: 2,
                        new: 3,
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Egress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x01),
                        payload_prefix2: Some(0x0501),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x01),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 80,
                        tcp_flags: 0x18,
                        seq: Some(1),
                        ack: Some(1),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Ingress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x00),
                        payload_prefix2: Some(0x0500),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x00),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 80,
                        tcp_flags: 0x18,
                        seq: Some(2),
                        ack: Some(2),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Egress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x01),
                        payload_prefix2: Some(0x0501),
                        payload_prefix4: Some(0x05010003),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x01),
                            (2u16, 0x00),
                            (3u16, 0x03),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 92,
                        tcp_flags: 0x18,
                        seq: Some(3),
                        ack: Some(3),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Ingress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x00),
                        payload_prefix2: Some(0x0500),
                        payload_prefix4: Some(0x05000001),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x00),
                            (2u16, 0x00),
                            (3u16, 0x01),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 92,
                        tcp_flags: 0x18,
                        seq: Some(4),
                        ack: Some(4),
                        window: Some(65535),
                    }),
                },
            ]
        } else if is_http_connect_tunnel {
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
                        sk_cookie: 166,
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
                route_fact(2, base + Duration::from_millis(10), 166, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 166,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54100,
                        dport: 8080,
                        family: 2,
                        old: 1,
                        new: 2,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 166,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54100,
                        dport: 8080,
                        family: 2,
                        old: 2,
                        new: 3,
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(166),
                        dir: PacketDir::Egress,
                        local_port: Some(54100),
                        remote_port: Some(8080),
                        payload_byte0: Some(0x43),
                        payload_byte1: Some(0x4f),
                        payload_prefix2: Some(0x434f),
                        payload_prefix4: Some(0x434f4e4e),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x43),
                            (1u16, 0x4f),
                            (2u16, 0x4e),
                            (3u16, 0x4e),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 110,
                        tcp_flags: 0x18,
                        seq: Some(1),
                        ack: Some(1),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(166),
                        dir: PacketDir::Ingress,
                        local_port: Some(54100),
                        remote_port: Some(8080),
                        payload_byte0: Some(0x32),
                        payload_byte1: Some(0x30),
                        payload_prefix2: Some(0x3230),
                        payload_prefix4: Some(0x32303020),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x32),
                            (1u16, 0x30),
                            (2u16, 0x30),
                            (3u16, 0x20),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 96,
                        tcp_flags: 0x18,
                        seq: Some(2),
                        ack: Some(2),
                        window: Some(65535),
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
    use super::data_api::{
        ApiRenderedTarget, ApiSnapshot, api_response_for_request, api_snapshot_meta_json,
        update_api_snapshot_for_scan, update_api_snapshot_for_single,
    };
    use super::external_analysis::{
        ExternalAnalysisConfig, set_external_analysis_config, test_guard,
    };
    use super::{
        AnalysisAugmenter, AnalysisSnapshot, Cli, IngestMode, ReportFormat, analysis_snapshot,
        analysis_snapshot_json, analysis_snapshot_with_augmenters, annotate_export_trust,
        filter_export_by_pid, findings_json, list_entries_json, list_entries_text,
        list_protocols_json, list_protocols_text, protocol_dsl_path, push_analysis_augmentation,
        render_report_outputs, route_fact, run_binding_demo, scan_report_html, scan_report_json,
        scan_report_text, scan_targets_for_cli, scan_targets_from_set_file, summary_json,
        summary_line,
    };
    use gewyvern::dsl::compile_file;
    use gewyvern::export::ExportBundle;
    use gewyvern::flow::{ProcessView, ProgramFinding, ProgramFindingCause, ProgramOperation};
    use gewyvern::ledger::{
        CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
        SockLineageFact, TcpStateFact,
    };
    use gewyvern::runtime::{RuntimeSession, SessionConfig};
    use gewyvern::template::TemplateBinding;
    use std::fs;
    #[cfg(target_family = "unix")]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::time::Instant;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(target_family = "unix")]
    fn with_fake_etragon_hook<T>(output_json: &str, test: impl FnOnce() -> T) -> T {
        let _guard = test_guard();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let script_path = std::env::temp_dir().join(format!("fake-etragon-{unique}.sh"));
        fs::write(
            &script_path,
            format!(
                "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{}'\n",
                output_json
            ),
        )
        .expect("fake etragon hook should be writable");
        let mut permissions = fs::metadata(&script_path)
            .expect("fake etragon hook should exist")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script_path, permissions)
            .expect("fake etragon hook should be executable");
        set_external_analysis_config(Some(ExternalAnalysisConfig {
            engine_bin: script_path.to_string_lossy().into_owned(),
            python_worker: None,
            python_bin: None,
        }));
        let outcome = test();
        set_external_analysis_config(None);
        let _ = fs::remove_file(&script_path);
        outcome
    }

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
    fn analysis_snapshot_supports_composable_augmenters() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let augmenter = MlHookAugmenter;
        let snapshot = analysis_snapshot_with_augmenters(&export, &[&augmenter]);

        assert_eq!(snapshot.primary_failure_confidence, "ml-candidate");
        assert!(
            snapshot
                .competing_hypotheses
                .contains(&"augmenter:ml_rerank_hook".to_string()),
            "augmenters should be able to enrich the shared analysis snapshot",
        );
        assert!(
            snapshot
                .augmentations
                .iter()
                .any(|item| item.name == "ml_rerank_hook"),
            "external augmenters should append custom machine-readable annotations"
        );
        assert!(
            snapshot
                .augmentations
                .iter()
                .any(|item| item.name == "automation_recommendation"),
            "built-in augmenters should remain active when external augmenters are composed"
        );
        let json = analysis_snapshot_json(&snapshot);
        assert!(json.contains("\"augmentations\":["));
        assert!(json.contains("\"name\":\"ml_rerank_hook\""));
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn analysis_snapshot_merges_external_etragon_augmentations() {
        with_fake_etragon_hook(
            "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_targeted_escalation\",\"summary\":\"external engine suggests targeted escalation\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"http_request_response\"}}]}",
            || {
                let binding =
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                        .expect("http_request_path DSL should compile");
                let export = annotate_export_trust(
                    run_binding_demo(binding),
                    &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
                );
                let snapshot = analysis_snapshot(&export);
                let json = analysis_snapshot_json(&snapshot);
                assert!(
                    snapshot
                        .augmentations
                        .iter()
                        .any(|item| item.name == "ml_candidate_targeted_escalation")
                );
                assert!(json.contains("\"producer_pass\":\"fake_etragon\""));
                assert!(json.contains("\"name\":\"ml_candidate_targeted_escalation\""));
            },
        );
    }

    #[cfg(target_family = "unix")]
    #[test]
    fn summary_and_findings_json_expose_external_augmentations() {
        with_fake_etragon_hook(
            "{\"augmentations\":[{\"kind\":\"ml-candidate\",\"name\":\"ml_candidate_manual_review\",\"summary\":\"external engine suggests manual review\",\"confidence\":\"candidate\",\"producer_stage\":\"candidate\",\"producer_pass\":\"fake_etragon\",\"data\":{\"module\":\"connection_establishment\"}}]}",
            || {
                let binding =
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                        .expect("http_request_path DSL should compile");
                let export = annotate_export_trust(
                    run_binding_demo(binding),
                    &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
                );
                let summary = summary_json("dsl_demo", &export);
                let findings = findings_json("dsl_demo", &export);
                assert!(summary.contains("\"augmentations\":["));
                assert!(summary.contains("\"name\":\"ml_candidate_manual_review\""));
                assert!(findings.contains("\"augmentations\":["));
                assert!(findings.contains("\"producer_pass\":\"fake_etragon\""));
            },
        );
    }

    #[test]
    fn analysis_snapshot_adds_unverified_ingest_augmentation() {
        let cli =
            Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();
        let export = annotate_export_trust(
            run_binding_demo(
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile"),
            ),
            &cli,
        );
        let snapshot = analysis_snapshot(&export);
        let json = analysis_snapshot_json(&snapshot);
        assert!(
            snapshot
                .augmentations
                .iter()
                .any(|item| item.name == "unverified_ingest_lineage"),
            "snapshot should expose an advisory trust augmentation"
        );
        assert!(json.contains("\"name\":\"unverified_ingest_lineage\""));
        assert!(json.contains("\"kind\":\"trust\""));
        assert!(json.contains("\"name\":\"automation_recommendation\""));
        assert!(json.contains("\"action\":\"avoid_pid_strong_actions\""));
    }

    #[test]
    fn analysis_snapshot_adds_competing_hypotheses_augmentation() {
        let process = synthetic_process_view(9101, "curl");
        let dns_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy")
                        .expect("dns_udp_process DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
            ),
            &process,
        );
        let mut http_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                        .expect("http_request_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            ),
            &process,
        );
        let http_flow = http_export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut http_export,
            &http_flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let export = merge_exports_for_tests(vec![dns_export, http_export]);
        let snapshot = analysis_snapshot(&export);
        let json = analysis_snapshot_json(&snapshot);
        assert!(
            snapshot
                .augmentations
                .iter()
                .any(|item| item.name == "competing_hypotheses"),
            "snapshot should expose an advisory ambiguity augmentation"
        );
        assert!(json.contains("\"name\":\"competing_hypotheses\""));
        assert!(json.contains("\"kind\":\"analysis\""));
        assert!(json.contains("\"name\":\"automation_recommendation\""));
        assert!(json.contains("\"action\":\"keep_multiple_hypotheses\""));
    }

    #[test]
    fn analysis_snapshot_adds_missing_transition_recommendation() {
        let mut export = annotate_export_trust(
            run_binding_demo(
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let snapshot = analysis_snapshot(&export);
        let json = analysis_snapshot_json(&snapshot);
        assert!(
            snapshot
                .augmentations
                .iter()
                .any(|item| item.name == "automation_recommendation"),
            "snapshot should expose an automation-friendly recommendation augmentation"
        );
        assert!(json.contains("\"action\":\"collect_more_runtime_evidence\""));
        assert!(json.contains("\"reason\":\"missing_transition\""));
    }

    struct MlHookAugmenter;

    impl AnalysisAugmenter for MlHookAugmenter {
        fn augment(&self, _export: &ExportBundle, snapshot: &mut AnalysisSnapshot) {
            snapshot.primary_failure_confidence = "ml-candidate".into();
            snapshot
                .competing_hypotheses
                .push("augmenter:ml_rerank_hook".into());
            push_analysis_augmentation(
                snapshot,
                "ml-hook",
                "ml_rerank_hook",
                "placeholder augmentation slot for future rerank/enrich passes",
                "advisory",
                Some("candidate".into()),
                Some("MlHookAugmenter".into()),
                Some("{\"source\":\"test\"}".into()),
            );
        }
    }

    fn push_synthetic_missing_stage_finding(
        export: &mut gewyvern::export::ExportBundle,
        flow: &gewyvern::flow::ProgramFlow,
        module_label: &str,
        network_module_kind: &str,
        phase: &str,
        phase_kind: &str,
        phase_transition: &str,
        phase_transition_kind: &str,
        suspect_area: &str,
        summary: &str,
        supporting_fragment: &str,
        evidence_trace: &str,
    ) {
        export.program_findings.push(ProgramFinding {
            program_flow: flow.id,
            process: flow.process.clone(),
            operation: flow.operation.clone(),
            module_label: module_label.into(),
            network_module_kind: network_module_kind.into(),
            phase: Some(phase.into()),
            phase_kind: Some(phase_kind.into()),
            phase_transition: Some(phase_transition.into()),
            phase_transition_kind: Some(phase_transition_kind.into()),
            suspect_area: suspect_area.into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: summary.into(),
            supporting_fragments: vec![supporting_fragment.into()],
            evidence_trace: vec![evidence_trace.into()],
        });
    }

    fn synthetic_process_view(pid: u32, comm: &str) -> ProcessView {
        ProcessView {
            pid,
            tid: pid,
            cgroup_id: 4242,
            comm: comm.into(),
        }
    }

    fn coerce_export_process(
        mut export: gewyvern::export::ExportBundle,
        process: &ProcessView,
    ) -> gewyvern::export::ExportBundle {
        for flow in &mut export.program_flows {
            flow.process = Some(process.clone());
        }
        for finding in &mut export.program_findings {
            finding.process = Some(process.clone());
        }
        for finding in &mut export.module_findings {
            finding.process = Some(process.clone());
        }
        export
    }

    fn merge_exports_for_tests(
        exports: Vec<gewyvern::export::ExportBundle>,
    ) -> gewyvern::export::ExportBundle {
        let mut iter = exports.into_iter();
        let mut merged = iter.next().expect("expected at least one export");
        for export in iter {
            merged.facts.extend(export.facts);
            merged.rejected_facts.extend(export.rejected_facts);
            merged
                .rejected_fact_summary
                .extend(export.rejected_fact_summary);
            merged.flows.extend(export.flows);
            merged.program_flows.extend(export.program_flows);
            merged.program_findings.extend(export.program_findings);
            merged.module_findings.extend(export.module_findings);
            merged.reasons.extend(export.reasons);
        }
        merged.debug_summary.accepted_facts = merged.facts.len() as u64;
        merged.debug_summary.rejected_facts = merged.rejected_facts.len() as u64;
        merged.debug_summary.flows = merged.flows.len() as u64;
        merged.debug_summary.program_flows = merged.program_flows.len() as u64;
        merged.debug_summary.program_findings = merged.program_findings.len() as u64;
        merged.debug_summary.module_findings = merged.module_findings.len() as u64;
        merged.debug_summary.reasons = merged.reasons.len() as u64;
        merged
    }

    fn sock_lineage_fact_for_tests(id: u64, cookie: u64, pid: u32, comm: &str) -> FactEnvelope {
        let mut comm_bytes = [0u8; 16];
        let bytes = comm.as_bytes();
        let len = bytes.len().min(comm_bytes.len());
        comm_bytes[..len].copy_from_slice(&bytes[..len]);

        FactEnvelope {
            id: FactId(id),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(1),
            fragment_id: "sock_lineage_fragment".into(),
            kind: FactKind::SockLineage(SockLineageFact {
                netns: 1,
                sk_cookie: cookie,
                pid,
                tid: pid,
                cgroup_id: 4242,
                comm: comm_bytes,
            }),
        }
    }

    fn tcp_state_fact_with_ports_for_tests(
        id: u64,
        cookie: u64,
        old: u8,
        new: u8,
        sport: u16,
        dport: u16,
    ) -> FactEnvelope {
        FactEnvelope {
            id: FactId(id),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(1),
            fragment_id: "tcp_state_fragment".into(),
            kind: FactKind::TcpState(TcpStateFact {
                netns: 1,
                sk_cookie: cookie,
                saddr: [0; 16],
                daddr: [0; 16],
                sport,
                dport,
                family: 2,
                old,
                new,
            }),
        }
    }

    fn packet_fact_with_dir_and_payload_for_tests(
        id: u64,
        cookie: u64,
        tcp_flags: u16,
        dir: PacketDir,
        local_port: Option<u16>,
        remote_port: Option<u16>,
        payload_byte0: Option<u8>,
        payload_prefix2: Option<u16>,
        payload_prefix4: Option<u32>,
    ) -> FactEnvelope {
        FactEnvelope {
            id: FactId(id),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(1),
            fragment_id: "tcp_packet_meta_fragment".into(),
            kind: FactKind::PacketMeta(PacketMetaFact {
                netns: 1,
                sk_cookie: Some(cookie),
                dir,
                local_port: local_port.or(Some(42310)),
                remote_port: remote_port.or(Some(443)),
                payload_byte0,
                payload_byte1: None,
                payload_prefix2,
                payload_prefix4,
                payload_byte4: None,
                payload_byte5: None,
                payload_byte9: None,
                payload_byte10: None,
                payload_byte13: None,
                payload_bytes: std::collections::BTreeMap::new(),
                l3_proto: 0x0800,
                l4_proto: 6,
                tot_len: 60,
                tcp_flags,
                seq: Some(id as u32),
                ack: None,
                window: Some(65535),
            }),
        }
    }

    fn packet_fact_with_dir_and_payload_bytes_for_tests(
        id: u64,
        cookie: u64,
        tcp_flags: u16,
        dir: PacketDir,
        local_port: Option<u16>,
        remote_port: Option<u16>,
        payload_bytes: &[(u16, u8)],
    ) -> FactEnvelope {
        let byte_at = |target: u16| {
            payload_bytes
                .iter()
                .find_map(|(offset, value)| (*offset == target).then_some(*value))
        };
        let payload_byte0 = byte_at(0);
        let payload_byte1 = byte_at(1);
        let payload_byte2 = byte_at(2);
        let payload_byte3 = byte_at(3);
        FactEnvelope {
            id: FactId(id),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(1),
            fragment_id: "tcp_packet_meta_fragment".into(),
            kind: FactKind::PacketMeta(PacketMetaFact {
                netns: 1,
                sk_cookie: Some(cookie),
                dir,
                local_port: local_port.or(Some(42310)),
                remote_port: remote_port.or(Some(443)),
                payload_byte0,
                payload_byte1,
                payload_prefix2: payload_byte0
                    .zip(payload_byte1)
                    .map(|(b0, b1)| u16::from_be_bytes([b0, b1])),
                payload_prefix4: payload_byte0
                    .zip(payload_byte1)
                    .zip(payload_byte2)
                    .zip(payload_byte3)
                    .map(|(((b0, b1), b2), b3)| u32::from_be_bytes([b0, b1, b2, b3])),
                payload_byte4: byte_at(4),
                payload_byte5: byte_at(5),
                payload_byte9: byte_at(9),
                payload_byte10: byte_at(10),
                payload_byte13: byte_at(13),
                payload_bytes: payload_bytes.iter().copied().collect(),
                l3_proto: 0x0800,
                l4_proto: 6,
                tot_len: 60,
                tcp_flags,
                seq: Some(id as u32),
                ack: None,
                window: Some(65535),
            }),
        }
    }

    fn udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
        id: u64,
        cookie: u64,
        tot_len: u32,
        dir: PacketDir,
        local_port: Option<u16>,
        remote_port: Option<u16>,
        payload_byte0: Option<u8>,
        payload_prefix4: Option<u32>,
    ) -> FactEnvelope {
        FactEnvelope {
            id: FactId(id),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(1),
            fragment_id: "udp_packet_meta_fragment".into(),
            kind: FactKind::PacketMeta(PacketMetaFact {
                netns: 1,
                sk_cookie: Some(cookie),
                dir,
                local_port,
                remote_port,
                payload_byte0,
                payload_byte1: None,
                payload_prefix2: None,
                payload_prefix4,
                payload_byte4: None,
                payload_byte5: None,
                payload_byte9: None,
                payload_byte10: None,
                payload_byte13: None,
                payload_bytes: std::collections::BTreeMap::new(),
                l3_proto: 0x0800,
                l4_proto: 17,
                tot_len,
                tcp_flags: 0,
                seq: None,
                ack: None,
                window: None,
            }),
        }
    }

    fn export_from_test_facts(binding: TemplateBinding, facts: Vec<FactEnvelope>) -> ExportBundle {
        let config = SessionConfig::for_binding(binding).expect("binding should validate");
        let mut session = RuntimeSession::start(config).expect("session should start");
        for fact in facts {
            session.ingest(fact);
        }
        session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
        session.export_bundle()
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
        assert_eq!(cli.ingest_mode, IngestMode::RemoteAdvisory);
    }

    #[test]
    fn cli_accepts_loopback_tcp_socket_without_remote_flag() {
        let cli =
            Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();
        assert_eq!(cli.ingest_mode, IngestMode::LocalAdvisory);
    }

    #[test]
    fn cli_accepts_explicit_ingest_mode() {
        let cli = Cli::from_args([
            "--tcp-socket".to_string(),
            "0.0.0.0:9000".to_string(),
            "--ingest-mode".to_string(),
            "remote-advisory".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.ingest_mode, IngestMode::RemoteAdvisory);
    }

    #[test]
    fn cli_accepts_legacy_socket_trust_alias() {
        let cli = Cli::from_args([
            "--tcp-socket".to_string(),
            "0.0.0.0:9000".to_string(),
            "--socket-trust".to_string(),
            "unsafe-remote".to_string(),
        ])
        .unwrap();
        assert_eq!(cli.ingest_mode, IngestMode::RemoteAdvisory);
    }

    #[test]
    fn cli_rejects_unknown_ingest_mode() {
        let err = Cli::from_args(["--ingest-mode".to_string(), "mystery".to_string()]).unwrap_err();
        assert!(err.contains("ingest mode") || err.contains("采集模式"));
    }

    #[test]
    fn cli_rejects_pid_filter_for_socket_ingest() {
        let err = Cli::from_args([
            "--tcp-socket".to_string(),
            "127.0.0.1:9000".to_string(),
            "--pid".to_string(),
            "4242".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("--pid"));
        assert!(err.contains("socket"));
    }

    #[test]
    fn cli_rejects_api_socket_without_serve() {
        let err = Cli::from_args([
            "--tcp-socket".to_string(),
            "127.0.0.1:9000".to_string(),
            "--api-socket".to_string(),
            "127.0.0.1:9100".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("--api-socket"));
        assert!(err.contains("--serve"));
    }

    #[test]
    fn cli_rejects_remote_api_socket_without_explicit_flag() {
        let err = Cli::from_args([
            "--tcp-socket".to_string(),
            "127.0.0.1:9000".to_string(),
            "--serve".to_string(),
            "--api-socket".to_string(),
            "0.0.0.0:9100".to_string(),
        ])
        .unwrap_err();
        assert!(err.contains("--allow-remote-api"));
    }

    #[test]
    fn cli_accepts_remote_api_socket_with_explicit_flag() {
        let cli = Cli::from_args([
            "--tcp-socket".to_string(),
            "127.0.0.1:9000".to_string(),
            "--serve".to_string(),
            "--api-socket".to_string(),
            "0.0.0.0:9100".to_string(),
            "--allow-remote-api".to_string(),
        ])
        .expect("explicit remote api opt-in should be accepted");
        assert_eq!(cli.api_socket.as_deref(), Some("0.0.0.0:9100"));
        assert!(cli.allow_remote_api);
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
        assert!(json.contains("\"ingest_mode\":\"demo\""));
        assert!(json.contains("\"ingest_mode_note\":\"synthetic demo mode: useful for exercising flows and reports, not for real process attribution\""));
        assert!(json.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
    }

    #[test]
    fn summary_json_marks_socket_ingest_as_unverified_local() {
        let cli =
            Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(run_binding_demo(binding), &cli);
        let json = summary_json("socket_session", &export);
        assert!(json.contains("\"ingest_mode\":\"local-advisory\""));
        assert!(json.contains("\"ingest_mode_note\":\"local advisory mode: facts come from a local socket source, but lineage is still unverified\""));
        assert!(json.contains("\"ingest_trust_mode\":\"unverified-local\""));
        assert!(json.contains("\"pid_attribution_status\":\"unverified\""));
        assert!(json.contains(
            "\"pid_attribution_note\":\"pid-scoped conclusions are advisory only because ingest lineage is unverified\""
        ));
    }

    #[test]
    fn summary_json_exposes_single_object_identity_fields() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"kind\":\"single\""));
        assert!(json.contains("\"name\":\"dsl_demo\""));
        assert!(json.contains("\"demo\":\"dsl_demo\""));
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
        assert!(report.contains("\"kind\":\"scan\""));
        assert!(report.contains("\"name\":null"));
        assert!(report.contains("\"target_count\":2"));
        assert!(report.contains("\"scan_all\":true"));
        assert!(report.contains("\"total_targets\":2"));
        assert!(report.contains("\"healthy_targets\":1"));
        assert!(report.contains("\"attention_targets\":1"));
        assert!(report.contains("\"target\":\"scan:http:request\""));
        assert!(report.contains("\"target\":\"scan:http:response\""));
        assert!(report.contains("\"ingest_mode\":\"demo\""));
        assert!(report.contains("\"ingest_mode_note\":\"synthetic demo mode: useful for exercising flows and reports, not for real process attribution\""));
        assert!(report.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
        assert!(report.contains("\"pid_attribution_status\":\"synthetic\""));
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
        assert!(report.contains("failure detail:"));
        assert!(report.contains("suspect modules:"));
        assert!(report.contains("mode:</strong> demo"));
        assert!(report.contains("Mode note:</strong> synthetic demo mode: useful for exercising flows and reports, not for real process attribution"));
        assert!(report.contains("trust:</strong> synthetic-demo"));
        assert!(report.contains("pid attribution:</strong> synthetic"));
        assert!(report.contains(
            "PID attribution note:</strong> pid-scoped conclusions come from synthetic demo lineage"
        ));
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
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy")
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
        assert_eq!(
            crate::primary_failure_mode_for_export(&export),
            "no_response"
        );
        assert_eq!(
            crate::primary_failure_detail_for_export(&export),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::suspect_modules_for_export(&export),
            "http_request_path"
        );
    }

    #[test]
    fn single_target_json_report_wraps_protocol_result() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy")
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
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
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
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
        assert!(json.contains("\"failure_mode\":\"no_response\""));
        assert!(json.contains("\"failure_detail\":\"request_sent_no_reply\""));
    }

    #[test]
    fn failure_mode_label_classifies_database_directory_quic_tls_http3_and_hy2_families() {
        assert_eq!(
            crate::failure_mode_label("attention", "database_error_handling", "receive_error", &[],),
            "semantic_error"
        );
        assert_eq!(
            crate::failure_mode_label("attention", "directory_write", "receive_modify_denied", &[],),
            "server_denied"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "proxy_negotiation",
                "receive_connect_denied",
                &[],
            ),
            "server_denied"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "proxy_tunnel_establishment",
                "receive_connect_denied",
                &[],
            ),
            "server_denied"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "proxy_authentication",
                "receive_auth_required",
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
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "remote_access_session",
                "connect->receive_server_banner",
                &["transport_io".into()],
            ),
            "setup_incomplete"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "remote_access_session",
                "receive_server_banner->send_key_exchange_init",
                &["transport_io".into()],
            ),
            "not_sent"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "authentication_exchange",
                "connect->receive_banner",
                &["transport_io".into()],
            ),
            "setup_incomplete"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "directory_write",
                "receive_modify_denied",
                &[],
            ),
            "access_denied"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_negotiation",
                "receive_connect_denied",
                &[],
            ),
            "access_denied"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_tunnel_establishment",
                "receive_connect_denied",
                &[],
            ),
            "access_denied"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_authentication",
                "receive_auth_required",
                &[],
            ),
            "auth_required"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_negotiation",
                "send_connect_request->receive_connect_success",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_tunnel_establishment",
                "send_connect_request->receive_connect_established",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "quic_handshake",
                "send_initial->receive_handshake",
                &[],
            ),
            "handshake_incomplete"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "remote_access_session",
                "connect->receive_server_banner",
                &["transport_io".into()],
            ),
            "handshake_incomplete"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "remote_access_session",
                "receive_server_banner->send_key_exchange_init",
                &["transport_io".into()],
            ),
            "followup_not_sent"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "authentication_exchange",
                "connect->receive_banner",
                &["transport_io".into()],
            ),
            "handshake_incomplete"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "authentication_exchange",
                "receive_password_required->send_auth_pass",
                &["transport_io".into()],
            ),
            "not_sent"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "authentication_exchange",
                "receive_password_required->send_auth_pass",
                &["transport_io".into()],
            ),
            "followup_not_sent"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "authentication_exchange",
                "send_auth_pass->receive_auth_ok",
                &["transport_io".into()],
            ),
            "no_response"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "authentication_exchange",
                "send_auth_pass->receive_auth_ok",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "file_transfer_session",
                "send_list->receive_transfer_open",
                &["transport_io".into()],
            ),
            "no_response"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "file_transfer_session",
                "send_port->receive_port_ready",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_mode_label(
                "attention",
                "file_transfer_session",
                "send_port->receive_port_ready",
                &["transport_io".into()],
            ),
            "no_response"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "file_transfer_session",
                "send_list->receive_transfer_open",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "tls_handshake",
                "connect->establish",
                &["route_io".into()],
            ),
            "route_or_connect_blocked"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "tls_handshake",
                "send_client_hello->receive_server_hello",
                &[],
            ),
            "handshake_incomplete"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "http3_request_response",
                "send_request_stream->receive_response_stream",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_authentication",
                "send_auth_request_stream->receive_auth_ok_stream",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_udp_relay",
                "send_udp_relay_datagram->receive_udp_relay_datagram",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
        assert_eq!(
            crate::failure_detail_label(
                "attention",
                "proxy_tcp_relay",
                "send_tcp_request_stream->receive_tcp_response_stream",
                &["transport_io".into()],
            ),
            "request_sent_no_reply"
        );
    }

    #[test]
    fn failure_confidence_and_basis_distinguish_direct_and_inferred_failures() {
        assert_eq!(
            crate::failure_basis_label(
                "attention",
                "proxy_authentication",
                "receive_auth_required",
                &[],
            ),
            "direct_protocol_signal"
        );
        assert_eq!(
            crate::failure_confidence_label(
                "attention",
                "proxy_authentication",
                "receive_auth_required",
                &[],
            ),
            "high"
        );
        assert_eq!(
            crate::failure_basis_label(
                "attention",
                "http3_request_response",
                "send_request_stream->receive_response_stream",
                &["transport_io".into()],
            ),
            "missing_transition"
        );
        assert_eq!(
            crate::failure_confidence_label(
                "attention",
                "http3_request_response",
                "send_request_stream->receive_response_stream",
                &["transport_io".into()],
            ),
            "medium"
        );
        assert_eq!(
            crate::failure_basis_label(
                "attention",
                "tls_handshake",
                "connect->establish",
                &["route_io".into()],
            ),
            "missing_transition"
        );
    }

    #[test]
    fn process_profiles_lower_confidence_for_competing_missing_transition_hypotheses() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let primary_flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &primary_flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing http response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );

        let mut competing_flow = primary_flow.clone();
        competing_flow.id = gewyvern::flow::ProgramFlowId(primary_flow.id.0 + 5000);
        export.program_flows.push(competing_flow.clone());
        push_synthetic_missing_stage_finding(
            &mut export,
            &competing_flow,
            "http_connect_authenticated_tunnel_path",
            "proxy_authentication",
            "receive_auth_ok",
            "receive_payload",
            "send_auth_request->receive_auth_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing proxy auth response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );

        let json = crate::process_network_profiles_json(&export);
        assert!(
            json.contains("\"module_kinds\":[\"http_request_response\",\"proxy_authentication\"]"),
            "json={}",
            json
        );
        assert!(
            json.contains(
                "\"missing_transitions\":[\"send_auth_request->receive_auth_ok\",\"send_request->receive_response\"]"
            ),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_confidence\":\"low\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_basis\":\"missing_transition\""),
            "json={}",
            json
        );
        assert!(json.contains("\"ambiguous\":true"), "json={}", json);
        assert!(json.contains("\"competing_hypotheses\":["), "json={}", json);
        assert!(
            json.contains("\"module:proxy_authentication\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"transition:send_request->receive_response\"")
                || json.contains("\"transition:send_auth_request->receive_auth_ok\""),
            "json={}",
            json
        );
    }

    #[test]
    fn process_profiles_lower_direct_signal_confidence_for_competing_module_hypotheses() {
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
            module_label: "http_connect_auth_required_path".into(),
            network_module_kind: "proxy_authentication".into(),
            phase: Some("receive_auth_required".into()),
            phase_kind: Some("receive_payload".into()),
            phase_transition: None,
            phase_transition_kind: None,
            suspect_area: "authentication".into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: "synthetic competing proxy auth requirement".into(),
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            evidence_trace: vec!["synthetic:direct_protocol_signal".into()],
        });

        let json = crate::process_network_profiles_json(&export);
        assert!(
            json.contains("\"module_kinds\":[\"http_request_response\",\"proxy_authentication\"]"),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_confidence\":\"medium\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
            "json={}",
            json
        );
        assert!(json.contains("\"ambiguous\":true"), "json={}", json);
    }

    #[test]
    fn summary_json_exposes_ambiguous_competing_hypotheses() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let primary_flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &primary_flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing http response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let mut competing_flow = primary_flow.clone();
        competing_flow.id = gewyvern::flow::ProgramFlowId(primary_flow.id.0 + 6000);
        export.program_flows.push(competing_flow.clone());
        push_synthetic_missing_stage_finding(
            &mut export,
            &competing_flow,
            "http_connect_authenticated_tunnel_path",
            "proxy_authentication",
            "receive_auth_ok",
            "receive_payload",
            "send_auth_request->receive_auth_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing proxy auth response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );

        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"ambiguous\":true"), "json={}", json);
        assert!(json.contains("\"competing_hypotheses\":["), "json={}", json);
        assert!(
            json.contains("\"module:proxy_authentication\""),
            "json={}",
            json
        );
    }

    #[test]
    fn mixed_dns_tls_http_profile_stays_ambiguous_and_low_confidence() {
        let process = synthetic_process_view(7001, "curl");
        let dns_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy")
                        .expect("dns_udp_process DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
            ),
            &process,
        );
        let tls_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy")
                        .expect("tls_client_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            ),
            &process,
        );
        let mut http_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                        .expect("http_request_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            ),
            &process,
        );
        let http_flow = http_export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut http_export,
            &http_flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing http response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );

        let export = merge_exports_for_tests(vec![dns_export, tls_export, http_export]);
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"http_request_response\""));
        assert!(json.contains("\"ambiguous\":true"), "json={}", json);
        assert!(
            json.contains("\"primary_failure_confidence\":\"low\""),
            "json={}",
            json
        );
        assert!(json.contains("\"module:name_resolution\""), "json={}", json);
        assert!(json.contains("\"module:tls_handshake\""), "json={}", json);
    }

    #[test]
    fn mixed_proxy_tunnel_and_upstream_request_exposes_competing_hypotheses() {
        let process = synthetic_process_view(7002, "apt");
        let proxy_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file(
                        "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_authenticated_tunnel_path.gewy",
                    )
                    .expect("http_connect_authenticated_tunnel_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            ),
            &process,
        );
        let mut http_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                        .expect("http_request_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
            ),
            &process,
        );
        let http_flow = http_export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut http_export,
            &http_flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing upstream http response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );

        let export = merge_exports_for_tests(vec![proxy_export, http_export]);
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"ambiguous\":true"), "json={}", json);
        assert!(
            json.contains("\"primary_failure_confidence\":\"low\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"module:proxy_authentication\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"transition:send_request->receive_response\""),
            "json={}",
            json
        );
    }

    #[test]
    fn mixed_quic_http3_hy2_profile_stays_conservative() {
        let process = synthetic_process_view(7003, "proxy");
        let quic_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file(
                        "/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy",
                    )
                    .expect("quic_stream_session_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
            ),
            &process,
        );
        let mut http3_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy")
                        .expect("http3_request_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
            ),
            &process,
        );
        let http3_flow = http3_export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut http3_export,
            &http3_flow,
            "http3_request_path",
            "http3_request_response",
            "receive_response_stream",
            "receive_payload",
            "send_request_stream->receive_response_stream",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing http3 response",
            "quic_frame_meta_fragment",
            "missing_signal:quic_frame_observed",
        );
        let hy2_export = coerce_export_process(
            annotate_export_trust(
                run_binding_demo(
                    compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy")
                        .expect("hy2_auth_path DSL should compile"),
                ),
                &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
            ),
            &process,
        );

        let export = merge_exports_for_tests(vec![quic_export, http3_export, hy2_export]);
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"http3_request_response\""));
        assert!(json.contains("\"ambiguous\":true"), "json={}", json);
        assert!(
            json.contains("\"primary_failure_confidence\":\"low\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"module:quic_stream_session\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"module:proxy_authentication\""),
            "json={}",
            json
        );
    }

    #[test]
    fn summary_json_carries_modern_protocol_failure_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy")
            .expect("http3_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http3_request_path",
            "http3_request_response",
            "receive_response_stream",
            "receive_payload",
            "send_request_stream->receive_response_stream",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing http3 response",
            "quic_frame_meta_fragment",
            "missing_signal:quic_frame_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"http3_request_response\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"failure_detail\":\"request_sent_no_reply\""));
    }

    #[test]
    fn summary_json_carries_tls_handshake_incomplete_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy")
            .expect("tls_client_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "tls_client_path",
            "tls_handshake",
            "receive_server_hello",
            "receive_payload",
            "send_client_hello->receive_server_hello",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing tls server hello",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"tls_handshake\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"setup_incomplete\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_detail\":\"handshake_incomplete\""));
    }

    #[test]
    fn summary_json_carries_ssh_banner_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy")
            .expect("ssh_session_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8281, 53022, "ssh-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8281,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8281, 1, 2, 53022, 22),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ssh_session_path",
            "remote_access_session",
            "receive_server_banner",
            "receive_payload",
            "connect->receive_server_banner",
            "initiate_connection->receive_payload",
            "transport_io",
            "synthetic missing ssh server banner",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"remote_access_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"setup_incomplete\""));
        assert!(json.contains("\"primary_failure_detail\":\"handshake_incomplete\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ssh_kex_followup_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy")
            .expect("ssh_session_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8282, 53023, "ssh-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8282,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8282, 1, 2, 53023, 22),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8282,
                        0x18,
                        PacketDir::Ingress,
                        Some(53023),
                        Some(22),
                        Some(0x53),
                        Some(0x5353),
                        Some(0x5353482d),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8282,
                        0x18,
                        PacketDir::Egress,
                        Some(53023),
                        Some(22),
                        Some(0x53),
                        Some(0x5353),
                        Some(0x5353482d),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"remote_access_session\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"not_sent\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_detail\":\"followup_not_sent\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ssh_auth_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssh_auth_path.gewy")
            .expect("ssh_auth_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8289, 53028, "ssh-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8289,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8289, 1, 2, 53028, 22),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8289,
                        0x18,
                        PacketDir::Ingress,
                        Some(53028),
                        Some(22),
                        Some(0x53),
                        Some(0x5353),
                        Some(0x5353482d),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8289,
                        0x18,
                        PacketDir::Egress,
                        Some(53028),
                        Some(22),
                        Some(0x53),
                        Some(0x5353),
                        Some(0x5353482d),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        8289,
                        0x18,
                        PacketDir::Egress,
                        Some(53028),
                        Some(22),
                        &[(0, 0x00), (4, 0x10), (5, 0x14)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        8289,
                        0x18,
                        PacketDir::Egress,
                        Some(53028),
                        Some(22),
                        &[(0, 0x00), (4, 0x10), (5, 0x32)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ssh_auth_path",
            "remote_access_authentication",
            "receive_auth_success",
            "receive_payload",
            "send_auth_request->receive_auth_success",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing ssh auth success",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"remote_access_authentication\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ssh_channel_timeout_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssh_channel_session_path.gewy")
                .expect("ssh_channel_session_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8290, 53029, "ssh-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8290,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8290, 1, 2, 53029, 22),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8290,
                        0x18,
                        PacketDir::Ingress,
                        Some(53029),
                        Some(22),
                        Some(0x53),
                        Some(0x5353),
                        Some(0x5353482d),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8290,
                        0x18,
                        PacketDir::Egress,
                        Some(53029),
                        Some(22),
                        Some(0x53),
                        Some(0x5353),
                        Some(0x5353482d),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        8290,
                        0x18,
                        PacketDir::Egress,
                        Some(53029),
                        Some(22),
                        &[(0, 0x00), (4, 0x10), (5, 0x14)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        8290,
                        0x18,
                        PacketDir::Egress,
                        Some(53029),
                        Some(22),
                        &[(0, 0x00), (4, 0x10), (5, 0x32)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        8,
                        8290,
                        0x18,
                        PacketDir::Ingress,
                        Some(53029),
                        Some(22),
                        &[(0, 0x00), (4, 0x10), (5, 0x34)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        9,
                        8290,
                        0x18,
                        PacketDir::Egress,
                        Some(53029),
                        Some(22),
                        &[(0, 0x00), (4, 0x10), (5, 0x5a)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ssh_channel_session_path",
            "remote_access_session",
            "receive_channel_open_confirmation",
            "receive_payload",
            "send_channel_open->receive_channel_open_confirmation",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing ssh channel open confirmation",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"remote_access_session\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ftp_banner_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ftp_session_path.gewy")
            .expect("ftp_session_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8285, 53182, "ftp-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8285,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8285, 1, 2, 53182, 21),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ftp_session_path",
            "authentication_exchange",
            "receive_banner",
            "receive_payload",
            "connect->receive_banner",
            "initiate_connection->receive_payload",
            "transport_io",
            "synthetic missing ftp banner",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"setup_incomplete\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"handshake_incomplete\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ftp_auth_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ftp_session_path.gewy")
            .expect("ftp_session_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8286, 53183, "ftp-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8286,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8286, 1, 2, 53183, 21),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8286,
                        0x18,
                        PacketDir::Ingress,
                        Some(53183),
                        Some(21),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8286,
                        0x18,
                        PacketDir::Egress,
                        Some(53183),
                        Some(21),
                        Some(0x55),
                        Some(0x5553),
                        Some(0x55534552),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        8286,
                        0x18,
                        PacketDir::Ingress,
                        Some(53183),
                        Some(21),
                        Some(0x33),
                        Some(0x3333),
                        Some(0x33333120),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        8286,
                        0x18,
                        PacketDir::Egress,
                        Some(53183),
                        Some(21),
                        Some(0x50),
                        Some(0x5041),
                        Some(0x50415353),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ftp_session_path",
            "authentication_exchange",
            "receive_auth_ok",
            "receive_payload",
            "send_auth_pass->receive_auth_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing ftp auth ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ftp_auth_followup_missing_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ftp_session_path.gewy")
            .expect("ftp_session_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8287, 53184, "ftp-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8287,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8287, 1, 2, 53184, 21),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8287,
                        0x18,
                        PacketDir::Ingress,
                        Some(53184),
                        Some(21),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8287,
                        0x18,
                        PacketDir::Egress,
                        Some(53184),
                        Some(21),
                        Some(0x55),
                        Some(0x5553),
                        Some(0x55534552),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        8287,
                        0x18,
                        PacketDir::Ingress,
                        Some(53184),
                        Some(21),
                        Some(0x33),
                        Some(0x3333),
                        Some(0x33333120),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ftp_session_path",
            "authentication_exchange",
            "send_auth_pass",
            "emit_payload",
            "receive_password_required->send_auth_pass",
            "receive_payload->emit_payload",
            "transport_io",
            "synthetic missing ftp auth pass",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"not_sent\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"followup_not_sent\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ftp_list_timeout_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ftp_passive_list_path.gewy")
                .expect("ftp_passive_list_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8288, 53185, "ftp-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8288,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8288, 1, 2, 53185, 21),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8288,
                        0x18,
                        PacketDir::Ingress,
                        Some(53185),
                        Some(21),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8288,
                        0x18,
                        PacketDir::Egress,
                        Some(53185),
                        Some(21),
                        Some(0x55),
                        Some(0x5553),
                        Some(0x55534552),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        8288,
                        0x18,
                        PacketDir::Ingress,
                        Some(53185),
                        Some(21),
                        Some(0x33),
                        Some(0x3333),
                        Some(0x33333120),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        8288,
                        0x18,
                        PacketDir::Egress,
                        Some(53185),
                        Some(21),
                        Some(0x50),
                        Some(0x5041),
                        Some(0x50415353),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        8288,
                        0x18,
                        PacketDir::Ingress,
                        Some(53185),
                        Some(21),
                        Some(0x32),
                        Some(0x3233),
                        Some(0x32333020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        9,
                        8288,
                        0x18,
                        PacketDir::Egress,
                        Some(53185),
                        Some(21),
                        Some(0x50),
                        Some(0x5041),
                        Some(0x50415356),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        10,
                        8288,
                        0x18,
                        PacketDir::Ingress,
                        Some(53185),
                        Some(21),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323720),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        11,
                        8288,
                        0x18,
                        PacketDir::Egress,
                        Some(53185),
                        Some(21),
                        Some(0x4c),
                        Some(0x4c49),
                        Some(0x4c495354),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ftp_passive_list_path",
            "file_transfer_session",
            "receive_transfer_open",
            "receive_payload",
            "send_list->receive_transfer_open",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing ftp transfer open",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"file_transfer_session\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ftp_active_port_timeout_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_list_path.gewy")
                .expect("ftp_active_list_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8301, 53042, "ftp-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8301,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8301, 1, 2, 53042, 21),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8301,
                        0x18,
                        PacketDir::Ingress,
                        Some(53042),
                        Some(21),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8301,
                        0x18,
                        PacketDir::Egress,
                        Some(53042),
                        Some(21),
                        Some(0x55),
                        Some(0x5553),
                        Some(0x55534552),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        8301,
                        0x18,
                        PacketDir::Ingress,
                        Some(53042),
                        Some(21),
                        Some(0x33),
                        Some(0x3333),
                        Some(0x33333120),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        8301,
                        0x18,
                        PacketDir::Egress,
                        Some(53042),
                        Some(21),
                        Some(0x50),
                        Some(0x5041),
                        Some(0x50415353),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        8301,
                        0x18,
                        PacketDir::Ingress,
                        Some(53042),
                        Some(21),
                        Some(0x32),
                        Some(0x3233),
                        Some(0x32333020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        9,
                        8301,
                        0x18,
                        PacketDir::Egress,
                        Some(53042),
                        Some(21),
                        Some(0x50),
                        Some(0x504f),
                        Some(0x504f5254),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "ftp_active_list_path",
            "file_transfer_session",
            "receive_port_ready",
            "receive_payload",
            "send_port->receive_port_ready",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing ftp port ready",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"file_transfer_session\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_ftp_denied_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ftp_denied_path.gewy")
            .expect("ftp_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8302, 53053, "ftp-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8302,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8302, 1, 2, 53053, 21),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8302,
                        0x18,
                        PacketDir::Ingress,
                        Some(53053),
                        Some(21),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8302,
                        0x18,
                        PacketDir::Egress,
                        Some(53053),
                        Some(21),
                        Some(0x55),
                        Some(0x5553),
                        Some(0x55534552),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        8302,
                        0x18,
                        PacketDir::Ingress,
                        Some(53053),
                        Some(21),
                        Some(0x33),
                        Some(0x3333),
                        Some(0x33333120),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        8302,
                        0x18,
                        PacketDir::Egress,
                        Some(53053),
                        Some(21),
                        Some(0x50),
                        Some(0x5041),
                        Some(0x50415353),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        8302,
                        0x18,
                        PacketDir::Ingress,
                        Some(53053),
                        Some(21),
                        Some(0x35),
                        Some(0x3533),
                        Some(0x35333020),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_http3_request_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy")
            .expect("http3_request_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http3_request_path",
            "http3_request_response",
            "receive_response_stream",
            "receive_payload",
            "send_request_stream->receive_response_stream",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing http3 response",
            "quic_frame_meta_fragment",
            "missing_signal:quic_frame_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"http3_request_response\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    }

    #[test]
    fn summary_json_carries_smtp_auth_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_auth_path.gewy")
            .expect("smtp_auth_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82911, 53013, "postfix-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82911,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82911, 1, 2, 53013, 25),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82911,
                        0x18,
                        PacketDir::Ingress,
                        Some(53013),
                        Some(25),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82911,
                        0x18,
                        PacketDir::Egress,
                        Some(53013),
                        Some(25),
                        Some(0x45),
                        Some(0x4548),
                        Some(0x45484c4f),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        82911,
                        0x18,
                        PacketDir::Ingress,
                        Some(53013),
                        Some(25),
                        Some(0x32),
                        Some(0x3235),
                        Some(0x32353020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        82911,
                        0x18,
                        PacketDir::Egress,
                        Some(53013),
                        Some(25),
                        Some(0x41),
                        Some(0x4155),
                        Some(0x41555448),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "smtp_auth_path",
            "authentication_exchange",
            "receive_auth_ok",
            "receive_payload",
            "send_auth_request->receive_auth_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing smtp auth ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_imap_auth_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/imap_auth_path.gewy")
            .expect("imap_auth_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82921, 53041, "imap-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82921,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82921, 1, 2, 53041, 143),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82921,
                        0x18,
                        PacketDir::Ingress,
                        Some(53041),
                        Some(143),
                        Some(0x2a),
                        Some(0x2a20),
                        Some(0x2a204f4b),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82921,
                        0x18,
                        PacketDir::Egress,
                        Some(53041),
                        Some(143),
                        &[
                            (0, 0x41),
                            (1, 0x30),
                            (2, 0x30),
                            (3, 0x31),
                            (5, 0x4c),
                            (6, 0x4f),
                            (7, 0x47),
                            (8, 0x49),
                            (9, 0x4e),
                        ],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "imap_auth_path",
            "authentication_exchange",
            "receive_auth_ok",
            "receive_payload",
            "send_auth_request->receive_auth_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing imap auth ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    }

    #[test]
    fn summary_json_carries_smtp_mail_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_mail_path.gewy")
            .expect("smtp_mail_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82912, 53016, "postfix-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82912,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82912, 1, 2, 53016, 25),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82912,
                        0x18,
                        PacketDir::Ingress,
                        Some(53016),
                        Some(25),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82912,
                        0x18,
                        PacketDir::Egress,
                        Some(53016),
                        Some(25),
                        Some(0x45),
                        Some(0x4548),
                        Some(0x45484c4f),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        82912,
                        0x18,
                        PacketDir::Ingress,
                        Some(53016),
                        Some(25),
                        Some(0x32),
                        Some(0x3235),
                        Some(0x32353020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        82912,
                        0x18,
                        PacketDir::Egress,
                        Some(53016),
                        Some(25),
                        Some(0x41),
                        Some(0x4155),
                        Some(0x41555448),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        82912,
                        0x18,
                        PacketDir::Ingress,
                        Some(53016),
                        Some(25),
                        Some(0x32),
                        Some(0x3233),
                        Some(0x32333520),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        9,
                        82912,
                        0x18,
                        PacketDir::Egress,
                        Some(53016),
                        Some(25),
                        Some(0x4d),
                        Some(0x4d41),
                        Some(0x4d41494c),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "smtp_mail_path",
            "mail_session",
            "receive_mail_ok",
            "receive_payload",
            "send_mail_from->receive_mail_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing smtp mail ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_smtp_rcpt_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_rcpt_path.gewy")
            .expect("smtp_rcpt_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82913, 53017, "postfix-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82913,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82913, 1, 2, 53017, 25),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82913,
                        0x18,
                        PacketDir::Ingress,
                        Some(53017),
                        Some(25),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82913,
                        0x18,
                        PacketDir::Egress,
                        Some(53017),
                        Some(25),
                        Some(0x45),
                        Some(0x4548),
                        Some(0x45484c4f),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        82913,
                        0x18,
                        PacketDir::Ingress,
                        Some(53017),
                        Some(25),
                        Some(0x32),
                        Some(0x3235),
                        Some(0x32353020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        82913,
                        0x18,
                        PacketDir::Egress,
                        Some(53017),
                        Some(25),
                        Some(0x41),
                        Some(0x4155),
                        Some(0x41555448),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        82913,
                        0x18,
                        PacketDir::Ingress,
                        Some(53017),
                        Some(25),
                        Some(0x32),
                        Some(0x3233),
                        Some(0x32333520),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        9,
                        82913,
                        0x18,
                        PacketDir::Egress,
                        Some(53017),
                        Some(25),
                        Some(0x4d),
                        Some(0x4d41),
                        Some(0x4d41494c),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        10,
                        82913,
                        0x18,
                        PacketDir::Ingress,
                        Some(53017),
                        Some(25),
                        &[
                            (0, 0x32),
                            (1, 0x35),
                            (2, 0x30),
                            (3, 0x20),
                            (4, 0x32),
                            (5, 0x2e),
                            (6, 0x31),
                            (7, 0x2e),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        11,
                        82913,
                        0x18,
                        PacketDir::Egress,
                        Some(53017),
                        Some(25),
                        Some(0x52),
                        Some(0x5243),
                        Some(0x52435054),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "smtp_rcpt_path",
            "mail_session",
            "receive_rcpt_ok",
            "receive_payload",
            "send_rcpt_to->receive_rcpt_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing smtp rcpt ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_smtp_rcpt_denied_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_rcpt_denied_path.gewy")
                .expect("smtp_rcpt_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82916, 53022, "postfix-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82916,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82916, 1, 2, 53022, 25),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82916,
                        0x18,
                        PacketDir::Ingress,
                        Some(53022),
                        Some(25),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82916,
                        0x18,
                        PacketDir::Egress,
                        Some(53022),
                        Some(25),
                        Some(0x45),
                        Some(0x4548),
                        Some(0x45484c4f),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        82916,
                        0x18,
                        PacketDir::Ingress,
                        Some(53022),
                        Some(25),
                        Some(0x32),
                        Some(0x3235),
                        Some(0x32353020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        82916,
                        0x18,
                        PacketDir::Egress,
                        Some(53022),
                        Some(25),
                        Some(0x41),
                        Some(0x4155),
                        Some(0x41555448),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        82916,
                        0x18,
                        PacketDir::Ingress,
                        Some(53022),
                        Some(25),
                        Some(0x32),
                        Some(0x3233),
                        Some(0x32333520),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        9,
                        82916,
                        0x18,
                        PacketDir::Egress,
                        Some(53022),
                        Some(25),
                        Some(0x4d),
                        Some(0x4d41),
                        Some(0x4d41494c),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        10,
                        82916,
                        0x18,
                        PacketDir::Ingress,
                        Some(53022),
                        Some(25),
                        &[
                            (0, 0x32),
                            (1, 0x35),
                            (2, 0x30),
                            (3, 0x20),
                            (4, 0x32),
                            (5, 0x2e),
                            (6, 0x31),
                            (7, 0x2e),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        11,
                        82916,
                        0x18,
                        PacketDir::Egress,
                        Some(53022),
                        Some(25),
                        Some(0x52),
                        Some(0x5243),
                        Some(0x52435054),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        12,
                        82916,
                        0x18,
                        PacketDir::Ingress,
                        Some(53022),
                        Some(25),
                        Some(0x35),
                        Some(0x3535),
                        Some(0x35353020),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_ldap_bind_denied_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_denied_path.gewy")
                .expect("ldap_bind_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82917, 54030, "ldapbind"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82917,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82917, 1, 2, 54030, 389),
                    tcp_state_fact_with_ports_for_tests(4, 82917, 2, 3, 54030, 389),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82917,
                        0x18,
                        PacketDir::Egress,
                        Some(54030),
                        Some(389),
                        &[(0, 0x30), (4, 0x01), (5, 0x60)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82917,
                        0x18,
                        PacketDir::Ingress,
                        Some(54030),
                        Some(389),
                        &[(0, 0x30), (4, 0x01), (5, 0x61), (9, 0x31)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"directory_bind\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"server_denied\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"access_denied\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_confidence\":\"high\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
            "json={}",
            json
        );
    }

    #[test]
    fn summary_json_carries_ldap_modify_denied_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy")
                .expect("ldap_modify_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82931, 54031, "ldapmodify"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82931,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82931, 1, 2, 54031, 389),
                    tcp_state_fact_with_ports_for_tests(4, 82931, 2, 3, 54031, 389),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82931,
                        0x18,
                        PacketDir::Egress,
                        Some(54031),
                        Some(389),
                        &[(0, 0x30), (4, 0x01), (5, 0x66)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82931,
                        0x18,
                        PacketDir::Ingress,
                        Some(54031),
                        Some(389),
                        &[(0, 0x30), (4, 0x01), (5, 0x67), (9, 0x32)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"directory_write\""));
        assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_ldap_modify_constraint_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy")
                .expect("ldap_modify_constraint_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82932, 54032, "ldapmodify"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82932,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82932, 1, 2, 54032, 389),
                    tcp_state_fact_with_ports_for_tests(4, 82932, 2, 3, 54032, 389),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82932,
                        0x18,
                        PacketDir::Egress,
                        Some(54032),
                        Some(389),
                        &[(0, 0x30), (4, 0x01), (5, 0x66)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82932,
                        0x18,
                        PacketDir::Ingress,
                        Some(54032),
                        Some(389),
                        &[(0, 0x30), (4, 0x01), (5, 0x67), (9, 0x13)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"directory_write\""));
        assert!(json.contains("\"primary_failure_mode\":\"semantic_error\""));
        assert!(json.contains("\"primary_failure_detail\":\"protocol_constraint_violation\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_smtp_data_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_path.gewy")
            .expect("smtp_data_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82914, 53018, "postfix-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82914,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82914, 1, 2, 53018, 25),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82914,
                        0x18,
                        PacketDir::Ingress,
                        Some(53018),
                        Some(25),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82914,
                        0x18,
                        PacketDir::Egress,
                        Some(53018),
                        Some(25),
                        Some(0x45),
                        Some(0x4548),
                        Some(0x45484c4f),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        82914,
                        0x18,
                        PacketDir::Ingress,
                        Some(53018),
                        Some(25),
                        Some(0x32),
                        Some(0x3235),
                        Some(0x32353020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        82914,
                        0x18,
                        PacketDir::Egress,
                        Some(53018),
                        Some(25),
                        Some(0x41),
                        Some(0x4155),
                        Some(0x41555448),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        82914,
                        0x18,
                        PacketDir::Ingress,
                        Some(53018),
                        Some(25),
                        Some(0x32),
                        Some(0x3233),
                        Some(0x32333520),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        9,
                        82914,
                        0x18,
                        PacketDir::Egress,
                        Some(53018),
                        Some(25),
                        Some(0x4d),
                        Some(0x4d41),
                        Some(0x4d41494c),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        10,
                        82914,
                        0x18,
                        PacketDir::Ingress,
                        Some(53018),
                        Some(25),
                        &[
                            (0, 0x32),
                            (1, 0x35),
                            (2, 0x30),
                            (3, 0x20),
                            (4, 0x32),
                            (5, 0x2e),
                            (6, 0x31),
                            (7, 0x2e),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        11,
                        82914,
                        0x18,
                        PacketDir::Egress,
                        Some(53018),
                        Some(25),
                        Some(0x52),
                        Some(0x5243),
                        Some(0x52435054),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        12,
                        82914,
                        0x18,
                        PacketDir::Ingress,
                        Some(53018),
                        Some(25),
                        &[
                            (0, 0x32),
                            (1, 0x35),
                            (2, 0x30),
                            (3, 0x20),
                            (4, 0x32),
                            (5, 0x2e),
                            (6, 0x31),
                            (7, 0x2e),
                            (8, 0x35),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        13,
                        82914,
                        0x18,
                        PacketDir::Egress,
                        Some(53018),
                        Some(25),
                        Some(0x44),
                        Some(0x4441),
                        Some(0x44415441),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        14,
                        82914,
                        0x18,
                        PacketDir::Ingress,
                        Some(53018),
                        Some(25),
                        Some(0x33),
                        Some(0x3335),
                        Some(0x33353420),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        15,
                        82914,
                        0x18,
                        PacketDir::Egress,
                        Some(53018),
                        Some(25),
                        &[(0, 0x0d), (1, 0x0a), (2, 0x2e), (3, 0x0d), (4, 0x0a)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "smtp_data_path",
            "mail_session",
            "receive_message_queued",
            "receive_payload",
            "send_message_body->receive_message_queued",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing smtp queued ack",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_smtp_data_denied_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_denied_path.gewy")
                .expect("smtp_data_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82915, 53020, "postfix-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82915,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82915, 1, 2, 53020, 25),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82915,
                        0x18,
                        PacketDir::Ingress,
                        Some(53020),
                        Some(25),
                        Some(0x32),
                        Some(0x3232),
                        Some(0x32323020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82915,
                        0x18,
                        PacketDir::Egress,
                        Some(53020),
                        Some(25),
                        Some(0x45),
                        Some(0x4548),
                        Some(0x45484c4f),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        82915,
                        0x18,
                        PacketDir::Ingress,
                        Some(53020),
                        Some(25),
                        Some(0x32),
                        Some(0x3235),
                        Some(0x32353020),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        7,
                        82915,
                        0x18,
                        PacketDir::Egress,
                        Some(53020),
                        Some(25),
                        Some(0x41),
                        Some(0x4155),
                        Some(0x41555448),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        8,
                        82915,
                        0x18,
                        PacketDir::Ingress,
                        Some(53020),
                        Some(25),
                        Some(0x32),
                        Some(0x3233),
                        Some(0x32333520),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        9,
                        82915,
                        0x18,
                        PacketDir::Egress,
                        Some(53020),
                        Some(25),
                        Some(0x4d),
                        Some(0x4d41),
                        Some(0x4d41494c),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        10,
                        82915,
                        0x18,
                        PacketDir::Ingress,
                        Some(53020),
                        Some(25),
                        &[
                            (0, 0x32),
                            (1, 0x35),
                            (2, 0x30),
                            (3, 0x20),
                            (4, 0x32),
                            (5, 0x2e),
                            (6, 0x31),
                            (7, 0x2e),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        11,
                        82915,
                        0x18,
                        PacketDir::Egress,
                        Some(53020),
                        Some(25),
                        Some(0x52),
                        Some(0x5243),
                        Some(0x52435054),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        12,
                        82915,
                        0x18,
                        PacketDir::Ingress,
                        Some(53020),
                        Some(25),
                        &[
                            (0, 0x32),
                            (1, 0x35),
                            (2, 0x30),
                            (3, 0x20),
                            (4, 0x32),
                            (5, 0x2e),
                            (6, 0x31),
                            (7, 0x2e),
                            (8, 0x35),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        13,
                        82915,
                        0x18,
                        PacketDir::Egress,
                        Some(53020),
                        Some(25),
                        Some(0x44),
                        Some(0x4441),
                        Some(0x44415441),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        14,
                        82915,
                        0x18,
                        PacketDir::Ingress,
                        Some(53020),
                        Some(25),
                        Some(0x33),
                        Some(0x3335),
                        Some(0x33353420),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        15,
                        82915,
                        0x18,
                        PacketDir::Egress,
                        Some(53020),
                        Some(25),
                        &[(0, 0x0d), (1, 0x0a), (2, 0x2e), (3, 0x0d), (4, 0x0a)],
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        16,
                        82915,
                        0x18,
                        PacketDir::Ingress,
                        Some(53020),
                        Some(25),
                        Some(0x35),
                        Some(0x3535),
                        Some(0x35353020),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(
            json.contains("\"primary_module_kind\":\"mail_session\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_mode\":\"server_denied\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"access_denied\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_confidence\":\"high\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
            "json={}",
            json
        );
    }

    #[test]
    fn summary_json_carries_hy2_auth_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy")
            .expect("hy2_auth_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "hy2_auth_path",
            "proxy_authentication",
            "receive_auth_ok_stream",
            "receive_payload",
            "send_auth_request_stream->receive_auth_ok_stream",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing hy2 auth ok",
            "quic_frame_meta_fragment",
            "missing_signal:quic_frame_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"proxy_authentication\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
    }

    #[test]
    fn summary_json_carries_hy2_tcp_relay_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy")
            .expect("hy2_tcp_relay_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "hy2_tcp_relay_path",
            "proxy_tcp_relay",
            "receive_tcp_response_stream",
            "receive_payload",
            "send_tcp_request_stream->receive_tcp_response_stream",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing hy2 tcp response",
            "quic_frame_meta_fragment",
            "missing_signal:quic_frame_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"proxy_tcp_relay\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_socks5_timeout_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_session_path.gewy")
                .expect("socks5_session_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8283, 53180, "proxy-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8283,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8283, 1, 2, 53180, 1080),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8283,
                        0x18,
                        PacketDir::Egress,
                        Some(53180),
                        Some(1080),
                        Some(0x05),
                        Some(0x0501),
                        None,
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        8283,
                        0x18,
                        PacketDir::Ingress,
                        Some(53180),
                        Some(1080),
                        Some(0x05),
                        Some(0x0500),
                        None,
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        8283,
                        0x18,
                        PacketDir::Egress,
                        Some(53180),
                        Some(1080),
                        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "socks5_session_path",
            "proxy_negotiation",
            "receive_connect_success",
            "receive_payload",
            "send_connect_request->receive_connect_success",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing socks5 connect success",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"proxy_negotiation\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
    }

    #[test]
    fn summary_json_carries_socks5_auth_connect_denied_detail() {
        let binding = compile_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_connect_denied_path.gewy",
        )
        .expect("socks5_auth_connect_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82831, 53186, "proxy-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82831,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82831, 1, 2, 53186, 1080),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        4,
                        82831,
                        0x18,
                        PacketDir::Egress,
                        Some(53186),
                        Some(1080),
                        &[(0, 0x05), (1, 0x01), (2, 0x02)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82831,
                        0x18,
                        PacketDir::Ingress,
                        Some(53186),
                        Some(1080),
                        &[(0, 0x05), (1, 0x02)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82831,
                        0x18,
                        PacketDir::Egress,
                        Some(53186),
                        Some(1080),
                        &[(0, 0x01), (1, 0x01)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        82831,
                        0x18,
                        PacketDir::Ingress,
                        Some(53186),
                        Some(1080),
                        &[(0, 0x01), (1, 0x00)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        8,
                        82831,
                        0x18,
                        PacketDir::Egress,
                        Some(53186),
                        Some(1080),
                        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        9,
                        82831,
                        0x18,
                        PacketDir::Ingress,
                        Some(53186),
                        Some(1080),
                        &[(0, 0x05), (1, 0x05), (2, 0x00), (3, 0x01)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"proxy_negotiation\""));
        assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_imap_auth_denied_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/imap_auth_denied_path.gewy")
                .expect("imap_auth_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82922, 53042, "imap-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82922,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82922, 1, 2, 53042, 143),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82922,
                        0x18,
                        PacketDir::Ingress,
                        Some(53042),
                        Some(143),
                        Some(0x2a),
                        Some(0x2a20),
                        Some(0x2a204f4b),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82922,
                        0x18,
                        PacketDir::Egress,
                        Some(53042),
                        Some(143),
                        &[
                            (0, 0x41),
                            (1, 0x30),
                            (2, 0x30),
                            (3, 0x31),
                            (5, 0x4c),
                            (6, 0x4f),
                            (7, 0x47),
                            (8, 0x49),
                            (9, 0x4e),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82922,
                        0x18,
                        PacketDir::Ingress,
                        Some(53042),
                        Some(143),
                        &[
                            (0, 0x41),
                            (1, 0x30),
                            (2, 0x30),
                            (3, 0x31),
                            (5, 0x4e),
                            (6, 0x4f),
                        ],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_imap_select_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/imap_select_path.gewy")
            .expect("imap_select_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82928, 53050, "imap-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82928,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82928, 1, 2, 53050, 143),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82928,
                        0x18,
                        PacketDir::Ingress,
                        Some(53050),
                        Some(143),
                        Some(0x2a),
                        Some(0x2a20),
                        Some(0x2a204f4b),
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82928,
                        0x18,
                        PacketDir::Egress,
                        Some(53050),
                        Some(143),
                        &[
                            (0, 0x41),
                            (1, 0x30),
                            (2, 0x30),
                            (3, 0x31),
                            (5, 0x4c),
                            (6, 0x4f),
                            (7, 0x47),
                            (8, 0x49),
                            (9, 0x4e),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82928,
                        0x18,
                        PacketDir::Ingress,
                        Some(53050),
                        Some(143),
                        &[
                            (0, 0x41),
                            (1, 0x30),
                            (2, 0x30),
                            (3, 0x31),
                            (5, 0x4f),
                            (6, 0x4b),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        82928,
                        0x18,
                        PacketDir::Egress,
                        Some(53050),
                        Some(143),
                        &[
                            (0, 0x41),
                            (1, 0x30),
                            (2, 0x30),
                            (3, 0x32),
                            (5, 0x53),
                            (6, 0x45),
                            (7, 0x4c),
                            (8, 0x45),
                            (9, 0x43),
                            (10, 0x54),
                        ],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "imap_select_path",
            "mail_session",
            "receive_mailbox_selected",
            "receive_payload",
            "send_select->receive_mailbox_selected",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing imap mailbox selected",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_pop3_auth_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/pop3_auth_path.gewy")
            .expect("pop3_auth_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82923, 53043, "pop3-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82923,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82923, 1, 2, 53043, 110),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        4,
                        82923,
                        0x18,
                        PacketDir::Ingress,
                        Some(53043),
                        Some(110),
                        &[
                            (0, 0x2b),
                            (1, 0x4f),
                            (2, 0x4b),
                            (3, 0x20),
                            (5, 0x50),
                            (6, 0x4f),
                            (7, 0x50),
                            (8, 0x33),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82923,
                        0x18,
                        PacketDir::Egress,
                        Some(53043),
                        Some(110),
                        &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82923,
                        0x18,
                        PacketDir::Ingress,
                        Some(53043),
                        Some(110),
                        &[
                            (0, 0x2b),
                            (1, 0x4f),
                            (2, 0x4b),
                            (3, 0x20),
                            (5, 0x55),
                            (6, 0x73),
                            (7, 0x65),
                            (8, 0x72),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        82923,
                        0x18,
                        PacketDir::Egress,
                        Some(53043),
                        Some(110),
                        &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "pop3_auth_path",
            "authentication_exchange",
            "receive_auth_ok",
            "receive_payload",
            "send_auth_pass->receive_auth_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing pop3 auth ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_pop3_auth_denied_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/pop3_auth_denied_path.gewy")
                .expect("pop3_auth_denied_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82924, 53044, "pop3-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82924,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82924, 1, 2, 53044, 110),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        4,
                        82924,
                        0x18,
                        PacketDir::Ingress,
                        Some(53044),
                        Some(110),
                        &[
                            (0, 0x2b),
                            (1, 0x4f),
                            (2, 0x4b),
                            (3, 0x20),
                            (5, 0x50),
                            (6, 0x4f),
                            (7, 0x50),
                            (8, 0x33),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82924,
                        0x18,
                        PacketDir::Egress,
                        Some(53044),
                        Some(110),
                        &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82924,
                        0x18,
                        PacketDir::Ingress,
                        Some(53044),
                        Some(110),
                        &[
                            (0, 0x2b),
                            (1, 0x4f),
                            (2, 0x4b),
                            (3, 0x20),
                            (5, 0x55),
                            (6, 0x73),
                            (7, 0x65),
                            (8, 0x72),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        82924,
                        0x18,
                        PacketDir::Egress,
                        Some(53044),
                        Some(110),
                        &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        8,
                        82924,
                        0x18,
                        PacketDir::Ingress,
                        Some(53044),
                        Some(110),
                        &[
                            (0, 0x2d),
                            (1, 0x45),
                            (2, 0x52),
                            (3, 0x52),
                            (5, 0x61),
                            (6, 0x75),
                            (7, 0x74),
                            (8, 0x68),
                        ],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_pop3_list_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/pop3_list_path.gewy")
            .expect("pop3_list_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82929, 53051, "pop3-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82929,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82929, 1, 2, 53051, 110),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        4,
                        82929,
                        0x18,
                        PacketDir::Ingress,
                        Some(53051),
                        Some(110),
                        &[
                            (0, 0x2b),
                            (1, 0x4f),
                            (2, 0x4b),
                            (3, 0x20),
                            (5, 0x50),
                            (6, 0x4f),
                            (7, 0x50),
                            (8, 0x33),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82929,
                        0x18,
                        PacketDir::Egress,
                        Some(53051),
                        Some(110),
                        &[(0, 0x55), (1, 0x53), (2, 0x45), (3, 0x52)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82929,
                        0x18,
                        PacketDir::Ingress,
                        Some(53051),
                        Some(110),
                        &[
                            (0, 0x2b),
                            (1, 0x4f),
                            (2, 0x4b),
                            (3, 0x20),
                            (5, 0x55),
                            (6, 0x73),
                            (7, 0x65),
                            (8, 0x72),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        82929,
                        0x18,
                        PacketDir::Egress,
                        Some(53051),
                        Some(110),
                        &[(0, 0x50), (1, 0x41), (2, 0x53), (3, 0x53)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        8,
                        82929,
                        0x18,
                        PacketDir::Ingress,
                        Some(53051),
                        Some(110),
                        &[
                            (0, 0x2b),
                            (1, 0x4f),
                            (2, 0x4b),
                            (3, 0x20),
                            (5, 0x4d),
                            (6, 0x61),
                            (7, 0x69),
                            (8, 0x6c),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        9,
                        82929,
                        0x18,
                        PacketDir::Egress,
                        Some(53051),
                        Some(110),
                        &[(0, 0x4c), (1, 0x49), (2, 0x53), (3, 0x54)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "pop3_list_path",
            "mail_session",
            "receive_list_ready",
            "receive_payload",
            "send_list->receive_list_ready",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing pop3 list ready",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"mail_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_kerberos_as_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/kerberos_as_path.gewy")
            .expect("kerberos_as_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82925, 53045, "kinit"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82925,
                        7,
                        SessionId(1),
                    ),
                    udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
                        3,
                        82925,
                        120,
                        PacketDir::Egress,
                        Some(53045),
                        Some(88),
                        Some(0x6a),
                        None,
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "kerberos_as_path",
            "authentication_exchange",
            "receive_as_reply",
            "receive_datagram",
            "send_as_request->receive_as_reply",
            "emit_datagram->receive_datagram",
            "transport_io",
            "synthetic missing kerberos as reply",
            "udp_packet_meta_fragment",
            "missing_signal:datagram_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    }

    #[test]
    fn summary_json_carries_kerberos_as_error_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/kerberos_as_error_path.gewy")
                .expect("kerberos_as_error_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82926, 53046, "kinit"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82926,
                        7,
                        SessionId(1),
                    ),
                    udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
                        3,
                        82926,
                        120,
                        PacketDir::Egress,
                        Some(53046),
                        Some(88),
                        Some(0x6a),
                        None,
                    ),
                    udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
                        4,
                        82926,
                        100,
                        PacketDir::Ingress,
                        Some(53046),
                        Some(88),
                        Some(0x7e),
                        None,
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"authentication_exchange\""));
        assert!(json.contains("\"primary_failure_mode\":\"semantic_error\""));
        assert!(json.contains("\"primary_failure_detail\":\"protocol_error\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_rtsp_setup_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/rtsp_setup_path.gewy")
            .expect("rtsp_setup_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82927, 53049, "vlc"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82927,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82927, 1, 2, 53049, 554),
                    tcp_state_fact_with_ports_for_tests(4, 82927, 2, 3, 53049, 554),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82927,
                        0x18,
                        PacketDir::Egress,
                        Some(53049),
                        Some(554),
                        &[(0, 0x4f), (1, 0x50), (2, 0x54), (3, 0x49)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82927,
                        0x18,
                        PacketDir::Ingress,
                        Some(53049),
                        Some(554),
                        &[
                            (0, 0x52),
                            (1, 0x54),
                            (2, 0x53),
                            (3, 0x50),
                            (9, 0x32),
                            (10, 0x30),
                            (11, 0x30),
                            (17, 0x50),
                            (18, 0x75),
                            (19, 0x62),
                            (20, 0x6c),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        82927,
                        0x18,
                        PacketDir::Egress,
                        Some(53049),
                        Some(554),
                        &[(0, 0x44), (1, 0x45), (2, 0x53), (3, 0x43)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        8,
                        82927,
                        0x18,
                        PacketDir::Ingress,
                        Some(53049),
                        Some(554),
                        &[
                            (0, 0x52),
                            (1, 0x54),
                            (2, 0x53),
                            (3, 0x50),
                            (9, 0x32),
                            (10, 0x30),
                            (11, 0x30),
                            (17, 0x43),
                            (18, 0x6f),
                            (19, 0x6e),
                            (20, 0x74),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        9,
                        82927,
                        0x18,
                        PacketDir::Egress,
                        Some(53049),
                        Some(554),
                        &[(0, 0x53), (1, 0x45), (2, 0x54), (3, 0x55)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "rtsp_setup_path",
            "signaling_session",
            "receive_setup_ok",
            "receive_payload",
            "send_setup->receive_setup_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing rtsp setup ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"signaling_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_rtsp_describe_timeout_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/rtsp_describe_path.gewy")
            .expect("rtsp_describe_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82930, 53052, "vlc"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82930,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82930, 1, 2, 53052, 554),
                    tcp_state_fact_with_ports_for_tests(4, 82930, 2, 3, 53052, 554),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        5,
                        82930,
                        0x18,
                        PacketDir::Egress,
                        Some(53052),
                        Some(554),
                        &[(0, 0x4f), (1, 0x50), (2, 0x54), (3, 0x49)],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        6,
                        82930,
                        0x18,
                        PacketDir::Ingress,
                        Some(53052),
                        Some(554),
                        &[
                            (0, 0x52),
                            (1, 0x54),
                            (2, 0x53),
                            (3, 0x50),
                            (9, 0x32),
                            (10, 0x30),
                            (11, 0x30),
                            (17, 0x50),
                            (18, 0x75),
                            (19, 0x62),
                            (20, 0x6c),
                        ],
                    ),
                    packet_fact_with_dir_and_payload_bytes_for_tests(
                        7,
                        82930,
                        0x18,
                        PacketDir::Egress,
                        Some(53052),
                        Some(554),
                        &[(0, 0x44), (1, 0x45), (2, 0x53), (3, 0x43)],
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "rtsp_describe_path",
            "signaling_session",
            "receive_describe_ok",
            "receive_payload",
            "send_describe->receive_describe_ok",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing rtsp describe ok",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"signaling_session\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_http_connect_timeout_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_tunnel_path.gewy")
                .expect("http_connect_tunnel_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 8284, 53181, "proxy-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        8284,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 8284, 1, 2, 53181, 8080),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        8284,
                        0x18,
                        PacketDir::Egress,
                        Some(53181),
                        Some(8080),
                        Some(0x43),
                        Some(0x434f),
                        Some(0x434f4e4e),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http_connect_tunnel_path",
            "proxy_tunnel_establishment",
            "receive_connect_established",
            "receive_payload",
            "send_connect_request->receive_connect_established",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing http connect established",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"proxy_tunnel_establishment\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"no_response\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""),
            "json={}",
            json
        );
        assert!(json.contains("\"primary_failure_confidence\":\"medium\""));
        assert!(json.contains("\"primary_failure_basis\":\"missing_transition\""));
    }

    #[test]
    fn summary_json_carries_http_connect_auth_required_detail() {
        let binding = compile_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_auth_required_path.gewy",
        )
        .expect("http_connect_auth_required_path DSL should compile");
        let export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82840, 53185, "proxy-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82840,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82840, 1, 2, 53185, 8080),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82840,
                        0x18,
                        PacketDir::Egress,
                        Some(53185),
                        Some(8080),
                        Some(0x43),
                        Some(0x434f),
                        Some(0x434f4e4e),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82840,
                        0x18,
                        PacketDir::Ingress,
                        Some(53185),
                        Some(8080),
                        Some(0x34),
                        Some(0x3430),
                        Some(0x34303720),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"proxy_authentication\""));
        assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
        assert!(json.contains("\"primary_failure_detail\":\"auth_required\""));
        assert!(json.contains("\"primary_failure_confidence\":\"high\""));
        assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
    }

    #[test]
    fn summary_json_carries_http_connect_authenticated_tunnel_pending_auth_detail() {
        let binding = compile_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_authenticated_tunnel_path.gewy",
        )
        .expect("http_connect_authenticated_tunnel_path DSL should compile");
        let mut export = annotate_export_trust(
            export_from_test_facts(
                binding,
                vec![
                    sock_lineage_fact_for_tests(1, 82841, 53187, "proxy-client"),
                    route_fact(
                        2,
                        SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                        82841,
                        7,
                        SessionId(1),
                    ),
                    tcp_state_fact_with_ports_for_tests(3, 82841, 1, 2, 53187, 8080),
                    packet_fact_with_dir_and_payload_for_tests(
                        4,
                        82841,
                        0x18,
                        PacketDir::Egress,
                        Some(53187),
                        Some(8080),
                        Some(0x43),
                        Some(0x434f),
                        Some(0x434f4e4e),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        5,
                        82841,
                        0x18,
                        PacketDir::Ingress,
                        Some(53187),
                        Some(8080),
                        Some(0x34),
                        Some(0x3430),
                        Some(0x34303720),
                    ),
                    packet_fact_with_dir_and_payload_for_tests(
                        6,
                        82841,
                        0x18,
                        PacketDir::Egress,
                        Some(53187),
                        Some(8080),
                        Some(0x43),
                        Some(0x434f),
                        Some(0x434f4e4e),
                    ),
                ],
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http_connect_authenticated_tunnel_path",
            "proxy_authentication",
            "receive_connect_established",
            "receive_payload",
            "send_connect_request->receive_connect_established",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing authenticated http connect established",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"proxy_authentication\""));
        assert!(
            json.contains("\"primary_failure_mode\":\"server_denied\""),
            "json={}",
            json
        );
        assert!(
            json.contains("\"primary_failure_detail\":\"auth_required\""),
            "json={}",
            json
        );
    }

    #[test]
    fn summary_json_carries_http3_server_timeout_detail() {
        let binding =
            compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy")
                .expect("http3_server_response_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http3_server_response_path",
            "http3_request_response",
            "send_response_stream",
            "emit_payload",
            "receive_request_stream->send_response_stream",
            "receive_payload->emit_payload",
            "transport_io",
            "synthetic missing http3 server response",
            "quic_frame_meta_fragment",
            "missing_signal:quic_frame_observed",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"http3_request_response\""));
        assert!(json.contains("\"primary_failure_mode\":\"not_sent\""));
        assert!(json.contains("\"primary_failure_detail\":\"followup_not_sent\""));
    }

    #[test]
    fn summary_json_carries_tls_route_blocked_detail() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy")
            .expect("tls_client_path DSL should compile");
        let mut export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let flow = export.program_flows[0].clone();
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "tls_client_path",
            "tls_handshake",
            "establish",
            "establish_connection",
            "connect->establish",
            "initiate_connection->establish_connection",
            "route_io",
            "synthetic blocked tls route/connect",
            "route_meta_fragment",
            "missing_signal:route_resolution",
        );
        let json = summary_json("dsl_demo", &export);
        assert!(json.contains("\"primary_module_kind\":\"tls_handshake\""));
        assert!(json.contains("\"primary_failure_mode\":\"setup_incomplete\""));
        assert!(json.contains("\"primary_failure_detail\":\"route_or_connect_blocked\""));
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
        push_synthetic_missing_stage_finding(
            &mut export,
            &flow,
            "http_request_path",
            "http_request_response",
            "receive_response",
            "receive_payload",
            "send_request->receive_response",
            "emit_payload->receive_payload",
            "transport_io",
            "synthetic missing response",
            "tcp_packet_meta_fragment",
            "missing_signal:packet_observed",
        );
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
        assert!(json.contains("\"pid_attribution_status\":\"synthetic\""));
        assert!(json.contains(
            "\"pid_attribution_note\":\"pid-scoped conclusions come from synthetic demo lineage\""
        ));
        assert!(json.contains("\"ingest_mode\":\"demo\""));
        assert!(json.contains("\"ingest_mode_note\":\"synthetic demo mode: useful for exercising flows and reports, not for real process attribution\""));
        assert!(json.contains("\"ingest_trust_mode\":\"synthetic-demo\""));
        assert!(json.contains("\"network_module_kind\":\"http_request_response\""));
        assert!(json.contains("\"network_module_kinds\":[\"http_request_response\"]"));
        assert!(json.contains("\"process_network_profiles\":["));
        assert!(json.contains("\"primary_module_kind\":\"http_request_response\""));
        assert!(json.contains("\"primary_failure_stage\":\"send_request->receive_response\""));
        assert!(json.contains("\"primary_failure_mode\":\"no_response\""));
        assert!(json.contains("\"primary_failure_detail\":\"request_sent_no_reply\""));
    }

    #[test]
    fn api_snapshot_meta_and_routes_cover_single_export() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let state = Arc::new(Mutex::new(ApiSnapshot::default()));
        update_api_snapshot_for_single(
            &state,
            ApiRenderedTarget {
                name: "dsl_demo".into(),
                summary_text: summary_line("dsl_demo", &export),
                summary_json: summary_json("dsl_demo", &export),
                findings_json: findings_json("dsl_demo", &export),
                analysis_json: analysis_snapshot_json(&analysis_snapshot(&export)),
                export_json: export.to_json(),
                report_json: scan_report_json(&[("dsl_demo".to_string(), export.clone())]),
                report_html: scan_report_html(&[("dsl_demo".to_string(), export.clone())]),
            },
        );
        let snapshot = state.lock().unwrap().clone();

        let meta = api_snapshot_meta_json(&snapshot);
        assert!(meta.contains("\"kind\":\"single\""));
        assert!(meta.contains("\"name\":\"dsl_demo\""));
        assert!(meta.contains("\"target_names\":[\"dsl_demo\"]"));
        assert!(meta.contains("\"has_analysis_json\":true"));
        assert!(meta.contains("\"has_export_json\":true"));

        let (_, _, targets_body) = api_response_for_request("/v1/latest/targets", &snapshot);
        assert!(targets_body.contains("\"targets\":[\"dsl_demo\"]"));

        let (_, _, summary_body) = api_response_for_request("/v1/latest/summary.json", &snapshot);
        assert!(summary_body.contains("\"demo\":\"dsl_demo\""));
        let (_, _, analysis_body) = api_response_for_request("/v1/latest/analysis.json", &snapshot);
        assert!(analysis_body.contains("\"primary_module_kind\""));
        assert!(analysis_body.contains("\"protocol_flows\""));
        assert!(analysis_body.contains("\"augmentations\":["));
        assert!(analysis_body.contains("\"name\":\"automation_recommendation\""));

        let (_, _, export_body) = api_response_for_request("/v1/latest/export.json", &snapshot);
        assert!(export_body.contains("\"template_id\""));

        let (_, _, target_summary_body) =
            api_response_for_request("/v1/latest/targets/dsl_demo/summary.json", &snapshot);
        assert!(target_summary_body.contains("\"demo\":\"dsl_demo\""));
        let (_, _, target_analysis_body) =
            api_response_for_request("/v1/latest/targets/dsl_demo/analysis.json", &snapshot);
        assert!(target_analysis_body.contains("\"primary_failure_mode\""));
    }

    #[test]
    fn api_snapshot_routes_cover_scan_export() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let outputs = vec![("scan:http:request".to_string(), export)];
        let state = Arc::new(Mutex::new(ApiSnapshot::default()));
        let rendered_targets = outputs
            .iter()
            .map(|(name, export)| ApiRenderedTarget {
                name: name.clone(),
                summary_text: summary_line(name, export),
                summary_json: summary_json(name, export),
                findings_json: findings_json(name, export),
                analysis_json: analysis_snapshot_json(&analysis_snapshot(export)),
                export_json: export.to_json(),
                report_json: scan_report_json(&[(name.clone(), export.clone())]),
                report_html: scan_report_html(&[(name.clone(), export.clone())]),
            })
            .collect::<Vec<_>>();
        update_api_snapshot_for_scan(
            &state,
            rendered_targets,
            scan_report_text(&outputs),
            scan_report_json(&outputs),
            format!(
                "[{}]",
                outputs
                    .iter()
                    .map(|(name, export)| format!(
                        "{{\"target\":\"{}\",\"analysis\":{}}}",
                        name.replace('\\', "\\\\").replace('"', "\\\""),
                        analysis_snapshot_json(&analysis_snapshot(export)),
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            scan_report_json(&outputs),
            scan_report_html(&outputs),
        );
        let snapshot = state.lock().unwrap().clone();

        let (health_status, _, health_body) = api_response_for_request("/health", &snapshot);
        assert_eq!(health_status, 200);
        assert!(health_body.contains("\"has_snapshot\":true"));

        let (cap_status, _, cap_body) = api_response_for_request("/v1/capabilities", &snapshot);
        assert_eq!(cap_status, 200);
        assert!(cap_body.contains("\"service\":\"gewyvern-api\""));

        let (targets_status, _, targets_body) =
            api_response_for_request("/v1/latest/targets", &snapshot);
        assert_eq!(targets_status, 200);
        assert!(targets_body.contains("\"targets\":[\"scan:http:request\"]"));
        let (analysis_status, _, analysis_body) =
            api_response_for_request("/v1/latest/analysis.json", &snapshot);
        assert_eq!(analysis_status, 200);
        assert!(analysis_body.contains("\"target\":\"scan:http:request\""));
        assert!(analysis_body.contains("\"augmentations\":["));
        assert!(analysis_body.contains("\"name\":\"automation_recommendation\""));

        let (report_status, _, report_body) =
            api_response_for_request("/v1/latest/report.json", &snapshot);
        assert_eq!(report_status, 200);
        assert!(report_body.contains("\"scan_all\":true"));

        let (target_status, _, target_body) = api_response_for_request(
            "/v1/latest/targets/scan:http:request/report.json",
            &snapshot,
        );
        assert_eq!(target_status, 200);
        assert!(target_body.contains("\"target\":\"scan:http:request\""));
        let (target_analysis_status, _, target_analysis_body) = api_response_for_request(
            "/v1/latest/targets/scan:http:request/analysis.json",
            &snapshot,
        );
        assert_eq!(target_analysis_status, 200);
        assert!(target_analysis_body.contains("\"primary_module_kind\""));

        let (findings_status, _, _) =
            api_response_for_request("/v1/latest/findings.json", &snapshot);
        assert_eq!(findings_status, 404);
    }

    #[test]
    fn api_target_list_exposes_url_safe_path_segments() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .expect("http_request_path DSL should compile");
        let export = annotate_export_trust(
            run_binding_demo(binding),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        );
        let state = Arc::new(Mutex::new(ApiSnapshot::default()));
        update_api_snapshot_for_single(
            &state,
            ApiRenderedTarget {
                name: "scan:http request/%".into(),
                summary_text: summary_line("scan:http request/%", &export),
                summary_json: summary_json("scan:http request/%", &export),
                findings_json: findings_json("scan:http request/%", &export),
                analysis_json: analysis_snapshot_json(&analysis_snapshot(&export)),
                export_json: export.to_json(),
                report_json: scan_report_json(&[(
                    "scan:http request/%".to_string(),
                    export.clone(),
                )]),
                report_html: scan_report_html(&[(
                    "scan:http request/%".to_string(),
                    export.clone(),
                )]),
            },
        );
        let snapshot = state.lock().unwrap().clone();

        let (_, _, meta_body) = api_response_for_request("/v1/latest/meta", &snapshot);
        assert!(meta_body.contains("\"path_segment\":\"scan:http%20request%2F%25\""));

        let (_, _, targets_body) = api_response_for_request("/v1/latest/targets", &snapshot);
        assert!(targets_body.contains("\"path_segment_encoding\":\"percent-encoding\""));
        assert!(
            targets_body.contains("\"url_path\":\"/v1/latest/targets/scan:http%20request%2F%25\"")
        );

        let (target_status, _, target_body) = api_response_for_request(
            "/v1/latest/targets/scan:http%20request%2F%25/summary.json",
            &snapshot,
        );
        assert_eq!(target_status, 200);
        assert!(target_body.contains("\"demo\":\"scan:http request/%\""));
        let (analysis_status, _, analysis_body) = api_response_for_request(
            "/v1/latest/targets/scan:http%20request%2F%25/analysis.json",
            &snapshot,
        );
        assert_eq!(analysis_status, 200);
        assert!(analysis_body.contains("\"primary_module_kind\""));
    }

    #[test]
    fn api_rejects_invalid_target_path_percent_encoding() {
        let snapshot = ApiSnapshot::default();
        let (status, _, body) =
            api_response_for_request("/v1/latest/targets/bad%2/report.json", &snapshot);
        assert_eq!(status, 400);
        assert!(body.contains("\"error\":\"invalid_target_path_segment\""));
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
