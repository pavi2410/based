#!/usr/bin/env python3
"""Bump Homebrew cask and/or render winget manifests from a GitHub Release.

GitHub Releases remain the artifact source of truth. This script reads asset
URLs and SHA-256 digests from the release API and updates external manifests.

Usage:
  VERSION=v2026.5.2 python3 script/bump-packaging.py --homebrew
  VERSION=v2026.5.2 python3 script/bump-packaging.py --winget-snapshot
  VERSION=v2026.5.2 python3 script/bump-packaging.py --homebrew --dry-run

Environment:
  VERSION              Release tag (v2026.5.2) or bare version (2026.5.2)
  GITHUB_TOKEN         Optional; gh CLI uses its auth when unset
  HOMEBREW_TAP_TOKEN   PAT with contents:write on pavi2410/homebrew-tap (--homebrew push)
  HOMEBREW_TAP_REPO    Default: pavi2410/homebrew-tap
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

GITHUB_REPO = "pavi2410/based"
DEFAULT_TAP_REPO = "pavi2410/homebrew-tap"

MAC_DMG_RE = re.compile(r"Based_.*_aarch64\.dmg$", re.I)
WIN_EXE_RE = re.compile(r"based_.*_x64-setup\.exe$", re.I)

ROOT = Path(__file__).resolve().parent.parent
HOMEBREW_TEMPLATE = ROOT / "packaging/homebrew/based.rb.template"
WINGET_TEMPLATES = {
    "pavi2410.Based.yaml": ROOT / "packaging/winget/pavi2410.Based.yaml.template",
    "pavi2410.Based.installer.yaml": ROOT
    / "packaging/winget/pavi2410.Based.installer.yaml.template",
    "pavi2410.Based.locale.en-US.yaml": ROOT
    / "packaging/winget/pavi2410.Based.locale.en-US.yaml.template",
}


def normalize_tag(version: str) -> str:
    version = version.strip()
    return version if version.startswith("v") else f"v{version}"


def bare_version(tag: str) -> str:
    return tag.lstrip("v")


def parse_digest(digest: str | None) -> str:
    if not digest:
        raise ValueError("missing digest on release asset")
    prefix = "sha256:"
    if digest.startswith(prefix):
        return digest[len(prefix) :]
    return digest


def fetch_release(tag: str) -> dict:
    """Return release JSON via gh CLI or GitHub REST API."""
    try:
        result = subprocess.run(
            [
                "gh",
                "release",
                "view",
                tag,
                "--repo",
                GITHUB_REPO,
                "--json",
                "tagName,assets",
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        return json.loads(result.stdout)
    except (FileNotFoundError, subprocess.CalledProcessError):
        pass

    token = os.environ.get("GITHUB_TOKEN", "")
    url = f"https://api.github.com/repos/{GITHUB_REPO}/releases/tags/{tag}"
    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": "based-bump-packaging",
            **({"Authorization": f"Bearer {token}"} if token else {}),
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        print(f"error: GitHub API {e.code} for {url}", file=sys.stderr)
        raise SystemExit(1) from e

    return {
        "tagName": data["tag_name"],
        "assets": [
            {
                "name": a["name"],
                "digest": a.get("digest"),
                "url": a["browser_download_url"],
            }
            for a in data.get("assets", [])
        ],
    }


def find_asset(assets: list[dict], pattern: re.Pattern[str]) -> dict:
    for asset in assets:
        if pattern.search(asset.get("name", "")):
            return asset
    names = [a.get("name") for a in assets]
    raise SystemExit(
        f"error: no asset matching {pattern.pattern!r} in release assets: {names}"
    )


def render_template(path: Path, mapping: dict[str, str]) -> str:
    text = path.read_text(encoding="utf-8")
    for key, value in mapping.items():
        text = text.replace(f"{{{{{key}}}}}", value)
    if "{{" in text:
        print(f"warning: unreplaced placeholders in {path.name}", file=sys.stderr)
    return text


def bump_homebrew(
    version: str,
    tag: str,
    dmg_sha256: str,
    *,
    dry_run: bool,
    tap_repo: str,
) -> None:
    cask_body = render_template(
        HOMEBREW_TEMPLATE,
        {"VERSION": version, "SHA256": dmg_sha256},
    )

    if dry_run:
        print("--- Casks/based.rb (dry-run) ---")
        print(cask_body, end="" if cask_body.endswith("\n") else "\n")
        return

    token = os.environ.get("HOMEBREW_TAP_TOKEN")
    if not token:
        print(
            "error: HOMEBREW_TAP_TOKEN is required to push homebrew-tap",
            file=sys.stderr,
        )
        raise SystemExit(1)

    clone_url = f"https://x-access-token:{token}@github.com/{tap_repo}.git"
    with tempfile.TemporaryDirectory(prefix="homebrew-tap-") as tmp:
        tap_dir = Path(tmp) / "tap"
        subprocess.run(
            ["git", "clone", "--depth", "1", clone_url, str(tap_dir)],
            check=True,
            capture_output=True,
        )
        cask_path = tap_dir / "Casks" / "based.rb"
        cask_path.parent.mkdir(parents=True, exist_ok=True)
        cask_path.write_text(cask_body, encoding="utf-8")

        subprocess.run(["git", "add", "Casks/based.rb"], cwd=tap_dir, check=True)
        status = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=tap_dir,
            check=True,
            capture_output=True,
            text=True,
        )
        if not status.stdout.strip():
            print(f"homebrew: Casks/based.rb already at {version}; nothing to commit")
            return

        subprocess.run(
            ["git", "commit", "-m", f"Bump cask to {version}"],
            cwd=tap_dir,
            check=True,
            env={
                **os.environ,
                "GIT_AUTHOR_NAME": "github-actions[bot]",
                "GIT_AUTHOR_EMAIL": "41898282+github-actions[bot]@users.noreply.github.com",
                "GIT_COMMITTER_NAME": "github-actions[bot]",
                "GIT_COMMITTER_EMAIL": "41898282+github-actions[bot]@users.noreply.github.com",
            },
        )
        subprocess.run(["git", "push", "origin", "HEAD"], cwd=tap_dir, check=True)
        print(f"homebrew: pushed Casks/based.rb for {version} to {tap_repo}")


def winget_snapshot(
    version: str,
    tag: str,
    installer_url: str,
    installer_sha256: str,
) -> None:
    mapping = {
        "VERSION": version,
        "INSTALLER_URL": installer_url,
        "INSTALLER_SHA256": installer_sha256,
    }
    out_dir = ROOT / "packaging/winget/generated" / version
    out_dir.mkdir(parents=True, exist_ok=True)

    for filename, template_path in WINGET_TEMPLATES.items():
        body = render_template(template_path, mapping)
        (out_dir / filename).write_text(body, encoding="utf-8")
        print(f"winget: wrote {out_dir / filename}")

    print(f"winget: copy {out_dir}/ into winget-pkgs — see packaging/winget/BOOTSTRAP.md")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--homebrew",
        action="store_true",
        help="Update and push pavi2410/homebrew-tap Casks/based.rb",
    )
    parser.add_argument(
        "--winget-snapshot",
        action="store_true",
        help="Render winget YAML under packaging/winget/generated/",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print homebrew cask without pushing",
    )
    parser.add_argument(
        "--tap-repo",
        default=os.environ.get("HOMEBREW_TAP_REPO", DEFAULT_TAP_REPO),
        help=f"Homebrew tap repo (default: {DEFAULT_TAP_REPO})",
    )
    args = parser.parse_args()

    if not args.homebrew and not args.winget_snapshot and not args.dry_run:
        parser.error("specify --homebrew, --winget-snapshot, and/or --dry-run")

    raw_version = os.environ.get("VERSION", "")
    if not raw_version:
        print("error: set VERSION (e.g. VERSION=v2026.5.2)", file=sys.stderr)
        return 1

    tag = normalize_tag(raw_version)
    version = bare_version(tag)

    release = fetch_release(tag)
    assets = release.get("assets", [])

    if args.homebrew or args.dry_run:
        dmg = find_asset(assets, MAC_DMG_RE)
        bump_homebrew(
            version,
            tag,
            parse_digest(dmg.get("digest")),
            dry_run=args.dry_run,
            tap_repo=args.tap_repo,
        )

    if args.winget_snapshot:
        exe = find_asset(assets, WIN_EXE_RE)
        winget_snapshot(
            version,
            tag,
            exe.get("url") or f"https://github.com/{GITHUB_REPO}/releases/download/{tag}/{exe['name']}",
            parse_digest(exe.get("digest")),
        )

    return 0


if __name__ == "__main__":
    sys.exit(main())
