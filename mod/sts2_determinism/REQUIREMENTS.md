# What sts2_determinism has to do

> **This document and the code it describes were written by Claude, not by a
> human, and have not been reviewed line by line.** It is a working record, not
> a specification anyone signed off. Claims marked *measured*, *built* or
> *mutation-tested* were checked by running something. Claims marked *resolved*,
> or argued from the decompiled source, were not.

Each requirement is written so it can be failed. Where something is not yet
measured or not yet known, it says so rather than assuming.

**`D` is determinism, `H` is headless.** These were one sequence, R1–R14, when
this was one mod. The split renumbered them, and rewrote all 146 references in
the source of four mods and the Rust client. The prefix means a reference names
its own file, so `H6` never has to be looked up to find out where it lives.

This mod makes the game reproducible. It drives nothing and observes nothing.
[`sts2_headless`](../sts2_headless/REQUIREMENTS.md) does that, and everything it
claims rests on this one holding. See [README.md](README.md) for what is
actually implemented and how much of it is verified.

## The requirements, and where they live

| | requirement | file |
|---|---|---|
| **D1** | Identical inputs produce identical outputs | **here** |
| **D2** | Refuses to answer for the wrong build | **here** |
| **D3** | The player's own saves are never touched | **here** |
| **H1** | A run cannot start without an explicit seed | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H2** | The legal command set is reported | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H3** | One command advances exactly one decision | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H4** | The observation is complete | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H5** | Divergence is detectable out of combat too | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H6** | Scenarios can be constructed, not only replayed | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H7** | Throughput is measured and reported | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H8** | The process boundary follows the measurement | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H9** | The protocol is machine-readable and versioned | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H10** | Failure is loud | [headless](../sts2_headless/REQUIREMENTS.md) |
| **H11** | The driver ends its own process | [headless](../sts2_headless/REQUIREMENTS.md) |

---

## D1 — Identical inputs produce identical outputs

The same seed and the same command sequence must yield byte-identical
observations, in a fresh process, on a different machine, on a different day.

*Why it is not free.* `Rng.Chaotic` is seeded from `DateTimeOffset.Now`, and
there are 193 references to it. All the ones we checked are view-layer or gated,
but three traps remain, none of which fails loudly:

- `SeedHelper` invents a seed when none is supplied (H1).
- `MonsterModel.Rng` returns `Chaotic` **silently** when queried on a
  non-mutable canonical model.
- `EncounterModel.DebugRandomizeRng()` reseeds from the wall clock at
  one-second granularity. `FightConsoleCmd` calls it unconditionally (H6, in
  headless).

The last is the shape to watch for. The nondeterminism is coarse enough that
short test runs often agree. So it reads as flakiness, not as a broken oracle.

*Check.* Replay one fixed script three times in three fresh processes and
diff the full observation logs. Then do it again on another machine. Any
difference is a defect in the oracle, not in the simulator.

*First result: passes, single-machine.* Three fresh processes on seed `ORACLE1`
produced identical state, RNG counters, checksum sequence and decision count.
A cold profile (modded profile directory deleted first) matched a warm one.

*Not yet checked:* another machine, and anything beyond one combat.

*A hazard this check nearly missed.* Profile progress persists across runs, so
the second run of anything is not the first. `Player has seen ftue shuffle_ftue!`
appeared on an early run and never again. That made a later comparison of two
warm runs look clean, while saying nothing about a fresh profile. The driver now
calls `SaveManager.SetFtuesEnabled(false)`, and the check deletes the profile
between runs. **Determinism comparisons between two already-warmed runs prove
less than they appear to.**

---

## D2 — It refuses to answer for the wrong build

*Built.* `Determinism.Init` hard-stops unless `release_info.json` reports
`main_assembly_hash: 1692500715` (v0.107.1). Every claim in this file is a claim
about one build's behaviour, so answering for a different one is worse than not
answering.

The check lives here rather than in the driver because the pin exists to protect
these claims. `sts2_headless` inherits it. It checks `Determinism.Active` before
starting, and a failed pin leaves that false. **Loading is not succeeding.** The
loader guarantees this mod ran first, not that it worked.

---

## D3 — The player's own saves are never touched

The oracle writes to no real profile and no real save. `SetUpTest` defaults
`shouldSave: false`. Keep it that way, and confirm nothing else writes.

*Check.* Checksum the profile directory before and after a full run.

*Result: satisfied for the player's data, not for ours.* A driven run leaves the
unmodded profile untouched. This install has none, and everything lands under
`default/1/modded/profile1/`. What does change there is `saves/progress.save`,
and its `.backup`, because run-level actions record profile progress.
`RelicCmd.Obtain` calls `SaveManager.MarkRelicAsSeen`, and fighting an enemy
records a discovery. Godot's own logs rotate, and Sentry writes a per-run
breadcrumb directory. Neither is ours to control.

So D3 holds in the sense that matters. But the residue is a **determinism**
concern rather than a tidiness one (D1). Profile state that persists across runs
is state the oracle did not declare. Treat the modded profile as disposable. The
D1 check deletes it between runs rather than trusting it to be inert.
