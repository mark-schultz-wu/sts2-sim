# What it does, and its limits

This page says what the simulator does, how well that is checked, and where it
stops. Each number has a date and a source. The numbers are measured. None is
a goal.

## What it simulates

- **Builds.** Two Early Access builds of Slay the Spire 2: v0.107.1 and
  v0.111.0. Each has its own content tables. Where the builds differ, each
  has its own rules.
- **Content.** Every card, relic, potion, power, enchantment, monster, event
  and modifier that a run of either build can reach. `cargo xtask coverage`
  counts it, and leaves out the game's test content (`Mock*`, `Deprecated*`).
- **Runs.** Whole runs, from Neow to the end of the last act, at each ascension
  level and with each custom-mode modifier.
- **Players.** One player, or a co-op run of several. Co-op has limits, below.
- **Randomness.** The game's own random number streams, so a seed gives the
  game's map, encounters, rewards and shuffles.
- **Moves.** Each position lists its legal moves, as the game offers them.
  The same seed and the same moves give the same run, every time.

It does not simulate the game's text, art, sound or screens. A position names
each thing by its id and numbers.

## How well it agrees with the game

Every check below compares the simulator with the real game, run headless
through the mods in `mod/`.

| Check | Size | Result | Date |
|---|---|---|---|
| Single-player scenarios | 1,431 | all agree | 2026-11-02 |
| Co-op scenarios | 45 | all agree | 2026-11-01 |
| Recorded runs, v0.107.1 | 3,714,375 positions | all agree | 2026-11-01 |
| Recorded runs, v0.111.0 | 1,323,927 positions | all agree | 2026-11-01 |
| Mixed hunts, from seed 10000 | 3,000 runs per build | 8 parted on v0.107.1, 9 on v0.111.0, all since fixed | 2026-10 |
| Mixed hunts, from seed 20000 | 3,000 runs per build | 1 and 3 parted, all since fixed | 2026-10 |
| Mixed hunts, from seed 30000 | 3,000 runs per build | 3 and 2 parted, all since fixed | 2026-10 |
| Mixed hunts, from seed 40000 | 3,000 runs per build | 2 and 3 parted, all since fixed | 2026-11-02 |
| Mixed hunts, from seed 50000 | 3,000 runs per build | 1 and 2 parted, all since fixed | 2026-11-02 |
| Co-op hunts, two players | 4 pairs of characters, seeds 0 to 47 | all agree | 2026-11-01 |

A *mixed* hunt spreads its seeds over every ascension level and every
modifier, and plays each with random legal moves. A run *parts* where the game
and the simulator first disagree on a position.

The recorded runs and scenarios are in `crates/sts2-golden/tests`. The golden
tests replay them without the game, so every change is held to them.

## Speed

On the fight benchmark (`cargo run --release -p sts2-sim --example
fight_bench`), the engine costs about 9,900 CPU instructions per action. That
is random legal play for the Ironclad, with the starter deck, through the 22
Act 1 Overgrowth encounters. A replayed whole run costs about 14,000
instructions per step, the map included.

The engine runs on one thread. Runs are independent, so many run at once on
many cores.

## Limits

- **Two builds only.** Each new build of the game needs its content dumped and
  its changed rules ported. Builds before v0.107.1 used another random number
  generator, which the simulator does not have.
- **No Python interface yet.** The simulator is a Rust library, a command-line
  runner and a terminal front end.
- **Co-op.**
  - Two-player runs agree on every hunted seed.
  - Three-player runs agree in scenarios, but have no hunts yet.
  - Some co-op rules that no hunted run reaches still wait for a port.
    `docs/agent-dev/BLOCKERS.md` lists them. One is the turn order while a
    question waits. Another is relics that check which player played a card.
    A third is two players who each hold Fur Coat.
- **Unported paths stop the run.** A path with no port is `unimplemented!()`
  and panics with the path's name. It never plays on with a guess. The census
  of 1,500 random runs reaches none.
- **The game's own code is not all reached.** A sample of 20 hunted runs
  entered 7,023 of the game's 15,756 rules methods. Many of the rest are
  screen code or unreachable. Nobody sorted them yet.
- **Logic that lives in the game's screens.** A few of the game's rules run in
  its screen code, which a headless game skips. An audit in October 2026
  moved the ones it found into the mods. Where it missed one, the simulator
  and the headless game agree with each other, and not with the game on
  screen.
- **Checking against the game needs the game.** The mods and the hunts need an
  install of the pinned build and the .NET SDK, and run on Linux. The
  simulator alone needs neither.
