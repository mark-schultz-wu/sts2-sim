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
| `monsterdump` | HP ranges and move state machines, at every ascension | `crates/sts2-content/src/v0_107_1/dumps/monsters.txt` |
| `powerdump` | every power's static data | `crates/sts2-content/src/v0_107_1/dumps/powers.txt` |
| `rngdump` | 1024 draws per seed, every `Rng` method | `crates/sts2-rng/tests/megarandom.txt` |

Then `cargo xtask codegen`.

## The one rule

Every dump stamps `ModelIdSerializationCache.Hash` in its header, and the Rust
side refuses a dump whose hash does not match the version module it generates
into. A dump taken against a different build fails loudly rather than quietly
replacing a table with one describing a game nobody is talking to.

## Ascension

`AscensionManager.HasLevel` is `_level >= (int)level`, so ascension is a scalar
and each level is a threshold: anything behind
`AscensionHelper.GetValueIfAscension` is a step function of the run's level.

`monsterdump` therefore reads every value at all eleven levels and emits a
`scale*` row wherever one actually moves, rather than dumping one ascension and
calling it the table. Which levels matter is measured, not read off the source —
which is how Gremlin Merc turned up gating its damage on `ToughEnemies` where
every other monster gates on `DeadlyEnemies`.

## What is not dumped

Behaviour. A card's effect, a relic's hook, a monster's move body, and the
lambdas a monster's branch weights are written in are all code, and code gets
ported by hand.

Where a value *might* be code, the dump says `dynamic` rather than guessing —
`CardDump` marks a card that overrides `Type` or `TargetType`, `MonsterDump`
marks a branch weight whose delegate could read mutable state, and `PowerDump`
marks any member whose IL is not literally "push a number, return". All three
are deliberately conservative: marking a constant as dynamic costs one method,
marking a dynamic as constant costs a divergence nobody can see.

`PowerDump` needs the IL because `CardDump`'s rule does not carry over.
"Overrides the base" works for cards, whose `CardModel.Type` has an
implementation to inherit; `PowerModel.Type` and `StackType` are `abstract`, so
every power overrides them and the rule would mark all 273 dynamic.
