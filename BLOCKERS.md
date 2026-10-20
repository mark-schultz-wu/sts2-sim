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

### 1. The splits a hunt finds

`cargo xtask hunt` plays fresh seeds on the game and the simulator side
by side, and keeps only the runs where they part. `--mixed` spreads the
seeds over every ascension level and modifier. `cargo xtask split --file
target/hunt/parted-<build>.bin <index> <name>` writes a kept run, up to
its split, as a scenario. A fix lands with its run, recorded whole, in
`crates/sts2-golden/tests/hunted.bin`. A split that many runs share
becomes its own item, above this one.

Found so far, by build, seed, character and setup:

- v0.107.1, IRONCLAD C000001040 at Scarcity: a shuffle that asks for a
  card inside a draw that cannot pause panics.
- v0.107.1, IRONCLAD C000001047 with All Star: a duplicate of a Strike
  the fight no longer holds panics.
- v0.107.1, SILENT C000001076 with Big Game Hunter: the relics part
  after a card selection is answered.
- v0.107.1, REGENT C000001056 with Insanity: the question parts after
  Divine is answered.
- v0.107.1, IRONCLAD C000001140 with Insanity: the game logs `Tried to pop
  action GenericHookGameAction` at a reward, and `sts2-headless` refuses
  the reply's `gameErrors` on a pending answer.
- v0.107.1, NECROBINDER C000000407: a turn's end against The Obscura,
  at the run's last step.
- v0.111.0, IRONCLAD C000000626 at Double Boss: the creatures part after
  the travel to the second boss, at step 799.

### 2. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

