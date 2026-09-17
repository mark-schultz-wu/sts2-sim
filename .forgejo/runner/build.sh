#!/usr/bin/env bash
# Build the image CI runs in, and tag it the way the runner's label names it.
#
# Run this after changing anything under this directory or a game lock, then
# restart the runner so a queued job picks the new image up:
#
#   ./.forgejo/runner/build.sh
#   systemctl --user restart forgejo-runner-sts2
#
# Forgetting to rebuild is not silent: `digest.sh` hashes the image inputs, the
# build bakes that value in, and each job compares it with its checkout.
#
# There is no registry, because the runner is on this machine and `force_pull`
# is off. A build needs a forge package token, because the image holds the
# pinned game. BuildKit receives it as a secret, so it never reaches a layer.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
# shellcheck source=/dev/null
source "$here/versions.env"

: "${RUST_VERSION:?rust-toolchain.toml must name a channel}"
: "${DOTNET_CHANNEL:?versions.env must set DOTNET_CHANNEL}"
: "${VALE_VERSION:?versions.env must set VALE_VERSION}"
: "${VALE_SHA256:?versions.env must set VALE_SHA256}"
: "${GAME_VERSION:?versions.env must set GAME_VERSION}"

token_file="${STS2_FORGE_TOKEN_FILE:-${XDG_CONFIG_HOME:-$HOME/.config}/sts2/token}"
cleanup_token=false
if [ -n "${STS2_FORGE_TOKEN:-}" ]; then
  token_file="$(mktemp)"
  cleanup_token=true
  printf '%s' "$STS2_FORGE_TOKEN" > "$token_file"
elif [ ! -f "$token_file" ]; then
  echo "build.sh: no forge token; see mod/game.sh for how to make one." >&2
  exit 1
fi

# Keep target/ and the rest of the checkout out of Docker's build context.
ctx="$(mktemp -d)"
cleanup() {
  rm -rf "$ctx"
  if $cleanup_token; then rm -f "$token_file"; fi
}
trap cleanup EXIT
cp "$here/Dockerfile" "$here/checkout.sh" "$ctx/"
mkdir -p "$ctx/mod/games"
cp "$repo/mod/game.sh" "$ctx/mod/"
cp "$repo"/mod/games/*.lock "$ctx/mod/games/"

digest="$("$here/digest.sh")"
repository="${STS2_CI_IMAGE_REPOSITORY:-sts2-ci}"
tag="$repository:$digest"

DOCKER_BUILDKIT=1 docker build \
  --secret "id=forge_token,src=$token_file" \
  --build-arg "RUST_VERSION=$RUST_VERSION" \
  --build-arg "DOTNET_CHANNEL=$DOTNET_CHANNEL" \
  --build-arg "VALE_VERSION=$VALE_VERSION" \
  --build-arg "VALE_SHA256=$VALE_SHA256" \
  --build-arg "GAME_VERSION=$GAME_VERSION" \
  --build-arg "STS2_CI_DIGEST=$digest" \
  --tag "$tag" \
  "$ctx"

# The Dockerfile asserts on the way through, so reaching here means the layers
# ran. This is the report, and it is what a person reads to see what they got.
echo
echo "built $tag  digest $digest"
docker run --rm "$tag" bash -lc '
  cargo --version; rustc --version; dotnet --version; just --version; vale --version
  echo "game: $(cat "$XDG_DATA_HOME/sts2/games/'"$GAME_VERSION"'/.pinned-version")"
  echo "digest: $(cat /etc/sts2-ci.digest)"'
