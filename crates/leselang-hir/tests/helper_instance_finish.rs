use std::cell::RefCell;
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_body::*;
use leselang_hir::helper_instance::*;
use leselang_hir::helper_instance_finish::*;
use leselang_hir::helper_templates::HelperTemplateLimits;
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::*;
use leselang_hir::source_call::SourceCallLimits;
use leselang_hir::source_cost::*;
use leselang_runtime_core::*;

struct Field {
    key: u8,
    bytes: Vec<u8>,
    drops: Rc<RefCell<usize>>,
}
impl Drop for Field {
    fn drop(&mut self) {
        *self.drops.borrow_mut() += 1;
    }
}
struct Observation {
    bytes: Vec<u8>,
}
struct PrivateError(&'static str);
type Data = Computation<Field, u32, (), ()>;
const LIMITS: HelperInstanceLimits = HelperInstanceLimits {
    source: SourceCallLimits {
        max_source_nodes: 128,
        max_source_depth: 16,
        max_lowered_nodes: 128,
        max_lowered_depth: 16,
        max_arguments: 64,
    },
    max_parameters: 8,
    max_bindings: 8,
    max_reserved_names: 16,
};
fn integer(value: u64) -> Data {
    Data::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn local(name: &str) -> Data {
    Data::Local { name: name.into() }
}
fn add(value: Data) -> Data {
    Data::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(value),
        right: Box::new(integer(1)),
    }
}
fn operand(value: Data) -> HelperInstanceArgument<Data> {
    HelperInstanceArgument {
        value,
        scalar_type: Some(ScalarType::Integer),
    }
}
fn field(key: u8, drops: &Rc<RefCell<usize>>) -> Data {
    Data::Field {
        field: Field {
            key,
            bytes: vec![7; 16],
            drops: Rc::clone(drops),
        },
        value: Box::new(local("record")),
    }
}
fn body(parameters: usize, unused: bool) -> HelperBody<Data, Observation> {
    LoweredHelperBody {
        parameters: (0..parameters)
            .map(|index| (format!("p{index}"), ScalarType::Integer))
            .collect(),
        expression: if unused || parameters == 0 {
            integer(42)
        } else {
            add(local("p0"))
        },
        result_type: Observation { bytes: vec![3; 16] },
        scalar_result: Some(ScalarType::Integer),
    }
    .prepare(
        HelperBodyLimits {
            template: HelperTemplateLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 8,
                max_parameters: 8,
            },
            source_cost: SourceCostLimits {
                max_nodes: 128,
                max_depth: 16,
            },
        },
        |_, _, result, scalar| {
            assert_eq!(result.bytes.len(), 16);
            assert_eq!(scalar, Some(ScalarType::Integer));
            Ok::<_, Infallible>(())
        },
        |_| Ok::<_, Infallible>(SourceCostExtra { nodes: 0, depth: 0 }),
    )
    .unwrap()
}
struct Domain;
impl PureTypeEnvironment<Field, u32> for Domain {
    type Result = ();
    fn field_type(&self, _: &(), field: &Field) -> Option<ScalarType> {
        match field.key {
            1 => Some(ScalarType::Integer),
            2 => Some(ScalarType::String),
            _ => None,
        }
    }
    fn member_result(&self, _: &(), _: &str, _: &u32) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<Field, u32> for Domain {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), field: &Field) -> Result<ScalarValue, ()> {
        if field.key == 1 {
            Ok(ScalarValue::Integer(41))
        } else {
            Err(())
        }
    }
    fn member(&self, _: &(), _: &str, _: &u32) -> Result<(), ()> {
        Err(())
    }
}
fn infer(value: &Data) -> Result<PureType<()>, PureTypeError> {
    infer_pure_type(
        value,
        &[("record", PureType::Result(()))],
        &Domain,
        TypeInferenceLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 8,
        },
    )
}
fn copy(value: &Data) -> Result<Data, PrivateError> {
    match value {
        Data::Literal {
            value: ScalarValue::Integer(value),
        } => Ok(integer(*value)),
        Data::Local { name } => Ok(local(name)),
        Data::Binary {
            operator: BinaryOperator::Add,
            left,
            right,
        } => Ok(Data::Binary {
            operator: BinaryOperator::Add,
            left: Box::new(copy(left)?),
            right: Box::new(copy(right)?),
        }),
        _ => Err(PrivateError("unsupported native copy input")),
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Failure {
    None,
    Argument,
    ArgumentUnwind,
    Copy,
    CopyUnwind,
    Alias,
    Admit,
    AdmitUnwind,
}
struct Finisher {
    events: Vec<&'static str>,
    indices: Vec<usize>,
    names: usize,
    failure: Failure,
    extra_depth: Option<usize>,
    original: *const Observation,
}
impl Finisher {
    fn new(body: &HelperBody<Data, Observation>) -> Self {
        Self {
            events: vec![],
            indices: vec![],
            names: 0,
            failure: Failure::None,
            extra_depth: None,
            original: std::ptr::from_ref(body.template().result_type()),
        }
    }
}
impl HelperInstanceFinisher<Field, u32, (), (), Observation> for Finisher {
    type Error = PrivateError;
    fn admit_argument(
        &mut self,
        value: &Data,
        expected: ScalarType,
        index: usize,
    ) -> Result<(), PrivateError> {
        self.events.push("argument");
        self.indices.push(index);
        if self.failure == Failure::ArgumentUnwind {
            panic!("native argument unwind")
        }
        if self.failure == Failure::Argument
            || infer(value).ok() != Some(PureType::Scalar(expected))
        {
            return Err(PrivateError("private argument rejection"));
        }
        Ok(())
    }
    fn argument_depth(&mut self, value: &Data) -> Result<usize, PrivateError> {
        self.events.push("depth");
        if let Some(depth) = self.extra_depth {
            return Ok(depth);
        }
        measure_source_cost(
            value,
            SourceCostLimits {
                max_nodes: 128,
                max_depth: 16,
            },
            |_| Ok::<_, Infallible>(SourceCostExtra { nodes: 0, depth: 0 }),
        )
        .map(|cost| cost.depth)
        .map_err(|_| PrivateError("private depth rejection"))
    }
    fn materialize(&mut self, body: &Data) -> Result<Data, PrivateError> {
        self.events.push("copy");
        if self.failure == Failure::CopyUnwind {
            panic!("native copy unwind")
        }
        if self.failure == Failure::Copy {
            return Err(PrivateError("private copy rejection"));
        }
        copy(body)
    }
    fn fresh_name(&mut self) -> Result<String, PrivateError> {
        self.events.push("alias");
        if self.failure == Failure::Alias {
            return Err(PrivateError("private alias rejection"));
        }
        let name = format!("alias{}", self.names);
        self.names += 1;
        Ok(name)
    }
    fn admit(&mut self, value: &Data, result: &Observation) -> Result<(), PrivateError> {
        self.events.push("admit");
        assert!(std::ptr::eq(result, self.original));
        assert_eq!(result.bytes.len(), 16);
        if self.failure == Failure::AdmitUnwind {
            panic!("native admission unwind")
        }
        if self.failure == Failure::Admit
            || infer(value).ok() != Some(PureType::Scalar(ScalarType::Integer))
        {
            return Err(PrivateError("private whole-output rejection"));
        }
        Ok(())
    }
}
fn finish<'body>(
    body: &'body HelperBody<Data, Observation>,
    arguments: Vec<HelperInstanceArgument<Data>>,
    limits: HelperInstanceLimits,
    counter: &mut usize,
    adapter: &mut Finisher,
) -> HelperInstanceResult<'body, Data, Observation, PrivateError> {
    finish_helper_instance(
        SelectedHelper {
            name: "bump",
            body,
            reserved_names: &["record", "unused_caller"],
            caller_depth: 0,
        },
        arguments,
        limits,
        counter,
        adapter,
    )
}
fn denied<Node, Error>(
    result: Result<Node, HelperInstanceError<Error>>,
) -> HelperInstanceError<Error> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected staged instance denial"),
    }
}

#[test]
fn already_compiled_operands_share_the_original_counter_and_once_finish_order() {
    let cached = body(1, false);
    let mut adapter = Finisher::new(&cached);
    let mut counter = 2;
    let (output, observation) = finish(
        &cached,
        vec![operand(integer(41))],
        LIMITS,
        &mut counter,
        &mut adapter,
    )
    .unwrap();
    assert_eq!(counter, 6);
    assert_eq!(
        adapter.events,
        ["argument", "depth", "copy", "alias", "admit"]
    );
    assert_eq!(adapter.indices, [0]);
    assert!(std::ptr::eq(observation, cached.template().result_type()));
    let mut fuel = Fuel::new(100);
    let mut prefix = vec![("record", PureValue::Result(()))];
    let result = evaluate_pure_in_scope(
        &output,
        &mut ScopeFrame::new(&mut prefix),
        &Domain,
        &mut fuel,
        PureEvaluationLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 8,
        },
    )
    .unwrap();
    assert!(matches!(
        result,
        PureValue::Scalar(ScalarValue::Integer(42))
    ));
    assert_eq!(prefix.len(), 1);
    assert_eq!(fuel.remaining(), 95);
}

#[test]
fn cached_and_combined_limits_precede_operand_native_hooks_or_counter_changes() {
    let cached = body(1, false);
    for limits in [
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 4,
                ..LIMITS.source
            },
            ..LIMITS
        },
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 1,
                ..LIMITS.source
            },
            ..LIMITS
        },
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_source_nodes: 16_385,
                ..LIMITS.source
            },
            ..LIMITS
        },
        HelperInstanceLimits {
            max_reserved_names: 1,
            ..LIMITS
        },
    ] {
        let mut counter = 2;
        let mut adapter = Finisher::new(&cached);
        assert!(
            finish(
                &cached,
                vec![operand(integer(41))],
                limits,
                &mut counter,
                &mut adapter
            )
            .is_err()
        );
        assert!(adapter.events.is_empty());
        assert_eq!(counter, 2);
    }
    let mut counter = 2;
    let mut adapter = Finisher::new(&cached);
    let limits = HelperInstanceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 5,
            max_lowered_depth: 2,
            max_source_nodes: 6,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(
        finish(
            &cached,
            vec![operand(integer(41))],
            limits,
            &mut counter,
            &mut adapter
        )
        .is_ok()
    );
    assert_eq!(counter, 6);
}

#[test]
fn argument_count_and_all_cold_scalar_facts_precede_every_native_hook() {
    let cached = body(2, true);
    for arguments in [
        vec![operand(integer(1))],
        vec![
            operand(integer(1)),
            operand(Data::Literal {
                value: ScalarValue::Boolean(true),
            }),
        ],
        vec![
            operand(integer(1)),
            HelperInstanceArgument {
                value: integer(2),
                scalar_type: None,
            },
        ],
    ] {
        let mut counter = 3;
        let mut adapter = Finisher::new(&cached);
        let error = denied(finish(
            &cached,
            arguments,
            LIMITS,
            &mut counter,
            &mut adapter,
        ));
        assert!(matches!(
            error,
            HelperInstanceError::ArgumentCount | HelperInstanceError::Argument { index: 1, .. }
        ));
        assert!(adapter.events.is_empty());
        assert_eq!(counter, 3);
    }
}

#[test]
fn a_complete_cold_argument_forest_is_bounded_before_any_native_type_observation() {
    let cached = body(2, true);
    let oversized = (0..8).fold(integer(1), |value, _| add(value));
    let mut counter = 20;
    let mut adapter = Finisher::new(&cached);
    let limits = HelperInstanceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 16,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(
        finish(
            &cached,
            vec![operand(integer(1)), operand(oversized)],
            limits,
            &mut counter,
            &mut adapter
        )
        .is_err()
    );
    assert!(adapter.events.is_empty());
    assert_eq!(counter, 20);
}

#[test]
fn forged_unused_native_argument_type_is_rejected_before_depth_copy_or_commit() {
    let cached = body(1, true);
    let drops = Rc::new(RefCell::new(0));
    let mut counter = 3;
    let mut adapter = Finisher::new(&cached);
    let error = denied(finish(
        &cached,
        vec![operand(field(2, &drops))],
        LIMITS,
        &mut counter,
        &mut adapter,
    ));
    assert!(matches!(
        error,
        HelperInstanceError::Native {
            phase: HelperInstancePhase::Argument { index: 0 },
            ..
        }
    ));
    assert_eq!(adapter.events, ["argument"]);
    assert_eq!(counter, 3);
    assert_eq!(*drops.borrow(), 1);
}

#[test]
fn original_move_only_operand_buffer_and_cached_result_observation_are_retained() {
    let cached = body(1, false);
    let drops = Rc::new(RefCell::new(0));
    let argument = field(1, &drops);
    let Data::Field { field, value } = &argument else {
        panic!()
    };
    let bytes = field.bytes.as_ptr();
    let local_ptr = value.as_ref() as *const Data;
    let result_bytes = cached.template().result_type().bytes.as_ptr();
    let mut counter = 3;
    let mut adapter = Finisher::new(&cached);
    let (output, observation) = finish(
        &cached,
        vec![operand(argument)],
        LIMITS,
        &mut counter,
        &mut adapter,
    )
    .unwrap();
    let Data::Bind { value, .. } = &output else {
        panic!()
    };
    let Data::Field { field, value } = value.as_ref() else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), bytes);
    assert_eq!(value.as_ref() as *const Data, local_ptr);
    assert_eq!(observation.bytes.as_ptr(), result_bytes);
    assert!(std::ptr::eq(observation, cached.template().result_type()));
    assert_eq!(*drops.borrow(), 0);
    drop(output);
    assert_eq!(*drops.borrow(), 1);
    assert_eq!(cached.template().result_type().bytes.as_ptr(), result_bytes);
}

#[test]
fn native_argument_error_and_unwind_release_operands_without_copy_or_commit() {
    let cached = body(1, false);
    for failure in [Failure::Argument, Failure::ArgumentUnwind] {
        let drops = Rc::new(RefCell::new(0));
        let mut counter = 3;
        let mut adapter = Finisher::new(&cached);
        adapter.failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| {
            finish(
                &cached,
                vec![operand(field(1, &drops))],
                LIMITS,
                &mut counter,
                &mut adapter,
            )
        }));
        if failure == Failure::ArgumentUnwind {
            assert!(result.is_err());
        } else {
            let error = denied(result.unwrap());
            assert!(!format!("{error:?} {error}").contains("private argument rejection"));
            assert!(std::error::Error::source(&error).is_none());
            let HelperInstanceError::Native {
                error,
                phase: HelperInstancePhase::Argument { index: 0 },
            } = error
            else {
                panic!()
            };
            assert_eq!(error.0, "private argument rejection");
        }
        assert_eq!(adapter.events, ["argument"]);
        assert_eq!(counter, 3);
        assert_eq!(*drops.borrow(), 1);
    }
}

#[test]
fn all_native_operand_types_are_admitted_before_any_depth_query_or_copy() {
    let cached = body(2, true);
    let mut counter = 3;
    let mut adapter = Finisher::new(&cached);
    finish(
        &cached,
        vec![operand(integer(1)), operand(integer(2))],
        LIMITS,
        &mut counter,
        &mut adapter,
    )
    .unwrap();
    assert_eq!(adapter.indices, [0, 1]);
    assert_eq!(
        adapter.events,
        [
            "argument", "argument", "depth", "depth", "copy", "alias", "alias", "admit"
        ]
    );
    assert_eq!(counter, 6);
}

#[test]
fn source_depth_overflow_and_insufficient_budget_leave_the_same_counter_unchanged() {
    let cached = body(1, false);
    for (start, depth, limits) in [
        (2, Some(usize::MAX), LIMITS),
        (
            2,
            None,
            HelperInstanceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 5,
                    ..LIMITS.source
                },
                ..LIMITS
            },
        ),
        (129, None, LIMITS),
    ] {
        let mut counter = start;
        let mut adapter = Finisher::new(&cached);
        adapter.extra_depth = depth;
        assert!(
            finish(
                &cached,
                vec![operand(integer(41))],
                limits,
                &mut counter,
                &mut adapter
            )
            .is_err()
        );
        assert_eq!(counter, start);
        assert!(!adapter.events.contains(&"copy"));
    }
}

#[test]
fn later_copy_alias_and_admission_error_or_unwind_never_refund_committed_expansion() {
    let cached = body(1, false);
    for failure in [
        Failure::Copy,
        Failure::CopyUnwind,
        Failure::Alias,
        Failure::Admit,
        Failure::AdmitUnwind,
    ] {
        let drops = Rc::new(RefCell::new(0));
        let mut counter = 3;
        let mut adapter = Finisher::new(&cached);
        adapter.failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| {
            finish(
                &cached,
                vec![operand(field(1, &drops))],
                LIMITS,
                &mut counter,
                &mut adapter,
            )
        }));
        match result {
            Ok(result) => assert!(result.is_err()),
            Err(_) => assert!(matches!(
                failure,
                Failure::CopyUnwind | Failure::AdmitUnwind
            )),
        }
        assert_eq!(counter, 7);
        assert_eq!(*drops.borrow(), 1);
        assert_eq!(
            adapter
                .events
                .iter()
                .filter(|event| **event == "copy")
                .count(),
            1
        );
        assert!(
            adapter
                .events
                .iter()
                .filter(|event| **event == "admit")
                .count()
                <= 1
        );
    }
}

#[test]
fn zero_parameter_completion_still_copies_and_admits_once_without_operand_hooks() {
    let cached = body(0, true);
    let mut counter = 1;
    let mut adapter = Finisher::new(&cached);
    let (output, _) = finish(&cached, vec![], LIMITS, &mut counter, &mut adapter).unwrap();
    assert!(matches!(
        output,
        Data::Literal {
            value: ScalarValue::Integer(42)
        }
    ));
    assert_eq!(adapter.events, ["copy", "admit"]);
    assert_eq!(counter, 2);
}

#[test]
fn reference_nested_same_helper_operands_groups_and_unused_types_keep_legacy_contracts() {
    let sources = [
        "fn bump(n: integer) = add(left: n, right: 1)\nfn main() = bump(n: bump(n: 40))",
        "fn read(text: string) = text\nfn main() = bind(group: seq(a: ui.assert_text(node_id: \"status\", expected: \"ok\")), body: ui.set_form_value(node_id: \"form\", field: \"value\", value: read(text: field(value: member(value: group, name: \"a\"), name: \"expected\"))))",
        "fn ignore(n: integer) = 42\nfn main() = ignore(n: 1)",
    ];
    for source in sources {
        let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        let restored = leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
    }
    let invalid = leselang_hir::lower(&leselang_syntax::parse(
        "fn ignore(n: integer) = 42\nfn main() = ignore(n: \"wrong\")",
    ))
    .unwrap_err();
    assert!(
        invalid
            .iter()
            .any(|diagnostic| diagnostic.code == "LSH1504" && diagnostic.span.is_some())
    );
}

#[test]
fn reference_deep_fold_prefix_uses_binding_quota_not_expression_root_depth() {
    let mut source = "constant()".to_owned();
    for index in 0..9 {
        source = format!(
            "fold(s{index}: 0, items: strings(), item: \"i{index}\", next: {source}, limit: 0)"
        );
    }
    let source = format!("fn constant() = 0\nfn main() = {source}");
    let program = leselang_hir::lower(&leselang_syntax::parse(&source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let restored = leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
    let residual = leselang_hir::computation::Computation::Literal {
        value: ScalarValue::Integer(0),
    };
    let prefix = (0..18)
        .map(|index| {
            (
                format!("v{index}"),
                leselang_hir::Type::Scalar(ScalarType::Integer),
            )
        })
        .collect::<Vec<_>>();
    assert!(residual.validate_in_scope(&prefix).is_err());
}

#[test]
fn reference_full_width_caller_group_keeps_exact_last_member_signature() {
    let branches = (0..64)
        .map(|index| format!("b{index}: ui.assert_text(node_id: \"status\", expected: \"ok\")"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "fn echo(text: string) = text\nfn main() = bind(group: seq({branches}), body: echo(text: field(value: member(value: group, name: \"b63\"), name: \"expected\")))"
    );
    let program = leselang_hir::lower(&leselang_syntax::parse(&source)).unwrap();
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let restored = leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
}
