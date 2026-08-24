use gewyvern::dsl::build_lockfile;
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
            if let Ok(value) = env::var(key)
                && value.to_ascii_lowercase().starts_with("zh")
            {
                return Self::Zh;
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
    let args = env::args().collect::<Vec<_>>();
    if let Some(output) = informational_output(&args, locale) {
        println!("{output}");
        return;
    }
    let cli = parse_cli(args, locale).unwrap_or_else(|err| {
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

fn informational_output(args: &[String], locale: UiLocale) -> Option<String> {
    if args
        .iter()
        .skip(1)
        .any(|argument| matches!(argument.as_str(), "-h" | "--help"))
    {
        return Some(locale.usage().to_string());
    }
    if args
        .iter()
        .skip(1)
        .any(|argument| matches!(argument.as_str(), "-V" | "--version"))
    {
        return Some(format!("gewyc {}", env!("CARGO_PKG_VERSION")));
    }
    None
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
        "ir" => ExplainFocus::Ir,
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
    if root.exists() && !root.is_dir() {
        return Err(format!(
            "init target '{}' exists but is not a directory",
            root.display()
        ));
    }
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let package_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && *name != ".")
        .map(normalize_package_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "gewy_app".to_string());

    let manifest_path = root.join("gewy.pkg");
    if !manifest_path.exists() {
        fs::write(&manifest_path, render_init_manifest(&package_name))
            .map_err(|err| err.to_string())?;
    }

    let entry_path = root.join("main.gewy");
    if !entry_path.exists() {
        fs::write(&entry_path, render_init_entry(&package_name)).map_err(|err| err.to_string())?;
    }

    let module_path = root.join("module.gewy");
    if !module_path.exists() {
        fs::write(&module_path, render_init_module(&package_name))
            .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn normalize_package_name(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut previous_was_separator = false;
    for ch in input.chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            normalized.push(lowered);
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }
    let normalized = normalized.trim_matches('_').to_string();
    if normalized
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        format!("gewy_{normalized}")
    } else {
        normalized
    }
}

fn render_init_manifest(package_name: &str) -> String {
    format!(
        "# gewylang package manifest\n\
         # update name/version when the package gains a clearer identity\n\
         name={package_name}\n\
         version=0.1.0\n\
         entry=main.gewy\n\
         # local deps: dep.std=../stdlib\n"
    )
}

fn render_init_entry(package_name: &str) -> String {
    format!(
        "# main.gewy is the single package entrypoint.\n\
         # keep this file short and move reusable helpers into module.gewy.\n\
         template :{package_name}\n\
         |> window :default_5s\n\
         |> reason :udp_datagram_l1\n\
         |> include \"./module.gewy\"\n\
         |> use :network_module\n"
    )
}

fn render_init_module(package_name: &str) -> String {
    format!(
        concat!(
            "# module.gewy is for reusable function units.\n",
            "# rename network_module once the package tells a more specific story.\n",
            "fn network_module() =\n",
            "  let model_name = :{package_name}_model\n",
            "  let module_name = :{package_name}\n",
            "  let op_name = :datagram_exchange\n",
            "  |> fragment :udp_packet_meta_fragment\n",
            "  |> fragment :route_meta_fragment\n",
            "  |> fragment :sock_lineage_fragment\n",
            "  |> operation $op_name\n",
            "  |> program_model $model_name\n",
            "  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: $module_name, phase: :bind)\n",
            "  |> program_rule(predicate: \"datagram_observed:udp\", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true, module: $module_name, phase: :send_request)\n",
            "  |> param(:sock_lineage_fragment.capture_comm, true)\n",
        ),
        package_name = package_name,
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
mod tests;
