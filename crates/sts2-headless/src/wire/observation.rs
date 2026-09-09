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
use std::collections::{BTreeMap, HashMap};
use sts2_core::{Cost, ShopCard};

use serde::Deserialize;
use sts2_core::{
    Attack, Blocked, CardIdent, Carried, Channelled, ChestOption, Combat, ContentVersion, Coord,
    Counters, Creature, Entry, EventOption, FlagSet, Held, Intent, Map, NextMove, Node,
    Observation, Orbs, Playable, Preview, Reachable, RestOption, Room, Stack, Stock, Take,
    Undrinkable,
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
}

/// The observation a captured state spells.
///
/// Borrowed rather than owned. `&Value` is a `Deserializer`, so the mirror is
/// built out of the state where it sits. Going through `from_value` deep clones
/// every observation, which at a few hundred decisions a second is the whole
/// point of the digest undone.
pub fn observation<V: ContentVersion>(state: &serde_json::Value) -> Result<Observation<V>, Error> {
    let wire = WireObservation::<V>::deserialize(state).map_err(|e: serde_json::Error| Error {
        at: "observation".into(),
        why: Why::Unknown {
            what: "observation",
            value: e.to_string(),
        },
    })?;
    wire.into_domain()
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
/// `Creature.CombatId` is a plain `uint?` the game fills in when a creature
/// joins a fight, and an observation is captured once the game is quiescent. So
/// a null here means something this does not understand, and saying so beats
/// propagating a maybe.
fn combat_id(path: &str, id: Option<u32>) -> Result<u32, Error> {
    id.ok_or_else(|| at(path, Why::Absent("CombatId")))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", bound = "")]
struct WireObservation<V: ContentVersion> {
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
    /// Both counter sets in one dictionary, the per-player ones behind a
    /// `player.` prefix.
    rng: BTreeMap<String, u32>,
    /// Absent, not null, when the room offers nothing.
    #[serde(default)]
    room: Option<WireRoom<V>>,
    #[serde(default)]
    combat: Option<WireCombat<V>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireMap<V: ContentVersion> {
    at: Option<String>,
    visited: Vec<String>,
    travelable: Vec<WireReachable<V>>,
    points: Vec<WireNode<V>>,
    proceed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireReachable<V: ContentVersion> {
    at: String,
    #[serde(rename = "type")]
    kind: V::MapPointType,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireNode<V: ContentVersion> {
    at: String,
    #[serde(rename = "type")]
    kind: V::MapPointType,
    to: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", deny_unknown_fields, bound = "")]
enum WireRoom<V: ContentVersion> {
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
struct WireRestOption<V: ContentVersion> {
    index: u8,
    id: V::RestSiteOption,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireChestOption<V: ContentVersion> {
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
struct WireEventOption<V: ContentVersion> {
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
struct WireCarried<V: ContentVersion> {
    id: V::Relic,
    title: Option<String>,
    text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireHeld<V: ContentVersion> {
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
struct WireCombat<V: ContentVersion> {
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
struct WireOrbs<V: ContentVersion> {
    capacity: i32,
    queue: Vec<WireChannelled<V>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireChannelled<V: ContentVersion> {
    id: V::Orb,
    passive: i32,
    evoke: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", bound = "")]
struct WireCreature<V: ContentVersion> {
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
struct WireStack<V: ContentVersion> {
    id: V::Power,
    amount: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WireIntent<V: ContentVersion> {
    #[serde(rename = "type")]
    kind: V::IntentKind,
    /// Both or neither: the oracle writes them inside one `if`.
    #[serde(default)]
    damage: Option<i32>,
    #[serde(default)]
    repeats: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, bound = "")]
struct WirePlayable<V: ContentVersion> {
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
    target: Option<u32>,
    text: String,
    vars: BTreeMap<String, i32>,
    base: BTreeMap<String, i32>,
}

impl<V: ContentVersion> WireObservation<V> {
    fn into_domain(self) -> Result<Observation<V>, Error> {
        Ok(Observation {
            act: self.act,
            floor: self.floor,
            hp: self.hp,
            max_hp: self.max_hp,
            alive: self.alive,
            run_over: self.run_over,
            victory: self.victory,
            gold: self.gold,
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
            potion_slots: self.potion_slots,
            deck: cards("deck", &self.deck)?,
            in_combat: self.in_combat,
            map: self.map.into_domain()?,
            rng: counters(&self.rng)?,
            room: self.room.map(WireRoom::into_domain).transpose()?,
            combat: self.combat.map(WireCombat::into_domain).transpose()?,
        })
    }
}

/// A pile, with each card's path on a failure.
fn cards<V: ContentVersion>(path: &str, texts: &[String]) -> Result<Vec<CardIdent<V>>, Error> {
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

/// The two counter sets, split on the prefix the oracle flattened them with.
///
/// A key that is neither a run stream nor `player.` plus a player stream is a
/// stream this build has no name for. That is a divergence rather than
/// something to skip.
fn counters<V: ContentVersion>(flat: &BTreeMap<String, u32>) -> Result<Counters<V>, Error> {
    let mut run = HashMap::new();
    let mut player = HashMap::new();

    for (key, count) in flat {
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
                player.insert(named::<V::PlayerRngType>(name).ok_or_else(unknown)?, *count);
            }
            None => {
                run.insert(named::<V::RunRngType>(key).ok_or_else(unknown)?, *count);
            }
        }
    }

    Ok(Counters { run, player })
}

impl<V: ContentVersion> WireMap<V> {
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
                        to: coords(&format!("map.points[{i}].to"), &p.to)?,
                    })
                })
                .collect::<Result<_, Error>>()?,
            proceed: self.proceed,
        })
    }
}

impl<V: ContentVersion> WireRoom<V> {
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
                    .map(|o| EventOption {
                        index: o.index,
                        key: o.key,
                        title: o.title,
                        text: o.text,
                        locked: o.locked,
                        proceed: o.proceed,
                        chosen: o.chosen,
                        relic: o.relic,
                    })
                    .collect(),
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
    fn into_domain<V: ContentVersion>(self, i: usize) -> Result<Entry<V>, Error> {
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
            cost: self.cost,
            title: self.title,
            text: self.text,
            stocked: self.stocked,
            affordable: self.affordable,
            blocked: blocked(&path, self.blocked.as_deref())?,
        })
    }
}

impl<V: ContentVersion> WireCombat<V> {
    fn into_domain(self) -> Result<Combat<V>, Error> {
        Ok(Combat {
            turn: self.turn,
            phase: self.phase,
            energy: self.energy,
            stars: self.stars,
            round: self.round,
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
            orbs: self.orbs.map(|o| Orbs {
                capacity: o.capacity,
                queue: o
                    .queue
                    .into_iter()
                    .map(|c| Channelled {
                        orb: c.id,
                        passive: c.passive,
                        evoke: c.evoke,
                    })
                    .collect(),
            }),
        })
    }
}

impl<V: ContentVersion> WireCreature<V> {
    fn into_domain(self, i: usize) -> Result<Creature<V>, Error> {
        let path = format!("combat.creatures[{i}]");

        // Both come off `c.Monster`, so they are absent together or present
        // together. One without the other is the mod having changed under this.
        let next_move = match (self.intent, self.intents) {
            (Some(id), Some(intents)) => Some(NextMove {
                id,
                intents: intents
                    .into_iter()
                    .map(|w| Intent {
                        kind: w.kind,
                        attack: match (w.damage, w.repeats) {
                            (Some(damage), Some(repeats)) => Some(Attack { damage, repeats }),
                            _ => None,
                        },
                    })
                    .collect(),
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
            hp: self.hp,
            max_hp: self.max_hp,
            block: self.block,
            alive: self.alive,
            powers: self
                .powers
                .into_iter()
                .map(|p| Stack {
                    power: p.id,
                    amount: p.amount,
                })
                .collect(),
            next_move,
        })
    }
}

impl<V: ContentVersion> WireCarried<V> {
    fn into_domain(self) -> Carried<V> {
        Carried {
            relic: self.id,
            title: self.title,
            text: self.text,
        }
    }
}

impl<V: ContentVersion> WireHeld<V> {
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

impl<V: ContentVersion> WirePlayable<V> {
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
                .map(|p| Preview {
                    target: p.target,
                    text: p.text,
                    vars: p.vars,
                    base: p.base,
                })
                .collect(),
        })
    }
}

/// A `[Flags]` enum as `ToString()` wrote it: one name, or several joined with
/// `", "`.
///
/// `None` never reaches here. The oracle writes null for it, and the empty set
/// is what that becomes.
pub(crate) fn reasons<V: ContentVersion>(
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
            "relics": [{"id": "BURNING_BLOOD", "title": "Burning Blood",
                        "text": "At the end of combat, heal 6 HP."}],
            "potions": [{"id": "FIRE_POTION", "title": "Fire Potion",
                         "text": "Deal 20 damage.", "usable": false,
                         "reason": "OutOfCombat", "targets": []}],
            "potionSlots": 3,
            "deck": ["STRIKE_IRONCLAD", "BASH+1/SMOG*ADROIT"],
            "inCombat": false,
            "map": {
                "at": "3,11",
                "visited": ["3,11"],
                "travelable": [{"at": "2,10", "type": "Monster"}],
                "points": [{"at": "3,11", "type": "Ancient", "to": ["2,10"]}],
                "proceed": false
            },
            "rng": {"Shuffle": 4, "MonsterAi": 0, "player.Rewards": 2}
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
    fn the_smallest_observation_is_one() {
        let obs = parse(&base()).expect("the base should parse");

        assert_eq!(obs.gold, 99);
        assert_eq!(obs.deck.len(), 2);
        assert_eq!(obs.deck[1].upgrade, 1);
        assert_eq!(obs.map.at, Some(Coord::new(3, 11)));
        assert_eq!(obs.map.points[0].to, [Coord::new(2, 10)]);
        assert!(obs.room.is_none() && obs.combat.is_none());
    }

    /// The two counter sets come back apart, keyed by their own enums.
    #[test]
    fn the_rng_counters_split_on_the_prefix_they_were_flattened_with() {
        use sts2_content::v0_107_1::runstate::{PlayerRngType, RunRngType};

        let obs = parse(&base()).unwrap();
        assert_eq!(obs.rng.run[&RunRngType::Shuffle], 4);
        assert_eq!(obs.rng.run[&RunRngType::MonsterAi], 0);
        assert_eq!(obs.rng.player[&PlayerRngType::Rewards], 2);

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
            json!([{"id": "AKABEKO", "title": null, "text": null}]),
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
        assert_eq!(obs.potions[0].targets, vec![3, 4]);
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
        assert_eq!(c.powers[0].amount, 2);

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
