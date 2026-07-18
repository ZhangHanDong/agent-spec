//! YAML export projection for the intent compiler.
//!
//! Confirmed requirement documents are hand-owned canonical IR; this module
//! projects them into the constrained `requirements.yaml` dialect that the
//! YAML frontend imports. The exported file is a derived projection — never a
//! source of truth — and the round-trip law anchors correctness:
//! `export → import → export` is byte-identical.

use std::path::Path;

use super::intake::RequirementImportError;

#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// When non-empty, restrict export to these requirement ids.
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportOutcome {
    pub yaml: String,
    /// Requirement ids excluded by governance status, with the status.
    pub excluded: Vec<String>,
    /// Content the dialect cannot carry (reported, never silently dropped).
    pub lossy: Vec<String>,
}

/// Render requirement documents under `knowledge_dir` into the YAML dialect.
pub fn export_requirements_yaml(
    knowledge_dir: &Path,
    opts: &ExportOptions,
) -> Result<ExportOutcome, RequirementImportError> {
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge_dir);
    let mut by_id: std::collections::BTreeMap<String, &crate::spec_knowledge::RequirementNode> =
        graph.nodes.iter().map(|n| (n.id.clone(), n)).collect();

    if !opts.ids.is_empty() {
        let wanted: std::collections::BTreeSet<String> = opts
            .ids
            .iter()
            .map(|i| i.trim().to_ascii_uppercase())
            .collect();
        for id in &wanted {
            if !by_id.contains_key(id) {
                return Err(err(format!("unknown requirement id for export: {id}")));
            }
        }
        by_id.retain(|id, _| wanted.contains(id));
    }

    let mut excluded = Vec::new();
    let mut lossy = Vec::new();
    let mut out = String::from("requirements:\n");
    let mut wrote_any = false;

    for (id, node) in &by_id {
        let status = match node.status {
            Some(crate::spec_knowledge::DecisionStatus::Proposed) => "proposed",
            Some(crate::spec_knowledge::DecisionStatus::Accepted) => "accepted",
            Some(crate::spec_knowledge::DecisionStatus::Superseded) => {
                excluded.push(format!("{id} (superseded)"));
                continue;
            }
            Some(crate::spec_knowledge::DecisionStatus::Deprecated) => {
                excluded.push(format!("{id} (deprecated)"));
                continue;
            }
            Some(crate::spec_knowledge::DecisionStatus::Rejected) => {
                excluded.push(format!("{id} (rejected)"));
                continue;
            }
            None => {
                excluded.push(format!("{id} (missing status)"));
                continue;
            }
        };

        let slug = id
            .strip_prefix("REQ-")
            .ok_or_else(|| err(format!("{id}: exportable ids must start with REQ-")))?
            .to_ascii_lowercase();
        let title = expressible(&node.title, id)?;
        let description = expressible(&reflow(&node.problem), id)?;

        out.push_str(&format!("  - id: {slug}\n"));
        out.push_str(&format!("    title: \"{title}\"\n"));
        out.push_str("    type: FOLDER\n");
        out.push_str(&format!("    status: {status}\n"));
        if !description.is_empty() {
            out.push_str(&format!("    description: \"{description}\"\n"));
        }

        let mut deps: Vec<String> = node
            .dependencies
            .iter()
            .map(|d| d.strip_prefix("REQ-").unwrap_or(d).to_ascii_lowercase())
            .collect();
        deps.sort();
        deps.dedup();
        if !deps.is_empty() {
            out.push_str("    dependencies:\n");
            for dep in deps {
                out.push_str(&format!("      - {dep}\n"));
            }
        }

        if !node.scenarios.is_empty() {
            out.push_str("    scenarios:\n");
            for scenario in &node.scenarios {
                let (given, when, then) = scenario_buckets(scenario, id)?;
                out.push_str(&format!(
                    "      - name: \"{}\"\n",
                    expressible(&scenario.name, id)?
                ));
                out.push_str(&format!(
                    "        given: \"{}\"\n",
                    expressible(&given, id)?
                ));
                out.push_str(&format!("        when: \"{}\"\n", expressible(&when, id)?));
                out.push_str(&format!("        then: \"{}\"\n", expressible(&then, id)?));
            }
        }

        if node.clauses.is_empty() {
            return Err(err(format!("{id}: document has no clauses to export")));
        }
        out.push_str("    children:\n");
        let clause_prefix = format!("{id}-");
        for clause in &node.clauses {
            let clause_id = clause
                .id
                .as_deref()
                .ok_or_else(|| err(format!("{id}: a clause without an id cannot be exported")))?;
            let suffix = clause_id.strip_prefix(&clause_prefix).ok_or_else(|| {
                err(format!(
                    "{clause_id}: clause id does not extend the document id {id}"
                ))
            })?;
            let leaf_id = suffix.to_ascii_lowercase();
            out.push_str(&format!("      - id: {leaf_id}\n"));
            out.push_str(&format!("        title: \"{}\"\n", humanize(&leaf_id)));
            out.push_str("        type: ATOMIC\n");
            out.push_str(&format!(
                "        statement: \"{}\"\n",
                expressible(&clause.text, clause_id)?
            ));
        }
        wrote_any = true;

        // lossiness report: dialect cannot carry these document parts
        if !node.tags.is_empty() {
            lossy.push(format!("{id}: tags are not carried by the dialect"));
        }
        if !node.source_trace.is_empty() {
            lossy.push(format!("{id}: Source Trace is not carried by the dialect"));
        }
        if !node.open_questions.is_empty() {
            lossy.push(format!(
                "{id}: Open Questions are not carried by the dialect"
            ));
        }
    }

    if !wrote_any {
        return Err(err("no exportable requirement documents".to_string()));
    }
    Ok(ExportOutcome {
        yaml: out,
        excluded,
        lossy,
    })
}

/// Export and write to `path` (a `.yaml`/`.yml` target). Nothing is written
/// when rendering fails. With `check`, compare against the existing file and
/// error on drift instead of writing.
pub fn write_export(
    knowledge_dir: &Path,
    path: &Path,
    opts: &ExportOptions,
    check: bool,
) -> Result<ExportOutcome, RequirementImportError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("yaml") | Some("yml")) {
        return Err(err(format!(
            "export target must end in .yaml or .yml: {}",
            path.display()
        )));
    }
    let outcome = export_requirements_yaml(knowledge_dir, opts)?;
    if check {
        let actual = std::fs::read_to_string(path).unwrap_or_default();
        if actual != outcome.yaml {
            return Err(err(format!(
                "exported projection drifted: {}",
                path.display()
            )));
        }
        return Ok(outcome);
    }
    std::fs::write(path, &outcome.yaml)
        .map_err(|e| err(format!("cannot write {}: {e}", path.display())))?;
    Ok(outcome)
}

// ── ARC-native export projection ────────────────────────────────────

/// Render confirmed requirement documents as a single-root ARC-native tree
/// that the reference loader consumes directly (`yaml.safe_load` + non-empty
/// root id). Dotted source ids recorded at import time are restored; the
/// projection is derived and obeys the same round-trip law as the v1.1
/// dialect: export → import → export is byte-identical.
pub fn export_requirements_arc_native(
    knowledge_dir: &Path,
    root_name: &str,
    opts: &ExportOptions,
) -> Result<ExportOutcome, RequirementImportError> {
    let graph = crate::spec_knowledge::build_requirement_graph(knowledge_dir);
    let mut by_id: std::collections::BTreeMap<String, &crate::spec_knowledge::RequirementNode> =
        graph.nodes.iter().map(|n| (n.id.clone(), n)).collect();

    if !opts.ids.is_empty() {
        let wanted: std::collections::BTreeSet<String> = opts
            .ids
            .iter()
            .map(|i| i.trim().to_ascii_uppercase())
            .collect();
        for id in &wanted {
            if !by_id.contains_key(id) {
                return Err(err(format!("unknown requirement id for export: {id}")));
            }
        }
        by_id.retain(|id, _| wanted.contains(id));
    }

    let mut excluded = Vec::new();
    let mut lossy = Vec::new();
    let mut out = String::new();
    out.push_str("id: ROOT\n");
    out.push_str(&format!("name: \"{}\"\n", expressible(root_name, "ROOT")?));
    out.push_str("type: FOLDER\n");
    out.push_str("dependencies: []\n");
    out.push_str("children:\n");
    let mut wrote_any = false;

    for (id, node) in &by_id {
        match node.status {
            Some(crate::spec_knowledge::DecisionStatus::Proposed)
            | Some(crate::spec_knowledge::DecisionStatus::Accepted) => {}
            Some(crate::spec_knowledge::DecisionStatus::Superseded) => {
                excluded.push(format!("{id} (superseded)"));
                continue;
            }
            Some(crate::spec_knowledge::DecisionStatus::Deprecated) => {
                excluded.push(format!("{id} (deprecated)"));
                continue;
            }
            Some(crate::spec_knowledge::DecisionStatus::Rejected) => {
                excluded.push(format!("{id} (rejected)"));
                continue;
            }
            None => {
                excluded.push(format!("{id} (missing status)"));
                continue;
            }
        }

        // Source-id fidelity: scan the raw document once for the frontmatter
        // `source-id:` line and per-clause `<!-- source-id: ... -->` comments.
        let raw = std::fs::read_to_string(&node.source_path)
            .map_err(|e| err(format!("cannot read {}: {e}", node.source_path.display())))?;
        let doc_source_id = frontmatter_lines(&raw)
            .into_iter()
            .find_map(|line| line.strip_prefix("source-id: ").map(str::to_string));
        let clause_source_ids = clause_source_id_map(&raw);

        let folder_id = doc_source_id.clone().unwrap_or_else(|| id.to_string());
        out.push_str(&format!("  - id: {folder_id}\n"));
        out.push_str(&format!(
            "    name: \"{}\"\n",
            expressible(&node.title, id)?
        ));
        out.push_str("    type: FOLDER\n");
        let description = expressible(&reflow(&node.problem), id)?;
        if !description.is_empty() {
            out.push_str(&format!("    description: \"{description}\"\n"));
        }

        let mut deps: Vec<String> = node.dependencies.clone();
        deps.sort();
        deps.dedup();
        if deps.is_empty() {
            out.push_str("    dependencies: []\n");
        } else {
            out.push_str("    dependencies:\n");
            for dep in deps {
                out.push_str(&format!("      - {dep}\n"));
            }
        }

        if node.clauses.is_empty() {
            return Err(err(format!("{id}: document has no clauses to export")));
        }
        out.push_str("    children:\n");
        for clause in &node.clauses {
            let clause_id = clause
                .id
                .as_deref()
                .ok_or_else(|| err(format!("{id}: a clause without an id cannot be exported")))?;
            let atomic_id = clause_source_ids
                .get(clause_id)
                .cloned()
                .unwrap_or_else(|| clause_id.to_string());
            let suffix = clause_id
                .strip_prefix(&format!("{id}-"))
                .unwrap_or(clause_id)
                .to_ascii_lowercase();
            out.push_str(&format!("      - id: {atomic_id}\n"));
            out.push_str(&format!("        name: \"{}\"\n", humanize(&suffix)));
            out.push_str("        type: ATOMIC\n");
            out.push_str(&format!(
                "        description: \"{}\"\n",
                expressible(&clause.text, clause_id)?
            ));
            out.push_str("        dependencies: []\n");
        }

        if !node.scenarios.is_empty() {
            out.push_str("    scenarios:\n");
            for scenario in &node.scenarios {
                out.push_str(&format!(
                    "      - name: \"{}\"\n",
                    expressible(&scenario.name, id)?
                ));
                out.push_str("        steps:\n");
                for step in &scenario.steps {
                    out.push_str(&format!(
                        "          - keyword: {}\n",
                        step.keyword.to_ascii_uppercase()
                    ));
                    out.push_str(&format!(
                        "            content: \"{}\"\n",
                        expressible(&step.content, id)?
                    ));
                }
            }
        }
        wrote_any = true;

        if !node.tags.is_empty() {
            lossy.push(format!("{id}: tags are not carried by the ARC-native tree"));
        }
        if !node.source_trace.is_empty() {
            lossy.push(format!(
                "{id}: Source Trace is not carried by the ARC-native tree"
            ));
        }
        if !node.open_questions.is_empty() {
            lossy.push(format!(
                "{id}: Open Questions are not carried by the ARC-native tree"
            ));
        }
        if node.status == Some(crate::spec_knowledge::DecisionStatus::Accepted) {
            lossy.push(format!(
                "{id}: governance status is not carried by the ARC-native tree (re-import yields proposed)"
            ));
        }
    }

    if !wrote_any {
        return Err(err("no exportable requirement documents".to_string()));
    }
    Ok(ExportOutcome {
        yaml: out,
        excluded,
        lossy,
    })
}

/// Lines strictly between the first and second `---` frontmatter fences.
fn frontmatter_lines(raw: &str) -> Vec<&str> {
    let mut lines = raw.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    lines
        .map(str::trim)
        .take_while(|line| *line != "---")
        .collect()
}

/// Map clause ids to their recorded `<!-- source-id: ... -->` comments.
fn clause_source_id_map(raw: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    let mut last_clause: Option<String> = None;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                last_clause = Some(rest[..end].to_string());
            }
        } else if let Some(comment) = trimmed
            .strip_prefix("<!-- source-id: ")
            .and_then(|rest| rest.strip_suffix(" -->"))
            && let Some(clause) = last_clause.take()
        {
            map.insert(clause, comment.trim().to_string());
        }
    }
    map
}

/// Export the ARC-native projection to `path`, honoring the same `.yaml`
/// target and `--check` drift semantics as the v1.1 exporter.
pub fn write_arc_native_export(
    knowledge_dir: &Path,
    path: &Path,
    root_name: &str,
    opts: &ExportOptions,
    check: bool,
) -> Result<ExportOutcome, RequirementImportError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("yaml") | Some("yml")) {
        return Err(err(format!(
            "export target must end in .yaml or .yml: {}",
            path.display()
        )));
    }
    let outcome = export_requirements_arc_native(knowledge_dir, root_name, opts)?;
    if check {
        let actual = std::fs::read_to_string(path).unwrap_or_default();
        if actual != outcome.yaml {
            return Err(err(format!(
                "exported projection drifted: {}",
                path.display()
            )));
        }
        return Ok(outcome);
    }
    std::fs::write(path, &outcome.yaml)
        .map_err(|e| err(format!("cannot write {}: {e}", path.display())))?;
    Ok(outcome)
}

fn err(message: String) -> RequirementImportError {
    RequirementImportError { message }
}

/// Single-line reflow: paragraphs and line breaks collapse to single spaces.
fn reflow(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The dialect's double-quoted scalars carry no escapes.
fn expressible(text: &str, owner: &str) -> Result<String, RequirementImportError> {
    let text = text.trim();
    if text.contains('"') || text.contains('\\') {
        return Err(err(format!(
            "{owner}: content contains characters the dialect cannot carry (double quote or backslash)"
        )));
    }
    Ok(text.to_string())
}

fn humanize(leaf_id: &str) -> String {
    let words = leaf_id.replace('-', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => words,
    }
}

/// Fold scenario steps into the dialect's single given/when/then scalars.
fn scenario_buckets(
    scenario: &crate::spec_knowledge::RequirementScenario,
    owner: &str,
) -> Result<(String, String, String), RequirementImportError> {
    let mut given: Vec<String> = Vec::new();
    let mut when: Vec<String> = Vec::new();
    let mut then: Vec<String> = Vec::new();
    let mut current: Option<u8> = None;
    for step in &scenario.steps {
        let keyword = step.keyword.trim();
        let bucket = match keyword {
            "Given" | "假设" => Some(0u8),
            "When" | "当" => Some(1),
            "Then" | "那么" => Some(2),
            "And" | "But" | "并且" | "但是" => current,
            _ => current,
        };
        let Some(bucket) = bucket else {
            return Err(err(format!(
                "{owner}: scenario `{}` starts with an unmappable step keyword `{keyword}`",
                scenario.name
            )));
        };
        current = Some(bucket);
        let target = match bucket {
            0 => &mut given,
            1 => &mut when,
            _ => &mut then,
        };
        target.push(step.content.trim().to_string());
    }
    if given.is_empty() || when.is_empty() || then.is_empty() {
        return Err(err(format!(
            "{owner}: scenario `{}` needs given, when, and then steps for the dialect",
            scenario.name
        )));
    }
    Ok((given.join("; "), when.join("; "), then.join("; ")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn make_knowledge(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("requirements")).unwrap();
        dir
    }

    fn write_alpha(dir: &Path) {
        fs::write(
            dir.join("requirements/req-alpha.md"),
            "---\nkind: requirement\nid: REQ-ALPHA\ntitle: \"Alpha Flow\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Alpha Flow\n\n## Problem\n\nAlpha needs a flow.\n\n## Requirements\n\n[REQ-ALPHA-FIRST-STEP] The system MUST run the first step.\n\n[REQ-ALPHA-SECOND-STEP] The system MUST run the second step.\n\n## Scenarios\n\nScenario: First step works\n  Given a clean state\n  When the first step runs\n  Then the state is advanced\n\n## Dependencies\n\n- REQ-BETA\n\n## Source Trace\n\n- example:alpha\n",
        )
        .unwrap();
    }

    fn write_beta(dir: &Path) {
        fs::write(
            dir.join("requirements/req-beta.md"),
            "---\nkind: requirement\nid: REQ-BETA\ntitle: \"Beta Base\"\nstatus: proposed\nliveness: auto\ntags: []\n---\n\n# Beta Base\n\n## Problem\n\nBeta is the base.\n\n## Requirements\n\n[REQ-BETA-ONLY] The system MUST provide the base.\n",
        )
        .unwrap();
    }

    fn write_gamma_superseded(dir: &Path) {
        fs::write(
            dir.join("requirements/req-gamma.md"),
            "---\nkind: requirement\nid: REQ-GAMMA\ntitle: \"Gamma Old\"\nstatus: superseded\nliveness: auto\ntags: []\n---\n\n# Gamma Old\n\n## Problem\n\nOld.\n\n## Requirements\n\n[REQ-GAMMA-OLD] The system MUST stay historical.\n",
        )
        .unwrap();
    }

    #[test]
    fn test_yaml_export_renders_corpus_tree() {
        let dir = make_knowledge("yaml-export-corpus");
        write_alpha(&dir);
        write_beta(&dir);
        write_gamma_superseded(&dir);

        let outcome = export_requirements_yaml(&dir, &ExportOptions::default()).unwrap();
        let yaml = &outcome.yaml;
        assert!(yaml.contains("- id: alpha\n"), "{yaml}");
        assert!(yaml.contains("title: \"Alpha Flow\""));
        assert!(yaml.contains("status: accepted"));
        assert!(yaml.contains("status: proposed"));
        assert!(yaml.contains("- id: first-step"));
        assert!(
            yaml.contains("title: \"First step\""),
            "synthesized leaf title: {yaml}"
        );
        assert!(yaml.contains("statement: \"The system MUST run the first step.\""));
        // folder-level dependencies and scenarios
        let alpha_block =
            &yaml[yaml.find("- id: alpha").unwrap()..yaml.find("- id: beta").unwrap()];
        assert!(
            alpha_block.contains("dependencies:\n      - beta"),
            "{alpha_block}"
        );
        assert!(alpha_block.contains("scenarios:"));
        assert!(alpha_block.contains("name: \"First step works\""));
        assert!(alpha_block.contains("given: \"a clean state\""));
        // superseded is excluded, not silently dropped
        assert!(!yaml.contains("gamma"));
        assert!(outcome.excluded.iter().any(|e| e.contains("REQ-GAMMA")));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_export_id_filter_restricts_output() {
        let dir = make_knowledge("yaml-export-filter");
        write_alpha(&dir);
        write_beta(&dir);
        fs::write(
            dir.join("requirements/req-delta.md"),
            "---\nkind: requirement\nid: REQ-DELTA\ntitle: \"Delta\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Delta\n\n## Problem\n\nd\n\n## Requirements\n\n[REQ-DELTA-ONE] The system MUST do delta.\n",
        )
        .unwrap();

        let outcome = export_requirements_yaml(
            &dir,
            &ExportOptions {
                ids: vec!["REQ-ALPHA".into(), "REQ-BETA".into()],
            },
        )
        .unwrap();
        assert!(outcome.yaml.contains("- id: alpha"));
        assert!(outcome.yaml.contains("- id: beta"));
        assert!(!outcome.yaml.contains("delta"));

        let err = export_requirements_yaml(
            &dir,
            &ExportOptions {
                ids: vec!["REQ-GHOST".into()],
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("REQ-GHOST"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_export_import_roundtrip_fixpoint() {
        let dir = make_knowledge("yaml-export-fixpoint");
        write_alpha(&dir);
        write_beta(&dir);

        let first = export_requirements_yaml(&dir, &ExportOptions::default()).unwrap();
        let docs =
            crate::spec_knowledge::import_requirements_yaml(&first.yaml, "roundtrip.yaml").unwrap();
        let reimported = make_knowledge("yaml-export-fixpoint-2");
        crate::spec_knowledge::write_generated_docs(&reimported.join("requirements"), &docs)
            .unwrap();
        let second = export_requirements_yaml(&reimported, &ExportOptions::default()).unwrap();
        assert_eq!(
            first.yaml, second.yaml,
            "export -> import -> export must be a fixpoint"
        );
        fs::remove_dir_all(dir).ok();
        fs::remove_dir_all(reimported).ok();
    }

    #[test]
    fn test_yaml_frontend_accepts_folder_dependencies_and_scenarios() {
        let input = r#"requirements:
  - id: alpha
    title: "Alpha"
    type: FOLDER
    status: accepted
    description: "Alpha folder."
    dependencies:
      - beta
    scenarios:
      - name: "Folder level works"
        given: "a folder scenario"
        when: "it is imported"
        then: "it lands in the Scenarios section"
    children:
      - id: one
        title: "One"
        type: ATOMIC
        statement: "The system MUST do one."
"#;
        let docs =
            crate::spec_knowledge::import_requirements_yaml(input, "folder-level.yaml").unwrap();
        let alpha = &docs[0];
        let deps = alpha.content.split("## Dependencies").nth(1).unwrap();
        assert!(deps.contains("- REQ-BETA"), "{}", alpha.content);
        let scenarios = alpha.content.split("## Scenarios").nth(1).unwrap();
        assert!(scenarios.contains("Scenario: Folder level works"));
        assert!(scenarios.contains("  Given a folder scenario"));
        fs::remove_dir_all(std::env::temp_dir().join("nonexistent")).ok();
    }

    #[test]
    fn test_yaml_export_rejects_nonconforming_clause_ids() {
        let dir = make_knowledge("yaml-export-nonconforming");
        fs::write(
            dir.join("requirements/req-alpha.md"),
            "---\nkind: requirement\nid: REQ-ALPHA\ntitle: \"Alpha\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Alpha\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-OTHER-THING] The system MUST not fit this document.\n",
        )
        .unwrap();
        let target = dir.join("requirements.yaml");
        let err = write_export(&dir, &target, &ExportOptions::default(), false).unwrap_err();
        assert!(err.to_string().contains("REQ-OTHER-THING"), "{err}");
        assert!(!target.exists(), "no file may be written on failure");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_export_rejects_inexpressible_scalars() {
        let dir = make_knowledge("yaml-export-inexpressible");
        fs::write(
            dir.join("requirements/req-alpha.md"),
            "---\nkind: requirement\nid: REQ-ALPHA\ntitle: \"Alpha\"\nstatus: accepted\nliveness: auto\ntags: []\n---\n\n# Alpha\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-ALPHA-QUOTE] The system MUST reject a \"quoted\" statement.\n",
        )
        .unwrap();
        let target = dir.join("requirements.yaml");
        let err = write_export(&dir, &target, &ExportOptions::default(), false).unwrap_err();
        assert!(err.to_string().contains("REQ-ALPHA"), "{err}");
        assert!(!target.exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_export_reports_excluded_statuses() {
        let dir = make_knowledge("yaml-export-excluded");
        write_alpha(&dir);
        write_beta(&dir);
        write_gamma_superseded(&dir);
        let outcome = export_requirements_yaml(&dir, &ExportOptions::default()).unwrap();
        assert!(!outcome.yaml.contains("Gamma"));
        assert!(
            outcome
                .excluded
                .iter()
                .any(|e| e.contains("REQ-GAMMA") && e.contains("superseded")),
            "{:?}",
            outcome.excluded
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_export_check_detects_drift() {
        let dir = make_knowledge("yaml-export-drift");
        write_alpha(&dir);
        write_beta(&dir);
        let target = dir.join("requirements.yaml");
        write_export(&dir, &target, &ExportOptions::default(), false).unwrap();
        // fresh check passes
        write_export(&dir, &target, &ExportOptions::default(), true).unwrap();
        // manual edit drifts the projection
        let mut text = fs::read_to_string(&target).unwrap();
        text.push_str("# manual note\n");
        fs::write(&target, text).unwrap();
        let err = write_export(&dir, &target, &ExportOptions::default(), true).unwrap_err();
        assert!(
            err.to_string().contains("requirements.yaml"),
            "drift must name the target: {err}"
        );
        fs::remove_dir_all(dir).ok();
    }
}
