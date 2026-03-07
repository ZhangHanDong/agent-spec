//! VCS context detection and retrieval.
//!
//! Supports Git and jj. Detection is automatic based on directory
//! markers (.git / .jj). All jj interaction is through CLI calls—
//! no jj-lib dependency.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

// ── Types ───────────────────────────────────────────────────────

/// Detected version control system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VcsType {
    Git,
    Jj,
    None,
}

/// Snapshot of VCS state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcsContext {
    pub vcs_type: VcsType,
    /// Git: short commit hash. jj: short change ID.
    pub change_ref: String,
    /// jj only: short operation ID. None for Git.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_ref: Option<String>,
}

// ── Detection ───────────────────────────────────────────────────

/// Auto-detect VCS type from the given directory.
/// Prefers jj when colocated (both .jj and .git exist).
pub fn detect_vcs_type(repo_root: &Path) -> VcsType {
    if repo_root.join(".jj").exists() {
        VcsType::Jj
    } else if repo_root.join(".git").exists() {
        VcsType::Git
    } else {
        VcsType::None
    }
}

// ── Context Retrieval ───────────────────────────────────────────

/// Get the current VCS context. Returns None if no VCS is detected
/// or if the necessary CLI commands fail.
pub fn get_vcs_context(repo_root: &Path) -> Option<VcsContext> {
    match detect_vcs_type(repo_root) {
        VcsType::Jj => get_jj_context(repo_root),
        VcsType::Git => get_git_context(repo_root),
        VcsType::None => None,
    }
}

fn get_jj_context(repo_root: &Path) -> Option<VcsContext> {
    let change_ref = run_jj(repo_root, &[
        "log", "-r", "@", "--no-graph", "-T", "change_id.short()",
    ])?;
    let operation_ref = run_jj(repo_root, &[
        "op", "log", "--limit", "1", "--no-graph", "-T", "self.id().short()",
    ]);
    Some(VcsContext {
        vcs_type: VcsType::Jj,
        change_ref,
        operation_ref,
    })
}

fn get_git_context(repo_root: &Path) -> Option<VcsContext> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let change_ref = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if change_ref.is_empty() {
        return None;
    }
    Some(VcsContext {
        vcs_type: VcsType::Git,
        change_ref,
        operation_ref: None,
    })
}

// ── Cross-Run Diff ──────────────────────────────────────────────

/// Get the list of files changed between two jj operations.
/// Returns None if jj is unavailable or the command fails.
pub fn jj_diff_between_ops(repo_root: &Path, from_op: &str, to_op: &str) -> Option<Vec<String>> {
    let output = run_jj(repo_root, &[
        "diff", "--name-only", "--from", from_op, "--to", to_op,
    ])?;
    let files: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    if files.is_empty() { None } else { Some(files) }
}

// ── Helpers ─────────────────────────────────────────────────────

/// Run a jj command and return trimmed stdout, or None on failure.
fn run_jj(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("jj")
        .current_dir(repo_root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_vcs_detect_prefers_jj_when_colocated() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-vcs-colocate-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::create_dir_all(dir.join(".jj")).unwrap();

        assert_eq!(detect_vcs_type(&dir), VcsType::Jj);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_vcs_detect_returns_git_when_only_git() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-vcs-git-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join(".git")).unwrap();

        assert_eq!(detect_vcs_type(&dir), VcsType::Git);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_vcs_detect_returns_none_outside_repo() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-vcs-none-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        assert_eq!(detect_vcs_type(&dir), VcsType::None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_vcs_context_returns_jj_ids() {
        // Skip if jj is not installed
        let jj_available = std::process::Command::new("jj")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !jj_available {
            eprintln!("skipping test_vcs_context_returns_jj_ids: jj not available");
            return;
        }

        let dir = std::env::temp_dir().join(format!(
            "agent-spec-vcs-jj-ctx-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        // Init jj repo
        let init = std::process::Command::new("jj")
            .args(["git", "init"])
            .current_dir(&dir)
            .output();
        if init.is_err() || !init.as_ref().unwrap().status.success() {
            let _ = fs::remove_dir_all(&dir);
            eprintln!("skipping test_vcs_context_returns_jj_ids: jj git init failed");
            return;
        }

        // Create a file and commit
        fs::write(dir.join("test.txt"), "hello").unwrap();
        let _ = std::process::Command::new("jj")
            .args(["commit", "-m", "test commit"])
            .current_dir(&dir)
            .output();

        let ctx = get_vcs_context(&dir);
        assert!(ctx.is_some(), "should return VCS context in jj repo");
        let ctx = ctx.unwrap();
        assert_eq!(ctx.vcs_type, VcsType::Jj);
        assert!(!ctx.change_ref.is_empty(), "change_ref should be non-empty");
        assert!(ctx.operation_ref.is_some(), "operation_ref should be Some");
        assert!(
            !ctx.operation_ref.as_ref().unwrap().is_empty(),
            "operation_ref should be non-empty"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_vcs_context_returns_git_hash() {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-vcs-git-ctx-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();

        // Init git repo with one commit
        let _ = std::process::Command::new("git")
            .args(["init"])
            .current_dir(&dir)
            .output();
        let _ = std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&dir)
            .output();
        let _ = std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&dir)
            .output();
        fs::write(dir.join("test.txt"), "hello").unwrap();
        let _ = std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&dir)
            .output();
        let _ = std::process::Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&dir)
            .output();

        let ctx = get_vcs_context(&dir);
        assert!(ctx.is_some(), "should return VCS context in git repo");
        let ctx = ctx.unwrap();
        assert_eq!(ctx.vcs_type, VcsType::Git);
        assert!(!ctx.change_ref.is_empty(), "change_ref should be non-empty short hash");
        assert!(ctx.operation_ref.is_none(), "operation_ref should be None for git");

        let _ = fs::remove_dir_all(&dir);
    }
}
