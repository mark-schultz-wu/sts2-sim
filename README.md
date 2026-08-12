# sts

Agents that play **Slay the Spire 2**, and the simulator they train in.

The game can already be driven programmatically, but only at a few hundred steps/s.
Training wants on the order of 10⁹ steps, which at that rate is weeks. So the real game
is not the environment — it is the **oracle**: the reference that a fast reimplementation
is checked against, step by step, on the same seed.

That makes correctness the hard part rather than speed.

## Where this is

Nothing is built yet. An earlier `sts-schema` crate tried to define a shared vocabulary
for both games before anything could produce or consume it, and was removed. It is
preserved in commit `604ef72` if any of it turns out to be worth recovering.

The current goal is a working oracle: ergonomic bindings against the real STS2, modded if
that is what it takes, that a simulator can be diffed against. Types get defined when
something on both ends of them exists.

[What the oracle has to do](mod/sts_oracle/REQUIREMENTS.md) is the standard it is held to —
written to be failed, so the tool can be checked against it rather than argued about.

## Scope

STS1 is out of scope. The pinned STS2 assemblies in `~/sts-artifacts/sts2-cli-lib/` are
v0.107.1, build 23811903, and must not be re-copied from Steam — that would silently
re-pin the project to whatever is current.
