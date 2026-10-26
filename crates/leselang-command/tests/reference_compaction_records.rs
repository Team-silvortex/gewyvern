use leselang_command::reference_compaction_records::*;
use leselang_command::{ContinuationToken, MAX_CONTINUATION_BYTES, MergePlan, Step, Value};
use leselang_runtime_core::Fault;
use std::collections::BTreeSet;

fn error() -> Fault {
    Fault {
        code: "NATIVE".into(),
        message: "original native refusal".into(),
    }
}
fn token(s: &str) -> ContinuationToken {
    ContinuationToken::from_wire(s.into())
}
fn plan() -> Vec<u8> {
    serde_json::to_vec(&MergePlan::new(["a", "b"]).unwrap()).unwrap()
}
fn branch(token: &str, bytes: i64) -> StoredCompactionBranch {
    StoredCompactionBranch {
        token: token.into(),
        bytes,
        state: "completed".into(),
    }
}
struct Group {
    group: Option<StoredCompactionGroup>,
    branches: Vec<StoredCompactionBranch>,
    trace: Vec<String>,
    deleted: Vec<String>,
    group_count: usize,
    branch_counts: Vec<usize>,
    fail_at: Option<usize>,
    panic_at: Option<usize>,
    original: Option<Fault>,
}
impl Group {
    fn new() -> Self {
        Self {
            group: Some(StoredCompactionGroup {
                bytes: 7,
                plan: plan(),
            }),
            branches: vec![branch("later", 11), branch("earlier", 13)],
            trace: vec![],
            deleted: vec![],
            group_count: 1,
            branch_counts: vec![1, 1],
            fail_at: None,
            panic_at: None,
            original: Some(error()),
        }
    }
    fn call(&mut self, label: String) -> Result<(), Fault> {
        let index = self.trace.len();
        self.trace.push(label);
        assert_ne!(self.panic_at, Some(index), "native unwind");
        if self.fail_at == Some(index) {
            Err(self.original.take().unwrap())
        } else {
            Ok(())
        }
    }
}
impl GroupCompactionOwner for Group {
    fn group(&mut self, t: &str) -> Result<StoredCompactionGroup, Fault> {
        self.call(format!("group:{t}"))?;
        Ok(self.group.take().unwrap())
    }
    fn branches(&mut self, t: &str) -> Result<Vec<StoredCompactionBranch>, Fault> {
        self.call(format!("branches:{t}"))?;
        Ok(std::mem::take(&mut self.branches))
    }
    fn delete_group(&mut self, t: &str) -> Result<usize, Fault> {
        self.call(format!("delete-group:{t}"))?;
        self.deleted.push(t.into());
        Ok(self.group_count)
    }
    fn delete_branch(&mut self, t: &str) -> Result<usize, Fault> {
        self.call(format!("delete-branch:{t}"))?;
        self.deleted.push(t.into());
        Ok(self.branch_counts.remove(0))
    }
}
#[test]
fn group_deletes_in_original_order_after_complete_preflight_without_token_normalization() {
    let mut h = Group::new();
    assert_eq!(delete_completed_group(&mut h, "opaque"), Ok(31));
    assert_eq!(
        h.trace,
        [
            "group:opaque",
            "branches:opaque",
            "delete-group:opaque",
            "delete-branch:later",
            "delete-branch:earlier"
        ]
    );
    assert_eq!(h.deleted, ["opaque", "later", "earlier"]);
}
#[test]
fn branch_read_fault_precedes_plan_admission_and_moves_without_reconstruction() {
    let mut h = Group::new();
    h.group.as_mut().unwrap().plan = b"{".to_vec();
    h.fail_at = Some(1);
    let pointer = h.original.as_ref().unwrap().message.as_ptr();
    let e = delete_completed_group(&mut h, "g").unwrap_err();
    assert_eq!(e, error());
    assert_eq!(e.message.as_ptr(), pointer);
    assert_eq!(h.trace, ["group:g", "branches:g"]);
    assert!(h.deleted.is_empty());
}
#[test]
fn plan_then_completeness_then_size_faults_all_precede_deletion() {
    for defect in 0..7 {
        let mut h = Group::new();
        match defect {
            0 => {
                h.group.as_mut().unwrap().plan = b"{".to_vec();
                h.group.as_mut().unwrap().bytes = -1;
                h.branches.clear();
            }
            1 => {
                h.group.as_mut().unwrap().plan =
                    br#"{"order":"parallel","branches":[],"result_binding":null}"#.to_vec()
            }
            2 => {
                h.branches.pop();
                h.group.as_mut().unwrap().bytes = -1;
            }
            3 => {
                h.branches[1].state = "pending".into();
                h.group.as_mut().unwrap().bytes = -1;
            }
            4 => h.group.as_mut().unwrap().bytes = -1,
            5 => h.branches[1].bytes = -1,
            6 => h.group.as_mut().unwrap().plan = vec![b' '; MAX_CONTINUATION_BYTES + 1],
            _ => unreachable!(),
        }
        let e = delete_completed_group(&mut h, "g").unwrap_err();
        assert_eq!(h.trace, ["group:g", "branches:g"]);
        assert!(h.deleted.is_empty());
        let (code, message) = match defect {
            0 => ("LSV4007", "invalid journal JSON"),
            1 => ("LSV2401", "structured merge plan is invalid"),
            2 | 3 => (
                "LSV4030",
                "completed merge group has an incomplete branch set",
            ),
            4 => ("LSV4007", "merge group size is invalid"),
            5 => ("LSV4007", "merge branch size is invalid"),
            _ => ("LSV4006", "journal entry exceeds"),
        };
        assert_eq!(e.code, code, "defect {defect}");
        assert!(e.message.starts_with(message), "{e:?}");
    }
}
#[test]
fn byte_accumulation_saturates_without_overflow_or_reordering() {
    let mut h = Group::new();
    let large = if usize::BITS == 64 {
        i64::MAX
    } else {
        usize::MAX as i64
    };
    h.group.as_mut().unwrap().bytes = large;
    for b in &mut h.branches {
        b.bytes = large;
    }
    assert_eq!(delete_completed_group(&mut h, "g"), Ok(usize::MAX));
    assert_eq!(h.deleted, ["g", "later", "earlier"]);
}
#[test]
fn invalid_delete_cardinality_stops_without_undoing_already_deleted_prefix() {
    for stop in 0..3 {
        for count in [0, 2] {
            let mut h = Group::new();
            if stop == 0 {
                h.group_count = count;
            } else {
                h.branch_counts[stop - 1] = count;
            }
            let e = delete_completed_group(&mut h, "g").unwrap_err();
            assert_eq!(e.code, "LSV4030");
            assert_eq!(
                e.message,
                if stop == 0 {
                    "completed merge group changed during compaction"
                } else {
                    "merge branch changed during compaction"
                }
            );
            assert_eq!(h.deleted.len(), stop + 1);
            assert_eq!(h.trace.len(), stop + 3);
        }
    }
}
#[test]
fn every_native_error_or_unwind_escapes_without_retry_or_compensation() {
    for stop in 0..5 {
        let mut h = Group::new();
        h.fail_at = Some(stop);
        let pointer = h.original.as_ref().unwrap().message.as_ptr();
        let e = delete_completed_group(&mut h, "g").unwrap_err();
        assert_eq!(e, error());
        assert_eq!(e.message.as_ptr(), pointer);
        assert_eq!(h.trace.len(), stop + 1);
        assert_eq!(h.deleted.len(), stop.saturating_sub(2));
        let mut h = Group::new();
        h.panic_at = Some(stop);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| delete_completed_group(
                &mut h, "g"
            )))
            .is_err()
        );
        assert_eq!(h.trace.len(), stop + 1);
        assert_eq!(h.deleted.len(), stop.saturating_sub(2));
    }
}
struct Read {
    bytes: i64,
    tokens: BTreeSet<String>,
    trace: Vec<String>,
    fail: Option<Fault>,
}
impl CompactionRecordSource for Read {
    fn effect_bytes(&mut self, t: &str) -> Result<i64, Fault> {
        self.trace.push(format!("bytes:{t}"));
        if let Some(e) = self.fail.take() {
            Err(e)
        } else {
            Ok(self.bytes)
        }
    }
    fn completed_tokens(&mut self) -> Result<BTreeSet<String>, Fault> {
        self.trace.push("tokens".into());
        if let Some(e) = self.fail.take() {
            Err(e)
        } else {
            Ok(std::mem::take(&mut self.tokens))
        }
    }
}
#[test]
fn effect_size_preserves_zero_negative_range_and_native_query_error() {
    for size in [-1, 0, 1, i64::MAX] {
        let mut h = Read {
            bytes: size,
            tokens: BTreeSet::new(),
            trace: vec![],
            fail: None,
        };
        let expected = usize::try_from(size).map_err(|_| Fault {
            code: "LSV4007".into(),
            message: "journal compaction size is invalid".into(),
        });
        assert_eq!(completed_effect_bytes(&mut h, "opaque"), expected);
        assert_eq!(h.trace, ["bytes:opaque"]);
    }
    let mut h = Read {
        bytes: -1,
        tokens: BTreeSet::new(),
        trace: vec![],
        fail: Some(error()),
    };
    let pointer = h.fail.as_ref().unwrap().message.as_ptr();
    let e = completed_effect_bytes(&mut h, "").unwrap_err();
    assert_eq!(e, error());
    assert_eq!(e.message.as_ptr(), pointer);
    assert_eq!(h.trace, ["bytes:"]);
}
#[test]
fn reconciliation_keeps_local_order_duplicates_and_opaque_tokens_without_payload_admission() {
    let step = Step::Done(Value::UiFocus {
        node_id: String::new(),
    });
    let local = ["b", "keep", "b", "", "a"].map(|t| (token(t), step.clone()));
    let before = serde_json::to_vec(&local).unwrap();
    let mut h = Read {
        bytes: 0,
        tokens: BTreeSet::from(["keep".into(), "elsewhere".into()]),
        trace: vec![],
        fail: None,
    };
    assert_eq!(
        stale_local_completed(&mut h, &local).unwrap(),
        [token("b"), token("b"), token(""), token("a")]
    );
    assert_eq!(h.trace, ["tokens"]);
    assert_eq!(serde_json::to_vec(&local).unwrap(), before);
}
#[test]
fn empty_local_still_reads_native_snapshot_and_propagates_its_original_error() {
    let mut h = Read {
        bytes: 0,
        tokens: BTreeSet::new(),
        trace: vec![],
        fail: Some(error()),
    };
    let pointer = h.fail.as_ref().unwrap().message.as_ptr();
    let e = stale_local_completed(&mut h, &[]).unwrap_err();
    assert_eq!(e, error());
    assert_eq!(e.message.as_ptr(), pointer);
    assert_eq!(h.trace, ["tokens"]);
}
