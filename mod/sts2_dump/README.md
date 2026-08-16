# sts2_dump

Prints the game's own tables so the simulator's content can be generated from
them, rather than from someone reading the decompiled source and typing numbers.

Its own mod, not a mode of `sts2_determinism_test`: these dumps assert nothing
and test nothing. They read the model database and print it.

## Running

Each dump is a `STS2_HEADLESS_MODE`, and output goes to the log between
`[<mode>] begin` and `[<mode>] end`:

```
./mod/deploy.sh
cd "$STS2_GAME_DIR" && LD_PRELOAD=libgcc_s.so.1 STS2_HEADLESS_MODE=carddump \
    ./SlayTheSpire2 --headless --force-steam=off
```

| Mode | What it prints | Lands in |
| --- | --- | --- |
| `carddump` | every card's static data | `crates/sts2-content/src/v0_107_1/dumps/cards.txt` |
| `rngdump` | 1024 draws per seed, every `Rng` method | `crates/sts2-rng/tests/megarandom.txt` |

Then `cargo xtask codegen`.

## The one rule

Every dump stamps `ModelIdSerializationCache.Hash` in its header, and the Rust
side refuses a dump whose hash does not match the version module it generates
into. A dump taken against a different build fails loudly rather than quietly
replacing a table with one describing a game nobody is talking to.

## What is not dumped

Behaviour. A card's effect, a relic's hook, a monster's move body, and the
and a monster's move body are all code, and code gets ported by hand.

Where a value *might* be code, the dump says `dynamic` rather than guessing —
`CardDump` marks a card that overrides `Type` or `TargetType` rather than
reading it off a model with no combat running. Deliberately conservative:
marking a constant as dynamic costs one method, marking a dynamic as constant
costs a divergence nobody can see.
