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

122 assertions, in two groups.

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

**That a card asking a question parks and can be answered** — `BurningPact`
selects one card from hand, so playing it blocks the game on a choice. The
selector the driver installs had no coverage beyond being asserted to exist. The
answer goes through `Pending.Accept` with the JSON a client would send, including
an out-of-range index, which must be refused rather than silently becoming a
different move.

**That an error-level log reaches the client once** — captured, drained, not
drained twice. Plumbing only: see its comment for why a real swallowed failure
could not be provoked.

**That a restart inherits nothing** — every case here restarts between
scenarios and assumes the next begins clean, and nothing checked it. The
assertion is the whole-state digest rather than a hand-picked list of fields,
since a list only catches leaks somebody already thought of.

Not mutation-tested, and the reason is recorded rather than papered over: the
obvious mutation, skipping `RunManager.CleanUp`, is caught by the game itself
with *"State is already set"*, so a leaky restart cannot easily be synthesised.
What is established is that the assertion is not vacuous — the paired check that
the run *did* change before restarting passes, so the digest is sensitive to
exactly the things a leak would carry over.

**That a whole fight can be played** — every other case takes one or two
decisions; this takes a fight's worth, which is the only way to exercise what
happens *between* them: enemy turns, turn transitions, the piles cycling, and
quiescence holding across dozens of decisions rather than two.

The scenario is what makes it a test rather than a gamble. A hundred Gorgets is
400 Plating at combat start, so the player cannot lose inside a few hundred
turns and the policy can be the dumbest available — first playable card, then
end the turn. A test whose policy has to be *good* fails when the policy is
wrong rather than when the driver is. It wins in 7 turns at full health.

That is scenario construction earning its keep: not reaching a state the game
would produce, but reaching one where the thing under test is the only variable.
Incidentally established — `RelicCmd.Obtain` accepts duplicates, which the real
game never offers.

**That winning a fight offers rewards** — reachable now that the console's
`kill all` works. The point is not that a reward can be taken but that the game
*asks*: with `TestMode` on and no `testSelector`, `RewardsSet.Offer` takes every
reward itself, and an oracle that quietly pockets them produces observations no
simulator can reproduce.

**That travelling moves the run and changes what is next** — the map cases are
otherwise all about which points are *legal*, and none of them travel. The
interesting assertion is the third: a simulator tracking position but not the
map's shape would pass a check on `at` alone and diverge on the floor after.

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
