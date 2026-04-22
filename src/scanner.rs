use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    NodeJs,
    CSharp,
    DotNet,
    Python,
    Maven,
    Gradle,
    Go,
    PHP,
    Ruby,
    Swift,
    Haskell,
    Elm,
    Dart,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "Rust"),
            Language::NodeJs => write!(f, "Node.js"),
            Language::CSharp => write!(f, "C#"),
            Language::DotNet => write!(f, ".NET"),
            Language::Python => write!(f, "Python"),
            Language::Maven => write!(f, "Maven"),
            Language::Gradle => write!(f, "Gradle"),
            Language::Go => write!(f, "Go"),
            Language::PHP => write!(f, "PHP"),
            Language::Ruby => write!(f, "Ruby"),
            Language::Swift => write!(f, "Swift"),
            Language::Haskell => write!(f, "Haskell"),
            Language::Elm => write!(f, "Elm"),
            Language::Dart => write!(f, "Dart"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct ArtifactEntry {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub language: Language,
    pub size_bytes: u64,
}

#[derive(Debug)]
#[allow(dead_code)]
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

    // .NET NuGet (packages.config style): packages/
    if names.iter().any(|n| n == "packages.config") {
        let p = dir.join("packages");
        if p.is_dir() {
            found.push((p, Language::DotNet));
        }
    }

    // .NET Paket: paket.dependencies -> packages/, .paket/
    if names.iter().any(|n| n == "paket.dependencies") {
        for folder in &["packages", ".paket"] {
            let p = dir.join(folder);
            if p.is_dir() {
                found.push((p, Language::DotNet));
            }
        }
    }

    // Python: requirements.txt / pyproject.toml / setup.py -> .venv/, venv/
    if names
        .iter()
        .any(|n| n == "requirements.txt" || n == "pyproject.toml" || n == "setup.py")
    {
        for folder in &[".venv", "venv"] {
            let p = dir.join(folder);
            if p.is_dir() {
                found.push((p, Language::Python));
            }
        }
    }

    // Java/Maven: pom.xml -> target/
    // Checked after Rust so both can match independently in mixed projects.
    if names.iter().any(|n| n == "pom.xml") {
        let p = dir.join("target");
        if p.is_dir() {
            found.push((p, Language::Maven));
        }
    }

    // Gradle (Java/Kotlin/Android): build.gradle* / settings.gradle* -> build/, .gradle/
    if names.iter().any(|n| {
        n == "build.gradle"
            || n == "build.gradle.kts"
            || n == "settings.gradle"
            || n == "settings.gradle.kts"
    }) {
        for folder in &["build", ".gradle"] {
            let p = dir.join(folder);
            if p.is_dir() {
                found.push((p, Language::Gradle));
            }
        }
    }

    // Go: go.mod -> vendor/
    if names.iter().any(|n| n == "go.mod") {
        let p = dir.join("vendor");
        if p.is_dir() {
            found.push((p, Language::Go));
        }
    }

    // PHP/Composer: composer.json -> vendor/
    if names.iter().any(|n| n == "composer.json") {
        let p = dir.join("vendor");
        if p.is_dir() {
            found.push((p, Language::PHP));
        }
    }

    // Ruby/Bundler: Gemfile -> vendor/, .bundle/
    if names.iter().any(|n| n == "Gemfile") {
        for folder in &["vendor", ".bundle"] {
            let p = dir.join(folder);
            if p.is_dir() {
                found.push((p, Language::Ruby));
            }
        }
    }

    // Swift/SPM: Package.swift -> .build/
    if names.iter().any(|n| n == "Package.swift") {
        let p = dir.join(".build");
        if p.is_dir() {
            found.push((p, Language::Swift));
        }
    }

    // Haskell/Stack: stack.yaml -> .stack-work/
    if names.iter().any(|n| n == "stack.yaml") {
        let p = dir.join(".stack-work");
        if p.is_dir() {
            found.push((p, Language::Haskell));
        }
    }

    // Elm: elm.json -> elm-stuff/
    if names.iter().any(|n| n == "elm.json") {
        let p = dir.join("elm-stuff");
        if p.is_dir() {
            found.push((p, Language::Elm));
        }
    }

    // Dart/Flutter: pubspec.yaml -> .dart_tool/, build/
    if names.iter().any(|n| n == "pubspec.yaml") {
        for folder in &[".dart_tool", "build"] {
            let p = dir.join(folder);
            if p.is_dir() {
                found.push((p, Language::Dart));
            }
        }
    }

    found
}

use crossbeam_channel::Sender;
use rayon::prelude::*;
use walkdir::WalkDir;

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

#[allow(dead_code)]
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
                "target"
                    | "node_modules"
                    | "bin"
                    | "obj"
                    | ".git"
                    | ".venv"
                    | "venv"
                    | "vendor"
                    | ".bundle"
                    | "build"
                    | ".gradle"
                    | ".build"
                    | ".stack-work"
                    | "elm-stuff"
                    | ".dart_tool"
                    | "packages"
                    | ".paket"
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
    fn detects_dotnet_nuget_packages() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("packages.config"), "<packages/>").unwrap();
        fs::create_dir(tmp.path().join("packages")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::DotNet);
        assert!(results[0].0.ends_with("packages"));
    }

    #[test]
    fn detects_dotnet_paket() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("paket.dependencies"), "source https://nuget.org/api/v2").unwrap();
        fs::create_dir(tmp.path().join("packages")).unwrap();
        fs::create_dir(tmp.path().join(".paket")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, l)| *l == Language::DotNet));
    }

    #[test]
    fn detects_python_venv() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("requirements.txt"), "requests").unwrap();
        fs::create_dir(tmp.path().join(".venv")).unwrap();
        fs::create_dir(tmp.path().join("venv")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, l)| *l == Language::Python));
    }

    #[test]
    fn detects_maven_target() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("pom.xml"), "<project/>").unwrap();
        fs::create_dir(tmp.path().join("target")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::Maven);
    }

    #[test]
    fn detects_gradle_build_and_cache() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("build.gradle"), "plugins {}").unwrap();
        fs::create_dir(tmp.path().join("build")).unwrap();
        fs::create_dir(tmp.path().join(".gradle")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, l)| *l == Language::Gradle));
    }

    #[test]
    fn detects_go_vendor() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("go.mod"), "module example.com/m\ngo 1.21").unwrap();
        fs::create_dir(tmp.path().join("vendor")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::Go);
        assert!(results[0].0.ends_with("vendor"));
    }

    #[test]
    fn detects_php_vendor() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("composer.json"), "{}").unwrap();
        fs::create_dir(tmp.path().join("vendor")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::PHP);
    }

    #[test]
    fn detects_ruby_vendor_and_bundle() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Gemfile"), "source 'https://rubygems.org'").unwrap();
        fs::create_dir(tmp.path().join("vendor")).unwrap();
        fs::create_dir(tmp.path().join(".bundle")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, l)| *l == Language::Ruby));
    }

    #[test]
    fn detects_swift_build() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("Package.swift"), "// swift-tools-version:5.5").unwrap();
        fs::create_dir(tmp.path().join(".build")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::Swift);
    }

    #[test]
    fn detects_haskell_stack_work() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("stack.yaml"), "resolver: lts-21.0").unwrap();
        fs::create_dir(tmp.path().join(".stack-work")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::Haskell);
    }

    #[test]
    fn detects_elm_stuff() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("elm.json"), r#"{"type":"application"}"#).unwrap();
        fs::create_dir(tmp.path().join("elm-stuff")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, Language::Elm);
    }

    #[test]
    fn detects_dart_flutter() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("pubspec.yaml"), "name: myapp").unwrap();
        fs::create_dir(tmp.path().join(".dart_tool")).unwrap();
        fs::create_dir(tmp.path().join("build")).unwrap();
        let results = detect_artifacts(tmp.path());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, l)| *l == Language::Dart));
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
    fn no_false_positive_vendor_without_marker() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("vendor")).unwrap();
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
