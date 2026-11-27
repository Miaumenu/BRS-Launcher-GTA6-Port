#!/usr/bin/env python3
"""Check required project docs and local Markdown links using only stdlib."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"(?<!!)\[[^\]]+\]\((<?[^)]+>?)\)")
FENCED = re.compile(r"```.*?```|~~~.*?~~~", re.DOTALL)
REQUIRED = (
    "README.md",
    "CONTRIBUTING.md",
    "docs/architecture.md",
    "docs/hardware-compatibility.md",
    "docs/gpu-portability.md",
    "reports/hardware/_TEMPLATE.md",
    "configs/gta6-pc-port.example.toml",
)
ARCHIVE_DIRS = {"docs/history", "reports/archive"}


def markdown_files() -> list[Path]:
    files = []
    for path in ROOT.rglob("*.md"):
        relative = path.relative_to(ROOT).as_posix()
        if any(relative == archived or relative.startswith(archived + "/") for archived in ARCHIVE_DIRS):
            continue
        if ".git" not in path.parts and "target" not in path.parts:
            files.append(path)
    return files


def main() -> int:
    errors: list[str] = []
    for relative in REQUIRED:
        if not (ROOT / relative).is_file():
            errors.append(f"required file is missing: {relative}")

    for source in markdown_files():
        text = FENCED.sub("", source.read_text(encoding="utf-8"))
        for raw in LINK.findall(text):
            destination = raw.strip().strip("<>").strip()
            if not destination or destination.startswith("#"):
                continue
            parsed = urlsplit(destination)
            if parsed.scheme or destination.startswith("//"):
                continue
            target = unquote(parsed.path)
            if not target:
                continue
            candidate = (ROOT / target.lstrip("/")) if target.startswith("/") else source.parent / target
            if not candidate.exists():
                errors.append(
                    f"{source.relative_to(ROOT).as_posix()}: local link target is missing: {destination}"
                )

    if errors:
        print("Documentation check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"Documentation check passed ({len(markdown_files())} Markdown files checked).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
