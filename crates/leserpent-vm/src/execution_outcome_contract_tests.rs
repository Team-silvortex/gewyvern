use super::*;
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};

fn decode<T: DeserializeOwned + Serialize + std::fmt::Debug>(
    bytes: &[u8],
) -> Result<(Vec<u8>, String), String> {
    serde_json::from_slice::<T>(bytes)
        .map(|value| (serde_json::to_vec(&value).unwrap(), format!("{value:?}")))
        .map_err(|error| error.to_string())
}

fn compare<T, Previous>(value: &T) -> usize
where
    T: DeserializeOwned + Serialize + std::fmt::Debug,
    Previous: DeserializeOwned + Serialize + std::fmt::Debug,
{
    let original = serde_json::to_vec(value).unwrap();
    assert_eq!(decode::<T>(&original).unwrap().0, original);
    for defect in 0..12 {
        let mut bytes = original.clone();
        let mut wire: Json = serde_json::from_slice(&bytes).unwrap();
        let first = wire.as_object().unwrap().keys().next().unwrap().clone();
        match defect {
            0 => {}
            1 => {
                bytes.pop();
            }
            2 => bytes.extend_from_slice(b" null"),
            3 => bytes = b"null".to_vec(),
            4 => bytes = [b"[".as_slice(), &bytes, b"]"].concat(),
            5 => {
                wire["extra"] = json!(true);
                bytes = serde_json::to_vec(&wire).unwrap();
            }
            6 => {
                bytes.pop();
                bytes.extend_from_slice(format!(",{}:{}}}", json!(first), wire[&first]).as_bytes());
            }
            7 => {
                wire.as_object_mut().unwrap().remove(&first);
                bytes = serde_json::to_vec(&wire).unwrap();
            }
            8 => {
                wire["kind"] = json!("future_variant");
                bytes = serde_json::to_vec(&wire).unwrap();
            }
            9 => {
                wire["payload"] = Json::Null;
                bytes = serde_json::to_vec(&wire).unwrap();
            }
            10 => {
                let target = wire.get_mut("payload").filter(|v| v.is_object());
                if let Some(target) = target {
                    target["extra"] = json!(true);
                } else {
                    wire["request"] = json!({"effect_id": "wrong"});
                }
                bytes = serde_json::to_vec(&wire).unwrap();
            }
            11 => bytes = b"\"\xff\"".to_vec(),
            _ => unreachable!(),
        }
        assert_eq!(
            decode::<T>(&bytes),
            decode::<Previous>(&bytes),
            "defect {defect}: {original:?}"
        );
    }
    12
}

#[test]
fn execution_observations_keep_frozen_wire_debug_and_field_policy_for_seventy_operations() {
    let mut comparisons = 0;
    for request in continuation_contract_tests::requests() {
        let named = NamedEffectRequest {
            branch: "branch-a".into(),
            request: request.clone(),
        };
        comparisons += compare::<_, frozen::NamedEffectRequest>(&named);
        for size in [0, 1, 2, 64, 65] {
            let batch = StructuredEffectBatch {
                merge_token: ContinuationToken::from_wire("merge-1".into()),
                branches: vec![named.clone(); size],
            };
            comparisons += compare::<_, frozen::StructuredEffectBatch>(&batch);
            let step = Step::Effects(Box::new(batch));
            comparisons += compare::<_, frozen::Step>(&step);
            comparisons +=
                compare::<_, frozen::RetryDisposition>(&RetryDisposition::Terminal(step));
        }
        for (completed_branches, total_branches) in
            [(0, 0), (1, 2), (3, 2), (usize::MAX, usize::MAX)]
        {
            let wait = MergeWait {
                merge_token: ContinuationToken::from_wire("invalid/token".into()),
                completed_branches,
                total_branches,
            };
            comparisons += compare::<_, frozen::MergeWait>(&wait);
            let step = Step::Waiting(wait);
            comparisons += compare::<_, frozen::Step>(&step);
            comparisons +=
                compare::<_, frozen::RetryDisposition>(&RetryDisposition::Terminal(step));
        }
        let error = EffectError {
            class: EffectErrorClass::Transient,
            code: "busy".into(),
            message: "try later".into(),
        };
        let failed = FailedEffect {
            error: error.clone(),
            retry_count: u32::MAX,
        };
        comparisons += compare::<_, frozen::FailedEffect>(&failed);
        let cancellation = |reason, observed_at_ms| Cancellation {
            continuation: request.continuation.token.clone(),
            reason,
            observed_at_ms,
        };
        for step in [
            Step::Done(Value::UiFocus {
                node_id: "node-a".into(),
            }),
            Step::Effect(Box::new(request.clone())),
            Step::Yield(request.continuation.clone()),
            Step::Cancelled(cancellation(CancellationReason::Requested, 0)),
            Step::Cancelled(cancellation(CancellationReason::DeadlineExceeded, u64::MAX)),
            Step::Failed(failed),
            Step::Fault(Fault {
                code: String::new(),
                message: "raw diagnostic".into(),
            }),
        ] {
            comparisons += compare::<_, frozen::Step>(&step);
            comparisons +=
                compare::<_, frozen::RetryDisposition>(&RetryDisposition::Terminal(step));
        }
        for count in [0, 1, u32::MAX] {
            let lease = DispatchLease {
                request: request.clone(),
                attempt: count,
                retry_count: count,
                lease_expires_at_ms: u64::MAX,
            };
            comparisons += compare::<_, frozen::DispatchLease>(&lease);
            comparisons +=
                compare::<_, frozen::DispatchClaim>(&DispatchClaim::Leased(Box::new(lease)));
            let retry = RetrySchedule {
                retry_count: count,
                ready_at_ms: u64::MAX,
                error: error.clone(),
            };
            comparisons += compare::<_, frozen::RetrySchedule>(&retry);
            comparisons +=
                compare::<_, frozen::RetryDisposition>(&RetryDisposition::Scheduled(retry));
        }
    }
    for claim in [
        DispatchClaim::Idle,
        DispatchClaim::Backpressured(SchedulerPressure {
            pending_dispatches: usize::MAX,
            active_leases: usize::MAX,
            limits: SchedulerLimits::default(),
        }),
    ] {
        comparisons += compare::<_, frozen::DispatchClaim>(&claim);
    }
    assert_eq!(comparisons, 46_224);
}

#[test]
fn nested_request_decode_keeps_admission_while_raw_yield_and_observations_remain_data() {
    let mut comparisons = 0;
    for mut request in continuation_contract_tests::requests() {
        request.effect_id = "wrong".into();
        let named = NamedEffectRequest {
            branch: "a".into(),
            request: request.clone(),
        };
        let batch = StructuredEffectBatch {
            merge_token: ContinuationToken::from_wire("merge-1".into()),
            branches: vec![named.clone()],
        };
        let lease = DispatchLease {
            request: request.clone(),
            attempt: 1,
            retry_count: 0,
            lease_expires_at_ms: 10,
        };
        macro_rules! refuse {
            ($value:expr, $ty:ty, $previous:ty) => {{
                let bytes = serde_json::to_vec(&$value).unwrap();
                let actual = decode::<$ty>(&bytes);
                assert!(actual.as_ref().unwrap_err().contains("LSV2010"));
                assert_eq!(actual, decode::<$previous>(&bytes));
                comparisons += 1;
            }};
        }
        refuse!(named, NamedEffectRequest, frozen::NamedEffectRequest);
        refuse!(batch, StructuredEffectBatch, frozen::StructuredEffectBatch);
        refuse!(lease, DispatchLease, frozen::DispatchLease);
        refuse!(
            DispatchClaim::Leased(Box::new(lease)),
            DispatchClaim,
            frozen::DispatchClaim
        );
        for step in [
            Step::Effect(Box::new(request.clone())),
            Step::Effects(Box::new(batch)),
        ] {
            refuse!(step, Step, frozen::Step);
            refuse!(
                RetryDisposition::Terminal(step),
                RetryDisposition,
                frozen::RetryDisposition
            );
        }
        request.continuation.token = ContinuationToken::from_wire("invalid/token".into());
        let raw = Step::Yield(request.continuation);
        let bytes = serde_json::to_vec(&raw).unwrap();
        assert_eq!(decode::<Step>(&bytes), decode::<frozen::Step>(&bytes));
        assert!(decode::<Step>(&bytes).is_ok());
        comparisons += 1;
    }
    assert_eq!(comparisons, 630);
}

mod frozen {
    use super::{
        Cancellation, ContinuationImage, ContinuationToken, EffectError, EffectRequest, Fault,
        SchedulerPressure, Value,
    };
    use serde::{Deserialize, Serialize};
    /// Released specialization of host-neutral owned dispatch data; no wire conversion.
    pub type DispatchLease = leselang_runtime_core::DispatchLease<EffectRequest>;

    pub type FailedEffect = leselang_runtime_core::FailedEffect<EffectError>;
    pub type RetrySchedule = leselang_runtime_core::RetrySchedule<EffectError>;

    // Keep the released enum (including variant imports); terminal policy stays product-owned.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
    pub enum RetryDisposition {
        Scheduled(RetrySchedule),
        Terminal(Step),
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct NamedEffectRequest {
        pub branch: String,
        pub request: EffectRequest,
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct StructuredEffectBatch {
        pub merge_token: ContinuationToken,
        pub branches: Vec<NamedEffectRequest>,
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct MergeWait {
        pub merge_token: ContinuationToken,
        pub completed_branches: usize,
        pub total_branches: usize,
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
    pub enum Step {
        Done(Value),
        Effect(Box<EffectRequest>),
        Effects(Box<StructuredEffectBatch>),
        Waiting(MergeWait),
        Yield(ContinuationImage),
        Cancelled(Cancellation),
        Failed(FailedEffect),
        Fault(Fault),
    }

    /// Backpressure never grants a lease or increments a delivery attempt.
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(
        tag = "kind",
        content = "payload",
        rename_all = "snake_case",
        deny_unknown_fields
    )]
    pub enum DispatchClaim {
        Leased(Box<DispatchLease>),
        Idle,
        Backpressured(SchedulerPressure),
    }
}
