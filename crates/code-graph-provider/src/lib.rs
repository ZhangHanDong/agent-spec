//! Provider-neutral producer contract for external Code Graph adapters.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PROVIDER_IR_VERSION: u32 = 1;
pub const PROVIDER_MANIFEST_SCHEMA: &str = "agent-spec/code-graph-provider/manifest-v1";
pub const PROVIDER_REGISTRATION_SCHEMA: &str =
    "agent-spec/code-graph-provider/registration-v1";
pub const EXTRACTION_PAYLOAD_SCHEMA: &str =
    "agent-spec/code-graph-provider/extraction-payload-v1";
pub const EXTRACTION_ARTIFACT_SCHEMA: &str =
    "agent-spec/code-graph-provider/extraction-artifact-v1";
pub const ENRICHMENT_PAYLOAD_SCHEMA: &str =
    "agent-spec/code-graph-provider/enrichment-payload-v1";
pub const ENRICHMENT_ARTIFACT_SCHEMA: &str =
    "agent-spec/code-graph-provider/enrichment-artifact-v1";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceConfidence {
    Exact,
    Candidate,
    Heuristic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvidence {
    pub extractor: String,
    pub extractor_version: String,
    pub evidence: String,
    pub confidence: EvidenceConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FreshnessState {
    Fresh,
    Stale,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshnessFact {
    pub path: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshnessReport {
    pub state: FreshnessState,
    pub inputs: Vec<FreshnessFact>,
    #[serde(default)]
    pub affected_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpan {
    pub line_start: u32,
    pub column_start: u32,
    pub line_end: u32,
    pub column_end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderNode {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub span: Option<SourceSpan>,
    pub provenance: ProviderEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub provenance: ProviderEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderQueryHint {
    pub node_id: String,
    pub kind: String,
    pub message: String,
    pub provenance: ProviderEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionPayload {
    pub schema: String,
    pub provider_id: String,
    pub provider_version: String,
    pub language: String,
    pub worktree_id: String,
    pub freshness: FreshnessReport,
    #[serde(default)]
    pub nodes: Vec<ProviderNode>,
    #[serde(default)]
    pub edges: Vec<ProviderEdge>,
    #[serde(default)]
    pub diagnostics: Vec<ProviderDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionArtifact {
    pub schema: String,
    pub ir_version: u32,
    pub graph_fingerprint: String,
    pub payload: ExtractionPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrichmentPayload {
    pub schema: String,
    pub provider_id: String,
    pub provider_version: String,
    pub language: String,
    pub worktree_id: String,
    pub base_graph_fingerprint: String,
    #[serde(default)]
    pub edges: Vec<ProviderEdge>,
    #[serde(default)]
    pub query_hints: Vec<ProviderQueryHint>,
    #[serde(default)]
    pub diagnostics: Vec<ProviderDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrichmentArtifact {
    pub schema: String,
    pub ir_version: u32,
    pub enrichment_fingerprint: String,
    pub payload: EnrichmentPayload,
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

pub fn project_extraction(
    manifest: &ProviderManifest,
    expected_worktree: &str,
    mut payload: ExtractionPayload,
) -> Result<ExtractionArtifact, ProviderError> {
    validate_manifest(manifest)?;
    if manifest.role != ProviderRole::Extractor {
        return Err(ProviderError::new(
            "provider-role",
            "an extraction payload requires an extractor manifest",
        ));
    }
    validate_payload_identity(
        manifest,
        expected_worktree,
        &payload.schema,
        EXTRACTION_PAYLOAD_SCHEMA,
        &payload.provider_id,
        &payload.provider_version,
        &payload.language,
        &payload.worktree_id,
    )?;
    validate_freshness(&mut payload.freshness, &payload.diagnostics)?;
    validate_diagnostics(manifest, &mut payload.diagnostics)?;

    payload.nodes.sort_by(|left, right| left.id.cmp(&right.id));
    let mut node_ids = BTreeSet::new();
    for node in &payload.nodes {
        if !node.id.starts_with(&format!("{}:", manifest.provider_id))
            || node.id.len() == manifest.provider_id.len() + 1
            || !node_ids.insert(node.id.as_str())
        {
            return Err(ProviderError::new(
                "provider-node-id",
                format!("node id `{}` is not unique and provider-scoped", node.id),
            ));
        }
        if node.name.trim().is_empty() || !valid_identifier(&node.kind) {
            return Err(ProviderError::new(
                "provider-node",
                format!("node `{}` has an invalid name or kind", node.id),
            ));
        }
        validate_repository_path(&node.path)?;
        validate_span(node.span.as_ref())?;
        validate_evidence(&node.provenance)?;
    }

    payload.edges.sort_by(edge_order);
    let mut edge_keys = BTreeSet::new();
    for edge in &payload.edges {
        let key = (&edge.from, &edge.to, &edge.kind);
        if !edge_keys.insert(key) {
            return Err(ProviderError::new(
                "provider-edge",
                "duplicate extraction edge",
            ));
        }
        if !node_ids.contains(edge.from.as_str())
            || (!node_ids.contains(edge.to.as_str())
                && !edge.to.starts_with("external:")
                && !edge.to.starts_with("unresolved:"))
            || !valid_identifier(&edge.kind)
        {
            return Err(ProviderError::new(
                "provider-edge",
                format!("edge {} -> {} is invalid", edge.from, edge.to),
            ));
        }
        if edge.kind == "contains"
            && !manifest
                .capabilities
                .contains(&ProviderCapability::Containment)
            || edge.kind != "contains"
                && !manifest
                    .capabilities
                    .contains(&ProviderCapability::BasicReferences)
        {
            return Err(ProviderError::new(
                "provider-manifest-capability",
                format!("edge kind `{}` was not declared", edge.kind),
            ));
        }
        validate_evidence(&edge.provenance)?;
    }

    let graph_fingerprint = canonical_fingerprint(&(PROVIDER_IR_VERSION, &payload))?;
    Ok(ExtractionArtifact {
        schema: EXTRACTION_ARTIFACT_SCHEMA.to_string(),
        ir_version: PROVIDER_IR_VERSION,
        graph_fingerprint,
        payload,
    })
}

pub fn project_enrichment(
    manifest: &ProviderManifest,
    expected_worktree: &str,
    mut payload: EnrichmentPayload,
) -> Result<EnrichmentArtifact, ProviderError> {
    validate_manifest(manifest)?;
    if manifest.role != ProviderRole::SemanticEnricher {
        return Err(ProviderError::new(
            "provider-role",
            "an enrichment payload requires a semantic-enricher manifest",
        ));
    }
    validate_payload_identity(
        manifest,
        expected_worktree,
        &payload.schema,
        ENRICHMENT_PAYLOAD_SCHEMA,
        &payload.provider_id,
        &payload.provider_version,
        &payload.language,
        &payload.worktree_id,
    )?;
    if !is_lower_hex_fingerprint(&payload.base_graph_fingerprint) {
        return Err(ProviderError::new(
            "provider-fingerprint",
            "enrichment payload has an invalid base graph fingerprint",
        ));
    }
    if payload.edges.is_empty() && payload.query_hints.is_empty() {
        return Err(ProviderError::new(
            "provider-enrichment",
            "enrichment payload must contain an edge or query hint",
        ));
    }
    if !payload.edges.is_empty()
        && !manifest
            .capabilities
            .contains(&ProviderCapability::SemanticEdges)
        || !payload.query_hints.is_empty()
            && !manifest
                .capabilities
                .contains(&ProviderCapability::QueryHints)
    {
        return Err(ProviderError::new(
            "provider-manifest-capability",
            "enrichment output was not declared by the manifest",
        ));
    }
    payload.edges.sort_by(edge_order);
    let mut edge_keys = BTreeSet::new();
    for edge in &payload.edges {
        if edge.from.trim().is_empty()
            || edge.to.trim().is_empty()
            || !valid_identifier(&edge.kind)
            || !edge_keys.insert((&edge.from, &edge.to, &edge.kind))
        {
            return Err(ProviderError::new(
                "provider-edge",
                "enrichment edge is empty, malformed, or duplicated",
            ));
        }
        validate_evidence(&edge.provenance)?;
    }
    payload.query_hints.sort_by(|left, right| {
        (&left.node_id, &left.kind, &left.message).cmp(&(
            &right.node_id,
            &right.kind,
            &right.message,
        ))
    });
    let mut hints = BTreeSet::new();
    for hint in &payload.query_hints {
        if hint.node_id.trim().is_empty()
            || !valid_identifier(&hint.kind)
            || hint.message.trim().is_empty()
            || !hints.insert((&hint.node_id, &hint.kind, &hint.message))
        {
            return Err(ProviderError::new(
                "provider-query-hint",
                "query hint is empty, malformed, or duplicated",
            ));
        }
        validate_evidence(&hint.provenance)?;
    }
    validate_diagnostics(manifest, &mut payload.diagnostics)?;
    let enrichment_fingerprint = canonical_fingerprint(&(PROVIDER_IR_VERSION, &payload))?;
    Ok(EnrichmentArtifact {
        schema: ENRICHMENT_ARTIFACT_SCHEMA.to_string(),
        ir_version: PROVIDER_IR_VERSION,
        enrichment_fingerprint,
        payload,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_payload_identity(
    manifest: &ProviderManifest,
    expected_worktree: &str,
    actual_schema: &str,
    expected_schema: &str,
    provider_id: &str,
    provider_version: &str,
    language: &str,
    worktree_id: &str,
) -> Result<(), ProviderError> {
    if actual_schema != expected_schema {
        return Err(ProviderError::new(
            "provider-schema",
            format!("unsupported provider payload schema `{actual_schema}`"),
        ));
    }
    if provider_id != manifest.provider_id
        || provider_version != manifest.provider_version
        || language != manifest.language
    {
        return Err(ProviderError::new(
            "provider-identity",
            "payload provider identity does not match its manifest",
        ));
    }
    if expected_worktree.trim().is_empty() || worktree_id != expected_worktree {
        return Err(ProviderError::new(
            "provider-worktree-mismatch",
            format!("expected worktree `{expected_worktree}`, received `{worktree_id}`"),
        ));
    }
    Ok(())
}

fn validate_freshness(
    freshness: &mut FreshnessReport,
    diagnostics: &[ProviderDiagnostic],
) -> Result<(), ProviderError> {
    freshness
        .inputs
        .sort_by(|left, right| left.path.cmp(&right.path));
    let mut input_paths = BTreeSet::new();
    for input in &freshness.inputs {
        validate_repository_path(&input.path)?;
        if !is_lower_hex_fingerprint(&input.fingerprint)
            || !input_paths.insert(input.path.as_str())
        {
            return Err(ProviderError::new(
                "provider-freshness",
                "freshness inputs need unique paths and 64-character fingerprints",
            ));
        }
    }
    if freshness.inputs.is_empty() {
        return Err(ProviderError::new(
            "provider-freshness",
            "freshness input facts must not be empty",
        ));
    }
    freshness.affected_paths.sort();
    freshness.affected_paths.dedup();
    for path in &freshness.affected_paths {
        validate_repository_path(path)?;
    }
    match freshness.state {
        FreshnessState::Fresh if !freshness.affected_paths.is_empty() => Err(ProviderError::new(
            "provider-freshness",
            "fresh payload cannot declare affected stale or partial paths",
        )),
        FreshnessState::Stale | FreshnessState::Partial
            if freshness.affected_paths.is_empty() || diagnostics.is_empty() =>
        {
            Err(ProviderError::new(
                "provider-freshness",
                "stale and partial payloads require affected paths and diagnostics",
            ))
        }
        _ => Ok(()),
    }
}

fn validate_diagnostics(
    manifest: &ProviderManifest,
    diagnostics: &mut Vec<ProviderDiagnostic>,
) -> Result<(), ProviderError> {
    if diagnostics.len() > manifest.limits.max_diagnostics {
        return Err(ProviderError::new(
            "provider-diagnostic-limit",
            "provider emitted more diagnostics than its manifest allows",
        ));
    }
    diagnostics.sort_by(|left, right| {
        (&left.code, &left.path, &left.message).cmp(&(&right.code, &right.path, &right.message))
    });
    for diagnostic in diagnostics {
        if !valid_identifier(&diagnostic.code) || diagnostic.message.trim().is_empty() {
            return Err(ProviderError::new(
                "provider-diagnostic",
                "provider diagnostic has an invalid code or empty message",
            ));
        }
        if let Some(path) = &diagnostic.path {
            validate_repository_path(path)?;
        }
    }
    Ok(())
}

fn validate_evidence(evidence: &ProviderEvidence) -> Result<(), ProviderError> {
    if !valid_identifier(&evidence.extractor)
        || evidence.extractor_version.trim().is_empty()
        || evidence.evidence.trim().is_empty()
    {
        return Err(ProviderError::new(
            "provider-evidence",
            "provider fact must carry extractor, version, evidence, and confidence",
        ));
    }
    Ok(())
}

fn validate_span(span: Option<&SourceSpan>) -> Result<(), ProviderError> {
    if span.is_some_and(|span| {
        span.line_start == 0
            || span.line_end < span.line_start
            || span.line_end == span.line_start && span.column_end < span.column_start
    }) {
        return Err(ProviderError::new(
            "provider-span",
            "source span is reversed or uses a zero line",
        ));
    }
    Ok(())
}

fn validate_repository_path(path: &str) -> Result<(), ProviderError> {
    if path.trim().is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(ProviderError::new(
            "provider-path",
            format!("`{path}` is not a normalized repository-relative path"),
        ));
    }
    Ok(())
}

fn edge_order(left: &ProviderEdge, right: &ProviderEdge) -> std::cmp::Ordering {
    (&left.from, &left.to, &left.kind).cmp(&(&right.from, &right.to, &right.kind))
}

fn canonical_fingerprint(value: &impl Serialize) -> Result<String, ProviderError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        ProviderError::new(
            "provider-serialization",
            format!("failed to serialize canonical provider artifact: {error}"),
        )
    })?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn is_lower_hex_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
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
#[allow(clippy::unwrap_used)]
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

    fn evidence() -> ProviderEvidence {
        ProviderEvidence {
            extractor: "fixture-parser".to_string(),
            extractor_version: "1.0.0".to_string(),
            evidence: "fixture syntax".to_string(),
            confidence: EvidenceConfidence::Exact,
        }
    }

    fn extraction_payload() -> ExtractionPayload {
        ExtractionPayload {
            schema: EXTRACTION_PAYLOAD_SCHEMA.to_string(),
            provider_id: "fixture-extractor".to_string(),
            provider_version: "1.0.0".to_string(),
            language: "fixture".to_string(),
            worktree_id: "worktree-main".to_string(),
            freshness: FreshnessReport {
                state: FreshnessState::Fresh,
                inputs: vec![FreshnessFact {
                    path: "src/lib.fixture".to_string(),
                    fingerprint:
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_string(),
                }],
                affected_paths: Vec::new(),
            },
            nodes: vec![
                ProviderNode {
                    id: "fixture-extractor:module:root".to_string(),
                    name: "root".to_string(),
                    kind: "module".to_string(),
                    path: "src/lib.fixture".to_string(),
                    span: None,
                    provenance: evidence(),
                },
                ProviderNode {
                    id: "fixture-extractor:function:root/run".to_string(),
                    name: "run".to_string(),
                    kind: "function".to_string(),
                    path: "src/lib.fixture".to_string(),
                    span: Some(SourceSpan {
                        line_start: 2,
                        column_start: 1,
                        line_end: 3,
                        column_end: 2,
                    }),
                    provenance: evidence(),
                },
            ],
            edges: vec![ProviderEdge {
                from: "fixture-extractor:module:root".to_string(),
                to: "fixture-extractor:function:root/run".to_string(),
                kind: "contains".to_string(),
                provenance: evidence(),
            }],
            diagnostics: Vec::new(),
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

    #[test]
    fn test_extraction_projection_is_stable_and_provider_scoped() {
        let manifest = extractor_manifest();
        let first = project_extraction(&manifest, "worktree-main", extraction_payload()).unwrap();

        let mut reordered = extraction_payload();
        reordered.nodes.reverse();
        let second = project_extraction(&manifest, "worktree-main", reordered).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.schema, EXTRACTION_ARTIFACT_SCHEMA);
        assert_eq!(first.ir_version, PROVIDER_IR_VERSION);
        assert!(first
            .payload
            .nodes
            .iter()
            .all(|node| node.id.starts_with("fixture-extractor:")));
        assert_eq!(first.graph_fingerprint.len(), 64);
    }

    #[test]
    fn test_extraction_rejects_unscoped_ids_and_unsafe_paths() {
        let manifest = extractor_manifest();
        let mut unscoped = extraction_payload();
        unscoped.nodes[0].id = "other:module:root".to_string();
        assert_eq!(
            project_extraction(&manifest, "worktree-main", unscoped)
                .unwrap_err()
                .code(),
            "provider-node-id"
        );

        for unsafe_path in ["/src/lib.fixture", "src\\lib.fixture", "src/../secret"] {
            let mut payload = extraction_payload();
            payload.nodes[0].path = unsafe_path.to_string();
            assert_eq!(
                project_extraction(&manifest, "worktree-main", payload)
                    .unwrap_err()
                    .code(),
                "provider-path"
            );
        }
    }

    #[test]
    fn test_projection_preserves_partial_and_stale_diagnostics() {
        let manifest = extractor_manifest();
        for state in [FreshnessState::Partial, FreshnessState::Stale] {
            let mut payload = extraction_payload();
            payload.freshness.state = state;
            payload.freshness.affected_paths = vec!["src/broken.fixture".to_string()];
            payload.diagnostics.push(ProviderDiagnostic {
                code: "fixture-parse".to_string(),
                severity: DiagnosticSeverity::Warning,
                message: "fixture parser retained partial facts".to_string(),
                path: Some("src/broken.fixture".to_string()),
            });
            let artifact = project_extraction(&manifest, "worktree-main", payload).unwrap();
            assert_eq!(artifact.payload.freshness.state, state);
            assert_eq!(
                artifact.payload.freshness.affected_paths,
                ["src/broken.fixture"]
            );
            assert_eq!(artifact.payload.diagnostics.len(), 1);
        }
    }

    #[test]
    fn test_projection_rejects_wrong_worktree() {
        let error = project_extraction(
            &extractor_manifest(),
            "worktree-other",
            extraction_payload(),
        )
        .unwrap_err();
        assert_eq!(error.code(), "provider-worktree-mismatch");
    }

    #[test]
    fn test_enricher_schema_is_additive_and_evidence_bearing() {
        let mut manifest = extractor_manifest();
        manifest.provider_id = "fixture-enricher".to_string();
        manifest.role = ProviderRole::SemanticEnricher;
        manifest.capabilities = BTreeSet::from([
            ProviderCapability::SemanticEdges,
            ProviderCapability::QueryHints,
        ]);
        let payload = EnrichmentPayload {
            schema: ENRICHMENT_PAYLOAD_SCHEMA.to_string(),
            provider_id: manifest.provider_id.clone(),
            provider_version: manifest.provider_version.clone(),
            language: manifest.language.clone(),
            worktree_id: "worktree-main".to_string(),
            base_graph_fingerprint:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            edges: vec![ProviderEdge {
                from: "fixture-extractor:function:root/run".to_string(),
                to: "external:runtime:dispatch".to_string(),
                kind: "dispatch-candidate".to_string(),
                provenance: ProviderEvidence {
                    confidence: EvidenceConfidence::Candidate,
                    ..evidence()
                },
            }],
            query_hints: vec![ProviderQueryHint {
                node_id: "fixture-extractor:function:root/run".to_string(),
                kind: "runtime-boundary".to_string(),
                message: "runtime dispatch continues beyond the static graph".to_string(),
                provenance: ProviderEvidence {
                    confidence: EvidenceConfidence::Heuristic,
                    ..evidence()
                },
            }],
            diagnostics: Vec::new(),
        };

        let artifact = project_enrichment(&manifest, "worktree-main", payload).unwrap();
        let value = serde_json::to_value(&artifact).unwrap();
        assert!(value["payload"].get("nodes").is_none());
        assert!(value["payload"].get("requirements").is_none());
        assert_eq!(artifact.enrichment_fingerprint.len(), 64);

        let mut missing_evidence = artifact.payload;
        missing_evidence.edges[0].provenance.evidence.clear();
        assert_eq!(
            project_enrichment(&manifest, "worktree-main", missing_evidence)
                .unwrap_err()
                .code(),
            "provider-evidence"
        );
    }
}
