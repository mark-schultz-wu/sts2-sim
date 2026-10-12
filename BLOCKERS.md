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

### 1. The recorder

The game plays census seeds, picking at random from its own legal
moves, with the census's Gorgets and Mercury Hourglass. Each run is
kept compactly as its seed and character, then a move index into the
legal list and a rules digest for each step. All the runs share one
binary file. Recording is run on purpose, never by CI. CI replays the file in the
sim alone.

Each game runs with a profile of its own (`Config::data_home`). Run 32 at
once. At that width 400 moves took 31.6 seconds, against 26.5 for one
game. So 1500 runs take about an hour. A game crashes now and then, a
segfault in a `WorkerThread` at one address, seen on 2026-10-09 and
2026-10-11. The connection closes, and the recorder plays that run
again rather than keep a cut one.

### 2. Coverage reports the recorded runs

`cargo xtask coverage` reports how far the sim follows each recorded
run and 0RACLE1-REGENT-2, by character. It counts steps matched and runs
matched to the end, and lists the commonest first splits. The check
ratchets the total of matched steps so it cannot fall.

### 3. 0RACLE1-REGENT-2 parts at step 537

Sovereign Blade previews 29 damage in the game and 24 in the sim.

### 4. Every split the recorded runs find

Each first split becomes its own item, here, as the recorded runs
reach it. The live `unimplemented!()` branches surface this way.

### 5. A listener walk that can pause

The game walks `AfterCardPlayed` listeners one at a time, and a draw
can pause the walk. The sim raises them all first, so Bygone Effigy's
Slow counts a card early in the preview while Stratagem's question
waits. Needs a design, for Mark.

### 6. Ascension

The sim threads the ascension level through, but every scenario and
census run plays at `None`. Record runs at each level, and each split
they find becomes an item.

### 7. Run modifiers

A run started with any modifier panics (`crates/sts2-sim/src/engine/sim.rs`).
Hoarder's removal ban is one rule known to need one. Port the custom
mode's modifiers, and record runs with each.

### 8. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

### 9. v0.111.0

Only the generated tables exist. Its behaviour layer has to be written,
and its runs recorded.
