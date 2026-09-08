#!/usr/bin/env bash
# Check out the commit a job is for.
#
# Baked into the image as `sts2-checkout`, because the alternative fetches an
# action over the network at the start of every job. A name that failed to
# resolve took down two jobs of a run whose code was fine. That is the failure
# this image exists to stop: everything a job needs is in it.
#
# What it does is what `actions/checkout` did, less the parts no job here uses:
# no submodules, no tags, no LFS, one repository.
set -euo pipefail

: "${GITHUB_SERVER_URL:?the runner sets this}"
: "${GITHUB_REPOSITORY:?the runner sets this}"
: "${GITHUB_SHA:?the runner sets this}"
: "${STS2_FORGE_TOKEN:?the job has to pass the token for this run}"

# On the command line rather than in `.git/config`. The workspace outlives the
# job, so a token written into it would outlive the job too.
auth="Authorization: Basic $(printf 'oauth2:%s' "$STS2_FORGE_TOKEN" | base64 -w0)"

git init -q .
git remote remove origin 2>/dev/null || true
git remote add origin "$GITHUB_SERVER_URL/$GITHUB_REPOSITORY.git"

# Every branch, so the history is whole. `vale-lint --commits` walks from the
# base branch, and a shallow checkout has nothing to walk. The commit is asked
# for by name as well, because a pull request's head is what this job is for
# and the branch it sits on may have moved.
git -c http.extraheader="$auth" fetch -q --no-tags --prune --force \
  --no-recurse-submodules origin \
  "+refs/heads/*:refs/remotes/origin/*" \
  "+$GITHUB_SHA:refs/remotes/ci/head"

git checkout -q --force --detach refs/remotes/ci/head
# Tracked files come back with the checkout. Untracked ones are a previous
# job's leavings, and `-x` is left off so a build directory survives.
git clean -qfd

test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
