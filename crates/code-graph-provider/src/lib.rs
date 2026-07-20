//! Provider-neutral producer contract for external Code Graph adapters.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PROVIDER_IR_VERSION: u32 = 1;
pub const PROVIDER_MANIFEST_SCHEMA: &str = "agent-spec/code-graph-provider/manifest-v1";
pub const PROVIDER_REGISTRATION_SCHEMA: &str =
    "agent-spec/code-graph-provider/registration-v1";

#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ProviderError {
    code: &'static str,
    message: String,
}

impl ProviderError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderRole {
    Extractor,
    SemanticEnricher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderCapability {
    Nodes,
    Containment,
    BasicReferences,
    SemanticEdges,
    QueryHints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartupProtocol {
    StdioJsonV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaRange {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceLimits {
    pub timeout_ms: u64,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub max_diagnostics: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderManifest {
    pub schema: String,
    pub provider_id: String,
    pub provider_version: String,
    pub language: String,
    pub ir_schema: SchemaRange,
    pub role: ProviderRole,
    pub capabilities: BTreeSet<ProviderCapability>,
    pub startup: StartupProtocol,
    pub freshness_inputs: Vec<String>,
    pub limits: ResourceLimits,
    pub deterministic: bool,
    pub supports_no_daemon: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderRegistration {
    pub schema: String,
    pub provider_id: String,
    #[serde(default)]
    pub enabled: bool,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
}

pub fn validate_manifest(manifest: &ProviderManifest) -> Result<(), ProviderError> {
    if manifest.schema != PROVIDER_MANIFEST_SCHEMA
        || !valid_identifier(&manifest.provider_id)
        || !valid_identifier(&manifest.language)
        || manifest.provider_version.trim().is_empty()
        || manifest.provider_version.len() > 64
    {
        return Err(ProviderError::new(
            "provider-manifest",
            "manifest schema, provider identity, language, or version is invalid",
        ));
    }
    if manifest.ir_schema.min == 0
        || manifest.ir_schema.min > manifest.ir_schema.max
        || !(manifest.ir_schema.min..=manifest.ir_schema.max).contains(&PROVIDER_IR_VERSION)
    {
        return Err(ProviderError::new(
            "provider-manifest-schema",
            format!(
                "provider schema range {}..={} does not include IR v{PROVIDER_IR_VERSION}",
                manifest.ir_schema.min, manifest.ir_schema.max
            ),
        ));
    }
    let valid_capabilities = match manifest.role {
        ProviderRole::Extractor => {
            manifest.capabilities.contains(&ProviderCapability::Nodes)
                && manifest
                    .capabilities
                    .contains(&ProviderCapability::Containment)
                && manifest.capabilities.iter().all(|capability| {
                    matches!(
                        capability,
                        ProviderCapability::Nodes
                            | ProviderCapability::Containment
                            | ProviderCapability::BasicReferences
                    )
                })
        }
        ProviderRole::SemanticEnricher => {
            manifest.capabilities.iter().any(|capability| {
                matches!(
                    capability,
                    ProviderCapability::SemanticEdges | ProviderCapability::QueryHints
                )
            }) && manifest.capabilities.iter().all(|capability| {
                matches!(
                    capability,
                    ProviderCapability::SemanticEdges | ProviderCapability::QueryHints
                )
            })
        }
    };
    if !valid_capabilities {
        return Err(ProviderError::new(
            "provider-manifest-capability",
            "provider capabilities are missing or incompatible with its role",
        ));
    }
    if manifest.freshness_inputs.is_empty()
        || manifest
            .freshness_inputs
            .iter()
            .any(|pattern| !valid_freshness_pattern(pattern))
    {
        return Err(ProviderError::new(
            "provider-manifest-freshness",
            "freshness inputs must be non-empty normalized repository-relative patterns",
        ));
    }
    let limits = &manifest.limits;
    if !(10..=300_000).contains(&limits.timeout_ms)
        || !(256..=64 * 1024 * 1024).contains(&limits.max_stdout_bytes)
        || !(256..=16 * 1024 * 1024).contains(&limits.max_stderr_bytes)
        || !(1..=10_000).contains(&limits.max_diagnostics)
    {
        return Err(ProviderError::new(
            "provider-manifest-limit",
            "provider resource limits are outside the supported bounded range",
        ));
    }
    if !manifest.deterministic || !manifest.supports_no_daemon {
        return Err(ProviderError::new(
            "provider-manifest-mode",
            "F1 providers must declare deterministic and no-daemon support",
        ));
    }
    Ok(())
}

pub fn validate_registration(
    manifest: &ProviderManifest,
    registration: &ProviderRegistration,
) -> Result<(), ProviderError> {
    validate_manifest(manifest)?;
    if registration.schema != PROVIDER_REGISTRATION_SCHEMA
        || registration.provider_id != manifest.provider_id
    {
        return Err(ProviderError::new(
            "provider-registration",
            "registration schema or provider identity does not match the manifest",
        ));
    }
    if !registration.enabled {
        return Err(ProviderError::new(
            "provider-disabled",
            format!("provider {} is not enabled for this project", manifest.provider_id),
        ));
    }
    if registration.executable.trim().is_empty()
        || registration.executable.contains('\0')
        || registration.args.iter().any(|arg| arg.contains('\0'))
        || registration
            .cwd
            .as_deref()
            .is_some_and(|cwd| cwd.trim().is_empty() || cwd.contains('\0'))
    {
        return Err(ProviderError::new(
            "provider-registration",
            "registration executable, argv, or cwd is invalid",
        ));
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
}

fn valid_freshness_pattern(pattern: &str) -> bool {
    !pattern.trim().is_empty()
        && !pattern.starts_with('/')
        && !pattern.contains('\\')
        && pattern
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn limits() -> ResourceLimits {
        ResourceLimits {
            timeout_ms: 1_000,
            max_stdout_bytes: 64 * 1024,
            max_stderr_bytes: 16 * 1024,
            max_diagnostics: 32,
        }
    }

    fn extractor_manifest() -> ProviderManifest {
        ProviderManifest {
            schema: PROVIDER_MANIFEST_SCHEMA.to_string(),
            provider_id: "fixture-extractor".to_string(),
            provider_version: "1.0.0".to_string(),
            language: "fixture".to_string(),
            ir_schema: SchemaRange { min: 1, max: 1 },
            role: ProviderRole::Extractor,
            capabilities: BTreeSet::from([
                ProviderCapability::Nodes,
                ProviderCapability::Containment,
                ProviderCapability::BasicReferences,
            ]),
            startup: StartupProtocol::StdioJsonV1,
            freshness_inputs: vec!["src/**".to_string()],
            limits: limits(),
            deterministic: true,
            supports_no_daemon: true,
        }
    }

    #[test]
    fn test_provider_sdk_stays_rust_atlas_independent() {
        let manifest = include_str!("../Cargo.toml");
        assert!(!manifest.contains("rust-atlas"));
        assert!(!manifest.contains("path = \"../..\""));
    }

    #[test]
    fn test_manifest_validates_role_schema_capabilities_and_limits() {
        validate_manifest(&extractor_manifest()).unwrap();

        let mut invalid_role = extractor_manifest();
        invalid_role
            .capabilities
            .insert(ProviderCapability::SemanticEdges);
        assert_eq!(
            validate_manifest(&invalid_role).unwrap_err().code(),
            "provider-manifest-capability"
        );

        let mut invalid_schema = extractor_manifest();
        invalid_schema.ir_schema = SchemaRange { min: 2, max: 3 };
        assert_eq!(
            validate_manifest(&invalid_schema).unwrap_err().code(),
            "provider-manifest-schema"
        );

        let mut unbounded = extractor_manifest();
        unbounded.limits.max_stdout_bytes = usize::MAX;
        assert_eq!(
            validate_manifest(&unbounded).unwrap_err().code(),
            "provider-manifest-limit"
        );
    }

    #[test]
    fn test_registration_is_opt_in_and_uses_literal_argv() {
        let manifest = extractor_manifest();
        let mut registration = ProviderRegistration {
            schema: PROVIDER_REGISTRATION_SCHEMA.to_string(),
            provider_id: manifest.provider_id.clone(),
            enabled: false,
            executable: "/opt/provider/bin/extract".to_string(),
            args: vec!["--flag=value with spaces".to_string()],
            cwd: Some(".".to_string()),
        };
        assert_eq!(
            validate_registration(&manifest, &registration)
                .unwrap_err()
                .code(),
            "provider-disabled"
        );

        registration.enabled = true;
        validate_registration(&manifest, &registration).unwrap();
        assert_eq!(registration.args, ["--flag=value with spaces"]);

        registration.executable.clear();
        assert_eq!(
            validate_registration(&manifest, &registration)
                .unwrap_err()
                .code(),
            "provider-registration"
        );
    }
}
