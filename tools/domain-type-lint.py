#!/usr/bin/env python3
"""Refuse concrete game-number representations in version-owned Rust code."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys


CONCRETE = re.compile(r"\bi32\b|\bsts2_core::Fixed\b")
VERSION = re.compile(r"v\d+_\d+_\d+")


def sources(paths: list[str]) -> list[pathlib.Path]:
    if paths:
        candidates = [pathlib.Path(raw) for raw in paths]
    else:
        candidates = list(pathlib.Path("crates/sts2-content/src").glob("v*/tests"))
        candidates += list(pathlib.Path("crates/sts2-content/src").glob("v*/behaviour"))

    found: list[pathlib.Path] = []
    for path in candidates:
        if path.is_dir():
            found.extend(sorted(path.rglob("*.rs")))
        elif path.suffix == ".rs":
            found.append(path)
    return [path for path in found if "generated" not in path.parts]


def code_lines(text: str) -> list[str]:
    """Remove comments, while keeping line numbers stable."""
    kept: list[str] = []
    blocked = False
    for line in text.splitlines():
        code = ""
        at = 0
        while at < len(line):
            if blocked:
                end = line.find("*/", at)
                if end < 0:
                    at = len(line)
                else:
                    blocked = False
                    at = end + 2
            else:
                block = line.find("/*", at)
                comment = line.find("//", at)
                ends = [end for end in (block, comment) if end >= 0]
                if not ends:
                    code += line[at:]
                    at = len(line)
                else:
                    end = min(ends)
                    code += line[at:end]
                    if end == comment:
                        at = len(line)
                    else:
                        blocked = True
                        at = end + 2
        kept.append(code)
    return kept


def findings(path: pathlib.Path) -> list[str]:
    if not any(VERSION.fullmatch(part) for part in path.parts):
        return []
    return [
        f"{path}:{line}: concrete game number `{match.group(0)}`; use `Number` or a domain alias"
        for line, code in enumerate(code_lines(path.read_text()), start=1)
        # The random stream's wire algorithm names i32 itself. It is not a
        # content version's number domain.
        if not re.search(r"\btype Int = i32\b|\bfn counter\([^)]*\) -> i32\b", code)
        for match in CONCRETE.finditer(code)
    ]


def self_test() -> int:
    assert CONCRETE.search("let _: i32 = 1;")
    assert CONCRETE.search("let _: sts2_core::Fixed = value;")
    assert not CONCRETE.search("let _: CardNumber = value;")
    assert code_lines("/// `i32` is documentation.\nlet _: Number = 1;")[0] == ""
    print("domain-type-lint: self-test passed", file=sys.stderr)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()

    errors = [error for path in sources(args.paths) for error in findings(path)]
    for error in errors:
        print(error)
    if errors:
        print(f"domain-type-lint: {len(errors)} finding(s)", file=sys.stderr)
        return 1
    print("domain-type-lint: clean", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
