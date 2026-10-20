//! Field of Man-Sized Holes: two cards out for a Normality, or one card
//! enchanted with Perfect Fit.

use crate::ids::{CardIds as _, EnchantmentIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `CardsVar(2)`.
const REMOVED: usize = 2;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct FieldOfManSizedHoles;

impl<V: crate::Build> EventBehavior<V> for FieldOfManSizedHoles {
    type State = State;

    /// Whether Perfect Fit takes any deck card, which the shared eligibility
    /// check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::FIELD_OF_MAN_SIZED_HOLES,
            &[
                (V::EventOption::FIELD_OF_MAN_SIZED_HOLES_RESIST, false),
                (
                    V::EventOption::FIELD_OF_MAN_SIZED_HOLES_ENTER_YOUR_HOLE,
                    false,
                ),
            ],
            state.chosen,
        )
    }

    /// `Resist`: `FromDeckForRemoval` for two, then a Normality into the
    /// deck. `EnterYourHole`: `FromDeckForEnchantment` for one, then Perfect
    /// Fit. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Remove,
                    count: REMOVED,
                },
                HookEffect::AddToDeck(V::Card::NORMALITY),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Enchant {
                        enchantment: V::Enchantment::PERFECT_FIT,
                        amount: 1,
                        kind: None,
                    },
                    count: 1,
                },
                HookEffect::FinishEvent,
            ],
        }
    }
}
