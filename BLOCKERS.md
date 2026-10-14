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

### 2. What a draw drew, when the draw pauses

`CardPileCmd.Draw` returns the cards it drew, and Scrape discards the
ones that cost energy. A shuffle inside the draw can ask, as Stratagem
does, and the draw finishes after the answer. The sim finds what Scrape
drew by comparing the hand before and after, and that comparison runs
before the draw pauses. Recorded run 1424 shows it: Scrape keeps two
Strikes and a Compact the game discards. Pillage reads its draw the same
way. The history keeps the card a draw drew but not which copy. Carrying
the copies across the pause needs a design, for Mark.

### 3. A listener walk that can pause

The game walks `AfterCardPlayed` listeners one at a time, and a draw
can pause the walk. The sim raises them all first, so Bygone Effigy's
Slow counts a card early in the preview while Stratagem's question
waits.

The walk also asks `Contains(item)` as it reaches each listener, so one
an earlier listener took off is skipped. Recorded run 1045 shows it:
Countdown's Doom sets off Sleight of Flesh, which kills the Living
Shield, and the game never asks its Rampart. The sim asked Rampart with
the rest before any of them acted, and the Turret Operator gains block.
Needs a design, for Mark.

### 4. Ascension

The sim threads the ascension level through, but every scenario and
census run plays at `None`. Record runs at each level, and each split
they find becomes an item.

### 5. Run modifiers

A run started with any modifier panics (`crates/sts2-sim/src/engine/sim.rs`).
Hoarder's removal ban is one rule known to need one. Port the custom
mode's modifiers, and record runs with each.

### 6. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

### 7. v0.111.0

Only the generated tables exist. Its behaviour layer has to be written,
and its runs recorded.
