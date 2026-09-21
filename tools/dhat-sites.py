#!/usr/bin/env python3
"""Summarise a dhat profile as allocations per source site.

dhat records every allocation with the stack that made it. This rolls those
up by the first frame inside this workspace, which is the line that actually
asked for the memory, and reports how many of them each line made.

    cargo build --release -p sts2-golden --example clone_sites
    valgrind --tool=dhat --fullpath-after= --dhat-out-file=dhat.json \\
        ./target/release/examples/clone_sites
    ./tools/dhat-sites.py dhat.json --divide-by 200

`--fullpath-after=` with nothing after it makes valgrind keep each frame's
whole source path. Without it a frame names only the file, and this cannot
tell the workspace's `mod.rs` from the standard library's.

`--divide-by` is the number of times the thing under test ran, so the counts
read as allocations per branch rather than per profile.
"""

import argparse
import json
import re
from collections import Counter

# A dhat frame looks like:
#   0x1234: alloc::vec::Vec<T>::clone (vec.rs:2876)
FRAME = re.compile(r"^0x[0-9a-fA-F]+: (.*) \((.*)\)$")

# Where this checkout's crates live, as a whole path names them.
WORKSPACE = "/crates/"

# One innermost `<...>`, removed until none are left.
GENERICS = re.compile(r"<[^<>]*>")


def interesting(frame):
    """A frame's name and location, if its source is in this workspace.

    An optimised build inlines the call site into the allocator frames above
    it, and each inlined frame keeps its own file and line. So the frame worth
    reporting is the innermost one whose file is ours: `alloc` and `smallvec`
    frames name our types in their generics, but not the line that asked.
    """
    hit = FRAME.match(frame)
    if not hit:
        return None
    name, where = hit.groups()
    if WORKSPACE not in where:
        return None
    return f"{short(name)}  ({where.split(WORKSPACE, 1)[1]})"


def short(name):
    """A function's name without the generic arguments that bury it."""
    while (tidied := GENERICS.sub("", name)) != name:
        name = tidied
    return name


def by_name(frame):
    """The old attribution, for a profile recorded without whole paths."""
    hit = FRAME.match(frame)
    if not hit or "sts2" not in hit.group(1):
        return None
    return f"{hit.group(1)}  ({hit.group(2)})"


def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("profile", help="the --dhat-out-file that valgrind wrote")
    ap.add_argument(
        "--divide-by",
        type=float,
        default=1.0,
        help="iterations in the profiled loop, so counts read per iteration",
    )
    ap.add_argument("--top", type=int, default=25, help="how many sites to print")
    args = ap.parse_args()

    with open(args.profile) as handle:
        profile = json.load(handle)

    frames = profile["ftbl"]
    whole_paths = any(WORKSPACE in frame for frame in frames)
    site_of = interesting if whole_paths else by_name
    if not whole_paths:
        print("no whole paths in this profile; record it with --fullpath-after=")
    calls, bytes_at = Counter(), Counter()
    for record in profile["pps"]:
        site = next(
            (
                found
                for frame in record["fs"]
                if (found := site_of(frames[frame]))
            ),
            "(no workspace frame)",
        )
        calls[site] += record["tbk"]
        bytes_at[site] += record["tb"]

    scale = args.divide_by
    print(f"\n{'allocs':>8} {'bytes':>9}  site\n")
    for site, count in calls.most_common(args.top):
        print(f"{count / scale:8.2f} {bytes_at[site] / scale:9.0f}  {site}")
    print(f"\n{sum(calls.values()) / scale:8.2f} {sum(bytes_at.values()) / scale:9.0f}  total")


if __name__ == "__main__":
    main()
