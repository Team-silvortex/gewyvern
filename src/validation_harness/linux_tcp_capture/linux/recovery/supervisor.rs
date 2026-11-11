use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{ValidationError, WORKER_PARENT, tool};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Resource {
    fn capture(path: &Path) -> Result<Self, ValidationError> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(ValidationError::new(
                "capture resource is not an owned directory",
            ));
        }
        Ok(Self {
            path: path.to_owned(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    fn validate_path(&self, root: &str, pid: u32) -> Result<(), ValidationError> {
        let prefix = format!("gewyvern-capture-{pid}-");
        let suffix = self
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix(&prefix));
        if self.path.parent() != Some(Path::new(root))
            || !suffix.is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(ValidationError::new(
                "refusing foreign capture resource path",
            ));
        }
        Ok(())
    }

    fn exists_owned(&self) -> Result<bool, ValidationError> {
        match fs::symlink_metadata(&self.path) {
            Ok(meta)
                if meta.is_dir()
                    && !meta.file_type().is_symlink()
                    && meta.dev() == self.device
                    && meta.ino() == self.inode =>
            {
                Ok(true)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Ok(_) => Err(ValidationError::new(
                "capture resource identity changed; refusing cleanup",
            )),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Ready {
    pid: u32,
    pins: [Resource; 2],
    cgroup: Resource,
    pub(super) link_ids: [u32; 2],
}

impl Ready {
    pub(super) fn capture(
        trace: &Path,
        packet: &Path,
        cgroup: &Path,
        link_ids: [u32; 2],
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            pid: std::process::id(),
            pins: [Resource::capture(trace)?, Resource::capture(packet)?],
            cgroup: Resource::capture(cgroup)?,
            link_ids,
        })
    }

    fn validate(&self, pid: u32) -> Result<(), ValidationError> {
        if self.pid != pid
            || pid <= 1
            || self.link_ids.contains(&0)
            || self.link_ids[0] == self.link_ids[1]
            || self.pins[0].path == self.pins[1].path
        {
            return Err(ValidationError::new(
                "invalid capture worker ownership manifest",
            ));
        }
        for resource in &self.pins {
            resource.validate_path("/sys/fs/bpf", pid)?;
        }
        self.cgroup.validate_path("/sys/fs/cgroup", pid)?;
        Ok(())
    }

    fn reap(&self) -> Result<usize, ValidationError> {
        self.validate(self.pid)?;
        // Called only after waiting for the exact owned Child. Never sweep by prefix.
        let mut removed = 0;
        for resource in &self.pins {
            if resource.exists_owned()? {
                fs::remove_dir_all(&resource.path)?;
                removed += 1;
            }
        }
        if self.cgroup.exists_owned()? {
            if !fs::read_to_string(self.cgroup.path.join("cgroup.procs"))?
                .trim()
                .is_empty()
            {
                return Err(ValidationError::new(
                    "refusing cleanup of a populated capture cgroup",
                ));
            }
            fs::remove_dir(&self.cgroup.path)?;
            removed += 1;
        }
        Ok(removed)
    }

    pub(super) fn verify_removed(&self) -> Result<(), ValidationError> {
        for resource in self.pins.iter().chain(std::iter::once(&self.cgroup)) {
            if resource.exists_owned()? {
                return Err(ValidationError::new(
                    "owned capture resource remained after cleanup",
                ));
            }
        }
        Ok(())
    }
}

pub(super) fn verify_links(
    ids: &[u32; 2],
    present: bool,
    out: &Path,
) -> Result<(), ValidationError> {
    let bytes = tool(Command::new("bpftool").args(["-j", "link", "show"]), out)?;
    let links: Vec<Value> = serde_json::from_slice(&bytes)?;
    for id in ids {
        if links
            .iter()
            .any(|link| link["id"].as_u64() == Some(u64::from(*id)))
            != present
        {
            return Err(ValidationError::new(
                "owned BPF link lifetime did not follow the worker",
            ));
        }
    }
    Ok(())
}

struct Worker {
    child: Child,
    ready: Option<Ready>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
        if self.child.wait().is_ok()
            && let Some(ready) = &self.ready
            && let Err(error) = ready.reap()
        {
            eprintln!("owned recovery worker cleanup failed: {error}");
        }
    }
}

fn read_ready(path: &Path) -> Result<Ready, ValidationError> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > 16 * 1024 {
        return Err(ValidationError::new("invalid capture worker manifest file"));
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 {
        return Err(ValidationError::new("oversized capture worker manifest"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn wait_for_exit(child: &mut Child) -> Result<ExitStatus, ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(ValidationError::new(
                "capture worker did not exit within its deadline",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn exercise(out: &Path, mode: &str) -> Result<Value, ValidationError> {
    if !matches!(mode, "error" | "abort" | "kill") {
        return Err(ValidationError::new("invalid capture exit scenario"));
    }
    let path = out.join(format!("worker-{mode}"));
    let child = Command::new(std::env::current_exe()?)
        .args([
            "--json",
            "linux-tcp-capture",
            "--capture-recovery",
            "--out-dir",
        ])
        .arg(&path)
        .env(WORKER_PARENT, std::process::id().to_string())
        .stdin(Stdio::piped())
        .stdout(File::create(out.join(format!("worker-{mode}-stdout.log")))?)
        .stderr(File::create(out.join(format!("worker-{mode}-stderr.log")))?)
        .spawn()?;
    let mut worker = Worker { child, ready: None };
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if path.join("ready.json").exists() {
            let ready = read_ready(&path.join("ready.json"))?;
            ready.validate(worker.child.id())?;
            for resource in ready.pins.iter().chain(std::iter::once(&ready.cgroup)) {
                if !resource.exists_owned()? {
                    return Err(ValidationError::new(
                        "worker resource disappeared before fault injection",
                    ));
                }
            }
            worker.ready = Some(ready);
            break;
        }
        if worker.child.try_wait()?.is_some() {
            return Err(ValidationError::new(
                "capture worker exited before readiness; inspect worker logs",
            ));
        }
        if Instant::now() >= deadline {
            return Err(ValidationError::new("capture worker readiness timed out"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let ready = worker
        .ready
        .as_ref()
        .ok_or_else(|| ValidationError::new("missing worker readiness"))?;
    verify_links(&ready.link_ids, true, out)?;
    if mode == "kill" {
        worker.child.kill()?;
    } else {
        worker
            .child
            .stdin
            .take()
            .ok_or_else(|| ValidationError::new("missing worker control pipe"))?
            .write_all(if mode == "abort" { b"a" } else { b"e" })?;
    }
    let status = wait_for_exit(&mut worker.child)?;
    let expected_signal = match mode {
        "abort" => Some(libc::SIGABRT),
        "kill" => Some(libc::SIGKILL),
        _ => None,
    };
    if status.success() || status.signal() != expected_signal || status.core_dumped() {
        return Err(ValidationError::new(format!(
            "unexpected worker exit or core dump: {status}"
        )));
    }
    if path.join("summary.json").exists() || path.join("packet-export.json").exists() {
        return Err(ValidationError::new(
            "crashed worker published a successful capture",
        ));
    }
    // Check detachment before unlinking any residual map/program pins.
    verify_links(&ready.link_ids, false, out)?;
    let removed = ready.reap()?;
    ready.verify_removed()?;
    if (mode == "error" && removed != 0) || (mode != "error" && removed != 3) {
        return Err(ValidationError::new(
            "unexpected unwind/orphan cleanup behavior",
        ));
    }
    Ok(
        json!({"test_passed":true,"signal":status.signal(),"exit_code":status.code(),"core_dumped":status.core_dumped(),
        "links_detached_before_cleanup":true,"link_ids":ready.link_ids,"reaped_owned_directories":removed,
        "success_artifacts_absent":true,"owned_resources_removed":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_path_cannot_escape_or_select_another_worker() {
        for path in [
            "/sys/fs/bpf/gewyvern-capture-77-123",
            "/sys/fs/bpf/gewyvern-capture-12-123/child",
            "/sys/fs/cgroup/gewyvern-capture-12-123",
            "/tmp/gewyvern-capture-12-123",
            "/sys/fs/bpf/gewyvern-capture-12-",
        ] {
            assert!(
                Resource {
                    path: path.into(),
                    device: 1,
                    inode: 1
                }
                .validate_path("/sys/fs/bpf", 12)
                .is_err()
            );
        }
        assert!(
            Resource {
                path: "/sys/fs/bpf/gewyvern-capture-12-123".into(),
                device: 1,
                inode: 1
            }
            .validate_path("/sys/fs/bpf", 12)
            .is_ok()
        );
    }

    #[test]
    fn changed_identity_or_symlink_is_never_owned() {
        let root =
            std::env::temp_dir().join(format!("gewy-capture-ownership-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let dir = root.join("owned");
        fs::create_dir(&dir).unwrap();
        let mut resource = Resource::capture(&dir).unwrap();
        resource.inode += 1;
        assert!(resource.exists_owned().is_err());
        fs::remove_dir(&dir).unwrap();
        std::os::unix::fs::symlink(&root, &dir).unwrap();
        assert!(resource.exists_owned().is_err());
        fs::remove_file(&dir).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}
