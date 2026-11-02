# What sts2_headless has to do

> **This document and the code it describes were written by Claude, not by a
> human, and have not been reviewed line by line.** It is a working record, not
> a specification anyone signed off. Claims marked *measured*, *built* or
> *mutation-tested* were checked by running something. Claims marked *resolved*,
> or argued from the decompiled source, were not.

Each requirement is written so it can be failed. Where something is not yet
measured or not yet known, it says so rather than assuming.

**`D` is determinism, `H` is headless.** These were one sequence, R1–R14, when
this was one mod. The split renumbered them, and rewrote all 146 references in
the source of four mods and the Rust client. The prefix means a reference names
its own file, so `H6` never has to be looked up to find out where it lives.

The simulator is a Rust reimplementation of Slay the Spire 2. It is checked by
putting the real game and the simulator in the same state, feeding both the same
decisions, and comparing after every one. This document is the standard the
driver is held to. If the driver satisfies everything here, a simulator that
agrees with it is correct in the ways we can check. One that disagrees has a
real bug.

Reproducibility is not in this file. [`sts2_determinism`](../sts2_determinism/REQUIREMENTS.md)
owns D1, D2 and D3, and this mod refuses to start without it.

## The requirements, and where they live

| | requirement | file |
|---|---|---|
| **D1** | Identical inputs produce identical outputs | [determinism](../sts2_determinism/REQUIREMENTS.md) |
| **D2** | Refuses to answer for the wrong build | [determinism](../sts2_determinism/REQUIREMENTS.md) |
| **D3** | Never writes to the player's own saves | [determinism](../sts2_determinism/REQUIREMENTS.md) |
| **H1** | A run cannot start without an explicit seed | **here** |
| **H2** | The legal command set is reported | **here** |
| **H3** | One command advances exactly one decision | **here** |
| **H4** | The observation is complete | **here** |
| **H5** | Divergence is detectable out of combat too | **here** |
| **H6** | Scenarios can be constructed, not only replayed | **here** |
| **H7** | Throughput is measured and reported | **here** |
| **H8** | The process boundary follows the measurement | **here** |
| **H9** | The protocol is machine-readable and versioned | **here** |
| **H10** | Failure is loud | **here** |
| **H11** | The driver ends its own process | **here** |
| **H12** | A process that hangs still dies | **here** |

---

## The shape of the thing

One process runs the real game, headless, with three mods loaded.
[`sts2_determinism`](../sts2_determinism/README.md) makes it reproducible. This
one drives and observes it. The build installs a test mod for each only when its suite runs.

It speaks a line-oriented protocol. Given a scenario, it constructs the state
and reports an observation. Thereafter the simulator sends one decision, and the
oracle replies with the next observation.

It is spoken to over a loopback TCP connection, from behind an interface that
keeps the choice reversible. See H9. Whether the simulator prefers to host it
in-process is still open (H8).

Three capabilities carry it:

- **construct** a scenario (H6). Mechanisms identified. Fresh runs and run
  modifiers built.
- **advance** by exactly one decision (H3). **Working**, at 132/sec in-process
  and 63/sec over the protocol.
- **observe** completely (H4, H2, H5). **Working in combat, on the map, and in
  every room type**, byte-reproducible across processes through the protocol.
  Run-end and orbs are the remaining gaps.

---
---

## H1 — A run cannot start without an explicit seed

The tool refuses rather than inventing one.

*Confirmed trap.* `RunState.CreateForTest(..., string? seed = null)` calls
`SeedHelper.GetRandomSeed()` when seed is null. That defaults to `Rng.Chaotic`,
which is wall-clock. The convenient constructor is the nondeterministic one, and
it fails silently. Prefer `RunState.CreateForNewRun(..., string seed)`, where
the seed is non-optional. `CreateForTest` also defaults its player to
`Deprived`, whose `IsPlayable` is false. That is a second reason not to lean on
it.

*Check.* Start a run with no seed. It must exit non-zero with a clear message,
rather than begin playing.

---

## H2 — The legal command set is reported, in and out of combat

At every decision point the oracle states exactly which commands the game
accepts. Which cards are playable, and against which targets. Which potions,
which map nodes, and which reward, shop and campfire choices. Which cards can be
selected in a selection prompt.

*Why this is the most valuable requirement.* Generating legal moves is where a
reimplementation goes wrong quietly. Two implementations can agree on the result
of every move that was tried, while disagreeing on which moves exist. Comparing
state after a command only catches the first kind of bug. Comparing the legal
set catches the second.

*Check, and a correction to it.* Every command the oracle enumerates must
execute without error. But the stated converse, that commands outside the set
are rejected, **does not work for map travel**.
`MapTravel.GetTravelablePointsFrom` has exactly one caller, `NMapScreen`, and
`RunManager.EnterMapCoord` enters any unvisited coordinate handed to it. So the
model layer never rejects an illegal map move, and the complement fuzz passes
vacuously. Map legality lives in the UI. There is nothing underneath to say
no.

Map travel is the exception, not the rule. Rest sites, events and shops all
carry their legality on the model, through `IsEnabled`, `IsLocked` and
`EnoughGold`. So the converse check does work for them.

What does work is metamorphic, and it generalises: **the game's own `override`
sites enumerate every place a rule varies.** For each, construct a state where
the override is active and require the oracle's output to *change*. An oracle
that ignores the hook reports the baseline twice and fails. The matrix is
mechanically derivable: 2 overrides of `ShouldAllowFreeTravel`, 7 of
`ShouldPlay`, and 55 of `IsAllowed`.

*Built:* `SelfTest.cs`, `STS2_HEADLESS_MODE=selftest`.

**The matrix is discovered, not written down.** Reflection over the game
assembly finds every non-abstract `AbstractModel` subclass declaring an override
of a legality hook. An override here is a method whose `GetBaseDefinition` is
declared elsewhere. It currently reports:

```
ShouldAllowFreeTravel: WingedBoots, Flight
ShouldPlay:            VelvetChoker, ChainsOfBindingPower, RingingPower,
                       SlothPower, SmoggyPower, Enthralled, Normality
9 legality-hook overrides across 2 hooks
```

matching a hand grep exactly. A hook added in a future build appears as
UNCOVERED rather than silently going untested. *Noticing* a case exists is
automatic. *Constructing* one is manual. **All nine are covered**, in 28
assertions in 3.9s: 25 hook assertions plus the boss-row map case.

| case | trigger | check |
|---|---|---|
| `WingedBoots` | hold the relic | travelable set widens, 1 → 3 nodes |
| *(boss row)* | stand on the last row, then hold the relic | the boss stays the one legal move |
| `Flight` | restart the run holding the modifier | travelable set widens, 1 → 3 nodes |
| `Enthralled` | card in hand | playable 5 → 1, reason `BlockedByHook` |
| `SmoggyPower` | power + `Smog` on a card | that card blocked |
| `ChainsOfBindingPower` | power + `Bound` on two, play one | the other blocked |
| `RingingPower` | power, then any play | afflicted cards blocked |
| `SlothPower` | power, then 1 play | next card blocked |
| `Normality` | card in hand, then 3 plays | next card blocked |
| `VelvetChoker` | hold the relic, then 6 plays | next card blocked |

The last three share one mechanism, a count of cards played this turn. So they
share one case, parameterised by the threshold, and by where the rule lives: a
power, a card, or a relic. Reaching six plays needs granted energy *and* a
re-drawn hand. The first attempt stalled at five with 27 energy spare, because
the binding limit was cards, not cost.

`Flight` is the one case that cannot be switched on mid-run. Modifiers are
installed only by `RunManager.InitializeNewRun`. So its metamorphic pair is two
runs at one seed, rather than two moments in one run. That also makes it the
only exercise H8's multi-scenario claim gets. `RunState.AddModifierDebug` avoids
the restart, and was rejected. It appends to the list without calling
`OnRunCreated`, which leaves the modifier's `RunState` unset and its
`ClearsPlayerDeck` pass skipped. `Flight` happens to survive that. Relying on
the happenstance makes the case a bad template for modifiers that do not.

### Four things the suite got wrong before the oracle did

Worth recording, because each is a way a green suite can be worthless.

**Order dependence.** All in-combat cases first shared one fight. `Enthralled`
left its card in hand, which forbids playing anything else. So later cases
failed for reasons unrelated to the rule under test. Each case now enters its
own fight. Order-dependent tests are worse than missing ones. They fail as
though the product were broken.

**Assuming a shared shape.** `Smoggy`, `Ringing` and `Chains of Binding` look
alike, but they bite at different times. They bite on presence, after any play,
and after a *bound* play respectively. Asserting Smoggy's behaviour of all three
produced two false failures.

**An unsatisfiable scenario.** The Ringing case tried to play a card without the
affliction. A diagnostic showed the power afflicts the entire hand on
application, so no such card existed. The scenario was impossible, not the
rule.

**An uncontrolled comparison.** The `Flight` case compares two runs rather than
two moments, so a difference in the *map* reads as a difference in the *rule*.
The case then "proves" free travel widens the map, when all the restart did was
generate a different one. It now fingerprints the whole map,
edges included, and refuses to compare unless the two agree.

In all four the suite was wrong and the oracle right. That is the expected ratio
early on, and the reason each failure was diagnosed rather than assumed to be a
product bug.

We deliberately exclude `IsAllowed`, despite its 55 overrides. It decides
whether a relic can appear in a run at all, not what the legal command set is.
So there is no observation field for it to move.

The suite is mutation-tested. It is known to fail when the oracle is wrong, not
merely to pass when it is right:

| mutation | result |
|---|---|
| `MapTravel` → `current.Children` | FAIL ×2, `WingedBoots` and `Flight`, both 1 → 1 nodes |
| `MapLegality` → `MapTravel` alone, no boss row | FAIL, boss case, 1 → 0 nodes |
| `CanPlay` → energy affordability only | FAIL, 5 → 6 playable |
| `Restart` uses a different seed per scenario | FAIL, same-map guard, before the widening is compared |

The last one is a mutation of the *test*, not the oracle. It confirms that the
`Flight` case's control does real work.

The suite reads the travelable set and the legal set back out of the emitted
observation, rather than calling `MapTravel` or `CanPlay` again. A test that
re-derives the answer the same way the reporter did agrees with a broken
reporter.

*Two things it taught immediately.* Run at floor 1, it failed. The oracle was
not wrong. The next row held exactly that node's two children. Free travel and
normal travel then coincide, and the scenario had no way to tell them apart. **A
scenario that cannot discriminate is not evidence.** So the test now searches
for a position where the two rules differ, and fails loudly if none exists.

It is also mutation-tested. Reverting the oracle to `current.Children` makes it
fail (`1 -> 1 nodes`), and restoring `MapTravel` makes it pass (`1 -> 3`). So
the test is known to catch the bug it was written for.

*Resolved for combat: the game exposes it on the model, not the UI.*

`CardModel` has a `TargetType` property and three checks. `CanPlay` says
whether the card can be played, and hands back an `UnplayableReason` and the
model that prevents it. `CanPlayTargeting` and `IsValidTarget` take a
creature.

`CanPlay` covers the unplayable keyword, energy and star cost
(`HasEnoughResourcesFor`), no-living-allies, hook blocks, and card logic.
`IsValidTarget` covers side and aliveness against the ten-case `TargetType`.
So in-combat legality is *inherited* from the game rather than reimplemented by
us, which is what this requirement wanted.

*Bonus worth exploiting.* `UnplayableReason` is a `[Flags]` enum, so the oracle
can report **why** a card is illegal, not just that it is. Comparing reasons
rather than booleans localises a simulator bug to the specific rule it got
wrong. Otherwise we are left to bisect.

*Map legality is also on the model, and reading the graph is wrong.*

`GetTravelablePointsFrom` gives the whole next row when
`Hook.ShouldAllowFreeTravel` allows free travel. Otherwise it gives the
current point's children.

The obvious derivation is "the current node's children". It is correct until the
first run holding `Winged Boots` or the `Flight` modifier. Both override
`ShouldAllowFreeTravel`, and widen the legal set to the whole next row. It is
then silently wrong, in a way no state comparison catches. That is the exact
failure mode this requirement exists to prevent. The oracle calls
`MapTravel.GetTravelablePointsFrom`.

*This is not a hypothetical, and the evidence is external.* `sts2-cli` is the
most developed headless STS2 harness in existence, and it has exactly this bug.
`MapSelectDecision` builds its choices from `currentPoint.Children`
(`RunSimulator.cs:2022`), and the repository contains zero references to
`MapTravel`, `ShouldAllowFreeTravel`, `WingedBoots` or `Flight`. Its combat
legality is right, because it uses `CardModel.CanPlay`. So the defect is
specifically the rule that lives one call deeper than the obvious graph walk.

*And `MapTravel` is not the whole rule either.* The oracle now delegates to
`MapLegality`, a mirror of `NMapScreen.RecalculateTravelability`. The boss lives
**outside the grid**. `BossMapPoint` is at row `GetRowCount()`, and
`StartingMapPoint` and `SecondBossMapPoint` are likewise absent from `Grid`, so
`GetAllMapPoints` and `GetPointsInRow` never yield them. The order of the
screen's checks is load-bearing. The boss row is tested *before* free travel is
consulted.

The two rules compose into a bug neither has alone. Stand on the last grid row
with free travel active. `MapTravel` returns `GetPointsInRow(rowCount)`, which
is off the end of the grid, so empty. The fallback to `Children`, which does
contain the boss, is skipped precisely because free travel applies. The oracle
reported **no legal move at all** on the floor before the boss, for any run
holding `Winged Boots` or `Flight`. It is mutation-tested. Reverting
`MapLegality` to `MapTravel` alone gives `before: [3,16] after: []`.

*Travel is a command.* `RunManager.EnterMapCoord(MapCoord)` is public and does
the whole step. The oracle checks legality first, and that check is **not**
redundant. `EnterMapCoord` refuses only coords already visited. Otherwise it
enters any coordinate handed to it, and does so silently. It is the one command
where the model layer lets an illegal move through. That is the concrete reason
H2 requires the legal set to be published, rather than a rejection to be relied
on.

### Rewards, and what the UI owns

*Built.* A won fight now offers its rewards as a pending decision, and taking
them lands, at `gold 99 → 117, deck 10 → 11`. Skipping takes nothing. Taking the
gold and leaving the card moves gold only. A card reward answered without a
`card` index is refused rather than defaulted. Choosing is the one thing the
oracle must not do.

*The problem it had to solve was structural.* Before this, a won fight granted
nothing. Gold stayed 99, and the deck stayed at 10. Rewards were not silently
auto-taken either. The step never happened.

`CombatRoom.OfferRoomEndRewards()` is public and does the work. But on the live
path its only caller is `NCombatUi.OnCombatWon`. The trigger is in the UI, not
the model. This is worse than the map case. There the *rule* lived in a screen
while the model still owned the step. Here the **step itself** is UI-owned, so
the oracle has to supply it.

Two consequences before any of this can be built:

1. **`TestMode` changes the semantics.** `RewardsSet.Offer` branches on
   `TestMode.IsOn`, which the oracle sets. With `testSelector` null it selects
   *every* reward, and throws if the set is left incomplete. So installing
   `RewardsSet.testSelector` does not add a decision point. It takes one away
   from the game and gives it to the client. `ThrowInTestIfRewardsNotTaken` is
   also turned off, because a real player can leave a rewards screen having
   taken some and not others.
2. **The protocol needs pending decisions.** A reward offer arrives *inside* the
   action cascade, during quiescence, not in response to a command. So H9's
   "one request, one response" needs a notion of the game asking a question.

*Designed. See H9, "Pending decisions".* The investigation that produced it also
corrected the assumption above. Rewards are the **only** UI-owned trigger. Rest
sites, events and shops all begin from `EnterInternal` on the model side, so
they are ordinary observable states reached by `travel`, not pending decisions.
What needs the new machinery is the three blocking callbacks. The first of them,
`GetSelectedCards`, fires inside a single `play_card` for any card that discards
or exhausts, so it is a combat concern too.

*`OfferRoomEndRewards` does behave when driven by us*, which was the open risk.
It is fired from a `CombatManager.CombatWon` subscription, once per process.
`CombatManager` outlives `CleanUp`, so re-subscribing per run offers every set
as many times as the process started runs.

**A race worth recording, because it is the third of its kind.** Rewards are
applied *after* combat ends, and the readiness test treats "not in combat" as
ready. So the wait returned the moment the answer was accepted, and the client
saw its own rewards not applied. The first fix put the busy flag inside the
applier, which was still wrong. The continuation resumes asynchronously, so
there is a window between accepting the answer and the applier starting, in
which the run looks idle. The flag has to be set where the answer is
*accepted*.

Together with H3's turn-transition bug and the map's boss row, all three of this
project's hardest bugs have one shape. **A state that is legitimately reachable,
briefly, and indistinguishable from the state that the code wants.**

### Rest sites, events and shops

*Built,* and they needed no new protocol machinery, as designed. Each is an
observable room state. The observation grows a `room` block, and the client
answers with an ordinary command. That is `choose_option {index}` for rest sites
and events, and `buy {index}` for shops. Both rooms use
`ChooseLocalOption(int)` on their synchronizer, disambiguated by room type.

| exercised | result |
|---|---|
| rest site `HEAL` at 0/80 hp | 0 → 24, and the remaining options clear |
| event `AROMA_OF_CHAOS` / `LET_GO` | parks a nested `select_cards`, answered, resolved |
| shop buy `STAMPEDE` @36 | gold 99 → 63, deck 10 → 11 |
| shop buy card removal @75 | parks a `select_cards`, answered, deck 10 → 9 |
| bad index, disabled option, locked option | refused, room unchanged |
| unaffordable purchase | refused, naming cost and gold |
| `choose_option` in a shop | refused |

`enter_room {type}` reaches any of them directly (H6). Otherwise travelling to a
shop means hunting a seed that puts one on floor 2, and that is the seed-hunting
this project exists to avoid.

**The event case is the one that validates the design.** `LET_GO` leads to a
card selection, so an event option parks a `select_cards` *inside* itself. The
first attempt awaited `EventSynchronizer.AwaitPendingOptionTasks()` directly,
and **deadlocked**. The handler stopped inside the very work that had
to ask the client a question it had no way left to send. Registering the work
with `BackgroundWork` instead, tracked and never awaited, lets the wait loop
exit on the pending. The run still does not count as quiescent.

That was the fourth instance of one bug, so the flags are now a single counter
rather than one more each time. The shape, stated once. **Out of combat, every
readiness test the driver has is trivially true, so work started outside the
action queue is invisible to it.**

Three more came since, all the same shape, and all found by driving
the game rather than by reading it. Those are `choose_option` on `SMITH`, `grant`
of a relic whose `AfterObtained` parks (Royal Stamp), and `Chest.Pick` of one of
the ten enchant-on-obtain relics. The fourth is `buy` of a shop's card removal
entry, from the fuzzer at move 72 of a random run. `OnTryPurchaseWrapper` awaits
`DoMerchantCardRemoval`, which parks a card selection.

The rule, since the count keeps going up: **a handler never awaits work that can
park a question.** It tracks the work and waits at the decision point. What that
costs is the refusal the awaited call returns. So every refusal a handler can
give has to be decided in front of the work rather than read off it. That is what
`buy` now does with `IsStocked`, `EnoughGold` and `Observation.Blocked`, the same
three the observation already reports.

*Run-end, found here and now handled.* A driven run that lost kept accepting
commands, and a dead player was observed healing at a rest site. The real game
cannot reach that state. Every observation after that point described something
no simulator can be asked to reproduce. A finished run now refuses anything that
advances it, naming victory or defeat. `observe`, `start` and `quit` still
work.

The two are not the same question. `RunManager.WinRun` calls
`GuaranteeKillAllPlayers`, so `IsGameOver` is true for a win as well as a loss.
`CurrentRoom.IsVictoryRoom` is what separates them. The observation reports
`alive`, `runOver` and `victory` (the last only after the run ends).

---

## H3 — One command advances the state exactly one decision

The oracle issues one command and returns when the game is quiescent and waiting
for the next player decision. It never chooses on our behalf and never advances
past a decision point.

*Note on granularity.* A single player decision cascades into many
`GameAction`s. Playing a card enqueues damage, triggers and draws. The unit of
the protocol is the **player decision**, not the `GameAction`. The action stream
underneath is reported (H4), but it is not the command boundary.

*Resolved: quiescence is three phases, and an empty queue alone is wrong.*
`ActionExecutor.IsRunning` is false and `FinishedExecutingActions()` returns
`Task.CompletedTask` **both** when the queue drains and when it did not yet
pick our action up. Waiting on those alone returns immediately, and lets the
caller run ahead of the game. The first driver did exactly that. It reported 300
decisions in 0.07s while only five actions had actually executed.

So:

1. `await issued.CompletionTask`. Every `GameAction` carries its own completion
   source, which cannot be satisfied early.
2. Then drain the cascade it triggered, until `ActionQueueSet.IsEmpty` and
   `!ActionExecutor.IsRunning`.
3. Then wait until the game wants input again. That means `!PlayerActionsDisabled`
   and `PlayerTurnPhase.Play`. For a turn-ending decision it also means a turn
   number strictly greater than the one being left.

*Phase three was missed for a while, and the protocol is what exposed it.*
`EndPlayerTurnAction.ExecuteAction` calls `PlayerCmd.EndTurn` and returns
`Task.CompletedTask` immediately. So the action reports itself finished while
the turn transition has not started, meaning the enemy turn and the next draw.
The queue is genuinely empty in that gap. Phases 1 and 2 both pass, and the
oracle reports a state the game is about to leave.

The in-process driver never noticed. It re-read the live model on the next
iteration, and picked up wherever the game got to. Over the protocol the
observation is authoritative, and the failure became loud immediately. A
client acting on the returned observation was found a full turn behind the
oracle. It believed turn 2 while the game was on turn 3, with no command in
between. It then addressed a card by an index that no longer meant what it
meant before, and the oracle rejected it.

**A stale observation is worse than a missing one**, and this is the sharpest
illustration of H10 so far. Nothing errored, nothing was defaulted, and the
oracle's answer was out of date. Waiting on a settling delay papers over
it. The fix uses the game's own readiness signals, and for turn ends a monotonic
turn counter, which no transient can imitate.

*And a cancelled action never completes at all.* When combat ends mid-cascade,
the executor stops with actions still queued. The decision we issued is
abandoned, not finished. It was observed as `EndPlayerTurnAction turn 5
not completed: running=False current=<null> empty=False combat=False`. Waiting
on `CompletionTask` alone hangs forever. The driver must treat the fight ending
as a legitimate reason for a decision never to resolve. Every wait needs a bound
rather than trusting the task.

*Check.* A decision count that runs ahead of the `AfterActionExecuted` count is
the signature of this bug. Assert that they stay consistent.

---

## H4 — The observation is complete

After each command, the observation carries the draw, hand, discard and exhaust
piles in order. It carries every creature with HP, block, statuses and intent.
It carries orbs, gold, potions, relics, floor, map position and the pending
decision. It carries **the RNG counter for every stream**.

*Why not just use the game's own state sync.* `NetFullCombatState.FromRun`
carries piles and creatures only while `CombatManager.Instance.IsInProgress`. So
out of combat it says nothing about the deck. It also deliberately removes
`PlayerRngType.Rewards` and `PlayerRngType.Shops` from what it compares. Our
observation must be a superset, not a rename of theirs.

*Why RNG counters matter separately.* Two implementations can produce the same
visible state while having consumed different numbers of random draws. That
divergence is invisible now and fatal three decisions later. Counters catch it
at the step where it happens.

*The streams, in full.* Twelve are run-level, from `RunRngType`: UpFront,
Shuffle, UnknownMapPoint, CombatCardGeneration, CombatPotionGeneration,
CombatCardSelection, CombatEnergyCosts, CombatTargets, MonsterAi, Niche,
CombatOrbs and TreasureRoomRelics. Three are per-player, from `PlayerRngType`:
Rewards, Shops and Transformations. That is fifteen counters per
observation.

*Check.* Reach one state two ways: replayed from floor 1, and constructed
directly (H6). The observations must be equal.

### Built, in `Observation.cs`

Emitted as one JSON line per decision. It carries act, floor, hp, gold, relics,
potions and deck. In combat it also carries turn, phase, energy and round. It
carries all four piles in order, every creature with hp, block and powers, and
the legal command set. Card identity includes upgrade level and affliction, since the id alone
cannot distinguish two decks. All fifteen RNG counters are reported, run-level
and per-player.

Two fields exist here that the game itself never ships. The first is **monster
intent**. It is absent from `NetFullCombatState`, because peers re-derive it,
and it is serialised nowhere. It is reported here as `HEADBUTT_MOVE` or
`THRASH_MOVE`, because a simulator choosing the wrong monster move is exactly
what this catches. The second is **the reason a card is illegal**, from the
`UnplayableReason` flags. A disagreement then names the rule rather than needing
a bisect. `EnergyCostTooHigh` is confirmed firing.

Absence is distinguished from emptiness. The `combat` object is omitted out of
combat, rather than serialised with empty piles. So "not in combat" and "in
combat holding nothing" cannot compare equal.

*Result.* Two cold-profile processes on one seed produced **byte-identical**
36KB observation streams, 22 observations each.

*Not yet in it:* the pending decision's identity, and map coordinates, since
only act and floor are carried. Nor orbs, because nobody drove a Defect run
yet. Nor out-of-combat legality, which is H2's remaining gap.

---

## H5 — Divergence is detectable out of combat too

`RunManager.SendPostActionChecksum` is gated on
`CombatManager.Instance.IsInProgress`, so the game's own per-action checksums
exist only inside combat. Outside combat there is only the coarse room-exit
checksum, and that covers rewards, shops, campfires, map movement, card removal
and upgrade.

The oracle supplies its own digest over the full observation so that every step
is comparable, not just combat steps.

*Built.* A SHA-256 over the full observation, truncated to 8 bytes. This is also
what makes the stream affordable. The full observation is about 1.6KB per
decision, at 27% creatures, 27% legal moves, 19% RNG counters and 11% deck. Most
of that is repeated key names for values that rarely change. At 132
decisions/sec it is 200KB/s to say "still agreeing".

*What the digest does not buy is time.* Over the protocol, digest-only and full
observations measure within 5% of each other (H7). The cost of a decision is
frame-boundary latency, not serialisation. The saving is bandwidth and log size.
That is worth having, and it is not what this section originally implied.

Digests cut a fight's stream by **99%**, at 17 bytes a step against about 1.6KB.
The original measurement, 36,410 bytes to 374, was taken over a 22-decision
fight. The same fight is 17 decisions now that H3's quiescence is correct, so
the totals moved while the per-step ratio did not.

The full observation is available on demand, with `STS2_HEADLESS_OBS=full` or per
request. That fallback costs nothing precisely **because D1 holds**, since the
diverging run replays exactly. Compacting the JSON was the alternative, and it
is worse. It saves perhaps 40%, and it makes the failure path harder to read,
which is the path that matters.

*Constraint found in `InitializeShared`.* `ChecksumTracker.IsEnabled` is set to
`!TestMode.IsOn && NetService.Type is one of {2,3,4}`. `NetGameType` is
`{None, Singleplayer, Host, Client, Replay}`, so that set is Host, Client and
Replay. Checksums are therefore off for a singleplayer service **regardless** of
`TestMode`. And `TestMode.IsOn = true`, which `sts2_determinism` sets to skip
animation waits, disables them anyway. Two independent reasons, both silent.

`IsEnabled` has a public setter, so the fix is to re-enable it after `SetUpTest`
returns. Nothing about enabling it needs a peer. `GenerateChecksum` only sends
over the wire when `NetService.Type == Client`. Otherwise it computes locally
and fires
`ChecksumGenerated(NetChecksumData, string context, NetFullCombatState)`. That
hands us the full state alongside the hash, not merely the hash.

*Check.* Perturb one card in the deck out of combat. The digest must change.
Separately, assert `ChecksumTracker.IsEnabled` is true once a run is set up. It
defaults to true at construction, and later code turns it off, so a regression
here is silent.

---

## H6 — Scenarios can be constructed, not only replayed

Start from a specified state, without playing to it. That means this deck,
these relics, this HP, this floor, this encounter. This is firm, and not
contingent on what it costs to build. H4's strongest check depends on it, and a
suite that can only reach states by replaying to them is a suite nobody adds
tests to.

*Why.* Differential testing wants targeted coverage of rare interactions.
Reaching a state by replaying a run is slow, and it breaks whenever anything
upstream of the state changes. That makes the test suite fragile in exactly the
way that discourages adding tests.

### Editing is the point

The motivating case is not "replay to an interesting state". It is "**give the
player this relic on floor 1 and test it in the first fight**". No seed-searching
for a run where the relic happens to be offered.

Two mechanisms cover it, and neither requires the game to gain a save format.

**Boundary state.** `RunState.FromSerializable(SerializableRun save)` restores
players, decks, relics, potions, acts, modifiers, visited map coords, events
seen, and map point history. Through `RunRngSet.FromSave` it also restores exact
RNG stream positions. A restored run resumes with
`RunManager.LoadIntoLatestMapCoord`, so it comes back **at the map**, not inside
a room.

It reaches room boundaries only. `SerializablePlayer` has **no combat state at
all**: no hand, no piles, no pile order, no block, no statuses, and no *current*
energy. `SerializableRoom` has no creatures, enemy HP or intents. The game does
not persist a combat in progress.

`NetFullCombatState` carries far more, but it is write-only here, and it is a
*fingerprint* rather than a state description. `CreatureState` is
`{monsterId, playerId, currentHp, maxHp, block, powers}` with no intent, and
`MonsterModel.NextMove` is `private set` and serialized nowhere. Anything two
peers can re-derive independently need not ship.

**The dev console**, below, covers the rest, including mid-combat.

*A trap waiting for scenario construction.* Content availability is decided by
the `UnlockState` handed to the run, and every pool gates on it
(`GetUnlockedRelics(UnlockState)` and friends drop epoch-locked entries). A
fresh run gets `UnlockState.all`, meaning all epochs, all encounters and
999999999 runs, so everything is available. But `RunState.FromSerializable`
rebuilds it from `SerializableUnlockState` instead. So **a constructed scenario
can silently have less content available than a fresh run**. It then differs
from the simulator without anything in the scenario saying so. Constructed
scenarios must set the unlock state explicitly, rather than inheriting whatever
the save carried.

Note also that the game's `unlock` console command is *not* the tool for this.
`UnlockCards` calls `Progress.MarkCardAsSeen`, which is compendium "seen it"
state with no effect on availability. It also writes to `progress.save`, the
persistent profile residue D1 and D3 work to avoid.

### Construction must go through the game's own paths

Injecting a relic into `SerializablePlayer.Relics` produces a state the game
never produces. Load goes `FromSerializable` → `PopulateRelics` →
`AddRelicInternal`, and **nothing calls `AfterObtained()` on that path**. Only
`RelicCmd` and `RunManager.FinalizeStartingRelics` call it.

That is right for loading a save, where the save already reflects the effect. It
is wrong for injection. `Pear` grants max HP in `AfterObtained`. `PunchDagger`,
`TriBoomerang`, `Claws`, `Byrdpip`, `LeadPaperweight`, `PhialHolster`,
`TanxsWhistle` and others add cards or otherwise mutate the run there. Inject
`Pear` by editing the relic list and you get the relic without the max HP,
silently, and every subsequent divergence is our fixture's fault.

**So a scenario grants a relic as a command, not as a field.** Generalised, the
editable part of a scenario is expressed as the game's own construction
operations wherever one exists. Raw state covers only what has no such
operation. This is
the same reachability principle as the prefix, applied at the boundary.

### The game already has that command vocabulary: its dev console

MegaCrit's own `RelicConsoleCmd` resolves this the same way. It obtains the
relic through `RelicCmd.Obtain`, not a write to `Player.Relics`. It also calls
`.ToMutable()` on the canonical model, which is what keeps `MonsterModel.Rng`
from silently handing back `Rng.Chaotic`.

**The debug gate lets us in without patching.** `NDevConsole._Ready` allows
debug commands in four cases. They are the editor, test mode, mods loaded,
and the full console turned on in the settings. We satisfy two of the
four. `NGame.IsReleaseGame()` is hardcoded `true`, which
is what made `--autoslay` dead code. This gate is different. It deliberately
accommodates mods.

**And the console logic is separable from its UI.**
`MegaCrit.Sts2.Core.DevConsole.DevConsole` is a plain class with a public
`DevConsole(bool shouldAllowDebugCommands)` and public
`ProcessCommand(string)` and `ProcessNetCommand(Player?, string)`. Both return
`CmdResult`, a success flag and message, which H10 can surface. `NDevConsole`
merely hosts one. Its `_Ready` also builds fonts, text buffers and prompt
styling we have no use for. Construct our own with `true`, and the node is out
of the picture.

The vocabulary covers boundary editing (`relic`, `card`, `upgradecard`,
`removecard`, `enchant`, `gold`, `potion`, `heal`) and navigation (`fight`,
`room`, `travel`, `act`, `event`). It also covers **mid-combat construction**
(`applypower`, `afflict`, `block`, `draw`, `damage`, `kill`, `energy`,
`stars`).

Mid-combat construction through these commands is not "paths the game never
uses". They are the game's paths, and that is what makes a bespoke combat save
format unnecessary.

**Console commands are debug tools, not oracle primitives, and some injected
nondeterminism on purpose.** `FightConsoleCmd` calls
`encounterModel.DebugRandomizeRng()`, which seeds the encounter's generator
with the seconds since the Unix epoch.

That is wall-clock, at one-second granularity. A scenario built with `fight`
reproduces for runs landing in the same second, and diverges for runs a second
apart. That presents as flakiness rather than as an obvious break.

**This paragraph used to end "so the oracle avoids that command", and that was
the wrong conclusion.** Avoiding it is what produced an `enter_fight` that
teleports into one hardcoded encounter at an unassigned map point. The real game
cannot reach that state, and meanwhile the console's whole vocabulary sat
unused. Making the game deterministic is what the mod is *for*.

`sts2_determinism` now patches `DebugRandomizeRng` to do nothing. That does not
reimplement seeding. It gets out of the way of the game's own. An unset `_rng`
is the signal for `GenerateMonstersWithSlots` to seed from
`runState.Rng.Seed + TotalFloor + hash(Id.Entry)`. That is measured, not argued.

So the console is available, and the placeholder is gone. `fight` takes an
encounter and goes through `FightConsoleCmd`, refusing unknown names with the
game's own message. That is the first command adopted from the console, and the
one this whole argument was about.

**Every command adopted must still be audited** for `Rng.Chaotic`,
`DebugRandomizeRng`, and wall-clock reads. The `Rng.Chaotic` half of that audit
is done. There are 193 uses, all presentation except `MonsterModel.Rng`. That
one silently returns the clock-seeded stream for a canonical model, and now
warns when read. Adopt them one at a time, by reading each one, not
wholesale.

*To verify at runtime.* That each command we depend on works headless, since
some can touch UI singletons. Also that `CmdResult.success` is false rather than
a silent no-op when one does not.

**Done: `start` takes a character and an ascension level.** `Player.CreateForNewRun`
has a non-generic overload taking a `CharacterModel`, so the lookup is the same
reflection-over-`ModelDb` shape used for modifiers, over `ModelDb.AllCharacters`.
Naming one that does not exist is refused, listing the five that do.

```json
{"cmd":"start","seed":"ORACLE1","character":"DEFECT","ascension":5}
```

Verified. `DEFECT` starts 75/75 with a `STRIKE_DEFECT` deck, and `SILENT` at
ascension 5 starts 56/70. Ascension is passed straight through to
`RunState.CreateForNewRun` rather than pinned.

**This closed H4's orb gap as a side effect.** Orbs were unreported because no
Defect run had ever been driven, so the fields had never been written or
checked. The observation now carries the orb queue *in order*, because the same
orbs in a different order evoke differently. Capacity is separate from count,
since empty slots are part of the state. A Defect opening reports
`{"capacity": 3, "queue": [{"id": "LIGHTNING_ORB", "passive": 3, "evoke": 8}]}`,
which is Cracked Core channelling at combat start.

*Check.* Earn a relic legitimately in one run, and grant it by scenario in
another. From the same anchor the two observations must be identical. Run this
over every relic with an `AfterObtained` override. Those are exactly the ones
where field-poking diverges.

### The oracle constructs; the simulator loads the resulting observation

Construction has effects beyond the visible state. `RelicCmd.Obtain` removes the
relic from `player.RelicGrabBag` and `runState.SharedRelicGrabBag`. That changes
which relics future draws can produce, without consuming a draw. It also calls
`AfterObtained()`, which for some relics draws RNG. A construction *script*
replayed by the simulator has to reproduce all of that. That means teaching the
simulator a dev console it has no business having.

So the split is:

1. A scenario names construction operations **for the oracle**.
2. The oracle executes them and emits a **full observation** (H4).
3. The simulator starts from that observation.

Loading happens on the simulator side. **The game needs no save-state loader**,
which is why none is specified here.

Handing the simulator the starting state does not weaken the test. The starting
state is not what is under test. The transitions are. It is in fact safer than
having both sides interpret a scenario spec independently. If the observation
format lacks a field, the simulator defaults it while the oracle holds the
real value. The comparison then diverges at step one, visibly. Under independent
interpretation both sides default it, agree, and the gap stays invisible.

The scenario format itself stays language-neutral and small, and the
scenario→game translation lives in this mod. So the game's save schema never
enters Rust, and a game update churns only C#.

### Reaching states no command can express

Commands cannot dictate ordering. `draw` draws, but does not say which cards, so
exact pile order is not directly constructible. Where that matters, a scenario
can carry an ordered **decision prefix**, replayed from the constructed
state.

Two properties make this cheap rather than fragile. Anchored at combat entry,
the prefix stays inside one fight. Every state it reaches is reachable by
construction. It also bisects, so shrinking a divergence to a minimal repro is
list-shortening.

The prefix is a sequence of **decisions**, not `GameAction`s. Some decisions we
push onto the action queue. Others the game **pulls** from us mid-action,
through `ICardSelector` in `Core.TestSupport`. It has two methods.
`GetSelectedCards` takes the options and how many to pick at least and at
most, and returns the picked cards as a task. `GetSelectedCardReward` takes a
card reward's options and its alternatives.

`CardSelectCmd.UseSelector(selector)` installs it. That call keeps a stack
and returns an `IDisposable` scope. A prefix of queued actions alone stalls the
first time a card asks the player to choose. The driver serves both push and
pull decision points from one ordered script, which is the same boundary H3
defines.

*Priority: low.* We do not expect pile order to matter for the scenarios we care
about. Build this only when a scenario needs it.

### Scenarios carry their expected digest

A scenario that includes a prefix names a state without describing it, so the
file cannot be read to learn what it produces. Each scenario therefore stores
the observation digest it is expected to reach. That makes it self-validating. A
game update that silently changes what a scenario produces shows up as a failing
scenario. It is not a mystery divergence blamed on the simulator.

*Check.* Construct a state, snapshot it, and confirm the snapshot is the state
asked for. Then take a constructed state and a replayed state that must be
identical, and confirm they produce identical observations. That is the same
check as H4.

*Unexplored seam:* `Core.TestSupport` also contains `TestRngInjector`.

---

## H7 — Throughput is measured and reported

The tool reports its own commands/sec so the number is a fact rather than an
estimate.

**Measured, single combat, greedy policy:**

| | |
|---|---|
| game load | 0.7s, once per process |
| run start to standing on the map | 1.6s from process launch |
| one full fight (17 decisions), in-process | 0.13s, **132/sec** |
| the same fight over the protocol, digests only | 0.27s, **63/sec** |
| the same fight over the protocol, full observations | 0.28s, **60/sec** |
| scenario turnaround, `CleanUp` → standing on the map again | 0.3s |

**An earlier "~250 decisions/sec" here was wrong and is withdrawn.** It was
measured against the quiescence bug H3 now records. The driver returned before
turn transitions happened. So it counted decisions issued against stale
state, at 22 rather than 17 for the same fight. It also skipped the wait that it had to time. Correcting quiescence roughly halved the number. The honest
figure is 132/sec in-process.

For comparison, the game's own `AutoSlayer` caps near ten interactions/sec. But
that is `AutoSlayConfig.pollingInterval` and `buttonClickDelay` both being
100ms. It is a property of driving the UI, not a floor on the engine.

*Where the protocol's 2× goes, and where it does not.* Digests and full
observations run within 5% of each other, so serialisation is not the cost. That
also means H5's digest saves bandwidth rather than time. The remaining 8ms per
decision is latency, not work. A loopback roundtrip is about 30µs. But our
continuations resume on `GodotSynchronizationContext`, which pumps on the frame
callback. So each externally-driven decision waits for a frame boundary that an
in-process loop does not. Raising the headless frame rate is the obvious lead,
and it is untried.

A hundred-decision combat prefix replays in about 1.6 seconds at 63 per
second. That still
supports H6 leaning on replay, though less comfortably than the withdrawn number
suggested.

Scenario turnaround is measured from the two restarts the `Flight` case performs
(H2). Each is 0.3s, against 0.7s of one-time game load. So a batch of scenarios
in one process costs roughly two fights' worth of time per scenario boundary.
That is the number H8 wanted, and it favours in-process.

*Still to measure.* A full run rather than one fight, and whether the frame-rate
lead above recovers the protocol's 2×.

---

## H8 — The process boundary follows the measurement

Whether the simulator hosts the oracle in-process, or talks to it over a pipe,
is deferred until H7 has a number. The driving code is the same either way, and
the switch is cheap. We must not pick now and discover later
that the choice cost an order of magnitude.

*Resolved.* `RunManager.SetUpTest` throws if `State != null`. But
`RunManager.CleanUp(bool graceful = true)` is public, and nulls `State` in a
`finally`, having reset `ActionQueueSet` and `CardSelectCmd` and disposed every
synchronizer. So a process runs many scenarios, as `SetUpTest` → drive →
`CleanUp` → `SetUpTest`. The startup cost amortizes over a whole batch, not over
each scenario. That removes the main argument for the out-of-process design.

*Check.* Two scenarios in one process must produce the same observations as the
same two scenarios in two fresh processes. That is the real test of whether
`CleanUp` leaves anything behind. It is where cross-scenario state leaks show
up, and they look like simulator bugs.

*Partly demonstrated,* as a side effect of the `Flight` case (H2). That case
needs a second run, because modifiers are fixed at run creation.
`OracleDriver.Restart` does `CleanUp` → `BuildRunState` → `StartRun`. The suite
now runs a third and fourth run, after seven fights, eight relic and power
grants, and roughly forty card plays in the first. Both restarts reproduce the
first run's start state exactly:

```
run 2 (no modifiers): floor=1 hp=80/80 gold=99 relics=[BURNING_BLOOD] deck=10
run 3 (FLIGHT):       floor=1 hp=80/80 gold=99 relics=[BURNING_BLOOD] deck=10
both: UpFront=411, all fourteen other counters 0
```

Their maps are byte-identical under a fingerprint covering every node's
coordinate, type and edges. The suite asserts that rather than assuming it,
since the `Flight` comparison is invalid without it.

That is real evidence against a leak, and it is not yet the stated check. It
compares runs *within* one process against each other, not against fresh
processes. It also compares the state at run start rather than after driving. A
leak that only manifests deeper into a run stays invisible. The full check
stays open.

---

## H9 — The protocol is machine-readable and versioned

The consumer is Rust, not a human. Line-delimited JSON, with a version field
that the simulator checks on connect and refuses to proceed on mismatch.

*Built:* `Protocol.cs` and `Channel.cs`, `STS2_HEADLESS_MODE=serve`.

One request per line, one response per request, strictly in order. There is no
server-initiated traffic after the greeting, so the client never demultiplexes.
A decision and its observation are a matched pair, and that is what makes H3's
"one command, one decision" checkable from outside the process. The greeting
states the version, the build it answers for, and the command list:

```json
{"type":"hello","protocol":1,"game":"v0.107.1","assemblyHash":1692500715,
 "commands":["start","fight","play_card","end_turn","observe","quit"]}
{"cmd":"play_card","index":2,"target":1,"full":true}
{"type":"observation","seq":3,"cmd":"play_card","resolved":true,"digest":"…","state":{…}}
```

Cards are addressed by hand index. That is how the observation reports the hand,
and the only way to disambiguate two identical cards. Targets are addressed by
`CombatId`, a stable identity that does not shift when a creature dies. An index
into the living-creature list does shift.

**Every failure answers.** Unknown command, bad index, unplayable card,
malformed JSON, and any exception from the game all produce an `error` naming
the cause. An unplayable card's error carries its `UnplayableReason` and the
model that blocked it. Nothing no-ops. A no-op that returns the previous
observation is indistinguishable from a decision that legitimately changed
nothing (H10).

### Pending decisions

*Built for card selection and combat rewards:* `Pending.cs`,
`OracleCardSelector`, `Protocol.TakeRewards`. The room states are designed below
but not implemented. H2's out-of-combat work all
routes through this, so it is settled once rather than six times.

**There are two kinds of decision point, and conflating them is the mistake to
avoid.**

*Observable room states.* Rest sites, events and shops are entered by a command
we already have, which is `travel`. Their triggers are model-owned.
`RestSiteRoom.EnterInternal` calls `BeginRestSite`, `EventRoom.EnterInternal`
calls `BeginEvent`, and `MerchantRoom.EnterInternal` populates `Inventories`. So
after travelling, the game is *quiescent and idle*, exactly as it is at the
start of a combat turn. These need no new machinery at all. The observation
grows a `room` block, and the client answers with an ordinary command.

```json
"room": {"type":"RestSite","options":[{"index":0,"id":"HEAL","enabled":true}]}
"room": {"type":"Event","id":"EVENT.NEOW","options":[{"index":0,"locked":false}]}
"room": {"type":"Shop","entries":[{"index":0,"kind":"card","id":"CLEAVE",
                                   "cost":75,"affordable":true,"stocked":true}]}
```

`choose_option {index}` serves rest sites and events. Both are
`ChooseLocalOption(int)` on their respective synchronizer, and the room type
disambiguates. Shops take `buy {index}` and `leave`, because a shop is not a
single choice.

*Blocking callbacks.* The other three arrive **inside** the action cascade, with
the game stopped mid-action, waiting on us:

| callback | signature | when |
|---|---|---|
| `ICardSelector.GetSelectedCards` | async | any card-selection prompt |
| `ICardSelector.GetSelectedCardReward` | **sync** | choosing among a card reward |
| `RewardsSet.testSelector` | async | a reward set is offered |

The first is not an out-of-combat concern at all. A card that discards or
exhausts on play blocks *inside a single `play_card`*. So this mechanism is
needed regardless of rooms, and combat exercises it first.

**The shape.** Our selector implementations do not choose. They park:

```csharp
Task<IEnumerable<CardModel>> GetSelectedCards(options, min, max)
{
    _pending = new Pending(kind: "select_cards", options, min, max);
    return await _pending.Answer.Task;      // a TaskCompletionSource
}
```

Quiescence gains one early exit, `_pending != null`. It is checked **before**
the drained-queue test, because a blocked action means the queue is by
definition not drained. The response becomes a third message type:

```json
{"type":"pending","seq":7,"kind":"select_cards","min":1,"max":1,
 "digest":"…","options":[{"index":0,"card":"STRIKE_IRONCLAD"}]}
```

It is answered by `{"cmd":"answer","cards":[0]}`. This works end to end. Granting
`BURNING_PACT` ("exhaust a card, draw 2") and playing it parks the action, and
offers the five remaining hand cards. Then `{"cmd":"answer","cards":[2]}`
exhausts exactly that card and draws. Every invalid answer is refused with the
pending intact: an out-of-range index, a repeated index, and a count outside
`min..max`.

Note the `digest`. A pending point is a legitimate comparison point, because
both implementations are at "the game asks which card to discard". So D1
and H5 apply to it unchanged. It is *mid-cascade*, which means a simulator must
model the same interleaving. That is a real constraint on the simulator, and it
is the point of reporting it rather than hiding it.

**One shape is forced on us.** `GetSelectedCardReward` is synchronous, so the
client cannot be consulted from inside it. But `testSelector` is async and runs
first. So the reward pending must ask for the whole choice at once: which
rewards to take, *and*, for a card reward, which card. The sync callback then
returns the earlier decision:

```json
{"type":"pending","seq":9,"kind":"rewards","skippable":true,"rewards":[
  {"index":0,"type":"gold","amount":23},
  {"index":1,"type":"card","cards":["STRIKE_IRONCLAD","CLEAVE","BODY_SLAM"]}]}
```
```json
{"cmd":"answer","take":[{"index":0},{"index":1,"card":1}]}
```

This is the only place the protocol's shape is dictated by a method signature
rather than chosen. Know that before writing the code, not halfway through it.

*Rewards go through `testSelector` rather than around it.* `RewardsSet.Offer`
also calls `_synchronizer.BeginRewardsSet` and asserts the set is complete
afterwards, and `SelectLocalReward` throws if no set is registered. Generating
rewards with `GenerateWithoutOffering` and driving selection ourselves means
reimplementing that bookkeeping. That is the exact class of mistake this
document keeps catching. The combat-end trigger still has to be ours
(`CombatRoom.OfferRoomEndRewards`, whose only live caller is `NCombatUi`), but
the *offer* is the game's.

**Invariants.**

1. Every response is exactly one of `hello`, `observation`, `pending`, `error`,
   `bye`. An `observation` always carries the legal set. A `pending` always
   carries its options.
2. While a `pending` is outstanding the only valid commands are `answer`,
   `observe` and `quit`. Anything else is an `error` and the pending survives.
   The client must not wander off while the game is stopped mid-action.
3. `answer` validates against the options the pending stated, then resumes and
   waits for the next decision point, which is sometimes another `pending`.
   Nesting is handled by the loop, not by special cases.
4. `seq` increments on every response, `pending` included.

**Waiting for the client is not waiting for the game.** Quiescence's timeout
bounds exist to catch a wedged engine. They must not fire while a human or a
simulator thinks. The design keeps these separate for free. The wait loop
*returns* as soon as a pending appears, so the unbounded wait is the client's
own `ReadLine`, never a bounded loop.

**Cancellation.** An outstanding pending whose run is torn down leaves the game
awaiting a `TaskCompletionSource` nobody will complete. `Restart` must fail any
outstanding pending explicitly. `CleanUp` already calls `CardSelectCmd.Reset()`,
but our own `TaskCompletionSource` is not its business.

### Why it is TCP

There was an `IChannel` interface here, with a stdio implementation beside the
TCP one. The transport was the least settled part of this requirement, and the
most expensive to get permanently wrong. It settled. Stdio was unusable by
anything but a human with `grep`, for the reason below, and no client ever ran
without a port. So both the second implementation and the interface over them
are gone. One implementation behind an interface is a guess about a second one.

**stdout is not available**, which is the constraint that drives everything
else. The engine writes 141 lines there before we emit anything. They are a
Godot banner, FMOD, the Sentry extension, and an unprefixed multi-line system
dump containing `PATH` and every mount point. Only 49 of those come from the
game's `ConsoleLogPrinter`. The rest are native and unreachable from a Harmony
patch.
Prefix-filtering a stream carrying arbitrary paths is exactly the fragility H10
exists to prevent.

*How sts2-cli solves this, and why we cannot.* It replaces `GodotSharp.dll`
with its own `GodotStubs`, in which `GD.Print` is literally
`Console.Error.WriteLine`. All game logging then goes to stderr, and stdout is
pure protocol. That is only available to a harness that does not run Godot.
Stubbing the engine to free up a file descriptor trades this oracle's entire
premise for a transport convenience.

So the choice was an inherited fd 3, or a socket. Both were prototyped, and fd 3
works. TCP on loopback won for three reasons. The client binds `127.0.0.1:0` and
passes the ephemeral port in the environment, so there is no port-allocation
race and no connect-retry loop. It needs no `unsafe`, no `dup2` and no
platform-specific spawn code on the Rust side. And it is full duplex on one
descriptor, so the peer disappearing is a single detectable event rather than
two half-failures. A loopback listener accepts from any local process, so the
client passes a token the oracle presents as its first line.

A person first ran every command above by hand over the stdio implementation,
before that implementation was removed. Nothing but a human was ever able to
drive it, for the reason stated.

*Check, satisfied.* Two fresh processes replaying one 17-decision script over
TCP produced identical digest sequences. That verifies D1 end to end through the
protocol, rather than only in-process.

---

## H10 — Failure is loud

No default-valued observation, no swallowed exception, no partial state
presented as complete. Any exception from the game surfaces as a protocol error
that fails the test.

*Why this is a requirement and not a style note.* Every silent-degradation path
turns a simulator bug into a passing test. That is the one failure mode that
makes the whole exercise worthless, because it is unfalsifiable from the outside.

*Check.* Inject an exception into a game action. The run must fail visibly.

**Satisfied, and it took a real bug to get there.** The game catches most of its
own exceptions and logs them rather than letting them escape. So an action can
fail completely while the driver watches it complete normally. Granting a card
to hand without registering it with the `CombatState` made
`CardModel.OnPlayWrapper` throw `"BURNING_PACT must be added to a CombatState
before playing it"`. The game swallowed it, still spent the energy, and the
protocol answered `"resolved": true` with a clean observation. **Nothing in the
driver's own view was wrong**, so no amount of care in the driver catches it.

`GameErrors` now subscribes to `Log.LogCallback`. It drains at the start of
every command, so errors are attributed to the command that caused them. It then
attaches them to whatever response that command produces:

```json
{"type":"observation","seq":3,"cmd":"play_card","resolved":true,"digest":"…",
 "gameErrors":["System.InvalidOperationException: BURNING_PACT must be added to
 a CombatState before playing it"]}
```

The oracle does not judge which errors matter. It refuses to hide any of them.
Mutation-tested by restoring the missing registration: the field appears on
exactly the `play_card` that caused it, and disappears when the bug is fixed.

This also gives open question 4 a mechanism. The `Missing screenShake target!`
errors now show up in `gameErrors` on whichever commands produce them, rather
than having to be argued about from inspection.

---

## H11 — The driver ends its own process

When the driver is done the process exits, rather than returning to the game's
main loop.

*Why this earns a requirement.* Without it a two-second run occupied a process
until an external timeout. SIGTERM is not reliably fatal here. Instances
survived `pkill` for thirteen hours and needed SIGKILL, while `timeout` reported
`dumped core` and made a hang look like a crash. Any batch of scenarios (H8)
leaks a process per scenario.

Quitting goes through the scene tree, which is how the game exits itself
(`NGame` calls `GetTree().Quit(0)`), so shutdown takes its normal course.

*Check.* A full run costs ~2.9s wall clock end to end, exits 0, and leaves no
process behind.

---

## H12 — A process that hangs still dies

A headless process ends on SIGTERM, and ends itself if it has not finished
within a wall-clock budget.

*Why this earns a requirement.* H11 covers the driver finishing. It says nothing
about the driver never finishing, and that is the case that hurts. When a body
awaits something that never completes, `Driver.Run`'s `finally` is unreachable,
and every exit path in this mod is behind it. The process then sits at full tilt
holding whatever the run allocated.

This is not hypothetical. On 2026-08-14 a hung `EveryCharacterCanBeDriven` was
sent SIGTERM by its supervisor's ten-minute timeout, ignored it, and stayed.
Successive runs stacked more of them. Over about forty minutes the machine went
from 22% memory to 96.8%, with swap completely full. Load average reached 154,
113 tasks were blocked in uninterruptible sleep, and ssh stopped answering. The
power button failed to complete a shutdown, because shutdown is SIGTERM to
everything, and these instances do not answer SIGTERM. It needed the power
switch.

Godot answers SIGTERM by asking its main loop to close, and that is the one
thread that is not answering. So the signal that is supposed to be the safety
net is the one thing guaranteed not to work here. Hence a handler that does not go
through the scene tree, and a clock that does not go through the driver.

The implementation deliberately uses nothing that can block: `write(2)` to
stderr and `_exit(2)`, off the main loop. `Log`, `Console` and
`Environment.Exit` are all reachable only through locks that the stuck thread can
hold. And `Environment.Exit` has already been observed here never to complete
(see H11).

Not armed when `STS2_HEADLESS_MODE` is unset. That process is somebody's actual
game, and it must keep Godot's signal handling so it saves on the way out.

*Check.* With `STS2_HEADLESS_TIMEOUT=3` a selftest that needs ~9s exits 124 at
3.7s, naming the budget. With the clock off, SIGTERM to a mid-run instance exits
143 in 26ms and leaves no process behind. A default run is unaffected: 144
assertions pass and it exits 0.

---

---

## Not requirements

- **Playing well.** The oracle chooses nothing. Command selection belongs to the
  test harness or the agent, never here.
- **Changing the game.** No content, no rule changes. The mod's only job is to
  expose and drive what already exists.
- **Being the training environment.** That is the simulator's job. The whole
  reason the oracle exists is that the real game is too slow for it.
- **Save states in the game.** The oracle constructs a state and reports it. It
  never restores one. Restoring means authoring a complete combat state
  description the game does not have, monster AI state included, which is
  serialized nowhere. It also means validating that description against the
  states it is meant to reproduce. Nothing here needs that. The simulator will
  likely grow its own save states for search and shrinking, but those are
  internal to the simulator and are not oracle tooling.
- **Rendering.** Headless only.
- **Co-op.** Deferred. Single-player must work first, and nothing here makes
  co-op impossible later.

---

## Open questions

1. Does `CleanUp` leave cross-scenario state behind? Narrowed, not closed. Two
   restarts after a heavily-driven first run reproduce its start state and map
   exactly. But that is one process compared against itself, at run start. (H8)
2. Does each dev console command we depend on work headless, and does it report
   failure rather than no-op silently? **Answered for `fight`, the only one
   adopted so far.** `FightConsoleCmd.Process` runs headless. Neither it nor
   `AbstractConsoleCmd` touches Godot or a Node type. An unknown encounter
   comes back as `CmdResult(success: false, "Encounter 'X' not found")`, which
   the protocol surfaces as a refusal. Each further command still needs asking.
   (H6, H10)
3. *(answered: see below.)*

*Answered so far:*

- **Granting a card to hand without registering it now throws rather than being
  swallowed.** H10's account says that case made `OnPlayWrapper` throw and the
  game catch and log it. The energy was spent anyway, and the protocol answered
  `"resolved": true` over a clean observation. That is the failure `GameErrors`
  was built for. Reproducing it deliberately in the selftest produces
  `InvalidOperationException: BURNING_PACT has no owner` straight out through
  the driver instead: loud, and a different failure. Either the path changed or
  the original account is imprecise. The suite therefore tests `GameErrors` for its
  plumbing: captured, handed over once, not twice. It does not test against a
  real swallowed failure, because we no longer have one to hand.

- **The `Missing screenShake target!` errors are presentation-only, and are now
  suppressed.** `NScreenShake.Shake` and `Rumble` log and return when
  `ShakeTarget` is null. The branch they skip only constructs a value whose
  effect is `ShakeTarget.Position = ...`, so with no target the method already
  did nothing. `sts2_headless` patches them to skip silently in that case.
  Proof rather than argument: the digests of a four-command scenario are
  byte-identical before and after the patch, and the damage still lands.

  This was worth fixing because it made `gameErrors` unusable. The model layer
  calls presentation inline. `CreatureCmd` asks for a shake when damage lands,
  so an ordinary card play logged an error. Across this session's captured runs
  that one message accounts for 352 of the game's error output. Every other line
  is our own test output or a real failure. README.md tells a reader not to
  trust an observation carrying `gameErrors`. A warning that fires every turn is
  one people learn to skip. That is how the half-completed action it exists to
  catch gets through. A live test now asserts a card play reports none.

- One process can run many scenarios. `RunManager.CleanUp` is public and nulls
  `State`. (H8)
- `CleanUp`'s `NetService.Disconnect` is harmless: on
  `NetSingleplayerGameService` every method is an empty body and `IsConnected`
  is hardcoded true, so there is no connection to re-establish. (H8)
- In-combat card legality is exposed on the model. (H2)
- **Out-of-combat legality is on the model too, for all four.** Map travel uses
  `NMapScreen`'s rule (mirrored in `MapLegality`). Rest sites use
  `RestSiteOption.IsEnabled`, events use `EventOption.IsLocked`, and shops use
  `MerchantEntry.EnoughGold` and `IsStocked`. Selection is model-layer
  throughout, and `TestMode.IsOn` makes the screens return null at creation
  rather than needing to be bypassed. Only the combat-reward *trigger* is
  UI-owned. (H2)
- **Godot's main loop drives our continuations.** No hand-pumped
  `SynchronizationContext` is needed. The stack shows
  `GodotSynchronizationContext.ExecutePendingContinuations` ←
  `GodotTaskScheduler.Activate` ← `ScriptManagerBridge.FrameCallback`. This is
  the thing sts2-cli pumps by hand up to 1000 times per action. (H3)
- Quiescence is the issued action's `CompletionTask`, then queue drain. It
  carries a bound, since cancelled actions never complete. (H3)
- **No run scene is needed.** Instantiating `res://scenes/run.tscn` segfaults
  under `--headless`, which forces the dummy rendering device regardless of
  `--rendering-driver`. The run does not need it. The presentation calls the
  model layer makes inline fail loudly as `Missing screenShake target!` and are
  caught, and combat runs real turns to completion. A full fight was played
  headless with no run UI at all.
- A run starts at the model layer in **1.6s** from process launch, and 0.7s of
  that is game load, paid once per process. It reaches floor 1 on the map with a
  correct Ironclad state and `UpFront=411` consumed. (H7)
- Scenario construction has two mechanisms. `RunState.FromSerializable` covers
  boundary state, and the dev console covers everything else. The debug gate
  admits mods without patching. (H6)
- We do not need a combat save format, so it is not a requirement. (H6)
