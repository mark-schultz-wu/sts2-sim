# Blockers

What stands between the simulator and a complete port, solo and co-op, in
the order to work them. `/blocker-grind` takes the first open item, one
PR at a time.

## Done means

Every recorded run matches the simulator step by step, to the run's end.
The game plays each one itself, picking at random from its own legal
moves. The recorded runs cover:

- each character, at every ascension level;
- runs with each modifier the game's custom mode offers;
- both builds, v0.107.1 and v0.111.0;
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

Speed is a goal as well as a guard. Item 3 says the target.

## How the list changes

- A PR that fixes an item deletes it, in the same commit.
- A PR that finds a new blocker adds it, where it belongs in the order,
  in the same commit. A blocker found while fixing another goes above it
  only when the fix cannot land without it.
- An item too big for one PR is split into items that each fit one.
- An item that needs Mark's decision says so, and the grind stops there
  and asks.

## Open

### 1. Co-op

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
   Of sixteen hunted seeds, three match to the run's end, and every seed
   that reaches a fight matches through its first fight and rewards. The fight holds
   every player, and the turn ends once every player ends it. A monster's
   move reaches every player, with its health and powered block scaled by
   the player count. Each relic reads its own owner. A card play that
   stops on a question waits at the front of its player's queue, and the
   other players act around it. A won fight hands each player back their
   health and rolls their own rewards. A player who dies beside a living
   one leaves the fight's cards, and a win revives them. A win cancels
   the actions still waiting, as the game's does. A merchant stocks an
   inventory for each player, who buys from it alone. Each player goes
   through their own copy of an event. Hunts stay at
   `--width 4`: at `--width 8` two games of sixteen failed in Godot
   itself. Next, in the order of `docs/COOP-FIGHTS.md` and where the
   hunted runs stop:
   - The rest of the scheduler (steps 3 and 4 of `docs/SCHEDULER.md`). A
     question the turn loop asks belongs to no player's queue yet. Each
     phase of the turn's end panics at one in a fight of several players.
     So does the turn's end while a player's action waits on a question.
     Card plays, potions and the turn's end go through the queues, with
     the action types, gating and cancellations.
   - A potion thrown at another player, which panics in four of the
     sixteen hunted runs: the potions act on their drinker.
   - Monster power scaling by the player count, where four hunted runs
     part. A monster's Plating reads 24 in the game where the simulator
     has 8. Slippery, Artifact and Shriek differ too.
   - Seed 14 parts at step 3, after its first travel.
   - A rest site in a run of several players offers Mend, which the
     simulator leaves out. One hunted run parts there.
   - Unrelenting makes the wrong player's attack free. One hunted run
     parts there.
   - The simulator ticks down a dead player's Frail, and the game does
     not. One hunted run parts there.
   - A shared event, which the players vote through, and an event that
     opens on a fight panic in a run of several players.
   - A fight won inside an event, and the extra rewards a monster gives,
     which panic in a run of several players.
   - Content that reads "the player" where the game reads an owner:
     - `FightView::player`, which about a dozen power rules read, panics
       in a fight of several players.
     - The decision hooks `ShouldDie` and `ShouldDieLate` carry the
       fight's player for a relic to compare with.
     - `RoomEntered` carries the first player's deck size and health,
       which a relic and the Terminal modifier read.
     - Content whose comment says "a fight here has one" player.
     - The rest of `Board::seat`'s callers, which a card or potion reaches
       for its own player and a monster's move does not yet.
   - A player who dies inside their own card or potion beside a living
     one, and a death that leaves every standing player ready, panic.
4. Co-op content, as the runs reach it. Massive Scroll is ported. The
   Concoct power's `AfterDamageGiven` stops one hunted run.

### 2. The engine's command frames as coroutines

Every rule in content that pauses is a `diapause` coroutine, and so are
the turn loop and the run layer's pickups (`docs/SCHEDULER.md`,
`docs/RUN-PAUSES.md`). The engine still holds the frames of a command
inside one action by hand. That is a draw, an auto-play, `PowerCmd.Apply`
over several targets, and an attack's hits. They are `Deferred`'s
command variants and `Then::Play` and `Then::Pick`, with `Live`'s fields
that only hold them. Each carrier moves on its own, which is step 5 of
`docs/SCHEDULER.md`. It changes no order. Mark chose on 2026-10-27 to
leave it until co-op fights are done, since they do not need it.

### 3. Speed

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

### 4. The splits a hunt finds

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

### 5. Coverage of the game's own code

`cargo xtask hunt --coverage` has the mod count which methods of the
game's rules code each run enters: models, commands, hooks and combat,
not screens. `cargo xtask game-coverage` adds the counts up by area and
lists the methods no run entered. Twenty runs entered 7,023 of 15,756
methods. Counting makes a hunt about four times slower.

Each gap gets a scenario, a hunt that starts from a built position, or a
note that play cannot reach it.
