# Based — Git-Friendly Database Client

Based is a local-first desktop database client written in Rust. Connection configs and saved queries live in a committed `.based/` folder — no backend service, data stays on your machine.

**Website:** [based.pavi2410.com](https://based.pavi2410.com)

## Features

- Native GPUI desktop app ([`apps/desktop`](apps/desktop))
- Git-friendly project format: `project.toml`, `connections/`, and `queries/` under `.based/`
- PostgreSQL, SQLite, and MongoDB workflows
- In-app updates on release builds (signed updater manifests)
- No backend service — connections and queries run locally

See **[The `.based` project format](docs/based-project/README.md)** for the full specification.

## Project structure

```text
based/
├── apps/
│   ├── desktop/   # Main native app (Rust + GPUI)
│   └── web/       # Marketing site (Astro + Cloudflare)
├── crates/        # Shared libraries (project format, engines, query layer)
├── docs/          # Project format spec and engineering docs
├── .based/        # Example project config for this repo
├── Cargo.toml
└── mise.toml      # Task runner
```

For architecture details, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Quick start

### Prerequisites

- Rust (latest stable)
- [mise](https://mise.jdx.dev/) task runner

On Linux, run [Zed's `script/linux`](https://github.com/zed-industries/zed/blob/main/script/linux) once for GPUI build dependencies.

Optional — local databases for development:

```bash
docker compose up -d   # Postgres → localhost:15432, MongoDB → localhost:37017
```

### Tasks

| Task | Command |
|------|---------|
| Run (dev) | `mise run dev` |
| Release build | `mise run build` |
| Fast check | `mise run check` |
| Lint | `mise run lint` |
| Format | `mise run format` |
| Test | `mise run test` |
| Clean | `mise run clean` |
| Package installers | `mise run package` |

## Installing

Download the latest stable release from [GitHub Releases](https://github.com/pavi2410/based/releases). Stable builds are the latest non-prerelease; release candidates are tagged with `-rc.N` and marked as prereleases.

| Platform | Artifact | Package manager |
|----------|----------|-----------------|
| macOS (Apple Silicon) | `.dmg` | `brew tap pavi2410/tap && brew install --cask based` |
| Linux | `.deb` or `.AppImage` | — |
| Windows | `.exe` NSIS installer | `winget install pavi2410.Based` |

Each release asset shows a **SHA-256 digest** on the [GitHub Releases](https://github.com/pavi2410/based/releases) page (and in the API as `assets[].digest`). After downloading an installer, compare your local hash:

```bash
shasum -a 256 Based_2026.5.0_aarch64.dmg
```

Installers are **unsigned**. macOS may show Gatekeeper warnings; open **System Settings → Privacy & Security** and choose **Open Anyway**, or right-click the app and choose **Open**. Windows SmartScreen may warn on first run — choose **More info → Run anyway**.

Homebrew and winget install the same GitHub Release binaries — see [packaging/README.md](packaging/README.md) for maintainer details.

## Support

If Based is useful to you, consider [sponsoring development on GitHub](https://github.com/sponsors/pavi2410).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, local packaging, and the release workflow.
