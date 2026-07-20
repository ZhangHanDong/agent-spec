use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::locking::{ReaderLease, ReaderRegistryLease, WriterLease, inspect_reader_lease};
use crate::{AtlasError, Capability};

const POINTER_SCHEMA_VERSION: u32 = 1;
const MANIFEST_SCHEMA_VERSION: u32 = 1;
const CURRENT_FILE: &str = "CURRENT.json";
const GENERATIONS_DIR: &str = "generations";
const STAGING_DIR: &str = ".staging";
const MANIFEST_FILE: &str = "generation.json";

static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct GraphSnapshot {
    pub data_dir: PathBuf,
    pub generation: Option<String>,
    lease: Option<ReaderLease>,
}

impl PartialEq for GraphSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.data_dir == other.data_dir && self.generation == other.generation
    }
}

impl Eq for GraphSnapshot {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ReclaimReport {
    pub(crate) removed_generations: Vec<String>,
    pub(crate) removed_staging: Vec<String>,
    pub(crate) diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GenerationManifest {
    pub(crate) schema_version: u32,
    pub(crate) generation: String,
    pub(crate) base_generation: Option<String>,
    pub(crate) graph_fingerprint: String,
    pub(crate) capability: Capability,
    pub(crate) input_plan_fingerprint: Option<String>,
    pub(crate) artifacts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerationPointer {
    schema_version: u32,
    generation: String,
}

#[derive(Serialize)]
struct GenerationIdentity<'a> {
    schema_version: u32,
    base_generation: &'a Option<String>,
    graph_fingerprint: &'a str,
    capability: &'a Capability,
    input_plan_fingerprint: &'a Option<String>,
    artifacts: &'a BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PublishFault {
    None,
    ManifestWrite,
    FinalRename,
    PointerWrite,
}

pub(crate) struct GenerationTransaction {
    graph_root: PathBuf,
    staging: PathBuf,
    base_generation: Option<String>,
    committed: bool,
}

impl GenerationTransaction {
    pub(crate) fn begin(
        graph_root: &Path,
        base: Option<&GraphSnapshot>,
    ) -> Result<Self, AtlasError> {
        fs::create_dir_all(graph_root).map_err(io_error)?;
        let staging_root = graph_root.join(STAGING_DIR);
        fs::create_dir_all(&staging_root).map_err(io_error)?;
        let nonce = NEXT_STAGING.fetch_add(1, Ordering::Relaxed);
        let staging = staging_root.join(format!("{}-{nonce}", std::process::id()));
        fs::create_dir(&staging).map_err(io_error)?;
        if let Some(base) = base
            && let Err(error) = clone_snapshot(&base.data_dir, &staging)
        {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        Ok(Self {
            graph_root: graph_root.to_path_buf(),
            staging,
            base_generation: base.and_then(|snapshot| snapshot.generation.clone()),
            committed: false,
        })
    }

    pub(crate) fn data_dir(&self) -> &Path {
        &self.staging
    }

    pub(crate) fn publish(
        self,
        graph_fingerprint: &str,
        capability: &Capability,
        input_plan_fingerprint: Option<&str>,
    ) -> Result<GraphSnapshot, AtlasError> {
        self.publish_with_fault(
            graph_fingerprint,
            capability,
            input_plan_fingerprint,
            PublishFault::None,
        )
    }

    pub(crate) fn publish_with_fault(
        mut self,
        graph_fingerprint: &str,
        capability: &Capability,
        input_plan_fingerprint: Option<&str>,
        fault: PublishFault,
    ) -> Result<GraphSnapshot, AtlasError> {
        if fault == PublishFault::ManifestWrite {
            return Err(injected_failure("manifest-write"));
        }

        let artifacts = artifact_hashes(&self.staging)?;
        let input_plan_fingerprint = input_plan_fingerprint.map(str::to_string);
        let identity = GenerationIdentity {
            schema_version: MANIFEST_SCHEMA_VERSION,
            base_generation: &self.base_generation,
            graph_fingerprint,
            capability,
            input_plan_fingerprint: &input_plan_fingerprint,
            artifacts: &artifacts,
        };
        let identity_bytes =
            serde_json::to_vec(&identity).map_err(|error| AtlasError::Io(error.to_string()))?;
        let generation = format!("g-{}", blake3::hash(&identity_bytes).to_hex());
        validate_generation_id(&generation)?;
        let manifest = GenerationManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            generation: generation.clone(),
            base_generation: self.base_generation.clone(),
            graph_fingerprint: graph_fingerprint.to_string(),
            capability: capability.clone(),
            input_plan_fingerprint,
            artifacts,
        };
        crate::index::write_json_atomic(&self.staging.join(MANIFEST_FILE), &manifest)?;
        sync_directory(&self.staging)?;

        if fault == PublishFault::FinalRename {
            return Err(injected_failure("final-rename"));
        }

        let generations = self.graph_root.join(GENERATIONS_DIR);
        fs::create_dir_all(&generations).map_err(io_error)?;
        let final_dir = generations.join(&generation);
        if final_dir.exists() {
            let existing = read_manifest(&final_dir)?;
            if existing != manifest {
                return Err(AtlasError::Invariant(format!(
                    "generation id collision for {generation}"
                )));
            }
            fs::remove_dir_all(&self.staging).map_err(io_error)?;
        } else {
            match fs::rename(&self.staging, &final_dir) {
                Ok(()) => {}
                Err(_) if final_dir.exists() => {
                    let existing = read_manifest(&final_dir)?;
                    if existing != manifest {
                        return Err(AtlasError::Invariant(format!(
                            "generation id collision for {generation}"
                        )));
                    }
                    fs::remove_dir_all(&self.staging).map_err(io_error)?;
                }
                Err(error) => return Err(io_error(error)),
            }
        }
        self.committed = true;
        sync_directory(&generations)?;

        if fault == PublishFault::PointerWrite {
            return Err(injected_failure("pointer-write"));
        }
        let pointer = GenerationPointer {
            schema_version: POINTER_SCHEMA_VERSION,
            generation: generation.clone(),
        };
        crate::index::write_json_atomic(&self.graph_root.join(CURRENT_FILE), &pointer)?;
        sync_directory(&self.graph_root)?;
        Ok(GraphSnapshot {
            data_dir: final_dir,
            generation: Some(generation),
            lease: None,
        })
    }
}

impl Drop for GenerationTransaction {
    fn drop(&mut self) {
        if !self.committed {
            let _ = cleanup_owned_staging(&self.staging);
        }
    }
}

fn cleanup_owned_staging(path: &Path) -> Result<(), AtlasError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

pub(crate) fn resolve_snapshot(graph_root: &Path) -> Result<GraphSnapshot, AtlasError> {
    let registry = ReaderRegistryLease::acquire_shared(graph_root)?;
    let mut snapshot =
        resolve_optional_snapshot(graph_root)?.ok_or_else(|| AtlasError::MissingGraph {
            graph_dir: graph_root.display().to_string(),
        })?;
    if let Some(generation) = snapshot.generation.as_deref() {
        snapshot.lease = Some(ReaderLease::create(graph_root, generation, &registry)?);
    }
    Ok(snapshot)
}

pub(crate) fn artifacts_match_manifest(snapshot: &GraphSnapshot) -> bool {
    let Some(expected_generation) = snapshot.generation.as_deref() else {
        return false;
    };
    let Ok(manifest) = read_manifest(&snapshot.data_dir) else {
        return false;
    };
    manifest.generation == expected_generation
        && artifact_hashes(&snapshot.data_dir).is_ok_and(|actual| actual == manifest.artifacts)
}

pub(crate) fn resolve_optional_snapshot(
    graph_root: &Path,
) -> Result<Option<GraphSnapshot>, AtlasError> {
    let pointer_path = graph_root.join(CURRENT_FILE);
    match fs::read_to_string(&pointer_path) {
        Ok(text) => {
            let pointer: GenerationPointer = serde_json::from_str(&text).map_err(|error| {
                AtlasError::Invariant(format!(
                    "invalid generation pointer {}: {error}",
                    pointer_path.display()
                ))
            })?;
            if pointer.schema_version != POINTER_SCHEMA_VERSION {
                return Err(AtlasError::Invariant(format!(
                    "generation pointer schema v{} != v{}",
                    pointer.schema_version, POINTER_SCHEMA_VERSION
                )));
            }
            validate_generation_id(&pointer.generation)?;
            let data_dir = graph_root.join(GENERATIONS_DIR).join(&pointer.generation);
            reject_symlink(&data_dir)?;
            let manifest = read_manifest(&data_dir)?;
            if manifest.generation != pointer.generation {
                return Err(AtlasError::Invariant(format!(
                    "generation manifest {} does not match pointer {}",
                    manifest.generation, pointer.generation
                )));
            }
            Ok(Some(GraphSnapshot {
                data_dir,
                generation: Some(pointer.generation),
                lease: None,
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if graph_root.join("meta.json").is_file() {
                Ok(Some(GraphSnapshot {
                    data_dir: graph_root.to_path_buf(),
                    generation: None,
                    lease: None,
                }))
            } else {
                Ok(None)
            }
        }
        Err(error) => Err(io_error(error)),
    }
}

pub(crate) fn safe_reclaim(
    graph_root: &Path,
    writer: &WriterLease,
) -> Result<ReclaimReport, AtlasError> {
    writer.assert_graph(graph_root)?;
    let graph_root = fs::canonicalize(graph_root).map_err(io_error)?;
    let registry = ReaderRegistryLease::acquire_exclusive(&graph_root)?;
    let current = resolve_optional_snapshot(&graph_root)?.and_then(|snapshot| snapshot.generation);
    let mut report = ReclaimReport::default();
    let mut active_generations = BTreeSet::new();
    let readers = graph_root.join(".runtime/readers");
    let reader_scan = sorted_entries(&readers, "reader lease", &mut report.diagnostics)?;
    let mut ambiguous_reader = !reader_scan.complete;
    for entry in reader_scan.entries {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                ambiguous_reader = true;
                report.diagnostics.push(format!(
                    "cannot inspect reader lease {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        if !file_type.is_file() || file_type.is_symlink() {
            ambiguous_reader = true;
            report.diagnostics.push(format!(
                "reader lease path is not a regular file: {}",
                path.display()
            ));
            continue;
        }
        match inspect_reader_lease(&path) {
            Ok(Some(generation)) => {
                active_generations.insert(generation);
            }
            Ok(None) => {}
            Err(error) => {
                ambiguous_reader = true;
                report.diagnostics.push(format!(
                    "cannot prove reader lease inactive at {}: {error}",
                    path.display()
                ));
            }
        }
    }
    drop(registry);

    let generations = graph_root.join(GENERATIONS_DIR);
    for entry in sorted_entries(&generations, "generation", &mut report.diagnostics)?.entries {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            report.diagnostics.push(format!(
                "generation name is not valid UTF-8: {}",
                path.display()
            ));
            continue;
        };
        if let Err(error) = validate_generation_id(&name) {
            report.diagnostics.push(format!(
                "generation entry is not reclaimable at {}: {error}",
                path.display()
            ));
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                report.diagnostics.push(format!(
                    "cannot inspect generation {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            report.diagnostics.push(format!(
                "generation path is not a real directory: {}",
                path.display()
            ));
            continue;
        }
        if current.as_deref() == Some(name.as_str())
            || ambiguous_reader
            || active_generations.contains(&name)
        {
            continue;
        }
        match fs::remove_dir_all(&path) {
            Ok(()) => report.removed_generations.push(name),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => report.diagnostics.push(format!(
                "cannot reclaim generation {}: {error}",
                path.display()
            )),
        }
    }

    let staging = graph_root.join(STAGING_DIR);
    for entry in sorted_entries(&staging, "staging", &mut report.diagnostics)?.entries {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            report.diagnostics.push(format!(
                "staging name is not valid UTF-8: {}",
                path.display()
            ));
            continue;
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                report.diagnostics.push(format!(
                    "cannot inspect staging path {}: {error}",
                    path.display()
                ));
                continue;
            }
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            report.diagnostics.push(format!(
                "staging path is not a real directory: {}",
                path.display()
            ));
            continue;
        }
        match fs::remove_dir_all(&path) {
            Ok(()) => report.removed_staging.push(name),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => report.diagnostics.push(format!(
                "cannot reclaim staging path {}: {error}",
                path.display()
            )),
        }
    }

    Ok(report)
}

struct DirectoryScan {
    entries: Vec<fs::DirEntry>,
    complete: bool,
}

fn sorted_entries(
    directory: &Path,
    label: &str,
    diagnostics: &mut Vec<String>,
) -> Result<DirectoryScan, AtlasError> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            diagnostics.push(format!(
                "{label} directory is not a real directory: {}",
                directory.display()
            ));
            return Ok(DirectoryScan {
                entries: Vec::new(),
                complete: false,
            });
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DirectoryScan {
                entries: Vec::new(),
                complete: true,
            });
        }
        Err(error) => {
            diagnostics.push(format!(
                "cannot inspect {label} directory {}: {error}",
                directory.display()
            ));
            return Ok(DirectoryScan {
                entries: Vec::new(),
                complete: false,
            });
        }
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(format!(
                "cannot scan {label} directory {}: {error}",
                directory.display()
            ));
            return Ok(DirectoryScan {
                entries: Vec::new(),
                complete: false,
            });
        }
    };
    let mut entries = entries.collect::<Result<Vec<_>, _>>().map_err(io_error)?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(DirectoryScan {
        entries,
        complete: true,
    })
}

fn validate_generation_id(value: &str) -> Result<(), AtlasError> {
    let valid = value.len() == 66
        && value.starts_with("g-")
        && value[2..].bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(AtlasError::Invariant(format!(
            "unsafe generation id `{value}`"
        )))
    }
}

fn read_manifest(data_dir: &Path) -> Result<GenerationManifest, AtlasError> {
    let path = data_dir.join(MANIFEST_FILE);
    let text = fs::read_to_string(&path).map_err(|error| {
        AtlasError::Invariant(format!(
            "cannot read generation manifest {}: {error}",
            path.display()
        ))
    })?;
    let manifest: GenerationManifest = serde_json::from_str(&text).map_err(|error| {
        AtlasError::Invariant(format!(
            "invalid generation manifest {}: {error}",
            path.display()
        ))
    })?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(AtlasError::Invariant(format!(
            "generation manifest schema v{} != v{}",
            manifest.schema_version, MANIFEST_SCHEMA_VERSION
        )));
    }
    validate_generation_id(&manifest.generation)?;
    Ok(manifest)
}

fn reject_symlink(path: &Path) -> Result<(), AtlasError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        AtlasError::Invariant(format!(
            "cannot access generation directory {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AtlasError::Invariant(format!(
            "generation path {} is not a real directory",
            path.display()
        )));
    }
    Ok(())
}

fn clone_snapshot(source: &Path, destination: &Path) -> Result<(), AtlasError> {
    clone_directory(source, destination, true)
}

fn clone_directory(source: &Path, destination: &Path, top_level: bool) -> Result<(), AtlasError> {
    let mut entries = fs::read_dir(source)
        .map_err(io_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_error)?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if top_level
            && matches!(
                name_text.as_ref(),
                CURRENT_FILE
                    | GENERATIONS_DIR
                    | STAGING_DIR
                    | MANIFEST_FILE
                    | "orphans.json"
                    | ".runtime"
            )
        {
            continue;
        }
        let file_type = entry.file_type().map_err(io_error)?;
        let target = destination.join(&name);
        if file_type.is_symlink() {
            return Err(AtlasError::Invariant(format!(
                "generation source contains symlink {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            fs::create_dir(&target).map_err(io_error)?;
            clone_directory(&entry.path(), &target, false)?;
        } else if file_type.is_file() {
            if fs::hard_link(entry.path(), &target).is_err() {
                fs::copy(entry.path(), &target).map_err(io_error)?;
            }
        } else {
            return Err(AtlasError::Invariant(format!(
                "generation source contains unsupported entry {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn artifact_hashes(data_dir: &Path) -> Result<BTreeMap<String, String>, AtlasError> {
    let mut artifacts = BTreeMap::new();
    collect_artifacts(data_dir, data_dir, &mut artifacts)?;
    Ok(artifacts)
}

fn collect_artifacts(
    root: &Path,
    directory: &Path,
    artifacts: &mut BTreeMap<String, String>,
) -> Result<(), AtlasError> {
    let mut entries = fs::read_dir(directory)
        .map_err(io_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_error)?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let file_type = entry.file_type().map_err(io_error)?;
        if file_type.is_symlink() {
            return Err(AtlasError::Invariant(format!(
                "staged generation contains symlink {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            collect_artifacts(root, &entry.path(), artifacts)?;
        } else if file_type.is_file() && entry.file_name() != MANIFEST_FILE {
            let bytes = fs::read(entry.path()).map_err(io_error)?;
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|error| AtlasError::Invariant(error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            artifacts.insert(relative, blake3::hash(&bytes).to_hex().to_string());
        }
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), AtlasError> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(io_error)
}

fn io_error(error: std::io::Error) -> AtlasError {
    AtlasError::Io(error.to_string())
}

fn injected_failure(stage: &str) -> AtlasError {
    AtlasError::Io(format!("injected generation {stage} failure"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::locking::WriterLease;

    fn temp_dir(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rust-atlas-generation-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn publish_marker(
        graph: &Path,
        marker: &str,
        fault: PublishFault,
    ) -> Result<GraphSnapshot, AtlasError> {
        let base = resolve_optional_snapshot(graph)?;
        let transaction = GenerationTransaction::begin(graph, base.as_ref())?;
        crate::index::write_json_atomic(&transaction.data_dir().join("marker.json"), &marker)?;
        let fingerprint = blake3::hash(marker.as_bytes()).to_hex();
        transaction.publish_with_fault(fingerprint.as_ref(), &Capability::default(), None, fault)
    }

    fn read_marker(snapshot: &GraphSnapshot) -> String {
        serde_json::from_slice(&fs::read(snapshot.data_dir.join("marker.json")).unwrap()).unwrap()
    }

    #[test]
    fn test_atlas_reader_pins_one_generation_during_pointer_change() {
        let graph = temp_dir("pin");
        let first = publish_marker(&graph, "first", PublishFault::None).unwrap();
        let pinned = resolve_snapshot(&graph).unwrap();
        assert_eq!(pinned, first);

        let second = publish_marker(&graph, "second", PublishFault::None).unwrap();

        assert_ne!(first.generation, second.generation);
        assert_eq!(read_marker(&pinned), "first");
        assert_eq!(read_marker(&resolve_snapshot(&graph).unwrap()), "second");
    }

    #[test]
    fn test_atlas_generation_failures_keep_committed_baseline() {
        for fault in [
            PublishFault::ManifestWrite,
            PublishFault::FinalRename,
            PublishFault::PointerWrite,
        ] {
            let graph = temp_dir("failure");
            let baseline = publish_marker(&graph, "baseline", PublishFault::None).unwrap();
            let pointer_before = fs::read(graph.join("CURRENT.json")).unwrap();

            assert!(publish_marker(&graph, "changed", fault).is_err());

            assert_eq!(
                fs::read(graph.join("CURRENT.json")).unwrap(),
                pointer_before
            );
            assert_eq!(resolve_snapshot(&graph).unwrap(), baseline);
            assert_eq!(read_marker(&baseline), "baseline");
        }
    }

    #[test]
    fn test_atlas_legacy_generation_migrates_only_after_success() {
        let graph = temp_dir("legacy");
        fs::write(graph.join("meta.json"), "legacy").unwrap();
        fs::create_dir_all(graph.join(".runtime")).unwrap();
        fs::write(graph.join(".runtime/legacy.lock"), "runtime").unwrap();
        let legacy = resolve_snapshot(&graph).unwrap();
        assert_eq!(legacy.data_dir, graph);
        assert_eq!(legacy.generation, None);

        assert!(publish_marker(&graph, "failed", PublishFault::PointerWrite).is_err());
        assert!(!graph.join("CURRENT.json").exists());
        assert_eq!(resolve_snapshot(&graph).unwrap().generation, None);

        let committed = publish_marker(&graph, "committed", PublishFault::None).unwrap();
        assert!(committed.generation.is_some());
        assert!(!committed.data_dir.join(".runtime").exists());
        assert_eq!(resolve_snapshot(&graph).unwrap(), committed);
        assert_eq!(
            fs::read_to_string(graph.join("meta.json")).unwrap(),
            "legacy"
        );
    }

    #[test]
    fn test_atlas_owned_staging_cleanup_is_idempotent_and_preserves_active_generation() {
        let graph = temp_dir("owned-cleanup");
        let baseline = publish_marker(&graph, "baseline", PublishFault::None).unwrap();
        let transaction = GenerationTransaction::begin(&graph, Some(&baseline)).unwrap();
        let staging = transaction.staging.clone();
        assert!(staging.is_dir());
        drop(transaction);

        assert!(!staging.exists());
        cleanup_owned_staging(&staging).unwrap();
        cleanup_owned_staging(&staging).unwrap();
        assert_eq!(resolve_snapshot(&graph).unwrap(), baseline);
        assert_eq!(read_marker(&baseline), "baseline");
    }

    #[test]
    fn test_atlas_reader_lease_preserves_old_generation_until_drop() {
        let graph = temp_dir("reader-lease");
        let first = publish_marker(&graph, "first", PublishFault::None).unwrap();
        let pinned = resolve_snapshot(&graph).unwrap();
        let pinned_clone = pinned.clone();
        let writer = WriterLease::try_acquire(&graph).unwrap();
        let second = publish_marker(&graph, "second", PublishFault::None).unwrap();

        let retained = safe_reclaim(&graph, &writer).unwrap();
        assert!(retained.removed_generations.is_empty());
        assert!(first.data_dir.is_dir());
        assert!(second.data_dir.is_dir());

        drop(pinned);
        let still_retained = safe_reclaim(&graph, &writer).unwrap();
        assert!(still_retained.removed_generations.is_empty());
        assert!(first.data_dir.is_dir());

        drop(pinned_clone);
        let reclaimed = safe_reclaim(&graph, &writer).unwrap();
        assert_eq!(
            reclaimed.removed_generations,
            vec![first.generation.unwrap()]
        );
        assert!(!first.data_dir.exists());
        assert!(second.data_dir.is_dir());
        fs::remove_dir_all(graph).ok();
    }

    #[test]
    fn test_atlas_reclamation_retains_generations_for_ambiguous_reader_directory() {
        let graph = temp_dir("ambiguous-reader-directory");
        let first = publish_marker(&graph, "first", PublishFault::None).unwrap();
        let second = publish_marker(&graph, "second", PublishFault::None).unwrap();
        let readers = graph.join(".runtime/readers");
        fs::create_dir_all(readers.parent().unwrap()).unwrap();
        fs::write(&readers, "not-a-directory").unwrap();
        let writer = WriterLease::try_acquire(&graph).unwrap();

        let report = safe_reclaim(&graph, &writer).unwrap();

        assert!(report.removed_generations.is_empty());
        assert!(!report.diagnostics.is_empty());
        assert!(first.data_dir.is_dir());
        assert!(second.data_dir.is_dir());
        fs::remove_dir_all(graph).ok();
    }

    #[test]
    fn test_atlas_reclamation_retains_generations_for_ambiguous_lease() {
        let graph = temp_dir("ambiguous-lease");
        let first = publish_marker(&graph, "first", PublishFault::None).unwrap();
        let second = publish_marker(&graph, "second", PublishFault::None).unwrap();
        let readers = graph.join(".runtime/readers");
        fs::create_dir_all(&readers).unwrap();
        let lease_path = readers.join("locked-malformed.json");
        let mut lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lease_path)
            .unwrap();
        lease.write_all(b"not-json\n").unwrap();
        lease.lock_shared().unwrap();
        let writer = WriterLease::try_acquire(&graph).unwrap();

        let report = safe_reclaim(&graph, &writer).unwrap();

        assert!(report.removed_generations.is_empty());
        assert!(!report.diagnostics.is_empty());
        assert!(first.data_dir.is_dir());
        assert!(second.data_dir.is_dir());
        lease.unlock().unwrap();
        fs::remove_dir_all(graph).ok();
    }

    #[test]
    fn test_atlas_safe_reclamation_cleans_abandoned_staging_idempotently() {
        let graph = temp_dir("abandoned-staging");
        let current = publish_marker(&graph, "current", PublishFault::None).unwrap();
        let abandoned = graph.join(".staging/abandoned");
        fs::create_dir_all(&abandoned).unwrap();
        fs::write(abandoned.join("partial.json"), b"partial").unwrap();
        let current_bytes = fs::read(current.data_dir.join("marker.json")).unwrap();
        let writer = WriterLease::try_acquire(&graph).unwrap();

        let first = safe_reclaim(&graph, &writer).unwrap();
        let second = safe_reclaim(&graph, &writer).unwrap();

        assert_eq!(first.removed_staging, vec!["abandoned"]);
        assert!(second.removed_staging.is_empty());
        assert!(!abandoned.exists());
        assert_eq!(
            fs::read(current.data_dir.join("marker.json")).unwrap(),
            current_bytes
        );
        fs::remove_dir_all(graph).ok();
    }
}
