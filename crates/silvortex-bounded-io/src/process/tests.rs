use super::*;

const MODE: &str = "SILVORTEX_PROCESS_PROBE";
fn command(mode: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "process::tests::child_probe", "--nocapture"])
        .env(MODE, mode)
        .stdin(Stdio::null());
    command
}
fn capture(mode: &str, timeout: Duration) -> io::Result<Output> {
    run_command_output(
        &mut command(mode),
        None,
        timeout,
        OutputLimits::new(4096, 4096),
        "process probe",
    )
}

#[test]
fn child_probe() {
    let Ok(mode) = std::env::var(MODE) else {
        return;
    };
    match mode.as_str() {
        "sleep" => {
            if let Ok(path) = std::env::var("SILVORTEX_PROCESS_PID_FILE") {
                std::fs::write(path, std::process::id().to_string()).unwrap();
            }
            thread::sleep(Duration::from_millis(1500));
        }
        "parent" | "escaped" => {
            let mut child = command("sleep");
            child
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            #[cfg(unix)]
            if mode == "escaped" {
                use std::os::unix::process::CommandExt;
                child.process_group(0);
            }
            let mut child = child.spawn().unwrap();
            // The probe parent exits without joining: exercise inherited pipes.
            thread::spawn(move || {
                let _ = child.wait();
            });
        }
        "stdout-flood" | "stderr-flood" => {
            if mode == "stdout-flood" {
                print!("{}", "x".repeat(8192));
                io::stdout().flush().unwrap();
            } else {
                eprint!("{}", "e".repeat(8192));
                io::stderr().flush().unwrap();
            }
            thread::sleep(Duration::from_millis(1500));
        }
        "duplex" => {
            io::stdout().write_all(&vec![b'o'; 128 * 1024]).unwrap();
            io::stderr().write_all(&vec![b'e'; 128 * 1024]).unwrap();
            let mut payload = Vec::new();
            io::stdin().read_to_end(&mut payload).unwrap();
            assert_eq!(payload, vec![b'i'; 256 * 1024]);
        }
        "nonzero" => {
            eprintln!("expected child failure");
            std::process::exit(7);
        }
        "capacity" => {
            let permits = Permit::reserve(MAX_IO_WORKERS).unwrap();
            assert_eq!(
                capture("nonzero", Duration::from_secs(1))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            drop(permits);
            assert_eq!(
                capture("nonzero", Duration::from_secs(2))
                    .unwrap()
                    .status
                    .code(),
                Some(7)
            );
            assert_eq!(IO_WORKERS.load(Ordering::Acquire), 0);
        }
        #[cfg(unix)]
        "stress" => {
            #[cfg(target_os = "linux")]
            let before = (
                std::fs::read_dir("/proc/self/fd").unwrap().count(),
                std::fs::read_dir("/proc/self/task").unwrap().count(),
            );
            for _ in 0..8 {
                assert_eq!(
                    capture("parent", Duration::from_millis(100))
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::TimedOut
                );
                assert_eq!(
                    IO_WORKERS.load(Ordering::Acquire),
                    0,
                    "I/O workers leaked after inherited pipes"
                );
                assert_eq!(
                    capture("stdout-flood", Duration::from_secs(2))
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::InvalidData
                );
                assert_eq!(
                    IO_WORKERS.load(Ordering::Acquire),
                    0,
                    "I/O workers leaked after output overflow"
                );
            }
            #[cfg(target_os = "linux")]
            {
                assert!(std::fs::read_dir("/proc/self/fd").unwrap().count() <= before.0);
                assert!(std::fs::read_dir("/proc/self/task").unwrap().count() <= before.1);
            }
            assert!(
                capture("ok", Duration::from_secs(2))
                    .unwrap()
                    .status
                    .success()
            );
        }
        _ => (),
    }
}

#[test]
fn collects_both_streams_while_writing_more_than_pipe_capacity() {
    let output = run_command_output(
        &mut command("duplex"),
        Some(&vec![b'i'; 256 * 1024]),
        Duration::from_secs(5),
        OutputLimits::new(256 * 1024, 256 * 1024),
        "duplex probe",
    )
    .unwrap();
    assert!(
        output.status.success(),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.iter().filter(|byte| **byte == b'o').count() >= 128 * 1024);
    assert_eq!(output.stderr, vec![b'e'; 128 * 1024]);
}

#[test]
fn inherited_pipes_share_the_child_deadline() {
    let started = Instant::now();
    let error = capture("parent", Duration::from_millis(150)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[cfg(unix)]
#[test]
fn escaped_descendant_cannot_keep_pipe_workers_alive() {
    let started = Instant::now();
    let error = capture("escaped", Duration::from_millis(150)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn output_overflow_terminates_without_waiting_for_eof() {
    for mode in ["stdout-flood", "stderr-flood"] {
        let started = Instant::now();
        let error = capture(mode, Duration::from_secs(5)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeded 4096 bytes"));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn failure_kills_owned_group_but_not_an_unrelated_child() {
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut unrelated = OwnedChild(
        command("sleep")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let path = std::env::temp_dir().join(format!("bounded-group-probe-{}.pid", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let error = run_command_output(
        command("parent").env("SILVORTEX_PROCESS_PID_FILE", &path),
        None,
        Duration::from_millis(500),
        OutputLimits::new(4096, 4096),
        "group isolation probe",
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let pid = std::fs::read_to_string(&path)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let _ = std::fs::remove_file(path);
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"));
        if stat
            .as_ref()
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
            || stat.as_ref().is_ok_and(|stat| {
                stat.rsplit_once(") ")
                    .is_some_and(|(_, tail)| tail.starts_with('Z'))
            })
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "owned descendant remained running"
        );
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        unrelated.0.try_wait().unwrap().is_none(),
        "unrelated child was terminated"
    );
}

#[test]
fn preserves_nonzero_exit_and_rejects_invalid_limits_before_spawn() {
    assert_eq!(
        capture("nonzero", Duration::from_secs(2))
            .unwrap()
            .status
            .code(),
        Some(7)
    );
    for (timeout, limits) in [
        (Duration::ZERO, OutputLimits::new(1, 1)),
        (Duration::MAX, OutputLimits::new(1, 1)),
        (Duration::from_secs(1), OutputLimits::new(0, 1)),
    ] {
        let error = run_command_output(
            &mut Command::new("/does-not-exist/process-probe"),
            None,
            timeout,
            limits,
            "invalid probe",
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}

#[test]
fn worker_saturation_is_rejected_before_spawn_and_releases_permits() {
    let result = capture("capacity", Duration::from_secs(5)).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[cfg(unix)]
#[test]
fn repeated_failures_release_threads_descriptors_and_allow_recovery() {
    let result = capture("stress", Duration::from_secs(10)).unwrap();
    assert!(
        result.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
