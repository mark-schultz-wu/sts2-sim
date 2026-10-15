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

### 1. Card waits in the other pools

As the Ironclad's and the Silent's: a resume point where a command can
leave a question and a command that runs on follows it. A draw, an
auto-play and a card's own selection already queue behind a question, so
none needs one. Thorns and Centennial Puzzle can ask on a hit or a loss.
Vicious can on Vulnerable, Dark Embrace on an exhaust, Reflex on a
discard, and Stratagem on a shuffle.
A scenario for each card one of them reaches. The Regent,
then the colorless, event and token cards.
Generated plays already wait.

### 2. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. `cargo xtask split <index> <name>` writes a
run, up to its first split, as a scenario, which names what parts. The
live `unimplemented!()` branches surface this way.

### 3. v0.111.0

Only the generated tables exist. Codegen runs per build as it does now.
The behaviour is written once, for v0.107.1. A v0.111.0 rule whose game
code did not change calls that one. The v0.107.1 rule first becomes
generic over the build. A rule that changed gets one of its own. Then record its runs.

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

