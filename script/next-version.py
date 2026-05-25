#!/usr/bin/env python3
"""Compute the next CalVer version (vYYYY.M.PATCH) from git tags.

Scans existing ``v*`` tags in the current repository, finds the highest
PATCH for the current UTC ``YYYY.M``, and prints the next version on
stdout. If no tag exists for the current month, prints ``vYYYY.M.0``.

Output format matches ``script/set-version.py``'s validator: 4-digit
year, non-zero-padded month, non-zero-padded patch. The non-padding is
required because Cargo enforces strict SemVer in ``[package].version``
and rejects leading zeros (e.g. ``2026.05.0``).

Designed to be the single source of truth for "what version are we
about to release", used by both CI (in ``.github/workflows/release.yml``)
and maintainers locally (``python3 script/next-version.py``).
"""

from __future__ import annotations

import datetime as dt
import re
import subprocess
import sys

TAG_RE = re.compile(r"^v(\d{4})\.([1-9]\d*)\.(0|[1-9]\d*)$")


def git_tags() -> list[str]:
    """Return all ``v*`` tags in the current repository."""
    result = subprocess.run(
        ["git", "tag", "--list", "v*"],
        check=True,
        capture_output=True,
        text=True,
    )
    return [line.strip() for line in result.stdout.splitlines() if line.strip()]


def next_patch(tags: list[str], year: int, month: int) -> int:
    """Return the next PATCH number for ``year.month`` given existing tags.

    Tags that don't match the strict ``vYYYY.M.PATCH`` shape (e.g.
    legacy ``v0.1.0`` semver tags or zero-padded ``v2026.05.0``) are
    ignored, so a single bad tag can't poison the computation.
    """
    highest = -1
    for tag in tags:
        m = TAG_RE.match(tag)
        if not m:
            continue
        ty, tm, tp = int(m.group(1)), int(m.group(2)), int(m.group(3))
        if ty == year and tm == month and tp > highest:
            highest = tp
    return highest + 1


def main() -> int:
    now = dt.datetime.now(dt.timezone.utc)
    patch = next_patch(git_tags(), now.year, now.month)
    print(f"v{now.year}.{now.month}.{patch}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
