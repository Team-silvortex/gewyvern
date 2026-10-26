use super::*;

use bounded_json_tests::{frozen_encode_json_capped, frozen_validate_json_size_capped};

// Independent pre-migration gates, including the old canonical identity policy.
pub(super) fn frozen_validate_restorable_image(image: &ContinuationImage) -> Result<(), Fault> {
    continuation_schema_tests::frozen_validate_image(image)?;
    let sequence = image
        .token
        .as_str()
        .strip_prefix("continuation-")
        .and_then(|suffix| suffix.parse::<u64>().ok());
    if sequence.is_none_or(|sequence| {
        sequence == 0
            || sequence > i64::MAX as u64 - 1
            || image.token.as_str() != format!("continuation-{sequence}")
    }) {
        return Err(Fault {
            code: "LSV2006".into(),
            message: "continuation token has a non-canonical or exhausted sequence".into(),
        });
    }
    canonical_source(&image.pending_effect).map_err(|_| Fault {
        code: "LSV2017".into(),
        message: "continuation pending effect is not canonical".into(),
    })?;
    Ok(())
}

fn frozen_validate_size(image: &ContinuationImage) -> Result<(), Fault> {
    if let Some(binding) = &image.result_binding {
        binding.validate_structure()?;
    }
    frozen_validate_json_size_capped(image, 64 * 1024, "continuation")?;
    frozen_validate_restorable_image(image)
}

fn frozen_encode(image: &ContinuationImage) -> Result<Vec<u8>, Fault> {
    frozen_validate_size(image)?;
    frozen_encode_json_capped(image, 64 * 1024, "continuation")
}

fn frozen_decode(bytes: &[u8]) -> Result<ContinuationImage, Fault> {
    if bytes.len() > 64 * 1024 {
        return Err(Fault {
            code: "LSV3002".into(),
            message: "continuation exceeds 65536 bytes".into(),
        });
    }
    let image = serde_json::from_slice(bytes).map_err(|error| Fault {
        code: "LSV3003".into(),
        message: format!("invalid continuation: {error}"),
    })?;
    frozen_validate_restorable_image(&image)?;
    Ok(image)
}

fn compare(image: &ContinuationImage) -> usize {
    let before = serde_json::to_vec(image).unwrap();
    let expected = frozen_encode(image);
    assert_eq!(encode_continuation(image), expected);
    assert_eq!(leselang_command::encode_continuation(image), expected);
    assert_eq!(
        validate_continuation_size(image),
        frozen_validate_size(image)
    );
    assert_eq!(
        validate_restorable_image(image),
        frozen_validate_restorable_image(image)
    );
    let expected = frozen_decode(&before);
    assert_eq!(decode_continuation(&before), expected);
    assert_eq!(leselang_command::decode_continuation(&before), expected);
    assert_eq!(serde_json::to_vec(image).unwrap(), before);
    6
}

#[test]
fn all_operation_codecs_preserve_frozen_bytes_faults_and_conflicting_gate_order() {
    let mut comparisons = 0;
    for request in continuation_contract_tests::requests() {
        for defect in 0..18 {
            let mut image = request.continuation.clone();
            match defect {
                0 => {}
                1 => image.schema_version = 0,
                2 => image.token = ContinuationToken::from_wire("".into()),
                3 => image.token = ContinuationToken::from_wire("continuation-01".into()),
                4 => {
                    image.token =
                        ContinuationToken::from_wire("continuation-9223372036854775807".into())
                }
                5 => image.program_counter = 2,
                6 => image.result_type = Type::Structured,
                7 => image.fuel_remaining = MAX_EXECUTION_FUEL + 1,
                8 => image.deadline_ms = 0,
                9 => image.deadline_at_ms = Some(u64::MAX),
                10 => image.max_output_items = DEFAULT_MAX_OUTPUT_ITEMS + 1,
                11 => {
                    image.pending_effect = Effect::UiFocus {
                        node_id: String::new(),
                    }
                }
                12 => {
                    image.result_type = Type::UiFocus;
                    image.pending_effect = Effect::UiFocus {
                        node_id: String::new(),
                    };
                }
                13 => {
                    image.token = ContinuationToken::from_wire("continuation-01".into());
                    image.result_type = Type::UiFocus;
                    image.pending_effect = Effect::UiFocus {
                        node_id: String::new(),
                    };
                }
                14 => {
                    image.schema_version = 0;
                    image.token = ContinuationToken::from_wire("".into());
                    image.fuel_remaining = u64::MAX;
                }
                15 => image.max_output_items = 0,
                16 => image.deadline_at_ms = None,
                17 => image.group_result = Some(ContinuationToken::from_wire("merge-01".into())),
                _ => unreachable!(),
            }
            comparisons += compare(&image);
        }
    }
    assert_eq!(comparisons, 7_560);
}

#[test]
fn every_released_schema_keeps_frozen_static_admission_without_group_authority() {
    let base = continuation_contract_tests::requests()
        .remove(9)
        .continuation;
    let mut comparisons = 0;
    for binding in continuation_schema_tests::bindings() {
        for schema_version in 0..=12 {
            for group_result in [None, Some(ContinuationToken::from_wire("merge-7".into()))] {
                let mut image = base.clone();
                image.result_binding = binding.clone().map(Box::new);
                image.schema_version = schema_version;
                image.group_result = group_result;
                comparisons += compare(&image);
            }
        }
    }
    assert_eq!(comparisons, 2_652);
}

#[test]
fn bounded_codec_keeps_raw_decode_limits_and_recursive_preflight_before_encoding() {
    use leserpent_hir::computation::Computation;
    let base = continuation_contract_tests::requests()
        .remove(9)
        .continuation;
    for size in [65_535, 65_536, 65_537] {
        let mut bytes = serde_json::to_vec(&base).unwrap();
        bytes.resize(size, b' ');
        assert_eq!(decode_continuation(&bytes), frozen_decode(&bytes));
    }
    let mut recursive = Computation::Literal {
        value: ScalarValue::None,
    };
    for _ in 0..128 {
        recursive = Computation::Bind {
            name: "deep".into(),
            value: Box::new(recursive),
            body: Box::new(Computation::Literal {
                value: ScalarValue::String("x".repeat(600)),
            }),
        };
    }
    let mut deep = base.clone();
    deep.schema_version = 0;
    deep.token = ContinuationToken::from_wire("".into());
    deep.result_binding = Some(Box::new(ResultBinding {
        name: "reply".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: recursive,
    }));
    assert_eq!(encode_continuation(&deep), frozen_encode(&deep));
    assert_eq!(encode_continuation(&deep).unwrap_err().code, "LSV1405");
    let mut huge = base;
    huge.schema_version = 0;
    huge.pending_effect = Effect::UiFocus {
        node_id: "x".repeat(MAX_CONTINUATION_BYTES),
    };
    assert_eq!(encode_continuation(&huge), frozen_encode(&huge));
    assert_eq!(encode_continuation(&huge).unwrap_err().code, "LSV3002");
    for bytes in [
        b"".to_vec(),
        b"{} {}".to_vec(),
        b"\"\xff\"".to_vec(),
        vec![b'['; 65_537],
    ] {
        assert_eq!(decode_continuation(&bytes), frozen_decode(&bytes));
    }
}
