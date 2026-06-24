//! Deterministic MCP tool implementations (§10). Thin read layer over the
//! knowledge parser, the satisfies index, trace/liveness, and SpecGateway. No
//! RAG, no model calls — every answer is a pure function of the files on disk.

use crate::spec_knowledge::context::{list_context, read_context};
use crate::spec_knowledge::guidance::{applies_to, applies_to_path, applies_to_stack, skills};
use crate::spec_knowledge::index::build_satisfies_index;
use crate::spec_knowledge::model::{KnowledgeDoc, KnowledgeKind};
use crate::spec_knowledge::parser::parse_knowledge;
use crate::spec_knowledge::trace::{build_trace, verify_spec_rollup};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Roots the MCP tools read from.
#[derive(Debug, Clone)]
pub struct McpContext {
    pub knowledge: PathBuf,
    pub specs: PathBuf,
    pub code: PathBuf,
}

const ALL_KINDS: [KnowledgeKind; 4] = [
    KnowledgeKind::Decision,
    KnowledgeKind::Requirement,
    KnowledgeKind::Guidance,
    KnowledgeKind::Proposal,
];

/// Tool name -> JSON-Schema description, for `tools/list`.
pub fn tool_specs() -> Value {
    json!([
        { "name": "knowledge.find", "description": "Find knowledge artifacts by id, tag, or path.",
          "inputSchema": { "type": "object", "properties": {
            "id": {"type": "string"}, "tag": {"type": "string"}, "path": {"type": "string"} } } },
        { "name": "knowledge.governing", "description": "Decisions governing a code path (via satisfying-spec boundaries) plus current liveness.",
          "inputSchema": { "type": "object", "properties": { "path": {"type": "string"} }, "required": ["path"] } },
        { "name": "liveness.status", "description": "Current liveness of a decision id.",
          "inputSchema": { "type": "object", "properties": { "id": {"type": "string"} }, "required": ["id"] } },
        { "name": "spec.contract", "description": "The task contract for a spec by name.",
          "inputSchema": { "type": "object", "properties": { "name": {"type": "string"} }, "required": ["name"] } },
        { "name": "guidance.for", "description": "Guidance and designated skills for a path or stack.",
          "inputSchema": { "type": "object", "properties": { "path": {"type": "string"}, "stack": {"type": "string"} } } },
        { "name": "context.read", "description": "Read free-form context by path, or list all when no path is given.",
          "inputSchema": { "type": "object", "properties": { "path": {"type": "string"} } } }
    ])
}

/// Dispatch a tool call. Returns the tool's structured JSON payload.
pub fn dispatch(name: &str, args: &Value, ctx: &McpContext) -> Result<Value, String> {
    match name {
        "knowledge.find" => knowledge_find(args, ctx),
        "knowledge.governing" => knowledge_governing(args, ctx),
        "liveness.status" => liveness_status(args, ctx),
        "spec.contract" => spec_contract(args, ctx),
        "guidance.for" => guidance_for(args, ctx),
        "context.read" => context_read(args, ctx),
        other => Err(format!("unknown tool '{other}'")),
    }
}

fn arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
}

// ── collectors ──────────────────────────────────────────────────

fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_md(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("md")
            && p.file_name().and_then(|n| n.to_str()) != Some("README.md")
        {
            out.push(p);
        }
    }
}

/// All knowledge docs under `knowledge/`, sorted by id.
fn collect_all(knowledge: &Path) -> Vec<KnowledgeDoc> {
    let mut files = Vec::new();
    for kind in ALL_KINDS {
        collect_md(&knowledge.join(kind.dir()), &mut files);
    }
    files.sort();
    let mut docs: Vec<KnowledgeDoc> = files
        .iter()
        .filter_map(|p| parse_knowledge(p).ok())
        .collect();
    docs.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
    docs
}

fn find_by_id(knowledge: &Path, id: &str) -> Option<KnowledgeDoc> {
    let target = id.to_ascii_uppercase();
    collect_all(knowledge)
        .into_iter()
        .find(|d| d.meta.id == target)
}

fn doc_summary(d: &KnowledgeDoc) -> Value {
    json!({
        "id": d.meta.id,
        "kind": d.meta.kind,
        "status": d.meta.status,
        "liveness_declared": d.meta.liveness,
        "tags": d.meta.tags,
        "path": d.source_path.display().to_string(),
    })
}

// ── tools ───────────────────────────────────────────────────────

fn knowledge_find(args: &Value, ctx: &McpContext) -> Result<Value, String> {
    let docs = collect_all(&ctx.knowledge);
    let id = arg(args, "id").map(|s| s.to_ascii_uppercase());
    let tag = arg(args, "tag");
    let path = arg(args, "path");

    let hits: Vec<Value> = docs
        .iter()
        .filter(|d| id.as_deref().is_none_or(|i| d.meta.id == i))
        .filter(|d| tag.is_none_or(|t| d.meta.tags.iter().any(|x| x.eq_ignore_ascii_case(t))))
        .filter(|d| {
            path.is_none_or(|p| {
                // guidance: scope match; others: source path contains the query
                if d.meta.kind == KnowledgeKind::Guidance {
                    applies_to_path(d, p)
                } else {
                    d.source_path.display().to_string().contains(p)
                }
            })
        })
        .map(doc_summary)
        .collect();
    Ok(json!({ "results": hits }))
}

fn knowledge_governing(args: &Value, ctx: &McpContext) -> Result<Value, String> {
    let path = arg(args, "path").ok_or("missing 'path'")?;
    let index = build_satisfies_index(&ctx.specs);

    let mut governing = Vec::new();
    for (decision_id, spec_paths) in &index {
        // Which satisfying specs have an allow-boundary covering `path`?
        let via: Vec<String> = spec_paths
            .iter()
            .filter(|sp| spec_allows_path(sp, path))
            .map(|sp| sp.display().to_string())
            .collect();
        if via.is_empty() {
            continue;
        }
        let Some(decision) = find_by_id(&ctx.knowledge, decision_id) else {
            continue;
        };
        let report = build_trace(&decision, &index, |sp| verify_spec_rollup(sp, &ctx.code));
        governing.push(json!({
            "id": decision.meta.id,
            "liveness": report.liveness,
            "via_specs": via,
        }));
    }
    Ok(json!({ "path": path, "governing": governing }))
}

/// Whether a spec's task contract has an Allow boundary glob matching `path`.
fn spec_allows_path(spec_path: &Path, path: &str) -> bool {
    let Ok(gw) = crate::spec_gateway::SpecGateway::load(spec_path) else {
        return false;
    };
    gw.contract()
        .allowed_changes
        .iter()
        .any(|g| crate::spec_knowledge::guidance::glob_match(g, path))
}

fn liveness_status(args: &Value, ctx: &McpContext) -> Result<Value, String> {
    let id = arg(args, "id").ok_or("missing 'id'")?;
    let decision = find_by_id(&ctx.knowledge, id)
        .ok_or_else(|| format!("no decision with id {}", id.to_ascii_uppercase()))?;
    let index = build_satisfies_index(&ctx.specs);
    let report = build_trace(&decision, &index, |sp| verify_spec_rollup(sp, &ctx.code));
    serde_json::to_value(&report).map_err(|e| e.to_string())
}

fn spec_contract(args: &Value, ctx: &McpContext) -> Result<Value, String> {
    let name = arg(args, "name").ok_or("missing 'name'")?;
    let spec_path = resolve_spec(&ctx.specs, name)
        .ok_or_else(|| format!("no spec named '{name}' under {}", ctx.specs.display()))?;
    let gw = crate::spec_gateway::SpecGateway::load(&spec_path).map_err(|e| e.to_string())?;
    serde_json::to_value(gw.contract()).map_err(|e| e.to_string())
}

fn resolve_spec(specs: &Path, name: &str) -> Option<PathBuf> {
    let mut files = Vec::new();
    collect_spec_files(specs, &mut files);
    files.sort();
    files.into_iter().find(|p| {
        let stem = p
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.trim_end_matches(".md").trim_end_matches(".spec"))
            .unwrap_or_default();
        stem == name || stem == format!("{name}.spec") || stem.trim_end_matches(".spec") == name
    })
}

fn collect_spec_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_spec_files(&p, out);
        } else {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if name.ends_with(".spec.md") || name.ends_with(".spec") {
                out.push(p);
            }
        }
    }
}

fn guidance_for(args: &Value, ctx: &McpContext) -> Result<Value, String> {
    let path = arg(args, "path");
    let stack = arg(args, "stack");
    let docs = crate::spec_knowledge::collect_guidance(&ctx.knowledge);
    let hits: Vec<Value> = docs
        .iter()
        .filter(|d| {
            let p_ok = path.map(|p| applies_to_path(d, p)).unwrap_or(false);
            let s_ok = stack.map(|s| applies_to_stack(d, s)).unwrap_or(false);
            // no filter given -> all; otherwise any matching dimension
            (path.is_none() && stack.is_none()) || p_ok || s_ok
        })
        .map(|d| {
            json!({
                "id": d.meta.id,
                "scope": d.section("Scope").map(|s| s.body.trim()).unwrap_or_default(),
                "instructions": d.section("Instructions").map(|s| s.body.trim()).unwrap_or_default(),
                "applies_to": applies_to(d),
                "skills": skills(d),
            })
        })
        .collect();
    Ok(json!({ "guidance": hits }))
}

fn context_read(args: &Value, ctx: &McpContext) -> Result<Value, String> {
    let dir = ctx.knowledge.join("context");
    match arg(args, "path") {
        Some(p) => {
            let content = read_context(&dir, p)?;
            Ok(json!({ "path": p, "content": content }))
        }
        None => Ok(json!({ "files": list_context(&dir) })),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> (PathBuf, McpContext) {
        let root = std::env::temp_dir().join(format!("kll-mcp-{}-{tag}", std::process::id()));
        let knowledge = root.join("knowledge");
        let specs = root.join("specs");
        std::fs::create_dir_all(knowledge.join("decisions")).unwrap();
        std::fs::create_dir_all(knowledge.join("guidance")).unwrap();
        std::fs::create_dir_all(knowledge.join("context")).unwrap();
        std::fs::create_dir_all(&specs).unwrap();
        std::fs::write(
            knowledge.join("decisions/adr-001-x.md"),
            "---\nkind: decision\nid: ADR-001\nstatus: accepted\n---\n## Context\nc\n## Decision\nd\n## Consequences\nGood. Bad.\n",
        )
        .unwrap();
        std::fs::write(
            knowledge.join("guidance/g-001-rust.md"),
            "---\nkind: guidance\nid: G-001\nliveness: n/a\ntags: [rust]\n---\n## Scope\nrust\n## Instructions\nprefer ?\n## Applies To\nsrc/**\n## Skills\n- tdd\n",
        )
        .unwrap();
        std::fs::write(knowledge.join("context/notes.md"), "freeform").unwrap();
        std::fs::write(
            specs.join("task-a.spec.md"),
            "spec: task\nname: \"A\"\nsatisfies: [ADR-001]\n---\n## Intent\nx\n## Boundaries\n\n### Allowed Changes\n- src/**\n",
        )
        .unwrap();
        let ctx = McpContext {
            knowledge,
            specs,
            code: root.clone(),
        };
        (root, ctx)
    }

    #[test]
    fn test_find_by_id_and_tag() {
        let (root, ctx) = fixture("find");
        let r = dispatch("knowledge.find", &json!({"id": "adr-001"}), &ctx).unwrap();
        assert_eq!(r["results"].as_array().unwrap().len(), 1);
        let r = dispatch("knowledge.find", &json!({"tag": "rust"}), &ctx).unwrap();
        assert_eq!(r["results"][0]["id"], "G-001");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_guidance_for_path_and_context_read() {
        let (root, ctx) = fixture("guidance");
        let r = dispatch("guidance.for", &json!({"path": "src/main.rs"}), &ctx).unwrap();
        assert_eq!(r["guidance"][0]["id"], "G-001");
        assert_eq!(r["guidance"][0]["skills"][0], "tdd");

        let r = dispatch("context.read", &json!({"path": "notes.md"}), &ctx).unwrap();
        assert_eq!(r["content"], "freeform");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_governing_and_liveness() {
        let (root, ctx) = fixture("gov");
        let r = dispatch("knowledge.governing", &json!({"path": "src/lib.rs"}), &ctx).unwrap();
        assert_eq!(r["governing"][0]["id"], "ADR-001");
        let r = dispatch("liveness.status", &json!({"id": "ADR-001"}), &ctx).unwrap();
        assert_eq!(r["decision_id"], "ADR-001");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn test_unknown_tool_errors() {
        let (root, ctx) = fixture("unknown");
        assert!(dispatch("nope.nope", &json!({}), &ctx).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
