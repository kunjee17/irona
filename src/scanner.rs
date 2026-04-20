use std::fs;
use std::path::PathBuf;

use crossbeam_channel::Sender;
use rayon::prelude::*;
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    NodeJs,
    CSharp,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "Rust"),
            Language::NodeJs => write!(f, "Node.js"),
            Language::CSharp => write!(f, "C#"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactEntry {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub language: Language,
    pub size_bytes: u64,
}

#[derive(Debug)]
pub enum ScanMessage {
    Found(ArtifactEntry),
    Done,
}

/// Checks `dir` for marker files and returns artifact subdirectories found.
/// Only returns a folder if its parent contains the expected marker — avoids
/// false positives on unrelated folders named "bin" or "target".
pub fn detect_artifacts(dir: &std::path::Path) -> Vec<(PathBuf, Language)> {
    let mut found = Vec::new();

    let names: Vec<String> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
        Err(_) => return found,
    };

    // Rust: Cargo.toml -> target/
    if names.iter().any(|n| n == "Cargo.toml") {
        let p = dir.join("target");
        if p.is_dir() {
            found.push((p, Language::Rust));
        }
    }

    // Node.js: package.json -> node_modules/
    if names.iter().any(|n| n == "package.json") {
        let p = dir.join("node_modules");
        if p.is_dir() {
            found.push((p, Language::NodeJs));
        }
    }

    // C#: *.csproj or *.sln -> bin/ and obj/
    if names
        .iter()
        .any(|n| n.ends_with(".csproj") || n.ends_with(".sln"))
    {
        for folder in &["bin", "obj"] {
            let p = dir.join(folder);
            if p.is_dir() {
                found.push((p, Language::CSharp));
            }
        }
    }

    found
}

/// Sums sizes of all files under `path` recursively.
pub fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

pub fn scan(root: PathBuf, tx: Sender<ScanMessage>) {
    // Phase 1: walkdir to collect candidate artifact paths (fast — metadata only).
    // filter_entry skips descending INTO known artifact dirs, preventing
    // redundant deep traversal of e.g. target/ which can be millions of files.
    let mut candidates: Vec<(PathBuf, Language)> = Vec::new();

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if !e.file_type().is_dir() {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            !matches!(
                name.as_ref(),
                "target" | "node_modules" | "bin" | "obj" | ".git"
            )
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir())
    {
        candidates.extend(detect_artifacts(entry.path()));
    }

    // Phase 2: rayon calculates sizes in parallel, sends each result immediately.
    candidates.par_iter().for_each(|(path, language)| {
        let size_bytes = dir_size(path);
        tx.send(ScanMessage::Found(ArtifactEntry {
            path: path.clone(),
            language: language.clone(),
            size_bytes,
        }))
        .ok();
    });

    tx.send(ScanMessage::Done).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_rust_target() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::Rust);
        assert!(results[0].0.ends_with("target"));
    }

    #[test]
    fn detects_node_modules() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        fs::create_dir(tmp.path().join("node_modules")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::NodeJs);
        assert!(results[0].0.ends_with("node_modules"));
    }

    #[test]
    fn detects_csharp_bin_and_obj() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("App.csproj"), "<Project/>").unwrap();
        fs::create_dir(tmp.path().join("bin")).unwrap();
        fs::create_dir(tmp.path().join("obj")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, l)| *l == Language::CSharp));
    }

    #[test]
    fn no_false_positive_target_without_cargo_toml() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn no_false_positive_bin_without_csproj() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("bin")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert!(results.is_empty());
    }

    #[test]
    fn calculates_dir_size() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello").unwrap(); // 5 bytes
        fs::write(tmp.path().join("b.txt"), "world!").unwrap(); // 6 bytes
        assert_eq!(dir_size(tmp.path()), 11);
    }
}
