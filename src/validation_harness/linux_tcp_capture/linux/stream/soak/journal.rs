use super::ValidationError;
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Clone, Copy, Default)]
struct Range {
    first: u64,
    last: u64,
    peak: u64,
}

pub(in super::super) struct Journal {
    memory: BufWriter<File>,
    heap: BufWriter<File>,
    ranges: [Range; 3],
    count: usize,
    heap_count: usize,
}

const KEYS: [&str; 3] = ["rss_kib", "anonymous_rss_kib", "cgroup_memory_bytes"];

fn writer(path: &Path) -> Result<BufWriter<File>, ValidationError> {
    Ok(BufWriter::with_capacity(
        8192,
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?,
    ))
}

fn append(writer: &mut BufWriter<File>, value: &Value) -> Result<(), ValidationError> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

impl Journal {
    pub(in super::super) fn new(out: &Path) -> Result<Self, ValidationError> {
        Ok(Self {
            memory: writer(&out.join("memory-samples.jsonl"))?,
            heap: writer(&out.join("heap-samples.jsonl"))?,
            ranges: [Range::default(); 3],
            count: 0,
            heap_count: 0,
        })
    }
    pub(in super::super) fn record(&mut self, sample: &Value) -> Result<(), ValidationError> {
        let values = KEYS.map(|key| sample[key].as_u64());
        if values.iter().any(Option::is_none) {
            return Err(ValidationError::new("missing journal memory counter"));
        }
        append(&mut self.memory, sample)?;
        for (range, value) in self.ranges.iter_mut().zip(values) {
            let value = value.ok_or_else(|| ValidationError::new("missing memory value"))?;
            if self.count == 0 {
                range.first = value;
            }
            range.last = value;
            range.peak = range.peak.max(value);
        }
        self.count += 1;
        Ok(())
    }
    pub(in super::super) fn record_heap(&mut self, sample: &Value) -> Result<(), ValidationError> {
        append(&mut self.heap, sample)?;
        self.heap_count += 1;
        Ok(())
    }
    pub(in super::super) fn report(&self) -> Value {
        let mut out = json!({"memory_sample_count":self.count,"heap_sample_count":self.heap_count,
            "memory_samples_file":"memory-samples.jsonl","heap_samples_file":"heap-samples.jsonl",
            "retained_memory_samples":0,"retained_heap_samples":0,"journal_buffer_bytes":16384});
        for (key, r) in KEYS.iter().zip(self.ranges) {
            out[*key] = json!({"first":r.first,"last":r.last,"peak":r.peak,"last_minus_first":i128::from(r.last)-i128::from(r.first)});
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn journal_uses_constant_retention_and_keeps_full_disk_samples() {
        use std::os::unix::fs::DirBuilderExt;
        let root = std::env::temp_dir().join(format!(
            "gewyvern-journal-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let mut journal = Journal::new(&root).unwrap();
        for n in 0..10_000 {
            journal
                .record(&json!({"rss_kib":n,"anonymous_rss_kib":n,"cgroup_memory_bytes":n}))
                .unwrap();
        }
        journal.record_heap(&json!({"sample":1})).unwrap();
        let report = journal.report();
        assert_eq!(report["rss_kib"]["last"], 9999);
        assert_eq!(report["memory_sample_count"], 10000);
        assert_eq!(report["retained_memory_samples"], 0);
        assert_eq!(report["retained_heap_samples"], 0);
        assert!(journal.record(&json!({"rss_kib":1})).is_err());
        drop(journal);
        assert_eq!(
            std::fs::read_to_string(root.join("memory-samples.jsonl"))
                .unwrap()
                .lines()
                .count(),
            10000
        );
        assert!(Journal::new(&root).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
