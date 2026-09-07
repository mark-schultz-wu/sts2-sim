#!/usr/bin/env bash
# Build the image CI runs in, and tag it the way the runner's label names it.
#
# Run this after changing the Dockerfile or versions.env, then restart the
# runner so a queued job picks the new image up:
#
#   ./.forgejo/runner/build.sh
#   systemctl --user restart forgejo-runner-sts2
#
# The runner reaches it through its `sts2` label, which is what `runs-on` in
# the workflow names. Its other label, `docker`, is the stock image and stays
# until no branch asks for it.
#
# There is no registry: the runner is on this machine and `force_pull` is off
# in its config, so a local tag is what it runs. That is also why this is a
# script and not a workflow. Nothing in CI can build the thing CI runs in.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=/dev/null
source "$here/versions.env"

: "${RUST_VERSION:?versions.env must set RUST_VERSION}"
: "${DOTNET_CHANNEL:?versions.env must set DOTNET_CHANNEL}"

tag="${STS2_CI_IMAGE:-sts2-ci:latest}"

docker build \
  --build-arg "RUST_VERSION=$RUST_VERSION" \
  --build-arg "DOTNET_CHANNEL=$DOTNET_CHANNEL" \
  --tag "$tag" \
  "$here"

# The Dockerfile asserts on the way through, so reaching here means the layers
# ran. This is the report, and it is what a person reads to see what they got.
echo
echo "built $tag"
docker run --rm "$tag" bash -lc 'cargo --version; rustc --version; dotnet --version; just --version'
