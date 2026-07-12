#![allow(unused_imports)]

pub mod architecture;
pub mod live;
pub mod model;
pub mod project_map;
pub mod sources;

pub use architecture::{
    build_architecture_inventory, render_architecture_mermaid, render_architecture_module_mermaid,
};
pub use live::{
    check_live_wiki, check_live_wiki_with_changed_paths, collect_wiki_articles, init_live_wiki,
    inspect_live_wiki_path, lint_live_wiki, query_live_wiki, render_wiki_index, seed_live_wiki,
    seed_live_wiki_check, status_from_changed_paths, update_wiki_meta, wiki_status,
    write_project_map_artifacts, write_wiki_index,
};
pub use model::{
    ArchitectureDependency, ArchitectureEntrypoint, ArchitectureInventory, ArchitectureModule,
    ArchitectureModuleEdge, ArchitecturePackage, ArchitectureTarget, WikiArticle, WikiCheckReport,
    WikiDiagnostic, WikiInitReport, WikiInspectReport, WikiMeta, WikiQueryMatch, WikiQueryReport,
    WikiRequirementLink, WikiSeedReport, WikiSource, WikiSourceKind, WikiSourceOptions,
    WikiSourceSet, WikiSpecLink, WikiStaleArticle, WikiStatusReport, WikiTraceLink,
};
pub use project_map::{
    WikiExternalProject, WikiProjectEdge, WikiProjectFlow, WikiProjectInspectReport,
    WikiProjectMap, build_project_map, inspect_wiki_project, render_project_map_mermaid,
};
pub use sources::{discover_wiki_sources, fingerprint_bytes, fingerprint_file, path_to_slash};
