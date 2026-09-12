# sts2-content

Everything that the game contains, with one module for each build.

This crate holds cards, relics, potions, monsters, encounters, events, powers,
and acts. For each one it gives the cost, the damage, the pool it comes from,
and the act it appears in.

## Nobody types these numbers

A mod inside the game reads the numbers out of the game. The mod writes them to
a text file, which this project calls a dump.

`cargo xtask codegen` makes Rust from those dumps. No file in `generated/` is
written by hand, and each file there says so at the top.

A wrong number in a table is a fault in the dump or in the generator. The table
is never the place to correct it.

## One module for each build

A build is `v0_107_1`. Each build is a cargo feature, so two builds can be
present together and can be compared.

Two builds are here now:

| module | feature | build |
|---|---|---|
| `v0_107_1` | `v0_107_1` | v0.107.1, the default |
| `v0_111_0` | `v0_111_0` | v0.111.0, the beta line |

`v0_107_1` is the oldest build this repository can model. v0.107.1 replaced the
game's PRNG with xoshiro256\*\*, and `sts2-sim` owns that generator.
The simulator's RNG tests say what it takes to pin an older one.

A build is known by its hashes, not by its name. The same name can appear in more
than one build, and a name says nothing about whether the ids still agree. There are two
hashes:

- `contentHash` moves when content is added, removed, or renamed.
- `assemblyHash` moves when any code changes.

The tools compare both hashes before anything trusts a table.

## How to name a build

Name the build one time. Then take everything else through the trait:

```rust
type V = V0_107_1;
type Card = <V as ContentVersion>::Card;
```

If you name anything in `v0_107_1` except `V0_107_1`, the build goes back into
every line that names it. The trait parameter exists to remove that
coupling.

## The shapes a table is written in

Three types carry most of the tables, and each is a decision about how a
generated row is spelled.

### `Field`, for a value some content works out

Every domain has a few. A card overrides `Type`, a monster's branch weight is
written as a lambda, and a power's `StackType` follows the sign of its amount.
`Field::Computed` is the simulator's cue to ask rather than look up.

The dumps err towards `Computed`. Marking a constant as computed costs one
method. Marking a computed value as constant costs a divergence nobody can see.

### `Scaled`, for a number ascension changes

Ascension is a scalar, not a set of flags. `AscensionManager.HasLevel` is
`_level >= (int)level`, so each `AscensionLevel` is a threshold, and any value
behind `AscensionHelper.GetValueIfAscension` is a step function of the run's
level.

One threshold, not a list of them, because `GetValueIfAscension(level,
ascensionValue, fallbackValue)` takes exactly one. A second means a monster
nesting two calls, which nothing in this build does. Codegen refuses rather than
truncating if one ever appears.

It is also what keeps the type small. 433 intents carry two of these each, and a
slice costs 16 bytes apiece to say "no threshold" 319 times over.

The thresholds themselves are measured rather than read out of the source. The
dump reads every value at every level and records where it actually moves. That
is how Gremlin Merc turned up. Its damage steps at `ToughEnemies`, where every
other monster's steps at `DeadlyEnemies`.

### `Few`, for a short list laid out where it is used

The generated tables are full of "at most a handful". An encounter fields at
most four monsters, and can ever field at most six. A `&'static [T]` spells one
of those as a pointer and a length. That is sixteen bytes to reach a thing
usually smaller than the pointer, plus an indirection to follow and a relocation
to keep. `Few` is the same list inline.

`N` is a cap the generator checks rather than a guess. A build that grows a
fifth spawn fails generation instead of truncating. `Few::new` is a `const fn`
called from a `const`, so overflowing it is a compile error and not a panic in a
run.

Items are always a prefix, because `new` fills from the front. So a `None` is
the end of the list and never a hole in it.

## Four checks on a table

Four layers check a table, and each one finds what the layer before it cannot:

1. Each dump carries the stamp of the build it came from.
2. A test walks the dump row by row against the table.
3. Tests pin the claims that a dump cannot make alone.
4. The oracle plays the real game, one decision at a time.

Both builds have all four. Layers two and three are a `tests` module, ported
between builds rather than copied. Twenty-five of its assertions are counts and
sets that moved from v0.107.1 to v0.111.0.

`ContentVersion` and the `Table` traits are in `sts2-core`. This crate
implements them.
