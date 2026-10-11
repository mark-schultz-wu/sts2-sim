//! Everything true of a run at one decision point.
//!
//! This is what two implementations are compared on. The oracle derives one
//! from `RunState`. A simulator derives one from whatever it keeps instead.
//! Neither machine can be compared to the other. These snapshots can.
//!
//! The crate README says what this shape is for, and which fields stay prose.

use std::sync::Arc;

use smallvec::SmallVec;

use crate::{CardIdent, ContentVersion, Coord, Cost, MapPointKinds, ModelId};

/// Everything the simulator is compared against after a decision.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation<V: ContentVersion> {
    pub act: V::Count,
    pub floor: V::Count,
    pub hp: V::Health,
    pub max_hp: V::Health,
    pub alive: bool,
    /// The run has ended, won or lost.
    pub run_over: bool,
    /// Whether it was won, and nothing at all while it is still going.
    ///
    /// This is not a third state of `run_over`. Winning kills every player too,
    /// because `RunManager.WinRun` calls `GuaranteeKillAllPlayers`. So `alive`
    /// cannot tell a win from a loss. This field is the one that can.
    pub victory: Option<bool>,
    pub gold: V::Number,
    pub relics: Vec<Carried<V>>,
    pub potions: Vec<Held<V>>,
    /// How many potions the belt holds, which is not how many are in it.
    ///
    /// A full belt is what makes a potion refuse to be bought, and relics move
    /// the count, so `potions.len()` cannot answer it.
    pub potion_slots: V::Count,
    pub deck: Vec<CardIdent<V>>,
    pub in_combat: bool,
    pub map: Map<V>,
    pub rng: Positions<V>,
    /// The choices this room offers, for a room that offers any.
    pub room: Option<Room<V>>,
    /// The fight, while there is one.
    pub combat: Option<Combat<V>>,
    /// Every decision the run accepts, in the order a driver picks from.
    ///
    /// The game's own list, which the simulator's `legal` is held to. A
    /// pending question's answers come alone. Otherwise the belt comes first,
    /// then the room's options, then the fight's plays and its end, or the
    /// map.
    pub moves: Vec<crate::Decision<V>>,
}

impl<V: ContentVersion> Observation<V> {
    /// The fight, when there is one and the run is in it.
    ///
    /// [`Observation::in_combat`] and the presence of [`Observation::combat`]
    /// answer different questions. The block is still reported through the
    /// moment a fight is ending, and a card played into that window is refused.
    /// Ask this when you mean "is there a fight to act in".
    pub fn fight(&self) -> Option<&Combat<V>> {
        self.combat.as_ref().filter(|_| self.in_combat)
    }
}

/// Where the run is standing, and where it can go.
#[derive(Debug, Clone, PartialEq)]
pub struct Map<V: ContentVersion> {
    /// Nothing before the run has stood anywhere, and for a fight jumped into
    /// off the map.
    pub at: Option<Coord>,
    pub visited: Vec<Coord>,
    /// Every node reachable from here. Empty when the only move left is
    /// [`Map::proceed`].
    pub travelable: Vec<Reachable<V>>,
    /// The shape of the act, which does not change as it is played.
    ///
    /// Shared and not owned. An act's nodes settle when the act is generated.
    /// A report is built after every decision, so copying seventy of them each
    /// time buys nothing.
    pub points: Arc<[Node<V>]>,
    /// The one move the graph cannot express: leaving a finished act.
    ///
    /// An act's boss sits outside the grid with no children. So `travelable` is
    /// correctly empty once it is beaten, and the run has nowhere to go.
    pub proceed: bool,
}

/// A node the run can travel to.
///
/// `Reachable` and not `Step`, which in this crate is a line of a recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reachable<V: ContentVersion> {
    pub at: Coord,
    pub kind: V::MapPointType,
}

/// No node stands on a coordinate a caller asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this act's map has no node at {at}")]
pub struct NoNode {
    pub at: Coord,
}

/// What an act's map answers, whoever is holding one.
///
/// An observation carries the act's nodes, and so does whatever generated
/// them. Both get asked the same questions here.
///
/// [`ActMap::nodes`] is the whole of it. The rest are written in terms of that
/// one, for an implementation to replace. A map that decides its own order can
/// search where this walks.
pub trait ActMap<V: ContentVersion> {
    /// Every node of the act, in the order an observation lists them.
    fn nodes(&self) -> &[Node<V>];

    /// The node standing on a coordinate.
    ///
    /// A coordinate this map does not hold came off some other map, which is
    /// an error and not an empty answer.
    fn at(&self, at: Coord) -> Result<&Node<V>, NoNode> {
        self.nodes()
            .iter()
            .find(|node| node.at == at)
            .ok_or(NoNode { at })
    }

    /// Where a run stands on this map before it has moved anywhere.
    fn ancient(&self) -> Option<&Node<V>> {
        self.nodes()
            .iter()
            .find(|node| node.kind == V::MapPointType::ANCIENT)
    }
}

/// An observation's own nodes answer for the act it reported.
impl<V: ContentVersion> ActMap<V> for [Node<V>] {
    fn nodes(&self) -> &[Node<V>] {
        self
    }
}

/// A node in the act, and the nodes it leads on to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Node<V: ContentVersion> {
    pub at: Coord,
    pub kind: V::MapPointType,
    /// `Children` verbatim, which is what the game draws its lines from.
    ///
    /// Inline, because a row holds seven nodes at most, and the first node
    /// leads on to every node of the first row.
    pub to: smallvec::SmallVec<[Coord; 7]>,
}

/// Where every random stream stands.
///
/// Two implementations can agree on every visible field while standing at
/// different places in a stream. That divergence decides something several
/// moves later, so the streams are compared directly.
///
/// Two maps because the game keeps two sets: twelve per run, and three per
/// player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Positions<V: ContentVersion> {
    pub run: Keyed<V::RunRngType, Position>,
    pub player: Keyed<V::PlayerRngType, Position>,
}

/// Where one stream stands, the way the game saves it.
///
/// `SerializableRng` is `{counter, s0..s3}`, and this is that record. The
/// count says how far the stream has been drawn. The words are the generator
/// that count left behind.
///
/// Only the count is on every build. `v0.111.0` saves both, and `v0.107.1`
/// saves the count by itself. So `words` says what a build's save format
/// hands over, not what a stream has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub counter: u32,
    pub words: Option<[u64; 4]>,
}

impl Position {
    /// A stream a build reports by its draw count alone.
    pub fn counted(counter: u32) -> Position {
        Position {
            counter,
            words: None,
        }
    }
}

/// A room that asks something, and what it offers.
///
/// Not pending decisions. A rest site, an event and a shop are all set up by
/// the room itself, so the game is quiescent and idle in each. They are state
/// to be read and answered with an ordinary command.
#[derive(Debug, Clone, PartialEq)]
pub enum Room<V: ContentVersion> {
    RestSite {
        options: Vec<RestOption<V>>,
    },
    Treasure {
        finished: bool,
        options: Vec<ChestOption<V>>,
    },
    Event {
        event: V::Event,
        finished: bool,
        /// The prose, resolved through the game's own localisation. Absent
        /// where the table has no key. An option built with a title and no
        /// description points at a key nobody wrote.
        title: Option<String>,
        text: Option<String>,
        options: Vec<EventOption<V>>,
    },
    Shop {
        entries: Vec<Entry<V>>,
    },
}

/// One thing a rest site offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RestOption<V: ContentVersion> {
    pub index: u8,
    /// Which of the nine: `SMITH`, `HEAL` and seven more.
    ///
    /// Not the same question as which options this rest site has. Two rest
    /// sites offer different subsets because a relic added one, and the index
    /// beside this is a position in *this* list.
    pub id: V::RestSiteOption,
    pub enabled: bool,
}

/// One thing a chest offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChestOption<V: ContentVersion> {
    pub index: u8,
    pub take: Take<V>,
    /// Whether this chest has already been answered. That is a fact about the
    /// chest, not about this option, and every option carries the same answer.
    pub chosen: bool,
}

/// What a chest option does.
///
/// Skipping is one of the options and not a separate command. The screen offers
/// it as a button beside the relics, and leaving without choosing is not a way
/// out. `OnRoomExited` clears the session only when it was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Take<V: ContentVersion> {
    Relic(V::Relic),
    Skip,
}

/// Which option an event is offering, as the game's key names it.
///
/// The game spells this `BYRDONIS_NEST.pages.INITIAL.options.EAT`. A simulator
/// reports one, and building that sentence puts a formatter on the path of
/// every report. So the parts are carried apart.
///
/// The `pages.INITIAL.options` between them is not stored. Every key in every
/// recording has it, and a reader refuses a key without it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventOptionKey<V: ContentVersion> {
    /// The event offering it, which is the key's first segment.
    pub event: V::Event,
    /// What the option is called within the page.
    pub option: V::EventOptionId,
}

/// The last segment of an event option's key.
///
/// Each build owns this vocabulary. The wire adapter parses external names
/// into it. The simulator only constructs typed identities.
pub trait EventOptionKinds<V: ContentVersion>:
    std::fmt::Debug + Copy + Eq + std::hash::Hash
{
    fn from_relic(relic: V::Relic) -> Self;
    fn relic(self) -> Option<V::Relic>;
}

/// One thing an event offers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventOption<V: ContentVersion> {
    pub index: u8,
    /// Which option this is, which names it without saying anything about it.
    pub key: EventOptionKey<V>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub locked: bool,
    /// Closes the event rather than turning a page.
    pub proceed: bool,
    pub chosen: bool,
    /// The relic this option hands over, for the options that hand one over.
    pub relic: Option<V::Relic>,
}

/// A card on a shop's shelf, priced in both senses.
///
/// A shop entry used to carry the identity alone, which let a client name the
/// card and say nothing about it. Playing it is the reason to buy it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShopCard<V: ContentVersion> {
    pub card: CardIdent<V>,
    /// See [`Playable::cost`](crate::report::observation::Playable::cost).
    pub cost: Cost,
    pub star_cost: Cost,
}

/// One thing a shop has for sale.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry<V: ContentVersion> {
    pub index: u8,
    pub stock: Stock<V>,
    /// The price in gold. What a card on the shelf costs to *play* is in
    /// [`Stock::Card`].
    pub cost: V::Number,
    /// What the shop calls it, and what it does, in the game's own words.
    ///
    /// The same descriptions the belt and the relic panel show for things
    /// already owned. Absent where the shop generated no item for the slot.
    pub title: Option<String>,
    pub text: Option<String>,
    pub stocked: bool,
    pub affordable: bool,
    /// Why it cannot be bought, beyond stock and gold.
    pub blocked: Option<Blocked>,
}

/// What a shop entry sells.
///
/// The thing sold is absent where the shop has not generated it. A card entry
/// holds a `CreationResult`, which does not always produce one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stock<V: ContentVersion> {
    /// The card and what it costs to play, which is a different question from
    /// what it costs to buy.
    Card(Option<ShopCard<V>>),
    Relic(Option<V::Relic>),
    Potion(Option<V::Potion>),
    /// The one entry that sells a service rather than a thing.
    CardRemoval,
}

/// The knowable reasons a stocked, affordable entry still refuses.
///
/// One today. `Hook.ShouldProcurePotion` is the other way
/// `PotionCmd.TryToProcure` fails, and it cannot be answered without running
/// the hook. So it is not reported, and the game corrects a caller that trusts
/// this further.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Blocked {
    /// A potion, with a full belt.
    NoPotionSlot,
}

/// The fight, while there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct Combat<V: ContentVersion> {
    pub turn: V::Count,
    pub phase: V::PlayerTurnPhase,
    pub energy: V::Count,
    pub stars: V::Count,
    pub round: V::Count,
    /// Which fight this is, which nothing else says: two encounters can field
    /// the same monsters at different counts. Absent for a combat with no
    /// encounter behind it, which `CombatState` allows.
    pub encounter: Option<V::Encounter>,
    pub hand: Vec<CardIdent<V>>,
    pub draw: Vec<CardIdent<V>>,
    pub discard: Vec<CardIdent<V>>,
    pub exhaust: Vec<CardIdent<V>>,
    pub creatures: Vec<Creature<V>>,
    /// Every card in hand, with the reason each unplayable one is unplayable.
    pub legal: Vec<Playable<V>>,
    /// Absent when the subsystem is inert, meaning no capacity and nothing
    /// channelled, rather than reported as an empty queue.
    pub orbs: Option<Orbs<V>>,
}

/// The orb queue, which is a queue: the same orbs in another order evoke
/// differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orbs<V: ContentVersion> {
    /// Separate from the count, because empty slots are part of the state.
    pub capacity: V::Count,
    pub queue: Vec<Channelled<V>>,
}

impl<V: ContentVersion> Orbs<V> {
    /// The empty slots, which channelling fills before it evokes.
    ///
    /// A queue longer than the capacity is not a state to render around. It is
    /// two halves of one observation contradicting each other, so whoever built
    /// this value is wrong. Everything that builds one rejects that pair before
    /// it gets here.
    pub fn free(&self) -> usize {
        let capacity = num_traits::ToPrimitive::to_usize(&self.capacity)
            .expect("orb capacity fits an addressable list");
        capacity
            .checked_sub(self.queue.len())
            .expect("more orbs channelled than the queue holds")
    }
}

/// One orb in a slot, with the numbers it currently has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Channelled<V: ContentVersion> {
    pub orb: V::Orb,
    pub passive: V::Number,
    pub evoke: V::Number,
}

/// One creature in a fight, as the game's `CombatId` names one.
///
/// A card's commands are pointed at one of these, and [`crate::Fight::target`]
/// is where a rule gets one. Rules code compares two of them and reads neither
/// out. The number is for the edges that spell an id or read one.
///
/// A byte, because the roster bounds it: the player is `0` and the monsters
/// are `index + 1`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct CombatId(nonmax::NonMaxU8);

impl std::fmt::Display for CombatId {
    /// The number the game spells, for a front end saying which creature.
    ///
    /// Reading one out as words is not a way back to one: nothing parses this.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CombatId {
    /// The creature this number names.
    pub fn new(id: nonmax::NonMaxU8) -> Self {
        Self(id)
    }

    /// The creature this number names, or [`OutOfRange`](crate::OutOfRange) for one no fight
    /// assigns.
    pub fn try_new(id: u8) -> Result<Self, crate::OutOfRange> {
        nonmax::NonMaxU8::new(id).map(Self).ok_or(crate::OutOfRange)
    }

    /// The number the game spells.
    pub fn get(self) -> u8 {
        self.0.get()
    }
}

/// One creature in a fight.
#[derive(Debug, Clone, PartialEq)]
pub struct Creature<V: ContentVersion> {
    /// Stable for as long as the fight lasts, which an index into the living
    /// creatures is not.
    pub id: CombatId,
    /// Absent for a creature that is neither a monster nor a player.
    pub model: Option<ModelId<V>>,
    /// Not the same question as whether it is a monster: a charmed monster
    /// fights on the player's side and is still a monster.
    pub side: V::CombatSide,
    pub hp: V::Health,
    pub max_hp: V::Health,
    pub block: V::Number,
    pub alive: bool,
    pub powers: Vec<Stack<V>>,
    /// What it will do next, for a creature that is a monster.
    ///
    /// The game ships intent nowhere. `NetFullCombatState` omits it because
    /// peers re-derive it. A simulator picking the wrong move is exactly what
    /// this catches, so the oracle reports it.
    pub next_move: Option<NextMove<V>>,
}

/// One power on a creature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stack<V: ContentVersion> {
    pub power: V::Power,
    pub amount: crate::PowerAmount<V::Number>,
}

/// The move a monster has chosen, and what it amounts to.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct NextMove<V: ContentVersion> {
    /// Which move, by the name the game and the observation both spell it with.
    ///
    /// The name alone, which is the precision the wire has. It is not an
    /// identity on its own. Six monsters have a move called `NOTHING_MOVE`, and
    /// they are six different states. But the creature carrying this names the
    /// monster, and the pair is exact.
    pub id: V::MoveId,
    /// A move carries a list, so an attack that also blocks reports two, in the
    /// order the game draws them.
    ///
    /// Inline, because three is the longest any move draws, and a fight
    /// carries one of these per monster.
    pub intents: SmallVec<[Intent<V>; INTENTS_INLINE]>,
}

/// How many intents one move telegraphs, before reaching the heap.
const INTENTS_INLINE: usize = 3;

impl<V: ContentVersion> Clone for NextMove<V> {
    /// Copies the intents rather than walking them.
    ///
    /// `SmallVec` clones a list one item at a time, because its `Clone` cannot
    /// see that the item is `Copy`. An [`Intent`] is.
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            intents: SmallVec::from_slice(&self.intents),
        }
    }
}

/// One thing a move telegraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Intent<V: ContentVersion> {
    pub kind: V::IntentKind,
    /// The numbers, for an attack.
    ///
    /// Two independent facts and not a sum. Only an `AttackIntent` carries
    /// numbers, and the oracle also leaves them out for one whose creature had
    /// no combat state to compute them against. `DefendIntent` has no block
    /// field at all, and neither do the other twelve. The game draws a shield
    /// with no number for the same reason.
    pub attack: Option<Attack<V>>,
}

/// What an attack will land.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Attack<V: ContentVersion> {
    /// After `Hook.ModifyDamage`, so this is the number that will land rather
    /// than the move's base value.
    pub damage: V::Number,
    /// How many times it lands, which is `WithHitCount`. A count and not a
    /// number: an attack lands a whole number of times, and never fewer than
    /// none.
    pub repeats: V::Count,
}

/// One relic the player is carrying, and what it says.
///
/// Deliberately not `Hash`. [`crate::Digest`] walks this field by field, and a
/// derived `Hash` lets its `id` swallow the whole struct and skip the walk.
///
/// The prose is here rather than in a table because it is not static.
/// `RelicModel.DynamicDescription` formats a live count in, so two relics with
/// the same id read differently in one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried<V: ContentVersion> {
    pub relic: V::Relic,
    /// The number it shows, which is `RelicModel.DisplayAmount`.
    ///
    /// A relic that counts something a rule reads counts it here: Bone Tea's
    /// combats left, Pocketwatch's cards played this turn. A relic that counts
    /// nothing reports zero, which is what the game reports for it.
    pub amount: V::Number,
    /// Its name, as the game shows it. Absent where the localisation table has
    /// no key. That is a fault, and it says so by being absent rather than by
    /// falling back to the id.
    pub title: Option<String>,
    /// What it does, with this relic's own numbers in it, carrying the game's
    /// own markup.
    pub text: Option<String>,
}

/// One potion in the belt, and whether it can be drunk.
///
/// Deliberately the same shape as [`Playable`]: a reason that is `None` exactly
/// when it can be drunk, and the creatures it takes as a target. A front end
/// reads those two fields and applies no rule of its own. An implementation
/// that gets drinkability wrong is a divergence, not a screen bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held<V: ContentVersion> {
    pub potion: V::Potion,
    /// Its name, as the game shows it. Absent for the same reason a relic's is.
    pub title: Option<String>,
    pub text: Option<String>,
    /// Why it cannot be drunk, or nothing for one that can.
    pub reason: Option<Undrinkable>,
    /// The creatures it can legally be pointed at.
    ///
    /// Empty is not "it cannot be drunk". A potion that takes no target has
    /// none, and so does one drunk outside combat, where there are no combat
    /// ids to name. Both are drunk with no target at all.
    pub targets: Targets,
}

/// Why a potion cannot be drunk right now.
///
/// Not a generated enum, because the game has none. `NPotionPopup` decides by
/// enabling and disabling a button, and the conditions live in `RefreshButtons`
/// and the switch above it. This is that decision written down, in the order the
/// game applies it.
///
/// Serialised as the variant's own name, which is how the mod spells it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Undrinkable {
    /// The player is dead. Nothing in the belt is drinkable then.
    Dead,
    /// Already on its way. `PotionModel.IsQueued`, which the game sets the
    /// moment a use is enqueued and clears if it is cancelled.
    Queued,
    /// Drunk by the game itself and never by hand: `PotionUsage.Automatic`.
    Automatic,
    /// Combat only, and there is no combat.
    OutOfCombat,
    /// There is a combat, but it is not yours to act in. The other side is
    /// taking its turn, a card select screen is up, or the game has actions
    /// disabled while something resolves.
    NotYourTurn,
    /// Something stops potions being used or removed. Distinct from the rest
    /// because it also stops the potion being thrown away.
    Stuck,
    /// The potion's own `PassesCustomUsabilityCheck` says no.
    ItsOwnCheck,
}

/// One card in hand, and whether it can be played.
#[derive(Debug, Clone, PartialEq)]
pub struct Playable<V: ContentVersion> {
    pub card: CardIdent<V>,
    /// What playing it costs now, with every modifier already applied.
    ///
    /// [`Cost::None`] rather than a number for a card not priced in that pool
    /// at all. Every card outside the Regent's has no star cost, and a curse
    /// has no energy cost. The game spells both `-1`. The wire is where that
    /// stops.
    pub cost: Cost,
    pub star_cost: Cost,
    pub target: V::TargetType,
    /// Every rule this card fails, or nothing for a card that can be played.
    ///
    /// `None` is the whole of "playable", because `CardModel.CanPlay` returns
    /// `reason == None`. So there is no second field saying the same thing.
    /// The set is never empty when it is present.
    pub reason: Option<V::UnplayableReasons>,
    /// Which model said no, for the rule that is one model saying no.
    pub preventer: Option<ModelId<V>>,
    /// The creatures this card can legally be pointed at.
    pub targets: Targets,
    /// What it does, with everything applied.
    ///
    /// On the heap, although 99 of every 100 hold a single preview. A
    /// [`Preview`] is 128 bytes, so inlining one grows a `Playable` from 72
    /// to 184. Moving that through the hand costs more than the allocation it
    /// saves, and measured 6% slower to build a report.
    pub effect: Vec<Preview<V>>,
}

/// The creatures one card or potion can be pointed at.
///
/// Inline to eight, which is free. A `SmallVec` unions its inline array with
/// the pointer and length it otherwise holds. So up to eight one-byte ids
/// take the same 24 bytes as a `Vec`, and sixteen take 32.
///
/// The widest fight in the recordings puts five creatures on the board.
pub type Targets = SmallVec<[CombatId; TARGETS_INLINE]>;

/// How many creatures a card can name without reaching the heap.
const TARGETS_INLINE: usize = 8;

/// What a card does right now, against one target.
///
/// A card's numbers are not static data. Strike's 6 is 9 with Strength and 13
/// against a Vulnerable target, and none of that is a property of the card.
///
/// One entry per distinct outcome. A targeted attack is previewed against each
/// creature it can hit. Identical previews collapse to one with no target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview<V: ContentVersion> {
    /// Which creature this was computed against, or nothing for a preview that
    /// does not vary by target.
    pub target: Option<CombatId>,
    /// The rules text with these numbers in it, carrying the game's own
    /// markup. Nothing at all for an implementation with no localisation
    /// tables, which is not the same as a card that says nothing.
    pub text: Option<String>,
    /// The card's variables, as they resolve right now.
    ///
    /// Keyed by the build's own variable, not by its name. A key outside the
    /// set this build prints is a divergence the reader reports, and no caller
    /// spells one to look it up.
    pub vars: Keyed<V::CardVar, V::Number>,
    /// The same variables as the card prints them, upgrades and enchantment
    /// included. Equal to `vars` exactly when nothing is modifying the card.
    pub base: Keyed<V::CardVar, V::Number>,
}

/// A handful of values, by a key the build names.
///
/// Sorted by key with no repeats, the way a `BTreeMap` of them read. Repeated
/// keys keep the last value, as that map did.
///
/// Every map in an observation is this shape: a preview's numbers, and where
/// each stream stands. A report builds a fresh one per decision, and hashing
/// four entries costs more than walking them.
#[derive(PartialEq, Eq)]
pub struct Keyed<K, N>(SmallVec<[(K, N); KEYED_INLINE]>);

impl<K: Copy, N: Copy> Clone for Keyed<K, N> {
    /// Copies the entries rather than walking them.
    ///
    /// `SmallVec` clones a list one item at a time, because its `Clone` cannot
    /// see that the item is `Copy`. Every key and value in here is, and a
    /// report clones these on the way out.
    fn clone(&self) -> Self {
        Self(SmallVec::from_slice(&self.0))
    }
}

/// How many entries a map holds without reaching the heap.
///
/// Four, which holds the numbers all but four of the ported build's cards
/// print, and the streams the game keeps per player. A run's twelve streams
/// spill, into one allocation that [`Keyed::from_iter`] sizes up front.
const KEYED_INLINE: usize = 4;

impl<K: Ord, N> Keyed<K, N> {
    /// The value filed under `key`.
    pub fn get(&self, key: &K) -> Option<&N> {
        self.0
            .binary_search_by(|(at, _)| at.cmp(key))
            .ok()
            .map(|at| &self.0[at].1)
    }

    /// The value filed under `key`, to write on.
    pub fn get_mut(&mut self, key: &K) -> Option<&mut N> {
        self.0
            .binary_search_by(|(at, _)| at.cmp(key))
            .ok()
            .map(|at| &mut self.0[at].1)
    }

    /// Files `value` under `key`, and hands back what was there.
    pub fn insert(&mut self, key: K, value: N) -> Option<N> {
        match self.0.binary_search_by(|(at, _)| at.cmp(&key)) {
            Ok(at) => Some(std::mem::replace(&mut self.0[at].1, value)),
            Err(at) => {
                self.0.insert(at, (key, value));
                None
            }
        }
    }
}

/// Reading one, which is every use but building it and looking one up. No
/// ordering bound, because the order was settled when it was built.
impl<K, N> Keyed<K, N> {
    /// A map with nothing in it.
    pub fn new() -> Self {
        Self(SmallVec::new())
    }

    /// Every pair, in key order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&K, &N)> {
        self.0.iter().map(|(key, value)| (key, value))
    }

    /// Every key, in order.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.0.iter().map(|(key, _)| key)
    }

    /// Every value, in key order.
    pub fn values(&self) -> impl Iterator<Item = &N> {
        self.0.iter().map(|(_, value)| value)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<K: Ord, N> FromIterator<(K, N)> for Keyed<K, N> {
    fn from_iter<I: IntoIterator<Item = (K, N)>>(pairs: I) -> Self {
        let pairs = pairs.into_iter();
        // One allocation for a map that spills, rather than the doubling a
        // sequence of inserts would do.
        let mut held: SmallVec<[(K, N); KEYED_INLINE]> =
            SmallVec::with_capacity(pairs.size_hint().0);
        for (key, value) in pairs {
            match held.binary_search_by(|(at, _)| at.cmp(&key)) {
                Ok(at) => held[at].1 = value,
                Err(at) => held.insert(at, (key, value)),
            }
        }
        Self(held)
    }
}

impl<K, N> Default for Keyed<K, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Ord, N, const COUNT: usize> From<[(K, N); COUNT]> for Keyed<K, N> {
    fn from(pairs: [(K, N); COUNT]) -> Self {
        pairs.into_iter().collect()
    }
}

/// So that a failure prints what the map it replaced printed.
impl<K: std::fmt::Debug, N: std::fmt::Debug> std::fmt::Debug for Keyed<K, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.0.iter().map(|(key, value)| (key, value)))
            .finish()
    }
}

/// A key a card does not print is a mistake in the caller, the way indexing a
/// map with one was.
impl<K: Ord, N> std::ops::Index<&K> for Keyed<K, N> {
    type Output = N;

    fn index(&self, key: &K) -> &N {
        self.get(key).expect("the card prints this number")
    }
}

#[cfg(test)]
mod tests {
    use super::Keyed;

    #[test]
    fn the_numbers_read_back_in_key_order() {
        let vars = Keyed::from([(2, "two"), (0, "zero"), (1, "one")]);
        let read: Vec<_> = vars.iter().map(|(key, value)| (*key, *value)).collect();
        assert_eq!(read, [(0, "zero"), (1, "one"), (2, "two")]);
    }

    #[test]
    fn a_repeated_key_keeps_the_last_value() {
        let vars = Keyed::from([(0, "first"), (1, "one"), (0, "last")]);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars.get(&0), Some(&"last"));
    }

    #[test]
    fn writing_over_a_key_hands_back_what_was_there() {
        let mut vars = Keyed::from([(0, "zero")]);
        assert_eq!(vars.insert(0, "written"), Some("zero"));
        assert_eq!(vars.insert(1, "one"), None);
        assert_eq!(vars[&0], "written");
        assert_eq!(vars.get(&2), None);
    }

    /// The `BTreeMap` this replaced printed its entries this way, and a
    /// divergence is read as printed.
    #[test]
    fn it_prints_as_a_map() {
        assert_eq!(
            format!("{:?}", Keyed::from([(0, 6), (1, 9)])),
            "{0: 6, 1: 9}"
        );
    }
}
