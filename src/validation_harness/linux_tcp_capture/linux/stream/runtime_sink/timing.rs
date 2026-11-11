use std::time::Duration;

use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct Timing {
    calls: u64,
    total_ns: u128,
    max_ns: u128,
}

impl Timing {
    pub(super) fn record(&mut self, elapsed: Duration) {
        self.calls += 1;
        self.total_ns += elapsed.as_nanos();
        self.max_ns = self.max_ns.max(elapsed.as_nanos());
    }

    fn report(&self) -> Value {
        json!({"calls":self.calls,"total_ms":self.total_ns as f64 / 1_000_000.0,
            "mean_us":if self.calls == 0 { 0.0 } else { self.total_ns as f64 / self.calls as f64 / 1_000.0 },
            "max_us":self.max_ns as f64 / 1_000.0})
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
        json!({"scope":"consumer-thread-wall-time-including-setup-and-warmup-not-CPU-time",
            "catalog_discovery":self.catalog_discovery.report(),
            "conversion":self.conversion.report(),"ingest":self.ingest.report(),
            "rotation":self.rotation.report(),"analysis":self.analysis.report(),
            "validation":self.validation.report(),"replay":self.replay.report(),"release":self.release.report()})
    }
}
