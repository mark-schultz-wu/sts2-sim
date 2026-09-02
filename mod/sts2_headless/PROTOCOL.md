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
{"type":"hello","protocol":1,"game":"v0.107.1","commit":"59260271",
 "assemblyHash":1692500715,"contentHash":3954186980,
 "commands":["start","fight","travel","play_card","end_turn","answer",
             "grant","enter_room","choose_option","buy","observe","quit"]}
```

Every field but `protocol` is read from the running build rather than compiled
in, so a client that checks them is checking something. `game`, `commit` and
`assemblyHash` come from the game's own `ReleaseInfo`. A build without one
reports `"UNRELEASED"`, `"UNKNOWN"` and `0`.

**`contentHash` is the one to check before trusting generated content tables.**
It is XxHash32 over the model-id namespace. The game uses this exact value to
refuse a replay recorded against different content. It moves when content moves.
An assembly change that adds no cards leaves a generated id table perfectly
valid, and a `game` string can repeat across builds.

## Responses

| `type` | when |
|---|---|
| `observation` | the command advanced the game and it is now idle |
| `pending` | the game has stopped to ask something |
| `error` | the command was refused; the game did not advance |
| `bye` | reply to `quit`, and the last line sent |

### observation

```json
{"type":"observation","seq":3,"cmd":"play_card","resolved":true,"digest":"A21E…"}
```

- `seq` increments per observation or pending, never per request.
- `resolved` is `false` when combat ended mid-cascade and the decision was
  abandoned rather than completed. That is not an error. But a simulator that
  resolved it fully has diverged, even if the resulting state looks agreeable.
- `digest` is always present.
- `state` is the full observation, and arrives only when asked. Ask with
  `"full": true` on the request, or `STS2_HEADLESS_OBS=full` for the whole
  session. It is roughly 1.6KB. It arrives as a nested object, not a string of
  JSON.
- `gameErrors` is an array, present only when the game logged errors during the
  command. It is never suppressed. The command can have half-happened, and only
  the client can decide whether to trust the state.

### error

```json
{"type":"error","cmd":"travel","message":"(2,1) is not travelable from here; legal: (1,1) (3,1)"}
```

Messages are written to be read. An illegal `travel` lists the legal coords. An
unplayable `play_card` names the reason and the blocking model. A bad
`enter_room` lists the room types.

## Commands

| cmd | arguments |
|---|---|
| `start` | `seed`, `character`, `ascension` — a level name such as `DeadlyEnemies` or a number, default `0`, matched case-insensitively; `modifiers` — an array of modifier entry names, matched case-insensitively |
| `fight` | `encounter` — a model entry name, e.g. `BOWLBUGS_WEAK` |
| `enter_room` | `type` — a `RoomType` name, case-insensitive |
| `travel` | `col`, `row` — integers |
| `play_card` | `index` — into the hand; `target` — a creature's `CombatId`, or absent |
| `end_turn` | — |
| `choose_option` | `index` — into the options the last observation reported |
| `buy` | `index` — into the shop entries |
| `grant` | `card` and/or `relic` — entry names; at least one required |
| `answer` | see below; shape depends on the pending |
| `observe` | — |
| `quit` | — |

`"full": true` is honoured on any command, since every one of them answers with an
observation.

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

## Pendings

A `pending` reports that the game has stopped mid-action to ask something. It
replaces the reply to whatever command was issued.

### select_cards

```json
{"type":"pending","seq":7,"cmd":"play_card","kind":"select_cards",
 "min":1,"max":1,"asker":"DiscardAction",
 "options":[{"index":0,"card":"STRIKE"},{"index":1,"card":"BASH+1"}]}
```

`asker` is the running action, not the game's own prompt text. `ICardSelector`
receives only options and bounds. So this is the best attribution available, and
it is labelled as such rather than dressed up.

Answer with option indices, as `{"cmd":"answer","cards":[0]}`. Repeating an
index is refused. The same card twice is one selection to the game, and two to
the client.

### rewards

```json
{"type":"pending","seq":9,"cmd":"end_turn","kind":"rewards","skippable":true,
 "rewards":[{"index":0,"type":"GoldReward","populated":true,"amount":25},
            {"index":1,"type":"CardReward","populated":true,"cards":["STRIKE","BASH+1"]}]}
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

Card identifiers are the entry name, plus `+N` when upgraded, plus `/AFFLICTION`
when afflicted: `STRIKE`, `BASH+1`, `STRIKE/BURDEN`.
