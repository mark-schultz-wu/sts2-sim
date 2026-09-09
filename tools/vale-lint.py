#!/usr/bin/env python3
"""Run Vale on every piece of prose in the repository."""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
DIRECT_SUFFIXES = {
    ".c", ".cc", ".cpp", ".cs", ".go", ".h", ".hpp", ".java", ".js",
    ".jsx", ".md", ".mdown", ".markdown", ".php", ".py", ".rb", ".rs",
    ".rst", ".tsx", ".ts",
}
HASH_SUFFIXES = {".bash", ".env", ".sh", ".toml", ".yaml", ".yml", ".zsh"}
HASH_NAMES = {"Dockerfile", "pre-push", "versions.env"}
HASH_COMMENT = re.compile(r"^\s*#(?!\!)(?:\s?)(.*)$")


def git(*args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=ROOT, check=True, text=True, stdout=subprocess.PIPE
    )
    return result.stdout


def resolve(name: str) -> str | None:
    """Return the commit that `name` points at, or None if git does not know it."""
    found = subprocess.run(
        ["git", "rev-parse", "--verify", f"{name}^{{commit}}"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    return found.stdout.strip() if found.returncode == 0 else None


def base_commit(requested: str | None) -> str:
    """Find the commit that this branch started from.

    A pull request names its base branch in GITHUB_BASE_REF. A push and a
    manual run have no base branch, so the parent commit is the base. The
    checkout must hold the full history, because a shallow one has no parent.
    """
    name = requested or os.environ.get("GITHUB_BASE_REF") or ""
    candidates = [name, f"origin/{name}"] if name else ["HEAD^"]
    for candidate in candidates:
        commit = resolve(candidate)
        if commit is not None:
            return git("merge-base", "HEAD", commit).strip()
    if name:
        raise SystemExit(
            f"vale-lint: the checkout has no branch {name!r}. "
            "The job must check out the full history."
        )
    if resolve("HEAD") is None:
        raise SystemExit("vale-lint: the checkout has no commits")
    raise SystemExit(
        "vale-lint: HEAD has no parent commit. A shallow checkout causes this. "
        "The job must set fetch-depth: 0, or you must name a base with --base."
    )


def is_prose_source(path: pathlib.Path) -> bool:
    suffix = path.suffix.lower()
    return suffix in DIRECT_SUFFIXES | HASH_SUFFIXES or path.name in HASH_NAMES


def source_files() -> list[str]:
    """Every tracked file that holds prose the style applies to."""
    names = git("ls-files", "-z").split("\0")
    return sorted(
        name for name in names
        if name and is_prose_source(pathlib.Path(name)) and (ROOT / name).is_file()
    )


def extract_hash_comments(source: pathlib.Path, destination: pathlib.Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)
    output: list[str] = []
    for line in source.read_text(encoding="utf-8").splitlines():
        match = HASH_COMMENT.match(line)
        content = match.group(1) if match else ""
        # An indented run inside a comment is an example, such as a usage
        # block. Markdown reads it as a paragraph when no blank line precedes
        # it, which joins the commands into one long sentence. Drop it instead,
        # which keeps every other line at the number the file gives it.
        output.append("" if content.startswith("    ") else content)
    destination.write_text("\n".join(output) + "\n", encoding="utf-8")


def run_vale(files: list[str], vale: str) -> list[tuple[str, dict[str, object]]]:
    direct: list[str] = []
    path_map: dict[str, str] = {}
    with tempfile.TemporaryDirectory(prefix="sts2-vale-") as raw_temp:
        temp = pathlib.Path(raw_temp)
        for name in files:
            source = ROOT / name
            if source.suffix.lower() in HASH_SUFFIXES or source.name in HASH_NAMES:
                target = temp / f"{name}.md"
                extract_hash_comments(source, target)
                direct.append(str(target))
                path_map[str(target)] = name
            else:
                direct.append(str(source))
                path_map[str(source)] = name

        if not direct:
            return []
        result = subprocess.run(
            [vale, "--config", str(ROOT / ".vale.ini"), "--output=JSON", *direct],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if result.returncode not in (0, 1):
            sys.stderr.write(result.stderr)
            raise SystemExit(result.returncode)
        report = json.loads(result.stdout or "{}")
        findings: list[tuple[str, dict[str, object]]] = []
        for reported, alerts in report.items():
            absolute = str(pathlib.Path(reported).resolve())
            original = path_map.get(absolute)
            if original is None:
                continue
            for alert in alerts:
                findings.append((original, alert))
        return findings


FIXTURES = (
    (
        "SentenceLength",
        "This sentence has more than twenty-five words because it repeats unnecessary "
        "words that add no useful facts and force the reader to remember too much "
        "information at one time.",
        "This sentence uses few words. It gives one fact to the reader.",
    ),
    (
        "Modals",
        "You should run it.",
        "Run it.",
    ),
    (
        "PassiveVoice",
        "The schema was carefully reviewed by legal.",
        "Legal reviewed the schema.",
    ),
    (
        "PerfectTense",
        "We have identified an error.",
        "We found an error. The game has nineteen versions of the content tables.",
    ),
    (
        "ProgressiveTense",
        "The service is running slowly.",
        "The service runs slowly. There is nothing in the table.",
    ),
    (
        "Punctuation",
        "The tool does not work; fix it now.",
        "The tool does not work. Fix it now.",
    ),
    (
        "Punctuation",
        "The tool does not work\u2014fix it now.",
        "The tool does not work. Fix it now.",
    ),
    (
        "Punctuation",
        "The tool does not work -- fix it now.",
        "The tool does not work. Fix it now.",
    ),
    (
        "WordChoice",
        "This robust tool leverages the cache in order to ensure speed.",
        "This tool reads the cache, so it answers in under one second.",
    ),
    (
        "Contractions",
        "We aren't ready.",
        "We are not ready.",
    ),
    (
        "Abbreviations",
        "The runner reads two hosts, e.g. git.schultz-wu.com and ghcr.io.",
        "The runner reads two hosts. The hosts are git.schultz-wu.com and ghcr.io.",
    ),
)


def self_test(vale: str) -> int:
    """Prove that each rule reports bad prose and accepts the corrected prose."""
    with tempfile.TemporaryDirectory(prefix="sts2-vale-test-") as raw_temp:
        temp = pathlib.Path(raw_temp)
        wanted: dict[str, str] = {}
        for index, (name, bad, good) in enumerate(FIXTURES):
            bad_file = temp / f"{index:02d}-{name}-bad.md"
            good_file = temp / f"{index:02d}-{name}-good.md"
            bad_file.write_text(bad + "\n", encoding="utf-8")
            good_file.write_text(good + "\n", encoding="utf-8")
            wanted[str(bad_file)] = name
            wanted[str(good_file)] = ""
        result = subprocess.run(
            [vale, "--config", str(ROOT / ".vale.ini"), "--output=JSON",
             *sorted(wanted)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    if result.returncode not in (0, 1):
        sys.stderr.write(result.stderr)
        return result.returncode
    report = json.loads(result.stdout or "{}")
    found: dict[str, set[str]] = {path: set() for path in wanted}
    for reported, alerts in report.items():
        path = str(pathlib.Path(reported).resolve())
        if path not in found:
            continue
        found[path].update(str(alert["Check"]) for alert in alerts)

    failures: list[str] = []
    for path, name in sorted(wanted.items()):
        checks = found[path]
        label = pathlib.Path(path).name
        if name:
            if f"SimpleEnglish.{name}" not in checks:
                failures.append(f"{label}: rule {name} did not report the bad prose")
        elif checks:
            failures.append(f"{label}: false positive from " + ", ".join(sorted(checks)))
    if failures:
        for failure in failures:
            print(f"vale-lint: {failure}", file=sys.stderr)
        return 1
    print(f"vale-lint: self-test passed {len(FIXTURES)} rule fixtures", file=sys.stderr)
    return 0


def lint_texts(vale: str, texts: dict[str, str]) -> int:
    """Lint each named piece of markdown. Report every finding in each one."""
    with tempfile.TemporaryDirectory(prefix="sts2-vale-text-") as raw_temp:
        temp = pathlib.Path(raw_temp)
        labels: dict[str, str] = {}
        for index, (label, text) in enumerate(texts.items()):
            sample = temp / f"{index:04d}.md"
            sample.write_text(text, encoding="utf-8")
            labels[str(sample)] = label
        result = subprocess.run(
            [vale, "--config", str(ROOT / ".vale.ini"), "--output=JSON", *sorted(labels)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    if result.returncode not in (0, 1):
        sys.stderr.write(result.stderr)
        return result.returncode
    findings: list[tuple[str, int, int, str, str]] = []
    for reported, alerts in json.loads(result.stdout or "{}").items():
        label = labels.get(str(pathlib.Path(reported).resolve()))
        if label is None:
            continue
        for alert in alerts:
            span = alert.get("Span", [1])
            column = span[0] if isinstance(span, list) and span else 1
            findings.append(
                (label, int(alert["Line"]), column, alert["Check"], alert["Message"])
            )
    for label, line, column, check, message in sorted(findings):
        print(f"{label}:{line}:{column}: {check}: {message}")
    return 1 if findings else 0


def lint_stdin(vale: str, label: str) -> int:
    """Lint the markdown text on standard input. Report every finding."""
    text = sys.stdin.read()
    if not text.strip():
        print(f"vale-lint: {label} is empty", file=sys.stderr)
        return 1
    status = lint_texts(vale, {label: text})
    if status == 0:
        print(f"vale-lint: {label} is clean", file=sys.stderr)
    return status


def commit_message(commit: str) -> str:
    """Return the prose of a commit message, without its trailers.

    A trailer names a person or a session. It is a record rather than prose,
    and its long lines are not sentences, so the style does not apply to it.
    """
    message = git("show", "--no-patch", "--format=%B", commit)
    trailers = git("show", "--no-patch", "--format=%(trailers)", commit).strip()
    if trailers and message.rstrip().endswith(trailers):
        message = message.rstrip()[: -len(trailers)]
    return message.strip()


def lint_commits(vale: str, base: str) -> int:
    """Lint the message of each commit that this branch adds to its base.

    A merge commit carries a message that the forge writes, so it is skipped.
    A commit whose own tree holds no `.vale.ini` was written before this style
    existed, so the style does not apply to it either. That rule needs no list
    of commits to grandfather, and it covers every commit written from here on.
    """
    commits = git("rev-list", "--no-merges", "--reverse", f"{base}..HEAD").split()
    texts: dict[str, str] = {}
    skipped = 0
    for commit in commits:
        known = subprocess.run(
            ["git", "cat-file", "-e", f"{commit}:.vale.ini"],
            cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        if known.returncode != 0:
            skipped += 1
            continue
        message = commit_message(commit)
        if not message:
            raise SystemExit(f"vale-lint: commit {commit[:8]} has an empty message")
        texts[f"commit {commit[:8]}"] = message
    if not texts:
        print(
            f"vale-lint: no commit message to lint ({skipped} predate the style)",
            file=sys.stderr,
        )
        return 0
    status = lint_texts(vale, texts)
    if status == 0:
        print(f"vale-lint: {len(texts)} commit message(s) are clean", file=sys.stderr)
    return status


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="branch or commit to compare with HEAD")
    parser.add_argument("--self-test", action="store_true", help="test the configured rules")
    parser.add_argument(
        "--stdin",
        metavar="LABEL",
        help="lint the markdown on standard input and name it LABEL",
    )
    parser.add_argument(
        "--commits",
        action="store_true",
        help="lint the message of each commit this branch adds to its base",
    )
    args = parser.parse_args()
    vale = shutil.which("vale")
    if vale is None:
        print("vale-lint: vale is not installed", file=sys.stderr)
        return 2
    if args.self_test:
        return self_test(vale)
    if args.stdin:
        return lint_stdin(vale, args.stdin)
    if args.commits:
        return lint_commits(vale, base_commit(args.base))
    findings = run_vale(source_files(), vale)
    for path, alert in sorted(
        findings, key=lambda f: (f[0], int(f[1]["Line"]), str(f[1]["Check"]))
    ):
        span = alert.get("Span", [1])
        column = span[0] if isinstance(span, list) and span else 1
        print(f"{path}:{alert['Line']}:{column}: {alert['Check']}: {alert['Message']}")
    if findings:
        print(f"vale-lint: {len(findings)} finding(s)", file=sys.stderr)
        return 1
    print("vale-lint: clean", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
