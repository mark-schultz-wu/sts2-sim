# One runner

Every way this repository plays a run is one driver, built from four
parts:

- a **table**: what holds the run, which is the simulator or the real game.
- a **player** for each seat: what picks that seat's moves.
- **layers** around the table, each doing one job, as tower's middleware
  does.
- the **lobby**: how many seats, which characters, the ascension, the
  modifiers and the seed.

Solo is a lobby of one. Co-op is a lobby of several. Nothing else changes
between them.

## Where it stands

Eleven entry points play or replay a run today. They share one random
pick, `Pick`. The rest is written again in each:

| Piece | Copies |
|---|---|
| The relics a hunt carries | 9 |
| Picking the build from `STS2_GAME_VERSION` | 5 |
| Comparing the simulator with the game, step by step | 6 |
| Catching a panic and saying where the run stopped | 5 |
| Writing an STS2REC run | 2, plus a JSONL tape in the fuzzer |
| The verdict line, and a parser for each | 4 |
| A pool of runs in xtask | 5 |

Some shared parts exist, but no entry point uses them. `Recorder` wraps
any `Play` and records it, and only tests use it. `SimSeat` is not a
`Seat`, so `Coop` cannot drive the simulator. The co-op hunt and the
golden test drive a `Vec<SimSeat>` by hand. `follow_noting` refuses a run
of several players.

The two traits are part of why. `Play` is one run on one machine. `Seat`
is one player's machine in a co-op run. The game is one process for each
seat. The simulator keeps one machine's view for each seat, in one value,
and each takes every player's moves. Neither trait fits both.

## The table

`sts2_core::RunTable`, in `crates/sts2-core/src/play/table.rs`:

```rust
trait RunTable<V> {
    type Error;
    type Host;
    /// One seat's view, lent: read off the run, or the last report.
    type Seen<'a>: Observed<V>;
    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error>;
    fn seats(&self) -> usize;
    fn seen(&mut self, seat: SeatIndex) -> Result<Self::Seen<'_>, Self::Error>;
    /// The moves the seat can make now, none while it waits on the rest.
    fn moves(&mut self, seat: SeatIndex, moves: &mut Vec<Decision<V>>) -> Result<(), Self::Error>;
    /// A refusal is an answer, not a fault.
    fn play(&mut self, seat: SeatIndex, decision: &Decision<V>) -> Result<Settled<V>, Self::Error>;
    /// The report and the question, built only when something asks.
    fn snapshot(&mut self, seat: SeatIndex) -> Result<Snapshot<V>, Self::Error>;
    fn construct(&mut self, seat: SeatIndex, setup: &Setup<V>) -> Result<Settled<V>, Self::Error>;
    /// What the games logged as errors. None at a table with no game.
    fn logged(&mut self) -> Vec<(SeatIndex, String)>;
}
```

- `SimTable` keeps a run for each seat's machine, and plays every move on
  each. A solo run is a table of one.
- The game implements it as a lobby of processes, one for each seat:
  `GameSeat`, as `Coop` holds them now.
- `RunTable` replaces `Play` and `Seat`. The interactive `sts2` daemon keeps
  its own one-command-per-process shape on top of a table of one.

`play` hands back no observation. Building one costs more than the step.
So the census, which never reads one, pays nothing for it. A player reads
`seen`, and a layer that needs the question asks `snapshot`.

## Players

```rust
trait Player<V> {
    /// Which of `moves` seat `seat` makes.
    fn choose<T: RunTable<V>>(&mut self, seat: SeatIndex, table: &mut T, moves: &[Decision<V>])
        -> Result<usize, T::Error>;
}
```

- `Random`: `Pick`, with its end-turn odds and the room rule. A layer
  never sees the rule, because it is the player's own.
- `Uniform`: every legal move alike, Cancel included. The fuzzer's.
- `Replay`: the move indices of a recorded run, seat by seat.
- `Human`: the terminal UI.
- Later, an AI.

Which seat acts next is the driver's: `Coop::step`'s rule today. A seat
with no moves waits.

## Layers

Each layer is a `RunTable` that wraps a `RunTable`, and does one job:

- `Carried`: grants each seat its relics before the first move: 25
  Gorgets and a Mercury Hourglass.
- `Recorded`: writes the run as STS2REC, a seat, a move index and a
  digest after each move. It replaces the hand-written writers, and
  `Recorder` and `Tape` go.
- `Hunted`: plays every move into a simulator beside the table, and
  stops at the first seat whose view parts. It is the one compare: the
  hunt, the co-op hunt, `follow_run --diff` and the golden tests use it.
- `LogGuard`: stops on an error the game logs. Only the game's table
  has a log.
- `RefusalGuard`: stops on a move the table refuses after reporting it
  legal. That is the fuzzer's finding, kept.
- `Capped`: stops after a number of steps, or of steps with no move.

## One verdict

Every run ends on one `sts2_core::Verdict`, printed as its last line:

```text
verdict <end> <step> <act> <floor> <seat or -> <place or -> <detail>
verdict panicked 817 1 26 - crates/sts2-sim/src/run/state.rs:5925:17 not implemented: ...
```

The end is one of `won`, `lost`, `capped`, `stalled`, `agreed`, `parted`,
`panicked`, `refused`, `failed`, `game-error` and `game-gone`. The act and
floor are the furthest any seat reached. The place is where a panic was
raised. `sts2-core` derives no serde, so the line is written and read by
`Display` and `FromStr` there, and xtask reads it with that one parser.

A panic still ends the process. The runner's panic hook prints the verdict
first, with how far the run got.

## The runner and xtask

One binary, `sts2-run`:

```text
sts2-run <seed> <characters> [--build v0.107.1] [--table sim|game]
         [--player random|uniform|replay FILE]
         [--carried] [--record FILE] [--hunt] [--steps N]
         [--ascension ID] [--modifiers ID,...]
```

`<characters>` is one character for a solo run, or several joined by
commas, one seat each.

xtask keeps its commands, each one runner configuration and one pool of
runs:

| Command | Runner |
|---|---|
| `census` | `--table sim --player random --carried` |
| `record` | `--table game --player random --carried --record` |
| `hunt` | the same, `--hunt`, keeping only runs that part |
| `coop` | `hunt` with several characters |
| `followed`, `split` | `--table sim --player replay` |
| `game-trace` | `--table game --player replay --hunt` |

`--at` and `--scenario` move from `follow_run` into `xtask followed`,
since they read a recorded run and play nothing new.

## What goes

- `random_run`, `record_run`, `coop_run`, `follow_run`: into `sts2-run`.
- `sts2-fuzz`: `--table game --player uniform`, with `RefusalGuard` and
  `LogGuard`. Its coverage file and clock seed go with it.
- `sim` (bin): `--table sim --player human`.
- `Play`, `Seat`, `Recorder`, `Tape`, `SimSeat`: into `RunTable` and the
  layers.
- The cli's `scenario` example and `map_bench`, which nothing runs.
  `where_it_goes`, `clone_sites`, `digest_sites` and `legal_shapes` go too.

These stay as they are:

- `sts2`, the daemon a script or agent drives one command at a time.
- `fight_bench`, spire-mentor's comparison.
- The replay under `cargo xtask instructions`.

## Speed

The census is the hot path. The runner is generic over the table, the
player and the layers, so the census's runner compiles to the loop
`random_run` is now. `play` builds no observation, and `Pick` reads only
the moves. `cargo xtask instructions` measures the replay, which this
does not touch. The census's own speed is measured before and after.

## The order of the work

Each step is one PR, with the golden tests, the census and the hunt.

1. `RunTable`, `SimTable`, `Verdict`, `Player` and `Random`, `Carried`, and
   `sts2-run` on the simulator. The census moves onto it, and
   `random_run` goes (done). The simulator's table plays co-op runs too,
   so a census of several players needs no game.
2. The game's tables, solo and co-op, and the layers `Recorded`, `Hunted`
   and `LogGuard`. `record`, `hunt` and `coop` move onto `sts2-run`, and
   `record_run` and `coop_run` go. The run format and the compare move
   into `sts2-run`. One pool of runs in xtask.
3. `Replay`: `follow_run` and the golden replay tests move onto it. A
   co-op run replays like a solo one, so `follow_noting`'s refusal goes.
4. `Uniform`, `RefusalGuard` and `Human`: the fuzzer and `sim` go.
   `Play`, `Seat`, `Recorder` and `SimSeat` go. The `sts2` daemon reads a
   `RunTable`. The unused examples go.
5. ARCHITECTURE.md and STS2-CORE.md say what is above.
