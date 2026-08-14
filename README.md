# sts

Agents that play **Slay the Spire 2**, and the simulator they train in.

The game can already be driven programmatically, but only at a few hundred steps/s.
Training wants on the order of 10⁹ steps, which at that rate is weeks. So the real game
is not the environment — it is the **oracle**: the reference that a fast reimplementation
is checked against, step by step, on the same seed.

That makes correctness the hard part rather than speed.

## Where this is

**The oracle works.** `mod/sts_oracle/` runs the real game headless and answers questions
about it: start a seeded run, advance by exactly one decision, report the legal command
set, observe the whole state in combat, on the map, and in rest sites, events and shops.
It carries its own legality suite that runs inside the game, and a mode that dumps the
game's model database. [What the oracle has to do](mod/sts_oracle/REQUIREMENTS.md) is the
standard it is held to — written to be failed, so the tool can be checked against it
rather than argued about.

**The simulator is not started.** That is the next thing, and it is the actual product.

## How this is built

A first pass at the Rust side is preserved on the `slop-v2` branch: a protocol layer, a
generated id crate, and an oracle client. It was abandoned rather than continued, for two
reasons worth stating so they don't recur.

It was **speculative**. The state was shrunk from 2752 to 400 bytes for a branching search
that did not exist; 281 event options were given types nothing consumed; a TUI and a trace
format were planned ahead of the engine they were meant to serve. Every step was locally
defensible, which is why it kept happening.

And it was **written by the wrong party**. The hard part of this project is not Rust, it
is knowing what STS2 actually does — knowledge that lives in the decompiled C#. Handing
over conclusions instead of source leaves the simulator's author without the one thing
they need.

So:

- **The simulator is written by hand, by Mark**, along with the debugging that comes with
  it. It is the environment the ML work happens in; knowing how it works is the point.
- **The mod, and the Rust that interfaces with it, are Claude's**, as are documentation
  and tests for code that already exists. C# is deliberately out of Mark's way.
- **Nothing gets built until something demands it.** The oracle is right there — let a
  failing differential test say what is missing next, rather than designing the build
  order in advance. No optimisation without a benchmark that justifies it.

Anything on `slop-v2` worth having can be cherry-picked when a caller for it exists.

## Scope

STS1 is out of scope. The pinned STS2 assemblies in `~/sts-artifacts/sts2-cli-lib/` are
v0.107.1, build 23811903, and must not be re-copied from Steam — that would silently
re-pin the project to whatever is current.
