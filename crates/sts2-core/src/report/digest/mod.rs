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

mod xxhash;

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use xxhash::XxHash64;

use crate::{
    Attack, Blocked, CardIdent, Carried, Channelled, ChestOption, Combat, ContentVersion, Coord,
    Counters, Creature, Entry, EventOption, FlagSet, Held, Intent, Map, ModelId, NextMove, Node,
    Observation, Orbs, Playable, Preview, Reachable, RestOption, Room, Stack, Stock, Take,
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
}

impl std::fmt::Display for Digest {
    /// Sixteen lowercase hex digits, always. A fixed width, so a column of them
    /// lines up.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl<V: ContentVersion> Observation<V> {
    /// This observation, as eight bytes.
    pub fn digest(&self) -> Digest {
        let mut sink = Sink::new();
        sink.observation(self);
        Digest(sink.finish())
    }
}

// The tags. Written out as constants rather than as `as u8` on some Rust enum,
// because a tag is part of the digest's definition and a variant's position is
// not: reordering `Room`'s variants must not change what a rest site digests to.
const ROOM_REST_SITE: u8 = 1;
const ROOM_TREASURE: u8 = 2;
const ROOM_EVENT: u8 = 3;
const ROOM_SHOP: u8 = 4;

const TAKE_RELIC: u8 = 1;
const TAKE_SKIP: u8 = 2;

const STOCK_CARD: u8 = 1;
const STOCK_RELIC: u8 = 2;
const STOCK_POTION: u8 = 3;
const STOCK_CARD_REMOVAL: u8 = 4;

const BLOCKED_NO_POTION_SLOT: u8 = 1;

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
    fn new() -> Sink {
        Sink {
            hash: XxHash64::new(),
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

    fn u64(&mut self, value: u64) {
        self.hash.write(&value.to_le_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
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
        let held: Vec<&S::Member> = S::MEMBERS.iter().filter(|m| set.contains(**m)).collect();
        self.list(&held, |sink, member| sink.id(*member));
    }

    /// A map with no order of its own.
    ///
    /// The count, then the wrapping sum of a sub-digest per entry. The crate
    /// README says why a sum.
    ///
    /// Each entry is digested whole, key and value together, so two maps that
    /// swap two values differ.
    fn counters<K: Hash + Copy>(&mut self, counters: &HashMap<K, u32>) {
        self.len(counters.len());

        let mut total: u64 = 0;
        for (stream, draws) in counters {
            let mut entry = Sink::new();
            entry.id(stream);
            entry.u32(*draws);
            total = total.wrapping_add(entry.finish());
        }
        self.u64(total);
    }

    /// A map that has an order, walked in it.
    fn vars(&mut self, vars: &BTreeMap<String, i32>) {
        self.len(vars.len());
        for (name, value) in vars {
            self.str(name);
            self.i32(*value);
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

    fn observation<V: ContentVersion>(&mut self, observed: &Observation<V>) {
        let Observation {
            act,
            floor,
            hp,
            max_hp,
            alive,
            run_over,
            victory,
            gold,
            relics,
            potions,
            potion_slots,
            deck,
            in_combat,
            map,
            rng,
            room,
            combat,
        } = observed;

        self.i32(*act);
        self.i32(*floor);
        self.i32(*hp);
        self.i32(*max_hp);
        self.bool(*alive);
        self.bool(*run_over);
        self.option(victory, |sink, won| sink.bool(*won));
        self.i32(*gold);
        self.list(relics, |sink, relic| sink.carried(relic));
        self.list(potions, |sink, potion| sink.held(potion));
        self.len(*potion_slots);
        self.list(deck, |sink, card| sink.card(card));
        self.bool(*in_combat);
        self.map(map);
        self.counters_of(rng);
        self.option(room, |sink, room| sink.room(room));
        self.option(combat, |sink, combat| sink.combat(combat));
    }

    fn map<V: ContentVersion>(&mut self, map: &Map<V>) {
        let Map {
            at,
            visited,
            travelable,
            points,
            proceed,
        } = map;

        self.option(at, |sink, at| sink.coord(at));
        self.list(visited, |sink, at| sink.coord(at));
        self.list(travelable, |sink, step| {
            let Reachable { at, kind } = step;
            sink.coord(at);
            sink.id(kind);
        });
        self.list(points, |sink, node| {
            let Node { at, kind, to } = node;
            sink.coord(at);
            sink.id(kind);
            sink.list(to, |sink, at| sink.coord(at));
        });
        self.bool(*proceed);
    }

    fn counters_of<V: ContentVersion>(&mut self, rng: &Counters<V>) {
        let Counters { run, player } = rng;
        self.counters(run);
        self.counters(player);
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
                self.option(title, |sink, title| sink.str(title));
                self.option(text, |sink, text| sink.str(text));
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
                    sink.str(key);
                    sink.option(title, |sink, title| sink.str(title));
                    sink.option(text, |sink, text| sink.str(text));
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
                        stocked,
                        affordable,
                        blocked,
                    } = entry;
                    sink.u8(*index);
                    match stock {
                        Stock::Card(card) => {
                            sink.u8(STOCK_CARD);
                            sink.option(card, |sink, card| sink.card(card));
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
                    sink.i32(*cost);
                    sink.bool(*stocked);
                    sink.bool(*affordable);
                    sink.option(blocked, |sink, blocked| match blocked {
                        Blocked::NoPotionSlot => sink.u8(BLOCKED_NO_POTION_SLOT),
                    });
                });
            }
        }
    }

    fn combat<V: ContentVersion>(&mut self, combat: &Combat<V>) {
        let Combat {
            turn,
            phase,
            energy,
            round,
            encounter,
            hand,
            draw,
            discard,
            exhaust,
            creatures,
            legal,
            orbs,
        } = combat;

        self.i32(*turn);
        self.id(phase);
        self.i32(*energy);
        self.i32(*round);
        self.option(encounter, |sink, encounter| sink.id(encounter));
        for pile in [hand, draw, discard, exhaust] {
            self.list(pile, |sink, card| sink.card(card));
        }
        self.list(creatures, |sink, creature| sink.creature(creature));
        self.list(legal, |sink, playable| sink.playable(playable));
        self.option(orbs, |sink, orbs| {
            let Orbs { capacity, queue } = orbs;
            sink.i32(*capacity);
            sink.list(queue, |sink, channelled| {
                let Channelled {
                    orb,
                    passive,
                    evoke,
                } = channelled;
                sink.id(orb);
                sink.i32(*passive);
                sink.i32(*evoke);
            });
        });
    }

    fn creature<V: ContentVersion>(&mut self, creature: &Creature<V>) {
        let Creature {
            id,
            model,
            side,
            hp,
            max_hp,
            block,
            alive,
            powers,
            next_move,
        } = creature;

        self.u32(*id);
        self.option(model, |sink, model| sink.model(model));
        self.id(side);
        self.i32(*hp);
        self.i32(*max_hp);
        self.i32(*block);
        self.bool(*alive);
        self.list(powers, |sink, stack| {
            let Stack { power, amount } = stack;
            sink.id(power);
            sink.i32(*amount);
        });
        self.option(next_move, |sink, next| {
            let NextMove { id, intents } = next;
            sink.id(id);
            sink.list(intents, |sink, intent| {
                let Intent { kind, attack } = intent;
                sink.id(kind);
                sink.option(attack, |sink, attack| {
                    let Attack { damage, repeats } = attack;
                    sink.i32(*damage);
                    sink.u8(*repeats);
                });
            });
        });
    }

    fn carried<V: ContentVersion>(&mut self, carried: &Carried<V>) {
        let Carried { relic, title, text } = carried;
        self.id(relic);
        self.option(title, |sink, title| sink.str(title));
        self.option(text, |sink, text| sink.str(text));
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
        self.option(title, |sink, title| sink.str(title));
        self.option(text, |sink, text| sink.str(text));
        self.option(reason, |sink, why| sink.id(why));
        self.list(targets, |sink, id| sink.u32(*id));
    }

    fn playable<V: ContentVersion>(&mut self, playable: &Playable<V>) {
        let Playable {
            card,
            cost,
            target,
            reason,
            preventer,
            targets,
            effect,
        } = playable;

        self.card(card);
        self.i32(*cost);
        self.id(target);
        self.option(reason, |sink, reasons| sink.flags(reasons));
        self.option(preventer, |sink, model| sink.model(model));
        self.list(targets, |sink, id| sink.u32(*id));
        self.list(effect, |sink, preview| {
            let Preview {
                target,
                text,
                vars,
                base,
            } = preview;
            sink.option(target, |sink, id| sink.u32(*id));
            sink.str(text);
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

    /// The counters digest the same however the map stored them, and differ
    /// when the numbers move between streams.
    #[test]
    fn a_counter_map_has_no_order_but_has_pairs() {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
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
            sink.counters(&HashMap::from(entries));
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
