use super::*;
use crate::host_call::{HostOperation, invalid_argument};
use crate::result_field::ResultField;
use leselang_runtime_core::{ArgumentTypeError, ScopeFrame, StructureBudget};
use leselang_syntax::NamedArgument;

pub const MAX_COMPUTATION_NODES: usize = 1_024;

pub use leselang_runtime_core::{
    BinaryOperator, MAX_LOOP_ITERATIONS, MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS,
    OptionalStringValue, ScalarType, ScalarValue, StringListValue, UnaryOperator,
};

// Source expansion cost belongs to HIR, not the host-neutral data contract.
pub(crate) fn source_shape_extra(value: &ScalarValue) -> (usize, usize) {
    let extra = crate::source_cost::literal_source_extra(value);
    (extra.nodes, extra.depth)
}

pub use crate::ir::GroupKind;

/// Reference-host specializations of the shared control IR, without conversion.
pub type Computation = crate::ir::Computation<ResultField, HostOperation, Effect, Type>;
pub type ComputedBranch = crate::ir::ComputedBranch<Computation, Type>;
pub type ComputedArgument = crate::ir::ComputedArgument<Computation>;

/// A bounded named group signature supplied by a validated embedding environment.
#[derive(Clone)]
pub struct GroupLocalType {
    pub name: String,
    pub members: Vec<(String, HostOperation)>,
}

pub(super) struct LocalType {
    ty: Type,
    members: Option<Vec<(String, HostOperation)>>,
}

impl LocalType {
    pub(super) fn ty(&self) -> Type {
        self.ty
    }

    pub(super) fn members(&self) -> Option<&[(String, HostOperation)]> {
        self.members.as_deref()
    }
}

pub(super) type TypeScope<'scope, 'names> = ScopeFrame<'scope, 'names, LocalType>;

enum MemberSourceType<'scope> {
    Bound(&'scope LocalType),
    Selected(Type),
}

struct MemberSourceAdapter<'scope>(std::marker::PhantomData<&'scope LocalType>);

impl<'source, 'scope>
    crate::bound_projection_source::BoundProjectionSourceEnvironment<
        'source,
        ResultField,
        HostOperation,
    > for MemberSourceAdapter<'scope>
{
    type Result = MemberSourceType<'scope>;
    type Error = std::convert::Infallible;

    fn field(
        &mut self,
        _: &Self::Result,
        _: &'source str,
    ) -> Result<Option<(ResultField, ScalarType)>, Self::Error> {
        Ok(None)
    }

    fn member(
        &mut self,
        result: &Self::Result,
        _: &'source str,
        name: &'source str,
    ) -> Result<Option<(HostOperation, Self::Result)>, Self::Error> {
        let MemberSourceType::Bound(local) = result else {
            return Ok(None);
        };
        Ok(local.members().and_then(|members| {
            members
                .iter()
                .find(|(member, _)| member == name)
                .map(|(_, operation)| {
                    (
                        *operation,
                        MemberSourceType::Selected(operation.result_type()),
                    )
                })
        }))
    }
}

impl From<Type> for LocalType {
    fn from(ty: Type) -> Self {
        Self { ty, members: None }
    }
}

fn group_members(
    expression: &Computation,
    scope: &TypeScope<'_, '_>,
) -> Option<Vec<(String, HostOperation)>> {
    match expression {
        Computation::Bind { value, body, .. } if value.is_pure() => group_members(body, scope),
        Computation::Local { name } => scope.get(name)?.members.clone(),
        Computation::Host { .. } | Computation::Group { .. } | Computation::Choose { .. } => {
            group_signature(expression).map(|(_, members)| members)
        }
        _ => None,
    }
}

// Conditional exports are a closed signature, never a union of branch members.
fn group_signature(expression: &Computation) -> Option<(GroupKind, Vec<(String, HostOperation)>)> {
    expression.validate_structure().ok()?;
    let signature = crate::group_exports::observe_group_exports(
        expression,
        crate::group_exports::GroupExportLimits {
            max_nodes: MAX_COMPUTATION_NODES,
            max_depth: MAX_EFFECT_NESTING_DEPTH,
            max_groups: MAX_COMPUTATION_NODES,
            max_members: MAX_ALL_BRANCHES,
        },
        |effect| crate::pure_reference::native_group_exports(effect).ok_or(()),
        |branch| branch.value.prepared_atomic_operation().ok_or(()),
        |left, right| Ok(left == right),
    )
    .ok()?;
    Some((
        signature.kind,
        signature
            .members
            .into_iter()
            .map(|member| (member.name.to_owned(), member.operation))
            .collect(),
    ))
}

struct BindingValue {
    ty: Type,
    members: Option<Vec<(String, HostOperation)>>,
    group_member_count: Option<usize>,
    captures_result: bool,
    captures_group: bool,
    function_result: bool,
}

struct BindingAdapter<'adapter, 'scope, 'source> {
    scope: &'adapter mut TypeScope<'scope, 'source>,
    visited: &'adapter mut usize,
    depth: usize,
    functions: &'adapter mut functions::FunctionTemplates,
}

impl<'source>
    crate::binding_source::BindingSourceAdapter<'source, ResultField, HostOperation, Effect, Type>
    for BindingAdapter<'_, '_, 'source>
{
    type Value = BindingValue;
    type Result = Type;
    type Error = Vec<Diagnostic>;

    fn lower_value(
        &mut self,
        binding: &'source NamedArgument,
    ) -> Result<(Computation, BindingValue), Vec<Diagnostic>> {
        let (value, value_type) = lower_expression_with_functions(
            &binding.value,
            self.scope,
            self.visited,
            self.depth + 1,
            self.functions,
        )?;
        let captures_result = !value.is_pure();
        let direct_function_result = captures_result
            && matches!(&binding.value,
            Expression::Call { callee, .. } if self.functions.contains(callee));
        let members = group_members(&value, self.scope);
        let group_member_count = members.as_ref().map(Vec::len);
        let captures_group = captures_result && members.is_some();
        let atomic = value
            .prepared_atomic_operation()
            .is_some_and(|operation| operation.result_type() == value_type);
        let function_result = direct_function_result
            || (captures_result
                && !atomic
                && !captures_group
                && matches!(value_type, Type::Scalar(_))
                && crate::function_flow::is_data_call_selection(
                    &binding.value,
                    &value,
                    self.functions,
                ));
        if captures_result && !atomic && !captures_group && !function_result {
            return Err(invalid(
                "LSH1408",
                "result binding requires one prepared atomic operation or a flat named group",
                binding.span,
            ));
        }
        Ok((
            value,
            BindingValue {
                ty: value_type,
                members,
                group_member_count,
                captures_result,
                captures_group,
                function_result,
            },
        ))
    }

    fn lower_body(
        &mut self,
        source: &crate::binding_source::BindingSourceForm<'source>,
        _: &Computation,
        metadata: &mut BindingValue,
    ) -> Result<(Computation, Type), Vec<Diagnostic>> {
        let binding = source.binding();
        let mut local = self.scope.nested();
        local
            .push(
                &binding.name,
                LocalType {
                    ty: metadata.ty,
                    members: metadata.members.take(),
                },
            )
            .map_err(|_| {
                invalid(
                    "LSH1403",
                    "local name must be bounded and cannot shadow an active binding",
                    binding.span,
                )
            })?;
        lower_expression_with_functions(
            source.body(),
            &mut local,
            self.visited,
            self.depth + 1,
            self.functions,
        )
    }

    fn finish(
        &mut self,
        source: &crate::binding_source::BindingSourceForm<'source>,
        value: Computation,
        metadata: BindingValue,
        body: Computation,
        result_type: Type,
    ) -> Result<(Computation, Type), Vec<Diagnostic>> {
        let span = source.span();
        if metadata.captures_group
            && !((body.is_pure() && matches!(result_type, Type::Scalar(_)))
                || body.is_atomic_tail()
                || (body.is_result_flow() && matches!(result_type, Type::Scalar(_))))
        {
            return Err(invalid(
                "LSH1412",
                "a captured group requires a pure scalar body, one atomic tail, or a bounded scalar result flow",
                span,
            ));
        }
        if metadata.captures_group
            && !body.is_pure()
            && metadata.group_member_count.is_none_or(|count| {
                body.atomic_flow_bound()
                    .is_none_or(|bound| count.saturating_add(bound) > MAX_SEQUENCE_STEPS)
            })
        {
            return Err(invalid(
                "LSH1412",
                "a result-driven group and its longest atomic chain must fit in 64 steps",
                span,
            ));
        }
        if metadata.captures_result
            && !((body.is_result_flow() && matches!(result_type, Type::Scalar(_)))
                || body.is_result_chain())
        {
            return Err(invalid(
                "LSH1408",
                "a captured host result requires a pure scalar body or a bounded atomic result chain",
                span,
            ));
        }
        let expression = if metadata.function_result {
            crate::function_flow::bind_result(source.binding().name.clone(), value, body, span)?
        } else {
            source.construct(value, body)
        };
        Ok((expression, result_type))
    }
}

impl Computation {
    /// Checks bounded structure only; lexical types and authority need full HIR validation.
    pub fn validate_structure(&self) -> Result<(), CanonicalSourceError> {
        validate_shape(self)
    }

    /// Recognizes one atomic tail effect, with only pure preparation and selection.
    /// Callers must validate structure and lexical types before using this classification.
    pub fn is_atomic_tail(&self) -> bool {
        match self {
            Self::Host { effect } => HostOperation::for_effect(effect).is_some(),
            Self::Call { arguments, .. } => {
                arguments.iter().all(|argument| argument.value.is_pure())
            }
            Self::Bind { value, body, .. } => value.is_pure() && body.is_atomic_tail(),
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_atomic_tail() && otherwise.is_atomic_tail(),
            _ => false,
        }
    }

    /// One statically typed operation after pure preparation/selection, never a capture.
    /// Validate bounded structure and lexical types before relying on this signature.
    pub fn prepared_atomic_operation(&self) -> Option<HostOperation> {
        let mut pending = vec![(self, 0usize, true)];
        let mut budget = StructureBudget::new(MAX_COMPUTATION_NODES, MAX_EFFECT_NESTING_DEPTH);
        let mut signature = None;
        while let Some((value, depth, atomic)) = pending.pop() {
            let (extra_nodes, extra_depth) = match value {
                Self::Literal { value } => source_shape_extra(value),
                _ => (0, 0),
            };
            budget.visit(depth, extra_nodes, extra_depth).ok()?;
            if !atomic {
                if matches!(
                    value,
                    Self::Host { .. } | Self::Call { .. } | Self::Group { .. }
                ) {
                    return None;
                }
                for child in value.children() {
                    budget.check_pending(pending.len(), 1).ok()?;
                    pending.push((child, depth + 1, false));
                }
                continue;
            }
            let operation = match value {
                Self::Host { effect } => {
                    budget.visit(depth, 0, 1).ok()?;
                    HostOperation::for_effect(effect)?
                }
                Self::Call {
                    operation,
                    arguments,
                } => {
                    for argument in arguments {
                        budget.check_pending(pending.len(), 1).ok()?;
                        pending.push((&argument.value, depth + 1, false));
                    }
                    *operation
                }
                Self::Bind { value, body, .. } => {
                    budget.check_pending(pending.len(), 2).ok()?;
                    pending.push((value, depth + 1, false));
                    pending.push((body, depth + 1, true));
                    continue;
                }
                Self::Choose {
                    when,
                    then,
                    otherwise,
                } => {
                    budget.check_pending(pending.len(), 3).ok()?;
                    pending.push((when, depth + 1, false));
                    pending.push((then, depth + 1, true));
                    pending.push((otherwise, depth + 1, true));
                    continue;
                }
                _ => return None,
            };
            if signature.is_some_and(|expected| expected != operation) {
                return None;
            }
            signature = Some(operation);
        }
        signature
    }

    /// One captured atomic suspension followed by a pure body, with pure preparation/selection.
    pub fn is_atomic_capture(&self) -> bool {
        match self {
            Self::Bind { value, body, .. } if value.is_pure() => body.is_atomic_capture(),
            Self::Bind { value, body, .. } => {
                value.prepared_atomic_operation().is_some() && body.is_pure()
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_atomic_capture() && otherwise.is_atomic_capture(),
            _ => false,
        }
    }

    /// A bounded atomic chain, excluding effectful operands and dynamic groups.
    /// Every non-pure branch must reach an atomic suspension before returning.
    pub fn is_result_chain(&self) -> bool {
        match self {
            Self::Host { .. } | Self::Call { .. } => self.is_atomic_tail(),
            Self::Bind { value, body, .. } => {
                if value.is_pure() {
                    body.is_result_chain()
                } else {
                    value.prepared_atomic_operation().is_some()
                        && (body.is_pure() || body.is_result_chain())
                }
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_result_chain() && otherwise.is_result_chain(),
            _ => false,
        }
    }

    /// Maximum atomic suspensions on any path of a statically bounded result chain.
    /// Pure preparation does not consume a graph slot; cold branches still reserve slots.
    pub fn atomic_chain_bound(&self) -> Option<usize> {
        if !self.is_pure() && !self.is_result_chain() {
            return None;
        }
        self.atomic_flow_bound()
    }

    /// Includes scalar early exits while reserving the longest possible cold path.
    pub fn atomic_flow_bound(&self) -> Option<usize> {
        if !self.is_result_flow() {
            return None;
        }
        if self.is_pure() {
            return Some(0);
        }
        match self {
            Self::Host { .. } | Self::Call { .. } if self.is_atomic_tail() => Some(1),
            Self::Bind { value, body, .. } if value.is_pure() => body.atomic_flow_bound(),
            Self::Bind { value, body, .. } if value.prepared_atomic_operation().is_some() => {
                body.atomic_flow_bound()?.checked_add(1)
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } if when.is_pure() => Some(
                then.atomic_flow_bound()?
                    .max(otherwise.atomic_flow_bound()?),
            ),
            _ => None,
        }
    }

    /// Allows pure exits between atomic suspensions; callers must require a scalar final type.
    pub fn is_result_flow(&self) -> bool {
        if self.is_pure() {
            return true;
        }
        match self {
            Self::Host { .. } | Self::Call { .. } => self.is_atomic_tail(),
            Self::Bind { value, body, .. } => {
                (value.is_pure() || value.prepared_atomic_operation().is_some())
                    && body.is_result_flow()
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_result_flow() && otherwise.is_result_flow(),
            _ => false,
        }
    }

    /// Whether any path returns before another effect, without evaluating its guards.
    pub fn can_return_without_suspending(&self) -> bool {
        if self.is_pure() {
            return true;
        }
        match self {
            Self::Bind { value, body, .. } => {
                value.is_pure() && body.can_return_without_suspending()
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => {
                when.is_pure()
                    && (then.can_return_without_suspending()
                        || otherwise.can_return_without_suspending())
            }
            _ => false,
        }
    }

    /// Revalidates a residual computation against a bounded, typed lexical environment.
    pub fn validate_in_scope(
        &self,
        scope: &[(String, Type)],
    ) -> Result<Type, CanonicalSourceError> {
        self.validate_in_group_scope(scope, &[])
    }

    pub(super) fn validate_in_type_scope(
        &self,
        scope: &TypeScope<'_, '_>,
    ) -> Result<Type, CanonicalSourceError> {
        if scope.len() > crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        let mut values = Vec::new();
        let mut groups = Vec::new();
        for (name, local) in scope.bindings() {
            if let Some(members) = &local.members {
                if members.is_empty() || members.len() > MAX_SEQUENCE_STEPS {
                    return Err(CanonicalSourceError::RoundTripMismatch);
                }
                groups.push(GroupLocalType {
                    name: (*name).to_owned(),
                    members: members.clone(),
                });
            } else {
                values.push(((*name).to_owned(), local.ty));
            }
        }
        self.validate_scoped(
            &values,
            &groups,
            crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
            0,
            MAX_SEQUENCE_STEPS,
        )
    }

    /// Revalidates a residual body using closed, statically named group-member signatures.
    pub fn validate_in_group_scope(
        &self,
        scope: &[(String, Type)],
        groups: &[GroupLocalType],
    ) -> Result<Type, CanonicalSourceError> {
        self.validate_scoped(
            scope,
            groups,
            MAX_EFFECT_NESTING_DEPTH,
            scope.len().saturating_add(groups.len()),
            MAX_SEQUENCE_STEPS - 1,
        )
    }

    fn validate_scoped(
        &self,
        scope: &[(String, Type)],
        groups: &[GroupLocalType],
        max_bindings: usize,
        root_depth: usize,
        max_group_members: usize,
    ) -> Result<Type, CanonicalSourceError> {
        validate_shape(self)?;
        let mut names = HashSet::new();
        let scope_len = scope.len().saturating_add(groups.len());
        if scope_len > max_bindings
            || scope
                .iter()
                .any(|(name, _)| !valid_local(name) || !names.insert(name))
            || groups.iter().any(|group| {
                let mut members = HashSet::new();
                !valid_local(&group.name)
                    || !names.insert(&group.name)
                    || group.members.is_empty()
                    || group.members.len() > max_group_members
                    || group.members.iter().any(|(name, _)| {
                        name.is_empty()
                            || name.len() > MAX_BRANCH_NAME_BYTES
                            || !name.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
                            })
                            || !members.insert(name)
                    })
            })
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        let source = format!("fn main() = {}", source(self));
        let formatted = format_syntax(&parse(&source)).map_err(CanonicalSourceError::Syntax)?;
        let tree = parse(&formatted);
        let function = tree
            .function
            .ok_or(CanonicalSourceError::RoundTripMismatch)?;
        let mut visited = scope_len;
        let mut bindings = scope
            .iter()
            .map(|(name, ty)| (name.as_str(), LocalType::from(*ty)))
            .chain(groups.iter().map(|group| {
                (
                    group.name.as_str(),
                    LocalType {
                        ty: Type::Structured,
                        members: Some(group.members.clone()),
                    },
                )
            }))
            .collect();
        let mut lexical = ScopeFrame::new(&mut bindings);
        let (roundtrip, ty) =
            lower_expression(&function.body, &mut lexical, &mut visited, root_depth)
                .map_err(CanonicalSourceError::InvalidEffect)?;
        if roundtrip != *self {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        if self.is_pure()
            && crate::pure_reference::infer(self, scope, groups)
                .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
                != ty
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        if let Self::Call {
            operation,
            arguments,
        } = self
            && crate::pure_reference::infer_call(*operation, arguments, scope, groups)
                .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
                != ty
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        let prepared_call = !matches!(self, Self::Call { .. })
            && self.prepared_atomic_operation().is_some()
            && crate::prepared_typing::call_leaves_only(self);
        if prepared_call
            && crate::pure_reference::infer_prepared(self, scope, groups)
                .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
                != ty
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        if !prepared_call
            && !matches!(self, Self::Call { .. })
            && !self.is_pure()
            && crate::flow_typing::call_flow_nodes_only(self)
            && crate::pure_reference::infer_flow(self, scope, groups)
                .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
                != ty
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        if crate::flow_typing::group_flow_nodes_only(self)
            && crate::pure_reference::infer_group_flow(self, scope, groups)
                .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
                != ty
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        if crate::flow_typing::host_flow_nodes_only(self, crate::pure_reference::supports_host_flow)
            && crate::pure_reference::infer_host_flow(self, scope, groups)
                .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
                != ty
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        if crate::flow_typing::host_group_flow_nodes_only(
            self,
            crate::pure_reference::supports_host_flow,
        ) && crate::pure_reference::infer_host_group_flow(self, scope, groups)
            .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
            != ty
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        Ok(ty)
    }

    /// Includes cold branches. Validate bounded structure before inspecting authority.
    pub fn required_capabilities(&self) -> BTreeSet<&'static str> {
        let mut required = BTreeSet::new();
        let mut pending = vec![self];
        while let Some(expression) = pending.pop() {
            match expression {
                Self::Host { effect } => required.extend(required_capabilities_for_effect(effect)),
                Self::Call { operation, .. } => {
                    required.insert(operation.required_capability());
                }
                _ => {}
            }
            pending.extend(expression.children());
        }
        required
    }
}

fn has_computed_arguments(arguments: &[NamedArgument]) -> bool {
    arguments.iter().any(|argument| {
        matches!(
            argument.value,
            Expression::Call { .. } | Expression::Reference { .. }
        )
    })
}

pub(crate) fn is_computation(expression: &Expression) -> bool {
    let mut pending = vec![(expression, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        if depth > MAX_EFFECT_NESTING_DEPTH {
            return true;
        }
        let Expression::Call {
            callee, arguments, ..
        } = expression
        else {
            return true;
        };
        if matches!(
            callee.as_str(),
            "bind" | "loop" | "fold" | "strings" | "choose" | "recover" | "field" | "member"
        ) || BinaryOperator::parse(callee).is_some()
            || UnaryOperator::parse(callee).is_some()
            || (HostOperation::parse(callee).is_some() && has_computed_arguments(arguments))
        {
            return true;
        }
        match callee.as_str() {
            "seq" | "all" => pending.extend(
                arguments
                    .iter()
                    .map(|argument| (&argument.value, depth + 1)),
            ),
            "repeat" => pending.extend(
                arguments
                    .iter()
                    .filter(|argument| argument.name == "body")
                    .map(|argument| (&argument.value, depth + 1)),
            ),
            _ => {}
        }
    }
    false
}

fn invalid(code: &str, message: impl Into<String>, span: Span) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: code.to_string(),
        message: message.into(),
        span: Some(span),
    }]
}

fn projection_source_diagnostics(
    error: crate::projection_source::ProjectionSourceError<Vec<Diagnostic>>,
    at: Span,
) -> Vec<Diagnostic> {
    use crate::projection_source::ProjectionSourceError;
    match error {
        ProjectionSourceError::Names { span } => invalid(
            "LSH1401",
            "expected exactly the named arguments: value, name",
            span,
        ),
        ProjectionSourceError::FieldName { span } => {
            invalid("LSH1409", "field name must be a string literal", span)
        }
        ProjectionSourceError::MemberShape { span } => invalid(
            "LSH1412",
            "member requires a bound group reference and a literal step name",
            span,
        ),
        ProjectionSourceError::Native { error, .. } => error,
        ProjectionSourceError::NonResult { .. }
        | ProjectionSourceError::FieldNotExported { .. }
        | ProjectionSourceError::Produced {
            error: crate::pure_typing::PureTypeError::Impure,
            ..
        } => invalid(
            "LSH1409",
            "field is not exported by this bound result type",
            at,
        ),
        _ => invalid(
            "LSH1405",
            "computation exceeds its node or nesting limit",
            at,
        ),
    }
}

fn scalar_source_diagnostics(
    error: crate::scalar_source::ScalarSourceError<Vec<Diagnostic>>,
    at: Span,
) -> Vec<Diagnostic> {
    use crate::scalar_source::ScalarSourceError;
    match error {
        ScalarSourceError::Native { error, .. } => error,
        ScalarSourceError::Names { span, expected } => invalid(
            "LSH1401",
            format!(
                "expected exactly the named arguments: {}",
                expected.join(", ")
            ),
            span,
        ),
        ScalarSourceError::LiteralLimit { span } => {
            invalid("LSH1405", "scalar string exceeds 4096 bytes", span)
        }
        ScalarSourceError::NonScalar { span } => invalid(
            "LSH1402",
            "expected a pure scalar expression, not a host operation",
            span,
        ),
        ScalarSourceError::Binary {
            span,
            operator,
            left,
            right,
        } => invalid(
            "LSH1402",
            format!("{} does not accept {left:?} and {right:?}", operator.name()),
            span,
        ),
        ScalarSourceError::Unary { span, operator } => invalid(
            "LSH1402",
            format!(
                "{} requires a pure {}",
                operator.name(),
                operator.expected_input()
            ),
            span,
        ),
        ScalarSourceError::Condition { span } => {
            invalid("LSH1402", "choose when requires a boolean", span)
        }
        ScalarSourceError::BranchTypes { span } => invalid(
            "LSH1404",
            "choose branches must have the same result type",
            span,
        ),
        ScalarSourceError::RecoveryTypes { span } => invalid(
            "LSH1411",
            "recover value and fallback must be pure expressions of the same scalar type",
            span,
        ),
        ScalarSourceError::StringCount { span } => {
            invalid("LSH1405", "string list exceeds 64 entries", span)
        }
        ScalarSourceError::StringLabels { span } => invalid(
            "LSH1403",
            "string list labels must be bounded and unique",
            span,
        ),
        ScalarSourceError::StringItem { span } => {
            invalid("LSH1402", "string list entries require strings", span)
        }
        ScalarSourceError::StringBytes { span } => {
            invalid("LSH1405", "string list exceeds 4096 bytes", span)
        }
        ScalarSourceError::BindingShape { span } => {
            invalid("LSH1401", "bind requires one named value and 'body'", span)
        }
        ScalarSourceError::Bindings { span, .. } => invalid(
            "LSH1403",
            "local name must be bounded and cannot shadow an active binding",
            span,
        ),
        ScalarSourceError::LoopLimit {
            span,
            exceeds_bound,
        } => invalid(
            "LSH1410",
            if exceeds_bound {
                "loop limit exceeds 1024 iterations"
            } else {
                "loop limit must be an integer literal from 0 through 1024"
            },
            span,
        ),
        ScalarSourceError::LoopCondition { span } => {
            invalid("LSH1402", "loop while requires a boolean", span)
        }
        ScalarSourceError::LoopState { span } => invalid(
            "LSH1402",
            "loop next must preserve the initial state's scalar type",
            span,
        ),
        ScalarSourceError::FoldItem { span } => {
            invalid("LSH1413", "fold item requires a literal local name", span)
        }
        ScalarSourceError::FoldBindings { span, .. } => invalid(
            "LSH1403",
            "fold locals must be bounded, distinct and cannot shadow active bindings",
            span,
        ),
        ScalarSourceError::FoldLimit {
            span,
            exceeds_bound,
        } => invalid(
            "LSH1413",
            if exceeds_bound {
                "fold limit exceeds 64 entries"
            } else {
                "fold limit requires an integer literal from 0 through 64"
            },
            span,
        ),
        ScalarSourceError::FoldItems { span } => {
            invalid("LSH1402", "fold items requires a string_list", span)
        }
        ScalarSourceError::FoldState { span } => invalid(
            "LSH1402",
            "fold next must preserve its initial state's type",
            span,
        ),
        _ => invalid(
            "LSH1405",
            "computation exceeds its node or nesting limit",
            at,
        ),
    }
}

pub(super) fn lower_computation(expression: &Expression) -> Result<LoweredEffect, Vec<Diagnostic>> {
    lower_computation_with_functions(expression, &mut functions::FunctionTemplates::default())
}

pub(super) fn lower_computation_with_functions(
    expression: &Expression,
    functions: &mut functions::FunctionTemplates,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let (expression, result_type) =
        lower_expression_with_functions(expression, &mut scope, &mut 0, 0, functions)?;
    validate_shape(&expression).map_err(|error| match error {
        CanonicalSourceError::InvalidEffect(errors) => errors,
        _ => invalid(
            "LSH1405",
            "invalid computation shape",
            Span { start: 0, end: 0 },
        ),
    })?;
    let effect = Effect::Compute {
        expression: Box::new(expression),
    };
    let required_capabilities = required_capabilities_for_effect(&effect)
        .into_iter()
        .map(str::to_string)
        .collect();
    Ok(LoweredEffect {
        effect,
        result_type,
        required_capabilities,
    })
}

pub(super) fn lower_expression<'a>(
    expression: &'a Expression,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    lower_expression_with_functions(
        expression,
        scope,
        visited,
        depth,
        &mut functions::FunctionTemplates::default(),
    )
}

pub(super) fn lower_expression_with_functions<'a>(
    expression: &'a Expression,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    *visited += 1;
    let span = expression_span(expression);
    if *visited > MAX_COMPUTATION_NODES || depth > MAX_EFFECT_NESTING_DEPTH {
        return Err(invalid(
            "LSH1405",
            "computation exceeds its node or nesting limit",
            span,
        ));
    }
    if crate::scalar_source::primitive(expression) {
        let (value, ty) = crate::scalar_source::lower_form(expression, |child| {
            let (value, ty) =
                lower_expression_with_functions(child, scope, visited, depth + 1, functions)
                    .map_err(|error| crate::scalar_source::ScalarSourceError::Native {
                        span: expression_span(child),
                        error,
                    })?;
            let pure = value.is_pure();
            Ok(crate::scalar_source::Operand {
                value,
                pure,
                scalar_type: match ty {
                    Type::Scalar(ty) => Some(ty),
                    _ => None,
                },
            })
        })
        .map_err(|error| scalar_source_diagnostics(error, span))?;
        return Ok((value, Type::Scalar(ty)));
    }
    if let Expression::Reference { name, .. } = expression {
        return scope
            .get(name)
            .map(|ty| (Computation::Local { name: name.clone() }, ty.ty))
            .ok_or_else(|| invalid("LSH1403", format!("undefined local '{name}'"), span));
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(invalid("LSH1401", "invalid computation", span));
    };
    if matches!(callee.as_str(), "seq" | "repeat" | "all") {
        return computed_group::lower(expression, scope, visited, depth, functions);
    }
    if callee == "loop" {
        return lower_loop(arguments, span, scope, visited, depth, functions);
    }
    if callee == "fold" {
        return lower_fold(arguments, span, scope, visited, depth, functions);
    }
    if callee == "bind" {
        let prefix = scope
            .bindings()
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        return crate::binding_source::lower_binding_source(
            expression,
            &prefix,
            crate::binding_source::BindingSourceLimits {
                source: crate::source_call::SourceCallLimits {
                    max_source_nodes: MAX_COMPUTATION_NODES,
                    max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_lowered_nodes: MAX_COMPUTATION_NODES,
                    max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
                },
                max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
            },
            &mut BindingAdapter {
                scope,
                visited,
                depth,
                functions,
            },
        )
        .map_err(|error| match error {
            crate::binding_source::BindingSourceError::Source(error) => {
                scalar_source_diagnostics(error, span)
            }
            crate::binding_source::BindingSourceError::Native { error, .. } => error,
            crate::binding_source::BindingSourceError::Output { .. } => invalid(
                "LSH1405",
                "computation exceeds its node or nesting limit",
                span,
            ),
        });
    }
    if callee == "member" {
        use crate::bound_projection_source::{
            BoundProjectionSourceError, BoundProjectionSourceLimits, lower_bound_projection_source,
        };
        use crate::projection_source::ProjectionSourceError;
        use crate::pure_typing::PureType;

        // Native observations borrow the exact prefix; group signatures are not
        // cloned, flattened into a global name registry or inferred from tags.
        let observed = scope
            .bindings()
            .iter()
            .map(|(_, local)| match local.ty {
                Type::Scalar(ty) => PureType::Scalar(ty),
                _ => PureType::Result(MemberSourceType::Bound(local)),
            })
            .collect::<Vec<_>>();
        let prefix = scope
            .bindings()
            .iter()
            .zip(&observed)
            .map(|((name, _), ty)| (*name, ty))
            .collect::<Vec<_>>();
        let (value, ty) = lower_bound_projection_source(
            expression,
            &prefix,
            BoundProjectionSourceLimits {
                source: crate::source_call::SourceCallLimits {
                    max_source_nodes: MAX_COMPUTATION_NODES,
                    max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_lowered_nodes: MAX_COMPUTATION_NODES,
                    max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
                },
                max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
            },
            &mut MemberSourceAdapter(std::marker::PhantomData),
        )
        .map_err(|error| match error {
            BoundProjectionSourceError::Projection(ProjectionSourceError::Names { span }) => {
                invalid(
                    "LSH1401",
                    "expected exactly the named arguments: value, name",
                    span,
                )
            }
            BoundProjectionSourceError::Projection(
                ProjectionSourceError::MemberShape { .. } | ProjectionSourceError::FieldName { .. },
            )
            | BoundProjectionSourceError::BoundReference { .. } => invalid(
                "LSH1412",
                "member requires a bound group reference and a literal step name",
                span,
            ),
            BoundProjectionSourceError::Unbound { .. }
            | BoundProjectionSourceError::Projection(
                ProjectionSourceError::MemberNotExported { .. }
                | ProjectionSourceError::MemberNames { .. }
                | ProjectionSourceError::NonResult { .. },
            ) => invalid(
                "LSH1412",
                "member is not exported by this bound group",
                span,
            ),
            BoundProjectionSourceError::Projection(ProjectionSourceError::Native {
                error, ..
            }) => match error {},
            _ => invalid(
                "LSH1405",
                "computation exceeds its node or nesting limit",
                span,
            ),
        })?;
        return match ty {
            PureType::Result(MemberSourceType::Selected(ty)) => Ok((value, ty)),
            _ => Err(invalid(
                "LSH1405",
                "invalid native member result observation",
                span,
            )),
        };
    }
    if callee == "field" {
        let source = crate::projection_source::field_source(arguments, span)
            .map_err(|error| projection_source_diagnostics(error, span))?;
        let (value, ty) = source
            .lower_preflighted(
                crate::source_call::SourceCallLimits {
                    max_source_nodes: MAX_COMPUTATION_NODES,
                    max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_lowered_nodes: MAX_COMPUTATION_NODES,
                    max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
                },
                |child| {
                    lower_expression_with_functions(child, scope, visited, depth + 1, functions)
                },
                |name| {
                    ResultField::parse(name)
                        .ok_or_else(|| invalid("LSH1409", "unknown result field", span))
                },
                |input_type, field| Ok(field.result_type(*input_type).map(|ty| (field, ty))),
            )
            .map_err(|error| projection_source_diagnostics(error, span))?;
        return Ok((value, Type::Scalar(ty)));
    }
    if callee == "choose" {
        use crate::choice_source::ChoiceSourceError;
        return crate::choice_source::lower_choice_source(
            expression,
            crate::source_call::SourceCallLimits {
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                max_lowered_nodes: MAX_COMPUTATION_NODES,
                max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
            },
            |_, child| lower_expression_with_functions(child, scope, visited, depth + 1, functions),
            |_, ty| Ok(*ty == Type::Scalar(ScalarType::Boolean)),
            |left, right| Ok(left == right),
        )
        .map_err(|error| match error {
            ChoiceSourceError::Source(error) => scalar_source_diagnostics(
                crate::scalar_source::ScalarSourceError::Source(error),
                span,
            ),
            ChoiceSourceError::Native { error, .. } => error,
            ChoiceSourceError::Names { span } => invalid(
                "LSH1401",
                "expected exactly the named arguments: when, then, otherwise",
                span,
            ),
            ChoiceSourceError::Condition { span }
            | ChoiceSourceError::ProducedWhen {
                span,
                error: crate::pure_typing::PureTypeError::Impure,
            } => invalid("LSH1402", "choose when requires a boolean", span),
            ChoiceSourceError::BranchTypes { span } => invalid(
                "LSH1404",
                "choose branches must have the same result type",
                span,
            ),
            ChoiceSourceError::Output { .. } | ChoiceSourceError::ProducedWhen { .. } => invalid(
                "LSH1405",
                "computation exceeds its node or nesting limit",
                span,
            ),
        });
    }
    if functions.contains(callee) {
        return functions::lower_call(callee, arguments, span, scope, visited, depth, functions);
    }
    if let Some(operation) = HostOperation::parse(callee)
        && has_computed_arguments(arguments)
    {
        use crate::source_call::{
            SourceCallError, SourceCallFinishError, SourceCallFinishLimits, SourceCallLimits,
            lower_preflighted_arguments,
        };
        let lowered = lower_preflighted_arguments(
            arguments,
            operation.schema(),
            SourceCallLimits {
                max_source_nodes: MAX_COMPUTATION_NODES,
                max_source_depth: MAX_EFFECT_NESTING_DEPTH,
                max_lowered_nodes: MAX_COMPUTATION_NODES,
                max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
                max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
            },
            |argument| {
                let (value, ty) = lower_expression_with_functions(
                    &argument.value,
                    scope,
                    visited,
                    depth + 1,
                    functions,
                )?;
                Ok((
                    value,
                    match ty {
                        Type::Scalar(ty) => Some(ty),
                        _ => None,
                    },
                ))
            },
        )
        .map_err(|error| match error {
            SourceCallError::Lowering { error, .. } => error,
            SourceCallError::ArgumentLimit => {
                invalid_argument("too many host arguments", Some(span))
            }
            SourceCallError::Names(_) => invalid_argument(
                format!("invalid named arguments for {}", operation.name()),
                Some(span),
            ),
            SourceCallError::Argument {
                parameter_index,
                argument_index,
                error,
            } => {
                let argument = &arguments[argument_index];
                match error {
                    ArgumentTypeError::Impure | ArgumentTypeError::NonScalar => invalid(
                        "LSH1402",
                        "expected a pure scalar expression, not a host operation",
                        argument.span,
                    ),
                    _ => invalid_argument(
                        format!(
                            "invalid {} argument '{}'",
                            operation.name(),
                            operation.parameters()[parameter_index].name
                        ),
                        Some(argument.span),
                    ),
                }
            }
            _ => invalid(
                "LSH1405",
                "computation exceeds its node or nesting limit",
                span,
            ),
        })?;
        return lowered
            .finish_call(
                SourceCallFinishLimits {
                    max_nodes: MAX_COMPUTATION_NODES,
                    max_depth: MAX_EFFECT_NESTING_DEPTH,
                    max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
                },
                |_| Ok::<_, std::convert::Infallible>((operation, operation.result_type())),
                |call, result, schema| {
                    let Computation::Call { arguments, .. } = call else {
                        return Err(invalid("LSH1405", "invalid computed host call", span));
                    };
                    if std::ptr::eq(schema, operation.schema())
                        && crate::pure_reference::infer_source_call_in_scope(
                            operation, arguments, scope,
                        )
                        .ok()
                            == Some(*result)
                    {
                        Ok(())
                    } else {
                        Err(invalid(
                            "LSH1405",
                            "computation exceeds its node or nesting limit",
                            span,
                        ))
                    }
                },
            )
            .map_err(|error| match error {
                SourceCallFinishError::Mapping(error) => match error {},
                SourceCallFinishError::Admission(error) => error,
                _ => invalid(
                    "LSH1405",
                    "computation exceeds its node or nesting limit",
                    span,
                ),
            });
    }
    crate::host_source::lower_preflighted_host_source(
        expression,
        crate::source_call::SourceCallLimits {
            max_source_nodes: MAX_COMPUTATION_NODES,
            max_source_depth: MAX_EFFECT_NESTING_DEPTH,
            max_lowered_nodes: MAX_COMPUTATION_NODES,
            max_lowered_depth: MAX_EFFECT_NESTING_DEPTH,
            max_arguments: crate::source_call::MAX_SOURCE_CALL_ARGUMENTS,
        },
        |source| {
            let lowered = lower_effect(source)?;
            Ok((lowered.effect, lowered.result_type))
        },
        |_, effect, result| {
            if contains_computation(effect) {
                return Err(invalid(
                    "LSH1406",
                    "computation must wrap a host group, not occur inside its branches",
                    span,
                ));
            }
            let operation = HostOperation::parse(callee);
            if operation == HostOperation::for_effect(effect)
                && operation.map(HostOperation::result_type) == Some(*result)
            {
                Ok(())
            } else {
                Err(invalid(
                    "LSH1999",
                    "invalid native host source observation",
                    span,
                ))
            }
        },
    )
    .map_err(|error| match error {
        crate::host_source::HostSourceError::Preparation { error, .. }
        | crate::host_source::HostSourceError::Admission { error, .. } => error,
        _ => invalid(
            "LSH1405",
            "computation exceeds its node or nesting limit",
            span,
        ),
    })
}

fn lower_loop<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    use crate::scalar_source::{Operand, ScalarSourceError};
    let source =
        crate::scalar_source::loop_source(arguments, span).map_err(|error| match error {
            ScalarSourceError::LoopShape {
                span,
                missing_state: true,
            } => invalid("LSH1410", "loop requires a named initial state", span),
            ScalarSourceError::LoopShape {
                span,
                missing_state: false,
            } => {
                let name = arguments
                    .iter()
                    .find(|argument| !["while", "next", "limit"].contains(&argument.name.as_str()))
                    .map(|argument| argument.name.as_str())
                    .unwrap_or("");
                invalid(
                    "LSH1401",
                    format!("expected exactly the named arguments: {name}, while, next, limit"),
                    span,
                )
            }
            ScalarSourceError::Bindings { span, .. } => invalid(
                "LSH1403",
                "loop state name must be bounded and cannot shadow an active binding",
                span,
            ),
            _ => scalar_source_diagnostics(error, span),
        })?;
    let binding = source.binding;
    if scope.get(&binding.name).is_some() {
        return Err(invalid(
            "LSH1403",
            "loop state name must be bounded and cannot shadow an active binding",
            binding.span,
        ));
    }
    let limit = source
        .limit()
        .map_err(|error| scalar_source_diagnostics(error, span))?;
    let (initial, state_type) =
        lower_expression_with_functions(&binding.value, scope, visited, depth + 1, functions)?;
    let (initial, scalar_type) = crate::scalar_source::scalar_operand(
        Operand {
            pure: initial.is_pure(),
            value: initial,
            scalar_type: match state_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        expression_span(&binding.value),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    let body = {
        let mut local = scope.nested();
        local.push(&binding.name, state_type.into()).map_err(|_| {
            invalid(
                "LSH1403",
                "loop state name must be bounded and cannot shadow an active binding",
                binding.span,
            )
        })?;
        crate::scalar_source::lower_loop_body(&source, scalar_type, |child| {
            let (value, ty) =
                lower_expression_with_functions(child, &mut local, visited, depth + 1, functions)
                    .map_err(|error| ScalarSourceError::Native {
                    span: expression_span(child),
                    error,
                })?;
            Ok(Operand {
                pure: value.is_pure(),
                value,
                scalar_type: match ty {
                    Type::Scalar(ty) => Some(ty),
                    _ => None,
                },
            })
        })
        .map_err(|error| scalar_source_diagnostics(error, span))?
    };
    Ok((source.construct(initial, body, limit), state_type))
}

fn lower_fold<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    use crate::scalar_source::{Operand, ScalarSourceError};
    let source =
        crate::scalar_source::fold_source(arguments, span).map_err(|error| match error {
            ScalarSourceError::FoldShape {
                span,
                missing_state: true,
            } => invalid("LSH1413", "fold requires a named initial state", span),
            ScalarSourceError::FoldShape {
                span,
                missing_state: false,
            } => {
                let name = arguments
                    .iter()
                    .find(|argument| {
                        !["items", "item", "next", "limit"].contains(&argument.name.as_str())
                    })
                    .map(|argument| argument.name.as_str())
                    .unwrap_or("");
                invalid(
                    "LSH1401",
                    format!(
                        "expected exactly the named arguments: {name}, items, item, next, limit"
                    ),
                    span,
                )
            }
            _ => scalar_source_diagnostics(error, span),
        })?;
    source
        .check_scope(scope, None)
        .map_err(|error| scalar_source_diagnostics(error, span))?;
    let limit = source
        .limit()
        .map_err(|error| scalar_source_diagnostics(error, span))?;
    // The collection is prepared before the accumulator, independent of argument spelling order.
    let (items, items_type) =
        lower_expression_with_functions(source.items, scope, visited, depth + 1, functions)?;
    let items = crate::scalar_source::fold_items(
        Operand {
            pure: items.is_pure(),
            value: items,
            scalar_type: match items_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        expression_span(source.items),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    let (initial, state_type) = lower_expression_with_functions(
        &source.binding.value,
        scope,
        visited,
        depth + 1,
        functions,
    )?;
    let (initial, scalar_type) = crate::scalar_source::scalar_operand(
        Operand {
            pure: initial.is_pure(),
            value: initial,
            scalar_type: match state_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        expression_span(&source.binding.value),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    let (next, next_type) = {
        let mut local = scope.nested();
        for (name, ty) in [
            (source.binding.name.as_str(), state_type),
            (source.item, Type::Scalar(ScalarType::String)),
        ] {
            local.push(name, ty.into()).map_err(|_| {
                scalar_source_diagnostics(
                    source.binding_error(crate::pure_typing::PureTypeError::ShadowedBinding),
                    span,
                )
            })?;
        }
        lower_expression_with_functions(source.next, &mut local, visited, depth + 1, functions)?
    };
    let next = crate::scalar_source::fold_next(
        Operand {
            pure: next.is_pure(),
            value: next,
            scalar_type: match next_type {
                Type::Scalar(ty) => Some(ty),
                _ => None,
            },
        },
        scalar_type,
        expression_span(source.next),
    )
    .map_err(|error| scalar_source_diagnostics(error, span))?;
    Ok((source.construct(items, initial, next, limit), state_type))
}

pub(super) fn valid_local(name: &str) -> bool {
    crate::pure_typing::valid_local_name(name)
}

pub(super) fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "all"
            | "seq"
            | "repeat"
            | "bind"
            | "loop"
            | "fold"
            | "strings"
            | "choose"
            | "recover"
            | "field"
            | "member"
    ) || BinaryOperator::parse(name).is_some()
        || UnaryOperator::parse(name).is_some()
}

pub(crate) fn contains_computation(effect: &Effect) -> bool {
    let mut pending = vec![effect];
    while let Some(effect) = pending.pop() {
        match effect {
            Effect::Compute { .. } => return true,
            Effect::All { branches } | Effect::Sequence { steps: branches } => {
                pending.extend(branches.iter().map(|branch| &branch.effect));
            }
            _ => {}
        }
    }
    false
}

pub(super) fn validate_shape(expression: &Computation) -> Result<(), CanonicalSourceError> {
    let mut pending = vec![(expression, 0usize)];
    let mut budget = StructureBudget::new(MAX_COMPUTATION_NODES, MAX_EFFECT_NESTING_DEPTH);
    while let Some((expression, depth)) = pending.pop() {
        // Literal constructors retain the source node/depth budget after constant folding.
        let (literal_nodes, literal_depth) = match expression {
            Computation::Literal { value } => source_shape_extra(value),
            _ => (0, 0),
        };
        let shape = budget.visit(depth, literal_nodes, literal_depth);
        let invalid_value =
            match expression {
                Computation::Literal { value } => !value.is_bounded(),
                Computation::Strings { items } => items.len() > MAX_STRING_LIST_ITEMS,
                Computation::Local { name } | Computation::Bind { name, .. } => !valid_local(name),
                Computation::Member { group, name, .. } => {
                    !valid_local(group) || name.len() > MAX_BRANCH_NAME_BYTES
                }
                Computation::Loop { name, limit, .. } => {
                    !valid_local(name)
                        || matches!(name.as_str(), "while" | "next" | "limit")
                        || *limit > MAX_LOOP_ITERATIONS
                }
                Computation::Fold {
                    name, item, limit, ..
                } => {
                    !valid_local(name)
                        || !valid_local(item)
                        || name == item
                        || matches!(name.as_str(), "items" | "item" | "next" | "limit")
                        || *limit > MAX_STRING_LIST_ITEMS as u64
                }
                Computation::Call {
                    operation,
                    arguments,
                } => {
                    arguments.len() > operation.parameters().len()
                        || arguments
                            .iter()
                            .any(|argument| argument.name.len() > MAX_BRANCH_NAME_BYTES)
                }
                Computation::Group {
                    group_kind,
                    branches,
                } => {
                    let minimum = if *group_kind == GroupKind::Sequence {
                        1
                    } else {
                        2
                    };
                    !(minimum..=MAX_ALL_BRANCHES).contains(&branches.len())
                        || branches.iter().any(|branch| {
                            branch.name.len() > MAX_BRANCH_NAME_BYTES
                                || branch.value.prepared_atomic_operation().is_none_or(
                                    |operation| branch.result_type != operation.result_type(),
                                )
                        })
                }
                _ => false,
            };
        if shape.is_err() || invalid_value {
            return Err(CanonicalSourceError::InvalidEffect(invalid(
                "LSH1405",
                "invalid or oversized computation",
                Span { start: 0, end: 0 },
            )));
        }
        if let Computation::Host { effect } = expression {
            let mut effects = vec![(effect.as_ref(), depth)];
            while let Some((effect, effect_depth)) = effects.pop() {
                if budget.visit(effect_depth, 0, 1).is_err()
                    || matches!(effect, Effect::Compute { .. })
                {
                    return Err(CanonicalSourceError::InvalidEffect(invalid(
                        "LSH1405",
                        "invalid or oversized computation host graph",
                        Span { start: 0, end: 0 },
                    )));
                }
                if let Effect::All { branches } | Effect::Sequence { steps: branches } = effect {
                    if branches.len() > MAX_ALL_BRANCHES {
                        return Err(CanonicalSourceError::InvalidEffect(invalid(
                            "LSH1405",
                            "oversized computation host group",
                            Span { start: 0, end: 0 },
                        )));
                    }
                    effects.extend(
                        branches
                            .iter()
                            .map(|branch| (&branch.effect, effect_depth + 1)),
                    );
                }
            }
        }
        pending.extend(expression.children().map(|child| (child, depth + 1)));
    }
    Ok(())
}

pub(super) fn source(expression: &Computation) -> String {
    match expression {
        Computation::Literal { value } => match value {
            ScalarValue::Integer(value) => value.to_string(),
            ScalarValue::Boolean(value) => value.to_string(),
            ScalarValue::String(value) => quote(value),
            ScalarValue::None => "none".to_string(),
            ScalarValue::OptionalString(value) => format!(
                "optional_string(value: {})",
                value.0.as_deref().map_or_else(|| "none".into(), quote)
            ),
            ScalarValue::StringList(value) => format!(
                "strings({})",
                value
                    .0
                    .iter()
                    .enumerate()
                    .map(|(index, value)| format!("item{index}: {}", quote(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
        Computation::Strings { items } => format!(
            "strings({})",
            items
                .iter()
                .enumerate()
                .map(|(index, value)| format!("item{index}: {}", source(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Computation::Local { name } => name.clone(),
        Computation::Member { group, name, .. } => {
            format!("member(value: {group}, name: {})", quote(name))
        }
        Computation::Field { value, field } => format!(
            "field(value: {}, name: {})",
            source(value),
            quote(field.name())
        ),
        Computation::Binary {
            operator,
            left,
            right,
        } => format!(
            "{}(left: {}, right: {})",
            operator.name(),
            source(left),
            source(right)
        ),
        Computation::Unary { operator, value } => {
            format!("{}(value: {})", operator.name(), source(value))
        }
        Computation::Recover { value, fallback } => format!(
            "recover(value: {}, fallback: {})",
            source(value),
            source(fallback)
        ),
        Computation::Bind { name, value, body } => {
            format!("bind({name}: {}, body: {})", source(value), source(body))
        }
        Computation::Loop {
            name,
            initial,
            condition,
            next,
            limit,
        } => format!(
            "loop({name}: {}, while: {}, next: {}, limit: {limit})",
            source(initial),
            source(condition),
            source(next)
        ),
        Computation::Fold {
            name,
            item,
            items,
            initial,
            next,
            limit,
        } => format!(
            "fold({name}: {}, items: {}, item: {}, next: {}, limit: {limit})",
            source(initial),
            source(items),
            quote(item),
            source(next)
        ),
        Computation::Choose {
            when,
            then,
            otherwise,
        } => format!(
            "choose(when: {}, then: {}, otherwise: {})",
            source(when),
            source(then),
            source(otherwise)
        ),
        Computation::Host { effect } => canonical_effect_source(effect, 0),
        Computation::Call {
            operation,
            arguments,
        } => format!(
            "{}({})",
            operation.name(),
            arguments
                .iter()
                .map(|argument| format!("{}: {}", argument.name, source(&argument.value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Computation::Group {
            group_kind,
            branches,
        } => format!(
            "{}({})",
            if *group_kind == GroupKind::Sequence {
                "seq"
            } else {
                "all"
            },
            branches
                .iter()
                .map(|branch| format!("{}: {}", branch.name, source(&branch.value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn native_host_source_charges_one_original_root_and_never_visits_literal_metadata() {
        for (body, succeeds) in [
            ("runtime.list()", true),
            ("runtime.inspect(runtime_id: \"a\")", true),
            ("runtime.inspect(runtime_id: 7)", false),
            ("unknown.call()", false),
        ] {
            let tree = parse(&format!("fn main() = {body}"));
            let mut bindings = Vec::new();
            let mut scope = ScopeFrame::new(&mut bindings);
            let mut visited = 0;
            let result = lower_expression(
                &tree.function.as_ref().unwrap().body,
                &mut scope,
                &mut visited,
                0,
            );
            assert_eq!(result.is_ok(), succeeds, "{body}");
            assert_eq!(visited, 1, "{body}");
            assert!(scope.is_empty());
            if let Ok((value, _)) = result {
                assert!(matches!(value, Computation::Host { .. }));
            }
        }
    }

    #[test]
    fn field_stages_charge_original_child_once_without_lowering_name_metadata() {
        for (body, expected) in [
            ("field(value: row, name: \"count\")", None),
            ("field(value: row, name: 1)", Some("LSH1409")),
            ("field(value: row, name: \"unknown\")", Some("LSH1409")),
        ] {
            let tree = parse(&format!("fn main() = {body}"));
            let mut bindings = vec![("row", Type::RuntimeList.into())];
            let mut scope = ScopeFrame::new(&mut bindings);
            let mut visited = 0;
            let result = lower_expression(
                &tree.function.as_ref().unwrap().body,
                &mut scope,
                &mut visited,
                0,
            );
            assert_eq!(visited, 2);
            assert_eq!(scope.len(), 1);
            assert_eq!(scope.local_len(), 0);
            if let Some(code) = expected {
                assert_eq!(result.unwrap_err()[0].code, code);
            } else {
                assert_eq!(result.unwrap().1, Type::Scalar(ScalarType::Integer));
            }
        }
    }

    #[test]
    fn failed_lowering_cleans_locals_without_copying_parent_names_or_refunding_nodes() {
        for body in [
            "bind(tmp: outer, body: bind(inner: tmp, body: missing))",
            r#"loop(tmp: outer, while: true, next: "bad", limit: 1)"#,
            "loop(tmp: outer, while: 1, next: tmp, limit: 0)",
            r#"fold(tmp: outer, items: strings(a: "x"), item: "entry", next: missing, limit: 1)"#,
            r#"fold(tmp: outer, items: strings(a: "x"), item: "entry", next: entry, limit: 1)"#,
        ] {
            let tree = parse(&format!("fn main() = {body}"));
            let good = parse("fn main() = bind(tmp: outer, body: tmp)");
            let name = String::from("outer");
            let mut bindings = vec![(name.as_str(), Type::Scalar(ScalarType::Integer).into())];
            let mut scope = ScopeFrame::new(&mut bindings);
            let mut visited = 1;
            assert!(
                lower_expression(
                    &tree.function.as_ref().unwrap().body,
                    &mut scope,
                    &mut visited,
                    1
                )
                .is_err(),
                "{body}"
            );
            let spent = visited;
            assert!(spent > 1);
            assert_eq!(scope.len(), 1);
            assert_eq!(scope.local_len(), 0);
            assert_eq!(scope.bindings()[0].0.as_ptr(), name.as_ptr());
            assert_eq!(
                scope.get("outer").unwrap().ty,
                Type::Scalar(ScalarType::Integer)
            );
            let (_, ty) = lower_expression(
                &good.function.as_ref().unwrap().body,
                &mut scope,
                &mut visited,
                1,
            )
            .unwrap();
            assert_eq!(ty, Type::Scalar(ScalarType::Integer));
            assert_eq!(visited, spent + 3);
            assert_eq!(scope.len(), 1);
            assert_eq!(scope.local_len(), 0);
        }
    }
}
