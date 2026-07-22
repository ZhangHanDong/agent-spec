use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::AtlasError;

pub struct WriterLease {
    file: File,
    graph_root: PathBuf,
}

pub struct DaemonLease {
    file: File,
}

pub(crate) struct ReaderRegistryLease {
    file: File,
}

#[derive(Clone)]
pub(crate) struct ReaderLease {
    inner: Arc<ReaderLeaseInner>,
}

struct ReaderLeaseInner {
    file: Option<File>,
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReaderLeaseRecord {
    schema_id: String,
    generation: String,
    pid: u32,
    nonce: String,
    created_at_ms: u64,
}

const READER_SCHEMA: &str = "rust-atlas/reader-lease-v1";
const MAX_READER_LEASE_BYTES: u64 = 1024 * 1024;

impl WriterLease {
    pub fn try_acquire(graph_root: &Path) -> Result<Self, AtlasError> {
        let (file, graph_root) = try_acquire_lock(graph_root, "build.lock", |graph_root| {
            AtlasError::WriterBusy { graph_root }
        })?;
        Ok(Self { file, graph_root })
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

impl DaemonLease {
    pub fn try_acquire(graph_root: &Path) -> Result<Self, AtlasError> {
        let (file, _) = try_acquire_lock(graph_root, "daemon.lock", |graph_root| {
            AtlasError::DaemonBusy { graph_root }
        })?;
        Ok(Self { file })
    }
}

impl ReaderRegistryLease {
    pub(crate) fn acquire_shared(graph_root: &Path) -> Result<Self, AtlasError> {
        let file = open_runtime_lock(graph_root, "readers.lock")?;
        FileExt::lock_shared(&file).map_err(lock_io)?;
        Ok(Self { file })
    }

    pub(crate) fn acquire_exclusive(graph_root: &Path) -> Result<Self, AtlasError> {
        let file = open_runtime_lock(graph_root, "readers.lock")?;
        FileExt::lock_exclusive(&file).map_err(lock_io)?;
        Ok(Self { file })
    }
}

impl ReaderLease {
    pub(crate) fn create(
        graph_root: &Path,
        generation: &str,
        _registry: &ReaderRegistryLease,
    ) -> Result<Self, AtlasError> {
        validate_generation(generation)?;
        let readers = readers_dir(graph_root)?;
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| AtlasError::Io(error.to_string()))?;
        let nonce = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = readers.join(format!("{}-{nonce}.json", std::process::id()));
        reject_symlink(&path)?;
        let record = ReaderLeaseRecord {
            schema_id: READER_SCHEMA.to_string(),
            generation: generation.to_string(),
            pid: std::process::id(),
            nonce,
            created_at_ms: now_ms()?,
        };
        validate_reader_record(&record)?;
        let mut bytes =
            serde_json::to_vec_pretty(&record).map_err(|error| AtlasError::LiveState {
                detail: error.to_string(),
            })?;
        bytes.push(b'\n');
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(lock_io)?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(lock_io(error));
        }
        if let Err(error) = FileExt::lock_shared(&file) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(lock_io(error));
        }
        Ok(Self {
            inner: Arc::new(ReaderLeaseInner {
                file: Some(file),
                path,
            }),
        })
    }
}

impl std::fmt::Debug for ReaderLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReaderLease")
            .field("path", &self.inner.path)
            .finish_non_exhaustive()
    }
}

impl Drop for WriterLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl Drop for DaemonLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl Drop for ReaderRegistryLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl Drop for ReaderLeaseInner {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = FileExt::unlock(&file);
            drop(file);
        }
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn inspect_reader_lease(path: &Path) -> Result<Option<String>, AtlasError> {
    reject_symlink(path)?;
    let mut file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(lock_io(error)),
    };
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            let _ = FileExt::unlock(&file);
            drop(file);
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(lock_io(error)),
            }
            Ok(None)
        }
        Err(error) if is_contention(&error) => {
            let metadata = file.metadata().map_err(lock_io)?;
            if metadata.len() > MAX_READER_LEASE_BYTES {
                return Err(AtlasError::LiveState {
                    detail: format!("active reader lease exceeds 1 MiB: {}", path.display()),
                });
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map_err(lock_io)?;
            let record: ReaderLeaseRecord =
                serde_json::from_slice(&bytes).map_err(|error| AtlasError::LiveState {
                    detail: format!("invalid active reader lease {}: {error}", path.display()),
                })?;
            validate_reader_record(&record)?;
            let expected_name = format!("{}-{}.json", record.pid, record.nonce);
            if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
                return Err(AtlasError::LiveState {
                    detail: format!(
                        "active reader lease identity does not match path {}",
                        path.display()
                    ),
                });
            }
            Ok(Some(record.generation))
        }
        Err(error) => Err(lock_io(error)),
    }
}

fn try_acquire_lock(
    graph_root: &Path,
    name: &str,
    busy: impl FnOnce(String) -> AtlasError,
) -> Result<(File, PathBuf), AtlasError> {
    fs::create_dir_all(graph_root).map_err(lock_io)?;
    let graph_root = fs::canonicalize(graph_root).map_err(lock_io)?;
    let runtime = graph_root.join(".runtime");
    reject_symlink(&runtime)?;
    fs::create_dir_all(&runtime).map_err(lock_io)?;
    let lock_path = runtime.join(name);
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
        Ok(()) => Ok((file, graph_root)),
        Err(error) if is_contention(&error) => Err(busy(graph_root.to_string_lossy().into_owned())),
        Err(error) => Err(lock_io(error)),
    }
}

fn open_runtime_lock(graph_root: &Path, name: &str) -> Result<File, AtlasError> {
    fs::create_dir_all(graph_root).map_err(lock_io)?;
    let graph_root = fs::canonicalize(graph_root).map_err(lock_io)?;
    let runtime = graph_root.join(".runtime");
    reject_symlink(&runtime)?;
    fs::create_dir_all(&runtime).map_err(lock_io)?;
    let path = runtime.join(name);
    reject_symlink(&path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(lock_io)?;
    if fs::canonicalize(&runtime).map_err(lock_io)? != runtime
        || fs::canonicalize(&path).map_err(lock_io)? != path
    {
        return Err(AtlasError::LiveState {
            detail: format!("runtime lock escaped the graph: {}", path.display()),
        });
    }
    Ok(file)
}

fn readers_dir(graph_root: &Path) -> Result<PathBuf, AtlasError> {
    let graph_root = fs::canonicalize(graph_root).map_err(lock_io)?;
    let readers = graph_root.join(".runtime/readers");
    reject_symlink(&readers)?;
    fs::create_dir_all(&readers).map_err(lock_io)?;
    if fs::canonicalize(&readers).map_err(lock_io)? != readers {
        return Err(AtlasError::LiveState {
            detail: format!("reader directory escaped the graph: {}", readers.display()),
        });
    }
    Ok(readers)
}

fn validate_reader_record(record: &ReaderLeaseRecord) -> Result<(), AtlasError> {
    if record.schema_id != READER_SCHEMA
        || record.pid == 0
        || record.created_at_ms == 0
        || record.nonce.len() != 32
        || !record.nonce.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AtlasError::LiveState {
            detail: "invalid reader lease identity".to_string(),
        });
    }
    validate_generation(&record.generation)
}

fn validate_generation(generation: &str) -> Result<(), AtlasError> {
    if generation.len() == 66
        && generation.starts_with("g-")
        && generation[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(AtlasError::LiveState {
            detail: format!("invalid reader generation `{generation}`"),
        })
    }
}

fn now_ms() -> Result<u64, AtlasError> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| AtlasError::Io(error.to_string()))?
        .as_millis();
    u64::try_from(millis).map_err(|_| AtlasError::Io("system time exceeds u64".to_string()))
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
