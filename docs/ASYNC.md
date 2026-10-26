# How the simulator pauses

Status: decided, 2026-10-25. Paused work becomes coroutines built by
`diapause`, option 4 below, with the game's scheduler under them.

## The problem

The game's rules are async C#. A card, a hook or a monster move can stop
in the middle to ask a player a question, and go on after the answer.
At some of those stops, the code that called the paused work runs on
before the paused work ends.

The simulator is synchronous Rust. It writes each stop by hand:

- In the fight engine, 22 `Deferred` variants, 5 `Then` variants, 4
  `Setup` stages and about 25 other types hold paused work. About 3,500
  to 4,000 lines of `sts2-sim`, 10% of it, exist to pause and resume.
  `carry_setup` alone is 895 lines.
- In content, 145 `Resume` variants and 370 `fight.waiting()` checks name
  a resume point at each place a card can stop.
- 57 `unimplemented!` stops are pauses in places the simulator cannot
  resume. A further 127 are hook sites that cannot carry out an effect a
  listener asks for, because each site matches effects by hand.
- 159 of 957 commits are about pausing, resuming or their order. Since
  2026-10-15 it is 40 of 118, which is 34%, and the share rises.

The hunts find these one at a time. The plan of 2026-10-23 (#633) made
the run-on rule general and listed the game's pause sites. That helps,
but each new site still needs its own resume point.

Co-op makes the problem larger. Each player can have work paused on a
choice while the other players play cards. `Live.asking` holds one
question. Only 1 of the 22 `Deferred` variants knows its seat, and
`run_deferred` runs every continuation as the seat that answered, the
enemy turn's too.

## How the game pauses

`docs/GAME-ASYNC.md` describes the game's model, with the lines it is read
from. In short:

1. **One thread.** Nothing runs in parallel. Control moves only where an
   `await` meets unfinished work. Under the oracle every wait
   returns at once, so the order is exact and repeatable.
2. **Many coroutines alive at once.** The turn loop, the action
   executor, each card play paused on a choice, and each paused hook are
   all suspended coroutines.
3. **One action at a time.** Each player has a queue of actions. The
   executor runs the queue front with the smallest action id. It skips a
   queue whose front waits on a choice. A paused action gets a new, larger
   id when it resumes.
4. **Boundaries.** A choice suspends every frame between the question
   and the nearest boundary. The code that called the boundary runs on at
   once.
   - A card play's boundary is the executor. Other queues run.
   - A hook's boundary is the hook site. The suspended rest of the hook
     becomes a new hook action at the end of the chooser's queue. So it
     runs after the actions that player already queued.
   - A card's `OnPlay` is a boundary only when a player other than its
     owner chooses.
5. **Sites differ in whether they wait.** The turn-end sites wait for
   paused hooks at a barrier. `AfterDeath` and `BeforeSideTurnStart` never
   wait.
6. **Action types gate when work runs.** A hook action marked
   play-phase-only waits until the play phase. So a player whose turn
   setup paused falls behind the other players' setup, `AfterSideTurnStart`,
   the orbs and the win check.
7. **The host numbers the actions.** Every peer applies them in the
   host's order.

## What a faithful simulator needs

Two parts, and the second is not optional:

1. **A way to hold paused work.** Any number of nested frames, held as
   data, with more than one chain paused at once.
2. **The scheduler.** Each player's queue, the ids, the boundaries, the
   action types and the barriers, as `ActionQueueSet` and
   `CombatManager` have them.

The scheduler is what gives the game's order across players, and the
order of a hook's paused rest against that player's queued cards. A
simulator without it copies that order case by case, which is what the
hunts keep finding. The scheduler is small. Only player actions and the
paused rest of a hook are queue entries. Hooks run inline unless they
pause, so a step that pauses nothing adds one queue entry at most.

The real choice is part 1.

## Ways to hold paused work

A fight paused on a question must clone, because AI search branches
there. Speed is a hard limit too: spire-mentor's W1 costs about 7,400
instructions an action, and ours costs 9,872.

| | Order as the game has it | Clones paused | Speed | Port | Toolchain |
|---|---|---|---|---|---|
| 1. Resume points by hand | Case by case | Yes | Today's | None, then each site | Stable |
| 2. Frame stack (#671) | Yes | Yes | +4% to +10%, estimated | ~18,000 lines at once | Stable |
| 3. Clone coroutines | Yes | Yes | No cost found on the toy | Pause sites only | Nightly |
| 4. `diapause` | Yes | Yes | No cost found on the toy | Pause sites only | Stable |

### 1. Resume points by hand

What the simulator does now, with a resume point and a seat added at each
site co-op reaches.

- It keeps today's speed, and every continuation is already data that
  clones.
- It does not converge. Each site is found by a hunt and written by hand.
  Co-op multiplies the sites by the seats, and it adds the cross-player
  order, which no site can see.
- spire-mentor, our speed target, works this way. It holds one open
  question, 13 `pending_*` resume fields and about 30 choice resolvers.
  It answers a question nested in a resumed auto-play with option 0, and
  it has no co-op engine. So its speed does not come from how it pauses,
  and it shows where this road ends.

### 2. A frame stack

The spike in #671 (`docs/INTERPRETER-SPIKE.md` on its branch). Paused
work is a stack of plain-data frames, and one loop steps the top frame.

- The order of v0.111.0 split 1 falls out with no special case. A paused
  fight clones, and each copy answers on its own.
- Each frame costs 5 to 7 ns over a direct call. From the real engine's
  call counts that is +4% to +10% a step, an estimate.
- A card becomes a `match` on its step, unless a macro writes the steps.
  Closures, such as an attack's, become data.
- The port rewrites about 18,000 lines of the engine, cards, potions and
  monsters.

### 3. Clone coroutines on nightly Rust

Nightly Rust's `#[coroutine]` closures compile straight-line code into
a state machine, as `async` does. With `coroutine_clone`, a suspended
coroutine is `Clone` when its locals are. A test in this investigation
showed:

- A card calls a hook that asks. Two such tasks suspend while their
  caller runs on. The world that holds both suspended chains clones, and
  the two copies answer differently.
- A Havoc-like auto-play recurses three deep through a boxed task. Forks
  taken at each of the three pauses come out right.
- On a path that does not pause, the coroutine runs at 1.36 ns. The
  same code as a plain function runs at 1.42 ns. The coroutine inlines.
- A borrow of the fight held across a `yield` is a compile error.

What it needs:

- Only the methods on the pause-site list become coroutines. The list
  (`tools/pause_sites`) holds 85 in v0.111.0. So do the carriers between
  them and a boundary: auto-play, draw, shuffle, death, exhaust and
  discard. Code
  that cannot pause stays a plain function.
- The fight reaches the coroutine as a pointer, because a coroutine
  cannot take a `&mut` borrow that lasts across its resumes. An accessor
  that borrows the resume argument keeps the `unsafe` in one place: the
  borrow ends before each `yield`, and the compiler checks that.
- Recursion and `dyn` dispatch need a `Box`. That allocates only when work
  pauses.
- It can land one pool at a time, beside the `Resume` enums it replaces.

What it risks:

- The repository pins a nightly. `coroutines` is unstable, as it was
  in 2016, and no plan to stabilise it is known. The feature can change, and
  each nightly bump is a CI image rebuild.
- A suspended task cannot be hashed or serialised. A paused state can be
  rebuilt by replaying its decisions, as the run files already do.
- The compile time with many coroutines is unknown.

### 4. Clone coroutines on stable, by `diapause`

`diapause` 0.2.0 is a proc macro that turns a function into a state
machine on stable Rust. Each suspension point is a variant of an enum
that holds the variables live there, so `#[derive(Clone)]` clones a
suspended coroutine. `yield_all!` delegates to a nested coroutine, and
`yield_all!(box sub)` recurses. It has no `unsafe`, and it allocates only
where a boxed delegate suspends.

The test from option 3 ran on it unchanged in behaviour:

- Two paused chains behind a caller that ran on, cloned and answered
  differently.
- Forks at each of three nested pauses of a recursive auto-play.
- About 1 ns on a path that does not pause, as a plain function.

Our own tests of shadowing, nested loops with `break`, `continue` and
`return`, drops, a clone inside a loop, and `?` pass too. Its random
differential test passes 2,000 cases on two seeds.

What it needs:

- A type that the macro can read for each local held across a suspension,
  such as `let x: u32 = ...`.
- A delegate in a typed local first: `let sub: f::State = f(io);`.
- Suspensions in statement position.
- A local patch for a borrowed world. Upstream puts the resume type's
  lifetime on the state, and then the world cannot hold its own paused
  tasks. The patch lends the world again at each step, and a binding that
  keeps it across a suspension does not compile. `vendor/README.md` lists
  the changes.
- No inner binding, such as a closure's parameter, with the name of an
  outer local. The macro counts it as a use and stores the outer one.

What it risks: few people use it. It has one author and no outside users,
and its first release came out on 2026-10-10. So the version is
pinned, and a reviewed copy is in `vendor/`, with a README that says what
was checked.

### Ruled out

- **Rust `async`.** The compiler builds the same state machines, but a
  future is never `Clone`.
- **Stackful coroutines** (`corosensei`, `may`). A suspended stack cannot
  be copied.
- **Replay from a checkpoint.** It cannot run on. The game runs the
  start of X, then other work W, then the end of X after the answer.
  Replay unwinds X and loses its locals. Replaying X's start after W
  applies it twice. In co-op the replays grow without bound. It is useful
  only to rebuild a paused state that was saved as decisions.
- **Effect-handler and generator crates** (`effing-mad`, `corophage`,
  `next-gen`). None clones a suspended state.
- **`genawaiter`.** It drives an `async` block, so its code reads well
  and many projects use it. But a suspended generator never clones, for
  the reason Rust `async` does not. It also costs about 25 ns on a path
  that does not pause, against about 1 ns for `diapause`.

## Other engines

No engine found clones a suspended, nested effect written as straight
code, or holds more than one suspended chain.

- StS1 simulators (`sts_lightspeed`, slay-the-cli) follow StS1, whose
  game is an action queue. A pause stops the queue.
- MTG and Hearthstone engines (phase.rs, SabberStone) keep effects as
  data and store the rest of the chain when a choice opens.
- Forge, XMage, Fireplace and Hearthbreaker answer a nested choice on the
  spot, at random or by a heuristic, and clone only at rest.

## A harness finding

The co-op harness clears a seat's in-flight flag once the game starts
each action the seat sent. The game can still be busy then. Hook
actions can still wait in a queue when the next seat's move goes in,
and the other seats can still be behind. An action cancelled as it is
queued never starts, so the flag can stay set. Both are read off the
code, not seen in a run. The fix belongs in the mod, whichever option
wins: a move returns once the queues are empty or this seat is asked.

## Decision

Option 4 at the pause sites, with the scheduler. Mark chose it on
2026-10-25, and it comes before co-op fights. It makes them simpler. Each
player's paused work is a coroutine in that player's queue. The fight
state loses the fields that only hold paused work.

The order:

1. A spike on the real engine (done). `SetupPlayerTurn`, where v0.111.0
   split 1 pauses for Hellraiser and Rampart, and the Silent pool's plays
   that stop are coroutines. The results:
   - `cargo xtask instructions`: +0.5%, all of it from the setup. The
     Silent pool added nothing.
   - A paused card play is a variant of the pool's coroutine, inline in
     the card's `Resume`. Nothing is boxed.
   - A paused fight is no bigger. `Live`, `Deferred` and the card's
     `Resume` keep their sizes, 4,112, 376 and 148 bytes, so a clone
     copies what it did before.
   - Compile time: a release build of `sts2-cli` after a change to core
     takes 150 s, against 148 s on main. A dev build is unchanged, at 4 s.
2. The pause sites in content move over, one pool at a time. Each takes
   away the `Resume` variants it replaces. They come before the scheduler
   because each pool stands alone, and the recorded runs prove it.
3. The scheduler with one player. It takes away the engine's `Deferred`
   work. The scenarios, the golden replays and the hunts prove it.
   `docs/SCHEDULER.md` is its design.
4. Co-op fights, on top.

## Open question

1. Do the run layer's pauses (`Pickup`, `Unfinished`) move with the
   fight's, or stay hand-written?
