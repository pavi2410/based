# Contributing to Based

## Development

Prerequisites, common tasks, and optional local database setup are in [README.md](README.md).

On Linux, run [Zed's `script/linux`](https://github.com/zed-industries/zed/blob/main/script/linux) once for GPUI build dependencies before building or packaging.

Git hooks (via [lefthook](https://github.com/evilmartians/lefthook)) run automatically after `mise install`:

- **pre-commit:** Rust — `lint-fix` → `format`; Web — `lint-fix` → `format` (oxlint/oxfmt)
- **pre-push:** Rust — `lint` → `test`; Web — `lint` → `check` (astro check)

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
   - Builds installers for macOS (arm64), Linux, and Windows with `cargo-packager`, plus signed updater bundles and a `latest.json` manifest for in-app updates.
   - Publishes a GitHub Release at the new tag with auto-generated notes from PRs/commits since the previous tag and all installers.
   - Bumps the [Homebrew cask](https://github.com/pavi2410/homebrew-tap) and opens a [winget-pkgs](https://github.com/microsoft/winget-pkgs) PR when `HOMEBREW_TAP_TOKEN` / `WINGET_RELEASE_TOKEN` secrets are set.
3. (Optional) Edit the release on GitHub afterward to polish the auto-generated notes.

### In-app updater signing (one-time setup)

Generate a minisign key pair for update manifests:

```bash
cargo install cargo-packager --locked
cargo packager signer generate --path apps/desktop/assets/updater-key --ci --force
```

- Commit **`apps/desktop/assets/updater-key.pub`** (already gitignored: `apps/desktop/assets/updater-key` private key).
- Store the private key in the GitHub Actions secret **`UPDATER_PRIVATE_KEY`** using the [GitHub CLI](https://cli.github.com/):

```bash
# From the repo root, after generating the key pair above:
gh secret set UPDATER_PRIVATE_KEY < apps/desktop/assets/updater-key

# Or inline (same result):
gh secret set UPDATER_PRIVATE_KEY --body "$(cat apps/desktop/assets/updater-key)"

# Verify the secret exists (value is never shown):
gh secret list | grep UPDATER_PRIVATE_KEY
```

Each release job signs platform updater artifacts (`.app.tar.gz`, `.AppImage`, NSIS `.exe`) and uploads `latest.json` alongside user-facing installers. Update signing is separate from Apple/Microsoft code signing — Gatekeeper/SmartScreen warnings on first install may still appear.

### Manual download verification

GitHub exposes a SHA-256 digest for every release asset. After downloading, compare locally:

```bash
shasum -a 256 Based_2026.5.0_aarch64.dmg
# vs assets[].digest from GET /repos/pavi2410/based/releases/latest
```

### Notes

- **No bot push to `main`.** The version is never committed back; the tag is created by `gh release create` pointing at the workflow's commit SHA.
- **Do not hand-edit** the `[package].version` in `apps/desktop/Cargo.toml` — it should always be `0.0.0-dev`. The release flow owns the real version end-to-end.
- **First release:** if no prior `v*` tag exists for the current month, the action starts at `.0`; no bootstrap tag is required.

## Package managers (Homebrew + winget)

GitHub Releases remain the only artifact builder. Homebrew and winget are thin manifests (URL + SHA-256) updated after each release. See [packaging/README.md](packaging/README.md) for filename contracts.

### Homebrew (macOS arm64)

Tap repo: [pavi2410/homebrew-tap](https://github.com/pavi2410/homebrew-tap)

```bash
brew tap pavi2410/tap
brew install --cask based
```

After each release, CI runs `script/bump-packaging.py --homebrew` when **`HOMEBREW_TAP_TOKEN`** is set (fine-grained PAT with **contents: write** on `pavi2410/homebrew-tap` only).

Manual bump:

```bash
VERSION=v2026.5.2 python3 script/bump-packaging.py --homebrew --dry-run
HOMEBREW_TAP_TOKEN=... VERSION=v2026.5.2 python3 script/bump-packaging.py --homebrew
```

### winget (Windows x64)

Package ID: **`pavi2410.Based`**

```powershell
winget install pavi2410.Based
```

**One-time bootstrap:** fork [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs), copy manifests from `packaging/winget/` templates (or run `VERSION=v2026.5.2 python3 script/bump-packaging.py --winget-snapshot`), and open a PR. Full steps: [packaging/winget/BOOTSTRAP.md](packaging/winget/BOOTSTRAP.md).

**Ongoing updates:** set GitHub secret **`WINGET_RELEASE_TOKEN`** (PAT with fork + PR access to your `winget-pkgs` fork). The release workflow runs [winget-releaser](https://github.com/vedantmgoyal9/winget-releaser) and opens a bump PR — merge when convenient.
