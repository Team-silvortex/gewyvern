use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::fragment::{builtin_registry, BindingDiagnostics, EvidenceTier, RuleTier};
use gewyvern::ledger::FactKindTag;
use gewyvern::reason::ReasonProfile;
use gewyvern::template::{FragmentParamValue, TemplateBinding};
use std::env;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputMode {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Compile,
    Diagnostics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    command: Command,
    path: String,
    output: OutputMode,
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
                "用法: gewyc <compile|diagnostics> <path.gewy> [--json]\n\
                 用法: gewyc <path.gewy> [--json]"
            }
            Self::En => {
                "usage: gewyc <compile|diagnostics> <path.gewy> [--json]\n\
                 usage: gewyc <path.gewy> [--json]"
            }
        }
    }

    fn msg(self, key: &str) -> &'static str {
        match (self, key) {
            (Self::Zh, "missing_path") => "缺少 .gewy 文件路径",
            (Self::Zh, "unknown_arg") => "未知参数",
            (Self::Zh, "compile_failed") => "DSL 编译失败",
            (Self::Zh, "validate_failed") => "binding 校验失败",
            (Self::Zh, "diagnostics_failed") => "binding 诊断失败",
            (_, "missing_path") => "missing .gewy file path",
            (_, "unknown_arg") => "unknown argument",
            (_, "compile_failed") => "dsl compile failed",
            (_, "validate_failed") => "binding validation failed",
            (_, "diagnostics_failed") => "binding diagnostics failed",
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

    match cli.command {
        Command::Compile => run_compile(cli, locale),
        Command::Diagnostics => run_diagnostics(cli, locale),
    }
}

fn parse_cli(args: Vec<String>, locale: UiLocale) -> Result<Cli, String> {
    let mut command = Command::Compile;
    let mut path = None;
    let mut output = OutputMode::Text;

    for arg in args.into_iter().skip(1) {
        match arg.as_str() {
            "--json" => output = OutputMode::Json,
            "compile" if path.is_none() => command = Command::Compile,
            "diagnostics" if path.is_none() => command = Command::Diagnostics,
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
        path,
        output,
    })
}

fn run_compile(cli: Cli, locale: UiLocale) {
    let binding = compile_file(&cli.path).unwrap_or_else(|err| {
        eprintln!("{}: {err:?}", locale.msg("compile_failed"));
        std::process::exit(1);
    });
    builtin_registry().validate_binding(&binding).unwrap_or_else(|err| {
        eprintln!("{}: {err:?}", locale.msg("validate_failed"));
        std::process::exit(1);
    });

    let out = match cli.output {
        OutputMode::Text => binding_text(&binding),
        OutputMode::Json => binding_json(&binding),
    };
    println!("{out}");
}

fn run_diagnostics(cli: Cli, locale: UiLocale) {
    let binding = compile_file(&cli.path).unwrap_or_else(|err| {
        eprintln!("{}: {err:?}", locale.msg("compile_failed"));
        std::process::exit(1);
    });
    let diagnostics = builtin_registry()
        .binding_diagnostics(&binding)
        .unwrap_or_else(|err| {
            eprintln!("{}: {err:?}", locale.msg("diagnostics_failed"));
            std::process::exit(1);
        });

    let out = match cli.output {
        OutputMode::Text => diagnostics_text(&binding, &diagnostics),
        OutputMode::Json => diagnostics_json(&binding, &diagnostics),
    };
    println!("{out}");
}

fn binding_text(binding: &TemplateBinding) -> String {
    let mut lines = vec![
        format!("template={}", binding.template.id),
        format!("fragments={}", binding.template.fragment_set.join(",")),
    ];

    if let Some(window) = &binding.template.window_profile {
        lines.push(format!(
            "window={} duration_ms={} lateness_ms={}",
            window.id, window.duration_ms, window.lateness_ms
        ));
    }

    if let Some(reason) = &binding.template.reason_profile {
        lines.push(format!("reason={}", reason_profile_text(reason)));
    }

    if let Some(model) = &binding.template.program_model {
        lines.push(format!(
            "program_model={} operation={} rules={}",
            model.id,
            program_operation_text(&model.operation),
            model.rules.len()
        ));
    }

    for (fragment, params) in &binding.fragment_params {
        for (key, value) in params {
            lines.push(format!(
                "param={fragment}.{key}={}",
                fragment_param_text(value)
            ));
        }
    }

    for (fact_kind, tier) in &binding.evidence_overrides {
        lines.push(format!("evidence={fact_kind}:{}", evidence_tier_text(tier)));
    }

    lines.join("\n")
}

fn binding_json(binding: &TemplateBinding) -> String {
    let fragment_params = binding
        .fragment_params
        .iter()
        .map(|(fragment, params)| {
            format!(
                "\"{fragment}\":{{{}}}",
                params
                    .iter()
                    .map(|(key, value)| format!("\"{key}\":{}", fragment_param_json(value)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let evidence_overrides = binding
        .evidence_overrides
        .iter()
        .map(|(fact_kind, tier)| format!("\"{fact_kind}\":\"{}\"", evidence_tier_text(tier)))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        concat!(
            "{{",
            "\"template_id\":\"{}\",",
            "\"fragments\":[{}],",
            "\"window\":{},",
            "\"reason_profile\":{},",
            "\"program_model\":{},",
            "\"fragment_params\":{{{}}},",
            "\"evidence_overrides\":{{{}}}",
            "}}"
        ),
        binding.template.id,
        binding
            .template
            .fragment_set
            .iter()
            .map(|fragment| format!("\"{fragment}\""))
            .collect::<Vec<_>>()
            .join(","),
        binding
            .template
            .window_profile
            .as_ref()
            .map_or("null".into(), |window| format!(
                "{{\"id\":\"{}\",\"duration_ms\":{},\"lateness_ms\":{}}}",
                window.id, window.duration_ms, window.lateness_ms
            )),
        binding
            .template
            .reason_profile
            .as_ref()
            .map_or("null".into(), reason_profile_json),
        binding
            .template
            .program_model
            .as_ref()
            .map_or("null".into(), |model| format!(
                "{{\"id\":\"{}\",\"operation\":\"{}\",\"rules\":{}}}",
                model.id,
                program_operation_text(&model.operation),
                model.rules.len()
            )),
        fragment_params,
        evidence_overrides
    )
}

fn diagnostics_text(binding: &TemplateBinding, diagnostics: &BindingDiagnostics) -> String {
    let mut lines = vec![
        format!("template={}", binding.template.id),
        format!("fragments={}", binding.template.fragment_set.join(",")),
    ];

    if let Some(model) = &diagnostics.program_model {
        lines.push(format!("program_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  program_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                rule_tier_text(&rule.tier),
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
            ));
        }
    }

    if let Some(model) = &diagnostics.reason_model {
        lines.push(format!("reason_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  reason_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                rule_tier_text(&rule.tier),
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
            ));
        }
    }

    lines.join("\n")
}

fn diagnostics_json(binding: &TemplateBinding, diagnostics: &BindingDiagnostics) -> String {
    format!(
        concat!(
            "{{",
            "\"template_id\":\"{}\",",
            "\"fragments\":[{}],",
            "\"program_model\":{},",
            "\"reason_model\":{}",
            "}}"
        ),
        binding.template.id,
        binding
            .template
            .fragment_set
            .iter()
            .map(|fragment| format!("\"{fragment}\""))
            .collect::<Vec<_>>()
            .join(","),
        diagnostics.program_model.as_ref().map_or("null".into(), model_diagnostics_json),
        diagnostics.reason_model.as_ref().map_or("null".into(), model_diagnostics_json),
    )
}

fn model_diagnostics_json(model: &gewyvern::fragment::ModelDiagnostics) -> String {
    format!(
        "{{\"model\":\"{}\",\"rules\":[{}]}}",
        model.model,
        model
            .rules
            .iter()
            .map(|rule| format!(
                "{{\"rule_index\":{},\"tier\":\"{}\",\"supported\":{},\"required_facts\":[{}],\"supporting_fragments\":[{}],\"missing_facts\":[{}]}}",
                rule.rule_index,
                rule_tier_text(&rule.tier),
                rule.supported,
                fact_tag_json_list(&rule.required_facts),
                string_json_list(&rule.supporting_fragments),
                fact_tag_json_list(&rule.missing_facts),
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn fact_tag_json_list(items: &[FactKindTag]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(",")
}

fn string_json_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(",")
}

fn reason_profile_text(profile: &ReasonProfile) -> String {
    match profile {
        ReasonProfile::HandshakeL1 | ReasonProfile::UdpDatagramL1 => profile.id().into(),
        ReasonProfile::Declarative(model) => format!("declarative:{} rules={}", model.id, model.rules.len()),
    }
}

fn reason_profile_json(profile: &ReasonProfile) -> String {
    match profile {
        ReasonProfile::HandshakeL1 | ReasonProfile::UdpDatagramL1 => {
            format!("{{\"kind\":\"builtin\",\"id\":\"{}\"}}", profile.id())
        }
        ReasonProfile::Declarative(model) => format!(
            "{{\"kind\":\"declarative\",\"id\":\"{}\",\"rules\":{}}}",
            model.id,
            model.rules.len()
        ),
    }
}

fn program_operation_text(operation: &ProgramOperation) -> &str {
    match operation {
        ProgramOperation::ConnectFlow => "connect_flow",
        ProgramOperation::DatagramExchange => "datagram_exchange",
        ProgramOperation::Custom(id) => id.as_str(),
        ProgramOperation::Unknown => "unknown",
    }
}

fn fragment_param_text(value: &FragmentParamValue) -> String {
    match value {
        FragmentParamValue::Bool(value) => value.to_string(),
        FragmentParamValue::U64(value) => value.to_string(),
        FragmentParamValue::String(value) => value.clone(),
    }
}

fn fragment_param_json(value: &FragmentParamValue) -> String {
    match value {
        FragmentParamValue::Bool(value) => value.to_string(),
        FragmentParamValue::U64(value) => value.to_string(),
        FragmentParamValue::String(value) => format!("\"{value}\""),
    }
}

fn evidence_tier_text(tier: &EvidenceTier) -> &'static str {
    match tier {
        EvidenceTier::CoreRequirement => "core_requirement",
        EvidenceTier::OptionalEnhancement => "optional_enhancement",
    }
}

fn rule_tier_text(tier: &RuleTier) -> &'static str {
    match tier {
        RuleTier::CoreRequirement => "core_requirement",
        RuleTier::OptionalEnhancement => "optional_enhancement",
        RuleTier::Unsupported => "unsupported",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                path: "dsl/udp_process_debug.gewy".into(),
                output: OutputMode::Text,
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
        assert_eq!(cli.output, OutputMode::Json);
    }

    #[test]
    fn binding_json_mentions_template_id() {
        let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
            .unwrap();
        let json = binding_json(&binding);
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
        assert!(json.contains("\"program_model\""));
    }
}
