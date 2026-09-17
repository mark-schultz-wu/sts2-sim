#!/usr/bin/env python3
"""Summarise a dhat profile as allocations per source site.

dhat records every allocation with the stack that made it. This rolls those
up by the first frame inside this workspace, which is the line that actually
asked for the memory, and reports how many of them each line made.

    cargo build --release -p sts2-golden --example clone_sites
    valgrind --tool=dhat --dhat-out-file=dhat.json \\
        ./target/release/examples/clone_sites
    ./tools/dhat-sites.py dhat.json --divide-by 200

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


def interesting(frame):
    """A frame's name and location, unless it names nothing of this workspace.

    The allocator frames above a call site are all `alloc::` and `core::`, and
    an optimised build inlines the call site into them. So the frame worth
    reporting is the innermost one that names a type or function of ours,
    whichever crate spelled it.
    """
    hit = FRAME.match(frame)
    if not hit:
        return None
    name, where = hit.groups()
    if "sts2" not in name:
        return None
    return f"{name}  ({where})"


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
    calls, bytes_at = Counter(), Counter()
    for record in profile["pps"]:
        site = next(
            (
                found
                for frame in record["fs"]
                if (found := interesting(frames[frame]))
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
