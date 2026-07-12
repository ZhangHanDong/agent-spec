// Embed the compiler build identity for provenance manifests (v2 records the
// exact build a compilation ran under). No git → the env stays unset and the
// runtime falls back to the literal `unknown`.
fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .unwrap_or_default();
    if !commit.is_empty() {
        println!("cargo:rustc-env=AGENT_SPEC_BUILD_COMMIT={commit}");
    }
}
