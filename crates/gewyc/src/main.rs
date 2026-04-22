use gewyvern::gewyc::{
    CompilerEnvelope, RenderFormat, compile_envelope_file, render_binding_report,
    render_diagnostics_report, render_envelope_report, render_findings_report,
    render_stages_report,
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
    Diagnostics,
    Findings,
    Stages,
    Envelope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EmitTarget {
    Binding,
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
                "用法: gewyc <compile|diagnostics|findings|stages|envelope> <path.gewy> [--json] [--out path]\n\
                 用法: gewyc <path.gewy> [--json] [--emit binding|diagnostics|findings|stages|envelope] [--out path]"
            }
            Self::En => {
                "usage: gewyc <compile|diagnostics|findings|stages|envelope> <path.gewy> [--json] [--out path]\n\
                 usage: gewyc <path.gewy> [--json] [--emit binding|diagnostics|findings|stages|envelope] [--out path]"
            }
        }
    }

    fn msg(self, key: &str) -> &'static str {
        match (self, key) {
            (Self::Zh, "missing_path") => "缺少 .gewy 文件路径",
            (Self::Zh, "unknown_arg") => "未知参数",
            (Self::Zh, "compile_failed") => "DSL 编译失败",
            (Self::Zh, "diagnostics_failed") => "binding 诊断失败",
            (Self::Zh, "stages_failed") => "compiler 阶段报告生成失败",
            (Self::Zh, "missing_emit") => "缺少 --emit 的值，期望 binding、diagnostics、findings、stages 或 envelope",
            (Self::Zh, "missing_out") => "缺少 --out 的值，期望输出路径",
            (Self::Zh, "write_failed") => "写入输出失败",
            (_, "missing_path") => "missing .gewy file path",
            (_, "unknown_arg") => "unknown argument",
            (_, "compile_failed") => "dsl compile failed",
            (_, "diagnostics_failed") => "binding diagnostics failed",
            (_, "stages_failed") => "compiler stages report failed",
            (_, "missing_emit") => "missing value for --emit, expected binding, diagnostics, findings, stages, or envelope",
            (_, "missing_out") => "missing value for --out, expected an output path",
            (_, "write_failed") => "failed to write output",
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
        EmitTarget::Binding => run_compile(cli, locale),
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
    let mut out = None;

    let mut iter = args.into_iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => output = OutputMode::Json,
            "--emit" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{}\n{}", locale.msg("missing_emit"), locale.usage()))?;
                emit = match value.as_str() {
                    "binding" => EmitTarget::Binding,
                    "diagnostics" => EmitTarget::Diagnostics,
                    "findings" => EmitTarget::Findings,
                    "stages" => EmitTarget::Stages,
                    "envelope" => EmitTarget::Envelope,
                    _ => {
                        return Err(format!(
                            "{}: {value}\n{}",
                            locale.msg("unknown_arg"),
                            locale.usage()
                        ))
                    }
                };
            }
            "--out" => {
                out = Some(
                    iter.next()
                        .ok_or_else(|| format!("{}\n{}", locale.msg("missing_out"), locale.usage()))?,
                );
            }
            "compile" if path.is_none() => {
                command = Command::Compile;
                emit = EmitTarget::Binding;
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
            value if value.starts_with('-') => {
                return Err(format!("{}: {value}\n{}", locale.msg("unknown_arg"), locale.usage()))
            }
            value if path.is_none() => path = Some(value.to_string()),
            value => {
                return Err(format!("{}: {value}\n{}", locale.msg("unknown_arg"), locale.usage()))
            }
        }
    }

    let path = path.ok_or_else(|| format!("{}\n{}", locale.msg("missing_path"), locale.usage()))?;
    Ok(Cli {
        command,
        emit,
        path,
        output,
        out,
    })
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
        RenderFormat, compile_binding_report_file, compile_envelope_file, render_binding_report,
        render_envelope_report,
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
            envelope.binding.as_ref().map(|report| report.template_id.as_str()),
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
}
