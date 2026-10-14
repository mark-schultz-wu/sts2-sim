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

### 1. Double Boss

The sim has not ported the last rung, `DoubleBoss`. `RunManager` draws a
second boss for the last act off `UpFront` as it builds the run, so every
draw after it moves too. The sim ports every other rung: recorded runs at
each of levels 1 to 9 match, but for the splits in the next item. Recording at level 10
waits for this one.

### 2. Splits the ascension runs find

`cargo xtask record --ascension <rung> --append` recorded 100 runs at each
of levels 1 to 9. Six part from the game, each for a reason of its own:

- Runs 1855 and 1955, the Queen. A token the player auto-plays kills the
  Torch Head Amalgam, and the sim's Queen enrages. The game's Queen goes
  on burning bright, so her `AfterDeath` there does not set her
  `HasAmalgamDied`, or her branch has not resolved.
- Run 1800. The run's RNG counter is one ahead of the game's as a turn
  ends against a Slimed Berserker.
- Run 2090. A relic's amount reads 0 in the sim and 5 in the game as
  Synchronize is played.
- Run 2259. The Relic Trader's trade leaves the belt in another order.
- Run 2317. Vicious's play leaves the player two hit points short.

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

### 5. Run modifiers

A run started with any modifier panics (`crates/sts2-sim/src/engine/sim.rs`).
Hoarder's removal ban is one rule known to need one. Port the custom
mode's modifiers, and record runs with each.

### 6. v0.111.0

Only the generated tables exist. Its behaviour layer has to be written,
and its runs recorded.
### 7. Every hook on the walk cursor

Every event hook walks with the cursor, and the eager `notify` goes. The
cursor's snapshot costs 0.7% on the three hooks it walks now, so every
hook needs a cheaper one first. Power instances and card ids only grow, so
a cursor can skip what arrived after the walk began and take no snapshot.

`AfterCardPlayed` is the hook that needs it most, since every play walks
it. A draw inside the walk can stop on a question, as Iron Club's does on
Stratagem's pick. The game has not yet asked the listeners after it, and
the sim asked them all first. So Pinpoint in hand counts a Defend early,
and shows one cheaper while the pick waits. A walk that parks there, on
the snapshot cursor, cost 2.5% a step.

### 8. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

