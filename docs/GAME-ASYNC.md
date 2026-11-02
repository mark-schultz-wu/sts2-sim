# The game's async model

This document describes the decompiled C# of v0.111.0, and notes where v0.107.1
differs. `docs/ASYNC.md` uses it to choose how the simulator pauses.

Paths are relative to `target/decompiled/v0.111.0-1579942752/` unless they say
otherwise. `GA/` = `MegaCrit.Sts2.Core.GameActions/`, `GAM/` =
`MegaCrit.Sts2.Core.GameActions.Multiplayer/`, `CM` =
`MegaCrit.Sts2.Core.Combat/CombatManager.cs`, `Hook` =
`MegaCrit.Sts2.Core.Hooks/Hook.cs`, `CSC` = `MegaCrit.Sts2.Core.Commands/CardSelectCmd.cs`.
Mod paths are under `mod/sts2_headless/` in this repository.

Where a statement is an inference (not read off one line), it says
"(inference)".

---

## 0. The semantics a faithful sim must reproduce (summary)

1. **One thread, cooperative, depth-first.** There is no parallelism. Every
   piece of rules code runs on the Godot main thread. Under the oracle,
   `TestMode.IsOn` makes `NonInteractiveMode.IsActive` true
   (`MegaCrit.Sts2.Core.Helpers/NonInteractiveMode.cs`). So every
   `Cmd.Wait`/`CustomScaledWait` returns at once (`Commands/Cmd.cs:38,56`).
   The executor also awaits an action directly instead of polling frames
   (`GA/ActionExecutor.cs:140-145`). Completing a `TaskCompletionSource` runs
   the awaiting continuation **inline, inside `SetResult`**. The mod relies on
   that: `Pending.cs:165-170` opts out of it with
   `RunContinuationsAsynchronously`. So control moves only where an `await`
   meets an incomplete task. "Interleaving" means switching between
   suspended coroutines at those points.
2. **Several coroutines are alive at once.** They are:
   * the **turn loop** (`CM:499-626`), which is *not* a GameAction
   * the **executor loop** (`GA/ActionExecutor.cs:123-197`)
   * each **suspended GameAction** (a card play or potion paused on a choice)
   * each **paused hook task** (now owned by a `GenericHookGameAction`)
   * each **pending question** (UI/selector/remote choice)
3. **Actions run one at a time.** The executor picks the ready queue front with
   the **smallest action id** across the per-player FIFO queues. It skips a
   queue whose front is `GatheringPlayerChoice`, and a paused queue whose front
   is `CombatPlayPhaseOnly` (`GAM/ActionQueueSet.cs:179-251`). An action that
   pauses for a choice gives the executor back, and the executor runs other
   queues' fronts. The paused action resumes only when the executor picks it
   again, under a **fresh, larger id** assigned at resume
   (`ActionQueueSet.cs:499-520`).
4. **Pause boundaries ("run-on").** A choice is signalled on a
   `PlayerChoiceContext`. Every frame between the signal and that context's
   boundary is suspended. The code that called the boundary **runs on at
   once**:
   * `GameActionPlayerChoiceContext` (card play, potion): the boundary is the
     executor. The whole action is suspended, and other queues run.
   * `HookPlayerChoiceContext`: the boundary is
     `AssignTaskAndWaitForPauseOrCompletion` /
     `WaitForPauseOrCompletionWithoutAssigningTask`. The caller (the next hook
     listener, the next player's turn setup, the rest of the turn) runs on.
     The paused tail is moved into a lazily created `GenericHookGameAction`.
     That action is appended to the **chooser's** queue with a new id, and the
     tail continues only when the executor runs that action.
   * `BranchingPlayerChoiceContext` (card `OnPlay`, potion `OnUse`, some
     draws): this is a boundary **only for a choice by a player other than
     the owner**. For the owner, it passes the pause through to the enclosing
     context.
   * `BlockingPlayerChoiceContext`: no boundary. Everything up to the turn loop
     waits inline (enemy turn, `AttackCommand` with no context, some relics).
   * A global `CardSelectCmd.Selector` (test/pushed selector) **skips the
     signal entirely**, so no pause happens (`CSC:265-268, 332, 342`). This is
     how Whispering Earring's auto-play never pauses.
5. **Whether the caller later waits for the paused tail depends on the
   site.** Some sites collect `WaitForCompletion()` and wait at a barrier:
   `EndPlayerTurnPhaseOne`, `DoTurnEnd`, `FlushPlayerHand`, `BeforeSideTurnEnd`,
   `AfterSideTurnEnd` (two barriers), and `BeforeFlush`. Others never wait:
   `Hook.AfterDeath`, `AfterDiedToDoom`, `BeforeSideTurnStart`,
   `StartTurn`'s orb step, and the turn-start setup, which is waited on only
   through the auto-pre-play chain. Branching (cross-player) never waits
   either.
6. **The type of a hook action decides when it can run.** `Combat` can run in
   the enemy turn. `CombatPlayPhaseOnly` waits until the queues unpause at
   `PlayPhase`. Turn-start setup pauses are `CombatPlayPhaseOnly`, so a
   paused player's setup tail runs **after** `AfterSideTurnStart`, every
   other player's setup and auto-pre-play, and the win check.
7. **Ids are global and come from the host.** Action ids are one counter,
   bumped on every enqueue (even one that is cancelled at once) and on every
   resume. Hook ids are a separate local counter, bumped when a choice
   begins on a hook context. Every peer derives the same ids from the host's
   message order.
8. **Cancellation is part of the rules.** End of turn cancels waiting
   player-driven actions and keeps cancelling new ones
   (`StartCancellingAllPlayerDrivenCombatActions`). A hand choice (`FromHand*`)
   cancels the chooser's queued card plays and keeps cancelling new ones
   until the choice resumes. End of combat cancels every combat action that is
   not executing, paused ones included. A card play quietly does nothing if
   its card is no longer in the hand.

---

## 1. GameAction lifecycle, the executor, and the queues (Q1)

### States
`MegaCrit.Sts2.Core.Entities.Actions/GameActionState.cs`: `None,
WaitingForExecution, Executing, GatheringPlayerChoice, ReadyToResumeExecuting,
Finished, Canceled`.

`GameActionType` (`MegaCrit.Sts2.Core.Entities.Multiplayer/GameActionType.cs`):
* `Combat`: cancelled at the end of combat.
* `CombatPlayPhaseOnly`: also deferred if requested outside the play phase.
* `NonCombat`: held during combat.
* `Any`.

The class comment says a GameAction is *only* a wrapper for player input, and
that monster moves are not actions (`GA/GameAction.cs` header, combined-listing
lines 258-278).

### Transitions (`GA/GameAction.cs`)
* `OnEnqueued(afterFinished, id)` (`:104`): `None → WaitingForExecution`. It
  sets `Id` and subscribes `ActionQueueSet.PopAction` to `AfterFinished`.
* `Execute()` (`:116`) has two entry states:
  * From `WaitingForExecution`: `→ Executing`, then
    `_executionTask = RunSafely(ExecuteAction())`, which runs synchronously up
    to its first real suspension.
  * From `ReadyToResumeExecuting`: `→ Executing`, then
    `_executeAfterResumptionTaskSource.SetResult()` (`:130`). This resumes the
    suspended body inline.

  In both cases it then awaits `WhenAny(_executionTask, _pauseForPlayerChoiceTaskSource)`
  (`:137`). If the body completed, the `finally` does these steps:
  1. `→ Finished`.
  2. `JustBeforeFinished` (checksum timing).
  3. `_completionSource.TrySetResult()` (`:147`). This wakes `CompletionTask`
     awaiters **before** the pop.
  4. `AfterFinished` (`:148`), which pops it off its queue.

  Otherwise it logs "paused" and returns, and the body stays suspended.
* `PauseForPlayerChoice()` (`:188`): `Executing → GatheringPlayerChoice`.
  Creates the resume TCS and completes the pause TCS, which ends `Execute()`.
* `ResumeAfterGatheringPlayerChoice(newId)` (`:168`): `GatheringPlayerChoice →
  ReadyToResumeExecuting`, and **replaces `Id`**.
* `WaitForActionToResumeExecutingAfterPlayerChoice()` (`:180`): what the
  choosing code awaits after the choice. It completes when the executor
  calls `Execute()` again.
* `Cancel()` (`:203`): `→ Canceled`, then `CancelAction()`. The completion
  source is cancelled **deferred** (`CallDeferred`).

### The executor (`GA/ActionExecutor.cs`)
* It starts when the queue changes and it is not already running (`:114-121`).
* `ExecuteActions` (`:123-197`) loops `GetReadyAction()`:
  1. Skip `Canceled` actions.
  2. Fire `BeforeActionExecuted`. This fires on **every** `Execute`,
     resumes included.
  3. `await readyAction.Execute()`. That returns when the action finishes
     **or pauses**.
  4. Unless the action was `EndPlayerTurnAction`/`ReadyToBeginEnemyTurnAction`,
     `await CombatManager.CheckWinCondition()` (`:161-171`). This runs after
     a pause too.
  5. Pick the next action.
* `Pause()`/`Unpause()` gate the loop. In the v0.111.0 combat flow, the
  player turn start unpauses the executor (`CM:834`). Nothing pauses it again
  during combat. `CM:579-581` only brackets combat start.

### Picking across players (`GAM/ActionQueueSet.cs:179-251`)
There is one FIFO per player (`:22-37`). The pick considers only `actions[0]`
of each queue:
* Canceled fronts are dropped (`:190-194`).
* `NonCombat` is held in combat, and `Combat`/`CombatPlayPhaseOnly` out of it
  (`:200-217`).
* A paused queue (`isPaused`) holds a `CombatPlayPhaseOnly` front (`:218`).
* A `GatheringPlayerChoice` front blocks **its own queue** (`:223-227`).

Among the rest, the **minimum `Id`** wins (`:232`). So a player's queued
actions never overtake each other, and across players the order is by id.

### Pausing for a choice
`PauseActionForPlayerChoice(action, options)` (`ActionQueueSet.cs:261-293`):
1. Asserts the action is the front of its owner's queue (`:264`).
2. Calls `action.PauseForPlayerChoice()`.
3. If a resume already arrived for this id (`_actionsWaitingForResumption`),
   it resumes at once with the stored new id (`:270-292`).
4. With `CancelPlayCardActions`, it cancels the owner's `WaitingForExecution`
   `PlayCardAction`s (below the stored resume id, if any). It also sets
   `actionCancellingPlayCardActions`. Until the resume, the queue then cancels
   any new `PlayCardAction` at enqueue (`:280-284`, `:124-129`).
5. Fires `ActionQueueChanged`.

### Resuming
`ResumeActionWithoutSynchronizing(id)` (`:499-520`):
1. **Allocates a new id** (`GetAndIncrementActionId`).
2. If the action is `GatheringPlayerChoice`: clears the play-card
   cancellation, sets `ReadyToResumeExecuting` with the new id, and fires
   `ActionQueueChanged`.
3. Otherwise it stores `(oldId, newId)` for when the pause arrives.

The comment at `:637-647` gives the reason. The new id is larger than
everything already queued, so actions queued before the resume run before the
resumed tail. Within the owner's queue the paused action is still at index 0,
so it stays first there. Against other queues it competes by its new id.

### Ids and ordering
* `_nextId` is one counter for all queues.
* It is bumped in `EnqueueWithoutSynchronizing` (`:113`) **before** the
  cancel checks, so a cancelled-at-enqueue action still uses an id.
* It is bumped again in each resume (`:502`).
* `FastForwardNextActionId` exists for replays (`:612`).

### Is execution ever concurrent?
No. There is one action `Executing` at a time (`CurrentlyRunningAction`,
`ActionExecutor.cs:50`). Other work interleaves **only at awaits on
incomplete tasks**. Those are:
* the pause/resume TCSs
* `HookPlayerChoiceContext`'s three TCSs
* the turn loop's end-turn and begin-enemy-turn signals (`CM:666, 682`)
* `Task.Yield()` in `AfterAllPlayersReadyToBeginEnemyTurn` (`CM:1714`)
* a `CompletionTask` wait (`CM:675`, `CM:1478-1507`)
* question answers (selector, `WaitForRemoteChoice`) and network messages.

The turn loop is a separate coroutine. It often runs *inside* the executor's
call stack: an `EndPlayerTurnAction` → `SetReadyToEndTurn` →
`endTurnSignalSource.TrySetResult` (`CM:963`) resumes the turn loop inline.
The turn loop then awaits the running action's `CompletionTask` (`CM:671-680`).
It resumes again inside that action's `finally`, before the pop
(`GameAction.cs:147-148`) (inference from the inline-continuation rule).

---

## 2. Player choice contexts and the exact "run-on" rule (Q2)

### Who signals
Only `CardSelectCmd` calls `SignalPlayerChoiceBegun`/`Ended`: 12 call sites,
all in `CSC`. The shape (`CSC:252-305`, `FromChooseACardScreen`) is:

```
UndoEndTurnIfNecessary(player)              // un-ends a player who had ended the turn (CSC:1025-1031)
if (Selector != null) answer inline         // NO signal, NO pause
else {
  choiceId = PlayerChoiceSynchronizer.ReserveChoiceId(player)
  await context.SignalPlayerChoiceBegun(player, options)
  if local: LocalSelector or the UI screen, then SyncLocalChoice
  else:     await WaitForRemoteChoice(player, choiceId)
  await context.SignalPlayerChoiceEnded()
}
```

`FromHand` and `FromHandForUpgrade` signal with `CancelPlayCardActions`
(`CSC:832, 923`). `FromHandForDiscard` goes through `FromHand`. Every other
selection uses `None`.

### The context classes (`GAM/`)

| class | `SignalPlayerChoiceBegun` | `SignalPlayerChoiceEnded` | boundary |
|---|---|---|---|
| `GameActionPlayerChoiceContext` (`:34-56`) | `PauseActionForPlayerChoice(Action)`, synchronous | request resume, then await `WaitForActionToResumeExecuting…` | the executor: the whole GameAction is suspended |
| `HookPlayerChoiceContext` (`:163-211`) | creates (lazily) a `GenericHookGameAction`, enqueues it, waits until it **starts executing**, then pauses it | request resume, then await re-execution | `AssignTaskAndWaitForPauseOrCompletion` / `WaitForPauseOrCompletionWithoutAssigningTask` |
| `BranchingPlayerChoiceContext` (`:77-123`) | chooser is the original owner (or the owner is null): delegate to the original context. Otherwise: create a `HookPlayerChoiceContext` for the chooser and signal its own `_pausedCompletionSource` | delegates | its own `AssignTaskAndWait…`, only for a cross-player choice |
| `BlockingPlayerChoiceContext` | no-op | no-op | none: the caller waits inline for the answer |
| `ThrowingPlayerChoiceContext` | throws | throws | n/a (events, Sleight of Flesh) |

`PushModel`/`PopModel` (`PlayerChoiceContext.cs:44-64`) is a model stack used
only for display. Hooks push each listener (`Hook.cs`, 24 sites).

### `HookPlayerChoiceContext` in detail (`GAM/HookPlayerChoiceContext.cs`)
It holds three TCSs: `_taskAssigned`, `_pausedBeforeTaskAssigned`, and
`_paused` (`:32-36`).

* `AssignTaskAndWaitForPauseOrCompletion(task)` (`:124-137`):
  1. `Task = task`. If there is a `Source`, `Task` is a wrapper that calls
     `InvokeExecutionFinished` after it.
  2. `_taskAssigned.SetResult()`.
  3. `await WhenAny(task, _paused)`.
  4. Returns `task.IsCompleted` (false = paused).
* `WaitForPauseOrCompletionWithoutAssigningTask(task)` (`:146-150`): `await
  WhenAny(task, _pausedBeforeTaskAssigned)`. It **does not** assign `Task`. A
  later `AssignTaskAndWait…` on the same context must supply it.
* `SignalPlayerChoiceBegun` on the first choice (`:177-200`):
  1. `_gameAction = GenerateHookAction(chooser.NetId, _gameActionType)`.
     This bumps the hook id.
  2. `_pausedBeforeTaskAssigned.SetResult()`.
  3. If `Task == null`, `await _taskAssigned` (suspends).
  4. `_gameAction.SetChoiceContext(this)`.
  5. If the chooser is local: `RequestEnqueueHookAction`. On the host or in
     solo, that enqueues at once with a fresh action id.
  6. `_paused.SetResult()`.
  7. `await _gameAction.ExecutionStartedTask`.
  8. `ActionQueueSet.PauseActionForPlayerChoice(_gameAction, options)`.
* A second choice on the same context (`_gameAction != null`, `:169-176`)
  checks that the hook action is the running action, then pauses it again
  (`:201`).
* `GenericHookGameAction.ExecuteAction` (`GA/GenericHookGameAction.cs:54-63`):
  1. Await the choice context being set.
  2. `_executionStarted.SetResult()`. This resumes step 8 above inline.
  3. `await ChoiceContext.Task`: the whole rest of the hooked task.
* `WaitForCompletion()` (`:213-224`): `await Task`, then `await
  GameAction.CompletionTask`.

### The run-on sequence (a hook listener under `AssignTaskAndWait…`)
`Hook.AfterDeath` (`Hook:462-476`) is the example:

1. `Task t = model.AfterDeath(ctx, …)` runs the listener **synchronously**
   down to `CardSelectCmd` → `ctx.SignalPlayerChoiceBegun`. That generates the
   hook action and completes `_pausedBeforeTaskAssigned`. `Task` is still
   null, so it suspends on `_taskAssigned`. Control falls out of every frame
   back to `Hook.AfterDeath` with `t` incomplete.
2. `ctx.AssignTaskAndWait(t)` sets `Task`, then `_taskAssigned.SetResult()`.
   **Inline**, the signal continues:
   * `SetChoiceContext`.
   * `RequestEnqueueHookAction`. That fires `ActionQueueChanged`.
     * If the executor is idle and the action is ready, the executor **runs
       it right now, inside this call**. Typical case: a `Combat`-type hook in
       the enemy turn, when no action runs.
       `ExecuteAction` completes `_executionStarted` and suspends on
       `ChoiceContext.Task`.
     * If an action already runs (for example, this death came from a card
       play), nothing happens yet.
   * `_paused.SetResult()`.
   * `await ExecutionStartedTask`:
     * If the action already started: the signal continues inline.
       `PauseActionForPlayerChoice` → `GatheringPlayerChoice` → `Execute`
       returns. The executor loop runs `CheckWinCondition` and then **any
       other ready action**, all still nested inside this call (inference).
       Then the choice is put (`LocalSelector`/UI/remote wait), and it
       suspends.
     * If it did not start: it suspends here.
3. Back in `AssignTaskAndWait`: `WhenAny(t, _paused)` is already complete, so
   it returns `false`. `Hook.AfterDeath` **moves to the next listener**.
4. The paused tail (the rest of the listener after the choice, then
   `InvokeExecutionFinished`) runs when the executor resumes the hook action.
   * If the action had not started yet, it starts when the executor reaches
     it. It sits at the **end of the chooser's queue**, behind that player's
     already-queued card plays, and its id competes with other players'
     fronts. At start, `_executionStarted` → pause → the choice is put.
   * After the answer: `SignalPlayerChoiceEnded` → `RequestResume` → new id →
     `ReadyToResumeExecuting`. When the executor picks it, `Execute` →
     `_executeAfterResumption.SetResult()` → the tail runs inline inside the
     hook action.

So the rule is this. **When a nested task pauses, the frames between the
signal and the nearest owning boundary are frozen, and that boundary's caller
runs on synchronously.** The frozen continuation becomes a queued action. Its
start is ordered by the id it got at enqueue time. Its continuation after the
choice is ordered by the id it got at resume time.

### Turn start: the per-player setup loop (`CM:688-858`)
1. `BeforeTurnStart` per creature, then `Hook.BeforeSideTurnStart` (`:721`).
   Each listener has its own `Combat` boundary, and nothing waits for a
   paused tail (`Hook:1156-1170`).
2. Phase `Start`. Clear ready sets, make new signal TCSs (`:726-733`), and
   prepare enemy intents (`:739-744`). Then `AfterTurnStart` per creature and
   `AfterBlockCleared` (`:752-769`).
3. **For each player in order** (`:770-781`):
   * `ctx = new HookPlayerChoiceContext(player, local, CombatPlayPhaseOnly)`.
   * `task = SetupPlayerTurn(…, ctx)`: energy reset → `AfterEnergyReset` →
     `BeforeHandDraw(ctx)` → hand-draw modify → `CardPileCmd.Draw(ctx)` →
     `AfterPlayerTurnStart(ctx)` (`:880-933`).
   * `await ctx.WaitForPauseOrCompletionWithoutAssigningTask(task)`.

   On a pause, the setup stays suspended inside `SignalPlayerChoiceBegun`,
   waiting for `_taskAssigned`. No hook action is in a queue yet. The loop
   goes on to **the next player's setup**.
4. `Hook.AfterSideTurnStart` (`:783`), with no context. It runs **before**
   the paused player's remaining setup listeners, draw, and
   `AfterPlayerTurnStart` listeners.
5. Per player, `OrbQueue.AfterTurnStart` under a fresh `CombatPlayPhaseOnly`
   boundary (`:786-797`). Nothing waits for it.
6. A dead player, or one not taking this turn, is set ready to end the turn.
   If that makes every player ready, it **returns** (`:799-812`).
7. **For each player** (`:813-829`):
   `RunAutoPrePlayPhase(turnState, ctx, setupTask, player)` under
   `ctx.AssignTaskAndWait…`. `RunAutoPrePlayPhase` (`:862-869`) first
   `await setupTask`, then phase `AutoPrePlay` → `CheckForEmptyHand` →
   `Hook.AfterAutoPrePlayPhaseEntered(ctx)` (Mayhem, Whispering Earring) →
   phase `Play`.
   * For a player whose setup paused, this assignment releases the suspended
     signal. The signal enqueues the hook action (`CombatPlayPhaseOnly`). The
     queues stay paused, so it **cannot start**. The await returns at
     once (`_paused` set), and the loop goes on to the next player's
     auto-pre-play.
   * For a player whose setup completed, the auto-pre-play runs now, and can
     itself pause the same way.
8. `CheckWinCondition` (`:830`). Then `ActionExecutor.Unpause()` and
   `SetCombatState(PlayPhase)` (`:834-835`). That sends any deferred
   `CombatPlayPhaseOnly` requests, then unpauses every queue
   (`ActionQueueSynchronizer.cs:120-128`). **Only now** do the turn-start hook
   actions run, in id order. Each one pauses, puts its question, and after
   the answer runs the remaining setup, then auto-pre-play, then phase `Play`.
9. The turn loop then awaits the end-turn signal (`CM:666`).

`SetupPlayerTurn`'s own comment states the intent (`CM:871-878`):

```
If the player's turn start executes a player choice (e.g. Mayhem plays Cosmic
Indifference), then the entire sequence is paused for this player. However,
other players' turn start sequences may continue, and they may play cards while
this is occuring.
```

---

## 3. Hooks raised during an action (Q3)

**By default, hooks run inline.** A `Hook.X` dispatcher that takes a
`PlayerChoiceContext` pushes each listener onto the caller's context and
`await`s it. Examples are `AfterCardDrawn` `Hook:202-218`, `AfterCardPlayed`
`:278-294`, `AfterShuffle` `:1142-1151`, and `AfterPlayerTurnStart` `:894-916`.
A pause inside such a listener propagates to the caller's boundary. Inside a
`PlayCardAction`, that suspends the whole action. Hooks with no context
(for example, `BeforeCardPlayed`, `AfterEnergyReset`, `AfterSideTurnStart`) are
awaited inline and cannot ask for a choice.

**Only these create a new boundary** (`new HookPlayerChoiceContext`):

| site | type | does the caller wait for a paused tail? |
|---|---|---|
| `Hook.AfterDeath` (`Hook:462-476`) | `Combat` (`NonCombat` out of combat) | no |
| `Hook.AfterDiedToDoom` (`:496-510`) | `Combat` | no |
| `Hook.BeforeSideTurnStart` (`:1156-1170`) | `Combat` | no |
| `Hook.BeforeFlush` (+Late) (`:544-567`) | `Combat` | yes: `WhenAll` after both loops |
| `Hook.BeforeSideTurnEnd` (VeryEarly/Early/normal) (`:1244-1274`) | `Combat` | yes: `WhenAll` after all three loops |
| `Hook.AfterSideTurnEnd` (+Late) (`:1279-1304`) | `Combat` | yes: `WhenAll` after each loop |
| `CM` turn-start setup (`:776-778`) | `CombatPlayPhaseOnly` | only through `RunAutoPrePlayPhase` |
| `CM` orb `AfterTurnStart` (`:791-793`) | `CombatPlayPhaseOnly` | no |
| `CM` `AfterAutoPostPlayPhaseEntered` (`:1544-1553`) | `CombatPlayPhaseOnly` | yes: `WaitForCompletion` per player |
| `CM` `DoTurnEnd` (`:1569-1578`) | `Combat` | yes |
| `CM` `FlushPlayerHand` (`:1762-1771`) | `CombatPlayPhaseOnly` | yes |
| `CardModel.OnPlayWrapper` around `OnPlay` (`Models/CardModel.cs:1931-1934`) | Branching, `Combat` | cross-player only; no |
| `PotionModel.OnUseWrapper` around `OnUse` (`Models/PotionModel.cs:333-336`) | Branching, `Combat` | cross-player only; no |
| `CardPileCmd.DrawWithoutBlockingOnOtherPlayers` (`Commands/CardPileCmd.cs:1000-1005`) | Branching, `Combat` | cross-player only; no |

**When hook-created actions run.** A `GenericHookGameAction` is created only
when a choice actually begins (`HookPlayerChoiceContext.cs:169-183`). It is
appended to the **chooser's** queue with the next action id. It is not a
"player-driven" action (`ActionQueueSet.IsGameActionPlayerDriven`,
`:163-170`), so end-of-turn cancelling does not touch it. Its type decides
whether a paused queue holds it:
* `Combat` hook actions run whenever their id is lowest, even in the enemy
  turn.
* `CombatPlayPhaseOnly` ones wait for `PlayPhase`.

Relative to queued player actions, it is pure id order:
* Actions enqueued before the pause (other players' plays, and **the same
  player's own queued plays behind the running action**) run before the hook
  action starts.
* Actions enqueued while the choice is open run before the resumed tail.

**Cross-player choice inside a card.** The Branching contexts let the owner's
card play run on while another player chooses
(`BranchingPlayerChoiceContext.cs:11-43`, Tutor's comment). For example, the
owner's `AfterCardPlayed` (Unceasing Top's draw) runs before the chooser's
pick, and `OnPlay`'s tail runs later on the chooser's queue.
`OnPlayWrapper` never waits for that tail (`CardModel.cs:1934-1966`).

---

## 4. Turn structure with several players (Q4)

The turn loop is `StartCombatInternal` (`CM:576-626`). It runs `StartTurn`,
then `AwaitTurnEndAndSwitchSides`, then `StartTurn(enemy)`, and repeats.

* **StartTurn**: see §2. The enemy side (`:840-852`) sets
  `IsEnemyTurnStarted`, takes a checksum, checks for a win, then runs
  `ExecuteEnemyTurn`.
* **End turn** (player-driven): `EndPlayerTurnAction` (`GA/EndPlayerTurnAction.cs`,
  `CombatPlayPhaseOnly`). It does nothing if the turn number is no longer
  current.
  Otherwise it calls `PlayerCmd.EndTurn` (`Commands/PlayerCmd.cs:279-289`) →
  `CM.SetReadyToEndTurn` (`CM:935-967`):
  1. Adds the player to `PlayersReadyToEndTurn`.
  2. If **all** players are ready (`AllPlayersReadyToEndTurn`, `:1058-1074`,
     always true in solo), it completes the end-turn signal with
     `EndTurnSignal(runningAction if player-driven, turnNumber, player, …)`.
* **Undo**: `UndoEndPlayerTurnAction` → `UndoReadyToEndTurn` (`CM:969-993`).
  Any `CardSelectCmd` choice for a player who ended the turn also forces it,
  on the player side (`CSC:1025-1031`).
* **AwaitTurnEndAndSwitchSides** (`CM:635-686`):
  1. Await the end-turn signal.
  2. Await the running player action's `CompletionTask`.
  3. `AfterAllPlayersReadyToEndTurn` (`:1440-1476`):
     * `SetCombatState(EndTurnPhaseOne)`. That calls
       `StartCancellingAllPlayerDrivenCombatActions`
       (`ActionQueueSet.cs:331-349`): it **cancels every queued
       player-driven combat action** (`WaitingForExecution`, excluding hook
       actions and `ReadyToBeginEnemyTurnAction`) and **keeps cancelling new
       ones at enqueue** (`:130-135`).
     * `WaitUntilQueueIsEmptyOrWaitingOnNonPlayerDrivenAction`
       (`:1478-1507`).
     * `EndPlayerTurnPhaseOneInternal` (`:1518-1589`):
       1. Per player, `AfterAutoPostPlayPhaseEntered` under a boundary, then
          wait for all of them. Phase `End`.
       2. `Hook.BeforeSideTurnEnd`, then a win check.
       3. Per player, `DoTurnEnd` (orb `BeforeTurnEnd`, Ethereal exhausts,
          turn-end-in-hand cards) under a boundary, then wait for all of
          them. Then a win check.
       4. Per player, `Hook.BeforeFlush`. Then a checksum and a win check.
     * Each machine then requests a `ReadyToBeginEnemyTurnAction` for **its
       local player** (`:1466-1469`). This is a `Combat` type, not
       player-driven.
  4. Await the begin-enemy-turn signal. `SetReadyToBeginEnemyTurn`
     (`CM:1006-1033`) completes it when all players are in (solo: at once).
  5. `AfterAllPlayersReadyToBeginEnemyTurn` (`:1702-1730`):
     * `SetCombatState(NotPlayPhase)`. That calls `PauseAllPlayerQueues`,
       which also clears the cancelling flag (`ActionQueueSet.cs:312-321`).
     * `await Task.Yield()`. This is a real post to the loop.
     * `EndPlayerTurnPhaseTwoInternal` (`:1741-1778`): per player,
       `FlushPlayerHand` (discard non-retained, `AfterFlush`,
       `EndOfTurnCleanup`) under a `CombatPlayPhaseOnly` boundary, then wait
       for all of them. Then `Hook.AfterSideTurnEnd`.
     * `SwitchFromPlayerToEnemySide` (`:1820-1850`).
* **Extra turns**: `SwitchFromPlayerToEnemySide` collects players with
  `Hook.ShouldTakeExtraTurn` into `PlayersTakingExtraTurn`. `SwitchSides`
  (`:1852-1893`) keeps the player side, bumps only those players' turn
  numbers, and does not bump `RoundNumber`. Then `AfterTakingExtraTurn`.
  `StartTurn` and the end-turn phases use `PlayersTakingExtraTurn` instead of
  all players (`:699-712, 1526-1530, 1749-1752`). Players not in the extra
  turn are set ready at once (`:799-812`).
* **Enemy turn**: `ExecuteEnemyTurn` (`:1405-1438`):
  1. For each enemy still in combat: `enemy.TakeTurn()`, then a win check.
  2. `EndEnemyTurn` (`:1076-1097`): `EndEnemyTurnInternal` (`:1691-1700`,
     which runs `BeforeSideTurnEnd`, `EndOfTurnCleanup` per player, and
     `AfterSideTurnEnd`), then a win check, then `SwitchSides`.

  Monster moves are not actions. A choice inside them uses whatever context
  they pass, typically `BlockingPlayerChoiceContext` (`KnowledgeDemon.cs:186`).
  `AttackCommand` falls back to Blocking when it has no context
  (`Commands.Builders/AttackCommand.cs:669-672`). So the enemy turn blocks on
  it. The exception is a `Combat` hook action created by an `AfterDeath` or
  side-turn hook, which can run in the enemy turn.

**Synchronizer combat states** (`ActionQueueSynchronizer.cs:80-138`):
* `PreCombatSetup`: queues accept combat actions but hold them.
* `NotPlayPhase`: queues paused.
* `PlayPhase`: deferred requests sent, queues unpaused.
* `EndTurnPhaseOne`: cancel player-driven actions.
* `NotInCombat`: `CombatEnded` cancels non-executing combat actions, deferred
  requests are cancelled, queues unpaused.

---

## 5. ActionQueueSynchronizer (Q5)

`GAM/ActionQueueSynchronizer.cs`.

**What is synchronized?** Only enqueue and resume events:
* `RequestEnqueue`: a full `INetAction` (card, potion, end turn, undo, ready,
  votes).
* `RequestEnqueueHookAction`: just the hook id, owner, and type.
* `RequestResumeActionAfterPlayerChoice`: the action id.

Choice *results* travel separately through `PlayerChoiceSynchronizer`
(`SyncLocalChoice`/`WaitForRemoteChoice`, keyed by a per-player choice id
from `ReserveChoiceId`, `PlayerChoiceSynchronizer.cs:69-78`). No peer sends
state. Every peer runs the same rules, and checksums detect a divergence.

**The host orders everything.**
* The host (or solo) enqueues at once and broadcasts `ActionEnqueuedMessage`
  (`:242-257`).
* A client sends a request and enqueues only when the host's confirmation
  arrives (`:146-172, 308-317`).
* Hook actions and resumes take the same route (`:190-240, 259-295`).

Because every peer applies the host's message stream in the same order, the
local `_nextId` counters agree (`ActionQueueSet.cs:637-653`). Hook ids
(`_nextHookId`, `:174-179`) are generated locally when the hook signals. That
is deterministic because every peer runs the same hook code.
`GetHookActionForId` (`:380-404`) matches the host's message to the locally
generated object. Either side can create it first.

**What decides the order of two players' actions?**
1. The order in which the **host** handles their enqueue requests. The host's
   own input is immediate, and a client's input waits on a round trip. That
   gives the ids.
2. Per-player FIFO.
3. The executor's min-id pick over queue fronts.
4. Resumes get fresh ids in host order.
5. `CombatPlayPhaseOnly` requests made in `NotPlayPhase` are parked
   (`_requestedActionsWaitingForPlayerTurn`, `:148-153`) and re-requested at
   `PlayPhase` in their original order (`:120-127`).

---

## 6. The headless harness (Q6)

**Solo** (`Driver.Quiesce`, `Driver.cs:525-570`): it polls with
`Task.Delay(1)` until the *issued* action's `CompletionTask` is done, a
question is open, or combat ended. Then it calls `WaitForDecisionPoint`.

**Co-op** (`Peers.Shared`): `Quiesce` skips the completion wait and goes
straight to `WaitForDecisionPoint()` (`Driver.cs:530-533`). That also drops
`turnMustAdvancePast` for `end_turn`. `observe` calls the same
`WaitForDecisionPoint` first (`Protocol.cs:238-245`).
`WaitOnce` (`Driver.cs:649-702`) returns when the first of these holds:
1. `Pending.Current != null`: this seat has a question.
2. `queue.IsEmpty && !executor.IsRunning && !Peers.InFlight &&
   ReadyForInput(...)` (`:661`). `ActionQueueSet.IsEmpty` means **all
   players' queues are empty** (`ActionQueueSet.cs:65-75, 620-635`).
   `ReadyForInput` (`:705-730`) needs no `BackgroundWork`, and in combat this
   seat's `Phase == Play` and `!PlayerActionsDisabled`.
3. `!Peers.InFlight && Peers.HeldByOthers(state) && (!BackgroundWork.InFlight
   || AwaitedChoices().Count > 0)` (`:672-677`). `HeldByOthers`
   (`Peers.cs:114-122`) holds when this seat has **ended the turn** or
   **voted**, or some `PlayerChoiceSynchronizer` remote choice is
   outstanding.

**What `inFlight` means.** `Peers.InFlight` is `_sent > 0` (`Peers.cs:40`).
`_sent` goes up in `Peers.Enqueue` (`:47-54`). It goes down in `Peers.Started`
(`:57-63`), which is subscribed to `ActionExecutor.BeforeActionExecuted`
(`Driver.cs:746`). It decrements for **any** action owned by the local player
that **starts or resumes**. So `in_flight == false`
(`crates/sts2-core/src/play/seat.rs:51-52, 72`) means only "as many local
actions started executing here as this seat sent". It does **not** mean:
* that the move finished
* that the queue is idle
* that the **same** action started.

A local `GenericHookGameAction` starting, or a local paused action resuming,
also decrements it. And an action **cancelled at enqueue** never fires
`BeforeActionExecuted` (`ActionQueueSet.cs:124-148`, `ActionExecutor.cs:133-137`),
so `_sent` never comes back down. The seat then reports `in_flight` forever,
and `Seated::moves` is empty. That is a real hazard (inference). It applies
once a seat submits a card while the queue cancels new plays. For example,
after all players ended the turn, or during a `FromHand` choice.

**Can hook-enqueued actions still wait in a queue when the next seat's move
goes in?** Yes, in exits 1 and 3, and on the *other* machines in every case:
* **Exit 1** (own question): other queues can still hold actions:
  * other players' plays
  * `CombatPlayPhaseOnly` hook actions waiting for `PlayPhase`
  * `ReadyToBeginEnemyTurnAction`s

  The executor can be mid-way through them. (Single thread: the check runs
  between loop pumps, so "mid-way" means suspended.)
* **Exit 3** (held by others): nothing about the queue is checked. After the
  last seat ends the turn, `EndedTurn(me)` stays true until the next
  `StartTurn` clears `PlayersReadyToEndTurn` (`CM:728`). So `end_turn` and
  `observe` can return **mid-transition**. For example, while waiting for another
  machine's `ReadyToBeginEnemyTurnAction`, with phase `End` and the enemy
  turn not yet run (inference from the conditions).
* **Other machines**: `LobbyTable` (`crates/sts2-headless/src/session/table.rs`)
  observes every seat, and the runner then plays on one. Seat A's `play` returning says
  nothing about seat B's game having **received** A's action. B's own
  `WaitForDecisionPoint` can see an empty, idle queue before A's
  `ActionEnqueuedMessage` arrives, and report a stale state. Host ordering is
  still safe: a client seat waits for its own echo, so the host has already
  enqueued its action before `play` returns. B's snapshot, and so B's legal
  moves, can lag. A stale `PlayCardAction` is then dropped or cancelled
  inside the game (`GA/PlayCardAction.cs:53-75`).

So the exact condition is `Driver.cs:661` OR `Driver.cs:672-674` OR
`Pending.Current != null`. Only the `:661` exit guarantees that **this
machine's** queues are empty and its executor is idle.

---

## 7. Nesting depth, worst-case chains, and `pause_sites` (Q7)

Frames are listed outermost first. **[B]** marks a boundary. The question is
put to the innermost context whose boundary has not yet been crossed.

1. **Mayhem auto-plays a card that asks.** This is the turn-start case that
   `CM:871-878` describes. The frames are:
   1. `CM.StartTurn` (`:825`)
   2. `RunAutoPrePlayPhase` **[B: player-setup ctx, `CombatPlayPhaseOnly`]**
   3. `Hook.AfterAutoPrePlayPhaseEntered` (`Hook:940`)
   4. `MayhemPower.AfterAutoPrePlayPhaseEntered` (`Models.Powers/MayhemPower.cs:16-20`)
   5. `CardPileCmd.AutoPlayFromDrawPile` (`CardPileCmd.cs:1145-1175`), which can shuffle → `AfterShuffle` → Stratagem asks
   6. `CardCmd.AutoPlay` (`CardCmd.cs:51-130`)
   7. `CardModel.OnPlayWrapper`
   8. Branching (same player, so it passes through)
   9. `CosmicIndifference.OnPlay`
   10. `CardSelectCmd.FromCombatPile`
   11. `SignalPlayerChoiceBegun`

   That is about 10 async frames, and one boundary deep.
2. **Hellraiser auto-plays a drawn Seeker Strike during the hand draw.** The
   frames are:
   1. `CM.StartTurn` (`:777`)
   2. `SetupPlayerTurn` **[B]**
   3. `CardPileCmd.Draw`
   4. `DrawInternal`
   5. `Hook.AfterCardDrawn` (`Hook:202`)
   6. `HellraiserPower.AfterCardDrawnEarly` (`HellraiserPower.cs:37-65`)
   7. `CardCmd.AutoPlay`
   8. `OnPlayWrapper`
   9. `SeekerStrike.OnPlay` (`SeekerStrike.cs:37`)
   10. `FromCombatPile`
   11. signal

   The draw loop itself stays frozen mid-hand. The remaining draws happen in
   the hook action after the pick. This is the v0.111.0 "split 1" shape
   (Hellraiser/Rampart) in memory `interpreter-spike`. `BlockingPlayerChoiceContext`'s
   comment names the enemy-turn variant: Centennial Puzzle → draw → Seeking
   Strike through Hellraiser (`BlockingPlayerChoiceContext.cs:11-15`).
3. **Boundary inside a boundary: a kill during an auto-play draws into a
   question.** The frames are:
   1. `PlayCardAction` **[B: executor]**
   2. `OnPlayWrapper`
   3. `OnPlay`
   4. `AttackCommand`
   5. `CreatureCmd.Damage`
   6. `Kill`
   7. `Hook.AfterDeath` **[B: per listener, `Combat`]** (`Hook:462-476`, `CreatureCmd.cs:552`)
   8. `GremlinHorn.AfterDeath` (`GremlinHorn.cs:24-30`)
   9. `CardPileCmd.Draw`
   10. (empty pile) `Shuffle`
   11. `Hook.AfterShuffle`
   12. `StratagemPower.AfterShuffle` (`StratagemPower.cs:17-24`)
   13. `FromCombatPile`
   14. signal

   Or, after the draw, the chain goes on through other frames:
   1. `AfterCardDrawn`
   2. Hellraiser
   3. `AutoPlay`
   4. another `OnPlayWrapper`
   5. `SeekerStrike.OnPlay`
   6. signal

   * The card play runs on (rest of `OnPlay`, `AfterCardPlayed`, the result
     pile) and finishes.
   * The Gremlin Horn tail becomes a `Combat` hook action at the end of the
     player's queue.

   Depth is about 18–20 frames. The scenario
   `crates/sts2-golden/tests/scenarios/gremlin-horn-s-draw-pauses-alone-after-a-doom-kill.txt`
   is the enemy-turn variant (Doom kill in `BeforeSideTurnEnd`). There the
   question waits while the enemy turn and the next turn start around it.
4. **Whispering Earring** (`Models.Relics/WhisperingEarring.cs:41-80`):
   `AfterAutoPrePlayPhaseEnteredLate` auto-plays up to 13 cards under
   `CardSelectCmd.PushSelector(new VakuuCardSelector())`. Each card's choices
   take the `Selector != null` branch, so there is **no signal and no
   pause**. That holds even for cards that otherwise pause the turn
   start (see memory `pushed-selector-auto-answers`).
5. **Cross-player** (v0.111.0 only): a card whose `OnPlay` asks another
   player, or Skill Potion thrown at an ally. `OnPlayWrapper` → Branching
   **[B]** → a new `HookPlayerChoiceContext` on the chooser's queue. The
   owner's `AfterCardPlayed` and result-pile move run first.

### `tools/pause_sites`
`tools/pause_sites/Program.cs` (no README) uses Mono.Cecil over `sts2.dll`:
* It walks the call graph back from `PlayerChoiceContext.SignalPlayerChoiceBegun`.
* It folds async state machines and lambdas into their owners.
* It follows virtual calls into overrides.
* It stops at `AssignTaskAndWaitForPauseOrCompletion` and its sibling ("a
  listener a method calls through one pauses alone", `:110-114` of the
  `grep` view).

It prints two kinds of line:
* `pauses <owner> -> <callee>`: a call that can pause.
* `dispatches <virtual> -> <overrides>`: overridable methods with a pausing
  override.

`--why <Type.Method>` prints the shortest chain.

The committed lists are `crates/sts2-golden/tests/pause-sites-v0.107.1.txt` and
`-v0.111.0.txt`, checked by `crates/sts2-golden/tests/pause_sites.rs` and
`xtask/src/pause_sites.rs`.

| build | `pauses` lines | `dispatches` lines | `CardSelectCmd` | cards | potions | powers | relics | events | monsters | modifiers |
|---|---|---|---|---|---|---|---|---|---|---|
| v0.107.1 | 81 | 10 | 10 | 40 | 9 | 6 | 7 | 4 | 3 | 2 |
| v0.111.0 | 85 | 9 | 12 | 42 | 9 | 5 | 8 | 4 | 3 | 2 |

The v0.111.0 changes:
* Added: Abundance, Tutor, Toasty Mittens.
* Dropped: Well-Laid Plans' `BeforeFlushLate` pause.
* Renamed: Knowledge Demon's move.
* Event card picks now go through `EventModel.SelectCardsToAddToDeckFromGrid`.

The pausing content is a fixed, small set:
* about 42 card `OnPlay`s
* 9 potions
* 5–6 powers (Entropy, Tools of the Trade, Tyranny, Foregone Conclusion,
  Stratagem)
* 7–8 relics (mostly `AfterObtained`)
* 2 events
* Knowledge Demon
* Sealed Deck

The **depth** does not come from these leaves. It comes from the generic
carriers between a boundary and a leaf: auto-play, draw, shuffle, death,
exhaust, and discard (Sly).

---

## 8. Other things a faithful sim must reproduce (Q8)

* **Win/loss checks.** `CheckWinCondition` runs after every executed action,
  including one that only paused (`ActionExecutor.cs:161-171`), and at fixed
  turn-loop points (`CM:830, 847, 1089, 1422, 1560, 1579, 1589`).
  `LoseCombat` only marks `PendingLoss`, which is processed at the next check
  (`CM:1265-1289`).
* **End of combat.** `CombatEnded` cancels every queued combat action that is
  not `Executing` (`ActionQueueSet.cs:376-405`), including **paused** hook and
  card actions (`GatheringPlayerChoice` ≠ `Executing`). Their tails never
  run. The turn loop dies through `turnState.Ct`. `IsOverOrEnding` guards
  draws, auto-plays and selections (`CardCmd.cs:53`, `CSC` `FromHand:819`,
  `CardModel.cs` loop). A card or potion effect can be suspended past its
  combat's end, which is why `BeginCardOrPotionEffect` returns a `CombatId`
  and later steps check `IsCurrentLiveCombat` (`CM:373-415`).
* **Death in the middle of an action.**
  * `OnPlayWrapper` returns early after `OnPlay`, the enchantment, the
    affliction, or `AfterCardPlayed` if the owner died (`CardModel.cs:1935-1968`).
    It skips the rest, including the result-pile move.
  * `HandlePlayerDeath` defers card removal while that player is inside a
    card/potion effect (`_cardOrPotionEffectDepth`), and
    `EndCardOrPotionEffect` does the removal at depth 0 (`CM:396-415,
    1221-1243`).
  * `CheckForEmptyHand` is suppressed while the effect depth is non-zero
    (`CM:1174-1180`).
  * Hook dispatch skips inactive players (`IsActiveForHooks`,
    `CombatState.cs:425`).
* **Cancellation of queued card plays.** There are three sources:
  * at end of turn (`isCancellingPlayerDrivenCombatActions`, §4)
  * during a `FromHand`/`FromHandForUpgrade` choice
    (`actionCancellingPlayCardActions`, which also cancels plays already
    queued)
  * at execution, when the card is no longer in hand (`PlayCardAction.cs:53-58`,
    a silent return) or cannot be played or targeted (`:71-75`, `Cancel()`).

  `EndPlayerTurnAction`/`Undo…` are ignored when the turn number is stale
  (`EndPlayerTurnAction.cs:28-40`).
* **A choice un-ends the turn.** `UndoEndTurnIfNecessary` (`CSC:1025-1031`).
* **The listener list is snapshotted per walk.** `CombatState.IterateHookListeners`
  builds a list up front (`CombatState.cs:411-…`). This is relevant to
  listeners added or removed during a paused walk.
* **The executor can run other actions inside a signal.** Suppose a `Combat`
  hook action enters the queue while the executor is idle. Then the executor
  starts inside `SignalPlayerChoiceBegun`. It can run other ready actions
  before the paused caller runs on (§2, step 2). This matters in the enemy turn,
  and in co-op.
* **`Task.Yield` before phase two** (`CM:1714`). This is a real
  yield-to-loop point. Any ready `Combat` action (for example, hook actions and
  other machines' `ReadyToBeginEnemyTurnAction`) can run between "queues
  paused" and the hand flush.
* **A latent deadlock.** A `CombatPlayPhaseOnly` hook action created in
  `FlushPlayerHand` (`AfterFlush`) is enqueued while the queues are paused,
  and phase two `WaitForCompletion`s it. If an `AfterFlush` listener ever
  asks, the turn hangs (inference). Neither build's pause list has such a
  listener.
* **Turn-start setup quirk.** If a player dies between setup and
  auto-pre-play, `RunAutoPrePlayPhase` is skipped (`CM:822`). A setup paused
  on `_taskAssigned` then never gets a task (inference).
* **Checksums** are taken at `JustBeforeFinished` of each action and at named
  turn-loop points (`CM:798, 843, 1430, 1588, 1776`).

---

## 9. v0.107.1 vs v0.111.0 differences that matter

* **v0.107.1 has no `BranchingPlayerChoiceContext`.** `OnPlay` is awaited
  directly with the action's context
  (`v0.107.1-1692500715/MegaCrit.Sts2.Core.Models/CardModel.cs:1931`). So a
  cross-player choice there blocks the owner's queue.
* **v0.107.1 has no single turn-loop coroutine.**
  * `SetReadyToEndTurn` fires `AfterAllPlayersReadyToEndTurn` (or
    `WaitForActionThenEndTurn`) with `TaskHelper.RunSafely`. It defers this
    while `_inPlayerTurnSetup` is set (`CombatManager.cs:684-731`).
  * `SetReadyToBeginEnemyTurn` likewise fires
    `AfterAllPlayersReadyToBeginEnemyTurn` (`:784`).
  * v0.111.0 replaced both with signals awaited by one turn loop, plus
    stale-turn guards (`CM:635-686, 1440-1460`).

  The ordering is meant to be the same (inference).
* **The power-owner fallback differs.** In `HookPlayerChoiceContext`, an
  enemy's power belongs to `combatState.Players[0]` in v0.107.1. In
  v0.111.0 it is `GetOwner(...)` → the first player (`HookPlayerChoiceContext.cs:97-106`).
  This affects which queue an enemy-power hook action lands on in co-op.
* **The oracle's selector differs.** In v0.111.0 the oracle installs its
  selector as `LocalSelector`, so `SignalPlayerChoiceBegun` really runs
  (`mod/sts2_headless/Compat/stateful-rng.cs:40-42`). In v0.107.1 it is the
  global `Selector` (`Compat/counter-rng.cs:22-24`), with
  `ScreenPathSelections` patches that restore the screen path (memory
  `v111-local-selector`).

---

## 10. What this implies for the sim architecture (observations, not a design)

* **The core abstraction is a scheduler.** It has per-player FIFO queues of
  "tasks", picked by min global id. A task can be suspended at a known set of
  points. Alongside it there is exactly one non-queued coroutine (the turn
  loop) that blocks on barriers (signals, `WaitForCompletion`, `WhenAll`).
* **A pause is a continuation transplant.** On a pause at a hook boundary,
  the frozen frames (signal → boundary) become the body of a new queued task.
  The boundary's caller continues at once. The sim needs:
  1. a way to represent "frames from leaf to nearest boundary" as data
  2. the boundary table of §3 (type, and whether the caller later waits)
  3. id assignment at **enqueue** and again at **resume**.

  Today's hand-written `Resume`/`Deferred` points (memory
  `interpreter-spike`) encode (1) per site. The "run-on rule copied as one
  case" bug class comes from not having (2)/(3) as general machinery.
* **The leaves are few and static. The carriers are what nest.** About 85
  sites, listed per build. The depth comes from a handful of re-entrant
  carriers: auto-play → `OnPlayWrapper`, draw → `AfterCardDrawn`, shuffle →
  `AfterShuffle`, kill → `AfterDeath`, exhaust, and Sly discard. A sim that
  makes those carriers resumable covers nearly every chain.
* **Co-op adds four rules:**
  * pick across queues by id
  * `Combat` vs `CombatPlayPhaseOnly` gating
  * cancellation flags
  * every peer computing the same hook ids.

  It also adds the harness caveats in §6: `in_flight` miscounting,
  mid-transition observations, and observations on a peer that has not
  received a move yet.
