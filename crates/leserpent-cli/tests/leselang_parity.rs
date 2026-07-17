use leselang_command::{LoweringContext, PlannedOperation, lower_effect};
use leselang_hir::lower;
use leselang_syntax::parse;
use leserpent_cli::{CliCommand, export_leselang, parse_args, request_for};
use leserpent_domain::{
    CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet,
    Command, CommandId, CommandOrigin, Confirmation, IdempotencyKey, Principal,
};
use leserpent_protocol::ProtocolRequest;

#[test]
fn runtime_list_cli_and_leselang_lower_to_the_same_normalized_query() {
    let options = parse_args(
        [
            "runtime",
            "list",
            "--environment",
            "production",
            "--cluster",
            "edge-a",
            "--role",
            "debugger",
        ]
        .into_iter()
        .map(str::to_string),
        Some("/tmp/leserpent.sock".into()),
        Some("operator-a".into()),
    )
    .unwrap();
    let ProtocolRequest::Query(cli_query) = request_for(&options).unwrap().request else {
        panic!("CLI list must produce a query");
    };
    let source = export_leselang(
        &parse_args(
            [
                "runtime",
                "list",
                "--environment",
                "production",
                "--cluster",
                "edge-a",
                "--role",
                "debugger",
                "--export-leselang",
            ]
            .into_iter()
            .map(str::to_string),
            None,
            Some("operator-a".into()),
        )
        .unwrap(),
    )
    .unwrap();
    let program = lower(&parse(&source)).unwrap();
    let plan = lower_effect(
        &program.function.effect,
        &LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision: None,
            command_id: CommandId::new("unused-command").unwrap(),
            idempotency_key: IdempotencyKey::new("unused-effect").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
        },
    )
    .unwrap();
    let PlannedOperation::Query(leselang_query) = plan.operation else {
        panic!("Leselang list must produce a query");
    };
    assert_eq!(cli_query, leselang_query);
}

#[test]
fn exported_refresh_lowers_to_the_same_domain_command() {
    let options = parse_args(
        ["runtime", "refresh", "runtime-a", "--export-leselang"]
            .into_iter()
            .map(str::to_string),
        None,
        Some("operator-a".into()),
    )
    .unwrap();
    let CliCommand::RuntimeRefresh(cli_refresh) = &options.command else {
        panic!("CLI must parse runtime refresh");
    };
    let source = export_leselang(&options).unwrap();
    let program = lower(&parse(&source)).unwrap();
    let plan = lower_effect(
        &program.function.effect,
        &LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            expected_revision: None,
            command_id: CommandId::new("parity-command").unwrap(),
            idempotency_key: IdempotencyKey::new("parity-effect").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
        },
    )
    .unwrap();
    let PlannedOperation::Command(command) = plan.operation else {
        panic!("exported refresh must lower to a command");
    };
    let Command::RuntimeRefresh { runtime_id } = command.command else {
        panic!("refresh export must lower to runtime refresh");
    };
    assert_eq!(runtime_id, cli_refresh.runtime_id);
}

#[test]
fn exported_capability_refresh_lowers_to_the_same_domain_command() {
    let options = parse_args(
        [
            "runtime",
            "refresh-capabilities",
            "runtime-a",
            "--export-leselang",
        ]
        .into_iter()
        .map(str::to_string),
        None,
        Some("operator-a".into()),
    )
    .unwrap();
    let CliCommand::RuntimeCapabilitiesRefresh(cli_refresh) = &options.command else {
        panic!("CLI must parse runtime capability refresh");
    };
    let source = export_leselang(&options).unwrap();
    let program = lower(&parse(&source)).unwrap();
    let plan = lower_effect(
        &program.function.effect,
        &LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            expected_revision: None,
            command_id: CommandId::new("parity-command").unwrap(),
            idempotency_key: IdempotencyKey::new("parity-effect").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
        },
    )
    .unwrap();
    let PlannedOperation::Command(command) = plan.operation else {
        panic!("exported capability refresh must lower to a command");
    };
    assert!(matches!(
        command.command,
        Command::RuntimeCapabilitiesRefresh { runtime_id }
            if runtime_id == cli_refresh.runtime_id
    ));
}

#[test]
fn exported_deployment_lowers_to_the_same_confirmed_domain_command() {
    let options = parse_args(
        [
            "runtime",
            "deploy",
            "runtime-a",
            "--pipeline-kind",
            "http/request",
            "--target",
            "pid:42",
            "--export-leselang",
        ]
        .into_iter()
        .map(str::to_string),
        None,
        Some("operator-a".into()),
    )
    .unwrap();
    let CliCommand::RuntimeDeploy(cli_deploy) = &options.command else {
        panic!("CLI must parse runtime deploy");
    };
    let source = export_leselang(&options).unwrap();
    let program = lower(&parse(&source)).unwrap();
    let plan = lower_effect(
        &program.function.effect,
        &LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
            expected_revision: None,
            command_id: CommandId::new("parity-command").unwrap(),
            idempotency_key: IdempotencyKey::new("parity-effect").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
        },
    )
    .unwrap();
    let PlannedOperation::Command(command) = plan.operation else {
        panic!("exported deployment must lower to a command");
    };
    assert_eq!(command.confirmation, Confirmation::Confirmed);
    assert!(matches!(
        command.command,
        Command::RuntimeDeploy {
            runtime_id,
            pipeline_kind,
            target,
        } if runtime_id == cli_deploy.runtime_id
            && pipeline_kind == cli_deploy.pipeline_kind
            && target == cli_deploy.target
    ));
}

#[test]
fn runtime_inspect_cli_and_leselang_lower_to_the_same_query() {
    let options = parse_args(
        ["runtime", "inspect", "runtime-a"]
            .into_iter()
            .map(str::to_string),
        Some("/tmp/leserpent.sock".into()),
        Some("operator-a".into()),
    )
    .unwrap();
    let ProtocolRequest::Query(cli_query) = request_for(&options).unwrap().request else {
        panic!("CLI inspect must produce a query");
    };
    let program = lower(&parse(
        "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
    ))
    .unwrap();
    let plan = lower_effect(
        &program.function.effect,
        &LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision: None,
            command_id: CommandId::new("unused-command").unwrap(),
            idempotency_key: IdempotencyKey::new("unused-effect").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
        },
    )
    .unwrap();
    let PlannedOperation::Query(leselang_query) = plan.operation else {
        panic!("Leselang inspect must produce a query");
    };
    assert_eq!(cli_query, leselang_query);
}

#[test]
fn runtime_history_cli_and_leselang_lower_to_the_same_query() {
    let options = parse_args(
        ["runtime", "history", "runtime-a"]
            .into_iter()
            .map(str::to_string),
        Some("/tmp/leserpent.sock".into()),
        Some("operator-a".into()),
    )
    .unwrap();
    let ProtocolRequest::Query(cli_query) = request_for(&options).unwrap().request else {
        panic!("CLI history must produce a query");
    };
    let program = lower(&parse(
        "fn main() = runtime.history(runtime_id: \"runtime-a\")",
    ))
    .unwrap();
    let plan = lower_effect(
        &program.function.effect,
        &LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision: None,
            command_id: CommandId::new("unused-command").unwrap(),
            idempotency_key: IdempotencyKey::new("unused-effect").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
        },
    )
    .unwrap();
    let PlannedOperation::Query(leselang_query) = plan.operation else {
        panic!("Leselang history must produce a query");
    };
    assert_eq!(cli_query, leselang_query);
}

#[test]
fn runtime_logs_cli_and_leselang_lower_to_the_same_bounded_query() {
    let options = parse_args(
        ["runtime", "logs", "runtime-a"]
            .into_iter()
            .map(str::to_string),
        Some("/tmp/leserpent.sock".into()),
        Some("operator-a".into()),
    )
    .unwrap();
    let ProtocolRequest::Query(cli_query) = request_for(&options).unwrap().request else {
        panic!("CLI logs must produce a query");
    };
    let program = lower(&parse(
        "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
    ))
    .unwrap();
    let plan = lower_effect(
        &program.function.effect,
        &LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision: None,
            command_id: CommandId::new("unused-command").unwrap(),
            idempotency_key: IdempotencyKey::new("unused-effect").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
        },
    )
    .unwrap();
    let PlannedOperation::Query(leselang_query) = plan.operation else {
        panic!("Leselang logs must produce a query");
    };
    assert_eq!(cli_query, leselang_query);
}
