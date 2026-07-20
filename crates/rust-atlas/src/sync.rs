use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::generation;
use crate::live::{LiveRuntimeState, LiveRuntimeStatus, PendingJournal, RetryClass, RetryPolicy};
use crate::locking::WriterLease;
use crate::{
    AtlasError, BuildDiagnostic, BuildOptions, BuildReport, Meta, MirBuildOptions,
    build_with_meta_locked, input_plan, read_meta_at,
};

pub struct SyncRequest<'a> {
    pub code_root: &'a Path,
    pub graph_root: &'a Path,
    pub build_options: &'a BuildOptions,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncReceipt {
    pub snapshot_watermark: u64,
    pub pending_before: usize,
    pub pending_after: usize,
    pub build: BuildReport,
    pub runtime: LiveRuntimeStatus,
}

pub fn sync_once(request: SyncRequest<'_>) -> Result<SyncReceipt, AtlasError> {
    let journal = PendingJournal::open(request.graph_root)?;
    let snapshot = journal.snapshot()?;
    let pending_before = snapshot.events.len();
    let mut runtime = LiveRuntimeStatus::load(request.graph_root)?;
    let writer = match WriterLease::try_acquire(request.graph_root) {
        Ok(writer) => writer,
        Err(error) => {
            record_sync_failure(
                request.graph_root,
                &mut runtime,
                RetryClass::WriterLock,
                &error,
                pending_before,
            );
            return Err(error);
        }
    };
    let (build_options, known_meta, retain_authority, mir_options) =
        match retained_build_context(request.graph_root, request.build_options) {
            Ok(context) => context,
            Err(error) => {
                record_sync_failure(
                    request.graph_root,
                    &mut runtime,
                    RetryClass::Ordinary,
                    &error,
                    pending_before,
                );
                return Err(error);
            }
        };
    let build = build_with_meta_locked(
        request.code_root,
        request.graph_root,
        &build_options,
        known_meta,
        retain_authority,
        mir_options.as_ref(),
        &writer,
    );
    let mut build = match build {
        Ok(build) => build,
        Err(error) => {
            record_sync_failure(
                request.graph_root,
                &mut runtime,
                RetryClass::Ordinary,
                &error,
                pending_before,
            );
            return Err(error);
        }
    };

    let unparsed = build.unparsed.iter().cloned().collect::<BTreeSet<_>>();
    let remaining = match journal.acknowledge(&snapshot, &unparsed) {
        Ok(remaining) => remaining,
        Err(error) => {
            build.diagnostics.push(BuildDiagnostic {
                code: "live-maintenance-failed".to_string(),
                severity: "warning".to_string(),
                message: format!(
                    "generation committed but pending acknowledgement failed: {error}"
                ),
            });
            journal.snapshot()?
        }
    };
    runtime.retry.reset_after_success();
    runtime.pending_paths = remaining
        .events
        .iter()
        .map(|event| event.path.clone())
        .collect();
    runtime.state = if runtime.watch_healthy == Some(false) {
        LiveRuntimeState::Degraded
    } else if runtime.pending_paths.is_empty() {
        LiveRuntimeState::Healthy
    } else {
        LiveRuntimeState::Pending
    };
    runtime.generation = Some(build.generation.clone());
    runtime.diagnostics = runtime.watch_diagnostic.iter().cloned().collect();
    if !unparsed.is_empty() {
        runtime.diagnostics.push(format!(
            "{} unparsed source path(s) remain pending",
            unparsed.len()
        ));
    }
    if let Err(error) = runtime.store(request.graph_root) {
        runtime.diagnostics.push(format!(
            "generation committed but live status persistence failed: {error}"
        ));
        build.diagnostics.push(BuildDiagnostic {
            code: "live-maintenance-failed".to_string(),
            severity: "warning".to_string(),
            message: error.to_string(),
        });
    }
    drop(writer);

    Ok(SyncReceipt {
        snapshot_watermark: snapshot.watermark,
        pending_before,
        pending_after: remaining.events.len(),
        build,
        runtime,
    })
}

fn retained_build_context(
    graph_root: &Path,
    requested: &BuildOptions,
) -> Result<(BuildOptions, Option<Meta>, bool, Option<MirBuildOptions>), AtlasError> {
    let Some(snapshot) = generation::resolve_optional_snapshot(graph_root)? else {
        return Ok((requested.clone(), None, false, None));
    };
    if snapshot.generation.is_some() && !generation::artifacts_match_manifest(&snapshot) {
        return Err(AtlasError::Invariant(
            "committed generation artifact integrity check failed; run `atlas build`".to_string(),
        ));
    }
    let meta = read_meta_at(&snapshot.data_dir)?;
    let mut options = requested.clone();
    if snapshot.generation.is_some() {
        let plan = input_plan::load_committed(&snapshot.data_dir).ok_or_else(|| {
            AtlasError::Invariant(
                "committed generation input plan is missing or incompatible; run `atlas build`"
                    .to_string(),
            )
        })?;
        plan.apply_build_inputs(&mut options);
    }
    options.scip_index = options.scip_index.or_else(|| {
        meta.capability
            .scip_index
            .as_deref()
            .map(PathBuf::from)
            .filter(|path| path.exists())
    });
    options.dynamic_dispatch = meta.capability.dynamic_dispatch;
    let mir = meta
        .capability
        .mir_overlay
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .map(|overlay| MirBuildOptions {
            overlay: Some(overlay),
            driver: None,
        });
    Ok((options, Some(meta), true, mir))
}

fn record_sync_failure(
    graph_root: &Path,
    runtime: &mut LiveRuntimeStatus,
    class: RetryClass,
    error: &AtlasError,
    pending: usize,
) {
    let prior_state = runtime.state;
    let outcome = runtime
        .retry
        .record_failure(class, error.to_string(), &RetryPolicy::default());
    runtime.state = if runtime.watch_healthy == Some(false) {
        LiveRuntimeState::Degraded
    } else {
        match outcome {
            Ok(outcome) if outcome.degraded => LiveRuntimeState::Degraded,
            _ if prior_state == LiveRuntimeState::Warming => LiveRuntimeState::Warming,
            _ if pending > 0 => LiveRuntimeState::Pending,
            _ => prior_state,
        }
    };
    runtime.diagnostics = vec![error.to_string()];
    let _ = runtime.store(graph_root);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use super::*;
    use crate::live::{
        LiveRuntimeState, LiveRuntimeStatus, PendingJournal, RetryClass, RetryPolicy, RetryState,
    };
    use crate::{AtlasError, BuildOptions, build};

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
            "[package]\nname = \"sync-test\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn value() -> u32 { 1 }\n").unwrap();
        build(&code, &graph, &BuildOptions::default()).unwrap();
        (code, graph)
    }

    #[test]
    fn test_atlas_retry_state_resets_after_successful_sync() {
        let (code, graph) = fixture("sync-retry-reset");
        fs::write(code.join("src/lib.rs"), "pub fn value() -> u32 { 2 }\n").unwrap();
        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/lib.rs", 100).unwrap();
        let policy = RetryPolicy::default();
        let mut status = LiveRuntimeStatus::new(LiveRuntimeState::Pending);
        status
            .retry
            .record_failure(RetryClass::WriterLock, "busy", &policy)
            .unwrap();
        status
            .retry
            .record_failure(RetryClass::Ordinary, "failed", &policy)
            .unwrap();
        status.store(&graph).unwrap();

        let receipt = sync_once(SyncRequest {
            code_root: &code,
            graph_root: &graph,
            build_options: &BuildOptions::default(),
        })
        .unwrap();

        assert_eq!(receipt.snapshot_watermark, 1);
        assert_eq!(receipt.pending_before, 1);
        assert_eq!(receipt.pending_after, 0);
        assert_eq!(receipt.runtime.state, LiveRuntimeState::Healthy);
        assert_eq!(receipt.runtime.retry, RetryState::default());
        assert_eq!(
            receipt.runtime.generation.as_deref(),
            Some(receipt.build.generation.as_str())
        );
        fs::remove_dir_all(graph.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_failed_sync_preserves_pending_events() {
        let (code, graph) = fixture("sync-failure-pending");
        let journal = PendingJournal::open(&graph).unwrap();
        fs::write(code.join("src/lib.rs"), "pub fn broken(\n").unwrap();
        journal.record("src/lib.rs", 100).unwrap();

        let receipt = sync_once(SyncRequest {
            code_root: &code,
            graph_root: &graph,
            build_options: &BuildOptions::default(),
        })
        .unwrap();
        assert_eq!(receipt.build.unparsed, vec!["src/lib.rs"]);
        assert_eq!(receipt.pending_after, 1);
        assert_eq!(receipt.runtime.state, LiveRuntimeState::Pending);

        journal.record("Cargo.toml", 200).unwrap();
        let pending_before = fs::read(journal.path()).unwrap();
        let pointer_before = fs::read(graph.join("CURRENT.json")).unwrap();
        let cancelled = Arc::new(AtomicBool::new(true));
        let error = sync_once(SyncRequest {
            code_root: &code,
            graph_root: &graph,
            build_options: &BuildOptions {
                cancellation: Some(cancelled),
                ..BuildOptions::default()
            },
        })
        .unwrap_err();

        assert!(matches!(error, AtlasError::Cancelled));
        assert_eq!(fs::read(journal.path()).unwrap(), pending_before);
        assert_eq!(
            fs::read(graph.join("CURRENT.json")).unwrap(),
            pointer_before
        );
        let status = LiveRuntimeStatus::load(&graph).unwrap();
        assert_eq!(status.retry.ordinary_attempts, 1);
        assert_eq!(status.pending_paths, vec!["Cargo.toml", "src/lib.rs"]);
        fs::remove_dir_all(graph.parent().unwrap()).ok();
    }

    #[test]
    fn test_atlas_sync_preserves_watch_degradation_after_success() {
        let (code, graph) = fixture("sync-watch-degraded");
        fs::write(code.join("src/lib.rs"), "pub fn value() -> u32 { 2 }\n").unwrap();
        let journal = PendingJournal::open(&graph).unwrap();
        journal.record("src/lib.rs", 100).unwrap();
        let mut status = LiveRuntimeStatus::new(LiveRuntimeState::Degraded);
        status.watch_healthy = Some(false);
        status.watch_diagnostic = Some("partial watch coverage".to_string());
        status.diagnostics = vec!["partial watch coverage".to_string()];
        status.store(&graph).unwrap();

        let receipt = sync_once(SyncRequest {
            code_root: &code,
            graph_root: &graph,
            build_options: &BuildOptions::default(),
        })
        .unwrap();

        assert_eq!(receipt.pending_after, 0);
        assert_eq!(receipt.runtime.state, LiveRuntimeState::Degraded);
        assert_eq!(receipt.runtime.watch_healthy, Some(false));
        assert_eq!(
            receipt.runtime.watch_diagnostic.as_deref(),
            Some("partial watch coverage")
        );
        fs::remove_dir_all(graph.parent().unwrap()).ok();
    }
}
