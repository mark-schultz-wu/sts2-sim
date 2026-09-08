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
: "${GITHUB_TOKEN:?the job has to pass the token for this run}"

# On the command line rather than in `.git/config`. The workspace outlives the
# job, and a token written into one outlives the job with it.
auth="AUTHORIZATION: basic $(printf 'x-access-token:%s' "$GITHUB_TOKEN" | base64 -w0)"

git init -q .
git remote remove origin 2>/dev/null || true
git remote add origin "$GITHUB_SERVER_URL/$GITHUB_REPOSITORY.git"

# Every branch, so the history is whole. `vale-lint --commits` walks from the
# base branch, and a shallow checkout has nothing to walk.
#
# The commit is asked for by name as well. A pull request head is what the job
# is for, and the branch it sits on can move under it.
git -c http.extraheader="$auth" fetch -q --no-tags --prune --force \
  --no-recurse-submodules origin \
  "+refs/heads/*:refs/remotes/origin/*" \
  "+$GITHUB_SHA:refs/remotes/ci/head"

git checkout -q --force --detach refs/remotes/ci/head
# Tracked files come back with the checkout. Untracked ones are a previous
# job's leavings, and `-x` is left off so a build directory survives.
git clean -qfd

test "$(git rev-parse HEAD)" = "$GITHUB_SHA"
