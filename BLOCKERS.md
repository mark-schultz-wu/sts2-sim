# Blockers

What stands between the simulator and a complete port, solo and co-op, in
the order to work them. `/blocker-grind` takes the first open item, one
PR at a time.

## Done means

Every recorded run matches the simulator step by step, to the run's end.
The game plays each one itself, picking at random from its own legal
moves. The recorded runs cover:

- each character, at every ascension level
- runs with each modifier the game's custom mode offers
- both builds, v0.107.1 and v0.111.0
- the long run 0RACLE1-REGENT-2.

Two more checks say when that is enough:

- `cargo xtask hunt --mixed` plays 3,000 runs in a row on each build with
  no split. Then fewer than one run in 1,000 splits, with 95% confidence.
- `cargo xtask reached` shows no rules code that play can reach and no
  run reaches. A scenario covers each gap, or the gap is noted as
  unreachable.
- The game's own rules code shows no method that play can reach and no
  hunt enters. This finds what the simulator never ported, which
  `reached` cannot see.

Speed is a goal as well as a guard. Item 4 says the target.

## How the list changes

- A PR that fixes an item deletes it, in the same commit.
- A PR that finds a new blocker adds it, where it belongs in the order,
  in the same commit. A blocker found while fixing another goes above it
  only when the fix cannot land without it.
- An item too big for one PR is split into items that each fit one.
- An item that needs Mark's decision says so, and the grind stops there
  and asks.

## Open

### 1. The co-op audit

PRs #736 to #753 fixed co-op bugs that the hunted seeds reached. The
audit of 2026-10-27 lists other code with the same shape, which no seed
reached yet. Work it in
this order:

1. Whom a hook is about, in the four parts of `docs/HOOK-SUBJECTS.md`.
   They close most of classes A, B and C. Part 1 is done.
2. The mod's items F1 to F4.
3. The turn's phases for each player: the block clear, the hooks before
   the turn's end, and the start of a side's turn.
4. Run state: Proceed after the boss, Spoils, `for_run` and the ancient
   heal.
5. The player-count rules in class E.
6. The rest of F, then G.

After PRs #736 to #753, the sixteen hunted co-op seeds match to the
run's end. This audit sorts the bugs those PRs fixed into classes. Then it
lists the code that has the same shape and that no seed reached yet.
Paths are under `crates/` unless they start with `mod/`. "Game" means the
v0.111.0 decompiled source.

#### The classes

| Class | Fixed in | Sites still open |
|---|---|---|
| A. A listener does not compare the event's player with its owner | #744, #746, #751, #752, #753 | about 150 |
| B. A listener's effect lands on the acting player, not on its owner | #746, #751, #752 | about 20 engine sites |
| C. A hook carries the host's or the actor's numbers | ShouldDie in #746 | about 10 |
| D. State the game keeps for each player is held once | #737, #738, #743, #745, #747, #748 | about 12 |
| E. The sim lacks a rule the game gives for a player count | #740, #742, #750 | about 35 |
| F. A mod stand-in acts for one player only | #736, #743, #745 | 12 |
| G. Co-op behaviour is a stub or an `unimplemented!` | #753 | about 8, plus 32 panics |

Classes A, B and C share one root. The sim's hooks name no player, and
`hook::apply` (`sts2-sim/src/combat/hook.rs:53-80`) sends `GainBlock`,
`Heal`, `ApplyToOwner`, `Draw` and the damage effects to
`fight.actor()`. Its comment says why: "a power only answers one that
its own owner is party to". That holds only in a fight with one
player.

#### A. Missing owner checks

In the game, nearly every override starts with `card.Owner !=
base.Owner`, `player != base.Owner` or something like it. The sim
lacks many of these checks, often with a comment that says "a fight
here has one player". `grep -rn 'here has one'
crates/sts2-content/src` lists about 140 of these comments.

##### The hook carries the player already, so only the content changes

- `CardPlayed`:
  - 20 relics. They are Art of War, Daughter of the Wind, Brilliant
    Scarf, Game Piece, Helical Dart, Iron Club and Ivory Tile. Also
    Kunai, Kusarigama, Letter Opener, Lost Wisp, Mummified Hand and
    Nunchaku. Also Ornamental Fan, Rainbow Ring, Razor Tooth,
    Permafrost, Pocketwatch, Ripple Basin and Velvet Choker.
  - Tuning Fork is the 21st relic, but `tuning_fork.rs` was not
    checked.
  - Powers: Devour Life, Master Planner, Rage, Haunt, Void Form,
    Panache, Tender, Withering Presence.
  - Cards: Pinpoint, Banshee's Cry.
  - Galvanic and Vital Spark act on `fight.player()`. The game acts on
    the card's owner.
- `CardPlaying`:
  - Relics: Intimidating Helmet, Music Box, Pen Nib.
  - Powers: Storm, Calamity, Serpent Form, Rupture, Afterimage,
    Monologue, Subroutine, The Sealed Throne, Spirit of Ash, Danse
    Macabre, Sloth, Chains of Binding.
  - The Stomp card.
  - Strangle and Oblivion compare with `fight.player()`. The game
    compares with the power's applier.
- `CardEnteredCombat`: Ghost Seed, Ringing, Hex, Tangled, Sword Sage,
  Phantom Blades.
- `CardGeneratedForCombat`: Regalite, in both builds, and Rocket
  Punch.
- `EnergyCost` and `EnergyCostLate`: Spiked Gauntlets, Brilliant
  Scarf, Corruption, Void Form, Borrowed Time, Curious, Tangled.
- `ShouldFlush`: Runic Pyramid, Ringing Triangle, Well-Laid Plans.
- `ShouldDraw`: Fiddle. `ShouldPlayerResetEnergy`: Ice Cream.
- `ShouldClearBlock`: Sturdy Clamp compares with `player`, which is
  the host's.
- `DamageMultiplicative`: Undying Sigil reads `fight.player()`.
- The Imbued enchantment (`AutoPrePlayPhase { player }`) needs the
  card's owner passed to `enchantments::event_hook`.

##### The hook carries no player yet

Each of these needs the field first.

- `CardDrawn` and `CardDrawnEarly`: Automation, Corrosive Wave,
  Confused, Speedster, Iteration, Pagestorm, Chains of Binding,
  Hellraiser.
- `CardExhausted`: Burning Sticks, Charon's Ashes, Forgotten Soul,
  Joss Paper, Dark Embrace, Feel No Pain.
- `CardDiscarded`: Tingsha, Tough Bandages.
- `CardPlayedLate`: Right Hand Hand, Make It So.
- `Shuffled`: Biiig Hug, The Abacus, Stratagem.
- `Flushed`: Bookmark. `FlushingLate`: Well-Laid Plans on v0.107.1.
- `ShouldPlay`: Velvet Choker, Sloth, Chains of Binding, Normality,
  Enthralled. One player's Normality blocks the other's cards.
- `CardSource` carries no owner. This affects:
  - Mystic Lighter, Miniature Cannon, Strike Dummy, Fake Strike Dummy,
    Pen Nib, Vitruvian Minion, Pael's Legion, Vambrace
  - Lethality, Gigantification, Dexterity, Unmovable.
- Dexterity is a special case. The game buffs block from the card's
  owner. The sim buffs the block's recipient instead, which is wrong
  for a card that gives block to a teammate.
- Single hooks:
  - `XValue`: Chemical X.
  - `StarCost`: Brilliant Scarf, Void Form.
  - `OrbPassiveTriggerCount`: Gold-Plated Cables.
  - `OrbChanneled`: Metronome, Thunder.
  - `EnergySpent`: Orbit.
  - `StarsGained`: Black Hole.
  - `Forged`: Hammer Time.
  - `AfterAttack`: Bone Flute, which needs the pet's owner.
  - `PotionUsed`: Reptile Trinket.
  - `PotionUsing`: Surrounded.
- `ShouldTakeExtraTurn` and `TakingExtraTurn`: Pael's Eye and
  Ambergris.
- Run hooks:
  - `GoldGained`: Dragon Fruit, Ectoplasm, Bowler Hat.
  - `AddedToDeck`: Bing Bong, Book of Five Rings, Darkstone Periapt,
    Lucky Fysh, Hoarder.
  - `RestSiteHealRewards`: Dream Catcher, Tiny Mailbox.
- Run walks that use `notify_run`, where `notify_run_for(seat)` is
  enough. The seat is in scope at each site:
  - `CardRewardCreationOptions`: Dingy Rug, Prismatic Gem.
  - `CardRewardAlternatives` and `CardRewardAlternativeChosen`:
    Pael's Wing.
- Decision hooks asked of every seat with no player:
  - Sozu (`ShouldProcurePotion`). One player's Sozu blocks every
    player's potions.
  - Miniature Tent.
  - White Beast Statue.

##### History entries record no player

`HistoryEntry` (`sts2-core/src/content.rs:4883`) records no player for
these entries: `Drawn`, `Attacked`, `BlockGained`, `EnergySpent`,
`StarsModified`, `Afflicted`, `Discarded`, `Exhausted`, `Generated`,
`Channelled` and `PowerReceived`. So every count of them adds up both
players. These read such a count:

- Iteration, Chains of Binding, Unmovable, Flatten
- Murder, Rattle, Radiate, Memento Mori, Death March, Evil Eye
- `live.rs:2821` `unprompted_plays_this_turn`, which feeds Pael's
  Eye.

#### B. Effects for the acting player

- `clear_block` (`turn.rs:3706`, from `turn.rs:358`):
  - only the host keeps their turn-1 block
  - another seat's Anchor block is cleared
  - `ask_relic(live, host, ..)` panics for another seat's relic.
- Walks of a hook raised for one player build their boards for that
  player, not for the listener's owner. Part 2's owner checks make the
  two the same. These are the hand draw, the turn's start for a player,
  the flush, the energy reset and the extra turn.
- Monster powers that use `fight.player()`:
  - Paper Cuts, Sleight of Flesh
  - Possess Strength and Possess Speed, where the game keeps a count
    for each player
  - Sandpit's removal, where the game kills `base.Target`.
- Run sites:
  - `room_entering` TransformTo (`run/state.rs:1887`) looks only in the
    traveller's deck
  - `room_entered` (`run/state.rs:5760`) parks a pickup on the host
  - `ancient_heal` (`run/travel.rs:115`) heals only the traveller, but
    the game heals each player
  - `Live::for_run` (`live.rs:2719`) has no seat, and the deciding seat
    drains it.

#### C. Hook values from one player

- At the turn's end, the hooks carry the host's numbers:
  - `BeforeSideTurnEndVeryEarly { block }`, at `turn.rs:1154`
  - `BeforeSideTurnEnd { energy, hand }`, at `turn.rs:1240`
  - `SideTurnEnd { block }`, at `turn.rs:4118`.

  This affects Orichalcum, Fake Orichalcum, Pael's Tears, Screaming
  Flagon and Parrying Shield.
- `BeforeSideTurnEndEarly { plays }` counts every player's plays.
- `RoomEntered` in a fight (`start.rs:216`) carries the host's deck
  and health. The run's walk refreshes them for each owner, but this
  walk does not.
- `holding_potions` on `CombatStarting` (`start.rs:422`) is the
  host's. On `PotionUsed` (`play.rs:1903`, `board.rs:2511`) and
  `PotionDiscarded` (`run/state.rs:2157`, `run/state.rs:2891`) it is
  the actor's. `PotionProcured` carries no player. All of this affects
  Belt Buckle.

#### D. State held once

- `proceed(seat)` (`run/state.rs:2604`): one player's Proceed after the
  boss moves the whole run on. The game's `ActChangeSynchronizer`
  waits for every player.
- `spoils` (`run/state.rs:44`) is cleared after the first player's
  chest, so a second Spoils Map never pays.
- `extra_rewards` (`live.rs:2705`) is one list. The game keys it by
  player. This is known, and it panics.
- `Live` has single slots: `after_pick`, `drawn_late`, `drew`,
  `hook_draws` and `asking`. Two players can each park one at once.
- Fight-in-event: `resuming` and `run/fight.rs:313-410` are built for
  one player, behind the panics.
- Out of a fight, damage kills the player. The game heals them to 1
  instead when there are several players (`CreatureCmd.cs:522`).
- Possess counts the stolen stacks once, as listed under B.

#### E. Player-count rules

- Events:
  - About 23 events allow themselves only if `Players.All(..)` holds,
    and Ranwid the Elder checks `Players.Any`. `run/travel.rs:179`
    asks one seat.
  - Jungle Maze Adventure and Dense Vegetation are refused in co-op
    when any player's health is low.
  - The game never allows Fake Merchant in co-op.
  - War Historian Repy ends its event in co-op.
- `GoldenPathActMap.cs:42` drops a point type in co-op. Golden
  Compass ignores this.
- Monster numbers:
  - Knowledge Demon's heal is `30 * Players.Count`
  - Waterfall Giant's Siphon heal scales the same way
  - Tough Egg's hatchling health and Test Subject's respawn health are
    scaled by `ScaleHpForMultiplayer`.
- In Act 3, a fight of several players panics at `monster.rs:139`.
- `BurningBlood` and `BlackBlood` heal a dead teammate. The game checks
  `!Owner.Creature.IsDead`.
- Modifiers: Terminal, Cursed Run and Night Terrors act for one seat,
  where the game acts for each player. Every modifier also panics in a
  run of several players, at `run/state.rs:1471` and
  `run/state.rs:4887`.

#### F. The mod

High confidence:

1. `Patches.cs:362` `BundleWithoutAScreen`. With TestMode on, the
   game takes `bundles[0]` for the other player before it reaches the
   branch that waits for their choice. The seat never reads the other
   player's pick.
2. `Protocol.cs:630` sends a proceed event option, Trial's Double
   Down, through `EventSynchronizer`. The screen runs it locally. The
   other seat then shows a popup the game does not show there, and in a shared
   event the option becomes a vote.
3. `Moves.cs:227` offers a chest skip in co-op. `NTreasureRoom` offers
   Skip only when `Players.Count == 1`.
4. `MapLegality.cs:115` lets the player travel once their own chest
   vote is in. The screen waits until every player's pick is in and
   the relics are awarded.

Medium:

5. `Protocol.cs:628` tracks a shared event's options as before and
   after slices of the pending tasks, which is the Drowning Beacon
   shape.
6. `Protocol.cs:845`: `_sent` does not count the move to the next act,
   and the mod still offers Proceed after it.
7. `Peers.cs:68` decrements `_sent` for any action the local player
   owns, not only the ones it sent.
8. There is no move to take back the end of the turn
   (`UndoEndPlayerTurnAction`).
9. A dead local player has no moves outside a fight, which can
   deadlock the map.

Low:

10. `Pending.Park` allows one open question at a time.
11. `Patches.cs:1162` marks the card removal used in every player's
    shop.
12. `Peers.Report` leaves out the event votes, the chest votes and
    whether a player is ready for the next act.

#### G. Stubs and panics

- Powers:
  - Sneaky: block when another player plays an Attack
  - Beacon of Hope: shares block with teammates
  - Tank: gives Guarded to the other players
  - Covered: `unimplemented!`
  - Hammer Time: forges for every other player.
- 32 `unimplemented!` panics in `sts2-sim` name several players. Most
  of them are in `turn_loop.rs`, `board.rs` and `run/state.rs`, and
  `BLOCKERS.md` already lists them.

### 2. Co-op

Co-op is in scope, before the AI front end. The headless game plays co-op on
v0.111.0, one game for each seat, and `cargo xtask coop` plays runs at random.
Each seat implements `sts2_core::Seat`, and `sts2_core::Coop` decides which
seat acts. A run's `Start` names every player, and both run formats hold runs
of any number of players, so the runs that end are kept.

Next is a simulator that holds a player per seat. Each seat keeps the whole
run as its own machine sees it. The work goes in this order:

1. With one player and no change in behaviour, the simulator learns to hold
   several. The run state keeps its players and their belts by seat. A
   fight keeps each player's piles, energy and orbs, borrows every belt,
   and acts for a given player (all done). Next, card, relic and potion
   behaviour learns its owner, where it now reads "the player". The co-op
   runs check it, so it moves into step 3, where they reach it.
2. A simulator seat, and `cargo xtask coop --hunt` playing it beside each
   game seat, step by step, as a hunt does (done).
3. The co-op rules, in the order the runs reach them. A run of several
   players starts, each player meets Neow, and the map takes votes (done).
   The card pools deal the co-op cards in a run of several players (done).
   Every hunted seed that reaches a fight matches through its first fight
   and rewards. The fight holds
   every player, and the turn ends once every player ends it. A monster's
   move reaches every player, with its health and powered block scaled by
   the player count. Each relic reads its own owner. A card play that
   stops on a question waits at the front of its player's queue, and the
   other players act around it. A won fight hands each player back their
   health and rolls their own rewards. A player who dies beside a living
   one leaves the fight's cards, and a win revives them. A win cancels
   the actions still waiting, as the game's does. A merchant stocks an
   inventory for each player, who buys from it alone. Each player goes
   through their own copy of an event. A potion aimed at a player acts
   for that player, who answers what it asks, and its drinker applies its
   powers. A fresh power on
   an enemy scales by the player count, and an enemy's Plating loses a
   stack for each player. Each player has their own rest site, which
   offers Mend in a run of several players. A power that makes a card
   free, and a relic that rewrites a card joining a deck, read the card's
   owner. A treasure chest holds a relic for each player, and awards them
   by the players' votes. A monster's attack, and a player's death in it,
   reach each player's own listeners. The shared relic bag passes over
   what left any player's bag. A dead player's powers hear no hook, as
   their relics, potions and cards do not. A power's turn-end asks act
   for the player carrying it, and so do a side's turn-start asks. A
   choice put to a player who ended the turn takes that end back. All
   sixteen hunted seeds match to the run's end. Hunts stay at
   `--width 4`: at `--width 8` two games of sixteen failed in Godot
   itself. The golden tests follow the hunted runs in
   `coop-v0.111.0.bin` without the game. So a change to the simulator
   needs no hunt to show it keeps them. Next, in the order of `docs/COOP-FIGHTS.md` and where the
   hunted runs stop:
   - The rest of the scheduler (steps 3 and 4 of `docs/SCHEDULER.md`). A
     question the turn loop asks belongs to no player's queue yet. Each
     phase of the turn's end panics at one in a fight of several players.
     So does the turn's end while a player's action waits on a question.
     Card plays, potions and the turn's end go through the queues, with
     the action types, gating and cancellations.
   - A question that waits on a player's own card play names no asker,
     where the game names the play.
   - A shared event, which the players vote through, and an event that
     opens on a fight panic in a run of several players.
   - A fight won inside an event, and the extra rewards a monster gives,
     which panic in a run of several players.
   - Content that reads "the player" where the game reads an owner:
     - `FightView::player`, which about a dozen power rules read, panics
       in a fight of several players.
     - `RoomEntered` carries the first player's deck size and health,
       which a relic and the Terminal modifier read.
     - Every relic that hears `CardPlayed` compares the card's owner
       with its own in the game, 27 in all. Shuriken does in the
       simulator, and the other 25 do not yet.
     - Content whose comment says "a fight here has one" player.
     - The rest of `Board::seat`'s callers, which a card or potion reaches
       for its own player and a monster's move does not yet.
   - A player who dies inside their own card or potion beside a living
     one, and a death that leaves every standing player ready, panic.
4. Co-op content, as the runs reach it. Massive Scroll, Concoct and Tag
   Team are ported.

### 3. The engine's command frames as coroutines

Every rule in content that pauses is a `diapause` coroutine, and so are
the turn loop and the run layer's pickups (`docs/SCHEDULER.md`,
`docs/RUN-PAUSES.md`). The engine still holds the frames of a command
inside one action by hand. That is a draw, an auto-play, `PowerCmd.Apply`
over several targets, and an attack's hits. They are `Deferred`'s
command variants and `Then::Play` and `Then::Pick`, with `Live`'s fields
that only hold them. Each carrier moves on its own, which is step 5 of
`docs/SCHEDULER.md`. It changes no order. Mark chose on 2026-10-27 to
leave it until co-op fights are done, since they do not need it.

### 4. Speed

The target is spire-mentor's W1 workload at its cost or less. W1 plays
random legal moves for the Ironclad, with the starter deck, against the
22 Act 1 Overgrowth encounters in turn. Its engine costs about 7,400
instructions per action on this machine. `cargo run --release -p sts2-sim
--example fight_bench` plays the same workload here, and it costs 9,872.
Count it under callgrind at 3 reps and at 1, and divide the difference by
the actions between them.

The easy savings are taken. What is left is spread over the hook walks and
the turn's bookkeeping. The design work for it is a flat fight state that
copies cheaply, and a cheaper walk. Both hold a player per seat, so they
come after co-op. `cargo xtask instructions` still guards each PR, per replayed
step.

### 5. The splits a hunt finds

`cargo xtask hunt` plays fresh seeds on the game and the simulator side
by side, and keeps only the runs where they part. `--mixed` spreads the
seeds over every ascension level and modifier. `cargo xtask split --file
target/hunt/parted-<build>.bin <index> <name>` writes a kept run, up to
its split, as a scenario. A fix lands with its run, recorded whole, in
`crates/sts2-golden/tests/hunted.bin`. A v0.111.0 run goes in
`hunted-v0.111.0.bin`. A split that many runs share becomes its own item, above this
one.

`cargo xtask game-trace <index> --file <parted file> <pattern>...`
prints the game's own calls in the moves before a kept run's split.
`cargo xtask pause-sites --why <method>` prints a chain of calls from a
game method to a pause.

A mixed hunt of 7,000 runs on v0.111.0 parted 258, and all 258 are
fixed. A second round of 3,000 mixed runs on each build started from
seed 10000. It parted 8 runs on v0.107.1 and 9 on v0.111.0, and one
v0.107.1 run stopped on a game error. All of these are fixed.

A third round, from seed 20000, parted 1 run on v0.107.1 and 3 on
v0.111.0. All 4 are fixed. A fourth round, from seed 30000, parted 3 on
v0.107.1 and 2 on v0.111.0, and all 5 are fixed.

Next, hunt both builds again toward 3,000 runs in a row with no split.

### 6. Coverage of the game's own code

`cargo xtask hunt --coverage` has the mod count which methods of the
game's rules code each run enters: models, commands, hooks and combat,
not screens. `cargo xtask game-coverage` adds the counts up by area and
lists the methods no run entered. Twenty runs entered 7,023 of 15,756
methods. Counting makes a hunt about four times slower.

Each gap gets a scenario, a hunt that starts from a built position, or a
note that play cannot reach it.
