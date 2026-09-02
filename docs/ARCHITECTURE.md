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
sts2-rng        the game's draw stream, verified against 1024 draws per seed
sts2-content    v0_107_1/generated/   xtask writes this, never hand-edited
                v0_107_1/dumps/       what the game printed
sts2-headless   a client for the headless mod, the oracle's transport
xtask           cargo xtask codegen
```

Planned, and the subject of most of this document:

```
sts2-content    v0_107_1/behaviour/   hand-ported, never generated
sts2-sim-core   the nouns and the traits. Combat state, creature, piles, and the
                hook trait content implements. No content, no engine.
sts2-sim        the engine. Turn loop, map generation, rewards, shops, run
                progression. Generic over a content version.
```

Dependencies run `sts2-content → sts2-sim-core` and `sts2-sim → sts2-sim-core`.
Content depends on the traits it implements. The engine depends on the same
traits and calls them. Neither depends on the other.

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
`Game::launch` compare the build it is talking to against the build its tables
describe, and refuse. So a client physically cannot drive the wrong game. Before
this, that check was the caller's to remember.

The one door around it is `Game::send_raw`. The typed API is exhaustive over
what the tables know. That is exactly why it cannot express a request the game
must refuse. An encounter that does not exist is one, and a `RoomType` codegen
drops is another. Testing the refusal path needs those. That is a different
operation from driving the game, so it says so at the call site rather than in
the type of every field.

## Two things called hooks

Worth separating, because only one of them forces a crate boundary.

- **`trait Relic { fn modify_damage(&self, state: &mut CombatState, …) }`.**
  Content implements, engine calls. Needed regardless.
- **`trait CombatState`.** Engine implements, content calls. Needed only because
  content lives in a different crate from the state.

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

Not yet built, and deliberately: it needs a second build to test against. The
mechanism is name resolution rather than macros, because the compiler checks it.

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
items. That is a number to measure against a second dump, not to design around
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
