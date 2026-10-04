//! Fixed host signatures for computed arguments. Values never become source code.

use super::*;
use crate::computation::{MAX_SCALAR_STRING_BYTES, ScalarType, ScalarValue};
use leselang_runtime_core::{
    NamedArgumentBindings, NamedParameter, OperationCatalog, OperationCatalogError,
    OperationCatalogLimits, OperationSchema, ScalarArgumentDomain, ScalarArgumentType,
    ScalarTypeSet, check_argument_type,
};
use leselang_syntax::NamedArgument;
use std::sync::OnceLock;

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
    const fn scalar_types(self) -> ScalarTypeSet {
        let text = ScalarTypeSet::only(ScalarType::String);
        match self {
            Self::Filter | Self::Target => text.with(ScalarType::None),
            Self::OptionalText => text.with(ScalarType::None).with(ScalarType::OptionalString),
            _ => text,
        }
    }

    pub(crate) fn accepts(self, ty: ScalarType) -> bool {
        self.scalar_types().contains(ty)
    }

    pub(crate) fn validate(self, value: &ScalarValue) -> bool {
        check_argument_type(&self, ScalarArgumentType::literal(value)).is_ok()
    }
}

impl ScalarArgumentDomain for ArgumentDomain {
    fn scalar_types(&self) -> ScalarTypeSet {
        (*self).scalar_types()
    }

    fn accepts_literal(&self, value: &ScalarValue) -> bool {
        let Some(value) = value.text() else {
            return true;
        };
        match self {
            Self::Filter => validate_runtime_filter_value(value),
            Self::Runtime => RuntimeId::new(value).is_ok(),
            Self::Pipeline => validate_deployment_intent(value, None).is_ok(),
            Self::Target => validate_deployment_intent("deploy", Some(value)).is_ok(),
            Self::Session => validate_debugger_session_id(value).is_ok(),
            Self::Node => validate_ui_node_id(value),
            Self::Direction => matches!(value, "next" | "previous" | "first" | "last"),
            Self::Selection => matches!(value, "selected" | "unselected"),
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

pub(crate) type Parameter = NamedParameter<&'static str, ArgumentDomain>;

const fn required(name: &'static str, domain: ArgumentDomain) -> Parameter {
    Parameter::required(name, domain)
}

const fn optional(name: &'static str, domain: ArgumentDomain) -> Parameter {
    Parameter::optional(name, domain)
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

pub(crate) struct ReferenceResult {
    operation: HostOperation,
    pub(crate) ty: Type,
}

pub(crate) type ReferenceSchema = OperationSchema<
    'static,
    &'static str,
    &'static str,
    ArgumentDomain,
    ReferenceResult,
    &'static str,
>;
type ReferenceCatalog = OperationCatalog<
    'static,
    &'static str,
    &'static str,
    ArgumentDomain,
    ReferenceResult,
    &'static str,
>;

const REFERENCE_CATALOG_VERSION: u32 = 1;

fn operation_catalog() -> Option<&'static ReferenceCatalog> {
    static CATALOG: OnceLock<Result<ReferenceCatalog, OperationCatalogError>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            OperationCatalog::new(
                REFERENCE_CATALOG_VERSION,
                OPERATION_SCHEMAS,
                OperationCatalogLimits {
                    max_operations: OPERATION_SCHEMAS.len(),
                    max_parameters_per_operation: 3,
                },
            )
        })
        .as_ref()
        .ok()
}

macro_rules! operations {
    ($($variant:ident => ($name:literal, $capability:ident, $parameters:ident)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
        pub enum HostOperation {
            $(#[serde(rename = $name)] $variant),+
        }

        impl HostOperation {
            pub fn name(self) -> &'static str {
                self.schema().key
            }

            pub fn result_type(self) -> Type {
                self.schema().result.ty
            }

            pub fn required_capability(self) -> &'static str {
                self.schema().required_capability
            }

            pub(crate) fn parse(name: &str) -> Option<Self> {
                operation_catalog()?.lookup(name, REFERENCE_CATALOG_VERSION).ok()
                    .map(|schema| schema.result.operation)
            }

            pub(crate) fn parameters(self) -> &'static [Parameter] {
                self.schema().parameters
            }

            pub(crate) fn schema(self) -> &'static ReferenceSchema {
                // Enum/table order is emitted together; this index is never a protocol ID.
                &OPERATION_SCHEMAS[self as usize]
            }

            pub fn for_effect(effect: &Effect) -> Option<Self> {
                match effect {
                    $(Effect::$variant { .. } => Some(Self::$variant)),+,
                    Effect::Compute { .. } | Effect::All { .. } | Effect::Sequence { .. } => None,
                }
            }
        }

        const OPERATION_SCHEMAS: &[ReferenceSchema] = &[
            $(OperationSchema {
                key: $name,
                parameters: $parameters,
                result: ReferenceResult { operation: HostOperation::$variant, ty: Type::$variant },
                required_capability: $capability,
            }),+
        ];

        #[cfg(test)]
        const OPERATIONS: &[HostOperation] = &[$(HostOperation::$variant),+];

        #[cfg(test)]
        const LEGACY_OPERATION_SIGNATURES: &[(HostOperation, &str, Type, &str, &[Parameter])] = &[
            $((HostOperation::$variant, $name, Type::$variant, $capability, $parameters)),+
        ];
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
        self.bind_names(names, span).map(|_| ())
    }

    pub(crate) fn bind_names<'a>(
        self,
        names: &'a [&'a str],
        span: Option<Span>,
    ) -> Result<NamedArgumentBindings<'a, &'a str, ArgumentDomain>, Vec<Diagnostic>> {
        self.schema().bind_arguments(names).map_err(|_| {
            invalid_argument(format!("invalid named arguments for {}", self.name()), span)
        })
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
                ScalarValue::OptionalString(text) => {
                    let parameter = self
                        .parameters()
                        .iter()
                        .find(|parameter| parameter.name == name)
                        .ok_or_else(|| invalid_argument("unknown host argument", None))?;
                    if !parameter.domain.accepts(ScalarType::OptionalString)
                        || !parameter.domain.validate(value)
                    {
                        return Err(invalid_argument(
                            "optional string requires an optional text argument",
                            None,
                        ));
                    }
                    match &text.0 {
                        Some(text) => Expression::String {
                            value: text.clone(),
                            span,
                        },
                        None => Expression::None { span },
                    }
                }
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
    use leselang_runtime_core::validate_named_arguments;

    #[test]
    fn reference_catalog_is_valid_and_every_operation_uses_its_original_schema_row() {
        let catalog = operation_catalog().expect("generated catalog must validate");
        assert_eq!(catalog.version(), REFERENCE_CATALOG_VERSION);
        for operation in OPERATIONS {
            let schema = catalog
                .lookup(operation.name(), REFERENCE_CATALOG_VERSION)
                .unwrap();
            assert!(std::ptr::eq(schema, operation.schema()));
            assert_eq!(schema.result.operation, *operation);
            assert!(
                catalog
                    .authorize(
                        operation.name(),
                        REFERENCE_CATALOG_VERSION,
                        &[operation.required_capability()]
                    )
                    .is_ok()
            );
            assert_eq!(
                catalog
                    .authorize(operation.name(), REFERENCE_CATALOG_VERSION, &[])
                    .unwrap_err(),
                OperationCatalogError::CapabilityDenied
            );
            assert_eq!(
                catalog
                    .lookup(operation.name(), REFERENCE_CATALOG_VERSION + 1)
                    .unwrap_err(),
                OperationCatalogError::UnsupportedVersion
            );
        }
    }

    #[test]
    fn operation_metadata_and_wire_tags_keep_the_legacy_seventy_operation_contract() {
        assert_eq!(LEGACY_OPERATION_SIGNATURES.len(), 70);
        for &(operation, name, result, capability, parameters) in LEGACY_OPERATION_SIGNATURES {
            assert_eq!(operation.name(), name);
            assert_eq!(operation.result_type(), result);
            assert_eq!(operation.required_capability(), capability);
            assert_eq!(operation.parameters().len(), parameters.len());
            for (actual, expected) in operation.parameters().iter().zip(parameters) {
                assert_eq!(actual.name, expected.name);
                assert_eq!(actual.required, expected.required);
                assert_eq!(
                    std::mem::discriminant(&actual.domain),
                    std::mem::discriminant(&expected.domain)
                );
            }
            assert_eq!(HostOperation::parse(name), Some(operation));
            assert_eq!(
                serde_json::to_value(operation).unwrap(),
                serde_json::json!(name)
            );
            assert_eq!(
                serde_json::from_value::<HostOperation>(serde_json::json!(name)).unwrap(),
                operation
            );
            assert_eq!(HostOperation::parse(&name.to_uppercase()), None);
            assert_eq!(HostOperation::parse(&format!("{name} ")), None);
        }
        for unknown in ["", "device.move", "ui.private", "runtime.list.extra"] {
            assert_eq!(HostOperation::parse(unknown), None);
        }
    }

    #[test]
    fn every_host_signature_binds_reordered_names_to_original_parameter_metadata() {
        for operation in OPERATIONS {
            for required_only in [false, true] {
                let names: Vec<_> = operation
                    .parameters()
                    .iter()
                    .rev()
                    .filter(|parameter| !required_only || parameter.required)
                    .map(|parameter| parameter.name)
                    .collect();
                let bindings = operation.bind_names(&names, None).unwrap();
                let expected: Vec<_> = operation
                    .parameters()
                    .iter()
                    .filter_map(|parameter| {
                        names
                            .iter()
                            .position(|name| *name == parameter.name)
                            .map(|index| (parameter, index))
                    })
                    .collect();
                let actual: Vec<_> = bindings.iter().collect();
                assert_eq!(actual.len(), expected.len());
                for ((parameter, index), (original, original_index)) in actual.iter().zip(expected)
                {
                    assert!(std::ptr::eq(*parameter, original));
                    assert_eq!(*index, original_index);
                }
            }
        }
    }

    #[test]
    fn every_host_parameter_keeps_its_closed_legacy_scalar_type_admission() {
        for operation in OPERATIONS {
            for parameter in operation.parameters() {
                for ty in [
                    ScalarType::Integer,
                    ScalarType::Boolean,
                    ScalarType::String,
                    ScalarType::None,
                    ScalarType::OptionalString,
                    ScalarType::StringList,
                ] {
                    let expected = ty == ScalarType::String
                        || (ty == ScalarType::OptionalString
                            && matches!(parameter.domain, OptionalText))
                        || (ty == ScalarType::None
                            && matches!(parameter.domain, Filter | Target | OptionalText));
                    assert_eq!(
                        parameter.domain.accepts(ty),
                        expected,
                        "{} {} {ty:?}",
                        operation.name(),
                        parameter.name
                    );
                    assert_eq!(
                        check_argument_type(
                            &parameter.domain,
                            ScalarArgumentType::expression(Some(ty), true)
                        )
                        .is_ok(),
                        expected,
                        "shared checker: {} {} {ty:?}",
                        operation.name(),
                        parameter.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_host_signature_is_the_core_metadata_type_not_a_conversion_wrapper() {
        for operation in OPERATIONS {
            let schema: &[NamedParameter<&str, ArgumentDomain>] = operation.parameters();
            assert!(std::ptr::eq(
                schema.as_ptr(),
                operation.parameters().as_ptr()
            ));
            validate_named_arguments(
                &schema
                    .iter()
                    .map(|parameter| parameter.name)
                    .collect::<Vec<_>>(),
                schema,
            )
            .unwrap();
        }
    }

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
            operation
                .schema()
                .check_argument_types(
                    &values
                        .iter()
                        .map(|(name, _)| name.as_str())
                        .collect::<Vec<_>>(),
                    &values
                        .iter()
                        .map(|(_, value)| ScalarArgumentType::literal(value))
                        .collect::<Vec<_>>(),
                )
                .unwrap();
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
