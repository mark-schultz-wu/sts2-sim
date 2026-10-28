use std::sync::Arc;

mod observed;
mod pickup;

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
use crate::legal::Legal;
use crate::map::ActMap;
use crate::reward::{Odds, Offer};
use crate::run::{Opening, Rarity, Settled};
use crate::stream::Streams;
use sts2_core::Gift;
use sts2_core::{Reason, Refusal};

/// The authoritative state of the run slice implemented so far.
///
/// Reports are built from this state. They are never stored and edited as if
/// they were the run itself.
#[derive(Debug)]
pub(crate) struct State<V: Rules> {
    shared: Arc<RunData<V>>,
    /// What each of the run's modifiers saves, in their order. Shared until
    /// a modifier writes it.
    modifier_states: Arc<Vec<V::ModifierState>>,
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
    players: Players<V>,
    /// `MapSelectionSynchronizer`'s `map_point_selection` stream, which the
    /// host draws a run of several players' destination from.
    map_votes: MegaRng,
    /// Each player's relics, by seat. Apart from the rest of a player, so
    /// a fight can borrow every belt at once.
    belts: Belts<V>,
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
    unknown_odds: [f32; 4],
    /// Every relic that has left the shared bag, which a treasure chest pulls
    /// from, in the order it left. A chest deals out of it alone. A pull from
    /// any player's bag, a merchant's stock, an obtain and a modifier's
    /// removal take the relic out of both.
    shared_dealt: Arc<Vec<V::Relic>>,
    /// Each time the shared bag refilled a spent rarity, which only it does:
    /// the rarity, and how far `shared_dealt` stood then. What left after
    /// that left the refilled deque.
    shared_refills: Arc<Vec<(Rarity<V>, usize)>>,
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
    /// What the fight, won, left in `CombatRoom.ExtraRewards`, where it left
    /// anything.
    pub(super) extras: Option<Box<super::fight::Extras<V>>>,
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

/// `UnknownMapPointOdds`, as a run opens with them, in [`UnknownRoom`]'s
/// order.
///
/// They are what an unknown room resolves to, and they climb as a run walks
/// into ones that stay events. The elite's are negative, so it never rolls
/// unless a modifier says otherwise.
const UNKNOWN_ODDS: [f32; 4] = [0.1, -1.0, 0.02, 0.03];

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
    ascension: V::AscensionLevel,
    /// `UnknownMapPointOdds`'s base odds, in [`UnknownRoom`]'s order. Each
    /// goes back to its base once rolled and as an act opens, and rises by
    /// it when another room rolls. A modifier can set them as the run is
    /// created.
    unknown_base: [f32; 4],
    /// `RunState.Modifiers`, in the order the run was started with them.
    modifiers: Vec<sts2_core::RunModifier<V>>,
}

/// A reward of a set while `Hook.ModifyRewards` runs. It is filled in
/// already, or a hook added or swapped it in, and `Populate` fills it in
/// once the hooks are through.
enum Filling<V: Rules> {
    Filled(Offer<V>),
    /// `RelicReward(player)`, of no set rarity.
    Relic,
    /// `CardReward` at the odds of `room`, rerollable once `CanReroll` is set.
    Card {
        room: crate::reward::combat::Room,
        reroll: bool,
    },
}

/// An act's layout, which a run of several players walks a room shorter:
/// `GetNumberOfRooms(isMultiplayer)`.
fn layout<V: Rules>(act: V::Act, multiplayer: bool) -> sts2_core::Layout {
    let mut layout = V::act_layout(act);
    layout.rooms -= usize::from(multiplayer);
    layout
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

/// The health an ancient's heal leaves the run at, from `hp` of `max_hp`.
/// `SetCurrentHpInternal` truncates what four fifths of the gap leaves.
fn ancient_heal<V: Rules>(
    ascension: V::AscensionLevel,
    hp: V::Health,
    max_hp: V::Health,
) -> V::Health {
    use sts2_core::AscensionKinds as _;

    match ascension.has(<V::AscensionLevel as sts2_core::AscensionKinds>::WEARY_TRAVELER) {
        true => {
            let (four, five): (V::Health, V::Health) = (
                num_traits::cast(4).expect("four fits a health"),
                num_traits::cast(5).expect("five fits a health"),
            );
            hp + (max_hp - hp) * four / five
        }
        false => max_hp,
    }
}

/// `MerchantCardRemovalEntry.CalcCost`, in the build's own number.
fn removal_cost<V: Rules>(used: usize, ascension: V::AscensionLevel) -> V::Number {
    num_traits::cast(super::shop::removal_cost::<V>(used, ascension))
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

    /// The number it shows, which is `RelicModel.DisplayAmount`, while a
    /// fight is `fighting` or not, on the player's `turn`.
    pub(crate) fn amount(&self, fighting: bool, turn: Option<V::Count>) -> V::Number {
        self.state.display_amount(self.relic, fighting, turn)
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
    /// Each slot's relic, apart from its saved fields, so a relic's hook can
    /// read the belt while it writes its own fields.
    relics: SmallVec<[V::Relic; BELT_INLINE]>,
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
            relics: self.relics.clone(),
            heard: self.heard,
            slots: self.slots,
        }
    }

    /// Into the belt already here, so its lists keep what they allocated.
    fn clone_from(&mut self, source: &Self) {
        self.held.clone_from(&source.held);
        self.relics.clone_from(&source.relics);
        self.heard = source.heard;
        self.slots = source.slots;
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
            relics: SmallVec::new(),
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
        self.relics.push(held.relic);
        self.held.push(held);
    }

    /// `Owner.Relics`, which a relic's hook reads.
    pub(crate) fn relics(&self) -> &[V::Relic] {
        &self.relics
    }

    /// The saved fields of the relic in `slot`, beside the belt it sits on.
    pub(crate) fn state_beside(&mut self, slot: usize) -> (&mut V::RelicState, &[V::Relic]) {
        (&mut self.held[slot].state, &self.relics)
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

    /// Whether any relic on the belt overrides a hook in `hooks`.
    pub(crate) fn hear(&self, hooks: sts2_core::Overrides) -> bool {
        self.heard.intersects(hooks)
    }

    /// Each relic on the belt hears its hooks again, as a revived player's
    /// do.
    pub(crate) fn rehear(&mut self) {
        let held = std::mem::take(&mut self.held);
        *self = held.into_iter().collect();
    }

    /// No relic on the belt hears a hook after, as a dead player's relics
    /// hear none.
    pub(crate) fn deafen(&mut self) {
        self.heard = sts2_core::Overrides::default();
        self.slots = [0; sts2_core::ModelHook::ALL.len()];
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

/// `RunState.Players`: each player, in seat order.
pub(crate) type Players<V> = crate::seats::BySeat<Player<V>>;

/// Each player's relic belt, in seat order.
pub(crate) type Belts<V> = crate::seats::BySeat<Belt<V>>;

/// The small, mutable part of one player's run state.
#[derive(Debug)]
pub(crate) struct Player<V: Rules> {
    character: V::Character,
    /// See [`sts2_core::Player::wins`].
    wins: u32,
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
    /// `IsActiveForHooks` off: the player died, and their deck, relics and
    /// potions hear no hook after.
    deaf: bool,
    /// The cards a select-cards question has been handed so far, in the order
    /// they were picked. Empty whenever no selection is under way.
    picked: sts2_core::Picks,
    odds: Odds,
    /// Every relic this run has dealt out of a bag, in the order it dealt
    /// them.
    ///
    /// The bags sit in the setup that every branch of a run shares. A branch
    /// that deals a relic must not deal it to the other, so what moves lives
    /// here. It is a handful over a run, not the whole table.
    dealt_relics: Arc<Vec<V::Relic>>,
    /// `PlayerCombatState.TurnNumber` as the last fight left it. A fight's
    /// state outlives it until the next one starts.
    last_turn: Option<V::Count>,
    /// `PlayerExtraFields.CardShopRemovalsUsed`, which every merchant's
    /// removal price climbs with. It counts over the run, not the room.
    removals_used: usize,
    /// Card rewards that left their set without being offered or skipped,
    /// as Vintage swaps a relic in for one. Each still hears
    /// `RelicObtained`, which only its own selection or skip turns off, for
    /// the rest of the run.
    unheld_card_rewards: Arc<Vec<Offer<V>>>,
    /// Where this player voted to travel, until the run moves.
    vote: Option<Coord>,
}

impl<V: Rules> Clone for State<V> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
            modifier_states: Arc::clone(&self.modifier_states),
            map: Arc::clone(&self.map),
            spoils: self.spoils,
            players: self.players.clone(),
            belts: self.belts.clone(),
            visited_events: Arc::clone(&self.visited_events),
            shared_dealt: Arc::clone(&self.shared_dealt),
            shared_refills: Arc::clone(&self.shared_refills),
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
            modifier_states,
            map,
            spoils,
            act,
            floor,
            visited,
            players,
            map_votes,
            belts,
            streams,
            normal_fights,
            elite_fights,
            events_used,
            visited_events,
            unknown_odds,
            shared_dealt,
            shared_refills,
            phase,
            resuming,
        } = self;
        shared.clone_from(&source.shared);
        modifier_states.clone_from(&source.modifier_states);
        map.clone_from(&source.map);
        *spoils = source.spoils;
        *act = source.act;
        *floor = source.floor;
        visited.copy_from(&source.visited);
        players.clone_from(&source.players);
        *map_votes = source.map_votes;
        belts.clone_from(&source.belts);
        *streams = source.streams;
        *normal_fights = source.normal_fights;
        *elite_fights = source.elite_fights;
        *events_used = source.events_used;
        visited_events.clone_from(&source.visited_events);
        *unknown_odds = source.unknown_odds;
        shared_dealt.clone_from(&source.shared_dealt);
        shared_refills.clone_from(&source.shared_refills);
        resuming.clone_from(&source.resuming);
        match (phase, &source.phase) {
            (Phase::Combat(live), Phase::Combat(from)) => live.clone_from(from),
            (
                phase @ (Phase::Rewards {
                    spent: Spent(Some(_)),
                    ..
                }
                | Phase::Lost {
                    spent: Spent(Some(_)),
                    ..
                }),
                Phase::Combat(from),
            ) => {
                let (Phase::Rewards { spent, .. } | Phase::Lost { spent, .. }) = phase else {
                    unreachable!("the arm matched a phase holding a spent fight")
                };
                let mut live = spent.0.take().expect("the arm matched a spent fight");
                live.clone_from(from);
                *phase = Phase::Combat(live);
            }
            (phase, from) => *phase = from.clone(),
        }
    }
}

impl<V: Rules> Clone for Player<V> {
    fn clone(&self) -> Self {
        Self {
            character: self.character,
            wins: self.wins,
            potions: self.potions,
            potion_positions: self.potion_positions,
            deck: Arc::clone(&self.deck),
            minted: self.minted,
            hp: self.hp,
            max_hp: self.max_hp,
            gold: self.gold,
            potion_slots: self.potion_slots,
            deaf: self.deaf,
            picked: self.picked,
            odds: self.odds,
            dealt_relics: Arc::clone(&self.dealt_relics),
            last_turn: self.last_turn,
            removals_used: self.removals_used,
            unheld_card_rewards: Arc::clone(&self.unheld_card_rewards),
            vote: self.vote,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        let Player {
            character,
            wins,
            potions,
            potion_positions,
            deck,
            minted,
            hp,
            max_hp,
            gold,
            potion_slots,
            deaf,
            picked,
            odds,
            dealt_relics,
            last_turn,
            removals_used,
            unheld_card_rewards,
            vote,
        } = self;
        *character = source.character;
        *wins = source.wins;
        potions.copy_from(&source.potions);
        potion_positions.copy_from(&source.potion_positions);
        deck.clone_from(&source.deck);
        *minted = source.minted;
        *hp = source.hp;
        *max_hp = source.max_hp;
        *gold = source.gold;
        *potion_slots = source.potion_slots;
        *deaf = source.deaf;
        *picked = source.picked;
        *odds = source.odds;
        dealt_relics.clone_from(&source.dealt_relics);
        *last_turn = source.last_turn;
        *removals_used = source.removals_used;
        unheld_card_rewards.clone_from(&source.unheld_card_rewards);
        *vote = source.vote;
    }
}

/// One step of a pickup: an effect to carry out, a walk over a hook's
/// listeners, or a question to ask.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Then<V: Rules> {
    Effect(sts2_core::HookEffect<V>),
    Walk(RunWalking<V>),
    Ask(Pickup<V>),
    /// `ClearAfterPurchase` of the shelf a merchant sold, once its pickup
    /// is done, for the gold it cost. A Lord's Parasol purchase goes on from
    /// the next shelf.
    Purchased {
        shelf: u8,
        spent: u32,
        parasol: bool,
    },
}

/// A pickup's steps, in order.
pub(super) type Steps<V> = Vec<Then<V>>;

/// What one step of a pickup, or the answer to its question, came to.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Stepped<V: Rules> {
    /// Done, and the pickup goes on.
    Done,
    /// A question to ask.
    Ask(Pickup<V>),
    /// A walk over a hook's listeners.
    Walk(RunWalking<V>),
    /// The steps of a pickup of the seat's own, such as a relic obtained or
    /// an event resumed, which run first. Then what comes after them.
    Nested(sts2_core::SeatIndex, Steps<V>, After<V>),
}

/// What a pickup goes on with once a pickup inside it is done.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum After<V: Rules> {
    /// Its next step.
    Done,
    /// The reward screen the inner pickup was taken off, asked again while
    /// it has rewards left. It first hears the relics obtained since the
    /// belt held `since`, then marks the reward at `taking` taken.
    Screen {
        offered: Vec<Offer<V>>,
        skippable: bool,
        since: usize,
        taking: Option<usize>,
    },
    /// The rest of a walk, whose listener asked for the inner pickup.
    Walk(RunWalking<V>),
}

impl<V: Rules> From<sts2_core::HookEffect<V>> for Then<V> {
    fn from(effect: sts2_core::HookEffect<V>) -> Self {
        Then::Effect(effect)
    }
}

/// A walk outside a fight partway through: the hook, and the listeners the
/// game listed as it began that it has still to ask.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RunWalking<V: Rules> {
    hook: sts2_core::EventHook<V>,
    listed: RunListed<V>,
}

/// The listeners [`State::run_listed`] found, in the game's order: every
/// player's deck, then each player's belt and potions, then the rest.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RunListed<V: Rules> {
    deck: SmallVec<[RunListener<V>; 2]>,
    /// Each player's listening relic slots, in seat order, one bit a slot,
    /// the first slot lowest. Many relics hear a room's hooks, so a belt
    /// stays a set.
    belts: SmallVec<[(sts2_core::SeatIndex, u64); 1]>,
    /// Each player's potions, in seat order, which follow that player's
    /// relics. Then the run's modifiers.
    after: SmallVec<[RunListener<V>; 2]>,
    /// How far a walk has taken the list.
    taken: Cursor,
}

/// A place in a [`RunListed`]: how much of each part is behind it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Cursor {
    /// The slots of the belt under way still to come.
    relics: u64,
    deck: u16,
    after: u16,
    belt: u8,
    /// Whether `relics` holds the belt under way's slots yet.
    begun: bool,
}

impl<V: Rules> RunListed<V> {
    /// The listener at `at`, which moves past it.
    #[inline(always)]
    fn next_from(&self, at: &mut Cursor) -> Option<RunListener<V>> {
        let deck = self.deck.as_slice();
        if usize::from(at.deck) < deck.len() {
            at.deck += 1;
            return Some(deck[usize::from(at.deck) - 1]);
        }
        let (belts, after) = (self.belts.as_slice(), self.after.as_slice());
        while usize::from(at.belt) < belts.len() {
            let (seat, relics) = belts[usize::from(at.belt)];
            if !at.begun {
                (at.relics, at.begun) = (relics, true);
            }
            if at.relics != 0 {
                let slot = at.relics.trailing_zeros() as usize;
                at.relics &= at.relics - 1;
                return Some(RunListener::Relic { seat, slot });
            }
            match after.get(usize::from(at.after)) {
                Some(&potion @ RunListener::Potion { seat: owner, .. }) if owner == seat => {
                    at.after += 1;
                    return Some(potion);
                }
                _ => (at.belt, at.begun) = (at.belt + 1, false),
            }
        }
        let &listener = after.get(usize::from(at.after))?;
        at.after += 1;
        Some(listener)
    }

    /// Whether nothing listens.
    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.deck.is_empty() && self.belts.is_empty() && self.after.is_empty()
    }

    /// The first listener a walk has not taken, taken off the list.
    fn take_first(&mut self) -> Option<RunListener<V>> {
        let mut at = self.taken;
        let listener = self.next_from(&mut at);
        self.taken = at;
        listener
    }

    /// Each listener, in order.
    pub(super) fn iter(&self) -> impl Iterator<Item = RunListener<V>> + '_ {
        let mut at = Cursor::default();
        std::iter::from_fn(move || self.next_from(&mut at))
    }
}

/// One listener `RunState.IterateHookListeners` lists outside a fight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RunListener<V: Rules> {
    /// The deck card `id` in `seat`'s deck.
    DeckCard {
        seat: sts2_core::SeatIndex,
        id: sts2_core::DeckCardId,
        card: V::Card,
    },
    /// The enchantment on the deck card `id` in `seat`'s deck.
    Enchantment {
        seat: sts2_core::SeatIndex,
        id: sts2_core::DeckCardId,
        enchanted: sts2_core::Enchanted<V>,
    },
    /// The relic in slot `slot` of `seat`'s belt.
    Relic {
        seat: sts2_core::SeatIndex,
        slot: usize,
    },
    Potion {
        seat: sts2_core::SeatIndex,
        potion: V::Potion,
    },
    /// The run's modifier at `at`.
    Modifier {
        at: usize,
        modifier: sts2_core::RunModifier<V>,
    },
}

impl<V: Rules> RunListener<V> {
    /// The player whose model this is. A modifier is the run's.
    pub(super) fn seat(&self) -> Option<sts2_core::SeatIndex> {
        match *self {
            RunListener::DeckCard { seat, .. }
            | RunListener::Enchantment { seat, .. }
            | RunListener::Relic { seat, .. }
            | RunListener::Potion { seat, .. } => Some(seat),
            RunListener::Modifier { .. } => None,
        }
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
pub(crate) const BELT: usize = 10;

/// How many map points one act's run stands on. An act is shorter than this.
const VISITED: usize = 32;

/// The health a `Combat`-layout event's monsters roll as it opens, in the
/// order they stand.
pub(super) type Layout<V> = SmallVec<[<V as sts2_core::ContentVersion>::Health; 2]>;

/// Exactly one room or pending action can own the run at a time.
#[derive(Debug, Clone)]
pub(super) struct EnteredEvent<V: Rules> {
    pub event: V::Event,
    pub seats: crate::seats::BySeat<EventSeat<V>>,
    pub streams: Streams<V>,
    pub events_used: usize,
    pub unknown_odds: [f32; 4],
}

/// The storage of a fight that is over. A reused node writes its next fight
/// into it, and allocates none. A copy of the run holds none.
pub(super) struct Spent<V: Rules>(Option<Box<crate::combat::Live<V>>>);

impl<V: Rules> Clone for Spent<V> {
    fn clone(&self) -> Self {
        Spent(None)
    }
}

impl<V: Rules> std::fmt::Debug for Spent<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Spent")
    }
}

/// One player's Neow. Each player meets Neow alone, off a stream of their
/// own.
#[derive(Debug, Clone)]
pub(super) struct NeowSeat<V: Rules> {
    options: SmallVec<[V::Relic; 4]>,
    chosen: Option<nonmax::NonMaxU8>,
    /// `ModifierOptions`: the run's modifiers whose option Neow has not
    /// offered and run yet, by their place among the run's. The first is
    /// the one Neow shows. A run with modifiers is offered no blessing.
    modifier_options: SmallVec<[u8; 4]>,
    asking: Option<Asked<V>>,
}

impl<V: Rules> NeowSeat<V> {
    /// Whether this player is done with Neow: a blessing chosen and
    /// answered for, and every modifier's option run.
    fn finished(&self) -> bool {
        (self.chosen.is_some() || self.options.is_empty())
            && self.modifier_options.is_empty()
            && self.asking.is_none()
    }
}

/// One player's screen after a won fight.
#[derive(Debug, Clone)]
pub(super) struct RewardSeat<V: Rules> {
    /// The fight as this player saw it end, which stays in the observation
    /// until the run leaves the room.
    combat: Combat<V>,
    offered: Vec<Offer<V>>,
    /// The card removal taken and waiting on the card to remove, by its
    /// place on the screen.
    removing: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) enum Phase<V: Rules> {
    /// Each player's Neow, by seat.
    Neow(crate::seats::BySeat<NeowSeat<V>>),
    /// The map itself, which an act opens on and no room owns.
    Map,
    /// Boxed, so a run moves a pointer rather than a fight. A reused node
    /// writes into the box it already has.
    Combat(Box<crate::combat::Live<V>>),
    /// A fight that is over, and each player's rewards it has not handed
    /// out yet, by seat.
    Rewards {
        seats: crate::seats::BySeat<RewardSeat<V>>,
        spent: Spent<V>,
    },
    Event {
        seats: crate::seats::BySeat<EventSeat<V>>,
    },
    /// A treasure room: `TreasureRoomRelicSynchronizer`'s relics and each
    /// player's vote.
    Treasure {
        /// `CurrentRelics`: one for each player whose `ShouldGenerateTreasure`
        /// holds, while the picking is open. Empty once the relics are
        /// awarded, and for a chest that holds none.
        relics: Vec<V::Relic>,
        /// Each player's vote: a relic's index, or nothing for a skip. No
        /// vote until they answer.
        votes: crate::seats::BySeat<Option<Option<u8>>>,
    },
    /// A rest site, and how far through its one option the run is.
    ///
    /// The options empty when the chosen one resolves. A rest site offers one
    /// choice, and the game stops offering any once it is taken. That is what
    /// an observation reports, rather than a flag beside them.
    RestSite {
        seats: crate::seats::BySeat<RestSeat<V>>,
    },
    /// The fight the run died in, as it stood when the player fell.
    ///
    /// `ProcessPendingLoss` ends the combat and the run with it. The room is
    /// still the one it was fought in, so the fight is still reported. Nothing
    /// is offered, because a dead run takes nothing.
    Lost {
        /// The fight as each player saw it end, by seat.
        combats: crate::seats::BySeat<Combat<V>>,
        spent: Spent<V>,
    },
    /// The run won: The Architect's `PROCEED`, which is `RunManager.WinRun`.
    /// `WinRun` kills every player, which leaves nothing to decide.
    ///
    /// The room stays The Architect's, with the option that won marked
    /// chosen, as `EventOption.WasChosen` reports it.
    Won { room: Option<Room<V>> },
    /// A merchant, and how far through each player's shelves they have
    /// bought.
    ///
    /// `MerchantRoom.Inventories`: one inventory for each player, stocked in
    /// seat order when the run walks in. Each player sees and buys from
    /// their own, and answers their own purchase's questions.
    Shop {
        seats: crate::seats::BySeat<ShopSeat<V>>,
    },
}

/// One player's side of a merchant.
#[derive(Debug, Clone)]
pub(super) struct ShopSeat<V: Rules> {
    /// The player's inventory. A slot bought stops being stocked rather than
    /// leaving the list, because the index the next decision names is the
    /// slot's own.
    entries: Vec<sts2_core::Entry<V>>,
    /// What a relic the player bought here asks as it is picked up.
    asking: Option<Asked<V>>,
}

/// One player's rest site. `RestSiteSynchronizer.BeginRestSite` generates
/// options for each player, and each chooses their own.
#[derive(Debug, Clone)]
pub(super) struct RestSeat<V: Rules> {
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
}

impl<V: Rules> RestSeat<V> {
    /// A rest site offering `options`, with nothing chosen yet.
    fn offering(options: Vec<V::RestSiteOption>) -> Self {
        RestSeat {
            options,
            chosen: None,
            rested: false,
            upgrading: false,
            asking: None,
        }
    }
}

/// One player's copy of an event. `EventSynchronizer.BeginEvent` begins one
/// for each player, and each chooses their own options.
#[derive(Debug, Clone)]
pub(super) struct EventSeat<V: Rules> {
    state: V::EventState,
    /// `EventModel.Rng`, seeded with the player's slot.
    rng: MegaRng,
    /// What an option's effects asked, which the room parks on.
    asking: Option<Asked<V>>,
    /// `EventModel.IsFinished`, which the option that ends the event sets
    /// last.
    finished: bool,
    /// The health a `Combat`-layout event's monsters rolled as it opened.
    /// Empty for any other event.
    layout: Layout<V>,
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
    /// `DoLocalMerchantCardRemoval`: one card of the deck less its `Eternal`
    /// ones, curses first, which the merchant takes out. A `free` one is Lord's
    /// Parasol's, which spends nothing.
    CardRemoval { free: bool },
}

/// A pickup's question, and the pickup standing still on it, which goes on
/// once the question is answered.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Asked<V: Rules> {
    pub(super) pickup: Pickup<V>,
    pub(super) rest: Box<pickup::pickup::State<V>>,
}

/// What an unknown room rolls besides an event, in the order
/// `UnknownMapPointOdds` walks its odds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnknownRoom {
    Monster,
    Elite,
    Treasure,
    Shop,
}

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
    arriving: Vec<(sts2_core::SeatIndex, sts2_core::HookEffect<V>)>,
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

impl<V: Rules> Phase<V> {
    /// Whether `seat`'s player can leave the room this phase stands in.
    ///
    /// A question rather than a flag, because two of the three rooms that
    /// finish already say so in what they hold. Neow has chosen a blessing and
    /// answered for it. A rest site has spent its options. Only the chest
    /// needs a bit of its own, because it keeps the relic it offered either
    /// way.
    fn finished(&self, seat: sts2_core::SeatIndex) -> bool {
        match self {
            Phase::Neow(seats) => seats[seat].finished(),
            // `voteReceived`, or a picking already over.
            Phase::Treasure { relics, votes } => votes[seat].is_some() || relics.is_empty(),
            Phase::RestSite { seats } => seats[seat].options.is_empty(),
            // A merchant never runs out. The run leaves when it decides to,
            // and a shelf it has not bought is still there.
            Phase::Shop { seats } => seats[seat].asking.is_none(),
            // The map is not a room, so there is nothing to leave.
            Phase::Map
            | Phase::Combat(_)
            | Phase::Rewards { .. }
            | Phase::Lost { .. }
            | Phase::Won { .. }
            | Phase::Event { .. } => false,
        }
    }

    /// The question a pickup or an event option parked `seat`'s room on.
    fn asking(&self, seat: sts2_core::SeatIndex) -> Option<&Asked<V>> {
        match self {
            Phase::Neow(seats) => seats[seat].asking.as_ref(),
            Phase::Shop { seats } => seats[seat].asking.as_ref(),
            Phase::Event { seats } => seats[seat].asking.as_ref(),
            Phase::RestSite { seats } => seats[seat].asking.as_ref(),
            _ => None,
        }
    }

    /// Where the rooms that park keep what `seat`'s room is parked on.
    fn asking_mut(&mut self, seat: sts2_core::SeatIndex) -> Option<&mut Option<Asked<V>>> {
        match self {
            Phase::Neow(seats) => Some(&mut seats[seat].asking),
            Phase::Shop { seats } => Some(&mut seats[seat].asking),
            Phase::Event { seats } => Some(&mut seats[seat].asking),
            Phase::RestSite { seats } => Some(&mut seats[seat].asking),
            _ => None,
        }
    }
}

impl<V: Rules> State<V> {
    /// The question that blocks every decision except its answer.
    pub(crate) fn pending(&self, seat: sts2_core::SeatIndex) -> Option<sts2_core::QuestionKind> {
        match &self.phase {
            // The kind alone, without building the options the question lists.
            Phase::Combat(live) => live
                .selecting(seat)
                .map(|_| sts2_core::QuestionKind::SelectCards),
            Phase::Neow { .. } | Phase::Event { .. } | Phase::Shop { .. } => {
                self.phase.asking(seat).map(|asked| match asked.pickup {
                    Pickup::Rewards { .. } => sts2_core::QuestionKind::Rewards,
                    Pickup::Bundles { .. } => sts2_core::QuestionKind::ChooseBundle,
                    Pickup::ConfirmAbandon => sts2_core::QuestionKind::Confirm,
                    Pickup::Event => self.event_question(seat).kind(),
                    _ => sts2_core::QuestionKind::SelectCards,
                })
            }
            Phase::Rewards { seats, .. } if seats[seat].removing.is_some() => {
                Some(sts2_core::QuestionKind::SelectCards)
            }
            Phase::Rewards { seats, .. }
                if seats[seat].offered.iter().any(|offer| !offer.taken()) =>
            {
                Some(sts2_core::QuestionKind::Rewards)
            }
            // Only a heal's rewards park at a rest site.
            Phase::RestSite { seats } if seats[seat].asking.is_some() => {
                Some(sts2_core::QuestionKind::Rewards)
            }
            Phase::RestSite { seats } if seats[seat].upgrading => {
                Some(sts2_core::QuestionKind::SelectCards)
            }
            _ => None,
        }
    }

    /// Whether the decision that just ran finished, or left a question open.
    ///
    /// Reads the phase the way [`State::pending`] does. A command knows this
    /// without a report, which is why it is here rather than derived from one.
    pub(crate) fn decided(&self, seat: sts2_core::SeatIndex) -> Decided {
        match self.pending(seat) {
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
        second_boss: bool,
        multiplayer: bool,
    ) -> ActMap<V> {
        let named = MAP_STREAMS
            .get(index)
            .unwrap_or_else(|| unimplemented!("the map stream a run's act {index} draws from"));
        let mut rng: MegaRng = sts2_core::rng::named::<V, _>(raw_seed, named);
        ActMap::<V>::generate(
            &mut rng,
            layout::<V>(act, multiplayer),
            ascension,
            second_boss,
            None,
        )
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
        let second_boss = self.shared.opening.acts[self.act].1.second_boss.is_some();
        let multiplayer = self.players.len() > 1;
        let mut map = Self::act_map(
            self.shared.raw_seed,
            act,
            self.act,
            self.ascension(),
            second_boss,
            multiplayer,
        );
        // Which class the map is, which a later listener reads.
        #[derive(PartialEq)]
        enum Made {
            Standard,
            Spoils,
            FixedPath,
        }
        let mut made = Made::Standard;
        let hook = sts2_core::EventHook::GeneratedMap { act: self.act };
        for effect in self.notify_run(hook) {
            match effect {
                sts2_core::HookEffect::SpoilsMap => {
                    let mut rng: MegaRng =
                        sts2_core::rng::named::<V, _>(self.shared.raw_seed, "spoils_map");
                    map = ActMap::<V>::spoils(
                        &mut rng,
                        layout::<V>(act, multiplayer),
                        self.ascension(),
                        None,
                    )
                    .expect("a Spoils layout fits a packed map path");
                    made = Made::Spoils;
                }
                sts2_core::HookEffect::FixedPathMap(rooms) => {
                    map = ActMap::fixed_path(rooms);
                    made = Made::FixedPath;
                }
                // `MapPointTypeCounts(map)` with the elites scaled, then the
                // map made again over it. A map of neither class stays.
                sts2_core::HookEffect::ScaledElitesMap(by) => {
                    use sts2_core::MapPointKinds as _;
                    let count = |kind| map.nodes().iter().filter(|node| node.kind == kind).count();
                    let by = num_traits::ToPrimitive::to_f32(&by.exact())
                        .expect("an elite scale fits a float");
                    // `(int)Math.Round((float)count * scale)`, which rounds
                    // a half to even.
                    let elites = (count(V::MapPointType::ELITE) as f32 * by) as f64;
                    let recount = crate::map::Recount {
                        unknowns: count(V::MapPointType::UNKNOWN),
                        rests: count(V::MapPointType::REST_SITE),
                        elites: elites.round_ties_even() as usize,
                    };
                    let layout = layout::<V>(act, multiplayer);
                    map = match made {
                        Made::Standard => {
                            let named = MAP_STREAMS.get(self.act).unwrap_or_else(|| {
                                unimplemented!("the map stream act {} draws from", self.act)
                            });
                            let mut rng: MegaRng =
                                sts2_core::rng::named::<V, _>(self.shared.raw_seed, named);
                            ActMap::<V>::generate(
                                &mut rng,
                                layout,
                                self.ascension(),
                                second_boss,
                                Some(recount),
                            )
                            .expect("an act's layout fits a packed map path")
                        }
                        Made::Spoils => {
                            let mut rng: MegaRng =
                                sts2_core::rng::named::<V, _>(self.shared.raw_seed, "spoils_map");
                            ActMap::<V>::spoils(&mut rng, layout, self.ascension(), Some(recount))
                                .expect("a Spoils layout fits a packed map path")
                        }
                        Made::FixedPath => map,
                    };
                }
                effect => unimplemented!("a listener asking for {effect:?} as a map is generated"),
            }
        }
        // `ModifyGeneratedMapLate`, over what the first pass left, which
        // `Hook.ModifyGeneratedMap` runs as it returns.
        let hook = sts2_core::EventHook::GeneratedMapLate { act: self.act };
        for (owner, effect) in self.notify_run_owned(hook) {
            match effect {
                sts2_core::HookEffect::SpoilsQuest { card, gold } => {
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
                sts2_core::HookEffect::MarkCombatPoints { relic, count, name } => {
                    self.mark_combat_points(owner, &map, relic, count, name);
                }
                effect => unimplemented!("a relic asking for {effect:?} late in a map's making"),
            }
        }
        let hook = sts2_core::EventHook::MapGenerated { act: self.act };
        if let Some(effect) = self.notify_run(hook).first() {
            unimplemented!("a listener asking for {effect:?} once a map is generated");
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
        seat: sts2_core::SeatIndex,
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
        let held = (self.belts[seat].iter_mut())
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

    /// A new run of `players`, in seat order, at `ascension` with
    /// `modifiers`.
    ///
    /// # Panics
    ///
    /// Modifiers in a run of several players, which the co-op lobby sets
    /// none of.
    pub(crate) fn start(
        seed: &Seed,
        players: &[sts2_core::Player<V>],
        ascension: V::AscensionLevel,
        modifiers: &[sts2_core::RunModifier<V>],
    ) -> Self {
        if players.len() > 1 && !modifiers.is_empty() {
            unimplemented!("a run of {} players started with modifiers", players.len());
        }
        let raw_seed = stream_seed::<V>(seed);
        let acts = super::setup::run_acts::<V>(raw_seed);
        let mut streams = Streams::<V>::opening(raw_seed, players.len());
        let characters: SmallVec<[V::Character; 1]> = players.iter().map(|p| p.character).collect();
        let opening = Opening::draw(
            streams.run(V::RunRngType::UP_FRONT),
            &characters,
            &acts,
            ascension,
        );

        let second_boss = opening.acts[0].1.second_boss.is_some();
        let multiplayer = players.len() > 1;
        let map = Self::act_map(raw_seed, acts[0], 0, ascension, second_boss, multiplayer);
        let at = map
            .ancient()
            .expect("a generated act map opens somewhere")
            .at;

        // `Neow.GenerateInitialOptions`: a run with modifiers is offered what
        // they put in front of it instead, and no ported modifier offers
        // anything. Nothing offered leaves the event finished.
        // Each player has a Neow of their own, off a stream their seat is
        // mixed into.
        let neow_options = |seat: u64| match modifiers.is_empty() {
            true => {
                let mut neow_rng: MegaRng = Blessing::<V>::stream(raw_seed, seat);
                Blessing::<V>::roll(&mut neow_rng, players.len()).options
            }
            false => Default::default(),
        };

        // `ModifierModel.OnRunCreated`, once the bags are filled. The deck it
        // clears is cleared as the deck is built, below.
        let mut unknown_base = UNKNOWN_ODDS;
        let mut unknown_odds = UNKNOWN_ODDS;
        let mut out_of_bags = Vec::new();
        let mut modifier_states: Vec<V::ModifierState> = modifiers
            .iter()
            .map(|entry| sts2_core::ModifierModel::new(entry.modifier))
            .collect();
        for (&entry, state) in modifiers.iter().zip(&mut modifier_states) {
            let modifier = entry.modifier;
            let mut asked = sts2_core::HookEffects::new();
            V::modifier_event_hook(entry, state, sts2_core::EventHook::RunCreated, &mut asked);
            for effect in asked {
                match effect {
                    sts2_core::HookEffect::RemoveFromRelicBags(relic) => out_of_bags.push(relic),
                    sts2_core::HookEffect::SetUnknownRoomOdds { room, odds, base } => {
                        use sts2_core::RoomKinds as _;
                        let at = [
                            V::RoomType::MONSTER,
                            V::RoomType::ELITE,
                            V::RoomType::TREASURE,
                            V::RoomType::SHOP,
                        ]
                        .iter()
                        .position(|&kind| kind == room)
                        .unwrap_or_else(|| unimplemented!("unknown-room odds for {room:?}"));
                        let float = |fixed: sts2_core::Fixed| {
                            num_traits::ToPrimitive::to_f32(&fixed.exact())
                                .expect("odds fit a float")
                        };
                        unknown_odds[at] = float(odds);
                        unknown_base[at] = float(base);
                    }
                    effect => {
                        unimplemented!("{modifier:?} asking for {effect:?} as the run is created")
                    }
                }
            }
        }

        let player = |&sts2_core::Player { character, wins }: &sts2_core::Player<V>| {
            let mut player = Player {
                character,
                wins,
                potions: Bounded::new(),
                potion_positions: Bounded::new(),
                deck: Arc::new(Vec::new()),
                minted: sts2_core::DeckCardId::new(0),
                // Neow's `BeforeEventStarted` sets it to nothing and heals what
                // is missing, which a weary traveller gets four fifths of.
                hp: ancient_heal::<V>(ascension, num_traits::zero(), V::starting_hp(character)),
                max_hp: V::starting_hp(character),
                gold: num_traits::cast(V::starting_gold(character))
                    .expect("starting gold fits the build's game-number type"),
                potion_slots: num_traits::cast(V::starting_potion_slots())
                    .expect("starting potion slots fit the build's count type"),
                deaf: false,
                picked: sts2_core::Picks::none(),
                odds: Odds::default(),
                dealt_relics: Arc::new(out_of_bags.clone()),
                last_turn: None,
                removals_used: 0,
                unheld_card_rewards: Arc::new(Vec::new()),
                vote: None,
            };
            // `AscensionManager.ApplyEffectsTo`: a slot fewer, and the curse on
            // the end of the deck.
            if sts2_core::AscensionKinds::has(
                ascension,
                <V::AscensionLevel as sts2_core::AscensionKinds>::TIGHT_BELT,
            ) {
                player.potion_slots = player.potion_slots - num_traits::one();
            }
            // `OnRunCreated` empties the deck for a modifier that clears it, and
            // the ascension's curse goes on after.
            let cleared = modifiers
                .iter()
                .any(|entry| sts2_core::ModifierTable::clears_deck(entry.modifier));
            let starting = match cleared {
                true => Vec::new(),
                false => V::starting_deck(character).into_iter().collect(),
            };
            let deck = starting
                .into_iter()
                .chain(V::ascension_deck(ascension))
                .map(|card| sts2_core::DeckCard::new(player.mint(), CardIdent::new(card)))
                .collect();
            player.deck = Arc::new(deck);
            player
        };
        let mut seated = players.iter().map(player);
        let host = seated.next().expect("a run has a player");
        let others: Vec<Player<V>> = seated.collect();
        let belt = |player: &sts2_core::Player<V>| -> Belt<V> {
            V::starting_relics(player.character)
                .into_iter()
                .map(HeldRelic::new)
                .collect()
        };
        let neow_seat = |seat: usize| NeowSeat {
            options: neow_options(seat as u64).into_iter().collect(),
            chosen: None,
            modifier_options: modifiers
                .iter()
                .enumerate()
                .filter(|(_, entry)| sts2_core::ModifierTable::offers_neow_option(entry.modifier))
                .map(|(at, _)| u8::try_from(at).expect("a run has few modifiers"))
                .collect(),
            asking: None,
        };
        let mut state = Self {
            shared: Arc::new(RunData {
                opening,
                raw_seed,
                ascension,
                unknown_base,
                modifiers: modifiers.to_vec(),
            }),
            modifier_states: Arc::new(modifier_states),
            map: Arc::new(map),
            spoils: None,
            act: 0,
            floor: num_traits::one(),
            visited: [at].into_iter().collect(),
            players: Players::new(host, others),
            map_votes: sts2_core::rng::named::<V, _>(raw_seed, "map_point_selection"),
            belts: Belts::new(belt(&players[0]), players[1..].iter().map(belt).collect()),
            normal_fights: 0,
            elite_fights: 0,
            // A run opens in Neow's room, which is an event room.
            events_used: 1,
            visited_events: Arc::new(Vec::new()),
            unknown_odds,
            shared_dealt: Arc::new(out_of_bags),
            shared_refills: Arc::new(Vec::new()),
            streams,
            resuming: None,
            phase: Phase::Neow(crate::seats::BySeat::new(
                neow_seat(0),
                (1..players.len()).map(neow_seat).collect(),
            )),
        };
        // `RunManager.GenerateMap` runs its hooks for the first act too, and
        // a modifier is the only listener a new run holds that hears them.
        let maps = sts2_core::Overrides::of(sts2_core::ModelHook::ModifyGeneratedMap)
            .with(sts2_core::ModelHook::ModifyGeneratedMapLate);
        if modifiers
            .iter()
            .any(|entry| sts2_core::ModifierTable::overrides(entry.modifier).intersects(maps))
        {
            state.map = Arc::new(state.generate_map(acts[0]));
        }
        // `EnterAct` enters Neow's room, which is an event room the map
        // opens, and then raises `AfterActEntered`.
        let point = state.point_type(Location::MapPoint(at));
        state.room_entered(<V::RoomType as sts2_core::RoomKinds>::EVENT, false, point);
        state.act_entered();
        state
    }

    /// `Hook.AfterActEntered`, once the run stands in the act's first room.
    ///
    /// # Panics
    ///
    /// A listener whose answer asks a question.
    fn act_entered(&mut self) {
        if self.walk_run(sts2_core::EventHook::ActEntered).is_some() {
            unimplemented!("a question asked as an act is entered");
        }
    }

    pub(crate) fn raw_seed(&self) -> u64 {
        self.shared.raw_seed
    }

    pub(crate) fn ascension(&self) -> V::AscensionLevel {
        self.shared.ascension
    }

    pub(crate) fn character(&self, seat: sts2_core::SeatIndex) -> V::Character {
        self.players[seat].character
    }

    /// The fight the run is standing in, or nothing outside one.
    pub(super) fn fight(&self) -> Option<&crate::combat::Live<V>> {
        match &self.phase {
            Phase::Combat(live) => Some(live),
            Phase::Map
            | Phase::Neow { .. }
            | Phase::Rewards { .. }
            | Phase::Lost { .. }
            | Phase::Won { .. }
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
    fn rest_option_enabled(&self, seat: sts2_core::SeatIndex, option: V::RestSiteOption) -> bool {
        use sts2_core::RestKinds;

        if option == <V::RestSiteOption as RestKinds<V>>::SMITH {
            return self.upgradable(seat).next().is_some();
        }
        if option == <V::RestSiteOption as RestKinds<V>>::COOK {
            return self.offered(seat, sts2_core::DeckAct::Remove).len() >= COOK_CARDS;
        }
        true
    }

    /// The deck's cards that `CardModel.IsUpgradable` holds, each with where
    /// it sits in the deck.
    ///
    /// A selection to upgrade offers these and nothing else. The index an
    /// answer names is this list's, so the deck's own place has to be carried
    /// alongside to apply it.
    fn upgradable(
        &self,
        seat: sts2_core::SeatIndex,
    ) -> impl Iterator<Item = (usize, CardIdent<V>)> + '_ {
        use sts2_core::CardTable as _;

        self.players[seat]
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
    fn offered(
        &self,
        seat: sts2_core::SeatIndex,
        act: sts2_core::DeckAct<V>,
    ) -> Vec<(usize, CardIdent<V>)> {
        use sts2_core::CardTable as _;

        let deck = self.players[seat]
            .deck
            .iter()
            .map(|held| held.ident)
            .enumerate();
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
            sts2_core::DeckAct::Upgrade => self.upgradable(seat).collect(),
            // `IsTransformable` in the deck is `IsRemovable`, and a quest
            // card is never offered.
            sts2_core::DeckAct::Transform(_)
            | sts2_core::DeckAct::TransformUpgraded(_)
            | sts2_core::DeckAct::TransformTogether(_)
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

    /// `Hook.AfterCombatEnd`, `Hook.AfterCombatVictoryEarly` and
    /// `Hook.AfterCombatVictory`, for the fight the run stands in, which is
    /// won. `RunState.IterateHookListeners` lists the deck's cards, each with
    /// its enchantment, ahead of the fight's listeners. A card that asks to
    /// leave the deck leaves it before the fight's listeners are asked.
    ///
    /// # Panics
    ///
    /// A deck listener hearing either victory hook, or asking for anything
    /// but to leave the deck.
    pub(super) fn combat_ended(&mut self) {
        use sts2_core::{ModelHook, Overrides};

        let Phase::Combat(live) = &self.phase else {
            unreachable!("a won fight is the one the run stands in")
        };
        let hook = sts2_core::EventHook::CombatEnd { room: live.room() };
        let mut asked = sts2_core::HookEffects::new();
        // Each player's deck, in seat order.
        for seat in self.players.seats() {
            for listener in self.deck_listed(seat, Overrides::of(ModelHook::AfterCombatEnd)) {
                if !self.ask_run_listener(listener, hook, &mut asked) {
                    continue;
                }
                for effect in asked.drain(..) {
                    match (listener, effect) {
                        // `CardPileCmd.RemoveFromDeck`.
                        (
                            RunListener::DeckCard { id, .. },
                            sts2_core::HookEffect::RemoveFromDeck,
                        ) => {
                            let at = self.players[seat]
                                .deck
                                .iter()
                                .position(|held| held.id == id);
                            let at = at.expect("a card the deck held as it was asked");
                            self.players[seat].deck_mut().remove(at);
                        }
                        (listener, effect) => {
                            unimplemented!("{listener:?} in the deck asking for {effect:?}")
                        }
                    }
                }
            }
            let victory = Overrides::of(ModelHook::AfterCombatVictoryEarly)
                .union(Overrides::of(ModelHook::AfterCombatVictory));
            if let Some(listener) = self.deck_listed(seat, victory).first() {
                unimplemented!("{listener:?} in the deck hearing a won fight");
            }
        }
        let Phase::Combat(live) = &mut self.phase else {
            unreachable!("a won fight is the one the run stands in")
        };
        crate::combat::combat_ended::<V>(live, &mut self.belts);
    }

    /// `Hook.BeforeRoomEntered`, which `RunManager.EnterRoom` raises once the
    /// room is pushed and before it is entered. `point` is the map point's
    /// type, and `base` is whether the room is the point's first.
    ///
    /// # Panics
    ///
    /// A listener asking for anything but a deck card's own transform.
    pub(super) fn room_entering(
        &mut self,
        seat: sts2_core::SeatIndex,
        point: Option<V::MapPointType>,
        base: bool,
    ) {
        let hook = sts2_core::EventHook::RoomEntering { point, base };
        self.walk_run_landing(hook, |state, listener, effect| match (listener, effect) {
            // `CardCmd.TransformTo`: out of the deck, and the new card at its
            // end.
            (RunListener::DeckCard { id, .. }, sts2_core::HookEffect::TransformTo(into)) => {
                if let Some(at) = state.players[seat]
                    .deck
                    .iter()
                    .position(|held| held.id == id)
                {
                    state.act_on_deck(seat, sts2_core::DeckAct::TransformInto(into), &[at]);
                }
            }
            (listener, effect) => {
                unimplemented!("{listener:?} asking for {effect:?} as a room is entered")
            }
        });
    }

    /// Carry `act` out on the deck cards at `taken`, in the order they were
    /// taken.
    fn act_on_deck(
        &mut self,
        seat: sts2_core::SeatIndex,
        act: sts2_core::DeckAct<V>,
        taken: &[usize],
    ) {
        use sts2_core::rng::GameRng as _;

        match act {
            // Each taken card, in id order, kept on the relic and then
            // `CardPileCmd.RemoveFromDeck`. The relic is the last of its kind
            // on the belt, which is the one just obtained.
            sts2_core::DeckAct::RemoveOntoRelic(relic) => {
                use sts2_core::{CardTable as _, RelicModel as _};

                let mut kept: Vec<sts2_core::DeckCard<V>> = taken
                    .iter()
                    .map(|&at| self.players[seat].deck[at])
                    .collect();
                kept.sort_by_key(|card| card.ident.card.order());
                let mut taken = taken.to_vec();
                taken.sort_unstable_by(|a, b| b.cmp(a));
                for at in taken {
                    self.players[seat].deck_mut().remove(at);
                }
                let slot = (self.belts[seat].iter())
                    .rposition(|held| held.relic == relic)
                    .unwrap_or_else(|| unimplemented!("{relic:?} keeping cards off the belt"));
                self.belts[seat][slot].state.hold_cards(&kept);
            }
            sts2_core::DeckAct::Remove | sts2_core::DeckAct::RemoveBasicTagged(_) => {
                let mut taken = taken.to_vec();
                taken.sort_unstable_by(|a, b| b.cmp(a));
                for at in taken {
                    self.players[seat].deck_mut().remove(at);
                }
            }
            // `CardCmd.Upgrade`, which passes over a card it cannot raise.
            sts2_core::DeckAct::Upgrade => {
                use sts2_core::CardTable as _;

                for &at in taken {
                    let card = &mut self.players[seat].deck_mut()[at].ident;
                    if card.upgrade < card.card.max_upgrade() {
                        card.upgrade += 1;
                    }
                }
            }
            // A replacement drawn for each card in the order the cards were
            // taken. `CardCmd.TransformToRandom` a card at a time lands each at
            // the deck's end in that order. `CardCmd.Transform` over the list
            // sorts them back into their originals' deck order first.
            sts2_core::DeckAct::Transform(stream)
            | sts2_core::DeckAct::TransformUpgraded(stream)
            | sts2_core::DeckAct::TransformTogether(stream) => {
                let upgrade = matches!(act, sts2_core::DeckAct::TransformUpgraded(_));
                let together = matches!(act, sts2_core::DeckAct::TransformTogether(_));
                let rng = stream_of(seat, &mut self.phase, &mut self.streams, stream);
                let mut replaced: Vec<(usize, CardIdent<V>)> = taken
                    .iter()
                    .map(|&at| {
                        let options = V::transform_options(
                            self.players[seat].deck[at].ident.card,
                            false,
                            self.players.len() > 1,
                        );
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
                if together {
                    replaced.sort_by_key(|(at, _)| *at);
                }
                let mut gone: Vec<usize> = replaced.iter().map(|(at, _)| *at).collect();
                gone.sort_unstable_by(|a, b| b.cmp(a));
                for at in gone {
                    self.players[seat].deck_mut().remove(at);
                }
                for (_, card) in replaced {
                    self.add_transformed(seat, card);
                }
            }
            // `CardCmd.Transform` with the replacements made first. The
            // originals leave the deck, and the replacements join its end in
            // the originals' deck order.
            sts2_core::DeckAct::TransformInto(into) => {
                use sts2_core::CardTable as _;

                // `CardCmd.Transform` takes each card out in the order it was
                // picked, noting where it stood once the ones before it were
                // gone. It adds the replacements sorted by that, and a tie
                // keeps the pick order: `List.Sort` on a list this short is
                // an insertion sort.
                let mut order: Vec<(usize, usize)> = taken
                    .iter()
                    .enumerate()
                    .map(|(picked, &at)| {
                        let before = taken[..picked].iter().filter(|&&gone| gone < at).count();
                        (at - before, at)
                    })
                    .collect();
                order.sort_by_key(|&(stood, _)| stood);
                let replaced: Vec<CardIdent<V>> = order
                    .iter()
                    .map(|&(_, at)| {
                        let original = self.players[seat].deck[at].ident;
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
                let mut gone = taken.to_vec();
                gone.sort_unstable();
                for &at in gone.iter().rev() {
                    self.players[seat].deck_mut().remove(at);
                }
                for card in replaced {
                    self.add_transformed(seat, card);
                }
            }
            // `CardCmd.TransformTo`, which lands the named card where
            // `CardCmd.Transform` lands a drawn one.
            sts2_core::DeckAct::TransformBasicInto(into) => {
                let mut gone = taken.to_vec();
                gone.sort_unstable_by(|a, b| b.cmp(a));
                for &at in &gone {
                    self.players[seat].deck_mut().remove(at);
                }
                for _ in &gone {
                    self.add_transformed(seat, CardIdent::new(into));
                }
            }
            sts2_core::DeckAct::Enchant {
                enchantment,
                amount,
                ..
            } => {
                for &at in taken {
                    V::enchant(
                        enchantment,
                        amount,
                        &mut self.players[seat].deck_mut()[at].ident,
                    );
                }
            }
        }
    }

    /// Take the potion at this index out of the belt, which is
    /// `RemoveBeforeUse`.
    pub(super) fn drank(&mut self, seat: sts2_core::SeatIndex, at: usize) {
        self.players[seat].potions.remove(at);
        self.players[seat].potion_positions.remove(at);
        self.belt_to_fight(seat);
    }

    /// Whether the card selection parked here has a back button, which its
    /// `CardSelectorPrefs.Cancelable` puts there. Smith, Cook, the shop's
    /// paid card removal and the card-removal reward each set it. Lord's
    /// Parasol's free removal does not.
    fn cancelable(&self, seat: sts2_core::SeatIndex) -> bool {
        match &self.phase {
            Phase::RestSite { seats } if seats[seat].upgrading => true,
            // Cook, which is the one rest site option that removes cards.
            Phase::RestSite { seats } => {
                let RestSeat { asking, chosen, .. } = &seats[seat];
                let (Some(asked), Some(option)) = (asking, chosen) else {
                    return false;
                };
                *option == <V::RestSiteOption as sts2_core::RestKinds<V>>::COOK
                    && matches!(
                        asked.pickup,
                        Pickup::FromDeck {
                            act: sts2_core::DeckAct::Remove,
                            ..
                        }
                    )
            }
            Phase::Shop { seats } => seats[seat]
                .asking
                .as_ref()
                .is_some_and(|asked| asked.pickup == Pickup::CardRemoval { free: false }),
            Phase::Rewards { seats, .. } => seats[seat].removing.is_some(),
            _ => false,
        }
    }

    /// The back button on a cancelable card selection: the game gets no
    /// cards, and what opened the selection is left undone. Smith and Cook
    /// return false from `OnSelect`, so the option stays and the rest site
    /// is as it was. The shop's removal is not bought, and the removal
    /// reward is not taken.
    fn cancel(&mut self, seat: sts2_core::SeatIndex) -> Settled<V> {
        if !self.cancelable(seat) {
            return Err(sts2_core::Refusal {
                what: "answer".into(),
                reason: sts2_core::Reason::Malformed {
                    message: "this selection cannot be cancelled".into(),
                },
            });
        }
        self.players[seat].picked = sts2_core::Picks::none();
        match &mut self.phase {
            Phase::RestSite { seats } => {
                let rest = &mut seats[seat];
                rest.upgrading = false;
                rest.asking = None;
                rest.chosen = None;
            }
            Phase::Shop { seats } => seats[seat].asking = None,
            Phase::Rewards { seats, .. } => seats[seat].removing = None,
            _ => unreachable!("only a cancelable selection is cancelled"),
        }
        Ok(self.decided(seat))
    }

    /// `DiscardPotionGameAction`, which the potion's popup enqueues:
    /// `PotionCmd.Discard`, so the potion leaves the belt and
    /// `AfterPotionDiscarded` is raised, in a fight or out.
    pub(crate) fn discard_potion(
        &mut self,
        seat: sts2_core::SeatIndex,
        index: sts2_core::BeltIndex,
    ) -> Result<super::Settled<V>, Error> {
        let at = usize::from(index.get());
        if at >= self.players[seat].potions.len() {
            return Ok(Err(sts2_core::Refusal {
                what: "discard_potion".into(),
                reason: sts2_core::Reason::OutOfRange {
                    index: i64::from(index.get()),
                    count: self.players[seat].potions.len(),
                },
            }));
        }
        self.drank(seat, at);
        self.belt_changed(
            seat,
            sts2_core::EventHook::PotionDiscarded {
                in_fight: matches!(self.phase, Phase::Combat(_)),
                holding_potions: self.holding_potions(seat),
            },
        );
        Ok(Ok(Decided::Resolved))
    }

    /// `Owner.Potions.Any()`.
    pub(super) fn holding_potions(&self, seat: sts2_core::SeatIndex) -> bool {
        !self.players[seat].potions.is_empty()
    }

    /// The belt's count, onto the fight the run stands in.
    fn belt_to_fight(&mut self, seat: sts2_core::SeatIndex) {
        if let Phase::Combat(live) = &mut self.phase {
            live.hold_potions(seat, &self.players[seat].potions);
        }
    }

    /// `PotionCmd.TryToProcure` once the potion is in: `AfterPotionProcured`.
    ///
    /// # Panics
    ///
    /// A relic asking for anything outside a fight.
    fn procured(&mut self, seat: sts2_core::SeatIndex) {
        self.belt_to_fight(seat);
        let hook = sts2_core::EventHook::PotionProcured {
            in_fight: matches!(self.phase, Phase::Combat(_)),
        };
        self.belt_changed(seat, hook);
    }

    /// `hook`, for a change to the belt, raised on the fight the run stands
    /// in or else on the belt.
    ///
    /// # Panics
    ///
    /// A relic asking for anything outside a fight.
    fn belt_changed(&mut self, seat: sts2_core::SeatIndex, hook: sts2_core::EventHook<V>) {
        if let Some(fighting) = self.fighting() {
            crate::combat::belt_changed(seat, fighting, hook);
            return;
        }
        if let Some(effect) = self.notify_run(hook).first() {
            unimplemented!("a relic asking for {effect:?} on {hook:?} outside a fight");
        }
    }

    /// `PotionCmd.TryToProcure` into the slot at `slot`, which lands while
    /// that slot is open and no relic refuses it.
    fn procure_into(&mut self, seat: sts2_core::SeatIndex, potion: V::Potion, slot: usize) -> bool {
        let slot = u8::try_from(slot).expect("a belt slot fits a byte");
        if self.procure_refused(seat, potion) || self.players[seat].potion_positions.contains(&slot)
        {
            return false;
        }
        let at = self.players[seat]
            .potion_positions
            .partition_point(|held| *held < slot);
        self.players[seat].potion_positions.insert(at, slot);
        self.players[seat].potions.insert(at, potion);
        self.procured(seat);
        true
    }

    /// `PotionCmd.TryToProcure`: the potion in the first empty physical belt
    /// slot, unless a relic refuses it.
    fn procure(&mut self, seat: sts2_core::SeatIndex, potion: V::Potion) -> bool {
        if self.procure_refused(seat, potion) {
            return false;
        }
        let slots: usize = num_traits::cast(self.players[seat].potion_slots)
            .expect("the belt has an addressable number of slots");
        let Some(slot) = (0..slots)
            .map(|slot| u8::try_from(slot).expect("a belt slot fits a byte"))
            .find(|slot| !self.players[seat].potion_positions.contains(slot))
        else {
            return false;
        };
        let at = self.players[seat]
            .potion_positions
            .partition_point(|held| *held < slot);
        self.players[seat].potion_positions.insert(at, slot);
        self.players[seat].potions.insert(at, potion);
        self.procured(seat);
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
            belts: &mut self.belts,
        })
    }

    /// Take the fight out of the run, for a decision that ended it, and lend
    /// the belt its report reads.
    ///
    /// What replaces it is the caller's to say, straight after.
    pub(super) fn left_fight(&mut self) -> (Box<crate::combat::Live<V>>, &Belts<V>) {
        match std::mem::replace(&mut self.phase, Phase::Map) {
            Phase::Combat(live) => (live, &self.belts),
            _ => unreachable!("a fight that ended is the one the run stood in"),
        }
    }

    /// Remove from the deck the copies the fight took for good, from `since`
    /// on in the fight's own list of them.
    ///
    /// The fight keeps every copy it took, because a won fight offers each one
    /// back. So a decision passes the length the list had before it began, and
    /// only what it added leaves the deck.
    pub(super) fn lose_cards(&mut self, seat: sts2_core::SeatIndex, since: usize) {
        let Phase::Combat(live) = &mut self.phase else {
            return;
        };
        for next in since..live.removed_from_deck(seat).len() {
            let id = live.removed_from_deck(seat)[next];
            let Some(at) = self.players[seat]
                .deck
                .iter()
                .position(|held| held.id == id)
            else {
                continue;
            };
            let card = self.players[seat].deck_mut().remove(at);
            live.take_from_deck(seat, card);
        }
    }

    /// What the fight asked of the run during a decision, carried out.
    ///
    /// # Panics
    ///
    /// The powers taken off during the decision, which no walk asks once
    /// it is over.
    pub(super) fn forget_detached(&mut self) {
        if let Phase::Combat(live) = &mut self.phase {
            live.forget_detached();
        }
    }

    /// Something that asks the run a question.
    pub(super) fn asked_by_fight(&mut self, seat: sts2_core::SeatIndex) {
        let Phase::Combat(live) = &mut self.phase else {
            return;
        };
        if !live.asked_for_run() {
            return;
        }
        let asked = live.take_for_run();
        if self.carry_out(seat, asked).is_some() {
            unimplemented!("a fight asking the run a question mid-decision");
        }
    }

    /// The player's gold, back from the fight that held it.
    pub(super) fn gold_from_fight(&mut self, seat: sts2_core::SeatIndex) {
        if let Phase::Combat(live) = &self.phase {
            self.players[seat].gold = live.gold(seat);
        }
    }

    /// The potions the fight spent out of the belt, which `RemoveBeforeUse`
    /// took in the order the fight spent them.
    pub(super) fn potions_from_fight(&mut self, seat: sts2_core::SeatIndex) {
        let Phase::Combat(live) = &mut self.phase else {
            return;
        };
        for &at in &live.take_spent_potions(seat) {
            self.players[seat].potions.remove(usize::from(at));
            self.players[seat].potion_positions.remove(usize::from(at));
        }
    }

    /// Park the run on the rewards a won fight left each player, in seat
    /// order.
    ///
    /// What the fight did to each player comes out with it. The creature was
    /// built from the run's own numbers and is where they stood while the
    /// fight lasted.
    /// `spent` is the fight's own storage.
    pub(super) fn won(
        &mut self,
        screens: Vec<(Combat<V>, Vec<Offer<V>>)>,
        spent: Box<crate::combat::Live<V>>,
    ) {
        let mut seats = Vec::with_capacity(screens.len());
        for (seat, (combat, offered)) in self.players.seats().zip(screens) {
            self.players[seat].last_turn = Some(combat.turn);
            if let Some(player) = combat.creatures.iter().find(|c| c.id == seat.creature()) {
                self.players[seat].hp = player.hp;
                self.players[seat].max_hp = player.max_hp;
            }
            seats.push(RewardSeat {
                combat,
                offered,
                removing: None,
            });
        }
        let mut seats = seats.into_iter();
        let host = seats.next().expect("a run has a player");
        self.phase = Phase::Rewards {
            seats: crate::seats::BySeat::new(host, seats.collect()),
            spent: Spent(Some(spent)),
        };
    }

    /// Park the run on the fight it died in.
    ///
    /// The creature was built from the run's own numbers, and where it fell is
    /// where they stayed. Nothing writes them back, because a run that is over
    /// has nowhere to carry them to.
    ///
    /// `spent` is the fight's own storage, where there is one.
    pub(super) fn lost(
        &mut self,
        combats: Vec<Combat<V>>,
        spent: Option<Box<crate::combat::Live<V>>>,
    ) {
        for (seat, combat) in self.players.seats().zip(&combats) {
            self.players[seat].last_turn = Some(combat.turn);
        }
        let mut combats = combats.into_iter();
        let host = combats.next().expect("a run has a player");
        self.phase = Phase::Lost {
            combats: crate::seats::BySeat::new(host, combats.collect()),
            spent: Spent(spent),
        };
    }

    /// The stream a reward draws from, and the pity counters it moves.
    ///
    /// Together, because a roll reads and writes both, and they are two fields
    /// of the same run.
    pub(super) fn rewarding(&mut self, seat: sts2_core::SeatIndex) -> (&mut MegaRng, &mut Odds) {
        (
            self.streams.player(seat, V::PlayerRngType::REWARDS),
            &mut self.players[seat].odds,
        )
    }

    /// `RelicFactory.PullNextRelicFromFront` at `rarity`, through the shop
    /// filter for a stocked pull. [`pulled_rarity`] is the rarity it deals at.
    ///
    /// `floor` is `TotalFloor`, which `IsAllowed` reads. An event's room
    /// counts already as its options pull, while the run's own floor moves
    /// only once it enters.
    pub(super) fn pull_relic_as(
        &mut self,
        seat: sts2_core::SeatIndex,
        pull: sts2_core::RelicPull<V>,
        rarity: Rarity<V>,
        floor: V::Count,
    ) -> V::Relic {
        use sts2_core::RelicTable as _;

        let floor = num_traits::cast(floor).expect("a floor fits a usize");
        let relic = match pull {
            sts2_core::RelicPull::Rolled => self.shared.opening.player_relics[seat.at()].front(
                rarity,
                &self.players[seat].dealt_relics,
                floor,
                self.players.len(),
            ),
            sts2_core::RelicPull::Stocked(_) => self.shared.opening.player_relics[seat.at()]
                .front_where(
                    rarity,
                    &self.players[seat].dealt_relics,
                    floor,
                    self.players.len(),
                    V::Relic::allowed_in_shops,
                ),
        };
        self.out_of_both_bags(seat, relic);
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
    pub(super) fn pull_relic(&mut self, seat: sts2_core::SeatIndex, rarity: Rarity<V>) -> V::Relic {
        let relic = self.shared.opening.player_relics[seat.at()].front(
            rarity,
            &self.players[seat].dealt_relics,
            num_traits::cast(self.floor).expect("a floor fits a usize"),
            self.players.len(),
        );
        self.out_of_both_bags(seat, relic);
        relic
    }

    /// `SharedRelicGrabBag.PullFromFront`: the next relic in the shared bag,
    /// dealt. What left either bag has left this one.
    ///
    /// The shared bag refreshes. A rarity with nothing left is laid back in
    /// the order the bag was filled, then dealt from. A rarity above it is
    /// walked to only where that leaves nothing too.
    ///
    /// # Panics
    ///
    /// A bag with nothing left at the rarity or above, where the game falls
    /// back to a relic of its own.
    pub(super) fn deal_shared_relic(&mut self, rarity: Rarity<V>) -> V::Relic {
        let mut relic = self.shared_front_at(rarity);
        if relic.is_none() {
            let mark = (rarity, self.shared_dealt.len());
            Arc::make_mut(&mut self.shared_refills).push(mark);
            relic = self.shared_front_at(rarity);
        }
        let mut at = rarity;
        while relic.is_none() {
            at = super::setup::rarity_above::<V>(at)
                .unwrap_or_else(|| unimplemented!("a shared relic bag with nothing left"));
            relic = self.shared_front_at(at);
        }
        let relic = relic.expect("the walk stops on a relic");
        Arc::make_mut(&mut self.shared_dealt).push(relic);
        relic
    }

    /// The first relic the shared bag holds at `rarity` alone, as its deque
    /// stands since its last refill, if it had one.
    fn shared_front_at(&self, rarity: Rarity<V>) -> Option<V::Relic> {
        let floor = num_traits::cast(self.floor).expect("a floor fits a usize");
        let refill = self
            .shared_refills
            .iter()
            .rev()
            .find(|mark| mark.0 == rarity);
        let left = &self.shared_dealt[refill.map_or(0, |&(_, since)| since)..];
        self.shared.opening.shared_relics.first_at(
            rarity,
            refill.is_some(),
            floor,
            self.players.len(),
            |relic| left.contains(&relic),
        )
    }

    /// `relic` taken out of `seat`'s bag and the shared one, so neither deals
    /// it.
    fn out_of_both_bags(&mut self, seat: sts2_core::SeatIndex, relic: V::Relic) {
        Arc::make_mut(&mut self.players[seat].dealt_relics).push(relic);
        Arc::make_mut(&mut self.shared_dealt).push(relic);
    }

    /// `Player.CanRemovePotions` off: an event that holds the potions while it
    /// runs, and has not finished.
    pub(super) fn potions_held(&self, seat: sts2_core::SeatIndex) -> bool {
        match &self.phase {
            Phase::Event { seats } if !seats[seat].finished => {
                V::event_holds_potions(&seats[seat].state)
            }
            _ => false,
        }
    }

    /// Which act the run stands in, counted from zero.
    pub(super) fn act(&self) -> usize {
        self.act
    }

    /// Whether the run stands on a boss point that leads on to another, which
    /// is `SecondBossMapPoint != null && CurrentMapCoord == BossMapPoint.coord`.
    fn boss_ahead(&self) -> bool {
        use sts2_core::{ActMap as _, MapPointKinds as _};

        self.visited
            .last()
            .and_then(|&at| self.map.at(at).ok())
            .is_some_and(|here| here.kind == V::MapPointType::BOSS && !here.to.is_empty())
    }

    /// `effects` from an event resuming after its fight, where the one at
    /// `at` offers its rewards and then the fight's `extras`. `OfferCustom`
    /// fills them in that order, so the extras draw after the event's own.
    ///
    /// # Panics
    ///
    /// An effect before the offer asking a question.
    fn offer_with_extras(
        &mut self,
        seat: sts2_core::SeatIndex,
        effects: &[sts2_core::HookEffect<V>],
        at: usize,
        extras: &super::fight::Extras<V>,
    ) -> Decided {
        let (before, after) = effects.split_at(at);
        if let Some(asked) = self.carry_out(seat, before.iter().copied()) {
            let pickup = asked.pickup;
            unimplemented!("an event asking for {pickup:?} before it offers its rewards");
        }
        let sts2_core::HookEffect::OfferRewardsWithRoomExtras(rewards) = after[0] else {
            unreachable!("the offer stands at {at}")
        };
        let rest: Steps<V> = after[1..].iter().map(|&effect| effect.into()).collect();
        if self.players[seat].hp == num_traits::zero() {
            return match self.carry_out(seat, rest) {
                None => Decided::Resolved,
                Some(asked) => self.park(seat, asked),
            };
        }
        let stream = sts2_core::Stream::Player(V::PlayerRngType::REWARDS);
        let mut offered = self.filled_rewards(seat, rewards, stream);
        offered.extend(self.extra_offers(seat, extras));
        self.modify_rewards(seat, &mut offered, None);
        crate::reward::sort(&mut offered);
        let screen = Then::Ask(Pickup::Rewards {
            offered,
            skippable: true,
        });
        match self.carry_out(seat, std::iter::once(screen).chain(rest)) {
            None => Decided::Resolved,
            Some(asked) => self.park(seat, asked),
        }
    }

    /// `CurrentActIndex >= Acts.Count - 1`: whether the run stands in its
    /// last act.
    pub(super) fn in_last_act(&self) -> bool {
        self.act + 1 >= self.shared.opening.acts.len()
    }

    /// `RunManager.EnterNextAct`, which the finished act's boss room offers.
    /// Past the last act, a run in its victory room wins, and a run anywhere
    /// else enters The Architect, whose room that is.
    ///
    /// `SetActInternal` moves the index on, forgets the last act's points,
    /// resets the question mark odds and generates the new map.
    ///
    /// After a fight an event entered to come back to, the event resumes:
    /// `ProceedFromTerminalRewardsScreen` is `ResumePreviousRoom` there.
    pub(crate) fn proceed(&mut self, seat: sts2_core::SeatIndex) -> Result<Decided, Error> {
        if let Some(resuming) = self.resuming.take() {
            let Resuming {
                event,
                escaped,
                extras,
            } = *resuming;
            self.phase = event;
            let Phase::Event { seats } = &mut self.phase else {
                unreachable!("only an event resumes after its fight")
            };
            let effects = V::resume_event_from_fight(&mut seats[seat].state, escaped);
            let offering = effects.iter().position(|effect| {
                matches!(effect, sts2_core::HookEffect::OfferRewardsWithRoomExtras(_))
            });
            if let (Some(at), Some(extras)) = (offering, extras) {
                return Ok(self.offer_with_extras(seat, &effects, at, &extras));
            }
            let effects: sts2_core::HookEffects<V> = effects
                .into_iter()
                .filter_map(|effect| match effect {
                    // An empty list offers nothing.
                    sts2_core::HookEffect::OfferRewardsWithRoomExtras([]) => None,
                    sts2_core::HookEffect::OfferRewardsWithRoomExtras(rewards) => {
                        Some(sts2_core::HookEffect::OfferRewards(rewards))
                    }
                    effect => Some(effect),
                })
                .collect();
            return Ok(match self.carry_out(seat, effects) {
                None => Decided::Resolved,
                Some(asked) => self.park(seat, asked),
            });
        }
        let next = self.act + 1;
        let Some(&(act, _)) = self.shared.opening.acts.get(next) else {
            if self.in_victory_room(seat) {
                let mut room = self.event_room(seat);
                if let Some(Room::Event { options, .. }) = &mut room {
                    for option in options {
                        option.chosen = true;
                    }
                }
                self.phase = Phase::Won { room };
                return Ok(Decided::Resolved);
            }
            return self.enter_architect(seat);
        };
        self.act = next;
        self.map = Arc::new(self.generate_map(act));
        self.visited.clear();
        // `Odds.UnknownMapPoint.ResetToBase`, which is the only one of the
        // run's odds an act puts back.
        self.unknown_odds = self.shared.unknown_base;
        // Each act deals its own encounters and is met from the front of its
        // own list.
        self.normal_fights = 0;
        self.elite_fights = 0;
        self.events_used = 0;
        self.phase = Phase::Map;
        self.act_entered();
        Ok(Decided::Resolved)
    }

    /// `Owner.PlayerCombatState?.TurnNumber`: the fight's while one runs, and
    /// otherwise the last one's.
    fn player_turn(&self, seat: sts2_core::SeatIndex) -> Option<V::Count> {
        match &self.phase {
            Phase::Combat(live) => Some(live.turn),
            _ => self.players[seat].last_turn,
        }
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
    pub(super) fn obtain(
        &mut self,
        seat: sts2_core::SeatIndex,
        relic: V::Relic,
    ) -> Option<Asked<V>> {
        self.obtain_as(seat, relic, false)
    }

    /// `RelicCmd.Obtain` of a relic that is wax where `wax` says so.
    pub(super) fn obtain_as(
        &mut self,
        seat: sts2_core::SeatIndex,
        relic: V::Relic,
        wax: bool,
    ) -> Option<Asked<V>> {
        let steps = self.obtained_held(seat, relic, wax, None);
        self.carry_out(seat, steps)
    }

    /// `RelicCmd.Obtain`, of a relic set up for `named` first where it names
    /// a character, up to its `AfterObtained`: the steps of its pickup.
    fn obtained_held(
        &mut self,
        seat: sts2_core::SeatIndex,
        relic: V::Relic,
        wax: bool,
        named: Option<V::Character>,
    ) -> Steps<V> {
        use sts2_core::RelicTable as _;

        // Out of both bags, unless a run can hold more than one. Noted again
        // where it was dealt already, since a refilled shared bag holds it
        // again until it is obtained.
        if !relic.stackable() {
            self.out_of_both_bags(seat, relic);
        }
        let mut held = HeldRelic {
            wax,
            ..HeldRelic::new(relic)
        };
        if let Some(character) = named {
            let mut asked = sts2_core::HookEffects::new();
            // Set up before it joins the belt, which it reads as it stands.
            V::relic_event_hook(
                &mut held.state,
                self.belts[seat].relics(),
                self.relic_owner(seat),
                sts2_core::EventHook::CharacterNamed(character),
                &mut asked,
            );
            if let Some(effect) = asked.first() {
                unimplemented!("{relic:?} asking for {effect:?} as it is set up");
            }
        }
        self.belts[seat].push(held);
        let slot = self.belts[seat].len() - 1;
        // `AddRelicInternal` raises `RelicObtained` before `AfterObtained`.
        // The card rewards that never left it hear it first, in the order
        // they were made.
        if !self.players[seat].unheld_card_rewards.is_empty() {
            let mut unheld =
                std::mem::take(Arc::make_mut(&mut self.players[seat].unheld_card_rewards));
            self.reward_options_on_obtained(seat, slot, &mut unheld);
            *Arc::make_mut(&mut self.players[seat].unheld_card_rewards) = unheld;
        }
        let (character, act, turn) = (self.character(seat), self.act, self.player_turn(seat));
        let effects = crate::combat::obtained::<V>(
            self.relic_owner(seat),
            self.belts[seat].state_beside(slot),
            character,
            act,
            turn,
        );
        effects.into_iter().map(Then::Effect).collect()
    }

    /// Carry out what a pickup asked the run for, in order, as far as the
    /// first effect that asks a question.
    ///
    /// The question comes back with the pickup standing still on it, which
    /// goes on once it is answered, as `AfterObtained` awaits the answer.
    pub(super) fn carry_out(
        &mut self,
        seat: sts2_core::SeatIndex,
        effects: impl IntoIterator<Item = impl Into<Then<V>>>,
    ) -> Option<Asked<V>> {
        let steps: Steps<V> = effects.into_iter().map(Into::into).collect();
        if steps.is_empty() {
            return None;
        }
        let mut rest = pickup::pickup::<V>(seat, steps);
        let lent = pickup::Lent {
            run: self,
            answer: None,
        };
        match rest.start_with(lent) {
            diapause::CoroutineState::Yielded(pickup) => Some(Asked {
                pickup,
                rest: Box::new(rest),
            }),
            diapause::CoroutineState::Complete(()) => None,
        }
    }

    /// The paused pickup in `asked`, once `answer` answers its question.
    fn answer_asked(
        &mut self,
        seat: sts2_core::SeatIndex,
        asked: Asked<V>,
        answer: &Answer,
    ) -> Decided {
        let Asked { mut rest, .. } = asked;
        let lent = pickup::Lent {
            run: self,
            answer: Some(*answer),
        };
        match diapause::Coroutine::resume(&mut *rest, lent) {
            diapause::CoroutineState::Yielded(pickup) => self.park(seat, Asked { pickup, rest }),
            diapause::CoroutineState::Complete(()) => self.pickup_over(seat),
        }
    }

    /// One step of a pickup: carried out where it asks nothing.
    fn step(&mut self, seat: sts2_core::SeatIndex, then: Then<V>) -> Stepped<V> {
        use sts2_core::HookEffect;

        let effect = match then {
            Then::Effect(effect) => effect,
            Then::Walk(walking) => return Stepped::Walk(walking),
            Then::Ask(pickup) => return Stepped::Ask(pickup),
            Then::Purchased {
                shelf,
                spent,
                parasol,
            } => {
                self.purchased(seat, shelf, Self::gold(spent));
                return match parasol {
                    true => Stepped::Nested(
                        seat,
                        self.purchase_everything(seat, Some(shelf)),
                        After::Done,
                    ),
                    false => Stepped::Done,
                };
            }
        };
        {
            let pickup = match effect {
                // `DeckVersion`'s own setters, on the deck card while it is
                // still in the deck.
                HookEffect::WriteDeckCard { card, write } => {
                    if let Some(at) = self.players[seat]
                        .deck
                        .iter()
                        .position(|held| held.id == card)
                    {
                        V::write_deck_card(&mut self.players[seat].deck_mut()[at], write);
                    }
                    return Stepped::Done;
                }
                HookEffect::UpgradeRandomDeckCards {
                    count,
                    stream,
                    kind,
                } => {
                    self.upgrade_random_deck_cards(seat, count, stream, kind);
                    return Stepped::Done;
                }
                HookEffect::GainGold(amount) => {
                    self.gain_gold(seat, amount);
                    return Stepped::Done;
                }
                HookEffect::GainGoldExact(amount) => {
                    self.gain_gold_exact(seat, amount);
                    return Stepped::Done;
                }
                // `CardFactory.CreateForReward`, then `CardPileCmd.Add` of
                // each card to the deck.
                HookEffect::AddRolledToDeck(reward) => {
                    let pool = matches!(reward, sts2_core::CustomReward::CharacterCards { .. })
                        .then(|| self.reward_pool(seat, false));
                    let stream = sts2_core::Stream::Player(V::PlayerRngType::REWARDS);
                    for card in self.custom_cards(seat, reward, stream, pool.as_ref()) {
                        self.add_to_deck(seat, card);
                    }
                    return Stepped::Done;
                }
                HookEffect::PurchaseEverything => {
                    let steps = self.purchase_everything(seat, None);
                    return Stepped::Nested(seat, steps, After::Done);
                }
                HookEffect::ItemPurchased(gold) => {
                    self.item_purchased(seat, gold);
                    return Stepped::Done;
                }
                HookEffect::MarkCombatPoints { relic, count, name } => {
                    let map = Arc::clone(&self.map);
                    self.mark_combat_points(seat, &map, relic, count, name);
                    return Stepped::Done;
                }
                // `PotionCmd.Discard`.
                HookEffect::DiscardPotion(at) => {
                    self.drank(seat, at);
                    self.belt_changed(
                        seat,
                        sts2_core::EventHook::PotionDiscarded {
                            in_fight: matches!(self.phase, Phase::Combat(_)),
                            holding_potions: self.holding_potions(seat),
                        },
                    );
                    return Stepped::Done;
                }
                // `RewardsCmd.OfferCustom` inside the event, which goes on
                // once its rewards are done.
                HookEffect::MimicRestSiteHeal => match self.rest_site_heal(seat, true) {
                    None => return Stepped::Done,
                    Some(screen) => screen,
                },
                HookEffect::FightInEvent { .. } => {
                    unreachable!("a fight in an event is the whole of its option")
                }
                HookEffect::EnterNextAct => {
                    unreachable!("entering the next act is the whole of its option")
                }
                HookEffect::FinishEvent => {
                    let Phase::Event { seats } = &mut self.phase else {
                        unreachable!("only an event's option finishes it")
                    };
                    seats[seat].finished = true;
                    return Stepped::Done;
                }
                HookEffect::ResumeEvent => {
                    // Lent out of the run for as long as it reads it, as a
                    // choice is.
                    let mut phase = std::mem::replace(&mut self.phase, Phase::Map);
                    let Phase::Event { seats } = &mut phase else {
                        unreachable!("only an event's option resumes it")
                    };
                    let EventSeat { state, rng, .. } = &mut seats[seat];
                    let resumed = V::resume_event(state, &self.seen_by(seat), rng);
                    self.phase = phase;
                    let steps = resumed.into_iter().map(Then::Effect).collect();
                    return Stepped::Nested(seat, steps, After::Done);
                }
                HookEffect::LoseGold(amount) => {
                    let left = self.players[seat].gold - amount;
                    self.players[seat].gold = left.max(num_traits::zero());
                    return Stepped::Done;
                }
                HookEffect::LoseAllGold => {
                    self.players[seat].gold = num_traits::zero();
                    return Stepped::Done;
                }
                HookEffect::OfferOtherPoolCards { rewards, pools } => {
                    let mut offered: Vec<Offer<V>> = (0..rewards)
                        .map(|_| Offer::Card {
                            cards: self.other_pool_cards(seat, pools),
                            rolled: None,
                            reroll: false,
                            sacrifice: false,
                            taken: false,
                        })
                        .collect();
                    // `CardReward.Populate` over cards set by hand, which
                    // raises the reward's own pass with `IsCardReward`.
                    self.reward_offers(seat, &mut offered);
                    self.modify_rewards(seat, &mut offered, None);
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
                    bundles: self.bundles(seat, count, commons, uncommons, same_card),
                },
                // The relic's own pickup asks first, and the effects after
                // it wait behind what it asked.
                HookEffect::ObtainRelicFor { relic, character } => {
                    let steps = self.obtained_held(seat, relic, false, Some(character));
                    return Stepped::Nested(seat, steps, After::Done);
                }
                HookEffect::ObtainRelic(_) | HookEffect::ObtainNextRelic(_) => {
                    let relic = match effect {
                        HookEffect::ObtainRelic(relic) => relic,
                        HookEffect::ObtainNextRelic(pull) => {
                            let rarity = pulled_rarity(
                                pull,
                                self.streams.player(seat, V::PlayerRngType::REWARDS),
                            );
                            self.pull_relic_as(seat, pull, rarity, self.floor)
                        }
                        _ => unreachable!("matched above"),
                    };
                    let steps = self.obtained_held(seat, relic, false, None);
                    return Stepped::Nested(seat, steps, After::Done);
                }
                HookEffect::AddToDeck(card) => {
                    self.add_to_deck(seat, CardIdent::new(card));
                    return Stepped::Done;
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
                        self.add_to_deck(seat, CardIdent::new(card));
                    }
                    return Stepped::Done;
                }
                HookEffect::AddRegularCard(filter) => {
                    self.card_reward_unheard(seat);
                    let character = self.character(seat);
                    let pool = V::reward_cards(character, self.players.len() > 1);
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
                        self.ascension(),
                        self.streams.player(seat, V::PlayerRngType::REWARDS),
                        sts2_core::Drafted {
                            common: &common,
                            uncommon: &uncommon,
                            rare: &rare,
                        },
                        1,
                        None,
                    );
                    for card in drawn {
                        let mut card = [card];
                        self.reward_options(seat, &mut card, false);
                        self.add_to_deck(seat, card[0]);
                    }
                    return Stepped::Done;
                }
                HookEffect::AddUpgradedToDeck(card) => {
                    let mut ident = CardIdent::new(card);
                    if sts2_core::CardTable::max_upgrade(card) > 0 {
                        ident.upgrade = 1;
                    }
                    self.add_to_deck(seat, ident);
                    return Stepped::Done;
                }
                HookEffect::RemoveDeckCardAt(at) => {
                    self.players[seat].deck_mut().remove(at);
                    return Stepped::Done;
                }
                // `CardCmd.TransformTo`: each leaves its place, and the
                // replacements join the deck's end in deck order.
                HookEffect::TransformFirstInDeck(pairs) => {
                    let deck = self.players[seat].deck_mut();
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
                        self.add_transformed(seat, ident);
                    }
                    return Stepped::Done;
                }
                HookEffect::TransformInDeck { from, into } => {
                    let deck = self.players[seat].deck_mut();
                    let before = deck.len();
                    deck.retain(|held| held.ident.card != from);
                    let transformed = before - deck.len();
                    for _ in 0..transformed {
                        self.add_transformed(seat, CardIdent::new(into));
                    }
                    return Stepped::Done;
                }
                HookEffect::AddUniformCard { rarity } => {
                    for card in self.uniform_cards(seat, 1, rarity) {
                        self.add_to_deck(seat, CardIdent::new(card));
                    }
                    return Stepped::Done;
                }
                HookEffect::ReplaceRelic { from, to } => {
                    let Some(slot) = self.belts[seat].iter().position(|held| held.relic == from)
                    else {
                        unimplemented!("replacing {from:?}, which the run does not carry");
                    };
                    self.belts[seat].replace(slot, HeldRelic::new(to));
                    let (character, act, turn) =
                        (self.character(seat), self.act, self.player_turn(seat));
                    let effects = crate::combat::obtained::<V>(
                        self.relic_owner(seat),
                        self.belts[seat].state_beside(slot),
                        character,
                        act,
                        turn,
                    );
                    if let Some(effect) = effects.first() {
                        unimplemented!("{to:?} asking for {effect:?} as it replaces {from:?}");
                    }
                    return Stepped::Done;
                }
                HookEffect::ReplaceStarterRelic { upgrades, fallback } => {
                    use sts2_core::{RarityKinds, RelicTable as _};

                    let Some(slot) = self.belts[seat]
                        .iter()
                        .position(|held| held.relic.rarity() == <_ as RarityKinds>::STARTER)
                    else {
                        unimplemented!("replacing a starter relic the run does not carry");
                    };
                    let starter = self.belts[seat][slot].relic;
                    let upgraded = upgrades
                        .iter()
                        .find(|&&(from, _)| from == starter)
                        .map_or(fallback, |&(_, to)| to);
                    self.belts[seat].replace(slot, HeldRelic::new(upgraded));
                    let (character, act, turn) =
                        (self.character(seat), self.act, self.player_turn(seat));
                    let effects = crate::combat::obtained::<V>(
                        self.relic_owner(seat),
                        self.belts[seat].state_beside(slot),
                        character,
                        act,
                        turn,
                    );
                    if let Some(effect) = effects.first() {
                        unimplemented!(
                            "{upgraded:?} asking for {effect:?} as it replaces {starter:?}"
                        );
                    }
                    return Stepped::Done;
                }
                HookEffect::DrawFromStream { stream, below } => {
                    use sts2_core::rng::GameRng as _;

                    let bound = sts2_core::rng::Bound::try_new(
                        num_traits::cast(below).expect("a bound fits the RNG integer"),
                    )
                    .expect("a draw has somewhere to land");
                    let _drawn =
                        stream_of(seat, &mut self.phase, &mut self.streams, stream).next_int(bound);
                    return Stepped::Done;
                }
                HookEffect::RemoveRelic(relic) => {
                    let Some(slot) = self.belts[seat].iter().position(|held| held.relic == relic)
                    else {
                        unimplemented!("removing {relic:?}, which the run does not carry");
                    };
                    self.belts[seat].remove(slot);
                    return Stepped::Done;
                }
                // The relic carries its option out itself.
                HookEffect::RestOptionOfRelic(relic) => {
                    let Some(slot) = self.belts[seat].iter().position(|held| held.relic == relic)
                    else {
                        unimplemented!("an option of {relic:?}, which the run does not carry");
                    };
                    let relic_owner = self.relic_owner(seat);
                    let (state, belt) = self.belts[seat].state_beside(slot);
                    let mut asked = sts2_core::HookEffects::new();
                    let taken = sts2_core::EventHook::RestOptionTaken;
                    V::relic_event_hook(state, belt, relic_owner, taken, &mut asked);
                    if let Some(effect) = asked.first() {
                        unimplemented!("{relic:?} asking for {effect:?} as its option is taken");
                    }
                    return Stepped::Done;
                }
                HookEffect::ProcurePotion(potion) => {
                    self.procure(seat, potion);
                    return Stepped::Done;
                }
                HookEffect::GainPotionSlots(slots) => {
                    let slots: V::Count = num_traits::cast(slots)
                        .unwrap_or_else(|| unimplemented!("{slots} more potion slots"));
                    self.players[seat].potion_slots = self.players[seat].potion_slots + slots;
                    return Stepped::Done;
                }
                HookEffect::FillPotionSlots { stream } => {
                    let character = self.character(seat);
                    // `HasOpenPotionSlots`, before each potion is made.
                    while self.players[seat].potions.len()
                        < num_traits::cast::<_, usize>(self.players[seat].potion_slots)
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
                        if !self.procure(seat, potion) {
                            break;
                        }
                    }
                    return Stepped::Done;
                }
                // `TryToProcure` fails on a full belt, and the potion is lost.
                HookEffect::ProcureRandomPotions {
                    count,
                    stream,
                    in_combat,
                    into_gained,
                } => {
                    let character = self.character(seat);
                    let potions = crate::reward::potion::distinct::<V, _>(
                        self.streams.run(stream),
                        character,
                        count,
                        in_combat,
                    );
                    let slots: usize = num_traits::cast(self.players[seat].potion_slots)
                        .expect("the belt has an addressable number of slots");
                    for (at, potion) in potions.into_iter().enumerate() {
                        match into_gained {
                            true => self.procure_into(seat, potion, slots - count + at),
                            false => self.procure(seat, potion),
                        };
                    }
                    return Stepped::Done;
                }
                // `NextItem`, which draws nothing from an empty list and
                // names nothing to upgrade.
                HookEffect::UpgradeRandomDeckCard { stream } => {
                    use sts2_core::rng::GameRng as _;

                    let upgradable: Vec<usize> = self.upgradable(seat).map(|(at, _)| at).collect();
                    let rng = stream_of(seat, &mut self.phase, &mut self.streams, stream);
                    if let Ok(&at) = rng.choose(&upgradable) {
                        self.act_on_deck(seat, sts2_core::DeckAct::Upgrade, &[at]);
                    }
                    return Stepped::Done;
                }
                // `NextItem` again. What the card saved stays, and its numbers
                // are worked out from it at the new level.
                HookEffect::DowngradeRandomDeckCard { stream } => {
                    use sts2_core::rng::GameRng as _;

                    let upgraded: Vec<usize> = self.players[seat]
                        .deck
                        .iter()
                        .enumerate()
                        .filter(|(_, held)| held.ident.upgrade > 0)
                        .map(|(at, _)| at)
                        .collect();
                    let rng = stream_of(seat, &mut self.phase, &mut self.streams, stream);
                    if let Ok(&at) = rng.choose(&upgraded) {
                        self.players[seat].deck_mut()[at].ident.upgrade = 0;
                    }
                    return Stepped::Done;
                }
                HookEffect::EnchantBasicTagged {
                    tag,
                    enchantment,
                    amount,
                } => {
                    use sts2_core::{CardRarityKinds as _, CardTable as _};

                    let taken: Vec<usize> = (self.players[seat].deck.iter().enumerate())
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
                    self.act_on_deck(seat, act, &taken);
                    return Stepped::Done;
                }
                HookEffect::CloneEnchantedIntoDeck(enchantment) => {
                    let cloned: Vec<sts2_core::DeckCard<V>> = (self.players[seat].deck.iter())
                        .filter(|held| {
                            held.ident
                                .enchantment
                                .is_some_and(|enchanted| enchanted.enchantment == enchantment)
                        })
                        .copied()
                        .collect();
                    for card in cloned {
                        let id = self.players[seat].mint();
                        self.add_deck_card(seat, sts2_core::DeckCard { id, ..card });
                    }
                    return Stepped::Done;
                }
                HookEffect::EnchantRandomDeckCards {
                    enchantment,
                    amount,
                    count,
                    stream,
                } => {
                    use sts2_core::rng::GameRng as _;

                    let mut taken: Vec<usize> = (self.players[seat].deck.iter().enumerate())
                        .filter(|(_, held)| V::can_enchant(enchantment, &held.ident))
                        .map(|(at, _)| at)
                        .collect();
                    stream_of(seat, &mut self.phase, &mut self.streams, stream)
                        .unstable_shuffle(&mut taken);
                    taken.truncate(count);
                    let act = sts2_core::DeckAct::Enchant {
                        enchantment,
                        amount,
                        kind: None,
                    };
                    self.act_on_deck(seat, act, &taken);
                    return Stepped::Done;
                }
                HookEffect::EnchantDeck {
                    enchantment,
                    amount,
                } => {
                    let taken: Vec<usize> = (self.players[seat].deck.iter().enumerate())
                        .filter(|(_, held)| V::can_enchant(enchantment, &held.ident))
                        .map(|(at, _)| at)
                        .collect();
                    let act = sts2_core::DeckAct::Enchant {
                        enchantment,
                        amount,
                        kind: None,
                    };
                    self.act_on_deck(seat, act, &taken);
                    return Stepped::Done;
                }
                // `NextItem` over what the relic keeps, off the rewards
                // stream. The card is made anew, upgraded where it can be,
                // then `CardPileCmd.Add` to the deck.
                HookEffect::ReturnHeldCard(relic) => {
                    use sts2_core::rng::GameRng as _;
                    use sts2_core::{CardTable as _, RelicModel as _};

                    let slot = (self.belts[seat].iter())
                        .rposition(|held| held.relic == relic)
                        .unwrap_or_else(|| {
                            unimplemented!("{relic:?} handing back a card off the belt")
                        });
                    let count = self.belts[seat][slot].state.held_cards().len();
                    let places: Vec<usize> = (0..count).collect();
                    let Ok(&at) = self
                        .streams
                        .player(seat, V::PlayerRngType::REWARDS)
                        .choose(&places)
                    else {
                        return Stepped::Done;
                    };
                    let mut card = self.belts[seat][slot].state.release_card(at);
                    if card.ident.upgrade < card.ident.card.max_upgrade() {
                        card.ident.upgrade += 1;
                    }
                    let id = self.players[seat].mint();
                    self.add_deck_card(seat, sts2_core::DeckCard { id, ..card });
                    return Stepped::Done;
                }
                // `State.Map` in place, and `RemoveStaleVisitedMapCoords`,
                // which keeps the points the new map still has.
                HookEffect::RegenerateMap => {
                    use sts2_core::ActMap as _;

                    let (act, _) = self.shared.opening.acts[self.act];
                    let map = self.generate_map(act);
                    self.visited.retain(|&at| map.at(at).is_ok());
                    self.map = Arc::new(map);
                    return Stepped::Done;
                }
                HookEffect::UpgradeDeck => {
                    let upgradable: Vec<usize> = self.upgradable(seat).map(|(at, _)| at).collect();
                    self.act_on_deck(seat, sts2_core::DeckAct::Upgrade, &upgradable);
                    return Stepped::Done;
                }
                HookEffect::OnDeckCards { act, picks } => {
                    let taken: Vec<usize> = picks
                        .iter()
                        .filter_map(|&pick| self.picked(seat, pick))
                        .collect();
                    self.act_on_deck(seat, act, &taken);
                    return Stepped::Done;
                }
                HookEffect::GainMaxHp(amount) => {
                    let amount = self.health(amount);
                    self.players[seat].max_hp = self.players[seat].max_hp + amount;
                    self.heal(seat, amount);
                    return Stepped::Done;
                }
                // The part of the health above the new maximum is lost as
                // unblockable damage first. The maximum never falls below one.
                HookEffect::LoseMaxHp(amount) => {
                    let amount = self.health(amount);
                    let one: V::Health = num_traits::one();
                    let max = if self.players[seat].max_hp > amount {
                        self.players[seat].max_hp - amount
                    } else {
                        num_traits::zero()
                    };
                    if self.players[seat].hp > max {
                        self.damage_owner(seat, self.players[seat].hp - max);
                    }
                    self.players[seat].max_hp = max.max(one);
                    self.players[seat].hp = self.players[seat].hp.min(self.players[seat].max_hp);
                    return Stepped::Done;
                }
                HookEffect::DamageOwner(amount) => {
                    let amount = self.health(amount);
                    self.damage_owner(seat, amount);
                    return Stepped::Done;
                }
                HookEffect::Heal(amount) => {
                    let amount = self.health(amount);
                    self.heal(seat, amount);
                    return Stepped::Done;
                }
                HookEffect::HealMissing => {
                    self.heal(seat, self.players[seat].max_hp - self.players[seat].hp);
                    return Stepped::Done;
                }
                // `SetCurrentHpInternal` truncates, and the health it adds to
                // is whole, so the share heals its whole part.
                HookEffect::HealPercent(percent) => {
                    let percent: V::Health =
                        num_traits::cast(percent).expect("a share of health fits the run's health");
                    let hundred: V::Health = num_traits::cast(100).expect("a hundred fits health");
                    self.heal(seat, self.players[seat].max_hp * percent / hundred);
                    return Stepped::Done;
                }
                // `RewardsSet.Offer` makes nothing for a dead player.
                HookEffect::AddUniformToDeck {
                    colorless,
                    draws,
                    copies,
                } => {
                    self.add_uniform_to_deck(seat, colorless, draws, copies);
                    return Stepped::Done;
                }
                // `RelicGrabBag.Remove`, which counts the relic as dealt.
                HookEffect::RemoveFromRelicBags(relic) => {
                    self.out_of_both_bags(seat, relic);
                    return Stepped::Done;
                }
                HookEffect::OfferRewards(_) if self.players[seat].hp == num_traits::zero() => {
                    return Stepped::Done;
                }
                HookEffect::OfferRewards(rewards) => Pickup::Rewards {
                    offered: self.custom_rewards(
                        seat,
                        rewards,
                        sts2_core::Stream::Player(V::PlayerRngType::REWARDS),
                    ),
                    skippable: true,
                },
                HookEffect::SelectReward(reward) => Pickup::Rewards {
                    offered: self.filled_rewards(
                        seat,
                        std::slice::from_ref(&reward),
                        sts2_core::Stream::Player(V::PlayerRngType::REWARDS),
                    ),
                    skippable: false,
                },
                // `RewardsSet.WithSkippingDisallowed`, over a `RelicReward` for
                // each relic, which reports `RelicRarity.None`.
                HookEffect::OfferNeowRelics { count, except } => {
                    use sts2_core::rng::GameRng as _;

                    let players = self.players.len();
                    let mut relics: Vec<V::Relic> = V::neow_options()
                        .filter(|&relic| relic != except && V::allowed_at_neow(relic, players))
                        .collect();
                    self.streams
                        .player(seat, V::PlayerRngType::REWARDS)
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
                    let Phase::Event { seats } = &self.phase else {
                        unreachable!("only an event offers rewards of its own")
                    };
                    let rewards = V::event_rewards(&seats[seat].state);
                    let offered = self.custom_rewards(seat, &rewards, sts2_core::Stream::Event);
                    // `RewardsSet.Offer` puts up no screen for an empty set
                    // outside a fight, which `OfferCustom` does not allow.
                    if offered.is_empty() {
                        return Stepped::Done;
                    }
                    Pickup::Rewards {
                        offered,
                        skippable: true,
                    }
                }
                HookEffect::AskEvent => Pickup::Event,
                HookEffect::CloneDeckCard { excluding } => Pickup::CloneDeckCard { excluding },
                // `CloneCard(Deck.Cards[i])` for each place the deck had as it
                // began, so the clones are not cloned again. A clone keeps
                // what its card's model saved.
                HookEffect::CloneDeck => {
                    for at in 0..self.players[seat].deck.len() {
                        let card = sts2_core::DeckCard {
                            id: self.players[seat].mint(),
                            ..self.players[seat].deck[at]
                        };
                        self.add_deck_card(seat, card);
                    }
                    return Stepped::Done;
                }
                // `CardSelectCmd` takes the whole list without asking when it
                // holds no more than it must take, and returns nothing from an
                // empty one.
                HookEffect::TransformBasicStrikesAndDefends(stream) => {
                    use sts2_core::{CardRarityKinds, CardTable as _, CardTagKinds};

                    let taken: Vec<usize> = self.players[seat]
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
                    self.act_on_deck(seat, sts2_core::DeckAct::Transform(stream), &taken);
                    return Stepped::Done;
                }
                // `FromDeckForEnchantment` sorts the cards it is handed back
                // into deck order, so the shuffle moves the stream alone.
                HookEffect::SelectFromShuffledDeck { act, count } => {
                    let mut offered = self.offered(seat, act);
                    let rng = self.streams.run(V::RunRngType::NICHE);
                    sts2_core::rng::GameRng::unstable_shuffle(rng, &mut offered);
                    if offered.len() <= count {
                        let taken: Vec<usize> = offered.into_iter().map(|(at, _)| at).collect();
                        self.act_on_deck(seat, act, &taken);
                        return Stepped::Done;
                    }
                    Pickup::FromDeck {
                        act,
                        min: count,
                        count,
                    }
                }
                HookEffect::SelectFromDeck { act, count } => {
                    let offered = self.offered(seat, act);
                    if offered.len() <= count {
                        let taken: Vec<usize> = offered.into_iter().map(|(at, _)| at).collect();
                        self.act_on_deck(seat, act, &taken);
                        return Stepped::Done;
                    }
                    Pickup::FromDeck {
                        act,
                        min: count,
                        count,
                    }
                }
                HookEffect::SelectFromDeckConfirmed { act, min, max } => {
                    if self.offered(seat, act).is_empty() {
                        return Stepped::Done;
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
                        self.card_reward_unheard(seat);
                        let mut drawn: Vec<V::Card> = Vec::with_capacity(per_rarity);
                        for _ in 0..per_rarity {
                            let card = self.uniform_card_of(seat, pool, rarity, &drawn);
                            drawn.push(card);
                        }
                        let mut made: Vec<_> = drawn.into_iter().map(CardIdent::new).collect();
                        self.reward_options(seat, &mut made, false);
                        offered.extend(made);
                    }
                    Pickup::Cards { offered, min, max }
                }
                HookEffect::ChooseCards {
                    draw,
                    count,
                    min,
                    max,
                    card_reward,
                } => {
                    let drawn = match draw {
                        sts2_core::CardDraw::Uniform(rarity) => self
                            .uniform_cards(seat, count, rarity)
                            .into_iter()
                            .map(CardIdent::new)
                            .collect(),
                        sts2_core::CardDraw::Regular => {
                            self.card_reward_unheard(seat);
                            let character = self.character(seat);
                            crate::reward::card::regular::<V, _>(
                                self.ascension(),
                                self.streams.player(seat, V::PlayerRngType::REWARDS),
                                V::reward_cards(character, self.players.len() > 1),
                                count,
                                None,
                            )
                        }
                        sts2_core::CardDraw::Encounter => {
                            self.card_reward_unheard(seat);
                            let character = self.character(seat);
                            let ascension = self.ascension();
                            let multiplayer = self.players.len() > 1;
                            crate::reward::card::encounter::<V, _>(
                                self.streams.player(seat, V::PlayerRngType::REWARDS),
                                &mut self.players[seat].odds,
                                ascension,
                                V::reward_cards(character, multiplayer),
                                count,
                                None,
                                crate::reward::card::RarityOdds::Regular,
                            )
                        }
                    };
                    let mut offered: Vec<_> = drawn;
                    self.reward_options(seat, &mut offered, card_reward);
                    Pickup::Cards { offered, min, max }
                }
                HookEffect::ChooseColorlessCards { count } => {
                    self.card_reward_unheard(seat);
                    let act = self.act();
                    let mut offered = crate::reward::card::regular::<V, _>(
                        self.ascension(),
                        self.streams.player(seat, V::PlayerRngType::REWARDS),
                        V::colorless_cards(self.players.len() > 1),
                        count,
                        Some(act),
                    );
                    self.reward_options(seat, &mut offered, false);
                    Pickup::Cards {
                        offered,
                        min: 0,
                        max: 1,
                    }
                }
                HookEffect::ChooseMultiplayerCards {
                    count,
                    colorless_first,
                } => {
                    use crate::reward::card::{Pool, Source};
                    use sts2_core::CardTable as _;

                    self.card_reward_unheard(seat);
                    let act = self.act();
                    let mut sources: [Source<V>; 2] =
                        [Source::Character(self.character(seat)), Source::Colorless];
                    if colorless_first {
                        sources.reverse();
                    }
                    let pool = Pool::of(&sources, self.players.len() > 1);
                    let drafted = pool.drafted();
                    let only = |cards: &[V::Card]| -> Vec<V::Card> {
                        (cards.iter().copied())
                            .filter(|card| {
                                card.multiplayer_constraint()
                                    == sts2_core::MultiplayerConstraint::MultiplayerOnly
                            })
                            .collect()
                    };
                    let (common, uncommon, rare) = (
                        only(drafted.common),
                        only(drafted.uncommon),
                        only(drafted.rare),
                    );
                    let mut offered = crate::reward::card::regular::<V, _>(
                        self.ascension(),
                        self.streams.player(seat, V::PlayerRngType::REWARDS),
                        sts2_core::Drafted {
                            common: &common,
                            uncommon: &uncommon,
                            rare: &rare,
                        },
                        count,
                        Some(act),
                    );
                    self.reward_options(seat, &mut offered, false);
                    Pickup::Cards {
                        offered,
                        min: 0,
                        max: 1,
                    }
                }
                effect => unimplemented!("a run-level relic effect: {effect:?}"),
            };
            Stepped::Ask(pickup)
        }
    }

    /// Where in the deck `pick` names, if any card fits it.
    fn picked(&self, seat: sts2_core::SeatIndex, pick: sts2_core::DeckPick<V>) -> Option<usize> {
        use sts2_core::{CardRarityKinds, CardTable as _};

        let basic = |tag| {
            move |(_, card): &(usize, &CardIdent<V>)| {
                card.card.rarity() == CardRarityKinds::BASIC && card.card.tagged(tag)
            }
        };
        let mut deck = self.players[seat]
            .deck
            .iter()
            .map(|held| &held.ident)
            .enumerate();
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
    fn gain_gold(&mut self, seat: sts2_core::SeatIndex, amount: V::Number) {
        self.gain_gold_exact(seat, crate::combat::exact::<V>(amount));
    }

    /// [`State::gain_gold`] of a `decimal`, which the hooks scale before the
    /// fraction is dropped.
    fn gain_gold_exact(&mut self, seat: sts2_core::SeatIndex, amount: sts2_core::Decimal) {
        let hook = sts2_core::ModifyHook::GoldGained {
            player: seat.creature(),
        };
        let mut gained = amount;
        let mut left = self.belts[seat].hearing(sts2_core::Overrides::of(hook.model_hook()));
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            let listener: crate::combat::Heard<'_, V> = sts2_core::Listener::Relic {
                state: &self.belts[seat][slot].state,
                owner: seat.creature(),
                fight: None,
                belt: self.belts[seat].relics(),
            };
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Scaled(scale) => gained *= scale.exact(),
                // The listener's own number in place of what it was handed.
                sts2_core::Modify::Replaced(amount) => gained = crate::combat::exact::<V>(amount),
                answer => unimplemented!("a relic answering {hook:?} with {answer:?}"),
            }
        }
        if let Some(effect) = self
            .notify_run(sts2_core::EventHook::GoldGainModified)
            .first()
        {
            unimplemented!("a relic asking for {effect:?} as a gold gain is modified");
        }
        // `PlayerCmd.GainGold` returns before the gold lands where nothing is
        // left to gain, so `AfterGoldGained` runs only after a gain.
        if gained > sts2_core::Decimal::ZERO {
            self.players[seat].gold = self.players[seat].gold + crate::combat::whole::<V>(gained);
            let hook = sts2_core::EventHook::GoldGained {
                player: seat.creature(),
            };
            if let Some(asked) = self.walk_run(hook) {
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
    fn heal(&mut self, seat: sts2_core::SeatIndex, amount: V::Health) {
        self.players[seat].hp = (self.players[seat].hp + amount).min(self.players[seat].max_hp);
    }

    /// Seat `seat`'s relics' `Owner`: its player's creature and health.
    fn relic_owner(&self, seat: sts2_core::SeatIndex) -> sts2_core::RelicOwner<V> {
        sts2_core::RelicOwner {
            creature: seat.creature(),
            health: self.player_health(seat),
        }
    }

    /// The player's hit points, as a hook hands them to a relic.
    fn player_health(&self, seat: sts2_core::SeatIndex) -> sts2_core::PlayerHealth<V> {
        sts2_core::PlayerHealth {
            hp: self.players[seat].hp,
            max_hp: self.players[seat].max_hp,
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
    fn damage_owner(&mut self, seat: sts2_core::SeatIndex, amount: V::Health) {
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
        let amount = self
            .hp_lost_after_osty(seat, amount)
            .min(self.players[seat].hp);
        let killed = amount == self.players[seat].hp && amount > num_traits::zero();
        self.players[seat].hp = self.players[seat].hp - amount;
        // `Hook.AfterCurrentHpChanged`, for damage that took hit points.
        if amount > num_traits::zero() {
            let hook = sts2_core::EventHook::CurrentHpChanged {
                creature: seat.creature(),
                monster: None,
                pet_of: None,
                delta: -num_traits::cast::<_, V::Number>(amount)
                    .expect("a health fits a game number"),
                in_fight: false,
            };
            if let Some(effect) = self.notify_run(hook).first() {
                unimplemented!("a relic asking for {effect:?} as damage lands outside a fight");
            }
        }
        // `CreatureCmd.Kill`: `BeforeDeath`, `ShouldDie`, then `AfterDeath`.
        // With every player dead, the run is over, which the player's hit
        // points at nothing now say.
        if killed {
            self.unheard(ModelHook::BeforeDeath);
            let preventer = self.death_preventer(seat);
            let hook = sts2_core::EventHook::AfterDeath {
                creature: seat.creature(),
                side: <V::CombatSide as sts2_core::SideKinds>::PLAYER,
                monster: None,
                removal_prevented: preventer.is_some(),
            };
            if let Some(effect) = self.notify_run(hook).first() {
                unimplemented!(
                    "a listener asking for {effect:?} as the player dies outside a fight"
                );
            }
            match preventer {
                Some((owner, slot)) => self.death_prevented(owner, slot),
                // `DeactivateHooks`, once the death has gone through.
                None => {
                    self.belts[seat].deafen();
                    self.players[seat].deaf = true;
                }
            }
        }
    }

    /// `Hook.ShouldDie`, then `Hook.ShouldDieLate`, for `seat`'s player
    /// outside a fight: the first relic that keeps them alive, and whose belt
    /// it is on. Each relic compares the player with its own owner.
    ///
    /// # Panics
    ///
    /// A listener other than a relic, such as a potion that overrides
    /// `ShouldDie`, which outside a fight nothing here carries out.
    fn death_preventer(&self, seat: sts2_core::SeatIndex) -> Option<(sts2_core::SeatIndex, usize)> {
        let creature = seat.creature();
        let asked = [
            sts2_core::DecisionHook::ShouldDie { creature },
            sts2_core::DecisionHook::ShouldDieLate { creature },
        ];
        asked.into_iter().find_map(|hook| {
            let model_hook = hook.model_hook().expect("a death's questions are raised");
            let listed = self.run_listed(model_hook);
            listed.iter().find_map(|listener| match listener {
                RunListener::Relic { seat: owner, slot } => V::decide_hook(
                    crate::combat::Heard::<V>::Relic {
                        state: &self.belts[owner][slot].state,
                        owner: owner.creature(),
                        fight: None,
                        belt: self.belts[owner].relics(),
                    },
                    hook,
                )
                .is_decision_and(|dies| !dies)
                .then_some((owner, slot)),
                listener => {
                    unimplemented!("{listener:?} asked whether the player dies outside a fight")
                }
            })
        })
    }

    /// `AfterPreventingDeath`, asked of the relic in `slot` alone, once it
    /// kept the player alive outside a fight.
    ///
    /// # Panics
    ///
    /// The relic asking for anything but a heal.
    fn death_prevented(&mut self, seat: sts2_core::SeatIndex, slot: usize) {
        let max_hp: V::Number =
            num_traits::cast(self.players[seat].max_hp).expect("maximum health fits a game number");
        let hook = sts2_core::EventHook::DeathPrevented { max_hp };
        let relic = self.belts[seat][slot].relic;
        let relic_owner = self.relic_owner(seat);
        let (state, belt) = self.belts[seat].state_beside(slot);
        let mut asked = sts2_core::HookEffects::new();
        V::relic_event_hook(state, belt, relic_owner, hook, &mut asked);
        for effect in asked {
            match effect {
                sts2_core::HookEffect::Heal(amount) => {
                    let amount = self.health(amount);
                    self.heal(seat, amount);
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
    fn hp_lost_after_osty(&self, seat: sts2_core::SeatIndex, amount: V::Health) -> V::Health {
        let player = seat.creature();
        let mut lost: V::Number = num_traits::cast(amount).expect("a health fits a game number");
        for hook in [
            sts2_core::ModifyHook::HpLostAfterOsty {
                target: player,
                in_fight: false,
            },
            sts2_core::ModifyHook::HpLostAfterOstyLate {
                target: player,
                dealer: None,
                powered: false,
            },
        ] {
            let mut left = self.belts[seat].hearing(sts2_core::Overrides::of(hook.model_hook()));
            while left != 0 {
                let slot = left.trailing_zeros() as usize;
                left &= left - 1;
                let listener: crate::combat::Heard<'_, V> = sts2_core::Listener::Relic {
                    state: &self.belts[seat][slot].state,
                    owner: seat.creature(),
                    fight: None,
                    belt: self.belts[seat].relics(),
                };
                match V::modify_hook(listener, hook) {
                    sts2_core::Modify::Nothing => {}
                    sts2_core::Modify::Added(more) => lost = lost + more,
                    sts2_core::Modify::Less(less) => lost = (lost - less).max(num_traits::zero()),
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
    fn rest_site_heal(&mut self, seat: sts2_core::SeatIndex, mimicked: bool) -> Option<Pickup<V>> {
        self.healed_at_rest_site(seat, mimicked);
        self.rest_site_heal_rewards(seat, mimicked)
    }

    /// `seat`'s player healed by `GetHealAmount`: three tenths of the
    /// maximum after `ModifyRestSiteHealAmount`. Then `AfterRestSiteHeal`.
    ///
    /// # Panics
    ///
    /// A relic changing the amount other than by adding to it, or a
    /// listener asking for something.
    fn healed_at_rest_site(&mut self, seat: sts2_core::SeatIndex, mimicked: bool) {
        let max_hp: u64 = num_traits::cast(self.players[seat].max_hp)
            .expect("maximum health fits an unsigned rest calculation");
        let mut healed: V::Number =
            num_traits::cast(max_hp * 3 / 10).expect("rest healing fits a game number");
        let max_hp: V::Number =
            num_traits::cast(max_hp).expect("maximum health fits a game number");
        let hook = sts2_core::ModifyHook::RestSiteHealAmount { max_hp };
        let mut left = self.belts[seat].hearing(sts2_core::Overrides::of(hook.model_hook()));
        while left != 0 {
            let slot = left.trailing_zeros() as usize;
            left &= left - 1;
            let listener: crate::combat::Heard<'_, V> = sts2_core::Listener::Relic {
                state: &self.belts[seat][slot].state,
                owner: seat.creature(),
                fight: None,
                belt: self.belts[seat].relics(),
            };
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Added(more) => healed = healed + more,
                answer => unimplemented!("a relic answering {hook:?} with {answer:?}"),
            }
        }
        for modifier in self.shared.modifiers.iter().map(|entry| entry.modifier) {
            if !sts2_core::ModifierTable::overrides(modifier).contains(hook.model_hook()) {
                continue;
            }
            let listener: crate::combat::Heard<'_, V> = sts2_core::Listener::Modifier(modifier);
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Replaced(with) => healed = with,
                answer => unimplemented!("a modifier answering {hook:?} with {answer:?}"),
            }
        }
        let healed: V::Health =
            num_traits::cast(healed).expect("rest healing fits the build's health type");
        self.heal(seat, healed);
        let max_hp =
            num_traits::cast(self.players[seat].max_hp).expect("maximum health fits a game number");
        let player = seat.creature();
        let hook = sts2_core::EventHook::RestSiteHeal {
            player,
            mimicked,
            max_hp,
        };
        if let Some(asked) = self.walk_run(hook) {
            let pickup = asked.pickup;
            unimplemented!("a rest site's heal asking for {pickup:?}");
        }
    }

    /// `MendRestSiteOption.OnSelect` aimed at `target`'s player:
    /// `CreatureCmd.Heal` by their `GetHealAmount`, then `AfterRestSiteHeal`
    /// for them, which is not mimicked. It offers no rewards.
    ///
    /// # Panics
    ///
    /// Any other option aimed at a player, or Mend aimed at its own.
    pub(crate) fn mend(
        &mut self,
        seat: sts2_core::SeatIndex,
        index: u8,
        target: sts2_core::SeatIndex,
    ) -> Result<Decided, Error> {
        let Phase::RestSite { seats } = &mut self.phase else {
            unimplemented!("an option aimed at {target:?}'s player outside a rest site");
        };
        let option = *seats[seat]
            .options
            .get(usize::from(index))
            .unwrap_or_else(|| unimplemented!("an option the rest site does not offer"));
        if option != <V::RestSiteOption as sts2_core::RestKinds<V>>::MEND {
            unimplemented!("{option:?} aimed at {target:?}'s player");
        }
        if target == seat {
            unimplemented!("Mend aimed at its own player");
        }
        seats[seat].chosen = Some(option);
        self.healed_at_rest_site(target, false);
        self.rest_option_through(seat);
        Ok(Decided::Resolved)
    }

    /// `Hook.ModifyRestSiteHealRewards`, then `RewardsCmd.OfferCustom`. Each
    /// listener adds to one list, and the list is offered once, as one set.
    fn rest_site_heal_rewards(
        &mut self,
        seat: sts2_core::SeatIndex,
        mimicked: bool,
    ) -> Option<Pickup<V>> {
        let mut rewards: Vec<sts2_core::CustomReward<V>> = Vec::new();
        let hook = sts2_core::EventHook::RestSiteHealRewards {
            player: seat.creature(),
            mimicked,
        };
        self.walk_run_landing(hook, |_, listener, effect| match effect {
            sts2_core::HookEffect::OfferRewards(added) => rewards.extend_from_slice(added),
            effect => unimplemented!("{listener:?} adding {effect:?} to a rest site's rewards"),
        });
        // `RewardsSet.Offer` makes nothing for a dead player.
        if rewards.is_empty() || self.players[seat].hp == num_traits::zero() {
            return None;
        }
        let offered = self.custom_rewards(
            seat,
            &rewards,
            sts2_core::Stream::Player(V::PlayerRngType::REWARDS),
        );
        Some(Pickup::Rewards {
            offered,
            skippable: true,
        })
    }

    /// `RewardsSet.WithCustomRewards` and `GenerateWithoutOffering`, off
    /// `stream`. The potions are drawn as their rewards are made. Each reward
    /// then fills in in the order given, and the hooks and the sort follow.
    fn custom_rewards(
        &mut self,
        seat: sts2_core::SeatIndex,
        rewards: &[sts2_core::CustomReward<V>],
        stream: sts2_core::Stream<V>,
    ) -> Vec<Offer<V>> {
        let mut offered = self.filled_rewards(seat, rewards, stream);
        self.modify_rewards(seat, &mut offered, None);
        crate::reward::sort(&mut offered);
        offered
    }

    /// Each of `rewards` made and filled in, in order, off `stream`, with no
    /// set's hooks or sort over them.
    fn filled_rewards(
        &mut self,
        seat: sts2_core::SeatIndex,
        rewards: &[sts2_core::CustomReward<V>],
        stream: sts2_core::Stream<V>,
    ) -> Vec<Offer<V>> {
        use sts2_core::CustomReward;
        use sts2_core::rng::GameRng as _;

        let character = self.character(seat);
        if rewards.iter().any(|reward| {
            matches!(
                reward,
                CustomReward::CharacterCards { .. }
                    | CustomReward::ColorlessCards(_)
                    | CustomReward::CardsOf { .. }
                    | CustomReward::UpgradedCardsOf { .. }
            )
        }) {
            self.card_reward_unheard(seat);
        }
        // `new PotionReward(potion, player)`: the potion is drawn before the
        // set fills anything in.
        let mut potions = SmallVec::<[V::Potion; 4]>::new();
        for reward in rewards {
            let rng = stream_of(seat, &mut self.phase, &mut self.streams, stream);
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
            .map(|rarity| (self.pull_relic(seat, rarity), rarity))
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
            .then(|| self.reward_pool(seat, false));
        // What a fight's own card reward adds, which the monster room's
        // reward reads the same way.
        let added =
            match (rewards.iter()).any(|reward| matches!(reward, CustomReward::MonsterRoomCards)) {
                true => self.card_reward_added(seat, false),
                false => Vec::new(),
            };
        let ascension = self.ascension();
        let act = self.act();
        let mut offered: Vec<Offer<V>> = Vec::with_capacity(rewards.len());
        for reward in rewards {
            let rng = stream_of(seat, &mut self.phase, &mut self.streams, stream);
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
                        relic: self.pull_relic(seat, rarity),
                        rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
                        wax: matches!(reward, CustomReward::WaxRelic),
                        taken: false,
                    }
                }
                CustomReward::SpecificRelic(relic) => Offer::Relic {
                    relic,
                    rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
                    wax: false,
                    taken: false,
                },
                CustomReward::Gold(amount) => Offer::Gold {
                    amount: crate::reward::fixed_gold::<V, _>(rng, amount),
                    taken: false,
                },
                reward @ (CustomReward::CharacterCards { .. }
                | CustomReward::ColorlessCards(_)
                | CustomReward::CardsOf { .. }
                | CustomReward::UpgradedCardsOf { .. }) => Offer::Card {
                    cards: self.custom_cards(seat, reward, stream, pool.as_ref()),
                    rolled: Some(crate::reward::Rolled::Custom { reward, stream }),
                    reroll: false,
                    sacrifice: false,
                    taken: false,
                },
                CustomReward::MonsterRoomCards => crate::reward::combat::card_reward::<V, _>(
                    rng,
                    &mut self.players[seat].odds,
                    ascension,
                    pool.as_ref().expect("a monster room's reward has its pool"),
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
                CustomReward::SpecialCard(card) => Offer::SpecialCard {
                    card: CardIdent::new(card),
                    state: <V::CardState as sts2_core::CardModel<V>>::new(card),
                    returning: None,
                    taken: false,
                },
            });
        }
        self.reward_offers(seat, &mut offered);
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
        seat: sts2_core::SeatIndex,
        reward: sts2_core::CustomReward<V>,
        stream: sts2_core::Stream<V>,
        pool: Option<&crate::reward::card::Pool<V>>,
    ) -> Vec<CardIdent<V>> {
        use sts2_core::CustomReward;

        let (character, act, ascension) = (self.character(seat), self.act(), self.ascension());
        let multiplayer = self.players.len() > 1;
        // What a pool's hooks leave of a rarity, which they read before the
        // first card is drawn.
        let hooked = match reward {
            CustomReward::CardsOf {
                pool,
                rarity,
                no_pool_modifications: false,
                ..
            } => Some(self.cards_of(seat, pool.unwrap_or(character), rarity)),
            _ => None,
        };
        let rng = stream_of(seat, &mut self.phase, &mut self.streams, stream);
        match reward {
            CustomReward::CharacterCards {
                count,
                upgrade_roll,
            } => crate::reward::card::regular::<V, _>(
                ascension,
                rng,
                pool.expect("a character's card reward has its pools")
                    .drafted(),
                count,
                upgrade_roll.then_some(act),
            ),
            CustomReward::ColorlessCards(count) => crate::reward::card::regular::<V, _>(
                ascension,
                rng,
                V::colorless_cards(self.players.len() > 1),
                count,
                None,
            ),
            CustomReward::CardsOf {
                pool,
                rarity,
                count,
                upgrade_roll,
                ..
            } => crate::reward::card::uniform_rolled::<V, _>(
                ascension,
                rng,
                match &hooked {
                    Some(cards) => cards,
                    None => V::reward_cards(pool.unwrap_or(character), self.players.len() > 1)
                        .at(rarity),
                },
                count,
                upgrade_roll.then_some(act),
            ),
            CustomReward::UpgradedCardsOf {
                rarity,
                kind,
                count,
            } => crate::reward::card::upgraded_of::<V, _>(
                rng,
                &V::merchant_cards(character, kind, rarity, multiplayer),
                count,
            ),
            reward => unreachable!("{reward:?} makes no pack of cards"),
        }
    }

    /// `Hook.ModifyRewards`, once a set is filled in: `TryModifyRewards`,
    /// then `TryModifyRewardsLate`. A reward they add or swap in is filled
    /// in once both are through, in the set's order. The set's sort then
    /// puts it among its own kind.
    ///
    /// # Panics
    ///
    /// A listener modifying rewards some other way.
    pub(super) fn modify_rewards(
        &mut self,
        seat: sts2_core::SeatIndex,
        offered: &mut Vec<Offer<V>>,
        room: Option<V::RoomType>,
    ) {
        use crate::reward::combat::Room;
        use sts2_core::RoomKinds as _;

        let mut set: Vec<Filling<V>> = offered.drain(..).map(Filling::Filled).collect();
        let hook = sts2_core::EventHook::ModifyRewards {
            room,
            act: self.act(),
            acts: self.shared.opening.acts.len(),
        };
        for effect in self.notify_run_for(seat, hook) {
            match effect {
                sts2_core::HookEffect::AddRelicRewards(count) => {
                    set.extend((0..count).map(|_| Filling::Relic));
                }
                sts2_core::HookEffect::AddCardReward(room) => set.push(Filling::Card {
                    room: match room {
                        room if room == V::RoomType::ELITE => Room::Elite,
                        room if room == V::RoomType::BOSS => Room::Boss,
                        _ => Room::Monster,
                    },
                    reroll: false,
                }),
                // `GoldReward(amount)` sets its amount as it is built, so it
                // is populated already and draws nothing.
                sts2_core::HookEffect::AddGoldReward(amount) => {
                    set.push(Filling::Filled(Offer::Gold {
                        amount,
                        taken: false,
                    }))
                }
                effect => unimplemented!("a listener modifying rewards with {effect:?}"),
            }
        }
        let late = self.notify_run_for(seat, sts2_core::EventHook::ModifyRewardsLate { room });
        for effect in late {
            match effect {
                sts2_core::HookEffect::RerollableCardRewards => {
                    for filling in set.iter_mut() {
                        if let Filling::Filled(Offer::Card { reroll, .. })
                        | Filling::Card { reroll, .. } = filling
                        {
                            *reroll = true;
                        }
                    }
                }
                // `new GoldReward(Amount * 2)`, populated as it is built.
                sts2_core::HookEffect::DoubleGoldRewards => {
                    for filling in set.iter_mut() {
                        if let Filling::Filled(Offer::Gold { amount, .. }) = filling {
                            *amount = *amount + *amount;
                        }
                    }
                }
                // `new RelicReward(player)` where each card reward stood. The
                // card reward is neither offered nor skipped, so it goes on
                // hearing `RelicObtained`. One a listener added is not
                // populated yet, and holds no cards.
                sts2_core::HookEffect::RelicRewardsForCardRewards => {
                    for filling in set.iter_mut() {
                        let unheld = match std::mem::replace(filling, Filling::Relic) {
                            Filling::Filled(offer @ Offer::Card { .. }) => offer,
                            Filling::Card { room, reroll } => Offer::Card {
                                cards: Vec::new(),
                                rolled: Some(crate::reward::Rolled::Room {
                                    room,
                                    from_combat: false,
                                }),
                                reroll,
                                sacrifice: false,
                                taken: false,
                            },
                            other => {
                                *filling = other;
                                continue;
                            }
                        };
                        Arc::make_mut(&mut self.players[seat].unheld_card_rewards).push(unheld);
                    }
                }
                effect => unimplemented!("a listener modifying rewards late with {effect:?}"),
            }
        }
        for filling in set {
            let offer = match filling {
                Filling::Filled(offer) => offer,
                // `RelicReward.Populate` of no set rarity.
                Filling::Relic => {
                    let (rng, _) = self.rewarding(seat);
                    let rarity = crate::reward::relic_rarity::<V, _>(rng);
                    Offer::Relic {
                        relic: self.pull_relic(seat, rarity),
                        rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
                        wax: false,
                        taken: false,
                    }
                }
                // `CardReward.Populate`: three cards at the odds of the room
                // named.
                Filling::Card { room, reroll } => {
                    let ascension = self.ascension();
                    let act = self.act();
                    // `CardCreationOptions.ForRoom`, whose source is an
                    // encounter, with no `IsFromCombat`.
                    let added = self.card_reward_added(seat, false);
                    let pool = self.reward_pool(seat, room == Room::Elite);
                    let (rng, odds) = self.rewarding(seat);
                    let Offer::Card { mut cards, .. } = crate::reward::combat::card_reward::<V, _>(
                        rng, odds, ascension, &pool, act, room, &added,
                    ) else {
                        unreachable!("a card reward rolls a card offer");
                    };
                    self.reward_options(seat, &mut cards, true);
                    Offer::Card {
                        cards,
                        rolled: Some(crate::reward::Rolled::Room {
                            room,
                            from_combat: false,
                        }),
                        reroll,
                        sacrifice: false,
                        taken: false,
                    }
                }
            };
            offered.push(offer);
        }
        self.card_reward_alternatives(seat, offered);
    }

    /// `Hook.ModifyCardRewardAlternatives` over each card reward of a set.
    fn card_reward_alternatives(&mut self, seat: sts2_core::SeatIndex, offered: &mut [Offer<V>]) {
        let hook = sts2_core::EventHook::CardRewardAlternatives {
            player: seat.creature(),
        };
        for offer in offered {
            let Offer::Card { sacrifice, .. } = offer else {
                continue;
            };
            for effect in self.notify_run(hook) {
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
    pub(super) fn add_to_deck(&mut self, seat: sts2_core::SeatIndex, card: CardIdent<V>) {
        let id = self.players[seat].mint();
        self.add_deck_card(seat, sts2_core::DeckCard::new(id, card));
    }

    /// [`State::add_to_deck`] for a card that carries what its model saved.
    pub(super) fn add_deck_card(
        &mut self,
        seat: sts2_core::SeatIndex,
        card: sts2_core::DeckCard<V>,
    ) {
        self.land_in_deck(seat, card, false);
    }

    /// `CardCmd.Transform`'s replacement for a card the deck held, which
    /// lands at the deck's bottom.
    fn add_transformed(&mut self, seat: sts2_core::SeatIndex, card: CardIdent<V>) {
        let id = self.players[seat].mint();
        self.land_in_deck(seat, sts2_core::DeckCard::new(id, card), true);
    }

    /// `Hook.ModifyCardBeingAddedToDeck` over `card`, then the card at the
    /// deck's bottom and `AfterCardChangedPiles` for it.
    fn land_in_deck(
        &mut self,
        seat: sts2_core::SeatIndex,
        mut card: sts2_core::DeckCard<V>,
        transformed: bool,
    ) {
        use sts2_core::EventHook;

        // `Hook.ModifyCardBeingAddedToDeck`, both passes.
        card.ident = self.rewritten(seat, card.ident, |card| EventHook::CardBeingAddedToDeck {
            card,
        });
        self.unheard(sts2_core::ModelHook::TryModifyCardBeingAddedToDeckLate);

        self.players[seat].deck_mut().push(card);
        self.joined_deck(seat, card, transformed);
    }

    /// `AfterCardChangedPiles` for `card`, which just joined the deck with no
    /// `clonedBy`, and what its listeners ask for, each carried out before
    /// the next is asked.
    ///
    /// # Panics
    ///
    /// A listener other than a relic or a modifier asking for a copy.
    fn joined_deck(
        &mut self,
        seat: sts2_core::SeatIndex,
        card: sts2_core::DeckCard<V>,
        transformed: bool,
    ) {
        let hook = sts2_core::EventHook::AddedToDeck {
            card: card.ident.card,
            player: seat.creature(),
            id: card.id,
            cloned: false,
            transformed,
        };
        self.walk_run_landing(hook, |state, listener, effect| match effect {
            sts2_core::HookEffect::CopyIntoDeck(copies) => {
                let copier = match listener {
                    RunListener::Relic { seat: owner, slot } => Copier::Relic(owner, slot),
                    RunListener::Modifier { at, .. } => Copier::Modifier(at),
                    copier => unimplemented!("{copier:?} copying a card into the deck"),
                };
                state.copy_into_deck(seat, card, copies, copier);
            }
            effect => state.card_joined_deck(seat, effect),
        });
    }

    /// `RunState.CloneCard` of `card`, and the copy handed to `copier`. Then
    /// `CardPileCmd.Add` to the deck's bottom with `clonedBy` the copier,
    /// which the deck's hooks hear as a clone. Each of `copies` in turn.
    ///
    /// # Panics
    ///
    /// The copier asking for anything over its copy, or a listener replacing
    /// the copy as it joins.
    fn copy_into_deck(
        &mut self,
        seat: sts2_core::SeatIndex,
        card: sts2_core::DeckCard<V>,
        copies: usize,
        copier: Copier,
    ) {
        use sts2_core::EventHook;

        for _ in 0..copies {
            let mut copy = sts2_core::DeckCard {
                id: self.players[seat].mint(),
                ..card
            };
            let made = EventHook::CopyMade { id: copy.id };
            let mut asked = sts2_core::HookEffects::new();
            match copier {
                Copier::Relic(owner, slot) => {
                    let relic_owner = self.relic_owner(owner);
                    let (state, belt) = self.belts[owner].state_beside(slot);
                    V::relic_event_hook(state, belt, relic_owner, made, &mut asked);
                }
                Copier::Modifier(at) => {
                    let entry = self.shared.modifiers[at];
                    let state = &mut Arc::make_mut(&mut self.modifier_states)[at];
                    V::modifier_event_hook(entry, state, made, &mut asked);
                }
            }
            if let Some(effect) = asked.first() {
                unimplemented!("{copier:?} asking for {effect:?} for its copy");
            }
            let rewritten = self.rewritten(seat, copy.ident, |card| {
                EventHook::CardBeingAddedToDeck { card }
            });
            if rewritten != copy.ident {
                unimplemented!("a listener replacing a copy {copier:?} made");
            }
            copy.ident = rewritten;
            self.unheard(sts2_core::ModelHook::TryModifyCardBeingAddedToDeckLate);
            self.players[seat].deck_mut().push(copy);
            let hook = EventHook::AddedToDeck {
                card: copy.ident.card,
                player: seat.creature(),
                id: copy.id,
                cloned: true,
                transformed: false,
            };
            self.walk_run_landing(hook, |state, _, effect| {
                state.card_joined_deck(seat, effect)
            });
        }
    }

    /// What a relic asks for as a card joins the deck, other than a copy of
    /// it.
    ///
    /// # Panics
    ///
    /// Anything but gold, a heal or more maximum health.
    fn card_joined_deck(&mut self, seat: sts2_core::SeatIndex, effect: sts2_core::HookEffect<V>) {
        match effect {
            sts2_core::HookEffect::GainGold(amount) => self.gain_gold(seat, amount),
            sts2_core::HookEffect::Heal(amount) => {
                let healed: V::Health =
                    num_traits::cast(amount).expect("a relic's heal fits the build's health");
                self.heal(seat, healed);
            }
            // `CreatureCmd.GainMaxHp`: the maximum grows, then a heal of as
            // much.
            sts2_core::HookEffect::GainMaxHp(amount) => {
                let amount = self.health(amount);
                self.players[seat].max_hp = self.players[seat].max_hp + amount;
                self.heal(seat, amount);
            }
            effect => unimplemented!("a relic asking for {effect:?} as a card joins the deck"),
        }
    }

    /// `RunState.IterateHookListeners` outside a fight: every listener that
    /// overrides `hook`, in the game's order.
    ///
    /// That is each player's deck in seat order, each card followed by its
    /// enchantment. Then each player's belt but a melted relic, and their
    /// potions. Then the run's modifiers.
    pub(super) fn run_listed(&self, hook: sts2_core::ModelHook) -> RunListed<V> {
        let hooks = sts2_core::Overrides::of(hook);
        let mut listed = RunListed {
            deck: SmallVec::new(),
            belts: SmallVec::new(),
            after: SmallVec::new(),
            taken: Cursor::default(),
        };
        self.list_player(
            &mut listed,
            sts2_core::SeatIndex(0),
            self.players.host(),
            hook,
        );
        for (seat, player) in self.players.others() {
            self.list_player(&mut listed, seat, player, hook);
        }
        for (at, &modifier) in self.shared.modifiers.iter().enumerate() {
            if sts2_core::ModifierTable::overrides(modifier.modifier).intersects(hooks) {
                listed.after.push(RunListener::Modifier { at, modifier });
            }
        }
        listed
    }

    /// One player's part of [`Self::run_listed`]: its deck, its belt and
    /// its potions.
    #[inline(always)]
    fn list_player(
        &self,
        listed: &mut RunListed<V>,
        seat: sts2_core::SeatIndex,
        player: &Player<V>,
        hook: sts2_core::ModelHook,
    ) {
        let hooks = sts2_core::Overrides::of(hook);
        let relics = self.belts[seat].hearing_hook(hook);
        let mut potions = false;
        // `IsActiveForHooks`, which a dead player's deck and potions fail. A
        // dead player's belt is deafened.
        if !player.deaf {
            let deck = self.deck_listed(seat, hooks);
            match listed.deck.is_empty() {
                true => listed.deck = deck,
                false => listed.deck.extend(deck),
            }
            for &potion in player.potions.iter() {
                if sts2_core::PotionTable::overrides(potion).intersects(hooks) {
                    listed.after.push(RunListener::Potion { seat, potion });
                    potions = true;
                }
            }
        }
        if relics != 0 || potions {
            listed.belts.push((seat, relics));
        }
    }

    /// The deck's part of [`Self::run_listed`]: each card that overrides one
    /// of `hooks`, then its enchantment if that does.
    #[inline(always)]
    fn deck_listed(
        &self,
        seat: sts2_core::SeatIndex,
        hooks: sts2_core::Overrides,
    ) -> SmallVec<[RunListener<V>; 2]> {
        use sts2_core::{CardTable as _, EnchantmentTable as _};

        let mut listed = SmallVec::new();
        // Most hooks no card's or enchantment's class overrides.
        if !V::Card::heard_by_any().intersects(hooks)
            && !V::Enchantment::heard_by_any().intersects(hooks)
        {
            return listed;
        }
        for held in self.players[seat].deck.iter() {
            let (id, card) = (held.id, held.ident.card);
            if card.overrides().intersects(hooks) {
                listed.push(RunListener::DeckCard { seat, id, card });
            }
            if let Some(enchanted) = held.ident.enchantment
                && enchanted.enchantment.overrides().intersects(hooks)
            {
                listed.push(RunListener::Enchantment {
                    seat,
                    id,
                    enchanted,
                });
            }
        }
        listed
    }

    /// `hook` asked of `listener`, outside a fight, with what it asks for
    /// pushed onto `asked`. Whether the run still holds it, which is
    /// `RunState.Contains`: a deck card taken out of the deck, or a potion
    /// used, is asked nothing.
    ///
    /// # Panics
    ///
    /// An enchantment on a deck card, which nothing here asks outside a
    /// fight.
    #[inline(always)]
    fn ask_run_listener(
        &mut self,
        listener: RunListener<V>,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) -> bool {
        match listener {
            RunListener::DeckCard { seat, id, card } => {
                let player = &mut self.players[seat];
                let Some(at) = player.deck.iter().position(|held| held.id == id) else {
                    return false;
                };
                let state = &mut player.deck_mut()[at].state;
                V::deck_card_event_hook(card, state, hook, asked);
            }
            RunListener::Enchantment { enchanted, .. } => {
                unimplemented!("{enchanted:?} on a deck card hearing {hook:?}")
            }
            RunListener::Relic { seat, slot } => {
                let relic_owner = self.relic_owner(seat);
                let (state, belt) = self.belts[seat].state_beside(slot);
                V::relic_event_hook(state, belt, relic_owner, hook, asked);
            }
            RunListener::Potion { seat, potion } => {
                if !self.players[seat].potions.contains(&potion) {
                    return false;
                }
                V::potion_event_hook(potion, hook, asked);
            }
            RunListener::Modifier { at, modifier } => {
                let state = &mut Arc::make_mut(&mut self.modifier_states)[at];
                V::modifier_event_hook(modifier, state, hook, asked);
            }
        }
        true
    }

    /// The player a listener's asks are carried out for: its owner. A
    /// modifier is the run's, and a solo run has one player to act on.
    ///
    /// # Panics
    ///
    /// A modifier's asks in a run of several players, which nobody has
    /// ported.
    fn owner(&self, listener: RunListener<V>) -> sts2_core::SeatIndex {
        match (listener.seat(), self.players.len()) {
            (Some(seat), _) => seat,
            (None, 1) => sts2_core::SeatIndex(0),
            (None, players) => {
                unimplemented!("{listener:?} asking in a run of {players} players")
            }
        }
    }

    /// `hook`'s walk outside a fight, as `Hook` makes it. Each listener the
    /// run still holds is asked in turn. What it asks for is carried out
    /// before the next is asked, and a question it asks parks the rest of
    /// the walk behind it.
    pub(super) fn walk_run(&mut self, hook: sts2_core::EventHook<V>) -> Option<Asked<V>> {
        let walking = self.run_walking(hook)?;
        // Each listener's asks run as its owner's, so the walk's own seat is
        // never read.
        self.carry_out(sts2_core::SeatIndex(0), [Then::Walk(walking)])
    }

    /// The walk over `hook`'s listeners, where any listens.
    fn run_walking(&self, hook: sts2_core::EventHook<V>) -> Option<RunWalking<V>> {
        let model_hook = hook.model_hook()?;
        let listed = self.run_listed(model_hook);
        (!listed.is_empty()).then_some(RunWalking { hook, listed })
    }

    /// The next listener in `walking` that asks for something: whose it is,
    /// and the steps of what it asks for.
    fn walk_next(
        &mut self,
        walking: &mut RunWalking<V>,
    ) -> Option<(sts2_core::SeatIndex, Steps<V>)> {
        let mut asked = sts2_core::HookEffects::new();
        loop {
            let listener = walking.listed.take_first()?;
            let hook = self.refreshed(listener.seat(), walking.hook);
            if !self.ask_run_listener(listener, hook, &mut asked) || asked.is_empty() {
                continue;
            }
            let owner = self.owner(listener);
            return Some((owner, asked.drain(..).map(Then::Effect).collect()));
        }
    }

    /// [`Self::walk_run`] where `land` carries out each listener's answer,
    /// which asks no question.
    fn walk_run_landing(
        &mut self,
        hook: sts2_core::EventHook<V>,
        mut land: impl FnMut(&mut Self, RunListener<V>, sts2_core::HookEffect<V>),
    ) {
        let Some(model_hook) = hook.model_hook() else {
            return;
        };
        let listed = self.run_listed(model_hook);
        let mut asked = sts2_core::HookEffects::new();
        for listener in listed.iter() {
            let hook = self.refreshed(listener.seat(), hook);
            if !self.ask_run_listener(listener, hook, &mut asked) {
                continue;
            }
            for effect in std::mem::take(&mut asked) {
                land(self, listener, effect);
            }
        }
    }

    /// `hook` with what it reads off the run as it stands, which a listener
    /// reads afresh once the one before it has acted. What it reads of a
    /// player is its owner's. A modifier, which no player owns, reads the
    /// hook as it was raised.
    #[inline(always)]
    fn refreshed(
        &self,
        owner: Option<sts2_core::SeatIndex>,
        hook: sts2_core::EventHook<V>,
    ) -> sts2_core::EventHook<V> {
        use sts2_core::EventHook;

        let Some(seat) = owner else {
            return hook;
        };
        match hook {
            EventHook::RoomEntered {
                room,
                strayed,
                point,
                base,
                ..
            } => EventHook::RoomEntered {
                room,
                strayed,
                point,
                deck: self.players[seat].deck.len(),
                player: self.player_health(seat),
                base,
            },
            EventHook::PotionUsed {
                player, in_fight, ..
            } => EventHook::PotionUsed {
                player,
                in_fight,
                holding_potions: self.holding_potions(seat),
            },
            // The healed player's, which an earlier listener can move.
            EventHook::RestSiteHeal {
                player, mimicked, ..
            } => EventHook::RestSiteHeal {
                player,
                mimicked,
                max_hp: num_traits::cast(self.players[sts2_core::SeatIndex(player.get())].max_hp)
                    .expect("maximum health fits a game number"),
            },
            hook => hook,
        }
    }

    /// `hook` raised on every listener outside a fight, in
    /// [`Self::run_listed`] order, and what they answered, in that order.
    ///
    /// For a hook whose listeners change something the run hands them, such
    /// as a reward set or a map, and change nothing else. A hook whose
    /// listeners act on the run is walked with [`Self::walk_run`].
    pub(super) fn notify_run(
        &mut self,
        hook: sts2_core::EventHook<V>,
    ) -> sts2_core::HookEffects<V> {
        let mut asked = sts2_core::HookEffects::new();
        let Some(model_hook) = hook.model_hook() else {
            return asked;
        };
        for listener in self.run_listed(model_hook).iter() {
            self.ask_run_listener(listener, hook, &mut asked);
        }
        asked
    }

    /// [`Self::notify_run`] over `seat`'s own listeners and the run's, for a
    /// hook raised for one player's reward. Every other player's listener
    /// returns from it at once, as each of the game's checks `player !=
    /// Owner`.
    pub(super) fn notify_run_for(
        &mut self,
        seat: sts2_core::SeatIndex,
        hook: sts2_core::EventHook<V>,
    ) -> sts2_core::HookEffects<V> {
        let mut asked = sts2_core::HookEffects::new();
        let Some(model_hook) = hook.model_hook() else {
            return asked;
        };
        for listener in self.run_listed(model_hook).iter() {
            if listener.seat().is_none_or(|owner| owner == seat) {
                self.ask_run_listener(listener, hook, &mut asked);
            }
        }
        asked
    }

    /// [`Self::notify_run`], each answer beside the player whose listener
    /// gave it, for answers that act on their own player.
    pub(super) fn notify_run_owned(
        &mut self,
        hook: sts2_core::EventHook<V>,
    ) -> SmallVec<[(sts2_core::SeatIndex, sts2_core::HookEffect<V>); 2]> {
        let mut owned = SmallVec::new();
        let Some(model_hook) = hook.model_hook() else {
            return owned;
        };
        let mut asked = sts2_core::HookEffects::new();
        for listener in self.run_listed(model_hook).iter() {
            self.ask_run_listener(listener, hook, &mut asked);
            let owner = self.owner(listener);
            owned.extend(asked.drain(..).map(|effect| (owner, effect)));
        }
        owned
    }

    /// A question or a number asked of every listener outside a fight that
    /// overrides `hook`, in [`Self::run_listed`] order, with every
    /// hook that listener overrides.
    ///
    /// # Panics
    ///
    /// A deck card or its enchantment, whose answers outside a fight nobody
    /// has ported.
    pub(super) fn run_each(
        &self,
        hook: sts2_core::ModelHook,
        mut ask: impl FnMut(crate::combat::Heard<'_, V>, sts2_core::Overrides),
    ) {
        use sts2_core::RelicTable as _;

        for listener in self.run_listed(hook).iter() {
            match listener {
                RunListener::DeckCard { card, .. } => {
                    unimplemented!("{card:?} in the deck answering {}", hook.name())
                }
                RunListener::Enchantment { enchanted, .. } => {
                    unimplemented!("{enchanted:?} on a deck card answering {}", hook.name())
                }
                RunListener::Relic { seat, slot } => ask(
                    sts2_core::Listener::Relic {
                        state: &self.belts[seat][slot].state,
                        owner: seat.creature(),
                        fight: None,
                        belt: self.belts[seat].relics(),
                    },
                    self.belts[seat][slot].relic.overrides(),
                ),
                RunListener::Potion { potion, seat } => ask(
                    sts2_core::Listener::Potion {
                        potion,
                        owner: seat.creature(),
                    },
                    sts2_core::PotionTable::overrides(potion),
                ),
                RunListener::Modifier { modifier, .. } => ask(
                    sts2_core::Listener::Modifier(modifier.modifier),
                    sts2_core::ModifierTable::overrides(modifier.modifier),
                ),
            }
        }
    }

    /// `hook`, asked of each listener that overrides it, outside a fight.
    ///
    /// # Panics
    ///
    /// A listener that changes the value.
    fn belt_leaves(&self, hook: sts2_core::ModifyHook<V>) {
        self.run_each(hook.model_hook(), |listener, _| {
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                answer => {
                    unimplemented!("a listener answering {hook:?} outside a fight with {answer:?}")
                }
            }
        });
    }

    /// The deck's cards whose class overrides `hook`, in
    /// [`Self::run_listed`] order, which the run asks through its content.
    ///
    /// # Panics
    ///
    /// Any other listener to `hook`, whose answer nobody has ported.
    pub(super) fn deck_hearing(&self, hook: sts2_core::ModelHook) -> SmallVec<[V::Card; 2]> {
        let listed = self.run_listed(hook);
        (listed.iter())
            .map(|listener| match listener {
                RunListener::DeckCard { card, .. } => card,
                listener => unimplemented!("{listener:?} listening to {}", hook.name()),
            })
            .collect()
    }

    /// A hook the run raises outside a fight and no ported listener answers.
    ///
    /// # Panics
    ///
    /// Any listener to `hook`.
    pub(super) fn unheard(&self, hook: sts2_core::ModelHook) {
        if let Some(listener) = self.run_listed(hook).iter().next() {
            unimplemented!("{listener:?} listening to {} outside a fight", hook.name());
        }
    }

    /// `CardFactory.CreateForReward` at `Uniform` odds for cards of one
    /// rarity, off the rewards stream, no two the same.
    ///
    /// Each is one `NextItem` over the character's reward pool at that
    /// rarity, less what this call drew already. `NoUpgradeRoll` rolls
    /// nothing else.
    fn uniform_cards(
        &mut self,
        seat: sts2_core::SeatIndex,
        count: usize,
        rarity: sts2_core::RewardRarity,
    ) -> Vec<V::Card> {
        self.card_reward_unheard(seat);
        let mut drawn: Vec<V::Card> = Vec::with_capacity(count);
        for _ in 0..count {
            let card = self.uniform_card(seat, rarity, &drawn);
            drawn.push(card);
        }
        drawn
    }

    /// One card from each of `pools` other characters' pools, for
    /// [`sts2_core::HookEffect::OfferOtherPoolCards`].
    ///
    /// `Character::ALL` lists the characters in id order, which is the order
    /// `StableShuffle` sorts their pools into first.
    fn other_pool_cards(&mut self, seat: sts2_core::SeatIndex, pools: usize) -> Vec<CardIdent<V>> {
        use sts2_core::rng::GameRng as _;
        use sts2_core::{CharacterTable as _, Table as _};

        let own = self.character(seat);
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
                self.card_reward_unheard(seat);
                let (act, ascension) = (self.act(), self.ascension());
                let rng = self.streams.player(seat, V::PlayerRngType::REWARDS);
                let [card] = crate::reward::card::regular::<V, _>(
                    ascension,
                    rng,
                    V::reward_cards(character, self.players.len() > 1),
                    1,
                    Some(act),
                )[..] else {
                    unreachable!("one card drawn is one card");
                };
                self.rewritten(seat, card, |card| sts2_core::EventHook::CardRewardOption {
                    card,
                    card_reward: false,
                })
            })
            .collect()
    }

    /// `ScrollBoxes.GenerateRandomBundles`: each bundle `commons` commons then
    /// `uncommons` uncommons, `NextItem` off the rewards stream over the
    /// cards no bundle holds yet. See [`sts2_core::HookEffect::OfferBundles`].
    fn bundles(
        &mut self,
        seat: sts2_core::SeatIndex,
        count: usize,
        commons: usize,
        uncommons: usize,
        same_card: Option<(V::Card, u32)>,
    ) -> Vec<Vec<V::Card>> {
        use sts2_core::rng::{Bound, GameRng as _};

        let pool = V::reward_cards(self.character(seat), self.players.len() > 1);
        let rng = self.streams.player(seat, V::PlayerRngType::REWARDS);
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
    fn uniform_card(
        &mut self,
        seat: sts2_core::SeatIndex,
        rarity: sts2_core::RewardRarity,
        drawn: &[V::Card],
    ) -> V::Card {
        self.uniform_card_of(seat, self.character(seat), rarity, drawn)
    }

    /// [`State::uniform_card`] over another character's pool.
    fn uniform_card_of(
        &mut self,
        seat: sts2_core::SeatIndex,
        character: V::Character,
        rarity: sts2_core::RewardRarity,
        drawn: &[V::Card],
    ) -> V::Card {
        use sts2_core::rng::GameRng as _;

        let pool = V::reward_cards(character, self.players.len() > 1).at(rarity);
        let left: Vec<V::Card> = pool
            .iter()
            .copied()
            .filter(|card| !drawn.contains(card))
            .collect();
        *self
            .streams
            .player(seat, V::PlayerRngType::REWARDS)
            .choose(&left)
            .unwrap_or_else(|_| unimplemented!("a reward pool with no {rarity:?} card left"))
    }

    /// The pools a `CardReward` over the character's own draws from, once
    /// `Hook.ModifyCardRewardCreationOptions` has had its say.
    ///
    /// # Panics
    ///
    /// A relic asking for anything but more pools.
    pub(super) fn reward_pool(
        &mut self,
        seat: sts2_core::SeatIndex,
        elite_encounter: bool,
    ) -> crate::reward::card::Pool<V> {
        use crate::reward::card::Source;

        let own = self.character(seat);
        match self.reward_hooks(seat, Source::Character(own), elite_encounter) {
            Hooked::Uniform(cards) => crate::reward::card::Pool::Uniform(cards),
            // `Except` drops a card the pools hold already, so a character
            // already among them adds nothing.
            Hooked::Pools { mut pools, concat } => {
                if let Some(character) = concat
                    && !pools.contains(&Source::Character(character))
                {
                    pools.push(Source::Character(character));
                }
                crate::reward::card::Pool::of(&pools, self.players.len() > 1)
            }
        }
    }

    /// The cards a reward of one rarity over `character`'s pool draws from,
    /// at even odds, once `Hook.ModifyCardRewardCreationOptions` has had its
    /// say. The pools hold the rarity alone. A character `Concat` added
    /// brings every rarity, since its cards skip the filter. `Uniform` then
    /// drops basic and ancient cards, and `Except` drops a repeat.
    ///
    /// # Panics
    ///
    /// A listener asking for rares at even odds, which only a won elite's
    /// reward asks.
    fn cards_of(
        &mut self,
        seat: sts2_core::SeatIndex,
        character: V::Character,
        rarity: sts2_core::RewardRarity,
    ) -> Vec<V::Card> {
        use crate::reward::card::Source;

        let Hooked::Pools { pools, concat } =
            self.reward_hooks(seat, Source::Character(character), false)
        else {
            unimplemented!("rares at even odds over a reward of one rarity");
        };
        let mut cards: Vec<V::Card> = pools
            .iter()
            .flat_map(|source| {
                source
                    .drafted(self.players.len() > 1)
                    .at(rarity)
                    .iter()
                    .copied()
            })
            .collect();
        if let Some(added) = concat {
            cards.extend(
                V::unlocked_cards(added, self.players.len() > 1)
                    .iter()
                    .copied()
                    .filter(|&card| V::reward_rarity(card) != sts2_core::RewardRarity::Other),
            );
        }
        let mut seen = Vec::with_capacity(cards.len());
        cards.retain(|&card| {
            let first = !seen.contains(&card);
            seen.push(card);
            first
        });
        cards
    }

    /// `Hook.ModifyCardRewardCreationOptions` over a card reward's options
    /// that draw from `base`.
    ///
    /// # Panics
    ///
    /// A listener asking for anything over a pool at even odds, or for
    /// something no card reward's options carry.
    fn reward_hooks(
        &mut self,
        seat: sts2_core::SeatIndex,
        base: crate::reward::card::Source<V>,
        elite_encounter: bool,
    ) -> Hooked<V> {
        use crate::reward::card::Source;

        let own = self.character(seat);
        let mut pools: SmallVec<[Source<V>; 6]> = smallvec::smallvec![base];
        let mut concat = None;
        let hook = sts2_core::EventHook::CardRewardCreationOptions {
            player: seat.creature(),
            elite_encounter,
        };
        let mut asked = self.notify_run(hook).into_iter();
        while let Some(effect) = asked.next() {
            match effect {
                // `WithCustomPool` of the rares the pools hold, or the
                // character's own, at even odds. Nothing after it is ported.
                sts2_core::HookEffect::RaresAtEvenOdds => {
                    if let Some(effect) = asked.next() {
                        unimplemented!("a listener asking for {effect:?} over a pool at even odds");
                    }
                    if let Some(character) = concat
                        && !pools.contains(&Source::Character(character))
                    {
                        pools.push(Source::Character(character));
                    }
                    let possible = crate::reward::card::Pool::of(&pools, self.players.len() > 1);
                    let mut cards = possible.drafted().rare.to_vec();
                    if cards.is_empty() {
                        cards = crate::reward::card::Pool::<V>::of(
                            &[Source::Character(own)],
                            self.players.len() > 1,
                        )
                        .drafted()
                        .rare
                        .to_vec();
                    }
                    return Hooked::Uniform(cards);
                }
                // `Union`: the new pools first, then what was there and is
                // not among them, in a build that kept it.
                sts2_core::HookEffect::AddCharacterPools(added) => {
                    if concat.is_some() {
                        unimplemented!("a pool joined to options that hold a custom pool");
                    }
                    let mut joined: SmallVec<[Source<V>; 6]> = added
                        .iter()
                        .map(|&character| Source::Character(character))
                        .collect();
                    if !V::card_pools_cleared_before_read() {
                        let rest: SmallVec<[Source<V>; 6]> = pools
                            .iter()
                            .copied()
                            .filter(|held| !joined.contains(held))
                            .collect();
                        joined.extend(rest);
                    }
                    pools = joined;
                }
                // `Concat` of the colorless pool, where it is not there.
                sts2_core::HookEffect::AddColorlessPool => {
                    if concat.is_some() {
                        unimplemented!("a pool joined to options that hold a custom pool");
                    }
                    if !pools.contains(&Source::Colorless) {
                        pools.push(Source::Colorless);
                    }
                }
                // `WithCustomPool` of the pools' cards, then the character's
                // unlocked cards, every rarity.
                sts2_core::HookEffect::ConcatCharacterPool(character) => {
                    if concat.is_some() {
                        unimplemented!("a second character's cards after a custom pool");
                    }
                    concat = Some(character);
                }
                // `Union`: the character's pool after the rest, where it is
                // not there. The reward's filter stays on it.
                sts2_core::HookEffect::UnionCharacterPool(character) => {
                    if concat.is_some() {
                        unimplemented!("a pool joined to options that hold a custom pool");
                    }
                    if !pools.contains(&Source::Character(character)) {
                        pools.push(Source::Character(character));
                    }
                }
                effect => {
                    unimplemented!("a relic asking for {effect:?} over a card reward's pools")
                }
            }
        }
        Hooked::Pools { pools, concat }
    }

    /// `CardFactory.CreateForReward`'s hooks that no relic here answers: the
    /// upgrade odds, and the first pass over the cards it made. The late pass
    /// is [`State::reward_options`].
    ///
    /// # Panics
    ///
    /// A relic listening to one of them.
    pub(super) fn card_reward_unheard(&mut self, seat: sts2_core::SeatIndex) {
        self.unheard(sts2_core::ModelHook::ModifyCardRewardUpgradeOdds);
        if let Some(effect) = self
            .notify_run_for(
                seat,
                sts2_core::EventHook::CardRewardOptions {
                    encounter: false,
                    from_combat: false,
                },
            )
            .first()
        {
            unimplemented!("a relic asking for {effect:?} over a card reward");
        }
    }

    /// `TryModifyCardRewardOptions` over a card reward at a room's odds: the
    /// kind of each option the belt adds. `from_combat` is `IsFromCombat`,
    /// which a won fight's own card reward carries and an extra one does not.
    pub(super) fn card_reward_added(
        &mut self,
        seat: sts2_core::SeatIndex,
        from_combat: bool,
    ) -> Vec<sts2_core::CardKind> {
        self.unheard(sts2_core::ModelHook::ModifyCardRewardUpgradeOdds);
        self.notify_run_for(
            seat,
            sts2_core::EventHook::CardRewardOptions {
                encounter: true,
                from_combat,
            },
        )
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
    /// Each listener here answers it where its own first answer was yes.
    /// `card_reward` is `CardCreationFlags.IsCardReward`, which only a
    /// `CardReward` sets.
    pub(super) fn reward_options(
        &mut self,
        seat: sts2_core::SeatIndex,
        cards: &mut [CardIdent<V>],
        card_reward: bool,
    ) {
        // Listener by listener, each over the whole reward: one relic's card
        // can be another's to enchant or not. Only `seat`'s own answer.
        let hook = sts2_core::ModelHook::TryModifyCardRewardOptionsLate;
        for listener in self.run_listed(hook).iter() {
            match listener {
                RunListener::Relic { seat: owner, slot } if owner == seat => {
                    self.reward_options_of(owner, slot, cards, card_reward)
                }
                RunListener::Relic { .. } => {}
                listener => unimplemented!("{listener:?} changing a card reward's options"),
            }
        }
        let modified = sts2_core::EventHook::CardRewardOptionsModified { card_reward };
        let asked = self.notify_run_for(seat, modified);
        if let Some(effect) = asked.first() {
            unimplemented!("a relic asking for {effect:?} once a card reward was modified");
        }
    }

    /// `CardReward.OnRelicObtained` for the relic in `slot`, over each card
    /// reward in `offered` not yet taken. The relic answers its own
    /// `TryModifyCardRewardOptions` and `TryModifyCardRewardOptionsLate`,
    /// then `AfterModifyingRewards` where they changed anything.
    fn reward_options_on_obtained(
        &mut self,
        seat: sts2_core::SeatIndex,
        slot: usize,
        offered: &mut [Offer<V>],
    ) {
        let hears = |state: &Self, hook| {
            state.belts[seat].hearing(sts2_core::Overrides::of(hook)) & (1 << slot) != 0
        };
        let early = hears(self, sts2_core::ModelHook::TryModifyCardRewardOptions);
        let late = hears(self, sts2_core::ModelHook::TryModifyCardRewardOptionsLate);
        for offer in offered {
            if let Offer::Card {
                cards,
                rolled,
                taken: false,
                ..
            } = offer
            {
                if early {
                    let (encounter, from_combat) = match rolled {
                        Some(crate::reward::Rolled::Room { from_combat, .. }) => {
                            (true, *from_combat)
                        }
                        _ => (false, false),
                    };
                    let mut asked = sts2_core::HookEffects::new();
                    let options = sts2_core::EventHook::CardRewardOptions {
                        encounter,
                        from_combat,
                    };
                    let relic_owner = self.relic_owner(seat);
                    let (state, belt) = self.belts[seat].state_beside(slot);
                    V::relic_event_hook(state, belt, relic_owner, options, &mut asked);
                    if let Some(effect) = asked.first() {
                        unimplemented!("a relic obtained over a card reward asking for {effect:?}");
                    }
                }
                if late {
                    self.reward_options_of(seat, slot, cards, true);
                }
            }
        }
        // `AfterModifyingRewards`, for a relic whose hooks changed the cards.
        // `Hook.AfterModifyingCardRewardOptions` is not raised here, so Silver
        // Crucible counts no use.
        if (early || late) && hears(self, sts2_core::ModelHook::AfterModifyingRewards) {
            unimplemented!(
                "a relic changing a card reward's options that hears AfterModifyingRewards"
            );
        }
    }

    /// `TryModifyCardRewardOptionsLate` of the relic in `slot` over each card
    /// a reward offers.
    fn reward_options_of(
        &mut self,
        owner: sts2_core::SeatIndex,
        slot: usize,
        cards: &mut [CardIdent<V>],
        card_reward: bool,
    ) {
        use sts2_core::rng::GameRng as _;

        for card in cards.iter_mut() {
            let mut asked = sts2_core::HookEffects::new();
            let option = sts2_core::EventHook::CardRewardOption {
                card: *card,
                card_reward,
            };
            let relic_owner = self.relic_owner(owner);
            let (state, belt) = self.belts[owner].state_beside(slot);
            V::relic_event_hook(state, belt, relic_owner, option, &mut asked);
            *card = rewrite(*card, asked);
        }
        let mut asked = sts2_core::HookEffects::new();
        let pick = sts2_core::EventHook::CardRewardPick { card_reward };
        let relic_owner = self.relic_owner(owner);
        let (state, belt) = self.belts[owner].state_beside(slot);
        V::relic_event_hook(state, belt, relic_owner, pick, &mut asked);
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

    /// [`State::reward_options`] over every card reward in `offered`.
    pub(super) fn reward_offers(&mut self, seat: sts2_core::SeatIndex, offered: &mut [Offer<V>]) {
        for offer in offered {
            if let Offer::Card { cards, .. } = offer {
                self.reward_options(seat, cards, true);
            }
        }
    }

    /// `card`, which is `seat`'s player's, after each answer to `hook`: a
    /// clone upgraded or enchanted, which takes its place. Each relic that
    /// answers compares the card's owner with its own, so `seat`'s listeners
    /// and the run's are asked.
    ///
    /// # Panics
    ///
    /// A relic asking for anything else.
    pub(super) fn rewritten(
        &mut self,
        seat: sts2_core::SeatIndex,
        card: CardIdent<V>,
        hook: impl Fn(CardIdent<V>) -> sts2_core::EventHook<V>,
    ) -> CardIdent<V> {
        rewrite(card, self.notify_run_for(seat, hook(card)))
    }

    /// Pick a relic up where nothing can park on what its pickup asks.
    ///
    /// Every room but the shop hands a relic over with no way to suspend the
    /// handover. A pickup that asks a question there stops by name rather than
    /// be answered by whoever happens to be holding the run.
    fn obtain_here(&mut self, seat: sts2_core::SeatIndex, relic: V::Relic) {
        if let Some(asked) = self.obtain(seat, relic) {
            let pickup = asked.pickup;
            unimplemented!("{relic:?} asking for {pickup:?} where it was picked up");
        }
    }

    /// `AncientEventModel.BeforeEventStarted`: `CreatureCmd.Heal` of what the
    /// run is missing, which a weary traveller gets four fifths of.
    pub(super) fn ancient_heal(&mut self, seat: sts2_core::SeatIndex) {
        let (hp, max_hp) = (self.players[seat].hp, self.players[seat].max_hp);
        self.heal(seat, ancient_heal::<V>(self.ascension(), hp, max_hp) - hp);
    }

    /// The ancient this act opens on, which it rolled with its rooms.
    pub(super) fn act_ancient(&self) -> V::Event {
        self.shared.opening.acts[self.act()].1.ancient
    }

    pub(super) fn event_candidates(&self) -> &[V::Event] {
        &self.shared.opening.acts[self.act()].1.events
    }

    pub(super) fn event_state(&self) -> (usize, &[V::Event], [f32; 4]) {
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

    /// `AfterRoomEntered` for a room no fight opens in. A shop parks on what
    /// its listeners ask, as Lord's Parasol's purchases do, and the walk's
    /// later listeners wait for the answer.
    ///
    /// # Panics
    ///
    /// A listener asking in any other room.
    fn room_entered(&mut self, room: V::RoomType, strayed: bool, point: Option<V::MapPointType>) {
        // Each listener reads its own player's numbers, which the walk fills
        // in. These are the host's, for the modifiers, which read none.
        let host = sts2_core::SeatIndex(0);
        let entered = sts2_core::EventHook::RoomEntered {
            room,
            strayed,
            point,
            deck: self.players[host].deck.len(),
            player: self.player_health(host),
            // A room no fight opens in is never one an event opened.
            base: true,
        };
        if let Some(asked) = self.walk_run(entered) {
            if !matches!(self.phase, Phase::Shop { .. }) {
                let pickup = asked.pickup;
                unimplemented!("entering a room asking for {pickup:?}");
            }
            self.park(host, asked);
        }
    }

    /// `RunState.CurrentRoom`'s `RoomType`, for a room no fight opens in.
    /// Nothing on the map, and in or after a fight, as
    /// [`Rules::computed_potion_check`] takes it.
    pub(super) fn quiet_room(&self) -> Option<V::RoomType> {
        use sts2_core::RoomKinds as _;

        match &self.phase {
            Phase::Neow { .. } | Phase::Event { .. } | Phase::Won { .. } => {
                Some(V::RoomType::EVENT)
            }
            Phase::Shop { .. } => Some(V::RoomType::SHOP),
            Phase::Treasure { .. } => Some(V::RoomType::TREASURE),
            Phase::RestSite { .. } => Some(V::RoomType::REST_SITE),
            Phase::Map | Phase::Combat(_) | Phase::Rewards { .. } | Phase::Lost { .. } => None,
        }
    }

    /// `FoulPotion.GetFoulPotionMerchantTarget` finding a merchant's button:
    /// a shop's, whose inventory a run with no screen never opens, or the
    /// one an event shows.
    pub(super) fn merchant(&self, seat: sts2_core::SeatIndex) -> bool {
        match &self.phase {
            Phase::Shop { .. } => true,
            Phase::Event { seats } => V::event_merchant(&seats[seat].state),
            _ => false,
        }
    }

    /// [`Rules::event_potion_thrown`] on the event the run stands in.
    pub(super) fn potion_thrown_at_event(
        &mut self,
        seat: sts2_core::SeatIndex,
        potion: V::Potion,
    ) -> sts2_core::HookEffects<V> {
        let Phase::Event { seats } = &mut self.phase else {
            unreachable!("only an event's merchant is thrown at")
        };
        V::event_potion_thrown(&mut seats[seat].state, potion)
    }

    /// The type of the map point at `at`. None off the map.
    pub(super) fn point_type(&self, at: Location) -> Option<V::MapPointType> {
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

    /// `EventSynchronizer.BeginEvent`: a copy of `event` for each player, in
    /// seat order. Each copy's stream is seeded with its player's slot.
    /// `floor` is the floor its room stands on.
    ///
    /// # Panics
    ///
    /// A shared event, or one that opens on a fight, in a run of several
    /// players.
    pub(super) fn opened_event(
        &mut self,
        event: V::Event,
        floor: V::Count,
        streams: &mut Streams<V>,
    ) -> Result<crate::seats::BySeat<EventSeat<V>>, Error> {
        use sts2_core::EventTable as _;

        if self.players.len() > 1 {
            if event.shared() {
                unimplemented!("{event:?}, which every player goes through together");
            }
            // `GenerateInternalCombatStateIfNecessary` rolls the local
            // player's alone.
            if event.combat_layout().is_some() {
                unimplemented!("{event:?}, which opens on a fight, in a run of several players");
            }
        }
        let mut seats = self.players.seats();
        let mut open = |seat: sts2_core::SeatIndex| {
            let slot = u64::from(seat.0);
            let mut rng = crate::event::stream::<V, MegaRng>(self.raw_seed(), slot, event);
            let (state, layout) = self.open_event(seat, event, floor, &mut rng, streams)?;
            Ok(EventSeat {
                state,
                rng,
                asking: None,
                finished: false,
                layout,
            })
        };
        let host = open(seats.next().expect("a run has a player"))?;
        let others = seats.map(open).collect::<Result<Vec<_>, Error>>()?;
        Ok(crate::seats::BySeat::new(host, others))
    }

    /// The fight a `Combat`-layout event builds, then `GenerateInitialOptions`:
    /// the event's own draws, what its options set up off the player's
    /// rewards stream, and the relics it pulls. `floor` is the floor its room
    /// stands on.
    pub(super) fn open_event(
        &mut self,
        seat: sts2_core::SeatIndex,
        event: V::Event,
        floor: V::Count,
        rng: &mut crate::rng::MegaRng,
        streams: &mut Streams<V>,
    ) -> Result<(V::EventState, Layout<V>), Error> {
        use sts2_core::EventTable as _;

        // `GenerateInternalCombatState`, before the options generate, on the
        // floor the room is about to stand on.
        let layout = match event.combat_layout() {
            Some(encounter) => crate::combat::roll_layout::<V>(
                self.raw_seed(),
                floor,
                self.ascension(),
                encounter,
                streams,
            )?,
            None => SmallVec::new(),
        };
        let mut state = V::enter_event(event, &self.seen_by(seat), rng);
        V::set_up_event(
            &mut state,
            &self.seen_by(seat),
            streams.player(seat, V::PlayerRngType::REWARDS),
        );
        let pulled: SmallVec<[V::Relic; 4]> = V::event_relics_pulled(&state)
            .iter()
            .map(|&pull| {
                let rarity = pulled_rarity(pull, streams.player(seat, V::PlayerRngType::REWARDS));
                self.pull_relic_as(seat, pull, rarity, floor)
            })
            .collect();
        if !pulled.is_empty() {
            V::hand_event_pulled_relics(&mut state, &pulled);
        }
        V::stock_event(&mut state, streams.player(seat, V::PlayerRngType::SHOPS));
        Ok((state, layout))
    }

    /// `AbstractRoom.IsVictoryRoom`: The Architect's room.
    fn in_victory_room(&self, seat: sts2_core::SeatIndex) -> bool {
        matches!(
            self.event_room(seat),
            Some(Room::Event { event, .. }) if event == V::the_architect()
        )
    }

    /// `EnterRoom` of The Architect's room, which `EnterNextAct` does past the
    /// last act. The room has no map point, so the run gains no floor and
    /// stands where it stood. `AfterRoomEntered` hears the point it stands on.
    fn enter_architect(&mut self, seat: sts2_core::SeatIndex) -> Result<Decided, Error> {
        let point = self
            .visited
            .last()
            .and_then(|&at| self.point_type(Location::MapPoint(at)));
        self.room_entering(seat, point, true);
        let event = V::the_architect();
        let mut streams = *self.streams();
        let seats = self.opened_event(event, self.floor, &mut streams)?;
        self.streams = streams;
        Arc::make_mut(&mut self.visited_events).push(event);
        // `ActModel.MarkRoomVisited`.
        self.events_used += 1;
        self.phase = Phase::Event { seats };
        self.room_entered(<V::RoomType as sts2_core::RoomKinds>::EVENT, false, point);
        Ok(Decided::Resolved)
    }

    pub(super) fn enter_event(&mut self, to: Coord, entered: EnteredEvent<V>) {
        let EnteredEvent {
            event,
            seats,
            streams,
            events_used,
            unknown_odds,
        } = entered;
        self.streams = streams;
        self.events_used = events_used;
        self.unknown_odds = unknown_odds;
        Arc::make_mut(&mut self.visited_events).push(event);
        self.enter(Location::MapPoint(to), Phase::Event { seats });
    }

    /// Enter a treasure room: `AfterRoomEntered`, then `BeginRelicPicking`.
    /// That deals a relic for each player whose `ShouldGenerateTreasure`
    /// holds. `RollRarity` rolls its rarity off the chest's own stream, and
    /// it comes off the front of the shared bag.
    pub(super) fn enter_treasure(&mut self, at: Location, streams: Streams<V>) {
        self.streams = streams;
        let votes = crate::seats::BySeat::new(None, self.seats().skip(1).map(|_| None).collect());
        self.enter(
            at,
            Phase::Treasure {
                relics: Vec::new(),
                votes,
            },
        );
        let mut relics = Vec::new();
        for seat in self.seats() {
            if !self.generates_treasure(seat) {
                continue;
            }
            let rarity = crate::reward::relic_rarity::<V, _>(
                self.streams.run(V::RunRngType::TREASURE_ROOM_RELICS),
            );
            // `SharedRelicGrabBag.PullFromFront`, which leaves the player's
            // bag holding it. Taking it from the chest obtains it, which
            // takes it out of both.
            relics.push(self.deal_shared_relic(rarity));
        }
        if let Phase::Treasure { relics: chest, .. } = &mut self.phase {
            *chest = relics;
        }
    }

    /// `PickRelicAction`: `seat`'s vote for the relic at `index`, or for none
    /// one past the last. The chest opens first, as answering it is how the
    /// player gets at it. `OnPicked` awards the relics once every player has
    /// voted. A skip in a run of one returns before it records a vote,
    /// which leaves the relics showing.
    ///
    /// # Panics
    ///
    /// A vote from a player who voted already, or for a relic not offered.
    fn pick_relic(&mut self, seat: sts2_core::SeatIndex, index: u8) {
        if self.phase.finished(seat) {
            unimplemented!("choosing from a finished treasure room");
        }
        self.open_chest(seat);
        let players = self.player_count();
        let Phase::Treasure { relics, votes } = &mut self.phase else {
            unreachable!("a relic is picked in a treasure room");
        };
        let vote = match usize::from(index) {
            at if at < relics.len() => Some(index),
            at if at == relics.len() => None,
            _ => unimplemented!("an option the treasure room does not offer"),
        };
        votes[seat] = Some(vote);
        if vote.is_none() && players == 1 {
            return;
        }
        if votes.any(|vote| vote.is_none()) {
            return;
        }
        let relics = std::mem::take(relics);
        let votes: Vec<Option<u8>> = votes.seated().map(|(_, vote)| vote.flatten()).collect();
        let awarded = super::treasure::awarded::<V>(
            &relics,
            &votes,
            self.streams.run(V::RunRngType::TREASURE_ROOM_RELICS),
        );
        // `NTreasureRoomRelicCollection.AnimateRelicAwards`: each result's
        // relic obtained, and moved to the fallback of every other player's
        // bag.
        for (winner, relic) in awarded {
            if let Some(winner) = winner {
                self.obtain_here(winner, relic);
            }
            for other in self.seats().filter(|&other| Some(other) != winner) {
                Arc::make_mut(&mut self.players[other].dealt_relics).push(relic);
            }
        }
    }

    /// `NTreasureRoom.OpenChest`'s `DoNormalRewards`, which runs as the
    /// player first answers the chest: `OneOffSynchronizer.
    /// DoTreasureRoomRewards`. Where `ShouldGenerateTreasure` lets it, that is
    /// `NextInt(42, 53)` gold off the player's Rewards stream, three quarters
    /// of it under Poverty, then `TryHandleSpoilsMap`.
    fn open_chest(&mut self, seat: sts2_core::SeatIndex) {
        use sts2_core::rng::GameRng as _;
        use sts2_core::{AscensionKinds as _, PlayerStreamKinds as _};

        if !self.generates_treasure(seat) {
            return;
        }
        let span = sts2_core::rng::Span::new(CHEST_GOLD.0, CHEST_GOLD.1)
            .expect("the chest's gold is a span");
        let rolled = self
            .streams
            .player(seat, V::PlayerRngType::REWARDS)
            .next_int_range(span);
        let mut gold = f64::from(rolled);
        if self
            .ascension()
            .has(<V::AscensionLevel as sts2_core::AscensionKinds>::POVERTY)
        {
            gold *= POVERTY_GOLD;
        }
        // `(int)gold`, which truncates.
        self.gain_gold(
            seat,
            num_traits::cast(gold.trunc()).expect("a chest's gold is a game number"),
        );
        self.spoils_paid(seat);
    }

    /// `TryHandleSpoilsMap`, where the run stands on the Spoils Map's quest
    /// point. Each Spoils Map in the deck pays its gold and leaves the deck
    /// through `OnQuestComplete`, and the quest is done.
    fn spoils_paid(&mut self, seat: sts2_core::SeatIndex) {
        let (Some(&at), Some((point, card, gold))) = (self.visited.last(), self.spoils) else {
            return;
        };
        if at != point {
            return;
        }
        let paying: Vec<usize> = (self.players[seat].deck.iter().enumerate())
            .filter(|(_, held)| held.ident.card == card)
            .map(|(at, _)| at)
            .collect();
        for &at in paying.iter().rev() {
            self.gain_gold(seat, gold);
            self.players[seat].deck_mut().remove(at);
        }
        self.spoils = None;
    }

    /// `Hook.ShouldGenerateTreasure` for `seat`'s player, which a relic of
    /// theirs or a modifier can answer no to. Silver Crucible, the one relic
    /// that answers, says yes for another player.
    fn generates_treasure(&self, seat: sts2_core::SeatIndex) -> bool {
        let hook = sts2_core::DecisionHook::ShouldGenerateTreasure;
        let model_hook = sts2_core::ModelHook::ShouldGenerateTreasure;
        let mut generates = true;
        self.run_each(model_hook, |listener, _| {
            if let sts2_core::Listener::Relic { owner, .. } = listener
                && owner != seat.creature()
            {
                return;
            }
            generates &= !V::decide_hook(listener, hook).is_decision_and(|generate| !generate);
        });
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
    pub(crate) fn buy(&mut self, seat: sts2_core::SeatIndex, index: u8) -> Result<Decided, Error> {
        if let Phase::Event { .. } = self.phase {
            return Ok(self.buy_in_event(seat, index));
        }
        let Phase::Shop { seats } = &self.phase else {
            unimplemented!("buying outside a shop");
        };
        let Some(entry) = seats[seat]
            .entries
            .iter()
            .find(|entry| entry.index == index)
        else {
            unimplemented!("buying a shelf this merchant does not have");
        };
        if !entry.stocked {
            unimplemented!("buying a shelf this merchant has sold");
        }
        let (stock, cost) = (entry.stock, self.merchant_price(seat, entry.cost));
        if cost > self.players[seat].gold {
            unimplemented!("buying a shelf this run cannot afford");
        }
        let spent: u32 = num_traits::cast(cost).expect("a price is no less than nothing");

        // `PlayerCmd.LoseGold` runs after the entry's own purchase in the
        // game. Nothing between the two reads the gold, so it is spent here,
        // and handed back by a purchase that fails.
        self.players[seat].gold = self.players[seat].gold - cost;
        match stock {
            sts2_core::Stock::Card(Some(on_sale)) => self.add_to_deck(seat, on_sale.card),
            // `AfterObtained` awaits the pickup, as any room's does, and
            // `ClearAfterPurchase` runs once the entry's own purchase returns.
            // So a shelf whose purchase is still asking stays stocked until it
            // does. A purchase that has not returned leaves the decision
            // unresolved, which is what the game reports.
            sts2_core::Stock::Relic(Some(relic)) => {
                let mut steps = self.obtained_held(seat, relic, false, None);
                steps.push(Then::Purchased {
                    shelf: index,
                    spent,
                    parasol: false,
                });
                return Ok(match self.carry_out(seat, steps) {
                    Some(asked) => self.park(seat, asked),
                    None => Decided::Resolved,
                });
            }
            // `TryToProcure` refuses where a relic's `ShouldProcurePotion`
            // says no, and the purchase fails with it. Nothing is spent and
            // the shelf keeps its potion. A full belt is refused before any
            // purchase, as the game's own check is.
            sts2_core::Stock::Potion(Some(potion)) => {
                if !self.procure(seat, potion) {
                    self.players[seat].gold = self.players[seat].gold + cost;
                    return Ok(Decided::Resolved);
                }
            }
            // `DoLocalMerchantCardRemoval` opens a cancelable selection and
            // spends the gold once a card comes back, so nothing is spent on
            // this one until then.
            sts2_core::Stock::CardRemoval => {
                self.players[seat].gold = self.players[seat].gold + cost;
                let removal = Then::Ask(Pickup::CardRemoval { free: false });
                return Ok(match self.carry_out(seat, [removal]) {
                    Some(asked) => self.park(seat, asked),
                    None => Decided::Resolved,
                });
            }
            stock => unimplemented!("buying {stock:?}"),
        }
        self.purchased(seat, index, cost);
        Ok(Decided::Resolved)
    }

    /// A purchase from the shop an event's room is, which the event carries
    /// out.
    ///
    /// # Panics
    ///
    /// An entry the room does not sell or the run cannot afford.
    fn buy_in_event(&mut self, seat: sts2_core::SeatIndex, index: u8) -> Decided {
        let Some(Room::Shop { entries }) = self.event_room(seat) else {
            unimplemented!("buying in an event that is not a shop");
        };
        let Some(entry) = entries.iter().find(|entry| entry.index == index) else {
            unimplemented!("buying a shelf this event does not have");
        };
        if !entry.stocked || !entry.affordable {
            unimplemented!("buying a shelf this event cannot sell");
        }
        let Phase::Event { seats } = &mut self.phase else {
            unreachable!("the phase was an event when this began");
        };
        let effects = V::buy_in_event(&mut seats[seat].state, index);
        match self.carry_out(seat, effects) {
            Some(next) => self.park(seat, next),
            None => Decided::Resolved,
        }
    }

    /// `CloneCard`, then `CardPileCmd.Add` to the deck.
    ///
    /// The selection offers the deck less what the relic's filter names, so the
    /// index is into that narrowed list rather than into the deck.
    fn cloned(&mut self, seat: sts2_core::SeatIndex, index: usize, excluding: sts2_core::CardKind) {
        let offered = self.clonable(seat, excluding);
        let Some(card) = offered.get(index).copied() else {
            unimplemented!("a clone naming a card that was not offered");
        };
        // `CloneCard`, which keeps what the card's model saved.
        let card = sts2_core::DeckCard {
            id: self.players[seat].mint(),
            ..card
        };
        self.add_deck_card(seat, card);
    }

    /// Take the card the merchant's removal was pointed at out of the deck.
    /// A `free` removal spends nothing, and counts among the removals used.
    ///
    /// The index is the removal list's, which is the deck less its `Eternal`
    /// cards, curses first.
    fn removed(&mut self, seat: sts2_core::SeatIndex, index: usize, free: bool) {
        let Some(&(at, _)) = self.offered(seat, sts2_core::DeckAct::Remove).get(index) else {
            unimplemented!("a removal naming a card the selection did not offer");
        };
        self.players[seat].deck_mut().remove(at);
        // `DoLocalMerchantCardRemoval` spends the gold once the card comes
        // back, and `CardShopRemovalsUsed` climbs over the run.
        let spent = match free {
            true => num_traits::zero(),
            false => self.merchant_price(
                seat,
                removal_cost::<V>(self.players[seat].removals_used, self.ascension()),
            ),
        };
        self.players[seat].gold = self.players[seat].gold - spent;
        self.players[seat].removals_used += 1;
        // `NMerchantCardRemoval.OnCardRemovalUsed` calls `SetUsed`, and
        // `IsStocked` is `!Used`, so this merchant sells no second removal.
        // The next merchant reads the raised count.
        if let Phase::Shop { seats } = &mut self.phase {
            for entry in seats[seat]
                .entries
                .iter_mut()
                .filter(|entry| entry.stock == sts2_core::Stock::CardRemoval)
            {
                entry.stocked = false;
            }
        }
        self.item_purchased(seat, spent);
    }

    /// `LordsParasol.PurchaseEverything`, as its owner enters a merchant.
    /// `OnTryPurchaseWrapper` with `ignoreCost` on each shelf, in order. A
    /// sold-out shelf or a refused potion buys nothing. The card removal comes
    /// last, and cannot be cancelled.
    ///
    /// A relic's pickup that asks stands the rest still. They go on from the
    /// shelf `after` it once the pickup is done.
    fn purchase_everything(&mut self, seat: sts2_core::SeatIndex, after: Option<u8>) -> Steps<V> {
        let Phase::Shop { seats } = &self.phase else {
            unimplemented!("Lord's Parasol buying outside a merchant's room");
        };
        let shelves: Vec<(u8, sts2_core::Stock<V>, bool)> = seats[seat]
            .entries
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
                sts2_core::Stock::Card(Some(on_sale)) => self.add_to_deck(seat, on_sale.card),
                // Its pickup, then the purchases from the next shelf.
                sts2_core::Stock::Relic(Some(relic)) => {
                    let mut steps = self.obtained_held(seat, relic, false, None);
                    steps.push(Then::Purchased {
                        shelf: index,
                        spent: 0,
                        parasol: true,
                    });
                    return steps;
                }
                sts2_core::Stock::Potion(Some(potion)) => {
                    if !self.procure(seat, potion) {
                        continue;
                    }
                }
                stock => unimplemented!("Lord's Parasol buying {stock:?}"),
            }
            // `ignoreCost`, which spends nothing.
            self.purchased(seat, index, num_traits::zero());
        }
        match removal {
            true => vec![Then::Ask(Pickup::CardRemoval { free: true })],
            false => Vec::new(),
        }
    }

    /// The deck as a pickup's selection offers it.
    fn clonable(
        &self,
        seat: sts2_core::SeatIndex,
        excluding: sts2_core::CardKind,
    ) -> Vec<sts2_core::DeckCard<V>> {
        self.players[seat]
            .deck
            .iter()
            .filter(|held| V::card_kind(held.ident.card) != Some(excluding))
            .copied()
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
    pub(crate) fn enter_room(
        &mut self,
        seat: sts2_core::SeatIndex,
        room: V::RoomType,
    ) -> Result<Decided, Error> {
        self.room_entering(seat, self.point_type(Location::NoLocation), true);
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

    /// `Hook.ModifyMerchantCardPool` over a character shelf: the character
    /// whose cards a listener adds after the shelf's own. Every card the
    /// merchant stocks asks it, and the answer is the same each time.
    ///
    /// # Panics
    ///
    /// A listener asking for anything else, or for two characters.
    fn merchant_card_pool(&mut self) -> Option<V::Character> {
        let mut added = None;
        for effect in self.notify_run(sts2_core::EventHook::MerchantCardPool) {
            match effect {
                sts2_core::HookEffect::ConcatCharacterPool(character) if added.is_none() => {
                    added = Some(character);
                }
                effect => {
                    unimplemented!("a listener asking for {effect:?} over a merchant's cards")
                }
            }
        }
        added
    }

    /// `MerchantRoom`, which runs `MerchantInventory.CreateForNormalMerchant`
    /// for each player in seat order as the room opens. So every draw it
    /// takes is paid for on the way in, not at a shelf.
    ///
    /// Built rather than entered, because what a merchant stocks does not
    /// depend on how the run reached it.
    pub(super) fn stocked_shop(&mut self) -> Phase<V> {
        let mut seats = self.players.seats();
        let mut stocked = |seat| ShopSeat {
            entries: self.inventory(seat),
            asking: None,
        };
        let host = stocked(seats.next().expect("a run has a player"));
        let others = seats.map(stocked).collect();
        Phase::Shop {
            seats: crate::seats::BySeat::new(host, others),
        }
    }

    /// `MerchantInventory.CreateForNormalMerchant` for `seat`'s player.
    fn inventory(&mut self, seat: sts2_core::SeatIndex) -> Vec<sts2_core::Entry<V>> {
        let added = self.merchant_card_pool();
        let mut streams = self.streams;
        let mut dealt = self.players[seat].dealt_relics.to_vec();
        let stocked_from = dealt.len();
        let entries = super::shop::Merchant::<V> {
            streams: &mut streams,
            player_relics: &self.shared.opening.player_relics[seat.at()],
            dealt_relics: &mut dealt,
            // Stocked as the room is built, which `EnterRoom` does once the
            // room's floor counts.
            floor: num_traits::cast(self.floor + num_traits::one()).expect("a floor fits a usize"),
            character: self.players[seat].character,
            players: self.players.len(),
            added,
            offset: self.players[seat].odds.rarity,
            ascension: self.ascension(),
            removals_used: self.players[seat].removals_used,
        }
        .stock(seat);

        self.streams = streams;
        // `PullNextRelicFromBack` takes what it stocks out of the shared bag
        // too.
        Arc::make_mut(&mut self.shared_dealt).extend_from_slice(&dealt[stocked_from..]);
        self.players[seat].dealt_relics = Arc::new(dealt);
        // `Hook.ModifyMerchantCardCreationResults` over the cards it stocked.
        let entries = entries
            .into_iter()
            .map(|mut entry| {
                if let sts2_core::Stock::Card(Some(on_sale)) = &mut entry.stock {
                    on_sale.card = self.rewritten(seat, on_sale.card, |card| {
                        sts2_core::EventHook::MerchantCardCreated { card }
                    });
                    // What it costs to play, once the hook upgraded it.
                    (on_sale.cost, on_sale.star_cost) = crate::combat::printed_costs(on_sale.card);
                }
                entry
            })
            .collect::<Vec<_>>();
        // `NMerchantCardRemoval.FillSlot` marks the removal used where
        // `ShouldAllowMerchantCardRemoval` says no, and `IsStocked` is
        // `!Used`.
        let mut entries = entries;
        if !self.card_removal_allowed() {
            for entry in
                (entries.iter_mut()).filter(|entry| entry.stock == sts2_core::Stock::CardRemoval)
            {
                entry.stocked = false;
            }
        }
        entries
    }

    /// Enter a rest site, with the options this run's belt and deck open it on.
    pub(super) fn enter_rest_site(&mut self, at: Location) {
        let mut seats = self.players.seats();
        let host = RestSeat::offering(self.rest_options(seats.next().expect("a run has a player")));
        let others = seats
            .map(|seat| RestSeat::offering(self.rest_options(seat)))
            .collect();
        let seats = crate::seats::BySeat::new(host, others);
        self.enter(at, Phase::RestSite { seats });
    }

    /// `RestSiteOption.Generate` for `seat`'s player: Heal, Smith, and Mend
    /// in a run of several players. Then `TryModifyRestSiteOptions`, which
    /// the deck answers first, then the belt, then the modifiers. Each relic
    /// compares the player with its owner.
    fn rest_options(&mut self, seat: sts2_core::SeatIndex) -> Vec<V::RestSiteOption> {
        let mut added: Vec<V::RestSiteOption> = Vec::new();
        let mut withdrawn: Vec<V::RestSiteOption> = Vec::new();
        for effect in self.notify_run_for(seat, sts2_core::EventHook::RestSiteOptions) {
            match effect {
                sts2_core::HookEffect::OfferRestOption(option) => added.push(option),
                sts2_core::HookEffect::WithdrawRestOption(option) => withdrawn.push(option),
                effect => unimplemented!("a listener asking for {effect:?} as a rest site opens"),
            }
        }
        let mut options =
            <V::RestSiteOption as sts2_core::RestKinds<V>>::offered(self.players.len(), &added);
        options.retain(|option| !withdrawn.contains(option));
        options
    }

    /// Enter a fight that is already built, where the run reached it.
    ///
    /// The act counts the fight here rather than where it was built, because
    /// the count is how far down its list the run has walked. A fight the run
    /// never walked into moves no list on.
    pub(super) fn enter_fight(
        &mut self,
        seat: sts2_core::SeatIndex,
        fight: CreatedFight<V>,
        at: Location,
    ) -> Result<Decided, Error> {
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
        let ended = match ended {
            None => self.turn_ended_by_play(seat)?,
            ended => ended,
        };
        // A fight its opening ended goes straight on, the way one a decision
        // ended does.
        Ok(self.settle(seat, ended, 0))
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
        // to an event alone, and a relic can take a room type out of. The
        // roll still draws.
        let mut narrowed = false;
        let mut removed: SmallVec<[V::RoomType; 2]> = SmallVec::new();
        for effect in self.notify_run(sts2_core::EventHook::UnknownRoomTypes { act: self.act }) {
            match effect {
                sts2_core::HookEffect::EventRoomsOnly => narrowed = true,
                sts2_core::HookEffect::RemoveRoomType(room) => removed.push(room),
                effect => unimplemented!("a relic asking for {effect:?} as an unknown room rolls"),
            }
        }
        let removes = |room| removed.contains(&room);
        let node = |at| self.map.at(at).expect("a map edge leads to a node");
        let after = &node(to).to;
        let all_shops = !after.is_empty()
            && after
                .iter()
                .all(|&at| node(at).kind == V::MapPointType::SHOP);
        // The room the run leaves is the one it still stands in.
        let in_shop = matches!(self.phase, Phase::Shop { .. });
        use sts2_core::RoomKinds as _;
        let types = [
            V::RoomType::MONSTER,
            V::RoomType::ELITE,
            V::RoomType::TREASURE,
            V::RoomType::SHOP,
        ];
        let barred = [
            narrowed || removes(types[0]),
            narrowed || removes(types[1]),
            narrowed || removes(types[2]),
            narrowed || in_shop || all_shops || removes(types[3]),
        ];
        let rooms = [
            UnknownRoom::Monster,
            UnknownRoom::Elite,
            UnknownRoom::Treasure,
            UnknownRoom::Shop,
        ];
        let roll = streams.run(V::RunRngType::UNKNOWN_MAP_POINT).next_double() as f32;
        let mut cumulative = 0.0;
        // A room with negative odds never rolls.
        let rolled = (0..rooms.len()).find(|&at| {
            if barred[at] || self.unknown_odds[at] < 0.0 {
                return false;
            }
            cumulative += self.unknown_odds[at];
            roll <= cumulative
        });
        let base = self.shared.unknown_base;
        for at in 0..rooms.len() {
            if Some(at) == rolled {
                self.unknown_odds[at] = base[at];
            } else if !barred[at] {
                let rise = self.odds_increase(types[at], base[at]);
                self.unknown_odds[at] += rise;
            }
        }
        rolled.map(|at| rooms[at])
    }

    /// `Hook.ModifyOddsIncreaseForUnrolledRoomType` over `base`, how far an
    /// unknown room's odds of being `room` rise when it rolls something
    /// else. Each listener answers it in turn.
    ///
    /// # Panics
    ///
    /// A listener answering other than by scaling.
    fn odds_increase(&self, room: V::RoomType, base: f32) -> f32 {
        let hook = sts2_core::ModifyHook::OddsIncreaseForUnrolledRoomType { room };
        let mut rise = base;
        self.run_each(hook.model_hook(), |listener, _| {
            match V::modify_hook(listener, hook) {
                sts2_core::Modify::Nothing => {}
                sts2_core::Modify::Scaled(by) => {
                    rise *= num_traits::ToPrimitive::to_f32(&by.exact())
                        .expect("an odds scale fits a float");
                }
                answer => unimplemented!("a listener answering {hook:?} with {answer:?}"),
            }
        });
        rise
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
    pub(super) fn positions(&self, seat: sts2_core::SeatIndex) -> Positions<V> {
        self.streams.positions(seat)
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
        self.build_fight(encounter, queue, at, floor, &[], &[], true)
    }

    /// [`State::create_fight`] on a floor the caller names. `base` is whether
    /// the fight's room is the run's `BaseRoom`, which a fight an event opens
    /// inside its own room is not.
    #[allow(clippy::too_many_arguments)]
    fn build_fight(
        &mut self,
        encounter: V::Encounter,
        queue: EncounterQueue,
        at: Location,
        floor: V::Count,
        rolled: &[V::Health],
        roster: &[V::Monster],
        base: bool,
    ) -> Result<CreatedFight<V>, Error> {
        let (raw_seed, ascension) = (self.raw_seed(), self.ascension());
        // A fight an event opens moves the run nowhere.
        let (strayed, point) = (base && self.strays(at), self.point_type(at));
        let standing: Vec<_> = (0..self.players.len())
            .map(|at| {
                self.standing(sts2_core::SeatIndex(
                    u8::try_from(at).expect("a run's players fit a u8"),
                ))
            })
            .collect();
        let players: Vec<crate::combat::EnteringPlayer<'_, V>> = self
            .players
            .seated()
            .zip(&standing)
            .map(|((_, player), standing)| crate::combat::EnteringPlayer {
                character: player.character,
                hp: standing.hp,
                max_hp: standing.max_hp,
                gold: player.gold,
                potions: &player.potions,
                alive: standing.alive,
                deck: &player.deck,
            })
            .collect();
        let entering = crate::combat::Entering {
            floor,
            act: self.act,
            modifiers: &self.shared.modifiers,
            strayed,
            point,
            coord: match at {
                Location::MapPoint(coord) => Some(coord),
                _ => None,
            },
            rolled,
            roster,
            base,
        };
        let crate::combat::Entered {
            live,
            streams,
            ended,
            arriving,
        } = crate::combat::enter::<V>(
            raw_seed,
            ascension,
            encounter,
            &entering,
            &players,
            &self.streams,
            &mut self.belts,
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
    /// room stack is two deep, so `MarkRoomVisited` counts nothing.
    ///
    /// The fight is won to a monster room's rewards, then `rewards`, then
    /// any `own_rewards`. The run then travels on, or with `resume` goes back
    /// to the event.
    pub(super) fn fight_in_event(
        &mut self,
        seat: sts2_core::SeatIndex,
        encounter: V::Encounter,
        rewards: &[sts2_core::CustomReward<V>],
        roster: &[V::Monster],
        resume: bool,
        own_rewards: bool,
    ) -> Result<Decided, Error> {
        let own = match (own_rewards, &self.phase) {
            (true, Phase::Event { seats }) => V::event_rewards(&seats[seat].state),
            (true, _) => unreachable!("only an event has rewards of its own"),
            (false, _) => Vec::new(),
        };
        let CreatedFight {
            live,
            streams,
            ended,
            arriving,
            ..
        } = {
            let rolled = match &self.phase {
                Phase::Event { seats } => seats[seat].layout.clone(),
                _ => SmallVec::new(),
            };
            // `CurrentMapPoint` is still the event's, which `AfterRoomEntered`
            // reads as the fight's room opens.
            let at = self
                .visited
                .last()
                .map_or(Location::NoLocation, |&point| Location::MapPoint(point));
            // The fight's room stacks on the event's.
            self.room_entering(seat, self.point_type(at), false);
            self.build_fight(
                encounter,
                EncounterQueue::Named,
                at,
                self.floor,
                &rolled,
                roster,
                false,
            )?
        };
        let mut live = live;
        // `AddExtraReward`, as the room is built and before the fight opens.
        for &reward in rewards.iter().chain(&own) {
            live.add_extra_reward(match reward {
                sts2_core::CustomReward::SpecialCard(card) => {
                    crate::combat::ExtraReward::SpecialCard(CardIdent::new(card))
                }
                sts2_core::CustomReward::RolledRelic => crate::combat::ExtraReward::Relic,
                sts2_core::CustomReward::SpecificRelic(relic) => {
                    crate::combat::ExtraReward::SpecificRelic(relic)
                }
                sts2_core::CustomReward::Potion => crate::combat::ExtraReward::Potion,
                reward => unimplemented!("{reward:?} among a fight's extra rewards"),
            });
        }
        self.drew(streams);
        let event = std::mem::replace(&mut self.phase, Phase::Combat(live));
        if resume {
            self.resuming = Some(Box::new(Resuming {
                event,
                escaped: false,
                extras: None,
            }));
        }
        self.arrived(arriving);
        let ended = match ended {
            None => self.turn_ended_by_play(seat)?,
            ended => ended,
        };
        Ok(self.settle(seat, ended, 0))
    }

    /// Carry out what a fight's opening asked of the run, which asks nothing
    /// back.
    fn arrived(&mut self, arriving: Vec<(sts2_core::SeatIndex, sts2_core::HookEffect<V>)>) {
        for (owner, effect) in arriving {
            if let Some(asked) = self.carry_out(owner, [effect]) {
                let pickup = asked.pickup;
                unimplemented!("a fight's opening asking for {pickup:?}");
            }
        }
    }

    /// Put the run into a named fight, rather than travel to one.
    ///
    /// `FightConsoleCmd`, which is `EnterRoomDebug` at
    /// `MapPointType.Unassigned`. The floor advances and the map does not, so
    /// map and room disagree by construction. That is what the command is for:
    /// reaching a chosen fight without hunting for a seed that puts one on the
    /// way. The encounter is named, so the act's own list is not drawn from.
    pub(crate) fn fight_here(
        &mut self,
        seat: sts2_core::SeatIndex,
        encounter: V::Encounter,
    ) -> Result<Decided, Error> {
        self.room_entering(seat, self.point_type(Location::NoLocation), true);
        let fight = self.create_fight(encounter, EncounterQueue::Named, Location::NoLocation)?;
        self.enter_fight(seat, fight, Location::NoLocation)
    }

    /// Enter a named event, off the map.
    ///
    /// `EventConsoleCmd`: a map point appended to the history, then
    /// `RunManager.EnterRoom` with the event's room. That seeds the event's
    /// own stream, and counts it against the act the way travelling does.
    pub(crate) fn event_here(
        &mut self,
        seat: sts2_core::SeatIndex,
        event: V::Event,
    ) -> Result<Decided, Error> {
        // The console lists no Architect, and the mod enters its room the way
        // the run does.
        if event == V::the_architect() {
            return self.enter_architect(seat);
        }
        self.room_entering(seat, self.point_type(Location::NoLocation), true);
        let mut streams = *self.streams();
        let floor = self.floor + num_traits::one();
        let seats = self.opened_event(event, floor, &mut streams)?;
        self.streams = streams;
        Arc::make_mut(&mut self.visited_events).push(event);
        self.enter(Location::NoLocation, Phase::Event { seats });
        Ok(Decided::Resolved)
    }

    /// Put a power on a creature in the fight.
    ///
    /// `ApplyPowerConsoleCmd`, which applies it through the command a card
    /// applies one with. So a debuff put on the player here skips its next
    /// tick, exactly as one a monster applied does.
    pub(crate) fn apply_power(
        &mut self,
        seat: sts2_core::SeatIndex,
        at: sts2_core::CombatId,
        power: V::Power,
        amount: sts2_core::PowerAmount<V::Number>,
    ) -> Result<Decided, Error> {
        let Some(fighting) = self.fighting() else {
            unimplemented!("a power applied outside a fight");
        };
        crate::combat::apply_power(seat, fighting, at, power, amount);
        Ok(Decided::Resolved)
    }

    /// Set the hit points of the player, or of the creature `at` in the
    /// fight, which is `CreatureCmd.SetCurrentHp`.
    ///
    /// In a fight it raises `AfterCurrentHpChanged` there. Outside one the
    /// run's relics hear it, as they hear damage outside a fight.
    pub(crate) fn set_hp(
        &mut self,
        seat: sts2_core::SeatIndex,
        at: Option<sts2_core::CombatId>,
        hp: V::Health,
    ) -> Result<Decided, Error> {
        if let Some(fighting) = self.fighting() {
            let at = at.unwrap_or(seat.creature());
            crate::combat::set_hp(seat, fighting, at, hp);
            return Ok(Decided::Resolved);
        }
        if at.is_some() {
            unimplemented!("hit points set on a creature outside a fight");
        }
        let before = self.players[seat].hp;
        self.players[seat].hp = hp.min(self.players[seat].max_hp);
        if self.players[seat].hp != before {
            let delta = num_traits::cast::<_, V::Number>(self.players[seat].hp)
                .expect("a health fits a game number")
                - num_traits::cast::<_, V::Number>(before).expect("a health fits a game number");
            let hook = sts2_core::EventHook::CurrentHpChanged {
                creature: seat.creature(),
                monster: None,
                pet_of: None,
                delta,
                in_fight: false,
            };
            if let Some(effect) = self.notify_run(hook).first() {
                unimplemented!(
                    "a relic asking for {effect:?} as hit points are set outside a fight"
                );
            }
        }
        Ok(Decided::Resolved)
    }

    /// Enchant the fight's copy of the card at `index` in hand, which is
    /// `EnchantConsoleCmd`.
    pub(crate) fn enchant(
        &mut self,
        seat: sts2_core::SeatIndex,
        enchantment: V::Enchantment,
        amount: u8,
        index: sts2_core::HandIndex,
    ) -> Result<super::Settled<V>, Error> {
        let Some(fighting) = self.fighting() else {
            unimplemented!("an enchantment put on a card outside a fight");
        };
        Ok(
            crate::combat::enchant(seat, fighting, enchantment, amount, index)
                .map(|()| Decided::Resolved)
                .map_err(|reason| sts2_core::Refusal {
                    what: "enchant".into(),
                    reason,
                }),
        )
    }

    /// Put a card, a relic or a potion straight into the run.
    ///
    /// `Driver.GrantCard`, `RelicCmd.Obtain` and `PotionCmd.TryToProcure`,
    /// which are the paths ordinary play uses. A relic arrives through the
    /// command that runs its `AfterObtained`. The simulator runs no relic's
    /// hooks, so it cannot answer for one.
    pub(crate) fn grant(
        &mut self,
        seat: sts2_core::SeatIndex,
        gift: &Gift<V>,
    ) -> Result<Decided, Error> {
        match gift {
            // `GrantCard` in a fight in progress: the fight's own card, into
            // hand, and the deck as it was.
            Gift::Card(card) => match self.fighting() {
                Some(fighting) => crate::combat::grant_card(seat, fighting, *card),
                None => self.add_to_deck(seat, CardIdent::new(*card)),
            },
            Gift::Potion(potion) => {
                // `TryToProcure` refuses a full belt rather than growing one,
                // and the belt in the next observation says whether it landed.
                self.procure(seat, *potion);
            }
            // `RelicCmd.Obtain`, whose pickup asks where the room can park on
            // it, as Neow's can.
            Gift::Relic(relic) => {
                if self.phase.asking_mut(seat).is_some() {
                    return Ok(match self.obtain(seat, *relic) {
                        None => Decided::Resolved,
                        Some(asked) => self.park(seat, asked),
                    });
                }
                self.obtain_here(seat, *relic)
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

    /// `RoomSet.NextBossEncounter`: the boss an act rolled, or its second
    /// once a boss room is behind the run in this act.
    pub(super) fn boss_encounter(&self, act: usize) -> Option<V::Encounter> {
        use sts2_core::{ActMap as _, MapPointKinds as _};

        let (_, rooms) = self.shared.opening.acts.get(act)?;
        let fought = self.visited.iter().any(|&at| {
            self.map
                .at(at)
                .is_ok_and(|node| node.kind == V::MapPointType::BOSS)
        });
        Some(match (fought, rooms.second_boss) {
            (true, Some(second)) => second,
            _ => rooms.boss,
        })
    }

    pub(crate) fn choose(
        &mut self,
        seat: sts2_core::SeatIndex,
        index: u8,
    ) -> Result<Decided, Error> {
        if let Phase::Treasure { .. } = self.phase {
            self.pick_relic(seat, index);
            return Ok(Decided::Resolved);
        }
        if let Phase::RestSite { seats } = &self.phase {
            if self.phase.finished(seat) {
                unimplemented!("choosing at a rest site that is spent");
            }
            let option = *seats[seat]
                .options
                .get(usize::from(index))
                .unwrap_or_else(|| unimplemented!("an option the rest site does not offer"));
            if option == <V::RestSiteOption as sts2_core::RestKinds<V>>::MEND {
                unimplemented!("Mend aimed at nobody");
            }
            if let Phase::RestSite { seats } = &mut self.phase {
                seats[seat].chosen = Some(option);
            }
            // The option's `OnSelect` awaits the heal's rewards, so the
            // options stay up until the screen is done.
            if option == <V::RestSiteOption as sts2_core::RestKinds<V>>::HEAL {
                let asked = match self.rest_site_heal(seat, false) {
                    Some(screen) => self.carry_out(seat, [Then::Ask(screen)]),
                    None => None,
                };
                return Ok(match asked {
                    None => {
                        self.rest_option_through(seat);
                        Decided::Resolved
                    }
                    Some(asked) => self.park(seat, asked),
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
                if !self.rest_option_enabled(seat, option) {
                    unimplemented!("a rest site option the run cannot take");
                }
                // The options stay up while the option's pickup asks.
                let effects = sts2_core::RestKinds::selected(option);
                return Ok(match self.carry_out(seat, effects) {
                    None => {
                        self.rest_option_through(seat);
                        Decided::Resolved
                    }
                    Some(asked) => self.park(seat, asked),
                });
            }
            if option != <V::RestSiteOption as sts2_core::RestKinds<V>>::SMITH {
                unimplemented!("the {option:?} rest site option");
            }
            if !self.rest_option_enabled(seat, option) {
                unimplemented!("a rest site option the run cannot take");
            }
            let Phase::RestSite { seats } = &mut self.phase else {
                unreachable!("the phase was a rest site when this began");
            };
            seats[seat].upgrading = true;
            // The option opened a card selection rather than finishing. The
            // game reports the decision as unresolved until the selection is
            // confirmed, because `CardSelectorPrefs` makes it cancelable.
            return Ok(Decided::Abandoned);
        }
        if let Some(room) = self.event_room(seat) {
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
            let mut phase = std::mem::replace(&mut self.phase, Phase::Map);
            let Phase::Event { seats } = &mut phase else {
                unreachable!()
            };
            let chosen = &mut seats[seat];
            let effects = V::choose_event(
                &mut chosen.state,
                index,
                &self.seen_by(seat),
                &mut chosen.rng,
            );
            chosen.asking = None;
            chosen.finished = false;
            self.phase = phase;
            if let [sts2_core::HookEffect::EnterNextAct] = effects.as_slice() {
                return self.proceed(seat);
            }
            if let [
                sts2_core::HookEffect::FightInEvent {
                    encounter,
                    rewards,
                    roster,
                    resume,
                    own_rewards,
                },
            ] = effects.as_slice()
            {
                return self.fight_in_event(
                    seat,
                    *encounter,
                    rewards,
                    roster,
                    *resume,
                    *own_rewards,
                );
            }
            return Ok(match self.carry_out(seat, effects) {
                None => Decided::Resolved,
                Some(asked) => self.park(seat, asked),
            });
        }
        let Phase::Neow(seats) = &mut self.phase else {
            unimplemented!("choosing an option inside a fight");
        };
        let NeowSeat {
            options,
            chosen,
            modifier_options,
            ..
        } = &mut seats[seat];
        // `OnModifierOptionSelected`: the modifier's option, then the next.
        if !modifier_options.is_empty() {
            if index != 0 {
                unimplemented!("an option Neow does not offer");
            }
            let at = usize::from(modifier_options.remove(0));
            return Ok(self.neow_modifier_option(seat, at));
        }
        let option = *options
            .get(index as usize)
            .unwrap_or_else(|| unimplemented!("an option the blessing does not offer"));
        *chosen = nonmax::NonMaxU8::new(index);
        // `RelicCmd.Obtain`, and whatever the relic's `AfterObtained`
        // asks, which Neow's screen parks on.
        Ok(match self.obtain(seat, option) {
            None => Decided::Resolved,
            Some(asked) => self.park(seat, asked),
        })
    }

    /// What the modifier at `at` among the run's does as its Neow option is
    /// chosen.
    ///
    /// # Panics
    ///
    /// The modifier asking for something no Neow option carries out.
    fn neow_modifier_option(&mut self, seat: sts2_core::SeatIndex, at: usize) -> Decided {
        let entry = self.shared.modifiers[at];
        let mut asked = sts2_core::HookEffects::new();
        let state = &mut Arc::make_mut(&mut self.modifier_states)[at];
        V::modifier_event_hook(
            entry,
            state,
            sts2_core::EventHook::NeowOptionChosen,
            &mut asked,
        );
        match self.carry_out(seat, asked) {
            None => Decided::Resolved,
            Some(asked) => self.park(seat, asked),
        }
    }

    /// `CardFactory.CreateForReward` for one card, `draws` times, at
    /// `Uniform` odds. Each draws over the unlocked colorless pool or the
    /// character's own, and `copies` of each card go into the deck.
    fn add_uniform_to_deck(
        &mut self,
        seat: sts2_core::SeatIndex,
        colorless: bool,
        draws: usize,
        copies: usize,
    ) {
        let pool = match colorless {
            true => V::unlocked_colorless_cards(self.players.len() > 1),
            false => V::unlocked_cards(self.character(seat), self.players.len() > 1),
        };
        // `Uniform` leaves out the basic and ancient cards, which
        // are every rarity a pool holds past the three.
        let cards: Vec<V::Card> = pool
            .iter()
            .copied()
            .filter(|&card| V::reward_rarity(card) != sts2_core::RewardRarity::Other)
            .collect();
        for _ in 0..draws {
            // `TryModifyCardRewardOptions` over the one card.
            self.card_reward_unheard(seat);
            let [card] = crate::reward::card::uniform_rolled::<V, _>(
                self.ascension(),
                self.streams.player(seat, V::PlayerRngType::REWARDS),
                &cards,
                1,
                None,
            )[..] else {
                unreachable!("one draw is one card");
            };
            for _ in 0..copies {
                self.add_to_deck(seat, card);
            }
        }
    }

    /// What a pickup was part of, once the pickup is done.
    fn pickup_over(&mut self, seat: sts2_core::SeatIndex) -> Decided {
        // A rest site's option is done once its pickup is. A pickup no
        // option made, such as a relic granted there, finishes none.
        if let Phase::RestSite { seats } = &self.phase
            && seats[seat].chosen.is_some()
        {
            self.rest_option_through(seat);
        }
        Decided::Resolved
    }

    /// What `answer` to `asked`, a pickup's question, comes to.
    fn answered(
        &mut self,
        seat: sts2_core::SeatIndex,
        asked: Pickup<V>,
        answer: &Answer,
    ) -> Stepped<V> {
        let picks = || {
            let Answer::Cards(picks) = answer else {
                unimplemented!("an answer that is not a card selection");
            };
            *picks
        };
        match asked {
            // The screen stays up until every reward is taken or the rest
            // are skipped. A relic whose pickup asks stands the screen still
            // under its question.
            Pickup::Rewards {
                mut offered,
                skippable,
            } => {
                let since = self.belts[seat].len();
                match self.take_reward(seat, &mut offered, answer) {
                    Some((steps, taking)) => {
                        let screen = After::Screen {
                            offered,
                            skippable,
                            since,
                            taking,
                        };
                        Stepped::Nested(seat, steps, screen)
                    }
                    None => self.screen_left(offered, skippable),
                }
            }
            Pickup::Event => {
                let Phase::Event { seats } = &mut self.phase else {
                    unreachable!("only an event asks its own question")
                };
                let effects = V::answer_event(&mut seats[seat].state, answer);
                let steps = effects.into_iter().map(Then::Effect).collect();
                Stepped::Nested(seat, steps, After::Done)
            }
            // A yes is `GuaranteeKillAllPlayers`, which leaves the run with a
            // dead player and nothing more to decide. A no closes the popup
            // on the page it opened from, whose options stay.
            Pickup::ConfirmAbandon => {
                let Answer::Confirm(yes) = *answer else {
                    unimplemented!("answering the abandon-run popup with {answer:?}");
                };
                if yes {
                    self.players[seat].hp = num_traits::zero();
                }
                Stepped::Done
            }
            // `CardPileCmd.Add` of each card of the bundle chosen.
            Pickup::Bundles { bundles } => {
                let chosen = match picks().iter().collect::<Vec<_>>()[..] {
                    [one] => bundles.get(one),
                    _ => None,
                };
                let Some(chosen) = chosen else {
                    unimplemented!("a bundle choice naming other than one bundle offered");
                };
                for &card in chosen {
                    self.add_to_deck(seat, CardIdent::new(card));
                }
                Stepped::Done
            }
            Pickup::Cards { offered, min, max } => {
                let picks = picks();
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
                    self.add_to_deck(seat, card);
                }
                Stepped::Done
            }
            Pickup::FromDeck { act, min, count } => {
                let picks = picks();
                let offered = self.offered(seat, act);
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
                self.act_on_deck(seat, act, &taken);
                Stepped::Done
            }
            Pickup::CloneDeckCard { excluding } => {
                let Some(index) = picks().only() else {
                    unimplemented!("a clone answered with other than one card");
                };
                self.cloned(seat, index, excluding);
                Stepped::Done
            }
            Pickup::CardRemoval { free } => {
                let Some(index) = picks().only() else {
                    unimplemented!("a card removal answered with other than one card");
                };
                self.removed(seat, index, free);
                Stepped::Done
            }
        }
    }

    /// What a pickup goes on with once a pickup inside it is done. A reward
    /// screen asked again first hears the relics obtained since, which its
    /// card rewards hear as `RelicObtained`.
    fn after_nested(&mut self, seat: sts2_core::SeatIndex, after: After<V>) -> Stepped<V> {
        match after {
            After::Done => Stepped::Done,
            After::Walk(walking) => Stepped::Walk(walking),
            After::Screen {
                mut offered,
                skippable,
                since,
                taking,
            } => {
                self.heard_on_screen(seat, &mut offered, since, taking);
                self.screen_left(offered, skippable)
            }
        }
    }

    /// The relics obtained since the belt held `since`, as the card rewards
    /// on screen hear them, then the reward at `taking` taken.
    fn heard_on_screen(
        &mut self,
        seat: sts2_core::SeatIndex,
        offered: &mut [Offer<V>],
        since: usize,
        taking: Option<usize>,
    ) {
        for slot in since..self.belts[seat].len() {
            self.reward_options_on_obtained(seat, slot, offered);
        }
        if let Some(Offer::Card { taken, .. }) = taking.and_then(|at| offered.get_mut(at)) {
            *taken = true;
        }
    }

    /// A reward screen asked again while it has rewards left.
    fn screen_left(&self, offered: Vec<Offer<V>>, skippable: bool) -> Stepped<V> {
        match offered.iter().any(|offer| !offer.taken()) {
            true => Stepped::Ask(Pickup::Rewards { offered, skippable }),
            false => Stepped::Done,
        }
    }

    /// Park the room on what a pickup or an event option asks.
    ///
    /// # Panics
    ///
    /// A clone outside a shop, the only room whose shelf asks for one.
    fn park(&mut self, seat: sts2_core::SeatIndex, asked: Asked<V>) -> Decided {
        if let (Pickup::CloneDeckCard { .. }, false) =
            (&asked.pickup, matches!(self.phase, Phase::Shop { .. }))
        {
            unimplemented!("{:?} asked for outside a shop", asked.pickup);
        }
        let Some(asking) = self.phase.asking_mut(seat) else {
            unreachable!("only Neow and an event park on what they ask")
        };
        *asking = Some(asked);
        Decided::Abandoned
    }

    /// Answers the question the run is parked on, a click at a time for a
    /// card selection.
    ///
    /// `CardSelection.Accept` in the mod, which checks a pick as the game's
    /// screen does. It takes a card offered and not picked already, while
    /// the selection takes more. The picks go to the question once it is
    /// done.
    pub(crate) fn respond(
        &mut self,
        seat: sts2_core::SeatIndex,
        answer: &Answer,
    ) -> Result<Settled<V>, Error> {
        if let Answer::Cancel = answer {
            return Ok(self.cancel(seat));
        }
        let Some((min, max, offered)) = self.selecting(seat) else {
            if let Answer::Pick(_) | Answer::Undo | Answer::Done = answer {
                unimplemented!("{answer:?} to a question that selects no cards");
            }
            return self.answer(seat, answer).map(Ok);
        };
        let refused = |reason| {
            Ok(Err(Refusal {
                what: "answer".into(),
                reason,
            }))
        };
        let wrong_count = |chosen: usize| Reason::WrongCount {
            chosen,
            min: u32::try_from(min).expect("a selection counts in a few cards"),
            max: u32::try_from(max).expect("a selection counts in a few cards"),
        };
        // Each card checked as a pick would be, after the ones already held.
        let mut picked = self.players[seat].picked;
        let more: &[usize] = match answer {
            Answer::Pick(at) => &[usize::from(*at)],
            Answer::Cards(cards) => &cards.iter().collect::<Vec<_>>(),
            _ => &[],
        };
        for &at in more {
            if at >= offered {
                return refused(Reason::OutOfRange {
                    index: i64::try_from(at).expect("an option index fits"),
                    count: offered,
                });
            }
            if picked.iter().any(|taken| taken == at) {
                return refused(Reason::Duplicate { index: at });
            }
            if picked.len() >= max {
                return refused(wrong_count(picked.len() + 1));
            }
            picked.push(at);
        }
        match answer {
            Answer::Pick(_) => {
                self.players[seat].picked = picked;
                Ok(Ok(Decided::Resolved))
            }
            Answer::Undo => match self.players[seat].picked.pop() {
                Some(_) => Ok(Ok(Decided::Resolved)),
                None => refused(Reason::NothingPicked),
            },
            Answer::Done | Answer::Cards(_) => {
                if picked.len() < min {
                    return refused(wrong_count(picked.len()));
                }
                self.players[seat].picked = sts2_core::Picks::none();
                self.answer(seat, &Answer::Cards(picked)).map(Ok)
            }
            _ => unimplemented!("{answer:?} to a select-cards question"),
        }
    }

    pub(crate) fn answer(
        &mut self,
        seat: sts2_core::SeatIndex,
        answer: &Answer,
    ) -> Result<Decided, Error> {
        if let Phase::Rewards { .. } = self.phase {
            return self.take(seat, answer);
        }
        if matches!(&self.phase, Phase::Combat(live) if live.question(seat).is_some()) {
            return self.answer_fight(seat, answer);
        }
        if matches!(&self.phase, Phase::RestSite { seats } if seats[seat].upgrading) {
            return self.smith(seat, answer);
        }
        let Some(asking) = self.phase.asking_mut(seat) else {
            unimplemented!("answering a question inside a fight");
        };
        let asked = asking
            .take()
            .unwrap_or_else(|| unimplemented!("an answer to a question nobody asked"));
        Ok(self.answer_asked(seat, asked, answer))
    }

    /// The question `seat`'s event parks on.
    fn event_question(&self, seat: sts2_core::SeatIndex) -> Question<V> {
        let Phase::Event { seats } = &self.phase else {
            unreachable!("only an event asks its own question")
        };
        V::event_question(&seats[seat].state)
            .unwrap_or_else(|| unreachable!("an event parked on no question"))
    }

    /// Upgrade the card the smith was pointed at.
    ///
    /// `CardCmd.Upgrade` raises the level by one. The selection offers the
    /// deck's upgradable cards alone, so the index is that list's and the
    /// deck's own place comes back with it.
    ///
    /// The rest site is spent once this lands: its options empty, which is how
    /// the game reports a rest site nobody can act at again.
    fn smith(&mut self, seat: sts2_core::SeatIndex, answer: &Answer) -> Result<Decided, Error> {
        let Answer::Cards(picks) = answer else {
            unimplemented!("an answer to the smith that is not a card selection");
        };
        let Some(index) = picks.only() else {
            unimplemented!("a smith selection that is not one card");
        };
        let (at, _) = self
            .upgradable(seat)
            .nth(index)
            .unwrap_or_else(|| unimplemented!("upgrading a card the selection did not offer"));
        self.players[seat].deck_mut()[at].ident.upgrade += 1;

        let Phase::RestSite { seats } = &mut self.phase else {
            unreachable!("the phase was a rest site when this began");
        };
        seats[seat].upgrading = false;
        self.rest_option_through(seat);
        Ok(Decided::Resolved)
    }

    /// Take rewards from the screen a won fight put up.
    ///
    /// `RewardsSetSynchronizer.SelectLocalReward`, one entry at a time. A
    /// reward stays in the set once it is taken, because the indices an answer
    /// names never renumber.
    fn take(&mut self, seat: sts2_core::SeatIndex, answer: &Answer) -> Result<Decided, Error> {
        let Phase::Rewards { seats, .. } = &mut self.phase else {
            unreachable!("the phase was a reward screen when this began");
        };
        let RewardSeat {
            offered, removing, ..
        } = &mut seats[seat];
        // The removal a card removal reward waits on.
        if let Some(at) = *removing {
            let Answer::Cards(picks) = answer else {
                unimplemented!("a card removal answered with other than a card");
            };
            let Some(pick) = picks.only() else {
                unimplemented!("a card removal answered with other than one card");
            };
            *removing = None;
            let Some(&(in_deck, _)) = self.offered(seat, sts2_core::DeckAct::Remove).get(pick)
            else {
                unimplemented!("a removal naming a card the selection did not offer");
            };
            self.act_on_deck(seat, sts2_core::DeckAct::Remove, &[in_deck]);
            let Phase::Rewards { seats, .. } = &mut self.phase else {
                unreachable!("removing a deck card leaves the reward screen up");
            };
            seats[seat].offered[at] = Offer::CardRemoval { taken: true };
            return Ok(self.decided(seat));
        }
        // `DoUnsyncedCardRemoval`, which is taken once a card comes back.
        if let Answer::Take(take) = *answer
            && let Some(Offer::CardRemoval { taken: false }) = offered.get(take.index())
        {
            *removing = Some(take.index());
            return Ok(self.decided(seat));
        }
        let mut offered = std::mem::take(offered);
        let since = self.belts[seat].len();
        if let Some((steps, taking)) = self.take_reward(seat, &mut offered, answer) {
            if let Some(asked) = self.carry_out(seat, steps) {
                let pickup = asked.pickup;
                unimplemented!("a relic asking for {pickup:?} on a fight's reward screen");
            }
            self.heard_on_screen(seat, &mut offered, since, taking);
        }
        let Phase::Rewards { seats, .. } = &mut self.phase else {
            unreachable!("taking a fight's reward leaves its screen up");
        };
        seats[seat].offered = offered;
        Ok(self.decided(seat))
    }

    /// Take one reward off a screen, or skip what is left of it.
    ///
    /// # Panics
    ///
    /// An answer the screen does not accept, or a potion for a full belt.
    #[must_use]
    fn take_reward(
        &mut self,
        seat: sts2_core::SeatIndex,
        offered: &mut [Offer<V>],
        answer: &Answer,
    ) -> Option<(Steps<V>, Option<usize>)> {
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
        let slots: usize = num_traits::cast(self.players[seat].potion_slots)
            .expect("the belt has an addressable number of slots");
        let offer = offered
            .get_mut(take.index())
            .unwrap_or_else(|| unimplemented!("a reward the screen does not offer"));
        match (offer, take) {
            (Offer::Gold { amount, taken }, TakeReward::NonCard { .. }) => {
                let amount = *amount;
                *taken = true;
                self.gain_gold(seat, amount);
            }
            (Offer::Potion { potion, taken }, TakeReward::NonCard { .. }) => {
                let potion = *potion;
                // `PotionCmd.TryToProcure` fails on a full belt, and the
                // reward is then neither taken nor gone.
                if self.players[seat].potions.len() >= slots {
                    return None;
                }
                *taken = true;
                self.procure(seat, potion);
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
                self.add_to_deck(seat, chosen);
            }
            (
                Offer::SpecialCard {
                    card,
                    state,
                    returning,
                    taken,
                },
                TakeReward::NonCard { .. },
            ) => {
                let (ident, state) = (*card, *state);
                *taken = true;
                let id = returning.unwrap_or_else(|| self.players[seat].mint());
                self.add_deck_card(seat, sts2_core::DeckCard { id, ident, state });
            }
            (
                Offer::Relic {
                    relic, wax, taken, ..
                },
                TakeReward::NonCard { .. },
            ) => {
                let (relic, wax) = (*relic, *wax);
                *taken = true;
                return Some((self.obtained_held(seat, relic, wax, None), None));
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
                *cards = match how {
                    crate::reward::Rolled::Room { room, from_combat } => {
                        let pool =
                            self.reward_pool(seat, room == crate::reward::combat::Room::Elite);
                        let (ascension, act) = (self.ascension(), self.act());
                        let added = self.card_reward_added(seat, from_combat);
                        let (rng, odds) = self.rewarding(seat);
                        let Offer::Card { cards, .. } = crate::reward::combat::card_reward::<V, _>(
                            rng, odds, ascension, &pool, act, room, &added,
                        ) else {
                            unreachable!("a card reward rolls a card offer");
                        };
                        cards
                    }
                    crate::reward::Rolled::Custom { reward, stream } => {
                        let pool = self.reward_pool(seat, false);
                        self.card_reward_unheard(seat);
                        self.custom_cards(seat, reward, stream, Some(&pool))
                    }
                };
                self.reward_options(seat, cards, true);
            }
            // The alternative's `OnSelect`, which completes the reward.
            (
                Offer::Card { sacrifice, .. },
                TakeReward::Alternative {
                    alternative: alternative @ sts2_core::RewardAlternative::Sacrifice,
                    ..
                },
            ) => {
                if !*sacrifice {
                    unimplemented!("a sacrifice of a card reward that offers none");
                }
                // The reward leaves `RelicObtained` only as its selection
                // completes. So a relic the sacrifice gives, as Pael's Wing's
                // does, reaches this reward's cards as well as the others'.
                // The caller hands them the relics once the walk is done, and
                // marks the reward taken after.
                let chosen = sts2_core::EventHook::CardRewardAlternativeChosen(alternative);
                let Some(walking) = self.run_walking(chosen) else {
                    if let Some(Offer::Card { taken, .. }) = offered.get_mut(take.index()) {
                        *taken = true;
                    }
                    return None;
                };
                return Some((
                    [Then::Walk(walking)].into_iter().collect(),
                    Some(take.index()),
                ));
            }
            _ => unimplemented!("a reward taken as the wrong kind"),
        }
        None
    }

    /// Where the player stands.
    ///
    /// A live fight holds these numbers, because a card can move them.
    /// Anywhere else the run holds them. See `Player::hp`.
    fn standing(&self, seat: sts2_core::SeatIndex) -> Standing<V> {
        match &self.phase {
            Phase::Combat(live) => {
                let player = live.player(seat);
                Standing {
                    hp: player.hp,
                    max_hp: player.max_hp,
                    alive: player.alive,
                }
            }
            // The player fell in the fight, and the fight is where its own
            // numbers stayed.
            Phase::Lost { combats, .. } => {
                let player = combats[seat]
                    .creatures
                    .iter()
                    .find(|c| c.id == seat.creature())
                    .expect("a lost fight has the player in it");
                Standing {
                    hp: player.hp,
                    max_hp: player.max_hp,
                    alive: player.alive,
                }
            }
            // `GuaranteeKillAllPlayers`, which `WinRun` ends on.
            Phase::Won { .. } => Standing {
                hp: num_traits::zero(),
                max_hp: self.players[seat].max_hp,
                alive: false,
            },
            Phase::Map
            | Phase::Neow { .. }
            | Phase::Rewards { .. }
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => Standing {
                hp: self.players[seat].hp,
                max_hp: self.players[seat].max_hp,
                // Only a death outside a fight leaves the run with none.
                alive: self.players[seat].hp > num_traits::zero(),
            },
        }
    }

    /// The belt, and why each potion in it cannot be drunk.
    ///
    /// Beside the report rather than inside it, because deciding whether a
    /// potion can be drunk reads this list and nothing else around it.
    pub(crate) fn belt(&self, seat: sts2_core::SeatIndex) -> Vec<sts2_core::Held<V>> {
        // A fight that is over is not one a potion can be drunk in, so a live
        // fight is the only one read.
        crate::combat::belt::<V>(
            seat,
            &self.players[seat].potions,
            self.fight(),
            self.merchant(seat),
            self.standing(seat).alive,
            self.potions_held(seat),
        )
        .collect()
    }

    /// Every decision the run accepts right now, in the order a driver picks
    /// from.
    ///
    /// Read off the run, never off its report. Each phase reads what its
    /// report reads and runs the rules the report runs, in the report's order:
    /// a pending question alone, or the belt, then the room, then the map.
    pub(crate) fn legal(&self, seat: sts2_core::SeatIndex) -> Legal<V> {
        let mut moves = Legal::new();
        self.legal_into(seat, &mut moves);
        moves
    }

    /// [`State::legal`], onto the end of `moves`.
    pub(crate) fn legal_into(&self, seat: sts2_core::SeatIndex, moves: &mut Legal<V>) {
        let alive = self.standing(seat).alive;
        if let Some((min, max, offered)) = self.selecting(seat)
            && (alive || self.in_combat())
        {
            crate::legal::picks(min, max, offered, &self.players[seat].picked, moves);
            if self.cancelable(seat) {
                moves.push(Decision::Answer(Answer::Cancel));
            }
            return;
        }
        if let Phase::Combat(live) = &self.phase {
            // A player who ended the turn has nothing to do until the rest
            // have, as the screen greys the hand out.
            if live.ready(seat) {
                return;
            }
            crate::combat::legal_moves::<V>(
                seat,
                live,
                &self.belts,
                &self.players[seat].potions,
                alive,
                moves,
            );
            self.discards(seat, alive, moves);
            return;
        }

        // `IsGameOver`: a run whose player is dead takes nothing more.
        if !alive {
            return;
        }
        if self.answers(seat, moves) {
            if self.pending(seat) == Some(sts2_core::QuestionKind::Rewards) {
                self.discards(seat, alive, moves);
            }
            return;
        }
        // No fight is live here, so a drink names no creature.
        crate::combat::drinks::<V>(
            seat,
            &self.players[seat].potions,
            None,
            self.merchant(seat),
            alive,
            self.potions_held(seat),
            moves,
        );
        self.room_moves(seat, moves);
        if self.can_proceed() {
            moves.push(Decision::Proceed);
        } else {
            moves.extend(
                self.travelable(seat)
                    .into_iter()
                    .map(|point| Decision::Travel { to: point.at }),
            );
        }
        self.discards(seat, alive, moves);
    }

    /// A discard for each potion in the belt, last in every list, as the
    /// mod's `Moves.Discards` lists them. The popup's button is shut for a
    /// dead owner, and no potion is queued at a decision.
    fn discards(&self, seat: sts2_core::SeatIndex, alive: bool, moves: &mut Legal<V>) {
        if !alive {
            return;
        }
        moves.extend((0..self.players[seat].potions.len()).map(|at| {
            Decision::DiscardPotion {
                index: u8::try_from(at)
                    .ok()
                    .and_then(|at| sts2_core::BeltIndex::try_new(at).ok())
                    .expect("a belt fits a byte"),
            }
        }));
    }

    /// Whether the room the run stands in lets it go to the map.
    ///
    /// `NMapScreen.IsTravelEnabled`, which each room's screen turns on when
    /// it is done with the run. The map room and a merchant's do so as they
    /// open, and a rest site once an option went through. A chest waits for
    /// its relic, an event until it finishes, and a fight for its rewards.
    fn travel_enabled(&self, seat: sts2_core::SeatIndex) -> bool {
        // The map stays shut until the event under the fight is resumed.
        if self.resuming.is_some()
            || !self.standing(seat).alive
            || self.phase.asking(seat).is_some()
        {
            return false;
        }
        match &self.phase {
            Phase::Map => true,
            Phase::Shop { .. } => true,
            Phase::Neow { .. } | Phase::Treasure { .. } => self.phase.finished(seat),
            // `NFakeMerchant` turns it on as it opens, like a merchant's room.
            Phase::Event { seats } => {
                seats[seat].finished || matches!(self.event_room(seat), Some(Room::Shop { .. }))
            }
            // `OnActiveScreenUpdated` puts the proceed button up for a rest
            // site that offers nothing at all.
            Phase::RestSite { seats } => {
                let rest = &seats[seat];
                (rest.rested || rest.options.is_empty()) && rest.chosen.is_none() && !rest.upgrading
            }
            Phase::Rewards { seats, .. } => {
                seats[seat].removing.is_none() && seats[seat].offered.iter().all(Offer::taken)
            }
            Phase::Combat(_) | Phase::Lost { .. } | Phase::Won { .. } => false,
        }
    }

    /// The answers to the question the run is parked on, if it is parked.
    ///
    /// Each is the question [`State::report`] asks in that phase, read for its
    /// sizes and its rewards alone.
    fn answers(&self, seat: sts2_core::SeatIndex, moves: &mut Legal<V>) -> bool {
        // `legal` answered a selection already, with its picks.
        match &self.phase {
            Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. }
                if self.phase.asking(seat).is_some() =>
            {
                let Some(asked) = self.phase.asking(seat) else {
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
                    Pickup::Cards { .. }
                    | Pickup::FromDeck { .. }
                    | Pickup::CloneDeckCard { .. }
                    | Pickup::CardRemoval { .. } => {
                        unreachable!("a selection's moves are its picks")
                    }
                    Pickup::Rewards { offered, skippable } => {
                        reward_moves(offered, *skippable, moves)
                    }
                    Pickup::Event => event_moves(self.event_question(seat), moves),
                }
            }
            Phase::Rewards { seats, .. }
                if seats[seat].offered.iter().any(|offer| !offer.taken()) =>
            {
                reward_moves(&seats[seat].offered, true, moves)
            }
            _ => return false,
        }
        true
    }

    /// The bounds of the select-cards question the run is parked on, and how
    /// many cards it offers: `(min, max, offered)`.
    ///
    /// The one place a selection's size is read, for its moves and for
    /// refusing a pick alike.
    fn selecting(&self, seat: sts2_core::SeatIndex) -> Option<(usize, usize, usize)> {
        match &self.phase {
            Phase::Combat(live) => live
                .selecting(seat)
                .map(|(min, max, offered)| (min as usize, max as usize, offered)),
            Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. }
                if self.phase.asking(seat).is_some() =>
            {
                match &self.phase.asking(seat)?.pickup {
                    Pickup::Cards { offered, min, max } => Some((*min, *max, offered.len())),
                    Pickup::FromDeck { act, min, count } => {
                        Some((*min, *count, self.offered(seat, *act).len()))
                    }
                    Pickup::CloneDeckCard { excluding } => {
                        Some((1, 1, self.clonable(seat, *excluding).len()))
                    }
                    Pickup::CardRemoval { .. } => {
                        Some((1, 1, self.offered(seat, sts2_core::DeckAct::Remove).len()))
                    }
                    _ => None,
                }
            }
            // `CardSelectCmd.FromDeckForUpgrade`: one card, never none.
            Phase::RestSite { seats } if seats[seat].upgrading => {
                Some((1, 1, self.upgradable(seat).count()))
            }
            Phase::Rewards { seats, .. } if seats[seat].removing.is_some() => {
                Some((1, 1, self.offered(seat, sts2_core::DeckAct::Remove).len()))
            }
            _ => None,
        }
    }

    /// The options a room lets the run take, and the stock it lets it buy.
    ///
    /// The rules the report's flags carry. A rest option can be disabled, a
    /// chest answered, and an event option chosen or locked. Stock can be gone,
    /// cost more than the purse, or be a potion for a full belt.
    fn room_moves(&self, seat: sts2_core::SeatIndex, moves: &mut Legal<V>) {
        let finished = self.phase.finished(seat);
        match &self.phase {
            Phase::Neow(seats) if !finished && !seats[seat].modifier_options.is_empty() => {
                if self.phase.asking(seat).is_none() {
                    moves.push(Decision::ChooseOption {
                        index: 0,
                        target: None,
                    });
                }
            }
            Phase::Neow(seats) if !finished => moves.extend(
                (0..seats[seat].options.len())
                    .map(option_index)
                    .filter(|&index| seats[seat].chosen != nonmax::NonMaxU8::new(index))
                    .map(|index| Decision::ChooseOption {
                        index,
                        target: None,
                    }),
            ),
            // An event's options are the content's to spell, so its room is
            // built. That is the room alone, and only in an event.
            // An event whose room is a shop sells what it reports.
            Phase::Event { .. } => match self.event_room(seat) {
                Some(Room::Event { options, .. }) => moves.extend(
                    options
                        .iter()
                        .filter(|option| !option.locked && !option.chosen)
                        .map(|option| Decision::ChooseOption {
                            index: option.index,
                            target: None,
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
            // Each relic, then the skip one past them.
            Phase::Treasure { relics, .. } if !self.phase.finished(seat) => {
                moves.extend((0..=relics.len()).map(|index| Decision::ChooseOption {
                    index: option_index(index),
                    target: None,
                }));
            }
            Phase::RestSite { seats } => {
                for (index, &id) in seats[seat].options.iter().enumerate() {
                    if !self.rest_option_enabled(seat, id) {
                        continue;
                    }
                    let index = option_index(index);
                    // Mend aims at another player, dead or alive.
                    if id == <V::RestSiteOption as sts2_core::RestKinds<V>>::MEND {
                        moves.extend(self.seats().filter(|&other| other != seat).map(|other| {
                            Decision::ChooseOption {
                                index,
                                target: Some(other),
                            }
                        }));
                        continue;
                    }
                    moves.push(Decision::ChooseOption {
                        index,
                        target: None,
                    });
                }
            }
            Phase::Shop { seats } => {
                let gold = self.players[seat].gold;
                let room_for_a_potion =
                    num_traits::cast::<_, usize>(self.players[seat].potion_slots)
                        .expect("a potion slot count fits this machine")
                        > self.players[seat].potions.len();
                moves.extend(
                    seats[seat]
                        .entries
                        .iter()
                        .filter(|entry| {
                            entry.stocked
                                && self.merchant_price(seat, entry.cost) <= gold
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
    pub(crate) fn travelable(&self, seat: sts2_core::SeatIndex) -> SmallVec<[Reachable<V>; 7]> {
        if !self.travel_enabled(seat) {
            return SmallVec::new();
        }
        self.reachable()
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
    fn procure_refused(&self, seat: sts2_core::SeatIndex, potion: V::Potion) -> bool {
        let hook = sts2_core::DecisionHook::ShouldProcurePotion {
            player: seat.creature(),
            potion,
        };
        let model_hook = sts2_core::ModelHook::ShouldProcurePotion;
        let mut refused = false;
        self.run_each(model_hook, |listener, _| {
            refused |= V::decide_hook(listener, hook).is_decision_and(|allowed| !allowed);
        });
        if refused {
            return true;
        }
        false
    }

    /// `MerchantEntry.Cost` in a merchant's room: `Hook.ModifyMerchantPrice`
    /// over what the shelf holds, which `(int)` truncates.
    fn merchant_price(&self, seat: sts2_core::SeatIndex, cost: V::Number) -> V::Number {
        let hook = sts2_core::ModifyHook::MerchantPrice;
        let mut left = self.belts[seat].hearing(sts2_core::Overrides::of(hook.model_hook()));
        if left == 0 {
            return cost;
        }
        let mut price = crate::combat::exact::<V>(cost);
        while left != 0 {
            let held = &self.belts[seat][left.trailing_zeros() as usize];
            left &= left - 1;
            match V::modify_hook(
                crate::combat::Heard::<V>::Relic {
                    state: &held.state,
                    owner: seat.creature(),
                    fight: None,
                    belt: self.belts[seat].relics(),
                },
                hook,
            ) {
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
    fn rest_option_through(&mut self, seat: sts2_core::SeatIndex) {
        let hook = sts2_core::DecisionHook::ShouldDisableRemainingRestSiteOptions {
            player: seat.creature(),
        };
        let model_hook = sts2_core::ModelHook::ShouldDisableRemainingRestSiteOptions;
        let mut disable = true;
        self.run_each(model_hook, |listener, _| {
            disable &= !V::decide_hook(listener, hook).is_decision_and(|disable| !disable);
        });
        let Phase::RestSite { seats } = &mut self.phase else {
            unreachable!("an option went through at a rest site");
        };
        let RestSeat {
            options,
            chosen,
            rested,
            ..
        } = &mut seats[seat];
        let chosen = chosen
            .take()
            .unwrap_or_else(|| unreachable!("an option went through that was chosen"));
        *rested = true;
        match disable {
            true => options.clear(),
            false => options.retain(|&option| option != chosen),
        }
    }

    /// `Hook.ShouldRefillMerchantEntry` for `seat`'s purchase, which a
    /// relic of that player's can answer yes to. The game's compares the
    /// buyer with its owner.
    fn refills(&self, seat: sts2_core::SeatIndex) -> bool {
        let hook = sts2_core::DecisionHook::ShouldRefillMerchantEntry;
        let model_hook = sts2_core::ModelHook::ShouldRefillMerchantEntry;
        let mut refills = false;
        self.run_each(model_hook, |listener, _| {
            if let sts2_core::Listener::Relic { owner, .. } = listener
                && owner != seat.creature()
            {
                return;
            }
            refills |= V::decide_hook(listener, hook).is_decision_and(|refill| refill);
        });
        refills
    }

    /// What a merchant's shelf does once its purchase went through:
    /// `RestockAfterPurchase` where a listener refills it, and
    /// `ClearAfterPurchase` otherwise.
    fn purchased(&mut self, seat: sts2_core::SeatIndex, index: u8, spent: V::Number) {
        self.clear_or_restock(seat, index);
        self.item_purchased(seat, spent);
    }

    /// `Hook.AfterItemPurchased`, once an entry's purchase has returned and
    /// its shelf has cleared or restocked.
    ///
    /// # Panics
    ///
    /// A relic asking for something as it hears of the purchase.
    fn item_purchased(&mut self, seat: sts2_core::SeatIndex, gold: V::Number) {
        let asked = self.notify_run_for(seat, sts2_core::EventHook::ItemPurchased { gold });
        if let Some(effect) = asked.first() {
            unimplemented!("a relic asking for {effect:?} after a purchase");
        }
        self.entries_updated(seat);
    }

    /// `MerchantInventory.UpdateEntries`, once a purchase completes:
    /// `Hook.ModifyMerchantCardCreationResults` again over every card a
    /// shelf holds. An egg bought here upgrades what is already for sale.
    /// The price stays what the shelf was stocked at.
    fn entries_updated(&mut self, seat: sts2_core::SeatIndex) {
        let Phase::Shop { seats } = &self.phase else {
            return;
        };
        let held: Vec<(usize, CardIdent<V>)> = seats[seat]
            .entries
            .iter()
            .enumerate()
            .filter_map(|(at, entry)| match entry.stock {
                sts2_core::Stock::Card(Some(on_sale)) if entry.stocked => Some((at, on_sale.card)),
                _ => None,
            })
            .collect();
        for (at, card) in held {
            let card = self.rewritten(seat, card, |card| {
                sts2_core::EventHook::MerchantCardCreated { card }
            });
            let Phase::Shop { seats } = &mut self.phase else {
                unreachable!("a shop stays a shop while it updates");
            };
            if let sts2_core::Stock::Card(Some(on_sale)) = &mut seats[seat].entries[at].stock {
                on_sale.card = card;
                (on_sale.cost, on_sale.star_cost) = crate::combat::printed_costs(card);
            }
        }
    }

    /// The gold a parked purchase noted, back in the run's own number.
    fn gold(spent: u32) -> V::Number {
        num_traits::cast(spent).expect("gold spent fits the run's number")
    }

    /// `RestockAfterPurchase` where a relic refills the shelf, and
    /// `ClearAfterPurchase` otherwise.
    fn clear_or_restock(&mut self, seat: sts2_core::SeatIndex, index: u8) {
        if !self.refills(seat) {
            let Phase::Shop { seats } = &mut self.phase else {
                unreachable!("a purchase is made in a shop");
            };
            sold(&mut seats[seat].entries, index);
            return;
        }
        let Phase::Shop { seats } = &self.phase else {
            unreachable!("a purchase is made in a shop");
        };
        let entries = &seats[seat].entries;
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
        let added = self.merchant_card_pool();
        let mut streams = self.streams;
        let mut dealt = self.players[seat].dealt_relics.to_vec();
        let stocked_from = dealt.len();
        let (stock, cost) = super::shop::Merchant::<V> {
            streams: &mut streams,
            player_relics: &self.shared.opening.player_relics[seat.at()],
            dealt_relics: &mut dealt,
            floor: num_traits::cast(self.floor).expect("a floor fits a usize"),
            character: self.players[seat].character,
            players: self.players.len(),
            added,
            offset: self.players[seat].odds.rarity,
            ascension: self.ascension(),
            removals_used: self.players[seat].removals_used,
        }
        .restock(seat, usize::from(index), stock, &on_shelves);
        self.streams = streams;
        // `PullNextRelicFromBack` takes what it stocks out of the shared bag
        // too.
        Arc::make_mut(&mut self.shared_dealt).extend_from_slice(&dealt[stocked_from..]);
        self.players[seat].dealt_relics = Arc::new(dealt);
        // `Hook.ModifyMerchantCardCreationResults` over the card it stocked.
        let stock = match stock {
            sts2_core::Stock::Card(Some(mut on_sale)) => {
                on_sale.card = self.rewritten(seat, on_sale.card, |card| {
                    sts2_core::EventHook::MerchantCardCreated { card }
                });
                (on_sale.cost, on_sale.star_cost) = crate::combat::printed_costs(on_sale.card);
                sts2_core::Stock::Card(Some(on_sale))
            }
            stock => stock,
        };
        let Phase::Shop { seats } = &mut self.phase else {
            unreachable!("a purchase is made in a shop");
        };
        let entry = &mut seats[seat].entries[at];
        entry.stock = stock;
        entry.cost = num_traits::cast(cost).expect("a merchant's price fits the build's number");
        entry.stocked = true;
    }

    /// `Hook.ShouldForcePotionReward` for a fight won in `room`, which any
    /// relic on the belt can answer yes to.
    pub(super) fn potion_forced(&self, seat: sts2_core::SeatIndex, room: V::RoomType) -> bool {
        let hook = sts2_core::DecisionHook::ShouldForcePotionReward {
            player: seat.creature(),
            room,
        };
        let model_hook = sts2_core::ModelHook::ShouldForcePotionReward;
        let mut forced = false;
        self.run_each(model_hook, |listener, _| {
            forced |= V::decide_hook(listener, hook).is_decision_and(|force| force);
        });
        forced
    }

    /// `Hook.ShouldAllowMerchantCardRemoval`, which any relic on the belt
    /// or any modifier can answer no to.
    fn card_removal_allowed(&self) -> bool {
        let hook = sts2_core::DecisionHook::ShouldAllowMerchantCardRemoval;
        let model_hook = sts2_core::ModelHook::ShouldAllowMerchantCardRemoval;
        let mut allowed = true;
        self.run_each(model_hook, |listener, _| {
            allowed &= !V::decide_hook(listener, hook).is_decision_and(|allowed| !allowed);
        });
        allowed
    }

    /// `Hook.ShouldAllowFreeTravel`, which any relic on the belt or any
    /// modifier can answer yes to.
    fn free_travel(&self) -> bool {
        let hook = sts2_core::DecisionHook::ShouldAllowFreeTravel;
        let model_hook = sts2_core::ModelHook::ShouldAllowFreeTravel;
        let mut free = false;
        self.run_each(model_hook, |listener, _| {
            free |= V::decide_hook(listener, hook).is_decision_and(|allowed| allowed);
        });
        free
    }

    /// Derive the comparison snapshot from the run's authoritative state.
    pub(crate) fn report(&self, seat: sts2_core::SeatIndex) -> Snapshot<V> {
        let standing = self.standing(seat);

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
                victory: (!standing.alive).then_some(matches!(self.phase, Phase::Won { .. })),
                gold: self.players[seat].gold,
                relics: self.carried(seat).collect(),
                potions: self.belt(seat),
                potion_slots: self.players[seat].potion_slots,
                deck: self.players[seat]
                    .deck
                    .iter()
                    .map(|held| held.ident)
                    .collect(),
                in_combat: self.in_combat(),
                map: Map {
                    at: self.visited.last().copied(),
                    visited: self.visited.to_vec(),
                    travelable: self.travelable(seat).into_vec(),
                    points: self.map.shared(),
                    proceed: self.can_proceed(),
                },
                rng: self.positions(seat),
                room: self.room(seat),
                combat: self.fight_report(seat),
                moves: self.legal(seat).into_vec(),
            },
            asking: self.asking(seat).map(|mut question| {
                if let Question::SelectCards { picked, .. } = &mut question {
                    *picked = self.players[seat].picked;
                }
                question
            }),
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
    ///
    /// The first of two bosses is not a way out: travel to the second is.
    fn can_proceed(&self) -> bool {
        let Phase::Rewards { seats, .. } = &self.phase else {
            return false;
        };
        // Every player fought the same encounter.
        let combat = &seats.host().combat;
        if self.boss_ahead() {
            return false;
        }
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
    fn carried(
        &self,
        seat: sts2_core::SeatIndex,
    ) -> impl ExactSizeIterator<Item = Carried<V>> + '_ {
        self.belts[seat].iter().map(move |held| Carried {
            relic: held.relic,
            amount: held.amount(
                matches!(self.phase, Phase::Combat(_)),
                self.player_turn(seat),
            ),
            title: None,
            text: None,
        })
    }

    /// The fight the run is standing in, as its report shows it.
    fn fight_report(&self, seat: sts2_core::SeatIndex) -> Option<Combat<V>> {
        match &self.phase {
            Phase::Combat(live) => Some(live.report(seat, &self.belts)),
            // `IsInProgress` is false once the loss is processed, and the
            // room it was fought in is still the room the run is standing in.
            // A won fight is still reported for the same reason: the run has
            // not left the room it was fought in.
            Phase::Lost { combats, .. } => Some(combats[seat].clone()),
            // The player's creature is the run's own, so what happens to it
            // between the fight and leaving the room shows on it.
            Phase::Rewards { seats, .. } => {
                let mut combat = seats[seat].combat.clone();
                let player = seat.creature();
                if let Some(creature) = combat.creatures.iter_mut().find(|c| c.id == player) {
                    creature.hp = self.players[seat].hp;
                    creature.max_hp = self.players[seat].max_hp;
                }
                Some(combat)
            }
            Phase::Map
            | Phase::Won { .. }
            | Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => None,
        }
    }

    /// The question the run is parked on, if it is parked.
    fn asking(&self, seat: sts2_core::SeatIndex) -> Option<Question<V>> {
        match &self.phase {
            Phase::RestSite { seats } if seats[seat].asking.is_none() => {
                seats[seat].upgrading.then(|| Question::SelectCards {
                    min: 1,
                    max: 1,
                    asker: None,
                    options: self
                        .upgradable(seat)
                        .enumerate()
                        .map(|(index, (_, card))| CardOption {
                            index,
                            card,
                            text: None,
                        })
                        .collect(),
                    picked: sts2_core::Picks::none(),
                    pile: None,
                })
            }
            // A rest site parks only on what an option's pickup asks.
            Phase::Neow { .. }
            | Phase::Event { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => {
                self.phase.asking(seat).map(|asking| {
                    let (min, max, cards): (u32, u32, Vec<CardIdent<V>>) = match &asking.pickup {
                        Pickup::Rewards { offered, skippable } => {
                            return asked::<V>(offered, *skippable);
                        }
                        Pickup::Event => return self.event_question(seat),
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
                        // The deck less what the relic's own filter leaves out.
                        Pickup::CloneDeckCard { excluding } => (
                            1,
                            1,
                            self.clonable(seat, *excluding)
                                .into_iter()
                                .map(|held| held.ident)
                                .collect(),
                        ),
                        // The deck less its `Eternal` cards, curses first.
                        Pickup::CardRemoval { .. } => (
                            1,
                            1,
                            self.offered(seat, sts2_core::DeckAct::Remove)
                                .into_iter()
                                .map(|(_, card)| card)
                                .collect(),
                        ),
                        Pickup::FromDeck { act, min, count } => (
                            u32::try_from(*min).expect("a selection counts in a few cards"),
                            u32::try_from(*count).expect("a selection counts in a few cards"),
                            self.offered(seat, *act)
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
                        picked: sts2_core::Picks::none(),
                        pile: None,
                    }
                })
            }
            Phase::Combat(live) => live.question(seat),
            // `CardSelectCmd.FromDeckForRemoval`: one card of the deck less
            // its `Eternal` ones, curses first.
            Phase::Rewards { seats, .. } if seats[seat].removing.is_some() => {
                Some(Question::SelectCards {
                    min: 1,
                    max: 1,
                    asker: None,
                    options: self
                        .offered(seat, sts2_core::DeckAct::Remove)
                        .into_iter()
                        .enumerate()
                        .map(|(index, (_, card))| CardOption {
                            index,
                            card,
                            text: None,
                        })
                        .collect(),
                    picked: sts2_core::Picks::none(),
                    pile: None,
                })
            }
            Phase::Rewards { seats, .. } => {
                let offered = &seats[seat].offered;
                offered
                    .iter()
                    .any(|offer| !offer.taken())
                    .then(|| asked::<V>(offered, true))
            }
            // `CardSelectCmd.FromDeckForUpgrade` offers the deck's upgradable
            // cards alone, so a card already at its highest is not offered and
            // the indices are that list's rather than the deck's. One card, and
            // the selection cannot be empty.
            // An act opens on its map, which is no room and asks nothing.
            Phase::Map | Phase::Lost { .. } | Phase::Won { .. } | Phase::Treasure { .. } => None,
        }
    }

    /// The event the run stands in, as its page shows it. A finished one
    /// offers nothing.
    fn event_room(&self, seat: sts2_core::SeatIndex) -> Option<Room<V>> {
        let Phase::Event { seats } = &self.phase else {
            return None;
        };
        let EventSeat {
            state, finished, ..
        } = &seats[seat];
        let mut room = V::observe_event(state, &self.seen_by(seat));
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
    fn room(&self, seat: sts2_core::SeatIndex) -> Option<Room<V>> {
        let finished = self.phase.finished(seat);
        match &self.phase {
            Phase::Neow(seats) => {
                let NeowSeat {
                    options: neow_options,
                    chosen: neow_chosen,
                    modifier_options,
                    asking,
                } = &seats[seat];
                // Neow shows the next modifier's option alone. While one
                // still runs, its option stays up, chosen.
                let offering: SmallVec<[usize; 4]> = (self.shared.modifiers.iter())
                    .enumerate()
                    .filter(|(_, entry)| {
                        sts2_core::ModifierTable::offers_neow_option(entry.modifier)
                    })
                    .map(|(at, _)| at)
                    .collect();
                let running = (asking.is_some() && offering.len() > modifier_options.len())
                    .then(|| offering[offering.len() - modifier_options.len() - 1]);
                let shown = match running {
                    Some(at) => Some((at, true)),
                    None => modifier_options.first().map(|&at| (usize::from(at), false)),
                };
                if let Some((at, chosen)) = shown {
                    let modifier = self.shared.modifiers[at].modifier;
                    return Some(Room::Event {
                        event: V::neow_event(),
                        finished,
                        title: None,
                        text: None,
                        options: vec![EventOption {
                            index: 0,
                            key: EventOptionKey {
                                event: V::neow_event(),
                                option: V::EventOptionId::from_modifier(modifier),
                            },
                            title: None,
                            text: None,
                            locked: false,
                            proceed: false,
                            chosen,
                            relic: None,
                        }],
                    });
                }
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
            Phase::Event { .. } => self.event_room(seat),
            Phase::Won { room } => room.clone(),
            Phase::Treasure { relics, .. } => {
                // The relics on offer and the skip. Awarding them empties the
                // chest. A skip by the one player leaves them showing,
                // answered, until the room is left.
                let finished = self.phase.finished(seat);
                let takes = relics.iter().map(|&relic| sts2_core::Take::Relic(relic));
                let options = match relics.is_empty() {
                    true => Vec::new(),
                    false => takes
                        .chain([sts2_core::Take::Skip])
                        .enumerate()
                        .map(|(index, take)| sts2_core::ChestOption {
                            index: option_index(index),
                            take,
                            chosen: finished,
                        })
                        .collect(),
                };
                Some(Room::Treasure { finished, options })
            }
            Phase::RestSite { seats } => Some(Room::RestSite {
                options: seats[seat]
                    .options
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, id)| sts2_core::RestOption {
                        index: index as u8,
                        id,
                        enabled: self.rest_option_enabled(seat, id),
                    })
                    .collect(),
            }),
            Phase::Shop { seats } => {
                // `EnoughGold` and the potion slot check are derived, the way
                // the belt is. Both move as the run buys, and neither is a
                // fact about the shelf.
                let gold = self.players[seat].gold;
                let room_for_a_potion =
                    num_traits::cast::<_, usize>(self.players[seat].potion_slots)
                        .expect("a potion slot count fits this machine")
                        > self.players[seat].potions.len();
                let entries = seats[seat]
                    .entries
                    .iter()
                    .cloned()
                    .map(|entry| sts2_core::Entry {
                        cost: self.merchant_price(seat, entry.cost),
                        affordable: self.merchant_price(seat, entry.cost) <= gold,
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
            Phase::Map | Phase::Combat(_) | Phase::Lost { .. } | Phase::Rewards { .. } => None,
        }
    }
}

impl<V: Rules> State<V> {
    fn upgrade_random_deck_cards(
        &mut self,
        seat: sts2_core::SeatIndex,
        count: usize,
        stream: sts2_core::Stream<V>,
        kind: Option<sts2_core::CardKind>,
    ) {
        use sts2_core::CardTable as _;
        use sts2_core::rng::GameRng as _;

        // `CardSelectCmd` shuffles the upgradable cards alone, so a card
        // already at its highest neither takes a place nor moves the shuffle.
        let deck = self.players[seat].deck.clone();
        let mut upgradable: Vec<usize> = self
            .upgradable(seat)
            .filter(|(_, card)| kind.is_none_or(|kind| V::card_kind(card.card) == Some(kind)))
            .map(|(at, _)| at)
            .collect();
        stream_of(seat, &mut self.phase, &mut self.streams, stream)
            .stable_shuffle_by_key(&mut upgradable, |&at| {
                (deck[at].ident.card.order(), deck[at].ident.upgrade)
            });
        for at in upgradable.into_iter().take(count) {
            self.players[seat].deck_mut()[at].ident.upgrade += 1;
        }
    }
}

impl<V: Rules> sts2_core::EventView<V> for AtSeat<'_, V> {
    fn act(&self) -> V::Count {
        num_traits::cast(self.state.act()).expect("an act index fits the build count")
    }

    fn floor(&self) -> V::Count {
        self.state.floor
    }

    fn gold(&self) -> V::Number {
        self.state.players[self.seat].gold
    }

    fn hp(&self) -> V::Health {
        self.state.players[self.seat].hp
    }

    fn max_hp(&self) -> V::Health {
        self.state.players[self.seat].max_hp
    }

    fn potions(&self) -> impl Iterator<Item = V::Potion> + '_ {
        self.state.players[self.seat].potions.iter().copied()
    }

    fn open_potion_slot(&self) -> bool {
        let slots: usize = num_traits::cast(self.state.players[self.seat].potion_slots)
            .expect("a belt's slots fit a usize");
        self.state.players[self.seat].potions.len() < slots
    }

    fn character(&self) -> V::Character {
        self.state.players[self.seat].character
    }

    fn multiplayer(&self) -> bool {
        self.state.players.len() > 1
    }

    fn wins(&self) -> u32 {
        self.state.players[self.seat].wins
    }

    fn modifiers(&self) -> &[sts2_core::RunModifier<V>] {
        &self.state.shared.modifiers
    }

    fn relics(&self) -> impl Iterator<Item = sts2_core::CarriedRelic<V>> + '_ {
        use sts2_core::RelicModel as _;

        self.state.belts[self.seat]
            .iter()
            .map(|held| sts2_core::CarriedRelic {
                relic: held.relic,
                used_up: held.state.used_up(held.relic),
                melted: held.melted,
            })
    }

    fn deck(&self) -> Vec<CardIdent<V>> {
        self.state.players[self.seat]
            .deck
            .iter()
            .map(|held| held.ident)
            .collect()
    }

    fn room(&self) -> Option<V::RoomType> {
        self.state.quiet_room()
    }

    fn relics_left(&self) -> bool {
        self.state.shared.opening.player_relics[self.seat.at()].has_left(
            &self.state.players[self.seat].dealt_relics,
            num_traits::cast(self.state.floor).expect("a floor fits a usize"),
            self.state.players.len(),
        )
    }
}

/// The run as one seat sees it, which is what its observation and its
/// events read.
#[derive(Debug)]
pub(crate) struct AtSeat<'a, V: Rules> {
    pub(crate) state: &'a State<V>,
    pub(crate) seat: sts2_core::SeatIndex,
}

impl<V: Rules> Clone for AtSeat<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for AtSeat<'_, V> {}

impl<V: Rules> State<V> {
    /// The run as `seat` sees it.
    /// `MapSelectionSynchronizer.PlayerVotedForMapCoord`: `seat`'s vote to
    /// travel to `to`, in a run of several players.
    ///
    /// Once every player has voted, the host draws one of the votes, in seat
    /// order, off its `map_point_selection` stream. It draws even where the
    /// votes agree. Then the run travels there.
    pub(crate) fn vote(&mut self, seat: sts2_core::SeatIndex, to: Coord) -> Result<Decided, Error> {
        use sts2_core::rng::GameRng as _;

        self.players[seat].vote = Some(to);
        let votes: Option<SmallVec<[Coord; 4]>> =
            self.players.iter_mut().map(|player| player.vote).collect();
        let Some(votes) = votes else {
            return Ok(Decided::Resolved);
        };
        for player in self.players.iter_mut() {
            player.vote = None;
        }
        let chosen = *self
            .map_votes
            .choose(&votes)
            .expect("every player voted somewhere");
        let host = sts2_core::SeatIndex(0);
        let moving = self.resolve(host, chosen).unwrap_or_else(|reason| {
            unreachable!("a voted point the host cannot reach: {reason:?}")
        });
        self.travel(host, moving)
    }

    /// How many players the run has.
    pub(crate) fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Every seat, in order.
    pub(crate) fn seats(&self) -> impl Iterator<Item = sts2_core::SeatIndex> + use<V> {
        self.players.seats()
    }

    pub(crate) fn seen_by(&self, seat: sts2_core::SeatIndex) -> AtSeat<'_, V> {
        AtSeat { state: self, seat }
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
    seat: sts2_core::SeatIndex,
    phase: &'a mut Phase<V>,
    streams: &'a mut Streams<V>,
    stream: sts2_core::Stream<V>,
) -> &'a mut MegaRng {
    match (stream, phase) {
        (sts2_core::Stream::Event, Phase::Event { seats }) => &mut seats[seat].rng,
        (sts2_core::Stream::Event, _) => unreachable!("an event's stream outside its room"),
        (stream, _) => streams.of(seat, stream),
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

/// What `Hook.ModifyCardRewardCreationOptions` made of a card reward's
/// options.
enum Hooked<V: Rules> {
    /// `WithCustomPool` of these at `CardRarityOddsType.Uniform`.
    Uniform(Vec<V::Card>),
    /// The pools the options draw from, and the character whose unlocked
    /// cards `Concat` put after them.
    Pools {
        pools: SmallVec<[crate::reward::card::Source<V>; 6]>,
        concat: Option<V::Character>,
    },
}

/// Who asked a copy into the deck: a relic by its slot on the belt, or a run
/// modifier by its place among the run's.
#[derive(Debug, Clone, Copy)]
enum Copier {
    /// The relic in a slot of a seat's belt.
    Relic(sts2_core::SeatIndex, usize),
    Modifier(usize),
}

#[cfg(test)]
mod tests {
    /// The one seat of the solo runs these play.
    const SOLO: sts2_core::SeatIndex = sts2_core::SeatIndex(0);

    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::ascension::AscensionLevel;
    use sts2_content::v0_107_1::characters::Character;
    use sts2_core::Seed;

    use super::*;

    type V = V0_107_1;

    fn opened() -> State<V> {
        State::start(
            &Seed::new("0RACLE1").expect("the fixture's seed is a seed"),
            &[sts2_core::Player {
                character: Character::Regent,
                wins: 0,
            }],
            AscensionLevel::None,
            &[],
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
        run.grant(SOLO, &Gift::Potion(Potion::BlockPotion))
            .expect("a potion can be granted");
        run.fight_here(SOLO, Encounter::SlimesWeak)
            .expect("the simulator can build this fight");

        // Where the player fell, which is what the run keeps.
        let mut combat = run
            .fight()
            .expect("the run is in the fight")
            .report(SOLO, &run.belts);
        let player = combat
            .creatures
            .iter_mut()
            .find(|c| c.side == <V as sts2_core::ContentVersion>::CombatSide::PLAYER)
            .expect("a fight has the player in it");
        player.hp = 0;
        player.alive = false;
        run.lost(vec![combat], None);

        let seen = run.report(SOLO).observation;
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
        run.players[SOLO].hp = 3;
        run.damage_owner(SOLO, 13);

        let seen = run.report(SOLO).observation;
        assert!(!seen.alive, "the player is dead");
        assert_eq!(seen.hp, 0, "with no hit points left");
        assert!(seen.run_over, "so the run is over");
        assert_eq!(seen.victory, Some(false), "and it was not a victory");
        assert!(run.legal(SOLO).is_empty(), "and it takes nothing more");
    }

    /// Past the last act's boss the run enters The Architect, on the floor it
    /// stood on. Its last option wins: `WinRun` kills every player, and the
    /// room it ends in is the victory room.
    #[test]
    fn a_run_past_its_last_act_meets_the_architect_and_wins() {
        let mut run = opened();
        run.act = run.shared.opening.acts.len() - 1;
        let floor = run.floor;
        run.proceed(SOLO)
            .expect("the last act's boss room is left by proceeding");
        assert!(
            run.in_victory_room(SOLO),
            "the run stands in The Architect's room"
        );
        assert_eq!(run.floor, floor, "which stands on no floor of its own");

        // A Regent with no wins hears three lines: two to answer, then
        // `PROCEED`.
        for _ in 0..3 {
            assert!(
                !run.report(SOLO).observation.run_over,
                "the run is not over yet"
            );
            run.choose(SOLO, 0).expect("its one option is taken");
        }

        let seen = run.report(SOLO).observation;
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
        run.grant(SOLO, &Gift::Relic(Relic::MiniatureTent))
            .expect("a relic can be granted");
        run.phase = Phase::RestSite {
            seats: crate::seats::BySeat::new(
                RestSeat::offering(vec![
                    <V as sts2_core::ContentVersion>::RestSiteOption::HEAL,
                    <V as sts2_core::ContentVersion>::RestSiteOption::SMITH,
                ]),
                Vec::new(),
            ),
        };
        let travels = |run: &State<V>| {
            run.legal(SOLO)
                .iter()
                .any(|decision| matches!(decision, Decision::Travel { .. }))
        };
        assert!(
            !travels(&run),
            "nothing leaves before an option goes through"
        );

        run.choose(SOLO, 0).expect("the heal can be chosen");
        let legal = run.legal(SOLO);
        assert!(
            legal.contains(&Decision::ChooseOption {
                index: 0,
                target: None
            }),
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
            .map(|card| sts2_core::DeckCard::new(run.players[SOLO].mint(), card))
            .collect();
        run.players[SOLO].deck = Arc::new(deck);
        run.phase = Phase::RestSite {
            seats: crate::seats::BySeat::new(
                RestSeat::offering(vec![
                    <V as sts2_core::ContentVersion>::RestSiteOption::HEAL,
                    <V as sts2_core::ContentVersion>::RestSiteOption::SMITH,
                ]),
                Vec::new(),
            ),
        };

        run.choose(SOLO, 1).expect("the smith can be chosen");
        let parked = run.report(SOLO);
        let Some(Question::SelectCards { options, .. }) = parked.asking else {
            panic!("the smith asks which card to upgrade: {:?}", parked.asking);
        };
        assert_eq!(
            options.iter().map(|option| option.card).collect::<Vec<_>>(),
            [strike, big_bang],
            "the copy already upgraded is not offered"
        );

        run.answer(SOLO, &Answer::Cards([1].into()))
            .expect("the second offered card can be upgraded");
        assert_eq!(
            run.players[SOLO]
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
        let before = run.report(SOLO).observation;
        run.fight_here(SOLO, Encounter::SlimesWeak)
            .expect("the simulator can build this fight");
        let after = run.report(SOLO).observation;

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
        run.belts[SOLO].push(HeldRelic::new(Relic::BoneTea));
        // A deck of the one card whose upgrade is ported. What upgrading does
        // to a card is the card's own rule, and this is about the relic that
        // asks for it.
        let deck = std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 10)
            .map(|card| sts2_core::DeckCard::new(run.players[SOLO].mint(), card))
            .collect();
        run.players[SOLO].deck = Arc::new(deck);

        let mut opening_hands = Vec::new();
        for _ in 0..2 {
            run.fight_here(SOLO, Encounter::SlimesWeak)
                .expect("the simulator can build this fight");
            let combat = run
                .report(SOLO)
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
            run.belts[SOLO]
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
        run.belts[SOLO].push(HeldRelic::new(Relic::Pocketwatch));
        let deck = std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 20)
            .map(|card| sts2_core::DeckCard::new(run.players[SOLO].mint(), card))
            .collect();
        run.players[SOLO].deck = Arc::new(deck);
        run.fight_here(SOLO, Encounter::SlimesWeak)
            .expect("the simulator can build this fight");

        let shown = |run: &State<V>| {
            let observation = run.report(SOLO).observation;
            let relic = observation
                .relics
                .iter()
                .find(|carried| carried.relic == Relic::Pocketwatch)
                .expect("the belt holds the relic");
            relic.amount
        };
        assert_eq!(shown(&run), 0, "nothing has been played yet");

        run.play_card(SOLO, hand(0), Some(at(1)))
            .expect("a Strike is ported")
            .expect("and is legal here");
        assert_eq!(shown(&run), 1, "one card played this turn");

        run.end_turn(SOLO)
            .expect("the simulator can end this turn")
            .expect("and the turn is not refused");
        let combat = run
            .report(SOLO)
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
            .map(|card| sts2_core::DeckCard::new(run.players[SOLO].mint(), card))
            .collect();
        run.players[SOLO].deck = Arc::new(deck);
        // The hopper's graph opens on `ThieveryMove`.
        run.fight_here(SOLO, Encounter::ThievingHopperWeak)
            .expect("the simulator can build this fight");

        run.end_turn(SOLO)
            .expect("the simulator can end this turn")
            .expect("and the turn is not refused");
        assert_eq!(
            run.players[SOLO].deck.len(),
            19,
            "the hopper took one Strike"
        );

        run.play_card(SOLO, hand(0), Some(at(1)))
            .expect("a Strike is ported")
            .expect("and is legal here");
        assert_eq!(
            run.players[SOLO].deck.len(),
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
        run.fight_here(SOLO, Encounter::SlimesWeak)
            .expect("the simulator can build this fight");
        let before = run.report(SOLO).observation;
        run.end_turn(SOLO)
            .expect("the simulator can end this turn")
            .expect("and the turn is not refused");
        let after = run.report(SOLO).observation;

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
            .choose(SOLO, 0)
            .expect("the simulator implements this option");
        assert_eq!(run.belts[SOLO].len() + 1, branch.belts[SOLO].len());
    }
}
