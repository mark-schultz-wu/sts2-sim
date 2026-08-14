#!/usr/bin/env bash
# Build every mod here and install them into the game's mods directory.
#
# The game loads mods from <gamedir>/mods/<dir>/, expecting a *.json manifest
# and an assembly named exactly <manifest id>.dll. Load order comes from the
# `dependencies` field in each manifest — the loader topologically sorts on it,
# and refuses to load a mod whose dependencies are absent, naming them. So the
# order here does not matter; only that all of them arrive.
set -euo pipefail

# Dependencies first, so a build failure in one stops before anything installs
# a half-updated set.
MODS=(sts2_determinism sts2_headless sts2_headless_test)

GAME_DIR="${STS2_GAME_DIR:?is not set. The repo .envrc exports it: run 'direnv allow' in the repo root, or set it by hand}"
PROFILE_DIR="${STS2_PROFILE_DIR:-$HOME/.local/share/SlayTheSpire2/default/1}"
HERE="$(cd "$(dirname "$0")" && pwd)"

export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
export PATH="$DOTNET_ROOT:$PATH"

for mod in "${MODS[@]}"; do
  dotnet build -c Release -v q --nologo "$HERE/$mod/$mod.csproj"
done

for mod in "${MODS[@]}"; do
  DEST="$GAME_DIR/mods/$mod"
  mkdir -p "$DEST"
  cp "$HERE/$mod/bin/Release/net9.0/$mod.dll" "$DEST/"
  cp "$HERE/$mod/manifest.json" "$DEST/"
  echo "installed -> $DEST"
done

# Mods stay Disabled until the player has accepted the mod warning. Headless
# never shows that popup, so set the flag the popup would have set.
SETTINGS="$PROFILE_DIR/settings.save"
if [ -f "$SETTINGS" ]; then
  [ -f "$SETTINGS.pre-oracle" ] || cp "$SETTINGS" "$SETTINGS.pre-oracle"
  python3 - "$SETTINGS" <<'PY'
import json, sys
p = sys.argv[1]
d = json.load(open(p))
ms = d.get("mod_settings") or {}
if ms.get("mods_enabled") is True:
    print("mods already enabled")
else:
    ms["mods_enabled"] = True
    d["mod_settings"] = ms
    json.dump(d, open(p, "w"), indent=2)
    print("enabled mods in", p)
PY
else
  echo "note: no settings.save at $SETTINGS yet; run the game once, then re-run this" >&2
fi
