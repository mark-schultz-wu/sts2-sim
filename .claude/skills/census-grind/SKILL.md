---
name: census-grind
description: One iteration of the STS2 port loop. Run the census, port the stops that hold each character's lowest run down, and land them as the next stacked PR. Use it with /loop to grind unsupervised.
---

# Census grind

**Goal:** raise the lowest floor each character reaches in the census,
300 runs a character. A run ends on its first panic, and nothing steps round
one. A panicking move is a priority to port, never a move to avoid.

One invocation is one iteration: one batch, one PR. Then stop and report in a
few lines. `/loop` calls it again.

## 1. Where the stack stands

- `git fetch -q forgejo`, then list the open PRs:
  `tea api "repos/mark/sts2/pulls?state=open"`.
- Base the new branch on the head of the highest open PR in the stack, or
  on `forgejo/main` when none is open. Check that the PR is still open right
  before branching, and again before pushing: a merged base swallows the
  stack.
- `git checkout -b agent/<name> <base>`. A failed checkout stops the
  iteration, because old `agent/*` branches linger.

## 2. Measure

```sh
cargo build -q --release -p sts2-run
cargo xtask census --runs 300 --top 20 > $SCRATCH/census-<n>.txt
```

Read **"lowest stop, by character"** first. The batch is the stops at the
lowest floor, over every character whose lowest stop is that floor. Group
them by kind: Neow relics together, one card pool's plays together, one
power's hooks together, one encounter with its monsters. When the lowest
floor has more than one PR's worth, take the kind that stops the most runs
there.

If a character's **lowest loss** is below its lowest stop, check that the
death is legitimate before anything else. Find the fight, and compare the
decisive numbers with the decompiled game. A wrong death is a bug to fix. A
right one is reported, and the goal for that character is its lowest stop.

## 3. Port

For each stop in the batch:

1. Read the game's code in `target/decompiled/v0.107.1-1692500715/`, and
   mirror it. An unported branch inside it stays `unimplemented!()`.
2. Write a scenario in `crates/sts2-golden/tests/scenarios/`. Record it from
   the game with `STS2_UPDATE=1 timeout 1500 cargo test -q -p sts2-golden
   --test scenarios`, then compare without `STS2_UPDATE`. Read the game's
   transcript: a passing scenario can agree by accident.
3. Break the rule and watch the scenario fail for the right reason. Back the
   file up first and restore it with `cp`, never `mv`.
4. A mod bug the scenario exposes gets fixed in the same PR.

## 4. Check

- `cargo fmt`, `./tools/prose-lint.py <files>`, then `just check-repo`.
- `cargo test -p sts2-golden`, which is CI's golden job and needs the
  game. Its divergence test fails when the long run reaches past its
  `BASELINE` as well as short of it. A port that takes the run further
  raises `tests/golden/<run>/BASELINE` in the same PR.
- `cargo xtask instructions <label> --vs <last label>`. Explain any cost
  over 0.3%, or cut it.
- Census again. Every character's lowest stop holds or rises, and every act
  count holds or rises. A drop is the next thing fixed, unless it is a silent
  gap made loud, which the PR says.

## 5. Land

- One commit. Write the message to a file and lint it with
  `./tools/vale-lint.py --stdin commit`. It ends with the `Co-Authored-By`
  line from the system prompt.
- The PR body is the message less its subject line and trailers, plus
  "Its base, #N, is open. This PR targets its branch." or "Its base is
  main." Then the "Generated with Claude Code" line. Lint it with
  `--stdin pr`.
- Push to `forgejo`, then open the PR with `tea pr create --base <base>`.
  Never push to main, and do not poll CI.

## 6. Report

Say which stops went, each character's lowest stop before and after, the
act counts, the instruction cost, and the PR number.

## Stop and ask Mark instead of porting when

- a stop needs a design change to core types beyond what the port needs;
- the oracle cannot record the scenario, and the fix to the mod is unclear;
- a regression has no explanation.
