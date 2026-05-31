# winget bootstrap (one-time)

Submit **pavi2410.Based** to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) once, then let [winget-releaser](https://github.com/vedantmgoyal/winget-releaser) open update PRs on each release.

Use a **working** release (v2026.5.2 or later — not v2026.5.1, which crashes on launch).

## Prerequisites

1. Fork [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs/fork)
2. Install [wingetcreate](https://github.com/microsoft/winget-cli/blob/master/doc/windows/package-manager/winget/create.md) or use GitHub Codespaces on your fork
3. Store a PAT in GitHub secret **`WINGET_RELEASE_TOKEN`** (repo scope on your fork + PR to upstream)

## Generate manifests from a release

From the `based` repo root after publishing v2026.5.2:

```bash
VERSION=v2026.5.2 python3 script/bump-packaging.py --winget-snapshot
```

Output: `packaging/winget/generated/2026.5.2/` (three YAML files ready to copy).

Or fetch assets manually:

```bash
gh release view v2026.5.2 --repo pavi2410/based --json assets \
  --jq '.assets[] | select(.name | test("x64-setup.exe$")) | {name, digest, url: .browser_download_url}'
```

## Copy into winget-pkgs

On your fork, create:

```
manifests/p/pavi2410/Based/2026.5.2/
  pavi2410.Based.yaml
  pavi2410.Based.locale.en-US.yaml
```

The version manifest inlines installer fields; you only need **version + locale** files (drop `.installer.yaml` unless validation asks for it).

## Validate locally (Windows or CI)

```powershell
winget validate manifests/p/pavi2410/Based/2026.5.2/pavi2410.Based.yaml
```

Or use the winget-pkgs PR template — automated validation runs on the PR.

## Open PR

1. Branch: `based-2026.5.2` (or similar)
2. PR title: `New package: pavi2410.Based version 2026.5.2`
3. Fill the checklist in the PR template
4. Wait for community review (often 1–3 days)

## After merge

Users can install:

```powershell
winget install pavi2410.Based
```

## Ongoing updates

The release workflow runs **winget-releaser** when `WINGET_RELEASE_TOKEN` is set. It opens a PR bumping `PackageVersion` and installer URL/SHA-256 — merge when convenient.

If the secret is missing, the step is skipped with a warning (same pattern as `UPDATER_PRIVATE_KEY`).
