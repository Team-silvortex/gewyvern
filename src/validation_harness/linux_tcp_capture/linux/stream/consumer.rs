use std::cell::RefCell;
use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::load::metrics::Latency;
use super::ValidationError;
use super::ring::{Record, Ring};
use super::runtime_sink::RuntimeSink;

pub(super) fn monotonic_ns() -> Result<u64, ValidationError> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid initialized timespec is writable for this synchronous clock call.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let seconds = u64::try_from(time.tv_sec)
        .map_err(|_| ValidationError::new("invalid monotonic seconds"))?;
    let nanos =
        u64::try_from(time.tv_nsec).map_err(|_| ValidationError::new("invalid monotonic nanos"))?;
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|s| s.checked_add(nanos))
        .filter(|_| nanos < 1_000_000_000)
        .ok_or_else(|| ValidationError::new("monotonic clock overflow"))
}

struct Control {
    stop: AtomicBool,
    pause: AtomicBool,
    paused: AtomicBool,
    measured_since: AtomicU64,
    received: AtomicU64,
}

pub(super) struct Stats {
    pub received: u64,
    pub sequence_gaps: u64,
    last: [Option<u64>; 2],
    latency: Latency,
    first: Vec<Record>,
    runtime: Option<RuntimeSink>,
}

impl Stats {
    fn new(runtime: bool) -> Result<Self, ValidationError> {
        Ok(Self {
            received: 0,
            sequence_gaps: 0,
            last: [None; 2],
            latency: Latency::new(),
            first: Vec::with_capacity(8),
            runtime: runtime.then(RuntimeSink::new).transpose()?,
        })
    }

    fn accept(
        &mut self,
        record: &Record,
        expected: &[(u64, u16, u16, u32); 2],
        control: &Control,
    ) -> Result<(), ValidationError> {
        let index = expected
            .iter()
            .position(|e| e.0 == record.cookie)
            .ok_or_else(|| ValidationError::new("stream captured a foreign cookie"))?;
        if record.sport != expected[index].2
            || record.dport != expected[index].1
            || record.source != 0x7f000001
            || record.destination != 0x7f000001
            || record.payload_len != 64
            || record.prefix_len != 64
            || record.prefix[8..].iter().any(|b| *b != index as u8)
        {
            return Err(ValidationError::new(
                "stream echo identity/payload mismatch",
            ));
        }
        let sequence = u64::from_le_bytes(
            record.prefix[..8]
                .try_into()
                .map_err(|_| ValidationError::new("bad echo sequence"))?,
        );
        let next = self.last[index]
            .map_or(Some(0), |v| v.checked_add(1))
            .ok_or_else(|| ValidationError::new("echo sequence overflow"))?;
        if sequence < next {
            return Err(ValidationError::new("duplicate or reordered echo response"));
        }
        self.sequence_gaps = self
            .sequence_gaps
            .checked_add(sequence - next)
            .ok_or_else(|| ValidationError::new("echo gap counter overflow"))?;
        self.last[index] = Some(sequence);
        let measured = record.monotonic_ns >= control.measured_since.load(Ordering::Acquire);
        if let Some(runtime) = &mut self.runtime {
            runtime.accept(record, expected[index].3, measured)?;
        }
        let age = monotonic_ns()?
            .checked_sub(record.monotonic_ns)
            .ok_or_else(|| ValidationError::new("stream event is in the future"))?;
        self.received += 1;
        if measured {
            self.latency.record(Duration::from_nanos(age));
        }
        if self.first.len() < 8 {
            self.first.push(*record);
        }
        control.received.store(self.received, Ordering::Release);
        Ok(())
    }

    fn flush_runtime(&mut self) -> Result<(), ValidationError> {
        if let Some(runtime) = &mut self.runtime {
            runtime.flush(true)?;
        }
        Ok(())
    }

    pub(super) fn report(&self) -> Result<Value, ValidationError> {
        let first: Vec<Value> = self
            .first
            .iter()
            .map(|r| {
                json!({"event_sequence":r.sequence,"monotonic_ns":r.monotonic_ns,
            "cookie":r.cookie,"tcp_seq":r.seq,"prefix":&r.prefix[..]})
            })
            .collect();
        let runtime = self.runtime.as_ref().map(RuntimeSink::report).transpose()?;
        if let Some(report) = &runtime
            && (report["analyzed_facts"] != self.received
                || report["measured_analyzed_facts"] != self.latency.count())
        {
            return Err(ValidationError::new("runtime and consumer counts diverged"));
        }
        Ok(
            json!({"received_events":self.received,"measured_received_events":self.latency.count(),
            "observed_echo_sequence_gaps":self.sequence_gaps,"last_echo_sequences":self.last,
            "delivery_latency":self.latency.report(),"first_records":first,
            "runtime":runtime,
            "sink":if self.runtime.is_some() { "bounded-independent-runtime-batches" } else { "bounded-echo-integrity-and-latency-observer-not-runtime-ledger" }}),
        )
    }
}

pub(super) struct Consumer {
    control: Arc<Control>,
    thread: Option<JoinHandle<Result<Stats, ValidationError>>>,
}

impl Consumer {
    pub(super) fn start(
        map: OwnedFd,
        expected: [(u64, u16, u16, u32); 2],
        runtime: bool,
    ) -> Result<Self, ValidationError> {
        let control = Arc::new(Control {
            stop: AtomicBool::new(false),
            pause: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            measured_since: AtomicU64::new(u64::MAX),
            received: AtomicU64::new(0),
        });
        let state = Arc::clone(&control);
        let (tx, rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("capture-ring-consumer".into())
            .spawn(move || {
                let stats = match Stats::new(runtime) {
                    Ok(stats) => RefCell::new(stats),
                    Err(error) => {
                        let _ = tx.send(Err(error.to_string()));
                        return Err(error);
                    }
                };
                let mut ring = match Ring::new(map, |record: &Record| {
                    stats.borrow_mut().accept(record, &expected, &state)
                }) {
                    Ok(ring) => ring,
                    Err(error) => {
                        let _ = tx.send(Err(error.to_string()));
                        return Err(error);
                    }
                };
                tx.send(Ok(()))
                    .map_err(|e| ValidationError::new(e.to_string()))?;
                while !state.stop.load(Ordering::Acquire) {
                    let count = ring.consume()?;
                    if count == 0 && state.pause.load(Ordering::Acquire) {
                        stats.borrow_mut().flush_runtime()?;
                        state.paused.store(true, Ordering::Release);
                        while state.pause.load(Ordering::Acquire)
                            && !state.stop.load(Ordering::Acquire)
                        {
                            thread::sleep(Duration::from_millis(1));
                        }
                        state.paused.store(false, Ordering::Release);
                    } else if count == 0 {
                        ring.wait()?;
                    }
                }
                // The owner detaches after its producers stop, so final draining is finite.
                let deadline = Instant::now() + Duration::from_secs(3);
                while ring.consume()? > 0 {
                    if Instant::now() > deadline {
                        return Err(ValidationError::new("stream final drain timed out"));
                    }
                }
                drop(ring);
                let mut stats = stats.into_inner();
                stats.flush_runtime()?;
                Ok(stats)
            })?;
        let this = Self {
            control,
            thread: Some(thread),
        };
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|e| ValidationError::new(e.to_string()))?
            .map_err(ValidationError::new)?;
        Ok(this)
    }

    pub(super) fn pause_after_drain(&self) -> Result<u64, ValidationError> {
        self.control.pause.store(true, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(3);
        while !self.control.paused.load(Ordering::Acquire) {
            if self.failed() || Instant::now() > deadline {
                return Err(ValidationError::new(
                    "consumer did not reach an empty paused checkpoint",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(self.control.received.load(Ordering::Acquire))
    }

    pub(super) fn begin_measurement(&self) -> Result<(), ValidationError> {
        self.control
            .measured_since
            .store(monotonic_ns()?, Ordering::Release);
        Ok(())
    }
    pub(super) fn resume(&self) {
        self.control.pause.store(false, Ordering::Release);
    }
    pub(super) fn failed(&self) -> bool {
        self.thread.as_ref().is_some_and(JoinHandle::is_finished)
    }

    pub(super) fn finish(mut self) -> Result<Stats, ValidationError> {
        self.control.stop.store(true, Ordering::Release);
        self.resume();
        self.thread
            .take()
            .ok_or_else(|| ValidationError::new("consumer already stopped"))?
            .join()
            .map_err(|_| ValidationError::new("consumer thread panicked"))?
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        self.control.stop.store(true, Ordering::Release);
        self.control.pause.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
