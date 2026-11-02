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

### Joins copied on the path that does not suspend

A block where two paths meet, after an `if` or a `match` whose arms can
yield, was a state of its own. Every run that reached it moved its live
locals into the state enum and back out through the dispatch loop, even
when nothing suspended. A body that rarely stops paid that on each run.
The simulator's end of turn spent about 1,000 instructions a turn on it.

`cfg::inline_joins` runs after the lowering, in `expand` and
`expand_debug`. It copies a join for the paths into it, with the blocks
only it leads to. Each copy is plain Rust in the arm that reaches it. A
yield in a copy comes with its resume point, so each copy has its own. A
join on a loop stays a state, and so does a block that defines a direct
borrow. A copied block's jump markers get fresh jumps of their own. A
budget of the body's own size, and at least 128 statements, bounds the
copies.

The path that does not suspend gets its copies first. A path that
resumed from a yield came through the dispatch loop already, so it can
reach a join as a state. Copied for every path, a chain of joins with a
yield before each doubles at each step. On the simulator's turn loop,
that used up the budget before the turn's start, which then cost +0.5%
in `cargo xtask instructions`. Copied for the hot
paths first, the chain grows by one copy of each join, and the cost goes.
The budget left over then goes to every path, as fewer states make a
smaller enum to compile.

The lowering's own tests see the CFG without the copies. On the fork,
upstream's macro-core suite, runtime suite and compile-fail cases pass.
One compile-fail case,
`value_let_missing_annotation`, had to hold its value across a second
yield, since the copy took away the first state it crossed. The random
differential test passes 2,000 cases on two seeds. On 1,000 cases of the
second seed, rustc peaks at 12.9 GB, against 19.4 GB with no pass. The
test takes 83 s, against 129 s. At 2,000 cases it peaks at 27.6 GB, close
to what the machine holds. Copying every path alike peaked at 21.9 GB.
`sts2-content` and `sts2-sim` build in the same time and memory with
either. This change is ours alone.

### A spent inclusive range saves the same state on every toolchain

A `for` loop over `a..=b` keeps its place in a `__RangeInclusiveIter`,
with an explicit `done` flag, because std's `RangeInclusive` does not
serialize its own. After each element the wrapper copied back the
`start` that std's `next` left. Where std leaves `start` after the last
element changed between releases. Rust 1.99 moves it past `end`, so
`1..=1` saves as `start: 2`, and the serde test that expects 1 failed on
upstream's main too. Now the wrapper copies `start` back only while the
range has elements left. The saved state is then the same on every
toolchain, and the serde suite passes. The fix is offered upstream in
<https://github.com/todesking/diapause/pull/3>.
