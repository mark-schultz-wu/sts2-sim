# sts2_headless_test

The legality suite for [`sts2_headless`](../sts2_headless/README.md), run inside
the real game.

Install it to run the tests. Leave it out to ship a driver with no tests in it —
which is the reason it is a separate mod.

---

## ⚠️ This code is LLM-generated

**Every C# file in this directory was written by Claude, not by a human.** It has
not been reviewed line by line.

---

## Why it has to run inside the game

Most of what it checks could be driven over the protocol from outside — free
travel widening the map, a blocked card reporting its reason. One part cannot:

`Discover()` reflects over the game assembly to find every legality hook, and
compares that set against the cases the suite covers. That is what makes a hook
added by a future patch show up as `UNCOVERED` rather than silently going
untested. A test that only knows the hooks somebody remembered to list is a test
that agrees with whatever the code already does.

The afflicted-card cases are generic over game types (`TPower`, `TAffliction`)
for the same reason.

## Running it

```bash
just selftest
```

Which deploys all three mods and sets `STS2_HEADLESS_MODE=selftest`. This mod
claims that mode; `sts2_headless` claims `serve` and ignores it. Both go through
`Driver.Run`, so the suite and the protocol server bring a run up exactly the
same way — they differ only in what they do with it.

Exits non-zero on failure, so it is usable as a gate.

## What it covers

89 assertions, in two groups.

**That the observation is complete (H4)** — every field the observation promises
is in it, out of combat and in, down to each creature reporting its intent. The
other assertions here are about legality, and none of them would notice the
observation quietly losing a field. That is the failure this project is least
able to survive: a simulator defaults what it is not told, the oracle has the
real value, and the two agree because neither knows the field exists. A
divergence that never appears is worse than one that does.

The key lists are written out rather than derived from the capture, which would
only assert that the code does what the code does. Written down, removing a
field fails and adding one is a deliberate edit.

**That a command answers only when the game has settled (H3)** — the standing
hazard of this codebase, with four recorded bugs where the driver answered
before the work finished. Asserting `ReadyForInput` would prove nothing, since
that is the predicate `Quiesce` waits on and every one of those bugs *was* that
predicate returning true too early. So it uses a signal the predicate does not
consult: take the digest, let the engine run, take it again. A stable digest is
evidence the state had settled; an unstable one is proof it had not.

Ending a turn is the case that carries this. A card play survives a mutation
that stops the driver waiting at all, because the action's own completion does
most of the work — only a turn transition needs the turn counter to advance, and
only that check fails when quiescence is broken.

**That the legal command set is right (H2)** — all nine of the game's
legality-hook overrides. Notable
ones, because they were written wrong first:

- Three check the **preventer** a blocked card reports, against the model
  `CanPlay` itself picked rather than against an expected name. Written the other
  way round they failed: an afflicted card is blocked by the power its affliction
  applied, not by the affliction, because powers come first in the hook listener
  order and `ShouldPlay` returns the first refusal.
- The map cases restart the run and assert the same map regenerates, because a
  test that compares two different maps proves nothing.

Several are **mutation-tested**: the driver was deliberately broken to confirm
the test fails, then fixed to confirm it passes. A test that has never been seen
to fail is not evidence.

## What it does not cover

Determinism itself. These assertions run against a game configured by
`sts2_determinism`, so a configuration that broke the driver would show up here —
but nothing in this suite replays a seed and compares digests. That check lives
on the Rust side, in `the_same_seed_gives_the_same_digest`.
