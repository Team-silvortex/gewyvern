use std::cell::Cell;
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::helper_source::*;
use leselang_hir::helper_templates::{HelperTemplate, HelperTemplateLimits};
use leselang_hir::ir::Computation;
use leselang_hir::source_call::*;
use leselang_runtime_core::*;
use leselang_syntax::{Expression, NamedArgument, parse};

const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 64,
    max_source_depth: 8,
    max_lowered_nodes: 64,
    max_lowered_depth: 8,
    max_arguments: 4,
};
const HELPER: HelperSourceLimits = HelperSourceLimits {
    source: LIMITS,
    max_parameters: 4,
};
struct NativeError<'a>(&'a str);
// These borrowed native fields and schemas intentionally need no Clone/Debug/serde/Send.
struct Field<'a> {
    bytes: &'a [u8],
    drops: &'a Cell<usize>,
}
impl Drop for Field<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Opaque;
type Data<'a> = Computation<Field<'a>, Opaque, Opaque, Opaque>;
fn source(text: &str) -> Expression {
    let parsed = parse(text);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn args(expression: &Expression) -> &[NamedArgument] {
    let Expression::Call { arguments, .. } = expression else {
        panic!()
    };
    arguments
}
fn integer<'a>() -> (Data<'a>, Option<ScalarType>) {
    (
        Data::Literal {
            value: ScalarValue::Integer(7),
        },
        Some(ScalarType::Integer),
    )
}
fn field<'a>(bytes: &'a [u8], drops: &'a Cell<usize>) -> Data<'a> {
    Data::Field {
        value: Box::new(Data::Local {
            name: "original".into(),
        }),
        field: Field { bytes, drops },
    }
}
struct Domain<'a> {
    allowed: &'a Cell<bool>,
    queries: &'a Cell<usize>,
}
impl ScalarArgumentDomain for Domain<'_> {
    fn scalar_types(&self) -> ScalarTypeSet {
        self.queries.set(self.queries.get() + 1);
        ScalarTypeSet::only(ScalarType::Integer)
    }
    fn accepts_literal(&self, _: &ScalarValue) -> bool {
        self.queries.set(self.queries.get() + 1);
        self.allowed.get()
    }
}

#[test]
fn helper_binding_lowers_original_arguments_in_declaration_order_without_copying_native_slots() {
    let input = source("fn main() = helper(b: 2, a: 1)");
    let arguments = args(&input);
    let parameters = [
        NamedParameter::required("a", ScalarType::Integer),
        NamedParameter::required("b", ScalarType::Integer),
    ];
    let bound = bind_helper_source_arguments::<Infallible>(arguments, &parameters, HELPER).unwrap();
    assert_eq!(format!("{bound:?}"), "BoundHelperSourceArguments");
    let bytes = vec![3; 31];
    let drops = Cell::new(0);
    let mut order = vec![];
    let lowered = bound
        .lower(|arg| {
            order.push(arg.name.as_str());
            Ok::<_, Infallible>((field(&bytes, &drops), Some(ScalarType::Integer)))
        })
        .unwrap();
    assert_eq!(order, ["a", "b"]);
    for (parameter_index, argument_index) in [(0, 1), (1, 0)] {
        let row = &lowered[parameter_index];
        assert_eq!(
            (row.parameter_index, row.argument_index),
            (parameter_index, argument_index)
        );
        assert!(std::ptr::eq(row.parameter, &parameters[parameter_index]));
        assert!(std::ptr::eq(row.argument, &arguments[argument_index]));
        let Data::Field { field, .. } = &row.value else {
            panic!()
        };
        assert_eq!(field.bytes.as_ptr(), bytes.as_ptr());
    }
    assert_eq!(drops.get(), 0);
    drop(lowered);
    assert_eq!(drops.get(), 2);
}

#[test]
fn binding_does_not_require_native_domain_traits_or_query_host_metadata() {
    let input = source("fn main() = untrusted(a: 1)");
    let parameters = [NamedParameter::required("a", Opaque)];
    let schema = SourceSchema {
        key: Opaque,
        parameters: &parameters,
        result: Opaque,
        required_capability: Opaque,
    };
    let bound =
        bind_source_arguments::<_, _, _, _, Infallible>(args(&input), &schema, LIMITS).unwrap();
    assert_eq!(format!("{bound:?}"), "BoundSourceArguments");
    drop(bound);
}

#[test]
fn delayed_host_lowering_rechecks_current_domain_and_preserves_optional_omission() {
    let input = source("fn main() = host(a: 1)");
    let allowed = Cell::new(true);
    let queries = Cell::new(0);
    let parameters = [
        NamedParameter::required(
            "a",
            Domain {
                allowed: &allowed,
                queries: &queries,
            },
        ),
        NamedParameter::optional(
            "omitted",
            Domain {
                allowed: &allowed,
                queries: &queries,
            },
        ),
    ];
    let schema = SourceSchema {
        key: Opaque,
        parameters: &parameters,
        result: Opaque,
        required_capability: Opaque,
    };
    let bound =
        bind_source_arguments::<_, _, _, _, Infallible>(args(&input), &schema, LIMITS).unwrap();
    assert_eq!(queries.get(), 0);
    allowed.set(false);
    let mut calls = 0;
    let result = bound.lower(|arg| {
        calls += 1;
        assert!(std::ptr::eq(arg, &args(&input)[0]));
        Ok::<_, Infallible>(integer())
    });
    assert!(matches!(
        result,
        Err(SourceCallError::Argument {
            parameter_index: 0,
            argument_index: 0,
            ..
        })
    ));
    assert_eq!((calls, queries.get()), (1, 2));
    allowed.set(true);
    let lowered = bind_source_arguments::<_, _, _, _, Infallible>(args(&input), &schema, LIMITS)
        .unwrap()
        .lower(|_| Ok::<_, Infallible>(integer()))
        .unwrap();
    assert!(std::ptr::eq(lowered.schema(), &schema));
    assert_eq!(lowered.arguments().len(), 1);
    assert_eq!(queries.get(), 4);
}

#[test]
fn all_top_level_names_and_count_limits_precede_lowering_or_native_domains() {
    let allowed = Cell::new(true);
    let queries = Cell::new(0);
    let parameters = [NamedParameter::required(
        "a",
        Domain {
            allowed: &allowed,
            queries: &queries,
        },
    )];
    let schema = SourceSchema {
        key: Opaque,
        parameters: &parameters,
        result: Opaque,
        required_capability: Opaque,
    };
    for text in [
        "fn main() = host(x: 1)",
        "fn main() = host()",
        "fn main() = host(a: 1, b: 2)",
    ] {
        let input = source(text);
        assert!(
            bind_source_arguments::<_, _, _, _, Infallible>(args(&input), &schema, LIMITS).is_err()
        );
    }
    let input = source("fn main() = host(a: 1)");
    assert!(matches!(
        bind_source_arguments::<_, _, _, _, Infallible>(
            args(&input),
            &schema,
            SourceCallLimits {
                max_arguments: 65,
                ..LIMITS
            }
        ),
        Err(SourceCallError::InvalidLimits)
    ));
    assert_eq!(queries.get(), 0);
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    assert!(matches!(
        bind_helper_source_arguments::<Infallible>(
            args(&input),
            &parameters,
            HelperSourceLimits {
                max_parameters: 9,
                ..HELPER
            }
        ),
        Err(HelperSourceError::InvalidLimits)
    ));
}

#[test]
fn named_binding_is_not_a_cold_source_receipt_and_strict_source_entry_stays_strict() {
    let input = source("fn main() = helper(a: nested(value: 1))");
    let parameters = [NamedParameter::required("a", ScalarType::Integer)];
    let limits = HelperSourceLimits {
        source: SourceCallLimits {
            max_source_nodes: 0,
            ..LIMITS
        },
        ..HELPER
    };
    let bound =
        bind_helper_source_arguments::<Infallible>(args(&input), &parameters, limits).unwrap();
    let message = String::from("host must reject cold source under its own policy");
    let result = bound.lower(|_| Err::<(Data<'_>, Option<ScalarType>), _>(NativeError(&message)));
    let Err(HelperSourceError::Lowering { error, .. }) = result else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), message.as_ptr());
    let strict = lower_helper_arguments(
        &input,
        HelperSignature {
            name: "helper",
            parameters: &parameters,
        },
        limits,
        |_| -> Result<(Data<'_>, Option<ScalarType>), Infallible> {
            panic!("strict source check must run first")
        },
    );
    assert!(matches!(
        strict,
        Err(HelperSourceError::Shape {
            error: SourceShapeError::Structure(_),
            ..
        })
    ));
}

#[test]
fn later_native_error_or_unwind_drops_prior_values_without_refunding_native_counter() {
    let input = source("fn main() = helper(b: 2, a: 1)");
    let parameters = [
        NamedParameter::required("a", ScalarType::Integer),
        NamedParameter::required("b", ScalarType::Integer),
    ];
    let bytes = vec![5; 17];
    for unwind in [false, true] {
        let drops = Cell::new(0);
        let count = Cell::new(0);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let bound =
                bind_helper_source_arguments::<Infallible>(args(&input), &parameters, HELPER)
                    .unwrap();
            bound.lower(|arg| {
                count.set(count.get() + 1);
                if arg.name == "b" {
                    assert!(!unwind, "native unwind");
                    return Err(NativeError("later"));
                }
                Ok((field(&bytes, &drops), Some(ScalarType::Integer)))
            })
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            let Ok(Err(HelperSourceError::Lowering {
                argument_index: 0,
                error,
            })) = outcome
            else {
                panic!()
            };
            assert_eq!(error.0, "later");
        }
        assert_eq!((count.get(), drops.get()), (2, 1));
    }
}

#[test]
fn output_root_budget_precedes_host_lowering_and_empty_helper_has_no_call_root() {
    let parameters: [NamedParameter<&str, ScalarTypeSet>; 0] = [];
    let schema = SourceSchema {
        key: Opaque,
        parameters: &parameters,
        result: Opaque,
        required_capability: Opaque,
    };
    let limits = SourceCallLimits {
        max_lowered_nodes: 0,
        ..LIMITS
    };
    let bound = bind_source_arguments::<_, _, _, _, Infallible>(&[], &schema, limits).unwrap();
    let result =
        bound.lower(|_| -> Result<(Data<'_>, Option<ScalarType>), Infallible> { panic!() });
    assert!(matches!(
        result,
        Err(SourceCallError::OutputRoot(StructureError::NodeLimit))
    ));
    let helper = bind_helper_source_arguments::<Infallible>(
        &[],
        &[],
        HelperSourceLimits {
            source: limits,
            ..HELPER
        },
    )
    .unwrap();
    assert!(
        helper
            .lower(|_| -> Result<(Data<'_>, Option<ScalarType>), Infallible> { panic!() })
            .unwrap()
            .is_empty()
    );
}

#[test]
fn cached_template_revalidation_uses_current_bounds_without_copying_native_fields() {
    let bytes = vec![9; 19];
    let drops = Cell::new(0);
    let limits = HelperTemplateLimits {
        max_nodes: 2,
        max_depth: 1,
        max_bindings: 1,
        max_parameters: 1,
    };
    let template = HelperTemplate::new(
        vec![("original".into(), ScalarType::Integer)],
        field(&bytes, &drops),
        Opaque,
        limits,
    )
    .unwrap();
    let before = template.body() as *const _;
    assert_eq!(template.revalidate(limits).unwrap(), template.shape());
    for tighter in [
        HelperTemplateLimits {
            max_nodes: 1,
            ..limits
        },
        HelperTemplateLimits {
            max_depth: 0,
            ..limits
        },
        HelperTemplateLimits {
            max_bindings: 0,
            ..limits
        },
        HelperTemplateLimits {
            max_parameters: 0,
            ..limits
        },
    ] {
        assert!(template.revalidate(tighter).is_err());
    }
    assert_eq!(template.revalidate(limits).unwrap(), template.shape());
    assert_eq!(template.body() as *const _, before);
    let Data::Field { field, .. } = template.body() else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), bytes.as_ptr());
    assert_eq!(drops.get(), 0);
    drop(template);
    assert_eq!(drops.get(), 1);
}
