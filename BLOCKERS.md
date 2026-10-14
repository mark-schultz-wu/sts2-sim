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

### 1. A power the walk reaches after it came off

Every event hook that carries anything out walks with the snapshot
cursor, `listen::Snapshot`, begun through `listen::Start`. The eager
`notify` stays only where its callers check that nobody answers, and at
`ModifyShuffleOrder`, which the game asks before the cards move.

The walk also still asks a power an earlier listener took off, as a model
on its own. Nemesis takes Intangible off as the enemies' turn ends, and
the game then decrements the Intangible it took off. The snapshot has to
keep that model, and keep it writable. No recorded run parts on it yet.

### 2. Every other split the recorded runs find

`cargo xtask coverage` lists the commonest first splits, by the move
that led to each. Each one with more than a handful of runs becomes its
own item, above this one. `cargo xtask split <index> <name>` writes a
run, up to its first split, as a scenario, which names what parts. The
live `unimplemented!()` branches surface this way.

### 3. Run modifiers that change the run's own rules

- Deadly Events: `AfterRunCreated` sets the odds and takes Juzu Bracelet
  out of the relic bags.
- Cursed Run: `AfterActEntered`, which gets a `ModelHook` of its own.
- Big Game Hunter: `ModifyGeneratedMap` and card rewards.

Record 100 runs with each.

### 4. Run modifiers in a fight

Murderous and Terminal. `CombatState` lists the modifiers last among its
listeners, after the cards, so they join the fight's walk there. They
answer `AfterRoomEntered` and `AfterCreatureAddedToCombat`. This waits
for item 1. Record 100 runs with each.

### 5. Run modifiers that carry data

Hoarder saves the cards it copied, so it needs state of its own per run.
Character Cards names a character, which the start command, the run and
the recorded-run format have to carry. Mark revisits both after item 1.

### 6. Run modifiers at Neow

All Star, Specialized, Draft, Sealed Deck and Insanity. Neow offers each
modifier's option in turn, and three of them empty the starting deck
first. Draft and Sealed Deck build that deck through repeated picks. Mark
revisits the design after item 1. Record 100 runs with each.

### 7. v0.111.0

Only the generated tables exist. Codegen runs per build as it does now.
The behaviour is written once, for v0.107.1. A v0.111.0 rule whose game
code did not change calls that one. The v0.107.1 rule first becomes
generic over the build. A rule that changed gets one of its own. Then record its runs.

### 8. A walk that shares only who listens

The snapshot copies each walk's listeners. Most walks see none arrive or
leave: 1.3% of `AfterCardPlayed` walks do, and 28% of `AfterSideTurnEnd`
ones. Sharing the fight's containers behind `Arc` cost 2.6% a step before
any walk used them, and listeners write hit points and amounts mid-walk.
A cheaper walk shares only which listeners there are and their order,
kept apart from what they hold. That is part of the flat-state pass,
which needs a design first.

### 9. A speed target

Mark sets the target. It is measured by `cargo xtask instructions`,
per replayed step.

