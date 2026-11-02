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
   They close most of classes A, B and C. All four parts are done, and
   what they leave is below.
2. The mod's items F1 to F4 (done).
3. The turn's phases for each player (done). The audit of 2026-10-28
   found one more, which waits for the scheduler under item 2.
4. Run state (done).
5. The player-count rules in class E.
6. The rest of F, then G.

After PRs #736 to #753, the sixteen hunted co-op seeds matched to the
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

Every relic, power and card that the audit found compares the player
now. Frantic Escape did not: it bumped the first Sandpit, where the game
bumps the one aimed at the card's owner (seed 1). So the audit of
2026-10-28 read the 49 comments left against the game. Ambergris and
Painful Stabs acted for the wrong player, and are fixed. These are still
wrong with several players:

- After the hand size is worked out, every player's Pollinous Core
  hears `AfterModifyingHandDraw`. The game asks only the listeners that
  changed that player's hand size. Two cores at four turns both reset on
  the first player's draw. No run reaches it yet, so check it first.
- v0.107.1 only: Diamond Diadem and Juggling count every player's
  plays. Co-op runs on v0.111.0, so this matters only if co-op ever
  runs on v0.107.1.

The other comments describe code that is already right, and are stale.
They go as the code near them changes.

#### B. Effects for the acting player

- Walks of a hook raised for one player build their boards for that
  player, not for the listener's owner. Part 2's owner checks make the
  two the same. These are the hand draw, the turn's start for a player,
  the flush, the energy reset and the extra turn.
- Run sites: `room_entered` (`run/state.rs:5811`) parks a pickup on
  the host. A run question does not say which player asked it, so the
  pickup cannot name its asker yet.

#### C. Hook values from one player

The fight's walk and the run's fill in each listener's own player's
block, energy, hand, deck, health, plays and belt. What is left:

- A modifier reads `RoomEntered`'s numbers as the walk raised them,
  which are the first player's. Terminal reads them for every player.

#### D. State held once

- `Live` has single slots: `after_pick`, `drawn_late`, `drew`,
  `hook_draws` and `asking`. Two players can each park one at once.
  The scheduler's step 6, a question for each seat, removes `asking`.
  A question several players answer at once waits as a task on each
  player's queue. A setup's, a pre-play phase's and the Knowledge
  Demon's do.
- A treasure chest awards every player a relic at once. Some relics
  ask as they are obtained, such as Astrolabe or Pandora's Box. Such a
  relic panics in `obtain_here`. The treasure phase keeps no question for
  each player. About 25 relics ask in `AfterObtained`.
- Out of a fight, damage kills the player. The game heals them to 1
  instead when there are several players (`CreatureCmd.cs:522`).

#### E. Player-count rules

- Events:
  - About 23 events allow themselves only if `Players.All(..)` holds,
    and Ranwid the Elder checks `Players.Any`. `run/travel.rs:179`
    asks one seat.
  - Jungle Maze Adventure and Dense Vegetation are refused in co-op
    when any player's health is low.
  - The game never allows Fake Merchant in co-op.
  - War Historian Repy ends its event in co-op.
- Monster numbers:
  - Battleworn Dummy's options show each setting's health scaled by
    `ScaleHpForMultiplayer`.
- In Act 3, a fight of several players panics at `monster.rs:139`.
- `BurningBlood` and `BlackBlood` heal a dead teammate. The game checks
  `!Owner.Creature.IsDead`.
- Modifiers: Terminal, Cursed Run and Night Terrors act for one seat,
  where the game acts for each player. Every modifier also panics in a
  run of several players, at `run/state.rs:1471` and
  `run/state.rs:4887`.

#### F. The mod

The high-confidence items, F1 to F4, are done.

Medium:

5. `Protocol.cs:628` tracks a shared event's options as before and
   after slices of the pending tasks, which is the Drowning Beacon
   shape.
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

- Powers: Covered is `unimplemented!`.
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
   one leaves the fight's cards, and a win revives them. A player whose
   teammates the same command brought to nothing keeps them, and one who
   dies as the fight ends keeps their energy. A death that readies the
   last player not yet ready ends the turn once the play is over. A run of several players
   never offers an event option that kills its player, Trial's Double
   Down among them. A fight an event entered, once won, resumes
   every player's copy of the event. A win cancels
   the actions still waiting, as the game's does. A merchant stocks an
   inventory for each player, who buys from it alone. An item's purchase
   runs on the buyer's machine, and its results reach the other machines
   once it is over. Each room a travel enters syncs every machine's copy
   of each other player from that player's own machine, as
   `CombatStateSynchronizer` does. Each player goes
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
   choice put to a player who ended the turn takes that end back. A
   player who dies inside their own card keeps their cards until the
   card is over. The run is over only once every player is dead. A
   player who earns an extra turn takes it alone, while the others wait
   ready. A filtered card choice lists its pile once its choice has
   signalled. So a potion thrown at a player lists their hand once their
   queue reaches it. A pick from any pile but the hand keeps the cards it
   listed, whatever joins the pile while it waits. An auto-play whose
   pick is another player's ends as the pick begins, and the pre-play
   phase goes on. The other player's own pre-play phase runs all the
   same. Whispering Earring's pushed selector answers its picks as they
   come up, before the next player's pre-play phase. Each player's turn
   end runs their orbs, even after a teammate's turn end wins the
   fight. A potion discard reaches the other machines by its slot, and
   does nothing where their copy of the slot is empty. A filtered pick
   from the hand lists it again after each decision. So it offers a card
   a teammate gives while it waits. Whispering Earring aims a card at an
   ally drawn at random from the other living players. A card another
   player's potion auto-plays runs in the drinker's choice context, so
   its owner's pick branches onto the owner's queue. A lobby's clients
   join in seat order, so a run of three players seats them the same way
   every time. Fur Coat seeds its marks with its owner's slot, as
   `new Rng(Owner, Id)` does. A card made for a dead player goes into no
   pile, and every listener still hears it made. A card auto-played at
   any ally draws its target from the other living players alone, never
   from a pet. The mod waits on its own player's part of a shared event's
   option alone, not on a reward screen it opened for another player.

   Since 2026-10-28 each player in a co-op hunt carries 25 Gorgets and a
   Mercury Hourglass, as in a solo hunt. So the runs go deep.
   On 2026-10-30 a hunt of 96 seeds covered four pairs of characters.
   Ironclad and Silent played seeds 0 to 47. Defect and Necrobinder,
   Regent and Ironclad, and Silent and Defect played seeds 0 to 15. 53 seeds
   agreed to their end. On 2026-11-01 a fresh hunt
   of seeds 0 to 15 for each of the four pairs agreed on 63 of the 64
   seeds. The last, Silent and Defect seed 2, needed the room-entry sync,
   and agrees too. A hunt of seeds 16 to 31 for the four pairs agreed on 55
   of the 64 seeds, and all 64 agree now. A hunt of seeds 32 to 47 agreed
   on 60 of the 64. Defect and Necrobinder seeds 33 and 39 agree now. The
   harness lost Regent and Ironclad seed 43 to a game that exited as it
   started, and the seed agrees when it plays again. Ironclad and Silent
   seed 34 hung in the mod, and all 64 agree now.
   What is still open is listed below, worked by
   class, with the seeds that show it.
   Hunts stay at `--width 4`: at `--width 8` two games of sixteen failed
   in Godot itself. A lobby moves only once every game stands still and
   every link's reliable packets are all taken in, so load changes no
   run. A game whose player ended the turn or voted stands still only
   while another player is yet to. Once all are done, it is on its way
   to the next turn or room. Two recordings of the co-op scenarios on
   2026-10-30 came out the same, byte for byte. The narrower ally
   targets changed the moves the hunted runs chose from. So
   `coop-v0.111.0.bin` holds the sixteen Ironclad and Silent seeds hunted
   again, and 17 runs the hunts since found a split in. The simulator
   follows 58,855 of their 58,856 positions. Each machine logs the shared actions in the
   order it took them in, in `order.jsonl` under its profile. The golden tests follow the hunted runs in
   `coop-v0.111.0.bin` without the game. So a change to the simulator
   needs no hunt to show it keeps them. A rule no hunted run reaches gets
   a co-op scenario in `crates/sts2-golden/tests/coop-scenarios/`. Next, in the order of
   `docs/COOP-FIGHTS.md` and where the hunted runs stop:
   - Imitation Learning copies a teammate's Power card, but no hunted run
     reaches the copy yet. Check it against the game when one does.
   - A card or potion whose pick another player makes splits as
     `BranchingPlayerChoiceContext` does, but for a card with replays,
     which is still `unimplemented!`.
   - The rest of the scheduler (steps 3 and 4 of `docs/SCHEDULER.md`). A
     question the post-play phase asks in a fight of several players
     waits on its player's queue, and the turn's end waits for every
     player's. The turn's other phases still panic at a question there:
     `DoTurnEnd`'s cards, the pick before the flush, and a side hook's
     listener that pauses. So does the turn's end while a player's
     action waits on a question. No hunted seed reaches these now.
     Card plays, potions and the turn's end go through the queues, with
     the action types, gating and cancellations.
   - A question that waits on a player's own card play names no asker,
     where the game names the play.
   - Content that reads "the player" where the game reads an owner:
     - `RoomEntered` carries the first player's deck size and health,
       which a relic and the Terminal modifier read.
     - Every relic that hears `CardPlayed` compares the card's owner
       with its own in the game, 27 in all. Shuriken does in the
       simulator, and the other 25 do not yet.
     - Content whose comment says "a fight here has one" player.
     - The rest of `Board::seat`'s callers, which a card or potion reaches
       for its own player and a monster's move does not yet. No hunted
       seed reaches one now.
   - The hand screen checks a pick again whenever the fight changes
     (`NPlayerHand.RevalidateSelectionAfterStateChange`). A pick the
     filter no longer lets through drops, and a pick left with nothing
     picked and nothing to pick ends with no cards. Neither the mod nor
     the simulator does this yet. Both offer the hand as it stands.
   - Two players who each hold Fur Coat. The game leaves out of each
     coat's marks the points another coat marked, which the simulator's
     map does not record, so it is `unimplemented!`.
   - The splits the hunt of 2026-10-30 found, by where they show:
     - A co-op scenario's `enter_room` setup hangs: the host leaves its
       room and enters none, and the reply never comes. The mod's
       `EnterRoom` calls `EnterRoomDebug`, where `fight` goes through the
       console's own command. So no co-op scenario enters a shop yet.
     - A setup that jumps out of a fight into a room keeps the health the
       game set inside the fight, and the simulator drops it. A scenario
       that sets health with `hp` and then enters a room shows it.
4. Co-op content, as the runs reach it. Massive Scroll, Concoct, Tag
   Team and the rest of v0.111.0's co-op cards are ported. The Ball
   goes to a teammate as `CardPileCmd.GiveToAnotherPlayer` gives it.

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

`sts2-run follow <parted file> <index> --diff <dir>` plays a kept run on
the game beside the simulator, and writes both sides where they part.
`cargo xtask game-trace <index> --file <parted file> <pattern>...`
prints the game's own calls in the moves before a kept run's split.
A card shows as its id, then `#n` for its id in the fight, and `(deck n)`
for its place in the deck. Like cards differ only in these.
`cargo xtask pause-sites --why <method>` prints a chain of calls from a
game method to a pause.

A mixed hunt of 7,000 runs on v0.111.0 parted 258, and all 258 are
fixed. A second round of 3,000 mixed runs on each build started from
seed 10000. It parted 8 runs on v0.107.1 and 9 on v0.111.0, and one
v0.107.1 run stopped on a game error. All of these are fixed.

A third round, from seed 20000, parted 1 run on v0.107.1 and 3 on
v0.111.0. All 4 are fixed. A fourth round, from seed 30000, parted 3 on
v0.107.1 and 2 on v0.111.0, and all 5 are fixed.

A fifth round, from seed 40000, parted 2 on v0.107.1 and 3 on v0.111.0,
and all 5 are fixed.

Next, hunt both builds again toward 3,000 runs in a row with no split.

### 6. Coverage of the game's own code

`cargo xtask hunt --coverage` has the mod count which methods of the
game's rules code each run enters: models, commands, hooks and combat,
not screens. `cargo xtask game-coverage` adds the counts up by area and
lists the methods no run entered. Twenty runs entered 7,023 of 15,756
methods. Counting makes a hunt about four times slower.

Each gap gets a scenario, a hunt that starts from a built position, or a
note that play cannot reach it.
