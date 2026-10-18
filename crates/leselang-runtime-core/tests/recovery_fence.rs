use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CheckpointCommitError, CheckpointRecovery, GroupReceiptError, RecoveryPayload, RecoveryRecord,
    ReservationError, ReservationObservation, ReservationProofError, SequenceReservations,
    StoredGroupState, validate_group_receipt, validate_reservation_proof,
};

mod group_batch {
    use super::*;
    use leselang_runtime_core::{
        GroupRecoveryBatchError as Error, GroupRecoveryBatchHost, GroupRecoveryBatchLimits,
        inspect_group_recovery_batch,
    };

    struct Row {
        id: usize,
        maximum: u64,
        slots: Vec<u64>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Row {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    struct Native(Rc<()>);
    struct Host {
        events: Vec<(usize, &'static str)>,
        failure: Option<(usize, &'static str)>,
        panic_at: usize,
        marker: Rc<()>,
    }
    impl Host {
        fn observe(&mut self, id: usize, stage: &'static str) {
            self.events.push((id, stage));
            assert_ne!(self.events.len(), self.panic_at, "native unwind");
        }
        fn check(&mut self, id: usize, stage: &'static str) -> Result<(), Native> {
            self.observe(id, stage);
            if self.failure == Some((id, stage)) {
                Err(Native(self.marker.clone()))
            } else {
                Ok(())
            }
        }
    }
    impl GroupRecoveryBatchHost<Row> for Host {
        type Group = Row;
        type Error = Native;
        fn admit_group(&mut self, row: Row) -> Result<Row, Native> {
            self.check(row.id, "admit")?;
            Ok(row)
        }
        fn highest_sequence(&mut self, row: &Row) -> u64 {
            self.observe(row.id, "maximum");
            row.maximum
        }
        fn reserved_sequences<'a>(&'a mut self, row: &'a Row) -> impl Iterator<Item = u64> + 'a {
            self.observe(row.id, "slots");
            row.slots
                .iter()
                .copied()
                .inspect(move |_| self.observe(row.id, "next"))
        }
        fn validate_order(&mut self, row: &Row) -> Result<(), Native> {
            self.check(row.id, "order")
        }
        fn inspect_group(&mut self, row: Row) -> Result<(), Native> {
            self.check(row.id, "group")
        }
    }
    fn limits() -> GroupRecoveryBatchLimits {
        GroupRecoveryBatchLimits {
            max_groups: 3,
            max_reservations: 4,
            max_sequence: 100,
        }
    }
    fn host() -> Host {
        Host {
            events: vec![],
            failure: None,
            panic_at: 0,
            marker: Rc::new(()),
        }
    }
    fn rows(drops: &Rc<Cell<usize>>) -> Vec<Row> {
        vec![
            Row {
                id: 0,
                maximum: 30,
                slots: vec![2, 4],
                drops: drops.clone(),
            },
            Row {
                id: 1,
                maximum: 0,
                slots: vec![],
                drops: drops.clone(),
            },
            Row {
                id: 2,
                maximum: 10,
                slots: vec![8, 40],
                drops: drops.clone(),
            },
        ]
    }
    fn events() -> Vec<(usize, &'static str)> {
        let mut events = vec![];
        for id in 0..3 {
            events.extend([(id, "admit"), (id, "maximum"), (id, "slots")]);
            if id != 1 {
                events.extend([(id, "next"), (id, "next")]);
            }
            events.extend([(id, "order"), (id, "group")]);
        }
        events
    }
    #[test]
    fn group_batch_preserves_order_limits_and_global_reservation_summary() {
        let drops = Rc::new(Cell::new(0));
        let mut host = host();
        let summary = inspect_group_recovery_batch(rows(&drops), limits(), &mut host).unwrap();
        assert_eq!(summary.groups, 3);
        assert_eq!(summary.highest_sequence, 40);
        assert_eq!(summary.reservations.count, 4);
        assert_eq!(summary.reservations.highest_sequence, 40);
        assert_eq!(host.events, events());
        assert_eq!(drops.get(), 3);
        host.events.clear();
        let empty = inspect_group_recovery_batch(
            Vec::<Row>::new(),
            GroupRecoveryBatchLimits {
                max_groups: 0,
                max_reservations: 0,
                max_sequence: 0,
            },
            &mut host,
        )
        .unwrap();
        assert_eq!(
            (
                empty.groups,
                empty.highest_sequence,
                empty.reservations.count
            ),
            (0, 0, 0)
        );
        assert!(host.events.is_empty());
        assert!(matches!(
            inspect_group_recovery_batch(
                rows(&drops),
                GroupRecoveryBatchLimits {
                    max_groups: 2,
                    ..limits()
                },
                &mut host
            ),
            Err(Error::Limit)
        ));
        assert!(host.events.is_empty());
        assert_eq!(drops.get(), 6);
    }
    #[test]
    fn group_batch_reservations_reject_duplicates_zero_overflow_and_late_limits_before_order() {
        for (slot, max_count, max_sequence, expected) in [
            (4, 4, 100, ReservationError::Duplicate),
            (0, 4, 100, ReservationError::Sequence),
            (101, 4, 100, ReservationError::Sequence),
            (u64::MAX, 4, u64::MAX, ReservationError::Sequence),
            (40, 3, 100, ReservationError::Limit),
            (4, 3, 100, ReservationError::Limit),
        ] {
            let drops = Rc::new(Cell::new(0));
            let mut rows = rows(&drops);
            rows[2].slots[1] = slot;
            let mut host = host();
            assert!(matches!(inspect_group_recovery_batch(rows,
                GroupRecoveryBatchLimits { max_reservations: max_count, max_sequence, ..limits() },
                &mut host), Err(Error::Reservation(error)) if error == expected));
            assert_eq!(host.events, events()[..17]);
            assert_eq!(drops.get(), 3);
        }
    }
    #[test]
    fn group_batch_earlier_group_fault_precedes_later_duplicate_reservation() {
        let drops = Rc::new(Cell::new(0));
        let mut rows = rows(&drops);
        rows[2].slots = vec![2, 4];
        let mut host = host();
        host.failure = Some((0, "group"));
        assert!(matches!(
            inspect_group_recovery_batch(rows, limits(), &mut host),
            Err(Error::Group(_))
        ));
        assert_eq!(host.events, events()[..7]);
        assert_eq!(drops.get(), 3);
    }

    #[test]
    fn group_batch_native_failures_and_every_unwind_release_unpublished_rows_once() {
        for id in 0..3 {
            for stage in ["admit", "order", "group"] {
                let drops = Rc::new(Cell::new(0));
                let mut host = host();
                host.failure = Some((id, stage));
                let error =
                    inspect_group_recovery_batch(rows(&drops), limits(), &mut host).unwrap_err();
                assert!(std::error::Error::source(&error).is_none());
                assert!(!format!("{error:?} {error}").contains("private"));
                let native = match error {
                    Error::Admission(native) if stage == "admit" => native,
                    Error::Order(native) if stage == "order" => native,
                    Error::Group(native) if stage == "group" => native,
                    _ => panic!("wrong native stage"),
                };
                assert!(Rc::ptr_eq(&native.0, &host.marker));
                let expected = events();
                let count = expected
                    .iter()
                    .position(|entry| *entry == (id, stage))
                    .unwrap()
                    + 1;
                assert_eq!(host.events, expected[..count]);
                assert_eq!(drops.get(), 3);
            }
        }
        for panic_at in 1..=events().len() {
            let drops = Rc::new(Cell::new(0));
            let mut host = host();
            host.panic_at = panic_at;
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    inspect_group_recovery_batch(rows(&drops), limits(), &mut host)
                }))
                .is_err()
            );
            assert_eq!(host.events, events()[..panic_at]);
            assert_eq!(drops.get(), 3);
        }
    }
    #[test]
    fn group_batch_late_failure_blocks_checkpoint_publication_and_commit() {
        for failure in [None, Some((2, "group"))] {
            for fail_commit in [false, true] {
                let drops = Rc::new(Cell::new(0));
                let marker = Rc::new(());
                let mut recovery = CheckpointRecovery::<u32, Rc<()>, ()>::new(1, 100);
                recovery
                    .push(RecoveryRecord {
                        key: 7,
                        sequence: 1,
                        payload: RecoveryPayload::Pending(marker.clone()),
                    })
                    .unwrap();
                let mut host = host();
                host.failure = failure;
                let committed = Cell::new(false);
                let result = recovery.finish(&mut host, |host, _| {
                    let summary = inspect_group_recovery_batch(rows(&drops), limits(), *host)?;
                    if fail_commit {
                        return Err(Error::Group(Native(Rc::new(()))));
                    }
                    committed.set(true);
                    Ok(summary)
                });
                assert_eq!(committed.get(), failure.is_none() && !fail_commit);
                assert_eq!(result.is_ok(), committed.get());
                drop(result);
                assert_eq!(Rc::strong_count(&marker), 1);
                assert_eq!(drops.get(), 3);
            }
        }
    }
}

mod group_pipeline {
    use super::*;
    use leselang_runtime_core::{
        GroupGraphError, GroupGraphMember, GroupRecoveryError as Error, GroupRecoveryHost,
        GroupRecoveryPolicy, inspect_group_recovery,
    };
    use std::hash::{Hash, Hasher};

    struct Key(u64, Rc<()>);
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Key {}
    impl Hash for Key {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.0.hash(state);
        }
    }
    struct Row {
        name: &'static str,
        position: usize,
        key: Key,
        completed: bool,
    }
    struct Owner {
        marker: Rc<()>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Owner {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    struct Native(Owner);
    struct Host {
        state: u8,
        events: Vec<String>,
        fail: Option<&'static str>,
        panic_at: usize,
        error_drops: Rc<Cell<usize>>,
        receipt_marker: Rc<()>,
        saw_receipt: bool,
    }
    impl Host {
        fn new(state: u8, marker: &Rc<()>) -> Self {
            Self {
                state,
                events: vec![],
                fail: None,
                panic_at: 0,
                error_drops: Rc::new(Cell::new(0)),
                receipt_marker: marker.clone(),
                saw_receipt: false,
            }
        }
        fn observe(&mut self, event: String) {
            self.events.push(event);
            assert_ne!(self.events.len(), self.panic_at, "trusted native unwind");
        }
        fn error(&self) -> Native {
            Native(Owner {
                marker: self.receipt_marker.clone(),
                drops: self.error_drops.clone(),
            })
        }
        fn check(&mut self, event: &'static str) -> Result<(), Native> {
            self.observe(event.into());
            if self.fail == Some(event) {
                Err(self.error())
            } else {
                Ok(())
            }
        }
    }
    impl GroupRecoveryHost<Row, Key> for Host {
        type Encoded = Owner;
        type Receipt = Owner;
        type Error = Native;
        fn member<'a>(
            &mut self,
            index: usize,
            row: &'a Row,
        ) -> Result<GroupGraphMember<'a, Key>, Native>
        where
            Key: 'a,
        {
            self.check(if index == 0 { "member0" } else { "member1" })?;
            Ok(GroupGraphMember {
                name: row.name,
                position: Some(row.position),
                identity: &row.key,
            })
        }
        fn is_completed(&mut self, row: &Row) -> bool {
            self.observe(format!("completed{}", row.position));
            row.completed
        }
        fn validate_chain(&mut self) -> Result<(), Native> {
            self.check("chain")
        }
        fn validate_pending_sequence(&mut self, rows: &[Row]) -> Result<(), Native> {
            assert_eq!(rows.len(), 2);
            self.check("sequence")
        }
        fn stored_state(&mut self) -> Result<StoredGroupState, Native> {
            self.check("state")?;
            match self.state {
                0 => Ok(StoredGroupState::Pending),
                1 => Ok(StoredGroupState::Completed),
                _ => Err(self.error()),
            }
        }
        fn admit_receipt(&mut self, receipt: Owner) -> Result<Owner, Native> {
            self.check("receipt")?;
            Ok(receipt)
        }
        fn validate_records(&mut self, receipt: Option<&Owner>) -> Result<(), Native> {
            self.check("records")?;
            if let Some(receipt) = receipt {
                assert!(Rc::ptr_eq(&receipt.marker, &self.receipt_marker));
            }
            self.saw_receipt = receipt.is_some();
            Ok(())
        }
    }
    fn rows(first: bool, second: bool, marker: &Rc<()>) -> [Row; 2] {
        [
            Row {
                name: "left",
                position: 0,
                key: Key(1, marker.clone()),
                completed: first,
            },
            Row {
                name: "right",
                position: 1,
                key: Key(2, marker.clone()),
                completed: second,
            },
        ]
    }
    fn policy() -> GroupRecoveryPolicy {
        GroupRecoveryPolicy {
            max_members: 2,
            require_chain: true,
            require_pending_sequence: true,
        }
    }
    fn input(marker: &Rc<()>, drops: &Rc<Cell<usize>>) -> Owner {
        Owner {
            marker: marker.clone(),
            drops: drops.clone(),
        }
    }

    #[test]
    fn complete_group_recovery_preserves_state_matrix_short_circuit_and_original_receipt() {
        let marker = Rc::new(());
        let owner = Key(0, marker.clone());
        let mut cases = 0;
        for state in 0..3 {
            for first in [false, true] {
                for second in [false, true] {
                    for present in [false, true] {
                        for chain in [false, true] {
                            for sequence in [false, true] {
                                let rows = rows(first, second, &marker);
                                let drops = Rc::new(Cell::new(0));
                                let mut host = Host::new(state, &marker);
                                let object: &mut dyn GroupRecoveryHost<
                                    Row,
                                    Key,
                                    Encoded = Owner,
                                    Receipt = Owner,
                                    Error = Native,
                                > = &mut host;
                                let result = inspect_group_recovery(
                                    &owner,
                                    &["left", "right"],
                                    &rows,
                                    GroupRecoveryPolicy {
                                        require_chain: chain,
                                        require_pending_sequence: sequence,
                                        ..policy()
                                    },
                                    present.then(|| input(&marker, &drops)),
                                    object,
                                );
                                let valid = (state == 0 && !(first && second) && !present)
                                    || (state == 1 && first && second && present);
                                assert_eq!(result.is_ok(), valid);
                                let mut expected = vec!["member0", "member1", "completed0"];
                                if first {
                                    expected.push("completed1");
                                }
                                if chain {
                                    expected.push("chain");
                                }
                                if sequence {
                                    expected.push("sequence");
                                }
                                expected.push("state");
                                if valid {
                                    if present {
                                        expected.push("receipt");
                                    }
                                    expected.push("records");
                                    let receipt = result.unwrap();
                                    assert_eq!(host.saw_receipt, present);
                                    assert_eq!(drops.get(), 0);
                                    if let Some(receipt) = &receipt {
                                        assert!(Rc::ptr_eq(&receipt.marker, &marker));
                                    }
                                    drop(receipt);
                                } else {
                                    drop(result);
                                }
                                assert_eq!(host.events, expected);
                                assert_eq!(drops.get(), usize::from(present));
                                assert!(Rc::ptr_eq(&rows[0].key.1, &marker));
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 96);
    }

    #[test]
    fn group_recovery_graph_refusal_precedes_native_work_and_drops_unobserved_inputs() {
        let marker = Rc::new(());
        let owner = Key(0, marker.clone());
        for defect in 0..7 {
            let mut rows = rows(true, true, &marker);
            let drops = Rc::new(Cell::new(0));
            let mut host = Host::new(1, &marker);
            let mut policy = policy();
            match defect {
                0 => policy.max_members = 65,
                1 => policy.max_members = 1,
                2 => rows[0].position = 1,
                3 => rows[0].name = "wrong",
                4 => rows[0].key.0 = 0,
                5 => rows[1].key.0 = 1,
                6 => host.fail = Some("member0"),
                _ => unreachable!(),
            }
            let error = inspect_group_recovery(
                &owner,
                &["left", "right"],
                &rows,
                policy,
                Some(input(&marker, &drops)),
                &mut host,
            )
            .err()
            .expect("invalid graph admitted");
            assert!(matches!(error, Error::Graph(_)));
            assert_eq!(
                host.events.len(),
                if defect < 2 {
                    0
                } else if defect == 5 {
                    2
                } else {
                    1
                }
            );
            assert_eq!(drops.get(), 1);
        }
    }

    #[test]
    fn group_recovery_keeps_each_native_failure_owned_redacted_and_stops_before_later_gates() {
        let marker = Rc::new(());
        let owner = Key(0, marker.clone());
        let rows = rows(true, true, &marker);
        for fail in [
            "member0", "member1", "chain", "sequence", "state", "receipt", "records",
        ] {
            let drops = Rc::new(Cell::new(0));
            let mut host = Host::new(1, &marker);
            host.fail = Some(fail);
            let error = inspect_group_recovery(
                &owner,
                &["left", "right"],
                &rows,
                policy(),
                Some(input(&marker, &drops)),
                &mut host,
            )
            .err()
            .expect("native failure ignored");
            assert_eq!(host.events.last().unwrap(), fail);
            assert_eq!(drops.get(), 1);
            assert_eq!(format!("{error:?}"), format!("{error}"));
            assert!(!format!("{error:?}").contains("private"));
            assert!(std::error::Error::source(&error).is_none());
            let native = match error {
                Error::Graph(GroupGraphError::Native { error, .. })
                    if fail.starts_with("member") =>
                {
                    error
                }
                Error::Chain(e) if fail == "chain" => e,
                Error::Sequence(e) if fail == "sequence" => e,
                Error::State(e) if fail == "state" => e,
                Error::Receipt(GroupReceiptError::Native(e)) if fail == "receipt" => e,
                Error::Records(e) if fail == "records" => e,
                _ => panic!("native error lost its stage"),
            };
            assert!(Rc::ptr_eq(&native.0.drops, &host.error_drops));
            assert_eq!(host.error_drops.get(), 0);
            drop(native);
            assert_eq!(host.error_drops.get(), 1);
        }
    }

    #[test]
    fn group_recovery_unwind_at_every_hook_never_retries_or_returns_a_partial_receipt() {
        let marker = Rc::new(());
        let owner = Key(0, marker.clone());
        let rows = rows(true, true, &marker);
        let drops = Rc::new(Cell::new(0));
        let mut host = Host::new(1, &marker);
        drop(
            inspect_group_recovery(
                &owner,
                &["left", "right"],
                &rows,
                policy(),
                Some(input(&marker, &drops)),
                &mut host,
            )
            .unwrap(),
        );
        let expected = host.events;
        for cutoff in 1..=expected.len() {
            let mut host = Host::new(1, &marker);
            host.panic_at = cutoff;
            let drops = Rc::new(Cell::new(0));
            assert!(
                catch_unwind(AssertUnwindSafe(|| inspect_group_recovery(
                    &owner,
                    &["left", "right"],
                    &rows,
                    policy(),
                    Some(input(&marker, &drops)),
                    &mut host
                )))
                .is_err()
            );
            assert_eq!(host.events, expected[..cutoff]);
            assert_eq!(drops.get(), 1);
            assert!(!host.saw_receipt);
        }
    }

    #[test]
    fn native_checkpoint_publishes_only_after_group_records_and_commit_with_distinct_native_keys() {
        for domain in ["widgets", "jobs"] {
            for late_failure in [false, true] {
                for commit_failure in [false, true] {
                    let marker = Rc::new(());
                    let drops = Rc::new(Cell::new(0));
                    let staged_drops = Rc::new(Cell::new(0));
                    let committed = Cell::new(false);
                    let owner = Key(0, marker.clone());
                    let rows = rows(true, true, &marker);
                    let mut recovery = CheckpointRecovery::<String, Owner, Owner>::new(1, 10);
                    recovery
                        .push(RecoveryRecord {
                            key: domain.into(),
                            sequence: 1,
                            payload: RecoveryPayload::Completed(input(&marker, &staged_drops)),
                        })
                        .unwrap();
                    let mut host = Host::new(1, &marker);
                    if late_failure {
                        host.fail = Some("records");
                    }
                    let result = recovery.finish((), |_, snapshot| {
                        assert_eq!(snapshot.completed().len(), 1);
                        let receipt = inspect_group_recovery(
                            &owner,
                            &["left", "right"],
                            &rows,
                            policy(),
                            Some(input(&marker, &drops)),
                            &mut host,
                        )?;
                        if commit_failure {
                            return Err(Error::Records(host.error()));
                        }
                        committed.set(true);
                        Ok(receipt)
                    });
                    assert_eq!(result.is_ok(), !late_failure && !commit_failure);
                    assert_eq!(committed.get(), result.is_ok());
                    assert_eq!(staged_drops.get(), usize::from(result.is_err()));
                    if let Ok((snapshot, receipt)) = &result {
                        assert_eq!(snapshot.completed().len(), 1);
                        assert!(Rc::ptr_eq(&receipt.as_ref().unwrap().marker, &marker));
                    }
                    drop(result);
                    assert_eq!(drops.get(), 1);
                    assert_eq!(staged_drops.get(), 1);
                }
            }
        }
    }
}

#[test]
fn complete_cross_group_index_preserves_gaps_full_width_and_inclusive_limits() {
    let mut reservations = SequenceReservations::new(3, u64::MAX);
    for sequence in [19, 2, u64::MAX - 1] {
        reservations.insert(sequence).unwrap();
    }
    let summary = reservations.finish().unwrap();
    assert_eq!(summary.count, 3);
    assert_eq!(summary.highest_sequence, u64::MAX - 1);
    let empty = SequenceReservations::new(0, 0).finish().unwrap();
    assert_eq!(empty.count, 0);
    assert_eq!(empty.highest_sequence, 0);
}

#[test]
fn duplicate_reservation_closes_the_entire_pass_without_overwrite_or_partial_summary() {
    let mut reservations = SequenceReservations::new(3, 100);
    reservations.insert(7).unwrap();
    reservations.insert(9).unwrap();
    assert_eq!(reservations.insert(7), Err(ReservationError::Duplicate));
    assert_eq!(reservations.insert(10), Err(ReservationError::Closed));
    assert_eq!(reservations.finish(), Err(ReservationError::Closed));
}

#[test]
fn count_sequence_and_duplicate_precedence_never_refill_repair_or_wrap() {
    for (count, ceiling, sequence, expected) in [
        (0, 0, 0, ReservationError::Limit),
        (1, 0, 1, ReservationError::Sequence),
        (1, 9, 10, ReservationError::Sequence),
        (1, 100, 0, ReservationError::Sequence),
        (1, u64::MAX, u64::MAX, ReservationError::Sequence),
    ] {
        let mut reservations = SequenceReservations::new(count, ceiling);
        assert_eq!(reservations.insert(sequence), Err(expected));
        assert_eq!(reservations.insert(1), Err(ReservationError::Closed));
        assert_eq!(reservations.finish(), Err(ReservationError::Closed));
    }
    let mut full = SequenceReservations::new(1, 10);
    full.insert(1).unwrap();
    assert_eq!(full.insert(1), Err(ReservationError::Limit));
    assert_eq!(full.finish(), Err(ReservationError::Closed));
}

#[test]
fn original_watermark_presence_and_alias_match_the_frozen_ordered_truth_table() {
    for watermark in [None, Some(0), Some(1), Some(2), Some(100), Some(u64::MAX)] {
        for sequence in [1, 2, 99, u64::MAX - 1] {
            for expected_record in [false, true] {
                for record_exists in [false, true] {
                    for aliases_owner in [false, true] {
                        let original = if watermark.is_none_or(|w| w == 0 || sequence >= w) {
                            Err(ReservationProofError::Watermark)
                        } else if record_exists != expected_record {
                            Err(ReservationProofError::Presence)
                        } else if aliases_owner {
                            Err(ReservationProofError::Alias)
                        } else {
                            Ok(())
                        };
                        assert_eq!(
                            validate_reservation_proof(
                                sequence,
                                expected_record,
                                ReservationObservation {
                                    allocation_watermark: watermark,
                                    record_exists,
                                    aliases_owner
                                }
                            ),
                            original
                        );
                    }
                }
            }
        }
    }
    assert_eq!(
        validate_reservation_proof(
            0,
            true,
            ReservationObservation {
                allocation_watermark: None,
                record_exists: false,
                aliases_owner: true,
            }
        ),
        Err(ReservationProofError::Sequence)
    );
}

#[test]
fn imported_high_identity_cannot_launder_a_cold_reservation_or_repair_original_watermark() {
    let metadata = Cell::new(40);
    let mut recovery = CheckpointRecovery::<u64, (), ()>::new(1, 1000);
    recovery
        .push(RecoveryRecord {
            key: 100,
            sequence: 100,
            payload: RecoveryPayload::Pending(()),
        })
        .unwrap();
    let result = recovery.finish((), |_, snapshot| {
        validate_reservation_proof(
            50,
            false,
            ReservationObservation {
                allocation_watermark: Some(metadata.get()),
                record_exists: false,
                aliases_owner: false,
            },
        )?;
        metadata.set(snapshot.highest_sequence() + 1);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(CheckpointCommitError::Host(
            ReservationProofError::Watermark
        ))
    ));
    assert_eq!(metadata.get(), 40);
}

struct Native {
    id: usize,
    original: Box<[u8]>,
    drops: Rc<RefCell<Vec<usize>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}
fn native(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Native {
    Native {
        id,
        original: b"private-host-receipt".to_vec().into_boxed_slice(),
        drops: drops.clone(),
    }
}

#[test]
fn complete_member_state_and_receipt_presence_gate_the_once_only_native_hook() {
    for state in [StoredGroupState::Pending, StoredGroupState::Completed] {
        for all_completed in [false, true] {
            for present in [false, true] {
                let drops = Rc::new(RefCell::new(Vec::new()));
                let input = present.then(|| native(1, &drops));
                let called = Cell::new(0);
                let result = validate_group_receipt(state, all_completed, input, |input| {
                    called.set(called.get() + 1);
                    Ok::<_, Infallible>(input)
                });
                match (state, all_completed, present) {
                    (StoredGroupState::Pending, false, false) => {
                        assert!(matches!(result, Ok(None)));
                        assert_eq!(called.get(), 0);
                    }
                    (StoredGroupState::Completed, true, true) => {
                        assert!(matches!(&result, Ok(Some(receipt)) if receipt.id == 1));
                        assert_eq!(called.get(), 1);
                        assert!(drops.borrow().is_empty());
                    }
                    _ => {
                        assert!(matches!(result, Err(GroupReceiptError::State)));
                        assert_eq!(called.get(), 0);
                        assert_eq!(drops.borrow().len(), usize::from(present));
                    }
                }
                drop(result);
                assert_eq!(drops.borrow().len(), usize::from(present));
            }
        }
    }
}

#[test]
fn admitted_receipt_keeps_original_move_only_gui_local_payload_and_buffer() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let input = native(1, &drops);
    let pointer = input.original.as_ptr();
    let receipt = validate_group_receipt(StoredGroupState::Completed, true, Some(input), |input| {
        Ok::<_, Native>(input)
    })
    .unwrap()
    .unwrap();
    assert_eq!(receipt.original.as_ptr(), pointer);
    assert!(drops.borrow().is_empty());
    drop(receipt);
    assert_eq!(*drops.borrow(), [1]);
}

#[test]
fn native_receipt_refusal_moves_exact_error_without_formatting_or_source_leaks() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let error = native(2, &drops);
    let pointer = error.original.as_ptr();
    let result = validate_group_receipt(
        StoredGroupState::Completed,
        true,
        Some(native(1, &drops)),
        move |input| {
            drop(input);
            Err::<Native, _>(error)
        },
    )
    .err()
    .unwrap();
    assert_eq!(*drops.borrow(), [1]);
    assert!(!format!("{result:?}: {result}").contains("private-host-receipt"));
    assert!(std::error::Error::source(&result).is_none());
    let GroupReceiptError::Native(error) = result else {
        panic!()
    };
    assert_eq!(error.original.as_ptr(), pointer);
    drop(error);
    assert_eq!(*drops.borrow(), [1, 2]);
}

#[test]
fn uncalled_hook_owner_and_refused_input_clean_up_before_state_error_handoff() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = native(2, &drops);
    let result = validate_group_receipt(
        StoredGroupState::Pending,
        false,
        Some(native(1, &drops)),
        move |_| -> Result<(), ()> {
            let _borrow_complete_owner = &owner;
            panic!("cold hook must not run");
        },
    );
    assert!(matches!(result, Err(GroupReceiptError::State)));
    assert_eq!(*drops.borrow(), [1, 2]);
}

struct PanicOwner {
    native: Native,
}
impl Drop for PanicOwner {
    fn drop(&mut self) {
        panic!("native hook cleanup");
    }
}

#[test]
fn uncalled_hook_cleanup_unwind_cannot_hand_off_a_refused_receipt() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = PanicOwner {
        native: native(2, &drops),
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = validate_group_receipt(
            StoredGroupState::Pending,
            false,
            Some(native(1, &drops)),
            move |_| {
                let _borrow_complete_owner = &owner;
                Ok::<_, ()>(owner.native.id)
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(*drops.borrow(), [1, 2]);
}

#[test]
fn native_receipt_hook_unwind_consumes_input_without_retry_or_fabricated_completion() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let calls = Cell::new(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = validate_group_receipt(
            StoredGroupState::Completed,
            true,
            Some(native(1, &drops)),
            |input| -> Result<(), ()> {
                calls.set(calls.get() + 1);
                let _held = input;
                panic!("native decode unwind");
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(*drops.borrow(), [1]);
}

struct Batch {
    reserved: u64,
    state: StoredGroupState,
    all_completed: bool,
    receipt: Option<Native>,
}

fn native_checkpoint<Key: Ord>(keys: [Key; 2], corrupt: bool) {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let calls = Cell::new(0);
    let commits = Cell::new(0);
    let mut recovery = CheckpointRecovery::<Key, Batch, ()>::new(2, 100);
    for (index, key) in keys.into_iter().enumerate() {
        recovery
            .push(RecoveryRecord {
                key,
                sequence: index as u64 + 1,
                payload: RecoveryPayload::Pending(Batch {
                    reserved: if corrupt { 10 } else { 10 + index as u64 },
                    state: StoredGroupState::Completed,
                    all_completed: true,
                    receipt: Some(native(index + 1, &drops)),
                }),
            })
            .unwrap();
    }
    let result = recovery.finish((), |_, snapshot| {
        let mut reservations = SequenceReservations::new(2, 100);
        for batch in snapshot.pending().values() {
            reservations.insert(batch.reserved)?;
        }
        let summary = reservations.finish()?;
        assert_eq!(summary.count, 2);
        for batch in snapshot.pending().values() {
            validate_reservation_proof(
                batch.reserved,
                false,
                ReservationObservation {
                    allocation_watermark: Some(20),
                    record_exists: false,
                    aliases_owner: false,
                },
            )
            .map_err(|_| ReservationError::Sequence)?;
            validate_group_receipt(
                batch.state,
                batch.all_completed,
                batch.receipt.as_ref(),
                |receipt| {
                    calls.set(calls.get() + 1);
                    assert_eq!(&*receipt.original, b"private-host-receipt");
                    Ok::<_, Infallible>(receipt)
                },
            )
            .map_err(|_| ReservationError::Sequence)?;
        }
        commits.set(commits.get() + 1);
        Ok(())
    });
    if corrupt {
        assert!(matches!(
            result,
            Err(CheckpointCommitError::Host(ReservationError::Duplicate))
        ));
        assert_eq!(calls.get(), 0);
        assert_eq!(commits.get(), 0);
        assert_eq!(drops.borrow().len(), 2);
    } else {
        let (snapshot, ()) = result.unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(commits.get(), 1);
        assert!(drops.borrow().is_empty());
        drop(snapshot);
        assert_eq!(drops.borrow().len(), 2);
    }
}

#[test]
fn unrelated_numeric_and_gui_identity_schemas_share_fences_before_checkpoint_commit() {
    for corrupt in [true, false] {
        native_checkpoint([1u64, 2], corrupt);
        native_checkpoint([[1u8; 16], [2u8; 16]], corrupt);
    }
}
