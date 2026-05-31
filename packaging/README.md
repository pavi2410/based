# Packaging manifests

GitHub Releases are the **only artifact builder**. Homebrew and winget are thin manifests (download URL + SHA-256) updated after each release.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for release workflow and automation secrets.

## Artifact filename contract

Do not rename release assets casually — package managers match on these patterns.

| Platform | Filename pattern | Example |
|----------|------------------|---------|
| macOS arm64 | `Based_{version}_aarch64.dmg` | `Based_2026.5.2_aarch64.dmg` |
| Windows x64 | `based_{version}_x64-setup.exe` | `based_2026.5.2_x64-setup.exe` |
| Linux deb | `based_{version}_amd64.deb` | `based_2026.5.2_amd64.deb` |
| Linux AppImage | `based_{version}_x86_64.AppImage` | `based_2026.5.2_x86_64.AppImage` |

- **Git tag:** `v{version}` (e.g. `v2026.5.2`)
- **Version in filenames:** no leading `v` (e.g. `2026.5.2`)
- macOS DMG uses capital `Based_`; Windows/Linux use lowercase `based_`
- CI builds **macOS arm64 only** — Homebrew cask declares `depends_on arch: :arm64`

## SHA-256 source

Use GitHub release API digests (same as the website install cards):

```bash
gh release view v2026.5.2 --repo pavi2410/based --json assets \
  --jq '.assets[] | {name, digest}'
```

Strip the `sha256:` prefix for Homebrew cask and winget `InstallerSha256` fields.

## Layout

```
packaging/
  README.md                 # this file
  homebrew/
    based.rb.template       # rendered by script/bump-packaging.py
  winget/
    *.yaml.template         # reference manifests for bootstrap PR
    BOOTSTRAP.md            # one-time winget-pkgs submission guide
```

## Distribution channels

| Channel | Repo / location | Updated by |
|---------|-----------------|------------|
| Direct download | [GitHub Releases](https://github.com/pavi2410/based/releases) | `release.yml` build job |
| In-app updater | `latest.json` on release | `release.yml` release job |
| Homebrew cask | [pavi2410/homebrew-tap](https://github.com/pavi2410/homebrew-tap) | `script/bump-packaging.py --homebrew` |
| winget | [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) | [winget-releaser](https://github.com/vedantmgoyal/winget-releaser) on release |

## Manual bump (before automation or if CI secret missing)

```bash
VERSION=v2026.5.2 python3 script/bump-packaging.py --homebrew --dry-run
HOMEBREW_TAP_TOKEN=... VERSION=v2026.5.2 python3 script/bump-packaging.py --homebrew
```
