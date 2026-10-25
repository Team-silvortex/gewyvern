use std::{fs, path::Path, process::Command};

#[test]
fn pending_registration_policy_keeps_native_admission_watermark_and_replay_boundaries() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let ordered = |source: &str, fragments: &[&str]| {
        let mut offset = 0;
        for f in fragments {
            offset += source[offset..].find(f).expect(f) + f.len();
        }
    };
    let adapter = read("crates/leselang-command/src/reference_pending_commit.rs");
    for required in [
        "pub trait MemoryPendingJournal",
        "pub trait DurablePendingTransaction",
        "fn commit(self, kind: PendingCommit)",
        "Option<&EffectRequest>",
        "pub struct PendingEncoding<'a>",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "catch_unwind",
        "loop {",
        "thread",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let memory = adapter
        .split("pub fn register_memory")
        .nth(1)
        .unwrap()
        .split("pub struct PendingEncoding")
        .next()
        .unwrap();
    ordered(
        memory,
        &[
            "let Some(request) = request",
            "validate_effect_request(request)?",
            "validate_continuation_encoding_size(&request.continuation)?",
            "request.continuation.token.clone()",
            "host.request(&token)",
            "existing == request",
            "host.check_capacity(limits)?",
            "host.insert_pending(token, request)",
        ],
    );
    let durable = adapter.split("pub fn register_durable").nth(1).unwrap();
    ordered(
        durable,
        &[
            "host.effect(token)?",
            "existing.state == \"pending\" && existing.image == encoding.image",
            "if let Some(request) = request",
            "host.dispatch(token)?",
            "decode_journal_value(&dispatch.request",
            "durable_request != *request",
            "host.advance_watermark(encoding.sequence)?",
            "return host.commit(PendingCommit::Existing)",
            "host.record_count()? >= MAX_JOURNAL_RECORDS",
            "if request.is_some()",
            "host.check_capacity(limits)?",
            "host.ensure_growth(",
            "host.insert_effect(image, encoding.image)?",
            "host.insert_dispatch(token, bytes)?",
            "host.advance_watermark(encoding.sequence)?",
            "host.commit(PendingCommit::New)",
        ],
    );
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(journal.contains("reference_pending_commit::register_memory(self, request, limits)"));
    assert!(!journal.contains("fn ensure_dispatch_matches("));
    let native = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn record_pending(")
        .nth(1)
        .unwrap()
        .split("fn record_merge_graph(")
        .next()
        .unwrap();
    ordered(
        native,
        &[
            "validate_image(image)?",
            "validate_effect_request(request)?",
            "&request.continuation != image",
            "encode_json_capped(image",
            "canonical_continuation_sequence(&image.token)",
            "encode_json_capped(request",
            "transaction_with_behavior(TransactionBehavior::Immediate)",
            "reference_pending_commit::register_durable(",
            "pending_commit::SqlitePendingTransaction(transaction)",
            "request: request_bytes.as_deref()",
        ],
    );
    assert!(!native.contains("existing.state"));
    let owner = read("crates/leselang-vm/src/journal/pending_commit.rs");
    for required in [
        "self.dispatches.get(token)",
        "request: request.clone()",
        "scheduler_pressure(0, limits).pending_dispatches",
        "rusqlite::Transaction<'a>",
        "load_record(&self.0, token)",
        "load_dispatch(&self.0, token)",
        "journal_record_count(&self.0)",
        "sql_scheduler_pressure(&self.0, 0, limits)?",
        "ensure_growth(&self.0, additional)",
        "INSERT INTO vm_effects",
        "INSERT INTO vm_dispatches",
        "map(i64::try_from)",
        "effect deadline is out of range",
        "advance_sequence_watermark(&self.0, sequence)",
        "self.0.commit()",
        "failed to commit pending journal record",
        "failed to commit pending effect",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    assert!(!owner.contains("durable_request"));
    let cases = read("crates/leselang-vm/src/journal/pending_commit_contract_tests.rs");
    assert!(!cases.contains("reference_pending_commit::"));
    assert_eq!(cases.matches("fn frozen_record_pending(").count(), 2);
    assert!(cases.contains("fn frozen_ensure_dispatch_matches("));
    for count in [840, 2, 192, 6, 4] {
        assert!(cases.contains(&format!("assert_eq!(cases, {count})")));
    }
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_pending_commit"));
    assert!(ci.contains("--lib pending_commit_contract_tests"));
}

#[test]
fn expiration_batches_keep_borrowed_memory_order_and_native_snapshot_transaction_boundaries() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let ordered = |source: &str, fragments: &[&str]| {
        let mut offset = 0;
        for f in fragments {
            offset += source[offset..].find(f).expect(f) + f.len();
        }
    };
    let adapter = read("crates/leselang-command/src/reference_expiration_commit.rs");
    for required in [
        "pub trait MemoryExpirationJournal",
        "pub trait DurableExpirationTransaction",
        "fn commit(self)",
        "pub image: &'a ContinuationImage",
        "impl Iterator<Item = MemoryExpirationState<'_>>",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "catch_unwind",
        "loop {",
        "thread",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let memory = adapter
        .split("pub fn expire_memory")
        .nth(1)
        .unwrap()
        .split("pub struct TimedEffectRecord")
        .next()
        .unwrap();
    ordered(
        memory,
        &[
            "for state in host.dispatches()",
            "*state.acknowledged",
            "deadline > now_ms",
            "deadline_cancellation(image, now_ms)",
            "*state.acknowledged = true",
            "*state.lease_expires_at_ms = None",
            "*state.terminal_step = Some(step.clone())",
            "expired.push((image.clone(), step))",
            "for (image, _) in &expired",
            "host.finalize_group(&image.token)?",
        ],
    );
    assert!(!memory.contains("validate_"));
    assert!(!memory.contains(".collect"));
    let durable = adapter.split("pub fn expire_durable").nth(1).unwrap();
    ordered(
        durable,
        &[
            "host.due_records()?",
            "Vec::with_capacity(records.len())",
            "for record in records",
            "host.admit_image(&record.image)?",
            "image.token.as_str() != record.token",
            "deadline > now_ms",
            "deadline_cancellation(&image, now_ms)",
            "host.encode_terminal(&step)?",
            "host.ensure_growth(step_bytes.len())?",
            "host.complete(&record.token, &step_bytes)?",
            "host.finalize_group(&record.token)?",
            "expired.push((image, step))",
            "host.commit()?",
            "Ok(expired)",
        ],
    );
    assert_eq!(durable.matches("host.due_records()").count(), 1);
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(journal.contains("reference_expiration_commit::expire_memory(self, now_ms)"));
    let native = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn expire_due(")
        .nth(1)
        .unwrap()
        .split("fn compact_completed(")
        .next()
        .unwrap();
    ordered(
        native,
        &[
            "i64::try_from(now_ms)",
            "LSV4015",
            "transaction_with_behavior(TransactionBehavior::Immediate)",
            "reference_expiration_commit::expire_durable(",
            "expiration_commit::SqliteExpirationTransaction { transaction, now }",
        ],
    );
    assert!(!native.contains("SELECT"));
    let owner = read("crates/leselang-vm/src/journal/expiration_commit.rs");
    for required in [
        "self.dispatches",
        ".values_mut()",
        "image: &dispatch.request.continuation",
        "rusqlite::Transaction<'a>",
        "state = 'pending' AND deadline_at_ms IS NOT NULL AND deadline_at_ms <= ?1",
        "ORDER BY token ASC",
        ".collect::<Result<Vec<_>, _>>()",
        "drop(statement)",
        "failed to prepare timeout scan",
        "failed to scan timed effects",
        "failed to read timed effect",
        "decode_bounded(bytes, MAX_CONTINUATION_BYTES)?",
        "validate_image(&image)?",
        "encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, \"terminal step\")",
        "ensure_growth(&self.transaction, additional)",
        "complete_terminal_record(&self.transaction, token, step_bytes)",
        "finalize_merge_group(&self.transaction, token)",
        "failed to commit effect timeouts",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    let cases = read("crates/leselang-vm/src/journal/expiration_commit_contract_tests.rs");
    assert!(!cases.contains("reference_expiration_commit::"));
    assert_eq!(cases.matches("fn frozen_expire_due(").count(), 2);
    for count in [700, 2, 140, 10] {
        assert!(cases.contains(&format!("assert_eq!(cases, {count})")));
    }
    assert!(
        cases.contains("public_memory_finalization_failure_does_not_publish_partial_cache_results")
    );
    assert!(cases.contains("reopened.resume_at(i, 20, reply())"));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_expiration_commit"));
    assert!(ci.contains("--lib expiration_commit_contract_tests"));
}

#[test]
fn ordinary_completion_policy_preserves_backend_precedence_and_native_transaction_ownership() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let ordered = |source: &str, fragments: &[&str]| {
        let mut offset = 0;
        for fragment in fragments {
            offset += source[offset..].find(fragment).expect(fragment) + fragment.len();
        }
    };
    let adapter = read("crates/leselang-command/src/reference_completion_commit.rs");
    for required in [
        "pub trait MemoryCompletionJournal",
        "pub trait DurableCompletionTransaction",
        "fn commit(self, kind: CompletionCommit)",
        "pub lease_expires_at_ms: Option<u64>",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "catch_unwind",
        "loop {",
        "thread",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let memory = adapter
        .split("pub fn complete_memory")
        .nth(1)
        .unwrap()
        .split("pub enum CompletionCommit")
        .next()
        .unwrap();
    ordered(
        memory,
        &[
            "host.sequence_ready(&image.token)",
            "host.dispatch(&image.token)",
            "state.lease_expires_at_ms.is_some() && !*state.acknowledged",
            "*state.acknowledged = true",
            "*state.terminal_step = Some(step.clone())",
            "host.finalize_group(&image.token)?",
        ],
    );
    assert!(!memory.contains("validate_terminal_step"));
    assert!(!memory.contains("lease_expires_at_ms ="));
    let durable = adapter.split("pub fn complete_durable").nth(1).unwrap();
    ordered(
        durable,
        &[
            "host.effect(token)?",
            "if existing.image != image_bytes",
            "if existing.state == \"completed\"",
            "decode_journal_value(&bytes",
            "validate_terminal_step(&authoritative)?",
            "host.commit(CompletionCommit::Replay)?",
            "return Ok(authoritative)",
            "host.dispatch(token)?",
            "dispatch.state == \"leased\"",
            "host.ensure_sequence_ready(token)?",
            "host.ensure_growth(step_bytes.len())?",
            "host.complete_effect(token, step_bytes)?",
            "host.acknowledge_dispatch(token)?",
            "host.finalize_group(token)?",
            "host.commit(CompletionCommit::Completion)?",
        ],
    );
    assert!(!durable.contains("decode_journal_value(&dispatch.request"));
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(journal.contains("reference_completion_commit::complete_memory(self, image, step)"));
    let native = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn record_completed(")
        .nth(1)
        .unwrap()
        .split("fn acknowledge_dispatch(")
        .next()
        .unwrap();
    ordered(
        native,
        &[
            "validate_image(image)?",
            "validate_terminal_step(step)?",
            "encode_json_capped(image",
            "encode_json_capped(step",
            "transaction_with_behavior(TransactionBehavior::Immediate)",
            "reference_completion_commit::complete_durable(",
            "completion_commit::SqliteCompletionTransaction(transaction)",
        ],
    );
    assert!(!native.contains("existing.state"));
    let owner = read("crates/leselang-vm/src/journal/completion_commit.rs");
    for required in [
        "rusqlite::Transaction<'a>",
        "load_record(&self.0, token)",
        "load_dispatch(&self.0, token)",
        "ensure_sequence_ready(&self.0, token)",
        "ensure_growth(&self.0, additional)",
        "UPDATE vm_effects",
        "UPDATE vm_dispatches",
        "finalize_merge_group(&self.0",
        "self.0.commit()",
        "failed to complete effect",
        "failed to acknowledge dispatch",
        "failed to close journal transaction",
        "failed to commit completed effect",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    assert!(!owner.contains("complete_terminal_record"));
    let cases = read("crates/leselang-vm/src/journal/completion_commit_contract_tests.rs");
    assert!(!cases.contains("reference_completion_commit::"));
    assert_eq!(cases.matches("fn frozen_record_completed(").count(), 2);
    for count in [700, 4, 2, 150, 10] {
        assert!(cases.contains(&format!("assert_eq!(cases, {count})")));
    }
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_completion_commit"));
    assert!(ci.contains("--lib completion_commit_contract_tests"));
}

#[test]
fn acknowledgement_policy_keeps_native_admission_and_distinct_memory_replay_semantics() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let ordered = |source: &str, fragments: &[&str]| {
        let mut offset = 0;
        for fragment in fragments {
            offset += source[offset..].find(fragment).expect(fragment) + fragment.len();
        }
    };
    let adapter = read("crates/leselang-command/src/reference_acknowledgement_commit.rs");
    for required in [
        "pub trait MemoryAcknowledgementJournal",
        "pub trait DurableAcknowledgementTransaction",
        "fn commit(self, kind: AcknowledgementCommit)",
        "pub request: &'a EffectRequest",
        "pub acknowledged: &'a mut bool",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "catch_unwind",
        "loop {",
        "thread",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let memory = adapter.split("pub fn acknowledge_memory").nth(1).unwrap();
    ordered(
        memory,
        &[
            "validate_terminal_step(step)?",
            ".dispatch(token)",
            "if *state.acknowledged",
            "return Ok(step.clone())",
            "validate_observed_lease(",
            "same_request: state.request == &lease.request",
            "*state.acknowledged = true",
            "*state.lease_expires_at_ms = None",
            "*state.terminal_step = Some(step.clone())",
            "host.finalize_group(token)?",
        ],
    );
    let durable = adapter.split("pub fn acknowledge_durable").nth(1).unwrap();
    ordered(
        durable,
        &[
            ".effect(token)?",
            "if existing.image != image_bytes",
            "if existing.state == \"completed\"",
            "decode_journal_value(&bytes",
            "validate_terminal_step(&authoritative)?",
            "host.commit(AcknowledgementCommit::Replay)?",
            "return Ok(authoritative)",
            ".dispatch(token)?",
            "decode_journal_value(&dispatch.request",
            "validate_observed_lease(",
            "same_request: request == lease.request",
            "active: dispatch.state == \"leased\"",
            "host.ensure_growth(step_bytes.len())?",
            "host.complete_effect(token, step_bytes)?",
            "host.acknowledge_dispatch(token)?",
            "host.finalize_group(token)?",
            "host.commit(AcknowledgementCommit::Acknowledgement)?",
        ],
    );
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(journal.contains("reference_acknowledgement_commit::acknowledge_memory("));
    let native = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn acknowledge_dispatch(")
        .nth(1)
        .unwrap()
        .split("fn report_error(")
        .next()
        .unwrap();
    ordered(
        native,
        &[
            "validate_effect_request(&lease.request)?",
            "validate_terminal_step(step)?",
            "if lease.attempt == 0 || lease.attempt > MAX_DISPATCH_ATTEMPTS",
            "encode_json_capped(image",
            "encode_json_capped(&lease.request",
            "encode_json_capped(step",
            "transaction_with_behavior(TransactionBehavior::Immediate)",
            "reference_acknowledgement_commit::acknowledge_durable(",
            "acknowledgement_commit::SqliteAcknowledgementTransaction(transaction)",
        ],
    );
    assert!(!native.contains("existing.state"));
    let owner = read("crates/leselang-vm/src/journal/acknowledgement_commit.rs");
    for required in [
        "request: &dispatch.request",
        "rusqlite::Transaction<'a>",
        "load_record(&self.0, token)",
        "load_dispatch(&self.0, token)",
        "ensure_growth(&self.0, additional)",
        "UPDATE vm_effects",
        "UPDATE vm_dispatches",
        "finalize_merge_group(&self.0",
        "self.0.commit()",
        "failed to complete dispatch",
        "failed to acknowledge dispatch",
        "failed to close dispatch transaction",
        "failed to commit dispatch result",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    assert!(!owner.contains("complete_terminal_record"));
    assert!(!owner.contains("dispatch.request =="));
    let cases = read("crates/leselang-vm/src/journal/acknowledgement_commit_contract_tests.rs");
    assert!(!cases.contains("reference_acknowledgement_commit::"));
    assert_eq!(cases.matches("fn frozen_acknowledge_dispatch(").count(), 2);
    for count in [1400, 2, 180, 10] {
        assert!(cases.contains(&format!("assert_eq!(cases, {count})")));
    }
    assert!(cases.contains(
        "public_memory_acknowledgement_replays_winner_despite_low_level_proposal_semantics"
    ));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_acknowledgement_commit"));
    assert!(ci.contains("--lib acknowledgement_commit_contract_tests"));
}

#[test]
fn retry_journal_policy_keeps_native_storage_and_distinct_lease_replay_order() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let ordered = |source: &str, fragments: &[&str]| {
        let mut offset = 0;
        for fragment in fragments {
            offset += source[offset..].find(fragment).expect(fragment) + fragment.len();
        }
    };
    let adapter = read("crates/leselang-command/src/reference_retry_commit.rs");
    for required in [
        "pub trait MemoryRetryJournal",
        "pub trait DurableRetryTransaction",
        "fn commit(self, kind: RetryCommit)",
        "pub retry_count: &'a mut u32",
        "pub observation: DispatchLeaseObservation",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "catch_unwind",
        "loop {",
        "thread",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    ordered(
        &adapter,
        &[
            "host",
            ".dispatch(lease)",
            "validate_observed_lease(lease, now_ms, state.observation)?",
            "validate_proposed_retry(*state.retry_count, schedule, now_ms)?",
            "*state.retry_count = schedule.retry_count",
            "*state.ready_at_ms = schedule.ready_at_ms",
            "*state.last_error = Some(schedule.error.clone())",
            "*state.lease_expires_at_ms = None",
            "validate_terminal_step(step)?",
            "*state.acknowledged = true",
            "*state.lease_expires_at_ms = None",
            "*state.terminal_step = Some(step.clone())",
            "if matches!(disposition, RetryDisposition::Terminal(_))",
            "host.finalize_group(",
        ],
    );
    let durable = adapter.split("pub fn report_durable").nth(1).unwrap();
    ordered(
        durable,
        &[
            ".effect(token)?",
            "if existing.image != image_bytes",
            "if existing.state == \"completed\"",
            "decode_journal_value(",
            "validate_terminal_step(&authoritative)?",
            "host.commit(RetryCommit::Replay)?",
            "return Ok(RetryDisposition::Terminal(authoritative))",
            ".dispatch(token)?",
            "decode_journal_value(&dispatch.request",
            "validate_observed_lease(",
            "same_request: request == lease.request",
            "active: dispatch.state == \"leased\"",
            "validate_proposed_retry(dispatch.retry_count, schedule, now_ms)?",
            "host.encode_error(&schedule.error)?",
            "i64::try_from(schedule.ready_at_ms)",
            "host.ensure_growth(error_bytes.len())?",
            "host.schedule(",
            "validate_terminal_step(step)?",
            "host.encode_terminal(step)?",
            "host.ensure_growth(step_bytes.len())?",
            "host.complete(token, &step_bytes)?",
            "host.finalize_group(token)?",
            "host.commit(RetryCommit::Retry)?",
        ],
    );
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(
        journal.contains("reference_retry_commit::report_memory(self, lease, now_ms, disposition)")
    );
    let native = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn report_error(")
        .nth(1)
        .unwrap()
        .split("fn cancel(")
        .next()
        .unwrap();
    ordered(
        native,
        &[
            "validate_effect_request(&lease.request)?",
            "encode_json_capped(image",
            "transaction_with_behavior(TransactionBehavior::Immediate)",
            "reference_retry_commit::report_durable(",
            "retry_commit::SqliteRetryTransaction(transaction)",
        ],
    );
    assert!(!native.contains("existing.state"));
    assert!(journal.contains("reference_recovery::StoredDispatch as DispatchRecord"));
    let owner = read("crates/leselang-vm/src/journal/retry_commit.rs");
    for required in [
        "rusqlite::Transaction<'a>",
        "load_record(&self.0, token)",
        "load_dispatch(&self.0, token)",
        "encode_json_capped(error, 2_048, \"effect error\")",
        "encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, \"terminal step\")",
        "ensure_growth(&self.0, additional)",
        "UPDATE vm_dispatches",
        "complete_terminal_record(&self.0",
        "finalize_merge_group(&self.0",
        "self.0.commit()",
        "same_request: dispatch.request == lease.request",
        "failed to close retry transaction",
        "failed to commit effect retry",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    let cases = read("crates/leselang-vm/src/journal/retry_commit_contract_tests.rs");
    assert!(!cases.contains("reference_retry_commit::"));
    assert_eq!(cases.matches("fn frozen_report_error(").count(), 2);
    for count in [1540, 2, 170, 10] {
        assert!(cases.contains(&format!("assert_eq!(cases, {count})")));
    }
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_retry_commit"));
    assert!(ci.contains("--lib retry_commit_contract_tests"));
}

#[test]
fn ordinary_cancellation_policy_preserves_native_ownership_and_distinct_replay_validation() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let ordered = |source: &str, fragments: &[&str]| {
        let mut offset = 0;
        for fragment in fragments {
            offset += source[offset..].find(fragment).expect(fragment) + fragment.len();
        }
    };
    let adapter = read("crates/leselang-command/src/reference_cancellation_commit.rs");
    for required in [
        "pub trait MemoryCancellationJournal",
        "pub trait DurableCancellationTransaction",
        "fn commit(self, kind: CancellationCommit)",
        "pub acknowledged: &'a mut bool",
        "pub lease_expires_at_ms: &'a mut Option<u64>",
        "pub terminal_step: &'a mut Option<Step>",
        "pub use crate::reference_recovery::StoredEffect;",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "catch_unwind",
        "loop {",
        "thread",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    ordered(
        &adapter,
        &[
            "validate_cancellation_step(image, step)?",
            "if let Some(state) = host.dispatch(&image.token)",
            "*state.acknowledged = true;",
            "*state.lease_expires_at_ms = None;",
            "*state.terminal_step = Some(step.clone());",
            "}\n    host.finalize_group(&image.token)?;",
        ],
    );
    let durable = adapter.split("pub fn cancel_durable").nth(1).unwrap();
    ordered(
        durable,
        &[
            ".effect(image.token.as_str())?",
            "if existing.image != image_bytes",
            "if existing.state == \"completed\"",
            "decode_journal_value(",
            "validate_terminal_step(&authoritative)?",
            "host.commit(CancellationCommit::Replay)?",
            "return Ok(authoritative)",
            "host.ensure_growth(step_bytes.len())?",
            "host.complete(image.token.as_str(), step_bytes)?",
            "host.finalize_group(image.token.as_str())?",
            "host.commit(CancellationCommit::Cancellation)?",
        ],
    );
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(journal.contains("reference_cancellation_commit::cancel_memory(self, image, step)"));
    let native = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn cancel(")
        .nth(1)
        .unwrap()
        .split("fn cancel_audited(")
        .next()
        .unwrap();
    ordered(
        native,
        &[
            "validate_image(image)?",
            "validate_cancellation_step(image, step)?",
            "encode_json_capped(image",
            "encode_json_capped(step",
            "transaction_with_behavior(TransactionBehavior::Immediate)",
            "reference_cancellation_commit::cancel_durable(",
            "cancellation_commit::SqliteCancellationTransaction(transaction)",
        ],
    );
    assert!(!native.contains("existing.state"));
    let owner = read("crates/leselang-vm/src/journal/cancellation_commit.rs");
    for required in [
        "rusqlite::Transaction<'a>",
        "load_record(&self.0, token)",
        "ensure_growth(&self.0, additional)",
        "complete_terminal_record(&self.0",
        "finalize_merge_group(&self.0",
        "self.0.commit()",
        ".get_mut(token)",
        "failed to close cancellation transaction",
        "failed to commit effect cancellation",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    assert!(
        read("crates/leselang-command/src/reference_recovery.rs")
            .contains("pub struct StoredEffect")
    );
    assert!(
        read("crates/leselang-command/src/reference_audit_commit.rs")
            .contains("pub use crate::reference_recovery::StoredEffect;")
    );
    let cases = read("crates/leselang-vm/src/journal/cancellation_commit_contract_tests.rs");
    assert!(!cases.contains("reference_cancellation_commit::"));
    assert_eq!(cases.matches("fn frozen_cancel(").count(), 2);
    for count in [980, 2, 216, 9] {
        assert!(cases.contains(&format!("assert_eq!(cases, {count})")));
    }
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_cancellation_commit"));
    assert!(ci.contains("--lib cancellation_commit_contract_tests"));
}

#[test]
fn audit_read_projection_is_adapter_owned_without_native_query_or_error_ownership() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let adapter = read("crates/leselang-command/src/reference_audit_read.rs");
    for required in [
        "pub trait ReferenceAuditReadRow",
        "type Error;",
        "pub fn decode_record",
        "pub fn decode_replay",
        "CommandId::new(row.text(0)?)",
        "CommandId::new(command_id.clone())",
        "row.invalid_data()",
        "pub type StoredAudit",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "Fault",
        "validate_debugger_audit(",
        "valid_continuation_token(",
        "IdempotencyKey::new",
        "std::fs",
        "catch_unwind",
        "fn commit",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let replay = adapter.split("pub fn decode_replay").nth(1).unwrap();
    let mut offset = 0;
    for ordered in [
        "row.text(0)?",
        "row.text(1)?",
        "row.text(2)?",
        "CommandId::new(command_id.clone())",
        "row.text(3)?",
        "row.text(4)?",
        "row.text(5)?",
        "row.integer(6)?",
        "row.integer(7)?",
    ] {
        offset += replay[offset..].find(ordered).unwrap() + ordered.len();
    }
    let journal = read("crates/leselang-vm/src/journal.rs");
    let record = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn debugger_audit(&self, command_id: &str)")
        .nth(1)
        .unwrap()
        .split("fn expire_due(")
        .next()
        .unwrap();
    for required in [
        "WHERE command_id = ?1",
        "audit_read::decode_record",
        ".optional()",
        "failed to read debugger cancellation audit",
    ] {
        assert!(record.contains(required), "{required}");
    }
    let replay = journal
        .split("fn load_debugger_audit(")
        .nth(1)
        .unwrap()
        .split("fn finalize_merge_group(")
        .next()
        .unwrap();
    for required in [
        "WHERE command_id = ?1 OR (principal_id = ?2 AND idempotency_key = ?3)",
        "audit_read::decode_replay",
        ".optional()",
        "failed to load debugger cancellation audit",
    ] {
        assert!(replay.contains(required), "{required}");
    }
    assert!(!replay.contains("CommandId::new"));
    assert!(!record.contains("CommandId::new"));
    let owner = read("crates/leselang-vm/src/journal/audit_read.rs");
    assert_eq!(owner.matches("self.0.get(column)").count(), 2);
    assert!(owner.contains("type Error = rusqlite::Error"));
    assert!(owner.contains("rusqlite::Error::InvalidQuery"));
    assert!(owner.contains("reference_audit_read::decode_record"));
    assert!(owner.contains("reference_audit_read::decode_replay"));
    for absent in ["CommandId::new", "parse_command_origin", "Revision("] {
        assert!(!owner.contains(absent));
    }
    assert!(
        read("crates/leselang-command/src/reference_audit_commit.rs")
            .contains("pub use crate::reference_audit_read::StoredAudit;")
    );
    let cases = read("crates/leselang-vm/src/journal/audit_read_contract_tests.rs");
    let frozen = cases.split("// Frozen released projection").nth(1).unwrap();
    assert!(!frozen.contains("reference_audit_read"));
    assert!(!frozen.contains("audit_read::"));
    assert!(cases.contains("assert_eq!(cases, 1610)"));
    assert!(cases.contains("assert_eq!(cases, 180)"));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_audit_read"));
    assert!(ci.contains("--lib audit_read_contract_tests"));
}

#[test]
fn audit_commit_policy_moves_but_native_transaction_and_memory_mutation_ownership_remain() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let adapter = read("crates/leselang-command/src/reference_audit_commit.rs");
    for required in [
        "pub trait MemoryAuditJournal",
        "pub trait DurableAuditTransaction",
        "fn commit(self, kind: AuditCommit)",
        "pub fn cancel_memory",
        "pub fn cancel_durable",
        "audit_matches_record(audit, &record)",
        "debugger idempotency key was reused",
        "MAX_JOURNAL_ENTRY_BYTES",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "std::thread",
        "catch_unwind",
        "validate_terminal_step(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let mut offset = adapter.find("let revision =").unwrap();
    for ordered in [
        "let observed_at_ms =",
        "host.insert(",
        "host.ensure_growth(",
        "host.complete(",
        "host.finalize_group(",
        "host.commit(AuditCommit::Cancellation)?",
    ] {
        offset += adapter[offset..].find(ordered).unwrap() + ordered.len();
    }
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(
        journal.contains("reference_audit_commit::cancel_memory(self, image, step, audit, now_ms)")
    );
    let durable = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn cancel_audited(")
        .nth(1)
        .unwrap()
        .split("fn debugger_audit(")
        .next()
        .unwrap();
    let mut offset = 0;
    for ordered in [
        "validate_image(image)?",
        "validate_cancellation_step(image, step)?",
        "encode_json_capped(image",
        "encode_json_capped(step",
        "transaction_with_behavior(TransactionBehavior::Immediate)",
        "reference_audit_commit::cancel_durable(",
    ] {
        offset += durable[offset..].find(ordered).unwrap() + ordered.len();
    }
    assert!(!durable.contains("audit_matches_record"));
    let owner = read("crates/leselang-vm/src/journal/audit_commit.rs");
    for required in [
        "rusqlite::Transaction<'a>",
        "load_debugger_audit(&self.0, audit)",
        "load_record(&self.0, token)",
        "INSERT INTO vm_debugger_audit",
        "complete_terminal_record(&self.0",
        "finalize_merge_group(&self.0",
        "self.0.commit()",
        "EphemeralJournal::cancel(self, image, step)",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    assert!(!owner.contains("audit_matches_record"));
    let cases = read("crates/leselang-vm/src/journal/audit_commit_contract_tests.rs");
    assert!(!cases.contains("reference_audit_commit::"));
    for count in [1260, 625, 25] {
        assert!(cases.contains(&format!("assert_eq!(cases, {count})")));
    }
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_audit_commit"));
    assert!(ci.contains("--lib audit_commit_contract_tests"));
}

#[test]
fn audit_recovery_policy_is_adapter_owned_while_native_reads_stay_in_the_checkpoint() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let adapter = read("crates/leselang-command/src/reference_audit_recovery.rs");
    for required in [
        "pub trait ReferenceAuditRow",
        "pub trait ReferenceAuditSource",
        "pub fn inspect_audit_record",
        "pub fn validate_audit_records",
        "validate_debugger_audit(&context)",
        "valid_continuation_token(&token)",
        "CancellationReason::Requested",
        "terminal_observed_at == observed_at_ms",
        "decode_journal_value(terminal_bytes.as_ref(), MAX_JOURNAL_ENTRY_BYTES)",
        "row.invalid_origin()",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "std::thread",
        "catch_unwind",
        "validate_terminal_step(",
        "transaction.commit()",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    assert!(
        adapter.find("source.index_count()?").unwrap()
            < adapter.find("source.audit_count()?").unwrap()
    );
    assert!(
        adapter.find("source.audit_count()?").unwrap()
            < adapter.find("source.inspect_records()?").unwrap()
    );
    let journal = read("crates/leselang-vm/src/journal.rs");
    let owner = journal
        .split("fn validate_debugger_audit_records(")
        .nth(1)
        .unwrap()
        .split("struct MergeGraphHost")
        .next()
        .unwrap();
    for required in [
        "reference_audit_recovery::validate_audit_records",
        "reference_audit_recovery::inspect_audit_record",
        "ORDER BY a.command_id ASC",
        "self.row",
        ".get(column.index())",
        "column.error_context()",
        "rusqlite::Error::InvalidQuery",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    for forbidden in [
        "DebuggerAuditContext",
        "CommandId::new",
        "IdempotencyKey::new",
        "CancellationReason",
        "decode_bounded",
        "transaction.commit()",
    ] {
        assert!(!owner.contains(forbidden), "{forbidden}");
    }
    assert!(journal.contains("validate_debugger_audit_records(self.connection()?)"));
    let cases = read("crates/leselang-vm/src/journal/audit_recovery_contract_tests.rs");
    let frozen = cases.split("fn frozen_validate_audits(").nth(1).unwrap();
    assert!(!frozen.contains("reference_audit_recovery"));
    assert!(frozen.contains("crate::validate_debugger_audit(&context)"));
    assert!(cases.contains("assert_eq!(cases, 640)"));
    assert!(cases.contains("assert_eq!(cases, 35)"));
    let ci = read(".github/workflows/ci.yml");
    for required in [
        "--test reference_audit_recovery",
        "--lib audit_recovery_contract_tests",
        "--test debugger_audit_durable --test debugger_audit_memory",
    ] {
        assert!(ci.contains(required), "{required}");
    }
}

#[test]
fn reference_journal_schema_moves_without_native_file_or_transaction_ownership() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let adapter = read("crates/leselang-command/src/reference_journal_schema.rs");
    for required in [
        "pub const JOURNAL_SCHEMA_VERSION: u32 = 10",
        "pub fn migrate(",
        "FnMut(&'static str, &'static str) -> Result<(), Fault>",
        "BEGIN IMMEDIATE;",
        "CREATE TABLE vm_debugger_audit",
        "CREATE INDEX IF NOT EXISTS vm_dispatch_attempt_order_idx",
        "unsupported journal version",
        "LSV4004",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "std::thread",
        "Connection",
        "catch_unwind",
        "ROLLBACK",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let journal = read("crates/leselang-vm/src/journal.rs");
    let open = journal
        .split("impl SqliteJournal {")
        .nth(1)
        .unwrap()
        .split("fn load(")
        .next()
        .unwrap();
    for required in [
        "create_private_journal(path)?",
        "SQLITE_OPEN_NOFOLLOW",
        "tighten_permissions(path)?",
        "Limit::SQLITE_LIMIT_LENGTH",
        "busy_timeout(JOURNAL_BUSY_TIMEOUT)",
        "PRAGMA trusted_schema = OFF",
        "query_row(\"PRAGMA user_version\"",
        "reference_journal_schema::migrate(version, |sql, context|",
        ".execute_batch(sql)",
        "journal_error(\"LSV4003\", context, error)",
    ] {
        assert!(open.contains(required), "{required}");
    }
    for absent in [
        "CREATE TABLE",
        "if version <",
        "PRAGMA user_version =",
        "unsupported journal version",
    ] {
        assert!(!open.contains(absent), "{absent}");
    }
    assert!(
        open.find("PRAGMA trusted_schema = OFF").unwrap()
            < open.find("reference_journal_schema::migrate").unwrap()
    );
    assert!(
        journal.contains(
            "pub use leselang_command::reference_journal_schema::JOURNAL_SCHEMA_VERSION;"
        )
    );
    let cases = read("crates/leselang-vm/src/journal/schema_contract_tests.rs");
    let frozen = cases.split("fn frozen_schema(").nth(1).unwrap();
    assert!(!frozen.contains("reference_journal_schema"));
    assert!(frozen.contains("const JOURNAL_SCHEMA_VERSION: u32 = 10"));
    assert!(frozen.contains("CREATE TABLE vm_dispatches"));
    assert!(cases.contains("assert_eq!(cases, 180)"));
    let ci = read(".github/workflows/ci.yml");
    for required in [
        "--test reference_journal_schema",
        "--lib schema_contract_tests",
        "--lib journal_v",
    ] {
        assert!(ci.contains(required), "{required}");
    }
}

#[test]
fn root_admission_lifecycle_is_adapter_owned_with_a_compatible_private_vm_port() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_admission.rs"))
            .unwrap();
    for required in [
        "pub trait ReferenceAdmissionHost",
        "pub struct RootAdmission",
        "pub enum AdmissionPoll",
        "inner: Admission<AdmissionRoot>",
        "deadline_at_ms - now_ms",
        "error.code == \"LSV2501\"",
        "host: &mut Host",
        "AdmissionOwner(host)",
        "#[must_use",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::thread",
        "std::time",
        "unsafe",
        "catch_unwind",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let facade = fs::read_to_string(root().join("crates/leselang-vm/src/admission.rs")).unwrap();
    for required in [
        "pub use leselang_command::reference_admission::AdmissionPoll;",
        "inner: ReferenceRootAdmission",
        "std::fmt::Debug::fmt(&self.inner, formatter)",
        "NativeAdmissionOwner(vm)",
        "impl ReferenceAdmissionHost for NativeAdmissionOwner<'_>",
        "self.0.start_timed(",
        "#[must_use",
    ] {
        assert!(facade.contains(required), "{required}");
    }
    for absent in [
        "LSV2501",
        "AdmissionAttempt",
        "struct AdmissionRoot",
        "pub enum AdmissionPoll",
        "impl ReferenceAdmissionHost for Vm",
        "impl AdmissionAdapter",
    ] {
        assert!(!facade.contains(absent), "{absent}");
    }
    let parity =
        fs::read_to_string(root().join("crates/leselang-vm/src/admission_contract_tests.rs"))
            .unwrap();
    let frozen = parity.split("mod frozen {").nth(1).unwrap();
    assert!(!frozen.contains("reference_admission"));
    assert!(frozen.contains("impl AdmissionAdapter<AdmissionRoot> for Vm"));
    assert!(frozen.contains("error.code == \"LSV2501\""));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    for required in [
        "--test reference_admission",
        "--lib admission_contract_tests",
        "--test admission",
    ] {
        assert!(ci.contains(required), "{required}");
    }
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn root_publication_is_product_policy_over_private_native_commit_and_cache_hooks() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let adapter = read("crates/leselang-command/src/reference_publication.rs");
    assert!(adapter.contains("pub trait ReferenceRootPublicationHost"));
    assert!(adapter.contains("pub(crate) fn publish_group<"));
    assert!(!adapter.contains("pub fn publish_group<"));
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        ".clone()",
        "catch_unwind",
        "record_pending(",
        "record_merge_graph_with_limits(",
        "allocate_sequence(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let vm = read("crates/leselang-vm/src/leserpent_profile.rs");
    let owner = vm
        .split_once("impl leselang_command::reference_publication::ReferenceRootPublicationHost")
        .unwrap()
        .1
        .split_once("fn encode_json_capped")
        .unwrap()
        .0;
    assert!(owner.contains("for ReferenceStartOwner<'_>"));
    assert!(!owner.contains("for Vm"));
    assert!(owner.contains("Some(request)"));
    assert!(owner.contains("self.0.scheduler_limits"));
    assert!(owner.contains("self.0.pending.insert(image.token.clone(), image.clone())"));
    for forbidden in ["Step::", "for branch", "if order", "Err(", "unwrap", "LSV"] {
        assert!(!owner.contains(forbidden), "{forbidden}");
    }
    let tests = read("crates/leselang-vm/src/publication_contract_tests.rs");
    for count in [64, 13] {
        assert!(tests.contains(&format!("assert_eq!(cases, {count})")));
    }
    let frozen = tests
        .split_once("// Frozen pre-extraction publication.")
        .unwrap()
        .1;
    assert!(!frozen.contains("reference_publication"));
    assert!(frozen.contains("record_merge_graph_with_limits("));
    assert!(frozen.contains(".record_pending("));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_publication"));
    assert!(ci.contains("--lib publication_contract_tests"));
}

#[test]
fn startup_policy_keeps_validation_before_native_open_and_recovery_before_publication() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let vm = read("crates/leselang-vm/src/leserpent_profile.rs");
    let adapter = read("crates/leselang-command/src/reference_startup.rs");
    let constructors = vm
        .split_once("    pub fn new(fuel_limit")
        .unwrap()
        .1
        .split_once("    /// Lowering capacity")
        .unwrap()
        .0;
    for entry in [
        "reference_startup::in_memory(",
        "reference_startup::in_memory_with_limits(",
        "reference_startup::recover(",
        ".map(Self::from_startup)",
    ] {
        assert!(constructors.contains(entry), "{entry}");
    }
    assert!(!constructors.contains(".min("));
    assert!(!constructors.contains("BTreeMap::new()"));
    assert!(!constructors.contains("LSV"));
    let durable = constructors
        .split_once("reference_startup::recover(")
        .unwrap()
        .1
        .split_once("    fn from_startup(")
        .unwrap()
        .0;
    assert!(durable.find("|| {").unwrap() < durable.find("Journal::open(path.as_ref())?").unwrap());
    assert!(
        durable.find("Journal::open(path.as_ref())?").unwrap()
            < durable.find("RecoveredJournal {").unwrap()
    );
    assert!(!durable.contains("ephemeral"));
    assert!(!durable.contains("unwrap"));
    let flow = adapter.split_once("pub fn recover<").unwrap().1;
    let mut offset = 0;
    for stage in [
        "validate_limits(limits)?",
        "let recovered = open()?",
        "Ok(StartupState {",
        "fuel_limit.min(MAX_EXECUTION_FUEL)",
        "pending: recovered.pending",
        "completed: recovered.completed",
        "journal: recovered.journal",
    ] {
        offset += flow[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out of order {stage}"))
            + stage.len();
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "std::path",
        ".clone()",
        "catch_unwind",
        "loop {",
        "Serialize",
        "Deserialize",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let journal = read("crates/leselang-vm/src/journal.rs");
    let open = journal
        .split_once("pub fn open(path: &Path)")
        .unwrap()
        .1
        .split_once("pub fn allocate_sequence")
        .unwrap()
        .0;
    assert!(
        open.find("SqliteJournal::open(path)?").unwrap() < open.find("journal.load()?").unwrap()
    );
    assert!(
        open.find("journal.load()?").unwrap()
            < open.find("Ok((Self::Sqlite(journal), snapshot))").unwrap()
    );
    let frozen = read("crates/leselang-vm/src/startup_contract_tests.rs");
    let frozen = frozen
        .split_once("// Frozen pre-extraction constructors")
        .unwrap()
        .1;
    assert!(!frozen.contains("reference_startup"));
    assert!(frozen.contains("Journal::open(path.as_ref())?"));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_startup"));
    assert!(ci.contains("--lib startup_contract_tests"));
}

#[test]
fn sequence_selection_keeps_native_reservation_before_watermark_and_local_occupancy() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let vm = read("crates/leselang-vm/src/leserpent_profile.rs");
    let adapter = read("crates/leselang-command/src/reference_allocation.rs");
    let bridge = vm
        .split_once("    fn allocate_sequence(&mut self)")
        .unwrap()
        .1
        .split_once("    fn expire_due(")
        .unwrap()
        .0;
    assert!(bridge.contains("reference_allocation::allocate_sequence("));
    assert!(
        bridge
            .split_whitespace()
            .collect::<String>()
            .replace(",)", ")")
            .contains("&mutReferenceAllocationOwner(self)")
    );
    for forbidden in [
        "self.journal",
        "self.pending",
        "self.completed",
        "self.next_effect_id",
        "loop",
        "format!",
        "LSV",
    ] {
        assert!(!bridge.contains(forbidden));
    }
    let owner = vm
        .split_once("struct ReferenceAllocationOwner")
        .unwrap()
        .1
        .split_once("struct ReferenceClaimOwner")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for hook in [
        "self.0.next_effect_id",
        "self.0.journal.allocate_sequence(local_next)",
        "self.0.next_effect_id=next_sequence",
        "self.0.pending.contains_key(token)",
        "self.0.completed.contains_key(token)",
    ] {
        assert!(owner.contains(hook), "{hook}");
    }
    for forbidden in [".clone()", ".insert(", ".remove(", ".max("] {
        assert!(!owner.contains(forbidden));
    }
    assert!(
        !vm.split_whitespace()
            .collect::<String>()
            .contains("ReferenceAllocationHostforVm")
    );
    assert!(vm.contains("#[cfg(test)]\nfn effect_sequence_exhausted_fault("));
    let flow = adapter.split_once("pub fn allocate_sequence").unwrap().1;
    let mut offset = 0;
    for stage in [
        "loop {",
        "host.local_next()",
        "host.allocate_sequence(local_next)?",
        "sequence == 0 || sequence > MAX_EFFECT_SEQUENCE",
        "SequenceWatermarkRequirement::after_observation(sequence, u64::MAX)",
        "host.publish_next(next)",
        "format!(\"continuation-{sequence}\")",
        "!host.pending_contains(&token) && !host.completed_contains(&token)",
        "return Ok(sequence)",
    ] {
        offset += flow[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out of order {stage}"))
            + stage.len();
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "std::time",
        "std::thread",
        ".clone()",
        "sort_",
        ".max(",
    ] {
        assert!(!adapter.contains(forbidden));
    }
    assert!(adapter.contains("ReferenceAllocationHost + ?Sized"));
    let frozen = read("crates/leselang-vm/src/allocation_contract_tests.rs");
    let frozen = frozen
        .split_once("// Frozen pre-extraction selection")
        .unwrap()
        .1;
    assert!(frozen.contains("vm.journal.allocate_sequence(vm.next_effect_id)?"));
    assert!(!frozen.contains("reference_allocation"));
    assert!(!frozen.contains("vm.allocate_sequence()"));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_allocation"));
    assert!(ci.contains("--lib allocation_contract_tests"));
}

#[test]
fn maintenance_policy_preserves_admission_sort_commit_and_native_cache_mutations() {
    let read = |path| fs::read_to_string(root().join(path)).unwrap();
    let vm = read("crates/leselang-vm/src/leserpent_profile.rs");
    let adapter = read("crates/leselang-command/src/reference_settlement/maintenance.rs");
    for (from, to, delegation) in [
        (
            "pub fn compact_journal(",
            "fn replay_completed(",
            "maintenance::compact_journal(",
        ),
        (
            "fn expire_due(",
            "fn finish_cancellation(",
            "maintenance::expire_due(",
        ),
    ] {
        let bridge = vm.split_once(from).unwrap().1.split_once(to).unwrap().0;
        assert!(bridge.contains(delegation));
        assert!(bridge.contains("&mut ReferenceMaintenanceOwner(self)"));
        for absent in [
            "self.journal",
            "self.pending",
            "self.completed",
            ".clone()",
            "validate_",
            "sort_by",
        ] {
            assert!(!bridge.contains(absent));
        }
    }
    let owner = vm
        .split_once("struct ReferenceMaintenanceOwner")
        .unwrap()
        .1
        .split_once("impl leselang_command::reference_claim")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for required in [
        ".map(|(token,step)|(token.clone(),step.clone()))",
        "self.0.journal.compact_completed(local_completed,policy)",
        "self.0.completed.remove(token)",
        "self.0.journal.expire_due(now_ms)",
        "self.0.pending.remove(token)",
        "self.0.completed.insert(token,step)",
    ] {
        assert!(owner.contains(required), "{required}");
    }
    assert_eq!(owner.matches(".clone()").count(), 2);
    let compact_vm = vm.split_whitespace().collect::<String>();
    for forbidden in [
        "ReferenceRetentionHostforVm",
        "ReferenceExpirationHostforVm",
    ] {
        assert!(!compact_vm.contains(forbidden));
    }
    assert!(vm.contains("#[cfg(test)]\nfn validate_scheduler_time("));
    assert!(vm.contains("#[cfg(test)]\nfn validate_retention_policy("));
    for forbidden in [
        ".clone()",
        "sort_unstable",
        "visible_step",
        "rusqlite",
        "std::fs",
        "std::time",
        "deadline_cancellation(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    for (from, stages) in [
        (
            "pub fn compact_journal",
            vec![
                "validate_retention_policy(policy, MAX_JOURNAL_RECORDS)",
                "host.completed_snapshot()",
                "local_completed.sort_by(",
                "host.compact_completed(&local_completed, policy)?",
                "for token in &compacted.pruned_tokens",
                "host.remove_completed(token)",
                "removed_records: compacted.removed_records",
            ],
        ),
        (
            "pub fn expire_due",
            vec![
                "validate_clock(now_ms)?",
                "host.expire_due(now_ms)?",
                "for (image, step) in expired",
                "host.remove_pending(&image.token)",
                "host.insert_expired(image.token, step)",
            ],
        ),
    ] {
        let flow = adapter.split_once(from).unwrap().1;
        let mut offset = 0;
        for stage in stages {
            offset += flow[offset..]
                .find(stage)
                .unwrap_or_else(|| panic!("missing/out of order {stage}"))
                + stage.len();
        }
    }
    let journal = read("crates/leselang-vm/src/journal.rs");
    assert!(journal.contains(
        "pub(crate) use leselang_command::reference_settlement::maintenance::JournalCompaction"
    ));
    assert!(!journal.contains("struct JournalCompaction"));
    let frozen = read("crates/leselang-vm/src/maintenance_contract_tests.rs");
    let frozen = frozen
        .split_once("// Frozen pre-extraction coordination")
        .unwrap()
        .1;
    assert!(!frozen.contains("reference_settlement"));
    assert!(!frozen.contains("vm.compact_journal("));
    assert!(!frozen.contains("vm.expire_due("));
    assert!(frozen.contains("vm.journal.compact_completed(&local_completed, policy)?"));
    assert!(frozen.contains("vm.journal.expire_due(now_ms)?"));
    let ci = read(".github/workflows/ci.yml");
    assert!(ci.contains("--test reference_maintenance"));
    assert!(ci.contains("--lib maintenance_contract_tests"));
}

#[test]
fn group_visibility_and_completion_cache_policy_keep_native_storage_and_borrowing() {
    let root = root();
    let read = |path| std::fs::read_to_string(root.join(path)).unwrap();
    let vm = read("crates/leselang-vm/src/leserpent_profile.rs");
    let adapter = read("crates/leselang-command/src/reference_settlement/visibility.rs");
    let journal = read("crates/leselang-vm/src/journal.rs");
    let frozen = read("crates/leselang-vm/src/visibility_contract_tests.rs");
    let wrappers = vm
        .split_once("    fn reconcile_completed(")
        .unwrap()
        .1
        .split_once("    fn allocate_sequence(")
        .unwrap()
        .0;
    for delegation in [
        "visibility::reconcile_completed(",
        "visibility::visible_step(",
        "&mut ReferenceVisibilityOwner(self)",
        "Ok(self.completed.get(token))",
    ] {
        assert!(wrappers.contains(delegation), "{delegation}");
    }
    for forbidden in [
        "self.journal",
        ".clone()",
        ".remove(",
        ".insert(",
        "MergeProgress::",
        "EffectObservation::",
    ] {
        assert!(!wrappers.contains(forbidden), "{forbidden}");
    }
    assert!(vm.contains("struct ReferenceVisibilityOwner<'a>(&'a mut Vm)"));
    assert!(!vm.contains("ReferenceCompletionCacheHost for Vm"));
    assert!(!vm.contains("ReferenceGroupVisibilityHost for Vm"));
    let owner = vm
        .split_once("struct ReferenceVisibilityOwner")
        .unwrap()
        .1
        .split_once("impl leselang_command::reference_settlement::interruption")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for operation in [
        "self.0.completed.contains_key(token)",
        "self.0.journal.completed_exists(token)",
        "self.0.journal.observe_effect(token)",
        "self.0.completed.remove(token)",
        "self.0.completed.insert(token.clone(), step)",
        "self.0.journal.merge_progress(token)",
        "self.0.pending.remove(token).is_some()",
        "self.0.completed.entry(token).or_insert_with(|| step.clone())",
        "request.continuation.token.clone()",
        "request.continuation.clone()",
        "debug_assert!(",
    ] {
        assert!(
            owner.contains(&operation.split_whitespace().collect::<String>()),
            "{operation}"
        );
    }
    assert_eq!(owner.matches(".clone()").count(), 4);
    assert!(!adapter.contains(".clone()"));
    assert!(adapter.contains("EffectObservation::Untracked | EffectObservation::Pending => {}"));
    assert!(adapter.contains("EffectObservation::Missing => host.remove_pending(token)"));
    assert!(adapter.contains("if host.remove_pending(&token)"));
    assert!(adapter.contains("host.cache_merged_if_absent(token, &step)"));
    assert!(adapter.contains("host.debug_assert_merge_result(&merge_token)"));
    let next = adapter
        .split_once("Ok(MergeProgress::Next {")
        .unwrap()
        .1
        .split_once("Ok(MergeProgress::Pending")
        .unwrap()
        .0;
    assert!(next.find("host.remove_pending").unwrap() < next.find("host.cache_next").unwrap());
    let reconcile = adapter
        .split_once("pub fn reconcile_completed")
        .unwrap()
        .1
        .split_once("pub trait ReferenceGroupVisibilityHost")
        .unwrap()
        .0;
    assert!(
        reconcile.find("host.remove_pending(token);").unwrap()
            < reconcile
                .find("host.insert_completed(token, *step)")
                .unwrap()
    );
    assert!(
        journal.contains("pub(crate) use leselang_command::reference_settlement::visibility::{")
    );
    assert!(!journal.contains("enum MergeProgress"));
    assert!(!journal.contains("enum EffectObservation"));
    let oracles = frozen
        .split_once("// Frozen before visibility/cache policy extraction")
        .unwrap()
        .1;
    assert!(oracles.contains("vm.journal.observe_effect(token)?"));
    assert!(oracles.contains("vm.journal.merge_progress(branch_token)"));
    for migrated in [
        "visibility::",
        "vm.visible_step(",
        "vm.reconcile_completed(",
    ] {
        assert!(!oracles.contains(migrated));
    }
}

fn assert_interruption_settlement_boundary(vm: &str) {
    for (from, to, delegation) in [
        (
            "fn settle_effect_error(",
            "fn publish_retry_terminal(",
            "interruption::settle_effect_error(",
        ),
        (
            "fn finish_audited_cancellation(",
            "pub fn debugger_audit(",
            "interruption::finish_audited_cancellation(",
        ),
        (
            "fn finish_cancellation(",
            "struct ReferenceInterruptionSettlementOwner",
            "interruption::finish_cancellation(",
        ),
    ] {
        let bridge = vm.split_once(from).unwrap().1.split_once(to).unwrap().0;
        assert!(bridge.contains(delegation));
        for absent in [
            "self.journal",
            "self.pending",
            "self.completed",
            "self.visible_step",
            ".clone()",
        ] {
            assert!(!bridge.contains(absent), "{from}: {absent}");
        }
    }
    let owner = vm
        .split_once("struct ReferenceInterruptionSettlementOwner")
        .unwrap()
        .1
        .split_once("struct ReferenceClaimOwner")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for hook in [
        "self.0.journal.cancel(image,proposed)",
        "self.0.journal.cancel_audited(image,proposed,audit,now_ms)",
        "self.0.journal.report_error(lease,now_ms,proposed)",
        "self.0.publish_completed(image,authoritative)",
        "self.0.publish_retry_terminal(image,authoritative)",
    ] {
        assert!(owner.contains(hook), "{hook}");
    }
    let publication = vm
        .split_once("fn publish_retry_terminal(")
        .unwrap()
        .1
        .split_once("pub fn cancel_effect(")
        .unwrap()
        .0;
    let mut offset = 0;
    for stage in [
        "self.pending.remove(&image.token)",
        "self.completed.insert(image.token.clone(), step.clone())",
        "self.visible_step(&image.token, step.clone())",
    ] {
        offset += publication[offset..].find(stage).unwrap() + stage.len();
    }
    assert_eq!(publication.matches(".clone()").count(), 3);
    let adapter = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_settlement/interruption.rs"),
    )
    .unwrap();
    let audited = adapter
        .split_once("pub fn finish_audited_cancellation<")
        .unwrap()
        .1
        .split_once("pub fn settle_effect_error<")
        .unwrap()
        .0;
    let retry = adapter.split_once("pub fn settle_effect_error<").unwrap().1;
    for (flow, stages) in [
        (
            audited,
            vec![
                "host.cancel_audited(image, step, audit, now_ms)?",
                "matches!(authoritative, Step::Cancelled(_))",
                "host.publish_cancellation(image, authoritative)",
                "return Err(error)",
                "Ok(record)",
            ],
        ),
        (
            retry,
            vec![
                "host.report_error(lease, now_ms, disposition)?",
                "RetryDisposition::Terminal(step) = &authoritative",
                "Ok(RetryDisposition::Terminal(",
                "host.publish_retry_terminal(&lease.request.continuation, step)",
                "Ok(authoritative)",
            ],
        ),
    ] {
        let mut offset = 0;
        for stage in stages {
            offset += flow[offset..].find(stage).unwrap() + stage.len();
        }
    }
}

#[test]
fn cancellation_and_retry_settlement_keep_distinct_returns_and_native_publication() {
    let adapter = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_settlement/interruption.rs"),
    )
    .unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel",
        "BTreeMap",
        "LSV",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert_eq!(adapter.matches(".clone()").count(), 1);
    assert!(adapter.contains("continuation: image.token.clone()"));
    for name in [
        "ReferenceCancellationSettlementHost + ?Sized",
        "ReferenceRetrySettlementHost + ?Sized",
    ] {
        assert!(adapter.contains(name));
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert_interruption_settlement_boundary(&vm);
    let successor = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    assert!(successor.contains("pub(super) fn publish_completed("));
    assert_settlement_commit_before_publication(&successor);
    let cancellation =
        fs::read_to_string(root().join("crates/leselang-vm/src/cancellation_contract_tests.rs"))
            .unwrap();
    let retry =
        fs::read_to_string(root().join("crates/leselang-vm/src/retry_contract_tests.rs")).unwrap();
    assert!(!cancellation.contains("vm.finish_cancellation("));
    assert!(
        cancellation.contains(
            "frozen_finish_cancellation(vm, image, CancellationReason::Requested, now_ms)"
        )
    );
    for oracle in [&cancellation, &retry] {
        assert!(!oracle.contains("reference_settlement"));
        assert!(!oracle.contains("vm.publish_completed("));
        assert!(!oracle.contains("vm.publish_retry_terminal("));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_interruption_settlement"));
}

fn assert_settlement_commit_before_publication(vm: &str) {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_settlement.rs"))
            .unwrap();
    let result = adapter
        .split_once("pub fn record_result<")
        .unwrap()
        .1
        .split_once("pub fn commit_successor<")
        .unwrap()
        .0;
    for commit in [
        "host.acknowledge_dispatch(lease, now_ms, &step)",
        "host.record_completed(image, &step)",
    ] {
        assert!(
            result.find(commit).unwrap()
                < result.find("let authoritative = match recorded").unwrap()
        );
    }
    for flow in [
        result,
        adapter.split_once("pub fn commit_successor<").unwrap().1,
    ] {
        assert!(
            flow.find("Err(error) => return Step::Fault(error)")
                .unwrap()
                < flow
                    .find("host.publish_completed(image, authoritative)")
                    .unwrap()
        );
        assert_eq!(flow.matches("host.publish_completed(").count(), 1);
    }
    let owner = vm
        .split_once("struct ReferenceSettlementOwner")
        .unwrap()
        .1
        .split_once("impl Vm {")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for call in [
        "self.0.journal.record_completed(image,proposed)",
        "self.0.journal.acknowledge_dispatch(lease,now_ms,proposed)",
        "self.0.journal.record_successor(parent,proposed,next,lease)",
        "self.0.publish_completed(image,authoritative)",
    ] {
        assert!(owner.contains(call), "{call}");
    }
    let native = vm.split_once("impl Vm {").unwrap().1;
    let bridges = native
        .split_once("fn commit_successor(")
        .unwrap()
        .1
        .split_once("fn publish_completed(")
        .unwrap()
        .0;
    for call in [
        "reference_settlement::commit_successor(",
        "reference_settlement::record_result(",
    ] {
        assert!(bridges.contains(call));
    }
    for absent in [
        "self.journal",
        "self.pending",
        "self.completed",
        "self.visible_step",
        ".clone()",
    ] {
        assert!(!bridges.contains(absent));
    }
    let publication = native
        .split_once("fn publish_completed(")
        .unwrap()
        .1
        .split_once("#[cfg(test)]")
        .unwrap()
        .0;
    let mut offset = 0;
    for stage in [
        "self.pending.remove(&image.token)",
        "self.completed",
        ".insert(image.token.clone(), authoritative.clone())",
        "self.visible_step(&image.token, authoritative)",
    ] {
        offset += publication[offset..].find(stage).unwrap() + stage.len();
    }
    assert_eq!(publication.matches(".clone()").count(), 2);
}

#[test]
fn product_settlement_routes_preserve_native_commit_before_publication_and_peer_winners() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_settlement.rs"))
            .unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel",
        ".clone()",
        "BTreeMap",
        "LSV",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("ReferenceSettlementHost + ?Sized"));
    assert!(adapter.contains("Backend-specific replay semantics are not normalized here"));
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    assert_settlement_commit_before_publication(&vm);
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/settlement_contract_tests.rs"))
            .unwrap();
    assert!(!frozen.contains("reference_settlement"));
    assert!(!frozen.contains("vm.publish_completed("));
    assert!(frozen.contains("assert_eq!(comparisons, 4)"));
    let completion =
        fs::read_to_string(root().join("crates/leselang-vm/src/completion_contract_tests.rs"))
            .unwrap();
    let assembly = fs::read_to_string(
        root().join("crates/leselang-vm/src/successor_assembly_contract_tests.rs"),
    )
    .unwrap();
    for source in [&completion, &assembly] {
        assert!(source.contains("frozen_record_result("));
        assert!(!source.contains("vm.record_result("));
    }
    assert!(completion.contains("assembly_contract_tests::frozen_complete_successor("));
    assert!(assembly.contains("vm.journal.record_successor(&parent, &step, &next, lease)"));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_settlement"));
    assert!(ci.contains("--lib settlement_contract_tests::settlement_sqlite"));
}

#[test]
fn result_conversion_and_bound_observation_are_product_owned_without_settlement_authority() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_result.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        ".clone()",
        "record_completed",
        "acknowledge_dispatch",
        "BTreeMap",
        "impl HostResultDomain",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert_eq!(adapter.matches("Fuel::new(").count(), 1);
    let entry = adapter
        .split_once("pub fn step_from_effect_result(")
        .unwrap()
        .1;
    let mut offset = 0;
    for ordered in [
        "atomic_step_from_effect_result(image, operation, result)",
        "let Some(binding) = &image.result_binding",
        "let Step::Done(value) = step",
        "accept_bound_value(image, &value)",
        "Fuel::new(image.fuel_remaining)",
        "HostOperation::for_effect(&image.pending_effect)",
        "reference_computation::resume(binding, value, operation, &mut fuel)",
    ] {
        offset += entry[offset..].find(ordered).unwrap() + ordered.len();
    }
    for required in [
        "PendingReply::new(image.token.as_str(), image, &declaration)",
        "accepted.into_parts()",
        "observation-only checkpoint grants no new execution authority",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let bridge = vm
        .split_once("fn step_from_effect_result(")
        .unwrap()
        .1
        .split_once("fn fault(code:")
        .unwrap()
        .0;
    for required in [
        "reference_result::step_from_effect_result(",
        "reference_result::atomic_step_from_effect_result(",
    ] {
        assert!(bridge.contains(required));
    }
    for absent in [
        "LSV",
        "Fuel::new",
        "result_binding",
        "into_effect_value",
        "accept_bound_value",
    ] {
        assert!(!bridge.contains(absent));
    }
    let oracle = fs::read_to_string(
        root().join("crates/leselang-vm/src/result_conversion_contract_tests.rs"),
    )
    .unwrap();
    assert!(oracle.contains("assert_eq!(comparisons, 6720)"));
    assert!(
        !oracle
            .split_once("pub(crate) mod frozen")
            .unwrap()
            .1
            .contains("reference_result")
    );
    let completion =
        fs::read_to_string(root().join("crates/leselang-vm/src/completion_contract_tests.rs"))
            .unwrap();
    assert!(completion.contains("use crate::result_conversion_contract_tests::frozen::{"));
    for count in [5600, 64] {
        assert!(completion.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_result"));
    assert!(ci.contains("--lib result_conversion_contract_tests::conversion_matches"));
    assert!(ci.contains("--lib completion_contract_tests::result_completion"));
}

#[test]
fn claim_entries_preserve_native_issuance_and_post_commit_cache_adoption() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_claim.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel::new",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("ReferenceClaimHost + ?Sized"));
    let entry = adapter
        .split_once("pub fn try_claim_effect<")
        .unwrap()
        .1
        .split_once("pub fn adopt_claimed_image(")
        .unwrap()
        .0;
    assert!(!entry.contains(".clone()"));
    let mut offset = 0;
    for ordered in [
        "validate_clock(now_ms)?",
        "validate_lease_clock(now_ms, lease_ms)?",
        "host.expire_due(now_ms)?",
        "host.claim_dispatch(now_ms, lease_ms)?",
        "DispatchClaim::Leased(lease)",
        "host.adopt_claimed_image(&lease.request.continuation)?",
        "Ok(claim)",
    ] {
        offset += entry[offset..].find(ordered).unwrap() + ordered.len();
    }
    assert!(adapter.contains("Err(lease_capacity_fault(pressure))"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let bridges = vm
        .split_once("pub fn claim_effect(")
        .unwrap()
        .1
        .split_once("fn adopt_claimed_image(")
        .unwrap()
        .0;
    for required in [
        "reference_claim::claim_effect(",
        "reference_claim::try_claim_effect(",
    ] {
        assert!(bridges.contains(required));
    }
    for absent in ["self.journal", "self.pending", "LSV", "self.expire_due"] {
        assert!(!bridges.contains(absent));
    }
    let adoption = vm
        .split_once("fn adopt_claimed_image(")
        .unwrap()
        .1
        .split_once("pub fn acknowledge_effect(")
        .unwrap()
        .0;
    assert!(adoption.contains("reference_claim::adopt_claimed_image(&mut self.pending, image)"));
    assert!(!adoption.contains(".clone()"));
    assert!(!adoption.contains(".entry("));
    let cache = adapter
        .split_once("pub fn adopt_claimed_image(")
        .unwrap()
        .1
        .split_once("pub fn validate_claimed_image(")
        .unwrap()
        .0;
    assert!(cache.contains("pending.entry(image.token.clone())"));
    assert!(cache.contains("validate_claimed_image(entry.get(), image)?"));
    assert!(cache.contains("entry.insert(image.clone())"));
    assert_eq!(cache.matches(".clone()").count(), 2);
    let owner = vm
        .split_once("struct ReferenceClaimOwner")
        .unwrap()
        .1
        .split_once("struct ReferenceRetryOwner")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for hook in [
        "self.0.expire_due(now_ms)",
        "self.0.journal.claim_dispatch(now_ms,lease_ms,self.0.scheduler_limits)",
        "self.0.adopt_claimed_image(image)",
    ] {
        assert!(owner.contains(hook), "{hook}");
    }
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/claim_contract_tests.rs")).unwrap();
    assert!(
        !oracle
            .split_once("// Frozen complete claim entries")
            .unwrap()
            .1
            .contains("reference_claim")
    );
    for count in [2800, 176] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    assert!(oracle.contains("assert_eq!(observed_write_failures, BTreeSet::from([1, 2, 3]))"));
    assert!(oracle.contains("lease is committed before local conflict"));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_claim"));
    assert!(ci.contains("--lib claim_contract_tests::claim_entries"));
}

#[test]
fn retry_entry_preserves_replay_order_and_native_settlement_authority() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_retry.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel::new",
        ".clone()",
        "report_error(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("ReferenceRetryHost + ?Sized"));
    assert!(adapter.contains("lease: &DispatchLease"));
    assert!(adapter.contains("disposition: &RetryDisposition"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let bridge = vm
        .split_once("pub fn report_effect_error(")
        .unwrap()
        .1
        .split_once("fn settle_effect_error(")
        .unwrap()
        .0;
    assert!(bridge.contains("reference_retry::report_effect_error("));
    for absent in [
        "self.journal",
        "self.pending",
        "LSV",
        "plan_effect_retry(",
        "self.replay_completed",
    ] {
        assert!(!bridge.contains(absent), "{absent}");
    }
    assert_interruption_settlement_boundary(&vm);
    let owner = vm
        .split_once("struct ReferenceRetryOwner")
        .unwrap()
        .1
        .split_once("struct ReferenceCancellationOwner")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for hook in [
        "self.0.expire_due(now_ms)",
        "self.0.replay_completed(token)",
        "self.0.visible_step(token,completed)",
        "self.0.settle_effect_error(lease,now_ms,disposition)",
    ] {
        assert!(owner.contains(hook), "{hook}");
    }
    assert!(vm.contains("leselang_command::reference_retry::effect_retry_fault(error)"));
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/retry_contract_tests.rs")).unwrap();
    assert!(
        !oracle
            .split_once("// Frozen complete retry entry")
            .unwrap()
            .1
            .contains("reference_retry")
    );
    for count in [1680, 192] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    assert!(oracle.contains("assert_eq!(observed_write_failures, BTreeSet::from([1, 2, 3]))"));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_retry"));
    assert!(ci.contains("--lib retry_contract_tests::retry_entry"));
}

#[test]
fn cancellation_entries_preserve_distinct_replay_and_native_commit_boundaries() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_cancellation.rs"))
            .unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel::new",
        "record_completed(",
        "cancel_audited(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("ReferenceCancellationHost + ?Sized"));
    assert!(adapter.contains("Result<Option<&Step>, Fault>"));
    let ordinary = adapter
        .split_once("pub fn cancel_effect<")
        .unwrap()
        .1
        .split_once("pub fn cancel_effect_audited<")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    let mut offset = 0;
    for stage in [
        "validate_clock(now_ms)",
        "host.expire_due(now_ms)",
        "host.replay_completed(&image.token)",
        "host.visible_step(&image.token,completed)",
        "host.pending(&image.token)",
        "stored!=image",
        "host.finish_cancellation(image,CancellationReason::Requested,now_ms)",
    ] {
        offset += ordinary[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    let audited = adapter
        .split_once("pub fn cancel_effect_audited<")
        .unwrap()
        .1
        .split_once("pub fn validate_audit(")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    let mut offset = 0;
    for stage in [
        "validate_audit(audit)?",
        "validate_clock(now_ms)?",
        "host.expire_due(now_ms)?",
        "host.reconcile_completed(&image.token)?",
        "CancellationReason::Requested",
        "host.pending(&image.token)",
        "letstep=Step::Cancelled(",
        "host.finish_audited_cancellation(image,&step,audit,now_ms)",
    ] {
        offset += audited[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    assert!(!ordinary.contains(".clone()"));
    assert_eq!(audited.matches(".clone()").count(), 1);
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let bridges = vm
        .split_once("pub fn cancel_effect(")
        .unwrap()
        .1
        .split_once("fn finish_audited_cancellation(")
        .unwrap()
        .0;
    assert!(bridges.contains("reference_cancellation::cancel_effect("));
    assert!(bridges.contains("reference_cancellation::cancel_effect_audited("));
    for absent in [
        "LSV",
        "self.pending",
        "self.journal",
        "self.expire_due",
        "self.replay_completed",
    ] {
        assert!(!bridges.contains(absent), "{absent}");
    }
    let owner = vm
        .split_once("struct ReferenceCancellationOwner")
        .unwrap()
        .1
        .split_once("struct ReferenceReplyOwner")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for hook in [
        "self.0.expire_due(now_ms)",
        "self.0.replay_completed(token)",
        "self.0.reconcile_completed(token)",
        "self.0.visible_step(token,completed)",
        "self.0.pending.get(token)",
        "self.0.finish_cancellation(image,reason,now_ms)",
        "self.0.finish_audited_cancellation(image,step,audit,now_ms)",
    ] {
        assert!(owner.contains(hook), "{hook}");
    }
    assert_interruption_settlement_boundary(&vm);
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/cancellation_contract_tests.rs"))
            .unwrap();
    assert!(
        !oracle
            .split_once("// Frozen complete cancellation")
            .unwrap()
            .1
            .contains("reference_cancellation")
    );
    for count in [2240, 96] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_cancellation"));
    assert!(ci.contains("--lib cancellation_contract_tests::cancellation_entries"));
}

#[test]
fn product_result_completion_keeps_native_authority_and_saved_fuel_boundaries() {
    let adapter = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_result_completion.rs"),
    )
    .unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        ".clone()",
        "allocate_sequence(",
        "record_completed(",
        "acknowledge_dispatch(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("ReferenceCompletionHost + ?Sized"));
    let coordination = adapter
        .split_once("pub fn complete_result<")
        .unwrap()
        .1
        .split_once("enum PreparedResult")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    let ordered = [
        "let operation = lease.map(",
        "host.atomic_step_from_effect_result(",
        "host.validate_result_output(",
        "materialize(image, &value)",
        "host.complete_successor(",
        "GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION",
        "host.step_from_effect_result(",
        "host.record_result(",
    ];
    let mut offset = 0;
    for stage in ordered {
        let stage = stage.split_whitespace().collect::<String>();
        offset += coordination[offset..]
            .find(&stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    let materialize = adapter.split_once("fn materialize(").unwrap().1;
    let ordered = [
        "image.result_binding.as_ref()",
        "Fuel::new(image.fuel_remaining)",
        "HostOperation::for_effect(&image.pending_effect)",
        "computation::resume_outcome(",
        "fuel.remaining() == 0",
        "HostOperation::for_effect(&effect)",
        "effect.into_owned()",
    ];
    let mut offset = 0;
    for stage in ordered {
        offset += materialize[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let bridge = vm
        .split_once("pub(super) fn complete_result(")
        .unwrap()
        .1
        .split_once("fn complete_successor(")
        .unwrap()
        .0;
    assert!(bridge.contains("reference_result_completion::complete_result("));
    for absent in ["schema_version", "Fuel::", "journal", "materialize", "LSV"] {
        assert!(!bridge.contains(absent));
    }
    let owner = vm
        .split_once("struct ReferenceCompletionOwner")
        .unwrap()
        .1
        .split_once("impl Vm {")
        .unwrap()
        .0;
    let compact = owner.split_whitespace().collect::<String>();
    for hook in [
        "atomic_step_from_effect_result(image,operation,result)",
        "step_from_effect_result(image,operation,result)",
        "self.0.journal.validate_result_output(image,value)",
        "self.0.complete_successor(image,lease,value,effect,binding,fuel)",
        "self.0.record_result(image,lease,step)",
    ] {
        assert!(compact.contains(hook), "{hook}");
    }
    assert_settlement_commit_before_publication(&vm);
    assert!(!vm.contains("fn materialize("));
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/completion_contract_tests.rs"))
            .unwrap();
    let frozen = oracle.split_once("// Frozen result coordinator").unwrap().1;
    assert!(!frozen.contains("reference_result_completion"));
    for count in [5600, 64] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_result_completion"));
    assert!(ci.contains("--lib completion_contract_tests::result_completion"));
}

#[test]
fn successor_assembly_is_product_owned_but_native_reservation_and_commit_stay_authoritative() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_successor.rs"))
            .unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel::new",
        "record_completed(",
        "record_successor(",
        "acknowledge_dispatch(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("ReferenceSuccessorHost + ?Sized"));
    let flow = adapter
        .split_once("pub fn complete_successor<")
        .unwrap()
        .1
        .split_whitespace()
        .collect::<String>();
    let mut offset = 0;
    for stage in [
        "host.effect_request(&image.token)",
        "request.continuation==*image",
        "host.allocate_sequence()",
        "parent.operation.authority()",
        "HostOperation::for_effect(&effect)",
        "host.build_effect_request(ReservedEffectInput",
        "next.continuation.max_output_items=image.max_output_items",
        "preflighted_result_binding_schema_version(&binding)",
        "validate_effect_request(&next)",
        "validate_continuation_size(&next.continuation)",
        "letstep=Step::Done(value)",
        "host.commit_successor(image,lease,&parent,&step,&next)",
    ] {
        offset += flow[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    assert_eq!(flow.matches("host.record_result(").count(), 3);
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let owner = vm
        .split_once("struct ReferenceSuccessorOwner")
        .unwrap()
        .1
        .split_once("impl Vm {")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    for hook in [
        "self.0.journal.effect_request(token)",
        "self.0.allocate_sequence()",
        "Vm::build_effect_request(",
        "self.0.record_result(image,lease,step)",
        "self.0.commit_successor(image,lease,parent,step,next)",
    ] {
        assert!(owner.contains(hook), "{hook}");
    }
    let native = vm.split_once("impl Vm {").unwrap().1;
    let bridge = native
        .split_once("fn complete_successor(")
        .unwrap()
        .1
        .split_once("fn commit_successor(")
        .unwrap()
        .0;
    assert!(bridge.contains("reference_successor::complete_successor("));
    for absent in [
        "LSV",
        "allocate_sequence",
        "journal",
        "schema_version",
        "build_effect_request",
    ] {
        assert!(!bridge.contains(absent), "{absent}");
    }
    assert_settlement_commit_before_publication(&vm);
    let oracle = fs::read_to_string(
        root().join("crates/leselang-vm/src/successor_assembly_contract_tests.rs"),
    )
    .unwrap();
    assert!(
        !oracle
            .split_once("fn frozen_complete_successor(")
            .unwrap()
            .1
            .contains("reference_successor")
    );
    for count in [7280, 112] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_successor"));
    assert!(ci.contains("--lib assembly_contract_tests::successor_assembly"));
}

#[test]
fn product_reply_entries_preserve_distinct_replay_order_and_native_settlement() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_reply.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel::new",
        ".clone()",
        "record_completed(",
        "acknowledge_dispatch(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("ReferenceReplyHost + ?Sized"));
    let ordinary = adapter
        .split_once("fn resume_inner<")
        .unwrap()
        .1
        .split_once("pub fn acknowledge_effect<")
        .unwrap()
        .0;
    let positions = [
        "host.replay_completed(",
        "validate_continuation_image(image)",
        "host.pending(",
        "Effect::RuntimeRefresh",
        "host.complete_result(image, None, result)",
    ]
    .map(|s| ordinary.find(s).unwrap());
    assert!(positions.windows(2).all(|p| p[0] < p[1]));
    let timed = adapter
        .split_once("pub fn resume_at<")
        .unwrap()
        .1
        .split_once("fn resume_inner<")
        .unwrap()
        .0;
    assert!(
        timed.find("validate_clock(now_ms)").unwrap()
            < timed.find("host.expire_due(now_ms)").unwrap()
    );
    let ack = adapter.split_once("pub fn acknowledge_effect<").unwrap().1;
    let positions = [
        "host.expire_due(now_ms)",
        "validate_effect_request(&lease.request)",
        "host.replay_completed(",
        "host.pending(",
        "host.complete_result(image, Some((lease, now_ms)), result)",
    ]
    .map(|s| ack.find(s).unwrap());
    assert!(positions.windows(2).all(|p| p[0] < p[1]));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for (from, to) in [
        ("pub fn resume(", "pub fn claim_effect("),
        ("pub fn acknowledge_effect(", "pub fn report_effect_error("),
    ] {
        let bridge = vm.split_once(from).unwrap().1.split_once(to).unwrap().0;
        for absent in [
            "LSV",
            "self.replay_completed",
            "self.expire_due",
            "self.pending",
            "self.complete_result",
            ".clone()",
        ] {
            assert!(!bridge.contains(absent), "{absent}");
        }
        assert!(bridge.contains("&mut ReferenceReplyOwner(self)"));
    }
    let owner = vm
        .split_once("struct ReferenceReplyOwner")
        .unwrap()
        .1
        .split_once("struct ReferenceStartOwner")
        .unwrap()
        .0;
    for hook in [
        "self.0.expire_due(now_ms)",
        "self.0.replay_completed(token)",
        "self.0.visible_step(token, completed)",
        "self.0.pending.get(token)",
        "self.0.complete_result(image, lease, result)",
    ] {
        assert!(owner.contains(hook));
    }
    let successor = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    assert!(
        successor.contains("pub(super) use leselang_command::reference_reply::CompletionLease;")
    );
    assert_settlement_commit_before_publication(&successor);
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/reply_contract_tests.rs")).unwrap();
    assert!(
        !oracle
            .split_once("fn frozen_resume(")
            .unwrap()
            .1
            .contains("reference_reply")
    );
    for count in [3360, 64] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_reply"));
    assert!(ci.contains("--lib reply_contract_tests::reply_entries"));
}

#[test]
fn product_restore_keeps_preflight_duplicates_and_native_commit_before_publication() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_restore.rs"))
            .unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "std::fs",
        "std::time",
        "Fuel::new",
        "record_pending(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let full = adapter.split_once("pub fn restore_request<").unwrap().1;
    assert!(
        full.find("validate_effect_request(&request)?").unwrap()
            < full.find("request.continuation.clone()").unwrap()
    );
    let ordered = [
        "validate_continuation_size(&image)?",
        "image.group_result.is_some()",
        "request.is_none()",
        "host.pending(&image.token)",
        "host.commit_existing(&image, request)",
        "host.is_completed(&image.token)",
        "canonical_continuation_sequence(&image.token)",
        "SequenceWatermarkRequirement::after_observation",
        "host.commit_new(image, request, next_sequence)",
    ]
    .map(|s| adapter.find(s).unwrap());
    assert!(ordered.windows(2).all(|w| w[0] < w[1]));
    assert!(adapter.contains("ReferenceRestoreHost + ?Sized"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let bridge = vm
        .split_once("pub fn restore(")
        .unwrap()
        .1
        .split_once("pub fn resume(")
        .unwrap()
        .0;
    for absent in ["LSV", "validate_", ".insert(", ".max(", ".clone()"] {
        assert!(!bridge.contains(absent), "{absent}");
    }
    assert_eq!(
        bridge
            .matches("reference_restore::ReferenceRestoreCache")
            .count(),
        2
    );
    assert!(bridge.contains(".restore(image,"));
    assert!(bridge.contains(".restore_request(request,"));
    assert_eq!(
        bridge
            .matches(".record_pending(image, request, self.scheduler_limits)")
            .count(),
        2
    );
    assert!(!vm.contains("struct ReferenceRestoreOwner"));
    let owner = adapter
        .split_once("impl<Commit> ReferenceRestoreHost for CacheImport")
        .unwrap()
        .1
        .split_once("/// Trusted owner")
        .unwrap()
        .0;
    let fresh = owner.split_once("fn commit_new(").unwrap().1;
    let compact: String = fresh.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.find("(self.commit)(&image,request)?").unwrap()
            < compact.find("*self.cache.next_sequence=").unwrap()
    );
    assert!(
        compact.find("*self.cache.next_sequence=").unwrap()
            < compact.find("self.cache.pending.insert(").unwrap()
    );
    let existing = owner
        .split_once("fn commit_existing(")
        .unwrap()
        .1
        .split_once("fn commit_new(")
        .unwrap()
        .0;
    assert!(existing.contains("(self.commit)(image, request)"));
    assert!(!existing.contains("next_sequence"));
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/restore_contract_tests.rs"))
            .unwrap();
    assert!(
        !oracle
            .split_once("fn frozen_restore(")
            .unwrap()
            .1
            .contains("reference_restore")
    );
    for count in [2240, 66, 24] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_restore"));
    assert!(ci.contains("--lib restore_contract_tests::"));
}

#[test]
fn product_root_start_keeps_preflight_order_and_original_native_publication_hooks() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_start.rs")).unwrap();
    let positions = [
        "if fuel_limit == 0",
        "authorize(program, &capabilities)",
        "let mut fuel = Fuel::new(fuel_limit)",
        "crate::reference_computation::evaluate(expression, &mut fuel)",
        "validate_binding(binding, effect)",
        "if fuel_remaining == 0",
        "canonical_source(effect).is_err()",
        "return host.start_group(",
        "host.check_admission(1)",
        "host.allocate_sequence()",
        "match build_effect_request(",
        "host.commit_atomic(request)",
    ]
    .map(|gate| adapter.find(gate).unwrap());
    assert!(positions.windows(2).all(|p| p[0] < p[1]));
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "record_pending(",
        "self.pending",
        "std::fs",
        "std::time",
        "binding.clone()",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    assert!(adapter.contains("ReferenceStartHost: ReferenceGroupStartHost"));
    assert!(adapter.contains("ReferenceStartHost + ?Sized"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let bridge = vm
        .split_once("fn start_inner(")
        .unwrap()
        .1
        .split_once("fn start_group(")
        .unwrap()
        .0;
    for forbidden in [
        "Fuel::new",
        "authorize(",
        "canonical_source(",
        "computation::evaluate",
        "LSV",
        ".clone()",
    ] {
        assert!(!bridge.contains(forbidden), "{forbidden}");
    }
    assert!(bridge.contains("leselang_command::reference_start::start("));
    assert!(bridge.contains("fuel_remaining: self.fuel_limit"));
    let owner = vm
        .split_once("struct ReferenceStartOwner")
        .unwrap()
        .1
        .split_once("fn encode_json_capped")
        .unwrap()
        .0;
    assert!(owner.contains("reference_publication::publish_atomic(request, self)"));
    assert!(owner.contains("self.0.journal.record_pending("));
    assert!(owner.contains("self.0.pending.insert(image.token.clone(), image.clone())"));
    let publication =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_publication.rs"))
            .unwrap();
    let atomic = publication
        .split_once("pub fn publish_atomic<")
        .unwrap()
        .1
        .split_once("pub(crate) fn publish_group<")
        .unwrap()
        .0;
    let commit = atomic.find("host.commit_atomic(&request)").unwrap();
    let pending = atomic
        .find("host.publish_pending(&request.continuation)")
        .unwrap();
    let visible = atomic.find("Step::Effect(Box::new(request))").unwrap();
    assert!(commit < pending && pending < visible);
    assert!(owner.contains("self.0.start_group("));
    let timed = vm
        .split_once("pub fn start_timed(")
        .unwrap()
        .1
        .split_once("fn start_inner(")
        .unwrap()
        .0;
    assert!(
        timed.find("validate_effect_clock(").unwrap() < timed.find("self.expire_due(").unwrap()
    );
    assert!(timed.find("self.expire_due(").unwrap() < timed.find("self.start_inner(").unwrap());
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/start_contract_tests.rs")).unwrap();
    let frozen = oracle.split_once("fn frozen_start_inner(").unwrap().1;
    assert!(!frozen.contains("reference_start"));
    assert!(frozen.contains("authorize(program, &capabilities)"));
    let compact: String = frozen.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("vm.journal.record_pending("));
    for count in [8400, 320, 24] {
        assert!(oracle.contains(&format!("assert_eq!(comparisons, {count})")));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_start"));
    assert!(ci.contains("--lib start_contract_tests::root_start"));
}

#[test]
fn product_group_start_retains_native_reservations_and_commit_before_publication() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_start.rs"))
            .unwrap();
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "record_merge_graph_with_limits(",
        "self.pending",
        "std::fs",
        "std::time",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let preflight = adapter.find("validate_merge_plan(&plan)?").unwrap();
    let capacity = adapter
        .find("host.check_admission(branches.len())?")
        .unwrap();
    let group = adapter
        .find("let group_sequence = host.allocate_sequence()?")
        .unwrap();
    let prefix = adapter
        .find("for (index, branch) in branches.iter().enumerate()")
        .unwrap();
    let successors = adapter.find("if order.has_group_successor()").unwrap();
    assert!(preflight < capacity && capacity < group && group < prefix && prefix < successors);
    assert!(adapter.contains("fuel.charge(index as u64)"));
    assert!(adapter.contains("body_fuel.charge(branches.len() as u64)"));
    assert!(adapter.contains("ReferenceGroupStartHost + ?Sized"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let start = vm
        .split_once("fn start_group(")
        .unwrap()
        .1
        .split_once("fn build_effect_request(")
        .unwrap()
        .0;
    for removed in [
        "Fuel::new",
        "GroupResultBinding {",
        "validate_merge_plan(",
        "GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION",
        "NamedEffectRequest {",
    ] {
        assert!(!start.contains(removed), "{removed}");
    }
    let owner = vm
        .split_once("struct ReferenceStartOwner")
        .unwrap()
        .1
        .split_once("fn encode_json_capped")
        .unwrap()
        .0;
    assert!(start.contains("&mut ReferenceStartOwner(self)"));
    assert!(owner.contains("self.0.allocate_sequence()"));
    assert!(owner.contains(".check_admission(additional, self.0.scheduler_limits)"));
    assert!(start.contains("leselang_command::reference_group_start::start_group("));
    for removed in ["self.journal", "self.pending", "Step::", "named.remove("] {
        assert!(!start.contains(removed), "{removed}");
    }
    assert!(
        owner
            .split_whitespace()
            .collect::<String>()
            .contains("self.0.journal.record_merge_graph_with_limits(")
    );
    let prepare = adapter.find("match prepare_group_start(").unwrap();
    let publish = adapter
        .find("crate::reference_publication::publish_group(prepared, host)")
        .unwrap();
    assert!(prepare < publish);
    let publication =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_publication.rs"))
            .unwrap();
    let group = publication
        .split_once("pub(crate) fn publish_group<")
        .unwrap()
        .1;
    let commit = group
        .find("host.commit_group(&merge_token, &plan, &graph)")
        .unwrap();
    let pending = group
        .find("host.publish_pending(&branch.request.continuation)")
        .unwrap();
    let visible = group
        .find("Step::Effect(Box::new(named.remove(0).request))")
        .unwrap();
    assert!(commit < pending && pending < visible);
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_start_contract_tests.rs"))
            .unwrap();
    let frozen = oracle.split_once("fn frozen_start_group(").unwrap().1;
    assert!(!frozen.contains("reference_group_start"));
    assert!(frozen.contains("let mut body_fuel = Fuel::new(fuel_remaining)"));
    assert!(oracle.contains("assert_eq!(comparisons, 15120)"));
    assert!(oracle.contains("assert_eq!(comparisons, 36)"));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_group_start"));
    assert!(ci.contains("--lib group_start"));
}

#[test]
fn complete_product_computation_policy_is_adapter_owned_over_the_original_evaluator() {
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    let bridge = vm.split_once("#[cfg(test)]").unwrap().0;
    assert!(bridge.contains("pub(super) use leselang_command::reference_computation::{"));
    for entry in ["resume_group", "resume_outcome", "resume_group_outcome"] {
        assert!(bridge.contains(entry));
    }
    assert!(
        vm.contains("pub(super) use leselang_command::reference_computation::{evaluate, resume};")
    );
    let result =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_result.rs")).unwrap();
    assert!(
        result
            .contains("crate::reference_computation::resume(binding, value, operation, &mut fuel)")
    );
    for forbidden in ["fn ", "Fuel", "LSV", "ScopeFrame", "ReferenceCaptureHost"] {
        assert!(!bridge.contains(forbidden), "{forbidden}");
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let production = adapter.split_once("#[cfg(test)]").unwrap().0;
    for retained in [
        "evaluate_effects_in_scope(",
        "crate::resume_preflighted_result_binding_in_scope(",
        "crate::PreflightedResultEnvironment::new(",
        "crate::reference_request::result_binding_limits().groups",
        "crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)",
        "CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error)",
        "PureEvaluationFault::Native(fault) => fault",
        "if self.restore_saved_results",
    ] {
        assert!(production.contains(retained), "{retained}");
    }
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "Fuel::new",
        "apply_binary(",
        "apply_unary(",
        "allocate_sequence(",
        ".commit(",
    ] {
        assert!(!production.contains(forbidden), "{forbidden}");
    }
    let oracle = fs::read_to_string(
        root().join("crates/leselang-vm/src/computation_contract_tests/frozen.rs"),
    )
    .unwrap();
    assert!(!oracle.contains("reference_computation"));
    assert!(oracle.contains("evaluate_effects_in_scope("));
    assert!(oracle.contains("impl leselang_command::ResultCaptureHost for ReferenceCaptureHost"));
    let matrix =
        fs::read_to_string(root().join("crates/leselang-vm/src/computation_contract_tests.rs"))
            .unwrap();
    assert!(matrix.contains("assert_eq!(comparisons, 11640)"));
    assert!(matrix.contains("assert_eq!(comparisons, 2600)"));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--lib --test reference_computation"));
    let external =
        fs::read_to_string(root().join("crates/leselang-command/tests/reference_computation.rs"))
            .unwrap();
    assert!(!external.contains("leselang_vm"));
    assert!(external.contains("std::ptr::eq(&*actual, &**effect)"));
    assert!(external.contains("assert_eq!(fuel.remaining(), 99)"));
}

#[test]
fn group_reentry_coordination_uses_original_execution_and_authority_hooks() {
    let group = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let bridge = group.split_once("struct GroupExecutionHost;").unwrap().1;
    for entry in ["prepare_initial", "advance_capture", "finish"] {
        assert!(group.contains(&format!("reference_group_execution::{entry}(")));
    }
    for original in [
        "crate::computation::resume_group_outcome(binding, value, branches, fuel)",
        "crate::computation::resume_outcome(binding, value, operation, fuel)",
        "crate::computation::resume_group(binding, value, branches, fuel)",
        "group.build_successor(",
        "crate::successor::validate_pair(parent, result, next)",
    ] {
        assert!(bridge.contains(original), "{original}");
    }
    assert!(
        !group
            .split_once("#[cfg(all(test,")
            .unwrap()
            .0
            .contains("Fuel::new(")
    );
    assert!(
        group.contains(
            "pub(super) use leselang_command::reference_group_execution::CaptureProgress;"
        )
    );
    let computation =
        fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    assert!(computation.contains(
        "pub(super) use leselang_command::execution_outcome::ComputationOutcome as Outcome;"
    ));
    assert!(!computation.contains("enum Outcome"));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_execution.rs"))
            .unwrap();
    assert!(adapter.contains("Host: ReferenceGroupExecutionHost + ?Sized"));
    assert!(adapter.contains("Fuel::new(group.fuel_remaining)"));
    assert!(adapter.contains("Fuel::new(image.fuel_remaining)"));
    let initial = adapter
        .split_once("pub fn prepare_initial")
        .unwrap()
        .1
        .split_once("pub fn advance_capture")
        .unwrap()
        .0;
    let capture = adapter
        .split_once("pub fn advance_capture")
        .unwrap()
        .1
        .split_once("pub fn finish")
        .unwrap()
        .0;
    for (body, stages) in [
        (
            initial,
            vec![
                "host.resume_group(",
                "Outcome::Scalar",
                "_ => return Err(invalid())",
                "require_successor_fuel(&fuel)?",
                "host.build_successor(",
            ],
        ),
        (
            capture,
            vec![
                "validate_saved_capture(group, request, prefix, &operations)?",
                "host.resume_capture(",
                "Outcome::Scalar",
                "require_successor_fuel(&fuel)?",
                "host.build_successor(",
                "host.validate_successor(",
                "Ok(CaptureProgress::Pending",
            ],
        ),
    ] {
        let mut offset = 0;
        for stage in stages {
            offset += body[offset..]
                .find(stage)
                .unwrap_or_else(|| panic!("{stage}"))
                + stage.len();
        }
    }
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "Vm::",
        ".commit(",
        "allocate_sequence(",
        "eval_computation(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let native =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_execution_contract_tests.rs"))
            .unwrap();
    assert!(native.contains("assert_eq!(comparisons, 1944)"));
    assert!(native.contains("assert_eq!(comparisons, 2916)"));
    let frozen = native.split_once("impl FrozenGroup<'_>").unwrap().1;
    assert!(frozen.contains("crate::computation::resume_group_outcome("));
    assert!(frozen.contains("crate::computation::resume_outcome("));
    assert!(!frozen.contains("reference_group_execution"));
    let direct = fs::read_to_string(
        root().join("crates/leselang-command/tests/reference_group_execution.rs"),
    )
    .unwrap();
    assert!(direct.contains("assert_eq!(comparisons, 5760)"));
    for case in [
        "group_outcome_classification_precedes_zero_remaining_fuel_and_never_refills",
        "group_coordination_keeps_borrowed_effects_and_moves_original_native_capture",
        "group_coordinator_never_retries_or_hands_off_after_any_native_hook_unwind",
    ] {
        assert!(direct.contains(case), "{case}");
    }
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_group_execution")
    );
}

#[test]
fn group_successor_assembly_is_adapter_owned_with_original_builder_and_live_gates() {
    let group = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let build = group
        .split_once("impl ReferenceGroupResultBinding for GroupResultBinding")
        .unwrap()
        .1
        .split_once("fn build_successor(")
        .unwrap()
        .1
        .split_once("fn advance_capture(")
        .unwrap()
        .0;
    for required in [
        "reference_group_successor::build_group_successor(",
        "GroupSuccessorContext",
        "crate::Vm::build_effect_request(",
        "input.effect",
        "input.principal",
        "input.capabilities",
        "input.expected_revision",
        "input.deadline_ms",
        "input.deadline_at_ms",
        "input.fuel_remaining",
    ] {
        assert!(build.contains(required), "{required}");
    }
    for moved in [
        "HostOperation::for_effect",
        "reserved_sequences()",
        "continuation.schema_version =",
        "continuation.group_result =",
        "validate_effect_request(",
        "validate_continuation_size(",
    ] {
        assert!(!build.contains(moved), "{moved}");
    }
    assert!(group.contains("crate::computation::resume_group_outcome("));
    assert!(group.contains("crate::computation::resume_outcome("));
    assert!(group.contains("crate::successor::validate_pair("));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_successor.rs"))
            .unwrap();
    let mut offset = 0;
    for stage in [
        "HostOperation::for_effect(effect)",
        "context.first.operation.authority()",
        "let mut request = build(",
        ".nth(context.position)",
        "principal: principal.clone()",
        "fuel_remaining: context.fuel_remaining",
        "request.continuation.schema_version =",
        "request.continuation.result_binding = binding",
        "request.budget.max_output_items =",
        "validate_effect_request(&request)?",
        "validate_continuation_size(&request.continuation)?",
    ] {
        offset += adapter[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("{stage}"))
            + stage.len();
    }
    assert!(adapter.contains("build: impl FnOnce(ReservedEffectInput<'_>)"));
    for forbidden in [
        "leselang_vm",
        "Fuel::",
        "allocate_sequence(",
        "rusqlite",
        "resume_outcome(",
        ".commit(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let native =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_successor_contract_tests.rs"))
            .unwrap();
    assert!(native.contains("assert_eq!(comparisons, 8960)"));
    assert!(native.contains("assert_eq!(successes, 560)"));
    let frozen = native
        .split_once("fn frozen_build(")
        .unwrap()
        .1
        .split_once("#[test]")
        .unwrap()
        .0;
    assert!(frozen.contains("Vm::build_effect_request("));
    assert!(!frozen.contains("reference_group_successor"));
    let direct = fs::read_to_string(
        root().join("crates/leselang-command/tests/reference_group_successor.rs"),
    )
    .unwrap();
    assert!(direct.contains("assert_eq!(comparisons, 384)"));
    assert!(direct.contains("actual_trace, expected_trace"));
    assert!(direct.contains("saved_pointer"));
    assert!(
        direct.contains(
            "group_successor_preflight_releases_uncalled_builder_and_unwind_never_retries"
        )
    );
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_group_successor")
    );
}

#[test]
fn checkpoint_product_coordination_uses_one_native_transaction_owner() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_checkpoint.rs"))
            .unwrap();
    assert!(
        journal
            .contains("pub use leselang_command::reference_checkpoint::MAX_JOURNAL_TOTAL_BYTES;")
    );
    assert!(!journal.contains("fn checkpoint_fault("));
    assert!(!journal.contains("CheckpointRecovery::new("));
    let storage = journal
        .split_once("impl ReferenceCheckpointSource for SqliteRecoveryFinalizer")
        .unwrap()
        .1
        .split_once("impl leselang_runtime_core::CheckpointFinalizer for SqliteRecoveryFinalizer")
        .unwrap()
        .0;
    for boundary in [
        "journal_record_count(self.connection()?)",
        "journal_payload_bytes(self.connection()?)",
        "let connection = self.connection()?",
        "FROM vm_effects ORDER BY token ASC",
        "push(recover_effect_record(connection, row)?)?",
    ] {
        assert!(storage.contains(boundary), "{boundary}");
    }
    for forbidden in [
        "collect(",
        "CheckpointRecovery",
        "transaction_with_behavior",
        ".commit(",
    ] {
        assert!(!storage.contains(forbidden), "{forbidden}");
    }
    let mut offset = 0;
    for phase in [
        "source.record_count()?",
        "count > MAX_JOURNAL_RECORDS",
        "source.payload_bytes()?",
        "total_bytes > MAX_JOURNAL_TOTAL_BYTES",
        "CheckpointRecovery::new(MAX_JOURNAL_RECORDS, MAX_EFFECT_SEQUENCE)",
        "source.stage_records(",
        ".finish_recovery(source)",
    ] {
        offset += adapter[offset..]
            .find(phase)
            .unwrap_or_else(|| panic!("{phase}"))
            + phase.len();
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "Fuel::",
        "Vm::",
        ".execute(",
        ".commit(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    assert!(adapter.contains("CheckpointFinalizer<Commit = u64, Error = Fault>"));
    let direct =
        fs::read_to_string(root().join("crates/leselang-command/tests/reference_checkpoint.rs"))
            .unwrap();
    assert!(direct.contains("assert_eq!(comparisons, 1560)"));
    assert!(direct.contains("actual_trace, expected_trace"));
    let frozen = direct
        .split_once("fn frozen(")
        .unwrap()
        .1
        .split_once("fn frozen_fault(")
        .unwrap()
        .0;
    assert!(!frozen.contains("recover_checkpoint("));
    assert!(!frozen.contains("finish_recovery("));
    assert!(frozen.contains(".finish(source,"));
    for test in [
        "checkpoint_uses_original_owned_payloads_and_cleans_up_before_publication",
        "checkpoint_rechecks_stream_limit_and_poisoned_staging_never_finalizes",
        "checkpoint_unwind_at_every_owner_boundary_cannot_publish_or_retry",
    ] {
        assert!(direct.contains(test), "{test}");
    }
}

#[test]
fn effect_record_policy_is_adapter_owned_without_checkpoint_publication_authority() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_recovery.rs"))
            .unwrap();
    assert!(journal.contains("use leselang_command::reference_recovery::{"));
    assert!(
        journal.contains("pub use leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES;")
    );
    assert!(journal.contains("decode_journal_value as decode_bounded"));
    assert!(!journal.contains("fn decode_bounded<"));
    assert!(!journal.contains("RecordRecoveryHost for SqliteRecoveryRecord"));
    let storage = journal
        .split_once("fn recover_effect_record(")
        .unwrap()
        .1
        .split("fn completed_effect_logical_bytes(")
        .next()
        .unwrap();
    assert!(storage.find(".get(0)").unwrap() < storage.find(".get(1)").unwrap());
    assert!(storage.find(".get(1)").unwrap() < storage.find("inspect_effect_record(").unwrap());
    for required in [
        "impl ReferenceRecordSource for SqliteRecoveryRecord",
        "group_binding::validate_owner(self.connection, image)",
        "successor::validate_record(self.connection, image, completed)",
    ] {
        assert!(storage.contains(required), "{required}");
    }
    for moved in [
        "fn sequence(",
        "fn stored_state(",
        "fn pending_payload(",
        "fn completed_payload(",
        "fn admit_completed(",
    ] {
        assert!(!storage.contains(moved), "{moved}");
        assert!(adapter.contains(moved), "{moved}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "Journal::",
        "CheckpointRecovery::new",
        "std::fs",
        "Fuel::",
        ".execute(",
        ".commit(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    assert_eq!(
        adapter
            .matches("inspect_record_recovery(&mut RecordPolicy")
            .count(),
        1
    );
    assert!(adapter.contains("type Bytes: AsRef<[u8]>"));
    assert!(adapter.contains("Source: ReferenceRecordSource + ?Sized"));
    let direct =
        fs::read_to_string(root().join("crates/leselang-command/tests/reference_recovery.rs"))
            .unwrap();
    assert!(direct.contains("assert_eq!(cases, 3456)"));
    assert!(direct.contains("actual.trace, expected.trace"));
    let frozen = direct
        .split_once("fn frozen(")
        .unwrap()
        .1
        .split("fn comparable(")
        .next()
        .unwrap();
    for forbidden in [
        "inspect_record_recovery(",
        "inspect_effect_record(",
        "decode_journal_value(",
    ] {
        assert!(!frozen.contains(forbidden));
    }
    let integration =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(
        integration.contains("inspected_record_is_not_authority_to_replace_a_live_continuation")
    );
    let sql = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/record_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(sql.contains("assert_eq!(cases, 64)"));
    assert!(sql.contains("assert_eq!(comparisons, 784)"));
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_recovery")
    );
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Product Effect Record Recovery Ownership")
    );
}

#[test]
fn static_dispatch_policy_is_adapter_owned_while_observations_and_commit_stay_local() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_dispatch.rs"))
            .unwrap();
    assert!(journal.contains("use leselang_command::reference_dispatch::{"));
    assert!(
        journal
            .contains("pub(super) use leselang_command::reference_dispatch::validate_lease_clock;")
    );
    for name in [
        "validate_proposed_retry",
        "validate_observed_lease",
        "validate_lease_clock",
        "validate_dispatch_clock",
        "lease_expiration",
        "next_attempt",
    ] {
        assert!(!journal.contains(&format!("fn {name}(")), "{name}");
        assert!(adapter.contains(&format!("pub fn {name}(")), "{name}");
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(vm.contains("pub use leselang_command::reference_dispatch::{"));
    for constant in [
        "DEFAULT_DISPATCH_LEASE_MS",
        "MAX_DISPATCH_LEASE_MS",
        "MAX_DISPATCH_ATTEMPTS",
    ] {
        assert!(!vm.contains(&format!("pub const {constant}:")));
        assert!(adapter.contains(&format!("pub const {constant}:")));
    }
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "serde_json",
        "Journal::",
        "Fuel::",
        ".clone()",
        "SystemTime",
        "std::fs",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    for retained in [
        "fn validate_ephemeral_lease(",
        "fn validate_stored_lease(",
        "same_request: dispatch.request == lease.request",
        "active: !dispatch.acknowledged",
        "active: dispatch.state == \"leased\"",
        "TransactionBehavior::Immediate",
        "pub use leselang_command::reference_journal_schema::JOURNAL_SCHEMA_VERSION;",
    ] {
        assert!(journal.contains(retained), "{retained}");
    }
    let retry =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_retry_commit.rs"))
            .unwrap();
    assert!(retry.contains("validate_proposed_retry(dispatch.retry_count, schedule, now_ms)?"));
    assert!(retry.contains("validate_proposed_retry(*state.retry_count, schedule, now_ms)?"));
    assert!(retry.contains("same_request: request == lease.request"));
    let acknowledgement = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_acknowledgement_commit.rs"),
    )
    .unwrap();
    assert!(acknowledgement.contains("same_request: request == lease.request"));
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/reference_dispatch.rs"))
            .unwrap();
    let frozen = tests
        .split_once("mod frozen {")
        .unwrap()
        .1
        .split("#[test]")
        .next()
        .unwrap();
    for absent in [
        "reference_dispatch::",
        "validate_dispatch_lease(",
        "validate_retry_schedule(",
        "validate_effect_error(",
        "plan_dispatch_lease(",
    ] {
        assert!(!frozen.contains(absent), "{absent}");
    }
    for cases in [
        "assert_eq!(count, 49)",
        "assert_eq!(count, 768)",
        "assert_eq!(count, 9600)",
    ] {
        assert!(tests.contains(cases));
    }
    let integration =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(integration.contains(
        "memory_dispatch_adapter_proposals_cannot_replace_current_leases_or_commit_retries"
    ));
    let durable =
        fs::read_to_string(root().join("crates/leselang-vm/tests/dispatch_scheduling.rs")).unwrap();
    assert!(durable.contains("durable_dispatch_adapter_proposals_remain_fenced_across_restarts"));
    for source in [&integration, &durable] {
        assert!(source.contains("mod reference_dispatch_authority;"));
        assert!(source.contains("reference_dispatch_authority::exercise("));
    }
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_dispatch")
    );
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Static Dispatch Policy Ownership")
    );
}

#[test]
fn journal_completion_policy_is_adapter_owned_without_durable_authority() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_completion.rs"))
            .unwrap();
    assert!(journal.contains("use leselang_command::reference_completion::{"));
    for name in [
        "validate_terminal_step",
        "validate_cancellation_step",
        "deadline_cancellation",
        "terminal_step_outcome",
        "sequence_stop",
    ] {
        assert!(!journal.contains(&format!("fn {name}(")));
        assert!(!journal.contains(&format!("fn {name}<'a>(")));
        assert!(adapter.contains(&format!("pub fn {name}")));
    }
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "Journal::",
        "Fuel::new",
        "serde_json",
        "validate_branch_outcome(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    for required in [
        "validate_value(value, 1)",
        "validate_terminal_step(step)?",
        "cancellation.continuation == image.token",
        "OutcomeOrder::Sequential",
        "LSV4007",
        "LSV2404",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let oracle = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/reference_completion_contract_tests.rs"),
    )
    .unwrap();
    assert!(oracle.contains("assert_eq!(cases.len(), 379)"));
    assert!(oracle.contains("assert_eq!(count, 729)"));
    let frozen = oracle.split("#[test]").next().unwrap();
    assert!(!frozen.contains("reference_completion::"));
    assert!(frozen.contains("legacy_validate_value"));
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs"))
            .unwrap()
            .contains("adapter_completion_policy_cannot_settle_or_cancel_pending_execution")
    );
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_completion")
    );
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Static Journal Completion Ownership")
    );
}

#[test]
fn reference_merge_policy_is_adapter_owned_without_group_execution_authority() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for removed in [
        "fn validate_merge_plan(",
        "fn merge_plan_admission_limits(",
        "fn merge_plan_admission_fault(",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    assert!(vm.contains("pub(crate) use leselang_command::reference_merge::validate_merge_plan;"));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_merge.rs")).unwrap();
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "Journal::",
        "Fuel::new",
        ".clone()",
        "resume_group",
        "build_successor",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    for required in [
        "crate::validate_merge_plan(",
        "crate::validate_group_binding(",
        "bounded_json::measure(plan, MAX_CONTINUATION_BYTES)",
        "GroupBindingError::Encoding(fault) => fault",
        "merge_plan_admission_fault",
        "group_binding_admission_limits",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let group = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    for retained in [
        "crate::computation::resume_group_outcome(",
        "fn build_successor(",
        "fn finish(",
        "crate::computation::resume_group(",
    ] {
        assert!(group.contains(retained), "{retained}");
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/src/merge_plan_contract_tests.rs"))
            .unwrap();
    assert!(proof.contains(
        "static_merge_admission_neither_reserves_ids_nor_restores_or_finishes_live_groups"
    ));
    let oracle = proof
        .split_once("fn frozen_validate_merge_plan(")
        .unwrap()
        .1
        .split_once("fn frozen_successor_fault(")
        .unwrap()
        .0;
    assert!(oracle.contains("frozen_validate_group(binding, plan)?"));
    assert!(!oracle.contains("binding.validate(plan)"));
    assert!(!oracle.contains("reference_merge::"));
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_merge")
    );
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Static Merge Plan Ownership")
    );
}

#[test]
fn reference_value_policy_is_adapter_owned_without_execution_or_journal_authority() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for removed in [
        "pub const MAX_STRUCTURED_VALUE_DEPTH:",
        "fn validate_value(",
        "fn value_validation_limits(",
        "fn value_bounds_fault(",
        "fn validate_branch_outcome(",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    assert!(vm.contains("pub use leselang_command::reference_value::MAX_STRUCTURED_VALUE_DEPTH;"));
    assert!(vm.contains("pub(crate) use leselang_command::reference_value::{"));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_value.rs")).unwrap();
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "Journal::",
        "Fuel::new",
        "serde_json",
        ".clone()",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    for required in [
        "crate::validate_value_items(",
        "validate_value(value, 2)",
        "valid_continuation_token",
        "validate_effect_error",
        "value_bounds_fault",
        "LSV2403",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("use leselang_command::reference_completion::{"));
    let completion =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_completion.rs"))
            .unwrap();
    assert!(completion.contains("pub fn validate_terminal_step(step: &Step)"));
    assert!(completion.contains("completed journal output exceeds runtime limit"));
    assert!(!journal.contains("reference_value::validate_branch_outcome"));
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(
        proof.contains("adapter_value_admission_does_not_cancel_or_override_pending_execution")
    );
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_value"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Static Value And Branch Outcome Ownership"));
}

#[test]
fn continuation_codec_is_adapter_owned_while_restore_and_consumers_keep_execution_boundaries() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let codec =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_continuation.rs"))
            .unwrap();
    for removed in [
        "pub fn encode_continuation(",
        "pub fn decode_continuation(",
        "fn validate_pending_effect_contract(",
        "pub const MAX_CONTINUATION_BYTES:",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    assert!(vm.contains("pub use leselang_command::{MAX_CONTINUATION_BYTES, decode_continuation, encode_continuation}"));
    assert!(vm.contains("pub(crate) use leselang_command::reference_continuation::validate_continuation_encoding_size;"));
    assert!(vm.contains("#[cfg(test)]\npub(crate) use leselang_command::reference_continuation::validate_continuation_size;"));
    for absent in [
        "leselang_vm",
        "rusqlite",
        "Journal::",
        "Fuel::new",
        ".record_pending(",
        "disable_recursion_limit",
    ] {
        assert!(!codec.contains(absent), "{absent}");
    }
    let encode = codec
        .split_once("pub fn encode_continuation(")
        .unwrap()
        .1
        .split_once("pub fn validate_continuation_size(")
        .unwrap()
        .0;
    assert!(
        encode
            .find("validate_continuation_encoding_size(image)?")
            .unwrap()
            < encode.find("validate_restorable_image(image)?").unwrap()
    );
    assert!(
        encode.find("validate_restorable_image(image)?").unwrap()
            < encode.find("bounded_json::encode(").unwrap()
    );
    let preflight = codec
        .split_once("pub fn validate_continuation_encoding_size(")
        .unwrap()
        .1
        .split_once("pub fn decode_continuation(")
        .unwrap()
        .0;
    assert!(
        preflight
            .find("crate::validate_result_binding_structure(")
            .unwrap()
            < preflight.find("bounded_json::measure(").unwrap()
    );
    let restore =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_restore.rs"))
            .unwrap();
    let mut rest = restore.as_str();
    for gate in [
        "validate_continuation_size(&image)?",
        "if image.group_result.is_some()",
        "if let Some(current) = host.pending(&image.token)",
        "if host.is_completed(&image.token)",
        "SequenceWatermarkRequirement::after_observation",
        "host.commit_new(image, request, next_sequence)",
    ] {
        rest = rest
            .split_once(gate)
            .unwrap_or_else(|| panic!("lost restore gate {gate}"))
            .1;
    }
    let owner = restore
        .split_once("impl<Commit> ReferenceRestoreHost for CacheImport")
        .unwrap()
        .1
        .split_once("/// Trusted owner")
        .unwrap()
        .0;
    let mut rest = owner.split_once("fn commit_new(").unwrap().1;
    for gate in [
        "(self.commit)(&image, request)?",
        "*self.cache.next_sequence =",
        "self.cache.pending.insert(",
    ] {
        rest = rest.split_once(gate).unwrap().1;
    }
    let observe =
        fs::read_to_string(root().join("crates/leselang-observe/src/vm_bridge.rs")).unwrap();
    assert!(observe.contains("use leselang_vm::Vm;"));
    assert!(!observe.contains("leselang_vm::{"));
    let daemon = fs::read_to_string(root().join("crates/leserpentd/src/debugger.rs")).unwrap();
    assert!(daemon.contains("use leselang_vm::{DEFAULT_FUEL, Vm};"));
    assert!(daemon.contains("use leselang_command::{"));
    for path in ["debugger.rs", "remote.rs", "wire.rs"] {
        let source = fs::read_to_string(root().join("crates/leserpentd/src").join(path)).unwrap();
        for removed in [
            "leselang_vm::Value",
            "leselang_vm::ScalarValue",
            "leselang_vm::PresentationOperation",
            "leselang_vm::MAX_MERGE_BRANCHES",
        ] {
            assert!(!source.contains(removed), "{path}: {removed}");
        }
    }
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/continuation_codec_contract_tests.rs"),
    )
    .unwrap();
    let oracle = frozen.split_once("fn compare(").unwrap().0;
    assert!(!oracle.contains("leselang_command"));
    assert!(oracle.contains("continuation_schema_tests::frozen_validate_image(image)?"));
    assert!(oracle.contains("frozen_encode_json_capped"));
    assert!(frozen.contains("7_560"));
    assert!(frozen.contains("2_652"));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("--test reference_continuation"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Static Continuation Codec Ownership"));
}

#[test]
fn observation_projection_features_really_remove_vm_and_storage_dependencies() {
    for (features, vm, sqlite) in [
        (vec!["--no-default-features"], false, false),
        (
            vec!["--no-default-features", "--features", "vm-bridge"],
            true,
            false,
        ),
        (vec![], true, true),
        (
            vec!["--no-default-features", "--features", "sqlite-journal"],
            true,
            true,
        ),
    ] {
        let output = Command::new(env!("CARGO"))
            .args([
                "tree",
                "-p",
                "leselang-observe",
                "--locked",
                "--offline",
                "--edges",
                "normal,build,dev",
                "--prefix",
                "none",
            ])
            .args(features)
            .current_dir(root())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let tree = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            tree.lines().any(|line| line.starts_with("leselang-vm v")),
            vm
        );
        assert_eq!(
            tree.lines().any(|line| line.starts_with("rusqlite v")),
            sqlite
        );
        assert_eq!(
            tree.lines()
                .any(|line| line.starts_with("libsqlite3-sys v")),
            sqlite
        );
        assert!(
            tree.lines()
                .any(|line| line.starts_with("leselang-command v"))
        );
        assert!(
            tree.lines()
                .any(|line| line.starts_with("leserpent-domain v"))
        );
    }
}

#[test]
fn read_only_observation_and_optional_mutation_keep_the_original_authority_boundary() {
    let source = fs::read_to_string(root().join("crates/leselang-observe/src/lib.rs")).unwrap();
    let (production, tests) = source
        .split_once("#[cfg(all(test, feature = \"vm-bridge\"))]")
        .unwrap();
    assert!(!production.contains("leselang_vm"));
    assert!(production.contains("use leselang_runtime_core::Fault"));
    assert!(
        production.contains("use leselang_command::reference_request::validate_effect_request")
    );
    assert!(
        production.contains(
            "validate_effect_request(request).map_err(ObserveError::InvalidEffectRequest)?"
        )
    );
    assert!(production.contains("#[cfg(feature = \"vm-bridge\")]\nmod vm_bridge;"));
    assert!(production.contains("#[cfg(feature = \"vm-bridge\")]\npub use vm_bridge::{execute_debugger_cancel, execute_debugger_cancel_effect};"));
    assert!(tests.contains("debugger_cancel_requires_capability_and_confirmation_then_applies"));
    let bridge =
        fs::read_to_string(root().join("crates/leselang-observe/src/vm_bridge.rs")).unwrap();
    let mut rest = bridge.as_str();
    for original_gate in [
        "inspect_debugger_cancel_plan(plan, projection)?",
        "encode_continuation(image).map_err(ObserveError::InvalidDebuggerContinuation)?",
        "if image.program_counter != projection.program_counter",
        "if inspection.dry_run",
        "let audit = DebuggerAuditContext",
        ".cancel_effect_audited(image, now_ms, &audit)",
        "audited_at_ms: Some(record.observed_at_ms)",
        "if mutation.status != DebuggerMutationStatus::Applied",
    ] {
        rest = rest
            .split_once(original_gate)
            .unwrap_or_else(|| panic!("lost ordered bridge gate {original_gate}"))
            .1;
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-observe/tests/projection_profile.rs"))
            .unwrap();
    let (read_only, bridge_tests) = tests.split_once("#[cfg(feature = \"vm-bridge\")]").unwrap();
    assert!(!read_only.contains("leselang_vm"));
    assert_eq!(read_only.matches("#[test]").count(), 5);
    assert_eq!(bridge_tests.matches("#[test]").count(), 2);
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    for command in [
        "cargo +1.98.0 test -p leselang-observe --locked --no-default-features",
        "cargo +1.98.0 clippy -p leselang-observe --locked --no-default-features --all-targets -- -D warnings",
        "cargo +1.98.0 test -p leselang-observe --locked --no-default-features --features vm-bridge",
        "cargo +1.98.0 clippy -p leselang-observe --locked --no-default-features --features vm-bridge --all-targets -- -D warnings",
    ] {
        assert!(ci.contains(command), "{command}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split_once("### Optional Observation VM Bridge")
        .unwrap()
        .1;
    assert!(section.contains("Cargo features are additive, not an authorization boundary"));
    assert!(section.contains("not a\nhost-neutral UI adapter or complete standalone VM"));
}

#[test]
fn execution_observation_types_move_without_terminal_or_dispatch_ownership() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/execution_outcome.rs"))
            .unwrap();
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let scheduler =
        fs::read_to_string(root().join("crates/leselang-vm/src/scheduling.rs")).unwrap();
    for declaration in [
        "pub enum Step",
        "pub enum RetryDisposition",
        "pub enum DispatchClaim",
        "pub struct NamedEffectRequest",
        "pub struct StructuredEffectBatch",
        "pub struct MergeWait",
    ] {
        assert!(adapter.contains(declaration), "{declaration}");
        assert!(!vm.contains(declaration));
        assert!(!scheduler.contains(declaration));
    }
    for absent in [
        "leselang_vm",
        "rusqlite",
        "Fuel::",
        "LSV",
        "pub type Step",
        "pub type RetryDisposition",
        "pub type DispatchClaim",
        "pub fn ",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert_eq!(adapter.matches("deny_unknown_fields").count(), 1);
    assert!(vm.contains(
        "pub use leselang_command::{MergeWait, NamedEffectRequest, Step, StructuredEffectBatch}"
    ));
    assert!(vm.contains(
        "pub use leselang_command::{DispatchLease, FailedEffect, RetryDisposition, RetrySchedule}"
    ));
    assert!(
        scheduler.contains(
            "pub use leselang_command::{DispatchClaim, SchedulerLimits, SchedulerPressure}"
        )
    );
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("use leselang_command::reference_completion::{"));
    let completion =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_completion.rs"))
            .unwrap();
    assert!(
        completion
            .contains("Step::Effect(_) | Step::Effects(_) | Step::Waiting(_) | Step::Yield(_)")
    );
    let dispatch =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_dispatch.rs"))
            .unwrap();
    assert!(journal.contains("use leselang_command::reference_dispatch::{"));
    assert!(dispatch.contains("validate_dispatch_lease("));
    assert!(journal.contains("adapter_step_decode_does_not_replace_original_terminal_admission"));
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/execution_outcome_contract_tests.rs"),
    )
    .unwrap();
    assert!(frozen.contains("46_224"));
    assert!(frozen.contains("assert_eq!(comparisons, 630)"));
    let frozen = frozen.split_once("mod frozen {").unwrap().1;
    assert!(!frozen.contains("leselang_command"));
    assert!(frozen.contains("pub enum Step"));
    assert!(frozen.contains("pub enum DispatchClaim"));
    let memory =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(memory.contains(
        "execution_observations_are_exact_adapter_types_without_completion_or_lease_authority"
    ));
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Execution Observation Ownership")
    );
}

#[test]
fn concrete_product_request_and_static_policy_move_without_execution_authority() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(vm.contains("pub use leselang_command::{ContinuationImage, EffectRequest}"));
    for removed in [
        "pub struct EffectRequest",
        "impl<'de> Deserialize<'de> for EffectRequest",
        "fn continuation_admission_limits(",
        "fn request_admission_fault(",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    for required in [
        "pub struct EffectRequest",
        "pub fn as_view(",
        "pub fn validate_effect_request(",
        "pub fn continuation_admission_limits(",
        "pub fn request_admission_fault(",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "rusqlite",
        "Fuel::new",
        ".clone()",
        "Journal",
        "canonical_source(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let restore = vm
        .split_once("pub fn restore_request(")
        .unwrap()
        .1
        .split_once("pub fn resume(")
        .unwrap()
        .0;
    assert!(restore.contains("reference_restore::ReferenceRestoreCache"));
    assert!(restore.contains(".restore_request(request,"));
    assert!(restore.contains(".record_pending(image, request, self.scheduler_limits)"));
    let restore =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_restore.rs"))
            .unwrap();
    assert!(
        restore.find("validate_effect_request(&request)?").unwrap()
            < restore
                .find("restore_inner(request.continuation.clone()")
                .unwrap()
    );
    assert!(restore.contains("validate_continuation_size(&image)?"));
    assert!(restore.contains("(self.commit)(&image, request)?"));
    let continuation =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_continuation.rs"))
            .unwrap();
    assert!(continuation.contains("canonical_source(&image.pending_effect)"));
    let codec =
        fs::read_to_string(root().join("crates/leselang-vm/src/request_codec_tests.rs")).unwrap();
    assert!(codec.contains("operation_request_tests::frozen_validate_effect_request(&request)"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(consumer.contains("use leselang_command::{CancellationReason, EffectRequest}"));
    assert!(
        consumer.contains(
            "adapter_request_decoding_is_exact_data_without_restore_or_mutation_authority"
        )
    );
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Concrete Product Request Ownership"));
}

#[test]
fn product_identity_and_terminal_data_move_without_runtime_or_storage_authority() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(vm.contains("pub use leselang_command::ContinuationToken"));
    assert!(vm.contains("pub use leselang_command::{BranchCompletion, BranchOutcome, Cancellation, CancellationReason}"));
    assert!(vm.contains("pub(crate) use leselang_command::continuation_identity::"));
    for removed in [
        "pub struct ContinuationToken",
        "pub struct Cancellation",
        "pub enum CancellationReason",
        "pub struct BranchCompletion",
        "pub enum BranchOutcome",
        "fn canonical_continuation_sequence(",
        "fn continuation_age_order(",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    for retained in [
        "pub use leselang_command::{MergeWait, NamedEffectRequest, Step, StructuredEffectBatch}",
        "pub use leselang_command::{DispatchLease, FailedEffect, RetryDisposition, RetrySchedule}",
        "pub(crate) use leselang_command::reference_value::{",
        "invalid_continuation_sequence_fault",
    ] {
        assert!(vm.contains(retained), "{retained}");
    }
    let allocation =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_allocation.rs"))
            .unwrap();
    assert!(allocation.contains("if sequence == 0 || sequence > MAX_EFFECT_SEQUENCE"));
    assert!(vm.contains("reference_allocation::allocate_sequence("));
    let identity =
        fs::read_to_string(root().join("crates/leselang-command/src/continuation_identity.rs"))
            .unwrap();
    let continuation =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_continuation.rs"))
            .unwrap();
    assert!(continuation.contains("validate_restorable_image(&image)?"));
    let terminal =
        fs::read_to_string(root().join("crates/leselang-command/src/terminal_outcome.rs")).unwrap();
    for source in [&identity, &terminal] {
        for absent in ["leselang_vm", "rusqlite", "Fuel::", "LSV", "Transaction"] {
            assert!(!source.contains(absent), "{absent}");
        }
    }
    assert!(identity.contains("pub struct ContinuationToken(String)"));
    assert!(!identity.contains("pub struct ContinuationToken(pub"));
    assert!(identity.contains("pub fn from_wire(value: String) -> Self"));
    assert!(identity.contains("pub fn canonical_merge_sequence("));
    assert!(terminal.contains("pub enum BranchOutcome"));
    assert!(terminal.contains("pub enum CancellationReason"));
    assert!(!terminal.contains("deny_unknown_fields"));
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("merge_reserved_sequence(group_token, plan)"));
    let batch =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_batch.rs"))
            .unwrap();
    assert!(batch.contains("canonical_merge_sequence(token)"));
    assert!(batch.contains("canonical_continuation_sequence(token)"));
    assert!(journal.contains("fn advance_sequence_watermark("));
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/identity_outcome_contract_tests.rs"),
    )
    .unwrap();
    let frozen = frozen.split_once("mod frozen {").unwrap().1;
    assert!(!frozen.contains("leselang_command"));
    assert!(frozen.contains("pub struct ContinuationToken(String)"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(consumer.contains(
        "identity_and_terminal_data_are_exact_adapter_types_without_cancellation_authority"
    ));
    let clocks =
        fs::read_to_string(root().join("crates/leselang-vm/tests/clock_boundaries.rs")).unwrap();
    assert!(clocks.contains("use leselang_command::CancellationReason"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Identity And Terminal Data Ownership"));
}

#[test]
fn bounded_input_decoding_is_shared_but_does_not_replace_product_admission() {
    let codec =
        fs::read_to_string(root().join("crates/leselang-command/src/bounded_json.rs")).unwrap();
    let decode = codec
        .split_once("pub fn decode<'de, T: Deserialize<'de>>(")
        .unwrap()
        .1
        .split_once("#[derive(Debug)]")
        .unwrap()
        .0;
    assert!(
        decode.find("bytes.len() > limit").unwrap() < decode.find("from_slice(bytes)").unwrap()
    );
    assert!(!decode.contains("DeserializeOwned"));
    assert!(!codec.contains("disable_recursion_limit"));
    assert!(codec.contains("pub enum BoundedJsonDecodeError"));
    let codec =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_continuation.rs"))
            .unwrap();
    let continuation = codec
        .split_once("pub fn decode_continuation(")
        .unwrap()
        .1
        .split_once("fn validate_pending_effect_contract(")
        .unwrap()
        .0;
    assert!(
        continuation.find("bounded_json::decode(").unwrap()
            < continuation
                .find("validate_restorable_image(&image)?")
                .unwrap()
    );
    assert!(!continuation.contains("serde_json::from_slice"));
    for code in ["LSV3002", "LSV3003"] {
        assert!(continuation.contains(code));
    }
    let plan = fs::read_to_string(root().join("crates/leselang-command/src/lib.rs")).unwrap();
    let plan = plan
        .split_once("pub fn decode_plan(")
        .unwrap()
        .1
        .split_once("pub fn lower_effect(")
        .unwrap()
        .0;
    assert!(
        plan.find("bounded_json::decode(").unwrap() < plan.find("plan.schema_version !=").unwrap()
    );
    assert!(
        plan.find("plan.schema_version !=").unwrap() < plan.find("validate_plan(&plan)?").unwrap()
    );
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("decode_journal_value as decode_bounded"));
    let recovery =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_recovery.rs"))
            .unwrap();
    let decode = recovery
        .split_once("pub fn decode_journal_value<")
        .unwrap()
        .1
        .split_once("struct RecordPolicy<")
        .unwrap()
        .0;
    assert!(decode.contains("bounded_json::decode(bytes, limit)"));
    assert!(!decode.contains("serde_json::from_slice"));
    for code in ["LSV4006", "LSV4007"] {
        assert!(decode.contains(code));
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/bounded_decode_tests.rs"))
            .unwrap();
    let frozen = tests
        .split_once("fn frozen_decode<")
        .unwrap()
        .1
        .split_once("fn compare<")
        .unwrap()
        .0;
    assert!(frozen.contains("serde_json::from_slice(bytes)"));
    assert!(!frozen.contains("bounded_json::decode"));
    assert!(tests.contains("assert_eq!(snapshot(&connection), before)"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Bounded Decode Ownership"));
}

#[test]
fn product_scheduler_policy_moves_without_owning_leases_or_journal_admission() {
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/scheduling.rs")).unwrap();
    assert!(
        vm.contains(
            "pub use leselang_command::{DispatchClaim, SchedulerLimits, SchedulerPressure}"
        )
    );
    assert!(!vm.contains("pub struct SchedulerLimits"));
    assert!(!vm.contains("pub struct SchedulerPressure"));
    let outcomes =
        fs::read_to_string(root().join("crates/leselang-command/src/execution_outcome.rs"))
            .unwrap();
    assert!(outcomes.contains("pub enum DispatchClaim"));
    assert!(!vm.contains("pub enum DispatchClaim"));
    assert!(!vm.contains("pub type DispatchClaim"));
    for delegated in [
        "leselang_command::reference_startup::validate_limits(self)",
        "check_scheduler_admission(self, pending, additional)",
        "scheduler_lease_capacity_exhausted(self)",
    ] {
        assert!(vm.contains(delegated), "{delegated}");
    }
    let startup =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_startup.rs"))
            .unwrap();
    assert!(startup.contains("validate_scheduler_limits(limits).map_err"));
    assert!(startup.contains("LSV2500"));
    for code in ["LSV2501", "LSV2503"] {
        assert!(vm.contains(code));
    }
    assert!(vm.contains("leselang_command::reference_claim::lease_capacity_fault(self)"));
    let claim =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_claim.rs")).unwrap();
    assert!(claim.contains("LSV2502"));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/scheduler_policy.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "Fault",
        "LSV",
        "DispatchClaim",
        "DispatchLease",
        "Fuel",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("use crate::retention_policy::MAX_JOURNAL_RECORDS"));
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("pub use leselang_command::retention_policy::MAX_JOURNAL_RECORDS"));
    assert!(journal.contains("limits.check_admission("));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/scheduling_contract_tests.rs"))
            .unwrap();
    let frozen = frozen.split_once("mod frozen {").unwrap().1;
    assert!(!frozen.contains("leselang_command"));
    assert!(frozen.contains("check_dispatch_admission("));
    let integration =
        fs::read_to_string(root().join("crates/leselang-vm/tests/backpressure.rs")).unwrap();
    assert!(integration.contains("use leselang_command::{SchedulerLimits, SchedulerPressure}"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Scheduler Policy Ownership"));
}

#[test]
fn product_retention_policy_is_shared_and_memory_compaction_preflights_before_mutation() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(vm.contains("pub use leselang_command::{CompactionReport, RetentionPolicy}"));
    assert!(!vm.contains("pub struct RetentionPolicy"));
    assert!(!vm.contains("pub struct CompactionReport"));
    let maintenance = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_settlement/maintenance.rs"),
    )
    .unwrap();
    assert!(maintenance.contains("validate_retention_policy(policy, MAX_JOURNAL_RECORDS)"));
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert_eq!(
        journal
            .matches("policy.removal_count(completed_count)")
            .count(),
        2
    );
    let compact = journal
        .split_once("impl EphemeralJournal {")
        .unwrap()
        .1
        .split_once("    fn compact_completed(")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    let preflight = compact
        .split_once("for unit in units.iter().take(remove_count)")
        .unwrap()
        .1
        .split_once("for unit in units.into_iter().take(remove_count)")
        .unwrap()
        .0;
    assert_eq!(preflight.matches("measure_json_capped(").count(), 5);
    for absent in [".remove(", ".retain(", ".get_mut(", "encode_json_capped("] {
        assert!(!preflight.contains(absent), "{absent}");
    }
    assert!(compact.contains("self.debugger_audits"));
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/retention_policy.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "rusqlite",
        "ContinuationToken",
        "LSV",
        "Fuel::",
    ] {
        assert!(!policy.contains(absent), "{absent}");
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/retention_tests.rs"))
            .unwrap();
    let frozen = tests.split_once("fn frozen_compact(").unwrap().1;
    assert!(!frozen.contains("measure_json_capped("));
    assert!(!frozen.contains("removal_count("));
    assert!(frozen.contains("saturating_sub(policy.max_completed_records)"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Retention Policy Ownership"));
}

#[test]
fn product_bounded_json_is_shared_without_moving_admission_or_journal_ownership() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for absent in [
        "struct CappedWriter",
        "struct CappedCounter",
        "serde_json::to_writer(",
    ] {
        assert!(!vm.contains(absent), "{absent}");
    }
    for required in [
        "leselang_command::bounded_json::encode(value, limit)",
        "leselang_command::bounded_json::measure(value, limit)",
        "\"LSV3001\"",
        "\"LSV3002\"",
    ] {
        assert!(vm.contains(required), "{required}");
    }
    let codec =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_continuation.rs"))
            .unwrap();
    for required in [
        "crate::validate_result_binding_structure(",
        "validate_restorable_image(image)?",
        "bounded_json::measure(image, MAX_CONTINUATION_BYTES)",
    ] {
        assert!(codec.contains(required), "{required}");
    }
    let plan = fs::read_to_string(root().join("crates/leselang-command/src/lib.rs")).unwrap();
    let plan = plan
        .split_once("pub fn encode_plan(")
        .unwrap()
        .1
        .split_once("pub fn decode_plan(")
        .unwrap()
        .0;
    assert!(
        plan.find("validate_plan(plan)?").unwrap() < plan.find("bounded_json::encode(").unwrap()
    );
    assert!(!plan.contains("CommandPlanWriter"));
    let codec =
        fs::read_to_string(root().join("crates/leselang-command/src/bounded_json.rs")).unwrap();
    for required in [
        "pub fn encode<",
        "pub fn measure<",
        "self.bytes.checked_add(buffer.len())",
        "if let Some(attempted) = writer.overflow",
        "Option<&mut Vec<u8>>",
    ] {
        assert!(codec.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "rusqlite",
        "Fuel::",
        "LSV",
        "ContinuationImage",
    ] {
        assert!(!codec.contains(absent), "{absent}");
    }
    assert!(
        codec
            .find("if let Some(attempted) = writer.overflow")
            .unwrap()
            < codec.find("result.map_err(").unwrap()
    );
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Bounded JSON Ownership"));
}

#[test]
fn product_merge_completion_moves_ordering_without_moving_vm_finalization() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let merge = vm
        .split_once("fn merge_declared_inner(")
        .unwrap()
        .1
        .split_once("pub(crate) use leselang_command::reference_merge::validate_merge_plan;")
        .unwrap()
        .0;
    let mut offset = 0;
    for ordered in [
        "leselang_command::reference_merge_output::assemble_merge(",
        "binding.validate_member(branch, value)",
        "binding.finish(value)",
    ] {
        offset += merge[offset..].find(ordered).unwrap() + ordered.len();
    }
    assert!(!merge.contains("by_branch"));
    assert!(!merge.contains("saturating_add"));
    let output =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_merge_output.rs"))
            .unwrap();
    let mut offset = 0;
    for ordered in [
        "validate_merge_plan(plan)?",
        "merge_preflighted_completions(",
        "validate_value(value, 1)?",
        "bounded_json::measure(&step, MAX_JOURNAL_ENTRY_BYTES)",
    ] {
        offset += output[offset..].find(ordered).unwrap() + ordered.len();
    }
    for absent in [
        "binding.finish(",
        "Fuel::new",
        "rusqlite",
        "leselang_vm",
        "computation::",
    ] {
        assert!(!output.contains(absent), "{absent}");
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/merge_completion.rs")).unwrap();
    for required in [
        "pub trait MergeCompletionHost",
        "pub fn merge_preflighted_completions",
        "completions.len() != plan.branches.len()",
        "by_branch.contains_key(&branch)",
        "output_items.saturating_add",
        "host.validate_member(",
        "for branch in &plan.branches",
        "Callers must first bound and validate the plan",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "rusqlite",
        "Fuel::new",
        "ContinuationToken",
        "LSV",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/merge_completion_tests.rs"))
            .unwrap();
    assert!(frozen.contains("9_720"));
    let frozen = frozen.split_once("fn frozen_merge(").unwrap().1;
    assert!(!frozen.contains("merge_preflighted_completions"));
    assert!(frozen.contains("by_branch.contains_key"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Merge Completion Ownership"));
}

#[test]
fn product_retry_policy_is_adapter_owned_but_replay_and_journal_fencing_keep_native_owners() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for removed in [
        "pub struct RetryPolicy",
        "pub enum EffectErrorClass",
        "pub struct EffectError",
        "fn validate_retry_policy(",
        "plan_retry_schedule(",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    assert!(vm.contains("pub use leselang_command::{EffectError, EffectErrorClass, RetryPolicy}"));
    assert!(vm.contains("leselang_command::retry_policy::validate_effect_error(error)"));
    let coordination =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_retry.rs")).unwrap();
    let reporting = coordination
        .split_once("pub fn report_effect_error<")
        .unwrap()
        .1;
    let mut offset = 0;
    for ordered in [
        "validate_clock(now_ms)?",
        "host.expire_due(now_ms)?",
        "host.replay_completed(",
        "crate::retry_policy::plan_effect_retry(",
        "RetryDisposition::Terminal(Step::Failed(",
        "host.settle_error(",
    ] {
        offset += reporting[offset..].find(ordered).unwrap() + ordered.len();
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/retry_policy.rs")).unwrap();
    let planning = adapter.split_once("pub fn plan_effect_retry(").unwrap().1;
    let mut offset = 0;
    for ordered in [
        "validate_effect_error(error)?",
        "EffectErrorClass::Permanent",
        "validate_retry_policy(policy)?",
        "plan_retry_schedule(",
    ] {
        offset += planning[offset..].find(ordered).unwrap() + ordered.len();
    }
    for absent in ["leselang_vm", "rusqlite", "Fuel::new", "LSV", ".clone()"] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("A proposal is not dispatch authority"));
    let integration =
        fs::read_to_string(root().join("crates/leselang-vm/tests/retry_backoff.rs")).unwrap();
    assert!(
        integration.contains("use leselang_command::{EffectError, EffectErrorClass, RetryPolicy}")
    );
    assert!(integration.contains("terminal_replay_precedes_late_invalid_error_and_policy"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/retry_policy_adapter_tests.rs"))
            .unwrap();
    assert!(frozen.contains("60_480"));
    assert!(
        !frozen
            .split_once("mod frozen {")
            .unwrap()
            .1
            .contains("leselang_command")
    );
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Semantic Retry Ownership"));
}

#[test]
fn product_debugger_audit_metadata_is_adapter_owned_without_moving_commit_authority() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/debugger_audit.rs")).unwrap();
    let observe =
        fs::read_to_string(root().join("crates/leselang-observe/src/vm_bridge.rs")).unwrap();
    for removed in [
        "pub struct DebuggerAuditContext",
        "pub struct DebuggerAuditRecord",
        "let valid_session =",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    assert!(vm.contains("leselang_command::reference_cancellation::validate_audit(audit)"));
    assert!(journal.contains("use leselang_command::debugger_audit::command_origin_label"));
    assert!(journal.contains("use leselang_command::debugger_audit::{"));
    assert!(!journal.contains("fn debugger_audit_record("));
    assert!(!journal.contains("fn audit_matches_record("));
    assert!(journal.contains("audit_read::decode_record"));
    assert!(observe.contains(
        "use leselang_command::{\n    CommandPlan, ContinuationImage, DebuggerAuditContext, DebuggerCancelResult, encode_continuation,\n}"
    ));
    assert_eq!(adapter.matches("#[serde(deny_unknown_fields)]").count(), 2);
    for absent in [
        "leselang_vm",
        "rusqlite",
        "ContinuationToken",
        "Fuel",
        "LSV",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    assert!(adapter.contains("A match is not replay permission"));
    assert!(adapter.contains("A returned record is not evidence"));
    let coordination =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_cancellation.rs"))
            .unwrap();
    let cancel = coordination
        .split_once("pub fn cancel_effect_audited<")
        .unwrap()
        .1
        .split_once("pub fn validate_audit(")
        .unwrap()
        .0;
    let mut offset = 0;
    for ordered in [
        "validate_audit(audit)?",
        "validate_clock(now_ms)?",
        "host.expire_due(now_ms)?",
        "host.reconcile_completed(&image.token)?",
        "host.finish_audited_cancellation(image, &step, audit, now_ms)",
    ] {
        offset += cancel[offset..].find(ordered).unwrap() + ordered.len();
    }
    let commit =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_audit_commit.rs"))
            .unwrap();
    for moved in [
        "existing.token == &image.token",
        "existing.idempotency_key == audit.idempotency_key.as_str()",
        "audit_matches_record(audit, existing.record)",
        "completed effect has no matching debugger audit",
    ] {
        assert!(commit.contains(moved), "{moved}");
    }
    assert!(!journal.contains("crate::validate_debugger_audit(&context)"));
    assert!(journal.contains("reference_audit_recovery::validate_audit_records"));
    let recovery =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_audit_recovery.rs"))
            .unwrap();
    assert!(recovery.contains("validate_debugger_audit(&context)"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/debugger_audit_adapter_tests.rs"))
            .unwrap();
    let frozen = frozen.split_once("mod frozen {").unwrap().1;
    assert!(!frozen.contains("leselang_command"));
    assert!(frozen.contains("let valid_session ="));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Debugger Audit Ownership"));
}

#[test]
fn product_request_codec_is_adapter_owned_and_vm_admission_stays_mandatory() {
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(vm.contains("pub use leselang_command::{ContinuationImage, EffectRequest}"));
    let decode = request
        .split("impl<'de> Deserialize<'de> for EffectRequest")
        .nth(1)
        .unwrap()
        .split("pub fn result_binding_limits")
        .next()
        .unwrap();
    let parsed = decode
        .find("crate::decode_effect_request(deserializer)?")
        .unwrap();
    let admitted = decode.find("validate_effect_request(&request)").unwrap();
    let returned = decode.find("Ok(request)").unwrap();
    assert!(parsed < admitted && admitted < returned);
    for removed in [
        "struct CurrentEffectRequestWire",
        "struct LegacyQueryEffectRequestWire",
        "enum EffectRequestWire",
    ] {
        assert!(!vm.contains(removed), "{removed}");
    }
    let codec =
        fs::read_to_string(root().join("crates/leselang-command/src/request_codec.rs")).unwrap();
    for required in [
        "pub struct DecodedEffectRequest<Continuation>",
        "pub fn decode_effect_request<'de, D, Continuation>",
        "Continuation: Deserialize<'de>",
        "enum EffectRequestWire<Continuation>",
        "operation: EffectOperation::Query(query)",
        "Callers must bound ingress",
    ] {
        assert!(codec.contains(required), "{required}");
    }
    assert_eq!(codec.matches("#[serde(deny_unknown_fields)]").count(), 2);
    for absent in [
        "leselang_vm",
        "rusqlite",
        "Fuel::new",
        ".clone()",
        "ContinuationToken",
        "validate_effect_request(",
    ] {
        assert!(!codec.contains(absent), "{absent}");
    }
    let source =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(source.contains("leselang_command::decode_effect_request(deserializer)?"));
    assert!(source.contains("validate_effect_request("));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/request_codec_tests.rs")).unwrap();
    assert!(frozen.contains("EffectRequestWire::deserialize(deserializer)?"));
    assert!(!frozen.contains("decode_effect_request("));
    assert!(frozen.contains("74 * 18"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Product Request Codec Ownership"));
    assert!(docs.contains("Decoded data is deliberately untrusted"));
}

fn assert_complete_checkpoint_finalization_order() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/checkpoint.rs")).unwrap();
    let ordered = core
        .split_once("pub fn finish_recovery<")
        .unwrap()
        .1
        .split_once("pub fn finish<")
        .unwrap()
        .0
        .split_whitespace()
        .collect::<String>();
    let mut offset = 0;
    for stage in [
        "self.finish(host,",
        "host.validate_dispatches()",
        "host.validate_graphs()",
        "snapshot.highest_sequence().max(graph_sequence)",
        "host.validate_audits()",
        "host.advance_watermark(highest_sequence)",
        "match host.commit()",
        "Ok(()) => Ok(metadata)",
        "Err(error) =>",
        "drop(metadata)",
        "Err(CheckpointFinalizationError::Commit(error))",
    ] {
        let compact = stage.split_whitespace().collect::<String>();
        offset += ordered[offset..]
            .find(&compact)
            .unwrap_or_else(|| panic!("missing/reordered {stage}"))
            + compact.len();
    }
    for absent in [
        "rusqlite",
        "leserpent",
        "DebuggerAuditContext",
        "EffectRequest",
        "unwrap(",
        "expect(",
    ] {
        assert!(!ordered.contains(absent), "{absent}");
    }
}

#[test]
fn complete_checkpoint_finalization_is_shared_without_importing_product_audit_policy() {
    assert_complete_checkpoint_finalization_order();
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(code.contains("recover_checkpoint(SqliteRecoveryFinalizer"));
    let adapter = code
        .split_once("impl leselang_runtime_core::CheckpointFinalizer for SqliteRecoveryFinalizer")
        .unwrap()
        .1
        .split_once("fn validate_debugger_audit_records(")
        .unwrap()
        .0;
    let compact_adapter = adapter.split_whitespace().collect::<String>();
    for boundary in [
        "validate_dispatch_records(self.connection()?)",
        "validate_merge_graph_records(self.connection()?)",
        "validate_debugger_audit_records(self.connection()?)",
        "advance_sequence_watermark(self.connection()?, highest_sequence)",
        "self.transaction.take()",
        "transaction.commit()",
    ] {
        assert!(
            compact_adapter.contains(&boundary.split_whitespace().collect::<String>()),
            "{boundary}"
        );
    }
    assert!(code.contains("reference_audit_recovery::validate_audit_records"));
    let audit =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_audit_recovery.rs"))
            .unwrap();
    assert!(audit.contains("validate_debugger_audit(&context)"));
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/finalization_contract_tests.rs"),
    )
    .unwrap();
    assert!(!frozen.contains("finish_recovery("));
    for proof in [
        "assert_eq!(comparisons, 420)",
        "DEFERRABLE INITIALLY DEFERRED",
        "all_tables(&actual.connection), before",
        "original.is_autocommit()",
    ] {
        assert!(frozen.contains(proof), "{proof}");
    }
    let consumer =
        fs::read_to_string(root().join("crates/leselang-runtime-core/tests/checkpoint.rs"))
            .unwrap();
    for test in [
        "ordered_finalization_joins_original_maxima_without_clamping_or_early_handoff",
        "ordered_finalization_keeps_native_error_ownership_and_exact_refusal_prefix",
        "ordered_finalization_each_hook_unwind_discards_staging_without_retry",
        "ordered_finalization_cleanup_unwind_keeps_snapshot_metadata_and_error_local",
        "ordered_finalization_commit_then_unwind_is_uncertain_not_replay_permission",
        "ordered_finalization_of_closed_staging_releases_host_without_any_hook",
    ] {
        assert!(consumer.contains(test), "{test}");
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/checkpoint.rs"));
    assert!(exporter.contains("consumer/tests/checkpoint_recovery.rs"));
}

#[test]
fn complete_dispatch_recovery_is_shared_before_checkpoint_commit() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/dispatch_recovery.rs"))
            .unwrap();
    let body = core
        .split_once("pub fn inspect_dispatch_recovery<")
        .unwrap()
        .1
        .split_whitespace()
        .collect::<String>();
    let mut offset = 0;
    for stage in [
        "host.admit_request()",
        "host.validate_identity(&request)",
        "host.validate_image(&request)",
        "host.read_metadata()",
        "metadata.attempt < 0",
        "metadata.ready_at_ms < 0",
        "(metadata.retry_count == 0) != metadata.last_error.is_none()",
        "if let Some(original) = metadata.last_error",
        "host.validate_retry_error(original)",
        "metadata.acknowledged != metadata.effect_completed",
        "drop(request)",
        "Ok(())",
    ] {
        let compact = stage.split_whitespace().collect::<String>();
        offset += body[offset..]
            .find(&compact)
            .unwrap_or_else(|| panic!("missing/reordered {stage}"))
            + compact.len();
    }
    for absent in [
        "rusqlite",
        "leselang_command",
        "leselang_vm",
        "EffectRequest",
        "serde_json",
        "Fuel::",
        ".clone()",
        "unwrap(",
        "expect(",
    ] {
        assert!(!core.contains(absent), "{absent}");
    }
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let pass = code
        .split_once("fn validate_dispatch_records(")
        .unwrap()
        .1
        .split_once("fn validate_debugger_audit_records(")
        .unwrap()
        .0;
    for boundary in [
        "recover_dispatch_record(row)?",
        "inspect_dispatch_record(&mut SqliteRecoveryDispatch { row }, &token)",
        "impl ReferenceDispatchSource for SqliteRecoveryDispatch",
        "invalid dispatch request",
        "invalid dispatch continuation image",
        "invalid dispatch retry error",
    ] {
        assert!(pass.contains(boundary), "{boundary}");
    }
    let adapter = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_dispatch_recovery.rs"),
    )
    .unwrap();
    for boundary in [
        "inspect_dispatch_recovery(",
        "DispatchRecoveryHost for DispatchPolicy",
        "MAX_JOURNAL_ENTRY_BYTES",
        "MAX_CONTINUATION_BYTES",
        "MAX_DISPATCH_ATTEMPTS",
        "MAX_SEMANTIC_RETRIES",
        "decode_journal_value(bytes.as_ref(), 2_048)",
        "validate_effect_request(&request)?",
        "request.continuation.token.as_str() != self.token",
        "request_image != effect_image.as_ref()",
    ] {
        assert!(adapter.contains(boundary), "{boundary}");
    }
    assert!(!pass.contains("if attempt < 0"));
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/dispatch_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(!frozen.contains("inspect_dispatch_recovery("));
    assert!(frozen.contains("assert_eq!(comparisons, 1176)"));
    assert!(frozen.contains("assert_eq!(snapshot(&tx), before)"));
    assert!(frozen.contains(
        "late_dispatch_refusal_blocks_snapshot_and_watermark_until_original_data_is_repaired"
    ));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/dispatch_recovery.rs"));
    assert!(exporter.contains("consumer/tests/dispatch_plan.rs"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-runtime-core/tests/dispatch_plan.rs"))
            .unwrap();
    for test in [
        "dispatch_recovery_numeric_and_history_order_matches_boundary_oracle",
        "dispatch_recovery_preserves_move_only_local_owners_and_lazy_error_decode",
        "dispatch_recovery_native_errors_are_owned_redacted_and_stop_at_first_fault",
        "dispatch_recovery_every_hook_unwind_and_final_cleanup_stop_without_retry",
        "dispatch_recovery_refusal_or_commit_failure_never_publishes_staged_checkpoint",
    ] {
        assert!(consumer.contains(test), "{test}");
    }
}

#[test]
fn product_dispatch_recovery_keeps_sql_reads_and_authority_outside_the_adapter() {
    let adapter = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_dispatch_recovery.rs"),
    )
    .unwrap();
    for absent in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "std::time",
        ".commit()",
        "encode_continuation(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let storage = journal
        .split_once("impl ReferenceDispatchSource for SqliteRecoveryDispatch")
        .unwrap()
        .1
        .split_once("struct SqliteRecoveryFinalizer")
        .unwrap()
        .0;
    let mut cursor = 0;
    for column in [1, 9, 2, 3, 8, 5, 6, 7] {
        let needle = format!(".get({column})");
        cursor += storage[cursor..].find(&needle).unwrap() + needle.len();
    }
    for absent in [
        "decode_bounded",
        "validate_effect_request",
        "DispatchRecoveryError",
        ".get(4)",
        ".commit()",
    ] {
        assert!(!storage.contains(absent), "{absent}");
    }
    let direct = fs::read_to_string(
        root().join("crates/leselang-command/tests/reference_dispatch_recovery.rs"),
    )
    .unwrap();
    for case in [
        "dispatch_policy_matches_frozen_fault_precedence_and_lazy_read_traces",
        "dispatch_numeric_history_error_and_completion_boundaries_match_frozen_policy",
        "request_bound_is_checked_before_parsing_and_image_comparison_is_byte_exact",
        "local_move_only_byte_owners_and_each_unwind_stop_without_retry",
    ] {
        assert!(direct.contains(case), "{case}");
    }
    let authority = fs::read_to_string(
        root().join("crates/leselang-vm/tests/support/reference_dispatch_authority.rs"),
    )
    .unwrap();
    assert!(authority.contains("inspect_supplied_dispatch(&forged)"));
    assert!(authority.contains("inspect_supplied_dispatch(&original)"));
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_dispatch_recovery")
    );
}

fn assert_complete_record_recovery_order() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/record_recovery.rs"))
            .unwrap();
    let body = core
        .split_once("pub fn inspect_record_recovery<")
        .unwrap()
        .1
        .split_whitespace()
        .collect::<String>();
    let mut offset = 0;
    for stage in [
        "host.admit_image()",
        "host.sequence(&image)",
        "host.validate_identity(&image)",
        "host.validate_deadline(&image)",
        "host.validate_owner(&image)",
        "host.stored_state()",
        "StoredRecordState::Pending",
        "host.validate_successor(&image, None)",
        "host.pending_payload(image)",
        "StoredRecordState::Completed",
        "host.admit_completed()",
        "host.validate_successor(&image, Some(&completed))",
        "host.completed_payload(image, completed)",
        "Ok(RecoveryRecord",
    ] {
        let compact = stage.split_whitespace().collect::<String>();
        offset += body[offset..]
            .find(&compact)
            .unwrap_or_else(|| panic!("missing/reordered {stage}"))
            + compact.len();
    }
    for absent in [
        "rusqlite",
        "leselang_command",
        "leselang_vm",
        "ContinuationImage",
        "serde_json",
        "Fuel::",
        ".clone()",
        "unwrap(",
        "expect(",
    ] {
        assert!(!core.contains(absent), "{absent}");
    }
}

#[test]
fn complete_effect_record_recovery_is_shared_before_opaque_checkpoint_staging() {
    assert_complete_record_recovery_order();
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(code.contains("impl ReferenceRecordSource for SqliteRecoveryRecord"));
    let compact = code.split_whitespace().collect::<String>();
    assert!(compact.contains("inspect_effect_record(&mutSqliteRecoveryRecord"));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_recovery.rs"))
            .unwrap();
    assert!(adapter.contains("RecordRecoveryHost for RecordPolicy<"));
    assert!(adapter.contains("inspect_record_recovery(&mut RecordPolicy"));
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/record_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(!frozen.contains("inspect_record_recovery("));
    assert!(frozen.contains("assert_eq!(comparisons, 784)"));
    assert!(frozen.contains("assert_eq!(snapshot(&tx), before)"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-runtime-core/tests/checkpoint.rs"))
            .unwrap();
    for test in [
        "stored_record_moves_original_noncomparable_local_owners_and_defers_pending_receipts",
        "stored_record_errors_keep_original_ownership_and_exact_native_stage",
        "stored_record_every_native_unwind_and_late_cleanup_panic_prevent_payload_handoff",
        "stored_record_late_refusal_or_commit_failure_never_publishes_checkpoint_prefix",
    ] {
        assert!(consumer.contains(test), "{test}");
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/record_recovery.rs"));
    assert!(exporter.contains("consumer/tests/checkpoint_recovery.rs"));
}

fn assert_complete_group_recovery_order() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_recovery.rs"))
            .unwrap();
    let ordered = core.split_once("pub fn inspect_group_recovery<").unwrap().1;
    let mut offset = 0;
    for stage in [
        "validate_group_graph(",
        "rows.iter().all(|row| host.is_completed(row))",
        "host.validate_chain()",
        "host.validate_pending_sequence(rows)",
        "host.stored_state()",
        "validate_group_receipt(",
        "host.validate_records(receipt.as_ref())",
        "Ok(receipt)",
    ] {
        offset += ordered[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("lost recovery order: {stage}"))
            + stage.len();
    }
}

fn assert_complete_group_batch_order() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_recovery_batch.rs"))
            .unwrap();
    let body = core
        .split_once("pub fn inspect_group_recovery_batch<")
        .unwrap()
        .1;
    let mut previous = 0;
    for step in [
        "groups.len() > limits.max_groups",
        "SequenceReservations::new(",
        "for stored in groups",
        ".admit_group(stored)",
        "host.highest_sequence(&group)",
        "host.reserved_sequences(&group)",
        ".insert(sequence)",
        "host.validate_order(&group)",
        "host.inspect_group(group)",
        ".finish()",
        "Ok(GroupRecoveryBatchSummary",
    ] {
        let position = body.find(step).unwrap_or_else(|| panic!("missing {step}"));
        assert!(position >= previous, "reordered {step}");
        previous = position;
    }
    for absent in [
        "rusqlite",
        "leselang_command",
        "leselang_vm",
        "Fuel::",
        ".clone()",
        "unwrap(",
        "expect(",
    ] {
        assert!(!core.contains(absent), "{absent}");
    }
}

#[test]
fn complete_stored_group_batch_uses_shared_global_reservations_before_checkpoint_commit() {
    assert_complete_group_batch_order();
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    for required in [
        "inspect_group_batch(groups, &mut RecoveredGroupsHost(connection))",
        "impl ReferenceGroupBatchProofs<Vec<u8>> for RecoveredGroupsHost",
        "validate_recovered_group(self.0, &token, &plan, &branches, &state, terminal)",
    ] {
        assert!(journal.contains(required), "{required}");
    }
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_batch.rs"))
            .unwrap();
    for required in [
        "inspect_group_recovery_batch(",
        "max_groups: MAX_JOURNAL_RECORDS",
        "max_reservations: MAX_JOURNAL_RECORDS * MAX_MERGE_BRANCHES",
        "GroupRecoveryBatchError::Reservation(_) => invalid_group_binding()",
        "summary.highest_sequence",
    ] {
        assert!(policy.contains(required), "{required}");
    }
    let mut offset = policy.find("fn admit_group(").unwrap();
    for required in [
        "valid_continuation_token(&token)",
        "self.0.token_conflicts(&token)?",
        "decode_journal_value(plan_bytes.as_ref(), MAX_CONTINUATION_BYTES)?",
        "validate_merge_plan(&plan)",
    ] {
        offset += policy[offset..].find(required).unwrap() + required.len();
    }
    for absent in [
        "rusqlite",
        "std::fs",
        "leselang_vm",
        "Fuel::",
        ".commit()",
        ".finish(value)",
    ] {
        assert!(!policy.contains(absent), "{absent}");
    }
    assert!(!journal.contains("fn merge_reserved_sequence("));
    assert!(!journal.contains("fn execution_order_label("));
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/group_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 364)"));
    assert!(!frozen.contains("inspect_group_recovery_batch("));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/group_recovery_batch.rs"));
    assert!(exporter.contains("consumer/tests/recovery_fences.rs"));
}

fn assert_complete_successor_admission_order() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_admission.rs"))
            .unwrap();
    let body = adapter
        .split_once("pub fn validate_successor_pair")
        .unwrap()
        .1;
    let mut offset = 0;
    for stage in [
        "validate_effect_request(",
        ".map_err(Parent)?",
        "validate_effect_request(",
        ".map_err(Next)?",
        "completed.ok_or(Completion)?",
        "matches!(value, Value::Scalar",
        "host.validate_completed_value(",
        "validate_preflighted_successor_pair(",
    ] {
        offset += body[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
}

#[test]
fn complete_successor_admission_shares_order_without_native_storage_or_error_payloads() {
    assert_complete_successor_admission_order();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_admission.rs"))
            .unwrap();
    for required in [
        "pub trait SuccessorAdmissionHost<Token>",
        "pub enum SuccessorAdmissionError<Native>",
        "completed: Option<&Value>",
        "Host: SuccessorAdmissionHost<Token> + ?Sized",
        "impl<Native> std::fmt::Debug",
        "impl<Native> std::error::Error",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "serde_json",
        "Fuel::",
        "Journal",
        ".clone()",
        "impl Default",
        "LSV",
    ] {
        assert!(
            !adapter.contains(absent),
            "successor admission grew {absent}"
        );
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    for required in [
        "leselang_command::validate_successor_pair(",
        "validate_bound_value(parent.continuation, value)",
        "SuccessorAdmissionError::Value(error) => error",
        "request_admission_fault(error)",
    ] {
        assert!(vm.contains(required), "{required}");
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/successor_pair_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("frozen_validate_effect_request(parent)?"));
    assert!(frozen.contains("frozen_validate_effect_request(next)?"));
    assert!(frozen.contains("assert_eq!(comparisons, 5_400)"));
    assert!(!frozen.contains("validate_bound_value("));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(
        consumer
            .contains("complete_successor_admission_rechecks_requests_before_completion_and_value")
    );
    assert!(consumer.contains(
        "complete_successor_admission_borrows_local_owners_and_preserves_each_native_unwind"
    ));
}

#[test]
fn complete_request_admission_is_one_shared_ordered_gate_without_native_authority() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/request_admission.rs"))
            .unwrap();
    for required in [
        "pub enum RequestAdmissionError",
        "pub fn validate_effect_request<Token>",
        "request: EffectRequestView<'_, Token>",
        "limits: ContinuationAdmissionLimits",
        "group_owner_text: impl FnOnce(&Token) -> &str",
        "validate_token: impl FnOnce(&Token) -> bool",
        "canonical_sequence: impl FnOnce(&Token) -> Option<u64>",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let ordered = adapter
        .split_once("pub fn validate_effect_request")
        .unwrap()
        .1;
    let stages = [
        "validate_continuation_frame(",
        ".map_err(RequestAdmissionError::Frame)?",
        "validate_preflighted_effect_request(",
    ]
    .map(|stage| ordered.find(stage).unwrap());
    assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    for absent in [
        "leselang_vm",
        "serde_json",
        "Fuel::",
        "Journal",
        ".clone()",
        "impl Default",
        "LSV",
        "canonical_source(",
    ] {
        assert!(!adapter.contains(absent), "request admission grew {absent}");
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(vm.contains("pub use leselang_command::reference_request::{"));
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    let admission = request
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn request_correspondence_fault(")
        .unwrap()
        .0;
    for required in [
        "crate::validate_effect_request(",
        "continuation_admission_limits()",
        "ContinuationToken::as_str",
        "valid_continuation_token,",
        "valid_merge_branch_name,",
        "canonical_continuation_sequence,",
        "RequestAdmissionError::Frame(error)",
        "RequestAdmissionError::Correspondence(error)",
    ] {
        assert!(admission.contains(required), "{required}");
    }
    assert!(!admission.contains("validate_image("));
    assert!(!admission.contains("validate_preflighted_effect_request("));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(frozen.contains(
        "crate::continuation_schema_tests::frozen_validate_image(&request.continuation)?"
    ));
    assert!(frozen.contains(
        "complete_request_admission_preserves_frozen_frame_before_correspondence_and_decode"
    ));
    assert!(frozen.contains("4_200"));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for proof in [
        "checked_request_admission_covers_seventy_operations_without_copying_original_frames",
        "checked_request_admission_keeps_frame_before_cold_authority_and_canonical_identity",
        "checked_request_admission_borrows_opaque_local_owners_in_original_hook_order",
        "checked_request_admission_propagates_each_native_unwind_and_drops_unused_hooks",
        "checked_request_admission_rechecks_mutable_data_without_promoting_live_grants",
    ] {
        assert!(proofs.contains(proof), "{proof}");
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test host_protocol"));
}

#[test]
fn successor_pair_correspondence_is_shared_after_native_request_and_value_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    for required in [
        "pub fn validate_preflighted_successor_pair<Token>",
        "pub enum SuccessorCorrespondenceError",
        "parent: EffectRequestView<'_, Token>",
        "limits: ResultBindingLimits",
        "token_text: impl Fn(&Token) -> &str",
        "parent.has_same_inherited_context(&next)",
        "limits.captures.max_saved_bindings",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm::",
        "Fuel::",
        "serde_json",
        ".clone()",
        "Journal",
        "impl Default",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let pair = source
        .split_once("pub(super) fn validate_pair(")
        .unwrap()
        .1
        .split_once("struct ReferenceCompletionOwner")
        .unwrap()
        .0;
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    for absent in [
        "let schema_valid",
        "first.fuel_remaining <=",
        "validate_extension(",
        "prior.validate(",
    ] {
        assert!(!pair.contains(absent), "duplicated {absent}");
    }
    assert!(pair.contains("SuccessorCorrespondenceError::Mismatch => invalid()"));
    assert!(pair.contains("SuccessorCorrespondenceError::InvalidBinding"));
    assert!(pair.contains("crate::result_binding::invalid()"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/successor_pair_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(pairs.len(), 9)"));
    assert!(frozen.contains("assert_eq!(comparisons, 576)"));
    assert!(!frozen.contains("validate_preflighted_successor_pair"));
    assert!(!frozen.contains("has_same_inherited_context"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    for proof in [
        "source_successor_pairs_share_wire_reentry_without_extra_fuel_or_preparation",
        "successor_pair_refuses_torn_rows_without_mutating_original_frames",
        "successor_pair_borrows_opaque_local_tokens_and_preserves_hook_order_and_unwind",
    ] {
        assert!(consumer.contains(proof));
    }
}

#[test]
fn group_requests_share_borrowed_correspondence_without_replacing_recovery_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_requests.rs")).unwrap();
    for required in [
        "pub struct GroupRequestContext",
        "pub struct InvalidGroupRequests",
        "pub fn validate_preflighted_group_requests",
        "Fn(&'a Request) -> EffectRequestView<'a, Token>",
        "pub fn preflighted_capture_accepts_type_at",
        "validate_result_binding(",
        "saved.group.matches_signature(branches)",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "leselang_vm::",
        "Fuel::",
        "serde_json",
        ".clone()",
        ".collect::<",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let admission = vm
        .split_once("impl ReferenceGroupResultBinding")
        .unwrap()
        .1
        .split_once("fn validate_requests(")
        .unwrap()
        .1
        .split_once("fn validate_member(")
        .unwrap()
        .0;
    assert!(
        admission.find("self.validate(plan)?").unwrap()
            < admission
                .find("validate_preflighted_group_requests(")
                .unwrap()
    );
    assert!(admission.contains("crate::canonical_continuation_sequence"));
    assert!(admission.contains(".map_err(|_| invalid())"));
    assert!(!admission.contains("let mut previous_fuel"));
    assert!(!vm.contains("fn capture_accepts_type_at("));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_request_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 2880)"));
    assert!(frozen.contains("assert_eq!(snapshots_count, 40)"));
    assert!(!frozen.contains("validate_preflighted_group_requests"));
    assert!(!frozen.contains("has_same_inherited_context"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(consumer.contains("leselang_command::validate_merge_graph("));
    assert!(consumer.contains("verify_requests(&active_plan, &requests, &fuel, &prepared)"));
}

#[test]
fn request_frame_correspondence_is_shared_without_weakening_checked_deserialization() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    for required in [
        "pub struct EffectRequestView<'a, Token>",
        "pub enum RequestCorrespondenceError",
        "pub fn validate_preflighted_effect_request<Token>",
        "canonical_sequence: impl FnOnce(&Token) -> Option<u64>",
        "&'a ContinuationFrame<Token, Revision, Type, Effect, ResultBinding>",
    ] {
        assert!(adapter.contains(required));
    }
    assert_eq!(adapter.matches("canonical_sequence(&").count(), 1);
    for absent in [
        "leselang_vm::",
        "Fuel::",
        "serde_json",
        ".clone()",
        "Journal",
    ] {
        assert!(!adapter.contains(absent));
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(!vm.contains("impl<'de> Deserialize<'de> for EffectRequest"));
    assert!(request.contains("impl<'de> Deserialize<'de> for EffectRequest"));
    assert!(request.contains("validate_effect_request(&request)"));
    assert!(vm.contains("validate_effect_request(&request)"));
    let admission = request
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn request_correspondence_fault(")
        .unwrap()
        .0;
    assert!(admission.contains("crate::validate_effect_request("));
    let complete =
        fs::read_to_string(root().join("crates/leselang-command/src/request_admission.rs"))
            .unwrap();
    let ordered = complete
        .split_once("pub fn validate_effect_request")
        .unwrap()
        .1;
    assert!(
        ordered.find("validate_continuation_frame(").unwrap()
            < ordered
                .find("validate_preflighted_effect_request(")
                .unwrap()
    );
    assert!(admission.contains("canonical_continuation_sequence,"));
    for absent in ["required_capabilities()", "format!", ".validate_request("] {
        assert!(!admission.contains(absent));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(!frozen.contains("validate_preflighted_effect_request"));
    assert!(frozen.contains(
        "request_correspondence_preserves_cold_authority_identity_and_budget_fault_order"
    ));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(consumer.contains("validate_effect_request("));
    assert!(!consumer.contains("validate_preflighted_effect_request("));
    assert!(consumer.contains(
        "source_atomic_requests_share_frame_correspondence_before_wire_restored_reentry"
    ));
}

#[test]
fn adapter_captured_execution_proof_reuses_shared_hir_without_vm_or_storage() {
    let source =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for required in [
        "leselang_hir::lower(&leselang_syntax::parse(source))",
        "evaluate_effects_in_scope(",
        "resume_preflighted_result_binding_in_scope(",
        "PreflightedResultEnvironment::new(",
        "impl ResultExecutionHost for Adapter",
        "validate_result_binding(",
        "validate_group_capture_correspondence(",
        "validate_result_binding_extension(",
        "validate_host_result(",
        "validate_continuation_frame(",
        "impl ResultValueHost for Adapter",
        "ResultValueDomain {",
        "into_effect_value(EffectValueContext",
        "prepare_preflighted_effect_in_scope(",
        "preparation_failure(error)",
        "scalar_copy_cost",
        "0..complete.spent",
        "locals.is_empty()",
        "adapter_executes_group_aliases_captured_successors_and_scalar_tail_without_vm",
        "adapter_reentry_and_wire_restoration_use_one_meter_at_every_fuel_boundary",
        "adapter_restores_all_five_closed_vocabularies_without_upgrading_successor_captures",
        "invalid_frames_cold_fields_replies_and_revoked_admission_never_spend_or_prepare",
        "scalar_recovery_and_conditional_successors_do_not_catch_native_projection_or_fuel_failures",
        "malformed_group_receipts_are_rejected_before_scope_restore_with_retryable_original_data",
        "computed_groups_share_product_preparation_through_wire_reentry_and_every_fuel_cutoff",
        "computed_group_failures_keep_cold_preflight_and_late_failure_boundaries",
        "preparation_borrows_admitted_host_payloads_and_redacts_non_debug_native_errors",
        "source_restored_frames_refuse_torn_admission_without_repair",
    ] {
        assert!(
            compact.contains(&required.split_whitespace().collect::<String>()),
            "adapter execution proof lost {required}"
        );
    }
    for absent in [
        "leselang_vm::",
        "rusqlite::",
        "apply_binary(",
        "apply_unary(",
    ] {
        assert!(
            !source.contains(absent),
            "adapter fixture duplicated or coupled {absent}"
        );
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(
        workflow
            .contains("cargo +1.98.0 test -p leselang-command --locked --test captured_execution")
    );
    let output = Command::new("cargo")
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "leselang-command",
            "--edges",
            "normal,build,dev",
            "--prefix",
            "none",
        ])
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dependencies = String::from_utf8(output.stdout).unwrap();
    for absent in ["leselang-vm ", "rusqlite ", "libsqlite3-sys "] {
        assert!(
            !dependencies.lines().any(|line| line.starts_with(absent)),
            "capture proof linked {absent}"
        );
    }
    for required in [
        "leselang-command ",
        "leselang-hir ",
        "leselang-runtime-core ",
        "leselang-syntax ",
    ] {
        assert!(dependencies.lines().any(|line| line.starts_with(required)));
    }
}

#[test]
fn product_execution_environment_shares_policy_and_mechanics_without_a_new_interpreter() {
    let source =
        fs::read_to_string(root().join("crates/leselang-command/src/execution_environment.rs"))
            .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for required in [
        "pub trait ResultExecutionHost: ResultCaptureHost",
        "pub struct ResultExecutionLimits",
        "host: &'host Host",
        "Host: ResultExecutionHost + ?Sized",
        "PureEvaluationEnvironment<ResultField, HostOperation>",
        "EffectEvaluationEnvironment<'value, ResultField, HostOperation, Effect, Type>",
        "self.host.preflight_effect(expression)",
        "self.host.before_prepare_effect(expression)?",
        "self.host.prepared_effect(&effect)?",
        "self.host.before_projection(*field)?",
        "result.project_scalar(*field)",
        "group.select_member(name, *operation)",
        "prepare_preflighted_effect_in_scope(expression, scope, self, fuel, self.limits.effects)",
        "snapshot_preflighted_result_scope(",
        "self.host.valid_group_name(name)",
        "self.host.preparation_failure(failure)",
        "self.host.capture_failure(failure)",
    ] {
        assert!(
            compact.contains(&required.split_whitespace().collect::<String>()),
            "shared environment lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "rusqlite",
        "LSV",
        "Fuel::new",
        "impl Default",
        "fuel.charge",
        "apply_binary(",
        "match expression",
        "evaluate_effects_in_scope(",
    ] {
        assert!(
            !source.contains(absent),
            "environment gained policy or interpreter {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let production = vm.split_once("#[cfg(test)]").unwrap().0;
    assert!(production.contains("crate::PreflightedResultEnvironment::new("));
    assert!(production.contains("impl crate::ResultExecutionHost for ReferenceCaptureHost"));
    for absent in [
        "struct ResultEnvironment",
        "impl PureEvaluationEnvironment",
        "EffectEvaluationEnvironment<'a",
        "prepare_preflighted_effect_in_scope(",
        "snapshot_preflighted_result_scope(",
    ] {
        assert!(
            !production.contains(absent),
            "VM still duplicates environment {absent}"
        );
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/capture_scope.rs")).unwrap();
    for required in [
        "shared_environment_borrows_one_unsized_host_for_recursive_group_preparation",
        "shared_environment_refusals_preserve_native_ownership_and_completed_work",
        "shared_environment_projection_policy_is_not_scalar_recovery",
        "shared_environment_unwind_keeps_the_original_meter_and_cleans_local_prefix",
        "dyn ResultExecutionHost<Error = Native>",
    ] {
        assert!(proof.contains(required));
    }
}

#[test]
fn capture_scope_bridge_is_adapter_owned_with_explicit_host_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_scope.rs")).unwrap();
    for required in [
        "pub trait ResultCaptureHost",
        "pub struct ResultCaptureLimits",
        "pub fn snapshot_preflighted_result_scope",
        "pub fn resume_preflighted_result_binding_in_scope",
        "snapshot_captured_scope_state(",
        "resume_captured_effects_in_scope(",
        "host.charge_scalar(value, fuel)?",
        "host.charge_result(&result, fuel)?",
        "host.charge_group(&group, fuel)?",
        "host.allow_saved_projection()?",
        "drop(validate_group_name)",
        "ResultView::Projected(result)",
        "ResultView::ProjectedGroup(group)",
    ] {
        assert!(adapter.contains(required), "capture bridge lost {required}");
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Journal",
        "Fuel::new",
        "impl Default",
        "Principal",
        "scalar_copy_cost",
        "fuel.charge",
        "apply_binary(",
    ] {
        assert!(
            !adapter.contains(absent),
            "capture bridge acquired host policy {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let production = vm.split_once("#[cfg(test)]").unwrap().0;
    for absent in [
        "CaptureSnapshotHooks",
        "CapturedEffectHooks",
        "snapshot_captured_scope_state(",
        "resume_captured_effects_in_scope(",
        "CapturedValue::",
    ] {
        assert!(
            !production.contains(absent),
            "VM duplicated capture bridge {absent}"
        );
    }
    for required in [
        "impl crate::ResultCaptureHost",
        "charge(fuel, 1 + string_cost(value))",
        "if self.restore_saved_results",
        "view_fault(error)",
    ] {
        assert!(
            production.contains(required),
            "VM lost native policy {required}"
        );
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/capture_scope.rs")).unwrap();
    for required in [
        "snapshot_joins_mixed_views_with_explicit_accounting_and_closed_saved_bytes",
        "snapshot_preflight_and_projection_refusal_preserve_native_charge_order",
        "reentry_borrows_original_projections_and_preserves_parent_scope_and_wire",
        "reentry_refusals_keep_row_order_native_payloads_and_duplicate_cleanup",
        "native_unwind_never_refills_retries_or_leaks_restored_rows",
        "predicate_cleanup_unwind_drops_the_locally_owned_native_error_before_handoff",
    ] {
        assert!(proof.contains(required));
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test captured_execution --test capture_scope"));
}

#[test]
fn product_request_preparation_is_adapter_owned_over_one_hir_executor() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_preparation.rs"))
            .unwrap();
    for required in [
        "pub enum EffectPreparationError<Native>",
        "pub fn prepare_preflighted_effect_in_scope",
        "limits: EffectEvaluationLimits",
        "evaluate_effects_in_scope(",
        ".evaluate_computed_arguments(",
        "operation.resolve(&values)",
        "Cow::Borrowed(effect)",
        "branch.result_type != operation.result_type()",
    ] {
        assert!(adapter.contains(required), "adapter lost {required}");
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Journal",
        "serde",
        "Fuel::new",
        "impl Default",
        "apply_binary(",
        "apply_unary(",
    ] {
        assert!(!adapter.contains(absent), "preparation acquired {absent}");
    }
    let source =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let production = source.split_once("#[cfg(test)]").unwrap().0;
    assert!(production.contains("crate::PreflightedResultEnvironment::new("));
    assert!(
        production
            .contains("EffectPreparationError::Evaluation(failure) => effect_failure(failure)")
    );
    assert!(production.contains("call_failure(failure, operation)"));
    assert!(production.contains("diagnostic_code"));
    for absent in [
        ".evaluate_computed_arguments(",
        "operation.resolve(&values)",
        "resolved.push(HirBranch",
    ] {
        assert!(!production.contains(absent), "VM still owns {absent}");
    }
    let baseline = fs::read_to_string(
        root()
            .join("crates/leselang-command/src/reference_computation/effect_preparation_tests.rs"),
    )
    .unwrap();
    assert!(
        baseline
            .contains("effect_preparation_preserves_frozen_requests_fuel_fault_order_and_scope")
    );
    assert!(baseline.contains("for grant in 0..160"));
}

#[test]
fn product_result_views_are_adapter_owned_with_vm_budget_and_fault_mapping() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/result_view.rs")).unwrap();
    for required in [
        "pub enum ResultView<'a>",
        "pub enum ResultViewError",
        "pub fn project_scalar(",
        "pub fn select_member(",
        "pub fn capture_result(",
        "pub fn capture_group(",
        "limits: GroupCaptureLimits",
        "validate_name: impl FnMut(&str) -> bool",
        "member.name == name && member.result.operation == operation",
        "Self::Projected(result) => Ok(result.clone())",
        "Self::ProjectedGroup(group) => Ok(group.clone())",
    ] {
        assert!(adapter.contains(required), "result view lost {required}");
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "serde",
        "MAX_MERGE_BRANCHES",
        "Principal",
        "impl Default",
    ] {
        assert!(!adapter.contains(absent), "result view acquired {absent}");
    }
    let source =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let production = source.split_once("#[cfg(test)]").unwrap().0;
    for required in [
        "use crate::ResultView;",
        "crate::PreflightedResultEnvironment::new(",
        "ResultViewError::InvalidAccess => invalid()",
        "ResultViewError::InvalidProjection => invalid_projection()",
        "error(\"LSV1402\"",
        "resume_preflighted_result_binding_in_scope(",
        "impl crate::ResultCaptureHost",
        "charge_projection(fuel, result)",
        "charge_group_projection(fuel, group)",
    ] {
        assert!(production.contains(required), "VM lost {required}");
    }
    for absent in [
        "enum ResultView",
        "ProjectedResult::capture(",
        "ProjectedGroup::capture(",
        "member.name == name",
        "field.name == name",
        "result.field(field)",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    let baseline = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_computation/result_view_tests.rs"),
    )
    .unwrap();
    assert!(
        baseline.contains(
            "result_views_preserve_frozen_projection_selection_copy_and_fault_boundaries"
        )
    );
    let matrix =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(matrix.contains(
        "adapter_result_view_lifecycle_covers_seventy_operations_and_five_saved_vocabularies"
    ));
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/result_view.rs")).unwrap();
    for proof in [
        "views_borrow_original_members_and_copy_only_at_explicit_projection_boundaries",
        "views_preserve_saved_bytes_and_first_match_without_claiming_admission",
        "raw_capture_policy_keeps_projection_before_names_and_propagates_native_unwind",
        "diagnostics_are_payload_free_and_legacy_fields_are_never_filled_from_raw_catalogs",
    ] {
        assert!(tests.contains(proof));
    }
}

#[test]
fn product_capture_replay_policy_is_adapter_owned_without_moving_authority_or_fault_codes() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_replay.rs")).unwrap();
    for required in [
        "pub enum CaptureReplayError",
        "pub fn validate_result_binding_extension(",
        "pub fn validate_group_capture_correspondence(",
        "max_saved_bindings: usize",
        "limits: CaptureCorrespondenceLimits",
        "group_limits: GroupCaptureLimits",
        "validate_group_name: impl FnMut(&str) -> bool",
        "fields.len() != operations.len()",
    ] {
        assert!(adapter.contains(required));
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "serde",
        ".clone()",
        "MAX_EFFECT_NESTING_DEPTH",
        "MAX_MERGE_BRANCHES",
        "Continuation",
        "Principal",
    ] {
        assert!(
            !adapter.contains(absent),
            "capture adapter acquired {absent}"
        );
    }
    let successor = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let successor = successor
        .split_once("#[cfg(test)]\nmod extension_tests {")
        .unwrap()
        .0;
    assert!(!successor.contains("impl CaptureExtensionDomain"));
    assert!(!successor.contains("validate_capture_extension("));
    assert!(successor.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    assert!(correspondence.contains("validate_result_binding_extension("));
    assert!(successor.contains("crate::result_binding::invalid()"));
    assert!(successor.contains("code: \"LSV1407\""));
    let group = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let group = group.split_once("#[cfg(all(test").unwrap().0;
    assert!(!group.contains("validate_capture_correspondence("));
    assert!(group.contains("leselang_command::reference_group_binding::validate_saved_capture("));
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_binding.rs"))
            .unwrap();
    assert!(policy.contains("crate::validate_group_capture_correspondence("));
    assert!(policy.contains("CaptureReplayError::InvalidProjection => invalid_projection()"));
    assert!(policy.contains("continuation_admission_fault(ContinuationAdmissionError::Binding)"));
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(group.contains("leselang_command::reference_merge::invalid_group_binding as invalid"));
    let merge =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_merge.rs")).unwrap();
    let invalid = merge
        .split_once("pub fn invalid_group_binding()")
        .unwrap()
        .1
        .split_once("fn valid_merge_branch_name(")
        .unwrap()
        .0;
    assert!(invalid.contains("continuation_admission_fault(ContinuationAdmissionError::Schema("));
    assert!(invalid.contains("continuation_schema::ContinuationSchemaError::Group"));
    assert!(request.contains("code: \"LSV1409\""));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/capture_replay.rs")).unwrap();
    for proof in [
        "extension_keeps_aliases_named_retention_and_distinct_late_projection_errors",
        "correspondence_keeps_ordered_groups_full_row_scans_and_error_categories",
        "caller_limits_and_row_shapes_precede_callbacks_without_implicit_vm_defaults",
        "name_callback_refusal_and_unwind_keep_prefix_order_without_mutating_captures",
    ] {
        assert!(proofs.contains(proof));
    }
}

#[test]
fn complete_product_binding_validation_is_adapter_owned_with_explicit_vm_limits() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/result_binding.rs")).unwrap();
    for required in [
        "pub type ResultBinding =",
        "pub struct ResultBindingLimits",
        "pub fn validate_result_binding_structure(",
        "pub fn validate_result_binding(",
        "captures: CaptureValidationLimits",
        "groups: GroupCaptureLimits",
        "accesses: ProjectionAccessLimits",
        "validate_group_name: impl FnMut(&str) -> bool",
        "validate_captured_bindings(",
        "validate_captured_projection_accesses(",
        "validate_in_group_scope(&scope, &groups)",
        "validate_in_scope(&[])",
        "binding.locals.iter().rev()",
    ] {
        assert!(
            adapter.contains(required),
            "binding adapter lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "CONTINUATION_SCHEMA_VERSION",
        "impl Default",
        "MAX_MERGE_BRANCHES",
    ] {
        assert!(
            !adapter.contains(absent),
            "binding adapter acquired {absent}"
        );
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    for required in [
        "pub use leselang_command::{",
        "ResultBinding,",
        "fn schema_version(&self)",
        "leselang_command::continuation_schema::preflighted_result_binding_schema_version(self)",
        "leselang_command::validate_result_binding(",
        "leselang_command::validate_result_binding_structure(",
        "result_binding_limits()",
        "crate::valid_merge_branch_name",
        ".map_err(projection_fault)",
    ] {
        assert!(production.contains(required));
    }
    for absent in [
        "pub type ResultBinding",
        "validate_captured_bindings(",
        "GroupLocalType",
        "validate_in_group_scope(",
        "validate_in_scope(",
        "Computation::Bind",
        "self.body.clone()",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    assert!(
        reference
            .contains("restored_binding_keeps_scalar_result_group_types_and_canonical_refusals")
    );
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/result_binding.rs")).unwrap();
    for proof in [
        "complete_validation_keeps_scalar_literal_scope_and_typed_saved_scope_paths",
        "complete_validation_rejects_legacy_widening_names_and_forged_group_types",
        "explicit_limits_and_ordered_name_hooks_precede_access_and_canonical_work",
    ] {
        assert!(proofs.contains(proof));
    }
}

#[test]
fn group_plan_correspondence_is_adapter_owned_between_structure_and_deep_vm_gates() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_plan.rs")).unwrap();
    for required in [
        "pub struct GroupPlanLimits",
        "pub struct InvalidGroupPlan",
        "pub fn plan_branches(",
        "pub fn plan_successor_name_at(",
        "pub fn validate_preflighted_plan(",
        "limits.max_branches",
        "limits.max_effect_sequence",
        "limits.max_fuel",
        "HostOperation::for_effect(&branch.effect)",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "leselang_vm",
        "Journal",
        "EffectRequest",
        "Fuel::",
        "LSV",
        "MAX_MERGE_BRANCHES",
        "MAX_EFFECT_SEQUENCE",
        "MAX_EXECUTION_FUEL",
        "impl Default",
        "serde",
        ".clone()",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let implementation = reference
        .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
        .unwrap()
        .1;
    let validate = implementation
        .split_once("fn validate(")
        .unwrap()
        .1
        .split_once("fn validate_requests(")
        .unwrap()
        .0;
    assert!(
        validate.contains("leselang_command::reference_merge::validate_group_binding(self, plan)")
    );
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_merge.rs")).unwrap();
    for helper in [
        "max_branches: MAX_MERGE_BRANCHES",
        "max_effect_sequence: MAX_EFFECT_SEQUENCE",
        "max_fuel: MAX_EXECUTION_FUEL",
        "binding: result_binding_limits()",
        "bounded_json::measure(plan, MAX_CONTINUATION_BYTES)",
        "continuation_admission_fault(ContinuationAdmissionError::Binding)",
        "GroupBindingError::Plan => invalid_group_binding()",
        "GroupBindingError::Encoding(fault) => fault",
    ] {
        assert!(policy.contains(helper), "{helper}");
    }
    for forbidden in [
        "HostOperation::",
        "atomic_flow_bound()",
        "reserved_sequences()",
        "ExecutionOrder::",
    ] {
        assert!(
            !validate.contains(forbidden),
            "duplicated policy: {forbidden}"
        );
    }
    assert!(
        implementation.contains("self.plan_successor_name_at(position, crate::MAX_MERGE_BRANCHES)")
    );
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_plan_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 3360)"));
    assert!(!frozen.contains("validate_preflighted_plan"));
    assert!(!frozen.contains("plan_successor_name_at"));
    let vertical =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(vertical.contains(
        "fn adapter_saved_group_plan_gates_source_reentry_without_vm_or_resource_changes()"
    ));
    assert!(vertical.contains("leselang_command::validate_merge_plan("));
    assert!(vertical.contains("leselang_command::validate_merge_graph("));
    assert!(!vertical.contains("leselang_vm"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("GroupPlanLimits"));
    assert!(docs.contains("group-plan correspondence is not recovery admission"));
}

#[test]
fn complete_group_binding_admission_is_shared_with_explicit_native_encoding_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_admission.rs")).unwrap();
    for required in [
        "pub struct GroupBindingLimits",
        "pub enum GroupBindingError<Encoding>",
        "pub fn validate_group_binding<Encoding>",
        "validate_encoding: impl FnOnce(&MergePlan) -> Result<(), Encoding>",
        "mut validate_group_name: impl FnMut(&str) -> bool",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let entry = adapter
        .split_once("pub fn validate_group_binding<Encoding>")
        .unwrap()
        .1;
    let mut offset = 0;
    for stage in [
        "validate_result_binding_structure(",
        ".map_err(|_| GroupBindingError::Binding)?",
        ".validate_preflighted_plan(plan, limits.plan)",
        "validate_encoding(plan).map_err(GroupBindingError::Encoding)?",
        "let ty = validate_result_binding(",
        "!matches!(ty, Type::Scalar(_))",
    ] {
        offset += entry[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    for forbidden in [
        "leselang_vm",
        "Journal",
        "EffectRequest",
        "Fuel::",
        "MAX_",
        "serde",
        ".clone()",
        "impl Default",
        "LSV",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let vertical =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    let admit = vertical
        .split_once("fn admit(plan: &MergePlan)")
        .unwrap()
        .1
        .split_once("for mode in")
        .unwrap()
        .0;
    assert!(admit.contains("leselang_command::validate_merge_plan("));
    assert!(admit.contains("plan_policy()"));
    assert!(!admit.contains("validate_result_binding_structure("));
    assert!(!admit.contains("validate_result_binding("));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("GroupBindingLimits"));
    assert!(docs.contains("native encoding errors move unchanged"));
}

#[test]
fn structured_plan_protocol_and_shape_admission_are_adapter_owned_with_deep_vm_gates() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/merge_plan.rs")).unwrap();
    for required in [
        "pub struct MergePlan",
        "pub enum ExecutionOrder",
        "pub struct MergePlanShapeLimits",
        "pub enum MergePlanShapeError",
        "pub fn validate_shape(",
        "pub fn valid_merge_branch_name(",
        "pub fn new(",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "Journal",
        "EffectRequest",
        "Fuel::",
        "binding.validate(",
        "validate_merge_plan(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let constructor = adapter
        .split_once("pub fn new(")
        .unwrap()
        .1
        .split_once("/// Explicit shape policy")
        .unwrap()
        .0;
    assert!(
        constructor.find(".collect()").unwrap() < constructor.find("plan.validate_shape(").unwrap()
    );
    assert!(constructor.contains("code: \"LSV2401\""));
    let shape = adapter.split_once("pub fn validate_shape(").unwrap().1;
    assert!(!shape.contains("LSV"));
    assert!(!shape.contains("MAX_MERGE_BRANCHES"));
    assert!(!shape.contains("MAX_MERGE_BRANCH_NAME_BYTES"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for absent in [
        "pub struct MergePlan",
        "pub enum ExecutionOrder",
        "impl MergePlan",
        "pub const MAX_MERGE_BRANCHES",
        "pub const MAX_MERGE_BRANCH_NAME_BYTES",
    ] {
        assert!(!vm.contains(absent), "VM duplicated {absent}");
    }
    assert!(vm.contains("pub(crate) use leselang_command::reference_merge::validate_merge_plan;"));
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_merge.rs")).unwrap();
    let validation = policy
        .split_once("pub fn validate_merge_plan(")
        .unwrap()
        .1
        .split_once("fn valid_merge_branch_name(")
        .unwrap()
        .0;
    for stage in [
        "crate::validate_merge_plan(",
        "merge_plan_admission_limits()",
        "validate_plan_encoding",
        ".map_err(merge_plan_admission_fault)",
        "MergePlanShapeError::GroupBinding => invalid_group_binding()",
        "MergePlanShapeError::Successor =>",
        "RequestCorrespondenceError::SuccessorAuthority",
        "code: \"LSV2401\"",
    ] {
        assert!(validation.contains(stage), "{stage}");
    }
    for absent in ["HashSet", "ExecutionOrder::", "minimum"] {
        assert!(!validation.contains(absent));
    }
    assert!(
        vm.contains("leselang_command::valid_merge_branch_name(name, MAX_MERGE_BRANCH_NAME_BYTES)")
    );
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/merge_plan_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("struct FrozenMergePlan"));
    assert!(frozen.contains("enum FrozenExecutionOrder"));
    assert!(frozen.contains("assert_eq!(comparisons, 1120)"));
    assert!(!frozen.contains("validate_shape"));
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test result_binding --test merge_plan"));
}

#[test]
fn group_continuation_data_and_successor_versions_are_product_owned_not_vm_owned() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_continuation.rs"))
            .unwrap();
    for required in [
        "pub struct GroupResultBinding",
        "#[serde(deny_unknown_fields)]",
        "pub fn is_conditional(",
        "pub fn is_dataflow(",
        "pub fn captures_successor(",
        "pub fn reserved_sequences(",
        "pub fn continuation_schema(",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "Journal",
        "Fuel::",
        "LSV",
        "MAX_EFFECT_SEQUENCE",
        "MAX_EXECUTION_FUEL",
        "EffectRequest",
        "MergePlan",
        ".clone()",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    assert!(reference.contains("pub use leselang_command::GroupResultBinding;"));
    assert!(reference.contains("pub(super) trait ReferenceGroupResultBinding"));
    assert!(reference.contains("impl ReferenceGroupResultBinding for GroupResultBinding"));
    assert!(!reference.contains("pub struct GroupResultBinding"));
    let production = reference.split_once("#[cfg(all(test").unwrap().0;
    for required in [
        "leselang_command::reference_merge::validate_group_binding(self, plan)",
        "leselang_command::reference_merge::invalid_group_binding as invalid",
        "leselang_command::reference_group_execution::advance_capture(",
        "crate::successor::validate_pair(",
    ] {
        assert!(production.contains(required), "{required}");
    }
    assert!(reference.contains("#[cfg(test)]\n    fn validate_requests("));
    let shared_graph =
        fs::read_to_string(root().join("crates/leselang-command/src/merge_admission.rs")).unwrap();
    assert!(shared_graph.contains("validate_preflighted_group_requests("));
    for absent in [
        "fn is_conditional(",
        "fn is_dataflow(",
        "fn captures_successor(",
        "fn continuation_schema(",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    let successor = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let pair = successor
        .split_once("pub(super) fn validate_pair(")
        .unwrap()
        .1
        .split_once("struct ReferenceCompletionOwner")
        .unwrap()
        .0;
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let shared =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    let mut offset = 0;
    for stage in [
        "let schema_valid = successor_schema_matches(",
        "if !schema_valid",
        "parent.has_same_inherited_context(&next)",
        "first.fuel_remaining <= second.fuel_remaining",
        "canonical_sequence(&first.token)",
        "validate_result_binding_extension(",
    ] {
        offset += shared[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    assert!(!pair.contains("match first.schema_version"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_protocol_tests.rs")).unwrap();
    assert!(frozen.contains("struct FrozenGroupResultBinding"));
    assert!(frozen.contains("assert_eq!(comparisons, 288)"));
    assert!(!frozen.contains("leselang_command"));
}

#[test]
fn continuation_schema_policy_is_adapter_owned_after_structure_preflight() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/continuation_schema.rs"))
            .unwrap();
    for required in [
        "pub enum ContinuationSchemaError",
        "pub fn preflighted_result_binding_schema_version(",
        "pub fn validate_preflighted_continuation_schema(",
        "max_sequence: u64",
        "binding: Option<&ResultBinding>",
        "GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION: u32 = 11",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "Journal",
        "LSV",
        "MAX_EFFECT_SEQUENCE",
        "impl Default",
        "pub struct ContinuationImage",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(reference.contains("pub use leselang_command::continuation_schema::{"));
    assert!(!reference.contains("pub const CONTINUATION_SCHEMA_VERSION"));
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(reference.contains("validate_continuation_image as validate_image"));
    let validation = request
        .split_once("pub fn validate_continuation_image(")
        .unwrap()
        .1
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .0;
    let admission =
        fs::read_to_string(root().join("crates/leselang-command/src/continuation_admission.rs"))
            .unwrap();
    let ordered = admission
        .split_once("pub fn validate_continuation_frame")
        .unwrap()
        .1;
    let mut offset = 0;
    for step in [
        "validate_result_binding_structure(",
        "validate_preflighted_continuation_schema(",
        "image.result_binding.as_deref()",
        "image.group_result.as_ref().map(group_owner_text)",
        "validate_token(&image.token)",
        "image.program_counter != 1",
        "HostOperation::for_effect(",
        "validate_result_binding(",
        ".resource_budget()",
    ] {
        offset += ordered[offset..]
            .find(step)
            .unwrap_or_else(|| panic!("missing/out-of-order {step}"))
            + step.len();
    }
    for required in [
        "crate::validate_continuation_frame(",
        "result_binding_limits()",
        "max_group_sequence: MAX_EFFECT_SEQUENCE",
        "ContinuationToken::as_str",
        "valid_continuation_token,",
        "valid_merge_branch_name,",
        "\"LSV1409\"",
        "ContinuationSchemaError::Version { .. })",
        "\"LSV2001\"",
    ] {
        assert!(validation.contains(required), "reference lost {required}");
    }
    for absent in [
        "strip_prefix(\"merge-\")",
        "binding.schema_version()",
        "binding.body.is_pure()",
        "expected_version",
        "image.program_counter != 1",
        "HostOperation::for_effect(",
        "image.result_type !=",
    ] {
        assert!(!validation.contains(absent), "VM duplicated {absent}");
    }
    let codec =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_continuation.rs"))
            .unwrap();
    let restore = codec
        .split_once("fn validate_restorable_image(")
        .unwrap()
        .1
        .split_once("fn validate_continuation_encoding_size(")
        .unwrap()
        .0;
    let stages = [
        "validate_continuation_image(image)?",
        "canonical_continuation_sequence(",
        "validate_pending_effect_contract(image)",
    ]
    .map(|stage| restore.find(stage).unwrap());
    assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-vm/src/continuation_schema_tests.rs"))
            .unwrap();
    assert!(
        proofs.contains("continuation_schema_preserves_frozen_selection_and_frame_refusal_order")
    );
    assert!(proofs.contains("continuation_structure_precedes_schema_even_for_oversized_body"));
    assert!(proofs.contains("22_848"));
    assert!(
        proofs.contains(
            "frame_admission_preserves_all_operations_budget_edges_and_conflicting_faults"
        )
    );
    assert!(proofs.contains("1_260"));
    assert!(!proofs.contains("leselang_command::continuation_schema"));
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test host_protocol --test result_binding"));
    assert!(workflow.contains("--test continuation_admission"));
    for absent in [
        "leselang_vm",
        "serde_json",
        "Journal",
        "Fuel::",
        "impl Default",
        "LSV",
    ] {
        assert!(
            !admission.contains(absent),
            "shared admission grew {absent}"
        );
    }
}

#[test]
fn product_capture_access_policy_is_adapter_owned_without_bypassing_vm_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/projection_access.rs"))
            .unwrap();
    for required in [
        "pub fn validate_captured_projection_accesses(",
        "limits: ProjectionAccessLimits",
        "with_captured_scope(",
        "observe_projection_accesses(",
        "left & right",
        "self.call(operation)",
        "member.result.operation == *operation",
    ] {
        assert!(
            adapter.contains(required),
            "product access policy lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "serde",
        ".clone()",
        "MAX_EFFECT_NESTING_DEPTH",
        "MAX_COMPUTATION_NODES",
    ] {
        assert!(
            !adapter.contains(absent),
            "product access policy acquired {absent}"
        );
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    let binding =
        fs::read_to_string(root().join("crates/leselang-command/src/result_binding.rs")).unwrap();
    assert!(production.contains("leselang_command::validate_result_binding("));
    let composed = binding
        .split_once("pub fn validate_result_binding(")
        .unwrap()
        .1;
    let delegated = composed
        .find("validate_captured_projection_accesses(")
        .unwrap();
    assert!(composed.find("validate_result_binding_structure(").unwrap() < delegated);
    assert!(
        delegated
            < composed
                .find("validate_canonical_binding_type(binding, pending)")
                .unwrap()
    );
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(production.contains("leselang_command::reference_request::result_binding_limits()"));
    assert!(production.contains(".map_err(projection_fault)"));
    assert!(production.contains("leselang_command::ContinuationAdmissionError::Binding"));
    for retained in [
        "max_nodes: leselang_hir::computation::MAX_COMPUTATION_NODES",
        "max_depth: MAX_EFFECT_NESTING_DEPTH",
        "LSV1405",
    ] {
        assert!(request.contains(retained));
    }
    for absent in [
        "fn field_bit(",
        "fn operation_fields(",
        "AvailableProjection",
        "ProjectionAccessPolicy",
        "with_captured_scope(",
        "fn validate_projected_accesses(",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/projection_access.rs"))
            .unwrap();
    for proof in [
        "closed_fields_are_checked_in_cold_recovery_loops_folds_and_call_arguments",
        "capture_names_share_one_namespace_and_lexical_siblings_do_not_leak",
        "saved_members_require_exact_names_and_operations_but_fallback_is_not_type_admission",
        "explicit_physical_walk_limits_are_inclusive_and_cover_unselected_branches",
    ] {
        assert!(proofs.contains(proof));
    }
    let vm_proofs =
        fs::read_to_string(root().join("crates/leselang-vm/tests/projection_scope.rs")).unwrap();
    assert!(
        vm_proofs
            .contains("adapter_access_success_never_bypasses_vm_schema_or_canonical_group_checks")
    );
}

#[test]
fn product_group_capture_contract_is_adapter_owned_with_explicit_vm_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/projected_group.rs")).unwrap();
    assert!(adapter.contains("impl ProjectedGroupExt for ProjectedGroup"));
    assert!(adapter.contains("limits: GroupCaptureLimits"));
    assert!(adapter.contains("validate_name: impl FnMut(&str) -> bool"));
    assert!(adapter.contains(
        "validate_projected_group(self, limits, validate_name, ProjectedResult::validate)"
    ));
    let matching = adapter.rsplit_once("fn matches_members(").unwrap().1;
    assert!(
        matching
            .find("self.validate(limits, validate_name)?")
            .unwrap()
            < matching.find("self.matches_signature(branches)").unwrap()
    );
    for absent in [
        "leselang_vm",
        "MAX_MERGE",
        "Fuel",
        "LSV",
        "Journal",
        "serde",
        "pub struct ProjectedGroup",
    ] {
        assert!(!adapter.contains(absent), "adapter acquired {absent}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    assert!(production.contains(
        "#[cfg(test)]\npub(super) use leselang_command::ProjectedGroupExt as ReferenceProjectedGroup;"
    ));
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(production.contains("result_binding_limits().groups"));
    assert!(production.contains("leselang_command::reference_request::result_binding_limits()"));
    assert!(request.contains("max_members: crate::MAX_MERGE_BRANCHES - 1"));
    assert!(request.contains("min_members: 1"));
    assert!(production.contains("crate::valid_merge_branch_name"));
    assert!(!production.contains("impl ReferenceProjectedGroup"));
    assert!(!production.contains("fn matches_members("));
    assert!(!production.contains("fn matches_signature("));
    assert!(reference.contains("group_capture_preserves_original_limits_names_and_refusal_order"));
    assert!(
        reference.contains(
            "group_capture_keeps_legacy_versions_order_and_projection_only_correspondence"
        )
    );
    let cases =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(cases.contains(
        "group_captures_cover_seventy_operations_with_mixed_versions_and_exact_member_order"
    ));
    assert!(cases.contains(
        "group_capture_policy_is_explicit_and_name_hooks_keep_original_validation_order"
    ));
}

#[test]
fn product_scalar_projection_is_adapter_owned_without_moving_execution_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/scalar_projection.rs"))
            .unwrap();
    assert!(
        adapter.contains("pub fn project_scalar(&self, field: ResultField) -> Option<ScalarValue>")
    );
    for absent in [
        "serde_json",
        "Fault",
        "Fuel",
        "Journal",
        "ProjectedResult",
        "projection_version",
        "leselang_vm",
    ] {
        assert!(!adapter.contains(absent), "adapter acquired {absent}");
    }
    let binding =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    assert!(!binding.contains("pub(super) fn project("));
    let view =
        fs::read_to_string(root().join("crates/leselang-command/src/result_view.rs")).unwrap();
    assert!(view.contains(".project_scalar(field)"));
    assert!(view.contains(".ok_or(ResultViewError::InvalidProjection)"));
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(request.contains("ContinuationAdmissionError::Binding => \"LSV1405\""));
    for retained in [
        "leselang_command::validate_result_binding_structure(",
        ".map_err(projection_fault)",
        "leselang_command::reference_request::continuation_admission_fault(",
        "leselang_command::ContinuationAdmissionError::Binding",
    ] {
        assert!(
            binding.contains(retained),
            "VM lost capture policy: {retained}"
        );
    }
    let computation =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    assert!(computation.contains("self.project_scalar(field).map_err(view_fault)"));
    assert!(
        computation.contains("fn charge_projection(fuel: &mut Fuel, result: &ProjectedResult)")
    );
    for test in [
        "scalar_projection_keeps_five_capture_versions_and_frozen_field_wire",
        "scalar_projection_keeps_invalid_frame_fault_and_capture_correspondence",
    ] {
        assert!(binding.contains(test));
    }
    let cases =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(
        cases.contains(
            "scalar_projection_covers_all_seventy_seven_types_and_twelve_explicit_fields"
        )
    );
    assert!(cases.contains("scalar_projection_is_not_bounds_validation_or_execution_admission"));
}

#[test]
fn product_capture_frames_and_validation_live_in_adapter_with_exact_reference_reexports() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/projected_result.rs")).unwrap();
    for required in [
        "pub struct ProjectedResult",
        "pub struct InvalidProjectedResult",
        "pub type ProjectedField = ScalarProjectionField<ResultField>",
        "validate_scalar_projection(&self.fields, expected)",
        "projection_fields(self.projection_version)",
        "valid_kind_token(self.operation, &stored.value)",
        "default = \"legacy_projection_version\"",
        "skip_serializing_if = \"is_legacy_projection\"",
    ] {
        assert!(adapter.contains(required), "adapter lost {required}");
    }
    for absent in [
        "leselang_vm",
        "Fault",
        "LSV",
        "Journal",
        "Fuel",
        "rusqlite",
        "ContinuationImage",
    ] {
        assert!(
            !adapter.contains(absent),
            "product capture acquired execution policy: {absent}"
        );
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    assert!(reference.contains("pub use leselang_command::{"));
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    for absent in [
        "pub struct ProjectedResult",
        "impl ProjectedResult",
        "fn projection_fields",
        "fn valid_kind_token",
        "serde",
        "UiSemantic",
        "UiFormInputKind",
    ] {
        assert!(!production.contains(absent), "VM retained {absent}");
    }
    assert!(
        reference
            .contains("projection_fault(_: leselang_command::InvalidProjectedResult) -> Fault")
    );
    assert!(reference.contains("leselang_command::validate_result_binding_structure("));
    assert!(reference.contains("leselang_command::validate_result_binding("));
    let cases =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(cases.contains(
        "product_captures_preserve_seventy_operations_five_versions_and_exact_wire_order"
    ));
    assert!(
        cases.contains(
            "product_capture_shape_never_certifies_raw_identity_or_trusted_restore_version"
        )
    );
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/tests/capture_contract.rs")).unwrap();
    assert!(proof.contains(
        "product_capture_reexports_are_exact_adapter_types_without_conversion_or_buffer_copies"
    ));
}

#[test]
fn continuation_type_validation_reuses_host_signatures_without_owning_product_mapping() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/continuation_admission.rs"))
            .unwrap();
    let validation = adapter
        .split_once("pub fn validate_continuation_frame")
        .unwrap()
        .1;
    assert_eq!(
        validation
            .matches("HostOperation::for_effect(&image.pending_effect)")
            .count(),
        1
    );
    assert!(validation.contains(".map(|operation| operation.result_type())"));
    for absent in [
        "Effect::",
        "Type::",
        ".clone()",
        "lower_effect(",
        "canonical_source(",
    ] {
        assert!(
            !validation.contains(absent),
            "frame validator acquired {absent}"
        );
    }
    let stages = [
        "image.program_counter != 1",
        "HostOperation::for_effect(",
        "image.result_type != expected_type",
        "validate_result_binding(",
        ".resource_budget()",
    ]
    .map(|stage| validation.find(stage).unwrap());
    assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-vm/src/continuation_contract_tests.rs"))
            .unwrap();
    for proof in [
        "continuation_signatures_keep_all_seventy_wire_and_restore_round_trips",
        "continuation_signatures_reject_every_cross_operation_and_non_host_result_type",
        "continuation_type_refusals_do_not_mutate_live_state_or_advance_identity",
        "continuation_signature_faults_keep_frame_before_type_before_budget_order",
        "continuation_signature_lookup_does_not_admit_structured_or_compute_frames",
        "continuation_signature_lookup_leaves_canonical_validation_at_the_restore_boundary",
        "continuation_signatures_reopen_all_operations_and_refuse_tampered_journal_types",
    ] {
        assert!(proofs.contains(proof));
    }
    assert!(!proofs.contains("HostOperation::"));
}

#[test]
fn control_plane_reply_identity_rules_are_adapter_owned_but_context_fences_stay_in_vm() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/control_plane_reply.rs"))
            .unwrap();
    for required in [
        "pub fn matches_control_plane_effect(",
        "saved_operation: Option<&EffectOperation>",
        "result.command_id == command.command_id",
        "result.status == CommandStatus::Applied",
        "result.observed_at_ms <= i64::MAX as u64",
    ] {
        assert!(adapter.contains(required));
    }
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        ".clone()",
        "serde_json",
        "Type::",
        "Value::",
        "LSV210",
        "max_output_items",
        "expected_revision",
        "canonical_source(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed reply checker acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let projection = vm
        .split_once("fn atomic_step_from_effect_result(")
        .unwrap()
        .1
        .split_once("fn fault(code:")
        .unwrap()
        .0;
    for absent in [
        "Effect::",
        "CommandStatus::",
        "command.command_id",
        "entries.iter().all",
        "result.session_id",
        "observed_at_ms <=",
    ] {
        assert!(
            !projection.contains(absent),
            "VM retained reply product predicate {absent}"
        );
    }
    assert!(projection.contains("reference_result::atomic_step_from_effect_result("));
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    assert_eq!(
        projection
            .matches("self.matches_control_plane_effect(context.effect, context.operation)")
            .count(),
        1
    );
    let signature = projection.find("HostOperation::for_effect(").unwrap();
    let correspondence = projection
        .find("self.matches_control_plane_effect(")
        .unwrap();
    let conversion = projection
        .find("self.into_control_plane_value(context.result_type)")
        .unwrap();
    let revision = projection.find("expected != revision").unwrap();
    let items = projection.find("count > limit").unwrap();
    assert!(
        signature < correspondence
            && correspondence < revision
            && revision < items
            && items < conversion
    );
    assert!(projection.contains("MAX_RUNTIME_LOG_QUERY_ENTRIES"));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_reply_tests.rs"))
            .unwrap();
    for proof in [
        "control_plane_replies_freeze_cross_category_type_and_legacy_image_only_matrix",
        "control_plane_reply_fault_order_is_correspondence_then_revision_then_item_limit",
        "control_plane_reply_identity_status_and_timestamp_rules_are_preserved",
        "control_plane_replies_keep_dispatch_only_mutations_and_once_terminal_replay",
        "control_plane_replies_keep_journal_restart_and_changed_receipt_replay",
    ] {
        assert!(proofs.contains(proof));
    }
}

#[test]
fn presentation_reply_correspondence_is_borrowed_and_adapter_owned() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation_reply.rs"))
            .unwrap();
    assert!(adapter.contains(
        "pub fn matches_effect(&self, effect: &Effect, operation: Option<&EffectOperation>) -> bool"
    ));
    assert_eq!(adapter.matches("Effect::Ui").count(), 62);
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "from_effect(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed product checker acquired {absent}"
        );
    }
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    assert_eq!(
        projection
            .matches("reply.matches_effect(context.effect, context.operation)")
            .count(),
        1
    );
    assert!(
        projection.find("reply.matches_effect(").unwrap()
            < projection
                .find("self.into_control_plane_value(context.result_type)")
                .unwrap()
    );
    assert!(!projection.contains("operation.is_none_or"));
    assert!(!projection.contains("PresentationEnvelope {"));
    assert!(!projection.contains("Effect::Ui"));
    assert!(!projection.contains("Type::Ui"));
    let correspondence = projection.find("reply.matches_effect(").unwrap();
    let conversion = projection.find("Value::from(reply)").unwrap();
    let type_gate = projection
        .find("value.result_type() != context.result_type")
        .unwrap();
    let done = projection.find("return Ok(value)").unwrap();
    assert!(correspondence < conversion && conversion < type_gate && type_gate < done);
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    assert!(
        cases.contains(
            "presentation_reply_projection_keeps_legacy_restore_types_and_mismatch_faults"
        )
    );
    assert!(cases.contains("presentation_reply_projection_moves_owned_strings_without_copying"));
}

#[test]
fn released_product_values_and_exhaustive_reply_projection_are_adapter_owned() {
    let adapter = fs::read_to_string(root().join("crates/leselang-command/src/value.rs")).unwrap();
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation_value.rs"))
            .unwrap();
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for declaration in ["pub enum Value", "pub struct StructuredField"] {
        assert!(adapter.contains(declaration));
        assert!(!vm.contains(declaration));
    }
    assert!(adapter.contains("pub fn result_type(&self) -> Type"));
    assert!(projection.contains("impl From<PresentationResult> for Value"));
    assert_eq!(projection.matches("PresentationResult::").count(), 62);
    for source in [&adapter, &projection] {
        for absent in [
            "use leselang_vm",
            "ContinuationImage",
            "Fuel::",
            "Journal",
            "serde_json",
            ".clone()",
            "_ =>",
        ] {
            assert!(
                !source.contains(absent),
                "product data mapping acquired {absent}"
            );
        }
    }
    let group = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    assert!(
        group.contains(
            "#[cfg(test)]\npub(super) fn value_type(value: &Value) -> Type {\n    value.result_type()\n}"
        )
    );
    assert!(!group.contains("macro_rules! atomic_types"));
}

#[test]
fn control_plane_value_conversion_follows_vm_gates_without_owning_execution_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/control_plane_value.rs"))
            .unwrap();
    assert!(
        adapter
            .contains("pub fn into_control_plane_value(self, result_type: Type) -> Option<Value>")
    );
    assert_eq!(adapter.matches("Type::").count(), 8);
    assert_eq!(adapter.matches("Box::new(").count(), 1);
    assert!(adapter.contains("_ => return None"));
    for absent in [
        ".clone()",
        "serde_json",
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "expected_revision",
        "max_output_items",
        "CommandStatus",
        "matches_control_plane_effect",
        "LSV210",
    ] {
        assert!(
            !adapter.contains(absent),
            "owned converter acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let projection = vm
        .split_once("fn atomic_step_from_effect_result(")
        .unwrap()
        .1
        .split_once("fn fault(code:")
        .unwrap()
        .0;
    for absent in [
        "Value::Runtime",
        "Value::DebuggerCancel",
        "Type::",
        "Box::new(",
    ] {
        assert!(
            !projection.contains(absent),
            "VM retained product conversion {absent}"
        );
    }
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    assert_eq!(projection.matches("expected != revision").count(), 1);
    assert_eq!(projection.matches("count > limit").count(), 1);
    assert_eq!(projection.matches("into_control_plane_value(").count(), 1);
    assert!(
        projection.find("count > limit").unwrap()
            < projection.find("into_control_plane_value(").unwrap()
    );
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_reply_tests.rs"))
            .unwrap();
    assert!(
        proof.contains(
            "control_plane_projection_keeps_query_revision_and_cardinality_policy_matrix"
        )
    );
    assert!(
        proof.contains(
            "control_plane_projection_moves_original_collections_strings_and_command_boxes"
        )
    );
}

#[test]
fn complete_effect_value_conversion_is_adapter_owned_with_explicit_original_context() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    for required in [
        "pub struct EffectValueContext<'a>",
        "pub enum EffectValueError",
        "pub fn into_effect_value(",
        "RevisionConflict",
        "OutputLimit",
        "MAX_RUNTIME_LOG_QUERY_ENTRIES",
        "formatter.write_str(\"EffectValueContext\")",
    ] {
        assert!(
            adapter.contains(required),
            "effect value boundary lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "ContinuationImage",
        "Fuel",
        "Journal",
        "serde",
        ".clone()",
        "impl Default",
        "LSV",
    ] {
        assert!(
            !adapter.contains(absent),
            "effect value boundary acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let boundary = vm
        .split_once("fn atomic_step_from_effect_result(")
        .unwrap()
        .1
        .split_once("fn fault(code:")
        .unwrap()
        .0;
    assert!(boundary.contains("reference_result::atomic_step_from_effect_result("));
    let product =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_result.rs")).unwrap();
    let boundary = product
        .split_once("pub fn atomic_step_from_effect_result(")
        .unwrap()
        .1;
    for required in [
        "effect: &image.pending_effect",
        "result_type: image.result_type",
        "expected_revision: image.expected_revision",
        "max_output_items: image.max_output_items",
        "EffectValueError::Mismatch",
        "EffectValueError::RevisionConflict",
        "EffectValueError::OutputLimit",
        "LSV2101",
        "LSV2102",
        "LSV2103",
    ] {
        assert!(
            boundary.contains(required),
            "reference mapping lost {required}"
        );
    }
    for absent in [
        "matches_control_plane_effect",
        "matches_effect(",
        "query_bounds",
        "into_control_plane_value",
        "Value::from(",
        "HostOperation",
        "QueryResult",
    ] {
        assert!(!boundary.contains(absent), "VM still composes {absent}");
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for required in [
        "composed_effect_values_cover_seventy_operations_types_and_legacy_context",
        "composed_effect_values_preserve_query_revision_count_and_mismatch_precedence",
        "composed_effect_values_move_payloads_without_granting_admission",
    ] {
        assert!(proof.contains(required));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_reply_tests.rs"))
            .unwrap();
    assert!(frozen.contains("fn legacy_atomic_step_from_effect_result("));
    assert!(
        frozen.contains("composed_reply_conversion_matches_frozen_native_wire_and_refusal_order")
    );
}

#[test]
fn control_plane_request_correspondence_is_adapter_owned_but_identity_generation_stays_in_vm() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/control_plane_request.rs"))
            .unwrap();
    for surface in [
        "pub fn query_matches_effect(",
        "pub fn command_matches_effect(",
        "expected_command_id: &str",
        "expected_idempotency_key: &str",
        "expected_revision: Option<Revision>",
    ] {
        assert!(adapter.contains(surface));
    }
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "format!(",
        "lower_effect(",
        "validate_effect_identity(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed control-plane matcher acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(vm.contains("pub use leselang_command::reference_request::{"));
    let admission = request
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn request_correspondence_fault(")
        .unwrap()
        .0;
    for absent in [
        "Query::",
        "Command::",
        "CommandOrigin::",
        "Confirmation::",
        "command.dry_run",
    ] {
        assert!(
            !admission.contains(absent),
            "VM admission retained product predicate {absent}"
        );
    }
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    let token = correspondence.find("canonical_sequence(&").unwrap();
    let command_id = correspondence
        .find("format!(\"leselang-command-{sequence}\")")
        .unwrap();
    let key = correspondence
        .find("format!(\"leselang-effect-{sequence}\")")
        .unwrap();
    let operation = correspondence.find(".validate_request(").unwrap();
    assert!(token < command_id && command_id < key && key < operation);
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_request.rs"))
            .unwrap();
    let query = policy
        .split_once("EffectOperation::Query(query)")
        .unwrap()
        .1;
    assert!(
        query.find("validate_effect_identity(").unwrap()
            < query
                .find("query_matches_effect(&query.query, effect)")
                .unwrap()
    );
    assert_eq!(policy.matches("command_matches_effect(").count(), 3);
    for branch in policy.split("EffectOperation::Command(command)").skip(1) {
        assert!(
            branch
                .split("command_matches_effect(")
                .next()
                .unwrap()
                .contains("validate_effect_identity(")
        );
    }
    for absent in [
        "validate_effect_identity(",
        "command_matches_effect(",
        "query_matches_effect(",
    ] {
        assert!(!admission.contains(absent));
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_request_tests.rs"))
            .unwrap();
    for name in [
        "control_plane_admission_keeps_image_identity_authority_payload_and_budget_order",
        "control_plane_admission_preserves_query_command_category_faults",
        "control_plane_admission_preserves_command_identity_confirmation_and_log_policy",
    ] {
        assert!(proof.contains(name));
    }
}

#[test]
fn composed_operation_request_policy_is_vm_free_without_bypassing_reference_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_request.rs"))
            .unwrap();
    for required in [
        "pub struct OperationRequestContext",
        "pub enum OperationRequestError",
        "pub fn validate_request(",
        "pub fn authority(",
    ] {
        assert!(adapter.contains(required));
    }
    let manifest = fs::read_to_string(root().join("crates/leselang-command/Cargo.toml")).unwrap();
    for absent in ["leselang-vm", "rusqlite"] {
        assert!(!manifest.contains(absent));
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(!vm.contains("fn validate_effect_identity("));
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(vm.contains("validate_effect_request(&request)"));
    for mapping in [
        "RequestCorrespondenceError::Operation(OperationRequestError::Capability) => \"LSV2011\"",
        "RequestCorrespondenceError::Operation(OperationRequestError::Envelope) => \"LSV2012\"",
        "RequestCorrespondenceError::Operation(OperationRequestError::Mismatch)",
        "RequestCorrespondenceError::Budget => \"LSV2013\"",
        "validate_effect_request(&request)",
    ] {
        assert!(request.contains(mapping));
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for test in [
        "operation_request_admission_covers_seventy_operations_without_vm_or_wire_changes",
        "operation_request_category_precedes_authority_and_authority_precedes_payload",
        "operation_request_identity_limits_and_diagnostics_remain_closed_and_redacted",
        "operation_request_uses_supplied_command_expectations_without_inventing_identity",
        "operation_request_correspondence_is_not_source_or_live_authority_admission",
    ] {
        assert!(tests.contains(test));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(frozen.contains("fn frozen_validate_effect_request("));
    assert!(
        frozen.contains("operation_request_adapter_preserves_frozen_seventy_operation_admission")
    );
}

#[test]
fn presentation_request_payload_checks_are_adapter_owned_without_reordering_vm_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation_request.rs"))
            .unwrap();
    assert!(adapter.contains("pub fn matches_effect(&self, effect: &Effect) -> bool"));
    assert_eq!(adapter.matches("Effect::Ui").count(), 62);
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "from_effect(",
        "_ =>",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed request matcher acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    assert!(vm.contains("pub use leselang_command::reference_request::{"));
    let admission = request
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn request_correspondence_fault(")
        .unwrap()
        .0;
    assert!(!admission.contains("Effect::Ui"));
    assert!(!admission.contains("PresentationOperation::"));
    assert!(admission.contains("crate::validate_effect_request("));
    let complete =
        fs::read_to_string(root().join("crates/leselang-command/src/request_admission.rs"))
            .unwrap();
    let ordered = complete
        .split_once("pub fn validate_effect_request")
        .unwrap()
        .1;
    assert!(
        ordered.find("validate_continuation_frame(").unwrap()
            < ordered
                .find("validate_preflighted_effect_request(")
                .unwrap()
    );
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    let successor = correspondence.find(".required_capabilities()").unwrap();
    let token = correspondence.find("canonical_sequence(&").unwrap();
    let identity = correspondence
        .find("request.effect_id != expected_effect_id")
        .unwrap();
    let operation = correspondence.find(".validate_request(").unwrap();
    let budget = correspondence
        .find("if *request.budget != request.continuation.resource_budget()")
        .unwrap();
    assert!(successor < token && token < identity && identity < operation && operation < budget);
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_request.rs"))
            .unwrap();
    let gui = policy
        .split_once("(effect, EffectOperation::Presentation(presentation))")
        .unwrap()
        .1;
    let category = gui.find("HostOperation::for_effect(effect)").unwrap();
    let capability = gui
        .find("operation.required_capability() == CAPABILITY_UI_PRESENTATION")
        .unwrap();
    let authority = gui.find("validate_effect_identity(").unwrap();
    let payload = gui
        .find("presentation.operation.matches_effect(effect)")
        .unwrap();
    assert!(category < capability && capability < authority && authority < payload);
    assert_eq!(
        policy
            .matches("presentation.operation.matches_effect(effect)")
            .count(),
        1
    );
    for absent in [
        "ContinuationImage",
        "leselang_vm",
        "Fuel::",
        "Journal",
        ".clone()",
        "format!(",
    ] {
        assert!(!policy.contains(absent));
    }
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    for proof in [
        "presentation_admission_keeps_image_identity_authority_payload_and_budget_order",
        "presentation_admission_does_not_apply_gui_authority_to_non_gui_effects",
    ] {
        assert!(cases.contains(proof));
    }
}

#[test]
fn product_value_bounds_are_adapter_owned_but_execution_gates_stay_in_vm() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/value_validation.rs")).unwrap();
    assert!(
        adapter
            .contains("pub fn bounded_leaf_items(&self, max_query_items: usize) -> Option<usize>")
    );
    assert_eq!(adapter.matches("Value::Ui").count(), 62);
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "BTreeSet",
        "fn validate_value(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed product leaf checker acquired {absent}"
        );
    }
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_value.rs")).unwrap();
    let traversal = policy
        .split_once("pub fn validate_branch_outcome(")
        .unwrap()
        .0;
    assert!(!traversal.contains("Value::Ui"));
    assert!(!traversal.contains("Value::Runtime"));
    for required in [
        "crate::validate_value_items(",
        "max_depth: MAX_STRUCTURED_VALUE_DEPTH",
        "max_fields: MAX_MERGE_BRANCHES",
        "max_query_items: DEFAULT_MAX_OUTPUT_ITEMS",
        "ValueBoundsError::Nesting",
        "ValueBoundsError::Fields",
        "ValueBoundsError::Value",
    ] {
        assert!(
            traversal.contains(required),
            "VM lost value policy {required}"
        );
    }
    assert!(!traversal.contains("for field in fields"));
    assert!(!traversal.contains("BTreeSet"));
    let tree =
        fs::read_to_string(root().join("crates/leselang-command/src/structured_value.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "LSV",
        "ContinuationImage",
        "Journal",
        "serde_json",
        ".clone()",
        "Fuel",
        "impl Default",
        "MAX_STRUCTURED_VALUE_DEPTH",
        "MAX_MERGE_BRANCHES",
        "DEFAULT_MAX_OUTPUT_ITEMS",
    ] {
        assert!(
            !tree.contains(absent),
            "value traversal acquired host policy {absent}"
        );
    }
    for required in [
        "pub struct ValueValidationLimits",
        "pub enum ValueBoundsError",
        "parents: Vec<Fields<'_>>",
        "depth > limits.max_depth",
        "fields.is_empty() || fields.len() > limits.max_fields",
        "!validate_name(&field.name) || !parent.names.insert(field.name.as_str())",
        "depth.checked_add(1)",
        ".bounded_leaf_items(limits.max_query_items)",
    ] {
        assert!(
            tree.contains(required),
            "shared value traversal lost {required}"
        );
    }
    assert_eq!(
        tree.matches("validate_value_items(").count(),
        1,
        "no recursive value traversal"
    );
    let bound =
        fs::read_to_string(root().join("crates/leselang-command/src/result_domain.rs")).unwrap();
    let leaf = bound
        .find("let count = validate_value_items(reply, 0, self.value_limits")
        .unwrap();
    let budget = bound
        .find("count > self.max_output_items || self.max_output_items == 0")
        .unwrap();
    let bytes = bound.find(".validate_encoded_value(reply)").unwrap();
    assert!(leaf < budget && budget < bytes);
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    for proof in [
        "product_leaf_validation_keeps_counts_and_saved_value_compatibility",
        "product_leaf_validation_keeps_recursive_fault_order_and_messages",
    ] {
        assert!(cases.contains(proof));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/value_validation_tests.rs"))
            .unwrap();
    assert!(
        frozen
            .contains("value_traversal_matches_frozen_counts_fault_precedence_and_unchanged_bytes")
    );
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/structured_value.rs"))
            .unwrap();
    for required in [
        "declaration_order_and_local_duplicate_scopes_preserve_refusal_precedence",
        "borrowed_names_payloads_and_native_unwind_do_not_mutate_input_or_retry_hooks",
        "deep_explicit_policy_uses_no_recursive_rust_stack",
    ] {
        assert!(tests.contains(required));
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test capture_scope --test structured_value"));
}

#[test]
fn received_result_domain_is_adapter_owned_with_explicit_native_policy_and_pending_ownership() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/result_domain.rs")).unwrap();
    for required in [
        "pub trait ResultValueHost",
        "pub struct ResultValueDomain<'host, Host: ?Sized>",
        "pub enum ResultValueError<Native>",
        "impl<Host: ResultValueHost + ?Sized> HostResultDomain<Value>",
        "reply.result_type() == self.result_type",
        "pub value_limits: ValueValidationLimits",
        "pub max_output_items: usize",
        "pub host: &'host Host",
        "self.host.valid_field_name(name)",
        ".map_err(ResultValueError::Encoding)",
    ] {
        assert!(
            adapter.contains(required),
            "result-domain adapter lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "ContinuationImage",
        "Journal",
        "serde_json",
        ".clone()",
        "Fuel",
        "impl Default",
        "LSV",
        "fn authorize",
        "PendingReply::new",
    ] {
        assert!(
            !adapter.contains(absent),
            "result-domain adapter acquired {absent}"
        );
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/host_result.rs")).unwrap();
    let production =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_result.rs")).unwrap();
    assert!(vm.contains("use leselang_command::reference_result::validate_bound_value"));
    assert!(!vm.contains("impl ResultValueHost"));
    for required in [
        "result_type: image.result_type",
        "value_limits: crate::reference_value::value_validation_limits()",
        "max_output_items: image.max_output_items",
        "host: &BoundResultHost",
        "crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)",
        "crate::bounded_json::measure(reply, MAX_JOURNAL_ENTRY_BYTES)",
        "ResultValueError::Bounds(error) => {",
        "crate::reference_value::value_bounds_fault(error)",
        "ResultValueError::Encoding(error) => error",
    ] {
        assert!(
            production.contains(required),
            "reference policy lost {required}"
        );
    }
    for absent in [
        "impl HostResultDomain",
        "crate::validate_value(reply",
        "items >",
        "reply.result_type()",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    assert!(vm.contains("bound_value_domain_matches_frozen_fault_order_bytes_and_original_owners"));
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/result_domain.rs")).unwrap();
    for required in [
        "result_domain_preserves_type_tree_output_encoding_order_and_zero_limit",
        "dynamic_host_keeps_native_error_ownership_without_debug_or_clone_bounds",
        "pending_rejection_keeps_original_reply_and_frame_until_explicit_once_only_handoff",
        "native_unwind_seals_pending_state_and_scope_before_frame_cleanup",
        "scope_cancellation_wins_over_native_refusal_and_never_rearms",
    ] {
        assert!(proof.contains(required));
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test structured_value --test result_domain"));
}

#[test]
fn presentation_payload_mapping_and_request_construction_keep_original_gate_order() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation.rs")).unwrap();
    assert!(adapter.contains("pub fn from_effect(effect: &Effect) -> Option<Self>"));
    for absent in [
        "use leselang_vm",
        "Fuel::new",
        "ContinuationImage",
        "Journal",
        "LoweringContext",
        "_ =>",
    ] {
        assert!(
            !adapter.contains(absent),
            "pure payload mapper acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let build = vm
        .split_once("fn build_effect_request(")
        .unwrap()
        .1
        .split_once("pub fn restore(")
        .unwrap()
        .0;
    assert!(!build.contains("PresentationEnvelope {"));
    assert!(!build.contains("Effect::Ui"));
    assert!(build.contains("leselang_command::reference_request_build::build_effect_request("));
    let build =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request_build.rs"))
            .unwrap();
    let debit = build.find("fuel.charge(1)").unwrap();
    let image = build.find("let image = ContinuationImage").unwrap();
    let map = build.find("crate::lower_effect_operation(").unwrap();
    let validation = build.find("validate_effect_request(&request)?").unwrap();
    assert!(debit < image && image < map && map < validation);
    assert!(!build.contains("lower_effect("));
    assert!(!build.contains("origin: CommandOrigin::Leselang"));
    assert!(!build.contains("Confirmation::Confirmed"));
    assert!(!build.contains("image.clone()"));
    let lowering =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_lowering.rs"))
            .unwrap();
    for required in [
        "PresentationOperation::from_effect(effect)",
        "lower_effect(",
        "origin: CommandOrigin::Leselang",
        "Confirmation::Confirmed",
        "dry_run: false",
    ] {
        assert!(lowering.contains(required));
    }
    let gui = lowering
        .find("PresentationOperation::from_effect(effect)")
        .unwrap();
    let identity = lowering.find("command_identity()").unwrap();
    let control = lowering.find("let plan = lower_effect(").unwrap();
    assert!(gui < identity && identity < control);
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    assert!(
        cases.contains("presentation_request_mapping_keeps_frozen_bytes_and_single_budget_debit")
    );
    assert!(
        cases.contains(
            "presentation_request_refusals_keep_fuel_shape_type_and_authority_precedence"
        )
    );
}

#[test]
fn composed_operation_lowering_does_not_own_request_budget_or_live_reservations() {
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_lowering.rs"))
            .unwrap();
    for absent in [
        "leselang_vm",
        "ContinuationImage",
        "Fuel",
        "Journal",
        "serde_json",
        "leselang-command-",
        "leselang-effect-",
    ] {
        assert!(!policy.contains(absent));
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let build = vm
        .split_once("fn build_effect_request(")
        .unwrap()
        .1
        .split_once("pub fn restore(")
        .unwrap()
        .0;
    assert!(build.contains("reference_request_build::ReservedEffectInput {"));
    for absent in [
        "Fuel::new",
        "format!(",
        "validate_effect_request",
        ".clone()",
        "LSV",
    ] {
        assert!(!build.contains(absent));
    }
    let build =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request_build.rs"))
            .unwrap();
    for required in [
        "fuel.charge(1)",
        "CommandId::new(format!(\"leselang-command-{sequence}\"))",
        "IdempotencyKey::new(format!(\"leselang-effect-{sequence}\"))",
        "OperationLoweringError::Identity",
        "OperationLoweringError::Lowering",
        "validate_effect_request(&request)?",
    ] {
        assert!(build.contains(required));
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for name in [
        "operation_lowering_composes_seventy_wire_shapes_with_lazy_identity_and_original_gui_owner",
        "operation_lowering_identity_refusal_precedes_control_plane_capability_checks",
        "operation_lowering_rejects_structured_effects_after_exactly_one_identity_attempt",
        "operation_lowering_gui_construction_is_not_authorization_or_request_admission",
        "operation_lowering_identity_factory_unwind_does_not_retry_or_retain_owned_capture",
    ] {
        assert!(tests.contains(name));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(frozen.contains("fn frozen_build_effect_request("));
    assert!(
        frozen.contains("operation_lowering_preserves_frozen_build_wire_fuel_and_refusal_order")
    );
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test result_domain --test host_protocol"));
}

#[test]
fn request_construction_is_static_policy_not_live_journal_authority() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request_build.rs"))
            .unwrap();
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "allocate_sequence(",
        "record_pending(",
        "claim_dispatch(",
        "std::fs",
        "std::thread",
        "std::time",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    assert_eq!(adapter.matches("fuel.charge(1)").count(), 1);
    assert_eq!(adapter.matches("pending_effect: effect.clone()").count(), 1);
    assert!(!adapter.contains("image.clone()"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(vm.contains("reference_allocation::allocate_sequence("));
    assert!(vm.contains("self.0.journal.allocate_sequence(local_next)"));
    assert!(vm.contains(".record_pending(image, request, self.scheduler_limits)"));
    assert!(vm.contains(".claim_dispatch(now_ms, lease_ms, self.0.scheduler_limits)"));
    let oracle =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    let frozen = oracle
        .split_once("fn frozen_build_effect_request(")
        .unwrap()
        .1
        .split_once("#[test]")
        .unwrap()
        .0;
    assert!(!frozen.contains("reference_request_build"));
    assert!(!frozen.contains("lower_effect_operation"));
    assert!(oracle.contains("assert_eq!(comparisons, 12960)"));
    let alias =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_successor.rs"))
            .unwrap();
    assert!(alias.contains("pub use crate::reference_request_build::ReservedEffectInput;"));
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(ci.contains("request_construction --no-fail-fast"));
}

#[test]
fn product_wire_consumers_do_not_import_the_language_vm_or_its_journal_backend() {
    for package in ["leselang-command", "leserpent-protocol"] {
        let output = Command::new(env!("CARGO"))
            .args([
                "tree",
                "--locked",
                "--offline",
                "-p",
                package,
                "--edges",
                "normal",
                "--prefix",
                "none",
            ])
            .current_dir(root())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dependencies = String::from_utf8(output.stdout).unwrap();
        assert!(dependencies.contains("leselang-command v"));
        for absent in ["leselang-vm v", "rusqlite v", "libsqlite3-sys v"] {
            assert!(
                !dependencies.contains(absent),
                "{package} still imports {absent}"
            );
        }
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/host_protocol.rs")).unwrap();
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for declaration in [
        "enum EffectOperation",
        "enum EffectResult",
        "struct PresentationEnvelope",
        "enum PresentationOperation",
        "enum PresentationResult",
        "struct DebuggerCancelResult",
    ] {
        assert!(adapter.contains(&format!("pub {declaration}")));
        assert!(!vm.contains(&format!("pub {declaration}")));
    }
    assert!(vm.contains("pub use leselang_command::{"));
    assert!(
        fs::read_to_string(root().join("crates/leserpent-protocol/src/lib.rs"))
            .unwrap()
            .contains("use leselang_command::PresentationOperation;")
    );
    let native = fs::read_to_string(root().join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(!native.contains("leselang_command"));
}

#[test]
fn shared_watermark_planning_keeps_native_validation_reads_and_commit_before_publish() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let advance = journal
        .split("fn advance_sequence_watermark(")
        .nth(1)
        .unwrap()
        .split("fn tighten_permissions(")
        .next()
        .unwrap();
    let requirement = advance
        .find("SequenceWatermarkRequirement::after_observation(")
        .unwrap();
    let read = advance
        .find("SELECT next_sequence FROM vm_metadata")
        .unwrap();
    let signed = advance.find("u64::try_from(stored)").unwrap();
    let reconcile = advance.find(".reconcile(stored)").unwrap();
    let write = advance
        .find("UPDATE vm_metadata SET next_sequence")
        .unwrap();
    assert!(requirement < read && read < signed && signed < reconcile && reconcile < write);
    for required in [
        "i64::MAX as u64",
        "if update.write_required",
        "Ok(update.next_sequence)",
        "LSV4009",
        "LSV4007",
        "LSV4008",
    ] {
        assert!(advance.contains(required));
    }
    let allocation = journal
        .rsplit("fn allocate_sequence(&mut self, local_next:")
        .next()
        .unwrap()
        .split("fn record_pending(")
        .next()
        .unwrap();
    let numeric = allocation
        .find("SequenceWatermarkRequirement::after_observation(")
        .unwrap();
    assert!(allocation.find("TransactionBehavior::Immediate").unwrap() < numeric);
    assert!(allocation.find("SELECT next_sequence").unwrap() < numeric);
    assert!(numeric < allocation.find("UPDATE vm_metadata").unwrap());
    assert!(allocation.find(".commit()").unwrap() < allocation.find("Ok(sequence)").unwrap());
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let restore =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_restore.rs"))
            .unwrap();
    let next = restore
        .find("SequenceWatermarkRequirement::after_observation(")
        .unwrap();
    assert!(
        restore
            .find("canonical_continuation_sequence(&image.token)")
            .unwrap()
            < next
    );
    assert!(next < restore.rfind("host.commit_new(").unwrap());
    assert!(vm.contains(".record_pending(image, request, self.scheduler_limits)"));
    let owner = restore
        .split_once("impl<Commit> ReferenceRestoreHost for CacheImport")
        .unwrap()
        .1
        .split_once("/// Trusted owner")
        .unwrap()
        .0;
    let commit = owner.split_once("fn commit_new(").unwrap().1;
    assert!(
        commit.find("(self.commit)(&image, request)?").unwrap()
            < commit.find("*self.cache.next_sequence =").unwrap()
    );
    assert!(journal.contains("watermark_planning_preserves_frozen_sqlite_fault_order_and_writes"));
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/tests/restore_allocator.rs"))
            .unwrap()
            .contains(
                "watermark_planning_preserves_every_table_on_import_recovery_and_exhaustion_refusal"
            )
    );
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/sequence_watermark.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
        "UPDATE",
        "INSERT",
    ] {
        assert!(!core.contains(absent), "shared watermark imported {absent}");
    }
    assert!(core.contains("stored_watermark.max(self.required)"));
    assert!(core.contains("write_required: self.required > stored_watermark"));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/sequence_watermark.rs")
    );
}

#[test]
fn continuation_context_kernel_preserves_native_phase_order_in_every_succession_path() {
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let helper = source
        .split("pub(super) fn same_inherited_context(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn validate_pair(")
        .next()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        helper
            .split_whitespace()
            .collect::<String>()
            .contains("original.as_view().has_same_inherited_context(&saved.as_view())")
    );
    let shared_context =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    let shared_context = shared_context
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "validate_inherited_context(",
        "self.continuation.resource_budget()",
        "saved.continuation.resource_budget()",
        "self.operation.authority() == saved.operation.authority()",
        "self.continuation.expected_revision == saved.continuation.expected_revision",
    ] {
        assert!(
            shared_context.contains(required),
            "context adapter lost {required}"
        );
    }
    assert!(!helper.contains("Fuel::new"));
    let pair = source
        .split("pub(super) fn validate_pair(")
        .nth(1)
        .unwrap()
        .split("struct ReferenceCompletionOwner")
        .next()
        .unwrap();
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let pair =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    let context = pair
        .find("parent.has_same_inherited_context(&next)")
        .unwrap();
    assert!(pair.find("!schema_valid").unwrap() < context);
    assert!(
        context
            < pair
                .find("first.fuel_remaining <= second.fuel_remaining")
                .unwrap()
    );
    assert!(context < pair.find("canonical_sequence(&first.token)").unwrap());
    let groups =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let admission = groups
        .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
        .unwrap()
        .1
        .split("fn validate_requests(")
        .nth(1)
        .unwrap()
        .split("fn validate_member(")
        .next()
        .unwrap();
    assert!(admission.contains("leselang_command::validate_preflighted_group_requests("));
    let admission =
        fs::read_to_string(root().join("crates/leselang-command/src/group_requests.rs")).unwrap();
    assert_eq!(
        admission
            .matches("first.has_same_inherited_context(&request)")
            .count(),
        2
    );
    let (prefix, tail) = admission.split_once("let mut previous_fuel").unwrap();
    assert!(
        prefix
            .find("Some(image.fuel_remaining) != expected_fuel")
            .unwrap()
            < prefix.find("same_inherited_context").unwrap()
    );
    assert!(
        tail.find("image.fuel_remaining >= previous_fuel").unwrap()
            < tail.find("same_inherited_context").unwrap()
    );
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/continuation_context.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
        "fuel_remaining",
    ] {
        assert!(!core.contains(absent), "shared context imported {absent}");
    }
    for required in [
        "drop(same_authority)",
        "drop(same_revision)",
        "original.deadline_at_ms != saved.deadline_at_ms",
    ] {
        assert!(core.contains(required));
    }
    assert!(
        source.contains("inherited_context_keeps_frozen_native_agreement_across_operation_domains")
    );
    assert!(fs::read_to_string(root().join("crates/leselang-vm/tests/successor.rs")).unwrap()
        .contains("inherited_context_refusal_preserves_journal_and_imported_watermark_for_every_succession_kind"));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/continuation_context.rs")
    );
}

#[test]
fn successor_capture_extension_shares_retention_without_authority_or_saved_body_execution() {
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let pair = source
        .split("pub(super) fn validate_pair(")
        .nth(1)
        .unwrap()
        .split("struct ReferenceCompletionOwner")
        .next()
        .unwrap();
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    let correspondence = correspondence.split_whitespace().collect::<String>();
    let extension = correspondence
        .find("validate_result_binding_extension(prior,")
        .unwrap();
    for guard in [
        "parent.has_same_inherited_context(&next)",
        "first.fuel_remaining <= second.fuel_remaining",
        "canonical_sequence(&first.token)",
        "validate_result_binding(prior, &first.pending_effect, limits, &mut validate_group_name,",
    ] {
        assert!(
            correspondence
                .find(&guard.split_whitespace().collect::<String>())
                .unwrap()
                < extension,
            "extension displaced {guard}"
        );
    }
    for retained in [
        "leselang_command::validate_result_binding_extension(",
        "leselang_hir::MAX_EFFECT_NESTING_DEPTH",
        "leselang_command::CaptureReplayError::InvalidProjection",
        "leselang_command::CaptureReplayError::Mismatch => invalid()",
        "capture_extension_preserves_frozen_native_alias_retention_and_late_fault_order",
    ] {
        assert!(source.contains(retained), "VM extension lost {retained}");
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_replay.rs")).unwrap();
    for retained in [
        "validate_capture_extension(",
        "old.group == saved.group",
        "old.result == saved.result",
        "saved.name == self.prior.name",
        "CaptureExtensionError::Native { error, .. } => error",
    ] {
        assert!(
            adapter.contains(retained),
            "extension adapter lost {retained}"
        );
    }
    assert!(!pair.contains("Fuel::new"));
    let journal =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/successor.rs")).unwrap();
    assert!(journal.contains("validate_pair(parent, step, next)?"));
    assert!(journal.contains("validate_pair(parent, &result, &next)?"));
    assert!(fs::read_to_string(root().join("crates/leselang-vm/tests/dataflow.rs")).unwrap()
        .contains("successor_capture_extension_refusal_preserves_every_journal_table_and_original_budget"));
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/capture_extension.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent));
    }
    assert!(core.contains("drop(domain)"));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/capture_extension.rs")
    );
}

#[test]
fn live_and_recovered_capture_correspondence_delegates_traversal_without_projection_or_authority_policy()
 {
    let source =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let bridge = source
        .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
        .unwrap()
        .1
        .split("fn validate_saved_capture(")
        .nth(1)
        .unwrap()
        .split("mod correspondence_tests")
        .next()
        .unwrap();
    assert!(bridge.contains("leselang_command::reference_group_binding::validate_saved_capture("));
    assert!(!bridge.contains("CaptureCorrespondenceLimits"));
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_binding.rs"))
            .unwrap();
    let gate = policy
        .split("pub fn validate_saved_capture(")
        .nth(1)
        .unwrap();
    for required in [
        "fields.len() < branches.len()",
        "fields.len() != operations.len()",
        "max_saved_bindings: leselang_hir::MAX_EFFECT_NESTING_DEPTH",
        "max_original_rows: MAX_MERGE_BRANCHES",
        "crate::validate_group_capture_correspondence(",
        "result_binding_limits().groups",
        "valid_merge_branch_name",
        "CaptureReplayError::InvalidProjection => invalid_projection()",
        "CaptureReplayError::Mismatch => invalid()",
    ] {
        assert!(gate.contains(required), "native capture lost {required}");
    }
    assert!(
        gate.find(".ok_or_else(invalid)?").unwrap()
            < gate
                .find("crate::validate_group_capture_correspondence(")
                .unwrap()
    );
    assert!(
        source.contains(
            "leselang_command::reference_group_binding::validate_member(self, name, value)"
        )
    );
    let member = policy
        .split("pub fn validate_member(")
        .nth(1)
        .unwrap()
        .split("pub fn validate_saved_capture(")
        .next()
        .unwrap();
    assert!(
        member.find("reserved_sequences().count()").unwrap()
            < member.find("let branch = group").unwrap()
    );
    assert!(
        member.contains("group.preflighted_capture_accepts_type_at(index, value.result_type())")
    );
    assert!(member.contains("crate::validate_result_binding("));
    for forbidden in [
        "Fuel::new",
        "resume_outcome",
        "resume_group",
        "rusqlite",
        "commit()",
        "evaluate(",
    ] {
        assert!(!policy.contains(forbidden));
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_replay.rs")).unwrap();
    for retained in [
        "validate_capture_correspondence(",
        ".matches_members(",
        "&fields[..branches.len()]",
        "matches_capture(operations[original], &field.value)",
        "CaptureCorrespondenceError::Native { error, .. } => error",
    ] {
        assert!(adapter.contains(retained));
    }
    assert!(
        source
            .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
            .unwrap()
            .1
            .split("fn advance_capture(")
            .nth(1)
            .unwrap()
            .contains("leselang_command::reference_group_execution::advance_capture(")
    );
    let execution =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_execution.rs"))
            .unwrap();
    let capture = execution.split_once("pub fn advance_capture").unwrap().1;
    let preflight = "validate_saved_capture(group, request, prefix, &operations)?";
    assert!(capture.find(preflight).unwrap() < capture.find("host.resume_capture(").unwrap());
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/group_binding.rs"))
            .unwrap()
            .contains("binding.validate_saved_capture(request, fields, operations)")
    );
    assert!(
        fs::read_to_string(root().join("crates/leselang-command/src/reference_bound_records.rs"))
            .unwrap()
            .contains(
                "proofs.validate_capture(binding, &request, &prefix_fields, &prefix_operations)?"
            )
    );
    assert!(source.contains("saved_capture_correspondence_preserves_frozen_native_fault_order"));
    assert!(fs::read_to_string(root().join("crates/leselang-vm/tests/parallel_group_tail.rs")).unwrap().contains("false_group_alias_and_result_capture_correspondence_blocks_recovery_without_mutation"));
    let core = fs::read_to_string(
        root().join("crates/leselang-runtime-core/src/capture_correspondence.rs"),
    )
    .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent));
    }
    assert!(core.contains("matched |= match_result("));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/capture_correspondence.rs")
    );
}

#[test]
fn live_sequence_and_bound_recovery_share_stop_precedence_without_replay_or_settlement() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("use leselang_command::reference_completion::{"));
    let completion =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_completion.rs"))
            .unwrap();
    let sequence = completion
        .split("fn sequence_stop<'a>(")
        .nth(1)
        .unwrap()
        .split("pub fn terminal_step_outcome(")
        .next()
        .unwrap();
    for required in [
        "OrderedOutcomeSummary::new(",
        "OutcomeOrder::Sequential",
        "validate_terminal_step(step)?",
        "observe_success(validate_value(value, 0)?",
        "observe_stop(step.clone())",
        "if outcomes.has_stop()",
        "outcomes.finish(overflow)",
    ] {
        assert!(sequence.contains(required), "sequence lost {required}");
    }
    let bound =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/group_binding.rs")).unwrap();
    let records =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_bound_records.rs"))
            .unwrap();
    let recovery = records.as_str();
    for required in [
        "OrderedOutcomeSummary::new(",
        "OutcomeOrder::Sequential",
        "OutcomeOrder::Parallel",
        "observe_success(validate_value(&value, 0)?",
        "outcomes.observe_stop(step)",
        "proofs.validate_capture(",
        "proofs.validate_reservations(plan)?",
        "validate_bound_group_receipt(",
    ] {
        assert!(
            recovery.contains(required),
            "bound recovery lost {required}"
        );
    }
    let native = bound
        .split("pub(super) fn validate_records(")
        .nth(1)
        .unwrap();
    for required in [
        "LEFT JOIN vm_dispatches d ON d.token = b.branch_token",
        "ORDER BY b.position ASC",
        "row.get::<_, String>(0)?",
        "row.get::<_, Option<Vec<u8>>>(1)?",
        "row.get::<_, Option<Vec<u8>>>(2)?",
        "inspect_bound_group_records(",
        "rows.map(|row|",
        "binding.validate_saved_capture(request, fields, operations)",
        "crate::successor::validate_pair(previous, completed, request)",
        "binding.validate_member(name, value)",
        "validate_merge_graph_input(token, plan, requests)",
    ] {
        assert!(native.contains(required), "record bridge lost {required}");
    }
    assert!(!native.contains(".collect::<"));
    assert!(!native.contains("prefix_fields"));
    assert!(recovery.find("for row in rows {").unwrap() < recovery.find(".collect::<").unwrap());
    assert!(
        recovery.find("proofs.validate_capture(").unwrap()
            < recovery.find("\n        previous_step = None;").unwrap()
    );
    assert!(
        recovery.find("\n        previous_step = None;").unwrap()
            < recovery.find("if let Some(bytes) = step").unwrap()
    );
    for forbidden in [
        "rusqlite",
        "Fuel::new",
        "evaluate(",
        "advance_sequence_watermark(",
    ] {
        assert!(!recovery.contains(forbidden));
    }
    let receipt =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_bound_receipt.rs"))
            .unwrap();
    assert!(
        receipt.find("if let Some(terminal) = terminal").unwrap()
            < receipt.find(".finish(|items, limit|").unwrap()
    );
    assert!(receipt.contains("if let Some(failure) = summary.first_stop"));
    assert!(receipt.contains("validate_group_binding(binding, plan)?"));
    assert!(!recovery.contains("binding.validate(plan)?"));
    assert!(!recovery.contains(".finish(|items, limit|"));
    assert!(!receipt.contains("rusqlite"));
    let classifications =
        fs::read_to_string(root().join("crates/leselang-command/src/group_plan.rs")).unwrap();
    let vm_binding =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    assert!(classifications.contains("pub fn has_successor(&self, plan: &MergePlan) -> bool"));
    assert!(!vm_binding.contains("fn has_successor("));
    for source in [sequence, recovery, &receipt] {
        for absent in ["Fuel::new", "resume_group(", "commit()"] {
            assert!(!source.contains(absent));
        }
    }
    assert!(
        journal.contains("sequence_stop_preserves_frozen_early_exit_and_native_fault_precedence")
    );
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/tests/group_receipts.rs"))
            .unwrap()
            .contains("bound_output_precedence_receipts_cannot_replace_aggregate_or_first_stop")
    );
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/outcome_summary.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent));
    }
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/outcome_summary.rs")
    );
}

#[test]
fn pending_sequence_recovery_delegates_frontier_phases_without_dispatch_or_evaluation() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_recovery.rs"))
            .unwrap();
    let graph = adapter.as_str();
    assert!(
        graph.contains(
            "require_pending_sequence: plan.order.is_sequential() && state == \"pending\""
        )
    );
    assert!(graph.contains("self.proofs.validate_blocked_dispatches(first_pending)"));
    assert_complete_group_recovery_order();
    let frontier = adapter
        .split("pub fn inspect_pending_sequence(")
        .nth(1)
        .unwrap()
        .split("fn invalid_graph(")
        .next()
        .unwrap();
    for required in [
        "inspect_sequential_frontier(",
        "MAX_MERGE_BRANCHES",
        "MAX_JOURNAL_ENTRY_BYTES",
        "sequence_stop(terminal_steps.iter())?",
        "SequenceFrontierError::Native { error, .. } => error",
        "SequenceFrontierError::OutOfOrder",
    ] {
        assert!(
            frontier.contains(required),
            "native frontier lost {required}"
        );
    }
    assert!(
        frontier.find("decode_journal_value::<Step>(").unwrap()
            < frontier.find("validate_blocked(first_pending)").unwrap()
    );
    let storage = journal
        .split("fn validate_blocked_dispatches(\n")
        .nth(1)
        .unwrap()
        .split("fn recover_group_receipt(")
        .next()
        .unwrap();
    assert!(storage.contains("b.position > ?2"));
    assert!(storage.contains("d.attempt != 0 OR d.retry_count != 0"));
    assert!(!storage.contains("decode_bounded"));
    for absent in [
        "Fuel::new",
        "resume(",
        "commit()",
        "advance_sequence_watermark(",
    ] {
        assert!(!frontier.contains(absent));
    }
    for proof in [
        "sequence_frontier_preserves_frozen_sqlite_phase_and_fault_order",
        "invalid_sequence_frontier_blocks_snapshot_and_watermark_repair_without_mutation",
    ] {
        assert!(journal.contains(proof));
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/sequence_frontier.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(
            !core.contains(absent),
            "frontier core grew policy: {absent}"
        );
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/sequence_frontier.rs"));
}

#[test]
fn product_group_recovery_policy_preserves_explicit_host_proofs_and_has_no_storage() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_recovery.rs"))
            .unwrap();
    for required in [
        "pub trait ReferenceGroupProofs",
        "Proofs: ReferenceGroupProofs + ?Sized",
        "Bytes: AsRef<[u8]>",
        "inspect_group_recovery(",
        "inspect_sequential_frontier(",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "rusqlite",
        "leselang_vm",
        "std::fs",
        "Fuel::new",
        ".commit()",
        "merge_declared(",
        "group_binding::finish(",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let proofs = journal
        .split_once("impl ReferenceGroupProofs for RecoveredGroupHost")
        .unwrap()
        .1
        .split_once("fn validate_pending_sequence(")
        .unwrap()
        .0;
    for required in [
        "successor::validate_chain(self.connection, self.token)",
        "validate_blocked_dispatches(self.connection, self.token, first_pending)",
        "validate_unbound_group_receipt(self.plan, self.branches, terminal)",
        "group_binding::validate_records(self.connection, self.token, self.plan, terminal)",
    ] {
        assert!(proofs.contains(required), "{required}");
    }
    assert!(!proofs.contains("decode_bounded"));
    let direct = fs::read_to_string(
        root().join("crates/leselang-command/tests/reference_group_recovery.rs"),
    )
    .unwrap();
    for required in [
        "assert_eq!(cases, 12600)",
        "assert_eq!(cases, 324)",
        "assert_eq!(cases, 8)",
        "borrowed_receipt_bounds_and_original_proof_payload_are_preserved",
    ] {
        assert!(direct.contains(required), "{required}");
    }
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs"))
            .unwrap()
            .contains("inspected_group_cannot_publish_a_receipt_or_replace_live_member_authority")
    );
    assert!(
        fs::read_to_string(root().join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test reference_group_recovery")
    );
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Product Single Group Recovery Ownership")
    );
}

#[test]
fn complete_stored_group_recovery_is_core_owned_without_native_storage_or_product_types() {
    assert_complete_group_recovery_order();
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_recovery.rs"))
            .unwrap();
    for required in [
        "pub trait GroupRecoveryHost",
        "pub struct GroupRecoveryPolicy",
        "pub enum GroupRecoveryError",
        "Host: GroupRecoveryHost<Row, Key> + ?Sized",
    ] {
        assert!(core.contains(required), "{required}");
    }
    for forbidden in [
        "leselang_command",
        "leselang_vm",
        "rusqlite",
        "Fuel::",
        "LSV",
        "serde_json",
        ".clone()",
        "impl Default",
    ] {
        assert!(!core.contains(forbidden), "{forbidden}");
    }
    let reference = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(reference.contains("impl ReferenceGroupProofs for RecoveredGroupHost"));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_recovery.rs"))
            .unwrap();
    assert!(adapter.contains("GroupRecoveryHost<RecoveredMergeBranch, str>"));
    assert!(adapter.contains("inspect_group_recovery("));
    assert!(
        reference
            .contains("#[cfg(all(test, feature = \"sqlite-journal\"))]\nfn recover_group_receipt(")
    );
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/group_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 374)"));
    assert!(frozen.contains("assert_eq!(snapshot(&tx), before)"));
    assert!(!frozen.contains("inspect_group_recovery("));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-runtime-core/tests/recovery_fence.rs"))
            .unwrap();
    for required in [
        "complete_group_recovery_preserves_state_matrix_short_circuit_and_original_receipt",
        "group_recovery_keeps_each_native_failure_owned_redacted_and_stops_before_later_gates",
        "group_recovery_unwind_at_every_hook_never_retries_or_returns_a_partial_receipt",
        "native_checkpoint_publishes_only_after_group_records_and_commit_with_distinct_native_keys",
    ] {
        assert!(consumer.contains(required), "{required}");
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/group_recovery.rs"));
    assert!(exporter.contains("consumer/tests/recovery_fences.rs"));
}

#[test]
fn native_chain_recovery_delegates_order_not_policy_or_execution() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let code =
        fs::read_to_string(root.join("crates/leselang-vm/src/journal/successor.rs")).unwrap();
    let chain = code.split("pub(super) fn validate_chain(").nth(1).unwrap();
    for required in [
        "ORDER BY b.position",
        "leselang_runtime_core::inspect_successor_chain(",
        "crate::MAX_MERGE_BRANCHES",
        "previous.1.as_deref().ok_or_else(invalid)?",
        "MAX_JOURNAL_ENTRY_BYTES",
        "validate_pair(parent, &result, &next)?",
        "SuccessorChainError::Native { error, .. } => error",
        "_ => invalid()",
    ] {
        assert!(
            chain.contains(required),
            "missing native boundary: {required}"
        );
    }
    assert!(
        chain.find("let result: Step = decode_bounded(").unwrap()
            < chain
                .find("let next = request_for(connection, &following.0)?")
                .unwrap()
    );
    for absent in [
        "windows(2)",
        "Fuel::new",
        "resume(",
        "commit()",
        "advance_sequence_watermark",
    ] {
        assert!(
            !chain.contains(absent),
            "chain wrapper grew behavior: {absent}"
        );
    }
    let core = fs::read_to_string(root.join("crates/leselang-runtime-core/src/successor_chain.rs"))
        .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent), "chain core grew policy: {absent}");
    }
    let journal = fs::read_to_string(root.join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("successor_chain_inspection_preserves_frozen_sqlite_fault_order"));
    let exporter = fs::read_to_string(root.join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/successor_chain.rs"));
}

#[test]
fn sqlite_reservation_and_completion_fences_are_shared_without_repair_or_execution_authority() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let graph = code
        .split("fn validate_merge_graph_records(")
        .nth(1)
        .unwrap()
        .split("fn advance_sequence_watermark(")
        .next()
        .unwrap();
    for boundary in [
        "inspect_group_batch(groups, &mut RecoveredGroupsHost(connection))",
        "inspect_group_record(",
        "validate_unbound_group_receipt(self.plan, self.branches, terminal)",
        "validate_terminal_step(&terminal)?",
        "group_binding::validate_records(self.connection, self.token, self.plan, terminal)",
    ] {
        assert!(graph.contains(boundary), "native graph lost {boundary}");
    }
    assert!(!graph.contains("advance_sequence_watermark("));
    assert!(!graph.contains("resume_group("));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_recovery.rs"))
            .unwrap();
    for boundary in [
        "inspect_group_recovery(",
        "GroupRecoveryError::Receipt(GroupReceiptError::Native(error)) => error",
        "GroupRecoveryError::Receipt(GroupReceiptError::State) => invalid_group_receipt_state()",
    ] {
        assert!(adapter.contains(boundary), "adapter lost {boundary}");
    }
    assert_complete_group_batch_order();
    assert_complete_group_recovery_order();
    let bound =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/group_binding.rs")).unwrap();
    let reservations = fs::read_to_string(
        root().join("crates/leselang-command/src/reference_group_reservations.rs"),
    )
    .unwrap();
    for boundary in [
        "validate_reservation_proof(",
        "allocation_watermark: u64::try_from(next_sequence).ok()",
        "record_exists: exists",
        "aliases_owner: aliases_group",
        "index < created",
        ".checked_sub(",
    ] {
        assert!(reservations.contains(boundary));
    }
    for boundary in [
        "inspect_group_reservations(plan, &mut GroupReservationSource(self.0))",
        "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
        "SELECT EXISTS(SELECT 1 FROM vm_effects WHERE token = ?1)",
        "SELECT EXISTS(SELECT 1 FROM vm_merge_groups WHERE token = ?1)",
    ] {
        assert!(bound.contains(boundary));
    }
    let records =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_bound_records.rs"))
            .unwrap();
    let graph = records
        .find("proofs.validate_graph(token, plan, &graph)?")
        .unwrap();
    let prefix = records.find("&& successful_prefix !=").unwrap();
    let reservation_index = records.find("proofs.validate_reservations(plan)?").unwrap();
    let receipt = records.find("    validate_bound_group_receipt(").unwrap();
    assert!(graph < prefix && prefix < reservation_index && reservation_index < receipt);
    assert!(!bound.contains("advance_sequence_watermark("));
    for forbidden in [
        "rusqlite",
        "advance_sequence_watermark(",
        "Fuel::new",
        "commit()",
    ] {
        assert!(!reservations.contains(forbidden));
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/recovery_fence.rs"))
            .unwrap();
    for absent in [
        "leserpent",
        "rusqlite",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "impl Default",
    ] {
        assert!(
            !core.contains(absent),
            "recovery fence grew policy: {absent}"
        );
    }
    for proof in [
        "group_receipt_recovery_preserves_frozen_state_and_decode_fault_precedence",
        "cross_group_reservation_refusal_blocks_snapshot_and_imported_watermark_repair",
        "cold_slot_allocation_proof_uses_original_watermark_before_high_identity_repair",
    ] {
        assert!(
            code.contains(proof)
                || fs::read_to_string(root().join("crates/leselang-vm/tests/group_tail.rs"))
                    .unwrap()
                    .contains(proof)
        );
    }
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/recovery_fences.rs")
    );
}

#[test]
fn native_unbound_receipt_correspondence_uses_original_data_merge_without_evaluation() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_recovery.rs"))
            .unwrap();
    let gate = adapter
        .split("fn admit_receipt(")
        .nth(1)
        .unwrap()
        .split("fn validate_records(")
        .next()
        .unwrap();
    assert!(
        gate.find("validate_terminal_step(&terminal)?").unwrap()
            < gate
                .find("self.proofs.validate_unbound_receipt(&terminal)?")
                .unwrap()
    );
    assert!(code.contains(
        "use leselang_command::reference_group_recovery::validate_unbound_group_receipt;"
    ));
    assert!(!code.contains("fn validate_unbound_group_receipt("));
    let correspondence = adapter
        .split("fn validate_unbound_group_receipt(")
        .nth(1)
        .unwrap()
        .split("fn invalid_graph(")
        .next()
        .unwrap();
    for required in [
        "if plan.result_binding.is_some()",
        "return Ok(())",
        "branches.len() != plan.branches.len()",
        "branches.len() > MAX_MERGE_BRANCHES",
        "decode_journal_value::<Step>(",
        "MAX_JOURNAL_ENTRY_BYTES",
        "sequence_stop(steps.iter())?",
        "terminal_step_outcome(step)?",
        "plan.order.has_group_successor()",
        "crate::reference_merge_output::assemble_merge(",
        "crate::reference_merge_output::terminal_merge(plan, result)?",
        "if &expected != terminal",
        "merge group receipt does not match original branch results",
    ] {
        assert!(
            correspondence.contains(required),
            "receipt correspondence lost {required}"
        );
    }
    for absent in [
        "Fuel::new",
        "resume_captured_effects",
        ".finish(value)",
        "restore_request",
        "advance_sequence_watermark(",
        "execute(",
    ] {
        assert!(!correspondence.contains(absent));
    }
    let actual = code
        .split("fn validate_merge_graph_records(")
        .nth(1)
        .unwrap()
        .split("fn recover_group_receipt(")
        .next()
        .unwrap();
    assert!(
        actual.find("inspect_group_record(").unwrap()
            < actual.find("validate_unbound_group_receipt(").unwrap()
    );
    assert!(
        actual.find("validate_unbound_group_receipt(").unwrap()
            < actual.find("group_binding::validate_records(").unwrap()
    );
    assert_complete_group_recovery_order();
    assert!(code.contains(
        "group_receipt_native_admission_runs_once_after_state_decode_and_terminal_validation"
    ));
    let tests =
        fs::read_to_string(root().join("crates/leselang-vm/tests/group_receipts.rs")).unwrap();
    for proof in [
        "unbound_completed_receipt_must_match_original_members_before_watermark_repair",
        "original_field_values_names_and_declared_order_are_not_interchangeable",
        "structurally_valid_fault_cannot_replace_success_or_change_original_refusal",
        "original_cancellation_and_failure_metadata_are_part_of_receipt_correspondence",
        "parallel_failure_choice_follows_declared_order_not_completion_chronology",
    ] {
        assert!(tests.contains(proof));
    }
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/Cargo.toml"))
            .unwrap()
            .contains("name = \"group_receipts\"\nrequired-features = [\"sqlite-journal\"]")
    );
}

#[test]
fn actual_group_request_and_recovery_graphs_use_the_shared_bounded_identity_kernel() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let input = code
        .split("fn validate_merge_graph_input(")
        .nth(1)
        .unwrap()
        .split("fn validate_merge_graph_records(")
        .next()
        .unwrap();
    for boundary in [
        "leselang_command::validate_merge_graph(",
        "plan: crate::merge_plan_admission_limits()",
        "frame: crate::continuation_admission_limits()",
        "|request| request.as_view()",
        "&MergeGraphHost",
        "crate::merge_plan_admission_fault(error)",
        "GroupGraphError::Native {",
        "crate::request_admission_fault(error)",
        "crate::group_binding::admission_fault(error)",
    ] {
        assert!(input.contains(boundary), "group admission lost {boundary}");
    }
    assert!(!input.contains("HashSet"));
    let records = code
        .split("fn validate_merge_graph_records(")
        .nth(1)
        .unwrap()
        .split("fn advance_sequence_watermark(")
        .next()
        .unwrap();
    for boundary in [
        "inspect_group_record(",
        "inspect_group_batch(groups, &mut RecoveredGroupsHost(connection))",
        "successor::validate_chain(self.connection, self.token)",
        "validate_terminal_step(&terminal)?",
        "group_binding::validate_records(self.connection, self.token, self.plan, terminal)",
    ] {
        assert!(
            records.contains(boundary),
            "native recovery lost {boundary}"
        );
    }
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_group_recovery.rs"))
            .unwrap();
    for boundary in [
        "inspect_group_recovery(",
        "usize::try_from(branch.1).ok()",
        "identity: branch.4.as_str()",
        "sequence_stop(terminal_steps.iter())?",
    ] {
        assert!(
            policy.contains(boundary),
            "adapter recovery lost {boundary}"
        );
    }
    for proof in [
        "group_graph_admission_preserves_frozen_fault_precedence",
        "late_group_graph_refusal_rolls_back_recovery_without_repairing_watermark",
    ] {
        assert!(code.contains(proof));
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_graph.rs")).unwrap();
    for absent in [
        "leserpent",
        "rusqlite",
        "EffectRequest",
        "Fuel::new",
        "Serialize",
        "Deserialize",
    ] {
        assert!(
            !core.contains(absent),
            "graph kernel grew native policy: {absent}"
        );
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/group_graph.rs"));
}

#[test]
fn complete_merge_admission_is_shared_without_importing_reference_recovery_authority() {
    let shared =
        fs::read_to_string(root().join("crates/leselang-command/src/merge_admission.rs")).unwrap();
    let plan = shared
        .split_once("pub fn validate_merge_plan<Native>")
        .unwrap()
        .1;
    assert!(
        plan.find("plan.validate_shape(").unwrap() < plan.find("validate_group_binding(").unwrap()
    );
    let graph = shared.split_once("pub fn validate_merge_graph<").unwrap().1;
    let mut offset = 0;
    for stage in [
        "validate_merge_plan(",
        "if !host.valid_token(owner)",
        "validate_group_graph(",
        "validate_effect_request(",
        "Ok(GroupGraphMember {",
        "let result_type = validate_group_binding(",
        "validate_preflighted_group_requests(",
        "view(row).continuation.group_result.is_some()",
    ] {
        offset += graph[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("lost ordered gate: {stage}"))
            + stage.len();
    }
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "Fuel::",
        "LSV",
        ".clone()",
        ".collect()",
        "serde_json",
        "impl Default",
    ] {
        assert!(!shared.contains(forbidden), "{forbidden}");
    }
    assert!(shared.contains("Host: MergeGraphAdmissionHost<Token> + ?Sized"));
    assert!(shared.contains("Token: PartialEq"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_request_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 2_880)"));
    assert!(frozen.contains("assert_eq!(comparisons, 14)"));
    let oracle = frozen
        .split_once("fn frozen_graph(")
        .unwrap()
        .1
        .split_once("fn frozen_validate_requests(")
        .unwrap()
        .0;
    assert!(!oracle.contains("leselang_command::validate_merge_graph("));
    assert!(!oracle.contains("leselang_command::validate_merge_plan("));
    assert!(oracle.contains("frozen_validate_effect_request(request)?"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/merge_plan.rs")).unwrap();
    for proof in [
        "complete_graph_borrows_local_opaque_rows_and_rechecks_all_ten_bound_plan_forms",
        "complete_graph_keeps_plan_owner_count_request_identity_and_correspondence_order",
        "complete_graph_preserves_both_owned_encoding_failures_and_each_unwind_prefix",
        "complete_unbound_plan_skips_encoding_and_rejects_bound_members_without_authorizing_them",
    ] {
        assert!(consumer.contains(proof));
    }
    assert!(!consumer.contains("leselang_vm"));
}

#[test]
fn actual_atomic_and_group_saved_bodies_use_the_shared_hir_entry_without_body_replacement() {
    let code =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_computation.rs"))
            .unwrap();
    let reentry = code
        .split("fn resume_captured_outcome")
        .nth(1)
        .unwrap()
        .split("fn charge_projection")
        .next()
        .unwrap();
    for boundary in [
        "crate::resume_preflighted_result_binding_in_scope(",
        "ReferenceCaptureHost",
        "restore_saved_results",
        "effect_outcome(outcome.into_legacy())",
        "calculation_fault(effect_failure(error))",
    ] {
        assert!(
            reentry.contains(boundary),
            "saved execution lost {boundary}"
        );
    }
    assert!(!reentry.contains("body:"));
    assert!(!reentry.contains("evaluate_inner("));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_scope.rs")).unwrap();
    let reentry = adapter
        .split("pub fn resume_preflighted_result_binding_in_scope")
        .nth(1)
        .unwrap();
    for boundary in [
        "resume_captured_effects_in_scope(",
        "CapturedEffectHooks {",
        "host.charge_scalar(value, fuel)?",
        "host.charge_result(result, fuel)?",
        "host.charge_group(group, fuel)?",
        "host.allow_saved_projection()?",
    ] {
        assert!(
            reentry.contains(boundary),
            "adapter reentry lost {boundary}"
        );
    }
    assert!(!reentry.contains("body:"));
    let shared =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_reentry.rs")).unwrap();
    for boundary in [
        "with_captured_scope_state(",
        "evaluate_resumable_effects_in_scope(body, scope, environment, fuel, limits)",
        "original Fuel",
        "after restoration and before body execution",
        "not saved-frame admission or reply acceptance",
    ] {
        assert!(shared.contains(boundary));
    }
    let consumer =
        fs::read_to_string(root().join("crates/leselang-hir/tests/durable_control.rs")).unwrap();
    assert!(consumer.contains("let outcome = resume_captured_effects_in_scope("));
    assert!(!consumer.contains("with_captured_scope("));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/captured_reentry.rs"));
}

#[test]
fn saved_frame_is_shared_data_while_reference_restore_and_authority_remain_native() {
    let code =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(code.contains("pub use leselang_runtime_core::ResourceBudget;"));
    assert!(code.contains("pub use leselang_command::{ContinuationImage, EffectRequest}"));
    let request =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_request.rs"))
            .unwrap();
    for boundary in [
        "pub type ContinuationImage = leselang_runtime_core::ContinuationFrame<",
        "crate::validate_continuation_frame(",
        "budget: ResourceBudgetLimits",
        "ContinuationAdmissionError::Budget(ResourceBudgetError::Fuel)",
        "ContinuationAdmissionError::Budget(ResourceBudgetError::AbsoluteDeadline)",
        "crate::validate_effect_request(",
        "budget: &self.budget,",
    ] {
        assert!(
            request.contains(boundary),
            "reference frame lost {boundary}"
        );
    }
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    assert!(
        correspondence.contains("if *request.budget != request.continuation.resource_budget()")
    );
    for removed in ["pub struct ContinuationImage", "pub struct ResourceBudget"] {
        assert!(!code.contains(removed));
    }
    let core = fs::read_to_string(root().join("crates/leselang-runtime-core/src/continuation.rs"))
        .unwrap();
    for absent in [
        "leserpent",
        "leselang_hir",
        "rusqlite",
        "Fuel::new",
        "impl Default",
    ] {
        assert!(!core.contains(absent), "saved data grew policy: {absent}");
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    for proof in [
        "crates/leselang-runtime-core/tests/continuation.rs",
        "crates/leselang-hir/tests/durable_control.rs",
        "consumer/tests/continuation_frame.rs",
        "consumer/tests/durable_control.rs",
    ] {
        assert!(exporter.contains(proof));
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    for boundary in [
        "Shared Saved Frames And Budgets",
        "mutable unchecked data",
        "consumer-owned",
        "crash-atomic",
        "complete independent",
    ] {
        assert!(docs.contains(boundary));
    }
}

#[test]
fn actual_sqlite_recovery_uses_host_neutral_staging_without_moving_frame_or_authority_policy() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let load = code
        .split("fn load(&mut self)")
        .nth(1)
        .unwrap()
        .split("fn completed_exists")
        .next()
        .unwrap();
    for boundary in [
        "TransactionBehavior::Immediate",
        "recover_checkpoint(SqliteRecoveryFinalizer",
        "snapshot.into_parts()",
    ] {
        assert!(load.contains(boundary), "actual recovery lost {boundary}");
    }
    assert!(!load.contains("let mut pending = BTreeMap::new()"));
    let record = code
        .split("fn recover_effect_record(")
        .nth(1)
        .unwrap()
        .split("fn completed_effect_logical_bytes(")
        .next()
        .unwrap();
    for boundary in [
        "inspect_effect_record(",
        "group_binding::validate_owner(self.connection, image)",
        "successor::validate_record(self.connection, image, completed)",
    ] {
        assert!(record.contains(boundary), "{boundary}");
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_recovery.rs"))
            .unwrap();
    for boundary in [
        "inspect_record_recovery(",
        "validate_continuation_image(&image)?",
        "MAX_CONTINUATION_BYTES",
        "canonical_continuation_sequence(&image.token)",
        "image.token.as_str() != self.token",
        "stored_deadline != image.deadline_at_ms",
        "u64::try_from(value)",
        "self.source.validate_owner(image)",
        "self.source.validate_successor(image, completed)",
        "MAX_JOURNAL_ENTRY_BYTES",
        "validate_terminal_step(&step)?",
    ] {
        assert!(adapter.contains(boundary), "{boundary}");
    }
    let cleanup = adapter.split("fn completed_payload(").nth(1).unwrap();
    assert!(cleanup.find("drop((").unwrap() < cleanup.find("(token, completed)").unwrap());
    assert_complete_record_recovery_order();
    assert_complete_checkpoint_finalization_order();
    assert!(code.contains("recovery_commit_failure_releases_staging_and_rolls_back_the_watermark"));
    assert!(code.contains("recovery_rejects_a_late_invalid_row_without_exposing_the_valid_prefix"));
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/checkpoint.rs")).unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "ContinuationImage",
        "EffectRequest",
        "Serialize",
        "Deserialize",
    ] {
        assert!(!core.contains(absent), "checkpoint core retained {absent}");
    }
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("crates/leselang-runtime-core/tests/checkpoint.rs")
    );
}

fn tree(features: &[&str]) -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "leselang-vm",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
        ])
        .args(features)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn memory_vm_dependency_closure_omits_native_sqlite_without_claiming_product_independence() {
    let dependencies = tree(&["--no-default-features", "--features", "leserpent-profile"]);
    for absent in ["rusqlite", "libsqlite3-sys"] {
        assert!(
            !dependencies.lines().any(|l| l.starts_with(absent)),
            "memory VM retained {absent}"
        );
    }
    for retained in [
        "leselang-hir",
        "leselang-runtime-core",
        "leselang-command",
        "leserpent-domain",
        "leselang-host-contract",
    ] {
        assert!(
            dependencies.lines().any(|l| l.starts_with(retained)),
            "reference VM boundary changed: {retained}"
        );
    }
}

#[test]
fn native_vm_facade_dependency_closure_omits_all_product_storage_and_gui_contracts() {
    let dependencies = tree(&["--no-default-features"]);
    for required in [
        "leselang-vm",
        "leselang-hir",
        "leselang-runtime-core",
        "leselang-syntax",
    ] {
        assert!(dependencies.lines().any(|line| line.starts_with(required)));
    }
    for forbidden in [
        "leselang-command",
        "leselang-host-contract",
        "leserpent",
        "silvortex",
        "rusqlite",
        "libsqlite3-sys",
        "tokio",
        "gewyvern",
        "serde_json",
    ] {
        assert!(
            !dependencies.lines().any(|line| line.starts_with(forbidden)),
            "native VM retained {forbidden}"
        );
    }
    let root_source = fs::read_to_string(root().join("crates/leselang-vm/src/lib.rs")).unwrap();
    assert!(
        root_source.contains(
            "#[cfg(feature = \"leserpent-profile\")]\ninclude!(\"leserpent_profile.rs\");"
        )
    );
    let native = fs::read_to_string(root().join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(
        native.contains("pub use leselang_hir::effect_stack_session::StackedControlSession as Vm;")
    );
    assert!(!native.contains("fn poll"));
    assert!(!native.contains("include!(\"leserpent_profile"));
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(
        workflow.contains("cargo +1.98.0 test -p leselang-vm --locked --no-default-features\n")
    );
    assert!(workflow.contains("cargo +1.98.0 clippy -p leselang-vm --locked --no-default-features --all-targets -- -D warnings"));
    assert!(workflow.contains(
        "rustfmt +1.98.0 --edition 2024 --check crates/leselang-vm/src/leserpent_profile.rs"
    ));
}

#[test]
fn default_and_explicit_sqlite_features_keep_the_durable_backend_and_released_api() {
    for flags in [
        &[][..],
        &["--no-default-features", "--features", "sqlite-journal"][..],
    ] {
        let dependencies = tree(flags);
        assert!(dependencies.contains("rusqlite v"));
        assert!(dependencies.contains("libsqlite3-sys v"));
    }
    let manifest = fs::read_to_string(root().join("crates/leselang-vm/Cargo.toml")).unwrap();
    assert!(manifest.contains("default = [\"sqlite-journal\"]"));
    assert!(manifest.contains("sqlite-journal = [\"leserpent-profile\", \"dep:rusqlite\"]"));
    assert!(manifest.contains("rusqlite = { workspace = true, optional = true }"));
    let code =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(code.contains("Journal::ephemeral,"));
    for method in ["open_journal(", "open_journal_with_limits("] {
        assert!(code.contains(&format!(
            "#[cfg(feature = \"sqlite-journal\")]\n    pub fn {method}"
        )));
    }
    assert!(code.contains("must-not-fall-back.sqlite3"));
    assert!(code.contains("let (journal, snapshot) = Journal::open(path.as_ref())?"));
}

#[test]
fn only_database_dependent_integration_suites_require_sqlite_and_memory_verticals_stay_enabled() {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--no-deps",
        ])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(output.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let vm = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "leselang-vm")
        .unwrap();
    let mut durable = 0;
    let mut memory = 0;
    let mut native = 0;
    for target in vm["targets"].as_array().unwrap() {
        if target["kind"] != serde_json::json!(["test"]) {
            continue;
        }
        let source = fs::read_to_string(target["src_path"].as_str().unwrap()).unwrap();
        if source.contains("open_journal") || source.contains("rusqlite") {
            assert_eq!(
                target["required-features"],
                serde_json::json!(["sqlite-journal"])
            );
            durable += 1;
        } else if matches!(
            target["name"].as_str(),
            Some("native_embedding" | "native_host_pipeline")
        ) {
            assert!(
                target["required-features"].is_null()
                    || target["required-features"] == serde_json::json!([])
            );
            assert!(source.contains("leselang_vm::native"));
            assert!(!source.contains("leserpent_domain"));
            native += 1;
        } else {
            assert_eq!(
                target["required-features"],
                serde_json::json!(["leserpent-profile"])
            );
            memory += 1;
        }
    }
    assert_eq!(durable, 37);
    assert_eq!(memory, 16);
    assert_eq!(native, 2);
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(!proof.contains("#[cfg("));
    assert!(!proof.contains("std::fs"));
    for name in [
        "memory_profile_executes_typed_helpers_and_preserves_legacy_scalar_wire",
        "memory_profile_seq_keeps_reply_gates_one_budget_and_once_only_completion",
        "memory_profile_all_reenters_captured_successor_only_after_complete_original_barrier",
        "memory_profile_fences_forged_images_wrong_reply_types_and_independent_local_roots",
        "memory_profile_cancel_or_deadline_seals_captured_tail_and_replays_terminal_receipt",
        "memory_profile_retry_backpressure_and_compaction_keep_original_dispatch_authority",
    ] {
        assert!(proof.contains(name));
    }
}

#[test]
fn ci_and_documentation_distinguish_database_free_reference_vm_from_independent_language_vm() {
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("cargo +1.98.0 test -p leselang-vm --locked --no-default-features --features leserpent-profile"));
    assert!(workflow.contains("cargo +1.98.0 clippy -p leselang-vm --locked --no-default-features --features leserpent-profile --all-targets -- -D warnings"));
    assert!(workflow.contains("cargo test --workspace --all-features --locked"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Optional Reference VM Storage")
        .nth(1)
        .unwrap();
    for boundary in [
        "optional VM SQLite backend",
        "default-features = false",
        "no silent memory fallback",
        "Cargo features are additive",
        "not a capability or security boundary",
        "still depends on Leserpent command/result contracts",
        "not the complete product-free independent VM",
    ] {
        assert!(
            section.contains(boundary),
            "VM storage docs lost {boundary}"
        );
    }
}
