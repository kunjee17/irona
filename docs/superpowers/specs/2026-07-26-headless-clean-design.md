# Headless clean mode — design

Issue: [#15](https://github.com/kunjee17/irona/issues/15)
Date: 2026-07-26

## Problem

AI coding tools run shell commands at lifecycle events — Claude Code fires a `Stop`
hook when a session ends. irona is a good fit for post-session cleanup, but its
only entry point drives an interactive TUI, which cannot run where no terminal is
attached.

## Goal

A non-interactive mode that scans a path, deletes every artifact directory it
finds, and prints one summary line. No prompts, no alternate screen, no raw mode.

## CLI surface

Three flags are added to the existing `Args`. The positional path argument and the
default TUI behaviour are unchanged.

```
irona                                   # TUI, cwd
irona ~/Workspace                       # TUI, given path
irona --clean ~/Workspace               # scan, delete all, print summary
irona --clean --dry-run ~/Workspace     # scan, print, delete nothing
irona --clean --include-gitignored ~/W  # also sweep gitignore-only matches
```

`--dry-run` and `--include-gitignored` are declared `requires = "clean"`, so clap
rejects them when `--clean` is absent.

A `--only rust,node` language filter was considered and deferred. It needs a name
parser for all fifteen `Language` variants plus tests, nothing depends on it yet,
and it can be added later without breaking this CLI.

## Control flow

`main()` branches on `args.clean` before any terminal setup. In headless mode
`enable_raw_mode`, `EnterAlternateScreen`, and the TUI panic hook are never
reached. The TUI path is untouched.

## Module layout

New module `src/headless.rs`:

| Item | Role |
| --- | --- |
| `CleanOptions { dry_run, include_gitignored }` | Input |
| `CleanSummary { directories, failed, bytes, elapsed }` | Result. `directories` counts what was deleted, or what would be deleted on a dry run |
| `collect(root, opts) -> Vec<ArtifactEntry>` | Drives `scanner::scan` on a thread, drains the channel to completion, applies the gitignore filter |
| `run(root, opts) -> CleanSummary` | Collects, then `rt.block_on(deleter::delete_all(...))` unless dry-run; aggregates results |
| `format_summary(&CleanSummary, dry_run) -> String` | Pure function, unit-testable |

`scanner.rs`, `deleter.rs`, `model/`, and `components/` are unchanged. Both engines
are already independent of the TUI and are reused as they stand.

`format_bytes` moves from `render.rs` to a new `src/format.rs`, with its four
tests, and `render.rs` imports it from there. Headless output needs the helper and
should not depend on the ratatui rendering module to get it.

## Gitignore safety

`scanner::scan` reports two kinds of hit. Marker-based hits are directories backed
by a project file — `target/` next to a `Cargo.toml`, `node_modules/` next to a
`package.json`. Gitignore hits carry `Language::GitIgnore` and are any ignored
directory outside a four-name denylist (`.git`, `.vscode`, `.idea`, `.github`).

In the TUI a person reads the list before selecting. Unattended, a gitignored
`data/`, `logs/`, or `secrets/` would be deleted with no one watching. Headless
mode therefore drops `Language::GitIgnore` entries by default;
`--include-gitignored` restores the full sweep for users who want it. TUI
behaviour does not change.

## Output

One line on stdout, so a hook log stays one line per run:

```
irona: freed 2.3 GB from 4 directories in 1.2s
irona: would free 2.3 GB from 4 directories (dry run)
irona: nothing to clean in /home/kunjee/Workspace
irona: freed 1.1 GB from 3 directories in 0.8s (1 failed)
```

Per-directory failures print to stderr, one line each, and do not disturb the
stdout summary.

Byte counts sum `size_bytes` over the entries that were deleted successfully. A
dry run sums every in-scope entry.

## Exit codes

`0` on success, including when nothing was found — a clean workspace must not make
a hook fail noisily. `1` when at least one deletion failed. Scan errors are
already swallowed inside the scanner and do not reach this layer.

A path that does not exist yields no entries and so reports "nothing to clean" and
exits `0`, matching the scanner's existing behaviour rather than adding a separate
validation error.

## Testing

Written test-first, run with `cargo nextest run`.

- `format_summary` for all four output shapes: deleted, dry run, nothing found,
  partial failure.
- `run` against a `TempDir` holding `Cargo.toml` and `target/`: the directory is
  gone afterwards and the reported byte count matches.
- The same fixture with `dry_run: true`: the directory still exists and the byte
  count is still reported.
- A gitignored directory is left alone by default.
- The same directory is deleted when `include_gitignored` is set.

## Documentation

README gains a headless section covering the three flags, the exit codes, and the
Claude Code `settings.json` `Stop` hook example from the issue.

## Known interaction

PR #17 also edits the `Args` struct in `main.rs` to add `--mcp`. Whichever branch
merges second needs a small conflict resolution there. Nothing structural overlaps.
