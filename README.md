# sts

Agents that play **Slay the Spire 1 and 2**, and the simulator they train in.

Both games can already be driven programmatically, but only at 6–870 steps/s. Training
wants on the order of 10⁹ steps, which at that rate is weeks. So the real games are not
the environment — they are the **oracle**: the reference that a fast reimplementation is
checked against, step by step, on the same seed.

That makes correctness the hard part rather than speed. Every backend — simulator and real
game alike — implements one trait and emits one state type, so a differential test is a
struct comparison and a divergence names the step and field where it started.

Both games are in scope because they share a skeleton but differ in every detail, which
keeps the engine parameterised instead of quietly specialised to STS1.

## Layout

| | |
|---|---|
| `sts-schema` | the shared vocabulary: `Obs`, `Action`, the `SpireEnv` trait, and the diff |
