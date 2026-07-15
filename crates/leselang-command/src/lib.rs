use leselang_hir::Effect;
use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command, CommandEnvelope,
    CommandId, CommandOrigin, Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey, Principal,
    Query, QueryEnvelope, Revision,
};
pub use leserpent_domain::{COMMAND_PLAN_SCHEMA_VERSION, CommandPlan, PlannedOperation};

pub const MAX_COMMAND_PLAN_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringContext {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub expected_revision: Option<Revision>,
    pub command_id: CommandId,
    pub idempotency_key: IdempotencyKey,
    pub origin: CommandOrigin,
    pub confirmation: Confirmation,
    pub dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoweringError {
    StructuredEffectRequiresExpansion,
    MissingCapability { capability: &'static str },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    UnsupportedSchema { actual: u32, expected: u32 },
    InvalidPlan(String),
}

pub fn encode_plan(plan: &CommandPlan) -> Result<Vec<u8>, PlanCodecError> {
    validate_plan(plan)?;
    let bytes =
        serde_json::to_vec(plan).map_err(|error| PlanCodecError::InvalidJson(error.to_string()))?;
    if bytes.len() > MAX_COMMAND_PLAN_BYTES {
        return Err(PlanCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_COMMAND_PLAN_BYTES,
        });
    }
    Ok(bytes)
}

pub fn decode_plan(bytes: &[u8]) -> Result<CommandPlan, PlanCodecError> {
    if bytes.len() > MAX_COMMAND_PLAN_BYTES {
        return Err(PlanCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_COMMAND_PLAN_BYTES,
        });
    }
    let plan: CommandPlan = serde_json::from_slice(bytes)
        .map_err(|error| PlanCodecError::InvalidJson(error.to_string()))?;
    if plan.schema_version != COMMAND_PLAN_SCHEMA_VERSION {
        return Err(PlanCodecError::UnsupportedSchema {
            actual: plan.schema_version,
            expected: COMMAND_PLAN_SCHEMA_VERSION,
        });
    }
    validate_plan(&plan)?;
    Ok(plan)
}

pub fn lower_effect(
    effect: &Effect,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    let required_capability = match effect {
        Effect::RuntimeList { .. } => CAPABILITY_RUNTIME_READ,
        Effect::RuntimeRefresh { .. } => CAPABILITY_RUNTIME_REFRESH,
        Effect::All { .. } => return Err(LoweringError::StructuredEffectRequiresExpansion),
    };
    if !context.capabilities.contains(required_capability) {
        return Err(LoweringError::MissingCapability {
            capability: required_capability,
        });
    }
    let plan = match effect {
        Effect::RuntimeList { filter } => CommandPlan {
            schema_version: COMMAND_PLAN_SCHEMA_VERSION,
            required_capability: CAPABILITY_RUNTIME_READ.to_string(),
            operation: PlannedOperation::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: context.principal.clone(),
                capabilities: context.capabilities.clone(),
                query: Query::RuntimeList {
                    filter: filter.clone().normalized(),
                },
            }),
        },
        Effect::RuntimeRefresh { runtime_id } => CommandPlan {
            schema_version: COMMAND_PLAN_SCHEMA_VERSION,
            required_capability: CAPABILITY_RUNTIME_REFRESH.to_string(),
            operation: PlannedOperation::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: context.command_id.clone(),
                idempotency_key: context.idempotency_key.clone(),
                expected_revision: context.expected_revision,
                principal: context.principal.clone(),
                capabilities: context.capabilities.clone(),
                origin: context.origin,
                confirmation: context.confirmation,
                dry_run: context.dry_run,
                command: Command::RuntimeRefresh {
                    runtime_id: runtime_id.clone(),
                },
            }),
        },
        Effect::All { .. } => unreachable!("structured effects returned before lowering"),
    };
    Ok(plan)
}

fn validate_plan(plan: &CommandPlan) -> Result<(), PlanCodecError> {
    plan.validate()
        .map_err(|error| PlanCodecError::InvalidPlan(error.to_string()))
}

#[cfg(test)]
mod tests {
    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leserpent_domain::{RuntimeId, RuntimeListFilter};

    use super::*;

    fn context(origin: CommandOrigin) -> LoweringContext {
        LoweringContext {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH]),
            expected_revision: Some(Revision(7)),
            command_id: CommandId::new("command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("effect-a").unwrap(),
            origin,
            confirmation: Confirmation::Confirmed,
            dry_run: true,
        }
    }

    #[test]
    fn runtime_list_lowers_to_normalized_query_plan() {
        let program = lower(&parse(
            "fn main() = runtime.list(environment: \" production \", role: none)",
        ))
        .unwrap();
        let plan =
            lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)).unwrap();

        assert_eq!(plan.required_capability, CAPABILITY_RUNTIME_READ);
        assert!(matches!(
            plan.operation,
            PlannedOperation::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                query: Query::RuntimeList {
                    filter: RuntimeListFilter {
                        environment: Some(ref environment),
                        cluster: None,
                        role: None,
                    },
                },
                ..
            }) if environment == "production"
        ));
    }

    #[test]
    fn runtime_refresh_lowers_to_frontend_neutral_command_semantics() {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let leselang =
            lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)).unwrap();
        let cli = lower_effect(&program.function.effect, &context(CommandOrigin::Cli)).unwrap();

        let (PlannedOperation::Command(mut leselang), PlannedOperation::Command(cli)) =
            (leselang.operation, cli.operation)
        else {
            panic!("runtime.refresh must produce commands");
        };
        assert_eq!(leselang.origin, CommandOrigin::Leselang);
        assert_eq!(cli.origin, CommandOrigin::Cli);
        leselang.origin = CommandOrigin::Cli;
        assert_eq!(leselang, cli);
        assert!(matches!(
            cli.command,
            Command::RuntimeRefresh { runtime_id } if runtime_id == RuntimeId::new("runtime-a").unwrap()
        ));
    }

    #[test]
    fn structured_effect_requires_vm_branch_expansion() {
        let program = lower(&parse(
            "fn main() = all(read: runtime.list(), refresh: runtime.refresh(runtime_id: \"runtime-a\"))",
        ))
        .unwrap();
        assert_eq!(
            lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)),
            Err(LoweringError::StructuredEffectRequiresExpansion)
        );
    }

    #[test]
    fn command_plan_codec_round_trips_and_rejects_unknown_or_oversized_input() {
        let program = lower(&parse("fn main() = runtime.list(role: \"edge\")")).unwrap();
        let plan = lower_effect(&program.function.effect, &context(CommandOrigin::Cli)).unwrap();
        let bytes = encode_plan(&plan).unwrap();
        assert_eq!(decode_plan(&bytes).unwrap(), plan);

        let mut unknown = serde_json::to_value(&plan).unwrap();
        unknown["schema_version"] = serde_json::json!(COMMAND_PLAN_SCHEMA_VERSION + 1);
        let unknown = serde_json::to_vec(&unknown).unwrap();
        assert_eq!(
            decode_plan(&unknown),
            Err(PlanCodecError::UnsupportedSchema {
                actual: COMMAND_PLAN_SCHEMA_VERSION + 1,
                expected: COMMAND_PLAN_SCHEMA_VERSION,
            })
        );

        let oversized = vec![b' '; MAX_COMMAND_PLAN_BYTES + 1];
        assert_eq!(
            decode_plan(&oversized),
            Err(PlanCodecError::Oversized {
                size: MAX_COMMAND_PLAN_BYTES + 1,
                limit: MAX_COMMAND_PLAN_BYTES,
            })
        );

        let mut forged = plan.clone();
        forged.required_capability = CAPABILITY_RUNTIME_REFRESH.to_string();
        assert_eq!(
            encode_plan(&forged),
            Err(PlanCodecError::InvalidPlan(
                "operation requires capability 'runtime.read'".into()
            ))
        );
    }

    #[test]
    fn lowering_fails_closed_when_context_lacks_effect_capability() {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut context = context(CommandOrigin::Leselang);
        context.capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_READ]);
        assert_eq!(
            lower_effect(&program.function.effect, &context),
            Err(LoweringError::MissingCapability {
                capability: CAPABILITY_RUNTIME_REFRESH,
            })
        );
    }
}
