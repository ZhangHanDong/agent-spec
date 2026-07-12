//! YAML dialect frontend for the intent compiler.
//!
//! Translates reference-style requirement trees (`requirements.yaml` with
//! FOLDER grouping nodes and ATOMIC leaves) into Requirement IR documents
//! under `knowledge/requirements/`. The IR and every downstream stage stay
//! frozen: generated documents flow through `lint-knowledge`, `graph`,
//! `work-units`, and `plan` unchanged.
//!
//! The accepted YAML subset is deliberately small (documented in
//! `docs/intent-compiler/yaml-frontend-v1.md`): two-space indentation,
//! scalar strings, block lists, and maps with known keys. Anything else is
//! a `yaml-unsupported-construct` diagnostic, never a partial import.

use std::path::{Path, PathBuf};

use super::intake::RequirementImportError;

pub const YAML_PROVENANCE_KEY: &str = "source: imported-yaml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedRequirementDoc {
    pub filename: String,
    pub content: String,
}

/// Parse the YAML dialect and render Requirement IR documents.
pub fn import_requirements_yaml(
    input: &str,
    source_name: &str,
) -> Result<Vec<GeneratedRequirementDoc>, RequirementImportError> {
    let root = parse_subset_yaml(input)?;
    let folders = extract_folders(&root)?;
    let node_owner = index_nodes(&folders)?;

    let mut docs = Vec::new();
    for folder in &folders {
        docs.push(GeneratedRequirementDoc {
            filename: format!("req-{}.md", folder.id),
            content: render_folder_doc(folder, &node_owner, source_name),
        });
    }
    Ok(docs)
}

/// Write generated documents under `out`, refusing to overwrite any existing
/// file that lacks the imported-yaml provenance marker. The ownership check
/// runs for every target before the first write.
pub fn write_generated_docs(
    out: &Path,
    docs: &[GeneratedRequirementDoc],
) -> Result<Vec<PathBuf>, RequirementImportError> {
    let mut planned = Vec::new();
    for doc in docs {
        let path = out.join(&doc.filename);
        if path.exists() {
            let existing = std::fs::read_to_string(&path)
                .map_err(|error| err(&format!("cannot read {}: {error}", path.display())))?;
            if !frontmatter_has_provenance(&existing) {
                return Err(err(&format!(
                    "refusing to overwrite {}: existing file lacks the `{YAML_PROVENANCE_KEY}` provenance marker",
                    doc.filename
                )));
            }
        }
        planned.push((path, doc.content.clone()));
    }
    std::fs::create_dir_all(out)
        .map_err(|error| err(&format!("cannot create {}: {error}", out.display())))?;
    let mut written = Vec::new();
    for (path, content) in planned {
        std::fs::write(&path, content)
            .map_err(|error| err(&format!("cannot write {}: {error}", path.display())))?;
        written.push(path);
    }
    Ok(written)
}

fn frontmatter_has_provenance(content: &str) -> bool {
    let Some(rest) = content.strip_prefix("---\n") else {
        return false;
    };
    let Some(end) = rest.find("\n---") else {
        return false;
    };
    rest[..end]
        .lines()
        .any(|line| line.trim() == YAML_PROVENANCE_KEY)
}

// ── YAML subset parsing ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum YamlValue {
    Scalar(String),
    List(Vec<YamlValue>),
    Map(Vec<(String, YamlValue)>),
}

impl YamlValue {
    fn get(&self, key: &str) -> Option<&YamlValue> {
        match self {
            YamlValue::Map(entries) => entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    fn keys(&self) -> Vec<&str> {
        match self {
            YamlValue::Map(entries) => entries.iter().map(|(k, _)| k.as_str()).collect(),
            _ => Vec::new(),
        }
    }
}

struct Line<'a> {
    indent: usize,
    content: &'a str,
    number: usize,
}

fn err(message: &str) -> RequirementImportError {
    RequirementImportError {
        message: message.to_string(),
    }
}

fn unsupported(line_number: usize, what: &str) -> RequirementImportError {
    err(&format!(
        "yaml-unsupported-construct: line {line_number}: {what}"
    ))
}

fn parse_subset_yaml(input: &str) -> Result<YamlValue, RequirementImportError> {
    let mut lines = Vec::new();
    for (idx, raw) in input.lines().enumerate() {
        let number = idx + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let indent_len = raw.len() - raw.trim_start_matches([' ', '\t']).len();
        if raw[..indent_len].contains('\t') {
            return Err(unsupported(number, "tab indentation"));
        }
        let content = raw[indent_len..].trim_end();
        if content.starts_with('#') {
            continue;
        }
        if content == "---" || content == "..." {
            return Err(unsupported(number, "multi-document stream separator"));
        }
        if indent_len % 2 != 0 {
            return Err(unsupported(
                number,
                "indentation is not a multiple of two spaces",
            ));
        }
        lines.push(Line {
            indent: indent_len,
            content,
            number,
        });
    }
    if lines.is_empty() {
        return Err(err("yaml source contains no content"));
    }
    let (value, next) = parse_block(&lines, 0, 0)?;
    if next != lines.len() {
        return Err(unsupported(
            lines[next].number,
            "content outside the root block",
        ));
    }
    Ok(value)
}

fn parse_block(
    lines: &[Line<'_>],
    start: usize,
    indent: usize,
) -> Result<(YamlValue, usize), RequirementImportError> {
    if lines[start].content.starts_with("- ") || lines[start].content == "-" {
        parse_list(lines, start, indent)
    } else {
        parse_map(lines, start, indent)
    }
}

fn parse_list(
    lines: &[Line<'_>],
    start: usize,
    indent: usize,
) -> Result<(YamlValue, usize), RequirementImportError> {
    let mut items = Vec::new();
    let mut i = start;
    while i < lines.len() && lines[i].indent == indent {
        let line = &lines[i];
        let Some(rest) = line
            .content
            .strip_prefix("- ")
            .or_else(|| (line.content == "-").then_some(""))
        else {
            break;
        };
        let rest = rest.trim();
        if rest.is_empty() {
            return Err(unsupported(line.number, "empty list item"));
        }
        if looks_like_map_entry(rest) {
            // list item that opens a map: `- key: value`, continuation at indent+2
            let (first_key, first_value_raw) = split_map_entry(rest, line.number)?;
            let mut entries = Vec::new();
            i += 1;
            if first_value_raw.is_empty() {
                let (child, next) = expect_child_block(lines, i, indent + 4, line.number)?;
                entries.push((first_key, child));
                i = next;
            } else {
                entries.push((
                    first_key,
                    YamlValue::Scalar(parse_scalar(first_value_raw, line.number)?),
                ));
            }
            while i < lines.len()
                && lines[i].indent == indent + 2
                && !lines[i].content.starts_with("- ")
            {
                let entry_line = &lines[i];
                let (key, value_raw) = split_map_entry(entry_line.content, entry_line.number)?;
                if entries.iter().any(|(k, _)| *k == key) {
                    return Err(err(&format!(
                        "line {}: duplicate key `{key}`",
                        entry_line.number
                    )));
                }
                i += 1;
                if value_raw.is_empty() {
                    let (child, next) =
                        expect_child_block(lines, i, indent + 4, entry_line.number)?;
                    entries.push((key, child));
                    i = next;
                } else {
                    entries.push((
                        key,
                        YamlValue::Scalar(parse_scalar(value_raw, entry_line.number)?),
                    ));
                }
            }
            items.push(YamlValue::Map(entries));
        } else {
            items.push(YamlValue::Scalar(parse_scalar(rest, line.number)?));
            i += 1;
        }
    }
    Ok((YamlValue::List(items), i))
}

fn parse_map(
    lines: &[Line<'_>],
    start: usize,
    indent: usize,
) -> Result<(YamlValue, usize), RequirementImportError> {
    let mut entries: Vec<(String, YamlValue)> = Vec::new();
    let mut i = start;
    while i < lines.len() && lines[i].indent == indent && !lines[i].content.starts_with("- ") {
        let line = &lines[i];
        let (key, value_raw) = split_map_entry(line.content, line.number)?;
        if entries.iter().any(|(k, _)| *k == key) {
            return Err(err(&format!("line {}: duplicate key `{key}`", line.number)));
        }
        i += 1;
        if value_raw.is_empty() {
            let (child, next) = expect_child_block(lines, i, indent + 2, line.number)?;
            entries.push((key, child));
            i = next;
        } else {
            entries.push((
                key,
                YamlValue::Scalar(parse_scalar(value_raw, line.number)?),
            ));
        }
    }
    if entries.is_empty() {
        return Err(unsupported(lines[start].number, "expected a map entry"));
    }
    Ok((YamlValue::Map(entries), i))
}

fn expect_child_block(
    lines: &[Line<'_>],
    i: usize,
    child_indent: usize,
    parent_line: usize,
) -> Result<(YamlValue, usize), RequirementImportError> {
    if i >= lines.len() || lines[i].indent < child_indent {
        return Err(unsupported(
            parent_line,
            "key opens a block but no indented content follows (flow style is not supported)",
        ));
    }
    if lines[i].indent > child_indent {
        return Err(unsupported(lines[i].number, "unexpected extra indentation"));
    }
    parse_block(lines, i, child_indent)
}

fn looks_like_map_entry(content: &str) -> bool {
    match content.find(':') {
        Some(pos) => {
            let after = &content[pos + 1..];
            after.is_empty() || after.starts_with(' ')
        }
        None => false,
    }
}

fn split_map_entry(
    content: &str,
    line_number: usize,
) -> Result<(String, &str), RequirementImportError> {
    let Some(pos) = content.find(':') else {
        return Err(unsupported(line_number, "expected `key:` or `key: value`"));
    };
    let key = content[..pos].trim();
    if key.is_empty()
        || key.starts_with(['"', '\'', '?', '&', '*'])
        || !key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Err(unsupported(line_number, "unsupported map key form"));
    }
    let after = &content[pos + 1..];
    if !after.is_empty() && !after.starts_with(' ') {
        return Err(unsupported(line_number, "expected a space after `:`"));
    }
    Ok((key.to_string(), after.trim()))
}

fn parse_scalar(raw: &str, line_number: usize) -> Result<String, RequirementImportError> {
    if raw.starts_with(['&', '*']) {
        return Err(unsupported(line_number, "anchors and aliases"));
    }
    if raw.starts_with(['[', '{']) {
        return Err(unsupported(line_number, "flow-style collections"));
    }
    if raw == ">" || raw == "|" || raw.starts_with("> ") || raw.starts_with("| ") {
        return Err(unsupported(line_number, "block scalars"));
    }
    if let Some(inner) = raw.strip_prefix('"') {
        let Some(inner) = inner.strip_suffix('"') else {
            return Err(unsupported(line_number, "unterminated quoted scalar"));
        };
        if inner.contains('"') || inner.contains('\\') {
            return Err(unsupported(
                line_number,
                "escape sequences in quoted scalar",
            ));
        }
        return Ok(inner.to_string());
    }
    if raw.starts_with('\'') {
        return Err(unsupported(line_number, "single-quoted scalars"));
    }
    Ok(raw.to_string())
}

// ── Mapping to the Requirement IR ───────────────────────────────────

struct FolderNode {
    id: String,
    title: String,
    status: String,
    description: Option<String>,
    dependencies: Vec<String>,
    scenarios: Vec<ScenarioNode>,
    leaves: Vec<LeafNode>,
}

struct LeafNode {
    id: String,
    statement: String,
    dependencies: Vec<String>,
    scenarios: Vec<ScenarioNode>,
}

struct ScenarioNode {
    name: String,
    given: String,
    when: String,
    then: String,
}

const FOLDER_KEYS: [&str; 8] = [
    "id",
    "title",
    "type",
    "status",
    "description",
    "dependencies",
    "scenarios",
    "children",
];
const LEAF_KEYS: [&str; 6] = [
    "id",
    "title",
    "type",
    "statement",
    "dependencies",
    "scenarios",
];
const SCENARIO_KEYS: [&str; 4] = ["name", "given", "when", "then"];

fn extract_folders(root: &YamlValue) -> Result<Vec<FolderNode>, RequirementImportError> {
    let keys = root.keys();
    if keys != vec!["requirements"] {
        return Err(err(
            "yaml root must be a single `requirements:` list of FOLDER nodes",
        ));
    }
    let Some(YamlValue::List(folders)) = root.get("requirements") else {
        return Err(err("`requirements:` must hold a block list"));
    };
    if folders.is_empty() {
        return Err(err("`requirements:` list is empty"));
    }
    folders.iter().map(extract_folder).collect()
}

fn extract_folder(value: &YamlValue) -> Result<FolderNode, RequirementImportError> {
    reject_unknown_keys(value, &FOLDER_KEYS, "FOLDER node")?;
    let id = required_scalar(value, "id", "FOLDER node")?;
    validate_node_id(&id)?;
    let node_type = required_scalar(value, "type", &format!("node `{id}`"))?;
    if node_type != "FOLDER" {
        return Err(err(&format!(
            "top-level node `{id}` must have type FOLDER (nested trees are not supported)"
        )));
    }
    let title = required_scalar(value, "title", &format!("FOLDER `{id}`"))?;
    let status = match optional_scalar(value, "status")? {
        None => "proposed".to_string(),
        Some(status) if status == "proposed" || status == "accepted" => status,
        Some(other) => {
            return Err(err(&format!(
                "FOLDER `{id}`: unknown status `{other}`; expected proposed or accepted"
            )));
        }
    };
    let description = optional_scalar(value, "description")?;
    let dependencies = match value.get("dependencies") {
        None => Vec::new(),
        Some(YamlValue::List(items)) => items
            .iter()
            .map(|item| match item {
                YamlValue::Scalar(dep) => {
                    validate_node_id(dep)?;
                    Ok(dep.clone())
                }
                _ => Err(err(&format!(
                    "FOLDER `{id}`: dependencies must be a list of node ids"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(err(&format!(
                "FOLDER `{id}`: `dependencies:` must be a block list"
            )));
        }
    };
    let scenarios = match value.get("scenarios") {
        None => Vec::new(),
        Some(YamlValue::List(items)) => items
            .iter()
            .map(|item| extract_scenario(item, &id))
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(err(&format!(
                "FOLDER `{id}`: `scenarios:` must be a block list"
            )));
        }
    };
    let Some(YamlValue::List(children)) = value.get("children") else {
        return Err(err(&format!(
            "FOLDER `{id}` must carry a non-empty `children:` block list"
        )));
    };
    if children.is_empty() {
        return Err(err(&format!("FOLDER `{id}` has no children")));
    }
    let leaves = children
        .iter()
        .map(|child| extract_leaf(child, &id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FolderNode {
        id,
        title,
        status,
        description,
        dependencies,
        scenarios,
        leaves,
    })
}

fn extract_leaf(value: &YamlValue, folder_id: &str) -> Result<LeafNode, RequirementImportError> {
    reject_unknown_keys(value, &LEAF_KEYS, &format!("child of FOLDER `{folder_id}`"))?;
    let id = required_scalar(value, "id", &format!("child of FOLDER `{folder_id}`"))?;
    validate_node_id(&id)?;
    let node_type = required_scalar(value, "type", &format!("node `{id}`"))?;
    if node_type == "FOLDER" {
        return Err(err(&format!(
            "node `{id}`: nested FOLDER nodes are not supported; only top-level folders map to documents"
        )));
    }
    if node_type != "ATOMIC" {
        return Err(err(&format!(
            "node `{id}` has unknown type `{node_type}`; expected ATOMIC"
        )));
    }
    required_scalar(value, "title", &format!("ATOMIC `{id}`"))?;
    let statement = required_scalar(value, "statement", &format!("ATOMIC `{id}`"))?;
    let dependencies = match value.get("dependencies") {
        None => Vec::new(),
        Some(YamlValue::List(items)) => items
            .iter()
            .map(|item| match item {
                YamlValue::Scalar(dep) => {
                    validate_node_id(dep)?;
                    Ok(dep.clone())
                }
                _ => Err(err(&format!(
                    "ATOMIC `{id}`: dependencies must be a list of node ids"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(err(&format!(
                "ATOMIC `{id}`: `dependencies:` must be a block list"
            )));
        }
    };
    let scenarios = match value.get("scenarios") {
        None => Vec::new(),
        Some(YamlValue::List(items)) => items
            .iter()
            .map(|item| extract_scenario(item, &id))
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(err(&format!(
                "ATOMIC `{id}`: `scenarios:` must be a block list"
            )));
        }
    };
    Ok(LeafNode {
        id,
        statement,
        dependencies,
        scenarios,
    })
}

fn extract_scenario(
    value: &YamlValue,
    leaf_id: &str,
) -> Result<ScenarioNode, RequirementImportError> {
    reject_unknown_keys(value, &SCENARIO_KEYS, &format!("scenario of `{leaf_id}`"))?;
    Ok(ScenarioNode {
        name: required_scalar(value, "name", &format!("scenario of `{leaf_id}`"))?,
        given: required_scalar(value, "given", &format!("scenario of `{leaf_id}`"))?,
        when: required_scalar(value, "when", &format!("scenario of `{leaf_id}`"))?,
        then: required_scalar(value, "then", &format!("scenario of `{leaf_id}`"))?,
    })
}

fn reject_unknown_keys(
    value: &YamlValue,
    known: &[&str],
    context: &str,
) -> Result<(), RequirementImportError> {
    let keys = value.keys();
    if keys.is_empty() {
        return Err(err(&format!("{context} must be a map")));
    }
    for key in keys {
        if !known.contains(&key) {
            return Err(err(&format!("{context}: unknown key `{key}`")));
        }
    }
    Ok(())
}

fn required_scalar(
    value: &YamlValue,
    key: &str,
    context: &str,
) -> Result<String, RequirementImportError> {
    match value.get(key) {
        Some(YamlValue::Scalar(scalar)) if !scalar.trim().is_empty() => {
            Ok(scalar.trim().to_string())
        }
        Some(_) => Err(err(&format!("{context}: `{key}` must be a scalar"))),
        None => Err(err(&format!("{context}: `{key}` is required"))),
    }
}

fn optional_scalar(value: &YamlValue, key: &str) -> Result<Option<String>, RequirementImportError> {
    match value.get(key) {
        None => Ok(None),
        Some(YamlValue::Scalar(scalar)) => Ok(Some(scalar.trim().to_string())),
        Some(_) => Err(err(&format!("`{key}` must be a scalar"))),
    }
}

fn validate_node_id(id: &str) -> Result<(), RequirementImportError> {
    let valid = !id.is_empty()
        && id.as_bytes()[0].is_ascii_lowercase()
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-');
    if !valid {
        return Err(err(&format!(
            "unsafe node id `{id}`; expected lowercase ASCII alphanumeric segments separated by single hyphens"
        )));
    }
    Ok(())
}

fn index_nodes(
    folders: &[FolderNode],
) -> Result<std::collections::BTreeMap<String, String>, RequirementImportError> {
    let mut owner = std::collections::BTreeMap::new();
    for folder in folders {
        if owner.insert(folder.id.clone(), folder.id.clone()).is_some() {
            return Err(err(&format!("duplicate node id `{}`", folder.id)));
        }
        for leaf in &folder.leaves {
            if owner.insert(leaf.id.clone(), folder.id.clone()).is_some() {
                return Err(err(&format!("duplicate node id `{}`", leaf.id)));
            }
        }
    }
    Ok(owner)
}

fn doc_id(folder_id: &str) -> String {
    format!("REQ-{}", folder_id.to_ascii_uppercase())
}

fn render_folder_doc(
    folder: &FolderNode,
    node_owner: &std::collections::BTreeMap<String, String>,
    source_name: &str,
) -> String {
    let id = doc_id(&folder.id);
    let mut deps = std::collections::BTreeSet::new();
    for dep in &folder.dependencies {
        let target_folder = node_owner.get(dep).cloned();
        match target_folder {
            Some(owner) if owner == folder.id => {}
            Some(owner) => {
                deps.insert(doc_id(&owner));
            }
            None => {
                deps.insert(doc_id(dep));
            }
        }
    }
    for leaf in &folder.leaves {
        for dep in &leaf.dependencies {
            let target_folder = node_owner.get(dep).cloned();
            match target_folder {
                Some(owner) if owner == folder.id => {}
                Some(owner) => {
                    deps.insert(doc_id(&owner));
                }
                None => {
                    deps.insert(doc_id(dep));
                }
            }
        }
    }
    deps.remove(&id);

    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("kind: requirement\n");
    out.push_str(&format!("id: {id}\n"));
    out.push_str(&format!(
        "title: \"{}\"\n",
        folder.title.replace('"', "\\\"")
    ));
    out.push_str(&format!("status: {}\n", folder.status));
    out.push_str("liveness: auto\n");
    out.push_str(&format!("{YAML_PROVENANCE_KEY}\n"));
    out.push_str("tags: [imported-yaml]\n");
    out.push_str("---\n\n");
    out.push_str(&format!("# {}\n\n", folder.title));
    out.push_str("## Problem\n\n");
    match &folder.description {
        Some(description) if !description.is_empty() => {
            out.push_str(&format!("{description}\n\n"));
        }
        _ => {
            out.push_str(&format!("Imported from `{source_name}`.\n\n"));
        }
    }
    out.push_str("## Requirements\n\n");
    for leaf in &folder.leaves {
        out.push_str(&format!(
            "[{id}-{}] {}\n\n",
            leaf.id.to_ascii_uppercase(),
            leaf.statement
        ));
    }
    // scenarios live in a dedicated section so the requirement graph
    // recognizes them and work units can become Ready
    if !folder.scenarios.is_empty() || folder.leaves.iter().any(|leaf| !leaf.scenarios.is_empty()) {
        out.push_str("## Scenarios\n\n");
        for scenario in &folder.scenarios {
            out.push_str(&format!("Scenario: {}\n", scenario.name));
            out.push_str(&format!("  Given {}\n", scenario.given));
            out.push_str(&format!("  When {}\n", scenario.when));
            out.push_str(&format!("  Then {}\n\n", scenario.then));
        }
        for leaf in &folder.leaves {
            for scenario in &leaf.scenarios {
                out.push_str(&format!("Scenario: {}\n", scenario.name));
                out.push_str(&format!("  Given {}\n", scenario.given));
                out.push_str(&format!("  When {}\n", scenario.when));
                out.push_str(&format!("  Then {}\n\n", scenario.then));
            }
        }
    }
    if !deps.is_empty() {
        out.push_str("## Dependencies\n\n");
        for dep in &deps {
            out.push_str(&format!("- {dep}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Source Trace\n\n");
    out.push_str(&format!("- imported-yaml: {source_name}\n"));
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    const FIXTURE: &str = "fixtures/requirements-yaml/requirements.yaml";

    fn fixture_input() -> String {
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE)).unwrap()
    }

    fn make_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_yaml_frontend_parses_folder_atomic_tree() {
        let docs = import_requirements_yaml(&fixture_input(), FIXTURE).unwrap();
        assert_eq!(docs.len(), 2);
        for doc in &docs {
            assert!(
                doc.content.contains(YAML_PROVENANCE_KEY),
                "{}",
                doc.filename
            );
            assert!(doc.content.contains("kind: requirement"));
            assert!(doc.content.contains("## Problem"));
            assert!(doc.content.contains("## Requirements"));
        }
        // declared status maps through; undeclared defaults to proposed
        let flights = docs
            .iter()
            .find(|d| d.filename == "req-flight-search.md")
            .unwrap();
        assert!(flights.content.contains("status: accepted"));
        let booking = docs
            .iter()
            .find(|d| d.filename == "req-booking.md")
            .unwrap();
        assert!(booking.content.contains("status: proposed"));

        let bad_status = import_requirements_yaml(
            "requirements:\n  - id: a-b\n    title: \"A\"\n    type: FOLDER\n    status: draft\n    children:\n      - id: leaf-x\n        title: \"Leaf\"\n        type: ATOMIC\n        statement: \"The system MUST hold.\"\n",
            "bad-status.yaml",
        )
        .unwrap_err();
        assert!(bad_status.to_string().contains("draft"));
    }

    #[test]
    fn test_yaml_frontend_maps_folders_to_docs_and_leaves_to_clauses() {
        let docs = import_requirements_yaml(&fixture_input(), FIXTURE).unwrap();
        let flights = docs
            .iter()
            .find(|d| d.filename == "req-flight-search.md")
            .unwrap();
        assert!(flights.content.contains("id: REQ-FLIGHT-SEARCH"));
        assert!(
            flights
                .content
                .contains("[REQ-FLIGHT-SEARCH-SEARCH-FLIGHTS] The system MUST")
        );
        // scenarios live in a dedicated ## Scenarios section so the
        // requirement graph recognizes them (work units need scenarios
        // to become Ready)
        let scenarios_section = flights.content.split("## Scenarios").nth(1).unwrap();
        assert!(scenarios_section.contains("Scenario: Empty search results"));
        assert!(
            flights.content.find("## Requirements").unwrap()
                < flights.content.find("## Scenarios").unwrap()
        );
        assert!(flights.content.contains("Scenario: Empty search results"));
        assert!(
            flights
                .content
                .contains("  Given no flight matches the query")
        );
        assert!(flights.content.contains("  When the visitor searches"));
        assert!(
            flights
                .content
                .contains("  Then the system returns an empty result page")
        );

        let booking = docs
            .iter()
            .find(|d| d.filename == "req-booking.md")
            .unwrap();
        assert!(booking.content.contains("id: REQ-BOOKING"));
        assert!(booking.content.contains("[REQ-BOOKING-CREATE-BOOKING]"));
        assert!(booking.content.contains("[REQ-BOOKING-CANCEL-BOOKING]"));
    }

    #[test]
    fn test_yaml_frontend_maps_dependencies_into_graph() {
        let dir = make_temp_dir("yaml-frontend-graph-ok");
        let requirements_dir = dir.join("requirements");
        let docs = import_requirements_yaml(&fixture_input(), FIXTURE).unwrap();
        let booking = docs
            .iter()
            .find(|d| d.filename == "req-booking.md")
            .unwrap();
        let deps_section = booking.content.split("## Dependencies").nth(1).unwrap();
        assert!(deps_section.contains("- REQ-FLIGHT-SEARCH"));
        // same-folder dependencies never become doc-level edges
        assert!(!deps_section.contains("- REQ-BOOKING"));

        write_generated_docs(&requirements_dir, &docs).unwrap();
        let graph = crate::spec_knowledge::build_requirement_graph(&dir);
        let mut diagnostics = graph.diagnostics.clone();
        diagnostics.extend(crate::spec_knowledge::validate_requirement_graph(&graph));
        assert!(
            diagnostics.iter().all(|d| d.severity != "error"),
            "imported corpus must pass the existing requirement graph: {diagnostics:?}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_frontend_import_then_graph_reports_dangling_dependency() {
        let dir = make_temp_dir("yaml-frontend-graph-dangling");
        let requirements_dir = dir.join("requirements");
        let input = r#"requirements:
  - id: alpha
    title: "Alpha"
    type: FOLDER
    description: "Alpha folder."
    children:
      - id: leaf-one
        title: "Leaf one"
        type: ATOMIC
        statement: "The system MUST do the first thing."
        dependencies:
          - missing-node
"#;
        let docs = import_requirements_yaml(input, "dangling.yaml").unwrap();
        let alpha = &docs[0];
        assert!(alpha.content.contains("- REQ-MISSING-NODE"));

        write_generated_docs(&requirements_dir, &docs).unwrap();
        let graph = crate::spec_knowledge::build_requirement_graph(&dir);
        let mut diagnostics = graph.diagnostics.clone();
        diagnostics.extend(crate::spec_knowledge::validate_requirement_graph(&graph));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.severity == "error" && d.message.contains("REQ-MISSING-NODE")),
            "dangling dependency must be caught by the existing graph gate: {diagnostics:?}"
        );
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_frontend_rejects_unsupported_yaml() {
        for (label, input) in [
            (
                "anchor",
                "requirements:\n  - id: a\n    title: &t \"A\"\n    type: FOLDER\n",
            ),
            (
                "alias",
                "requirements:\n  - id: a\n    title: *t\n    type: FOLDER\n",
            ),
            (
                "flow-list",
                "requirements:\n  - id: a\n    title: \"A\"\n    type: FOLDER\n    children: []\n",
            ),
            ("flow-map", "requirements: {id: a}\n"),
            ("multi-doc", "---\nrequirements:\n  - id: a\n---\n"),
            ("tab-indent", "requirements:\n\t- id: a\n"),
        ] {
            let err = import_requirements_yaml(input, "bad.yaml").unwrap_err();
            assert!(
                err.to_string().contains("yaml-unsupported-construct"),
                "{label}: {err}"
            );
        }
    }

    #[test]
    fn test_yaml_frontend_rejects_unsafe_node_ids() {
        let input = r#"requirements:
  - id: ../../escape
    title: "Escape"
    type: FOLDER
    children:
      - id: leaf
        title: "Leaf"
        type: ATOMIC
        statement: "The system MUST stay inside the sandbox."
"#;
        let err = import_requirements_yaml(input, "unsafe.yaml").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("../../escape"), "{message}");
        // rejection, not a silent rename
        assert!(!message.contains("REQ-ESCAPE"), "{message}");
    }

    #[test]
    fn test_yaml_frontend_reimport_is_idempotent() {
        let dir = make_temp_dir("yaml-frontend-idempotent");
        let docs = import_requirements_yaml(&fixture_input(), FIXTURE).unwrap();
        let first = write_generated_docs(&dir, &docs).unwrap();
        let snapshot: Vec<(PathBuf, String)> = first
            .iter()
            .map(|p| (p.clone(), fs::read_to_string(p).unwrap()))
            .collect();

        let docs_again = import_requirements_yaml(&fixture_input(), FIXTURE).unwrap();
        let second = write_generated_docs(&dir, &docs_again).unwrap();
        assert_eq!(first, second);
        for (path, content) in snapshot {
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                content,
                "re-import must be byte-identical: {}",
                path.display()
            );
        }
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn test_yaml_frontend_refuses_overwriting_unmarked_files() {
        let dir = make_temp_dir("yaml-frontend-ownership");
        let docs = import_requirements_yaml(&fixture_input(), FIXTURE).unwrap();
        let human = dir.join(&docs[0].filename);
        let human_content = "---\nkind: requirement\nid: REQ-HUMAN\ntitle: \"Hand-written\"\nliveness: auto\ntags: []\n---\n\n## Problem\n\np\n\n## Requirements\n\n[REQ-HUMAN] The system MUST respect human-authored files.\n";
        fs::write(&human, human_content).unwrap();

        let err = write_generated_docs(&dir, &docs).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains(&docs[0].filename),
            "ownership diagnostic must name the file: {message}"
        );
        assert_eq!(
            fs::read_to_string(&human).unwrap(),
            human_content,
            "human-authored file must be untouched"
        );
        // all-or-nothing: no sibling file may have been written either
        assert!(!dir.join(&docs[1].filename).exists());
        fs::remove_dir_all(dir).ok();
    }
}
