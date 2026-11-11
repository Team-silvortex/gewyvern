use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::ValidationError;

#[cfg(test)]
pub(super) mod allocations;

pub(super) struct Stamp {
    wall: Instant,
    cpu: Option<u64>,
    #[cfg(test)]
    allocations: allocations::Counts,
}

pub(super) fn thread_cpu_ns() -> Result<u64, ValidationError> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: the synchronous call writes a valid initialized timespec.
    if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let seconds = u64::try_from(time.tv_sec).ok();
    let nanos = u64::try_from(time.tv_nsec)
        .ok()
        .filter(|n| *n < 1_000_000_000);
    seconds
        .zip(nanos)
        .and_then(|(s, n)| s.checked_mul(1_000_000_000)?.checked_add(n))
        .ok_or_else(|| ValidationError::new("invalid thread CPU clock"))
}

#[derive(Default)]
pub(super) struct Timing {
    calls: u64,
    total_ns: u128,
    max_ns: u128,
    cpu_samples: u64,
    cpu_ns: u128,
    cpu_max_ns: u64,
    sampled_wall_ns: u128,
    residual_ns: u128,
    clock_overlaps: u64,
    #[cfg(test)]
    allocations: allocations::Counts,
}

impl Timing {
    pub(super) fn start(&self, sample_every: u64) -> Result<Stamp, ValidationError> {
        let wall = Instant::now();
        let cpu = self
            .calls
            .is_multiple_of(sample_every)
            .then(thread_cpu_ns)
            .transpose()?;
        Ok(Stamp {
            wall,
            cpu,
            #[cfg(test)]
            allocations: allocations::snapshot(),
        })
    }

    pub(super) fn record(&mut self, start: Stamp) -> Result<(), ValidationError> {
        #[cfg(test)]
        let allocations = allocations::snapshot().since(start.allocations);
        let cpu = start
            .cpu
            .map(|begin| {
                thread_cpu_ns()?
                    .checked_sub(begin)
                    .ok_or_else(|| ValidationError::new("thread CPU clock went backwards"))
            })
            .transpose()?;
        let elapsed = start.wall.elapsed();
        self.record_sample(elapsed, cpu);
        #[cfg(test)]
        self.allocations.add(allocations);
        Ok(())
    }

    fn record_sample(&mut self, elapsed: Duration, cpu: Option<u64>) {
        self.calls += 1;
        self.total_ns += elapsed.as_nanos();
        self.max_ns = self.max_ns.max(elapsed.as_nanos());
        if let Some(cpu) = cpu {
            self.cpu_samples += 1;
            self.cpu_ns += u128::from(cpu);
            self.cpu_max_ns = self.cpu_max_ns.max(cpu);
            self.sampled_wall_ns += elapsed.as_nanos();
            self.residual_ns += elapsed.as_nanos().saturating_sub(u128::from(cpu));
            self.clock_overlaps += u64::from(u128::from(cpu) > elapsed.as_nanos());
        }
    }

    pub(super) fn report(&self) -> Value {
        let report = json!({"calls":self.calls,"total_ms":self.total_ns as f64 / 1_000_000.0,
            "mean_us":if self.calls == 0 { 0.0 } else { self.total_ns as f64 / self.calls as f64 / 1_000.0 },
            "max_us":self.max_ns as f64 / 1_000.0,
            "cpu_samples":self.cpu_samples,"sampled_cpu_ms":self.cpu_ns as f64 / 1e6,
            "sampled_cpu_mean_us":if self.cpu_samples == 0 { None } else { Some(self.cpu_ns as f64 / self.cpu_samples as f64 / 1e3) },
            "sampled_cpu_max_us":self.cpu_max_ns as f64 / 1e3,
            "sampled_wall_ms":self.sampled_wall_ns as f64 / 1e6,
            "sampled_non_cpu_residual_ms":self.residual_ns as f64 / 1e6,
            "cpu_exceeded_enclosing_wall_samples":self.clock_overlaps});
        #[cfg(test)]
        let report = {
            let mut report = report;
            report["test_allocations"] = self.allocations.report();
            report
        };
        report
    }
}

#[derive(Default)]
pub(super) struct Profile {
    pub catalog_discovery: Timing,
    pub conversion: Timing,
    pub ingest: Timing,
    pub rotation: Timing,
    pub analysis: Timing,
    pub validation: Timing,
    pub replay: Timing,
    pub release: Timing,
}

impl Profile {
    pub(super) fn report(&self) -> Value {
        json!({"scope":"consumer-thread-wall-and-sampled-CPU-including-setup-and-warmup",
            "cpu_clock":"CLOCK_THREAD_CPUTIME_ID",
            "cpu_sampling":"every-257th-conversion-and-ingest;every-batch-stage",
            "timing_caveat":"includes-clock-overhead;non-CPU-residual-is-not-quota-only-wait;no-extrapolation-of-sampled-CPU",
            "catalog_discovery":self.catalog_discovery.report(),
            "conversion":self.conversion.report(),"ingest":self.ingest.report(),
            "rotation":self.rotation.report(),"analysis":self.analysis.report(),
            "validation":self.validation.report(),"replay":self.replay.report(),"release":self.release.report()})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_samples_pair_with_their_own_wall_intervals() {
        let mut timing = Timing::default();
        assert!(timing.report()["sampled_cpu_mean_us"].is_null());
        timing.record_sample(Duration::from_micros(10), Some(3_000));
        timing.record_sample(Duration::from_micros(100), None);
        timing.record_sample(Duration::from_micros(2), Some(2_001));
        let report = timing.report();
        assert_eq!(report["calls"], 3);
        assert_eq!(report["cpu_samples"], 2);
        assert_eq!(report["sampled_wall_ms"], 0.012);
        assert_eq!(report["sampled_non_cpu_residual_ms"], 0.007);
        assert_eq!(report["cpu_exceeded_enclosing_wall_samples"], 1);
    }

    #[test]
    fn cpu_sampling_does_not_lock_to_the_same_batch_slot() {
        let mut timing = Timing::default();
        for _ in 0..775 {
            let started = timing.start(257).unwrap();
            timing.record(started).unwrap();
        }
        assert_eq!(timing.cpu_samples, 4);
        assert!(thread_cpu_ns().unwrap() > 0);
    }
}
