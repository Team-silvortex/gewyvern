use super::*;
use serde_json::json;

// Frozen size/serde/admission order, independent of the adapter's decoder.
fn frozen_decode_continuation(bytes: &[u8]) -> Result<ContinuationImage, Fault> {
    if bytes.len() > MAX_CONTINUATION_BYTES {
        return Err(Fault {
            code: "LSV3002".to_string(),
            message: format!("continuation exceeds {MAX_CONTINUATION_BYTES} bytes"),
        });
    }
    let image = serde_json::from_slice(bytes).map_err(|error| Fault {
        code: "LSV3003".to_string(),
        message: format!("invalid continuation: {error}"),
    })?;
    continuation_codec_contract_tests::frozen_validate_restorable_image(&image)?;
    Ok(image)
}

#[test]
fn continuation_decode_keeps_every_released_operation_and_refusal_order() {
    let mut comparisons = 0;
    for request in continuation_contract_tests::requests() {
        for defect in 0..14 {
            let mut value = serde_json::to_value(&request.continuation).unwrap();
            match defect {
                5 => value["token"] = json!(""),
                6 => value["token"] = json!("continuation-01"),
                7 => value["fuel_remaining"] = json!(MAX_EXECUTION_FUEL + 1),
                8 => value["schema_version"] = json!(0),
                9 => {
                    value.as_object_mut().unwrap().remove("token");
                }
                10 => value["pending_effect"] = json!(false),
                13 => value["extra"] = json!(true),
                _ => {}
            }
            let mut bytes = serde_json::to_vec(&value).unwrap();
            match defect {
                1 => {
                    bytes.pop();
                }
                2 => bytes.extend_from_slice(b" null"),
                3 => bytes.extend_from_slice(b" \n"),
                4 => {
                    bytes.pop();
                    bytes.extend_from_slice(br#","token":"continuation-1"}"#);
                }
                11 => bytes.resize(MAX_CONTINUATION_BYTES, b' '),
                12 => bytes.resize(MAX_CONTINUATION_BYTES + 1, b' '),
                _ => {}
            }
            let before = bytes.clone();
            let expected = frozen_decode_continuation(&bytes);
            if matches!(defect, 0 | 3 | 11) {
                assert_eq!(expected, Ok(request.continuation.clone()));
            } else {
                assert!(expected.is_err(), "defect {defect}");
            }
            assert_eq!(decode_continuation(&bytes), expected, "defect {defect}");
            assert_eq!(bytes, before);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 980);
}

#[test]
fn decoded_json_is_not_a_restoration_certificate_or_an_execution() {
    let mut image = continuation_contract_tests::requests()
        .remove(0)
        .continuation;
    image.token = ContinuationToken::from_wire("continuation-01".into());
    let bytes = serde_json::to_vec(&image).unwrap();
    let decoded: ContinuationImage =
        leselang_command::bounded_json::decode(&bytes, MAX_CONTINUATION_BYTES).unwrap();
    assert_eq!(decoded, image);
    assert_eq!(decode_continuation(&bytes).unwrap_err().code, "LSV2006");
    let mut vm = Vm::new(100);
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.restore(decoded).unwrap_err().code, "LSV2006");
    assert_eq!(vm.pending_count(), 0);
    for bytes in [
        b"".to_vec(),
        b"\"\xff\"".to_vec(),
        b"{} {}".to_vec(),
        vec![b'{'; MAX_CONTINUATION_BYTES + 1],
    ] {
        assert_eq!(
            decode_continuation(&bytes),
            frozen_decode_continuation(&bytes)
        );
    }
}
