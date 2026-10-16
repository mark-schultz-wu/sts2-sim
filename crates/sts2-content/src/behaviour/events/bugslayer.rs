//! Bugslayer: Exterminate or Squash into the deck.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct Bugslayer;

impl<V: crate::Build> EventBehavior<V> for Bugslayer {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::BUGSLAYER,
            &[
                (V::EventOption::BUGSLAYER_EXTERMINATION, false),
                (V::EventOption::BUGSLAYER_SQUASH, false),
            ],
            state.chosen,
        )
    }

    /// `Extermination`: an Exterminate into the deck. `Squash`: a Squash.
    /// Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let card = match option {
            0 => V::Card::EXTERMINATE,
            _ => V::Card::SQUASH,
        };
        smallvec::smallvec![HookEffect::AddToDeck(card), HookEffect::FinishEvent]
    }
}
