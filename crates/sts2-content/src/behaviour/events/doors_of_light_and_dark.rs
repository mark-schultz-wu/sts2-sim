//! Doors of Light and Dark: two cards upgraded, or one removed.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{DeckAct, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

/// `CardsVar(2)`.
const CARDS: usize = 2;

#[derive(Debug, Clone)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct DoorsOfLightAndDark;

impl<V: crate::Build> EventBehavior<V> for DoorsOfLightAndDark {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State { chosen: None }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::DOORS_OF_LIGHT_AND_DARK,
            &[
                (V::EventOption::DOORS_OF_LIGHT_AND_DARK_LIGHT, false),
                (V::EventOption::DOORS_OF_LIGHT_AND_DARK_DARK, false),
            ],
            state.chosen,
        )
    }

    /// `Light`: the deck's upgradable cards stably shuffled off the event's
    /// stream, and its `Cards` of them upgraded. `Dark`: one card of the
    /// deck the player picks, removed. Each finishes the event.
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
                HookEffect::UpgradeRandomDeckCards {
                    count: CARDS,
                    stream: Stream::Event,
                    kind: None,
                },
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::SelectFromDeck {
                    act: DeckAct::Remove,
                    count: 1,
                },
                HookEffect::FinishEvent,
            ],
        }
    }
}
