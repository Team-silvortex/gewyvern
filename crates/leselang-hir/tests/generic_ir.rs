use std::{cell::Cell, rc::Rc};

use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_runtime_core::{
    BinaryOperator, NamedParameter, OperationCatalog, OperationCatalogError,
    OperationCatalogLimits, OperationSchema, ScalarType, ScalarTypeSet, ScalarValue, UnaryOperator,
};

type TextIr = Computation<String, String, String, String>;

fn integer(value: u64) -> TextIr {
    TextIr::Literal {
        value: ScalarValue::Integer(value),
    }
}

#[test]
fn unrelated_gui_and_device_catalogs_supply_different_operation_and_result_types_to_one_ir() {
    #[derive(PartialEq)]
    enum GuiOperation {
        Caption,
    }
    enum GuiField {
        Applied,
    }
    enum GuiEffect {
        Native,
    }
    enum GuiResult {
        Receipt,
    }
    type GuiIr = Computation<GuiField, GuiOperation, GuiEffect, GuiResult>;
    let parameters = [NamedParameter::required(
        "caption",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let schemas = [OperationSchema {
        key: GuiOperation::Caption,
        parameters: &parameters,
        result: GuiResult::Receipt,
        required_capability: "view.edit",
    }];
    let limits = OperationCatalogLimits {
        max_operations: 1,
        max_parameters_per_operation: 1,
    };
    let catalog = OperationCatalog::new(7, &schemas, limits).unwrap();
    let schema = catalog
        .authorize(&GuiOperation::Caption, 7, &["view.edit"])
        .unwrap();
    let names = ["caption"];
    schema.bind_arguments(&names).unwrap();
    let value = ScalarValue::String("ready".into());
    parameters[0].domain.validate(&value).unwrap();
    let gui = GuiIr::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "caption".into(),
            value: GuiIr::Call {
                operation: GuiOperation::Caption,
                arguments: vec![ComputedArgument {
                    name: names[0].into(),
                    value: GuiIr::Literal { value },
                }],
            },
            result_type: GuiResult::Receipt,
        }],
    };
    assert!(!gui.is_pure());
    let GuiIr::Group { branches, .. } = gui else {
        panic!("expected group")
    };
    let projected = GuiIr::Field {
        value: Box::new(GuiIr::Local {
            name: branches[0].name.clone(),
        }),
        field: GuiField::Applied,
    };
    assert!(projected.is_pure());
    assert!(
        !GuiIr::Host {
            effect: Box::new(GuiEffect::Native)
        }
        .is_pure()
    );

    struct DeviceEffect(Rc<String>);
    type DeviceIr = Computation<u16, u32, DeviceEffect, ScalarType>;
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: ScalarType::Boolean,
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(9, &schemas, limits).unwrap();
    let schema = catalog.authorize(&17, 9, &[31]).unwrap();
    parameters[0]
        .domain
        .validate(&ScalarValue::Integer(4))
        .unwrap();
    let device = DeviceIr::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "move".into(),
            value: DeviceIr::Call {
                operation: schema.key,
                arguments: vec![ComputedArgument {
                    name: "position".into(),
                    value: DeviceIr::Literal {
                        value: ScalarValue::Integer(4),
                    },
                }],
            },
            result_type: schema.result,
        }],
    };
    assert!(!device.is_pure());
    let effect = DeviceEffect(Rc::new("native receipt resource".into()));
    let device = DeviceIr::Host {
        effect: Box::new(effect),
    };
    assert_eq!(device.children().count(), 0);
    let DeviceIr::Host { effect } = device else {
        panic!("expected effect")
    };
    assert_eq!(Rc::strong_count(&effect.0), 1);
    // These are shared native IR/catalog proofs, not a generic parser or executor.
}

#[test]
fn all_control_node_variants_keep_exact_tags_field_order_and_strict_data_roundtrips() {
    let leaf = || Box::new(integer(7));
    let literal = r#"{"kind":"literal","value":{"kind":"integer","value":7}}"#;
    let mut cases = vec![
        (integer(7), literal.to_string()),
        (
            TextIr::Strings {
                items: vec![integer(7)],
            },
            format!(r#"{{"kind":"strings","items":[{literal}]}}"#),
        ),
        (
            TextIr::Local { name: "x".into() },
            r#"{"kind":"local","name":"x"}"#.into(),
        ),
        (
            TextIr::Field {
                value: leaf(),
                field: "position".into(),
            },
            format!(r#"{{"kind":"field","value":{literal},"field":"position"}}"#),
        ),
        (
            TextIr::Member {
                group: "g".into(),
                name: "m".into(),
                operation: "device.move".into(),
            },
            r#"{"kind":"member","group":"g","name":"m","operation":"device.move"}"#.into(),
        ),
        (
            TextIr::Binary {
                operator: BinaryOperator::Add,
                left: leaf(),
                right: leaf(),
            },
            format!(r#"{{"kind":"binary","operator":"add","left":{literal},"right":{literal}}}"#),
        ),
        (
            TextIr::Unary {
                operator: UnaryOperator::Not,
                value: leaf(),
            },
            format!(r#"{{"kind":"unary","operator":"not","value":{literal}}}"#),
        ),
        (
            TextIr::Bind {
                name: "x".into(),
                value: leaf(),
                body: leaf(),
            },
            format!(r#"{{"kind":"bind","name":"x","value":{literal},"body":{literal}}}"#),
        ),
        (
            TextIr::Loop {
                name: "x".into(),
                initial: leaf(),
                condition: leaf(),
                next: leaf(),
                limit: 3,
            },
            format!(
                r#"{{"kind":"loop","name":"x","initial":{literal},"condition":{literal},"next":{literal},"limit":3}}"#
            ),
        ),
        (
            TextIr::Fold {
                name: "x".into(),
                item: "item".into(),
                items: leaf(),
                initial: leaf(),
                next: leaf(),
                limit: 3,
            },
            format!(
                r#"{{"kind":"fold","name":"x","item":"item","items":{literal},"initial":{literal},"next":{literal},"limit":3}}"#
            ),
        ),
        (
            TextIr::Choose {
                when: leaf(),
                then: leaf(),
                otherwise: leaf(),
            },
            format!(
                r#"{{"kind":"choose","when":{literal},"then":{literal},"otherwise":{literal}}}"#
            ),
        ),
        (
            TextIr::Recover {
                value: leaf(),
                fallback: leaf(),
            },
            format!(r#"{{"kind":"recover","value":{literal},"fallback":{literal}}}"#),
        ),
        (
            TextIr::Host {
                effect: Box::new("native-effect".into()),
            },
            r#"{"kind":"host","effect":"native-effect"}"#.into(),
        ),
        (
            TextIr::Call {
                operation: "device.move".into(),
                arguments: vec![ComputedArgument {
                    name: "position".into(),
                    value: integer(7),
                }],
            },
            format!(
                r#"{{"kind":"call","operation":"device.move","arguments":[{{"name":"position","value":{literal}}}]}}"#
            ),
        ),
    ];
    for (kind, tag) in [
        (GroupKind::Sequence, "sequence"),
        (GroupKind::Parallel, "parallel"),
    ] {
        cases.push((TextIr::Group { group_kind: kind, branches: vec![ComputedBranch { name: "m".into(), value: integer(7), result_type: "receipt".into() }] },
            format!(r#"{{"kind":"group","group_kind":"{tag}","branches":[{{"name":"m","value":{literal},"result_type":"receipt"}}]}}"#)));
    }
    assert_eq!(cases.len(), 16);
    for (node, expected) in cases {
        assert_eq!(serde_json::to_string(&node).unwrap(), expected);
        assert_eq!(serde_json::from_str::<TextIr>(&expected).unwrap(), node);
        let with_extra = format!("{},\"private\":true}}", &expected[..expected.len() - 1]);
        assert!(serde_json::from_str::<TextIr>(&with_extra).is_err());
    }
    for invalid in [
        r#"{"kind":"unknown"}"#,
        r#"{"kind":"local","name":"x","name":"y"}"#,
        r#"{"kind":"call","operation":"device.move","arguments":[{"name":"x","value":{"kind":"local","name":"x"},"extra":true}]}"#,
        r#"{"kind":"group","group_kind":"sequence","branches":[{"name":"x","value":{"kind":"local","name":"x"},"result_type":"receipt","extra":true}]}"#,
    ] {
        assert!(serde_json::from_str::<TextIr>(invalid).is_err());
    }
}

#[test]
fn every_child_layout_preserves_declaration_order_original_borrows_and_fused_reverse_walks() {
    let leaf = |n| Box::new(integer(n));
    let cases = [
        (
            TextIr::Binary {
                operator: BinaryOperator::Add,
                left: leaf(1),
                right: leaf(2),
            },
            vec![1, 2],
        ),
        (
            TextIr::Unary {
                operator: UnaryOperator::Not,
                value: leaf(1),
            },
            vec![1],
        ),
        (
            TextIr::Field {
                value: leaf(1),
                field: "f".into(),
            },
            vec![1],
        ),
        (
            TextIr::Bind {
                name: "x".into(),
                value: leaf(1),
                body: leaf(2),
            },
            vec![1, 2],
        ),
        (
            TextIr::Recover {
                value: leaf(1),
                fallback: leaf(2),
            },
            vec![1, 2],
        ),
        (
            TextIr::Loop {
                name: "x".into(),
                initial: leaf(1),
                condition: leaf(2),
                next: leaf(3),
                limit: 0,
            },
            vec![1, 2, 3],
        ),
        (
            TextIr::Fold {
                name: "x".into(),
                item: "y".into(),
                items: leaf(1),
                initial: leaf(2),
                next: leaf(3),
                limit: 0,
            },
            vec![1, 2, 3],
        ),
        (
            TextIr::Choose {
                when: leaf(1),
                then: leaf(2),
                otherwise: leaf(3),
            },
            vec![1, 2, 3],
        ),
        (
            TextIr::Strings {
                items: vec![integer(1), integer(2), integer(3)],
            },
            vec![1, 2, 3],
        ),
        (
            TextIr::Call {
                operation: "op".into(),
                arguments: (1..=3)
                    .map(|n| ComputedArgument {
                        name: n.to_string(),
                        value: integer(n),
                    })
                    .collect(),
            },
            vec![1, 2, 3],
        ),
        (
            TextIr::Group {
                group_kind: GroupKind::Parallel,
                branches: (1..=3)
                    .map(|n| ComputedBranch {
                        name: n.to_string(),
                        value: integer(n),
                        result_type: "receipt".into(),
                    })
                    .collect(),
            },
            vec![1, 2, 3],
        ),
    ];
    let number = |value: &TextIr| match value {
        TextIr::Literal {
            value: ScalarValue::Integer(n),
        } => *n,
        _ => panic!("expected integer"),
    };
    for (node, expected) in cases {
        let originals: Vec<_> = node.children().collect();
        assert_eq!(
            originals
                .iter()
                .map(|node| number(node))
                .collect::<Vec<_>>(),
            expected
        );
        for (again, original) in node.children().zip(&originals) {
            assert!(std::ptr::eq(again, *original));
        }
        let mut reverse = node.children();
        for n in expected.iter().rev() {
            assert_eq!(number(reverse.next_back().unwrap()), *n);
        }
        for _ in 0..3 {
            assert!(reverse.next().is_none());
            assert!(reverse.next_back().is_none());
        }
        let mut mixed = node.children();
        assert_eq!(number(mixed.next().unwrap()), expected[0]);
        if expected.len() > 1 {
            assert_eq!(
                number(mixed.next_back().unwrap()),
                *expected.last().unwrap()
            );
        }
    }
}

#[test]
fn opaque_nonclone_nonserde_gui_local_slots_need_no_traits_for_native_ir_inspection() {
    struct Native(Rc<Cell<usize>>);
    impl Drop for Native {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    type NativeIr = Computation<Native, Native, Native, Native>;
    let drops = Rc::new(Cell::new(0));
    let node = NativeIr::Field {
        value: Box::new(NativeIr::Member {
            group: "g".into(),
            name: "m".into(),
            operation: Native(drops.clone()),
        }),
        field: Native(drops.clone()),
    };
    assert!(node.is_pure());
    assert_eq!(node.children().count(), 1);
    assert_eq!(drops.get(), 0);
    drop(node);
    assert_eq!(drops.get(), 2);
    let node = NativeIr::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "m".into(),
            value: NativeIr::Host {
                effect: Box::new(Native(drops.clone())),
            },
            result_type: Native(drops.clone()),
        }],
    };
    assert!(!node.is_pure());
    assert_eq!(node.children().count(), 1);
    assert_eq!(drops.get(), 2);
    drop(node);
    assert_eq!(drops.get(), 4);
}

#[test]
fn cold_effects_and_empty_effect_nodes_are_not_pure_and_native_host_graphs_stay_opaque() {
    for effect in [
        TextIr::Call {
            operation: "unknown".into(),
            arguments: vec![],
        },
        TextIr::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![],
        },
        TextIr::Host {
            effect: Box::new("pure-looking native payload".into()),
        },
    ] {
        assert_eq!(effect.children().count(), 0);
        assert!(!effect.is_pure());
        let node = TextIr::Choose {
            when: Box::new(TextIr::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(integer(1)),
            otherwise: Box::new(effect),
        };
        assert!(!node.is_pure());
        assert_eq!(node.children().count(), 3);
    }
    let member = TextIr::Member {
        group: "g".into(),
        name: "m".into(),
        operation: "op".into(),
    };
    assert!(member.is_pure());
    assert_eq!(member.children().count(), 0);
}

#[test]
fn data_decode_and_previous_purity_observations_do_not_authorize_types_versions_or_dispatch() {
    let mut node = integer(1);
    assert!(node.is_pure());
    node = serde_json::from_str(r#"{"kind":"call","operation":"unknown","arguments":[]}"#).unwrap();
    assert!(!node.is_pure());
    let parameters: [NamedParameter<&str, ()>; 0] = [];
    let schemas = [OperationSchema {
        key: "known".to_string(),
        parameters: &parameters,
        result: (),
        required_capability: "edit",
    }];
    let catalog = OperationCatalog::new(
        1,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let TextIr::Call { operation, .. } = node else {
        panic!("expected call")
    };
    assert_eq!(
        catalog
            .authorize(operation.as_str(), 2, &["edit"])
            .unwrap_err(),
        OperationCatalogError::UnsupportedVersion
    );
    assert_eq!(
        catalog
            .authorize(operation.as_str(), 1, &["edit"])
            .unwrap_err(),
        OperationCatalogError::UnknownOperation
    );
    assert_eq!(
        catalog.authorize("known", 1, &[]).unwrap_err(),
        OperationCatalogError::CapabilityDenied
    );
    let invalid_type = TextIr::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(TextIr::Literal {
            value: ScalarValue::Boolean(true),
        }),
        right: Box::new(integer(1)),
    };
    assert!(invalid_type.is_pure());
    assert_eq!(
        BinaryOperator::Add.result_type(ScalarType::Boolean, ScalarType::Integer),
        None
    );
}

#[test]
fn reference_lowering_is_a_direct_shared_ir_specialization_with_no_tree_conversion() {
    use leselang_hir::{Effect, Type, host_call::HostOperation, result_field::ResultField};
    type ReferenceIr = Computation<ResultField, HostOperation, Effect, Type>;
    let program = leselang_hir::lower(&leselang_syntax::parse(
        r#"fn main() = ui.focus(node_id: concat(left: "tar", right: "get"))"#,
    ))
    .unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computation")
    };
    let shared: &ReferenceIr = expression;
    let legacy: &leselang_hir::computation::Computation = shared;
    assert!(std::ptr::eq(shared, legacy));
    let Computation::Call {
        operation,
        arguments,
    } = shared
    else {
        panic!("expected computed call")
    };
    assert_eq!(*operation, HostOperation::UiFocus);
    let legacy_arguments: &[leselang_hir::computation::ComputedArgument] = arguments;
    assert!(std::ptr::eq(legacy_arguments.as_ptr(), arguments.as_ptr()));
    assert_eq!(
        serde_json::to_string(shared).unwrap(),
        r#"{"kind":"call","operation":"ui.focus","arguments":[{"name":"node_id","value":{"kind":"binary","operator":"concat","left":{"kind":"literal","value":{"kind":"string","value":"tar"}},"right":{"kind":"literal","value":{"kind":"string","value":"get"}}}}]}"#
    );
    shared.validate_structure().unwrap();
    assert!(shared.validate_in_scope(&[]).is_ok());
    assert_eq!(
        shared
            .required_capabilities()
            .into_iter()
            .collect::<Vec<_>>(),
        ["ui.presentation"]
    );
}

#[test]
fn thread_transfer_is_conditional_on_host_slots_without_an_interpreter_global_lock() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let node = integer(1);
    assert_send_sync(&node);
    std::thread::scope(|scope| {
        scope.spawn(|| assert!(node.is_pure())).join().unwrap();
    });
}
