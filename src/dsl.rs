use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::fragment::{EvidenceTier, RegistryError, builtin_registry};
use crate::ir::{
    FlowPredicate, NarrativeTemplate, ObservationScope, PayloadByteMatch, PayloadByteSequenceMatch,
    SignalKind,
};
use crate::ledger::{FactKindTag, PacketDir, QuicFrameType, QuicPacketType};
use crate::program::{ProgramModel, ProgramNarrative, ProgramRule};
use crate::reason::{ReasonKeyEvent, ReasonModel, ReasonNarrative, ReasonProfile, ReasonRule};
use crate::template::{
    FragmentParamValue, Template, TemplateBinding, WindowProfile, default_5s_window,
    default_program_model_for_reason_profile,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const PACKAGE_MANIFEST_FILE: &str = "gewy.pkg";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendDslKind {
    Pipeline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendModuleSummary {
    pub kind: FrontendDslKind,
    pub function_count: usize,
    pub function_nodes: Vec<FrontendFunctionNode>,
    pub merged_step_count: usize,
    pub include_sources: Vec<String>,
    pub use_edges: Vec<FrontendUseEdge>,
    pub graph_nodes: Vec<FrontendGraphNode>,
    pub graph_edges: Vec<FrontendGraphEdge>,
    pub expansion_previews: Vec<FrontendExpansionPreview>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendFunctionNode {
    pub name: String,
    pub step_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendUseEdge {
    pub from: String,
    pub to: String,
    pub line: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendGraphNodeKind {
    Entry,
    File,
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphNode {
    pub id: String,
    pub kind: FrontendGraphNodeKind,
    pub step_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendGraphEdgeKind {
    Include,
    Use,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphEdge {
    pub from: String,
    pub to: String,
    pub kind: FrontendGraphEdgeKind,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendExpansionPreview {
    pub scope: String,
    pub local_bindings: Vec<String>,
    pub steps: Vec<String>,
    pub use_targets: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DslError {
    Located {
        line: usize,
        column: Option<usize>,
        inner: Box<DslError>,
    },
    InvalidLine(String),
    MissingField(&'static str),
    InvalidValue(String),
    Registry(RegistryError),
    Io(String),
}

pub fn read_file(path: &str) -> Result<String, DslError> {
    fs::read_to_string(path).map_err(|err| DslError::Io(err.to_string()))
}

pub fn parse_file_unvalidated(path: &str) -> Result<TemplateBinding, DslError> {
    let package = resolve_package_context(path)?;
    let resolved = package.entry_file.clone();
    let input = read_file(&resolved)?;
    parse_str_unvalidated_with_base(&input, Some(&package))
}

pub fn compile_file(path: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_file_unvalidated(path)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn parse_str_unvalidated(input: &str) -> Result<TemplateBinding, DslError> {
    parse_str_unvalidated_with_base(input, None)
}

pub fn summarize_frontend_file(path: &str) -> Result<FrontendModuleSummary, DslError> {
    let package = resolve_package_context(path)?;
    let resolved = package.entry_file.clone();
    let input = read_file(&resolved)?;
    summarize_frontend_str_with_base(&input, Some(&package))
}

pub fn summarize_frontend_str(input: &str) -> Result<FrontendModuleSummary, DslError> {
    summarize_frontend_str_with_base(input, None)
}

fn parse_str_unvalidated_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<TemplateBinding, DslError> {
    if looks_like_pipeline_dsl(input) {
        let legacy = pipeline_to_legacy(input, package)?;
        return parse_legacy_str_unvalidated(&legacy);
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}

fn summarize_frontend_str_with_base(
    input: &str,
    package: Option<&PackageContext>,
) -> Result<FrontendModuleSummary, DslError> {
    if looks_like_pipeline_dsl(input) {
        let module = parse_pipeline_module(input, package, true)?;
        let function_nodes = module
            .functions
            .iter()
            .map(|(name, function)| FrontendFunctionNode {
                name: name.clone(),
                step_count: function.body.len(),
            })
            .collect();
        let merged_step_count = module.body.len()
            + module
                .functions
                .values()
                .map(|function| function.body.len())
                .sum::<usize>();
        let use_edges = pipeline_use_edges(&module);
        let graph_nodes = pipeline_graph_nodes(&module);
        let graph_edges = pipeline_graph_edges(&module);
        let expansion_previews = pipeline_expansion_previews(&module);
        return Ok(FrontendModuleSummary {
            kind: FrontendDslKind::Pipeline,
            function_count: module.functions.len(),
            function_nodes,
            merged_step_count,
            include_sources: module.include_sources,
            use_edges,
            graph_nodes,
            graph_edges,
            expansion_previews,
        });
    }
    Err(DslError::InvalidValue(
        "gewylang now only supports the pipeline stable subset".into(),
    ))
}

fn pipeline_use_edges(module: &PipelineModule) -> Vec<FrontendUseEdge> {
    let mut edges = Vec::new();
    append_use_edges("entry", &module.body, &mut edges);
    for (function_name, function) in &module.functions {
        append_use_edges(function_name, &function.body, &mut edges);
    }
    edges
}

fn append_use_edges(scope: &str, calls: &[PipelineCall], output: &mut Vec<FrontendUseEdge>) {
    for call in calls {
        if call.name == "use" {
            if let Ok(target) = parse_pipeline_single_arg(&call.args, "use") {
                output.push(FrontendUseEdge {
                    from: scope.to_string(),
                    to: target,
                    line: call.line_no,
                });
            }
        }
    }
}

fn pipeline_graph_nodes(module: &PipelineModule) -> Vec<FrontendGraphNode> {
    let mut nodes = Vec::new();
    nodes.push(FrontendGraphNode {
        id: "entry".to_string(),
        kind: FrontendGraphNodeKind::Entry,
        step_count: Some(module.body.len()),
    });
    for source in &module.include_sources {
        nodes.push(FrontendGraphNode {
            id: format!("file:{source}"),
            kind: FrontendGraphNodeKind::File,
            step_count: None,
        });
    }
    for (name, function) in &module.functions {
        nodes.push(FrontendGraphNode {
            id: format!("fn:{name}"),
            kind: FrontendGraphNodeKind::Function,
            step_count: Some(function.body.len()),
        });
    }
    nodes
}

fn pipeline_graph_edges(module: &PipelineModule) -> Vec<FrontendGraphEdge> {
    let mut edges = module.include_edges.clone();
    for edge in &module.use_edges {
        edges.push(FrontendGraphEdge {
            from: scope_graph_id(&edge.from),
            to: format!("fn:{}", edge.to),
            kind: FrontendGraphEdgeKind::Use,
            line: edge.line,
        });
    }
    edges.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then(left.to.cmp(&right.to))
            .then(
                frontend_graph_edge_kind_rank(left.kind)
                    .cmp(&frontend_graph_edge_kind_rank(right.kind)),
            )
    });
    edges
}

fn pipeline_expansion_previews(module: &PipelineModule) -> Vec<FrontendExpansionPreview> {
    let mut previews = vec![FrontendExpansionPreview {
        scope: "entry".to_string(),
        local_bindings: Vec::new(),
        steps: module.body.iter().map(pipeline_call_preview).collect(),
        use_targets: module
            .body
            .iter()
            .filter(|call| call.name == "use")
            .filter_map(|call| parse_pipeline_single_arg(&call.args, "use").ok())
            .collect(),
    }];
    previews.extend(module.functions.iter().map(|(name, function)| {
        FrontendExpansionPreview {
            scope: name.clone(),
            local_bindings: function
                .local_bindings
                .iter()
                .map(|binding| binding.name.clone())
                .collect(),
            steps: function.body.iter().map(pipeline_call_preview).collect(),
            use_targets: function
                .body
                .iter()
                .filter(|call| call.name == "use")
                .filter_map(|call| parse_pipeline_single_arg(&call.args, "use").ok())
                .collect(),
        }
    }));
    previews
}

fn pipeline_call_preview(call: &PipelineCall) -> String {
    if call.args.is_empty() {
        call.name.clone()
    } else {
        format!("{}({})", call.name, call.args.join(", "))
    }
}

fn scope_graph_id(scope: &str) -> String {
    if scope == "entry" {
        "entry".to_string()
    } else {
        format!("fn:{scope}")
    }
}

fn frontend_graph_edge_kind_rank(kind: FrontendGraphEdgeKind) -> u8 {
    match kind {
        FrontendGraphEdgeKind::Include => 0,
        FrontendGraphEdgeKind::Use => 1,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackageContext {
    root_dir: PathBuf,
    entry_file: String,
    dependencies: BTreeMap<String, PathBuf>,
}

fn resolve_package_context(path: &str) -> Result<PackageContext, DslError> {
    let path = Path::new(path);
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext == "gewy")
    {
        let entry_file = path.to_string_lossy().into_owned();
        let root_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(PackageContext {
            root_dir,
            entry_file,
            dependencies: BTreeMap::new(),
        });
    }

    let manifest_path = if path.is_dir() {
        path.join(PACKAGE_MANIFEST_FILE)
    } else if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == PACKAGE_MANIFEST_FILE)
    {
        path.to_path_buf()
    } else {
        return Ok(PackageContext {
            entry_file: path.to_string_lossy().into_owned(),
            root_dir: path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            dependencies: BTreeMap::new(),
        });
    };

    let manifest = read_package_manifest(&manifest_path)?;
    let package_root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let package_root = canonicalize_existing_path(&package_root)?;
    let entry_path = canonicalize_existing_path(&package_root.join(manifest.entry))?;
    ensure_within_root(&entry_path, &package_root)?;
    Ok(PackageContext {
        entry_file: entry_path.to_string_lossy().into_owned(),
        root_dir: package_root,
        dependencies: manifest.dependencies,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackageManifest {
    name: String,
    version: String,
    entry: String,
    sources: BTreeMap<String, PathBuf>,
    dependencies: BTreeMap<String, PathBuf>,
}

pub fn build_lockfile(path: &str) -> Result<String, DslError> {
    let path = Path::new(path);
    let manifest_path = if path.is_dir() {
        path.join(PACKAGE_MANIFEST_FILE)
    } else {
        path.to_path_buf()
    };
    let manifest = read_package_manifest(&manifest_path)?;
    let package_root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let package_root = canonicalize_existing_path(&package_root)?;

    let mut lines = vec![
        format!("name={}", manifest.name),
        format!("version={}", manifest.version),
        format!("entry={}", manifest.entry),
        format!("root={}", package_root.to_string_lossy()),
    ];

    for (name, source_root) in manifest.sources {
        lines.push(format!("source.{name}={}", source_root.to_string_lossy()));
    }
    for (name, dep_root) in manifest.dependencies {
        lines.push(format!("dep.{name}={}", dep_root.to_string_lossy()));
    }
    Ok(lines.join("\n") + "\n")
}

fn read_package_manifest(path: &Path) -> Result<PackageManifest, DslError> {
    let input = fs::read_to_string(path).map_err(|err| DslError::Io(err.to_string()))?;
    let mut name = None;
    let mut version = None;
    let mut entry = None;
    let mut sources = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    let manifest_root = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| DslError::InvalidLine(line.into()))?;
        let value = value.trim().trim_matches('"').to_string();
        match key.trim() {
            "name" => name = Some(value),
            "version" => version = Some(value),
            "entry" => entry = Some(value),
            source if source.starts_with("source.") => {
                let source_path = canonicalize_existing_path(&manifest_root.join(value))?;
                sources.insert(source["source.".len()..].trim().to_string(), source_path);
            }
            dep if dep.starts_with("dep.") => {
                let dep_path = resolve_dependency_root(&manifest_root, &sources, &value)?;
                dependencies.insert(dep["dep.".len()..].trim().to_string(), dep_path);
            }
            _ => {}
        }
    }

    Ok(PackageManifest {
        name: name.ok_or(DslError::MissingField("name"))?,
        version: version.ok_or(DslError::MissingField("version"))?,
        entry: entry.ok_or(DslError::MissingField("entry"))?,
        sources,
        dependencies,
    })
}

fn resolve_dependency_root(
    manifest_root: &Path,
    sources: &BTreeMap<String, PathBuf>,
    value: &str,
) -> Result<PathBuf, DslError> {
    if let Some(rest) = value.strip_prefix("source:") {
        let (source_name, package_path) = rest.split_once('/').ok_or_else(|| {
            DslError::InvalidValue(format!(
                "invalid source dependency '{value}', expected source:<name>/<package>"
            ))
        })?;
        let source_root = sources.get(source_name).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown package source '{source_name}'"))
        })?;
        return canonicalize_existing_path(&source_root.join(package_path));
    }
    canonicalize_existing_path(&manifest_root.join(value))
}

fn parse_legacy_str_unvalidated(input: &str) -> Result<TemplateBinding, DslError> {
    let mut template_id = None;
    let mut window_profile = None;
    let mut inline_window_duration_ms = None;
    let mut inline_window_lateness_ms = None;
    let mut reason_profile = None;
    let mut reason_model_id = None;
    let mut reason_rules = Vec::new();
    let mut fragment_set = Vec::new();
    let mut program_model_id = None;
    let mut operation = None;
    let mut rules = Vec::new();
    let mut fragment_params = Vec::new();
    let mut evidence_overrides = Vec::new();

    for (line_no, raw_line) in input.lines().enumerate() {
        let line_no = line_no + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| DslError::InvalidLine(line.into()).at_line(line_no))?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "template" => template_id = Some(value.to_string()),
            "window" => {
                window_profile =
                    Some(parse_window_profile(value).map_err(|err| err.at_line(line_no))?)
            }
            "window.duration_ms" => {
                inline_window_duration_ms =
                    Some(parse_u64(value, key).map_err(|err| err.at_line(line_no))?);
            }
            "window.lateness_ms" => {
                inline_window_lateness_ms =
                    Some(parse_u64(value, key).map_err(|err| err.at_line(line_no))?);
            }
            "reason" => {
                reason_profile = Some(ReasonProfile::from_id(value).ok_or_else(|| {
                    DslError::InvalidValue(format!("unknown reason profile '{value}'"))
                        .at_line(line_no)
                })?)
            }
            "reason_model" => reason_model_id = Some(value.to_string()),
            "reason.rule" => {
                reason_rules.push(parse_reason_rule(value).map_err(|err| err.at_line(line_no))?)
            }
            "fragment" => fragment_set.push(value.to_string()),
            "program_model" => program_model_id = Some(value.to_string()),
            "operation" => operation = Some(parse_operation(value)),
            "rule" => rules.push(parse_rule(value).map_err(|err| err.at_line(line_no))?),
            "param" => {
                fragment_params.push(parse_param_entry(value).map_err(|err| err.at_line(line_no))?)
            }
            "evidence" => evidence_overrides
                .push(parse_evidence_override(value).map_err(|err| err.at_line(line_no))?),
            other => {
                return Err(
                    DslError::InvalidValue(format!("unknown DSL key '{other}'")).at_line(line_no)
                );
            }
        }
    }

    let template_id = template_id.ok_or(DslError::MissingField("template"))?;
    let window_profile = build_window_profile(
        window_profile,
        inline_window_duration_ms,
        inline_window_lateness_ms,
    )?;
    let reason_profile =
        build_reason_profile(&template_id, reason_profile, reason_model_id, reason_rules)?;
    let program_model = build_program_model(
        &template_id,
        &reason_profile,
        program_model_id,
        operation,
        rules,
    )?;

    let template = Template {
        id: Box::leak(template_id.into_boxed_str()),
        fragment_set: fragment_set
            .into_iter()
            .map(|item| Box::leak(item.into_boxed_str()) as &'static str)
            .collect(),
        window_profile: Some(window_profile),
        reason_profile: Some(reason_profile),
        program_model: Some(program_model),
    };

    let mut binding = template.bind();
    for (fragment_id, key, value) in fragment_params {
        binding = binding.with_fragment_param(fragment_id, key, value);
    }
    for (fact_kind, tier) in evidence_overrides {
        binding = binding.with_evidence_tier(fact_kind, tier);
    }
    Ok(binding)
}

fn looks_like_pipeline_dsl(input: &str) -> bool {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .next()
        .is_some_and(|line| {
            (line.starts_with("template(") && line.ends_with(')'))
                || parse_pipeline_function_head(line).is_some()
        })
}

fn pipeline_to_legacy(input: &str, package: Option<&PackageContext>) -> Result<String, DslError> {
    let module = parse_pipeline_module(input, package, true)?;
    lower_pipeline_module_to_legacy(&module, true)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineModule {
    template: Option<PipelineCall>,
    body: Vec<PipelineCall>,
    functions: BTreeMap<String, PipelineFunction>,
    include_sources: Vec<String>,
    include_edges: Vec<FrontendGraphEdge>,
    use_edges: Vec<FrontendUseEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineFunction {
    params: Vec<String>,
    local_bindings: Vec<PipelineLetBinding>,
    body: Vec<PipelineCall>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineCall {
    line_no: usize,
    column_no: usize,
    name: String,
    args: Vec<String>,
    arg_columns: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PipelineFunctionBodySyntax {
    Block,
    Expression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineLetBinding {
    name: String,
    value: String,
}

fn parse_pipeline_module(
    input: &str,
    package: Option<&PackageContext>,
    allow_template_head: bool,
) -> Result<PipelineModule, DslError> {
    let mut module = PipelineModule {
        template: None,
        body: Vec::new(),
        functions: BTreeMap::new(),
        include_sources: Vec::new(),
        include_edges: Vec::new(),
        use_edges: Vec::new(),
    };
    let mut include_stack = package
        .map(|package| vec![PathBuf::from(&package.entry_file)])
        .unwrap_or_default();
    parse_pipeline_module_into(
        input,
        package,
        allow_template_head,
        &mut module,
        None,
        "entry",
        &mut include_stack,
    )?;

    if allow_template_head && module.template.is_none() {
        return Err(DslError::InvalidValue(
            "pipeline DSL must start with template(...)".into(),
        ));
    }

    Ok(module)
}

fn parse_pipeline_module_into(
    input: &str,
    package: Option<&PackageContext>,
    allow_template_head: bool,
    module: &mut PipelineModule,
    function_name: Option<&str>,
    source_graph_id: &str,
    include_stack: &mut Vec<PathBuf>,
) -> Result<(), DslError> {
    let lines = input.lines().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < lines.len() {
        let line_no = index + 1;
        let raw_line = lines[index];
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            index += 1;
            continue;
        }

        if let Some((signature, body_syntax)) = parse_pipeline_function_head(line) {
            let signature_column = raw_line.find("fn ").map(|idx| idx + 4).unwrap_or(1);
            let (name, params) = parse_pipeline_function_signature(signature)
                .map_err(|err| err.reanchor_line_column(line_no, signature_column))?;
            let mut local_bindings = Vec::new();
            let mut body = Vec::new();
            index += 1;
            match body_syntax {
                PipelineFunctionBodySyntax::Block => {
                    while index < lines.len() {
                        let raw_body_line = lines[index];
                        let body_line = raw_body_line.trim();
                        if body_line == "}" {
                            break;
                        }
                        if !body_line.is_empty() && !body_line.starts_with('#') {
                            parse_pipeline_function_body_line(
                                module,
                                &name,
                                raw_body_line,
                                body_line,
                                index + 1,
                                &params,
                                &mut local_bindings,
                                &mut body,
                            )?;
                        }
                        index += 1;
                    }
                    if index == lines.len() {
                        return Err(DslError::InvalidValue(
                            "unclosed pipeline function block".into(),
                        )
                        .at_line(line_no));
                    }
                    index += 1;
                }
                PipelineFunctionBodySyntax::Expression => {
                    while index < lines.len() {
                        let raw_body_line = lines[index];
                        let body_line = raw_body_line.trim();
                        if body_line.is_empty() || body_line.starts_with('#') {
                            index += 1;
                            continue;
                        }
                        if !body_line.starts_with("|>") && !body_line.starts_with("let ") {
                            break;
                        }
                        parse_pipeline_function_body_line(
                            module,
                            &name,
                            raw_body_line,
                            body_line,
                            index + 1,
                            &params,
                            &mut local_bindings,
                            &mut body,
                        )?;
                        index += 1;
                    }
                    if body.is_empty() {
                        return Err(DslError::InvalidValue(
                            "pipeline function expressions must contain '|>' steps".into(),
                        )
                        .at_line(line_no));
                    }
                }
            }
            module.functions.insert(
                name.to_string(),
                PipelineFunction {
                    params,
                    local_bindings,
                    body,
                },
            );
            continue;
        }

        let call = if module.template.is_some() || !allow_template_head {
            line.strip_prefix("|>")
                .ok_or_else(|| {
                    DslError::InvalidValue(
                        "pipeline DSL steps after template must start with '|>'".into(),
                    )
                    .at_line(line_no)
                })?
                .trim()
        } else {
            line
        };

        let call_column = raw_line.find(call).map(|idx| idx + 1).unwrap_or(1);
        let (name, args, arg_columns) = parse_pipeline_call(call)
            .map_err(|err| err.reanchor_line_column(line_no, call_column))?;
        match name.as_str() {
            "template" => {
                if module.template.is_some() || !allow_template_head {
                    return Err(DslError::InvalidValue(
                        "pipeline DSL supports exactly one template() head".into(),
                    )
                    .at_line(line_no));
                }
                module.template = Some(PipelineCall {
                    line_no,
                    column_no: call_column,
                    name,
                    args,
                    arg_columns: arg_columns
                        .into_iter()
                        .map(|column| call_column + column.saturating_sub(1))
                        .collect(),
                });
            }
            "include" => {
                let include = parse_pipeline_single_arg(&args, "include")?;
                let package = package.ok_or_else(|| {
                    DslError::InvalidValue(
                        "pipeline include() requires a filesystem-backed entry file".into(),
                    )
                    .at_line(line_no)
                })?;
                let include_path =
                    resolve_include_path(package, &include).map_err(|err| err.at_line(line_no))?;
                if include_stack.contains(&include_path) {
                    return Err(DslError::InvalidValue(format!(
                        "pipeline include cycle detected at '{}'",
                        include_path.to_string_lossy()
                    ))
                    .at_line(line_no));
                }
                module
                    .include_sources
                    .push(include_path.to_string_lossy().into_owned());
                module.include_edges.push(FrontendGraphEdge {
                    from: source_graph_id.to_string(),
                    to: format!("file:{}", include_path.to_string_lossy()),
                    kind: FrontendGraphEdgeKind::Include,
                    line: line_no,
                });
                let include_input = fs::read_to_string(&include_path)
                    .map_err(|err| DslError::Io(err.to_string()).at_line(line_no))?;
                let include_root = include_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| package.root_dir.clone());
                let include_package = PackageContext {
                    root_dir: include_root,
                    entry_file: include_path.to_string_lossy().into_owned(),
                    dependencies: package.dependencies.clone(),
                };
                include_stack.push(include_path.clone());
                parse_pipeline_module_into(
                    &include_input,
                    Some(&include_package),
                    false,
                    module,
                    function_name,
                    &format!("file:{}", include_path.to_string_lossy()),
                    include_stack,
                )
                .map_err(|err| err.at_line(line_no))?;
                include_stack.pop();
            }
            other => {
                let target = if let Some(function_name) = function_name {
                    &mut module
                        .functions
                        .get_mut(function_name)
                        .expect("function exists while parsing")
                        .body
                } else {
                    &mut module.body
                };
                target.push(PipelineCall {
                    line_no,
                    column_no: call_column,
                    name: other.to_string(),
                    args,
                    arg_columns: arg_columns
                        .into_iter()
                        .map(|column| call_column + column.saturating_sub(1))
                        .collect(),
                });
                if other == "use" {
                    if let Ok(target_name) =
                        parse_pipeline_single_arg(&target.last().unwrap().args, "use")
                    {
                        module.use_edges.push(FrontendUseEdge {
                            from: function_name.unwrap_or("entry").to_string(),
                            to: target_name,
                            line: line_no,
                        });
                    }
                }
            }
        }
        index += 1;
    }

    Ok(())
}

fn parse_pipeline_function_head(line: &str) -> Option<(&str, PipelineFunctionBodySyntax)> {
    if let Some(header) = line.strip_suffix('{') {
        let header = header.trim();
        let signature = header.strip_prefix("fn ")?;
        return Some((signature, PipelineFunctionBodySyntax::Block));
    }
    if let Some(header) = line.strip_suffix("=>") {
        let header = header.trim();
        let signature = header.strip_prefix("fn ")?;
        return Some((signature, PipelineFunctionBodySyntax::Expression));
    }
    if let Some(header) = line.strip_suffix('=') {
        let header = header.trim();
        let signature = header.strip_prefix("fn ")?;
        return Some((signature, PipelineFunctionBodySyntax::Expression));
    }
    None
}

fn parse_pipeline_function_body_line(
    module: &mut PipelineModule,
    function_name: &str,
    raw_body_line: &str,
    body_line: &str,
    line_no: usize,
    params: &[String],
    local_bindings: &mut Vec<PipelineLetBinding>,
    output: &mut Vec<PipelineCall>,
) -> Result<(), DslError> {
    let body_column = raw_body_line
        .find(body_line)
        .map(|idx| idx + 1)
        .unwrap_or(1);
    if let Some(binding) = parse_pipeline_let_binding(body_line)
        .map_err(|err| err.reanchor_line_column(line_no, body_column))?
    {
        if params.iter().any(|param| param == &binding.name)
            || local_bindings
                .iter()
                .any(|existing| existing.name == binding.name)
        {
            return Err(DslError::InvalidValue(format!(
                "duplicate pipeline local binding '{}'",
                binding.name
            ))
            .at_line(line_no));
        }
        local_bindings.push(binding);
        return Ok(());
    }
    push_pipeline_function_call(
        module,
        function_name,
        raw_body_line,
        body_line,
        line_no,
        output,
    )
}

fn push_pipeline_function_call(
    module: &mut PipelineModule,
    function_name: &str,
    raw_body_line: &str,
    body_line: &str,
    line_no: usize,
    output: &mut Vec<PipelineCall>,
) -> Result<(), DslError> {
    let nested_call = body_line.strip_prefix("|>").ok_or_else(|| {
        DslError::InvalidValue(
            "pipeline function bodies may only contain 'let' bindings or '|>' steps".into(),
        )
        .at_line(line_no)
    })?;
    let nested_call = nested_call.trim();
    let nested_call_column = raw_body_line
        .find(nested_call)
        .map(|idx| idx + 1)
        .unwrap_or(1);
    let (nested_name, nested_args, nested_arg_columns) = parse_pipeline_call(nested_call)
        .map_err(|err| err.reanchor_line_column(line_no, nested_call_column))?;
    if nested_name == "use" {
        if let Ok(target_name) = parse_pipeline_single_arg(&nested_args, "use") {
            module.use_edges.push(FrontendUseEdge {
                from: function_name.trim().to_string(),
                to: target_name,
                line: line_no,
            });
        }
    }
    output.push(PipelineCall {
        line_no,
        column_no: nested_call_column,
        name: nested_name,
        args: nested_args,
        arg_columns: nested_arg_columns
            .into_iter()
            .map(|column| nested_call_column + column.saturating_sub(1))
            .collect(),
    });
    Ok(())
}

fn parse_pipeline_let_binding(line: &str) -> Result<Option<PipelineLetBinding>, DslError> {
    let Some(remainder) = line.strip_prefix("let ") else {
        return Ok(None);
    };
    let (name, value) = remainder.split_once('=').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid let binding '{line}'"))
            .at_line_column(0, Some(line.len() + 1))
    })?;
    let name = parse_pipeline_param_name(name)?;
    let value = value.trim();
    if value.is_empty() {
        return Err(DslError::InvalidValue(format!(
            "pipeline let binding '{name}' requires a value"
        ))
        .at_line_column(0, Some(line.len() + 1)));
    }
    Ok(Some(PipelineLetBinding {
        name,
        value: value.to_string(),
    }))
}

fn lower_pipeline_module_to_legacy(
    module: &PipelineModule,
    allow_template_head: bool,
) -> Result<String, DslError> {
    let mut output = Vec::<String>::new();
    if let Some(template) = &module.template {
        output.push(format!(
            "template={}",
            parse_pipeline_single_arg(&template.args, "template")
                .map_err(|err| err.at_line(template.line_no))?
        ));
    } else if allow_template_head {
        return Err(DslError::InvalidValue(
            "pipeline DSL must start with template(...)".into(),
        ));
    }

    lower_pipeline_calls(
        &module.body,
        module,
        &mut output,
        allow_template_head,
        &mut Vec::new(),
        &BTreeMap::new(),
    )?;
    Ok(output.join("\n"))
}

fn lower_pipeline_calls(
    calls: &[PipelineCall],
    module: &PipelineModule,
    output: &mut Vec<String>,
    allow_template_head: bool,
    use_stack: &mut Vec<String>,
    bindings: &BTreeMap<String, String>,
) -> Result<(), DslError> {
    for call in calls {
        lower_pipeline_call(
            call,
            module,
            output,
            allow_template_head,
            use_stack,
            bindings,
        )?;
    }
    Ok(())
}

fn lower_pipeline_call(
    call: &PipelineCall,
    module: &PipelineModule,
    output: &mut Vec<String>,
    allow_template_head: bool,
    use_stack: &mut Vec<String>,
    bindings: &BTreeMap<String, String>,
) -> Result<(), DslError> {
    let line_no = call.line_no;
    let column_no = call.column_no;
    let resolved_args = call
        .args
        .iter()
        .map(|arg| substitute_pipeline_arg(arg, bindings))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
    match call.name.as_str() {
        "template" => {
            if !allow_template_head {
                return Err(DslError::InvalidValue(
                    "pipeline DSL supports exactly one template() head".into(),
                )
                .at_line(line_no));
            }
            output.push(format!(
                "template={}",
                parse_pipeline_single_arg(&resolved_args, "template")
                    .map_err(|err| err.reanchor_line_column(line_no, column_no))?
            ));
        }
        "use" => {
            let (function_name, actuals) = parse_pipeline_use_call(&resolved_args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
            if use_stack.contains(&function_name) {
                return Err(DslError::InvalidValue(format!(
                    "pipeline use cycle detected at function '{function_name}'"
                ))
                .at_line(line_no));
            }
            let function = module.functions.get(&function_name).ok_or_else(|| {
                DslError::InvalidValue(format!("unknown pipeline function '{function_name}'"))
                    .at_line(line_no)
            })?;
            if function.params.len() != actuals.len() {
                return Err(DslError::InvalidValue(format!(
                    "pipeline function '{function_name}' expects {} args, got {}",
                    function.params.len(),
                    actuals.len()
                ))
                .at_line(line_no));
            }
            let function_bindings = function
                .params
                .iter()
                .cloned()
                .zip(actuals)
                .collect::<BTreeMap<_, _>>();
            let mut function_bindings = function_bindings;
            for binding in &function.local_bindings {
                let resolved = substitute_pipeline_arg(&binding.value, &function_bindings)
                    .map_err(|err| err.reanchor_line_column(line_no, column_no))?;
                function_bindings.insert(binding.name.clone(), parse_pipeline_literal(&resolved));
            }
            use_stack.push(function_name.clone());
            lower_pipeline_calls(
                &function.body,
                module,
                output,
                false,
                use_stack,
                &function_bindings,
            )?;
            use_stack.pop();
        }
        "include" => {
            return Err(DslError::InvalidValue(
                "pipeline include() should be resolved before lowering".into(),
            )
            .at_line(line_no));
        }
        "window" => lower_pipeline_window(&resolved_args, &call.arg_columns, column_no, output)
            .map_err(|err| err.at_line(line_no))?,
        "reason" => output.push(format!(
            "reason={}",
            parse_pipeline_single_arg(&resolved_args, "reason")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "reason_model" => output.push(format!(
            "reason_model={}",
            parse_pipeline_single_arg(&resolved_args, "reason_model")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "fragment" => output.push(format!(
            "fragment={}",
            parse_pipeline_single_arg(&resolved_args, "fragment")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "program_model" => output.push(format!(
            "program_model={}",
            parse_pipeline_single_arg(&resolved_args, "program_model")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "operation" => output.push(format!(
            "operation={}",
            parse_pipeline_single_arg(&resolved_args, "operation")
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "param" => output.push(format!(
            "param={}",
            lower_pipeline_param(&resolved_args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "evidence" => output.push(format!(
            "evidence={}",
            lower_pipeline_evidence(&resolved_args)
                .map_err(|err| err.reanchor_line_column(line_no, column_no))?
        )),
        "program_rule" => output.push(
            lower_pipeline_rule(&resolved_args, &call.arg_columns, column_no, false)
                .map_err(|err| err.at_line(line_no))?,
        ),
        "reason_rule" => output.push(
            lower_pipeline_rule(&resolved_args, &call.arg_columns, column_no, true)
                .map_err(|err| err.at_line(line_no))?,
        ),
        other => {
            return Err(
                DslError::InvalidValue(format!("unknown pipeline DSL step '{other}'"))
                    .at_line(line_no),
            );
        }
    }
    Ok(())
}

fn resolve_include_path(package: &PackageContext, include: &str) -> Result<PathBuf, DslError> {
    if let Some((dep, file)) = include.split_once(':') {
        let dep_root = package
            .dependencies
            .get(dep)
            .ok_or_else(|| DslError::InvalidValue(format!("unknown package dependency '{dep}'")))?;
        let resolved = canonicalize_existing_path(&dep_root.join(file))?;
        ensure_within_root(&resolved, dep_root)?;
        return Ok(resolved);
    }
    let resolved = canonicalize_existing_path(&package.root_dir.join(include))?;
    ensure_within_root(&resolved, &package.root_dir)?;
    Ok(resolved)
}

fn canonicalize_existing_path(path: &Path) -> Result<PathBuf, DslError> {
    fs::canonicalize(path).map_err(|err| DslError::Io(err.to_string()))
}

fn ensure_within_root(path: &Path, root: &Path) -> Result<(), DslError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(DslError::InvalidValue(format!(
            "path '{}' escapes package root '{}'",
            path.to_string_lossy(),
            root.to_string_lossy()
        )))
    }
}

fn parse_pipeline_call(line: &str) -> Result<(String, Vec<String>, Vec<usize>), DslError> {
    let open = line.find('(').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
            .at_line_column(0, Some(line.len() + 1))
    })?;
    let name = line[..open].trim();
    let inner = line[open + 1..].strip_suffix(')').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
            .at_line_column(0, Some(line.len() + 1))
    })?;
    if name.is_empty() {
        return Err(
            DslError::InvalidValue(format!("invalid pipeline call '{line}'"))
                .at_line_column(0, Some(1)),
        );
    }
    let args_with_columns = split_pipeline_args_with_columns(inner, open + 2);
    Ok((
        name.to_string(),
        args_with_columns
            .iter()
            .map(|(_, value)| value.clone())
            .collect(),
        args_with_columns
            .into_iter()
            .map(|(column, _)| column)
            .collect(),
    ))
}

fn parse_pipeline_function_signature(signature: &str) -> Result<(String, Vec<String>), DslError> {
    let open = signature.find('(').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid function signature '{signature}'"))
            .at_line_column(0, Some(signature.len() + 1))
    })?;
    let close = signature.rfind(')').ok_or_else(|| {
        DslError::InvalidValue(format!("invalid function signature '{signature}'"))
            .at_line_column(0, Some(signature.len() + 1))
    })?;
    if close < open {
        return Err(
            DslError::InvalidValue(format!("invalid function signature '{signature}'"))
                .at_line_column(0, Some(close + 1)),
        );
    }
    let name = signature[..open].trim();
    if name.is_empty() {
        return Err(
            DslError::InvalidValue(format!("invalid function signature '{signature}'"))
                .at_line_column(0, Some(1)),
        );
    }
    let params_src = &signature[open + 1..close];
    let params = if params_src.trim().is_empty() {
        Vec::new()
    } else {
        split_pipeline_args(params_src)
            .into_iter()
            .map(|param| parse_pipeline_param_name(&param))
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok((name.to_string(), params))
}

fn parse_pipeline_param_name(param: &str) -> Result<String, DslError> {
    let trimmed = param.trim();
    let value = trimmed
        .strip_prefix(':')
        .unwrap_or(trimmed)
        .trim()
        .to_string();
    if value.is_empty() {
        return Err(
            DslError::InvalidValue("pipeline parameter name cannot be empty".into())
                .at_line_column(0, Some(1)),
        );
    }
    Ok(value)
}

fn split_pipeline_args(input: &str) -> Vec<String> {
    split_pipeline_args_with_columns(input, 1)
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

fn split_pipeline_args_with_columns(input: &str, base_column: usize) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let chars = input.char_indices().peekable();
    for (idx, ch) in chars {
        match ch {
            '"' => in_string = !in_string,
            ',' if !in_string => {
                let raw = &input[start..idx];
                let trimmed = raw.trim();
                if !trimmed.is_empty() {
                    let leading = raw.find(trimmed).unwrap_or(0);
                    parts.push((base_column + start + leading, trimmed.to_string()));
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    let raw_tail = &input[start..];
    let tail = raw_tail.trim();
    if !tail.is_empty() {
        let leading = raw_tail.find(tail).unwrap_or(0);
        parts.push((base_column + start + leading, tail.to_string()));
    }
    parts
}

fn parse_pipeline_literal(value: &str) -> String {
    let value = value.trim();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        value[1..value.len() - 1].to_string()
    } else if let Some(atom) = value.strip_prefix(':') {
        atom.trim().to_string()
    } else {
        value.to_string()
    }
}

fn parse_pipeline_single_arg(args: &[String], step: &str) -> Result<String, DslError> {
    if args.len() != 1 {
        return Err(DslError::InvalidValue(format!(
            "pipeline step '{step}' expects exactly one argument"
        ))
        .at_line_column(0, Some(1)));
    }
    Ok(parse_pipeline_literal(&args[0]))
}

fn parse_pipeline_use_call(args: &[String]) -> Result<(String, Vec<String>), DslError> {
    if args.is_empty() {
        return Err(DslError::InvalidValue(
            "pipeline step 'use' expects at least one argument".into(),
        )
        .at_line_column(0, Some(1)));
    }
    let function_name = parse_pipeline_literal(&args[0]);
    let actuals = args[1..]
        .iter()
        .map(|arg| parse_pipeline_literal(arg))
        .collect::<Vec<_>>();
    Ok((function_name, actuals))
}

fn substitute_pipeline_arg(
    arg: &str,
    bindings: &BTreeMap<String, String>,
) -> Result<String, DslError> {
    let mut result = arg.to_string();
    while let Some(start) = result.find("${") {
        let tail = &result[start + 2..];
        let end_rel = tail.find('}').ok_or_else(|| {
            DslError::InvalidValue(format!("unclosed pipeline placeholder in '{arg}'"))
                .at_line_column(0, Some(start + 1))
        })?;
        let end = start + 2 + end_rel;
        let key = result[start + 2..end].trim();
        let value = bindings.get(key).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown pipeline parameter '{key}'"))
                .at_line_column(0, Some(start + 3))
        })?;
        result.replace_range(start..=end, value);
    }
    Ok(result)
}

fn looks_like_pipeline_keyword_arg(arg: &str) -> bool {
    let arg = arg.trim();
    if arg.starts_with(':') || arg.starts_with('"') {
        return false;
    }
    arg.split_once(':')
        .is_some_and(|(key, _)| !key.trim().is_empty())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PipelineKeywordArg {
    value: String,
    value_column: usize,
}

fn parse_pipeline_keywords_with_columns(
    args: &[String],
    arg_columns: &[usize],
    step: &str,
) -> Result<BTreeMap<String, PipelineKeywordArg>, DslError> {
    let mut keywords = BTreeMap::new();
    for (arg, arg_column) in args.iter().zip(arg_columns.iter()) {
        let (key, value) = arg.split_once(':').ok_or_else(|| {
            DslError::InvalidValue(format!(
                "pipeline step '{step}' expected keyword argument, got '{arg}'"
            ))
            .at_line_column(0, Some(*arg_column))
        })?;
        let value_trimmed = value.trim();
        let value_offset = value.find(value_trimmed).unwrap_or(0);
        keywords.insert(
            key.trim().to_string(),
            PipelineKeywordArg {
                value: parse_pipeline_literal(value),
                value_column: arg_column + key.len() + 1 + value_offset,
            },
        );
    }
    Ok(keywords)
}

fn lower_pipeline_window(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
    output: &mut Vec<String>,
) -> Result<(), DslError> {
    if args.len() == 1 && !looks_like_pipeline_keyword_arg(&args[0]) {
        output.push(format!("window={}", parse_pipeline_literal(&args[0])));
        return Ok(());
    }
    let keywords = parse_pipeline_keywords_with_columns(args, arg_columns, "window")?;
    let duration_ms = keywords
        .get("duration_ms")
        .ok_or(DslError::MissingField("duration_ms").at_line_column(0, Some(call_column)))?;
    let lateness_ms = keywords
        .get("lateness_ms")
        .ok_or(DslError::MissingField("lateness_ms").at_line_column(0, Some(call_column)))?;
    output.push(format!("window.duration_ms={}", duration_ms.value));
    output.push(format!("window.lateness_ms={}", lateness_ms.value));
    Ok(())
}

fn lower_pipeline_param(args: &[String]) -> Result<String, DslError> {
    if args.len() != 2 {
        return Err(DslError::InvalidValue(
            "pipeline step 'param' expects target and value".into(),
        )
        .at_line_column(0, Some(1)));
    }
    Ok(format!(
        "{}={}",
        parse_pipeline_literal(&args[0]),
        parse_pipeline_literal(&args[1])
    ))
}

fn lower_pipeline_evidence(args: &[String]) -> Result<String, DslError> {
    if args.len() != 2 {
        return Err(DslError::InvalidValue(
            "pipeline step 'evidence' expects fact kind and tier".into(),
        )
        .at_line_column(0, Some(1)));
    }
    Ok(format!(
        "{}:{}",
        parse_pipeline_literal(&args[0]),
        parse_pipeline_literal(&args[1])
    ))
}

fn lower_pipeline_rule(
    args: &[String],
    arg_columns: &[usize],
    call_column: usize,
    reason_rule: bool,
) -> Result<String, DslError> {
    let keywords = parse_pipeline_keywords_with_columns(
        args,
        arg_columns,
        if reason_rule {
            "reason_rule"
        } else {
            "program_rule"
        },
    )?;
    let predicate = keywords
        .get("predicate")
        .ok_or(DslError::MissingField("predicate").at_line_column(0, Some(call_column)))?;
    let signal_key = if reason_rule { "key_event" } else { "stage" };
    let signal = keywords
        .get(signal_key)
        .ok_or(DslError::MissingField(signal_key).at_line_column(0, Some(call_column)))?;
    let narrative = keywords
        .get("narrative")
        .ok_or(DslError::MissingField("narrative").at_line_column(0, Some(call_column)))?;
    let dedupe = keywords
        .get("dedupe")
        .ok_or(DslError::MissingField("dedupe").at_line_column(0, Some(call_column)))?;
    parse_flow_predicate(&predicate.value)
        .map_err(|err| err.reanchor_line_column(0, predicate.value_column))?;
    if reason_rule {
        parse_reason_key_event(&signal.value)
            .map_err(|err| err.reanchor_line_column(0, signal.value_column))?;
    } else {
        parse_stage(&signal.value)
            .map_err(|err| err.reanchor_line_column(0, signal.value_column))?;
    }
    parse_bool(&dedupe.value).map_err(|err| err.reanchor_line_column(0, dedupe.value_column))?;
    let mut value = format!(
        "{};{};{};{}",
        predicate.value, signal.value, narrative.value, dedupe.value
    );
    if let Some(module) = keywords.get("module") {
        value.push(';');
        value.push_str(&module.value);
        if let Some(phase) = keywords.get("phase") {
            value.push(';');
            value.push_str(&phase.value);
        }
    } else if let Some(phase) = keywords.get("phase") {
        return Err(DslError::InvalidValue(format!(
            "pipeline rule phase '{}' requires module",
            phase.value
        ))
        .at_line_column(0, Some(phase.value_column)));
    }
    Ok(if reason_rule {
        format!("reason.rule={value}")
    } else {
        format!("rule={value}")
    })
}

pub fn compile_str(input: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_str_unvalidated(input)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn validate_compiled_binding(binding: &TemplateBinding) -> Result<(), RegistryError> {
    builtin_registry().validate_binding(binding)
}

impl DslError {
    pub fn at_line(self, line: usize) -> Self {
        self.at_line_column(line, None)
    }

    pub fn at_line_column(self, line: usize, column: Option<usize>) -> Self {
        match self {
            Self::Located {
                line: existing_line,
                column: existing_column,
                inner,
            } => Self::Located {
                line: if existing_line == 0 {
                    line
                } else {
                    existing_line
                },
                column: existing_column.or(column),
                inner,
            },
            other => Self::Located {
                line,
                column,
                inner: Box::new(other),
            },
        }
    }

    pub fn reanchor_line_column(self, line: usize, column_offset: usize) -> Self {
        match self {
            Self::Located {
                line: existing_line,
                column,
                inner,
            } => Self::Located {
                line: if existing_line == 0 {
                    line
                } else {
                    existing_line
                },
                column: column.map(|value| value + column_offset.saturating_sub(1)),
                inner,
            },
            other => Self::Located {
                line,
                column: Some(column_offset),
                inner: Box::new(other),
            },
        }
    }

    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Located { line, .. } => Some(*line),
            _ => None,
        }
    }

    pub fn column(&self) -> Option<usize> {
        match self {
            Self::Located { column, .. } => *column,
            _ => None,
        }
    }

    pub fn root(&self) -> &DslError {
        match self {
            Self::Located { inner, .. } => inner.root(),
            other => other,
        }
    }
}

fn parse_window_profile(value: &str) -> Result<WindowProfile, DslError> {
    match value {
        "default_5s" => Ok(default_5s_window()),
        other => Err(DslError::InvalidValue(format!(
            "unknown window profile '{other}'"
        ))),
    }
}

fn build_window_profile(
    profile: Option<WindowProfile>,
    duration_ms: Option<u64>,
    lateness_ms: Option<u64>,
) -> Result<WindowProfile, DslError> {
    if let Some(profile) = profile {
        return Ok(profile);
    }
    match (duration_ms, lateness_ms) {
        (Some(duration_ms), Some(lateness_ms)) => Ok(WindowProfile {
            id: "inline",
            duration_ms,
            lateness_ms,
        }),
        (None, None) => Err(DslError::MissingField("window")),
        _ => Err(DslError::MissingField("window")),
    }
}

fn build_program_model(
    template_id: &str,
    reason_profile: &ReasonProfile,
    program_model_id: Option<String>,
    operation: Option<ProgramOperation>,
    rules: Vec<ProgramRule>,
) -> Result<ProgramModel, DslError> {
    match (program_model_id, operation, rules.is_empty()) {
        (None, None, true) => Ok(default_program_model_for_reason_profile(reason_profile)),
        (program_model_id, operation, _) => {
            let operation = operation.ok_or(DslError::MissingField("operation"))?;
            let id = program_model_id.unwrap_or_else(|| format!("{template_id}_dsl_model"));
            Ok(ProgramModel {
                id: Box::leak(id.into_boxed_str()),
                operation,
                rules,
            })
        }
    }
}

fn build_reason_profile(
    template_id: &str,
    profile: Option<ReasonProfile>,
    reason_model_id: Option<String>,
    reason_rules: Vec<ReasonRule>,
) -> Result<ReasonProfile, DslError> {
    if reason_rules.is_empty() {
        return profile.ok_or(DslError::MissingField("reason"));
    }

    let id = reason_model_id.unwrap_or_else(|| format!("{template_id}_reason_model"));
    Ok(ReasonProfile::Declarative(ReasonModel {
        id: Box::leak(id.into_boxed_str()),
        rules: reason_rules,
    }))
}

fn parse_operation(value: &str) -> ProgramOperation {
    match value {
        "connect_flow" => ProgramOperation::ConnectFlow,
        "datagram_exchange" => ProgramOperation::DatagramExchange,
        "unknown" => ProgramOperation::Unknown,
        other => ProgramOperation::Custom(other.into()),
    }
}

fn parse_rule(value: &str) -> Result<ProgramRule, DslError> {
    let parts = split_top_level_with_columns(value, ';', 1);
    if !(4..=6).contains(&parts.len()) {
        return Err(DslError::InvalidValue(format!("invalid rule '{value}'"))
            .at_line_column(0, Some(value.len() + 1)));
    }

    Ok(ProgramRule {
        predicate: parse_flow_predicate(&parts[0].1)
            .map_err(|err| err.reanchor_line_column(0, parts[0].0))?,
        signal: parse_stage(&parts[1].1).map_err(|err| err.reanchor_line_column(0, parts[1].0))?,
        narrative: parse_narrative(&parts[2].1),
        dedupe: parse_bool(&parts[3].1).map_err(|err| err.reanchor_line_column(0, parts[3].0))?,
        module: parts.get(4).map(|(_, value)| value.clone()),
        phase: parts.get(5).map(|(_, value)| value.clone()),
    })
}

fn parse_reason_rule(value: &str) -> Result<ReasonRule, DslError> {
    let parts = split_top_level_with_columns(value, ';', 1);
    if !(4..=6).contains(&parts.len()) {
        return Err(
            DslError::InvalidValue(format!("invalid reason rule '{value}'"))
                .at_line_column(0, Some(value.len() + 1)),
        );
    }

    Ok(ReasonRule {
        predicate: parse_flow_predicate(&parts[0].1)
            .map_err(|err| err.reanchor_line_column(0, parts[0].0))?,
        signal: parse_reason_key_event(&parts[1].1)
            .map_err(|err| err.reanchor_line_column(0, parts[1].0))?,
        narrative: parse_reason_narrative(&parts[2].1),
        dedupe: parse_bool(&parts[3].1).map_err(|err| err.reanchor_line_column(0, parts[3].0))?,
        module: parts.get(4).map(|(_, value)| value.clone()),
        phase: parts.get(5).map(|(_, value)| value.clone()),
    })
}

fn parse_stage(value: &str) -> Result<Option<ProgramStageKind>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(
            SignalKind::from_id(other)
                .ok_or_else(|| DslError::InvalidValue(format!("unknown stage '{other}'")))?,
        ),
    })
}

fn parse_narrative(value: &str) -> ProgramNarrative {
    parse_narrative_template(value)
}

fn parse_flow_predicate(value: &str) -> Result<FlowPredicate, DslError> {
    if let Some(inner) = value
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let base_column = value.find(inner).unwrap_or(0) + 1;
        return Ok(FlowPredicate::All(
            split_top_level_with_columns(inner, ',', base_column)
                .into_iter()
                .map(|(column, part)| {
                    parse_flow_predicate(&part).map_err(|err| err.reanchor_line_column(0, column))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Some(inner) = value
        .strip_prefix("any(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let base_column = value.find(inner).unwrap_or(0) + 1;
        return Ok(FlowPredicate::Any(
            split_top_level_with_columns(inner, ',', base_column)
                .into_iter()
                .map(|(column, part)| {
                    parse_flow_predicate(&part).map_err(|err| err.reanchor_line_column(0, column))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    match value {
        "process_bound" => Ok(FlowPredicate::ProcessBound),
        "socket_state_observed" => Ok(FlowPredicate::socket_state_observed(None, None, None)),
        other if other.starts_with("socket_state_observed:") => {
            let suffix = &other["socket_state_observed:".len()..];
            let parts =
                split_qualifier_parts_with_columns(suffix, "socket_state_observed:".len() + 1);
            let mut part_index = 0usize;
            let (first_column, first) = qualifier_part_at(&parts, part_index);
            part_index += 1;
            let (local_port, remote_port, port, port_column) = match first {
                "local" | "sport" => {
                    let (column, value) =
                        qualifier_part_opt(&parts, part_index).ok_or_else(|| {
                            DslError::InvalidValue("missing socket local port qualifier".into())
                                .at_line_column(0, Some(first_column))
                        })?;
                    part_index += 1;
                    (true, false, value, column)
                }
                "remote" | "dport" => {
                    let (column, value) =
                        qualifier_part_opt(&parts, part_index).ok_or_else(|| {
                            DslError::InvalidValue("missing socket remote port qualifier".into())
                                .at_line_column(0, Some(first_column))
                        })?;
                    part_index += 1;
                    (false, true, value, column)
                }
                _ => (false, true, first, first_column),
            };
            let port = parse_named_port(port, "socket_state_observed")
                .map_err(|err| err.reanchor_line_column(0, port_column))?;
            let min_new_state = match qualifier_part_opt(&parts, part_index).map(|(_, value)| value)
            {
                None => None,
                Some("established") => {
                    part_index += 1;
                    Some(3)
                }
                Some(other) => {
                    return Err(DslError::InvalidValue(format!(
                        "unknown socket_state_observed state qualifier '{other}'"
                    ))
                    .at_line_column(0, Some(qualifier_part_at(&parts, part_index).0)));
                }
            };
            if let Some((extra_column, extra)) = qualifier_part_opt(&parts, part_index) {
                return Err(DslError::InvalidValue(format!(
                    "unexpected socket_state_observed suffix '{extra}'"
                ))
                .at_line_column(0, Some(extra_column)));
            }
            Ok(FlowPredicate::socket_state_observed(
                local_port.then_some(port),
                remote_port.then_some(port),
                min_new_state,
            ))
        }
        "route_resolved" => Ok(FlowPredicate::RouteResolved),
        other if other.starts_with("quic_packet_observed:") => {
            let suffix = &other["quic_packet_observed:".len()..];
            let parts =
                split_qualifier_parts_with_columns(suffix, "quic_packet_observed:".len() + 1);
            let mut part_index = 0usize;
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut min_len = None;
            let mut long_header = None;
            let mut packet_type = None;
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "quic_packet_observed",
                    "QUIC",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "min_len" => {
                            min_len = Some(
                                parse_u32_qualifier(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "quic_packet_observed",
                                    "QUIC min_len",
                                    "min_len",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "long_header" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue(
                                        "missing QUIC long_header qualifier".into(),
                                    )
                                    .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            long_header = Some(
                                parse_bool(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        "type" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue("missing QUIC type qualifier".into())
                                        .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            packet_type = Some(
                                parse_quic_packet_type(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unexpected QUIC predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::quic_packet_observed(
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                min_len,
                long_header,
                packet_type,
            ))
        }
        other if other.starts_with("quic_frame_observed:") => {
            let suffix = &other["quic_frame_observed:".len()..];
            let parts =
                split_qualifier_parts_with_columns(suffix, "quic_frame_observed:".len() + 1);
            let mut part_index = 0usize;
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut packet_type = None;
            let mut frame_type = None;
            let mut byte_matches = Vec::new();
            let mut byte_sequences = Vec::new();
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "quic_frame_observed",
                    "QUIC",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "type" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue("missing QUIC type qualifier".into())
                                        .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            packet_type = Some(
                                parse_quic_packet_type(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        "frame" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue("missing QUIC frame qualifier".into())
                                        .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            frame_type = Some(
                                parse_quic_frame_type(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        "byte_at" => {
                            byte_matches.push(
                                parse_payload_byte_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "quic_frame_observed",
                                    "QUIC",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "bytes_at" => {
                            byte_sequences.push(
                                parse_payload_byte_sequence_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "quic_frame_observed",
                                    "QUIC",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unexpected QUIC frame predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::quic_frame_observed(
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                packet_type,
                frame_type.ok_or_else(|| {
                    DslError::InvalidValue(
                        "quic_frame_observed requires a frame:<type> qualifier".into(),
                    )
                    .at_line_column(0, Some("quic_frame_observed:".len() + 1))
                })?,
                byte_matches,
                byte_sequences,
            ))
        }
        other if other.starts_with("datagram_observed:") => {
            let suffix = &other["datagram_observed:".len()..];
            let parts = split_qualifier_parts_with_columns(suffix, "datagram_observed:".len() + 1);
            let mut part_index = 0usize;
            let (proto_column, proto) = qualifier_part_at(&parts, part_index);
            part_index += 1;
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto.parse::<u8>().map_err(|_| {
                    DslError::InvalidValue(format!("unknown datagram proto '{proto}'"))
                        .at_line_column(0, Some(proto_column))
                })?,
            };
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut min_len = None;
            let mut first_byte_mask = None;
            let mut first_byte_value = None;
            let mut prefix2 = None;
            let mut prefix4 = None;
            let mut byte13_mask = None;
            let mut byte13_value = None;
            let mut byte_matches = Vec::new();
            let mut byte_sequences = Vec::new();
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "datagram_observed",
                    "datagram",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "min_len" => {
                            min_len = Some(parse_u32_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram min_len",
                                "min_len",
                            )?);
                        }
                        "byte0_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram byte0_mask",
                                "byte0_mask",
                            )?;
                            first_byte_mask = Some(mask);
                            first_byte_value = Some(value);
                        }
                        "prefix2" => {
                            prefix2 = Some(parse_u16_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram prefix2",
                                "prefix2",
                            )?);
                        }
                        "prefix4" => {
                            prefix4 = Some(parse_u32_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram prefix4",
                                "prefix4",
                            )?);
                        }
                        "byte13_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram byte13_mask",
                                "byte13_mask",
                            )?;
                            byte13_mask = Some(mask);
                            byte13_value = Some(value);
                        }
                        "byte_at" => {
                            byte_matches.push(
                                parse_payload_byte_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "datagram_observed",
                                    "datagram",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "bytes_at" => {
                            byte_sequences.push(
                                parse_payload_byte_sequence_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "datagram_observed",
                                    "datagram",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unknown datagram predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::datagram_observed(
                l4_proto,
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                min_len,
                first_byte_mask,
                first_byte_value,
                prefix2,
                prefix4,
                byte13_mask,
                byte13_value,
                byte_matches,
                byte_sequences,
            ))
        }
        other if other.starts_with("packet_observed:") => {
            let suffix = &other["packet_observed:".len()..];
            let parts = split_qualifier_parts_with_columns(suffix, "packet_observed:".len() + 1);
            let mut part_index = 0usize;
            let (proto_column, proto) = qualifier_part_at(&parts, part_index);
            part_index += 1;
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto.parse::<u8>().map_err(|_| {
                    DslError::InvalidValue(format!("unknown packet proto '{proto}'"))
                        .at_line_column(0, Some(proto_column))
                })?,
            };
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut first_byte_mask = None;
            let mut first_byte_value = None;
            let mut prefix4 = None;
            let mut byte4_mask = None;
            let mut byte4_value = None;
            let mut byte13_mask = None;
            let mut byte13_value = None;
            let mut byte_matches = Vec::new();
            let mut byte_sequences = Vec::new();
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "packet_observed",
                    "packet",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "byte0_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet byte0_mask",
                                "byte0_mask",
                            )?;
                            first_byte_mask = Some(mask);
                            first_byte_value = Some(value);
                        }
                        "prefix4" => {
                            prefix4 = Some(parse_u32_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet prefix4",
                                "prefix4",
                            )?);
                        }
                        "byte4_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet byte4_mask",
                                "byte4_mask",
                            )?;
                            byte4_mask = Some(mask);
                            byte4_value = Some(value);
                        }
                        "byte13_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet byte13_mask",
                                "byte13_mask",
                            )?;
                            byte13_mask = Some(mask);
                            byte13_value = Some(value);
                        }
                        "byte_at" => {
                            byte_matches.push(
                                parse_payload_byte_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "packet_observed",
                                    "packet",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "bytes_at" => {
                            byte_sequences.push(
                                parse_payload_byte_sequence_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "packet_observed",
                                    "packet",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unexpected packet predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::packet_observed(
                l4_proto,
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                first_byte_mask,
                first_byte_value,
                prefix4,
                byte4_mask,
                byte4_value,
                byte13_mask,
                byte13_value,
                byte_matches,
                byte_sequences,
            ))
        }
        other => Err(DslError::InvalidValue(format!(
            "unknown predicate '{other}'"
        ))),
    }
}

struct QualifierPartsCursor<'a> {
    parts: &'a [(usize, String)],
    index: &'a mut usize,
}

impl<'a> QualifierPartsCursor<'a> {
    fn new(parts: &'a [(usize, String)], index: &'a mut usize) -> Self {
        Self { parts, index }
    }
}

impl<'a> Iterator for QualifierPartsCursor<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let (_, value) = self.parts.get(*self.index)?;
        *self.index += 1;
        Some(value.as_str())
    }
}

fn split_qualifier_parts_with_columns(input: &str, base_column: usize) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    for (idx, ch) in input.char_indices() {
        if ch == ':' {
            let raw = &input[start..idx];
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let leading = raw.find(trimmed).unwrap_or(0);
                parts.push((base_column + start + leading, trimmed.to_string()));
            }
            start = idx + 1;
        }
    }
    let raw_tail = &input[start..];
    let tail = raw_tail.trim();
    if !tail.is_empty() {
        let leading = raw_tail.find(tail).unwrap_or(0);
        parts.push((base_column + start + leading, tail.to_string()));
    }
    parts
}

fn qualifier_part_at(parts: &[(usize, String)], index: usize) -> (usize, &str) {
    let (column, value) = &parts[index];
    (*column, value.as_str())
}

fn qualifier_part_opt(parts: &[(usize, String)], index: usize) -> Option<(usize, &str)> {
    parts
        .get(index)
        .map(|(column, value)| (*column, value.as_str()))
}

fn parse_reason_key_event(value: &str) -> Result<Option<ReasonKeyEvent>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(SignalKind::from_id(other).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown reason key event '{other}'"))
        })?),
    })
}

fn parse_reason_narrative(value: &str) -> ReasonNarrative {
    parse_narrative_template(value)
}

fn parse_named_port(value: &str, predicate: &str) -> Result<u16, DslError> {
    match value {
        "quic" | "https" | "hy2" | "hysteria2" => Ok(443),
        "http" => Ok(80),
        "dhcp_client" | "bootpc" => Ok(68),
        "dhcp_server" | "bootps" | "dhcp" => Ok(67),
        "mdns" => Ok(5353),
        "ssdp" => Ok(1900),
        "wireguard" => Ok(51820),
        "coap" => Ok(5683),
        "ntp" => Ok(123),
        "stun" => Ok(3478),
        "postgres" => Ok(5432),
        "mysql" => Ok(3306),
        "memcached" => Ok(11211),
        "amqp" => Ok(5672),
        "ldap" => Ok(389),
        "redis" => Ok(6379),
        "mqtt" => Ok(1883),
        "radius" => Ok(1812),
        "gtpu" => Ok(2152),
        "sip" => Ok(5060),
        "rtsp" => Ok(554),
        "socks" | "socks5" => Ok(1080),
        "ftp" => Ok(21),
        "smtp" => Ok(25),
        "imap" => Ok(143),
        "pop3" => Ok(110),
        "ssh" => Ok(22),
        "snmp" => Ok(161),
        "kerberos" => Ok(88),
        other => other
            .parse::<u16>()
            .map_err(|_| DslError::InvalidValue(format!("unknown {predicate} port '{other}'"))),
    }
}

fn parse_u8_literal(value: &str, predicate: &str, field: &str) -> Result<u8, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u8::from_str_radix(hex, 16)
    } else {
        value.parse::<u8>()
    };
    parsed.map_err(|_| DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'")))
}

fn parse_u8_sequence_literal(
    value: &str,
    predicate: &str,
    field: &str,
) -> Result<Vec<u8>, DslError> {
    let bytes = value
        .split(',')
        .map(|byte| parse_u8_literal(byte.trim(), predicate, field))
        .collect::<Result<Vec<_>, _>>()?;
    if bytes.is_empty() {
        return Err(DslError::InvalidValue(format!(
            "invalid {predicate} {field} '{value}'"
        )));
    }
    Ok(bytes)
}

fn parse_payload_byte_match<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
) -> Result<PayloadByteMatch, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let offset = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} byte_at offset qualifier"))
    })?;
    let mask = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} byte_at mask qualifier"))
    })?;
    let value = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} byte_at value qualifier"))
    })?;
    Ok(PayloadByteMatch {
        offset: parse_u16_literal(offset, predicate, "byte_at_offset")?,
        mask: parse_u8_literal(mask, predicate, "byte_at_mask")?,
        value: parse_u8_literal(value, predicate, "byte_at_value")?,
    })
}

fn parse_payload_byte_sequence_match<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
) -> Result<PayloadByteSequenceMatch, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let offset = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!("missing {subject} bytes_at offset qualifier"))
    })?;
    let bytes = parts.next().ok_or_else(|| {
        DslError::InvalidValue(format!(
            "missing {subject} bytes_at byte sequence qualifier"
        ))
    })?;
    Ok(PayloadByteSequenceMatch {
        offset: parse_u16_literal(offset, predicate, "bytes_at_offset")?,
        bytes: parse_u8_sequence_literal(bytes, predicate, "bytes_at")?,
    })
}

fn parse_scope_qualifier<'a, I>(
    part: &str,
    parts: &mut I,
    predicate: &str,
    subject: &str,
    dir: &mut Option<PacketDir>,
    local_port: &mut Option<u16>,
    remote_port: &mut Option<u16>,
) -> Result<bool, DslError>
where
    I: Iterator<Item = &'a str>,
{
    match part {
        "egress" | "local_to_remote" => {
            *dir = Some(PacketDir::Egress);
            Ok(true)
        }
        "ingress" | "remote_to_local" => {
            *dir = Some(PacketDir::Ingress);
            Ok(true)
        }
        "local" | "sport" => {
            let port = parts.next().ok_or_else(|| {
                DslError::InvalidValue(format!("missing {subject} local port qualifier"))
            })?;
            *local_port = Some(parse_named_port(port, predicate)?);
            Ok(true)
        }
        "remote" | "dport" => {
            let port = parts.next().ok_or_else(|| {
                DslError::InvalidValue(format!("missing {subject} remote port qualifier"))
            })?;
            *remote_port = Some(parse_named_port(port, predicate)?);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_u8_mask_value_qualifier<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
    field: &str,
) -> Result<(u8, u8), DslError>
where
    I: Iterator<Item = &'a str>,
{
    let mask = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} mask qualifier")))?;
    let value = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} value qualifier")))?;
    Ok((
        parse_u8_literal(mask, predicate, field)?,
        parse_u8_literal(value, predicate, &format!("{field}_value"))?,
    ))
}

fn parse_u16_qualifier<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
    field: &str,
) -> Result<u16, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let value = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} qualifier")))?;
    parse_u16_literal(value, predicate, field)
}

fn parse_u32_qualifier<'a, I>(
    parts: &mut I,
    predicate: &str,
    subject: &str,
    field: &str,
) -> Result<u32, DslError>
where
    I: Iterator<Item = &'a str>,
{
    let value = parts
        .next()
        .ok_or_else(|| DslError::InvalidValue(format!("missing {subject} qualifier")))?;
    parse_u32_literal(value, predicate, field)
}

fn parse_u16_literal(value: &str, predicate: &str, field: &str) -> Result<u16, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
    } else {
        value.parse::<u16>()
    };
    parsed.map_err(|_| DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'")))
}

fn parse_u32_literal(value: &str, predicate: &str, field: &str) -> Result<u32, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
    } else {
        value.parse::<u32>()
    };
    parsed.map_err(|_| DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'")))
}

fn parse_narrative_template(value: &str) -> NarrativeTemplate {
    match value {
        "none" => NarrativeTemplate::None,
        "process_bound" => NarrativeTemplate::ProcessBound,
        "packet_observed" => NarrativeTemplate::PacketObserved,
        "transport_payload_sent" => NarrativeTemplate::TransportPayloadSent,
        "transport_payload_received" => NarrativeTemplate::TransportPayloadReceived,
        "tcp_state_transition" => NarrativeTemplate::TcpStateTransition,
        "route_changed" => NarrativeTemplate::RouteChanged,
        "udp_datagram_observed" => NarrativeTemplate::UdpDatagramObserved,
        "udp_datagram_sent" => NarrativeTemplate::UdpDatagramSent,
        "udp_datagram_received" => NarrativeTemplate::UdpDatagramReceived,
        other if other.starts_with("static:") => {
            NarrativeTemplate::Static(Box::leak(other[7..].to_string().into_boxed_str()))
        }
        other => NarrativeTemplate::Static(Box::leak(other.to_string().into_boxed_str())),
    }
}

fn parse_param_entry(
    value: &str,
) -> Result<(&'static str, &'static str, FragmentParamValue), DslError> {
    let (lhs, rhs) = value
        .split_once('=')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param '{value}'")))?;
    let (fragment_id, key) = lhs
        .split_once('.')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param target '{lhs}'")))?;

    Ok((
        Box::leak(fragment_id.trim().to_string().into_boxed_str()),
        Box::leak(key.trim().to_string().into_boxed_str()),
        parse_param_value(rhs.trim())?,
    ))
}

fn parse_evidence_override(value: &str) -> Result<(FactKindTag, EvidenceTier), DslError> {
    let (fact_kind, tier) = value
        .split_once(':')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid evidence override '{value}'")))?;
    let fact_kind = FactKindTag::from_str(fact_kind.trim()).ok_or_else(|| {
        DslError::InvalidValue(format!("unknown evidence fact kind '{}'", fact_kind.trim()))
    })?;
    let tier = match tier.trim() {
        "core_requirement" => EvidenceTier::CoreRequirement,
        "optional_enhancement" => EvidenceTier::OptionalEnhancement,
        other => {
            return Err(DslError::InvalidValue(format!(
                "unknown evidence tier '{other}'"
            )));
        }
    };
    Ok((fact_kind, tier))
}

fn parse_param_value(value: &str) -> Result<FragmentParamValue, DslError> {
    if matches!(value, "true" | "false") {
        return Ok(FragmentParamValue::Bool(parse_bool(value)?));
    }
    if let Ok(value) = value.parse::<u64>() {
        return Ok(FragmentParamValue::U64(value));
    }
    Ok(FragmentParamValue::String(
        value.trim_matches('"').to_string(),
    ))
}

fn parse_bool(value: &str) -> Result<bool, DslError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(DslError::InvalidValue(format!("invalid bool '{other}'"))),
    }
}

fn parse_quic_packet_type(value: &str) -> Result<QuicPacketType, DslError> {
    match value {
        "initial" => Ok(QuicPacketType::Initial),
        "0rtt" | "zero_rtt" => Ok(QuicPacketType::ZeroRtt),
        "handshake" => Ok(QuicPacketType::Handshake),
        "retry" => Ok(QuicPacketType::Retry),
        other => Err(DslError::InvalidValue(format!(
            "unknown QUIC packet type '{other}'"
        ))),
    }
}

fn parse_quic_frame_type(value: &str) -> Result<QuicFrameType, DslError> {
    match value {
        "crypto" => Ok(QuicFrameType::Crypto),
        "ack" => Ok(QuicFrameType::Ack),
        "stream" => Ok(QuicFrameType::Stream),
        "datagram" => Ok(QuicFrameType::Datagram),
        "connection_close" | "close" => Ok(QuicFrameType::ConnectionClose),
        other => Err(DslError::InvalidValue(format!(
            "unknown QUIC frame type '{other}'"
        ))),
    }
}

fn parse_u64(value: &str, key: &str) -> Result<u64, DslError> {
    value
        .parse::<u64>()
        .map_err(|_| DslError::InvalidValue(format!("invalid u64 for '{key}': '{value}'")))
}

fn split_top_level_with_columns(
    input: &str,
    delimiter: char,
    base_column: usize,
) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if ch == delimiter && depth == 0 => {
                let raw = &input[start..idx];
                let trimmed = raw.trim();
                let leading = raw.find(trimmed).unwrap_or(0);
                parts.push((base_column + start + leading, trimmed.to_string()));
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    let raw = &input[start..];
    let trimmed = raw.trim();
    let leading = raw.find(trimmed).unwrap_or(0);
    parts.push((base_column + start + leading, trimmed.to_string()));
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rule_reanchors_invalid_stage_column() {
        let input = "process_bound;not_a_stage;static:test;true";
        let err = parse_rule(input).expect_err("invalid stage");
        assert_eq!(err.column(), Some(input.find("not_a_stage").unwrap() + 1));
    }

    #[test]
    fn parse_rule_reanchors_composite_predicate_child_column() {
        let input = "all(process_bound, packet_observed:tcp:remote:mysql:byte_at:not_u16:255:1);connect_flow;static:test;true";
        let err = parse_rule(input).expect_err("invalid composite predicate child");
        assert_eq!(err.column(), Some(input.find("byte_at").unwrap() + 1));
    }

    #[test]
    fn parse_reason_rule_reanchors_invalid_key_event_column() {
        let input = "process_bound;not_a_reason_event;static:test;true";
        let err = parse_reason_rule(input).expect_err("invalid reason key event");
        assert_eq!(
            err.column(),
            Some(input.find("not_a_reason_event").unwrap() + 1)
        );
    }
}
