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
| Package installers | `mise run package` |

## Installing

Download the latest stable release from [GitHub Releases](https://github.com/pavi2410/based/releases). Stable builds are the latest non-prerelease; release candidates are tagged with `-rc.N` and marked as prereleases.

| Platform | Artifact |
|----------|----------|
| macOS (Apple Silicon) | `.dmg` from the macOS arm64 job |
| macOS (Intel) | `.dmg` from the macOS x64 job |
| Linux | `.deb` or `.AppImage` |
| Windows | `.exe` NSIS installer |

Each release asset shows a **SHA-256 digest** on the [GitHub Releases](https://github.com/pavi2410/based/releases) page (and in the API as `assets[].digest`). After downloading an installer, compare your local hash:

```bash
shasum -a 256 Based_2026.5.0_aarch64.dmg
```

Installers are **unsigned**. macOS may show Gatekeeper warnings; open **System Settings → Privacy & Security** and choose **Open Anyway**, or right-click the app and choose **Open**. Windows SmartScreen may warn on first run — choose **More info → Run anyway**.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, local packaging, and the release workflow.
