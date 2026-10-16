//! Byrdonis Nest: Max HP, or the egg.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `MaxHpVar(7)`.
const MAX_HP: i32 = 7;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct ByrdonisNest;

impl<V: crate::Build> EventBehavior<V> for ByrdonisNest {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        !crate::behaviour::events::has_event_pet::<V>(run)
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::BYRDONIS_NEST,
            &[
                (V::EventOption::BYRDONIS_NEST_EAT, false),
                (V::EventOption::BYRDONIS_NEST_TAKE, false),
            ],
            state.chosen,
        )
    }

    /// `Eat`: `CreatureCmd.GainMaxHp`. `Take`: Byrdonis Egg into the deck.
    /// Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![HookEffect::GainMaxHp(MAX_HP), HookEffect::FinishEvent],
            _ => smallvec::smallvec![
                HookEffect::AddToDeck(V::Card::BYRDONIS_EGG),
                HookEffect::FinishEvent,
            ],
        }
    }
}
