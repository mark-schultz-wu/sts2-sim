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

### 1. A relic's `IsAllowed`

Each pull from a relic bag first takes out, for good, every relic whose
`IsAllowed` is false then: `RemoveDisallowedRelicsFromDeques`. 23 relics
override it. Shovel and Book of Five Rings stop at the act 3 chest, and
others read the deck, the gold or the relics held. The sim reads none of
it, so after one goes the bags part. Shovel, Book of Five Rings and Meal
Ticket stand in the sim's chest or shop where the game has another.
`setup.rs`'s `RelicBag` needs the rule, and each relic its override.

### 2. Travel

97 runs first part after a move on the map. Maw Bank, a skipped chest
relic and an upgraded card's calculated base are fixed. Of a sample of
eight, the other three were the relic bags of item 1.

`cargo xtask split <index> <name>` writes a run, up to its first split,
as a scenario.

### 3. Decimillipede's turn

89 runs first part after a turn ends against the Decimillipede elite.

### 4. Soul Fysh's turn

53 runs first part after a turn ends against the Soul Fysh boss.

### 5. Pael's Flesh

47 runs first part after Pael offers its Flesh.

### 6. Sovereign Blade

46 runs first part after Sovereign Blade is played.

### 7. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. The live `unimplemented!()` branches surface
this way.

### 8. A listener walk that can pause

The game walks `AfterCardPlayed` listeners one at a time, and a draw
can pause the walk. The sim raises them all first, so Bygone Effigy's
Slow counts a card early in the preview while Stratagem's question
waits. Needs a design, for Mark.

### 9. Ascension

The sim threads the ascension level through, but every scenario and
census run plays at `None`. Record runs at each level, and each split
they find becomes an item.

### 10. Run modifiers

A run started with any modifier panics (`crates/sts2-sim/src/engine/sim.rs`).
Hoarder's removal ban is one rule known to need one. Port the custom
mode's modifiers, and record runs with each.

### 11. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

### 12. v0.111.0

Only the generated tables exist. Its behaviour layer has to be written,
and its runs recorded.
