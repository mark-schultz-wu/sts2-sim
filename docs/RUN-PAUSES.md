# The run layer's pauses, as coroutines

Status: a design, 2026-10-26. Mark chose on 2026-10-26 to move the run
layer's pauses to `diapause` coroutines before co-op fights, so every pause
is held the same way. `docs/ASYNC.md` has the reasons for coroutines.

## Where the run layer stands

Outside a fight, a relic's pickup, an event's option and a rest site's
option hand the run a list of effects. `State::carry_out` runs them in
order. The first effect that asks a question stops it, and the question
comes back as an `Asked`:

- `pickup`: the question, a `Pickup`. It is a card selection from the
  deck, cards or bundles to take, a reward screen, the event's own
  question, or the abandon-run popup.
- `rest`: the effects after it, which wait for the answer as the game's
  `await` does. A walk over a hook's listeners that a question stopped
  waits there too, as a `Then::Walk`.
- `under`: the question this one interrupted. A relic taken off a reward
  screen asks from inside it, since `RewardsSet` awaits the pickup.

A room parks on the `Asked`: Neow, an event, a rest site and a shop each
hold one. `State::answer` applies the answer by the kind of `Pickup`. Then
`done_asking` runs `rest`, takes up `under`, and finishes what the pickup
was part of: a shop's purchase or a rest site's option.

A shop's purchase waits on its own: `Unfinished` names a card removal, a
clone a relic asks for, or a relic's pickup. Each is the rest of
`OnTryPurchaseWrapper`, which the game awaits.

These are hand-written continuations. `rest` and `under` are the frames
of nested `await`s, as `Deferred` was in a fight.

What stays: the reward screen after a fight (`Phase::Rewards`) and an
event's own state. Each is a screen the player works through, held as
data, and not a function stopped part-way.

## The target

1. **A pickup is a coroutine.** `carry_out` becomes `pickup`, a
   `diapause` coroutine over the effects. It yields the `Pickup` it asks,
   and it is resumed with the run lent for one step and the answer. What
   `answer` does for each kind of `Pickup` moves into it, after its yield.
2. **Nesting is the coroutine stack.** An effect that runs effects of its
   own, such as a relic obtained inside the list or an event resumed,
   delegates to a boxed `pickup` with `yield_all!`. `rest` and `under`
   go, and so does `Then`. A walk that a question stops is a loop in the
   coroutine.
3. **A room parks on the coroutine.** It holds the question shown and the
   paused `pickup`. The observation and the legal answers read the
   question, as they read `Asked::pickup` now.
4. **A purchase is a coroutine.** `purchase` spends, obtains or removes,
   and clears the shelf after, with Lord's Parasol's next shelf.
   `Unfinished` goes.

The run lent for a step is `&mut State`. The paused coroutine comes out
of its room for the step, as a fight's coroutines come out of `Live`.

## Cost

The run layer runs once per room, against about 13,000 instructions a
step in a fight. A pickup that asks nothing runs to its end on its first
step, and no room holds it. `cargo xtask instructions` checks each step.

## The order of the work

Each step is one PR, with the golden replays, the census and
`cargo xtask instructions` against the step before.

1. **`pickup`.** `carry_out`, `Asked`, `rest`, `under`, `Then` and the
   answer for each kind of `Pickup` become the coroutine. The rooms park
   on it.
2. **`purchase`.** The shop's `Unfinished` becomes the coroutine, which
   delegates to `pickup` for a relic's.
