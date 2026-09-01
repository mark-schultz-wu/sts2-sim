#!/usr/bin/env bash
# Move the pinned game between this machine and the forge.
#
# Everything here compiles or runs against Slay the Spire 2, and the copy Steam
# manages is the wrong thing to point at: it is world-writable, it updates when
# it likes, and STS2 churns 20-50 cards on a minor version. A run whose result
# changed because Steam patched overnight is not a failing test, it is a lie.
#
# So one build lives in a private generic package on the forge, and mod/game.lock
# pins it by hash. The package holds two things:
#
#   the three assemblies the mods compile against    17M, fetched by every CI run
#   the whole install as one tarball                 2.0G, fetched once per machine
#
# Both carry the same version, so the thing we compile against and the thing we
# run can never drift apart.
#
# Usage:
#     mod/game.sh fetch            # the three assemblies, into the pin directory
#     mod/game.sh fetch-game       # the whole install, unpacked
#     mod/game.sh publish          # from a machine with the game installed
#
# All three need a forge token with package scope, from STS2_FORGE_TOKEN or from
# the token file named below.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
LOCK="$HERE/game.lock"

FORGE="${STS2_FORGE_URL:-https://code.schultz-wu.com}"
OWNER="${STS2_FORGE_OWNER:-mark}"

# Under XDG_DATA_HOME, beside the game's own profile directory. It is derived
# data -- refetchable, and large -- so it does not belong next to source, and
# it is not a cache either: losing it means a 1.2G download, not a rebuild.
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}/sts2"
PIN_DIR="${STS2_PIN_DIR:-$DATA_HOME/pin}"
VENDORED="${STS2_VENDORED_GAME:-$DATA_HOME/game}"

# The lock is a sha256sum manifest, so `sha256sum -c` reads it directly and the
# package coordinates ride along in comments it ignores.
PACKAGE="$(sed -n 's/^# package: //p' "$LOCK")"
VERSION="$(sed -n 's/^# version: //p' "$LOCK")"
BASE="$FORGE/api/packages/$OWNER/generic/$PACKAGE/$VERSION"

ASSEMBLIES=(0Harmony.dll GodotSharp.dll sts2.dll)
ARCHIVE=sts2-game.tar.zst

# Staging lives at file scope, not inside the function that fills it: an EXIT
# trap runs after the function has returned, so a `local` it names is already
# out of scope and `set -u` turns the cleanup into the error it reports.
STAGED=""
trap 'if [ -n "$STAGED" ]; then rm -rf "$STAGED"; fi' EXIT

# Steam's copy is the source for `publish` and for nothing else.
STEAM_DIR="${STS2_STEAM_DIR:-/etc/wolf/profile-data/user/WolfSteam/.steam/steam/steamapps/common/Slay the Spire 2}"

# Ours, not Steam's: build output we install, and an empty directory left behind
# by an older workflow. Neither belongs in a pin of what the game shipped.
EXCLUDE=(--exclude=./mods --exclude=./crates)

# Where the token lives when it is not in the environment. Re-pinning is rare
# enough that nobody remembers the token, and often enough that pasting it onto
# a command line each time is how it ends up in a shell history.
TOKEN_FILE="${STS2_FORGE_TOKEN_FILE:-${XDG_CONFIG_HOME:-$HOME/.config}/sts2/token}"

# Resolved once, up front, and never inside a command substitution: `${x:?}`
# there exits only the subshell, so curl would go on to send an empty header
# and report someone else's 401 instead of this message.
require_token() {
  [ -n "${STS2_FORGE_TOKEN:-}" ] && return

  if [ -f "$TOKEN_FILE" ]; then
    # Refused when anyone else can read it, for the same reason ssh refuses a
    # world-readable private key.
    local mode; mode="$(stat -c '%a' "$TOKEN_FILE")"
    if [ "${mode: -2}" != "00" ]; then
      echo "mod/game.sh: $TOKEN_FILE is readable by others (mode $mode)." >&2
      echo "Run: chmod 600 $TOKEN_FILE" >&2
      exit 1
    fi
    STS2_FORGE_TOKEN="$(tr -d '[:space:]' < "$TOKEN_FILE")"
    [ -n "$STS2_FORGE_TOKEN" ] && return
    echo "mod/game.sh: $TOKEN_FILE is empty." >&2
    exit 1
  fi

  echo "mod/game.sh: no forge token." >&2
  echo "Either set STS2_FORGE_TOKEN, or write one to $TOKEN_FILE (chmod 600)." >&2
  echo "Make it under $FORGE/user/settings/applications with package scope," >&2
  echo "leaving the repository selector alone: package scopes are account-wide," >&2
  echo "and Forgejo drops them from a token restricted to repositories." >&2
  exit 1
}

# One line per name, in a form `sha256sum -c` accepts.
manifest_for() {
  local name
  for name in "$@"; do grep -E "[[:space:]]$name\$" "$LOCK"; done
}

download() {
  curl --fail --silent --show-error --location \
    --header "Authorization: token $STS2_FORGE_TOKEN" --output "$2" "$BASE/$1"
}

fetch() {
  mkdir -p "$PIN_DIR"
  local sums; sums="$(manifest_for "${ASSEMBLIES[@]}")"

  # Verify before spending the network: on a machine that already has them this
  # is the whole recipe.
  if (cd "$PIN_DIR" && echo "$sums" | sha256sum -c --quiet - >/dev/null 2>&1); then
    echo "assemblies already pinned at $VERSION in $PIN_DIR"
    return
  fi

  require_token
  local f
  for f in "${ASSEMBLIES[@]}"; do
    echo "fetching $f"
    download "$f" "$PIN_DIR/$f"
  done

  # The check that matters. The one above only decided whether to download.
  (cd "$PIN_DIR" && echo "$sums" | sha256sum -c --quiet -)
  echo "assemblies pinned at $VERSION in $PIN_DIR"
}

fetch_game() {
  # A 2.0G tarball is not re-verified on every call, so the unpacked tree keeps
  # a stamp saying which version produced it. The hash below is still checked
  # on the way in; the stamp only answers "do I need to do this again".
  if [ "$(cat "$VENDORED/.pinned-version" 2>/dev/null)" = "$VERSION" ]; then
    echo "game already unpacked at $VERSION in $VENDORED"
    return
  fi

  require_token
  STAGED="$(mktemp -d)"

  echo "fetching $ARCHIVE (1.2G)"
  download "$ARCHIVE" "$STAGED/$ARCHIVE"
  (cd "$STAGED" && manifest_for "$ARCHIVE" | sha256sum -c --quiet -)

  rm -rf "$VENDORED"
  mkdir -p "$VENDORED"
  tar --extract --zstd --file "$STAGED/$ARCHIVE" --directory "$VENDORED"
  echo "$VERSION" > "$VENDORED/.pinned-version"
  echo "game unpacked at $VERSION in $VENDORED"
}

archive_into() {
  # Sorted names, flattened timestamps and owners, single-threaded zstd: the
  # same install has to produce the same bytes, or re-publishing a build would
  # invalidate a lock that nothing actually changed.
  tar --create --directory "$STEAM_DIR" "${EXCLUDE[@]}" \
      --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner . \
    | zstd -3 -T1 -q -o "$1"
}

publish() {
  require_token
  STAGED="$(mktemp -d)"

  local f
  for f in "${ASSEMBLIES[@]}"; do
    cp "$STEAM_DIR/data_sts2_linuxbsd_x86_64/$f" "$STAGED/$f"
  done

  echo "packing $STEAM_DIR"
  archive_into "$STAGED/$ARCHIVE"

  for f in "${ASSEMBLIES[@]}" "$ARCHIVE"; do
    echo "publishing $f"
    curl --fail --silent --show-error --request PUT \
      --header "Authorization: token $STS2_FORGE_TOKEN" \
      --upload-file "$STAGED/$f" "$BASE/$f"
  done

  echo
  echo "published $PACKAGE $VERSION. Its hashes, for mod/game.lock:"
  (cd "$STAGED" && sha256sum "${ASSEMBLIES[@]}" "$ARCHIVE")
}

case "${1:-}" in
  fetch) fetch ;;
  fetch-game) fetch_game ;;
  publish) publish ;;
  *)
    cat >&2 <<'USAGE'
Usage:
    mod/game.sh fetch            # the three assemblies, into the pin directory
    mod/game.sh fetch-game       # the whole install, unpacked
    mod/game.sh publish          # from a machine with the game installed

All three need a forge token with package scope, in STS2_FORGE_TOKEN or in
${XDG_CONFIG_HOME:-~/.config}/sts2/token (chmod 600).
USAGE
    exit 2 ;;
esac
