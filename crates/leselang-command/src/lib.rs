use leselang_hir::Effect;
use leserpent_domain::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command, CommandEnvelope, CommandId, CommandOrigin,
    Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey, MAX_RUNTIME_LOG_QUERY_ENTRIES, Principal,
    Query, QueryEnvelope, Revision, RuntimeId, RuntimeListFilter,
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
    FrontendLocalEffect,
    MissingCapability { capability: &'static str },
    InvalidInput { field: &'static str },
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
    if matches!(
        effect,
        Effect::UiActivate { .. }
            | Effect::UiFocus { .. }
            | Effect::UiNavigateFocus { .. }
            | Effect::UiScrollIntoView { .. }
            | Effect::UiAssertVisible { .. }
            | Effect::UiAssertHidden { .. }
            | Effect::UiWaitHidden { .. }
            | Effect::UiAssertRealized { .. }
            | Effect::UiWaitRealized { .. }
            | Effect::UiWaitVisible { .. }
            | Effect::UiWaitEnabled { .. }
            | Effect::UiWaitDisabled { .. }
            | Effect::UiOpenWindow { .. }
            | Effect::UiCloseWindow { .. }
            | Effect::UiAssertWindowOpen { .. }
            | Effect::UiWaitWindowOpen { .. }
            | Effect::UiAssertWindowClosed { .. }
            | Effect::UiWaitWindowClosed { .. }
            | Effect::UiWaitFocused { .. }
            | Effect::UiAssertFocused { .. }
            | Effect::UiWaitUnfocused { .. }
            | Effect::UiAssertUnfocused { .. }
            | Effect::UiAssertEnabled { .. }
            | Effect::UiAssertDisabled { .. }
            | Effect::UiAssertChildCount { .. }
            | Effect::UiWaitChildCount { .. }
            | Effect::UiSetSelection { .. }
            | Effect::UiAssertSelection { .. }
            | Effect::UiWaitSelection { .. }
            | Effect::UiAssertText { .. }
            | Effect::UiWaitText { .. }
            | Effect::UiAssertAutomationId { .. }
            | Effect::UiAssertNodeKind { .. }
            | Effect::UiWaitNodeKind { .. }
            | Effect::UiAssertActionKind { .. }
            | Effect::UiWaitActionKind { .. }
            | Effect::UiAssertActionLabel { .. }
            | Effect::UiWaitActionLabel { .. }
            | Effect::UiAssertActionAvailable { .. }
            | Effect::UiWaitActionAvailable { .. }
            | Effect::UiAssertActionUnavailableReason { .. }
            | Effect::UiWaitActionUnavailableReason { .. }
            | Effect::UiSubmitForm { .. }
            | Effect::UiCancelForm { .. }
            | Effect::UiSetFormValue { .. }
            | Effect::UiAssertFormValue { .. }
            | Effect::UiWaitFormValue { .. }
            | Effect::UiAssertFormField { .. }
            | Effect::UiAssertFormFieldInputKind { .. }
            | Effect::UiAssertFormFieldRequired { .. }
            | Effect::UiAssertFormFieldMaxLength { .. }
            | Effect::UiAssertFormFieldPlaceholder { .. }
            | Effect::UiWaitFormField { .. }
            | Effect::UiWaitFormFieldInputKind { .. }
            | Effect::UiWaitFormFieldRequired { .. }
            | Effect::UiWaitFormFieldMaxLength { .. }
            | Effect::UiWaitFormFieldPlaceholder { .. }
            | Effect::UiAssertAccessibleName { .. }
            | Effect::UiWaitAccessibleName { .. }
            | Effect::UiAssertAccessibleDescription { .. }
            | Effect::UiWaitAccessibleDescription { .. }
    ) {
        return Err(LoweringError::FrontendLocalEffect);
    }
    let required_capability = match effect {
        Effect::RuntimeList { .. }
        | Effect::RuntimeInspect { .. }
        | Effect::RuntimeHistory { .. }
        | Effect::RuntimeLogs { .. } => CAPABILITY_RUNTIME_READ,
        Effect::RuntimeRefresh { .. } | Effect::RuntimeCapabilitiesRefresh { .. } => {
            CAPABILITY_RUNTIME_REFRESH
        }
        Effect::RuntimeDeploy { .. } => CAPABILITY_RUNTIME_DEPLOY,
        Effect::DebuggerCancel { .. } => CAPABILITY_DEBUGGER_CONTROL,
        Effect::UiActivate { .. }
        | Effect::UiFocus { .. }
        | Effect::UiNavigateFocus { .. }
        | Effect::UiScrollIntoView { .. }
        | Effect::UiAssertVisible { .. }
        | Effect::UiAssertHidden { .. }
        | Effect::UiWaitHidden { .. }
        | Effect::UiAssertRealized { .. }
        | Effect::UiWaitRealized { .. }
        | Effect::UiWaitVisible { .. }
        | Effect::UiWaitEnabled { .. }
        | Effect::UiWaitDisabled { .. }
        | Effect::UiOpenWindow { .. }
        | Effect::UiCloseWindow { .. }
        | Effect::UiAssertWindowOpen { .. }
        | Effect::UiWaitWindowOpen { .. }
        | Effect::UiAssertWindowClosed { .. }
        | Effect::UiWaitWindowClosed { .. }
        | Effect::UiWaitFocused { .. }
        | Effect::UiAssertFocused { .. }
        | Effect::UiWaitUnfocused { .. }
        | Effect::UiAssertUnfocused { .. }
        | Effect::UiAssertEnabled { .. }
        | Effect::UiAssertDisabled { .. }
        | Effect::UiAssertChildCount { .. }
        | Effect::UiWaitChildCount { .. }
        | Effect::UiSetSelection { .. }
        | Effect::UiAssertSelection { .. }
        | Effect::UiWaitSelection { .. }
        | Effect::UiAssertText { .. }
        | Effect::UiWaitText { .. }
        | Effect::UiAssertAutomationId { .. }
        | Effect::UiAssertNodeKind { .. }
        | Effect::UiWaitNodeKind { .. }
        | Effect::UiAssertActionKind { .. }
        | Effect::UiWaitActionKind { .. }
        | Effect::UiAssertActionLabel { .. }
        | Effect::UiWaitActionLabel { .. }
        | Effect::UiAssertActionAvailable { .. }
        | Effect::UiWaitActionAvailable { .. }
        | Effect::UiAssertActionUnavailableReason { .. }
        | Effect::UiWaitActionUnavailableReason { .. }
        | Effect::UiSubmitForm { .. }
        | Effect::UiCancelForm { .. }
        | Effect::UiSetFormValue { .. }
        | Effect::UiAssertFormValue { .. }
        | Effect::UiWaitFormValue { .. }
        | Effect::UiAssertFormField { .. }
        | Effect::UiAssertFormFieldInputKind { .. }
        | Effect::UiAssertFormFieldRequired { .. }
        | Effect::UiAssertFormFieldMaxLength { .. }
        | Effect::UiAssertFormFieldPlaceholder { .. }
        | Effect::UiWaitFormField { .. }
        | Effect::UiWaitFormFieldInputKind { .. }
        | Effect::UiWaitFormFieldRequired { .. }
        | Effect::UiWaitFormFieldMaxLength { .. }
        | Effect::UiWaitFormFieldPlaceholder { .. }
        | Effect::UiAssertAccessibleName { .. }
        | Effect::UiWaitAccessibleName { .. }
        | Effect::UiAssertAccessibleDescription { .. }
        | Effect::UiWaitAccessibleDescription { .. } => {
            unreachable!("frontend-local effects returned before lowering")
        }
        Effect::All { .. } => return Err(LoweringError::StructuredEffectRequiresExpansion),
    };
    if !context.capabilities.contains(required_capability) {
        return Err(LoweringError::MissingCapability {
            capability: required_capability,
        });
    }
    let plan = match effect {
        Effect::RuntimeList { filter } => plan_runtime_list(filter, context)?,
        Effect::RuntimeInspect { runtime_id } => plan_runtime_inspect(runtime_id, context)?,
        Effect::RuntimeHistory { runtime_id } => plan_runtime_history(runtime_id, context)?,
        Effect::RuntimeLogs { runtime_id } => plan_runtime_logs(runtime_id, context)?,
        Effect::RuntimeRefresh { runtime_id } => plan_runtime_refresh(runtime_id, context)?,
        Effect::RuntimeCapabilitiesRefresh { runtime_id } => {
            plan_runtime_capabilities_refresh(runtime_id, context)?
        }
        Effect::RuntimeDeploy {
            runtime_id,
            pipeline_kind,
            target,
        } => plan_runtime_deploy(runtime_id, pipeline_kind, target.as_deref(), context)?,
        Effect::DebuggerCancel { session_id } => plan_debugger_cancel(session_id, context)?,
        Effect::UiActivate { .. }
        | Effect::UiFocus { .. }
        | Effect::UiNavigateFocus { .. }
        | Effect::UiScrollIntoView { .. }
        | Effect::UiAssertVisible { .. }
        | Effect::UiAssertHidden { .. }
        | Effect::UiWaitHidden { .. }
        | Effect::UiAssertRealized { .. }
        | Effect::UiWaitRealized { .. }
        | Effect::UiWaitVisible { .. }
        | Effect::UiWaitEnabled { .. }
        | Effect::UiWaitDisabled { .. }
        | Effect::UiOpenWindow { .. }
        | Effect::UiCloseWindow { .. }
        | Effect::UiAssertWindowOpen { .. }
        | Effect::UiWaitWindowOpen { .. }
        | Effect::UiAssertWindowClosed { .. }
        | Effect::UiWaitWindowClosed { .. }
        | Effect::UiWaitFocused { .. }
        | Effect::UiAssertFocused { .. }
        | Effect::UiWaitUnfocused { .. }
        | Effect::UiAssertUnfocused { .. }
        | Effect::UiAssertEnabled { .. }
        | Effect::UiAssertDisabled { .. }
        | Effect::UiAssertChildCount { .. }
        | Effect::UiWaitChildCount { .. }
        | Effect::UiSetSelection { .. }
        | Effect::UiAssertSelection { .. }
        | Effect::UiWaitSelection { .. }
        | Effect::UiAssertText { .. }
        | Effect::UiWaitText { .. }
        | Effect::UiAssertAutomationId { .. }
        | Effect::UiAssertNodeKind { .. }
        | Effect::UiWaitNodeKind { .. }
        | Effect::UiAssertActionKind { .. }
        | Effect::UiWaitActionKind { .. }
        | Effect::UiAssertActionLabel { .. }
        | Effect::UiWaitActionLabel { .. }
        | Effect::UiAssertActionAvailable { .. }
        | Effect::UiWaitActionAvailable { .. }
        | Effect::UiAssertActionUnavailableReason { .. }
        | Effect::UiWaitActionUnavailableReason { .. }
        | Effect::UiSubmitForm { .. }
        | Effect::UiCancelForm { .. }
        | Effect::UiSetFormValue { .. }
        | Effect::UiAssertFormValue { .. }
        | Effect::UiWaitFormValue { .. }
        | Effect::UiAssertFormField { .. }
        | Effect::UiAssertFormFieldInputKind { .. }
        | Effect::UiAssertFormFieldRequired { .. }
        | Effect::UiAssertFormFieldMaxLength { .. }
        | Effect::UiAssertFormFieldPlaceholder { .. }
        | Effect::UiWaitFormField { .. }
        | Effect::UiWaitFormFieldInputKind { .. }
        | Effect::UiWaitFormFieldRequired { .. }
        | Effect::UiWaitFormFieldMaxLength { .. }
        | Effect::UiWaitFormFieldPlaceholder { .. }
        | Effect::UiAssertAccessibleName { .. }
        | Effect::UiWaitAccessibleName { .. }
        | Effect::UiAssertAccessibleDescription { .. }
        | Effect::UiWaitAccessibleDescription { .. } => {
            unreachable!("frontend-local effects returned before lowering")
        }
        Effect::All { .. } => unreachable!("structured effects returned before lowering"),
    };
    Ok(plan)
}

pub fn plan_runtime_list(
    filter: &RuntimeListFilter,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    plan_runtime_query(
        Query::RuntimeList {
            filter: filter.clone().normalized(),
        },
        context,
    )
}

pub fn plan_runtime_inspect(
    runtime_id: &RuntimeId,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    plan_runtime_query(
        Query::RuntimeInspect {
            runtime_id: runtime_id.clone(),
        },
        context,
    )
}

pub fn plan_runtime_history(
    runtime_id: &RuntimeId,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    plan_runtime_query(
        Query::RuntimeHistory {
            runtime_id: runtime_id.clone(),
        },
        context,
    )
}

pub fn plan_runtime_logs(
    runtime_id: &RuntimeId,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    plan_runtime_query(
        Query::RuntimeLogs {
            runtime_id: runtime_id.clone(),
            after_sequence: None,
            limit: MAX_RUNTIME_LOG_QUERY_ENTRIES,
        },
        context,
    )
}

fn plan_runtime_query(
    query: Query,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    if !context.capabilities.contains(CAPABILITY_RUNTIME_READ) {
        return Err(LoweringError::MissingCapability {
            capability: CAPABILITY_RUNTIME_READ,
        });
    }
    Ok(CommandPlan {
        schema_version: COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: CAPABILITY_RUNTIME_READ.to_string(),
        operation: PlannedOperation::Query(QueryEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            principal: context.principal.clone(),
            capabilities: context.capabilities.clone(),
            query,
        }),
    })
}

pub fn plan_runtime_refresh(
    runtime_id: &RuntimeId,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    if !context.capabilities.contains(CAPABILITY_RUNTIME_REFRESH) {
        return Err(LoweringError::MissingCapability {
            capability: CAPABILITY_RUNTIME_REFRESH,
        });
    }
    Ok(CommandPlan {
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
    })
}

pub fn plan_runtime_capabilities_refresh(
    runtime_id: &RuntimeId,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    if !context.capabilities.contains(CAPABILITY_RUNTIME_REFRESH) {
        return Err(LoweringError::MissingCapability {
            capability: CAPABILITY_RUNTIME_REFRESH,
        });
    }
    Ok(CommandPlan {
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
            command: Command::RuntimeCapabilitiesRefresh {
                runtime_id: runtime_id.clone(),
            },
        }),
    })
}

pub fn plan_runtime_deploy(
    runtime_id: &RuntimeId,
    pipeline_kind: &str,
    target: Option<&str>,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    if !context.capabilities.contains(CAPABILITY_RUNTIME_DEPLOY) {
        return Err(LoweringError::MissingCapability {
            capability: CAPABILITY_RUNTIME_DEPLOY,
        });
    }
    let plan = CommandPlan {
        schema_version: COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: CAPABILITY_RUNTIME_DEPLOY.to_string(),
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
            command: Command::RuntimeDeploy {
                runtime_id: runtime_id.clone(),
                pipeline_kind: pipeline_kind.to_string(),
                target: target.map(str::to_string),
            },
        }),
    };
    plan.validate().map_err(|_| LoweringError::InvalidInput {
        field: "deployment_intent",
    })?;
    Ok(plan)
}

pub fn plan_debugger_cancel(
    session_id: &str,
    context: &LoweringContext,
) -> Result<CommandPlan, LoweringError> {
    if !context.capabilities.contains(CAPABILITY_DEBUGGER_CONTROL) {
        return Err(LoweringError::MissingCapability {
            capability: CAPABILITY_DEBUGGER_CONTROL,
        });
    }
    let plan = CommandPlan {
        schema_version: COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: CAPABILITY_DEBUGGER_CONTROL.to_string(),
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
            command: Command::DebuggerCancel {
                session_id: session_id.to_string(),
            },
        }),
    };
    plan.validate().map_err(|_| LoweringError::InvalidInput {
        field: "session_id",
    })?;
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
    fn frontend_presentation_effect_cannot_become_a_control_plane_plan() {
        for source in [
            "fn main() = ui.activate(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:refresh\", direction: \"next\")",
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_hidden(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_hidden(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_realized(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_realized(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_visible(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_enabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.wait_disabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.open_window(node_id: \"runtime-a:card\")",
            "fn main() = ui.close_window(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_window_open(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_window_open(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_window_closed(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_window_closed(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_focused(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.wait_unfocused(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_unfocused(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_enabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_disabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_child_count(node_id: \"fleet-root\", count: \"3\")",
            "fn main() = ui.wait_child_count(node_id: \"fleet-root\", count: \"4\")",
            "fn main() = ui.assert_selection(node_id: \"runtime-a:card\", state: \"selected\")",
            "fn main() = ui.wait_selection(node_id: \"runtime-a:card\", state: \"unselected\")",
            "fn main() = ui.assert_text(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
            "fn main() = ui.wait_text(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\", expected: \"fleet-title\")",
            "fn main() = ui.assert_node_kind(node_id: \"fleet-title\", kind: \"heading\")",
            "fn main() = ui.wait_node_kind(node_id: \"fleet-title\", kind: \"heading\")",
            "fn main() = ui.assert_action_kind(node_id: \"runtime-a:refresh\", kind: \"runtime_refresh\")",
            "fn main() = ui.wait_action_kind(node_id: \"runtime-a:refresh\", kind: \"runtime_refresh\")",
            "fn main() = ui.assert_action_label(node_id: \"runtime-a:refresh\", expected: \"Refresh runtime\")",
            "fn main() = ui.wait_action_label(node_id: \"runtime-a:refresh\", expected: \"Refresh runtime\")",
            "fn main() = ui.assert_action_available(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.wait_action_available(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"Verification action is temporarily unavailable\")",
            "fn main() = ui.wait_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"Verification action is temporarily unavailable\")",
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"Pipeline kind\")",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: \"path_token\")",
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: \"required\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"128\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"http/request\")",
            "fn main() = ui.wait_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"Pipeline kind\")",
            "fn main() = ui.wait_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: \"path_token\")",
            "fn main() = ui.wait_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: \"required\")",
            "fn main() = ui.wait_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"128\")",
            "fn main() = ui.wait_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"http/request\")",
            "fn main() = ui.assert_accessible_name(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
            "fn main() = ui.wait_accessible_name(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"Open the read-only runtime workspace\")",
            "fn main() = ui.wait_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"Open the read-only runtime workspace\")",
        ] {
            let program = lower(&parse(source)).unwrap();
            assert_eq!(
                lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)),
                Err(LoweringError::FrontendLocalEffect)
            );
        }
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
    fn runtime_capabilities_refresh_uses_the_shared_mutation_contract() {
        let program = lower(&parse(
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let plan =
            lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)).unwrap();

        assert_eq!(plan.required_capability, CAPABILITY_RUNTIME_REFRESH);
        assert!(matches!(
            plan.operation,
            PlannedOperation::Command(CommandEnvelope {
                origin: CommandOrigin::Leselang,
                command: Command::RuntimeCapabilitiesRefresh { runtime_id },
                ..
            }) if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn debugger_cancel_plan_is_capability_gated_and_session_validated() {
        let mut context = context(CommandOrigin::Gui);
        context.capabilities = CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]);
        let plan = plan_debugger_cancel("session-a", &context).unwrap();
        assert_eq!(plan.required_capability, CAPABILITY_DEBUGGER_CONTROL);
        let PlannedOperation::Command(command) = &plan.operation else {
            panic!("debugger cancel must be a command");
        };
        assert_eq!(command.expected_revision, Some(Revision(7)));
        assert!(command.dry_run);
        assert_eq!(command.confirmation, Confirmation::Confirmed);
        assert!(matches!(
            &command.command,
            Command::DebuggerCancel { session_id } if session_id == "session-a"
        ));
        assert_eq!(decode_plan(&encode_plan(&plan).unwrap()).unwrap(), plan);
        assert_eq!(
            plan_debugger_cancel("invalid session", &context),
            Err(LoweringError::InvalidInput {
                field: "session_id",
            })
        );
    }

    #[test]
    fn runtime_inspect_lowers_to_frontend_neutral_query_plan() {
        let program = lower(&parse(
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let plan =
            lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)).unwrap();
        assert_eq!(plan.required_capability, CAPABILITY_RUNTIME_READ);
        assert!(matches!(
            plan.operation,
            PlannedOperation::Query(QueryEnvelope {
                query: Query::RuntimeInspect { runtime_id },
                ..
            }) if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_history_lowers_to_frontend_neutral_query_plan() {
        let program = lower(&parse(
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let plan =
            lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)).unwrap();
        assert_eq!(plan.required_capability, CAPABILITY_RUNTIME_READ);
        assert!(matches!(
            plan.operation,
            PlannedOperation::Query(QueryEnvelope {
                query: Query::RuntimeHistory { runtime_id },
                ..
            }) if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_logs_lowers_to_bounded_frontend_neutral_query_plan() {
        let program = lower(&parse(
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let plan =
            lower_effect(&program.function.effect, &context(CommandOrigin::Leselang)).unwrap();
        assert_eq!(plan.required_capability, CAPABILITY_RUNTIME_READ);
        assert!(matches!(
            plan.operation,
            PlannedOperation::Query(QueryEnvelope {
                query: Query::RuntimeLogs {
                    runtime_id,
                    after_sequence: None,
                    limit: MAX_RUNTIME_LOG_QUERY_ENTRIES,
                },
                ..
            }) if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_deploy_lowers_to_confirmed_typed_command_plan() {
        let program = lower(&parse(
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: none)",
        ))
        .unwrap();
        let mut context = context(CommandOrigin::Leselang);
        context.capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]);
        context.confirmation = Confirmation::Confirmed;
        context.dry_run = false;
        let plan = lower_effect(&program.function.effect, &context).unwrap();
        assert_eq!(plan.required_capability, CAPABILITY_RUNTIME_DEPLOY);
        assert!(matches!(
            plan.operation,
            PlannedOperation::Command(CommandEnvelope {
                confirmation: Confirmation::Confirmed,
                command: Command::RuntimeDeploy {
                    runtime_id,
                    pipeline_kind,
                    target: None,
                },
                ..
            }) if runtime_id.as_str() == "runtime-a" && pipeline_kind == "http/request"
        ));

        context.confirmation = Confirmation::NotRequired;
        context.dry_run = false;
        assert_eq!(
            lower_effect(&program.function.effect, &context),
            Err(LoweringError::InvalidInput {
                field: "deployment_intent",
            })
        );
        context.dry_run = true;
        assert!(lower_effect(&program.function.effect, &context).is_ok());
    }

    #[test]
    fn debugger_cancel_language_effect_uses_the_shared_control_plan() {
        let program = lower(&parse(
            "fn main() = debugger.cancel(session_id: \"session-a\")",
        ))
        .unwrap();
        let mut context = context(CommandOrigin::Leselang);
        context.capabilities = CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]);
        context.confirmation = Confirmation::Confirmed;
        context.dry_run = false;
        let plan = lower_effect(&program.function.effect, &context).unwrap();
        assert_eq!(plan.required_capability, CAPABILITY_DEBUGGER_CONTROL);
        assert!(matches!(
            plan.operation,
            PlannedOperation::Command(CommandEnvelope {
                origin: CommandOrigin::Leselang,
                command: Command::DebuggerCancel { ref session_id },
                ..
            }) if session_id == "session-a"
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
