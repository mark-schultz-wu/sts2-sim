# sts-schema

The language the simulator and the real games both speak.

Nothing here talks to a game. This crate defines *what an observation is*, *what an action
is*, and *what it means for two observations to disagree* — so that everything downstream
can be compared rather than translated.

## The problem

A differential test needs the simulator and the real game to produce comparable output.
There are two ways to arrange that, and only one of them works.

The obvious one is a wrapper per backend that converts each game's native output into
whatever the test wants. That puts a translation layer between the thing being tested and
the thing testing it, so every disagreement has three possible causes — the simulator, the
game, or the translation — and the translation is the one nobody suspects.

The other is to define the state type **once** and make every backend emit it directly. A
differential test is then a struct comparison. That is this crate. `SpireEnv` is the trait
every backend implements; `Obs` is what they all emit.

```
  simulator  ──impl SpireEnv──┐
                              ├──> Obs ──> diff::first_divergence ──> "step 14, piles.draw[2].id"
  real game  ──impl SpireEnv──┘
```

## Obs — one decision point

`Obs` describes the game at a point where somebody must choose: hp, gold, floor, act, the
current screen, the legal actions, and — in combat — energy, turn, block, monsters, and
the card piles.

Four things about it are decisions rather than details.

### Union, never intersection

STS1 exposes 109 state fields; STS2 exposes 161; the overlap is partial. Reducing to what
both share would discard STS1's pile contents and STS2's precomputed damage — the two
things most worth checking.

So the shape is a **shared core plus typed extensions**: fields both games have are
first-class, and each `Card` and `Monster` carries an `extra: BTreeMap<String, Value>` for
what only one game has (STS2 enchantments and keywords, STS1 `ethereal` and `exhausts`).
`Card::vars` is separate from `extra` and holds per-instance values — a Ritual Dagger with
accumulated damage versus a fresh one. Without it, two piles compare equal whenever the
card names line up.

### Absent means "cannot say", not "zero"

`energy`, `turn`, `block`, `Card::uid` and `Card::cost` are all `Option`. This is load
bearing in both directions:

- Outside combat, `energy: None` is different from `energy: Some(0)`.
- STS2 exposes no per-instance card handle at all, so `uid` is `None` there — permanently,
  not situationally.

`diff::compare` therefore only compares a field when **both** sides have an opinion. If it
treated `None` as a value, every STS2 diff would fail on `uid` and bury the real signal.

### Provenance is mandatory

Every `Obs` carries which game, which build, which adapter, and which mods were loaded.
No field has a serde `default`, so an observation that cannot say what produced it fails
to parse — there is a test pinning each field separately.

`mods` is the one that most wants a default and most must not have one. Defaulting it
means a producer that forgets the list emits an observation claiming *vanilla* — both the
commonest configuration and the exact false claim that went unnoticed. An empty list has
to be an assertion, not an absence.

This is not bookkeeping. A determinism mod once went unloaded for weeks because the mod
list was set in two places and no result recorded which target had produced it. Making
provenance structural turns a class of silent wrongness into a parse error.

### RNG counters are part of the state

`rng: BTreeMap<String, Stream>` records each named stream's seed and draw counter. This is
the difference between a report that says *"the states differ"* and one that says
*"`shuffle` advanced 14 draws on one side and 8 on the other"* — the first is nearly
useless, the second names the bug.

Availability varies (STS2 emits all 15 streams; STS1 only under the `rngspy` mod), which is
exactly why the compare-only-what-both-have rule exists.

### Raw — evidence, not state

`Obs::raw` carries the untouched backend payload, and does two unusual things: it
**compares equal to everything**, and it **never serialises**.

Both follow from what it is for. Two observations that agree on every modelled field *do*
agree, whatever their source payloads looked like; including `raw` in equality would make
every cross-implementation comparison fail by construction. And serialising it would
roughly double a trace.

It exists because normalisation is lossy and the loss is invisible. Every investigation so
far — a stale acknowledgement, a half-drawn hand, an unmodelled dialogue screen — needed
the original payload, and each time it meant writing a throwaway probe, because a report
can only show fields somebody thought to model.

## Action — fully formed, never guessed

An `Action` is a `kind`, a human-readable `label` for reports, and its `args`. Backends
enumerate the legal set; callers pick from it.

The alternative — a caller constructing `play 1` and discovering it needed `play 1 0` —
is a class of bug that shows up as an illegal-action error at a random depth. A search or
a policy needs the enumerated set anyway, so nothing is lost by requiring it.

## SpireEnv — the stepping interface

```rust
pub trait SpireEnv {
    fn observe(&self) -> Obs;
    fn legal_actions(&self) -> Vec<Action>;
    fn step(&mut self, action: &Action) -> Result<(), StepError>;
    fn done(&self) -> bool { self.observe().done }
}
```

**`step` returns `()`, not an `Obs`.** Stepping must not force observation. An earlier
single-call version made every step pay for a full report — 2.4 µs and 67 allocations —
even when the caller only wanted to advance. A differential test steps both sides then
observes both; a search steps and observes only when it wants to look.

**`StepError` separates an illegal action from a dead backend.** They are different in
kind: a rejected action is a policy bug the driver recovers from by choosing again, while a
closed pipe or a dead container is unrecoverable. Collapsing both into a string is what
makes a harness retry something that will never succeed.

### Policy randomness is passed in, not owned

`Policy::act` takes `&mut self` — real agents carry a network, a search tree, counters —
but its randomness arrives as a `&mut PolicyRng` argument.

An agent owning a hidden `thread_rng` is a clock by another name: it looks deterministic,
replays differently, and the harness cannot notice. Three separate harnesses in this
project each rolled their own "deterministic" play loop; one oscillated a variable number
of times before escaping, and the resulting non-reproducibility was misread as a game bug
until the traces were diffed. With the generator supplied by the caller, *same seed plus
same agent* is reproducible by construction.

`PolicyRng` is deliberately **not** one of the game's streams. An agent drawing from
`shuffleRng` would shift every later reshuffle in the game it is playing.

## What this crate is not

`Obs` is a **report** type — owned `String`s, `BTreeMap`s, `Vec`s. Against oracles running
at 6–163 steps/s it is four orders of magnitude cheaper than the thing it measures, so the
cost is irrelevant and self-describing actions are worth it.

It is the wrong shape for tree search or training, which want the raw state and a `Copy`
action. If a rollout ever appears in a profile, check it is not going through here.

## Contents

| | |
|---|---|
| `src/lib.rs` | `Obs`, `Action`, `Card`, `Monster`, `Provenance`, `Stream`, `Raw`, `Screen`, `Pile` |
| `src/env.rs` | `SpireEnv`, `StepError`, `Policy`, `PolicyRng`, `rollout` |
| `src/diff.rs` | `compare`, `rng_deltas`, `first_divergence` |
| `src/bin/schema-check.rs` | validates newline-delimited JSON observations from a producer |

`first_divergence` halts at the first disagreeing step rather than collecting every
difference: once two runs diverge they are exploring different games, so later deltas are
consequences rather than evidence.

```
cargo test -p sts-schema
some-producer | cargo run -q -p sts-schema --bin schema-check
```

## Known rough edges

This crate was imported from an earlier branch and has not yet been reworked. Carried over
with it:

- **Stale references to a Python producer.** `lib.rs` and `schema-check.rs` both describe
  an `oracle/types.py` mirror and a `python3 -m oracle.schema` invocation. That facade no
  longer exists and will not — the oracle adapters are Rust. `schema-check` is still
  useful as a conformance check on any JSON producer, but its documented usage is fiction.
- **A dangling doc reference.** `diff.rs` cites `docs/spec/08-interface.md` §8.5, which
  lives on `spikes-and-sts2-source` and not here.
- **Possible duplication.** `sts-diff/src/lib.rs` on that branch is 492 lines and may
  overlap substantially with `diff.rs`; that needs resolving before both land.
- **`env.rs` bundles four concerns** — the trait, a generator, two baseline policies, and
  a driver loop — which is fine now and probably wrong once agents are real.
