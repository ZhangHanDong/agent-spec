use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{AtlasError, index::write_json_atomic};

const PENDING_SCHEMA: &str = "rust-atlas/pending-events-v1";
const LIVE_STATUS_SCHEMA: &str = "rust-atlas/live-status-v1";
const PENDING_FILE: &str = "pending.json";
const LIVE_STATUS_FILE: &str = "status.json";
const MAX_LIVE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PENDING_EVENTS: usize = 100_000;
const MAX_VALUE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingEvent {
    pub path: String,
    pub first_sequence: u64,
    pub latest_sequence: u64,
    pub first_seen_ms: u64,
    pub latest_seen_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSnapshot {
    pub watermark: u64,
    pub events: Vec<PendingEvent>,
}

#[derive(Debug, Clone)]
pub struct PendingJournal {
    runtime_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingFile {
    schema: String,
    next_sequence: u64,
    events: BTreeMap<String, PendingEvent>,
}

impl Default for PendingFile {
    fn default() -> Self {
        Self {
            schema: PENDING_SCHEMA.to_string(),
            next_sequence: 1,
            events: BTreeMap::new(),
        }
    }
}

impl PendingJournal {
    pub fn open(graph_root: &Path) -> Result<Self, AtlasError> {
        let runtime_dir = graph_root.join(".runtime");
        reject_symlink(&runtime_dir)?;
        std::fs::create_dir_all(&runtime_dir).map_err(live_io)?;
        Ok(Self { runtime_dir })
    }

    pub(crate) fn for_read(graph_root: &Path) -> Result<Self, AtlasError> {
        let runtime_dir = graph_root.join(".runtime");
        reject_symlink(&runtime_dir)?;
        Ok(Self { runtime_dir })
    }

    pub fn path(&self) -> PathBuf {
        self.runtime_dir.join(PENDING_FILE)
    }

    pub fn record(&self, path: &str, seen_ms: u64) -> Result<PendingEvent, AtlasError> {
        validate_relative_path(path)?;
        let mut pending = self.load()?;
        if !pending.events.contains_key(path) && pending.events.len() == MAX_PENDING_EVENTS {
            return Err(live_error(
                "pending journal exceeds the bounded event limit",
            ));
        }
        let sequence = pending.next_sequence;
        pending.next_sequence = pending
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| live_error("pending event sequence exhausted"))?;
        let event = pending
            .events
            .entry(path.to_string())
            .and_modify(|event| {
                event.latest_sequence = sequence;
                event.latest_seen_ms = seen_ms;
            })
            .or_insert_with(|| PendingEvent {
                path: path.to_string(),
                first_sequence: sequence,
                latest_sequence: sequence,
                first_seen_ms: seen_ms,
                latest_seen_ms: seen_ms,
            })
            .clone();
        pending.validate()?;
        self.write(&pending)?;
        Ok(event)
    }

    pub fn snapshot(&self) -> Result<PendingSnapshot, AtlasError> {
        let pending = self.load()?;
        Ok(PendingSnapshot {
            watermark: pending.next_sequence.saturating_sub(1),
            events: pending.events.into_values().collect(),
        })
    }

    pub fn acknowledge(
        &self,
        snapshot: &PendingSnapshot,
        unparsed: &BTreeSet<String>,
    ) -> Result<PendingSnapshot, AtlasError> {
        validate_snapshot(snapshot)?;
        for path in unparsed {
            validate_relative_path(path)?;
        }
        let snapshot_sequences = snapshot
            .events
            .iter()
            .map(|event| (event.path.as_str(), event.latest_sequence))
            .collect::<BTreeMap<_, _>>();
        let mut pending = self.load()?;
        let before = pending.events.len();
        pending.events.retain(|path, event| {
            unparsed.contains(path)
                || event.latest_sequence > snapshot.watermark
                || snapshot_sequences.get(path.as_str()).copied() != Some(event.latest_sequence)
        });
        if pending.events.len() != before {
            pending.validate()?;
            self.write(&pending)?;
        }
        Ok(PendingSnapshot {
            watermark: pending.next_sequence.saturating_sub(1),
            events: pending.events.into_values().collect(),
        })
    }

    fn load(&self) -> Result<PendingFile, AtlasError> {
        let path = self.path();
        let bytes = match bounded_read(&path)? {
            Some(bytes) => bytes,
            None => return Ok(PendingFile::default()),
        };
        let pending: PendingFile = serde_json::from_slice(&bytes)
            .map_err(|error| live_error(format!("invalid pending journal: {error}")))?;
        pending.validate()?;
        Ok(pending)
    }

    fn write(&self, pending: &PendingFile) -> Result<(), AtlasError> {
        pending.validate()?;
        ensure_serialized_size(pending, "pending journal")?;
        write_json_atomic(&self.path(), pending)
    }
}

impl PendingFile {
    fn validate(&self) -> Result<(), AtlasError> {
        if self.schema != PENDING_SCHEMA {
            return Err(live_error(format!(
                "pending schema `{}` != `{PENDING_SCHEMA}`",
                self.schema
            )));
        }
        if self.next_sequence == 0 {
            return Err(live_error("pending next_sequence must be positive"));
        }
        if self.events.len() > MAX_PENDING_EVENTS {
            return Err(live_error(
                "pending journal exceeds the bounded event limit",
            ));
        }
        for (path, event) in &self.events {
            validate_relative_path(path)?;
            if path != &event.path {
                return Err(live_error("pending event key does not match its path"));
            }
            validate_event(event, self.next_sequence.saturating_sub(1))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    WriterLock,
    Ordinary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 100,
            max_delay_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryState {
    pub writer_lock_attempts: u32,
    pub ordinary_attempts: u32,
    pub last_error: Option<String>,
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryOutcome {
    pub class: RetryClass,
    pub attempt: u32,
    pub delay_ms: Option<u64>,
    pub degraded: bool,
}

impl RetryState {
    pub fn record_failure(
        &mut self,
        class: RetryClass,
        error: impl Into<String>,
        policy: &RetryPolicy,
    ) -> Result<RetryOutcome, AtlasError> {
        policy.validate()?;
        let error = error.into();
        validate_text(&error, "retry error")?;
        let attempts = match class {
            RetryClass::WriterLock => &mut self.writer_lock_attempts,
            RetryClass::Ordinary => &mut self.ordinary_attempts,
        };
        *attempts = attempts
            .checked_add(1)
            .ok_or_else(|| live_error("retry attempt counter exhausted"))?;
        let attempt = *attempts;
        self.last_error = Some(error);
        if attempt > policy.max_attempts {
            self.degraded_reason = Some(format!(
                "{} retry budget exhausted after {attempt} failures",
                class.label()
            ));
            return Ok(RetryOutcome {
                class,
                attempt,
                delay_ms: None,
                degraded: true,
            });
        }
        let exponent = attempt.saturating_sub(1).min(63);
        let factor = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
        let delay_ms = policy
            .base_delay_ms
            .saturating_mul(factor)
            .min(policy.max_delay_ms);
        Ok(RetryOutcome {
            class,
            attempt,
            delay_ms: Some(delay_ms),
            degraded: false,
        })
    }

    pub fn reset_after_success(&mut self) {
        *self = Self::default();
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded_reason.is_some()
    }
}

impl RetryPolicy {
    fn validate(&self) -> Result<(), AtlasError> {
        if self.max_attempts == 0 {
            return Err(live_error("retry max_attempts must be positive"));
        }
        if self.base_delay_ms == 0 || self.max_delay_ms < self.base_delay_ms {
            return Err(live_error(
                "retry delays require 0 < base_delay_ms <= max_delay_ms",
            ));
        }
        Ok(())
    }
}

impl RetryClass {
    fn label(self) -> &'static str {
        match self {
            Self::WriterLock => "writer-lock",
            Self::Ordinary => "ordinary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LiveRuntimeState {
    Warming,
    Healthy,
    Pending,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveRuntimeStatus {
    schema: String,
    pub state: LiveRuntimeState,
    pub pending_paths: Vec<String>,
    pub retry: RetryState,
    pub generation: Option<String>,
    pub diagnostics: Vec<String>,
}

impl LiveRuntimeStatus {
    pub fn new(state: LiveRuntimeState) -> Self {
        Self {
            schema: LIVE_STATUS_SCHEMA.to_string(),
            state,
            pending_paths: Vec::new(),
            retry: RetryState::default(),
            generation: None,
            diagnostics: Vec::new(),
        }
    }

    pub fn load(graph_root: &Path) -> Result<Self, AtlasError> {
        let journal = PendingJournal::for_read(graph_root)?;
        let pending = journal.snapshot()?;
        let path = journal.runtime_dir.join(LIVE_STATUS_FILE);
        let mut status = match bounded_read(&path)? {
            Some(bytes) => serde_json::from_slice::<Self>(&bytes)
                .map_err(|error| live_error(format!("invalid live status: {error}")))?,
            None => Self::new(LiveRuntimeState::Unavailable),
        };
        status.validate()?;
        status.pending_paths = pending.events.into_iter().map(|event| event.path).collect();
        if status.retry.is_degraded() {
            status.state = LiveRuntimeState::Degraded;
        } else if !status.pending_paths.is_empty()
            && !matches!(
                status.state,
                LiveRuntimeState::Warming | LiveRuntimeState::Unavailable
            )
        {
            status.state = LiveRuntimeState::Pending;
        }
        status.validate()?;
        Ok(status)
    }

    pub fn store(&self, graph_root: &Path) -> Result<(), AtlasError> {
        self.validate()?;
        ensure_serialized_size(self, "live status")?;
        let journal = PendingJournal::open(graph_root)?;
        write_json_atomic(&journal.runtime_dir.join(LIVE_STATUS_FILE), self)
    }

    fn validate(&self) -> Result<(), AtlasError> {
        if self.schema != LIVE_STATUS_SCHEMA {
            return Err(live_error(format!(
                "live status schema `{}` != `{LIVE_STATUS_SCHEMA}`",
                self.schema
            )));
        }
        if self.pending_paths.len() > MAX_PENDING_EVENTS
            || self.diagnostics.len() > MAX_PENDING_EVENTS
        {
            return Err(live_error("live status exceeds the bounded item limit"));
        }
        for path in &self.pending_paths {
            validate_relative_path(path)?;
        }
        for diagnostic in &self.diagnostics {
            validate_text(diagnostic, "live diagnostic")?;
        }
        if let Some(generation) = &self.generation {
            validate_text(generation, "live generation")?;
        }
        if let Some(error) = &self.retry.last_error {
            validate_text(error, "retry error")?;
        }
        if let Some(reason) = &self.retry.degraded_reason {
            validate_text(reason, "degraded reason")?;
        }
        Ok(())
    }
}

fn bounded_read(path: &Path) -> Result<Option<Vec<u8>>, AtlasError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(live_io(error)),
    };
    if metadata.file_type().is_symlink() {
        return Err(live_error(format!(
            "live state path must not be a symlink: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_LIVE_FILE_BYTES {
        return Err(live_error(format!(
            "{} exceeds the {} byte limit",
            path.display(),
            MAX_LIVE_FILE_BYTES
        )));
    }
    std::fs::read(path).map(Some).map_err(live_io)
}

fn validate_snapshot(snapshot: &PendingSnapshot) -> Result<(), AtlasError> {
    if snapshot.events.len() > MAX_PENDING_EVENTS {
        return Err(live_error(
            "pending snapshot exceeds the bounded event limit",
        ));
    }
    let mut paths = BTreeSet::new();
    for event in &snapshot.events {
        validate_relative_path(&event.path)?;
        validate_event(event, snapshot.watermark)?;
        if !paths.insert(event.path.as_str()) {
            return Err(live_error("pending snapshot contains duplicate paths"));
        }
    }
    Ok(())
}

fn validate_event(event: &PendingEvent, watermark: u64) -> Result<(), AtlasError> {
    validate_relative_path(&event.path)?;
    if event.first_sequence == 0
        || event.first_sequence > event.latest_sequence
        || event.latest_sequence > watermark
        || event.first_seen_ms > event.latest_seen_ms
    {
        return Err(live_error("pending event sequence or timestamp is invalid"));
    }
    Ok(())
}

fn validate_relative_path(value: &str) -> Result<(), AtlasError> {
    validate_text(value, "pending path")?;
    let path = Path::new(value);
    if value.contains('\\')
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
                    | Component::CurDir
            )
        })
    {
        return Err(live_error(format!("unsafe pending path `{value}`")));
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if normalized != value {
        return Err(live_error(format!("non-normalized pending path `{value}`")));
    }
    Ok(())
}

fn ensure_serialized_size<T: Serialize>(value: &T, label: &str) -> Result<(), AtlasError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| live_error(format!("cannot serialize {label}: {error}")))?;
    if bytes.len() as u64 > MAX_LIVE_FILE_BYTES {
        return Err(live_error(format!(
            "{label} exceeds the {MAX_LIVE_FILE_BYTES} byte limit"
        )));
    }
    Ok(())
}

fn validate_text(value: &str, label: &str) -> Result<(), AtlasError> {
    if value.is_empty() || value.len() > MAX_VALUE_BYTES || value.chars().any(char::is_control) {
        return Err(live_error(format!("unsafe {label}")));
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), AtlasError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(live_error(format!(
            "runtime path must not be a symlink: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(live_io(error)),
    }
}

fn live_error(detail: impl Into<String>) -> AtlasError {
    AtlasError::LiveState {
        detail: detail.into(),
    }
}

fn live_io(error: std::io::Error) -> AtlasError {
    live_error(error.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn graph_dir(name: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let graph = std::env::temp_dir().join(format!(
            "rust-atlas-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&graph).unwrap();
        graph
    }

    #[test]
    fn test_atlas_pending_watermark_preserves_mid_sync_events() {
        let graph = graph_dir("pending-watermark");
        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/a.rs", 100).unwrap();
        journal.record("src/committed.rs", 101).unwrap();
        journal.record("src/unparsed.rs", 102).unwrap();
        let snapshot = journal.snapshot().unwrap();

        journal.record("src/a.rs", 200).unwrap();
        journal.record("Cargo.toml", 201).unwrap();
        let unparsed = BTreeSet::from(["src/unparsed.rs".to_string()]);
        let remaining = journal.acknowledge(&snapshot, &unparsed).unwrap();

        assert_eq!(snapshot.watermark, 3);
        assert_eq!(
            remaining
                .events
                .iter()
                .map(|event| (
                    event.path.as_str(),
                    event.first_sequence,
                    event.latest_sequence
                ))
                .collect::<Vec<_>>(),
            vec![
                ("Cargo.toml", 5, 5),
                ("src/a.rs", 1, 4),
                ("src/unparsed.rs", 3, 3),
            ]
        );
        fs::remove_dir_all(graph).ok();
    }

    #[test]
    fn test_atlas_failed_sync_preserves_pending_events() {
        let graph = graph_dir("failed-sync-pending");
        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/lib.rs", 100).unwrap();
        journal.record("Cargo.toml", 101).unwrap();
        let before = fs::read(journal.path()).unwrap();
        let policy = RetryPolicy::default();
        let mut retry = RetryState::default();

        for (class, error) in [
            (RetryClass::WriterLock, "writer busy"),
            (RetryClass::Ordinary, "cancelled"),
            (RetryClass::Ordinary, "extractor failed"),
            (RetryClass::Ordinary, "I/O failed"),
        ] {
            retry.record_failure(class, error, &policy).unwrap();
            assert_eq!(fs::read(journal.path()).unwrap(), before);
        }
        assert_eq!(journal.snapshot().unwrap().events.len(), 2);
        fs::remove_dir_all(graph).ok();
    }

    #[test]
    fn test_atlas_retry_budget_degrades_without_dropping_pending() {
        let graph = graph_dir("retry-degraded");
        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/lib.rs", 100).unwrap();
        let before = fs::read(journal.path()).unwrap();
        let policy = RetryPolicy::default();
        let mut retry = RetryState::default();

        let delays = (0..5)
            .map(|_| {
                retry
                    .record_failure(RetryClass::WriterLock, "writer busy", &policy)
                    .unwrap()
                    .delay_ms
            })
            .collect::<Vec<_>>();
        assert_eq!(
            delays,
            vec![Some(100), Some(200), Some(400), Some(800), Some(1600)]
        );
        assert_eq!(retry.ordinary_attempts, 0);
        let degraded = retry
            .record_failure(RetryClass::WriterLock, "writer busy", &policy)
            .unwrap();
        assert!(degraded.degraded);
        assert_eq!(degraded.delay_ms, None);
        assert_eq!(retry.writer_lock_attempts, 6);
        assert_eq!(fs::read(journal.path()).unwrap(), before);
        fs::remove_dir_all(graph).ok();
    }

    #[test]
    fn test_atlas_retry_state_resets_after_successful_sync() {
        let graph = graph_dir("retry-reset");
        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/lib.rs", 100).unwrap();
        let snapshot = journal.snapshot().unwrap();
        let mut retry = RetryState::default();
        let policy = RetryPolicy::default();
        retry
            .record_failure(RetryClass::WriterLock, "writer busy", &policy)
            .unwrap();
        retry
            .record_failure(RetryClass::Ordinary, "extractor failed", &policy)
            .unwrap();
        let mut status = LiveRuntimeStatus::new(LiveRuntimeState::Pending);
        status.pending_paths = vec!["src/lib.rs".to_string()];
        status.retry = retry.clone();
        status.store(&graph).unwrap();
        assert_eq!(LiveRuntimeStatus::load(&graph).unwrap().retry, retry);

        retry.reset_after_success();
        let remaining = journal.acknowledge(&snapshot, &BTreeSet::new()).unwrap();
        status.state = LiveRuntimeState::Healthy;
        status.pending_paths.clear();
        status.retry = retry.clone();
        status.store(&graph).unwrap();

        assert_eq!(retry, RetryState::default());
        assert!(remaining.events.is_empty());
        assert_eq!(
            LiveRuntimeStatus::load(&graph).unwrap(),
            LiveRuntimeStatus::new(LiveRuntimeState::Healthy)
        );
        fs::remove_dir_all(graph).ok();
    }

    #[test]
    fn test_atlas_live_state_rejects_corrupt_schema_paths_and_size() {
        let graph = graph_dir("live-state-corrupt");
        let journal = PendingJournal::open(&graph).unwrap();
        fs::write(
            journal.path(),
            r#"{"schema":"rust-atlas/pending-events-v1","next_sequence":2,"events":{"../escape.rs":{"path":"../escape.rs","first_sequence":1,"latest_sequence":1,"first_seen_ms":1,"latest_seen_ms":1}},"unknown":true}"#,
        )
        .unwrap();
        assert!(
            journal
                .snapshot()
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );

        fs::write(
            journal.path(),
            r#"{"schema":"rust-atlas/pending-events-v1","next_sequence":2,"events":{"../escape.rs":{"path":"../escape.rs","first_sequence":1,"latest_sequence":1,"first_seen_ms":1,"latest_seen_ms":1}}}"#,
        )
        .unwrap();
        assert!(
            journal
                .snapshot()
                .unwrap_err()
                .to_string()
                .contains("unsafe pending path")
        );

        let oversized = fs::File::create(journal.path()).unwrap();
        oversized.set_len(MAX_LIVE_FILE_BYTES + 1).unwrap();
        assert!(
            journal
                .snapshot()
                .unwrap_err()
                .to_string()
                .contains("byte limit")
        );
        fs::remove_dir_all(graph).ok();
    }
}
