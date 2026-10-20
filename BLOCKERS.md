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

### 1. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. `cargo xtask split <index> <name>` writes a
run, up to its first split, as a scenario, which names what parts. The
live `unimplemented!()` branches surface this way.

### 2. v0.111.0's own rules

Every rule is in `sts2_content::behaviour`, generic over `V: Build`,
and v0.107.1's `Rules` calls it. v0.111.0's `Rules` uses the same
behaviour. A rule whose game code changed between the builds gets a
port of its own, chosen by the build. Content new in v0.111.0 is
ported there. Dump and record with the mod built for v0.111.0, and its
recorded runs are the measure.

What the shared behaviour asks of a build:

- `BuildContent`: the content traits in `sts2_content::ids`, the shared
  vocabulary types, and `Lists`, which a `lists!` macro beside each rule
  writes for the build.
- `Tables`: the lookups into a row that names content.
- `OwnRules`: the build's generated move and play scripts, and its own
  ports of rules that name content another build lacks. v0.107.1's own
  are Diamond Diadem's and Outbreak's powers, Scare, and Expect a Fight.
  The Amalgamator, Battleworn Dummy and Tinker Time events are its own
  too. A shared dispatcher asks the build first and answers
  itself when the hook passes.
- A shared module that waits on a question resumes at its own `Resume`,
  which the build's wraps through `From`.

`cargo xtask generic <file>` rewrites a build's rule to be generic, for
a port that starts from the other build's code. A move can shift what
the compiler inlines. The relics' and the cards' decision dispatchers
stay out of line, since the walk inlining them cost 3.9% and 3.4%.

### 3. A walk that shares only who listens

The snapshot copies each walk's listeners. Most walks see none arrive or
leave: 1.3% of `AfterCardPlayed` walks do, and 28% of `AfterSideTurnEnd`
ones. Sharing the fight's containers behind `Arc` cost 2.6% a step before
any walk used them, and listeners write hit points and amounts mid-walk.
A cheaper walk shares only which listeners there are and their order,
kept apart from what they hold. That is part of the flat-state pass,
which needs a design first.

### 4. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

