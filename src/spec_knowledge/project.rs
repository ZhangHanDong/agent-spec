//! Project guidance artifacts into tool-native formats (§6.4 consumption).
//! Feeds `gen-integrations`: collects `knowledge/guidance/` docs and renders a
//! Markdown block that can be appended to CLAUDE.md / AGENTS.md / etc.

use crate::spec_knowledge::guidance::{applies_to, applies_to_path, skills};
use crate::spec_knowledge::model::{KnowledgeDoc, KnowledgeKind};
use crate::spec_knowledge::parser::parse_knowledge;
use std::path::{Path, PathBuf};

/// Collect all `guidance` docs under `<knowledge>/guidance/` (recursive,
/// best-effort: unparseable / wrong-kind files are skipped), sorted by id.
pub fn collect_guidance(knowledge_dir: &Path) -> Vec<KnowledgeDoc> {
    let dir = knowledge_dir.join("guidance");
    let mut files = Vec::new();
    collect_md(&dir, &mut files);
    files.sort();
    let mut docs: Vec<KnowledgeDoc> = files
        .iter()
        .filter_map(|p| parse_knowledge(p).ok())
        .filter(|d| d.meta.kind == KnowledgeKind::Guidance)
        .collect();
    docs.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
    docs
}

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

/// Render guidance as a Markdown block. When `path` is `Some`, only guidance
/// applying to that path is included; `None` includes all.
pub fn render_guidance_md(docs: &[KnowledgeDoc], path: Option<&str>) -> String {
    let selected: Vec<&KnowledgeDoc> = docs
        .iter()
        .filter(|d| path.map(|p| applies_to_path(d, p)).unwrap_or(true))
        .collect();
    if selected.is_empty() {
        return String::new();
    }
    let mut s =
        String::from("<!-- agent-spec:guidance -->\n## Guidance (from knowledge/guidance)\n\n");
    for d in selected {
        let scope = d
            .section("Scope")
            .map(|x| x.body.trim())
            .unwrap_or_default();
        s.push_str(&format!("### {} — {}\n\n", d.meta.id, scope));
        if let Some(instr) = d.section("Instructions") {
            s.push_str(instr.body.trim());
            s.push_str("\n\n");
        }
        let globs = applies_to(d);
        if !globs.is_empty() {
            s.push_str(&format!("_Applies to:_ {}\n\n", globs.join(", ")));
        }
        let sk = skills(d);
        if !sk.is_empty() {
            s.push_str(&format!("_Skills:_ {}\n\n", sk.join(", ")));
        }
    }
    s.push_str("<!-- /agent-spec:guidance -->\n");
    s
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_and_render_guidance() {
        let dir = std::env::temp_dir().join(format!("kll-proj-{}", std::process::id()));
        let g = dir.join("guidance");
        std::fs::create_dir_all(&g).unwrap();
        std::fs::write(
            g.join("g-001-rust.md"),
            "---\nkind: guidance\nid: G-001\nliveness: n/a\n---\n## Scope\nRust modules\n## Instructions\nPrefer ? over unwrap.\n## Applies To\nsrc/**\n## Skills\n- tdd\n",
        )
        .unwrap();
        // a non-guidance file is ignored
        std::fs::write(g.join("README.md"), "# Guidance\n").unwrap();

        let docs = collect_guidance(&dir);
        assert_eq!(docs.len(), 1);

        let all = render_guidance_md(&docs, None);
        assert!(all.contains("G-001"));
        assert!(all.contains("Prefer ? over unwrap."));
        assert!(all.contains("agent-spec:guidance"));

        // path filter
        assert!(render_guidance_md(&docs, Some("src/main.rs")).contains("G-001"));
        assert!(render_guidance_md(&docs, Some("docs/x.md")).is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }
}
