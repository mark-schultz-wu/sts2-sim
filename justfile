# Every part of this repository's workflow that is a command someone runs.
# Anything with real logic in it belongs in `cargo xtask`, and this file calls
# that instead of growing its own.
#
# The four arcs, in the order a new machine meets them:
#
#   set up      vendor-game, then deploy
#   change code check, which is what CI runs
#   exercise it selftest, determinism-test, live
#   re-pin      publish-game, vendor-game, codegen, check
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

# The same set CI runs, so that a green run here means a green run there.
[doc("Everything CI runs: tests, lints, and the mod build")]
check: test lint mods

[doc("The Rust suite, minus the tests that need the game")]
test:
    cargo test --workspace

[doc("Clippy, rustfmt, and the prose rules")]
lint: prose
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# Documentation is part of the interface, and rots the same way code does.
[doc("The prose rules alone, over the Rust documentation")]
prose:
    ./tools/prose-lint.py

# Needs the game's own assemblies, which are not in the repository. A machine
# with the vendored game already has them; otherwise `mod/game.sh fetch` brings
# down just the three. mod/game.lock says which build.
[doc("Compile the C# mods, with no game to install them into")]
mods:
    ./mod/build.sh

# The dumps live in sts2-content/src/<version>/. Every dump carries the build it came from, and codegen refuses one that does
# not match the version module it generates into, so this fails loudly after a
# re-pin rather than quietly mixing two builds.
[doc("Regenerate the content tables from the dumps")]
codegen:
    cargo xtask codegen

# Unpacked where .envrc points. What a fresh machine runs before anything that touches the game. 1.2G over the
# wire, 2.0G unpacked, and a no-op once the unpacked copy says it is already at
# the version in mod/game.lock. A machine that only compiles the mods can take
# the 17M of assemblies alone with `mod/game.sh fetch`.
[doc("Bring the pinned game down and unpack it (1.2G)")]
vendor-game:
    ./mod/game.sh fetch-game

# Half of re-pinning: this publishes, then mod/game.lock takes the version and
# hashes it prints, then `just vendor-game` brings the new build back down.
#
# Straight to Forgejo rather than through nginx, which caps a request body at
# 512M and would refuse the 1.2G archive after uploading all of it. An explicit
# STS2_FORGE_URL still wins, for a forge that is not this one.
[doc("Publish the Steam-installed build as the new pin")]
publish-game:
    STS2_FORGE_URL="${STS2_FORGE_URL:-http://127.0.0.1:3001}" ./mod/game.sh publish

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
