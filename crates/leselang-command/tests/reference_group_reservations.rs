use std::{cell::Cell, panic::AssertUnwindSafe, rc::Rc};

use leselang_command::reference_group_reservations::{
    ReferenceReservationSource, inspect_group_reservations,
};
use leselang_command::reference_merge::{invalid_group_binding, validate_merge_plan};
use leselang_command::{ExecutionOrder, GroupResultBinding, MergePlan, ResultBinding};
use leselang_runtime_core::{Fault, ScalarValue};
use leserpent_hir::{Effect, HirBranch, Type, computation::Computation};

fn plan(parallel: bool, created: usize) -> MergePlan {
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let branches = ["a", "b"]
        .into_iter()
        .map(|name| HirBranch {
            name: name.into(),
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
            result_type: Type::UiFocus,
        })
        .collect();
    let plan = MergePlan {
        branches: ["a", "b", "successor", "successor_1"][..2 + created]
            .iter()
            .map(|name| (*name).into())
            .collect(),
        order: if parallel {
            ExecutionOrder::ParallelGroupDataflow
        } else {
            ExecutionOrder::GroupDataflow
        },
        result_binding: Some(Box::new(GroupResultBinding {
            pending: if parallel {
                Effect::All { branches }
            } else {
                Effect::Sequence { steps: branches }
            },
            binding: Box::new(ResultBinding {
                name: "group".into(),
                locals: vec![],
                results: vec![],
                groups: vec![],
                body: Computation::Bind {
                    name: "first".into(),
                    value: Box::new(host.clone()),
                    body: Box::new(Computation::Bind {
                        name: "second".into(),
                        value: Box::new(host),
                        body: Box::new(Computation::Literal {
                            value: ScalarValue::None,
                        }),
                    }),
                },
            }),
            fuel_remaining: 100,
            successor_sequence: Some(100),
            additional_successor_sequences: vec![107],
        })),
    };
    validate_merge_plan(&plan).unwrap();
    plan
}

// Deliberately independent of the adapter and shared proof kernel.
fn frozen(plan: &MergePlan, source: &mut dyn ReferenceReservationSource) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    if binding.successor_sequence.is_some() {
        let watermark = source.allocation_watermark()?;
        for (index, sequence) in binding
            .successor_sequence
            .into_iter()
            .chain(binding.additional_successor_sequences.iter().copied())
            .enumerate()
        {
            let exists = source.effect_exists(&format!("continuation-{sequence}"))?;
            let aliases = source.group_exists(&format!("merge-{sequence}"))?;
            let branches = match &binding.pending {
                Effect::All { branches } | Effect::Sequence { steps: branches } => branches,
                _ => return Err(invalid_group_binding()),
            };
            let expected = index < plan.branches.len() - branches.len();
            if sequence == 0
                || watermark <= 0
                || sequence >= watermark as u64
                || exists != expected
                || aliases
            {
                return Err(invalid_group_binding());
            }
        }
    }
    Ok(())
}

struct Source<'a> {
    trace: &'a mut Vec<String>,
    watermark: i64,
    present: u8,
    aliases: u8,
    fail_at: usize,
    panic_at: usize,
    reads: Rc<Cell<usize>>,
}

impl Source<'_> {
    fn read(&mut self, key: String) -> Result<(), Fault> {
        self.trace.push(key);
        let position = self.trace.len();
        self.reads.set(self.reads.get() + 1);
        assert_ne!(position, self.panic_at, "original owner unwind");
        if position == self.fail_at {
            return Err(Fault {
                code: format!("native-{position}"),
                message: "original read fault".into(),
            });
        }
        Ok(())
    }
}

impl ReferenceReservationSource for Source<'_> {
    fn allocation_watermark(&mut self) -> Result<i64, Fault> {
        self.read("watermark".into())?;
        Ok(self.watermark)
    }
    fn effect_exists(&mut self, token: &str) -> Result<bool, Fault> {
        self.read(format!("effect:{token}"))?;
        Ok(self.present & if token == "continuation-100" { 1 } else { 2 } != 0)
    }
    fn group_exists(&mut self, token: &str) -> Result<bool, Fault> {
        self.read(format!("group:{token}"))?;
        Ok(self.aliases & if token == "merge-100" { 1 } else { 2 } != 0)
    }
}

#[test]
fn original_reservation_proofs_preserve_frozen_faults_reads_and_inputs() {
    let mut comparisons = 0;
    for parallel in [false, true] {
        for created in 0..=2 {
            let plan = plan(parallel, created);
            let original = plan.clone();
            for watermark in [i64::MIN, -1, 0, 100, 101, 107, 108, i64::MAX] {
                for present in 0..4 {
                    for aliases in 0..4 {
                        for fail_at in 0..=5 {
                            let mut traces = [vec![], vec![]];
                            let results = traces.each_mut().map(|trace| Source {
                                trace,
                                watermark,
                                present,
                                aliases,
                                fail_at,
                                panic_at: 0,
                                reads: Rc::default(),
                            });
                            let [mut actual, mut expected] = results;
                            assert_eq!(
                                inspect_group_reservations(&plan, &mut actual),
                                frozen(&plan, &mut expected)
                            );
                            assert_eq!(actual.trace, expected.trace);
                            assert_eq!(plan, original);
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 4608);
}

#[test]
fn reservations_are_observations_not_plan_admission_or_watermark_repair() {
    let mut trace = vec![];
    let mut source = Source {
        trace: &mut trace,
        watermark: 108,
        present: 0,
        aliases: 0,
        fail_at: 0,
        panic_at: 0,
        reads: Rc::default(),
    };
    let mut original = plan(false, 0);
    assert_eq!(inspect_group_reservations(&original, &mut source), Ok(()));
    assert_eq!(
        source.trace,
        &[
            "watermark",
            "effect:continuation-100",
            "group:merge-100",
            "effect:continuation-107",
            "group:merge-107",
        ]
    );
    // A later inspection must read the original watermark again, not cache success.
    source.trace.clear();
    source.watermark = 107;
    assert_eq!(
        inspect_group_reservations(&original, &mut source),
        Err(invalid_group_binding())
    );
    assert_eq!(source.trace.len(), 5);
    assert_eq!(source.watermark, 107);
    original.result_binding.as_mut().unwrap().successor_sequence = None;
    source.trace.clear();
    source.fail_at = 1;
    assert_eq!(inspect_group_reservations(&original, &mut source), Ok(()));
    assert!(source.trace.is_empty());
    assert!(validate_merge_plan(&original).is_err());
    original.result_binding = None;
    assert_eq!(inspect_group_reservations(&original, &mut source), Ok(()));
    assert!(source.trace.is_empty());
}

#[test]
fn malformed_shape_and_sequence_boundaries_fail_after_original_reads_without_panicking() {
    for defect in 0..6 {
        let mut plan = plan(false, 0);
        let binding = plan.result_binding.as_mut().unwrap();
        match defect {
            0 => binding.successor_sequence = Some(0),
            1 => binding.successor_sequence = Some(i64::MAX as u64),
            2 => binding.successor_sequence = Some(u64::MAX),
            3 => {
                binding.pending = Effect::UiFocus {
                    node_id: "bad".into(),
                }
            }
            4 => plan.branches.clear(),
            5 => binding.successor_sequence = Some(i64::MAX as u64 - 1),
            _ => unreachable!(),
        }
        let mut trace = vec![];
        let mut source = Source {
            trace: &mut trace,
            watermark: i64::MAX,
            present: 0,
            aliases: 0,
            fail_at: 0,
            panic_at: 0,
            reads: Rc::default(),
        };
        let result = inspect_group_reservations(&plan, &mut source);
        if defect == 5 {
            // Range admission alone is not ordering/uniqueness admission.
            assert_eq!(result, Ok(()));
            assert!(validate_merge_plan(&plan).is_err());
        } else {
            assert_eq!(result, Err(invalid_group_binding()));
            assert_eq!(source.trace.len(), 3);
            source.trace.clear();
            source.fail_at = 3;
            assert_eq!(
                inspect_group_reservations(&plan, &mut source)
                    .unwrap_err()
                    .code,
                "native-3"
            );
        }
    }
}

#[test]
fn local_unsized_source_unwinds_without_later_reads_or_cached_authority() {
    let plan = plan(true, 2);
    for panic_at in 1..=5 {
        let mut trace = vec![];
        let reads = Rc::new(Cell::new(0));
        let mut source = Source {
            trace: &mut trace,
            watermark: 108,
            present: 3,
            aliases: 0,
            fail_at: 0,
            panic_at,
            reads: reads.clone(),
        };
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let borrowed: &mut dyn ReferenceReservationSource = &mut source;
            inspect_group_reservations(&plan, borrowed)
        }));
        assert!(result.is_err());
        assert_eq!(source.trace.len(), panic_at);
        assert_eq!(reads.get(), panic_at);
        source.trace.clear();
        source.panic_at = 0;
        assert_eq!(inspect_group_reservations(&plan, &mut source), Ok(()));
        assert_eq!(source.trace.len(), 5);
        assert_eq!(reads.get(), panic_at + 5);
    }
}
