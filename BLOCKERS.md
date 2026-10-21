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

Two more checks say when that is enough:

- `cargo xtask hunt --mixed` plays 3,000 runs in a row on each build with
  no split. Then fewer than one run in 1,000 splits, with 95% confidence.
- `cargo xtask reached` shows no rules code that play can reach and no
  run reaches. A scenario covers each gap, or the gap is noted as
  unreachable.
- The game's own rules code shows no method that play can reach and no
  hunt enters. This finds what the simulator never ported, which
  `reached` cannot see.

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
`crates/sts2-golden/tests/hunted.bin`. A v0.111.0 run goes in
`hunted-v0.111.0.bin`. A split that many runs share becomes its own item, above this
one.

A mixed hunt of 7,000 runs on v0.111.0 parted 258. Of those, 15 are still open:

- A Poison stack a death took off, spent. 1 run.
- A shuffle's question outside a draw, with listeners behind it. 1 run.
- A block gain's decimal, which `CreatureCmd.GainBlock` hands back
  before `GainBlockInternal` floors it. Dodge and Roll applies Block Next
  Turn of 0.75, and the power stands at 0. The simulator works in whole
  numbers. This needs Mark's decision on how a fraction reaches a power.
  1 run.
- 12 runs that each part in their own place, in fights, card plays,
  travel and potions.

Clear these before the next hunt.

### 2. Game logic that runs only in a screen

Some of the game's rules live in `Nodes.*` screen code, which never runs
headless. Throwing a Foul Potion at a merchant, and the Fake Merchant
fight it starts, are two. The oracle and the simulator agree to leave
them out, so no hunt finds them. Sweep the game for such logic, make
the mod offer each as a move, and port it.

### 3. Coverage of the game's own code

The mod counts which methods of the game's rules code a hunt enters:
models, commands, hooks and combat, not screens. Each build keeps a
total, and a `cargo xtask` subcommand lists the methods no hunt
entered. Counting is per method at first. Each gap gets a scenario, a
hunt that starts from a built position, or a note that play cannot
reach it.

### 4. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

