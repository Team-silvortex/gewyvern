use std::cell::Cell;
use std::rc::Rc;

use leselang_runtime_core::{
    NamedArgumentError, NamedParameter, ScalarType, validate_named_arguments,
};

#[test]
fn unrelated_native_host_schemas_share_name_preflight_without_domain_constraints() {
    #[derive(PartialEq)]
    enum GuiKey {
        Target,
        Caption,
    }
    let gui = [
        NamedParameter::required(GuiKey::Target, ScalarType::String),
        NamedParameter::optional(GuiKey::Caption, ScalarType::OptionalString),
    ];
    validate_named_arguments(&[GuiKey::Caption, GuiKey::Target], &gui).unwrap();
    validate_named_arguments(&[GuiKey::Target], &gui).unwrap();

    struct DeviceDomain(Rc<String>);
    let robot = [
        NamedParameter::required(17u32, DeviceDomain(Rc::new("position".into()))),
        NamedParameter::optional(42u32, DeviceDomain(Rc::new("speed".into()))),
    ];
    validate_named_arguments(&[42, 17], &robot).unwrap();
    validate_named_arguments(&[17], &robot).unwrap();
    assert_eq!(robot[0].domain.0.as_str(), "position");
    assert_eq!(Rc::strong_count(&robot[0].domain.0), 1);
}

#[test]
fn required_presence_optional_omission_and_empty_signatures_are_explicit() {
    const SCHEMA: &[NamedParameter<&str, ScalarType>] = &[
        NamedParameter::required("target", ScalarType::String),
        NamedParameter::optional("caption", ScalarType::OptionalString),
    ];
    validate_named_arguments(&["target"], SCHEMA).unwrap();
    validate_named_arguments(&["caption", "target"], SCHEMA).unwrap();
    assert_eq!(
        validate_named_arguments(&["caption"], SCHEMA),
        Err(NamedArgumentError::MissingArgument { parameter_index: 0 })
    );
    validate_named_arguments::<u8, ()>(&[], &[]).unwrap();
    validate_named_arguments(&[], &[NamedParameter::optional("caption", ())]).unwrap();
    assert_eq!(
        validate_named_arguments(&[1u8], &[] as &[NamedParameter<u8, ()>]),
        Err(NamedArgumentError::TooManyArguments)
    );
}

#[test]
fn error_positions_and_count_schema_submission_required_order_are_stable() {
    let schema = [
        NamedParameter::required("target", ()),
        NamedParameter::optional("caption", ()),
        NamedParameter::required("enabled", ()),
    ];
    for (names, expected) in [
        (
            vec!["target", "caption", "enabled", "extra"],
            NamedArgumentError::TooManyArguments,
        ),
        (
            vec!["target", "target"],
            NamedArgumentError::DuplicateArgument { index: 1 },
        ),
        (
            vec!["target", "secret", "secret"],
            NamedArgumentError::UnknownArgument { index: 1 },
        ),
        (
            vec!["caption"],
            NamedArgumentError::MissingArgument { parameter_index: 0 },
        ),
        (
            vec!["target"],
            NamedArgumentError::MissingArgument { parameter_index: 2 },
        ),
    ] {
        assert_eq!(validate_named_arguments(&names, &schema), Err(expected));
        assert!(!format!("{expected:?}: {expected}").contains("secret"));
    }
    let duplicated = [
        NamedParameter::optional("same", ()),
        NamedParameter::required("same", ()),
    ];
    assert_eq!(
        validate_named_arguments(&["unknown"], &duplicated),
        Err(NamedArgumentError::DuplicateParameter { index: 1 })
    );
    assert_eq!(
        validate_named_arguments(&["unknown"; 3], &duplicated),
        Err(NamedArgumentError::TooManyArguments)
    );
}

#[test]
fn mutable_schema_data_is_rechecked_not_a_lasting_signature_certificate() {
    let mut schema = [
        NamedParameter::optional(1, ()),
        NamedParameter::optional(2, ()),
    ];
    validate_named_arguments(&[], &schema).unwrap();
    schema[1].required = true;
    assert_eq!(
        validate_named_arguments(&[], &schema),
        Err(NamedArgumentError::MissingArgument { parameter_index: 1 })
    );
    schema[1].name = 1;
    assert_eq!(
        validate_named_arguments(&[1], &schema),
        Err(NamedArgumentError::DuplicateParameter { index: 1 })
    );
}

#[test]
fn key_grammar_normalization_value_types_and_operation_domains_remain_host_owned() {
    let private = "private".repeat(1000);
    for key in ["", "\u{754c}", private.as_str(), "Case", "case"] {
        let schema = [NamedParameter::required(key, ScalarType::None)];
        validate_named_arguments(&[key], &schema).unwrap();
    }
    let schema = [NamedParameter::required("Case", ())];
    let error = validate_named_arguments(&["case"], &schema).unwrap_err();
    assert_eq!(error, NamedArgumentError::UnknownArgument { index: 0 });
    assert_eq!(error.to_string(), "unknown named argument key");
    // A type/domain tag is metadata: no value exists here to validate or coerce.
}

#[test]
fn preflight_borrows_nonclone_nondebug_keys_and_thread_local_opaque_domains() {
    #[derive(PartialEq)]
    struct Key(String);
    struct Domain {
        private: Rc<String>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Domain {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    let private = Rc::new(String::from("never read or formatted"));
    let drops = Rc::new(Cell::new(0));
    let schema = [NamedParameter::required(
        Key("target".into()),
        Domain {
            private: private.clone(),
            drops: drops.clone(),
        },
    )];
    let names = [Key("target".into())];
    let pointer = names[0].0.as_ptr();
    for _ in 0..3 {
        validate_named_arguments(&names, &schema).unwrap();
        assert_eq!(
            validate_named_arguments(&[], &schema),
            Err(NamedArgumentError::MissingArgument { parameter_index: 0 })
        );
        assert_eq!(names[0].0.as_ptr(), pointer);
        assert!(Rc::ptr_eq(&schema[0].domain.private, &private));
        assert_eq!(Rc::strong_count(&private), 2);
        assert_eq!(drops.get(), 0);
    }
    drop(schema);
    assert_eq!(drops.get(), 1);
}

#[test]
fn excessive_count_is_rejected_without_invoking_native_key_comparisons() {
    struct Key;
    impl PartialEq for Key {
        fn eq(&self, _: &Self) -> bool {
            panic!("key comparison must not run on excessive argument counts")
        }
    }
    let schema = [
        NamedParameter::required(Key, ()),
        NamedParameter::optional(Key, ()),
    ];
    assert_eq!(
        validate_named_arguments(&[Key, Key, Key], &schema),
        Err(NamedArgumentError::TooManyArguments)
    );
}

#[test]
fn native_key_comparison_unwind_propagates_without_fabricated_language_outcomes() {
    struct Key(Rc<Cell<usize>>);
    impl PartialEq for Key {
        fn eq(&self, _: &Self) -> bool {
            self.0.set(self.0.get() + 1);
            panic!("trusted native comparison failed")
        }
    }
    let calls = Rc::new(Cell::new(0));
    let schema = [NamedParameter::required(Key(calls.clone()), ())];
    let names = [Key(calls.clone())];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        validate_named_arguments(&names, &schema)
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(Rc::strong_count(&calls), 3);
    assert_eq!(names.len(), 1);
    assert!(schema[0].required);
}

#[test]
fn exhaustive_small_valid_schemas_match_the_legacy_shape_predicate() {
    for required in 0u8..8 {
        let schema = (0u8..3)
            .map(|name| NamedParameter {
                name,
                domain: (),
                required: required & (1 << name) != 0,
            })
            .collect::<Vec<_>>();
        for length in 0..=4 {
            for mut code in 0..4usize.pow(length as u32) {
                let names = (0..length)
                    .map(|_| {
                        let name = (code % 4) as u8;
                        code /= 4;
                        name
                    })
                    .collect::<Vec<_>>();
                let valid = names.len() <= schema.len()
                    && names.iter().enumerate().all(|(index, name)| {
                        !names[..index].contains(name) && schema.iter().any(|p| p.name == *name)
                    })
                    && schema
                        .iter()
                        .all(|p| !p.required || names.contains(&p.name));
                assert_eq!(validate_named_arguments(&names, &schema).is_ok(), valid);
            }
        }
    }
}
