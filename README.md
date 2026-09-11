# sts

An STS2 simulator, meant to be fast enough to train agents in.

The real game can be driven programmatically, but only at a few hundred steps/s.
A training run wants about 10⁹. So the game is not the environment. It is the
reference that the simulator answers to, step by step, on the same seed.

So far that's five mods, the content they print, and a Rust side that consumes
both. The simulator itself has not started.

## The mods

| mod | what it does |
|---|---|
| [`sts2_determinism`](mod/sts2_determinism/README.md) | makes the game reproducible: same seed, same run |
| [`sts2_headless`](mod/sts2_headless/README.md) | drives it with no renderer and no human, over a line protocol |
| [`sts2_dump`](mod/sts2_dump/README.md) | prints the game's own content tables |
| [`sts2_determinism_test`](mod/sts2_determinism_test/README.md) | measures what the first one claims |
| [`sts2_headless_test`](mod/sts2_headless_test/README.md) | the legality suite for the second |

Each test mod is installed only to run its suite, so nothing shipped carries
tests. `mod/deploy.sh` builds and installs all five. The game's own manifest
`dependencies` field orders them.

[The protocol](mod/sts2_headless/PROTOCOL.md) is the wire contract on one page.
The mods answer to a standard, split between
[determinism](mod/sts2_determinism/REQUIREMENTS.md) (D1–D3) and
[headless](mod/sts2_headless/REQUIREMENTS.md) (H1–H12), so a requirement's
prefix names the file it lives in.

## The crates

| crate | what it is |
|---|---|
| `sts2-core` | the vocabulary both a game and a simulator answer in |
| `sts2-content` | the game's content as Rust tables, generated from the dumps |
| `sts2-headless` | a client for the headless mod: launch, handshake, one typed call per decision |
| `sts2-sim` | the game engine and its draw stream, checked against the real game |
| [`sts2-cli`](crates/sts2-cli/README.md) | `sts2`, which plays a run at a terminal, and `sts2-fuzz`, which plays one at random |
| `xtask` | `cargo xtask dump` asks the game for its tables, and `codegen` turns them into `sts2-content` |

Content is thirteen domains: acts, afflictions, cards, characters, enchantments,
encounters, events, modifiers, monsters, orbs, potions, powers and relics. The
hook surface comes with them. It is measured, but it has no table, because a
simulator implements it rather than looking it up.

What a piece of content *does* is code, and gets ported by hand. What it *is* is
a table. The table is generated, so that nobody types a number the game already
knows. The generated tree is committed rather than built by a `build.rs`, so a
content patch reads as a diff.

Every dump names the build it came from, with two hashes. The content hash covers
the set of model ids. The assembly hash moves on any code change at all.

## More than one build

STS2 churns 20-50 cards on a minor version, so a pin is what stops a result
changing because the game did. Each pinned build has a lock under `mod/games/`.
`STS2_GAME_VERSION` picks which one everything uses. The mods compile against
it, the dumps land under it, and `sts2` drives it.

Two builds are pinned: v0.107.1 and v0.111.0. Both compile the whole mod set
from the same sources, with one adapter for each build under every mod's
`Compat/`. Both ship generated tables. `just check` builds every lock, so a
change that works only against the build you deployed fails.

v0.107.1 is the floor. It is the build that replaced the game's PRNG with
xoshiro256\*\*, and `sts2-sim` owns that generator.

One hash alone is not enough. A rebalance that adds no content leaves the content
hash equal and every generated number wrong. Codegen refuses to generate when two
dumps disagree, so a table can never quietly come to describe a game that nobody
talks to.

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) is where things live, and why. It
covers the content/code dividing rule, and why ported behaviour belongs beside
the tables rather than in the engine. It also covers how the hook surface is held
to what was measured, and the four layers a table has to survive to be believed.
