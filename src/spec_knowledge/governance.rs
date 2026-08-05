//! Corpus-level governance lint (§9): id-conflict, supersession integrity, and
//! stale references across the whole knowledge set. Per-doc section/forcing-
//! function rules live in `lint`; this module needs every doc at once.
//!
//! Self-referential exemption (§9) is handled by the collectors that feed this:
//! they scan only the typed kind directories, never `standards/**` or README.

use crate::spec_core::{LintDiagnostic, Severity, Span};
use crate::spec_knowledge::lint::{lint_decision, lint_guidance, lint_requirement};
use crate::spec_knowledge::model::{DecisionStatus, KnowledgeDoc, KnowledgeKind};
use crate::spec_knowledge::parser::parse_knowledge;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeParseError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct KnowledgeCollection {
    pub docs: Vec<KnowledgeDoc>,
    pub parse_errors: Vec<KnowledgeParseError>,
}

/// Collect every typed knowledge doc under `knowledge_dir` (the four kind
/// directories), skipping README files and unparseable docs. Sorted by id.
/// `standards/**` is never scanned — that is the §9 self-referential exemption.
pub fn collect_knowledge(knowledge_dir: &Path) -> Vec<KnowledgeDoc> {
    collect_knowledge_checked(knowledge_dir).docs
}

/// Collect every typed knowledge doc and preserve parse failures for governance
/// gates. Sorted by id / path for deterministic reports.
pub fn collect_knowledge_checked(knowledge_dir: &Path) -> KnowledgeCollection {
    const KINDS: [KnowledgeKind; 4] = [
        KnowledgeKind::Decision,
        KnowledgeKind::Requirement,
        KnowledgeKind::Guidance,
        KnowledgeKind::Proposal,
    ];
    if !knowledge_dir.is_dir() {
        return KnowledgeCollection {
            docs: Vec::new(),
            parse_errors: vec![KnowledgeParseError {
                path: knowledge_dir.to_path_buf(),
                message: "knowledge root does not exist or is not a directory".into(),
            }],
        };
    }
    let mut files = Vec::new();
    let mut parse_errors = Vec::new();
    for kind in KINDS {
        let dir = knowledge_dir.join(kind.dir());
        collect_md(&dir, kind, &mut files, &mut parse_errors);
    }
    files.sort_by(|left, right| left.1.cmp(&right.1));
    let mut docs = Vec::new();
    for (expected_kind, path) in files {
        match parse_knowledge(&path) {
            Ok(doc) if doc.meta.kind == expected_kind => docs.push(doc),
            Ok(doc) => parse_errors.push(KnowledgeParseError {
                path,
                message: format!(
                    "knowledge kind {:?} does not match `{}/` directory",
                    doc.meta.kind,
                    expected_kind.dir()
                ),
            }),
            Err(message) => parse_errors.push(KnowledgeParseError { path, message }),
        }
    }
    docs.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
    parse_errors.sort_by(|a, b| a.path.cmp(&b.path));
    KnowledgeCollection { docs, parse_errors }
}

fn collect_md(
    dir: &Path,
    expected_kind: KnowledgeKind,
    out: &mut Vec<(KnowledgeKind, PathBuf)>,
    errors: &mut Vec<KnowledgeParseError>,
) {
    if !dir.exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        errors.push(KnowledgeParseError {
            path: dir.to_path_buf(),
            message: "knowledge directory could not be read".into(),
        });
        return;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            errors.push(KnowledgeParseError {
                path: dir.to_path_buf(),
                message: "knowledge directory entry could not be read".into(),
            });
            continue;
        };
        let p = entry.path();
        let Ok(file_type) = entry.file_type() else {
            errors.push(KnowledgeParseError {
                path: p,
                message: "knowledge entry type could not be read".into(),
            });
            continue;
        };
        if file_type.is_symlink() {
            errors.push(KnowledgeParseError {
                path: p,
                message: "symlinked knowledge entries are not allowed".into(),
            });
        } else if file_type.is_dir() {
            collect_md(&p, expected_kind, out, errors);
        } else if p.extension().and_then(|e| e.to_str()) == Some("md") {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            // README and `*-template.md` are scaffolding, not artifacts.
            if name != "README.md" && !name.ends_with("-template.md") {
                out.push((expected_kind, p));
            }
        }
    }
}

/// Per-document lint dispatched by kind.
pub fn lint_doc(doc: &KnowledgeDoc) -> Vec<LintDiagnostic> {
    match doc.meta.kind {
        KnowledgeKind::Decision => lint_decision(doc),
        KnowledgeKind::Requirement => lint_requirement(doc),
        KnowledgeKind::Guidance => lint_guidance(doc),
        KnowledgeKind::Proposal => crate::spec_knowledge::proposal::lint_proposal(doc),
    }
}

fn diag(rule: &str, severity: Severity, msg: String) -> LintDiagnostic {
    LintDiagnostic {
        rule: rule.into(),
        severity,
        message: msg,
        span: Span::default(),
        suggestion: None,
    }
}

/// Lint the whole corpus: id conflicts, supersession integrity, stale refs.
pub fn lint_corpus(docs: &[KnowledgeDoc]) -> Vec<LintDiagnostic> {
    let mut out = Vec::new();

    // id -> docs holding it (for conflict detection).
    let mut by_id: BTreeMap<&str, Vec<&KnowledgeDoc>> = BTreeMap::new();
    for d in docs {
        by_id.entry(d.meta.id.as_str()).or_default().push(d);
    }

    // §6.0 conflict: two files resolving to the same id is an error.
    for (id, holders) in &by_id {
        if holders.len() > 1 {
            out.push(diag(
                "knowledge-id-conflict",
                Severity::Error,
                format!("id {id} is declared by {} files", holders.len()),
            ));
        }
    }

    let superseded: std::collections::BTreeSet<&str> = docs
        .iter()
        .filter(|d| d.meta.status == Some(DecisionStatus::Superseded))
        .map(|d| d.meta.id.as_str())
        .collect();

    for d in docs {
        // Supersession integrity: the `supersedes` target must exist and be marked.
        if let Some(target) = &d.meta.supersedes {
            match by_id.get(target.as_str()) {
                None => out.push(diag(
                    "supersession-dangling",
                    Severity::Error,
                    format!("{} supersedes {target}, which does not exist", d.meta.id),
                )),
                Some(holders) => {
                    let marked = holders
                        .iter()
                        .any(|h| h.meta.status == Some(DecisionStatus::Superseded));
                    if !marked {
                        out.push(diag(
                            "supersession-target-not-marked",
                            Severity::Warning,
                            format!(
                                "{} supersedes {target}, but {target} is not marked `status: superseded`",
                                d.meta.id
                            ),
                        ));
                    }
                }
            }
        }

        // Stale reference: a doc body should not point at a superseded id
        // (unless it is the very doc that supersedes it).
        let own_target = d.meta.supersedes.as_deref();
        for refid in referenced_ids(d) {
            if superseded.contains(refid.as_str()) && Some(refid.as_str()) != own_target {
                out.push(diag(
                    "references-superseded",
                    Severity::Warning,
                    format!("{} references superseded id {refid}", d.meta.id),
                ));
            }
        }

        // Produces integrity: a proposal's produced ids should exist (§6.3),
        // and once the proposal is accepted the produced decision must link
        // back through its `## Source Trace` so the edge is walkable in both
        // directions.
        if d.meta.kind == KnowledgeKind::Proposal {
            for produced in crate::spec_knowledge::proposal::produces(d) {
                let Some(target) = by_id.get(produced.as_str()).and_then(|v| v.first()) else {
                    out.push(diag(
                        "produces-dangling",
                        Severity::Warning,
                        format!("{} produces {produced}, which does not exist", d.meta.id),
                    ));
                    continue;
                };
                if d.meta.status == Some(DecisionStatus::Accepted)
                    && target.meta.kind == KnowledgeKind::Decision
                {
                    let back_linked = target
                        .section("Source Trace")
                        .is_some_and(|section| section_references_id(&section.body, &d.meta.id));
                    if !back_linked {
                        out.push(diag(
                            "produces-link-integrity",
                            Severity::Warning,
                            format!(
                                "{produced} exists but its `## Source Trace` does not link back to {} (missing back-link direction: decision -> proposal)",
                                d.meta.id
                            ),
                        ));
                    }
                }
            }
        }
    }

    out
}

fn section_references_id(body: &str, id: &str) -> bool {
    body.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .any(|token| token.eq_ignore_ascii_case(id))
}

/// Scan a doc's section bodies for `LETTERS-DIGITS` id tokens (e.g. ADR-001),
/// excluding the doc's own id. De-duplicated, in first-seen order.
fn referenced_ids(doc: &KnowledgeDoc) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for section in &doc.sections {
        for token in section
            .body
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        {
            if is_id_token(token) {
                let up = token.to_ascii_uppercase();
                if up != doc.meta.id && seen.insert(up.clone()) {
                    out.push(up);
                }
            }
        }
    }
    out
}

fn is_id_token(t: &str) -> bool {
    let Some((letters, digits)) = t.split_once('-') else {
        return false;
    };
    !letters.is_empty()
        && letters.chars().all(|c| c.is_ascii_alphabetic())
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spec_knowledge::parser::parse_knowledge_str;
    use std::path::Path;

    fn parse(input: &str, name: &str) -> KnowledgeDoc {
        parse_knowledge_str(input, Path::new(name)).unwrap()
    }

    #[test]
    fn test_id_conflict_is_error() {
        let a = parse(
            "---\nkind: decision\nid: ADR-001\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "a.md",
        );
        let b = parse(
            "---\nkind: decision\nid: ADR-001\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "b.md",
        );
        let rules: Vec<_> = lint_corpus(&[a, b])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"knowledge-id-conflict".to_string()));
    }

    #[test]
    fn test_dangling_and_unmarked_supersession() {
        // ADR-002 supersedes ADR-001 which exists but is NOT marked superseded.
        let old = parse(
            "---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-001.md",
        );
        let new = parse(
            "---\nkind: decision\nid: ADR-002\nstatus: accepted\nsupersedes: ADR-001\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-002.md",
        );
        let rules: Vec<_> = lint_corpus(&[old, new])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"supersession-target-not-marked".to_string()));

        // Dangling: supersedes a non-existent id.
        let lone = parse(
            "---\nkind: decision\nid: ADR-003\nsupersedes: ADR-099\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-003.md",
        );
        let rules: Vec<_> = lint_corpus(&[lone])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"supersession-dangling".to_string()));
    }

    #[test]
    fn test_produces_dangling_warns() {
        let prop = parse(
            "---\nkind: proposal\nid: LEP-001\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Produces: ADR-404\n",
            "lep-001.md",
        );
        let rules: Vec<_> = lint_corpus(&[prop])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"produces-dangling".to_string()));
    }

    #[test]
    fn test_produces_link_integrity_names_missing_backlink() {
        let prop = parse(
            "---\nkind: proposal\nid: LEP-001\nstatus: accepted\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Produces: ADR-007\n",
            "lep-001.md",
        );
        let dec_no_backlink = parse(
            "---\nkind: decision\nid: ADR-007\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Source Trace\n- ratified in review\n",
            "adr-007.md",
        );
        let out = lint_corpus(&[prop.clone(), dec_no_backlink]);
        let hit = out
            .iter()
            .find(|d| d.rule == "produces-link-integrity")
            .unwrap_or_else(|| panic!("accepted proposal with non-backlinking decision must warn"));
        assert!(
            hit.message.contains("decision -> proposal"),
            "message names the missing direction: {}",
            hit.message
        );
        assert!(hit.message.contains("LEP-001") && hit.message.contains("ADR-007"));

        let dec_backlinked = parse(
            "---\nkind: decision\nid: ADR-007\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Source Trace\n- proposal: LEP-001\n",
            "adr-007.md",
        );
        let out = lint_corpus(&[prop, dec_backlinked]);
        assert!(
            !out.iter().any(|d| d.rule == "produces-link-integrity"),
            "backlinked decision must not warn"
        );
    }

    #[test]
    fn test_produces_link_integrity_requires_exact_backlink_id() {
        let prop = parse(
            "---\nkind: proposal\nid: LEP-001\nstatus: accepted\nliveness: n/a\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Produces: ADR-007\n",
            "lep-001.md",
        );
        let dec = parse(
            "---\nkind: decision\nid: ADR-007\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n## Source Trace\n- proposal: LEP-0010\n",
            "adr-007.md",
        );
        let out = lint_corpus(&[prop, dec]);
        assert!(
            out.iter()
                .any(|diagnostic| diagnostic.rule == "produces-link-integrity"),
            "LEP-0010 must not satisfy the exact LEP-001 backlink"
        );
    }

    #[test]
    fn test_references_superseded_warns() {
        let dead = parse(
            "---\nkind: decision\nid: ADR-001\nstatus: superseded\n---\n## Context\nc\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-001.md",
        );
        let cites = parse(
            "---\nkind: decision\nid: ADR-005\n---\n## Context\nas decided in ADR-001\n## Decision\nd\n## Consequences\ng/b\n",
            "adr-005.md",
        );
        let rules: Vec<_> = lint_corpus(&[dead, cites])
            .iter()
            .map(|d| d.rule.clone())
            .collect();
        assert!(rules.contains(&"references-superseded".to_string()));
    }

    #[test]
    fn test_collect_knowledge_checked_reports_parse_errors() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("kll-governance-{stamp}"));
        let decisions = root.join("decisions");
        std::fs::create_dir_all(&decisions).unwrap();
        std::fs::write(
            decisions.join("adr-001-bad.md"),
            "---\nkind: decision\nid: ADR-001\nliveness: forever\n---\n## Context\nc\n",
        )
        .unwrap();

        let collection = collect_knowledge_checked(&root);

        assert!(collection.docs.is_empty());
        assert_eq!(collection.parse_errors.len(), 1);
        assert_eq!(
            collection.parse_errors[0].path,
            decisions.join("adr-001-bad.md")
        );
        assert!(
            collection.parse_errors[0]
                .message
                .contains("unknown liveness")
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_collect_knowledge_checked_rejects_missing_root_and_kind_mismatch() {
        let missing =
            std::env::temp_dir().join(format!("knowledge-missing-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&missing);
        let collection = collect_knowledge_checked(&missing);
        assert!(!collection.parse_errors.is_empty());

        let root =
            std::env::temp_dir().join(format!("knowledge-kind-mismatch-{}", std::process::id()));
        let requirements = root.join("requirements");
        std::fs::create_dir_all(&requirements).unwrap();
        std::fs::write(
            requirements.join("req-wrong.md"),
            "---\nkind: decision\nid: ADR-WRONG\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood. Bad.\n",
        )
        .unwrap();

        let collection = collect_knowledge_checked(&root);
        assert!(collection.docs.is_empty());
        assert!(collection.parse_errors.iter().any(|error| {
            error.message.contains("kind") && error.message.contains("requirements")
        }));
        std::fs::remove_dir_all(root).ok();
    }
}
