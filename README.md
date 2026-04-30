# Based - Native Desktop Database App

Based is a local-first, git-friendly desktop database client written in Rust.

## Features
- Native GPUI desktop client (`apps/desktop`)
- Local project metadata in `.based/config.toml`
- SQLite, PostgreSQL, and MongoDB workflows
- No backend service; data stays on your machine

## Project Structure

```text
based/
├── apps/
│   └── desktop/   # Main native desktop app (Rust + GPUI)
├── docs/
├── .based/
└── mise.toml             # Task runner configuration
```

## Quick Start

### Prerequisites
- Rust (latest stable)
- [mise](https://mise.jdx.dev/) task runner

### Tasks

| Task | Command |
|------|---------|
| Run (dev) | `mise run dev` |
| Release build | `mise run build` |
| Fast check | `mise run check` |
| Lint | `mise run lint` |
| Format | `mise run format` |
| Test | `mise run test` |
