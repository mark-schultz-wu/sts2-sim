# sts

An STS2 simulator, meant to be fast enough to train agents in.

The real game can be driven programmatically, but only at a few hundred steps/s —
against the ~10⁹ a training run wants. So the game isn't the environment. It's the
reference the simulator gets checked against, step by step, on the same seed.

So far that's four mods and a Rust client for them. The simulator itself isn't
started.

| mod | what it does |
|---|---|
| [`sts2_determinism`](mod/sts2_determinism/README.md) | makes the game reproducible: same seed, same run |
| [`sts2_headless`](mod/sts2_headless/README.md) | drives it with no renderer and no human, over a line protocol |
| [`sts2_determinism_test`](mod/sts2_determinism_test/README.md) | measures what the first one claims |
| [`sts2_headless_test`](mod/sts2_headless_test/README.md) | the legality suite for the second |

Each test mod is installed only to run its suite, so neither shipped mod carries
tests. `mod/deploy.sh` builds and installs all four; the game's own manifest
`dependencies` field orders them.

[The protocol](mod/sts2_headless/PROTOCOL.md) is the wire contract on one page.
The standard the mods are held to is split between
[determinism](mod/sts2_determinism/REQUIREMENTS.md) (D1–D3) and
[headless](mod/sts2_headless/REQUIREMENTS.md) (H1–H12), so a requirement's
prefix names the file it lives in.
