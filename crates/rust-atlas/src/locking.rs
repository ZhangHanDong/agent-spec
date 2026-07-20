use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use fs2::FileExt;

use crate::AtlasError;

pub struct WriterLease {
    file: File,
    graph_root: PathBuf,
}

impl WriterLease {
    pub fn try_acquire(graph_root: &Path) -> Result<Self, AtlasError> {
        fs::create_dir_all(graph_root).map_err(lock_io)?;
        let graph_root = fs::canonicalize(graph_root).map_err(lock_io)?;
        let runtime = graph_root.join(".runtime");
        reject_symlink(&runtime)?;
        fs::create_dir_all(&runtime).map_err(lock_io)?;
        let lock_path = runtime.join("build.lock");
        reject_symlink(&lock_path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(lock_io)?;
        if fs::canonicalize(&runtime).map_err(lock_io)? != runtime
            || fs::canonicalize(&lock_path).map_err(lock_io)? != lock_path
        {
            return Err(AtlasError::LiveState {
                detail: format!(
                    "lock path escaped the graph runtime: {}",
                    lock_path.display()
                ),
            });
        }
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Self { file, graph_root }),
            Err(error) if is_contention(&error) => Err(AtlasError::WriterBusy {
                graph_root: graph_root.to_string_lossy().into_owned(),
            }),
            Err(error) => Err(lock_io(error)),
        }
    }

    pub(crate) fn assert_graph(&self, graph_root: &Path) -> Result<(), AtlasError> {
        let graph_root = fs::canonicalize(graph_root).map_err(lock_io)?;
        if graph_root == self.graph_root {
            Ok(())
        } else {
            Err(AtlasError::Invariant(format!(
                "writer lease for {} cannot publish graph {}",
                self.graph_root.display(),
                graph_root.display()
            )))
        }
    }
}

impl Drop for WriterLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn reject_symlink(path: &Path) -> Result<(), AtlasError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(AtlasError::LiveState {
            detail: format!("lock path must not be a symlink: {}", path.display()),
        }),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(lock_io(error)),
    }
}

fn is_contention(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock || matches!(error.raw_os_error(), Some(11 | 35))
}

fn lock_io(error: std::io::Error) -> AtlasError {
    AtlasError::Io(error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::{AtlasError, BuildOptions, build, live::PendingJournal};

    fn fixture(name: &str) -> (PathBuf, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rust-atlas-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let code = root.join("code");
        let graph = root.join("graph");
        fs::create_dir_all(code.join("src")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"writer-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn value() -> u32 { 1 }\n").unwrap();
        (code, graph)
    }

    fn entries(path: &Path) -> Vec<String> {
        let mut entries = fs::read_dir(path)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<_>>();
        entries.sort();
        entries
    }

    #[test]
    fn test_atlas_writer_lock_allows_only_one_build() {
        let (code, graph) = fixture("writer-lock");
        build(&code, &graph, &BuildOptions::default()).unwrap();
        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/lib.rs", 100).unwrap();
        let pointer_before = fs::read(graph.join("CURRENT.json")).unwrap();
        let pending_before = fs::read(journal.path()).unwrap();
        let staging_before = entries(&graph.join(".staging"));
        let owner = WriterLease::try_acquire(&graph).unwrap();

        let error = build(&code, &graph, &BuildOptions::default()).unwrap_err();

        assert!(matches!(error, AtlasError::WriterBusy { .. }));
        assert_eq!(
            fs::read(graph.join("CURRENT.json")).unwrap(),
            pointer_before
        );
        assert_eq!(fs::read(journal.path()).unwrap(), pending_before);
        assert_eq!(entries(&graph.join(".staging")), staging_before);
        drop(owner);
        fs::remove_dir_all(graph.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_dead_writer_lock_is_recoverable() {
        let (_code, graph) = fixture("dead-writer");
        let owner = WriterLease::try_acquire(&graph).unwrap();
        let lock_path = graph.join(".runtime/build.lock");
        assert!(lock_path.is_file());
        drop(owner);

        let replacement = WriterLease::try_acquire(&graph).unwrap();
        assert!(lock_path.is_file());
        drop(replacement);
        fs::remove_dir_all(graph.parent().unwrap()).ok();
    }
}
