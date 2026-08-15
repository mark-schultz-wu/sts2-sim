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

32 assertions across all nine of the game's legality-hook overrides. Notable
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
