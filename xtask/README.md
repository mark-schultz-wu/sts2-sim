# xtask

Repo chores with real logic in them. `cargo xtask <command>`.

Two of its commands build content. `instructions`, `trace`, `allocations` and
`census` measure the simulator, and their module docs say how.

`dump` runs the game's dump modes and files what each one prints. Most modes
describe content and land under `crates/sts2-content/src/<version>/dumps/`. Two
are ground truth for tests, which is not content, and land beside those tests.
Two modes launch nothing. `cardplaydump` and `monstermovedump` read the
decompiled assembly. The game's text is the game's own writing, so the
repository holds none of it. `sts2-cli`'s build script reads it from the
player's install.

`codegen` turns those dumps into `sts2-content`'s tables, one domain per dump,
and fails the build if two dumps came from different builds.

## Which build

Both commands work on the build `STS2_GAME_VERSION` names, and default to
`v0.107.1`. That is the same setting `.envrc` exports, `mod/build.sh` compiles
the mods against, and the `sts2` front end drives. A name with no lock under
`mod/games/` is refused, so a typo cannot create a directory nobody meant.

`dump` also needs `STS2_GAME_DIR` pointing at that build's install. The mods
refuse to start against any build but the one that built them. So a mismatch stops
rather than producing a dump from the wrong game.

The generated tree is committed rather than produced by a `build.rs`, so that a
content change reads as a diff. The whole point of this project is to see when
the game and the simulator disagree. "Which 40 cards did that patch touch" is
worth being able to see.

Each domain is a submodule of `src/codegen/` and a file or directory under
`generated/`. A domain reads exactly one dump and writes exactly one tree.
Adding relics means adding a dump, a submodule, and a line in `DOMAINS`.

This file holds the decisions each domain made about the shape of what it emits.
The module docs beside the code say what a domain generates. This says why the
table looks the way it does.

## What every domain shares

### The discriminant width

Layout only. Serde goes through the variant rename, so nothing on the wire or in
a dump depends on the number a variant compiles to.

It is still worth choosing rather than defaulting to the wide one, because a
discriminant sits next to other fields. An encounter's `Spawn` is a `Monster`
beside a slot index. That is two bytes when `Monster` is a `u8` and four when it
is a `u16`, and it appears four times in every roster.

So the domains differ. 88 cards in the largest pool and 126 monsters fit a byte,
and 273 powers do not.

### The two hashes

Every dump carries the build it came from on its `# build` line, as two hashes,
because one is not enough.

`content_hash` is XxHash32 over the *set of model ids*. It moves when content is
added, removed or renamed. It does not move when a card's damage goes from 6 to
7, or when a hook is added, since neither changes an id.

`assembly_hash` is shipped in the game's `release_info.json` and identifies the
release, so it moves on any code change at all.

A dump anchored on the content hash alone is accepted by a build that has
quietly rebalanced every number in it. Both are checked.

### Bitsets

A bitset over a generated enum, in the two shapes the domains need.

Every domain grows one sooner or later. A card's keywords, a monster's intents,
the families an encounter fields, the card types an enchantment can be put on.
They arrived independently and came out the same, which is the point. The shape
is forced rather than chosen.

#### Two shapes, and the difference is real

`plain` is one word with a `contains`. It is right when the member either is or
is not in the set, which is most of them.

`computed` is two words and answers with a `Field<bool>`. It is right when some
of the underlying members are `virtual` and a few models override them. A member
then has three answers and not two.

#### Why the second shape needs two words

A single bitset has to spell "computed" as either true or false, and there is no
honest choice between them. So the set carries the answers and the computed-ness
separately, and the generated `get` hands back a `Field<bool>`. There is then no
way to ask the question that cannot be answered. There is deliberately no
`contains`, because it is the lie this exists to avoid.

#### What a domain still owns

The member names and their docs, and the prose on the types. Those say what the
members *are*, which is the part that differs. Everything here says how a set of
them is spelled, which is the part that does not.

#### Every plain set implements `FlagSet`

`plain` emits `impl sts2_core::FlagSet` beside the inherent methods, which is
how a caller holding only a `ContentVersion` builds one and asks it.
Unconditional rather than an `Extra`. The trait asks for `from_members` and
`contains`, which every plain set has anyway, so making it optional is a switch
with nothing behind it.

A set is over the whole enum, not over the members one caller happens to use. An
enchantment can only be put on three of the six card types, and `CardTypeSet`
still spans all six. Narrowing the type to its first caller makes the second one
wrong.

### A module that is a directory

A generated module that is a directory rather than a file.

All three domains outgrew one file, and each outgrew it the same way. `mod.rs`
keeps the shape: the field enums, the data struct, and the outer enum. The rows
go into buckets beside it. What a bucket *is* differs, and that is the
interesting part, so it stays in the domain. A card pool decides what a
character can draft. An act decides which encounters can roll a monster. A
power's kind is the one thing the game classifies a power by.

What is identical is everything after that decision. Banner the file, import
exactly what it names, write it, and declare it in the parent. Three times, once
each, until this.

#### The imports are the part worth sharing

A bucket file that imports a name it never uses is a warning, and `just lint`
runs `-D warnings`. So the import list is computed per file from what the body
actually says. That job is small enough to copy rather than
share, and three files did copy it. `Split::finish` is the third copy of it, and the first
that only exists once.

It matches whole identifiers rather than substrings. In this build the two rules
happen to agree. `RepeatCount` contains the text `Repeat` and
`PowerInstanceType` contains `PowerType`, but no bucket names either longer one
without naming the shorter one too. So the difference is invisible in the
generated tree, and stays invisible until it is not. The tokenising rule is
right for a reason rather than by luck, and `uses` is tested directly because
the output cannot currently tell them apart.

## The domains

### Acts

#### `index` is not a position

`OVERGROWTH` and `UNDERDOCKS` are both index 0. They are the two alternative
first acts, and `is_default` is what tells them apart. A run's shape is 0 then 1
then 2 with a branch at the start, not a straight sequence. Anything treating
the index as a unique key is wrong about the first act.

It also carries a sign: the deprecated act is -1.

#### What is not here

`AllEvents`, `AllAncients` and `GenerateAllEncounters` are the same relations
the event and encounter dumps already write from the other side. Printing them
here as well makes two sources for one fact. The tests cross-check the two
rather than the table trusting itself.

`GetMapPointTypes` takes an `Rng` and rolls the map's composition, so it is the
map generator and gets ported by hand.

### Afflictions

#### `afflicts` is the whole type rule

The dump asks `CanAfflictCardType` about every card type, and unlike
`EnchantmentModel.CanEnchant` the base `CanAfflict` refuses no type outright. So
the probe is the answer rather than half of it: nine of the ten accept every
card type, and `TAINTED` accepts only skills.

What the base `CanAfflict` does add is two conditions on the card rather than
its type, and both are flags here. An unplayable card is refused unless
`AfflictsUnplayableCards`. A card that already carries an affliction is refused
unless `Stackable` and it is the same affliction.

The cards domain does not generate `CardType::None`, so the nine that accept
it in the dump cannot say so here. It is the one thing dropped between the dump
and this table.

#### `RefusesSomeCards` is not a column

No affliction overrides `CanAfflict`. Five enchantments do, which is why the
flag is what an offer needs there. Here there is no rule beyond the card's type
anywhere in the build. The dump measures it anyway, and generation fails if one
ever appears.

#### One file

Ten rows and no grouping to split on: afflictions have no pool, rarity or act.

### The ascension ladder

#### Why this is its own domain

It was generated by the monsters domain, out of enum rows in that dump. The same
reason `Act` was: the monster tables needed it first. It is not a monster fact.
Ascension is run-wide, and it gates monster HP, encounter gold and a pile of
run-level rules. So it now has a dump, a module and a test of its own, and
monsters imports it.

It is also version-dependent, which is the reason it is generated at all rather
than written down once. Builds move what a level means: a number shifts, a level
is inserted, two swap places. A hard-coded ladder goes on describing an older
game while every scaled value around it is generated against a newer one.

#### What is not here

What a level *does*. Poverty cutting gold, Ascender's Bane adding a curse,
Inflation raising shop prices. Those are hooks and gated values, ported by hand.
Where a level moves a number, the monster and encounter dumps already measure it
at every level and emit a `scale*` row. This is the ladder those rows index
into.

### Cards

Card identity nests by the game's own twelve card pools, so `Strike` needs no
suffix. `IroncladCard::Strike` and `SilentCard::Strike` are different variants
of different types. The game disambiguates with a flat namespace instead, as
`STRIKE_IRONCLAD`. That id is preserved as the serde rename, so the wire form is
unchanged and the generated file stays diffable against the game's own names.

Only ten of 578 ids actually carry a pool suffix, all of them `STRIKE_*` and
`DEFEND_*`. The stripping rule matters for exactly those, and is a no-op
everywhere else.

One file per pool rather than one for everything. A content patch usually
touches one character. That reads as a diff to one file, rather than a churn in
the middle of eight thousand lines.

### Card plays

What a card does when it is played, upgraded, or left in hand at the end of a
turn. The game writes that as code, so no dump mode can print it. `cardplaydump`
decompiles the pinned `sts2.dll` with `ilspycmd` and reads the C# instead.

#### A whitelist, and nothing partial

Each statement of `OnPlay` either matches one command exactly, or it stops the
card. One statement outside the list leaves the whole card to a hand port, and
the generated arm is `unimplemented!()` under a comment naming that statement.
The list is short on purpose: attack, block, apply a power, draw, energy, stars
and forge. Each one maps to the `Fight` method a hand port already calls.

A statement that only animates, plays a sound or waits is left out, because it
changes nothing a fight holds. It qualifies by what it calls: Godot's node
classes, `SfxCmd`, `VfxCmd`, `Cmd.Wait` and the character animation pickers.
It never qualifies by where it stands.

#### Where the game's own list matters

`base.CombatState.HittableEnemies` taken into a local is a snapshot, and the
generated play takes one where the C# does. Crush Under hits every enemy and
then weakens the list it took first. An Infested enemy it kills spawns
Wrigglers, and those were never on that list.

#### What the card overrides beside `OnPlay`

`IsPlayable` and `GetResultPileTypeForCardPlay` change what a play is, and the
simulator reads neither off a card. A card overriding either keeps no generated
play. `HasTurnEndInHandEffect` marks a card whose turn end is ported by hand.

#### The hashes

The dump's header names the version, commit and assembly hash from
`release_info.json`. It has no content hash, which only a running game works
out. Codegen checks the other three against `cards.txt`.

### Characters

#### Which pools are typed

`relic_pool` and `potion_pool` are the relics and potions domains' own types, so
the compiler checks them. `card_pool` is an enum declared here.

Cards have no pool type to point at. That domain *buckets* by pool but never
names one, since a card's pool is which enum it is a variant of. Such a type is
no use here anyway. Two characters draw from `MOCK_CARD_POOL`, which is not one
of the twelve the cards domain generates. So `CardPool` stays local, and is the
only one that does.

`potion_pool` used to be declared here for the same reason, and is not any more.
The potions domain has a `PotionPool`, and the five a character names are a
subset of its ten, so this points at it.

#### No file split

Eight rows. To split them is to file for its own sake.

### Enchantments

#### `enchants` is the effective set, not the probe

The dump asks `CanEnchantCardType` about every card type, and that method
answers for more types than an enchantment can actually reach. Seventeen of the
twenty-four say yes to `Curse`, `Status`, `Quest` and even the game's
`CardType.None`. The base implementation returns true for anything, and the real
filter is elsewhere.

`EnchantmentModel.CanEnchant` refuses card types 4, 5 and 6, which are Status,
Curse and Quest, for every enchantment before it ever asks `CanEnchantCardType`.
So the set generated here is the probe intersected with the three types that
survive that, which is what an enchantment can be put on. The raw answers stay
in the dump, so the intersection is visible rather than assumed.

The cards domain does not generate `CardType.None` at all, so there is no way
to spell it here even if it meant something.

`CardTypeSet` itself is the cards domain's, and spans all six card types. It
keeps all six rather than the three an enchantment can reach. A set over `CardType`
belongs beside `CardType`, and cutting it down to its first caller is how the
second one gets it wrong.

#### `IsStackable` is not a column

No enchantment overrides it, so all twenty-four take the `false` default. A
column is the same value twenty-four times, which says nothing about any of
them. The same reason `StartingPotions` is not a character column.

#### One file

Twenty-four rows and no grouping to split on: enchantments have no pool, no
rarity and no act.

### Encounters

#### `Roster` has three answers, not two

`EncounterModel.GenerateMonsters` is a method, and 81 of the 96 return the same
fight every time. Fourteen roll one from an `Rng` seeded per floor. Those cannot
be a table. They are `Roster::Random`, and get ported by hand the way monster
move bodies do.

The third is `Roster::Configured`, which is one encounter. The Battleworn Dummy
event refuses to generate until the event tells it which of three friends turned
up. Its roster is neither a list nor a roll, and whatever starts the fight
decides. Filing that as `Fixed` records whichever branch happened to be
reachable, which is a lie with a plausible shape.

#### Slots are two different lists

`slots` is what `EncounterModel.GetNextSlot` searches, which is the first slot
no living enemy stands in. So it is where a creature added *mid-combat* goes. A
spawn's slot is where a monster starts. They are not the same list, and codegen
does not try to make them one. `KNIGHTS_ELITE` declares no `Slots` at all and
starts three knights in `first`, `second` and `third`, and two more encounters
do the same. Resolving a spawn's slot into an index fails on exactly those
three.

#### Monsters are typed

`possible` and a spawn's monster generate as `Monster` variants rather than ids.
The compiler is then what checks an encounter names a monster the build has.
That is why this domain reads one dump and still cannot drift from another. A
monster removed from `monsters.txt` fails the build here.

### Events

#### Two things called "shared"

The game has both and they are unrelated, which is worth saying because the
names do not.

`ModelDb.AllSharedEvents` is a pool of nineteen that any act can roll,
concatenated onto every act's own list. That is where an event comes from, and
here it is the `anyact` bucket.

`EventModel.IsShared` is the multiplayer sense. `EventSynchronizer` uses it to
decide whether every player goes through the event together. The layout only
shows its label once there is more than one player. Eight events set it, and
they are *not* the nineteen. `BATTLEWORN_DUMMY` sets it and sits in one act, and
`FAKE_MERCHANT` is in the pool and sets it too. So it generates as
`shared_between_players`, which is what it means.

#### `deterministic` is not `!shared_between_players`

`IsDeterministic` defaults to `!IsShared`, and reading that as a definition is
wrong. `CRYSTAL_SPHERE` overrides it and is neither shared nor deterministic.
`EventRoom` branches on determinism alone, so the two are dumped as two columns
and the table keeps them apart.

### Modifiers

#### Which hooks are not here

Deliberately. A simulator's ported Midas already knows it hooks rewards, so a
list of overridden method names describes the game's API surface rather than the
game. A caller can do nothing with such a list that the ported body does not
already do.

What is generated is what a caller needs *before* any ported code runs. Whether
a run can pick this, what picking it rules out, and the two things that happen
at setup.

#### `kind` is the two lists, and there is a third state

`ModelDb.GoodModifiers` and `BadModifiers` are what a run offers from, and
between them they are the whole classification. A modifier in neither is one no
run can pick. That is `None` here, and it is the deprecated one: the same shape
the deprecated act takes, and for the same reason.

#### Exclusion is a set, not a pair

`MutuallyExclusiveModifiers` is a list of sets and there is one set in it,
holding three. A pair is the easy read, and it is wrong the moment a fourth
arrives. So `EXCLUSIVE` is a list of `ModifierSet`, and `intersects` is the
question a caller asks: does my earlier choice conflict with this one.

### Monsters

#### State ids become indices

The game addresses states by string id and looks them up in a
`Dictionary<string, MonsterState>`. Here a state is its position in the
monster's own `states` slice, so a transition is a `u8` rather than a hash
lookup. The ids survive on moves only. `StateLog` records moves and the
observation reports them. Branch nodes have `ShouldAppearInLogs => false`, and
nothing outside the graph ever names them.

#### One file per act

`mod.rs` holds the shape and the `Monster` enum. Each monster's graph is a named
const in the file for the acts it can be met in. `data` is then one arm per
monster naming that const, which reads as an index rather than as eight thousand
lines of table.

The buckets partition, which "the act it is in" does not. `summoned` takes the
24 monsters no encounter can roll, and `shared` the two that turn up in more
than one act. Filing those under whichever act came first puts a monster
somewhere its own `acts()` never mentions.

#### The repeat columns collapse

`GetStateWeight` blocks a branch if the repeat type says so *or* the cooldown
does, and one of the two is always the weaker:

- a cooldown of `c >= 1` beside `CannotRepeat` or `CanRepeatXTimes(n)` swallows
  it. The in-a-row test only fires when the most recent log entry is this
  branch, and the cooldown already fires on that.
- a cooldown beside `UseOnlyOnce` is swallowed by it, because "in the last `c`"
  implies "in the log".
- a cooldown beside `CanRepeatForever` is the only rule there is.

So the three columns collapse into one `Repeat` without losing behaviour.
Generation drops `Cooldown(One)`. It is the same predicate as `InARow(One)`, and
one spelling of a rule is worth more than a spelling that follows the dump's
column.

Everything the columns say that the type cannot is an error rather than a value.
Generation drops a `maxTimes` on a member that does not read one, and says nothing.

#### Why the branch order matters

`RandomBranchState.GetNextState` sums the weights, draws exactly one
`rng.NextFloat(max)`, and subtracts weights walking the list in declaration
order. So the order branches are emitted in is not presentation. It decides
which move a given roll produces. Branch rows keep dump order for that reason,
while states are sorted by id, which is what the dump already did.

### Monster moves

What each monster's move does, read off the decompiled assembly the way card
plays are. A move is the method its `MoveState` names in
`GenerateMoveStateMachine`, and the rules are the card rules. One statement
outside the list leaves the move to a hand port, and presentation drops out
by what it calls.

#### An attack is the hit its intent advertises

The table already carries each intent's damage and hit count, and
`attack_opponents` reads them. So an attack matches only when it names the
same members its intent names: `Attack(PeckDamage).WithHitCount(PeckRepeat)`
beside `MultiAttackIntent(PeckDamage, PeckRepeat)`. Anything else about the
hit stops the move.

#### Numbers the class names

A move reads its other numbers off the class: a constant, a property holding
one, or `AscensionHelper.GetValueIfAscension`. The generated move asks the
fight's ascension for the last. A field the class can write is not a number
this reads, because a move can change it.

A negative amount stops a move, as it stops a card. For some powers it means
one endless instance, and for others a debt.

### Orbs

#### The list a roll indexes

There are two lists of orbs in this build and they are not the same list.
`ModelDb.Orbs` holds four and is used by nothing that plays.
`OrbModel.GetRandomOrb` draws from a private `_validOrbs` holding five, and that
is what `Chaos` and `TrashToTreasurePower` roll against. Generating from
`ModelDb.Orbs` quietly makes Chaos never channel a Glass orb.

`ROLLABLE` is the second list, in its own order, because `GetRandomOrb` is
`rng.NextItem(_validOrbs)`. The position of an orb in that array is which roll
produces it, so the order is the RNG contract and not a listing order.

Generation drops the first list, because a caller that reaches for it reaches
for the trap. Generation checks it instead: every orb `ModelDb.Orbs` names has
to be one a roll can produce. That is true here, and it is what makes the dead
list harmless. A build that broke it fails generation, rather than leaving two
plausible lists side by side.

#### The values are numbers, and nothing more

There is no taxonomy of *kinds* of orb value here, and there was. An earlier
version sorted them by whether the getter read a literal, a field the orb keeps,
or an expression over other values. That sorted the C# rather than the orb, and
Plasma is the case that shows it. Plasma's evoke is twice its passive and is
spelled `1m` and `2m`. Glass's evoke is twice its passive and is spelled
`PassiveVal * 2`. The same relationship, and the table gave them different
shapes for no reason a caller can act on. A simulator that can double a passive
can double either one, whether the number behind it is a literal or a counter it
keeps.

So a value is `Option<NonMaxU8>` plus [`hooked`](#hooked-is-the-focus-question),
and `None` carries no meaning beyond "the getter names no number to take".
Glass's evoke is the only one, and it is not a special orb. It is a getter with
no numeral in it. How any of these values combine, decay or accumulate is the
orb's body, which is ported by hand like every other body in this crate.

The dump still measures which values live in a field the orb keeps. That is a
real thing about the game, and a build that moved one changed behaviour. Generation
drops it, because the number is the same either way, and the tests pin
it from the dump instead.

#### `hooked` is the Focus question

A hooked value runs `ModifyOrbValue`, and two models implement that hook:
`FocusPower` adds its amount to every hooked value, and `InfusedCore` adds to
Lightning's. That makes the flag worth carrying. An unhooked value is one Focus
does not reach: Plasma's energy at either end, and Dark's evoke. Dark's evoke
feels Focus only through the passive it accumulates.

#### What is not here

The bodies: which creature Lightning hits, that Dark picks the weakest enemy,
that Glass hits everything. Presentation: colours, icons, sounds and text.

### Potions

#### Both `None`s throw

Generation drops `PotionUsage.None` altogether. No potion has it, and `NPotionPopup`
throws `"No potions should have 'None' usage."` when it sees one. The game
declares it out of bounds, and the build agrees.

`PotionRarity.None` is generated as an absent `rarity`, which is the move
`CardRarity.None` made and for the same reason. `PotionRarityExtensions
.ToLocString` throws `ArgumentOutOfRangeException` on it, so it is not a rarity
the game can name. Two models carry it, `DEPRECATED_POTION` and a mock, and
neither is content. `Option<PotionRarity>` says the true thing: they have no
rarity rather than a rarity called none.

#### Pools partition

Every potion is stocked by exactly one pool, which is not true next door: a
relic can be in two and a monster in none. So the buckets are the pools, with no
crossover file and no leftovers.

### Powers

#### Why some cells are `Computed`

`PowerModel` declares all of these `virtual` or `abstract`, so any of them can
be worked out from combat state, and a few are. `ShrinkPower` stacks as a
counter or as a single, on the sign of its amount. `MonologuePower` reads a
dynamic var. The twenty `ITemporaryPower` descendants decide buff-or-debuff from
a `protected IsPositive`. The dump marks those `dynamic` and they generate as
`Field::Computed`, which is the simulator's cue to ask rather than look up.

#### The flags

Five of the eight columns are yes-or-no. Five booleans in a row is a shape where
every call site has to remember an order nothing enforces. They are one
`PowerFlags` bitset instead, on the same pattern as `KeywordSet` and
`IntentSet`. One difference is worth knowing. Those reuse a game enum and its
discriminants, and this one has no game enum behind it. The members are five
separate `PowerModel` overrides, so the numbering is ours, chosen as the order
the dump writes them.

The set carries the computed-ness too, which is why it is two `u8`s rather than
one. A flag is true, false, or worked out at the time, and `get` returns that as
a `Field<bool>`. So there is no way to read a computed flag as a plain `false`,
which is what a single bitset offers.

#### The wire form

The id is the serde rename and nothing else spells it. There is no `id` accessor
beside it. Two spellings of one string is a way for them to disagree, and the
derive is the one the wire already depends on.

#### Why the files are split by `kind`

The game groups powers by nothing. They are one C# namespace and one source
directory. The class hierarchy is flat but for three abstract bases.
`ModelDb.AllPowers` is a reflection sweep over every `PowerModel` subclass
rather than a registry, and every asset path is derived from the id. Powers have
no pools the way cards do, and no acts the way monsters do. Any power can be
applied to anything that can carry it.

What the game does classify a power by is `Type`, and it is the one fact
everything branches on. `PowerCmd` checks it before Artifact eats a debuff,
`Rend` and `Misery` count debuffs, and the badge and hover tips sort by it. That
is the same kind of thing `pool` is for cards: a column of the dump that the
game itself switches on. So it is what the buckets are.

It partitions cleanly, and the third bucket is a family rather than a remainder.
The twenty powers whose `Type` is computed are exactly the `ITemporaryPower`
descendants: fourteen `TemporaryStrengthPower`, three `TemporaryDexterityPower`,
and three `TemporaryFocusPower`. Those read buff-or-debuff off the sign of their
own amount.

`Power` itself stays one flat enum. A power's kind is a `Field` and twenty of
them compute it, so a `BuffPower` type is a claim the table cannot make.
`GetTypeForAmount` flips the answer on a negative amount anyway. The split is
where the rows are written, not what they are.

### Relics

#### `Price` rather than a number

`RelicModel.MerchantCost` returns 999999999 for a relic that is not for sale.
That is a sentinel and not a price. Nothing does arithmetic on it, and a shop
comparing gold against it compares against a number that means "never". So it is
`Price::NotSold`, the same move `Cost::None` makes for a card's -1.

It has to be read per relic rather than derived from rarity. The base
implementation maps Ancient, Starter and Event to the sentinel, but nine relics
override `MerchantCost` outright. So of the 35 Event-rarity relics, 26 are
unsellable and 9 have a real price. Deriving gets those nine wrong.

The price is flat. Both properties were read at all eleven ascension levels and
nothing moved, so unlike an encounter's gold there is no `Scaled` here. That is
a measurement rather than an assumption, and the reason the dump sweeps at all.

#### `Undrafted` rather than `None`

`RelicRarity.None` is not the C# zero standing in for "unset". Two relics carry
it, and it means the rarity system does not reach them. `RelicGrabBag` gives it
no deque, as `rarity == None ? null : GetDeque(rarity)`. So no roll produces
one, and they arrive only when something hands them over. `CIRCLET` is the
fallback pool's only member, which is what you get when there is nothing left to
get, and `DEPRECATED_RELIC` is the deprecated pool's. The wire keeps the game's
spelling, and the Rust name says what it does.

### Rest site options

#### Why this is a domain at all

A rest site option is not a model: no id in the model database, no row in
`ModelDb`. So a snapshot's `SMITH` had nothing to become but a `String`. A
`String` cannot tell a misspelling from an option this build does not have.

### The run's vocabulary

#### Every member is kept

Cards drop the C# zero where it means "absent" and spell it `Option` instead.
Not here. These zeros are values the game sets and the observation reports.
`CombatManager.SetPhaseForAllPlayers(PlayerTurnPhase.None)` runs at three
points, `StandardActMap` builds a grid of `MapPointType.Unassigned` and then
assigns over it, and `GetOppositeSide` carries `CombatSide.None` through to
itself.

[`UnplayableReason`](#unplayablereason-is-numbered-by-bit) is the one exception,
and for a different reason.

#### `UnplayableReason` is numbered by bit

It is the build's one `[Flags]` enum. `CardModel.CanPlay` starts at `None` and
`|=` a bit per rule the card fails. A card carries several at once, so a single
choice is the wrong shape. The set is `UnplayableReasons`, and the empty set is
the game's `None`, which is why `None` is not a member.

The game numbers the rest 2, 4, 8, 16, 32, 64, which are bit *values*. The
generated enum is numbered 1..=6, the *index* of each bit, so that the set built
over it holds the game's own word. `1 << HasUnplayableKeyword` is 2, which is
what the game calls it. Bit 0 goes unused, because the game skips value 1.
