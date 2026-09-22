//! Eight bytes standing for a whole observation.
//!
//! The fast path of the differential test. Two implementations produce an
//! [`Observation`] each after the same decision. Comparing the two structures
//! field by field says *where* they diverged. Comparing eight bytes says
//! *whether* they did, cheaply enough to do it after every decision of every
//! run.
//!
//! # Why the digest moved out of the game
//!
//! The oracle already sends one, and it can only ever compare the game to
//! itself. It is SHA-256 over a JSON string built in C#. So it carries
//! dictionary insertion order, .NET number formatting, and every
//! absent-versus-null decision `Observation.cs` happened to make. A simulator
//! cannot reproduce that short of reimplementing .NET's serializer, which is
//! why every comparison this workspace can run today is game-against-game.
//!
//! This one is defined over the shared type, so both implementations compute it
//! from the same thing and agree by construction. The mod's digest stays where
//! it is, demoted to a determinism check for the mod. That is all it was ever
//! able to be.
//!
//! # What the digest is over
//!
//! The encoding is this module's, written out by hand, and deliberately not
//! borrowed from a serde format. A serde format was the C# digest's mistake in
//! another language. Every integer is little-endian and fixed-width, every
//! sequence carries its length, and every sum carries a tag this file chose.
//!
//! An id goes through [`std::hash::Hash`], which for a fieldless enum is its
//! discriminant. [`Sink`] implements [`std::hash::Hasher`], so a value writes
//! itself into this same encoding rather than into the native-endian defaults.
//! See [`Sink::id`] for why the name is not what is hashed.
//!
//! # Adding a field
//!
//! Every walk below destructures its struct exhaustively. A field added to
//! [`Observation`] and not to the digest is then a compile error, rather than a
//! divergence the digest silently cannot see. That is the whole reason the
//! walks are written out instead of derived.
//!
//! # The one neighbour
//!
//! What is left in this file is the walk and nothing else. `xxhash` is the
//! function the bytes go into, and it is its own module because it is not about
//! hashing an observation.

mod section;
mod xxhash;

pub use section::Section;

use std::borrow::Borrow;
use std::hash::Hash;

use num_traits::ToPrimitive;
use xxhash::XxHash64;

use crate::{
    Attack, Blocked, CardIdent, CardVars, Carried, Channelled, ChestOption, ContentVersion, Coord,
    Cost, Entry, EventOption, EventOptionKey, EventOptionKinds, FlagSet, Held, Intent, Keyed,
    ModelId, NextMove, Node, Observed, ObservedCreature, ObservedFight, ObservedPlayable, Orbs,
    PowerAmount, Preview, Reachable, RestOption, Room, Stack, Stock, Take,
};

/// One observation, as eight bytes.
///
/// Sixty-four bits is the width the comparison wants rather than the width the
/// content needs. A differential run compares one pair per decision. The chance
/// of a collision hiding a divergence is the per-pair `2^-64` times the number
/// of decisions ever run. No plausible number of runs makes that worth carrying
/// more bytes for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Digest(u64);

impl Digest {
    /// The bytes, for a caller that wants to store or send them.
    pub const fn to_u64(self) -> u64 {
        self.0
    }

    /// One read back, which is what a stored comparison is made of.
    pub const fn from_u64(bytes: u64) -> Digest {
        Digest(bytes)
    }
}

impl std::fmt::Display for Digest {
    /// Sixteen lowercase hex digits, always. A fixed width, so a column of them
    /// lines up.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// An observation, or anything that lends its facts, as eight bytes.
///
/// Provided for every [`Observed`], so an owned observation and a run that
/// lends its facts are hashed by the same walk.
pub trait Digested<V: ContentVersion>: Observed<V> + Sized {
    /// Everything, prose included. What two copies of the *game* are compared
    /// on, where a changed description is a difference worth seeing.
    fn digest(&self) -> Digest {
        let mut sink = Sink::new();
        sink.observation(self);
        Digest(sink.finish())
    }

    /// The same, without the strings a person reads.
    ///
    /// What the simulator is held to.
    ///
    /// The game formats its descriptions from localised templates. An
    /// implementation without those tables cannot produce them, and holding it
    /// to them tests a text formatter, not the rules.
    ///
    /// Everything a rule can observe stays, down to the RNG draw counts. A
    /// card's numbers stay through `Preview::vars`. Only sentences go.
    fn rules_digest(&self) -> Digest {
        let mut sink = Sink::rules();
        sink.observation(self);
        Digest(sink.finish())
    }

    /// One rules digest per [`Section`], so a comparison that disagrees can
    /// name the part that moved.
    fn sections(&self) -> [(Section, Digest); Section::ALL.len()] {
        section::sections(self)
    }
}

impl<V: ContentVersion, O: Observed<V>> Digested<V> for O {}

// The tags. Written out as constants rather than as `as u8` on some Rust enum,
// because a tag is part of the digest's definition and a variant's position is
// not: reordering `Room`'s variants must not change what a rest site digests to.
const ROOM_REST_SITE: u8 = 1;
const ROOM_TREASURE: u8 = 2;
const ROOM_EVENT: u8 = 3;
const ROOM_SHOP: u8 = 4;
const EVENT_OPTION_RELIC: u8 = 1;
const EVENT_OPTION_SAID: u8 = 2;

const TAKE_RELIC: u8 = 1;
const TAKE_SKIP: u8 = 2;

const STOCK_CARD: u8 = 1;
const STOCK_RELIC: u8 = 2;
const STOCK_POTION: u8 = 3;
const STOCK_CARD_REMOVAL: u8 = 4;

const BLOCKED_NO_POTION_SLOT: u8 = 1;

const POWER_COUNTED: u8 = 1;
const POWER_ENDLESS: u8 = 2;

const MODEL_ACT: u8 = 1;
const MODEL_AFFLICTION: u8 = 2;
const MODEL_CARD: u8 = 3;
const MODEL_CHARACTER: u8 = 4;
const MODEL_ENCHANTMENT: u8 = 5;
const MODEL_ENCOUNTER: u8 = 6;
const MODEL_EVENT: u8 = 7;
const MODEL_MODIFIER: u8 = 8;
const MODEL_MONSTER: u8 = 9;
const MODEL_ORB: u8 = 10;
const MODEL_POTION: u8 = 11;
const MODEL_POWER: u8 = 12;
const MODEL_RELIC: u8 = 13;

/// The absent half of an `Option`, and the present half.
const NONE: u8 = 0;
const SOME: u8 = 1;

/// The canonical encoding, going straight into the hash.
///
/// Nothing is buffered: an observation is a few kilobytes and the encoding is
/// never wanted as bytes, only as its hash.
struct Sink {
    hash: XxHash64,
    /// Whether strings a person reads are part of the answer.
    ///
    /// See [`Observation::rules_digest`]. The nested sinks this crate takes are
    /// over ids and counts, so they carry no prose either way.
    prose: bool,
}

/// So a value can hash *itself* into the encoding.
///
/// Every write is little-endian and fixed-width, like the rest of the encoding.
/// `Hasher`'s own defaults are native-endian, which makes the digest a fact
/// about the machine.
///
/// `finish` is unreachable. `Hash::hash` only ever writes, and the digest is
/// taken by [`Sink::finish`], which consumes the sink. The trait requires the
/// method, so it says what it is rather than pretending to an answer.
impl std::hash::Hasher for Sink {
    fn write(&mut self, bytes: &[u8]) {
        self.hash.write(bytes);
    }

    fn write_u8(&mut self, n: u8) {
        self.write(&n.to_le_bytes());
    }

    fn write_u16(&mut self, n: u16) {
        self.write(&n.to_le_bytes());
    }

    fn write_u32(&mut self, n: u32) {
        self.write(&n.to_le_bytes());
    }

    fn write_u64(&mut self, n: u64) {
        self.write(&n.to_le_bytes());
    }

    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }

    fn write_i8(&mut self, n: i8) {
        self.write_u8(n as u8);
    }

    fn write_i16(&mut self, n: i16) {
        self.write_u16(n as u16);
    }

    fn write_i32(&mut self, n: i32) {
        self.write_u32(n as u32);
    }

    fn write_i64(&mut self, n: i64) {
        self.write_u64(n as u64);
    }

    fn write_isize(&mut self, n: isize) {
        self.write_i64(n as i64);
    }

    fn finish(&self) -> u64 {
        unreachable!("a Sink is finished through Sink::finish, which consumes it")
    }
}

impl Sink {
    /// Everything the observation carries, prose included.
    fn new() -> Sink {
        Sink {
            hash: XxHash64::new(),
            prose: true,
        }
    }

    /// Everything except what a person reads.
    fn rules() -> Sink {
        Sink {
            hash: XxHash64::new(),
            prose: false,
        }
    }

    /// A string written for a person rather than for a rule.
    ///
    /// Skipped entirely rather than replaced with a placeholder, in both the
    /// present and absent cases, so two observations that differ only in their
    /// wording collide. That is the point.
    fn prose_option(&mut self, value: &Option<String>) {
        if self.prose {
            self.option(value, |sink, said| sink.str(said));
        }
    }

    fn finish(&self) -> u64 {
        self.hash.finish()
    }

    fn u8(&mut self, value: u8) {
        self.hash.write(&[value]);
    }

    fn u32(&mut self, value: u32) {
        self.hash.write(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.hash.write(&value.to_le_bytes());
    }

    /// A game-domain number, encoded in the legacy digest width.
    fn number(&mut self, value: &impl ToPrimitive) {
        self.i32(
            value
                .to_i32()
                .expect("game number must fit the digest format"),
        );
    }

    fn u64(&mut self, value: u64) {
        self.hash.write(&value.to_le_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    /// A discriminant then the number, so a card that costs nothing and one
    /// that is not priced at all do not digest the same.
    ///
    /// They are different states of the game, and a digest that cannot tell
    /// them apart cannot notice one turning into the other.
    fn cost(&mut self, value: Cost) {
        match value {
            Cost::X => self.u8(0),
            Cost::Fixed(n) => {
                self.u8(1);
                self.u8(n.get());
            }
            Cost::None => self.u8(2),
        }
    }

    /// A count, always eight bytes, so a `usize` of another width digests the
    /// same.
    fn len(&mut self, value: usize) {
        self.u64(value as u64);
    }

    /// Length then bytes, so `["ab"]` and `["a", "b"]` cannot collide.
    fn str(&mut self, value: &str) {
        self.len(value.len());
        self.hash.write(value.as_bytes());
    }

    fn option<T>(&mut self, value: &Option<T>, mut each: impl FnMut(&mut Sink, &T)) {
        match value {
            None => self.u8(NONE),
            Some(value) => {
                self.u8(SOME);
                each(self, value);
            }
        }
    }

    /// Count then elements in order. Order is load-bearing everywhere it
    /// appears. A draw pile is a queue, and an intent list is the order the
    /// game draws them in.
    fn list<T>(&mut self, values: &[T], mut each: impl FnMut(&mut Sink, &T)) {
        self.len(values.len());
        for value in values {
            each(self, value);
        }
    }

    /// A [`Sink::list`] of what an iterator lends, which knows its count before
    /// it is walked.
    fn seq<T>(
        &mut self,
        values: impl ExactSizeIterator<Item = T>,
        mut each: impl FnMut(&mut Sink, T),
    ) {
        self.len(values.len());
        for value in values {
            each(self, value);
        }
    }

    /// A [`Sink::option`] of what is lent by value.
    fn maybe<T>(&mut self, value: Option<T>, each: impl FnOnce(&mut Sink, T)) {
        match value {
            None => self.u8(NONE),
            Some(value) => {
                self.u8(SOME);
                each(self, value);
            }
        }
    }

    /// An id, by the only thing about it this crate can see: which value it is.
    ///
    /// Through [`Hash`], which every generated id derives and which for a
    /// fieldless enum is its discriminant. Not by name. The question a digest
    /// asks is "is this the same value", and a discriminant answers it.
    ///
    /// It used to hash the name. The crate README says what that bought, and
    /// what it cost.
    fn id<T: Hash>(&mut self, value: &T) {
        value.hash(self);
    }

    /// The members a set holds, in the member enum's own order.
    fn flags<S>(&mut self, set: &S)
    where
        S: FlagSet,
        S::Member: Hash,
    {
        let held = || S::MEMBERS.iter().filter(|m| set.contains(**m));
        self.len(held().count());
        for member in held() {
            self.id(member);
        }
    }

    /// A map digested without reading its order.
    ///
    /// The count, then the wrapping sum of a sub-digest per entry. The crate
    /// README says why a sum.
    ///
    /// Each entry is digested whole, key and value together, so two maps that
    /// swap two values differ. The streams are sorted now, and this still
    /// takes the sum. Walking them in order instead moves every committed
    /// digest.
    fn positions<K: Hash>(&mut self, streams: impl ExactSizeIterator<Item = (K, crate::Position)>) {
        self.len(streams.len());

        let mut total: u64 = 0;
        for (stream, position) in streams {
            let mut entry = Sink::new();
            entry.id(&stream);
            entry.u32(position.counter);
            total = total.wrapping_add(entry.finish());
        }
        self.u64(total);
    }

    /// A map that has an order, walked in it.
    ///
    /// Keyed by the build's own variable, and digested by the game's name for
    /// it. Two builds that print the same number under the same name agree
    /// here, whatever each spells its own variant.
    fn vars<K: CardVars, N: ToPrimitive>(&mut self, vars: &Keyed<K, N>) {
        self.len(vars.len());
        for (key, value) in vars.iter() {
            self.str(key.name());
            self.number(value);
        }
    }

    /// Which option an event offers, by the parts its key names.
    fn event_key<V: ContentVersion>(&mut self, key: &EventOptionKey<V>, locked: bool) {
        let EventOptionKey { event, option } = key;
        self.id(event);
        if !locked && let Some(relic) = option.relic() {
            self.u8(EVENT_OPTION_RELIC);
            self.id(&relic);
        } else {
            self.u8(EVENT_OPTION_SAID);
            self.id(option);
        }
    }

    fn coord(&mut self, at: &Coord) {
        let Coord { col, row } = at;
        self.u8(*col);
        self.u8(*row);
    }

    fn card<V: ContentVersion>(&mut self, card: &CardIdent<V>) {
        let CardIdent {
            card,
            upgrade,
            affliction,
            enchantment,
        } = card;
        self.id(card);
        self.u8(*upgrade);
        self.option(affliction, |sink, a| sink.id(a));
        self.option(enchantment, |sink, e| sink.id(e));
    }

    fn model<V: ContentVersion>(&mut self, model: &ModelId<V>) {
        match model {
            ModelId::Act(id) => {
                self.u8(MODEL_ACT);
                self.id(id);
            }
            ModelId::Affliction(id) => {
                self.u8(MODEL_AFFLICTION);
                self.id(id);
            }
            ModelId::Card(id) => {
                self.u8(MODEL_CARD);
                self.id(id);
            }
            ModelId::Character(id) => {
                self.u8(MODEL_CHARACTER);
                self.id(id);
            }
            ModelId::Enchantment(id) => {
                self.u8(MODEL_ENCHANTMENT);
                self.id(id);
            }
            ModelId::Encounter(id) => {
                self.u8(MODEL_ENCOUNTER);
                self.id(id);
            }
            ModelId::Event(id) => {
                self.u8(MODEL_EVENT);
                self.id(id);
            }
            ModelId::Modifier(id) => {
                self.u8(MODEL_MODIFIER);
                self.id(id);
            }
            ModelId::Monster(id) => {
                self.u8(MODEL_MONSTER);
                self.id(id);
            }
            ModelId::Orb(id) => {
                self.u8(MODEL_ORB);
                self.id(id);
            }
            ModelId::Potion(id) => {
                self.u8(MODEL_POTION);
                self.id(id);
            }
            ModelId::Power(id) => {
                self.u8(MODEL_POWER);
                self.id(id);
            }
            ModelId::Relic(id) => {
                self.u8(MODEL_RELIC);
                self.id(id);
            }
        }
    }

    fn observation<V: ContentVersion>(&mut self, observed: &impl Observed<V>) {
        self.number(&observed.act());
        self.number(&observed.floor());
        self.number(&observed.hp());
        self.number(&observed.max_hp());
        self.bool(observed.alive());
        self.bool(observed.run_over());
        self.maybe(observed.victory(), |sink, won| sink.bool(won));
        self.number(&observed.gold());
        self.seq(observed.relics(), |sink, relic| {
            sink.carried(relic.borrow())
        });
        self.seq(observed.potions(), |sink, potion| {
            sink.held(potion.borrow())
        });
        self.len(
            observed
                .potion_slots()
                .to_usize()
                .expect("potion slot count must fit the digest format"),
        );
        self.seq(observed.deck(), |sink, card| sink.card(&card));
        self.bool(observed.in_combat());
        self.map(observed);
        self.counters_of(observed);
        self.maybe(observed.room(), |sink, room| sink.room(room.borrow()));
        self.maybe(observed.combat(), |sink, combat| sink.combat(&combat));
    }

    fn map<V: ContentVersion>(&mut self, observed: &impl Observed<V>) {
        self.maybe(observed.at(), |sink, at| sink.coord(&at));
        self.seq(observed.visited(), |sink, at| sink.coord(&at));
        self.seq(observed.travelable(), |sink, step| {
            let Reachable { at, kind } = step;
            sink.coord(&at);
            sink.id(&kind);
        });
        self.list(observed.points(), |sink, node| {
            let Node { at, kind, to } = node;
            sink.coord(at);
            sink.id(kind);
            sink.list(to, |sink, at| sink.coord(at));
        });
        self.bool(observed.proceed());
    }

    /// The draw counts, and deliberately not the generator words beside them.
    ///
    /// A build that saves the count by itself reports no words. Hashing them
    /// makes two builds of one game disagree by construction, so the words are
    /// compared where both sides carry them. That is a check the digest cannot
    /// express.
    fn counters_of<V: ContentVersion>(&mut self, observed: &impl Observed<V>) {
        self.positions(observed.run_streams());
        self.positions(observed.player_streams());
    }

    fn room<V: ContentVersion>(&mut self, room: &Room<V>) {
        match room {
            Room::RestSite { options } => {
                self.u8(ROOM_REST_SITE);
                self.list(options, |sink, option| {
                    let RestOption { index, id, enabled } = option;
                    sink.u8(*index);
                    sink.id(id);
                    sink.bool(*enabled);
                });
            }

            Room::Treasure { finished, options } => {
                self.u8(ROOM_TREASURE);
                self.bool(*finished);
                self.list(options, |sink, option| {
                    let ChestOption {
                        index,
                        take,
                        chosen,
                    } = option;
                    sink.u8(*index);
                    match take {
                        Take::Relic(relic) => {
                            sink.u8(TAKE_RELIC);
                            sink.id(relic);
                        }
                        Take::Skip => sink.u8(TAKE_SKIP),
                    }
                    sink.bool(*chosen);
                });
            }

            Room::Event {
                event,
                finished,
                title,
                text,
                options,
            } => {
                self.u8(ROOM_EVENT);
                self.id(event);
                self.bool(*finished);
                self.prose_option(title);
                self.prose_option(text);
                self.list(options, |sink, option| {
                    let EventOption {
                        index,
                        key,
                        title,
                        text,
                        locked,
                        proceed,
                        chosen,
                        relic,
                    } = option;
                    sink.u8(*index);
                    // The key is an id, not prose: it names the option without
                    // saying anything about it, and two options never share one.
                    sink.event_key(key, *locked);
                    sink.prose_option(title);
                    sink.prose_option(text);
                    sink.bool(*locked);
                    sink.bool(*proceed);
                    sink.bool(*chosen);
                    sink.option(relic, |sink, relic| sink.id(relic));
                });
            }

            Room::Shop { entries } => {
                self.u8(ROOM_SHOP);
                self.list(entries, |sink, entry| {
                    let Entry {
                        index,
                        stock,
                        cost,
                        title,
                        text,
                        stocked,
                        affordable,
                        blocked,
                    } = entry;
                    sink.u8(*index);
                    match stock {
                        Stock::Card(card) => {
                            sink.u8(STOCK_CARD);
                            sink.option(card, |sink, on_sale| {
                                sink.card(&on_sale.card);
                                sink.cost(on_sale.cost);
                                sink.cost(on_sale.star_cost);
                            });
                        }
                        Stock::Relic(relic) => {
                            sink.u8(STOCK_RELIC);
                            sink.option(relic, |sink, relic| sink.id(relic));
                        }
                        Stock::Potion(potion) => {
                            sink.u8(STOCK_POTION);
                            sink.option(potion, |sink, potion| sink.id(potion));
                        }
                        Stock::CardRemoval => sink.u8(STOCK_CARD_REMOVAL),
                    }
                    sink.number(cost);
                    sink.prose_option(title);
                    sink.prose_option(text);
                    sink.bool(*stocked);
                    sink.bool(*affordable);
                    sink.option(blocked, |sink, blocked| match blocked {
                        Blocked::NoPotionSlot => sink.u8(BLOCKED_NO_POTION_SLOT),
                    });
                });
            }
        }
    }

    /// The fight's own scalars, which is everything about it that is not a
    /// list of cards or creatures.
    ///
    /// Split out because a scenario hashes it on its own. See
    /// [`crate::Section`].
    fn fight_scalars<V: ContentVersion>(&mut self, fight: &impl ObservedFight<V>) {
        self.number(&fight.turn());
        self.id(&fight.phase());
        self.number(&fight.energy());
        self.number(&fight.stars());
        self.number(&fight.round());
        self.maybe(fight.encounter(), |sink, encounter| sink.id(&encounter));
    }

    /// The orb queue, which sits at the end of a fight's walk because that is
    /// where it has always sat. Its own method so a section reaches it too.
    fn orbs<V: ContentVersion>(&mut self, orbs: Option<&Orbs<V>>) {
        self.maybe(orbs, |sink, orbs| {
            let Orbs { capacity, queue } = orbs;
            sink.number(capacity);
            sink.list(queue, |sink, channelled| {
                let Channelled {
                    orb,
                    passive,
                    evoke,
                } = channelled;
                sink.id(orb);
                sink.number(passive);
                sink.number(evoke);
            });
        });
    }

    fn combat<V: ContentVersion>(&mut self, fight: &impl ObservedFight<V>) {
        self.fight_scalars(fight);
        self.seq(fight.hand(), |sink, card| sink.card(&card));
        self.seq(fight.draw(), |sink, card| sink.card(&card));
        self.seq(fight.discard(), |sink, card| sink.card(&card));
        self.seq(fight.exhaust(), |sink, card| sink.card(&card));
        self.seq(fight.creatures(), |sink, creature| sink.creature(&creature));
        self.seq(fight.legal(), |sink, playable| sink.playable(&playable));
        self.orbs(fight.orbs());
    }

    fn creature<V: ContentVersion>(&mut self, creature: &impl ObservedCreature<V>) {
        self.id(&creature.id());
        self.maybe(creature.model(), |sink, model| sink.model(&model));
        self.id(&creature.side());
        self.number(&creature.hp());
        self.number(&creature.max_hp());
        self.number(&creature.block());
        self.bool(creature.alive());
        self.seq(creature.powers(), |sink, stack| {
            let Stack { power, amount } = stack;
            sink.id(&power);
            match amount {
                PowerAmount::Counted(amount) => {
                    sink.u8(POWER_COUNTED);
                    sink.number(&amount);
                }
                PowerAmount::Endless => sink.u8(POWER_ENDLESS),
            }
        });
        self.maybe(creature.next_move(), |sink, next| {
            let NextMove { id, intents } = next.borrow();
            sink.id(id);
            sink.list(intents, |sink, intent| {
                let Intent { kind, attack } = intent;
                sink.id(kind);
                sink.option(attack, |sink, attack| {
                    let Attack { damage, repeats } = attack;
                    sink.number(damage);
                    sink.number(repeats);
                });
            });
        });
    }

    fn carried<V: ContentVersion>(&mut self, carried: &Carried<V>) {
        let Carried {
            relic,
            amount,
            title,
            text,
        } = carried;
        self.id(relic);
        self.number(amount);
        self.prose_option(title);
        self.prose_option(text);
    }

    fn held<V: ContentVersion>(&mut self, held: &Held<V>) {
        let Held {
            potion,
            title,
            text,
            reason,
            targets,
        } = held;
        self.id(potion);
        self.prose_option(title);
        self.prose_option(text);
        self.option(reason, |sink, why| sink.id(why));
        self.list(targets, |sink, id| sink.id(id));
    }

    fn playable<V: ContentVersion>(&mut self, playable: &impl ObservedPlayable<V>) {
        self.card(&playable.card());
        self.cost(playable.cost());
        self.cost(playable.star_cost());
        self.id(&playable.target());
        self.maybe(playable.reason(), |sink, reasons| sink.flags(&reasons));
        self.maybe(playable.preventer(), |sink, model| sink.model(&model));
        self.list(playable.targets(), |sink, id| sink.id(id));
        self.seq(playable.effect(), |sink, preview| {
            let Preview {
                target,
                text,
                vars,
                base,
            } = preview.borrow();
            sink.option(target, |sink, id| sink.id(id));
            // The numbers this sentence has in it are `vars` and `base`, which
            // are digested either way. This is the sentence.
            sink.prose_option(text);
            sink.vars(vars);
            sink.vars(base);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A length in front of every string, so a boundary cannot be moved.
    #[test]
    fn a_run_of_strings_is_not_its_concatenation() {
        let mut split = Sink::new();
        split.str("ab");
        split.str("c");

        let mut joined = Sink::new();
        joined.str("a");
        joined.str("bc");

        assert_ne!(split.finish(), joined.finish());
    }

    /// A count in front of every list, for the same reason.
    #[test]
    fn a_run_of_lists_is_not_its_concatenation() {
        let mut split = Sink::new();
        split.list(&[1u8, 2], |sink, n| sink.u8(*n));
        split.list(&[3u8], |sink, n| sink.u8(*n));

        let mut joined = Sink::new();
        joined.list(&[1u8], |sink, n| sink.u8(*n));
        joined.list(&[2u8, 3], |sink, n| sink.u8(*n));

        assert_ne!(split.finish(), joined.finish());
    }

    /// An absent value is not the same as a present one, whatever it holds.
    #[test]
    fn nothing_is_not_something() {
        let mut absent = Sink::new();
        absent.option(&None::<u8>, |sink, n| sink.u8(*n));

        let mut zero = Sink::new();
        zero.option(&Some(0u8), |sink, n| sink.u8(*n));

        assert_ne!(absent.finish(), zero.finish());
    }

    /// The counters digest the same however they were handed over, and differ
    /// when the numbers move between streams.
    ///
    /// The map sorts them now, so the first of these holds by construction.
    /// It stays because the digest is what this pins, and the digest must not
    /// start reading an order.
    #[test]
    fn a_counter_map_has_no_order_but_has_pairs() {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        enum Stream {
            #[serde(rename = "A")]
            A,
            #[serde(rename = "B")]
            B,
            #[serde(rename = "C")]
            C,
        }

        let of = |entries: [(Stream, u32); 3]| {
            let mut sink = Sink::new();
            let keyed =
                Keyed::from(entries.map(|(stream, at)| (stream, crate::Position::counted(at))));
            sink.positions(keyed.iter().map(|(&stream, &at)| (stream, at)));
            sink.finish()
        };

        use Stream::{A, B, C};
        assert_eq!(
            of([(A, 1), (B, 2), (C, 3)]),
            of([(C, 3), (A, 1), (B, 2)]),
            "which order the map stored them in is not part of the state"
        );
        assert_ne!(
            of([(A, 1), (B, 2), (C, 3)]),
            of([(A, 2), (B, 1), (C, 3)]),
            "two streams swapping their draw counts is a different position"
        );
    }

    /// Sixteen hex digits, always, so a column of digests lines up.
    #[test]
    fn a_digest_is_a_fixed_width() {
        assert_eq!(Digest(0).to_string(), "0000000000000000");
        assert_eq!(Digest(u64::MAX).to_string(), "ffffffffffffffff");
        assert_eq!(Digest(0xabc).to_string(), "0000000000000abc");
    }
}
