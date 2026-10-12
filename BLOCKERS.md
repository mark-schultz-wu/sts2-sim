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

### 1. Pael's Flesh

47 runs first part after Pael offers its Flesh.

### 2. Test Subject's turn

47 runs first part after a turn ends against the Test Subject boss.

### 3. The Knights' turn

46 runs first part after a turn ends against the Knights elite.

### 4. The Slumbering Beetle's turn

31 runs first part after a turn ends against a Slumbering Beetle.

### 5. The Queen's turn

30 runs first part after a turn ends against the Queen boss.

### 6. A Divine answer

30 runs first part after answering a Divine question.

### 7. Aeonglass's turn

29 runs first part after a turn ends against the Aeonglass boss.

### 8. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. `cargo xtask split <index> <name>` writes a
run, up to its first split, as a scenario, which names what parts. The
live `unimplemented!()` branches surface this way.

### 9. A listener walk that can pause

The game walks `AfterCardPlayed` listeners one at a time, and a draw
can pause the walk. The sim raises them all first, so Bygone Effigy's
Slow counts a card early in the preview while Stratagem's question
waits. Needs a design, for Mark.

### 10. Ascension

The sim threads the ascension level through, but every scenario and
census run plays at `None`. Record runs at each level, and each split
they find becomes an item.

### 11. Run modifiers

A run started with any modifier panics (`crates/sts2-sim/src/engine/sim.rs`).
Hoarder's removal ban is one rule known to need one. Port the custom
mode's modifiers, and record runs with each.

### 12. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

### 13. v0.111.0

Only the generated tables exist. Its behaviour layer has to be written,
and its runs recorded.
