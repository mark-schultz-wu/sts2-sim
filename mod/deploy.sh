#!/usr/bin/env bash
# Build sts_oracle and install it into the game's mods directory.
#
# The game loads mods from <gamedir>/mods/<dir>/, expecting a *.json manifest
# and an assembly named exactly <manifest id>.dll.
set -euo pipefail

GAME_DIR="${STS2_GAME_DIR:-/etc/wolf/profile-data/user/WolfSteam/.steam/steam/steamapps/common/Slay the Spire 2}"
PROFILE_DIR="${STS2_PROFILE_DIR:-$HOME/.local/share/SlayTheSpire2/default/1}"
HERE="$(cd "$(dirname "$0")" && pwd)"

export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
export PATH="$DOTNET_ROOT:$PATH"

dotnet build -c Release -v q --nologo "$HERE/sts_oracle/sts_oracle.csproj"

DEST="$GAME_DIR/mods/sts_oracle"
mkdir -p "$DEST"
cp "$HERE/sts_oracle/bin/Release/net9.0/sts_oracle.dll" "$DEST/"
cp "$HERE/sts_oracle/manifest.json" "$DEST/"
echo "installed -> $DEST"

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
