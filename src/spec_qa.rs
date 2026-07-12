use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum QaClass {
    A,
    B,
    C,
}

impl QaClass {
    pub fn try_parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_uppercase().as_str() {
            "A" => Ok(QaClass::A),
            "B" => Ok(QaClass::B),
            "C" => Ok(QaClass::C),
            other => Err(format!(
                "unsupported QA risk class `{other}`; expected A, B, or C"
            )),
        }
    }

    pub fn parse(value: Option<&str>) -> Self {
        value
            .and_then(|value| Self::try_parse(value).ok())
            .unwrap_or(QaClass::B)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QaEvidenceKind {
    Lifecycle,
    Trace,
    TargetedTests,
    AdversarialReview,
}

pub fn required_evidence_for(class: QaClass) -> Vec<QaEvidenceKind> {
    match class {
        QaClass::A => vec![
            QaEvidenceKind::Lifecycle,
            QaEvidenceKind::Trace,
            QaEvidenceKind::TargetedTests,
            QaEvidenceKind::AdversarialReview,
        ],
        QaClass::B => vec![QaEvidenceKind::Lifecycle, QaEvidenceKind::Trace],
        QaClass::C => vec![QaEvidenceKind::Lifecycle],
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QaEvidenceState {
    pub lifecycle: bool,
    pub trace: bool,
    pub targeted_tests: bool,
    pub adversarial_review: bool,
}

pub fn missing_evidence(class: QaClass, state: QaEvidenceState) -> Vec<QaEvidenceKind> {
    required_evidence_for(class)
        .into_iter()
        .filter(|kind| match kind {
            QaEvidenceKind::Lifecycle => !state.lifecycle,
            QaEvidenceKind::Trace => !state.trace,
            QaEvidenceKind::TargetedTests => !state.targeted_tests,
            QaEvidenceKind::AdversarialReview => !state.adversarial_review,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qa_class_a_requires_lifecycle_trace_targeted_tests_and_adversarial_review() {
        let evidence = required_evidence_for(QaClass::A);
        assert!(evidence.contains(&QaEvidenceKind::Lifecycle));
        assert!(evidence.contains(&QaEvidenceKind::Trace));
        assert!(evidence.contains(&QaEvidenceKind::TargetedTests));
        assert!(evidence.contains(&QaEvidenceKind::AdversarialReview));
    }

    #[test]
    fn test_qa_class_defaults_to_b_for_unknown_values() {
        assert_eq!(QaClass::parse(Some("A")), QaClass::A);
        assert_eq!(QaClass::parse(Some("B")), QaClass::B);
        assert_eq!(QaClass::parse(Some("C")), QaClass::C);
        assert_eq!(QaClass::parse(Some("unknown")), QaClass::B);
        assert_eq!(QaClass::parse(None), QaClass::B);
        assert!(QaClass::try_parse("unknown").is_err());
    }

    #[test]
    fn test_qa_gate_reports_missing_class_a_and_b_evidence() {
        let state = QaEvidenceState {
            lifecycle: true,
            trace: false,
            targeted_tests: true,
            adversarial_review: false,
        };
        assert_eq!(
            missing_evidence(QaClass::A, state),
            vec![QaEvidenceKind::Trace, QaEvidenceKind::AdversarialReview]
        );
        assert_eq!(
            missing_evidence(QaClass::B, state),
            vec![QaEvidenceKind::Trace]
        );
        assert!(missing_evidence(QaClass::C, state).is_empty());
    }
}
