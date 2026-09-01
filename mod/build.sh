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

for mod in "${MODS[@]}"; do
  dotnet build -c Release -v q --nologo "$@" "$HERE/$mod/$mod.csproj"
done
