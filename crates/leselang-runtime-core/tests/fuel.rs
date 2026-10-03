use leselang_runtime_core::{Fuel, FuelExhausted};

#[test]
fn exact_debits_are_monotone_and_observation_does_not_spend_or_refill() {
    let mut fuel = Fuel::new(8);
    for (cost, expected) in [(1, 7), (2, 5), (0, 5), (5, 0), (0, 0)] {
        assert_eq!(fuel.charge(cost), Ok(()));
        for _ in 0..100 {
            assert_eq!(fuel.remaining(), expected);
        }
    }
    assert_eq!(fuel.charge(1), Err(FuelExhausted));
    assert_eq!(fuel.remaining(), 0);
}

#[test]
fn rejected_debits_leave_the_original_counter_unchanged() {
    let mut fuel = Fuel::new(3);
    for cost in [4, u64::MAX, 5] {
        assert_eq!(fuel.charge(cost), Err(FuelExhausted));
        assert_eq!(fuel.remaining(), 3);
    }
    assert_eq!(fuel.charge(3), Ok(()));
    assert_eq!(fuel.remaining(), 0);
}

#[test]
fn all_small_budget_and_cost_pairs_match_checked_integer_accounting() {
    for initial in 0_u64..=64 {
        for cost in 0..=64 {
            let mut fuel = Fuel::new(initial);
            if let Some(expected) = initial.checked_sub(cost) {
                assert_eq!(fuel.charge(cost), Ok(()));
                assert_eq!(fuel.remaining(), expected);
            } else {
                assert_eq!(fuel.charge(cost), Err(FuelExhausted));
                assert_eq!(fuel.remaining(), initial);
            }
        }
    }
}

#[test]
fn full_width_counters_never_wrap_and_have_no_product_specific_clamp() {
    let mut fuel = Fuel::new(u64::MAX);
    assert_eq!(fuel.remaining(), u64::MAX);
    assert_eq!(fuel.charge(u64::MAX - 1), Ok(()));
    assert_eq!(fuel.remaining(), 1);
    assert_eq!(fuel.charge(u64::MAX), Err(FuelExhausted));
    assert_eq!(fuel.remaining(), 1);
    assert_eq!(fuel.charge(1), Ok(()));
    assert_eq!(fuel.charge(u64::MAX), Err(FuelExhausted));
    assert_eq!(fuel.remaining(), 0);
}

#[test]
fn independently_granted_contexts_do_not_share_or_poison_accounting() {
    let mut first = Fuel::new(2);
    let mut second = Fuel::new(7);
    first.charge(2).unwrap();
    assert_eq!(first.charge(1), Err(FuelExhausted));
    assert_eq!(second.remaining(), 7);
    second.charge(3).unwrap();
    assert_eq!(first.remaining(), 0);
    assert_eq!(second.remaining(), 4);
}

#[test]
fn moving_the_meter_between_threads_preserves_consumption_without_sync_mutation() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Fuel>();
    let mut fuel = Fuel::new(10);
    fuel.charge(3).unwrap();
    let worker = std::thread::spawn(move || {
        assert_eq!(fuel.remaining(), 7);
        fuel.charge(4).unwrap();
        fuel
    });
    let mut fuel = worker.join().unwrap();
    assert_eq!(fuel.remaining(), 3);
    fuel.charge(3).unwrap();
    assert_eq!(fuel.remaining(), 0);
}

#[test]
fn host_validated_reentry_starts_from_remaining_count_not_a_default_grant() {
    let saved_remaining = {
        let mut fuel = Fuel::new(10);
        fuel.charge(6).unwrap();
        fuel.remaining()
    };
    let mut restored = Fuel::new(saved_remaining);
    assert_eq!(restored.remaining(), 4);
    restored.charge(4).unwrap();
    assert_eq!(restored.charge(1), Err(FuelExhausted));
}

#[test]
fn accounting_is_constant_size_and_error_does_not_embed_host_context() {
    assert_eq!(std::mem::size_of::<Fuel>(), std::mem::size_of::<u64>());
    assert_eq!(std::mem::size_of::<FuelExhausted>(), 0);
    let error: &dyn std::error::Error = &FuelExhausted;
    assert_eq!(error.to_string(), "execution fuel exhausted");
    assert!(error.source().is_none());
    assert_eq!(format!("{FuelExhausted:?}"), "FuelExhausted");
}
