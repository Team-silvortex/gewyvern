use leselang_runtime_core::capped_exponential_delay;

#[test]
fn first_delay_is_the_base_and_subsequent_delays_double_before_the_cap() {
    for (doublings, expected) in [10, 20, 40, 80, 160, 320].into_iter().enumerate() {
        assert_eq!(
            capped_exponential_delay(10, 1_000, doublings as u32),
            expected
        );
    }
}

#[test]
fn non_power_of_two_caps_are_exact_and_never_overshot() {
    for (doublings, expected) in [7, 14, 25, 25, 25].into_iter().enumerate() {
        assert_eq!(capped_exponential_delay(7, 25, doublings as u32), expected);
    }
    assert_eq!(capped_exponential_delay(250, 30_000, 6), 16_000);
    assert_eq!(capped_exponential_delay(250, 30_000, 7), 30_000);
}

#[test]
fn zero_base_and_zero_cap_remain_zero_even_at_the_largest_exponent() {
    for doublings in [0, 1, 63, 64, 127, u32::MAX] {
        assert_eq!(capped_exponential_delay(0, u64::MAX, doublings), 0);
        assert_eq!(capped_exponential_delay(0, 0, doublings), 0);
        assert_eq!(capped_exponential_delay(u64::MAX, 0, doublings), 0);
    }
}

#[test]
fn multiplication_and_shift_overflow_return_the_cap_without_wrapping() {
    assert_eq!(capped_exponential_delay(1, u64::MAX, 63), 1_u64 << 63);
    assert_eq!(capped_exponential_delay(1, u64::MAX, 64), u64::MAX);
    assert_eq!(capped_exponential_delay(3, u64::MAX, 62), 3_u64 << 62);
    assert_eq!(capped_exponential_delay(3, u64::MAX, 63), u64::MAX);
    assert_eq!(capped_exponential_delay(u64::MAX, u64::MAX, 0), u64::MAX);
    for doublings in [1, 63, 64, 65, 127, u32::MAX] {
        assert_eq!(capped_exponential_delay(u64::MAX, 99, doublings), 99);
        assert_eq!(capped_exponential_delay(1, 99, doublings.max(7)), 99);
    }
}

#[test]
fn a_cap_below_the_base_is_arithmetic_not_a_host_policy_error() {
    for doublings in [0, 1, 64, u32::MAX] {
        assert_eq!(capped_exponential_delay(100, 7, doublings), 7);
    }
    assert_eq!(capped_exponential_delay(7, 7, 0), 7);
    assert_eq!(capped_exponential_delay(7, 7, u32::MAX), 7);
}

#[test]
fn full_width_samples_match_an_independent_wide_integer_oracle() {
    let samples = [
        0,
        1,
        2,
        3,
        7,
        25,
        250,
        30_000,
        1_u64 << 63,
        u64::MAX - 1,
        u64::MAX,
    ];
    for base in samples {
        for cap in samples {
            for doublings in (0..=130).chain([u32::MAX]) {
                let exact = if base == 0 {
                    0
                } else {
                    1_u128
                        .checked_shl(doublings)
                        .and_then(|multiplier| u128::from(base).checked_mul(multiplier))
                        .unwrap_or(u128::MAX)
                };
                assert_eq!(
                    capped_exponential_delay(base, cap, doublings),
                    exact.min(u128::from(cap)) as u64,
                    "base={base}, cap={cap}, doublings={doublings}"
                );
            }
        }
    }
}

#[test]
fn repeated_and_out_of_order_calls_have_no_attempt_or_clock_state() {
    let observed =
        [4, 0, u32::MAX, 1, 0, 2, 1].map(|doublings| capped_exponential_delay(10, 25, doublings));
    assert_eq!(observed, [25, 10, 25, 20, 10, 25, 20]);
    assert_eq!(capped_exponential_delay(1, u64::MAX, 53), 1_u64 << 53);
    assert_eq!(capped_exponential_delay(3, u64::MAX, 53), 3_u64 << 53);
}
