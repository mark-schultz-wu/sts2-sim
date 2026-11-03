#!/usr/bin/env bash
# Move the pinned game between this machine and the forge.
#
# Everything here compiles or runs against Slay the Spire 2. Do not point at
# the copy Steam manages. That copy is world-writable, it updates when it
# likes, and STS2 churns 20-50 cards on a minor version. A run whose result
# changed because Steam patched overnight is not a failing test, it is a lie.
#
# So each build lives as one version of a private generic package on the forge,
# and a lock under mod/games/ pins it by hash. The package holds two things:
#
#   the three assemblies the mods compile against    17M, fetched by every CI run
#   the whole install as one tarball                 2.0G, fetched once per machine
#
# Both carry the same version, so the thing we compile against and the thing we
# run can never drift apart.
#
# Without access to that forge, bring your own copy. Anyone who owns the game
# can download the exact build from Steam with `acquire`, which knows each
# build's manifest. `install` then checks the copy against the
# lock and puts it where the build and the tests look.
#
# Usage:
#     mod/game.sh acquire v0.107.1
#     mod/game.sh install v0.107.1
#     mod/game.sh fetch v0.107.1
#     mod/game.sh fetch-game v0.111.0
#     mod/game.sh publish v0.111.0 # source from STS2_STEAM_DIR
#
# Forge operations need a package token. Acquisition asks Steam interactively
# and stores only Steam's refresh token, through DepotDownloader itself.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
COMMAND="${1:-}"
GAME="${2:-${STS2_GAME_VERSION:-v0.107.1}}"
LOCK="$HERE/games/$GAME.lock"

if [ "$COMMAND" != acquire ] && [ ! -f "$LOCK" ]; then
  echo "mod/game.sh: unknown game version '$GAME' (no $LOCK)." >&2
  exit 2
fi

FORGE="${STS2_FORGE_URL:-https://code.schultz-wu.com}"
OWNER="${STS2_FORGE_OWNER:-mark}"

# Under XDG_DATA_HOME, beside the game's own profile directory. It holds
# derived data, which the machine can fetch again, and which is large. So it
# does not belong next to source. It is not a cache either, because a loss
# means a 1.2G download rather than a rebuild.
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}/sts2"
PIN_DIR="${STS2_PIN_DIR:-$DATA_HOME/pins/$GAME}"
VENDORED="${STS2_VENDORED_GAME:-$DATA_HOME/games/$GAME}"
ACQUIRED="${STS2_STEAM_DIR:-$DATA_HOME/steam/$GAME}"

# The lock is a sha256sum manifest, so `sha256sum -c` reads it directly and the
# package coordinates ride along in comments it ignores.
PACKAGE="$(sed -n 's/^# package: //p' "$LOCK" 2>/dev/null || true)"

# Each build's manifest in Steam's Linux depot, for `acquire`. It lives here
# and not in the lock, because the CI runner's image hashes every lock.
case "$GAME" in
  v0.107.1) MANIFEST=7881097836989620034 ;;
  v0.111.0) MANIFEST=1506251310063253032 ;;
  *) MANIFEST="" ;;
esac
VERSION="$(sed -n 's/^# version: //p' "$LOCK" 2>/dev/null || true)"
BASE="$FORGE/api/packages/$OWNER/generic/$PACKAGE/$VERSION"

ASSEMBLIES=(0Harmony.dll GodotSharp.dll sts2.dll)
ARCHIVE=sts2-game.tar.zst

# Staging lives at file scope rather than inside the function that fills it.
# An EXIT trap runs after the function returns, so a `local` that it names
# already went out of scope. Then `set -u` turns the cleanup into the error
# that it reports.
STAGED=""
trap 'if [ -n "$STAGED" ]; then rm -rf "$STAGED"; fi' EXIT

# Steam's copy is the source for `publish` and for nothing else.
STEAM_DIR="${STS2_STEAM_DIR:-/etc/wolf/profile-data/user/WolfSteam/.steam/steam/steamapps/common/Slay the Spire 2}"

# Ours, not Steam's: build output we install, and an empty directory left behind
# by an older workflow. Neither belongs in a pin of what the game shipped.
EXCLUDE=(--exclude=./mods --exclude=./crates)

# Where the token lives when it is not in the environment. Re-pinning is rare
# enough that nobody remembers the token. It is also frequent enough that a
# token pasted onto the command line each time ends up in a shell history.
TOKEN_FILE="${STS2_FORGE_TOKEN_FILE:-${XDG_CONFIG_HOME:-$HOME/.config}/sts2/token}"

acquire() {
  local manifest="${3:-$MANIFEST}"
  local branch="${4:-public}"
  if [ -z "$manifest" ] || [ -z "${STS2_STEAM_USERNAME:-}" ]; then
    echo "Usage: STS2_STEAM_USERNAME=name mod/game.sh acquire <version> [manifest] [branch]" >&2
    exit 2
  fi

  local downloader="${DEPOT_DOWNLOADER:-DepotDownloader}"
  mkdir -p "$ACQUIRED"
  "$downloader" \
    -app 2868840 \
    -depot 2868843 \
    -branch "$branch" \
    -manifest "$manifest" \
    -dir "$ACQUIRED" \
    -username "$STS2_STEAM_USERNAME" \
    -remember-password

  make_runnable "$ACQUIRED"
}

# Steam ships this depot with almost every file 0755. DepotDownloader writes
# 0644 for all of them, so an install it produced cannot start: the game exits
# 126 and env reports "Permission denied" on ./SlayTheSpire2.
#
# Restored here, at the one place that loses it, so the archive `publish` packs
# is a faithful copy of an install. A repair after unpacking leaves the
# published bytes wrong, and hides that from whoever fetched them.
#
# ELF magic and a shebang, rather than a list of names. The set of programs in
# the install is the game's business, and a list is one more thing to keep in
# step.
make_runnable() {
  local fixed=0 f
  while IFS= read -r -d '' f; do
    case "$(head -c 4 "$f" | od -An -tx1 | tr -d ' \n')" in
      7f454c46|2321*) chmod +x "$f"; fixed=$((fixed + 1)) ;;
    esac
  done < <(find "$1" -type f ! -perm -u+x -print0)
  echo "made $fixed file(s) executable in $1"
}

# Resolved once, up front, and never inside a command substitution. There
# `${x:?}` exits only the subshell, so curl goes on to send an empty header
# and reports someone else's 401 instead of this message.
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
  # The one fetch CI still makes over the network, so it retries.
  # --retry-all-errors because the default --retry covers a transient HTTP
  # status and not a connection that never opened, which is what we saw.
  curl --fail --silent --show-error --location \
    --retry 8 --retry-all-errors --retry-delay 2 \
    --connect-timeout 15 \
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
  # a stamp saying which version produced it. The code below still checks the
  # hash on the way in. The stamp only answers "do I need to do this again".
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

# A copy that did not come from the forge: one `acquire` downloaded, or one
# the user brings. Its assemblies are checked against the lock, which is what
# makes it the pinned build. This does not check the archive's hash, because
# only a publish packs one.
install() {
  local source="${3:-$ACQUIRED}"
  local sums; sums="$(manifest_for "${ASSEMBLIES[@]}")"
  local data="$source/data_sts2_linuxbsd_x86_64"

  if ! (cd "$data" 2>/dev/null && echo "$sums" | sha256sum -c --quiet -); then
    echo "mod/game.sh: $source is not the $GAME pinned by $LOCK." >&2
    echo "Its assemblies in $data do not match the lock." >&2
    exit 1
  fi

  make_runnable "$source"
  mkdir -p "$PIN_DIR"
  local f
  for f in "${ASSEMBLIES[@]}"; do cp "$data/$f" "$PIN_DIR/$f"; done

  if [ "$(realpath "$source")" != "$(realpath -m "$VENDORED")" ]; then
    rm -rf "$VENDORED"
    mkdir -p "$(dirname "$VENDORED")"
    cp -a --reflink=auto "$source" "$VENDORED"
  fi
  echo "$VERSION" > "$VENDORED/.pinned-version"
  echo "game installed at $VERSION in $VENDORED"
}

archive_into() {
  # Sorted names, flattened timestamps and owners, single-threaded zstd. The
  # same install has to produce the same bytes. Otherwise a second publish of a
  # build invalidates a lock that nothing actually changed.
  tar --create --directory "$STEAM_DIR" "${EXCLUDE[@]}" \
      --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner . \
    | zstd -3 -T1 -q -o "$1"
}

publish() {
  require_token

  # The archive has to be a faithful copy of an install. An install whose
  # executable is not executable does not start: the game exits 126 and env
  # reports "Permission denied". DepotDownloader writes 0644 for every file it
  # fetches, which is how one publish went out that way. `acquire` restores the
  # bit now, and this code refuses a source that still lacks it. The archive is
  # the last place the mistake is cheap to fix.
  if [ ! -x "$STEAM_DIR/SlayTheSpire2" ]; then
    echo "mod/game.sh: $STEAM_DIR/SlayTheSpire2 is not executable." >&2
    echo "An install packed from here cannot start. Re-acquire it, or restore" >&2
    echo "the mode on the ELF files and shell scripts before publishing." >&2
    exit 1
  fi

  STAGED="$(mktemp -d)"

  local f
  for f in "${ASSEMBLIES[@]}"; do
    cp "$STEAM_DIR/data_sts2_linuxbsd_x86_64/$f" "$STAGED/$f"
  done

  echo "packing $STEAM_DIR"
  archive_into "$STAGED/$ARCHIVE"

  # Forgejo answers 409 rather than overwriting a file a version already has.
  # That is the right default. A pin has to stay the bytes that it was, and a
  # publish that quietly replaces one invalidates every lock that names it.
  # A replacement corrects an artifact that was wrong at the start, which is
  # rare enough to ask for by name.
  local f status
  for f in "${ASSEMBLIES[@]}" "$ARCHIVE"; do
    if [ -n "${STS2_FORGE_REPLACE:-}" ]; then
      echo "removing published $f"
      curl --fail --silent --show-error --request DELETE \
        --header "Authorization: token $STS2_FORGE_TOKEN" "$BASE/$f" || true
    fi

    echo "publishing $f"
    status="$(curl --silent --show-error --output /dev/stderr --write-out '%{http_code}' \
      --request PUT --header "Authorization: token $STS2_FORGE_TOKEN" \
      --upload-file "$STAGED/$f" "$BASE/$f")"

    case "$status" in
      2*) ;;
      409)
        echo >&2
        echo "mod/game.sh: $PACKAGE $VERSION already has $f." >&2
        echo "A published pin is not overwritten by accident. To correct one:" >&2
        echo "    STS2_FORGE_REPLACE=1 mod/game.sh publish $GAME" >&2
        echo "Then put the hashes it prints in $LOCK." >&2
        exit 1
        ;;
      *)
        echo "mod/game.sh: publishing $f answered $status." >&2
        exit 1
        ;;
    esac
  done

  echo
  echo "published $PACKAGE $VERSION. Its hashes, for $LOCK:"
  (cd "$STAGED" && sha256sum "${ASSEMBLIES[@]}" "$ARCHIVE")
}

case "$COMMAND" in
  acquire) acquire "$@" ;;
  fetch) fetch ;;
  fetch-game) fetch_game ;;
  install) install "$@" ;;
  publish) publish ;;
  *)
    cat >&2 <<'USAGE'
Usage:
    mod/game.sh acquire <version> [manifest] [branch]
    mod/game.sh install <version> [dir]
    mod/game.sh fetch [version]
    mod/game.sh fetch-game [version]
    mod/game.sh publish [version]

Versions are lock names under mod/games/; the default is v0.107.1.
Publish reads its source from STS2_STEAM_DIR.

Fetch, fetch-game and publish need a forge token with package scope, in STS2_FORGE_TOKEN or in
${XDG_CONFIG_HOME:-~/.config}/sts2/token (chmod 600).

Acquire needs STS2_STEAM_USERNAME, DepotDownloader, and a Steam account that
owns the game. Install needs neither a token nor Steam. Its directory defaults
to where acquire puts the build. It may prompt for a
password or Steam Guard code; neither is handled or stored by this script.
USAGE
    exit 2 ;;
esac
