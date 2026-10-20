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
its split, as a scenario. A fix lands with its run in the recorded file,
and a split that many runs share becomes its own item, above this one.

The first hunts found these:

- Lead Paperweight obtained outside Neow's blessing panics. The fix, and
  its 15 runs, are on the parked branch `agent/fresh-runs`.
- The Waterfall Giant's Pressure Gun raises its damage after the hit
  that kills the player. The intent shows the raised damage in the
  game, and the simulator does not read it again. Two runs on
  `agent/fresh-runs`.
- A turn's end against The Obscura, at the run's last step. One run on
  `agent/fresh-runs`, not yet diffed.
- v0.111.0 at Double Boss parts in Creatures after the travel to the
  second boss: IRONCLAD C000000626, step 799.

### 2. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

