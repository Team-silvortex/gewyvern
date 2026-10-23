use super::group_recovery_contract_tests::{Fixture, snapshot};
use super::*;

fn frozen_decode<T: serde::de::DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, Fault> {
    if bytes.len() > limit {
        return Err(journal_fault(
            "LSV4006",
            format!("journal entry exceeds {limit} bytes"),
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| journal_error("LSV4007", "invalid journal JSON", error))
}

fn compare<T: serde::de::DeserializeOwned + serde::Serialize>(value: &T) -> usize {
    let original = serde_json::to_vec(value).unwrap();
    let mut comparisons = 0;
    for defect in 0..6 {
        let mut bytes = original.clone();
        match defect {
            1 => {
                bytes.pop();
            }
            2 => bytes.extend_from_slice(b" null"),
            3 => bytes.extend_from_slice(b"\n\t"),
            4 => bytes = b"\"\xff\"".to_vec(),
            5 => bytes = b"false".to_vec(),
            _ => {}
        }
        for limit in [
            0,
            1,
            bytes.len() - 1,
            bytes.len(),
            bytes.len() + 1,
            MAX_JOURNAL_ENTRY_BYTES,
        ] {
            let before = bytes.clone();
            let wire = |value: T| serde_json::to_vec(&value).unwrap();
            assert_eq!(
                decode_bounded::<T>(&bytes, limit).map(wire),
                frozen_decode::<T>(&bytes, limit).map(wire),
                "defect {defect}, limit {limit}"
            );
            assert_eq!(bytes, before);
            comparisons += 1;
        }
    }
    comparisons
}

#[test]
fn journal_decode_keeps_exact_faults_for_images_requests_and_steps() {
    let mut comparisons = 0;
    for request in crate::continuation_contract_tests::requests() {
        comparisons += compare(&request.continuation);
        comparisons += compare(&request);
        comparisons += compare(&Step::Effect(Box::new(request)));
    }
    assert_eq!(comparisons, 7_560);
}

#[test]
fn corrupt_durable_images_refuse_reopen_without_mutating_original_rows() {
    let sources = [
        r#"fn main() = ui.focus(node_id: "a")"#,
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(a: ui.focus(node_id: "a"), body: ui.focus(node_id: field(value: a, name: "node_id")))"#,
    ];
    let mut refusals = 0;
    for source in sources {
        for stage in [0, 2] {
            let fixture = Fixture::new(source, stage);
            let connection = Connection::open(&fixture.0).unwrap();
            let (token, original): (String, Vec<u8>) = connection
                .query_row(
                    "SELECT token, image FROM vm_effects ORDER BY token LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            for defect in 0..4 {
                let bytes = match defect {
                    0 => b"{".to_vec(),
                    1 => vec![b'{'; MAX_CONTINUATION_BYTES + 1],
                    2 => b"\"\xff\"".to_vec(),
                    3 => [original.as_slice(), b" null"].concat(),
                    _ => unreachable!(),
                };
                connection
                    .execute(
                        "UPDATE vm_effects SET image = ?1 WHERE token = ?2",
                        params![bytes, token],
                    )
                    .unwrap();
                let before = snapshot(&connection);
                let expected =
                    frozen_decode::<ContinuationImage>(&bytes, MAX_CONTINUATION_BYTES).unwrap_err();
                let error = crate::Vm::open_journal(&fixture.0, 500).err().unwrap();
                assert_eq!(error, expected, "{source}, {stage}, {defect}");
                assert_eq!(snapshot(&connection), before);
                refusals += 1;
            }
            connection
                .execute(
                    "UPDATE vm_effects SET image = ?1 WHERE token = ?2",
                    params![original, token],
                )
                .unwrap();
            assert!(crate::Vm::open_journal(&fixture.0, 500).is_ok());
        }
    }
    assert_eq!(refusals, 32);
}
