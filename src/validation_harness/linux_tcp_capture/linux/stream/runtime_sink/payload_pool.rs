use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::BATCH_LIMIT;

#[derive(Default)]
pub(super) struct PayloadPool {
    buffers: Vec<BTreeMap<u16, u8>>,
    reused: u64,
    created: u64,
}

impl PayloadPool {
    pub(super) fn take(&mut self, bytes: &[u8]) -> BTreeMap<u16, u8> {
        if let Some(mut buffer) = self.buffers.pop()
            && buffer.len() == bytes.len()
        {
            let mut contiguous = true;
            for ((offset, value), (index, byte)) in buffer.iter_mut().zip(bytes.iter().enumerate())
            {
                contiguous &= usize::from(*offset) == index;
                *value = *byte;
            }
            if contiguous {
                self.reused += 1;
                return buffer;
            }
        }
        self.created += 1;
        bytes
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u16, *b))
            .collect()
    }

    pub(super) fn recycle(&mut self, buffer: BTreeMap<u16, u8>) {
        if self.buffers.len() < BATCH_LIMIT && buffer.len() <= 64 {
            self.buffers.push(buffer);
        }
    }

    pub(super) fn report(&self) -> Value {
        json!({"buffer_limit":BATCH_LIMIT,"max_payload_entries_per_buffer":64,
            "retained_buffers":self.buffers.len(),"reused_buffers":self.reused,
            "created_buffers":self.created,"scope":"capture-adapter-owned-payload-indices-only;released-with-sink"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_lengths_and_sparse_offsets_cannot_retain_old_evidence() {
        let mut pool = PayloadPool::default();
        for bytes in [
            &[7; 64][..],
            &[42; 64][..],
            &[9, 8, 7][..],
            &[],
            &[1; 64][..],
        ] {
            let buffer = pool.take(bytes);
            assert_eq!(
                buffer,
                bytes
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (i as u16, *b))
                    .collect()
            );
            pool.recycle(buffer);
        }
        pool.buffers.clear();
        pool.recycle(BTreeMap::from([(4, 99), (6, 77)]));
        assert_eq!(pool.take(&[1, 2]), BTreeMap::from([(0, 1), (1, 2)]));
    }

    #[test]
    fn pool_cannot_retain_more_than_one_bounded_batch() {
        let mut pool = PayloadPool::default();
        pool.recycle((0..65).map(|i| (i, 0)).collect());
        assert!(pool.buffers.is_empty());
        for _ in 0..BATCH_LIMIT + 3 {
            pool.recycle((0..64).map(|i| (i, 0)).collect());
        }
        assert_eq!(pool.buffers.len(), BATCH_LIMIT);
        let buffer = pool.take(&[1; 64]);
        assert_eq!(pool.reused, 1);
        assert_eq!(pool.created, 0);
        assert!(buffer.values().all(|b| *b == 1));
    }
}
