use leselang_command::{LoweringContext, PlannedOperation, lower_effect};
use leselang_hir::lower;
use leselang_syntax::parse;
use leserpent_cli::{CliCommand, export_leselang, parse_args};
use leserpent_domain::{
    CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command, CommandId, CommandOrigin, Confirmation,
    IdempotencyKey, Principal,
};

#[test]
fn exported_refresh_lowers_to_the_same_domain_command() {
    let options = parse_args(
        [
            "runtime",
            "refresh",
            "runtime-a",
            "--export-leselang",
        ]
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
