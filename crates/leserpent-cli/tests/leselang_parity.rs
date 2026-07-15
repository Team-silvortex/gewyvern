use leselang_command::{LoweringContext, PlannedOperation, lower_effect};
use leselang_hir::lower;
use leselang_syntax::parse;
use leserpent_cli::{CliCommand, export_leselang, parse_args, request_for};
use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command, CommandId,
    CommandOrigin, Confirmation, IdempotencyKey, Principal,
};
use leserpent_protocol::ProtocolRequest;

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
    let Command::RuntimeRefresh { runtime_id } = command.command;
    assert_eq!(runtime_id, cli_refresh.runtime_id);
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
