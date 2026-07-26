# Headless Clean Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `irona --clean <path>` mode that scans, deletes every artifact directory it finds, prints one summary line, and never touches the terminal — so it can run from an AI tool's session-end hook.

**Architecture:** A new `src/headless.rs` drives the existing `scanner::scan` and `deleter::delete_all` engines, both already independent of the TUI. `main()` branches on `--clean` before any terminal setup, so raw mode and the alternate screen are never entered. `format_bytes` moves out of `render.rs` into a new `src/format.rs` so headless output does not depend on the ratatui rendering module.

**Tech Stack:** Rust 2021, clap 4 (derive), crossbeam-channel, tokio (current runtime, `block_on`), tempfile for tests, `cargo nextest run`.

**Spec:** `docs/superpowers/specs/2026-07-26-headless-clean-design.md`

## Global Constraints

- Rust only. No scripts, no other languages.
- No comments unless the WHY is non-obvious.
- No placeholder code or TODO stubs in committed work.
- **No mid-task commits.** The project CLAUDE.md requires one commit for the whole branch, made only when the task is complete. Tasks below therefore end in a verification step, not a commit. The single commit happens in Task 7.
- Tests run with `cargo nextest run`, not `cargo test`.
- The pre-commit hook runs `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo build`. All three must pass before Task 7.
- Existing TUI behaviour must not change. `irona` and `irona <path>` keep launching the TUI.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `src/format.rs` | Create | `format_bytes` — human-readable byte sizes, shared by TUI and headless |
| `src/headless.rs` | Create | Options, summary type, scan collection, delete orchestration, summary formatting |
| `src/render.rs` | Modify | Drop `format_bytes` and its tests; import from `crate::format` |
| `src/main.rs` | Modify | Declare the two new modules; add three CLI flags; branch to headless before terminal setup; set exit code |
| `README.md` | Modify | Document headless mode and the Claude Code hook example |

---

### Task 1: Extract `format_bytes` into its own module

Headless output needs `format_bytes`, which currently lives in `render.rs` alongside ratatui widget code. Move it first so later tasks can import it from a neutral place.

**Files:**
- Create: `src/format.rs`
- Modify: `src/render.rs` (delete lines 200-232: the `format_bytes` function and the whole `#[cfg(test)] mod tests` block, which contains only its four tests; add an import at the top)
- Modify: `src/main.rs:1-6` (module declarations)

**Interfaces:**
- Consumes: nothing
- Produces: `crate::format::format_bytes(bytes: u64) -> String`

- [ ] **Step 1: Create `src/format.rs` with the function and its tests moved verbatim**

```rust
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_gb() {
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }
    #[test]
    fn format_bytes_mb() {
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
    }
    #[test]
    fn format_bytes_kb() {
        assert_eq!(format_bytes(1_024), "1.0 KB");
    }
    #[test]
    fn format_bytes_bytes() {
        assert_eq!(format_bytes(512), "512 B");
    }
}
```

- [ ] **Step 2: Delete the old copy from `src/render.rs`**

Remove `pub fn format_bytes` (line 200) through the end of the file — the function and the `#[cfg(test)] mod tests` block below it, which holds only the four tests just moved. Then add the import to the existing `use` block at the top:

```rust
use crate::components::three_row_layout;
use crate::format::format_bytes;
use crate::model::{AppModel, AppStatus, DeleteState};
```

The four call sites inside `render.rs` (lines 53, 75, 126, 189) are unchanged — they now resolve through the import.

- [ ] **Step 3: Declare the module in `src/main.rs`**

Module declarations are alphabetical. Insert `mod format;` after `mod errors;`:

```rust
mod components;
mod deleter;
mod errors;
mod format;
mod model;
mod render;
mod scanner;
```

- [ ] **Step 4: Verify the move changed nothing**

Run: `cargo nextest run`
Expected: PASS, same test count as before the move (the four `format_bytes` tests now report under `format::tests` instead of `render::tests`).

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. If clippy reports `format_bytes` as unused from `render`, a call site was missed.

---

### Task 2: `CleanOptions`, `CleanSummary`, and `format_summary`

The pure output layer. No filesystem, no scanning — build and test it first so the summary wording is locked down before the machinery exists.

**Files:**
- Create: `src/headless.rs`
- Modify: `src/main.rs` (add `mod headless;`)

**Interfaces:**
- Consumes: `crate::format::format_bytes` (Task 1)
- Produces:
  - `pub struct CleanOptions { pub dry_run: bool, pub include_gitignored: bool }` — derives `Debug, Clone, Copy`
  - `pub struct CleanSummary { pub directories: usize, pub failed: usize, pub bytes: u64, pub elapsed: Duration }` — derives `Debug, Default, PartialEq`
  - `pub fn format_summary(root: &Path, summary: &CleanSummary, dry_run: bool) -> String`

`directories` counts directories successfully deleted, or — on a dry run — directories that would be deleted.

- [ ] **Step 1: Write the failing tests**

Create `src/headless.rs` containing only the type definitions with empty bodies is *not* the approach — write the tests first, against the signatures above:

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
```

Add `mod headless;` to `src/main.rs` after `mod format;` so the module compiles.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run headless`
Expected: FAIL to compile — `cannot find type CleanSummary`, `cannot find function format_summary`.

- [ ] **Step 3: Write the minimal implementation**

At the top of `src/headless.rs`, above the test module:

```rust
use crate::format::format_bytes;
use std::path::Path;
use std::time::Duration;

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
```

`CleanOptions` is unused until Task 3, so this step will emit a dead-code warning. That is expected and is resolved by Task 3; do not silence it with an attribute.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run headless`
Expected: PASS, 5 tests.

---

### Task 3: `collect` — drive the scanner and apply the gitignore filter

**Files:**
- Modify: `src/headless.rs`

**Interfaces:**
- Consumes: `scanner::scan(root: PathBuf, tx: Sender<ScanMessage>)`, `scanner::ArtifactEntry { path, language, size_bytes }`, `scanner::Language::GitIgnore`, `scanner::ScanMessage::{Found, Done}`, and `CleanOptions` from Task 2
- Produces: `fn collect(root: PathBuf, opts: CleanOptions) -> Vec<ArtifactEntry>` — module-private, used by `run` in Task 4 and by tests in the same module

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` in `src/headless.rs`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run headless`
Expected: FAIL to compile — `cannot find function collect`.

- [ ] **Step 3: Write the minimal implementation**

Extend the imports at the top of `src/headless.rs`:

```rust
use crate::format::format_bytes;
use crate::scanner::{self, ArtifactEntry, Language, ScanMessage};
use crossbeam_channel::unbounded;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
```

Add below `format_summary`:

```rust
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
```

`scanner::scan` sends `Done` last, so breaking on it and joining the thread cannot deadlock.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run headless`
Expected: PASS, 8 tests.

---

### Task 4: `run` — delete or dry-run, and aggregate

**Files:**
- Modify: `src/headless.rs`

**Interfaces:**
- Consumes: `collect` (Task 3), `CleanOptions` / `CleanSummary` (Task 2), `deleter::delete_all(paths: Vec<(usize, PathBuf)>) -> Vec<DeleteResult>` where `DeleteResult { index, elapsed, outcome: Result<(), IronaError> }`
- Produces: `pub fn run(root: PathBuf, opts: CleanOptions) -> CleanSummary`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/headless.rs`:

```rust
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
```

These are plain `#[test]` functions, not `#[tokio::test]`. `run` builds its own tokio runtime internally, which would panic if called from inside an existing runtime.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run headless`
Expected: FAIL to compile — `cannot find function run`.

- [ ] **Step 3: Write the minimal implementation**

Add `use crate::deleter;` and `use std::time::Instant;` to the imports, then add below `collect`:

```rust
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
```

`result.index` originates from the `enumerate` above, so indexing `entries` is always in bounds.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run headless`
Expected: PASS, 12 tests.

---

### Task 5: CLI flags and the headless branch in `main`

**Files:**
- Modify: `src/main.rs:28-37` (the `Args` struct and the top of `main`)

**Interfaces:**
- Consumes: `headless::{run, format_summary, CleanOptions}` (Tasks 2 and 4)
- Produces: the `--clean`, `--dry-run`, `--include-gitignored` CLI surface

- [ ] **Step 1: Write the failing tests**

Append to the bottom of `src/main.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn args_definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn parses_clean_with_path() {
        let args = Args::try_parse_from(["irona", "--clean", "/tmp/w"]).unwrap();
        assert!(args.clean);
        assert!(!args.dry_run);
        assert!(!args.include_gitignored);
        assert_eq!(args.path, PathBuf::from("/tmp/w"));
    }

    #[test]
    fn rejects_dry_run_without_clean() {
        assert!(Args::try_parse_from(["irona", "--dry-run"]).is_err());
    }

    #[test]
    fn rejects_include_gitignored_without_clean() {
        assert!(Args::try_parse_from(["irona", "--include-gitignored"]).is_err());
    }

    #[test]
    fn bare_path_still_means_tui() {
        let args = Args::try_parse_from(["irona", "/tmp/w"]).unwrap();
        assert!(!args.clean);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run --bin irona tests::`
Expected: FAIL to compile — `no field clean on type Args`.

- [ ] **Step 3: Add the flags**

Replace the `Args` struct at `src/main.rs:28-33`:

```rust
#[derive(Parser)]
#[command(name = "irona", about = "Reclaim disk space from build artifacts")]
struct Args {
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Delete all artifacts without launching the TUI
    #[arg(long)]
    clean: bool,

    /// With --clean: report what would be freed without deleting anything
    #[arg(long, requires = "clean")]
    dry_run: bool,

    /// With --clean: also delete directories matched only by .gitignore
    #[arg(long, requires = "clean")]
    include_gitignored: bool,
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run --bin irona tests::`
Expected: PASS, 5 tests.

- [ ] **Step 5: Branch to headless before any terminal setup**

In `src/main.rs`, insert the branch immediately after the `root` binding and *before* `std::panic::take_hook()`:

```rust
fn main() -> Result<()> {
    let args = Args::parse();
    let root = args.path.canonicalize().unwrap_or(args.path);

    if args.clean {
        let opts = headless::CleanOptions {
            dry_run: args.dry_run,
            include_gitignored: args.include_gitignored,
        };
        let summary = headless::run(root.clone(), opts);
        println!("{}", headless::format_summary(&root, &summary, opts.dry_run));
        if summary.failed > 0 {
            std::process::exit(1);
        }
        return Ok(());
    }

    let original_hook = std::panic::take_hook();
```

Everything from `let original_hook` down is unchanged. Placing the branch here guarantees `enable_raw_mode`, `EnterAlternateScreen`, and the TUI panic hook are never reached in a hook context.

- [ ] **Step 6: Verify the whole suite and both modes by hand**

Run: `cargo nextest run`
Expected: PASS, all tests.

Run: `cargo run -- --clean --dry-run /tmp/irona-smoke` after `mkdir -p /tmp/irona-smoke/proj/target && touch /tmp/irona-smoke/proj/Cargo.toml && dd if=/dev/zero of=/tmp/irona-smoke/proj/target/blob bs=1024 count=2048`
Expected: one stdout line, `irona: would free 2.0 MB from 1 directory (dry run)`, and `/tmp/irona-smoke/proj/target` still present. No screen clearing, no alternate screen.

Run: `cargo run -- --clean /tmp/irona-smoke && echo "exit=$?"`
Expected: `irona: freed 2.0 MB from 1 directory in 0.0s`, then `exit=0`, and the `target` directory gone.

Run: `cargo run -- --clean /tmp/irona-smoke && echo "exit=$?"` a second time
Expected: `irona: nothing to clean in /tmp/irona-smoke`, then `exit=0`.

Run: `cargo run -- --dry-run`
Expected: clap error — `the following required arguments were not provided: --clean`.

Clean up: `rm -rf /tmp/irona-smoke`

---

### Task 6: Document headless mode

**Files:**
- Modify: `README.md` (insert a new section between `## Usage` ending at line 46 and `## Supported Languages` at line 48)

**Interfaces:**
- Consumes: the CLI surface from Task 5
- Produces: nothing code-facing

- [ ] **Step 1: Add the section**

Insert after the Usage paragraph and before `## Supported Languages`:

```markdown
## Headless mode

`--clean` skips the TUI entirely: it scans, deletes everything it finds, and prints one summary line. Built for AI tool session-end hooks and any other non-interactive context.

```
irona --clean ~/Workspace                  # scan, delete, report
irona --clean --dry-run ~/Workspace        # report only, delete nothing
irona --clean --include-gitignored ~/W     # also sweep gitignore-only matches
```

```
irona: freed 2.3 GB from 4 directories in 1.2s
irona: would free 2.3 GB from 4 directories (dry run)
irona: nothing to clean in /home/you/Workspace
```

By default headless mode deletes only directories backed by a marker file — `target/` next to a `Cargo.toml`, `node_modules/` next to a `package.json`, and the rest of the table below. Directories found only through `.gitignore` are left alone, because unattended they could include a gitignored `data/`, `logs/`, or `secrets/`. Pass `--include-gitignored` to sweep those too. The TUI is unaffected and still shows both kinds.

Exit code is `0` on success, including when nothing was found, so a clean workspace never fails your hook. It is `1` only if a deletion failed; the failing paths are reported on stderr.

### Claude Code hook

Sweep build artifacts every time a session ends, via `settings.json`:

```json
{
  "hooks": {
    "Stop": [{ "command": "irona --clean ~/Workspace" }]
  }
}
```

Run it once with `--dry-run` first to see what it would take.
```

- [ ] **Step 2: Verify the rendered output**

Read the README section back and confirm the nested code fences render correctly and the flag spellings match `Args` exactly (`--clean`, `--dry-run`, `--include-gitignored`).

---

### Task 7: Verify, commit, and open the PR

The project uses one commit per branch, made only when the work is complete. This is that commit.

**Files:** none modified — verification and git only.

- [ ] **Step 1: Run the full gate**

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo nextest run
```

Expected: fmt clean, no clippy warnings, all tests pass. Do not proceed on any failure — fix it and rerun.

- [ ] **Step 2: Review the diff**

```bash
git status --short
git diff
```

Expected files: `src/format.rs` (new), `src/headless.rs` (new), `src/main.rs`, `src/render.rs`, `README.md`, and the two docs under `docs/superpowers/`. Nothing else. Confirm no TODO stubs and no commented-out code.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "$(cat <<'EOF'
feat: add headless --clean mode for non-interactive cleanup

Adds `irona --clean <path>`, which scans, deletes every artifact
directory it finds, and prints one summary line without touching the
terminal — usable from an AI tool's session-end hook.

`--dry-run` reports without deleting. Gitignore-only matches are skipped
by default since an unattended sweep could take out a gitignored data/
or secrets/ directory; `--include-gitignored` opts back in.

Exit code is 0 on success including nothing-found, 1 if a delete failed.

Moves format_bytes out of render.rs into src/format.rs so headless
output does not depend on the ratatui rendering module.

Closes #15

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 4: Push and open the PR**

```bash
git push -u origin feat/headless-clean
gh pr create --title "feat: headless --clean mode for AI tool post-session hooks" --body "$(cat <<'EOF'
Closes #15.

## What

`irona --clean <path>` scans, deletes every artifact directory it finds, prints one summary line, and never enters raw mode or the alternate screen — so it works from a Claude Code `Stop` hook or any non-interactive context.

```
irona --clean ~/Workspace                  # scan, delete, report
irona --clean --dry-run ~/Workspace        # report only, delete nothing
irona --clean --include-gitignored ~/W     # also sweep gitignore-only matches
```

## Decisions

Answering the issue's open questions:

- **Flag, not subcommand.** Keeps the existing `irona <path>` positional working with no clap restructuring.
- **`--dry-run` included.** The safety valve before wiring a destructive command into a hook.
- **Exit `0` on nothing-found**, `1` only when a delete failed. A clean workspace must not fail a hook noisily.
- **`--only rust,node` deferred.** Needs a name parser for all fifteen `Language` variants and nothing depends on it yet.

One decision not in the issue: gitignore-detected directories are **excluded by default** in headless mode. In the TUI a person reads the list before selecting; unattended, a gitignored `data/`, `logs/`, or `secrets/` would be deleted with nobody watching. `--include-gitignored` restores the full sweep. TUI behaviour is unchanged.

## Structure

New `src/headless.rs` drives the existing `scanner::scan` and `deleter::delete_all`, which were already TUI-independent. `format_bytes` moves from `render.rs` to a new `src/format.rs` so headless output does not depend on the rendering module.

## Note

PR #17 also edits the `Args` struct in `main.rs` to add `--mcp`. Whichever merges second needs a small conflict resolution there; nothing structural overlaps.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

- [ ] **Step 5: Confirm the PR exists**

Run: `gh pr view --json number,title,url`
Expected: the new PR number and URL.

---

## Self-Review

**Spec coverage:**

| Spec section | Task |
| --- | --- |
| CLI surface, three flags, `requires = "clean"` | Task 5 |
| `--only` deferred | Recorded in Task 7 PR body; no task, by design |
| Control flow branches before terminal setup | Task 5, Step 5 |
| `CleanOptions` / `CleanSummary` | Task 2 |
| `collect` | Task 3 |
| `run` | Task 4 |
| `format_summary` | Task 2 |
| `format_bytes` moves to `src/format.rs` | Task 1 |
| Gitignore excluded by default, `--include-gitignored` opts in | Tasks 3 and 5 |
| One stdout line, failures on stderr | Task 2 (stdout wording), Task 4 (stderr per-path) |
| Byte counts sum successful deletions; dry run sums all in scope | Task 4 |
| Exit `0` including nothing-found, `1` on delete failure | Task 5 |
| Missing path reports nothing-to-clean, exits `0` | Task 4, `run_handles_missing_path` |
| All five test cases from the spec | Tasks 2, 3, 4 |
| README section plus hook example | Task 6 |
| PR #17 interaction noted | Task 7 PR body |

No gaps.

**Type consistency:** `CleanOptions { dry_run, include_gitignored }` and `CleanSummary { directories, failed, bytes, elapsed }` are used with the same field names in Tasks 2, 3, 4, and 5. `format_summary(root, summary, dry_run)` keeps its three-argument shape at both its definition (Task 2) and its call site (Task 5). `collect` and `run` both take `(PathBuf, CleanOptions)`. `DeleteResult.index` and `.outcome` match `src/deleter.rs:5-10`.

**Note on the spec:** the spec's summary table said `deleted`; it has been updated to `directories` to match this plan, since the field also counts would-be deletions on a dry run.
