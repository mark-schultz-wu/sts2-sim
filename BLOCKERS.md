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

### 1. Coverage reports the recorded runs

`cargo xtask coverage` reports how far the sim follows each recorded
run and 0RACLE1-REGENT-2, by character. It counts steps matched and runs
matched to the end, and lists the commonest first splits. The check
ratchets the total of matched steps so it cannot fall.

`crates/sts2-golden/tests/recorded.bin` holds the census's 1500 runs.
`cargo xtask record` plays them again, on purpose and never in CI. The
sim follows 4 to the end, and 747k of their 1.32M steps. The first
splits spread wide, from step 14 on.

### 2. 0RACLE1-REGENT-2 parts at step 537

Sovereign Blade previews 29 damage in the game and 24 in the sim.

### 3. Every split the recorded runs find

Each first split becomes its own item, here, as the recorded runs
reach it. The live `unimplemented!()` branches surface this way.

### 4. A listener walk that can pause

The game walks `AfterCardPlayed` listeners one at a time, and a draw
can pause the walk. The sim raises them all first, so Bygone Effigy's
Slow counts a card early in the preview while Stratagem's question
waits. Needs a design, for Mark.

### 5. Ascension

The sim threads the ascension level through, but every scenario and
census run plays at `None`. Record runs at each level, and each split
they find becomes an item.

### 6. Run modifiers

A run started with any modifier panics (`crates/sts2-sim/src/engine/sim.rs`).
Hoarder's removal ban is one rule known to need one. Port the custom
mode's modifiers, and record runs with each.

### 7. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

### 8. v0.111.0

Only the generated tables exist. Its behaviour layer has to be written,
and its runs recorded.
