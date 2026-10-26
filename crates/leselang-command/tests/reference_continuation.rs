use leselang_command::reference_continuation::{
    validate_continuation_encoding_size, validate_continuation_size, validate_restorable_image,
};
use leselang_command::{
    ContinuationImage, ContinuationToken, MAX_CONTINUATION_BYTES, ResultBinding,
    decode_continuation, encode_continuation,
};
use leselang_host_contract::Revision;
use leserpent_hir::computation::{Computation, ScalarValue};
use leserpent_hir::{Effect, Type};

fn image() -> ContinuationImage {
    ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: Some(Revision(3)),
        result_type: Type::UiFocus,
        pending_effect: Effect::UiFocus {
            node_id: "node-a".into(),
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    }
}

#[test]
fn protocol_consumer_round_trips_and_revalidates_mutable_images_without_a_vm() {
    let image = image();
    let bytes = encode_continuation(&image).unwrap();
    assert_eq!(bytes, serde_json::to_vec(&image).unwrap());
    assert_eq!(decode_continuation(&bytes).unwrap(), image);
    assert_eq!(validate_continuation_size(&image), Ok(()));
    let mut decoded = decode_continuation(&bytes).unwrap();
    decoded.token = ContinuationToken::from_wire("continuation-007".into());
    assert_eq!(validate_continuation_encoding_size(&decoded), Ok(()));
    assert_eq!(
        validate_restorable_image(&decoded).unwrap_err().code,
        "LSV2006"
    );
    assert_eq!(encode_continuation(&decoded).unwrap_err().code, "LSV2006");
    assert_eq!(
        decode_continuation(&serde_json::to_vec(&decoded).unwrap())
            .unwrap_err()
            .code,
        "LSV2006"
    );
    decoded.result_type = Type::Structured;
    assert_eq!(encode_continuation(&decoded).unwrap_err().code, "LSV2016");
    decoded.schema_version = 0;
    assert_eq!(encode_continuation(&decoded).unwrap_err().code, "LSV2001");
}

#[test]
fn ingress_length_precedes_json_and_exact_limit_whitespace_remains_valid() {
    let image = image();
    let raw = encode_continuation(&image).unwrap();
    for size in [
        MAX_CONTINUATION_BYTES - 1,
        MAX_CONTINUATION_BYTES,
        MAX_CONTINUATION_BYTES + 1,
    ] {
        let mut bytes = raw.clone();
        bytes.resize(size, b' ');
        if size <= MAX_CONTINUATION_BYTES {
            assert_eq!(decode_continuation(&bytes), Ok(image.clone()));
        } else {
            let fault = decode_continuation(&bytes).unwrap_err();
            assert_eq!(fault.code, "LSV3002");
            assert_eq!(fault.message, "continuation exceeds 65536 bytes");
        }
    }
    for bytes in [b"{} {}".to_vec(), b"\"\xff\"".to_vec(), vec![b'['; 256]] {
        assert_eq!(decode_continuation(&bytes).unwrap_err().code, "LSV3003");
    }
    assert_eq!(
        decode_continuation(&vec![b'['; MAX_CONTINUATION_BYTES + 1])
            .unwrap_err()
            .code,
        "LSV3002"
    );
    let duplicate = String::from_utf8(raw).unwrap().replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    assert_eq!(
        decode_continuation(duplicate.as_bytes()).unwrap_err().code,
        "LSV3003"
    );
}

#[test]
fn encoding_keeps_structure_before_size_before_static_admission() {
    let mut frame = image();
    frame.schema_version = 0;
    frame.pending_effect = Effect::UiFocus {
        node_id: String::new(),
    };
    let empty_bytes = serde_json::to_vec(&frame).unwrap().len();
    for offset in [-1isize, 0, 1] {
        let mut candidate = frame.clone();
        candidate.pending_effect = Effect::UiFocus {
            node_id: "x"
                .repeat((MAX_CONTINUATION_BYTES - empty_bytes).saturating_add_signed(offset)),
        };
        assert_eq!(
            serde_json::to_vec(&candidate).unwrap().len(),
            MAX_CONTINUATION_BYTES.saturating_add_signed(offset)
        );
        let error = encode_continuation(&candidate).unwrap_err();
        assert_eq!(error.code, if offset > 0 { "LSV3002" } else { "LSV2001" });
    }
    let mut body = Computation::Literal {
        value: ScalarValue::None,
    };
    for _ in 0..128 {
        body = Computation::Bind {
            name: "deep".into(),
            value: Box::new(body),
            body: Box::new(Computation::Literal {
                value: ScalarValue::String("x".repeat(600)),
            }),
        };
    }
    frame.result_binding = Some(Box::new(ResultBinding {
        name: "reply".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body,
    }));
    assert_eq!(
        validate_continuation_encoding_size(&frame)
            .unwrap_err()
            .code,
        "LSV1405"
    );
    assert_eq!(encode_continuation(&frame).unwrap_err().code, "LSV1405");
}

#[test]
fn canonical_source_and_group_metadata_are_static_data_not_a_journal() {
    let mut frame = image();
    frame.pending_effect = Effect::UiFocus {
        node_id: String::new(),
    };
    assert_eq!(
        leselang_command::reference_request::validate_continuation_image(&frame),
        Ok(())
    );
    assert_eq!(
        validate_restorable_image(&frame).unwrap_err().code,
        "LSV2017"
    );
    assert_eq!(encode_continuation(&frame).unwrap_err().code, "LSV2017");
    assert_eq!(
        decode_continuation(&serde_json::to_vec(&frame).unwrap())
            .unwrap_err()
            .code,
        "LSV2017"
    );
    let mut member = image();
    member.schema_version = 6;
    member.group_result = Some(ContinuationToken::from_wire("merge-3".into()));
    let bytes = encode_continuation(&member).unwrap();
    assert_eq!(decode_continuation(&bytes).unwrap(), member);
}
