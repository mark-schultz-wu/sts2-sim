# Other tools for Slay the Spire AI

Other projects do some of what this one does, and some do more. This page
compares them as of 2026-11-01. Facts about another project come from its own
README or paper, unless a line says we measured it. If a line here is wrong,
please open an issue.

## Simulators of Slay the Spire 2

| Project | Language | Builds | Scope | How it is checked | Python |
|---|---|---|---|---|---|
| this one | Rust | v0.107.1, v0.111.0 | whole runs, all content, co-op | scenarios, recorded runs and hunts against the game ([STATUS.md](STATUS.md)) | no |
| [spire-mentor](https://github.com/tfriedel/spire-mentor) | Rust, with Python and TypeScript | v0.107.1 | whole runs, all content | parity tests and fuzzing against the game's code: 99.4% to 100% by character | yes |
| [sts2-solver](https://github.com/ytcs/sts2-solver) | Rust | v0.111.0 | combat | a headless oracle and a differential tester | yes |
| [sts2-rl-agent](https://github.com/zhiyue/sts2-rl-agent) | Python | not stated | whole runs, all characters | "verified against the decompiled C# source" | yes |
| [slay-the-spire-2-emulator](https://github.com/Zamiell/slay-the-spire-2-emulator) | C# | not stated | part of Act 1 for the Ironclad | traces against the real game | yes |

Smaller STS2 projects include [evangambit/sts2](https://github.com/evangambit/sts2),
[r33hab/sts2](https://github.com/r33hab/sts2) and
[StS2Sim](https://next.tangled.org/cee.wtf/sts2-sim), which runs the game's own
assembly against a test dummy.

### spire-mentor

spire-mentor is the closest to this project, and ahead of it in several ways.
It comes with Python bindings and a trained AI: MCTS for fights and PPO for the run.
It has a web workbench, an MCP server, and a mod that plays the AI in the real
game. It reports its win rates. Its engine is faster too. On our fight
benchmark we measured it at about 7,400 instructions per action, against about
9,900 for ours.

This project adds the second build, co-op runs, and its own way to check
against the game.

## Interfaces to the real game

These drive the real game rather than simulate it. They are what most LLM
benchmarks use, because they need no port of the rules.

- [STS2MCP](https://github.com/Gennadiyev/STS2MCP): a mod with a REST API and
  an MCP server.
- [STS2-Agent](https://github.com/CharTyr/STS2-Agent): a mod with an HTTP and
  MCP interface.
- [autoSpire](https://github.com/LightEnding/autoSpire): an MCP mod with co-op.

The mods in this repository drive the game too, but for checking the
simulator. They are not a general interface.

## Slay the Spire 1

- [sts_lightspeed](https://github.com/gamerpuppy/sts_lightspeed): a fast C++
  simulator of the first game, for the Ironclad.
- [CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod) and
  [spirecomm](https://github.com/ForgottenArbiter/spirecomm): the interface
  most STS1 bots and benchmarks drive the game through.
- [bottled_ai](https://github.com/xaved88/bottled_ai): a rule-based bot on
  CommunicationMod.

This project does not cover the first game.

## Benchmarks and papers

- [SpireBench](https://arxiv.org/abs/2610.12596): LLMs playing STS1 at its
  highest difficulty.
- [AgenticSTS](https://github.com/AlayaLab/AgenticSTS): an STS2 testbed for
  LLM agents with memory.
- [Epoch AI](https://epoch.ai/publications/ai-systems-are-improving-rapidly-at-slay-the-spire-and-its-sequel):
  LLMs playing both games through the screen.
- [Orak](https://arxiv.org/abs/2506.03610): a benchmark across many games,
  STS1 among them.
