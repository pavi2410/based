# Contributing to Based

## Development

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

On Linux, run [Zed's `script/linux`](https://github.com/zed-industries/zed/blob/main/script/linux) once for GPUI build dependencies before building or packaging.

### Local packaging

Install [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) (`cargo install cargo-packager --locked`), then:

```bash
mise run package
```

Installers are written to `apps/desktop/dist/`. Packager configuration lives in the `[package.metadata.packager]` section of [`apps/desktop/Cargo.toml`](apps/desktop/Cargo.toml).

## Releasing

Releases are published via the **Release** GitHub Actions workflow (`.github/workflows/release.yml`, `workflow_dispatch`). Pick a bump type only — the workflow computes semver, commits the version bump, builds all platforms, tags, and publishes to [GitHub Releases](https://github.com/pavi2410/based/releases).

Semver logic lives in [`script/bump-version.py`](script/bump-version.py).

### Bump types

| Bump | From | To | GitHub release |
|------|------|----|----------------|
| `minor` | `0.1.0` | `0.2.0-rc.1` | prerelease |
| `rc` | `0.2.0-rc.1` | `0.2.0-rc.2` | prerelease |
| `stable` | `0.2.0-rc.2` | `0.2.0` | latest stable |
| `patch` | `0.2.0` | `0.2.1` | latest stable |
| `major` | `0.2.0` | `1.0.0-rc.1` | prerelease |

Versions containing `-rc` are published as GitHub prereleases; stable versions become the latest release.

**Do not hand-edit** `version` in `apps/desktop/Cargo.toml` before a release — the workflow owns the bump. Hand-edit only when recovering from a failed run.

### Example flow

1. Merge features on `main`.
2. Actions → **Release** → `bump: minor` → `v0.2.0-rc.1` + installers.
3. Smoke-test; fix bugs on `main`.
4. Actions → **Release** → `bump: rc` → `v0.2.0-rc.2`.
5. When ready, `bump: stable` → `v0.2.0` as latest.
6. Hotfix → `bump: patch` → `v0.2.1`.

**First release from `0.1.0`:** use `bump: minor` (not `rc`) to open the `0.2.0-rc.1` line.

Branch protection on `main` must allow `github-actions[bot]` to push version commits.
