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

## The oldest build this can model

v0.107.1 and later. That build replaced the game's PRNG with xoshiro256\*\*,
after a player showed that the streams of a run were correlated. Read
[the writeup](https://tck.mn/blog/correlated-randomness-sts2/) and
[the patch notes](https://slaythespire.wiki.gg/wiki/Slay_the_Spire_2:V0.107.1_-_Major_Update_2).

Every build before it drew from C# `System.Random` instead. `MegaRng` cannot
produce those numbers, and no amount of adapting the mods will change that. To
pin a build older than v0.107.1, port `System.Random` here first, and give it
its own ground truth.

This is why `mod/games/` starts at v0.107.1.

## One stream, two ways of seeding it

The draw stream is the same on both pinned builds. `the_two_builds_draw_the_same_numbers`
holds the 1028 draw rows of one dump against the other, and they are equal.

What v0.111.0 moved is how a stream is *named*, and `Seeding` is that:

| | hash | over | seed width |
|---|---|---|---|
| `Interleaved` | two interleaved `i32` lanes | UTF-16 code units | `u32` |
| `XxHash` | XXH64, seeded with zero | UTF-8 bytes | `u64` |

The width is not a detail beside the hash. On `Interleaved` the sum of a run's
seed and a name's hash wraps at 2³². On `XxHash` it does not, so the same two
numbers added together land somewhere else.

Both are replayed against their own dump's `hash` rows, which carry the hash and
the seed a named stream starts from. `XxHash` also has to reproduce the
published XXH64 vector for the empty input.

## What it holds

This crate holds these types:

- `MegaRng`. The stream. It is xoshiro256\*\*, seeded through splitmix64, and
  counted. One of these serves every pinned build.
- `Seeding`, with `Interleaved` and `XxHash`. How a build turns a name into the
  number a stream starts from.
- `Bound`, `Span`, and `Interval`. The shapes that a draw can ask for. A bad
  range fails when you build it, not when you draw from it.
- `Seeding::stream_seed`. It turns a `Seed` into the number that the streams of
  a run start from.

## Where Seed lives

`Seed` is in `sts2-core`, not here. Anything that drives a game names a seed,
and it must not compile a random number generator to do that.

So `Seed` is a word, and this crate is the machine that uses the word.

## Who uses this crate

Almost nothing, at this time. The fuzzer uses a `stream_seed` for its own dice,
and picks one build's because those dice are never the game's. The real game
draws its own numbers inside itself.

This crate was written for the simulator. The simulator must produce the same
numbers as the game. If it does not, nothing else it does can agree.
