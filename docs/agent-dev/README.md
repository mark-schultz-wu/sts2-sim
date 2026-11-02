# How the work was run

Most of this repository was written by Claude in Claude Code, with Mark
Schultz-Wu choosing what to work on and reviewing what landed. This folder
holds the working notes of that process. They are kept as they were used,
not tidied for readers.

- [`BLOCKERS.md`](BLOCKERS.md) is the work list. It says what "done" means,
  and lists what still stands between the simulator and that, in order.
  Each iteration of the loop took its first open item.
- `.claude/skills/` at the repository root holds the loops that worked
  through it, such as `blocker-grind`. They stay there because Claude Code
  reads skills from that place alone.

Each change landed as a pull request. Its message says what changed, what it
mirrors in the game, and how a scenario or a hunt checks it.
