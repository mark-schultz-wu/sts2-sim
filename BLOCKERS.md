# Blockers

What stands between the simulator and a complete single-player port, in
the order to work them. `/blocker-grind` takes the first open item, one
PR at a time.

## Done means

Every recorded run matches the simulator step by step, to the run's end.
The game plays each one itself, picking at random from its own legal
moves. The recorded runs cover:

- each character, at every ascension level;
- runs with each modifier the game's custom mode offers;
- both builds, v0.107.1 and v0.111.0;
- the long run 0RACLE1-REGENT-2.

Speed is a goal as well as a guard. The target is still to be set.

## How the list changes

- A PR that fixes an item deletes it, in the same commit.
- A PR that finds a new blocker adds it, where it belongs in the order,
  in the same commit. A blocker found while fixing another goes above it
  only when the fix cannot land without it.
- An item too big for one PR is split into items that each fit one.
- An item that needs Mark's decision says so, and the grind stops there
  and asks.

## Open

### 1. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. `cargo xtask split <index> <name>` writes a
run, up to its first split, as a scenario, which names what parts. The
live `unimplemented!()` branches surface this way.

### 2. v0.111.0's changed rules

v0.111.0's `Hooks` and `Rules` are the shared ones in
`sts2_content::behaviour`, which mirror v0.107.1's game code. A rule
whose game code changed between the builds gets a port of its own,
chosen by the build through `OwnRules`. Content new in v0.111.0 is
ported there. `crates/sts2-golden/tests/recorded-v0.111.0.bin` holds the
census's runs on v0.111.0, and `STS2_GAME_VERSION=v0.111.0 cargo xtask
coverage` says where they part. Once none of them panics,
`recorded.rs` follows them as it does v0.107.1's, against a count of
its own.

What each shared module asks of a build, so a port knows where to go:

- `BuildContent`: the content traits in `sts2_content::ids`, the shared
  vocabulary types, and `Lists`, which a `lists!` macro beside each rule
  writes for the build.
- `Tables`: the lookups into a row that names content.
- `sts2_content::vars`: the numbers a relic, power, potion or
  enchantment prints, as consts on the build's enum. A port reads a
  number there, and a number one build lacks fails to build for it.
- `OwnRules`: the build's generated move and play scripts, and its own
  ports of rules that name content another build lacks. A shared
  dispatcher asks the build first and answers itself when the hook
  passes.
- The `rules!` macro, where a method that only reads a table forwards
  to the build's own tables.

`cargo xtask generic <file>` rewrites a build's rule to be generic, for
a port that starts from the other build's code. A move can shift what
the compiler inlines. The relics' and the cards' decision dispatchers
stay out of line, since the walk inlining them cost 3.9% and 3.4%.

The simulator follows 832,828 of the 1,321,928 recorded positions, 63%,
and 608 of 1,500 runs whole. A run that panics counts the positions
before its panic. `follow_run <file> <index> --diff <dir>` plays a run
on the game beside the simulator and names the sections where they first
differ, which a split with no panic needs. The commonest first splits:

- Positions that part with no panic. 51 part at the end of a turn
  against Axebots, and 37 at a travel. 39 part at a play of Forgotten
  Ritual, and 32 at Tezcatara's Toy Box. Some after a travel open a fight where the game
  plays and exhausts the hand, and the simulator asks for one card.
- Potions new here: Ambergris and the Ambergris power.
- Diamond Diadem and Outbreak changed, and are v0.111.0's own,
  unported.
- Distinguished Cape adds two curses where it took nine maximum health,
  Sere Talon's curses moved, and Inky lost its damage. Each reads a
  number v0.111.0 lacks, so its shared port holds v0.107.1's.

About 60 of the game's engine files changed too, outside the content
models: its commands, game actions, entities and runs. `sts2-sim`
mirrors v0.107.1's engine and has no seam for a build to differ. A
split that traces to the engine needs that seam, which needs Mark's
design first.

### 3. A walk that shares only who listens

The snapshot copies each walk's listeners. Most walks see none arrive or
leave: 1.3% of `AfterCardPlayed` walks do, and 28% of `AfterSideTurnEnd`
ones. Sharing the fight's containers behind `Arc` cost 2.6% a step before
any walk used them, and listeners write hit points and amounts mid-walk.
A cheaper walk shares only which listeners there are and their order,
kept apart from what they hold. That is part of the flat-state pass,
which needs a design first.

### 4. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

