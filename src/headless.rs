use crate::deleter;
use crate::format::format_bytes;
use crate::scanner::{self, ArtifactEntry, Language, ScanMessage};
use crossbeam_channel::unbounded;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct CleanOptions {
    pub dry_run: bool,
    pub include_gitignored: bool,
}

#[derive(Debug, Default, PartialEq)]
pub struct CleanSummary {
    pub directories: usize,
    pub failed: usize,
    pub bytes: u64,
    pub elapsed: Duration,
}

pub fn format_summary(root: &Path, summary: &CleanSummary, dry_run: bool) -> String {
    if summary.directories == 0 && summary.failed == 0 {
        return format!("irona: nothing to clean in {}", root.display());
    }

    if dry_run {
        return format!(
            "irona: would free {} from {} {} (dry run)",
            format_bytes(summary.bytes),
            summary.directories,
            plural_dirs(summary.directories)
        );
    }

    let mut line = format!(
        "irona: freed {} from {} {} in {:.1}s",
        format_bytes(summary.bytes),
        summary.directories,
        plural_dirs(summary.directories),
        summary.elapsed.as_secs_f64()
    );
    if summary.failed > 0 {
        line.push_str(&format!(" ({} failed)", summary.failed));
    }
    line
}

fn plural_dirs(n: usize) -> &'static str {
    if n == 1 {
        "directory"
    } else {
        "directories"
    }
}

fn collect(root: PathBuf, opts: CleanOptions) -> Vec<ArtifactEntry> {
    let (tx, rx) = unbounded::<ScanMessage>();
    let handle = thread::spawn(move || scanner::scan(root, tx));

    let mut entries = Vec::new();
    for msg in rx {
        match msg {
            ScanMessage::Found(entry) => {
                if opts.include_gitignored || entry.language != Language::GitIgnore {
                    entries.push(entry);
                }
            }
            ScanMessage::Done => break,
        }
    }
    let _ = handle.join();

    entries
}

pub fn run(root: PathBuf, opts: CleanOptions) -> CleanSummary {
    let start = Instant::now();
    let entries = collect(root, opts);

    if opts.dry_run {
        return CleanSummary {
            directories: entries.len(),
            failed: 0,
            bytes: entries.iter().map(|e| e.size_bytes).sum(),
            elapsed: start.elapsed(),
        };
    }

    if entries.is_empty() {
        return CleanSummary {
            elapsed: start.elapsed(),
            ..Default::default()
        };
    }

    let paths: Vec<(usize, PathBuf)> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (index, entry.path.clone()))
        .collect();

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("irona: could not start async runtime: {e}");
            return CleanSummary {
                directories: 0,
                failed: entries.len(),
                bytes: 0,
                elapsed: start.elapsed(),
            };
        }
    };

    let mut summary = CleanSummary::default();
    for result in rt.block_on(deleter::delete_all(paths)) {
        let entry = &entries[result.index];
        match result.outcome {
            Ok(()) => {
                summary.directories += 1;
                summary.bytes += entry.size_bytes;
            }
            Err(e) => {
                summary.failed += 1;
                eprintln!("irona: {}: {}", entry.path.display(), e);
            }
        }
    }
    summary.elapsed = start.elapsed();

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn rust_project(dir: &Path) {
        fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
        fs::create_dir(dir.join("target")).unwrap();
        fs::write(dir.join("target").join("blob"), vec![0u8; 2048]).unwrap();
    }

    fn opts(include_gitignored: bool) -> CleanOptions {
        CleanOptions {
            dry_run: false,
            include_gitignored,
        }
    }

    #[test]
    fn collect_finds_marker_artifacts() {
        let tmp = TempDir::new().unwrap();
        rust_project(tmp.path());

        let entries = collect(tmp.path().to_path_buf(), opts(false));

        assert_eq!(entries.len(), 1);
        assert!(entries[0].path.ends_with("target"));
        assert_eq!(entries[0].size_bytes, 2048);
    }

    #[test]
    fn collect_skips_gitignored_by_default() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".gitignore"), "dist/\n").unwrap();
        fs::create_dir(tmp.path().join("dist")).unwrap();

        let entries = collect(tmp.path().to_path_buf(), opts(false));

        assert!(entries.is_empty());
    }

    #[test]
    fn collect_includes_gitignored_when_opted_in() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".gitignore"), "dist/\n").unwrap();
        fs::create_dir(tmp.path().join("dist")).unwrap();

        let entries = collect(tmp.path().to_path_buf(), opts(true));

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].language, Language::GitIgnore);
        assert!(entries[0].path.ends_with("dist"));
    }

    #[test]
    fn run_deletes_artifacts_and_reports_bytes() {
        let tmp = TempDir::new().unwrap();
        rust_project(tmp.path());
        let target = tmp.path().join("target");

        let summary = run(tmp.path().to_path_buf(), opts(false));

        assert_eq!(summary.directories, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.bytes, 2048);
        assert!(!target.exists());
    }

    #[test]
    fn run_dry_run_reports_without_deleting() {
        let tmp = TempDir::new().unwrap();
        rust_project(tmp.path());
        let target = tmp.path().join("target");

        let summary = run(
            tmp.path().to_path_buf(),
            CleanOptions {
                dry_run: true,
                include_gitignored: false,
            },
        );

        assert_eq!(summary.directories, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.bytes, 2048);
        assert!(target.exists());
    }

    #[test]
    fn run_reports_nothing_for_empty_tree() {
        let tmp = TempDir::new().unwrap();

        let summary = run(tmp.path().to_path_buf(), opts(false));

        assert_eq!(summary.directories, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.bytes, 0);
    }

    #[test]
    fn run_handles_missing_path() {
        let summary = run(PathBuf::from("/nonexistent/xyz/abc"), opts(false));

        assert_eq!(summary.directories, 0);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn summary_reports_freed_space() {
        let s = CleanSummary {
            directories: 4,
            failed: 0,
            bytes: 2_469_606_195,
            elapsed: Duration::from_millis(1200),
        };
        assert_eq!(
            format_summary(Path::new("/w"), &s, false),
            "irona: freed 2.3 GB from 4 directories in 1.2s"
        );
    }

    #[test]
    fn summary_reports_dry_run() {
        let s = CleanSummary {
            directories: 4,
            failed: 0,
            bytes: 2_469_606_195,
            elapsed: Duration::from_millis(1200),
        };
        assert_eq!(
            format_summary(Path::new("/w"), &s, true),
            "irona: would free 2.3 GB from 4 directories (dry run)"
        );
    }

    #[test]
    fn summary_reports_nothing_found() {
        let s = CleanSummary::default();
        assert_eq!(
            format_summary(Path::new("/home/kunjee/Workspace"), &s, false),
            "irona: nothing to clean in /home/kunjee/Workspace"
        );
    }

    #[test]
    fn summary_reports_failures() {
        let s = CleanSummary {
            directories: 3,
            failed: 1,
            bytes: 1_181_116_006,
            elapsed: Duration::from_millis(800),
        };
        assert_eq!(
            format_summary(Path::new("/w"), &s, false),
            "irona: freed 1.1 GB from 3 directories in 0.8s (1 failed)"
        );
    }

    #[test]
    fn summary_uses_singular_for_one_directory() {
        let s = CleanSummary {
            directories: 1,
            failed: 0,
            bytes: 1_024,
            elapsed: Duration::from_millis(100),
        };
        assert_eq!(
            format_summary(Path::new("/w"), &s, false),
            "irona: freed 1.0 KB from 1 directory in 0.1s"
        );
    }
}
