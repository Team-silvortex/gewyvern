use std::env;
use std::path::PathBuf;
use std::process;

use gewyvern::project_status::{
    Independence, Lifecycle, Maturity, Priority, StatusCatalog, StatusCellView,
    default_catalog_path,
};

fn main() {
    match run(env::args().skip(1).collect()) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("status failed: {error}");
            process::exit(1);
        }
    }
}

fn run(args: Vec<String>) -> Result<String, String> {
    let options = Options::parse(args)?;
    if options.command == Command::Help {
        return Ok(usage().to_string());
    }
    let catalog = StatusCatalog::load(&options.catalog)?;
    if let Err(errors) = catalog.validate(&options.repository_root) {
        return Err(format!(
            "catalog validation failed:\n- {}",
            errors.join("\n- ")
        ));
    }

    if options.command == Command::Validate {
        return if options.json {
            Ok(format!(
                "{{\"schema_version\":{},\"status\":\"valid\",\"cells\":{},\"coverage_requirements\":{}}}",
                catalog.schema_version,
                catalog.cells.len(),
                catalog.coverage_requirements.len()
            ))
        } else {
            Ok(format!(
                "status catalog valid: schema={} cells={} coverage_requirements={}",
                catalog.schema_version,
                catalog.cells.len(),
                catalog.coverage_requirements.len()
            ))
        };
    }

    let summary = catalog.summary(options.limit);
    if options.command == Command::Summary {
        return if options.json {
            serde_json::to_string_pretty(&summary).map_err(|err| err.to_string())
        } else {
            Ok(render_summary(&summary))
        };
    }

    let mut cells = catalog.views();
    cells.retain(|cell| match options.command {
        Command::Weakest => cell.priority != Priority::Deferred,
        Command::Mature => cell.maturity == Maturity::Mature,
        Command::Standalone => {
            cell.priority != Priority::Deferred
                && cell.independence != Independence::Internal
                && cell.maturity >= Maturity::Stabilizing
                && !matches!(cell.maturity, Maturity::Blocked | Maturity::Deprecated)
        }
        Command::Developing => {
            cell.priority != Priority::Deferred
                && matches!(
                    cell.maturity,
                    Maturity::Incubating
                        | Maturity::Developing
                        | Maturity::Stabilizing
                        | Maturity::Blocked
                )
        }
        Command::Deferred => cell.priority == Priority::Deferred,
        Command::Summary | Command::Validate | Command::Help => unreachable!(),
    });
    if let Some(architecture) = options.architecture.as_deref() {
        cells.retain(|cell| cell.architecture == architecture);
    }
    if let Some(module) = options.module.as_deref() {
        cells.retain(|cell| cell.module == module);
    }
    if let Some(feature) = options.feature.as_deref() {
        cells.retain(|cell| cell.feature == feature);
    }
    if let Some(lifecycle) = options.lifecycle {
        cells.retain(|cell| cell.lifecycle == lifecycle);
    }
    if let Some(maturity) = options.maturity {
        cells.retain(|cell| cell.maturity == maturity);
    }
    if let Some(priority) = options.priority {
        cells.retain(|cell| cell.priority == priority);
    }
    if matches!(
        options.command,
        Command::Weakest | Command::Developing | Command::Deferred
    ) {
        cells.sort_by_key(|cell| (cell.score, cell.id.clone()));
    } else {
        cells.sort_by_key(|cell| (std::cmp::Reverse(cell.score), cell.id.clone()));
    }
    cells.truncate(options.limit);

    if options.json {
        serde_json::to_string_pretty(&cells).map_err(|err| err.to_string())
    } else {
        Ok(render_cells(&cells))
    }
}

fn render_summary(summary: &gewyvern::project_status::StatusSummary) -> String {
    let mut output = format!(
        "{} status tensor\ncheckpoint: {}\ncalibration: {} as-of {}\ndelivery: strength={}/100 completion={}/100 across {} active cells\nportfolio: strength={}/100 completion={}/100 across {} cells (deferred={})\ncoverage: {} requirements across {} architectures (ownership={} gates={} proof={})\n\n",
        summary.project,
        summary.checkpoint,
        summary.calibration.model,
        summary.calibration.as_of,
        summary.overall_score,
        summary.delivery_completion,
        summary.cell_count - summary.deferred_cell_count,
        summary.portfolio_score,
        summary.portfolio_completion,
        summary.cell_count,
        summary.deferred_cell_count,
        summary.coverage.requirement_count,
        summary.coverage.architecture_count,
        summary.coverage.ownership_boundary_count,
        summary.coverage.roadmap_gate_count,
        summary.coverage.proof_shelf_count
    );
    output.push_str("lifecycles:\n");
    for group in &summary.lifecycles {
        output.push_str(&format!(
            "- {} score={}/100 completion={} active={}/{} weakest={}\n",
            group.label,
            group.score,
            group.completion,
            group.active_cell_count,
            group.cell_count,
            group.weakest_cell
        ));
    }
    output.push_str("\narchitectures:\n");
    for group in &summary.architectures {
        output.push_str(&format!(
            "- {} score={}/100 completion={} active={}/{} weakest={}\n",
            group.label,
            group.score,
            group.completion,
            group.active_cell_count,
            group.cell_count,
            group.weakest_cell
        ));
    }
    output.push_str("\nmodules:\n");
    for group in &summary.modules {
        output.push_str(&format!(
            "- {} score={}/100 completion={} active={}/{} weakest={}\n",
            group.label,
            group.score,
            group.completion,
            group.active_cell_count,
            group.cell_count,
            group.weakest_cell
        ));
    }
    output.push_str("\nweakest:\n");
    output.push_str(&render_cells(&summary.weakest));
    output.push_str("\nindependently usable:\n");
    output.push_str(&render_cells(&summary.independently_usable));
    output.push_str("\nin development:\n");
    output.push_str(&render_cells(&summary.in_development));
    output.push_str("\ndeferred:\n");
    output.push_str(&render_cells(&summary.deferred));
    output.trim_end().to_string()
}

fn render_cells(cells: &[StatusCellView]) -> String {
    if cells.is_empty() {
        return "(none)\n".to_string();
    }
    cells
        .iter()
        .map(|cell| {
            format!(
                "- {} score={}/100 priority={:?} maturity={:?} completion={} confidence={:?} independence={:?} contract={}@{} blockers={} next={}",
                cell.id,
                cell.score,
                cell.priority,
                cell.maturity,
                cell.completion,
                cell.confidence,
                cell.independence,
                cell.contract_id,
                cell.contract_version,
                cell.blocker_count,
                cell.next_gate
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Summary,
    Validate,
    Weakest,
    Mature,
    Standalone,
    Developing,
    Deferred,
    Help,
}

struct Options {
    command: Command,
    catalog: PathBuf,
    json: bool,
    limit: usize,
    architecture: Option<String>,
    module: Option<String>,
    feature: Option<String>,
    lifecycle: Option<Lifecycle>,
    maturity: Option<Maturity>,
    priority: Option<Priority>,
    repository_root: PathBuf,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut command = Command::Summary;
        let mut catalog = default_catalog_path();
        let mut json = false;
        let mut limit = 5usize;
        let mut architecture = None;
        let mut module = None;
        let mut feature = None;
        let mut lifecycle = None;
        let mut maturity = None;
        let mut priority = None;
        let mut repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut index = 0usize;
        while index < args.len() {
            match args[index].as_str() {
                "summary" => command = Command::Summary,
                "validate" => command = Command::Validate,
                "weakest" => command = Command::Weakest,
                "mature" => command = Command::Mature,
                "standalone" => command = Command::Standalone,
                "developing" => command = Command::Developing,
                "deferred" => command = Command::Deferred,
                "--json" => json = true,
                "--catalog" => {
                    index += 1;
                    catalog = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--catalog requires a path".to_string())?,
                    );
                }
                "--limit" => {
                    index += 1;
                    limit = args
                        .get(index)
                        .ok_or_else(|| "--limit requires a positive integer".to_string())?
                        .parse()
                        .map_err(|_| "--limit requires a positive integer".to_string())?;
                    if limit == 0 {
                        return Err("--limit requires a positive integer".to_string());
                    }
                }
                "--architecture" => {
                    index += 1;
                    architecture = Some(
                        args.get(index)
                            .ok_or_else(|| "--architecture requires an id".to_string())?
                            .clone(),
                    );
                }
                "--module" => {
                    index += 1;
                    module = Some(
                        args.get(index)
                            .ok_or_else(|| "--module requires an id".to_string())?
                            .clone(),
                    );
                }
                "--feature" => {
                    index += 1;
                    feature = Some(
                        args.get(index)
                            .ok_or_else(|| "--feature requires an id".to_string())?
                            .clone(),
                    );
                }
                "--lifecycle" => {
                    index += 1;
                    lifecycle =
                        Some(parse_lifecycle(args.get(index).ok_or_else(|| {
                            "--lifecycle requires a value".to_string()
                        })?)?);
                }
                "--maturity" => {
                    index += 1;
                    maturity = Some(parse_maturity(
                        args.get(index)
                            .ok_or_else(|| "--maturity requires a value".to_string())?,
                    )?);
                }
                "--priority" => {
                    index += 1;
                    priority = Some(parse_priority(
                        args.get(index)
                            .ok_or_else(|| "--priority requires a value".to_string())?,
                    )?);
                }
                "--root" => {
                    index += 1;
                    repository_root = PathBuf::from(
                        args.get(index)
                            .ok_or_else(|| "--root requires a path".to_string())?,
                    );
                }
                "--help" | "-h" | "help" => command = Command::Help,
                unknown => return Err(format!("unknown status argument '{unknown}'\n{}", usage())),
            }
            index += 1;
        }
        Ok(Self {
            command,
            catalog,
            json,
            limit,
            architecture,
            module,
            feature,
            lifecycle,
            maturity,
            priority,
            repository_root,
        })
    }
}

fn usage() -> &'static str {
    "usage: gewyvern_status [summary|validate|weakest|mature|standalone|developing|deferred] [--json] [--limit N] [--architecture ID] [--module ID] [--feature ID] [--lifecycle current|bridge|target|retired] [--priority critical|active|maintenance|deferred] [--maturity planned|incubating|developing|stabilizing|mature|deprecated|blocked] [--catalog PATH] [--root PATH]"
}

fn parse_lifecycle(value: &str) -> Result<Lifecycle, String> {
    match value {
        "current" => Ok(Lifecycle::Current),
        "bridge" => Ok(Lifecycle::Bridge),
        "target" => Ok(Lifecycle::Target),
        "retired" => Ok(Lifecycle::Retired),
        _ => Err(format!("unknown lifecycle '{value}'")),
    }
}

fn parse_maturity(value: &str) -> Result<Maturity, String> {
    match value {
        "planned" => Ok(Maturity::Planned),
        "incubating" => Ok(Maturity::Incubating),
        "developing" => Ok(Maturity::Developing),
        "stabilizing" => Ok(Maturity::Stabilizing),
        "mature" => Ok(Maturity::Mature),
        "deprecated" => Ok(Maturity::Deprecated),
        "blocked" => Ok(Maturity::Blocked),
        _ => Err(format!("unknown maturity '{value}'")),
    }
}

fn parse_priority(value: &str) -> Result<Priority, String> {
    match value {
        "critical" => Ok(Priority::Critical),
        "active" => Ok(Priority::Active),
        "maintenance" => Ok(Priority::Maintenance),
        "deferred" => Ok(Priority::Deferred),
        _ => Err(format!("unknown priority '{value}'")),
    }
}
