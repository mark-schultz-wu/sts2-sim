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

### 2. The behaviour generic over the build

The v0.107.1 behaviour moves out of its build into
`sts2_content::behaviour`, generic over `V: Build`, one pool or domain
at a time. `cargo xtask generic <file>` does the mechanical part. A
content path becomes a const from `sts2_content::ids`,
`V::Orb::LIGHTNING_ORB`, and a pattern naming one becomes a guard,
since a `match` cannot name a generic const. A match left with no
catch-all gains an `unreachable!()` arm. It lists what it leaves for a
hand. Each move adds to `Build` the bounds its rules read. A row lookup
a build spelled as an inherent method becomes a `sts2_core::Table`
impl in codegen. A spike on the powers' hooks and every card play
measured 12,264 instructions a step before and 12,184 after. Every
recorded run still matches after each move.

The orbs are shared. The rest: enchantments, modifiers,
acts, encounters, potions, relics, powers, monsters, events, and the
cards, pool by pool.

### 3. v0.111.0's own rules

v0.111.0's `Rules` uses the shared behaviour. A rule whose game code
changed between the builds gets a port of its own, chosen by the build.
Content new in v0.111.0 is ported there. Dump and record with the
mod built for v0.111.0, and its recorded runs are the measure.

### 4. A walk that shares only who listens

The snapshot copies each walk's listeners. Most walks see none arrive or
leave: 1.3% of `AfterCardPlayed` walks do, and 28% of `AfterSideTurnEnd`
ones. Sharing the fight's containers behind `Arc` cost 2.6% a step before
any walk used them, and listeners write hit points and amounts mid-walk.
A cheaper walk shares only which listeners there are and their order,
kept apart from what they hold. That is part of the flat-state pass,
which needs a design first.

### 5. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

