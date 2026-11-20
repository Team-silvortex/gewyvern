use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixListener;
use std::path::Path;

use super::{SocketInputError, bind_unix_socket_listener, remove_unix_socket_file};

struct SocketPath<'a> {
    path: &'a Path,
    identity: Option<(u64, u64)>,
}

impl SocketPath<'_> {
    fn cleanup(&mut self) -> Result<(), SocketInputError> {
        let Some(identity) = self.identity.take() else {
            return Ok(());
        };
        match fs::symlink_metadata(self.path) {
            Ok(metadata)
                if metadata.file_type().is_socket()
                    && (metadata.dev(), metadata.ino()) == identity =>
            {
                fs::remove_file(self.path)
                    .map_err(|error| SocketInputError::BindFailed(error.to_string()))
            }
            Ok(_) => Err(SocketInputError::BindFailed(
                "socket path replaced; refusing to remove a different entry".into(),
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(SocketInputError::BindFailed(error.to_string())),
        }
    }
}

impl Drop for SocketPath<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub(super) fn with_listener<T>(
    path: &str,
    run: impl FnOnce(&UnixListener) -> Result<T, SocketInputError>,
) -> Result<T, SocketInputError> {
    // As with bind/remove, the caller must use a trusted parent directory;
    // inode checks avoid stale ownership, not concurrent directory mutation.
    remove_unix_socket_file(path)?;
    let listener = bind_unix_socket_listener(path)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| SocketInputError::BindFailed(error.to_string()))?;
    let mut owned = SocketPath {
        path: Path::new(path),
        identity: Some((metadata.dev(), metadata.ino())),
    };
    let result = run(&listener);
    drop(listener);
    let cleanup = owned.cleanup();
    // Preserve the original read/parse/runtime error even if cleanup also fails.
    result.and_then(|value| cleanup.map(|()| value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::DirBuilderExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn directory() -> Directory {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            Directory(std::env::temp_dir().join(format!("gw-unix-{}-{nonce}", std::process::id())));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory.0)
            .unwrap();
        directory
    }

    #[test]
    fn successful_and_failed_sessions_remove_their_socket_and_can_rebind() {
        let directory = directory();
        let path = directory.0.join("facts.sock");
        for fail in [true, false, true, false] {
            let result = with_listener(path.to_str().unwrap(), |_| {
                if fail {
                    Err(SocketInputError::ReadFailed("injected".into()))
                } else {
                    Ok(())
                }
            });
            assert_eq!(result.is_err(), fail);
            assert!(!path.exists());
        }
    }

    #[test]
    fn cleanup_preserves_replacement_files_and_sockets() {
        let directory = directory();
        for socket in [false, true] {
            let path = directory.0.join(if socket { "socket" } else { "file" });
            let mut replacement = None;
            let result = with_listener(path.to_str().unwrap(), |_| {
                fs::remove_file(&path).unwrap();
                if socket {
                    replacement = Some(UnixListener::bind(&path).unwrap());
                } else {
                    fs::write(&path, "keep").unwrap();
                }
                Ok(())
            });
            assert!(matches!(result, Err(SocketInputError::BindFailed(_))));
            assert!(path.exists());
            if !socket {
                assert_eq!(fs::read_to_string(&path).unwrap(), "keep");
            }
        }
    }

    #[test]
    fn cleanup_runs_on_unwind() {
        let directory = directory();
        let path = directory.0.join("facts.sock");
        assert!(
            std::panic::catch_unwind(|| {
                let _: Result<(), _> =
                    with_listener(path.to_str().unwrap(), |_| panic!("injected"));
            })
            .is_err()
        );
        assert!(!path.exists());
    }
}
