use std::sync::Arc;

use crate::rng::MegaRng;
use num_traits::ToPrimitive as _;
use smallvec::SmallVec;
use sts2_core::ActMap as _;
use sts2_core::rng::stream_seed;
use sts2_core::{
    ActTable, Answer, Applied, CardIdent, CardOption, Carried, Combat, Coord, EventOption,
    EventOptionKey, EventOptionKinds, Map, Observation, PlayerStreamKinds, Positions, Question,
    Reachable, RelicModel as _, RewardCard, RewardDetail, RewardKind, Room, Rules, RunStreamKinds,
    Seed, Table, TakeReward,
};

use crate::combat::Error;
use crate::event::Blessing;
use crate::map::ActMap;
use crate::reward::{Odds, Offer};
use crate::run::{Opening, Rarity};
use crate::stream::Streams;
use sts2_core::Gift;

/// The authoritative state of the run slice implemented so far.
///
/// Reports are built from this state. They are never stored and edited as if
/// they were the run itself.
#[derive(Debug, Clone)]
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
    visited: Vec<Coord>,
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
    visited_events: Vec<V::Event>,
    unknown_odds: [f32; 3],
    odds: Odds,
    /// Every relic this run has dealt out of a bag, in the order it dealt
    /// them.
    ///
    /// The bags sit in the setup that every branch of a run shares. A branch
    /// that deals a relic must not deal it to the other, so what moves lives
    /// here. It is a handful over a run, not the whole table.
    dealt_relics: SmallVec<[V::Relic; 8]>,
    phase: Phase<V>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EncounterQueue {
    Normal,
    Elite,
    /// An act rolls one boss and fights it once, so a boss takes nothing from
    /// a queue and moves no count on.
    Boss,
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

/// One relic the run carries, and what its own model saves.
///
/// `RelicModel` is an instance per relic held, and its saved fields outlive
/// the fight they were written in. So the run carries them, and hands them
/// back whenever a hook asks that relic something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeldRelic<V: Rules> {
    pub(crate) relic: V::Relic,
    pub(crate) state: V::RelicState,
}

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
#[derive(Debug, Clone)]
struct Player<V: Rules> {
    relics: SmallVec<[HeldRelic<V>; 8]>,
    /// The belt, in physical slot order.
    potions: SmallVec<[V::Potion; 3]>,
    /// The physical slot beside each potion. Empty slots survive drinking.
    potion_positions: SmallVec<[usize; 3]>,
    deck: Vec<CardIdent<V>>,
    /// What the run carries between fights.
    ///
    /// The game keeps one number. Here a fight holds a copy, because a fight
    /// is resolved whole and thrown away if it cannot be. The creature is
    /// built from this and written back to it. A report reads the fight's
    /// while there is one.
    hp: V::Health,
    max_hp: V::Health,
    gold: V::Number,
    potion_slots: V::Count,
}

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
enum Phase<V: Rules> {
    Neow {
        options: SmallVec<[V::Relic; 4]>,
        chosen: Option<u8>,
        selection: Option<SmallVec<[V::Card; 4]>>,
    },
    /// The map itself, which an act opens on and no room owns.
    Map,
    Combat(crate::combat::Live<V>),
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
            // The map is not a room, so there is nothing to leave.
            Phase::Map | Phase::Combat(_) | Phase::Rewards { .. } | Phase::Event { .. } => false,
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
            _ => None,
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
            visited: vec![at],
            player: Player {
                relics: V::starting_relics(character)
                    .into_iter()
                    .map(HeldRelic::new)
                    .collect(),
                potions: SmallVec::new(),
                potion_positions: SmallVec::new(),
                deck: V::starting_deck(character)
                    .into_iter()
                    .map(CardIdent::new)
                    .collect(),
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
            visited_events: Vec::new(),
            unknown_odds: UNKNOWN_ODDS,
            odds: Odds::default(),
            dealt_relics: SmallVec::new(),
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
            | Phase::Event { .. }
            | Phase::Treasure { .. }
            | Phase::RestSite { .. } => None,
        }
    }

    /// Whether a rest site option can be taken.
    ///
    /// `RestSiteOption.IsEnabled` is true unless the option overrides it.
    /// `SMITH` does: it is off when the deck holds nothing left to upgrade.
    fn rest_option_enabled(&self, option: V::RestSiteOption) -> bool {
        use sts2_core::{CardTable as _, RestKinds};

        if option != <V::RestSiteOption as RestKinds<V>>::SMITH {
            return true;
        }
        self.player
            .deck
            .iter()
            .any(|card| card.upgrade < card.card.max_upgrade())
    }

    /// The belt as the relics in it stand, which a fight hands back written.
    pub(super) fn relics(&self) -> &[HeldRelic<V>] {
        &self.player.relics
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
        let Some(slot) = (0..slots).find(|slot| !self.player.potion_positions.contains(slot))
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

    /// Put the belt a fight handed back in place of the one it was given.
    pub(super) fn hold(&mut self, relics: Vec<HeldRelic<V>>) {
        self.player.relics = relics.into_iter().collect();
    }

    /// Put the fight a decision left behind in place of the one it acted on.
    ///
    /// Whole, rather than edited where it stands. A decision the simulator
    /// cannot carry out never reaches this, so the run keeps the fight it had.
    pub(super) fn fought(&mut self, live: crate::combat::Live<V>) {
        self.phase = Phase::Combat(live);
    }

    /// Remove the deck copies a combat command took permanently.
    pub(super) fn lose_cards(&mut self, cards: &[CardIdent<V>]) {
        for card in cards {
            let Some(at) = self.player.deck.iter().position(|held| held == card) else {
                continue;
            };
            self.player.deck.remove(at);
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
        self.dealt_relics.push(relic);
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
    pub(crate) fn proceed(&mut self) -> Result<Applied<V>, Error> {
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
        Ok(self.report())
    }

    /// `RelicCmd.Obtain`: the relic joins the belt, and its own pickup runs.
    ///
    /// The model's `AfterObtained` state is written as the relic is built. A
    /// row that carries `UponPickup` then changes the run itself.
    pub(super) fn obtain(&mut self, relic: V::Relic) {
        let mut held = HeldRelic::new(relic);
        let effects = crate::combat::obtained::<V>(&mut held);
        self.player.relics.push(held);
        for effect in effects {
            match effect {
                sts2_core::HookEffect::UpgradeRandomDeckCards { count, stream } => {
                    self.upgrade_random_deck_cards(count, stream);
                }
                effect => unimplemented!("a run-level relic effect: {effect:?}"),
            }
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

    pub(super) fn enter_event(&mut self, to: Coord, entered: EnteredEvent<V>) {
        let EnteredEvent {
            event,
            state,
            rng,
            streams,
            events_used,
            unknown_odds,
        } = entered;
        self.floor = self.floor + num_traits::one();
        self.visited.push(to);
        self.streams = streams;
        self.events_used = events_used;
        self.unknown_odds = unknown_odds;
        self.visited_events.push(event);
        self.phase = Phase::Event { state, rng };
    }

    /// Enter a treasure room after resolving its relic and stream counter.
    pub(super) fn enter_treasure(&mut self, to: Coord, relic: V::Relic, streams: Streams<V>) {
        self.floor = self.floor + num_traits::one();
        self.visited.push(to);
        self.streams = streams;
        self.dealt_relics.push(relic);
        self.phase = Phase::Treasure {
            relic,
            finished: false,
        };
    }

    /// Enter a rest site, with the options this run's belt and deck open it on.
    pub(super) fn enter_rest_site(&mut self, to: Coord) {
        self.floor = self.floor + num_traits::one();
        self.visited.push(to);
        let deck = self.player.deck.to_vec();
        self.phase = Phase::RestSite {
            options: <V::RestSiteOption as sts2_core::RestKinds<V>>::offered(
                &self.held_relics(),
                &deck,
            ),
            upgrading: false,
        };
    }

    /// The belt as the models the hook reads, rather than what a fight writes.
    fn held_relics(&self) -> Vec<V::Relic> {
        self.player.relics.iter().map(|held| held.relic).collect()
    }

    pub(super) fn enter_combat(
        &mut self,
        to: Coord,
        queue: EncounterQueue,
        streams: Streams<V>,
        live: crate::combat::Live<V>,
    ) {
        self.floor = self.floor + num_traits::one();
        match queue {
            EncounterQueue::Normal => self.normal_fights += 1,
            EncounterQueue::Elite => self.elite_fights += 1,
            EncounterQueue::Boss => {}
        }
        self.visited.push(to);
        self.drew(streams);
        self.phase = Phase::Combat(live);
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
    /// The relics act on a copy, which is kept only once the fight is built.
    /// A fight that cannot be built leaves the run where it stood. A relic
    /// that spent a charge on such a fight has spent nothing.
    pub(super) fn open_combat(
        &mut self,
        encounter: V::Encounter,
        observation: &Observation<V>,
    ) -> Result<(crate::combat::Live<V>, Streams<V>), Error> {
        let mut relics = self.player.relics.clone();
        let opened = crate::combat::enter::<V>(
            self.raw_seed(),
            self.character(),
            self.ascension(),
            encounter,
            observation,
            &self.streams,
            &mut relics,
        )?;
        self.player.relics = relics;
        Ok(opened)
    }

    /// Put the run into a named fight, rather than travel to one.
    ///
    /// `FightConsoleCmd`, which is `EnterRoomDebug` at
    /// `MapPointType.Unassigned`. The floor advances and the map does not, so
    /// map and room disagree by construction. That is what the command is for:
    /// reaching a chosen fight without hunting for a seed that puts one on the
    /// way. The encounter is named, so the act's own list is not drawn from.
    pub(crate) fn fight_here(&mut self, encounter: V::Encounter) -> Result<Applied<V>, Error> {
        let mut observation = self.report().observation;
        observation.floor = observation.floor + num_traits::one();
        let (ongoing, streams) = self.open_combat(encounter, &observation)?;

        self.floor = self.floor + num_traits::one();
        self.drew(streams);
        self.fought(ongoing);
        Ok(self.report())
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
    ) -> Result<Applied<V>, Error> {
        let Some(ongoing) = self.fight() else {
            unimplemented!("a power applied outside a fight");
        };
        let applied =
            crate::combat::apply_power(ongoing, at, power, amount, self.streams(), self.relics())?;

        self.drew(applied.streams);
        self.hold(applied.relics);
        self.fought(applied.live);
        Ok(self.report())
    }

    /// Put a card, a relic or a potion straight into the run.
    ///
    /// `Driver.GrantCard`, `RelicCmd.Obtain` and `PotionCmd.TryToProcure`,
    /// which are the paths ordinary play uses. A relic arrives through the
    /// command that runs its `AfterObtained`. The simulator runs no relic's
    /// hooks, so it cannot answer for one.
    pub(crate) fn grant(&mut self, gift: &Gift<V>) -> Result<Applied<V>, Error> {
        match gift {
            Gift::Card(card) => self.player.deck.push(CardIdent::new(*card)),
            Gift::Potion(potion) => {
                // `TryToProcure` refuses a full belt rather than growing one,
                // and the belt in the next observation says whether it landed.
                self.procure(*potion);
            }
            Gift::Relic(relic) => {
                self.obtain(*relic);
            }
        }
        Ok(self.report())
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

    pub(crate) fn choose(&mut self, index: u8) -> Result<Applied<V>, Error> {
        if let Phase::Treasure { relic, finished } = &mut self.phase {
            if *finished {
                unimplemented!("choosing from a finished treasure room");
            }
            let relic = *relic;
            *finished = true;
            match index {
                0 => self.obtain(relic),
                1 => {}
                _ => unimplemented!("an option the treasure room does not offer"),
            }
            return Ok(self.report());
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
                return Ok(self.report());
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
            let mut applied = self.report();
            applied.resolved = false;
            return Ok(applied);
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
                sts2_core::EventFlow::Finished => return Ok(self.report()),
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
        let mut applied = self.report();
        applied.resolved = false;
        Ok(applied)
    }

    pub(crate) fn answer(&mut self, answer: &Answer) -> Result<Applied<V>, Error> {
        if let Phase::Rewards { .. } = self.phase {
            return self.take(answer);
        }
        if matches!(&self.phase, Phase::Combat(live) if live.question().is_some()) {
            return self.answer_fight(answer);
        }
        if matches!(&self.phase, Phase::RestSite { upgrading, .. } if *upgrading) {
            return self.smith(answer);
        }
        let Phase::Neow { selection, .. } = &mut self.phase else {
            unimplemented!("answering a question inside a fight");
        };
        let cards = selection
            .as_ref()
            .unwrap_or_else(|| unimplemented!("an answer to a question nobody asked"));
        let Answer::Cards(indices) = answer else {
            unimplemented!("an answer that is not a card selection");
        };
        if indices.len() > 1 {
            unimplemented!("a card selection of more than one");
        }
        let chosen: Option<Vec<_>> = indices
            .iter()
            .map(|&index| cards.get(index).copied().map(CardIdent::new))
            .collect();
        let Some(chosen) = chosen else {
            unimplemented!("a card selection naming a card that was not offered");
        };

        self.player.deck.extend(chosen);
        *selection = None;
        Ok(self.report())
    }

    /// Upgrade the card the smith was pointed at.
    ///
    /// `CardCmd.Upgrade` raises the level by one, and the selection offers the
    /// whole deck, so the index is the deck's. A card already at its maximum
    /// is offered and cannot be taken.
    ///
    /// The rest site is spent once this lands: its options empty, which is how
    /// the game reports a rest site nobody can act at again.
    fn smith(&mut self, answer: &Answer) -> Result<Applied<V>, Error> {
        use sts2_core::CardTable as _;

        let Answer::Cards(indices) = answer else {
            unimplemented!("an answer to the smith that is not a card selection");
        };
        let [index] = indices[..] else {
            unimplemented!("a smith selection that is not one card");
        };
        let card = self
            .player
            .deck
            .get_mut(index)
            .unwrap_or_else(|| unimplemented!("upgrading a card the deck does not hold"));
        if card.upgrade >= card.card.max_upgrade() {
            unimplemented!("upgrading a card that is already at its highest");
        }
        card.upgrade += 1;

        let Phase::RestSite {
            options, upgrading, ..
        } = &mut self.phase
        else {
            unreachable!("the phase was a rest site when this began");
        };
        options.clear();
        *upgrading = false;
        Ok(self.report())
    }

    /// Take rewards from the screen a won fight put up.
    ///
    /// `RewardsSetSynchronizer.SelectLocalReward`, one entry at a time. A
    /// reward stays in the set once it is taken, because the indices an answer
    /// names never renumber.
    fn take(&mut self, answer: &Answer) -> Result<Applied<V>, Error> {
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
            return Ok(self.report());
        }
        let Answer::Take(taking) = answer else {
            unimplemented!("an answer the reward screen does not accept");
        };
        let slots: usize = num_traits::cast(self.player.potion_slots)
            .expect("the belt has an addressable number of slots");
        for take in taking {
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
                        .get(*card)
                        .copied()
                        .unwrap_or_else(|| unimplemented!("a card the reward does not offer"));
                    *taken = true;
                    self.player.deck.push(chosen);
                }
                (Offer::SpecialCard { card, taken }, TakeReward::NonCard { .. }) => {
                    let card = *card;
                    *taken = true;
                    self.player.deck.push(card);
                }
                (Offer::Relic { relic, taken }, TakeReward::NonCard { .. }) => {
                    let relic = *relic;
                    *taken = true;
                    self.obtain(relic);
                }
                _ => unimplemented!("a reward taken as the wrong kind"),
            }
        }

        let mut applied = self.report();
        applied.resolved = applied.asking.is_none();
        Ok(applied)
    }

    /// Derive the comparison snapshot from the run's authoritative state.
    pub(crate) fn report(&self) -> Applied<V> {
        let map = &self.map;
        let standing = self.visited.last().map(|&at| {
            map.at(at)
                .expect("the run stands on a point of its own map")
        });
        // `MapLegality.TravelableFrom`, which reads the last point visited
        // rather than a room the run is in. An act with none visited has just
        // begun, and its map opens on its own starting node.
        let reachable: Vec<Coord> = match standing {
            Some(point) => point.to.clone(),
            None => vec![
                map.ancient()
                    .expect("a generated act map opens somewhere")
                    .at,
            ],
        };
        let travelable = reachable
            .into_iter()
            .map(|at| Reachable {
                at,
                kind: map.at(at).expect("a map edge leads to a node").kind,
            })
            .collect();
        let at_boss = standing
            .is_some_and(|point| point.kind == <V::MapPointType as sts2_core::MapPointKinds>::BOSS);
        let finished = self.phase.finished();
        let (asking, room, combat, in_combat) = match &self.phase {
            Phase::Neow {
                options: neow_options,
                chosen: neow_chosen,
                selection: card_selection,
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
                let asking = card_selection.as_ref().map(|cards| Question::SelectCards {
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
                });
                let room = Room::Event {
                    event: V::neow_event(),
                    finished,
                    title: None,
                    text: None,
                    options: if finished { Vec::new() } else { options },
                };
                (asking, Some(room), None, false)
            }
            Phase::Combat(live) => {
                let asking = live.question();
                (asking, None, Some(live.report()), true)
            }
            Phase::Rewards { combat, offered } => (
                offered
                    .iter()
                    .any(|offer| !offer.taken())
                    .then(|| asked::<V>(offered)),
                None,
                Some(combat.clone()),
                // The fight is over. It is still reported, because the run has
                // not left the room it was fought in.
                false,
            ),
            // An act opens on its map, which is no room and asks nothing.
            Phase::Map => (None, None, None, false),
            Phase::Event { state, .. } => (None, Some(V::observe_event(state, self)), None, false),
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
                (
                    None,
                    Some(Room::Treasure {
                        finished: *finished,
                        options,
                    }),
                    None,
                    false,
                )
            }
            Phase::RestSite {
                options, upgrading, ..
            } => {
                // `CardSelectCmd.FromDeckForUpgrade` offers the whole deck and
                // the indices are the deck's, so an unupgradable card is
                // offered and cannot be taken. One card, and the selection
                // cannot be empty.
                let asking = upgrading.then(|| Question::SelectCards {
                    min: 1,
                    max: 1,
                    asker: None,
                    options: self
                        .player
                        .deck
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(index, card)| CardOption {
                            index,
                            card,
                            text: None,
                        })
                        .collect(),
                });
                let offered = options
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, id)| sts2_core::RestOption {
                        index: index as u8,
                        id,
                        enabled: self.rest_option_enabled(id),
                    })
                    .collect();
                (
                    asking,
                    Some(Room::RestSite { options: offered }),
                    None,
                    false,
                )
            }
        };
        // Where the player stands. A live fight holds it, because a card can
        // move it and the fight is resolved on a copy; anywhere else the run
        // does. See `Player::hp`.
        let standing: Standing<V> = match &self.phase {
            Phase::Combat(live) => {
                let player = live.player();
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
            | Phase::RestSite { .. } => Standing {
                hp: self.player.hp,
                max_hp: self.player.max_hp,
                alive: true,
            },
        };

        Applied {
            observation: Observation {
                act: num_traits::cast(self.act).expect("an act index is a game number"),
                floor: self.floor,
                hp: standing.hp,
                max_hp: standing.max_hp,
                alive: standing.alive,
                run_over: false,
                victory: None,
                gold: self.player.gold,
                relics: self
                    .player
                    .relics
                    .iter()
                    .map(|held| Carried {
                        relic: held.relic,
                        amount: held.amount(),
                        title: None,
                        text: None,
                    })
                    .collect(),
                potions: crate::combat::belt::<V>(
                    &self.player.potions,
                    // A fight that is over is not one a potion can be drunk
                    // in. `in_combat` is that question already answered.
                    combat.as_ref().filter(|_| in_combat),
                ),
                potion_slots: self.player.potion_slots,
                deck: self.player.deck.clone(),
                in_combat,
                map: Map {
                    at: self.visited.last().copied(),
                    visited: self.visited.clone(),
                    travelable,
                    points: map.shared(),
                    // `MapLegality.CanProceed`, and the fight it is standing
                    // in being over. A boss room with its combat still running
                    // is an ordinary fight, and leaving one is not a move.
                    proceed: at_boss && !in_combat,
                },
                rng: self.positions(),
                room,
                combat,
            },
            asking,
            resolved: true,
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
        let mut upgradable: Vec<usize> = deck
            .iter()
            .enumerate()
            .filter(|(_, card)| card.upgrade < card.card.max_upgrade())
            .map(|(at, _)| at)
            .collect();
        self.streams
            .run(stream)
            .stable_shuffle_by_key(&mut upgradable, |&at| {
                (deck[at].card.order(), deck[at].upgrade)
            });
        for at in upgradable.into_iter().take(count) {
            self.player.deck[at].upgrade += 1;
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
        self.obtain(relic);
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
        run.player.deck =
            std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 10).collect();

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
        run.player.deck =
            std::iter::repeat_n(CardIdent::new(Cards::Regent(RegentCard::Strike)), 20).collect();
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
