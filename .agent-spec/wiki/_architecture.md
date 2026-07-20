---
title: "Architecture"
type: architecture
source_files:
  - Cargo.lock
  - Cargo.toml
  - build.rs
  - crates/rust-atlas/Cargo.toml
  - crates/rust-atlas/src/lib.rs
  - src/main.rs
---
# Architecture

- Inventory: [architecture/inventory.json](architecture/inventory.json)
- Workspace diagram: [architecture/workspace.mmd](architecture/workspace.mmd)
- Module diagram: [architecture/modules.mmd](architecture/modules.mmd)
- Project map data: [architecture/project-map.json](architecture/project-map.json)
- Project map diagram: [architecture/project-map.mmd](architecture/project-map.mmd)
- Provider: `rust-cargo`
- Packages: 2
- Dependencies: 17
- Modules: 78
- Module edges: 151

Reviewed for Atlas D2: committed generations change graph publication, not the
workspace package or dependency topology summarized above.

Reviewed for Atlas D3: the optional local daemon adds runtime modules but no
workspace package or external service dependency.
