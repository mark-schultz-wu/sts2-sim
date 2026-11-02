# sts2-core

The words that every other crate in this project speaks.

This crate says what a game of Slay the Spire 2 looks like from outside. It
gives the decisions a player can make, the steps that a run is made of, and the
shape of a build's content.

It does not play a game. There is no game in it.

## Why it exists

Two programs will play the game. One is the real game, with no window. The other
is a simulator in Rust, which does not exist yet.

Both must answer the same questions in the same way. If they do not, a
comparison between them proves nothing.

Thus both implement the traits in this crate, and neither one depends on the
other.

## What it holds

This crate holds these types:

- `Decision`. A move a player makes, such as travel, play a card, or end a turn.
- `Setup`. A way to build a position that normal play takes an hour to make.
  Not every implementation can do this.
- `Step`. A decision or a setup. One line of a recording.
- `Recording`. A run, as the steps that made it. This crate keeps it in memory
  only.
- `Seed`. One spelling of a seed. `Seed::new` refuses any other spelling, and
  `Seed::fold` is the one function that changes text, for text a person typed.
- `Observation`. Everything that is true of a run at one decision point. This is
  the value that the two implementations are compared on.
- `Digest`. Eight bytes that stand for one observation. Two implementations that
  agree get the same eight bytes.
- `Coord`, `CardIdent` and `ModelId`. The three things that the game packs into
  one string. Each one is several fields, so each one is a type here.
- `ContentVersion` and the `Table` traits. The shape that a build's content
  fills in.

## What it does not hold

This crate has no serde derives. Nothing in it knows how to write itself to a
file or to a socket. That is the work of whatever does the write.

The real game's spelling of a decision is in `sts2-headless`, beside every other
thing that the real game spells.

`serde` is still in the manifest, but only as a bound. Something must be able to
spell the ids of a build.

## Where it sits

```
sts2-core       depends on nothing
sts2-content    implements ContentVersion for one build
sts2-headless   implements the game, for real
sts2-sim        will implement the game, later
```

The plan for this crate is in `docs/STS2-CORE.md`.

## What an observation is shaped for

An observation is shaped for a reader, not for a wire.

The oracle's spelling is a compression, and a reasonable one. An observation
ships about 1.6KB per decision, at roughly 250 decisions a second. So a
coordinate is `"3,11"`, and a shop entry says `kind` beside an `id` whose
meaning depends on that kind.

This crate undoes every one of those. `sts2-headless` owns the spelling. This
crate owns the shape.

Therefore the sums are sums. A shop entry is a `Stock`, not a class name and a
string. A chest option is a `Take`, not an `id` that is sometimes `"SKIP"`. An
intent's numbers travel together, or they do not travel. The wire can spell
combinations that the game never produces. None of them survive into here.

## What is still a string

Prose is, and so are the keys it is looked up by. This means an event's title
and text, an option's localisation key, a card preview's rules text, and the
names of the variables in it.

Every name that identifies content is an id from `ContentVersion`. That goes
down to a monster's move and a rest site's options. Those two were the last
strings. Each of them is a dump now.

## Recording a run

A run is a seed and a sequence of decisions. The build is deterministic, so
replaying the decisions reproduces the run exactly: the same shuffles, the same
monster moves, the same rewards on offer. A recording holds no state. It holds
what was decided, and the game works out the rest.

A recording is only meaningful against a build. The steps name cards, encounters
and rooms, and those move between versions. A recording replayed against the
wrong build is not a broken file. It is a different run that happens to parse.
So `Recording<V>` is parameterised the way everything else here is, and there is
no version field.

A run of several players is a recording too. `Start` names each player in seat
order, and the recording says which seat took each step. A solo run is the run
of one player.

The format is in-memory only, for now. A recording outlives either
implementation, so the format is this crate's to choose. Inheriting the oracle's
wire spelling hands that choice to the oracle instead. Until this crate chooses,
the format lives in `sts2-headless`, written in terms of the wire types.

### Why the recorder is a wrapper

Recording is not a fact about an implementation. The oracle and the simulator
both produce steps and both must be recordable. A run taken from one has to
replay into the other, and that comparison is the point of the project. A
wrapper is the only place that can be true of both without either of them
carrying it.

It also puts recording where the *whole* run is visible. The daemon has its own
recorder for a different reason. A `sts2` client is a fresh process per command,
so nothing but the daemon sees two decisions in a row. An in-process driver has
no daemon and had no way to record at all: the fuzzer, a differential harness, a
simulator being exercised.

### What is recorded, and when

After the inner game answers, and only what it accepted.

Not before. A refused move did not happen, and a recording holding one replays
as a different run. It is refused again, which is noise, or, worse, accepted by
the implementation being replayed into, which takes the run somewhere the
original never went. The tape says what the run *was*.

A recording never holds an observe. It moves nothing, so replaying it only makes
a replay slower.

### A tape that fails stops, and does not fail the move

A step that cannot be written ends the recording and is reported separately. It
does not turn into an error, because the move *happened*. The game applied it,
and telling the caller otherwise leaves it believing a decision it already made
did not take place.

A file missing a step in the middle is worse than a file that stops. It replays
as a different run, with nothing about it saying so.

## Why a refusal is a type

It was a sentence. A sentence cannot be compared. Two implementations refusing
the same move for the same reason had to agree on prose first. A simulator ended
up writing English to pass a test. What makes two refusals comparable is
`PartialEq`, and that is all the type owes anybody.

The prose is a front end's. Writing it in this crate means naming a card, and
this crate does not know what a build calls anything. The answer is a fact about
the build, and the question is one a person asks.

### The four kinds, and the fifth that is not one

Every refusal the oracle can give was walked, and they sort into four.

- *Wrong moment* is a command that is fine against a run that is not there.
- *No such thing* is an index or an id that names nothing.
- *The rules say no* is the game looking and saying no.
- *Answering a question* is its own set. A parked game accepts one command, and
  every way of getting it wrong is particular to the question.

The fifth is `Reason::Malformed`, and it is the measure of what the typed
vocabulary bought. About half of the oracle's refusal sites answer a request
this crate's types cannot build. That is a missing field, a command that is not
a command, or a number where a name belongs. A `Decision` cannot spell any of
them, so a caller driving a game through a `RunTable` never sees one. Only `send_raw`
reaches them, and it exists to test exactly that.

## The random numbers

`rng` is the game's stream, ported. Slay the Spire 2 draws each card, rolls each
reward, and picks each monster move from one seeded stream. Replaying a run
therefore means producing the same numbers in the same order.

`GameRng` is the trait: an implementation supplies seeding and one raw draw, and
every gameplay method defaults to the game's own arithmetic. So a replacement
can get the core wrong, never the semantics. `Bound`, `Span` and `Interval` are
the shapes a draw asks for. A bad range fails where it is built, rather than
where it is drawn from.

Nobody trusts the port because it reads correctly. `tests/ground_truth.rs`
replays `megarandom.txt`, dumped from the game, and requires an exact match on
every draw. That test is the only reason to trust anything here.

It lived in a crate of its own until nothing outside this one used it. `Seed` is
here, and so is the machine that uses a seed. The fuzzer that once borrowed
`stream_seed` folds its own dice instead. Those dice pick between the oracle's
legal moves, and must never touch a stream that the run draws from.

## The digest

`Sink::id` hashes an id through `Hash`, which for a fieldless enum is its
discriminant. Not by name. This crate does not know what a build calls anything,
and does not need to. The question a digest asks is "is this the same value",
and a discriminant answers it.

It used to hash the name, so that a codegen renumbering a table without renaming
anything left the digest where it was. Nothing wanted that. A digest is compared
against another implementation of the *same build*. The golden files and the
scenario answers pin some, and each is rewritten from its cached transcript when
the format moves. One taken against another build is meaningless whatever it
hashed. It cost one read of the name out of a `#[serde(rename)]`. Serializing
the value through a whitelist `Serializer` did that, per id, per observation,
after every decision.

The widths are the smallest the content fits. A discriminant is two bytes and a
length is two, and a conversion that does not fit panics. Every byte written
is a byte hashed, after every decision of every run.

A counter map is walked in its keys' order. Both sides hold every stream, sorted
by the stream's own type, so the order is the same on each.

An act's map is hashed on its own, and its digest stands in for the points. The
points are settled when the act is generated, and they are the largest thing in
most observations. So the simulator digests them once and keeps the answer
beside them.

## The model id

The game's `ModelId` is a category and an entry, and it carries both because the
entry alone is ambiguous. `LOST_WISP` is an event and a relic, and `BYRDPIP` and
`PAELS_LEGION` are monsters and relics.

The build has nineteen model categories, one per direct subclass of
`AbstractModel`, and thirteen are in `ModelId` here. The six absent ones are
`ACHIEVEMENT`, `BADGE`, `CARD_POOL`, `POTION_POOL`, `RELIC_POOL` and
`SINGLETON`. `ContentVersion` names a type for none of them.

An observation carrying one of those six fails to parse. That is the outcome to
want, because it means the game reported something nothing here can hold.

Two of the six reach a fight. Every run clones both badges into its state, and
the multiplayer scaling model is a singleton. Neither can be the model that
stops a card. Only seven models in the build override `ShouldPlay`, and the hook
dump names them: six powers and Velvet Choker.

## How a build's content is described

The generated tables are the measurement of one build. Every value of a kind,
and a row of static facts for each. `Table` is that table as an interface, so a
caller holding only a `ContentVersion` can sweep the content without naming a
build.

The codegen implements these traits beside the tables it writes. An adapter
written over them afterwards is a second copy of each table's shape, and two
copies drift.

`Table::Data` is opaque here. A row's fields are version-generated content like
anything else, so a generic caller can only hand a row back to code that names
the build. What a generic caller needs instead is a method on one of the kind
traits.

An id's spelling comes from serde and nowhere else. These traits hand over
values, not names.

### What the kind traits are for

Each kind trait carries the keys a coverage measurement cuts on, and the cuts
differ by kind.

- Encounters. Eight of this build's 96 are mocks the map cannot roll, and a
  further handful belong to events. A flat count measures the wrong thing.
  Grouping by act and room type turns "61 of 96" into "every elite in the
  Overgrowth, and no boss in Glory". That says where to look next.
- Cards. Rarity says how often a card turns up at all. The pool says which runs
  can ever see it. A run is one character, so most of the coloured cards are out
  of reach of any single run however long it goes on.
- Monsters and characters. A monster in no act is one only another monster
  summons. Three characters are not runs a measurement counts itself as missing:
  the placeholder, the Deprived, and the `random` pseudo-character.
- Powers. One question, because one caller has one. A front end drawing a power
  on a creature has to know which way it points, and the game's own tables say.
  A display that guesses `SHRINK_POWER` is a buff, because it is not in some
  short list, lies about a quarter of the game.

`RelicTable::Rarity` is the one key on these traits whose bounds are a shared
vocabulary's rather than a sweep's. A reward screen reports the rarity it drew
from beside the relic it drew. It reports one beside no relic at all, where
`Populate` found nothing. So this type appears in `Question`, which two
implementations compare. Its neighbours are read by a caller that already knows
the build, and stay as narrow as their callers need.

### Flag sets

The generated sets are a word with a `contains`, and the word is the game's own
where the game has one. `UnplayableReasons` holds exactly the `[Flags]` value
`CanPlay` produced. A caller holding only a `ContentVersion` cannot reach the
inherent `new`. So `FlagSet` is the two methods that let the wire layer build a
set and a reader ask it.

`MEMBERS` is what turns `contains` into "what is in here". A generic caller
cannot enumerate the member enum. Without it a set can be asked about a member
it already holds, and about nothing else. The digest is what wanted it, because
a set has to be walked to be hashed.

The trait is deliberately not the whole surface. `iter`, `is_empty` and
`intersects` are emitted only where something calls them, and a caller with the
concrete type has all of them. Putting them on the trait forces every set to
carry every method to satisfy one bound.

### Naming the build

The generated tables are concrete types per build. Without a seam, anything that works against
content, such as the oracle client or the simulator, holds one
of them for good. `ContentVersion` is the seam. Name the build once, as a type parameter,
and every id follows from it.

Each associated type spells its bounds out. Collapsing them into a
blanket-implemented alias constrains nothing further. It also makes a missing
bound report as `V::Card: Id` rather than name the absent trait.

The supertraits are what a `#[derive]` on a generic holding these asks for.
`Command<V>` derives `Debug`, `Clone`, `Eq` and `Serialize`, and each of those
writes a bound on `V` itself even though `V` is a zero-sized marker.
