# sts_oracle

A Slay the Spire 2 mod that turns the real game into a differential-testing
oracle. Not a gameplay mod — it adds no content and changes no rules.

```bash
./mod/deploy.sh          # build, install into <gamedir>/mods/, enable mods
cd "$STS2_GAME_DIR" && ./SlayTheSpire2 --headless --force-steam=off
```

## Why a mod rather than a harness

The game's logic is inseparable from Godot — 1102 of its 3424 source files use
Godot types — so anything that loads `sts2.dll` outside the engine has to fake
`GodotSharp`. A fake that returns `default` where the engine would return
something real gives an oracle that is quietly wrong, which is worse than one
that crashes. Running the real game removes the question.

`ModManager` is a supported extension point, so nothing here patches `sts2.dll`
on disk: there is no patched binary to re-derive when the game updates.

## Running it

Two flags matter:

- `--headless` — no renderer.
- `--force-steam=off` — without it, Steam init fails three times and startup
  aborts before mods load. The game reads this as editor mode and skips the
  platform layer.

`--nomods` disables mod loading entirely, if you need a clean baseline.

Startup to main menu is ~10s. That is per-process, not per-step.

## The version pin

`Oracle.Init` refuses to run unless `release_info.json` reports
`main_assembly_hash: 1692500715` (v0.107.1). Steam can update the install
underneath us, and an oracle answering for a different build than the one we
reason about is worse than no oracle, so a mismatch is a hard stop rather than
a warning.

The install's `sts2.dll` is currently byte-identical to the pinned
`~/sts-artifacts/sts2-cli-lib/sts2.dll.original`, which is what the mod compiles
against.

## Mod loading, as the game implements it

- Mods live in `<gamedir>/mods/<dir>/`, holding a `*.json` manifest and an
  assembly named exactly `<manifest id>.dll`.
- A class carrying `[ModInitializer("Method")]` has that method called during
  startup. With no such attribute the loader instead runs `Harmony.PatchAll`.
- Mods stay `Disabled` until `mod_settings.mods_enabled` is true in
  `settings.save` — normally set by an in-game popup that headless never shows,
  so `deploy.sh` sets it (backing the file up first).
- Modded runs get their own profile: `user://default/1/modded/profile1`.
