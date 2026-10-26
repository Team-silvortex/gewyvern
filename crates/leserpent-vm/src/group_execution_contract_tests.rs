use super::*;
use crate::ScalarValue;
use leserpent_hir::computation::Computation;

fn fixture(parallel: bool, kind: usize) -> GroupResultBinding {
    let (mut group, _) = super::successor_contract_tests::fixture(parallel, kind.clamp(1, 4));
    group.binding.name = "group".into();
    if kind == 0 {
        group.binding.body = Computation::Literal {
            value: ScalarValue::Integer(7),
        };
        group.successor_sequence = None;
        group.additional_successor_sequences.clear();
    }
    if kind == 5 {
        let Computation::Choose { when, .. } = &mut group.binding.body else {
            panic!()
        };
        **when = Computation::Literal {
            value: ScalarValue::Boolean(false),
        };
    }
    group
}

fn first() -> EffectRequest {
    crate::Vm::build_effect_request(
        &Effect::UiFocus {
            node_id: "a".into(),
        },
        Type::UiFocus,
        1,
        crate::Principal::new("original-operator").unwrap(),
        crate::CapabilitySet::new(["ui.presentation"]),
        Some(crate::Revision(42)),
        30_000,
        Some(31_000),
        500,
    )
    .unwrap()
}
fn prefix() -> Value {
    Value::Structured {
        fields: vec![crate::StructuredField {
            name: "a".into(),
            value: Box::new(Value::UiFocus {
                node_id: "a".into(),
            }),
        }],
    }
}
fn comparable(result: Result<CaptureProgress, Fault>) -> Result<Step, Fault> {
    result.map(|value| match value {
        CaptureProgress::Done(value) => Step::Done(Value::Scalar { value }),
        CaptureProgress::Pending(request) => Step::Effect(request),
    })
}

#[test]
fn group_execution_keeps_frozen_initial_and_scalar_finish_at_every_fuel_boundary() {
    let owner = ContinuationToken::from_wire("merge-1".into());
    let first = first();
    let value = prefix();
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 0..=5 {
            for remaining in 0..=80 {
                let mut group = fixture(parallel, kind);
                group.fuel_remaining = remaining;
                let before = serde_json::to_vec(&(&group, &first, &value)).unwrap();
                let actual = comparable(group.prepare_initial(&owner, &first, &value));
                let expected =
                    comparable(FrozenGroup(&group).prepare_initial(&owner, &first, &value));
                assert_eq!(
                    actual, expected,
                    "parallel={parallel}, kind={kind}, fuel={remaining}"
                );
                assert_eq!(
                    serde_json::to_vec(&actual).unwrap(),
                    serde_json::to_vec(&expected).unwrap()
                );
                if remaining == 80 && kind != 0 {
                    assert!(actual.is_ok(), "{actual:?}");
                }
                let actual = group.finish(&value);
                assert_eq!(actual, FrozenGroup(&group).finish(&value));
                if remaining == 80 && (kind == 0 || kind == 5) {
                    assert!(matches!(actual, Step::Done(_)));
                }
                assert_eq!(
                    serde_json::to_vec(&(&group, &first, &value)).unwrap(),
                    before
                );
                comparisons += 2;
            }
        }
    }
    assert_eq!(comparisons, 1944);
}

#[test]
fn group_capture_execution_keeps_frozen_saved_fuel_prefix_and_successor_pair_faults() {
    let owner = ContinuationToken::from_wire("merge-1".into());
    let first = first();
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 2..=4 {
            let mut group = fixture(parallel, kind);
            group.fuel_remaining = 500;
            let CaptureProgress::Pending(next) =
                group.prepare_initial(&owner, &first, &prefix()).unwrap()
            else {
                panic!()
            };
            for remaining in 0..=80 {
                for defect in 0..6 {
                    let mut requests = vec![first.clone(), (*next).clone()];
                    requests[1].continuation.fuel_remaining = remaining;
                    requests[1].budget.fuel_remaining = remaining;
                    let mut value = prefix();
                    let Value::Structured { fields } = &mut value else {
                        panic!()
                    };
                    fields.push(crate::StructuredField {
                        name: "successor".into(),
                        value: Box::new(Value::UiFocus {
                            node_id: "next".into(),
                        }),
                    });
                    match defect {
                        0 => {}
                        1 => fields[0].name = "wrong".into(),
                        2 => {
                            requests[0].continuation.pending_effect =
                                Effect::All { branches: vec![] }
                        }
                        3 => requests[1].continuation.result_binding = None,
                        4 => {
                            requests[1].continuation.pending_effect =
                                Effect::All { branches: vec![] }
                        }
                        5 => requests[1].budget.fuel_remaining += 1,
                        _ => unreachable!(),
                    }
                    let before = serde_json::to_vec(&(&group, &requests, &value)).unwrap();
                    let actual = comparable(group.advance_capture(&owner, &requests, &value));
                    let expected =
                        comparable(FrozenGroup(&group).advance_capture(&owner, &requests, &value));
                    assert_eq!(
                        actual, expected,
                        "parallel={parallel}, kind={kind}, fuel={remaining}, defect={defect}"
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&expected).unwrap()
                    );
                    if defect == 0 && remaining == 80 {
                        assert!(actual.is_ok(), "{actual:?}");
                    }
                    assert_eq!(
                        serde_json::to_vec(&(&group, &requests, &value)).unwrap(),
                        before
                    );
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 2916);
}

struct FrozenGroup<'a>(&'a GroupResultBinding);
impl std::ops::Deref for FrozenGroup<'_> {
    type Target = GroupResultBinding;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

// Frozen coordination. Existing evaluator/build/proof boundaries remain shared.
impl FrozenGroup<'_> {
    fn prepare_initial(
        &self,
        owner: &ContinuationToken,
        first: &EffectRequest,
        value: &Value,
    ) -> Result<CaptureProgress, Fault> {
        let mut fuel = Fuel::new(self.fuel_remaining);
        let (effect, binding) = match crate::computation::resume_group_outcome(
            &self.binding,
            value,
            self.branches()?,
            &mut fuel,
        )? {
            crate::computation::Outcome::Scalar(value) if self.is_conditional() => {
                return Ok(CaptureProgress::Done(value));
            }
            crate::computation::Outcome::Host(effect) if !self.captures_successor() => {
                (effect, None)
            }
            crate::computation::Outcome::BoundHost { effect, binding }
                if self.captures_successor() =>
            {
                (effect, Some(binding))
            }
            _ => return Err(invalid()),
        };
        if fuel.remaining() == 0 {
            return Err(Fault {
                code: "LSV1001".into(),
                message: "execution fuel exhausted before group successor".into(),
            });
        }
        self.build_successor(owner, first, &effect, binding, fuel.remaining(), 0)
            .map(|request| CaptureProgress::Pending(Box::new(request)))
    }

    fn advance_capture(
        &self,
        owner: &ContinuationToken,
        requests: &[EffectRequest],
        value: &Value,
    ) -> Result<CaptureProgress, Fault> {
        let Value::Structured { fields } = value else {
            return Err(invalid());
        };
        let (tail, prefix) = fields.split_last().ok_or_else(invalid)?;
        let request = requests.last().ok_or_else(invalid)?;
        let operations = requests
            .iter()
            .take(prefix.len())
            .map(|request| {
                HostOperation::for_effect(&request.continuation.pending_effect).ok_or_else(invalid)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.validate_saved_capture(request, prefix, &operations)?;
        let image = &request.continuation;
        let saved = image.result_binding.as_ref().ok_or_else(invalid)?;
        let operation = HostOperation::for_effect(&image.pending_effect).ok_or_else(invalid)?;
        let mut fuel = Fuel::new(image.fuel_remaining);
        match crate::computation::resume_outcome(saved, &tail.value, operation, &mut fuel)? {
            crate::computation::Outcome::Scalar(value) => Ok(CaptureProgress::Done(value)),
            crate::computation::Outcome::BoundHost { effect, binding }
                if self.is_dataflow() || self.is_conditional() =>
            {
                if fuel.remaining() == 0 {
                    return Err(Fault {
                        code: "LSV1001".into(),
                        message: "execution fuel exhausted before group successor".into(),
                    });
                }
                let next = self.build_successor(
                    owner,
                    requests.first().ok_or_else(invalid)?,
                    &effect,
                    Some(binding),
                    fuel.remaining(),
                    requests.len() - self.branches()?.len(),
                )?;
                crate::successor::validate_pair(
                    request,
                    &Step::Done((*tail.value).clone()),
                    &next,
                )?;
                Ok(CaptureProgress::Pending(Box::new(next)))
            }
            _ => Err(invalid()),
        }
    }

    fn finish(&self, value: &Value) -> Step {
        let mut fuel = Fuel::new(self.fuel_remaining);
        match self.branches().and_then(|branches| {
            crate::computation::resume_group(&self.binding, value, branches, &mut fuel)
        }) {
            Ok(value) => Step::Done(Value::Scalar { value }),
            Err(error) => Step::Fault(error),
        }
    }
}
