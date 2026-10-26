---
name: blocker-grind
description: One iteration of the port loop, solo and co-op. Take the first open item in BLOCKERS.md, fix it, update the list with what the fix found, and land it as the next stacked PR. Use it with /loop to grind the list down.
---

# Blocker grind

**Goal:** an empty `BLOCKERS.md`, which means every recorded run matches
the simulator to its end. The file says what done means and how the
list changes. Read it first, every iteration.

One invocation is one iteration: one item, or the part of one that
fits a PR. Then stop and report in a few lines. `/loop` calls it again.

## 1. Where the stack stands

- `git fetch -q forgejo`, then list the open PRs:
  `tea api "repos/mark/sts2/pulls?state=open"`.
- Base the new branch on the head of the highest open PR in the stack,
  or on `forgejo/main` when none is open. Check that the PR is still
  open right before branching, and again before pushing: a merged base
  swallows the stack.
- `git checkout -b agent/<name> <base>`. A failed checkout stops the
  iteration, because old `agent/*` branches linger.

## 2. Pick the item

The first item under **Open** in the branch's `BLOCKERS.md`.

- Too big for one PR: split it into items that each fit one, in the
  same commit as the first part.
- Needs Mark's decision: stop the loop and ask. Never guess a design.
- Blocked by something not on the list: add that above it, and work it
  first.

## 3. Fix it

What the fix touches decides the checks.

- **Game rules.** Read the game's code in
  `target/decompiled/v0.107.1-1692500715/`, and mirror it. An unported
  branch inside it stays `unimplemented!()`. Write a scenario in
  `crates/sts2-golden/tests/scenarios/`, record it with
  `STS2_UPDATE=1 timeout 3000 cargo test -p sts2-golden --test scenarios`,
  compare without `STS2_UPDATE`, and read the game's transcript in
  `target/sts2-oracle/<name>.jsonl`. Break the rule with an asserting
  edit, back the file up first, restore it with `cp`, and watch the
  scenario fail.
- **The mod.** `just mods`, `./mod/deploy.sh`, then `just selftest` and
  `just determinism-test`, which CI's golden job runs. Changing the mod
  changes its identity, so every transcript is asked again. Never build
  `sts2-golden` from a tree whose mod differs from the deployed one.
  The build deletes each transcript before it asks the game again.
- **Tooling.** Reusable tools are `cargo xtask` subcommands.

A mod bug the work exposes gets fixed in the same PR.

## 4. Check

- `cargo fmt`, `./tools/prose-lint.py <files>`, then `just check-repo`.
- `cargo test -p sts2-golden`, which needs the game. The divergence test
  fails when the long run reaches past its `BASELINE` as well as short
  of it. A fix that takes the run further raises `BASELINE` in the same
  PR.
- `cargo xtask instructions <label> --vs <last label>`. Explain any cost
  over 0.3%, or cut it.
- The census: `cargo build -q --release -p sts2-golden --example
  random_run && timeout 1500 cargo xtask census --runs 300 --top 30`.
  Every character's lowest stop holds, and nothing new panics, unless
  the PR adds the new stop to `BLOCKERS.md` and says why.
- Once recorded runs exist, they are the measure. A fix moves the
  matched steps up, and none falls.

## 5. Update the list

In the same commit: delete what this PR fixed, add every new blocker
it found, in order, with what it needs and where it shows.

## 6. Land

- One commit. Write the message to a file and lint it with
  `./tools/vale-lint.py --stdin commit`. It ends with the
  `Co-Authored-By` line from the system prompt.
- The PR body is the message less its subject line and trailers, plus
  "Its base, #N, is open. This PR targets its branch." or "Its base is
  main." Then the "Generated with Claude Code" line. Lint it with
  `--stdin pr`.
- Push to `forgejo`, then open the PR with `tea pr create --base
  <base>`. Never push to main, and do not poll CI.

## 7. Report

Say what the item was and what landed, what the list gained or lost,
the census line, the instruction cost, and the PR number. Say which
item is next.

## Stop and ask Mark when

- An item needs a design decision, or says it does.
- A fix needs a change to core types beyond what it needs.
- The oracle cannot record what the fix needs, and the fix to the mod
  is unclear.
- A regression has no explanation.
- The next item is the speed target, which Mark sets.
