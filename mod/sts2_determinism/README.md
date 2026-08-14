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

| what | how |
|---|---|
| verifies the build pin, then `TestMode.IsOn = true` | at load |
| encounter RNG stays seeded from the run, not the clock | patches `EncounterModel.DebugRandomizeRng` to do nothing |
| first-time-user tutorials never appear | patches `ProgressSaveManager.SeenFtue` to return `true` |

`TestMode` is the game's own test seam. It skips every `Cmd.Wait` and the
animation paths that check it — a determinism matter as much as a speed one,
since a wait that depends on frame timing has an outcome that depends on the
machine. Verified to gate presentation only: `MapCmd` applies its model change
unconditionally and skips just the icon refresh, and nothing calls
`TestMode.AssertOff`.

First-time-user prompts both gate progress — a driver waits forever on a prompt
nothing will answer — and, once dismissed, write to the profile, so whether one
has been seen becomes state that persists across runs and that no seed accounts
for.

`SeenFtue` is patched rather than the `EnableFtues` property behind it, and
rather than calling `SetFtuesEnabled(false)` as the driver used to. Both
alternatives write: the setter calls `SaveProgress()`, and the property is read
by `ProgressState`'s own serialisation, so forcing it would persist
`EnableFtues = false` into the player's save. Patching the one question the game
actually asks changes no stored state at all.

## Launching requires `LD_PRELOAD=libgcc_s.so.1`

**Without it every patch throws and this mod stays inert.** Harmony fails with:

```
HarmonyLib.HarmonyException: Patching exception in method
  MegaCrit.Sts2.Core.Models.EncounterModel::DebugRandomizeRng()
---> System.DllNotFoundException: Unable to load shared library '/tmp/mm-exhelper.so.…'
     undefined symbol: _Unwind_RaiseException
```

MonoMod writes a native helper to `/tmp` and `dlopen`s it. That helper has **no
`DT_NEEDED` entries at all** — `readelf -d` shows none — while importing
`_Unwind_RaiseException`, `_Unwind_SetGR`, `_Unwind_SetIP` and friends from
libgcc. It relies entirely on the host process having libgcc loaded already, and
`SlayTheSpire2` does not link it. Preloading puts those symbols in the global
namespace before the helper is opened.

`sts2_headless`'s Rust client sets it when launching, and `just selftest` sets it
too. Anything else that starts the game must do the same. The failure is
particularly unkind if you forget: patches throw, `Active` stays false, the
driver refuses, and — before this was noticed — the process hung rather than
exiting, with the reason in a log nobody was reading.

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
resolves, that `sts2_headless` refuses and exits when this one has not activated
— found the hard way, as a hang — and that the patches apply under `LD_PRELOAD`
and throw without it.

**Applied but not yet measured:** that the `DebugRandomizeRng` patch produces
identical fights across seconds. The reasoning is above and rests on reading
`GenerateMonstersWithSlots`; what would settle it is two console-built fights on
one seed a second apart, compared by digest — and that needs the scenario command
`sts2_headless` does not have yet.

**Not verified at all:** that this list is complete. Determinism is a claim about
everything the game does, and this mod covers two things somebody noticed.
`Rng.Chaotic` and other wall-clock reads have not been audited exhaustively.
