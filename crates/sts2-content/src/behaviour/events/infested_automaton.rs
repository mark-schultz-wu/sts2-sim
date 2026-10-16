//! Infested Automaton: a Power from the character's pool, or a card that
//! costs nothing.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{CardKind, EventBehavior, EventView, HookEffect, HookEffects, PoolFilter, Room};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct InfestedAutomaton;

impl<V: crate::Build> EventBehavior<V> for InfestedAutomaton {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::INFESTED_AUTOMATON,
            &[
                (V::EventOption::INFESTED_AUTOMATON_STUDY, false),
                (V::EventOption::INFESTED_AUTOMATON_TOUCH_CORE, false),
            ],
            state.chosen,
        )
    }

    /// `Study`: one Power from the character's pool, `ForNonCombatWithDefaultOdds`,
    /// into the deck. `TouchCore`: one card whose canonical cost is nothing
    /// and not X, the same way. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let filter = match option {
            0 => PoolFilter::OfKind(CardKind::Power),
            _ => PoolFilter::FreeFixedCost,
        };
        smallvec::smallvec![HookEffect::AddRegularCard(filter), HookEffect::FinishEvent]
    }
}
