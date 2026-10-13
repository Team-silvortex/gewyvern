//! Saved operands are host data, not authority to replay a published request.
use leselang_hir::{
    call_evaluation::*,
    call_typing::CallTypeError,
    group_reply::assemble_group_reply,
    ir::{Computation, ComputedArgument, ComputedBranch, GroupKind},
    native_group_preparation::prepare_native_group_members,
    prepared_sequence::PreparedSequenceSession,
    pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits},
    sequence_evaluation::*,
};
use leselang_runtime_core::*;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct Domain<'a> {
    events: &'a RefCell<Vec<&'static str>>,
    allowed: bool,
    unwind: bool,
}
impl ScalarArgumentDomain for Domain<'_> {
    fn scalar_types(&self) -> ScalarTypeSet {
        self.events.borrow_mut().push("types");
        ScalarTypeSet::only(ScalarType::String)
    }
    fn accepts_literal(&self, _: &ScalarValue) -> bool {
        self.events.borrow_mut().push("literal");
        assert!(!self.unwind, "native domain unwind");
        self.allowed
    }
}
struct Opaque;
struct NativeError(Box<u8>);
fn row(
    parameter_index: usize,
    argument_index: usize,
    name: &str,
    value: ScalarValue,
) -> PreparedArgument<'_> {
    PreparedArgument {
        parameter_index,
        argument_index,
        name,
        value,
    }
}
fn text(value: &str) -> ScalarValue {
    ScalarValue::String(value.into())
}

#[test]
fn restored_operands_move_buffers_and_rebind_names_after_decoded_storage_is_dropped() {
    let events = RefCell::new(Vec::new());
    let domain = || Domain {
        events: &events,
        allowed: true,
        unwind: false,
    };
    let parameters = [
        NamedParameter::required("target", domain()),
        NamedParameter::optional("unused", domain()),
        NamedParameter::required("caption", domain()),
    ];
    let schema = OperationSchema {
        key: Opaque,
        parameters: &parameters,
        result: Opaque,
        required_capability: Opaque,
    };
    let (call, pointer) = {
        let names = [String::from("target"), String::from("caption")];
        let value = String::from("private caption");
        let pointer = value.as_ptr();
        let arguments = vec![
            row(0, 1, &names[0], text("window")),
            row(2, 0, &names[1], ScalarValue::String(value)),
        ];
        let call = PreparedCall::restore(&schema, arguments, 3, |selected, rows| {
            assert!(std::ptr::eq(selected, &schema));
            assert_eq!((rows[0].argument_index, rows[1].argument_index), (1, 0));
            events.borrow_mut().push("admit");
            Ok::<_, NativeError>(())
        })
        .unwrap();
        (call, pointer)
    };
    assert!(std::ptr::eq(call.schema(), &schema));
    assert_eq!(call.arguments().len(), 2);
    assert!(std::ptr::eq(
        call.arguments()[1].name.as_ptr(),
        parameters[2].name.as_ptr()
    ));
    let ScalarValue::String(value) = &call.arguments()[1].value else {
        panic!()
    };
    assert_eq!(value.as_ptr(), pointer);
    assert_eq!(
        *events.borrow(),
        ["admit", "types", "literal", "types", "literal"]
    );
    assert!(!format!("{call:?}").contains("private"));
}

#[test]
fn malformed_complete_rows_and_late_scalar_overflow_precede_all_native_callbacks() {
    for mode in 0..9 {
        let events = RefCell::new(Vec::new());
        let parameters = ["a", "b"].map(|name| {
            NamedParameter::required(
                name,
                Domain {
                    events: &events,
                    allowed: true,
                    unwind: false,
                },
            )
        });
        let schema = OperationSchema {
            key: (),
            parameters: &parameters,
            result: (),
            required_capability: (),
        };
        let mut arguments = vec![row(0, 1, "a", text("first")), row(1, 0, "b", text("last"))];
        match mode {
            0 => arguments[1].parameter_index = usize::MAX,
            1 => arguments[1].argument_index = usize::MAX,
            2 => arguments[1].argument_index = 1,
            3 => arguments.swap(0, 1),
            4 => arguments[1].name = "a",
            5 => arguments[1].name = "not-a-name",
            6 => {
                arguments.pop();
                arguments[0].argument_index = 0;
            }
            7 => arguments[1].value = ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
            8 => arguments[1].parameter_index = 0,
            _ => unreachable!(),
        }
        assert!(
            PreparedCall::restore(&schema, arguments, 2, |_, _| {
                events.borrow_mut().push("admit");
                Ok::<_, ()>(())
            })
            .is_err(),
            "mode {mode}"
        );
        assert!(events.borrow().is_empty(), "mode {mode}");
    }
}

#[test]
fn inclusive_sixty_four_operand_limit_and_empty_optional_calls_remain_valid() {
    let names: Vec<_> = (0..64).map(|i| format!("arg{i}")).collect();
    let parameters: Vec<_> = names
        .iter()
        .map(|name| {
            NamedParameter::required(name.as_str(), ScalarTypeSet::only(ScalarType::Integer))
        })
        .collect();
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let arguments = || {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| row(i, 63 - i, name, ScalarValue::Integer(i as u64)))
            .collect()
    };
    let call = PreparedCall::restore(&schema, arguments(), 64, |_, _| Ok::<_, ()>(())).unwrap();
    assert_eq!(call.arguments().len(), 64);
    assert!(matches!(
        PreparedCall::restore(&schema, arguments(), 65, |_, _| -> Result<(), ()> {
            panic!()
        }),
        Err(PreparedCallRestoreError::Preflight(
            CallTypeError::InvalidLimits
        ))
    ));
    assert!(matches!(
        PreparedCall::restore(&schema, arguments(), 63, |_, _| -> Result<(), ()> {
            panic!()
        }),
        Err(PreparedCallRestoreError::Preflight(
            CallTypeError::ArgumentLimit
        ))
    ));
    assert!(matches!(
        PreparedCall::restore(&schema, vec![], 63, |_, _| -> Result<(), ()> { panic!() }),
        Err(PreparedCallRestoreError::Preflight(
            CallTypeError::ParameterLimit
        ))
    ));
    let optional = [NamedParameter::optional(
        "absent",
        ScalarTypeSet::only(ScalarType::None),
    )];
    let schema = OperationSchema {
        key: (),
        parameters: &optional,
        result: (),
        required_capability: (),
    };
    assert!(
        PreparedCall::restore(&schema, vec![], 1, |_, rows| {
            assert!(rows.is_empty());
            Ok::<_, ()>(())
        })
        .unwrap()
        .arguments()
        .is_empty()
    );
    let empty = OperationSchema {
        key: (),
        parameters: &[] as &[NamedParameter<&str, ScalarTypeSet>],
        result: (),
        required_capability: (),
    };
    assert!(PreparedCall::restore(&empty, vec![], 0, |_, _| Ok::<_, ()>(())).is_ok());
}

#[test]
fn all_six_scalar_kinds_keep_their_original_values_without_coercion() {
    for value in [
        ScalarValue::Integer(7),
        ScalarValue::Boolean(true),
        text("seven"),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(Some("present".into()))),
        ScalarValue::StringList(StringListValue(vec!["a".into(), "b".into()])),
    ] {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(value.scalar_type()),
        )];
        let schema = OperationSchema {
            key: (),
            parameters: &parameters,
            result: (),
            required_capability: (),
        };
        let call = PreparedCall::restore(
            &schema,
            vec![row(0, 0, "value", value.clone())],
            1,
            |_, _| Ok::<_, ()>(()),
        )
        .unwrap();
        assert_eq!(call.arguments()[0].value, value);
    }
}

#[test]
fn live_admission_errors_move_verbatim_before_domain_checks_and_do_not_leak_payloads() {
    let events = RefCell::new(Vec::new());
    let parameters = [NamedParameter::required(
        "value",
        Domain {
            events: &events,
            allowed: false,
            unwind: false,
        },
    )];
    let schema = OperationSchema {
        key: (),
        parameters: &parameters,
        result: (),
        required_capability: (),
    };
    let error = NativeError(Box::new(42));
    let pointer = &*error.0 as *const u8;
    let failure = PreparedCall::restore(
        &schema,
        vec![row(0, 0, "value", text("secret"))],
        1,
        |_, _| Err(error),
    )
    .unwrap_err();
    assert!(!format!("{failure:?}").contains("secret"));
    let PreparedCallRestoreError::Native(error) = failure else {
        panic!()
    };
    assert!(std::ptr::eq(&*error.0, pointer));
    assert!(events.borrow().is_empty());
    assert!(matches!(
        PreparedCall::restore(
            &schema,
            vec![row(0, 0, "value", text("value"))],
            1,
            |_, _| Ok::<_, ()>(())
        ),
        Err(PreparedCallRestoreError::Argument {
            parameter_index: 0,
            argument_index: 0,
            error: ArgumentTypeError::InvalidLiteralDomain
        })
    ));
    assert_eq!(*events.borrow(), ["types", "literal"]);
    events.borrow_mut().clear();
    assert!(matches!(
        PreparedCall::restore(
            &schema,
            vec![row(0, 0, "value", ScalarValue::Integer(3))],
            1,
            |_, _| Ok::<_, ()>(())
        ),
        Err(PreparedCallRestoreError::Argument {
            error: ArgumentTypeError::TypeMismatch,
            ..
        })
    ));
    assert_eq!(*events.borrow(), ["types"]);
}

#[test]
fn native_unwind_and_cold_refusal_release_admission_owners_without_retry() {
    struct Owner(Rc<Cell<usize>>);
    impl Drop for Owner {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for mode in 0..3 {
        let events = RefCell::new(Vec::new());
        let dropped = Rc::new(Cell::new(0));
        let owner = Owner(dropped.clone());
        let parameters = [NamedParameter::required(
            "value",
            Domain {
                events: &events,
                allowed: true,
                unwind: mode == 2,
            },
        )];
        let schema = OperationSchema {
            key: (),
            parameters: &parameters,
            result: (),
            required_capability: (),
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            PreparedCall::restore(
                &schema,
                vec![row(
                    0,
                    0,
                    if mode == 0 { "wrong" } else { "value" },
                    text("v"),
                )],
                1,
                move |_, _| {
                    let _owner = owner;
                    assert!(mode != 1, "native admission unwind");
                    Ok::<_, ()>(())
                },
            )
        }));
        if mode == 0 {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        assert_eq!(dropped.get(), 1);
        assert_eq!(events.borrow().len(), if mode == 2 { 2 } else { 0 });
    }
}

mod queued {
    use super::*;
    type Node = Computation<(), String, (), ()>;
    type Schema<'a> = OperationSchema<'a, String, &'a str, ScalarTypeSet, ResultDomain, ()>;
    type Call<'a> = PreparedCall<'a, String, ScalarTypeSet, ResultDomain, ()>;
    type Queue<'a, Request> = PreparedSequenceSession<'a, Node, (), u64, ResultDomain, Request>;
    const LIMITS: SequenceEvaluationLimits = SequenceEvaluationLimits {
        max_nodes: 64,
        max_depth: 8,
        max_branches: 3,
    };
    const CALL: CallEvaluationLimits = CallEvaluationLimits {
        pure: PureEvaluationLimits {
            max_nodes: 16,
            max_depth: 8,
            max_bindings: 4,
        },
        max_arguments: 1,
    };

    #[derive(Clone, Copy, Serialize, Deserialize)]
    enum Profile {
        Counter,
        Panel,
    }
    impl Profile {
        fn operation(self) -> &'static str {
            match self {
                Self::Counter => "counter.write",
                Self::Panel => "panel.set_text",
            }
        }
        fn scalar(self) -> ScalarType {
            match self {
                Self::Counter => ScalarType::Integer,
                Self::Panel => ScalarType::String,
            }
        }
        fn value(self, index: usize) -> ScalarValue {
            match self {
                Self::Counter => ScalarValue::Integer(index as u64 + 10),
                Self::Panel => text(&format!("panel-{index}")),
            }
        }
    }
    struct ResultDomain(ScalarType);
    impl HostResultDomain<ScalarValue> for ResultDomain {
        type Error = &'static str;
        fn matches_type(&self, value: &ScalarValue) -> bool {
            value.scalar_type() == self.0
        }
        fn validate_value(&self, value: &ScalarValue) -> Result<(), Self::Error> {
            value.is_bounded().then_some(()).ok_or("bounds")
        }
    }
    #[derive(Default)]
    struct Service {
        prepared: Cell<usize>,
        restored: Cell<usize>,
        invoked: RefCell<Vec<ScalarValue>>,
        denied: Cell<bool>,
    }
    impl ReplyAuthority<u64> for Service {
        type Error = &'static str;
        fn authorize(&self, id: &u64) -> Result<(), Self::Error> {
            (!self.denied.get() && (1..=3).contains(id))
                .then_some(())
                .ok_or("authority")
        }
    }
    impl Service {
        fn invoke(&self, id: u64, request: Call<'_>) -> ScalarValue {
            self.authorize(&id).unwrap();
            let (_, mut arguments) = request.into_parts();
            let value = arguments.pop().unwrap().value;
            self.invoked.borrow_mut().push(value.clone());
            value
        }
    }
    struct Environment;
    impl PureEvaluationEnvironment<(), String> for Environment {
        type Result = ();
        type Error = &'static str;
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, Self::Error> {
            Err("field")
        }
        fn member(&self, _: &(), _: &str, _: &String) -> Result<(), Self::Error> {
            Err("member")
        }
    }
    fn schema<T>(profile: Profile, run: impl FnOnce(&Schema<'_>) -> T) -> T {
        let name = String::from("value");
        let parameters = [NamedParameter::required(
            name.as_str(),
            ScalarTypeSet::only(profile.scalar()),
        )];
        let schema = OperationSchema {
            key: profile.operation().into(),
            parameters: &parameters,
            result: ResultDomain(profile.scalar()),
            required_capability: (),
        };
        run(&schema)
    }
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Operand {
        name: String,
        parameter_index: usize,
        argument_index: usize,
        value: ScalarValue,
    }
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct SavedCall {
        operation: String,
        arguments: Vec<Operand>,
    }
    impl SavedCall {
        fn save(call: Call<'_>) -> Self {
            let (schema, arguments) = call.into_parts();
            Self {
                operation: schema.key.clone(),
                arguments: arguments
                    .into_iter()
                    .map(|row| Operand {
                        name: row.name.into(),
                        parameter_index: row.parameter_index,
                        argument_index: row.argument_index,
                        value: row.value,
                    })
                    .collect(),
            }
        }
        fn restore<'a>(
            self,
            schema: &'a Schema<'a>,
            service: &Service,
            id: u64,
        ) -> Result<Call<'a>, &'static str> {
            let (names, values): (Vec<_>, Vec<_>) = self
                .arguments
                .into_iter()
                .map(|row| {
                    (
                        row.name,
                        (row.parameter_index, row.argument_index, row.value),
                    )
                })
                .unzip();
            let arguments = names
                .iter()
                .zip(values)
                .map(|(name, (p, a, value))| row(p, a, name, value))
                .collect();
            PreparedCall::restore(schema, arguments, 1, |selected, _| {
                service.authorize(&id)?;
                service.restored.set(service.restored.get() + 1);
                (selected.key == self.operation)
                    .then_some(())
                    .ok_or("operation")
            })
            .map_err(|_| "operands")
        }
    }
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "state", content = "data", deny_unknown_fields)]
    enum SavedRow {
        Accepted(ScalarValue),
        Awaiting(ScalarValue),
        Prepared(SavedCall),
    }
    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Image {
        node: Node,
        rows: Vec<SavedRow>,
        remaining: u64,
    }

    fn save(profile: Profile, cut: usize, service: &Service) -> Vec<u8> {
        schema(profile, |schema| {
            let branches = (0..3)
                .map(|i| ComputedBranch {
                    name: format!("member{i}"),
                    result_type: (),
                    value: Node::Call {
                        operation: schema.key.clone(),
                        arguments: vec![ComputedArgument {
                            name: "value".into(),
                            value: Node::Literal {
                                value: profile.value(i),
                            },
                        }],
                    },
                })
                .collect();
            let node = Node::Group {
                group_kind: GroupKind::Sequence,
                branches,
            };
            let Node::Group { branches, .. } = &node else {
                unreachable!()
            };
            let mut fuel = Fuel::new(100);
            let prepared = prepare_native_group_members(
                GroupKind::Sequence,
                branches,
                3,
                &mut fuel,
                |row| Ok::<_, &'static str>(&row.name),
                |_| Ok(()),
                |_, branch, fuel| {
                    let Node::Call { arguments, .. } = &branch.value else {
                        return Err("call");
                    };
                    service.prepared.set(service.prepared.get() + 1);
                    evaluate_call_arguments_in_scope(
                        arguments,
                        schema,
                        &mut ScopeFrame::new(&mut vec![]),
                        &Environment,
                        fuel,
                        CALL,
                    )
                    .map_err(|_| "prepare")
                },
            )
            .unwrap();
            let mut inventory = Vec::new();
            let mut queue = Queue::start(
                &node,
                prepared,
                &mut fuel,
                LIMITS,
                |_, _, _| Ok::<_, &'static str>(()),
                |index, _, call, _| {
                    inventory.push(Some(call));
                    Ok(PreparedSequenceMember {
                        identity: index as u64 + 1,
                        declaration: &schema.result,
                        request: index,
                    })
                },
            )
            .unwrap();
            let prefix = cut / 2;
            let mut rows = Vec::new();
            for index in 0..3 {
                if index < prefix || (index == prefix && cut % 2 == 1) {
                    let SequencePoll::Request {
                        index: actual,
                        request,
                    } = queue.poll(|_, _, id, _, _| service.authorize(id)).unwrap()
                    else {
                        panic!()
                    };
                    assert_eq!(actual, index);
                    let reply =
                        service.invoke(index as u64 + 1, inventory[request].take().unwrap());
                    if index < prefix {
                        let row = queue
                            .try_accept(&(index as u64 + 1), reply, service)
                            .unwrap();
                        rows.push(SavedRow::Accepted(row.into_parts().4));
                    } else {
                        rows.push(SavedRow::Awaiting(reply));
                    }
                } else {
                    rows.push(SavedRow::Prepared(SavedCall::save(
                        inventory[index].take().unwrap(),
                    )));
                }
            }
            drop(queue);
            assert!(inventory.iter().all(Option::is_none));
            let image = Image {
                node,
                rows,
                remaining: fuel.remaining(),
            };
            let bytes = serde_json::to_vec(&image).unwrap();
            assert!(bytes.len() < 64 * 1024);
            bytes
        })
    }

    fn restore(
        profile: Profile,
        committed: &[u8],
        candidate: &[u8],
        service: &Service,
        revoke_after_admission: bool,
    ) -> Result<u64, &'static str> {
        // Fixture store correspondence, not a production signature or crash journal.
        if candidate.len() > 64 * 1024 || candidate != committed {
            return Err("original");
        }
        let image: Image = serde_json::from_slice(candidate).map_err(|_| "decode")?;
        schema(profile, |schema| {
            let Node::Group { branches, .. } = &image.node else {
                return Err("group");
            };
            if branches.len() != 3 || image.rows.len() != branches.len() || image.remaining > 100 {
                return Err("shape or budget");
            }
            let mut inbox = None;
            let mut saved = Vec::new();
            for (index, state) in image.rows.into_iter().enumerate() {
                let identity = index as u64 + 1;
                let reply = match state {
                    SavedRow::Prepared(call) => {
                        SavedSequenceReply::Prepared(PreparedSequenceMember {
                            identity,
                            declaration: &schema.result,
                            request: call.restore(schema, service, identity)?,
                        })
                    }
                    SavedRow::Awaiting(reply) => {
                        inbox = Some(reply);
                        SavedSequenceReply::Awaiting {
                            identity,
                            declaration: &schema.result,
                        }
                    }
                    SavedRow::Accepted(reply) => SavedSequenceReply::Accepted(
                        PendingReply::new(identity, (), &schema.result)
                            .try_accept(&identity, reply, service)
                            .map_err(|_| "prefix")?,
                    ),
                };
                saved.push(SavedSequenceMember {
                    index,
                    name: &branches[index].name,
                    reply,
                });
            }
            let (mut queue, mut rows) = Queue::restore(&image.node, saved, LIMITS, |snapshot| {
                for row in snapshot {
                    service.authorize(row.reply.identity())?;
                }
                Ok::<_, &'static str>(())
            })
            .map_err(|_| "frontier")?;
            service
                .denied
                .set(revoke_after_admission || service.denied.get());
            for index in rows.len()..3 {
                let reply = match queue
                    .poll(|_, _, id, domain, request| {
                        service.authorize(id)?;
                        std::ptr::eq(domain, &request.schema().result)
                            .then_some(())
                            .ok_or("domain")
                    })
                    .map_err(|_| "publish")?
                {
                    SequencePoll::Awaiting { index: actual } => {
                        assert_eq!(actual, index);
                        inbox.take().ok_or("inbox")?
                    }
                    SequencePoll::Request {
                        index: actual,
                        request,
                    } => {
                        assert_eq!(actual, index);
                        service.invoke(index as u64 + 1, request)
                    }
                    _ => return Err("frontier"),
                };
                rows.push(
                    queue
                        .try_accept(&(index as u64 + 1), reply, service)
                        .map_err(|_| "reply")?,
                );
            }
            assert!(matches!(
                queue
                    .poll(|_, _, _, _, _| Ok::<_, &'static str>(()))
                    .unwrap(),
                SequencePoll::Terminal(SequenceEnd::Completed)
            ));
            let mut fuel = Fuel::new(image.remaining);
            let group = assemble_group_reply(&image.node, rows, LIMITS, &mut fuel, |_, _| {
                service.authorize(&1)
            })
            .map_err(|_| "assemble")?;
            for (index, row) in group.members().iter().enumerate() {
                assert_eq!(*row.as_parts().4, profile.value(index));
            }
            assert_eq!(fuel.remaining(), image.remaining - 4);
            Ok(fuel.remaining())
        })
    }

    #[test]
    fn two_byte_codec_hosts_restore_real_prepared_calls_after_all_original_objects_are_dropped() {
        for profile in [Profile::Counter, Profile::Panel] {
            let mut remaining = None;
            for cut in 0..7 {
                let service = Service::default();
                let bytes = save(profile, cut, &service);
                let result = restore(profile, &bytes, &bytes, &service, false).unwrap();
                assert_eq!(service.prepared.get(), 3);
                assert_eq!(service.restored.get(), 3 - cut.div_ceil(2));
                assert_eq!(
                    *service.invoked.borrow(),
                    (0..3).map(|i| profile.value(i)).collect::<Vec<_>>()
                );
                assert_eq!(*remaining.get_or_insert(result), result);
            }
        }
    }

    #[test]
    fn decoded_calls_still_need_current_authority_at_restore_and_queued_publication() {
        for profile in [Profile::Counter, Profile::Panel] {
            for after_admission in [false, true] {
                let service = Service::default();
                let bytes = save(profile, 2, &service);
                service.denied.set(!after_admission);
                assert!(restore(profile, &bytes, &bytes, &service, after_admission).is_err());
                assert_eq!(service.prepared.get(), 3);
                assert_eq!(service.restored.get(), if after_admission { 2 } else { 0 });
                assert_eq!(service.invoked.borrow().len(), 1);
            }
        }
    }

    #[test]
    fn forged_saved_operands_and_fuel_do_not_enter_native_restoration_or_dispatch() {
        for profile in [Profile::Counter, Profile::Panel] {
            for mode in 0..3 {
                let service = Service::default();
                let bytes = save(profile, 0, &service);
                let mut image: Image = serde_json::from_slice(&bytes).unwrap();
                match mode {
                    0 => image.remaining += 1,
                    1 => {
                        if let SavedRow::Prepared(call) = &mut image.rows[2] {
                            call.operation = "other".into();
                        }
                    }
                    2 => {
                        if let SavedRow::Prepared(call) = &mut image.rows[2] {
                            call.arguments[0].value = profile.value(0);
                        }
                    }
                    _ => unreachable!(),
                }
                assert_eq!(
                    restore(
                        profile,
                        &bytes,
                        &serde_json::to_vec(&image).unwrap(),
                        &service,
                        false
                    ),
                    Err("original")
                );
                assert_eq!(service.prepared.get(), 3);
                assert_eq!(service.restored.get(), 0);
                assert!(service.invoked.borrow().is_empty());
            }
        }
    }
}
