use crate::spec_core::{Constraint, ResolvedSpec, Scenario, Section, SpecDocument, SpecError, SpecResult};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::parser::parse_spec;

/// Resolve a spec document by loading and merging its inheritance chain.
///
/// `search_dirs` is a list of directories to search for parent spec files.
/// Files are matched by name: `{inherits_value}.spec`
pub fn resolve_spec(doc: SpecDocument, search_dirs: &[&Path]) -> SpecResult<ResolvedSpec> {
    let owned_search_dirs;
    let search_dirs: Vec<&Path> = if search_dirs.is_empty() {
        owned_search_dirs = default_search_dirs(&doc.source_path);
        owned_search_dirs.iter().map(PathBuf::as_path).collect()
    } else {
        search_dirs.to_vec()
    };

    let mut chain = Vec::new();
    let mut visited = HashSet::new();

    // Collect the inheritance chain
    let mut current = doc.clone();
    while let Some(ref parent_name) = current.meta.inherits {
        if !visited.insert(parent_name.clone()) {
            let chain_str = visited.into_iter().collect::<Vec<_>>().join(" -> ");
            return Err(SpecError::CircularInheritance { chain: chain_str });
        }

        let parent_doc = find_and_parse_spec(parent_name, &search_dirs)?;
        chain.push(parent_doc.clone());
        current = parent_doc;
    }

    // Merge inheritable content: ancestors first, then task's own.
    let mut inherited_constraints = Vec::new();
    let mut inherited_decisions = Vec::new();
    for ancestor in chain.iter().rev() {
        inherited_constraints.extend(extract_constraints(ancestor));
        inherited_decisions.extend(extract_decisions(ancestor));
    }

    // Collect all scenarios from the task
    let all_scenarios = extract_scenarios(&doc);

    Ok(ResolvedSpec {
        task: doc,
        inherited_constraints,
        inherited_decisions,
        all_scenarios,
    })
}

fn default_search_dirs(source_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    let Some(mut current) = source_path.parent() else {
        return dirs;
    };

    loop {
        if seen.insert(current.to_path_buf()) {
            dirs.push(current.to_path_buf());
        }

        let specs_dir = current.join("specs");
        if specs_dir.is_dir() && seen.insert(specs_dir.clone()) {
            dirs.push(specs_dir);
        }

        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
    }

    dirs
}

fn find_and_parse_spec(name: &str, search_dirs: &[&Path]) -> SpecResult<SpecDocument> {
    let candidates = [
        format!("{name}.spec"),
        format!("{name}-spec.spec"),
        "org.spec".to_string(),
        "project.spec".to_string(),
    ];

    for dir in search_dirs {
        for candidate in &candidates {
            let path = dir.join(candidate);
            if path.exists() {
                return parse_spec(&path);
            }
        }
    }

    Err(SpecError::InheritanceNotFound {
        name: name.to_string(),
    })
}

fn extract_constraints(doc: &SpecDocument) -> Vec<Constraint> {
    doc.sections
        .iter()
        .filter_map(|s| match s {
            Section::Constraints { items, .. } => Some(items.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn extract_scenarios(doc: &SpecDocument) -> Vec<Scenario> {
    doc.sections
        .iter()
        .filter_map(|s| match s {
            Section::AcceptanceCriteria { scenarios, .. } => Some(scenarios.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn extract_decisions(doc: &SpecDocument) -> Vec<String> {
    doc.sections
        .iter()
        .filter_map(|s| match s {
            Section::Decisions { items, .. } => Some(items.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec_core::SpecLevel;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolves_parent_from_source_directory_when_no_search_dirs_are_provided() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-resolver-{stamp}"));
        fs::create_dir_all(&root).unwrap();

        let project_path = root.join("project.spec");
        fs::write(
            &project_path,
            r#"spec: project
name: "项目"
---

## 约束

### 禁止做
- 禁止使用 `panic!`
"#,
        )
        .unwrap();

        let task_path = root.join("task.spec");
        fs::write(
            &task_path,
            r#"spec: task
name: "任务"
inherits: project
---

## 意图

实现功能。

## 验收标准

场景: 正常路径
  假设 输入有效
  当 调用函数
  那么 返回 Ok
"#,
        )
        .unwrap();

        let doc = parse_spec(&task_path).unwrap();
        let resolved = resolve_spec(doc, &[]).unwrap();

        assert_eq!(resolved.task.meta.level, SpecLevel::Task);
        assert_eq!(resolved.inherited_constraints.len(), 1);
        assert_eq!(resolved.inherited_constraints[0].text, "禁止使用 `panic!`");
        assert!(resolved.inherited_decisions.is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_parent_from_nested_spec_directory_via_ancestor_specs_dir() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("agent-spec-resolver-nested-{stamp}"));
        let specs_dir = root.join("specs");
        let roadmap_dir = specs_dir.join("roadmap");
        fs::create_dir_all(&roadmap_dir).unwrap();

        let project_path = specs_dir.join("project.spec");
        fs::write(
            &project_path,
            r#"spec: project
name: "项目"
---

## 约束

### 必须做
- 必须保留顶层项目规则
"#,
        )
        .unwrap();

        let task_path = roadmap_dir.join("task.spec");
        fs::write(
            &task_path,
            r#"spec: task
name: "路线图任务"
inherits: project
---

## 意图

把路线图任务放在嵌套目录中。

## 完成条件

场景: 正常路径
  假设 存在顶层 `project.spec`
  当 从 `specs/roadmap/task.spec` 加载任务
  那么 继承链继续生效
"#,
        )
        .unwrap();

        let doc = parse_spec(&task_path).unwrap();
        let resolved = resolve_spec(doc, &[]).unwrap();

        assert_eq!(resolved.task.meta.level, SpecLevel::Task);
        assert_eq!(resolved.inherited_constraints.len(), 1);
        assert_eq!(resolved.inherited_constraints[0].text, "必须保留顶层项目规则");
        assert!(resolved.inherited_decisions.is_empty());

        let _ = fs::remove_dir_all(root);
    }
}
