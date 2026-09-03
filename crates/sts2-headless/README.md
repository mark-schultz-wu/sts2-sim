# sts2-headless

This crate runs the real game with no window. It drives the game one decision at
a time.

This is the oracle: the program that a simulator is compared against. It is the
real game, so what it says is true.

## How it works

A mod inside the game listens on a socket. This crate starts the game and waits
for the game to connect back. Then it sends one decision and reads the answer.

```rust
let mut game: Oracle<V> = Oracle::launch(Config::new(dir))?;
let answered = game.play(&Decision::Start { seed, character, ascension, modifiers })?;
```

`Oracle` implements the two traits in `sts2-core`. `Play` sends one decision.
`Construct` builds a position that ordinary play takes an hour to reach. Those
two traits are all that a caller needs to drive a run, and a simulator will
implement the same two.

When the `Oracle` value drops, the socket closes. The game sees that and stops. A
client that dies does not leave a game process behind. There is no `quit`
command in this crate, because `Drop` sends the protocol's own.

## It refuses the wrong game

Two checks happen before you can send a decision.

**Is this the build that the tables describe?** The game reports two hashes. If
either hash disagrees with the content tables, the launch fails. Every typed id
comes from tables built against one build, and to drive another build makes each
of those ids a guess.

**Is this the mod that this client was built against?** The greeting carries a
hash of the mod's own source. The client compares that hash to its own and
refuses a mismatch.

The second check finds a fault that no other check can find: a mod that somebody
edited and never deployed again. The hashes of the game do not move when you
install a new mod. Without this check, that mistake appears as a broken reply
four hundred moves into a run. With it, you get a clear message in two seconds.

## Where the JSON lives

All of it lives here. `sts2-core` has the words, and this crate has the
spelling.

`Decision::EnterRoom` has a field named `room`. On the wire that field is named
`type`, because `type` is a Rust keyword. That is a JSON problem, so it is a
problem for this crate alone.

What the game sends back is read into `sts2_core::Applied` on the way out. The
envelope stays here. The sequence number, the command name and the digest that
the game computed are facts about the protocol, not about the run. `Oracle::
reported` gives the last envelope to a caller that wants one.

### The mirror, and why it is strict

Deriving `Deserialize` onto the domain types means hand-written `Deserialize`
impls parsing strings. That is the wire module smeared across the domain instead
of sitting in one place. The mirror also does two things a derive cannot.

Every struct is `deny_unknown_fields`, so a field the mod starts sending fails
here rather than being dropped. An observation silently missing a field is
exactly the divergence this whole arrangement exists to notice.

And the redundant fields get checked instead of ignored. The wire says
`playable` beside `reason`, and the two are the same fact. It says a chest
option's `id` beside its `relic`. Reading one and dropping the other throws away
a free consistency check on the thing being trusted.

The same strictness runs on the packed strings. A coordinate, a card identity
and a model id each arrive as one string. Each unpacker refuses input the oracle
never sends: whitespace around a number, a `+0` upgrade, a trailing separator. The mod writes them with `$"{col},{row}"` and `s += "+" + level`.
Anything else means the protocol moved. The alternative was already in the tree
once, where `moves.rs` split on a comma, trimmed, and turned every failure into
an empty move list.

A coordinate has two spellings, and that is fine. The observation packs one into
`"3,11"`, and the `travel` command sends `{"col": 3, "row": 11}`. Both are the
oracle's. Neither had to be talked into agreeing with the other, which is the
whole reason the domain type carries no spelling of its own.

Every failure carries the path it came from. `combat.creatures[2].model` is
actionable, and "a model id did not parse" is not.

The envelope is strict for a reason this crate learned once. `Response::Pending`
carried `digest` and `state` on the wire with no fields to receive them, so
serde discarded both on every pending.

## Tests

`tests/live.rs` launches the real game. Each test costs a process and about
three seconds of startup, so every test there is `#[ignore]`d and `just live`
runs them.

The tests that drive a run are `#[serial(game)]`. Every game process shares one
profile directory, and a driven run writes to `saves/progress.save` inside it.
`RelicCmd.Obtain` calls `SaveManager.MarkRelicAsSeen`, and fighting an enemy
records a discovery. Two runs at once race on that file. D1 treats profile
residue as a determinism concern rather than a tidiness one, so it is not
something to race on and hope.

The attribute puts that constraint on the tests that have it, rather than on
whoever invokes cargo. Anything outside the `game` group still runs in parallel:
the unit tests, and the handshake tests that only bind loopback ports. So
`cargo test --include-ignored` is safe with no extra flags.

### What only a live test can check

A unit test beside a parser checks a shape this repository wrote down. It holds
the parser to how the mod's format strings were *read*. A live test holds it to
what the game sends, and that is the only place the two can be caught
disagreeing. A field that turned out to be nullable. An affliction in a shape
nobody wrote a test for. A category `ContentVersion` has no type for.

Every wire struct is `deny_unknown_fields`. So a key the mod adds and the client
does not know about fails a live test, rather than being dropped.

The rooms and the piles are walked deliberately rather than played into. Each
room carries fields no other room has: a shop's `entries`, a chest's `SKIP`, an
event's prose, a fight's `legal` and `creatures`. A run that happened not to
enter a shop leaves that branch of the conversion untested.

Every failure names its path. "A coordinate did not parse" is not actionable,
and "map.points[41].to[0] was `4,-1`" is.

### The deadlock class

Four bugs there have had one shape, and two of the live tests exist to hold it
shut.

A handler awaits a game call. That call parks a question for the client. The
serve loop cannot read the client's answer until the handler returns. So the
handler waits for something only its own return produces, and nothing replies.

Only a live test catches it. The C# selftest calls `Driver` directly and never
touches `Protocol`, so it exercises the same options happily while the protocol
path is wedged. That is what happened: 201 selftest assertions passed against a
`choose_option` that never replied.

Two of the four, as the tests name them:

- The rest site. `choose_option` awaited `sync.ChooseLocalOption(index)`. For
  SMITH that awaits `CardSelectCmd.FromDeckForUpgrade`. Measured before the fix:
  no reply, and the client timed out at 20s.
- The shop. `buy` awaited `MerchantEntry.OnTryPurchaseWrapper`. For the card
  removal entry that awaits `DoMerchantCardRemoval`, and then
  `CardSelectCmd.FromDeckForRemoval`. Measured before the fix: `buy 13` never
  replied, 72 moves into a random run. The fuzzer found this one. A person found
  the other three.

The option that resolves by itself is never the case under test. HEAL, and a
card, a relic or a potion in a shop, all pass against the broken code and prove
nothing.

## Recording files

A run is a seed and a list of decisions, so a recording holds no state.

A recording is the one artifact meant to outlive either implementation. The same
file replayed into the oracle and into the simulator is what "the same run"
means. So the *format* is `sts2-core`'s to choose, and it has not chosen one
yet. Until it does, the file is written in the protocol's own shape. That is
what every recording on disk already holds, and inventing a second format before
the first is designed strands them.

The format is JSON Lines: a build stamp, then one line per step.

```text
{"game":"v0.107.1","commit":"59260271","assemblyHash":1692500715,"contentHash":3954186980}
{"cmd":"start","seed":"SP1RE1","character":"IRONCLAD","ascension":"None"}
{"cmd":"travel","col":1,"row":1}
{"cmd":"play_card","index":3,"target":1}
```

One line per step rather than one document, because a recording is written while
the run is still being played. A session that is killed, times out or crashes
leaves a file complete up to its last decision, and it replays fine. A
half-written JSON array leaves nothing.

## Reaching a game somebody else launched

`Session` is the second implementation of `Play` in this crate, with the same
game behind it. `Oracle` owns a child process and dies with it. A `Session`
reaches one that a daemon is holding, so `sts2 play-card 0` can be a whole
process and still land in a run forty decisions deep.

A front end holds that type and not a socket, because the socket is a fact about
the oracle. The game boots in seconds and keeps every bit of run state, so it
has to outlive one command. A simulator is a struct in the caller's own address
space and outlives nothing. A front end written against `Play` does not have to
know which of those it has, and that is the whole point of the trait.

One connection per command, not one held open. The daemon serves one client at a
time, because the game is a single state machine. A held connection is a client
with no command in flight, keeping every other client out. Connecting costs a
syscall, and the command that follows costs a decision in a game engine.

The socket carries the build: `sts2-v0_107_1.sock` beside `sts2-v0_111_0.sock`.
The daemon relays a command the client already encoded against its own tables.
A client reaching a daemon that holds another build reads that game through the
wrong ids. Separate paths make that unreachable, and let two builds be held open
at once.
