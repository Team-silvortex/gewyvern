use super::*;
use leselang_hir::computation::Computation;
use serde::{Deserialize, Serialize};

// Frozen released group data and protocol rules, independent of adapter implementation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FrozenGroupResultBinding {
    pub pending: Effect,
    pub binding: Box<ResultBinding>,
    pub fuel_remaining: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub successor_sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_successor_sequences: Vec<u64>,
}

impl FrozenGroupResultBinding {
    fn is_conditional(&self) -> bool {
        !self.binding.body.is_pure()
            && !self.binding.body.is_result_chain()
            && self.binding.body.is_result_flow()
    }

    fn is_dataflow(&self) -> bool {
        self.binding.body.is_result_chain()
            && self
                .binding
                .body
                .atomic_chain_bound()
                .is_some_and(|bound| bound > 1)
    }

    fn captures_successor(&self) -> bool {
        self.binding.body.is_atomic_capture() || self.is_dataflow() || self.is_conditional()
    }

    fn reserved_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.successor_sequence
            .into_iter()
            .chain(self.additional_successor_sequences.iter().copied())
    }

    fn continuation_schema(&self) -> u32 {
        if self.is_conditional() {
            11
        } else if self.is_dataflow() {
            10
        } else if self.successor_sequence.is_some() && matches!(self.pending, Effect::All { .. }) {
            9
        } else if self.captures_successor() {
            8
        } else if self.successor_sequence.is_some() {
            7
        } else {
            6
        }
    }
}

fn bodies() -> Vec<Computation> {
    let pure = Computation::Literal {
        value: crate::ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let capture = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let chain = Computation::Bind {
        name: "first".into(),
        value: Box::new(host.clone()),
        body: Box::new(capture.clone()),
    };
    let conditional = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: crate::ScalarValue::Boolean(false),
        }),
        then: Box::new(capture.clone()),
        otherwise: Box::new(pure.clone()),
    };
    let not_a_flow = Computation::Choose {
        when: Box::new(host.clone()),
        then: Box::new(pure.clone()),
        otherwise: Box::new(pure.clone()),
    };
    vec![pure, host, capture, chain, conditional, not_a_flow]
}

#[test]
fn group_protocol_keeps_frozen_bytes_classification_and_unchecked_reservations() {
    let mut comparisons = 0;
    for body in bodies() {
        for pending in [
            Effect::All { branches: vec![] },
            Effect::Sequence { steps: vec![] },
            Effect::UiFocus {
                node_id: "not-a-group".into(),
            },
        ] {
            for successor_sequence in [None, Some(0), Some(1), Some(u64::MAX)] {
                for additional_successor_sequences in [vec![], vec![0], vec![2, 1], vec![u64::MAX]]
                {
                    let actual = GroupResultBinding {
                        pending: pending.clone(),
                        binding: Box::new(ResultBinding {
                            name: "result".into(),
                            locals: vec![],
                            results: vec![],
                            groups: vec![],
                            body: body.clone(),
                        }),
                        fuel_remaining: u64::MAX,
                        successor_sequence,
                        additional_successor_sequences,
                    };
                    let frozen = FrozenGroupResultBinding {
                        pending: actual.pending.clone(),
                        binding: actual.binding.clone(),
                        fuel_remaining: actual.fuel_remaining,
                        successor_sequence: actual.successor_sequence,
                        additional_successor_sequences: actual
                            .additional_successor_sequences
                            .clone(),
                    };
                    let bytes = serde_json::to_vec(&actual).unwrap();
                    assert_eq!(bytes, serde_json::to_vec(&frozen).unwrap());
                    assert_eq!(
                        serde_json::from_slice::<GroupResultBinding>(&bytes).unwrap(),
                        actual
                    );
                    assert_eq!(actual.is_conditional(), frozen.is_conditional());
                    assert_eq!(actual.is_dataflow(), frozen.is_dataflow());
                    assert_eq!(actual.captures_successor(), frozen.captures_successor());
                    assert_eq!(actual.continuation_schema(), frozen.continuation_schema());
                    assert_eq!(
                        actual.reserved_sequences().collect::<Vec<_>>(),
                        frozen.reserved_sequences().collect::<Vec<_>>()
                    );
                    let mut legacy = serde_json::to_value(&actual).unwrap();
                    legacy.as_object_mut().unwrap().remove("successor_sequence");
                    legacy
                        .as_object_mut()
                        .unwrap()
                        .remove("additional_successor_sequences");
                    let restored: GroupResultBinding =
                        serde_json::from_value(legacy.clone()).unwrap();
                    let old: FrozenGroupResultBinding =
                        serde_json::from_value(legacy.clone()).unwrap();
                    assert_eq!(restored.successor_sequence, None);
                    assert!(restored.additional_successor_sequences.is_empty());
                    assert_eq!(
                        serde_json::to_vec(&restored).unwrap(),
                        serde_json::to_vec(&old).unwrap()
                    );
                    for defect in 0..4 {
                        let mut invalid = legacy.clone();
                        match defect {
                            0 => {
                                invalid["unknown"] = true.into();
                            }
                            1 => {
                                invalid["binding"]["unknown"] = true.into();
                            }
                            2 => {
                                invalid.as_object_mut().unwrap().remove("pending");
                            }
                            3 => {
                                invalid["additional_successor_sequences"] = serde_json::json!([-1]);
                            }
                            _ => unreachable!(),
                        }
                        assert!(
                            serde_json::from_value::<GroupResultBinding>(invalid.clone()).is_err()
                        );
                        assert!(
                            serde_json::from_value::<FrozenGroupResultBinding>(invalid).is_err()
                        );
                    }
                    assert_eq!(serde_json::to_vec(&actual).unwrap(), bytes);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 288);
}
