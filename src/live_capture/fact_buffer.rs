//! Bounded, whole-group admission before service-window publication.
use super::CaptureError;
use crate::ledger::{FactEnvelope, FactKind, same_group};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

pub const UDP_OFFLOAD_ID_WINDOW: u64 = 4096;
pub const UDP_OFFLOAD_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FactBufferAccounting {
    pub retained: u64,
    pub userspace_dropped: u64,
    pub decode_rejected: u64,
}

struct Pending {
    facts: Vec<FactEnvelope>,
    count: usize,
    seen: u16,
    deadline: Instant,
}
enum Entry {
    Pending(Pending),
    Closed { budget: bool },
}

/// One instance per capture run. Aggregate IDs are monotonic allocations with
/// bounded out-of-order arrival, not cross-session or cross-producer identifiers.
/// Pending groups survive `take_facts`; all slots are reserved on first arrival.
pub struct CaptureFactBuffer {
    facts: Vec<FactEnvelope>,
    max_facts: usize,
    reserved: usize,
    highest: u64,
    entries: BTreeMap<u64, Entry>,
    deadlines: BTreeSet<(Instant, u64)>,
    finished: bool,
}

impl CaptureFactBuffer {
    pub fn new(max_facts: usize) -> Result<Self, CaptureError> {
        if !(1..=65536).contains(&max_facts) {
            return Err(CaptureError::new(
                "capture_options_invalid",
                "1..65536 facts required",
            ));
        }
        Ok(Self {
            facts: Vec::with_capacity(max_facts),
            max_facts,
            reserved: 0,
            highest: 0,
            entries: BTreeMap::new(),
            deadlines: BTreeSet::new(),
            finished: false,
        })
    }

    pub fn pending_records(&self) -> usize {
        self.entries
            .values()
            .map(|entry| match entry {
                Entry::Pending(pending) => pending.facts.len(),
                Entry::Closed { .. } => 0,
            })
            .sum()
    }

    pub fn take_facts(&mut self) -> Vec<FactEnvelope> {
        // Idle publications must not replace a full-sized ready allocation.
        if self.facts.is_empty() {
            return Vec::new();
        }
        std::mem::replace(&mut self.facts, Vec::with_capacity(self.max_facts))
    }

    pub fn push(&mut self, fact: FactEnvelope, now: Instant) -> FactBufferAccounting {
        let mut delta = self.expire(now);
        if self.finished {
            delta.decode_rejected += 1;
            return delta;
        }
        let provenance = match &fact.kind {
            FactKind::PacketMeta(packet) => packet.udp_offload,
            _ => None,
        };
        let Some(segment) = provenance else {
            if self.facts.len() + self.reserved < self.max_facts {
                self.facts.push(fact);
                delta.retained += 1;
            } else {
                delta.userspace_dropped += 1;
            }
            return delta;
        };
        let id = segment.aggregate_id;
        let valid =
            matches!(&fact.kind, FactKind::PacketMeta(packet) if segment.matches_packet(packet));
        if !valid {
            self.close_pending(id, &mut delta);
            delta.decode_rejected += 1;
            return delta;
        }
        self.highest = self.highest.max(id);
        let floor = self.highest.saturating_sub(UDP_OFFLOAD_ID_WINDOW - 1);
        while self
            .entries
            .first_key_value()
            .is_some_and(|(key, _)| *key < floor)
        {
            let Some((key, entry)) = self.entries.pop_first() else {
                break;
            };
            self.discard(entry, key, &mut delta);
        }
        if id < floor {
            delta.decode_rejected += 1;
            return delta;
        }
        if let Some(Entry::Closed { budget }) = self.entries.get(&id) {
            if *budget {
                delta.userspace_dropped += 1;
            } else {
                delta.decode_rejected += 1;
            }
            return delta;
        }
        if !self.entries.contains_key(&id) {
            let count = usize::from(segment.segment_count);
            if count > self.max_facts - self.facts.len() - self.reserved {
                self.entries.insert(id, Entry::Closed { budget: true });
                delta.userspace_dropped += 1;
                return delta;
            }
            let Some(deadline) = now.checked_add(UDP_OFFLOAD_WAIT) else {
                self.entries.insert(id, Entry::Closed { budget: false });
                delta.decode_rejected += 1;
                return delta;
            };
            self.reserved += count;
            self.deadlines.insert((deadline, id));
            self.entries.insert(
                id,
                Entry::Pending(Pending {
                    facts: Vec::with_capacity(count),
                    count,
                    seen: 0,
                    deadline,
                }),
            );
        }
        let Some(Entry::Pending(pending)) = self.entries.get_mut(&id) else {
            delta.decode_rejected += 1;
            return delta;
        };
        if pending.seen & (1 << segment.segment_index) != 0
            || pending.facts.iter().any(|previous| previous.id == fact.id)
            || pending
                .facts
                .first()
                .is_some_and(|first| !same_group(first, &fact))
        {
            self.close_pending(id, &mut delta);
            delta.decode_rejected += 1;
            return delta;
        }
        pending.seen |= 1 << segment.segment_index;
        pending.facts.push(fact);
        if pending.facts.len() == pending.count {
            let Some(Entry::Pending(mut complete)) = self.entries.remove(&id) else {
                return delta;
            };
            self.reserved -= complete.count;
            self.deadlines.remove(&(complete.deadline, id));
            delta.retained += complete.count as u64;
            self.facts.append(&mut complete.facts);
            self.entries.insert(id, Entry::Closed { budget: false });
        }
        delta
    }

    pub fn expire(&mut self, now: Instant) -> FactBufferAccounting {
        let mut delta = FactBufferAccounting::default();
        while self
            .deadlines
            .first()
            .is_some_and(|(deadline, _)| *deadline <= now)
        {
            let Some((_, id)) = self.deadlines.pop_first() else {
                break;
            };
            self.close_pending(id, &mut delta);
        }
        delta
    }

    pub fn finish(&mut self) -> FactBufferAccounting {
        self.finished = true;
        let mut delta = FactBufferAccounting::default();
        while let Some((id, entry)) = self.entries.pop_first() {
            self.discard(entry, id, &mut delta);
        }
        delta
    }

    fn close_pending(&mut self, id: u64, delta: &mut FactBufferAccounting) {
        if let Some(entry) = self.entries.remove(&id) {
            self.discard(entry, id, delta);
            self.entries.insert(id, Entry::Closed { budget: false });
        }
    }

    fn discard(&mut self, entry: Entry, id: u64, delta: &mut FactBufferAccounting) {
        if let Entry::Pending(pending) = entry {
            self.reserved -= pending.count;
            self.deadlines.remove(&(pending.deadline, id));
            delta.decode_rejected += pending.facts.len() as u64;
        }
    }
}

#[cfg(test)]
mod tests;
