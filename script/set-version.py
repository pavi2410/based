#!/usr/bin/env python3
"""Write a CalVer version into apps/desktop/Cargo.toml.

Usage: set-version.py YYYY.MM.N  (a leading ``v`` is accepted and stripped)

Validates the CalVer shape and rewrites the first ``version = "..."`` line of
``apps/desktop/Cargo.toml`` (which is the ``[package].version`` since it is
the first such line in the file).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

CALVER = re.compile(r"^\d{4}\.\d{2}\.\d+$")
CARGO = Path(__file__).resolve().parent.parent / "apps" / "desktop" / "Cargo.toml"


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: set-version.py YYYY.MM.N", file=sys.stderr)
        return 2
    version = sys.argv[1].lstrip("v")
    if not CALVER.match(version):
        print(f"invalid CalVer: {version!r}", file=sys.stderr)
        return 2
    text = CARGO.read_text(encoding="utf-8")
    new = re.sub(
        r'^(version\s*=\s*")[^"]+(")',
        rf"\g<1>{version}\g<2>",
        text,
        count=1,
        flags=re.MULTILINE,
    )
    if new == text:
        print(f"Cargo.toml [package].version not found in {CARGO}", file=sys.stderr)
        return 1
    CARGO.write_text(new, encoding="utf-8")
    print(f"set {CARGO} version to {version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
