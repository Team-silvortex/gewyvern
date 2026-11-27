//! Journal the eviction set before switching the index. Called under the shelf writer lock.
use super::{MAX_HISTORY_DIRECTORY_ENTRIES, publication};
use ring::digest::{Context, SHA256, digest};
use serde::{Deserialize, Serialize};
use silvortex_bounded_io::{atomic_write_bounded_private_file, read_bounded_utf8_regular_file};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

type Entries = [(u128, PathBuf)];
type Digest = [u8; 32];
const JOURNAL: &str = ".gewyvern-prune-v1.json";
const LIMIT: u64 = 1024 * 1024;

pub(super) fn index_id(entries: &Entries) -> Digest {
    let mut hash = Context::new(&SHA256);
    for (ts, _) in entries {
        hash.update(&ts.to_be_bytes());
    }
    hash.finish().as_ref().try_into().expect("SHA-256 digest")
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    previous: Digest,
    next: Digest,
    retired: Vec<Retired>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Retired {
    timestamp: u128,
    anchor: Anchor,
}

#[derive(Deserialize, Serialize, PartialEq, Eq)]
enum Anchor {
    Publication([u8; 16]),
    LegacyMeta(Digest),
    Empty,
}

fn metadata(path: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("inspect {}: {e}", path.display())),
    }
}

fn identify(dir: &Path, ts: u128) -> Result<Anchor, String> {
    if !metadata(dir)?.is_some_and(|m| m.is_dir()) {
        return Err(format!("not a real retired directory: {}", dir.display()));
    }
    if metadata(&dir.join(publication::STAMP))?.is_some() {
        return Ok(Anchor::Publication(publication::stamp_nonce(dir, ts)?));
    }
    if metadata(&dir.join("meta.json"))?.is_some() {
        #[derive(Deserialize)]
        struct Legacy {
            updated_unix_ms: u128,
        }
        let body = read_bounded_utf8_regular_file(&dir.join("meta.json"), LIMIT)
            .map_err(|e| e.to_string())?;
        if serde_json::from_str::<Legacy>(&body)
            .map_err(|e| e.to_string())?
            .updated_unix_ms
            != ts
        {
            return Err("legacy history metadata timestamp mismatch".into());
        }
        return Ok(Anchor::LegacyMeta(
            digest(&SHA256, body.as_bytes())
                .as_ref()
                .try_into()
                .expect("SHA-256 digest"),
        ));
    }
    if fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .next()
        .is_none()
    {
        return Ok(Anchor::Empty);
    }
    Err(format!("unidentified retired archive: {}", dir.display()))
}

pub(super) fn record(
    root: &Path,
    previous: Digest,
    kept: &Entries,
    retired: &Entries,
) -> Result<Option<Plan>, String> {
    if retired.is_empty() {
        return Ok(None);
    }
    if retired.len() > MAX_HISTORY_DIRECTORY_ENTRIES {
        return Err("too many history evictions".into());
    }
    let retired = retired
        .iter()
        .map(|(ts, path)| {
            Ok(Retired {
                timestamp: *ts,
                anchor: identify(path, *ts)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let plan = Plan {
        previous,
        next: index_id(kept),
        retired,
    };
    let bytes = serde_json::to_vec(&plan).map_err(|e| e.to_string())?;
    atomic_write_bounded_private_file(&root.join(JOURNAL), &bytes, LIMIT)
        .map_err(|e| format!("record history prune: {e}"))?;
    Ok(Some(plan))
}

pub(super) fn recover(root: &Path, committed: Option<&Entries>) -> Result<(), String> {
    let path = root.join(JOURNAL);
    if metadata(&path)?.is_none() {
        return Ok(());
    }
    let committed = committed.ok_or("pending history prune has no index")?;
    let bytes = read_bounded_utf8_regular_file(&path, LIMIT)
        .map_err(|e| format!("read history prune: {e}"))?;
    let plan: Plan =
        serde_json::from_str(&bytes).map_err(|e| format!("invalid history prune: {e}"))?;
    plan.finish(root, committed)
}

impl Plan {
    pub(super) fn finish(&self, root: &Path, committed: &Entries) -> Result<(), String> {
        let current = index_id(committed);
        if current == self.next {
            let live: BTreeSet<_> = committed.iter().map(|(ts, _)| *ts).collect();
            let mut seen = BTreeSet::new();
            if self.retired.is_empty()
                || self.retired.len() > MAX_HISTORY_DIRECTORY_ENTRIES
                || self
                    .retired
                    .iter()
                    .any(|r| live.contains(&r.timestamp) || !seen.insert(r.timestamp))
            {
                return Err("invalid history prune membership".into());
            }
            for item in &self.retired {
                item.remove(root)?;
            }
        } else if current != self.previous {
            return Err("history prune does not match committed index".into());
        }
        // At the previous index no eviction was committed; at the next all work finished.
        fs::remove_file(root.join(JOURNAL)).map_err(|e| format!("clear history prune: {e}"))
    }
}

impl Retired {
    fn remove(&self, root: &Path) -> Result<(), String> {
        let dir = root.join(self.timestamp.to_string());
        let Some(meta) = metadata(&dir)? else {
            return Ok(());
        };
        if !meta.is_dir() {
            return Err(format!("refusing non-directory prune: {}", dir.display()));
        }
        let identity = identify(&dir, self.timestamp)?;
        let anchor = match &identity {
            Anchor::Publication(_) => Some(publication::STAMP),
            Anchor::LegacyMeta(_) => Some("meta.json"),
            Anchor::Empty => None,
        };
        if let Some(anchor) = anchor {
            if identity != self.anchor {
                return Err(format!(
                    "retired archive identity changed: {}",
                    dir.display()
                ));
            }
            // Keep identity until all payload is gone; a partially pruned tree
            // is still identifiable after SIGKILL. Never recurse through links.
            for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_name() == anchor {
                    continue;
                }
                if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                    fs::remove_dir_all(entry.path())
                } else {
                    fs::remove_file(entry.path())
                }
                .map_err(|e| format!("prune {}: {e}", entry.path().display()))?;
            }
            fs::remove_file(dir.join(anchor)).map_err(|e| format!("remove prune anchor: {e}"))?;
        }
        // Also handles death after the last unlink but before rmdir.
        fs::remove_dir(&dir).map_err(|e| format!("finish pruning {}: {e}", dir.display()))
    }
}
