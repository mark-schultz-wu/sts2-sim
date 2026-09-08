# sts2_headless_test

The legality suite for [`sts2_headless`](../sts2_headless/README.md), run inside
the real game.

Install it to run the tests. Leave it out to ship a driver with no tests in it.
That is the reason it is a separate mod.

---

## ⚠️ This code is LLM-generated

**Every C# file in this directory was written by Claude, not by a human.** It has
not been reviewed line by line.

---

## Why it has to run inside the game

Most of what it checks can be driven over the protocol from outside. Free travel
widening the map, or a blocked card reporting its reason. One part cannot.

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
claims that mode. `sts2_headless` claims `serve` and ignores it. Both go through
`Driver.Run`, so the suite and the protocol server bring a run up exactly the
same way. They differ only in what they do with it.

Exits non-zero on failure, so it is usable as a gate.

## What it covers

144 assertions, in two groups.

**That the observation is complete (H4).** Every field the observation promises
is in it, out of combat and in, down to each creature reporting its intent. The
other assertions here are about legality, and none of them notices the
observation quietly losing a field. That is the failure this project is least
able to survive. A simulator defaults what nobody tells it, the oracle holds the
real value, and the two agree because neither knows the field exists. A
divergence that never appears is worse than one that does.

The key lists are written out rather than derived from the capture. A derived
list only asserts that the code does what the code does. Written down, removing
a field fails, and adding one is a deliberate edit.

**That a command answers only after the game settles (H3).** This is the
standing hazard of this codebase, with four recorded bugs where the driver
answered before the work finished. Asserting `ReadyForInput` proves nothing.
That is the predicate `Quiesce` waits on, and every one of those bugs *was* that
predicate returning true too early. So it uses a signal the predicate does not
consult. Take the digest, let the engine run, take it again. A stable digest is
evidence that the state settled. An unstable one proves that it did not.

Ending a turn is the case that carries this. A card play survives a mutation
that stops the driver waiting at all, because the action's own completion does
most of the work. Only a turn transition needs the turn counter to advance, and
only that check fails when quiescence is broken.

**That a card asking a question parks and can be answered.** `BurningPact`
selects one card from hand, so playing it blocks the game on a choice. The
selector the driver installs had no coverage beyond being asserted to exist. The
answer goes through `Pending.Accept` with the JSON a client sends. That includes
an out-of-range index, which must be refused rather than silently becoming a
different move.

**That an error-level log reaches the client once.** The mod captures it, drains it,
and never drains it twice. This checks plumbing only. See its comment for why the test
provokes no real swallowed failure.

**That a restart inherits nothing.** Every case here restarts between scenarios
and assumes the next begins clean, and nothing checked it. The assertion is the
whole-state digest rather than a hand-picked list of fields. A list only catches
leaks somebody already thought of.

Not mutation-tested, and the reason is recorded rather than papered over. The
obvious mutation, skipping `RunManager.CleanUp`, is caught by the game itself
with *"State is already set"*. So a leaky restart is hard to synthesise. What is
established is that the assertion is not vacuous. The paired check that the run
*did* change before restarting passes, so the digest is sensitive to exactly the
things a leak carries over.

**That a whole fight can be played.** Every other case takes one or two
decisions. This takes a fight's worth, which is the only way to exercise what
happens *between* them. That means enemy turns, turn transitions, the piles
cycling, and quiescence holding across dozens of decisions rather than two.

The scenario is what makes it a test rather than a gamble. A hundred Gorgets is
400 Plating at combat start. So the player cannot lose inside a few hundred
turns, and the policy can be the dumbest available: first playable card, then
end the turn. A test whose policy has to be *good* fails when the policy is
wrong rather than when the driver is. It wins in 7 turns at full health.

That is scenario construction earning its keep. It does not reach a state the
game produces. It reaches one where the thing under test is the only variable.
Established incidentally: `RelicCmd.Obtain` accepts duplicates, which the real
game never offers.

**That the run ends when the player dies**, and says which way. This rule has a
recorded failure behind it. A dead player was once observed healing at a rest
site. `victory` is asserted null while the run is live. Before the end it is not
a meaningful question, and a simulator reading `false` reads a claim nobody
made.

**That replaying a script gives the same sequence.** D1 was checked one step
deep, so a run agreeing at step one and diverging at step five passed. This
compares twelve steps, and checks they are *distinct* first. Two lists of
identical digests agree while proving nothing. It is also the shape the diff
harness uses.

**That all five characters can be driven.** Everything else here is the
Ironclad. The Defect matters most. Its orbs were reported as empty for as long
as nobody drove a Defect run. The fields existed, and nothing ever
wrote or read them. The gap was reachability, not difficulty.

**It found a driver bug**, which is the argument for driving all five rather
than one. The Necrobinder played 137 turns and left the enemies on 83 hp, their
full starting total. Then it died when its Plating ran out. `PlayOneCard` chose
its target with `!c.IsPlayer`, and a summoned ally is not the player. So once the
Necrobinder had a Bodyguard out, every Strike went into its own minion. Nothing
errored, because attacking your own minion is legal.
`Side == CombatSide.Enemy` fixes it, and all five now win in five or six
turns.

Only the Necrobinder summons. That is why four characters' worth of testing
missed it, and why the victory is asserted rather than mere survival. "It
survived" calls 137 futile turns a pass.

Writing it also turned up a limit that is not a bug. The first version hung on
the Silent. The Ironclad's opening deck asks nothing, so "play the first card"
never parks a decision. The moment a character's cards *do* ask, the game waits
for an answer that a policy of playing cards cannot give. Any policy driving
more than one character has to be able to say something.

**That winning a fight offers rewards.** This is reachable now that the
console's `kill all` works. The point is not that a reward can be taken. It is
that the game *asks*. With `TestMode` on and no `testSelector`, `RewardsSet.Offer`
takes every reward itself, and an oracle that quietly pockets them produces
observations no simulator can reproduce.

**That travelling moves the run and changes what is next.** The map cases are
otherwise all about which points are *legal*, and none of them travel. The third
assertion is the interesting one. A simulator tracking position but not the
map's shape passes a check on `at` alone, and diverges on the floor after.

**That the legal command set is right (H2).** This covers all nine of the game's
legality-hook overrides. Two are worth noting, because they were written wrong
first:

- Three check the **preventer** a blocked card reports, against the model
  `CanPlay` itself picked rather than against an expected name. Written the other
  way round they failed. An afflicted card is blocked by the power its affliction
  applied, not by the affliction. Powers come first in the hook listener order,
  and `ShouldPlay` returns the first refusal.
- The map cases restart the run and assert the same map regenerates, because a
  test that compares two different maps proves nothing.

Several are **mutation-tested.** We deliberately broke the driver to confirm
that the test fails, then fixed it to confirm that the test passes. A test that
nobody ever saw fail is not evidence.

## What it does not cover

Determinism itself. These assertions run against a game configured by
`sts2_determinism`, so a configuration that broke the driver shows up here. But
nothing in this suite replays a seed and compares digests. That check lives on
the Rust side, in `the_same_seed_gives_the_same_digest`.
