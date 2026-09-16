# Every part of this repository's workflow that is a command someone runs.
# Anything with real logic in it belongs in `cargo xtask`, and this file calls
# that instead of growing its own.
#
# The four arcs, in the order a new machine meets them:
#
#   set up      vale, hooks, vendor-game, then deploy
#   change code check, which is what CI runs
#   exercise it selftest, determinism-test, live
#   re-pin      publish-game, vendor-game, dump, codegen, check
#
# Policy: this file is reviewed periodically and recipes nobody runs are
# deleted. A command runner that accumulates dead recipes is worse than no
# command runner, because you stop trusting that the ones left still work.
#
# STS2_GAME_DIR comes from .envrc and is used straight out of the environment
# rather than through a just variable. `env()` is evaluated eagerly, so binding
# it here would make `just check` -- which never touches the game -- fail when
# direnv has not been allowed.

_default:
    @just --list

# The same set CI runs, so that a green run here means a green run there. Every
# step CI's `check` job runs belongs in one of these, or the promise this line
# makes is not true.
[doc("Everything CI runs: tests, lints, generated tables, and the mod build")]
check: check-repo mods

# The half of `check` that needs nothing but this repository, which is what the
# pre-push hook runs. `mods` is the other half, and it needs the game's own
# assemblies, which not every machine has.
[doc("Everything CI runs that needs only this repository")]
check-repo: test lint generated

[doc("The Rust suite, minus the tests that need the game")]
test:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo test --workspace --exclude sts2-golden
    # `sts2-golden` is excluded because two of its tests replay a recording
    # through a real game. These three hold the simulator to what the game
    # already wrote down, so they run here, with no game anywhere.
    cargo test -p sts2-golden --test map --test setup --test travel --test play

[doc("Clippy, rustfmt, and the prose rules")]
lint: prose
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# Documentation is part of the interface, and rots the same way code does. So is
# a commit message, which is why the last of these reads them too. CI lints the
# messages this branch adds to its base, and names that base itself. Here the
# base is an argument, because a local branch has no environment to read it out
# of.
#
# The base is `forgejo/main` and not `main`, because a local `main` goes stale
# and a stale base makes this read the whole history back to it. Fetch before a
# check that has to be exact.
#
# `--self-test` comes first. A rule that matches nothing reports a clean
# repository for the wrong reason.
[doc("The prose rules: structure, style, and this branch's commit messages")]
prose base="forgejo/main":
    ./tools/prose-lint.py
    ./tools/vale-lint.py --self-test
    ./tools/vale-lint.py
    ./tools/vale-lint.py --commits --base {{base}}

# Text outside the repository cannot be part of `check`: a local checkout does
# not hold the pull request description. Give that text a Just entry point too,
# so CI and a local check of copied text run the same command.
[doc("Lint prose supplied on standard input, such as a pull request description")]
prose-stdin label="standard input":
    ./tools/vale-lint.py --stdin {{quote(label)}}

# What lets a review skip the generated tables. A hand edit to a generated file,
# or a generator whose output nobody refreshed, fails here rather than as a
# divergence nobody can place. This reads the dumps in the repository and never
# launches the game.
[doc("Check the generated tables still reproduce from the dumps")]
generated:
    #!/usr/bin/env bash
    set -euo pipefail
    builds=0
    for lock in mod/games/*.lock; do
        STS2_GAME_VERSION="$(basename "$lock" .lock)" cargo run -q -p xtask -- codegen
        builds=$((builds + 1))
    done
    test "$builds" -gt 0
    git diff --exit-code -- 'crates/sts2-content/src/*/generated'

# How far the simulator gets through a real run before it disagrees with the
# game. The build script takes the game's answers once and keeps them under
# target/, so later runs need no game. `cargo clean` drops that cache.
[doc("How far the simulator replays a real run")]
divergence:
    cargo test -p sts2-golden --test divergence -- --nocapture

# What the game answers for every scenario, as one file. Needs the game and the
# mod, and is the only thing in the suite that launches one.
[doc("Rewrite the recorded scenario answers")]
scenarios: deploy
    STS2_UPDATE=1 cargo test -p sts2-golden --test scenarios -- --nocapture

# The reviewable answers derived from the cached transcript. A re-pin runs this
# to put any changed digest, map, or opening room in the diff.
[doc("Rewrite the committed oracle answers after a re-pin")]
golden: deploy
    STS2_UPDATE=1 cargo test -p sts2-golden --test committed -- --nocapture

# Needs the game's own assemblies, which are not in the repository. A machine
# with the vendored game already has them; otherwise `mod/game.sh fetch
# <version>` brings down just the three. The selected lock under mod/games/ says
# which build.
#
# Every lock under mod/games/, not just the default build: a lock there is the
# claim that the mods compile against that build, and this is what makes the
# claim true. Each mod compiles one adapter out of its Compat/ directory, so a
# change that only compiles against the build you happen to have deployed is a
# change that breaks the other.
[doc("Compile the C# mods against every supported build")]
mods:
    #!/usr/bin/env bash
    set -euo pipefail
    for lock in mod/games/*.lock; do
        version="$(basename "$lock" .lock)"
        echo "==> $version"
        STS2_GAME_VERSION="$version" ./mod/build.sh
    done

# Ask the game for its own tables, one launch per mode, about 30s for all 16.
#
# The front half of a re-pin: `dump` asks the new build what it contains, then
# `codegen` turns that into Rust. Re-running it against an unchanged build
# rewrites every dump with the bytes already there, so a diff here is the game
# having changed, never the pipeline.
[doc("Ask the game for its own tables, and file what it prints")]
dump: deploy
    cargo xtask dump

# The dumps live in sts2-content/src/<version>/. Every dump carries the build it came from, and codegen refuses one that does
# not match the version module it generates into, so this fails loudly after a
# re-pin rather than quietly mixing two builds.
[doc("Regenerate the content tables from the dumps")]
codegen:
    cargo xtask codegen

# The prose linter, which `just prose` needs and which no package manager here
# carries. Pinned by version and by checksum out of the same versions.env the
# runner image builds from, so a local check and a CI run read the same rules
# through the same binary.
[doc("Install the pinned prose linter into ~/.local/bin")]
vale:
    #!/usr/bin/env bash
    set -euo pipefail
    . .forgejo/runner/versions.env
    test -n "$VALE_VERSION"
    test -n "$VALE_SHA256"
    archive="$(mktemp -d)/vale.tar.gz"
    curl --proto '=https' --tlsv1.2 -fsSL \
      --retry 8 --retry-all-errors --retry-delay 2 \
      --connect-timeout 15 --max-time 300 \
      -o "$archive" \
      "https://github.com/vale-cli/vale/releases/download/v$VALE_VERSION/vale_${VALE_VERSION}_Linux_64-bit.tar.gz"
    echo "$VALE_SHA256  $archive" | sha256sum -c -
    mkdir -p "$HOME/.local/bin"
    tar -xzf "$archive" -C "$HOME/.local/bin" vale
    rm "$archive"
    # An install that installed nothing must fail here rather than two recipes
    # later, so this asks the binary what it is.
    test "$("$HOME/.local/bin/vale" --version | grep -o '[0-9.]*$')" = "$VALE_VERSION"

# Points git at .githooks/, which holds a pre-push that runs `check-repo`. Once
# per clone: a hook path is machine state, and a repository cannot set it.
[doc("Install this repository's git hooks")]
hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    git config core.hooksPath .githooks
    test "$(git config core.hooksPath)" = ".githooks"

# Unpacked where .envrc points. What a fresh machine runs before anything that touches the game. 1.2G over the
# wire, 2.0G unpacked, and a no-op once the unpacked copy says it is already at
# the selected version. A machine that only compiles the mods can take
# the 17M of assemblies alone with `mod/game.sh fetch`.
[doc("Bring the pinned game down and unpack it (1.2G)")]
vendor-game version="v0.107.1":
    ./mod/game.sh fetch-game {{version}}

# Steam is only an acquisition source. This places a historical manifest in a
# versioned staging directory; `publish-game` promotes it to the private forge.
[doc("Acquire one exact Steam manifest for later publishing")]
acquire-game version manifest branch="public":
    ./mod/game.sh acquire {{version}} {{manifest}} {{branch}}

# Half of re-pinning: this publishes from an acquired install after a lock under
# mod/games/ names its package version. Put the hashes it prints in that lock;
# then `just vendor-game <version>` proves Forgejo can reproduce the install.
#
# Straight to Forgejo rather than through nginx, which caps a request body at
# 512M and would refuse the 1.2G archive after uploading all of it. An explicit
# STS2_FORGE_URL still wins, for a forge that is not this one.
[doc("Publish the Steam-installed build as the new pin")]
publish-game version="v0.107.1" source="/etc/wolf/profile-data/user/WolfSteam/.steam/steam/steamapps/common/Slay the Spire 2":
    STS2_FORGE_URL="${STS2_FORGE_URL:-http://127.0.0.1:3001}" STS2_STEAM_DIR="{{source}}" ./mod/game.sh publish {{version}}

# Every recipe below depends on this, so its check for STS2_GAME_DIR is the one
# that catches an unloaded .envrc for all of them.
[doc("Build the mods and install them into the game")]
deploy:
    ./mod/deploy.sh

# Lives in mod/sts2_headless_test.
#
# LD_PRELOAD is not optional: MonoMod's native helper imports _Unwind_* from
# libgcc and declares no dependencies, and the game does not link libgcc, so
# without this every Harmony patch throws and sts2_determinism stays inert.
#
# `timeout -k` and not a bare `timeout`: the mod's own deadline (H12) is the
# first line of defence, and this is the second, for a build too broken to reach
# it. Without `-k` this would send a SIGTERM that an un-armed instance ignores,
# which is how a hung selftest once survived its supervisor and took the machine
# down with it.
[doc("The legality suite, inside the real game")]
selftest: deploy
    cd "$STS2_GAME_DIR" && timeout -k 5 300 env LD_PRELOAD=libgcc_s.so.1 STS2_HEADLESS_MODE=selftest \
        ./SlayTheSpire2 --headless --force-steam=off

# Lives in mod/sts2_determinism_test.
[doc("What sts2_determinism claims, measured")]
determinism-test: deploy
    cd "$STS2_GAME_DIR" && timeout -k 5 300 env LD_PRELOAD=libgcc_s.so.1 STS2_HEADLESS_MODE=determinism \
        ./SlayTheSpire2 --headless --force-steam=off

# ~3s of game startup each.
#
# No --test-threads=1: the tests that share the game's profile directory carry
# `#[serial(game)]` themselves, so `cargo test --include-ignored` is safe too.
[doc("The Rust tests that launch the game")]
live: deploy
    cargo test --workspace -- --ignored

# Owns its own game process rather than the daemon, so a run that kills the game
# kills nothing else. Every move is recorded beside the run, which is what makes
# a finding a file rather than a story. `docs/ARCHITECTURE.md` says what counts
# as one.
#
# Release, because a fuzz run is thousands of round trips and the debug build
# spends longer converting observations than the game spends producing them.
#
# Arguments go through: `just fuzz --seed ABCDEFGH --steps 200`. The build comes
# from STS2_GAME_VERSION, the same setting that chose STS2_GAME_DIR.
[doc("Random legal play until the run ends or something breaks")]
fuzz *args: deploy
    cargo run --release --bin sts2-fuzz -- {{args}}
