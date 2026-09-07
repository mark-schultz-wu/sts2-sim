#!/usr/bin/env bash
# Point the ordinary Forgejo runner at an already-built immutable image.
#
# This is the narrow interface used by the trusted infrastructure runner. It
# verifies the image before changing configuration, writes the config
# atomically, and only then restarts the unprivileged workload runner.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
digest="$("$here/digest.sh")"
repository="${STS2_CI_IMAGE_REPOSITORY:-sts2-ci}"
image="$repository:$digest"
config="${STS2_RUNNER_CONFIG:-$HOME/.config/forgejo-runner/sts2/config.yml}"
service="${STS2_RUNNER_SERVICE:-forgejo-runner-sts2.service}"

test -f "$config"
actual="$(docker run --rm "$image" cat /etc/sts2-ci.digest)"
test "$actual" = "$digest" || {
  echo "promote.sh: $image contains digest $actual, expected $digest" >&2
  exit 1
}

tmp="$(mktemp "$(dirname "$config")/.config.yml.XXXXXX")"
trap 'rm -f "$tmp"' EXIT
python3 - "$config" "$tmp" "$image" <<'PY'
import re
import sys

source, destination, image = sys.argv[1:]
text = open(source).read()
updated, count = re.subn(
    r"(?m)^(\s*- sts2:docker://)sts2-ci:[^\s]+\s*$",
    lambda match: match.group(1) + image,
    text,
)
if count != 1:
    raise SystemExit(f"promote.sh: expected one sts2 runner label, found {count}")
with open(destination, "w") as f:
    f.write(updated)
PY
chmod --reference="$config" "$tmp"
mv "$tmp" "$config"
trap - EXIT

systemctl --user restart "$service"
echo "promoted $image to $service"
