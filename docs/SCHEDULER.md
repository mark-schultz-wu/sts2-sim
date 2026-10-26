# The scheduler, with one player

Status: a design for review, 2026-10-26. It is step 3 of `docs/ASYNC.md`'s
decision. `docs/GAME-ASYNC.md` describes the game's model, and this file
cites its sections as §N.

## Where the engine stands

Every rule in content that stops on a question is a `diapause` coroutine
now: each card play, potion, monster move and `SetupPlayerTurn`. What we
still write by hand is in the engine, in `crates/sts2-sim/src/combat/`.

The engine has no action queue, no action ids and no turn coroutine. A
player's decision runs as one Rust call stack, and `end_turn` runs the
whole enemy turn and the next turn's start inside it. Where something
stops, the code does three things:

1. It sets the one question, `Live::asking`, with a `Then` that says what
   the answer resumes.
2. It pushes its own rest as a `Deferred` entry on one stack per fight,
   `Live::deferred`.
3. It returns up the stack with nothing.

An answer runs `resume_answered`, and that runs `run_deferred`. That pops
the stack until a new question stands. A hook tail paused at a boundary
that nobody waits for goes into `Live::paused`, one list per fight. It
runs only once the stack is empty, before the next decision.

The paused work falls into three groups. `Deferred` has 22 variants and
`Then` has 5.

| Group | What it is | `Deferred` variants | Game construct |
|---|---|---|---|
| Turn loop | The rest of the turn, held across a question | `Setup`, `PrePlay`, `PostPlay`, `TurnEndHooksOver`, `TurnEnd`, `Flush`, `AfterTurnEnd`, `Move`, `Rejoin` | The turn loop's frames in `CombatManager`, and its `WhenAll` barriers (§2, §4) |
| Hook tasks | A listener's rest after a pause at a boundary | (`Live::paused`, `Paused`) | `GenericHookGameAction` on the chooser's queue (§2, §3) |
| Command frames | The rest of an engine command inside one action | `Resume`, `Finish`, `PlayOver`, `Played`, `AutoPlay`, `AutoPlayAtRandom`, `AutoPlayTaken`, `AutoPlayFromDraw`, `ActOn`, `Land`, `Draw`, `Hits`, `OwnCommandOver` | Awaited frames inside one `GameAction`: draw, auto-play, `PowerCmd.Apply`, the attack's hits |

About 3,500 lines of `sts2-sim` exist mainly to stop, resume or order
this work, which is 8% of the crate. The largest parts are these:
`run_deferred` (312 lines), `resume_answered` (183), the turn chain in
`turn.rs` (about 1,100 in all), and the command frames in `board.rs`
(about 700).

Other fields that only hold paused work:

- On `Live`: `asking`, `deferred`, `paused`, `pause_boundary`,
  `after_pick`, `hook_draws`, `drawn_late`, `drew` and `waited_attack`.
- `max_energy` and `skipping`, which five `Deferred` variants carry to the
  rest of the turn.
- The cursor types that exist to resume: `PassRest`, `TurnEndWalk`,
  `TurnEndPass`, `Walked`, `InHand`, `Carried`, `Listened` and `Drew` in
  `turn.rs`.
- `Drawn`, `Lands`, `AutoPlayFromDraw` and `HookDraw` in `board.rs`.

### What the order gets wrong today

The order comes from the stack, so the innermost work finishes first.
The game orders by action id across queues instead (§1). Three places
look different from the game. These are read from the code, not yet
shown by a run:

1. **Two tails at turn start.** A paused orb `AfterTurnStart` task is
   queued in the game before the paused setup's hook action, so it runs
   first by id. The engine runs the setup and its pre-play first.
2. **`Combat` tails in the enemy turn.** A `Combat` hook action paused in
   the enemy turn, from `BeforeSideTurnStart`, `AfterDiedToDoom` or
   `AfterDeath`, runs at once in the game. The engine runs it only after
   `end_turn` returns, which is after the next turn's setup.
3. **`Hook.BeforeFlush`.** The engine raises only the late pass,
   `BeforeFlushLate`. No ported listener answers the main pass yet.

Each needs a scenario before the change that fixes it.

### What assumes one player

- `Live::asking` holds one question, and `Live::question` returns it
  whatever the seat.
- `Deferred` entries carry no seat, except `OwnCommandOver`.
  `run_deferred` runs each one as the seat that answered.
- `Paused` has no owner.
- The turn chain takes one seat. It has no loop over players for setup,
  pre-play, `AfterAutoPostPlayPhaseEntered`, `DoTurnEnd` or the flush.

## The target

The engine runs the game's model (§0):

- **A turn loop.** One coroutine, `CombatManager`'s `StartTurn` and
  `AwaitTurnEndAndSwitchSides`, stored in the fight. It waits on signals
  and barriers, and it runs the enemy turn inline.
- **Queues.** One FIFO of actions for each player. An action is a card
  play, a potion, an end turn, or a hook action.
- **Ids.** One counter, bumped on every enqueue and on every resume.
- **An executor.** It picks the ready queue front with the smallest id,
  and it checks for a win after each action it runs, paused or not.
- **Boundaries.** A hook boundary that pauses turns the frozen frames into
  a hook action on the chooser's queue. Its type, `Combat` or
  `CombatPlayPhaseOnly`, decides when it can run.
- **Barriers.** Where the game waits for paused tails (`WhenAll`), the turn
  loop yields until those hook actions finish.

### Data

```rust
/// The game's `ActionQueueSet`: every player's queue, and the ids.
struct Queues<V> {
    queues: SmallVec<[Queue<V>; 1]>,   // one per seat
    next_id: u32,
    play_phase: bool,                  // `PlayPhase` unpauses the queues
    cancelling: Cancelling,            // end of turn, `FromHand` picks
}

struct Queue<V> {
    actions: SmallVec<[Action<V>; 2]>, // FIFO; only the front runs
}

struct Action<V> {
    id: u32,
    kind: ActionType,                  // Combat, CombatPlayPhaseOnly
    state: ActionState,                // Waiting, Paused, Ready
    body: Body<V>,
}

enum Body<V> {
    PlayCard(PlayCard<V>),             // the play pipeline, with the card's coroutine
    UsePotion(UsePotion<V>),
    EndTurn,
    Hook(HookTask<V>),                 // a listener's frozen rest
}
```

The question moves into the action that asked it. Only a paused front of
a queue can hold one. So `Live::question(seat)` reads that seat's queue,
and each seat can have its own question, which co-op needs.

A `HookTask` is what `Paused` is today: the question, the frames frozen
under the boundary, and the hand it listed from. It gains an owner seat,
a type and an id.

### The executor

```text
drive(fight):
    loop:
        if the turn loop can go on: resume it          // a signal or barrier came
        else if a ready front exists: run the one with the smallest id
            then check for a win
        else: stop                                     // a question, or wait for input
```

A decision goes in through the queues:

- **Play a card, drink a potion.** Enqueue the action, then `drive`.
- **End turn.** Enqueue `EndPlayerTurnAction`. Running it signals the
  turn loop, as `SetReadyToEndTurn` does.
- **Answer.** Resume the paused action under a new id, then `drive`.

`turn_ended_by_play`, `selector_answered` and `ask_paused` in
`run/fight.rs` fold into `drive`.

### Boundaries

`Listening` keeps its job, and changes what it does with a pause:

| Today | Then |
|---|---|
| `leave` sets the question and its frames aside as a `Paused` | `leave` makes a hook action on the chooser's queue, with the next id and the site's type |
| `queue` pushes onto `Live::paused` | Nothing more: the hook action is in its queue |
| `join` rejoins the first task, and defers `Rejoin` for the rest | The turn loop yields until those hook actions finish |
| `none_waits` panics | The same, until the enemy side's sites are ported |

`Board::after_death` makes its own boundary today. It moves onto
`Listening` too.

### Command frames

Each action keeps its own stack of command frames: what `Live::deferred`
holds today for that action alone. Ordering never reaches inside one
action, so the frames need no queue. Moving them per action is enough
for the scheduler and for co-op.

Turning the command frames into coroutines is a later and separate step.
Each carrier, such as draw, auto-play, `PowerCmd.Apply` or the attack's
hits, can move on its own. Each one takes away its `Deferred` variants and
`Live` fields. It changes no order, and the golden replays prove each one.

## What goes

With the scheduler in:

- `Deferred::Setup`, `PrePlay`, `PostPlay`, `TurnEndHooksOver`, `TurnEnd`,
  `Flush`, `AfterTurnEnd`, `Move` and `Rejoin` become points in the turn
  loop.
- `Then::Setup`, `SetupChosen` and `Chosen` become answers to the turn
  loop's coroutine or a hook action.
- `Live::paused`, `Paused`, `ask_paused` and `set_aside`/`rejoin` go into
  the queues.
- `Live::asking` moves into the actions. `pause_boundary` goes, since an
  action knows whether it is under a boundary.
- The duplicate first-turn start in `start.rs:572-692` goes. The turn loop
  starts the first turn like any other.
- The cursor types that hold the turn's rest: `PassRest`, `TurnEndWalk`,
  `TurnEndPass`, `Walked`, `InHand`, `Carried` and `Listened`.

The command frames stay, one stack for each action, until their own
step.

## Cost and cloning

The fight must stay `Clone` and small, since search clones it at each
question.

- Most of the time the queues are empty. A solo player's card play runs
  as soon as it is queued. The queues are inline `SmallVec`s.
- The turn loop's coroutine holds what the `Deferred` turn variants hold
  today: walk cursors, `max_energy` and `skipping`. Its size is the
  largest of its paused points, not their sum.
- The executor adds a queue push, a pick and a pop to each decision. That
  is a few branches against about 13,000 instructions a step.

The spike below measures the turn loop's part, which is the costly one.

## The order of the work

Each step is one PR, with the golden replays, the census and
`cargo xtask instructions` against the step before.

1. **Scenarios for the three orders above.** Each records what the game
   does. Where the engine differs, the scenario fails until the step
   that fixes it lands.
2. **The turn loop as a coroutine.** End of turn, enemy turn and turn start
   become one coroutine in the fight. The turn variants of `Deferred` go,
   and so does `start.rs`'s duplicate. `Listening::join` becomes a yield.
   The order does not change yet.
3. **Queues and ids.** Card plays, potions, end turn and hook tasks go
   through the queues, and the executor picks by id. `Live::paused` and
   `Live::asking` go into the actions. The `Combat` tails in the enemy
   turn now run when the game runs them, and the scenario for that passes.
4. **Action types and gating.** `CombatPlayPhaseOnly` waits for the play
   phase, so the turn-start order matches the game. The cancellations
   follow: end of turn, and a `FromHand` pick.
5. **Command frames as coroutines**, one carrier at a time.
6. **Co-op**: a turn loop over players, a question for each seat, and
   cross-player boundaries.

Steps 2 to 4 are the scheduler with one player.

## Open questions

1. The spike's cost: lower `diapause` better first, or keep the turn loop
   hand-written? See the spike below.
2. The engine never raises `Hook.BeforeFlush`'s main pass. It is a rules gap, not a
   scheduler one. Fix it in step 1 with its scenario, or list it in
   `BLOCKERS.md` on its own?
3. Step 5 makes every engine command a possible coroutine. Can that wait
   until after co-op, since nothing in co-op needs it?

## The spike

`EndPlayerTurnPhaseOne` became one coroutine, `combat/ending.rs`, in the
shape step 2 gives it. The code is on the branch `agent/turn-loop-spike`,
and it does not land. It runs the post-play phase, `BeforeSideTurnEnd` and its
`WhenAll`, `DoTurnEnd` and `BeforeFlushLate` and its `WhenAll`. The rest of
the turn runs from its end as before. It replaces `Deferred::PostPlay`,
`TurnEndHooksOver`, `TurnEnd` and `Flush` with one variant, and four
functions of the turn chain with one.

A barrier is a yield. The coroutine yields each paused listener's task,
and its driver parks the coroutine and then puts the task back. That
keeps the order `Listening::join` gives today.

What it showed:

- **It works.** The golden replays pass, with the order unchanged.
- **Size.** `Live` and `Deferred` keep their sizes, 4,160 and 376 bytes,
  since the paused coroutine is boxed. Its state is 216 bytes.
- **Speed.** `cargo xtask instructions` goes from 13,145 to 13,391 a step,
  +1.9%. That is about 1,000 instructions for each turn's end.
- **Compile time.** No change. A release build of `sts2-cli` after a
  change to core takes 152 to 156 s on both.

The cost is not the turn loop's. It is how `diapause` lowers a body.
Each block boundary in a run packs the live locals into the state enum,
goes round the dispatch loop, and unpacks them. That holds for every
boundary, not only a yield. This body crosses about five boundaries a run, carrying a walk
cursor, a hand of cards and the skips. `__drive`'s own code is 950 of
the 1,000 instructions. Moving the loops out of the path that does not
stop changed nothing, so it is not the loops.

The same cost is in the setup coroutine, which added +0.5%, and smaller
in the card pools' coroutines, whose bodies cross fewer boundaries.

### What that means for the order

At this rate, a turn loop from the end of a turn through the enemy turn
to the next turn's start costs about +5%. That is more than the
budget for the whole refactor at one player, about +3%.

Two ways on:

1. **Lower `diapause` better, in the vendored copy.** A boundary that
   does not suspend can stay plain Rust: the locals stay where they are,
   and only a yield writes the state. That is what Rust's own coroutines
   do. It takes most of the cost off every coroutine, the setup's and the
   card pools' too. It is a change to `lower.rs` and `expand.rs`, and the
   same suites prove it. Upstream can take it too.
2. **Keep the turn loop as it is, and do steps 3 and 4 on it.** Queues,
   ids and gating do not need the turn loop to be a coroutine. The turn's
   chain of functions and its `Deferred` variants stay, and they are not
   the cleanup this pass wants.

The recommendation is the first: a spike on `diapause`'s lowering,
measured on this same coroutine, before step 2.
