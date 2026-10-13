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

### 1. What a draw drew, when the draw pauses

`CardPileCmd.Draw` returns the cards it drew, and Scrape discards the
ones that cost energy. A shuffle inside the draw can ask, as Stratagem
does, and the draw finishes after the answer. The sim finds what Scrape
drew by comparing the hand before and after, and that comparison runs
before the draw pauses. Recorded run 1424 shows it: Scrape keeps two
Strikes and a Compact the game discards. Pillage reads its draw the same
way, and a drawn card can leave the hand before the comparison runs, with
no pause. In recorded run 151 a Strike Pillage draws is discarded as it
lands. The sim stops drawing there, and the game draws an Evil Eye. Runs
79, 1220 and 1419 part the same way.

The design: `Fight::draw` hands back the copies it drew, in order. A
draw that pauses keeps the ones drawn so far in its deferred entry. The
card waits, and resumes with them once the draw is over. Pillage's loop
becomes a resume point.

### 2. A listener walk that can pause

The game walks `AfterCardPlayed` listeners one at a time, and a draw
can pause the walk. The sim raises them all first, so Bygone Effigy's
Slow counts a card early in the preview while Stratagem's question
waits.

The walk also asks `Contains(item)` as it reaches each listener, so one
an earlier listener took off is skipped. Recorded run 1045 shows it:
Countdown's Doom sets off Sleight of Flesh, which kills the Living
Shield, and the game never asks its Rampart. The sim asked Rampart with
the rest before any of them acted, and the Turret Operator gains block.

The turn's start walks its listeners the same way. In recorded runs 461
and 1008, Tools of the Trade's discard and Gambling Chip's choice stand
the walk still. The game has not yet reached Pendulum. The
sim counted Pendulum's turn before the question. Entropy's choice does
the same in runs 611, 616, 764, 1095 and 1466, and Tyranny's in runs 682
and 853.

A listener's `PowerCmd.Decrement` waits its turn in the walk as well. The
sim ticks every decrement as it raises the hook, ahead of what earlier
listeners asked for. In recorded run 267, the Soul Fysh's Intangible
ticks off before Demise's damage lands, which the game caps at one. In
run 1175, Veilpiercer's own decrement takes it off before Danse Macabre
reads Reap's cost. The game then blocks for the card at its full cost. The
sim worked the cost out once, before the walk.

The design: the walk takes a snapshot of its listeners and asks each one
as it reaches it, skipping any that left. An effect that asks a question
parks the rest of the snapshot, which goes on after the answer. A
decrement is an effect, applied in its turn. Every event hook walks this
way. Each held power carries a per-fight instance id, as a card does, so
the walk can tell a removed power from a new one.

### 3. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. `cargo xtask split <index> <name>` writes a
run, up to its first split, as a scenario, which names what parts. The
live `unimplemented!()` branches surface this way.

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
