# The oracle protocol

The wire contract, stated once. [README.md](README.md) covers building, running
and the version pin; [REQUIREMENTS.md](REQUIREMENTS.md) covers why the contract is
shaped this way and what has actually been verified. This file is just the shape.

Protocol version `1`, game `v0.107.1`, assembly hash `1692500715`.

## Connecting

**The oracle dials out.** It is not a server. The client binds `127.0.0.1:0`, takes
the ephemeral port the OS assigns, passes it to the game in the environment, and
then launches the game — so the client is already listening and there is no
port-allocation race and no connect-retry loop.

```
STS2_HEADLESS_MODE=serve STS2_HEADLESS_PORT=<port> [STS2_HEADLESS_TOKEN=<token>] \
  ./SlayTheSpire2 --headless --force-steam=off
```

With `STS2_HEADLESS_TOKEN` set, the oracle sends the token as the first line before
anything else. A loopback listener accepts from any local process, so **the client
must check it** — the oracle only presents it.

`STS2_HEADLESS_PORT` is required. There is no stdio fallback: the engine writes ~140
unprefixed lines to stdout before the oracle emits anything, so responses would
arrive interleaved with content nobody controls.

Closing the connection is a clean shutdown, not an error. The oracle exits on EOF,
so a client that dies does not leave a game process behind.

## Framing

Line-delimited JSON, UTF-8, one object per line. **One request, exactly one
response** — though not always the same kind of response.

Requests are `{"cmd": "<name>", ...}`. Every response carries `type`.

## Handshake

Sent unprompted on connect, before any request:

```json
{"type":"hello","protocol":1,"game":"v0.107.1","assemblyHash":1692500715,
 "commands":["start","enter_fight","travel","play_card","end_turn","answer",
             "grant","enter_room","choose_option","buy","observe","quit"]}
```

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

- `seq` — increments per observation or pending, never per request.
- `resolved` — `false` when combat ended mid-cascade and the decision was
  abandoned rather than completed. Not an error, but a simulator that resolved it
  fully has diverged even if the resulting state looks agreeable.
- `digest` — always present.
- `state` — the full observation, only when asked: `"full": true` on the request,
  or `STS2_HEADLESS_OBS=full` for the whole session. Roughly 1.6KB. It arrives as a
  nested object, not a string of JSON.
- `gameErrors` — an array, present only when the game logged errors during the
  command. Never suppressed: the command may have half-happened, and only the
  client can decide whether to trust the state.

### error

```json
{"type":"error","cmd":"travel","message":"(2,1) is not travelable from here; legal: (1,1) (3,1)"}
```

Messages are written to be read: illegal `travel` lists the legal coords,
unplayable `play_card` names the reason and the blocking model, and a bad
`enter_room` lists the room types.

## Commands

| cmd | arguments |
|---|---|
| `start` | `seed`, `character`, `ascension` (default `0`), `modifiers` — an array of modifier entry names, matched case-insensitively |
| `enter_fight` | — (see the warning below) |
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

### `enter_fight` is not a move

It teleports into one **hardcoded** encounter — `BowlbugsWeak` — at
`MapPointType.Unassigned`, abandoning whatever room the run was in. The floor
advances while `map.at` and `map.visited` do not, so map and room disagree by
construction, and a run reaches combat without ever having taken its Neow bonus.
That is a state the real game cannot produce.

It exists because the game's own `fight` console command calls
`encounterModel.DebugRandomizeRng()`, which seeds from `DateTime.UtcNow` at
one-second granularity — so a scenario built with it reproduces only for runs
landing in the same second. Avoiding that command rather than patching it is
what left this one hardcoded.

**Use `travel` to a `Monster` point instead.** It reaches a real fight, chosen by
the seed, on the map. `enter_fight` should be treated as a placeholder for the
parameterised scenario command H6 actually needs.

Naming a modifier, card or relic that does not exist is refused rather than
ignored — silently starting an unmodified run would hide the client bug behind a
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

`asker` is the running action, not the game's own prompt text — `ICardSelector`
receives only options and bounds, so this is the best attribution available and is
labelled as such rather than dressed up.

Answer with option indices: `{"cmd":"answer","cards":[0]}`. Repeating an index is
refused — the same card twice is one selection to the game and two to the client.

### rewards

```json
{"type":"pending","seq":9,"cmd":"end_turn","kind":"rewards","skippable":true,
 "rewards":[{"index":0,"type":"GoldReward","populated":true,"amount":25},
            {"index":1,"type":"CardReward","populated":true,"cards":["STRIKE","BASH+1"]}]}
```

`populated` is reported rather than assumed: a reward can exist in the set without
content, and an unpopulated reward is not the same as an absent one. Per type,
`GoldReward` adds `amount`, `CardReward` adds `cards`, `RelicReward` adds `relic`
and `rarity`, `PotionReward` adds `potion`.

Answer either `{"cmd":"answer","skip":true}` — refused if `skippable` is false — or
`{"cmd":"answer","take":[{"index":1,"card":0}]}`. A `CardReward` **must** carry a
`card` index; it is refused rather than defaulted to the first card, because
choosing for the client is the one thing the oracle must not do.

## Ordering rules

1. **While a pending is outstanding**, only `answer`, `observe` and `quit` are
   accepted. Anything else is refused and the pending survives the refusal.
2. **`answer` with nothing pending** is refused.
3. **Once the run is over**, only `observe`, `start` and `quit` are accepted; the
   error names victory or defeat. Without this the oracle kept taking commands
   after a defeat, and a dead player was observed healing at a rest site.

## Addressing

Three different schemes, each for a reason:

- **Cards in hand: by index.** It is how the observation reports the hand, and the
  only way to disambiguate two identical cards.
- **Targets: by `CombatId`.** A stable identity that does not shift when a creature
  dies, which an index into the living-creature list would.
- **Options and shop entries: by index, valid only against the observation they
  came from.** The rest-site option list *shrinks* — choosing one removes it, and
  `ShouldDisableRemainingRestSiteOptions` can clear the rest.

Card identifiers are the entry name, plus `+N` when upgraded, plus `/AFFLICTION`
when afflicted: `STRIKE`, `BASH+1`, `STRIKE/BURDEN`.
