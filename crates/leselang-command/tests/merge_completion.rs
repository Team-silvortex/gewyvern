use leselang_command::merge_completion::*;
use leselang_command::{ExecutionOrder, GroupResultBinding, MergePlan, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct Native {
    id: u8,
    drops: Rc<Cell<usize>>,
}

impl Drop for Native {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

enum Outcome {
    Value(usize, Value),
    Stop(Box<Native>),
    Invalid,
}

#[derive(Default)]
struct Host {
    calls: RefCell<Vec<&'static str>>,
    error: RefCell<Option<Box<Native>>>,
}

impl MergeCompletionHost for Host {
    type Completion = (String, Outcome);
    type Outcome = Outcome;
    type Terminal = Box<Native>;
    type Error = Box<Native>;
    fn into_parts(completion: Self::Completion) -> (String, Outcome) {
        completion
    }
    fn validate_outcome(&self, outcome: &Outcome) -> Result<usize, Box<Native>> {
        self.calls.borrow_mut().push("outcome");
        match outcome {
            Outcome::Value(items, _) => Ok(*items),
            Outcome::Stop(_) => Ok(0),
            Outcome::Invalid => Err(self.error.borrow_mut().take().unwrap()),
        }
    }
    fn validate_member(
        &self,
        _: &GroupResultBinding,
        _: &str,
        _: &Outcome,
    ) -> Result<(), Box<Native>> {
        self.calls.borrow_mut().push("member");
        Ok(())
    }
    fn into_value_or_terminal(outcome: Outcome) -> MergedCompletion<Box<Native>> {
        match outcome {
            Outcome::Value(_, value) => MergedCompletion::Value(value),
            Outcome::Stop(stop) => MergedCompletion::Terminal(stop),
            Outcome::Invalid => panic!("invalid data cannot reach assembly"),
        }
    }
}

fn value(name: &str, items: usize) -> (String, Outcome) {
    (
        name.into(),
        Outcome::Value(
            items,
            Value::UiFocus {
                node_id: name.into(),
            },
        ),
    )
}

#[test]
fn move_only_terminals_follow_declaration_after_all_outcomes_are_admitted() {
    let drops = Rc::new(Cell::new(0));
    let first = Box::new(Native {
        id: 1,
        drops: drops.clone(),
    });
    let original = &*first as *const Native;
    let second = Box::new(Native {
        id: 2,
        drops: drops.clone(),
    });
    let host = Host::default();
    let result = merge_preflighted_completions(
        &MergePlan::new(["a", "b"]).unwrap(),
        vec![
            ("b".into(), Outcome::Stop(second)),
            ("a".into(), Outcome::Stop(first)),
        ],
        0,
        10,
        true,
        &host,
    );
    let Ok(MergedCompletion::Terminal(stop)) = result else {
        panic!("expected original first stop");
    };
    assert_eq!(host.calls.borrow().len(), 2);
    assert_eq!(stop.id, 1);
    assert_eq!(&*stop as *const Native, original);
    assert_eq!(drops.get(), 1);
    drop(stop);
    assert_eq!(drops.get(), 2);
}

#[test]
fn late_native_refusal_is_not_hidden_by_an_earlier_stop_or_output_overflow() {
    for stopped in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let refusal = Box::new(Native {
            id: 9,
            drops: drops.clone(),
        });
        let original = &*refusal as *const Native;
        let host = Host {
            calls: RefCell::new(vec![]),
            error: RefCell::new(Some(refusal)),
        };
        let first = if stopped {
            (
                "a".into(),
                Outcome::Stop(Box::new(Native {
                    id: 1,
                    drops: drops.clone(),
                })),
            )
        } else {
            value("a", usize::MAX)
        };
        let result = merge_preflighted_completions(
            &MergePlan::new(["a", "b"]).unwrap(),
            vec![first, ("b".into(), Outcome::Invalid)],
            0,
            10,
            true,
            &host,
        );
        let Err(MergeCompletionError::Native(error)) = result else {
            panic!("expected native refusal");
        };
        assert_eq!(host.calls.borrow().len(), 2);
        assert_eq!(&*error as *const Native, original);
        assert_eq!(drops.get(), usize::from(stopped));
        drop(error);
        assert_eq!(drops.get(), usize::from(stopped) + 1);
    }
}

#[test]
fn aggregate_counts_saturate_and_overflow_precedes_terminal_choice() {
    let drops = Rc::new(Cell::new(0));
    let result = merge_preflighted_completions(
        &MergePlan::new(["stop", "a", "b"]).unwrap(),
        vec![
            (
                "stop".into(),
                Outcome::Stop(Box::new(Native {
                    id: 1,
                    drops: drops.clone(),
                })),
            ),
            value("a", usize::MAX),
            value("b", 1),
        ],
        1,
        10,
        true,
        &Host::default(),
    );
    assert!(matches!(
        result,
        Err(MergeCompletionError::OutputItems {
            items: usize::MAX,
            limit: 1
        })
    ));
    assert_eq!(drops.get(), 1);
}

#[test]
fn budget_count_and_names_refuse_before_unreachable_native_hooks() {
    let plan = MergePlan::new(["a", "b"]).unwrap();
    let host = Host::default();
    assert!(matches!(
        merge_preflighted_completions(&plan, vec![], 11, 10, true, &host),
        Err(MergeCompletionError::OutputBudget)
    ));
    assert!(matches!(
        merge_preflighted_completions(&plan, vec![], 10, 10, true, &host),
        Err(MergeCompletionError::Incomplete)
    ));
    assert!(host.calls.borrow().is_empty());
    for name in ["unknown", "invalid name", ""] {
        assert!(matches!(
            merge_preflighted_completions(
                &plan,
                vec![(name.into(), Outcome::Invalid), value("b", 1)],
                10,
                10,
                true,
                &host
            ),
            Err(MergeCompletionError::Mismatch)
        ));
        assert!(host.calls.borrow().is_empty());
    }
    assert!(matches!(
        merge_preflighted_completions(
            &plan,
            vec![value("a", 1), ("a".into(), Outcome::Invalid)],
            10,
            10,
            true,
            &host
        ),
        Err(MergeCompletionError::Mismatch)
    ));
    assert_eq!(host.calls.borrow().len(), 1);
}

#[test]
fn source_order_and_prefix_mode_remain_distinct_from_final_chain_selection() {
    for order in [
        ExecutionOrder::Parallel,
        ExecutionOrder::Sequential,
        ExecutionOrder::ResultChain,
        ExecutionOrder::Dataflow,
    ] {
        let names = match order {
            ExecutionOrder::ResultChain => ["result", "successor"],
            ExecutionOrder::Dataflow => ["step_1", "step_2"],
            _ => ["a", "b"],
        };
        let plan = MergePlan {
            branches: names.map(str::to_string).to_vec(),
            order,
            result_binding: None,
        };
        plan.validate_shape(leselang_command::MergePlanShapeLimits {
            max_branches: 64,
            max_name_bytes: 64,
        })
        .unwrap();
        for finish in [false, true] {
            let result = merge_preflighted_completions(
                &plan,
                vec![value(names[1], 1), value(names[0], 1)],
                2,
                10,
                finish,
                &Host::default(),
            );
            let Ok(MergedCompletion::Value(value)) = result else {
                panic!("expected a value");
            };
            if finish
                && matches!(
                    order,
                    ExecutionOrder::ResultChain | ExecutionOrder::Dataflow
                )
            {
                assert_eq!(
                    value,
                    Value::UiFocus {
                        node_id: names[1].into()
                    }
                );
            } else {
                let Value::Structured { fields } = value else {
                    panic!("expected all declared fields");
                };
                assert_eq!(
                    fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
                    names
                );
            }
        }
    }
}
