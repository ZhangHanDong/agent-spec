//! Deterministic PRD/issue requirement block intake.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementImportBlock {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub body: String,
    pub source_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementImportError {
    pub message: String,
}

impl fmt::Display for RequirementImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RequirementImportError {}

pub fn parse_requirement_blocks(
    input: &str,
    source_name: &str,
) -> Result<Vec<RequirementImportBlock>, RequirementImportError> {
    let mut blocks = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut rest = input;
    while let Some(start) = rest.find("<!-- agent-spec:requirement") {
        rest = &rest[start..];
        let Some(header_end) = rest.find("-->") else {
            return Err(err("requirement block opening marker is not closed"));
        };
        let header = &rest["<!-- agent-spec:requirement".len()..header_end];
        let after_header = &rest[header_end + "-->".len()..];
        let Some(close_start) = after_header.find("<!-- /agent-spec:requirement -->") else {
            return Err(err("requirement block closing marker is missing"));
        };
        let body = after_header[..close_start].trim().to_string();
        let attrs = parse_attrs(header);
        let id = required_attr(&attrs, "id")?.to_ascii_uppercase();
        crate::spec_knowledge::validate_knowledge_id(&id)
            .map_err(|message| RequirementImportError { message })?;
        if !ids.insert(id.clone()) {
            return Err(err(&format!("duplicate requirement id: {id}")));
        }
        let title = required_attr(&attrs, "title")?.to_string();
        let tags = optional_attr(&attrs, "tags")
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let source = optional_attr(&attrs, "source").map(str::to_string);

        blocks.push(RequirementImportBlock {
            id,
            title,
            tags,
            source,
            body,
            source_name: source_name.to_string(),
        });
        rest = &after_header[close_start + "<!-- /agent-spec:requirement -->".len()..];
    }
    Ok(blocks)
}

pub fn render_requirement_artifact(block: &RequirementImportBlock) -> String {
    let tags = if block.tags.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", block.tags.join(", "))
    };
    let mut body = block.body.trim().to_string();
    if let Some(source) = &block.source
        && !body.contains("## Source Trace")
    {
        let source_trace = format!("\n## Source Trace\n\n- {source}\n");
        if let Some(open_questions_pos) = body.find("## Open Questions") {
            body.insert_str(open_questions_pos, &source_trace);
        } else {
            body.push_str(&source_trace);
        }
    }

    format!(
        "---\nkind: requirement\nid: {}\ntitle: \"{}\"\nstatus: proposed\nliveness: auto\ntags: {}\n---\n\n{}\n",
        block.id,
        escape_title(&block.title),
        tags,
        body
    )
}

pub fn requirement_artifact_filename(block: &RequirementImportBlock) -> String {
    let slug = slugify(&block.title);
    let slug = if slug.is_empty() {
        "requirement".to_string()
    } else {
        slug
    };
    format!("{}-{slug}.md", block.id.to_ascii_lowercase())
}

fn err(message: &str) -> RequirementImportError {
    RequirementImportError {
        message: message.to_string(),
    }
}

fn required_attr<'a>(
    attrs: &'a [(String, String)],
    key: &str,
) -> Result<&'a str, RequirementImportError> {
    optional_attr(attrs, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| err(&format!("{key} is required")))
}

fn optional_attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

fn parse_attrs(input: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if key_start == i {
            break;
        }
        let key = input[key_start..i].trim().to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            break;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            let value = input[value_start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
            value
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            input[value_start..i].trim().to_string()
        };
        attrs.push((key, value));
    }
    attrs
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn escape_title(input: &str) -> String {
    input.replace('"', "\\\"")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_requirements_import_parses_block_and_renders_artifact() {
        let input = r#"Intro.
<!-- agent-spec:requirement id=REQ-101 title="User Login" tags=auth,web source=issue:#123 -->
## Problem

Users with existing accounts need to authenticate.

## Requirements

[REQ-101] The authentication service MUST create a login session when valid credentials are submitted.

## Scenarios

Scenario: Valid login
  Given the visitor has a valid persisted account
  When the visitor submits valid credentials
  Then the system establishes a login session

## Dependencies

- REQ-100

## Source Trace

- issue:#123

## Open Questions

None.
<!-- /agent-spec:requirement -->
"#;

        let blocks = parse_requirement_blocks(input, "issue-123.md").unwrap();
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.id, "REQ-101");
        assert_eq!(block.title, "User Login");
        assert_eq!(block.tags, vec!["auth", "web"]);
        assert_eq!(block.source, Some("issue:#123".to_string()));
        assert!(block.body.contains("## Requirements"));

        let rendered = render_requirement_artifact(block);
        assert!(rendered.contains("kind: requirement"));
        assert!(rendered.contains("id: REQ-101"));
        assert!(rendered.contains("title: \"User Login\""));
        assert!(rendered.contains("tags: [auth, web]"));
        assert!(
            rendered.contains("[REQ-101] The authentication service MUST create a login session")
        );
        assert!(
            rendered.find("## Source Trace").unwrap() < rendered.find("## Open Questions").unwrap(),
            "Source Trace must appear before Open Questions"
        );
        assert_eq!(
            requirement_artifact_filename(block),
            "req-101-user-login.md"
        );
    }

    #[test]
    fn test_requirements_import_rejects_missing_id() {
        let input = r#"<!-- agent-spec:requirement title="User Login" -->
## Problem

p

## Requirements

[REQ-101] The service MUST authenticate users.
<!-- /agent-spec:requirement -->"#;

        let err = parse_requirement_blocks(input, "bad.md").unwrap_err();
        assert!(err.to_string().contains("id is required"));
    }

    #[test]
    fn test_requirements_import_rejects_path_like_and_duplicate_ids() {
        for id in ["../../escape", "/tmp/escape", "REQ--EMPTY", "REQ.BAD"] {
            let input = format!(
                "<!-- agent-spec:requirement id={id} title=Escape -->\n## Problem\np\n<!-- /agent-spec:requirement -->"
            );
            assert!(parse_requirement_blocks(&input, "prd.md").is_err(), "{id}");
        }

        let duplicate = "<!-- agent-spec:requirement id=REQ-A title=First -->\nA\n<!-- /agent-spec:requirement -->\n<!-- agent-spec:requirement id=REQ-A title=Second -->\nB\n<!-- /agent-spec:requirement -->";
        assert!(parse_requirement_blocks(duplicate, "prd.md").is_err());
    }
}
