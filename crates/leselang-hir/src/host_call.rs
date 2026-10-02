//! Fixed host signatures for computed arguments. Values never become source code.

use super::*;
use crate::computation::{MAX_SCALAR_STRING_BYTES, ScalarType, ScalarValue};
use leselang_syntax::NamedArgument;

#[derive(Clone, Copy)]
pub(crate) enum ArgumentDomain {
    Filter,
    Runtime,
    Pipeline,
    Target,
    Session,
    Node,
    Direction,
    Selection,
    Count,
    Text,
    OptionalText,
    NodeKind,
    ActionKind,
    Field,
    FormValue,
    InputKind,
    Requirement,
    MaxLength,
}

impl ArgumentDomain {
    pub(crate) fn accepts(self, ty: ScalarType) -> bool {
        ty == ScalarType::String
            || (ty == ScalarType::None
                && matches!(self, Self::Filter | Self::Target | Self::OptionalText))
    }

    pub(crate) fn validate(self, value: &ScalarValue) -> bool {
        if !self.accepts(value.scalar_type()) {
            return false;
        }
        let ScalarValue::String(value) = value else {
            return true;
        };
        if value.len() > MAX_SCALAR_STRING_BYTES {
            return false;
        }
        match self {
            Self::Filter => validate_runtime_filter_value(value),
            Self::Runtime => RuntimeId::new(value.clone()).is_ok(),
            Self::Pipeline => validate_deployment_intent(value, None).is_ok(),
            Self::Target => validate_deployment_intent("deploy", Some(value)).is_ok(),
            Self::Session => validate_debugger_session_id(value).is_ok(),
            Self::Node => validate_ui_node_id(value),
            Self::Direction => matches!(value.as_str(), "next" | "previous" | "first" | "last"),
            Self::Selection => matches!(value.as_str(), "selected" | "unselected"),
            Self::Count => parse_child_count(value).is_some(),
            Self::Text | Self::OptionalText => validate_ui_expected_text(value),
            Self::NodeKind => parse_semantic_node_kind(value).is_some(),
            Self::ActionKind => parse_semantic_action_kind(value).is_some(),
            Self::Field => validate_ui_form_field_key(value),
            Self::FormValue => validate_ui_form_value(value),
            Self::InputKind => parse_form_input_kind(value).is_some(),
            Self::Requirement => parse_form_requirement_state(value).is_some(),
            Self::MaxLength => parse_form_max_length(value).is_some(),
        }
    }
}

pub(crate) struct Parameter {
    pub name: &'static str,
    pub domain: ArgumentDomain,
    pub required: bool,
}

const fn required(name: &'static str, domain: ArgumentDomain) -> Parameter {
    Parameter {
        name,
        domain,
        required: true,
    }
}

const fn optional(name: &'static str, domain: ArgumentDomain) -> Parameter {
    Parameter {
        name,
        domain,
        required: false,
    }
}

use ArgumentDomain::*;

const FILTERS: &[Parameter] = &[
    optional("environment", Filter),
    optional("cluster", Filter),
    optional("role", Filter),
];
const RUNTIME: &[Parameter] = &[required("runtime_id", Runtime)];
const DEPLOY: &[Parameter] = &[
    required("runtime_id", Runtime),
    required("pipeline_kind", Pipeline),
    optional("target", Target),
];
const SESSION: &[Parameter] = &[required("session_id", Session)];
const NODE: &[Parameter] = &[required("node_id", Node)];
const DIRECTION: &[Parameter] = &[required("node_id", Node), required("direction", Direction)];
const SELECTION: &[Parameter] = &[required("node_id", Node), required("state", Selection)];
const COUNT: &[Parameter] = &[required("node_id", Node), required("count", Count)];
const TEXT: &[Parameter] = &[required("node_id", Node), required("expected", Text)];
const OPTIONAL_TEXT: &[Parameter] = &[
    required("node_id", Node),
    required("expected", OptionalText),
];
const AUTOMATION_ID: &[Parameter] = &[required("node_id", Node), required("expected", Node)];
const NODE_KIND: &[Parameter] = &[required("node_id", Node), required("kind", NodeKind)];
const ACTION_KIND: &[Parameter] = &[required("node_id", Node), required("kind", ActionKind)];
const SET_FORM_VALUE: &[Parameter] = &[
    required("node_id", Node),
    required("field", Field),
    required("value", FormValue),
];
const FORM_VALUE: &[Parameter] = &[
    required("node_id", Node),
    required("field", Field),
    required("expected", FormValue),
];
const FORM_FIELD: &[Parameter] = &[
    required("node_id", Node),
    required("field", Field),
    required("expected", Text),
];
const INPUT_KIND: &[Parameter] = &[
    required("node_id", Node),
    required("field", Field),
    required("kind", InputKind),
];
const REQUIREMENT: &[Parameter] = &[
    required("node_id", Node),
    required("field", Field),
    required("state", Requirement),
];
const MAX_LENGTH: &[Parameter] = &[
    required("node_id", Node),
    required("field", Field),
    required("max_length", MaxLength),
];
const PLACEHOLDER: &[Parameter] = &[
    required("node_id", Node),
    required("field", Field),
    required("expected", OptionalText),
];

macro_rules! operations {
    ($($variant:ident => ($name:literal, $capability:ident, $parameters:ident)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        pub enum HostOperation {
            $(#[serde(rename = $name)] $variant),+
        }

        impl HostOperation {
            pub fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),+ }
            }

            pub fn result_type(self) -> Type {
                match self { $(Self::$variant => Type::$variant),+ }
            }

            pub fn required_capability(self) -> &'static str {
                match self { $(Self::$variant => $capability),+ }
            }

            pub(crate) fn parse(name: &str) -> Option<Self> {
                match name { $($name => Some(Self::$variant)),+, _ => None }
            }

            pub(crate) fn parameters(self) -> &'static [Parameter] {
                match self { $(Self::$variant => $parameters),+ }
            }

            pub fn for_effect(effect: &Effect) -> Option<Self> {
                match effect {
                    $(Effect::$variant { .. } => Some(Self::$variant)),+,
                    Effect::Compute { .. } | Effect::All { .. } | Effect::Sequence { .. } => None,
                }
            }
        }

        #[cfg(test)]
        const OPERATIONS: &[HostOperation] = &[$(HostOperation::$variant),+];
    };
}

operations! {
    RuntimeList => ("runtime.list", CAPABILITY_RUNTIME_READ, FILTERS),
    RuntimeInspect => ("runtime.inspect", CAPABILITY_RUNTIME_READ, RUNTIME),
    RuntimeHistory => ("runtime.history", CAPABILITY_RUNTIME_READ, RUNTIME),
    RuntimeLogs => ("runtime.logs", CAPABILITY_RUNTIME_READ, RUNTIME),
    RuntimeRefresh => ("runtime.refresh", CAPABILITY_RUNTIME_REFRESH, RUNTIME),
    RuntimeCapabilitiesRefresh => ("runtime.refresh_capabilities", CAPABILITY_RUNTIME_REFRESH, RUNTIME),
    RuntimeDeploy => ("runtime.deploy", CAPABILITY_RUNTIME_DEPLOY, DEPLOY),
    DebuggerCancel => ("debugger.cancel", CAPABILITY_DEBUGGER_CONTROL, SESSION),
    UiActivate => ("ui.activate", CAPABILITY_UI_PRESENTATION, NODE),
    UiFocus => ("ui.focus", CAPABILITY_UI_PRESENTATION, NODE),
    UiNavigateFocus => ("ui.navigate_focus", CAPABILITY_UI_PRESENTATION, DIRECTION),
    UiScrollIntoView => ("ui.scroll_into_view", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertVisible => ("ui.assert_visible", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertHidden => ("ui.assert_hidden", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitHidden => ("ui.wait_hidden", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertRealized => ("ui.assert_realized", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitRealized => ("ui.wait_realized", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitVisible => ("ui.wait_visible", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitEnabled => ("ui.wait_enabled", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitDisabled => ("ui.wait_disabled", CAPABILITY_UI_PRESENTATION, NODE),
    UiOpenWindow => ("ui.open_window", CAPABILITY_UI_PRESENTATION, NODE),
    UiCloseWindow => ("ui.close_window", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertWindowOpen => ("ui.assert_window_open", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitWindowOpen => ("ui.wait_window_open", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertWindowClosed => ("ui.assert_window_closed", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitWindowClosed => ("ui.wait_window_closed", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitFocused => ("ui.wait_focused", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertFocused => ("ui.assert_focused", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitUnfocused => ("ui.wait_unfocused", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertUnfocused => ("ui.assert_unfocused", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertEnabled => ("ui.assert_enabled", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertDisabled => ("ui.assert_disabled", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertChildCount => ("ui.assert_child_count", CAPABILITY_UI_PRESENTATION, COUNT),
    UiWaitChildCount => ("ui.wait_child_count", CAPABILITY_UI_PRESENTATION, COUNT),
    UiSetSelection => ("ui.set_selection", CAPABILITY_UI_PRESENTATION, SELECTION),
    UiAssertSelection => ("ui.assert_selection", CAPABILITY_UI_PRESENTATION, SELECTION),
    UiWaitSelection => ("ui.wait_selection", CAPABILITY_UI_PRESENTATION, SELECTION),
    UiAssertText => ("ui.assert_text", CAPABILITY_UI_PRESENTATION, TEXT),
    UiWaitText => ("ui.wait_text", CAPABILITY_UI_PRESENTATION, TEXT),
    UiAssertAutomationId => ("ui.assert_automation_id", CAPABILITY_UI_PRESENTATION, AUTOMATION_ID),
    UiWaitAutomationId => ("ui.wait_automation_id", CAPABILITY_UI_PRESENTATION, AUTOMATION_ID),
    UiAssertNodeKind => ("ui.assert_node_kind", CAPABILITY_UI_PRESENTATION, NODE_KIND),
    UiWaitNodeKind => ("ui.wait_node_kind", CAPABILITY_UI_PRESENTATION, NODE_KIND),
    UiAssertActionKind => ("ui.assert_action_kind", CAPABILITY_UI_PRESENTATION, ACTION_KIND),
    UiWaitActionKind => ("ui.wait_action_kind", CAPABILITY_UI_PRESENTATION, ACTION_KIND),
    UiAssertActionLabel => ("ui.assert_action_label", CAPABILITY_UI_PRESENTATION, TEXT),
    UiWaitActionLabel => ("ui.wait_action_label", CAPABILITY_UI_PRESENTATION, TEXT),
    UiAssertActionAvailable => ("ui.assert_action_available", CAPABILITY_UI_PRESENTATION, NODE),
    UiWaitActionAvailable => ("ui.wait_action_available", CAPABILITY_UI_PRESENTATION, NODE),
    UiAssertActionUnavailableReason => ("ui.assert_action_unavailable_reason", CAPABILITY_UI_PRESENTATION, OPTIONAL_TEXT),
    UiWaitActionUnavailableReason => ("ui.wait_action_unavailable_reason", CAPABILITY_UI_PRESENTATION, OPTIONAL_TEXT),
    UiSubmitForm => ("ui.submit_form", CAPABILITY_UI_PRESENTATION, NODE),
    UiCancelForm => ("ui.cancel_form", CAPABILITY_UI_PRESENTATION, NODE),
    UiSetFormValue => ("ui.set_form_value", CAPABILITY_UI_PRESENTATION, SET_FORM_VALUE),
    UiAssertFormValue => ("ui.assert_form_value", CAPABILITY_UI_PRESENTATION, FORM_VALUE),
    UiWaitFormValue => ("ui.wait_form_value", CAPABILITY_UI_PRESENTATION, FORM_VALUE),
    UiAssertFormField => ("ui.assert_form_field", CAPABILITY_UI_PRESENTATION, FORM_FIELD),
    UiWaitFormField => ("ui.wait_form_field", CAPABILITY_UI_PRESENTATION, FORM_FIELD),
    UiAssertFormFieldInputKind => ("ui.assert_form_field_input_kind", CAPABILITY_UI_PRESENTATION, INPUT_KIND),
    UiWaitFormFieldInputKind => ("ui.wait_form_field_input_kind", CAPABILITY_UI_PRESENTATION, INPUT_KIND),
    UiAssertFormFieldRequired => ("ui.assert_form_field_required", CAPABILITY_UI_PRESENTATION, REQUIREMENT),
    UiWaitFormFieldRequired => ("ui.wait_form_field_required", CAPABILITY_UI_PRESENTATION, REQUIREMENT),
    UiAssertFormFieldMaxLength => ("ui.assert_form_field_max_length", CAPABILITY_UI_PRESENTATION, MAX_LENGTH),
    UiWaitFormFieldMaxLength => ("ui.wait_form_field_max_length", CAPABILITY_UI_PRESENTATION, MAX_LENGTH),
    UiAssertFormFieldPlaceholder => ("ui.assert_form_field_placeholder", CAPABILITY_UI_PRESENTATION, PLACEHOLDER),
    UiWaitFormFieldPlaceholder => ("ui.wait_form_field_placeholder", CAPABILITY_UI_PRESENTATION, PLACEHOLDER),
    UiAssertAccessibleName => ("ui.assert_accessible_name", CAPABILITY_UI_PRESENTATION, TEXT),
    UiWaitAccessibleName => ("ui.wait_accessible_name", CAPABILITY_UI_PRESENTATION, TEXT),
    UiAssertAccessibleDescription => ("ui.assert_accessible_description", CAPABILITY_UI_PRESENTATION, TEXT),
    UiWaitAccessibleDescription => ("ui.wait_accessible_description", CAPABILITY_UI_PRESENTATION, TEXT),
}

pub(crate) fn invalid_argument(message: impl Into<String>, span: Option<Span>) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: "LSH1407".into(),
        message: message.into(),
        span,
    }]
}

impl HostOperation {
    pub(crate) fn validate_names(
        self,
        names: &[&str],
        span: Option<Span>,
    ) -> Result<(), Vec<Diagnostic>> {
        let parameters = self.parameters();
        if names.len() > parameters.len()
            || names.iter().enumerate().any(|(index, name)| {
                names[..index].contains(name)
                    || !parameters.iter().any(|parameter| parameter.name == *name)
            })
            || parameters
                .iter()
                .any(|parameter| parameter.required && !names.contains(&parameter.name))
        {
            return Err(invalid_argument(
                format!("invalid named arguments for {}", self.name()),
                span,
            ));
        }
        Ok(())
    }

    /// Resolve data into an existing atomic effect, using the literal path's domain validators.
    /// No parsing, execution, effect identifiers, or journal writes occur here.
    pub fn resolve(self, arguments: &[(String, ScalarValue)]) -> Result<Effect, Vec<Diagnostic>> {
        if arguments.len() > self.parameters().len() {
            return Err(invalid_argument("too many host arguments", None));
        }
        self.validate_names(
            &arguments
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            None,
        )?;
        let span = Span { start: 0, end: 0 };
        let mut literals = Vec::with_capacity(arguments.len());
        for (name, value) in arguments {
            let value = match value {
                ScalarValue::String(value) if value.len() <= MAX_SCALAR_STRING_BYTES => {
                    Expression::String {
                        value: value.clone(),
                        span,
                    }
                }
                ScalarValue::None => Expression::None { span },
                _ => {
                    return Err(invalid_argument(
                        "host argument requires a bounded string or none",
                        None,
                    ));
                }
            };
            literals.push(NamedArgument {
                name: name.clone(),
                value,
                span,
            });
        }
        let lowered = lower_atomic_effect(self.name(), &literals, span)?;
        Ok(lowered.effect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(domain: ArgumentDomain) -> ScalarValue {
        ScalarValue::String(
            match domain {
                Filter => "production",
                Runtime => "runtime-a",
                Pipeline => "run",
                Target => "/tmp/demo.gewy",
                Session => "session-a",
                Node => "runtime-a",
                Direction => "next",
                Selection => "selected",
                Count => "1",
                Text | OptionalText => "Ready",
                NodeKind => "runtime_card",
                ActionKind => "runtime_inspect",
                Field => "target",
                FormValue => "user text",
                InputKind => "trimmed_text",
                Requirement => "required",
                MaxLength => "64",
            }
            .into(),
        )
    }

    fn arguments(operation: HostOperation) -> Vec<(String, ScalarValue)> {
        operation
            .parameters()
            .iter()
            .map(|parameter| (parameter.name.into(), sample(parameter.domain)))
            .collect()
    }

    fn literal(value: &ScalarValue) -> String {
        crate::computation::source(&crate::computation::Computation::Literal {
            value: value.clone(),
        })
    }

    #[test]
    fn every_host_signature_matches_literal_and_computed_lowering() {
        assert_eq!(OPERATIONS.len(), 70);
        for &operation in OPERATIONS {
            let values = arguments(operation);
            let source_args = values
                .iter()
                .map(|(name, value)| format!("{name}: {}", literal(value)))
                .collect::<Vec<_>>()
                .join(", ");
            let original = lower(&parse(&format!(
                "fn main() = {}({source_args})",
                operation.name()
            )))
            .unwrap();
            assert_eq!(
                original.function.effect,
                operation.resolve(&values).unwrap()
            );
            assert_eq!(original.function.result_type, operation.result_type());
            assert_eq!(
                original.function.required_capabilities,
                [operation.required_capability()]
            );
            for (index, (_, value)) in values.iter().enumerate() {
                let source_args = values
                    .iter()
                    .enumerate()
                    .map(|(position, (name, value))| {
                        format!(
                            "{name}: {}",
                            if position == index {
                                "argument".into()
                            } else {
                                literal(value)
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let program = lower(&parse(&format!(
                    "fn main() = bind(argument: {}, body: {}({source_args}))",
                    literal(value),
                    operation.name()
                )))
                .unwrap();
                assert_eq!(program.function.result_type, original.function.result_type);
                assert_eq!(
                    program.function.required_capabilities,
                    original.function.required_capabilities
                );
                authorize(
                    &program,
                    &CapabilitySet::new([operation.required_capability()]),
                )
                .unwrap();
                let canonical = canonical_source(&program.function.effect).unwrap();
                assert_eq!(lower(&parse(&canonical)).unwrap(), program);
            }
        }
    }

    #[test]
    fn computed_literal_domains_agree_with_existing_atomic_validators() {
        let candidates = [
            ScalarValue::None,
            ScalarValue::Boolean(true),
            ScalarValue::Integer(1),
            ScalarValue::String(String::new()),
            ScalarValue::String("valid-token".into()),
            ScalarValue::String("bad\0text".into()),
            ScalarValue::String("bad\ntext".into()),
            ScalarValue::String(" ".into()),
            ScalarValue::String("0".into()),
            ScalarValue::String("257".into()),
            ScalarValue::String("4097".into()),
            ScalarValue::String("x".repeat(128)),
            ScalarValue::String("x".repeat(129)),
            ScalarValue::String("x".repeat(1025)),
            ScalarValue::String("x".repeat(4097)),
        ];
        for &operation in OPERATIONS {
            for (index, parameter) in operation.parameters().iter().enumerate() {
                for candidate in candidates
                    .iter()
                    .chain(std::iter::once(&sample(parameter.domain)))
                {
                    let mut values = arguments(operation);
                    values[index].1 = candidate.clone();
                    assert_eq!(
                        parameter.domain.validate(candidate),
                        operation.resolve(&values).is_ok(),
                        "{} {} {candidate:?}",
                        operation.name(),
                        parameter.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_host_signature_rejects_bad_shapes_and_preserves_optional_presence() {
        for &operation in OPERATIONS {
            let values = arguments(operation);
            let mut extra = values.clone();
            extra.push(("unknown".into(), ScalarValue::None));
            assert!(operation.resolve(&extra).is_err());
            for (index, parameter) in operation.parameters().iter().enumerate() {
                let mut missing = values.clone();
                missing.remove(index);
                assert_eq!(operation.resolve(&missing).is_err(), parameter.required);
                let mut duplicate = values.clone();
                duplicate.push(values[index].clone());
                assert!(operation.resolve(&duplicate).is_err());
                let mut null = values.clone();
                null[index].1 = ScalarValue::None;
                assert_eq!(
                    operation.resolve(&null).is_ok(),
                    parameter.domain.accepts(ScalarType::None)
                );
            }
        }
    }
}
