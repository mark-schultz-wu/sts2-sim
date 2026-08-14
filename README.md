# sts

An STS2 simulator, meant to be fast enough to train agents in.

The real game can be driven programmatically, but only at a few hundred steps/s —
against the ~10⁹ a training run wants. So the game isn't the environment. It's the
reference the simulator gets checked against, step by step, on the same seed.

So far that's a mod, `sts_oracle`, which drives the game headlessly, and a Rust
client for it. The simulator itself isn't started.

[What the mod does](mod/sts_oracle/README.md) ·
[the protocol](mod/sts_oracle/PROTOCOL.md) ·
[the standard it's held to](mod/sts_oracle/REQUIREMENTS.md)
