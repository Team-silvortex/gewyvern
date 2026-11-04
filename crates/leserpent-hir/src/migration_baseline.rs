//! Read-only observations captured from the retired private boundary oracles.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recorded {
    schema_version: u32,
    matrix: String,
    origin: Origin,
    forms: Vec<String>,
    cases: Vec<[usize; 5]>,
    outcomes: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    product_revision: String,
    native_revision: String,
    oracle_source_sha256: String,
    capture: String,
}

pub(super) struct Baseline {
    matrix: &'static str,
    cases: BTreeMap<[usize; 4], usize>,
    outcomes: Vec<Value>,
}

impl Baseline {
    pub(super) fn load(matrix: &'static str, raw: &str, forms: &[&str], count: usize) -> Self {
        Self::parse(matrix, raw, forms, count).expect("recorded oracle matrix must be valid")
    }

    fn parse(
        matrix: &'static str,
        raw: &str,
        forms: &[&str],
        count: usize,
    ) -> Result<Self, String> {
        let recorded: Recorded = serde_json::from_str(raw).map_err(|error| error.to_string())?;
        if recorded.schema_version != 1
            || recorded.matrix != matrix
            || recorded.forms != forms
            || recorded.cases.len() != count
            || count == 0
            || recorded.origin.product_revision != "2dc935f2530c2256a49c824d4cc87c0687a61043"
            || recorded.origin.native_revision != "5f86dfb8dd79a28986fe5eb4cbfe6b044796f935"
            || recorded.origin.oracle_source_sha256.len() != 64
            || !recorded
                .origin
                .oracle_source_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
            || recorded.origin.capture != "frozen coordinator after exact live comparison"
        {
            return Err("recorded oracle identity, inputs or coverage changed".into());
        }
        let mut cases = BTreeMap::new();
        let mut used = BTreeSet::new();
        for [form, prefix, starting, depth, outcome] in recorded.cases {
            if form >= forms.len()
                || outcome >= recorded.outcomes.len()
                || cases
                    .insert([form, prefix, starting, depth], outcome)
                    .is_some()
            {
                return Err("duplicate case or invalid recorded index".into());
            }
            used.insert(outcome);
        }
        if used.len() != recorded.outcomes.len() {
            return Err("unreferenced recorded outcome".into());
        }
        Ok(Self {
            matrix,
            cases,
            outcomes: recorded.outcomes,
        })
    }

    pub(super) fn assert_case(
        &mut self,
        form: usize,
        prefix: usize,
        starting: usize,
        depth: usize,
        actual: &impl Serialize,
    ) {
        let key = [form, prefix, starting, depth];
        let index = self
            .cases
            .remove(&key)
            .expect("case must be recorded and visited exactly once");
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            self.outcomes[index],
            "{} boundary: form={form}, prefix={prefix}, visited={starting}, depth={depth}",
            self.matrix
        );
    }

    pub(super) fn finish(self) {
        assert!(self.cases.is_empty(), "{} lost recorded cases", self.matrix);
    }
}

#[test]
fn recorded_matrices_reject_identity_coverage_and_index_drift() {
    let raw: Value = serde_json::from_str(include_str!(
        "../tests/fixtures/migration-boundaries/loop.json"
    ))
    .unwrap();
    let forms: Vec<_> = raw["forms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(Baseline::parse("loop", &raw.to_string(), &forms, 1152).is_ok());
    for (field, value) in [
        ("schema_version", serde_json::json!(2)),
        ("matrix", serde_json::json!("scalar")),
        ("unknown", Value::Null),
    ] {
        let mut changed = raw.clone();
        changed[field] = value;
        assert!(Baseline::parse("loop", &changed.to_string(), &forms, 1152).is_err());
    }
    for field in [
        "product_revision",
        "native_revision",
        "oracle_source_sha256",
        "capture",
    ] {
        let mut changed = raw.clone();
        changed["origin"][field] = serde_json::json!("substituted");
        assert!(Baseline::parse("loop", &changed.to_string(), &forms, 1152).is_err());
    }
    for mutation in 0..6 {
        let mut changed = raw.clone();
        match mutation {
            0 => {
                changed["cases"].as_array_mut().unwrap().pop();
            }
            1 => changed["cases"][1] = changed["cases"][0].clone(),
            2 => changed["cases"][0][0] = serde_json::json!(forms.len()),
            3 => {
                changed["cases"][0][4] =
                    serde_json::json!(raw["outcomes"].as_array().unwrap().len())
            }
            4 => changed["outcomes"]
                .as_array_mut()
                .unwrap()
                .push(Value::Null),
            _ => changed["forms"][0] = serde_json::json!("changed source"),
        }
        assert!(
            Baseline::parse("loop", &changed.to_string(), &forms, 1152).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn recorded_observations_reject_changed_tuples_repeated_cases_and_incomplete_execution() {
    let raw = include_str!("../tests/fixtures/migration-boundaries/loop.json");
    let value: Value = serde_json::from_str(raw).unwrap();
    let forms: Vec<_> = value["forms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    let first: [usize; 5] = serde_json::from_value(value["cases"][0].clone()).unwrap();
    let [form, prefix, starting, depth, index] = first;
    let expected = &value["outcomes"][index];
    for field in 0..3 {
        let mut baseline = Baseline::load("loop", raw, &forms, 1152);
        let mut changed = expected.clone();
        changed[field] = Value::Null;
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || baseline.assert_case(form, prefix, starting, depth, &changed)
            ))
            .is_err()
        );
    }
    let mut baseline = Baseline::load("loop", raw, &forms, 1152);
    baseline.assert_case(form, prefix, starting, depth, expected);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || baseline.assert_case(form, prefix, starting, depth, expected)
        ))
        .is_err()
    );
    assert!(std::panic::catch_unwind(|| baseline.finish()).is_err());
}
