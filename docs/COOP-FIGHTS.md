# Fights of several players

Status: a design, 2026-10-27. It is the first open part of `BLOCKERS.md`
item 1, co-op. Co-op runs on v0.111.0. The citations are to its decompiled
source, `target/decompiled/v0.111.0-1579942752/`.

## Where the simulator stands

`cargo xtask coop --hunt` plays a simulator seat beside each game seat.
Every seed matches each step through its first fight and its rewards. The run builds a fight of every player: their creatures
first, their decks shuffled in seat order, and the monsters' health
scaled. Each relic reads its own owner, as each of the game's compares
with `Owner`. An `AnyPlayer` potion can be aimed at any player. A player
who ends the turn waits with no moves until every player ends it. Then
each phase of the turn's end runs for each player in seat order, and the
side's hooks run once. A monster's attack hits every living player, the
cards a move gives go to each, and an enemy's powered block is scaled. A
card play that stops on a question waits at the front of its player's
queue with its command frames. The other players act around it. A won
fight hands each player back their health, and rolls each player's
rewards in seat order before any is offered. A player who dies beside a
living one loses their cards and sits out each turn. They come back at 1
HP when the fight is won.

What still assumes one player:

- `Live::asking` and `Live::deferred` hold the question and command
  frames of the action in play. A question the turn loop asks belongs to
  no player's queue.
- A phase of the turn's end that waits on a question, and an extra turn,
  panic in a fight of several players.
- A potion thrown at another player panics, and so does a player's death
  inside their own card or potion beside a living one.
- Nothing scales monster powers yet.
- `FightView::player` panics in a fight of several players.
- `monster.rs` reads seat 0's stream positions.

## The game's model

**Setup.** `CombatRoom.EnterInternal` adds each player in run order, so
the player creatures come first in `CombatState`'s creatures, then the
monsters (CombatRoom.cs:127-133, CombatState.cs:71-81). `SetUpCombat`
shuffles each player's deck in turn off the run's one `Shuffle` stream
(CombatManager.cs:444-468).

**Scaling by the player count `N`.** Only with `N` above 1:

- A monster's health is `hp * N * f`, truncated, with `f` the act's
  factor (Creature.cs:749-755, MultiplayerScalingModel.cs:70-87). It is
  1.1 in act 1 and 1.2 in act 2. In act 3 it is 1.3 for a boss and 1.2
  otherwise. A monster spawned in the fight scales too.
- A monster's powered block is multiplied by `N` for two players, and by
  `N * f` for more, after the additive modifiers
  (MultiplayerScalingModel.cs:52-68).
- A power on a monster that scales takes `amount * N * f`, truncated, by
  default. Plating, Slippery, Artifact and Skittish have formulas of
  their own (PowerCmd.cs:132-135, PowerModel.cs:503-506).
- Knowledge Demon's and Waterfall Giant's heals are linear in `N`.

**The player's turn.** `StartTurn` runs `SetupPlayerTurn` for each player
in order. It waits for each only until it pauses or completes, so one
player's choice does not hold up the next player's setup. Then come
`AfterSideTurnStart`, each player's orbs, and `RunAutoPrePlayPhase` for
each living player (CombatManager.cs:688-854). In the play phase every
player acts. A card play is one action on its player's queue. The
executor runs the ready front with the smallest id, so one player's
paused choice holds up that player's queue alone (ActionQueueSet.cs
179-251).

**The turn's end.** It starts once every player is ready, and a dead
player is readied at once. Each phase runs for each player in order. It
waits for each until it pauses, then for all to complete. The phases are
the post-play phase, `BeforeSideTurnEnd`, `DoTurnEnd` and `BeforeFlush`.
After `ReadyToBeginEnemyTurn` come each player's flush and
`AfterSideTurnEnd` (CombatManager.cs:1518-1778). An extra turn is taken by the players
`ShouldTakeExtraTurn` names, and the others are readied.

**The enemies' turn.** A move's targets are every player creature, and an
attack hits every living one (MonsterModel.cs:435-450,
AttackCommand.cs:257-267, 557-559). A status card or debuff a move gives
goes to every player. Thieving Hopper steals a card from each living
player. Only its animation reads the local player (ThievingHopper.cs:189).

**Hooks.** Listeners are walked creature by creature: each player's
powers, relics, potions, orbs and cards, in player order, then the
monsters (CombatState.cs:411-494). A dead player's listeners drop out.

**Death.** A dead player's creature stays in the fight. Its cards leave
it, and its energy and stars go to 0. On its side's turn it is readied at
once, and later turns skip its setup. The fight is lost once every player
is dead. A win revives a dead player at 1 HP before `AfterCombatEnd`
(CreatureCmd.cs:475-590, CombatManager.cs:1221-1317).

## The target

The simulator models one machine's view, as the rest of co-op does: seat
`k`'s fight applies every seat's decisions, and its observation is seat
`k`'s.

1. **A fight of several players.** The run builds one, with each player's
   creature first, their opening shuffles in order, and the monsters
   scaled. The block and power scaling are hooks, as the game's are.
2. **The turn loop over players.** The coroutine runs each phase for each
   player in order. A setup or a phase that pauses for one player lets
   the next go on, and the loop waits for all at the game's barriers.
3. **The scheduler's rest** (steps 3 and 4 of `docs/SCHEDULER.md`). A
   question for each seat, in the action that asked it. Card plays,
   potions and the turn's end go through the queues, with the action
   types, the gating and the cancellations.
4. **Moves over several players.** A move's attacks and the cards and
   powers it gives reach every living player. Thieving Hopper steals a
   card from each living player.
5. **Death beside a living player.** The dead player's cards leave, it is
   readied, its listeners drop out, and a win revives it.

## The order of the work

The hunt leads. Each PR takes the runs as far as it can and says where
they part next. `cargo xtask coop --hunt` matches more steps, and the solo
golden replays, census and `cargo xtask instructions` stay where they
were.
Steps 1 and 4 come first, since the first fight needs them. Steps 2 and 3
follow when a run's turn reaches a pause, and step 5 when a player dies.

## Open questions

1. The game lets a player take back their end of turn
   (`UndoEndPlayerTurnAction`). The headless seats never offer it. The
   recommendation is to leave it out until a front end wants it.
2. Thieving Hopper steals a card from each living player, the same on
   every machine. Only its animation reads the local player.
