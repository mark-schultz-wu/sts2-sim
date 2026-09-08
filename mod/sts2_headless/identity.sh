#!/bin/sh
# The mod's identity: a hash of its own source.
#
# The greeting carries this, and the Rust client refuses a mismatch. So "I
# rebuilt the client but forgot to redeploy the mod" fails at the handshake in
# two seconds. Otherwise it fails as a decode error four hundred decisions into
# a fuzz run. The game's build stamp cannot catch that, because a redeployed
# mod does not move the game's assembly or content hash.
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
# with its contents intact is still a different mod.
LC_ALL=C ls *.cs | LC_ALL=C sort | tr '\n' '\0' | LC_ALL=C xargs -0 sha256sum \
  | sha256sum | cut -c1-16
