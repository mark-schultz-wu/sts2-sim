#!/usr/bin/env python3
"""Lint Rust doc comments against a subset of ASD-STE100.

Usage:
    tools/prose-lint.py [PATH ...]
    tools/prose-lint.py --max-words 30 crates/sts2-core
    tools/prose-lint.py --explain
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

MAX_WORDS = 25
BANNED_MODALS = ["should", "would", "could", "may", "might"]
MAX_ITEM_LINES = 4
MAX_BLOCK_LINES = 12
HUB_FILES = ("mod.rs", "lib.rs")
DANGLING_ANY = "needs|need|is|are|was|were|has|have|does|do|gives|give|takes|take"
DANGLING_THE = "is|are|was|were|has|have|does|do"
MANDATED_SECTIONS = ("# errors", "# panics", "# safety")

SLOP = {
    "leverage": "use",
    "utilize": "use",
    "in order to": "to",
    "prior to": "before",
    "it is worth noting that": "",
    "simply": "",
    "just simply": "",
    "easily": "",
    "seamlessly": "",
    "robust": "",
    "powerful": "",
    "comprehensive": "",
    "functionality": "function",
    "enables you to": "you can",
    "allows you to": "you can",
    "is designed to": "",
    "facilitate": "help",
    "delve into": "read",
    "under the hood": "internally",
    "out of the box": "by default",
    "plethora": "many",
    "myriad": "many",
}

RULES = {
    "long-sentence": f"a sentence over {MAX_WORDS} words. Split it.",
    "em-dash": "an em-dash. Use a full stop, a comma, or parentheses.",
    "semicolon": "a semicolon. Write two sentences.",
    "banned-modal": "an ambiguous modal. Use `can`, `will` or `must`.",
    "slop": "a word that carries no fact.",
    "item-volume": f"over {MAX_ITEM_LINES} prose lines on one item.",
    "block-lines": f"a doc block over {MAX_BLOCK_LINES} lines.",
    "dangling-article": "an article with no noun. A word was deleted.",
    "doubled-word": "a word repeated. An edit landed twice.",
}

EXPLAIN = """\
The rules keep doc comments short, literal, and unambiguous. Sentence and block
limits prevent walls of text. The punctuation and modal rules require authors
to state the relation and force between clauses. Slop, dangling-article, and
doubled-word checks catch prose that carries no fact or was damaged by editing.

Passive voice and one-word-one-meaning are not checked because they require
judgement and would produce enough false positives to make the output noise.
"""


class Finding:
    __slots__ = ("path", "line", "rule", "detail")

    def __init__(self, path: str, line: int, rule: str, detail: str) -> None:
        self.path, self.line, self.rule, self.detail = path, line, rule, detail

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.rule}: {self.detail}"


class Block:
    __slots__ = ("kind", "lines")

    def __init__(self, kind: str, lines: list[tuple[int, str]]) -> None:
        self.kind, self.lines = kind, lines

    @property
    def start(self) -> int:
        return self.lines[0][0]


def doc_blocks(lines: list[str]) -> list[Block]:
    blocks: list[Block] = []
    kind = ""
    current: list[tuple[int, str]] = []
    for number, line in enumerate(lines, start=1):
        match = re.match(r"\s*(///|//!)\s?(.*)", line)
        # A code generator can contain escaped `/// ...\\n\\` text inside a
        # Rust string. The generated file is linted separately; this source
        # line is string data, not a doc comment in the generator itself.
        if match and ("\\n\\" in match.group(2) or line.rstrip().endswith("\\")):
            match = None
        if match and match.group(1) == kind:
            current.append((number, match.group(2)))
            continue
        if current:
            blocks.append(Block(kind, current))
            current = []
        kind = match.group(1) if match else ""
        if match:
            current = [(number, match.group(2))]
    if current:
        blocks.append(Block(kind, current))
    return blocks


def prose_lines(block: Block) -> list[tuple[int, str, bool]]:
    kept: list[tuple[int, str, bool]] = []
    fenced = False
    mandated = False
    for number, text in block.lines:
        stripped = text.strip()
        if stripped.startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        if stripped.startswith("#"):
            mandated = stripped.lower() in MANDATED_SECTIONS
            continue
        listed = re.match(r"(\d+\.|[-*>])\s", stripped)
        if not stripped or listed or stripped.startswith("|"):
            continue
        kept.append((number, text, mandated))
    return kept


def prose_of(block: Block) -> str:
    texts = [text.rstrip() for _, text, _ in prose_lines(block)]
    joined = " ".join(f"{text}." if text.endswith(":") else text for text in texts)
    return re.sub(r"`[^`]*`", "█", joined)


def volume(block: Block, path: pathlib.Path) -> list[Finding]:
    findings: list[Finding] = []
    hub_header = block.kind == "//!" and path.name in HUB_FILES
    if not hub_header and len(block.lines) > MAX_BLOCK_LINES:
        findings.append(
            Finding(str(path), block.start, "block-lines", f"{len(block.lines)} lines, over {MAX_BLOCK_LINES}")
        )
    if block.kind == "///":
        counted = [n for n, _, mandated in prose_lines(block) if not mandated]
        if len(counted) > MAX_ITEM_LINES:
            findings.append(
                Finding(str(path), block.start, "item-volume", f"{len(counted)} prose lines, over {MAX_ITEM_LINES}")
            )
    return findings


def check(path: pathlib.Path, max_words: int) -> list[Finding]:
    findings: list[Finding] = []
    lines = path.read_text(encoding="utf-8").split("\n")
    for block in doc_blocks(lines):
        findings.extend(volume(block, path))
        prose = prose_of(block)
        if not prose.strip():
            continue
        for sentence in re.split(r"(?<=[.!?])\s+", prose):
            words = len(sentence.split())
            if words > max_words:
                findings.append(Finding(str(path), block.start, "long-sentence", f"{words} words: {sentence[:70]}…"))
        fenced = False
        for number, text in block.lines:
            if text.strip().startswith("```"):
                fenced = not fenced
                continue
            if fenced:
                continue
            body = re.sub(r"`[^`]*`", "█", text)
            if "—" in body:
                findings.append(Finding(str(path), number, "em-dash", text.strip()[:70]))
            if ";" in body:
                findings.append(Finding(str(path), number, "semicolon", text.strip()[:70]))
        dangling = rf"\b(?:(?:a|an) (?:{DANGLING_ANY})|the (?:{DANGLING_THE}))(?![-\w])"
        for match in re.finditer(dangling, prose, re.I):
            findings.append(Finding(str(path), block.start, "dangling-article", f"`{match.group(0)}`"))
        for match in re.finditer(r"\b([a-z]{3,}) \1\b", prose, re.I):
            findings.append(Finding(str(path), block.start, "doubled-word", f"`{match.group(0)}`"))
        padded = f" {prose.lower()} "
        for modal in BANNED_MODALS:
            if re.search(rf"\b{modal}\b", padded):
                findings.append(Finding(str(path), block.start, "banned-modal", f"`{modal}`"))
        for word, better in SLOP.items():
            if re.search(rf"\b{re.escape(word)}\b", padded):
                advice = f"use `{better}`" if better else "delete it"
                findings.append(Finding(str(path), block.start, "slop", f"`{word}` — {advice}"))
    return findings


def sources(paths: list[str]) -> list[pathlib.Path]:
    found: list[pathlib.Path] = []
    for raw in paths:
        path = pathlib.Path(raw)
        if path.is_dir():
            found.extend(
                source
                for source in sorted(path.rglob("*.rs"))
                if "generated" not in source.parts
            )
        elif path.suffix == ".rs" and "generated" not in path.parts:
            found.append(path)
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", default=["crates", "xtask"])
    parser.add_argument("--max-words", type=int, default=MAX_WORDS)
    parser.add_argument("--rule", action="append", help="check only these rules")
    parser.add_argument("--explain", action="store_true", help="why each rule exists")
    parser.add_argument("--summary", action="store_true", help="counts only")
    args = parser.parse_args()
    if args.explain:
        print(EXPLAIN, end="")
        return 0
    findings = [finding for path in sources(args.paths) for finding in check(path, args.max_words)]
    if args.rule:
        findings = [finding for finding in findings if finding.rule in args.rule]
    counts: dict[str, int] = {}
    for finding in findings:
        counts[finding.rule] = counts.get(finding.rule, 0) + 1
    if not args.summary:
        for finding in findings:
            print(finding)
    if findings:
        order = sorted(counts.items(), key=lambda item: -item[1])
        print("\nprose-lint: " + ", ".join(f"{n} {rule}" for rule, n in order) + f" ({len(findings)} total).", file=sys.stderr)
        return 1
    print("prose-lint: clean", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
