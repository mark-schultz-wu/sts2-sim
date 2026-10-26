# Vendored crates

## diapause 0.2.0

`diapause`, `diapause-macro` and `diapause-macro-core` are the crates as
crates.io publishes them, with the local changes below. The root `Cargo.toml` pins
`diapause = "=0.2.0"` and builds all three from here through
`[patch.crates-io]`.

`diapause` turns a function into a state machine whose suspended state is
`Clone`, on stable Rust. The simulator needs that for the rules that pause
on a player's choice, as `docs/ASYNC.md` explains. It is what we need, so
it is better than writing our own.

But few people use it. On 2026-10-25 it had one author and no outside
users or issues. An AI model co-wrote most of its commits. Its
first release came out on 2026-10-10. So the pin and the copy are on purpose:

- The code cannot change under us. A new version is read before the pin
  moves.
- The code stays here if the project stops.

What was checked before it came in, on 2026-10-25:

- The published source matches the `v0.2.0` tag of
  <https://github.com/todesking/diapause> file for file.
- It depends on `syn`, `quote` and `proc-macro2` alone. The macro code
  reads no files, environment, network or processes. The code it
  generates has no `unsafe`.
- The one build script, in its test crate, writes generated test cases to
  `OUT_DIR` and does nothing else.
- Its own suite passes. Its random differential test passes 2,000 cases
  on two seeds and 1,400 on a third, the most that compile in 32 GB. That test runs each random body as a plain
  function and as a state machine, and clones and serialises the state at
  each suspension.
- Our own tests pass: shadowing, nested loops with `break`, `continue` and
  `return`, drops, a clone in the middle of a loop, and `?`.

## Local changes

A paused rule is stored inside the fight it works on, and the fight is
cloned while the rule is paused. So the rule borrows the fight for one step
at most. Upstream puts the resume value's lifetime on `State`, and then the
fight borrows itself. These changes lend the fight again at each
resume instead, with no `unsafe`:

- `resume = for<'f> T<'f>`. The lifetimes go on the `Coroutine` impl and
  on `__drive`, never on `State`. A binding that keeps the borrow into the
  state is a compile error.
- `yield_all!` carries the child's yielded value across its loop, not the
  resume value, so delegation works with a borrowed resume value.
- `resumed!()` borrows the current step's resume value where it is, and
  `State::start_with(value)` gives the first step one too. These are ours
  alone.

The first two are offered upstream in
<https://github.com/todesking/diapause/issues/2>, with the patch on
<https://github.com/mark-schultz-wu/diapause/tree/borrowed-resume>. The
upstream suites still pass with all the changes.
