#!/usr/bin/env python3
"""Build latest.json for cargo-packager-updater from signed release artifacts.

Scans a dist directory for updater bundles and their `.sig` sidecars, then
emits a static multi-platform manifest suitable for upload to GitHub Releases
as ``latest.json``.

Platform keys follow cargo-packager-updater: ``macos-aarch64``,
``macos-x86_64``, ``linux-x86_64``, ``windows-x86_64``.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

GITHUB_REPO = "pavi2410/based"

# (filename regex, platform key, updater format)
RULES: list[tuple[re.Pattern[str], str, str]] = [
    (re.compile(r"\.app\.tar\.gz$", re.I), "macos", "app"),
    (re.compile(r"\.AppImage$", re.I), "linux-x86_64", "appimage"),
    (re.compile(r"-setup\.exe$", re.I), "windows-x86_64", "nsis"),
    (re.compile(r"\.exe$", re.I), "windows-x86_64", "nsis"),
]


def arch_suffix(name: str) -> str | None:
    lower = name.lower()
    if "aarch64" in lower or "arm64" in lower:
        return "aarch64"
    if "x86_64" in lower or "_x64" in lower or "-x64" in lower or "amd64" in lower:
        return "x86_64"
    return None


def platform_for(name: str, default_platform: str, fmt: str) -> str | None:
    if default_platform == "macos":
        arch = arch_suffix(name) or "aarch64"
        return f"macos-{arch}"
    if default_platform in ("linux-x86_64", "windows-x86_64"):
        return default_platform
    return None


def read_signature(sig_path: Path) -> str:
    return sig_path.read_text(encoding="utf-8").strip()


def artifact_for_sig(sig_path: Path) -> Path | None:
    name = sig_path.name
    if not name.endswith(".sig"):
        return None
    candidate = sig_path.with_name(name[: -len(".sig")])
    if candidate.is_file():
        return candidate
    return None


def detect_entry(artifact: Path) -> tuple[str, str] | None:
    name = artifact.name
    for pattern, default_platform, fmt in RULES:
        if pattern.search(name):
            platform = platform_for(name, default_platform, fmt)
            if platform:
                return platform, fmt
    return None


def build_manifest(
    dist: Path,
    version: str,
    tag: str,
    notes_url: str | None,
) -> dict:
    platforms: dict[str, dict[str, str]] = {}

    for sig_path in sorted(dist.rglob("*.sig")):
        artifact = artifact_for_sig(sig_path)
        if artifact is None:
            continue
        detected = detect_entry(artifact)
        if detected is None:
            continue
        platform, fmt = detected
        url = (
            f"https://github.com/{GITHUB_REPO}/releases/download/{tag}/{artifact.name}"
        )
        platforms[platform] = {
            "url": url,
            "signature": read_signature(sig_path),
            "format": fmt,
        }

    if not platforms:
        print("error: no signed updater artifacts found in dist", file=sys.stderr)
        sys.exit(1)

    manifest: dict = {
        "version": version,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }
    if notes_url:
        manifest["notes"] = notes_url
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--dist",
        type=Path,
        default=Path("dist"),
        help="Directory containing signed updater bundles",
    )
    parser.add_argument(
        "--version",
        required=True,
        help="SemVer release version without leading v (e.g. 2026.5.1)",
    )
    parser.add_argument(
        "--tag",
        required=True,
        help="Git tag for download URLs (e.g. v2026.5.1)",
    )
    parser.add_argument(
        "--notes-url",
        default=None,
        help="Optional release notes URL for the notes field",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=None,
        help="Write manifest here (default: stdout)",
    )
    args = parser.parse_args()

    version = args.version.lstrip("v")
    tag = args.tag if args.tag.startswith("v") else f"v{args.tag}"
    notes = args.notes_url or f"https://github.com/{GITHUB_REPO}/releases/tag/{tag}"

    manifest = build_manifest(args.dist, version, tag, notes)
    payload = json.dumps(manifest, indent=2) + "\n"

    if args.output:
        args.output.write_text(payload, encoding="utf-8")
    else:
        sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    sys.exit(main())
