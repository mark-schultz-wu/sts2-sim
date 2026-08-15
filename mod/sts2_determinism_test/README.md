# sts2_determinism_test

Measures what [`sts2_determinism`](../sts2_determinism/README.md) claims, rather
than arguing it.

Install it to run the checks. Leave it out to ship the patches with no tests
attached.

---

## ⚠️ This code is LLM-generated

**Every C# file in this directory was written by Claude, not by a human.** It has
not been reviewed line by line.

---

## Running it

```bash
just determinism-test
```

Which deploys the mods and sets `STS2_HEADLESS_MODE=determinism`. This mod claims
that mode; `sts2_headless` claims `serve`, `sts2_headless_test` claims
`selftest`. All three go through `Driver.Run`, so they bring a run up the same
way and differ only in what they do with it.

Exits non-zero on failure, via `Driver.ExitCode`.

## What it checks

Six assertions across two claims.

**That patching `DebugRandomizeRng` restores seed-derived encounter RNG:**

1. A fresh encounter has no RNG yet.
2. **`DebugRandomizeRng` leaves it unset** — the patch working.
3. The RNG that generation does install is seeded with exactly
   `runState.Rng.Seed + runState.TotalFloor + hash(Id.Entry)`, the formula in
   `GenerateMonstersWithSlots`.
4. That seed is not a recent Unix timestamp.

**That tutorials are suppressed without writing to the profile:**

5. An unseen tutorial reports as already seen — *with the profile's own
   `EnableFtues` flag left on*, so the patch is what answers, not the stored
   state.
6. Asking did not turn that flag off.

Check 5 is set up carefully, because the first version was vacuous. It asked
about `"shuffle_ftue"`, which this profile had already completed — so unpatched
`SeenFtue` answered `true` from `FtueCompleted` and the mutation test survived.
It now asks about a fresh GUID, which cannot be in the completed set.

It also cannot assert `progress.save` is untouched: a run writes it for reasons
unrelated to tutorials, since `RelicCmd.Obtain` calls `MarkRelicAsSeen` and
fighting records a discovery.

## Why not "build the same fight twice and compare"

That was the obvious test and it is close to worthless here. `BowlbugsWeak`
picks its second monster from **two** options, so an unpatched build agrees with
itself half the time by chance. A regression would surface as a coin flip — and a
flaky failure gets rerun until it passes, which is worse than not testing at all.

Asserting the seed actually used is deterministic and decisive. Check 4 exists
because it makes the failure legible: a wall-clock seed is seconds since the
epoch, around 1.79 billion and climbing, and seeing that number next to "now"
identifies the bug immediately.

## Mutation-tested

The patch was reverted — `Prefix() => true`, letting the original run — and the
suite re-run. Three of the four checks failed:

```
FAIL DebugRandomizeRng leaves the rng unset
FAIL the rng is seeded from the run, not the clock (got 1786751429, expected 2426631198)
FAIL the rng is not a recent wall-clock value (1786751429 vs ~1786751429)
1 passed, 3 failed
```

`1786751429` was the current Unix time to the second. A test that has never been
seen to fail is not evidence; this one has.

## What it does not cover

Determinism generally. This measures one patch. `Rng.Chaotic` and other
wall-clock reads have not been audited, and nothing here replays a whole run and
compares digests — that check lives on the Rust side, in
`the_same_seed_gives_the_same_digest`.

## The cost of these assertions

Checks 1–3 read `EncounterModel._rng` by reflection, because the game exposes it
only as `protected`. This test therefore knows a private field name and will
break if the game renames it. It breaks loudly, with a message saying so, which
is the trade being made.
