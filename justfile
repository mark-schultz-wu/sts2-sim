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

# The mod's own legality suite, inside the real game.
selftest: deploy
    cd "$STS2_GAME_DIR" && STS2_HEADLESS_MODE=selftest ./SlayTheSpire2 --headless --force-steam=off

# The tests that launch the game, ~3s of startup each.
#
# No --test-threads=1: the tests that share the game's profile directory carry
# `#[serial(game)]` themselves, so `cargo test --include-ignored` is safe too.
live: deploy
    cargo test --workspace -- --ignored
