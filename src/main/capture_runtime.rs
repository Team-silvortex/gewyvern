use crate::Cli;
use gewyvern::machine_error::{ErrorCategory, MachineError};

#[cfg(not(target_os = "linux"))]
pub(crate) fn run(_: &Cli) -> Result<i32, MachineError> {
    Err(MachineError::new(
        "capture_platform_unsupported",
        ErrorCategory::Environment,
        "live kernel capture requires Linux",
        false,
        2,
    ))
}

#[cfg(target_os = "linux")]
pub(crate) fn run(cli: &Cli) -> Result<i32, MachineError> {
    run_with_api_factory(cli, || crate::serve_runtime::start_runtime_api(cli))
}

pub(crate) fn run_with_api_factory(
    cli: &Cli,
    start_api: impl FnOnce() -> Result<Option<crate::data_api::ApiService>, MachineError>,
) -> Result<i32, MachineError> {
    #[cfg(target_os = "linux")]
    {
        linux::run(cli, start_api)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = start_api;
        run(cli)
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use gewyvern::live_capture::{CaptureError, CaptureSnapshot, CaptureState, Collector};
    use gewyvern::runtime::{RuntimeSession, SessionConfig};
    use std::path::Path;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};

    struct Signals(Vec<signal_hook::SigId>);
    impl Drop for Signals {
        fn drop(&mut self) {
            for id in self.0.drain(..) {
                signal_hook::low_level::unregister(id);
            }
        }
    }

    fn error(error: CaptureError) -> MachineError {
        MachineError::new(
            error.code,
            ErrorCategory::Environment,
            error.to_string(),
            true,
            1,
        )
    }
    fn status(path: &Path, snapshot: &CaptureSnapshot) -> Result<(), MachineError> {
        let bytes = serde_json::to_vec(snapshot).map_err(|e| {
            error(CaptureError::new(
                "capture_status_encode_failed",
                e.to_string(),
            ))
        })?;
        silvortex_bounded_io::atomic_write_bounded_private_file(path, &bytes, 128 * 1024).map_err(
            |e| {
                error(CaptureError::new(
                    "capture_status_write_failed",
                    e.to_string(),
                ))
            },
        )
    }

    pub(super) fn run(
        cli: &Cli,
        start_api: impl FnOnce() -> Result<Option<crate::data_api::ApiService>, MachineError>,
    ) -> Result<i32, MachineError> {
        let options = cli
            .capture
            .options()
            .map_err(|e| error(CaptureError::new("capture_options_invalid", e)))?;
        let binding = cli.dsl_binding()?;
        let config = match binding {
            Some(binding) => SessionConfig::for_binding(binding),
            None => SessionConfig::for_template(gewyvern::template::udp_debug_template()),
        }?;
        let config = gewyvern::live_capture::session_config(config, &options).map_err(error)?;
        let status_path = cli.capture.status.clone().unwrap_or_else(|| {
            gewyvern::runtime_layout::runtime_layout()
                .state_root
                .join(format!("capture-{}.json", std::process::id()))
        });
        let path_key = |path: &Path| -> Result<std::path::PathBuf, MachineError> {
            let parent = path.parent().ok_or_else(|| {
                error(CaptureError::new(
                    "capture_output_invalid",
                    "output needs a parent directory",
                ))
            })?;
            let name = path.file_name().ok_or_else(|| {
                error(CaptureError::new(
                    "capture_output_invalid",
                    "output needs a file name",
                ))
            })?;
            Ok(std::fs::canonicalize(parent)
                .map_err(|e| {
                    error(CaptureError::new(
                        "capture_output_invalid",
                        format!("output parent must already exist: {e}"),
                    ))
                })?
                .join(name))
        };
        let output_path = cli.out_path.as_deref().ok_or_else(|| {
            error(CaptureError::new(
                "capture_output_invalid",
                "--out is required",
            ))
        })?;
        if path_key(Path::new(output_path))? == path_key(&status_path)? {
            return Err(error(CaptureError::new(
                "capture_output_conflict",
                "capture status and output must be different files",
            )));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let mut signals = Signals(Vec::new());
        for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
            signals.0.push(
                signal_hook::flag::register(signal, stop.clone()).map_err(|e| error(e.into()))?,
            );
        }
        let mut collector = Collector::start(options).map_err(error)?;
        let mut last = collector.snapshot().map_err(error)?;
        status(&status_path, &last)?;
        // A listener must not advertise readiness before real BPF attachment.
        let api = match start_api() {
            Ok(api) => api,
            Err(failure) => {
                if let Ok(snapshot) = collector.finish() {
                    last = snapshot;
                }
                last.state = CaptureState::Failed;
                last.incomplete = true;
                let _ = status(&status_path, &last);
                return Err(failure);
            }
        };
        let api_state = api.as_ref().map(crate::data_api::ApiService::state);
        let started = Instant::now();
        let mut window = Instant::now();
        let mut batches = 0usize;
        let publish = |facts: Vec<gewyvern::ledger::FactEnvelope>,
                       snapshot: &CaptureSnapshot|
         -> Result<(), MachineError> {
            let mut session = RuntimeSession::start(config.clone())?;
            // Kernel timestamps may interleave across CPUs; keep deterministic
            // timestamp/ID ordering inside this bounded analysis batch.
            let mut facts = facts;
            facts.sort_by_key(|fact| (fact.ts, fact.id));
            for fact in facts {
                session.ingest(fact);
            }
            let mut export = session.into_export_bundle();
            export.ingest_trust_mode = "unverified-local".into();
            export.debug_summary.degraded |= snapshot.incomplete;
            export.capture = Some(snapshot.clone());
            let name = crate::serve_runtime::single_runtime_target_name(&export);
            crate::serve_runtime::emit_rendered(cli, &name, &export, false, api_state)?;
            Ok(())
        };
        let result = (|| -> Result<(), MachineError> {
            if api_state.is_some() {
                publish(Vec::new(), &last)?;
            }
            loop {
                if stop.load(Ordering::Relaxed)
                    || cli
                        .capture
                        .seconds
                        .is_some_and(|seconds| started.elapsed() >= Duration::from_secs(seconds))
                {
                    break;
                }
                if !collector.scope_alive() {
                    return Err(error(CaptureError::new(
                        "capture_scope_removed",
                        "target cgroup removed or replaced",
                    )));
                }
                let consumed = collector.consume().map_err(error)?;
                if cli.serve && window.elapsed() >= Duration::from_millis(cli.capture.window_ms) {
                    if cli.max_sessions.is_some_and(|max| batches + 1 >= max) {
                        break;
                    }
                    last = collector.snapshot().map_err(error)?;
                    publish(collector.take_facts(), &last)?;
                    status(&status_path, &last)?;
                    batches += 1;
                    window = Instant::now();
                }
                if consumed == 0 {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            last = collector.finish().map_err(error)?;
            publish(collector.take_facts(), &last)?;
            status(&status_path, &last)?;
            Ok(())
        })();
        if let Err(failure) = result {
            if let Ok(snapshot) = collector.finish() {
                last = snapshot;
            }
            last.state = CaptureState::Failed;
            last.incomplete = true;
            if api_state.is_some() {
                // Refresh the API/persisted snapshot even when its output file failed.
                let _ = publish(collector.take_facts(), &last);
            }
            let _ = status(&status_path, &last);
            return Err(failure);
        }
        Ok(if last.incomplete { 3 } else { 0 })
    }
}
