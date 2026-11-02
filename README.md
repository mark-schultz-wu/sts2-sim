# sts2

A simulator of Slay the Spire 2's rules, written in Rust, for AI work.

It plays whole runs of the game: every character, card, relic, potion, monster
and event, at each ascension level and with each custom-mode modifier. It plays
two builds of the game, v0.107.1 and v0.111.0. It plays co-op runs of several
players too. Each step lists the legal moves, and the same seed and moves give
the same run.

The simulator answers to the real game. A set of mods runs the game with no
screen and lets a program drive it. Tools then play the game and the simulator
side by side on the same seeds, and stop where the two disagree.
[What it does, and its limits](docs/STATUS.md) gives the numbers.

This is a fan project. Mega Crit did not make it and has no part in it. The game
and its content are theirs. The code here is ours, and it holds none of the
game's files, art or text.

## What you need

- Rust, at the version `rust-toolchain.toml` names. `rustup` picks it up.
- For the simulator alone, nothing more.
- For the game's text in the terminal front end, an install of a supported
  build. See [The game's text](#the-games-text).
- To check the simulator against the game, an install of the pinned build, the
  .NET SDK, and the mods. See [`mod/README.md`](mod/README.md).

## Play a run

The terminal front end plays a run on the simulator:

```sh
cargo run --release -p sts2-cli --bin sts2 -- sim
```

A program plays runs through `sts2-run`. This plays one run at random:

```sh
cargo run --release -p sts2-run -- play C000000042 IRONCLAD --player random
```

`--build v0.111.0` picks the other build. `IRONCLAD,SILENT` seats two players.
[docs/RUNNERS.md](docs/RUNNERS.md) says how the runner, its players and its
layers fit together. `crates/sts2-sim/examples/fight_bench.rs` drives fights
straight from Rust.

There are no Python bindings yet.

## The game's text

What a card or relic says is the game's own writing, so this repository holds
none of it. The build script of `sts2-cli` copies it from your install of each
supported build. It looks in `STS2_GAME_DIR`, `STS2_STEAM_DIR` and
`$XDG_DATA_HOME/sts2/games/<build>`. It tells an install's build by the hash of
its `sts2.dll`, which `mod/games/<build>.lock` pins. Without an install, the
front end shows ids and numbers, and the build prints a warning.

## How it is checked

- About 1,430 single-player and 45 co-op scenarios. Each sets up a position
  in the real game, plays a few moves, and records what the game did. The
  simulator has to reach the same positions.
- Recorded runs, played whole by the game at random. The simulator follows
  each one step by step, through about 5 million positions in all.
- Hunts. `cargo xtask hunt --mixed` plays fresh seeds on the game and the
  simulator side by side, over every ascension level and modifier. It keeps
  only the runs where they part. Each part found so far is fixed.

The golden tests in `crates/sts2-golden` replay the scenarios and the recorded
runs without the game.

## The crates

| crate | what it is |
|---|---|
| `sts2-core` | the types a game and a simulator both answer in |
| `sts2-content` | the game's content: tables generated from the game's own data, and the ported rules beside them |
| `sts2-sim` | the engine: fights, turns, the map, rewards and the random number streams |
| `sts2-run` | one runner for a run on the simulator or the game, with a player for each seat |
| `sts2-cli` | `sts2`, the terminal front end |
| `sts2-headless` | the client for the headless mod |
| `sts2-golden` | the tests against what the game recorded |
| `sts2-pack` | reads files out of the game's pack |
| `xtask` | the tools: dumps, code generation, hunts, the census and the rest |

[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) says where things live, and why.

## How it was made

Most of this code was written by Claude, Anthropic's AI model, in Claude Code,
with Mark Schultz-Wu directing. The history is kept as it happened. Each pull
request says what it changed and how it was checked.
[docs/agent-dev/](docs/agent-dev/) holds the working notes of that process.

## License

The code is under either the [MIT license](LICENSE-MIT) or the
[Apache License 2.0](LICENSE-APACHE), at your choice. That covers the code in
this repository only. Slay the Spire 2 and its content belong to Mega Crit.

If Mega Crit asks for anything here to come down, it will.
