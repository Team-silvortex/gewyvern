use gewyvern::dsl::build_lockfile;
#[cfg(test)]
use gewyvern::gewyc::render_explain_report;
use gewyvern::gewyc::{
    CompilerEnvelope, ExplainFocus, FrontendFocus, RenderFormat, compile_envelope_file,
    compile_explain_report_file, compile_frontend_report_file, render_binding_report,
    render_diagnostics_report, render_envelope_report, render_explain_report_with_options,
    render_findings_report, render_frontend_report_with_options, render_stages_report,
};
use std::env;
use std::fs;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputMode {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Compile,
    Explain,
    Frontend,
    Diagnostics,
    Findings,
    Stages,
    Envelope,
    Init,
    Lock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EmitTarget {
    Binding,
    Explain,
    Frontend,
    Diagnostics,
    Findings,
    Stages,
    Envelope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    command: Command,
    emit: EmitTarget,
    path: String,
    output: OutputMode,
    focus: Option<String>,
    compact: bool,
    out: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiLocale {
    En,
    Zh,
}

impl UiLocale {
    fn detect() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = env::var(key) {
                if value.to_ascii_lowercase().starts_with("zh") {
                    return Self::Zh;
                }
            }
        }
        Self::En
    }

    fn usage(self) -> &'static str {
        match self {
            Self::Zh => {
                "用法: gewyc <compile|explain|frontend|diagnostics|findings|stages|envelope> <path.gewy> [--json] [--out path]\n\
                 用法: gewyc lock [dir|gewy.pkg] [--out path]\n\
                 用法: gewyc init [dir]\n\
                 用法: gewyc <path.gewy> [--json] [--emit binding|explain|frontend|diagnostics|findings|stages|envelope] [--out path]"
            }
            Self::En => {
                "usage: gewyc <compile|explain|frontend|diagnostics|findings|stages|envelope> <path.gewy> [--json] [--out path]\n\
                 usage: gewyc lock [dir|gewy.pkg] [--out path]\n\
                 usage: gewyc init [dir]\n\
                 usage: gewyc <path.gewy> [--json] [--emit binding|explain|frontend|diagnostics|findings|stages|envelope] [--out path]"
            }
        }
    }

    fn msg(self, key: &str) -> &'static str {
        match (self, key) {
            (Self::Zh, "missing_path") => "缺少 .gewy 文件路径",
            (Self::Zh, "unknown_arg") => "未知参数",
            (Self::Zh, "compile_failed") => "DSL 编译失败",
            (Self::Zh, "explain_failed") => "解释摘要生成失败",
            (Self::Zh, "frontend_failed") => "前端摘要生成失败",
            (Self::Zh, "diagnostics_failed") => "binding 诊断失败",
            (Self::Zh, "stages_failed") => "compiler 阶段报告生成失败",
            (Self::Zh, "missing_emit") => {
                "缺少 --emit 的值，期望 binding、explain、frontend、diagnostics、findings、stages 或 envelope"
            }
            (Self::Zh, "missing_out") => "缺少 --out 的值，期望输出路径",
            (Self::Zh, "missing_focus") => "缺少 --focus 的值",
            (Self::Zh, "invalid_focus") => "--focus 只适用于 explain、frontend 或对应的 --emit",
            (Self::Zh, "invalid_compact") => "--compact 只适用于 explain、frontend 或对应的 --emit",
            (Self::Zh, "write_failed") => "写入输出失败",
            (Self::Zh, "init_failed") => "初始化 gewy package 失败",
            (Self::Zh, "lock_failed") => "生成 gewy.lock 失败",
            (_, "missing_path") => "missing .gewy file path",
            (_, "unknown_arg") => "unknown argument",
            (_, "compile_failed") => "dsl compile failed",
            (_, "explain_failed") => "explain summary failed",
            (_, "frontend_failed") => "frontend summary failed",
            (_, "diagnostics_failed") => "binding diagnostics failed",
            (_, "stages_failed") => "compiler stages report failed",
            (_, "missing_emit") => {
                "missing value for --emit, expected binding, explain, frontend, diagnostics, findings, stages, or envelope"
            }
            (_, "missing_out") => "missing value for --out, expected an output path",
            (_, "missing_focus") => "missing value for --focus",
            (_, "invalid_focus") => {
                "--focus is only valid with explain/frontend or their --emit forms"
            }
            (_, "invalid_compact") => {
                "--compact is only valid with explain/frontend or their --emit forms"
            }
            (_, "write_failed") => "failed to write output",
            (_, "init_failed") => "failed to initialize gewy package",
            (_, "lock_failed") => "failed to build gewy.lock",
            _ => "error",
        }
    }
}

fn main() {
    let locale = UiLocale::detect();
    let cli = parse_cli(env::args().collect(), locale).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });

    match cli.emit {
        _ if cli.command == Command::Init => run_init(cli, locale),
        _ if cli.command == Command::Lock => run_lock(cli, locale),
        EmitTarget::Binding => run_compile(cli, locale),
        EmitTarget::Explain => run_explain(cli, locale),
        EmitTarget::Frontend => run_frontend(cli, locale),
        EmitTarget::Diagnostics => run_diagnostics(cli, locale),
        EmitTarget::Findings => run_findings(cli, locale),
        EmitTarget::Stages => run_stages(cli, locale),
        EmitTarget::Envelope => run_envelope(cli, locale),
    }
}

fn parse_cli(args: Vec<String>, locale: UiLocale) -> Result<Cli, String> {
    let mut command = Command::Compile;
    let mut emit = EmitTarget::Binding;
    let mut path = None;
    let mut output = OutputMode::Text;
    let mut focus = None;
    let mut compact = false;
    let mut out = None;

    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => output = OutputMode::Json,
            "--compact" => compact = true,
            "--emit" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{}\n{}", locale.msg("missing_emit"), locale.usage()))?;
                emit = match value.as_str() {
                    "binding" => EmitTarget::Binding,
                    "explain" => EmitTarget::Explain,
                    "frontend" => EmitTarget::Frontend,
                    "diagnostics" => EmitTarget::Diagnostics,
                    "findings" => EmitTarget::Findings,
                    "stages" => EmitTarget::Stages,
                    "envelope" => EmitTarget::Envelope,
                    _ => {
                        return Err(format!(
                            "{}: {value}\n{}",
                            locale.msg("unknown_arg"),
                            locale.usage()
                        ));
                    }
                };
            }
            "--out" => {
                out =
                    Some(iter.next().ok_or_else(|| {
                        format!("{}\n{}", locale.msg("missing_out"), locale.usage())
                    })?);
            }
            "--focus" => {
                let value = iter.next().ok_or_else(|| {
                    format!("{}\n{}", locale.msg("missing_focus"), locale.usage())
                })?;
                focus = Some(value);
            }
            "compile" if path.is_none() => {
                command = Command::Compile;
                emit = EmitTarget::Binding;
            }
            "explain" if path.is_none() => {
                command = Command::Explain;
                emit = EmitTarget::Explain;
            }
            "frontend" if path.is_none() => {
                command = Command::Frontend;
                emit = EmitTarget::Frontend;
            }
            "diagnostics" if path.is_none() => {
                command = Command::Diagnostics;
                emit = EmitTarget::Diagnostics;
            }
            "findings" if path.is_none() => {
                command = Command::Findings;
                emit = EmitTarget::Findings;
            }
            "stages" if path.is_none() => {
                command = Command::Stages;
                emit = EmitTarget::Stages;
            }
            "envelope" if path.is_none() => {
                command = Command::Envelope;
                emit = EmitTarget::Envelope;
            }
            "init" if path.is_none() => {
                command = Command::Init;
            }
            "lock" if path.is_none() => {
                command = Command::Lock;
            }
            value if value.starts_with('-') => {
                return Err(format!(
                    "{}: {value}\n{}",
                    locale.msg("unknown_arg"),
                    locale.usage()
                ));
            }
            value if path.is_none() => path = Some(value.to_string()),
            value => {
                return Err(format!(
                    "{}: {value}\n{}",
                    locale.msg("unknown_arg"),
                    locale.usage()
                ));
            }
        }
    }

    let path = if matches!(command, Command::Init | Command::Lock) {
        path.unwrap_or_else(|| ".".into())
    } else {
        path.ok_or_else(|| format!("{}\n{}", locale.msg("missing_path"), locale.usage()))?
    };
    if focus.is_some()
        && !(command == Command::Explain
            || emit == EmitTarget::Explain
            || command == Command::Frontend
            || emit == EmitTarget::Frontend)
    {
        return Err(format!(
            "{}\n{}",
            locale.msg("invalid_focus"),
            locale.usage()
        ));
    }
    if compact
        && !(command == Command::Explain
            || emit == EmitTarget::Explain
            || command == Command::Frontend
            || emit == EmitTarget::Frontend)
    {
        return Err(format!(
            "{}\n{}",
            locale.msg("invalid_compact"),
            locale.usage()
        ));
    }
    Ok(Cli {
        command,
        emit,
        path,
        output,
        focus,
        compact,
        out,
    })
}

fn run_lock(cli: Cli, locale: UiLocale) {
    let lock = build_lockfile(&cli.path).unwrap_or_else(|err| {
        eprintln!("{}: {err:?}", locale.msg("lock_failed"));
        std::process::exit(1);
    });
    let out_path = cli.out.unwrap_or_else(|| {
        let root = std::path::Path::new(&cli.path);
        if root.is_dir() {
            root.join("gewy.lock").to_string_lossy().into_owned()
        } else {
            root.parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("gewy.lock")
                .to_string_lossy()
                .into_owned()
        }
    });
    fs::write(&out_path, lock).unwrap_or_else(|err| {
        eprintln!("{}: {err}", locale.msg("write_failed"));
        std::process::exit(1);
    });
}

fn run_compile(cli: Cli, locale: UiLocale) {
    let envelope = compile_cli_envelope(&cli.path, locale, "compile_failed");
    let report = envelope.binding.unwrap_or_else(|| {
        eprintln!("{}", locale.msg("compile_failed"));
        std::process::exit(1);
    });
    let out = render_binding_report(&report, render_format(cli.output));
    emit_output(&out, cli.out.as_deref(), locale);
}

fn run_explain(cli: Cli, locale: UiLocale) {
    let report = compile_explain_report_file(&cli.path).unwrap_or_else(|err| {
        eprintln!("{}: {err:?}", locale.msg("explain_failed"));
        std::process::exit(1);
    });
    let focus = parse_explain_focus(cli.focus.as_deref(), locale);
    let out =
        render_explain_report_with_options(&report, render_format(cli.output), focus, cli.compact);
    emit_output(&out, cli.out.as_deref(), locale);
}

fn run_frontend(cli: Cli, locale: UiLocale) {
    let report = compile_frontend_report_file(&cli.path).unwrap_or_else(|err| {
        eprintln!("{}: {err:?}", locale.msg("frontend_failed"));
        std::process::exit(1);
    });
    let focus = parse_frontend_focus(cli.focus.as_deref(), locale);
    let out =
        render_frontend_report_with_options(&report, render_format(cli.output), focus, cli.compact);
    emit_output(&out, cli.out.as_deref(), locale);
}

fn parse_explain_focus(value: Option<&str>, locale: UiLocale) -> Option<ExplainFocus> {
    value.map(|value| match value {
        "parse" => ExplainFocus::Parse,
        "frontend" => ExplainFocus::Frontend,
        "binding" => ExplainFocus::Binding,
        "validation" => ExplainFocus::Validation,
        "diagnostics" => ExplainFocus::Diagnostics,
        "findings" => ExplainFocus::Findings,
        other => {
            eprintln!("{}: {other}\n{}", locale.msg("unknown_arg"), locale.usage());
            std::process::exit(2);
        }
    })
}

fn parse_frontend_focus(value: Option<&str>, locale: UiLocale) -> Option<FrontendFocus> {
    value.map(|value| match value {
        "functions" => FrontendFocus::Functions,
        "includes" => FrontendFocus::Includes,
        "graph" => FrontendFocus::Graph,
        "expansion" => FrontendFocus::Expansion,
        other => {
            eprintln!("{}: {other}\n{}", locale.msg("unknown_arg"), locale.usage());
            std::process::exit(2);
        }
    })
}

fn run_diagnostics(cli: Cli, locale: UiLocale) {
    let envelope = compile_cli_envelope(&cli.path, locale, "diagnostics_failed");
    let report = envelope.diagnostics.unwrap_or_else(|| {
        let err = envelope
            .findings
            .findings
            .first()
            .map(|finding| finding.message.clone())
            .unwrap_or_else(|| locale.msg("diagnostics_failed").to_string());
        eprintln!("{}: {err}", locale.msg("diagnostics_failed"));
        std::process::exit(1);
    });
    let out = render_diagnostics_report(&report, render_format(cli.output));
    emit_output(&out, cli.out.as_deref(), locale);
}

fn run_findings(cli: Cli, locale: UiLocale) {
    let envelope = compile_cli_envelope(&cli.path, locale, "compile_failed");
    let out = render_findings_report(&envelope.findings, render_format(cli.output));
    emit_output(&out, cli.out.as_deref(), locale);
}

fn run_stages(cli: Cli, locale: UiLocale) {
    let envelope = compile_cli_envelope(&cli.path, locale, "stages_failed");
    let out = render_stages_report(&envelope.stages, render_format(cli.output));
    emit_output(&out, cli.out.as_deref(), locale);
}

fn run_envelope(cli: Cli, locale: UiLocale) {
    let envelope = compile_cli_envelope(&cli.path, locale, "compile_failed");
    let out = render_envelope_report(&envelope, render_format(cli.output));
    emit_output(&out, cli.out.as_deref(), locale);
}

fn run_init(cli: Cli, locale: UiLocale) {
    initialize_package(&cli.path).unwrap_or_else(|err| {
        eprintln!("{}: {err}", locale.msg("init_failed"));
        std::process::exit(1);
    });
}

fn initialize_package(dir: &str) -> Result<(), String> {
    let root = std::path::Path::new(dir);
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let package_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != ".")
        .unwrap_or("gewy_app");

    let manifest_path = root.join("gewy.pkg");
    if !manifest_path.exists() {
        fs::write(&manifest_path, render_init_manifest(package_name))
            .map_err(|err| err.to_string())?;
    }

    let entry_path = root.join("main.gewy");
    if !entry_path.exists() {
        fs::write(&entry_path, render_init_entry(package_name)).map_err(|err| err.to_string())?;
    }

    let module_path = root.join("module.gewy");
    if !module_path.exists() {
        fs::write(&module_path, render_init_module(package_name)).map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn render_init_manifest(package_name: &str) -> String {
    format!(
        "name={package_name}\nversion=0.1.0\nentry=main.gewy\n# local deps: dep.std=../stdlib\n"
    )
}

fn render_init_entry(package_name: &str) -> String {
    format!(
        "template(:{package_name})\n|> window(:default_5s)\n|> reason(:udp_datagram_l1)\n|> include(\"./module.gewy\")\n|> use(:network_module)\n"
    )
}

fn render_init_module(package_name: &str) -> String {
    format!(
        "fn network_module() =\n  let model_name = :{package_name}_model\n  let module_name = :{package_name}\n  let op_name = :datagram_exchange\n  |> fragment(:udp_packet_meta_fragment)\n  |> fragment(:route_meta_fragment)\n  |> fragment(:sock_lineage_fragment)\n  |> operation(${{op_name}})\n  |> program_model(${{model_name}})\n  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: ${{module_name}}, phase: :bind)\n  |> program_rule(predicate: \"datagram_observed:udp\", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true, module: ${{module_name}}, phase: :send_request)\n  |> param(:sock_lineage_fragment.capture_comm, true)\n"
    )
}

fn compile_cli_envelope(path: &str, locale: UiLocale, error_key: &str) -> CompilerEnvelope {
    compile_envelope_file(path).unwrap_or_else(|err| {
        eprintln!("{}: {err:?}", locale.msg(error_key));
        std::process::exit(1);
    })
}

fn render_format(output: OutputMode) -> RenderFormat {
    match output {
        OutputMode::Text => RenderFormat::Text,
        OutputMode::Json => RenderFormat::Json,
    }
}

fn emit_output(rendered: &str, out: Option<&str>, locale: UiLocale) {
    if let Some(path) = out {
        fs::write(path, rendered).unwrap_or_else(|err| {
            eprintln!("{}: {err}", locale.msg("write_failed"));
            std::process::exit(1);
        });
        return;
    }
    println!("{rendered}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use gewyvern::gewyc::{
        RenderFormat, compile_binding_report_file, compile_envelope_file,
        compile_explain_report_file, compile_frontend_report_file, render_binding_report,
        render_envelope_report, render_explain_report_with_focus,
        render_explain_report_with_options, render_frontend_report,
        render_frontend_report_with_focus, render_frontend_report_with_options,
    };

    #[test]
    fn parse_cli_defaults_to_compile_command() {
        let cli = parse_cli(
            vec!["gewyc".into(), "dsl/udp_process_debug.gewy".into()],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(
            cli,
            Cli {
                command: Command::Compile,
                emit: EmitTarget::Binding,
                path: "dsl/udp_process_debug.gewy".into(),
                output: OutputMode::Text,
                focus: None,
                compact: false,
                out: None,
            }
        );
    }

    #[test]
    fn parse_cli_accepts_diagnostics_json_mode() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "diagnostics".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--json".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.command, Command::Diagnostics);
        assert_eq!(cli.emit, EmitTarget::Diagnostics);
        assert_eq!(cli.output, OutputMode::Json);
    }

    #[test]
    fn parse_cli_accepts_frontend_command() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "frontend".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--json".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.command, Command::Frontend);
        assert_eq!(cli.emit, EmitTarget::Frontend);
        assert_eq!(cli.output, OutputMode::Json);
    }

    #[test]
    fn parse_cli_accepts_explain_command() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "explain".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--json".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.command, Command::Explain);
        assert_eq!(cli.emit, EmitTarget::Explain);
        assert_eq!(cli.output, OutputMode::Json);
    }

    #[test]
    fn parse_cli_accepts_explain_focus() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "explain".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--focus".into(),
                "validation".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.focus.as_deref(), Some("validation"));
    }

    #[test]
    fn parse_cli_accepts_explain_binding_focus() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "explain".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--focus".into(),
                "binding".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.focus.as_deref(), Some("binding"));
    }

    #[test]
    fn parse_cli_accepts_frontend_focus() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "frontend".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--focus".into(),
                "graph".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.focus.as_deref(), Some("graph"));
    }

    #[test]
    fn parse_cli_accepts_compact_for_explain() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "explain".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--compact".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert!(cli.compact);
    }

    #[test]
    fn parse_cli_rejects_focus_outside_explain() {
        let err = parse_cli(
            vec![
                "gewyc".into(),
                "diagnostics".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--focus".into(),
                "parse".into(),
            ],
            UiLocale::En,
        )
        .unwrap_err();
        assert!(err.contains("--focus is only valid with explain/frontend"));
    }

    #[test]
    fn parse_cli_rejects_compact_outside_explain() {
        let err = parse_cli(
            vec![
                "gewyc".into(),
                "diagnostics".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--compact".into(),
            ],
            UiLocale::En,
        )
        .unwrap_err();
        assert!(err.contains("--compact is only valid with explain/frontend"));
    }

    #[test]
    fn parse_cli_accepts_emit_and_out_flags() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--emit".into(),
                "diagnostics".into(),
                "--out".into(),
                "/tmp/gewyc-out.json".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.command, Command::Compile);
        assert_eq!(cli.emit, EmitTarget::Diagnostics);
        assert_eq!(cli.out.as_deref(), Some("/tmp/gewyc-out.json"));
    }

    #[test]
    fn parse_cli_accepts_findings_command() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "findings".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--json".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.command, Command::Findings);
        assert_eq!(cli.emit, EmitTarget::Findings);
        assert_eq!(cli.output, OutputMode::Json);
    }

    #[test]
    fn parse_cli_accepts_stages_command() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "stages".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--json".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.command, Command::Stages);
        assert_eq!(cli.emit, EmitTarget::Stages);
        assert_eq!(cli.output, OutputMode::Json);
    }

    #[test]
    fn parse_cli_accepts_envelope_command() {
        let cli = parse_cli(
            vec![
                "gewyc".into(),
                "envelope".into(),
                "dsl/udp_process_debug.gewy".into(),
                "--json".into(),
            ],
            UiLocale::En,
        )
        .unwrap();
        assert_eq!(cli.command, Command::Envelope);
        assert_eq!(cli.emit, EmitTarget::Envelope);
        assert_eq!(cli.output, OutputMode::Json);
    }

    #[test]
    fn parse_cli_accepts_init_without_path() {
        let cli = parse_cli(vec!["gewyc".into(), "init".into()], UiLocale::En).unwrap();
        assert_eq!(cli.command, Command::Init);
        assert_eq!(cli.path, ".");
    }

    #[test]
    fn parse_cli_accepts_lock_without_path() {
        let cli = parse_cli(vec!["gewyc".into(), "lock".into()], UiLocale::En).unwrap();
        assert_eq!(cli.command, Command::Lock);
        assert_eq!(cli.path, ".");
    }

    #[test]
    fn binding_json_mentions_template_id() {
        let report = compile_binding_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let json = render_binding_report(&report, RenderFormat::Json);
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
        assert!(json.contains("\"program_model\""));
    }

    #[test]
    fn cli_envelope_collects_binding_and_stages_from_shared_entrypoint() {
        let envelope =
            compile_envelope_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        assert_eq!(
            envelope
                .binding
                .as_ref()
                .map(|report| report.template_id.as_str()),
            Some("udp_process_debug")
        );
        assert!(envelope.stages.parse.ok);
        assert!(envelope.stages.validation.ok);
    }

    #[test]
    fn envelope_json_mentions_all_surfaces() {
        let envelope =
            compile_envelope_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        let json = render_envelope_report(&envelope, RenderFormat::Json);
        assert!(json.contains("\"binding\":"));
        assert!(json.contains("\"diagnostics\":"));
        assert!(json.contains("\"findings\":{\"findings\":[]}"));
        assert!(json.contains("\"stages\":"));
    }

    #[test]
    fn init_templates_include_manifest_and_main_entry() {
        assert!(render_init_manifest("demo").contains("entry=main.gewy"));
        assert!(render_init_entry("demo").contains("|> include(\"./module.gewy\")"));
        assert!(render_init_entry("demo").contains("|> use(:network_module)"));
        assert!(render_init_module("demo").contains("fn network_module() ="));
        assert!(render_init_module("demo").contains("let model_name = :demo_model"));
        assert!(render_init_module("demo").contains("|> program_model(${model_name})"));
    }

    #[test]
    fn frontend_command_renders_pipeline_graph_summary() {
        let report = compile_frontend_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_frontend_report(&report, RenderFormat::Text);
        let json = render_frontend_report(&report, RenderFormat::Json);
        assert!(text.contains("kind=pipeline"));
        assert!(text.contains("function_nodes:"));
        assert!(text.contains("graph_nodes:"));
        assert!(text.contains("graph_edges:"));
        assert!(json.contains("\"kind\":\"pipeline\""));
        assert!(json.contains("\"graph_edges\""));
    }

    #[test]
    fn frontend_command_focuses_graph_section() {
        let report = compile_frontend_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_frontend_report_with_focus(
            &report,
            RenderFormat::Text,
            Some(FrontendFocus::Graph),
        );
        let json = render_frontend_report_with_focus(
            &report,
            RenderFormat::Json,
            Some(FrontendFocus::Graph),
        );
        assert!(text.contains("focus=graph"));
        assert!(text.contains("graph_nodes:"));
        assert!(text.contains("graph_edges:"));
        assert!(json.contains("\"focus\":\"graph\""));
        assert!(json.contains("\"focused_report\""));
    }

    #[test]
    fn frontend_command_focuses_expansion_section() {
        let report = compile_frontend_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_frontend_report_with_focus(
            &report,
            RenderFormat::Text,
            Some(FrontendFocus::Expansion),
        );
        let json = render_frontend_report_with_focus(
            &report,
            RenderFormat::Json,
            Some(FrontendFocus::Expansion),
        );
        assert!(text.contains("focus=expansion"));
        assert!(text.contains("expansion_previews:"));
        assert!(json.contains("\"focus\":\"expansion\""));
        assert!(json.contains("\"expansion_previews\""));
    }

    #[test]
    fn frontend_command_compact_text_stays_short() {
        let report = compile_frontend_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_frontend_report_with_options(&report, RenderFormat::Text, None, true);
        assert!(text.contains("kind=pipeline"));
        assert!(text.contains("includes="));
        assert!(!text.contains("function_nodes:"));
        assert!(!text.contains("graph_nodes:"));
    }

    #[test]
    fn explain_command_renders_human_oriented_compiler_summary() {
        let report = compile_explain_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_explain_report(&report, RenderFormat::Text);
        let json = render_explain_report(&report, RenderFormat::Json);
        assert!(text.contains("surface=explain"));
        assert!(text.contains("frontend:"));
        assert!(text.contains("validation:"));
        assert!(json.contains("\"summary\""));
        assert!(json.contains("\"findings\""));
    }

    #[test]
    fn explain_command_focuses_validation_section() {
        let report = compile_explain_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_explain_report_with_focus(
            &report,
            RenderFormat::Text,
            Some(ExplainFocus::Validation),
        );
        let json = render_explain_report_with_focus(
            &report,
            RenderFormat::Json,
            Some(ExplainFocus::Validation),
        );
        assert!(text.contains("focus=validation"));
        assert!(text.contains("unsupported_payload_offsets="));
        assert!(json.contains("\"focus\":\"validation\""));
        assert!(json.contains("\"focused_report\""));
    }

    #[test]
    fn explain_command_focuses_binding_section() {
        let report = compile_explain_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_explain_report_with_focus(
            &report,
            RenderFormat::Text,
            Some(ExplainFocus::Binding),
        );
        let json = render_explain_report_with_focus(
            &report,
            RenderFormat::Json,
            Some(ExplainFocus::Binding),
        );
        assert!(text.contains("focus=binding"));
        assert!(text.contains("lowered_binding_summary="));
        assert!(text.contains("binding_delta"));
        assert!(text.contains("binding_note="));
        assert!(json.contains("\"focus\":\"binding\""));
        assert!(json.contains("\"lowered_binding_summary\""));
        assert!(json.contains("\"frontend_lowering_delta\""));
        assert!(json.contains("\"binding_shape_note\""));
    }

    #[test]
    fn explain_command_compact_text_stays_short() {
        let report = compile_explain_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let text = render_explain_report_with_options(&report, RenderFormat::Text, None, true);
        assert!(text.contains("surface=explain ok=true"));
        assert!(text.contains("template=udp_process_debug"));
        assert!(!text.contains("frontend:"));
        assert!(!text.contains("validation:"));
    }

    #[test]
    fn lock_writes_resolved_dependency_and_source_entries() {
        let root = std::env::temp_dir().join(format!("gewy-lock-{}", std::process::id()));
        let app_dir = root.join("app");
        let registry_dir = root.join("registry");
        let dep_dir = registry_dir.join("udp_stdlib");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::create_dir_all(&dep_dir).unwrap();
        std::fs::write(
            app_dir.join("gewy.pkg"),
            format!(
                "name=lock_demo\nversion=0.1.0\nentry=main.gewy\nsource.local={}\ndep.std=source:local/udp_stdlib\n",
                registry_dir.to_string_lossy()
            ),
        )
        .unwrap();
        std::fs::write(app_dir.join("main.gewy"), "template(:lock_demo)\n").unwrap();

        let lock = gewyvern::dsl::build_lockfile(app_dir.to_str().unwrap()).unwrap();
        assert!(lock.contains("name=lock_demo"));
        assert!(lock.contains("source.local="));
        assert!(lock.contains("dep.std="));
    }
}
