# The sts2-core split

Pulling the shared vocabulary out of `sts2-headless` into a crate that both the
real game and the future simulator implement. The oracle then stops being a
transport with a front end bolted to it, and becomes one of two interchangeable
ways to play.

All of it landed. This file is the reference that the work followed.
It reverses two things [ARCHITECTURE.md](ARCHITECTURE.md) states, listed under
*Doc reversals* below. It also reverses several things it said itself, each
marked *reversed in the doing* where it stands.

## Why

The case for the split, as it stood before any of it was built. Kept in the
present tense it was written in, because it is the argument and not the status.
Each section below says what landed.

`sts2-headless` calls itself "the transport and nothing above it", and the
transport half is in good shape. It has launch, token, greeting, build check,
and one typed request per call, tested at the seams. The problem is what it
refuses to own.

**The observation is untyped.** `Response::Observation` carries
`state: serde_json::Value`, on the grounds that "the right shape depends on what
the simulator turns out to need". That reason expired on three counts:

* The shape is not unknown. `mod/sts2_headless/Observation.cs` is 778 lines of
  fixed, documented schema.
* Four consumers re-derive it independently. `sts2-cli` grew a `Field` trait,
  `str_at`/`int_at`/`list_at`/`is_true`/`is_false`, created precisely to stop
  `render` and `cli` disagreeing about an absent key. That is a workaround for a
  missing type. It is used for *decisions*: `cli::moves` picks the fuzzer's next
  move out of string-keyed lookups that return `None` on a typo.
* Every id in it is a `String`, in a project whose thesis is that a string id is
  a guess. `coverage.rs` already keeps an `unlisted` bucket for ids that are not
  in the tables, which is a parse failure discovered three layers late.

**The digest can only ever compare the game to itself.** A digest is SHA-256
over a JSON string built in C#: dictionary insertion order, .NET number
formatting, and every absent-vs-null decision in `Observation.cs`. A Rust
simulator cannot reproduce it short of reimplementing .NET's serializer. So
every comparison the crate can do today is game-against-game. That is the same
seed twice, the same encounter twice, or a recording replayed. `just live` says so outright: *"a
second game standing in for the simulator that does not exist yet."* That works
precisely because both sides are the game.

The crate is built for the test it can run today, not the test it exists for.

**Two things are parked here for want of a home.** One is `Recording`, which
ARCHITECTURE.md admits with *"there is no other candidate"*. The other is
`send_raw`/`send_line`/`parse_reply`/`greeting()`, plumbing for the `sts2-cli`
daemon relay wearing the oracle's clothes.

## The shape

Two type parameters, not one: which build, and which implementation. The first
is `ContentVersion`. The second needs a crate below both implementations.

```
sts2-content ──→ sts2-core ─┬─→ sts2-headless   the real game, over a socket
                           ├─→ sts2-sim        the Rust engine and RNG
                           └─→ sts2-cli        front end, generic over both
```

Not `sts2-sim-core`. That crate holds the engine's insides: the mutable
`CombatState`, and the hook trait content calls back into. Those describe how an
engine is built. `sts2-core` describes what any game answers to. `sts2-headless`
is an implementation of the second and has no business depending on
`Hook::ModifyDamage`.

The distinction that makes the split work: **`sts2-core` owns the observation (a
snapshot), not the state (a machine)**. The simulator's `CombatState` is
mutable, and holds RNG streams and an action queue. It *derives* an observation,
exactly as `Observation.cs` derives one from `RunState`. Snapshots are
comparable, states are not.

`Observation<V>` is a concrete struct, not a trait. If each implementation
brought its own observation type, comparison means nothing.

## The traits

**Superseded** on 2026-10-29 by `RunTable`, one trait for every table: the
simulator's, the game's, a lobby of games and the daemon's.
[RUNNERS.md](RUNNERS.md) says why. `Play`, `Construct`, `Seat`, `Coop` and
`Recorder` are gone. Each implementation keeps `begin`, `play`, `observe` and
`setup` as its own methods, and a run table over it is what generic code takes.
What follows is the reasoning for the split at the time.

Split so the real game cannot pick up methods that are not reflective of a real
game. The simulator, in turn, cannot be forced to fake ones it has not earned.

```rust
/// Playing. Every implementation, and nothing beyond what a run needs.
pub trait Play<V: ContentVersion> {
    type Error;
    fn play(&mut self, decision: &Decision<V>)
        -> Result<Result<Applied<V>, Refusal<V>>, Self::Error>;

    /// Where the run is standing, without moving it.
    fn observe(&mut self) -> Result<Result<Applied<V>, Refusal<V>>, Self::Error>;
}

/// Reaching a position ordinary play would take an hour to arrive at.
/// The oracle has this because the mod does. The sim gets it when it can.
pub trait Construct<V: ContentVersion>: Play<V> {
    fn setup(&mut self, step: &Setup<V>)
        -> Result<Result<Applied<V>, Refusal<V>>, Self::Error>;
}
```

The outer `Result` is whether the exchange happened. The inner is whether the
game accepted it. That is the existing `Reply` alias, kept.

A refusal is not an error. It is a legitimate, deterministic answer both
implementations must give. The fuzzer's headline finding is "the game refused a
move the oracle offered", which compares refusals against reported legality. A
*fault* is an error, and lives in the implementation's own `Self::Error`.
`sts2_headless::Error` gains a `Faulted` variant and stays as precise as the
ten-variant enum already there.

Three methods failed the test:

| method | verdict |
|---|---|
| `observe` | ***Reversed in the doing.*** The argument held only for a caller that holds the last `Applied`. A front end is one process per command, so the last answer went out with the last process. Without `observe` on the trait, such a caller must reach past the trait for its first look. One implementation then holds it from that line on. It is on `Play`, and every implementation can answer it: a simulator holds an observation, and the oracle asks the game for one. |
| `quit` | `Drop`. |
| `build()`/`stamp()` | `V::VERSION` already says it, statically. |

`Start` stays a `Decision` variant rather than becoming a constructor. It costs
a little honesty, because it is not a move within a run. It buys uniform
recordings and a one-method `Play` trait, which the dispatch enum also wants.

## The vocabulary

`Command<V>` splits in two and loses two variants. What is left is a decision
vocabulary, not a protocol. Nothing about it is socket-shaped except the serde
attributes.

| type | holds | notes |
|---|---|---|
| `Decision<V>` | `Start`, `Travel`, `EnterRoom`, `Proceed`, `PlayCard`, `EndTurn`, `ChooseOption`, `Buy`, `Answer` | What a player does. `Observe` and `Quit` are gone. |
| `Setup<V>` | `Fight`, `Grant(Gift)` | Scenario construction. |
| `Step<V>` | `Decision` \| `Setup` | What a `Recording` holds, so scenario runs stay recordable. Replaying one into a `Play`-only implementation fails loudly at the step needing `Construct`. That is correct, not a silent skip. |
| `Applied<V>` | `observation`, `asking: Option<Question<V>>`, `resolved: bool` | Replaces `Response`. `seq`, `cmd`, `digest` and `Bye` do not survive. |
| `Observation<V>` | the whole state, typed | Mirrors `Observation.cs`. The long pole. |
| `Refusal<V>` | `what`, `reason: Reason<V>` | A legitimate answer, not an error. `Reason` is the sum, 28 variants. See *Typed refusals*. |
| `Question<V>` | `SelectCards`, `Rewards`, `Divine` | Core semantics: both implementations stop mid-action to ask. `Divine` is the Crystal Sphere, the one event with a screen of its own. |
| `Recording<V>` | stamp, then one `Step` per line | Moves here. A list of decisions is implementation-independent by construction, so replaying one into either implementation is the same fold. That fold *is* the divergence test. |

`Decision<V>` deliberately does not carry a player id. It describes a choice,
which is also what a single-player caller supplies. The simulator wraps it in
an ordered action with an explicit actor before mutation. A future multiplayer
adapter gets that actor and order from the retail host. Local singleplayer uses
its sole player and local sequencer. The network envelope does not belong in
every front-end decision. An actor also cannot remain implicit inside engine
state, because that makes multiplayer a rewrite.

## Typed refusals

All forty `Error(...)` sites in `Protocol.cs` were walked. **About half cannot
occur through a typed API.** Those are `request has no 'cmd' string`, `unknown
command`, `malformed JSON`, `choose_option requires an integer 'index'`, and
`'target' must be a creature id`. They are JSON-shaped. That is a measurable
answer to what the typed vocabulary buys: it deletes half the refusal surface.

Two more went when the domain types stopped being able to spell them. A reward
being taken is `TakeReward::Card`/`NonCard` rather than an index with an
optional card. A grant is `Gift::Card`/`Relic` rather than two `Option`s. So
`take requires a 'card' index` and `grant requires 'card' or 'relic'` have no
caller left. Both were the same defect: a sum wearing a struct.

The survivors are four kinds, and a simulator will have all four. As built, 26
variants. The plan said three kinds and undercounted. *Answering a question* is
its own set. A parked game accepts one command, and every way of getting it
wrong is particular to the question.

| kind | variants |
|---|---|
| wrong moment | `NoRun`, `RunOver { victory }`, `NotInCombat`, `InCombat`, `WrongRoom { wanted, found }`, `NothingToChoose { found }`, `Pending { kind }`, `NothingPending` |
| no such thing | `OutOfRange { index, count }`, `NoSuchCreature { id }` |
| rules say no | `Unplayable { card, why, preventer }`, `WrongTarget { card, target, takes }`, `NotTravelable { at }`, `CannotProceed`, `OptionDisabled`, `OptionLocked`, `AlreadyAnswered`, `Unpurchasable { index, cost, gold, stocked, blocked }` |
| answering a question | `Duplicate { index }`, `WrongCount { chosen, min, max }`, `NotSkippable`, `AlreadyTaken { index }`, `NeedsCardChoice { index, count }`, `EmptyTake`, `OffGrid { x, y, width, height }`, `AlreadyUncovered { x, y }` |

Plus the two that carry a string on purpose, below: `Console` and `Malformed`.

Consequences:

* **The mod changes.** It emits a tagged reason rather than interpolated prose
  at ~20 call sites.
* **The prose survives, it moves**, twice. It went from C# string
  interpolation to a `Display` on `Reason`, and then out of `sts2-core`
  altogether. That is *reversed in the doing*. Writing *"BASH is not playable"*
  means knowing what a build calls a card, and this crate does not know it. It
  lives in `sts2-cli`'s `render::refusal`, still generated from the fields
  rather than parsed out of a message. That is the property the typed reason
  bought. What makes two refusals comparable is `PartialEq`, and that is all
  `Reason` owes anybody.
* **No separate wire type here**, unlike observations. *Reversed in the doing.*
  `sts2-core` has no serde derives at all, so there was nothing for the mod to
  emit the serde form *of*. And `Reason<V>` names a card, a room type and a
  model id. Every one of those arrives packed, and has to be read against the
  build's tables. `wire::refusal` is that reader. It carries the dotted path a
  failure came from the way `wire::observation` does: `reason.card` is
  actionable and "a card identity did not parse" is not.
* **Each variant describes itself**, even where the observation duplicates it.
  `Unplayable` keeps `why` and `preventer` although `legal[].reason` has them,
  because the observation in hand is the *pre-command* one. A refusal whose
  reason disagrees with what the last observation predicted is precisely the
  disagreement worth catching.

Two strings survive. `Driver.EnterFight` returns `FightConsoleCmd`'s own
message, deliberately: *"it knows why better than we do."* That is MegaCrit's
prose, and typing it means classifying their console output. `Malformed` carries
whatever the mod said about a request that was not a request. There is nothing
to compare, because a client sending one was not speaking the vocabulary in the
first place.

Both are `Reason` variants rather than a separate `ConstructRefused` the plan
imagined. `Console` reaches `Play` too. `EventConsoleCmd` declines the same way
`FightConsoleCmd` does, so quarantining it on the `Construct` half is a split
the game does not have.

## The wire layer

**The mod's spelling stays inside the adapter.** Private wire types in
`sts2-headless`, converted at the boundary into the domain types.

The temptation is to derive `Deserialize` straight onto `Observation<V>` and
skip a layer. That does not survive contact with what the mod sends. Every one
of these is a *compression*, chosen because the observation ships ~1.6KB per
decision at ~250 decisions/sec:

| wire | what it is | domain |
|---|---|---|
| `"1,2"` | a map coordinate | `Coord { col, row }` |
| `"STRIKE_IRONCLAD+1/SMOG*ADROIT"` | card id + upgrade + affliction + enchantment | `CardIdent<V>`, four typed fields |
| `"MONSTER/BYRDPIP"` | a model id whose category is ambiguous | `ModelId<V>`, an enum over categories |
| `{"Shuffle":44,"player.CardDraw":3}` | two RNG enums flattened, split by a `"player."` prefix | two typed counter maps |
| `"EnergyCostTooHigh, NoLivingAllies"` | a `[Flags]` enum, `ToString()`-ed | `V::UnplayableReasons`, one word |
| `kind: "MerchantRelicEntry"` beside `id` | a class name that says how to read a sibling | `Stock<V>`, a sum |
| `id: "SKIP"`, `skip: true`, `relic: null` | three keys for one choice | `Take<V>`, a sum |

Good decisions for a wire format, terrible types to build a simulator against.
Deriving `Deserialize` on the domain types means five hand-written `Deserialize`
impls parsing strings. That is a wire layer anyway, smeared invisibly across the
domain types instead of sitting in one file.

The drift objection does not apply. Drift needs two hand-maintained types
converging on one goal by hope. Here the wire types carry `deny_unknown_fields`
against the real mod. The conversion is total, and a domain field with no wire
source is a compile error. What it *does* need is a test against what the game
really sends, rather than against strings this repository wrote down.

It has one. `every_packed_string_in_a_real_observation_parses` drives a run,
enters a fight, and parses every coordinate, card identity and model id in both
observations. `every_room_parses_into_an_observation` walks a rest site, a shop,
a chest, an event and a fight. It converts each whole observation, and names the
path of anything that fails. Both are live runs rather than a fixture corpus,
which is the stronger version of the same check. A fixture is a recording of one
build, and these ask the build in the game directory.

`deny_unknown_fields` on every wire struct is what makes that wide rather than
shallow. A key the mod starts sending fails here instead of being dropped. An
observation quietly missing a field is exactly the divergence nothing else
notices.

`RewardDetail` is today's example of a wire shape that is a sum wearing a
struct's clothes. There the private wire type plus `TryFrom` pattern already in
the crate stays as is.

H10 carries over. A field an implementation cannot fill is `None`, never
defaulted, and the comparison must report `None`-vs-value as a divergence rather
than skipping it. Otherwise the simulator passes by not knowing things.

## Version-derived leaves

The refusal *variants* track the protocol. The *values inside them* track the
content version. Where the mod chooses to refuse is an authored decision, and
what a refusal says is the game's own vocabulary.

**Actually derived**, meaning dumped, generated, and reached as associated
types:

* `UnplayableReason` is a real `[Flags]` enum in the assembly. It is dumped and
  generated at `v0_107_1/generated/runstate`, numbered by bit index so the set
  over it holds the game's own word.
* Generation already covers `TargetType`, at `v0_107_1/generated/cards`.
* Generation already covers `RoomType`, and already as an associated type.

**Authored**, staying in `sts2-core`: the variant set itself. Nothing in the
game dumps "the ways a command can be refused". Making it a `V::Refusal`
associated type also inverts the dependency. `sts2-core` depends on
`sts2-content`, so `Refused`'s `Coord`, `CombatId` and `QuestionKind` have to
move into `sts2-content`, where they do not belong.

The failure mode favours this side too. A refusal variant no build can emit is
inert. A spurious `Card` variant instead corrupts pool sampling, which is the
reason `ContentVersion` exists at all.

`TargetType` used to be a standing violation of the name-the-build-once rule.
It was generated, but reached as `v0_107_1::generated::cards::TargetType` rather
than through the parameter. It and eight others are `ContentVersion` associated
types now, which is what let `Observation<V>` be written at all.

`UnplayableReasons` is the odd one, because it is a generated *set* rather than
an enum. A caller holding only a `ContentVersion` cannot reach its inherent
`new`. That is what `FlagSet` is for: two methods, `from_members` and
`contains`, emitted beside every plain set by the same codegen that emits the
set. Not the whole surface. `iter`, `is_empty` and `intersects` exist only where
something calls them. Putting them in the trait forces every set to carry every
method to satisfy one bound.

## Moving the digest

**Done.** The digest is defined in Rust, over the typed observation, canonically:

```rust
impl<V: ContentVersion> Observation<V> {
    pub fn digest(&self) -> Digest;
}
```

The fast path compares eight bytes. The slow path diffs the struct and names
the field.
Same throughput win as today, and it works against a simulator. It also cannot
be perturbed by serde attributes some C# file chose.

The encoding is `sts2-core`'s own, written out by hand, and not a serde format.
A serde format was the C# digest's mistake in another language. Every integer is
little-endian and fixed-width, every sequence carries its length, and every sum
carries a tag the crate chose. The hash underneath is xxHash64, held against the
reference vectors.

An id was the exception and is not any more. *Reversed in the doing, twice.* It
was hashed by name, spelled by serde, so that a codegen renumbering a table left
the digest where it was. Nothing wanted that property. No digest is persisted,
no test pins one, and a digest taken against another build is meaningless
whatever it hashed. Both sides of a comparison are the same `V` compiled into
the same process. It cost one read of the name out of a `#[serde(rename)]`,
by serializing the value through a 175-line whitelist `Serializer`. That ran per
id, per observation, after every decision.

So an id is hashed through `std::hash::Hash`, which this section used to reject
on the grounds that its byte stream is rustc's business. It is rustc's business,
and that is fine for the same reason the renumbering property was worthless. For
a fieldless enum the derive hashes the discriminant, which is all `sts2-core`
can see of an id and all a digest needs. `Sink` implements `Hasher` so a value
writes itself into the same little-endian encoding as everything else, rather
than into `Hasher`'s native-endian defaults.

Every walk destructures its struct exhaustively, so a field added to
`Observation` and not to the digest does not compile. `tests/digest.rs` nudges
each field in turn and requires a digest nothing else in the sweep produced.
That is what proves that the code then used the destructured field.

The mod's C# digest stays, demoted to an internal determinism check for the mod.
That is all it was ever able to be. `just live` asserts the two agree about
whether two runs are the same run. The game calling two runs different while
this crate calls them the same is an observation missing a field.

## What is left of sts2-headless

**Done.**

```
Oracle<V>     plays, observes and sets up; owns a child process; OracleTable
Session<V>    the same; reaches one a daemon holds; HeldTable
session/      process, handshake, build check, mod-identity check, the daemon
stamp.rs      build identity
wire/         the mod's JSON forms, and reading them into sts2-core types
```

One sentence: *launch the real game and make it a run table.*

Two implementations of it, because a run has to outlive one `sts2 act play-card 0`.
That is a fact about *this* implementation and not about playing. The game boots
in seconds and holds every bit of run state. A simulator is a struct in the
caller's own address space, and outlives nothing. So the daemon, the unix socket
and the request envelope live behind `Session<V>` rather than in front of it. A
front end written against `Play` never learns any of it is there. Nothing
in `sts2-core` has to serialize for that. What crosses the socket is
`Command<V>` and a reply line, which are this crate's own wire types.

Four things survive as inherent methods on `Oracle`, and each is off-trait for a
reason a simulator makes obvious. `greeting` is what the mod said about itself.
`logged` is what the game's own engine wrote while running a command, and a
simulator has no such log. `reported` is the last reply's envelope, which exists
so `just live` can hold the game's digest against this crate's. `send_raw` sends
a command the typed API cannot spell, which is how a refusal gets tested at all.

`send_line` is the daemon's relay, and the daemon is in this crate now, so it is
no longer plumbing wearing the oracle's clothes. `Recording` and the whole
command vocabulary already left.

`socket_path` takes the build, so there is one daemon for each. The relay hands
on a command the client encoded against its own tables. A client that reached
the wrong daemon reads that game through the wrong ids. The path is where that
is made unreachable.

## Dispatch in sts2-cli

Not written, and the reason changed. There are two implementations now,
`Oracle` and `Session`. They are the same game reached two ways, and a front end
wants one of them at a time rather than a choice between them. So `sts2-cli`
names its implementation once, in a type alias, and everything below that line
goes through `Construct`:

```rust
type Game<V> = Session<V>;    // which implementation
```

Pointing the binary at a simulator is that line.

The *build* is no longer an alias beside it. A person at a terminal has to be
able to say which game they mean. So `main` turns `--game-version` into a type
in one `match`, and calls a generic `drive<V>`. Everything under it is generic
over `V`, and a new build adds one arm. The enum earns its place
when a caller has to hold *both at once*. That caller is the differential test,
driving a decision into each and comparing what came back, and it is the next
thing to be written. What it will be then:

Two known implementations, so an enum: no `dyn`, no erasure pushed down into the
interface. **The enum lives in `sts2-cli`, never in `sts2-core`.** The interface
crate has no business knowing the list of its implementations.

```rust
enum AnyGame<V: ContentVersion> { Oracle(Oracle<V>), Sim(Sim<V>) }

impl<V: ContentVersion> Play<V> for AnyGame<V> {
    type Error = anyhow::Error;  // both flatten at the front end's boundary
    fn play(&mut self, d: &Decision<V>) -> ... {
        match self {
            Self::Oracle(g) => g.play(d).map_err(Into::into),
            Self::Sim(g)    => g.play(d).map_err(Into::into),
        }
    }
}
```

Multiple *versions* is a harder problem than multiple implementations, and is
deferred until there is a second build. `AnyGame<V0_107_1>` and
`AnyGame<V0_108_0>` are different types, and `V` appears in the return types. So
that enum goes at the outermost layer, with the command loop monomorphized per
arm. It is cheap to add later, and not worth designing against with one build
dumped.

## Rejected designs

Kept for the reason ARCHITECTURE.md keeps its own: a rejected design is what
stops the same design being proposed again. Each of these arrived and
died during this work.

**The mod's JSON as the canonical encoding.** It commits the simulator's
vocabulary to five compressions chosen for throughput. See *The wire layer*.

**A bespoke `Reply` enum.** `Moved { … } | Refused(Refusal)` is
`Result<Applied, Refused>` in a costume. The existing `Reply` alias already
spells it correctly.

**A shared `Broken` error enum.** It replaces `sts2_headless::Error`, ten
precise variants that `sts2-cli` turns into readable messages, with
`Box<dyn Error>`, to save one type parameter. A strict downgrade.

**`game_errors` on the shared outcome.** `Vec<String>` is `Log.LogCallback`'s
shape. That is the mod's spelling in the crate that keeps no mod spellings, and
the same mistake as `state: Value` one layer down. It also forces a simulator to
`format!()` a structured internal error into C# log lines. Faults are
per-implementation, so they live in each implementation's error type.

**Stringly-typed refusals.** A free-text `message` cannot be compared between
implementations without the simulator reproducing the game's English.

**`V::Refusal` as an associated type.** Inverts the crate dependency. See
*Version-derived leaves*.

**A protocol versioning scheme.** Superseded by mod identity, which is derived
rather than remembered. Landed: see phase 0.

**A `Named` trait in `sts2-core`.** Written, and abandoned the same day. The
complaint it answered was real. A build's id lived only in a `#[serde(rename)]`,
so reading it back meant serializing the value. The answer was to move the id
onto the type as data and generate serde from it. That fixed the mechanism and
kept the mistake, because naming content is not `sts2-core`'s job at all.
`sts2-headless` needs `V::Card: Serialize + DeserializeOwned` and nothing more.
Serde *is* the wire's serialization library, and that bound sits in
`ContentVersion` only because Rust puts a bound where the associated type is
declared. Core declares it and never calls it. Naming is a front end's job and
lives in `sts2-cli`'s `render`.

## Doc reversals

ARCHITECTURE.md states two things this reverses.

> [Recording] lives in `sts2-headless` because a step is a `Command<V>` and
> there is no other candidate.

It moves to `sts2-core`, which is now that candidate. The original rejection
still stands, and is *not* reversed. That rejection was of putting it in
`sts2-content`, generic over an abstract step. A `Step<V>` with two real
implementations is not an abstraction with one caller.

> The oracle. `sts2-headless` drives the real game one decision at a time and
> compares digests.

It compares *observations*, with a Rust-side digest as the fast path. The C#
digest was never able to be compared against anything but another copy of the
game.

## Order of work

Sequential. Each phase is unblocked by the one before, and the corpus in phase 2
is what makes phase 3 checkable rather than hopeful.

**0. Mod identity, done.** The greeting carries a hash of the mod's own source
(`identity.sh`, run by both builds), and the client refuses a mismatch at the
handshake. `PROTOCOL_VERSION` deleted. Verified live in both directions: a
non-redeployed edit fails in 1.5s with `WrongMod` instead of a decode error deep
in a run.

**1. Crate skeleton and the command split, done.** `sts2-core` holds
`Decision`/`Setup`/`Step`/`Answer`/`Recording`, with **no serde at all**.
`Observation` stays a `Value` for now, so this landed without touching the
payload.

That last part is a correction to what this document originally proposed. The
plan had the vocabulary carrying the protocol's serde attributes, on the
grounds that `Command` already serialized to the wire form. It does not survive
the split. `#[serde(tag = "cmd")]` and `EnterRoom`'s `room` → `type` rename are
the mod's spelling, in the crate that is supposed to keep none. So the domain
types carry no serialization, and `sts2-headless::Command` is the protocol's
shape with `From<&Decision<V>>` and `into_step()` across the boundary. The
compiler enforces it: a domain type physically cannot reach the socket.

`Recording`'s *file format* went with the wire types into `sts2-headless` rather
than staying with the type. A recording is the artifact meant to outlive either
implementation, so its format is `sts2-core`'s to choose, and it has not chosen
one. Inheriting the oracle's spelling by default is how it ends up belonging to
the oracle. In-memory `Recording<V>` is in `sts2-core`, and
`sts2_headless::recording::{open, append, read, write}` is the format, to be
redesigned deliberately.

**2. Capture a corpus, done differently.** The fuzzer reads every state it
walks past into an `Observation<V>`. A batch then puts the mirror under load,
with no corpus of files to keep. Fixtures are hand-written and completed by
`sts2-cli`'s `fixture` module, which is what catches one that is not a shape the
game sends.

The original plan: a fuzz batch across seeds and playable characters,
dumping every raw observation. Keep a handful as fixtures. Doubles as the
coverage batch `coverage.rs` was already heading toward.

**3. Type the observation, done.** `Observation<V>` plus private wire types in
the adapter, validated against every captured sample. Dump `UnplayableReason`,
and add `V::TargetType` and `V::UnplayableReason`. Delete `sts2-cli`'s `Field`
trait and the string-keyed move selection.

**4. The Rust digest and the traits, done.** `Observation::digest`, `just live`'s
digest assertions, `Play` and `Construct`, and `Oracle<V>` as the first
implementation. `Applied<V>`, `Question<V>` and `Refusal` came with the traits,
because a trait signature needs types to name.

**5. Typed refusals, done.** `Refusal<V>` and `Reason<V>`, 26 variants in four
kinds where the plan named three, plus `Console` and `Malformed`. `Refusals.cs`
on the mod side, and every refusal site rewritten. `{"type":"fault"}` is its own
message, so an adapter crash stops being filed as a game decision. The prose is
written from the data, in `sts2-cli` rather than here. See phase 6.

Three refusal sites threw where a refusal was the right
answer. Those are `grant` with a name the game does not have, and `start` with
an ascension or a modifier it does not have. The catch-all flattened
them into refusals. That is exactly the conflation the fault type exists to end,
so separating the two is what made them visible.

**6. The front end over the trait, done.** Not in the original list, because
this document stopped at the crate boundary and assumed a front end follows for
free. It did not: `sts2-cli` still spoke the protocol.

Two halves. *The socket left.* The daemon, the client and the request envelope
moved into `sts2-headless` behind `Session<V>`. `sts2-cli/src/session` is gone,
and what survives of it is `runs_dir`, which was never about the socket. *The
renderer stopped reading JSON.* `render/state.rs`, `combat.rs` and `map.rs`,
about seven hundred lines, walked `serde_json::Value` by key. A simulator
handing over an `Observation<V>` had nothing to print. They are written against
the shared types now. `Seen` had nothing left to hold and is gone, and the
`Field` trait that existed only because the renderer had no types went with it.
`render`, `cli` and `coverage` name nothing from `sts2-headless`.

Typing them turned three hand-rolled spellings back into the domain's. A shop
entry's kind was recovered by stripping `Merchant` and `Entry` off a C# class
name, and it is a match on `Stock` now. The orb queue was printed by
`Display`ing raw JSON. A creature's model came off a packed id by `rsplit('/')`.

And then the naming left `sts2-core`, which is the part worth remembering. See
*Moving the digest* and *A `Named` trait in `sts2-core`* under *Rejected
designs*. The crate declares the vocabulary, and does not know what a build
calls anything.

## Open questions

* **The slow path.** `Observation<V>` derives `PartialEq`, and the digest is the
  fast path over it. What is still missing is the `diff` that names the first
  divergent field. Equality says two runs parted ways, and nothing says where.
  The digest's walk is the shape that `diff` wants, so one description had better
  drive both, rather than two.
* **Nothing has a display name.** `CardData` is `{kind, rarity, target, cost,
  max_upgrade, keywords}`, and no table carries what a person calls the thing.
  So `sts2-cli` shows `STRIKE_IRONCLAD`, not because an id is the right thing to
  show but because it is the only string in the building. The game has them, as
  `LocString`, which the mod already resolves for event prose. No dump takes
  them.
* **Five untouched content kinds.** Affliction, AscensionLevel, Enchantment,
  Modifier and Orb still have inherent impls rather than `Table` impls.
  Mechanical. Convert when something needs them through `ContentVersion`.
  `Observation<V>` probably will, for orbs and afflictions.
