//! Only test-owned paths become read-only; stderr remains independently writable.
use super::*;
use crate::boundaries::collector_with_output_dir;

#[path = "api_archive.rs"]
mod api_archive;
#[path = "history_interruption.rs"]
mod history_interruption;

#[test]
#[ignore = "Linux root/BPF/mount; GEWY_READONLY_OUT=new-directory, GEWY_OFFLOAD_EXPIRY_FAMILY=4|6, GEWY_STATUS_CASE=running|terminal[-api]"]
fn production_capture_read_only_outputs_and_repair() {
    exercise(false);
}

#[test]
#[ignore = "Linux root/BPF/mount; GEWY_READONLY_OUT=new-directory, GEWY_OFFLOAD_EXPIRY_FAMILY=4|6, GEWY_STATUS_CASE=running-api|terminal-api"]
fn production_capture_read_only_api_and_outputs() {
    exercise(true);
}

fn mount_read_only(dir: &Path) {
    assert!(
        Command::new("mount")
            .args(["-o", "bind,ro"])
            .arg(dir)
            .arg(dir)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(
        fs::File::create_new(dir.join("write-probe"))
            .unwrap_err()
            .raw_os_error(),
        Some(libc::EROFS)
    );
}

fn exercise(block_api: bool) {
    // Descendants share the test thread's namespace, never the host mount table.
    assert_eq!(unsafe { libc::unshare(libc::CLONE_NEWNS) }, 0);
    assert!(
        Command::new("mount")
            .args(["--make-rprivate", "/"])
            .status()
            .unwrap()
            .success()
    );
    let case = std::env::var("GEWY_STATUS_CASE").unwrap();
    assert!(matches!(
        case.as_str(),
        "running" | "terminal" | "running-api" | "terminal-api"
    ));
    let api = case.ends_with("-api");
    assert!(!block_api || api);
    let mut options = OPTIONS.to_vec();
    if api {
        options.extend(["--api-socket", "127.0.0.1:0"]);
    }
    let spawn = |scope: &Scope, dir: &Path| {
        let runtime = dir.with_file_name("runtime");
        fs::create_dir_all(&runtime).unwrap();
        collector_with_output_dir(scope, PORT, &runtime, dir, &options)
    };
    let mut seed = None;
    let mut f = start_partial_using("GEWY_READONLY_OUT", |scope, dir| {
        if block_api {
            let seed_dir = dir.with_file_name("seed");
            fs::create_dir(&seed_dir).unwrap();
            let mut first = spawn(scope, &seed_dir);
            let id = wait_for_run(&mut first, &seed_dir, None);
            window(&mut first, &seed_dir, &id, 0);
            seed = Some(clean_stop(&mut first, &seed_dir, 0, 0));
        }
        spawn(scope, dir)
    });
    let dir = &f.capture_dir;
    let runtime = f.out.join("runtime");
    pause_main_loop(&mut f.capture, dir);
    let files = || ["status.json", "export.json"].map(|name| fs::read(dir.join(name)).unwrap());
    let before = files();
    let stale = snapshot(dir);
    assert_eq!(Some(&stale), f.partial.capture.as_ref());
    assert!(!stale.final_accounted && !stale.reconciles());
    assert_eq!(export(dir).facts, f.partial.facts);
    let state = runtime.join("state");
    let archive = seed.as_ref().map(|old| {
        assert_ne!(old.run_id, stale.run_id);
        api_archive::check(&state, &[old]);
        mount_read_only(&state);
        api_archive::fingerprint(&state)
    });
    mount_read_only(dir);
    phase(&mut f.server, &mut f.client, &f.out, 5);
    if case.starts_with("terminal") {
        signal(&f.capture, libc::SIGTERM);
    }
    signal(&f.capture, libc::SIGCONT);
    let exit = wait(&mut f.capture, 10).code().unwrap();
    assert_eq!(exit, 1);
    detached(&f.scope);
    assert_eq!(files(), before, "neither CLI file may advance");
    let log = fs::read_to_string(runtime.join("stderr.log")).unwrap();
    let envelope: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
    let error = &envelope["error"];
    assert_eq!(error["code"], "output_write_failed");
    assert_eq!(error["exit_code"], exit);
    fs::write(f.out.join("failure.log"), &log).unwrap();
    assert_eq!(error["category"], "io");
    assert_eq!(error["retryable"], true);
    let message = error["message"].as_str().unwrap();
    assert!(
        message.contains("export.json")
            && message.contains("status.json was not written [capture_status_write_failed]")
    );
    let latest = state.join("latest/api/v1/latest");
    let history = state.join("history/api/v1");
    let api_failure = if let Some(before) = &archive {
        assert_eq!(&api_archive::fingerprint(&state), before);
        api_archive::check(&state, &[seed.as_ref().unwrap()]);
        None
    } else if api {
        let failed = export(&latest);
        let s = failed.capture.as_ref().unwrap();
        assert_eq!(s.state, CaptureState::Failed);
        assert_eq!(s.run_id, stale.run_id);
        assert!(s.final_accounted && s.reconciles() && s.incomplete);
        assert_eq!(
            [
                s.kernel.matched,
                s.kernel.ring_dropped,
                s.received,
                s.retained,
                s.userspace_dropped,
                s.decode_rejected
            ],
            [42, 8, 34, 9, 16, 9]
        );
        check_packets(&failed, &client_packets(&f.out, 5, true));
        let dirs: Vec<_> = fs::read_dir(&history)
            .unwrap()
            .map(Result::unwrap)
            .filter(|e| e.file_type().unwrap().is_dir())
            .collect();
        assert_eq!(dirs.len(), 1);
        let archived = export(&dirs[0].path());
        assert_eq!(archived.capture, failed.capture);
        assert_eq!(archived.facts, failed.facts);
        Some(s.clone())
    } else {
        assert!(!latest.exists() && !history.exists());
        None
    };
    phase(&mut f.server, &mut f.client, &f.out, 6);
    assert_eq!(files(), before, "downtime must not update the old run");
    assert!(Command::new("umount").arg(dir).status().unwrap().success());
    if let Some(before) = &archive {
        assert!(
            Command::new("umount")
                .arg(&state)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(&api_archive::fingerprint(&state), before);
    }
    assert_eq!(files(), before, "repair must expose the original files");
    let mut restarted = spawn(&f.scope, dir);
    let run = wait_for_run(&mut restarted, dir, Some(&stale.run_id));
    let fresh = window(&mut restarted, dir, &run, 0);
    assert!(fresh.facts.is_empty());
    let fresh = fresh.capture.unwrap();
    assert!(fresh.reconciles() && !fresh.has_loss() && !fresh.incomplete);
    if let Some(old) = &seed {
        assert_ne!(fresh.run_id, old.run_id);
        api_archive::check(&state, &[old]);
    }
    phase(&mut f.server, &mut f.client, &f.out, 7);
    let recovered = window(&mut restarted, dir, &run, 5);
    check_recovery(&recovered, &f.out, 7, 1);
    let repaired = clean_stop(&mut restarted, dir, 5, 0);
    assert_eq!(repaired.run_id, run);
    if let Some(old) = &seed {
        api_archive::check(&state, &[&repaired, old]);
    }
    finish_peers(&mut f);
    let mut summary = json!({
        "case":case,"family":if family()==0 {4} else {6},"exit":exit,"error":error["code"],
        "status_error":"capture_status_write_failed","stale":stale,"api_failure":api_failure,"fresh":fresh,"repaired":repaired,
        "stale_status_sha256":hash(&before[0]),"stale_export_sha256":hash(&before[1])
    });
    if let Some(old) = seed {
        summary["archive"] = json!({"seed_run_id":old.run_id,"tree_sha256_before_and_after":archive,
            "history_before":1,"history_after_fault":1,"history_after_repair":2,
            "persistence_error_reported":message.contains("[snapshot_persist_failed]")});
    }
    fs::write(
        f.out.join("summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    assert_eq!(
        message.contains("[snapshot_persist_failed]"),
        block_api,
        "{message}"
    );
    if block_api {
        assert!(message.contains(state.to_str().unwrap()), "{message}");
    }
}
