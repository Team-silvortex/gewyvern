use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::bound_projection_source::*;
use leselang_hir::ir::Computation;
use leselang_hir::projection_source::ProjectionSourceError;
use leselang_hir::pure_typing::PureType;
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{ScalarType, StructureError};
use leselang_syntax::{Expression, parse};

struct Token {
    buffer: Box<[u8]>,
    owner: Rc<()>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Declaration {
    token: Token,
    group: bool,
}
struct PrivateError(&'static str);
type Node = Computation<Token, Token, Token, Token>;
const LIMITS: BoundProjectionSourceLimits = BoundProjectionSourceLimits {
    source: SourceCallLimits {
        max_source_nodes: 256,
        max_source_depth: 32,
        max_lowered_nodes: 256,
        max_lowered_depth: 32,
        max_arguments: 64,
    },
    max_bindings: 8,
};
const TYPES: [ScalarType; 6] = [
    ScalarType::Integer,
    ScalarType::Boolean,
    ScalarType::String,
    ScalarType::None,
    ScalarType::OptionalString,
    ScalarType::StringList,
];
struct Host {
    owner: Rc<()>,
    drops: Rc<Cell<usize>>,
    names: [&'static str; 6],
    queries: Vec<(*const Declaration, *const u8, String)>,
    slots: Vec<*const u8>,
    fail: bool,
    unwind: bool,
}
impl Host {
    fn new(names: [&'static str; 6]) -> Self {
        Self {
            owner: Rc::new(()),
            drops: Rc::new(Cell::new(0)),
            names,
            queries: Vec::new(),
            slots: Vec::new(),
            fail: false,
            unwind: false,
        }
    }
    fn token(&self) -> Token {
        Token {
            buffer: vec![7; 33].into_boxed_slice(),
            owner: self.owner.clone(),
            drops: self.drops.clone(),
        }
    }
    fn declaration(&self, group: bool) -> Declaration {
        Declaration {
            token: self.token(),
            group,
        }
    }
    fn step(&mut self, result: &Declaration, name: &str) -> Result<(), PrivateError> {
        self.queries.push((result, name.as_ptr(), name.into()));
        assert!(!self.unwind, "private native query panic");
        if self.fail {
            Err(PrivateError("private export credential"))
        } else {
            Ok(())
        }
    }
}
impl<'source> BoundProjectionSourceEnvironment<'source, Token, Token> for Host {
    type Result = Declaration;
    type Error = PrivateError;
    fn field(
        &mut self,
        result: &Declaration,
        name: &'source str,
    ) -> Result<Option<(Token, ScalarType)>, PrivateError> {
        self.step(result, name)?;
        if result.group || !Rc::ptr_eq(&result.token.owner, &self.owner) {
            return Ok(None);
        }
        let Some(index) = self.names.iter().position(|bound| *bound == name) else {
            return Ok(None);
        };
        let field = self.token();
        self.slots.push(field.buffer.as_ptr());
        Ok(Some((field, TYPES[index])))
    }
    fn member(
        &mut self,
        result: &Declaration,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Token, Declaration)>, PrivateError> {
        self.step(result, name)?;
        if !result.group
            || !Rc::ptr_eq(&result.token.owner, &self.owner)
            || group != "batch"
            || name != "ready-step"
        {
            return Ok(None);
        }
        let operation = self.token();
        let result = self.declaration(false);
        self.slots
            .extend([operation.buffer.as_ptr(), result.token.buffer.as_ptr()]);
        Ok(Some((operation, result)))
    }
}
fn source(text: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {text}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn lower(
    input: &Expression,
    prefix: &[(&str, &PureType<Declaration>)],
    limits: BoundProjectionSourceLimits,
    host: &mut Host,
) -> BoundProjectionSourceResult<Node, Declaration, PrivateError> {
    lower_bound_projection_source(input, prefix, limits, host)
}

#[test]
fn two_unrelated_schemas_borrow_original_result_metadata_and_export_all_six_scalars() {
    for names in [
        ["size", "open", "caption", "empty", "hint", "entries"],
        ["count", "live", "label", "unused", "maybe", "list"],
    ] {
        let mut host = Host::new(names);
        let observed = PureType::Result(host.declaration(false));
        let PureType::Result(declaration) = &observed else {
            panic!()
        };
        let ptr = declaration as *const _;
        let original = declaration.token.buffer.as_ptr();
        for (index, name) in names.into_iter().enumerate() {
            let input = source(&format!("field(name: \"{name}\", value: record)"));
            let (node, ty) = lower(&input, &[("record", &observed)], LIMITS, &mut host).unwrap();
            assert!(matches!(ty, PureType::Scalar(actual) if actual == TYPES[index]));
            let Node::Field { value, field } = node else {
                panic!()
            };
            assert!(matches!(value.as_ref(), Node::Local { name } if name == "record"));
            assert_eq!(field.buffer.as_ptr(), *host.slots.last().unwrap());
            assert_eq!(host.queries.last().unwrap().0, ptr);
            let Expression::Call { arguments, .. } = &input else {
                panic!()
            };
            let Expression::String {
                value: original_name,
                ..
            } = &arguments[0].value
            else {
                panic!()
            };
            assert_eq!(host.queries.last().unwrap().1, original_name.as_ptr());
            assert_eq!(declaration.token.buffer.as_ptr(), original);
            drop((value, field));
        }
        assert_eq!(host.queries.len(), 6);
        assert_eq!(
            host.drops.get(),
            6,
            "borrowed declaration must remain owned by the prefix"
        );
        drop(observed);
        assert_eq!(host.drops.get(), 7);
    }
}

#[test]
fn bound_group_member_uses_exact_group_observation_and_moves_returned_slots() {
    let mut host = Host::new(["a", "b", "c", "d", "e", "f"]);
    let observed = PureType::Result(host.declaration(true));
    let PureType::Result(group) = &observed else {
        panic!()
    };
    let (node, ty) = lower(
        &source("member(value: batch, name: \"ready-step\")"),
        &[("batch", &observed)],
        LIMITS,
        &mut host,
    )
    .unwrap();
    assert_eq!(host.queries[0].0, group as *const _);
    let Node::Member {
        group,
        name,
        operation,
    } = node
    else {
        panic!()
    };
    assert_eq!(group, "batch");
    assert_eq!(name, "ready-step");
    assert_eq!(operation.buffer.as_ptr(), host.slots[0]);
    let PureType::Result(member) = ty else {
        panic!()
    };
    assert_eq!(member.token.buffer.as_ptr(), host.slots[1]);
    assert_eq!(host.drops.get(), 0);
    drop((operation, member));
    assert_eq!(host.drops.get(), 2);
    drop(observed);
    assert_eq!(host.drops.get(), 3);
}

#[test]
fn missing_scalar_foreign_and_non_group_bindings_cannot_launder_exports_by_name() {
    let mut host = Host::new(["size", "open", "caption", "empty", "hint", "entries"]);
    let other = Host::new(host.names);
    let scalar = PureType::Scalar(ScalarType::Integer);
    let foreign = PureType::Result(other.declaration(false));
    let record = PureType::Result(host.declaration(false));
    assert!(matches!(
        lower(
            &source("field(value: record, name: \"size\")"),
            &[],
            LIMITS,
            &mut host
        ),
        Err(BoundProjectionSourceError::Unbound { .. })
    ));
    assert!(matches!(
        lower(
            &source("field(value: record, name: \"size\")"),
            &[("record", &scalar)],
            LIMITS,
            &mut host
        ),
        Err(BoundProjectionSourceError::Projection(
            ProjectionSourceError::NonResult { .. }
        ))
    ));
    assert!(host.queries.is_empty());
    assert!(matches!(
        lower(
            &source("field(value: record, name: \"size\")"),
            &[("record", &foreign)],
            LIMITS,
            &mut host
        ),
        Err(BoundProjectionSourceError::Projection(
            ProjectionSourceError::FieldNotExported { .. }
        ))
    ));
    assert!(matches!(
        lower(
            &source("member(value: batch, name: \"ready-step\")"),
            &[("batch", &record)],
            LIMITS,
            &mut host
        ),
        Err(BoundProjectionSourceError::Projection(
            ProjectionSourceError::MemberNotExported { .. }
        ))
    ));
    assert_eq!(host.queries.len(), 2);
    assert!(host.slots.is_empty());
}

#[test]
fn malformed_cold_projection_metadata_precedes_all_native_queries() {
    let mut host = Host::new(["size", "open", "caption", "empty", "hint", "entries"]);
    let observed = PureType::Result(host.declaration(false));
    for text in [
        "field(value: record)",
        "field(value: record, name: 1)",
        "field(value: record, name: \"size\", extra: 1)",
        "member(value: native.read(), name: \"ready-step\")",
        "member(value: batch, name: \"bad name\")",
        "field(value: choose(when: true, then: record, otherwise: field(value: record, name: 1)), name: \"size\")",
    ] {
        assert!(
            lower(&source(text), &[("record", &observed)], LIMITS, &mut host).is_err(),
            "{text}"
        );
        assert!(host.queries.is_empty());
    }
}

#[test]
fn bound_field_rejects_complex_inputs_without_implicit_lowering_or_name_lookup() {
    let mut host = Host::new(["size", "open", "caption", "empty", "hint", "entries"]);
    let observed = PureType::Result(host.declaration(false));
    assert!(matches!(
        lower(
            &source(
                "field(value: choose(when: true, then: record, otherwise: record), name: \"size\")"
            ),
            &[("record", &observed)],
            LIMITS,
            &mut host
        ),
        Err(BoundProjectionSourceError::BoundReference { .. })
    ));
    assert!(host.queries.is_empty());
}

#[test]
fn prefix_validity_quota_and_all_minimum_source_output_limits_precede_native_queries() {
    let mut host = Host::new(["size", "open", "caption", "empty", "hint", "entries"]);
    let observed = PureType::Result(host.declaration(false));
    let input = source("field(value: record, name: \"size\")");
    for prefix in [
        vec![("bad-name", &observed)],
        vec![("record", &observed), ("record", &observed)],
    ] {
        assert!(lower(&input, &prefix, LIMITS, &mut host).is_err());
    }
    for limits in [
        BoundProjectionSourceLimits {
            max_bindings: 0,
            ..LIMITS
        },
        BoundProjectionSourceLimits {
            max_bindings: 1025,
            ..LIMITS
        },
        BoundProjectionSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BoundProjectionSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BoundProjectionSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 1,
                ..LIMITS.source
            },
            ..LIMITS
        },
        BoundProjectionSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 0,
                ..LIMITS.source
            },
            ..LIMITS
        },
    ] {
        assert!(lower(&input, &[("record", &observed)], limits, &mut host).is_err());
    }
    assert!(host.queries.is_empty());
}

#[test]
fn inclusive_field_two_node_one_depth_and_member_one_node_zero_depth_bounds_are_exact() {
    let mut host = Host::new(["size", "open", "caption", "empty", "hint", "entries"]);
    let observed = PureType::Result(host.declaration(false));
    let limits = BoundProjectionSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 2,
            max_lowered_depth: 1,
            ..LIMITS.source
        },
        max_bindings: 1,
    };
    let output = lower(
        &source("field(value: record, name: \"size\")"),
        &[("record", &observed)],
        limits,
        &mut host,
    )
    .unwrap();
    drop(output);
    let group = PureType::Result(host.declaration(true));
    let limits = BoundProjectionSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 1,
            max_lowered_depth: 0,
            ..LIMITS.source
        },
        max_bindings: 1,
    };
    let (node, _) = lower(
        &source("member(value: batch, name: \"ready-step\")"),
        &[("batch", &group)],
        limits,
        &mut host,
    )
    .unwrap();
    assert!(matches!(node, Node::Member { .. }));
    let limits = BoundProjectionSourceLimits {
        source: SourceCallLimits {
            max_lowered_nodes: 0,
            ..LIMITS.source
        },
        ..LIMITS
    };
    assert!(matches!(
        lower(
            &source("member(value: batch, name: \"ready-step\")"),
            &[("batch", &group)],
            limits,
            &mut host
        ),
        Err(BoundProjectionSourceError::Projection(
            ProjectionSourceError::Generation {
                error: StructureError::NodeLimit,
                ..
            }
        ))
    ));
}

#[test]
fn native_error_and_unwind_do_not_copy_drop_or_requery_borrowed_metadata() {
    for member in [false, true] {
        for panic in [false, true] {
            let mut host = Host::new(["size", "open", "caption", "empty", "hint", "entries"]);
            let observed = PureType::Result(host.declaration(member));
            host.fail = !panic;
            host.unwind = panic;
            let input = source(if member {
                "member(value: batch, name: \"ready-step\")"
            } else {
                "field(value: record, name: \"size\")"
            });
            let name = if member { "batch" } else { "record" };
            let output = catch_unwind(AssertUnwindSafe(|| {
                lower(&input, &[(name, &observed)], LIMITS, &mut host)
            }));
            if panic {
                assert!(output.is_err());
            } else {
                let error = output.unwrap().err().unwrap();
                assert!(!format!("{error:?} {error}").contains("credential"));
                assert!(std::error::Error::source(&error).is_none());
                assert!(matches!(
                    error,
                    BoundProjectionSourceError::Projection(ProjectionSourceError::Native {
                        error: PrivateError("private export credential"),
                        ..
                    })
                ));
            }
            assert_eq!(host.queries.len(), 1);
            assert_eq!(host.drops.get(), 0);
            drop(observed);
            assert_eq!(host.drops.get(), 1);
        }
    }
}
