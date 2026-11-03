# Docs

## Start here

- [STATUS.md](STATUS.md): what the simulator does, how well it agrees with
  the game, and its limits.
- [COMPARISON.md](COMPARISON.md): other tools for Slay the Spire AI, and how
  this one differs.
- [ARCHITECTURE.md](ARCHITECTURE.md): where things live, and why.
- [RUNNERS.md](RUNNERS.md): how a run is played. One runner holds a table (the
  simulator or the game), a player for each seat, and layers.

## Design records

Each of these is a design as it was decided, with its date and status at the
top. Later work changed some details. The code is the final word, and these
say why it took its shape.

- [STS2-CORE.md](STS2-CORE.md): the shared types that the game client and the
  simulator both implement.
- [ASYNC.md](ASYNC.md): how the simulator pauses on a player's choice in the
  middle of a rule, with coroutines.
- [SCHEDULER.md](SCHEDULER.md): the game's action queue and turn order, in
  the engine.
- [RUN-PAUSES.md](RUN-PAUSES.md): the same pauses outside fights.
- [COOP-FIGHTS.md](COOP-FIGHTS.md): fights of several players.
- [HOOK-SUBJECTS.md](HOOK-SUBJECTS.md): in co-op, which player a hook is about.

## The game

- [GAME-ASYNC.md](GAME-ASYNC.md): how the game's own async rules, action
  queue and choices work, read off its decompiled code. Its citations are
  paths in a decompiled tree, which this repository does not hold.

## Elsewhere

- [`agent-dev/`](agent-dev/): the working notes of how the work was run.
- `mod/`: the game mods, each with a README. `mod/sts2_headless/PROTOCOL.md`
  is the wire protocol.
- `crates/*/README.md` and `xtask/README.md`: each crate's own notes.
