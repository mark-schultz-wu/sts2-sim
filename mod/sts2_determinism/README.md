# sts2_determinism

A Slay the Spire 2 mod that makes the game **reproducible**: the same seed
produces the same run, on any machine, on any day.

It adds no content, changes no rules, and drives nothing. It exists so that
"the game is deterministic" is one concern owned in one place.

---

## ⚠️ This code is LLM-generated

**Every C# file in this directory was written by Claude, not by a human.** It has
not been reviewed line by line. Treat it as you would an unfamiliar dependency
from a stranger: useful, plausibly correct in the ways it has been tested, and
not to be trusted beyond that.

Where a claim below is checked by something automatic, it says so. Where it is
only an argument from reading the decompiled game, it says that too.

---

## Why this is a separate mod

These settings used to live inside the driver, scattered across three files:
`TestMode` at the entry point, the FTUE call during run setup, and the card and
reward selectors inside the protocol server. That last placement was a real bug —
the mod's own test mode ran *instead of* the protocol server, so it never
installed the selectors, and the suite exercised the game in a different
configuration than the thing it was testing.

## What it does

| when | what |
|---|---|
| once, at load | verifies the build pin, then `TestMode.IsOn = true` |
| per run, via `PrepareRun()` | `SetFtuesEnabled(false)` |

`TestMode` is the game's own test seam. It skips every `Cmd.Wait` and the
animation paths that check it — a determinism matter as much as a speed one,
since a wait that depends on frame timing has an outcome that depends on the
machine. Verified to gate presentation only: `MapCmd` applies its model change
unconditionally and skips just the icon refresh, and nothing calls
`TestMode.AssertOff`.

First-time-user prompts both gate progress — a driver waits forever on a prompt
nothing will answer — and write to disk, so whether a profile has seen one
becomes state that persists across runs and that no seed accounts for.

## Why it calls instead of patching

This was designed to patch, so that it held however the game was driven. **It
cannot.** Harmony throws on the first patch attempt:

```
HarmonyLib.HarmonyException: Patching exception in method
  MegaCrit.Sts2.Core.Models.EncounterModel::DebugRandomizeRng()
---> System.DllNotFoundException: Unable to load shared library '/tmp/mm-exhelper.so.…'
     undefined symbol: _Unwind_RaiseException
```

MonoMod's native exception helper will not load against this Godot .NET runtime.
That is almost certainly why this repository has referenced `0Harmony` since the
beginning and never once used it, and the reference has now been dropped.

So the per-run settings are a call, and `PrepareRun()` is the one place to make
it from. A driver that has to remember to call something can forget — but one
call to one named thing is a great deal harder to forget than four assignments
scattered through whatever happens to be driving.

## The patch that is still owed

`EncounterModel.DebugRandomizeRng` is:

```csharp
_rng = new Rng((uint)(DateTime.UtcNow - DateTime.UnixEpoch).TotalSeconds);
```

Wall clock, at one-second granularity. The damage is subtler than "adds
randomness". `GenerateMonstersWithSlots` seeds the encounter itself, but only
`if (_rng == null)`, from
`runState.Rng.Seed + runState.TotalFloor + hash(Id.Entry)`. Assigning `_rng`
early **pre-empts** that, so the wall-clock value is used *instead of* the
seed-derived one.

`FightConsoleCmd` calls it unconditionally, which is why console-driven fights
are nondeterministic — reproducing for runs landing in the same second and
diverging for runs a second apart, which presents as flakiness rather than as an
obvious break. Neutralising it would need only a no-op patch, since the fix is to
leave `_rng` null and let the game seed it. **That is blocked on Harmony
working.** Until then, the driver avoids `FightConsoleCmd` rather than fixing it,
and the console's command vocabulary stays unavailable for building scenarios.

## What it deliberately does **not** do

- **No content, no rules.** No cards, relics, enemies or balance changes.
- **It does not drive the game.** It has no notion of a run, a command, or an
  observation. Something else does that.
- **It does not patch `sts2.dll`.**
- **It does not touch your saves** — except that forcing FTUEs off means the
  game stops recording that you have seen them.
- **It is not a gameplay mod.** `TestMode` changes control flow; do not load this
  while playing.

## The `Active` contract

`Determinism.Active` is `true` only once the game has actually been configured.

Dependent mods must check it rather than assuming. The loader guarantees this mod
*loaded* first — it refuses to load a mod whose dependencies are absent, naming
them — but **loading and succeeding are different things**: a build mismatch
leaves this mod loaded, inert, and the game untouched. `sts2_headless` checks it,
and exits the process rather than returning, because a headless game with no
driver runs until something external kills it.

## The version pin

`Init` refuses to run unless `release_info.json` reports
`main_assembly_hash: 1692500715` (v0.107.1). Every claim this mod makes is a
claim about one build's behaviour, so a mismatch is a hard stop.

## How much of it is actually verified

**Verified automatically:** nothing specific to this mod. The 31 assertions in
`sts2_headless`'s `SelfTest` exercise a game configured by it, and they pass —
so a configuration that broke the driver would show up. They do not test
determinism itself.

**Verified once, by hand:** that both mods load in dependency order
(`0: sts2_determinism`, `1: sts2_headless`), that the cross-mod reference
resolves, and that `sts2_headless` refuses and exits when this one has not
activated — the last of which was found the hard way, as a hang.

**Not verified at all:** that this list is complete. Determinism is a claim about
everything the game does, and this mod covers two things somebody noticed.
`Rng.Chaotic` and other wall-clock reads have not been audited exhaustively.
