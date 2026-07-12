use crate::spec_wiki::sources::{RepoPathIssue, repo_path_issue};
use crate::spec_wiki::{
    ArchitectureInventory, WikiArticle, WikiCheckReport, WikiDiagnostic, WikiInitReport,
    WikiInspectReport, WikiMeta, WikiQueryMatch, WikiQueryReport, WikiRequirementLink,
    WikiSeedReport, WikiSpecLink, WikiStaleArticle, WikiStatusReport, WikiTraceLink,
    build_architecture_inventory, build_project_map, path_to_slash, render_architecture_mermaid,
    render_architecture_module_mermaid, render_project_map_mermaid,
};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn init_live_wiki(
    root: &Path,
    wiki_dir: &Path,
) -> Result<WikiInitReport, Box<dyn std::error::Error>> {
    let mut files_written = Vec::new();
    let mut diagnostics = Vec::new();
    for dir in [
        "architecture",
        "modules",
        "concepts",
        "decisions",
        "learnings",
        "queries",
        "projects",
        "flows",
    ] {
        std::fs::create_dir_all(wiki_dir.join(dir))?;
    }

    let inventory = build_architecture_inventory(root);
    diagnostics.extend(inventory.diagnostics.clone());
    write_file(
        wiki_dir,
        Path::new("architecture/inventory.json"),
        &serde_json::to_string_pretty(&inventory)?,
        &mut files_written,
    )?;
    write_file(
        wiki_dir,
        Path::new("architecture/workspace.mmd"),
        &render_architecture_mermaid(&inventory),
        &mut files_written,
    )?;
    write_file(
        wiki_dir,
        Path::new("architecture/modules.mmd"),
        &render_architecture_module_mermaid(&inventory),
        &mut files_written,
    )?;
    let project_map = build_project_map(root, wiki_dir);
    diagnostics.extend(project_map.diagnostics.clone());
    files_written.extend(write_project_map_artifacts_from_map(
        wiki_dir,
        &project_map,
    )?);

    write_file(
        wiki_dir,
        Path::new("_architecture.md"),
        &render_architecture_article(&inventory),
        &mut files_written,
    )?;
    write_file(
        wiki_dir,
        Path::new("_patterns.md"),
        &render_patterns_article(&inventory),
        &mut files_written,
    )?;
    write_file(
        wiki_dir,
        Path::new("_log.md"),
        "# Log\n\n- Wiki initialized by `agent-spec wiki init`.\n",
        &mut files_written,
    )?;

    let meta = build_meta(root);
    write_file(
        wiki_dir,
        Path::new("_meta.json"),
        &serde_json::to_string_pretty(&meta)?,
        &mut files_written,
    )?;

    write_wiki_index(wiki_dir)?;
    files_written.push(PathBuf::from("_index.md"));

    Ok(WikiInitReport {
        wiki_dir: wiki_dir.to_path_buf(),
        files_written,
        diagnostics,
    })
}

pub fn write_wiki_index(
    wiki_dir: &Path,
) -> Result<Vec<WikiDiagnostic>, Box<dyn std::error::Error>> {
    let (out, diagnostics) = render_wiki_index(wiki_dir);
    std::fs::create_dir_all(wiki_dir)?;
    std::fs::write(wiki_dir.join("_index.md"), out)?;
    Ok(diagnostics)
}

pub fn render_wiki_index(wiki_dir: &Path) -> (String, Vec<WikiDiagnostic>) {
    let (articles, diagnostics) = collect_wiki_articles(wiki_dir);
    let mut grouped = BTreeMap::<String, Vec<WikiArticle>>::new();
    for article in articles {
        grouped
            .entry(article.article_type.clone())
            .or_default()
            .push(article);
    }
    let mut out = String::from("# Code Live Wiki\n\n");
    out.push_str("This index is generated from article frontmatter. Read it before opening individual wiki pages.\n\n");
    for (article_type, mut articles) in grouped {
        articles.sort_by(|left, right| left.path.cmp(&right.path));
        out.push_str(&format!("## {}\n\n", title_case(&article_type)));
        for article in articles {
            out.push_str(&format!(
                "- [{}]({})",
                article.title,
                path_to_slash(&article.path)
            ));
            if !article.source_files.is_empty() {
                let sources = article
                    .source_files
                    .iter()
                    .map(|path| format!("`{}`", path_to_slash(path)))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(" — {}", sources));
            }
            out.push('\n');
        }
        out.push('\n');
    }
    (out, diagnostics)
}

pub fn update_wiki_meta(
    root: &Path,
    wiki_dir: &Path,
) -> Result<WikiMeta, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(wiki_dir)?;
    let meta = build_meta(root);
    std::fs::write(
        wiki_dir.join("_meta.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;
    Ok(meta)
}

pub fn seed_live_wiki(
    root: &Path,
    wiki_dir: &Path,
) -> Result<WikiSeedReport, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(wiki_dir)?;
    let specs = seed_page_specs(root);
    let mut files_written = Vec::new();
    let mut missing_pages = Vec::new();
    let diagnostics = Vec::new();
    for spec in specs {
        let path = wiki_dir.join(&spec.path);
        if path.exists() {
            continue;
        }
        missing_pages.push(spec.path.clone());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, render_seed_page(&spec))?;
        files_written.push(spec.path);
    }
    if !files_written.is_empty() {
        write_wiki_index(wiki_dir)?;
        files_written.push(PathBuf::from("_index.md"));
    }
    Ok(WikiSeedReport {
        wiki_dir: wiki_dir.to_path_buf(),
        files_written,
        missing_pages,
        diagnostics,
    })
}

pub fn seed_live_wiki_check(root: &Path, wiki_dir: &Path) -> WikiSeedReport {
    let missing_pages = seed_page_specs(root)
        .into_iter()
        .filter_map(|spec| (!wiki_dir.join(&spec.path).exists()).then_some(spec.path))
        .collect::<Vec<_>>();
    let diagnostics = missing_pages
        .iter()
        .map(|path| WikiDiagnostic {
            code: "wiki-seed-page-missing".into(),
            severity: "error".into(),
            path: Some(path.clone()),
            message: "seedable live wiki page is missing".into(),
        })
        .collect::<Vec<_>>();
    WikiSeedReport {
        wiki_dir: wiki_dir.to_path_buf(),
        files_written: Vec::new(),
        missing_pages,
        diagnostics,
    }
}

pub fn query_live_wiki(wiki_dir: &Path, query: &str) -> WikiQueryReport {
    let (articles, diagnostics) = collect_wiki_articles(wiki_dir);
    let needle = query.to_ascii_lowercase();
    let mut matches = Vec::new();
    for article in articles {
        let content = std::fs::read_to_string(wiki_dir.join(&article.path)).unwrap_or_default();
        let haystacks = [
            article.title.to_ascii_lowercase(),
            article.article_type.to_ascii_lowercase(),
            article
                .tags
                .iter()
                .map(|tag| tag.to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(" "),
            article
                .source_files
                .iter()
                .map(|source| path_to_slash(source).to_ascii_lowercase())
                .collect::<Vec<_>>()
                .join(" "),
            content.to_ascii_lowercase(),
        ];
        let score = haystacks
            .iter()
            .map(|haystack| haystack.matches(&needle).count())
            .sum::<usize>();
        if score > 0 {
            matches.push(WikiQueryMatch {
                path: article.path,
                title: article.title,
                article_type: article.article_type,
                source_files: article.source_files,
                tags: article.tags,
                score,
            });
        }
    }
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(left.path.cmp(&right.path))
    });
    WikiQueryReport {
        wiki_dir: wiki_dir.to_path_buf(),
        query: query.into(),
        matches,
        diagnostics,
    }
}

pub fn inspect_live_wiki_path(
    root: &Path,
    wiki_dir: &Path,
    input_path: &Path,
) -> WikiInspectReport {
    let input_path = repo_relative_input(root, input_path);
    let (articles, diagnostics) = collect_wiki_articles(wiki_dir);
    let input_text = normalize_path(&input_path);
    let wiki_articles = articles
        .into_iter()
        .filter(|article| {
            article
                .source_files
                .iter()
                .map(|source| normalize_path(source))
                .any(|source| path_overlaps(&source, &input_text))
        })
        .map(|article| WikiQueryMatch {
            path: article.path,
            title: article.title,
            article_type: article.article_type,
            source_files: article.source_files,
            tags: article.tags,
            score: 1,
        })
        .collect::<Vec<_>>();

    let requirements = matching_requirements(root, &input_path);
    let requirement_ids = requirements
        .iter()
        .map(|requirement| requirement.id.clone())
        .collect::<BTreeSet<_>>();
    let specs = matching_specs(root, &requirement_ids);
    let trace_records = matching_trace_records(root, &input_path);

    WikiInspectReport {
        input_path,
        wiki_articles,
        requirements,
        specs,
        trace_records,
        diagnostics,
    }
}

pub fn wiki_status(root: &Path, wiki_dir: &Path) -> WikiStatusReport {
    let mut diagnostics = Vec::new();
    let meta = read_wiki_meta(wiki_dir);
    let first_run = meta
        .as_ref()
        .and_then(|meta| meta.last_compiled_commit.as_ref())
        .is_none();
    let mut changed_files = worktree_changed_files(root, &mut diagnostics);
    changed_files.sort();
    changed_files.dedup();
    let mut report = status_from_changed_paths(wiki_dir, &changed_files);
    report.first_run = first_run;
    report.changed_files = changed_files;
    report.diagnostics.extend(diagnostics);
    report
}

pub fn status_from_changed_paths(wiki_dir: &Path, changed_files: &[PathBuf]) -> WikiStatusReport {
    let (articles, diagnostics) = collect_wiki_articles(wiki_dir);
    let changed = changed_files
        .iter()
        .map(|path| normalize_path(path.as_path()))
        .collect::<Vec<_>>();
    let mut stale_articles = Vec::new();
    for article in articles {
        let article_path = normalize_path(&article.path);
        let article_updated = changed
            .iter()
            .any(|path| path == &article_path || path.ends_with(&format!("/{article_path}")));
        let article_sources = article
            .source_files
            .iter()
            .map(|path| normalize_path(path.as_path()))
            .collect::<Vec<_>>();
        let article_changed = changed_files
            .iter()
            .filter(|changed_path| {
                let changed_path = normalize_path(changed_path);
                article_sources
                    .iter()
                    .any(|source| path_overlaps(source, &changed_path))
            })
            .cloned()
            .collect::<Vec<_>>();
        if !article_updated
            && (!article_changed.is_empty()
                || article_sources
                    .iter()
                    .any(|source| changed.iter().any(|path| path_overlaps(source, path))))
        {
            stale_articles.push(WikiStaleArticle {
                path: article.path,
                title: article.title,
                source_files: article.source_files,
                changed_files: article_changed,
            });
        }
    }

    WikiStatusReport {
        wiki_dir: wiki_dir.to_path_buf(),
        first_run: false,
        changed_files: changed_files.to_vec(),
        stale_articles,
        diagnostics,
    }
}

pub fn lint_live_wiki(root: &Path, wiki_dir: &Path) -> WikiCheckReport {
    let mut diagnostics = Vec::new();
    if !wiki_dir.exists() {
        diagnostics.push(WikiDiagnostic {
            code: "wiki-live-missing".into(),
            severity: "error".into(),
            path: Some(wiki_dir.to_path_buf()),
            message: "code live wiki directory does not exist".into(),
        });
        return WikiCheckReport {
            out_dir: wiki_dir.to_path_buf(),
            diagnostics,
        };
    }

    for required in [
        "_index.md",
        "_architecture.md",
        "_patterns.md",
        "_log.md",
        "_meta.json",
        "architecture/inventory.json",
        "architecture/workspace.mmd",
        "architecture/modules.mmd",
    ] {
        if !wiki_dir.join(required).exists() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-live-required-file-missing".into(),
                severity: "error".into(),
                path: Some(PathBuf::from(required)),
                message: format!("required live wiki file is missing: {required}"),
            });
        }
    }

    let (expected_index, index_diagnostics) = render_wiki_index(wiki_dir);
    diagnostics.extend(index_diagnostics);
    match std::fs::read_to_string(wiki_dir.join("_index.md")) {
        Ok(actual_index) if actual_index != expected_index => diagnostics.push(WikiDiagnostic {
            code: "wiki-index-stale".into(),
            severity: "error".into(),
            path: Some(PathBuf::from("_index.md")),
            message: "live wiki index is stale; run `agent-spec wiki index`".into(),
        }),
        Ok(_) => {}
        Err(_) => {}
    }

    let (articles, article_diagnostics) = collect_wiki_articles(wiki_dir);
    diagnostics.extend(article_diagnostics);
    for article in articles {
        if article.source_files.is_empty() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-source-files-missing".into(),
                severity: "error".into(),
                path: Some(article.path.clone()),
                message: "live wiki articles must declare source_files frontmatter".into(),
            });
        }
        for source in &article.source_files {
            diagnostics.extend(validate_source_file(root, &article.path, source));
        }
        diagnostics.extend(check_article_links(wiki_dir, &article.path));
    }
    let project_map = build_project_map(root, wiki_dir);
    diagnostics.extend(project_map.diagnostics.clone());
    diagnostics.extend(project_map_artifact_diagnostics(wiki_dir, &project_map));

    WikiCheckReport {
        out_dir: wiki_dir.to_path_buf(),
        diagnostics,
    }
}

pub fn write_project_map_artifacts(
    root: &Path,
    wiki_dir: &Path,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let map = build_project_map(root, wiki_dir);
    write_project_map_artifacts_from_map(wiki_dir, &map)
}

fn write_project_map_artifacts_from_map(
    wiki_dir: &Path,
    map: &crate::spec_wiki::WikiProjectMap,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    std::fs::create_dir_all(wiki_dir.join("architecture"))?;
    let json_path = PathBuf::from("architecture/project-map.json");
    let mermaid_path = PathBuf::from("architecture/project-map.mmd");
    std::fs::write(
        wiki_dir.join(&json_path),
        serde_json::to_string_pretty(map)?,
    )?;
    std::fs::write(
        wiki_dir.join(&mermaid_path),
        render_project_map_mermaid(map),
    )?;
    files.push(json_path);
    files.push(mermaid_path);
    Ok(files)
}

fn project_map_artifact_diagnostics(
    wiki_dir: &Path,
    map: &crate::spec_wiki::WikiProjectMap,
) -> Vec<WikiDiagnostic> {
    let expected_json = match serde_json::to_string_pretty(map) {
        Ok(json) => json,
        Err(err) => {
            return vec![WikiDiagnostic {
                code: "wiki-project-map-json-render-failed".into(),
                severity: "error".into(),
                path: Some(PathBuf::from("architecture/project-map.json")),
                message: format!("project-map JSON could not be rendered: {err}"),
            }];
        }
    };
    let expected_mermaid = render_project_map_mermaid(map);
    let mut diagnostics = Vec::new();
    compare_project_map_artifact(
        wiki_dir,
        "architecture/project-map.json",
        &expected_json,
        "wiki-project-map-json-missing",
        "wiki-project-map-json-drift",
        &mut diagnostics,
    );
    compare_project_map_artifact(
        wiki_dir,
        "architecture/project-map.mmd",
        &expected_mermaid,
        "wiki-project-map-mermaid-missing",
        "wiki-project-map-mermaid-drift",
        &mut diagnostics,
    );
    diagnostics
}

fn compare_project_map_artifact(
    wiki_dir: &Path,
    relative: &str,
    expected: &str,
    missing_code: &str,
    drift_code: &str,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let path = wiki_dir.join(relative);
    match std::fs::read_to_string(&path) {
        Ok(actual) if actual != expected => diagnostics.push(WikiDiagnostic {
            code: drift_code.into(),
            severity: "error".into(),
            path: Some(PathBuf::from(relative)),
            message: format!("derived project-map artifact drifted: {relative}"),
        }),
        Ok(_) => {}
        Err(_) => diagnostics.push(WikiDiagnostic {
            code: missing_code.into(),
            severity: "error".into(),
            path: Some(PathBuf::from(relative)),
            message: format!("derived project-map artifact is missing: {relative}"),
        }),
    }
}

pub fn check_live_wiki(root: &Path, wiki_dir: &Path) -> WikiCheckReport {
    let mut report = lint_live_wiki(root, wiki_dir);
    let status = wiki_status(root, wiki_dir);
    report.diagnostics.extend(status.diagnostics);
    report
        .diagnostics
        .extend(stale_article_diagnostics(&status.stale_articles));
    report
}

pub fn check_live_wiki_with_changed_paths(
    root: &Path,
    wiki_dir: &Path,
    changed_files: &[PathBuf],
) -> WikiCheckReport {
    let mut report = lint_live_wiki(root, wiki_dir);
    let status = status_from_changed_paths(wiki_dir, changed_files);
    report.diagnostics.extend(status.diagnostics);
    report
        .diagnostics
        .extend(stale_article_diagnostics(&status.stale_articles));
    report
}

fn stale_article_diagnostics(stale_articles: &[WikiStaleArticle]) -> Vec<WikiDiagnostic> {
    stale_articles
        .iter()
        .map(|article| WikiDiagnostic {
            code: "wiki-article-stale".into(),
            severity: "error".into(),
            path: Some(article.path.clone()),
            message: format!(
                "live wiki article is stale for changed sources: {}",
                article
                    .changed_files
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
        .collect()
}

fn validate_source_file(root: &Path, article_path: &Path, source: &Path) -> Vec<WikiDiagnostic> {
    let mut diagnostics = Vec::new();
    match repo_path_issue(root, source) {
        Some(RepoPathIssue::Absolute) => diagnostics.push(WikiDiagnostic {
            code: "wiki-source-file-absolute".into(),
            severity: "error".into(),
            path: Some(article_path.to_path_buf()),
            message: format!(
                "live wiki source_files entries must be repo-relative: {}",
                source.display()
            ),
        }),
        Some(RepoPathIssue::ParentTraversal | RepoPathIssue::OutsideRoot) => {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-source-file-outside-root".into(),
                severity: "error".into(),
                path: Some(article_path.to_path_buf()),
                message: format!(
                    "live wiki source_files entry points outside the repo: {}",
                    source.display()
                ),
            });
        }
        Some(RepoPathIssue::Missing) => diagnostics.push(WikiDiagnostic {
            code: "wiki-source-file-missing".into(),
            severity: "error".into(),
            path: Some(source.to_path_buf()),
            message: "live wiki source file does not exist".into(),
        }),
        None => {}
    }
    diagnostics
}

fn check_article_links(wiki_dir: &Path, article_path: &Path) -> Vec<WikiDiagnostic> {
    let path = wiki_dir.join(article_path);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut diagnostics = Vec::new();
    for target in markdown_link_targets(&content) {
        let Some(target_path) = normalize_internal_link_target(&target) else {
            continue;
        };
        let base = article_path.parent().unwrap_or_else(|| Path::new(""));
        let candidate = wiki_dir.join(base).join(&target_path);
        if !candidate.exists() || !path_is_inside(wiki_dir, &candidate) {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-internal-link-broken".into(),
                severity: "error".into(),
                path: Some(article_path.to_path_buf()),
                message: format!("live wiki article links to missing target: {target}"),
            });
        }
    }
    diagnostics
}

fn markdown_link_targets(content: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("](") {
        let target_start = start + 2;
        let Some(end) = rest[target_start..].find(')') else {
            break;
        };
        let target = rest[target_start..target_start + end].trim();
        if !target.is_empty() {
            targets.push(target.to_string());
        }
        rest = &rest[target_start + end + 1..];
    }
    targets
}

fn normalize_internal_link_target(target: &str) -> Option<PathBuf> {
    if target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("data:")
    {
        return None;
    }
    let target = target
        .split('#')
        .next()
        .unwrap_or(target)
        .split('?')
        .next()
        .unwrap_or(target)
        .trim();
    if target.is_empty() {
        None
    } else {
        Some(PathBuf::from(target))
    }
}

fn path_is_inside(root: &Path, path: &Path) -> bool {
    match (root.canonicalize(), path.canonicalize()) {
        (Ok(root), Ok(path)) => path.strip_prefix(root).is_ok(),
        _ => false,
    }
}

pub fn collect_wiki_articles(wiki_dir: &Path) -> (Vec<WikiArticle>, Vec<WikiDiagnostic>) {
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    collect_markdown_files(wiki_dir, wiki_dir, &mut files, &mut diagnostics);
    files.sort();
    let mut articles = Vec::new();
    for rel in files {
        let file_name = rel.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if file_name == "_index.md" || file_name == "_log.md" {
            continue;
        }
        let path = wiki_dir.join(&rel);
        match std::fs::read_to_string(&path) {
            Ok(content) => match parse_article_frontmatter(&rel, &content) {
                Some(article) => articles.push(article),
                None => diagnostics.push(WikiDiagnostic {
                    code: "wiki-frontmatter-missing".into(),
                    severity: "error".into(),
                    path: Some(rel),
                    message: "live wiki article is missing frontmatter".into(),
                }),
            },
            Err(err) => diagnostics.push(WikiDiagnostic {
                code: "wiki-article-unreadable".into(),
                severity: "error".into(),
                path: Some(rel),
                message: format!("live wiki article could not be read: {err}"),
            }),
        }
    }
    (articles, diagnostics)
}

fn render_architecture_article(inventory: &ArchitectureInventory) -> String {
    let source_files = frontmatter_source_files(&inventory.source_files);
    format!(
        "---\ntitle: \"Architecture\"\ntype: architecture\nsource_files:\n{source_files}---\n# Architecture\n\n- Inventory: [architecture/inventory.json](architecture/inventory.json)\n- Workspace diagram: [architecture/workspace.mmd](architecture/workspace.mmd)\n- Module diagram: [architecture/modules.mmd](architecture/modules.mmd)\n- Project map data: [architecture/project-map.json](architecture/project-map.json)\n- Project map diagram: [architecture/project-map.mmd](architecture/project-map.mmd)\n- Provider: `{}`\n- Packages: {}\n- Dependencies: {}\n- Modules: {}\n- Module edges: {}\n",
        inventory.provider,
        inventory.packages.len(),
        inventory.dependencies.len(),
        inventory.modules.len(),
        inventory.module_edges.len()
    )
}

fn render_patterns_article(inventory: &ArchitectureInventory) -> String {
    let source_files = frontmatter_source_files(&inventory.source_files);
    "---\ntitle: \"Patterns\"\ntype: patterns\nsource_files:\n".to_string()
        + &source_files
        + "---\n# Patterns\n\nCapture cross-cutting implementation patterns here as they become durable.\n"
}

#[derive(Clone)]
struct SeedPageSpec {
    path: PathBuf,
    title: String,
    article_type: String,
    source_files: Vec<PathBuf>,
    tags: Vec<String>,
    status: String,
    body: String,
}

fn seed_page_specs(root: &Path) -> Vec<SeedPageSpec> {
    let candidates = [
        seed_page(
            "modules/main-cli.md",
            "Main CLI",
            "module",
            &["src/main.rs"],
            &["cli", "commands"],
            "Primary command dispatch and text/json formatting entrypoint.",
        ),
        seed_page(
            "modules/spec-parser.md",
            "Spec Parser",
            "module",
            &["src/spec_parser"],
            &["parser", "contract"],
            "Task Contract parsing, frontmatter parsing, and inheritance resolution.",
        ),
        seed_page(
            "modules/spec-lint.md",
            "Spec Lint",
            "module",
            &["src/spec_lint"],
            &["lint", "quality"],
            "Spec quality analysis and contract smell detection.",
        ),
        seed_page(
            "modules/spec-verify.md",
            "Spec Verify",
            "module",
            &["src/spec_verify"],
            &["verification", "lifecycle"],
            "Mechanical and inferential scenario verification.",
        ),
        seed_page(
            "modules/spec-knowledge.md",
            "Spec Knowledge",
            "module",
            &["src/spec_knowledge"],
            &["kll", "requirements"],
            "Knowledge liveness layer, intent compiler, trace, and governance.",
        ),
        seed_page(
            "modules/spec-wiki.md",
            "Spec Wiki",
            "module",
            &["src/spec_wiki"],
            &["wiki", "architecture"],
            "Repo-local code live wiki, architecture inventory, and source trace checks.",
        ),
        seed_page(
            "modules/spec-archive.md",
            "Spec Archive",
            "module",
            &["src/spec_archive.rs"],
            &["archive", "contracts"],
            "Archival summary and completed-spec compression.",
        ),
        seed_page(
            "concepts/task-contract.md",
            "Task Contract",
            "concept",
            &[
                "README.md",
                "AGENTS.md",
                "skills/agent-spec-tool-first/SKILL.md",
            ],
            &["contract", "workflow"],
            "Human-authored task contract that defines intent, decisions, boundaries, and completion criteria.",
        ),
        seed_page(
            "concepts/knowledge-liveness-layer.md",
            "Knowledge Liveness Layer",
            "concept",
            &["knowledge/requirements", "src/spec_knowledge"],
            &["kll", "liveness"],
            "Long-lived requirements and decisions with traceable liveness evidence.",
        ),
        seed_page(
            "concepts/intent-compiler.md",
            "Intent Compiler",
            "concept",
            &[
                "knowledge/requirements/req-requirements-compiler-plan-dag.md",
                "src/spec_knowledge/requirement_plan.rs",
            ],
            &["requirements", "compiler", "dag"],
            "Compiler-style lowering from KLL requirements into plans, work units, specs, and trace evidence.",
        ),
        seed_page(
            "concepts/lifecycle.md",
            "Lifecycle",
            "concept",
            &["src/spec_gateway/lifecycle.rs", "src/spec_verify"],
            &["lifecycle", "verification"],
            "Full lint and verification loop for a Task Contract.",
        ),
        seed_page(
            "concepts/trace-replay.md",
            "Trace And Replay",
            "concept",
            &[
                "src/spec_knowledge/trace_ledger.rs",
                "src/spec_knowledge/trace.rs",
            ],
            &["trace", "replay"],
            "Requirement-to-spec-to-scenario evidence chain and replay surfaces.",
        ),
        seed_page(
            "concepts/wiki-working-memory.md",
            "Wiki Working Memory",
            "concept",
            &[
                "skills/agent-spec-wiki/SKILL.md",
                ".agent-spec/wiki/_index.md",
            ],
            &["wiki", "working-memory"],
            "Maintained agent-readable wiki pages backed by source_files, not durable KLL truth.",
        ),
        seed_page(
            "decisions/knowledge-vs-docs.md",
            "Knowledge Versus Docs",
            "decision",
            &["skills/agent-spec-wiki/SKILL.md", "AGENTS.md"],
            &["knowledge", "docs", "wiki"],
            "Durable truth belongs in knowledge/, executable contracts in specs/, human docs in docs/, and agent working memory in .agent-spec/wiki/.",
        ),
        seed_page(
            "decisions/wiki-path.md",
            "Wiki Path",
            "decision",
            &["knowledge/requirements/req-code-live-wiki.md", ".gitignore"],
            &["wiki", "git"],
            ".agent-spec/wiki is trackable live wiki state while other .agent-spec runtime outputs stay ignored.",
        ),
        seed_page(
            "decisions/deterministic-cli.md",
            "Deterministic CLI",
            "decision",
            &["specs/task-code-live-wiki.spec.md", "src/spec_wiki"],
            &["deterministic", "non-goal"],
            "The CLI performs deterministic local analysis only; no LLM or network call writes wiki prose.",
        ),
    ];
    candidates
        .into_iter()
        .map(|mut spec| {
            spec.source_files
                .retain(|source| root.join(source).exists());
            spec
        })
        .filter(|spec| !spec.source_files.is_empty())
        .collect()
}

fn seed_page(
    path: &str,
    title: &str,
    article_type: &str,
    source_files: &[&str],
    tags: &[&str],
    summary: &str,
) -> SeedPageSpec {
    SeedPageSpec {
        path: PathBuf::from(path),
        title: title.into(),
        article_type: article_type.into(),
        source_files: source_files.iter().map(PathBuf::from).collect(),
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        status: "draft".into(),
        body: format!(
            "# {title}\n\n## Role\n\n{summary}\n\n## Maintenance\n\nUpdate this page when any listed `source_files` change in a way that alters the project understanding an agent should reuse.\n"
        ),
    }
}

fn render_seed_page(spec: &SeedPageSpec) -> String {
    let mut out = format!(
        "---\ntitle: \"{}\"\ntype: {}\nsource_files:\n",
        escape_frontmatter_string(&spec.title),
        spec.article_type
    );
    out.push_str(&frontmatter_source_files(&spec.source_files));
    out.push_str("tags:\n");
    for tag in &spec.tags {
        out.push_str(&format!("  - {}\n", tag));
    }
    out.push_str(&format!("status: {}\n---\n\n{}", spec.status, spec.body));
    out
}

fn escape_frontmatter_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn frontmatter_source_files(paths: &[PathBuf]) -> String {
    let mut out = String::new();
    if paths.is_empty() {
        out.push_str("  - Cargo.toml\n");
        return out;
    }
    for path in paths {
        out.push_str(&format!("  - {}\n", path_to_slash(path)));
    }
    out
}

fn write_file(
    wiki_dir: &Path,
    rel: &Path,
    content: &str,
    files_written: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = wiki_dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    files_written.push(rel.to_path_buf());
    Ok(())
}

fn build_meta(root: &Path) -> WikiMeta {
    let resolved_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    WikiMeta {
        project: resolved_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repository")
            .into(),
        repo_path: PathBuf::from("."),
        generator: "agent-spec code live wiki".into(),
        generator_version: env!("CARGO_PKG_VERSION").into(),
        last_compiled_commit: git_output(root, &["rev-parse", "HEAD"]),
        last_compiled_at: None,
    }
}

fn read_wiki_meta(wiki_dir: &Path) -> Option<WikiMeta> {
    let content = std::fs::read_to_string(wiki_dir.join("_meta.json")).ok()?;
    serde_json::from_str(&content).ok()
}

fn worktree_changed_files(root: &Path, diagnostics: &mut Vec<WikiDiagnostic>) -> Vec<PathBuf> {
    let mut changed = BTreeSet::new();
    for path in git_path_list(
        root,
        &["diff", "--relative", "--name-only", "--", "."],
        "wiki-git-worktree-diff-unavailable",
        "wiki-git-worktree-diff-failed",
        diagnostics,
    ) {
        changed.insert(path);
    }
    for path in git_path_list(
        root,
        &["diff", "--relative", "--name-only", "--cached", "--", "."],
        "wiki-git-staged-diff-unavailable",
        "wiki-git-staged-diff-failed",
        diagnostics,
    ) {
        changed.insert(path);
    }
    for path in git_path_list(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", "."],
        "wiki-git-untracked-unavailable",
        "wiki-git-untracked-failed",
        diagnostics,
    ) {
        changed.insert(path);
    }
    changed.into_iter().collect()
}

fn git_path_list(
    root: &Path,
    args: &[&str],
    unavailable_code: &str,
    failed_code: &str,
    diagnostics: &mut Vec<WikiDiagnostic>,
) -> Vec<PathBuf> {
    let output = Command::new("git").args(args).current_dir(root).output();
    let Ok(output) = output else {
        diagnostics.push(WikiDiagnostic {
            code: unavailable_code.into(),
            severity: "warning".into(),
            path: Some(root.to_path_buf()),
            message: format!("git {} could not be executed", args.join(" ")),
        });
        return Vec::new();
    };
    if !output.status.success() {
        diagnostics.push(WikiDiagnostic {
            code: failed_code.into(),
            severity: "warning".into(),
            path: Some(root.to_path_buf()),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
        return Vec::new();
    }
    let root_prefix = git_output(root, &["rev-parse", "--show-prefix"]).unwrap_or_default();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| strip_git_path_prefix(line, &root_prefix))
        .collect()
}

fn strip_git_path_prefix(path: &str, prefix: &str) -> Option<PathBuf> {
    let prefix = prefix.trim_start_matches("./");
    if prefix.is_empty() {
        return Some(PathBuf::from(path));
    }
    Some(
        path.strip_prefix(prefix)
            .filter(|relative| !relative.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path)),
    )
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn collect_markdown_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
    diagnostics: &mut Vec<WikiDiagnostic>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let Ok(file_type) = entry.file_type() else {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-article-type-unreadable".into(),
                severity: "error".into(),
                path: Some(rel),
                message: "wiki article file type could not be read".into(),
            });
            continue;
        };
        if file_type.is_symlink() {
            diagnostics.push(WikiDiagnostic {
                code: "wiki-article-symlink-rejected".into(),
                severity: "error".into(),
                path: Some(rel),
                message: "live wiki traversal rejects symbolic links".into(),
            });
        } else if file_type.is_dir() {
            collect_markdown_files(root, &path, out, diagnostics);
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md")
        {
            out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
}

fn parse_article_frontmatter(path: &Path, content: &str) -> Option<WikiArticle> {
    let frontmatter = frontmatter_body(content)?;
    let mut title = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled")
        .to_string();
    let mut article_type = "article".to_string();
    let mut source_files = Vec::new();
    let mut tags = Vec::new();
    let mut list_key: Option<&str> = None;

    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            match list_key {
                Some("source_files") => source_files.push(PathBuf::from(unquote(item))),
                Some("tags") => tags.push(unquote(item)),
                _ => {}
            }
            continue;
        }
        list_key = None;
        if let Some(value) = trimmed.strip_prefix("title:") {
            title = unquote(value.trim());
        } else if let Some(value) = trimmed.strip_prefix("type:") {
            article_type = unquote(value.trim());
        } else if trimmed == "source_files:" {
            list_key = Some("source_files");
        } else if trimmed == "tags:" {
            list_key = Some("tags");
        }
    }

    Some(WikiArticle {
        path: path.to_path_buf(),
        title,
        article_type,
        source_files,
        tags,
    })
}

fn frontmatter_body(content: &str) -> Option<&str> {
    let rest = content.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    Some(&rest[..end])
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("\\\"", "\"")
        .replace("\\\\", "\\")
}

fn normalize_path(path: &Path) -> String {
    path_to_slash(path)
}

fn path_overlaps(source: &str, changed: &str) -> bool {
    source == changed
        || changed
            .strip_prefix(source)
            .is_some_and(|rest| rest.starts_with('/'))
        || source
            .strip_prefix(changed)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn repo_relative_input(root: &Path, input: &Path) -> PathBuf {
    if input.is_absolute()
        && let (Ok(root), Ok(input)) = (root.canonicalize(), input.canonicalize())
        && let Ok(rel) = input.strip_prefix(root)
    {
        return rel.to_path_buf();
    }
    input.to_path_buf()
}

fn matching_requirements(root: &Path, input_path: &Path) -> Vec<WikiRequirementLink> {
    let knowledge_dir = root.join("knowledge");
    let input_text = normalize_path(input_path);
    let collection = crate::spec_knowledge::collect_knowledge_checked(&knowledge_dir);
    let mut out = Vec::new();
    for doc in collection.docs {
        if doc.meta.kind != crate::spec_knowledge::KnowledgeKind::Requirement {
            continue;
        }
        let source_path = repo_relative_input(root, &doc.source_path);
        let source_text = normalize_path(&source_path);
        let body_matches = doc.sections.iter().any(|section| {
            section
                .body
                .lines()
                .any(|line| line.contains(&input_text) || line.contains(&source_text))
        });
        if path_overlaps(&source_text, &input_text) || body_matches {
            out.push(WikiRequirementLink {
                id: doc.meta.id,
                title: doc.meta.title.unwrap_or_else(|| source_text.clone()),
                path: source_path,
            });
        }
    }
    out.sort_by(|left, right| left.id.cmp(&right.id).then(left.path.cmp(&right.path)));
    out
}

fn matching_specs(root: &Path, requirement_ids: &BTreeSet<String>) -> Vec<WikiSpecLink> {
    let specs_dir = root.join("specs");
    let mut out = Vec::new();
    for spec_path in spec_files(&specs_dir) {
        let Ok(doc) = crate::spec_parser::parse_spec(&spec_path) else {
            continue;
        };
        if doc
            .meta
            .satisfies
            .iter()
            .any(|id| requirement_ids.contains(id))
        {
            out.push(WikiSpecLink {
                name: doc.meta.name,
                path: repo_relative_input(root, &spec_path),
                satisfies: doc.meta.satisfies,
            });
        }
    }
    out.sort_by(|left, right| left.path.cmp(&right.path));
    out
}

fn matching_trace_records(root: &Path, input_path: &Path) -> Vec<WikiTraceLink> {
    let trace_dir = root.join(".agent-spec/trace");
    let ledger = crate::spec_knowledge::read_requirement_trace_ledgers(&trace_dir);
    let input_text = normalize_path(input_path);
    let mut out = Vec::new();
    for record in ledger.records {
        let requirement_source =
            normalize_path(&repo_relative_input(root, &record.requirement_source));
        let spec_path = normalize_path(&repo_relative_input(root, &record.spec_path));
        let code_targets = record
            .code_targets
            .iter()
            .map(|target| normalize_path(Path::new(target)))
            .collect::<Vec<_>>();
        let matched = path_overlaps(&requirement_source, &input_text)
            || path_overlaps(&spec_path, &input_text)
            || code_targets
                .iter()
                .any(|target| path_overlaps(target, &input_text));
        if matched {
            out.push(WikiTraceLink {
                run_id: record.run_id,
                requirement_id: record.requirement_id,
                work_unit_id: record.work_unit_id,
                spec_path: record.spec_path,
                scenario_name: record.scenario_name,
                test_selector: record.test_selector,
                verdict: format!("{:?}", record.verdict).to_ascii_lowercase(),
                timestamp: record.timestamp,
            });
        }
    }
    out.sort_by(|left, right| {
        left.requirement_id
            .cmp(&right.requirement_id)
            .then(left.timestamp.cmp(&right.timestamp))
            .then(left.scenario_name.cmp(&right.scenario_name))
    });
    out
}

fn spec_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_spec_files(dir, &mut out);
    out.sort();
    out
}

fn collect_spec_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if !matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some(".agent-spec" | "_archive" | "archive")
            ) {
                collect_spec_files(&path, out);
            }
        } else if file_type.is_file() {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if name.ends_with(".spec.md") || name.ends_with(".spec") {
                out.push(path);
            }
        }
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiki_meta_repo_path_is_portable() {
        let meta = build_meta(Path::new("."));

        assert_eq!(meta.repo_path, PathBuf::from("."));
        assert!(!meta.repo_path.is_absolute());
        assert_eq!(meta.last_compiled_at, None);
    }

    #[test]
    fn test_strip_git_prefix_accepts_root_or_subdir_relative_paths() {
        assert_eq!(
            strip_git_path_prefix("fixtures/wiki-mini/src/lib.rs", "fixtures/wiki-mini/"),
            Some(PathBuf::from("src/lib.rs"))
        );
        assert_eq!(
            strip_git_path_prefix("src/lib.rs", "fixtures/wiki-mini/"),
            Some(PathBuf::from("src/lib.rs"))
        );
        assert_eq!(
            strip_git_path_prefix("src/main.rs", ""),
            Some(PathBuf::from("src/main.rs"))
        );
    }

    #[test]
    fn test_status_does_not_mark_article_stale_when_article_is_updated_with_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(format!(
            "agent-spec-wiki-updated-article-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("modules"))?;
        std::fs::write(
            dir.join("modules/compiler.md"),
            "---\ntitle: Compiler\ntype: module\nsource_files:\n  - src/compiler.rs\n---\n# Compiler\n",
        )?;

        let stale = status_from_changed_paths(&dir, &[PathBuf::from("src/compiler.rs")]);
        assert_eq!(stale.stale_articles.len(), 1);

        let updated = status_from_changed_paths(
            &dir,
            &[
                PathBuf::from("src/compiler.rs"),
                PathBuf::from(".agent-spec/wiki/modules/compiler.md"),
            ],
        );
        assert!(updated.stale_articles.is_empty());
        let _ = std::fs::remove_dir_all(dir);
        Ok(())
    }
}
