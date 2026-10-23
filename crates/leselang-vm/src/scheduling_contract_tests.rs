use super::*;

fn policies() -> Vec<SchedulerLimits> {
    [0, 1, 2, 9_999, 10_000, 10_001, usize::MAX]
        .into_iter()
        .flat_map(|pending| {
            [0, 1, 2, 9_999, 10_000, 10_001, usize::MAX]
                .into_iter()
                .map(move |active| SchedulerLimits {
                    max_pending_dispatches: pending,
                    max_active_leases: active,
                })
        })
        .collect()
}

#[test]
fn scheduler_migration_keeps_exact_faults_overflow_priority_and_pressure_bytes() {
    let observations = [0, 1, 2, 9_999, 10_000, 10_001, usize::MAX - 1, usize::MAX];
    let mut comparisons = 0;
    for limits in policies() {
        let old = frozen::SchedulerLimits {
            max_pending_dispatches: limits.max_pending_dispatches,
            max_active_leases: limits.max_active_leases,
        };
        assert_eq!(limits.validate(), old.validate());
        assert_eq!(
            serde_json::to_vec(&limits).unwrap(),
            serde_json::to_vec(&old).unwrap()
        );
        for pending in observations {
            for additional in observations {
                assert_eq!(
                    limits.check_admission(pending, additional),
                    old.check_admission(pending, additional)
                );
                let pressure = SchedulerPressure {
                    pending_dispatches: pending,
                    active_leases: additional,
                    limits,
                };
                let old_pressure = frozen::SchedulerPressure {
                    pending_dispatches: pending,
                    active_leases: additional,
                    limits: old,
                };
                assert_eq!(
                    pressure.lease_capacity_exhausted(),
                    old_pressure.lease_capacity_exhausted()
                );
                assert_eq!(pressure.lease_fault(), old_pressure.lease_fault());
                assert_eq!(
                    serde_json::to_vec(&pressure).unwrap(),
                    serde_json::to_vec(&old_pressure).unwrap()
                );
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 3_136);
}

fn compare_wire<Actual, Previous>(source: &str)
where
    Actual: serde::de::DeserializeOwned + Serialize,
    Previous: serde::de::DeserializeOwned + Serialize,
{
    let actual = serde_json::from_str::<Actual>(source)
        .map(|value| serde_json::to_vec(&value).unwrap())
        .map_err(|error| error.to_string());
    let previous = serde_json::from_str::<Previous>(source)
        .map(|value| serde_json::to_vec(&value).unwrap())
        .map_err(|error| error.to_string());
    assert_eq!(actual, previous, "{source}");
}

#[test]
fn scheduler_migration_keeps_closed_shapes_and_released_dispatch_claim_variants() {
    use serde_json::json;
    let mut comparisons = 0;
    for limits in policies() {
        let wire = serde_json::to_value(limits).unwrap();
        let mut unknown = wire.clone();
        unknown["extra"] = json!(true);
        let mut missing = wire.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("max_pending_dispatches");
        let mut negative = wire.clone();
        negative["max_pending_dispatches"] = json!(-1);
        let duplicate = format!(
            "{},\"max_active_leases\":1}}",
            wire.to_string().trim_end_matches('}')
        );
        let mut wrong = wire.clone();
        wrong["max_active_leases"] = json!("one");
        for source in [
            wire.to_string(),
            unknown.to_string(),
            missing.to_string(),
            negative.to_string(),
            duplicate,
            wrong.to_string(),
        ] {
            compare_wire::<SchedulerLimits, frozen::SchedulerLimits>(&source);
            comparisons += 1;
        }
        let wire = serde_json::to_value(SchedulerPressure {
            pending_dispatches: usize::MAX,
            active_leases: usize::MAX,
            limits,
        })
        .unwrap();
        let mut unknown = wire.clone();
        unknown["extra"] = json!(true);
        let mut nested = wire.clone();
        nested["limits"]["extra"] = json!(true);
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove("limits");
        let mut negative = wire.clone();
        negative["active_leases"] = json!(-1);
        let text = wire.to_string();
        let duplicate = format!(
            "{},\"limits\":{}}}",
            &text[..text.len() - 1],
            wire["limits"]
        );
        for source in [
            text,
            unknown.to_string(),
            nested.to_string(),
            missing.to_string(),
            negative.to_string(),
            duplicate,
        ] {
            compare_wire::<SchedulerPressure, frozen::SchedulerPressure>(&source);
            comparisons += 1;
        }
    }
    let mut claims = vec![
        DispatchClaim::Idle,
        DispatchClaim::Backpressured(SchedulerPressure {
            pending_dispatches: 1,
            active_leases: 1,
            limits: SchedulerLimits::default(),
        }),
    ];
    for request in continuation_contract_tests::requests() {
        claims.push(DispatchClaim::Leased(Box::new(DispatchLease {
            request,
            attempt: 1,
            retry_count: 0,
            lease_expires_at_ms: 100,
        })));
    }
    for claim in claims {
        let wire = serde_json::to_value(claim).unwrap();
        let mut unknown = wire.clone();
        unknown["extra"] = json!(true);
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove("kind");
        let mut wrong = wire.clone();
        wrong["kind"] = json!("unknown");
        let text = wire.to_string();
        for source in [
            text.clone(),
            unknown.to_string(),
            missing.to_string(),
            wrong.to_string(),
            text[..text.len() - 1].into(),
            format!("{text} null"),
        ] {
            compare_wire::<DispatchClaim, frozen::DispatchClaim>(&source);
            comparisons += 1;
        }
    }
    for source in [
        "null", "true", "0", "\"text\"", "[]", "[1,2]", "[1,2,3]", "{}",
    ] {
        compare_wire::<SchedulerLimits, frozen::SchedulerLimits>(source);
        compare_wire::<SchedulerPressure, frozen::SchedulerPressure>(source);
        compare_wire::<DispatchClaim, frozen::DispatchClaim>(source);
        comparisons += 3;
    }
    assert_eq!(comparisons, 1_044);
}

#[test]
fn adapter_types_keep_vm_entry_points_and_enum_variant_imports() {
    use crate::DispatchClaim::{Backpressured, Idle};
    let limits: leselang_command::SchedulerLimits = SchedulerLimits::default();
    assert_eq!(limits.max_pending_dispatches, MAX_JOURNAL_RECORDS);
    assert_eq!(
        leselang_command::retention_policy::MAX_JOURNAL_RECORDS,
        MAX_JOURNAL_RECORDS
    );
    let mut vm = Vm::new_with_limits(DEFAULT_FUEL, limits).unwrap();
    let pressure: leselang_command::SchedulerPressure = vm.scheduler_pressure(1).unwrap();
    assert_eq!(pressure.limits, limits);
    assert_eq!(
        (pressure.pending_dispatches, pressure.active_leases),
        (0, 0)
    );
    assert_eq!(vm.try_claim_effect(1, 1).unwrap(), Idle);
    assert!(matches!(
        Backpressured(pressure),
        DispatchClaim::Backpressured(_)
    ));
}

// Frozen released scheduler DTOs and fault mapping, independent of the adapter.
mod frozen {
    use leselang_runtime_core::{
        DispatchAdmissionError, check_dispatch_admission, dispatch_lease_capacity_exhausted,
    };
    use serde::{Deserialize, Serialize};

    use crate::{DispatchLease, Fault, MAX_JOURNAL_RECORDS};

    /// Trusted host policy, applied consistently by every worker of one journal namespace.
    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct SchedulerLimits {
        pub max_pending_dispatches: usize,
        pub max_active_leases: usize,
    }

    impl Default for SchedulerLimits {
        fn default() -> Self {
            Self {
                max_pending_dispatches: MAX_JOURNAL_RECORDS,
                max_active_leases: MAX_JOURNAL_RECORDS,
            }
        }
    }

    impl SchedulerLimits {
        pub(crate) fn validate(self) -> Result<(), Fault> {
            if self.max_pending_dispatches == 0
                || self.max_pending_dispatches > MAX_JOURNAL_RECORDS
                || self.max_active_leases == 0
                || self.max_active_leases > MAX_JOURNAL_RECORDS
            {
                return Err(Fault {
                    code: "LSV2500".into(),
                    message: "scheduler limits must be between 1 and the journal record limit"
                        .into(),
                });
            }
            Ok(())
        }

        pub(crate) fn check_admission(
            self,
            pending: usize,
            additional: usize,
        ) -> Result<(), Fault> {
            check_dispatch_admission(pending, additional, self.max_pending_dispatches).map_err(|error| match error {
                DispatchAdmissionError::BatchTooLarge => Fault {
                    code: "LSV2503".into(),
                    message: format!(
                        "root needs {additional} dispatches, exceeding the configured limit {}",
                        self.max_pending_dispatches
                    ),
                },
                DispatchAdmissionError::Backpressured => Fault {
                    code: "LSV2501".into(),
                    message: format!(
                        "dispatch admission is backpressured: {pending} pending, {additional} requested, limit {}",
                        self.max_pending_dispatches
                    ),
                },
            })
        }
    }

    /// A read-only authoritative outbox snapshot, not a VM-local continuation count.
    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct SchedulerPressure {
        pub pending_dispatches: usize,
        pub active_leases: usize,
        pub limits: SchedulerLimits,
    }

    impl SchedulerPressure {
        pub(crate) fn lease_capacity_exhausted(self) -> bool {
            dispatch_lease_capacity_exhausted(self.active_leases, self.limits.max_active_leases)
        }

        pub(crate) fn lease_fault(self) -> Fault {
            Fault {
                code: "LSV2502".into(),
                message: format!(
                    "dispatch leasing is backpressured: {} active leases, limit {}",
                    self.active_leases, self.limits.max_active_leases
                ),
            }
        }
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
