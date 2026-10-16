//! Self-Help Book: an attack, a skill or a power enchanted twice over.

use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

use crate::behaviour::enchantments::can_enchant;
use crate::ids::{EventIds as _, EventOptionIds as _};

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

impl<V: crate::Build> EventBehavior<V> for SelfHelpBook {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `PlayerHasCardsAvailable`: a deck card of the reading's type that its
    /// enchantment takes.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, _rng: &mut R) -> State {
        let open = std::array::from_fn(|at| {
            let (_, enchantment, kind) = V::SELF_HELP_BOOK_READINGS[at];
            run.deck()
                .iter()
                .any(|card| V::card_kind(card.card) == Some(kind) && can_enchant(enchantment, card))
        });
        State { open, chosen: None }
    }

    /// A reading with no card to take is locked.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        if state.nothing_to_read() {
            return crate::behaviour::events::page::<V>(
                V::Event::SELF_HELP_BOOK,
                &[(V::EventOption::SELF_HELP_BOOK_NO_OPTIONS, false)],
                state.chosen,
            );
        }
        let options = [0, 1, 2].map(|at| (V::SELF_HELP_BOOK_READINGS[at].0, !state.open[at]));
        crate::behaviour::events::page::<V>(V::Event::SELF_HELP_BOOK, &options, state.chosen)
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
        let (_, enchantment, kind) = V::SELF_HELP_BOOK_READINGS[usize::from(option)];
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

/// `READINGS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// Each option's enchantment, the card type it takes, and how much of it.
        const SELF_HELP_BOOK_READINGS: &'static [(
            <$build as sts2_core::ContentVersion>::EventOption,
            <$build as sts2_core::ContentVersion>::Enchantment,
            sts2_core::CardKind,
        )] = &[
            (
                <$build as sts2_core::ContentVersion>::EventOption::SELF_HELP_BOOK_READ_THE_BACK,
                <$build as sts2_core::ContentVersion>::Enchantment::SHARP,
                sts2_core::CardKind::Attack,
            ),
            (
                <$build as sts2_core::ContentVersion>::EventOption::SELF_HELP_BOOK_READ_PASSAGE,
                <$build as sts2_core::ContentVersion>::Enchantment::NIMBLE,
                sts2_core::CardKind::Skill,
            ),
            (
                <$build as sts2_core::ContentVersion>::EventOption::SELF_HELP_BOOK_READ_ENTIRE_BOOK,
                <$build as sts2_core::ContentVersion>::Enchantment::SWIFT,
                sts2_core::CardKind::Power,
            ),
        ];
    };
}

pub(crate) use lists;
