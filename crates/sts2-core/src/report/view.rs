//! What a player can see of a run.
//!
//! [`Observation`] is built to catch two implementations disagreeing. So it
//! carries state the game keeps off the screen. A [`View`] is the part of a
//! [`Snapshot`] the game's own screens show. It is a type of its own, so a
//! hidden fact has nowhere to be carried. A front end holds a view, and never
//! the observation.
//!
//! What it leaves out, and why the game leaves it out too:
//!
//! - The random streams. Nothing on screen shows them.
//! - The draw pile's order. `NCardPileScreen` sorts the pile by rarity, then
//!   by id, and the view does too.
//! - A monster's move id. The screen draws the move's intents, not its name.
//! - The encounter. The screen shows the monsters, not the entry behind them.

use smallvec::SmallVec;

use crate::{
    CardIdent, CardTable, Carried, Combat, CombatId, ContentVersion, Creature, Decision, Held,
    Intent, Map, ModelId, Observation, Orbs, Playable, Question, Room, Snapshot, Stack,
};

/// Everything a player can see at one decision point.
#[derive(Debug, Clone, PartialEq)]
pub struct View<V: ContentVersion> {
    pub act: V::Count,
    pub floor: V::Count,
    pub hp: V::Health,
    pub max_hp: V::Health,
    pub alive: bool,
    /// The run has ended, won or lost.
    pub run_over: bool,
    /// Whether it was won, and nothing while it is still going.
    pub victory: Option<bool>,
    pub gold: V::Number,
    pub relics: Vec<Carried<V>>,
    pub potions: Vec<Held<V>>,
    /// How many potions the belt holds, which is not how many are in it.
    pub potion_slots: V::Count,
    pub deck: Vec<CardIdent<V>>,
    pub in_combat: bool,
    pub map: Map<V>,
    /// The choices this room offers, for a room that offers any.
    pub room: Option<Room<V>>,
    /// The fight, while there is one.
    pub combat: Option<SeenFight<V>>,
    /// Every decision the run accepts, in the order a driver picks from.
    pub moves: Vec<Decision<V>>,
    /// The question the game stopped to ask, for a decision that parked it.
    pub asking: Option<Question<V>>,
}

impl<V: ContentVersion> View<V> {
    /// The fight, when there is one and the run is in it. See
    /// [`Observation::fight`].
    pub fn fight(&self) -> Option<&SeenFight<V>> {
        self.combat.as_ref().filter(|_| self.in_combat)
    }
}

/// The fight, as the screen shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenFight<V: ContentVersion> {
    pub turn: V::Count,
    pub phase: V::PlayerTurnPhase,
    pub energy: V::Count,
    pub stars: V::Count,
    pub round: V::Count,
    pub hand: Vec<CardIdent<V>>,
    /// The draw pile's cards, in the order the game's pile screen lists them:
    /// by rarity, then by id. Never the order they will be drawn in.
    pub draw: Vec<CardIdent<V>>,
    pub discard: Vec<CardIdent<V>>,
    pub exhaust: Vec<CardIdent<V>>,
    pub creatures: Vec<SeenCreature<V>>,
    /// Every card in hand, with the reason each unplayable one is unplayable.
    pub legal: Vec<Playable<V>>,
    pub orbs: Option<Orbs<V>>,
}

/// One creature in a fight, as the screen shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenCreature<V: ContentVersion> {
    pub id: CombatId,
    pub model: Option<ModelId<V>>,
    pub side: V::CombatSide,
    pub hp: V::Health,
    pub max_hp: V::Health,
    pub block: V::Number,
    pub alive: bool,
    pub powers: Vec<Stack<V>>,
    /// What its next move telegraphs, for a monster. Empty for a creature
    /// with no move chosen.
    pub intents: SmallVec<[Intent<V>; 3]>,
}

impl<V: ContentVersion> Snapshot<V> {
    /// What a player standing here can see.
    pub fn view(&self) -> View<V> {
        View::of(&self.observation, self.asking.as_ref())
    }
}

impl<V: ContentVersion> View<V> {
    /// What a player can see of an observation and the question beside it.
    pub fn of(observed: &Observation<V>, asking: Option<&Question<V>>) -> View<V> {
        View {
            act: observed.act,
            floor: observed.floor,
            hp: observed.hp,
            max_hp: observed.max_hp,
            alive: observed.alive,
            run_over: observed.run_over,
            victory: observed.victory,
            gold: observed.gold,
            relics: observed.relics.clone(),
            potions: observed.potions.clone(),
            potion_slots: observed.potion_slots,
            deck: observed.deck.clone(),
            in_combat: observed.in_combat,
            map: observed.map.clone(),
            room: observed.room.clone(),
            combat: observed.combat.as_ref().map(SeenFight::of),
            moves: observed.moves.clone(),
            asking: asking.cloned().map(as_the_screen_asks),
        }
    }
}

impl<V: ContentVersion> SeenFight<V> {
    fn of(combat: &Combat<V>) -> SeenFight<V> {
        SeenFight {
            turn: combat.turn,
            phase: combat.phase,
            energy: combat.energy,
            stars: combat.stars,
            round: combat.round,
            hand: combat.hand.clone(),
            draw: as_the_pile_screen_lists(&combat.draw),
            discard: combat.discard.clone(),
            exhaust: combat.exhaust.clone(),
            creatures: combat.creatures.iter().map(SeenCreature::of).collect(),
            legal: combat.legal.clone(),
            orbs: combat.orbs.clone(),
        }
    }
}

impl<V: ContentVersion> SeenCreature<V> {
    fn of(creature: &Creature<V>) -> SeenCreature<V> {
        SeenCreature {
            id: creature.id,
            model: creature.model,
            side: creature.side,
            hp: creature.hp,
            max_hp: creature.max_hp,
            block: creature.block,
            alive: creature.alive,
            powers: creature.powers.clone(),
            intents: (creature.next_move.as_ref())
                .map(|next| SmallVec::from_slice(&next.intents))
                .unwrap_or_default(),
        }
    }
}

/// A question as the game's screen shows it: a choice out of the draw pile
/// sorted as `NCombatPileCardSelectScreen` sorts one, by rarity and then id.
/// Each option keeps its own index, which is what an answer names.
fn as_the_screen_asks<V: ContentVersion>(mut question: Question<V>) -> Question<V> {
    if let Question::SelectCards {
        options,
        pile: Some(crate::Pile::Draw),
        ..
    } = &mut question
    {
        options.sort_by_cached_key(|option| {
            let card = option.card;
            (
                card.card.rarity(),
                card.card.order(),
                card.upgrade,
                format!("{card:?}"),
            )
        });
    }
    question
}

/// A pile in the order `NCardPileScreen` shows the draw pile: by rarity, then
/// by id, which [`CardTable::order`] ranks as `ModelId.CompareTo` does.
///
/// The game's sort leaves two cards with the same rarity and id in an order
/// that follows the pile's. Here they are ordered by everything else about
/// them, so the result depends on which cards are in the pile and not on where.
fn as_the_pile_screen_lists<V: ContentVersion>(pile: &[CardIdent<V>]) -> Vec<CardIdent<V>> {
    let mut keyed: Vec<_> = (pile.iter())
        .map(|card| {
            let rest = format!("{card:?}");
            (
                (card.card.rarity(), card.card.order(), card.upgrade, rest),
                *card,
            )
        })
        .collect();
    keyed.sort_by(|(a, _), (b, _)| a.cmp(b));
    keyed.into_iter().map(|(_, card)| card).collect()
}
