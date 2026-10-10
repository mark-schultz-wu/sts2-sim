---
name: coverage-grind
description: One iteration of the STS2 coverage loop. Read `cargo xtask coverage`, port the next batch of content it counts as missing, and land it as the next stacked PR. Use it with /loop to grind unsupervised.
---

# Coverage grind

**Goal:** `cargo xtask coverage` reports every row at 100%. That covers
every act's moves, monsters, encounters and events, every card pool's
plays, both with code and complete, and every relic pool, potion and
power.

The tool counts code that exists. So two kinds of gap show up in it, and
both are fixed here:

- **The tool misreads.** The code exists, but the tool cannot see it.
  A hand port sits in a file it does not read, or in a pattern it does
  not parse. Fix the tool, and prove the fix by naming the code it now
  finds. Never widen what it accepts without that proof.
- **The content does not exist yet.** Port it.

The census still guards everything. No PR here lets a character's lowest
stop or an act count drop.

One invocation is one iteration: one batch, one PR. Then stop and report
in a few lines. `/loop` calls it again.

## 1. Where the stack stands

- `git fetch -q forgejo`, then list the open PRs:
  `tea api "repos/mark/sts2/pulls?state=open"`.
- Base the new branch on the head of the highest open PR in the stack, or
  on `forgejo/main` when none is open. Check that the PR is still open
  right before branching, and again before pushing: a merged base
  swallows the stack.
- `git checkout -b agent/<name> <base>`. A failed checkout stops the
  iteration, because old `agent/*` branches linger.

## 2. Measure

```sh
cargo xtask coverage > $SCRATCH/coverage-<n>.txt
```

The report must name what each row lacks, not just count it. If a
section only counts, the first batch adds a listing of the missing names
to `xtask/src/coverage.rs`. That batch is a PR of its own.

Pick the batch in this order, and take the first that has work:

1. **Misreads.** A row the census contradicts is a misread. Runs clear
   act 2, yet the tool counts Hive moves missing. Check each name the
   tool lists against the source before porting anything.
2. **Powers holding back plays and moves.** The "unported powers" list.
   Each one completes a card or a move that already has code.
3. **Card plays.** One pool's missing plays together, the pool with the
   most first.
4. **Monsters.** One encounter with its monsters, Hive before Underdocks.
5. **Relics, then potions, then events.** One kind per batch.

Size a batch the way the census grind does: one kind, and a PR's worth.
A few cards of one pool, one encounter, one family of relics.

## 3. Port

For each item in the batch:

1. Read the game's code in `target/decompiled/v0.107.1-1692500715/`, and
   mirror it. An unported branch inside it stays `unimplemented!()`.
2. Write a scenario in `crates/sts2-golden/tests/scenarios/`. Record it
   with `STS2_UPDATE=1 timeout 3000 cargo test -p sts2-golden --test
   scenarios > $SCRATCH/record.txt 2>&1`, then compare without
   `STS2_UPDATE`. Never pipe cargo into `head`. Read the game's
   transcript in `target/sts2-oracle/<name>.jsonl`: a passing scenario
   can agree by accident.
3. Break the rule with an asserting edit, and watch its own scenario
   fail. Back the file up first and restore it with `cp`, never `mv`.
4. A mod bug the scenario exposes gets fixed in the same PR.

Content the census never reaches, like Underdocks, still needs a
scenario. `{"cmd":"fight","encounter":...}` reaches any encounter.

## 4. Check

- `cargo fmt`, `./tools/prose-lint.py <files>`, then `just check-repo`.
- `cargo test -p sts2-golden`, which is CI's golden job and needs the
  game. Its divergence test fails when the long run reaches past its
  `BASELINE` as well as short of it. A port that takes the run further
  raises `tests/golden/<run>/BASELINE` in the same PR.
- `cargo xtask instructions <label> --vs <last label>`. Explain any cost
  over 0.3%, or cut it.
- `cargo xtask coverage` again. Each row in the batch rises, and no row
  falls.
- The census: `cargo build -q --release -p sts2-golden --example
  random_run && timeout 1500 cargo xtask census --runs 300 --top 30`.
  Every character's lowest stop holds or rises, and nothing panics. An
  act count moves by a few when new content changes what random play
  picks. Explain any drop, and fix one that is a bug.

## 5. Land

- One commit. Write the message to a file and lint it with
  `./tools/vale-lint.py --stdin commit`. It ends with the
  `Co-Authored-By` line from the system prompt.
- The PR body is the message less its subject line and trailers, plus
  "Its base, #N, is open. This PR targets its branch." or "Its base is
  main." Then the "Generated with Claude Code" line. Lint it with
  `--stdin pr`.
- Push to `forgejo`, then open the PR with `tea pr create --base
  <base>`. Never push to main, and do not poll CI.

## 6. Report

Say what the batch ported, or what the tool now reads. Give each row
before and after, the census line, the instruction cost, and the PR
number.

## Stop and ask Mark instead of porting when

- An item belongs out of the count rather than ported, as content the
  game can never offer. The tool already leaves out the `Mock` and
  `Deprecated` models, and the cards only a run with more than one
  player offers.
- A port needs a design change to core types beyond what it needs.
- The oracle cannot record the scenario, and the fix to the mod is
  unclear.
- A regression has no explanation.
