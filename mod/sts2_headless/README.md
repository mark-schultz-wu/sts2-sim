# sts2_headless

A Slay the Spire 2 mod that drives the real game with no renderer and no human,
and reports what it did — over a line-delimited JSON protocol.

Nothing here knows what it is being used *for*. Driving a game and observing the
result is the whole job; checking a reimplementation against it is what the
client on the other end does.

Reproducibility is not this mod's job either — [`sts2_determinism`](../sts2_determinism/README.md)
owns that, and the manifest declares a dependency so the loader runs it first
and refuses to load this without it.

---

## ⚠️ This code is LLM-generated

**Every C# file in this directory was written by Claude, not by a human.** It has
not been reviewed line by line. Treat it as you would an unfamiliar dependency
from a stranger: useful, plausibly correct in the ways it has been tested, and
not to be trusted beyond that.

This README exists because the usual defence — reading the diff — is not
available here. So instead of asking you to trust the code, it states as plainly
as possible **what the mod touches, what it deliberately does not, and what has
actually been verified rather than merely intended.**

Where a claim below is checked by something automatic, it says so. Where it is
only an argument from reading the decompiled game, it says that too. Those are
very different levels of confidence and the difference is not smoothed over.

---

## What it does

Runs a real STS2 run at the model layer — no main menu, no run scene — and
exposes it over a line-delimited JSON protocol on a loopback TCP socket:

```json
{"cmd":"play_card","index":2,"target":1}
{"type":"observation","seq":3,"cmd":"play_card","resolved":true,"digest":"A21E…"}
```

One request, one response. The response is either an observation of the whole
state, a question the game is waiting on, or an error.

## What it deliberately does **not** do

- **It adds no content and changes no rules.** No new cards, relics, enemies or
  balance changes. It only reads the game and drives it.
- **It never chooses a move.** Every decision comes from the client. Where the
  game asks the mod to pick a card, the mod refuses and forwards the question.
  Where a path is not implemented yet, it throws rather than picking for you.
- **It does not touch your saves.** `SetUpTest(shouldSave: false)`, and modded
  runs get their own profile directory (`user://default/1/modded/profile1`).
  Verified by checksumming the profile directory across a driven run.
  *Exception:* `deploy.sh` edits one field in `settings.save` to enable mods,
  backing the file up to `settings.save.pre-oracle` first.
- **It does not patch `sts2.dll`.** `ModManager` is a supported extension point,
  so there is no patched binary to re-derive when the game updates.
- **It is not a gameplay mod and should not be loaded while you play.** It takes
  over the run as soon as it starts, and quits the process when it finishes.

## What it changes about how the game behaves

This is the part worth knowing, since it is where "reads the game" stops being
strictly true. All of it is deliberate; none of it is hidden:

| change | why | risk |
|---|---|---|
| installs an `ICardSelector` and a reward selector | routes both prompts to the client | without them the game blocks on a UI that does not exist headless. Installed in `Driver.Run`, so every mode gets them — they used to be installed inside `Protocol.Serve`, and the selftest suite ran without them |
| `ChecksumTracker.IsEnabled = true` | re-enables the game's own divergence detector, which `TestMode` and a singleplayer service each silently disable | none known |
| `SetUpTest(shouldSave: false)` | a driven run must not write a save | none known |
| calls `GetTree().Quit(ExitCode)` when done | otherwise the process runs forever; SIGTERM proved unreliable | the process exits under the mod's control, not yours |
| patches `NScreenShake.Shake`/`Rumble` to skip when there is no target | with no screen the method already did nothing but log; suppressing it is what makes `gameErrors` mean something | a benign message is hidden, so anything genuinely wrong with screen shake would be too — nothing headless can act on it either way |
| no run scene is instantiated | `res://scenes/run.tscn` segfaults headless | the model layer calls presentation inline in places; those errors are caught and now reported (see `gameErrors`) |

`TestMode.IsOn` and the FTUE suppression are **not** in this table any more:
[`sts2_determinism`](../sts2_determinism/README.md) owns them, along with the
build pin and the encounter-RNG patch. This mod refuses to start if that one did
not activate.

## How much of it is actually verified

**Verified by an automatic check, re-run every build:**

- 144 assertions in [`sts2_headless_test`](../sts2_headless_test/README.md)
  (`STS2_HEADLESS_MODE=selftest`), covering all nine
  of the game's legality-hook overrides. The list of hooks is discovered by
  reflection over the game assembly, so a hook added by a future patch shows up
  as `UNCOVERED` rather than silently going untested.
  - Three of those check the *preventer* a blocked card reports, against the
    model `CanPlay` itself picked rather than against an expected name. Written
    the other way round first, they failed: an afflicted card is blocked by the
    power its affliction applied, not by the affliction, because powers come
    first in the hook listener order and `ShouldPlay` returns the first refusal.
- Several of those tests are **mutation-tested**: the oracle was deliberately
  broken to confirm the test fails, then fixed to confirm it passes. A test that
  has never been seen to fail is not evidence.
- Determinism: two fresh processes replaying one script produce byte-identical
  digest sequences.

**Verified once, by hand, not automated:** headless startup, map travel,
combat rewards, rest sites, events, shops, protocol error paths, throughput
numbers.

**Not verified at all:** anything on another machine; a full run rather than a
few rooms. All five characters are now driven through a fight, including the
Defect, whose orbs the observation reports.

### Bugs this found — in the oracle, not the game

Recorded because they calibrate how much to trust the rest. Each was invisible
until something specifically looked for it:

1. **The boss disappeared from the legal move set.** On the last map row with
   free travel active, the oracle reported *no legal move at all*. Two correct
   rules composing into a wrong answer.
2. **Observations were a full turn stale.** `end_turn` returned before the turn
   actually changed, so the client acted on a state the game had already left.
3. **A failed action was reported as success.** The game caught its own
   exception and logged it; the oracle saw the action "complete" and answered
   `"resolved": true`. Nothing in the oracle's own view was wrong.
4. **The same "answered before the work finished" bug, four times** — an empty
   action queue before the action was picked up, a turn that had not yet
   changed, rewards not yet applied, and an event option not yet resolved. Out
   of combat every readiness test the driver has is trivially true, so work
   started outside the action queue is invisible to it and the client is handed
   a state its own decision has not reached yet. Item 2 is one of these; they
   are listed together because the shape matters more than the instances.

Number 3 is why every response now carries a `gameErrors` field when the game
logged anything at error level during that command. **If you see `gameErrors` in
a response, do not trust the observation attached to it.**

Number 4 is the standing hazard of this codebase rather than four accidents, so
new code that starts work outside the action queue must register it with
`BackgroundWork` — otherwise it will reproduce the bug a fifth time.

## Running it

```bash
./deploy.sh          # build, install into <gamedir>/mods/, enable mods
cd "$STS2_GAME_DIR" && STS2_HEADLESS_MODE=selftest ./SlayTheSpire2 --headless --force-steam=off
```

Modes, via `STS2_HEADLESS_MODE`:

| mode | what it does |
|---|---|
| *(unset)* | mod loads and does nothing |
| `selftest` | claimed by `sts2_headless_test`; does nothing if that mod is not installed |
| `serve` | speaks the protocol over the TCP port the client passed in |

Other variables: `STS2_HEADLESS_PORT` (required by `serve`) + `STS2_HEADLESS_TOKEN`,
and `STS2_HEADLESS_OBS=full`.

`STS2_HEADLESS_TIMEOUT` is the wall-clock budget in seconds, default 600, `0` to
disable. Whenever a mode is set the process also makes SIGTERM and SIGINT
immediately fatal. Both are H12, and neither applies to a player's game. Do not
launch one of these unsupervised with the clock off: a hang holds everything the
run allocated, ignores the SIGTERM your supervisor and your shutdown both rely
on, and can take the machine with it.

## The version pin

`Oracle.Init` refuses to run unless `release_info.json` reports
`main_assembly_hash: 1692500715` (v0.107.1). Steam can update the install
underneath us, and an oracle answering for a different build than the one we
reason about is worse than no oracle, so a mismatch is a hard stop.

## Why a mod rather than a standalone harness

The game's logic is inseparable from Godot — 1102 of its 3424 source files use
Godot types — so anything loading `sts2.dll` outside the engine has to fake
`GodotSharp`. A fake that returns `default` where the engine returns something
real gives an oracle that is quietly wrong, which is worse than one that
crashes.

`sts2-cli` takes that route and it is instructive: its map legality reads
`currentPoint.Children`, with no reference anywhere to `MapTravel`,
`ShouldAllowFreeTravel`, `WingedBoots` or `Flight` — so a run holding either
gets a legal move set narrower than the real game's, silently. That is the
specific class of bug running the real engine is meant to remove.

## Mod loading, as the game implements it

- Mods live in `<gamedir>/mods/<dir>/`, holding a `*.json` manifest and an
  assembly named exactly `<manifest id>.dll`.
- A class carrying `[ModInitializer("Method")]` has that method called during
  startup. With no such attribute the loader instead runs `Harmony.PatchAll`.
- Mods stay `Disabled` until `mod_settings.mods_enabled` is true in
  `settings.save` — normally set by an in-game popup that headless never shows,
  so `deploy.sh` sets it.

## Further reading

[PROTOCOL.md](PROTOCOL.md) is the wire contract on one page: how to connect, the
four response types, every command and its arguments, the pendings, and the rules
about what is accepted when. Read it before writing a client.

[REQUIREMENTS.md](REQUIREMENTS.md) is the standard this is held to: twelve
requirements, H1–H12, each written so it can be failed, with what has been measured,
what has not, and what is still an open question. It is long, and it is the
honest record — including the parts that are not done.
