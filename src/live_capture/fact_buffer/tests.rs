use super::*;
use crate::ledger::{FactId, PacketDir, SessionId, UdpOffloadSegment, invalid_groups};
use crate::packet_capture::{CapturedUdpDatagram, PacketObservation};
use std::time::SystemTime;

fn fact(group: u64, index: u16) -> FactEnvelope {
    fact_with_count(group, index, 4)
}

fn fact_with_count(group: u64, index: u16, count: u16) -> FactEnvelope {
    let len = if index + 1 == count { 1 } else { 8 };
    let bytes = vec![index as u8; len];
    CapturedUdpDatagram {
        source: "192.0.2.1:4000".parse().unwrap(),
        destination: "192.0.2.2:53".parse().unwrap(),
        ip_header_len: 20,
        total_len: 28 + len as u32,
        payload_len: len as u16,
        payload_prefix: &bytes,
    }
    .into_offload_segment_fact(
        PacketObservation {
            id: FactId(u64::from(index) + 1),
            session: SessionId(1),
            ts: SystemTime::UNIX_EPOCH,
            cpu: 0,
            ifindex: 2,
            netns: 1,
            socket_cookie: 42,
            direction: PacketDir::Egress,
        },
        UdpOffloadSegment {
            aggregate_id: group,
            aggregate_payload_len: u32::from(count - 1) * 8 + 1,
            segment_size: 8,
            segment_index: index,
            segment_count: count,
            ip_header_len: 20,
        },
    )
    .unwrap()
}

fn ordinary() -> FactEnvelope {
    let mut fact = fact(1, 0);
    let FactKind::PacketMeta(packet) = &mut fact.kind else {
        panic!()
    };
    packet.udp_offload = None;
    fact
}

struct Harness {
    buffer: CaptureFactBuffer,
    received: u64,
    counts: FactBufferAccounting,
}
impl Harness {
    fn new(limit: usize) -> Self {
        Self {
            buffer: CaptureFactBuffer::new(limit).unwrap(),
            received: 0,
            counts: Default::default(),
        }
    }
    fn account(&mut self, delta: FactBufferAccounting) {
        self.counts.retained += delta.retained;
        self.counts.userspace_dropped += delta.userspace_dropped;
        self.counts.decode_rejected += delta.decode_rejected;
        assert_eq!(
            self.received,
            self.counts.retained
                + self.counts.userspace_dropped
                + self.counts.decode_rejected
                + self.buffer.pending_records() as u64
        );
        assert!(self.buffer.facts.len() + self.buffer.reserved <= self.buffer.max_facts);
        assert!(self.buffer.entries.len() <= UDP_OFFLOAD_ID_WINDOW as usize);
        assert_eq!(
            self.buffer.deadlines.len(),
            self.buffer
                .entries
                .values()
                .filter(|entry| matches!(entry, Entry::Pending(_)))
                .count()
        );
    }
    fn push(&mut self, fact: FactEnvelope, now: Instant) {
        self.received += 1;
        let delta = self.buffer.push(fact, now);
        self.account(delta);
    }
    fn finish(&mut self) {
        let delta = self.buffer.finish();
        self.account(delta);
        assert_eq!(self.buffer.reserved, 0);
        assert_eq!(self.buffer.pending_records(), 0);
    }
}

#[test]
fn group_waits_across_windows_and_callback_order() {
    let mut h = Harness::new(5);
    let now = Instant::now();
    h.push(fact(1, 3), now);
    h.push(fact(1, 1), now);
    h.push(ordinary(), now);
    let first = h.buffer.take_facts();
    assert_eq!(first.len(), 1);
    assert_eq!(h.buffer.pending_records(), 2);
    h.push(fact(1, 0), now + Duration::from_millis(500));
    assert!(h.buffer.take_facts().is_empty());
    h.push(fact(1, 2), now + Duration::from_millis(501));
    let group = h.buffer.take_facts();
    assert_eq!(group.len(), 4);
    assert!(invalid_groups(&group).is_empty());
    h.finish();
    assert_eq!(h.counts.retained, 5);
}

#[test]
fn reservation_and_budget_drop_are_atomic_for_the_group() {
    let now = Instant::now();
    let mut h = Harness::new(4);
    h.push(fact(1, 0), now);
    h.push(ordinary(), now);
    for index in 1..4 {
        h.push(fact(1, index), now);
    }
    assert_eq!(h.buffer.take_facts().len(), 4);
    assert_eq!(h.counts.userspace_dropped, 1);
    for _ in 0..2 {
        h.push(ordinary(), now);
    }
    h.push(fact(2, 0), now);
    assert_eq!(h.buffer.take_facts().len(), 2);
    // Freed window capacity must not resurrect the refused batch's tail.
    for index in 1..4 {
        h.push(fact(2, index), now);
    }
    assert!(h.buffer.take_facts().is_empty());
    h.finish();
    assert_eq!(
        h.counts,
        FactBufferAccounting {
            retained: 6,
            userspace_dropped: 5,
            decode_rejected: 0
        }
    );
    let mut tiny = Harness::new(1);
    for index in 0..4 {
        tiny.push(fact(1, index), now);
    }
    tiny.finish();
    assert_eq!(tiny.counts.userspace_dropped, 4);
    assert!(tiny.buffer.take_facts().is_empty());
}

#[test]
fn duplicate_index_or_fact_id_poison_pending_group() {
    let now = Instant::now();
    for same_index in [false, true] {
        let mut h = Harness::new(8);
        h.push(fact(1, 0), now);
        let mut duplicate = fact(1, if same_index { 0 } else { 1 });
        duplicate.id = FactId(1);
        h.push(duplicate, now);
        for index in 1..4 {
            h.push(fact(1, index), now);
        }
        assert!(h.buffer.take_facts().is_empty());
        h.finish();
        assert_eq!(h.counts.decode_rejected, 5);
    }
}

#[test]
fn conflicting_identity_metadata_or_invalid_record_poison_group() {
    let now = Instant::now();
    for case in 0..7 {
        let mut h = Harness::new(8);
        h.push(fact(1, 0), now);
        let mut bad = fact(1, 1);
        let FactKind::PacketMeta(packet) = &mut bad.kind else {
            panic!()
        };
        match case {
            0 => bad.session = SessionId(2),
            1 => bad.ts += Duration::from_millis(1),
            2 => bad.ifindex = Some(3),
            3 => packet.sk_cookie = Some(43),
            4 => packet.dir = PacketDir::Ingress,
            5 => packet.udp_offload.as_mut().unwrap().aggregate_payload_len = 26,
            6 => packet.payload_bytes.clear(),
            _ => unreachable!(),
        }
        h.push(bad, now);
        for index in 2..4 {
            h.push(fact(1, index), now);
        }
        assert!(h.buffer.take_facts().is_empty());
        h.finish();
        assert_eq!(h.counts.decode_rejected, 4, "case {case}");
    }
}

#[test]
fn timeout_releases_reservation_and_never_resurrects_group() {
    let mut h = Harness::new(4);
    let now = Instant::now();
    h.push(fact(1, 0), now);
    h.push(fact(1, 1), now + Duration::from_millis(1500));
    let delta = h
        .buffer
        .expire(now + UDP_OFFLOAD_WAIT - Duration::from_nanos(1));
    h.account(delta);
    assert_eq!(h.buffer.pending_records(), 2);
    let delta = h.buffer.expire(now + UDP_OFFLOAD_WAIT);
    h.account(delta);
    assert_eq!(h.buffer.reserved, 0);
    for index in 2..4 {
        h.push(fact(1, index), now + UDP_OFFLOAD_WAIT);
    }
    for index in 0..4 {
        h.push(fact(2, index), now + UDP_OFFLOAD_WAIT);
    }
    assert_eq!(h.buffer.take_facts().len(), 4);
    h.finish();
    assert_eq!(h.counts.decode_rejected, 4);
    assert_eq!(h.counts.retained, 4);
}

#[test]
fn two_and_sixteen_segment_masks_admit_complete_reversed_groups() {
    let mut h = Harness::new(16);
    let now = Instant::now();
    for count in [2, 16] {
        for index in (0..count).rev() {
            h.push(fact_with_count(u64::from(count), index, count), now);
        }
        let group = h.buffer.take_facts();
        assert_eq!(group.len(), usize::from(count));
        assert!(invalid_groups(&group).is_empty());
    }
    h.finish();
    assert_eq!(h.counts.retained, 18);
}

#[test]
fn shutdown_rejects_only_received_partial_records_and_is_idempotent() {
    let mut h = Harness::new(12);
    let now = Instant::now();
    for index in 0..4 {
        h.push(fact(1, index), now);
    }
    h.push(fact(2, 3), now);
    h.push(fact(3, 0), now);
    h.finish();
    assert_eq!(
        h.counts,
        FactBufferAccounting {
            retained: 4,
            userspace_dropped: 0,
            decode_rejected: 2
        }
    );
    assert_eq!(h.buffer.take_facts().len(), 4);
    h.finish();
    h.push(fact(4, 0), now);
    assert_eq!(h.counts.decode_rejected, 3);
    assert!(h.buffer.take_facts().is_empty());
}

#[test]
fn history_is_bounded_and_closed_groups_cannot_reenter_later_windows() {
    let mut h = Harness::new(4);
    let now = Instant::now();
    for group in 1..=UDP_OFFLOAD_ID_WINDOW + 5 {
        for index in 0..4 {
            h.push(fact(group, index), now);
        }
        assert_eq!(h.buffer.take_facts().len(), 4);
    }
    for group in [1, UDP_OFFLOAD_ID_WINDOW + 5] {
        for index in 0..4 {
            h.push(fact(group, index), now);
        }
    }
    assert!(h.buffer.take_facts().is_empty());
    assert_eq!(h.counts.decode_rejected, 8);
    h.finish();
    assert!(h.buffer.entries.is_empty());
}

#[test]
fn watermark_evicts_partial_group_and_accepts_bounded_reordering() {
    let mut h = Harness::new(8);
    let now = Instant::now();
    h.push(fact(1, 0), now);
    h.push(fact(UDP_OFFLOAD_ID_WINDOW + 1, 0), now);
    assert_eq!(h.counts.decode_rejected, 1);
    for index in 1..4 {
        h.push(fact(1, index), now);
    }
    for index in 0..4 {
        h.push(fact(UDP_OFFLOAD_ID_WINDOW, index), now);
    }
    for index in 1..4 {
        h.push(fact(UDP_OFFLOAD_ID_WINDOW + 1, index), now);
    }
    assert_eq!(h.buffer.take_facts().len(), 8);
    assert_eq!(h.counts.decode_rejected, 4);
    h.finish();
}

#[test]
fn ordinary_packets_remain_bounded_and_u64_id_edges_do_not_overflow() {
    assert!(CaptureFactBuffer::new(0).is_err());
    assert!(CaptureFactBuffer::new(65537).is_err());
    let mut h = Harness::new(4);
    let now = Instant::now();
    for _ in 0..5 {
        h.push(ordinary(), now);
    }
    assert_eq!(h.buffer.take_facts().len(), 4);
    assert_eq!(h.counts.userspace_dropped, 1);
    for index in 0..4 {
        h.push(fact(u64::MAX, index), now);
    }
    assert_eq!(h.buffer.take_facts().len(), 4);
    h.push(fact(1, 0), now);
    h.finish();
    assert_eq!(h.counts.decode_rejected, 1);
}
