# Based — project TODO

## Based CLI

Headless `based` binary for CI, scripts, and power users (separate from the desktop GPUI app). Reuse `based-project` and existing `mise` / `cargo` workflows where possible.

- [ ] **`based open <path>`** — validate `.based/`, print resolved project root (optional: set cwd for child processes)
- [ ] **`based validate`** — load and validate project manifest, connections, queries (wrap `based_project::load_project`)
- [ ] **`based check`** — fast compile check (`cargo check` workspace or targeted crates)
- [ ] **`based lint`** — clippy (`cargo clippy --workspace --all-targets`)
- [ ] **`based format`** — `cargo fmt --all` (+ web format if scoped to monorepo)
- [ ] **Workspace crate** — e.g. `apps/cli` or `crates/based-cli`, wired in root `Cargo.toml` and `mise.toml`

## Desktop

See [apps/desktop/TODO.md](apps/desktop/TODO.md) for Open Project follow-ups, tab strip upstream parity, and workspace-specific work.
