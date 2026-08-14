# Shell-shaped chores. Anything with real logic in it belongs in `cargo xtask`.
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

# Everything that must pass before a commit.
check: test lint

# The Rust suite, minus the tests that need the game.
test:
    cargo test --workspace

lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

# Build the mod and install it into the game's mods directory.
#
# Every recipe below depends on this, so its check for STS2_GAME_DIR is the one
# that catches an unloaded .envrc for all of them.
deploy:
    ./mod/deploy.sh

# The legality suite, inside the real game. Lives in mod/sts2_headless_test.
#
# LD_PRELOAD is not optional: MonoMod's native helper imports _Unwind_* from
# libgcc and declares no dependencies, and the game does not link libgcc, so
# without this every Harmony patch throws and sts2_determinism stays inert.
selftest: deploy
    cd "$STS2_GAME_DIR" && LD_PRELOAD=libgcc_s.so.1 STS2_HEADLESS_MODE=selftest \
        ./SlayTheSpire2 --headless --force-steam=off

# The tests that launch the game, ~3s of startup each.
#
# No --test-threads=1: the tests that share the game's profile directory carry
# `#[serial(game)]` themselves, so `cargo test --include-ignored` is safe too.
live: deploy
    cargo test --workspace -- --ignored
