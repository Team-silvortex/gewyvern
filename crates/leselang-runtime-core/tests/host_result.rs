use std::cell::Cell;
use std::rc::Rc;

use leselang_runtime_core::{
    HostResultDomain, HostResultError, MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS,
    OperationCatalog, OperationCatalogError, OperationCatalogLimits, OperationSchema,
    OptionalStringValue, ScalarContractError, ScalarType, ScalarTypeSet, ScalarValue,
    StringListValue, validate_host_result,
};

fn values() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(true),
        ScalarValue::String("received".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec!["z".into(), "".into(), "z".into()])),
    ]
}

#[test]
fn all_six_received_scalar_types_require_explicit_alternatives_without_coercion() {
    for reply in values() {
        for declared in values() {
            let domain = ScalarTypeSet::only(declared.scalar_type());
            assert_eq!(
                validate_host_result(&domain, &reply),
                if reply.scalar_type() == declared.scalar_type() {
                    Ok(())
                } else {
                    Err(HostResultError::TypeMismatch)
                }
            );
        }
        assert_eq!(
            validate_host_result(&ScalarTypeSet::empty(), &reply),
            Err(HostResultError::TypeMismatch)
        );
    }
    let nullable = ScalarTypeSet::only(ScalarType::None).with(ScalarType::OptionalString);
    assert_eq!(validate_host_result(&nullable, &ScalarValue::None), Ok(()));
    assert_eq!(
        validate_host_result(&nullable, &ScalarValue::String(String::new())),
        Err(HostResultError::TypeMismatch)
    );
}

#[test]
fn mutable_received_scalar_values_are_rechecked_with_original_bounds_and_buffers() {
    let maximum = "\u{754c}".repeat(MAX_SCALAR_STRING_BYTES / 3) + "x";
    for (reply, bounded) in [
        (ScalarValue::String(maximum.clone()), true),
        (ScalarValue::String(maximum.clone() + "x"), false),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some(String::new()))),
            true,
        ),
        (
            ScalarValue::OptionalString(OptionalStringValue(Some(maximum.clone() + "x"))),
            false,
        ),
        (
            ScalarValue::StringList(StringListValue(vec![maximum.clone(), String::new()])),
            true,
        ),
        (
            ScalarValue::StringList(StringListValue(vec![maximum, "x".into()])),
            false,
        ),
        (
            ScalarValue::StringList(StringListValue(vec![String::new(); MAX_STRING_LIST_ITEMS])),
            true,
        ),
        (
            ScalarValue::StringList(StringListValue(vec![
                String::new();
                MAX_STRING_LIST_ITEMS + 1
            ])),
            false,
        ),
    ] {
        let domain = ScalarTypeSet::only(reply.scalar_type());
        let original = serde_json::to_string(&reply).unwrap();
        assert_eq!(
            validate_host_result(&domain, &reply),
            if bounded {
                Ok(())
            } else {
                Err(HostResultError::InvalidValue(
                    ScalarContractError::UnboundedValue,
                ))
            }
        );
        assert_eq!(serde_json::to_string(&reply).unwrap(), original);
        assert_eq!(
            validate_host_result(&ScalarTypeSet::empty(), &reply),
            Err(HostResultError::TypeMismatch)
        );
    }
    let mut reply = ScalarValue::String("original buffer".into());
    let pointer = reply.text().unwrap().as_ptr();
    let domain = ScalarTypeSet::only(ScalarType::String);
    validate_host_result(&domain, &reply).unwrap();
    assert_eq!(reply.text().unwrap().as_ptr(), pointer);
    reply = ScalarValue::Boolean(true);
    assert_eq!(
        validate_host_result(&domain, &reply),
        Err(HostResultError::TypeMismatch)
    );
}

enum EditorReply {
    Caption { owner: Rc<()>, text: String },
    Closed,
}
struct EditorDomain {
    owner: Rc<()>,
    calls: Cell<usize>,
}
impl HostResultDomain<EditorReply> for EditorDomain {
    type Error = &'static str;
    fn matches_type(&self, reply: &EditorReply) -> bool {
        matches!(reply, EditorReply::Caption { .. })
    }
    fn validate_value(&self, reply: &EditorReply) -> Result<(), Self::Error> {
        self.calls.set(self.calls.get() + 1);
        match reply {
            EditorReply::Caption { owner, text }
                if Rc::ptr_eq(owner, &self.owner) && text.len() <= 64 =>
            {
                Ok(())
            }
            _ => Err("private editor rejection"),
        }
    }
}

struct DeviceReply {
    lease: u64,
    position: u64,
}
struct DeviceDomain {
    lease: u64,
}
impl HostResultDomain<DeviceReply> for DeviceDomain {
    type Error = u8;
    fn matches_type(&self, _: &DeviceReply) -> bool {
        true
    }
    fn validate_value(&self, reply: &DeviceReply) -> Result<(), Self::Error> {
        if reply.lease != self.lease {
            Err(1)
        } else if reply.position > 100 {
            Err(2)
        } else {
            Ok(())
        }
    }
}

#[test]
fn two_unrelated_native_reply_domains_use_original_operation_declarations() {
    let owner = Rc::new(());
    let schemas: [OperationSchema<'_, &str, &str, (), _, &str>; 1] = [OperationSchema {
        key: "editor.caption",
        parameters: &[],
        result: EditorDomain {
            owner: owner.clone(),
            calls: Cell::new(0),
        },
        required_capability: "editor.read",
    }];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let selected = catalog
        .authorize("editor.caption", 7, &["editor.read"])
        .unwrap();
    assert!(std::ptr::eq(selected, &schemas[0]));
    let reply = EditorReply::Caption {
        owner: owner.clone(),
        text: "actual value".into(),
    };
    assert_eq!(selected.check_result(&reply), Ok(()));
    assert_eq!(Rc::strong_count(&owner), 3);
    assert_eq!(
        selected.check_result(&EditorReply::Closed),
        Err(HostResultError::TypeMismatch)
    );
    assert_eq!(schemas[0].result.calls.get(), 1);
    let stale = EditorReply::Caption {
        owner: Rc::new(()),
        text: "actual value".into(),
    };
    assert_eq!(
        selected.check_result(&stale),
        Err(HostResultError::InvalidValue("private editor rejection"))
    );

    let schemas: [OperationSchema<'_, u32, u8, (), _, u8>; 1] = [OperationSchema {
        key: 17,
        parameters: &[],
        result: DeviceDomain { lease: 9 },
        required_capability: 31,
    }];
    let catalog = OperationCatalog::new(
        3,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let selected = catalog.authorize(&17, 3, &[31]).unwrap();
    assert_eq!(
        selected.check_result(&DeviceReply {
            lease: 9,
            position: 100
        }),
        Ok(())
    );
    assert_eq!(
        selected.check_result(&DeviceReply {
            lease: 8,
            position: 100
        }),
        Err(HostResultError::InvalidValue(1))
    );
    assert_eq!(
        selected.check_result(&DeviceReply {
            lease: 9,
            position: 101
        }),
        Err(HostResultError::InvalidValue(2))
    );
}

#[test]
fn version_operation_and_trusted_grant_preflight_never_enter_reply_callbacks_on_failure() {
    let schemas: [OperationSchema<'_, u8, u8, (), _, u8>; 1] = [OperationSchema {
        key: 1,
        parameters: &[],
        result: EditorDomain {
            owner: Rc::new(()),
            calls: Cell::new(0),
        },
        required_capability: 2,
    }];
    let catalog = OperationCatalog::new(
        3,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    for (key, version, grants, expected) in [
        (1, 0, vec![2], OperationCatalogError::UnsupportedVersion),
        (9, 3, vec![2], OperationCatalogError::UnknownOperation),
        (1, 3, vec![], OperationCatalogError::CapabilityDenied),
    ] {
        assert_eq!(
            catalog.authorize(&key, version, &grants).unwrap_err(),
            expected
        );
    }
    assert_eq!(schemas[0].result.calls.get(), 0);
}

#[test]
fn generated_errors_never_format_or_clone_native_payloads_and_preserve_failure_ownership() {
    struct PrivateError(Rc<()>);
    impl std::fmt::Debug for PrivateError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("private formatter")
        }
    }
    struct Domain(Rc<()>);
    impl HostResultDomain<()> for Domain {
        type Error = PrivateError;
        fn matches_type(&self, _: &()) -> bool {
            true
        }
        fn validate_value(&self, _: &()) -> Result<(), Self::Error> {
            Err(PrivateError(self.0.clone()))
        }
    }
    let owner = Rc::new(());
    let domain = Domain(owner.clone());
    let error = validate_host_result(&domain, &()).unwrap_err();
    assert_eq!(format!("{error:?}"), "HostResultError::InvalidValue");
    assert_eq!(
        error.to_string(),
        "host result failed native value validation"
    );
    assert!(std::error::Error::source(&error).is_none());
    let HostResultError::InvalidValue(payload) = error else {
        panic!()
    };
    assert!(Rc::ptr_eq(&payload.0, &owner));
    assert_eq!(Rc::strong_count(&owner), 3);
    drop(payload);
    assert_eq!(Rc::strong_count(&owner), 2);
}

#[test]
fn unsized_replies_and_native_trait_objects_need_no_clone_debug_serde_or_send() {
    struct TextDomain(Rc<()>);
    impl HostResultDomain<str> for TextDomain {
        type Error = ();
        fn matches_type(&self, _: &str) -> bool {
            true
        }
        fn validate_value(&self, reply: &str) -> Result<(), ()> {
            let _owner = &self.0;
            if reply.len() <= 4 { Ok(()) } else { Err(()) }
        }
    }
    let domain = TextDomain(Rc::new(()));
    let erased: &dyn HostResultDomain<str, Error = ()> = &domain;
    assert_eq!(validate_host_result(erased, "ok"), Ok(()));
    assert_eq!(
        validate_host_result(erased, "large"),
        Err(HostResultError::InvalidValue(()))
    );
}

#[test]
fn callback_unwind_propagates_without_retry_or_mutating_received_values() {
    struct Domain {
        type_calls: Cell<u8>,
        value_calls: Cell<u8>,
        panic_in_type: bool,
    }
    impl HostResultDomain<String> for Domain {
        type Error = ();
        fn matches_type(&self, _: &String) -> bool {
            self.type_calls.set(self.type_calls.get() + 1);
            assert!(!self.panic_in_type, "native type unwind");
            true
        }
        fn validate_value(&self, _: &String) -> Result<(), ()> {
            self.value_calls.set(self.value_calls.get() + 1);
            panic!("native value unwind")
        }
    }
    let reply = String::from("owned reply");
    let pointer = reply.as_ptr();
    for panic_in_type in [true, false] {
        let domain = Domain {
            type_calls: Cell::new(0),
            value_calls: Cell::new(0),
            panic_in_type,
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            validate_host_result(&domain, &reply)
        }));
        assert!(outcome.is_err());
        assert_eq!(domain.type_calls.get(), 1);
        assert_eq!(domain.value_calls.get(), u8::from(!panic_in_type));
        assert_eq!(reply, "owned reply");
        assert_eq!(reply.as_ptr(), pointer);
    }
}

#[test]
fn native_scalar_looking_failures_are_not_converted_into_type_or_recovery_success() {
    struct Domain;
    impl HostResultDomain<()> for Domain {
        type Error = leselang_runtime_core::ScalarError;
        fn matches_type(&self, _: &()) -> bool {
            true
        }
        fn validate_value(&self, _: &()) -> Result<(), Self::Error> {
            Err(leselang_runtime_core::ScalarError::IntegerArithmetic)
        }
    }
    assert_eq!(
        validate_host_result(&Domain, &()),
        Err(HostResultError::InvalidValue(
            leselang_runtime_core::ScalarError::IntegerArithmetic
        ))
    );
}
