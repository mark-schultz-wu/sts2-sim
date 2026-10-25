#!/bin/sh
# The oracle's identity: a hash of the source of every mod it loads.
#
# The greeting carries this, and the Rust client refuses a mismatch. So "I
# rebuilt the client but forgot to redeploy the mods" fails at the handshake in
# two seconds. Otherwise it fails as a decode error four hundred decisions into
# a fuzz run. The game's build stamp cannot catch that, because a redeployed
# mod does not move the game's assembly or content hash.
#
# Every mod that can change what a run does is in it: the headless driver, the
# setup commands, the fixes, reproducibility, co-op, and the tracing. Each build's
# Compat files are in it too, and each mod's manifest, which says what loads
# after what.
#
# One script, rather than one hasher in MSBuild and a second in the Rust build
# script. Two implementations of "the same hash" are two things that can
# disagree, and the symptom of a disagreement is that every mod looks stale.
#
# Deliberately over-strict: any source edit invalidates a deployed build, even a
# comment. To narrow it to the files that shape the wire, we must keep a list
# of them, and that list is the part that rots.
set -eu
cd "$(dirname "$0")"

# LC_ALL=C so the sort is by byte and not by whatever locale the caller has.
# `sha256sum` prints "<hash>  <name>", so names are covered too: a file renamed
# with its contents intact is still a different mod. Build output is left out.
LC_ALL=C find sts2_determinism sts2_fixes sts2_net sts2_headless sts2_setup sts2_observe \
  \( -path '*/bin' -o -path '*/obj' \) -prune -o \( -name '*.cs' -o -name manifest.json \) -print \
  | LC_ALL=C sort | tr '\n' '\0' | LC_ALL=C xargs -0 sha256sum | sha256sum | cut -c1-16
