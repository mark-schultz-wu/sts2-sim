# sts2_determinism

A Slay the Spire 2 mod that makes the game **reproducible**. The same seed
produces the same run, on any machine, on any day.

It adds no content, changes no rules, and drives nothing. It exists so that
"the game is deterministic" is one concern owned in one place.

---

## ⚠️ This code is LLM-generated

**Every C# file in this directory was written by Claude, not by a human.** It has
not been reviewed line by line. Treat it as an unfamiliar dependency from a
stranger. It is useful, plausibly correct in the ways it has been tested, and
not to be trusted beyond that.

Where a claim below is checked by something automatic, it says so. Where it is
only an argument from reading the decompiled game, it says that too.

---

## Why this is a separate mod

These settings used to live inside the driver, scattered across three files.
`TestMode` was at the entry point, the FTUE call was in run setup, and the card
and reward selectors were inside the protocol server. That last placement was a
real bug. The mod's own test mode ran *instead of* the protocol server, so it
never installed the selectors. The suite then exercised the game in a different
configuration than the thing it was testing.

## What it does

| what | how |
|---|---|
| verifies the build pin, and patches the game | at load |
| `TestMode.IsOn = true` | `Determinism.Arm`, called once the loader has finished |
| encounter RNG stays seeded from the run, not the clock | patches `EncounterModel.DebugRandomizeRng` to do nothing |
| first-time-user tutorials never appear | patches `ProgressSaveManager.SeenFtue` to return `true` |
| a canonical monster's RNG being read is reported | patches `MonsterModel.Rng` to warn, once per monster id |
| MonoMod's leftover native helpers are deleted | sweeps the temp directory at load |

`TestMode` is the game's own test seam. It skips every `Cmd.Wait`, and the
animation paths that check it. That is a determinism matter as much as a speed
one. A wait that depends on frame timing has an outcome that depends on the
machine. It was verified to gate presentation only. `MapCmd` applies its
model change unconditionally and skips just the icon refresh, and nothing calls
`TestMode.AssertOff`.

It is turned on after the mod loader has finished, not at load. From v0.111.0
the loader reads a mod's DLL only while `TestMode.IsOff`. This mod loads first,
so turning the seam on any earlier costs every mod behind it its assembly. The
loader reports that as one warning per mod and carries on.

`Driver.Run` is the single caller, because it is the one point every driver
passes through, and it already checks that this mod activated.

First-time-user prompts do two unwanted things. They gate progress, so a driver
waits forever on a prompt nothing answers. Once dismissed, they write to the
profile. Whether one has been seen then becomes state that persists across runs,
and that no seed accounts for.

`SeenFtue` is patched rather than the `EnableFtues` property behind it, and
rather than calling `SetFtuesEnabled(false)` as the driver used to. Both
alternatives write. The setter calls `SaveProgress()`, and the property is read
by `ProgressState`'s own serialisation, so forcing it persists
`EnableFtues = false` into the player's save. Patching the one question the game
actually asks changes no stored state at all.

## What the `Rng.Chaotic` audit found

`REQUIREMENTS.md` asks that every adopted command be audited for `Rng.Chaotic`,
`DebugRandomizeRng` and wall-clock reads. That had never been done. There are
**193** uses of `Rng.Chaotic`, a process-wide stream seeded once from the clock.

Nearly all are presentation. They cover monster skin variants (`TwoTailedRat`,
`Toadpole`), merchant dialogue lines, screen shake, leaf positions in
`DenseVegetation`, and card-deal animation timing in `CardPileCmd`.

Two were worth reading closely, and both are benign:

- **`YummyCookie`** uses it only inside `IconBaseName`, and only when
  `IsCanonical || Owner == null`. It picks which character's sprite to draw for
  an unowned relic. It is cosmetic, and never consulted by the model.
- **`Trial`** sets `DynamicVars["EntrantNumber"]` to a random 101–999 in
  `CalculateVars`. That *is* model state rather than presentation. But it is
  flavour text for the event's own string, and the observation does not report
  `DynamicVars`. Worth remembering if it ever does.

One was not benign, and has a patch. That is `MonsterModel.Rng`, below.

## The canonical-monster warning

`MonsterModel.Rng` is:

```csharp
get
{
    if (!base.IsMutable) return Rng.Chaotic;
    return _rng;
}
```

A **canonical** model is the shared template `ModelDb` holds, as opposed to the
per-run copy `ToMutable()` makes. Reading this property on one silently yields
the process-wide clock-seeded stream. That is the same shape as the
`DebugRandomizeRng` bug. It is a substitution rather than an error, and it
produces plausible values nothing in the run accounts for.

It does not currently bite. Monsters in a fight come from `GenerateMonsters()`
via `ToMutable()`, so they are mutable and use their own seeded `_rng`. The
warning exists because *"does not currently bite"* was a claim with no evidence
behind it. It now has some. No warning fires across the selftest suite, the
determinism suite, or the five live tests, which do fight real monsters.

A warning rather than a throw, deliberately. The game reads this property from
presentation paths a headless run does not exercise, but one day can. A
determinism mod that crashes the game to report a determinism risk has made
things worse.

## It cleans up after itself

Patching extracts a native helper to the temp directory, `dlopen`s it, and never
unlinks it. That is one per process, forever. 398 of them, totalling 7.5MB, had
accumulated here over eleven days before anyone looked. This mod is the reason
they exist, so it sweeps them at load. It sweeps everything older than a minute,
which leaves alone any that another process has just written and not yet
opened.

## Launching requires `LD_PRELOAD=libgcc_s.so.1`

**Without it every patch throws and this mod stays inert.** Harmony fails with:

```
HarmonyLib.HarmonyException: Patching exception in method
  MegaCrit.Sts2.Core.Models.EncounterModel::DebugRandomizeRng()
---> System.DllNotFoundException: Unable to load shared library '/tmp/mm-exhelper.so.…'
     undefined symbol: _Unwind_RaiseException
```

MonoMod writes a native helper to `/tmp` and `dlopen`s it. That helper has **no
`DT_NEEDED` entries at all**, as `readelf -d` shows, while it imports
`_Unwind_RaiseException`, `_Unwind_SetGR`, `_Unwind_SetIP` and friends from
libgcc. It relies entirely on the host process having libgcc loaded already, and
`SlayTheSpire2` does not link it. Preloading puts those symbols in the global
namespace before the helper is opened.

`sts2_headless`'s Rust client sets it when launching, and `just selftest` sets it
too. Anything else that starts the game must do the same. The failure is
particularly unkind if you forget. Patches throw, `Active` stays false, and the
driver refuses. Before this was noticed, the process hung rather than exiting,
with the reason in a log nobody was reading.

## What it deliberately does **not** do

- **No content, no rules.** No cards, relics, enemies or balance changes.
- **It does not drive the game.** It has no notion of a run, a command, or an
  observation. Something else does that.
- **It does not patch `sts2.dll`.**
- **It does not touch your saves.** The one exception is that forcing FTUEs off
  means the game stops recording that you have seen them.
- **It is not a gameplay mod.** `TestMode` changes control flow. Do not load
  this while playing.

## The `Active` contract

`Determinism.Active` is `true` only once the game has actually been configured.

Dependent mods must check it rather than assuming. The loader guarantees this
mod *loaded* first, because it refuses to load a mod whose dependencies are
absent, and names them. But **loading and succeeding are different things**. A
build mismatch leaves this mod loaded, inert, and the game untouched.
`sts2_headless` checks it, and exits the process rather than returning, because
a headless game with no driver runs until something external kills it.

## Further reading

[REQUIREMENTS.md](REQUIREMENTS.md) is the standard this mod is held to. It has
D1, D2 and D3, each written so it can be failed. `D` is this mod, and `H` is
[`sts2_headless`](../sts2_headless/REQUIREMENTS.md), which owns H1–H12. They were
one sequence, R1–R14, when this was one mod. Splitting it renumbered them, and
rewrote every reference in the source of four mods and the Rust client.

## The version pin

`Init` refuses to run unless `release_info.json` reports the assembly hash of the
build this was compiled against. Every claim this mod makes is a claim about one
build's behaviour, so a mismatch is a hard stop.

The hash is not written here. `mod/build.sh` reads it from the lock it was given,
and the csproj generates the constant. A literal beside the thing it describes is
a constant that agrees with whatever it was last edited to say.

## How much of it is actually verified

**Verified automatically**, by
[`sts2_determinism_test`](../sts2_determinism_test/README.md), five assertions,
all mutation-tested, and run against every pinned build:

- The encounter RNG patch. The seed actually used is exactly
  `runState.Rng.Seed + TotalFloor + hash(Id.Entry)`. Reverting the patch fails
  two of the three RNG checks, with the seed coming back as the current Unix
  timestamp.
- The FTUE patch. An unseen tutorial reports as seen *with the profile flag
  left on*, and asking does not turn it off.

The 144 assertions in `sts2_headless_test` also exercise a game configured by
this mod. A configuration that broke the driver shows up there.

**Verified once, by hand:** that both mods load in dependency order
(`0: sts2_determinism`, `1: sts2_headless`). Also that the cross-mod reference
resolves, and that the patches apply under `LD_PRELOAD` and throw without it.
Also that `sts2_headless` refuses and exits when this one has not activated,
which was found the hard way, as a hang.

**Not verified at all:** that this list is complete. Determinism is a claim
about everything the game does. This mod covers two things somebody noticed.
`Rng.Chaotic` and other wall-clock reads have not been audited exhaustively.
