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

### 2. v0.111.0 is a `Build`

Every rule is in `sts2_content::behaviour`, generic over `V: Build`.
`Hooks` and `Rules` are generic functions in `behaviour::rules`, which
the `rules!` macro in `build.rs` forwards a build's methods to. A rule
that is only a table read forwards to the build's own tables inside the
macro. v0.111.0 implements what the shared behaviour asks of a build and
calls `rules!`, so its `Rules` is the shared one:

- `BuildContent`: the content traits in `sts2_content::ids`, the shared
  vocabulary types, and `Lists`, which a `lists!` macro beside each rule
  writes for the build.
- `Tables`: the lookups into a row that names content.
- `OwnRules`: the build's generated move and play scripts, and its own
  ports of rules that name content another build lacks. v0.107.1's own
  are Diamond Diadem's and Outbreak's powers, Scare, and Expect a Fight.
  The Amalgamator, Battleworn Dummy and Tinker Time events are its own
  too. A shared dispatcher asks the build first and answers
  itself when the hook passes. v0.111.0 starts from v0.107.1's.
- The pinned state types, and `Resume = behaviour::Resume<Self>`, which
  wraps each shared module's own `Resume`.

`cargo xtask generic <file>` rewrites a build's rule to be generic, for
a port that starts from the other build's code. A move can shift what
the compiler inlines. The relics' and the cards' decision dispatchers
stay out of line, since the walk inlining them cost 3.9% and 3.4%.

### 3. Recorded v0.111.0 runs

Dump the build and record its runs with the mod built for it. Deploy
with `STS2_GAME_VERSION=v0.111.0`, and redeploy v0.107.1 after. Its
recorded runs are the measure for the next item.

### 4. v0.111.0's changed rules

A rule whose game code changed between the builds gets a port of its
own, chosen by the build through `OwnRules`. Content new in v0.111.0 is
ported there. The recorded runs from item 3 show which rules split.

### 5. A walk that shares only who listens

The snapshot copies each walk's listeners. Most walks see none arrive or
leave: 1.3% of `AfterCardPlayed` walks do, and 28% of `AfterSideTurnEnd`
ones. Sharing the fight's containers behind `Arc` cost 2.6% a step before
any walk used them, and listeners write hit points and amounts mid-walk.
A cheaper walk shares only which listeners there are and their order,
kept apart from what they hold. That is part of the flat-state pass,
which needs a design first.

### 6. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

