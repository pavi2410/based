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

Releases use CalVer in the form `vYYYY.M.PATCH`, where `PATCH` resets to `0` at the start of each calendar month (UTC) and increments per release within that month — for example, `v2026.5.0`, `v2026.5.1`, then `v2026.6.0`. Months 10-12 are double-digit (`v2026.10.0`). The month is **not** zero-padded because Cargo enforces strict SemVer in `[package].version` and forbids leading zeros (`2026.05.0` would be rejected). [`script/next-version.py`](script/next-version.py) is the single source of truth for "what version comes next" — both CI and maintainers can run it (`python3 script/next-version.py`) to preview the result.

The version in [`apps/desktop/Cargo.toml`](apps/desktop/Cargo.toml) is **permanently `0.0.0-dev`**. The real CalVer version is computed by CI and stamped into `Cargo.toml` in-memory on each runner before `cargo build`, so the installed binary reports the correct version via `env!("CARGO_PKG_VERSION")`. Local builds from `main` intentionally report `0.0.0-dev` — they are not release builds.

### To ship a release

1. Go to **Actions → Release → Run workflow** on GitHub, select `main`, click **Run workflow**.
2. CI:
   - Computes the next CalVer by running [`script/next-version.py`](script/next-version.py), which scans existing `v*` tags and emits `vYYYY.M.PATCH` for the current UTC month.
   - Stamps it into `Cargo.toml` on each runner via [`script/set-version.py`](script/set-version.py).
   - Builds installers for macOS (arm64, x64), Linux, and Windows with `cargo-packager`.
   - Publishes a GitHub Release at the new tag with auto-generated notes from PRs/commits since the previous tag, all installers, and a `checksums.txt`.
3. (Optional) Edit the release on GitHub afterward to polish the auto-generated notes.

### Notes

- **No bot push to `main`.** The version is never committed back; the tag is created by `gh release create` pointing at the workflow's commit SHA.
- **Do not hand-edit** the `[package].version` in `apps/desktop/Cargo.toml` — it should always be `0.0.0-dev`. The release flow owns the real version end-to-end.
- **First release:** if no prior `v*` tag exists for the current month, the action starts at `.0`; no bootstrap tag is required.
