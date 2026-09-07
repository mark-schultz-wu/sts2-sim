#!/usr/bin/env bash
# Build every mod here and install them into the game's mods directory.
#
# The game loads mods from <gamedir>/mods/<dir>/, expecting a *.json manifest
# and an assembly named exactly <manifest id>.dll. Load order comes from the
# `dependencies` field in each manifest. The loader topologically sorts on it,
# and refuses to load a mod whose dependencies are absent, naming them. So the
# order here does not matter. Only their arrival matters.
set -euo pipefail

GAME_DIR="${STS2_GAME_DIR:?is not set. The repo .envrc exports it: run 'direnv allow' in the repo root, or set it by hand}"
PROFILE_DIR="${STS2_PROFILE_DIR:-$HOME/.local/share/SlayTheSpire2/default/1}"
HERE="$(cd "$(dirname "$0")" && pwd)"

# All of them, in dependency order, or none: build.sh stops on the first
# failure so nothing below installs a half-updated set.
"$HERE/build.sh"

# A manifest is what makes a directory a mod, so the glob is the list. It needs
# no order, and it cannot fall behind the one build.sh keeps.
for manifest in "$HERE"/*/manifest.json; do
  mod="$(basename "$(dirname "$manifest")")"
  DEST="$GAME_DIR/mods/$mod"
  mkdir -p "$DEST"
  cp "$HERE/$mod/bin/Release/net9.0/$mod.dll" "$DEST/"
  cp "$manifest" "$DEST/"
  echo "installed -> $DEST"
done

# Mods stay Disabled until the player accepts the mod warning. Headless never
# shows that popup, so set the flag that the popup sets.
SETTINGS="$PROFILE_DIR/settings.save"
mkdir -p "$PROFILE_DIR"
[ ! -f "$SETTINGS" ] || [ -f "$SETTINGS.pre-oracle" ] \
  || cp "$SETTINGS" "$SETTINGS.pre-oracle"
python3 - "$SETTINGS" "$HERE" <<'PY'
import glob
import json
import os
import sys

path, mod_dir = sys.argv[1:]
if os.path.exists(path):
    with open(path) as f:
        settings = json.load(f)
else:
    # SettingsSave currently uses schema 8. Without the schema marker the game
    # treats this as a legacy or invalid save. It then rewrites the save with
    # defaults, and reports that nobody acknowledged the mod warning.
    settings = {"schema_version": 8}

mod_settings = settings.get("mod_settings") or {}
mods = {
    entry["id"]: entry
    for entry in mod_settings.get("mod_list") or []
    if entry.get("id")
}
for manifest_path in glob.glob(os.path.join(mod_dir, "*", "manifest.json")):
    with open(manifest_path) as f:
        mod_id = json.load(f)["id"]
    mods[mod_id] = {
        "id": mod_id,
        "is_enabled": True,
        "source": "mods_directory",
    }

mod_settings["mod_list"] = list(mods.values())
mod_settings["mods_enabled"] = True
settings["mod_settings"] = mod_settings
with open(path, "w") as f:
    json.dump(settings, f, indent=2)
    f.write("\n")
print("enabled mods in", path)
PY
