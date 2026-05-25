#!/usr/bin/env python3
"""Compute (and optionally write) the next semver for Based releases (RC model).

Usage: bump-version.py <rc|stable|patch|minor|major> [--write]

Prints the new version to stdout. With ``--write``, also rewrites the
``[package].version`` line of ``apps/desktop/Cargo.toml`` in place.
Exits non-zero on invalid transitions.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

RC_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)-rc\.(\d+)$")
STABLE_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
PACKAGE_VERSION_RE = re.compile(
    r"(\[package\][^\[]*?^version\s*=\s*\")([^\"]+)(\")",
    re.MULTILINE | re.DOTALL,
)


def fail(message: str) -> None:
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_current_version(cargo_toml: Path) -> str:
    text = cargo_toml.read_text(encoding="utf-8")
    match = PACKAGE_VERSION_RE.search(text)
    if not match:
        fail(f"could not read [package] version from {cargo_toml}")
    return match.group(2)


def write_new_version(cargo_toml: Path, new_version: str) -> None:
    text = cargo_toml.read_text(encoding="utf-8")
    updated, count = PACKAGE_VERSION_RE.subn(
        lambda m: m.group(1) + new_version + m.group(3),
        text,
        count=1,
    )
    if count != 1:
        fail(f"could not rewrite [package] version in {cargo_toml}")
    cargo_toml.write_text(updated, encoding="utf-8")


def bump_version(bump: str, current: str) -> str:
    m_rc = RC_RE.match(current)
    m_stable = STABLE_RE.match(current)

    if bump == "rc":
        if not m_rc:
            fail(
                f"cannot bump rc from non-rc version {current!r}; use minor or major"
            )
        major, minor, patch, rc = map(int, m_rc.groups())
        return f"{major}.{minor}.{patch}-rc.{rc + 1}"

    if bump == "stable":
        if not m_rc:
            fail(f"cannot cut stable from {current!r}; current version is not an rc")
        major, minor, patch, _rc = map(int, m_rc.groups())
        return f"{major}.{minor}.{patch}"

    if bump == "patch":
        if m_rc:
            fail(
                f"cannot patch from rc version {current!r}; cut stable first or bump rc"
            )
        if not m_stable:
            fail(f"invalid semver: {current!r}")
        major, minor, patch = map(int, m_stable.groups())
        return f"{major}.{minor}.{patch + 1}"

    if bump == "minor":
        if m_rc:
            fail(
                f"cannot start minor rc line from {current!r}; cut stable or bump rc first"
            )
        if not m_stable:
            fail(f"invalid semver: {current!r}")
        major, minor, _patch = map(int, m_stable.groups())
        return f"{major}.{minor + 1}.0-rc.1"

    if bump == "major":
        if m_rc:
            fail(
                f"cannot start major rc line from {current!r}; cut stable or bump rc first"
            )
        if not m_stable:
            fail(f"invalid semver: {current!r}")
        major, _minor, _patch = map(int, m_stable.groups())
        return f"{major + 1}.0.0-rc.1"

    fail(f"unknown bump type {bump!r}")
    raise AssertionError("unreachable")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "bump",
        choices=["rc", "stable", "patch", "minor", "major"],
        help="release bump type",
    )
    parser.add_argument(
        "--cargo-toml",
        default=os.environ.get("CARGO_TOML", "apps/desktop/Cargo.toml"),
        type=Path,
        help="path to apps/desktop/Cargo.toml (default: apps/desktop/Cargo.toml)",
    )
    parser.add_argument(
        "--write",
        action="store_true",
        help="rewrite [package].version in the Cargo.toml in place",
    )
    args = parser.parse_args()

    current = read_current_version(args.cargo_toml)
    new_version = bump_version(args.bump, current)
    if args.write:
        write_new_version(args.cargo_toml, new_version)
    print(new_version)


if __name__ == "__main__":
    main()
