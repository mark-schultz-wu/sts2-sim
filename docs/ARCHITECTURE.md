# Architecture

Where things live, and why they live there. Half of this describes crates that
do not exist yet. The layering was worked out before the engine was started, so
that the first file written lands in the right place. What is built and what is
planned is marked below.

Three of the decisions here were made wrong first and corrected. The wrong
versions are kept, because a rejected design is what stops the same design being
proposed again.

## The dividing rule

Everything in this project sorts into one of two piles.

**Content** is anything a dump can produce. A card's cost, a monster's HP range,
which encounters an act fields, what an enchantment can be put on. It is
generated from the running game, never typed, and checked against the dump it
came from.

**Code** is everything a human ports: what a card *does*, how a monster picks a
move, how the map is generated, how rewards roll.

The line is not "is it a number". Orb values are content. The odds constants in
`Core.Odds` are not. They are algorithm parameters, and half of them are inline
literals in method bodies rather than `const` fields. A dump of the fields looks
authoritative while omitting the rest.

A dump reports `dynamic` where a value is perhaps code, and the Rust side
generates `Field::Computed`. Every dump errs the same way. Marking a constant as
computed costs one method, and the reverse costs a divergence nobody can see.

## The crates

Built:

```
sts2-core       the shared vocabulary: decisions, observations, refusals, and
                the traits an implementation of the game satisfies. Both the
                oracle and the engine implement them. See STS2-CORE.md.
sts2-rng        the game's draw stream, verified against 1024 draws per seed
sts2-content    <build>/generated/    xtask writes this, never hand-edited
                <build>/dumps/        what the game printed
sts2-headless   a client for the headless mod, the oracle's transport
                recording.rs          a run, as the decisions that made it
sts2-cli        `sts2`: a daemon holding one game, and one command per
                invocation. `sts2-fuzz`: random legal play until something
                breaks
xtask           cargo xtask dump, cargo xtask codegen
```

`sts2-cli` is a front end, and the parts of it that are opinions say so. How an
observation reads at a terminal is its choice and not the oracle's, which is why
none of that lives in `sts2-headless`.

Planned, and the subject of most of this document:

```
sts2-content    <build>/behaviour/    hand-ported, never generated
sts2-sim-core   the nouns and the traits. Combat state, creature, piles, and the
                hook trait content implements. No content, no engine.
sts2-sim        the engine. Turn loop, map generation, rewards, shops, run
                progression. Generic over a content version.
```

Dependencies run `sts2-content → sts2-sim-core` and `sts2-sim → sts2-sim-core`.
Content depends on the traits it implements. The engine depends on the same
traits and calls them. Neither depends on the other.

The cycle that forces a third crate is worth stating, because it is not obvious
and every link in it is load-bearing. Behaviour is version-scoped, so it lives in
`sts2-content`. Strike's behaviour mutates combat state, so it must name that
type. Combat state belongs to an engine that is generic over the version, so the
engine cannot be named by content without pointing back. A crate both can name
and neither owns is what breaks it.

**Rejected: declaring those nouns and traits in `sts2-content` instead**, to save
a crate. It breaks the same cycle. Content states what it needs from a combat
state, and the engine provides it, so the objection is not correctness.

What it costs is the rule that makes `sts2-content` legible. That crate is
scoped to the game's own content. It is dumped from the C# binaries, or
hand-ported from them, and checked against the dump it came from. A mutable `Creature`
holding current HP is neither. It is a state machine this project designed.
Beside the generated tables it leaves the crate with no one-sentence answer to
"what belongs here". That question is what codegen, the dumps, the version
modules and the generated/ vs behaviour/ split are all organised around. A crate
is cheaper than that rule.

It also kept the dependency honest in a small way. `sts2-core` takes
`sts2-content` for the `ContentVersion` trait alone. The alternative compiles
engine declarations it never uses, in `sts2-headless` too.

## Seeing the map

`sts2 show map` draws the act. It is a command of its own rather than part of
every reply, because the map does not change while a fight is being played.
Fifteen rows printed after every card bury the fight.

Which room to enter next is the decision above every other one in a run, and it
is made by looking several rows ahead. That means whether the elite comes before
the shop, and whether this branch reaches a campfire at all. The one-line
summary in an observation answers "where am I and what is adjacent", which is
the wrong question for that.

The observation carries the graph (`map.points`) beside the legal moves
(`map.travelable`), because neither can be derived from the other. Legality
depends on the run and on free travel, and the graph does not. The renderer
marks the node the run is on, the nodes it can step to, and the ones already
walked. It draws the edges the game itself draws.

## Playing at random

`just fuzz` starts a run, stacks a hundred `Gorget`, and takes legal moves at
random until the run ends or something breaks. It drives its own game process
rather than the daemon, and records every move, so a finding is a file rather
than a story.

Three things count as a finding. The game refuses a move the oracle offered, the
game logs an error while running one, or the game dies. The first is the useful
one. It means the legality the oracle reports and the legality the game enforces
have come apart. That is precisely the class of bug a simulator inherits.

The legal set comes from `cli::moves`, which enumerates what `cli::choose`
accepts. That shared source is deliberate. A fuzzer with its own idea of
legality finds disagreements between itself and the CLI, rather than between the
oracle and the game.

It earns its keep immediately. Three runs found two real defects nobody
constructs by hand. One is a shop offering a potion to a full belt: stocked,
affordable, and refused. The other is every run stalling on floor 17 with the
act boss dead and nothing legal. The act transition was not in the protocol at
all.

## Recording a run

A run is a seed and a sequence of decisions. Determinism (D1) is what makes that
a complete description. The same decisions from the same seed reproduce the run
exactly, so a recording stores no state at all.

A recording is only meaningful against a build. The steps name cards, encounters
and rooms, and one replayed against the wrong version is not a broken file. It
is a different run that happens to parse. So the build is the type parameter,
`Recording<V>` beside `Oracle<V>` and `Command<V>`, and there is no version
field. The stamp on disk is checked against `V` on the way in. It goes through
`Stamp::describes`, which also decides whether a connecting game is the right
build.

It lives in `sts2-headless` because a step is a `Command<V>` and there is no
other candidate. It was briefly in `sts2-content`, generic over an abstract step,
which was wrong twice over. `sts2-content` knows what a build contains and has
no protocol to record. And a step type nothing else ever supplies is an
abstraction with one caller.

**Superseded**: [STS2-CORE.md](STS2-CORE.md) moves it to `sts2-core`, which is
now that other candidate. A recording is a list of decisions, and decisions stop
belonging to the transport once a second implementation takes them. The
`sts2-content` rejection above still stands: a step type with two real
implementations is not an abstraction with one caller.

The format is JSON Lines: a stamp, then one line per step. One line rather than
one document, because a recording is written while the run is still being
played. A session that is killed or times out then leaves a file complete up to
its last decision. The daemon relays commands untyped but records them typed. So
a line that cannot replay is refused when it is written, rather than when it is
read.

`sts2 run replay` picks a run back up rather than reproducing it and stopping.
The game is left sitting at the last decision, and ordinary commands carry on
from there.

`sts2 run start` records by default, to
`<state dir>/sts2/runs/<seed>-<character>.jsonl`. It is named for the run,
because that is the question anyone opening the directory is asking. It is
numbered on repeat, because a second attempt at a seed already kept is
deliberate. Which run turns out to be worth having is recognised afterwards, and
one that was not written down cannot be shown to anybody. A few kilobytes is the
cheaper side of that trade. `--record FILE` overrides the name, and `--no-record`
declines.

Replaying is the exception, where writing is always asked for. A bare `replay`
records nothing and does not touch the file. A command that silently appends to
a file nobody named is one you have to remember not to run. The file it appends
to is the one being investigated. On a replay, `--record FILE` says
where the run goes from here. Name the file being replayed and it is carried on,
appended to, since the steps just sent are already in it. That also means a
replay that fails halfway cannot truncate the run it was opened to look at. Name
any other file and it is written fresh, holding the replayed steps and
everything after.

`--steps N` lands mid-run, which makes a branch. Pointed at a fresh file, that is
just a shorter run. Pointed at the source it rewrites it, which is destructive,
and is what naming that file asked for.

It earns its place twice. Now, an interface complaint stops being anecdotal. The
run that read badly can be replayed to the exact decision, as often as the
rendering needs changing. Later, it is the input to a divergence test. The same
steps are driven into the oracle and into the simulator and compared step by
step. The recording is the thing that says what "the same run" means. `just live`
already runs that shape, with a second game standing in for the simulator that
does not exist yet.

## Why behaviour lives in content

This was decided the other way first. The obvious arrangement is to keep
`sts2-content` purely generated and put ported behaviour in the engine, next to
the code that calls it.

That is wrong, and the reason is versioning. `sts2-content` is version-scoped by
design, with a feature per build, so two versions can coexist and be diffed.
Behaviour living in the engine forces the engine to switch on version, as
`match (version, card)`. Version-specific behaviour belongs beside
version-specific tables. The engine stays generic over which version it is
handed.

`generated/` and `behaviour/` sit side by side under the version module. Codegen
only ever writes into `generated/`. Every file there opens with a banner saying
so, and that is what keeps the boundary legible inside one crate.

The arrangement costs something. Content and engine are different crates, so the
engine's state has to be reachable through a trait. MegaCrit pays that same cost
with `ICombatState`.

## Naming a build, once

`ContentVersion` in `sts2-content` is the seam. The generated tables are
concrete types per build. Without the trait, anything working against content is
pinned to one of them. The trait turns that into a type parameter, and the
associated types carry every id.

**The rule: reaching into a version module for anything but its marker type is a
mistake.** A caller names `V0_107_1` once and takes the rest through the trait:

```rust
type V = V0_107_1;
type Character = <V as ContentVersion>::Character;
```

not `use v0_107_1::characters::Character`, which puts the build back into every
call site. That is the exact coupling the parameter exists to remove. With the
aliases, pointing a test, an example or a binary at another build is one line.

The bounds are spelled out on each associated type rather than hidden behind a
blanket-implemented alias trait. An alias abbreviates and does nothing else. It
constrains nothing the bounds do not, and it makes a missing bound report as
`V::Card: Id` instead of naming the trait actually missing.

`Error` stays concrete. An error type generic over the build puts the parameter
in every signature that can fail, to carry a diagnostic string nobody matches
on.

The parameter earns more than typing. `ContentVersion::VERSION` lets
`Oracle::launch` compare the build it is talking to against the build its tables
describe, and refuse. So a client physically cannot drive the wrong game. Before
this, that check was the caller's to remember.

The one door around it is `Oracle::send_raw`. The typed API is exhaustive over
what the tables know. That is exactly why it cannot express a request the game
must refuse. An encounter that does not exist is one, and a `RoomType` codegen
drops is another. Testing the refusal path needs those. That is a different
operation from driving the game, so it says so at the call site rather than in
the type of every field.

## Two things called hooks

Worth separating, because they point in opposite directions and only one of
them is forced by the crate layout.

- **`trait Relic { fn modify_damage(&self, state: &mut CombatState, …) }`.**
  Content implements, engine calls. Needed regardless.
- **`trait CombatState`.** Engine implements, content calls. Needed only because
  content lives in a different crate from the state.

Both are declared in `sts2-sim-core`, which is what lets the dependency run one
way. The second one exists purely to pay for the crate boundary. An engine with
behaviour inside it needs no such trait at all, and that arrangement was
rejected for versioning reasons above.

## The hook trait is a superset

`AbstractModel` declares 181 virtual hooks. 155 are answered by at least one
model in this build, and 26 are slots nothing listens on. The engine owns the
turn structure, meaning what points exist and in what order. A content version
says which of those points it uses. So the trait is the union, and each version
implements a subset. That is the game's own shape: `public virtual` with an
empty body, overridden by a handful of models.

Hooks fold:

```csharp
foreach (AbstractModel item in IterateCombatHookListeners(combatState))
    num = item.ModifyOrbValue(orb, num);
```

so **iteration order is semantics, not style**, for anything non-commutative.
The order is explicit code in `CombatState.IterateHookListeners`. It runs allies
then enemies. Per creature it takes powers first, then the monster, or relics in
slot order then potions. At run level it takes deck cards with their
enchantments, then relics, potions, modifiers and badges. It is ported directly
rather than dumped, because reading it as data loses that it is a sequence.

Three ways the surface can change between builds, and what catches each:

| change | caught by | cost |
|---|---|---|
| hook added | the hook test: a version answers a hook the trait lacks | add a call site |
| signature changed | compile error in that version's `behaviour/` | localised |
| **call site moved** | oracle divergence only | forks that engine step |

Only the third is expensive, and nothing is designed for it yet. We have one
build dumped, so any scheme for version-varying call sites is invention rather
than measurement.

## Sharing behaviour across versions

Not yet built. The mechanism is name resolution rather than macros, because the
compiler checks it.

The second build to test against now exists. v0.111.0 moved 19 cards in and 1
out, 12 powers in and 2 out, and 13 encounters in and 3 out. Its hook surface
went from 181 to 182, and the two that moved are renames of the same shape. The
monster set did not move at all.

Behaviour is written against *unqualified* names (`Power::Vulnerable`,
`Card::Wound`). Each version module includes it under its own prelude:

```rust
// v0_108_0/behaviour/mod.rs
use super::generated::*;
include!("../../v0_107_1/behaviour/cards/strike.rs");  // unchanged in 108
mod bash;                                              // 108 rewrote it
```

Sharing is one line, and divergence is a real file. If v108 removed
`Power::Vulnerable`, the include fails to compile. The reuse is verified, not
assumed. How far it gets depends on how much behaviour names other content
items. That is a number to measure against the second dump, not to design around
now.

## How a table is known to be right

Four layers, each catching what the one before cannot.

1. **The build stamp.** Every dump names the build it came from:

   ```
   # build v0.107.1 commit 59260271 assembly 1692500715 content 3954186980
   ```

   Two hashes, because one is not enough. `content` is XxHash32 over the *set
   of model ids*. It moves when content is added, removed or renamed. It does
   **not** move when a card's damage goes from 6 to 7. A build that quietly
   rebalanced every number still accepts a dump anchored on it alone. `assembly`
   is shipped in the game's `release_info.json`, and moves on any code change.
   Codegen refuses to generate when two dumps disagree on any of it.

   Neither hash says anything about **the mod**, because redeploying one moves
   neither. So the greeting carries a third identity: the first 8 bytes of a
   SHA-256 over the mod's own sources, produced by
   `mod/sts2_headless/identity.sh`. That script is run by the mod's build and
   again by `sts2-headless`'s `build.rs`, so one script answers for both ends.
   The client refuses a mismatch at the handshake.
   It replaced a hand-maintained `protocol` integer, which had the same job and
   had to be remembered.

2. **The replay.** Each domain's test walks its dump row by row against the
   generated table. This catches a table that was not regenerated.

3. **Claims about the game.** A replay cannot check something the dump measured
   *inside* the running game. Fourteen encounters roll their roster. Gold moves
   at Poverty and nowhere else. Focus does not reach Plasma's energy. Each test
   file pins those by name, so a build that changes one has to say so in a test
   rather than sail through.

4. **The oracle.** `sts2-headless` drives the real game one decision at a time
   and compares digests. It is the only thing that checks a *body* is right, and
   the only thing that catches a hook firing at the wrong point.

   The digest it compares is `Observation<V>::digest`, taken in Rust over the
   typed observation, so both implementations derive it from the same value. The
   mod still sends one of its own, a SHA-256 over a JSON string built in C#.
   That one can only ever compare the game against another copy of the game.
   `just live` asserts the two agree about whether two runs are the same run.
   That is what catches an observation that dropped a field the game's state
   has. `just live` still runs a second game where the simulator will go,
   because the simulator does not exist yet.

## Dumping technique

Three rules the dumps arrived at, each from getting it wrong:

**Reflect over `ModelDb.AllAbstractModelSubtypes`, not the usage tables.**
`ModelDb.Monsters` and friends walk encounter and pool tables, so they miss
summon-only monsters and event-only encounters.

**Two lists are not one list.** `ModelDb.Orbs` holds four orbs, and nothing that
plays reads it. `OrbModel.GetRandomOrb` draws from a private `_validOrbs` holding
five. Generating from the first makes Chaos never channel a Glass orb. Where the
game states a thing twice, dump both and let a test hold them together.

**Do not build a taxonomy of the source.** An early `OrbAmount` sorted values by
whether the C# getter held a literal, a field or an expression. Plasma's evoke is
twice its passive, spelled `2m`. Glass's evoke is twice its passive, spelled
`PassiveVal * 2`. Same relationship, different variant. The enum described
MegaCrit's source rather than the game. Tables carry the numbers.

Where a dump measures something the table does not generate, the column stays in
the dump and a test pins it. `stored` does that for orb values, and
`refusesSomeCards` for afflictions. Codegen fails if a dropped column ever starts
varying.
