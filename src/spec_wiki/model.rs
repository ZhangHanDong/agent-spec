use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSourceSet {
    pub sources: Vec<WikiSource>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSource {
    pub kind: WikiSourceKind,
    pub path: PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum WikiSourceKind {
    Code,
    Cargo,
    Documentation,
    Knowledge,
    Spec,
    Trace,
    Archive,
    Asset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiDiagnostic {
    pub code: String,
    pub severity: String,
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiSourceOptions {
    pub include_archives: bool,
}

impl Default for WikiSourceOptions {
    fn default() -> Self {
        Self {
            include_archives: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiCheckReport {
    pub out_dir: PathBuf,
    pub diagnostics: Vec<WikiDiagnostic>,
}

impl WikiCheckReport {
    pub fn passed(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == "error")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiMeta {
    pub project: String,
    pub repo_path: PathBuf,
    pub generator: String,
    pub generator_version: String,
    pub last_compiled_commit: Option<String>,
    pub last_compiled_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiInitReport {
    pub wiki_dir: PathBuf,
    pub files_written: Vec<PathBuf>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSeedReport {
    pub wiki_dir: PathBuf,
    pub files_written: Vec<PathBuf>,
    pub missing_pages: Vec<PathBuf>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiQueryReport {
    pub wiki_dir: PathBuf,
    pub query: String,
    pub matches: Vec<WikiQueryMatch>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiQueryMatch {
    pub path: PathBuf,
    pub title: String,
    pub article_type: String,
    pub source_files: Vec<PathBuf>,
    pub tags: Vec<String>,
    pub score: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiInspectReport {
    pub input_path: PathBuf,
    pub wiki_articles: Vec<WikiQueryMatch>,
    pub requirements: Vec<WikiRequirementLink>,
    pub specs: Vec<WikiSpecLink>,
    pub trace_records: Vec<WikiTraceLink>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiRequirementLink {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiSpecLink {
    pub name: String,
    pub path: PathBuf,
    pub satisfies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiTraceLink {
    pub run_id: String,
    pub requirement_id: String,
    pub work_unit_id: String,
    pub spec_path: PathBuf,
    pub scenario_name: String,
    pub test_selector: Option<String>,
    pub verdict: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiArticle {
    pub path: PathBuf,
    pub title: String,
    pub article_type: String,
    pub source_files: Vec<PathBuf>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiStatusReport {
    pub wiki_dir: PathBuf,
    pub first_run: bool,
    pub changed_files: Vec<PathBuf>,
    pub stale_articles: Vec<WikiStaleArticle>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WikiStaleArticle {
    pub path: PathBuf,
    pub title: String,
    pub source_files: Vec<PathBuf>,
    pub changed_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureInventory {
    pub provider: String,
    pub root: PathBuf,
    pub packages: Vec<ArchitecturePackage>,
    pub targets: Vec<ArchitectureTarget>,
    pub dependencies: Vec<ArchitectureDependency>,
    pub modules: Vec<ArchitectureModule>,
    pub module_edges: Vec<ArchitectureModuleEdge>,
    pub entrypoints: Vec<ArchitectureEntrypoint>,
    pub source_files: Vec<PathBuf>,
    pub diagnostics: Vec<WikiDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitecturePackage {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub path: PathBuf,
    pub language: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureTarget {
    pub package: String,
    pub name: String,
    pub kind: String,
    pub src_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureDependency {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureModule {
    pub name: String,
    pub path: PathBuf,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ArchitectureModuleEdge {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchitectureEntrypoint {
    pub name: String,
    pub path: PathBuf,
    pub kind: String,
}
