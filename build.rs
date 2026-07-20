use std::process::Command;

fn git_stdout(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

// Embed the compiler build identity for provenance manifests (v2 records the
// exact build a compilation ran under). No git -> the env stays unset and the
// runtime falls back to the literal `unknown`.
fn main() {
    if let Some(head) = git_stdout(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    if let Some(symbolic_head) = git_stdout(&["rev-parse", "--symbolic-full-name", "HEAD"])
        && symbolic_head != "HEAD"
        && let Some(branch_ref) = git_stdout(&["rev-parse", "--git-path", &symbolic_head])
    {
        println!("cargo:rerun-if-changed={branch_ref}");
    }
    if let Some(commit) = git_stdout(&["rev-parse", "HEAD"]) {
        println!("cargo:rustc-env=AGENT_SPEC_BUILD_COMMIT={commit}");
    }
}
