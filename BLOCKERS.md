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

### 1. A walk a question stands still

The game walks `AfterCardPlayed` listeners one at a time, and a draw
can pause the walk. The sim raises them all first, so Bygone Effigy's
Slow counts a card early in the preview while Stratagem's question
waits.

The turn's start walks its listeners the same way. In recorded runs 461
and 1008, Tools of the Trade's discard and Gambling Chip's choice stand
the walk still. The game has not yet reached Pendulum. The
sim counted Pendulum's turn before the question. Entropy's choice does
the same in runs 611, 616, 764, 1095 and 1466, and Tyranny's in runs 682
and 853.

The design: an effect that asks a question parks the rest of the
cursor's snapshot, which goes on after the answer.

### 2. Every hook on the walk cursor

Every event hook walks with the cursor, and the eager `notify` goes. The
cursor's snapshot costs 0.7% on the three hooks it walks now, so every
hook needs a cheaper one first. Power instances and card ids only grow, so
a cursor can skip what arrived after the walk began and take no snapshot.

### 3. A power the walk reaches after it came off

`CombatState.Contains` asks of a power only whether its owner is still in
the fight. The walk still asks a power an earlier listener took off, as a
model on its own. Nemesis takes Intangible off as the enemies' turn ends, and the
game then decrements the Intangible it took off. That raises the
amount-changed hooks and writes the history. The sim's walk skips a power
its owner no longer carries, since it keeps no model for one taken off. No
recorded run parts on it yet. Keeping the model for the walk needs a
design, for Mark.

### 4. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. `cargo xtask split <index> <name>` writes a
run, up to its first split, as a scenario, which names what parts. The
live `unimplemented!()` branches surface this way.

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
