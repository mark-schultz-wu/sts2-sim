//! Bugslayer: Exterminate or Squash into the deck.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, EventCard};
use super::super::super::generated::events::{Event, EventOption as Said};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct Bugslayer;

impl EventBehavior<V> for Bugslayer {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::Bugslayer,
            &[
                (Said::BugslayerExtermination, false),
                (Said::BugslayerSquash, false),
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
            0 => EventCard::Exterminate,
            _ => EventCard::Squash,
        };
        smallvec::smallvec![
            HookEffect::AddToDeck(Cards::Event(card)),
            HookEffect::FinishEvent
        ]
    }
}
