use std::{cell::Cell, rc::Rc};

use leselang_runtime_core::{
    CalculationFailure, NamedArgumentError, NamedParameter, ScalarType, ScalarTypeSet, ScalarValue,
    bind_named_arguments, validate_named_arguments,
};

#[test]
fn unrelated_native_schemas_borrow_original_parameters_in_declaration_order() {
    #[derive(PartialEq)]
    enum GuiKey {
        Target,
        Caption,
    }
    let gui = [
        NamedParameter::required(GuiKey::Target, ScalarTypeSet::only(ScalarType::String)),
        NamedParameter::optional(GuiKey::Caption, ScalarTypeSet::only(ScalarType::None)),
    ];
    let names = [GuiKey::Caption, GuiKey::Target];
    let bindings = bind_named_arguments(&names, &gui).unwrap();
    let aligned: Vec<_> = bindings.iter().collect();
    assert!(std::ptr::eq(aligned[0].0, &gui[0]));
    assert!(std::ptr::eq(aligned[1].0, &gui[1]));
    assert_eq!([aligned[0].1, aligned[1].1], [1, 0]);

    struct DeviceDomain(Rc<String>);
    let device = [
        NamedParameter::optional(17u32, DeviceDomain(Rc::new("position".into()))),
        NamedParameter::required(42u32, DeviceDomain(Rc::new("speed".into()))),
    ];
    let names = [42u32];
    let bindings = bind_named_arguments(&names, &device).unwrap();
    let (parameter, index) = bindings.iter().next().unwrap();
    assert!(std::ptr::eq(parameter, &device[1]));
    assert_eq!(index, 0);
    assert_eq!(parameter.domain.0.as_str(), "speed");
    assert_eq!(Rc::strong_count(&parameter.domain.0), 1);
}

#[test]
fn empty_bindings_optional_omission_and_required_presence_do_not_create_defaults() {
    let empty = bind_named_arguments::<u8, ()>(&[], &[]).unwrap();
    assert_eq!(empty.iter().count(), 0);
    let parameters = [NamedParameter::optional(7u8, ())];
    let bindings = bind_named_arguments(&[], &parameters).unwrap();
    assert_eq!(bindings.iter().count(), 0);
    assert_eq!(
        format!("{bindings:?}"),
        "NamedArgumentBindings { arguments: 0, parameters: 1 }"
    );
    let parameters = [NamedParameter::required(7u8, ())];
    assert_eq!(
        bind_named_arguments(&[], &parameters).unwrap_err(),
        NamedArgumentError::MissingArgument { parameter_index: 0 }
    );
}

#[test]
fn exhaustive_small_submissions_keep_preflight_errors_and_legacy_binding_order() {
    let mut cases = 0;
    for required in 0u8..8 {
        let parameters: Vec<_> = (0..3)
            .map(|name| NamedParameter {
                name,
                domain: (),
                required: required & (1 << name) != 0,
            })
            .collect();
        for len in 0..=4 {
            for encoded in 0..4usize.pow(len) {
                let mut encoded = encoded;
                let names: Vec<_> = (0..len)
                    .map(|_| {
                        let name = encoded % 4;
                        encoded /= 4;
                        name
                    })
                    .collect();
                let expected = validate_named_arguments(&names, &parameters);
                match bind_named_arguments(&names, &parameters) {
                    Ok(bindings) => {
                        assert_eq!(expected, Ok(()));
                        let actual: Vec<_> = bindings
                            .iter()
                            .map(|(parameter, index)| (parameter.name, index))
                            .collect();
                        let legacy: Vec<_> = parameters
                            .iter()
                            .filter_map(|parameter| {
                                names
                                    .iter()
                                    .position(|name| *name == parameter.name)
                                    .map(|index| (parameter.name, index))
                            })
                            .collect();
                        assert_eq!(actual, legacy);
                        assert_eq!(actual.len(), names.len());
                    }
                    Err(error) => assert_eq!(expected, Err(error)),
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 2728);
}

#[test]
fn forward_reverse_and_repeated_metadata_walks_are_fused_and_do_not_consume_values() {
    let parameters = [
        NamedParameter::optional("first", ()),
        NamedParameter::optional("absent", ()),
        NamedParameter::required("middle", ()),
        NamedParameter::required("last", ()),
    ];
    let names = ["last", "first", "middle"];
    let bindings = bind_named_arguments(&names, &parameters).unwrap();
    let mut iter = bindings.iter();
    assert_eq!(
        iter.next()
            .map(|(parameter, index)| (parameter.name, index)),
        Some(("first", 1))
    );
    assert_eq!(
        iter.next_back()
            .map(|(parameter, index)| (parameter.name, index)),
        Some(("last", 0))
    );
    assert_eq!(
        iter.next()
            .map(|(parameter, index)| (parameter.name, index)),
        Some(("middle", 2))
    );
    for _ in 0..3 {
        assert!(iter.next().is_none());
        assert!(iter.next_back().is_none());
    }
    assert_eq!(
        bindings.iter().map(|(_, index)| index).collect::<Vec<_>>(),
        [1, 2, 0]
    );
    assert_eq!(
        bindings
            .iter()
            .rev()
            .map(|(_, index)| index)
            .collect::<Vec<_>>(),
        [0, 2, 1]
    );
}

#[test]
fn opaque_keys_domains_and_submitted_values_are_not_cloned_read_or_reordered() {
    #[derive(PartialEq)]
    struct Key(String);
    struct Domain(Rc<Cell<usize>>);
    impl Drop for Domain {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    struct Value(String);
    let drops = Rc::new(Cell::new(0));
    let parameters = [
        NamedParameter::required(Key("target".into()), Domain(drops.clone())),
        NamedParameter::optional(Key("caption".into()), Domain(drops.clone())),
    ];
    let names = [Key("caption".into()), Key("target".into())];
    let values = [Value("private text".into()), Value("node-a".into())];
    let pointers = [values[0].0.as_ptr(), values[1].0.as_ptr()];
    let name_pointer = names[0].0.as_ptr();
    let bindings = bind_named_arguments(&names, &parameters).unwrap();
    let ordered: Vec<_> = bindings.iter().map(|(_, index)| &values[index].0).collect();
    assert_eq!(ordered[0].as_ptr(), pointers[1]);
    assert_eq!(ordered[1].as_ptr(), pointers[0]);
    assert_eq!(names[0].0.as_ptr(), name_pointer);
    assert_eq!(drops.get(), 0);
    drop(parameters);
    assert_eq!(drops.get(), 2);
    assert_eq!(values[0].0, "private text");
    assert_eq!(values[1].0, "node-a");
}

#[test]
fn shape_success_does_not_validate_types_domains_fuel_or_execution_authority() {
    let parameters = [NamedParameter::required(
        "caption",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let names = ["caption"];
    let values = [ScalarValue::None];
    let bindings = bind_named_arguments(&names, &parameters).unwrap();
    let (parameter, index) = bindings.iter().next().unwrap();
    assert!(parameter.domain.validate(&values[index]).is_err());
    let values = [ScalarValue::String("invalid native format".into())];
    parameter.domain.validate(&values[index]).unwrap();
    assert!(values[index].text().unwrap().contains(' '));
    // Native format and authority checks are deliberately separate host responsibilities.
}

#[test]
fn debug_uses_only_counts_without_native_key_domain_or_payload_formatters() {
    #[derive(PartialEq)]
    struct Key(String);
    impl std::fmt::Debug for Key {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("private key formatter")
        }
    }
    struct Domain;
    impl std::fmt::Debug for Domain {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("private domain formatter")
        }
    }
    let parameters = [NamedParameter::required(
        Key("private".repeat(1000)),
        Domain,
    )];
    let names = [Key("private".repeat(1000))];
    let bindings = bind_named_arguments(&names, &parameters).unwrap();
    assert_eq!(
        format!("{bindings:?}"),
        "NamedArgumentBindings { arguments: 1, parameters: 1 }"
    );
}

#[test]
fn excessive_counts_fail_before_native_comparison_and_closed_errors_stay_external() {
    struct Key;
    impl PartialEq for Key {
        fn eq(&self, _: &Self) -> bool {
            panic!("comparison must not run")
        }
    }
    let parameters = [NamedParameter::optional(Key, ())];
    let names = [Key, Key];
    let error = bind_named_arguments(&names, &parameters).unwrap_err();
    assert_eq!(error, NamedArgumentError::TooManyArguments);
    let failure: CalculationFailure<NamedArgumentError> = error.into();
    assert!(!failure.is_recoverable());
}

#[test]
fn native_comparison_work_per_walk_is_bounded_by_both_caller_bounded_slices() {
    struct Key(u8, Rc<Cell<usize>>);
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.1.set(self.1.get() + 1);
            self.0 == other.0
        }
    }
    let calls = Rc::new(Cell::new(0));
    let parameters: Vec<_> = (0..5)
        .map(|key| NamedParameter::optional(Key(key, calls.clone()), ()))
        .collect();
    let names = [Key(4, calls.clone()), Key(0, calls.clone())];
    let bindings = bind_named_arguments(&names, &parameters).unwrap();
    for reverse in [false, true] {
        calls.set(0);
        let mut iter = bindings.iter();
        let mut visited = Vec::new();
        while let Some((_, index)) = if reverse {
            iter.next_back()
        } else {
            iter.next()
        } {
            visited.push(index);
        }
        assert_eq!(visited, if reverse { vec![0, 1] } else { vec![1, 0] });
        assert!(calls.get() <= parameters.len() * names.len());
    }
}

#[test]
fn native_key_unwind_propagates_before_any_host_value_evaluation() {
    struct Key(u8, Rc<Cell<bool>>);
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            assert!(!self.1.get(), "trusted native comparison failed");
            self.0 == other.0
        }
    }
    let armed = Rc::new(Cell::new(false));
    let parameters = [NamedParameter::required(Key(1, armed.clone()), ())];
    let names = [Key(1, armed.clone())];
    let bindings = bind_named_arguments(&names, &parameters).unwrap();
    armed.set(true);
    let evaluated = Cell::new(false);
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if bindings.iter().next().is_some() {
            evaluated.set(true);
        }
    }));
    assert!(failed.is_err());
    assert!(!evaluated.get());
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = bind_named_arguments(&names, &parameters);
    }));
    assert!(failed.is_err());
}
