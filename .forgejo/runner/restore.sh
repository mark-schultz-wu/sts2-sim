#!/usr/bin/env bash
# Build the runner image for this checkout when the host does not hold it.
#
# The `image` job runs this on the host runner before any job that runs in the
# image. The image lives only on this machine, so anything that deletes it, such
# as `docker system prune -a`, leaves the `sts2` label with nothing to start.
# That job then builds it again rather than failing every run until a person
# does.
#
# The builder runner takes one job at a time. So two runs that both find the
# image missing build it once: the second finds the first one's image.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repository="${STS2_CI_IMAGE_REPOSITORY:-sts2-ci}"
image="$repository:$("$here/digest.sh")"

if docker image inspect "$image" >/dev/null 2>&1; then
  echo "restore.sh: $image is present"
  exit 0
fi

echo "restore.sh: $image is missing, so this job builds it"
"$here/build.sh"
docker image inspect "$image" >/dev/null
