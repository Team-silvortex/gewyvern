use super::*;

// Include directory names as well as file bytes: even an unindexed, empty
// terminal entry would otherwise escape this comparison.
pub(super) fn fingerprint(state: &Path) -> String {
    fingerprint_paths(state, &["latest", "history"])
}

pub(super) fn fingerprint_paths(state: &Path, names: &[&str]) -> String {
    fn visit(state: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Option<String>>) {
        let ty = fs::symlink_metadata(path).unwrap().file_type();
        let relative = path.strip_prefix(state).unwrap().to_owned();
        if ty.is_dir() {
            entries.insert(relative, None);
            for entry in fs::read_dir(path).unwrap() {
                visit(state, &entry.unwrap().path(), entries);
            }
        } else {
            assert!(ty.is_file());
            entries.insert(relative, Some(hash(&fs::read(path).unwrap())));
        }
    }
    let mut entries = BTreeMap::new();
    for name in names {
        visit(state, &state.join(name), &mut entries);
    }
    hash(&serde_json::to_vec(&entries).unwrap())
}

pub(super) fn check(state: &Path, newest_first: &[&CaptureSnapshot]) {
    let root = state.join("history/api/v1");
    let index: Value = serde_json::from_slice(&fs::read(root.join("index.json")).unwrap()).unwrap();
    let rows = index["entries"].as_array().unwrap();
    assert_eq!(rows.len(), newest_first.len());
    let disk: BTreeSet<_> = fs::read_dir(&root)
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| e.file_type().unwrap().is_dir())
        .map(|e| e.path())
        .collect();
    let indexed: BTreeSet<_> = rows
        .iter()
        .map(|e| state.join(e["path"].as_str().unwrap()))
        .collect();
    assert_eq!(indexed, disk);
    assert_eq!(disk.len(), rows.len());
    for (row, expected) in rows.iter().zip(newest_first) {
        let dir = state.join(row["path"].as_str().unwrap());
        assert_eq!(export(&dir).capture.as_ref(), Some(*expected));
    }
    assert_eq!(
        export(&state.join("latest/api/v1/latest")).capture.as_ref(),
        newest_first.first().copied()
    );
}
