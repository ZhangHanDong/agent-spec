//! Derived liveness (§7). Never stored; computed from current spec verdicts.

use crate::spec_core::{Verdict, VerificationSummary};
use crate::spec_knowledge::model::{Liveness, LivenessDeclared};

/// Roll a single spec's verification summary into one representative verdict:
/// Fail if anything failed; Pass only if every scenario passed; otherwise a
/// not-yet-proven verdict (Skip stands in for skip/uncertain/pending/empty).
pub fn spec_rollup(summary: &VerificationSummary) -> Verdict {
    if summary.failed > 0 {
        Verdict::Fail
    } else if summary.total == 0
        || summary.skipped > 0
        || summary.uncertain > 0
        || summary.pending_review > 0
    {
        Verdict::Skip
    } else {
        Verdict::Pass
    }
}

/// Precedence ladder (§7), total and mutually exclusive:
/// 1. declared `n/a`            -> Na
/// 2. any satisfying spec Fail  -> Violated
/// 3. none, or any not-Pass     -> Unproven
/// 4. all Pass                  -> Honored
pub fn decision_liveness(declared: LivenessDeclared, spec_verdicts: &[Verdict]) -> Liveness {
    if declared == LivenessDeclared::Na {
        return Liveness::Na;
    }
    if spec_verdicts.contains(&Verdict::Fail) {
        return Liveness::Violated;
    }
    if spec_verdicts.is_empty() || spec_verdicts.iter().any(|v| *v != Verdict::Pass) {
        return Liveness::Unproven;
    }
    Liveness::Honored
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn summary(total: usize, passed: usize, failed: usize, skipped: usize) -> VerificationSummary {
        VerificationSummary {
            total,
            passed,
            failed,
            skipped,
            uncertain: 0,
            pending_review: 0,
        }
    }

    #[test]
    fn test_spec_rollup_fail_dominates() {
        assert_eq!(spec_rollup(&summary(3, 2, 1, 0)), Verdict::Fail);
    }

    #[test]
    fn test_spec_rollup_all_pass() {
        assert_eq!(spec_rollup(&summary(2, 2, 0, 0)), Verdict::Pass);
    }

    #[test]
    fn test_spec_rollup_skip_is_not_pass() {
        assert_eq!(spec_rollup(&summary(2, 1, 0, 1)), Verdict::Skip);
        assert_eq!(spec_rollup(&summary(0, 0, 0, 0)), Verdict::Skip);
    }

    #[test]
    fn test_liveness_na_short_circuits() {
        assert_eq!(
            decision_liveness(LivenessDeclared::Na, &[Verdict::Fail]),
            Liveness::Na
        );
    }

    #[test]
    fn test_liveness_violated_on_any_fail() {
        assert_eq!(
            decision_liveness(LivenessDeclared::Auto, &[Verdict::Pass, Verdict::Fail]),
            Liveness::Violated
        );
    }

    #[test]
    fn test_liveness_unproven_when_empty_or_not_all_pass() {
        assert_eq!(
            decision_liveness(LivenessDeclared::Auto, &[]),
            Liveness::Unproven
        );
        assert_eq!(
            decision_liveness(LivenessDeclared::Auto, &[Verdict::Pass, Verdict::Skip]),
            Liveness::Unproven
        );
    }

    #[test]
    fn test_liveness_honored_when_all_pass() {
        assert_eq!(
            decision_liveness(LivenessDeclared::Auto, &[Verdict::Pass, Verdict::Pass]),
            Liveness::Honored
        );
    }
}
