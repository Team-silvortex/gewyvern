use serde_json::{Value, json};

use super::super::ring::Record;
use super::ValidationError;
use super::traffic::byte_at;

pub(super) struct Ledger {
    expected: [(u64, u16, u16, u32); 2],
    totals: [u64; 2],
    offsets: [u64; 2],
    next_tcp: [Option<u32>; 2],
    epoch: usize,
    events: u64,
    prefixes: u64,
    short: u64,
    truncated: u64,
    lengths: [u64; 65],
}

impl Ledger {
    pub(super) fn new(expected: [(u64, u16, u16, u32); 2], totals: [u64; 2], epoch: usize) -> Self {
        Self {
            expected,
            totals,
            offsets: [0; 2],
            next_tcp: [None; 2],
            epoch,
            events: 0,
            prefixes: 0,
            short: 0,
            truncated: 0,
            lengths: [0; 65],
        }
    }

    pub(super) fn observe(&mut self, record: &Record) -> Result<u32, ValidationError> {
        let index = self
            .expected
            .iter()
            .position(|e| e.0 == record.cookie)
            .ok_or_else(|| ValidationError::new("mixed capture included an unselected cookie"))?;
        let expected = self.expected[index];
        let header = u16::from(record.ip_header_len) + u16::from(record.tcp_header_len);
        if record.sport != expected.2
            || record.dport != expected.1
            || record.source != 0x7f000001
            || record.destination != 0x7f000001
            || !(20..=60).contains(&record.ip_header_len)
            || !record.ip_header_len.is_multiple_of(4)
            || !(20..=60).contains(&record.tcp_header_len)
            || !record.tcp_header_len.is_multiple_of(4)
            || record.total_len.checked_sub(header) != Some(record.payload_len)
            || record.payload_len == 0
            || record.prefix_len != record.payload_len.min(64)
            || record.flags & 0x06 != 0
        {
            return Err(ValidationError::new(
                "mixed capture framing or endpoint mismatch",
            ));
        }
        if self.next_tcp[index].is_some_and(|seq| seq != record.seq) {
            return Err(ValidationError::new(
                "mixed capture gap, overlap or retransmission; not reassembled",
            ));
        }
        let offset = self.offsets[index];
        let end = offset
            .checked_add(u64::from(record.payload_len))
            .filter(|end| *end <= self.totals[index])
            .ok_or_else(|| ValidationError::new("mixed capture exceeds expected byte range"))?;
        if record.prefix[..usize::from(record.prefix_len)]
            .iter()
            .enumerate()
            .any(|(i, byte)| *byte != byte_at(offset + i as u64, index, self.epoch))
        {
            return Err(ValidationError::new(
                "mixed captured prefix differs from independent byte oracle",
            ));
        }
        self.offsets[index] = end;
        self.next_tcp[index] = Some(record.seq.wrapping_add(u32::from(record.payload_len)));
        self.events += 1;
        self.prefixes += u64::from(record.prefix_len);
        self.short += u64::from(record.payload_len < 64);
        self.truncated += u64::from(record.payload_len > 64);
        self.lengths[usize::from(record.prefix_len)] += 1;
        Ok(expected.3)
    }

    pub(super) fn finish(&self) -> Result<Value, ValidationError> {
        if self.offsets != self.totals || self.events == 0 {
            return Err(ValidationError::new(
                "mixed capture did not cover both complete response byte ranges",
            ));
        }
        Ok(self.report())
    }

    fn report(&self) -> Value {
        json!({"received_events":self.events,"payload_range_bytes":self.offsets,
            "captured_prefix_bytes":self.prefixes,"short_payload_events":self.short,
            "truncated_payload_events":self.truncated,
            "prefix_lengths_seen":self.lengths.iter().enumerate().filter(|(_, n)| **n > 0)
                .map(|(length, count)| json!({"length":length,"events":count})).collect::<Vec<_>>(),
            "tcp_ranges_contiguous":true,
            "scope":"owned-contiguous-TCP-payload-ranges-and-up-to-64-byte-prefixes;not-full-payload-capture-or-general-reassembly"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(cookie: u64, seq: u32, offset: u64, len: u16) -> Record {
        let id = (cookie - 1) as usize;
        let mut record = Record {
            sequence: 1,
            monotonic_ns: 0,
            cookie,
            cpu: 0,
            ifindex: 1,
            source: 0x7f000001,
            destination: 0x7f000001,
            seq,
            ack: 1,
            sport: 80,
            dport: 1000 + id as u16,
            total_len: 40 + len,
            window: 100,
            payload_len: len,
            prefix_len: len.min(64),
            flags: 0x18,
            ip_header_len: 20,
            tcp_header_len: 20,
            prefix: [0; 64],
        };
        for (i, byte) in record
            .prefix
            .iter_mut()
            .take(usize::from(record.prefix_len))
            .enumerate()
        {
            *byte = byte_at(offset + i as u64, id, 0);
        }
        record
    }
    fn new_ledger() -> Ledger {
        Ledger::new([(1, 1000, 80, 42), (2, 1001, 80, 42)], [71, 8], 0)
    }

    #[test]
    fn split_coalesced_truncated_and_wrapping_ranges_are_not_message_counts() {
        let mut ledger = new_ledger();
        ledger.observe(&packet(1, u32::MAX - 3, 0, 7)).unwrap();
        ledger.observe(&packet(2, 90, 0, 8)).unwrap();
        ledger.observe(&packet(1, 3, 7, 64)).unwrap();
        assert_eq!(ledger.finish().unwrap()["received_events"], 3);
        let mut ledger = new_ledger();
        ledger.observe(&packet(1, 10, 0, 71)).unwrap();
        ledger.observe(&packet(2, 90, 0, 8)).unwrap();
        let report = ledger.finish().unwrap();
        assert_eq!(report["received_events"], 2);
        assert_eq!(report["captured_prefix_bytes"], 72);
        assert_eq!(report["payload_range_bytes"], json!([71, 8]));
    }

    #[test]
    fn invalid_records_leave_the_oracle_unchanged() {
        let mut ledger = new_ledger();
        ledger.observe(&packet(1, 10, 0, 7)).unwrap();
        let good = packet(1, 17, 7, 64);
        for mutation in 0..8 {
            let mut bad = good;
            match mutation {
                0 => bad.seq += 1,
                1 => bad.seq -= 1,
                2 => bad.cookie = 99,
                3 => bad.prefix[9] ^= 1,
                4 => bad.prefix_len = 65,
                5 => bad.dport += 1,
                6 => bad.total_len += 1,
                _ => bad.flags |= 0x04,
            }
            let before = ledger.report();
            assert!(ledger.observe(&bad).is_err());
            assert_eq!(ledger.report(), before);
        }
        assert!(ledger.finish().is_err());
        ledger.observe(&good).unwrap();
        ledger.observe(&packet(2, 90, 0, 8)).unwrap();
        ledger.finish().unwrap();
        assert!(ledger.observe(&packet(2, 98, 8, 1)).is_err());
    }
}
