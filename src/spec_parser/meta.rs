use crate::spec_core::{Lang, SpecLevel, SpecMeta};

/// Parse front-matter block (before `---`) into SpecMeta.
pub fn parse_meta(lines: &[&str]) -> Result<SpecMeta, String> {
    let mut level = None;
    let mut name = None;
    let mut inherits = None;
    let mut lang = Vec::new();
    let mut tags = Vec::new();

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
    })
}

#[cfg(test)]
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
}
