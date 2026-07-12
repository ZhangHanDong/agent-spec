use crate::spec_core::{Lang, SpecLevel, SpecMeta};

/// Parse front-matter block (before `---`) into SpecMeta.
pub fn parse_meta(lines: &[&str]) -> Result<SpecMeta, String> {
    let mut level = None;
    let mut name = None;
    let mut inherits = None;
    let mut lang = Vec::new();
    let mut tags = Vec::new();
    let mut depends = Vec::new();
    let mut estimate = None;
    let mut capability = None;
    let mut satisfies = Vec::new();
    let mut risk = None;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim().trim_matches('"');

        match key.as_str() {
            "spec" => {
                level = Some(match value.to_lowercase().as_str() {
                    "org" => SpecLevel::Org,
                    "project" => SpecLevel::Project,
                    "capability" => SpecLevel::Capability,
                    "task" => SpecLevel::Task,
                    other => return Err(format!("unknown spec level: {other}")),
                });
            }
            "name" => {
                name = Some(value.to_string());
            }
            "inherits" => {
                let v = value.trim();
                if !v.is_empty() {
                    inherits = Some(v.to_string());
                }
            }
            "lang" => {
                for part in value.split(',') {
                    match part.trim().to_lowercase().as_str() {
                        "zh" => lang.push(Lang::Zh),
                        "en" => lang.push(Lang::En),
                        _ => {}
                    }
                }
            }
            "tags" => {
                let value = value.trim_start_matches('[').trim_end_matches(']');
                for tag in value.split(',') {
                    let t = tag.trim();
                    if !t.is_empty() {
                        tags.push(t.to_string());
                    }
                }
            }
            "depends" => {
                let value = value.trim_start_matches('[').trim_end_matches(']');
                for dep in value.split(',') {
                    let d = dep.trim();
                    if !d.is_empty() {
                        depends.push(d.to_string());
                    }
                }
            }
            "estimate" => {
                let v = value.trim();
                if !v.is_empty() {
                    estimate = Some(v.to_string());
                }
            }
            "capability" => {
                let v = value.trim();
                if !v.is_empty() {
                    capability = Some(v.to_string());
                }
            }
            "satisfies" => {
                let value = value.trim_start_matches('[').trim_end_matches(']');
                for id in value.split(',') {
                    let s = id.trim().trim_matches('"').to_ascii_uppercase();
                    if !s.is_empty() {
                        satisfies.push(s);
                    }
                }
            }
            "risk" => {
                let v = value.trim();
                if !v.is_empty() {
                    risk = Some(v.to_ascii_uppercase());
                }
            }
            _ => {} // ignore unknown keys
        }
    }

    Ok(SpecMeta {
        level: level.ok_or("missing 'spec:' field in front-matter")?,
        name: name.unwrap_or_else(|| "unnamed".to_string()),
        inherits,
        lang: if lang.is_empty() {
            vec![Lang::Zh, Lang::En]
        } else {
            lang
        },
        tags,
        depends,
        estimate,
        capability,
        satisfies,
        risk,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_meta() {
        let lines = vec![
            "spec: task",
            r#"name: "退款功能""#,
            "inherits: project",
            "tags: [payment, refund]",
            "lang: zh",
        ];
        let meta = parse_meta(&lines).unwrap();
        assert_eq!(meta.level, SpecLevel::Task);
        assert_eq!(meta.name, "退款功能");
        assert_eq!(meta.inherits, Some("project".into()));
        assert_eq!(meta.tags, vec!["payment", "refund"]);
        assert_eq!(meta.lang, vec![Lang::Zh]);
    }

    #[test]
    fn test_parse_minimal_meta() {
        let lines = vec!["spec: org"];
        let meta = parse_meta(&lines).unwrap();
        assert_eq!(meta.level, SpecLevel::Org);
        assert_eq!(meta.name, "unnamed");
        assert!(meta.inherits.is_none());
        assert_eq!(meta.lang, vec![Lang::Zh, Lang::En]);
    }

    #[test]
    fn test_parse_spec_depends_and_estimate_fields() {
        let lines = vec![
            "spec: task",
            r#"name: "依赖图测试""#,
            "inherits: project",
            "tags: [bootstrap]",
            "depends: [task-goal-gate]",
            "estimate: 3d",
        ];
        let meta = parse_meta(&lines).unwrap();
        assert_eq!(meta.depends, vec!["task-goal-gate"]);
        assert_eq!(meta.estimate, Some("3d".to_string()));
    }

    #[test]
    fn test_parse_meta_multiple_depends() {
        let lines = vec![
            "spec: task",
            r#"name: "多依赖""#,
            "depends: [task-a, task-b, task-c]",
        ];
        let meta = parse_meta(&lines).unwrap();
        assert_eq!(meta.depends, vec!["task-a", "task-b", "task-c"]);
    }

    #[test]
    fn test_parse_spec_risk_class_field() {
        let lines = vec!["spec: task", r#"name: "High Risk""#, "risk: A"];
        let meta = parse_meta(&lines).unwrap();
        assert_eq!(meta.risk.as_deref(), Some("A"));
    }

    #[test]
    fn test_parse_meta_no_depends_no_estimate() {
        let lines = vec!["spec: task", r#"name: "无依赖""#];
        let meta = parse_meta(&lines).unwrap();
        assert!(meta.depends.is_empty());
        assert!(meta.estimate.is_none());
    }

    // ---- Phase 3: capability ----

    #[test]
    fn test_parse_capability_spec_level() {
        let meta = parse_meta(&["spec: capability", r#"name: "ecosystem-import""#]).unwrap();
        assert_eq!(meta.level, SpecLevel::Capability);
        assert_eq!(meta.name, "ecosystem-import");
    }

    #[test]
    fn test_unknown_spec_level_rejected() {
        let err = parse_meta(&["spec: nonsense"]).unwrap_err();
        assert!(err.contains("unknown spec level"), "got: {err}");
    }

    #[test]
    fn test_parse_task_capability_field() {
        let meta =
            parse_meta(&["spec: task", r#"name: "t""#, "capability: ecosystem-import"]).unwrap();
        assert_eq!(meta.capability, Some("ecosystem-import".to_string()));
    }

    // ---- KLL: satisfies edge ----

    #[test]
    fn test_parse_satisfies_array() {
        let lines = vec![
            "spec: task",
            r#"name: "X""#,
            "satisfies: [adr-001, REQ-002]",
        ];
        let meta = parse_meta(&lines).unwrap();
        assert_eq!(
            meta.satisfies,
            vec!["ADR-001".to_string(), "REQ-002".to_string()]
        );
    }

    #[test]
    fn test_satisfies_defaults_empty() {
        let lines = vec!["spec: task", r#"name: "X""#];
        let meta = parse_meta(&lines).unwrap();
        assert!(meta.satisfies.is_empty());
    }
}
