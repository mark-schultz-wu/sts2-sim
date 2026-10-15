//! Self-Help Book: an attack, a skill or a power enchanted twice over.

use nonmax::NonMaxU8;
use sts2_core::{
    CardKind, DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Rules as _,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;
use super::super::super::generated::events::{Event, EventOption as Said};
use crate::behaviour::enchantments::can_enchant;

/// Each option's enchantment, the card type it takes, and how much of it.
const READINGS: [(Said, Enchantment, CardKind); 3] = [
    (
        Said::SelfHelpBookReadTheBack,
        Enchantment::Sharp,
        CardKind::Attack,
    ),
    (
        Said::SelfHelpBookReadPassage,
        Enchantment::Nimble,
        CardKind::Skill,
    ),
    (
        Said::SelfHelpBookReadEntireBook,
        Enchantment::Swift,
        CardKind::Power,
    ),
];

/// `IntVar("Enchantment1Amount", 2)`, and the same for the other two.
const AMOUNT: u8 = 2;

#[derive(Debug, Clone)]
pub struct State {
    /// Whether the deck held a card each reading takes, as the book opened.
    open: [bool; 3],
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

impl State {
    /// `GenerateInitialOptions`, which offers the one way out when no
    /// reading has a card to take.
    fn nothing_to_read(&self) -> bool {
        !self.open.contains(&true)
    }
}

pub struct SelfHelpBook;

impl EventBehavior<V> for SelfHelpBook {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `PlayerHasCardsAvailable`: a deck card of the reading's type that its
    /// enchantment takes.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, _rng: &mut R) -> State {
        let open = READINGS.map(|(_, enchantment, kind)| {
            run.deck()
                .iter()
                .any(|card| V::card_kind(card.card) == Some(kind) && can_enchant(enchantment, card))
        });
        State { open, chosen: None }
    }

    /// A reading with no card to take is locked.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        if state.nothing_to_read() {
            return super::page(
                Event::SelfHelpBook,
                &[(Said::SelfHelpBookNoOptions, false)],
                state.chosen,
            );
        }
        let options = [0, 1, 2].map(|at| (READINGS[at].0, !state.open[at]));
        super::page(Event::SelfHelpBook, &options, state.chosen)
    }

    /// A reading: `FromDeckForEnchantment` over the deck cards of its type,
    /// then its enchantment twice over. The way out takes nothing. Each
    /// finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        if state.nothing_to_read() {
            return smallvec::smallvec![HookEffect::FinishEvent];
        }
        let (_, enchantment, kind) = READINGS[usize::from(option)];
        smallvec::smallvec![
            HookEffect::SelectFromDeck {
                act: DeckAct::Enchant {
                    enchantment,
                    amount: AMOUNT,
                    kind: Some(kind),
                },
                count: 1,
            },
            HookEffect::FinishEvent,
        ]
    }
}
