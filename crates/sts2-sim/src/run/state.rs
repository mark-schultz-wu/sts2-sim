use std::sync::Arc;

mod observed;

use crate::bounded::Bounded;
use crate::rng::MegaRng;
use num_traits::ToPrimitive as _;
use smallvec::SmallVec;
use sts2_core::ActMap as _;
use sts2_core::rng::stream_seed;
use sts2_core::{
    ActTable, Answer, CardIdent, CardOption, Carried, Combat, Coord, Decided, Decision,
    EventOption, EventOptionKey, EventOptionKinds, Map, Observation, PlayerStreamKinds, Positions,
    Question, Reachable, RelicModel as _, RewardCard, RewardDetail, RewardKind, Room, Rules,
    RunStreamKinds, Seed, Snapshot, Table, TakeReward,
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
}

impl<V: Rules> Clone for HeldRelic<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for HeldRelic<V> {}

impl<V: Rules> HeldRelic<V> {
    /// The relic as a run picks it up, with its model as new as it is.
    pub(crate) fn new(relic: V::Relic) -> Self {
        Self {
            relic,
            state: <V::RelicState as sts2_core::RelicModel<V>>::new(relic),
        }
    }

    /// The number it shows, which is `RelicModel.DisplayAmount`.
    pub(crate) fn amount(&self) -> V::Number {
        self.state.display_amount(self.relic)
    }
}

/// The small, mutable part of one player's run state.
#[derive(Debug)]
struct Player<V: Rules> {
    relics: Bounded<HeldRelic<V>, RELICS>,
    /// The belt, in physical slot order.
    potions: Bounded<V::Potion, BELT>,
    /// The physical slot beside each potion. Empty slots survive drinking.
    potion_positions: Bounded<u8, BELT>,
    /// Shared until one is written. Every write is a room's: a shop, a
    /// reward, a rest site. Nothing inside a fight moves the deck, so a
    /// search expanding a fight copies the pointer and not the cards.
    deck: Arc<Vec<CardIdent<V>>>,
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
            player: self.player.clone(),
            visited_events: Arc::clone(&self.visited_events),
            dealt_relics: Arc::clone(&self.dealt_relics),
            phase: self.phase.clone(),
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
        } = self;
        shared.clone_from(&source.shared);
        map.clone_from(&source.map);
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
        match (phase, &source.phase) {
            (Phase::Combat(live), Phase::Combat(from)) => live.clone_from(from),
            (phase, from) => *phase = from.clone(),
        }
    }
}

impl<V: Rules> Clone for Player<V> {
    fn clone(&self) -> Self {
        Self {
            deck: Arc::clone(&self.deck),
            ..*self
        }
    }

    fn clone_from(&mut self, source: &Self) {
        let Player {
            relics,
            potions,
            potion_positions,
            deck,
            hp,
            max_hp,
            gold,
            potion_slots,
        } = self;
        relics.copy_from(&source.relics);
        potions.copy_from(&source.potions);
        potion_positions.copy_from(&source.potion_positions);
        deck.clone_from(&source.deck);
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
    fn deck_mut(&mut self) -> &mut Vec<CardIdent<V>> {
        Arc::make_mut(&mut self.deck)
    }
}

/// How many relics a run carries.
const RELICS: usize = 64;

/// How many potion slots a belt has.
const BELT: usize = 8;

/// How many map points one act's run stands on. An act is shorter than this.
const VISITED: usize = 32;

/// Exactly one room or pending action can own the run at a time.
#[derive(Debug, Clone)]
pub(super) struct EnteredEvent<V: Rules> {
    pub event: V::Event,
    pub state: V::EventState,
    pub rng: MegaRng,
    pub streams: Streams<V>,
    pub events_used: usize,
    pub unknown_odds: [f32; 3],
}

#[derive(Debug, Clone)]
pub(super) enum Phase<V: Rules> {
    Neow {
        options: SmallVec<[V::Relic; 4]>,
        chosen: Option<u8>,
        selection: Option<SmallVec<[V::Card; 4]>>,
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
    },
    Event {
        state: V::EventState,
        rng: MegaRng,
    },
    Treasure {
        relic: V::Relic,
        finished: bool,
    },
    /// A rest site, and how far through its one option the run is.
    ///
    /// The options empty when the chosen one resolves. A rest site offers one
    /// choice, and the game stops offering any once it is taken. That is what
    /// an observation reports, rather than a flag beside them.
    RestSite {
        options: Vec<V::RestSiteOption>,
        /// Waiting for the card the smith upgrades.
        upgrading: bool,
    },
    /// The fight the run died in, as it stood when the player fell.
    ///
    /// `ProcessPendingLoss` ends the combat and the run with it. The room is
    /// still the one it was fought in, so the fight is still reported. Nothing
    /// is offered, because a dead run takes nothing.
    Lost {
        combat: Combat<V>,
    },
    /// A merchant, and how far through its shelves the run has bought.
    ///
    /// The entries are stocked once, when the run walks in. A slot the run
    /// bought stops being stocked rather than leaving the list, because the
    /// index the next decision names is the slot's own.
    Shop {
        entries: Vec<sts2_core::Entry<V>>,
        /// The purchase this merchant has not finished.
        unfinished: Option<Unfinished>,
    },
}

/// What a relic's pickup is still asking the run for.
///
/// `AfterObtained` does not return until the answer does, so the room the
/// relic was picked up in is what parks on it. A room that cannot park on one
/// says so rather than answer it itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pickup {
    /// `CardSelectCmd.FromDeckGeneric`, and a clone of what it takes. The card
    /// type the relic's own filter leaves out comes with it.
    CloneDeckCard { excluding: sts2_core::CardKind },
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
    /// clears no shelf, so it names none.
    Removal,
    /// A relic's pickup, waiting for the card it copies. The shelf it came off
    /// clears when the answer lands.
    Pickup {
        shelf: u8,
        excluding: sts2_core::CardKind,
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
            Phase::Neow {
                chosen, selection, ..
            } => chosen.is_some() && selection.is_none(),
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
            | Phase::Event { .. } => false,
        }
    }
}

impl<V: Rules> State<V> {
    /// The question that blocks every decision except its answer.
    pub(crate) fn pending(&self) -> Option<sts2_core::QuestionKind> {
        match &self.phase {
            Phase::Combat(live) => live.question().map(|question| question.kind()),
            Phase::Neow { selection, .. } if selection.is_some() => {
                Some(sts2_core::QuestionKind::SelectCards)
            }
            Phase::Rewards { offered, .. } if offered.iter().any(|offer| !offer.taken()) => {
                Some(sts2_core::QuestionKind::Rewards)
            }
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

    pub(crate) fn start(
        seed: &Seed,
        character: V::Character,
        ascension: V::AscensionLevel,
    ) -> Self {
        let raw_seed = stream_seed::<V>(seed);
        let acts: Vec<V::Act> = V::Act::ALL
            .iter()
            .copied()
            .filter(|act| act.played())
            .collect();
        let mut streams = Streams::<V>::opening(raw_seed);
        let opening = Opening::draw(streams.run(V::RunRngType::UP_FRONT), character, &acts);

        let map = Self::act_map(raw_seed, acts[0], 0, ascension);
        let at = map
            .ancient()
            .expect("a generated act map opens somewhere")
            .at;

        let mut neow_rng: MegaRng = Blessing::<V>::stream(raw_seed, 0);
        let neow_options = Blessing::<V>::roll(&mut neow_rng, 1).options;

        Self {
            shared: Arc::new(RunData {
                opening,
                raw_seed,
                character,
                ascension,
            }),
            map: Arc::new(map),
            act: 0,
            floor: num_traits::one(),
            visited: [at].into_iter().collect(),
            player: Player {
                relics: V::starting_relics(character)
                    .into_iter()
                    .map(HeldRelic::new)
                    .collect(),
                potions: Bounded::new(),
                potion_positions: Bounded::new(),
                deck: Arc::new(
                    V::starting_deck(character)
                        .into_iter()
                        .map(CardIdent::new)
                        .collect(),
                ),
                hp: V::starting_hp(character),
                max_hp: V::starting_hp(character),
                gold: num_traits::cast(V::starting_gold(character))
                    .expect("starting gold fits the build's game-number type"),
                potion_slots: num_traits::cast(V::starting_potion_slots())
                    .expect("starting potion slots fit the build's count type"),
            },
            normal_fights: 0,
            elite_fights: 0,
            events_used: 0,
            visited_events: Arc::new(Vec::new()),
            unknown_odds: UNKNOWN_ODDS,
            odds: Odds::default(),
            dealt_relics: Arc::new(Vec::new()),
            removals_used: 0,
            streams,
            phase: Phase::Neow {
                options: neow_options.into_iter().collect(),
                chosen: None,
                selection: None,
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
    fn rest_option_enabled(&self, option: V::RestSiteOption) -> bool {
        use sts2_core::RestKinds;

        if option != <V::RestSiteOption as RestKinds<V>>::SMITH {
            return true;
        }
        self.upgradable().next().is_some()
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
            .copied()
            .enumerate()
            .filter(|(_, card)| card.upgrade < card.card.max_upgrade())
    }

    /// Take the potion at this index out of the belt, which is
    /// `RemoveBeforeUse`.
    pub(super) fn drank(&mut self, at: usize) {
        self.player.potions.remove(at);
        self.player.potion_positions.remove(at);
    }

    /// Put a potion in the first empty physical belt slot.
    fn procure(&mut self, potion: V::Potion) -> bool {
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

    /// Take the fight out of the run, for a decision that ended it.
    ///
    /// What replaces it is the caller's to say, straight after.
    pub(super) fn left_fight(&mut self) -> Box<crate::combat::Live<V>> {
        match std::mem::replace(&mut self.phase, Phase::Map) {
            Phase::Combat(live) => live,
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
        let Phase::Combat(live) = &self.phase else {
            return;
        };
        for card in &live.removed_from_deck()[since..] {
            let Some(at) = self.player.deck.iter().position(|held| held == card) else {
                continue;
            };
            self.player.deck_mut().remove(at);
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
        self.phase = Phase::Rewards { combat, offered };
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

    /// Which act the run stands in, counted from zero.
    pub(super) fn act(&self) -> usize {
        self.act
    }

    /// `RunManager.EnterNextAct`, which the finished act's boss room offers.
    ///
    /// `SetActInternal` moves the index on and forgets the points the last act
    /// visited. It puts the question mark odds back where they started, and
    /// generates the new act's map. The run then stands on the map itself
    /// rather than in a room, which is what an act with nothing visited is.
    ///
    /// # Panics
    ///
    /// A run that has beaten the last act's boss, which wins it.
    pub(crate) fn proceed(&mut self) -> Result<Decided, Error> {
        let next = self.act + 1;
        let (act, _) = *self
            .shared
            .opening
            .acts
            .get(next)
            .unwrap_or_else(|| unimplemented!("a run that has no act left to enter"));
        self.map = Arc::new(Self::act_map(
            self.shared.raw_seed,
            act,
            next,
            self.ascension(),
        ));
        self.act = next;
        self.visited.clear();
        // `Odds.UnknownMapPoint.ResetToBase`, which is the only one of the
        // run's odds an act puts back.
        self.unknown_odds = UNKNOWN_ODDS;
        // Each act deals its own encounters and is met from the front of its
        // own list.
        self.normal_fights = 0;
        self.elite_fights = 0;
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
    pub(super) fn obtain(&mut self, relic: V::Relic) -> Option<Pickup> {
        let mut held = HeldRelic::new(relic);
        let effects = crate::combat::obtained::<V>(&mut held);
        self.player.relics.push(held);
        let mut asking = None;
        for effect in effects {
            match effect {
                sts2_core::HookEffect::UpgradeRandomDeckCards { count, stream } => {
                    self.upgrade_random_deck_cards(count, stream);
                }
                sts2_core::HookEffect::CloneDeckCard { excluding } => {
                    asking = Some(Pickup::CloneDeckCard { excluding });
                }
                effect => unimplemented!("a run-level relic effect: {effect:?}"),
            }
        }
        asking
    }

    /// Pick a relic up where nothing can park on what its pickup asks.
    ///
    /// Every room but the shop hands a relic over with no way to suspend the
    /// handover. A pickup that asks a question there stops by name rather than
    /// be answered by whoever happens to be holding the run.
    fn obtain_here(&mut self, relic: V::Relic) {
        if let Some(pickup) = self.obtain(relic) {
            unimplemented!("{relic:?} asking for {pickup:?} where it was picked up");
        }
    }

    /// Put the run back to the health it started with.
    ///
    /// `CreatureCmd.Heal(MaxHp - CurrentHp)`, which is what an ancient gives a
    /// run before it offers anything.
    pub(super) fn heal_fully(&mut self) {
        self.player.hp = self.player.max_hp;
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
        self.floor = self.floor + num_traits::one();
        if let Location::MapPoint(to) = at {
            self.visited.push(to);
        }
        self.phase = phase;
    }

    pub(super) fn enter_event(&mut self, to: Coord, entered: EnteredEvent<V>) {
        let EnteredEvent {
            event,
            state,
            rng,
            streams,
            events_used,
            unknown_odds,
        } = entered;
        self.streams = streams;
        self.events_used = events_used;
        self.unknown_odds = unknown_odds;
        Arc::make_mut(&mut self.visited_events).push(event);
        self.enter(Location::MapPoint(to), Phase::Event { state, rng });
    }

    /// Enter a treasure room after resolving its relic and stream counter.
    pub(super) fn enter_treasure(&mut self, to: Coord, relic: V::Relic, streams: Streams<V>) {
        self.streams = streams;
        Arc::make_mut(&mut self.dealt_relics).push(relic);
        self.enter(
            Location::MapPoint(to),
            Phase::Treasure {
                relic,
                finished: false,
            },
        );
    }

    /// `MerchantEntry.OnTryPurchaseWrapper`: buy the entry at an index.
    ///
    /// Stock and gold are the two checks the wrapper makes, and both are
    /// already reported. A slot it sells clears rather than restocks, because
    /// `ShouldRefillMerchantEntry` is a hook and no ported relic answers it.
    ///
    /// The card removal is the one entry that does not clear. Its price climbs
    /// with every removal the run has bought, and it stays on the shelf.
    pub(crate) fn buy(&mut self, index: u8) -> Result<Decided, Error> {
        let Phase::Shop { entries, .. } = &self.phase else {
            unimplemented!("buying outside a shop");
        };
        let Some(entry) = entries.iter().find(|entry| entry.index == index) else {
            unimplemented!("buying a shelf this merchant does not have");
        };
        if !entry.stocked {
            unimplemented!("buying a shelf this merchant has sold");
        }
        if entry.cost > self.player.gold {
            unimplemented!("buying a shelf this run cannot afford");
        }
        let (stock, cost) = (entry.stock, entry.cost);

        // `PlayerCmd.LoseGold` runs after the entry's own purchase in the
        // game. Nothing between the two reads the gold, and a purchase that
        // cannot be carried out panics rather than returns, so it is spent
        // here.
        self.player.gold = self.player.gold - cost;
        let asking = match stock {
            sts2_core::Stock::Card(Some(on_sale)) => {
                self.player.deck_mut().push(on_sale.card);
                None
            }
            sts2_core::Stock::Relic(Some(relic)) => {
                self.obtain(relic)
                    .map(|Pickup::CloneDeckCard { excluding }| Unfinished::Pickup {
                        shelf: index,
                        excluding,
                    })
            }
            sts2_core::Stock::Potion(Some(potion)) => {
                if !self.procure(potion) {
                    unimplemented!("buying a potion with no slot to put it in");
                }
                None
            }
            // `DoLocalMerchantCardRemoval` opens a cancelable selection and
            // spends the gold once a card comes back, so nothing is spent on
            // this one until then.
            sts2_core::Stock::CardRemoval => {
                self.player.gold = self.player.gold + cost;
                Some(Unfinished::Removal)
            }
            stock => unimplemented!("buying {stock:?}"),
        };

        let Phase::Shop {
            entries,
            unfinished,
        } = &mut self.phase
        else {
            unreachable!("the phase was a shop when this began");
        };
        // `ClearAfterPurchase` runs once the entry's own purchase returns, so a
        // shelf whose purchase is still asking stays stocked until it does.
        match asking {
            Some(_) => *unfinished = asking,
            None => sold(entries, index),
        }
        // A purchase that has not returned leaves the decision unresolved,
        // which is what the game reports.
        Ok(match asking {
            Some(_) => Decided::Abandoned,
            None => Decided::Resolved,
        })
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
            Unfinished::Removal => self.removed(index),
            Unfinished::Pickup { shelf, excluding } => self.cloned(index, shelf, excluding),
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
    fn cloned(&mut self, index: usize, shelf: u8, excluding: sts2_core::CardKind) {
        let offered = self.clonable(excluding);
        let Some(card) = offered.get(index).copied() else {
            unimplemented!("a clone naming a card that was not offered");
        };
        self.player.deck_mut().push(card);
        // `OnTryPurchaseWrapper` has returned now, so `ClearAfterPurchase`
        // runs and the shelf the relic came off empties.
        let Phase::Shop { entries, .. } = &mut self.phase else {
            unreachable!("the phase was a shop when this began");
        };
        sold(entries, shelf);
    }

    /// Take the card the merchant's removal was pointed at out of the deck.
    ///
    /// The selection offers the whole deck, so the index is the deck's.
    fn removed(&mut self, index: usize) {
        if index >= self.player.deck.len() {
            unimplemented!("a removal naming a card the deck does not hold");
        }
        self.player.deck_mut().remove(index);
        // `DoLocalMerchantCardRemoval` spends the gold once the card comes
        // back, and `CardShopRemovalsUsed` climbs over the run.
        let spent = removal_cost::<V>(self.removals_used);
        self.player.gold = self.player.gold - spent;
        self.removals_used += 1;
        // The entry itself is left where it was. `CalcCost` runs in its
        // constructor and nowhere else, and `ClearAfterPurchase` is empty, so
        // this merchant still shows it stocked and at the price it opened on.
        // The next merchant reads the raised count.
    }

    /// The deck as a pickup's selection offers it.
    fn clonable(&self, excluding: sts2_core::CardKind) -> Vec<CardIdent<V>> {
        self.player
            .deck
            .iter()
            .copied()
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
        Phase::Shop {
            entries,
            unfinished: None,
        }
    }

    /// Enter a rest site, with the options this run's belt and deck open it on.
    pub(super) fn enter_rest_site(&mut self, to: Coord) {
        let deck = self.player.deck.to_vec();
        let options =
            <V::RestSiteOption as sts2_core::RestKinds<V>>::offered(&self.held_relics(), &deck);
        self.enter(
            Location::MapPoint(to),
            Phase::RestSite {
                options,
                upgrading: false,
            },
        );
    }

    /// The belt as the models the hook reads, rather than what a fight writes.
    fn held_relics(&self) -> Vec<V::Relic> {
        self.player.relics.iter().map(|held| held.relic).collect()
    }

    /// Enter a fight that is already built, where the run reached it.
    ///
    /// The act counts the fight here rather than where it was built, because
    /// the count is how far down its list the run has walked. A fight the run
    /// never walked into moves no list on.
    pub(super) fn enter_fight(&mut self, fight: CreatedFight<V>, at: Location) {
        let CreatedFight {
            live,
            streams,
            queue,
        } = fight;
        match queue {
            EncounterQueue::Normal => self.normal_fights += 1,
            EncounterQueue::Elite => self.elite_fights += 1,
            EncounterQueue::Boss | EncounterQueue::Named => {}
        }
        self.drew(streams);
        self.enter(at, Phase::Combat(live));
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
    pub(super) fn create_fight(
        &mut self,
        encounter: V::Encounter,
        queue: EncounterQueue,
    ) -> Result<CreatedFight<V>, Error> {
        let standing = self.standing();
        let (raw_seed, character, ascension) =
            (self.raw_seed(), self.character(), self.ascension());
        let entering = crate::combat::Entering {
            // The run steps up to the floor it fights on, and the encounter
            // is rolled against that one rather than the one behind it.
            floor: self.floor + num_traits::one(),
            hp: standing.hp,
            max_hp: standing.max_hp,
            alive: standing.alive,
            deck: &self.player.deck,
        };
        let (live, streams) = crate::combat::enter::<V>(
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
        })
    }

    /// Put the run into a named fight, rather than travel to one.
    ///
    /// `FightConsoleCmd`, which is `EnterRoomDebug` at
    /// `MapPointType.Unassigned`. The floor advances and the map does not, so
    /// map and room disagree by construction. That is what the command is for:
    /// reaching a chosen fight without hunting for a seed that puts one on the
    /// way. The encounter is named, so the act's own list is not drawn from.
    pub(crate) fn fight_here(&mut self, encounter: V::Encounter) -> Result<Decided, Error> {
        let fight = self.create_fight(encounter, EncounterQueue::Named)?;
        self.enter_fight(fight, Location::NoLocation);
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

    /// Put a card, a relic or a potion straight into the run.
    ///
    /// `Driver.GrantCard`, `RelicCmd.Obtain` and `PotionCmd.TryToProcure`,
    /// which are the paths ordinary play uses. A relic arrives through the
    /// command that runs its `AfterObtained`. The simulator runs no relic's
    /// hooks, so it cannot answer for one.
    pub(crate) fn grant(&mut self, gift: &Gift<V>) -> Result<Decided, Error> {
        match gift {
            Gift::Card(card) => self.player.deck_mut().push(CardIdent::new(*card)),
            Gift::Potion(potion) => {
                // `TryToProcure` refuses a full belt rather than growing one,
                // and the belt in the next observation says whether it landed.
                self.procure(*potion);
            }
            Gift::Relic(relic) => self.obtain_here(*relic),
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
            let relic = *relic;
            *finished = true;
            match index {
                0 => self.obtain_here(relic),
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
            if option == <V::RestSiteOption as sts2_core::RestKinds<V>>::HEAL {
                let max_hp: u64 = num_traits::cast(self.player.max_hp)
                    .expect("maximum health fits an unsigned rest calculation");
                let healed: V::Health = num_traits::cast(max_hp * 3 / 10)
                    .expect("rest healing fits the build's health type");
                self.player.hp = (self.player.hp + healed).min(self.player.max_hp);
                let Phase::RestSite { options, .. } = &mut self.phase else {
                    unreachable!("the phase was a rest site when this began");
                };
                options.clear();
                return Ok(Decided::Resolved);
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
        if matches!(self.phase, Phase::Event { .. }) {
            let room = match &self.phase {
                Phase::Event { state, .. } => V::observe_event(state, self),
                _ => unreachable!(),
            };
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
            let Phase::Event { mut state, mut rng } =
                std::mem::replace(&mut self.phase, Phase::Map)
            else {
                unreachable!()
            };
            let flow = V::choose_event(&mut state, index, self, &mut rng);
            self.phase = Phase::Event { state, rng };
            match flow {
                sts2_core::EventFlow::Finished => return Ok(Decided::Resolved),
                _ => unimplemented!("an event choice that does not finish"),
            }
        }
        let Phase::Neow {
            options, chosen, ..
        } = &mut self.phase
        else {
            unimplemented!("choosing an option inside a fight");
        };
        let option = options
            .get(index as usize)
            .unwrap_or_else(|| unimplemented!("an option the blessing does not offer"));
        if *option != V::lead_paperweight() {
            unimplemented!("what {option:?} does when Neow offers it");
        }

        *chosen = Some(index);
        // Not through `obtain`. The blessing is the one path that carries out
        // the paperweight's own `AfterObtained`, which is the colorless offer
        // below, and the relic's row carries no `UponPickup` besides.
        self.player
            .relics
            .push(HeldRelic::new(V::lead_paperweight()));
        let Phase::Neow { selection, .. } = &mut self.phase else {
            unreachable!("the phase was the blessing when this began")
        };
        *selection = Some(
            crate::reward::card::regular::<V, _>(
                self.streams.player(V::PlayerRngType::REWARDS),
                V::colorless_cards(),
                2,
            )
            .into_iter()
            .collect(),
        );
        Ok(Decided::Abandoned)
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
        if matches!(&self.phase, Phase::Shop { unfinished, .. } if unfinished.is_some()) {
            return self.finish_purchase(answer);
        }
        let Phase::Neow { selection, .. } = &mut self.phase else {
            unimplemented!("answering a question inside a fight");
        };
        let cards = selection
            .as_ref()
            .unwrap_or_else(|| unimplemented!("an answer to a question nobody asked"));
        let Answer::Cards(picks) = answer else {
            unimplemented!("an answer that is not a card selection");
        };
        if picks.len() > 1 {
            unimplemented!("a card selection of more than one");
        }
        let chosen: Option<Vec<_>> = picks
            .iter()
            .map(|index| cards.get(index).copied().map(CardIdent::new))
            .collect();
        let Some(chosen) = chosen else {
            unimplemented!("a card selection naming a card that was not offered");
        };

        self.player.deck_mut().extend(chosen);
        *selection = None;
        Ok(Decided::Resolved)
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
        self.player.deck_mut()[at].upgrade += 1;

        let Phase::RestSite {
            options, upgrading, ..
        } = &mut self.phase
        else {
            unreachable!("the phase was a rest site when this began");
        };
        options.clear();
        *upgrading = false;
        Ok(Decided::Resolved)
    }

    /// Take rewards from the screen a won fight put up.
    ///
    /// `RewardsSetSynchronizer.SelectLocalReward`, one entry at a time. A
    /// reward stays in the set once it is taken, because the indices an answer
    /// names never renumber.
    fn take(&mut self, answer: &Answer) -> Result<Decided, Error> {
        if let Answer::Skip = answer {
            let Phase::Rewards { offered, .. } = &mut self.phase else {
                unreachable!("the phase was a reward screen when this began");
            };
            for offer in offered {
                match offer {
                    Offer::Gold { taken, .. }
                    | Offer::Card { taken, .. }
                    | Offer::SpecialCard { taken, .. }
                    | Offer::Potion { taken, .. }
                    | Offer::Relic { taken, .. } => *taken = true,
                }
            }
            return Ok(Decided::Resolved);
        }
        let Answer::Take(take) = *answer else {
            unimplemented!("an answer the reward screen does not accept");
        };
        let slots: usize = num_traits::cast(self.player.potion_slots)
            .expect("the belt has an addressable number of slots");
        let Phase::Rewards { offered, .. } = &mut self.phase else {
            unreachable!("the phase was a reward screen when this began");
        };
        let offer = offered
            .get_mut(take.index())
            .unwrap_or_else(|| unimplemented!("a reward the screen does not offer"));
        match (offer, take) {
            (Offer::Gold { amount, taken }, TakeReward::NonCard { .. }) => {
                let amount = *amount;
                *taken = true;
                self.player.gold = self.player.gold + amount;
            }
            (Offer::Potion { potion, taken }, TakeReward::NonCard { .. }) => {
                let potion = *potion;
                // `PotionCmd.TryToProcure` fails on a full belt, and the
                // reward is then neither taken nor gone.
                if self.player.potions.len() >= slots {
                    unimplemented!("a potion offered to a full belt");
                }
                *taken = true;
                self.procure(potion);
            }
            (Offer::Card { cards, taken }, TakeReward::Card { card, .. }) => {
                let chosen = cards
                    .get(usize::from(card))
                    .copied()
                    .unwrap_or_else(|| unimplemented!("a card the reward does not offer"));
                *taken = true;
                self.player.deck_mut().push(chosen);
            }
            (Offer::SpecialCard { card, taken }, TakeReward::NonCard { .. }) => {
                let card = *card;
                *taken = true;
                self.player.deck_mut().push(card);
            }
            (Offer::Relic { relic, taken }, TakeReward::NonCard { .. }) => {
                let relic = *relic;
                *taken = true;
                self.obtain_here(relic);
            }
            _ => unimplemented!("a reward taken as the wrong kind"),
        }

        Ok(self.decided())
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
            Phase::Map
            | Phase::Neow { .. }
            | Phase::Rewards { .. }
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. }
            | Phase::Shop { .. } => Standing {
                hp: self.player.hp,
                max_hp: self.player.max_hp,
                alive: true,
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
        crate::combat::belt::<V>(&self.player.potions, self.fight(), self.standing().alive)
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
            return crate::combat::legal_moves::<V>(live, &self.player.potions, alive);
        }

        let mut moves = Legal::new();
        if self.answers(&mut moves) {
            return moves;
        }
        // No fight is live here, so a drink names no creature.
        crate::combat::drinks::<V>(&self.player.potions, None, alive, &mut moves);
        let before = moves.len();
        self.room_moves(&mut moves);
        if moves.len() > before {
            return moves;
        }

        if self.can_proceed() {
            moves.push(Decision::Proceed);
        } else {
            moves.extend(
                self.reachable()
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
            Phase::Neow {
                selection: Some(cards),
                ..
            } => selections(0, 1, cards.len(), moves),
            Phase::RestSite {
                upgrading: true, ..
            } => selections(1, 1, self.upgradable().count(), moves),
            Phase::Shop {
                unfinished: Some(unfinished),
                ..
            } => {
                let offered = match unfinished {
                    Unfinished::Removal => self.player.deck.len(),
                    Unfinished::Pickup { excluding, .. } => self.clonable(*excluding).len(),
                };
                selections(1, 1, offered, moves);
            }
            Phase::Rewards { offered, .. } if offered.iter().any(|offer| !offer.taken()) => {
                for (index, offer) in offered.iter().enumerate() {
                    if offer.taken() {
                        continue;
                    }
                    match offer {
                        Offer::Card { cards, .. } => moves.extend((0..cards.len()).map(|card| {
                            Decision::Answer(Answer::Take(TakeReward::card(index, card)))
                        })),
                        _ => {
                            moves.push(Decision::Answer(Answer::Take(TakeReward::non_card(index))))
                        }
                    }
                }
                // Nothing a fight offers disallows skipping. See `asked`.
                moves.push(Decision::Answer(Answer::Skip));
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
                    .filter(|&index| *chosen != Some(index))
                    .map(|index| Decision::ChooseOption { index }),
            ),
            // An event's options are the content's to spell, so its room is
            // built. That is the room alone, and only in an event.
            Phase::Event { state, .. } => {
                if let Room::Event { options, .. } = V::observe_event(state, self) {
                    moves.extend(
                        options
                            .iter()
                            .filter(|option| !option.locked && !option.chosen)
                            .map(|option| Decision::ChooseOption {
                                index: option.index,
                            }),
                    );
                }
            }
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
                                && entry.cost <= gold
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
        self.reachable().collect()
    }

    /// [`State::travelable`], lent rather than collected.
    fn reachable(&self) -> impl ExactSizeIterator<Item = Reachable<V>> + '_ {
        let map = &self.map;
        let from: &[Coord] = match self.visited.last() {
            Some(&at) => {
                &map.at(at)
                    .expect("the run stands on a point of its own map")
                    .to
            }
            None => std::slice::from_ref(
                &map.ancient()
                    .expect("a generated act map opens somewhere")
                    .at,
            ),
        };
        from.iter().map(move |&at| Reachable {
            at,
            kind: map.at(at).expect("a map edge leads to a node").kind,
        })
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
                // over. No room a run can reach is one yet, because the act
                // that ends a run in victory is not ported.
                victory: (!standing.alive).then_some(false),
                gold: self.player.gold,
                relics: self.carried().collect(),
                potions: self.belt(),
                potion_slots: self.player.potion_slots,
                deck: self.player.deck.to_vec(),
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
    fn can_proceed(&self) -> bool {
        let at_boss = self.visited.last().is_some_and(|&at| {
            self.map
                .at(at)
                .expect("the run stands on a point of its own map")
                .kind
                == <V::MapPointType as sts2_core::MapPointKinds>::BOSS
        });
        at_boss && !self.in_combat()
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
            Phase::Combat(live) => Some(live.report()),
            // `IsInProgress` is false once the loss is processed, and the
            // room it was fought in is still the room the run is standing in.
            // A won fight is still reported for the same reason: the run has
            // not left the room it was fought in.
            Phase::Lost { combat } | Phase::Rewards { combat, .. } => Some(combat.clone()),
            Phase::Map
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
            Phase::Neow { selection, .. } => {
                selection.as_ref().map(|cards| Question::SelectCards {
                    min: 0,
                    max: 1,
                    asker: None,
                    options: cards
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(index, card)| CardOption {
                            index,
                            card: CardIdent::new(card),
                            text: None,
                        })
                        .collect(),
                })
            }
            Phase::Combat(live) => live.question(),
            Phase::Rewards { offered, .. } => offered
                .iter()
                .any(|offer| !offer.taken())
                .then(|| asked::<V>(offered)),
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
            // whole deck, and a pickup reads the deck less what its own filter
            // leaves out. One card either way, and neither can be empty.
            Phase::Shop { unfinished, .. } => {
                let offered = match unfinished {
                    None => None,
                    Some(Unfinished::Removal) => Some(self.player.deck.to_vec()),
                    Some(Unfinished::Pickup { excluding, .. }) => Some(self.clonable(*excluding)),
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
            Phase::Map | Phase::Lost { .. } | Phase::Event { .. } | Phase::Treasure { .. } => None,
        }
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
                            chosen: *neow_chosen == Some(index),
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
            Phase::Event { state, .. } => Some(V::observe_event(state, self)),
            Phase::Treasure { relic, finished } => {
                let options = if *finished {
                    Vec::new()
                } else {
                    vec![
                        sts2_core::ChestOption {
                            index: 0,
                            take: sts2_core::Take::Relic(*relic),
                            chosen: false,
                        },
                        sts2_core::ChestOption {
                            index: 1,
                            take: sts2_core::Take::Skip,
                            chosen: false,
                        },
                    ]
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
                        affordable: entry.cost <= gold,
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
    fn upgrade_random_deck_cards(&mut self, count: usize, stream: V::RunRngType) {
        use sts2_core::CardTable as _;
        use sts2_core::rng::GameRng as _;

        // `CardSelectCmd` shuffles the upgradable cards alone, so a card
        // already at its highest neither takes a place nor moves the shuffle.
        let deck = self.player.deck.clone();
        let mut upgradable: Vec<usize> = self.upgradable().map(|(at, _)| at).collect();
        self.streams
            .run(stream)
            .stable_shuffle_by_key(&mut upgradable, |&at| {
                (deck[at].card.order(), deck[at].upgrade)
            });
        for at in upgradable.into_iter().take(count) {
            self.player.deck_mut()[at].upgrade += 1;
        }
    }
}

impl<V: Rules> sts2_core::EventView<V> for State<V> {
    fn act(&self) -> V::Count {
        num_traits::cast(self.act()).expect("an act index fits the build count")
    }

    fn gold(&self) -> V::Number {
        self.player.gold
    }

    fn has_potion(&self) -> bool {
        !self.player.potions.is_empty()
    }

    fn character(&self) -> V::Character {
        self.shared.character
    }

    fn relics(&self) -> impl Iterator<Item = V::Relic> + '_ {
        self.player.relics.iter().map(|held| held.relic)
    }

    fn deck(&self) -> &[CardIdent<V>] {
        &self.player.deck
    }
}

impl<V: Rules> sts2_core::EventContext<V> for State<V> {
    fn gold(&self) -> V::Number {
        self.player.gold
    }

    fn spend_gold(&mut self, amount: V::Number) -> bool {
        if self.player.gold < amount {
            return false;
        }
        self.player.gold = self.player.gold - amount;
        true
    }

    fn obtain_relic(&mut self, relic: V::Relic) {
        self.obtain_here(relic);
    }
}

impl<V: Rules> sts2_core::EventContext<V> for Player<V> {
    fn gold(&self) -> V::Number {
        self.gold
    }

    fn spend_gold(&mut self, amount: V::Number) -> bool {
        if self.gold < amount {
            return false;
        }
        self.gold = self.gold - amount;
        true
    }

    fn obtain_relic(&mut self, relic: V::Relic) {
        self.relics.push(HeldRelic::new(relic));
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

fn asked<V: Rules>(offered: &[Offer<V>]) -> Question<V> {
    Question::Rewards {
        // `RewardsSet` disallows skipping only where something says so, and
        // nothing a fight offers does.
        skippable: true,
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
                    Offer::Relic { relic, .. } => RewardKind::Relic {
                        relic: Some(*relic),
                        rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
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
                    },
                    Offer::SpecialCard { .. } => RewardKind::SpecialCard { card: None },
                },
            })
            .collect(),
    }
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
        let mut combat = run.fight().expect("the run is in the fight").report();
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

    /// `CardSelectCmd.FromDeckForUpgrade` offers the upgradable cards alone,
    /// so an answer's index is that list's rather than the deck's.
    ///
    /// The case that says which is a deck holding two copies of one card, one
    /// of them already upgraded. A selection over the whole deck lands on the
    /// copy before it.
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
        run.player.deck = Arc::new(vec![strike, upgraded, big_bang]);
        run.phase = Phase::RestSite {
            options: vec![
                <V as sts2_core::ContentVersion>::RestSiteOption::HEAL,
                <V as sts2_core::ContentVersion>::RestSiteOption::SMITH,
            ],
            upgrading: false,
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
            *run.player.deck,
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
        run.player.deck = Arc::new(
            std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 10).collect(),
        );

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
        run.player.deck = Arc::new(
            std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 20).collect(),
        );
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
        run.player.deck = Arc::new(
            std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 20).collect(),
        );
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
