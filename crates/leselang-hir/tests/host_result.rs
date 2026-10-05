use std::rc::Rc;

use leselang_hir::call_typing::{CallTypeHost, CallTypeLimits, infer_call_type};
use leselang_hir::ir::{Computation, ComputedArgument};
use leselang_hir::pure_typing::{PureTypeEnvironment, TypeInferenceLimits};
use leselang_runtime_core::{
    HostResultDomain, HostResultError, NamedParameter, OperationCatalog, OperationCatalogLimits,
    OperationSchema, ScalarType, ScalarTypeSet, ScalarValue, validate_host_result,
};

struct Environment;
impl<Operation> PureTypeEnvironment<(), Operation> for Environment {
    type Result = ();
    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}

const LIMITS: CallTypeLimits = CallTypeLimits {
    pure: TypeInferenceLimits {
        max_nodes: 8,
        max_depth: 4,
        max_bindings: 0,
    },
    max_arguments: 1,
};

enum EditorReply {
    Caption { owner: Rc<()>, text: String },
    Closed,
}
struct CaptionDeclaration(Rc<()>);
impl HostResultDomain<EditorReply> for CaptionDeclaration {
    type Error = ();
    fn matches_type(&self, reply: &EditorReply) -> bool {
        matches!(reply, EditorReply::Caption { .. })
    }
    fn validate_value(&self, reply: &EditorReply) -> Result<(), ()> {
        match reply {
            EditorReply::Caption { owner, text }
                if Rc::ptr_eq(owner, &self.0) && text.len() <= 64 =>
            {
                Ok(())
            }
            _ => Err(()),
        }
    }
}

#[test]
fn inferred_original_native_declaration_checks_actual_editor_reply_and_identity() {
    let owner = Rc::new(());
    let parameters = [NamedParameter::required(
        "caption",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let schemas = [OperationSchema {
        key: "editor.caption",
        parameters: &parameters,
        result: CaptionDeclaration(owner.clone()),
        required_capability: "editor.write",
    }];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let expression: Computation<(), &str, (), ()> = Computation::Call {
        operation: "editor.caption",
        arguments: vec![ComputedArgument {
            name: "caption".into(),
            value: Computation::Literal {
                value: ScalarValue::String("ready".into()),
            },
        }],
    };
    let host = CallTypeHost {
        catalog: &catalog,
        version: 7,
        granted: &["editor.write"],
        environment: &Environment,
    };
    let declaration = infer_call_type(&expression, &[], &host, LIMITS).unwrap();
    assert!(std::ptr::eq(declaration, &schemas[0].result));
    let reply = EditorReply::Caption {
        owner: owner.clone(),
        text: "actual reply".into(),
    };
    assert_eq!(validate_host_result(declaration, &reply), Ok(()));
    assert_eq!(Rc::strong_count(&owner), 3);
    assert_eq!(
        validate_host_result(declaration, &EditorReply::Closed),
        Err(HostResultError::TypeMismatch)
    );
    assert_eq!(
        validate_host_result(
            declaration,
            &EditorReply::Caption {
                owner: Rc::new(()),
                text: "same scalar type".into(),
            }
        ),
        Err(HostResultError::InvalidValue(()))
    );
}

struct PositionDeclaration;
impl HostResultDomain<ScalarValue> for PositionDeclaration {
    type Error = u8;
    fn matches_type(&self, reply: &ScalarValue) -> bool {
        reply.scalar_type() == ScalarType::Integer
    }
    fn validate_value(&self, reply: &ScalarValue) -> Result<(), u8> {
        match reply {
            ScalarValue::Integer(position) if *position <= 100 => Ok(()),
            _ => Err(9),
        }
    }
}

#[test]
fn unrelated_device_call_uses_the_same_static_to_received_result_boundary() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: PositionDeclaration,
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(
        3,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let expression: Computation<(), u32, (), ()> = Computation::Call {
        operation: 17,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value: Computation::Literal {
                value: ScalarValue::Integer(42),
            },
        }],
    };
    let host = CallTypeHost {
        catalog: &catalog,
        version: 3,
        granted: &[31],
        environment: &Environment,
    };
    let declaration = infer_call_type(&expression, &[], &host, LIMITS).unwrap();
    assert!(std::ptr::eq(declaration, &schemas[0].result));
    assert_eq!(
        validate_host_result(declaration, &ScalarValue::Integer(100)),
        Ok(())
    );
    assert_eq!(
        validate_host_result(declaration, &ScalarValue::Integer(101)),
        Err(HostResultError::InvalidValue(9))
    );
    assert_eq!(
        validate_host_result(declaration, &ScalarValue::String("100".into())),
        Err(HostResultError::TypeMismatch)
    );
}
