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

### 1. Pause boundaries in hooks

The game runs some calls under a pause boundary,
`AssignTaskAndWaitForPauseOrCompletion`. A choice signalled under it
pauses the task there, and the caller runs on to a join that waits for
the task. The simulator sets a paused task aside with `Live::set_aside`
and puts it back with `Live::rejoin`. A turn's setup does this, and the
join is `RunAutoPrePlayPhase`.

Each listener of these hooks has a boundary of its own:
`BeforeSideTurnStart`, `BeforeSideTurnEnd` and its two early passes,
`AfterSideTurnEnd` and its late pass, `BeforeFlush` and its late pass, and
`AfterDiedToDoom`. A listener that pauses lets the later listeners and
passes run first. The hook waits for every task at its end. The
simulator stops the hook on the first question, and the listeners after
it run late or not at all. `before_flush_late` drops them. In v0.111.0, a
choice with nothing to ask signals as well, and the simulator does not
stop on it.

`OrbQueue.AfterTurnStart` has a boundary with no join, as `AfterDeath`
has. Its task goes on as a queued action, which `Live::paused` holds for
`AfterDeath`.

### 2. Pause sites from the game's code

A pause point the simulator does not know about shows up only when a hunt
reaches it. A tool in `xtask` lists, from the game's own code, each
method that can reach `SignalPlayerChoiceBegun` or a question. Then it
checks that the simulator's matching code has a resume point there.

### 3. The splits a hunt finds

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
fixed. Then both builds were hunted again with 3,000 mixed runs each,
from seed 10000.

- v0.107.1 parted 8, and all 8 are fixed.
- One v0.107.1 run, IRONCLAD C000010510 with DoubleBoss, stopped on a
  game error at step 1304. The game tried to pop Stampede Power off a
  hook's choice context while Breakthrough was on top of it.
- v0.111.0 parted 9. Four are fixed. Five are left: two splits and three
  panics.

Fix those, then hunt both builds again toward 3,000 runs in a row with no
split.

### 4. Coverage of the game's own code

`cargo xtask hunt --coverage` has the mod count which methods of the
game's rules code each run enters: models, commands, hooks and combat,
not screens. `cargo xtask game-coverage` adds the counts up by area and
lists the methods no run entered. Twenty runs entered 7,023 of 15,756
methods. Counting makes a hunt about four times slower.

Each gap gets a scenario, a hunt that starts from a built position, or a
note that play cannot reach it.

### 5. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

