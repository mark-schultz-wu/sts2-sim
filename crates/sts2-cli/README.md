# sts2-cli

The `sts2` command. It plays the real game from a terminal, one line at a time.

```
$ sts2 run start SP1RE1 ironclad
$ sts2 show map
$ sts2 act travel 1 2
$ sts2 play 0 --target 1
$ sts2 act end-turn
```

## A daemon holds the game

The game takes a few seconds to start. To do that for each command makes the
tool too slow to use.

So `sts2 run start` leaves a small daemon in the background with the game open. Each
later command is a short process that talks to the daemon. The daemon stops
itself after a quiet period, so a forgotten game does not hold memory.

`sts2 run status` shows what runs, and `sts2 run stop` ends it.

## One daemon for each build

`--game-version` picks which pinned build to drive, and defaults to
`STS2_GAME_VERSION`. Each build gets a daemon and a socket of its own. Two
builds can be open at the same time, and a command cannot reach the wrong one.

The daemon relays commands the client already encoded against its own tables. A
client that reaches a daemon holding another build reads that game through the
wrong ids, and reports the answer as fact. Separate sockets make that
unreachable rather than wrong.

`STS2_GAME_DIR` must point at the same build. When it does not, the game says
which build it is at the handshake and the command stops.

## The tool writes down each run

By default the tool records each run to a file, as the list of decisions that
made it. The game is deterministic, so those decisions are the whole run. A
replay gives the same shuffles, the same monster moves, and the same rewards.

`sts2 run replay FILE` opens a run again at the point where it stopped. Normal
commands then continue from there. `--steps N` stops part way, which makes a
branch.

You learn which run is worth keeping after the run ends, and you cannot show a
run that nobody wrote down.

## How the game reads at a terminal

The renderer is an opinion. That is why it is here and not in `sts2-headless`.
That crate reports what the game said. How the game reads at a terminal is this
crate's choice, and a second front end can disagree.

`sts2 show map` draws the whole act. Which room to enter next is the largest decision
in a run, and you make it from several rows ahead.

## sts2-sim

`sts2-sim` is the same screen as a bare `sts2`, on the simulator. It needs no
game and no daemon. The run lives in the process that draws it, and the run ends
when you quit. Pick a character and a seed, and the run starts at once.

```
$ sts2-sim
$ sts2-sim --game-version v0.111.0
```

The default build is v0.107.1, which the simulator plays to the end of each
recorded run. A path that the simulator has no port for stops the program, and
the message names that path.

## sts2-fuzz

A second binary. It starts a run, grants itself a hundred `Gorget`, and takes
legal moves at random until the run ends or something breaks.

Three things count as a find, and all three stop the run:

- The game refuses a move that the oracle offered.
- The game logs an error while it runs a move.
- The game dies.

The first one is the useful one. It means that the legality the oracle reports
and the legality the game enforces do not agree. That is the exact fault that a
simulator copies.

The fuzzer found two real faults in its first three runs. A shop offered a potion
to a full potion belt. Each run also stopped on floor 17, because the protocol
had no command to leave a finished act.
