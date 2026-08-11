# What the oracle has to do

The simulator is a Rust reimplementation of Slay the Spire 2. It is checked by
putting the real game and the simulator in the same state, feeding both the same
decisions, and comparing after every one. This document is the standard the
oracle is held to: if the tool satisfies everything here, a simulator that
agrees with it is correct in the ways we can check, and one that disagrees has a
real bug.

Each requirement is written so it can be failed. Where something is not yet
measured or not yet known, it says so rather than assuming.

## The shape of the thing

One process runs the real game, headless, with our mod loaded. It speaks a
line-oriented protocol: given a scenario it constructs the state and reports an
observation; thereafter the simulator sends one decision and the oracle replies
with the next observation. Whether that process is spoken to over a pipe or
hosted in-process is **not decided** — see R9.

Three capabilities carry it:

- **construct** a scenario (R7) — mechanisms identified, only a fresh run built,
- **advance** by exactly one decision (R4) — **working**, ~250 decisions/sec,
- **observe** completely (R5, R3, R6) — **working in combat**, byte-reproducible;
  out-of-combat legality is the remaining gap.

---

## R1 — Identical inputs produce identical outputs

The same seed and the same command sequence must yield byte-identical
observations, in a fresh process, on a different machine, on a different day.

*Why it is not free.* `Rng.Chaotic` is seeded from `DateTimeOffset.Now`, and
there are 193 references to it. All the ones we checked are view-layer or gated,
but three traps remain, none of which fails loudly:

- `SeedHelper` invents a seed when none is supplied (R2).
- `MonsterModel.Rng` returns `Chaotic` **silently** when queried on a
  non-mutable canonical model.
- `EncounterModel.DebugRandomizeRng()` reseeds from the wall clock at
  one-second granularity, and `FightConsoleCmd` calls it unconditionally (R7).

The last is the shape to watch for: nondeterminism coarse enough that short
test runs often agree, so it reads as flakiness rather than as a broken oracle.

*Check.* Replay one fixed script three times in three fresh processes and
diff the full observation logs. Then do it again on another machine. Any
difference is a defect in the oracle, not in the simulator.

*First result: passes, single-machine.* Three fresh processes on seed `ORACLE1`
produced identical state, RNG counters, checksum sequence and decision count.
A cold profile (modded profile directory deleted first) matched a warm one.

*Not yet checked:* another machine, and anything beyond one combat.

*A hazard this check nearly missed.* Profile progress persists across runs, so
the second run of anything is not the first. `Player has seen ftue shuffle_ftue!`
appeared on an early run and never again, which made a later comparison of two
warm runs look clean while saying nothing about a fresh profile. The driver now
calls `SaveManager.SetFtuesEnabled(false)`, and the check deletes the profile
between runs. **Determinism comparisons between two already-warmed runs prove
less than they appear to.**

## R2 — A run cannot start without an explicit seed

The tool refuses rather than inventing one.

*Confirmed trap.* `RunState.CreateForTest(..., string? seed = null)` calls
`SeedHelper.GetRandomSeed()` when seed is null, which defaults to `Rng.Chaotic`
— wall-clock. The convenient constructor is the nondeterministic one, and it
fails silently. Prefer `RunState.CreateForNewRun(..., string seed)`, where the
seed is non-optional. `CreateForTest` also defaults its player to `Deprived`,
whose `IsPlayable` is false — a second reason not to lean on it.

*Check.* Start a run with no seed; it must exit non-zero with a clear message,
not begin playing.

## R3 — The legal command set is reported, in and out of combat

At every decision point the oracle states exactly which commands the game would
accept: which cards are playable and against which targets, which potions, which
map nodes, which reward and shop and campfire choices, which cards may be
selected in a selection prompt.

*Why this is the most valuable requirement.* Generating legal moves is where a
reimplementation goes wrong quietly. Two implementations can agree on the result
of every move that was tried, while disagreeing on which moves exist. Comparing
state after a command only catches the first kind of bug; comparing the legal
set catches the second.

*Check, and a correction to it.* Every command the oracle enumerates must
execute without error. But the stated converse — commands outside the set are
rejected — **does not work for out-of-combat legality**:
`MapTravel.GetTravelablePointsFrom` has exactly one caller, `NMapScreen`, so the
model layer never rejects an illegal map move and the complement fuzz passes
vacuously. Legality there lives in the UI; there is nothing underneath to say no.

What does work is metamorphic, and it generalises: **the game's own `override`
sites enumerate every place a rule varies.** For each, construct a state where
the override is active and require the oracle's output to *change*. An oracle
that ignores the hook reports the baseline twice and fails. The matrix is
mechanically derivable — 2 overrides of `ShouldAllowFreeTravel`, 7 of
`ShouldPlay`, 55 of `IsAllowed`.

*Built:* `SelfTest.cs`, `STS_ORACLE_MODE=selftest`, covering the first of those.
It reads the travelable set back out of the emitted observation rather than
calling `MapTravel` again, since a test that re-derives the answer the same way
the reporter did would agree with a broken reporter.

*Two things it taught immediately.* Run at floor 1 it failed — not because the
oracle was wrong but because the next row held exactly that node's two children,
so free travel and normal travel coincide and the scenario could not tell them
apart. **A scenario that cannot discriminate is not evidence**, so the test now
searches for a position where the two rules differ and fails loudly if none
exists. And, mutation-tested: reverting the oracle to `current.Children` makes
it fail (`1 -> 1 nodes`) and restoring `MapTravel` makes it pass (`1 -> 3`), so
the test is known to catch the bug it was written for.

*Resolved for combat: the game exposes it on the model, not the UI.*

```csharp
public virtual TargetType TargetType { get; }                              // CardModel:421
public bool CanPlay(out UnplayableReason reason, out AbstractModel? preventer)  // :1388
public bool CanPlayTargeting(Creature? target)                             // :1372
public bool IsValidTarget(Creature? target)
```

`CanPlay` covers the unplayable keyword, energy and star cost
(`HasEnoughResourcesFor`), no-living-allies, hook blocks, and card logic.
`IsValidTarget` covers side and aliveness against the ten-case `TargetType`.
So in-combat legality is *inherited* from the game rather than reimplemented by
us, which is what this requirement wanted.

*Bonus worth exploiting.* `UnplayableReason` is a `[Flags]` enum, so the oracle
can report **why** a card is illegal, not just that it is. Comparing reasons
rather than booleans localises a simulator bug to the specific rule it got
wrong, instead of leaving us to bisect.

*Map legality is also on the model, and reading the graph would have been wrong.*

```csharp
public static IEnumerable<MapPoint> GetTravelablePointsFrom(IRunState runState, MapPoint currentPoint)
{
    if (Hook.ShouldAllowFreeTravel(runState))
        return runState.Map.GetPointsInRow(currentPoint.coord.row + 1);
    return currentPoint.Children;
}
```

The obvious derivation — "the current node's children" — is correct until the
first run holding `Winged Boots` or the `Flight` modifier, both of which
override `ShouldAllowFreeTravel` and widen the legal set to the whole next row.
It would then have been silently wrong, in a way no state comparison catches,
which is the exact failure mode this requirement exists to prevent. The oracle
calls `MapTravel.GetTravelablePointsFrom`.

*Still open.* Reward, shop and campfire choices.

## R4 — One command advances the state exactly one decision

The oracle issues one command and returns when the game is quiescent and waiting
for the next player decision. It never chooses on our behalf and never advances
past a decision point.

*Note on granularity.* A single player decision cascades into many `GameAction`s
— playing a card enqueues damage, triggers, draws. The unit of the protocol is
the **player decision**, not the `GameAction`. The action stream underneath is
reported (R5) but is not the command boundary.

*Resolved: quiescence is two phases, and an empty queue alone is wrong.*
`ActionExecutor.IsRunning` is false and `FinishedExecutingActions()` returns
`Task.CompletedTask` **both** when the queue has drained and when it has not yet
picked our action up. Waiting on those alone returns immediately and lets the
caller run ahead of the game: the first driver did exactly that, reporting 300
decisions in 0.07s while only five actions had actually executed.

So:

1. `await issued.CompletionTask` — every `GameAction` carries its own completion
   source, which cannot be satisfied early.
2. Then drain the cascade it triggered, until `ActionQueueSet.IsEmpty` and
   `!ActionExecutor.IsRunning`.

*And a cancelled action never completes at all.* When combat ends mid-cascade
the executor stops with actions still queued and the decision we issued is
abandoned rather than finished — observed as `EndPlayerTurnAction turn 5 not
completed: running=False current=<null> empty=False combat=False`. Waiting on
`CompletionTask` alone hangs forever. The driver must treat the fight ending as
a legitimate reason for a decision never to resolve, and every wait needs a
bound rather than trusting the task.

*Check.* A decision count that runs ahead of `AfterActionExecuted` count is the
signature of this bug; assert they stay consistent.

## R5 — The observation is complete

After each command: draw/hand/discard/exhaust piles in order, every creature
with HP, block, statuses and intent, orbs, gold, potions, relics, floor and map
position, the pending decision, and **the RNG counter for every stream**.

*Why not just use the game's own state sync.* `NetFullCombatState.FromRun`
carries piles and creatures only while `CombatManager.Instance.IsInProgress`, so
out of combat it says nothing about the deck. It also deliberately removes
`PlayerRngType.Rewards` and `PlayerRngType.Shops` from what it compares. Our
observation must be a superset, not a rename of theirs.

*Why RNG counters matter separately.* Two implementations can produce the same
visible state while having consumed different numbers of random draws. That
divergence is invisible now and fatal three decisions later. Counters catch it
at the step where it happens.

*The streams, in full.* Twelve run-level (`RunRngType`: UpFront, Shuffle,
UnknownMapPoint, CombatCardGeneration, CombatPotionGeneration,
CombatCardSelection, CombatEnergyCosts, CombatTargets, MonsterAi, Niche,
CombatOrbs, TreasureRoomRelics) and three per-player (`PlayerRngType`: Rewards,
Shops, Transformations). Fifteen counters per observation.

*Check.* For a state reached two ways — replayed from floor 1, and constructed
directly (R7) — the observations must be equal.

### Built, in `Observation.cs`

Emitted as one JSON line per decision. Carries act, floor, hp, gold, relics,
potions, deck; and in combat: turn, phase, energy, round, all four piles in
order, every creature with hp/block/powers, and the legal command set. Card
identity includes upgrade level and affliction, since the id alone cannot
distinguish two decks. All fifteen RNG counters are reported, run-level and
per-player.

Two fields exist here that the game itself never ships. **Monster intent** —
absent from `NetFullCombatState` because peers re-derive it, and serialised
nowhere — is reported (`HEADBUTT_MOVE`, `THRASH_MOVE`), because a simulator
choosing the wrong monster move is exactly what this catches. And **the reason a
card is illegal**, from the `UnplayableReason` flags, so a disagreement names
the rule rather than needing a bisect; `EnergyCostTooHigh` is confirmed firing.

Absence is distinguished from emptiness: the `combat` object is omitted out of
combat rather than serialised with empty piles, so "not in combat" and "in
combat holding nothing" cannot compare equal.

*Result.* Two cold-profile processes on one seed produced **byte-identical**
36KB observation streams, 22 observations each.

*Not yet in it:* the pending decision's identity, map coordinates (only act and
floor), orbs (no Defect run driven yet), and out-of-combat legality, which is
R3's remaining gap.

## R6 — Divergence is detectable out of combat too

`RunManager.SendPostActionChecksum` is gated on
`CombatManager.Instance.IsInProgress`, so the game's own per-action checksums
exist only inside combat. Outside it — rewards, shops, campfires, map movement,
card removal and upgrade — there is only the coarse room-exit checksum.

The oracle supplies its own digest over the full observation so that every step
is comparable, not just combat steps.

*Built.* A SHA-256 over the full observation, truncated to 8 bytes. This is also
what makes the stream affordable: the full observation is ~1.6KB per decision
(27% creatures, 27% legal moves, 19% RNG counters, 11% deck — mostly repeated
key names for values that rarely change), and at 250 decisions/sec that is
400KB/s to say "still agreeing".

Digests cut a 22-decision fight from **36,410 bytes to 374** — 99%, 17 bytes a
step. The full observation is available on demand via `STS_ORACLE_OBS=full`,
and that fallback costs nothing precisely **because R1 holds**: the diverging
run replays exactly. Compacting the JSON was the alternative and is worse — it
saves perhaps 40% and makes the failure path harder to read, which is the path
that matters.

*Constraint found in `InitializeShared`.* `ChecksumTracker.IsEnabled` is set to
`!TestMode.IsOn && NetService.Type is one of {2,3,4}`. `NetGameType` is
`{None, Singleplayer, Host, Client, Replay}`, so that set is Host/Client/Replay
— meaning checksums are off for a singleplayer service **regardless** of
`TestMode`, and `TestMode.IsOn = true` (which the mod sets, to skip animation
waits) would disable them anyway. Two independent reasons, both silent.

`IsEnabled` has a public setter, so the fix is to re-enable it after `SetUpTest`
returns. Nothing about enabling it needs a peer: `GenerateChecksum` only sends
over the wire when `NetService.Type == Client`, and otherwise just computes
locally and fires
`ChecksumGenerated(NetChecksumData, string context, NetFullCombatState)` —
handing us the full state alongside the hash, not merely the hash.

*Check.* Perturb one card in the deck out of combat; the digest must change.
Separately, assert `ChecksumTracker.IsEnabled` is true once a run is set up —
it defaults to true at construction and is then turned off, so a regression here
is silent.

## R7 — Scenarios can be constructed, not only replayed

Start from a specified state — this deck, these relics, this HP, this floor,
this encounter — without playing to it. This is firm, not contingent on what it
costs to build: R5's strongest check depends on it, and a suite that can only
reach states by replaying to them is a suite nobody adds tests to.

*Why.* Differential testing wants targeted coverage of rare interactions.
Reaching a state by replaying a run is slow, and it breaks whenever anything
upstream of the state changes, which makes the test suite fragile in exactly the
way that discourages adding tests.

### Editing is the point

The motivating case is not "replay to an interesting state" but "**give the
player this relic on floor 1 and test it in the first fight**" — without
seed-searching for a run where the relic happens to be offered.

Two mechanisms cover it, and neither requires the game to gain a save format.

**Boundary state.** `RunState.FromSerializable(SerializableRun save)` restores
players, decks, relics, potions, acts, modifiers, visited map coords, events
seen, map point history, and — through `RunRngSet.FromSave` — exact RNG stream
positions. A restored run resumes via `RunManager.LoadIntoLatestMapCoord`, so it
comes back **at the map**, not inside a room.

It reaches room boundaries only. `SerializablePlayer` has **no combat state at
all** — no hand, no piles, no pile order, no block, no statuses, no *current*
energy — and `SerializableRoom` has no creatures, enemy HP or intents. The game
does not persist a combat in progress. (`NetFullCombatState` carries far more
but is write-only here, and is a *fingerprint* rather than a state description:
`CreatureState` is `{monsterId, playerId, currentHp, maxHp, block, powers}` with
no intent, and `MonsterModel.NextMove` is `private set` and serialized nowhere.
Anything two peers can re-derive independently need not ship.)

**The dev console**, below, covers the rest — including mid-combat.

*A trap waiting for scenario construction.* Content availability is decided by
the `UnlockState` handed to the run, and every pool gates on it
(`GetUnlockedRelics(UnlockState)` and friends drop epoch-locked entries). A
fresh run gets `UnlockState.all` — all epochs, all encounters, 999999999 runs —
so everything is available. But `RunState.FromSerializable` rebuilds it from
`SerializableUnlockState` instead, so **a constructed scenario can silently have
less content available than a fresh run**, and would differ from the simulator
without anything in the scenario saying so. Constructed scenarios must set the
unlock state explicitly rather than inheriting whatever the save carried.

Note also that the game's `unlock` console command is *not* the tool for this:
`UnlockCards` calls `Progress.MarkCardAsSeen`, which is compendium "seen it"
state with no effect on availability — and it writes to `progress.save`, the
persistent profile residue R1 and R13 work to avoid.

### Construction must go through the game's own paths

Injecting a relic into `SerializablePlayer.Relics` produces a state the game
would never produce. Load goes `FromSerializable` → `PopulateRelics` →
`AddRelicInternal`, and **`AfterObtained()` is never called on that path**. It
is called only from `RelicCmd` and from `RunManager.FinalizeStartingRelics`.

That is right for loading a save — the save already reflects the effect — and
wrong for injection. `Pear` grants max HP in `AfterObtained`; `PunchDagger`,
`TriBoomerang`, `Claws`, `Byrdpip`, `LeadPaperweight`, `PhialHolster`,
`TanxsWhistle` and others add cards or otherwise mutate the run there. Inject
`Pear` by editing the relic list and you get the relic without the max HP,
silently, and every subsequent divergence is our fixture's fault.

**So a scenario grants a relic as a command, not as a field.** Generalised: the
editable part of a scenario is expressed as the game's own construction
operations wherever one exists, and as raw state only where none does. This is
the same reachability principle as the prefix, applied at the boundary.

### The game already has that command vocabulary: its dev console

MegaCrit's own `RelicConsoleCmd` resolves this the same way:

```csharp
TaskHelper.RunSafely(RelicCmd.Obtain(relicById.ToMutable(), issuingPlayer));
```

`RelicCmd.Obtain`, not a write to `Player.Relics` — and `.ToMutable()` on the
canonical model, which is also what keeps `MonsterModel.Rng` from silently
handing back `Rng.Chaotic`.

**The debug gate lets us in without patching.** `NDevConsole._Ready`:

```csharp
bool shouldAllowDebugCommands = OS.HasFeature("editor") || TestMode.IsOn
    || ModManager.IsRunningModded() || SaveManager.Instance.SettingsSave.FullConsole;
```

We satisfy two of the four. Unlike `NGame.IsReleaseGame()` — hardcoded `true`,
which is what made `--autoslay` dead code — this gate deliberately accommodates
mods.

**And the console logic is separable from its UI.**
`MegaCrit.Sts2.Core.DevConsole.DevConsole` is a plain class with a public
`DevConsole(bool shouldAllowDebugCommands)` and public
`ProcessCommand(string)` / `ProcessNetCommand(Player?, string)`, both returning
`CmdResult` (a success flag and message — which R12 can surface). `NDevConsole`
merely hosts one; its `_Ready` also builds fonts, text buffers and prompt
styling we have no use for. Construct our own with `true` and the node is out of
the picture.

The vocabulary covers boundary editing (`relic`, `card`, `upgradecard`,
`removecard`, `enchant`, `gold`, `potion`, `heal`), navigation (`fight`, `room`,
`travel`, `act`, `event`) **and mid-combat construction** (`applypower`,
`afflict`, `block`, `draw`, `damage`, `kill`, `energy`, `stars`).

Mid-combat construction through these commands is not "paths the game never
uses" — they are the game's paths, which is what makes a bespoke combat save
format unnecessary.

**But console commands are debug tools, not oracle primitives, and some inject
nondeterminism on purpose.** `FightConsoleCmd` calls
`encounterModel.DebugRandomizeRng()`, which is

```csharp
_rng = new Rng((uint)(DateTime.UtcNow - DateTime.UnixEpoch).TotalSeconds);
```

— wall-clock, at one-second granularity. A scenario built with `fight` would
reproduce for runs landing in the same second and diverge for runs a second
apart, which presents as flakiness rather than as an obvious break. The oracle
therefore calls `RunManager.EnterRoomDebug(roomType, pointType, encounterModel)`
directly and leaves the encounter's RNG alone.

**Every command adopted must first be audited** for `Rng.Chaotic`,
`DebugRandomizeRng`, and wall-clock reads. Adopt them one at a time, by reading
each one, not wholesale.

*To verify at runtime.* That each command we depend on works headless — some may
touch UI singletons — and that `CmdResult.success` is false rather than a
silent no-op when one does not.

*Check.* Earn a relic legitimately in one run and grant it by scenario in
another; from the same anchor the two observations must be identical. Run this
over every relic with an `AfterObtained` override — those are exactly the ones
where field-poking diverges.

### The oracle constructs; the simulator loads the resulting observation

Construction has effects beyond the visible state. `RelicCmd.Obtain` removes the
relic from `player.RelicGrabBag` and `runState.SharedRelicGrabBag` — changing
which relics future draws can produce, without consuming a draw — and calls
`AfterObtained()`, which for some relics draws RNG. A construction *script*
replayed by the simulator would have to reproduce all of that, which means
teaching the simulator a dev console it has no business having.

So the split is:

1. A scenario names construction operations **for the oracle**.
2. The oracle executes them and emits a **full observation** (R5).
3. The simulator starts from that observation.

Loading happens on the simulator side. **The game needs no save-state loader**,
which is why none is specified here.

Handing the simulator the starting state does not weaken the test, because the
starting state is not what is under test — the transitions are. It is in fact
safer than having both sides interpret a scenario spec independently: if the
observation format is missing a field, the simulator defaults it while the
oracle has the real value, and the comparison diverges at step one, visibly.
Under independent interpretation both sides would default it, agree, and the gap
would stay invisible.

The scenario format itself stays language-neutral and small, and the
scenario→game translation lives in the mod, so the game's save schema never
enters Rust and a game update churns only C#.

### Reaching states no command can express

Commands cannot dictate ordering: `draw` draws but does not say which cards, so
exact pile order is not directly constructible. Where that matters, a scenario
may carry an ordered **decision prefix** replayed from the constructed state.

Two properties make this cheap rather than fragile: anchored at combat entry the
prefix stays inside one fight, and every state it reaches is reachable by
construction. It also bisects, so shrinking a divergence to a minimal repro is
list-shortening.

The prefix is a sequence of **decisions**, not `GameAction`s. Some decisions we
push onto the action queue; others the game **pulls** from us mid-action:

```csharp
public interface ICardSelector    // Core.TestSupport
{
    Task<IEnumerable<CardModel>> GetSelectedCards(IEnumerable<CardModel> options, int minSelect, int maxSelect);
    CardRewardSelection GetSelectedCardReward(IReadOnlyList<CardCreationResult> options, IReadOnlyList<CardRewardAlternative> alternatives);
}
```

installed via `CardSelectCmd.UseSelector(selector)`, which keeps a stack and
returns an `IDisposable` scope. A prefix of queued actions alone would stall the
first time a card asks the player to choose. The driver serves both push and
pull decision points from one ordered script — the same boundary R4 defines.

*Priority: low.* Pile order is not expected to matter for the scenarios we care
about. Build this only when a scenario actually needs it.

### Scenarios carry their expected digest

A scenario that includes a prefix names a state without describing it, so the
file cannot be read to learn what it produces. Each scenario therefore stores
the observation digest it is expected to reach. That makes it self-validating,
and makes a game update that silently changes what a scenario produces show up
as a failing scenario rather than as a mystery divergence blamed on the
simulator.

*Check.* Construct a state, snapshot it, and confirm the snapshot is the state
asked for. Then confirm a constructed state and a replayed state that should be
identical produce identical observations (the same check as R5).

*Unexplored seam:* `Core.TestSupport` also contains `TestRngInjector`.

## R8 — Throughput is measured and reported

The tool reports its own commands/sec so the number is a fact rather than an
estimate.

**First measurement, single combat, greedy policy:**

| | |
|---|---|
| game load | 0.7s, once per process |
| run start to standing on the map | 1.6s from process launch |
| one full fight (22 decisions) | 0.09s |
| sustained | **~250 decisions/sec** |

For comparison the game's own `AutoSlayer` caps near ten interactions/sec, but
that is `AutoSlayConfig.pollingInterval` and `buttonClickDelay` both being 100ms
— a property of driving the UI, not a floor on the engine.

At 250/s a hundred-decision combat prefix replays in well under half a second,
which is why R7 can lean on replay and why the O(1)-resume argument for a save
format stays weak.

*Still to measure.* A full run rather than one fight, and scenario turnaround
including `CleanUp` → `SetUpTest`.

## R9 — The process boundary follows the measurement

Whether the simulator hosts the oracle in-process or talks to it over a pipe is
deferred until R8 has a number, because the driving code is the same either way
and the switch is cheap. What must not happen is picking now and discovering
later that the choice cost an order of magnitude.

*Resolved.* `RunManager.SetUpTest` throws if `State != null`, but
`RunManager.CleanUp(bool graceful = true)` is public and nulls `State` in a
`finally`, having reset `ActionQueueSet` and `CardSelectCmd` and disposed every
synchronizer. So a process runs many scenarios: `SetUpTest` → drive → `CleanUp`
→ `SetUpTest`. The ~10s startup amortizes over a whole batch, not over each
scenario, which removes the main argument for the out-of-process design.

*Check.* Two scenarios in one process must produce the same observations as the
same two scenarios in two fresh processes. That is the real test of whether
`CleanUp` leaves anything behind — this is where cross-scenario state leaks
would show up, and they would look like simulator bugs.

## R10 — The protocol is machine-readable and versioned

The consumer is Rust, not a human. Line-delimited JSON, with a version field
that the simulator checks on connect and refuses to proceed on mismatch.

## R11 — The oracle refuses to answer for the wrong build

Already implemented: `Oracle.Init` hard-stops unless `release_info.json` reports
`main_assembly_hash: 1692500715` (v0.107.1). An oracle answering for a different
build than the one we reason about is worse than no oracle.

## R12 — Failure is loud

No default-valued observation, no swallowed exception, no partial state
presented as complete. Any exception from the game surfaces as a protocol error
that fails the test.

*Why this is a requirement and not a style note.* Every silent-degradation path
turns a simulator bug into a passing test. That is the one failure mode that
makes the whole exercise worthless, because it is unfalsifiable from the outside.

*Check.* Inject an exception into a game action; the run must fail visibly.

## R13 — The player's own saves are never touched

The oracle writes to no real profile and no real save. `SetUpTest` defaults
`shouldSave: false`; keep it that way, and confirm nothing else writes.

*Check.* Checksum the profile directory before and after a full run.

*Result: satisfied for the player's data, not for ours.* A driven run leaves the
unmodded profile untouched — this install has none, and everything lands under
`default/1/modded/profile1/`. What does change there is `saves/progress.save`
(and its `.backup`), because run-level actions record profile progress:
`RelicCmd.Obtain` calls `SaveManager.MarkRelicAsSeen`, and fighting an enemy
records a discovery. Godot's own logs rotate and Sentry writes a per-run
breadcrumb directory; neither is ours to control.

So R13 holds in the sense that matters, but the residue is a **determinism**
concern rather than a tidiness one (R1): profile state that persists across runs
is state the oracle did not declare. The modded profile should be treated as
disposable, and the R1 check deletes it between runs rather than trusting it to
be inert.

## R14 — The oracle ends its own process

When the driver is done the process exits, rather than returning to the game's
main loop.

*Why this earns a requirement.* Without it a two-second run occupied a process
until an external timeout, and SIGTERM is not reliably fatal here — instances
survived `pkill` for thirteen hours and needed SIGKILL, while `timeout` reported
`dumped core` and made a hang look like a crash. Any batch of scenarios (R9)
would leak a process per scenario.

Quitting goes through the scene tree, which is how the game exits itself
(`NGame` calls `GetTree().Quit(0)`), so shutdown takes its normal course.

*Check.* A full run costs ~2.9s wall clock end to end, exits 0, and leaves no
process behind. `STS_ORACLE_QUIT=0` keeps it up for inspection.

---

## Not requirements

- **Playing well.** The oracle chooses nothing. Command selection belongs to the
  test harness or the agent, never here.
- **Changing the game.** No content, no rule changes. The mod's only job is to
  expose and drive what already exists.
- **Being the training environment.** That is the simulator's job — the whole
  reason the oracle exists is that the real game is too slow for it.
- **Save states in the game.** The oracle constructs a state and reports it; it
  never restores one. Restoring would mean authoring a complete combat state
  description the game does not have — including monster AI state serialized
  nowhere — and validating it against the states it is meant to reproduce.
  Nothing here needs that. The simulator will likely grow its own save states
  for search and shrinking, but those are internal to the simulator and are not
  oracle tooling.
- **Rendering.** Headless only.
- **Co-op.** Deferred. Single-player must work first; nothing here should make
  co-op impossible later.

## Open questions

1. Is there a model-layer legality predicate for out-of-combat choices — map
   nodes, rewards, shops, campfires — as there is for cards? (R3)
2. Does `CleanUp` leave cross-scenario state behind? (R9)
3. Does each dev console command we depend on work headless, and does it report
   failure rather than no-op silently? (R7, R12)
4. Do the caught `Missing screenShake target!` errors have model-visible side
   effects? They are presentation-only by inspection, but the oracle should not
   rest on an unverified "probably harmless". (R12)

*Answered so far:*

- One process can run many scenarios — `RunManager.CleanUp` is public and nulls
  `State`. (R9)
- `CleanUp`'s `NetService.Disconnect` is harmless: on
  `NetSingleplayerGameService` every method is an empty body and `IsConnected`
  is hardcoded true, so there is no connection to re-establish. (R9)
- In-combat card legality is exposed on the model. (R3)
- **Godot's main loop drives our continuations.** No hand-pumped
  `SynchronizationContext` is needed; the stack shows
  `GodotSynchronizationContext.ExecutePendingContinuations` ←
  `GodotTaskScheduler.Activate` ← `ScriptManagerBridge.FrameCallback`. This is
  the thing sts2-cli pumps by hand up to 1000 times per action. (R4)
- Quiescence is the issued action's `CompletionTask`, then queue drain — with a
  bound, since cancelled actions never complete. (R4)
- **No run scene is needed.** Instantiating `res://scenes/run.tscn` segfaults
  under `--headless` (which forces the dummy rendering device regardless of
  `--rendering-driver`), but the run does not need it: the presentation calls
  the model layer makes inline fail loudly as `Missing screenShake target!` and
  are caught, and combat runs real turns to completion. A full fight was played
  headless with no run UI at all.
- A run starts at the model layer in **1.6s** from process launch (0.7s of that
  is game load, paid once per process), reaching floor 1 on the map with a
  correct Ironclad state and `UpFront=411` consumed. (R8)
- Scenario construction has two mechanisms — `RunState.FromSerializable` for
  boundary state, the dev console for everything else — and the debug gate
  admits mods without patching. (R7)
- A combat save format is not needed, so it is not a requirement. (R7)
