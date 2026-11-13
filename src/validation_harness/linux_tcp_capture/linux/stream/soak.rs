//! Bounded same-process retention diagnosis, never automatic production trimming.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use super::{ValidationError, ValidationReport, prepare_output};

const PHASE_SECONDS: u64 = 300;
const IDLE_SECONDS: u64 = 30;
const CYCLES: usize = 3;
mod journal;
pub(super) use journal::Journal;

fn status_counter(status: &str, name: &str) -> Result<u64, ValidationError> {
    status
        .lines()
        .find_map(|line| {
            line.strip_prefix(name)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
        .ok_or_else(|| ValidationError::new(format!("missing process counter {name}")))
}

pub(super) fn snapshot(out: &Path, name: &str) -> Result<Value, ValidationError> {
    let rust = crate::validation_harness::allocation_audit::snapshot();
    let status = fs::read_to_string("/proc/self/status")?;
    let descriptors = fs::read_dir("/proc/self/fd")?
        .collect::<Result<Vec<_>, _>>()?
        .len();
    let allocator = allocator_snapshot(&out.join(format!("{name}.xml")))?;
    Ok(json!({"rss_kib":status_counter(&status,"VmRSS:")?,
        "anonymous_rss_kib":status_counter(&status,"RssAnon:")?,
        "virtual_kib":status_counter(&status,"VmSize:")?,
        "threads":status_counter(&status,"Threads:")?,"file_descriptors":descriptors,
        "allocator":allocator,"rust_allocations":rust}))
}

#[cfg(target_env = "gnu")]
fn allocator_snapshot(path: &Path) -> Result<Value, ValidationError> {
    use std::io::Read;
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::os::unix::fs::OpenOptionsExt;
    let fd = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?
        .into_raw_fd();
    // SAFETY: fd is a fresh owned writable file. fdopen takes ownership on success.
    let stream = unsafe { libc::fdopen(fd, c"w".as_ptr()) };
    if stream.is_null() {
        let error = std::io::Error::last_os_error();
        // SAFETY: failed fdopen did not take ownership.
        drop(unsafe { fs::File::from_raw_fd(fd) });
        return Err(error.into());
    }
    // SAFETY: malloc_info uses the live FILE synchronously; fclose releases it once.
    let (result, closed) = unsafe {
        let result = libc::malloc_info(0, stream);
        (result, libc::fclose(stream))
    };
    if result != 0 || closed != 0 {
        return Err(ValidationError::new("allocator snapshot write failed"));
    }
    let mut xml = String::new();
    fs::File::open(path)?
        .take(1_048_577)
        .read_to_string(&mut xml)?;
    if xml.len() > 1_048_576 {
        return Err(ValidationError::new("allocator snapshot exceeds 1 MiB"));
    }
    parse_allocator(&xml)
}

#[cfg(not(target_env = "gnu"))]
fn allocator_snapshot(_path: &Path) -> Result<Value, ValidationError> {
    Err(ValidationError::new(
        "capture-soak allocator diagnosis requires glibc",
    ))
}

#[cfg(any(target_env = "gnu", test))]
fn parse_allocator(xml: &str) -> Result<Value, ValidationError> {
    if !xml.starts_with("<malloc version=\"1\">") || !xml.trim_end().ends_with("</malloc>") {
        return Err(ValidationError::new(
            "unsupported allocator snapshot format",
        ));
    }
    let totals = xml
        .rsplit_once("</heap>")
        .ok_or_else(|| ValidationError::new("missing allocator heaps"))?
        .1;
    let size = |prefix: &str| -> Result<u64, ValidationError> {
        let mut matches = totals
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with(prefix));
        let line = matches
            .next()
            .ok_or_else(|| ValidationError::new("missing allocator total"))?;
        if matches.next().is_some() {
            return Err(ValidationError::new("duplicate allocator total"));
        }
        line.split_once(" size=\"")
            .and_then(|(_, tail)| tail.split_once('"'))
            .and_then(|(value, _)| value.parse().ok())
            .ok_or_else(|| ValidationError::new("invalid allocator size"))
    };
    let system = size("<system type=\"current\"")?;
    let mapped = size("<total type=\"mmap\"")?;
    // glibc 2.43 removed fastbins entirely; older schemas report them even at zero.
    // A per-arena fastbin without its global total is still inconsistent evidence.
    let fastbin_total_present = totals.contains("<total type=\"fast\"");
    let fast = if xml.contains("<total type=\"fast\"") {
        size("<total type=\"fast\"")?
    } else {
        0
    };
    let free = fast
        .checked_add(size("<total type=\"rest\"")?)
        .ok_or_else(|| ValidationError::new("allocator free total overflow"))?;
    let nonfree = system
        .checked_sub(free)
        .and_then(|value| value.checked_add(mapped))
        .ok_or_else(|| ValidationError::new("allocator totals do not reconcile"))?;
    Ok(
        json!({"arenas":xml.matches("<heap nr=\"").count(),"fastbin_total_present":fastbin_total_present,"arena_system_bytes":system,
        "mmap_bytes":mapped,"arena_free_bytes":free,"nonfree_upper_bound_bytes":nonfree,
        "scope":"all-glibc-arenas; nonfree includes metadata, tcache and instrumentation, not exact live Rust bytes"}),
    )
}

pub(in super::super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_with_duration(out, PHASE_SECONDS)
}

pub(in super::super) fn run_hour(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    if crate::validation_harness::allocation_audit::snapshot().is_none() {
        return Err(ValidationError::new(
            "capture-soak-hour requires the validator built with --features capture-allocation-audit",
        ));
    }
    run_with_duration(out, 1200)
}

fn write_snapshot(out: &Path, name: &str) -> Result<(), ValidationError> {
    let value = snapshot(out, name)?;
    fs::write(
        out.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&value)?,
    )?;
    Ok(())
}

fn read_report(path: PathBuf) -> Result<Value, ValidationError> {
    // These are small, owned reports; never load the growing sample journals.
    crate::validation_harness::read_bounded_json_file(&path, "soak checkpoint", 1_048_576)
}

pub(in super::super) fn run_control(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    if crate::validation_harness::allocation_audit::snapshot().is_none() {
        return Err(ValidationError::new(
            "capture-memory-control requires --features capture-allocation-audit",
        ));
    }
    let out = out.unwrap_or_else(|| {
        crate::validation_harness::command::default_out_dir("capture-memory-control")
    });
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&out)?;
    write_snapshot(&out, "before-control")?;
    for index in 0..256 {
        write_snapshot(&out, &format!("main-control-{index:03}"))?;
    }
    write_snapshot(&out, "after-main-control")?;
    let thread_out = out.clone();
    std::thread::spawn(move || -> Result<(), ValidationError> {
        for index in 0..256 {
            write_snapshot(&thread_out, &format!("thread-control-{index:03}"))?;
        }
        Ok(())
    })
    .join()
    .map_err(|_| ValidationError::new("observer control thread panicked"))??;
    write_snapshot(&out, "after-thread-exit")?;
    #[cfg(target_env = "gnu")]
    // SAFETY: the observer thread has joined; diagnostic only, never a service policy.
    let trimmed = unsafe { libc::malloc_trim(0) } != 0;
    #[cfg(not(target_env = "gnu"))]
    let trimmed = false;
    write_snapshot(&out, "control-after-trim")?;
    let samples = |prefix: &str| {
        (0..256)
            .map(|index| read_report(out.join(format!("{prefix}-{index:03}.json"))))
            .collect::<Result<Vec<_>, _>>()
    };
    let summary = json!({"schema_version":1,"mode":"capture-memory-control","ok":true,"traffic_generated":false,
        "bpf_attached":false,"iterations_per_thread":256,
        "before":read_report(out.join("before-control.json"))?,
        "main":samples("main-control")?,"worker":samples("thread-control")?,
        "after_main":read_report(out.join("after-main-control.json"))?,
        "after_thread_exit":read_report(out.join("after-thread-exit.json"))?,
        "after_trim":read_report(out.join("control-after-trim.json"))?,"trim_returned_memory":trimmed,
        "interpretation":"No traffic or runtime sink; isolates observer allocations and libc retention. Rust counts during the worker include the waiting parent. Final result trees are assembled only after observations."});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: "Linux no-traffic allocation observer control".into(),
        out_dir: out,
        checks: vec![
            "no_network_or_bpf_effects".into(),
            "report_free_observer_iterations".into(),
            "worker_thread_joined_before_final_snapshot".into(),
        ],
    })
}

fn run_with_duration(
    out: Option<PathBuf>,
    seconds: u64,
) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "capture-soak")?;
    for index in 0..8 {
        write_snapshot(&out, &format!("observer-control-{index}"))?;
    }
    let before = snapshot(&out, "before")?;
    for index in 0..CYCLES {
        {
            eprintln!(
                "capture soak: cycle {}/{CYCLES}, {seconds}s load",
                index + 1
            );
            let result = super::phase_with_duration(
                &out.join(format!("cycle-{}", index + 1)),
                false,
                true,
                seconds,
                true,
            )?;
            let compact = json!({"elapsed_seconds":result["elapsed_seconds"],"measured_events":result["measured_events"],
                "measured_delivered_events":result["measured_delivered_events"],"measured_output_failures":result["measured_output_failures"],
                "observation_complete":result["observation_complete"],"delivered_events_per_second":result["delivered_events_per_second"],
                "rss_kib":result["rss_kib"],"cpu":result["cpu"],"observation_journal":result["observation_journal"],
                "analysis_latency":result["delivery"]["runtime"]["analysis_completion_latency"]});
            drop(result);
            let released = snapshot(&out, &format!("released-{index}"))?;
            eprintln!(
                "capture soak: cycle {} released, {IDLE_SECONDS}s idle",
                index + 1
            );
            std::thread::sleep(Duration::from_secs(IDLE_SECONDS));
            let idle = snapshot(&out, &format!("idle-{index}"))?;
            if idle["threads"] != before["threads"]
                || idle["file_descriptors"] != before["file_descriptors"]
            {
                return Err(ValidationError::new(
                    "soak retained threads or descriptors after teardown",
                ));
            }
            let report = json!({"load":compact,"released":released,"idle":idle});
            fs::write(
                out.join(format!("cycle-report-{index}.json")),
                serde_json::to_vec_pretty(&report)?,
            )?;
        }
        // No cycle reports or samples survive this boundary. Reconstruct the final
        // summary from disk only after the last memory observation.
        write_snapshot(&out, &format!("clean-{index}"))?;
        eprintln!("capture soak: clean checkpoint {} persisted", index + 1);
    }
    write_snapshot(&out, "before-trim")?;
    #[cfg(target_env = "gnu")]
    // SAFETY: capture threads are joined; this opt-in diagnostic releases free pages only.
    let trim_returned_memory = unsafe { libc::malloc_trim(0) } != 0;
    #[cfg(not(target_env = "gnu"))]
    let trim_returned_memory = false;
    write_snapshot(&out, "after-trim")?;
    super::super::http_boundaries::run(Some(out.join("after-soak-restart")))?;
    let cycles = (0..CYCLES)
        .map(|index| {
            let mut report = read_report(out.join(format!("cycle-report-{index}.json")))?;
            report["clean"] = read_report(out.join(format!("clean-{index}.json")))?;
            Ok(report)
        })
        .collect::<Result<Vec<Value>, ValidationError>>()?;
    let controls = (0..8)
        .map(|index| read_report(out.join(format!("observer-control-{index}.json"))))
        .collect::<Result<Vec<_>, _>>()?;
    let checks = vec![
        "three_same_process_load_cycles",
        "load_loss_accounting",
        "post_teardown_idle_observations",
        "all_arena_allocator_snapshots",
        "no_retained_threads_or_descriptors",
        "owned_bpf_and_cgroup_cleanup",
        "diagnostic_only_final_trim",
        "fresh_capture_replay_after_soak",
        "constant_space_sample_journals",
        "no_retained_cycle_reports_at_clean_checkpoints",
        "observer_only_control_snapshots",
    ];
    let summary = json!({"schema_version":2,"ok":true,"mode":if seconds == 1200 {"capture-soak-hour"} else {"capture-soak"},
        "cycles":cycles,"before":before,"observer_controls":controls,
        "phase_seconds":seconds,"idle_seconds":IDLE_SECONDS,"total_load_seconds":seconds*CYCLES as u64,
        "before_trim":read_report(out.join("before-trim.json"))?,"after_trim":read_report(out.join("after-trim.json"))?,
        "trim_returned_memory":trim_returned_memory,"forced_trim_during_load":false,"checks":checks,
        "normal_windows_complete":cycles.iter().all(|cycle|cycle["load"]["observation_complete"]==true),
        "interpretation":"Clean checkpoints exclude cycle report/sample trees. Rust counts are requested live bytes across threads, not C allocations/RSS; live-load pairs are not atomic snapshots. Compare quiescent checkpoints after warmup.",
        "not_proven":["unbounded-duration leak freedom","exact C allocation attribution","production continuous cross-batch sessions","multiple hosts or kernels"]});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: "Linux same-process capture runtime memory soak".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const XML: &str = "<malloc version=\"1\">\n<heap nr=\"0\">\n<system type=\"current\" size=\"999\"/>\n</heap>\n<total type=\"fast\" count=\"1\" size=\"100\"/>\n<total type=\"rest\" count=\"1\" size=\"300\"/>\n<total type=\"mmap\" count=\"1\" size=\"200\"/>\n<system type=\"current\" size=\"1000\"/>\n</malloc>\n";
    #[test]
    fn long_diagnostics_reject_an_inactive_allocator_before_creating_output() {
        let path =
            std::env::temp_dir().join(format!("gewyvern-inactive-audit-{}", std::process::id()));
        assert!(crate::validation_harness::allocation_audit::snapshot().is_none());
        for result in [
            run_hour(Some(path.clone())),
            run_control(Some(path.clone())),
        ] {
            let Err(error) = result else {
                panic!("memory diagnostics must reject an inactive audit");
            };
            assert!(error.to_string().contains("capture-allocation-audit"));
        }
        assert!(!path.exists());
    }
    #[test]
    fn allocator_parser_uses_global_totals_and_rejects_ambiguous_evidence() {
        let value = parse_allocator(XML).unwrap();
        assert_eq!(value["arena_free_bytes"], 400);
        assert_eq!(value["nonfree_upper_bound_bytes"], 800);
        assert_eq!(value["arenas"], 1);
        let no_fastbins = XML.replace("<total type=\"fast\" count=\"1\" size=\"100\"/>\n", "");
        let modern = parse_allocator(&no_fastbins).unwrap();
        assert_eq!(modern["arena_free_bytes"], 300);
        assert_eq!(modern["nonfree_upper_bound_bytes"], 900);
        assert_eq!(modern["fastbin_total_present"], false);
        assert!(
            parse_allocator(&no_fastbins.replace(
                "</heap>",
                "<total type=\"fast\" count=\"0\" size=\"0\"/>\n</heap>"
            ))
            .is_err()
        );
        for invalid in [
            XML.replace("version=\"1\"", "version=\"2\""),
            XML.replace("size=\"300\"", "size=\"2000\""),
            XML.replace(
                "</malloc>",
                "<system type=\"current\" size=\"5\"/>\n</malloc>",
            ),
            XML.replace("</malloc>", ""),
        ] {
            assert!(parse_allocator(&invalid).is_err());
        }
    }

    #[cfg(target_env = "gnu")]
    #[test]
    fn allocator_snapshot_is_private_exclusive_and_does_not_follow_symlinks() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
        use std::time::{SystemTime, UNIX_EPOCH};
        let root = std::env::temp_dir().join(format!(
            "gewyvern-soak-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let path = root.join("heap.xml");
        let snapshot = allocator_snapshot(&path).unwrap();
        assert!(snapshot["arenas"].as_u64().unwrap() > 0);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(allocator_snapshot(&path).is_err());
        let link = root.join("link.xml");
        symlink(&path, &link).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(allocator_snapshot(&link).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }
}
