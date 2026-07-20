use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, TrySendError, sync_channel};

use notify::{RecursiveMode, Watcher};
use serde::Serialize;

use crate::AtlasError;
use crate::live::PendingJournal;
use crate::scope::{AtlasScope, ScopeEntryKind};

pub const DEFAULT_MAX_WATCH_DIRECTORIES: usize = 50_000;
const EVENT_CHANNEL_CAPACITY: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatchPlatform {
    MacOs,
    Windows,
    Linux,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WatchMode {
    Recursive,
    NonRecursive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchTarget {
    pub path: PathBuf,
    pub mode: WatchMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum WatchCoverage {
    Complete,
    Partial {
        watched: usize,
        total: usize,
        reason: String,
    },
    Degraded {
        watched: usize,
        total: usize,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchPlan {
    pub targets: Vec<WatchTarget>,
    pub coverage: WatchCoverage,
}

impl WatchPlan {
    pub fn build(
        scope: &AtlasScope,
        platform: WatchPlatform,
        max_directories: usize,
    ) -> Result<Self, AtlasError> {
        if max_directories == 0 {
            return Err(watcher_error("watch directory limit must be positive"));
        }
        match platform {
            WatchPlatform::MacOs | WatchPlatform::Windows => Ok(Self {
                targets: vec![WatchTarget {
                    path: scope.code_root().to_path_buf(),
                    mode: WatchMode::Recursive,
                }],
                coverage: WatchCoverage::Complete,
            }),
            WatchPlatform::Linux | WatchPlatform::Other => {
                let directories = scope.watch_directories();
                let total = directories.len();
                let targets = directories
                    .into_iter()
                    .take(max_directories)
                    .map(|path| WatchTarget {
                        path,
                        mode: WatchMode::NonRecursive,
                    })
                    .collect::<Vec<_>>();
                let coverage = if targets.len() == total {
                    WatchCoverage::Complete
                } else {
                    WatchCoverage::Partial {
                        watched: targets.len(),
                        total,
                        reason: format!(
                            "directory cap {max_directories} covers only {} of {total} directories",
                            targets.len()
                        ),
                    }
                };
                Ok(Self { targets, coverage })
            }
        }
    }

    pub fn with_backend_error(mut self, reason: impl Into<String>) -> Self {
        let (watched, total) = self.coverage_counts();
        self.coverage = WatchCoverage::Degraded {
            watched,
            total,
            reason: reason.into(),
        };
        self
    }

    pub fn is_healthy(&self) -> bool {
        self.coverage == WatchCoverage::Complete
    }

    fn coverage_counts(&self) -> (usize, usize) {
        match &self.coverage {
            WatchCoverage::Complete => (self.targets.len(), self.targets.len()),
            WatchCoverage::Partial { watched, total, .. }
            | WatchCoverage::Degraded { watched, total, .. } => (*watched, *total),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchDrain {
    pub recorded: usize,
    pub coverage: WatchCoverage,
}

pub struct AtlasWatcher {
    watcher: notify::RecommendedWatcher,
    receiver: Receiver<notify::Result<notify::Event>>,
    overflowed: Arc<AtomicBool>,
    scope: AtlasScope,
    journal: PendingJournal,
    plan: WatchPlan,
    platform: WatchPlatform,
    max_directories: usize,
}

impl AtlasWatcher {
    pub fn start(
        scope: AtlasScope,
        journal: PendingJournal,
        max_directories: usize,
    ) -> Result<Self, AtlasError> {
        let platform = WatchPlatform::current();
        let mut plan = WatchPlan::build(&scope, platform, max_directories)?;
        let (sender, receiver) = sync_channel(EVENT_CHANNEL_CAPACITY);
        let overflowed = Arc::new(AtomicBool::new(false));
        let callback_overflow = Arc::clone(&overflowed);
        let mut watcher = notify::recommended_watcher(move |event| {
            if let Err(TrySendError::Full(_)) = sender.try_send(event) {
                callback_overflow.store(true, Ordering::Release);
            }
        })
        .map_err(|error| watcher_error(error.to_string()))?;

        for (installed, target) in plan.targets.iter().enumerate() {
            let mode = match target.mode {
                WatchMode::Recursive => RecursiveMode::Recursive,
                WatchMode::NonRecursive => RecursiveMode::NonRecursive,
            };
            if let Err(error) = watcher.watch(&target.path, mode) {
                let total = plan.coverage_counts().1;
                plan.coverage = WatchCoverage::Degraded {
                    watched: installed,
                    total,
                    reason: error.to_string(),
                };
                break;
            }
        }
        Ok(Self {
            watcher,
            receiver,
            overflowed,
            scope,
            journal,
            plan,
            platform,
            max_directories,
        })
    }

    pub fn plan(&self) -> &WatchPlan {
        &self.plan
    }

    pub fn drain(&mut self, seen_ms: u64) -> Result<WatchDrain, AtlasError> {
        let mut paths = Vec::new();
        loop {
            match self.receiver.try_recv() {
                Ok(Ok(event)) => paths.extend(event.paths),
                Ok(Err(error)) => {
                    self.degrade(error.to_string());
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        if self.overflowed.swap(false, Ordering::AcqRel) {
            self.degrade(format!(
                "watch event channel exceeded {EVENT_CHANNEL_CAPACITY} entries"
            ));
        }
        self.refresh_non_recursive_directories();
        let recorded = ingest_paths(&self.scope, &self.journal, paths, seen_ms)?;
        Ok(WatchDrain {
            recorded,
            coverage: self.plan.coverage.clone(),
        })
    }

    fn degrade(&mut self, reason: String) {
        let (watched, total) = self.plan.coverage_counts();
        self.plan.coverage = WatchCoverage::Degraded {
            watched,
            total,
            reason,
        };
    }

    fn refresh_non_recursive_directories(&mut self) {
        if !matches!(self.platform, WatchPlatform::Linux | WatchPlatform::Other) {
            return;
        }
        let directories = self.scope.watch_directories();
        let directory_set = directories.iter().cloned().collect::<BTreeSet<_>>();
        let stale = self
            .plan
            .targets
            .iter()
            .filter(|target| !directory_set.contains(&target.path))
            .map(|target| target.path.clone())
            .collect::<Vec<_>>();
        for path in stale {
            let _ = self.watcher.unwatch(&path);
            self.plan.targets.retain(|target| target.path != path);
        }

        let mut installed = self
            .plan
            .targets
            .iter()
            .map(|target| target.path.clone())
            .collect::<BTreeSet<_>>();
        for path in &directories {
            if installed.contains(path) || installed.len() == self.max_directories {
                continue;
            }
            if let Err(error) = self.watcher.watch(path, RecursiveMode::NonRecursive) {
                self.plan.coverage = WatchCoverage::Degraded {
                    watched: installed.len(),
                    total: directories.len(),
                    reason: error.to_string(),
                };
                return;
            }
            installed.insert(path.clone());
            self.plan.targets.push(WatchTarget {
                path: path.clone(),
                mode: WatchMode::NonRecursive,
            });
        }
        self.plan
            .targets
            .sort_by(|left, right| left.path.cmp(&right.path));
        self.plan.coverage = if installed.len() == directories.len() {
            WatchCoverage::Complete
        } else {
            WatchCoverage::Partial {
                watched: installed.len(),
                total: directories.len(),
                reason: format!(
                    "directory cap {} covers only {} of {} directories",
                    self.max_directories,
                    installed.len(),
                    directories.len()
                ),
            }
        };
    }
}

impl WatchPlatform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

pub fn ingest_paths(
    scope: &AtlasScope,
    journal: &PendingJournal,
    paths: impl IntoIterator<Item = PathBuf>,
    seen_ms: u64,
) -> Result<usize, AtlasError> {
    let mut accepted = BTreeSet::new();
    for path in paths {
        if matches!(
            scope.classify(&path)?,
            ScopeEntryKind::RustSource | ScopeEntryKind::CargoInput
        ) && let Some(relative) = scope.relative_path(&path)?
        {
            accepted.insert(relative);
        }
    }
    for path in &accepted {
        journal.record(path, seen_ms)?;
    }
    Ok(accepted.len())
}

fn watcher_error(detail: impl Into<String>) -> AtlasError {
    AtlasError::Watcher {
        detail: detail.into(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::live::PendingJournal;
    use crate::scope::AtlasScope;

    fn fixture(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "rust-atlas-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let code = root.join("code");
        let graph = code.join(".agent-spec/graph");
        fs::create_dir_all(code.join("src/nested")).unwrap();
        fs::create_dir_all(code.join("ignored")).unwrap();
        fs::create_dir_all(graph.join(".runtime")).unwrap();
        fs::write(
            code.join("Cargo.toml"),
            "[package]\nname = \"watch-test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(code.join(".gitignore"), "ignored/\n").unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn live() {}\n").unwrap();
        fs::write(code.join("src/nested/mod.rs"), "pub fn nested() {}\n").unwrap();
        fs::write(code.join("ignored/no.rs"), "fn ignored() {}\n").unwrap();
        fs::write(graph.join(".runtime/no.rs"), "fn runtime() {}\n").unwrap();
        let outside = root.join("outside.rs");
        fs::write(&outside, "fn outside() {}\n").unwrap();
        (code, graph, outside)
    }

    #[test]
    fn test_atlas_watch_plan_is_bounded_per_platform() {
        let (code, graph, _) = fixture("watch-plan");
        let scope = AtlasScope::discover(&code, &graph).unwrap();

        for platform in [WatchPlatform::MacOs, WatchPlatform::Windows] {
            let plan = WatchPlan::build(&scope, platform, 2).unwrap();
            assert_eq!(plan.targets.len(), 1);
            assert_eq!(plan.targets[0].mode, WatchMode::Recursive);
            assert_eq!(plan.coverage, WatchCoverage::Complete);
        }
        let linux = WatchPlan::build(&scope, WatchPlatform::Linux, 2).unwrap();
        assert_eq!(linux.targets.len(), 2);
        assert!(
            linux
                .targets
                .windows(2)
                .all(|pair| pair[0].path < pair[1].path)
        );
        assert!(matches!(
            linux.coverage,
            WatchCoverage::Partial {
                watched: 2,
                total,
                ..
            } if total > 2
        ));
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_watcher_records_source_and_manifest_events() {
        let (code, graph, outside) = fixture("watch-events");
        let scope = AtlasScope::discover(&code, &graph).unwrap();
        let journal = PendingJournal::open(&graph).unwrap();

        assert_eq!(
            ingest_paths(
                &scope,
                &journal,
                [
                    code.join("src/lib.rs"),
                    code.join("Cargo.toml"),
                    code.join("ignored/no.rs"),
                    graph.join(".runtime/no.rs"),
                    outside,
                ],
                100,
            )
            .unwrap(),
            2
        );
        ingest_paths(&scope, &journal, [code.join("src/lib.rs")], 200).unwrap();
        fs::remove_file(code.join("src/lib.rs")).unwrap();
        ingest_paths(&scope, &journal, [code.join("src/lib.rs")], 300).unwrap();

        let snapshot = journal.snapshot().unwrap();
        assert_eq!(
            snapshot
                .events
                .iter()
                .map(|event| (
                    event.path.as_str(),
                    event.first_sequence,
                    event.latest_sequence
                ))
                .collect::<Vec<_>>(),
            vec![("Cargo.toml", 1, 1), ("src/lib.rs", 2, 4)]
        );
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_watch_capacity_reports_partial_or_degraded() {
        let (code, graph, _) = fixture("watch-capacity");
        let scope = AtlasScope::discover(&code, &graph).unwrap();
        let partial = WatchPlan::build(&scope, WatchPlatform::Linux, 1).unwrap();
        assert!(matches!(partial.coverage, WatchCoverage::Partial { .. }));

        let degraded = partial.with_backend_error("watch descriptor limit reached");
        assert!(matches!(
            degraded.coverage,
            WatchCoverage::Degraded { ref reason, .. }
                if reason.contains("descriptor limit")
        ));
        assert!(!degraded.is_healthy());
        fs::remove_dir_all(code.parent().unwrap()).ok();
    }
}
