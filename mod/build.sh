#!/usr/bin/env bash
# Build every mod here, in dependency order.
#
# Split out of deploy.sh because CI builds the mods without a game to install
# them into, and a second copy of the list is a second thing to forget when a
# mod is added.
#
# The assemblies the mods compile against are not in the repository. Fetch them
# first with `mod/game.sh fetch`, which this does not do for you: on a machine
# with the vendored game they are already there, and a build recipe that reaches
# for the network on every run is a build recipe people stop trusting.
set -euo pipefail

# Dependencies first, so a build failure in one stops before a caller installs
# a half-updated set.
MODS=(sts2_determinism sts2_headless sts2_headless_test sts2_determinism_test sts2_dump)

HERE="$(cd "$(dirname "$0")" && pwd)"

export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
export PATH="$DOTNET_ROOT:$PATH"
# MSBuild imports environment variables as properties. Bridge the shell-facing
# spelling used by .envrc to the property Directory.Build.props consumes.
export Sts2GameVersion="${Sts2GameVersion:-${STS2_GAME_VERSION:-v0.107.1}}"
LOCK="$HERE/games/$Sts2GameVersion.lock"
if [ ! -f "$LOCK" ]; then
  echo "mod/build.sh: unknown game version '$Sts2GameVersion' (no $LOCK)." >&2
  exit 2
fi
# The ABI family names one coherent shape of MegaCrit's assembly, and each mod
# compiles the one file under its Compat/ that carries that name. Two builds in
# a family present the same surface to us; they are not the same game.
#
# In particular a family says nothing about the numbers. v0.107.1 replaced the
# game's PRNG with xoshiro256** and every build before it drew from C#
# System.Random, yet the two serialize an Rng the same way, so both answer to
# `counter-rng`. A family is what the mods compile against, never evidence that
# two builds agree on a seed.
export Sts2GameAbi="${Sts2GameAbi:-$(sed -n 's/^# abi: //p' "$LOCK")}"
if [ -z "$Sts2GameAbi" ]; then
  echo "mod/build.sh: $LOCK does not name an ABI family." >&2
  exit 2
fi
# Compiled into sts2_determinism, which refuses to start against any other
# build. Taken from the lock rather than written in the source, so that adding a
# build is adding a lock. A literal here was a constant validating itself.
export Sts2AssemblyHash="${Sts2AssemblyHash:-$(sed -n 's/^# assembly: //p' "$LOCK")}"
if [ -z "$Sts2AssemblyHash" ]; then
  echo "mod/build.sh: $LOCK does not name an assembly hash." >&2
  exit 2
fi

for mod in "${MODS[@]}"; do
  dotnet build -c Release -v q --nologo "$@" "$HERE/$mod/$mod.csproj"
done
