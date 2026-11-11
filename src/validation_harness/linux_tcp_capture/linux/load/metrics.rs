use std::fs;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use super::ValidationError;

const OVERFLOW_US: usize = 100_000;

pub(in super::super) struct Latency {
    bins: Vec<u64>,
    count: u64,
    sum_ns: u128,
    max_ns: u128,
}

impl Latency {
    pub(in super::super) fn new() -> Self {
        Self {
            bins: vec![0; OVERFLOW_US + 1],
            count: 0,
            sum_ns: 0,
            max_ns: 0,
        }
    }

    pub(in super::super) fn record(&mut self, elapsed: Duration) {
        let ns = elapsed.as_nanos();
        let bucket = ns.div_ceil(1_000).min(OVERFLOW_US as u128) as usize;
        self.bins[bucket] += 1;
        self.count += 1;
        self.sum_ns += ns;
        self.max_ns = self.max_ns.max(ns);
    }

    pub(in super::super) fn merge(&mut self, other: Self) {
        for (left, right) in self.bins.iter_mut().zip(other.bins) {
            *left += right;
        }
        self.count += other.count;
        self.sum_ns += other.sum_ns;
        self.max_ns = self.max_ns.max(other.max_ns);
    }

    fn percentile(&self, percent: u64) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let rank = (self.count * percent).div_ceil(100);
        let mut seen = 0;
        for (bucket, count) in self.bins.iter().enumerate() {
            seen += count;
            if seen >= rank {
                return (bucket < OVERFLOW_US).then_some(bucket);
            }
        }
        None
    }

    pub(in super::super) fn count(&self) -> u64 {
        self.count
    }

    pub(in super::super) fn report(&self) -> Value {
        json!({"requests":self.count,"mean_us":if self.count == 0 { 0.0 } else { self.sum_ns as f64 / self.count as f64 / 1000.0 },
            "p50_us_upper_bound":self.percentile(50),"p95_us_upper_bound":self.percentile(95),
            "p99_us_upper_bound":self.percentile(99),"max_us":self.max_ns as f64 / 1000.0,
            "histogram_resolution_us":1,"histogram_overflow_threshold_us":OVERFLOW_US,
            "histogram_overflow_count":self.bins[OVERFLOW_US],"null_percentile_means":"empty-or-overflow-not-zero"})
    }
}

fn field(input: &str, name: &str) -> Result<u64, ValidationError> {
    input
        .lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            (parts.next() == Some(name))
                .then(|| parts.next()?.parse::<u64>().ok())
                .flatten()
        })
        .ok_or_else(|| ValidationError::new(format!("missing resource counter {name}")))
}

pub(in super::super) struct Cpu {
    usage: u64,
    user: u64,
    system: u64,
    throttled: u64,
    throttled_us: u64,
}

impl Cpu {
    pub(in super::super) fn read(scope: &Path) -> Result<Self, ValidationError> {
        let input = fs::read_to_string(scope.join("cpu.stat"))?;
        Ok(Self {
            usage: field(&input, "usage_usec")?,
            user: field(&input, "user_usec")?,
            system: field(&input, "system_usec")?,
            throttled: field(&input, "nr_throttled")?,
            throttled_us: field(&input, "throttled_usec")?,
        })
    }

    pub(in super::super) fn delta(
        &self,
        end: &Self,
        elapsed: f64,
    ) -> Result<Value, ValidationError> {
        let diff = |a: u64, b: u64| {
            b.checked_sub(a)
                .ok_or_else(|| ValidationError::new("CPU counter went backwards"))
        };
        if !elapsed.is_finite() || elapsed <= 0.0 {
            return Err(ValidationError::new("invalid CPU sample interval"));
        }
        let usage = diff(self.usage, end.usage)?;
        Ok(
            json!({"usage_seconds":usage as f64 / 1e6,"percent_one_core":usage as f64 / (elapsed * 1e4),
            "percent_two_core_quota":usage as f64 / (elapsed * 2e4),
            "user_seconds":diff(self.user,end.user)? as f64 / 1e6,"system_seconds":diff(self.system,end.system)? as f64 / 1e6,
            "throttled_periods":diff(self.throttled,end.throttled)?,"throttled_seconds":diff(self.throttled_us,end.throttled_us)? as f64 / 1e6}),
        )
    }
}

pub(in super::super) fn memory(scope: &Path, seconds: f64) -> Result<Value, ValidationError> {
    let status = fs::read_to_string("/proc/self/status")?;
    let current = fs::read_to_string(scope.join("memory.current"))?
        .trim()
        .parse::<u64>()
        .map_err(|e| ValidationError::new(e.to_string()))?;
    Ok(
        json!({"elapsed_seconds":seconds,"rss_kib":field(&status,"VmRSS:")?,
        "anonymous_rss_kib":field(&status,"RssAnon:")?,"cgroup_memory_bytes":current}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latency_merge_and_nearest_rank_keep_overflow_visible() {
        let mut a = Latency::new();
        assert_eq!(a.percentile(99), None);
        for us in 1..=99 {
            a.record(Duration::from_micros(us));
        }
        let mut b = Latency::new();
        b.record(Duration::from_millis(200));
        a.merge(b);
        assert_eq!(a.count(), 100);
        assert_eq!(a.percentile(50), Some(50));
        assert_eq!(a.percentile(95), Some(95));
        assert_eq!(a.percentile(99), Some(99));
        assert_eq!(a.percentile(100), None);
        assert_eq!(a.report()["histogram_overflow_count"], 1);
        assert_eq!(a.report()["max_us"], 200_000.0);
    }
    #[test]
    fn cpu_delta_rejects_counter_reset_and_bad_time() {
        let first = Cpu {
            usage: 10,
            user: 5,
            system: 5,
            throttled: 0,
            throttled_us: 0,
        };
        let second = Cpu {
            usage: 2_000_010,
            user: 1_000_005,
            system: 1_000_005,
            throttled: 1,
            throttled_us: 20,
        };
        assert_eq!(
            first.delta(&second, 1.0).unwrap()["percent_one_core"],
            200.0
        );
        assert!(second.delta(&first, 1.0).is_err());
        assert!(first.delta(&second, 0.0).is_err());
        assert!(field("usage_usec broken", "usage_usec").is_err());
    }
}
