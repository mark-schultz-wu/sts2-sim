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
that mode. `sts2_headless` claims `serve`, and `sts2_headless_test` claims
`selftest`. All three go through `Driver.Run`, so they bring a run up the same
way, and differ only in what they do with it.

Exits non-zero on failure, via `Driver.ExitCode`.

## What it checks

Five assertions across two claims.

**That patching `DebugRandomizeRng` restores seed-derived encounter RNG:**

1. A fresh encounter has no RNG yet.
2. **`DebugRandomizeRng` leaves it unset.** That is the patch working.
3. The RNG that generation does install is seeded with exactly
   `runState.Rng.Seed + runState.TotalFloor + hash(Id.Entry)`, the formula in
   `GenerateMonstersWithSlots`.

**That tutorials are suppressed without writing to the profile:**

4. An unseen tutorial reports as already seen, *with the profile's own
   `EnableFtues` flag left on*. So the patch is what answers, not the stored
   state.
5. Asking did not turn that flag off.

How check 3 is made is per build. On v0.107.1 it compares the seed the game
stored. On v0.111.0 an `Rng` has no seed, so it builds the expected one, takes
the draw the encounter takes, and compares the whole state. `Compat/` holds each
version of it.

Check 4 is set up carefully, because the first version was vacuous. It asked
about `"shuffle_ftue"`, which this profile had already completed. So unpatched
`SeenFtue` answered `true` from `FtueCompleted`, and the mutation test survived.
It now asks about a fresh GUID, which cannot be in the completed set.

It also cannot assert `progress.save` is untouched. A run writes it for reasons
unrelated to tutorials. `RelicCmd.Obtain` calls `MarkRelicAsSeen`, and fighting
records a discovery.

## Why not "build the same fight twice and compare"

That was the obvious test, and it is close to worthless here. `BowlbugsWeak`
picks its second monster from **two** options, so an unpatched build agrees with
itself half the time by chance. A regression surfaces as a coin flip. A flaky
failure gets rerun until it passes, which is worse than not testing at all.

Asserting the seed actually used is deterministic and decisive. The failure is
legible too, because the check prints both seeds. A wall-clock seed is seconds
since the epoch, around 1.79 billion and climbing. Seeing that number next to
the one the run derives identifies the bug on sight.

There used to be a fourth check, saying the seed was not a recent timestamp. It
is gone. The seed the check above expects is derived from the run, so a clock
seed already fails it. From v0.111.0 an `Rng` exposes no seed to read. A check
that runs for one ABI family and not the other is worse than none.

## Mutation-tested

The patch was reverted, with `Prefix() => true` letting the original run, and
the suite was run again. Two of the five assertions failed:

```
PASS a fresh encounter has no rng yet
FAIL DebugRandomizeRng leaves the rng unset
FAIL the rng is seeded from the run, not the clock (seed 1788463306, expected seed 2423574609)
PASS an unseen tutorial reports as already seen, with FTUEs enabled
PASS asking did not turn the profile's FTUE flag off
3 passed, 2 failed
```

`1788463306` was the current Unix time to the second. A test that has never been
seen to fail is not evidence. This one has failed.

## What it does not cover

Determinism generally. This measures one patch. `Rng.Chaotic` and other
wall-clock reads have not been audited. Nothing here replays a whole run and
compares digests. That check lives on the Rust side, in
`the_same_seed_gives_the_same_digest`.

## The cost of these assertions

Checks 1–3 read `EncounterModel._rng` by reflection, because the game exposes it
only as `protected`. This test therefore knows a private field name, and it
breaks if the game renames it. It breaks loudly, with a message saying so. That
is the trade being made.
