#!/usr/bin/env bash
# Hash every input represented by the machine-local runner image. CI compares
# this with the value baked into the image so stale images fail loudly.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"

{
  for file in "$here"/Dockerfile "$here"/versions.env \
    "$here"/build.sh "$here"/digest.sh "$here"/checkout.sh; do
    sha256sum "$file" | cut -d' ' -f1
  done
  for lock in "$repo"/mod/games/*.lock; do
    sha256sum "$lock" | cut -d' ' -f1
  done
} | sha256sum | cut -c1-16
