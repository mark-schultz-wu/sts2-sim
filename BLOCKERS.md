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

`cargo xtask game-trace <index> --file <parted file> <pattern>...`
prints the game's own calls in the moves before a kept run's split.

A mixed hunt of 7,000 runs on v0.111.0 parted 258, and all 258 are
fixed. A fresh mixed hunt of 3,000 on v0.111.0, from seed 10000, parted
9. Six are splits and three are panics, two of them unported paths. Four
are Regent runs and two are ALL_STAR. Fix those, then hunt both builds
again toward 3,000 runs in a row with no split.

### 2. Game logic that runs only in a screen

Some of the game's rules live in `Nodes.*` screen code, which never runs
headless. The oracle and the simulator agree to leave them out, so no
hunt finds them. A sweep of the screens found these, each to become a
move the mod offers and the simulator ports:

- Discarding a potion, which `NPotionPopup` enqueues as
  `DiscardPotionGameAction`, in a fight or out. Mark chose to list it as
  a legal move and digest it, so every stored recorded and hunted run
  is recorded again with it.
- Backing out of a selection that can be cancelled: Smith, Cook, the
  shop's card removal and the card-removal reward.
- Throwing a Foul Potion at the shop's merchant, for 100 gold.
- Throwing a Foul Potion at the Fake Merchant, which starts its fight.
- An empty chest, which the game closes by itself and the mod turns into
  a skip.

### 3. Coverage of the game's own code

`cargo xtask hunt --coverage` has the mod count which methods of the
game's rules code each run enters: models, commands, hooks and combat,
not screens. `cargo xtask game-coverage` adds the counts up by area and
lists the methods no run entered. Twenty runs entered 7,023 of 15,756
methods. Counting makes a hunt about four times slower.

Each gap gets a scenario, a hunt that starts from a built position, or a
note that play cannot reach it.

### 4. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

