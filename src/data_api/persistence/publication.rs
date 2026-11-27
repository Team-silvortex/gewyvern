//! A single in-flight rename, protected by the shelf's existing writer lock.
use super::HISTORY_STAGE;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use silvortex_bounded_io::{atomic_write_bounded_private_file, read_bounded_utf8_regular_file};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const INTENT: &str = ".gewyvern-inflight-v1.json";
pub(super) const STAMP: &str = ".gewyvern-publication-v1.json";
const LIMIT: u64 = 256;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Publication {
    timestamp: u128,
    nonce: [u8; 16],
}

pub(super) fn record(root: &Path, stage: &Path, timestamp: u128) -> Result<(), String> {
    let mut nonce = [0; 16];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| "history publication nonce unavailable")?;
    let bytes = serde_json::to_vec(&Publication { timestamp, nonce }).map_err(|e| e.to_string())?;
    for path in [stage.join(STAMP), root.join(INTENT)] {
        atomic_write_bounded_private_file(&path, &bytes, LIMIT)
            .map_err(|e| format!("record history publication at {}: {e}", path.display()))?;
    }
    Ok(())
}

fn load(path: &Path) -> Result<Publication, String> {
    let bytes = read_bounded_utf8_regular_file(path, LIMIT)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&bytes)
        .map_err(|e| format!("invalid history publication {}: {e}", path.display()))
}

pub(super) fn stamp_nonce(dir: &Path, timestamp: u128) -> Result<[u8; 16], String> {
    let stamp = load(&dir.join(STAMP))?;
    if stamp.timestamp != timestamp {
        return Err("retired archive timestamp does not match its stamp".into());
    }
    Ok(stamp.nonce)
}

pub(super) fn recover(root: &Path, committed: Option<&[(u128, PathBuf)]>) -> Result<(), String> {
    let path = root.join(INTENT);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("inspect history publication: {e}")),
        Ok(_) => {}
    }
    let intent = load(&path)?;
    // Never infer commitment from a directory scan when a publication was underway.
    let committed = committed.ok_or("in-flight history publication has no committed index")?;
    if committed.iter().any(|(ts, _)| *ts == intent.timestamp) {
        return clear(root);
    }
    let dest = root.join(intent.timestamp.to_string());
    match fs::symlink_metadata(&dest) {
        Err(e) if e.kind() == ErrorKind::NotFound => return clear(root),
        Err(e) => return Err(format!("inspect interrupted archive: {e}")),
        Ok(m) if m.is_dir() && load(&dest.join(STAMP))? == intent => {}
        Ok(_) => {
            return Err(format!(
                "refusing mismatched interrupted archive: {}",
                dest.display()
            ));
        }
    }
    let stage = root.join(HISTORY_STAGE);
    match fs::symlink_metadata(&stage) {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(format!("inspect history stage: {e}")),
        Ok(_) => return Err("interrupted archive and history stage both exist".into()),
    }
    // Move the proven orphan back to the one reserved slot before clearing
    // the intent. A kill between these operations can safely repeat recovery.
    fs::rename(&dest, stage).map_err(|e| format!("reclaim interrupted archive: {e}"))?;
    clear(root)
}

pub(super) fn clear(root: &Path) -> Result<(), String> {
    fs::remove_file(root.join(INTENT)).map_err(|e| format!("clear history publication: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(label: &str) -> (PathBuf, PathBuf) {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gewy-rename-{label}-{}-{n}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let stage = super::super::prepare_history_stage(&root).unwrap();
        record(&root, &stage, 42).unwrap();
        (root, stage)
    }

    #[test]
    fn interrupted_rename_is_reversible_only_for_the_recorded_unindexed_publication() {
        let (root, stage) = pending("unindexed");
        let dest = root.join("42");
        fs::rename(&stage, &dest).unwrap();
        let other = root.join("43");
        fs::create_dir(&other).unwrap();
        fs::copy(dest.join(STAMP), other.join(STAMP)).unwrap();
        assert!(recover(&root, None).is_err());
        assert!(dest.is_dir() && root.join(INTENT).is_file());
        recover(&root, Some(&[])).unwrap();
        assert!(stage.is_dir() && other.is_dir() && !dest.exists());
        recover(&root, Some(&[])).unwrap();
        assert!(!root.join(INTENT).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pre_rename_and_post_commit_receipts_never_remove_committed_entries() {
        let (root, stage) = pending("indexed");
        recover(&root, Some(&[])).unwrap();
        assert!(stage.is_dir());
        record(&root, &stage, 42).unwrap();
        let dest = root.join("42");
        fs::rename(&stage, &dest).unwrap();
        recover(&root, Some(&[(42, dest.clone())])).unwrap();
        assert!(dest.is_dir() && !stage.exists() && !root.join(INTENT).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ambiguous_stage_and_replaced_generation_fail_without_removal() {
        let (root, stage) = pending("replaced");
        let dest = root.join("42");
        fs::rename(&stage, &dest).unwrap();
        fs::create_dir(&stage).unwrap();
        assert!(recover(&root, Some(&[])).is_err());
        fs::remove_dir(&stage).unwrap();
        fs::write(dest.join(STAMP), br#"{"timestamp":42,"nonce":[0]}"#).unwrap();
        assert!(recover(&root, Some(&[])).is_err());
        fs::write(
            dest.join(STAMP),
            serde_json::to_vec(&Publication {
                timestamp: 43,
                nonce: [0; 16],
            })
            .unwrap(),
        )
        .unwrap();
        assert!(recover(&root, Some(&[])).is_err());
        assert!(dest.is_dir() && root.join(INTENT).is_file() && !stage.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_archive_symlink_is_never_followed_or_moved() {
        let (root, stage) = pending("link");
        let outside = root.join("outside");
        fs::rename(&stage, &outside).unwrap();
        let dest = root.join("42");
        std::os::unix::fs::symlink(&outside, &dest).unwrap();
        assert!(recover(&root, Some(&[])).is_err());
        assert!(dest.is_symlink() && outside.join(STAMP).is_file() && !stage.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
