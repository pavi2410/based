# Based - Native Desktop Database App

Based is a local-first, git-friendly desktop database client written in Rust.

## Features
- Native GPUI desktop client (`apps/desktop-native`)
- Local project metadata in `.based/config.toml`
- SQLite, PostgreSQL, and MongoDB workflows
- No backend service; data stays on your machine

## Project Structure

```text
based/
├── apps/
│   └── desktop-native/   # Main native desktop app (Rust + GPUI)
├── docs/
├── .based/
└── mise.toml             # Task runner configuration
```

## Quick Start

### Prerequisites
- Rust (latest stable)

### Development

```bash
cargo run -p desktop-native
# or
mise run dev
```

### Build

```bash
cargo build -p desktop-native --release
# or
mise run build
```

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
```
