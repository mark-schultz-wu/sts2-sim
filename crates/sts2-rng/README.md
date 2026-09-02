# sts2-rng

The random numbers of the game, in Rust.

Slay the Spire 2 draws each card, rolls each reward, and picks each monster move
from a seeded stream. To play the same run two times, you need the same numbers
in the same order. This crate is that stream, ported from the game.

## Checked against the real game

Nobody trusts this port because it looks correct. A test compares it against
1024 draws for each seed, taken from the game. That test is in
`tests/ground_truth.rs`.

If the port drifts, that test fails. It is the only reason to trust anything
here.

## What it holds

This crate holds these types:

- `MegaRng`. The stream. It is xoshiro256\*\*, seeded through splitmix64, and
  counted.
- `Bound`, `Span`, and `Interval`. The shapes that a draw can ask for. A bad
  range fails when you build it, not when you draw from it.
- `stream_seed`. It turns a `Seed` into the number that the streams of a run
  start from.

## Where Seed lives

`Seed` is in `sts2-core`, not here. Anything that drives a game names a seed,
and it must not compile a random number generator to do that.

So `Seed` is a word, and this crate is the machine that uses the word.

## Who uses this crate

Almost nothing, at this time. The fuzzer uses `stream_seed` for its own dice.
The real game draws its own numbers inside itself.

This crate was written for the simulator. The simulator must produce the same
numbers as the game. If it does not, nothing else it does can agree.
