use gewylang_ir::{IR_WIRE_FORMAT, IR_WIRE_VERSION, MAX_IR_WIRE_BYTES};
use gewyvern::dsl::{
    GEWYLANG_ANALYSIS_IR_VERSION, GEWYLANG_BINDING_IR_VERSION, GEWYLANG_EXPANDED_AST_VERSION,
    GEWYLANG_LANGUAGE_ID, GEWYLANG_SYNTAX_VERSION, GewyLangContractStamp, GewyLangStage,
    compile_str,
};
use gewyvern::fragment::builtin_registry;
use std::fs;
use std::path::{Path, PathBuf};

const AUTHORITATIVE_DOCS: &[&str] = &[
    "docs/gewylang-llm-guide.md",
    "docs/gewylang-style.md",
    "docs/gewylang-migration.md",
    "docs/gewylang-contract.md",
    "docs/gewylang.ebnf",
    "docs/dsl.md",
    "docs/dsl-syntax.md",
    "docs/dsl-reference.md",
    "docs/gewylang-system.md",
    "docs/gewylang-evolution.md",
    "docs/book/tutorial-gewylang-package.md",
    "docs/book/reference-gewylang-package.md",
    "docs/book/explanation-gewylang-to-ir.md",
    "docs/book/explanation-gewylang-lightweight-types.md",
    "docs/book/reference-ir-lowering.md",
];

#[test]
fn language_contract_schema_matches_the_rust_stage_contract() {
    let root = repository_root();
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/contracts/gewylang-language-contract-v1.schema.json"))
            .expect("GewyLang contract schema must be readable"),
    )
    .expect("GewyLang contract schema must be valid JSON");

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["properties"]["language"]["const"],
        GEWYLANG_LANGUAGE_ID
    );
    assert_eq!(
        schema["properties"]["syntax_version"]["const"],
        GEWYLANG_SYNTAX_VERSION
    );
    assert_eq!(schema["properties"]["stage_version"]["const"], 1);

    let schema_stages = schema["properties"]["stage"]["enum"]
        .as_array()
        .expect("stage must be an enum")
        .iter()
        .map(|value| value.as_str().expect("stage ids must be strings"))
        .collect::<Vec<_>>();
    let schema_stage_version = schema["properties"]["stage_version"]["const"]
        .as_u64()
        .expect("stage version must be an unsigned integer");
    assert_eq!(
        schema_stages,
        vec!["expanded_ast", "binding_ir", "analysis_ir"]
    );

    for (stage, version) in [
        (GewyLangStage::ExpandedAst, GEWYLANG_EXPANDED_AST_VERSION),
        (GewyLangStage::BindingIr, GEWYLANG_BINDING_IR_VERSION),
        (GewyLangStage::AnalysisIr, GEWYLANG_ANALYSIS_IR_VERSION),
    ] {
        let stamp = GewyLangContractStamp::for_stage(stage);
        assert_eq!(stamp.language, GEWYLANG_LANGUAGE_ID);
        assert_eq!(stamp.syntax_version, GEWYLANG_SYNTAX_VERSION);
        assert_eq!(stamp.stage_version, version);
        assert_eq!(u64::from(version), schema_stage_version);
        assert!(schema_stages.contains(&stamp.stage.id()));
    }

    let contract = fs::read_to_string(root.join("docs/gewylang-contract.md"))
        .expect("GewyLang contract must be readable");
    for required in [
        "Source Syntax v1",
        "Expanded AST v1",
        "Binding IR v1",
        "Analysis IR v1",
        "`gewylang-ir`",
        "`BindingMaterializer`",
        "`CompilerProjectionHost`",
        "Canonical IR Fingerprints",
        "Structural IR Invariants",
        "Standalone IR Wire v1",
        "project_compiler_stages_checked",
        "16777216",
        "source_ir_fingerprint",
        "sha256:v1:",
        "cargo run -p gewyc -- ir",
        "Runtime Projections Are Separate",
        "schema_hint.schema_version",
        "Source Graph Safety Contract",
        "`256` source files",
        "`32` levels",
        "`4194304` bytes",
    ] {
        assert!(
            contract.contains(required),
            "GewyLang contract is missing {required}"
        );
    }
}

#[test]
fn standalone_ir_wire_schema_matches_the_rust_codec_contract() {
    let root = repository_root();
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/contracts/gewylang-ir-wire-v1.schema.json"))
            .expect("GewyLang IR wire schema must be readable"),
    )
    .expect("GewyLang IR wire schema must be valid JSON");

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["$defs"]["binding_envelope"]["properties"]["wire_format"]["const"],
        IR_WIRE_FORMAT
    );
    assert_eq!(
        schema["$defs"]["analysis_envelope"]["properties"]["wire_version"]["const"],
        IR_WIRE_VERSION
    );
    assert_eq!(
        schema["$defs"]["fingerprint"]["properties"]["algorithm"]["const"],
        "sha256"
    );
    assert_eq!(MAX_IR_WIRE_BYTES, 16 * 1024 * 1024);
}

#[test]
fn package_reference_locks_source_graph_resolution_and_budgets() {
    let reference =
        fs::read_to_string(repository_root().join("docs/book/reference-gewylang-package.md"))
            .expect("GewyLang package reference must be readable");
    for required in [
        "Resolution And Resource Limits",
        "`262144` bytes per regular source file",
        "`256` source files per compilation",
        "`32` nested filesystem include levels",
        "`4194304` aggregate source bytes",
        "aliases use the same",
    ] {
        assert!(
            reference.contains(required),
            "GewyLang package reference is missing {required}"
        );
    }
}

#[test]
fn dynamic_narrative_and_parameter_lowering_paths_do_not_leak_static_text() {
    let root = repository_root();
    let predicate = fs::read_to_string(root.join("src/dsl/predicate.rs")).unwrap();
    let materializer = fs::read_to_string(root.join("src/dsl/materializer.rs")).unwrap();
    let semantic_values = fs::read_to_string(root.join("src/dsl/semantic_values.rs")).unwrap();
    let lowering =
        fs::read_to_string(root.join("crates/gewylang-compiler/src/lowering.rs")).unwrap();
    let codec = fs::read_to_string(root.join("src/export/reason_codec/parse.rs")).unwrap();
    let attach_codec = fs::read_to_string(root.join("src/export/attach_codec.rs")).unwrap();
    let attach_decode = fs::read_to_string(root.join("src/export/attach_codec/decode.rs")).unwrap();
    let param_lowering = lowering
        .split("fn lower_pipeline_param")
        .nth(1)
        .and_then(|tail| tail.split("fn lower_pipeline_evidence").next())
        .unwrap();
    let narrative_codec = codec
        .split("fn parse_narrative_template")
        .nth(1)
        .and_then(|tail| tail.split("fn parse_reason_predicate_scope").next())
        .unwrap();

    assert!(!predicate.contains("Box::leak"));
    assert!(!materializer.contains("Box::leak"));
    assert!(!semantic_values.contains("Box::leak"));
    assert!(!param_lowering.contains("Box::leak"));
    assert!(!narrative_codec.contains("Box::leak"));
    assert!(!attach_codec.contains("Box::leak"));
    assert!(!attach_decode.contains("Box::leak"));
}

#[test]
fn canonical_pipeline_lowering_does_not_round_trip_through_legacy_text() {
    let root = repository_root();
    let entry = fs::read_to_string(root.join("src/dsl/entry.rs")).unwrap();
    let compiler = fs::read_to_string(root.join("crates/gewylang-compiler/src/lib.rs")).unwrap();
    let lowering =
        fs::read_to_string(root.join("crates/gewylang-compiler/src/lowering.rs")).unwrap();

    assert!(!entry.contains("pipeline_to_legacy"));
    assert!(!entry.contains("parse_legacy_str_unvalidated"));
    assert!(!lowering.contains("lower_pipeline_module_to_legacy"));
    assert!(entry.contains("lower_and_materialize_pipeline_module"));
    assert!(!entry.contains("build_binding_from_canonical_assignments"));
    assert!(compiler.contains("pub trait BindingMaterializer"));
    assert!(compiler.contains("host.materialize_binding(assignments)"));
    assert!(!entry.contains("build_binding_from_assignments"));
    assert!(!lowering.contains("LegacyAssignment"));
    assert!(!lowering.contains("format!(\"{}={}\""));
    assert!(!lowering.contains("format!(\"{}:{}\""));
}

#[test]
fn compiler_stage_projection_routes_through_the_independent_host_contract() {
    let root = repository_root();
    let contract = fs::read_to_string(root.join("crates/gewylang-ir/src/projection.rs")).unwrap();
    let adapter = fs::read_to_string(root.join("src/gewyc/projection_host.rs")).unwrap();
    let compiler = fs::read_to_string(root.join("src/gewyc.rs")).unwrap();

    assert!(contract.contains("pub trait CompilerProjectionHost"));
    assert!(contract.contains("pub fn project_compiler_stages"));
    assert!(contract.contains("pub fn project_compiler_stages_checked"));
    assert!(adapter.contains("impl CompilerProjectionHost for GewyvernProjectionHost"));
    assert!(compiler.contains("gewylang_ir::project_compiler_stages_checked"));
    assert!(!compiler.contains("ir_report_from_binding(&binding"));
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn canonical_docs_define_the_closed_string_escape_set() {
    let root = repository_root();
    let guide = fs::read_to_string(root.join("docs/gewylang-llm-guide.md")).unwrap();
    let grammar = fs::read_to_string(root.join("docs/gewylang.ebnf")).unwrap();

    assert!(guide.contains("Quoted strings decode exactly five escapes"));
    assert!(guide.contains("GEWYC-PARSE-INVALID-STRING-ESCAPE"));
    assert!(guide.contains("GEWYC-PARSE-INVALID-STRING-CHARACTER"));
    assert!(guide.contains("Parentheses inside quoted strings are literal text"));
    assert!(guide.contains("Inline `window` accepts only `duration_ms` and `lateness_ms`"));
    assert!(guide.contains("GEWYC-PARSE-DUPLICATE-WINDOW-FIELD"));
    assert!(guide.contains("GEWYC-PARSE-UNKNOWN-WINDOW-FIELD"));
    assert!(guide.contains("GEWYC-PARSE-EMPTY-ARGUMENT"));
    assert!(guide.contains("Every named argument requires a value after `:`"));
    assert!(guide.contains("GEWYC-PARSE-UNCLOSED-BLOCK-COMMENT"));
    assert!(guide.contains("GEWYC-PARSE-MULTIPLE-ASSIGNMENT-SEPARATORS"));
    assert!(guide.contains("GEWYC-PARSE-INVALID-ATOM"));
    assert!(guide.contains("GEWYC-PARSE-INVALID-KEYWORD-NAME"));
    assert!(guide.contains("GEWYC-PARSE-INVALID-PLACEHOLDER"));
    assert!(guide.contains("GEWYC-PARSE-INVALID-LITERAL"));
    assert!(guide.contains("GEWYC-PARSE-STRING-INTERPOLATION"));
    assert!(guide.contains("GEWYC-PARSE-INVALID-TEMPLATE-HEAD"));
    assert!(guide.contains("GEWYC-PARSE-SOURCE-TOO-LARGE"));
    assert!(grammar.contains("string_escape"));
    assert!(grammar.contains("String escapes are decoded during lowering"));
    assert!(grammar.contains("Raw control characters are invalid inside strings"));
    assert!(grammar.contains("unquoted nested call parentheses are invalid"));
    assert!(grammar.contains("Keyword fields are unique after rule aliases are normalized"));
    assert!(grammar.contains("Argument and parameter lists contain no empty slots"));
    assert!(grammar.contains("Every `/*` block comment is terminated by `*/`"));
    assert!(grammar.contains("Unquoted `=` appears exactly once in a `let` binding"));
    assert!(grammar.contains("Atom paths contain one or more dot-separated identifiers"));
    assert!(grammar.contains("Function parameter and local binding names are bare identifiers"));
    assert!(grammar.contains("Every line beginning with the `fn` keyword"));
    assert!(grammar.contains("A malformed or empty `template` declaration"));
    assert!(grammar.contains("at most 256 KiB before comment stripping"));
    assert!(grammar.contains("Keyword field names are bare identifiers"));
    assert!(grammar.contains("Every keyword argument has a non-empty value"));
    assert!(
        grammar.contains("Braced, empty, numeric-leading, and suffixed placeholders are invalid")
    );
    assert!(
        grammar.contains("Source values outside the five declared lexical families are invalid")
    );
    assert!(grammar.contains("Strings are opaque values and never interpolate `$name`"));
    assert!(!grammar.contains("raw_token"));
    assert!(!grammar.contains("escapes preserved"));
}

#[test]
fn authoritative_gewylang_docs_have_no_broken_local_links() {
    let root = repository_root();
    let mut checked = 0usize;

    for relative_doc in AUTHORITATIVE_DOCS {
        let doc = root.join(relative_doc);
        let source = fs::read_to_string(&doc).expect("authoritative doc must be readable");
        for target in markdown_link_targets(&source) {
            if target.starts_with('#')
                || target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }

            let path = target.split('#').next().unwrap_or_default();
            if path.is_empty() {
                continue;
            }
            checked += 1;
            let resolved = doc.parent().unwrap_or(Path::new(".")).join(path);
            assert!(
                resolved.exists(),
                "broken local link in {}: {} -> {}",
                relative_doc,
                target,
                resolved.display()
            );
        }
    }

    assert!(checked >= 25, "expected a meaningful local-link contract");
}

#[test]
fn marked_gewylang_examples_compile() {
    let mut checked = 0usize;
    for relative_doc in ["docs/gewylang-llm-guide.md", "docs/gewylang-style.md"] {
        let source = fs::read_to_string(repository_root().join(relative_doc))
            .expect("compiled-example doc must be readable");
        for (index, example) in fenced_blocks(&source, "gewy compile").iter().enumerate() {
            checked += 1;
            compile_str(example).unwrap_or_else(|error| {
                panic!(
                    "{relative_doc} example {} failed to compile: {error:?}",
                    index + 1
                )
            });
        }
    }
    assert!(checked >= 2, "expected compiled examples in canonical docs");
}

#[test]
fn reference_covers_registry_fragments_parameters_and_signal_ids() {
    let reference = fs::read_to_string(repository_root().join("docs/dsl-reference.md"))
        .expect("DSL reference must be readable");

    let registry = builtin_registry();
    for fragment_id in [
        "tcp_state_fragment",
        "tcp_packet_meta_fragment",
        "udp_packet_meta_fragment",
        "route_meta_fragment",
        "sock_lineage_fragment",
    ] {
        let fragment = registry
            .descriptor(fragment_id)
            .unwrap_or_else(|| panic!("built-in registry is missing {fragment_id}"));
        assert!(
            reference.contains(&fragment.id),
            "DSL reference is missing built-in fragment {}",
            fragment.id
        );
        for parameter in &fragment.params {
            assert!(
                reference.contains(&parameter.key),
                "DSL reference is missing parameter {}.{}",
                fragment.id,
                parameter.key
            );
        }
    }

    for signal in [
        "process_bound",
        "socket_state_transition",
        "packet_observed",
        "datagram_observed",
        "route_resolved",
        "syn_seen",
        "udp_datagram_seen",
        "process_identified",
        "state_change",
        "route_changed",
        "fin_or_rst",
    ] {
        assert!(
            reference.contains(signal),
            "DSL reference is missing signal {signal}"
        );
    }
}

#[test]
fn maintained_sources_follow_the_canonical_style() {
    let mut sources = Vec::new();
    collect_gewy_sources(&repository_root().join("dsl"), &mut sources);
    collect_gewy_sources(&repository_root().join("protocols"), &mut sources);
    assert!(
        sources.len() >= 700,
        "expected both maintained source shelves"
    );

    for source_path in sources {
        let source = fs::read_to_string(&source_path).expect("GewyLang source must be readable");
        let template_count = source
            .lines()
            .filter(|line| line.trim_start().starts_with("template :"))
            .count();
        assert_eq!(
            template_count,
            1,
            "{} must contain exactly one canonical template head",
            source_path.display()
        );

        for line in source.lines() {
            let line = line.trim();
            if line.starts_with("fn ") {
                assert!(
                    line.ends_with(" ="),
                    "non-canonical function head in {}: {line}",
                    source_path.display()
                );
            }
            assert_canonical_call(line, &source_path);
            assert_canonical_rule(line, &source_path);
            assert!(
                !is_legacy_assignment(line),
                "legacy assignment in {}: {line}",
                source_path.display()
            );
        }

        assert!(
            !source.contains("${")
                && !source.contains("=>")
                && !source.lines().any(|line| matches!(line.trim(), "{" | "}"))
                && !source
                    .lines()
                    .any(|line| line.trim_start().starts_with("template(")),
            "compatibility syntax found in {}",
            source_path.display()
        );
    }
}

#[test]
fn protocol_package_mirrors_match_their_dsl_sources() {
    let root = repository_root();
    let mut mains = Vec::new();
    collect_named_files(&root.join("protocols"), "main.gewy", &mut mains);
    assert_eq!(mains.len(), 361, "expected every protocol package entry");

    let mut mirrored = 0usize;
    for main in &mains {
        let source = fs::read_to_string(main).expect("protocol main must be readable");
        assert!(
            !source.trim_start().starts_with("include"),
            "include-only package entry is forbidden: {}",
            main.display()
        );
        let template_id = source
            .lines()
            .find_map(|line| line.trim().strip_prefix("template :"))
            .and_then(|rest| rest.split_whitespace().next())
            .expect("canonical package entry must declare a template");
        let dsl = root.join("dsl").join(format!("{template_id}.gewy"));
        if dsl.exists() {
            mirrored += 1;
            let canonical = fs::read_to_string(&dsl).expect("DSL mirror must be readable");
            assert_eq!(
                source,
                canonical,
                "protocol mirror drifted from {}",
                dsl.display()
            );
        }
    }
    assert_eq!(mirrored, 343, "expected every canonical DSL mirror");
    assert_eq!(
        mains.len() - mirrored,
        18,
        "expected the package-only canonical entries"
    );
}

fn assert_canonical_call(line: &str, source_path: &Path) {
    let Some(call) = line.strip_prefix("|>").map(str::trim_start) else {
        return;
    };
    let open = call.find('(');
    let whitespace = call.find(char::is_whitespace);
    if open.is_some_and(|open| whitespace.is_none_or(|whitespace| open < whitespace)) {
        let name = &call[..open.expect("open parenthesis exists")];
        assert_eq!(
            name,
            "window",
            "parenthesized non-window call in {}: {line}",
            source_path.display()
        );
    }
}

fn assert_canonical_rule(line: &str, source_path: &Path) {
    let expected_signal = if line.starts_with("|> program_rule") {
        Some(", stage:")
    } else if line.starts_with("|> reason_rule") {
        Some(", event:")
    } else {
        None
    };
    let Some(expected_signal) = expected_signal else {
        return;
    };
    assert!(
        (line.starts_with("|> program_rule pred:") || line.starts_with("|> reason_rule pred:"))
            && line.contains(expected_signal)
            && line.contains(", narr:")
            && line.contains(", dedupe:"),
        "non-canonical rule in {}: {line}",
        source_path.display()
    );

    let predicate = line
        .split_once("pred: ")
        .map(|(_, rest)| rest.split(',').next().unwrap_or_default())
        .unwrap_or_default();
    assert!(
        !predicate
            .strip_prefix(':')
            .is_some_and(|value| value.contains(':')),
        "complex predicate must be quoted or bound in {}: {line}",
        source_path.display()
    );
}

fn markdown_link_targets(source: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else {
            break;
        };
        targets.push(rest[..end].trim());
        rest = &rest[end + 1..];
    }
    targets
}

fn fenced_blocks(source: &str, info: &str) -> Vec<String> {
    let opening = format!("```{info}");
    let mut blocks = Vec::new();
    let mut current = None::<String>;

    for line in source.lines() {
        if current.is_none() && line.trim() == opening {
            current = Some(String::new());
            continue;
        }
        if line.trim() == "```" {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            continue;
        }
        if let Some(block) = current.as_mut() {
            block.push_str(line);
            block.push('\n');
        }
    }

    blocks
}

fn collect_gewy_sources(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("source directory must be readable") {
        let path = entry.expect("source entry must be readable").path();
        if path.is_dir() {
            collect_gewy_sources(&path, output);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "gewy")
        {
            output.push(path);
        }
    }
}

fn collect_named_files(directory: &Path, file_name: &str, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("source directory must be readable") {
        let path = entry.expect("source entry must be readable").path();
        if path.is_dir() {
            collect_named_files(&path, file_name, output);
        } else if path.file_name().is_some_and(|name| name == file_name) {
            output.push(path);
        }
    }
}

fn is_legacy_assignment(line: &str) -> bool {
    [
        "template=",
        "window=",
        "reason=",
        "fragment=",
        "program_model=",
        "reason_model=",
        "operation=",
        "rule=",
        "reason.rule=",
        "param=",
        "evidence=",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}
