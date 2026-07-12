use crate::spec_wiki::sources::{RepoPathIssue, repo_path_issue};
use crate::spec_wiki::{WikiDiagnostic, path_to_slash};
use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiProjectMap {
    pub version: u32,
    pub projects: Vec<WikiExternalProject>,
    pub flows: Vec<WikiProjectFlow>,
    pub edges: Vec<WikiProjectEdge>,
    #[serde(serialize_with = "serialize_diagnostics")]
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiExternalProject {
    pub id: String,
    pub title: String,
    pub repo: String,
    pub role: String,
    pub interfaces: Vec<String>,
    pub protocols: Vec<String>,
    pub status: String,
    #[serde(serialize_with = "serialize_paths")]
    pub source_files: Vec<PathBuf>,
    pub external_sources: Vec<String>,
    #[serde(serialize_with = "serialize_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiProjectFlow {
    pub id: String,
    pub title: String,
    pub projects: Vec<String>,
    pub kind: String,
    pub protocols: Vec<String>,
    pub requirements: Vec<String>,
    #[serde(serialize_with = "serialize_paths")]
    pub specs: Vec<PathBuf>,
    #[serde(serialize_with = "serialize_paths")]
    pub source_files: Vec<PathBuf>,
    pub external_sources: Vec<String>,
    #[serde(serialize_with = "serialize_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiProjectEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub flow_id: String,
    pub protocols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiProjectInspectReport {
    pub project_id: String,
    pub project: Option<WikiExternalProject>,
    pub flows: Vec<WikiProjectFlow>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

pub fn build_project_map(root: &Path, wiki_dir: &Path) -> WikiProjectMap {
    let mut diagnostics = Vec::new();
    let mut projects = read_project_articles(root, wiki_dir, &mut diagnostics);
    let mut flows = read_flow_articles(root, wiki_dir, &mut diagnostics);

    projects.sort_by(|left, right| left.id.cmp(&right.id).then(left.path.cmp(&right.path)));
    flows.sort_by(|left, right| left.id.cmp(&right.id).then(left.path.cmp(&right.path)));

    validate_project_map(root, &projects, &flows, &mut diagnostics);
    let edges = derive_edges(&flows);

    diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.code.cmp(&right.code))
            .then(left.message.cmp(&right.message))
    });

    WikiProjectMap {
        version: 1,
        projects,
        flows,
        edges,
        diagnostics,
    }
}

pub fn render_project_map_mermaid(map: &WikiProjectMap) -> String {
    let mut out = String::from("flowchart LR\n");
    for project in &map.projects {
        out.push_str(&format!(
            "  {}[\"{}\"]\n",
            mermaid_id(&project.id),
            mermaid_label(&project.id)
        ));
    }
    for edge in &map.edges {
        out.push_str(&format!(
            "  {} -->|{}| {}\n",
            mermaid_id(&edge.from),
            mermaid_label(&edge.kind),
            mermaid_id(&edge.to)
        ));
    }
    if map.projects.is_empty() {
        out.push_str("  none[\"No project articles\"]\n");
    }
    out
}

pub fn inspect_wiki_project(
    root: &Path,
    wiki_dir: &Path,
    project_id: &str,
) -> WikiProjectInspectReport {
    let map = build_project_map(root, wiki_dir);
    let project_id = project_id.to_ascii_lowercase();
    let project = map
        .projects
        .iter()
        .find(|project| project.id == project_id)
        .cloned();
    let flows = map
        .flows
        .iter()
        .filter(|flow| flow.projects.iter().any(|project| project == &project_id))
        .cloned()
        .collect::<Vec<_>>();
    let mut diagnostics = map.diagnostics;
    if project.is_none() {
        diagnostics.push(WikiDiagnostic {
            code: "wiki-project-not-found".into(),
            severity: "error".into(),
            path: None,
            message: format!("project id not found: {project_id}"),
        });
    }
    WikiProjectInspectReport {
        project_id,
        project,
        flows,
        diagnostics,
    }
}

fn read_project_articles(
    root: &Path,
    wiki_dir: &Path,
    diagnostics: &mut Vec<WikiDiagnostic>,
) -> Vec<WikiExternalProject> {
    read_markdown_articles(
        &wiki_dir.join("projects"),
        wiki_dir,
        "wiki-project-article-outside-wiki",
        "wiki-project-directory-unreadable",
        "wiki-project-directory-entry-unreadable",
        "wiki-project-article-symlink",
        diagnostics,
    )
    .into_iter()
    .filter_map(|path| parse_project_article(root, wiki_dir, &path, diagnostics))
    .collect()
}

fn read_flow_articles(
    root: &Path,
    wiki_dir: &Path,
    diagnostics: &mut Vec<WikiDiagnostic>,
) -> Vec<WikiProjectFlow> {
    read_markdown_articles(
        &wiki_dir.join("flows"),
        wiki_dir,
        "wiki-project-flow-article-outside-wiki",
        "wiki-project-flow-directory-unreadable",
        "wiki-project-flow-directory-entry-unreadable",
        "wiki-project-flow-article-symlink",
        diagnostics,
    )
    .into_iter()
    .filter_map(|path| parse_flow_article(root, wiki_dir, &path, diagnostics))
    .collect()
}

fn read_markdown_articles(
    dir: &Path,
    wiki_dir: &Path,
    outside_code: &str,
    directory_code: &str,
    entry_code: &str,
    symlink_code: &str,
    diagnostics: &mut Vec<WikiDiagnostic>,
) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            let message = if err.kind() == std::io::ErrorKind::NotFound {
                "article directory not found".to_string()
            } else {
                format!("article directory could not be read: {err}")
            };
            diagnostics.push(diag(directory_code, &rel_path(wiki_dir, dir), &message));
            return Vec::new();
        }
    };
    collect_markdown_article_entries(
        entries,
        dir,
        wiki_dir,
        outside_code,
        entry_code,
        symlink_code,
        diagnostics,
    )
}

fn collect_markdown_article_entries<I>(
    entries: I,
    dir: &Path,
    wiki_dir: &Path,
    outside_code: &str,
    entry_code: &str,
    symlink_code: &str,
    diagnostics: &mut Vec<WikiDiagnostic>,
) -> Vec<PathBuf>
where
    I: IntoIterator<Item = std::io::Result<std::fs::DirEntry>>,
{
    let canonical_wiki = wiki_dir.canonicalize().ok();
    let mut out = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                diagnostics.push(diag(
                    entry_code,
                    &rel_path(wiki_dir, dir),
                    &format!("article directory entry could not be read: {err}"),
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(err) => {
                diagnostics.push(diag(
                    entry_code,
                    &rel_path(wiki_dir, &path),
                    &format!("article entry type could not be read: {err}"),
                ));
                continue;
            }
        };
        if file_type.is_symlink() {
            diagnostics.push(diag(
                symlink_code,
                &rel_path(wiki_dir, &path),
                "project and flow articles must be regular files, not symlinks",
            ));
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if let (Some(wiki), Ok(candidate)) = (&canonical_wiki, path.canonicalize())
            && candidate.strip_prefix(wiki).is_err()
        {
            diagnostics.push(diag(
                outside_code,
                &rel_path(wiki_dir, &path),
                "article resolves outside the live wiki",
            ));
            continue;
        }
        out.push(path);
    }
    out.sort();
    out
}

fn parse_project_article(
    root: &Path,
    wiki_dir: &Path,
    path: &Path,
    diagnostics: &mut Vec<WikiDiagnostic>,
) -> Option<WikiExternalProject> {
    let _ = root;
    let rel = rel_path(wiki_dir, path);
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            diagnostics.push(diag(
                "wiki-project-article-unreadable",
                &rel,
                &format!("project article could not be read: {err}"),
            ));
            return None;
        }
    };
    let fm = match parse_frontmatter(&content) {
        Ok(fm) => fm,
        Err(err) => {
            diagnostics.push(diag(
                "wiki-project-article-frontmatter-invalid",
                &rel,
                &format!("project article frontmatter is invalid: {err}"),
            ));
            return None;
        }
    };
    if one(&fm, "type") != "external-project" {
        diagnostics.push(diag(
            "wiki-project-article-type-invalid",
            &rel,
            "articles under projects/ must use type: external-project",
        ));
        return None;
    }
    let id = one(&fm, "project_id");
    if id.is_empty() {
        diagnostics.push(diag(
            "wiki-project-id-missing",
            &rel,
            "project article is missing project_id",
        ));
        return None;
    }
    let missing_fields = missing_frontmatter_fields(
        &fm,
        &[
            "title",
            "repo",
            "role",
            "interfaces",
            "protocols",
            "status",
            "source_files",
            "external_sources",
        ],
    );
    for field in &missing_fields {
        diagnostics.push(diag(
            "wiki-project-field-missing",
            &rel,
            &format!("project article requires non-empty field: {field}"),
        ));
    }
    if !missing_fields.is_empty() {
        return None;
    }
    Some(WikiExternalProject {
        id,
        title: one(&fm, "title"),
        repo: one(&fm, "repo"),
        role: one(&fm, "role"),
        interfaces: many(&fm, "interfaces"),
        protocols: many(&fm, "protocols"),
        status: one(&fm, "status"),
        source_files: paths(many(&fm, "source_files")),
        external_sources: many(&fm, "external_sources"),
        path: rel,
    })
}

fn parse_flow_article(
    root: &Path,
    wiki_dir: &Path,
    path: &Path,
    diagnostics: &mut Vec<WikiDiagnostic>,
) -> Option<WikiProjectFlow> {
    let _ = root;
    let rel = rel_path(wiki_dir, path);
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            diagnostics.push(diag(
                "wiki-project-flow-article-unreadable",
                &rel,
                &format!("project flow article could not be read: {err}"),
            ));
            return None;
        }
    };
    let fm = match parse_frontmatter(&content) {
        Ok(fm) => fm,
        Err(err) => {
            diagnostics.push(diag(
                "wiki-project-flow-article-frontmatter-invalid",
                &rel,
                &format!("project flow article frontmatter is invalid: {err}"),
            ));
            return None;
        }
    };
    if one(&fm, "type") != "project-flow" {
        diagnostics.push(diag(
            "wiki-project-flow-article-type-invalid",
            &rel,
            "articles under flows/ must use type: project-flow",
        ));
        return None;
    }
    let id = one(&fm, "flow_id");
    if id.is_empty() {
        diagnostics.push(diag(
            "wiki-project-flow-id-missing",
            &rel,
            "flow article is missing flow_id",
        ));
        return None;
    }
    let missing_fields = missing_frontmatter_fields(
        &fm,
        &[
            "title",
            "projects",
            "kind",
            "protocols",
            "requirements",
            "specs",
            "source_files",
            "external_sources",
        ],
    );
    for field in &missing_fields {
        diagnostics.push(diag(
            "wiki-project-flow-field-missing",
            &rel,
            &format!("project flow article requires non-empty field: {field}"),
        ));
    }
    if !missing_fields.is_empty() {
        return None;
    }
    Some(WikiProjectFlow {
        id,
        title: one(&fm, "title"),
        projects: many(&fm, "projects"),
        kind: one(&fm, "kind"),
        protocols: many(&fm, "protocols"),
        requirements: many(&fm, "requirements"),
        specs: paths(many(&fm, "specs")),
        source_files: paths(many(&fm, "source_files")),
        external_sources: many(&fm, "external_sources"),
        path: rel,
    })
}

fn parse_frontmatter(content: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut map = BTreeMap::<String, Vec<String>>::new();
    let mut lines = content.lines().enumerate();
    if lines.next().map(|(_, line)| line) != Some("---") {
        return Err("line 1 must be `---`".into());
    }
    let mut current_key = String::new();
    for (index, line) in lines {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed == "---" {
            return Ok(map);
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(value) = trimmed.strip_prefix("- ") {
            if current_key.is_empty() {
                return Err(format!(
                    "line {line_number} has a list item without a preceding key"
                ));
            }
            map.entry(current_key.clone())
                .or_default()
                .push(unquote(value.trim()));
            continue;
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            if key.is_empty() {
                return Err(format!("line {line_number} has an empty key"));
            }
            if map.contains_key(key) {
                return Err(format!("line {line_number} duplicates key `{key}`"));
            }
            current_key = key.to_string();
            let value = value.trim();
            map.entry(current_key.clone()).or_default();
            if !value.is_empty() {
                for item in parse_scalar_or_inline_list(value) {
                    map.entry(current_key.clone()).or_default().push(item);
                }
            }
            continue;
        }
        return Err(format!("line {line_number} must use `key: value` syntax"));
    }
    Err("frontmatter is missing closing `---`".into())
}

fn missing_frontmatter_fields<'a>(
    map: &BTreeMap<String, Vec<String>>,
    required: &'a [&'a str],
) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|key| {
            map.get(*key).is_none_or(|values| {
                values.is_empty() || values.iter().all(|value| value.trim().is_empty())
            })
        })
        .collect()
}

fn parse_scalar_or_inline_list(value: &str) -> Vec<String> {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return inner
            .split(',')
            .map(|item| unquote(item.trim()))
            .filter(|item| !item.is_empty())
            .collect();
    }
    vec![unquote(value)]
}

fn one(map: &BTreeMap<String, Vec<String>>, key: &str) -> String {
    map.get(key)
        .and_then(|values| values.first())
        .cloned()
        .unwrap_or_default()
}

fn many(map: &BTreeMap<String, Vec<String>>, key: &str) -> Vec<String> {
    map.get(key).cloned().unwrap_or_default()
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn paths(values: Vec<String>) -> Vec<PathBuf> {
    values.into_iter().map(PathBuf::from).collect()
}

fn rel_path(base: &Path, path: &Path) -> PathBuf {
    PathBuf::from(portable_path(path.strip_prefix(base).unwrap_or(path)))
}

fn validate_project_map(
    root: &Path,
    projects: &[WikiExternalProject],
    flows: &[WikiProjectFlow],
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let mut ids = BTreeMap::<String, PathBuf>::new();
    for project in projects {
        if !valid_project_id(&project.id) {
            diagnostics.push(diag(
                "wiki-project-id-invalid",
                &project.path,
                "project_id must be lowercase kebab-case",
            ));
        }
        if let Some(existing) = ids.insert(project.id.clone(), project.path.clone()) {
            diagnostics.push(diag(
                "wiki-project-id-duplicate",
                &project.path,
                "project_id duplicates another project article",
            ));
            diagnostics.push(diag(
                "wiki-project-id-duplicate",
                &existing,
                "project_id duplicates another project article",
            ));
        }
        validate_repo_local_sources(root, &project.path, &project.source_files, diagnostics);
    }

    let known = ids.keys().cloned().collect::<BTreeSet<_>>();
    let known_requirements =
        crate::spec_knowledge::build_requirement_graph(&root.join("knowledge"))
            .nodes
            .into_iter()
            .map(|node| node.id.to_ascii_uppercase())
            .collect::<BTreeSet<_>>();
    let mut flow_ids = BTreeMap::<String, PathBuf>::new();
    for flow in flows {
        if !valid_project_id(&flow.id) {
            diagnostics.push(diag(
                "wiki-project-flow-id-invalid",
                &flow.path,
                "flow_id must be lowercase kebab-case",
            ));
        }
        if let Some(existing) = flow_ids.insert(flow.id.clone(), flow.path.clone()) {
            diagnostics.push(diag(
                "wiki-project-flow-id-duplicate",
                &flow.path,
                "flow_id duplicates another flow article",
            ));
            diagnostics.push(diag(
                "wiki-project-flow-id-duplicate",
                &existing,
                "flow_id duplicates another flow article",
            ));
        }
        let distinct_projects = flow.projects.iter().collect::<BTreeSet<_>>();
        if distinct_projects.len() != flow.projects.len() {
            diagnostics.push(diag(
                "wiki-project-flow-project-duplicate",
                &flow.path,
                "project-flow contains a duplicate project id",
            ));
        }
        if distinct_projects.len() < 2 {
            diagnostics.push(diag(
                "wiki-project-flow-too-small",
                &flow.path,
                "project-flow must reference at least two distinct projects",
            ));
        }
        for project in &flow.projects {
            if !known.contains(project) {
                diagnostics.push(diag(
                    "wiki-project-flow-unknown-project",
                    &flow.path,
                    &format!("project-flow references unknown project id: {project}"),
                ));
            }
        }
        validate_repo_local_sources(root, &flow.path, &flow.source_files, diagnostics);
        for requirement in &flow.requirements {
            if !known_requirements.contains(&requirement.to_ascii_uppercase()) {
                diagnostics.push(diag(
                    "wiki-project-requirement-unknown",
                    &flow.path,
                    &format!("flow references unknown requirement id: {requirement}"),
                ));
            }
        }
        for spec in &flow.specs {
            validate_spec_reference(root, &flow.path, spec, diagnostics);
        }
    }
}

fn valid_project_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
}

fn validate_repo_local_sources(
    root: &Path,
    article_path: &Path,
    source_files: &[PathBuf],
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    for source in source_files {
        match repo_path_issue(root, source) {
            Some(
                RepoPathIssue::Absolute
                | RepoPathIssue::ParentTraversal
                | RepoPathIssue::OutsideRoot,
            ) => {
                diagnostics.push(diag(
                    "wiki-project-source-outside-root",
                    article_path,
                    &format!(
                        "source_files entry must remain inside the repo: {}",
                        portable_path(source)
                    ),
                ));
            }
            Some(RepoPathIssue::Missing) => {
                diagnostics.push(diag(
                    "wiki-project-source-missing",
                    article_path,
                    &format!(
                        "source_files entry does not exist: {}",
                        portable_path(source)
                    ),
                ));
            }
            None => {}
        }
    }
}

fn validate_spec_reference(
    root: &Path,
    article_path: &Path,
    spec: &Path,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let spec_text = portable_path(spec);
    match repo_path_issue(root, spec) {
        Some(RepoPathIssue::Missing) => diagnostics.push(diag(
            "wiki-project-spec-missing",
            article_path,
            &format!("flow spec does not exist: {spec_text}"),
        )),
        Some(
            RepoPathIssue::Absolute | RepoPathIssue::ParentTraversal | RepoPathIssue::OutsideRoot,
        ) => {
            diagnostics.push(diag(
                "wiki-project-spec-outside-root",
                article_path,
                &format!("flow spec must remain inside the repo: {spec_text}"),
            ));
        }
        None => {
            let valid_suffix = spec_text.ends_with(".spec") || spec_text.ends_with(".spec.md");
            let is_task_contract = valid_suffix
                && crate::spec_parser::parse_spec(&root.join(spec))
                    .is_ok_and(|document| document.meta.level == crate::spec_core::SpecLevel::Task);
            if !is_task_contract {
                diagnostics.push(diag(
                    "wiki-project-spec-invalid",
                    article_path,
                    &format!("flow spec is not a parseable Task Contract: {spec_text}"),
                ));
            }
        }
    }
}

fn derive_edges(flows: &[WikiProjectFlow]) -> Vec<WikiProjectEdge> {
    let mut out = Vec::new();
    for flow in flows {
        for pair in flow.projects.windows(2) {
            out.push(WikiProjectEdge {
                from: pair[0].clone(),
                to: pair[1].clone(),
                kind: if flow.kind.is_empty() {
                    "depends_on".into()
                } else {
                    flow.kind.clone()
                },
                flow_id: flow.id.clone(),
                protocols: flow.protocols.clone(),
            });
        }
    }
    out.sort_by(|left, right| {
        left.from
            .cmp(&right.from)
            .then(left.to.cmp(&right.to))
            .then(left.flow_id.cmp(&right.flow_id))
    });
    out
}

fn mermaid_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn mermaid_label(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('|', "&#124;")
        .replace('"', "&quot;")
        .replace('\n', "<br/>")
        .replace('\r', "")
}

fn portable_path(path: &Path) -> String {
    path_to_slash(path).replace('\\', "/")
}

fn serialize_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&portable_path(path))
}

fn serialize_paths<S>(paths: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut sequence = serializer.serialize_seq(Some(paths.len()))?;
    for path in paths {
        sequence.serialize_element(&portable_path(path))?;
    }
    sequence.end()
}

#[derive(Serialize)]
struct PortableWikiDiagnostic<'a> {
    code: &'a str,
    severity: &'a str,
    path: Option<String>,
    message: &'a str,
}

fn serialize_diagnostics<S>(
    diagnostics: &[WikiDiagnostic],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut sequence = serializer.serialize_seq(Some(diagnostics.len()))?;
    for diagnostic in diagnostics {
        sequence.serialize_element(&PortableWikiDiagnostic {
            code: &diagnostic.code,
            severity: &diagnostic.severity,
            path: diagnostic.path.as_deref().map(portable_path),
            message: &diagnostic.message,
        })?;
    }
    sequence.end()
}

fn diag(code: &str, path: &Path, message: &str) -> WikiDiagnostic {
    WikiDiagnostic {
        code: code.into(),
        severity: "error".into(),
        path: Some(path.to_path_buf()),
        message: message.into(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".agent-spec/wiki/projects")).unwrap();
        fs::create_dir_all(dir.join(".agent-spec/wiki/flows")).unwrap();
        fs::create_dir_all(dir.join("knowledge/requirements")).unwrap();
        fs::create_dir_all(dir.join("specs")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname=\"agent-spec\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .unwrap();
        fs::write(
            dir.join("knowledge/requirements/req-cross-project-wiki.md"),
            "---\nkind: requirement\nid: REQ-CROSS-PROJECT-WIKI\ntitle: \"Cross Project Wiki\"\n---\n# Cross Project Wiki\n\n## Requirements\n\n[REQ-CROSS-PROJECT-WIKI] The system MUST map projects.\n\n## Scenarios\n\nScenario: Map projects\n  Given project articles\n  When the map builds\n  Then projects are present\n",
        )
        .unwrap();
        fs::write(
            dir.join("specs/task-cross-project-wiki.spec.md"),
            "spec: task\nname: \"Cross Project Wiki\"\nsatisfies: [REQ-CROSS-PROJECT-WIKI]\n---\n\n## Intent\n\nMap projects.\n\n## Completion Criteria\n\nScenario: Map projects\n  Test: test_map_projects\n  Given project articles\n  When the map builds\n  Then projects are present\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn test_project_map_builds_projects_flows_edges_and_external_sources() {
        let dir = fixture("wiki-project-map");
        let wiki = dir.join(".agent-spec/wiki");
        fs::write(
            wiki.join("projects/agent-spec.md"),
            "---\ntitle: \"agent-spec\"\ntype: external-project\nproject_id: agent-spec\nrepo: .\nrole: \"main project\"\ninterfaces:\n  - cli\nprotocols:\n  - filesystem\nstatus: active\nsource_files:\n  - Cargo.toml\nexternal_sources:\n  - ./README.md\ntags:\n  - main\n---\n# agent-spec\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/brain-rs.md"),
            "---\ntitle: \"brain-rs\"\ntype: external-project\nproject_id: brain-rs\nrepo: /Users/example/brain-rs\nrole: \"context provider\"\ninterfaces:\n  - cli\nprotocols:\n  - stdio\nstatus: active\nsource_files:\n  - Cargo.toml\nexternal_sources:\n  - /Users/example/brain-rs/README.md\ntags:\n  - dependency\n---\n# brain-rs\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/main-to-brain.md"),
            "---\ntitle: \"Main to brain-rs context flow\"\ntype: project-flow\nflow_id: main-to-brain\nprojects:\n  - agent-spec\n  - brain-rs\nkind: calls\nprotocols:\n  - stdio\nrequirements:\n  - REQ-CROSS-PROJECT-WIKI\nspecs:\n  - specs/task-cross-project-wiki.spec.md\nsource_files:\n  - Cargo.toml\nexternal_sources:\n  - /Users/example/brain-rs/src/lib.rs\ntags:\n  - data-flow\n---\n# Main to brain-rs context flow\n",
        )
        .unwrap();

        let map = build_project_map(&dir, &wiki);

        assert_eq!(map.projects.len(), 2);
        assert_eq!(map.flows.len(), 1);
        assert_eq!(map.edges.len(), 1);
        assert!(map.projects.iter().any(|project| project.id == "brain-rs"));
        assert_eq!(map.edges[0].from, "agent-spec");
        assert_eq!(map.edges[0].to, "brain-rs");
        assert_eq!(map.edges[0].kind, "calls");
        assert!(
            map.flows[0]
                .external_sources
                .contains(&"/Users/example/brain-rs/src/lib.rs".to_string())
        );
        assert!(map.diagnostics.is_empty(), "{:?}", map.diagnostics);

        let mermaid = render_project_map_mermaid(&map);
        assert!(mermaid.contains("agent-spec"));
        assert!(mermaid.contains("brain-rs"));
        assert!(mermaid.contains("calls"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_project_map_reports_unknown_flow_project() {
        let dir = fixture("wiki-project-map-broken");
        let wiki = dir.join(".agent-spec/wiki");
        fs::write(
            wiki.join("projects/agent-spec.md"),
            "---\ntitle: \"agent-spec\"\ntype: external-project\nproject_id: agent-spec\nrepo: .\nrole: \"main project\"\ninterfaces:\n  - cli\nprotocols:\n  - filesystem\nstatus: active\nsource_files:\n  - Cargo.toml\nexternal_sources:\n  - example/agent-spec\n---\n# agent-spec\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/broken.md"),
            "---\ntitle: \"Broken\"\ntype: project-flow\nflow_id: broken\nprojects:\n  - agent-spec\n  - missing-project\nkind: calls\nprotocols:\n  - stdio\nrequirements:\n  - REQ-CROSS-PROJECT-WIKI\nspecs:\n  - specs/task-cross-project-wiki.spec.md\nsource_files:\n  - Cargo.toml\nexternal_sources:\n  - example/missing-project\n---\n# Broken\n",
        )
        .unwrap();

        let map = build_project_map(&dir, &wiki);

        assert!(
            map.diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "wiki-project-flow-unknown-project")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_project_map_reports_contract_diagnostics() {
        let dir = fixture("wiki-project-map-contract-errors");
        let wiki = dir.join(".agent-spec/wiki");
        fs::write(
            wiki.join("projects/main.md"),
            "---\ntitle: \"Main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\nexternal_sources: [example/main]\n---\n# Main\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/main-copy.md"),
            "---\ntitle: \"Main Copy\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: duplicate\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\nexternal_sources: [example/main-copy]\n---\n# Main Copy\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/invalid.md"),
            "---\ntitle: \"Invalid\"\ntype: external-project\nproject_id: invalid--id\nrepo: .\nrole: invalid\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\nexternal_sources: [example/invalid]\n---\n# Invalid\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/wrong-type.md"),
            "---\ntitle: \"Wrong\"\ntype: module\nsource_files: [Cargo.toml]\n---\n# Wrong\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/repeated.md"),
            "---\ntitle: \"Repeated\"\ntype: project-flow\nflow_id: Invalid Flow\nprojects: [main, main]\nkind: calls\nprotocols: [stdio]\nrequirements: [REQ-CROSS-PROJECT-WIKI]\nspecs: [specs/task-cross-project-wiki.spec.md]\nsource_files: [missing.rs]\nexternal_sources: [example/repeated]\n---\n# Repeated\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/duplicate-a.md"),
            "---\ntitle: \"Duplicate A\"\ntype: project-flow\nflow_id: duplicate-flow\nprojects: [main, invalid--id]\nkind: calls\nprotocols: [stdio]\nrequirements: [REQ-CROSS-PROJECT-WIKI]\nspecs: [specs/task-cross-project-wiki.spec.md]\nsource_files: [Cargo.toml]\nexternal_sources: [example/duplicate-a]\n---\n# A\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/duplicate-b.md"),
            "---\ntitle: \"Duplicate B\"\ntype: project-flow\nflow_id: duplicate-flow\nprojects: [main, invalid--id]\nkind: calls\nprotocols: [stdio]\nrequirements: [REQ-CROSS-PROJECT-WIKI]\nspecs: [specs/task-cross-project-wiki.spec.md]\nsource_files: [Cargo.toml]\nexternal_sources: [example/duplicate-b]\n---\n# B\n",
        )
        .unwrap();

        let map = build_project_map(&dir, &wiki);
        let codes = map
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();

        for expected in [
            "wiki-project-id-invalid",
            "wiki-project-id-duplicate",
            "wiki-project-article-type-invalid",
            "wiki-project-flow-id-invalid",
            "wiki-project-flow-id-duplicate",
            "wiki-project-flow-project-duplicate",
            "wiki-project-flow-too-small",
            "wiki-project-source-missing",
        ] {
            assert!(codes.contains(&expected), "missing {expected}: {codes:?}");
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_project_map_rejects_incomplete_and_malformed_articles() {
        let dir = fixture("wiki-project-map-incomplete-articles");
        let wiki = dir.join(".agent-spec/wiki");
        for id in ["main", "dependency"] {
            fs::write(
                wiki.join("projects").join(format!("{id}.md")),
                format!(
                    "---\ntitle: \"{id}\"\ntype: external-project\nproject_id: {id}\nrepo: {id}\nrole: project\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\nexternal_sources: [example/{id}]\n---\n# {id}\n"
                ),
            )
            .unwrap();
        }
        fs::write(
            wiki.join("projects/incomplete.md"),
            "---\ntype: external-project\nproject_id: incomplete\nsource_files: [Cargo.toml]\n---\n# Incomplete\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/malformed.md"),
            "---\ntitle: \"Malformed\"\ntype: external-project\nproject_id: malformed\nrepo: malformed\nrole \"missing colon\"\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\nexternal_sources: [example/malformed]\n---\n# Malformed\n",
        )
        .unwrap();
        fs::write(
            wiki.join("projects/duplicate-key.md"),
            "---\ntitle: \"Duplicate key\"\ntype: external-project\nproject_id: duplicate-key\nproject_id: duplicate-key-again\nrepo: duplicate-key\nrole: project\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\nexternal_sources: [example/duplicate-key]\n---\n# Duplicate key\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/incomplete.md"),
            "---\ntitle: \"Incomplete flow\"\ntype: project-flow\nflow_id: incomplete-flow\nprojects: [main, dependency]\nkind: calls\nsource_files: [Cargo.toml]\n---\n# Incomplete flow\n",
        )
        .unwrap();

        let map = build_project_map(&dir, &wiki);

        for field in [
            "title",
            "repo",
            "role",
            "interfaces",
            "protocols",
            "status",
            "external_sources",
        ] {
            assert!(map.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "wiki-project-field-missing"
                    && diagnostic.path.as_deref() == Some(Path::new("projects/incomplete.md"))
                    && diagnostic.message.contains(field)
            }));
        }
        for field in ["protocols", "requirements", "specs", "external_sources"] {
            assert!(map.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "wiki-project-flow-field-missing"
                    && diagnostic.path.as_deref() == Some(Path::new("flows/incomplete.md"))
                    && diagnostic.message.contains(field)
            }));
        }
        assert!(map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-article-frontmatter-invalid"
                && diagnostic.path.as_deref() == Some(Path::new("projects/malformed.md"))
                && diagnostic.message.contains("line")
        }));
        assert!(map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-article-frontmatter-invalid"
                && diagnostic.path.as_deref() == Some(Path::new("projects/duplicate-key.md"))
                && diagnostic.message.contains("duplicates key `project_id`")
        }));
        assert!(
            !map.projects
                .iter()
                .any(|project| project.id == "incomplete")
        );
        assert!(!map.projects.iter().any(|project| project.id == "malformed"));
        assert!(map.flows.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_project_map_reports_invalid_requirement_and_spec_references() {
        let dir = fixture("wiki-project-map-invalid-trace-refs");
        let wiki = dir.join(".agent-spec/wiki");
        for id in ["main", "dependency"] {
            fs::write(
                wiki.join("projects").join(format!("{id}.md")),
                format!(
                    "---\ntitle: \"{id}\"\ntype: external-project\nproject_id: {id}\nrepo: .\nrole: project\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\n---\n# {id}\n"
                ),
            )
            .unwrap();
        }
        fs::write(
            dir.join("specs/project-overview.spec.md"),
            "spec: project\nname: \"Project Overview\"\n---\n\n## Intent\n\nDescribe the project.\n",
        )
        .unwrap();
        fs::write(
            wiki.join("flows/invalid-refs.md"),
            "---\ntitle: \"Invalid refs\"\ntype: project-flow\nflow_id: invalid-refs\nprojects: [main, dependency]\nkind: calls\nprotocols: [stdio]\nrequirements: [REQ-NOT-FOUND]\nspecs: [specs/missing.spec.md, specs/project-overview.spec.md]\nsource_files: [Cargo.toml]\nexternal_sources: [/outside/repository/README.md]\n---\n# Invalid refs\n",
        )
        .unwrap();

        let map = build_project_map(&dir, &wiki);

        assert!(map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-requirement-unknown"
                && diagnostic.message.contains("REQ-NOT-FOUND")
        }));
        assert!(map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-spec-missing"
                && diagnostic.message.contains("specs/missing.spec.md")
        }));
        assert!(map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-spec-invalid"
                && diagnostic
                    .message
                    .contains("specs/project-overview.spec.md")
        }));
        assert!(
            !map.diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("/outside/repository/README.md") })
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_project_map_reports_article_enumeration_failures() {
        let dir = fixture("wiki-project-map-unreadable-directory");
        let wiki = dir.join(".agent-spec/wiki");
        fs::remove_dir_all(wiki.join("flows")).unwrap();
        fs::write(wiki.join("flows"), "not a directory\n").unwrap();

        let map = build_project_map(&dir, &wiki);

        assert!(map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-flow-directory-unreadable"
                && diagnostic.path.as_deref() == Some(Path::new("flows"))
        }));

        fs::remove_file(wiki.join("flows")).unwrap();
        let missing_map = build_project_map(&dir, &wiki);
        assert!(missing_map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-flow-directory-unreadable"
                && diagnostic.path.as_deref() == Some(Path::new("flows"))
                && diagnostic.message.contains("not found")
        }));

        let mut entry_diagnostics = Vec::new();
        let entries: Vec<std::io::Result<std::fs::DirEntry>> =
            vec![Err(std::io::Error::other("entry failed"))];
        let paths = collect_markdown_article_entries(
            entries,
            wiki.join("projects").as_path(),
            &wiki,
            "wiki-project-article-outside-wiki",
            "wiki-project-directory-entry-unreadable",
            "wiki-project-article-symlink",
            &mut entry_diagnostics,
        );
        assert!(paths.is_empty());
        assert!(entry_diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-directory-entry-unreadable"
                && diagnostic.path.as_deref() == Some(Path::new("projects"))
        }));
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_project_map_rejects_symlinked_articles() {
        use std::os::unix::fs::symlink;

        let dir = fixture("wiki-project-map-symlink-article");
        let wiki = dir.join(".agent-spec/wiki");
        fs::write(
            wiki.join("main.md"),
            "---\ntitle: \"Main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [Cargo.toml]\n---\n# Main\n",
        )
        .unwrap();
        symlink("../main.md", wiki.join("projects/main.md")).unwrap();

        let map = build_project_map(&dir, &wiki);

        assert!(map.projects.is_empty());
        assert!(map.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "wiki-project-article-symlink"
                && diagnostic.path.as_deref() == Some(Path::new("projects/main.md"))
        }));
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_project_map_rejects_repo_relative_symlink_outside_root() {
        use std::os::unix::fs::symlink;

        let dir = fixture("wiki-project-map-symlink-source");
        let wiki = dir.join(".agent-spec/wiki");
        let outside =
            std::env::temp_dir().join(format!("wiki-project-map-outside-{}", std::process::id()));
        fs::write(&outside, "outside\n").unwrap();
        symlink(&outside, dir.join("outside-link.md")).unwrap();
        fs::write(
            wiki.join("projects/main.md"),
            "---\ntitle: \"Main\"\ntype: external-project\nproject_id: main\nrepo: .\nrole: main\ninterfaces: [cli]\nprotocols: [filesystem]\nstatus: active\nsource_files: [outside-link.md]\nexternal_sources: [example/main]\n---\n# Main\n",
        )
        .unwrap();

        let map = build_project_map(&dir, &wiki);

        assert!(
            map.diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "wiki-project-source-outside-root" })
        );
        let _ = fs::remove_file(outside);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_project_map_renders_safe_mermaid_and_portable_json() {
        let mut map = WikiProjectMap {
            version: 1,
            projects: vec![WikiExternalProject {
                id: "main-project".into(),
                title: "Main".into(),
                repo: ".".into(),
                role: "main".into(),
                interfaces: vec![],
                protocols: vec![],
                status: "active".into(),
                source_files: vec![PathBuf::from(r"src\main.rs")],
                external_sources: vec![],
                path: PathBuf::from(r"projects\main-project.md"),
            }],
            flows: vec![],
            edges: vec![WikiProjectEdge {
                from: "main-project".into(),
                to: "dependency".into(),
                kind: "calls|writes\nnext".into(),
                flow_id: "main-to-dependency".into(),
                protocols: vec![],
            }],
            diagnostics: vec![WikiDiagnostic {
                code: "wiki-project-test".into(),
                severity: "error".into(),
                path: Some(PathBuf::from(r"flows\broken.md")),
                message: "broken flow".into(),
            }],
        };
        map.projects.push(WikiExternalProject {
            id: "dependency".into(),
            title: "Dependency".into(),
            repo: "dependency".into(),
            role: "dependency".into(),
            interfaces: vec![],
            protocols: vec![],
            status: "active".into(),
            source_files: vec![],
            external_sources: vec![],
            path: PathBuf::from("projects/dependency.md"),
        });

        let json = serde_json::to_string(&map).unwrap();
        let mermaid = render_project_map_mermaid(&map);

        assert!(json.contains("src/main.rs"), "{json}");
        assert!(json.contains("projects/main-project.md"), "{json}");
        assert!(json.contains("flows/broken.md"), "{json}");
        assert!(!json.contains(r"src\\main.rs"), "{json}");
        assert!(!json.contains(r"flows\\broken.md"), "{json}");
        assert!(mermaid.contains("calls&#124;writes<br/>next"), "{mermaid}");
        assert!(!mermaid.contains("calls|writes"), "{mermaid}");
    }
}
