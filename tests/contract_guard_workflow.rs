#[test]
fn contract_guard_posts_compact_pr_comment_and_keeps_full_step_summary() {
    let workflow = include_str!("../.github/workflows/contract-guard.yml");

    for term in [
        "> /tmp/contract-comment.md",
        "Full per-contract details are available in the workflow run summary.",
        "cat /tmp/contract-summary.md >> \"$GITHUB_STEP_SUMMARY\"",
        "readFileSync('/tmp/contract-comment.md', 'utf8')",
    ] {
        assert!(
            workflow.contains(term),
            "missing compact contract report boundary {term}"
        );
    }
}
