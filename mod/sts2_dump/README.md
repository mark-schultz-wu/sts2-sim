# sts2_dump

Prints the game's own tables so the simulator's content can be generated from
them, rather than from someone reading the decompiled source and typing numbers.

Its own mod, not a mode of `sts2_determinism_test`: these dumps assert nothing
and test nothing. They read the model database and print it.

## Running

Each dump is a `STS2_HEADLESS_MODE`, and output goes to the log between
`[<mode>] begin` and `[<mode>] end`:

```
./mod/deploy.sh
cd "$STS2_GAME_DIR" && LD_PRELOAD=libgcc_s.so.1 STS2_HEADLESS_MODE=carddump \
    ./SlayTheSpire2 --headless --force-steam=off
```

Every mode but `rngdump` lands in `crates/sts2-content/src/v0_107_1/dumps/`,
under the name in the last column.

| Mode | What it prints | Lands in |
| --- | --- | --- |
| `actdump` | each act's length, and its bosses in discovery order | `acts.txt` |
| `afflictiondump` | what an affliction can be stuck to, and its flags | `afflictions.txt` |
| `carddump` | every card's static data | `cards.txt` |
| `characterdump` | starting deck, relics, HP and pools | `characters.txt` |
| `enchantmentdump` | what an enchantment can go on, and its flags | `enchantments.txt` |
| `encounterdump` | what starts a fight: room, roster, slots, acts | `encounters.txt` |
| `eventdump` | every event, and the acts that offer it | `events.txt` |
| `modifierdump` | run-wide rules: good or bad, what they rule out | `modifiers.txt` |
| `monsterdump` | HP ranges and move state machines, at every ascension | `monsters.txt` |
| `orbdump` | each orb's two values, its trigger, and the roll order | `orbs.txt` |
| `potiondump` | every potion's rarity, usage and target | `potions.txt` |
| `powerdump` | every power's static data | `powers.txt` |
| `relicdump` | every relic's rarity, price and pool | `relics.txt` |
| `rngdump` | 1024 draws per seed, every `Rng` method | `crates/sts2-rng/tests/megarandom.txt` |

Then `cargo xtask codegen`.

## The one rule

Every dump names the build it came from, and the Rust side refuses one that
does not match the version module it generates into:

```
# act data
# build v0.107.1 commit 59260271 assembly 1692500715 content 3954186980
```

Two hashes, because one is not enough. `content` is
`ModelIdSerializationCache.Hash` — XxHash32 over the *set of model ids*. It
moves when content is added, removed or renamed, which is most patches, since
STS2 churns 20-50 cards a minor version. It does **not** move when a card's
damage goes from 6 to 7, or when a hook is added, because neither changes an id.
A dump anchored on it alone would be accepted by a build that had quietly
rebalanced every number in it.

`assembly` closes that. `MainAssemblyHash` is shipped in the game's
`release_info.json` and identifies the release, so it moves on any code change
at all, and it survives this project's IL-patched `sts2.dll` because it
describes the release rather than the file. `sts2_determinism` already refuses
to start against an unpinned one; `sts2_headless` already reports it in the
greeting. The dumps were the last place trusting the content hash by itself.

See `Stamp`, which all thirteen share.

## Ascension

`AscensionManager.HasLevel` is `_level >= (int)level`, so ascension is a scalar
and each level is a threshold: anything behind
`AscensionHelper.GetValueIfAscension` is a step function of the run's level.

`monsterdump` and `encounterdump` therefore read every value at all eleven
levels and emit a `scale*` row wherever one actually moves, rather than dumping
one ascension and calling it the table — see `Ascension`, which both share.
Which levels matter is measured, not read off the source, which is how Gremlin
Merc turned up gating its damage on `ToughEnemies` where every other monster
gates on `DeadlyEnemies`, and how `FAKE_MERCHANT_EVENT_ENCOUNTER` turned up
paying a flat 300 gold where every other encounter is cut by Poverty.

## What is not dumped

Behaviour. A card's effect, a relic's hook, a monster's move body, and the
lambdas a monster's branch weights are written in are all code, and code gets
ported by hand.

Where a value *might* be code, the dump says `dynamic` rather than guessing —
`CardDump` marks a card that overrides `Type` or `TargetType`, `MonsterDump`
marks a branch weight whose delegate could read mutable state, and `PowerDump`
marks any member whose IL is not literally "push a number, return". All three
are deliberately conservative: marking a constant as dynamic costs one method,
marking a dynamic as constant costs a divergence nobody can see.

`PowerDump` needs the IL because `CardDump`'s rule does not carry over.
"Overrides the base" works for cards, whose `CardModel.Type` has an
implementation to inherit; `PowerModel.Type` and `StackType` are `abstract`, so
every power overrides them and the rule would mark all 273 dynamic.

`EncounterDump` cannot read either — `GenerateMonsters` is `abstract` too, and
its body calls out — so it asks the randomness instead. The method is run
against a fresh `Rng` and the `Counter` is read afterwards: an encounter that
drew nothing cannot have rolled anything, whatever its body looks like. That is
exact rather than conservative, and it is only possible because the dump runs
inside the real game.

`OrbDump` needs its own decoder rather than `Il`'s. `PassiveVal` and `EvokeVal`
are `decimal` and mostly wrapped in `ModifyOrbValue`, which asks a hook, which
reads an `Owner` a canonical model does not have — so reading the property
throws, and its IL is not "push a number, return" either. What it is instead is
four shapes wide, including `ldsfld Decimal::One`, which is how the C# compiler
spells `1m`. Anything else is `dynamic`, on the same terms as everywhere else.

It also reports two things a number alone cannot say. `hooked` is whether the
getter runs `ModifyOrbValue`, which is whether Focus reaches it, since
`FocusPower` and `InfusedCore` are what implement that hook. `stored` is whether
the number lives in a field the orb keeps, in which case what is dumped is the
field's initialiser — Dark's evoke and Glass's passive are the two, and a
simulator needs a slot for each.

`stored` is deliberately not "does the value change", and the Rust side does not
generate it — a value's number is the same whether a literal or a field holds it,
and sorting the values by that sorts the game's source rather than the game. It
is dumped because a build moving a value between the two would change behaviour
while the number stayed put, and the content tests pin it from here.

`EncounterDump` also leaves out presentation: camera scaling and offset, scene
paths, custom backgrounds, BGM and ambient SFX, and boss map-node art. A
simulator that never draws a frame has no use for any of it.

## Two lists that are not the same list

`ModelDb` is not always the live table. `ModelDb.Orbs` names four orbs and
nothing that plays reads it; `OrbModel.GetRandomOrb` draws from a private
`_validOrbs` of five, and that is what `Chaos` rolls against. `OrbDump` prints
both memberships so the difference is a row rather than a decision someone made
once — the same reason the dumps reflect over `ModelDb.AllAbstractModelSubtypes`
instead of walking the usage tables, which miss summon-only monsters and
event-only encounters.
