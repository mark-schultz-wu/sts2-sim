# The oracle protocol

The wire contract, stated once. [README.md](README.md) covers building, running
and the version pin. [REQUIREMENTS.md](REQUIREMENTS.md) covers why the contract
is shaped this way, and what has actually been verified. This file is just the
shape.

Protocol version `1`, game `v0.107.1`, content hash `3954186980`.

## Connecting

**The oracle dials out.** It is not a server. The client binds `127.0.0.1:0`,
takes the ephemeral port the OS assigns, and passes it to the game in the
environment. Then it launches the game. So the client is already listening.
There is no port-allocation race, and no connect-retry loop.

```
STS2_HEADLESS_MODE=serve STS2_HEADLESS_PORT=<port> [STS2_HEADLESS_TOKEN=<token>] \
  ./SlayTheSpire2 --headless --force-steam=off
```

With `STS2_HEADLESS_TOKEN` set, the oracle sends the token as the first line,
before anything else. A loopback listener accepts from any local process. So
**the client must check it**. The oracle only presents it.

`STS2_HEADLESS_PORT` is required. There is no stdio fallback. The engine writes
about 140 unprefixed lines to stdout before the oracle emits anything, so
responses arrive interleaved with content nobody controls.

Closing the connection is a clean shutdown, not an error. The oracle exits on
EOF, so a client that dies does not leave a game process behind.

## Framing

Line-delimited JSON, UTF-8, one object per line. **One request, exactly one
response.** It is not always the same kind of response.

Requests are `{"cmd": "<name>", ...}`. Every response carries `type`.

## Handshake

Sent unprompted on connect, before any request:

```json
{"type":"hello","mod":"ca01e9d8c118fe7f","game":"v0.107.1","commit":"59260271",
 "assemblyHash":1692500715,"contentHash":3954186980,
 "commands":["start","fight","travel","play_card","end_turn","answer",
             "grant","enter_room","choose_option","buy","observe","quit"]}
```

Every field is read from the running build rather than compiled in as a literal,
so a client that checks them checks something real. `game`, `commit` and
`assemblyHash` come from the game's own `ReleaseInfo`. A build without one
reports `"UNRELEASED"`, `"UNKNOWN"` and `0`.

**`mod` says which build of the oracle's mods answers**, as the first 8 bytes of
a SHA-256 over the sources of every mod it loads. That is `mod/identity.sh`, run
by this mod's build and by the Rust client's `build.rs`. Both derive it from the same files with the same
script. A client refuses a mismatch at the handshake.

This is what catches a mod that was edited and never redeployed, and nothing else
can. The three fields below identify the *game*, and redeploying a mod moves
neither its assembly nor its content hash. Without it that skew surfaces as a
decode failure deep in a run, blamed on whatever the run played at the time.

It replaces a hand-maintained `protocol` integer, which had the same job and
needed remembering to bump. Deliberately strict: any source edit invalidates a
deployed build, a comment included. Narrowing it to the files that shape the wire
means maintaining a list of them, and that list is what rots.

**`contentHash` is the one to check before trusting generated content tables.**
It is XxHash32 over the model-id namespace. The game uses this exact value to
refuse a replay recorded against different content. It moves when content moves.
An assembly change that adds no cards leaves a generated id table perfectly
valid, and a `game` string can repeat across builds.

## Responses

| `type` | when |
|---|---|
| `observation` | the command advanced the game and it is now idle |
| `pending` | the game stopped to ask something |
| `error` | the command was refused; the game did not advance |
| `fault` | the mod threw handling the command; whether the move was legal is unknown |
| `bye` | reply to `quit`, and the last line sent |

### observation

```json
{"type":"observation","seq":3,"cmd":"play_card","resolved":true,"digest":"A21E…"}
```

- `seq` increments per observation or pending, never per request.
- `resolved` is `false` when combat ended mid-cascade and the decision was
  abandoned rather than completed. That is not an error. But a simulator that
  resolved it fully diverged, even if the resulting state looks agreeable.
- `digest` is always present.
- `state` is the full observation, and arrives only when asked. Ask with
  `"full": true` on the request, or `STS2_HEADLESS_OBS=full` for the whole
  session. It is roughly 2.3KB in a fight. It arrives as a nested object, not a
  string of JSON.
- `gameErrors` is an array, present only when the game logged errors during the
  command. The mod never suppresses it. The command can half-happen, and only
  the client can decide whether to trust the state.

#### Every move the run accepts

`state.moves` lists every move the run accepts, each as the request that makes
it, such as `{"cmd":"play_card","index":2,"target":1}`. A driver can send any
entry back as it is.

The list is what the game's screens offer. A card is on it where `CanPlay` says
it can be played, at each creature `CanPlayTargeting` accepts, or at nobody
where that is accepted. A potion is on it where the belt's button is enabled. A
room offers the options and stock its screen leaves enabled. The map offers the
points `MapLegality` allows, or `proceed`.

The order is part of the answer. A driver that picks by index plays the same
run only when the list comes back the same way:

- A pending question's answers come alone, as `answer` requests.
- In a fight, the belt comes first, then each card in hand at each target, then
  `end_turn`.
- Otherwise, the belt comes first, then the room's options or stock, then
  `proceed` or each point to travel to.
- A dead player outside a fight has no moves.

#### What a card in hand will do

Each entry in `combat.legal` carries an `effect`: what playing that card does,
right now.

```json
{"card":"STRIKE_IRONCLAD","cost":1,"target":"AnyEnemy","playable":true,
 "targets":[1],
 "effect":[{"target":1,"text":"Deal [red]4[/red] damage.",
            "vars":{"Damage":4},"base":{"Damage":6}}]}
```

A card's numbers are not static data and are in no generated table, because they
are not a property of the card. Strike prints 6, deals 7 with Strength, 13 into a
Vulnerable target, and 4 under Shrink. These come from the game's own
`CardModel.UpdateDynamicVarPreview`, the call that fills in the number a player
reads off the card face. So they are the numbers the game itself prints, rather
than a reimplementation of them.

- `text` is the rules text with the numbers substituted, carrying the game's
  markup.
- `vars` is those numbers, by the game's own variable names (`Damage`, `Block`,
  `Repeat`, …).
- `base` is the same map at the card's printed value, upgrades and enchantment
  included. `vars` and `base` differ exactly when something modifies the card.

One entry per **distinct outcome**. A card that names a single creature is
previewed against each one it can legally hit, since that is where the numbers
differ. Identical entries collapse to one with `"target": null`, so an untargeted
card and a card whose damage does not vary both report once.

Do not read "this number moved" out of the markup. `[green]` and `[red]` do mark
a recomputed value on a card, but they are also just colours. Neow's *Lose
`[red]12[/red]` Max HP* is red because losing Max HP is bad. `base` is the field
that answers it.

#### Rooms say what they say

An event reports `title` and `text` beside its id, and each of its options reports
`title` and `text` beside its `key`. The keys stay, because they are the identity.
The words are there because `BYRDONIS_NEST.pages.INITIAL.options.EAT` names a
choice without saying anything about it. Either can be `null`. A page with no
description is ordinary, and the game's own tables are the authority on whether
there is one.

Two kinds of key name no page. The Architect's lines are
`THE_ARCHITECT.dialogue.<line>`, and the option that closes it is a bare
`PROCEED`.

#### A start pins the character's wins

The Architect ends a won run on a dialogue picked by the profile's wins with
the character. The profile keeps every run the game wins, so `start` sets the
count to its `wins`, none unless it says. Nothing else in a run reads it.

#### The map is a graph, not just a position

`state.map` carries three things, and none is derivable from the others.

```json
{"at":"3,0","visited":["3,0"],
 "travelable":[{"at":"2,1","type":"Monster"}],
 "points":[{"at":"3,0","type":"Ancient","to":["2,1","4,1"]}]}
```

- `at` and `visited` are where the run stands and where it went.
- `travelable` is where it can step **now**. This depends on the run, not only on
  the map: free travel (`Winged Boots`) widens it to a whole row.
- `points` is the shape of the act, which does not change as it is played. Every
  node, its type, and `to`, the nodes it leads on to.
- `proceed` is true when the act is behind the run and the `proceed` command is
  the move. An act's boss has no onward node, so `travelable` is correctly empty
  once it is beaten. Without this field a finished act is indistinguishable from
  a run that stopped.

Edges are `MapPoint.Children` verbatim, the same set `NMapScreen.DrawPaths` draws
its lines from. So a picture built from `points` is the picture the game shows.

The boss, the second boss and the starting node are **not in `Grid`**, so
`GetAllMapPoints` never yields them. They are added by hand. This is the same
trap described under map legality, and it bites late. An act reads correctly for
fifteen floors and then ends at a node that the field never reported.

#### A shop can offer what it will not sell

A shop entry reports `stocked` and `affordable`, which are the two checks
`MerchantEntry.OnTryPurchaseWrapper` makes. For a potion they are not the whole
story. A potion is procured into a slot, and `PotionCmd.TryToProcure` fails with
`TooFull` when there is none. So a full belt makes an entry that is stocked,
affordable, and refused.

`blocked` is that third answer: a string saying why, or absent. `potionSlots` on
the state is the belt's size, which `potions` alone cannot imply because relics
change it.

The remaining way a potion can be refused is `Hook.ShouldProcurePotion`, which
the mod cannot answer without running it, and so does not report.

#### The belt says what can be drunk, and the collection says what it does

`potions` is a list of objects rather than of ids:

```json
{"id":"FIRE_POTION","title":"Fire Potion","text":"Deal 20 damage.",
 "usable":true,"reason":null,"targets":[1,2]}
```

`usable`, `reason` and `targets` are the same three fields one card in `legal`
carries, and they mean the same things. `reason` is absent exactly when the
potion can be drunk, and `targets` is the creatures that must be chosen between.

Empty `targets` is not "cannot be drunk". A potion drunk outside combat has none,
because there are no combat ids to name, and neither does a self-targeted one.
Both are sent with no target, and the game points it at the drinker itself.
`PotionModel.EnqueueManualUse` fills that in.

That last case is worth stating, because `PotionModel.IsValidTarget` says
otherwise. It answers true for `Self` and `AnyPlayer`. `CardModel.IsValidTarget`
refuses every non-null target that is not `AnyEnemy` or `AnyAlly`, so the card
list is already the *choice* set rather than the valid-target set. The potion
list is filtered to match, on the game's own distinction. `NPotionPopup` labels
its button *throw* for `AnyEnemy`, `AllEnemies`, `TargetedNoCreature` and
`CanThrowAtAlly` (multiplayer only), and *drink* for everything else.
`CanThrowAtAlly` is asked rather than its target type read, because it carries
what the enum cannot. Swift Potion is `AnyPlayer`, and in a single-player run
there is no other player to throw it at.

`reason` is one of `Dead`, `Queued`, `Automatic`, `OutOfCombat`, `NotYourTurn`,
`Stuck`, `ItsOwnCheck`, reported in that order, the first that holds. There is
no game enum for this. `NPotionPopup` decides it by enabling and disabling a
button, in `_Ready`'s switch over `PotionUsage` and then in `RefreshButtons`.
`Stuck` is the player's use-or-remove-potions gate, which also stops the potion
being thrown away. `ItsOwnCheck` is the potion's own
`PassesCustomUsabilityCheck`.

`relics` is a list of objects for the same reason a potion is, minus the
legality:

```json
{"id":"BURNING_BLOOD","title":"Burning Blood",
 "text":"At the end of combat, heal 6 HP."}
```

Both prose fields are the game's own `LocString`s, formatted the way its hover
tips format them, through `RelicModel.DynamicDescription` and
`PotionModel.DynamicDescription`. That formatting is not a nicety. A relic that
counts prints its count, and rendering the string without the variables throws
rather than returning the raw text. It also means the prose is genuinely run
state and not build data. The same relic reads differently once its counter
moves, which is why it is reported here rather than dumped into a table.

`title` and `text` are absent, not defaulted, where the localisation table has no
key. That is a fault in the build rather than a name a client invents.

### error

```json
{"type":"error","cmd":"travel","reason":{"kind":"not_travelable","at":"2,1"}}
{"type":"error","cmd":"play_card","reason":{"kind":"unplayable","card":"BASH+1",
  "why":"EnergyCostTooHigh","preventer":null}}
{"type":"error","cmd":"buy","reason":{"kind":"unpurchasable","index":2,"cost":75,
  "gold":30,"stocked":true,"blocked":null}}
```

The reason is a tagged object, and there is no `message` beside it. It was a
sentence once, and a sentence cannot be compared. Two implementations refusing
the same move for the same reason had to agree on English before they were seen
to agree at all. The prose is the client's, written once from these fields.

Every field is a fact the client already has, out of the observation that it
acted on. That is an index and a count, a coordinate, a card id, or the numbers
a purchase turned on. It is what makes a refusal checkable rather than merely
informative. It is also why a refusal never restates the legal set. The
observation already carried it, and a second copy is free to disagree with the
first.

`kind` is one of:

| group | kinds |
|---|---|
| wrong moment | `no_run` `run_over` `not_in_combat` `in_combat` `wrong_room` `nothing_to_choose` `pending` `nothing_pending` |
| no such thing | `out_of_range` `no_such_creature` |
| the rules say no | `unplayable` `undrinkable` `wrong_target` `wrong_potion_target` `not_travelable` `cannot_proceed` `option_disabled` `option_locked` `already_answered` `unpurchasable` |
| answering a question | `duplicate` `wrong_count` `not_skippable` `already_taken` `needs_card_choice` `no_such_alternative` `empty_take` `off_grid` `already_uncovered` |
| prose, deliberately | `console` `malformed` |

The last two carry a `message` inside the reason, because there is nothing to
compare. `console` is the game's own `FightConsoleCmd` or `EventConsoleCmd`
declining in its own words. `malformed` is a request that is not a request. That
is a missing field, a command that is not a command, or a name this build does
not have. A client speaking the typed vocabulary cannot send one in the first
place.

### fault

```json
{"type":"fault","cmd":"play_card","error":"NullReferenceException",
 "message":"Object reference not set to an instance of an object"}
```

The mod threw. This is its own message type and not an `error`, because the two
mean opposite things. A refusal is the game's answer, and says the move was
illegal. A fault is the mod failing to ask the question, and says nothing about
the move at all. Flattened together, a bug in the adapter reads as a rule of the
game.

## Commands

| cmd | arguments |
|---|---|
| `start` | `seed`, `character`, `ascension` — a level name such as `DeadlyEnemies` or a number, default `0`, matched case-insensitively; `modifiers` — an array of modifier entry names, matched case-insensitively. `CHARACTER_CARDS` is `{"id": "CHARACTER_CARDS", "character": "DEFECT"}`, naming the character whose cards it adds |
| `start` | `wins` — the character's wins on the profile, default `0` |
| `fight` | `encounter` — a model entry name such as `BOWLBUGS_WEAK` |
| `event` | `event` — a model entry name such as `CRYSTAL_SPHERE` |
| `power` | `power` — a model entry name such as `WEAK_POWER`; `amount` — an integer; `target` — a creature's `CombatId` |
| `hp` | `hp` — a positive integer; `target` — a creature's `CombatId`, or absent for the player |
| `enter_room` | `type` — a `RoomType` name, case-insensitive |
| `travel` | `col`, `row` — integers |
| `play_card` | `index` — into the hand; `target` — a creature's `CombatId`, or absent |
| `end_turn` | — |
| `use_potion` | `index` — into the belt as `potions` reports it; `target` — a creature's `CombatId`, or absent. Out of a fight a Foul Potion is thrown at a merchant: see below |
| `discard_potion` | `index` — into the belt as `potions` reports it. The one command a reward screen takes besides `answer`: see below |
| `choose_option` | `index` — into the options the last observation reported |
| `buy` | `index` — into the shop entries |
| `grant` | `card`, `relic` or `potion` — one entry name each, and one command can carry all three. It can park a question: see below |
| `answer` | see below; shape depends on the pending |
| `observe` | — |
| `quit` | — |

`"full": true` is honoured on any command, since every one of them answers with an
observation.

### A Foul Potion thrown at a merchant

Out of a fight, a Foul Potion can be thrown only at a merchant's button.
Headless there is no button, so the mod passes the potion's check in a
`MerchantRoom`, and in the Fake Merchant's event until its fight starts. No
inventory is ever open headless. At a shop the throw gains the potion's 100
gold. At the Fake Merchant the mod calls `FakeMerchant.FoulPotionThrown`,
which `OnUse` calls only through the event's `NFakeMerchant` node. The
throw starts the event's fight.

### `fight` is scenario construction, not a move

It jumps straight into the named encounter at `MapPointType.Unassigned`, and
abandons whatever room the run was in. The floor advances while `map.at` and
`map.visited` do not. So map and room disagree by construction, and a run can
reach combat without having taken its Neow bonus.

That is the point rather than a defect. Reaching a chosen fight by travelling
means hunting for a seed that puts one where you want it, and H6 exists to avoid
exactly that. Use `travel` to a `Monster` point when you want the run the seed
actually produces.

It goes through the game's own `FightConsoleCmd`. So an unknown name comes back
with the game's own message, `Encounter 'X' not found`, rather than a guess of
ours. That command reseeds the encounter from the wall clock.
`sts2_determinism` patches that to nothing, which is what makes it usable here
at all.

Naming a modifier, card or relic that does not exist is refused rather than
ignored. Silently starting an unmodified run hides the client bug behind a
divergence three decisions later.

### `grant` runs the relic's hooks, which can ask something

A relic arrives through `RelicCmd.Obtain`, never by pushing onto a collection,
so its `AfterObtained` runs. The load path skips that hook, and a poked-in relic
produces a state the game never produces.

Which means `grant` can answer with a **pending** rather than an observation.
Royal Stamp's `AfterObtained` opens a one-card selection and enchants the
answer, so granting it asks before it finishes. Treat `grant` like any other
command that parks.

### `power` is scenario construction too

It puts a power on a creature in the fight, through the game's own
`ApplyPowerConsoleCmd`. That reaches `PowerCmd.Apply`, or `ModifyAmount` when
the creature already holds one, which is the path a card takes. So a debuff put
on the player this way has its `SkipNextDurationTick` set, exactly as one a
monster applied does. The position reached is one the game reaches.

A fight has to be on. There is no creature to name outside one, and the command
answers `This doesn't appear to be a combat!`.

`amount` is the game's own overloaded number. It is a count for most powers.
Below zero it is a single endless instance, for the one power that works its
stacking out at the time. `SHRINK_POWER` at `-1` is the Shrinker Beetle's, and
it never runs out. The client spells what it means and this passes the number
through.

`target` is a `CombatId`, the way every other command addresses a creature. The
console command underneath addresses one by its position in the combat state's
list. The mod translates, so a scenario names the creature an observation
named.

It exists because a position with two debuffs already on the player is an hour
of play away. The alternative is a recording of a run that reaches one.

### `discard_potion` throws a potion away

It is the discard button on a potion's popup. `NPotionPopup` enqueues a
`DiscardPotionGameAction`, which runs `PotionCmd.Discard` and raises
`AfterPotionDiscarded`, so Belt Buckle hears it. The popup offers it in a fight
or out, so every list of moves ends with a discard for each potion. The button is
disabled for a potion already queued, or an owner who is dead, and so is this.

A reward screen leaves the belt open, and a full belt is where a player discards
to make room. So while a `rewards` pending is parked, `discard_potion` is
accepted beside `answer`, and its reply asks the rewards again.

### `hp` is scenario construction too

It sets the hit points of the player, or of the creature `target` names in the
fight, through `CreatureCmd.SetCurrentHp`. That raises `AfterCurrentHpChanged`
and no damage hook, so no relic that counts hits wakes. Without a `target` it
is the player, in a fight or out. A `target` needs a fight.

It exists because a position at three hit points is a fight's worth of damage
away. On the way, the relics that count each hit change the position.

### `event` is scenario construction too

The same shape as `fight`, through the game's own `EventConsoleCmd`. It appends a
map point and enters an `EventRoom`, so map and room disagree exactly as they do
after a `fight`. An unknown name comes back as `Event 'X' not found`.

The console lists no `THE_ARCHITECT`. That one is entered the way the run
enters it past the last act's boss, with no map point and no floor. Off the
last act its `PROCEED` goes on to the next act, as the game's does.

It exists because `enter_room` takes a room *type*. An `Event` room gives
whatever event the seed offers, so reaching one particular event meant playing
to it. That was 515 recorded steps, in the case that made this necessary.

## Pendings

A `pending` reports that the game stopped mid-action to ask something. It
replaces the reply to whatever command was issued.

### select_cards

```json
{"type":"pending","seq":7,"cmd":"play_card","kind":"select_cards",
 "min":1,"max":1,"asker":"DiscardAction",
 "options":[{"index":0,"card":"STRIKE","text":"Deal 6 damage."},
            {"index":1,"card":"BASH+1","text":"Deal 10 damage.\nApply 3 Vulnerable."}],
 "picked":[],"pile":"Draw"}
```

`pile` is the `PileType` of the fight's pile every option stands in, when they
all stand in one. It is `null` outside a fight, for the deck, and for cards made
to be chosen between. The game never tells the selector the pile, so the
mod reads each card's own. A front end lists a draw pile sorted, as the game's screen
does, so the order of the draws does not show.

`asker` is the running action, not the game's own prompt text. `ICardSelector`
receives only options and bounds. So this is the best attribution available, and
it is labelled as such rather than dressed up.

`text` is the card's rules text with its own numbers in it, straight out of the
game's localisation tables. The mod does **not** apply combat hooks. This is what the
card does in general, which is the question a selection prompt asks. It carries
the game's own markup (`[gold]Block[/gold]`, `[img]…[/img]`), which is a
display's problem rather than the protocol's.

Answer a click at a time, as the game's screens take a selection:

- `{"cmd":"answer","pick":0}` adds one option after those already picked.
- `{"cmd":"answer","undo":true}` takes back the last pick.
- `{"cmd":"answer","done":true}` hands the picks to the game.

A pick or an undo leaves the question parked, and the reply asks it again with
`picked` holding the picks so far, in order. The order is part of the answer.
The game hands the cards back in click order, and Purity exhausts them in it.

`{"cmd":"answer","cards":[0,2]}` picks each of its options in turn after any
picks so far, then hands them over, in one answer.

`{"cmd":"answer","cancel":true}` is the screen's back button, on a selection
whose `CardSelectorPrefs` are `Cancelable`: Smith, Cook, the merchant's card
removal and the card-removal reward. The game gets no cards, so nothing that
opened the selection goes through. A rest site option stays, nobody buys the
removal, and the reward stays on its screen. The selector never sees the prefs, so a
patch on `CardSelectCmd` notes the flag for it. A selection that has no back
button refuses `cancel`, and only one that has one lists it.

Refusals leave the picks as they were:

- `out_of_range` is an index past the options.
- `duplicate` is an option picked already. The same card twice is one selection
  to the game, and two to the client.
- `wrong_count` is a pick past `max`, or done short of `min`.
- `nothing_picked` is an undo with no pick to take back.

### divine

```json
{"type":"pending","seq":9,"cmd":"choose_option","kind":"divine",
 "left":6,"width":11,"height":11,"tool":"Big",
 "cleared":[{"x":5,"y":5},{"x":5,"y":6}],
 "revealed":[{"item":"CrystalSphereGold","good":true,"x":2,"y":3,"width":2,"height":1}]}
```

The Crystal Sphere, and nothing else in the game: it is the one event with a
screen of its own. Both of its options `await CrystalSphereMinigame.PlayMinigame()`,
which shows `crystal_sphere_screen.tscn` and waits on a click. Headless, that
scene is not in the preload cache. Asking for it takes the process down without
an exception, and the log stops after `Asset not cached`. The mod skips the
screen and sends the clicks here instead.

`cleared` lists the cells already uncovered rather than the hidden ones. Hidden
is the starting state of all `width * height` of them, so the short list is the
one that says something. `revealed` is what the divinations turned up so
far, and `good` is false for a curse.

Answer with a cell and a tool: `{"cmd":"answer","divine":{"x":3,"y":7,"tool":"Big"}}`.
`Small` uncovers the named cell, `Big` its eight neighbours as well. Both cost one
divination, so `Big` is a gamble and not a saving. A cell off the grid or already
uncovered is refused, and the question survives the refusal.

The game's `CrystalSphereToolType` has a third member, `None`. The mod refuses it, because
`CellClicked` tests only for `Big`, so `None` and `Small` are one move under two
names.

When the last divination is spent the event resumes on its own, and pays out
what was uncovered. That arrives as an ordinary `rewards` pending, through
`DoLocalCrystalSphereRewards`.

### choose_bundle

```json
{"type":"pending","seq":3,"cmd":"grant","kind":"choose_bundle",
 "bundles":[{"index":0,"cards":[{"card":"POMMEL_STRIKE","text":"Deal 9 damage.\nDraw 1 card."},
                                {"card":"TWIN_STRIKE","text":"Deal 5 damage twice."},
                                {"card":"UPPERCUT","text":"Deal 13 damage.\nApply 1 Weak.\nApply 1 Vulnerable."}]},
            {"index":1,"cards":[…]}]}
```

Scroll Boxes' choose-a-bundle screen, which `CardSelectCmd.FromChooseABundleScreen`
shows. It asks no selector, and in `TestMode` it takes the first bundle unasked.
So the mod stands in for the screen, and asks here.

`text` is as `select_cards` gives it. Answer with one bundle's index, as
`{"cmd":"answer","cards":[1]}`. The bundle's cards all go into the deck.

### confirm

```json
{"type":"pending","seq":4,"cmd":"choose_option","kind":"confirm"}
```

The abandon-run popup, which Trial's Double Down opens. `NModalContainer` is null
headless, so the mod stands in for the popup's two buttons, and asks here.

Answer `{"cmd":"answer","confirm":true}` to abandon the run. The mod marks the run
abandoned and kills the player, as `AbandonInternal` does, so the run ends lost.
Answer `{"cmd":"answer","confirm":false}` to close the popup on the Reject page,
whose options stay.

### rewards

```json
{"type":"pending","seq":9,"cmd":"end_turn","kind":"rewards","skippable":true,
 "rewards":[{"index":0,"type":"GoldReward","populated":true,"taken":false,"amount":25},
            {"index":1,"type":"CardReward","populated":true,"taken":false,
             "cards":[{"card":"STRIKE","text":"Deal 6 damage."},{"card":"BASH+1"}]}]}
```

`populated` is reported rather than assumed. A reward can exist in the set
without content, and an unpopulated reward is not an absent one. Per type,
`GoldReward` adds `amount`, `CardReward` adds `cards`, `RelicReward` adds `relic`
and `rarity`, `PotionReward` adds `potion`.

Answer with either `{"cmd":"answer","skip":true}`, which is refused if
`skippable` is false, or `{"cmd":"answer","take":[{"index":1,"card":0}]}`. A
`CardReward` **must** carry a `card` index. It is refused rather than defaulted
to the first card, because choosing for the client is the one thing the oracle
must not do.

A `CardReward` that offers more than its cards adds `alternatives`, each by
its `OptionId`. `REROLL` comes first where `CanReroll` is set, then what
`ModifyCardRewardAlternatives` adds, such as `SACRIFICE`. `Skip` is left out,
since it leaves the reward untaken. Take one with
`{"cmd":"answer","take":[{"index":1,"alternative":"REROLL"}]}`, which is refused
as `no_such_alternative` when the reward does not offer it. `REROLL` keeps the
reward on the screen, and the next `rewards` pending shows its new cards.

**Taking is not the end of the screen.** A `take` applies what it names and
nothing else. If the set still has rewards in it, another `rewards` pending
arrives with the same list and `taken` set on the ones that are gone. Indices
never renumber, so a reward answers to the same number every time it is offered.
The only way to leave a reward behind is `skip`, which abandons everything not yet
taken and completes the set.

`taken` is also how a *failed* selection shows. `Reward.OnSelect` can return
false, as for a potion with no free slot. The reward is then neither taken nor
gone, so the set does not complete on its own, and the next question still
offers it.

An empty `take` is refused. It was the old spelling of "take nothing and forfeit
the rest". Treating it as a no-op leaves a client asking for behaviour that no
longer exists, and hearing nothing about it.

## Ordering rules

1. **While a pending is outstanding**, only `answer`, `observe` and `quit` are
   accepted. Anything else is refused and the pending survives the refusal.
2. **`answer` with nothing pending** is refused.
3. **Once the run is over**, only `observe`, `start` and `quit` are accepted.
   The error names victory or defeat. Without this the oracle kept taking
   commands after a defeat, and a dead player was observed healing at a rest
   site.

## Addressing

Three different schemes, each for a reason:

- **Cards in hand: by index.** It is how the observation reports the hand, and
  it is the only way to disambiguate two identical cards.
- **Targets: by `CombatId`.** This is a stable identity. It does not shift when
  a creature dies, and an index into the living-creature list does.
- **Options and shop entries: by index, valid only against the observation they
  came from.** The rest-site option list *shrinks*. Choosing one removes it, and
  `ShouldDisableRemainingRestSiteOptions` can clear the rest.

Card identifiers are the entry name, plus `+N` when upgraded, plus
`/AFFLICTION` when afflicted, plus `*ENCHANTMENT` when enchanted, in that
order: `STRIKE`, `BASH+1`, `STRIKE/SMOG`, `BASH+1/SMOG*ADROIT`.

The three are what a card carries into a pile, which is what makes two decks
comparable. An entry name is `[A-Z0-9_]` after `Slugify`, so none of the three
separators can appear inside one.
