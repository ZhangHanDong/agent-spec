use std::path::{Path, PathBuf};

use crate::{AtlasError, io_err, workspace_excludes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeEntryKind {
    RustSource,
    CargoInput,
    Ignored,
}

#[derive(Debug, Clone)]
pub struct AtlasScope {
    code_root: PathBuf,
    graph_root: PathBuf,
    workspace_excludes: Vec<PathBuf>,
}

impl AtlasScope {
    pub fn discover(code_root: &Path, graph_root: &Path) -> Result<Self, AtlasError> {
        let code_root = std::fs::canonicalize(code_root).map_err(io_err)?;
        let graph_root = canonical_or_absolute(graph_root)?;
        let workspace_excludes = workspace_excludes(&code_root)
            .into_iter()
            .map(|path| canonical_or_absolute(&path))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            code_root,
            graph_root,
            workspace_excludes,
        })
    }

    pub fn classify(&self, path: &Path) -> Result<ScopeEntryKind, AtlasError> {
        let path = match std::fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ScopeEntryKind::Ignored);
            }
            Err(error) => return Err(io_err(error)),
        };
        if self.source_files().binary_search(&path).is_ok() {
            return Ok(ScopeEntryKind::RustSource);
        }
        if self.cargo_input_files().binary_search(&path).is_ok() {
            return Ok(ScopeEntryKind::CargoInput);
        }
        Ok(ScopeEntryKind::Ignored)
    }

    pub fn source_files(&self) -> Vec<PathBuf> {
        self.walk(false)
            .into_iter()
            .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("rs"))
            .collect()
    }

    pub fn cargo_input_files(&self) -> Vec<PathBuf> {
        self.walk(true)
            .into_iter()
            .filter(|path| is_cargo_input(path))
            .collect()
    }

    pub fn cargo_manifest_files(&self) -> Vec<PathBuf> {
        self.cargo_input_files()
            .into_iter()
            .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml"))
            .collect()
    }

    pub fn watch_directories(&self) -> Vec<PathBuf> {
        let mut directories = ignore::WalkBuilder::new(&self.code_root)
            .hidden(false)
            .git_ignore(true)
            .require_git(false)
            .filter_entry({
                let scope = self.clone();
                move |entry| !scope.excludes_entry(entry.path())
            })
            .build()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_dir()))
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        directories.sort();
        directories.dedup();
        directories
    }

    fn walk(&self, include_hidden: bool) -> Vec<PathBuf> {
        let mut files = ignore::WalkBuilder::new(&self.code_root)
            .hidden(!include_hidden)
            .git_ignore(true)
            .require_git(false)
            .filter_entry({
                let scope = self.clone();
                move |entry| !scope.excludes_entry(entry.path())
            })
            .build()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
            .map(|entry| entry.into_path())
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    fn excludes_entry(&self, path: &Path) -> bool {
        if path == self.code_root {
            return false;
        }
        path.starts_with(&self.graph_root)
            || self
                .workspace_excludes
                .iter()
                .any(|excluded| path.starts_with(excluded))
            || path
                .components()
                .any(|component| matches!(component.as_os_str().to_str(), Some("target" | ".git")))
    }
}

fn is_cargo_input(path: &Path) -> bool {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("Cargo.toml" | "Cargo.lock" | "rust-toolchain" | "rust-toolchain.toml") => true,
        Some("config" | "config.toml") => {
            path.parent()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                == Some(".cargo")
        }
        _ => false,
    }
}

fn canonical_or_absolute(path: &Path) -> Result<PathBuf, AtlasError> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if path.is_absolute() {
                Ok(path.to_path_buf())
            } else {
                std::env::current_dir()
                    .map(|current| current.join(path))
                    .map_err(io_err)
            }
        }
        Err(error) => Err(io_err(error)),
    }
}
