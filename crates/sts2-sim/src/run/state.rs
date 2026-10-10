use std::sync::Arc;

mod observed;

use crate::bounded::Bounded;
use crate::rng::MegaRng;
use num_traits::ToPrimitive as _;
use smallvec::SmallVec;
use sts2_core::ActMap as _;
use sts2_core::rng::stream_seed;
use sts2_core::{
    Answer, CardIdent, CardOption, Carried, Combat, Coord, Decided, Decision, EventOption,
    EventOptionKey, EventOptionKinds, Map, Observation, PlayerStreamKinds, Positions, Question,
    Reachable, RelicModel as _, RewardCard, RewardDetail, RewardKind, Room, Rules, RunStreamKinds,
    Seed, Snapshot, TakeReward,
};

use crate::combat::Error;
use crate::event::Blessing;
use crate::legal::{Legal, selections};
use crate::map::ActMap;
use crate::reward::{Odds, Offer};
use crate::run::{Opening, Rarity};
use crate::stream::Streams;
use sts2_core::Gift;

/// The authoritative state of the run slice implemented so far.
///
/// Reports are built from this state. They are never stored and edited as if
/// they were the run itself.
#[derive(Debug)]
pub(crate) struct State<V: Rules> {
    shared: Arc<RunData<V>>,
    /// The act's map, which is generated again for every act.
    map: Arc<ActMap<V>>,
    /// The point the Spoils Map's quest stands on, with the card and the gold
    /// each copy of it pays there. Its `ModifyGeneratedMapLate` sets the point
    /// to the first Treasure point of its act.
    spoils: Option<(Coord, V::Card, V::Number)>,
    /// Which act the run stands in, counted from zero.
    act: usize,
    floor: V::Count,
    /// Every point this act's run has stood on, in order.
    ///
    /// Where the run stands is the last of them. An act that has just begun
    /// has none, because `SetActInternal` clears them. Its map opens on its
    /// own starting node rather than on a point the run stands at.
    ///
    /// Inline: an act's own height bounds it, and a coordinate is two bytes.
    visited: Bounded<Coord, VISITED>,
    player: Player<V>,
    streams: Streams<V>,
    /// How many of this act's normal fights the run has walked into.
    ///
    /// The act's list is dealt at the start of the run and met in order, so
    /// this is where the next one is. No observation reports it.
    ///
    /// A constructed fight does not move it. `EnterRoomDebug` is handed its
    /// encounter and takes none from the list.
    normal_fights: usize,
    /// How many of this act's elite fights the run has walked into.
    ///
    /// Elite encounters have their own dealt list and position. Entering one
    /// must not consume the next normal encounter.
    elite_fights: usize,
    /// `RoomSet.eventsVisited`: where this act's event queue stands.
    ///
    /// Every event room the run enters marks one, an ancient's included, and
    /// so does every event the queue passes over as not allowed.
    events_used: usize,
    /// Shared until one is written. A run walks into a handful of events over
    /// four acts, and no decision inside a fight touches the list.
    visited_events: Arc<Vec<V::Event>>,
    unknown_odds: [f32; 3],
    odds: Odds,
    /// Every relic this run has dealt out of a bag, in the order it dealt
    /// them.
    ///
    /// The bags sit in the setup that every branch of a run shares. A branch
    /// that deals a relic must not deal it to the other, so what moves lives
    /// here. It is a handful over a run, not the whole table.
    dealt_relics: Arc<Vec<V::Relic>>,
    /// `PlayerExtraFields.CardShopRemovalsUsed`, which every merchant's
    /// removal price climbs with. It counts over the run, not the room.
    removals_used: usize,
    phase: Phase<V>,
    /// The event a fight was entered from, which the rewards' proceed goes
    /// back to: the room under the fight in `RunState`'s room stack. Only a
    /// fight an event entered with `shouldResumeAfterCombat` has one.
    pub(super) resuming: Option<Box<Resuming<V>>>,
}

/// An event waiting under the fight it entered.
#[derive(Debug, Clone)]
pub(super) struct Resuming<V: Rules> {
    /// The event's own phase, as it stood when the fight began.
    pub(super) event: Phase<V>,
    /// Whether an enemy escaped the fight, once it is over.
    pub(super) escaped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EncounterQueue {
    Normal,
    Elite,
    /// An act rolls one boss and fights it once, so a boss takes nothing from
    /// a queue and moves no count on.
    Boss,
    /// A fight the debug command named, which is off the map and off every
    /// queue. `EnterRoomDebug` is handed its encounter and takes none from a
    /// list.
    Named,
}

/// Data generated once and shared by every branch of this run.
///
/// Data generated once and shared by every branch of this run.
///
/// What each act's map stream is named, which is `act_{index + 1}_map`.
///
/// Spelled out rather than built, because this crate reports parts and never
/// sentences. A run has four acts, and the game's own name for each is here.
const MAP_STREAMS: [&str; 4] = ["act_1_map", "act_2_map", "act_3_map", "act_4_map"];

/// `UnknownMapPointOdds`, as an act opens with them.
///
/// The three are what an unknown room resolves to, and they climb as a run
/// walks into ones that stay events.
const UNKNOWN_ODDS: [f32; 3] = [0.1, 0.02, 0.03];

/// `DoTreasureRoomRewards`' `NextInt(42, 53)`: a chest's gold, the top
/// excluded.
const CHEST_GOLD: (i32, i32) = (42, 53);

/// `AscensionHelper.PovertyAscensionGoldMultiplier`.
const POVERTY_GOLD: f64 = 0.75;

/// Every branch and every report reads one copy of it.
#[derive(Debug)]
struct RunData<V: Rules> {
    opening: Opening<V>,
    raw_seed: u64,
    character: V::Character,
    ascension: V::AscensionLevel,
}

/// `ClearAfterPurchase`, which empties one shelf.
///
/// Each subclass drops what it was holding, so the slot keeps its index and
/// its price and no longer names a thing. The removal is the one entry with an
/// empty `ClearAfterPurchase`, and nothing calls this for it.
fn sold<V: Rules>(entries: &mut [sts2_core::Entry<V>], index: u8) {
    for entry in entries.iter_mut() {
        if entry.index != index {
            continue;
        }
        entry.stocked = false;
        entry.stock = match entry.stock {
            sts2_core::Stock::Card(_) => sts2_core::Stock::Card(None),
            sts2_core::Stock::Relic(_) => sts2_core::Stock::Relic(None),
            sts2_core::Stock::Potion(_) => sts2_core::Stock::Potion(None),
            sts2_core::Stock::CardRemoval => sts2_core::Stock::CardRemoval,
        };
    }
}

/// `MerchantCardRemovalEntry.CalcCost`, in the build's own number.
fn removal_cost<V: Rules>(used: usize) -> V::Number {
    num_traits::cast(super::shop::removal_cost(used))
        .expect("a removal price fits the build's number")
}

/// One relic the run carries, and what its own model saves.
///
/// `RelicModel` is an instance per relic held, and its saved fields outlive
/// the fight they were written in. So the run carries them, and hands them
/// back whenever a hook asks that relic something.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HeldRelic<V: Rules> {
    pub(crate) relic: V::Relic,
    pub(crate) state: V::RelicState,
    /// `IsWax`: Toy Box melts it in time.
    pub(crate) wax: bool,
    /// `IsMelted`: still on the belt, and no longer a hook listener.
    pub(crate) melted: bool,
}

impl<V: Rules> Clone for HeldRelic<V> {
    fn clone(&self) -> Self {
        HeldRelic {
            relic: self.relic,
            state: self.state.clone(),
            wax: self.wax,
            melted: self.melted,
        }
    }
}

impl<V: Rules> HeldRelic<V> {
    /// The relic as a run picks it up, with its model as new as it is.
    pub(crate) fn new(relic: V::Relic) -> Self {
        Self {
            relic,
            state: <V::RelicState as sts2_core::RelicModel<V>>::new(relic),
            wax: false,
            melted: false,
        }
    }

    /// The number it shows, which is `RelicModel.DisplayAmount`.
    pub(crate) fn amount(&self) -> V::Number {
        self.state.display_amount(self.relic)
    }
}

/// The relics a run carries, and which of them override each hook.
///
/// `CookRestSiteOption`'s `_cardsToRemove`, which it needs removable to be
/// enabled.
const COOK_CARDS: usize = 2;

/// A relic joins or leaves the belt only through here, which keeps the slots
/// true. Everything else reads and writes the belt as a slice. A
/// slice reaches a relic's own saved fields, and cannot add a relic or take
/// one away.
#[derive(Debug)]
pub(crate) struct Belt<V: Rules> {
    held: SmallVec<[HeldRelic<V>; BELT_INLINE]>,
    /// Every hook some relic on the belt overrides.
    heard: sts2_core::Overrides,
    /// For each hook, the slots of the relics that override it, one bit a
    /// slot.
    slots: [u64; sts2_core::ModelHook::ALL.len()],
}

impl<V: Rules> Clone for Belt<V> {
    fn clone(&self) -> Self {
        Belt {
            held: self.held.clone(),
            heard: self.heard,
            slots: self.slots,
        }
    }
}

impl<V: Rules> std::ops::Deref for Belt<V> {
    type Target = [HeldRelic<V>];

    fn deref(&self) -> &[HeldRelic<V>] {
        &self.held
    }
}

impl<V: Rules> std::ops::DerefMut for Belt<V> {
    fn deref_mut(&mut self) -> &mut [HeldRelic<V>] {
        &mut self.held
    }
}

impl<V: Rules> FromIterator<HeldRelic<V>> for Belt<V> {
    fn from_iter<I: IntoIterator<Item = HeldRelic<V>>>(relics: I) -> Self {
        let mut belt = Self::new();
        for held in relics {
            belt.push(held);
        }
        belt
    }
}

impl<V: Rules> Belt<V> {
    pub(crate) fn new() -> Self {
        Self {
            held: SmallVec::new(),
            heard: sts2_core::Overrides::default(),
            slots: [0; sts2_core::ModelHook::ALL.len()],
        }
    }

    pub(crate) fn push(&mut self, held: HeldRelic<V>) {
        use sts2_core::RelicTable as _;

        // `IterateHookListeners` leaves a melted relic out.
        let overrides = match held.melted {
            true => sts2_core::Overrides::NONE,
            false => held.relic.overrides(),
        };
        // One bit a slot in each hook's word.
        if self.held.len() == RELICS {
            unimplemented!("more than {RELICS} relics on a belt");
        }
        let slot = 1 << self.held.len();
        for at in overrides.places() {
            self.slots[at] |= slot;
        }
        self.heard = self.heard.union(overrides);
        self.held.push(held);
    }

    /// Take the relic in `slot` off the belt. The slots after it close up,
    /// so every hook's slots are worked out again.
    pub(crate) fn remove(&mut self, slot: usize) -> HeldRelic<V> {
        let mut held = std::mem::take(&mut self.held);
        let taken = held.remove(slot);
        *self = held.into_iter().collect();
        taken
    }

    /// Put `held` in `slot` in place of what was there, as `RelicCmd.Replace`
    /// obtains the replacement at the original's index.
    pub(crate) fn replace(&mut self, slot: usize, held: HeldRelic<V>) {
        let mut belt = std::mem::take(&mut self.held);
        belt[slot] = held;
        *self = belt.into_iter().collect();
    }

    /// `RelicCmd.Melt` of the first wax relic not yet melted, where there is
    /// one. It stays in its slot, and hears no hook after.
    pub(crate) fn melt_first_wax(&mut self) {
        if let Some(slot) = self.held.iter().position(|held| held.wax && !held.melted) {
            let mut held = self.held[slot].clone();
            held.melted = true;
            self.replace(slot, held);
        }
    }

    pub(crate) fn copy_from(&mut self, source: &Self) {
        self.held.clone_from(&source.held);
        self.heard = source.heard;
        self.slots = source.slots;
    }

    /// Whether any relic on the belt overrides a hook in `hooks`.
    pub(crate) fn hear(&self, hooks: sts2_core::Overrides) -> bool {
        self.heard.intersects(hooks)
    }

    /// The slots of the relics that override `hook`, one bit a slot, the
    /// first slot lowest.
    pub(crate) fn hearing_hook(&self, hook: sts2_core::ModelHook) -> u64 {
        self.slots[hook as usize]
    }

    /// The slots of the relics that override a hook in `hooks`, one bit a
    /// slot, the first slot lowest.
    pub(crate) fn hearing(&self, hooks: sts2_core::Overrides) -> u64 {
        hooks.places().fold(0, |slots, at| slots | self.slots[at])
    }
}

/// The small, mutable part of one player's run state.
#[derive(Debug)]
struct Player<V: Rules> {
    relics: Belt<V>,
    /// The belt, in physical slot order.
    potions: Bounded<V::Potion, BELT>,
    /// The physical slot beside each potion. Empty slots survive drinking.
    potion_positions: Bounded<u8, BELT>,
    /// Shared until one is written. Every write is a room's: a shop, a
    /// reward, a rest site. Nothing inside a fight moves the deck, so a
    /// search expanding a fight copies the pointer and not the cards.
    deck: Arc<Vec<sts2_core::DeckCard<V>>>,
    /// The id the next card to join the deck takes.
    minted: sts2_core::DeckCardId,
    /// What the run carries between fights.
    ///
    /// The game keeps one number. Here the player's creature holds its own
    /// while a fight lasts. It is built from this and written back to it
    /// when the fight is won, and a report reads the fight's while there is
    /// one.
    hp: V::Health,
    max_hp: V::Health,
    gold: V::Number,
    potion_slots: V::Count,
}

impl<V: Rules> Clone for State<V> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            map: Arc::clone(&self.map),
            spoils: self.spoils,
            player: self.player.clone(),
            visited_events: Arc::clone(&self.visited_events),
            dealt_relics: Arc::clone(&self.dealt_relics),
            phase: self.phase.clone(),
            resuming: self.resuming.clone(),
            ..*self
        }
    }

    /// Into the run already here, which is how a search reuses a node.
    ///
    /// A fight is written into the fight already here, so its lists keep what
    /// they allocated. Any other phase is cloned.
    fn clone_from(&mut self, source: &Self) {
        let State {
            shared,
            map,
            spoils,
            act,
            floor,
            visited,
            player,
            streams,
            normal_fights,
            elite_fights,
            events_used,
            visited_events,
            unknown_odds,
            odds,
            dealt_relics,
            removals_used,
            phase,
            resuming,
        } = self;
        shared.clone_from(&source.shared);
        map.clone_from(&source.map);
        *spoils = source.spoils;
        *act = source.act;
        *floor = source.floor;
        visited.copy_from(&source.visited);
        player.clone_from(&source.player);
        *streams = source.streams;
        *normal_fights = source.normal_fights;
        *elite_fights = source.elite_fights;
        *events_used = source.events_used;
        visited_events.clone_from(&source.visited_events);
        *unknown_odds = source.unknown_odds;
        *odds = source.odds;
        dealt_relics.clone_from(&source.dealt_relics);
        *removals_used = source.removals_used;
        resuming.clone_from(&source.resuming);
        match (phase, &source.phase) {
            (Phase::Combat(live), Phase::Combat(from)) => live.clone_from(from),
            (phase, from) => *phase = from.clone(),
        }
    }
}

impl<V: Rules> Clone for Player<V> {
    fn clone(&self) -> Self {
        Self {
            relics: self.relics.clone(),
            potions: self.potions,
            potion_positions: self.potion_positions,
            deck: Arc::clone(&self.deck),
            minted: self.minted,
            hp: self.hp,
            max_hp: self.max_hp,
            gold: self.gold,
            potion_slots: self.potion_slots,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        let Player {
            relics,
            potions,
            potion_positions,
            deck,
            minted,
            hp,
            max_hp,
            gold,
            potion_slots,
        } = self;
        relics.copy_from(&source.relics);
        potions.copy_from(&source.potions);
        potion_positions.copy_from(&source.potion_positions);
        deck.clone_from(&source.deck);
        *minted = source.minted;
        *hp = source.hp;
        *max_hp = source.max_hp;
        *gold = source.gold;
        *potion_slots = source.potion_slots;
    }
}

impl<V: Rules> Player<V> {
    /// The deck, to write on.
    ///
    /// The copy the writer gets is its own, and every other branch of the run
    /// keeps the one they shared. See [`Player::deck`].
    fn deck_mut(&mut self) -> &mut Vec<sts2_core::DeckCard<V>> {
        Arc::make_mut(&mut self.deck)
    }

    /// A new id for a card joining the deck.
    fn mint(&mut self) -> sts2_core::DeckCardId {
        let id = self.minted;
        self.minted = id.next();
        id
    }
}

/// How many relics a run carries, which is the bits in a hook's word of
/// [`Belt`]'s slots.
const RELICS: usize = 64;

/// How many relics the belt holds before its list reaches the heap. A run
/// that has cleared the first act carries about this many.
const BELT_INLINE: usize = 16;

/// The most potion slots a belt can have.
///
/// `Player.MaxPotionCount` is a list with no bound. It starts at three, and
/// only Potion Belt (two), Alchemical Coffer (four) and Phial Holster (one)
/// add to it. A run holds each relic once.
const BELT: usize = 10;

/// How many map points one act's run stands on. An act is shorter than this.
const VISITED: usize = 32;

/// The health a `Combat`-layout event's monsters roll as it opens, in the
/// order they stand.
pub(super) type Layout<V> = SmallVec<[<V as sts2_core::ContentVersion>::Health; 2]>;

/// Exactly one room or pending action can own the run at a time.
#[derive(Debug, Clone)]
pub(super) struct EnteredEvent<V: Rules> {
    pub event: V::Event,
    pub state: V::EventState,
    pub layout: Layout<V>,
    pub rng: MegaRng,
    pub streams: Streams<V>,
    pub events_used: usize,
    pub unknown_odds: [f32; 3],
}

#[derive(Debug, Clone)]
pub(super) enum Phase<V: Rules> {
    Neow {
        options: SmallVec<[V::Relic; 4]>,
        chosen: Option<nonmax::NonMaxU8>,
        asking: Option<Asked<V>>,
    },
    /// The map itself, which an act opens on and no room owns.
    Map,
    /// Boxed, so a run moves a pointer rather than a fight. A reused node
    /// writes into the box it already has.
    Combat(Box<crate::combat::Live<V>>),
    /// A fight that is over, and the rewards it has not handed out yet.
    ///
    /// The fight stays in the observation. It reports the state it ended in
    /// until the run leaves the room.
    Rewards {
        combat: Combat<V>,
        offered: Vec<Offer<V>>,
        /// The card removal taken and waiting on the card to remove, by its
        /// place on the screen.
        removing: Option<usize>,
    },
    Event {
        state: V::EventState,
        rng: MegaRng,
        /// What an option's effects asked, which the room parks on.
        asking: Option<Asked<V>>,
        /// `EventModel.IsFinished`, which the option that ends the event
        /// sets last.
        finished: bool,
        /// The health a `Combat`-layout event's monsters rolled as it
        /// opened. Empty for any other event.
        layout: Layout<V>,
    },
    /// A treasure room, and the relic in its chest. An empty chest leaves the
    /// room finished as it opens.
    Treasure {
        relic: Option<V::Relic>,
        finished: bool,
    },
    /// A rest site, and how far through its one option the run is.
    ///
    /// The options empty when the chosen one resolves. A rest site offers one
    /// choice, and the game stops offering any once it is taken. That is what
    /// an observation reports, rather than a flag beside them.
    RestSite {
        options: Vec<V::RestSiteOption>,
        /// The option being carried out, until it goes through.
        chosen: Option<V::RestSiteOption>,
        /// Whether an option went through, which puts the proceed button up.
        rested: bool,
        /// Waiting for the card the smith upgrades.
        upgrading: bool,
        /// The rewards the heal offered, while they are on screen, or what an
        /// option's pickup asks.
        asking: Option<Asked<V>>,
    },
    /// The fight the run died in, as it stood when the player fell.
    ///
    /// `ProcessPendingLoss` ends the combat and the run with it. The room is
    /// still the one it was fought in, so the fight is still reported. Nothing
    /// is offered, because a dead run takes nothing.
    Lost { combat: Combat<V> },
    /// The last act's boss beaten, which `RunManager.WinRun` ends the run on.
    ///
    /// The game enters The Architect's room first. It is dialogue, and its
    /// proceed wins, so the run goes straight to the win. `WinRun` then kills
    /// every player, which leaves nothing to decide.
    Won,
    /// A merchant, and how far through its shelves the run has bought.
    ///
    /// The entries are stocked once, when the run walks in. A slot the run
    /// bought stops being stocked rather than leaving the list, because the
    /// index the next decision names is the slot's own.
    Shop {
        entries: Vec<sts2_core::Entry<V>>,
        /// The purchase this merchant has not finished.
        unfinished: Option<Unfinished>,
        /// What a relic bought here asks as it is picked up.
        asking: Option<Asked<V>>,
    },
}

/// What a relic's pickup is still asking the run for.
///
/// `AfterObtained` does not return until the answer does, so the room the
/// relic was picked up in is what parks on it. A room that cannot park on one
/// says so rather than answer it itself.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Pickup<V: Rules> {
    /// `CardSelectCmd.FromDeckGeneric`, and a clone of what it takes. The card
    /// type the relic's own filter leaves out comes with it.
    CloneDeckCard { excluding: sts2_core::CardKind },
    /// `CardSelectCmd.FromDeckFor*`, and what happens to each card it takes.
    FromDeck {
        act: sts2_core::DeckAct<V>,
        min: usize,
        count: usize,
    },
    /// Cards made for the run to take into its deck, between `min` and
    /// `max` of them. `FromChooseACardScreen` takes one or none, and
    /// `FromSimpleGridForRewards` its own count exactly.
    Cards {
        offered: Vec<CardIdent<V>>,
        min: usize,
        max: usize,
    },
    /// `CardSelectCmd.FromChooseABundleScreen`: one bundle of these, whose
    /// cards all go into the deck.
    Bundles { bundles: Vec<Vec<V::Card>> },
    /// `RewardsCmd.OfferCustom`: a screen to take rewards from until every
    /// one is taken or, where `skippable`, the rest are skipped.
    Rewards {
        offered: Vec<Offer<V>>,
        skippable: bool,
    },
    /// The event's own question, which [`Rules::event_question`] asks.
    Event,
    /// The abandon-run popup, whose yes ends the run.
    ConfirmAbandon,
}

/// A pickup's question, and the rest of its `AfterObtained`, which runs once
/// the question is answered.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Asked<V: Rules> {
    pub(super) pickup: Pickup<V>,
    pub(super) rest: SmallVec<[sts2_core::HookEffect<V>; 4]>,
    /// The question this one interrupted, taken up again once this one and
    /// its rest are done. A relic taken off a reward screen asks from inside
    /// it, as `RewardsSet` awaits the relic's pickup.
    pub(super) under: Option<Box<Asked<V>>>,
}

/// What an unknown room rolls besides an event, in the order
/// `UnknownMapPointOdds` walks its odds. The elite's odds are negative, so it
/// never rolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnknownRoom {
    Monster,
    Treasure,
    Shop,
}

/// `UnknownMapPointOdds`'s base odds for each, in [`UnknownRoom`]'s order,
/// which is also what each rises by when another room rolls.
const UNKNOWN_BASE: [f32; 3] = [0.1, 0.02, 0.03];

/// A fight built and not yet entered.
///
/// Built first because building can fail, and a fight that cannot be built
/// leaves the run where it stood. What the fight costs the run is spent as it
/// enters, and carried here until then. That is the streams it drew from, and
/// the act list it came off.
#[derive(Debug)]
pub(super) struct CreatedFight<V: Rules> {
    live: Box<crate::combat::Live<V>>,
    streams: Streams<V>,
    queue: EncounterQueue,
    /// How the fight ended before the player took a turn in it, if it did.
    ended: Option<crate::combat::Ended>,
    /// What the fight's opening asked of the run.
    arriving: Vec<sts2_core::HookEffect<V>>,
}

/// Where the run stands when it enters a room.
///
/// A room is reached two ways, and this is the whole of what they differ by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Location {
    /// The map point the run travelled to.
    MapPoint(Coord),
    /// No map point at all, which is `MapPointType.Unassigned`.
    ///
    /// Only the debug construction commands reach a room this way, and the
    /// scenarios are what use them. The run gains the floor and goes on
    /// standing where it stood, so map and room disagree by construction.
    NoLocation,
}

/// A purchase a merchant is waiting on an answer to.
///
/// `OnTryPurchaseWrapper` does not return while the entry's own purchase is
/// awaiting one, so the shelf and the question it opened are one thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Unfinished {
    /// The card removal, waiting for the card it takes out of the deck. It
    /// clears no shelf, so it names none. `free` for one bought with
    /// `ignoreCost`, which spends nothing.
    Removal { free: bool },
    /// A relic's pickup, waiting for the card it copies. The shelf it came off
    /// clears when the answer lands. `parasol` for one Lord's Parasol bought,
    /// whose purchases go on from the next shelf.
    Pickup {
        shelf: u8,
        excluding: sts2_core::CardKind,
        parasol: bool,
        /// The gold the purchase spent, which `AfterItemPurchased` hands on.
        spent: u32,
    },
    /// A relic's pickup the shop parks on as any room does. The shelf it
    /// came off clears once that pickup is done. `parasol` for one Lord's
    /// Parasol bought, whose purchases go on from the next shelf.
    Relic {
        shelf: u8,
        parasol: bool,
        spent: u32,
    },
}

impl<V: Rules> Phase<V> {
    /// Whether the run can leave the room this phase stands in.
    ///
    /// A question rather than a flag, because two of the three rooms that
    /// finish already say so in what they hold. Neow has chosen a blessing and
    /// answered for it. A rest site has spent its options. Only the chest
    /// needs a bit of its own, because it keeps the relic it offered either
    /// way.
    fn finished(&self) -> bool {
        match self {
            Phase::Neow { chosen, asking, .. } => chosen.is_some() && asking.is_none(),
            Phase::Treasure { finished, .. } => *finished,
            Phase::RestSite { options, .. } => options.is_empty(),
            // A merchant never runs out. The run leaves when it decides to,
            // and a shelf it has not bought is still there.
            Phase::Shop { unfinished, .. } => unfinished.is_none(),
            // The map is not a room, so there is nothing to leave.
            Phase::Map
            | Phase::Combat(_)
            | Phase::Rewards { .. }
            | Phase::Lost { .. }
            | Phase::Won
            | Phase::Event { .. } => false,
        }
    }

    /// The question a pickup or an event option parked the room on.
    fn asking(&self) -> Option<&Asked<V>> {
        match self {
            Phase::Neow { asking, .. }
            | Phase::Event { asking, .. }
            | Phase::RestSite { asking, .. }
            | Phase::Shop { asking, .. } => asking.as_ref(),
            _ => None,
        }
    }

    /// Where the rooms that park keep what they are parked on.
    fn asking_mut(&mut self) -> Option<&mut Option<Asked<V>>> {
        match self {
            Phase::Neow { asking, .. }
            | Phase::Event { asking, .. }
            | Phase::RestSite { asking, .. }
            | Phase::Shop { asking, .. } => Some(asking),
            _ => None,
        }
    }
}

impl<V: Rules> State<V> {
    /// The question that blocks every decision except its answer.
    pub(crate) fn pending(&self) -> Option<sts2_core::QuestionKind> {
        match &self.phase {
            // The kind alone, without building the options the question lists.
            Phase::Combat(live) => live
                .selecting()
                .map(|_| sts2_core::QuestionKind::SelectCards),
            Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::Shop {
                asking: Some(_), ..
            } => self.phase.asking().map(|asked| match asked.pickup {
                Pickup::Rewards { .. } => sts2_core::QuestionKind::Rewards,
                Pickup::Bundles { .. } => sts2_core::QuestionKind::ChooseBundle,
                Pickup::ConfirmAbandon => sts2_core::QuestionKind::Confirm,
                Pickup::Event => self.event_question().kind(),
                _ => sts2_core::QuestionKind::SelectCards,
            }),
            Phase::Rewards {
                removing: Some(_), ..
            } => Some(sts2_core::QuestionKind::SelectCards),
            Phase::Rewards { offered, .. } if offered.iter().any(|offer| !offer.taken()) => {
                Some(sts2_core::QuestionKind::Rewards)
            }
            // Only a heal's rewards park at a rest site.
            Phase::RestSite {
                asking: Some(_), ..
            } => Some(sts2_core::QuestionKind::Rewards),
            Phase::RestSite { upgrading, .. } if *upgrading => {
                Some(sts2_core::QuestionKind::SelectCards)
            }
            Phase::Shop {
                unfinished: Some(_),
                ..
            } => Some(sts2_core::QuestionKind::SelectCards),
            _ => None,
        }
    }

    /// Whether the decision that just ran finished, or left a question open.
    ///
    /// Reads the phase the way [`State::pending`] does. A command knows this
    /// without a report, which is why it is here rather than derived from one.
    pub(crate) fn decided(&self) -> Decided {
        match self.pending() {
            Some(_) => Decided::Abandoned,
            None => Decided::Resolved,
        }
    }

    /// Generate one act's map, off the stream the act's own index names.
    ///
    /// `new Rng(seed, $"act_{CurrentActIndex + 1}_map")`, which is why the
    /// second act's map is not the first act's under another name.
    #[tracing::instrument(level = "trace", skip_all)]
    fn act_map(
        raw_seed: u64,
        act: V::Act,
        index: usize,
        ascension: V::AscensionLevel,
    ) -> ActMap<V> {
        let named = MAP_STREAMS
            .get(index)
            .unwrap_or_else(|| unimplemented!("the map stream a run's act {index} draws from"));
        let mut rng: MegaRng = sts2_core::rng::named::<V, _>(raw_seed, named);
        ActMap::<V>::generate(&mut rng, V::act_layout(act), ascension)
            .expect("an act's layout fits a packed map path")
    }

    /// `RunManager.GenerateMap` for the act the run stands in: `CreateMap`,
    /// then `Hook.ModifyGeneratedMap`, then `Hook.AfterMapGenerated`.
    ///
    /// # Panics
    ///
    /// A deck card that changes the map, or a relic asking for anything but
    /// a map of its own.
    fn generate_map(&mut self, act: V::Act) -> ActMap<V> {
        let mut map = Self::act_map(self.shared.raw_seed, act, self.act, self.ascension());
        // `IterateHookListeners` walks the deck's cards ahead of the relics.
        let cards: Vec<V::Card> = self
            .deck_cards_hearing(sts2_core::ModelHook::ModifyGeneratedMap)
            .collect();
        for card in cards {
            match V::deck_card_modify_generated_map(card, self.act) {
                Some(sts2_core::ReplacedMap::Spoils) => {
                    let mut rng: MegaRng =
                        sts2_core::rng::named::<V, _>(self.shared.raw_seed, "spoils_map");
                    map = ActMap::<V>::spoils(&mut rng, V::act_layout(act), self.ascension())
                        .expect("a Spoils layout fits a packed map path");
                }
                None => {}
            }
        }
        let hook = sts2_core::EventHook::GeneratedMap { act: self.act };
        for effect in self.notify_belt(hook) {
            match effect {
                sts2_core::HookEffect::FixedPathMap(rooms) => map = ActMap::fixed_path(rooms),
                effect => unimplemented!("a relic asking for {effect:?} as a map is generated"),
            }
        }
        // `ModifyGeneratedMapLate`, over what the first pass left, which
        // `Hook.ModifyGeneratedMap` runs as it returns.
        let cards: Vec<V::Card> = self
            .deck_cards_hearing(sts2_core::ModelHook::ModifyGeneratedMapLate)
            .collect();
        for card in cards {
            if let Some(gold) = V::deck_card_modify_generated_map_late(card, self.act) {
                // `GetAllMapPoints` walks the grid a column at a time.
                let first = (map.nodes().iter())
                    .filter(|node| {
                        node.kind == <V::MapPointType as sts2_core::MapPointKinds>::TREASURE
                    })
                    .map(|node| node.at)
                    .min();
                if let Some(at) = first {
                    self.spoils = Some((at, card, gold));
                }
            }
        }
        let hook = sts2_core::EventHook::GeneratedMapLate { act: self.act };
        for effect in self.notify_belt(hook) {
            match effect {
                sts2_core::HookEffect::MarkCombatPoints { relic, count, name } => {
                    self.mark_combat_points(&map, relic, count, name);
                }
                effect => unimplemented!("a relic asking for {effect:?} late in a map's making"),
            }
        }
        let cards: Vec<V::Card> = self
            .deck_cards_hearing(sts2_core::ModelHook::AfterMapGenerated)
            .collect();
        for card in cards {
            V::deck_card_after_map_generated(card, self.act);
        }
        if self.player.relics.hear(sts2_core::Overrides::of(
            sts2_core::ModelHook::AfterMapGenerated,
        )) {
            unimplemented!("a relic listening to AfterMapGenerated");
        }
        map
    }

    /// `FurCoat.AddMarkedRooms` on `map`: the relic's marks kept where every
    /// one stands on a Monster or Elite point, and `count` new ones otherwise.
    /// The new ones are the map's Monster and Elite points, column by column.
    /// `UnstableShuffle` orders them off a stream of the run's seed and the
    /// relic's id.
    ///
    /// # Panics
    ///
    /// A relic that is not on the belt.
    fn mark_combat_points(
        &mut self,
        map: &ActMap<V>,
        relic: V::Relic,
        count: usize,
        name: &'static str,
    ) {
        use sts2_core::{ActMap as _, RelicModel as _};

        let fights = |kind: V::MapPointType| {
            kind == <V::MapPointType as sts2_core::MapPointKinds>::MONSTER
                || kind == <V::MapPointType as sts2_core::MapPointKinds>::ELITE
        };
        let held = (self.player.relics.iter_mut())
            .find(|held| held.relic == relic)
            .expect("a relic marking points is on the belt");
        let marked = held.state.marked_points();
        if !marked.is_empty()
            && marked
                .iter()
                .all(|&at| map.at(at).is_ok_and(|node| fights(node.kind)))
        {
            return;
        }
        let mut points: Vec<sts2_core::Coord> = (map.nodes().iter())
            .filter(|node| fights(node.kind))
            .map(|node| node.at)
            .collect();
        // `GetAllMapPoints` walks the grid a column at a time.
        points.sort_unstable();
        let mut rng: MegaRng = sts2_core::rng::named::<V, _>(self.shared.raw_seed, name);
        sts2_core::rng::GameRng::unstable_shuffle(&mut rng, &mut points);
        points.truncate(count);
        held.state.mark_points(&points);
    }

    pub(crate) fn start(
        seed: &Seed,
        character: V::Character,
        ascension: V::AscensionLevel,
    ) -> Self {
        let raw_seed = stream_seed::<V>(seed);
        let acts = super::setup::run_acts::<V>(raw_seed);
        let mut streams = Streams::<V>::opening(raw_seed);
        let opening = Opening::draw(streams.run(V::RunRngType::UP_FRONT), character, &acts);

        let map = Self::act_map(raw_seed, acts[0], 0, ascension);
        let at = map
            .ancient()
            .expect("a generated act map opens somewhere")
            .at;

        let mut neow_rng: MegaRng = Blessing::<V>::stream(raw_seed, 0);
        let neow_options = Blessing::<V>::roll(&mut neow_rng, 1).options;

        let mut player = Player {
            relics: V::starting_relics(character)
                .into_iter()
                .map(HeldRelic::new)
                .collect(),
            potions: Bounded::new(),
            potion_positions: Bounded::new(),
            deck: Arc::new(Vec::new()),
            minted: sts2_core::DeckCardId::new(0),
            hp: V::starting_hp(character),
            max_hp: V::starting_hp(character),
            gold: num_traits::cast(V::starting_gold(character))
                .expect("starting gold fits the build's game-number type"),
            potion_slots: num_traits::cast(V::starting_potion_slots())
                .expect("starting potion slots fit the build's count type"),
        };
        let deck = V::starting_deck(character)
            .into_iter()
            .map(|card| sts2_core::DeckCard::new(player.mint(), CardIdent::new(card)))
            .collect();
        player.deck = Arc::new(deck);
        Self {
            shared: Arc::new(RunData {
                opening,
                raw_seed,
                character,
                ascension,
            }),
            map: Arc::new(map),
            spoils: None,
            act: 0,
            floor: num_traits::one(),
            visited: [at].into_iter().collect(),
            player,
            normal_fights: 0,
            elite_fights: 0,
            // A run opens in Neow's room, which is an event room.
            events_used: 1,
            visited_events: Arc::new(Vec::new()),
            unknown_odds: UNKNOWN_ODDS,
            odds: Odds::default(),
            dealt_relics: Arc::new(Vec::new()),
            removals_used: 0,
            streams,
            resuming: None,
            phase: Phase::Neow {
                options: neow_options.into_iter().collect(),
                chosen: None,
                asking: None,
            },
        }
    }

    pub(crate) fn raw_seed(&self) -> u64 {
        self.shared.raw_seed
    }

    pub(crate) fn ascension(&self) -> V::AscensionLevel {
        self.shared.ascension
    }

    pub(crate) fn character(&self) -> V::Character {
        self.shared.character
    }

    /// The fight the run is standing in, or nothing outside one.
    pub(super) fn fight(&self) -> Option<&crate::combat::Live<V>> {
        match &self.phase {
            Phase::Combat(live) => Some(live),
            Phase::Map
            | Phase::Neow { .. }
            | Phase::Rewards { .. }
            | Phase::Lost { .. }
            | Phase::Won
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => None,
        }
    }

    /// Whether a rest site option can be taken.
    ///
    /// `RestSiteOption.IsEnabled` is true unless the option overrides it.
    /// `SMITH` does: it is off when the deck holds nothing left to upgrade.
    /// `COOK` is off below two cards the deck can remove.
    fn rest_option_enabled(&self, option: V::RestSiteOption) -> bool {
        use sts2_core::RestKinds;

        if option == <V::RestSiteOption as RestKinds<V>>::SMITH {
            return self.upgradable().next().is_some();
        }
        if option == <V::RestSiteOption as RestKinds<V>>::COOK {
            return self.offered(sts2_core::DeckAct::Remove).len() >= COOK_CARDS;
        }
        true
    }

    /// The deck's cards that `CardModel.IsUpgradable` holds, each with where
    /// it sits in the deck.
    ///
    /// A selection to upgrade offers these and nothing else. The index an
    /// answer names is this list's, so the deck's own place has to be carried
    /// alongside to apply it.
    fn upgradable(&self) -> impl Iterator<Item = (usize, CardIdent<V>)> + '_ {
        use sts2_core::CardTable as _;

        self.player
            .deck
            .iter()
            .map(|held| held.ident)
            .enumerate()
            .filter(|(_, card)| card.upgrade < card.card.max_upgrade())
    }

    /// The cards a selection for `act` offers, in the order it offers them,
    /// each with where it sits in the deck.
    ///
    /// An answer names this list's indices, so the deck's own place comes
    /// with each card. A removal lists curses first, which is the sort
    /// `FromDeckForRemoval` hands the screen.
    fn offered(&self, act: sts2_core::DeckAct<V>) -> Vec<(usize, CardIdent<V>)> {
        use sts2_core::CardTable as _;

        let deck = self.player.deck.iter().map(|held| held.ident).enumerate();
        match act {
            sts2_core::DeckAct::Remove => {
                let mut offered: Vec<_> = deck.filter(|(_, card)| !V::card_eternal(card)).collect();
                offered.sort_by_key(|(_, card)| {
                    V::card_kind(card.card) != Some(sts2_core::CardKind::Curse)
                });
                offered
            }
            // `IsRemovable` and `IsUpgradable`, curses first as a removal
            // sorts them.
            sts2_core::DeckAct::RemoveOntoRelic(_) => {
                let mut offered: Vec<_> = deck
                    .filter(|(_, card)| {
                        !V::card_eternal(card) && card.upgrade < card.card.max_upgrade()
                    })
                    .collect();
                offered.sort_by_key(|(_, card)| {
                    V::card_kind(card.card) != Some(sts2_core::CardKind::Curse)
                });
                offered
            }
            sts2_core::DeckAct::RemoveBasicTagged(tag) => deck
                .filter(|(_, card)| {
                    card.card.tagged(tag)
                        && card.card.rarity() == sts2_core::CardRarityKinds::BASIC
                        && !V::card_eternal(card)
                })
                .collect(),
            sts2_core::DeckAct::Upgrade => self.upgradable().collect(),
            // `IsTransformable` in the deck is `IsRemovable`, and a quest
            // card is never offered.
            sts2_core::DeckAct::Transform(_)
            | sts2_core::DeckAct::TransformUpgraded(_)
            | sts2_core::DeckAct::TransformInto(_) => deck
                .filter(|(_, card)| {
                    !V::card_eternal(card)
                        && V::card_kind(card.card) != Some(sts2_core::CardKind::Quest)
                })
                .collect(),
            sts2_core::DeckAct::TransformBasicInto(_) => deck
                .filter(|(_, card)| {
                    !V::card_eternal(card)
                        && card.card.rarity() == sts2_core::CardRarityKinds::BASIC
                })
                .collect(),
            sts2_core::DeckAct::Enchant {
                enchantment, kind, ..
            } => deck
                .filter(|(_, card)| {
                    kind.is_none_or(|kind| V::card_kind(card.card) == Some(kind))
                        && V::can_enchant(enchantment, card)
                })
                .collect(),
        }
    }

    /// `Hook.AfterCombatEnd` for the deck's own cards, each by what it
    /// saves. A card that asks to leave the deck leaves it.
    ///
    /// # Panics
    ///
    /// A card asking for anything else.
    pub(super) fn deck_combat_ended(&mut self, room: V::RoomType) {
        use sts2_core::CardTable as _;

        let hook = sts2_core::EventHook::CombatEnd { room };
        let mut at = 0;
        while at < self.player.deck.len() {
            let card = self.player.deck[at].ident.card;
            if !card
                .overrides()
                .contains(sts2_core::ModelHook::AfterCombatEnd)
            {
                at += 1;
                continue;
            }
            let mut asked = sts2_core::HookEffects::new();
            V::deck_card_event_hook(
                card,
                &mut self.player.deck_mut()[at].state,
                hook,
                &mut asked,
            );
            let mut leaves = false;
            for effect in asked {
                match effect {
                    // `CardPileCmd.RemoveFromDeck`.
                    sts2_core::HookEffect::RemoveFromDeck => leaves = true,
                    effect => unimplemented!("{card:?} in the deck asking for {effect:?}"),
                }
            }
            match leaves {
                true => {
                    self.player.deck_mut().remove(at);
                }
                false => at += 1,
            }
        }
    }

    /// Carry `act` out on the deck cards at `taken`, in the order they were
    /// taken.
    fn act_on_deck(&mut self, act: sts2_core::DeckAct<V>, taken: &[usize]) {
        use sts2_core::rng::GameRng as _;

        match act {
            // Each taken card, in id order, kept on the relic and then
            // `CardPileCmd.RemoveFromDeck`. The relic is the last of its kind
            // on the belt, which is the one just obtained.
            sts2_core::DeckAct::RemoveOntoRelic(relic) => {
                use sts2_core::{CardTable as _, RelicModel as _};

                let mut kept: Vec<sts2_core::DeckCard<V>> =
                    taken.iter().map(|&at| self.player.deck[at]).collect();
                kept.sort_by_key(|card| card.ident.card.order());
                let mut taken = taken.to_vec();
                taken.sort_unstable_by(|a, b| b.cmp(a));
                for at in taken {
                    self.player.deck_mut().remove(at);
                }
                let slot = (self.player.relics.iter())
                    .rposition(|held| held.relic == relic)
                    .unwrap_or_else(|| unimplemented!("{relic:?} keeping cards off the belt"));
                self.player.relics[slot].state.hold_cards(&kept);
            }
            sts2_core::DeckAct::Remove | sts2_core::DeckAct::RemoveBasicTagged(_) => {
                let mut taken = taken.to_vec();
                taken.sort_unstable_by(|a, b| b.cmp(a));
                for at in taken {
                    self.player.deck_mut().remove(at);
                }
            }
            // `CardCmd.Upgrade`, which passes over a card it cannot raise.
            sts2_core::DeckAct::Upgrade => {
                use sts2_core::CardTable as _;

                for &at in taken {
                    let card = &mut self.player.deck_mut()[at].ident;
                    if card.upgrade < card.card.max_upgrade() {
                        card.upgrade += 1;
                    }
                }
            }
            // `CardCmd.Transform`: every replacement is drawn first, in the
            // order the cards were taken. The originals leave the deck, and
            // the replacements join its end in the originals' deck order.
            sts2_core::DeckAct::Transform(stream)
            | sts2_core::DeckAct::TransformUpgraded(stream) => {
                let upgrade = matches!(act, sts2_core::DeckAct::TransformUpgraded(_));
                let rng = stream_of(&mut self.phase, &mut self.streams, stream);
                let mut replaced: Vec<(usize, CardIdent<V>)> = taken
                    .iter()
                    .map(|&at| {
                        let options = V::transform_options(self.player.deck[at].ident.card, false);
                        let card = *rng
                            .choose(&options)
                            .unwrap_or_else(|_| unimplemented!("a transform with no option"));
                        let mut ident = CardIdent::new(card);
                        if upgrade && sts2_core::CardTable::max_upgrade(card) > 0 {
                            ident.upgrade = 1;
                        }
                        (at, ident)
                    })
                    .collect();
                replaced.sort_by_key(|(at, _)| *at);
                let mut gone: Vec<usize> = replaced.iter().map(|(at, _)| *at).collect();
                gone.sort_unstable_by(|a, b| b.cmp(a));
                for at in gone {
                    self.player.deck_mut().remove(at);
                }
                for (_, card) in replaced {
                    self.add_to_deck(card);
                }
            }
            // `CardCmd.Transform` with the replacements made first. The
            // originals leave the deck, and the replacements join its end in
            // the originals' deck order.
            sts2_core::DeckAct::TransformInto(into) => {
                use sts2_core::CardTable as _;

                let mut gone = taken.to_vec();
                gone.sort_unstable();
                let replaced: Vec<CardIdent<V>> = gone
                    .iter()
                    .map(|&at| {
                        let original = self.player.deck[at].ident;
                        let mut ident = CardIdent::new(into);
                        if original.upgrade > 0 && into.max_upgrade() > 0 {
                            ident.upgrade = 1;
                        }
                        if let Some(enchanted) = original.enchantment
                            && V::can_enchant(enchanted.enchantment, &ident)
                        {
                            V::enchant(enchanted.enchantment, enchanted.amount, &mut ident);
                        }
                        ident
                    })
                    .collect();
                for &at in gone.iter().rev() {
                    self.player.deck_mut().remove(at);
                }
                for card in replaced {
                    self.add_to_deck(card);
                }
            }
            // `CardCmd.TransformTo`, which lands the named card where
            // `CardCmd.Transform` lands a drawn one.
            sts2_core::DeckAct::TransformBasicInto(into) => {
                let mut gone = taken.to_vec();
                gone.sort_unstable_by(|a, b| b.cmp(a));
                for &at in &gone {
                    self.player.deck_mut().remove(at);
                }
                for _ in &gone {
                    self.add_to_deck(CardIdent::new(into));
                }
            }
            sts2_core::DeckAct::Enchant {
                enchantment,
                amount,
                ..
            } => {
                for &at in taken {
                    V::enchant(enchantment, amount, &mut self.player.deck_mut()[at].ident);
                }
            }
        }
    }

    /// Take the potion at this index out of the belt, which is
    /// `RemoveBeforeUse`.
    pub(super) fn drank(&mut self, at: usize) {
        self.player.potions.remove(at);
        self.player.potion_positions.remove(at);
        self.belt_to_fight();
    }

    /// `Owner.Potions.Any()`.
    pub(super) fn holding_potions(&self) -> bool {
        !self.player.potions.is_empty()
    }

    /// The belt's count, onto the fight the run stands in.
    fn belt_to_fight(&mut self) {
        let held = u8::try_from(self.player.potions.len()).expect("a belt's potions fit a byte");
        if let Phase::Combat(live) = &mut self.phase {
            live.hold_potions(held);
        }
    }

    /// `PotionCmd.TryToProcure` once the potion is in: `AfterPotionProcured`.
    ///
    /// # Panics
    ///
    /// A relic asking for anything outside a fight.
    fn procured(&mut self) {
        self.belt_to_fight();
        let hook = sts2_core::EventHook::PotionProcured {
            in_fight: matches!(self.phase, Phase::Combat(_)),
        };
        self.belt_changed(hook);
    }

    /// `hook`, for a change to the belt, raised on the fight the run stands
    /// in or else on the belt.
    ///
    /// # Panics
    ///
    /// A relic asking for anything outside a fight.
    fn belt_changed(&mut self, hook: sts2_core::EventHook<V>) {
        if let Some(fighting) = self.fighting() {
            crate::combat::belt_changed(fighting, hook);
            return;
        }
        if let Some(effect) = self.notify_belt(hook).first() {
            unimplemented!("a relic asking for {effect:?} on {hook:?} outside a fight");
        }
    }

    /// `PotionCmd.TryToProcure` into the slot at `slot`, which lands while
    /// that slot is open and no relic refuses it.
    fn procure_into(&mut self, potion: V::Potion, slot: usize) -> bool {
        let slot = u8::try_from(slot).expect("a belt slot fits a byte");
        if self.procure_refused(potion) || self.player.potion_positions.contains(&slot) {
            return false;
        }
        let at = self
            .player
            .potion_positions
            .partition_point(|held| *held < slot);
        self.player.potion_positions.insert(at, slot);
        self.player.potions.insert(at, potion);
        self.procured();
        true
    }

    /// `PotionCmd.TryToProcure`: the potion in the first empty physical belt
    /// slot, unless a relic refuses it.
    fn procure(&mut self, potion: V::Potion) -> bool {
        if self.procure_refused(potion) {
            return false;
        }
        let slots: usize = num_traits::cast(self.player.potion_slots)
            .expect("the belt has an addressable number of slots");
        let Some(slot) = (0..slots)
            .map(|slot| u8::try_from(slot).expect("a belt slot fits a byte"))
            .find(|slot| !self.player.potion_positions.contains(slot))
        else {
            return false;
        };
        let at = self
            .player
            .potion_positions
            .partition_point(|held| *held < slot);
        self.player.potion_positions.insert(at, slot);
        self.player.potions.insert(at, potion);
        self.procured();
        true
    }

    /// The fight the run is standing in, with the streams and the belt its
    /// rules draw from and write on, or nothing outside one.
    ///
    /// Borrowed apart, so that a decision can work on all three at once.
    pub(super) fn fighting(&mut self) -> Option<crate::combat::Fighting<'_, V>> {
        let Phase::Combat(live) = &mut self.phase else {
            return None;
        };
        Some(crate::combat::Fighting {
            live,
            streams: &mut self.streams,
            relics: &mut self.player.relics,
        })
    }

    /// Take the fight out of the run, for a decision that ended it, and lend
    /// the belt its report reads.
    ///
    /// What replaces it is the caller's to say, straight after.
    pub(super) fn left_fight(&mut self) -> (Box<crate::combat::Live<V>>, &Belt<V>) {
        match std::mem::replace(&mut self.phase, Phase::Map) {
            Phase::Combat(live) => (live, &self.player.relics),
            _ => unreachable!("a fight that ended is the one the run stood in"),
        }
    }

    /// Remove from the deck the copies the fight took for good, from `since`
    /// on in the fight's own list of them.
    ///
    /// The fight keeps every copy it took, because a won fight offers each one
    /// back. So a decision passes the length the list had before it began, and
    /// only what it added leaves the deck.
    pub(super) fn lose_cards(&mut self, since: usize) {
        let Phase::Combat(live) = &mut self.phase else {
            return;
        };
        for next in since..live.removed_from_deck().len() {
            let id = live.removed_from_deck()[next];
            let Some(at) = self.player.deck.iter().position(|held| held.id == id) else {
                continue;
            };
            let card = self.player.deck_mut().remove(at);
            live.take_from_deck(card);
        }
    }

    /// What the fight asked of the run during a decision, carried out.
    ///
    /// # Panics
    ///
    /// Something that asks the run a question.
    pub(super) fn asked_by_fight(&mut self) {
        let Phase::Combat(live) = &mut self.phase else {
            return;
        };
        if !live.asked_for_run() {
            return;
        }
        let asked = live.take_for_run();
        if self.carry_out(asked).is_some() {
            unimplemented!("a fight asking the run a question mid-decision");
        }
    }

    /// The player's gold, back from the fight that held it.
    pub(super) fn gold_from_fight(&mut self) {
        if let Phase::Combat(live) = &self.phase {
            self.player.gold = live.gold();
        }
    }

    /// Park the run on the rewards a won fight left it.
    ///
    /// What the fight did to the player comes out with it. The creature was
    /// built from the run's own numbers and is where they stood while the
    /// fight lasted.
    pub(super) fn won(&mut self, combat: Combat<V>, offered: Vec<Offer<V>>) {
        if let Some(player) = combat
            .creatures
            .iter()
            .find(|c| c.id == self.player_id(&combat))
        {
            self.player.hp = player.hp;
            self.player.max_hp = player.max_hp;
        }
        self.phase = Phase::Rewards {
            combat,
            offered,
            removing: None,
        };
    }

    /// Park the run on the fight it died in.
    ///
    /// The creature was built from the run's own numbers, and where it fell is
    /// where they stayed. Nothing writes them back, because a run that is over
    /// has nowhere to carry them to.
    pub(super) fn lost(&mut self, combat: Combat<V>) {
        self.phase = Phase::Lost { combat };
    }

    /// The player's own creature in a fight.
    fn player_id(&self, combat: &Combat<V>) -> sts2_core::CombatId {
        combat
            .creatures
            .iter()
            .find(|c| c.side == <V::CombatSide as sts2_core::SideKinds>::PLAYER)
            .expect("a fight has the player in it")
            .id
    }

    /// The stream a reward draws from, and the pity counters it moves.
    ///
    /// Together, because a roll reads and writes both, and they are two fields
    /// of the same run.
    pub(super) fn rewarding(&mut self) -> (&mut MegaRng, &mut Odds) {
        (
            self.streams.player(V::PlayerRngType::REWARDS),
            &mut self.odds,
        )
    }

    /// `RelicFactory.PullNextRelicFromFront` at `rarity`, through the shop
    /// filter for a stocked pull. [`pulled_rarity`] is the rarity it deals at.
    pub(super) fn pull_relic_as(
        &mut self,
        pull: sts2_core::RelicPull<V>,
        rarity: Rarity<V>,
    ) -> V::Relic {
        use sts2_core::RelicTable as _;

        let relic = match pull {
            sts2_core::RelicPull::Rolled => return self.pull_relic(rarity),
            sts2_core::RelicPull::Stocked(_) => self.shared.opening.player_relics.front_where(
                rarity,
                &self.dealt_relics,
                V::Relic::allowed_in_shops,
            ),
        };
        Arc::make_mut(&mut self.dealt_relics).push(relic);
        relic
    }

    /// The run's two relic bags: the player's own, then the shared one.
    ///
    /// A relic dealt out of the first leaves the second with it, so whoever
    /// pulls one reaches both.
    /// `RelicFactory.PullNextRelicFromFront`, once the rarity is rolled.
    ///
    /// The player's bag deals it, and the run notes it as dealt. That note is
    /// what `SharedRelicGrabBag.Remove` does on the other side, so the shared
    /// bag needs no second one.
    pub(super) fn pull_relic(&mut self, rarity: Rarity<V>) -> V::Relic {
        let relic = self
            .shared
            .opening
            .player_relics
            .front(rarity, &self.dealt_relics);
        Arc::make_mut(&mut self.dealt_relics).push(relic);
        relic
    }

    /// The next relic in the shared bag, without dealing it yet.
    pub(super) fn next_shared_relic(&self, rarity: Rarity<V>) -> V::Relic {
        self.shared
            .opening
            .shared_relics
            .front(rarity, &self.dealt_relics)
    }

    /// `Player.CanRemovePotions` off: an event that holds the potions while it
    /// runs, and has not finished.
    pub(super) fn potions_held(&self) -> bool {
        match &self.phase {
            Phase::Event {
                state,
                finished: false,
                ..
            } => V::event_holds_potions(state),
            _ => false,
        }
    }

    /// Which act the run stands in, counted from zero.
    pub(super) fn act(&self) -> usize {
        self.act
    }

    /// `RunManager.EnterNextAct`, which the finished act's boss room offers.
    /// A run that has beaten the last act's boss wins.
    ///
    /// `SetActInternal` moves the index on, forgets the last act's points,
    /// resets the question mark odds and generates the new map.
    ///
    /// After a fight an event entered to come back to, the event resumes:
    /// `ProceedFromTerminalRewardsScreen` is `ResumePreviousRoom` there.
    pub(crate) fn proceed(&mut self) -> Result<Decided, Error> {
        if let Some(resuming) = self.resuming.take() {
            let Resuming { event, escaped } = *resuming;
            self.phase = event;
            let Phase::Event { state, .. } = &mut self.phase else {
                unreachable!("only an event resumes after its fight")
            };
            let effects = V::resume_event_from_fight(state, escaped);
            return Ok(match self.carry_out(effects) {
                None => Decided::Resolved,
                Some(asked) => self.park(asked),
            });
        }
        let next = self.act + 1;
        let Some(&(act, _)) = self.shared.opening.acts.get(next) else {
            self.phase = Phase::Won;
            return Ok(Decided::Resolved);
        };
        self.act = next;
        self.map = Arc::new(self.generate_map(act));
        self.visited.clear();
        // `Odds.UnknownMapPoint.ResetToBase`, which is the only one of the
        // run's odds an act puts back.
        self.unknown_odds = UNKNOWN_ODDS;
        // Each act deals its own encounters and is met from the front of its
        // own list.
        self.normal_fights = 0;
        self.elite_fights = 0;
        self.events_used = 0;
        self.phase = Phase::Map;
        Ok(Decided::Resolved)
    }

    /// `RelicCmd.Obtain`: the relic joins the belt, and its own pickup runs.
    ///
    /// The model's `AfterObtained` state is written as the relic is built. A
    /// row that carries `UponPickup` then changes the run itself.
    ///
    /// A pickup that asks a question comes back rather than being answered
    /// here, because `AfterObtained` does not return until the answer does.
    /// The room the relic was picked up in is what parks on it.
    #[must_use]
    pub(super) fn obtain(&mut self, relic: V::Relic) -> Option<Asked<V>> {
        self.obtain_as(relic, false)
    }

    /// `RelicCmd.Obtain` of a relic set up for `character` first.
    fn obtain_named(&mut self, relic: V::Relic, character: V::Character) -> Option<Asked<V>> {
        self.obtain_held(relic, false, Some(character))
    }

    /// `RelicCmd.Obtain` of a relic that is wax where `wax` says so.
    pub(super) fn obtain_as(&mut self, relic: V::Relic, wax: bool) -> Option<Asked<V>> {
        self.obtain_held(relic, wax, None)
    }

    /// `RelicCmd.Obtain`, of a relic set up for `named` first where it names
    /// a character.
    fn obtain_held(
        &mut self,
        relic: V::Relic,
        wax: bool,
        named: Option<V::Character>,
    ) -> Option<Asked<V>> {
        use sts2_core::RelicTable as _;

        // Out of both bags, unless a run can hold more than one.
        if !relic.stackable() && !self.dealt_relics.contains(&relic) {
            Arc::make_mut(&mut self.dealt_relics).push(relic);
        }
        let mut held = HeldRelic {
            wax,
            ..HeldRelic::new(relic)
        };
        if let Some(character) = named {
            let mut asked = sts2_core::HookEffects::new();
            V::relic_event_hook(
                &mut held.state,
                sts2_core::EventHook::CharacterNamed(character),
                &mut asked,
            );
            if let Some(effect) = asked.first() {
                unimplemented!("{relic:?} asking for {effect:?} as it is set up");
            }
        }
        let effects = crate::combat::obtained::<V>(&mut held, self.character(), self.act);
        self.player.relics.push(held);
        self.carry_out(effects)
    }

    /// Carry out what a pickup asked the run for, in order, as far as the
    /// first effect that asks a question.
    ///
    /// The question comes back with the effects after it, which wait for the
    /// answer the way `AfterObtained` awaits it.
    pub(super) fn carry_out(
        &mut self,
        effects: impl IntoIterator<Item = sts2_core::HookEffect<V>>,
    ) -> Option<Asked<V>> {
        use sts2_core::HookEffect;

        let mut effects = effects.into_iter();
        while let Some(effect) = effects.next() {
            let pickup = match effect {
                // `DeckVersion`'s own setters, on the deck card while it is
                // still in the deck.
                HookEffect::WriteDeckCard { card, write } => {
                    if let Some(at) = self.player.deck.iter().position(|held| held.id == card) {
                        V::write_deck_card(&mut self.player.deck_mut()[at], write);
                    }
                    continue;
                }
                HookEffect::UpgradeRandomDeckCards {
                    count,
                    stream,
                    kind,
                } => {
                    self.upgrade_random_deck_cards(count, stream, kind);
                    continue;
                }
                HookEffect::GainGold(amount) => {
                    self.gain_gold(amount);
                    continue;
                }
                // `CardFactory.CreateForReward`, then `CardPileCmd.Add` of
                // each card to the deck.
                HookEffect::AddRolledToDeck(reward) => {
                    let pool = matches!(reward, sts2_core::CustomReward::CharacterCards { .. })
                        .then(|| self.reward_pool());
                    let stream = sts2_core::Stream::Player(V::PlayerRngType::REWARDS);
                    for card in self.custom_cards(reward, stream, pool.as_ref()) {
                        self.add_to_deck(card);
                    }
                    continue;
                }
                HookEffect::PurchaseEverything => {
                    self.purchase_everything(None);
                    continue;
                }
                HookEffect::MarkCombatPoints { relic, count, name } => {
                    let map = Arc::clone(&self.map);
                    self.mark_combat_points(&map, relic, count, name);
                    continue;
                }
                // `PotionCmd.Discard`.
                HookEffect::DiscardPotion(at) => {
                    self.drank(at);
                    self.belt_changed(sts2_core::EventHook::PotionDiscarded {
                        in_fight: matches!(self.phase, Phase::Combat(_)),
                        holding_potions: self.holding_potions(),
                    });
                    continue;
                }
                HookEffect::MimicRestSiteHeal => {
                    if let Some(asked) = self.rest_site_heal(true) {
                        let pickup = asked.pickup;
                        unimplemented!("a heal an event gives offering {pickup:?}");
                    }
                    continue;
                }
                HookEffect::FightInEvent { .. } => {
                    unreachable!("a fight in an event is the whole of its option")
                }
                HookEffect::FinishEvent => {
                    let Phase::Event { finished, .. } = &mut self.phase else {
                        unreachable!("only an event's option finishes it")
                    };
                    *finished = true;
                    continue;
                }
                HookEffect::ResumeEvent => {
                    // Lent out of the run for as long as it reads it, as a
                    // choice is.
                    let Phase::Event {
                        mut state,
                        mut rng,
                        asking,
                        finished,
                        layout,
                    } = std::mem::replace(&mut self.phase, Phase::Map)
                    else {
                        unreachable!("only an event's option resumes it")
                    };
                    let resumed = V::resume_event(&mut state, self, &mut rng);
                    self.phase = Phase::Event {
                        state,
                        rng,
                        asking,
                        finished,
                        layout,
                    };
                    match self.carry_out(resumed) {
                        None => continue,
                        Some(mut asked) => {
                            asked.rest.extend(effects);
                            return Some(asked);
                        }
                    }
                }
                HookEffect::LoseGold(amount) => {
                    let left = self.player.gold - amount;
                    self.player.gold = left.max(num_traits::zero());
                    continue;
                }
                HookEffect::LoseAllGold => {
                    self.player.gold = num_traits::zero();
                    continue;
                }
                HookEffect::OfferOtherPoolCards { rewards, pools } => {
                    let mut offered: Vec<Offer<V>> = (0..rewards)
                        .map(|_| Offer::Card {
                            cards: self.other_pool_cards(pools),
                            rolled: None,
                            reroll: false,
                            sacrifice: false,
                            taken: false,
                        })
                        .collect();
                    // `CardReward.Populate` over cards set by hand, which
                    // raises the reward's own pass with `IsCardReward`.
                    self.reward_offers(&mut offered);
                    self.modify_rewards(&mut offered, None);
                    crate::reward::sort(&mut offered);
                    Pickup::Rewards {
                        offered,
                        skippable: true,
                    }
                }
                HookEffect::ConfirmAbandonRun => Pickup::ConfirmAbandon,
                HookEffect::OfferBundles {
                    count,
                    commons,
                    uncommons,
                    same_card,
                } => Pickup::Bundles {
                    bundles: self.bundles(count, commons, uncommons, same_card),
                },
                // The relic's own pickup asks first, and the effects after
                // it wait behind what it asked.
                HookEffect::ObtainRelicFor { relic, character } => {
                    match self.obtain_named(relic, character) {
                        None => continue,
                        Some(mut asked) => {
                            asked.rest.extend(effects);
                            return Some(asked);
                        }
                    }
                }
                HookEffect::ObtainRelic(_) | HookEffect::ObtainNextRelic(_) => {
                    let relic = match effect {
                        HookEffect::ObtainRelic(relic) => relic,
                        HookEffect::ObtainNextRelic(pull) => {
                            let rarity =
                                pulled_rarity(pull, self.streams.player(V::PlayerRngType::REWARDS));
                            self.pull_relic_as(pull, rarity)
                        }
                        _ => unreachable!("matched above"),
                    };
                    match self.obtain(relic) {
                        None => continue,
                        Some(mut asked) => {
                            asked.rest.extend(effects);
                            return Some(asked);
                        }
                    }
                }
                HookEffect::AddToDeck(card) => {
                    self.add_to_deck(CardIdent::new(card));
                    continue;
                }
                HookEffect::AddDistinctToDeck {
                    cards,
                    count,
                    stream,
                } => {
                    use sts2_core::rng::GameRng as _;

                    let mut left = cards.to_vec();
                    for _ in 0..count {
                        let Ok(&card) = self.streams.run(stream).choose(&left) else {
                            break;
                        };
                        left.retain(|&held| held != card);
                        self.add_to_deck(CardIdent::new(card));
                    }
                    continue;
                }
                HookEffect::AddRegularCard(filter) => {
                    self.card_reward_unheard();
                    let character = self.character();
                    let pool = V::reward_cards(character);
                    let narrow = |cards: &[V::Card]| -> Vec<V::Card> {
                        cards
                            .iter()
                            .copied()
                            .filter(|&card| crate::reward::card::keeps::<V>(filter, card))
                            .collect()
                    };
                    let (common, uncommon, rare) = (
                        narrow(pool.common),
                        narrow(pool.uncommon),
                        narrow(pool.rare),
                    );
                    let drawn = crate::reward::card::regular::<V, _>(
                        self.streams.player(V::PlayerRngType::REWARDS),
                        sts2_core::Drafted {
                            common: &common,
                            uncommon: &uncommon,
                            rare: &rare,
                        },
                        1,
                        false,
                    );
                    for card in drawn {
                        let mut card = [CardIdent::new(card)];
                        self.reward_options(&mut card);
                        self.add_to_deck(card[0]);
                    }
                    continue;
                }
                HookEffect::AddUpgradedToDeck(card) => {
                    let mut ident = CardIdent::new(card);
                    if sts2_core::CardTable::max_upgrade(card) > 0 {
                        ident.upgrade = 1;
                    }
                    self.add_to_deck(ident);
                    continue;
                }
                HookEffect::RemoveDeckCardAt(at) => {
                    self.player.deck_mut().remove(at);
                    continue;
                }
                // `CardCmd.TransformTo`: each leaves its place, and the
                // replacements join the deck's end in deck order.
                HookEffect::TransformFirstInDeck(pairs) => {
                    let deck = self.player.deck_mut();
                    let found = deck.iter().enumerate().find_map(|(at, held)| {
                        pairs
                            .iter()
                            .find(|(from, _)| *from == held.ident.card)
                            .map(|&(_, into)| (at, into))
                    });
                    if let Some((at, into)) = found {
                        let held = deck.remove(at);
                        let mut ident = CardIdent::new(into);
                        ident.upgrade = u8::from(held.ident.upgrade > 0);
                        ident.enchantment = held.ident.enchantment;
                        self.add_to_deck(ident);
                    }
                    continue;
                }
                HookEffect::TransformInDeck { from, into } => {
                    let deck = self.player.deck_mut();
                    let before = deck.len();
                    deck.retain(|held| held.ident.card != from);
                    let transformed = before - deck.len();
                    for _ in 0..transformed {
                        self.add_to_deck(CardIdent::new(into));
                    }
                    continue;
                }
                HookEffect::AddUniformCard { rarity } => {
                    for card in self.uniform_cards(1, rarity) {
                        self.add_to_deck(CardIdent::new(card));
                    }
                    continue;
                }
                HookEffect::ReplaceRelic { from, to } => {
                    let Some(slot) = self
                        .player
                        .relics
                        .iter()
                        .position(|held| held.relic == from)
                    else {
                        unimplemented!("replacing {from:?}, which the run does not carry");
                    };
                    let mut held = HeldRelic::new(to);
                    let effects =
                        crate::combat::obtained::<V>(&mut held, self.character(), self.act);
                    self.player.relics.replace(slot, held);
                    if let Some(effect) = effects.first() {
                        unimplemented!("{to:?} asking for {effect:?} as it replaces {from:?}");
                    }
                    continue;
                }
                HookEffect::ReplaceStarterRelic { upgrades, fallback } => {
                    use sts2_core::{RarityKinds, RelicTable as _};

                    let Some(slot) = self
                        .player
                        .relics
                        .iter()
                        .position(|held| held.relic.rarity() == <_ as RarityKinds>::STARTER)
                    else {
                        unimplemented!("replacing a starter relic the run does not carry");
                    };
                    let starter = self.player.relics[slot].relic;
                    let upgraded = upgrades
                        .iter()
                        .find(|&&(from, _)| from == starter)
                        .map_or(fallback, |&(_, to)| to);
                    let mut held = HeldRelic::new(upgraded);
                    let effects =
                        crate::combat::obtained::<V>(&mut held, self.character(), self.act);
                    self.player.relics.replace(slot, held);
                    if let Some(effect) = effects.first() {
                        unimplemented!(
                            "{upgraded:?} asking for {effect:?} as it replaces {starter:?}"
                        );
                    }
                    continue;
                }
                HookEffect::DrawFromStream { stream, below } => {
                    use sts2_core::rng::GameRng as _;

                    let bound = sts2_core::rng::Bound::try_new(
                        num_traits::cast(below).expect("a bound fits the RNG integer"),
                    )
                    .expect("a draw has somewhere to land");
                    let _drawn =
                        stream_of(&mut self.phase, &mut self.streams, stream).next_int(bound);
                    continue;
                }
                HookEffect::RemoveRelic(relic) => {
                    let Some(slot) = self
                        .player
                        .relics
                        .iter()
                        .position(|held| held.relic == relic)
                    else {
                        unimplemented!("removing {relic:?}, which the run does not carry");
                    };
                    self.player.relics.remove(slot);
                    continue;
                }
                // The relic carries its option out itself.
                HookEffect::RestOptionOfRelic(relic) => {
                    let Some(held) = self
                        .player
                        .relics
                        .iter_mut()
                        .find(|held| held.relic == relic)
                    else {
                        unimplemented!("an option of {relic:?}, which the run does not carry");
                    };
                    let mut asked = sts2_core::HookEffects::new();
                    let taken = sts2_core::EventHook::RestOptionTaken;
                    V::relic_event_hook(&mut held.state, taken, &mut asked);
                    if let Some(effect) = asked.first() {
                        unimplemented!("{relic:?} asking for {effect:?} as its option is taken");
                    }
                    continue;
                }
                HookEffect::ProcurePotion(potion) => {
                    self.procure(potion);
                    continue;
                }
                HookEffect::GainPotionSlots(slots) => {
                    let slots: V::Count = num_traits::cast(slots)
                        .unwrap_or_else(|| unimplemented!("{slots} more potion slots"));
                    self.player.potion_slots = self.player.potion_slots + slots;
                    continue;
                }
                HookEffect::FillPotionSlots { stream } => {
                    let character = self.character();
                    // `HasOpenPotionSlots`, before each potion is made.
                    while self.player.potions.len()
                        < num_traits::cast::<_, usize>(self.player.potion_slots)
                            .expect("the belt has an addressable number of slots")
                    {
                        let potion = crate::reward::potion::distinct::<V, _>(
                            self.streams.run(stream),
                            character,
                            1,
                            false,
                        );
                        let Some(&potion) = potion.first() else {
                            break;
                        };
                        if !self.procure(potion) {
                            break;
                        }
                    }
                    continue;
                }
                // `TryToProcure` fails on a full belt, and the potion is lost.
                HookEffect::ProcureRandomPotions {
                    count,
                    stream,
                    in_combat,
                    into_gained,
                } => {
                    let character = self.character();
                    let potions = crate::reward::potion::distinct::<V, _>(
                        self.streams.run(stream),
                        character,
                        count,
                        in_combat,
                    );
                    let slots: usize = num_traits::cast(self.player.potion_slots)
                        .expect("the belt has an addressable number of slots");
                    for (at, potion) in potions.into_iter().enumerate() {
                        match into_gained {
                            true => self.procure_into(potion, slots - count + at),
                            false => self.procure(potion),
                        };
                    }
                    continue;
                }
                // `NextItem`, which draws nothing from an empty list and
                // names nothing to upgrade.
                HookEffect::UpgradeRandomDeckCard { stream } => {
                    use sts2_core::rng::GameRng as _;

                    let upgradable: Vec<usize> = self.upgradable().map(|(at, _)| at).collect();
                    let rng = stream_of(&mut self.phase, &mut self.streams, stream);
                    if let Ok(&at) = rng.choose(&upgradable) {
                        self.act_on_deck(sts2_core::DeckAct::Upgrade, &[at]);
                    }
                    continue;
                }
                // `NextItem` again. What the card saved stays, and its numbers
                // are worked out from it at the new level.
                HookEffect::DowngradeRandomDeckCard { stream } => {
                    use sts2_core::rng::GameRng as _;

                    let upgraded: Vec<usize> = self
                        .player
                        .deck
                        .iter()
                        .enumerate()
                        .filter(|(_, held)| held.ident.upgrade > 0)
                        .map(|(at, _)| at)
                        .collect();
                    let rng = stream_of(&mut self.phase, &mut self.streams, stream);
                    if let Ok(&at) = rng.choose(&upgraded) {
                        self.player.deck_mut()[at].ident.upgrade = 0;
                    }
                    continue;
                }
                HookEffect::EnchantBasicTagged {
                    tag,
                    enchantment,
                    amount,
                } => {
                    use sts2_core::{CardRarityKinds as _, CardTable as _};

                    let taken: Vec<usize> = (self.player.deck.iter().enumerate())
                        .filter(|(_, held)| {
                            held.ident.card.rarity() == <_>::BASIC
                                && held.ident.card.tagged(tag)
                                && V::can_enchant(enchantment, &held.ident)
                        })
                        .map(|(at, _)| at)
                        .collect();
                    let act = sts2_core::DeckAct::Enchant {
                        enchantment,
                        amount,
                        kind: None,
                    };
                    self.act_on_deck(act, &taken);
                    continue;
                }
                HookEffect::CloneEnchantedIntoDeck(enchantment) => {
                    let cloned: Vec<sts2_core::DeckCard<V>> = (self.player.deck.iter())
                        .filter(|held| {
                            held.ident
                                .enchantment
                                .is_some_and(|enchanted| enchanted.enchantment == enchantment)
                        })
                        .copied()
                        .collect();
                    for card in cloned {
                        let id = self.player.mint();
                        self.add_deck_card(sts2_core::DeckCard { id, ..card });
                    }
                    continue;
                }
                HookEffect::EnchantDeck {
                    enchantment,
                    amount,
                } => {
                    let taken: Vec<usize> = (self.player.deck.iter().enumerate())
                        .filter(|(_, held)| V::can_enchant(enchantment, &held.ident))
                        .map(|(at, _)| at)
                        .collect();
                    let act = sts2_core::DeckAct::Enchant {
                        enchantment,
                        amount,
                        kind: None,
                    };
                    self.act_on_deck(act, &taken);
                    continue;
                }
                // `NextItem` over what the relic keeps, off the rewards
                // stream. The card is made anew, upgraded where it can be,
                // then `CardPileCmd.Add` to the deck.
                HookEffect::ReturnHeldCard(relic) => {
                    use sts2_core::rng::GameRng as _;
                    use sts2_core::{CardTable as _, RelicModel as _};

                    let slot = (self.player.relics.iter())
                        .rposition(|held| held.relic == relic)
                        .unwrap_or_else(|| {
                            unimplemented!("{relic:?} handing back a card off the belt")
                        });
                    let count = self.player.relics[slot].state.held_cards().len();
                    let places: Vec<usize> = (0..count).collect();
                    let Ok(&at) = self
                        .streams
                        .player(V::PlayerRngType::REWARDS)
                        .choose(&places)
                    else {
                        continue;
                    };
                    let mut card = self.player.relics[slot].state.release_card(at);
                    if card.ident.upgrade < card.ident.card.max_upgrade() {
                        card.ident.upgrade += 1;
                    }
                    let id = self.player.mint();
                    self.add_deck_card(sts2_core::DeckCard { id, ..card });
                    continue;
                }
                // `State.Map` in place, and `RemoveStaleVisitedMapCoords`,
                // which keeps the points the new map still has.
                HookEffect::RegenerateMap => {
                    use sts2_core::ActMap as _;

                    let (act, _) = self.shared.opening.acts[self.act];
                    let map = self.generate_map(act);
                    self.visited.retain(|&at| map.at(at).is_ok());
                    self.map = Arc::new(map);
                    continue;
                }
                HookEffect::UpgradeDeck => {
                    let upgradable: Vec<usize> = self.upgradable().map(|(at, _)| at).collect();
                    self.act_on_deck(sts2_core::DeckAct::Upgrade, &upgradable);
                    continue;
                }
                HookEffect::OnDeckCards { act, picks } => {
                    let taken: Vec<usize> =
                        picks.iter().filter_map(|&pick| self.picked(pick)).collect();
                    self.act_on_deck(act, &taken);
                    continue;
                }
                HookEffect::GainMaxHp(amount) => {
                    let amount = self.health(amount);
                    self.player.max_hp = self.player.max_hp + amount;
                    self.heal(amount);
                    continue;
                }
                // The part of the health above the new maximum is lost as
                // unblockable damage first. The maximum never falls below one.
                HookEffect::LoseMaxHp(amount) => {
                    let amount = self.health(amount);
                    let one: V::Health = num_traits::one();
                    let max = if self.player.max_hp > amount {
                        self.player.max_hp - amount
                    } else {
                        num_traits::zero()
                    };
                    if self.player.hp > max {
                        self.damage_owner(self.player.hp - max);
                    }
                    self.player.max_hp = max.max(one);
                    self.player.hp = self.player.hp.min(self.player.max_hp);
                    continue;
                }
                HookEffect::DamageOwner(amount) => {
                    let amount = self.health(amount);
                    self.damage_owner(amount);
                    continue;
                }
                HookEffect::Heal(amount) => {
                    let amount = self.health(amount);
                    self.heal(amount);
                    continue;
                }
                HookEffect::HealMissing => {
                    self.heal(self.player.max_hp - self.player.hp);
                    continue;
                }
                // `SetCurrentHpInternal` truncates, and the health it adds to
                // is whole, so the share heals its whole part.
                HookEffect::HealPercent(percent) => {
                    let percent: V::Health =
                        num_traits::cast(percent).expect("a share of health fits the run's health");
                    let hundred: V::Health = num_traits::cast(100).expect("a hundred fits health");
                    self.heal(self.player.max_hp * percent / hundred);
                    continue;
                }
                HookEffect::OfferRewards(rewards) => Pickup::Rewards {
                    offered: self.custom_rewards(
                        rewards,
                        sts2_core::Stream::Player(V::PlayerRngType::REWARDS),
                    ),
                    skippable: true,
                },
                // `RewardsSet.WithSkippingDisallowed`, over a `RelicReward` for
                // each relic, which reports `RelicRarity.None`.
                HookEffect::OfferNeowRelics { count, except } => {
                    use sts2_core::rng::GameRng as _;

                    // A run here has one player, as Neow's own roll does.
                    let mut relics: Vec<V::Relic> = V::neow_options()
                        .filter(|&relic| relic != except && V::allowed_at_neow(relic, 1))
                        .collect();
                    self.streams
                        .player(V::PlayerRngType::REWARDS)
                        .unstable_shuffle(&mut relics);
                    Pickup::Rewards {
                        offered: relics
                            .into_iter()
                            .take(count)
                            .map(|relic| Offer::Relic {
                                relic,
                                rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
                                wax: false,
                                taken: false,
                            })
                            .collect(),
                        skippable: false,
                    }
                }
                HookEffect::OfferEventRewards => {
                    let Phase::Event { state, .. } = &self.phase else {
                        unreachable!("only an event offers rewards of its own")
                    };
                    let rewards = V::event_rewards(state);
                    Pickup::Rewards {
                        offered: self.custom_rewards(&rewards, sts2_core::Stream::Event),
                        skippable: true,
                    }
                }
                HookEffect::AskEvent => Pickup::Event,
                HookEffect::CloneDeckCard { excluding } => Pickup::CloneDeckCard { excluding },
                // `Deck.Cards[i]` for each place the deck had as it began, so
                // the clones are not cloned again.
                HookEffect::CloneDeck => {
                    for at in 0..self.player.deck.len() {
                        let card = self.player.deck[at].ident;
                        self.add_to_deck(card);
                    }
                    continue;
                }
                // `CardSelectCmd` takes the whole list without asking when it
                // holds no more than it must take, and returns nothing from an
                // empty one.
                HookEffect::TransformBasicStrikesAndDefends(stream) => {
                    use sts2_core::{CardRarityKinds, CardTable as _, CardTagKinds};

                    let taken: Vec<usize> = self
                        .player
                        .deck
                        .iter()
                        .enumerate()
                        .filter(|(_, held)| {
                            let card = held.ident.card;
                            card.rarity() == <_ as CardRarityKinds>::BASIC
                                && (card.tagged(<_ as CardTagKinds>::STRIKE)
                                    || card.tagged(<_ as CardTagKinds>::DEFEND))
                                && !V::card_eternal(&held.ident)
                        })
                        .map(|(at, _)| at)
                        .collect();
                    self.act_on_deck(sts2_core::DeckAct::Transform(stream), &taken);
                    continue;
                }
                // `FromDeckForEnchantment` sorts the cards it is handed back
                // into deck order, so the shuffle moves the stream alone.
                HookEffect::SelectFromShuffledDeck { act, count } => {
                    let mut offered = self.offered(act);
                    let rng = self.streams.run(V::RunRngType::NICHE);
                    sts2_core::rng::GameRng::unstable_shuffle(rng, &mut offered);
                    if offered.len() <= count {
                        let taken: Vec<usize> = offered.into_iter().map(|(at, _)| at).collect();
                        self.act_on_deck(act, &taken);
                        continue;
                    }
                    Pickup::FromDeck {
                        act,
                        min: count,
                        count,
                    }
                }
                HookEffect::SelectFromDeck { act, count } => {
                    let offered = self.offered(act);
                    if offered.len() <= count {
                        let taken: Vec<usize> = offered.into_iter().map(|(at, _)| at).collect();
                        self.act_on_deck(act, &taken);
                        continue;
                    }
                    Pickup::FromDeck {
                        act,
                        min: count,
                        count,
                    }
                }
                HookEffect::SelectFromDeckConfirmed { act, min, max } => {
                    if self.offered(act).is_empty() {
                        continue;
                    }
                    Pickup::FromDeck {
                        act,
                        min,
                        count: max,
                    }
                }
                HookEffect::ChooseCardsOfPool {
                    pool,
                    per_rarity,
                    min,
                    max,
                } => {
                    use sts2_core::RewardRarity;

                    let mut offered: Vec<CardIdent<V>> = Vec::with_capacity(3 * per_rarity);
                    for rarity in [
                        RewardRarity::Common,
                        RewardRarity::Uncommon,
                        RewardRarity::Rare,
                    ] {
                        // Each its own `CreateForReward`, whose hooks run on
                        // what that one made.
                        self.card_reward_unheard();
                        let mut drawn: Vec<V::Card> = Vec::with_capacity(per_rarity);
                        for _ in 0..per_rarity {
                            let card = self.uniform_card_of(pool, rarity, &drawn);
                            drawn.push(card);
                        }
                        let mut made: Vec<_> = drawn.into_iter().map(CardIdent::new).collect();
                        self.reward_options(&mut made);
                        offered.extend(made);
                    }
                    Pickup::Cards { offered, min, max }
                }
                HookEffect::ChooseCards {
                    draw,
                    count,
                    min,
                    max,
                } => {
                    let drawn = match draw {
                        sts2_core::CardDraw::Uniform(rarity) => self.uniform_cards(count, rarity),
                        sts2_core::CardDraw::Regular => {
                            self.card_reward_unheard();
                            let character = self.character();
                            crate::reward::card::regular::<V, _>(
                                self.streams.player(V::PlayerRngType::REWARDS),
                                V::reward_cards(character),
                                count,
                                false,
                            )
                        }
                    };
                    let mut offered: Vec<_> = drawn.into_iter().map(CardIdent::new).collect();
                    self.reward_options(&mut offered);
                    Pickup::Cards { offered, min, max }
                }
                effect => unimplemented!("a run-level relic effect: {effect:?}"),
            };
            return Some(Asked {
                pickup,
                rest: effects.collect(),
                under: None,
            });
        }
        None
    }

    /// Where in the deck `pick` names, if any card fits it.
    fn picked(&self, pick: sts2_core::DeckPick<V>) -> Option<usize> {
        use sts2_core::{CardRarityKinds, CardTable as _};

        let basic = |tag| {
            move |(_, card): &(usize, &CardIdent<V>)| {
                card.card.rarity() == CardRarityKinds::BASIC && card.card.tagged(tag)
            }
        };
        let mut deck = self.player.deck.iter().map(|held| &held.ident).enumerate();
        let found = match pick {
            sts2_core::DeckPick::FirstBasic(tag) => deck.find(basic(tag)),
            sts2_core::DeckPick::LastBasic(tag) => deck.rev().find(basic(tag)),
        };
        found.map(|(at, _)| at)
    }

    /// `PlayerCmd.GainGold`, outside a fight: `ModifyGoldGained` over the
    /// belt as a decimal, `AfterModifyingGoldGained`, then the whole part
    /// gained where any is left.
    ///
    /// # Panics
    ///
    /// A relic answering one of the hooks in a way nothing here carries out.
    fn gain_gold(&mut self, amount: V::Number) {
        let hook = sts2_core::ModifyHook::GoldGained;
        let mut gained = crate::combat::exact::<V>(amount);
        let mut left = self
            .player
            .relics
            .hearing(sts2_core::Overrides::of(hook.model_hook()));
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            let listener: crate::combat::Heard<'_, V> =
                sts2_core::Listener::Relic(&self.player.relics[slot].state);
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Scaled(scale) => gained *= scale.exact(),
                // The listener's own number in place of what it was handed.
                sts2_core::Modify::Replaced(amount) => gained = crate::combat::exact::<V>(amount),
                answer => unimplemented!("a relic answering {hook:?} with {answer:?}"),
            }
        }
        if let Some(effect) = self
            .notify_belt(sts2_core::EventHook::GoldGainModified)
            .first()
        {
            unimplemented!("a relic asking for {effect:?} as a gold gain is modified");
        }
        // `PlayerCmd.GainGold` returns before the gold lands where nothing is
        // left to gain, so `AfterGoldGained` runs only after a gain.
        if gained > sts2_core::Decimal::ZERO {
            self.player.gold = self.player.gold + crate::combat::whole::<V>(gained);
            if self
                .deck_cards_hearing(sts2_core::ModelHook::AfterGoldGained)
                .next()
                .is_some()
            {
                unimplemented!("a deck card listening to AfterGoldGained");
            }
            let asked = self.notify_belt(sts2_core::EventHook::GoldGained);
            if let Some(asked) = self.carry_out(asked) {
                let pickup = asked.pickup;
                unimplemented!("a relic asking for {pickup:?} as gold is gained");
            }
        }
    }

    /// A game number as the run's health.
    ///
    /// # Panics
    ///
    /// A number no health can hold.
    fn health(&self, amount: V::Number) -> V::Health {
        num_traits::cast(amount).unwrap_or_else(|| unimplemented!("{amount:?} as a health"))
    }

    /// `CreatureCmd.Heal` outside a fight, which stops at the maximum. With
    /// no fight's state it raises no `AfterCurrentHpChanged`.
    fn heal(&mut self, amount: V::Health) {
        self.player.hp = (self.player.hp + amount).min(self.player.max_hp);
    }

    /// The player's hit points, as a hook hands them to a relic.
    fn player_health(&self) -> sts2_core::PlayerHealth<V> {
        sts2_core::PlayerHealth {
            hp: self.player.hp,
            max_hp: self.player.max_hp,
        }
    }

    /// `CreatureCmd.Damage` on the player outside a fight: unblockable,
    /// unpowered and from nobody.
    ///
    /// Damage that takes the last hit point kills the player, which ends the
    /// run where it stands.
    ///
    /// # Panics
    ///
    /// A relic listening to a hook the damage or the death raises.
    fn damage_owner(&mut self, amount: V::Health) {
        use sts2_core::ModelHook;

        // `Hook.ModifyDamage` asks each relic, for damage no creature deals,
        // from no card, and unpowered. A run with no fight names no target.
        for hook in [
            sts2_core::ModifyHook::DamageAdditive {
                target: None,
                dealer: None,
                card: None,
                powered: false,
            },
            sts2_core::ModifyHook::DamageMultiplicative {
                target: None,
                dealer: None,
                card: None,
                powered: false,
            },
            sts2_core::ModifyHook::DamageCap {
                target: None,
                dealer: None,
                powered: false,
            },
        ] {
            self.belt_leaves(hook);
        }
        for hook in [
            ModelHook::BeforeDamageReceived,
            ModelHook::ModifyHpLostBeforeOsty,
            ModelHook::ModifyHpLostBeforeOstyLate,
        ] {
            self.unheard(hook);
        }
        // `LoseHpInternal`: no more than the player holds, and a hit that
        // takes the last of it kills.
        let amount = self.hp_lost_after_osty(amount).min(self.player.hp);
        let killed = amount == self.player.hp && amount > num_traits::zero();
        self.player.hp = self.player.hp - amount;
        // `Hook.AfterCurrentHpChanged`, for damage that took hit points.
        if amount > num_traits::zero() {
            let hook = sts2_core::EventHook::CurrentHpChanged {
                creature: sts2_core::CombatId::new(nonmax::NonMaxU8::ZERO),
                monster: None,
                pet_of: None,
                delta: -num_traits::cast::<_, V::Number>(amount)
                    .expect("a health fits a game number"),
                in_fight: false,
                player: self.player_health(),
            };
            if let Some(effect) = self.notify_belt(hook).first() {
                unimplemented!("a relic asking for {effect:?} as damage lands outside a fight");
            }
        }
        // `CreatureCmd.Kill`: `BeforeDeath`, `ShouldDie`, then `AfterDeath`.
        // With every player dead, the run is over, which the player's hit
        // points at nothing now say.
        if killed {
            self.unheard(ModelHook::BeforeDeath);
            let preventer = self.death_preventer();
            self.unheard(ModelHook::AfterDeath);
            if let Some(slot) = preventer {
                self.death_prevented(slot);
            }
        }
    }

    /// `Hook.ShouldDieLate` for the player outside a fight: the slot of the
    /// first relic on the belt that keeps them alive.
    fn death_preventer(&self) -> Option<usize> {
        let player = sts2_core::CombatId::new(nonmax::NonMaxU8::ZERO);
        let hook = sts2_core::DecisionHook::ShouldDieLate {
            creature: player,
            player,
        };
        let hooks = sts2_core::Overrides::of(sts2_core::ModelHook::ShouldDieLate);
        let mut left = self.player.relics.hearing(hooks);
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            let held = &self.player.relics[slot].state;
            if V::decide_hook(crate::combat::Heard::<V>::Relic(held), hook)
                .is_decision_and(|dies| !dies)
            {
                return Some(slot);
            }
        }
        None
    }

    /// `AfterPreventingDeath`, asked of the relic in `slot` alone, once it
    /// kept the player alive outside a fight.
    ///
    /// # Panics
    ///
    /// The relic asking for anything but a heal.
    fn death_prevented(&mut self, slot: usize) {
        let max_hp: V::Number =
            num_traits::cast(self.player.max_hp).expect("maximum health fits a game number");
        let hook = sts2_core::EventHook::DeathPrevented { max_hp };
        let held = &mut self.player.relics[slot];
        let relic = held.relic;
        let mut asked = sts2_core::HookEffects::new();
        V::relic_event_hook(&mut held.state, hook, &mut asked);
        for effect in asked {
            match effect {
                sts2_core::HookEffect::Heal(amount) => {
                    let amount = self.health(amount);
                    self.heal(amount);
                }
                effect => {
                    unimplemented!("{relic:?} asking for {effect:?} as it keeps the player alive")
                }
            }
        }
    }

    /// `ModifyHpLost`'s two phases after Osty, as the belt answers them for
    /// the player outside a fight, floored at nothing. The player is the
    /// only creature there, so the target and the player are one.
    ///
    /// # Panics
    ///
    /// A relic answering with more than an amount to add.
    fn hp_lost_after_osty(&self, amount: V::Health) -> V::Health {
        let player = sts2_core::CombatId::new(nonmax::NonMaxU8::ZERO);
        let mut lost: V::Number = num_traits::cast(amount).expect("a health fits a game number");
        for hook in [
            sts2_core::ModifyHook::HpLostAfterOsty {
                target: player,
                player,
                in_fight: false,
            },
            sts2_core::ModifyHook::HpLostAfterOstyLate {
                target: player,
                player,
                dealer: None,
                osty: None,
                powered: false,
            },
        ] {
            let mut left = self
                .player
                .relics
                .hearing(sts2_core::Overrides::of(hook.model_hook()));
            while left != 0 {
                let slot = left.trailing_zeros() as usize;
                left &= left - 1;
                let listener: crate::combat::Heard<'_, V> =
                    sts2_core::Listener::Relic(&self.player.relics[slot].state);
                match V::modify_hook(listener, hook) {
                    sts2_core::Modify::Nothing => {}
                    sts2_core::Modify::Added(more) => lost = lost + more,
                    answer => {
                        unimplemented!("a relic answering {hook:?} outside a fight with {answer:?}")
                    }
                }
            }
        }
        num_traits::cast(lost.max(num_traits::zero())).expect("a loss fits the build's health")
    }

    /// `HealRestSiteOption.ExecuteRestSiteHeal`: three tenths of the maximum
    /// after `ModifyRestSiteHealAmount`, `AfterRestSiteHeal`, then the rewards
    /// a relic adds, which `RewardsCmd.OfferCustom` puts on screen where there
    /// are any. `mimicked` for an event's heal.
    ///
    /// # Panics
    ///
    /// A relic changing the amount other than by adding to it, or asking for
    /// something the run cannot carry out here.
    fn rest_site_heal(&mut self, mimicked: bool) -> Option<Asked<V>> {
        let max_hp: u64 = num_traits::cast(self.player.max_hp)
            .expect("maximum health fits an unsigned rest calculation");
        let mut healed: V::Number =
            num_traits::cast(max_hp * 3 / 10).expect("rest healing fits a game number");
        let hook = sts2_core::ModifyHook::RestSiteHealAmount;
        let mut left = self
            .player
            .relics
            .hearing(sts2_core::Overrides::of(hook.model_hook()));
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            let listener: crate::combat::Heard<'_, V> =
                sts2_core::Listener::Relic(&self.player.relics[slot].state);
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Added(more) => healed = healed + more,
                answer => unimplemented!("a relic answering {hook:?} with {answer:?}"),
            }
        }
        let healed: V::Health =
            num_traits::cast(healed).expect("rest healing fits the build's health type");
        self.heal(healed);
        let asked = self.notify_belt(sts2_core::EventHook::RestSiteHeal { mimicked });
        if let Some(asked) = self.carry_out(asked) {
            let pickup = asked.pickup;
            unimplemented!("a rest site's heal asking for {pickup:?}");
        }
        let rewards = self.notify_belt(sts2_core::EventHook::RestSiteHealRewards { mimicked });
        self.carry_out(rewards)
    }

    /// `RewardsSet.WithCustomRewards` and `GenerateWithoutOffering`, off
    /// `stream`. The potions are drawn as their rewards are made. Each reward
    /// then fills in in the order given, and the hooks and the sort follow.
    fn custom_rewards(
        &mut self,
        rewards: &[sts2_core::CustomReward<V>],
        stream: sts2_core::Stream<V>,
    ) -> Vec<Offer<V>> {
        use sts2_core::CustomReward;
        use sts2_core::rng::GameRng as _;

        let character = self.character();
        if rewards.iter().any(|reward| {
            matches!(
                reward,
                CustomReward::CharacterCards { .. }
                    | CustomReward::ColorlessCards(_)
                    | CustomReward::CardsOf { .. }
                    | CustomReward::UpgradedCardsOf { .. }
            )
        }) {
            self.card_reward_unheard();
        }
        // `new PotionReward(potion, player)`: the potion is drawn before the
        // set fills anything in.
        let mut potions = SmallVec::<[V::Potion; 4]>::new();
        for reward in rewards {
            let rng = stream_of(&mut self.phase, &mut self.streams, stream);
            let pool = match *reward {
                CustomReward::PotionOf(rarity) => V::reward_potions(character).at(rarity),
                CustomReward::UnlockedPotion => V::unlocked_potions(character),
                _ => continue,
            };
            potions.push(
                *rng.choose(pool)
                    .unwrap_or_else(|_| unimplemented!("no potion to offer for {reward:?}")),
            );
        }
        let mut potions = potions.into_iter();
        // A relic reward of a set rarity draws nothing, so its pulls can come
        // first.
        let mut relics = rewards
            .iter()
            .filter_map(|reward| match *reward {
                CustomReward::Relic(rarity) => Some(rarity),
                _ => None,
            })
            .map(|rarity| (self.pull_relic(rarity), rarity))
            .collect::<SmallVec<[(V::Relic, Rarity<V>); 4]>>()
            .into_iter();
        // The pools a character's card reward draws from, which no draw
        // moves, so worked out before the set fills in.
        let pool = (rewards.iter())
            .any(|reward| {
                matches!(
                    reward,
                    CustomReward::CharacterCards { .. } | CustomReward::MonsterRoomCards
                )
            })
            .then(|| self.reward_pool());
        // What a fight's own card reward adds, which the monster room's
        // reward reads the same way.
        let added =
            match (rewards.iter()).any(|reward| matches!(reward, CustomReward::MonsterRoomCards)) {
                true => self.card_reward_added(),
                false => Vec::new(),
            };
        let ascension = self.ascension();
        let act = self.act();
        let mut offered: Vec<Offer<V>> = Vec::with_capacity(rewards.len());
        for reward in rewards {
            let rng = stream_of(&mut self.phase, &mut self.streams, stream);
            offered.push(match *reward {
                CustomReward::Relic(_) => {
                    let (relic, rarity) = relics.next().expect("each relic reward was pulled");
                    Offer::Relic {
                        relic,
                        rarity,
                        wax: false,
                        taken: false,
                    }
                }
                CustomReward::RolledRelic | CustomReward::WaxRelic => {
                    let rarity = crate::reward::relic_rarity::<V, _>(rng);
                    Offer::Relic {
                        relic: self.pull_relic(rarity),
                        rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
                        wax: matches!(reward, CustomReward::WaxRelic),
                        taken: false,
                    }
                }
                CustomReward::Gold(amount) => Offer::Gold {
                    amount: crate::reward::fixed_gold::<V, _>(rng, amount),
                    taken: false,
                },
                reward @ (CustomReward::CharacterCards { .. }
                | CustomReward::ColorlessCards(_)
                | CustomReward::CardsOf { .. }
                | CustomReward::UpgradedCardsOf { .. }) => Offer::Card {
                    cards: self.custom_cards(reward, stream, pool.as_ref()),
                    rolled: Some(crate::reward::Rolled::Custom { reward, stream }),
                    reroll: false,
                    sacrifice: false,
                    taken: false,
                },
                CustomReward::MonsterRoomCards => crate::reward::combat::card_reward::<V, _>(
                    rng,
                    &mut self.odds,
                    ascension,
                    pool.as_ref()
                        .expect("a monster room's reward has its pool")
                        .drafted(),
                    act,
                    crate::reward::combat::Room::Monster,
                    &added,
                ),
                CustomReward::SpecificPotion(potion) => Offer::Potion {
                    potion,
                    taken: false,
                },
                CustomReward::PotionOf(_) | CustomReward::UnlockedPotion => Offer::Potion {
                    potion: potions.next().expect("each potion reward drew its potion"),
                    taken: false,
                },
                CustomReward::Potion => Offer::Potion {
                    potion: crate::reward::potion::rolled::<V, _>(rng, character),
                    taken: false,
                },
            });
        }
        self.reward_offers(&mut offered);
        self.modify_rewards(&mut offered, None);
        crate::reward::sort(&mut offered);
        offered
    }

    /// The pack a custom card reward rolls, off `stream`. `pool` is what a
    /// reward over the character's own pool draws from.
    ///
    /// # Panics
    ///
    /// A reward that makes no pack, or a character's reward with no pool.
    fn custom_cards(
        &mut self,
        reward: sts2_core::CustomReward<V>,
        stream: sts2_core::Stream<V>,
        pool: Option<&crate::reward::card::Pool<V>>,
    ) -> Vec<CardIdent<V>> {
        use sts2_core::CustomReward;

        let (character, act) = (self.character(), self.act());
        let rng = stream_of(&mut self.phase, &mut self.streams, stream);
        match reward {
            CustomReward::CharacterCards {
                count,
                upgrade_roll,
            } => crate::reward::card::regular::<V, _>(
                rng,
                pool.expect("a character's card reward has its pools")
                    .drafted(),
                count,
                upgrade_roll,
            )
            .into_iter()
            .map(CardIdent::new)
            .collect(),
            CustomReward::ColorlessCards(count) => {
                crate::reward::card::regular::<V, _>(rng, V::colorless_cards(), count, false)
                    .into_iter()
                    .map(CardIdent::new)
                    .collect()
            }
            CustomReward::CardsOf {
                pool,
                rarity,
                count,
                upgrade_roll,
            } => crate::reward::card::uniform_rolled::<V, _>(
                rng,
                V::reward_cards(pool.unwrap_or(character)).at(rarity),
                count,
                upgrade_roll.then_some(act),
            ),
            CustomReward::UpgradedCardsOf {
                rarity,
                kind,
                count,
            } => crate::reward::card::upgraded_of::<V, _>(
                rng,
                &V::merchant_cards(character, kind, rarity),
                count,
            ),
            reward => unreachable!("{reward:?} makes no pack of cards"),
        }
    }

    /// `Hook.ModifyRewards`, once a set is filled in. A reward it adds is
    /// filled in after, and the set's sort puts it among its own kind.
    ///
    /// # Panics
    ///
    /// A relic modifying rewards some other way, or late.
    pub(super) fn modify_rewards(
        &mut self,
        offered: &mut Vec<Offer<V>>,
        room: Option<V::RoomType>,
    ) {
        let hook = sts2_core::EventHook::ModifyRewards {
            room,
            act: self.act(),
            acts: self.shared.opening.acts.len(),
        };
        for effect in self.notify_belt(hook) {
            match effect {
                sts2_core::HookEffect::AddRelicRewards(count) => {
                    for _ in 0..count {
                        let (rng, _) = self.rewarding();
                        let rarity = crate::reward::relic_rarity::<V, _>(rng);
                        let relic = self.pull_relic(rarity);
                        offered.push(Offer::Relic {
                            relic,
                            rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
                            wax: false,
                            taken: false,
                        });
                    }
                }
                // `CardReward.Populate` after the hook: three cards at the
                // odds of the room named.
                sts2_core::HookEffect::AddCardReward(room) => {
                    use crate::reward::combat::{CARDS_OFFERED, Room};
                    use sts2_core::RoomKinds as _;

                    let room = match room {
                        room if room == V::RoomType::ELITE => Room::Elite,
                        room if room == V::RoomType::BOSS => Room::Boss,
                        _ => Room::Monster,
                    };
                    let ascension = self.ascension();
                    let act = self.act();
                    self.card_reward_unheard();
                    let pool = self.reward_pool();
                    let (rng, odds) = self.rewarding();
                    let mut cards = crate::reward::card::encounter::<V, _>(
                        rng,
                        odds,
                        ascension,
                        pool.drafted(),
                        CARDS_OFFERED,
                        act,
                        room.card_odds(),
                    );
                    self.reward_options(&mut cards);
                    offered.push(Offer::Card {
                        cards,
                        rolled: Some(crate::reward::Rolled::Room(room)),
                        reroll: false,
                        sacrifice: false,
                        taken: false,
                    });
                }
                // `GoldReward(amount)` sets its amount as it is built, so it
                // is populated already and draws nothing.
                sts2_core::HookEffect::AddGoldReward(amount) => offered.push(Offer::Gold {
                    amount,
                    taken: false,
                }),
                effect => unimplemented!("a relic modifying rewards with {effect:?}"),
            }
        }
        let late = self.notify_belt(sts2_core::EventHook::ModifyRewardsLate);
        for effect in late {
            match effect {
                sts2_core::HookEffect::RerollableCardRewards => {
                    for offer in offered.iter_mut() {
                        if let Offer::Card { reroll, .. } = offer {
                            *reroll = true;
                        }
                    }
                }
                effect => unimplemented!("a relic modifying rewards late with {effect:?}"),
            }
        }
        self.card_reward_alternatives(offered);
    }

    /// `Hook.ModifyCardRewardAlternatives` over each card reward of a set.
    fn card_reward_alternatives(&mut self, offered: &mut [Offer<V>]) {
        for offer in offered {
            let Offer::Card { sacrifice, .. } = offer else {
                continue;
            };
            for effect in self.notify_belt(sts2_core::EventHook::CardRewardAlternatives) {
                match effect {
                    sts2_core::HookEffect::OfferRewardAlternative(
                        sts2_core::RewardAlternative::Sacrifice,
                    ) => *sacrifice = true,
                    effect => unimplemented!("a relic adding {effect:?} to a card reward"),
                }
            }
        }
    }

    /// `CardPileCmd.Add` into the deck, at its bottom, and then
    /// `AfterCardChangedPiles` on the belt.
    ///
    /// # Panics
    ///
    /// A relic asking for anything but a clone of the card.
    pub(super) fn add_to_deck(&mut self, card: CardIdent<V>) {
        let id = self.player.mint();
        self.add_deck_card(sts2_core::DeckCard::new(id, card));
    }

    /// [`State::add_to_deck`] for a card that carries what its model saved.
    pub(super) fn add_deck_card(&mut self, mut card: sts2_core::DeckCard<V>) {
        use sts2_core::{EventHook, HookEffect};

        // `Hook.ModifyCardBeingAddedToDeck`, both passes.
        card.ident = self.rewritten(card.ident, |card| EventHook::CardBeingAddedToDeck { card });
        self.unheard(sts2_core::ModelHook::TryModifyCardBeingAddedToDeckLate);

        self.player.deck_mut().push(card);
        let hook = EventHook::AddedToDeck {
            card: card.ident.card,
            cloned: false,
        };
        for effect in self.notify_belt(hook) {
            match effect {
                HookEffect::CloneIntoDeck => {
                    let clone = sts2_core::DeckCard {
                        id: self.player.mint(),
                        ..card
                    };
                    self.player.deck_mut().push(clone);
                    let hook = EventHook::AddedToDeck {
                        card: card.ident.card,
                        cloned: true,
                    };
                    for effect in self.notify_belt(hook) {
                        self.card_joined_deck(effect);
                    }
                }
                effect => self.card_joined_deck(effect),
            }
        }
    }

    /// What a relic asks for as a card joins the deck, other than a copy of
    /// it.
    ///
    /// # Panics
    ///
    /// Anything but gold, a heal or more maximum health.
    fn card_joined_deck(&mut self, effect: sts2_core::HookEffect<V>) {
        match effect {
            sts2_core::HookEffect::GainGold(amount) => self.gain_gold(amount),
            sts2_core::HookEffect::Heal(amount) => {
                let healed: V::Health =
                    num_traits::cast(amount).expect("a relic's heal fits the build's health");
                self.heal(healed);
            }
            // `CreatureCmd.GainMaxHp`: the maximum grows, then a heal of as
            // much.
            sts2_core::HookEffect::GainMaxHp(amount) => {
                let amount = self.health(amount);
                self.player.max_hp = self.player.max_hp + amount;
                self.heal(amount);
            }
            effect => unimplemented!("a relic asking for {effect:?} as a card joins the deck"),
        }
    }

    /// `hook`, raised on each relic on the belt that overrides it, in slot
    /// order, and what they ask the run for.
    ///
    /// A run with no fight has no other listeners.
    pub(super) fn notify_belt(
        &mut self,
        hook: sts2_core::EventHook<V>,
    ) -> sts2_core::HookEffects<V> {
        let mut asked = sts2_core::HookEffects::new();
        let Some(model_hook) = hook.model_hook() else {
            return asked;
        };
        let mut left = self.player.relics.hearing_hook(model_hook);
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            V::relic_event_hook(&mut self.player.relics[slot].state, hook, &mut asked);
        }
        asked
    }

    /// `hook`, asked of each relic on the belt that overrides it, outside a
    /// fight.
    ///
    /// # Panics
    ///
    /// A relic that changes the value.
    fn belt_leaves(&self, hook: sts2_core::ModifyHook<V>) {
        let mut left = self
            .player
            .relics
            .hearing(sts2_core::Overrides::of(hook.model_hook()));
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            let listener: crate::combat::Heard<'_, V> =
                sts2_core::Listener::Relic(&self.player.relics[slot].state);
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                answer => {
                    unimplemented!("a relic answering {hook:?} outside a fight with {answer:?}")
                }
            }
        }
    }

    /// The deck's cards whose class overrides `hook`, which the run asks
    /// through its content. A relic listening to it stops by name.
    ///
    /// # Panics
    ///
    /// A relic on the belt, or a deck card's enchantment, listening to
    /// `hook`.
    pub(super) fn deck_hearing(
        &self,
        hook: sts2_core::ModelHook,
    ) -> impl Iterator<Item = V::Card> + '_ {
        if self.player.relics.hear(sts2_core::Overrides::of(hook)) {
            unimplemented!("a relic listening to {} outside a fight", hook.name());
        }
        self.deck_cards_hearing(hook)
    }

    /// [`Self::deck_hearing`], for a hook the caller also raises on the belt.
    ///
    /// # Panics
    ///
    /// A deck card's enchantment listening to `hook`.
    fn deck_cards_hearing(&self, hook: sts2_core::ModelHook) -> impl Iterator<Item = V::Card> + '_ {
        use sts2_core::{CardTable as _, EnchantmentTable as _};

        let hooks = sts2_core::Overrides::of(hook);
        if self.player.deck.iter().any(|held| {
            held.ident
                .enchantment
                .is_some_and(|enchanted| enchanted.enchantment.overrides().intersects(hooks))
        }) {
            unimplemented!(
                "an enchantment listening to {} outside a fight",
                hook.name()
            );
        }
        self.player
            .deck
            .iter()
            .map(|held| held.ident.card)
            .filter(move |card| card.overrides().intersects(hooks))
    }

    /// A hook the run raises outside a fight and no ported listener answers.
    ///
    /// # Panics
    ///
    /// A relic on the belt, a deck card or its enchantment listening to
    /// `hook`.
    pub(super) fn unheard(&self, hook: sts2_core::ModelHook) {
        use sts2_core::{CardTable as _, EnchantmentTable as _};

        let hooks = sts2_core::Overrides::of(hook);
        if self.player.relics.hear(hooks) {
            unimplemented!("a relic listening to {} outside a fight", hook.name());
        }
        // `IterateHookListeners` walks the deck's cards and their
        // enchantments ahead of the relics. Most hooks no card hears.
        if !V::Card::heard_by_any().intersects(hooks)
            && self
                .player
                .deck
                .iter()
                .all(|held| held.ident.enchantment.is_none())
        {
            return;
        }
        let heard = self.player.deck.iter().find(|held| {
            held.ident.card.overrides().intersects(hooks)
                || held
                    .ident
                    .enchantment
                    .is_some_and(|enchanted| enchanted.enchantment.overrides().intersects(hooks))
        });
        if let Some(held) = heard {
            let card = held.ident.card;
            unimplemented!(
                "{card:?} in the deck listening to {} outside a fight",
                hook.name()
            );
        }
    }

    /// `CardFactory.CreateForReward` at `Uniform` odds for cards of one
    /// rarity, off the rewards stream, no two the same.
    ///
    /// Each is one `NextItem` over the character's reward pool at that
    /// rarity, less what this call drew already. `NoUpgradeRoll` rolls
    /// nothing else.
    fn uniform_cards(&mut self, count: usize, rarity: sts2_core::RewardRarity) -> Vec<V::Card> {
        self.card_reward_unheard();
        let mut drawn: Vec<V::Card> = Vec::with_capacity(count);
        for _ in 0..count {
            let card = self.uniform_card(rarity, &drawn);
            drawn.push(card);
        }
        drawn
    }

    /// One card from each of `pools` other characters' pools, for
    /// [`sts2_core::HookEffect::OfferOtherPoolCards`].
    ///
    /// `Character::ALL` lists the characters in id order, which is the order
    /// `StableShuffle` sorts their pools into first.
    fn other_pool_cards(&mut self, pools: usize) -> Vec<CardIdent<V>> {
        use sts2_core::rng::GameRng as _;
        use sts2_core::{CharacterTable as _, Table as _};

        let own = self.character();
        let mut others: Vec<V::Character> = V::Character::ALL
            .iter()
            .copied()
            .filter(|&character| character.playable() && character != own)
            .collect();
        self.streams
            .run(V::RunRngType::NICHE)
            .unstable_shuffle(&mut others);
        others
            .into_iter()
            .take(pools)
            .map(|character| {
                // `CardFactory.CreateForReward` of one card, with its hooks.
                self.card_reward_unheard();
                let rng = self.streams.player(V::PlayerRngType::REWARDS);
                let [card] =
                    crate::reward::card::regular::<V, _>(rng, V::reward_cards(character), 1, true)
                        [..]
                else {
                    unreachable!("one card drawn is one card");
                };
                self.rewritten(CardIdent::new(card), |card| {
                    sts2_core::EventHook::CardRewardOption {
                        card,
                        card_reward: false,
                    }
                })
            })
            .collect()
    }

    /// `ScrollBoxes.GenerateRandomBundles`: each bundle `commons` commons then
    /// `uncommons` uncommons, `NextItem` off the rewards stream over the
    /// cards no bundle holds yet. See [`sts2_core::HookEffect::OfferBundles`].
    fn bundles(
        &mut self,
        count: usize,
        commons: usize,
        uncommons: usize,
        same_card: Option<(V::Card, u32)>,
    ) -> Vec<Vec<V::Card>> {
        use sts2_core::rng::{Bound, GameRng as _};

        let pool = V::reward_cards(self.character());
        let rng = self.streams.player(V::PlayerRngType::REWARDS);
        let mut used: Vec<V::Card> = Vec::new();
        let mut bundles = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some((card, percent)) = same_card {
                let hundred = Bound::try_new(100).expect("a hundred is a bound");
                let under = i32::try_from(percent).expect("a chance out of a hundred fits");
                if rng.next_int(hundred) < under {
                    bundles.push(vec![card; commons + uncommons]);
                    continue;
                }
            }
            let mut bundle = Vec::with_capacity(commons + uncommons);
            for (rarity, take) in [
                (sts2_core::RewardRarity::Common, commons),
                (sts2_core::RewardRarity::Uncommon, uncommons),
            ] {
                let mut left: Vec<V::Card> = pool
                    .at(rarity)
                    .iter()
                    .copied()
                    .filter(|card| !used.contains(card))
                    .collect();
                for _ in 0..take {
                    let Ok(&card) = rng.choose(&left) else {
                        unimplemented!("a reward pool with no {rarity:?} card left for a bundle");
                    };
                    left.retain(|&held| held != card);
                    used.push(card);
                    bundle.push(card);
                }
            }
            bundles.push(bundle);
        }
        bundles
    }

    /// One `NextItem` over the character's reward pool at `rarity`, less
    /// what the same call drew already.
    fn uniform_card(&mut self, rarity: sts2_core::RewardRarity, drawn: &[V::Card]) -> V::Card {
        self.uniform_card_of(self.character(), rarity, drawn)
    }

    /// [`State::uniform_card`] over another character's pool.
    fn uniform_card_of(
        &mut self,
        character: V::Character,
        rarity: sts2_core::RewardRarity,
        drawn: &[V::Card],
    ) -> V::Card {
        use sts2_core::rng::GameRng as _;

        let pool = V::reward_cards(character).at(rarity);
        let left: Vec<V::Card> = pool
            .iter()
            .copied()
            .filter(|card| !drawn.contains(card))
            .collect();
        *self
            .streams
            .player(V::PlayerRngType::REWARDS)
            .choose(&left)
            .unwrap_or_else(|_| unimplemented!("a reward pool with no {rarity:?} card left"))
    }

    /// The pools a `CardReward` over the character's own draws from, once
    /// `Hook.ModifyCardRewardCreationOptions` has had its say.
    ///
    /// # Panics
    ///
    /// A relic asking for anything but more pools.
    pub(super) fn reward_pool(&mut self) -> crate::reward::card::Pool<V> {
        use crate::reward::card::Source;

        let own = self.character();
        let mut pools: SmallVec<[Source<V>; 6]> = smallvec::smallvec![Source::Character(own)];
        for effect in self.notify_belt(sts2_core::EventHook::CardRewardCreationOptions) {
            match effect {
                // `Union`: the new pools first, then what was there and is
                // not among them.
                sts2_core::HookEffect::AddCharacterPools(added) => {
                    let mut joined: SmallVec<[Source<V>; 6]> = added
                        .iter()
                        .map(|&character| Source::Character(character))
                        .collect();
                    let rest: SmallVec<[Source<V>; 6]> = pools
                        .iter()
                        .copied()
                        .filter(|held| !joined.contains(held))
                        .collect();
                    joined.extend(rest);
                    pools = joined;
                }
                // `Concat` of the colorless pool, where it is not there.
                sts2_core::HookEffect::AddColorlessPool => {
                    if !pools.contains(&Source::Colorless) {
                        pools.push(Source::Colorless);
                    }
                }
                effect => {
                    unimplemented!("a relic asking for {effect:?} over a card reward's pools")
                }
            }
        }
        crate::reward::card::Pool::of(&pools)
    }

    /// `CardFactory.CreateForReward`'s hooks that no relic here answers: the
    /// upgrade odds, and the first pass over the cards it made. The late pass
    /// is [`State::reward_options`].
    ///
    /// # Panics
    ///
    /// A relic listening to one of them.
    pub(super) fn card_reward_unheard(&mut self) {
        self.unheard(sts2_core::ModelHook::ModifyCardRewardUpgradeOdds);
        if let Some(effect) = self
            .notify_belt(sts2_core::EventHook::CardRewardOptions { encounter: false })
            .first()
        {
            unimplemented!("a relic asking for {effect:?} over a card reward");
        }
    }

    /// `TryModifyCardRewardOptions` over a won fight's card reward: the kind
    /// of each option the belt adds.
    pub(super) fn card_reward_added(&mut self) -> Vec<sts2_core::CardKind> {
        self.unheard(sts2_core::ModelHook::ModifyCardRewardUpgradeOdds);
        self.notify_belt(sts2_core::EventHook::CardRewardOptions { encounter: true })
            .into_iter()
            .map(|effect| match effect {
                sts2_core::HookEffect::AddCardRewardOption(kind) => kind,
                effect => unimplemented!("a relic asking for {effect:?} over a card reward"),
            })
            .collect()
    }

    /// `TryModifyCardRewardOptionsLate` over each card a reward offers, then
    /// `AfterModifyingCardRewardOptions`.
    ///
    /// The game asks the second only of a listener that answered the first.
    /// Each listener here answers it where its own first answer was yes, for
    /// every reward that comes here is a `CardReward`'s.
    pub(super) fn reward_options(&mut self, cards: &mut [CardIdent<V>]) {
        use sts2_core::rng::GameRng as _;

        // Relic by relic, in the belt's order, each over the whole reward:
        // one relic's card can be another's to enchant or not.
        let hook = sts2_core::ModelHook::TryModifyCardRewardOptionsLate;
        let mut left = self.player.relics.hearing(sts2_core::Overrides::of(hook));
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            for card in cards.iter_mut() {
                let mut asked = sts2_core::HookEffects::new();
                let option = sts2_core::EventHook::CardRewardOption {
                    card: *card,
                    card_reward: true,
                };
                V::relic_event_hook(&mut self.player.relics[slot].state, option, &mut asked);
                *card = rewrite(*card, asked);
            }
            let mut asked = sts2_core::HookEffects::new();
            let pick = sts2_core::EventHook::CardRewardPick { card_reward: true };
            V::relic_event_hook(&mut self.player.relics[slot].state, pick, &mut asked);
            for effect in asked {
                match effect {
                    sts2_core::HookEffect::EnchantOneRewardCard {
                        enchantment,
                        amount,
                    } => {
                        let eligible: Vec<usize> = (0..cards.len())
                            .filter(|&at| V::can_enchant(enchantment, &cards[at]))
                            .collect();
                        let rng = self.streams.run(V::RunRngType::NICHE);
                        if let Ok(&at) = rng.choose(&eligible) {
                            V::enchant(enchantment, amount, &mut cards[at]);
                        }
                    }
                    effect => unimplemented!("a relic asking for {effect:?} over a card reward"),
                }
            }
        }
        let asked = self.notify_belt(sts2_core::EventHook::CardRewardOptionsModified);
        if let Some(effect) = asked.first() {
            unimplemented!("a relic asking for {effect:?} once a card reward was modified");
        }
    }

    /// [`State::reward_options`] over every card reward in `offered`.
    pub(super) fn reward_offers(&mut self, offered: &mut [Offer<V>]) {
        for offer in offered {
            if let Offer::Card { cards, .. } = offer {
                self.reward_options(cards);
            }
        }
    }

    /// `card` after each relic's answer to `hook`: a clone upgraded, which
    /// takes its place.
    ///
    /// # Panics
    ///
    /// A relic asking for anything else.
    pub(super) fn rewritten(
        &mut self,
        mut card: CardIdent<V>,
        hook: impl Fn(CardIdent<V>) -> sts2_core::EventHook<V>,
    ) -> CardIdent<V> {
        card = rewrite(card, self.notify_belt(hook(card)));
        card
    }

    /// Pick a relic up where nothing can park on what its pickup asks.
    ///
    /// Every room but the shop hands a relic over with no way to suspend the
    /// handover. A pickup that asks a question there stops by name rather than
    /// be answered by whoever happens to be holding the run.
    fn obtain_here(&mut self, relic: V::Relic) {
        if let Some(asked) = self.obtain(relic) {
            let pickup = asked.pickup;
            unimplemented!("{relic:?} asking for {pickup:?} where it was picked up");
        }
    }

    /// Put the run back to the health it started with.
    ///
    /// `CreatureCmd.Heal(MaxHp - CurrentHp)`, which is what an ancient gives a
    /// run before it offers anything.
    pub(super) fn heal_fully(&mut self) {
        self.heal(self.player.max_hp - self.player.hp);
    }

    /// The ancient this act opens on, which it rolled with its rooms.
    pub(super) fn act_ancient(&self) -> V::Event {
        self.shared.opening.acts[self.act()].1.ancient
    }

    pub(super) fn event_candidates(&self) -> &[V::Event] {
        &self.shared.opening.acts[self.act()].1.events
    }

    pub(super) fn event_state(&self) -> (usize, &[V::Event], [f32; 3]) {
        (self.events_used, &self.visited_events, self.unknown_odds)
    }

    /// `RunManager.EnterRoom`: the run leaves the map and stands in a room.
    ///
    /// `at` is the map point the run walked to. `EnterRoomDebug` passes
    /// nothing, because it enters at `MapPointType.Unassigned`. The run then
    /// gains the floor and still stands where it already stood.
    ///
    /// The room is built first. Building one can fail, and a room that cannot
    /// be built leaves the run where it stood.
    pub(super) fn enter(&mut self, at: Location, phase: Phase<V>) {
        let (strayed, point) = (self.strays(at), self.point_type(at));
        self.floor = self.floor + num_traits::one();
        if let Location::MapPoint(to) = at {
            self.visited.push(to);
        }
        // `ActModel.MarkRoomVisited`, which counts an event room against
        // the act's queue.
        if let Phase::Event { .. } = phase {
            self.events_used += 1;
        }
        // A fight raised its own as it opened.
        let room = match &phase {
            Phase::Event { .. } => Some(<V::RoomType as sts2_core::RoomKinds>::EVENT),
            Phase::Shop { .. } => Some(<V::RoomType as sts2_core::RoomKinds>::SHOP),
            Phase::Treasure { .. } => Some(<V::RoomType as sts2_core::RoomKinds>::TREASURE),
            Phase::RestSite { .. } => Some(<V::RoomType as sts2_core::RoomKinds>::REST_SITE),
            _ => None,
        };
        self.phase = phase;
        if let Some(room) = room {
            self.room_entered(room, strayed, point);
        }
    }

    /// `AfterRoomEntered` for a room no fight opens in.
    ///
    /// # Panics
    ///
    /// A relic asking for something the run cannot carry out as it enters.
    fn room_entered(&mut self, room: V::RoomType, strayed: bool, point: Option<V::MapPointType>) {
        let asked = self.notify_belt(sts2_core::EventHook::RoomEntered {
            room,
            strayed,
            point,
            deck: self.player.deck.len(),
            player: self.player_health(),
            // A room no fight opens in is never one an event opened.
            base: true,
        });
        if let Some(asked) = self.carry_out(asked) {
            let pickup = asked.pickup;
            unimplemented!("entering a room asking for {pickup:?}");
        }
    }

    /// The type of the map point at `at`. None off the map.
    fn point_type(&self, at: Location) -> Option<V::MapPointType> {
        use sts2_core::ActMap as _;

        let Location::MapPoint(to) = at else {
            return None;
        };
        self.map.at(to).ok().map(|node| node.kind)
    }

    /// Whether going to `at` leaves the map's lines: a point the one the run
    /// stands on does not lead on to. The first point of an act strays from
    /// nothing.
    fn strays(&self, at: Location) -> bool {
        use sts2_core::ActMap as _;

        let (Location::MapPoint(to), Some(&from)) = (at, self.visited.last()) else {
            return false;
        };
        self.map.at(from).is_ok_and(|from| !from.to.contains(&to))
    }

    /// The fight a `Combat`-layout event builds, then `GenerateInitialOptions`:
    /// the event's own draws, what its options set up off the player's
    /// rewards stream, and the relics it pulls.
    pub(super) fn open_event(
        &mut self,
        event: V::Event,
        rng: &mut crate::rng::MegaRng,
        streams: &mut Streams<V>,
    ) -> Result<(V::EventState, Layout<V>), Error> {
        use sts2_core::EventTable as _;

        // `GenerateInternalCombatState`, before the options generate, on the
        // floor the room is about to stand on.
        let layout = match event.combat_layout() {
            Some(encounter) => crate::combat::roll_layout::<V>(
                self.raw_seed(),
                self.floor + num_traits::one(),
                self.ascension(),
                encounter,
                streams,
            )?,
            None => SmallVec::new(),
        };
        let mut state = V::enter_event(event, self, rng);
        V::set_up_event(&mut state, self, streams.player(V::PlayerRngType::REWARDS));
        let pulled: SmallVec<[V::Relic; 4]> = V::event_relics_pulled(&state)
            .iter()
            .map(|&pull| {
                let rarity = pulled_rarity(pull, streams.player(V::PlayerRngType::REWARDS));
                self.pull_relic_as(pull, rarity)
            })
            .collect();
        if !pulled.is_empty() {
            V::hand_event_pulled_relics(&mut state, &pulled);
        }
        V::stock_event(&mut state, streams.player(V::PlayerRngType::SHOPS));
        Ok((state, layout))
    }

    pub(super) fn enter_event(&mut self, to: Coord, entered: EnteredEvent<V>) {
        let EnteredEvent {
            event,
            state,
            layout,
            rng,
            streams,
            events_used,
            unknown_odds,
        } = entered;
        self.streams = streams;
        self.events_used = events_used;
        self.unknown_odds = unknown_odds;
        Arc::make_mut(&mut self.visited_events).push(event);
        self.enter(
            Location::MapPoint(to),
            Phase::Event {
                state,
                rng,
                asking: None,
                finished: false,
                layout,
            },
        );
    }

    /// Enter a treasure room: `AfterRoomEntered`, then `BeginRelicPicking`.
    /// That rolls the chest's relic off its own stream with `RollRarity`,
    /// where `ShouldGenerateTreasure` lets it.
    pub(super) fn enter_treasure(&mut self, at: Location, streams: Streams<V>) {
        self.streams = streams;
        self.enter(
            at,
            Phase::Treasure {
                relic: None,
                finished: false,
            },
        );
        let relic = self.generates_treasure().then(|| {
            let rarity = crate::reward::relic_rarity::<V, _>(
                self.streams.run(V::RunRngType::TREASURE_ROOM_RELICS),
            );
            let relic = self.next_shared_relic(rarity);
            Arc::make_mut(&mut self.dealt_relics).push(relic);
            relic
        });
        if let Phase::Treasure {
            relic: chest,
            finished,
        } = &mut self.phase
        {
            *chest = relic;
            *finished = relic.is_none();
        }
    }

    /// `NTreasureRoom.OpenChest`'s `DoNormalRewards`, which runs as the
    /// player first answers the chest: `OneOffSynchronizer.
    /// DoTreasureRoomRewards`. Where `ShouldGenerateTreasure` lets it, that is
    /// `NextInt(42, 53)` gold off the player's Rewards stream, three quarters
    /// of it under Poverty, then `TryHandleSpoilsMap`.
    fn open_chest(&mut self) {
        use sts2_core::rng::GameRng as _;
        use sts2_core::{AscensionKinds as _, PlayerStreamKinds as _};

        if !self.generates_treasure() {
            return;
        }
        let span = sts2_core::rng::Span::new(CHEST_GOLD.0, CHEST_GOLD.1)
            .expect("the chest's gold is a span");
        let rolled = self
            .streams
            .player(V::PlayerRngType::REWARDS)
            .next_int_range(span);
        let mut gold = f64::from(rolled);
        if self
            .ascension()
            .has(<V::AscensionLevel as sts2_core::AscensionKinds>::POVERTY)
        {
            gold *= POVERTY_GOLD;
        }
        // `(int)gold`, which truncates.
        self.gain_gold(num_traits::cast(gold.trunc()).expect("a chest's gold is a game number"));
        self.spoils_paid();
    }

    /// `TryHandleSpoilsMap`, where the run stands on the Spoils Map's quest
    /// point. Each Spoils Map in the deck pays its gold and leaves the deck
    /// through `OnQuestComplete`, and the quest is done.
    fn spoils_paid(&mut self) {
        let (Some(&at), Some((point, card, gold))) = (self.visited.last(), self.spoils) else {
            return;
        };
        if at != point {
            return;
        }
        let paying: Vec<usize> = (self.player.deck.iter().enumerate())
            .filter(|(_, held)| held.ident.card == card)
            .map(|(at, _)| at)
            .collect();
        for &at in paying.iter().rev() {
            self.gain_gold(gold);
            self.player.deck_mut().remove(at);
        }
        self.spoils = None;
    }

    /// `Hook.ShouldGenerateTreasure`, which any relic on the belt can answer
    /// no to.
    fn generates_treasure(&self) -> bool {
        let hook = sts2_core::DecisionHook::ShouldGenerateTreasure;
        let hooks = sts2_core::Overrides::of(sts2_core::ModelHook::ShouldGenerateTreasure);
        let mut left = self.player.relics.hearing(hooks);
        let mut generates = true;
        while left != 0 {
            let held = &self.player.relics[left.trailing_zeros() as usize];
            left &= left - 1;
            generates &= !V::decide_hook(crate::combat::Heard::<V>::Relic(&held.state), hook)
                .is_decision_and(|generate| !generate);
        }
        generates
    }

    /// `MerchantEntry.OnTryPurchaseWrapper`: buy the entry at an index.
    ///
    /// Stock and gold are the two checks the wrapper makes, and both are
    /// already reported. A slot it sells clears rather than restocks, because
    /// `ShouldRefillMerchantEntry` is a hook and no ported relic answers it.
    ///
    /// The card removal is the one entry whose stock does not clear. Once used,
    /// it shows unstocked, and its price climbs at the next merchant.
    pub(crate) fn buy(&mut self, index: u8) -> Result<Decided, Error> {
        if let Phase::Event { .. } = self.phase {
            return Ok(self.buy_in_event(index));
        }
        let Phase::Shop { entries, .. } = &self.phase else {
            unimplemented!("buying outside a shop");
        };
        let Some(entry) = entries.iter().find(|entry| entry.index == index) else {
            unimplemented!("buying a shelf this merchant does not have");
        };
        if !entry.stocked {
            unimplemented!("buying a shelf this merchant has sold");
        }
        let (stock, cost) = (entry.stock, self.merchant_price(entry.cost));
        if cost > self.player.gold {
            unimplemented!("buying a shelf this run cannot afford");
        }
        let spent: u32 = num_traits::cast(cost).expect("a price is no less than nothing");

        // `PlayerCmd.LoseGold` runs after the entry's own purchase in the
        // game. Nothing between the two reads the gold, so it is spent here,
        // and handed back by a purchase that fails.
        self.player.gold = self.player.gold - cost;
        let asking = match stock {
            sts2_core::Stock::Card(Some(on_sale)) => {
                self.add_to_deck(on_sale.card);
                None
            }
            sts2_core::Stock::Relic(Some(relic)) => {
                self.obtain(relic).map(|asked| match asked.pickup {
                    Pickup::CloneDeckCard { excluding } if asked.rest.is_empty() => {
                        Unfinished::Pickup {
                            shelf: index,
                            excluding,
                            parasol: false,
                            spent,
                        }
                    }
                    Pickup::CloneDeckCard { .. } => {
                        unimplemented!("a clone asked for with more behind it in a shop")
                    }
                    // `AfterObtained` awaits the pickup, as any room's does.
                    _ => {
                        let Phase::Shop { asking, .. } = &mut self.phase else {
                            unreachable!("the phase was a shop when this began");
                        };
                        *asking = Some(asked);
                        Unfinished::Relic {
                            shelf: index,
                            parasol: false,
                            spent,
                        }
                    }
                })
            }
            // `TryToProcure` refuses where a relic's `ShouldProcurePotion`
            // says no, and the purchase fails with it. Nothing is spent and
            // the shelf keeps its potion. A full belt is refused before any
            // purchase, as the game's own check is.
            sts2_core::Stock::Potion(Some(potion)) => {
                if !self.procure(potion) {
                    self.player.gold = self.player.gold + cost;
                    return Ok(Decided::Resolved);
                }
                None
            }
            // `DoLocalMerchantCardRemoval` opens a cancelable selection and
            // spends the gold once a card comes back, so nothing is spent on
            // this one until then.
            sts2_core::Stock::CardRemoval => {
                self.player.gold = self.player.gold + cost;
                Some(Unfinished::Removal { free: false })
            }
            stock => unimplemented!("buying {stock:?}"),
        };

        // `ClearAfterPurchase` runs once the entry's own purchase returns, so a
        // shelf whose purchase is still asking stays stocked until it does.
        match asking {
            Some(_) => {
                let Phase::Shop { unfinished, .. } = &mut self.phase else {
                    unreachable!("the phase was a shop when this began");
                };
                *unfinished = asking;
            }
            None => self.purchased(index, cost),
        }
        // A purchase that has not returned leaves the decision unresolved,
        // which is what the game reports.
        Ok(match asking {
            Some(_) => Decided::Abandoned,
            None => Decided::Resolved,
        })
    }

    /// A purchase from the shop an event's room is, which the event carries
    /// out.
    ///
    /// # Panics
    ///
    /// An entry the room does not sell or the run cannot afford.
    fn buy_in_event(&mut self, index: u8) -> Decided {
        let Some(Room::Shop { entries }) = self.event_room() else {
            unimplemented!("buying in an event that is not a shop");
        };
        let Some(entry) = entries.iter().find(|entry| entry.index == index) else {
            unimplemented!("buying a shelf this event does not have");
        };
        if !entry.stocked || !entry.affordable {
            unimplemented!("buying a shelf this event cannot sell");
        }
        let Phase::Event { state, .. } = &mut self.phase else {
            unreachable!("the phase was an event when this began");
        };
        let effects = V::buy_in_event(state, index);
        match self.carry_out(effects) {
            Some(next) => self.park(next),
            None => Decided::Resolved,
        }
    }

    /// Finish the purchase this merchant is waiting on.
    ///
    /// One entry point, because the phase holds one unfinished purchase. Which
    /// of the two it is decides what the answer means and what clears after it.
    fn finish_purchase(&mut self, answer: &Answer) -> Result<Decided, Error> {
        let Phase::Shop { unfinished, .. } = &self.phase else {
            unreachable!("the phase was a shop when this began");
        };
        let unfinished = unfinished.expect("the merchant was waiting when this began");
        let Answer::Cards(picks) = answer else {
            unimplemented!("an answer to a purchase that is not a card selection");
        };
        let Some(index) = picks.only() else {
            unimplemented!("a purchase answered with other than one card");
        };
        match unfinished {
            Unfinished::Removal { free } => self.removed(index, free),
            Unfinished::Pickup {
                shelf,
                excluding,
                parasol,
                spent,
            } => {
                self.cloned(index, shelf, excluding, Self::gold(spent));
                if parasol {
                    // The question has its answer, and the purchases go on
                    // with no pickup waiting.
                    let Phase::Shop { unfinished, .. } = &mut self.phase else {
                        unreachable!("the phase was a shop when this began");
                    };
                    *unfinished = None;
                    self.purchase_everything(Some(shelf));
                    return Ok(Decided::Resolved);
                }
            }
            Unfinished::Relic { .. } => {
                unreachable!("a pickup the shop parks on answers as a room's")
            }
        }
        let Phase::Shop { unfinished, .. } = &mut self.phase else {
            unreachable!("the phase was a shop when this began");
        };
        *unfinished = None;
        Ok(Decided::Resolved)
    }

    /// `CloneCard`, then `CardPileCmd.Add` to the deck.
    ///
    /// The selection offers the deck less what the relic's filter names, so the
    /// index is into that narrowed list rather than into the deck.
    fn cloned(
        &mut self,
        index: usize,
        shelf: u8,
        excluding: sts2_core::CardKind,
        spent: V::Number,
    ) {
        let offered = self.clonable(excluding);
        let Some(card) = offered.get(index).copied() else {
            unimplemented!("a clone naming a card that was not offered");
        };
        self.add_to_deck(card);
        // `OnTryPurchaseWrapper` has returned now, so the shelf the relic came
        // off empties or restocks.
        self.purchased(shelf, spent);
    }

    /// Take the card the merchant's removal was pointed at out of the deck.
    /// A `free` removal spends nothing, and counts among the removals used.
    ///
    /// The index is the removal list's, which is the deck less its `Eternal`
    /// cards, curses first.
    fn removed(&mut self, index: usize, free: bool) {
        let Some(&(at, _)) = self.offered(sts2_core::DeckAct::Remove).get(index) else {
            unimplemented!("a removal naming a card the selection did not offer");
        };
        self.player.deck_mut().remove(at);
        // `DoLocalMerchantCardRemoval` spends the gold once the card comes
        // back, and `CardShopRemovalsUsed` climbs over the run.
        let spent = match free {
            true => num_traits::zero(),
            false => self.merchant_price(removal_cost::<V>(self.removals_used)),
        };
        self.player.gold = self.player.gold - spent;
        self.removals_used += 1;
        // `NMerchantCardRemoval.OnCardRemovalUsed` calls `SetUsed`, and
        // `IsStocked` is `!Used`, so this merchant sells no second removal.
        // The next merchant reads the raised count.
        if let Phase::Shop { entries, .. } = &mut self.phase {
            for entry in entries
                .iter_mut()
                .filter(|entry| entry.stock == sts2_core::Stock::CardRemoval)
            {
                entry.stocked = false;
            }
        }
        self.item_purchased(spent);
    }

    /// `LordsParasol.PurchaseEverything`, as its owner enters a merchant.
    /// `OnTryPurchaseWrapper` with `ignoreCost` on each shelf, in order. A
    /// sold-out shelf or a refused potion buys nothing. The card removal comes
    /// last, and cannot be cancelled.
    ///
    /// A relic's pickup that asks stands the rest still. They go on from the
    /// shelf `after` it once the pickup is done.
    ///
    /// # Panics
    ///
    /// A relic whose pickup asks for more behind a copy of a deck card.
    fn purchase_everything(&mut self, after: Option<u8>) {
        let Phase::Shop { entries, .. } = &self.phase else {
            unimplemented!("Lord's Parasol buying outside a merchant's room");
        };
        let shelves: Vec<(u8, sts2_core::Stock<V>, bool)> = entries
            .iter()
            .filter(|entry| after.is_none_or(|after| entry.index > after))
            .map(|entry| (entry.index, entry.stock, entry.stocked))
            .collect();
        let mut removal = false;
        for (index, stock, stocked) in shelves {
            match stock {
                sts2_core::Stock::CardRemoval => {
                    removal = stocked;
                    continue;
                }
                _ if !stocked => continue,
                sts2_core::Stock::Card(Some(on_sale)) => self.add_to_deck(on_sale.card),
                sts2_core::Stock::Relic(Some(relic)) => {
                    if let Some(asked) = self.obtain(relic) {
                        let Phase::Shop {
                            asking, unfinished, ..
                        } = &mut self.phase
                        else {
                            unreachable!("the phase was a shop when this began");
                        };
                        *unfinished = Some(match asked.pickup {
                            Pickup::CloneDeckCard { excluding } if asked.rest.is_empty() => {
                                Unfinished::Pickup {
                                    shelf: index,
                                    excluding,
                                    parasol: true,
                                    spent: 0,
                                }
                            }
                            Pickup::CloneDeckCard { .. } => {
                                unimplemented!("a clone asked for with more behind it in a shop")
                            }
                            _ => {
                                *asking = Some(asked);
                                Unfinished::Relic {
                                    shelf: index,
                                    parasol: true,
                                    spent: 0,
                                }
                            }
                        });
                        return;
                    }
                }
                sts2_core::Stock::Potion(Some(potion)) => {
                    if !self.procure(potion) {
                        continue;
                    }
                }
                stock => unimplemented!("Lord's Parasol buying {stock:?}"),
            }
            // `ignoreCost`, which spends nothing.
            self.purchased(index, num_traits::zero());
        }
        if removal {
            let Phase::Shop { unfinished, .. } = &mut self.phase else {
                unreachable!("the phase was a shop when this began");
            };
            *unfinished = Some(Unfinished::Removal { free: true });
        }
    }

    /// The abandon-run popup's answer. A yes is `GuaranteeKillAllPlayers`,
    /// which leaves the run with a dead player and nothing more to decide. A
    /// no closes the popup on the page it opened from, whose options stay.
    ///
    /// # Panics
    ///
    /// An answer other than a yes or a no, or effects queued behind the popup.
    fn confirmed_abandon(&mut self, answer: &Answer) -> Decided {
        let Answer::Confirm(yes) = *answer else {
            unimplemented!("answering the abandon-run popup with {answer:?}");
        };
        let asked = self
            .phase
            .asking_mut()
            .and_then(Option::take)
            .expect("the popup was asking when this began");
        if !asked.rest.is_empty() || asked.under.is_some() {
            unimplemented!("effects waiting behind the abandon-run popup");
        }
        if yes {
            self.player.hp = num_traits::zero();
        }
        Decided::Resolved
    }

    /// The deck as a pickup's selection offers it.
    fn clonable(&self, excluding: sts2_core::CardKind) -> Vec<CardIdent<V>> {
        self.player
            .deck
            .iter()
            .map(|held| held.ident)
            .filter(|card| V::card_kind(card.card) != Some(excluding))
            .collect()
    }

    /// Put the run into a room of a named type, rather than travel to one.
    ///
    /// `EnterRoomDebug` at `MapPointType.Unassigned`. The floor advances and
    /// the map does not, the way a constructed fight's does.
    ///
    /// A shop is the one type this builds. It is stocked the way a shop the
    /// run travelled to is, so a scenario stands in front of the shop a run
    /// walks into.
    pub(crate) fn enter_room(&mut self, room: V::RoomType) -> Result<Decided, Error> {
        if room == <V::RoomType as sts2_core::RoomKinds>::REST_SITE {
            self.enter_rest_site(Location::NoLocation);
            return Ok(Decided::Resolved);
        }
        if room == <V::RoomType as sts2_core::RoomKinds>::TREASURE {
            self.enter_treasure(Location::NoLocation, self.streams);
            return Ok(Decided::Resolved);
        }
        if room != <V::RoomType as sts2_core::RoomKinds>::SHOP {
            unimplemented!("constructing a {room:?} room");
        }
        let shop = self.stocked_shop();
        self.enter(Location::NoLocation, shop);
        Ok(Decided::Resolved)
    }

    /// `MerchantInventory.CreateForNormalMerchant`, which runs as the room
    /// opens. So every draw it takes is paid for on the way in, not at a shelf.
    ///
    /// Built rather than entered, because what a merchant stocks does not
    /// depend on how the run reached it.
    pub(super) fn stocked_shop(&mut self) -> Phase<V> {
        let mut streams = self.streams;
        let mut dealt = self.dealt_relics.to_vec();
        let entries = super::shop::Merchant::<V> {
            streams: &mut streams,
            player_relics: &self.shared.opening.player_relics,
            dealt_relics: &mut dealt,
            character: self.shared.character,
            offset: self.odds.rarity,
            removals_used: self.removals_used,
        }
        .stock();

        self.streams = streams;
        self.dealt_relics = Arc::new(dealt);
        // `Hook.ModifyMerchantCardCreationResults` over the cards it stocked.
        let entries = entries
            .into_iter()
            .map(|mut entry| {
                if let sts2_core::Stock::Card(Some(on_sale)) = &mut entry.stock {
                    on_sale.card = self.rewritten(on_sale.card, |card| {
                        sts2_core::EventHook::MerchantCardCreated { card }
                    });
                }
                entry
            })
            .collect();
        Phase::Shop {
            entries,
            unfinished: None,
            asking: None,
        }
    }

    /// Enter a rest site, with the options this run's belt and deck open it on.
    pub(super) fn enter_rest_site(&mut self, at: Location) {
        let deck: Vec<CardIdent<V>> = self.player.deck.iter().map(|held| held.ident).collect();
        // `TryModifyRestSiteOptions`, asked of the belt ahead of the deck.
        let from_belt: Vec<V::RestSiteOption> = self
            .notify_belt(sts2_core::EventHook::RestSiteOptions)
            .into_iter()
            .map(|effect| match effect {
                sts2_core::HookEffect::OfferRestOption(option) => option,
                effect => unimplemented!("a relic asking for {effect:?} as a rest site opens"),
            })
            .collect();
        let options = <V::RestSiteOption as sts2_core::RestKinds<V>>::offered(&from_belt, &deck);
        self.enter(
            at,
            Phase::RestSite {
                options,
                chosen: None,
                rested: false,
                upgrading: false,
                asking: None,
            },
        );
    }

    /// Enter a fight that is already built, where the run reached it.
    ///
    /// The act counts the fight here rather than where it was built, because
    /// the count is how far down its list the run has walked. A fight the run
    /// never walked into moves no list on.
    pub(super) fn enter_fight(&mut self, fight: CreatedFight<V>, at: Location) -> Decided {
        let CreatedFight {
            live,
            streams,
            queue,
            ended,
            arriving,
        } = fight;
        match queue {
            EncounterQueue::Normal => self.normal_fights += 1,
            EncounterQueue::Elite => self.elite_fights += 1,
            EncounterQueue::Boss | EncounterQueue::Named => {}
        }
        self.drew(streams);
        self.enter(at, Phase::Combat(live));
        self.arrived(arriving);
        // A fight its opening ended goes straight on, the way one a decision
        // ended does.
        self.settle(ended, 0)
    }

    /// `UnknownMapPointOdds.Roll`: a room other than an event, or nothing for
    /// an event.
    ///
    /// A shop is barred when the point the run leaves held one, or when every
    /// point after `to` is a shop, which is `BuildRoomTypeBlacklist`. The room
    /// rolled goes back to its base odds, and every other room not barred
    /// rises by its base.
    ///
    /// The odds are the run's own, and move here.
    ///
    /// # Panics
    ///
    /// A relic changing which rooms can roll, or how far the odds rise.
    pub(super) fn roll_unknown_room(
        &mut self,
        to: Coord,
        streams: &mut Streams<V>,
    ) -> Option<UnknownRoom> {
        use sts2_core::rng::GameRng as _;
        use sts2_core::{ActMap as _, MapPointKinds as _};

        // `Hook.ModifyUnknownMapPointRoomTypes`, which a deck card can narrow
        // to an event alone. The roll still draws.
        let narrowed = self
            .deck_cards_hearing(sts2_core::ModelHook::ModifyUnknownMapPointRoomTypes)
            .any(|card| V::deck_card_narrows_unknown_rooms(card, self.act));
        // The same hook on the belt, where a relic takes a room type out.
        let mut removed: SmallVec<[V::RoomType; 2]> = SmallVec::new();
        for effect in self.notify_belt(sts2_core::EventHook::UnknownRoomTypes { act: self.act }) {
            match effect {
                sts2_core::HookEffect::RemoveRoomType(room) => removed.push(room),
                effect => unimplemented!("a relic asking for {effect:?} as an unknown room rolls"),
            }
        }
        let removes = |room| removed.contains(&room);
        self.unheard(sts2_core::ModelHook::ModifyOddsIncreaseForUnrolledRoomType);
        let node = |at| self.map.at(at).expect("a map edge leads to a node");
        let after = &node(to).to;
        let all_shops = !after.is_empty()
            && after
                .iter()
                .all(|&at| node(at).kind == V::MapPointType::SHOP);
        // The room the run leaves is the one it still stands in.
        let in_shop = matches!(self.phase, Phase::Shop { .. });
        let barred = [
            narrowed || removes(<V::RoomType as sts2_core::RoomKinds>::MONSTER),
            narrowed || removes(<V::RoomType as sts2_core::RoomKinds>::TREASURE),
            narrowed
                || in_shop
                || all_shops
                || removes(<V::RoomType as sts2_core::RoomKinds>::SHOP),
        ];
        let odds = &mut self.unknown_odds;
        let rooms = [
            UnknownRoom::Monster,
            UnknownRoom::Treasure,
            UnknownRoom::Shop,
        ];
        let roll = streams.run(V::RunRngType::UNKNOWN_MAP_POINT).next_double() as f32;
        let mut cumulative = 0.0;
        let rolled = (0..rooms.len()).find(|&at| {
            if barred[at] {
                return false;
            }
            cumulative += odds[at];
            roll <= cumulative
        });
        for at in 0..rooms.len() {
            if Some(at) == rolled {
                odds[at] = UNKNOWN_BASE[at];
            } else if !barred[at] {
                odds[at] += UNKNOWN_BASE[at];
            }
        }
        rolled.map(|at| rooms[at])
    }

    /// Take back the streams a rule drew from.
    pub(super) fn drew(&mut self, streams: Streams<V>) {
        self.streams = streams;
    }

    /// The streams as they stand, which a rule draws from a copy of.
    pub(super) fn streams(&self) -> &Streams<V> {
        &self.streams
    }

    /// Where each stream stands, which is what a report carries.
    pub(super) fn positions(&self) -> Positions<V> {
        self.streams.positions()
    }

    /// Build the fight the run is walking into, and let its relics act.
    ///
    /// The relics act on a copy, kept only once the fight is built. A fight
    /// that cannot be built leaves the run where it stood.
    ///
    /// The floor advances first. `EnterRoom` writes the point into
    /// `MapPointHistory` before the room is set up, so `TotalFloor` counts
    /// this room already.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) fn create_fight(
        &mut self,
        encounter: V::Encounter,
        queue: EncounterQueue,
        at: Location,
    ) -> Result<CreatedFight<V>, Error> {
        let floor = self.floor + num_traits::one();
        self.build_fight(encounter, queue, at, floor, &[], &[])
    }

    /// [`State::create_fight`] on a floor the caller names.
    fn build_fight(
        &mut self,
        encounter: V::Encounter,
        queue: EncounterQueue,
        at: Location,
        floor: V::Count,
        rolled: &[V::Health],
        roster: &[V::Monster],
    ) -> Result<CreatedFight<V>, Error> {
        let standing = self.standing();
        let (raw_seed, character, ascension) =
            (self.raw_seed(), self.character(), self.ascension());
        let entering = crate::combat::Entering {
            floor,
            hp: standing.hp,
            max_hp: standing.max_hp,
            gold: self.player.gold,
            potions: u8::try_from(self.player.potions.len()).expect("a belt's potions fit a byte"),
            alive: standing.alive,
            deck: &self.player.deck,
            strayed: self.strays(at),
            point: self.point_type(at),
            coord: match at {
                Location::MapPoint(coord) => Some(coord),
                _ => None,
            },
            rolled,
            roster,
            // A fight an event opens stands inside the event's room.
            base: !matches!(self.phase, Phase::Event { .. }),
        };
        let crate::combat::Entered {
            live,
            streams,
            ended,
            arriving,
        } = crate::combat::enter::<V>(
            raw_seed,
            character,
            ascension,
            encounter,
            &entering,
            &self.streams,
            &mut self.player.relics,
        )?;
        Ok(CreatedFight {
            live,
            streams,
            queue,
            ended,
            arriving,
        })
    }

    /// `EventModel.EnterCombatWithoutExitingEvent`: a fight inside the event's
    /// room.
    ///
    /// `EnterRoomWithoutExitingCurrentRoom` adds a room to the point the run
    /// stands on rather than a point, so the floor stays where it is. The
    /// room stack is two deep, so `MarkRoomVisited` counts nothing. The fight
    /// is won to the rewards a monster room gives. The run then travels on
    /// from the event's point, or with `resume` goes back to the event.
    fn fight_in_event(
        &mut self,
        encounter: V::Encounter,
        reward: Option<V::Card>,
        roster: &[V::Monster],
        resume: bool,
    ) -> Result<Decided, Error> {
        let CreatedFight {
            live,
            streams,
            ended,
            arriving,
            ..
        } = {
            let rolled = match &self.phase {
                Phase::Event { layout, .. } => layout.clone(),
                _ => SmallVec::new(),
            };
            self.build_fight(
                encounter,
                EncounterQueue::Named,
                Location::NoLocation,
                self.floor,
                &rolled,
                roster,
            )?
        };
        let mut live = live;
        // `AddExtraReward`, as the room is built and before the fight opens.
        if let Some(card) = reward {
            live.add_extra_card(CardIdent::new(card));
        }
        self.drew(streams);
        let event = std::mem::replace(&mut self.phase, Phase::Combat(live));
        if resume {
            self.resuming = Some(Box::new(Resuming {
                event,
                escaped: false,
            }));
        }
        self.arrived(arriving);
        Ok(self.settle(ended, 0))
    }

    /// Carry out what a fight's opening asked of the run, which asks nothing
    /// back.
    fn arrived(&mut self, arriving: Vec<sts2_core::HookEffect<V>>) {
        if let Some(asked) = self.carry_out(arriving) {
            let pickup = asked.pickup;
            unimplemented!("a fight's opening asking for {pickup:?}");
        }
    }

    /// Put the run into a named fight, rather than travel to one.
    ///
    /// `FightConsoleCmd`, which is `EnterRoomDebug` at
    /// `MapPointType.Unassigned`. The floor advances and the map does not, so
    /// map and room disagree by construction. That is what the command is for:
    /// reaching a chosen fight without hunting for a seed that puts one on the
    /// way. The encounter is named, so the act's own list is not drawn from.
    pub(crate) fn fight_here(&mut self, encounter: V::Encounter) -> Result<Decided, Error> {
        let fight = self.create_fight(encounter, EncounterQueue::Named, Location::NoLocation)?;
        Ok(self.enter_fight(fight, Location::NoLocation))
    }

    /// Enter a named event, off the map.
    ///
    /// `EventConsoleCmd`: a map point appended to the history, then
    /// `RunManager.EnterRoom` with the event's room. That seeds the event's
    /// own stream, and counts it against the act the way travelling does.
    pub(crate) fn event_here(&mut self, event: V::Event) -> Result<Decided, Error> {
        let mut rng = crate::event::stream::<V, crate::rng::MegaRng>(self.raw_seed(), 0, event);
        let mut streams = *self.streams();
        let (state, layout) = self.open_event(event, &mut rng, &mut streams)?;
        self.streams = streams;
        Arc::make_mut(&mut self.visited_events).push(event);
        self.enter(
            Location::NoLocation,
            Phase::Event {
                state,
                rng,
                asking: None,
                finished: false,
                layout,
            },
        );
        Ok(Decided::Resolved)
    }

    /// Put a power on a creature in the fight.
    ///
    /// `ApplyPowerConsoleCmd`, which applies it through the command a card
    /// applies one with. So a debuff put on the player here skips its next
    /// tick, exactly as one a monster applied does.
    pub(crate) fn apply_power(
        &mut self,
        at: sts2_core::CombatId,
        power: V::Power,
        amount: sts2_core::PowerAmount<V::Number>,
    ) -> Result<Decided, Error> {
        let Some(fighting) = self.fighting() else {
            unimplemented!("a power applied outside a fight");
        };
        crate::combat::apply_power(fighting, at, power, amount);
        Ok(Decided::Resolved)
    }

    /// Enchant the fight's copy of the card at `index` in hand, which is
    /// `EnchantConsoleCmd`.
    pub(crate) fn enchant(
        &mut self,
        enchantment: V::Enchantment,
        amount: u8,
        index: sts2_core::HandIndex,
    ) -> Result<super::Settled<V>, Error> {
        let Some(fighting) = self.fighting() else {
            unimplemented!("an enchantment put on a card outside a fight");
        };
        Ok(crate::combat::enchant(fighting, enchantment, amount, index)
            .map(|()| Decided::Resolved)
            .map_err(|reason| sts2_core::Refusal {
                what: "enchant".into(),
                reason,
            }))
    }

    /// Put a card, a relic or a potion straight into the run.
    ///
    /// `Driver.GrantCard`, `RelicCmd.Obtain` and `PotionCmd.TryToProcure`,
    /// which are the paths ordinary play uses. A relic arrives through the
    /// command that runs its `AfterObtained`. The simulator runs no relic's
    /// hooks, so it cannot answer for one.
    pub(crate) fn grant(&mut self, gift: &Gift<V>) -> Result<Decided, Error> {
        match gift {
            // `GrantCard` in a fight in progress: the fight's own card, into
            // hand, and the deck as it was.
            Gift::Card(card) => match self.fighting() {
                Some(fighting) => crate::combat::grant_card(fighting, *card),
                None => self.add_to_deck(CardIdent::new(*card)),
            },
            Gift::Potion(potion) => {
                // `TryToProcure` refuses a full belt rather than growing one,
                // and the belt in the next observation says whether it landed.
                self.procure(*potion);
            }
            // `RelicCmd.Obtain`, whose pickup asks where the room can park on
            // it, as Neow's can.
            Gift::Relic(relic) => {
                if self.phase.asking_mut().is_some() {
                    return Ok(match self.obtain(*relic) {
                        None => Decided::Resolved,
                        Some(asked) => self.park(asked),
                    });
                }
                self.obtain_here(*relic)
            }
        }
        Ok(Decided::Resolved)
    }

    pub(super) fn normal_encounter(&self, act: usize) -> Option<V::Encounter> {
        self.shared
            .opening
            .acts
            .get(act)
            .and_then(|(_, rooms)| rooms.normal.get(self.normal_fights))
            .copied()
    }

    pub(super) fn elite_encounter(&self, act: usize) -> Option<V::Encounter> {
        self.shared
            .opening
            .acts
            .get(act)
            .and_then(|(_, rooms)| rooms.elite.get(self.elite_fights))
            .copied()
    }

    /// The one boss an act rolled, which is the fight out of it.
    pub(super) fn boss_encounter(&self, act: usize) -> Option<V::Encounter> {
        self.shared
            .opening
            .acts
            .get(act)
            .map(|(_, rooms)| rooms.boss)
    }

    pub(crate) fn choose(&mut self, index: u8) -> Result<Decided, Error> {
        if let Phase::Treasure { relic, finished } = &mut self.phase {
            if *finished {
                unimplemented!("choosing from a finished treasure room");
            }
            let Some(relic) = *relic else {
                unreachable!("an empty chest finishes its room as it opens");
            };
            *finished = true;
            self.open_chest();
            match index {
                // Taking the relic ends the session and clears `CurrentRelics`.
                // A skip leaves them showing until the room is left.
                0 => {
                    if let Phase::Treasure { relic, .. } = &mut self.phase {
                        *relic = None;
                    }
                    self.obtain_here(relic);
                }
                1 => {}
                _ => unimplemented!("an option the treasure room does not offer"),
            }
            return Ok(Decided::Resolved);
        }
        if let Phase::RestSite { options, .. } = &self.phase {
            if self.phase.finished() {
                unimplemented!("choosing at a rest site that is spent");
            }
            let option = *options
                .get(usize::from(index))
                .unwrap_or_else(|| unimplemented!("an option the rest site does not offer"));
            if let Phase::RestSite { chosen, .. } = &mut self.phase {
                *chosen = Some(option);
            }
            // The option's `OnSelect` awaits the heal's rewards, so the
            // options stay up until the screen is done.
            if option == <V::RestSiteOption as sts2_core::RestKinds<V>>::HEAL {
                return Ok(match self.rest_site_heal(false) {
                    None => {
                        self.rest_option_through();
                        Decided::Resolved
                    }
                    Some(asked) => self.park(asked),
                });
            }
            // `RestSiteOption.OnSelect` for an option the run carries out,
            // which uses the rest site up.
            if option == <V::RestSiteOption as sts2_core::RestKinds<V>>::HATCH
                || option == <V::RestSiteOption as sts2_core::RestKinds<V>>::COOK
                || option == <V::RestSiteOption as sts2_core::RestKinds<V>>::DIG
                || option == <V::RestSiteOption as sts2_core::RestKinds<V>>::LIFT
                || option == <V::RestSiteOption as sts2_core::RestKinds<V>>::KINDLE
                || option == <V::RestSiteOption as sts2_core::RestKinds<V>>::CLONE
            {
                if !self.rest_option_enabled(option) {
                    unimplemented!("a rest site option the run cannot take");
                }
                // The options stay up while the option's pickup asks.
                let effects = sts2_core::RestKinds::selected(option);
                return Ok(match self.carry_out(effects) {
                    None => {
                        self.rest_option_through();
                        Decided::Resolved
                    }
                    Some(asked) => self.park(asked),
                });
            }
            if option != <V::RestSiteOption as sts2_core::RestKinds<V>>::SMITH {
                unimplemented!("the {option:?} rest site option");
            }
            if !self.rest_option_enabled(option) {
                unimplemented!("a rest site option the run cannot take");
            }
            let Phase::RestSite { upgrading, .. } = &mut self.phase else {
                unreachable!("the phase was a rest site when this began");
            };
            *upgrading = true;
            // The option opened a card selection rather than finishing. The
            // game reports the decision as unresolved until the selection is
            // confirmed, because `CardSelectorPrefs` makes it cancelable.
            return Ok(Decided::Abandoned);
        }
        if let Some(room) = self.event_room() {
            let option = match room {
                Room::Event { options, .. } => options.get(index as usize).cloned(),
                _ => None,
            }
            .unwrap_or_else(|| unimplemented!("an option the event does not offer"));
            if option.locked {
                unimplemented!("a locked event option");
            }
            // The choice acts on the whole run, so the room is taken out of
            // the run for as long as it is acting and put back after. A relic
            // an event hands over is taken the way any other is.
            let Phase::Event {
                mut state,
                mut rng,
                layout,
                ..
            } = std::mem::replace(&mut self.phase, Phase::Map)
            else {
                unreachable!()
            };
            let effects = V::choose_event(&mut state, index, self, &mut rng);
            self.phase = Phase::Event {
                state,
                rng,
                asking: None,
                finished: false,
                layout,
            };
            if let [
                sts2_core::HookEffect::FightInEvent {
                    encounter,
                    reward,
                    roster,
                    resume,
                },
            ] = effects.as_slice()
            {
                return self.fight_in_event(*encounter, *reward, roster, *resume);
            }
            return Ok(match self.carry_out(effects) {
                None => Decided::Resolved,
                Some(asked) => self.park(asked),
            });
        }
        let Phase::Neow {
            options, chosen, ..
        } = &mut self.phase
        else {
            unimplemented!("choosing an option inside a fight");
        };
        let option = *options
            .get(index as usize)
            .unwrap_or_else(|| unimplemented!("an option the blessing does not offer"));
        *chosen = nonmax::NonMaxU8::new(index);
        if option != V::lead_paperweight() {
            // `RelicCmd.Obtain`, and whatever the relic's `AfterObtained`
            // asks, which Neow's screen parks on.
            return Ok(match self.obtain(option) {
                None => Decided::Resolved,
                Some(asked) => self.park(asked),
            });
        }

        // Not through `obtain`. The blessing is the one path that carries out
        // the paperweight's own `AfterObtained`, which is the colorless offer
        // below, and the relic's row carries no `UponPickup` besides.
        self.player
            .relics
            .push(HeldRelic::new(V::lead_paperweight()));
        self.card_reward_unheard();
        let offered = crate::reward::card::regular::<V, _>(
            self.streams.player(V::PlayerRngType::REWARDS),
            V::colorless_cards(),
            2,
            true,
        );
        let mut offered: Vec<_> = offered.into_iter().map(CardIdent::new).collect();
        self.reward_options(&mut offered);
        Ok(self.park(Asked {
            pickup: Pickup::Cards {
                offered,
                min: 0,
                max: 1,
            },
            rest: SmallVec::new(),
            under: None,
        }))
    }

    /// Take from a reward screen a room is parked on. The screen stays up
    /// until every reward is taken or the rest are skipped, and what asked
    /// for it then goes on.
    fn take_parked(&mut self, answer: &Answer) -> Decided {
        let Some(asking) = self.phase.asking_mut() else {
            unreachable!("the room was parked when this began");
        };
        let Some(Asked {
            pickup:
                Pickup::Rewards {
                    mut offered,
                    skippable,
                },
            rest,
            under,
        }) = asking.take()
        else {
            unreachable!("the room was parked on a reward screen");
        };
        let screen = |offered| Asked {
            pickup: Pickup::Rewards { offered, skippable },
            rest,
            under,
        };
        // A relic whose pickup asks stands the screen still under its question.
        if let Some(mut asked) = self.take_reward(&mut offered, answer) {
            asked.under = Some(Box::new(screen(offered)));
            return self.park(asked);
        }
        let screen = screen(offered);
        self.taken_up(screen)
    }

    /// A question whose answer is in, taken up again where it left off: a
    /// reward screen with rewards left is asked again. Anything else, and a
    /// screen with none left, carries out its rest and then takes up what it
    /// interrupted.
    fn taken_up(&mut self, asked: Asked<V>) -> Decided {
        let Asked {
            pickup,
            rest,
            under,
        } = asked;
        if let Pickup::Rewards { offered, skippable } = pickup
            && offered.iter().any(|offer| !offer.taken())
        {
            return self.park(Asked {
                pickup: Pickup::Rewards { offered, skippable },
                rest,
                under,
            });
        }
        self.done_asking(rest, under)
    }

    /// The rest of a question once it is answered, then what it interrupted.
    fn done_asking(
        &mut self,
        rest: impl IntoIterator<Item = sts2_core::HookEffect<V>>,
        under: Option<Box<Asked<V>>>,
    ) -> Decided {
        if let Some(mut next) = self.carry_out(rest) {
            next.under = under;
            return self.park(next);
        }
        if let Some(under) = under {
            return self.taken_up(*under);
        }
        // `ClearAfterPurchase`, once the relic bought has finished its pickup.
        if let Phase::Shop { unfinished, .. } = &mut self.phase
            && let Some(Unfinished::Relic {
                shelf,
                parasol,
                spent,
            }) = *unfinished
        {
            *unfinished = None;
            self.purchased(shelf, Self::gold(spent));
            if parasol {
                self.purchase_everything(Some(shelf));
            }
        }
        // A rest site's option is done once its pickup is.
        if let Phase::RestSite { .. } = &self.phase {
            self.rest_option_through();
        }
        Decided::Resolved
    }

    /// Park the room on what a pickup or an event option asks.
    ///
    /// # Panics
    ///
    /// A clone, which only a merchant's shelf asks for.
    fn park(&mut self, asked: Asked<V>) -> Decided {
        if let Pickup::CloneDeckCard { .. } = asked.pickup {
            unimplemented!("{:?} asked for outside a shop", asked.pickup);
        }
        let Some(asking) = self.phase.asking_mut() else {
            unreachable!("only Neow and an event park on what they ask")
        };
        *asking = Some(asked);
        Decided::Abandoned
    }

    pub(crate) fn answer(&mut self, answer: &Answer) -> Result<Decided, Error> {
        if let Phase::Rewards { .. } = self.phase {
            return self.take(answer);
        }
        if matches!(&self.phase, Phase::Combat(live) if live.question().is_some()) {
            return self.answer_fight(answer);
        }
        if matches!(&self.phase, Phase::RestSite { upgrading, .. } if *upgrading) {
            return self.smith(answer);
        }
        if matches!(
            &self.phase,
            Phase::Shop {
                unfinished: Some(Unfinished::Removal { .. } | Unfinished::Pickup { .. }),
                ..
            }
        ) {
            return self.finish_purchase(answer);
        }
        if let Some(Asked {
            pickup: Pickup::Rewards { .. },
            ..
        }) = self.phase.asking()
        {
            return Ok(self.take_parked(answer));
        }
        if let Some(Asked {
            pickup: Pickup::Event,
            ..
        }) = self.phase.asking()
        {
            return Ok(self.answer_event(answer));
        }
        if let Some(Asked {
            pickup: Pickup::ConfirmAbandon,
            ..
        }) = self.phase.asking()
        {
            return Ok(self.confirmed_abandon(answer));
        }
        let Some(asking) = self.phase.asking_mut() else {
            unimplemented!("answering a question inside a fight");
        };
        let Answer::Cards(picks) = answer else {
            unimplemented!("an answer that is not a card selection");
        };
        let asked = asking
            .take()
            .unwrap_or_else(|| unimplemented!("an answer to a question nobody asked"));
        match asked.pickup {
            // `CardPileCmd.Add` of each card of the bundle chosen.
            Pickup::Bundles { bundles } => {
                let chosen = match picks.iter().collect::<Vec<_>>()[..] {
                    [one] => bundles.get(one),
                    _ => None,
                };
                let Some(chosen) = chosen else {
                    unimplemented!("a bundle choice naming other than one bundle offered");
                };
                for &card in chosen {
                    self.add_to_deck(CardIdent::new(card));
                }
            }
            Pickup::Cards { offered, min, max } => {
                if picks.len() < min || picks.len() > max {
                    unimplemented!("a card selection of other than {min} to {max} cards");
                }
                let chosen: Option<Vec<_>> = picks
                    .iter()
                    .map(|index| offered.get(index).copied())
                    .collect();
                let Some(chosen) = chosen else {
                    unimplemented!("a card selection naming a card that was not offered");
                };
                for card in chosen {
                    self.add_to_deck(card);
                }
            }
            Pickup::FromDeck { act, min, count } => {
                let offered = self.offered(act);
                let taken: Option<Vec<usize>> = picks
                    .iter()
                    .map(|index| offered.get(index).map(|(at, _)| *at))
                    .collect();
                let Some(taken) = taken else {
                    unimplemented!("a deck selection naming a card that was not offered");
                };
                let mut distinct = taken.clone();
                distinct.sort_unstable();
                distinct.dedup();
                if taken.len() < min || taken.len() > count || distinct.len() != taken.len() {
                    unimplemented!(
                        "a deck selection of other than {min} to {count} different cards"
                    );
                }
                self.act_on_deck(act, &taken);
            }
            Pickup::CloneDeckCard { .. } => unreachable!("a room parks on no clone"),
            Pickup::Rewards { .. } => unreachable!("a reward screen is answered above"),
            Pickup::Event => unreachable!("an event's own question is answered above"),
            Pickup::ConfirmAbandon => unreachable!("the abandon-run popup is answered above"),
        }
        // The rest of the pickup, which can ask again.
        Ok(self.done_asking(asked.rest, asked.under))
    }

    /// The question the event the run stands in parks on.
    fn event_question(&self) -> Question<V> {
        let Phase::Event { state, .. } = &self.phase else {
            unreachable!("only an event asks its own question")
        };
        V::event_question(state).unwrap_or_else(|| unreachable!("an event parked on no question"))
    }

    /// Answer the event's own question, then carry out what the answer asked
    /// and the rest of what parked it.
    fn answer_event(&mut self, answer: &Answer) -> Decided {
        let Phase::Event { state, asking, .. } = &mut self.phase else {
            unreachable!("only an event asks its own question")
        };
        let asked = asking
            .take()
            .unwrap_or_else(|| unreachable!("the event's question is parked"));
        let mut effects = V::answer_event(state, answer);
        effects.extend(asked.rest);
        match self.carry_out(effects) {
            Some(next) => self.park(next),
            None => Decided::Resolved,
        }
    }

    /// Upgrade the card the smith was pointed at.
    ///
    /// `CardCmd.Upgrade` raises the level by one. The selection offers the
    /// deck's upgradable cards alone, so the index is that list's and the
    /// deck's own place comes back with it.
    ///
    /// The rest site is spent once this lands: its options empty, which is how
    /// the game reports a rest site nobody can act at again.
    fn smith(&mut self, answer: &Answer) -> Result<Decided, Error> {
        let Answer::Cards(picks) = answer else {
            unimplemented!("an answer to the smith that is not a card selection");
        };
        let Some(index) = picks.only() else {
            unimplemented!("a smith selection that is not one card");
        };
        let (at, _) = self
            .upgradable()
            .nth(index)
            .unwrap_or_else(|| unimplemented!("upgrading a card the selection did not offer"));
        self.player.deck_mut()[at].ident.upgrade += 1;

        let Phase::RestSite { upgrading, .. } = &mut self.phase else {
            unreachable!("the phase was a rest site when this began");
        };
        *upgrading = false;
        self.rest_option_through();
        Ok(Decided::Resolved)
    }

    /// Take rewards from the screen a won fight put up.
    ///
    /// `RewardsSetSynchronizer.SelectLocalReward`, one entry at a time. A
    /// reward stays in the set once it is taken, because the indices an answer
    /// names never renumber.
    fn take(&mut self, answer: &Answer) -> Result<Decided, Error> {
        let Phase::Rewards {
            offered, removing, ..
        } = &mut self.phase
        else {
            unreachable!("the phase was a reward screen when this began");
        };
        // The removal a card removal reward waits on.
        if let Some(at) = *removing {
            let Answer::Cards(picks) = answer else {
                unimplemented!("a card removal answered with other than a card");
            };
            let Some(pick) = picks.only() else {
                unimplemented!("a card removal answered with other than one card");
            };
            *removing = None;
            let Some(&(in_deck, _)) = self.offered(sts2_core::DeckAct::Remove).get(pick) else {
                unimplemented!("a removal naming a card the selection did not offer");
            };
            self.act_on_deck(sts2_core::DeckAct::Remove, &[in_deck]);
            let Phase::Rewards { offered, .. } = &mut self.phase else {
                unreachable!("removing a deck card leaves the reward screen up");
            };
            offered[at] = Offer::CardRemoval { taken: true };
            return Ok(self.decided());
        }
        // `DoUnsyncedCardRemoval`, which is taken once a card comes back.
        if let Answer::Take(take) = *answer
            && let Some(Offer::CardRemoval { taken: false }) = offered.get(take.index())
        {
            *removing = Some(take.index());
            return Ok(self.decided());
        }
        let mut offered = std::mem::take(offered);
        if let Some(asked) = self.take_reward(&mut offered, answer) {
            let pickup = asked.pickup;
            unimplemented!("a relic asking for {pickup:?} on a fight's reward screen");
        }
        let Phase::Rewards { offered: back, .. } = &mut self.phase else {
            unreachable!("taking a fight's reward leaves its screen up");
        };
        *back = offered;
        Ok(self.decided())
    }

    /// Take one reward off a screen, or skip what is left of it.
    ///
    /// # Panics
    ///
    /// An answer the screen does not accept, or a potion for a full belt.
    #[must_use]
    fn take_reward(&mut self, offered: &mut [Offer<V>], answer: &Answer) -> Option<Asked<V>> {
        if let Answer::Skip = answer {
            for offer in offered {
                match offer {
                    Offer::Gold { taken, .. }
                    | Offer::Card { taken, .. }
                    | Offer::SpecialCard { taken, .. }
                    | Offer::Potion { taken, .. }
                    | Offer::Relic { taken, .. }
                    | Offer::CardRemoval { taken } => *taken = true,
                }
            }
            return None;
        }
        let Answer::Take(take) = *answer else {
            unimplemented!("an answer the reward screen does not accept");
        };
        let slots: usize = num_traits::cast(self.player.potion_slots)
            .expect("the belt has an addressable number of slots");
        let offer = offered
            .get_mut(take.index())
            .unwrap_or_else(|| unimplemented!("a reward the screen does not offer"));
        match (offer, take) {
            (Offer::Gold { amount, taken }, TakeReward::NonCard { .. }) => {
                let amount = *amount;
                *taken = true;
                self.gain_gold(amount);
            }
            (Offer::Potion { potion, taken }, TakeReward::NonCard { .. }) => {
                let potion = *potion;
                // `PotionCmd.TryToProcure` fails on a full belt, and the
                // reward is then neither taken nor gone.
                if self.player.potions.len() >= slots {
                    return None;
                }
                *taken = true;
                self.procure(potion);
            }
            // The chosen card leaves the reward, which goes on listing the
            // cards not chosen.
            (Offer::Card { cards, taken, .. }, TakeReward::Card { card, .. }) => {
                let at = usize::from(card);
                if at >= cards.len() {
                    unimplemented!("a card the reward does not offer");
                }
                let chosen = cards.remove(at);
                *taken = true;
                self.add_to_deck(chosen);
            }
            (Offer::SpecialCard { card, state, taken }, TakeReward::NonCard { .. }) => {
                let (ident, state) = (*card, *state);
                *taken = true;
                let id = self.player.mint();
                self.add_deck_card(sts2_core::DeckCard { id, ident, state });
            }
            (
                Offer::Relic {
                    relic, wax, taken, ..
                },
                TakeReward::NonCard { .. },
            ) => {
                let (relic, wax) = (*relic, *wax);
                *taken = true;
                return self.obtain_as(relic, wax);
            }
            // `CardReward.Reroll`: `CanReroll` goes, and the pack is rolled
            // again as it was. The reward stays on offer.
            (
                Offer::Card {
                    cards,
                    rolled,
                    reroll,
                    ..
                },
                TakeReward::Alternative {
                    alternative: sts2_core::RewardAlternative::Reroll,
                    ..
                },
            ) => {
                if !*reroll {
                    unimplemented!("a reroll of a card reward that cannot reroll");
                }
                let how =
                    rolled.unwrap_or_else(|| unimplemented!("a reroll of a pack set by hand"));
                *reroll = false;
                let pool = self.reward_pool();
                *cards = match how {
                    crate::reward::Rolled::Room(room) => {
                        let (ascension, act) = (self.ascension(), self.act());
                        let added = self.card_reward_added();
                        let (rng, odds) = self.rewarding();
                        let Offer::Card { cards, .. } = crate::reward::combat::card_reward::<V, _>(
                            rng,
                            odds,
                            ascension,
                            pool.drafted(),
                            act,
                            room,
                            &added,
                        ) else {
                            unreachable!("a card reward rolls a card offer");
                        };
                        cards
                    }
                    crate::reward::Rolled::Custom { reward, stream } => {
                        self.card_reward_unheard();
                        self.custom_cards(reward, stream, Some(&pool))
                    }
                };
                self.reward_options(cards);
            }
            // The alternative's `OnSelect`, which completes the reward.
            (
                Offer::Card {
                    sacrifice, taken, ..
                },
                TakeReward::Alternative {
                    alternative: alternative @ sts2_core::RewardAlternative::Sacrifice,
                    ..
                },
            ) => {
                if !*sacrifice {
                    unimplemented!("a sacrifice of a card reward that offers none");
                }
                *taken = true;
                let chosen = sts2_core::EventHook::CardRewardAlternativeChosen(alternative);
                let effects = self.notify_belt(chosen);
                return self.carry_out(effects);
            }
            _ => unimplemented!("a reward taken as the wrong kind"),
        }
        None
    }

    /// Where the player stands.
    ///
    /// A live fight holds these numbers, because a card can move them.
    /// Anywhere else the run holds them. See `Player::hp`.
    fn standing(&self) -> Standing<V> {
        match &self.phase {
            Phase::Combat(live) => {
                let player = live.player();
                Standing {
                    hp: player.hp,
                    max_hp: player.max_hp,
                    alive: player.alive,
                }
            }
            // The player fell in the fight, and the fight is where its own
            // numbers stayed.
            Phase::Lost { combat } => {
                let player = combat
                    .creatures
                    .iter()
                    .find(|c| c.id == self.player_id(combat))
                    .expect("a lost fight has the player in it");
                Standing {
                    hp: player.hp,
                    max_hp: player.max_hp,
                    alive: player.alive,
                }
            }
            // `GuaranteeKillAllPlayers`, which `WinRun` ends on.
            Phase::Won => Standing {
                hp: num_traits::zero(),
                max_hp: self.player.max_hp,
                alive: false,
            },
            Phase::Map
            | Phase::Neow { .. }
            | Phase::Rewards { .. }
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => Standing {
                hp: self.player.hp,
                max_hp: self.player.max_hp,
                // Only a death outside a fight leaves the run with none.
                alive: self.player.hp > num_traits::zero(),
            },
        }
    }

    /// The belt, and why each potion in it cannot be drunk.
    ///
    /// Beside the report rather than inside it, because deciding whether a
    /// potion can be drunk reads this list and nothing else around it.
    pub(crate) fn belt(&self) -> Vec<sts2_core::Held<V>> {
        // A fight that is over is not one a potion can be drunk in, so a live
        // fight is the only one read.
        crate::combat::belt::<V>(
            &self.player.potions,
            self.fight(),
            self.standing().alive,
            self.potions_held(),
        )
        .collect()
    }

    /// Every decision the run accepts right now, in the order a driver picks
    /// from.
    ///
    /// Read off the run, never off its report. Each phase reads what its
    /// report reads and runs the rules the report runs, in the report's order:
    /// a pending question alone, or the belt, then the room, then the map.
    pub(crate) fn legal(&self) -> Legal<V> {
        let alive = self.standing().alive;
        if let Phase::Combat(live) = &self.phase {
            return crate::combat::legal_moves::<V>(
                live,
                &self.player.relics,
                &self.player.potions,
                alive,
            );
        }

        // `IsGameOver`: a run whose player is dead takes nothing more.
        let mut moves = Legal::new();
        if !alive {
            return moves;
        }
        if self.answers(&mut moves) {
            return moves;
        }
        // No fight is live here, so a drink names no creature.
        crate::combat::drinks::<V>(
            &self.player.potions,
            None,
            alive,
            self.potions_held(),
            &mut moves,
        );
        let before = moves.len();
        self.room_moves(&mut moves);
        // `NRestSiteRoom` puts its proceed button up once an option went
        // through, beside any options still left.
        let rested = matches!(self.phase, Phase::RestSite { rested: true, .. });
        if moves.len() > before && !rested {
            return moves;
        }

        if self.can_proceed() {
            moves.push(Decision::Proceed);
        } else {
            moves.extend(
                self.reachable()
                    .into_iter()
                    .map(|point| Decision::Travel { to: point.at }),
            );
        }
        moves
    }

    /// The answers to the question the run is parked on, if it is parked.
    ///
    /// Each is the question [`State::report`] asks in that phase, read for its
    /// sizes and its rewards alone.
    fn answers(&self, moves: &mut Legal<V>) -> bool {
        match &self.phase {
            Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. }
                if self.phase.asking().is_some() =>
            {
                let Some(asked) = self.phase.asking() else {
                    unreachable!("the guard found it parked")
                };
                match &asked.pickup {
                    Pickup::Bundles { bundles } => moves.extend(
                        (0..bundles.len())
                            .map(|at| Decision::Answer(Answer::Cards([at].into_iter().collect()))),
                    ),
                    Pickup::ConfirmAbandon => moves.extend([
                        Decision::Answer(Answer::Confirm(true)),
                        Decision::Answer(Answer::Confirm(false)),
                    ]),
                    Pickup::Cards { offered, min, max } => {
                        selections(*min, *max, offered.len(), moves)
                    }
                    Pickup::FromDeck { act, min, count } => {
                        selections(*min, *count, self.offered(*act).len(), moves)
                    }
                    Pickup::CloneDeckCard { .. } => unreachable!("a room parks on no clone"),
                    Pickup::Rewards { offered, skippable } => {
                        reward_moves(offered, *skippable, moves)
                    }
                    Pickup::Event => event_moves(self.event_question(), moves),
                }
            }
            Phase::RestSite {
                upgrading: true, ..
            } => selections(1, 1, self.upgradable().count(), moves),
            Phase::Shop {
                unfinished: Some(unfinished),
                ..
            } => {
                let offered = match unfinished {
                    Unfinished::Removal { .. } => self.offered(sts2_core::DeckAct::Remove).len(),
                    Unfinished::Pickup { excluding, .. } => self.clonable(*excluding).len(),
                    Unfinished::Relic { .. } => unreachable!("the parked pickup answers first"),
                };
                selections(1, 1, offered, moves);
            }
            Phase::Rewards {
                removing: Some(_), ..
            } => selections(1, 1, self.offered(sts2_core::DeckAct::Remove).len(), moves),
            Phase::Rewards { offered, .. } if offered.iter().any(|offer| !offer.taken()) => {
                reward_moves(offered, true, moves)
            }
            _ => return false,
        }
        true
    }

    /// The options a room lets the run take, and the stock it lets it buy.
    ///
    /// The rules the report's flags carry. A rest option can be disabled, a
    /// chest answered, and an event option chosen or locked. Stock can be gone,
    /// cost more than the purse, or be a potion for a full belt.
    fn room_moves(&self, moves: &mut Legal<V>) {
        let finished = self.phase.finished();
        match &self.phase {
            Phase::Neow {
                options, chosen, ..
            } if !finished => moves.extend(
                (0..options.len())
                    .map(option_index)
                    .filter(|&index| *chosen != nonmax::NonMaxU8::new(index))
                    .map(|index| Decision::ChooseOption { index }),
            ),
            // An event's options are the content's to spell, so its room is
            // built. That is the room alone, and only in an event.
            // An event whose room is a shop sells what it reports.
            Phase::Event { .. } => match self.event_room() {
                Some(Room::Event { options, .. }) => moves.extend(
                    options
                        .iter()
                        .filter(|option| !option.locked && !option.chosen)
                        .map(|option| Decision::ChooseOption {
                            index: option.index,
                        }),
                ),
                Some(Room::Shop { entries }) => moves.extend(
                    entries
                        .iter()
                        .filter(|entry| {
                            entry.stocked && entry.affordable && entry.blocked.is_none()
                        })
                        .map(|entry| Decision::Buy { index: entry.index }),
                ),
                _ => {}
            },
            Phase::Treasure {
                finished: false, ..
            } => moves.extend([0, 1].map(|index| Decision::ChooseOption { index })),
            Phase::RestSite { options, .. } => moves.extend(
                options
                    .iter()
                    .enumerate()
                    .filter(|&(_, &id)| self.rest_option_enabled(id))
                    .map(|(index, _)| Decision::ChooseOption {
                        index: option_index(index),
                    }),
            ),
            Phase::Shop { entries, .. } => {
                let gold = self.player.gold;
                let room_for_a_potion = num_traits::cast::<_, usize>(self.player.potion_slots)
                    .expect("a potion slot count fits this machine")
                    > self.player.potions.len();
                moves.extend(
                    entries
                        .iter()
                        .filter(|entry| {
                            entry.stocked
                                && self.merchant_price(entry.cost) <= gold
                                && (room_for_a_potion
                                    || !matches!(entry.stock, sts2_core::Stock::Potion(_)))
                        })
                        .map(|entry| Decision::Buy { index: entry.index }),
                );
            }
            _ => {}
        }
    }

    /// Where the run can move to from where it stands.
    ///
    /// `MapLegality.TravelableFrom`, which reads the last point visited rather
    /// than a room the run is in. An act with none visited has just begun, and
    /// its map opens on its own starting node.
    ///
    /// Beside the report rather than inside it, because deciding whether a
    /// move is legal reads this list and nothing else around it.
    pub(crate) fn travelable(&self) -> Vec<Reachable<V>> {
        // The map stays shut until the event under the fight is resumed.
        if self.resuming.is_some() {
            return Vec::new();
        }
        self.reachable().into_vec()
    }

    /// [`State::travelable`], lent rather than collected.
    /// `MapTravel.GetTravelablePointsFrom`: what the point the run stands on
    /// leads on to, or the whole next row under free travel. An act the run
    /// has entered no point of opens on its ancient alone.
    ///
    /// The boss sits past the grid, so no row holds it, and the last row
    /// leads on to it alone either way.
    fn reachable(&self) -> SmallVec<[Reachable<V>; 7]> {
        use sts2_core::MapPointKinds as _;

        let map = &self.map;
        let Some(&at) = self.visited.last() else {
            let ancient = map.ancient().expect("a generated act map opens somewhere");
            return smallvec::smallvec![Reachable {
                at: ancient.at,
                kind: ancient.kind,
            }];
        };
        let here = map
            .at(at)
            .expect("the run stands on a point of its own map");
        let row: SmallVec<[Coord; 7]> = match self.free_travel() {
            true => map
                .nodes()
                .iter()
                .filter(|node| node.at.row == here.at.row + 1 && node.kind != V::MapPointType::BOSS)
                .map(|node| node.at)
                .collect(),
            false => SmallVec::new(),
        };
        let to = if row.is_empty() { &here.to } else { &row };
        to.iter()
            .map(|&at| Reachable {
                at,
                kind: map.at(at).expect("a map edge leads to a node").kind,
            })
            .collect()
    }

    /// `Hook.ShouldProcurePotion`, which any relic on the belt can answer no
    /// to. A fight's other listeners never answer it.
    fn procure_refused(&self, potion: V::Potion) -> bool {
        let hook = sts2_core::DecisionHook::ShouldProcurePotion { potion };
        let hooks = sts2_core::Overrides::of(sts2_core::ModelHook::ShouldProcurePotion);
        let mut left = self.player.relics.hearing(hooks);
        while left != 0 {
            let held = &self.player.relics[left.trailing_zeros() as usize];
            left &= left - 1;
            if V::decide_hook(crate::combat::Heard::<V>::Relic(&held.state), hook)
                .is_decision_and(|allowed| !allowed)
            {
                return true;
            }
        }
        false
    }

    /// `MerchantEntry.Cost` in a merchant's room: `Hook.ModifyMerchantPrice`
    /// over what the shelf holds, which `(int)` truncates.
    fn merchant_price(&self, cost: V::Number) -> V::Number {
        let hook = sts2_core::ModifyHook::MerchantPrice;
        let mut left = self
            .player
            .relics
            .hearing(sts2_core::Overrides::of(hook.model_hook()));
        if left == 0 {
            return cost;
        }
        let mut price = crate::combat::exact::<V>(cost);
        while left != 0 {
            let held = &self.player.relics[left.trailing_zeros() as usize];
            left &= left - 1;
            match V::modify_hook(crate::combat::Heard::<V>::Relic(&held.state), hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Scaled(share) => price *= share.exact(),
                answer => unimplemented!("a relic answering {answer:?} for a merchant's price"),
            }
        }
        crate::combat::whole::<V>(price)
    }

    /// `RestSiteSynchronizer.ChooseOption` once the chosen option went
    /// through. `Hook.ShouldDisableRemainingRestSiteOptions` clears every
    /// option unless a relic answers no, and then only the chosen one goes.
    fn rest_option_through(&mut self) {
        let hook = sts2_core::DecisionHook::ShouldDisableRemainingRestSiteOptions;
        let mut left = self
            .player
            .relics
            .hearing_hook(sts2_core::ModelHook::ShouldDisableRemainingRestSiteOptions);
        let mut disable = true;
        while left != 0 {
            let held = &self.player.relics[left.trailing_zeros() as usize];
            left &= left - 1;
            disable &= !V::decide_hook(crate::combat::Heard::<V>::Relic(&held.state), hook)
                .is_decision_and(|disable| !disable);
        }
        let Phase::RestSite {
            options,
            chosen,
            rested,
            ..
        } = &mut self.phase
        else {
            unreachable!("an option went through at a rest site");
        };
        let chosen = chosen
            .take()
            .unwrap_or_else(|| unreachable!("an option went through that was chosen"));
        *rested = true;
        match disable {
            true => options.clear(),
            false => options.retain(|&option| option != chosen),
        }
    }

    /// `Hook.ShouldRefillMerchantEntry`, which any relic on the belt can
    /// answer yes to.
    fn refills(&self) -> bool {
        let hook = sts2_core::DecisionHook::ShouldRefillMerchantEntry;
        let hooks = sts2_core::Overrides::of(sts2_core::ModelHook::ShouldRefillMerchantEntry);
        let mut left = self.player.relics.hearing(hooks);
        let mut refills = false;
        while left != 0 {
            let held = &self.player.relics[left.trailing_zeros() as usize];
            left &= left - 1;
            refills |= V::decide_hook(crate::combat::Heard::<V>::Relic(&held.state), hook)
                .is_decision_and(|refill| refill);
        }
        refills
    }

    /// What a merchant's shelf does once its purchase went through:
    /// `RestockAfterPurchase` where a listener refills it, and
    /// `ClearAfterPurchase` otherwise.
    fn purchased(&mut self, index: u8, spent: V::Number) {
        self.clear_or_restock(index);
        self.item_purchased(spent);
    }

    /// `Hook.AfterItemPurchased`, once an entry's purchase has returned and
    /// its shelf has cleared or restocked.
    ///
    /// # Panics
    ///
    /// A relic asking for something as it hears of the purchase.
    fn item_purchased(&mut self, gold: V::Number) {
        let asked = self.notify_belt(sts2_core::EventHook::ItemPurchased { gold });
        if let Some(effect) = asked.first() {
            unimplemented!("a relic asking for {effect:?} after a purchase");
        }
    }

    /// The gold a parked purchase noted, back in the run's own number.
    fn gold(spent: u32) -> V::Number {
        num_traits::cast(spent).expect("gold spent fits the run's number")
    }

    /// `RestockAfterPurchase` where a relic refills the shelf, and
    /// `ClearAfterPurchase` otherwise.
    fn clear_or_restock(&mut self, index: u8) {
        if !self.refills() {
            let Phase::Shop { entries, .. } = &mut self.phase else {
                unreachable!("a purchase is made in a shop");
            };
            sold(entries, index);
            return;
        }
        let Phase::Shop { entries, .. } = &self.phase else {
            unreachable!("a purchase is made in a shop");
        };
        let at = entries
            .iter()
            .position(|entry| entry.index == index)
            .expect("the shelf bought from is on the merchant");
        let on_shelves: Vec<V::Card> = entries
            .iter()
            .filter_map(|entry| match entry.stock {
                sts2_core::Stock::Card(Some(on_sale)) => Some(on_sale.card.card),
                _ => None,
            })
            .collect();
        let stock = entries[at].stock;
        let mut streams = self.streams;
        let mut dealt = self.dealt_relics.to_vec();
        let (stock, cost) = super::shop::Merchant::<V> {
            streams: &mut streams,
            player_relics: &self.shared.opening.player_relics,
            dealt_relics: &mut dealt,
            character: self.shared.character,
            offset: self.odds.rarity,
            removals_used: self.removals_used,
        }
        .restock(usize::from(index), stock, &on_shelves);
        self.streams = streams;
        self.dealt_relics = Arc::new(dealt);
        // `Hook.ModifyMerchantCardCreationResults` over the card it stocked.
        let stock = match stock {
            sts2_core::Stock::Card(Some(mut on_sale)) => {
                on_sale.card = self.rewritten(on_sale.card, |card| {
                    sts2_core::EventHook::MerchantCardCreated { card }
                });
                sts2_core::Stock::Card(Some(on_sale))
            }
            stock => stock,
        };
        let Phase::Shop { entries, .. } = &mut self.phase else {
            unreachable!("a purchase is made in a shop");
        };
        let entry = &mut entries[at];
        entry.stock = stock;
        entry.cost = num_traits::cast(cost).expect("a merchant's price fits the build's number");
        entry.stocked = true;
    }

    /// `Hook.ShouldForcePotionReward` for a fight won in `room`, which any
    /// relic on the belt can answer yes to.
    pub(super) fn potion_forced(&self, room: V::RoomType) -> bool {
        let hook = sts2_core::DecisionHook::ShouldForcePotionReward { room };
        let hooks = sts2_core::Overrides::of(sts2_core::ModelHook::ShouldForcePotionReward);
        let mut left = self.player.relics.hearing(hooks);
        let mut forced = false;
        while left != 0 {
            let held = &self.player.relics[left.trailing_zeros() as usize];
            left &= left - 1;
            forced |= V::decide_hook(crate::combat::Heard::<V>::Relic(&held.state), hook)
                .is_decision_and(|force| force);
        }
        forced
    }

    /// `Hook.ShouldAllowFreeTravel`, which any relic on the belt can answer
    /// yes to.
    fn free_travel(&self) -> bool {
        let hook = sts2_core::DecisionHook::ShouldAllowFreeTravel;
        let hooks = sts2_core::Overrides::of(sts2_core::ModelHook::ShouldAllowFreeTravel);
        let mut left = self.player.relics.hearing(hooks);
        let mut free = false;
        while left != 0 {
            let held = &self.player.relics[left.trailing_zeros() as usize];
            left &= left - 1;
            free |= V::decide_hook(crate::combat::Heard::<V>::Relic(&held.state), hook)
                .is_decision_and(|allowed| allowed);
        }
        free
    }

    /// Derive the comparison snapshot from the run's authoritative state.
    pub(crate) fn report(&self) -> Snapshot<V> {
        let standing = self.standing();

        Snapshot {
            observation: Observation {
                act: num_traits::cast(self.act).expect("an act index is a game number"),
                floor: self.floor,
                hp: standing.hp,
                max_hp: standing.max_hp,
                alive: standing.alive,
                // `RunState.IsGameOver` is every player being dead, and a run
                // with one player has its answer in `alive`.
                run_over: !standing.alive,
                // `CurrentRoom.IsVictoryRoom`, asked only once the run is
                // over, which The Architect's room is.
                victory: (!standing.alive).then_some(matches!(self.phase, Phase::Won)),
                gold: self.player.gold,
                relics: self.carried().collect(),
                potions: self.belt(),
                potion_slots: self.player.potion_slots,
                deck: self.player.deck.iter().map(|held| held.ident).collect(),
                in_combat: self.in_combat(),
                map: Map {
                    at: self.visited.last().copied(),
                    visited: self.visited.to_vec(),
                    travelable: self.travelable(),
                    points: self.map.shared(),
                    proceed: self.can_proceed(),
                },
                rng: self.positions(),
                room: self.room(),
                combat: self.fight_report(),
            },
            asking: self.asking(),
        }
    }

    /// Whether a fight is running, which is `CombatManager.IsInProgress`.
    fn in_combat(&self) -> bool {
        matches!(self.phase, Phase::Combat(_))
    }

    /// `MapLegality.CanProceed`, and the fight it is standing in being over.
    /// A boss room with its combat still running is an ordinary fight, and
    /// leaving one is not a move.
    /// `MapLegality.CanProceed`: the run stands in a boss room, and its fight
    /// is over. The room is the fight's, wherever on the map it was fought.
    fn can_proceed(&self) -> bool {
        let Phase::Rewards { combat, .. } = &self.phase else {
            return false;
        };
        // `ProceedFromTerminalRewardsScreen` goes back to the event.
        if self.resuming.is_some() {
            return true;
        }
        combat.encounter.is_some_and(|encounter| {
            <V::Encounter as sts2_core::EncounterTable>::room(encounter)
                == <V::RoomType as sts2_core::RoomKinds>::BOSS
        })
    }

    /// The relics the run carries, as a report lists them.
    fn carried(&self) -> impl ExactSizeIterator<Item = Carried<V>> + '_ {
        self.player.relics.iter().map(|held| Carried {
            relic: held.relic,
            amount: held.amount(),
            title: None,
            text: None,
        })
    }

    /// The fight the run is standing in, as its report shows it.
    fn fight_report(&self) -> Option<Combat<V>> {
        match &self.phase {
            Phase::Combat(live) => Some(live.report(&self.player.relics)),
            // `IsInProgress` is false once the loss is processed, and the
            // room it was fought in is still the room the run is standing in.
            // A won fight is still reported for the same reason: the run has
            // not left the room it was fought in.
            Phase::Lost { combat } => Some(combat.clone()),
            // The player's creature is the run's own, so what happens to it
            // between the fight and leaving the room shows on it.
            Phase::Rewards { combat, .. } => {
                let mut combat = combat.clone();
                let player = self.player_id(&combat);
                if let Some(creature) = combat.creatures.iter_mut().find(|c| c.id == player) {
                    creature.hp = self.player.hp;
                    creature.max_hp = self.player.max_hp;
                }
                Some(combat)
            }
            Phase::Map
            | Phase::Won
            | Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => None,
        }
    }

    /// The question the run is parked on, if it is parked.
    fn asking(&self) -> Option<Question<V>> {
        match &self.phase {
            Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::RestSite {
                asking: Some(_), ..
            }
            | Phase::Shop {
                asking: Some(_), ..
            } => self.phase.asking().map(|asking| {
                let (min, max, cards): (u32, u32, Vec<CardIdent<V>>) = match &asking.pickup {
                    Pickup::Rewards { offered, skippable } => {
                        return asked::<V>(offered, *skippable);
                    }
                    Pickup::Event => return self.event_question(),
                    Pickup::ConfirmAbandon => return Question::Confirm,
                    Pickup::Bundles { bundles } => {
                        return Question::ChooseBundle {
                            bundles: bundles
                                .iter()
                                .enumerate()
                                .map(|(index, bundle)| sts2_core::BundleOption {
                                    index,
                                    cards: bundle
                                        .iter()
                                        .map(|&card| sts2_core::RewardCard {
                                            card: CardIdent::new(card),
                                            text: None,
                                        })
                                        .collect(),
                                })
                                .collect(),
                        };
                    }
                    Pickup::Cards { offered, min, max } => (
                        u32::try_from(*min).expect("a selection counts in a few cards"),
                        u32::try_from(*max).expect("a selection counts in a few cards"),
                        offered.clone(),
                    ),
                    Pickup::CloneDeckCard { .. } => unreachable!("a room parks on no clone"),
                    Pickup::FromDeck { act, min, count } => (
                        u32::try_from(*min).expect("a selection counts in a few cards"),
                        u32::try_from(*count).expect("a selection counts in a few cards"),
                        self.offered(*act)
                            .into_iter()
                            .map(|(_, card)| card)
                            .collect(),
                    ),
                };
                Question::SelectCards {
                    min,
                    max,
                    asker: None,
                    options: cards
                        .into_iter()
                        .enumerate()
                        .map(|(index, card)| CardOption {
                            index,
                            card,
                            text: None,
                        })
                        .collect(),
                }
            }),
            Phase::Combat(live) => live.question(),
            // `CardSelectCmd.FromDeckForRemoval`: one card of the deck less
            // its `Eternal` ones, curses first.
            Phase::Rewards {
                removing: Some(_), ..
            } => Some(Question::SelectCards {
                min: 1,
                max: 1,
                asker: None,
                options: self
                    .offered(sts2_core::DeckAct::Remove)
                    .into_iter()
                    .enumerate()
                    .map(|(index, (_, card))| CardOption {
                        index,
                        card,
                        text: None,
                    })
                    .collect(),
            }),
            Phase::Rewards { offered, .. } => offered
                .iter()
                .any(|offer| !offer.taken())
                .then(|| asked::<V>(offered, true)),
            // `CardSelectCmd.FromDeckForUpgrade` offers the deck's upgradable
            // cards alone, so a card already at its highest is not offered and
            // the indices are that list's rather than the deck's. One card, and
            // the selection cannot be empty.
            Phase::RestSite { upgrading, .. } => upgrading.then(|| Question::SelectCards {
                min: 1,
                max: 1,
                asker: None,
                options: self
                    .upgradable()
                    .enumerate()
                    .map(|(index, (_, card))| CardOption {
                        index,
                        card,
                        text: None,
                    })
                    .collect(),
            }),
            // Each selection offers a different list. The removal reads the
            // deck less its `Eternal` cards, curses first, and a pickup reads
            // the deck less what its own filter leaves out. One card either
            // way, and neither can be empty.
            Phase::Shop { unfinished, .. } => {
                let offered = match unfinished {
                    None => None,
                    Some(Unfinished::Removal { .. }) => Some(
                        self.offered(sts2_core::DeckAct::Remove)
                            .into_iter()
                            .map(|(_, card)| card)
                            .collect(),
                    ),
                    Some(Unfinished::Pickup { excluding, .. }) => Some(self.clonable(*excluding)),
                    Some(Unfinished::Relic { .. }) => {
                        unreachable!("the parked pickup is asked first")
                    }
                };
                offered.map(|offered| Question::SelectCards {
                    min: 1,
                    max: 1,
                    asker: None,
                    options: offered
                        .into_iter()
                        .enumerate()
                        .map(|(index, card)| CardOption {
                            index,
                            card,
                            text: None,
                        })
                        .collect(),
                })
            }
            // An act opens on its map, which is no room and asks nothing.
            Phase::Map | Phase::Lost { .. } | Phase::Won | Phase::Treasure { .. } => None,
        }
    }

    /// The event the run stands in, as its page shows it. A finished one
    /// offers nothing.
    fn event_room(&self) -> Option<Room<V>> {
        let Phase::Event {
            state, finished, ..
        } = &self.phase
        else {
            return None;
        };
        let mut room = V::observe_event(state, self);
        if let Room::Event {
            finished: shown,
            options,
            ..
        } = &mut room
            && *finished
        {
            *shown = true;
            options.clear();
        }
        Some(room)
    }

    /// The room the run is standing in, as its report shows it.
    fn room(&self) -> Option<Room<V>> {
        let finished = self.phase.finished();
        match &self.phase {
            Phase::Neow {
                options: neow_options,
                chosen: neow_chosen,
                ..
            } => {
                let options = neow_options
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, relic)| {
                        let index = u8::try_from(index)
                            .expect("a page offers few enough options to number");
                        EventOption {
                            index,
                            key: EventOptionKey {
                                event: V::neow_event(),
                                option: V::EventOptionId::from_relic(relic),
                            },
                            title: None,
                            text: None,
                            locked: false,
                            proceed: false,
                            chosen: *neow_chosen == nonmax::NonMaxU8::new(index),
                            relic: Some(relic),
                        }
                    })
                    .collect();
                Some(Room::Event {
                    event: V::neow_event(),
                    finished,
                    title: None,
                    text: None,
                    options: if finished { Vec::new() } else { options },
                })
            }
            Phase::Event { .. } => self.event_room(),
            Phase::Treasure { relic, finished } => {
                // A taken relic empties the chest. A skip leaves its relic
                // showing, answered, until the room is left.
                let options = match *relic {
                    None => Vec::new(),
                    Some(relic) => vec![
                        sts2_core::ChestOption {
                            index: 0,
                            take: sts2_core::Take::Relic(relic),
                            chosen: *finished,
                        },
                        sts2_core::ChestOption {
                            index: 1,
                            take: sts2_core::Take::Skip,
                            chosen: *finished,
                        },
                    ],
                };
                Some(Room::Treasure {
                    finished: *finished,
                    options,
                })
            }
            Phase::RestSite { options, .. } => Some(Room::RestSite {
                options: options
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, id)| sts2_core::RestOption {
                        index: index as u8,
                        id,
                        enabled: self.rest_option_enabled(id),
                    })
                    .collect(),
            }),
            Phase::Shop { entries, .. } => {
                // `EnoughGold` and the potion slot check are derived, the way
                // the belt is. Both move as the run buys, and neither is a
                // fact about the shelf.
                let gold = self.player.gold;
                let room_for_a_potion = num_traits::cast::<_, usize>(self.player.potion_slots)
                    .expect("a potion slot count fits this machine")
                    > self.player.potions.len();
                let entries = entries
                    .iter()
                    .cloned()
                    .map(|entry| sts2_core::Entry {
                        cost: self.merchant_price(entry.cost),
                        affordable: self.merchant_price(entry.cost) <= gold,
                        blocked: match entry.stock {
                            sts2_core::Stock::Potion(_) if !room_for_a_potion => {
                                Some(sts2_core::Blocked::NoPotionSlot)
                            }
                            _ => None,
                        },
                        ..entry
                    })
                    .collect();
                Some(Room::Shop { entries })
            }
            Phase::Map
            | Phase::Combat(_)
            | Phase::Lost { .. }
            | Phase::Won
            | Phase::Rewards { .. } => None,
        }
    }
}

impl<V: Rules> State<V> {
    fn upgrade_random_deck_cards(
        &mut self,
        count: usize,
        stream: sts2_core::Stream<V>,
        kind: Option<sts2_core::CardKind>,
    ) {
        use sts2_core::CardTable as _;
        use sts2_core::rng::GameRng as _;

        // `CardSelectCmd` shuffles the upgradable cards alone, so a card
        // already at its highest neither takes a place nor moves the shuffle.
        let deck = self.player.deck.clone();
        let mut upgradable: Vec<usize> = self
            .upgradable()
            .filter(|(_, card)| kind.is_none_or(|kind| V::card_kind(card.card) == Some(kind)))
            .map(|(at, _)| at)
            .collect();
        stream_of(&mut self.phase, &mut self.streams, stream)
            .stable_shuffle_by_key(&mut upgradable, |&at| {
                (deck[at].ident.card.order(), deck[at].ident.upgrade)
            });
        for at in upgradable.into_iter().take(count) {
            self.player.deck_mut()[at].ident.upgrade += 1;
        }
    }
}

impl<V: Rules> sts2_core::EventView<V> for State<V> {
    fn act(&self) -> V::Count {
        num_traits::cast(self.act()).expect("an act index fits the build count")
    }

    fn floor(&self) -> V::Count {
        self.floor
    }

    fn gold(&self) -> V::Number {
        self.player.gold
    }

    fn hp(&self) -> V::Health {
        self.player.hp
    }

    fn max_hp(&self) -> V::Health {
        self.player.max_hp
    }

    fn potions(&self) -> impl Iterator<Item = V::Potion> + '_ {
        self.player.potions.iter().copied()
    }

    fn open_potion_slot(&self) -> bool {
        let slots: usize =
            num_traits::cast(self.player.potion_slots).expect("a belt's slots fit a usize");
        self.player.potions.len() < slots
    }

    fn character(&self) -> V::Character {
        self.shared.character
    }

    fn relics(&self) -> impl Iterator<Item = sts2_core::CarriedRelic<V>> + '_ {
        use sts2_core::RelicModel as _;

        self.player
            .relics
            .iter()
            .map(|held| sts2_core::CarriedRelic {
                relic: held.relic,
                used_up: held.state.used_up(held.relic),
            })
    }

    fn deck(&self) -> Vec<CardIdent<V>> {
        self.player.deck.iter().map(|held| held.ident).collect()
    }

    fn relics_left(&self) -> bool {
        self.shared
            .opening
            .player_relics
            .has_left(&self.dealt_relics)
    }
}

/// Where the player stands, from whichever half of the run is holding it.
struct Standing<V: Rules> {
    hp: V::Health,
    max_hp: V::Health,
    alive: bool,
}

/// The rewards screen a won fight parks the run on.
///
/// Every reward stays in the list once it is taken, because the indices an
/// answer names never renumber.
/// A room option's position, as `ChooseOption` names it.
fn option_index(at: usize) -> u8 {
    u8::try_from(at).expect("a room offers few enough options to number")
}

/// Taking each reward still on offer, each card of a card reward on its own,
/// and skipping the rest.
fn reward_moves<V: Rules>(offered: &[Offer<V>], skippable: bool, moves: &mut Legal<V>) {
    for (index, offer) in offered.iter().enumerate() {
        if offer.taken() {
            continue;
        }
        match offer {
            Offer::Card { cards, .. } => {
                moves.extend(
                    (0..cards.len())
                        .map(|card| Decision::Answer(Answer::Take(TakeReward::card(index, card)))),
                );
                moves.extend(offer.alternatives().map(|alternative| {
                    Decision::Answer(Answer::Take(TakeReward::alternative(index, alternative)))
                }));
            }
            _ => moves.push(Decision::Answer(Answer::Take(TakeReward::non_card(index)))),
        }
    }
    if skippable {
        moves.push(Decision::Answer(Answer::Skip));
    }
}

/// The answers to an event's own question: each hidden cell of a grid, with
/// either tool.
fn event_moves<V: Rules>(question: Question<V>, moves: &mut Legal<V>) {
    let Question::Divine {
        width,
        height,
        cleared,
        ..
    } = question
    else {
        unimplemented!("the answers to {question:?}")
    };
    for x in 0..width {
        for y in 0..height {
            if cleared.contains(&sts2_core::Cell { x, y }) {
                continue;
            }
            for tool in [sts2_core::Tool::Small, sts2_core::Tool::Big] {
                moves.push(Decision::Answer(Answer::Divine { x, y, tool }));
            }
        }
    }
}

fn asked<V: Rules>(offered: &[Offer<V>], skippable: bool) -> Question<V> {
    Question::Rewards {
        skippable,
        rewards: offered
            .iter()
            .enumerate()
            .map(|(index, offer)| RewardDetail {
                index,
                // Nothing offers a reward it has not rolled: the whole set is
                // populated where it is generated.
                populated: true,
                taken: offer.taken(),
                reward: match offer {
                    Offer::Gold { amount, .. } => RewardKind::Gold {
                        amount: amount
                            .to_i64()
                            .expect("a fight's gold fits the reported width"),
                    },
                    Offer::Potion { potion, .. } => RewardKind::Potion {
                        potion: Some(*potion),
                    },
                    // The slot picks no rarity of its own, so it reports
                    // `RelicRarity.None` whatever the relic in it is.
                    Offer::Relic { relic, rarity, .. } => RewardKind::Relic {
                        relic: Some(*relic),
                        rarity: *rarity,
                    },
                    Offer::Card { cards, .. } => RewardKind::Card {
                        cards: cards
                            .iter()
                            .map(|card| RewardCard {
                                card: *card,
                                // The game formats this from its localisation
                                // tables, which the simulator does not have.
                                text: None,
                            })
                            .collect(),
                        alternatives: offer.alternatives().collect(),
                    },
                    Offer::SpecialCard { .. } => RewardKind::SpecialCard { card: None },
                    Offer::CardRemoval { .. } => RewardKind::Other {
                        kind: "CardRemovalReward".into(),
                    },
                },
            })
            .collect(),
    }
}

/// The rarity a pull deals at: its own, or one rolled off `rewards`.
fn pulled_rarity<V: Rules>(pull: sts2_core::RelicPull<V>, rewards: &mut MegaRng) -> Rarity<V> {
    match pull {
        sts2_core::RelicPull::Rolled => crate::reward::relic_rarity::<V, _>(rewards),
        sts2_core::RelicPull::Stocked(rarity) => rarity,
    }
}

/// The generator a stream names: one of the run's or the player's, or the
/// event the run stands in.
///
/// Over the two fields rather than the run, so a caller keeps the rest of the
/// run to read while it draws.
fn stream_of<'a, V: Rules>(
    phase: &'a mut Phase<V>,
    streams: &'a mut Streams<V>,
    stream: sts2_core::Stream<V>,
) -> &'a mut MegaRng {
    match (stream, phase) {
        (sts2_core::Stream::Event, Phase::Event { rng, .. }) => rng,
        (sts2_core::Stream::Event, _) => unreachable!("an event's stream outside its room"),
        (stream, _) => streams.of(stream),
    }
}

/// `card` after the effects relics asked for it: a clone upgraded or
/// enchanted, which takes its place.
///
/// # Panics
///
/// A relic asking for anything else.
fn rewrite<V: Rules>(mut card: CardIdent<V>, asked: sts2_core::HookEffects<V>) -> CardIdent<V> {
    for effect in asked {
        match effect {
            sts2_core::HookEffect::UpgradeCard => card.upgrade += 1,
            sts2_core::HookEffect::EnchantCard {
                enchantment,
                amount,
            } => V::enchant(enchantment, amount, &mut card),
            effect => unimplemented!("a relic asking for {effect:?} of a card it rewrites"),
        }
    }
    card
}

#[cfg(test)]
mod tests {
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::ascension::AscensionLevel;
    use sts2_content::v0_107_1::characters::Character;
    use sts2_core::Seed;

    use super::*;

    type V = V0_107_1;

    fn opened() -> State<V> {
        State::start(
            &Seed::new("0RACLE1").expect("the fixture's seed is a seed"),
            Character::Regent,
            AscensionLevel::None,
        )
    }

    /// A run whose player falls reports itself over, and where it fell.
    ///
    /// `RunState.IsGameOver` is every player being dead, and
    /// `CurrentRoom.IsVictoryRoom` is what tells a defeat from a win. The room
    /// the run died in is still the room it stands in, so the fight is still
    /// reported and the fighting is over.
    #[test]
    fn a_run_whose_player_died_is_over_and_lost() {
        use sts2_content::v0_107_1::encounters::Encounter;
        use sts2_content::v0_107_1::potions::Potion;
        use sts2_core::{Gift, SideKinds, Undrinkable};

        let mut run = opened();
        run.grant(&Gift::Potion(Potion::BlockPotion))
            .expect("a potion can be granted");
        run.fight_here(Encounter::SlimesWeak)
            .expect("the simulator can build this fight");

        // Where the player fell, which is what the run keeps.
        let mut combat = run
            .fight()
            .expect("the run is in the fight")
            .report(&run.player.relics);
        let player = combat
            .creatures
            .iter_mut()
            .find(|c| c.side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER)
            .expect("a fight has the player in it");
        player.hp = 0;
        player.alive = false;
        run.lost(combat);

        let seen = run.report().observation;
        assert!(!seen.alive, "the player is dead");
        assert!(seen.run_over, "so the run is over");
        assert_eq!(seen.victory, Some(false), "and it was not a victory");
        assert!(!seen.in_combat, "the fighting is over");
        assert!(
            seen.combat.is_some(),
            "the run has not left the room it died in"
        );
        assert_eq!(
            seen.potions
                .iter()
                .map(|held| held.reason)
                .collect::<Vec<_>>(),
            [Some(Undrinkable::Dead)],
            "a dead run drinks nothing"
        );
    }

    /// Damage outside a fight that takes the last hit point ends the run
    /// where it stands, as an event's does. Nothing more is taken, which is
    /// `IsGameOver` refusing every command.
    #[test]
    fn a_run_whose_player_dies_outside_a_fight_is_over_and_takes_nothing() {
        let mut run = opened();
        run.player.hp = 3;
        run.damage_owner(13);

        let seen = run.report().observation;
        assert!(!seen.alive, "the player is dead");
        assert_eq!(seen.hp, 0, "with no hit points left");
        assert!(seen.run_over, "so the run is over");
        assert_eq!(seen.victory, Some(false), "and it was not a victory");
        assert!(run.legal().is_empty(), "and it takes nothing more");
    }

    /// Past the last act's boss the run is won: `WinRun` kills every player,
    /// and the room it ends in is the victory room.
    #[test]
    fn a_run_past_its_last_act_is_over_and_won() {
        let mut run = opened();
        run.act = run.shared.opening.acts.len() - 1;
        run.proceed()
            .expect("the last act's boss room is left by proceeding");

        let seen = run.report().observation;
        assert!(seen.run_over, "so the run is over");
        assert!(!seen.alive, "and every player is dead");
        assert_eq!(seen.victory, Some(true), "and it was a victory");
    }

    /// `CardSelectCmd.FromDeckForUpgrade` offers the upgradable cards alone,
    /// so an answer's index is that list's rather than the deck's.
    ///
    /// The case that says which is a deck holding two copies of one card, one
    /// of them already upgraded. A selection over the whole deck lands on the
    /// copy before it.
    /// `NRestSiteRoom` puts its proceed button up once an option went
    /// through. Under Miniature Tent the other options stay beside it.
    #[test]
    fn a_rest_site_under_miniature_tent_offers_its_options_and_travel() {
        use sts2_content::v0_107_1::relics::Relic;
        use sts2_core::{Gift, RestKinds};

        let mut run = opened();
        run.grant(&Gift::Relic(Relic::MiniatureTent))
            .expect("a relic can be granted");
        run.phase = Phase::RestSite {
            options: vec![
                <V as sts2_core::ContentVersion>::RestSiteOption::HEAL,
                <V as sts2_core::ContentVersion>::RestSiteOption::SMITH,
            ],
            chosen: None,
            rested: false,
            upgrading: false,
            asking: None,
        };
        let travels = |run: &State<V>| {
            run.legal()
                .iter()
                .any(|decision| matches!(decision, Decision::Travel { .. }))
        };
        assert!(
            !travels(&run),
            "nothing leaves before an option goes through"
        );

        run.choose(0).expect("the heal can be chosen");
        let legal = run.legal();
        assert!(
            legal.contains(&Decision::ChooseOption { index: 0 }),
            "the smith is still up"
        );
        assert!(travels(&run), "and the run can leave beside it");
    }

    #[test]
    fn the_smith_offers_the_cards_it_can_upgrade_and_numbers_those() {
        use sts2_content::v0_107_1::cards::{Cards, RegentCard};
        use sts2_core::{Answer, Question, RestKinds};

        let mut run = opened();
        let strike = CardIdent::new(Cards::Regent(RegentCard::Strike));
        let upgraded = CardIdent {
            upgrade: 1,
            ..CardIdent::new(Cards::Regent(RegentCard::BigBang))
        };
        let big_bang = CardIdent::new(Cards::Regent(RegentCard::BigBang));
        let deck = [strike, upgraded, big_bang]
            .into_iter()
            .map(|card| sts2_core::DeckCard::new(run.player.mint(), card))
            .collect();
        run.player.deck = Arc::new(deck);
        run.phase = Phase::RestSite {
            options: vec![
                <V as sts2_core::ContentVersion>::RestSiteOption::HEAL,
                <V as sts2_core::ContentVersion>::RestSiteOption::SMITH,
            ],
            chosen: None,
            rested: false,
            upgrading: false,
            asking: None,
        };

        run.choose(1).expect("the smith can be chosen");
        let parked = run.report();
        let Some(Question::SelectCards { options, .. }) = parked.asking else {
            panic!("the smith asks which card to upgrade: {:?}", parked.asking);
        };
        assert_eq!(
            options.iter().map(|option| option.card).collect::<Vec<_>>(),
            [strike, big_bang],
            "the copy already upgraded is not offered"
        );

        run.answer(&Answer::Cards([1].into()))
            .expect("the second offered card can be upgraded");
        assert_eq!(
            run.player
                .deck
                .iter()
                .map(|held| held.ident)
                .collect::<Vec<_>>(),
            vec![
                strike,
                upgraded,
                CardIdent {
                    upgrade: 1,
                    ..big_bang
                }
            ],
            "the index names the offered list, not the deck"
        );
    }

    /// A branch has to be able to cross a thread, so that searching a run in
    /// parallel is a matter of handing the branches out.
    ///
    /// Nothing needs it yet. One `Rc` or one `RefCell` on a field loses it,
    /// and this fails on that commit rather than years later.
    #[test]
    fn a_branch_can_cross_a_thread() {
        fn assert<T: Send + Sync>() {}
        assert::<State<V>>();
        assert::<crate::Sim<V>>();
    }

    #[test]
    fn branching_a_run_shares_the_setup_it_drew() {
        let run = opened();
        let branch = run.clone();
        assert!(Arc::ptr_eq(&run.shared, &branch.shared));
    }

    /// A constructed fight is not a move: the floor advances, the map does
    /// not, and the act's own encounter list is not drawn from.
    #[test]
    fn a_fight_can_be_constructed_without_travelling_to_one() {
        use sts2_content::v0_107_1::encounters::Encounter;

        let mut run = opened();
        let before = run.report().observation;
        run.fight_here(Encounter::SlimesWeak)
            .expect("the simulator can build this fight");
        let after = run.report().observation;

        assert_eq!(after.map.at, before.map.at, "the map does not move");
        assert_eq!(after.map.visited, before.map.visited);
        assert!(after.floor > before.floor, "the floor does");
        assert!(after.room.is_none(), "the room it was in is abandoned");

        let combat = after.combat.expect("the run is in the fight");
        assert_eq!(combat.encounter, Some(Encounter::SlimesWeak));
        assert_eq!(run.normal_encounter(0), before_next(&run));
    }

    /// `BoneTea` opens at one combat and spends it on the first hand it sees.
    ///
    /// The relic stays in the belt after that, as it does on screen, and the
    /// second fight's hand is dealt as printed.
    #[test]
    fn bone_tea_upgrades_the_one_opening_hand_it_counted() {
        use sts2_content::v0_107_1::cards::{Cards, RegentCard};
        use sts2_content::v0_107_1::encounters::Encounter;
        use sts2_content::v0_107_1::relics::Relic;

        let mut run = opened();
        run.player.relics.push(HeldRelic::new(Relic::BoneTea));
        // A deck of the one card whose upgrade is ported. What upgrading does
        // to a card is the card's own rule, and this is about the relic that
        // asks for it.
        let deck = std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 10)
            .map(|card| sts2_core::DeckCard::new(run.player.mint(), card))
            .collect();
        run.player.deck = Arc::new(deck);

        let mut opening_hands = Vec::new();
        for _ in 0..2 {
            run.fight_here(Encounter::SlimesWeak)
                .expect("the simulator can build this fight");
            let combat = run
                .report()
                .observation
                .combat
                .expect("the run is in the fight");
            opening_hands.push(
                combat
                    .hand
                    .iter()
                    .map(|card| card.upgrade)
                    .collect::<Vec<_>>(),
            );
        }

        let [first, second] = opening_hands.as_slice() else {
            unreachable!("two fights were entered")
        };
        assert!(
            first.iter().all(|upgrade| *upgrade == 1),
            "the first hand is upgraded: {first:?}"
        );
        assert!(
            second.iter().all(|upgrade| *upgrade == 0),
            "and the second is not: {second:?}"
        );
        assert!(
            run.player
                .relics
                .iter()
                .any(|held| held.relic == Relic::BoneTea),
            "a spent relic stays in the belt"
        );
    }

    /// `Pocketwatch` counts the cards a turn plays, and pays the quiet ones.
    ///
    /// Three more cards next turn for a turn that played at most three, which
    /// is `ModifyHandDraw` reading the count this rotates at every turn start.
    #[test]
    fn pocketwatch_counts_a_turns_plays_and_draws_on_a_quiet_one() {
        use sts2_content::v0_107_1::cards::{Cards, RegentCard};
        use sts2_content::v0_107_1::encounters::Encounter;
        use sts2_content::v0_107_1::relics::Relic;
        use sts2_core::fixture::{at, hand};

        let mut run = opened();
        run.player.relics.push(HeldRelic::new(Relic::Pocketwatch));
        let deck = std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 20)
            .map(|card| sts2_core::DeckCard::new(run.player.mint(), card))
            .collect();
        run.player.deck = Arc::new(deck);
        run.fight_here(Encounter::SlimesWeak)
            .expect("the simulator can build this fight");

        let shown = |run: &State<V>| {
            let observation = run.report().observation;
            let relic = observation
                .relics
                .iter()
                .find(|carried| carried.relic == Relic::Pocketwatch)
                .expect("the belt holds the relic");
            relic.amount
        };
        assert_eq!(shown(&run), 0, "nothing has been played yet");

        run.play_card(hand(0), Some(at(1)))
            .expect("a Strike is ported")
            .expect("and is legal here");
        assert_eq!(shown(&run), 1, "one card played this turn");

        run.end_turn()
            .expect("the simulator can end this turn")
            .expect("and the turn is not refused");
        let combat = run
            .report()
            .observation
            .combat
            .expect("the run is still in the fight");
        assert_eq!(
            combat.hand.len(),
            8,
            "five printed, three for the quiet turn"
        );
        assert_eq!(shown(&run), 0, "the new turn has played nothing");
    }

    /// A card a monster takes for good leaves the deck once, however many
    /// decisions the fight goes on for.
    ///
    /// The fight keeps every copy it took, to offer back if it is won. So each
    /// decision takes out only what it added. Otherwise a deck of identical
    /// cards loses one more copy on every decision after the theft.
    #[test]
    fn a_stolen_card_leaves_the_deck_once() {
        use sts2_content::v0_107_1::cards::{Cards, RegentCard};
        use sts2_content::v0_107_1::encounters::Encounter;
        use sts2_core::fixture::{at, hand};

        let mut run = opened();
        let deck = std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 20)
            .map(|card| sts2_core::DeckCard::new(run.player.mint(), card))
            .collect();
        run.player.deck = Arc::new(deck);
        // The hopper's graph opens on `ThieveryMove`.
        run.fight_here(Encounter::ThievingHopperWeak)
            .expect("the simulator can build this fight");

        run.end_turn()
            .expect("the simulator can end this turn")
            .expect("and the turn is not refused");
        assert_eq!(run.player.deck.len(), 19, "the hopper took one Strike");

        run.play_card(hand(0), Some(at(1)))
            .expect("a Strike is ported")
            .expect("and is legal here");
        assert_eq!(
            run.player.deck.len(),
            19,
            "a decision after the theft takes nothing more"
        );
    }

    /// A fight takes the player's hit points with it, and hands them back.
    ///
    /// The run and the fight each hold a number and the game holds one. So
    /// this is the test that they are the same number.
    #[test]
    fn what_a_fight_costs_the_player_leaves_the_fight() {
        use sts2_content::v0_107_1::encounters::Encounter;

        let mut run = opened();
        run.fight_here(Encounter::SlimesWeak)
            .expect("the simulator can build this fight");
        let before = run.report().observation;
        run.end_turn()
            .expect("the simulator can end this turn")
            .expect("and the turn is not refused");
        let after = run.report().observation;

        assert!(
            after.hp < before.hp,
            "the slimes hit for something: {} then {}",
            before.hp,
            after.hp
        );
        assert_eq!(after.max_hp, before.max_hp, "and not for the maximum");
        let combat = after.combat.expect("the run is still in the fight");
        let player = &combat.creatures[0];
        assert_eq!(
            (player.hp, player.max_hp),
            (after.hp, after.max_hp),
            "the run says what the fight says"
        );
    }

    /// The encounter a travelling run meets next, which a constructed fight
    /// leaves where it was.
    fn before_next(run: &State<V>) -> Option<<V as sts2_core::ContentVersion>::Encounter> {
        run.shared.opening.acts[0].1.normal.first().copied()
    }

    #[test]
    fn a_branch_keeps_its_own_play() {
        let run = opened();
        let mut branch = run.clone();
        branch
            .choose(0)
            .expect("the simulator implements this option");
        assert_eq!(run.player.relics.len() + 1, branch.player.relics.len());
    }
}
