use super::{
    MAX_GEWYLANG_INCLUDE_DEPTH, MAX_GEWYLANG_SOURCE_GRAPH_BYTES, MAX_GEWYLANG_SOURCE_GRAPH_FILES,
    read_source_file,
};
use crate::SyntaxError as DslError;
use std::path::{Path, PathBuf};

pub(super) struct SourceGraphState {
    active_paths: Vec<PathBuf>,
    source_files: usize,
    source_bytes: usize,
}

impl SourceGraphState {
    pub(super) fn new(entry_path: Option<&Path>, source_bytes: usize) -> Result<Self, DslError> {
        if source_bytes > MAX_GEWYLANG_SOURCE_GRAPH_BYTES {
            return Err(source_graph_bytes_exceeded());
        }
        Ok(Self {
            active_paths: entry_path.into_iter().map(Path::to_path_buf).collect(),
            source_files: 1,
            source_bytes,
        })
    }

    pub(super) fn load_include(&mut self, path: &Path) -> Result<String, DslError> {
        if self.active_paths.iter().any(|active| active == path) {
            return Err(DslError::InvalidValue(format!(
                "pipeline include cycle detected at '{}'",
                path.display()
            )));
        }
        if self.active_paths.len() > MAX_GEWYLANG_INCLUDE_DEPTH {
            return Err(DslError::InvalidValue(format!(
                "gewylang include depth exceeds {MAX_GEWYLANG_INCLUDE_DEPTH}"
            )));
        }
        if self.source_files >= MAX_GEWYLANG_SOURCE_GRAPH_FILES {
            return Err(DslError::InvalidValue(format!(
                "gewylang source graph exceeds {MAX_GEWYLANG_SOURCE_GRAPH_FILES} files"
            )));
        }

        let input = read_source_file(path)?;
        let source_bytes = self
            .source_bytes
            .checked_add(input.len())
            .ok_or_else(source_graph_bytes_exceeded)?;
        if source_bytes > MAX_GEWYLANG_SOURCE_GRAPH_BYTES {
            return Err(source_graph_bytes_exceeded());
        }

        self.source_files += 1;
        self.source_bytes = source_bytes;
        self.active_paths.push(path.to_path_buf());
        Ok(input)
    }

    pub(super) fn leave_include(&mut self, path: &Path) {
        let active = self
            .active_paths
            .pop()
            .expect("included source must have an active graph entry");
        debug_assert_eq!(active, path);
    }
}

fn source_graph_bytes_exceeded() -> DslError {
    DslError::InvalidValue(format!(
        "gewylang source graph exceeds {MAX_GEWYLANG_SOURCE_GRAPH_BYTES} bytes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_source(label: &str, contents: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gewy-source-graph-{label}-{}-{unique}.gewy",
            std::process::id()
        ));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn entry_bytes_are_part_of_the_source_graph_budget() {
        let err = SourceGraphState::new(None, MAX_GEWYLANG_SOURCE_GRAPH_BYTES + 1)
            .err()
            .expect("oversized graph should fail");
        assert_eq!(
            err,
            DslError::InvalidValue(format!(
                "gewylang source graph exceeds {MAX_GEWYLANG_SOURCE_GRAPH_BYTES} bytes"
            ))
        );
    }

    #[test]
    fn source_file_budget_is_checked_before_opening_another_path() {
        let mut state = SourceGraphState {
            active_paths: Vec::new(),
            source_files: MAX_GEWYLANG_SOURCE_GRAPH_FILES,
            source_bytes: 0,
        };
        let err = state
            .load_include(Path::new("does-not-need-to-exist.gewy"))
            .unwrap_err();
        assert_eq!(
            err,
            DslError::InvalidValue(format!(
                "gewylang source graph exceeds {MAX_GEWYLANG_SOURCE_GRAPH_FILES} files"
            ))
        );
    }

    #[test]
    fn aggregate_source_bytes_are_checked_after_bounded_read() {
        let path = temp_source("aggregate-bytes", "ab");
        let mut state = SourceGraphState {
            active_paths: Vec::new(),
            source_files: 1,
            source_bytes: MAX_GEWYLANG_SOURCE_GRAPH_BYTES - 1,
        };
        let err = state.load_include(&path).unwrap_err();
        fs::remove_file(path).unwrap();
        assert_eq!(
            err,
            DslError::InvalidValue(format!(
                "gewylang source graph exceeds {MAX_GEWYLANG_SOURCE_GRAPH_BYTES} bytes"
            ))
        );
    }
}
