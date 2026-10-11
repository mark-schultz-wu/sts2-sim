//! The observation as the oracle spells it, and the way back to the domain.
//!
//! [`sts2_core::Observation`] is the shape, and this is the spelling. The two
//! are separate for the reason the rest of this module is separate. A second
//! implementation does not have to agree with the mod's `Dictionary<string,
//! object?>` to be comparable to it.
//!
//! The spelling is also a compression. The observation ships about 1.6KB per
//! decision at roughly 250 decisions a second. So a coordinate is `"3,11"`, and
//! a shop entry names a C# class beside an id whose meaning depends on it.
//!
//! The crate README says what this mirror does that a derive cannot, and why
//! every failure carries a path.

use super::cost::WireCost;
use std::collections::BTreeMap;
use sts2_core::{Cost, ShopCard};

use num_traits::{NumCast, ToPrimitive};
use serde::Deserialize;
use sts2_core::{
    Attack, Blocked, CardIdent, Carried, Channelled, ChestOption, Combat, CombatId, Coord,
    Creature, Entry, EventOption, EventOptionKey, FlagSet, Held, Intent, Map, NextMove, Node,
    Observation, Orbs, Playable, Position, Positions, PowerAmount, PowerTable, Preview, Reachable,
    RestOption, Room, Stack, Stock, Take, Undrinkable,
};

use super::ident::{self, named};

/// Why a captured observation is not one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{at}: {why}")]
pub struct Error {
    /// Where it went wrong, as a dotted path into the observation.
    pub at: String,
    pub why: Why,
}

/// What was wrong with it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Why {
    /// A packed string that is not the thing it claims to be.
    #[error("{0}")]
    Ident(ident::Error),
    /// A field the game types as nullable, where nothing here can stand in for
    /// the absence.
    #[error("no {0}, and there is no standing in for one")]
    Absent(&'static str),
    /// A field the shape that carries it requires.
    ///
    /// Beside [`Why::Absent`] and not the same thing. That one is a field the
    /// game legitimately leaves out and this crate cannot read past. This is a
    /// field the game promised. A `GoldReward` names its own `amount`, so its
    /// absence is the two sides disagreeing about the shape.
    #[error("a {from} with no {field}")]
    Missing { field: &'static str, from: String },
    /// A name outside the set this build has.
    #[error("{value:?} is no {what} this build has")]
    Unknown { what: &'static str, value: String },
    /// Two fields that say the same thing, saying different things.
    #[error("{0}")]
    Disagree(&'static str),
    /// A wire integer that the selected build's domain type cannot represent.
    #[error("{value} does not fit {what}")]
    OutOfRange { what: &'static str, value: String },
}

/// The observation a captured state spells.
///
/// Borrowed rather than owned. `&Value` is a `Deserializer`, so the mirror is
/// built out of the state where it sits. Going through `from_value` deep clones
/// every observation, which at a few hundred decisions a second is the whole
/// point of the digest undone.
pub fn observation<V: sts2_content_wire::WireContentVersion>(
    state: &serde_json::Value,
) -> Result<Observation<V>, Error> {
    let wire = WireObservation::<V>::deserialize(state).map_err(|e: serde_json::Error| Error {
        at: "observation".into(),
        why: Why::Unknown {
            what: "observation",
            value: e.to_string(),
        },
    })?;
    wire.into_domain()
}

/// One event option's localisation key, split into the parts it names.
///
/// `DENSE_VEGETATION.pages.REST.options.FIGHT`. The room already names the
/// event, so the first segment is held to it rather than read again. The two
/// disagreeing is the game saying two things at once.
fn option_key<V: sts2_content_wire::WireContentVersion>(
    path: &str,
    event: V::Event,
    text: &str,
) -> Result<EventOptionKey<V>, Error> {
    let named_event = serde_json::to_value(event)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| {
            at(
                path,
                Why::Unknown {
                    what: "event",
                    value: text.to_string(),
                },
            )
        })?;
    let shape = || {
        at(
            path,
            Why::Unknown {
                what: "option key of the shape `<event>.pages.<page>.options.<option>`",
                value: text.to_string(),
            },
        )
    };
    let (page, rest) = text
        .strip_prefix(&named_event)
        .and_then(|rest| rest.strip_prefix(".pages."))
        .and_then(|rest| rest.split_once(".options."))
        .ok_or_else(shape)?;
    if page.is_empty() || page.contains('.') || rest.is_empty() || rest.contains('.') {
        return Err(shape());
    }
    Ok(EventOptionKey {
        event,
        option: V::event_option(event, page, rest, named(rest))
            .unwrap_or_else(|| unimplemented!("event option {text}")),
    })
}

/// A path, built as the conversion walks down.
pub(crate) fn at(path: &str, why: Why) -> Error {
    Error {
        at: path.to_string(),
        why,
    }
}

/// One packed string, read with its path on the failure.
pub(crate) fn ident<T>(
    path: &str,
    text: &str,
    read: impl Fn(&str) -> Result<T, ident::Error>,
) -> Result<T, Error> {
    read(text).map_err(|e| at(path, Why::Ident(e)))
}

/// A `CombatId`, which the game types as nullable and a captured fight always
/// has.
///
/// Refused rather than carried as an `Option` through every reader.
/// How much of a power is on a creature, from the number the game sends.
///
/// The game overloads the sign. One power works its stacking out at the time,
/// and below zero it is a single endless instance. This is where that spelling
/// stops.
fn power_amount<V: sts2_content_wire::WireContentVersion>(
    path: &str,
    power: V::Power,
    amount: i32,
) -> Result<PowerAmount<V::Number>, Error> {
    if amount < 0 && power.endless_below_zero() {
        return Ok(PowerAmount::Endless);
    }
    Ok(PowerAmount::Counted(domain_number(path, amount)?))
}

/// `Creature.CombatId` is a plain `uint?` the game fills in when a creature
/// joins a fight, and an observation is captured once the game is quiescent. So
/// a null here means something this does not understand, and saying so beats
/// propagating a maybe.
fn combat_id(path: &str, id: Option<u32>) -> Result<CombatId, Error> {
    let id = id.ok_or_else(|| at(path, Why::Absent("CombatId")))?;
    // A creature id is a byte, because the roster bounds it. A fight this
    // crate cannot number is a divergence, not a wider id.
    u8::try_from(id)
        .ok()
        .and_then(|id| CombatId::try_new(id).ok())
        .ok_or_else(|| {
            at(
                path,
                Why::OutOfRange {
                    what: "a creature id",
                    value: id.to_string(),
                },
            )
        })
}

fn domain_number<T: NumCast>(path: &str, value: impl ToPrimitive + ToString) -> Result<T, Error> {
    let shown = value.to_string();
    T::from(value).ok_or_else(|| {
        at(
            path,
            Why::OutOfRange {
                what: "a game number",
                value: shown,
            },
        )
    })
}

fn domain_count<T: NumCast>(path: &str, value: impl ToPrimitive + ToString) -> Result<T, Error> {
    let shown = value.to_string();
    T::from(value).ok_or_else(|| {
        at(
            path,
            Why::OutOfRange {
                what: "a game count",
                value: shown,
            },
        )
    })
}

/// A preview's numbers, keyed by the build's own variable rather than by name.
///
/// A key this build does not print is a divergence. The game printed a number
/// under a name the tables do not have, and that is what the comparison exists
/// to notice.
fn domain_vars<V: sts2_content_wire::WireContentVersion, T: NumCast>(
    path: &str,
    values: BTreeMap<String, i32>,
) -> Result<sts2_core::Keyed<V::CardVar, T>, Error> {
    values
        .into_iter()
        .map(|(name, value)| {
            let key: V::CardVar = named(&name).ok_or_else(|| {
                at(
                    path,
                    Why::Unknown {
                        what: "printed card number",
                        value: name.clone(),
                    },
                )
            })?;
            domain_number(&format!("{path}.{name}"), value).map(|value| (key, value))
        })
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", bound = "")]
struct WireObservation<V: sts2_content_wire::WireContentVersion> {
    act: i32,
    floor: i32,
    hp: i32,
    max_hp: i32,
    alive: bool,
    run_over: bool,
    victory: Option<bool>,
    gold: i32,
    relics: Vec<WireCarried<V>>,
    potions: Vec<WireHeld<V>>,
    potion_slots: usize,
    deck: Vec<String>,
    in_combat: bool,
    map: WireMap<V>,
    /// Both stream sets in one dictionary, the per-player ones behind a
    /// `player.` prefix.
    rng: BTreeMap<String, WirePosition>,
    /// Absent, not null, when the room offers nothing.
    #[serde(default)]
    room: Option<WireRoom<V>>,
    #[serde(default)]
    combat: Option<WireCombat<V>>,
    /// Every move the game accepts, as the commands that make them.
    moves: Vec<super::command::Command<V>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireMap<V: sts2_content_wire::WireContentVersion> {
    at: Option<String>,
    visited: Vec<String>,
    travelable: Vec<WireReachable<V>>,
    points: Vec<WireNode<V>>,
    proceed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireReachable<V: sts2_content_wire::WireContentVersion> {
    at: String,
    #[serde(rename = "type")]
    kind: V::MapPointType,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireNode<V: sts2_content_wire::WireContentVersion> {
    at: String,
    #[serde(rename = "type")]
    kind: V::MapPointType,
    to: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", deny_unknown_fields, bound = "")]
enum WireRoom<V: sts2_content_wire::WireContentVersion> {
    RestSite {
        options: Vec<WireRestOption<V>>,
    },
    Treasure {
        finished: bool,
        options: Vec<WireChestOption<V>>,
    },
    Event {
        id: V::Event,
        finished: bool,
        title: Option<String>,
        text: Option<String>,
        options: Vec<WireEventOption<V>>,
    },
    Shop {
        entries: Vec<WireEntry>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireRestOption<V: sts2_content_wire::WireContentVersion> {
    index: u8,
    id: V::RestSiteOption,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireChestOption<V: sts2_content_wire::WireContentVersion> {
    index: u8,
    /// The relic's own entry name, or `SKIP`. Declared because it is on the
    /// wire and checked against the two below, which are what is read.
    id: String,
    #[serde(default)]
    relic: Option<V::Relic>,
    #[serde(default)]
    skip: Option<bool>,
    chosen: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireEventOption<V: sts2_content_wire::WireContentVersion> {
    index: u8,
    key: String,
    title: Option<String>,
    text: Option<String>,
    locked: bool,
    proceed: bool,
    chosen: bool,
    relic: Option<V::Relic>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEntry {
    index: u8,
    /// The C# class name, which is what says how to read `id`.
    kind: String,
    id: Option<String>,
    /// The price in gold, which is a plain number rather than a card cost.
    cost: i32,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    text: Option<String>,
    /// What a card on the shelf costs to play. Absent for every other kind,
    /// which is what `kind` already says.
    #[serde(rename = "cardCost", default)]
    card_cost: WireCost,
    #[serde(rename = "cardStarCost", default)]
    card_star_cost: WireCost,
    stocked: bool,
    affordable: bool,
    blocked: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireCarried<V: sts2_content_wire::WireContentVersion> {
    id: V::Relic,
    /// `RelicModel.DisplayAmount`, which is the number the relic shows.
    amount: V::Number,
    title: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireHeld<V: sts2_content_wire::WireContentVersion> {
    id: V::Potion,
    title: Option<String>,
    text: Option<String>,
    /// The same fact as `reason` being absent, and checked against it the way
    /// a card's `playable` is.
    usable: bool,
    reason: Option<Undrinkable>,
    targets: Vec<Option<u32>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireCombat<V: sts2_content_wire::WireContentVersion> {
    turn: i32,
    phase: V::PlayerTurnPhase,
    energy: i32,
    stars: i32,
    round: i32,
    encounter: Option<V::Encounter>,
    hand: Vec<String>,
    draw: Vec<String>,
    discard: Vec<String>,
    exhaust: Vec<String>,
    creatures: Vec<WireCreature<V>>,
    legal: Vec<WirePlayable<V>>,
    orbs: Option<WireOrbs<V>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireOrbs<V: sts2_content_wire::WireContentVersion> {
    capacity: i32,
    queue: Vec<WireChannelled<V>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireChannelled<V: sts2_content_wire::WireContentVersion> {
    id: V::Orb,
    passive: i32,
    evoke: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", bound = "")]
struct WireCreature<V: sts2_content_wire::WireContentVersion> {
    id: Option<u32>,
    model: Option<String>,
    side: V::CombatSide,
    hp: i32,
    max_hp: i32,
    block: i32,
    alive: bool,
    powers: Vec<WireStack<V>>,
    /// The move's own id, null for a creature that is not a monster.
    intent: Option<V::MoveId>,
    /// Null under exactly the same condition, which the conversion checks.
    intents: Option<Vec<WireIntent<V>>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireStack<V: sts2_content_wire::WireContentVersion> {
    id: V::Power,
    amount: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireIntent<V: sts2_content_wire::WireContentVersion> {
    #[serde(rename = "type")]
    kind: V::IntentKind,
    /// Both or neither: the oracle writes them inside one `if`.
    #[serde(default)]
    damage: Option<i32>,
    #[serde(default)]
    repeats: Option<V::Count>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WirePlayable<V: sts2_content_wire::WireContentVersion> {
    card: String,
    cost: WireCost,
    #[serde(rename = "starCost")]
    star_cost: WireCost,
    target: V::TargetType,
    /// The same fact as `reason` being absent. Read to be checked against it.
    playable: bool,
    /// The `[Flags]` enum `ToString()`-ed: `"EnergyCostTooHigh, NoLivingAllies"`.
    reason: Option<String>,
    preventer: Option<String>,
    targets: Vec<Option<u32>>,
    effect: Vec<WirePreview>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePreview {
    target: Option<CombatId>,
    text: String,
    vars: BTreeMap<String, i32>,
    base: BTreeMap<String, i32>,
}

impl<V: sts2_content_wire::WireContentVersion> WireObservation<V> {
    fn into_domain(self) -> Result<Observation<V>, Error> {
        Ok(Observation {
            act: domain_count("act", self.act)?,
            floor: domain_count("floor", self.floor)?,
            hp: domain_number("hp", self.hp)?,
            max_hp: domain_number("maxHp", self.max_hp)?,
            alive: self.alive,
            run_over: self.run_over,
            victory: self.victory,
            gold: domain_number("gold", self.gold)?,
            relics: self
                .relics
                .into_iter()
                .map(WireCarried::into_domain)
                .collect(),
            potions: self
                .potions
                .into_iter()
                .enumerate()
                .map(|(i, p)| p.into_domain(i))
                .collect::<Result<_, Error>>()?,
            potion_slots: domain_count("potionSlots", self.potion_slots)?,
            deck: cards("deck", &self.deck)?,
            in_combat: self.in_combat,
            map: self.map.into_domain()?,
            rng: counters(&self.rng)?,
            room: self.room.map(WireRoom::into_domain).transpose()?,
            combat: self.combat.map(WireCombat::into_domain).transpose()?,
            moves: self
                .moves
                .into_iter()
                .enumerate()
                .map(|(i, command)| match command.into_step() {
                    Some(sts2_core::Step::Decision(decision)) => Ok(decision),
                    _ => Err(at(
                        &format!("moves[{i}]"),
                        Why::Disagree("a move that is not a decision"),
                    )),
                })
                .collect::<Result<_, Error>>()?,
        })
    }
}

/// A pile, with each card's path on a failure.
fn cards<V: sts2_content_wire::WireContentVersion>(
    path: &str,
    texts: &[String],
) -> Result<Vec<CardIdent<V>>, Error> {
    texts
        .iter()
        .enumerate()
        .map(|(i, text)| ident(&format!("{path}[{i}]"), text, ident::card_ident::<V>))
        .collect()
}

/// A list of coordinates, likewise.
fn coords(path: &str, texts: &[String]) -> Result<Vec<Coord>, Error> {
    texts
        .iter()
        .enumerate()
        .map(|(i, text)| ident(&format!("{path}[{i}]"), text, ident::coord))
        .collect()
}

/// Where one stream stands, in either shape a build reports it.
///
/// `SerializableRng` is `{counter, s0..s3}`, and a build that saves it whole
/// sends the object. A build that saves the draw count alone sends the number,
/// and so does every recording made against one.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
enum WirePosition {
    Counted(u32),
    Stateful {
        counter: u32,
        s0: u64,
        s1: u64,
        s2: u64,
        s3: u64,
    },
}

impl From<WirePosition> for Position {
    fn from(position: WirePosition) -> Position {
        match position {
            WirePosition::Counted(counter) => Position::counted(counter),
            WirePosition::Stateful {
                counter,
                s0,
                s1,
                s2,
                s3,
            } => Position {
                counter,
                words: Some([s0, s1, s2, s3]),
            },
        }
    }
}

/// The two stream sets, split on the prefix the oracle flattened them with.
///
/// A key that is neither a run stream nor `player.` plus a player stream is a
/// stream this build has no name for. That is a divergence rather than
/// something to skip.
fn counters<V: sts2_content_wire::WireContentVersion>(
    flat: &BTreeMap<String, WirePosition>,
) -> Result<Positions<V>, Error> {
    let mut run = sts2_core::Keyed::new();
    let mut player = sts2_core::Keyed::new();

    for (key, at_stream) in flat {
        let unknown = || {
            at(
                "rng",
                Why::Unknown {
                    what: "random stream",
                    value: key.clone(),
                },
            )
        };

        match key.strip_prefix("player.") {
            Some(name) => {
                player.insert(
                    named::<V::PlayerRngType>(name).ok_or_else(unknown)?,
                    Position::from(*at_stream),
                );
            }
            None => {
                run.insert(
                    named::<V::RunRngType>(key).ok_or_else(unknown)?,
                    Position::from(*at_stream),
                );
            }
        }
    }

    Ok(Positions { run, player })
}

impl<V: sts2_content_wire::WireContentVersion> WireMap<V> {
    fn into_domain(self) -> Result<Map<V>, Error> {
        Ok(Map {
            at: self
                .at
                .as_deref()
                .map(|text| ident("map.at", text, ident::coord))
                .transpose()?,
            visited: coords("map.visited", &self.visited)?,
            travelable: self
                .travelable
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    Ok(Reachable {
                        at: ident(&format!("map.travelable[{i}].at"), &p.at, ident::coord)?,
                        kind: p.kind,
                    })
                })
                .collect::<Result<_, Error>>()?,
            points: self
                .points
                .into_iter()
                .enumerate()
                .map(|(i, p)| {
                    Ok(Node {
                        at: ident(&format!("map.points[{i}].at"), &p.at, ident::coord)?,
                        kind: p.kind,
                        to: coords(&format!("map.points[{i}].to"), &p.to)?.into(),
                    })
                })
                .collect::<Result<_, Error>>()?,
            proceed: self.proceed,
        })
    }
}

impl<V: sts2_content_wire::WireContentVersion> WireRoom<V> {
    fn into_domain(self) -> Result<Room<V>, Error> {
        Ok(match self {
            WireRoom::RestSite { options } => Room::RestSite {
                options: options
                    .into_iter()
                    .map(|o| RestOption {
                        index: o.index,
                        id: o.id,
                        enabled: o.enabled,
                    })
                    .collect(),
            },

            WireRoom::Treasure { finished, options } => Room::Treasure {
                finished,
                options: options
                    .into_iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let path = format!("room.options[{i}]");
                        // Exactly one of the two says what this option does.
                        // Both, or neither, is a shape the mod does not write.
                        let take = match (o.relic, o.skip.unwrap_or(false)) {
                            (Some(relic), false) => Take::Relic(relic),
                            (None, true) => Take::Skip,
                            _ => {
                                return Err(at(
                                    &path,
                                    Why::Disagree(
                                        "a chest option is a relic or the skip, and this is \
                                         both or neither",
                                    ),
                                ));
                            }
                        };
                        if matches!(take, Take::Skip) && o.id != "SKIP" {
                            return Err(at(
                                &path,
                                Why::Disagree("the skip option is not called SKIP"),
                            ));
                        }
                        Ok(ChestOption {
                            index: o.index,
                            take,
                            chosen: o.chosen,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            },

            WireRoom::Event {
                id,
                finished,
                title,
                text,
                options,
            } => Room::Event {
                event: id,
                finished,
                title,
                text,
                options: options
                    .into_iter()
                    .enumerate()
                    .map(|(i, o)| {
                        Ok(EventOption {
                            index: o.index,
                            key: match o.relic {
                                // An option keyed by a relic's display name,
                                // which the relic it names stands for.
                                Some(relic) if !o.key.contains(".pages.") => EventOptionKey {
                                    event: id,
                                    option: sts2_core::EventOptionKinds::from_relic(relic),
                                },
                                _ => {
                                    option_key::<V>(&format!("room.options[{i}].key"), id, &o.key)?
                                }
                            },
                            title: o.title,
                            text: o.text,
                            locked: o.locked,
                            proceed: o.proceed,
                            chosen: o.chosen,
                            relic: o.relic,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            },

            WireRoom::Shop { entries } => Room::Shop {
                entries: entries
                    .into_iter()
                    .enumerate()
                    .map(|(i, e)| e.into_domain(i))
                    .collect::<Result<_, Error>>()?,
            },
        })
    }
}

impl WireEntry {
    fn into_domain<V: sts2_content_wire::WireContentVersion>(
        self,
        i: usize,
    ) -> Result<Entry<V>, Error> {
        let path = format!("room.entries[{i}]");

        // `kind` is the C# class name, which is the only thing that says how to
        // read `id`. An unrecognised one is a fifth `MerchantEntry` subclass,
        // and guessing at its id would be inventing stock.
        let sold = self
            .id
            .as_deref()
            .map(|text| -> Result<Stock<V>, Error> {
                Ok(match self.kind.as_str() {
                    "MerchantCardEntry" => Stock::Card(Some(ShopCard {
                        card: ident(&path, text, ident::card_ident::<V>)?,
                        cost: cost(&format!("{path}.cardCost"), self.card_cost)?,
                        star_cost: cost(&format!("{path}.cardStarCost"), self.card_star_cost)?,
                    })),
                    "MerchantRelicEntry" => Stock::Relic(Some(name(&path, "relic", text)?)),
                    "MerchantPotionEntry" => Stock::Potion(Some(name(&path, "potion", text)?)),
                    _ => return Err(unknown(&path, "shop entry", &self.kind)),
                })
            })
            .transpose()?;

        let stock = match (sold, self.kind.as_str()) {
            (Some(stock), _) => stock,
            // No id, which the mod writes for an entry whose model has not been
            // generated yet -- and for the removal entry, which sells no thing.
            (None, "MerchantCardEntry") => Stock::Card(None),
            (None, "MerchantRelicEntry") => Stock::Relic(None),
            (None, "MerchantPotionEntry") => Stock::Potion(None),
            (None, "MerchantCardRemovalEntry") => Stock::CardRemoval,
            (None, kind) => return Err(unknown(&path, "shop entry", kind)),
        };

        Ok(Entry {
            index: self.index,
            stock,
            cost: domain_number(&format!("{path}.cost"), self.cost)?,
            title: self.title,
            text: self.text,
            stocked: self.stocked,
            affordable: self.affordable,
            blocked: blocked(&path, self.blocked.as_deref())?,
        })
    }
}

impl<V: sts2_content_wire::WireContentVersion> WireCombat<V> {
    fn into_domain(self) -> Result<Combat<V>, Error> {
        Ok(Combat {
            turn: domain_count("combat.turn", self.turn)?,
            phase: self.phase,
            energy: domain_count("combat.energy", self.energy)?,
            stars: domain_count("combat.stars", self.stars)?,
            round: domain_count("combat.round", self.round)?,
            encounter: self.encounter,
            hand: cards("combat.hand", &self.hand)?,
            draw: cards("combat.draw", &self.draw)?,
            discard: cards("combat.discard", &self.discard)?,
            exhaust: cards("combat.exhaust", &self.exhaust)?,
            creatures: self
                .creatures
                .into_iter()
                .enumerate()
                .map(|(i, c)| c.into_domain(i))
                .collect::<Result<_, Error>>()?,
            legal: self
                .legal
                .into_iter()
                .enumerate()
                .map(|(i, l)| l.into_domain(i))
                .collect::<Result<_, Error>>()?,
            orbs: self
                .orbs
                .map(|o| {
                    let capacity: V::Count = domain_count("combat.orbs.capacity", o.capacity)?;
                    // Two halves of one observation. A queue longer than the
                    // capacity is them contradicting each other, and nothing
                    // downstream has a sensible reading of that pair.
                    let slots = capacity.to_usize().ok_or_else(|| {
                        at(
                            "combat.orbs.capacity",
                            Why::OutOfRange {
                                what: "an addressable list",
                                value: o.capacity.to_string(),
                            },
                        )
                    })?;
                    if o.queue.len() > slots {
                        return Err(at(
                            "combat.orbs",
                            Why::Disagree("more orbs channelled than the queue holds"),
                        ));
                    }
                    Ok(Orbs {
                        capacity,
                        queue: o
                            .queue
                            .into_iter()
                            .enumerate()
                            .map(|(i, c)| {
                                Ok(Channelled {
                                    orb: c.id,
                                    passive: domain_number(
                                        &format!("combat.orbs.queue[{i}].passive"),
                                        c.passive,
                                    )?,
                                    evoke: domain_number(
                                        &format!("combat.orbs.queue[{i}].evoke"),
                                        c.evoke,
                                    )?,
                                })
                            })
                            .collect::<Result<_, Error>>()?,
                    })
                })
                .transpose()?,
        })
    }
}

impl<V: sts2_content_wire::WireContentVersion> WireCreature<V> {
    fn into_domain(self, i: usize) -> Result<Creature<V>, Error> {
        let path = format!("combat.creatures[{i}]");

        // Both come off `c.Monster`, so they are absent together or present
        // together. One without the other is the mod having changed under this.
        let next_move = match (self.intent, self.intents) {
            (Some(id), Some(intents)) => Some(NextMove {
                id,
                intents: intents
                    .into_iter()
                    .enumerate()
                    .map(|(j, w)| {
                        Ok(Intent {
                            kind: w.kind,
                            attack: match (w.damage, w.repeats) {
                                (Some(damage), Some(repeats)) => Some(Attack {
                                    damage: domain_number(
                                        &format!("{path}.intents[{j}].damage"),
                                        damage,
                                    )?,
                                    repeats,
                                }),
                                _ => None,
                            },
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            }),
            (None, None) => None,
            _ => {
                return Err(at(
                    &path,
                    Why::Disagree("a move id without its intents, or the reverse"),
                ));
            }
        };

        Ok(Creature {
            id: combat_id(&format!("{path}.id"), self.id)?,
            model: self
                .model
                .as_deref()
                .map(|text| ident(&format!("{path}.model"), text, ident::model_id::<V>))
                .transpose()?,
            side: self.side,
            hp: domain_number(&format!("{path}.hp"), self.hp)?,
            max_hp: domain_number(&format!("{path}.maxHp"), self.max_hp)?,
            block: domain_number(&format!("{path}.block"), self.block)?,
            alive: self.alive,
            powers: self
                .powers
                .into_iter()
                .enumerate()
                .map(|(j, p)| {
                    Ok(Stack {
                        power: p.id,
                        amount: power_amount::<V>(
                            &format!("{path}.powers[{j}].amount"),
                            p.id,
                            p.amount,
                        )?,
                    })
                })
                .collect::<Result<_, Error>>()?,
            next_move,
        })
    }
}

impl<V: sts2_content_wire::WireContentVersion> WireCarried<V> {
    fn into_domain(self) -> Carried<V> {
        Carried {
            relic: self.id,
            amount: self.amount,
            title: self.title,
            text: self.text,
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> WireHeld<V> {
    fn into_domain(self, i: usize) -> Result<Held<V>, Error> {
        let path = format!("potions[{i}]");

        // One fact written twice, checked rather than dropped, exactly as a
        // card's `playable` is: they can disagree only if the mod stopped
        // deriving one from the other.
        if self.usable != self.reason.is_none() {
            return Err(at(
                &path,
                Why::Disagree("`usable` and the absence of a reason disagree"),
            ));
        }

        Ok(Held {
            potion: self.id,
            title: self.title,
            text: self.text,
            reason: self.reason,
            targets: self
                .targets
                .into_iter()
                .enumerate()
                .map(|(j, id)| combat_id(&format!("{path}.targets[{j}]"), id))
                .collect::<Result<_, Error>>()?,
        })
    }
}

impl<V: sts2_content_wire::WireContentVersion> WirePlayable<V> {
    fn into_domain(self, i: usize) -> Result<Playable<V>, Error> {
        let path = format!("combat.legal[{i}]");

        // `CanPlay` returns `reason == None`, so the two are one fact written
        // twice. Checked rather than dropped: they can only disagree if the mod
        // stopped deriving one from the other.
        if self.playable != self.reason.is_none() {
            return Err(at(
                &path,
                Why::Disagree("`playable` and the absence of a reason disagree"),
            ));
        }

        let reason = self
            .reason
            .as_deref()
            .map(|text| reasons::<V>(&path, text))
            .transpose()?;

        Ok(Playable {
            card: ident(&format!("{path}.card"), &self.card, ident::card_ident::<V>)?,
            cost: cost(&format!("{path}.cost"), self.cost)?,
            star_cost: cost(&format!("{path}.starCost"), self.star_cost)?,
            target: self.target,
            reason,
            preventer: self
                .preventer
                .as_deref()
                .map(|text| ident(&format!("{path}.preventer"), text, ident::model_id::<V>))
                .transpose()?,
            targets: self
                .targets
                .into_iter()
                .enumerate()
                .map(|(j, id)| combat_id(&format!("{path}.targets[{j}]"), id))
                .collect::<Result<_, Error>>()?,
            effect: self
                .effect
                .into_iter()
                .enumerate()
                .map(|(j, p)| {
                    Ok(Preview {
                        target: p.target,
                        text: Some(p.text),
                        vars: domain_vars::<V, _>(&format!("{path}.effect[{j}].vars"), p.vars)?,
                        base: domain_vars::<V, _>(&format!("{path}.effect[{j}].base"), p.base)?,
                    })
                })
                .collect::<Result<_, Error>>()?,
        })
    }
}

/// A `[Flags]` enum as `ToString()` wrote it: one name, or several joined with
/// `", "`.
///
/// `None` never reaches here. The oracle writes null for it, and the empty set
/// is what that becomes.
pub(crate) fn reasons<V: sts2_content_wire::WireContentVersion>(
    path: &str,
    text: &str,
) -> Result<V::UnplayableReasons, Error> {
    let members = text
        .split(", ")
        .map(|name| {
            named::<V::UnplayableReason>(name)
                .ok_or_else(|| unknown(path, "reason a card is unplayable", name))
        })
        .collect::<Result<Vec<_>, Error>>()?;

    Ok(V::UnplayableReasons::from_members(&members))
}

/// Why a shop entry will not sell beyond stock and gold.
///
/// Shared with a refusal, which reports the same fact about the same entry: one
/// reading, so the two cannot disagree about what `"no potion slot"` means.
pub(crate) fn blocked(path: &str, text: Option<&str>) -> Result<Option<Blocked>, Error> {
    match text {
        None => Ok(None),
        Some("no potion slot") => Ok(Some(Blocked::NoPotionSlot)),
        Some(other) => Err(unknown(path, "reason an entry is blocked", other)),
    }
}

/// One generated id by name, with the path on a failure.
pub(crate) fn name<T: serde::de::DeserializeOwned>(
    path: &str,
    what: &'static str,
    text: &str,
) -> Result<T, Error> {
    named(text).ok_or_else(|| unknown(path, what, text))
}

/// A cost, or the path that carried something that is not one.
pub(crate) fn cost(path: &str, wire: WireCost) -> Result<Cost, Error> {
    wire.cost().map_err(|value| {
        at(
            path,
            Why::Unknown {
                what: "cost",
                value,
            },
        )
    })
}

pub(crate) fn unknown(path: &str, what: &'static str, value: &str) -> Error {
    at(
        path,
        Why::Unknown {
            what,
            value: value.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use sts2_core::fixture::at;

    /// A creature id, as an observation spells one.
    use super::*;
    use serde_json::{Value, json};
    use sts2_content::v0_107_1::V0_107_1;

    type V = V0_107_1;

    /// The smallest thing the mod calls an observation.
    ///
    /// Every required key, nothing optional. The tests below add to it or bend
    /// one field, so what each is about is the difference from this.
    fn base() -> Value {
        json!({
            "act": 0, "floor": 1, "hp": 80, "maxHp": 80, "alive": true,
            "runOver": false, "victory": null, "gold": 99,
            "relics": [{"id": "BURNING_BLOOD", "amount": 0,
                        "title": "Burning Blood",
                        "text": "At the end of combat, heal 6 HP."}],
            "potions": [{"id": "FIRE_POTION", "title": "Fire Potion",
                         "text": "Deal 20 damage.", "usable": false,
                         "reason": "OutOfCombat", "targets": []}],
            "potionSlots": 3,
            "deck": ["STRIKE_IRONCLAD", "BASH+1/SMOG*ADROIT:1"],
            "inCombat": false,
            "map": {
                "at": "3,11",
                "visited": ["3,11"],
                "travelable": [{"at": "2,10", "type": "Monster"}],
                "points": [{"at": "3,11", "type": "Ancient", "to": ["2,10"]}],
                "proceed": false
            },
            "rng": {"Shuffle": 4, "MonsterAi": 0, "player.Rewards": 2},
            "moves": [
                {"cmd": "travel", "col": 2, "row": 10},
                {"cmd": "answer", "pick": 1}
            ]
        })
    }

    fn parse(state: &Value) -> Result<Observation<V>, Error> {
        observation::<V>(state)
    }

    fn with(key: &str, value: Value) -> Value {
        let mut state = base();
        state[key] = value;
        state
    }

    #[test]
    fn the_moves_read_as_the_decisions_they_make() {
        let obs = parse(&base()).expect("the base should parse");

        assert_eq!(
            obs.moves,
            [
                sts2_core::Decision::Travel {
                    to: sts2_core::Coord::new(2, 10),
                },
                sts2_core::Decision::Answer(sts2_core::Answer::Pick(1)),
            ]
        );
    }

    #[test]
    fn a_move_that_is_not_a_decision_is_refused() {
        let start = with("moves", json!([{"cmd": "observe"}]));

        let err = parse(&start).expect_err("observing is not a move");
        assert_eq!(err.at, "moves[0]");
    }

    #[test]
    fn the_smallest_observation_is_one() {
        let obs = parse(&base()).expect("the base should parse");

        assert_eq!(obs.gold, 99);
        assert_eq!(obs.deck.len(), 2);
        assert_eq!(obs.deck[1].upgrade, 1);
        assert_eq!(obs.map.at, Some(Coord::new(3, 11)));
        assert_eq!(obs.map.points[0].to.as_slice(), [Coord::new(2, 10)]);
        assert!(obs.room.is_none() && obs.combat.is_none());
    }

    #[test]
    fn domain_numeric_boundaries_refuse_values_the_build_cannot_hold() {
        assert_eq!(domain_number::<i8>("hp", 127).unwrap(), 127);
        let number = domain_number::<i8>("hp", 128).unwrap_err();
        assert_eq!(number.at, "hp");
        assert!(matches!(number.why, Why::OutOfRange { .. }));

        assert_eq!(domain_count::<u8>("energy", 3).unwrap(), 3);
        let count = domain_count::<u8>("energy", -1).unwrap_err();
        assert_eq!(count.at, "energy");
        assert!(matches!(count.why, Why::OutOfRange { .. }));
    }

    /// The two stream sets come back apart, keyed by their own enums.
    #[test]
    fn the_rng_counters_split_on_the_prefix_they_were_flattened_with() {
        use sts2_content::v0_107_1::runstate::{PlayerRngType, RunRngType};

        let obs = parse(&base()).unwrap();
        assert_eq!(obs.rng.run[&RunRngType::Shuffle], Position::counted(4));
        assert_eq!(obs.rng.run[&RunRngType::MonsterAi], Position::counted(0));
        assert_eq!(
            obs.rng.player[&PlayerRngType::Rewards],
            Position::counted(2)
        );

        // A stream this build has no name for is a divergence, not a key to
        // skip: it means the game keeps a stream the simulator will never be
        // compared on.
        let strange = with("rng", json!({"Shuffle": 1, "Vibes": 7}));
        assert!(matches!(
            parse(&strange).unwrap_err().why,
            Why::Unknown { .. }
        ));

        // Including one that is a run stream wearing the player prefix.
        let misfiled = with("rng", json!({"player.Shuffle": 1}));
        assert!(parse(&misfiled).is_err());

        // A build that saves an `Rng` whole sends the record, and the words
        // come through beside the count. A build that saves the count alone
        // sends the number above, so both shapes reach the same type.
        let stateful = with(
            "rng",
            json!({"Shuffle": {"counter": 4, "s0": 1, "s1": 2, "s2": 3, "s3": 4}}),
        );
        assert_eq!(
            parse(&stateful).unwrap().rng.run[&RunRngType::Shuffle],
            Position {
                counter: 4,
                words: Some([1, 2, 3, 4]),
            }
        );
    }

    /// A field the mod adds is a failure, not a field that is dropped.
    ///
    /// The reason every wire struct is `deny_unknown_fields`. An observation
    /// quietly missing something is exactly the divergence this exists to
    /// notice, and otherwise it is invisible.
    #[test]
    fn a_key_this_does_not_know_fails_rather_than_vanishing() {
        let mut state = base();
        state["mystery"] = json!(1);
        assert!(parse(&state).is_err());

        let mut state = base();
        state["map"]["mystery"] = json!(1);
        assert!(parse(&state).is_err(), "nested structs too");
    }

    #[test]
    fn a_relic_carries_the_words_the_game_shows() {
        let obs = parse(&base()).expect("the base should parse");

        let [relic] = &obs.relics[..] else {
            panic!("one relic")
        };
        assert_eq!(relic.title.as_deref(), Some("Burning Blood"));
        assert_eq!(relic.amount, 0, "a relic that counts nothing shows zero");
        assert_eq!(
            relic.text.as_deref(),
            Some("At the end of combat, heal 6 HP.")
        );
    }

    /// A relic whose localisation key is missing reads as absent rather than as
    /// its id. A broken table is then visible instead of looking like a name.
    #[test]
    fn a_relic_with_no_words_keeps_its_id() {
        let state = with(
            "relics",
            json!([{"id": "AKABEKO", "amount": 0, "title": null, "text": null}]),
        );
        let obs = parse(&state).expect("a nameless relic still parses");

        assert_eq!(obs.relics[0].title, None);
        assert_eq!(obs.relics[0].text, None);
    }

    #[test]
    fn a_potion_says_why_it_cannot_be_drunk() {
        let obs = parse(&base()).expect("the base should parse");

        let [potion] = &obs.potions[..] else {
            panic!("one potion")
        };
        assert_eq!(potion.reason, Some(Undrinkable::OutOfCombat));
        assert!(potion.targets.is_empty());
    }

    #[test]
    fn a_potion_that_can_be_drunk_carries_what_to_drink_it_at() {
        let state = with(
            "potions",
            json!([{"id": "FIRE_POTION", "title": null, "text": null,
                    "usable": true, "reason": null, "targets": [3, 4]}]),
        );
        let obs = parse(&state).expect("a drinkable potion parses");

        assert_eq!(obs.potions[0].reason, None);
        assert_eq!(
            obs.potions[0].targets.as_slice(),
            [at(3), at(4)],
            "a potion names the creatures it can be drunk at"
        );
    }

    /// `usable` and `reason` are one fact written twice, the way a card's
    /// `playable` is, and get checked the same way.
    #[test]
    fn a_potion_that_is_usable_for_a_reason_is_refused() {
        let belt = |usable: bool, reason: Value| {
            parse(&with(
                "potions",
                json!([{"id": "FIRE_POTION", "title": null, "text": null,
                        "usable": usable, "reason": reason, "targets": []}]),
            ))
        };

        assert!(belt(true, json!(null)).is_ok());
        assert!(belt(false, json!("Automatic")).is_ok());

        for (usable, reason) in [(false, json!(null)), (true, json!("NotYourTurn"))] {
            let err = belt(usable, reason).unwrap_err();
            assert!(matches!(err.why, Why::Disagree(_)), "{err}");
            assert_eq!(err.at, "potions[0]", "the path names the potion");
        }
    }

    /// A potion's target is a `CombatId`, and a null one is a fault rather than
    /// a maybe. The same rule the hand's targets are read under.
    #[test]
    fn a_potion_target_with_no_combat_id_is_refused() {
        let err = parse(&with(
            "potions",
            json!([{"id": "FIRE_POTION", "title": null, "text": null,
                    "usable": true, "reason": null, "targets": [null]}]),
        ))
        .unwrap_err();

        assert_eq!(err.at, "potions[0].targets[0]", "{err}");
    }

    /// `playable` and `reason` are one fact written twice, and get checked.
    #[test]
    fn a_card_that_is_playable_for_a_reason_is_refused() {
        let legal = |playable: bool, reason: Value| {
            let mut state = base();
            state["combat"] = json!({
                "turn": 1, "phase": "Play", "energy": 3, "stars": 0, "round": 1,
                "encounter": null, "hand": [], "draw": [], "discard": [],
                "exhaust": [], "creatures": [], "orbs": null,
                "legal": [{
                    "card": "STRIKE_IRONCLAD", "cost": 1, "starCost": null, "target": "AnyEnemy",
                    "playable": playable, "reason": reason, "preventer": null,
                    "targets": [], "effect": []
                }]
            });
            parse(&state)
        };

        assert!(legal(true, json!(null)).is_ok());
        assert!(legal(false, json!("EnergyCostTooHigh")).is_ok());

        for (playable, reason) in [(false, json!(null)), (true, json!("BlockedByHook"))] {
            let err = legal(playable, reason).unwrap_err();
            assert!(matches!(err.why, Why::Disagree(_)), "{err}");
            assert_eq!(err.at, "combat.legal[0]", "the path names the card");
        }
    }

    /// Several reasons at once come back as the set the game had.
    #[test]
    fn the_reasons_a_card_fails_are_read_as_a_set() {
        use sts2_content::v0_107_1::runstate::UnplayableReason;

        let mut state = base();
        state["combat"] = json!({
            "turn": 1, "phase": "Play", "energy": 0, "stars": 0, "round": 1,
            "encounter": null, "hand": [], "draw": [], "discard": [],
            "exhaust": [], "creatures": [], "orbs": null,
            "legal": [{
                "card": "STRIKE_IRONCLAD", "cost": 1, "starCost": null, "target": "AnyAlly",
                "playable": false,
                "reason": "EnergyCostTooHigh, NoLivingAllies",
                "preventer": null, "targets": [], "effect": []
            }]
        });

        let obs = parse(&state).unwrap();
        let combat = obs.combat.unwrap();
        assert_eq!(combat.stars, 0);
        assert_eq!(combat.legal[0].star_cost, Cost::None);
        let reason = combat.legal[0].reason.unwrap();
        assert!(reason.contains(UnplayableReason::EnergyCostTooHigh));
        assert!(reason.contains(UnplayableReason::NoLivingAllies));
        assert!(!reason.contains(UnplayableReason::BlockedByHook));
    }

    /// An option keyed by a relic's display name is that relic, which the
    /// option reports beside it.
    #[test]
    fn an_option_keyed_by_a_relic_title_is_that_relic() {
        let mut state = base();
        state["room"] = json!({
            "type": "Event", "id": "DOLL_ROOM", "finished": false,
            "title": null, "text": null,
            "options": [{"index": 0, "key": "Daughter of the Wind", "title": null,
                         "text": null, "locked": false, "proceed": false,
                         "chosen": false, "relic": "DAUGHTER_OF_THE_WIND"}]
        });
        let room = parse(&state).expect("the key parses").room.expect("a room");
        let Room::Event { options, .. } = room else {
            panic!("not an event: {room:?}");
        };
        assert_eq!(
            options[0].key.option,
            sts2_content::v0_107_1::EventOptionId::Relic(
                sts2_content::v0_107_1::relics::Relic::DaughterOfTheWind
            )
        );
    }

    /// An option's key is read as its parts, and held to the room it is in.
    #[test]
    fn an_event_option_key_is_the_event_and_the_option_it_names() {
        let event = |key: &str| {
            let mut state = base();
            state["room"] = json!({
                "type": "Event", "id": "NEOW", "finished": false,
                "title": null, "text": null,
                "options": [{"index": 0, "key": key, "title": null, "text": null,
                             "locked": false, "proceed": false, "chosen": false,
                             "relic": null}]
            });
            parse(&state)
        };
        let read = |key: &str| {
            let room = event(key).expect("the key parses").room.expect("a room");
            match room {
                Room::Event { options, .. } => options[0].key.clone(),
                other => panic!("not an event: {other:?}"),
            }
        };

        // A name this build has is that relic, and one it does not have is
        // what the event called it.
        let neow = sts2_content::v0_107_1::events::Event::Neow;
        assert_eq!(
            read("NEOW.pages.INITIAL.options.NEOWS_TALISMAN"),
            EventOptionKey {
                event: neow,
                option: sts2_content::v0_107_1::EventOptionId::Relic(
                    sts2_content::v0_107_1::relics::Relic::NeowsTalisman
                ),
            }
        );
        assert!(std::panic::catch_unwind(|| read("NEOW.pages.INITIAL.options.LEAVE")).is_err());

        // The room already says which event this is. A key naming another one
        // is the observation saying two things at once.
        let other = event("OROBAS.pages.INITIAL.options.DRIFTWOOD").unwrap_err();
        assert_eq!(other.at, "room.options[0].key");
        assert!(
            matches!(&other.why, Why::Unknown { value, .. } if value == "OROBAS.pages.INITIAL.options.DRIFTWOOD"),
            "{other}"
        );

        // And a key that is not built the way every recorded key is built.
        for bad in [
            "NEOW",
            "NEOW.options.LEAVE",
            "NEOW.pages..options.LEAVE",
            "NEOW.pages.INITIAL.options.",
            "NEOW.pages.INITIAL.options.A.B",
        ] {
            let err = event(bad).unwrap_err();
            assert!(
                matches!(&err.why, Why::Unknown { value, .. } if value == bad),
                "{bad}: {err}"
            );
        }
    }

    /// A printed number the build has no key for is a divergence, not a skip.
    #[test]
    fn a_preview_variable_outside_the_build_is_refused() {
        let preview = |name: &str| {
            let mut state = base();
            state["combat"] = json!({
                "turn": 1, "phase": "Play", "energy": 3, "stars": 0, "round": 1,
                "encounter": null, "hand": ["STRIKE_REGENT"], "draw": [],
                "discard": [], "exhaust": [], "creatures": [], "orbs": null,
                "legal": [{
                    "card": "STRIKE_REGENT", "cost": 1, "starCost": null,
                    "target": "AnyEnemy", "playable": true, "reason": null,
                    "preventer": null, "targets": [],
                    "effect": [{"target": null, "text": "Deal 6 damage.",
                                "vars": {name: 6}, "base": {name: 6}}]
                }]
            });
            parse(&state)
        };

        let known = preview("Damage").expect("a key this build prints");
        let vars = &known.combat.expect("combat").legal[0].effect[0].vars;
        assert_eq!(
            vars[&sts2_content::v0_107_1::cards::CardVar::Damage],
            6,
            "the number comes back under the build's own key"
        );

        let err = preview("Vibes").unwrap_err();
        assert!(matches!(err.why, Why::Unknown { .. }), "{err}");
    }

    /// A chest option is a relic or the skip, and the wire can say both.
    #[test]
    fn a_chest_option_that_is_a_relic_and_the_skip_is_refused() {
        let chest = |option: Value| {
            let mut state = base();
            state["room"] = json!({
                "type": "Treasure", "finished": false, "options": [option]
            });
            parse(&state)
        };

        let relic = chest(json!({"index": 0, "id": "BURNING_BLOOD",
                                 "relic": "BURNING_BLOOD", "chosen": false}));
        assert!(matches!(relic.unwrap().room, Some(Room::Treasure { .. })));

        assert!(chest(json!({"index": 1, "id": "SKIP", "skip": true, "chosen": false})).is_ok());

        // Both, and neither.
        for bad in [
            json!({"index": 0, "id": "SKIP", "relic": "BURNING_BLOOD",
                   "skip": true, "chosen": false}),
            json!({"index": 0, "id": "SKIP", "chosen": false}),
        ] {
            let err = chest(bad).unwrap_err();
            assert!(matches!(err.why, Why::Disagree(_)), "{err}");
        }

        // The skip that is not called SKIP.
        let renamed = chest(json!({"index": 0, "id": "PASS", "skip": true, "chosen": false}));
        assert!(matches!(renamed.unwrap_err().why, Why::Disagree(_)));
    }

    /// The capacity and the queue are one state, and they can contradict.
    #[test]
    fn more_orbs_than_slots_is_refused_rather_than_rendered() {
        let orbs = |capacity: i32, channelled: usize| {
            let mut state = base();
            state["combat"] = json!({
                "turn": 1, "phase": "Play", "energy": 3, "stars": 0, "round": 1,
                "encounter": null, "hand": [], "draw": [], "discard": [],
                "exhaust": [], "legal": [], "creatures": [],
                "orbs": {
                    "capacity": capacity,
                    "queue": (0..channelled)
                        .map(|_| json!({"id": "LIGHTNING_ORB", "passive": 3, "evoke": 8}))
                        .collect::<Vec<_>>(),
                }
            });
            parse(&state)
        };

        let room = orbs(3, 1).unwrap();
        let queue = room.combat.unwrap().orbs.expect("a channelled orb");
        assert_eq!(queue.free(), 2);

        let full = orbs(1, 1).unwrap();
        assert_eq!(full.combat.unwrap().orbs.expect("orbs").free(), 0);

        let over = orbs(1, 2).unwrap_err();
        assert_eq!(over.at, "combat.orbs");
        assert!(matches!(over.why, Why::Disagree(_)), "{over}");
    }

    /// A creature reports its move id and its intents together or not at all.
    #[test]
    fn a_move_id_without_its_intents_is_refused() {
        let creature = |intent: Value, intents: Value| {
            let mut state = base();
            state["combat"] = json!({
                "turn": 1, "phase": "Play", "energy": 3, "stars": 0, "round": 1,
                "encounter": null, "hand": [], "draw": [], "discard": [],
                "exhaust": [], "legal": [], "orbs": null,
                "creatures": [{
                    "id": 1, "model": "MONSTER/BYRDPIP", "side": "Enemy",
                    "hp": 10, "maxHp": 10, "block": 0, "alive": true,
                    "powers": [{"id": "STRENGTH_POWER", "amount": 2}],
                    "intent": intent, "intents": intents
                }]
            });
            parse(&state)
        };

        let both = creature(
            json!("EBB_MOVE"),
            json!([{"type": "Attack", "damage": 12, "repeats": 2}]),
        )
        .unwrap();
        let c = &both.combat.unwrap().creatures[0];
        let next = c.next_move.as_ref().expect("a monster has a move");
        assert_eq!(next.id, sts2_content::v0_107_1::monsters::MoveId::EbbMove);
        assert_eq!(
            next.intents[0].attack,
            Some(Attack {
                damage: 12,
                repeats: 2
            })
        );
        assert_eq!(c.powers[0].amount, PowerAmount::Counted(2));

        // A player: neither, which is how the oracle spells "not a monster".
        assert!(
            creature(json!(null), json!(null))
                .unwrap()
                .combat
                .unwrap()
                .creatures[0]
                .next_move
                .is_none()
        );

        for (intent, intents) in [
            (json!("EBB_MOVE"), json!(null)),
            (json!(null), json!([{"type": "Attack"}])),
        ] {
            let err = creature(intent, intents).unwrap_err();
            assert!(matches!(err.why, Why::Disagree(_)), "{err}");
            assert_eq!(err.at, "combat.creatures[0]");
        }
    }

    /// An intent carries both numbers or neither, never one.
    #[test]
    fn an_intent_that_is_not_an_attack_carries_no_numbers() {
        let mut state = base();
        state["combat"] = json!({
            "turn": 1, "phase": "Play", "energy": 3, "stars": 0, "round": 1,
            "encounter": null, "hand": [], "draw": [], "discard": [],
            "exhaust": [], "legal": [], "orbs": null,
            "creatures": [{
                "id": 1, "model": "MONSTER/BYRDPIP", "side": "Enemy",
                "hp": 10, "maxHp": 10, "block": 0, "alive": true, "powers": [],
                "intent": "EBB_MOVE",
                "intents": [{"type": "Defend"}, {"type": "Buff"}]
            }]
        });

        let obs = parse(&state).unwrap();
        let intents = &obs.combat.unwrap().creatures[0]
            .next_move
            .as_ref()
            .unwrap()
            .intents
            .clone();
        assert!(intents.iter().all(|i| i.attack.is_none()));
        assert_eq!(intents.len(), 2, "a move telegraphs a list");
    }

    /// A `CombatId` the game left unset is refused rather than carried.
    #[test]
    fn a_creature_with_no_combat_id_is_refused() {
        let mut state = base();
        state["combat"] = json!({
            "turn": 1, "phase": "Play", "energy": 3, "stars": 0, "round": 1,
            "encounter": null, "hand": [], "draw": [], "discard": [],
            "exhaust": [], "legal": [], "orbs": null,
            "creatures": [{
                "id": null, "model": null, "side": "Player",
                "hp": 10, "maxHp": 10, "block": 0, "alive": true, "powers": [],
                "intent": null, "intents": null
            }]
        });

        let err = parse(&state).unwrap_err();
        assert_eq!(err.at, "combat.creatures[0].id");
        assert!(matches!(err.why, Why::Absent("CombatId")));
    }

    /// A shop entry's class name is what says how to read its id.
    #[test]
    fn a_shop_entry_is_read_by_the_class_that_sells_it() {
        let entry = |kind: &str, id: Value| {
            let mut state = base();
            state["room"] = json!({
                "type": "Shop",
                "entries": [{
                    "index": 0, "kind": kind, "id": id, "cost": 50,
                    "stocked": true, "affordable": true, "blocked": null
                }]
            });
            parse(&state)
        };

        let stock = |kind: &str, id: Value| match entry(kind, id).unwrap().room {
            Some(Room::Shop { entries }) => entries[0].stock,
            other => panic!("expected a shop, got {other:?}"),
        };

        assert!(matches!(
            stock("MerchantCardEntry", json!("BASH+1")),
            Stock::Card(Some(_))
        ));
        assert!(matches!(
            stock("MerchantRelicEntry", json!("BURNING_BLOOD")),
            Stock::Relic(Some(_))
        ));
        assert!(matches!(
            stock("MerchantPotionEntry", json!("ASHWATER")),
            Stock::Potion(Some(_))
        ));
        assert_eq!(
            stock("MerchantCardRemovalEntry", json!(null)),
            Stock::CardRemoval
        );

        // The shop has not generated the card behind the entry yet.
        assert_eq!(stock("MerchantCardEntry", json!(null)), Stock::Card(None));

        // A fifth MerchantEntry subclass. Guessing at its id would be inventing
        // stock, so it is refused by name.
        let err = entry("MerchantSoulEntry", json!("WHATEVER")).unwrap_err();
        assert!(matches!(err.why, Why::Unknown { .. }), "{err}");
        assert!(err.to_string().contains("MerchantSoulEntry"), "{err}");

        // A card id that is not a card, read as one because `kind` said so.
        let err = entry("MerchantCardEntry", json!("NOT_A_CARD")).unwrap_err();
        assert!(matches!(err.why, Why::Ident(_)), "{err}");
    }

    /// A blocked reason this does not know is a reason, not a message to keep.
    #[test]
    fn a_reason_an_entry_is_blocked_is_read_or_refused() {
        let blocked = |why: Value| {
            let mut state = base();
            state["room"] = json!({
                "type": "Shop",
                "entries": [{
                    "index": 0, "kind": "MerchantPotionEntry", "id": "ASHWATER",
                    "cost": 50, "stocked": true, "affordable": true, "blocked": why
                }]
            });
            parse(&state)
        };

        let read = |why: Value| match blocked(why).unwrap().room {
            Some(Room::Shop { entries }) => entries[0].blocked,
            other => panic!("expected a shop, got {other:?}"),
        };

        assert_eq!(read(json!(null)), None);
        assert_eq!(read(json!("no potion slot")), Some(Blocked::NoPotionSlot));
        assert!(blocked(json!("the merchant does not like you")).is_err());
    }

    /// Failures name where they happened, not just what happened.
    #[test]
    fn every_failure_carries_its_path() {
        let mut state = base();
        state["map"]["points"][0]["to"][0] = json!("4,-1");
        let err = parse(&state).unwrap_err();
        assert_eq!(err.at, "map.points[0].to[0]");
        assert!(err.to_string().contains("4,-1"), "{err}");

        let mut state = base();
        state["deck"][1] = json!("STRIKE_IRONCLAD+0");
        assert_eq!(parse(&state).unwrap_err().at, "deck[1]");
    }
}
