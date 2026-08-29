#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `lifecycle --resume` must never carry a pass forward from a checkpoint
//! taken on different spec content (REQ-CHECKPOINT-SPEC-FINGERPRINT).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SPEC: &str = r#"spec: task
name: "Resume Demo"
---

## Intent

Resume demo.

## Acceptance Criteria

Scenario: A
  Test: test_that_does_not_exist_anywhere
  Given x
  When y
  Then z
"#;

fn temp_dir(tag: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("agent-spec-resume-{tag}-{stamp}"));
    fs::create_dir_all(dir.join("specs")).unwrap();
    dir
}

fn write_spec(dir: &Path, extra: &str) -> PathBuf {
    let path = dir.join("specs/task-resume.spec.md");
    fs::write(&path, format!("{SPEC}{extra}")).unwrap();
    path
}

fn lifecycle(dir: &Path, spec: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_agent-spec"))
        .arg("lifecycle")
        .arg(spec)
        .arg("--code")
        .arg(dir)
        .arg("--run-log-dir")
        .arg(dir)
        .arg("--layers")
        .arg("lint,boundary")
        .arg("--min-score")
        .arg("0")
        .args(extra)
        .output()
        .expect("run agent-spec")
}

fn checkpoint_path(dir: &Path) -> PathBuf {
    dir.join(".agent-spec/checkpoint.json")
}

/// Run once (no resume) to get a real checkpoint, then flip scenario A to pass.
fn seed_checkpoint_with_pass(dir: &Path, spec: &Path) {
    let out = lifecycle(dir, spec, &["--format", "json"]);
    let cp_path = checkpoint_path(dir);
    assert!(
        cp_path.exists(),
        "first run must write a checkpoint: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut cp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&cp_path).unwrap()).unwrap();
    assert!(
        cp["spec_fingerprint"]
            .as_str()
            .is_some_and(|f| !f.is_empty()),
        "checkpoint must record a spec fingerprint: {cp}"
    );
    cp["scenarios"]["A"]["verdict"] = serde_json::json!("pass");
    fs::write(&cp_path, serde_json::to_string_pretty(&cp).unwrap()).unwrap();
}

fn verdict_of_a(stdout: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(stdout).expect("json output");
    v["verification"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["scenario_name"] == "A")
        .map(|r| r["verdict"].as_str().unwrap().to_string())
        .expect("scenario A present")
}

#[test]
fn test_lifecycle_resume_uses_fresh_checkpoint() {
    let dir = temp_dir("fresh");
    let spec = write_spec(&dir, "");
    seed_checkpoint_with_pass(&dir, &spec);

    let out = lifecycle(
        &dir,
        &spec,
        &["--resume", "incremental", "--format", "json"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(verdict_of_a(&stdout), "pass", "{stdout}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(v.get("checkpoint_diagnostic").is_none(), "{stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_lifecycle_resume_ignores_stale_checkpoint() {
    let dir = temp_dir("stale");
    let spec = write_spec(&dir, "");
    seed_checkpoint_with_pass(&dir, &spec);
    // Change the spec content (same name, same scenario names).
    write_spec(&dir, "\n<!-- edited after checkpoint -->\n");

    let out = lifecycle(
        &dir,
        &spec,
        &["--resume", "incremental", "--format", "json"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(verdict_of_a(&stdout), "skip", "{stdout}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let msg = v["checkpoint_diagnostic"]["message"].as_str().unwrap_or("");
    assert!(msg.contains("spec content changed"), "{stdout}");
    assert!(
        !stdout.contains("checkpoint:incremental"),
        "no verdict may be carried forward: {stdout}"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_lifecycle_resume_stale_checkpoint_warns_on_stderr() {
    let dir = temp_dir("stderr");
    let spec = write_spec(&dir, "");
    seed_checkpoint_with_pass(&dir, &spec);
    write_spec(&dir, "\n<!-- edited -->\n");

    let out = lifecycle(
        &dir,
        &spec,
        &["--resume", "incremental", "--format", "text"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stderr.contains("checkpoint ignored"), "stderr: {stderr}");
    assert!(!stdout.contains("checkpoint ignored"), "stdout: {stdout}");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_lifecycle_resume_rejects_corrupt_checkpoint() {
    let dir = temp_dir("corrupt");
    let spec = write_spec(&dir, "");
    let cp_path = checkpoint_path(&dir);
    fs::create_dir_all(cp_path.parent().unwrap()).unwrap();
    fs::write(&cp_path, "{ not json").unwrap();

    let out = lifecycle(&dir, &spec, &["--resume", "incremental"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("checkpoint"), "stderr: {stderr}");
    let _ = fs::remove_dir_all(dir);
}
