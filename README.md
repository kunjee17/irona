# irona

A terminal UI tool for reclaiming disk space from build artifacts. Supports Rust (`target/`), Node.js (`node_modules/`), and C# (`bin/`, `obj/`) projects.

## Requirements

- [Rust toolchain](https://rustup.rs/) (stable)

## Run from source

```bash
git clone https://github.com/kunjee17/irona
cd irona
cargo run
```

## Install from source

```bash
cargo install --path .
```

Then run with:

```bash
irona
```

## Usage

- **↑ / ↓** — navigate entries
- **Space** — select / deselect entry
- **d** — delete selected entries
- **q / Esc** — quit

irona scans your home directory for build artifact folders and shows their size. Select what you want to clean up and press `d` to delete.
